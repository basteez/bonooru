//! The driver (Story 3.4 D9): one `Session`, a byte port, a clock and the
//! owner. The same code runs on the bench (`SerialIo`, the terminal) and in
//! the dry run (`ocb-sim`'s fake controller on a fake clock).
//!
//! It is the adapter Story 3.3 describes: received bytes go to
//! `handle_rx`, `handle_timeout` runs only when `poll_timeout` is due, and
//! `poll_transmit` is drained after every input. The tool never builds a
//! frame itself.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ocb_core::link::{AbandonReason, LinkEvent, Phase, Session, SessionConfig, StatusView};
use ocb_core::protocol::consts::HAT_DOWN;
use ocb_core::protocol::{Info, Mode, NEUTRAL, NakReason, State};

use crate::soak::Soak;
use crate::tap::Tap;
use crate::timeline::Timeline;

/// The byte port and the clock (one object, so a fake clock can move with
/// the fake wire).
pub trait Io {
    /// µs on a monotonic clock.
    fn now_us(&mut self) -> u64;
    /// Appends received bytes to `buf`; waits at most until `until_us` (a
    /// real port waits at most its 1 ms read timeout per call).
    fn read(&mut self, until_us: u64, buf: &mut Vec<u8>) -> Result<(), String>;
    /// Writes all of `bytes`.
    fn write(&mut self, bytes: &[u8]) -> Result<(), String>;
}

/// The person at the bench.
pub trait Owner {
    /// Prints one line.
    fn say(&mut self, line: &str);
    /// Enter was pressed since the last call.
    fn enter_pressed(&mut self) -> bool;
}

/// What to play.
#[derive(Debug, Clone)]
pub enum Piece {
    /// A timeline file (first input and Enter before START).
    Melody(Timeline),
    /// The generated soak (no first input, no prompt).
    Soak {
        /// Length of the soak.
        duration_us: u64,
        /// One event every this many µs.
        period_us: u32,
        /// START lead.
        lead_us: u32,
    },
}

/// Time limits (tests shorten nothing: the fake clock makes them free).
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// INFO must arrive within this.
    pub connect_us: u64,
    /// `link_ok` and `console_ready` must come within this.
    pub ready_wait_us: u64,
    /// ... and hold this long (plug bounces).
    pub ready_stable_us: u64,
    /// The owner presses Enter within this.
    pub enter_wait_us: u64,
    /// STOP must be ACKed within this.
    pub stop_wait_us: u64,
    /// After the last event's time, `PLAYBACK_DONE` must come within this.
    pub tail_us: u64,
    /// Feed the session while fewer than this many events are unsent.
    pub backlog: usize,
    /// First input: presses of D-pad down, held and apart (µs).
    pub presses: u32,
    /// Press length.
    pub press_hold_us: u64,
    /// Release time between presses.
    pub press_gap_us: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            connect_us: 5_000_000,
            ready_wait_us: 120_000_000,
            ready_stable_us: 2_000_000,
            enter_wait_us: 300_000_000,
            stop_wait_us: 2_000_000,
            tail_us: 10_000_000,
            backlog: 256,
            presses: 3,
            press_hold_us: 150_000,
            press_gap_us: 300_000,
        }
    }
}

/// Exit codes (D11).
pub const EXIT_PASS: i32 = 0;
/// A check failed.
pub const EXIT_FAIL: i32 = 1;
/// Usage or file error (before the port opens).
pub const EXIT_USAGE: i32 = 2;
/// Ctrl+C.
pub const EXIT_INTERRUPTED: i32 = 130;

/// How a run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// The exit code.
    pub code: i32,
    /// `PASS`, `FAIL <reason>` or `INTERRUPTED …`.
    pub verdict: String,
    /// The most events ever unsent in the session at once.
    pub max_unsent: usize,
    /// Events handed to the session.
    pub queued: u64,
}

enum Stop {
    Fail(String),
    Interrupted,
}

type Step<T> = Result<T, Stop>;

struct Runner<'a, I: Io, O: Owner> {
    io: &'a mut I,
    owner: &'a mut O,
    interrupt: Arc<AtomicBool>,
    lim: Limits,
    s: Session,
    tap: Tap,
    buf: Vec<u8>,
    info: Option<Info>,
    status: Option<StatusView>,
    ready_since: Option<u64>,
    playback_done: bool,
    stopped: bool,
    trouble: Option<String>,
    timeline_build: bool,
    written: u64,
    max_unsent: usize,
    queued: u64,
}

/// Runs one piece. The port is already open with DTR asserted.
pub fn run<I: Io, O: Owner>(
    io: &mut I,
    owner: &mut O,
    interrupt: Arc<AtomicBool>,
    piece: Piece,
    lim: Limits,
    cfg: SessionConfig,
) -> Outcome {
    let mut r = Runner {
        io,
        owner,
        interrupt,
        lim,
        s: Session::new(cfg),
        tap: Tap::new(),
        buf: Vec::with_capacity(512),
        info: None,
        status: None,
        ready_since: None,
        playback_done: false,
        stopped: false,
        trouble: None,
        timeline_build: false,
        written: 0,
        max_unsent: 0,
        queued: 0,
    };
    let res = r.go(piece);
    let (code, verdict) = match res {
        Ok(v) => (EXIT_PASS, v),
        Err(Stop::Fail(why)) => {
            r.stop_quietly();
            (EXIT_FAIL, format!("FAIL {why}"))
        }
        Err(Stop::Interrupted) => {
            let line = r.on_interrupt();
            (EXIT_INTERRUPTED, line)
        }
    };
    r.owner.say(&verdict);
    Outcome {
        code,
        verdict,
        max_unsent: r.max_unsent,
        queued: r.queued,
    }
}

impl<I: Io, O: Owner> Runner<'_, I, O> {
    // ------------------------------------------------------------ the loop

    /// One pass: wait for bytes until `until_us` (or the session's timer),
    /// feed them, run the timers, write, read the events.
    fn pump(&mut self, until_us: u64) -> Result<(), String> {
        let now = self.io.now_us();
        let wake = self
            .s
            .poll_timeout()
            .map_or(until_us, |t| t.min(until_us))
            .max(now);
        self.buf.clear();
        let mut buf = std::mem::take(&mut self.buf);
        let read = self.io.read(wake, &mut buf);
        let now = self.io.now_us();
        if !buf.is_empty() {
            self.tap.feed(&buf);
            self.s.handle_rx(&buf, now);
        }
        self.buf = buf;
        read?;
        if self.s.poll_timeout().is_some_and(|t| t <= now) {
            self.s.handle_timeout(now);
        }
        while let Some(chunk) = self.s.poll_transmit(now) {
            self.io.write(&chunk)?;
            self.written += chunk.len() as u64;
        }
        while let Some(e) = self.s.poll_event() {
            self.on_event(e, now);
        }
        Ok(())
    }

    fn on_event(&mut self, e: LinkEvent, now: u64) {
        match e {
            LinkEvent::Connected(info) => self.info = Some(info),
            LinkEvent::Status(st) => {
                if st.link_ok && st.console_ready {
                    self.ready_since.get_or_insert(now);
                } else {
                    self.ready_since = None;
                }
                self.status = Some(st);
            }
            LinkEvent::PlaybackDone => self.playback_done = true,
            LinkEvent::Stopped => self.stopped = true,
            LinkEvent::PieceAbandoned { reason } => {
                if reason == AbandonReason::Nak(NakReason::BadState) {
                    self.timeline_build = true;
                }
                self.trouble
                    .get_or_insert_with(|| format!("piece abandoned: {reason:?}"));
            }
            LinkEvent::Refused { reason, ty } => {
                self.trouble
                    .get_or_insert_with(|| format!("type {ty:#04x} refused: NAK {reason:?}"));
            }
            LinkEvent::ControllerRebooted => {
                self.trouble
                    .get_or_insert_with(|| "the controller rebooted".to_owned());
            }
            LinkEvent::RequiredUpdate {
                controller_proto,
                app_proto,
                ..
            } => {
                self.trouble.get_or_insert_with(|| {
                    format!(
                        "protocol {controller_proto} on the controller, {app_proto} here: flash the matching firmware"
                    )
                });
            }
            LinkEvent::RecommendedUpdate {
                fw_ver, bridge_ver, ..
            } => self.owner.say(&format!(
                "note: firmware versions differ from this tool (controller {fw_ver:?}, bridge {bridge_ver:?})"
            )),
            LinkEvent::ControllerEvent(code) => self.owner.say(&format!("EVENT {code:?}")),
            LinkEvent::Malformed { ty } => {
                self.owner.say(&format!("malformed frame of type {ty:#04x} (ignored)"));
            }
            LinkEvent::UnknownNak { reason, .. } => {
                self.owner.say(&format!("unknown NAK reason {reason:#04x}"));
            }
        }
    }

    fn check_interrupt(&self) -> Step<()> {
        if self.interrupt.load(Ordering::SeqCst) {
            Err(Stop::Interrupted)
        } else {
            Ok(())
        }
    }

    /// Pumps until `done` or `deadline`; `Ok(false)` at the deadline.
    fn wait(&mut self, deadline: u64, mut done: impl FnMut(&mut Self) -> bool) -> Step<bool> {
        loop {
            self.check_interrupt()?;
            if done(self) {
                return Ok(true);
            }
            if self.io.now_us() >= deadline {
                return Ok(false);
            }
            self.pump(deadline).map_err(Stop::Fail)?;
        }
    }

    /// Pumps for `us`, failing on any trouble the session reports.
    fn hold(&mut self, us: u64) -> Step<()> {
        let end = self.io.now_us() + us;
        self.wait(end, |_| false)?;
        self.no_trouble()
    }

    fn no_trouble(&mut self) -> Step<()> {
        match self.trouble.take() {
            Some(t) => Err(Stop::Fail(t)),
            None => Ok(()),
        }
    }

    // ------------------------------------------------------------ phases

    fn go(&mut self, piece: Piece) -> Step<String> {
        self.connect()?;
        self.wait_ready()?;
        match piece {
            Piece::Melody(tl) => {
                self.first_input()?;
                self.wait_enter()?;
                let n = tl.events.len() as u64;
                let last = tl.events.last().map_or(0, |e| u64::from(e.0));
                self.play(tl.events.into_iter(), n, last, tl.lead_us)?;
                Ok("PASS".to_owned())
            }
            Piece::Soak {
                duration_us,
                period_us,
                lead_us,
            } => {
                let soak = Soak::new(duration_us, period_us);
                let n = u64::from(soak.len());
                let last = soak.last_t_us();
                let start = self.io.now_us();
                let first = self.tap.first;
                let written0 = self.written;
                self.play(soak, n, last, lead_us)?;
                self.soak_verdict(start, first, written0)
            }
        }
    }

    fn connect(&mut self) -> Step<()> {
        let now = self.io.now_us();
        self.s.open(now);
        let deadline = now + self.lim.connect_us;
        let ok = self.wait(deadline, |r| {
            r.info.is_some() || r.s.phase() == Phase::Incompatible
        })?;
        self.no_trouble()?;
        let Some(info) = self.info else {
            return Err(Stop::Fail(if ok {
                "incompatible controller firmware".to_owned()
            } else {
                "no INFO within 5 s: is the bridge plugged in, the controller powered and wired, \
                 the port the bridge's?"
                    .to_owned()
            }));
        };
        let host = match info.report_period_us {
            0 => "no USB host polling yet",
            7_000..=9_000 => "the console",
            1_500..=2_500 => "a computer (macOS completes the handshake itself)",
            _ => "unknown host",
        };
        self.owner.say(&format!(
            "INFO fw_ver {:?} bridge_ver {:?} queue_cap {} app_timeout_ms {} report_period_us {} ({host})",
            info.fw_ver, info.bridge_ver, info.queue_cap, info.app_timeout_ms, info.report_period_us
        ));
        Ok(())
    }

    fn wait_ready(&mut self) -> Step<()> {
        let start = self.io.now_us();
        let mut prompted = false;
        let stable = self.lim.ready_stable_us;
        let ok = self.wait(start + self.lim.ready_wait_us, |r| {
            let now = r.io.now_us();
            if !prompted && now >= start + 1_000_000 && r.ready_since.is_none() {
                prompted = true;
                r.owner.say(
                    "WAITING: plug the controller into the Switch's top USB-C port (Wired Communication ON); \
                     the bridge needs its own wire to the controller",
                );
            }
            r.ready_since.is_some_and(|t| now >= t + stable)
        })?;
        self.no_trouble()?;
        if !ok {
            let st = self.status;
            return Err(Stop::Fail(match st {
                None => "no STATUS from the controller".to_owned(),
                Some(s) if !s.link_ok => {
                    "link_ok 0: no bridge heartbeat reaches the controller".to_owned()
                }
                Some(_) => "console_ready never held 1 for 2 s within 120 s".to_owned(),
            }));
        }
        self.owner.say("link_ok 1 and console_ready 1");
        Ok(())
    }

    fn first_input(&mut self) -> Step<()> {
        let down = State {
            hat: HAT_DOWN,
            ..NEUTRAL
        };
        for i in 0..self.lim.presses {
            self.s
                .set_state(down)
                .map_err(|e| Stop::Fail(format!("SET_STATE refused locally: {e:?}")))?;
            self.hold(self.lim.press_hold_us)?;
            self.s
                .set_state(NEUTRAL)
                .map_err(|e| Stop::Fail(format!("SET_STATE refused locally: {e:?}")))?;
            self.hold(self.lim.press_gap_us)?;
            self.owner.say(&format!(
                "first input: D-pad down {}/{}",
                i + 1,
                self.lim.presses
            ));
        }
        Ok(())
    }

    fn wait_enter(&mut self) -> Step<()> {
        self.owner.say(
            "PROMPT: dismiss the controller connection screen with the Joy-Con, reopen the ocarina, \
             then press Enter here",
        );
        let deadline = self.io.now_us() + self.lim.enter_wait_us;
        let ok = self.wait(deadline, |r| r.trouble.is_some() || r.owner.enter_pressed())?;
        self.no_trouble()?;
        if !ok {
            return Err(Stop::Fail("no Enter within 300 s".to_owned()));
        }
        Ok(())
    }

    fn feed(&mut self, src: &mut impl Iterator<Item = (u32, State)>, upto: usize) -> Step<()> {
        while self.s.events_unsent() < upto {
            let Some((t, st)) = src.next() else { break };
            self.s
                .queue_event(t, st)
                .map_err(|e| Stop::Fail(format!("event at t_us {t} refused locally: {e:?}")))?;
            self.queued += 1;
        }
        self.max_unsent = self.max_unsent.max(self.s.events_unsent());
        Ok(())
    }

    fn play(
        &mut self,
        mut src: impl Iterator<Item = (u32, State)>,
        n: u64,
        last_t_us: u64,
        lead_us: u32,
    ) -> Step<()> {
        let cap = self
            .info
            .as_ref()
            .map_or(1, |i| usize::from(i.queue_cap).max(1));
        self.feed(&mut src, cap)?;
        self.s
            .start(lead_us)
            .map_err(|e| Stop::Fail(format!("START refused locally: {e:?}")))?;
        self.owner.say(&format!(
            "START lead {lead_us} us, {n} events, last at {:.3} s",
            last_t_us as f64 / 1e6
        ));
        let deadline = self.io.now_us() + u64::from(lead_us) + last_t_us + self.lim.tail_us;
        let backlog = self.lim.backlog;
        loop {
            self.check_interrupt()?;
            if self.trouble.is_some() {
                if self.timeline_build {
                    return Err(Stop::Fail(
                        "QUEUE_EVENT refused with NAK BAD_STATE: this controller build has the built-in \
                         timeline (fw-debug-timeline); flash fw-debug"
                            .to_owned(),
                    ));
                }
                self.no_trouble()?;
            }
            if self.playback_done {
                break;
            }
            if self.io.now_us() >= deadline {
                return Err(Stop::Fail(format!(
                    "no PLAYBACK_DONE within the piece plus {} s",
                    self.lim.tail_us / 1_000_000
                )));
            }
            self.feed(&mut src, backlog)?;
            self.pump(deadline).map_err(Stop::Fail)?;
        }
        if self.queued != n {
            return Err(Stop::Fail(format!("{} of {n} events queued", self.queued)));
        }
        let end = self.io.now_us() + self.lim.stop_wait_us;
        if !self.wait(end, |r| r.stopped)? {
            return Err(Stop::Fail(
                "STOP after PLAYBACK_DONE not acknowledged".to_owned(),
            ));
        }
        let st = self.s.send_stats();
        self.owner.say(&format!(
            "PLAYBACK_DONE: {n} events queued and played; resends {} rewinds {}; NAKs {}",
            st.resends,
            st.rewinds,
            self.tap.naks_line()
        ));
        Ok(())
    }

    fn soak_verdict(
        &mut self,
        start: u64,
        first_before: Option<crate::tap::DebugCounters>,
        written0: u64,
    ) -> Step<String> {
        let first = first_before.or(self.tap.first);
        let (Some(first), Some(last)) = (first, self.tap.last) else {
            return Err(Stop::Fail(
                "soak needs a debug controller build (fw-debug): STATUS has no debug counters"
                    .to_owned(),
            ));
        };
        if !first.has_v2() || !last.has_v2() {
            return Err(Stop::Fail(
                "soak needs the v2 debug counters (fw-debug from Story 2.6 on)".to_owned(),
            ));
        }
        let secs = (self.io.now_us().saturating_sub(start)) as f64 / 1e6;
        let bytes = self.written - written0;
        let load = if secs > 0.0 {
            100.0 * bytes as f64 / secs / 46_080.0
        } else {
            0.0
        };
        self.owner.say(&format!(
            "SOAK: {:.1} s, {} events, {bytes} bytes written ({load:.1} % of 460800 baud), transport NAKs {}",
            secs,
            self.queued,
            self.tap.transport_naks()
        ));
        self.owner.say(&format!("debug first: {}", first.line()));
        self.owner.say(&format!("debug last:  {}", last.line()));
        let moved = first.moved(&last);
        if !moved.is_empty() {
            return Err(Stop::Fail(format!("counters moved: {}", moved.join(", "))));
        }
        if self.tap.transport_naks() != 0 {
            return Err(Stop::Fail(format!(
                "transport NAKs during the soak: {}",
                self.tap.naks_line()
            )));
        }
        Ok("PASS".to_owned())
    }

    // ------------------------------------------------------------ endings

    /// After a failure: STOP if a session is open, so nothing stays held.
    fn stop_quietly(&mut self) {
        if self.s.phase() == Phase::Connected && self.s.stop().is_ok() {
            let end = self.io.now_us() + self.lim.stop_wait_us;
            self.stopped = false;
            while !self.stopped && self.io.now_us() < end {
                if self.pump(end).is_err() {
                    break;
                }
            }
        }
    }

    /// Ctrl+C: STOP, wait for its ACK, report the mode.
    fn on_interrupt(&mut self) -> String {
        if !matches!(self.s.phase(), Phase::Connected | Phase::Stopping) {
            return "INTERRUPTED before the session opened".to_owned();
        }
        if self.s.phase() == Phase::Connected {
            let _ = self.s.stop();
        }
        self.stopped = false;
        let end = self.io.now_us() + self.lim.stop_wait_us;
        let mut acked = false;
        while self.io.now_us() < end {
            if self.pump(end).is_err() {
                break;
            }
            if self.stopped {
                acked = true;
                // One more STATUS shows the mode after the STOP.
                let seen = self.tap.statuses;
                let end2 = (self.io.now_us() + 200_000).min(end.max(self.io.now_us() + 200_000));
                while self.tap.statuses == seen && self.io.now_us() < end2 {
                    if self.pump(end2).is_err() {
                        break;
                    }
                }
                break;
            }
        }
        let mode = self
            .status
            .map_or_else(|| "unknown".to_owned(), |s| format!("{:?}", s.mode));
        if acked {
            let idle = self.status.is_some_and(|s| s.mode == Mode::Idle);
            format!(
                "INTERRUPTED: STOP ACKed, controller mode {mode}{}",
                if idle { " (neutral)" } else { "" }
            )
        } else {
            format!("INTERRUPTED: STOP not acknowledged within 2 s (last mode {mode})")
        }
    }
}
