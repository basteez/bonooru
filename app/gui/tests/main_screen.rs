//! G.1: the main screen fills its regions in reading order with the right
//! colours, and the region rects in points do not depend on
//! `pixels_per_point` (AC 6). Checked on the tessellation input
//! (`FullOutput::shapes`), CPU only (Story 4.1 D6).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use egui::epaint::{CornerRadius, Shape};
use egui::{Color32, Rect, Theme, vec2};
use egui_kittest::Harness;
use ocb_app::app::{App, Mode};
use ocb_app::ui::layout::{Regions, regions};
use ocb_app::ui::theme;

fn harness(w: f32, h: f32, ppp: f32) -> Harness<'static, App> {
    let mut h = Harness::builder()
        .with_size(vec2(w, h))
        .with_pixels_per_point(ppp)
        .with_theme(Theme::Light)
        .build_eframe(|cc| App::new(cc, Mode::Main));
    h.run();
    h
}

/// Every filled rect painted in the last frame: (rect, fill, radius).
fn filled_rects(h: &Harness<'_, App>) -> Vec<(Rect, Color32, CornerRadius)> {
    fn walk(shape: &Shape, out: &mut Vec<(Rect, Color32, CornerRadius)>) {
        match shape {
            Shape::Rect(r) if r.fill != Color32::TRANSPARENT => {
                out.push((r.rect, r.fill, r.corner_radius));
            }
            Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for clipped in &h.output().shapes {
        walk(&clipped.shape, &mut out);
    }
    out
}

fn expected(r: &Regions) -> Vec<(&'static str, Rect, Color32, u8)> {
    vec![
        ("status", r.status, theme::PANEL_RAISED, 0),
        ("song", r.song, theme::PANEL, 0),
        ("note view", r.note_view, theme::FRAME, 0),
        ("card", r.note_card, theme::PARCHMENT, 12),
        ("gamepad", r.gamepad, theme::PANEL, 0),
        ("bottom", r.bottom, theme::PANEL_RAISED, 0),
    ]
}

#[test]
fn g1_regions_are_filled_in_reading_order_with_their_colours() {
    for (w, hgt) in [(1024.0, 680.0), (1280.0, 800.0)] {
        let h = harness(w, hgt, 1.0);
        let r = regions(h.ctx.content_rect());
        assert_eq!(h.ctx.content_rect().size(), vec2(w, hgt));
        let rects = filled_rects(&h);
        // Paint order of the region fills = reading order.
        let mut last = 0;
        for (name, rect, fill, radius) in expected(&r) {
            let pos = rects
                .iter()
                .position(|(rr, f, cr)| {
                    *rr == rect && *f == fill && *cr == CornerRadius::same(radius)
                })
                .unwrap_or_else(|| {
                    panic!("{name}: no fill {fill:?} r{radius} at {rect:?} ({w}x{hgt})")
                });
            assert!(
                pos >= last,
                "{name} painted out of reading order ({w}x{hgt})"
            );
            last = pos;
        }
    }
}

#[test]
fn g1_rects_in_points_do_not_depend_on_ppp() {
    let at = |ppp| {
        let h = harness(1280.0, 800.0, ppp);
        assert_eq!(h.ctx.pixels_per_point(), ppp);
        (regions(h.ctx.content_rect()), filled_rects(&h))
    };
    let (r1, s1) = at(1.0);
    for ppp in [1.5, 2.0] {
        let (r, s) = at(ppp);
        assert_eq!(r, r1, "regions at ppp {ppp}");
        assert_eq!(s, s1, "painted rects at ppp {ppp}");
    }
}
