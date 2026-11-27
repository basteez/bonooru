//! G.10: accessibility at the "basic" level (Story 4.2 decision, Story 4.3
//! AC 12). Every helper that draws an interactive or informative element is
//! one node with the closest `egui::Role` and a name; decorative painting
//! adds no node; the harness accessibility check stays on in every test.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use egui::{Color32, Rect, Role, Stroke, Ui, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use ocb_app::ui::gallery::readiness_dialog;
use ocb_app::ui::modal::Modals;
use ocb_app::ui::symbols::{self, Symbol};
use ocb_app::ui::widgets::{self, ButtonKind};
use ocb_app::ui::{paint, theme};

fn harness<'a>(draw: impl FnMut(&mut Ui) + 'a) -> Harness<'a> {
    let mut h = Harness::builder()
        .with_size(vec2(800.0, 600.0))
        .with_context_setup(ocb_app::setup)
        .build_ui(draw);
    h.run();
    h
}

#[test]
fn g10_buttons_are_named_buttons() {
    let h = harness(|ui| {
        widgets::button(ui, ButtonKind::Primary, "Play", true, Some(Symbol::Play));
        widgets::button(ui, ButtonKind::Ghost, "Cancel", true, None);
        widgets::button(ui, ButtonKind::Primary, "OK", false, None);
    });
    for label in ["Play", "Cancel", "OK"] {
        let node = h.get_by_role_and_label(Role::Button, label);
        assert_eq!(
            node.accesskit_node().is_disabled(),
            label == "OK",
            "{label} disabled flag"
        );
    }
}

/// Story 4.7 AC 9: STOP is one enabled `Role::Button` "Stop" with its
/// description; the painted ■ inside it adds no `Image` node.
#[test]
fn g10_stop_is_a_described_button() {
    let rect = Rect::from_min_size(pos2(20.0, 20.0), vec2(190.0, 60.0));
    let h = harness(move |ui| {
        widgets::stop_button(ui, rect);
    });
    let node = h.get_by_role_and_label(Role::Button, ocb_app::stop::STOP_A11Y_NAME);
    assert!(!node.accesskit_node().is_disabled());
    assert_eq!(
        node.accesskit_node().description().as_deref(),
        Some(ocb_app::stop::STOP_A11Y_DESCRIPTION)
    );
    assert_eq!(h.query_all_by_role(Role::Image).count(), 0, "no Image node");
    assert_eq!(h.query_all_by_role(Role::Button).count(), 1);
}

#[test]
fn g10_symbols_are_images_named_in_words() {
    let h = harness(|ui| {
        for s in Symbol::ALL {
            widgets::symbol(ui, s, 14.0, theme::TEXT, s.name());
        }
    });
    for s in Symbol::ALL {
        let node = h.get_by_role_and_label(Role::Image, s.name());
        assert!(!symbols::has_glyph(&node.accesskit_node().label().unwrap()));
    }
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "must be plain words")]
fn g10_symbol_name_with_a_glyph_is_refused() {
    let _ = harness(|ui| {
        widgets::symbol(ui, Symbol::Play, 14.0, theme::TEXT, "Play \u{25b6}");
    });
}

#[test]
fn g10_time_is_a_label() {
    let h = harness(|ui| {
        widgets::time(ui, "0:12 / 0:31", &theme::BODY);
    });
    let _ = h.get_by_role_and_label(Role::Label, "0:12 / 0:31");
}

#[test]
fn g10_dialog_is_named_with_its_title() {
    let mut modals = Modals::default();
    modals.request(readiness_dialog());
    let mut h = Harness::builder()
        .with_size(vec2(1280.0, 800.0))
        .with_context_setup(ocb_app::setup)
        .build_ui_state(
            |ui, m: &mut Modals| {
                let _ = m.show(ui.ctx());
            },
            modals,
        );
    h.run();
    let dialog = h.get_by_role_and_label(Role::Dialog, "Ready to play?");
    // The buttons are inside the dialog node.
    let inside: Vec<String> = dialog
        .children_recursive()
        .filter(|n| n.accesskit_node().role() == Role::Button)
        .filter_map(|n| n.accesskit_node().label())
        .collect();
    assert_eq!(inside.len(), 2, "{inside:?}");
    assert!(inside.contains(&"Play".to_owned()) && inside.contains(&"Cancel".to_owned()));
}

/// The decorative helpers and the painter variants, by name.
const DECORATIVE: [&str; 11] = [
    "focus ring",
    "glow",
    "text halo",
    "dashed rect",
    "convex fill",
    "corner brackets",
    "flourish",
    "checklist bullet",
    "symbol painter",
    "digits painter",
    "stroke floor",
];

fn decorate(ui: &mut Ui, which: Option<usize>) {
    // A focusable widget is always there, so the ring has a response.
    let resp = widgets::button(ui, ButtonKind::Ghost, "Base", true, None);
    let Some(i) = which else {
        return;
    };
    let p = ui.painter().clone();
    let ppp = ui.ctx().pixels_per_point();
    let r = Rect::from_min_size(pos2(100.0, 200.0), vec2(200.0, 100.0));
    let c = Color32::from_rgb(1, 2, 3);
    match DECORATIVE[i] {
        "focus ring" => {
            resp.request_focus();
            paint::paint_focus_ring(ui, &resp, 4.0, false);
        }
        "glow" => paint::paint_glow(&p, r, 8.0, 12, c),
        "text halo" => paint::paint_text_halo(&p, r, 12.0, 16, c),
        "dashed rect" => paint::paint_dashed_rect(&p, r, 6.0, Stroke::new(1.0, c), 5.0, 3.0),
        "convex fill" => {
            let _ = paint::fill_convex(
                &p,
                vec![r.left_top(), r.right_top(), r.center_bottom()],
                c,
                Stroke::NONE,
            );
        }
        "corner brackets" => paint::paint_corner_brackets(&p, r, c, ppp),
        "flourish" => paint::paint_flourish(&p, r.center_top(), c, ppp),
        "checklist bullet" => {
            let _ = paint::fill_convex(&p, paint::diamond_points(r.center(), 9.0), c, Stroke::NONE);
        }
        "symbol painter" => {
            for s in Symbol::ALL {
                symbols::paint_symbol(&p, r, s, c, ppp);
            }
        }
        "digits painter" => paint::paint_digits(&p, r.min, "0:12 / 0:31", &theme::BODY, c),
        "stroke floor" => {
            p.line_segment([r.left_top(), r.right_top()], paint::stroke(0.5, c, ppp));
        }
        other => panic!("unknown helper {other}"),
    }
}

fn node_count<S>(h: &Harness<'_, S>) -> usize {
    h.root().children_recursive().count()
}

#[test]
fn g10_decorative_painting_adds_no_node() {
    let mut h = Harness::builder()
        .with_size(vec2(800.0, 600.0))
        .with_context_setup(ocb_app::setup)
        .build_ui_state(|ui, which: &mut Option<usize>| decorate(ui, *which), None);
    h.run();
    let base = node_count(&h);
    assert!(base >= 2, "root content + the base button: {base}");
    for (i, name) in DECORATIVE.iter().enumerate() {
        *h.state_mut() = Some(i);
        h.run();
        assert_eq!(node_count(&h), base, "`{name}` added a node");
        *h.state_mut() = None;
        h.run();
        assert_eq!(node_count(&h), base, "after `{name}`");
    }
}

/// Counts calls of `with_accessibility_check` whose argument is anything
/// but the literal `true` (`false`, a variable, an expression), comments
/// and string contents ignored ([`common::code_only`]), whitespace removed.
fn check_disabled(source: &str) -> usize {
    let name = format!("{}(", "with_accessibility_check");
    let code: String = common::code_only(source)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    code.match_indices(&name)
        .filter(|(i, _)| !code[i + name.len()..].starts_with("true)"))
        .count()
}

#[test]
fn g10_check_off_scan_self_test() {
    let f = "with_accessibility_check";
    let cases: Vec<(String, usize)> = vec![
        (format!("Harness::builder().{f}(false)"), 1),
        (format!("b\n    .{f}( false )"), 1),
        (format!("let on = false; b.{f}(on)"), 1),
        (format!("b.{f}(!strict)"), 1),
        (format!("// b.{f}(false)"), 0),
        (format!("/* b.{f}(false) */"), 0),
        (format!("let s = \"b.{f}(false)\";"), 0),
        (format!("b.{f}(true)"), 0),
        (format!("b.{f}( true )"), 0),
    ];
    for (src, want) in cases {
        assert_eq!(check_disabled(&src), want, "{src}");
    }
}

#[test]
fn g10_accessibility_check_stays_on() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut files = Vec::new();
    common::rs_files(&dir, &mut files);
    assert!(files.len() >= 10, "scan found {} files", files.len());
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap();
        assert_eq!(
            check_disabled(&text),
            0,
            "{} turns the accessibility check off",
            f.display()
        );
    }
}
