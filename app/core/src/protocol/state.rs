//! The canonical 8-byte controller state (AD-10): `buttons` u16 LE (P1 bit
//! order, `BTN_*`), `hat` (0-7 clockwise from up, 8 = neutral), `lx, ly, rx,
//! ry` (centred at 0x80, `ly` 0x00 = up), `flags` (bit0 END, others
//! reserved). The codec does not validate field ranges (hat 9..=255, button
//! bits 14 and 15): the controller decides, as in `state.h`.

use super::consts::{HAT_NEUTRAL, STATE_FLAG_END, STATE_RESERVED_MASK, STATE_SIZE, STICK_CENTER};

/// One controller state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct State {
    /// `BTN_*` bits.
    pub buttons: u16,
    /// `HAT_*`.
    pub hat: u8,
    /// Left stick X.
    pub lx: u8,
    /// Left stick Y (0x00 = up).
    pub ly: u8,
    /// Right stick X.
    pub rx: u8,
    /// Right stick Y.
    pub ry: u8,
    /// `STATE_FLAG_END`; other bits reserved.
    pub flags: u8,
}

/// No button, hat neutral, sticks centred, no flag. On the wire:
/// `00 00 08 80 80 80 80 00`. The only definition in the app (AD-10).
pub const NEUTRAL: State = State {
    buttons: 0,
    hat: HAT_NEUTRAL,
    lx: STICK_CENTER,
    ly: STICK_CENTER,
    rx: STICK_CENTER,
    ry: STICK_CENTER,
    flags: 0,
};

impl State {
    /// Wire bytes.
    #[must_use]
    pub fn to_bytes(&self) -> [u8; STATE_SIZE] {
        let [b0, b1] = self.buttons.to_le_bytes();
        [
            b0, b1, self.hat, self.lx, self.ly, self.rx, self.ry, self.flags,
        ]
    }

    /// From wire bytes.
    #[must_use]
    pub fn from_bytes(bytes: &[u8; STATE_SIZE]) -> Self {
        let [b0, b1, hat, lx, ly, rx, ry, flags] = *bytes;
        Self {
            buttons: u16::from_le_bytes([b0, b1]),
            hat,
            lx,
            ly,
            rx,
            ry,
            flags,
        }
    }

    /// Every field but `flags` equals [`NEUTRAL`] (AD-8: END only with a
    /// neutral pose).
    #[must_use]
    pub fn is_neutral_pose(&self) -> bool {
        Self { flags: 0, ..*self } == NEUTRAL
    }

    /// Every field equals [`NEUTRAL`], flags included (`NEUTRAL|END` is not
    /// neutral).
    #[must_use]
    pub fn is_neutral(&self) -> bool {
        *self == NEUTRAL
    }

    /// The END flag is set.
    #[must_use]
    pub fn has_end(&self) -> bool {
        self.flags & STATE_FLAG_END != 0
    }

    /// Reserved flag bits (1..=7) are zero. Buttons are not validated.
    #[must_use]
    pub fn reserved_bits_ok(&self) -> bool {
        self.flags & STATE_RESERVED_MASK == 0
    }
}

impl Default for State {
    fn default() -> Self {
        NEUTRAL
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn predicates() {
        let end = State {
            flags: STATE_FLAG_END,
            ..NEUTRAL
        };
        assert!(NEUTRAL.is_neutral() && NEUTRAL.is_neutral_pose() && !NEUTRAL.has_end());
        assert!(
            end.is_neutral_pose() && !end.is_neutral() && end.has_end() && end.reserved_bits_ok()
        );
        let a = State {
            buttons: 0x0004,
            ..NEUTRAL
        };
        assert!(!a.is_neutral_pose());
        for bit in 1..8 {
            let s = State {
                flags: 1 << bit,
                ..NEUTRAL
            };
            assert!(!s.reserved_bits_ok(), "bit {bit}");
        }
        assert_eq!(NEUTRAL.to_bytes(), [0, 0, 8, 0x80, 0x80, 0x80, 0x80, 0]);
        assert_eq!(State::from_bytes(&NEUTRAL.to_bytes()), NEUTRAL);
    }
}
