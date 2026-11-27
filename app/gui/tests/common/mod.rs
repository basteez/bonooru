//! Shared helpers of the UI tests (Story 4.3): flattened frame shapes and
//! node queries. Not every test file uses every helper.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use egui::epaint::{ClippedShape, Shape};
use egui::{Color32, Pos2, Rect, pos2, vec2};
use ocb_app::ui::theme;

/// One painted shape: its index in paint order, the clip rect of its
/// `ClippedShape`, and the shape (nested `Shape::Vec`s walked).
#[derive(Clone, Debug)]
pub struct Flat {
    pub index: usize,
    pub clip: Rect,
    pub shape: Shape,
}

/// Every shape of the frame in paint order, `Shape::Vec` flattened.
pub fn shapes_flat(shapes: &[ClippedShape]) -> Vec<Flat> {
    fn walk(shape: &Shape, clip: Rect, out: &mut Vec<Flat>) {
        match shape {
            Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, out)),
            other => out.push(Flat {
                index: out.len(),
                clip,
                shape: other.clone(),
            }),
        }
    }
    let mut out = Vec::new();
    for c in shapes {
        walk(&c.shape, c.clip_rect, &mut out);
    }
    out
}

/// The stroke colour of a shape that has one.
pub fn stroke_color(shape: &Shape) -> Option<Color32> {
    match shape {
        Shape::LineSegment { stroke, .. } => Some(stroke.color),
        Shape::Path(p) => match &p.stroke.color {
            egui::epaint::ColorMode::Solid(c) => Some(*c),
            egui::epaint::ColorMode::UV(_) => None,
        },
        Shape::QuadraticBezier(q) => match &q.stroke.color {
            egui::epaint::ColorMode::Solid(c) => Some(*c),
            egui::epaint::ColorMode::UV(_) => None,
        },
        Shape::Rect(r) => Some(r.stroke.color),
        Shape::Circle(c) => Some(c.stroke.color),
        _ => None,
    }
}

/// The stroke width of a shape that has one (0 for none).
pub fn stroke_width(shape: &Shape) -> Option<f32> {
    match shape {
        Shape::LineSegment { stroke, .. } => Some(stroke.width),
        Shape::Path(p) => Some(p.stroke.width),
        Shape::QuadraticBezier(q) => Some(q.stroke.width),
        Shape::CubicBezier(q) => Some(q.stroke.width),
        Shape::Rect(r) => Some(r.stroke.width),
        Shape::Circle(c) => Some(c.stroke.width),
        Shape::Ellipse(e) => Some(e.stroke.width),
        _ => None,
    }
}

/// Whether a shape uses `color` as fill or stroke.
pub fn uses(shape: &Shape, color: Color32) -> bool {
    let fill = match shape {
        Shape::Path(p) => Some(p.fill),
        Shape::QuadraticBezier(q) => Some(q.fill),
        Shape::Rect(r) => Some(r.fill),
        Shape::Circle(c) => Some(c.fill),
        _ => None,
    };
    fill == Some(color) || stroke_color(shape) == Some(color)
}

/// Every text shape: (text, ink rect on screen, shape index).
pub fn texts(flat: &[Flat]) -> Vec<(String, Rect, usize)> {
    flat.iter()
        .filter_map(|f| match &f.shape {
            Shape::Text(t) => {
                let ink = t.galley.rect.union(t.galley.mesh_bounds);
                Some((
                    t.galley.text().to_owned(),
                    ink.translate(t.pos.to_vec2()),
                    f.index,
                ))
            }
            _ => None,
        })
        .collect()
}

/// `a` ⊆ `b` within `eps`.
pub fn inside(a: Rect, b: Rect, eps: f32) -> bool {
    a.min.x >= b.min.x - eps
        && a.min.y >= b.min.y - eps
        && a.max.x <= b.max.x + eps
        && a.max.y <= b.max.y + eps
}

/// Two rects equal within `eps`.
pub fn rect_eq(a: Rect, b: Rect, eps: f32) -> bool {
    (a.min - b.min).length() <= eps && (a.max - b.max).length() <= eps
}

/// All `.rs` files under `dir`.
pub fn rs_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rs_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Source text with `//` line comments removed (prose may name a pattern).
pub fn strip_line_comments(source: &str) -> String {
    source
        .lines()
        .map(|l| match l.find("//") {
            Some(i) => &l[..i],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The four bracket boxes of `rect` (TL, TR, BL, BR) and their mirror signs.
pub fn bracket_boxes(rect: Rect) -> [(Rect, f32, f32); 4] {
    let s = theme::spacing::CORNER_BRACKET_SIZE;
    let i = theme::components::CORNER_BRACKET_INSET;
    [
        (
            Rect::from_min_size(rect.min + vec2(i, i), vec2(s, s)),
            1.0,
            1.0,
        ),
        (
            Rect::from_min_size(pos2(rect.right() - i - s, rect.top() + i), vec2(s, s)),
            -1.0,
            1.0,
        ),
        (
            Rect::from_min_size(pos2(rect.left() + i, rect.bottom() - i - s), vec2(s, s)),
            1.0,
            -1.0,
        ),
        (
            Rect::from_min_size(
                pos2(rect.right() - i - s, rect.bottom() - i - s),
                vec2(s, s),
            ),
            -1.0,
            -1.0,
        ),
    ]
}

/// Four corner sets, each 1 outer path (2 pt), 1 quadratic (1.2 pt) and 1
/// dot (r 1.6) inside its box; the sets are mirror images of the top-left.
pub fn check_brackets(flat: &[Flat], rect: Rect, ctx: &str) {
    let boxes = bracket_boxes(rect);
    let mut outer: Vec<Vec<Pos2>> = vec![Vec::new(); 4];
    for (k, (bx, _, _)) in boxes.iter().enumerate() {
        let mine: Vec<&Flat> = flat
            .iter()
            .filter(|f| inside(f.shape.visual_bounding_rect(), *bx, 0.01))
            .collect();
        let paths: Vec<_> = mine
            .iter()
            .filter_map(|f| match &f.shape {
                Shape::Path(p) if (p.stroke.width - 2.0).abs() < 1e-4 => Some(p.points.clone()),
                _ => None,
            })
            .collect();
        let quads = mine
            .iter()
            .filter(|f| matches!(&f.shape, Shape::QuadraticBezier(q) if (q.stroke.width - 1.2).abs() < 1e-4))
            .count();
        let dots = mine
            .iter()
            .filter(|f| matches!(&f.shape, Shape::Circle(c) if (c.radius - 1.6).abs() < 1e-4))
            .count();
        assert_eq!(
            (paths.len(), quads, dots),
            (1, 1, 1),
            "{ctx}: corner {k} in {bx:?}"
        );
        outer[k] = paths[0].clone();
    }
    assert_eq!(
        flat.len(),
        12,
        "{ctx}: 4 × 3 shapes, nothing outside the boxes"
    );
    // Mirror images: corner k's points, mapped back to the top-left box.
    let tl = boxes[0].0;
    for (k, (bx, sx, sy)) in boxes.iter().enumerate() {
        for (p, q) in outer[k].iter().zip(&outer[0]) {
            let local_x = if *sx > 0.0 {
                p.x - bx.left()
            } else {
                bx.right() - p.x
            };
            let local_y = if *sy > 0.0 {
                p.y - bx.top()
            } else {
                bx.bottom() - p.y
            };
            assert!(
                (local_x - (q.x - tl.left())).abs() < 1e-3
                    && (local_y - (q.y - tl.top())).abs() < 1e-3,
                "{ctx}: corner {k} is not a mirror image"
            );
        }
    }
}
