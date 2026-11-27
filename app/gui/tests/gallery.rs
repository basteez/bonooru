//! G.3 (gallery part): the hidden debug gallery renders without a missing
//! glyph (egui_kittest fails on one), shows every colour token and every
//! typography token by its exact name, never overlaps two texts at five
//! scales, and every focusable node in it (sample dialogs included) shows
//! its focus ring when focused (Story 4.3 AC 1, AC 13).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use egui::{Theme, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use ocb_app::app::{App, Mode};
use ocb_app::ui::theme;

#[test]
fn g3_gallery_lists_every_colour_and_typography_token() {
    // At 1024x680 the scroll area clips the rows below the fold, but clipped
    // widgets stay in the AccessKit tree, so every row is queryable. At the
    // five scales of AC 13.
    for ppp in PPPS {
        let mut h = Harness::builder()
            .with_size(vec2(1024.0, 680.0))
            .with_pixels_per_point(ppp)
            .with_theme(Theme::Light)
            .build_eframe(|cc| App::with_link(cc, Mode::Gallery, common::fake_link(&cc.egui_ctx)));
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
                "ppp {ppp}: gallery must name `{name}` exactly once"
            );
        }
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
/// are not painted): `last_text_is_painted` checks it.
fn tall_gallery(ppp: f32) -> Harness<'static, App> {
    let mut h = Harness::builder()
        .with_size(vec2(1024.0, 7000.0))
        .with_pixels_per_point(ppp)
        .build_eframe(|cc| App::with_link(cc, Mode::Gallery, common::fake_link(&cc.egui_ctx)));
    h.run();
    h
}

/// The last section's last text, painted and above the window bottom.
fn last_text_is_painted(h: &Harness<'_, App>, ppp: f32) {
    let last = text_rects(h)
        .into_iter()
        .find(|(t, _)| t == "Disabled sample")
        .unwrap_or_else(|| panic!("ppp {ppp}: the last section is not painted"));
    assert!(
        last.1.bottom() < h.ctx.content_rect().bottom() - 24.0,
        "ppp {ppp}: the gallery runs past the harness: {:?}",
        last.1
    );
}

/// No two texts of the gallery overlap, ink included (owner review
/// 2026-11-27: the countdown "3" ran into the next typography row).
#[test]
fn g3_gallery_texts_never_overlap() {
    for ppp in PPPS {
        texts_never_overlap(ppp);
    }
}

/// The owner's macOS scale (2.0) and the Windows scales, fractional
/// included (Story 4.1 review: two bugs showed only at 1.25 and 1.75).
const PPPS: [f32; 5] = [1.0, 1.25, 1.5, 1.75, 2.0];

fn texts_never_overlap(ppp: f32) {
    let h = tall_gallery(ppp);
    last_text_is_painted(&h, ppp);
    let texts = text_rects(&h);
    assert!(texts.len() > 120, "too few texts: {}", texts.len());
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

/// The focus ring around `node`'s rect, if painted: a 2 pt `amber` or
/// `amber-ink` outside stroke at 2 pt offset.
fn ring_around(h: &Harness<'_, App>, node_rect: egui::Rect) -> bool {
    use egui::epaint::{Shape, StrokeKind};
    fn walk(shape: &Shape, want: egui::Rect, found: &mut bool) {
        match shape {
            Shape::Rect(r)
                if (r.stroke.color == theme::AMBER || r.stroke.color == theme::AMBER_INK)
                    && r.stroke.width == theme::spacing::FOCUS_RING_WIDTH
                    && r.stroke_kind == StrokeKind::Outside =>
            {
                let d = (r.rect.min - want.min).length() + (r.rect.max - want.max).length();
                if d < 0.05 {
                    *found = true;
                }
            }
            Shape::Vec(v) => v.iter().for_each(|s| walk(s, want, found)),
            _ => {}
        }
    }
    let want = node_rect.expand(theme::spacing::FOCUS_RING_OFFSET);
    let mut found = false;
    for c in &h.output().shapes {
        walk(&c.shape, want, &mut found);
    }
    found
}

/// Focuses every node under `scope` (or the whole tree) that supports
/// `Action::Focus`, and checks that its ring shows. Returns how many.
fn rings_for_focusable_nodes(h: &mut Harness<'_, App>, dialog: Option<&str>) -> usize {
    use egui::accesskit::{Action, Role};
    let ids: Vec<(Role, String)> = {
        let nodes: Vec<_> = match dialog {
            Some(title) => h
                .get_by_role_and_label(Role::Dialog, title)
                .children_recursive()
                .collect(),
            None => h.root().children_recursive().collect(),
        };
        nodes
            .into_iter()
            // egui advertises Focus on a disabled widget too, but never
            // gives it focus: only enabled nodes can show a ring.
            .filter(|n| {
                n.accesskit_node().data().supports_action(Action::Focus)
                    && !n.accesskit_node().is_disabled()
            })
            .map(|n| {
                (
                    n.accesskit_node().role(),
                    n.accesskit_node().label().unwrap_or_default(),
                )
            })
            .collect()
    };
    for (role, label) in &ids {
        let node = h.get_by_role_and_label(*role, label);
        node.focus();
        h.run();
        let node = h.get_by_role_and_label(*role, label);
        assert!(node.is_focused(), "`{label}` did not take focus");
        assert!(
            ring_around(h, node.rect()),
            "`{label}` shows no focus ring at {:?}",
            node.rect()
        );
    }
    ids.len()
}

#[test]
fn g3_every_focusable_node_shows_its_ring() {
    for ppp in [1.0, 1.25] {
        let mut h = tall_gallery(ppp);
        let page = rings_for_focusable_nodes(&mut h, None);
        // Two dark samples, the parchment sample, two dialog openers.
        assert_eq!(page, 5, "ppp {ppp}: focusable nodes on the page");
        for (opener, title, buttons) in [
            ("Open readiness dialog", "Ready to play?", 2),
            ("Open update dialog", "Firmware update needed", 1),
        ] {
            h.get_by_label(opener).click();
            h.run();
            assert_eq!(
                rings_for_focusable_nodes(&mut h, Some(title)),
                buttons,
                "ppp {ppp}: {title}"
            );
            h.key_press(egui::Key::Escape);
            h.run();
            assert!(!h.state().modals().is_open(), "Esc closes {title}");
        }
    }
}

/// The orb glow sample sits on the orb's own size (`fairy-orb-icon.size` =
/// `spacing.orb-icon-size`), a circle (radius half the side), as AC 13 asks
/// ("glow at each blur token on its control shape").
#[test]
fn g3_orb_glow_sample_has_the_orb_size() {
    use egui::epaint::Shape;
    let h = tall_gallery(1.0);
    let orb = theme::spacing::ORB_ICON_SIZE;
    // The orb's control: an `on-ember` fill with a blurred `amber` glow
    // centred on it (the colour swatch of `on-ember` has no glow).
    let rects: Vec<_> = h
        .output()
        .shapes
        .iter()
        .filter_map(|c| match &c.shape {
            Shape::Rect(r) => Some(r.clone()),
            _ => None,
        })
        .collect();
    let found: Vec<_> = rects
        .iter()
        .filter(|r| r.fill == theme::ON_EMBER)
        .filter(|r| {
            rects.iter().any(|g| {
                g.blur_width > 0.0
                    && g.fill == theme::AMBER
                    && (g.rect.center() - r.rect.center()).length() < 0.01
            })
        })
        .map(|r| (r.rect.size(), r.corner_radius))
        .collect();
    assert_eq!(found.len(), 1, "one orb sample: {found:?}");
    assert_eq!(found[0].0, vec2(orb, orb), "orb sample size");
    assert_eq!(found[0].1, theme::corner(orb / 2.0), "orb sample radius");
}
