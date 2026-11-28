//! G.19 (Story 4.7 AC 1–3, 9): the bottom bar's geometry (the pure layout
//! and what is drawn), STOP's idle look and focus ring, the safety note,
//! the AccessKit nodes, the copy and the purity of `src/stop.rs`. Checked
//! on the frame shapes and the AccessKit tree at 1024 × 680 and 1280 × 800,
//! ppp 1 and 2.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use egui::epaint::{Shape, TextShape};
use egui::{Color32, Key, Modifiers, Rect, Role, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use ocb_app::app::WorkerHealth;
use ocb_app::gating::Gate;
use ocb_app::leds;
use ocb_app::stop;
use ocb_app::ui::bottom_bar::{self, BarLayout};
use ocb_app::ui::layout::regions;
use ocb_app::ui::{main_screen, theme};

const SIZES: [(f32, f32); 2] = [(1024.0, 680.0), (1280.0, 800.0)];
const PPPS: [f32; 2] = [1.0, 2.0];

/// The main screen alone; the state is the rect it was drawn over.
fn screen(w: f32, h: f32, ppp: f32) -> Harness<'static, Rect> {
    let mut h = Harness::builder()
        .with_size(vec2(w, h))
        .with_pixels_per_point(ppp)
        .with_context_setup(ocb_app::setup)
        .build_ui_state(
            |ui, drawn: &mut Rect| {
                *drawn = ui.max_rect();
                let l = leds::evaluate(None, 0, WorkerHealth::Running);
                let _ = main_screen::show(
                    ui,
                    &l,
                    &Gate::default(),
                    &ocb_app::ui::gamepad::PadView::default(),
                );
            },
            Rect::NOTHING,
        );
    h.run();
    h
}

fn bar(h: &Harness<'_, Rect>) -> Rect {
    regions(*h.state()).bottom
}

fn faded(c: Color32, f: f32) -> Color32 {
    c.gamma_multiply(f)
}

fn close(a: Color32, b: Color32) -> bool {
    a.to_array()
        .iter()
        .zip(b.to_array())
        .all(|(x, y)| x.abs_diff(y) <= 1)
}

/// STOP's drawn frame: the one rect filled `stop-idle-fill` at 75 %.
fn stop_rect(flat: &[common::Flat]) -> egui::epaint::RectShape {
    let v: Vec<_> = flat
        .iter()
        .filter_map(|f| match &f.shape {
            Shape::Rect(r)
                if r.fill == faded(theme::STOP_IDLE_FILL, theme::components::STOP_IDLE_OPACITY) =>
            {
                Some(r.clone())
            }
            _ => None,
        })
        .collect();
    assert_eq!(v.len(), 1, "one STOP frame");
    v[0].clone()
}

fn text<'a>(flat: &'a [common::Flat], s: &str) -> &'a TextShape {
    let v: Vec<_> = flat
        .iter()
        .filter_map(|f| match &f.shape {
            Shape::Text(t) if t.galley.text() == s => Some(t),
            _ => None,
        })
        .collect();
    assert_eq!(v.len(), 1, "one `{s}` text");
    v[0]
}

/// The text colour as drawn: the glyph mesh's vertex colour (a painter's
/// opacity is applied there, epaint `adjust_colors`) times the shape's own
/// opacity factor.
fn drawn_color(t: &TextShape) -> Color32 {
    let colors: Vec<Color32> = t
        .galley
        .rows
        .iter()
        .flat_map(|r| r.row.visuals.mesh.vertices.iter().map(|v| v.color))
        .collect();
    assert!(!colors.is_empty(), "`{}` has glyphs", t.galley.text());
    assert!(
        colors.iter().all(|c| *c == colors[0]),
        "one colour per text"
    );
    colors[0].gamma_multiply(t.opacity_factor)
}

fn positive_overlap(a: Rect, b: Rect) -> bool {
    let i = a.intersect(b);
    i.width() > 0.01 && i.height() > 0.01
}

fn check_layout(l: &BarLayout, b: Rect, stop_w: f32, ctx: &str) {
    let lefts = [l.play_cursor.left(), l.play_start.left(), l.listen.left()];
    for (got, want) in lefts.iter().zip([16.0, 220.0, 424.0]) {
        assert!(
            (got - b.left() - want).abs() <= 0.5,
            "{ctx}: slot left {got} vs {want}"
        );
    }
    for s in [l.play_cursor, l.play_start, l.listen] {
        assert!(
            (s.width() - 190.0).abs() <= 0.5,
            "{ctx}: slot width {}",
            s.width()
        );
    }
    assert!(
        (l.safety.left() - b.left() - 628.0).abs() <= 0.5,
        "{ctx}: safety left"
    );
    assert!(
        (l.stop.right() - (b.right() - 16.0)).abs() <= 0.5,
        "{ctx}: STOP right"
    );
    assert!((l.stop.width() - stop_w).abs() <= 0.5, "{ctx}");
    assert!(
        (l.safety.right() - (l.stop.left() - 14.0)).abs() <= 0.5,
        "{ctx}: safety right"
    );
    let all = [l.play_cursor, l.play_start, l.listen, l.safety, l.stop];
    for (i, a) in all.iter().enumerate() {
        assert!(
            common::inside(*a, b, 0.01),
            "{ctx}: element {i} outside the bar"
        );
        for (j, c) in all.iter().enumerate().skip(i + 1) {
            assert!(!positive_overlap(*a, *c), "{ctx}: {i} overlaps {j}");
        }
    }
    for s in [l.play_cursor, l.play_start, l.listen, l.stop] {
        assert!((s.height() - 60.0).abs() <= 0.5, "{ctx}: height");
        assert!((s.top() - (b.top() + 14.0)).abs() <= 0.5, "{ctx}: top");
    }
}

#[test]
fn g19_layout() {
    for w in [1024.0, 1280.0] {
        let b = Rect::from_min_size(pos2(0.0, 600.0), vec2(w, 88.0));
        check_layout(&bottom_bar::layout(b, 190.0), b, 190.0, &format!("bar {w}"));
        // A wider STOP keeps the right edge.
        check_layout(
            &bottom_bar::layout(b, 230.0),
            b,
            230.0,
            &format!("bar {w} wide"),
        );
    }
}

#[test]
fn g19_drawn_geometry() {
    for (w, hgt) in SIZES {
        for ppp in PPPS {
            let ctx = format!("{w}x{hgt} ppp {ppp}");
            let h = screen(w, hgt, ppp);
            let b = bar(&h);
            let flat = common::shapes_flat(&h.output().shapes);
            let stop = stop_rect(&flat).rect;
            assert!((stop.height() - 60.0).abs() <= 0.5, "{ctx}");
            assert!((stop.top() - (b.top() + 14.0)).abs() <= 0.5, "{ctx}");
            assert!(stop.width() >= 190.0, "{ctx}");
            assert!((stop.right() - (b.right() - 16.0)).abs() <= 0.5, "{ctx}");
            let l = bottom_bar::layout(b, stop.width());
            check_layout(&l, b, stop.width(), &ctx);
            // The label block is centred on the bar.
            let block = common::texts(&flat)
                .into_iter()
                .filter(|(t, _, _)| t == stop::STOP_LABEL || t == stop::STOP_SUB)
                .map(|(_, r, _)| r)
                .reduce(Rect::union)
                .unwrap();
            assert!(
                (block.center().y - b.center().y).abs() <= 1.0,
                "{ctx}: block {block:?}"
            );
            // Nothing drawn in the slots, no node there either.
            for slot in [l.play_cursor, l.play_start, l.listen] {
                for f in &flat {
                    let r = f.shape.visual_bounding_rect();
                    if !r.is_positive()
                        || !common::inside(r, b, 0.01)
                        || common::rect_eq(r, b, 0.01)
                    {
                        continue;
                    }
                    assert!(!positive_overlap(r, slot), "{ctx}: {:?} in a slot", f.shape);
                }
                let nodes = h
                    .query_all(egui_kittest::kittest::by())
                    .filter(|n| n.accesskit_node().role() != Role::Window)
                    .filter(|n| n.accesskit_node().bounding_box().is_some())
                    .filter(|n| positive_overlap(n.rect(), slot))
                    .count();
                assert_eq!(nodes, 0, "{ctx}: a node in a slot");
            }
            // Every drawn element inside the bar region (those that are in
            // it at all: the bar's own shapes).
            for f in &flat {
                let r = f.shape.visual_bounding_rect();
                if r.is_positive() && r.top() >= b.top() - 0.01 {
                    assert!(common::inside(r, b, 0.5), "{ctx}: {r:?} leaves the bar");
                }
            }
        }
    }
}

#[test]
fn g19_stop_look() {
    use theme::components::STOP_IDLE_OPACITY as O;
    for (w, hgt) in SIZES {
        for ppp in PPPS {
            let ctx = format!("{w}x{hgt} ppp {ppp}");
            let h = screen(w, hgt, ppp);
            let flat = common::shapes_flat(&h.output().shapes);
            let fill = stop_rect(&flat);
            let stop = fill.rect;
            assert_eq!(fill.corner_radius, theme::corner(12.0), "{ctx}");
            assert_eq!(fill.blur_width, 0.0, "{ctx}: no blur");
            let borders: Vec<_> = flat
                .iter()
                .filter_map(|f| match &f.shape {
                    Shape::Rect(r)
                        if r.stroke.width > 0.0 && common::rect_eq(r.rect, stop, 0.01) =>
                    {
                        Some(r.clone())
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(borders.len(), 1, "{ctx}: one border");
            let frame = &borders[0];
            assert_eq!(frame.stroke.color, faded(theme::EMBER, O), "{ctx}: border");
            assert!(
                (frame.stroke.width - 1.0_f32.max(1.0 / ppp)).abs() < 1e-4,
                "{ctx}"
            );
            assert_eq!(frame.corner_radius, theme::corner(12.0), "{ctx}");
            assert_eq!(frame.blur_width, 0.0, "{ctx}: no blur");
            // No shadow / glow under STOP: no blurred rect near it.
            assert!(
                !flat.iter().any(|f| matches!(&f.shape, Shape::Rect(r)
                    if r.blur_width > 0.0 && positive_overlap(r.rect, stop.expand(30.0)))),
                "{ctx}: glow under STOP"
            );
            // The ■: a 16 pt box at left + 22, vertically centred.
            let sym: Vec<Rect> = flat
                .iter()
                .filter_map(|f| match &f.shape {
                    Shape::Rect(r) if r.fill == faded(theme::STOP_IDLE_TEXT, O) => Some(r.rect),
                    _ => None,
                })
                .collect();
            assert_eq!(sym.len(), 1, "{ctx}: one symbol");
            let s = sym[0];
            assert!(
                (s.left() - (stop.left() + 22.0)).abs() <= 0.5,
                "{ctx}: {s:?}"
            );
            assert!(
                (s.width() - 16.0).abs() <= 0.5 && (s.height() - 16.0).abs() <= 0.5,
                "{ctx}"
            );
            assert!((s.center().y - stop.center().y).abs() <= 0.5, "{ctx}");
            // "STOP" and "Esc".
            let label = text(&flat, stop::STOP_LABEL);
            let sub = text(&flat, stop::STOP_SUB);
            let lx = label.pos.x + label.galley.rect.left();
            assert!(
                (lx - (s.right() + 12.0)).abs() <= 0.5,
                "{ctx}: label at {lx}"
            );
            let lf = &label.galley.job.sections[0].format;
            assert_eq!(lf.font_id.size, 17.0, "{ctx}");
            assert_eq!(
                lf.font_id.family,
                theme::Family::Body.egui_family(),
                "{ctx}"
            );
            assert_eq!(
                sub.galley.job.sections[0].format.font_id.size, 12.5,
                "{ctx}"
            );
            assert!(
                sub.pos.y >= label.pos.y + label.galley.rect.height() - 0.5,
                "{ctx}: sub under label"
            );
            assert!(
                close(drawn_color(label), faded(theme::STOP_IDLE_TEXT, O)),
                "{ctx}: label colour"
            );
            assert!(
                close(drawn_color(sub), faded(theme::STOP_IDLE_TEXT, O * 0.85)),
                "{ctx}: sub colour {:?} vs {:?}",
                drawn_color(sub),
                faded(theme::STOP_IDLE_TEXT, O * 0.85)
            );
        }
    }
}

fn ring_rects(flat: &[common::Flat]) -> Vec<egui::epaint::RectShape> {
    flat.iter()
        .filter_map(|f| match &f.shape {
            Shape::Rect(r)
                if r.stroke.color == theme::AMBER
                    && (r.stroke.width - 2.0).abs() < 1e-4
                    && r.fill == Color32::TRANSPARENT =>
            {
                Some(r.clone())
            }
            _ => None,
        })
        .collect()
}

#[test]
fn g19_stop_focus_ring() {
    for ppp in PPPS {
        let mut h = screen(1280.0, 800.0, ppp);
        let stop = stop_rect(&common::shapes_flat(&h.output().shapes)).rect;
        let near = |r: &egui::epaint::RectShape| common::rect_eq(r.rect, stop.expand(2.0), 0.5);
        assert!(
            !ring_rects(&common::shapes_flat(&h.output().shapes))
                .iter()
                .any(near),
            "no ring without focus"
        );
        // Story 4.9: Tab now reaches the gamepad panel after the LEDs (and
        // stays there in the app); STOP is one Shift+Tab from nothing.
        h.key_press_modifiers(Modifiers::SHIFT, Key::Tab);
        h.run();
        assert_eq!(h.ctx.memory(|m| m.focused()), Some(bottom_bar::stop_id()));
        let rings: Vec<_> = ring_rects(&common::shapes_flat(&h.output().shapes))
            .into_iter()
            .filter(near)
            .collect();
        assert_eq!(rings.len(), 1, "ppp {ppp}: one ring around STOP");
        assert_eq!(rings[0].stroke.color.a(), 255, "full opacity");
        assert_eq!(rings[0].corner_radius, theme::corner(14.0));
    }
}

#[test]
fn g19_safety_note() {
    use theme::components::safety_note::SECOND_LINE_OPACITY;
    for (w, hgt) in SIZES {
        for ppp in PPPS {
            let ctx = format!("{w}x{hgt} ppp {ppp}");
            let h = screen(w, hgt, ppp);
            let b = bar(&h);
            let flat = common::shapes_flat(&h.output().shapes);
            let stop = stop_rect(&flat).rect;
            let area = bottom_bar::layout(b, stop.width()).safety;
            let right = stop.left() - 14.0 - 6.0;
            for (line, color) in [
                (stop::SAFETY_LINE_1, theme::TEXT_MUTED),
                (
                    stop::SAFETY_LINE_2,
                    theme::TEXT_MUTED.gamma_multiply(SECOND_LINE_OPACITY),
                ),
            ] {
                let t = text(&flat, line);
                let f = &t.galley.job.sections[0].format;
                assert_eq!(f.font_id.size, 13.0, "{ctx}");
                assert_eq!(drawn_color(t), color, "{ctx}: {line}");
                let g = t.galley.rect.translate(t.pos.to_vec2());
                assert!(
                    (g.right() - right).abs() <= 1.0,
                    "{ctx}: {line} right {}",
                    g.right()
                );
                for row in &t.galley.rows {
                    let r = row.rect().translate(t.pos.to_vec2());
                    assert!(
                        (r.right() - right).abs() <= 1.0,
                        "{ctx}: row right {}",
                        r.right()
                    );
                }
                let ink = g.union(t.galley.mesh_bounds.translate(t.pos.to_vec2()));
                assert!(
                    common::inside(ink, area, 0.5),
                    "{ctx}: {line} {ink:?} outside {area:?}"
                );
                assert!(common::inside(ink, b, 0.5), "{ctx}: {line} outside the bar");
                let node = h.get_by_role_and_label(Role::Label, line);
                assert!(
                    !node
                        .accesskit_node()
                        .data()
                        .supports_action(egui::accesskit::Action::Focus),
                    "{ctx}: {line} focusable"
                );
            }
            // Line 1 above line 2, the pair centred on the bar.
            let t1 = text(&flat, stop::SAFETY_LINE_1);
            let t2 = text(&flat, stop::SAFETY_LINE_2);
            let r1 = t1.galley.rect.translate(t1.pos.to_vec2());
            let r2 = t2.galley.rect.translate(t2.pos.to_vec2());
            assert!(r2.top() >= r1.bottom() - 0.5, "{ctx}");
            assert!(
                (r1.union(r2).center().y - b.center().y).abs() <= 1.0,
                "{ctx}"
            );
        }
    }
}

#[test]
fn g19_accesskit() {
    for (w, hgt) in SIZES {
        let h = screen(w, hgt, 1.0);
        let node = h.get_by_role_and_label(Role::Button, stop::STOP_A11Y_NAME);
        assert_eq!(
            node.accesskit_node().description().as_deref(),
            Some(stop::STOP_A11Y_DESCRIPTION)
        );
        let stop = node.rect();
        let images = h
            .query_all_by_role(Role::Image)
            .filter(|n| n.accesskit_node().bounding_box().is_some())
            .filter(|n| positive_overlap(n.rect(), stop))
            .count();
        assert_eq!(images, 0, "the ■ adds no node");
        assert!(
            h.query_by_label(stop::STOP_LABEL).is_none(),
            "no node named STOP"
        );
        assert!(h.query_by_label("■").is_none());
        // Story 4.8: the gamepad's 16 controls are buttons too; STOP is the
        // only other one.
        let pad: Vec<&str> = ocb_app::gamepad::Control::ALL
            .iter()
            .map(|c| c.a11y_name())
            .collect();
        assert_eq!(
            h.query_all_by_role(Role::Button)
                .filter(|n| !pad.contains(&n.accesskit_node().label().unwrap_or_default().as_str()))
                .count(),
            1,
            "STOP is the only button besides the gamepad's"
        );
    }
}

#[test]
fn g19_no_online_wording() {
    assert_eq!(
        leds::banned_word("Stay online"),
        Some("online"),
        "self-test"
    );
    for s in stop::COPY {
        assert_eq!(leds::banned_word(s), None, "{s}");
    }
    assert_eq!(stop::SAFETY_LINE_1, "Don't dock the Switch mid-song.");
    assert_eq!(stop::SAFETY_LINE_2, "Esc stops at any time.");
}

#[test]
fn g19_model_has_no_egui() {
    fn hits(src: &str) -> Vec<String> {
        common::code_only(src)
            .split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .filter(|w| matches!(*w, "egui" | "eframe" | "epaint" | "egui_kittest"))
            .map(str::to_owned)
            .collect()
    }
    assert!(hits("// egui\nlet s = \"egui\";").is_empty());
    assert_eq!(hits("use egui::Key;"), vec!["egui".to_owned()]);
    let src = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/stop.rs"),
    )
    .unwrap();
    assert!(src.contains("pub fn requested_line("), "the right file");
    assert_eq!(hits(&src), Vec::<String>::new());
}
