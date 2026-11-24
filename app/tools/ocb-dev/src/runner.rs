//! The driver (Story 3.4 D9): one `Session`, a byte port, a clock and the
//! owner. The same code runs on the bench (`SerialIo`, the terminal) and in
//! the dry run (`ocb-sim`'s fake controller on a fake clock).
//!
//! It is the adapter Story 3.3 describes: received bytes go to
//! `handle_rx`, `handle_timeout` runs only when `poll_timeout` is due, and
//! `poll_transmit` is drained after every input. The tool never builds a
//! frame itself.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ocb_core::link::{LinkEvent, Phase, Session, SessionConfig, StatusView};
use ocb_core::protocol::consts::HAT_DOWN;
use ocb_core::protocol::{Info, Mode, NEUTRAL, NakReason, State};

use crate::soak::Soak;
use crate::tap::{DebugCounters, Tap};
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
    /// Enter was pressed since the last call (or the last `discard_input`).
    fn enter_pressed(&mut self) -> bool;
    /// Forgets every Enter pressed so far (called right before the prompt,
    /// so only an Enter typed after it starts the piece).
    fn discard_input(&mut self);
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

/// Time limits and the first-input shape.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// INFO must arrive within this.
    pub connect_us: u64,
    /// `link_ok` and `console_ready` must come within this.
    pub ready_wait_us: u64,
    /// ... and hold this long (plug bounces).
    pub ready_stable_us: u64,
    /// A STATUS older than this does not count as "ready now".
    pub status_fresh_us: u64,
    /// No STATUS for this long once connected: the controller is gone.
    pub status_silent_us: u64,
    /// The owner presses Enter within this.
    pub enter_wait_us: u64,
    /// STOP (and the first-input SET_STATEs) must be ACKed within this.
    pub stop_wait_us: u64,
    /// After the last event's time, `PLAYBACK_DONE` must come within this.
    pub tail_us: u64,
    /// Feed the session while fewer than this many events are unsent.
    pub backlog: usize,
    /// First input: presses of D-pad down.
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
            status_fresh_us: 150_000,
            status_silent_us: 1_000_000,
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

fn secs(us: u64) -> String {
    let s = us as f64 / 1e6;
    if s.fract() == 0.0 {
        format!("{s:.0} s")
    } else {
        format!("{s:.2} s")
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
    /// The most events ever unsent in the session at once (the prefill).
    pub max_unsent: usize,
    /// The most events unsent right after a refill once START was sent.
    pub max_unsent_streaming: usize,
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
    soak_mode: bool,
    s: Session,
    tap: Tap,
    buf: Vec<u8>,
    info: Option<Info>,
    status: Option<StatusView>,
    status_at: Option<u64>,
    ready_since: Option<u64>,
    playback_done: bool,
    stopped: bool,
    trouble: Option<String>,
    written: u64,
    max_unsent: usize,
    max_unsent_streaming: usize,
    started: bool,
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
        soak_mode: matches!(piece, Piece::Soak { .. }),
        s: Session::new(cfg),
        tap: Tap::new(),
        buf: Vec::with_capacity(512),
        info: None,
        status: None,
        status_at: None,
        ready_since: None,
        playback_done: false,
        stopped: false,
        trouble: None,
        written: 0,
        max_unsent: 0,
        max_unsent_streaming: 0,
        started: false,
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
    let summary = r.summary();
    r.owner.say(&summary);
    r.owner.say(&verdict);
    Outcome {
        code,
        verdict,
        max_unsent: r.max_unsent,
        max_unsent_streaming: r.max_unsent_streaming,
        queued: r.queued,
    }
}

impl<I: Io, O: Owner> Runner<'_, I, O> {
    // ------------------------------------------------------------ the loop

    /// One pass: wait for bytes until `until_us` (or the session's timer),
    /// feed them, run the timers, write, read the events.
    fn pump(&mut self, until_us: u64) -> Result<(), String> {
        let now = self.io.now_us();
        // Wake for the session's timer, the caller's deadline, and the
        // moment a silent controller counts as gone (`check_alive`).
        // Once that moment has passed it bounds nothing: the endings
        // (`finish_stop`, `one_more_status`) still pump to their deadline.
        let silent_at = match (self.info, self.status_at) {
            (Some(_), Some(t)) if t + self.lim.status_silent_us + 1 > now => {
                t + self.lim.status_silent_us + 1
            }
            _ => u64::MAX,
        };
        let wake = self
            .s
            .poll_timeout()
            .map_or(until_us, |t| t.min(until_us))
            .min(silent_at)
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
                self.status_at = Some(now);
            }
            LinkEvent::PlaybackDone => self.playback_done = true,
            LinkEvent::Stopped => self.stopped = true,
            LinkEvent::PieceAbandoned { reason } => {
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
                self.owner
                    .say(&format!("malformed frame of type {ty:#04x} (ignored)"));
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

    /// Once connected, a controller that sends no STATUS for
    /// `status_silent_us` is gone (unplugged, powered off, hung).
    fn check_alive(&mut self) -> Step<()> {
        if self.info.is_none() {
            return Ok(());
        }
        let now = self.io.now_us();
        let last = self.status_at.unwrap_or(now);
        if now > last + self.lim.status_silent_us {
            return Err(Stop::Fail(format!(
                "no STATUS for {}: the controller stopped answering",
                secs(now - last)
            )));
        }
        Ok(())
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
            self.check_alive()?;
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

    /// Waits for one more STATUS (at most 200 ms), so the last STATUS read
    /// was sent after whatever just happened.
    fn one_more_status(&mut self) {
        let seen = self.tap.statuses;
        let end = self.io.now_us() + 200_000;
        while self.tap.statuses == seen && self.io.now_us() < end {
            if self.pump(end).is_err() {
                break;
            }
        }
    }

    // ------------------------------------------------------------ phases

    fn go(&mut self, piece: Piece) -> Step<String> {
        self.connect()?;
        self.wait_ready()?;
        self.check_queue_empty()?;
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
                // The baseline is the controller's state right before the
                // soak, not the session's first STATUS: a bounce or a CRC
                // NAK while waiting for the console is not the soak's.
                let Some(before) = self.tap.last.filter(DebugCounters::has_v2) else {
                    return Err(Stop::Fail(
                        "soak needs a debug controller build with the v2 counters (fw-debug): \
                         STATUS has none"
                            .to_owned(),
                    ));
                };
                let naks_before = self.tap.naks.clone();
                let soak = Soak::new(duration_us, period_us);
                let n = u64::from(soak.len());
                let last = soak.last_t_us();
                let start = self.io.now_us();
                let written0 = self.written;
                self.play(soak, n, last, lead_us)?;
                self.one_more_status();
                self.soak_verdict(start, before, &naks_before, written0)
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
                format!(
                    "no INFO within {}: is the bridge plugged in, the controller powered and wired, \
                     the port the bridge's?",
                    secs(self.lim.connect_us)
                )
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
        let fresh = self.lim.status_fresh_us;
        // The controller is powered by its USB host, so it is plugged in
        // before the tool starts (INFO came from it); only the console
        // handshake can still be pending.
        let hint = if self.soak_mode {
            "WAITING for console_ready: the controller's USB-C on the Mac (the soak's host)"
        } else {
            "WAITING for console_ready: console on, Wired Communication ON, the controller in the \
             console's top USB-C port"
        };
        let ok = self.wait(start + self.lim.ready_wait_us, |r| {
            let now = r.io.now_us();
            if !prompted && now >= start + 1_000_000 && r.ready_since.is_none() {
                prompted = true;
                r.owner.say(hint);
            }
            // Ready for `stable` and still ready now (a STATUS within
            // `fresh`): a controller that went silent after one good
            // STATUS does not pass.
            r.ready_since.is_some_and(|t| now >= t + stable)
                && r.status_at.is_some_and(|t| now <= t + fresh)
        })?;
        self.no_trouble()?;
        if !ok {
            let st = self.status;
            return Err(Stop::Fail(match st {
                None => "no STATUS from the controller".to_owned(),
                Some(s) if !s.link_ok => {
                    "link_ok 0: no bridge heartbeat reaches the controller".to_owned()
                }
                Some(_) => format!(
                    "console_ready never held 1 for {} within {}",
                    secs(stable),
                    secs(self.lim.ready_wait_us)
                ),
            }));
        }
        self.owner.say("link_ok 1 and console_ready 1");
        Ok(())
    }

    /// The `fw-debug-timeline` build loads its table at HELLO (Story 2.5
    /// D38): its queue is not empty before the tool queued anything.
    fn check_queue_empty(&mut self) -> Step<()> {
        let cap = self.info.map_or(0, |i| i.queue_cap);
        match self.status {
            Some(st) if st.mode == Mode::Idle && st.free < cap => Err(Stop::Fail(format!(
                "STATUS free {} < queue_cap {cap} before anything was queued: this controller build \
                 has the built-in timeline (fw-debug-timeline); flash fw-debug",
                st.free
            ))),
            _ => Ok(()),
        }
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
        // Every SET_STATE acknowledged (a refusal would have been trouble).
        let end = self.io.now_us() + self.lim.stop_wait_us;
        if !self.wait(end, |r| r.s.unacked() == 0)? {
            return Err(Stop::Fail(
                "the first-input SET_STATEs were not acknowledged".to_owned(),
            ));
        }
        self.no_trouble()
    }

    fn wait_enter(&mut self) -> Step<()> {
        self.owner.discard_input();
        self.owner.say(
            "PROMPT: dismiss the controller connection screen with the Joy-Con, reopen the ocarina, \
             then press Enter here",
        );
        let deadline = self.io.now_us() + self.lim.enter_wait_us;
        let ok = self.wait(deadline, |r| r.trouble.is_some() || r.owner.enter_pressed())?;
        self.no_trouble()?;
        if !ok {
            return Err(Stop::Fail(format!(
                "no Enter within {}",
                secs(self.lim.enter_wait_us)
            )));
        }
        Ok(())
    }

    fn feed(&mut self, src: &mut impl Iterator<Item = (u32, State)>, upto: usize) -> Step<()> {
        let mut added = false;
        while self.s.events_unsent() < upto {
            let Some((t, st)) = src.next() else { break };
            self.s
                .queue_event(t, st)
                .map_err(|e| Stop::Fail(format!("event at t_us {t} refused locally: {e:?}")))?;
            self.queued += 1;
            added = true;
        }
        let unsent = self.s.events_unsent();
        self.max_unsent = self.max_unsent.max(unsent);
        if added && self.started {
            self.max_unsent_streaming = self.max_unsent_streaming.max(unsent);
        }
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
        self.started = true;
        self.owner.say(&format!(
            "START lead {lead_us} us, {n} events, last at {:.3} s",
            last_t_us as f64 / 1e6
        ));
        let deadline = self.io.now_us() + u64::from(lead_us) + last_t_us + self.lim.tail_us;
        let backlog = self.lim.backlog;
        loop {
            self.check_interrupt()?;
            if let Some(t) = self.trouble.take() {
                let hint = if t.contains("Nak(BadState)") {
                    " (a fw-debug-timeline build refuses every QUEUE_EVENT; flash fw-debug)"
                } else {
                    ""
                };
                return Err(Stop::Fail(format!("{t}{hint}")));
            }
            if self.playback_done {
                break;
            }
            if self.io.now_us() >= deadline {
                return Err(Stop::Fail(format!(
                    "no PLAYBACK_DONE within the piece plus {}",
                    secs(self.lim.tail_us)
                )));
            }
            self.check_alive()?;
            self.feed(&mut src, backlog)?;
            self.pump(deadline).map_err(Stop::Fail)?;
        }
        if self.queued != n {
            return Err(Stop::Fail(format!("{} of {n} events queued", self.queued)));
        }
        let end = self.io.now_us() + self.lim.stop_wait_us;
        if !self.wait(end, |r| r.stopped)? {
            return Err(Stop::Fail(format!(
                "STOP after PLAYBACK_DONE not acknowledged within {}",
                secs(self.lim.stop_wait_us)
            )));
        }
        self.owner.say(&format!(
            "PLAYBACK_DONE: {n} events queued, the END event applied"
        ));
        Ok(())
    }

    fn soak_verdict(
        &mut self,
        start: u64,
        before: DebugCounters,
        naks_before: &BTreeMap<u8, u64>,
        written0: u64,
    ) -> Step<String> {
        let Some(after) = self.tap.last.filter(DebugCounters::has_v2) else {
            return Err(Stop::Fail(
                "the debug counters disappeared during the soak".to_owned(),
            ));
        };
        let duration = (self.io.now_us().saturating_sub(start)) as f64 / 1e6;
        let bytes = self.written - written0;
        let load = if duration > 0.0 {
            100.0 * bytes as f64 / duration / 46_080.0
        } else {
            0.0
        };
        let transport: u64 = self
            .tap
            .naks
            .iter()
            .filter(|(r, _)| NakReason::from_u8(**r).is_transport())
            .map(|(r, n)| n - naks_before.get(r).copied().unwrap_or(0))
            .sum();
        self.owner.say(&format!(
            "SOAK: {duration:.1} s, {} events, {bytes} bytes written ({load:.1} % of 460800 baud), \
             transport NAKs {transport}",
            self.queued
        ));
        self.owner.say(&format!("debug before: {}", before.line()));
        self.owner.say(&format!("debug after:  {}", after.line()));
        let moved = before.moved(&after);
        if !moved.is_empty() {
            return Err(Stop::Fail(format!("counters moved: {}", moved.join(", "))));
        }
        if transport != 0 {
            return Err(Stop::Fail(format!(
                "{transport} transport NAKs during the soak ({})",
                self.tap.naks_line()
            )));
        }
        Ok("PASS".to_owned())
    }

    // ------------------------------------------------------------ endings

    /// One line at the end of every run (Task 6.6).
    fn summary(&self) -> String {
        let st = self.s.send_stats();
        let debug = match (self.tap.first, self.tap.last) {
            (Some(a), Some(b)) => format!(
                "; debug first: dropped_bytes={} watchdog_trips={} tx_dropped={}, last: dropped_bytes={} \
                 watchdog_trips={} tx_dropped={}",
                a.dropped_bytes,
                a.watchdog_trips,
                a.tx_dropped,
                b.dropped_bytes,
                b.watchdog_trips,
                b.tx_dropped
            ),
            _ => String::new(),
        };
        format!(
            "SUMMARY: {} events queued, PLAYBACK_DONE {}, frames sent {}, resends {}, rewinds {}, NAKs {}{debug}",
            self.queued,
            if self.playback_done { "yes" } else { "no" },
            st.sent,
            st.resends,
            st.rewinds,
            self.tap.naks_line()
        )
    }

    /// Waits for the ACK of a STOP in flight (`Phase::Stopping`).
    fn finish_stop(&mut self) -> bool {
        let end = self.io.now_us() + self.lim.stop_wait_us;
        while self.io.now_us() < end {
            if self.stopped || self.s.phase() != Phase::Stopping {
                return self.stopped || self.s.phase() == Phase::Connected;
            }
            if self.pump(end).is_err() {
                return false;
            }
        }
        self.stopped
    }

    /// After a failure: make sure a STOP reached the controller, so nothing
    /// stays held (a STOP the session already started is waited for too).
    fn stop_quietly(&mut self) {
        self.stopped = false;
        match self.s.phase() {
            Phase::Connected => {
                if self.s.stop().is_ok() {
                    let _ = self.finish_stop();
                }
            }
            Phase::Stopping => {
                let _ = self.finish_stop();
            }
            Phase::Closed | Phase::Opening | Phase::Incompatible => {}
        }
    }

    /// Ctrl+C: STOP, wait for its ACK, report the mode.
    fn on_interrupt(&mut self) -> String {
        let phase = self.s.phase();
        if !matches!(phase, Phase::Connected | Phase::Stopping) {
            return format!("INTERRUPTED: the session was not open ({phase:?}), nothing to stop");
        }
        self.stopped = false;
        if phase == Phase::Connected {
            let _ = self.s.stop();
        }
        if !self.finish_stop() {
            let mode = self
                .status
                .map_or_else(|| "unknown".to_owned(), |s| format!("{:?}", s.mode));
            return format!(
                "INTERRUPTED: STOP not acknowledged within {} (last mode {mode}); \
                 unplug the bridge to make the controller go neutral",
                secs(self.lim.stop_wait_us)
            );
        }
        // One more STATUS shows the mode after the STOP.
        self.one_more_status();
        let mode = self
            .status
            .map_or_else(|| "unknown".to_owned(), |s| format!("{:?}", s.mode));
        let idle = self.status.is_some_and(|s| s.mode == Mode::Idle);
        format!(
            "INTERRUPTED: STOP ACKed, controller mode {mode}{}",
            if idle { " (neutral)" } else { "" }
        )
    }
}
