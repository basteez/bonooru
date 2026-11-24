//! The reliable send path (AD-6): one u8 seq space for every app frame after
//! a sync point (HELLO or STOP), go-back-N with cumulative ACKs, a resend
//! after `ACK_TIMEOUT_MS`, a rewind only on a transport NAK whose `expected`
//! is ahead of the last rewind point, and credits from STATUS.
//!
//! A frame gets its seq when it is **first sent**, not when it is pushed
//! (review fix of Story 3.3): QUEUE_EVENTs wait for credit in their own
//! queue, and control frames (START, SET_STATE, PING) never wait behind them.
//! So the seqs in use are always exactly the sent, unacknowledged frames, and
//! the next free seq (the one a STOP takes) is never one of them.
//!
//! Frames are numbered with an absolute counter (`abs`, u64): `abs` 0 is the
//! sync point itself (its seq is `sync_seq`), the first frame sent after it
//! is `abs` 1 with seq `sync_seq + 1`.

use std::collections::VecDeque;

use crate::protocol::consts::{ACK_TIMEOUT_MS, ACK_WINDOW, MSG_QUEUE_EVENT, MSG_START};

/// The ACK timeout in µs.
pub const ACK_TIMEOUT_US: u64 = ACK_TIMEOUT_MS as u64 * 1000;
/// The go-back-N window (frames).
pub const WINDOW: usize = ACK_WINDOW as usize;

/// How many acknowledged frames are remembered for credit accounting and
/// stale-seq matching. A STATUS lags the ACKs by at most the window; keeping
/// no more than that keeps forward (≤ 64) plus backward (≤ 64) matches well
/// inside the 256 seq values, so a seq never matches twice.
const HISTORY: usize = WINDOW;

/// One app frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pending {
    /// Message type.
    pub ty: u8,
    /// Body bytes.
    pub body: Vec<u8>,
    /// START only: the number of QUEUE_EVENTs pushed before it.
    after_qe: u64,
}

impl Pending {
    fn is_queue_event(&self) -> bool {
        self.ty == MSG_QUEUE_EVENT
    }
}

/// Counters for tests and the debug view.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SendStats {
    /// Frames handed out, resends included.
    pub sent: u64,
    /// Go-backs after the ACK timeout.
    pub resends: u64,
    /// Rewinds on a transport NAK.
    pub rewinds: u64,
}

/// The go-back-N send buffer.
#[derive(Debug, Clone)]
pub struct SendBuffer {
    sync_seq: u8,
    /// `abs` of `sent[0]` (the oldest unacknowledged frame).
    base: u64,
    /// Sent and not acknowledged, oldest first; `sent[i]` is `abs base + i`.
    sent: VecDeque<Pending>,
    /// Index into `sent` of the next frame to resend (`sent.len()`: none).
    next: usize,
    /// Control frames not sent yet (START, SET_STATE, PING), in order.
    ctl: VecDeque<Pending>,
    /// QUEUE_EVENTs not sent yet, in order.
    qe: VecDeque<Pending>,
    qe_pushed: u64,
    qe_first_sent: u64,
    /// When the frame at `base` was last sent (or `base` last moved).
    base_timer: u64,
    /// The last rewind point (`abs`), cleared by a timeout resend.
    rewind_at: Option<u64>,
    /// `is_queue_event` of acknowledged frames, newest last; `history[i]`
    /// is `abs = base - history.len() + i`.
    history: VecDeque<bool>,
    /// The last STATUS that mapped: (`free`, `abs` of `last_seq`).
    credit_base: Option<(u16, u64)>,
    stats: SendStats,
}

impl SendBuffer {
    /// An empty buffer after a sync point with seq `sync_seq`.
    #[must_use]
    pub fn new(sync_seq: u8) -> Self {
        Self {
            sync_seq,
            base: 1,
            sent: VecDeque::new(),
            next: 0,
            ctl: VecDeque::new(),
            qe: VecDeque::new(),
            qe_pushed: 0,
            qe_first_sent: 0,
            base_timer: 0,
            rewind_at: None,
            history: VecDeque::new(),
            credit_base: None,
            stats: SendStats::default(),
        }
    }

    fn seq_of(&self, abs: u64) -> u8 {
        // Truncation is the point: the seq wraps.
        self.sync_seq.wrapping_add((abs % 256) as u8)
    }

    /// The seq the next new frame will get (one past the sent frames): the
    /// seq a STOP takes, never one in flight.
    #[must_use]
    pub fn next_free_seq(&self) -> u8 {
        self.seq_of(self.base + self.sent.len() as u64)
    }

    /// The `abs` of the frame with this seq among the sent ones (forward
    /// from `base − 1`) or the remembered acknowledged ones (backward; the
    /// sync point is `abs` 0).
    fn abs_of(&self, seq: u8) -> Option<u64> {
        let before_base = self.base - 1;
        let fwd = u64::from(seq.wrapping_sub(self.seq_of(before_base)));
        if fwd <= self.sent.len() as u64 {
            return Some(before_base + fwd);
        }
        let back = u64::from(self.seq_of(before_base).wrapping_sub(seq));
        let known_back = (self.history.len() as u64).min(before_base);
        (back <= known_back).then(|| before_base - back)
    }

    /// The `abs` a seq maps to (sent, acknowledged and remembered, or the
    /// sync point), if any.
    #[must_use]
    pub fn position_of(&self, seq: u8) -> Option<u64> {
        self.abs_of(seq)
    }

    /// The `abs` the next new frame will get.
    #[must_use]
    pub fn next_abs(&self) -> u64 {
        self.base + self.sent.len() as u64
    }

    /// Queues a frame. QUEUE_EVENTs wait for credit in their own queue;
    /// other frames go out first (a START only once the QUEUE_EVENTs pushed
    /// before it are sent, or no credit is left for the next one).
    pub fn push(&mut self, ty: u8, body: Vec<u8>) {
        if ty == MSG_QUEUE_EVENT {
            self.qe_pushed += 1;
            self.qe.push_back(Pending {
                ty,
                body,
                after_qe: 0,
            });
        } else {
            let after_qe = if ty == MSG_START { self.qe_pushed } else { 0 };
            self.ctl.push_back(Pending { ty, body, after_qe });
        }
    }

    /// Frames sent and not acknowledged.
    #[must_use]
    pub fn unacked(&self) -> usize {
        self.sent.len()
    }

    /// Frames pushed and not acknowledged (sent or not).
    #[must_use]
    pub fn len(&self) -> usize {
        self.sent.len() + self.ctl.len() + self.qe.len()
    }

    /// QUEUE_EVENTs pushed and not sent yet.
    #[must_use]
    pub fn qe_len(&self) -> usize {
        self.qe.len()
    }

    /// Nothing pushed and unacknowledged.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// A control frame is waiting to be sent.
    #[must_use]
    pub fn has_unsent_control(&self) -> bool {
        !self.ctl.is_empty()
    }

    /// The window has room for a new frame.
    #[must_use]
    pub fn window_open(&self) -> bool {
        self.sent.len() < WINDOW
    }

    /// The counters.
    #[must_use]
    pub fn stats(&self) -> SendStats {
        self.stats
    }

    /// The sent frame with this seq, if any.
    #[must_use]
    pub fn sent_frame(&self, seq: u8) -> Option<&Pending> {
        let abs = self.abs_of(seq)?;
        let i = usize::try_from(abs.checked_sub(self.base)?).ok()?;
        self.sent.get(i)
    }

    /// Cumulative acknowledgement of every frame up to `seq`. Returns how
    /// many frames it retired. A seq outside `[base−1, base−1+sent]` is
    /// stale (or from before the sync point) and retires nothing.
    pub fn ack(&mut self, seq: u8, now_us: u64) -> usize {
        let Some(abs) = self.abs_of(seq) else {
            return 0;
        };
        if abs < self.base {
            return 0;
        }
        let n = usize::try_from(abs - self.base + 1)
            .unwrap_or(0)
            .min(self.sent.len());
        for _ in 0..n {
            if let Some(f) = self.sent.pop_front() {
                self.history.push_back(f.is_queue_event());
                if self.history.len() > HISTORY {
                    self.history.pop_front();
                }
            }
        }
        if n > 0 {
            self.base += n as u64;
            self.next = self.next.saturating_sub(n);
            self.base_timer = now_us;
        }
        n
    }

    /// A transport NAK: everything before `expected` is accepted; rewind to
    /// `expected` only if it is ahead of the last rewind point (AD-6).
    /// Returns true if it rewound.
    pub fn transport_nak(&mut self, expected: u8, now_us: u64) -> bool {
        self.ack(expected.wrapping_sub(1), now_us);
        let Some(abs) = self.abs_of(expected) else {
            return false;
        };
        if abs != self.base || self.sent.is_empty() {
            return false;
        }
        if self.rewind_at.is_some_and(|r| abs <= r) {
            return false;
        }
        self.rewind_at = Some(abs);
        self.next = 0;
        self.base_timer = now_us;
        self.stats.rewinds += 1;
        true
    }

    /// The resend timer: after `ACK_TIMEOUT_US` without progress, go back to
    /// the oldest unacknowledged frame and clear the rewind point. Returns
    /// true if it went back.
    pub fn check_timeout(&mut self, now_us: u64) -> bool {
        if !self.sent.is_empty() && now_us >= self.base_timer.saturating_add(ACK_TIMEOUT_US) {
            self.next = 0;
            self.rewind_at = None;
            self.base_timer = now_us;
            self.stats.resends += 1;
            return true;
        }
        false
    }

    /// When `check_timeout` will next act, if anything is unacknowledged.
    #[must_use]
    pub fn timeout_at(&self) -> Option<u64> {
        (!self.sent.is_empty()).then(|| self.base_timer.saturating_add(ACK_TIMEOUT_US))
    }

    /// A STATUS: `last_seq` acknowledges like an ACK, and (`free`,
    /// `last_seq`) becomes the credit base if `last_seq` maps. Returns the
    /// `abs` it mapped to.
    pub fn status(&mut self, free: u16, last_seq: u8, now_us: u64) -> Option<u64> {
        self.ack(last_seq, now_us);
        let abs = self.abs_of(last_seq)?;
        self.credit_base = Some((free, abs));
        Some(abs)
    }

    /// QUEUE_EVENTs sent (at least once) after `abs`.
    fn queue_events_after(&self, abs: u64) -> u64 {
        let hist_start = self.base - self.history.len() as u64;
        let old = self
            .history
            .iter()
            .enumerate()
            .filter(|&(i, &qe)| qe && hist_start + i as u64 > abs)
            .count();
        let new = self
            .sent
            .iter()
            .enumerate()
            .filter(|&(i, f)| f.is_queue_event() && self.base + i as u64 > abs)
            .count();
        (old + new) as u64
    }

    /// `STATUS.free − QUEUE_EVENTs sent after STATUS.last_seq` (AD-6);
    /// `None` before the first STATUS that maps into this sync period, or
    /// when the base is older than the remembered history.
    #[must_use]
    pub fn credit(&self) -> Option<i64> {
        let (free, abs) = self.credit_base?;
        let hist_start = self.base - self.history.len() as u64;
        if abs + 1 < hist_start {
            return None;
        }
        Some(i64::from(free) - self.queue_events_after(abs) as i64)
    }

    fn start_ready(&self, f: &Pending) -> bool {
        if f.ty != MSG_START {
            return true;
        }
        // Every QUEUE_EVENT pushed before START is out, or the prefill is
        // over: at least one is out and no credit is left for the next.
        self.qe_first_sent >= f.after_qe
            || (self.qe_first_sent > 0 && !self.credit().is_some_and(|c| c > 0))
    }

    /// The next frame to hand out, if the window and the credits allow:
    /// `(seq, type, body)`. A resend is always allowed; a new frame needs
    /// `unacked < 64`; a new QUEUE_EVENT also needs a positive credit.
    pub fn next_to_send(&mut self, now_us: u64) -> Option<(u8, u8, Vec<u8>)> {
        if let Some(f) = self.sent.get(self.next) {
            let out = (
                self.seq_of(self.base + self.next as u64),
                f.ty,
                f.body.clone(),
            );
            if self.next == 0 {
                self.base_timer = now_us;
            }
            self.next += 1;
            self.stats.sent += 1;
            return Some(out);
        }
        if !self.window_open() {
            return None;
        }
        let ctl_ready = self.ctl.front().is_some_and(|f| self.start_ready(f));
        let f = if ctl_ready {
            self.ctl.pop_front()?
        } else if self.credit().is_some_and(|c| c > 0) {
            let f = self.qe.pop_front()?;
            self.qe_first_sent += 1;
            f
        } else {
            return None;
        };
        let seq = self.next_free_seq();
        let out = (seq, f.ty, f.body.clone());
        if self.sent.is_empty() {
            self.base_timer = now_us;
        }
        self.sent.push_back(f);
        self.next = self.sent.len();
        self.stats.sent += 1;
        Some(out)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::protocol::consts::{MSG_PING, MSG_SET_STATE};

    fn qe(b: &mut SendBuffer, n: usize) {
        for _ in 0..n {
            b.push(MSG_QUEUE_EVENT, vec![0; 12]);
        }
    }

    fn drain(b: &mut SendBuffer, now: u64) -> Vec<u8> {
        let mut seqs = Vec::new();
        while let Some((s, _, _)) = b.next_to_send(now) {
            seqs.push(s);
        }
        seqs
    }

    fn drain_types(b: &mut SendBuffer, now: u64) -> Vec<u8> {
        let mut t = Vec::new();
        while let Some((_, ty, _)) = b.next_to_send(now) {
            t.push(ty);
        }
        t
    }

    #[test]
    fn no_queue_event_before_a_status_and_credit_limits() {
        let mut b = SendBuffer::new(250);
        qe(&mut b, 10);
        assert!(drain(&mut b, 0).is_empty());
        b.status(3, 250, 0);
        assert_eq!(drain(&mut b, 0), vec![251, 252, 253]);
        assert_eq!(b.credit(), Some(0));
        b.status(4, 252, 1);
        assert_eq!(b.credit(), Some(3));
        assert_eq!(drain(&mut b, 1), vec![254, 255, 0]);
    }

    #[test]
    fn control_frames_overtake_queue_events_waiting_for_credit() {
        let mut b = SendBuffer::new(0);
        b.status(2, 0, 0);
        qe(&mut b, 5);
        b.push(MSG_SET_STATE, vec![0; 8]);
        b.push(MSG_PING, vec![]);
        assert_eq!(
            drain_types(&mut b, 0),
            vec![MSG_SET_STATE, MSG_PING, MSG_QUEUE_EVENT, MSG_QUEUE_EVENT]
        );
        assert_eq!(
            b.next_free_seq(),
            5,
            "the next free seq follows the sent frames"
        );
    }

    #[test]
    fn start_waits_for_the_prefill_then_goes_without_more_credit() {
        let mut b = SendBuffer::new(0);
        b.status(3, 0, 0);
        qe(&mut b, 10);
        b.push(MSG_START, vec![0; 4]);
        qe(&mut b, 2);
        assert_eq!(
            drain_types(&mut b, 0),
            vec![MSG_QUEUE_EVENT, MSG_QUEUE_EVENT, MSG_QUEUE_EVENT, MSG_START]
        );
        let mut b = SendBuffer::new(0);
        b.status(100, 0, 0);
        qe(&mut b, 4);
        b.push(MSG_START, vec![0; 4]);
        qe(&mut b, 2);
        assert_eq!(
            drain_types(&mut b, 0),
            vec![
                MSG_QUEUE_EVENT,
                MSG_QUEUE_EVENT,
                MSG_QUEUE_EVENT,
                MSG_QUEUE_EVENT,
                MSG_START,
                MSG_QUEUE_EVENT,
                MSG_QUEUE_EVENT
            ]
        );
    }

    #[test]
    fn window_of_64_for_every_frame() {
        let mut b = SendBuffer::new(0);
        for _ in 0..70 {
            b.push(MSG_PING, vec![]);
        }
        assert_eq!(drain(&mut b, 0).len(), 64);
        assert!(!b.window_open());
        b.ack(10, 1);
        assert_eq!(drain(&mut b, 1).len(), 6);
    }

    #[test]
    fn rewind_only_ahead_of_the_last_rewind_point() {
        let mut b = SendBuffer::new(0);
        b.status(100, 0, 0);
        qe(&mut b, 10);
        assert_eq!(drain(&mut b, 0).len(), 10);
        assert!(b.transport_nak(4, 1));
        assert_eq!(b.unacked(), 7);
        assert!(!b.transport_nak(4, 2));
        assert_eq!(drain(&mut b, 2), vec![4, 5, 6, 7, 8, 9, 10]);
        assert!(!b.transport_nak(4, 3));
        assert!(b.transport_nak(6, 4));
        assert_eq!(b.stats().rewinds, 2);
    }

    #[test]
    fn timeout_resends_from_the_oldest_and_clears_the_rewind_point() {
        let mut b = SendBuffer::new(0);
        b.status(100, 0, 0);
        qe(&mut b, 3);
        assert_eq!(drain(&mut b, 1000), vec![1, 2, 3]);
        assert!(!b.check_timeout(1000 + ACK_TIMEOUT_US - 1));
        assert!(drain(&mut b, 1000 + ACK_TIMEOUT_US - 1).is_empty());
        assert_eq!(b.timeout_at(), Some(1000 + ACK_TIMEOUT_US));
        assert!(b.check_timeout(1000 + ACK_TIMEOUT_US));
        assert_eq!(drain(&mut b, 1000 + ACK_TIMEOUT_US), vec![1, 2, 3]);
        assert_eq!(b.stats().resends, 1);
    }

    #[test]
    fn stale_and_foreign_acks_retire_nothing() {
        let mut b = SendBuffer::new(0);
        b.status(100, 0, 0);
        qe(&mut b, 5);
        drain(&mut b, 0);
        assert_eq!(b.ack(200, 1), 0);
        assert_eq!(b.ack(0, 1), 0);
        assert_eq!(b.ack(9, 1), 0);
        assert_eq!(b.ack(2, 1), 2);
        assert_eq!(b.ack(1, 1), 0);
        assert_eq!(b.unacked(), 3);
    }

    #[test]
    fn a_seq_maps_at_most_once() {
        // 64 acknowledged (history) + 64 in flight: at most 129 seqs map.
        let mut b = SendBuffer::new(0);
        b.status(1000, 0, 0);
        qe(&mut b, 300);
        for _ in 0..4 {
            let s = drain(&mut b, 0);
            if let Some(&last) = s.last() {
                b.ack(last, 0);
            }
        }
        drain(&mut b, 0);
        let hits = (0..=255u8).filter(|&s| b.position_of(s).is_some()).count();
        assert!(hits <= 64 + 64 + 1, "{hits}");
    }

    #[test]
    fn wraps_around_the_seq_space() {
        let mut b = SendBuffer::new(0);
        b.status(1000, 0, 0);
        let mut acked = 0;
        for round in 0..20u64 {
            qe(&mut b, 50);
            let seqs = drain(&mut b, round);
            assert_eq!(seqs.len(), 50);
            acked += b.ack(*seqs.last().unwrap(), round);
            b.status(1000, *seqs.last().unwrap(), round);
        }
        assert_eq!(acked, 1000);
        assert!(b.is_empty());
    }

    #[test]
    fn credit_counts_frames_acked_after_the_status() {
        let mut b = SendBuffer::new(0);
        b.status(5, 0, 0);
        qe(&mut b, 5);
        assert_eq!(drain(&mut b, 0).len(), 5);
        b.ack(5, 1);
        assert_eq!(b.credit(), Some(0));
        b.status(2, 5, 2);
        assert_eq!(b.credit(), Some(2));
    }
}
