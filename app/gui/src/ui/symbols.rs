//! The 14 painted symbols (Story 4.3 AC 8, D9; the D-pad's left and right
//! triangles from Story 4.8, which paints D-pad down with `ChevronDown`, the
//! same shape): no bundled font covers
//! them all, so they are always painted, never typed. Original shapes after
//! the approved mocks (`mockups/key-dialogs.html`,
//! `directions-oot-2026-11-20.html`). Painting adds no accessibility node;
//! `ui::widgets::symbol` is the standalone node.

use egui::epaint::PathShape;
use egui::{Color32, Painter, Pos2, Rect, Stroke, pos2, vec2};

use super::paint;

/// A painted symbol.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Symbol {
    /// ▶
    Play,
    /// ■
    Stop,
    /// ⏮
    ToStart,
    /// ▾
    ChevronDown,
    /// ▲
    TriangleUp,
    /// ◀ (D-pad left, Story 4.8)
    TriangleLeft,
    /// ▶ as the D-pad's right arrow (Story 4.8); the same shape family as
    /// ▲, smaller than `Play`.
    TriangleRight,
    /// ⌫
    Backspace,
    /// ↵
    Return,
    /// ↑
    ArrowUp,
    /// ♯
    Sharp,
    /// ♪
    Note,
    /// ↶
    Undo,
    /// ↷ (↶ mirrored on the vertical axis)
    Redo,
}

impl Symbol {
    /// Every symbol, in the gallery's order.
    pub const ALL: [Self; 14] = [
        Self::Play,
        Self::Stop,
        Self::ToStart,
        Self::ChevronDown,
        Self::TriangleUp,
        Self::Backspace,
        Self::Return,
        Self::ArrowUp,
        Self::Sharp,
        Self::Note,
        Self::Undo,
        Self::Redo,
        Self::TriangleLeft,
        Self::TriangleRight,
    ];

    /// The Unicode character this symbol stands for: for tests and docs
    /// only, never painted.
    #[must_use]
    pub const fn glyph(self) -> char {
        match self {
            Self::Play => '▶',
            Self::Stop => '■',
            Self::ToStart => '⏮',
            Self::ChevronDown => '▾',
            Self::TriangleUp => '▲',
            Self::TriangleLeft => '◀',
            // U+25B8: `Play` already stands for U+25B6.
            Self::TriangleRight => '▸',
            Self::Backspace => '⌫',
            Self::Return => '↵',
            Self::ArrowUp => '↑',
            Self::Sharp => '♯',
            Self::Note => '♪',
            Self::Undo => '↶',
            Self::Redo => '↷',
        }
    }

    /// A plain-words name (the gallery's; call sites pass their own).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Play => "Play",
            Self::Stop => "Stop",
            Self::ToStart => "Back to start",
            Self::ChevronDown => "Expand",
            Self::TriangleUp => "Up",
            Self::TriangleLeft => "Left",
            Self::TriangleRight => "Right",
            Self::Backspace => "Backspace",
            Self::Return => "Return",
            Self::ArrowUp => "Arrow up",
            Self::Sharp => "Sharp",
            Self::Note => "Note",
            Self::Undo => "Undo",
            Self::Redo => "Redo",
        }
    }
}

/// Whether `name` contains one of the 14 symbol characters (a node name must
/// be plain words, AC 8).
#[must_use]
pub fn has_glyph(name: &str) -> bool {
    Symbol::ALL.iter().any(|s| name.contains(s.glyph()))
}

/// One drawing primitive in a symbol's viewBox units.
#[derive(Clone, Debug)]
enum Prim {
    /// A convex filled polygon.
    Fill(Vec<Pos2>),
    /// A filled rounded rect: min, max, corner radius.
    RoundRect(Pos2, Pos2, f32),
    /// An open polyline with a stroke width.
    Line(Vec<Pos2>, f32),
    /// A closed polyline with a stroke width.
    Loop(Vec<Pos2>, f32),
}

fn p(x: f32, y: f32) -> Pos2 {
    pos2(x, y)
}

/// A rotated ellipse as a convex polygon (the ♪ head).
fn ellipse(c: Pos2, rx: f32, ry: f32, angle: f32, n: usize) -> Vec<Pos2> {
    let (s, co) = angle.sin_cos();
    (0..n)
        .map(|i| {
            let t = std::f32::consts::TAU * (i as f32) / (n as f32);
            let (x, y) = (rx * t.cos(), ry * t.sin());
            pos2(c.x + x * co - y * s, c.y + x * s + y * co)
        })
        .collect()
}

/// The ↶ strokes (`key-dialogs.html` history button, 13 × 12, stroke 1.6).
fn undo_prims() -> Vec<Prim> {
    let w = 1.6;
    let head = vec![p(4.2, 2.2), p(1.4, 5.0), p(4.2, 7.8)];
    let mut body = vec![p(1.8, 5.0)];
    body.extend(paint::quad_points(
        p(8.0, 5.0),
        p(11.6, 5.0),
        p(11.6, 7.8),
        6,
    ));
    body.extend(
        paint::quad_points(p(11.6, 7.8), p(11.6, 10.6), p(8.4, 10.6), 6)
            .into_iter()
            .skip(1),
    );
    body.push(p(5.6, 10.6));
    vec![Prim::Line(head, w), Prim::Line(body, w)]
}

/// The viewBox (width, height) and primitives of `symbol`.
fn geometry(symbol: Symbol) -> ((f32, f32), Vec<Prim>) {
    match symbol {
        // Transport play: `M1.5 0.5 L9.5 5.5 L1.5 10.5 Z` in 10 × 11.
        Symbol::Play => (
            (10.0, 11.0),
            vec![Prim::Fill(vec![p(1.5, 0.5), p(9.5, 5.5), p(1.5, 10.5)])],
        ),
        // Transport stop: `rect 9 × 9 rx 1`.
        Symbol::Stop => (
            (9.0, 9.0),
            vec![Prim::RoundRect(p(0.0, 0.0), p(9.0, 9.0), 1.0)],
        ),
        // Bottom-bar "Play from start": `rect x1 y2 w3 h14` +
        // `M17 2 L5 9 L17 16 Z` in 18 × 18.
        Symbol::ToStart => (
            (18.0, 18.0),
            vec![
                Prim::RoundRect(p(1.0, 2.0), p(4.0, 16.0), 0.0),
                Prim::Fill(vec![p(17.0, 2.0), p(17.0, 16.0), p(5.0, 9.0)]),
            ],
        ),
        // Track-picker caret: `M0.5 0.5 L9.5 0.5 L5 6.5 Z` in 10 × 7.
        Symbol::ChevronDown => (
            (10.0, 7.0),
            vec![Prim::Fill(vec![p(0.5, 0.5), p(9.5, 0.5), p(5.0, 6.5)])],
        ),
        // The same caret pointing up.
        Symbol::TriangleUp => (
            (10.0, 7.0),
            vec![Prim::Fill(vec![p(5.0, 0.5), p(9.5, 6.5), p(0.5, 6.5)])],
        ),
        // The D-pad's side arrows (Story 4.8): ▲ turned (down is ChevronDown).
        Symbol::TriangleLeft => (
            (7.0, 10.0),
            vec![Prim::Fill(vec![p(6.5, 0.5), p(6.5, 9.5), p(0.5, 5.0)])],
        ),
        Symbol::TriangleRight => (
            (7.0, 10.0),
            vec![Prim::Fill(vec![p(0.5, 0.5), p(6.5, 5.0), p(0.5, 9.5)])],
        ),
        // Key-chip ⌫: a left-pointing pentagon outline and an ×.
        Symbol::Backspace => (
            (16.0, 12.0),
            vec![
                Prim::Loop(
                    vec![
                        p(5.0, 1.0),
                        p(15.0, 1.0),
                        p(15.0, 11.0),
                        p(5.0, 11.0),
                        p(1.0, 6.0),
                    ],
                    1.2,
                ),
                Prim::Line(vec![p(8.0, 4.0), p(12.0, 8.0)], 1.2),
                Prim::Line(vec![p(12.0, 4.0), p(8.0, 8.0)], 1.2),
            ],
        ),
        // Key-chip ↵: down then left, with an open arrowhead.
        Symbol::Return => (
            (14.0, 12.0),
            vec![
                Prim::Line(vec![p(12.5, 1.0), p(12.5, 7.0), p(2.0, 7.0)], 1.4),
                Prim::Line(vec![p(5.5, 3.5), p(2.0, 7.0), p(5.5, 10.5)], 1.4),
            ],
        ),
        // Fingering arrow ↑: stem and open head.
        Symbol::ArrowUp => (
            (10.0, 14.0),
            vec![
                Prim::Line(vec![p(5.0, 13.0), p(5.0, 1.5)], 1.4),
                Prim::Line(vec![p(1.5, 5.0), p(5.0, 1.5), p(8.5, 5.0)], 1.4),
            ],
        ),
        // ♯: two slanted verticals, two thick rising horizontals.
        Symbol::Sharp => (
            (12.0, 14.0),
            vec![
                Prim::Line(vec![p(4.8, 1.0), p(3.8, 13.0)], 1.1),
                Prim::Line(vec![p(8.6, 1.0), p(7.6, 13.0)], 1.1),
                Prim::Line(vec![p(1.0, 5.6), p(11.0, 4.4)], 2.0),
                Prim::Line(vec![p(1.0, 9.6), p(11.0, 8.4)], 2.0),
            ],
        ),
        // ♪: one tilted elliptic head (convex), a stem and a curved flag.
        Symbol::Note => {
            let mut flag = paint::quad_points(p(6.0, 1.0), p(6.6, 4.2), p(9.4, 5.2), 4);
            flag.extend(
                paint::quad_points(p(9.4, 5.2), p(10.2, 6.2), p(9.2, 8.4), 3)
                    .into_iter()
                    .skip(1),
            );
            (
                (11.0, 14.0),
                vec![
                    Prim::Fill(ellipse(p(3.7, 11.2), 2.7, 1.9, -0.35, 16)),
                    Prim::Line(vec![p(6.0, 11.0), p(6.0, 1.0)], 1.3),
                    Prim::Line(flag, 1.3),
                ],
            )
        }
        Symbol::Undo => ((13.0, 12.0), undo_prims()),
        // ↷ = ↶ mirrored on the vertical axis of its viewBox.
        Symbol::Redo => {
            let w = 13.0;
            let mirror = |v: Vec<Pos2>| v.into_iter().map(|q| p(w - q.x, q.y)).collect();
            let prims = undo_prims()
                .into_iter()
                .map(|prim| match prim {
                    Prim::Line(v, sw) => Prim::Line(mirror(v), sw),
                    other => other,
                })
                .collect();
            ((w, 12.0), prims)
        }
    }
}

/// How far (in half stroke widths) epaint's stroke of `points` reaches from
/// its centre line at the farthest vertex: 1 at a butt end or a straight
/// run, `1 / |n|` at a miter join (n = the mean of the two edge normals), and
/// the cut-corner reach at an open path's corner sharper than a right angle
/// (epaint `tessellator.rs`, `Path::add_open_points` / `add_line_loop`).
fn join_reach(points: &[Pos2], closed: bool) -> f32 {
    let n = points.len();
    if n < 3 {
        return 1.0;
    }
    let normal = |a: Pos2, b: Pos2| (b - a).normalized().rot90();
    let mut reach = 1.0_f32;
    let count = if closed { n } else { n - 2 };
    for k in 0..count {
        let (i0, i1, i2) = if closed {
            (k, (k + 1) % n, (k + 2) % n)
        } else {
            (k, k + 1, k + 2)
        };
        let (Some(&a), Some(&b), Some(&c)) = (points.get(i0), points.get(i1), points.get(i2))
        else {
            continue;
        };
        let (n0, n1) = (normal(a, b), normal(b, c));
        let mean = (n0 + n1) * 0.5;
        let lsq = mean.length_sq();
        let miter = if lsq > 0.0 { 1.0 / lsq.sqrt() } else { 1.0 };
        // Near a right angle f32 rounding decides which join epaint draws
        // (a mirrored path can fall on the other side), so take the larger.
        let r = if !closed && lsq < 0.5 + 1e-3 {
            let centre = mean.normalized();
            let n0c = (n0 + centre) * 0.5;
            let n1c = (n1 + centre) * 0.5;
            let cut = (1.0 / n0c.length()).max(1.0 / n1c.length());
            if lsq >= 0.5 - 1e-3 {
                cut.max(miter)
            } else {
                cut
            }
        } else {
            miter
        };
        if r.is_finite() {
            reach = reach.max(r);
        }
    }
    reach
}

/// Paints `symbol` inside `rect` (AC 8): the viewBox scaled uniformly and
/// centred, then shrunk so every floored stroke (half-width included) stays
/// inside `rect`. Only rect fills, [`paint::fill_convex`] polygons and
/// floored strokes; no text, no node.
pub fn paint_symbol(painter: &Painter, rect: Rect, symbol: Symbol, color: Color32, ppp: f32) {
    let ((vw, vh), prims) = geometry(symbol);
    // Per stroke: half its width (viewBox units) times how far epaint's
    // joins reach beyond the half width (miter up to √2 at a right angle,
    // more on a closed loop's sharp corner).
    let strokes: Vec<(f32, f32)> = prims
        .iter()
        .filter_map(|prim| match prim {
            Prim::Line(pts, w) => Some((*w / 2.0, join_reach(pts, false))),
            Prim::Loop(pts, w) => Some((*w / 2.0, join_reach(pts, true))),
            _ => None,
        })
        .collect();
    let fit = |r: Rect| (r.width() / vw).min(r.height() / vh).max(0.0);
    let s0 = fit(rect);
    // Stroke pad: the farthest floored join at the first scale; the final
    // scale is smaller, so its strokes never reach beyond the pad.
    let floor_half = 0.5 / ppp.max(f32::MIN_POSITIVE);
    let pad = strokes
        .iter()
        .map(|(half, reach)| (half * s0).max(floor_half) * reach)
        .fold(0.0_f32, f32::max);
    let inner = rect.shrink(pad);
    let s = fit(inner);
    let origin = inner.center() - vec2(vw, vh) * s / 2.0;
    let at = |q: Pos2| origin + q.to_vec2() * s;
    for prim in prims {
        match prim {
            Prim::Fill(points) => {
                // Convex by construction; a refusal would only drop it.
                let _ = paint::fill_convex(
                    painter,
                    points.into_iter().map(at).collect(),
                    color,
                    Stroke::NONE,
                );
            }
            Prim::RoundRect(min, max, r) => {
                painter.rect_filled(
                    Rect::from_min_max(at(min), at(max)),
                    egui::CornerRadius::same((r * s).round().clamp(0.0, 255.0) as u8),
                    color,
                );
            }
            Prim::Line(points, w) => {
                let st = paint::stroke(w * s, color, ppp);
                painter.add(PathShape::line(points.into_iter().map(at).collect(), st));
            }
            Prim::Loop(points, w) => {
                let st = paint::stroke(w * s, color, ppp);
                painter.add(PathShape::closed_line(
                    points.into_iter().map(at).collect(),
                    st,
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fourteen_distinct_glyphs() {
        let mut g: Vec<char> = Symbol::ALL.iter().map(|s| s.glyph()).collect();
        g.sort_unstable();
        g.dedup();
        assert_eq!(g.len(), 14);
        assert_eq!(
            g.iter().collect::<String>().chars().count(),
            "▶■⏮▾▲⌫↵↑♯♪↶↷◀▸".chars().count()
        );
        for s in Symbol::ALL {
            assert!(!has_glyph(s.name()), "{:?} name has a glyph", s);
        }
    }

    #[test]
    fn glyph_names_are_refused() {
        assert!(has_glyph("Play ▶"));
        assert!(has_glyph("♯"));
        assert!(!has_glyph("Play from cursor"));
    }
}
