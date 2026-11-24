//! Frames (AD-3): raw = `type | seq | len | body[len] | crc16 LE`, the CRC over
//! type, seq, len and body; COBS-encoded and closed by a `0x00` delimiter.
//!
//! [`decode_frame`] runs the first four receive checks of AD-3 in order: COBS,
//! length consistency (reported as a CRC error), CRC, type range. The seq and
//! semantic checks need session state and belong to the link (Story 3.3).
//! Same classification as `ocb_frame_decode` (Story 2.2 D13, D18).

use std::fmt;

use super::cobs;
use super::consts::{
    FRAME_CRC_SIZE, FRAME_DELIMITER, FRAME_HEADER_SIZE, MAX_BODY, MAX_ENCODED_FRAME, MAX_RAW_FRAME,
    MIN_RAW_FRAME, TYPE_APP_MIN, TYPE_BRIDGE_MAX, TYPE_CTRL_MIN,
};
use super::crc16::crc16;

/// Which end receives: decides the accepted type range (AD-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RxSide {
    /// Accepts `0x01..=0x7F`: app frames and bridge heartbeats.
    Controller,
    /// Accepts `0x80..=0xFF`: controller frames. The app's side.
    App,
}

/// Why a frame was refused. The names are the vector error names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    /// Zero-length run (an idle or session-opening `0x00`); never an error
    /// on the line, only counted.
    Empty,
    /// More than `MAX_ENCODED_FRAME` bytes without a delimiter.
    Overlong,
    /// Malformed COBS.
    Cobs,
    /// Length inconsistency or CRC mismatch.
    Crc,
    /// Type outside the receiver's range.
    Range,
    /// Encoding only: body longer than `MAX_BODY`.
    BodyTooLong,
}

impl FrameError {
    /// The vector error name (`vectors/README.md`).
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Empty => "EMPTY",
            Self::Overlong => "OVERLONG",
            Self::Cobs => "COBS",
            Self::Crc => "CRC",
            Self::Range => "RANGE",
            Self::BodyTooLong => "BODY_TOO_LONG",
        }
    }
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "frame error {}", self.name())
    }
}

impl std::error::Error for FrameError {}

/// One decoded frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// Message type.
    pub ty: u8,
    /// Sequence number.
    pub seq: u8,
    /// Body, at most `MAX_BODY` bytes.
    pub body: Vec<u8>,
}

/// True if `ty` is in the receive range of `side`.
#[must_use]
pub fn type_in_range(side: RxSide, ty: u8) -> bool {
    match side {
        RxSide::Controller => (TYPE_APP_MIN..=TYPE_BRIDGE_MAX).contains(&ty),
        RxSide::App => ty >= TYPE_CTRL_MIN,
    }
}

/// Encodes one frame: COBS bytes plus the trailing `0x00` (at most
/// `MAX_WIRE_FRAME` bytes). The type range is not checked.
pub fn encode_frame(ty: u8, seq: u8, body: &[u8]) -> Result<Vec<u8>, FrameError> {
    let len = u8::try_from(body.len()).map_err(|_| FrameError::BodyTooLong)?;
    if body.len() > MAX_BODY {
        return Err(FrameError::BodyTooLong);
    }
    let mut raw = Vec::with_capacity(FRAME_HEADER_SIZE + body.len() + FRAME_CRC_SIZE);
    raw.extend_from_slice(&[ty, seq, len]);
    raw.extend_from_slice(body);
    let crc = crc16(&raw);
    raw.extend_from_slice(&crc.to_le_bytes());
    let mut out = cobs::encode(&raw);
    out.push(FRAME_DELIMITER);
    Ok(out)
}

/// Decodes one frame from `enc`, the run **without** its delimiter.
pub fn decode_frame(side: RxSide, enc: &[u8]) -> Result<Frame, FrameError> {
    if enc.is_empty() {
        return Err(FrameError::Empty);
    }
    if enc.len() > MAX_ENCODED_FRAME {
        return Err(FrameError::Overlong);
    }
    // 1. COBS: any malformed input is a COBS error, checked over the whole
    // input first. A well-formed input that decodes to more than the largest
    // raw frame cannot satisfy the length rule: a CRC error (AD-3).
    let raw = cobs::decode(enc).map_err(|_| FrameError::Cobs)?;
    // 2. Length consistency, reported as a CRC error.
    if raw.len() < MIN_RAW_FRAME || raw.len() > MAX_RAW_FRAME {
        return Err(FrameError::Crc);
    }
    let &[ty, seq, len, ..] = raw.as_slice() else {
        return Err(FrameError::Crc);
    };
    if raw.len() != FRAME_HEADER_SIZE + usize::from(len) + FRAME_CRC_SIZE {
        return Err(FrameError::Crc);
    }
    // 3. CRC over type, seq, len, body; sent little-endian.
    let (covered, crc_bytes) = raw
        .split_at_checked(raw.len() - FRAME_CRC_SIZE)
        .ok_or(FrameError::Crc)?;
    let [lo, hi] = *crc_bytes else {
        return Err(FrameError::Crc);
    };
    if crc16(covered) != u16::from_le_bytes([lo, hi]) {
        return Err(FrameError::Crc);
    }
    // 4. Type range of this receiver.
    if !type_in_range(side, ty) {
        return Err(FrameError::Range);
    }
    let body = covered
        .get(FRAME_HEADER_SIZE..)
        .ok_or(FrameError::Crc)?
        .to_vec();
    Ok(Frame { ty, seq, body })
}
