//! Keys read at the top of every frame (Story 4.7; UX-DR41, UX-DR61,
//! UX-DR62), before any widget or dialog runs.
//!
//! - **Esc is STOP, always.** A fresh press (`repeat == false`, any
//!   modifiers) sets [`FrameKeys::stop`]. It is read, never consumed: the
//!   open dialog, if any, consumes the same Esc afterwards and closes as
//!   `Dismissed` (the contract on [`super::modal::Modals`]), so one press
//!   does both. Key repeats of a held Esc never ask again.
//! - **Space is consumed here** so it never activates a focused button
//!   (egui fake-clicks a focused clickable widget on Space or Enter);
//!   Cmd/Ctrl+Space stays for the OS and later shortcuts, and a focused
//!   text edit keeps Space. Epic 5 adds preview play/stop on it here.
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

fn command_held(m: Modifiers) -> bool {
    m.command || m.ctrl || m.mac_cmd
}

/// Reads Esc (not consumed) and consumes Space (unless a text edit has
/// focus or Cmd/Ctrl is held). See the module doc.
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
        ctx.input_mut(|i| {
            i.events.retain(|e| {
                !matches!(
                    e,
                    Event::Key {
                        key: Key::Space,
                        pressed: true,
                        modifiers,
                        ..
                    } if !command_held(*modifiers)
                )
            });
        });
    }
    FrameKeys { stop }
}
