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
        let mut prev_bottom: Option<f32> = None;
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
        }
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
            .find(|f| matches!(&f.shape, Shape::Rect(r) if r.blur_width == 12.0 && r.fill == theme::AMBER_GLOW && r.rect.contains_rect(play)))
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
        .with_secondary(ModalAction::new("Back")),
    );
    let go = h.get_by_role_and_label(Role::Button, "Go").rect();
    let flat = flat(&h);
    let a = (0.45_f32 * 255.0).round() as u8;
    let fill = flat
        .iter()
        .find_map(|f| match &f.shape {
            Shape::Rect(r) if common::rect_eq(r.rect, go, 0.01) && r.blur_width == 0.0 => {
                Some(r.fill)
            }
            _ => None,
        })
        .expect("disabled fill");
    assert_eq!(fill, theme::AMBER.gamma_multiply(0.45));
    assert!(fill.a().abs_diff(a) <= 1, "fill alpha {}", fill.a());
    let (_, _, t) = text(&flat, "Go");
    let color = t.galley.job.sections[0].format.color;
    assert!(color.a().abs_diff(a) <= 1, "text alpha {}", color.a());
    assert!(
        !flat.iter().any(
            |f| matches!(&f.shape, Shape::Rect(r) if r.blur_width > 0.0 && r.rect.contains_rect(go))
        ),
        "no glow when disabled"
    );
}

#[test]
fn g12_one_at_a_time() {
    let mut h = harness(1280.0, 800.0, 1.0);
    let a = readiness_dialog();
    let b = update_dialog();
    h.state_mut().modals.request(a.clone());
    h.state_mut().modals.request(b.clone());
    h.state_mut().modals.request(a.clone());
    assert_eq!(h.state().modals.queued(), 1, "duplicate ignored, B queued");
    let mut frames = 0;
    let mut step_checked = |h: &mut Harness<'_, State>| {
        for _ in 0..8 {
            h.step();
            frames += 1;
            assert!(scrims(&flat(h)) <= 1, "two scrims in one frame");
        }
    };
    step_checked(&mut h);
    h.run();
    assert_eq!(h.query_all_by_role(Role::Dialog).count(), 1);
    assert!(
        h.query_by_role_and_label(Role::Dialog, "Ready to play?")
            .is_some()
    );
    assert_eq!(scrims(&flat(&h)), 1);
    h.key_press(Key::Escape);
    step_checked(&mut h);
    h.run();
    assert_eq!(h.state().outcomes, vec![(a.id, ModalOutcome::Dismissed)]);
    assert_eq!(h.query_all_by_role(Role::Dialog).count(), 1);
    assert!(
        h.query_by_role_and_label(Role::Dialog, "Firmware update needed")
            .is_some()
    );
    h.key_press(Key::Escape);
    step_checked(&mut h);
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
