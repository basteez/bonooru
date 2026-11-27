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

mod common;

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
        .build_eframe(|cc| App::with_link(cc, Mode::Main, common::fake_link(&cc.egui_ctx)));
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
    // What is painted (not the pure `regions()` of the same rect, which is
    // ppp-independent by construction), including the fractional Windows
    // scales 1.25 and 1.75 where 6 pt = 7.5 px / 10.5 px.
    let at = |ppp| {
        let h = harness(1280.0, 800.0, ppp);
        assert_eq!(h.ctx.pixels_per_point(), ppp);
        filled_rects(&h)
    };
    let base = at(1.0);
    assert!(
        base.len() >= 7,
        "frame + 6 region fills expected, got {}",
        base.len()
    );
    for ppp in [1.25, 1.5, 1.75, 2.0] {
        assert_eq!(at(ppp), base, "painted rects at ppp {ppp}");
    }
}

/// UX-DR4 (Story 4.3 AC 9): the note-view card shows its four corner
/// brackets in `bracket`, one set per corner, inside the card.
#[test]
fn g1_note_card_has_corner_brackets() {
    for (w, hgt) in [(1024.0, 680.0), (1280.0, 800.0)] {
        for ppp in [1.0, 1.25] {
            let h = harness(w, hgt, ppp);
            let card = regions(h.ctx.content_rect()).note_card;
            let flat: Vec<_> = common::shapes_flat(&h.output().shapes)
                .into_iter()
                .filter(|f| common::uses(&f.shape, theme::BRACKET))
                .collect();
            for f in &flat {
                assert!(
                    common::inside(f.shape.visual_bounding_rect(), card, 0.01),
                    "bracket outside the card at {w}x{hgt}"
                );
            }
            common::check_brackets(&flat, card, &format!("{w}x{hgt} ppp {ppp}"));
        }
    }
}

/// Story 4.6 AC 5, moved by Story 4.8 AC 1, 11: with a firmware block the
/// gamepad region shows the inline hint in `typography.small` (12.5 pt)
/// `text-muted`, 10 pt inside its left and right edges, 8 pt under the
/// gamepad legend, as one `Role::Label`; without the block no hint is drawn.
#[test]
fn g1_gamepad_hint_geometry() {
    use egui::Role;
    use egui_kittest::kittest::{NodeT as _, Queryable as _};
    use ocb_app::app::WorkerHealth;
    use ocb_app::gating::{Block, Gate, HINT_FIRMWARE};
    use ocb_app::leds;
    use ocb_app::ui::main_screen;

    /// The main screen alone; the state is the rect it was drawn over.
    fn screen(w: f32, hgt: f32, ppp: f32, gate: Gate) -> Harness<'static, Rect> {
        let mut h = Harness::builder()
            .with_size(vec2(w, hgt))
            .with_pixels_per_point(ppp)
            .with_context_setup(ocb_app::setup)
            .build_ui_state(
                move |ui, drawn: &mut Rect| {
                    *drawn = ui.max_rect();
                    let l = leds::evaluate(None, 0, WorkerHealth::Running);
                    main_screen::show(ui, &l, &gate, &ocb_app::ui::gamepad::PadView::default());
                },
                Rect::NOTHING,
            );
        h.run();
        h
    }
    use ocb_core::protocol::consts::PROTO_VERSION;
    let block = Some(Block::FirmwareUpdate {
        controller_proto: PROTO_VERSION.wrapping_add(1),
        app_proto: PROTO_VERSION,
    });
    let blocked = Gate {
        gamepad: block,
        play: block,
    };
    for (w, hgt) in [(1024.0, 680.0), (1280.0, 800.0)] {
        for ppp in [1.0, 2.0] {
            let ctx = format!("{w}x{hgt} ppp {ppp}");
            let h = screen(w, hgt, ppp, blocked);
            let pad = regions(*h.state()).gamepad;
            let hints: Vec<_> = common::shapes_flat(&h.output().shapes)
                .into_iter()
                .filter_map(|f| match f.shape {
                    Shape::Text(t) if t.galley.text() == HINT_FIRMWARE => Some(t),
                    _ => None,
                })
                .collect();
            assert_eq!(hints.len(), 1, "{ctx}: one hint");
            let t = &hints[0];
            let ink = t.galley.rect.translate(t.pos.to_vec2());
            let format = &t.galley.job.sections[0].format;
            assert_eq!(format.font_id.size, 12.5, "{ctx}");
            assert_eq!(format.color, theme::TEXT_MUTED, "{ctx}");
            assert!(
                common::inside(ink, pad.shrink(10.0), 0.5),
                "{ctx}: {ink:?} not inside {pad:?} shrunk by 10"
            );
            // The drawn text, not only the helper: left at region + 10,
            // wrapped to the region width − 20.
            assert!(
                (ink.left() - (pad.left() + 10.0)).abs() <= 0.5,
                "{ctx}: left {} vs {}",
                ink.left(),
                pad.left() + 10.0
            );
            assert!(
                (t.galley.job.wrap.max_width - (pad.width() - 20.0)).abs() <= 0.5,
                "{ctx}: wrap width {} vs {}",
                t.galley.job.wrap.max_width,
                pad.width() - 20.0
            );
            let hint = main_screen::hint_rect(&h.ctx, pad);
            assert!(
                (ink.top() - hint.top()).abs() <= 0.5,
                "{ctx}: top {} vs {}",
                ink.top(),
                hint.top()
            );
            let lay = ocb_app::ui::gamepad::layout(
                pad,
                ocb_app::ui::gamepad::TextHeights::measure(&h.ctx),
            );
            assert!(
                (hint.top() - (lay.legend_rows[5].bottom() + 8.0)).abs() <= 0.5,
                "{ctx}: 8 pt under the legend"
            );
            // One label node, not focusable (no Focus action).
            let node = h.get_by_role_and_label(Role::Label, HINT_FIRMWARE);
            assert!(
                !node
                    .accesskit_node()
                    .data()
                    .supports_action(egui::accesskit::Action::Focus),
                "{ctx}: the hint advertises focus"
            );

            let free = screen(w, hgt, ppp, Gate::default());
            assert!(
                !common::shapes_flat(&free.output().shapes).iter().any(
                    |f| matches!(&f.shape, Shape::Text(t) if t.galley.text() == HINT_FIRMWARE)
                ),
                "{ctx}: hint without a block"
            );
            assert!(free.query_by_label(HINT_FIRMWARE).is_none(), "{ctx}");
        }
    }
}
