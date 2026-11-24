//! The simulation driver: one `Session`, one `FakeController`, a channel each
//! way and a fake clock. After every step it checks the session's
//! invariants independently of the session's own bookkeeping (C.7).

use std::collections::BTreeSet;

use ocb_core::link::{LinkEvent, Phase, Session, SessionConfig};
use ocb_core::protocol::consts::{ACK_WINDOW, MSG_HELLO, MSG_NAK, MSG_QUEUE_EVENT, MSG_STOP};
use ocb_core::protocol::{Message, NakReason, RxEvent, RxSide, Splitter, decode_frame};

use crate::channel::Channel;
use crate::controller::{ControllerConfig, FakeController};

/// What the invariant checker saw.
#[derive(Debug, Clone, Default)]
pub struct Observed {
    /// Every chunk the session sent: (time, bytes).
    pub tx: Vec<(u64, Vec<u8>)>,
    /// Every event the session reported: (time, event).
    pub events: Vec<(u64, LinkEvent)>,
    /// Transport NAKs delivered to the session.
    pub transport_naks: u64,
    /// Distinct frames (sync epoch, index) the delivered transport NAKs point at.
    pub transport_nak_points: BTreeSet<(u64, u64)>,
    /// Invariant violations (empty when all is well).
    pub violations: Vec<String>,
}

/// The driver.
#[derive(Debug)]
pub struct Sim {
    /// The app side.
    pub session: Session,
    /// The controller side.
    pub ctrl: FakeController,
    /// App → controller.
    pub up: Channel,
    /// Controller → app.
    pub down: Channel,
    /// The fake clock, µs.
    pub now: u64,
    /// The largest clock step, µs.
    pub step_us: u64,
    /// What happened.
    pub seen: Observed,
    sync_seq: Option<u8>,
    sent_new: Vec<(u8, bool)>,
    sync_epoch: u64,
    down_tap: Splitter,
}

impl Sim {
    /// A simulation with clean channels (100 µs latency) and 1 ms steps.
    #[must_use]
    pub fn new(cfg: SessionConfig, ctrl: ControllerConfig) -> Self {
        Self::with_channels(cfg, ctrl, Channel::clean(100), Channel::clean(100))
    }

    /// A simulation with these channels.
    #[must_use]
    pub fn with_channels(
        cfg: SessionConfig,
        ctrl: ControllerConfig,
        up: Channel,
        down: Channel,
    ) -> Self {
        Self {
            session: Session::new(cfg),
            ctrl: FakeController::new(ctrl),
            up,
            down,
            now: 1_000,
            step_us: 1_000,
            seen: Observed::default(),
            sync_seq: None,
            sent_new: Vec::new(),
            sync_epoch: 0,
            down_tap: Splitter::new(RxSide::App),
        }
    }

    fn violation(&mut self, msg: String) {
        if self.seen.violations.len() < 50 {
            self.seen
                .violations
                .push(format!("t={} µs: {msg}", self.now));
        }
    }

    /// Checks one chunk the session sent.
    fn check_tx(&mut self, chunk: &[u8], phase: Phase) {
        if chunk == [0] {
            return;
        }
        let Some((&0, enc)) = chunk.split_last() else {
            self.violation(format!("chunk without a delimiter: {chunk:02x?}"));
            return;
        };
        let f = match decode_frame(RxSide::Controller, enc) {
            Ok(f) => f,
            Err(e) => {
                self.violation(format!("the session sent a bad frame: {e}"));
                return;
            }
        };
        match phase {
            Phase::Opening if f.ty != MSG_HELLO => {
                self.violation(format!("type {:#04x} sent before INFO", f.ty));
            }
            Phase::Stopping if f.ty != MSG_STOP => {
                self.violation(format!("type {:#04x} sent between STOP and its ACK", f.ty));
            }
            Phase::Closed | Phase::Incompatible => {
                self.violation(format!("type {:#04x} sent in phase {phase:?}", f.ty));
            }
            _ => {}
        }
        if f.ty == MSG_HELLO || f.ty == MSG_STOP {
            if self.sync_seq != Some(f.seq) || !self.sent_new.is_empty() {
                self.sync_epoch += 1;
            }
            self.sync_seq = Some(f.seq);
            self.sent_new.clear();
            return;
        }
        let Some(sync) = self.sync_seq else {
            self.violation("app frame before any HELLO".to_owned());
            return;
        };
        let next_new = sync.wrapping_add(1).wrapping_add(self.sent_new.len() as u8);
        if f.seq != next_new {
            return; // a resend
        }
        let qe = f.ty == MSG_QUEUE_EVENT;
        self.sent_new.push((f.seq, qe));
        if !qe {
            return;
        }
        // Credits (AD-6): QUEUE_EVENTs sent after the last STATUS's
        // `last_seq` must not exceed its `free`.
        let Some(st) = self.session.status().copied() else {
            self.violation("QUEUE_EVENT before any STATUS".to_owned());
            return;
        };
        // The newest frame with that seq (a STATUS lags by at most the
        // window, so the newest is the right one), else the sync point.
        let pos = match self.sent_new.iter().rposition(|(s, _)| *s == st.last_seq) {
            Some(p) => Some(Some(p)),
            None if st.last_seq == sync && self.sent_new.len() < 256 => Some(None),
            None => None,
        };
        if let Some(pos) = pos {
            let start = pos.map_or(0, |p| p + 1);
            let after = self.sent_new.iter().skip(start).filter(|(_, q)| *q).count();
            if after > usize::from(st.free) {
                self.violation(format!(
                    "{after} QUEUE_EVENTs after last_seq {} with free {}",
                    st.last_seq, st.free
                ));
            }
        }
    }

    fn tap_down(&mut self, bytes: &[u8]) {
        for &b in bytes {
            if let RxEvent::Frame(f) = self.down_tap.push(b)
                && f.ty == MSG_NAK
                && let Ok(Message::Nak(n)) = Message::decode(f.ty, &f.body)
                && n.reason.is_transport()
                && !matches!(n.reason, NakReason::Unknown(_))
            {
                self.seen.transport_naks += 1;
                // The absolute frame the NAK points at: the newest sent frame
                // with that seq (a NAK lags by at most the window), or the
                // next new one.
                let pos = self
                    .sent_new
                    .iter()
                    .rposition(|(s, _)| *s == n.expected)
                    .map(|p| p as u64)
                    .or_else(|| {
                        let next = self
                            .sync_seq?
                            .wrapping_add(1)
                            .wrapping_add(self.sent_new.len() as u8);
                        (next == n.expected).then_some(self.sent_new.len() as u64)
                    });
                if let Some(pos) = pos {
                    self.seen
                        .transport_nak_points
                        .insert((self.sync_epoch, pos));
                }
            }
        }
    }

    /// One step of the fake clock.
    pub fn step(&mut self) {
        let now = self.now;
        let down = self.down.deliver(now);
        self.tap_down(&down);
        self.session.handle_rx(&down, now);
        self.session.handle_timeout(now);
        loop {
            let phase = self.session.phase();
            let Some(chunk) = self.session.poll_transmit(now) else {
                break;
            };
            self.check_tx(&chunk, phase);
            self.seen.tx.push((now, chunk.clone()));
            self.up.send(now, chunk);
        }
        self.up.flush_hold(now);
        let up = self.up.deliver(now);
        self.ctrl.handle_rx(&up, now);
        self.ctrl.tick(now);
        while let Some(f) = self.ctrl.poll_transmit() {
            self.down.send(now, f);
        }
        self.down.flush_hold(now);
        while let Some(e) = self.session.poll_event() {
            self.seen.events.push((now, e));
        }
        if self.session.unacked() > usize::from(ACK_WINDOW) {
            self.violation(format!("{} frames unacknowledged", self.session.unacked()));
        }
        if self.ctrl.queue_len() > self.ctrl.queue_cap() {
            self.violation("controller queue over capacity".to_owned());
        }
        let mut next = now + self.step_us;
        for t in [
            self.session.poll_timeout(),
            self.up.next_due(),
            self.down.next_due(),
        ]
        .into_iter()
        .flatten()
        {
            if t > now && t < next {
                next = t;
            }
        }
        self.now = next;
    }

    /// Steps until `done` is true or `max_us` of fake time pass. Returns
    /// whether `done` became true.
    pub fn run_until(&mut self, max_us: u64, mut done: impl FnMut(&Sim) -> bool) -> bool {
        let end = self.now + max_us;
        while self.now < end {
            self.step();
            if done(self) {
                return true;
            }
        }
        false
    }

    /// Steps for `us` of fake time.
    pub fn run_for(&mut self, us: u64) {
        let end = self.now + us;
        while self.now < end {
            self.step();
        }
    }

    /// Opens the session and runs until it is connected and has a STATUS
    /// (or 2 s pass). Returns whether it got there.
    pub fn connect(&mut self) -> bool {
        self.session.open(self.now);
        self.run_until(2_000_000, |s| {
            s.session.phase() == Phase::Connected && s.session.credit().is_some()
        })
    }

    /// The events seen so far, without the times.
    #[must_use]
    pub fn events(&self) -> Vec<LinkEvent> {
        self.seen.events.iter().map(|(_, e)| e.clone()).collect()
    }

    /// The decoded frames the session sent: (time, type, seq).
    #[must_use]
    pub fn tx_frames(&self) -> Vec<(u64, u8, u8)> {
        self.seen
            .tx
            .iter()
            .filter_map(|(t, c)| {
                let (_, enc) = c.split_last()?;
                let f = decode_frame(RxSide::Controller, enc).ok()?;
                Some((*t, f.ty, f.seq))
            })
            .collect()
    }
}
