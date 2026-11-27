//! G.11: the shared painting helpers (Story 4.3 AC 1–9): focus ring, glow,
//! text halo, dashed rounded rects, convex-only fills, the stroke floor,
//! digit cells, painted symbols and corner brackets. Geometry and colours
//! are read from the frame's shapes (`FullOutput::shapes`), CPU only.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::{Flat, inside, rect_eq, shapes_flat, stroke_width, uses};
use egui::epaint::{ClippedShape, CornerRadius, Shape, StrokeKind};
use egui::{Color32, Pos2, Rect, Response, Role, Sense, Stroke, Ui, WidgetInfo, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use ocb_app::ui::paint;
use ocb_app::ui::symbols::{self, Symbol};
use ocb_app::ui::theme;

/// A colour no theme token uses, to pick the helper's own shapes.
const MARK: Color32 = Color32::from_rgb(1, 2, 3);

/// Runs `draw` in a harness at `ppp` and returns the frame's shapes.
fn frame(ppp: f32, mut draw: impl FnMut(&mut Ui)) -> Vec<ClippedShape> {
    let mut h = Harness::builder()
        .with_size(vec2(600.0, 400.0))
        .with_pixels_per_point(ppp)
        .with_context_setup(ocb_app::setup)
        .build_ui(|ui| draw(ui));
    h.run();
    h.output().shapes.clone()
}

/// The bounds of every tessellated `MARK` triangle of the frame, feathering
/// off: what epaint really draws, miter joins included.
fn mesh_bounds(ppp: f32, mut draw: impl FnMut(&mut Ui)) -> Vec<Rect> {
    let mut h = Harness::builder()
        .with_size(vec2(600.0, 400.0))
        .with_pixels_per_point(ppp)
        .with_context_setup(|ctx| {
            ocb_app::setup(ctx);
            ctx.tessellation_options_mut(|o| o.feathering = false);
        })
        .build_ui(|ui| draw(ui));
    h.run();
    let prims = h.ctx.tessellate(h.output().shapes.clone(), ppp);
    let mut out = Vec::new();
    for p in prims {
        if let egui::epaint::Primitive::Mesh(mesh) = p.primitive {
            let pts: Vec<Pos2> = mesh
                .vertices
                .iter()
                .filter(|v| v.color == MARK)
                .map(|v| v.pos)
                .collect();
            if !pts.is_empty() {
                out.push(Rect::from_points(&pts));
            }
        }
    }
    assert!(!out.is_empty(), "nothing tessellated in MARK");
    out
}

fn marked(shapes: &[ClippedShape]) -> Vec<Flat> {
    shapes_flat(shapes)
        .into_iter()
        .filter(|f| uses(&f.shape, MARK))
        .collect()
}

// ---------------------------------------------------------------- AC 1

#[derive(Default)]
struct RingState {
    rect: Option<Rect>,
    on_parchment: bool,
}

fn ring_target(ui: &mut Ui, state: &mut RingState) -> Response {
    ui.add_space(30.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(80.0, 24.0), Sense::click());
    resp.widget_info(|| WidgetInfo::labeled(Role::Button, true, "Target"));
    // Painted with the `Ui` the widget was added to: the widget's own rect
    // never clips the ring.
    paint::paint_focus_ring(ui, &resp, theme::rounded::MD, state.on_parchment);
    state.rect = Some(rect);
    resp
}

fn rings(shapes: &[ClippedShape], color: Color32) -> Vec<(egui::epaint::RectShape, Rect)> {
    shapes_flat(shapes)
        .into_iter()
        .filter_map(|f| match f.shape {
            Shape::Rect(r) if r.stroke.color == color && r.stroke.width == 2.0 => Some((r, f.clip)),
            _ => None,
        })
        .collect()
}

#[test]
fn g11_focus_ring() {
    for ppp in [1.0, 1.25, 1.5, 2.0] {
        for on_parchment in [false, true] {
            let color = if on_parchment {
                theme::AMBER_INK
            } else {
                theme::AMBER
            };
            let mut h = Harness::builder()
                .with_size(vec2(300.0, 200.0))
                .with_pixels_per_point(ppp)
                .with_context_setup(ocb_app::setup)
                .build_ui_state(
                    |ui, s: &mut RingState| {
                        ring_target(ui, s);
                    },
                    RingState {
                        rect: None,
                        on_parchment,
                    },
                );
            h.run();
            assert!(
                rings(&h.output().shapes, color).is_empty(),
                "ppp {ppp}: ring without focus"
            );
            h.get_by_label("Target").focus();
            h.run();
            let target = h.state().rect.unwrap();
            let found = rings(&h.output().shapes, color);
            assert_eq!(
                found.len(),
                1,
                "ppp {ppp} parchment {on_parchment}: {found:?}"
            );
            let (ring, clip) = &found[0];
            assert_eq!(ring.stroke_kind, StrokeKind::Outside);
            assert!(
                rect_eq(ring.rect, target.expand(2.0), 0.01),
                "ppp {ppp}: ring {:?} vs {:?}",
                ring.rect,
                target.expand(2.0)
            );
            assert_eq!(ring.corner_radius, CornerRadius::same(8));
            assert!(
                inside(target.expand(4.0), *clip, 0.0),
                "ppp {ppp}: clip {clip:?} cuts the ring"
            );
        }
    }
}

/// How the ring's `Ui` is set up in [`g11_focus_ring_follows_its_ui`].
#[derive(Clone, Copy, Debug, PartialEq)]
enum RingUi {
    /// A viewport (like a `ScrollArea`'s) that cuts the widget in half.
    Clipped,
    /// Half opacity (like the dialog's fade-in).
    Faded,
    /// An invisible `Ui` (like a sizing pass).
    Invisible,
}

fn ring_in(ui: &mut Ui, mode: &mut (RingUi, Option<(Rect, Rect)>)) {
    ui.add_space(30.0);
    let viewport = Rect::from_min_max(ui.cursor().min, ui.cursor().min + vec2(300.0, 12.0));
    let mut builder = egui::UiBuilder::new();
    if mode.0 == RingUi::Invisible {
        builder = builder.invisible();
    }
    ui.scope_builder(builder, |ui| {
        match mode.0 {
            RingUi::Clipped => ui.set_clip_rect(viewport),
            RingUi::Faded => ui.multiply_opacity(0.5),
            RingUi::Invisible => {}
        }
        let (rect, resp) = ui.allocate_exact_size(vec2(80.0, 24.0), Sense::click());
        resp.widget_info(|| WidgetInfo::labeled(Role::Button, true, "Target"));
        if mode.0 == RingUi::Invisible {
            // An invisible Ui gives no focus; force it to test the painter.
            resp.request_focus();
        }
        paint::paint_focus_ring(ui, &resp, theme::rounded::MD, false);
        mode.1 = Some((rect, viewport));
    });
}

/// AC 1 / D7 (owner-delegated decision 2026-11-27): the ring follows the
/// `Ui` it is painted with: an ancestor clip cuts it, the `Ui`'s opacity
/// fades it, an invisible `Ui` paints none.
#[test]
fn g11_focus_ring_follows_its_ui() {
    for mode in [RingUi::Clipped, RingUi::Faded, RingUi::Invisible] {
        let mut h = Harness::builder()
            .with_size(vec2(400.0, 200.0))
            .with_context_setup(ocb_app::setup)
            .build_ui_state(ring_in, (mode, None));
        h.run();
        if mode != RingUi::Invisible {
            h.get_by_label("Target").focus();
            h.run();
        }
        let (target, viewport) = h.state().1.unwrap();
        let all: Vec<_> = shapes_flat(&h.output().shapes)
            .into_iter()
            .filter_map(|f| match f.shape {
                Shape::Rect(r) if r.stroke.width == 2.0 && r.stroke_kind == StrokeKind::Outside => {
                    Some((r, f.clip))
                }
                _ => None,
            })
            .collect();
        match mode {
            RingUi::Clipped => {
                assert_eq!(all.len(), 1, "{all:?}");
                let want = viewport.intersect(target.expand(2.0).expand(3.0));
                assert!(
                    rect_eq(all[0].1, want, 0.01),
                    "clip {:?} vs {want:?}",
                    all[0].1
                );
            }
            RingUi::Faded => {
                assert_eq!(all.len(), 1, "{all:?}");
                let a = all[0].0.stroke.color.a();
                assert!(a.abs_diff(128) <= 1, "ring alpha {a} at half opacity");
            }
            RingUi::Invisible => assert!(all.is_empty(), "ring in an invisible Ui: {all:?}"),
        }
    }
}

// ---------------------------------------------------------------- AC 2

#[test]
fn g11_glow() {
    let rect = Rect::from_min_size(pos2(100.0, 100.0), vec2(120.0, 34.0));
    let shapes = frame(1.0, |ui| {
        let p = ui.painter();
        paint::paint_glow(p, rect, 8.0, theme::glow::MODAL_PRIMARY, MARK);
        p.rect_filled(rect, theme::corner(8.0), Color32::from_rgb(9, 9, 9));
    });
    let flat = shapes_flat(&shapes);
    let glow = flat
        .iter()
        .find_map(|f| match &f.shape {
            Shape::Rect(r) if r.fill == MARK => Some((f.index, r.clone())),
            _ => None,
        })
        .expect("glow rect");
    // CSS blur 12 → epaint feather round(12 × 1.55) = 19.
    assert_eq!(glow.1.blur_width, 19.0);
    assert!(
        (glow.1.rect.center() - rect.center()).length() < 1e-4,
        "offset 0"
    );
    let control = flat
        .iter()
        .find(|f| matches!(&f.shape, Shape::Rect(r) if r.fill == Color32::from_rgb(9, 9, 9)))
        .expect("control");
    assert!(control.index > glow.0, "glow painted under the control");
}

#[test]
fn g11_glow_never_clamped() {
    for side in [4.0_f32, 11.0, 26.0, 60.0] {
        for blur in [8_u8, 10, 12, 16, 18] {
            let rect = Rect::from_min_size(pos2(200.0, 150.0), vec2(side, side));
            let shapes = frame(1.0, |ui| {
                paint::paint_glow(ui.painter(), rect, side / 2.0, blur, MARK);
            });
            let shadow = marked(&shapes)
                .into_iter()
                .find_map(|f| match f.shape {
                    Shape::Rect(r) => Some(r),
                    _ => None,
                })
                .unwrap();
            let min = shadow.rect.size().min_elem();
            let limit = f32::from(paint::feather_from_css_blur(blur)) + 0.1;
            assert!(min > limit, "side {side} blur {blur}: {min} <= {limit}");
            let spread = (min - side) / 2.0;
            assert!(
                (spread - spread.round()).abs() < 1e-4,
                "whole spread: {spread}"
            );
            assert!(
                spread == 0.0 || side + 2.0 * (spread - 1.0) <= limit,
                "side {side} blur {blur}: spread {spread} is not the smallest"
            );
        }
    }
}

/// The CSS Gaussian edge profile: α · Q(d / σ), σ = blur / 2.
fn css_profile(d: f32, css_blur: f32) -> f32 {
    let sigma = css_blur / 2.0;
    // Q(x) = erfc(x / √2) / 2, with erfc from Abramowitz–Stegun 7.1.26.
    let x = d / sigma / std::f32::consts::SQRT_2;
    let t = 1.0 / (1.0 + 0.327_591_1 * x);
    let poly = t
        * (0.254_829_6
            + t * (-0.284_496_7 + t * (1.421_413_7 + t * (-1.453_152 + t * 1.061_405_4))));
    0.5 * poly * (-x * x).exp()
}

/// The alpha (0–1) of the tessellated mesh at `p`, from the triangles whose
/// vertices are all pure red (the glow's own colour), interpolated.
fn red_alpha_at(prims: &[egui::epaint::ClippedPrimitive], p: Pos2) -> f32 {
    let mut best = 0.0_f32;
    for prim in prims {
        let egui::epaint::Primitive::Mesh(mesh) = &prim.primitive else {
            continue;
        };
        for tri in mesh.indices.as_chunks::<3>().0 {
            let v: Vec<_> = tri.iter().map(|&i| mesh.vertices[i as usize]).collect();
            if v.iter().any(|v| v.color.g() != 0 || v.color.b() != 0) {
                continue;
            }
            let (a, b, c) = (v[0].pos, v[1].pos, v[2].pos);
            let det = (b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y);
            if det.abs() < 1e-9 {
                continue;
            }
            let l1 = ((p.x - a.x) * (c.y - a.y) - (c.x - a.x) * (p.y - a.y)) / det;
            let l2 = ((b.x - a.x) * (p.y - a.y) - (p.x - a.x) * (b.y - a.y)) / det;
            let l0 = 1.0 - l1 - l2;
            if l0 >= -1e-4 && l1 >= -1e-4 && l2 >= -1e-4 {
                let alpha = l0 * f32::from(v[0].color.a())
                    + l1 * f32::from(v[1].color.a())
                    + l2 * f32::from(v[2].color.a());
                best = best.max(alpha / 255.0);
            }
        }
    }
    best
}

/// The tessellated glow follows the CSS `box-shadow` Gaussian of the same
/// token blur (σ = blur / 2) outside the control's edge, within 0.07 α, at
/// d = 0, σ/2, σ, 1.5σ and 2σ (owner eye check 2026-11-27).
#[test]
fn g11_glow_matches_the_css_gaussian() {
    for css_blur in [10_u8, 12, 16, 18] {
        let rect = Rect::from_min_size(pos2(100.0, 100.0), vec2(300.0, 80.0));
        let mut h = Harness::builder()
            .with_size(vec2(600.0, 400.0))
            .with_context_setup(ocb_app::setup)
            .build_ui(|ui| {
                paint::paint_glow(ui.painter(), rect, 0.0, css_blur, Color32::RED);
            });
        h.run();
        let prims = h.ctx.tessellate(h.output().shapes.clone(), 1.0);
        let sigma = f32::from(css_blur) / 2.0;
        for k in [0.0_f32, 0.5, 1.0, 1.5, 2.0] {
            let d = k * sigma;
            let got = red_alpha_at(&prims, pos2(rect.right() + d, rect.center().y));
            let want = css_profile(d, f32::from(css_blur));
            assert!(
                (got - want).abs() <= 0.07,
                "CSS blur {css_blur}: at d = {k}σ glow alpha {got:.3}, CSS {want:.3}"
            );
        }
    }
}

// ---------------------------------------------------------------- AC 3

#[test]
fn g11_text_halo() {
    let pad = 12.0;
    let mut ink = Rect::NOTHING;
    let shapes = frame(1.0, |ui| {
        let galley = paint::layout_line(ui.ctx(), &theme::COUNTDOWN, "3", theme::TEXT);
        let pos = pos2(150.0, 60.0);
        ink = paint::ink_rect(&galley, pos);
        let p = ui.painter();
        paint::paint_text_halo(p, ink, pad, theme::glow::PLAY_PRIMARY, MARK);
        p.galley(pos, galley, theme::TEXT);
    });
    let flat = shapes_flat(&shapes);
    let (halo_index, halo) = flat
        .iter()
        .find_map(|f| match &f.shape {
            Shape::Rect(r) if r.fill == MARK && r.blur_width > 0.0 => Some((f.index, r.rect)),
            _ => None,
        })
        .expect("halo");
    assert!(
        inside(ink.expand(pad), halo, 1e-3),
        "{halo:?} ⊉ {ink:?}+{pad}"
    );
    // The halo covers the glyph's ink (Cormorant's "3" draws below its
    // layout rect) and is painted before it.
    let (glyph_ink, text_index) = flat
        .iter()
        .find_map(|f| match &f.shape {
            Shape::Text(t) if t.galley.text() == "3" => {
                Some((t.galley.mesh_bounds.translate(t.pos.to_vec2()), f.index))
            }
            _ => None,
        })
        .unwrap();
    assert!(inside(glyph_ink.expand(pad), halo, 1e-3));
    assert!(halo_index < text_index, "halo behind the text");
}

/// The halo is centred on the glyph's ink: equal padding (± 0.5 pt) on all
/// four sides, for an old-style figure ("7", ink low in its line) and a
/// capital (owner eye check 2026-11-27).
#[test]
fn g11_text_halo_even_padding() {
    for (style, glyph) in [
        (theme::COUNTDOWN, "7"),
        (theme::COUNTDOWN, "H"),
        (theme::CARD_TITLE, "7"),
    ] {
        let pad = 12.0;
        let shapes = frame(1.0, |ui| {
            let galley = paint::layout_line(ui.ctx(), &style, glyph, theme::TEXT);
            let pos = pos2(150.0, 60.0);
            let p = ui.painter();
            paint::paint_text_halo(p, paint::ink_rect(&galley, pos), pad, 16, MARK);
            p.galley(pos, galley, theme::TEXT);
        });
        let flat = shapes_flat(&shapes);
        let halo = flat
            .iter()
            .find_map(|f| match &f.shape {
                Shape::Rect(r) if r.fill == MARK && r.blur_width > 0.0 => Some(r.rect),
                _ => None,
            })
            .expect("halo");
        let ink = flat
            .iter()
            .find_map(|f| match &f.shape {
                Shape::Text(t) if t.galley.text() == glyph => {
                    Some(t.galley.mesh_bounds.translate(t.pos.to_vec2()))
                }
                _ => None,
            })
            .expect("glyph");
        let sides = [
            ink.left() - halo.left(),
            halo.right() - ink.right(),
            ink.top() - halo.top(),
            halo.bottom() - ink.bottom(),
        ];
        let (lo, hi) = sides
            .iter()
            .fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(*v), hi.max(*v)));
        assert!(
            hi - lo <= 0.5,
            "`{glyph}` at {} pt: halo padding left/right/top/bottom {sides:?}",
            style.size
        );
    }
}

// ---------------------------------------------------------------- AC 4

/// Distance from `p` to the outline of a rounded rect.
fn outline_distance(rect: Rect, r: f32, p: Pos2) -> f32 {
    let inner = rect.shrink(r);
    let q = pos2(
        p.x.clamp(inner.left(), inner.right()),
        p.y.clamp(inner.top(), inner.bottom()),
    );
    if inner.contains(p) {
        // Inside the inner rect: distance to the nearest straight edge.
        let d = (p.x - rect.left())
            .min(rect.right() - p.x)
            .min(p.y - rect.top())
            .min(rect.bottom() - p.y);
        return d;
    }
    (p.distance(q) - r).abs()
}

fn dashes(shapes: &[ClippedShape]) -> Vec<(usize, [Pos2; 2], f32)> {
    shapes_flat(shapes)
        .into_iter()
        .filter_map(|f| match f.shape {
            Shape::LineSegment { points, stroke } if stroke.color == MARK => {
                Some((f.index, points, stroke.width))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn g11_dashed_rect() {
    // Sides that are not whole periods, so a phase restart per side shows.
    let rect = Rect::from_min_size(pos2(40.0, 40.0), vec2(125.0, 37.0));
    for radius in [0.0_f32, 4.0, 12.0, 9999.0] {
        for (dash, gap) in [(5.0_f32, 3.0_f32), (4.0, 4.0)] {
            let shapes = frame(1.0, |ui| {
                paint::paint_dashed_rect(
                    ui.painter(),
                    rect,
                    radius,
                    Stroke::new(1.0, MARK),
                    dash,
                    gap,
                );
            });
            let d = dashes(&shapes);
            assert!(!d.is_empty());
            // One dashed_line_many call: one contiguous run of shapes.
            for w in d.windows(2) {
                assert_eq!(w[1].0, w[0].0 + 1, "radius {radius}: run broken");
            }
            dash_phase_runs_on(&d, rect, radius, dash, gap);
            let r = radius.min(rect.size().min_elem() / 2.0);
            let mut total = 0.0;
            for (_, [a, b], _) in &d {
                for p in [a, b] {
                    let dist = outline_distance(rect, r, *p);
                    assert!(dist <= 0.1 + 1e-3, "radius {radius}: {p:?} is {dist} off");
                }
                total += a.distance(*b);
            }
            let perimeter =
                2.0 * (rect.width() + rect.height()) - 8.0 * r + std::f32::consts::TAU * r;
            let want = perimeter * dash / (dash + gap);
            assert!(
                (total - want).abs() <= dash,
                "radius {radius} {dash}/{gap}: dashes {total}, want {want} ± {dash}"
            );
        }
    }
}

/// The dash phase runs on around the corners: merged across the outline's
/// vertices, every dash starts a whole number of periods along the outline
/// and every dash but the last is `dash` long. Per-side dashing restarts the
/// phase at each corner and fails here even when a side is a whole number of
/// periods long.
fn dash_phase_runs_on(d: &[(usize, [Pos2; 2], f32)], rect: Rect, radius: f32, dash: f32, gap: f32) {
    let outline = paint::rounded_rect_points(rect, radius, 0.1);
    let mut cum = vec![0.0_f32];
    for w in outline.windows(2) {
        cum.push(cum.last().unwrap() + w[0].distance(w[1]));
    }
    let along = |p: Pos2| -> f32 {
        for (j, w) in outline.windows(2).enumerate() {
            let (a, b) = (w[0], w[1]);
            let len = a.distance(b);
            let t = if len > 0.0 {
                (p - a).dot(b - a) / (len * len)
            } else {
                0.0
            };
            if (-1e-4..=1.0 + 1e-4).contains(&t) && a.lerp(b, t.clamp(0.0, 1.0)).distance(p) < 1e-3
            {
                return cum[j] + t.clamp(0.0, 1.0) * len;
            }
        }
        panic!("{p:?} is not on the outline");
    };
    // Merge the segments of one dash (split at the outline's vertices).
    let mut merged: Vec<(Pos2, f32)> = Vec::new();
    let mut last_end: Option<Pos2> = None;
    for (_, [a, b], _) in d {
        match (last_end, merged.last_mut()) {
            (Some(e), Some(m)) if e.distance(*a) < 1e-4 => m.1 += a.distance(*b),
            _ => merged.push((*a, a.distance(*b))),
        }
        last_end = Some(*b);
    }
    let period = dash + gap;
    let n = merged.len();
    for (k, (start, len)) in merged.iter().enumerate() {
        let s = along(*start);
        let phase = s - (s / period).round() * period;
        assert!(
            phase.abs() < 0.01,
            "radius {radius} {dash}/{gap}: dash {k} starts at {s}, phase {phase}"
        );
        if k + 1 < n {
            assert!(
                (len - dash).abs() < 0.01,
                "radius {radius}: dash {k} is {len} long"
            );
        }
    }
}

/// Degenerate input never panics or hangs: an inverted, empty or NaN rect
/// has no outline, a chord error ≤ 0 is floored, a dash or gap ≤ 0 draws
/// nothing (epaint's dash loop would never end).
#[test]
fn g11_degenerate_inputs() {
    let inverted = Rect::from_min_max(pos2(50.0, 50.0), pos2(45.0, 40.0));
    let nan = Rect::from_min_size(pos2(f32::NAN, 0.0), vec2(10.0, 10.0));
    for r in [
        inverted,
        Rect::NOTHING,
        nan,
        Rect::from_min_size(pos2(0.0, 0.0), vec2(0.0, 5.0)),
    ] {
        assert!(paint::rounded_rect_points(r, 4.0, 0.1).is_empty(), "{r:?}");
    }
    let ok = Rect::from_min_size(pos2(0.0, 0.0), vec2(40.0, 20.0));
    for err in [0.0_f32, -1.0, f32::NAN] {
        let pts = paint::rounded_rect_points(ok, 8.0, err);
        assert!(
            pts.len() > 8 && pts.len() < 10_000,
            "chord error {err}: {}",
            pts.len()
        );
    }
    assert!(!paint::rounded_rect_points(ok, f32::NAN, 0.1).is_empty());
    let shapes = frame(1.0, |ui| {
        let p = ui.painter();
        let st = Stroke::new(1.0, MARK);
        for (dash, gap) in [
            (0.0, 3.0),
            (5.0, 0.0),
            (-1.0, 3.0),
            (f32::NAN, 3.0),
            (5.0, f32::INFINITY),
            // These two never end in epaint's dash loop.
            (0.0, 0.0),
            (-3.0, 2.0),
        ] {
            paint::paint_dashed_rect(p, ok, 4.0, st, dash, gap);
        }
        paint::paint_dashed_rect(p, inverted, 4.0, st, 5.0, 3.0);
    });
    assert!(marked(&shapes).is_empty(), "degenerate dashes painted");
}

// ---------------------------------------------------------------- AC 5

/// Collinear points are refused wherever they sit on screen (relative
/// thresholds), a vertex that doubles back is refused, and `fill_convex`
/// paints the cleaned list it validated.
#[test]
fn g11_convexity_is_position_independent() {
    let mut accepted = Vec::new();
    for i in 0..400 {
        let x = 500.0 + 1500.0 * (i as f32) / 400.0;
        let y = 300.0 + 0.37 * i as f32;
        let (d1, d2) = (3.7 + (i % 7) as f32, 11.3 + (i % 5) as f32);
        let pts = [
            pos2(x, y),
            pos2(x + d1, y + 0.5 * d1),
            pos2(x + d2, y + 0.5 * d2),
        ];
        if paint::is_convex(&pts).is_ok() {
            accepted.push(pts);
        }
    }
    assert!(
        accepted.is_empty(),
        "{} collinear triples accepted, e.g. {:?}",
        accepted.len(),
        accepted.first()
    );
    let back = [
        pos2(0.0, 0.0),
        pos2(10.0, 0.0),
        pos2(5.0, 0.0),
        pos2(5.0, 8.0),
    ];
    assert!(paint::is_convex(&back).is_err(), "doubling back accepted");
    // A far-away square is still accepted.
    let far: Vec<Pos2> = [(0.0, 0.0), (9.0, 0.0), (9.0, 9.0), (0.0, 9.0)]
        .iter()
        .map(|&(x, y)| pos2(1800.0 + x, 1200.0 + y))
        .collect();
    assert!(paint::is_convex(&far).is_ok());

    let raw = vec![
        pos2(100.0, 100.0),
        pos2(100.0, 100.0),
        pos2(140.0, 100.0),
        pos2(120.0, 130.0),
        pos2(100.0, 100.0),
    ];
    let shapes = frame(1.0, |ui| {
        paint::fill_convex(ui.painter(), raw.clone(), MARK, Stroke::NONE).unwrap();
    });
    let painted = path_points(&marked(&shapes));
    assert_eq!(
        painted,
        vec![vec![
            pos2(100.0, 100.0),
            pos2(140.0, 100.0),
            pos2(120.0, 130.0)
        ]],
        "painted points are not the cleaned list"
    );
}

#[test]
fn g11_fill_convex() {
    let p = |v: &[(f32, f32)]| -> Vec<Pos2> {
        v.iter()
            .map(|&(x, y)| pos2(100.0 + x * 10.0, 100.0 + y * 10.0))
            .collect()
    };
    let good: Vec<(&str, Vec<Pos2>)> = vec![
        ("triangle", p(&[(0.0, 0.0), (4.0, 0.0), (2.0, 3.0)])),
        (
            "square",
            p(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)]),
        ),
        ("diamond", paint::diamond_points(pos2(150.0, 150.0), 9.0)),
        (
            "flourish diamond",
            vec![
                pos2(110.0, 2.5),
                pos2(114.5, 7.0),
                pos2(110.0, 11.5),
                pos2(105.5, 7.0),
            ],
        ),
    ];
    let bad: Vec<(&str, Vec<Pos2>)> = vec![
        (
            "L",
            p(&[
                (0.0, 0.0),
                (4.0, 0.0),
                (4.0, 1.0),
                (1.0, 1.0),
                (1.0, 4.0),
                (0.0, 4.0),
            ]),
        ),
        (
            "bow-tie",
            p(&[(0.0, 0.0), (4.0, 4.0), (4.0, 0.0), (0.0, 4.0)]),
        ),
        ("2 points", p(&[(0.0, 0.0), (4.0, 0.0)])),
        ("collinear", p(&[(0.0, 0.0), (1.0, 1.0), (2.0, 2.0)])),
    ];
    for (name, pts, want_ok) in good
        .into_iter()
        .map(|(n, v)| (n, v, true))
        .chain(bad.into_iter().map(|(n, v)| (n, v, false)))
    {
        let mut result = None;
        let shapes = frame(1.0, |ui| {
            result = Some(paint::fill_convex(
                ui.painter(),
                pts.clone(),
                MARK,
                Stroke::NONE,
            ));
        });
        let added = marked(&shapes).len();
        assert_eq!(result.unwrap().is_ok(), want_ok, "{name}");
        assert_eq!(added, usize::from(want_ok), "{name}: shapes added");
    }
}

/// The `{ … }` body after `start` in `s` (brace-matched).
fn brace_body(s: &str, start: usize) -> &str {
    let body = &s[start..];
    let mut depth = 1;
    for (j, c) in body.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &body[..j];
                }
            }
            _ => {}
        }
    }
    body
}

/// The top-level comma-separated arguments of the call whose `(` ends
/// right before `start`.
fn call_args(s: &str, start: usize) -> Vec<&str> {
    let body = &s[start..];
    let mut depth = 1;
    let mut args = Vec::new();
    let mut from = 0;
    for (j, c) in body.char_indices() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    args.push(&body[from..j]);
                    return args;
                }
            }
            ',' if depth == 1 => {
                args.push(&body[from..j]);
                from = j + 1;
            }
            _ => {}
        }
    }
    args
}

/// Counts filled-shape constructions in Rust source, comments and string
/// contents ignored ([`common::code_only`]), whitespace removed:
/// `convex_polygon(` anywhere; a `PathShape {…}`, `QuadraticBezierShape {…}`
/// or `CubicBezierShape {…}` literal that sets `fill`; a `.fill =` field
/// assignment; a Bézier `from_points_stroke(points, closed, fill, stroke)`
/// whose fill is not `Color32::TRANSPARENT`; and an alias of a path or
/// Bézier shape type (`… as X`), which would hide the others.
fn filled_paths(source: &str) -> usize {
    let s: String = common::code_only(source)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let mut n = s.matches("convex_polygon(").count();
    for ty in ["PathShape{", "QuadraticBezierShape{", "CubicBezierShape{"] {
        let mut from = 0;
        while let Some(k) = s[from..].find(ty) {
            let start = from + k + ty.len();
            if brace_body(&s, start).contains("fill") {
                n += 1;
            }
            from = start;
        }
    }
    n += s.matches(".fill=").count() - s.matches(".fill==").count();
    let mut from = 0;
    while let Some(k) = s[from..].find("from_points_stroke(") {
        let start = from + k + "from_points_stroke(".len();
        let args = call_args(&s, start);
        let fill = args.get(2).copied().unwrap_or("");
        if !fill.ends_with("Color32::TRANSPARENT") {
            n += 1;
        }
        from = start;
    }
    for ty in ["PathShapeas", "BezierShapeas", "::Shapeas"] {
        n += s.matches(ty).count();
    }
    n
}

#[test]
fn g11_filled_path_scan_self_test() {
    let cases: &[(&str, usize)] = &[
        ("Shape::convex_polygon(pts, c, s)", 1),
        ("PathShape::convex_polygon(pts, c, s)", 1),
        ("PathShape { points, closed: true, fill: c, stroke }", 1),
        (
            "PathShape {\n  points,\n  closed: true,\n  fill,\n  stroke: s.into(),\n}",
            1,
        ),
        ("let mut s = PathShape::closed_line(p, st); s.fill = c;", 1),
        (
            "let mut s = PathShape::closed_line(p, st);\ns\n  .fill\n  = c;",
            1,
        ),
        (
            "QuadraticBezierShape::from_points_stroke([a, b, c], true, Color32::RED, st)",
            1,
        ),
        (
            "CubicBezierShape::from_points_stroke(pts, false, fill, st)",
            1,
        ),
        (
            "QuadraticBezierShape { points, closed: true, fill: c, stroke }",
            1,
        ),
        ("use egui::epaint::PathShape as P;", 1),
        ("use egui::epaint::QuadraticBezierShape as Q;", 1),
        ("use egui::Shape as S;", 1),
        (
            "QuadraticBezierShape::from_points_stroke([a, b, c], false, Color32::TRANSPARENT, st)",
            0,
        ),
        (
            "QuadraticBezierShape::from_points_stroke(leaf.map(at), false, egui::Color32::TRANSPARENT, stroke(w, c, ppp))",
            0,
        ),
        ("if r.fill == theme::AMBER {}", 0),
        ("PathShape::line(points, stroke)", 0),
        ("// never convex_polygon( here", 0),
        ("/* PathShape { fill } */ x", 0),
        ("let url = \"http://x\"; let p = convex_polygon(a);", 1),
        ("let s = \"convex_polygon(\";", 0),
        ("painter.rect_filled(r, 0.0, c)", 0),
    ];
    for (src, want) in cases {
        assert_eq!(filled_paths(src), *want, "{src}");
    }
}

#[test]
fn g11_no_filled_path_outside_paint() {
    let src = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    common::rs_files(&src, &mut files);
    assert!(files.len() > 5, "scan found {} files", files.len());
    let paint_rs = src.join("ui").join("paint.rs");
    for f in files {
        if f == paint_rs {
            continue;
        }
        let text = std::fs::read_to_string(&f).unwrap();
        assert_eq!(
            filled_paths(&text),
            0,
            "{} builds a filled path; use paint::fill_convex",
            f.display()
        );
    }
}

// ---------------------------------------------------------------- AC 6

#[test]
fn g11_stroke_floor() {
    for ppp in [1.0_f32, 1.25, 1.5, 2.0, 3.0] {
        let thin = paint::stroke(0.5, MARK, ppp).width;
        assert!(
            (thin - (1.0 / ppp).max(0.5)).abs() < 1e-6,
            "ppp {ppp}: 0.5 pt gives {thin}"
        );
        assert_eq!(paint::stroke(2.0, MARK, ppp).width, 2.0, "ppp {ppp}");
    }
}

/// Every stroke the helpers draw, asked at 0.5 pt where a width is asked,
/// is at least one physical pixel.
#[test]
fn g11_helper_strokes_floored() {
    for ppp in [1.0_f32, 1.5] {
        let shapes = frame(ppp, |ui| {
            let p = ui.painter();
            let thin = Stroke::new(0.5, MARK);
            let r = Rect::from_min_size(pos2(20.0, 20.0), vec2(100.0, 40.0));
            paint::paint_dashed_rect(p, r, 6.0, thin, 4.0, 3.0);
            let _ =
                paint::fill_convex(p, paint::diamond_points(pos2(200.0, 50.0), 9.0), MARK, thin);
            paint::paint_corner_brackets(
                p,
                Rect::from_min_size(pos2(20.0, 100.0), vec2(120.0, 80.0)),
                MARK,
                ppp,
            );
            paint::paint_flourish(p, pos2(300.0, 100.0), MARK, ppp);
            for (i, s) in Symbol::ALL.iter().enumerate() {
                let rect =
                    Rect::from_min_size(pos2(20.0 + i as f32 * 14.0, 250.0), vec2(10.0, 10.0));
                symbols::paint_symbol(p, rect, *s, MARK, ppp);
            }
        });
        let floor = 1.0 / ppp - 1e-6;
        let mut checked = 0;
        for f in marked(&shapes) {
            if let Some(w) = stroke_width(&f.shape)
                && w > 0.0
            {
                checked += 1;
                assert!(w >= floor, "ppp {ppp}: stroke {w} < 1 px: {:?}", f.shape);
            }
        }
        assert!(checked > 30, "only {checked} strokes checked");
    }
}

// ---------------------------------------------------------------- AC 7

/// Text shapes of a `paint_digits` call: (char, left x, galley width).
fn digit_shapes(shapes: &[ClippedShape]) -> Vec<(String, f32, f32)> {
    shapes_flat(shapes)
        .into_iter()
        .filter_map(|f| match f.shape {
            Shape::Text(t) if t.fallback_color == MARK => Some((
                t.galley.text().to_owned(),
                t.pos.x + t.galley.rect.min.x,
                t.galley.rect.width(),
            )),
            _ => None,
        })
        .collect()
}

#[test]
fn g11_digit_cells() {
    for (name, style) in [
        ("body", theme::BODY),
        ("button-lg-sub", theme::BUTTON_LG_SUB),
        ("small", theme::SMALL),
        // Nunito Sans's bundled digits are already tabular (all ten share
        // one advance), so the three styles above cannot show a cell bug;
        // Cormorant's old-style digits are proportional (`1` is far
        // narrower than `0`), so this style can.
        ("card-title", theme::CARD_TITLE),
    ] {
        for ppp in [1.0, 1.25, 1.75] {
            let mut widest = 0.0_f32;
            let draw = |text: &'static str| {
                frame(ppp, move |ui| {
                    paint::paint_digits(ui.painter(), pos2(50.0, 50.0), text, &style, MARK);
                })
            };
            let a = digit_shapes(&draw("0:11 / 1:11"));
            let b = digit_shapes(&draw("8:88 / 8:88"));
            frame(ppp, |ui| {
                for d in '0'..='9' {
                    let w = paint::layout_line(ui.ctx(), &style, &d.to_string(), MARK)
                        .rect
                        .width();
                    widest = widest.max(w);
                }
            });
            assert_eq!(a.len(), b.len(), "{name}: whitespace only advances");
            // Every character's cell sits at the same x: digit centres and
            // the shared separators line up.
            for ((ta, xa, wa), (tb, xb, wb)) in a.iter().zip(&b) {
                let (ca, cb) = (xa + wa / 2.0, xb + wb / 2.0);
                assert!(
                    (ca - cb).abs() < 0.01,
                    "{name} ppp {ppp}: `{ta}` at {ca}, `{tb}` at {cb}"
                );
            }
            // The first ':' starts one cell (the widest digit) after the
            // left edge.
            let colon = a.iter().find(|(t, _, _)| t == ":").unwrap();
            assert!(
                (colon.1 - (50.0 + widest)).abs() < 0.01,
                "{name} ppp {ppp}: cell {} vs widest digit {widest}",
                colon.1 - 50.0
            );
        }
    }
}

// ---------------------------------------------------------------- AC 8

fn path_points(flat: &[Flat]) -> Vec<Vec<Pos2>> {
    flat.iter()
        .filter_map(|f| match &f.shape {
            Shape::Path(p) => Some(p.points.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn g11_symbols() {
    for ppp in [1.0, 2.0] {
        for size in [10.0_f32, 12.0, 14.0, 18.0] {
            let rect = Rect::from_min_size(pos2(40.0, 40.0), vec2(size, size));
            let mut lists = Vec::new();
            for symbol in Symbol::ALL {
                let flat = marked(&frame(ppp, |ui| {
                    symbols::paint_symbol(ui.painter(), rect, symbol, MARK, ppp);
                }));
                assert!(!flat.is_empty(), "{symbol:?} at {size}: nothing painted");
                for f in &flat {
                    assert!(
                        !matches!(f.shape, Shape::Text(_)),
                        "{symbol:?}: text painted"
                    );
                    if let Shape::Path(p) = &f.shape
                        && p.fill != Color32::TRANSPARENT
                    {
                        assert!(
                            paint::is_convex(&p.points).is_ok(),
                            "{symbol:?}: concave fill"
                        );
                    }
                    let bounds = f.shape.visual_bounding_rect();
                    assert!(
                        inside(bounds, rect, 0.01),
                        "{symbol:?} {size} pt ppp {ppp}: {bounds:?} outside {rect:?}"
                    );
                }
                for b in mesh_bounds(ppp, |ui| {
                    symbols::paint_symbol(ui.painter(), rect, symbol, MARK, ppp);
                }) {
                    assert!(
                        inside(b, rect, 0.01),
                        "{symbol:?} {size} pt ppp {ppp}: tessellated {b:?} outside {rect:?} (miter)"
                    );
                }
                lists.push((
                    symbol,
                    format!("{:?}", flat.iter().map(|f| &f.shape).collect::<Vec<_>>()),
                    flat,
                ));
            }
            for (i, (a, la, _)) in lists.iter().enumerate() {
                for (b, lb, _) in lists.iter().skip(i + 1) {
                    assert_ne!(la, lb, "{a:?} and {b:?} paint the same at {size}");
                }
            }
            let undo = path_points(&lists[10].2);
            let redo = path_points(&lists[11].2);
            assert_eq!(undo.len(), redo.len());
            let cx = rect.center().x;
            for (u, r) in undo.iter().zip(&redo) {
                assert_eq!(u.len(), r.len());
                for (pu, pr) in u.iter().zip(r) {
                    assert!(
                        (pr.x - (2.0 * cx - pu.x)).abs() < 1e-3 && (pr.y - pu.y).abs() < 1e-3,
                        "{size} pt: redo {pr:?} is not undo {pu:?} mirrored"
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------- AC 9

#[test]
fn g11_corner_brackets() {
    for ppp in [1.0, 2.0] {
        let rect = Rect::from_min_size(pos2(30.0, 30.0), vec2(300.0, 200.0));
        let flat = marked(&frame(ppp, |ui| {
            paint::paint_corner_brackets(ui.painter(), rect, MARK, ppp);
        }));
        common::check_brackets(&flat, rect, &format!("ppp {ppp}"));
    }
}
