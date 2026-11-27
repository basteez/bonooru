//! G.15 (Story 4.5 AC 1–3, 11): the status strip drawn from hand-built
//! LEDs in a kittest host (no worker): layout and gaps from the AccessKit
//! bounds and the painted shapes, the four symbols, the tooltip on hover
//! and on keyboard focus, the tab order and the `Role::Status` nodes.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use egui::accesskit::Role;
use egui::epaint::Shape;
use egui::{Color32, Key, Pos2, Rect, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use ocb_app::leds::{Led, LedState, Leds, Reason};
use ocb_app::ui::status_strip::{self, BRAND_NAME, led_id};
use ocb_app::ui::{paint, theme};

const STRIP_H: f32 = 44.0;

fn led(state: LedState, label: &'static str, detail: Option<&str>, tooltip: &[&str]) -> Led {
    Led {
        state,
        label,
        detail: detail.map(str::to_owned),
        tooltip: tooltip.iter().map(|s| (*s).to_owned()).collect(),
        reason: Reason::Info,
        status_age_us: None,
        session_age_us: None,
    }
}

/// All three ok, with versions.
fn all_ok() -> Leds {
    Leds {
        pc_bridge: led(
            LedState::Ok,
            "Bridge",
            Some("v0.3.0"),
            &["Bridge v0.3.0 on /dev/cu.usbmodem2101."],
        ),
        bridge_controller: led(
            LedState::Ok,
            "Controller",
            Some("v0.3.0"),
            &["Controller v0.3.0, talking to the bridge."],
        ),
        switch: led(
            LedState::Ok,
            "Switch",
            Some("– ready"),
            &["The Switch is reading the controller."],
        ),
        next_change_in_us: None,
    }
}

/// All three in `state` (unknown: no detail).
fn all_in(state: LedState) -> Leds {
    let detail = |d: &'static str| (state != LedState::Unknown).then_some(d);
    Leds {
        pc_bridge: led(
            state,
            "Bridge",
            detail("– not found"),
            &["No bridge found.", "Plug the bridge into your PC."],
        ),
        bridge_controller: led(
            state,
            "Controller",
            detail("– not answering"),
            &[
                "No answer from the controller.",
                "Check the three wires between the boards and that the controller is plugged into the Switch.",
            ],
        ),
        switch: led(
            state,
            "Switch",
            detail("– not ready"),
            &[
                "Switch – not ready",
                "Turn on Pro Controller Wired Communication, wake the Switch, check the cable.",
            ],
        ),
        next_change_in_us: None,
    }
}

/// The strip rect of a host window.
fn strip(h: &Harness<'_, Leds>) -> Rect {
    let c = h.ctx.content_rect();
    Rect::from_min_size(c.min, vec2(c.width(), STRIP_H))
}

fn host(w: f32, hgt: f32, ppp: f32, leds: Leds) -> Harness<'static, Leds> {
    let mut h = Harness::builder()
        .with_size(vec2(w, hgt))
        .with_pixels_per_point(ppp)
        .with_context_setup(ocb_app::setup)
        .build_ui_state(
            |ui, leds: &mut Leds| {
                let c = ui.ctx().content_rect();
                let rect = Rect::from_min_size(c.min, vec2(c.width(), STRIP_H));
                status_strip::show(ui, rect, leds);
            },
            leds,
        );
    h.run();
    h
}

fn names(leds: &Leds) -> [String; 3] {
    leds.all().map(|(_, l)| l.name())
}

fn led_rects(h: &Harness<'_, Leds>) -> [Rect; 3] {
    let n = names(h.state());
    n.map(|name| h.get_by_role_and_label(Role::Status, &name).rect())
}

/// Every text shape: (text, position, first section colour).
fn text_shapes(h: &Harness<'_, Leds>) -> Vec<(String, Pos2, Color32)> {
    common::shapes_flat(&h.output().shapes)
        .into_iter()
        .filter_map(|f| match f.shape {
            Shape::Text(t) => Some((
                t.galley.text().to_owned(),
                t.pos,
                t.galley
                    .job
                    .sections
                    .first()
                    .map_or(Color32::PLACEHOLDER, |s| s.format.color),
            )),
            _ => None,
        })
        .collect()
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.5
}

#[test]
fn g15_layout() {
    for (w, hgt) in [(1024.0, 680.0), (1280.0, 800.0)] {
        for ppp in [1.0, 2.0] {
            let ctx = format!("{w}x{hgt} ppp {ppp}");
            let h = host(w, hgt, ppp, all_ok());
            let s = strip(&h);
            let brand = h.get_by_role_and_label(Role::Label, BRAND_NAME).rect();
            assert!(
                near(brand.left(), s.left() + 16.0),
                "{ctx}: brand at {brand:?}"
            );
            let r = led_rects(&h);
            assert!(near(r[0].left() - brand.right(), 28.0), "{ctx}: brand gap");
            for k in 0..2 {
                assert!(
                    near(r[k + 1].left() - r[k].right(), 20.0),
                    "{ctx}: gap {k}: {:?} {:?}",
                    r[k],
                    r[k + 1]
                );
            }
            let texts = text_shapes(&h);
            for (rect, label) in r.iter().zip(["Bridge", "Controller", "Switch"]) {
                assert!(
                    common::inside(*rect, s, 0.01),
                    "{ctx}: {label} inside the strip"
                );
                assert!(
                    near(rect.center().y, s.center().y),
                    "{ctx}: {label} centred"
                );
                // Symbol box: 4 pt padding, 11 pt wide; then 7 pt to the label.
                let symbol_right = rect.left() + 4.0 + 11.0;
                let (_, pos, color) = texts
                    .iter()
                    .find(|(t, _, _)| t == label)
                    .unwrap_or_else(|| panic!("{ctx}: no text {label}"));
                assert!(near(pos.x - symbol_right, 7.0), "{ctx}: {label} symbol gap");
                assert_eq!(*color, theme::TEXT, "{ctx}: label colour");
            }
            // Details in text-muted, after a space.
            for d in ["v0.3.0", "– ready"] {
                let (_, _, color) = texts.iter().find(|(t, _, _)| t == d).unwrap();
                assert_eq!(*color, theme::TEXT_MUTED, "{ctx}: {d}");
            }
            let (_, _, brand_color) = texts.iter().find(|(t, _, _)| t == BRAND_NAME).unwrap();
            assert_eq!(*brand_color, theme::PARCHMENT, "{ctx}: brand colour");
        }
    }
}

#[test]
fn g15_longest_labels_fit() {
    let bridge = [
        "v255.255.255",
        "– other board",
        "– not found",
        "– no access",
        "– in use",
        "– found",
    ];
    let controller = ["v255.255.255", "– not answering", "– no link"];
    let switch = ["– not ready", "– ready"];
    for b in bridge {
        for c in controller {
            for sw in switch {
                let leds = Leds {
                    pc_bridge: led(LedState::Warning, "Bridge", Some(b), &["x"]),
                    bridge_controller: led(LedState::Error, "Controller", Some(c), &["x"]),
                    switch: led(LedState::Warning, "Switch", Some(sw), &["x"]),
                    next_change_in_us: None,
                };
                let h = host(1024.0, 680.0, 1.0, leds);
                let s = strip(&h);
                let r = led_rects(&h);
                for (k, rect) in r.iter().enumerate() {
                    assert!(
                        common::inside(*rect, s, 0.01),
                        "{b}/{c}/{sw}: LED {k} {rect:?}"
                    );
                }
                assert!(r[0].right() < r[1].left() && r[1].right() < r[2].left());
            }
        }
    }
}

#[test]
fn g15_no_gear_no_setup_link() {
    let h = host(1280.0, 800.0, 1.0, all_ok());
    assert!(h.query_by_label("Settings").is_none());
    assert!(h.query_by_label_contains("Set up").is_none());
    assert!(h.query_by_label_contains("Reflash").is_none());
    for (t, _, _) in text_shapes(&h) {
        assert!(
            !t.contains("Set up") && !t.contains("Settings") && !t.contains("Reflash"),
            "{t}"
        );
    }
    // The strip has exactly the brand and the three LEDs.
    let s = strip(&h);
    let in_strip = h
        .query_all(egui_kittest::kittest::by())
        .filter(|n| n.accesskit_node().role() != Role::Window)
        .filter(|n| {
            matches!(
                n.accesskit_node().role(),
                Role::Label | Role::Status | Role::Button | Role::Link | Role::Image
            )
        })
        .filter(|n| common::inside(n.rect(), s, 0.5))
        .count();
    assert_eq!(in_strip, 4);
}

#[test]
fn g15_symbols() {
    let feather = f32::from(paint::feather_from_css_blur(theme::glow::STATUS_LED));
    for (state, color) in [
        (LedState::Ok, theme::STATUS_OK),
        (LedState::Warning, theme::STATUS_WARNING),
        (LedState::Error, theme::STATUS_ERROR),
        (LedState::Unknown, theme::STATUS_UNKNOWN),
    ] {
        let h = host(1280.0, 800.0, 1.0, all_in(state));
        let flat = common::shapes_flat(&h.output().shapes);
        for rect in led_rects(&h) {
            let bx = Rect::from_min_size(
                pos2(rect.left() + 4.0, rect.center().y - 5.5),
                vec2(11.0, 11.0),
            );
            let mine: Vec<&Shape> = flat
                .iter()
                .map(|f| &f.shape)
                .filter(|s| common::uses(s, color))
                .filter(|s| s.visual_bounding_rect().intersects(bx))
                .collect();
            for s in &mine {
                if !matches!(s, Shape::Rect(_)) {
                    assert!(
                        common::inside(s.visual_bounding_rect(), bx, 0.5),
                        "{state:?}: {s:?} outside the 11 × 11 box"
                    );
                }
            }
            match state {
                LedState::Ok => {
                    let glows: Vec<_> = mine
                        .iter()
                        .filter_map(|s| match s {
                            Shape::Rect(r) => Some(r.blur_width),
                            _ => None,
                        })
                        .collect();
                    assert_eq!(glows, vec![feather], "one glow, blur 8 css");
                    let dots: Vec<_> = mine
                        .iter()
                        .filter_map(|s| match s {
                            Shape::Circle(c) if c.fill == color => Some(c),
                            _ => None,
                        })
                        .collect();
                    assert_eq!(dots.len(), 1);
                    assert!(common::rect_eq(dots[0].visual_bounding_rect(), bx, 0.5));
                }
                LedState::Warning => {
                    let paths: Vec<_> = mine
                        .iter()
                        .filter_map(|s| match s {
                            Shape::Path(p) if p.fill == color => Some(p),
                            _ => None,
                        })
                        .collect();
                    assert_eq!(paths.len(), 1);
                    assert!(paint::is_convex(&paths[0].points).is_ok());
                }
                LedState::Error => {
                    let segs = mine
                        .iter()
                        .filter(|s| matches!(s, Shape::LineSegment { .. }))
                        .count();
                    assert_eq!(segs, 2, "two strokes");
                }
                LedState::Unknown => {
                    assert_eq!(mine.len(), 1);
                    match mine[0] {
                        Shape::Circle(c) => {
                            assert_eq!(c.fill, Color32::TRANSPARENT);
                            assert_eq!(c.stroke.color, color);
                        }
                        other => panic!("{other:?}"),
                    }
                }
            }
        }
        // Unknown: label only, no detail text.
        let texts: Vec<String> = text_shapes(&h).into_iter().map(|t| t.0).collect();
        let details = texts.iter().filter(|t| t.starts_with('–')).count();
        assert_eq!(
            details,
            if state == LedState::Unknown { 0 } else { 3 },
            "{state:?}: {texts:?}"
        );
    }
}

/// The focus ring of `rect` (2 pt amber, outside `rect.expand(2)`).
fn has_ring(h: &Harness<'_, Leds>, rect: Rect) -> bool {
    common::shapes_flat(&h.output().shapes)
        .iter()
        .any(|f| match &f.shape {
            Shape::Rect(r) => {
                r.stroke.color == theme::AMBER && common::rect_eq(r.rect, rect.expand(2.0), 0.01)
            }
            _ => false,
        })
}

#[test]
fn g15_tooltip_on_focus() {
    let leds = all_ok();
    let detail = leds.pc_bridge.tooltip[0].clone();
    let mut h = host(1280.0, 800.0, 1.0, leds);
    assert!(
        h.query_by_label(&detail).is_none(),
        "no tooltip before focus"
    );
    h.key_press(Key::Tab);
    h.run();
    assert_eq!(h.ctx.memory(|m| m.focused()), Some(led_id("pc_bridge")));
    let rect = led_rects(&h)[0];
    assert!(has_ring(&h, rect), "focus ring");
    let _ = h.get_by_label(&detail);
    // Hovered and focused: one tooltip.
    h.ctx.all_styles_mut(|s| s.interaction.tooltip_delay = 0.0);
    h.hover_at(rect.center());
    h.run();
    h.run();
    assert_eq!(h.query_all_by_label(&detail).count(), 1);
    // A fix line sits on its own line under the detail.
    let mut h = host(1280.0, 800.0, 1.0, all_in(LedState::Error));
    h.key_press(Key::Tab);
    h.run();
    let d = h.get_by_label("No bridge found.").rect();
    let f = h.get_by_label("Plug the bridge into your PC.").rect();
    assert!(f.top() >= d.bottom() - 0.5, "{d:?} {f:?}");
}

#[test]
fn g15_tooltip_on_hover() {
    let leds = all_ok();
    let detail = leds.bridge_controller.tooltip[0].clone();
    let mut h = host(1280.0, 800.0, 1.0, leds);
    h.ctx.all_styles_mut(|s| s.interaction.tooltip_delay = 0.0);
    let rect = led_rects(&h)[1];
    h.hover_at(rect.center());
    h.run();
    h.run();
    assert_eq!(h.query_all_by_label(&detail).count(), 1);
    assert_eq!(h.ctx.memory(|m| m.focused()), None, "hover does not focus");
    // The tooltip frame: panel-raised, 1 pt rim-soft, rounded.md.
    let frames = common::shapes_flat(&h.output().shapes)
        .into_iter()
        .filter(|f| match &f.shape {
            Shape::Rect(r) => {
                r.fill == theme::PANEL_RAISED
                    && r.stroke.color == theme::RIM_SOFT
                    && (r.stroke.width - 1.0).abs() < 1e-4
                    && r.corner_radius == theme::corner(theme::rounded::MD)
            }
            _ => false,
        })
        .count();
    assert_eq!(frames, 1, "one themed tooltip frame");
}

#[test]
fn g15_tab_order() {
    let mut h = host(1280.0, 800.0, 1.0, all_ok());
    for tag in ["pc_bridge", "bridge_controller", "switch"] {
        h.key_press(Key::Tab);
        h.run();
        assert_eq!(h.ctx.memory(|m| m.focused()), Some(led_id(tag)), "{tag}");
    }
    // Enter on a focused LED does nothing (no click, focus kept).
    h.key_press(Key::Enter);
    h.run();
    assert_eq!(h.ctx.memory(|m| m.focused()), Some(led_id("switch")));
}

#[test]
fn g15_accesskit() {
    for leds in [
        all_ok(),
        all_in(LedState::Warning),
        all_in(LedState::Error),
        all_in(LedState::Unknown),
    ] {
        let h = host(1280.0, 800.0, 1.0, leds.clone());
        assert_eq!(h.query_all_by_role(Role::Status).count(), 3);
        for (_, l) in leds.all() {
            let name = l.name();
            let node = h.get_by_role_and_label(Role::Status, &name);
            assert_eq!(
                node.accesskit_node().description(),
                Some(l.tooltip_text()),
                "{}",
                l.name()
            );
        }
        let _ = h.get_by_role_and_label(Role::Label, "Bonooru");
    }
    // Unknown names are the labels alone.
    let h = host(1280.0, 800.0, 1.0, all_in(LedState::Unknown));
    for name in ["Bridge", "Controller", "Switch"] {
        let _ = h.get_by_role_and_label(Role::Status, name);
    }
}
