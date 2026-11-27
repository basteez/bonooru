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
    // A scope clipped to the widget's own rect: the ring must still show.
    ui.scope(|ui| {
        ui.set_clip_rect(rect);
        paint::paint_focus_ring(&resp, theme::rounded::MD, state.on_parchment);
    });
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
    assert_eq!(glow.1.blur_width, 12.0);
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
            let limit = f32::from(blur) + 0.1;
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
    // The halo is sized from the ink: Cormorant's "3" draws below its
    // layout rect.
    let text = common::texts(&flat);
    let (_, text_ink, text_index) = text.iter().find(|(t, _, _)| t == "3").unwrap();
    assert!(inside(*text_ink, halo, 1e-3));
    assert!(halo_index < *text_index, "halo behind the text");
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

// ---------------------------------------------------------------- AC 5

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

/// Counts filled-path constructions in Rust source: `convex_polygon(`
/// anywhere, and a `PathShape { ... }` literal that sets `fill`.
fn filled_paths(source: &str) -> usize {
    let s = common::strip_line_comments(source);
    let mut n = s.matches("convex_polygon(").count();
    let mut rest = s.as_str();
    while let Some(i) = rest.find("PathShape {") {
        let body = &rest[i + "PathShape {".len()..];
        let mut depth = 1;
        let mut end = body.len();
        for (j, c) in body.char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = j;
                        break;
                    }
                }
                _ => {}
            }
        }
        if body[..end].contains("fill") {
            n += 1;
        }
        rest = &body[end..];
    }
    n
}

#[test]
fn g11_filled_path_scan_self_test() {
    assert_eq!(filled_paths("Shape::convex_polygon(pts, c, s)"), 1);
    assert_eq!(filled_paths("PathShape::convex_polygon(pts, c, s)"), 1);
    assert_eq!(
        filled_paths("PathShape { points, closed: true, fill: c, stroke }"),
        1
    );
    assert_eq!(
        filled_paths("PathShape {\n  points,\n  closed: true,\n  fill,\n  stroke: s.into(),\n}"),
        1
    );
    assert_eq!(filled_paths("PathShape::line(points, stroke)"), 0);
    assert_eq!(filled_paths("// never convex_polygon( here"), 0);
    assert_eq!(filled_paths("painter.rect_filled(r, 0.0, c)"), 0);
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
