//! On-screen gamepad model: which buttons are held, from pointer and keyboard, ignoring key
//! auto-repeat, and the 8-byte controller state from docs/link-protocol.md. GUI-free.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Button {
    A,
    B,
    X,
    Y,
    L,
    R,
    Up,
    Down,
    Left,
    Right,
}

pub const ALL: [Button; 10] = [
    Button::A,
    Button::B,
    Button::X,
    Button::Y,
    Button::L,
    Button::R,
    Button::Up,
    Button::Down,
    Button::Left,
    Button::Right,
];

impl Button {
    /// Bit in the `buttons:u16` field. Placeholder layout (Switch HID order Y,B,A,X,L,R).
    fn bit(self) -> Option<u16> {
        Some(match self {
            Button::Y => 0,
            Button::B => 1,
            Button::A => 2,
            Button::X => 3,
            Button::L => 4,
            Button::R => 5,
            _ => return None,
        })
    }
    fn index(self) -> usize {
        ALL.iter().position(|b| *b == self).unwrap()
    }
}

/// Held-state per source, so releasing a key does not release a button the mouse holds.
#[derive(Default, Debug)]
pub struct Pad {
    keys: [bool; 10],
    pointer: [bool; 10],
    pub presses: u32,
    pub releases: u32,
    pub repeats_ignored: u32,
}

impl Pad {
    pub fn held(&self, b: Button) -> bool {
        self.keys[b.index()] || self.pointer[b.index()]
    }

    /// Feed a key event. Returns true if the visible state changed.
    pub fn key(&mut self, b: Button, pressed: bool, repeat: bool) -> bool {
        if repeat {
            self.repeats_ignored += 1;
            return false;
        }
        self.set(b, pressed, true)
    }

    pub fn pointer(&mut self, b: Button, down: bool) -> bool {
        self.set(b, down, false)
    }

    fn set(&mut self, b: Button, on: bool, key: bool) -> bool {
        let before = self.held(b);
        let slot = if key {
            &mut self.keys[b.index()]
        } else {
            &mut self.pointer[b.index()]
        };
        *slot = on;
        let after = self.held(b);
        if after != before {
            if after {
                self.presses += 1;
            } else {
                self.releases += 1;
            }
        }
        after != before
    }

    /// Window lost focus: release everything (a stuck button is worse than a dropped one).
    pub fn release_all(&mut self) {
        for b in ALL {
            self.set(b, false, true);
            self.set(b, false, false);
        }
    }

    /// `buttons:u16 | hat:u8 | lx ly rx ry | flags` (link-protocol.md, "Controller state").
    pub fn state(&self) -> [u8; 8] {
        let mut buttons = 0u16;
        for b in ALL {
            if let (true, Some(bit)) = (self.held(b), b.bit()) {
                buttons |= 1 << bit;
            }
        }
        let (u, d, l, r) = (
            self.held(Button::Up),
            self.held(Button::Down),
            self.held(Button::Left),
            self.held(Button::Right),
        );
        // hat: 0=N,1=NE,2=E,3=SE,4=S,5=SW,6=W,7=NW,8=neutral; opposite directions cancel
        let hat = match (u && !d, d && !u, r && !l, l && !r) {
            (true, _, false, false) => 0,
            (true, _, true, _) => 1,
            (false, false, true, _) => 2,
            (_, true, true, _) => 3,
            (_, true, false, false) => 4,
            (_, true, _, true) => 5,
            (false, false, _, true) => 6,
            (true, _, _, true) => 7,
            _ => 8,
        };
        let [b0, b1] = buttons.to_le_bytes();
        [b0, b1, hat, 0x80, 0x80, 0x80, 0x80, 0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeat_is_ignored_and_sources_are_independent() {
        let mut p = Pad::default();
        assert!(p.key(Button::A, true, false));
        assert!(!p.key(Button::A, true, true));
        assert!(!p.key(Button::A, true, true));
        assert_eq!(p.presses, 1);
        assert_eq!(p.repeats_ignored, 2);
        p.pointer(Button::A, true);
        assert!(!p.key(Button::A, false, false), "pointer still holds A");
        assert!(p.held(Button::A));
        assert!(p.pointer(Button::A, false));
        assert_eq!(p.releases, 1);
    }

    #[test]
    fn state_bytes() {
        let mut p = Pad::default();
        assert_eq!(p.state(), [0, 0, 8, 0x80, 0x80, 0x80, 0x80, 0]);
        p.key(Button::A, true, false);
        p.key(Button::R, true, false);
        p.key(Button::Up, true, false);
        p.key(Button::Right, true, false);
        assert_eq!(p.state()[0], 0b0010_0100);
        assert_eq!(p.state()[2], 1);
        p.key(Button::Down, true, false); // up+down cancel -> E
        assert_eq!(p.state()[2], 2);
        p.release_all();
        assert_eq!(p.state(), [0, 0, 8, 0x80, 0x80, 0x80, 0x80, 0]);
    }
}
