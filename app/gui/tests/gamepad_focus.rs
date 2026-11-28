//! G.24 (Story 4.9 AC 3, 4, 9, 10): the gamepad panel's focused look (inset
//! amber ring, the stacked wash, the "Keys active" chip) at both widths, the
//! compact variant below 1280 pt (its pure layout and geometry, drawn
//! inside the 200 pt region, buttons only, key hints in tooltips), the
//! panel's `Role::Group` AccessKit node as the parent of every panel node,
//! the wording check of the new strings and the egui ↔ pad key table. The
//! panel is drawn alone over the gamepad region of a whole window (as G.22),
//! its focus requested before it is drawn when a check needs it focused.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use egui::accesskit::{Action, Role};
use egui::epaint::Shape;
use egui::{Event, Key, Pos2, Rect, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use ocb_app::gamepad::{CAPTION, CHIP_DPAD, Control, LEGEND, MOUSE, PadKey, Stick, TAB, Target};
use ocb_app::gating::Block;
use ocb_app::leds::banned_word;
use ocb_app::ui::gamepad::{
    self, CHIP_KEYS_ACTIVE, ControlShape, GROUP_KEYS_ACTIVE, GROUP_KEYS_OFF, PadGeometry, PadView,
    TextHeights, Variant, legend_row_label, panel_id, wash_colours,
};
use ocb_app::ui::layout::regions;
use ocb_app::ui::{pad_keys, theme};

/// The compact windows (content narrower than 1280 pt).
const COMPACT_SIZES: [(f32, f32); 2] = [(1024.0, 680.0), (1279.0, 800.0)];
/// One window of each variant.
const BOTH: [(f32, f32); 2] = [(1280.0, 800.0), (1024.0, 680.0)];
const PPPS: [f32; 2] = [1.0, 2.0];

fn region_of(w: f32, h: f32) -> Rect {
    regions(Rect::from_min_size(Pos2::ZERO, vec2(w, h))).gamepad
}

/// The panel alone over the gamepad region of a `w × h` window, focused or
/// not; the state is the region.
fn panel(w: f32, h: f32, ppp: f32, view: PadView, focused: bool) -> Harness<'static, Rect> {
    let mut hn = Harness::builder()
        .with_size(vec2(w, h))
        .with_pixels_per_point(ppp)
        .with_context_setup(ocb_app::setup)
        .build_ui_state(
            move |ui, region: &mut Rect| {
                *region = region_of(w, h);
                if focused {
                    ui.ctx().memory_mut(|m| m.request_focus(panel_id()));
                }
                let _ = gamepad::show(ui, *region, &view);
            },
            Rect::NOTHING,
        );
    hn.run();
    hn
}

fn views() -> Vec<(String, PadView)> {
    let mut v = vec![("open".to_owned(), PadView::default())];
    for b in Block::samples() {
        v.push((
            b.as_str().to_owned(),
            PadView {
                enabled: false,
                hint: Some(b.gamepad_hint()),
                ..PadView::default()
            },
        ));
    }
    v
}

/// The header boxes (tab, chip, underline) of the region's variant.
fn header(ctx: &egui::Context, region: Rect) -> (Rect, Rect, f32) {
    let text = TextHeights::measure(ctx);
    match Variant::of(region) {
        Variant::Full => {
            let l = gamepad::layout(region, text);
            (l.tab, l.chip, l.underline_y)
        }
        Variant::Compact => {
            let l = gamepad::compact_layout(region, text);
            (l.tab, l.chip, l.underline_y)
        }
    }
}

/// AC 3: only while the panel has focus, at both widths and densities: one
/// inset `amber` ring on the region's edge (2 pt, at least one pixel), six
/// inset wash strokes in `amber-wash` fading inward, then the chip ("Keys
/// active", `ink` on an `amber` pill) right-aligned in the header, its
/// bottom 3 pt above the underline, disjoint from the tab; all painted
/// before the panel content. Without focus: none of these.
#[test]
fn g24_focused_look() {
    for (w, h) in BOTH {
        for ppp in PPPS {
            let ctx_s = format!("{w}x{h} ppp {ppp}");
            let hn = panel(w, h, ppp, PadView::default(), true);
            let region = *hn.state();
            let flat = common::shapes_flat(&hn.output().shapes);
            let rings: Vec<_> = flat
                .iter()
                .filter_map(|f| match &f.shape {
                    Shape::Rect(r) if r.stroke.color == theme::AMBER => Some((f.index, r.clone())),
                    _ => None,
                })
                .collect();
            assert_eq!(rings.len(), 1, "{ctx_s}: one amber ring");
            let (ring_at, ring) = &rings[0];
            assert!(common::rect_eq(ring.rect, region, 0.01), "{ctx_s}");
            assert_eq!(ring.stroke_kind, egui::StrokeKind::Inside, "{ctx_s}");
            assert!(
                (ring.stroke.width - 2.0_f32.max(1.0 / ppp)).abs() < 1e-4,
                "{ctx_s}: {}",
                ring.stroke.width
            );
            let wash = wash_colours();
            assert_eq!(wash.len(), 6);
            for (i, c) in wash.iter().enumerate() {
                #[allow(clippy::cast_precision_loss)]
                let k = (6 - i) as f32 / 6.0;
                assert_eq!(*c, theme::AMBER_WASH.gamma_multiply(k), "wash {i}");
                let strokes: Vec<_> = flat
                    .iter()
                    .filter_map(|f| match &f.shape {
                        Shape::Rect(r) if r.stroke.color == *c && r.stroke.width > 0.0 => {
                            Some((f.index, r.clone()))
                        }
                        _ => None,
                    })
                    .collect();
                assert_eq!(strokes.len(), 1, "{ctx_s}: wash stroke {i}");
                let (at, s) = &strokes[0];
                #[allow(clippy::cast_precision_loss)]
                let inset = 2.0 + 3.0 * i as f32;
                assert!(
                    common::rect_eq(s.rect, region.shrink(inset), 0.01),
                    "{ctx_s}: {i}"
                );
                assert_eq!(s.stroke_kind, egui::StrokeKind::Inside);
                assert!((s.stroke.width - 3.0).abs() < 1e-4);
                assert!(at < ring_at, "{ctx_s}: the wash under the ring");
            }
            // The chip.
            let (tab, chip_box, underline_y) = header(&hn.ctx, region);
            let pills: Vec<_> = flat
                .iter()
                .filter_map(|f| match &f.shape {
                    Shape::Rect(r) if r.fill == theme::AMBER => Some(r.clone()),
                    _ => None,
                })
                .collect();
            assert_eq!(pills.len(), 1, "{ctx_s}: one chip");
            let chip = pills[0].rect;
            assert!(common::rect_eq(chip, chip_box, 0.01), "{ctx_s}");
            assert_eq!(
                pills[0].corner_radius,
                theme::corner(theme::rounded::FULL),
                "{ctx_s}: rounded.full"
            );
            assert!(common::inside(chip, region, 0.01), "{ctx_s}");
            assert!(
                !chip.intersects(tab),
                "{ctx_s}: chip {chip:?} ∩ tab {tab:?}"
            );
            assert!(
                (chip.bottom() - (underline_y - 0.5 - 3.0)).abs() <= 0.5,
                "{ctx_s}: chip bottom {} vs underline {underline_y}",
                chip.bottom()
            );
            assert!(
                (chip.right() - (region.right() - 10.0)).abs() <= 0.5,
                "{ctx_s}: right-aligned"
            );
            let texts = common::texts(&flat);
            let (_, ink, _) = texts
                .iter()
                .find(|(t, _, _)| t == CHIP_KEYS_ACTIVE)
                .unwrap_or_else(|| panic!("{ctx_s}: no chip text"));
            assert!(
                common::inside(*ink, chip, 0.5),
                "{ctx_s}: {ink:?} in {chip:?}"
            );
            let style = flat
                .iter()
                .find_map(|f| match &f.shape {
                    Shape::Text(t) if t.galley.text() == CHIP_KEYS_ACTIVE => {
                        Some(t.galley.job.sections[0].format.clone())
                    }
                    _ => None,
                })
                .unwrap();
            assert_eq!(style.font_id.size, 11.5, "{ctx_s}");
            assert_eq!(style.color, theme::INK, "{ctx_s}");
            assert_eq!(theme::CHIP.weight, 700.0);
            // The look is under the content: the ring before the tab fill.
            let tab_at = flat
                .iter()
                .find(|f| matches!(&f.shape, Shape::Rect(r) if r.fill == theme::PARCHMENT))
                .map(|f| f.index)
                .unwrap();
            assert!(*ring_at < tab_at, "{ctx_s}: the ring under the content");
            // No node for the chip.
            assert!(hn.query_by_label(CHIP_KEYS_ACTIVE).is_none(), "{ctx_s}");

            let off = panel(w, h, ppp, PadView::default(), false);
            let flat = common::shapes_flat(&off.output().shapes);
            let mut amber = flat.iter().filter(|f| {
                common::uses(&f.shape, theme::AMBER)
                    || wash.iter().any(|c| common::uses(&f.shape, *c))
            });
            assert!(
                amber.next().is_none(),
                "{ctx_s}: no focused look without focus"
            );
            assert!(
                !common::texts(&flat)
                    .iter()
                    .any(|(t, _, _)| t == CHIP_KEYS_ACTIVE),
                "{ctx_s}: no chip without focus"
            );
        }
    }
}

/// AC 9 (pure): the compact layout in the 200 pt region of 1024 × 680 and
/// 1279 × 800: tab at the padding, the caption 4 pt under the underline in a
/// box two lines high, the 180 × 124 grid 6 pt under it, the hint 8 pt under
/// the grid down to the padding; everything inside, pairwise disjoint.
#[test]
fn g24_compact_layout() {
    let text = TextHeights {
        tab: vec2(61.0, gamepad::TAB_TEXT_HEIGHT),
        caption: gamepad::CAPTION_HEIGHT,
        row: gamepad::LEGEND_TEXT_HEIGHT,
        chip: vec2(62.0, gamepad::CHIP_TEXT_HEIGHT),
    };
    for (w, h) in COMPACT_SIZES {
        let region = region_of(w, h);
        let ctx = format!("{w}x{h}");
        assert_eq!(region.width(), 200.0, "{ctx}");
        assert_eq!(Variant::of(region), Variant::Compact, "{ctx}");
        let l = gamepad::compact_layout(region, text);
        let inner = region.shrink(10.0);
        assert!((l.tab.min - inner.min).length() <= 0.5, "{ctx}");
        assert!((l.underline_y - (l.tab.bottom() + 0.5)).abs() <= 0.01);
        assert_eq!(l.underline_x, (inner.left(), inner.right()));
        assert!((l.caption.top() - (l.tab.bottom() + 1.0 + 4.0)).abs() <= 0.5);
        assert!((l.caption.height() - 34.0).abs() <= 0.01, "{ctx}");
        assert!((l.caption.width() - 180.0).abs() <= 0.01, "{ctx}");
        assert!((l.grid.top() - (l.caption.bottom() + 6.0)).abs() <= 0.5);
        assert!((l.grid.left() - inner.left()).abs() <= 0.5);
        assert!((l.grid.width() - 180.0).abs() <= 0.01 && (l.grid.height() - 124.0).abs() <= 0.01);
        assert!(
            (l.hint.top() - (l.grid.bottom() + 8.0)).abs() <= 0.5,
            "{ctx}"
        );
        assert!((l.hint.bottom() - inner.bottom()).abs() <= 0.5, "{ctx}");
        assert!(l.hint.height() >= 2.0 * 17.0, "{ctx}: two hint lines fit");
        let rects = [l.tab, l.chip, l.caption, l.grid, l.hint];
        for r in rects {
            assert!(common::inside(r, inner, 0.5), "{ctx}: {r:?}");
        }
        for (i, a) in rects.iter().enumerate() {
            for b in &rects[i + 1..] {
                assert!(
                    !a.intersects(*b) || a.intersect(*b).area() <= 0.0,
                    "{ctx}: {a:?} ∩ {b:?}"
                );
            }
        }
        // `main_screen::hint_rect` and `gamepad::hint_rect` agree with it.
        assert_eq!(gamepad::hint_rect(region, text), l.hint);
    }
    // At 1280 pt and wider: the full panel.
    assert_eq!(Variant::of(region_of(1280.0, 800.0)), Variant::Full);
}

/// AC 9: every compact control at Dev Notes › Compact geometry plus the
/// grid origin (± 0.5), the same shapes and sizes as the full pad; no
/// stick wells, no chips; the hit test by circle, rect and D-pad arm, with
/// free space between the wells.
#[test]
fn g24_compact_geometry() {
    let o = pos2(830.0, 120.0);
    let g = PadGeometry::compact(o);
    let rect = |x: f32, y: f32, w: f32, h: f32, r: f32| {
        ControlShape::Rect(Rect::from_min_size(o + vec2(x, y), vec2(w, h)), r)
    };
    let circle = |x: f32, y: f32, r: f32| (o + vec2(x, y), r);
    let want_rects = [
        (Control::Zl, rect(0.0, 0.0, 42.0, 15.0, 6.0)),
        (Control::L, rect(46.0, 0.0, 42.0, 15.0, 6.0)),
        (Control::R, rect(92.0, 0.0, 42.0, 15.0, 6.0)),
        (Control::Zr, rect(138.0, 0.0, 42.0, 15.0, 6.0)),
        (Control::Capture, rect(62.0, 28.5, 11.0, 11.0, 2.0)),
        (Control::DUp, rect(38.0, 71.0, 14.0, 14.0, 2.0)),
        (Control::DDown, rect(38.0, 95.0, 14.0, 14.0, 2.0)),
        (Control::DLeft, rect(26.0, 83.0, 14.0, 14.0, 2.0)),
        (Control::DRight, rect(50.0, 83.0, 14.0, 14.0, 2.0)),
    ];
    for (c, want) in want_rects {
        let (ControlShape::Rect(a, ra), ControlShape::Rect(b, rb)) = (g.control(c), want) else {
            panic!("{c:?} is not a rect");
        };
        assert!(common::rect_eq(a, b, 0.5), "{c:?}: {a:?} vs {b:?}");
        assert_eq!(ra, rb, "{c:?}");
    }
    for (c, (centre, r)) in [
        (Control::Minus, circle(22.5, 34.0, 7.0)),
        (Control::Home, circle(112.5, 34.0, 6.5)),
        (Control::Plus, circle(157.5, 34.0, 7.0)),
        (Control::X, circle(135.0, 72.0, 10.0)),
        (Control::Y, circle(117.0, 90.0, 10.0)),
        (Control::A, circle(153.0, 90.0, 10.0)),
        (Control::B, circle(135.0, 108.0, 10.0)),
    ] {
        let ControlShape::Circle(k) = g.control(c) else {
            panic!("{c:?} is not a circle");
        };
        assert!(k.center.distance(centre) <= 0.5, "{c:?}");
        assert_eq!(k.radius, r, "{c:?}");
    }
    assert!(g.dpad_well.center.distance(o + vec2(45.0, 90.0)) <= 0.5);
    assert_eq!(g.dpad_well.radius, 22.0);
    assert!(g.abxy_well.center.distance(o + vec2(135.0, 90.0)) <= 0.5);
    assert_eq!(g.abxy_well.radius, 31.0);
    assert!(common::rect_eq(
        g.dpad_centre,
        Rect::from_min_size(o + vec2(38.0, 83.0), vec2(14.0, 14.0)),
        0.5
    ));
    assert!(g.stick_wells().is_empty(), "no sticks in compact");
    assert!(g.chips.is_empty(), "no chips in compact");
    assert!(!g.has_body);
    // The same shapes as the full pad: radii and sizes match control by
    // control.
    let full = PadGeometry::new(Pos2::ZERO);
    for c in Control::ALL {
        match (g.control(c), full.control(c)) {
            (ControlShape::Circle(a), ControlShape::Circle(b)) => {
                assert_eq!(a.radius, b.radius, "{c:?}");
            }
            (ControlShape::Rect(a, ra), ControlShape::Rect(b, rb)) => {
                assert_eq!(ra, rb, "{c:?}");
                // The shoulders are narrower (42 pt, four in a row); the
                // D-pad arms and Capture keep their size.
                if !matches!(c, Control::Zl | Control::L | Control::R | Control::Zr) {
                    assert_eq!(a.size(), b.size(), "{c:?}");
                }
            }
            _ => panic!("{c:?}: shape kind differs"),
        }
    }
    // Every control inside the grid.
    let grid = Rect::from_min_size(o, vec2(180.0, 124.0));
    for (c, s) in &g.controls {
        assert!(common::inside(s.rect(), grid, 0.01), "{c:?}");
    }
    assert!(common::inside(g.abxy_well.rect(), grid, 0.01));
    assert!(common::inside(g.dpad_well.rect(), grid, 0.01));
    // The hit test.
    assert_eq!(
        g.hit(o + vec2(153.0, 90.0)),
        Some(Target::Control(Control::A))
    );
    assert_eq!(
        g.hit(o + vec2(153.0 + 7.5, 90.0 + 7.5)),
        None,
        "outside A's circle, inside its square"
    );
    assert_eq!(g.hit(o + vec2(90.0, 90.0)), None, "between the wells");
    assert_eq!(g.hit(o + vec2(45.0, 90.0)), None, "the D-pad centre");
    assert_eq!(
        g.hit(o + vec2(45.0, 75.0)),
        Some(Target::Control(Control::DUp))
    );
    assert_eq!(
        g.hit(o + vec2(65.0, 1.0)),
        Some(Target::Control(Control::L))
    );
}

/// AC 9: the compact panel at 1024 × 680 and 1279 × 800, ppp 1 and 2, open
/// and for every block, focused and not: every shape and text's ink inside
/// the 200 pt region; the caption in at most two lines inside its box; the
/// hint's text inside the hint box.
#[test]
fn g24_compact_drawn_inside() {
    for (w, h) in COMPACT_SIZES {
        for ppp in PPPS {
            for focused in [false, true] {
                for (name, view) in views() {
                    let hn = panel(w, h, ppp, view, focused);
                    let region = *hn.state();
                    let ctx = format!("{w}x{h} ppp {ppp} {name} focused {focused}");
                    let flat = common::shapes_flat(&hn.output().shapes);
                    assert!(flat.len() > 40, "{ctx}: {}", flat.len());
                    for f in &flat {
                        let b = f.shape.visual_bounding_rect();
                        let harness_bg =
                            matches!(&f.shape, Shape::Rect(r) if r.fill == theme::FRAME);
                        if b.is_positive() && !harness_bg {
                            assert!(common::inside(b, region, 0.5), "{ctx}: {b:?} {:?}", f.shape);
                        }
                    }
                    let lay = gamepad::compact_layout(region, TextHeights::measure(&hn.ctx));
                    for (t, ink, _) in common::texts(&flat) {
                        assert!(common::inside(ink, region, 0.5), "{ctx}: {t} {ink:?}");
                        if t == CAPTION {
                            assert!(common::inside(ink, lay.caption, 0.5), "{ctx}: caption");
                        }
                        if Some(t.as_str()) == view.hint {
                            assert!(common::inside(ink, lay.hint, 0.5), "{ctx}: hint");
                        }
                    }
                    let rows = flat
                        .iter()
                        .find_map(|f| match &f.shape {
                            Shape::Text(t) if t.galley.text() == CAPTION => {
                                Some(t.galley.rows.len())
                            }
                            _ => None,
                        })
                        .unwrap_or_else(|| panic!("{ctx}: no caption"));
                    assert!((1..=2).contains(&rows), "{ctx}: {rows} caption lines");
                }
            }
        }
    }
}

/// AC 9: the compact panel shows buttons only: no body (no mesh, no 1.5 pt
/// outline), no stick wells or knobs, no key chips, no legend, no "mouse";
/// 16 `Button` nodes and no `Slider`.
#[test]
fn g24_compact_contents() {
    for ppp in PPPS {
        let hn = panel(1024.0, 680.0, ppp, PadView::default(), false);
        let flat = common::shapes_flat(&hn.output().shapes);
        assert!(
            !flat.iter().any(|f| matches!(f.shape, Shape::Mesh(_))),
            "no body fill"
        );
        assert!(
            !flat.iter().any(|f| matches!(&f.shape, Shape::Path(p)
                if (p.stroke.width - theme::components::gamepad_body::BORDER.max(1.0 / ppp)).abs() < 1e-4
                    && p.closed)),
            "no body outline"
        );
        assert!(
            !flat.iter().any(
                |f| matches!(&f.shape, Shape::Circle(c) if (c.radius - 17.0).abs() < 0.01
                    || (c.radius - 11.0).abs() < 0.01)
            ),
            "no stick wells or knobs"
        );
        assert!(
            !flat.iter().any(|f| matches!(&f.shape, Shape::Rect(r)
                if r.fill == theme::components::gamepad_key_chip::FILL
                    && r.corner_radius == theme::corner(theme::components::gamepad_key_chip::RADIUS))),
            "no key chips"
        );
        let texts: Vec<String> = common::texts(&flat).into_iter().map(|t| t.0).collect();
        for t in ["Z", "Q", "E", "C", "K", "H", "J", CHIP_DPAD, MOUSE] {
            assert!(!texts.iter().any(|x| x == t), "{t} drawn");
        }
        for (n, k) in LEGEND {
            assert!(!texts.iter().any(|x| x == n || x == k), "legend {n}");
            assert!(hn.query_by_label(&legend_row_label(n, k)).is_none());
        }
        assert_eq!(hn.query_all_by_role(Role::Button).count(), 16);
        assert_eq!(hn.query_all_by_role(Role::Slider).count(), 0);
        let _ = hn.get_by_role_and_label(Role::Label, TAB);
        let _ = hn.get_by_role_and_label(Role::Label, CAPTION);
        // The letters, arrows and symbols are drawn on their controls.
        for c in [
            Control::Zl,
            Control::L,
            Control::R,
            Control::Zr,
            Control::X,
            Control::Y,
            Control::A,
            Control::B,
        ] {
            assert!(texts.iter().any(|x| x == c.a11y_name()), "{c:?} letter");
        }
    }
}

/// The text of the tooltip shown after hovering `p` (frames until it
/// shows, bounded).
fn tooltip_after_hover(hn: &mut Harness<'static, Rect>, p: Pos2) -> Option<(String, Rect)> {
    hn.input_mut().events.push(Event::PointerMoved(p));
    for _ in 0..12 {
        hn.step();
        let flat = common::shapes_flat(&hn.output().shapes);
        let frame = flat.iter().find_map(|f| match &f.shape {
            Shape::Rect(r)
                if r.fill == theme::tooltip_frame().fill
                    && r.stroke == theme::tooltip_frame().stroke =>
            {
                Some((r.rect, f.index))
            }
            _ => None,
        });
        if let Some((frame, at)) = frame {
            // The tooltip's own text: painted after its frame, inside it
            // (control letters under the tooltip are painted before).
            let t = common::texts(&flat)
                .into_iter()
                .find(|(_, ink, i)| *i > at && frame.contains_rect(*ink))
                .map(|(t, _, _)| t)?;
            return Some((t, frame));
        }
    }
    None
}

/// AC 9: hovering each compact control past egui's tooltip delay shows one
/// tooltip in `theme::tooltip_frame()` with the control's key hint (the
/// Copy table, e.g. "A: L key"); the full panel shows none.
#[test]
fn g24_compact_tooltips() {
    let mut hn = panel(1024.0, 680.0, 1.0, PadView::default(), false);
    let region = *hn.state();
    let lay = gamepad::compact_layout(region, TextHeights::measure(&hn.ctx));
    let geo = PadGeometry::compact(lay.grid.min);
    let a = geo.control(Control::A).rect().center();
    let (text, _) = tooltip_after_hover(&mut hn, a).expect("a tooltip on A");
    assert_eq!(text, "A: L key");
    for c in Control::ALL {
        // Away first, so each tooltip starts fresh.
        hn.input_mut()
            .events
            .push(Event::PointerMoved(pos2(5.0, 5.0)));
        hn.step();
        hn.step();
        let p = geo.control(c).rect().center();
        let (text, _) =
            tooltip_after_hover(&mut hn, p).unwrap_or_else(|| panic!("no tooltip on {c:?}"));
        assert_eq!(text, c.key_hint(), "{c:?}");
    }
    let mut full = panel(1280.0, 800.0, 1.0, PadView::default(), false);
    let region = *full.state();
    let lay = gamepad::layout(region, TextHeights::measure(&full.ctx));
    let a = PadGeometry::new(lay.pad.min)
        .control(Control::A)
        .rect()
        .center();
    assert!(
        tooltip_after_hover(&mut full, a).is_none(),
        "the full pad has its chips"
    );
}

/// The nearest ancestor of `n` that is not a plain container `Ui` node.
fn named_ancestor<'t>(n: &egui_kittest::Node<'t>) -> Option<egui_kittest::Node<'t>> {
    let mut p = n.parent();
    while let Some(q) = p {
        if q.accesskit_node().role() != Role::GenericContainer {
            return Some(q);
        }
        p = q.parent();
    }
    None
}

/// AC 10: one `Role::Group` node named "Gamepad – keys off" without focus
/// and "Gamepad – keys active" with it, focusable; every `Button`, `Slider`
/// and `Label` of the panel has it as its nearest non-container ancestor;
/// no control is focusable; the compact panel has 16 `Button`s and no
/// `Slider`.
#[test]
fn g24_accesskit_group() {
    for (w, h) in BOTH {
        for focused in [false, true] {
            let mut hn = panel(w, h, 1.0, PadView::default(), focused);
            // The name follows the focus of the frame it is drawn in.
            hn.run();
            let ctx = format!("{w}x{h} focused {focused}");
            let name = if focused {
                GROUP_KEYS_ACTIVE
            } else {
                GROUP_KEYS_OFF
            };
            let groups: Vec<_> = hn.query_all_by_role(Role::Group).collect();
            assert_eq!(groups.len(), 1, "{ctx}");
            let group = &groups[0];
            assert_eq!(
                group.accesskit_node().label().as_deref(),
                Some(name),
                "{ctx}"
            );
            assert!(
                group.accesskit_node().data().supports_action(Action::Focus),
                "{ctx}"
            );
            let group_id = group.accesskit_node().id();
            let mut members = 0;
            for role in [Role::Button, Role::Slider, Role::Label] {
                for n in hn.query_all_by_role(role) {
                    let parent = named_ancestor(&n)
                        .unwrap_or_else(|| panic!("{ctx}: {role:?} without a parent"));
                    assert_eq!(
                        parent.accesskit_node().id(),
                        group_id,
                        "{ctx}: {role:?} {:?} not under the group",
                        n.accesskit_node().label()
                    );
                    members += 1;
                    if role != Role::Label {
                        assert!(
                            !n.accesskit_node().data().supports_action(Action::Focus),
                            "{ctx}: a control is focusable"
                        );
                    }
                }
            }
            let compact = Variant::of(*hn.state()) == Variant::Compact;
            assert_eq!(hn.query_all_by_role(Role::Button).count(), 16, "{ctx}");
            assert_eq!(
                hn.query_all_by_role(Role::Slider).count(),
                if compact { 0 } else { 2 },
                "{ctx}"
            );
            // The controls, the tab and caption labels, plus the sticks and
            // the six legend rows in full.
            let want = if compact { 16 + 2 } else { 18 + 2 + 6 };
            assert!(members >= want, "{ctx}: {members}");
        }
    }
    let _ = Stick::ALL;
}

/// AC 3, 9, 10: no banned word in the chip, the group names or any compact
/// tooltip.
#[test]
fn g24_no_online_wording() {
    let mut strings = vec![
        CHIP_KEYS_ACTIVE.to_owned(),
        GROUP_KEYS_ACTIVE.to_owned(),
        GROUP_KEYS_OFF.to_owned(),
    ];
    strings.extend(Control::ALL.iter().map(|c| c.key_hint()));
    for s in &strings {
        assert_eq!(banned_word(s), None, "{s}");
    }
    assert!(GROUP_KEYS_ACTIVE.contains('\u{2013}'), "the en dash");
}

/// AC 4: the egui ↔ pad key table: every mapped physical key gives its
/// `PadKey`, and the 18 keys are all reached; unmapped keys give none.
#[test]
fn g24_pad_keys_table() {
    let table = [
        (Key::ArrowUp, PadKey::ArrowUp),
        (Key::ArrowDown, PadKey::ArrowDown),
        (Key::ArrowLeft, PadKey::ArrowLeft),
        (Key::ArrowRight, PadKey::ArrowRight),
        (Key::W, PadKey::W),
        (Key::A, PadKey::A),
        (Key::S, PadKey::S),
        (Key::D, PadKey::D),
        (Key::H, PadKey::H),
        (Key::J, PadKey::J),
        (Key::K, PadKey::K),
        (Key::L, PadKey::L),
        (Key::Q, PadKey::Q),
        (Key::E, PadKey::E),
        (Key::Z, PadKey::Z),
        (Key::C, PadKey::C),
        (Key::Enter, PadKey::Enter),
        (Key::Backspace, PadKey::Backspace),
    ];
    for (k, p) in table {
        assert_eq!(pad_keys::pad_key(k), Some(p), "{k:?}");
    }
    for p in PadKey::ALL {
        assert!(table.iter().any(|(_, q)| *q == p), "{p:?} unreachable");
    }
    for k in [
        Key::Space,
        Key::Tab,
        Key::Escape,
        Key::X,
        Key::Num1,
        Key::F,
        Key::Delete,
    ] {
        assert_eq!(pad_keys::pad_key(k), None, "{k:?}");
    }
}
