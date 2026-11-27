//! Fake backends for tests and dry runs (feature `sim`, never in a shipped
//! build): [`FakeBackend`] lists scripted ports on a [`FakeClock`] and backs
//! each open port with an `ocb_sim::FakeController` through two
//! `ocb_sim::Channel`s (the `SimIo` pattern of `ocb-dev`'s dry run);
//! [`BlockingBackend`] blocks in `list` and `open` on the real clock (G.13).

use std::collections::VecDeque;
use std::io;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use ocb_core::device::{BRIDGE_PID, BRIDGE_VID, PortInfo, Rules, UsbIds};
use ocb_sim::{Channel, ControllerConfig, FakeController};

use crate::log::LogHandle;
use crate::port::{Backend, OpenError, Port};
use crate::worker::{
    Clock, Endpoints, FakeClock, LinkSnapshot, TimedEvent, WorkerCore, WorkerEvent,
};

/// A change to a fake controller at a fake time.
pub type Hook = Box<dyn FnMut(&mut FakeController) + Send>;

/// What is behind a fake port.
#[derive(Debug)]
pub enum Device {
    /// A bridge with a powered, wired controller.
    Controller(Box<FakeController>),
    /// Nothing answers (a bridge alone, another board).
    Silent,
}

struct Link {
    up: Channel,
    down: Channel,
    pending: Vec<u8>,
    generation: u64,
}

/// One scripted port.
pub struct FakePort {
    /// As listed.
    pub info: PortInfo,
    /// Listed in these windows `[from, until)` of fake time.
    pub present: Vec<(u64, Option<u64>)>,
    /// The next open attempts fail with these, in order.
    pub open_failures: VecDeque<OpenError>,
    /// Every open attempt fails with this.
    pub always_fails: Option<OpenError>,
    /// Reads and writes fail while the port is not listed (an unplug);
    /// `false`: the port only vanishes from the listing.
    pub error_when_gone: bool,
    /// The device behind the port.
    pub device: Device,
    /// Changes to the controller at fake times (run once).
    pub hooks: Vec<(u64, Hook)>,
    /// The group owning the node (Linux permission message).
    pub group: Option<String>,
    /// Flip a bit of the next frame the app writes (a transport NAK).
    pub corrupt_next_write: bool,
    /// What `bytes_to_write` reports (over 4096: the worker drops chunks).
    pub queued: u32,
    /// The next read fails once (a glitch; the port stays listed).
    pub fail_next_read: bool,
    /// Writes make no progress: each call takes the read sleep of real
    /// time (if any) and times out, like a device that does not read.
    pub stall_writes: bool,
    /// Windows `[from, until)` of fake time in which the controller behind
    /// the port is cut off (unplugged or unpowered while the bridge stays):
    /// it gets no bytes, sends none and does not run; the port stays listed
    /// and readable (reads return 0 bytes). Hooks still run.
    pub mutes: Vec<(u64, Option<u64>)>,
    link: Option<Link>,
}

impl std::fmt::Debug for FakePort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FakePort")
            .field("info", &self.info)
            .field("present", &self.present)
            .finish_non_exhaustive()
    }
}

impl FakePort {
    /// A USB port with these ids, nothing behind it, listed from 0 on.
    #[must_use]
    pub fn usb(
        name: &str,
        vid: u16,
        pid: u16,
        serial: Option<&str>,
        product: Option<&str>,
    ) -> Self {
        Self {
            info: PortInfo {
                name: name.to_owned(),
                usb: Some(UsbIds {
                    vid,
                    pid,
                    serial: serial.map(str::to_owned),
                    manufacturer: Some("Bonooru".to_owned()),
                    product: product.map(str::to_owned),
                }),
            },
            present: vec![(0, None)],
            open_failures: VecDeque::new(),
            always_fails: None,
            error_when_gone: true,
            device: Device::Silent,
            hooks: Vec::new(),
            group: None,
            corrupt_next_write: false,
            queued: 0,
            fail_next_read: false,
            stall_writes: false,
            mutes: Vec::new(),
            link: None,
        }
    }

    /// The bridge (`2E8A:000A`, this product) with a default controller.
    #[must_use]
    pub fn bridge(name: &str, serial: &str, product: Option<&str>) -> Self {
        Self::usb(name, BRIDGE_VID, BRIDGE_PID, Some(serial), product)
            .controller(ControllerConfig::default())
    }

    /// A port that is not USB.
    #[must_use]
    pub fn plain(name: &str) -> Self {
        let mut p = Self::usb(name, 0, 0, None, None);
        p.info.usb = None;
        p
    }

    /// A powered controller with this configuration behind the port.
    #[must_use]
    pub fn controller(mut self, cfg: ControllerConfig) -> Self {
        self.device = Device::Controller(Box::new(FakeController::new(cfg)));
        self
    }

    /// Nothing behind the port.
    #[must_use]
    pub fn silent(mut self) -> Self {
        self.device = Device::Silent;
        self
    }

    /// Listed only in these windows.
    #[must_use]
    pub fn present(mut self, windows: Vec<(u64, Option<u64>)>) -> Self {
        self.present = windows;
        self
    }

    /// The next opens fail with these.
    #[must_use]
    pub fn fail_opens(mut self, errors: Vec<OpenError>) -> Self {
        self.open_failures = errors.into();
        self
    }

    /// Every open fails with this.
    #[must_use]
    pub fn always_fail(mut self, e: OpenError) -> Self {
        self.always_fails = Some(e);
        self
    }

    /// The port vanishes from the listing without read errors.
    #[must_use]
    pub fn silent_unplug(mut self) -> Self {
        self.error_when_gone = false;
        self
    }

    /// The node's group.
    #[must_use]
    pub fn group(mut self, g: &str) -> Self {
        self.group = Some(g.to_owned());
        self
    }

    /// A change to the controller at `t_us`.
    #[must_use]
    pub fn hook(mut self, t_us: u64, h: Hook) -> Self {
        self.hooks.push((t_us, h));
        self
    }

    /// From `t_us` the controller behind the port is cut off (Story 4.5:
    /// the controller unplugged from the Switch while the bridge stays).
    #[must_use]
    pub fn mute_at(mut self, t_us: u64) -> Self {
        self.mutes.push((t_us, None));
        self
    }

    /// Ends the open mute window at `t_us` (the controller is back; pair it
    /// with a hook at the same time to model a reboot).
    #[must_use]
    ///
    /// # Panics
    /// Without an open mute window (a misordered script would otherwise
    /// run with no mute at all and still pass).
    pub fn unmute_at(mut self, t_us: u64) -> Self {
        let open = self.mutes.iter_mut().rev().find(|w| w.1.is_none());
        assert!(open.is_some(), "unmute_at({t_us}) without an open mute_at");
        if let Some(w) = open {
            assert!(w.0 <= t_us, "unmute_at({t_us}) before its mute_at({})", w.0);
            w.1 = Some(t_us);
        }
        self
    }

    fn muted(&self, now: u64) -> bool {
        self.mutes
            .iter()
            .any(|&(a, b)| now >= a && b.is_none_or(|b| now < b))
    }

    fn listed(&self, now: u64) -> bool {
        self.present
            .iter()
            .any(|&(a, b)| now >= a && b.is_none_or(|b| now < b))
    }
}

/// Everything the fake backend recorded, and its ports.
#[derive(Debug, Default)]
pub struct FakeState {
    /// The fake clock.
    pub clock: FakeClock,
    /// The scripted ports.
    pub ports: Vec<FakePort>,
    /// When `list` was called.
    pub list_calls: Vec<u64>,
    /// Every open attempt: (time, name).
    pub opens: Vec<(u64, String)>,
    /// Every chunk written: (time, port, bytes).
    pub writes: Vec<(u64, String, Vec<u8>)>,
    /// Every DTR change: (time, port, level).
    pub dtr: Vec<(u64, String, bool)>,
    /// Every input clear: (time, port).
    pub clears: Vec<(u64, String)>,
    /// Every read that returned bytes: (time, port).
    pub reads: Vec<(u64, String)>,
    /// Every close (handle dropped): (time, port).
    pub closes: Vec<(u64, String)>,
    /// Every port operation in order: (time, port, op), op one of `open`,
    /// `dtr_on`, `dtr_off`, `clear`, `write`, `read`, `close`.
    pub ops: Vec<(u64, String, &'static str)>,
    /// `list` fails with this.
    pub list_error: Option<String>,
    /// Each `list` call moves the fake clock this far (a slow enumeration).
    pub list_delay_us: u64,
    /// The next `list` calls each move the fake clock this much more (one
    /// slow enumeration).
    pub list_stalls: VecDeque<u64>,
    /// A read with nothing to return sleeps this long of real time (a
    /// worker on a real thread then does not spin; the real port's 1 ms
    /// timeout does the same).
    pub read_sleep: Option<Duration>,
    /// The fake clock at every read call, bytes or not (a test on a real
    /// worker thread sees the worker's passes; Story 4.8 review).
    pub read_calls: Vec<u64>,
    /// While set, every read call waits (real time, 1 ms steps) before it
    /// reads: the worker thread is held in its pass, as a stuck worker
    /// (Story 4.8: a full command queue).
    pub hold: bool,
    generation: u64,
}

impl FakeState {
    fn port_mut(&mut self, name: &str) -> Option<&mut FakePort> {
        self.ports.iter_mut().find(|p| p.info.name == name)
    }

    /// Open attempts on `name`, their times.
    #[must_use]
    pub fn opens_of(&self, name: &str) -> Vec<u64> {
        self.opens
            .iter()
            .filter(|o| o.1 == name)
            .map(|o| o.0)
            .collect()
    }

    /// Writes to `name`, their times.
    #[must_use]
    pub fn writes_of(&self, name: &str) -> Vec<u64> {
        self.writes
            .iter()
            .filter(|w| w.1 == name)
            .map(|w| w.0)
            .collect()
    }

    /// The DTR level of `name` now (`false` if never set or closed).
    #[must_use]
    pub fn dtr_of(&self, name: &str) -> bool {
        self.dtr
            .iter()
            .rev()
            .find(|d| d.1 == name)
            .is_some_and(|d| d.2)
    }
}

/// A scripted backend on a fake clock. Clones share the state.
#[derive(Debug, Clone, Default)]
pub struct FakeBackend {
    inner: Arc<Mutex<FakeState>>,
}

impl FakeBackend {
    /// No ports yet, on `clock`.
    #[must_use]
    pub fn new(clock: FakeClock) -> Self {
        let b = Self::default();
        b.state().clock = clock;
        b
    }

    /// Adds a port.
    #[must_use]
    pub fn with(self, port: FakePort) -> Self {
        self.state().ports.push(port);
        self
    }

    /// The shared state.
    pub fn state(&self) -> MutexGuard<'_, FakeState> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Backend for FakeBackend {
    fn list(&mut self) -> Result<Vec<PortInfo>, String> {
        let mut st = self.state();
        let now = st.clock.now_us();
        st.list_calls.push(now);
        let delay = st.list_delay_us + st.list_stalls.pop_front().unwrap_or(0);
        st.clock.advance(delay);
        if let Some(e) = &st.list_error {
            return Err(e.clone());
        }
        Ok(st
            .ports
            .iter()
            .filter(|p| p.listed(now))
            .map(|p| p.info.clone())
            .collect())
    }

    fn open(&mut self, name: &str) -> Result<Box<dyn Port>, OpenError> {
        let mut st = self.state();
        let now = st.clock.now_us();
        st.opens.push((now, name.to_owned()));
        st.generation += 1;
        let generation = st.generation;
        let Some(p) = st.port_mut(name) else {
            return Err(OpenError::Gone);
        };
        if !p.listed(now) {
            return Err(OpenError::Gone);
        }
        if let Some(e) = p.open_failures.pop_front() {
            return Err(e);
        }
        if let Some(e) = &p.always_fails {
            return Err(e.clone());
        }
        st.ops.push((now, name.to_owned(), "open"));
        let Some(p) = st.port_mut(name) else {
            return Err(OpenError::Gone);
        };
        p.link = Some(Link {
            up: Channel::clean(100),
            down: Channel::clean(100),
            pending: Vec::new(),
            generation,
        });
        Ok(Box::new(FakeOpenPort {
            inner: Arc::clone(&self.inner),
            name: name.to_owned(),
            generation,
        }))
    }

    fn node_group(&self, name: &str) -> Option<String> {
        self.state().port_mut(name).and_then(|p| p.group.clone())
    }
}

struct FakeOpenPort {
    inner: Arc<Mutex<FakeState>>,
    name: String,
    generation: u64,
}

fn gone() -> io::Error {
    io::Error::other("Device not configured")
}

impl FakeOpenPort {
    fn state(&self) -> MutexGuard<'_, FakeState> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Runs the controller side of `p` at `now`: hooks, the app's bytes in,
/// its timers, its frames out.
fn controller_pass(p: &mut FakePort, now: u64) {
    let muted = p.muted(now);
    let Device::Controller(ctrl) = &mut p.device else {
        if let Some(l) = p.link.as_mut() {
            let _ = l.up.deliver(now);
        }
        return;
    };
    let mut i = 0;
    while i < p.hooks.len() {
        if p.hooks.get(i).is_some_and(|h| h.0 <= now) {
            let (_, mut h) = p.hooks.remove(i);
            h(ctrl);
        } else {
            i += 1;
        }
    }
    if muted {
        // Cut off: what the app wrote is lost, nothing comes back.
        if let Some(l) = p.link.as_mut() {
            l.up.flush_hold(now);
            let _ = l.up.deliver(now);
            l.down.flush_hold(now);
            let _ = l.down.deliver(now);
            l.pending.clear();
        }
        return;
    }
    let Some(l) = p.link.as_mut() else {
        ctrl.tick(now);
        return;
    };
    l.up.flush_hold(now);
    let up = l.up.deliver(now);
    ctrl.handle_rx(&up, now);
    ctrl.tick(now);
    while let Some(f) = ctrl.poll_transmit() {
        l.down.send(now, f);
    }
    l.down.flush_hold(now);
}

impl Port for FakeOpenPort {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let generation = self.generation;
        let name = self.name.clone();
        let mut st = self.state();
        while st.hold {
            drop(st);
            std::thread::sleep(Duration::from_millis(1));
            st = self.state();
        }
        let now = st.clock.now_us();
        st.read_calls.push(now);
        let Some(p) = st.port_mut(&name) else {
            return Err(gone());
        };
        if p.error_when_gone && !p.listed(now) {
            return Err(gone());
        }
        if std::mem::replace(&mut p.fail_next_read, false) {
            return Err(io::Error::other("Input/output error"));
        }
        if p.link.as_ref().is_none_or(|l| l.generation != generation) {
            return Err(gone());
        }
        controller_pass(p, now);
        let Some(l) = p.link.as_mut() else {
            return Err(gone());
        };
        let got = l.down.deliver(now);
        l.pending.extend(got);
        let n = l.pending.len().min(buf.len());
        for (d, s) in buf.iter_mut().zip(l.pending.drain(..n)) {
            *d = s;
        }
        if n > 0 {
            st.reads.push((now, name.clone()));
            st.ops.push((now, name, "read"));
        } else if let Some(d) = st.read_sleep {
            drop(st);
            std::thread::sleep(d);
        }
        Ok(n)
    }

    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let generation = self.generation;
        let name = self.name.clone();
        let mut st = self.state();
        let now = st.clock.now_us();
        let Some(p) = st.port_mut(&name) else {
            return Err(gone());
        };
        if p.error_when_gone && !p.listed(now) {
            return Err(gone());
        }
        if p.stall_writes {
            let sleep = st.read_sleep;
            drop(st);
            if let Some(d) = sleep {
                std::thread::sleep(d);
            }
            return Err(io::Error::from(io::ErrorKind::TimedOut));
        }
        let corrupt = std::mem::replace(&mut p.corrupt_next_write, false);
        let Some(l) = p.link.as_mut().filter(|l| l.generation == generation) else {
            return Err(gone());
        };
        let mut chunk = bytes.to_vec();
        if corrupt && chunk.len() > 2 {
            // A frame, not a lone 00: flip a bit of its first byte.
            if let Some(b) = chunk.first_mut() {
                *b ^= 0x10;
            }
        } else if corrupt {
            p.corrupt_next_write = true;
        }
        l.up.send(now, chunk);
        st.writes.push((now, name.clone(), bytes.to_vec()));
        st.ops.push((now, name, "write"));
        Ok(bytes.len())
    }

    fn bytes_to_write(&self) -> io::Result<u32> {
        let name = self.name.clone();
        Ok(self.state().port_mut(&name).map_or(0, |p| p.queued))
    }

    fn set_dtr(&mut self, on: bool) -> io::Result<()> {
        let name = self.name.clone();
        let mut st = self.state();
        let now = st.clock.now_us();
        st.dtr.push((now, name.clone(), on));
        st.ops
            .push((now, name, if on { "dtr_on" } else { "dtr_off" }));
        Ok(())
    }

    fn clear_input(&mut self) -> io::Result<()> {
        let name = self.name.clone();
        let mut st = self.state();
        let now = st.clock.now_us();
        st.clears.push((now, name.clone()));
        st.ops.push((now, name, "clear"));
        Ok(())
    }
}

impl Drop for FakeOpenPort {
    fn drop(&mut self) {
        let name = self.name.clone();
        let generation = self.generation;
        let mut st = self.state();
        let now = st.clock.now_us();
        // DTR is recorded only when the worker lowers it (`set_dtr`), so a
        // test of "DTR falls" can fail.
        st.closes.push((now, name.clone()));
        st.ops.push((now, name.clone(), "close"));
        if let Some(p) = st.port_mut(&name)
            && p.link.as_ref().is_some_and(|l| l.generation == generation)
        {
            p.link = None;
        }
    }
}

/// A backend whose `list` and `open` each block for `delay` of real time
/// (G.13: the UI must not wait on them). It lists one bridge port, whose
/// open then fails.
#[derive(Debug, Clone)]
pub struct BlockingBackend {
    /// How long each call blocks.
    pub delay: Duration,
}

impl Backend for BlockingBackend {
    fn list(&mut self) -> Result<Vec<PortInfo>, String> {
        std::thread::sleep(self.delay);
        Ok(vec![PortInfo {
            name: "/dev/cu.blocking".to_owned(),
            usb: Some(UsbIds {
                vid: BRIDGE_VID,
                pid: BRIDGE_PID,
                serial: Some("B".to_owned()),
                manufacturer: None,
                product: None,
            }),
        }])
    }

    fn open(&mut self, _name: &str) -> Result<Box<dyn Port>, OpenError> {
        std::thread::sleep(self.delay);
        Err(OpenError::Other("blocking backend".to_owned()))
    }
}

/// A worker stepped on the fake clock of a [`FakeBackend`], 1 ms per step
/// (the real loop's pace with a port open), with every event collected.
pub struct SimRig {
    /// The worker.
    pub core: WorkerCore<FakeBackend, FakeClock>,
    /// Its channels.
    pub ends: Endpoints,
    /// The shared fake clock.
    pub clock: FakeClock,
    /// The fake backend (shared state).
    pub backend: FakeBackend,
    /// Every event so far.
    pub events: Vec<TimedEvent>,
    /// The latest snapshot read.
    pub snapshot: Option<Arc<LinkSnapshot>>,
    /// Fake time per step, µs.
    pub tick_us: u64,
}

impl std::fmt::Debug for SimRig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SimRig")
            .field("now_us", &self.clock.now_us())
            .field("events", &self.events.len())
            .finish_non_exhaustive()
    }
}

impl SimRig {
    /// A worker on `backend` (and its clock) with these rules and log.
    #[must_use]
    pub fn new(backend: FakeBackend, rules: Rules, log: LogHandle) -> Self {
        let clock = backend.state().clock.clone();
        let (core, ends) =
            WorkerCore::new(backend.clone(), clock.clone(), rules, log, Box::new(|| {}));
        Self {
            core,
            ends,
            clock,
            backend,
            events: Vec::new(),
            snapshot: None,
            tick_us: 1_000,
        }
    }

    /// One worker pass, then the clock moves one tick.
    pub fn step(&mut self) {
        self.core.step();
        self.events.extend(self.ends.events.try_iter());
        if let Ok(s) = self.ends.snapshots.try_recv() {
            self.snapshot = Some(s);
        }
        self.clock.advance(self.tick_us);
    }

    /// Steps until the clock reaches `t_us`.
    pub fn run_until(&mut self, t_us: u64) {
        while self.clock.now_us() < t_us {
            self.step();
        }
    }

    /// The events matching `f`, with their times.
    pub fn find(&self, f: impl Fn(&WorkerEvent) -> bool) -> Vec<&TimedEvent> {
        self.events.iter().filter(|e| f(&e.event)).collect()
    }
}
