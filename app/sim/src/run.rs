//! The simulation driver: one `Session`, one `FakeController`, a channel each
//! way and a fake clock. It drives the session the way a real adapter must
//! (`handle_timeout` only when `poll_timeout` is due) and checks invariants
//! from the bytes on the wire only, never from the session's own counters
//! (C.7, Story 3.3 review fix).

use std::collections::BTreeSet;

use ocb_core::link::{LinkEvent, Session, SessionConfig};
use ocb_core::protocol::consts::{
    ACK_WINDOW, MSG_ACK, MSG_HELLO, MSG_INFO, MSG_NAK, MSG_QUEUE_EVENT, MSG_STATUS, MSG_STOP,
};
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
    // Wire-side bookkeeping, independent of the session.
    sync_seq: Option<u8>,
    sent_new: Vec<(u8, bool)>,
    /// Index into `sent_new` of the newest frame the controller confirmed.
    confirmed: Option<usize>,
    /// The last STATUS delivered: (`free`, `last_seq`).
    last_status: Option<(u16, u8)>,
    sync_epoch: u64,
    awaiting_info: bool,
    awaiting_stop_ack: Option<u8>,
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
            confirmed: None,
            last_status: None,
            sync_epoch: 0,
            awaiting_info: false,
            awaiting_stop_ack: None,
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

    /// The index in `sent_new` of the newest frame with this seq (a reply
    /// lags by at most the window), `None` for the sync point itself.
    fn index_of(&self, seq: u8) -> Option<Option<usize>> {
        let lo = self
            .sent_new
            .len()
            .saturating_sub(2 * usize::from(ACK_WINDOW));
        if let Some(p) = self.sent_new.iter().skip(lo).rposition(|(s, _)| *s == seq) {
            return Some(Some(lo + p));
        }
        (Some(seq) == self.sync_seq && self.sent_new.len() < 256).then_some(None)
    }

    fn confirm(&mut self, seq: u8) {
        if let Some(Some(i)) = self.index_of(seq)
            && self.confirmed.is_none_or(|c| i > c)
        {
            self.confirmed = Some(i);
        }
    }

    /// Checks one chunk the session sent, from the wire only.
    fn check_tx(&mut self, chunk: &[u8]) {
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
        if f.ty == MSG_HELLO {
            self.awaiting_info = true;
            self.awaiting_stop_ack = None;
        } else if self.awaiting_info {
            self.violation(format!("type {:#04x} sent before INFO", f.ty));
        } else if f.ty == MSG_STOP {
            self.awaiting_stop_ack = Some(f.seq);
        } else if self.awaiting_stop_ack.is_some() {
            self.violation(format!("type {:#04x} sent between STOP and its ACK", f.ty));
        }
        if f.ty == MSG_HELLO || f.ty == MSG_STOP {
            if self.sync_seq != Some(f.seq) || !self.sent_new.is_empty() {
                self.sync_epoch += 1;
            }
            self.sync_seq = Some(f.seq);
            self.sent_new.clear();
            self.confirmed = None;
            self.last_status = None;
            return;
        }
        let Some(sync) = self.sync_seq else {
            self.violation("app frame before any HELLO".to_owned());
            return;
        };
        let next_new = sync.wrapping_add(1).wrapping_add(self.sent_new.len() as u8);
        if f.seq != next_new {
            // A resend must be one of the unconfirmed frames.
            let first_open = self.confirmed.map_or(0, |c| c + 1);
            let ok = self
                .sent_new
                .iter()
                .skip(first_open)
                .any(|(s, _)| *s == f.seq);
            if !ok {
                self.violation(format!("seq {} is neither new nor unconfirmed", f.seq));
            }
            return;
        }
        let qe = f.ty == MSG_QUEUE_EVENT;
        self.sent_new.push((f.seq, qe));
        // The window (AD-6), from the wire: new frames not yet confirmed.
        let open = self.sent_new.len() - self.confirmed.map_or(0, |c| c + 1);
        if open > usize::from(ACK_WINDOW) {
            self.violation(format!("{open} frames in flight"));
        }
        if !qe {
            return;
        }
        // Credits (AD-6): QUEUE_EVENTs after the last delivered STATUS's
        // `last_seq` must not exceed its `free`.
        let Some((free, last_seq)) = self.last_status else {
            self.violation("QUEUE_EVENT before any STATUS of this sync period".to_owned());
            return;
        };
        if let Some(pos) = self.index_of(last_seq) {
            let start = pos.map_or(0, |p| p + 1);
            let after = self.sent_new.iter().skip(start).filter(|(_, q)| *q).count();
            if after > usize::from(free) {
                self.violation(format!(
                    "{after} QUEUE_EVENTs after last_seq {last_seq} with free {free}"
                ));
            }
        }
    }

    fn tap_down(&mut self, bytes: &[u8]) {
        for &b in bytes {
            let RxEvent::Frame(f) = self.down_tap.push(b) else {
                continue;
            };
            let Ok(msg) = Message::decode(f.ty, &f.body) else {
                continue;
            };
            match (f.ty, msg) {
                (MSG_INFO, _) => self.awaiting_info = false,
                (MSG_ACK, Message::Ack(a)) => {
                    if self.awaiting_stop_ack == Some(a.seq) {
                        self.awaiting_stop_ack = None;
                    }
                    self.confirm(a.seq);
                }
                (MSG_STATUS, Message::Status(s)) => {
                    // Only a STATUS that maps into this sync period counts.
                    if self.awaiting_stop_ack.is_none() && self.index_of(s.last_seq).is_some() {
                        self.last_status = Some((s.free, s.last_seq));
                        self.confirm(s.last_seq);
                    }
                }
                (MSG_NAK, Message::Nak(n)) => {
                    self.confirm(n.expected.wrapping_sub(1));
                    if n.reason.is_transport() && !matches!(n.reason, NakReason::Unknown(_)) {
                        self.seen.transport_naks += 1;
                        let pos = match self.index_of(n.expected) {
                            Some(Some(p)) => Some(p as u64),
                            _ => {
                                let next = self.sync_seq.map(|s| {
                                    s.wrapping_add(1).wrapping_add(self.sent_new.len() as u8)
                                });
                                (next == Some(n.expected)).then_some(self.sent_new.len() as u64)
                            }
                        };
                        if let Some(pos) = pos {
                            self.seen
                                .transport_nak_points
                                .insert((self.sync_epoch, pos));
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// One step of the fake clock.
    pub fn step(&mut self) {
        let now = self.now;
        let down = self.down.deliver(now);
        self.tap_down(&down);
        self.session.handle_rx(&down, now);
        // Strict timers: `handle_timeout` only when `poll_timeout` says so.
        if self.session.poll_timeout().is_some_and(|t| t <= now) {
            self.session.handle_timeout(now);
        }
        while let Some(chunk) = self.session.poll_transmit(now) {
            self.check_tx(&chunk);
            self.seen.tx.push((now, chunk.clone()));
            self.up.send(now, chunk);
        }
        if let Some(t) = self.session.poll_timeout()
            && t <= now
        {
            self.violation(format!(
                "poll_timeout {t} is still due after a full step (busy loop)"
            ));
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

    /// Opens the session and runs until it is connected and has a credit
    /// (a STATUS), or 2 s pass. Returns whether it got there.
    pub fn connect(&mut self) -> bool {
        self.session.open(self.now);
        self.run_until(2_000_000, |s| {
            s.session.phase() == ocb_core::link::Phase::Connected && s.session.credit().is_some()
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
