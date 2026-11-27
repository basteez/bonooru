//! G.12: the modal dialog component and its host (Story 4.3 AC 10, 11):
//! frame, scrim, flourish, checklist rows, actions, initial focus and Enter,
//! the disabled look, one dialog at a time, Esc, no close on a backdrop
//! click, Tab kept inside. CPU only, shapes from `FullOutput::shapes`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::{Flat, inside, shapes_flat};
use egui::epaint::Shape;
use egui::{Color32, Event, Id, Key, Modifiers, PointerButton, Pos2, Rect, Role, Ui, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use ocb_app::ui::gallery::{readiness_dialog, update_dialog};
use ocb_app::ui::modal::{ModalAction, ModalBody, ModalOutcome, ModalSpec, Modals};
use ocb_app::ui::theme::{self, components::modal_dialog as md};
use ocb_app::ui::widgets::{self, ButtonKind};

#[derive(Default)]
struct State {
    modals: Modals,
    outcomes: Vec<(Id, ModalOutcome)>,
    behind_clicks: usize,
}

fn app(ui: &mut Ui, s: &mut State) {
    if widgets::button(ui, ButtonKind::Ghost, "Behind", true, None).clicked() {
        s.behind_clicks += 1;
    }
    if let Some(o) = s.modals.show(ui.ctx()) {
        s.outcomes.push(o);
    }
}

fn harness(w: f32, h: f32, ppp: f32) -> Harness<'static, State> {
    let mut h = Harness::builder()
        .with_size(vec2(w, h))
        .with_pixels_per_point(ppp)
        .with_context_setup(ocb_app::setup)
        .build_ui_state(app, State::default());
    h.run();
    h
}

fn open(h: &mut Harness<'_, State>, spec: ModalSpec) {
    h.state_mut().modals.request(spec);
    h.run();
}

fn flat(h: &Harness<'_, State>) -> Vec<Flat> {
    shapes_flat(&h.output().shapes)
}

/// The dialog frame: filled `panel-raised`, 1 pt `rim` stroke.
fn frame_rect(flat: &[Flat]) -> Rect {
    let frames: Vec<_> = flat
        .iter()
        .filter_map(|f| match &f.shape {
            Shape::Rect(r)
                if r.fill == theme::PANEL_RAISED
                    && r.stroke.color == theme::RIM
                    && r.stroke.width == 1.0 =>
            {
                Some(r.clone())
            }
            _ => None,
        })
        .collect();
    assert_eq!(frames.len(), 1, "one dialog frame: {frames:?}");
    assert_eq!(frames[0].corner_radius, theme::corner(theme::rounded::LG));
    frames[0].rect
}

fn scrims(flat: &[Flat]) -> usize {
    flat.iter()
        .filter(|f| matches!(&f.shape, Shape::Rect(r) if r.fill == theme::SCRIM))
        .count()
}

fn text<'a>(flat: &'a [Flat], s: &str) -> (Rect, usize, &'a egui::epaint::TextShape) {
    flat.iter()
        .find_map(|f| match &f.shape {
            Shape::Text(t) if t.galley.text() == s => {
                let ink = t.galley.rect.translate(t.pos.to_vec2());
                Some((ink, f.index, t))
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("no text `{s}`"))
}

#[test]
fn g12_frame() {
    for (w, hgt) in [(1024.0, 680.0), (1280.0, 800.0)] {
        for ppp in [1.0, 1.5] {
            for width in [400.0, 440.0, 450.0, 460.0, 500.0] {
                let mut h = harness(w, hgt, ppp);
                open(&mut h, readiness_dialog().with_width(width));
                let flat = flat(&h);
                let frame = frame_rect(&flat);
                let want = f32::clamp(width, md::MIN_WIDTH, md::WIDTH);
                let ctx = format!("{w}x{hgt} ppp {ppp} width {width}");
                assert!(
                    (frame.width() - want).abs() <= 0.5,
                    "{ctx}: {}",
                    frame.width()
                );
                let centre = h.ctx.content_rect().center();
                assert!(
                    (frame.center() - centre).length() <= 1.0,
                    "{ctx}: off centre"
                );
                // No drop shadow: no blurred rect around the frame.
                assert!(
                    !flat.iter().any(|f| matches!(&f.shape,
                        Shape::Rect(r) if r.blur_width > 0.0 && r.rect.contains_rect(frame.shrink(30.0)))),
                    "{ctx}: shadow under the frame"
                );
                let (title, _, _) = text(&flat, "Ready to play?");
                assert!(
                    title.top() >= frame.top() + md::PADDING_Y - 0.01,
                    "{ctx}: title top"
                );
                for label in ["Play", "Cancel"] {
                    let b = h.get_by_role_and_label(Role::Button, label).rect();
                    assert!(
                        b.right() <= frame.right() - md::PADDING_X + 0.5,
                        "{ctx}: {label} right {} > {}",
                        b.right(),
                        frame.right() - md::PADDING_X
                    );
                }
            }
        }
    }
}

#[test]
fn g12_scrim() {
    let mut h = harness(1024.0, 680.0, 1.25);
    assert_eq!(scrims(&flat(&h)), 0, "no dialog, no scrim");
    open(&mut h, update_dialog());
    let content = h.ctx.content_rect();
    let found: Vec<_> = flat(&h)
        .into_iter()
        .filter_map(|f| match f.shape {
            Shape::Rect(r) if r.fill == theme::SCRIM => Some(r.rect),
            _ => None,
        })
        .collect();
    assert_eq!(found, vec![content]);
}

#[test]
fn g12_flourish() {
    for ppp in [1.0, 1.25] {
        let mut h = harness(1280.0, 800.0, ppp);
        open(&mut h, update_dialog());
        let flat = flat(&h);
        let frame = frame_rect(&flat);
        let (title, _, _) = text(&flat, "Firmware update needed");
        let bx = Rect::from_min_size(
            Pos2::new(
                frame.center().x - md::FLOURISH_WIDTH / 2.0,
                title.bottom() + md::FLOURISH_MARGIN_TOP,
            ),
            vec2(md::FLOURISH_WIDTH, md::FLOURISH_HEIGHT),
        );
        let mine: Vec<&Flat> = flat
            .iter()
            .filter(|f| {
                common::uses(&f.shape, theme::RIM)
                    && inside(f.shape.visual_bounding_rect(), bx, 0.01)
            })
            .collect();
        let lines = mine
            .iter()
            .filter(
                |f| matches!(&f.shape, Shape::LineSegment { stroke, .. } if stroke.width == 1.0),
            )
            .count();
        let quads = mine
            .iter()
            .filter(|f| matches!(&f.shape, Shape::QuadraticBezier(q) if (q.stroke.width - 1.2).abs() < 1e-4))
            .count();
        let diamonds: Vec<_> = mine
            .iter()
            .filter_map(|f| match &f.shape {
                Shape::Path(p) if p.fill == theme::RIM => Some(p.points.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(
            (lines, quads, diamonds.len()),
            (2, 4, 1),
            "ppp {ppp}: {bx:?}"
        );
        assert!(ocb_app::ui::paint::is_convex(&diamonds[0]).is_ok());
    }
}

#[test]
fn g12_checklist() {
    for ppp in [1.0, 1.5] {
        let mut h = harness(1280.0, 800.0, ppp);
        open(&mut h, readiness_dialog());
        let flat = flat(&h);
        let frame = frame_rect(&flat);
        let rows = [
            "The ocarina is out in free play.",
            "Don\u{2019}t touch the controller until the song ends.",
            "Don\u{2019}t dock the Switch.",
        ];
        let diamonds: Vec<Vec<Pos2>> = flat
            .iter()
            .filter_map(|f| match &f.shape {
                Shape::Path(p) if p.fill == theme::AMBER && p.points.len() == 4 => {
                    Some(p.points.clone())
                }
                _ => None,
            })
            .collect();
        assert_eq!(diamonds.len(), 3, "ppp {ppp}");
        let rules: Vec<f32> = flat
            .iter()
            .filter_map(|f| match &f.shape {
                Shape::LineSegment { points, stroke } if stroke.color == theme::RULE => {
                    assert_eq!(stroke.width, 1.0, "rule width");
                    Some(points[0].y - stroke.width / 2.0)
                }
                _ => None,
            })
            .collect();
        assert_eq!(rules.len(), 2, "ppp {ppp}: no rule after the last row");
        // Row 0 starts where the flourish's bottom margin ends.
        let (title, _, _) = text(&flat, "Ready to play?");
        let flourish_bottom = title.bottom()
            + md::FLOURISH_MARGIN_TOP
            + md::FLOURISH_HEIGHT
            + md::FLOURISH_MARGIN_BOTTOM;
        let mut prev_bottom: Option<f32> = Some(flourish_bottom);
        let mut last_text_bottom = 0.0;
        for (i, row) in rows.iter().enumerate() {
            let (rect, _, t) = text(&flat, row);
            assert_eq!(t.galley.job.sections[0].format.font_id.size, 14.5, "{row}");
            let d = &diamonds[i];
            for k in 0..4 {
                let side = d[k].distance(d[(k + 1) % 4]);
                assert!((side - md::BULLET_SIZE).abs() < 0.01, "side {side}");
            }
            let right = d.iter().map(|p| p.x).fold(f32::MIN, f32::max);
            assert!(
                (rect.left() - right - md::ROW_GAP).abs() <= 0.5,
                "ppp {ppp}: gap {}",
                rect.left() - right
            );
            assert!(rect.left() >= frame.left() + md::PADDING_X - 0.01);
            // 7 pt above the text (from the rule or the row start) and 7 pt
            // below it (to the next rule).
            if let Some(top) = prev_bottom {
                assert!(
                    (rect.top() - top - md::ROW_PADDING_Y).abs() <= 0.5,
                    "ppp {ppp}: row {i} top padding {}",
                    rect.top() - top
                );
            }
            if let Some(rule) = rules.get(i) {
                assert!(
                    (rule - rect.bottom() - md::ROW_PADDING_Y).abs() <= 0.5,
                    "ppp {ppp}: row {i} bottom padding {}",
                    rule - rect.bottom()
                );
                prev_bottom = Some(rule + 1.0);
            }
            last_text_bottom = rect.bottom();
        }
        // The last row: 7 pt below its text, then the checklist's bottom
        // margin, then the action row.
        let play = h.get_by_role_and_label(Role::Button, "Play").rect();
        let gap = play.top() - last_text_bottom;
        assert!(
            (gap - md::ROW_PADDING_Y - md::CHECKLIST_MARGIN_BOTTOM).abs() <= 0.5,
            "ppp {ppp}: last row bottom padding + margin {gap}"
        );
    }
}

#[test]
fn g12_actions() {
    for ppp in [1.0, 1.25, 1.75] {
        let mut h = harness(1280.0, 800.0, ppp);
        open(&mut h, readiness_dialog());
        let play = h.get_by_role_and_label(Role::Button, "Play").rect();
        let cancel = h.get_by_role_and_label(Role::Button, "Cancel").rect();
        let flat = flat(&h);
        for (label, r) in [("Play", play), ("Cancel", cancel)] {
            assert!(
                (r.height() - md::ACTION_HEIGHT).abs() < 0.01,
                "{label} height"
            );
            let (t, _, _) = text(&flat, label);
            assert!(
                (r.width() - t.width() - 2.0 * md::ACTION_PADDING_X).abs() <= 0.5,
                "ppp {ppp}: {label} width {} vs text {}",
                r.width(),
                t.width()
            );
        }
        assert!(play.left() > cancel.right(), "primary right of ghost");
        assert!(
            (play.left() - cancel.right() - md::ACTION_GAP).abs() <= 0.5,
            "ppp {ppp}: gap {}",
            play.left() - cancel.right()
        );
        let fill = |color: Color32, at: Rect| {
            flat.iter().find_map(|f| match &f.shape {
                Shape::Rect(r) if r.fill == color && common::rect_eq(r.rect, at, 0.01) => {
                    Some((f.index, r.clone()))
                }
                _ => None,
            })
        };
        let (pi, _) = fill(theme::AMBER, play).expect("amber primary fill");
        let glow = flat
            .iter()
            .find(|f| matches!(&f.shape, Shape::Rect(r) if r.blur_width == 19.0 && r.fill == theme::AMBER_GLOW && r.rect.contains_rect(play)))
            .expect("primary glow");
        assert!(glow.index < pi, "glow under the primary");
        let (_, ghost) = fill(theme::WELL, cancel).expect("well ghost fill");
        assert!(
            flat.iter().any(|f| matches!(&f.shape, Shape::Rect(r)
                if r.stroke.color == theme::RIM && r.stroke.width == 1.0 && common::rect_eq(r.rect, cancel, 0.01)))
                || (ghost.stroke.color == theme::RIM && ghost.stroke.width == 1.0),
            "ghost 1 pt rim border"
        );
    }
}

#[test]
fn g12_initial_focus_and_enter() {
    let mut h = harness(1280.0, 800.0, 1.0);
    let spec = readiness_dialog();
    let id = spec.id;
    open(&mut h, spec.clone());
    assert!(
        h.get_by_role_and_label(Role::Button, "Play").is_focused(),
        "primary focused on open"
    );
    h.key_press(Key::Enter);
    h.run();
    assert_eq!(h.state().outcomes, vec![(id, ModalOutcome::Primary)]);
    assert!(!h.state().modals.is_open());

    open(&mut h, spec);
    h.get_by_role_and_label(Role::Button, "Cancel").focus();
    h.run();
    h.key_press(Key::Enter);
    h.run();
    assert_eq!(
        h.state().outcomes.last(),
        Some(&(id, ModalOutcome::Secondary))
    );
}

#[test]
fn g12_disabled_button() {
    let mut h = harness(1280.0, 800.0, 1.0);
    open(
        &mut h,
        ModalSpec::new(
            "disabled",
            "Disabled primary",
            ModalBody::Text("Body".to_owned()),
            ModalAction::new("Go").disabled(),
        )
        .with_secondary(ModalAction::new("Back").disabled()),
    );
    let flat = flat(&h);
    // Primary and ghost look the same when disabled.
    for label in ["Go", "Back"] {
        let rect = h.get_by_role_and_label(Role::Button, label).rect();
        disabled_look(&flat, rect, label);
    }
}

/// The painted disabled look (owner decision 2026-11-27), measured against
/// literals: an opaque `panel-raised` fill (`#221A13`), a 1 pt `rule-strong`
/// border (`#4A3826`) inside, the label in `text-muted` (`#C4A676`) at full
/// effective alpha (section alpha × the shape's `opacity_factor`), no glow
/// and no amber or rim anywhere on the button.
fn disabled_look(flat: &[Flat], go: Rect, label: &str) {
    // The topmost unblurred filled rect at the button (the parent `Ui` may
    // paint its own background there first).
    let face = flat
        .iter()
        .filter_map(|f| match &f.shape {
            Shape::Rect(r)
                if common::rect_eq(r.rect, go, 0.01)
                    && r.blur_width == 0.0
                    && r.fill != Color32::TRANSPARENT =>
            {
                Some(r.clone())
            }
            _ => None,
        })
        .next_back()
        .expect("disabled fill");
    assert_eq!(
        face.fill,
        Color32::from_rgb(0x22, 0x1a, 0x13),
        "{label}: fill"
    );
    let border = flat
        .iter()
        .filter_map(|f| match &f.shape {
            Shape::Rect(r) if common::rect_eq(r.rect, go, 0.01) && r.stroke.width > 0.0 => {
                Some(r.stroke)
            }
            _ => None,
        })
        .next_back()
        .expect("disabled border");
    assert_eq!(
        border.color,
        Color32::from_rgb(0x4a, 0x38, 0x26),
        "{label}: border"
    );
    assert_eq!(border.width, 1.0, "{label}: border width");
    let (_, _, t) = text(flat, label);
    let section = t.galley.job.sections[0].format.color;
    assert_eq!(
        section,
        Color32::from_rgb(0xc4, 0xa6, 0x76),
        "{label}: label colour"
    );
    let effective = f32::from(section.a()) * t.opacity_factor;
    assert!(
        (effective - 255.0).abs() <= 1.0,
        "{label}: label effective alpha {effective} (section {}, opacity {})",
        section.a(),
        t.opacity_factor
    );
    assert!(
        !flat.iter().any(
            |f| matches!(&f.shape, Shape::Rect(r) if r.blur_width > 0.0 && r.rect.intersects(go))
        ),
        "{label}: no glow when disabled"
    );
    for warm in [theme::AMBER, theme::AMBER_GLOW, theme::RIM] {
        assert!(
            !flat.iter().any(|f| common::uses(&f.shape, warm)
                && common::inside(f.shape.visual_bounding_rect(), go.expand(1.0), 0.01)),
            "{label}: {warm:?} on a disabled button"
        );
    }
}

/// A button asked enabled inside a disabled parent `Ui` is disabled in look
/// (the neutral disabled look, with no extra egui fade), in AccessKit and in
/// behaviour (a click does nothing).
#[test]
fn g12_button_in_a_disabled_parent() {
    let mut h = Harness::builder()
        .with_size(vec2(400.0, 200.0))
        .with_context_setup(ocb_app::setup)
        .build_ui_state(
            |ui, clicks: &mut usize| {
                ui.add_enabled_ui(false, |ui| {
                    if widgets::button(ui, ButtonKind::Primary, "Go", true, None).clicked() {
                        *clicks += 1;
                    }
                });
            },
            0_usize,
        );
    h.run();
    let node = h.get_by_role_and_label(Role::Button, "Go");
    assert!(node.accesskit_node().is_disabled(), "AccessKit disabled");
    let go = node.rect();
    node.click();
    h.run();
    assert_eq!(*h.state(), 0, "a disabled button clicked");
    disabled_look(&shapes_flat(&h.output().shapes), go, "Go");
}

#[test]
fn g12_one_at_a_time() {
    let mut h = harness(1280.0, 800.0, 1.0);
    let a = readiness_dialog();
    let b = update_dialog();
    // Each request built separately: dedup is by id, not by value identity.
    h.state_mut().modals.request(readiness_dialog());
    h.state_mut().modals.request(update_dialog());
    h.state_mut().modals.request(readiness_dialog());
    h.state_mut().modals.request(update_dialog());
    assert_eq!(
        h.state().modals.queued(),
        1,
        "duplicates ignored, B queued once"
    );
    let mut frames = 0;
    let mut step_checked = |h: &mut Harness<'_, State>| {
        let mut shown = 0;
        for _ in 0..8 {
            h.step();
            frames += 1;
            let n = scrims(&flat(h));
            assert!(n <= 1, "two scrims in one frame");
            shown += n;
        }
        shown
    };
    assert!(step_checked(&mut h) >= 6, "A shown in the stepped frames");
    h.run();
    assert_eq!(h.query_all_by_role(Role::Dialog).count(), 1);
    assert!(
        h.query_by_role_and_label(Role::Dialog, "Ready to play?")
            .is_some()
    );
    assert_eq!(scrims(&flat(&h)), 1);
    h.key_press(Key::Escape);
    assert!(step_checked(&mut h) >= 6, "B shown after A closed");
    h.run();
    assert_eq!(h.state().outcomes, vec![(a.id, ModalOutcome::Dismissed)]);
    assert_eq!(h.query_all_by_role(Role::Dialog).count(), 1);
    assert!(
        h.query_by_role_and_label(Role::Dialog, "Firmware update needed")
            .is_some()
    );
    h.key_press(Key::Escape);
    assert_eq!(step_checked(&mut h), 0, "nothing shown after B closed");
    h.run();
    assert_eq!(
        h.state().outcomes.last(),
        Some(&(b.id, ModalOutcome::Dismissed))
    );
    assert_eq!(h.query_all_by_role(Role::Dialog).count(), 0);
    assert_eq!(scrims(&flat(&h)), 0);
    assert!(frames >= 24);
}

fn click_at(h: &Harness<'_, State>, pos: Pos2) {
    h.event(Event::PointerMoved(pos));
    for pressed in [true, false] {
        h.event(Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
    }
}

#[test]
fn g12_esc_backdrop_tab() {
    let mut h = harness(1280.0, 800.0, 1.0);
    let behind = h.get_by_role_and_label(Role::Button, "Behind").rect();
    let spec = readiness_dialog();
    open(&mut h, spec.clone());
    h.key_press(Key::Escape);
    h.run();
    assert_eq!(h.state().outcomes, vec![(spec.id, ModalOutcome::Dismissed)]);

    // A click on the backdrop (over the button behind) does nothing.
    open(&mut h, spec.clone());
    click_at(&h, behind.center());
    h.run();
    assert!(
        h.state().modals.is_open(),
        "backdrop click closed the dialog"
    );
    assert_eq!(
        h.state().behind_clicks,
        0,
        "click went through the backdrop"
    );
    assert_eq!(h.state().outcomes.len(), 1);

    // Tab and Shift+Tab stay inside the dialog.
    let inside_labels = ["Play", "Cancel"];
    for (n, modifiers) in [(5, Modifiers::NONE), (5, Modifiers::SHIFT)] {
        for _ in 0..n {
            h.key_press_modifiers(modifiers, Key::Tab);
            h.run();
            let focused: Vec<String> = h
                .root()
                .children_recursive()
                .filter(|node| node.is_focused())
                .map(|node| node.accesskit_node().label().unwrap_or_default())
                .collect();
            assert_eq!(focused.len(), 1, "one focused node: {focused:?}");
            assert!(
                inside_labels.contains(&focused[0].as_str()),
                "focus left the dialog: {focused:?}"
            );
        }
    }
    assert!(h.state().modals.is_open());
}

/// Each outcome is kept in the host until the requester reads it.
#[test]
fn g12_outcome_kept_until_read() {
    let mut h = harness(1280.0, 800.0, 1.0);
    let a = readiness_dialog();
    let b = update_dialog();
    open(&mut h, a.clone());
    h.state_mut().modals.request(b.clone());
    h.key_press(Key::Enter);
    h.run();
    h.key_press(Key::Escape);
    h.run();
    for _ in 0..3 {
        h.run(); // frames pass, nothing is read yet
    }
    let m = &mut h.state_mut().modals;
    assert_eq!(m.take_outcome(b.id), Some(ModalOutcome::Dismissed));
    assert_eq!(m.take_outcome(a.id), Some(ModalOutcome::Primary));
    assert_eq!(m.take_outcome(a.id), None, "read once");
    assert_eq!(m.take_outcome(Id::unique("never-requested")), None);
}

/// After a click on the backdrop the primary still has focus, so Enter
/// still answers.
#[test]
fn g12_enter_after_a_backdrop_click() {
    let mut h = harness(1280.0, 800.0, 1.0);
    let spec = readiness_dialog();
    open(&mut h, spec.clone());
    click_at(&h, Pos2::new(5.0, 5.0));
    h.run();
    assert!(h.state().modals.is_open());
    assert!(
        h.get_by_role_and_label(Role::Button, "Play").is_focused(),
        "focus lost after the backdrop click"
    );
    h.key_press(Key::Enter);
    h.run();
    assert_eq!(h.state().outcomes, vec![(spec.id, ModalOutcome::Primary)]);
}

fn key_event(h: &Harness<'_, State>, key: Key, pressed: bool, repeat: bool) {
    h.event(Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat,
        modifiers: Modifiers::NONE,
    });
}

/// A held Enter (with key repeat) answers the dialog it was pressed in, not
/// the queued one that opens while it is still down; once released, the
/// next press answers that one.
#[test]
fn g12_held_enter_does_not_answer_the_queued_dialog() {
    let mut h = harness(1280.0, 800.0, 1.0);
    let a = readiness_dialog();
    let b = update_dialog();
    open(&mut h, a.clone());
    h.state_mut().modals.request(b.clone());
    key_event(&h, Key::Enter, true, false);
    h.step();
    for _ in 0..20 {
        key_event(&h, Key::Enter, true, true);
        h.step();
    }
    assert_eq!(h.state().outcomes, vec![(a.id, ModalOutcome::Primary)]);
    assert_eq!(h.state().modals.open_id(), Some(b.id), "B still open");
    key_event(&h, Key::Enter, false, false);
    h.run();
    assert_eq!(
        h.state().modals.open_id(),
        Some(b.id),
        "release alone answers nothing"
    );
    h.key_press(Key::Enter);
    h.run();
    assert_eq!(
        h.state().outcomes.last(),
        Some(&(b.id, ModalOutcome::Primary))
    );
}

/// Esc pressed in the frame a dialog is requested closes it.
#[test]
fn g12_esc_on_the_opening_frame() {
    let mut h = harness(1280.0, 800.0, 1.0);
    let spec = update_dialog();
    h.state_mut().modals.request(spec.clone());
    h.key_press(Key::Escape);
    h.step();
    h.run();
    assert!(!h.state().modals.is_open(), "dialog still open");
    assert_eq!(h.state().outcomes, vec![(spec.id, ModalOutcome::Dismissed)]);
}

/// The frame border goes through the stroke floor: at ppp 0.5 one
/// physical pixel is 2 pt.
#[test]
fn g12_border_floor() {
    let mut h = harness(1280.0, 800.0, 0.5);
    open(&mut h, update_dialog());
    let widths: Vec<f32> = flat(&h)
        .iter()
        .filter_map(|f| match &f.shape {
            Shape::Rect(r) if r.fill == theme::PANEL_RAISED && r.stroke.color == theme::RIM => {
                Some(r.stroke.width)
            }
            _ => None,
        })
        .collect();
    assert_eq!(widths, vec![2.0], "border at ppp 0.5");
}
