//! G.3 (gallery part): the token gallery renders without a missing glyph
//! (egui_kittest fails on one) and shows every colour token and every
//! typography token by its exact name.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use egui::{Theme, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use ocb_app::app::{App, Mode};
use ocb_app::ui::theme;

#[test]
fn g3_gallery_lists_every_colour_and_typography_token() {
    // At 1024x680 the scroll area clips the rows below the fold, but clipped
    // widgets stay in the AccessKit tree, so every row is queryable.
    let mut h = Harness::builder()
        .with_size(vec2(1024.0, 680.0))
        .with_theme(Theme::Light)
        .build_eframe(|cc| App::new(cc, Mode::Gallery));
    h.run();
    let names = theme::color_tokens()
        .iter()
        .map(|t| t.name)
        .chain(theme::typography_tokens().iter().map(|(n, _)| *n));
    for name in names {
        // Exact label match: `label` does not match `note-label`.
        assert_eq!(
            h.query_all_by_label(name).count(),
            1,
            "gallery must show `{name}` exactly once"
        );
    }
}

/// Ink rect of every text shape in the frame: the galley's layout rect
/// united with its glyph mesh bounds (old-style figures and descenders can
/// draw outside the layout rect), translated to the screen.
fn text_rects(h: &Harness<'_, App>) -> Vec<(String, egui::Rect)> {
    use egui::epaint::Shape;
    fn walk(shape: &Shape, out: &mut Vec<(String, egui::Rect)>) {
        match shape {
            Shape::Text(t) => {
                let ink = t.galley.rect.union(t.galley.mesh_bounds);
                out.push((t.galley.text().to_owned(), ink.translate(t.pos.to_vec2())));
            }
            Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for c in &h.output().shapes {
        walk(&c.shape, &mut out);
    }
    out
}

/// A harness tall enough that nothing is scrolled out (off-screen labels
/// are not painted).
fn tall_gallery(ppp: f32) -> Harness<'static, App> {
    let mut h = Harness::builder()
        .with_size(vec2(1024.0, 4000.0))
        .with_pixels_per_point(ppp)
        .build_eframe(|cc| App::new(cc, Mode::Gallery));
    h.run();
    h
}

/// No two texts of the gallery overlap, ink included (owner review
/// 2026-11-27: the countdown "3" ran into the next typography row).
#[test]
fn g3_gallery_texts_never_overlap() {
    for ppp in PPPS {
        texts_never_overlap(ppp);
    }
}

/// The owner's macOS scale (2.0) and two others.
const PPPS: [f32; 3] = [1.0, 1.5, 2.0];

fn texts_never_overlap(ppp: f32) {
    let h = tall_gallery(ppp);
    let texts = text_rects(&h);
    assert!(texts.len() > 60, "too few texts: {}", texts.len());
    let mut hits = Vec::new();
    for (i, (a, ra)) in texts.iter().enumerate() {
        for (b, rb) in texts.iter().skip(i + 1) {
            if ra.intersects(*rb) && ra.intersect(*rb).area() > 0.0 {
                hits.push(format!("ppp {ppp}: `{a}` {ra:?} overlaps `{b}` {rb:?}"));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "overlapping texts:\n  {}",
        hits.join("\n  ")
    );
}

/// The amber-ink focus ring never covers the swatch's own label (owner
/// review 2026-11-27).
#[test]
fn g3_amber_ink_ring_clears_its_label() {
    for ppp in PPPS {
        ring_clears_label(ppp);
    }
}

fn ring_clears_label(ppp: f32) {
    use egui::epaint::Shape;
    let h = tall_gallery(ppp);
    let mut rings = Vec::new();
    for c in &h.output().shapes {
        if let Shape::Rect(r) = &c.shape
            && r.stroke.color == theme::AMBER_INK
        {
            // StrokeKind::Outside: the ring's outer edge.
            rings.push(r.rect.expand(r.stroke.width));
        }
    }
    assert_eq!(rings.len(), 2, "two ring samples expected");
    let labels: Vec<_> = text_rects(&h)
        .into_iter()
        .filter(|(t, _)| t == "parchment" || t == "parchment-header")
        .collect();
    // The two ring labels plus the two colour-swatch names of the same
    // tokens elsewhere on the page; every one must clear every ring.
    assert_eq!(
        labels.len(),
        4,
        "ring and swatch labels expected: {labels:?}"
    );
    for ring in &rings {
        for (t, r) in &labels {
            assert!(
                !ring.intersects(*r),
                "ppp {ppp}: ring {ring:?} covers label `{t}` {r:?}"
            );
        }
    }
}
