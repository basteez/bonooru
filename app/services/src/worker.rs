//! The serial worker (Story 4.4 AC 1, 4–8; AD-13, AD-16): one thread owns
//! the port and the `ocb_core::link::Session`. The UI sends
//! [`WorkerCommand`]s and receives [`TimedEvent`]s and `Arc<LinkSnapshot>`s
//! over `crossbeam-channel`; nothing mutable is shared.
//!
//! [`WorkerCore`] is the steppable state machine: one [`WorkerCore::step`]
//! drains the commands, scans every [`SCAN_PERIOD_US`], picks and opens a
//! port, and runs one pass of the session. The thread of [`spawn`] only
//! loops over `step`; tests step it on a fake clock.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub use crossbeam_channel::TryRecvError;
use crossbeam_channel::{Receiver, Sender, TrySendError, bounded};
use ocb_core::device::{self, Class, PortInfo, Rules};
use ocb_core::link::AbandonReason;
use ocb_core::link::{LinkEvent, Phase, Session, SessionConfig, StatusView};
use ocb_core::protocol::consts::MSG_HELLO;
use ocb_core::protocol::{Info, NakReason, RxSide, decode_frame};

use crate::log::LogHandle;
use crate::port::{Backend, OpenError, OpenPort, PortLost, permission_message};

/// Ports are listed this often, idle or connected (AC 4), µs.
pub const SCAN_PERIOD_US: u64 = 1_000_000;
/// A listed candidate whose open failed is retried this often (AC 6), µs.
pub const OPEN_RETRY_US: u64 = 250_000;
/// With several `Unconfirmed` ports and no `Ours`, each gets this long
/// without INFO before the next is tried (AC 4), µs.
pub const PROBE_WINDOW_US: u64 = 2_000_000;
/// Events waiting for the UI; more are dropped and counted.
pub const EVENTS_CAP: usize = 1024;
/// Scans kept in [`LinkSnapshot::recent_scans`].
pub const RECENT_SCANS: usize = 64;
/// The thread's sleep per pass while no port is open (no busy loop).
pub const IDLE_SLEEP: Duration = Duration::from_millis(10);

/// A monotonic clock in µs.
pub trait Clock: Send {
    /// µs since the clock's start.
    fn now_us(&self) -> u64;
}

/// The real clock: an `Instant` taken at creation.
#[derive(Debug, Clone, Copy)]
pub struct RealClock(Instant);

impl RealClock {
    /// A clock starting now.
    #[must_use]
    pub fn new() -> Self {
        Self(Instant::now())
    }
}

impl Default for RealClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for RealClock {
    fn now_us(&self) -> u64 {
        u64::try_from(self.0.elapsed().as_micros()).unwrap_or(u64::MAX)
    }
}

/// A clock that moves only when told to (tests, dry runs). Clones share
/// the time.
#[derive(Debug, Clone, Default)]
pub struct FakeClock(Arc<AtomicU64>);

impl FakeClock {
    /// A clock at `t_us`.
    #[must_use]
    pub fn at(t_us: u64) -> Self {
        Self(Arc::new(AtomicU64::new(t_us)))
    }

    /// Sets the time.
    pub fn set(&self, t_us: u64) {
        self.0.store(t_us, Ordering::SeqCst);
    }

    /// Moves the time forward.
    pub fn advance(&self, d_us: u64) {
        self.0.fetch_add(d_us, Ordering::SeqCst);
    }
}

impl Clock for FakeClock {
    fn now_us(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

/// The discovery rules of the platform this was built for.
#[must_use]
pub const fn host_rules() -> Rules {
    if cfg!(target_os = "macos") {
        Rules::MACOS
    } else if cfg!(windows) {
        Rules::WINDOWS
    } else {
        Rules::LINUX
    }
}

/// What the UI asks of the worker. Later stories add `SetState`, `Stop`,
/// the 1200-baud touch and the post-flash checks (AD-16).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum WorkerCommand {
    /// Close the port and end the thread.
    Shutdown,
}

/// Something the UI (or the probe) should know. Scans are not events (one
/// per second forever would crowd the bounded queue and the UI history):
/// their timing is in [`LinkSnapshot::recent_scans`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerEvent {
    /// A port appeared in the listing.
    PortListed {
        /// Its name.
        port: String,
        /// Its class.
        class: Class,
        /// Its USB serial, if any.
        serial: Option<String>,
    },
    /// A port left the listing.
    PortVanished {
        /// Its name.
        port: String,
    },
    /// A candidate was opened (DTR asserted, input cleared, session open).
    PortOpened {
        /// Its name.
        port: String,
        /// Its class.
        class: Class,
        /// Its USB serial, if any.
        serial: Option<String>,
    },
    /// The worker closed the port itself (probe window over, shutdown).
    PortClosed {
        /// Its name.
        port: String,
    },
    /// The port failed (read/write error, or it left the listing).
    PortLost {
        /// Its name.
        port: String,
        /// Why.
        reason: String,
        /// When the last byte was received (worker clock), if any.
        last_rx_us: Option<u64>,
    },
    /// The last connected bridge (same serial) came back under another name.
    PortRenamed {
        /// The old name.
        from: String,
        /// The new name.
        to: String,
    },
    /// A `2E8A:000A` port with another product string; never opened.
    ForeignBoard {
        /// Its name.
        port: String,
        /// Its product string.
        product: String,
    },
    /// The OS refused to open the port (Linux serial group, UX-DR65).
    PermissionDenied {
        /// Its name.
        port: String,
        /// The group owning the node.
        group: String,
        /// The message for the owner.
        message: String,
    },
    /// Another program holds the port.
    PortBusy {
        /// Its name.
        port: String,
    },
    /// A session event (STATUS is in the snapshot, not here).
    Link(LinkEvent),
    /// The log cannot be written; the app keeps running.
    LogUnavailable {
        /// Why.
        reason: String,
    },
}

/// A [`WorkerEvent`] with the worker-clock time it happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimedEvent {
    /// µs on the worker clock.
    pub at_us: u64,
    /// The event.
    pub event: WorkerEvent,
}

/// A port of the last listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeenPort {
    /// As listed.
    pub info: PortInfo,
    /// Its class.
    pub class: Class,
}

/// The port the worker is on (or trying).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chosen {
    /// Its name.
    pub name: String,
    /// Its USB serial.
    pub serial: Option<String>,
    /// Its class.
    pub class: Class,
    /// The handle is held.
    pub open: bool,
}

/// The last problem worth showing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LastError {
    /// See [`WorkerEvent::PermissionDenied`].
    PermissionDenied {
        /// The port.
        port: String,
        /// The message.
        message: String,
    },
    /// See [`WorkerEvent::PortBusy`].
    Busy {
        /// The port.
        port: String,
    },
    /// See [`WorkerEvent::LogUnavailable`].
    LogUnavailable {
        /// Why.
        reason: String,
    },
}

/// Counters of the worker.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counters {
    /// Frames (and lone `00`s) queued to the port.
    pub frames_written: u64,
    /// HELLO frames the session handed out (its cadence, whatever the
    /// port did with them).
    pub hello_sent: u64,
    /// Of those, HELLO frames dropped by the write rules (expected while
    /// the bridge has no controller link and stops reading USB).
    pub hello_dropped: u64,
    /// Chunks dropped by the write rules.
    pub dropped_chunks: u64,
    /// Go-back-N rewinds (transport NAKs) over every session.
    pub rewinds: u64,
    /// Log entries dropped.
    pub log_dropped: u64,
    /// Events dropped because the UI did not read them.
    pub events_dropped: u64,
}

/// What the UI reads (immutable, behind an `Arc`). Story 4.5 maps it to
/// the LEDs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkSnapshot {
    /// The worker clock at publish.
    pub now_us: u64,
    /// Scans done.
    pub scans: u64,
    /// The last [`RECENT_SCANS`] scans: (start, listing time), µs on the
    /// worker clock, oldest first.
    pub recent_scans: Vec<(u64, u64)>,
    /// The last listing.
    pub ports: Vec<SeenPort>,
    /// The port the worker is on or trying.
    pub chosen: Option<Chosen>,
    /// The session phase (`Closed` without a port).
    pub phase: Phase,
    /// INFO of the open session.
    pub info: Option<Info>,
    /// The last STATUS and when it arrived (worker clock, µs): the UI
    /// judges staleness against `now_us`.
    pub status: Option<(StatusView, u64)>,
    /// Counters.
    pub counters: Counters,
    /// Frames written per port name (only opened ports are ever written).
    pub port_writes: Vec<(String, u64)>,
    /// The last problem worth showing.
    pub last_error: Option<LastError>,
    /// When the current session last entered `Opening` (worker clock, µs):
    /// set when a session is opened on a port and again when the
    /// controller rebooted (mid-session `NAK VERSION`, or a STATUS of a
    /// freshly booted controller); `None` without a port. The LEDs time
    /// their INFO grace from it (Story 4.5).
    pub session_since_us: Option<u64>,
    /// When the INFO that made the session `Connected` or `Incompatible`
    /// arrived (worker clock, µs); cleared on open, reboot and loss. The
    /// STATUS staleness of the LEDs counts from it until the first STATUS.
    pub connected_us: Option<u64>,
    /// The INFO of a protocol mismatch ([`LinkEvent::RequiredUpdate`]):
    /// the session keeps no INFO in `Incompatible`, so the versions the
    /// LEDs show come from here (Story 4.5, 4.6). Cleared on open and loss.
    pub required_update: Option<Info>,
    /// The latest `NAK NOT_READY` or `NAK NO_LINK` (from
    /// [`LinkEvent::Refused`] or [`LinkEvent::PieceAbandoned`]) and when it
    /// arrived (worker clock, µs); other reasons are not kept. The LEDs
    /// treat it like the STATUS flag until a newer STATUS. Cleared on open,
    /// reboot and loss.
    pub last_nak: Option<(NakReason, u64)>,
}

/// The receiving ends and the command sender of a worker.
#[derive(Debug)]
pub struct Endpoints {
    /// Commands to the worker.
    pub commands: Sender<WorkerCommand>,
    /// Discrete events.
    pub events: Receiver<TimedEvent>,
    /// The latest snapshot (capacity 1, never grows).
    pub snapshots: Receiver<Arc<LinkSnapshot>>,
}

#[derive(Debug, Clone)]
struct Seen {
    info: PortInfo,
    class: Class,
    retry_at_us: u64,
    perm_reported: bool,
    busy_reported: bool,
}

struct Conn {
    name: String,
    serial: Option<String>,
    port: OpenPort,
    session: Session,
    opened_us: u64,
    last_rx_us: Option<u64>,
    /// INFO came (compatible or not): the round robin keeps this port.
    answered: bool,
    rewinds_seen: u64,
}

/// The steppable worker.
pub struct WorkerCore<B: Backend, C: Clock> {
    backend: B,
    clock: C,
    rules: Rules,
    log: LogHandle,
    session_cfg: SessionConfig,
    cmd_rx: Receiver<WorkerCommand>,
    ev_tx: Sender<TimedEvent>,
    snap_tx: Sender<Arc<LinkSnapshot>>,
    snap_rx: Receiver<Arc<LinkSnapshot>>,
    waker: Box<dyn Fn() + Send>,
    next_scan_us: Option<u64>,
    seen: Vec<Seen>,
    conn: Option<Conn>,
    rotate_after: Option<String>,
    last_serial: Option<String>,
    last_name: Option<String>,
    chosen: Option<Chosen>,
    counters: Counters,
    rewinds_closed: u64,
    dropped_closed: u64,
    port_writes: Vec<(String, u64)>,
    last_error: Option<LastError>,
    status: Option<(StatusView, u64)>,
    session_since_us: Option<u64>,
    connected_us: Option<u64>,
    required_update: Option<Info>,
    last_nak: Option<(NakReason, u64)>,
    scans: u64,
    recent_scans: std::collections::VecDeque<(u64, u64)>,
    shutdown: bool,
    dirty: bool,
    buf: Vec<u8>,
}

/// When the worker ends (shutdown, or a panic unwinding its thread) its
/// snapshot sender goes first, then the waker runs: the UI wakes and sees
/// the disconnect at once, not at its next input (Story 4.5 review: a dead
/// worker must never leave a frozen strip).
impl<B: Backend, C: Clock> Drop for WorkerCore<B, C> {
    fn drop(&mut self) {
        let (gone, _) = bounded(1);
        drop(std::mem::replace(&mut self.snap_tx, gone));
        (self.waker)();
    }
}

impl<B: Backend, C: Clock> fmt::Debug for WorkerCore<B, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WorkerCore")
            .field("scans", &self.scans)
            .field("open", &self.conn.as_ref().map(|c| c.name.clone()))
            .field("shutdown", &self.shutdown)
            .finish_non_exhaustive()
    }
}

fn hex3(v: [u8; 3]) -> String {
    let [a, b, c] = v;
    format!("{a}.{b}.{c}")
}

impl<B: Backend, C: Clock> WorkerCore<B, C> {
    /// A worker on `backend` and `clock` with these rules; `waker` is
    /// called after every snapshot published.
    pub fn new(
        backend: B,
        clock: C,
        rules: Rules,
        log: LogHandle,
        waker: Box<dyn Fn() + Send>,
    ) -> (Self, Endpoints) {
        let (cmd_tx, cmd_rx) = bounded(64);
        let (ev_tx, ev_rx) = bounded(EVENTS_CAP);
        let (snap_tx, snap_rx) = bounded(1);
        let core = Self {
            backend,
            clock,
            rules,
            log,
            session_cfg: SessionConfig::default(),
            cmd_rx,
            ev_tx,
            snap_tx,
            snap_rx: snap_rx.clone(),
            waker,
            next_scan_us: None,
            seen: Vec::new(),
            conn: None,
            rotate_after: None,
            last_serial: None,
            last_name: None,
            chosen: None,
            counters: Counters::default(),
            rewinds_closed: 0,
            dropped_closed: 0,
            port_writes: Vec::new(),
            last_error: None,
            status: None,
            session_since_us: None,
            connected_us: None,
            required_update: None,
            last_nak: None,
            scans: 0,
            recent_scans: std::collections::VecDeque::with_capacity(RECENT_SCANS),
            shutdown: false,
            dirty: true,
            buf: vec![0; 4096],
        };
        let ends = Endpoints {
            commands: cmd_tx,
            events: ev_rx,
            snapshots: snap_rx,
        };
        (core, ends)
    }

    /// Shutdown was handled: the port is closed and the loop must end.
    #[must_use]
    pub fn is_shut_down(&self) -> bool {
        self.shutdown
    }

    /// A port handle is held.
    #[must_use]
    pub fn has_port(&self) -> bool {
        self.conn.is_some()
    }

    /// The backend (tests inspect fakes through it).
    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// The open session, for tests and dry runs that drive commands later
    /// stories will send (SET_STATE). Not in shipped builds.
    #[cfg(feature = "sim")]
    pub fn session_mut(&mut self) -> Option<&mut Session> {
        self.conn.as_mut().map(|c| &mut c.session)
    }

    fn emit(&mut self, at_us: u64, event: WorkerEvent) {
        if let Err(TrySendError::Full(_)) = self.ev_tx.try_send(TimedEvent { at_us, event }) {
            self.counters.events_dropped += 1;
        }
        self.dirty = true;
    }

    /// One pass: commands, scan when due, pick and open, one session pass,
    /// a snapshot if anything changed.
    pub fn step(&mut self) {
        if self.shutdown {
            return;
        }
        // Only `Shutdown` exists so far; later commands join here. A
        // dropped handle (every sender gone) is a shutdown too, so a worker
        // never outlives its owner.
        if self
            .cmd_rx
            .try_iter()
            .any(|cmd| cmd == WorkerCommand::Shutdown)
            || matches!(self.cmd_rx.try_recv(), Err(TryRecvError::Disconnected))
        {
            let now = self.clock.now_us();
            self.close(now);
            self.shutdown = true;
            self.dirty = true;
            self.publish();
            return;
        }
        if let Some(reason) = self.log.take_unavailable() {
            let now = self.clock.now_us();
            // Busy and permission are reported once and stay the problem
            // worth showing: the log never hides them (Story 4.5 review).
            if !matches!(
                self.last_error,
                Some(LastError::Busy { .. } | LastError::PermissionDenied { .. })
            ) {
                self.last_error = Some(LastError::LogUnavailable {
                    reason: reason.clone(),
                });
            }
            self.emit(now, WorkerEvent::LogUnavailable { reason });
        }
        let now = self.clock.now_us();
        if self.next_scan_us.is_none_or(|t| now >= t) {
            self.scan(now);
        }
        if self.conn.is_none() {
            self.try_open();
        }
        if self.window_over() {
            // Probe window over without INFO: the next candidate, at once,
            // and nothing more is written to this one.
            let now = self.clock.now_us();
            if let Some(name) = self.conn.as_ref().map(|c| c.name.clone()) {
                self.close(now);
                if let Some(s) = self.seen.iter_mut().find(|s| s.info.name == name) {
                    s.retry_at_us = 0;
                }
                self.rotate_after = Some(name);
            }
            self.try_open();
        }
        if self.conn.is_some() {
            self.io_pass();
        }
        if self.dirty {
            self.publish();
        }
    }

    // ------------------------------------------------------------ scan

    fn scan(&mut self, now: u64) {
        let next = self.next_scan_us.unwrap_or(now);
        let mut next = next + SCAN_PERIOD_US;
        while next <= now {
            next += SCAN_PERIOD_US;
        }
        self.next_scan_us = Some(next);
        let listed = self.backend.list();
        let end = self.clock.now_us();
        self.scans += 1;
        if self.recent_scans.len() == RECENT_SCANS {
            self.recent_scans.pop_front();
        }
        self.recent_scans.push_back((now, end.saturating_sub(now)));
        self.dirty = true;
        let listed = match listed {
            Ok(l) => l,
            Err(e) => {
                self.log.log("scan_error", e);
                return;
            }
        };
        // Vanished first.
        let gone: Vec<String> = self
            .seen
            .iter()
            .filter(|s| !listed.iter().any(|p| p.name == s.info.name))
            .map(|s| s.info.name.clone())
            .collect();
        for name in gone {
            if self.conn.as_ref().is_some_and(|c| c.name == name) {
                self.lose(now, "the port left the port list".to_owned());
            }
            self.seen.retain(|s| s.info.name != name);
            self.log.log("port_vanished", format!("port={name}"));
            self.emit(now, WorkerEvent::PortVanished { port: name });
        }
        for info in listed {
            if let Some(s) = self.seen.iter_mut().find(|s| s.info.name == info.name) {
                // Same name: keep the flags, refresh the identity.
                let class = device::classify(&info, self.rules);
                s.info = info;
                s.class = class;
                continue;
            }
            let class = device::classify(&info, self.rules);
            let serial = info.usb.as_ref().and_then(|u| u.serial.clone());
            let usb = info.usb.as_ref().map_or_else(
                || "-".to_owned(),
                |u| format!("{:04X}:{:04X}", u.vid, u.pid),
            );
            let product = info
                .usb
                .as_ref()
                .and_then(|u| u.product.clone())
                .unwrap_or_else(|| "-".to_owned());
            let manufacturer = info
                .usb
                .as_ref()
                .and_then(|u| u.manufacturer.clone())
                .unwrap_or_else(|| "-".to_owned());
            self.log.log(
                "port_listed",
                format!(
                    "port={} usb={usb} serial={} manufacturer=\"{manufacturer}\" product=\"{product}\" class={}",
                    info.name,
                    serial.as_deref().unwrap_or("-"),
                    class.tag()
                ),
            );
            self.emit(
                now,
                WorkerEvent::PortListed {
                    port: info.name.clone(),
                    class: class.clone(),
                    serial,
                },
            );
            if let Class::Foreign { product } = &class {
                self.log.log(
                    "foreign",
                    format!("port={} product=\"{product}\" (never opened)", info.name),
                );
                self.emit(
                    now,
                    WorkerEvent::ForeignBoard {
                        port: info.name.clone(),
                        product: product.clone(),
                    },
                );
            }
            self.seen.push(Seen {
                info,
                class,
                retry_at_us: 0,
                perm_reported: false,
                busy_reported: false,
            });
        }
    }

    // ------------------------------------------------------------ choice

    fn round_robin(&self) -> bool {
        let ours = self.seen.iter().any(|s| s.class == Class::Ours);
        let unconfirmed = self
            .seen
            .iter()
            .filter(|s| s.class == Class::Unconfirmed)
            .count();
        !ours && unconfirmed > 1
    }

    /// The candidate to open now: in rank order (in round robin, starting
    /// after the last one tried), the first whose retry time has come. A
    /// candidate waiting to retry never blocks the others.
    fn target(&self, now: u64) -> Option<usize> {
        let pairs: Vec<(PortInfo, Class)> = self
            .seen
            .iter()
            .map(|s| (s.info.clone(), s.class.clone()))
            .collect();
        let order: Vec<String> = device::ranked(&pairs, self.last_serial.as_deref())
            .into_iter()
            .map(|p| p.name.clone())
            .collect();
        let start = match (&self.rotate_after, self.round_robin()) {
            (Some(prev), true) => order
                .iter()
                .position(|n| n == prev)
                .map_or(0, |i| (i + 1) % order.len().max(1)),
            _ => 0,
        };
        (0..order.len())
            .filter_map(|k| order.get((start + k) % order.len()))
            .filter_map(|name| self.seen.iter().position(|s| &s.info.name == name))
            .find(|&i| self.seen.get(i).is_some_and(|s| now >= s.retry_at_us))
    }

    fn window_over(&self) -> bool {
        let Some(c) = &self.conn else {
            return false;
        };
        !c.answered
            && self.round_robin()
            && self.clock.now_us().saturating_sub(c.opened_us) >= PROBE_WINDOW_US
    }

    // ------------------------------------------------------------ open

    fn try_open(&mut self) {
        let now = self.clock.now_us();
        let Some(i) = self.target(now) else {
            return;
        };
        let Some(s) = self.seen.get(i).cloned() else {
            return;
        };
        let serial = s.info.usb.as_ref().and_then(|u| u.serial.clone());
        let name = s.info.name.clone();
        self.chosen = Some(Chosen {
            name: name.clone(),
            serial: serial.clone(),
            class: s.class.clone(),
            open: false,
        });
        self.dirty = true;
        match self.backend.open(&name) {
            Ok(raw) => {
                let mut port = OpenPort::new(raw);
                if let Err(PortLost(e)) = port.start() {
                    self.log
                        .log("port_lost", format!("port={name} reason=\"{e}\""));
                    self.set_retry(&name, now + OPEN_RETRY_US);
                    self.emit(
                        now,
                        WorkerEvent::PortLost {
                            port: name,
                            reason: e,
                            last_rx_us: None,
                        },
                    );
                    return;
                }
                let mut session = Session::new(self.session_cfg);
                session.open(now);
                if let (Some(ls), Some(ln)) = (&self.last_serial, &self.last_name)
                    && serial.as_ref() == Some(ls)
                    && *ln != name
                {
                    let from = ln.clone();
                    self.log
                        .log("port_renamed", format!("from={from} to={name} serial={ls}"));
                    self.emit(
                        now,
                        WorkerEvent::PortRenamed {
                            from,
                            to: name.clone(),
                        },
                    );
                    self.last_name = Some(name.clone());
                }
                self.log.log(
                    "port_opened",
                    format!(
                        "port={name} class={} serial={}",
                        s.class.tag(),
                        serial.as_deref().unwrap_or("-")
                    ),
                );
                self.emit(
                    now,
                    WorkerEvent::PortOpened {
                        port: name.clone(),
                        class: s.class.clone(),
                        serial: serial.clone(),
                    },
                );
                if let Some(c) = self.chosen.as_mut() {
                    c.open = true;
                }
                // A new state for this port: a later hold or denial is news.
                if let Some(seen) = self.seen.iter_mut().find(|x| x.info.name == name) {
                    seen.busy_reported = false;
                    seen.perm_reported = false;
                }
                if matches!(self.last_error, Some(LastError::PermissionDenied { ref port, .. } | LastError::Busy { ref port }) if *port == name)
                {
                    self.last_error = None;
                }
                self.status = None;
                self.session_since_us = Some(now);
                self.connected_us = None;
                self.required_update = None;
                self.last_nak = None;
                self.conn = Some(Conn {
                    name,
                    serial,
                    port,
                    session,
                    opened_us: now,
                    last_rx_us: None,
                    answered: false,
                    rewinds_seen: 0,
                });
            }
            Err(e) => self.open_failed(now, &name, &e),
        }
    }

    fn set_retry(&mut self, name: &str, at: u64) {
        if let Some(s) = self.seen.iter_mut().find(|s| s.info.name == name) {
            s.retry_at_us = at;
        }
    }

    fn open_failed(&mut self, now: u64, name: &str, e: &OpenError) {
        if self.round_robin() {
            self.rotate_after = Some(name.to_owned());
        }
        match e {
            OpenError::PermissionDenied => {
                // Retried only at later scans, never in a loop (AC 7).
                let next = self.next_scan_us.unwrap_or(now + SCAN_PERIOD_US);
                self.set_retry(name, next);
                let first = self
                    .seen
                    .iter_mut()
                    .find(|s| s.info.name == name)
                    .is_some_and(|s| {
                        s.busy_reported = false;
                        !std::mem::replace(&mut s.perm_reported, true)
                    });
                if first {
                    let group = self
                        .backend
                        .node_group(name)
                        .unwrap_or_else(|| crate::port::DEFAULT_SERIAL_GROUP.to_owned());
                    let message = permission_message(name, &group);
                    self.log.log(
                        "permission_denied",
                        format!("port={name} group={group} message=\"{message}\""),
                    );
                    self.last_error = Some(LastError::PermissionDenied {
                        port: name.to_owned(),
                        message: message.clone(),
                    });
                    self.emit(
                        now,
                        WorkerEvent::PermissionDenied {
                            port: name.to_owned(),
                            group,
                            message,
                        },
                    );
                }
            }
            OpenError::Busy => {
                self.set_retry(name, now + OPEN_RETRY_US);
                let first = self
                    .seen
                    .iter_mut()
                    .find(|s| s.info.name == name)
                    .is_some_and(|s| {
                        s.perm_reported = false;
                        !std::mem::replace(&mut s.busy_reported, true)
                    });
                if first {
                    self.log.log("port_busy", format!("port={name}"));
                    self.last_error = Some(LastError::Busy {
                        port: name.to_owned(),
                    });
                    self.emit(
                        now,
                        WorkerEvent::PortBusy {
                            port: name.to_owned(),
                        },
                    );
                }
            }
            OpenError::Gone | OpenError::Other(_) => {
                // The node can be listed before it opens (`ocb-dev`
                // `reopen`): a failed open is "not yet" (AC 6).
                self.set_retry(name, now + OPEN_RETRY_US);
            }
        }
    }

    // ------------------------------------------------------------ close

    fn take_conn(&mut self) -> Option<Conn> {
        let mut c = self.conn.take()?;
        c.port.close();
        self.rewinds_closed += c.session.send_stats().rewinds;
        self.dropped_closed += c.port.dropped_chunks;
        if let Some(ch) = self.chosen.as_mut() {
            ch.open = false;
        }
        self.status = None;
        self.session_since_us = None;
        self.connected_us = None;
        self.required_update = None;
        self.last_nak = None;
        self.dirty = true;
        Some(c)
    }

    fn close(&mut self, now: u64) {
        if let Some(c) = self.take_conn() {
            self.log.log("port_closed", format!("port={}", c.name));
            self.emit(now, WorkerEvent::PortClosed { port: c.name });
        }
    }

    fn lose(&mut self, now: u64, reason: String) {
        if let Some(c) = self.take_conn() {
            self.log
                .log("port_lost", format!("port={} reason=\"{reason}\"", c.name));
            // Not at once: a node that opens but fails every read would
            // otherwise loop open → lost about 100 times a second.
            self.set_retry(&c.name, now + OPEN_RETRY_US);
            self.emit(
                now,
                WorkerEvent::PortLost {
                    port: c.name,
                    reason,
                    last_rx_us: c.last_rx_us,
                },
            );
        }
    }

    // ------------------------------------------------------------ session

    fn io_pass(&mut self) {
        let now = self.clock.now_us();
        let Some(c) = self.conn.as_mut() else {
            return;
        };
        match c.port.read(&mut self.buf) {
            Ok(0) => {}
            Ok(n) => {
                c.session.handle_rx(self.buf.get(..n).unwrap_or(&[]), now);
                c.last_rx_us = Some(now);
            }
            Err(PortLost(e)) => {
                self.lose(now, e);
                return;
            }
        }
        let now = self.clock.now_us();
        if c.session.poll_timeout().is_some_and(|t| t <= now) {
            c.session.handle_timeout(now);
        }
        let mut lost = None;
        while let Some(bytes) = c.session.poll_transmit(now) {
            // Published, so the frame counters are never stale (HELLO every
            // 100 ms at most while opening).
            self.dirty = true;
            let hello = bytes.split_last().is_some_and(|(&z, enc)| {
                z == 0 && decode_frame(RxSide::Controller, enc).is_ok_and(|f| f.ty == MSG_HELLO)
            });
            if hello {
                self.counters.hello_sent += 1;
            }
            if c.port.queue(&bytes) {
                self.counters.frames_written += 1;
                match self.port_writes.iter_mut().find(|p| p.0 == c.name) {
                    Some(p) => p.1 += 1,
                    None => self.port_writes.push((c.name.clone(), 1)),
                }
            } else if hello {
                self.counters.hello_dropped += 1;
            }
        }
        // A few ms at most per pass: a port that does not drain never holds
        // the worker (scans, shutdown and the log stay on time).
        if let Err(PortLost(e)) = c.port.flush(&self.clock) {
            lost = Some(e);
        }
        let mut events = Vec::new();
        while let Some(ev) = c.session.poll_event() {
            events.push(ev);
        }
        let rewinds = c.session.send_stats().rewinds;
        let new_rewinds = rewinds.saturating_sub(c.rewinds_seen);
        c.rewinds_seen = rewinds;
        let name = c.name.clone();
        for ev in events {
            self.on_link_event(now, &name, ev);
        }
        if new_rewinds > 0 {
            self.log.log(
                "nak",
                format!("transport port={name} rewinds=+{new_rewinds} total={rewinds}"),
            );
            self.dirty = true;
        }
        if let Some(e) = lost {
            self.lose(now, e);
        }
    }

    fn on_link_event(&mut self, now: u64, port: &str, ev: LinkEvent) {
        match &ev {
            LinkEvent::Status(v) => {
                self.status = Some((*v, now));
                self.dirty = true;
                return;
            }
            LinkEvent::Connected(info) => {
                if let Some(c) = self.conn.as_mut() {
                    c.answered = true;
                    self.last_serial.clone_from(&c.serial);
                    self.last_name = Some(c.name.clone());
                }
                self.connected_us = Some(now);
                self.rotate_after = None;
                self.log.log(
                    "version",
                    format!(
                        "connected port={port} proto={} fw={} bridge={} hid={:?} queue_cap={} app_timeout_ms={} report_period_us={}",
                        info.proto_ver,
                        hex3(info.fw_ver),
                        hex3(info.bridge_ver),
                        info.hid_profile,
                        info.queue_cap,
                        info.app_timeout_ms,
                        info.report_period_us
                    ),
                );
            }
            LinkEvent::RequiredUpdate {
                controller_proto,
                app_proto,
                info,
            } => {
                // It answered: the round robin must not close it (the
                // required-update screen of Story 4.6 stays put).
                if let Some(c) = self.conn.as_mut() {
                    c.answered = true;
                }
                self.connected_us = Some(now);
                self.required_update = Some(*info);
                self.log.log(
                "version",
                format!(
                    "required update port={port} controller_proto={controller_proto} app_proto={app_proto} fw={} bridge={}",
                    hex3(info.fw_ver),
                    hex3(info.bridge_ver)
                ),
                );
            }
            LinkEvent::RecommendedUpdate {
                fw_ver,
                bridge_ver,
                app_ver,
                fw_differs,
                bridge_differs,
            } => self.log.log(
                "version",
                format!(
                    "recommended update fw={} bridge={} app={} fw_differs={fw_differs} bridge_differs={bridge_differs}",
                    hex3(*fw_ver),
                    hex3(*bridge_ver),
                    hex3(*app_ver)
                ),
            ),
            LinkEvent::ControllerRebooted => {
                // The session HELLOs again: a new INFO grace, and nothing
                // of the previous boot may keep the LEDs "fresh".
                self.session_since_us = Some(now);
                self.connected_us = None;
                self.status = None;
                self.last_nak = None;
                self.log.log("reboot", format!("controller rebooted port={port}"));
            }
            LinkEvent::ControllerEvent(code) => self.log.log(
                "event",
                format!("code=0x{:02x} {code:?}", code.to_u8()),
            ),
            LinkEvent::PlaybackDone => self.log.log("event", "PLAYBACK_DONE"),
            LinkEvent::PieceAbandoned { reason } => {
                if let AbandonReason::Nak(r @ (NakReason::NotReady | NakReason::NoLink)) = reason {
                    self.last_nak = Some((*r, now));
                }
                self.log.log("nak", format!("PieceAbandoned reason={reason:?}"));
            }
            LinkEvent::Refused { reason, ty } => {
                if matches!(reason, NakReason::NotReady | NakReason::NoLink) {
                    self.last_nak = Some((*reason, now));
                }
                self.log.log(
                    "nak",
                    format!("Refused reason={reason:?} ty=0x{ty:02x}"),
                );
            }
            LinkEvent::UnknownNak {
                reason,
                expected,
                got,
            } => self.log.log(
                "nak",
                format!("unknown reason=0x{reason:02x} expected={expected} got={got}"),
            ),
            LinkEvent::Malformed { ty } => self.log.log("malformed", format!("ty=0x{ty:02x}")),
            LinkEvent::Stopped => {}
        }
        self.emit(now, WorkerEvent::Link(ev));
    }

    // ------------------------------------------------------------ publish

    fn snapshot(&self) -> LinkSnapshot {
        let mut counters = self.counters;
        let (rewinds, dropped) = self.conn.as_ref().map_or((0, 0), |c| {
            (c.session.send_stats().rewinds, c.port.dropped_chunks)
        });
        counters.rewinds = self.rewinds_closed + rewinds;
        counters.dropped_chunks = self.dropped_closed + dropped;
        counters.log_dropped = self.log.dropped_total();
        LinkSnapshot {
            now_us: self.clock.now_us(),
            scans: self.scans,
            recent_scans: self.recent_scans.iter().copied().collect(),
            ports: self
                .seen
                .iter()
                .map(|s| SeenPort {
                    info: s.info.clone(),
                    class: s.class.clone(),
                })
                .collect(),
            chosen: self.chosen.clone(),
            phase: self
                .conn
                .as_ref()
                .map_or(Phase::Closed, |c| c.session.phase()),
            info: self.conn.as_ref().and_then(|c| c.session.info().copied()),
            status: self.status,
            counters,
            port_writes: self.port_writes.clone(),
            last_error: self.last_error.clone(),
            session_since_us: self.session_since_us,
            connected_us: self.connected_us,
            required_update: self.required_update,
            last_nak: self.last_nak,
        }
    }

    fn publish(&mut self) {
        self.dirty = false;
        let snap = Arc::new(self.snapshot());
        // Capacity 1: drop the stale snapshot the UI has not read, so it
        // always gets the latest and nothing grows while the window is
        // hidden (crossbeam channels are MPMC).
        let _ = self.snap_rx.try_recv();
        let _ = self.snap_tx.try_send(snap);
        (self.waker)();
    }
}

/// A running worker thread and its channels.
#[derive(Debug)]
pub struct WorkerHandle {
    /// Commands to the worker.
    pub commands: Sender<WorkerCommand>,
    /// Discrete events.
    pub events: Receiver<TimedEvent>,
    /// The latest snapshot.
    pub snapshots: Receiver<Arc<LinkSnapshot>>,
    join: Option<JoinHandle<()>>,
}

impl WorkerHandle {
    /// Sends `Shutdown` and waits at most `limit` for the thread to end.
    /// Returns whether it ended (it is joined then); never hangs.
    pub fn shutdown(&mut self, limit: Duration) -> bool {
        let _ = self.commands.try_send(WorkerCommand::Shutdown);
        let Some(join) = self.join.take() else {
            return true;
        };
        let start = Instant::now();
        while !join.is_finished() {
            if start.elapsed() >= limit {
                // Left running; the process exit ends it.
                self.join = Some(join);
                return false;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        let _ = join.join();
        true
    }

    /// The thread has ended.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.join.as_ref().is_none_or(JoinHandle::is_finished)
    }
}

/// Starts the worker thread (`bonooru-serial`) on the real clock with the
/// host's rules.
///
/// # Errors
/// The OS error if the thread cannot be started.
pub fn spawn<B: Backend + 'static>(
    backend: B,
    log: LogHandle,
    waker: Box<dyn Fn() + Send>,
) -> std::io::Result<WorkerHandle> {
    spawn_with(backend, RealClock::new(), host_rules(), log, waker)
}

/// [`spawn`] with a given clock and rules.
///
/// # Errors
/// The OS error if the thread cannot be started.
pub fn spawn_with<B: Backend + 'static, C: Clock + 'static>(
    backend: B,
    clock: C,
    rules: Rules,
    log: LogHandle,
    waker: Box<dyn Fn() + Send>,
) -> std::io::Result<WorkerHandle> {
    let (mut core, ends) = WorkerCore::new(backend, clock, rules, log, waker);
    let join = std::thread::Builder::new()
        .name("bonooru-serial".to_owned())
        .spawn(move || {
            loop {
                core.step();
                if core.is_shut_down() {
                    break;
                }
                // With a port open, its 1 ms read timeout paces the loop.
                if !core.has_port() {
                    std::thread::sleep(IDLE_SLEEP);
                }
            }
        })?;
    Ok(WorkerHandle {
        commands: ends.commands,
        events: ends.events,
        snapshots: ends.snapshots,
        join: Some(join),
    })
}
