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

    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        let now = self.now.get();
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

/// A fw-debug style debug tail (v1 + v2, 21 bytes) with `dropped_bytes`
/// = `rx_err_bytes` = `dropped` (the boot break).
pub fn debug_tail(dropped: u16) -> Vec<u8> {
    let mut t = vec![0u8; 21];
    t[3..5].copy_from_slice(&dropped.to_le_bytes());
    t[7..9].copy_from_slice(&dropped.to_le_bytes());
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
