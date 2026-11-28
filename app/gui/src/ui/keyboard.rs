//! Every keyboard rule of the app, in one place (Story 5.0, retro action
//! E4-A2; Stories 4.7 and 4.9; `EXPERIENCE.md` › Interaction Primitives).
//! Only this module reads egui key events; the rest of the app acts on what
//! it returns.
//!
//! The rules ([`Rule`], in this order; [`Rule::ALL`] is the table the tests
//! iterate):
//!
//! | Rule | Phase | Key and context | Action | Event |
//! |---|---|---|---|---|
//! | `EscStop` | frame start | fresh Esc, any modifiers, any context | STOP | read, left in the input |
//! | `SpaceText` | frame start | Space press, a text edit focused | none (the text edit types it) | left |
//! | `SpaceOs` | frame start | Cmd/Ctrl+Space press, nothing focused | none (the OS's) | left |
//! | `SpaceListen` | frame start | fresh Space, no Cmd/Ctrl, no text edit, no dialog | Listen (preview play/stop) | consumed |
//! | `SpaceConsume` | frame start | any other Space press outside a text edit | none: Space never activates a button | consumed |
//! | `EnterRepeat` | frame start | Enter key repeat, no text edit focused | none: a held Enter activates once | consumed |
//! | `EnterText` | frame start | Enter key repeat, a text edit focused | none (the text edit's) | left |
//! | `Shortcut` | frame start | fresh Cmd/Ctrl combo of [`Shortcut`], any context | reported | left |
//! | `PadKey` | gamepad pass | a mapped physical key, no Cmd/Ctrl | a [`KeyEdge`] | consumed |
//! | `PadCommand` | gamepad pass | a mapped physical key with Cmd/Ctrl | a [`KeyEdge`] with `command` | left |
//! | `FocusEsc` | gamepad pass | Esc press without modifiers | the panel loses focus | read |
//! | `FocusTab` | gamepad pass | Tab or Shift+Tab press (repeats too) | lock fallback, `source=tab` | read |
//! | `FocusArrow` | gamepad pass | unmodified arrow press (repeats too) | lock fallback | read |
//! | `FocusAccessKit` | gamepad pass | AccessKit `Focus` action | no lock fallback | read |
//! | `LogicKeyUp` | `logic`, every pass | a mapped physical key's release in the raw input | the pad releases it | read |
//! | `DialogArming` | dialog | Enter, Space or Esc down | the dialog takes no answer until all are up | read |
//! | `DialogFreshEsc` | dialog, opening frame | fresh Esc | the dialog may still be dismissed | read |
//! | `DialogEsc` | dialog | Esc without modifiers, top dialog, no popup | `Dismissed` | consumed |
//!
//! **Phases.** [`frame_start`] runs once per frame in the first pass
//! (`ctx.current_pass_index() == 0`: input reaches only that pass, egui
//! 0.37 `RawInput::take`), before any widget, in every mode. The gamepad
//! pass ([`focus_scan`], [`pad_keys`], [`lock_focus`]) runs in the first
//! pass of the main screen after it was drawn: it depends on the panel's
//! focus. [`key_ups`] runs in `App::logic` in every pass, also when eframe
//! skips `ui` (a hidden window): a release seen only there still releases
//! (E4-A5). The dialog helpers run in `ui::modal`. egui's own
//! `Memory::begin_pass` already clears keyboard focus on Esc before the app
//! runs; that stays.
//!
//! **Esc is STOP, always.** Read, never consumed by [`frame_start`]: the
//! open dialog, if any, consumes the same Esc afterwards ([`dialog_takes_esc`])
//! and closes as `Dismissed`, so one press does both (except Esc with a
//! modifier, which the dialog ignores, and the firmware dialog's opening
//! frame, Story 4.6). Key repeats of a held Esc never ask again.
//!
//! **Buttons activate with Enter only (UX-DR61).** egui fake-clicks a
//! focused clickable widget on Space or Enter, whatever the modifiers, and
//! counts key repeats. So every Space press outside a text edit is consumed
//! at frame start (Cmd/Ctrl+Space too while a widget has focus; with nothing
//! focused it stays for the OS), and Enter key repeats are dropped.
//!
//! **Listen on Space** (Story 5.11 acts on it). A fresh Space press without
//! Cmd/Ctrl (Shift and Alt do not block it, as for the gamepad keys), with
//! no text edit focused and no dialog open, sets [`FrameKeys::listen`],
//! whatever widget or the gamepad panel has focus (Space is not a gamepad
//! key). Accepted limit: [`frame_start`] cannot know of a dialog requested
//! later in the same frame (the firmware dialog), so a Space in that very
//! frame still sets `listen`; the consumer re-checks the dialog host.
//!
//! **Global shortcuts** ([`Shortcut`]): fresh Cmd/Ctrl presses matched by
//! the logical key (so Cmd+Z follows the layout's letters), Alt held
//! matches none; reported in every context in [`FrameKeys::shortcuts`] and
//! left in the input. Since Story 5.0 nothing acts on them; the story that
//! adds each feature decides whether its row is consumed.
//!
//! **Text fields.** One test, [`Context::text_edit_focused`] read once at
//! frame start: a focused text edit keeps Space and Enter repeats, gets no
//! Listen; Esc is still STOP and the shortcuts are still reported.
//!
//! **Gamepad keys** (Story 4.9): matched by `Event::Key::physical_key`
//! (positional on any layout); an event without one does nothing (no
//! fallback to the logical key). Esc and Space are never mapped.

use egui::accesskit::Action;
use egui::{Context, Event, EventFilter, Key, Modifiers};

use crate::gamepad::PadKey;

/// One keyboard rule (the module doc's table, in its order).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Rule {
    EscStop,
    SpaceText,
    SpaceOs,
    SpaceListen,
    SpaceConsume,
    EnterRepeat,
    EnterText,
    Shortcut,
    PadKey,
    PadCommand,
    FocusEsc,
    FocusTab,
    FocusArrow,
    FocusAccessKit,
    LogicKeyUp,
    DialogArming,
    DialogFreshEsc,
    DialogEsc,
}

impl Rule {
    /// Every rule, in the table's order.
    pub const ALL: [Self; 18] = [
        Self::EscStop,
        Self::SpaceText,
        Self::SpaceOs,
        Self::SpaceListen,
        Self::SpaceConsume,
        Self::EnterRepeat,
        Self::EnterText,
        Self::Shortcut,
        Self::PadKey,
        Self::PadCommand,
        Self::FocusEsc,
        Self::FocusTab,
        Self::FocusArrow,
        Self::FocusAccessKit,
        Self::LogicKeyUp,
        Self::DialogArming,
        Self::DialogFreshEsc,
        Self::DialogEsc,
    ];

    /// The rule's place in [`Rule::ALL`]. Exhaustive: a new rule does not
    /// compile until it has a place (and G.26 then wants a test row).
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::EscStop => 0,
            Self::SpaceText => 1,
            Self::SpaceOs => 2,
            Self::SpaceListen => 3,
            Self::SpaceConsume => 4,
            Self::EnterRepeat => 5,
            Self::EnterText => 6,
            Self::Shortcut => 7,
            Self::PadKey => 8,
            Self::PadCommand => 9,
            Self::FocusEsc => 10,
            Self::FocusTab => 11,
            Self::FocusArrow => 12,
            Self::FocusAccessKit => 13,
            Self::LogicKeyUp => 14,
            Self::DialogArming => 15,
            Self::DialogFreshEsc => 16,
            Self::DialogEsc => 17,
        }
    }
}

/// A global shortcut (`EXPERIENCE.md` › Global shortcuts).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shortcut {
    /// Cmd/Ctrl+Enter.
    PlayFromCursor,
    /// Cmd/Ctrl+Shift+Enter.
    PlayFromStart,
    /// Cmd/Ctrl+O.
    Open,
    /// Cmd/Ctrl+S.
    Save,
    /// Cmd/Ctrl+Z.
    Undo,
    /// Cmd/Ctrl+Shift+Z.
    Redo,
}

/// Cmd, Ctrl or macOS Cmd held: the one definition of "a Cmd/Ctrl combo".
#[must_use]
pub fn command_held(m: Modifiers) -> bool {
    m.command || m.ctrl || m.mac_cmd
}

/// The shortcut of a key press, if any: Cmd/Ctrl held, Alt not; Shift
/// tells the paired shortcuts apart. `key` is the logical key.
#[must_use]
pub fn shortcut(key: Key, m: Modifiers) -> Option<Shortcut> {
    if !command_held(m) || m.alt {
        return None;
    }
    Some(match (key, m.shift) {
        (Key::Enter, false) => Shortcut::PlayFromCursor,
        (Key::Enter, true) => Shortcut::PlayFromStart,
        (Key::O, false) => Shortcut::Open,
        (Key::S, false) => Shortcut::Save,
        (Key::Z, false) => Shortcut::Undo,
        (Key::Z, true) => Shortcut::Redo,
        _ => return None,
    })
}

/// What [`frame_start`] needs to know about the frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameContext {
    /// A text edit has keyboard focus.
    pub text_edit: bool,
    /// Some widget has keyboard focus.
    pub widget_focused: bool,
    /// A dialog is open (known at frame start).
    pub dialog_open: bool,
}

/// One frame-start decision for one event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameRoute {
    pub rule: Rule,
    /// Left in the input (else consumed).
    pub keep: bool,
    pub stop: bool,
    pub listen: bool,
    pub shortcut: Option<Shortcut>,
}

impl FrameRoute {
    const fn new(rule: Rule, keep: bool) -> Self {
        Self {
            rule,
            keep,
            stop: false,
            listen: false,
            shortcut: None,
        }
    }
}

/// The frame-start rule for one event, if any (pure).
#[must_use]
pub fn frame_route(e: &Event, cx: FrameContext) -> Option<FrameRoute> {
    let Event::Key {
        key,
        pressed: true,
        repeat,
        modifiers,
        ..
    } = e
    else {
        return None;
    };
    let fresh = !*repeat;
    let command = command_held(*modifiers);
    match key {
        Key::Escape if fresh => Some(FrameRoute {
            stop: true,
            ..FrameRoute::new(Rule::EscStop, true)
        }),
        Key::Space if cx.text_edit => Some(FrameRoute::new(Rule::SpaceText, true)),
        Key::Space if command && !cx.widget_focused => Some(FrameRoute::new(Rule::SpaceOs, true)),
        Key::Space if fresh && !command && !cx.dialog_open => Some(FrameRoute {
            listen: true,
            ..FrameRoute::new(Rule::SpaceListen, false)
        }),
        Key::Space => Some(FrameRoute::new(Rule::SpaceConsume, false)),
        Key::Enter if *repeat && !cx.text_edit => Some(FrameRoute::new(Rule::EnterRepeat, false)),
        Key::Enter if *repeat => Some(FrameRoute::new(Rule::EnterText, true)),
        _ if fresh => shortcut(*key, *modifiers).map(|s| FrameRoute {
            shortcut: Some(s),
            ..FrameRoute::new(Rule::Shortcut, true)
        }),
        _ => None,
    }
}

/// What the frame's keys ask for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FrameKeys {
    /// A fresh Esc press: request STOP.
    pub stop: bool,
    /// A fresh Space press for the preview (Story 5.11 acts on it).
    pub listen: bool,
    /// The frame's global shortcuts, in order (nothing acts on them yet).
    pub shortcuts: Vec<Shortcut>,
}

/// Applies the frame-start rules: reads Esc, Listen and the shortcuts,
/// consumes what the table says. Call once per frame, in the first pass
/// only, before any widget. `dialog_open`: a dialog is open now.
pub fn frame_start(ctx: &Context, dialog_open: bool) -> FrameKeys {
    let cx = FrameContext {
        text_edit: ctx.text_edit_focused(),
        widget_focused: ctx.memory(|m| m.focused()).is_some(),
        dialog_open,
    };
    let mut keys = FrameKeys::default();
    ctx.input_mut(|i| {
        i.events.retain(|e| {
            let Some(r) = frame_route(e, cx) else {
                return true;
            };
            keys.stop |= r.stop;
            keys.listen |= r.listen;
            keys.shortcuts.extend(r.shortcut);
            r.keep
        });
    });
    keys
}

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

/// The gamepad-pass rule for one event, if any (pure): `PadKey` (consumed)
/// or `PadCommand` (left in the input).
#[must_use]
pub fn pad_route(e: &Event) -> Option<(Rule, KeyEdge)> {
    let Event::Key {
        physical_key: Some(physical),
        pressed,
        repeat,
        modifiers,
        ..
    } = e
    else {
        return None;
    };
    let key = pad_key(*physical)?;
    let command = command_held(*modifiers);
    let rule = if command {
        Rule::PadCommand
    } else {
        Rule::PadKey
    };
    Some((
        rule,
        KeyEdge {
            key,
            pressed: *pressed,
            repeat: *repeat,
            command,
        },
    ))
}

/// Reads this frame's mapped key events in order and consumes those without
/// Cmd/Ctrl. Call it only in the first pass of a frame in which the panel
/// has focus (then in every such frame, also when nothing is applied, so no
/// later widget sees them: Story 4.9 D8, review 2026-11-28).
pub fn pad_keys(ctx: &Context) -> Vec<KeyEdge> {
    let mut edges = Vec::new();
    ctx.input_mut(|i| {
        i.events.retain(|e| {
            let Some((rule, edge)) = pad_route(e) else {
                return true;
            };
            edges.push(edge);
            rule == Rule::PadCommand
        });
    });
    edges
}

/// The focus-relevant keys of the frame (the gamepad panel's focus).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FocusKeys {
    /// Esc without modifiers (egui clears focus on it).
    pub esc_plain: bool,
    /// Tab, Shift+Tab or an unmodified arrow: egui's focus navigation acts
    /// on it, so the lock's first-frame fallback may pull focus back.
    pub lock: bool,
    /// Tab or Shift+Tab.
    pub tab: bool,
    /// An AccessKit `Focus` action.
    pub accesskit_focus: bool,
}

/// The focus rule for one event, if any (pure). Only what egui's own focus
/// navigation acts on (review 2026-11-28): unmodified arrows and Tab,
/// Shift-only Shift+Tab. Key repeats count too: `Memory::begin_pass` (egui
/// 0.37 `memory/mod.rs` ~l.583) moves the focus on a repeated press as on a
/// fresh one.
#[must_use]
pub fn focus_route(e: &Event) -> Option<Rule> {
    match e {
        Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } => match key {
            Key::Escape if !modifiers.any() => Some(Rule::FocusEsc),
            Key::Tab if !modifiers.any() || modifiers.shift_only() => Some(Rule::FocusTab),
            Key::ArrowUp | Key::ArrowDown | Key::ArrowLeft | Key::ArrowRight
                if !modifiers.any() =>
            {
                Some(Rule::FocusArrow)
            }
            _ => None,
        },
        Event::AccessKitActionRequest(r) if r.action == Action::Focus => Some(Rule::FocusAccessKit),
        _ => None,
    }
}

/// Reads the frame's focus keys (nothing consumed).
#[must_use]
pub fn focus_scan(ctx: &Context) -> FocusKeys {
    ctx.input(|i| {
        let mut k = FocusKeys::default();
        for e in &i.events {
            match focus_route(e) {
                Some(Rule::FocusEsc) => k.esc_plain = true,
                Some(Rule::FocusTab) => {
                    k.tab = true;
                    k.lock = true;
                }
                Some(Rule::FocusArrow) => k.lock = true,
                Some(Rule::FocusAccessKit) => k.accesskit_focus = true,
                _ => {}
            }
        }
        k
    })
}

/// The gamepad panel's focus-lock filter (Story 4.9 AC 2, D11): Tab and the
/// arrows stay on the panel, Esc still leaves it.
#[must_use]
pub fn lock_filter() -> EventFilter {
    EventFilter {
        tab: true,
        horizontal_arrows: true,
        vertical_arrows: true,
        escape: false,
    }
}

/// Sets [`lock_filter`] on the focused widget `id` (egui accepts it only
/// from the widget's second frame of focus; set it after any
/// `request_focus` of the frame, which resets the filter).
pub fn lock_focus(ctx: &Context, id: egui::Id) {
    ctx.memory_mut(|m| m.set_focus_lock_filter(id, lock_filter()));
}

/// The pad keys released in `events` (rule `LogicKeyUp`; pure). `App::logic`
/// passes the raw input's events: eframe may run `logic` only and drop the
/// window's events for `ui`, so a key-up read there still ends its hold.
#[must_use]
pub fn key_ups(events: &[Event]) -> Vec<PadKey> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Key {
                pressed: false,
                physical_key: Some(k),
                ..
            } => pad_key(*k),
            _ => None,
        })
        .collect()
}

/// Keys that answer a dialog: Enter and Space click the focused button
/// (Space is consumed at frame start, so in practice Enter), Esc dismisses.
pub const ANSWER_KEYS: [Key; 3] = [Key::Enter, Key::Space, Key::Escape];

/// Rule `DialogArming`: an answer key is down.
#[must_use]
pub fn answer_key_down(ctx: &Context) -> bool {
    ctx.input(|i| ANSWER_KEYS.iter().any(|k| i.key_down(*k)))
}

/// Rule `DialogFreshEsc` (pure over the events): a fresh Esc press.
#[must_use]
pub fn has_fresh_esc(events: &[Event]) -> bool {
    events.iter().any(|e| {
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
}

/// Rule `DialogFreshEsc` on the frame's input.
#[must_use]
pub fn fresh_esc(ctx: &Context) -> bool {
    ctx.input(|i| has_fresh_esc(&i.events))
}

/// Rule `DialogEsc`: consumes an Esc without modifiers; true if there was
/// one. Call only for the top dialog with no popup open.
pub fn dialog_takes_esc(ctx: &Context) -> bool {
    ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
}
