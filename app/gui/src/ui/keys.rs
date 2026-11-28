//! Keys read at the top of every frame (Story 4.7; UX-DR41, UX-DR61,
//! UX-DR62), before any widget or dialog runs.
//!
//! - **Esc is STOP, always.** A fresh press (`repeat == false`, any
//!   modifiers) sets [`FrameKeys::stop`]. It is read, never consumed: the
//!   open dialog, if any, consumes the same Esc afterwards and closes as
//!   `Dismissed` (the contract on [`super::modal::Modals`]), so one press
//!   does both (except an Esc with Cmd/Ctrl, which `Modals` ignores, and
//!   the firmware dialog's opening frame, Story 4.6). Key repeats of a
//!   held Esc never ask again.
//! - **Buttons activate with Enter only (UX-DR61).** egui fake-clicks a
//!   focused clickable widget on Space or Enter, whatever the modifiers,
//!   and counts key repeats. Unless a text edit has focus, every Space
//!   press is consumed here (Cmd/Ctrl+Space too while a widget has focus;
//!   with nothing focused Cmd/Ctrl+Space stays for the OS and later
//!   shortcuts), and Enter key repeats are dropped, so a held Enter
//!   activates a focused button once, like Esc. A focused text edit keeps
//!   Space and Enter repeats. Epic 5 adds preview play/stop on Space here.
//!
//! Call [`frame_start`] once per frame, in the first pass only
//! (`ctx.current_pass_index() == 0`): input events reach only that pass
//! (egui 0.37 `RawInput::take`). egui's own `Memory::begin_pass` already
//! clears keyboard focus on Esc before the app runs; that stays.

use egui::{Context, Event, Key, Modifiers};

/// What the frame's keys ask for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameKeys {
    /// A fresh Esc press: request STOP.
    pub stop: bool,
}

/// Cmd, Ctrl or macOS Cmd held: the one definition of "a Cmd/Ctrl combo"
/// (also `ui::pad_keys`, Story 4.9).
pub(crate) fn command_held(m: Modifiers) -> bool {
    m.command || m.ctrl || m.mac_cmd
}

/// Reads Esc (not consumed), consumes Space and drops Enter repeats
/// (unless a text edit has focus). See the module doc.
pub fn frame_start(ctx: &Context) -> FrameKeys {
    let stop = ctx.input(|i| {
        i.events.iter().any(|e| {
            matches!(
                e,
                Event::Key {
                    key: Key::Escape,
                    pressed: true,
                    repeat: false,
                    ..
                }
            )
        })
    });
    if !ctx.text_edit_focused() {
        let focused = ctx.memory(|m| m.focused()).is_some();
        ctx.input_mut(|i| {
            i.events.retain(|e| match e {
                Event::Key {
                    key: Key::Space,
                    pressed: true,
                    modifiers,
                    ..
                } => !(focused || !command_held(*modifiers)),
                Event::Key {
                    key: Key::Enter,
                    pressed: true,
                    repeat: true,
                    ..
                } => false,
                _ => true,
            });
        });
    }
    FrameKeys { stop }
}
