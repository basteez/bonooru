//! G.26 (Story 5.0, retro action E4-A2): the keyboard routing module
//! `ocb_app::ui::keyboard`. Pure table tests over `egui::Event` values (no
//! `Context`), the frame-start harness moved from G.20, the pad key table
//! moved from G.24, the dialog helpers, and a source scan proving no other
//! file under `app/gui/src/` reads egui key events. No clock, no worker
//! thread, no wall-clock bound (E4-A4).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::collections::HashSet;
use std::path::Path;

use egui::accesskit::{self, Action};
use egui::{Event, Key, Modifiers, vec2};
use egui_kittest::Harness;
use ocb_app::gamepad::PadKey;
use ocb_app::ui::keyboard::{
    self, FrameContext, FrameKeys, KeyEdge, Rule, Shortcut, focus_route, frame_route, pad_route,
    shortcut,
};

fn esc(repeat: bool) -> Event {
    Event::Key {
        key: Key::Escape,
        physical_key: None,
        pressed: true,
        repeat,
        modifiers: Modifiers::NONE,
    }
}

fn key_event(key: Key, pressed: bool, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers,
    }
}

/// A key event as winit sends it: logical and physical key set.
fn ev(key: Key, pressed: bool, repeat: bool, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat,
        modifiers,
    }
}

fn press(key: Key, modifiers: Modifiers) -> Event {
    ev(key, true, false, modifiers)
}

fn repeat(key: Key, modifiers: Modifiers) -> Event {
    ev(key, true, true, modifiers)
}

/// Key releases in the next frame's input.
fn release<S>(h: &mut Harness<'_, S>, keys: &[Key]) {
    for k in keys {
        h.input_mut()
            .events
            .push(key_event(*k, false, Modifiers::NONE));
    }
}

const NONE: Modifiers = Modifiers::NONE;
const SHIFT: Modifiers = Modifiers::SHIFT;
const ALT: Modifiers = Modifiers::ALT;
const CTRL: Modifiers = Modifiers::CTRL;
const CMD: Modifiers = Modifiers::COMMAND;
const MAC_CMD: Modifiers = Modifiers::MAC_CMD;
const ALL_MODS: [Modifiers; 6] = [NONE, SHIFT, ALT, CTRL, CMD, MAC_CMD];

fn cx(text_edit: bool, widget_focused: bool, dialog_open: bool) -> FrameContext {
    FrameContext {
        text_edit,
        widget_focused,
        dialog_open,
    }
}

/// Every frame context (text edit implies a focused widget).
fn all_cx() -> Vec<FrameContext> {
    let mut v = Vec::new();
    for dialog in [false, true] {
        v.push(cx(false, false, dialog));
        v.push(cx(false, true, dialog));
        v.push(cx(true, true, dialog));
    }
    v
}

fn command(m: Modifiers) -> bool {
    m.command || m.ctrl || m.mac_cmd
}

/// The frame-start route's observable result.
#[derive(Debug, PartialEq, Eq)]
struct Out {
    rule: Option<Rule>,
    keep: bool,
    stop: bool,
    listen: bool,
    shortcut: Option<Shortcut>,
}

fn route(e: &Event, c: FrameContext) -> Out {
    match frame_route(e, c) {
        Some(r) => Out {
            rule: Some(r.rule),
            keep: r.keep,
            stop: r.stop,
            listen: r.listen,
            shortcut: r.shortcut,
        },
        None => Out {
            rule: None,
            keep: true,
            stop: false,
            listen: false,
            shortcut: None,
        },
    }
}

/// AC 2, 4–6, 8a: every rule of the table against every context flag, fresh
/// vs repeat and each modifier; every rule of `Rule::ALL` is exercised.
#[test]
fn g26_rule_table() {
    let mut covered: HashSet<Rule> = HashSet::new();
    let mut note = |o: &Out| {
        if let Some(r) = o.rule {
            covered.insert(r);
        }
    };
    for c in all_cx() {
        for m in ALL_MODS {
            let ctx = format!("{c:?} {m:?}");
            // EscStop: a fresh Esc, any modifiers, any context; read, kept.
            let o = route(&press(Key::Escape, m), c);
            note(&o);
            assert_eq!(o.rule, Some(Rule::EscStop), "{ctx}");
            assert!(o.stop && o.keep && !o.listen, "{ctx}");
            // An Esc repeat: nothing.
            let o = route(&repeat(Key::Escape, m), c);
            assert_eq!(o.rule, None, "Esc repeat {ctx}");
            assert!(!o.stop, "{ctx}");
            // An Esc release: nothing.
            assert_eq!(route(&ev(Key::Escape, false, false, m), c).rule, None);

            // Space, fresh and repeated.
            for rep in [false, true] {
                let o = route(&ev(Key::Space, true, rep, m), c);
                note(&o);
                let ctx = format!("{ctx} repeat={rep}");
                assert!(!o.stop && o.shortcut.is_none(), "{ctx}");
                if c.text_edit {
                    assert_eq!(o.rule, Some(Rule::SpaceText), "{ctx}");
                    assert!(o.keep && !o.listen, "{ctx}");
                } else if command(m) && !c.widget_focused {
                    assert_eq!(o.rule, Some(Rule::SpaceOs), "{ctx}");
                    assert!(o.keep && !o.listen, "{ctx}");
                } else if !rep && !command(m) && !m.alt && !c.dialog_open {
                    assert_eq!(o.rule, Some(Rule::SpaceListen), "{ctx}");
                    assert!(!o.keep && o.listen, "{ctx}");
                } else {
                    assert_eq!(o.rule, Some(Rule::SpaceConsume), "{ctx}");
                    assert!(!o.keep && !o.listen, "{ctx}");
                }
            }
            // A Space release: nothing.
            assert_eq!(route(&ev(Key::Space, false, false, m), c).rule, None);

            // Enter: a fresh press is a shortcut with `command` (Cmd on
            // macOS, Ctrl elsewhere; no Alt), else nothing (Ctrl alone is a
            // macOS Ctrl: no shortcut); a repeat is dropped outside a text
            // edit.
            let o = route(&press(Key::Enter, m), c);
            note(&o);
            if m.command {
                assert_eq!(o.rule, Some(Rule::Shortcut), "{ctx}");
                assert_eq!(o.shortcut, Some(Shortcut::PlayFromCursor), "{ctx}");
                assert!(o.keep, "{ctx}");
            } else {
                assert_eq!(o.rule, None, "{ctx}");
            }
            let o = route(&repeat(Key::Enter, m), c);
            note(&o);
            if c.text_edit {
                assert_eq!(o.rule, Some(Rule::EnterText), "{ctx}");
                assert!(o.keep, "{ctx}");
            } else {
                assert_eq!(o.rule, Some(Rule::EnterRepeat), "{ctx}");
                assert!(!o.keep, "{ctx}");
            }
            assert!(o.shortcut.is_none(), "a held shortcut is reported once");

            // A shortcut repeat (O) is not reported.
            assert_eq!(route(&repeat(Key::O, m), c).rule, None, "{ctx}");
            // Keys with no frame-start rule.
            for k in [Key::L, Key::Tab, Key::ArrowUp, Key::Backspace] {
                assert_eq!(route(&press(k, m), c).rule, None, "{k:?} {ctx}");
            }
        }
        // Shift never blocks Listen; Alt does (the Windows window menu).
        if !c.text_edit && !c.dialog_open {
            assert!(route(&press(Key::Space, SHIFT), c).listen, "{c:?} Shift");
            for m in [ALT, SHIFT | ALT] {
                let o = route(&press(Key::Space, m), c);
                assert_eq!(o.rule, Some(Rule::SpaceConsume), "{c:?} {m:?}");
                assert!(!o.listen && !o.keep, "{c:?} {m:?}");
            }
        }
    }
    // The gamepad live (a widget focused, no text edit): Space is Listen.
    assert!(route(&press(Key::Space, NONE), cx(false, true, false)).listen);
    // Cmd/Ctrl+Space with nothing focused stays for the OS.
    for m in [CMD, CTRL, MAC_CMD] {
        let o = route(&press(Key::Space, m), cx(false, false, false));
        assert!(o.keep && !o.listen, "{m:?}");
    }
    // Non-key events.
    assert_eq!(
        route(&Event::Text(" ".to_owned()), cx(false, false, false)).rule,
        None
    );

    // Gamepad pass (PadKey, PadCommand) and focus scan (Focus*).
    for m in ALL_MODS {
        let (rule, edge) = pad_route(&press(Key::L, m)).unwrap();
        covered.insert(rule);
        let want = if command(m) {
            Rule::PadCommand
        } else {
            Rule::PadKey
        };
        assert_eq!(rule, want, "{m:?}");
        assert_eq!(
            edge,
            KeyEdge {
                key: PadKey::L,
                pressed: true,
                repeat: false,
                command: command(m),
            }
        );
    }
    let (_, e) = pad_route(&ev(Key::L, false, false, CMD)).unwrap();
    assert!(!e.pressed && e.command, "a release with Cmd is an edge too");
    let (_, e) = pad_route(&repeat(Key::Enter, NONE)).unwrap();
    assert_eq!((e.key, e.repeat), (PadKey::Enter, true));
    // No physical key: nothing (no logical fallback).
    assert_eq!(pad_route(&key_event(Key::L, true, NONE)), None);
    for k in [Key::Space, Key::Escape, Key::Tab, Key::X] {
        assert_eq!(pad_route(&press(k, NONE)), None, "{k:?}");
    }

    for rep in [false, true] {
        for m in ALL_MODS {
            let e = ev(Key::Escape, true, rep, m);
            let want = (!m.any()).then_some(Rule::FocusEsc);
            assert_eq!(focus_route(&e), want, "Esc {m:?}");
            let e = ev(Key::Tab, true, rep, m);
            let want = (!m.any() || m.shift_only()).then_some(Rule::FocusTab);
            assert_eq!(focus_route(&e), want, "Tab {m:?} repeat={rep}");
            for k in [
                Key::ArrowUp,
                Key::ArrowDown,
                Key::ArrowLeft,
                Key::ArrowRight,
            ] {
                let want = (!m.any()).then_some(Rule::FocusArrow);
                assert_eq!(focus_route(&ev(k, true, rep, m)), want, "{k:?} {m:?}");
            }
        }
    }
    assert_eq!(focus_route(&ev(Key::Tab, false, false, NONE)), None);
    assert_eq!(focus_route(&press(Key::L, NONE)), None);
    let ak = |action| {
        Event::AccessKitActionRequest(accesskit::ActionRequest {
            action,
            target_tree: accesskit::TreeId::ROOT,
            target_node: accesskit::NodeId(1),
            data: None,
        })
    };
    assert_eq!(focus_route(&ak(Action::Focus)), Some(Rule::FocusAccessKit));
    assert_eq!(focus_route(&ak(Action::Click)), None);
    covered.extend([
        Rule::FocusEsc,
        Rule::FocusTab,
        Rule::FocusArrow,
        Rule::FocusAccessKit,
    ]);

    // LogicKeyUp.
    let raw = [
        ev(Key::L, false, false, CMD),
        ev(Key::L, true, false, NONE),
        key_event(Key::K, false, NONE),
        ev(Key::Space, false, false, NONE),
        ev(Key::ArrowUp, false, false, NONE),
    ];
    assert_eq!(keyboard::key_ups(&raw), vec![PadKey::L, PadKey::ArrowUp]);
    covered.insert(Rule::LogicKeyUp);

    // Dialog rules: `DialogFreshEsc` asserted here; `DialogArming` and
    // `DialogEsc` need a frame and are asserted in `g26_dialog_helpers`,
    // listed here only so this check names every rule of the table.
    assert!(keyboard::has_fresh_esc(&[esc(false)]));
    assert!(keyboard::has_fresh_esc(&[press(Key::Escape, SHIFT)]));
    assert!(!keyboard::has_fresh_esc(&[esc(true)]));
    assert!(!keyboard::has_fresh_esc(&[ev(
        Key::Escape,
        false,
        false,
        NONE
    )]));
    assert!(!keyboard::has_fresh_esc(&[press(Key::Enter, NONE)]));
    covered.insert(Rule::DialogFreshEsc);
    covered.extend([Rule::DialogArming, Rule::DialogEsc]);

    for (i, r) in Rule::ALL.into_iter().enumerate() {
        assert_eq!(r.index(), i, "{r:?} out of the table's order");
        assert!(covered.contains(&r), "{r:?} has no test row");
    }
    assert_eq!(covered.len(), Rule::ALL.len());
}

/// AC 5: the six global shortcuts and their near misses.
#[test]
fn g26_shortcuts() {
    let cmd_shift = CMD | SHIFT;
    // As egui-winit sends them: Cmd on macOS sets `mac_cmd` and `command`,
    // Ctrl on Windows/Linux `ctrl` and `command`.
    for m in [CMD, MAC_CMD | CMD, CTRL | CMD] {
        let s = m | SHIFT;
        assert_eq!(shortcut(Key::Enter, m), Some(Shortcut::PlayFromCursor));
        assert_eq!(shortcut(Key::Enter, s), Some(Shortcut::PlayFromStart));
        assert_eq!(shortcut(Key::O, m), Some(Shortcut::Open));
        assert_eq!(shortcut(Key::S, m), Some(Shortcut::Save));
        assert_eq!(shortcut(Key::Z, m), Some(Shortcut::Undo));
        assert_eq!(shortcut(Key::Z, s), Some(Shortcut::Redo));
        // Alt held: none.
        for k in [Key::Enter, Key::O, Key::S, Key::Z] {
            assert_eq!(shortcut(k, m | ALT), None, "{k:?} {m:?}");
        }
    }
    assert_eq!(shortcut(Key::O, cmd_shift), None);
    assert_eq!(shortcut(Key::S, cmd_shift), None);
    for k in [Key::P, Key::L, Key::Space, Key::Escape, Key::Num1] {
        assert_eq!(shortcut(k, CMD), None, "{k:?}");
    }
    // Ctrl on macOS (`ctrl` without `command`) is not a shortcut
    // (`EXPERIENCE.md`: Ctrl on Windows/Linux, Cmd on macOS).
    for m in [NONE, SHIFT, ALT, CTRL, CTRL | SHIFT, MAC_CMD] {
        for k in [Key::Enter, Key::O, Key::S, Key::Z] {
            assert_eq!(shortcut(k, m), None, "{k:?} {m:?}");
        }
    }
    // Matched by the logical key: on AZERTY the key labelled Z sits at the
    // US W position, and Cmd+Z (logical) is Undo whatever the physical key.
    let azerty_z = Event::Key {
        key: Key::Z,
        physical_key: Some(Key::W),
        pressed: true,
        repeat: false,
        modifiers: CMD,
    };
    let r = frame_route(&azerty_z, cx(false, false, false)).unwrap();
    assert_eq!(r.shortcut, Some(Shortcut::Undo));
    let azerty_w = Event::Key {
        key: Key::W,
        physical_key: Some(Key::Z),
        pressed: true,
        repeat: false,
        modifiers: CMD,
    };
    assert_eq!(frame_route(&azerty_w, cx(false, false, false)), None);
}

/// The harness of `g26_pad_keys_frame`.
#[derive(Default)]
struct PadState {
    edges: Vec<KeyEdge>,
    left: Vec<(Key, bool)>,
}

/// AC 8a: `pad_keys` in a frame: edges in order, plain mapped keys
/// consumed, Cmd/Ctrl ones and unmapped ones left in the input.
#[test]
fn g26_pad_keys_frame() {
    let mut h = Harness::builder()
        .with_size(vec2(200.0, 100.0))
        .build_ui_state(
            |ui, s: &mut PadState| {
                if ui.ctx().current_pass_index() == 0 {
                    s.edges = keyboard::pad_keys(ui.ctx());
                    s.left = ui.input(|i| {
                        i.events
                            .iter()
                            .filter_map(|e| match e {
                                Event::Key {
                                    key,
                                    pressed: true,
                                    modifiers,
                                    ..
                                } => Some((*key, command(*modifiers))),
                                _ => None,
                            })
                            .collect()
                    });
                }
            },
            PadState::default(),
        );
    h.run();
    h.input_mut().events.extend([
        press(Key::L, NONE),
        press(Key::L, CMD),
        press(Key::K, SHIFT),
        press(Key::X, NONE),
        key_event(Key::J, true, NONE),
    ]);
    h.step();
    let s = h.state();
    assert_eq!(
        s.edges
            .iter()
            .map(|e| (e.key, e.command))
            .collect::<Vec<_>>(),
        vec![(PadKey::L, false), (PadKey::L, true), (PadKey::K, false)]
    );
    assert_eq!(
        s.left,
        vec![(Key::L, true), (Key::X, false), (Key::J, false)],
        "Cmd+L, X and a J without a physical key stay; L and Shift+K go"
    );
}

/// The harness of `g26_dialog_helpers`.
#[derive(Default)]
struct DialogState {
    armed_blocked: bool,
    fresh: bool,
    took: bool,
    esc_left: bool,
}

/// AC 1, 8a: the dialog rules on a real frame: `DialogArming` (an answer
/// key down), `DialogFreshEsc`, `DialogEsc` (an Esc without modifiers is
/// consumed, a Cmd+Esc is not).
#[test]
fn g26_dialog_helpers() {
    let mut h = Harness::builder()
        .with_size(vec2(200.0, 100.0))
        .build_ui_state(
            |ui, s: &mut DialogState| {
                if ui.ctx().current_pass_index() == 0 {
                    let ctx = ui.ctx().clone();
                    s.armed_blocked = keyboard::answer_key_down(&ctx);
                    s.fresh = keyboard::fresh_esc(&ctx);
                    s.took = keyboard::dialog_takes_esc(&ctx);
                    s.esc_left = ui.input(|i| {
                        i.events.iter().any(|e| {
                            matches!(
                                e,
                                Event::Key {
                                    key: Key::Escape,
                                    ..
                                }
                            )
                        })
                    });
                }
            },
            DialogState::default(),
        );
    h.run();
    assert!(!h.state().armed_blocked);
    assert_eq!(keyboard::ANSWER_KEYS, [Key::Enter, Key::Space, Key::Escape]);
    for k in [Key::Enter, Key::Space, Key::Escape] {
        h.input_mut().events.push(key_event(k, true, NONE));
        h.step();
        assert!(h.state().armed_blocked, "{k:?} down");
        release(&mut h, &[k]);
        h.step();
        assert!(!h.state().armed_blocked, "{k:?} up");
    }
    h.input_mut().events.push(key_event(Key::Enter, true, NONE));
    h.step();
    h.input_mut()
        .events
        .push(key_event(Key::Escape, true, NONE));
    h.step();
    assert!(h.state().fresh && h.state().took && !h.state().esc_left);
    release(&mut h, &[Key::Escape, Key::Enter]);
    h.step();
    h.input_mut().events.push(key_event(Key::Escape, true, CMD));
    h.step();
    assert!(h.state().fresh, "a fresh Esc with Cmd");
    assert!(
        !h.state().took && h.state().esc_left,
        "Cmd+Esc taken by the dialog"
    );
    release(&mut h, &[Key::Escape]);
    h.step();
    // Shift and Alt do not stop the dialog taking Esc (egui's `consume_key`
    // ignores them).
    for m in [SHIFT, ALT] {
        h.input_mut().events.push(key_event(Key::Escape, true, m));
        h.step();
        assert!(h.state().took && !h.state().esc_left, "{m:?}+Esc not taken");
        release(&mut h, &[Key::Escape]);
        h.step();
    }
}

/// Tokens of egui key input that only `ui/keyboard.rs` may use (AC 1).
fn forbidden(code: &str) -> Vec<String> {
    let ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut hits = Vec::new();
    for sub in [
        "consume_key",
        "key_pressed",
        "key_released",
        "keys_down",
        "set_focus_lock_filter",
        "consume_shortcut",
        "num_presses",
        "EventFilter",
        "event_filter",
    ] {
        if code.contains(sub) {
            hits.push(sub.to_owned());
        }
    }
    // Word-bounded: `Key::` (not `PadKey::`), `egui::Key`, `Event::Key`.
    for tok in ["Key::", "egui::Key", "Event::Key"] {
        let mut from = 0;
        while let Some(i) = code[from..].find(tok) {
            let at = from + i;
            let before = code[..at].chars().next_back();
            let after = code[at + tok.len()..].chars().next();
            let word_end = tok.ends_with("::") || !after.is_some_and(ident);
            if !before.is_some_and(ident) && word_end {
                hits.push(tok.to_owned());
            }
            from = at + tok.len();
        }
    }
    // `InputState::key_down(…)`: any receiver but the pad model's
    // (`pad.key_down`, `p.key_down` in its unit tests) and its definition.
    let mut from = 0;
    while let Some(i) = code[from..].find("key_down(") {
        let at = from + i;
        let before = code[..at].trim_end();
        let receiver: String = before
            .strip_suffix('.')
            .unwrap_or("")
            .chars()
            .rev()
            .take_while(|c| ident(*c))
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        let allowed = before.ends_with("fn") || receiver == "pad" || receiver == "p";
        let word = !code[..at].chars().next_back().is_some_and(ident);
        if word && !allowed {
            hits.push("key_down(".to_owned());
        }
        from = at + "key_down(".len();
    }
    // `Key` as a word inside a `use egui::…;` / `use eframe::egui::…;`
    // statement, and glob imports of egui.
    for head in ["use egui::", "use eframe::egui::"] {
        let mut from = 0;
        while let Some(i) = code[from..].find(head) {
            let at = from + i;
            let end = code[at..].find(';').map_or(code.len(), |e| at + e);
            let stmt = &code[at..end];
            if stmt.split(|c: char| !ident(c)).any(|w| w == "Key") {
                hits.push("use egui::{Key}".to_owned());
            }
            if stmt.contains('*') {
                hits.push("use egui::*".to_owned());
            }
            from = end;
        }
    }
    hits
}

/// AC 1: one owner. No file under `app/gui/src/` but `ui/keyboard.rs` reads
/// egui key events; the old modules are gone. The scan's own checks first.
#[test]
fn g26_one_owner() {
    for bad in [
        "if i.consume_key(m, k) {}",
        "let k = Key::Escape;",
        "fn f(_: egui::Key) {}",
        "matches!(e, egui::Event::Key { .. })",
        "use egui::{Context, Key, Modifiers};",
        "m.set_focus_lock_filter(id, f);",
        "i.keys_down.len()",
        "if i.key_down(k) {}",
        "let p = i.num_presses(k);",
        "i.consume_shortcut(&s)",
        "let f = egui::EventFilter::default();",
        "use eframe::egui::{Key, Ui};",
        "use egui::*;",
    ] {
        assert!(!forbidden(bad).is_empty(), "missed: {bad}");
    }
    for ok in [
        "p.key_down(PadKey::L, 0);",
        "self.pad.key_down(e.key, now);",
        "pub fn key_down(&mut self, key: PadKey, now_us: u64) {",
        "let k = PadKey::ALL;",
        "use crate::gamepad::{PadKey, Pad};",
        "use egui::{Context, KeyboardShortcut};",
        "let monkey = 1;",
        "egui::Keyboard",
    ] {
        assert_eq!(forbidden(ok), Vec::<String>::new(), "false hit: {ok}");
    }
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    assert!(src.join("ui/keyboard.rs").is_file());
    assert!(!src.join("ui/keys.rs").exists(), "ui/keys.rs still there");
    assert!(
        !src.join("ui/pad_keys.rs").exists(),
        "ui/pad_keys.rs still there"
    );
    let mut files = Vec::new();
    common::rs_files(&src, &mut files);
    assert!(files.len() > 10);
    let mut offenders = Vec::new();
    for f in files {
        if f.ends_with("ui/keyboard.rs") {
            continue;
        }
        let code = common::code_only(&std::fs::read_to_string(&f).unwrap());
        for (n, line) in code.lines().enumerate() {
            for hit in forbidden(line) {
                offenders.push(format!("{}:{}: {hit}", f.display(), n + 1));
            }
        }
        // Multi-line `use egui::{…}` lists.
        for hit in forbidden(&code) {
            if hit.starts_with("use ") {
                offenders.push(format!("{}: {hit}", f.display()));
            }
        }
    }
    offenders.sort();
    offenders.dedup();
    assert!(
        offenders.is_empty(),
        "key input outside ui::keyboard:\n{}",
        offenders.join("\n")
    );
}

/// E4-A5, review 2026-11-28: `App::logic` takes its key releases from the
/// raw input. kittest cannot tell `raw.events` from `events` (it always runs
/// `logic` before `ui` on one input, so nothing is consumed yet when
/// `logic` reads it), so the call itself is pinned here.
#[test]
fn g26_logic_reads_raw_events() {
    let app = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app.rs");
    let code = common::code_only(&std::fs::read_to_string(app).unwrap());
    let compact: String = code.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(
        compact.contains("keyboard::key_ups(&i.raw.events)"),
        "App::logic must pass the raw input's events to keyboard::key_ups"
    );
    assert_eq!(compact.matches("keyboard::key_ups(").count(), 1);
}

/// The harness of `g26_frame_start_keys`: `frame_start`, then a text edit.
#[derive(Default)]
struct KeysState {
    keys: FrameKeys,
    esc_left: bool,
    space_left: bool,
    cmd_space_left: bool,
    text: String,
    focus: bool,
    dialog: bool,
    o_left: bool,
}

/// Moved from G.20 (Story 4.7 AC 4, 6; Story 5.0 AC 4–6, 8b): `frame_start` reads Esc without consuming it (also when a text
/// field had focus: egui's `Memory::begin_pass` clears that focus on Esc
/// before the read, and Esc is read anyway), consumes Space (Cmd/Ctrl+Space
/// only while a widget has focus), and leaves Space to a focused text edit;
/// since Story 5.0 also Listen (fresh Space outside a text edit and a
/// dialog) and the shortcuts (reported, left in the input).
#[test]
fn g26_frame_start_keys() {
    let edit_id = egui::Id::unique("g20-edit");
    let mut h = Harness::builder()
        .with_size(vec2(400.0, 200.0))
        .build_ui_state(
            move |ui, s: &mut KeysState| {
                // First pass only, as the app does (a discarded pass reruns
                // the frame with no input events).
                let first = ui.ctx().current_pass_index() == 0;
                if first {
                    s.keys = keyboard::frame_start(ui.ctx(), s.dialog);
                }
                let left = |k: Key, cmd: bool| {
                    ui.input(|i| {
                        i.events.iter().any(|e| {
                            matches!(e, Event::Key { key, pressed: true, modifiers, .. }
                                if *key == k && modifiers.command == cmd)
                        })
                    })
                };
                if first {
                    s.esc_left = left(Key::Escape, false);
                    s.space_left = left(Key::Space, false);
                    s.cmd_space_left = left(Key::Space, true);
                    s.o_left = left(Key::O, true);
                }
                let label = ui.label("Edit");
                let resp = ui
                    .add(egui::TextEdit::singleline(&mut s.text).id(edit_id))
                    .labelled_by(label.id);
                if s.focus {
                    resp.request_focus();
                }
            },
            KeysState::default(),
        );
    h.run();
    // No text edit focused: Esc read, kept; Space consumed; Cmd+Space kept.
    h.input_mut().events.push(esc(false));
    h.input_mut()
        .events
        .push(key_event(Key::Space, true, Modifiers::NONE));
    h.input_mut()
        .events
        .push(key_event(Key::Space, true, Modifiers::COMMAND));
    h.input_mut()
        .events
        .push(key_event(Key::O, true, Modifiers::COMMAND));
    release(&mut h, &[Key::Escape, Key::Space, Key::O]);
    h.step();
    assert!(h.state().keys.stop);
    assert!(h.state().keys.listen, "a fresh Space is Listen");
    assert_eq!(h.state().keys.shortcuts, vec![Shortcut::Open]);
    assert!(h.state().o_left, "Cmd+O consumed");
    assert!(h.state().esc_left, "Esc consumed");
    assert!(!h.state().space_left, "Space not consumed");
    assert!(h.state().cmd_space_left, "Cmd+Space consumed");
    // A focused text edit: Esc still read and kept, Space reaches it.
    h.state_mut().focus = true;
    h.run();
    assert!(h.ctx.text_edit_focused());
    h.state_mut().focus = false;
    h.input_mut()
        .events
        .push(key_event(Key::Space, true, Modifiers::NONE));
    h.input_mut().events.push(Event::Text(" ".to_owned()));
    release(&mut h, &[Key::Space]);
    h.input_mut()
        .events
        .push(key_event(Key::O, true, Modifiers::COMMAND));
    release(&mut h, &[Key::O]);
    h.step();
    assert!(!h.state().keys.stop);
    assert!(!h.state().keys.listen, "Listen in a text edit");
    assert_eq!(
        h.state().keys.shortcuts,
        vec![Shortcut::Open],
        "shortcuts are global"
    );
    assert!(h.state().space_left, "Space taken from the text edit");
    assert_eq!(h.state().text, " ", "the text edit got its space");
    assert!(
        h.ctx.text_edit_focused(),
        "the text edit has focus before the Esc"
    );
    h.input_mut().events.push(esc(false));
    h.step();
    assert!(h.state().keys.stop, "Esc with a focused text edit");
    assert!(h.state().esc_left);
    // Esc still held: the next press is a repeat (egui marks a press of a
    // key that is down as a repeat), no STOP.
    h.input_mut().events.push(esc(true));
    h.step();
    assert!(!h.state().keys.stop, "a repeat is not a fresh press");
    release(&mut h, &[Key::Escape]);
    h.step();
    // A dialog open (the caller says so): Space consumed, no Listen.
    h.state_mut().dialog = true;
    h.input_mut()
        .events
        .push(key_event(Key::Space, true, Modifiers::NONE));
    release(&mut h, &[Key::Space]);
    h.step();
    assert!(!h.state().keys.listen, "Listen with a dialog open");
    assert!(
        !h.state().space_left,
        "Space not consumed with a dialog open"
    );
    // Space held: its repeats never set Listen again.
    h.state_mut().dialog = false;
    h.input_mut()
        .events
        .push(key_event(Key::Space, true, Modifiers::NONE));
    h.step();
    assert!(h.state().keys.listen);
    h.input_mut().events.push(Event::Key {
        key: Key::Space,
        physical_key: None,
        pressed: true,
        repeat: true,
        modifiers: Modifiers::NONE,
    });
    h.step();
    assert!(!h.state().keys.listen, "a Space repeat is Listen");
    assert!(!h.state().space_left, "a Space repeat not consumed");
    release(&mut h, &[Key::Space]);
    h.step();
}

/// Moved from G.24 (Story 4.9 AC 4; Story 5.0 AC 8a): the egui ↔ pad key table: every mapped physical key gives its
/// `PadKey`, and the 18 keys are all reached; unmapped keys give none.
#[test]
fn g26_pad_keys_table() {
    let table = [
        (Key::ArrowUp, PadKey::ArrowUp),
        (Key::ArrowDown, PadKey::ArrowDown),
        (Key::ArrowLeft, PadKey::ArrowLeft),
        (Key::ArrowRight, PadKey::ArrowRight),
        (Key::W, PadKey::W),
        (Key::A, PadKey::A),
        (Key::S, PadKey::S),
        (Key::D, PadKey::D),
        (Key::H, PadKey::H),
        (Key::J, PadKey::J),
        (Key::K, PadKey::K),
        (Key::L, PadKey::L),
        (Key::Q, PadKey::Q),
        (Key::E, PadKey::E),
        (Key::Z, PadKey::Z),
        (Key::C, PadKey::C),
        (Key::Enter, PadKey::Enter),
        (Key::Backspace, PadKey::Backspace),
    ];
    for (k, p) in table {
        assert_eq!(keyboard::pad_key(k), Some(p), "{k:?}");
    }
    for p in PadKey::ALL {
        assert!(table.iter().any(|(_, q)| *q == p), "{p:?} unreachable");
    }
    for k in [
        Key::Space,
        Key::Tab,
        Key::Escape,
        Key::X,
        Key::Num1,
        Key::F,
        Key::Delete,
    ] {
        assert_eq!(keyboard::pad_key(k), None, "{k:?}");
    }
}
