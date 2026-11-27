//! G.21 (Story 4.7 owner eye check): hover and pressed states of every
//! button. An enabled button's fill and border are mixed toward
//! `colors.text` under the pointer and toward black while pressed
//! (`components.state-layer`); geometry, label, glow and focus ring do not
//! change; a disabled button shows no change. Checked on the frame shapes
//! for the shared dialog buttons, STOP (on the main screen) and the
//! gallery's static samples.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use egui::epaint::{RectShape, Shape};
use egui::{Color32, Event, Key, Modifiers, PointerButton, Rect, Role, Ui, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use ocb_app::app::WorkerHealth;
use ocb_app::gating::Gate;
use ocb_app::leds;
use ocb_app::ui::main_screen;
use ocb_app::ui::paint::ButtonState::{self, Hovered, Idle, Pressed};
use ocb_app::ui::theme::{self, components::state_layer as sl};
use ocb_app::ui::widgets::{self, ButtonKind};

/// The rect shapes drawn exactly over `rect`: (fill shapes, stroke shapes).
fn rects_at(flat: &[common::Flat], rect: Rect) -> (Vec<RectShape>, Vec<RectShape>) {
    let at: Vec<RectShape> = flat
        .iter()
        .filter_map(|f| match &f.shape {
            Shape::Rect(r) if common::rect_eq(r.rect, rect, 0.01) && r.blur_width == 0.0 => {
                Some(r.clone())
            }
            _ => None,
        })
        .collect();
    let fills = at
        .iter()
        .filter(|r| r.fill != Color32::TRANSPARENT)
        .cloned()
        .collect();
    let strokes = at
        .iter()
        .filter(|r| r.fill == Color32::TRANSPARENT && r.stroke.width > 0.0)
        .cloned()
        .collect();
    (fills, strokes)
}

fn press(h: &mut Harness<'_, impl Sized>, pos: egui::Pos2, pressed: bool) {
    h.input_mut().events.push(Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    });
    h.step();
}

#[test]
fn g21_tint() {
    for c in [
        theme::AMBER,
        theme::WELL,
        theme::RIM,
        theme::STOP_IDLE_FILL,
        theme::EMBER,
    ] {
        assert_eq!(Idle.tint(c), c);
        assert_eq!(Hovered.tint(c), c.lerp_to_gamma(theme::TEXT, 0.10));
        assert_eq!(Pressed.tint(c), c.lerp_to_gamma(Color32::BLACK, 0.20));
        // Visible but subtle: the largest channel step is 4..=60.
        for s in [Hovered, Pressed] {
            let d = s
                .tint(c)
                .to_array()
                .iter()
                .zip(c.to_array())
                .map(|(a, b)| a.abs_diff(b))
                .max()
                .unwrap();
            assert!((4..=60).contains(&d), "{s:?} on {c:?}: step {d}");
        }
    }
    assert_eq!(sl::HOVER_TOWARD, theme::TEXT);
    assert_eq!(sl::PRESSED_TOWARD, Color32::BLACK);
}

fn buttons(ui: &mut Ui) {
    ui.horizontal(|ui| {
        widgets::button(ui, ButtonKind::Primary, "Play", true, None);
        widgets::button(ui, ButtonKind::Ghost, "Cancel", true, None);
        widgets::button(ui, ButtonKind::Primary, "OK", false, None);
    });
}

/// (fill colour, border colour if any) of the button named `label`.
fn look(h: &Harness<'_>, label: &str) -> (Rect, Color32, Option<Color32>) {
    let rect = h.get_by_role_and_label(Role::Button, label).rect();
    let flat = common::shapes_flat(&h.output().shapes);
    let (fills, strokes) = rects_at(&flat, rect);
    assert_eq!(fills.len(), 1, "{label}: one fill");
    assert!(strokes.len() <= 1, "{label}");
    (rect, fills[0].fill, strokes.first().map(|s| s.stroke.color))
}

#[test]
fn g21_dialog_buttons() {
    let mut h = Harness::builder()
        .with_size(vec2(600.0, 200.0))
        .with_context_setup(ocb_app::setup)
        .build_ui(buttons);
    h.run();
    use theme::components::{BUTTON_DISABLED_BORDER, BUTTON_DISABLED_FILL};
    let cases = [
        ("Play", theme::AMBER, None, true),
        ("Cancel", theme::WELL, Some(theme::RIM), true),
        (
            "OK",
            BUTTON_DISABLED_FILL,
            Some(BUTTON_DISABLED_BORDER),
            false,
        ),
    ];
    for (label, fill, border, enabled) in cases {
        let (rect, f0, b0) = look(&h, label);
        assert_eq!((f0, b0), (fill, border), "{label} idle");
        let expect = |s: ButtonState| {
            let s = if enabled { s } else { Idle };
            (s.tint(fill), border.map(|b| s.tint(b)))
        };
        h.input_mut()
            .events
            .push(Event::PointerMoved(rect.center()));
        h.run();
        let (r1, f1, b1) = look(&h, label);
        assert_eq!((f1, b1), expect(Hovered), "{label} hover");
        assert_eq!(r1, rect, "{label}: hover moves nothing");
        press(&mut h, rect.center(), true);
        let (r2, f2, b2) = look(&h, label);
        assert_eq!((f2, b2), expect(Pressed), "{label} pressed");
        assert_eq!(r2, rect, "{label}: press moves nothing");
        press(&mut h, rect.center(), false);
        h.input_mut().events.push(Event::PointerGone);
        h.run();
        assert_eq!(look(&h, label).1, fill, "{label}: back to idle");
        if enabled {
            assert_ne!(f1, fill, "{label}: hover visible");
            assert_ne!(f2, fill, "{label}: pressed visible");
        }
    }
}

fn screen() -> Harness<'static> {
    let mut h = Harness::builder()
        .with_size(vec2(1280.0, 800.0))
        .with_context_setup(ocb_app::setup)
        .build_ui(|ui| {
            let l = leds::evaluate(None, 0, WorkerHealth::Running);
            let _ = main_screen::show(ui, &l, &Gate::default());
        });
    h.run();
    h
}

/// STOP's fill and border as drawn (the 75 % applied).
fn stop_look(h: &Harness<'_>) -> (Rect, Color32, Color32) {
    let rect = h.get_by_role_and_label(Role::Button, "Stop").rect();
    let (fills, strokes) = rects_at(&common::shapes_flat(&h.output().shapes), rect);
    assert_eq!(fills.len(), 1);
    assert_eq!(strokes.len(), 1);
    (rect, fills[0].fill, strokes[0].stroke.color)
}

#[test]
fn g21_stop_states() {
    let o = theme::components::STOP_IDLE_OPACITY;
    let want = |s: ButtonState| {
        (
            s.tint(theme::STOP_IDLE_FILL).gamma_multiply(o),
            s.tint(theme::EMBER).gamma_multiply(o),
        )
    };
    let mut h = screen();
    let (rect, f, b) = stop_look(&h);
    assert_eq!((f, b), want(Idle));
    h.input_mut()
        .events
        .push(Event::PointerMoved(rect.center()));
    h.run();
    let (r, f, b) = stop_look(&h);
    assert_eq!((f, b), want(Hovered), "hover");
    assert_eq!(r, rect);
    press(&mut h, rect.center(), true);
    let (r, f, b) = stop_look(&h);
    assert_eq!((f, b), want(Pressed), "pressed");
    assert_eq!(r, rect);
    press(&mut h, rect.center(), false);
    // The focus ring is unchanged under the pointer: 2 pt amber, alpha 255.
    let mut h = screen();
    for _ in 0..4 {
        h.key_press(Key::Tab);
        h.run();
    }
    h.input_mut()
        .events
        .push(Event::PointerMoved(rect.center()));
    h.run();
    let rings = common::shapes_flat(&h.output().shapes)
        .into_iter()
        .filter(|f| {
            matches!(&f.shape, Shape::Rect(r)
            if r.stroke.color == theme::AMBER
                && (r.stroke.width - 2.0).abs() < 1e-4
                && common::rect_eq(r.rect, rect.expand(2.0), 0.5))
        })
        .count();
    assert_eq!(rings, 1, "focus ring while hovered");
    assert_eq!(stop_look(&h).1, want(Hovered).0);
}

#[test]
fn g21_gallery_samples() {
    use ocb_app::app::{App, Mode};
    let mut h = Harness::builder()
        .with_size(vec2(1024.0, 7000.0))
        .build_eframe(|cc| App::with_link(cc, Mode::Gallery, common::fake_link(&cc.egui_ctx)));
    h.run();
    let flat = common::shapes_flat(&h.output().shapes);
    let fills = |c: Color32| {
        flat.iter()
            .filter(|f| matches!(&f.shape, Shape::Rect(r) if r.fill == c))
            .count()
    };
    let o = theme::components::STOP_IDLE_OPACITY;
    for s in [Idle, Hovered, Pressed] {
        assert_eq!(
            fills(s.tint(theme::STOP_IDLE_FILL).gamma_multiply(o)),
            1,
            "STOP {s:?}"
        );
        assert!(fills(s.tint(theme::AMBER)) >= 1, "primary {s:?}");
        assert!(fills(s.tint(theme::WELL)) >= 1, "ghost {s:?}");
    }
    // The disabled row: three samples, all the idle neutral fill, no tint.
    use theme::components::BUTTON_DISABLED_FILL;
    assert!(fills(Hovered.tint(BUTTON_DISABLED_FILL)) == 0);
    assert!(fills(Pressed.tint(BUTTON_DISABLED_FILL)) == 0);
    // The samples add no button node (painted only).
    assert!(h.query_by_label("Sample").is_none());
}
