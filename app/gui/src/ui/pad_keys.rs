//! The gamepad's keys (Story 4.9; UX-DR37, D1, D7, D8): egui's key events
//! turned into [`PadKey`] edges by their **physical** key, read by the app
//! only while the panel has keyboard focus, in the frame's first pass, after
//! the main screen was drawn.
//!
//! - A key is matched by `Event::Key::physical_key` (positional on any
//!   layout); an event without one does nothing (no fallback to the logical
//!   key, D1).
//! - Mapped events without Cmd/Ctrl are consumed after they are read, so no
//!   later widget sees them while the gamepad has focus (D8). Events with
//!   Cmd/Ctrl stay in the input for the app's global shortcuts (D7).
//! - Esc and Space are never mapped: Esc is STOP (read by
//!   [`super::keys::frame_start`], never consumed), Space is consumed there.
//!
//! Separate from `ui::keys` because `keys::frame_start` runs before every
//! widget in every mode, while these keys depend on the panel's focus.

use egui::{Context, Event, Key};

use super::keys::command_held;
use crate::gamepad::PadKey;

/// The pad key at a physical key position, if any (the only egui ↔ model
/// mapping; the table is `gamepad::PadKey`'s).
#[must_use]
pub fn pad_key(physical: Key) -> Option<PadKey> {
    Some(match physical {
        Key::ArrowUp => PadKey::ArrowUp,
        Key::ArrowDown => PadKey::ArrowDown,
        Key::ArrowLeft => PadKey::ArrowLeft,
        Key::ArrowRight => PadKey::ArrowRight,
        Key::W => PadKey::W,
        Key::A => PadKey::A,
        Key::S => PadKey::S,
        Key::D => PadKey::D,
        Key::H => PadKey::H,
        Key::J => PadKey::J,
        Key::K => PadKey::K,
        Key::L => PadKey::L,
        Key::Q => PadKey::Q,
        Key::E => PadKey::E,
        Key::Z => PadKey::Z,
        Key::C => PadKey::C,
        Key::Enter => PadKey::Enter,
        Key::Backspace => PadKey::Backspace,
        _ => return None,
    })
}

/// One mapped key event, in the order the frame received them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEdge {
    /// The pad key (from the physical key).
    pub key: PadKey,
    /// A press (else a release).
    pub pressed: bool,
    /// egui flags a press of a key already down (an OS key repeat).
    pub repeat: bool,
    /// Cmd, Ctrl or macOS Cmd was held: a press is left alone.
    pub command: bool,
}

/// Reads this frame's mapped key events in order and consumes those without
/// Cmd/Ctrl (see the module doc). Call it only in the first pass of a frame
/// in which the panel has focus.
pub fn take(ctx: &Context) -> Vec<KeyEdge> {
    let mut edges = Vec::new();
    ctx.input_mut(|i| {
        i.events.retain(|e| {
            let Event::Key {
                physical_key: Some(physical),
                pressed,
                repeat,
                modifiers,
                ..
            } = e
            else {
                return true;
            };
            let Some(key) = pad_key(*physical) else {
                return true;
            };
            let command = command_held(*modifiers);
            edges.push(KeyEdge {
                key,
                pressed: *pressed,
                repeat: *repeat,
                command,
            });
            command
        });
    });
    edges
}
