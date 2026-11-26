//! The dry run (Story 3.4 Task 8, retro action #27): the runner of the bench
//! tool against `ocb-sim`'s fake controller, through two channels, on a fake
//! clock that only moves when the runner waits.
#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ocb_core::link::SessionConfig;
use ocb_core::protocol::{RxSide, decode_frame};
use ocb_dev::runner::{self, Io, Limits, Outcome, Owner, Piece};
use ocb_sim::{Channel, ControllerConfig, FakeController, Impair};

/// A scheduled change of the fake controller.
pub type Hook = Box<dyn FnMut(&mut FakeController)>;

/// The fake wire and clock.
pub struct SimIo {
    pub ctrl: FakeController,
    pub up: Channel,
    pub down: Channel,
    pub now: Rc<Cell<u64>>,
    pub step_us: u64,
    /// Every frame type the tool wrote, with the time.
    pub written: Vec<(u64, u8)>,
    /// Raise the interrupt flag at this time.
    pub interrupt_at: Option<(u64, Arc<AtomicBool>)>,
    /// Run these at their time (once).
    pub hooks: Vec<(u64, Hook)>,
    /// From this time on the controller's bytes are lost (it went silent).
    pub silence_at: Option<u64>,
    /// A.3: losses to make, one per round, 2 s after the piece starts
    /// playing; the bridge or the wire comes back 1 s later.
    pub loss_plan: Vec<Loss>,
    /// This many pieces play to the end before the plan starts.
    pub loss_skip: u32,
    this_play_skips: bool,
    /// The bridge is unplugged: the port fails.
    pub unplugged: bool,
    playing_since: Option<u64>,
    restore_at: Option<(u64, Loss)>,
    /// H.3: the console sleeps in this window (both directions silent,
    /// the controller reboots at the end, `console_ready` 0 for 1.5 s).
    pub sleep: Option<(u64, u64)>,
    /// After the sleep the controller comes back with the link down (no
    /// heartbeat reaches it): its STATUS is not "fresh" and no probe can run.
    pub wake_link_down: bool,
    /// The debug tail given with `set_debug` (kept across a reboot).
    pub debug_extra: Option<Vec<u8>>,
    /// The controller's bytes are lost in this window (a STATUS gap).
    pub down_gap: Option<(u64, u64)>,
}

/// How a failsafe round ends in the dry run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Loss {
    Unplug,
    Wire,
    /// The bridge is unplugged but the controller never notices (a broken
    /// watchdog): the round must fail.
    UnplugUnnoticed,
}

impl SimIo {
    pub fn new(cfg: ControllerConfig, up: Channel, down: Channel) -> Self {
        Self {
            ctrl: FakeController::new(cfg),
            up,
            down,
            now: Rc::new(Cell::new(1_000)),
            step_us: 1_000,
            written: Vec::new(),
            interrupt_at: None,
            hooks: Vec::new(),
            silence_at: None,
            loss_plan: Vec::new(),
            loss_skip: 0,
            this_play_skips: false,
            unplugged: false,
            playing_since: None,
            restore_at: None,
            sleep: None,
            wake_link_down: false,
            debug_extra: None,
            down_gap: None,
        }
    }

    /// Sets the controller's debug tail (and keeps it across a reboot).
    pub fn set_debug(&mut self, extra: Option<Vec<u8>>) {
        self.debug_extra = extra.clone();
        self.ctrl.set_debug_extra(extra);
    }

    /// A.3 in the dry run: 2 s into a playing piece, the next planned loss;
    /// 1 s later the bridge (a fresh port) or the wire comes back.
    fn losses(&mut self, now: u64) {
        if let Some((t, kind)) = self.restore_at
            && now >= t
        {
            self.restore_at = None;
            if kind != Loss::Wire {
                self.unplugged = false;
                self.up = Channel::clean(100);
                self.down = Channel::clean(100);
            }
            self.ctrl.set_link_ok(true);
        }
        if self.ctrl.mode() == ocb_core::protocol::Mode::Playing {
            if self.playing_since.is_none() {
                self.this_play_skips = self.loss_skip > 0;
                self.loss_skip = self.loss_skip.saturating_sub(1);
            }
            let since = *self.playing_since.get_or_insert(now);
            if self.restore_at.is_none()
                && !self.this_play_skips
                && now >= since + 2_000_000
                && !self.loss_plan.is_empty()
            {
                let kind = self.loss_plan.remove(0);
                if kind != Loss::Wire {
                    self.unplugged = true;
                }
                if kind != Loss::UnplugUnnoticed {
                    self.ctrl.set_link_ok(false);
                }
                self.restore_at = Some((now + 1_000_000, kind));
            }
        } else {
            self.playing_since = None;
        }
    }

    pub fn clean(cfg: ControllerConfig) -> Self {
        Self::new(cfg, Channel::clean(100), Channel::clean(100))
    }

    pub fn lossy(cfg: ControllerConfig, p: f64, seed: u64) -> Self {
        Self::new(
            cfg,
            Channel::new(Impair::all(p), 100, seed),
            Channel::new(Impair::all(p), 100, seed.wrapping_mul(31).wrapping_add(7)),
        )
    }

    pub fn written_of(&self, ty: u8) -> Vec<u64> {
        self.written
            .iter()
            .filter(|w| w.1 == ty)
            .map(|w| w.0)
            .collect()
    }

    fn controller_pass(&mut self, now: u64) {
        if let Some((t, flag)) = &self.interrupt_at
            && now >= *t
        {
            flag.store(true, Ordering::SeqCst);
        }
        let mut i = 0;
        while i < self.hooks.len() {
            if self.hooks[i].0 <= now {
                let (_, mut h) = self.hooks.remove(i);
                h(&mut self.ctrl);
            } else {
                i += 1;
            }
        }
        self.losses(now);
        if let Some((a, b)) = self.sleep {
            if now >= a && now < b {
                let off = Impair {
                    drop: 1.0,
                    ..Impair::default()
                };
                self.up.set_impair(off);
                self.down.set_impair(off);
            } else if now >= b {
                self.sleep = None;
                self.up = Channel::clean(100);
                self.down = Channel::clean(100);
                let extra = self.debug_extra.clone();
                self.ctrl.reboot();
                self.ctrl.set_debug_extra(extra);
                self.ctrl.set_console_ready(false);
                if self.wake_link_down {
                    self.ctrl.set_link_ok(false);
                } else {
                    self.hooks
                        .push((now + 1_500_000, Box::new(|c| c.set_console_ready(true))));
                }
            }
        }
        if let Some((a, b)) = self.down_gap {
            let drop = if now >= a && now < b { 1.0 } else { 0.0 };
            self.down.set_impair(Impair {
                drop,
                ..Impair::default()
            });
            if now >= b {
                self.down_gap = None;
            }
        }
        if self.silence_at.is_some_and(|t| now >= t) {
            self.down.set_impair(Impair {
                drop: 1.0,
                ..Impair::default()
            });
        }
        self.up.flush_hold(now);
        let up = self.up.deliver(now);
        self.ctrl.handle_rx(&up, now);
        self.ctrl.tick(now);
        while let Some(f) = self.ctrl.poll_transmit() {
            self.down.send(now, f);
        }
        self.down.flush_hold(now);
    }
}

impl Io for SimIo {
    fn now_us(&mut self) -> u64 {
        self.now.get()
    }

    fn read(&mut self, until_us: u64, buf: &mut Vec<u8>) -> Result<(), String> {
        loop {
            let now = self.now.get();
            self.controller_pass(now);
            if self.unplugged {
                return Err("serial read: Device not configured".to_owned());
            }
            let got = self.down.deliver(now);
            if !got.is_empty() {
                buf.extend_from_slice(&got);
                return Ok(());
            }
            if now >= until_us {
                return Ok(());
            }
            let mut next = (now + self.step_us).min(until_us);
            for t in [self.up.next_due(), self.down.next_due()]
                .into_iter()
                .flatten()
            {
                if t > now && t < next {
                    next = t;
                }
            }
            self.now.set(next.max(now + 1));
        }
    }

    fn reopen(&mut self) -> Result<bool, String> {
        let now = self.now.get();
        self.controller_pass(now);
        Ok(!self.unplugged)
    }

    fn idle_until(&mut self, t_us: u64) {
        while self.now.get() < t_us {
            let now = self.now.get();
            self.controller_pass(now);
            let _ = self.down.deliver(now); // nobody listens: the port is gone
            self.now.set((now + self.step_us).min(t_us));
        }
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        let now = self.now.get();
        if self.unplugged {
            return Err("serial write: Device not configured".to_owned());
        }
        if let Some((&0, enc)) = bytes.split_last()
            && let Ok(f) = decode_frame(RxSide::Controller, enc)
        {
            self.written.push((now, f.ty));
        }
        self.up.send(now, bytes.to_vec());
        Ok(())
    }
}

/// The owner at the bench: presses Enter (edge-triggered, like a line on
/// stdin) at set fake times: `early` absolute times, and once
/// `enter_after_prompt` after the prompt.
pub struct ScriptedOwner {
    pub now: Rc<Cell<u64>>,
    pub enter_after_prompt: Option<u64>,
    pub presses: Vec<u64>,
    pub lines: Vec<String>,
    pub prompted_at: Option<u64>,
}

impl ScriptedOwner {
    pub fn new(now: Rc<Cell<u64>>, enter_after_prompt_us: Option<u64>) -> Self {
        Self {
            now,
            enter_after_prompt: enter_after_prompt_us,
            presses: Vec::new(),
            lines: Vec::new(),
            prompted_at: None,
        }
    }

    fn take_due(&mut self) -> bool {
        let now = self.now.get();
        let before = self.presses.len();
        self.presses.retain(|&t| t > now);
        self.presses.len() != before
    }

    pub fn text(&self) -> String {
        self.lines.join("\n")
    }
}

impl Owner for ScriptedOwner {
    fn say(&mut self, line: &str) {
        if line.starts_with("PROMPT") {
            let now = self.now.get();
            self.prompted_at = Some(now);
            if let Some(after) = self.enter_after_prompt {
                self.presses.push(now + after);
            }
        }
        self.lines.push(line.to_owned());
    }

    fn enter_pressed(&mut self) -> bool {
        self.take_due()
    }

    fn discard_input(&mut self) {
        let _ = self.take_due();
    }
}

/// Runs a piece on `io` with the owner pressing Enter `enter_after_us`
/// after the prompt.
pub fn run(
    io: &mut SimIo,
    piece: Piece,
    enter_after_us: Option<u64>,
    interrupt: Arc<AtomicBool>,
) -> (Outcome, ScriptedOwner) {
    run_with(io, piece, enter_after_us, Vec::new(), interrupt)
}

/// As `run`, with Enter also pressed at these absolute times.
pub fn run_with(
    io: &mut SimIo,
    piece: Piece,
    enter_after_us: Option<u64>,
    early_presses: Vec<u64>,
    interrupt: Arc<AtomicBool>,
) -> (Outcome, ScriptedOwner) {
    let mut owner = ScriptedOwner::new(Rc::clone(&io.now), enter_after_us);
    owner.presses = early_presses;
    let out = runner::run(
        io,
        &mut owner,
        interrupt,
        piece,
        Limits::default(),
        SessionConfig::default(),
    );
    (out, owner)
}

/// A Story 4.0 debug tail (v1 + v2, an empty v3 window, v4, v5, v6: 72
/// bytes) with `dropped_bytes` = `rx_err_bytes` = `dropped` (the boot
/// break); the fake controller fills the v5 and v6 blocks itself.
pub fn debug_tail(dropped: u16) -> Vec<u8> {
    let mut t = debug_tail_3_5(dropped);
    t.resize(72, 0);
    t
}

/// A Story 3.5 debug tail (no v6 block: 70 bytes), so the shorter tail of
/// an older debug build stays covered.
pub fn debug_tail_3_5(dropped: u16) -> Vec<u8> {
    let mut t = vec![0u8; 70];
    t[3..5].copy_from_slice(&dropped.to_le_bytes());
    t[7..9].copy_from_slice(&dropped.to_le_bytes());
    t
}

/// A fw-debug-o3 style tail: v1 + v2, an empty P2 log window, and the v4
/// block with `in_max_us` and `in_slow`.
pub fn debug_tail_v4(in_max_us: u16, in_slow: u16) -> Vec<u8> {
    let mut t = debug_tail(1);
    t[23] = 0; // p2_log_n
    t[24 + 28..24 + 30].copy_from_slice(&in_max_us.to_le_bytes());
    t[24 + 30..24 + 32].copy_from_slice(&in_slow.to_le_bytes());
    t
}

/// The committed melody.
pub fn melody_text() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/timelines/ode-to-joy-90bpm.csv"
    ))
    .unwrap()
}
