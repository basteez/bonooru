//! Streaming receive splitter and counters (AD-3, AD-4), the same semantics as
//! `firmware/common/proto/rx.c` (Story 2.2 D10, D12). Fed one byte at a time,
//! no I/O; any number of instances can coexist.

use super::consts::{FRAME_DELIMITER, MAX_ENCODED_FRAME};
use super::frame::{Frame, FrameError, RxSide, decode_frame};

/// Receive counters. Every delimiter that ends a run increments exactly one
/// of `ok`, `empty`, `cobs`, `crc`, `range`; an overlong run is counted in
/// `overlong` instead (once, when its first excess byte arrives). `empty`
/// also counts the session-opening and idle `0x00` bytes: informational,
/// never an error.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RxStats {
    /// Frames delivered.
    pub ok: u32,
    /// Empty runs.
    pub empty: u32,
    /// Malformed COBS runs.
    pub cobs: u32,
    /// Length or CRC errors.
    pub crc: u32,
    /// Types outside the receive range.
    pub range: u32,
    /// Runs longer than `MAX_ENCODED_FRAME`.
    pub overlong: u32,
}

/// What one byte produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RxEvent {
    /// Byte consumed, nothing ready.
    None,
    /// A valid frame.
    Frame(Frame),
    /// A delimiter ended a run that was dropped, for this reason (already
    /// counted; an overlong run was counted when it became overlong).
    Dropped(FrameError),
}

/// The splitter for one receiving side.
#[derive(Debug, Clone)]
pub struct Splitter {
    side: RxSide,
    buf: Vec<u8>,
    discarding: bool,
    stats: RxStats,
}

impl Splitter {
    /// A new splitter for `side` (the app uses [`RxSide::App`]).
    #[must_use]
    pub fn new(side: RxSide) -> Self {
        Self {
            side,
            buf: Vec::with_capacity(MAX_ENCODED_FRAME),
            discarding: false,
            stats: RxStats::default(),
        }
    }

    /// The counters so far.
    #[must_use]
    pub fn stats(&self) -> RxStats {
        self.stats
    }

    fn count(&mut self, err: FrameError) {
        let s = &mut self.stats;
        let c = match err {
            FrameError::Empty => &mut s.empty,
            FrameError::Cobs => &mut s.cobs,
            FrameError::Range => &mut s.range,
            FrameError::Overlong => &mut s.overlong,
            FrameError::Crc | FrameError::BodyTooLong => &mut s.crc,
        };
        *c = c.wrapping_add(1);
    }

    /// Feeds one byte.
    pub fn push(&mut self, byte: u8) -> RxEvent {
        if byte != FRAME_DELIMITER {
            if self.discarding {
                return RxEvent::None;
            }
            if self.buf.len() >= MAX_ENCODED_FRAME {
                // First excess byte: count the overlong run once, drop the rest.
                self.count(FrameError::Overlong);
                self.discarding = true;
                self.buf.clear();
                return RxEvent::None;
            }
            self.buf.push(byte);
            return RxEvent::None;
        }
        if self.discarding {
            self.discarding = false;
            self.buf.clear();
            return RxEvent::Dropped(FrameError::Overlong);
        }
        let result = decode_frame(self.side, &self.buf);
        self.buf.clear();
        match result {
            Ok(frame) => {
                self.stats.ok = self.stats.ok.wrapping_add(1);
                RxEvent::Frame(frame)
            }
            Err(err) => {
                self.count(err);
                RxEvent::Dropped(err)
            }
        }
    }

    /// Feeds a slice and returns the frames it completed, in order.
    pub fn push_slice(&mut self, bytes: &[u8]) -> Vec<Frame> {
        bytes
            .iter()
            .filter_map(|&b| match self.push(b) {
                RxEvent::Frame(f) => Some(f),
                RxEvent::None | RxEvent::Dropped(_) => None,
            })
            .collect()
    }
}
