//! COBS (Consistent Overhead Byte Stuffing), written from the published
//! description: S. Cheshire and M. Baker, IEEE/ACM Transactions on Networking
//! 7(2):159-172, 1999. Same rules as `firmware/common/proto/cobs.c`:
//!
//! - a code byte `0x01..=0xFE` is followed by `code - 1` non-zero bytes and an
//!   implied `0x00`, unless the block ends the input;
//! - `0xFF` is followed by 254 non-zero bytes and no implied `0x00`; a `0xFF`
//!   block that ends the input is not followed by a final code byte;
//! - the empty input encodes to `0x01`.
//!
//! The delimiter `0x00` is not part of the encoding.

use std::fmt;

/// Why a COBS input could not be decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CobsError {
    /// Zero-length input (a valid encoding has at least one code byte).
    Empty,
    /// A `0x00` byte inside the encoded input.
    Zero,
    /// A code byte announces more bytes than the input holds.
    Truncated,
}

impl fmt::Display for CobsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Empty => "COBS: empty input",
            Self::Zero => "COBS: zero byte inside the encoding",
            Self::Truncated => "COBS: block runs past the end of the input",
        })
    }
}

impl std::error::Error for CobsError {}

fn set(out: &mut [u8], pos: usize, value: u8) {
    if let Some(slot) = out.get_mut(pos) {
        *slot = value;
    }
}

/// Encodes `input` (no delimiter added). The output never contains `0x00`
/// and is at most `input.len() + input.len() / 254 + 1` bytes long.
#[must_use]
pub fn encode(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len() + input.len() / 254 + 1);
    let mut code_pos = 0usize;
    let mut code: u8 = 1;
    out.push(0); // the first block's code byte, filled in later
    for (i, &b) in input.iter().enumerate() {
        if b != 0 {
            out.push(b);
            code += 1;
            if code != 0xFF {
                continue;
            }
            // 254 data bytes: close the block, no implied zero.
            set(&mut out, code_pos, 0xFF);
            code = 1;
            if i + 1 == input.len() {
                return out;
            }
        } else {
            set(&mut out, code_pos, code);
            code = 1;
        }
        // Reserve the next block's code byte.
        code_pos = out.len();
        out.push(0);
    }
    set(&mut out, code_pos, code);
    out
}

/// Decodes `input` (without its delimiter). Any malformed input is an error.
pub fn decode(input: &[u8]) -> Result<Vec<u8>, CobsError> {
    if input.is_empty() {
        return Err(CobsError::Empty);
    }
    let mut out = Vec::with_capacity(input.len());
    let mut rest = input;
    while let Some((&code, tail)) = rest.split_first() {
        if code == 0 {
            return Err(CobsError::Zero);
        }
        let n = usize::from(code) - 1;
        let (block, after) = tail.split_at_checked(n).ok_or(CobsError::Truncated)?;
        if block.contains(&0) {
            return Err(CobsError::Zero);
        }
        out.extend_from_slice(block);
        if code != 0xFF && !after.is_empty() {
            out.push(0);
        }
        rest = after;
    }
    Ok(out)
}
