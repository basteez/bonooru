//! The reliable send path (AD-6): one u8 seq space for every app frame after
//! a sync point (HELLO or STOP), go-back-N with cumulative ACKs, a resend
//! after `ACK_TIMEOUT_MS`, a rewind only on a transport NAK whose `expected`
//! is ahead of the last rewind point, and credits from STATUS.
//!
//! Frames are numbered with an absolute counter (`abs`, u64) so wrap-around
//! never confuses the window: `abs` 0 is the sync point itself (its seq is
//! `sync_seq`), the first frame after it is `abs` 1 with seq `sync_seq + 1`.

use std::collections::VecDeque;

use crate::protocol::consts::{ACK_TIMEOUT_MS, ACK_WINDOW, MSG_QUEUE_EVENT};

/// The ACK timeout in µs.
pub const ACK_TIMEOUT_US: u64 = ACK_TIMEOUT_MS as u64 * 1000;
/// The go-back-N window (frames).
pub const WINDOW: u64 = ACK_WINDOW as u64;

/// How many acknowledged frames are remembered for credit accounting: a
/// STATUS can lag behind the ACKs by at most the window.
const HISTORY: usize = 256;

/// One app frame waiting for its ACK (or not yet sent).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pending {
    /// Message type.
    pub ty: u8,
    /// Body bytes.
    pub body: Vec<u8>,
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
    /// `abs` of `frames[0]` (the oldest unacknowledged frame).
    base: u64,
    /// Frames from `base` on: the first `high` were sent at least once.
    frames: VecDeque<Pending>,
    /// Index into `frames` of the next frame to hand out.
    next: usize,
    /// How many of `frames` were sent at least once.
    high: usize,
    /// When the frame at `base` was last sent (or `base` last moved).
    base_timer: u64,
    /// The last rewind point (`abs`), cleared by a timeout resend.
    rewind_at: Option<u64>,
    /// `is_queue_event` of acknowledged frames, newest last; `history[i]`
    /// is `abs = base - history.len() + i`.
    history: VecDeque<bool>,
    /// The last STATUS that mapped into the window: (`free`, `abs` of
    /// `last_seq`).
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
            frames: VecDeque::new(),
            next: 0,
            high: 0,
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

    /// The seq the next pushed frame will get.
    #[must_use]
    pub fn next_free_seq(&self) -> u8 {
        self.seq_of(self.base + self.frames.len() as u64)
    }

    /// The `abs` of the acknowledged-or-sent frame with this seq, searching
    /// from `base − 1` forward over the sent frames, then backward over the
    /// remembered history (the sync point is `abs` 0).
    fn abs_of(&self, seq: u8) -> Option<u64> {
        let before_base = self.base - 1;
        let fwd = u64::from(seq.wrapping_sub(self.seq_of(before_base)));
        if fwd <= self.high as u64 {
            return Some(before_base + fwd);
        }
        let back = u64::from(self.seq_of(before_base).wrapping_sub(seq));
        let known_back = (self.history.len() as u64).min(before_base);
        (back <= known_back).then(|| before_base - back)
    }

    /// Appends a frame; it goes out when the window allows.
    pub fn push(&mut self, ty: u8, body: Vec<u8>) -> u8 {
        let seq = self.next_free_seq();
        self.frames.push_back(Pending { ty, body });
        seq
    }

    /// Frames sent and not acknowledged.
    #[must_use]
    pub fn unacked(&self) -> usize {
        self.high
    }

    /// Frames pushed and not acknowledged (sent or not).
    #[must_use]
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// A pushed frame was never sent yet (or a resend is pending).
    #[must_use]
    pub fn has_unsent(&self) -> bool {
        self.next < self.frames.len()
    }

    /// Nothing pushed and unacknowledged.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// The counters.
    #[must_use]
    pub fn stats(&self) -> SendStats {
        self.stats
    }

    /// The pending frame with this seq among the sent ones, if any.
    #[must_use]
    pub fn sent_frame(&self, seq: u8) -> Option<&Pending> {
        let abs = self.abs_of(seq)?;
        let i = usize::try_from(abs.checked_sub(self.base)?).ok()?;
        (i < self.high).then(|| self.frames.get(i)).flatten()
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
            .min(self.high);
        for _ in 0..n {
            if let Some(f) = self.frames.pop_front() {
                self.history.push_back(f.is_queue_event());
                if self.history.len() > HISTORY {
                    self.history.pop_front();
                }
            }
        }
        if n > 0 {
            self.base += n as u64;
            self.high -= n;
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
        // After the ack above, `expected` can only be `base` if it is a
        // sent frame (or one past the sent ones: nothing to rewind).
        if abs != self.base || self.high == 0 {
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
    /// the oldest unacknowledged frame and clear the rewind point.
    pub fn check_timeout(&mut self, now_us: u64) {
        if self.high > 0 && now_us.saturating_sub(self.base_timer) >= ACK_TIMEOUT_US {
            self.next = 0;
            self.rewind_at = None;
            self.base_timer = now_us;
            self.stats.resends += 1;
        }
    }

    /// When `check_timeout` will next act, if anything is unacknowledged.
    #[must_use]
    pub fn timeout_at(&self) -> Option<u64> {
        (self.high > 0).then_some(self.base_timer + ACK_TIMEOUT_US)
    }

    /// A STATUS: `last_seq` acknowledges like an ACK, and (`free`,
    /// `last_seq`) becomes the credit base if `last_seq` maps into the known
    /// frames.
    pub fn status(&mut self, free: u16, last_seq: u8, now_us: u64) {
        self.ack(last_seq, now_us);
        if let Some(abs) = self.abs_of(last_seq) {
            self.credit_base = Some((free, abs));
        }
    }

    /// QUEUE_EVENTs sent (at least once) after the credit base's `last_seq`.
    fn queue_events_after(&self, abs: u64) -> u64 {
        let hist_start = self.base - self.history.len() as u64;
        let mut n = 0u64;
        for (i, &qe) in self.history.iter().enumerate() {
            if qe && hist_start + i as u64 > abs {
                n += 1;
            }
        }
        for (i, f) in self.frames.iter().take(self.high).enumerate() {
            if f.is_queue_event() && self.base + i as u64 > abs {
                n += 1;
            }
        }
        n
    }

    /// `STATUS.free − QUEUE_EVENTs sent after STATUS.last_seq` (AD-6);
    /// `None` before the first STATUS that maps into this sync period.
    #[must_use]
    pub fn credit(&self) -> Option<i64> {
        let (free, abs) = self.credit_base?;
        // A base older than the remembered history cannot be counted: no
        // credit until a fresher STATUS.
        let hist_start = self.base - self.history.len() as u64;
        if abs + 1 < hist_start {
            return None;
        }
        Some(i64::from(free) - self.queue_events_after(abs) as i64)
    }

    /// The next frame to hand out, if the window and the credits allow:
    /// `(seq, type, body)`. A resend is always allowed; a new frame needs
    /// `unacked < 64` and, for a QUEUE_EVENT, a positive credit.
    pub fn next_to_send(&mut self, now_us: u64) -> Option<(u8, u8, Vec<u8>)> {
        let f = self.frames.get(self.next)?;
        if self.next >= self.high {
            if self.high as u64 >= WINDOW {
                return None;
            }
            if f.is_queue_event() && !self.credit().is_some_and(|c| c > 0) {
                return None;
            }
        }
        let out = (
            self.seq_of(self.base + self.next as u64),
            f.ty,
            f.body.clone(),
        );
        if self.next == 0 {
            self.base_timer = now_us;
        }
        self.next += 1;
        self.high = self.high.max(self.next);
        self.stats.sent += 1;
        Some(out)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::protocol::consts::{MSG_PING, MSG_START};

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

    #[test]
    fn no_queue_event_before_a_status_and_credit_limits() {
        let mut b = SendBuffer::new(250);
        qe(&mut b, 10);
        assert!(drain(&mut b, 0).is_empty());
        b.status(3, 250, 0);
        assert_eq!(drain(&mut b, 0), vec![251, 252, 253]);
        assert_eq!(b.credit(), Some(0));
        // A STATUS that saw two of them and has 4 free: 4 − 1 in flight = 3.
        b.status(4, 252, 1);
        assert_eq!(b.credit(), Some(3));
        assert_eq!(drain(&mut b, 1), vec![254, 255, 0]);
    }

    #[test]
    fn non_queue_frames_need_no_credit_but_count_in_the_window() {
        let mut b = SendBuffer::new(0);
        b.push(MSG_START, vec![0; 4]);
        b.push(MSG_PING, vec![]);
        assert_eq!(drain(&mut b, 0), vec![1, 2]);
        let mut b = SendBuffer::new(0);
        for _ in 0..70 {
            b.push(MSG_PING, vec![]);
        }
        assert_eq!(drain(&mut b, 0).len(), 64);
        b.ack(10, 1);
        assert_eq!(drain(&mut b, 1).len(), 6);
    }

    #[test]
    fn rewind_only_ahead_of_the_last_rewind_point() {
        let mut b = SendBuffer::new(0);
        b.status(100, 0, 0);
        qe(&mut b, 10);
        assert_eq!(drain(&mut b, 0).len(), 10);
        assert!(b.transport_nak(4, 1)); // frames 1..3 accepted, rewind to 4
        assert_eq!(b.unacked(), 7);
        assert!(!b.transport_nak(4, 2)); // same point: ignored
        assert_eq!(drain(&mut b, 2), vec![4, 5, 6, 7, 8, 9, 10]);
        assert!(!b.transport_nak(4, 3)); // duplicate of an old NAK
        assert!(b.transport_nak(6, 4)); // ahead: rewinds
        assert_eq!(b.stats().rewinds, 2);
    }

    #[test]
    fn timeout_resends_from_the_oldest_and_clears_the_rewind_point() {
        let mut b = SendBuffer::new(0);
        b.status(100, 0, 0);
        qe(&mut b, 3);
        assert_eq!(drain(&mut b, 1000), vec![1, 2, 3]);
        b.check_timeout(1000 + ACK_TIMEOUT_US - 1);
        assert!(drain(&mut b, 1000 + ACK_TIMEOUT_US - 1).is_empty());
        assert_eq!(b.timeout_at(), Some(1000 + ACK_TIMEOUT_US));
        b.check_timeout(1000 + ACK_TIMEOUT_US);
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
        assert_eq!(b.ack(0, 1), 0); // the sync point itself
        assert_eq!(b.ack(9, 1), 0); // beyond what was sent
        assert_eq!(b.ack(2, 1), 2);
        assert_eq!(b.ack(1, 1), 0); // older than base: stale
        assert_eq!(b.unacked(), 3);
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
        // ACKs arrive before the next STATUS: the old base still counts them.
        b.ack(5, 1);
        assert_eq!(b.credit(), Some(0));
        b.status(2, 5, 2); // 3 still queued on the controller
        assert_eq!(b.credit(), Some(2));
    }
}
