//! Painter-level helpers (Story 4.3, UX-DR3, UX-DR4): the egui limits are
//! handled here once. Focus ring, glow, text halo, dashed rounded rects,
//! convex-only fills, the stroke floor, fixed-width digit cells, corner
//! brackets and the dialog flourish.
//!
//! Nothing in this module adds an accessibility node (Story 4.2, G.10): it
//! only paints. `ui::widgets` turns a helper into a node. Outside this file
//! no source file builds a filled path (G.11 source scan): fills go through
//! [`fill_convex`], which refuses a concave polygon instead of letting
//! epaint draw it wrong.

use std::fmt;
use std::sync::Arc;

use egui::epaint::{PathShape, QuadraticBezierShape, RectShape, Shadow};
use egui::text::{LayoutJob, TextWrapping};
use egui::{
    Align, Color32, Context, FontSelection, Galley, Painter, Pos2, Rect, Response, Shape, Stroke,
    StrokeKind, WidgetText, pos2, vec2,
};

use super::theme::{self, TypeStyle};

/// A stroke of `width` points, never thinner than one physical pixel
/// (`max(width, 1 / ppp)`, AC 6). Every stroke of the helpers goes through
/// it.
#[must_use]
pub fn stroke(width: f32, color: Color32, ppp: f32) -> Stroke {
    let floor = if ppp > 0.0 { 1.0 / ppp } else { 1.0 };
    Stroke::new(width.max(floor), color)
}

/// [`stroke`] with the painter's `pixels_per_point`.
fn floored(painter: &Painter, s: Stroke) -> Stroke {
    stroke(s.width, s.color, painter.pixels_per_point())
}

/// The focus ring (AC 1, UX-DR62): only while `resp.has_focus()`, one
/// `focus-ring-width` stroke in `amber` (`amber-ink` on parchment), outside
/// `resp.rect` grown by `focus-ring-offset`, corner radius `radius` +
/// offset. It paints on the widget's layer with its own clip rect (the
/// ring's outer rect plus one point for the anti-aliasing fringe), so the
/// widget's clip rect never hides it (D7).
pub fn paint_focus_ring(resp: &Response, radius: f32, on_parchment: bool) {
    if !resp.has_focus() {
        return;
    }
    let offset = theme::spacing::FOCUS_RING_OFFSET;
    let width = theme::spacing::FOCUS_RING_WIDTH;
    let ring = resp.rect.expand(offset);
    let clip = ring.expand(width + 1.0);
    let painter = Painter::new(resp.ctx.clone(), resp.layer_id, clip);
    let color = if on_parchment {
        theme::AMBER_INK
    } else {
        theme::AMBER
    };
    painter.rect_stroke(
        ring,
        theme::corner(radius + offset),
        stroke(width, color, painter.pixels_per_point()),
        StrokeKind::Outside,
    );
}

/// The smallest whole `spread` for which the shadow rect's shorter side is
/// greater than `blur` + 0.1 (+ 2 × stroke, none here), so the tessellator
/// never caps the blur (epaint `tessellator.rs`: blur is limited to the
/// shorter side − 0.1 − 2 × stroke).
#[must_use]
pub fn glow_spread(rect: Rect, blur: u8) -> u8 {
    let need = f32::from(blur) + 0.1 - rect.size().min_elem();
    if need < 0.0 {
        return 0;
    }
    // Saturating float-to-int cast: a rect this small is degenerate anyway.
    ((need / 2.0).floor() + 1.0) as u8
}

/// A glow under a control (AC 2): one `epaint::Shadow` with offset 0, the
/// token blur, the spread of [`glow_spread`] and `color`. A circle is a
/// square `rect` with `radius` = half its side. Paint it before the control.
pub fn paint_glow(painter: &Painter, rect: Rect, radius: f32, blur: u8, color: Color32) {
    let shadow = Shadow {
        offset: [0, 0],
        blur,
        spread: glow_spread(rect, blur),
        color,
    };
    // No pixel rounding: a glow has no edge to keep crisp, and rounding
    // could shrink the rect below the blur at fractional scales.
    let shape: RectShape = shadow
        .as_shape(rect, theme::corner(radius))
        .with_round_to_pixels(false);
    painter.add(shape);
}

/// A soft halo behind text (AC 3): a blurred rounded rect ([`paint_glow`])
/// over the galley's ink bounds (`galley.rect ∪ galley.mesh_bounds`, moved
/// to where the galley is painted) grown by `pad`. Paint it before the text.
pub fn paint_text_halo(painter: &Painter, ink_rect: Rect, pad: f32, blur: u8, color: Color32) {
    paint_glow(painter, ink_rect.expand(pad), pad, blur, color);
}

/// The ink bounds of a galley painted at `pos` (layout rect united with the
/// glyph mesh bounds: Cormorant's old-style figures draw outside the layout
/// rect).
#[must_use]
pub fn ink_rect(galley: &Galley, pos: Pos2) -> Rect {
    galley
        .rect
        .union(galley.mesh_bounds)
        .translate(pos.to_vec2())
}

/// The outline of a rounded rect as one closed point list, clockwise on
/// screen, starting at the end of the top-left arc, with the first point
/// repeated at the end. The radius is clamped to half the shorter side; each
/// quarter arc is split so that no chord is more than `max_chord_error`
/// from the true arc.
#[must_use]
pub fn rounded_rect_points(rect: Rect, radius: f32, max_chord_error: f32) -> Vec<Pos2> {
    let r = radius.clamp(0.0, rect.size().min_elem() / 2.0);
    let quarter = std::f32::consts::FRAC_PI_2;
    // Sagitta of a chord spanning angle θ: r (1 − cos(θ / 2)).
    let steps = if r <= max_chord_error {
        1
    } else {
        let theta = 2.0 * (1.0 - max_chord_error / r).acos();
        // Saturating cast; `theta` > 0 here.
        (quarter / theta).ceil().max(1.0) as usize
    };
    // Corner centres and start angles, clockwise from the top-right corner
    // (screen y grows down, so angle −π/2 points up).
    let corners = [
        (pos2(rect.right() - r, rect.top() + r), -quarter),
        (pos2(rect.right() - r, rect.bottom() - r), 0.0),
        (pos2(rect.left() + r, rect.bottom() - r), quarter),
        (pos2(rect.left() + r, rect.top() + r), 2.0 * quarter),
    ];
    let mut points: Vec<Pos2> = Vec::with_capacity(4 * (steps + 1) + 1);
    for (centre, start) in corners {
        for i in 0..=steps {
            let a = start + quarter * (i as f32) / (steps as f32);
            let p = centre + r * vec2(a.cos(), a.sin());
            if points.last().is_none_or(|last| last.distance(p) > 1e-4) {
                points.push(p);
            }
        }
    }
    // Open the outline at the top edge (the end of the top-left arc), so a
    // dash pattern starts on a straight run, then close it.
    if let Some(first) = points.pop() {
        points.insert(0, first);
    }
    if let Some(&first) = points.first() {
        points.push(first);
    }
    points
}

/// A dashed rounded rect (AC 4): the outline of [`rounded_rect_points`]
/// (chords within 0.1 pt of the arcs) dashed in one `dashed_line_many`
/// call, so the dash phase runs on around the corners. The stroke width is
/// floored.
pub fn paint_dashed_rect(
    painter: &Painter,
    rect: Rect,
    radius: f32,
    stroke: Stroke,
    dash: f32,
    gap: f32,
) {
    let points = rounded_rect_points(rect, radius, 0.1);
    let mut shapes = Vec::new();
    Shape::dashed_line_many(&points, floored(painter, stroke), dash, gap, &mut shapes);
    painter.extend(shapes);
}

/// A polygon [`fill_convex`] refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotConvex {
    /// Fewer than three distinct points.
    TooFewPoints,
    /// All points on one line (zero area).
    ZeroArea,
    /// A concave vertex or a self-intersection.
    NotConvex,
}

impl fmt::Display for NotConvex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::TooFewPoints => "fewer than three points",
            Self::ZeroArea => "zero area",
            Self::NotConvex => "concave or self-intersecting",
        })
    }
}

impl std::error::Error for NotConvex {}

/// Checks that `points` (open or closed, any winding) form a convex
/// polygon: at least three distinct points, non-zero area, every turn of the
/// same sign (collinear vertices allowed) and one full turn in total (no
/// self-intersection such as a star).
///
/// # Errors
/// [`NotConvex`] naming the first rule broken.
pub fn is_convex(points: &[Pos2]) -> Result<(), NotConvex> {
    let mut pts: Vec<Pos2> = Vec::with_capacity(points.len());
    for &p in points {
        if pts.last().is_none_or(|last| last.distance(p) > 1e-6) {
            pts.push(p);
        }
    }
    while pts.len() > 1
        && pts
            .first()
            .zip(pts.last())
            .is_some_and(|(a, b)| a.distance(*b) <= 1e-6)
    {
        pts.pop();
    }
    let n = pts.len();
    if n < 3 {
        return Err(NotConvex::TooFewPoints);
    }
    let at = |i: usize| pts.get(i % n).copied().unwrap_or(Pos2::ZERO);
    let mut area2 = 0.0_f32;
    let mut sign = 0.0_f32;
    let mut turning = 0.0_f32;
    for i in 0..n {
        let (a, b, c) = (at(i), at(i + 1), at(i + 2));
        area2 += a.x * b.y - b.x * a.y;
        let (e1, e2) = (b - a, c - b);
        let cross = e1.x * e2.y - e1.y * e2.x;
        let dot = e1.x * e2.x + e1.y * e2.y;
        let scale = e1.length() * e2.length();
        if cross.abs() > 1e-6 * scale.max(1e-12) {
            if sign == 0.0 {
                sign = cross.signum();
            } else if cross.signum() != sign {
                return Err(NotConvex::NotConvex);
            }
        }
        turning += cross.atan2(dot);
    }
    if area2.abs() <= 1e-6 {
        return Err(NotConvex::ZeroArea);
    }
    // A simple convex polygon turns exactly once (±2π); a star turns more.
    if (turning.abs() - std::f32::consts::TAU).abs() > 1e-2 {
        return Err(NotConvex::NotConvex);
    }
    Ok(())
}

/// Adds a filled polygon only if [`is_convex`] accepts it (AC 5); otherwise
/// adds nothing. epaint fills only convex polygons correctly. The stroke (if
/// any) is floored.
///
/// # Errors
/// [`NotConvex`] when the polygon is refused; nothing is painted then.
pub fn fill_convex(
    painter: &Painter,
    points: Vec<Pos2>,
    fill: Color32,
    stroke: Stroke,
) -> Result<(), NotConvex> {
    is_convex(&points)?;
    let stroke = if stroke.is_empty() {
        Stroke::NONE
    } else {
        floored(painter, stroke)
    };
    painter.add(PathShape::convex_polygon(points, fill, stroke));
    Ok(())
}

/// A galley of `text` in a typography token, one line, no wrap.
#[must_use]
pub fn layout_line(ctx: &Context, style: &TypeStyle, text: &str, color: Color32) -> Arc<Galley> {
    layout(ctx, style, text, color, f32::INFINITY, Align::Min)
}

/// A galley of `text` in a typography token, wrapped at `wrap_width`, rows
/// aligned by `halign` (with [`Align::Center`] the galley is centred on
/// x = 0, so paint it at the centre of its box).
#[must_use]
pub fn layout(
    ctx: &Context,
    style: &TypeStyle,
    text: &str,
    color: Color32,
    wrap_width: f32,
    halign: Align,
) -> Arc<Galley> {
    let global = ctx.global_style();
    let job = WidgetText::from(theme::rich(style, text).color(color)).into_layout_job(
        &global,
        FontSelection::Default,
        Align::Min,
    );
    let mut job: LayoutJob = Arc::unwrap_or_clone(job);
    job.wrap = TextWrapping::wrap_at_width(wrap_width);
    job.halign = halign;
    ctx.fonts_mut(|f| f.layout_job(job))
}

/// The width of a digit cell: the widest advance of `0`–`9` in `style`,
/// measured from laid-out galleys.
#[must_use]
pub fn digit_cell_width(ctx: &Context, style: &TypeStyle) -> f32 {
    ('0'..='9')
        .map(|d| {
            layout_line(ctx, style, &d.to_string(), Color32::PLACEHOLDER)
                .rect
                .width()
        })
        .fold(0.0, f32::max)
}

/// The cells of `text` for [`paint_digits`]: per character its left x
/// offset, width and whether it is a digit; plus the total size.
fn digit_cells(ctx: &Context, style: &TypeStyle, text: &str) -> (Vec<(char, f32, f32)>, f32) {
    let cell = digit_cell_width(ctx, style);
    let mut x = 0.0;
    let mut out = Vec::with_capacity(text.len());
    for ch in text.chars() {
        let w = if ch.is_ascii_digit() {
            cell
        } else {
            layout_line(ctx, style, &ch.to_string(), Color32::PLACEHOLDER)
                .rect
                .width()
        };
        out.push((ch, x, w));
        x += w;
    }
    (out, x)
}

/// The size [`paint_digits`] needs for `text`.
#[must_use]
pub fn digits_size(ctx: &Context, style: &TypeStyle, text: &str) -> egui::Vec2 {
    let (_, width) = digit_cells(ctx, style, text);
    let height = layout_line(ctx, style, "0", Color32::PLACEHOLDER)
        .rect
        .height();
    vec2(width, height)
}

/// Text in fixed-width digit cells (AC 7): each character is its own text
/// call; every ASCII digit is centred in a cell as wide as the widest digit
/// of the style, any other character takes its own advance, so a running
/// time never jitters. `pos` is the left top. Whitespace only advances.
pub fn paint_digits(painter: &Painter, pos: Pos2, text: &str, style: &TypeStyle, color: Color32) {
    let ctx = painter.ctx();
    let (cells, _) = digit_cells(ctx, style, text);
    for (ch, x, w) in cells {
        if ch.is_whitespace() {
            continue;
        }
        let galley = layout_line(ctx, style, &ch.to_string(), color);
        let left = pos.x + x + (w - galley.rect.width()) / 2.0;
        painter.galley(pos2(left, pos.y), galley, color);
    }
}

/// A quadratic Bézier from `p0` through control `c` to `p1`, flattened into
/// `segments` chords (end points included, `p0` first).
#[must_use]
pub fn quad_points(p0: Pos2, c: Pos2, p1: Pos2, segments: usize) -> Vec<Pos2> {
    let n = segments.max(1);
    (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let u = 1.0 - t;
            pos2(
                u * u * p0.x + 2.0 * u * t * c.x + t * t * p1.x,
                u * u * p0.y + 2.0 * u * t * c.y + t * t * p1.y,
            )
        })
        .collect()
}

/// Corner bracket geometry, `mockups/key-dialogs.html` `.corner`, in its
/// 22 × 22 viewBox (top-left corner; the others are mirrored).
mod bracket {
    /// Outer L: `M2,20 L2,6 Q2,2 6,2 L20,2`.
    pub const OUTER_START: (f32, f32) = (2.0, 20.0);
    pub const OUTER_ELBOW_IN: (f32, f32) = (2.0, 6.0);
    pub const OUTER_CONTROL: (f32, f32) = (2.0, 2.0);
    pub const OUTER_ELBOW_OUT: (f32, f32) = (6.0, 2.0);
    pub const OUTER_END: (f32, f32) = (20.0, 2.0);
    pub const OUTER_WIDTH: f32 = 2.0;
    /// Inner curve: `M7,14 Q7,7 14,7`.
    pub const INNER: [(f32, f32); 3] = [(7.0, 14.0), (7.0, 7.0), (14.0, 7.0)];
    pub const INNER_WIDTH: f32 = 1.2;
    /// Dot at the elbow: centre (7, 7), r 1.6.
    pub const DOT: (f32, f32) = (7.0, 7.0);
    pub const DOT_RADIUS: f32 = 1.6;
    /// The mock's viewBox side; scaled to `spacing.corner-bracket-size`.
    pub const VIEWBOX: f32 = 22.0;
}

/// Four corner brackets (AC 9, UX-DR4), each in a `corner-bracket-size` box
/// inset `components.corner-bracket.inset` from its corner, mirrored per
/// corner: a 2 pt outer L with a rounded elbow, a 1.2 pt inner curve and a
/// 1.6 pt dot, all in `color` (`bracket` on parchment, `rim` on dark).
pub fn paint_corner_brackets(painter: &Painter, rect: Rect, color: Color32, ppp: f32) {
    use bracket::*;
    let size = theme::spacing::CORNER_BRACKET_SIZE;
    let inset = theme::components::CORNER_BRACKET_INSET;
    let k = size / VIEWBOX;
    // (box origin corner, x direction, y direction) per corner.
    let corners = [
        (pos2(rect.left() + inset, rect.top() + inset), 1.0, 1.0),
        (pos2(rect.right() - inset, rect.top() + inset), -1.0, 1.0),
        (pos2(rect.left() + inset, rect.bottom() - inset), 1.0, -1.0),
        (
            pos2(rect.right() - inset, rect.bottom() - inset),
            -1.0,
            -1.0,
        ),
    ];
    for (origin, sx, sy) in corners {
        let at = |(x, y): (f32, f32)| pos2(origin.x + sx * x * k, origin.y + sy * y * k);
        let mut outer = vec![at(OUTER_START)];
        outer.extend(quad_points(
            at(OUTER_ELBOW_IN),
            at(OUTER_CONTROL),
            at(OUTER_ELBOW_OUT),
            8,
        ));
        outer.push(at(OUTER_END));
        painter.add(PathShape::line(outer, stroke(OUTER_WIDTH * k, color, ppp)));
        let [p0, c, p1] = INNER.map(at);
        painter.add(QuadraticBezierShape::from_points_stroke(
            [p0, c, p1],
            false,
            Color32::TRANSPARENT,
            stroke(INNER_WIDTH * k, color, ppp),
        ));
        painter.circle_filled(at(DOT), DOT_RADIUS * k, color);
    }
}

/// Flourish geometry, `mockups/key-dialogs.html` `.flourish`, in its
/// 220 × 14 viewBox.
mod flourish {
    pub const VIEWBOX: (f32, f32) = (220.0, 14.0);
    /// `M4 7 L92 7`, `M128 7 L216 7`, stroke 1.
    pub const HAIRLINES: [[(f32, f32); 2]; 2] =
        [[(4.0, 7.0), (92.0, 7.0)], [(128.0, 7.0), (216.0, 7.0)]];
    pub const HAIRLINE_WIDTH: f32 = 1.0;
    /// `M92 7 Q100 1 106 7 Q100 13 92 7` and its mirror: four quadratics.
    pub const LEAVES: [[(f32, f32); 3]; 4] = [
        [(92.0, 7.0), (100.0, 1.0), (106.0, 7.0)],
        [(106.0, 7.0), (100.0, 13.0), (92.0, 7.0)],
        [(128.0, 7.0), (120.0, 1.0), (114.0, 7.0)],
        [(114.0, 7.0), (120.0, 13.0), (128.0, 7.0)],
    ];
    pub const LEAF_WIDTH: f32 = 1.2;
    /// `M110 2.5 L114.5 7 L110 11.5 L105.5 7 Z`, filled.
    pub const DIAMOND: [(f32, f32); 4] = [(110.0, 2.5), (114.5, 7.0), (110.0, 11.5), (105.5, 7.0)];
}

/// The dialog flourish (AC 10): a `flourish-width` × `flourish-height` box
/// whose top centre is `center_top`: two 1 pt hairlines, two leaves of
/// 1.2 pt curves and one convex diamond, all in `color`.
pub fn paint_flourish(painter: &Painter, center_top: Pos2, color: Color32, ppp: f32) {
    use flourish::*;
    let w = theme::components::modal_dialog::FLOURISH_WIDTH;
    let h = theme::components::modal_dialog::FLOURISH_HEIGHT;
    let (kx, ky) = (w / VIEWBOX.0, h / VIEWBOX.1);
    let left = center_top.x - w / 2.0;
    let at = |(x, y): (f32, f32)| pos2(left + x * kx, center_top.y + y * ky);
    for [a, b] in HAIRLINES {
        painter.line_segment([at(a), at(b)], stroke(HAIRLINE_WIDTH, color, ppp));
    }
    for leaf in LEAVES {
        painter.add(QuadraticBezierShape::from_points_stroke(
            leaf.map(at),
            false,
            Color32::TRANSPARENT,
            stroke(LEAF_WIDTH, color, ppp),
        ));
    }
    // The diamond is convex by construction; a refusal would only drop it.
    let _ = fill_convex(painter, DIAMOND.map(at).to_vec(), color, Stroke::NONE);
}

/// A diamond (a square of side `side` turned 45°) centred on `center`:
/// the checklist bullet (AC 10). Its diagonal is `side × √2`.
#[must_use]
pub fn diamond_points(center: Pos2, side: f32) -> Vec<Pos2> {
    let h = side * std::f32::consts::FRAC_1_SQRT_2;
    vec![
        pos2(center.x, center.y - h),
        pos2(center.x + h, center.y),
        pos2(center.x, center.y + h),
        pos2(center.x - h, center.y),
    ]
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing, clippy::panic, clippy::unwrap_used)]

    use super::*;

    #[test]
    fn stroke_floor_is_one_physical_pixel() {
        for ppp in [1.0_f32, 1.25, 1.5, 2.0, 3.0] {
            let thin = stroke(0.5, Color32::RED, ppp).width;
            let want = (1.0 / ppp).max(0.5);
            assert!((thin - want).abs() < 1e-6, "ppp {ppp}: {thin}");
            assert_eq!(stroke(2.0, Color32::RED, ppp).width, 2.0, "ppp {ppp}");
        }
    }

    #[test]
    fn glow_spread_is_the_smallest_whole_number() {
        for side in [4.0_f32, 11.0, 26.0, 60.0] {
            for blur in [8_u8, 10, 12, 16, 18] {
                let rect = Rect::from_min_size(Pos2::ZERO, vec2(side * 2.0, side));
                let s = glow_spread(rect, blur);
                let ok = |s: u8| side + 2.0 * f32::from(s) > f32::from(blur) + 0.1;
                assert!(ok(s), "side {side} blur {blur}: spread {s} clamps");
                assert!(
                    s == 0 || !ok(s - 1),
                    "side {side} blur {blur}: {s} not smallest"
                );
            }
        }
    }

    /// Distance from `p` to the outline of the rounded rect.
    fn outline_distance(rect: Rect, r: f32, p: Pos2) -> f32 {
        let inner = rect.shrink(r);
        let q = pos2(
            p.x.clamp(inner.left(), inner.right()),
            p.y.clamp(inner.top(), inner.bottom()),
        );
        (p.distance(q) - r).abs()
    }

    #[test]
    fn rounded_rect_points_lie_on_the_outline() {
        let rect = Rect::from_min_size(pos2(10.0, 20.0), vec2(120.0, 40.0));
        for radius in [0.0_f32, 4.0, 12.0, 9999.0] {
            let pts = rounded_rect_points(rect, radius, 0.1);
            let r = radius.min(20.0);
            assert_eq!(pts.first(), pts.last(), "closed");
            assert!(pts.len() >= 5, "radius {radius}: {}", pts.len());
            for p in &pts {
                assert!(
                    outline_distance(rect, r, *p) < 1e-3,
                    "radius {radius}: {p:?}"
                );
            }
            // Every chord midpoint within 0.1 pt of the arc.
            for w in pts.windows(2) {
                let mid = w[0].lerp(w[1], 0.5);
                assert!(
                    outline_distance(rect, r, mid) <= 0.1 + 1e-4,
                    "radius {radius}: chord {w:?}"
                );
            }
        }
    }

    #[test]
    fn convexity() {
        let p = |v: &[(f32, f32)]| v.iter().map(|&(x, y)| pos2(x, y)).collect::<Vec<_>>();
        assert_eq!(
            is_convex(&p(&[(0.0, 0.0), (10.0, 0.0), (5.0, 8.0)])),
            Ok(())
        );
        assert_eq!(
            is_convex(&p(&[
                (0.0, 0.0),
                (10.0, 0.0),
                (10.0, 10.0),
                (0.0, 10.0),
                (0.0, 0.0)
            ])),
            Ok(()),
            "closed square"
        );
        assert_eq!(is_convex(&diamond_points(pos2(5.0, 5.0), 9.0)), Ok(()));
        assert_eq!(
            is_convex(&p(&[
                (0.0, 0.0),
                (10.0, 0.0),
                (10.0, 4.0),
                (4.0, 4.0),
                (4.0, 10.0),
                (0.0, 10.0)
            ])),
            Err(NotConvex::NotConvex),
            "L"
        );
        assert_eq!(
            is_convex(&p(&[(0.0, 0.0), (10.0, 10.0), (10.0, 0.0), (0.0, 10.0)])),
            Err(NotConvex::NotConvex),
            "bow-tie"
        );
        assert_eq!(
            is_convex(&p(&[(0.0, 0.0), (1.0, 1.0)])),
            Err(NotConvex::TooFewPoints)
        );
        assert_eq!(
            is_convex(&p(&[(0.0, 0.0), (1.0, 1.0), (2.0, 2.0)])),
            Err(NotConvex::ZeroArea)
        );
        // A five-pointed star turns the same way at every vertex but twice
        // in total.
        let star: Vec<Pos2> = (0..5)
            .map(|i| {
                let a = std::f32::consts::TAU * (i as f32) * 2.0 / 5.0;
                pos2(a.cos(), a.sin())
            })
            .collect();
        assert_eq!(is_convex(&star), Err(NotConvex::NotConvex), "star");
    }

    #[test]
    fn quad_points_end_on_the_end_points() {
        let q = quad_points(pos2(0.0, 0.0), pos2(5.0, 10.0), pos2(10.0, 0.0), 4);
        assert_eq!(q.len(), 5);
        assert_eq!(q[0], pos2(0.0, 0.0));
        assert_eq!(q[4], pos2(10.0, 0.0));
        assert_eq!(q[2], pos2(5.0, 5.0));
    }
}
