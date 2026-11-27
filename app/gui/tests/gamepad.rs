//! G.22 (Story 4.8 AC 1–5, 11–15): the gamepad panel drawn alone and on
//! the main screen: the layout (padding, caption, pad, legend, hint), the
//! pad geometry from the mock's svg, the token look (no mock hex), convex
//! fills and the one body outline, the legend, the circle hit test, the
//! AccessKit nodes (16 buttons, 2 sliders, no Focus), the hover and pressed
//! tints (none while blocked), the inline hint clipped to its rect, the
//! wording check and the pure model's source.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use egui::accesskit::{Action, Role};
use egui::epaint::Shape;
use egui::{Color32, Event, Pos2, Rect, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use ocb_app::gamepad::{
    CAPTION, CAPTION_EXEMPT_PHRASE, CHIP_TEXTS, Control, LEGEND, MOUSE, Stick, TAB, Target,
};
use ocb_app::gating::Block;
use ocb_app::leds::banned_word;
use ocb_app::ui::gamepad::{
    self, BODY_QUAD_SEGMENTS, BODY_SEGMENTS, BODY_START, ControlShape, PadGeometry, PadView,
    TextHeights,
};
use ocb_app::ui::layout::regions;
use ocb_app::ui::paint::{self, ButtonState};
use ocb_app::ui::theme;

const SIZES: [(f32, f32); 2] = [(1024.0, 680.0), (1280.0, 800.0)];
const PPPS: [f32; 2] = [1.0, 2.0];

/// The panel alone over the gamepad region of a `w × h` window; the state
/// is the region.
fn panel(w: f32, h: f32, ppp: f32, view: PadView) -> Harness<'static, Rect> {
    let mut hn = Harness::builder()
        .with_size(vec2(w, h))
        .with_pixels_per_point(ppp)
        .with_context_setup(ocb_app::setup)
        .build_ui_state(
            move |ui, region: &mut Rect| {
                *region = region_of(w, h);
                let r = *region;
                let _ = gamepad::show(ui, r, &view);
            },
            Rect::NOTHING,
        );
    hn.run();
    hn
}

fn blocked(b: Block) -> PadView {
    PadView {
        enabled: false,
        hint: Some(b.gamepad_hint()),
        ..PadView::default()
    }
}

fn views() -> Vec<(String, PadView)> {
    let mut v = vec![("open".to_owned(), PadView::default())];
    for b in Block::samples() {
        v.push((b.as_str().to_owned(), blocked(b)));
    }
    v
}

fn region_of(w: f32, h: f32) -> Rect {
    regions(Rect::from_min_size(Pos2::ZERO, vec2(w, h))).gamepad
}

fn heights() -> TextHeights {
    TextHeights {
        tab: vec2(61.0, gamepad::TAB_TEXT_HEIGHT),
        caption: gamepad::CAPTION_HEIGHT,
        row: gamepad::LEGEND_TEXT_HEIGHT,
    }
}

/// AC 1: padding 10, tab over the underline, caption 4 under it, the pad
/// (280 × 200, centred) 6 under the caption, the legend 6 under the pad,
/// the hint 8 under the legend, everything inside the region, pairwise
/// disjoint.
#[test]
fn g22_panel_layout() {
    for (w, h) in SIZES {
        let region = region_of(w, h);
        let l = gamepad::layout(region, heights());
        let ctx = format!("{w}x{h}");
        assert!(
            (l.tab.left() - (region.left() + 10.0)).abs() <= 0.5,
            "{ctx}"
        );
        assert!((l.tab.top() - (region.top() + 10.0)).abs() <= 0.5, "{ctx}");
        assert!((l.underline_y - (l.tab.bottom() + 0.5)).abs() <= 0.01);
        assert_eq!(l.underline_x, (region.left() + 10.0, region.right() - 10.0));
        assert!(
            (l.caption.top() - (l.tab.bottom() + 1.0 + 4.0)).abs() <= 0.5,
            "{ctx}"
        );
        assert!(
            (l.pad.top() - (l.caption.bottom() + 6.0)).abs() <= 0.5,
            "{ctx}"
        );
        assert!((l.pad.width() - 280.0).abs() <= 0.5 && (l.pad.height() - 200.0).abs() <= 0.5);
        assert!(
            (l.pad.center().x - region.center().x).abs() <= 0.5,
            "{ctx}: centred"
        );
        assert!(
            (l.pad.left() - (region.left() + 10.0)).abs() <= 0.5,
            "{ctx}"
        );
        assert!((l.legend_rows[0].top() - (l.pad.bottom() + 6.0)).abs() <= 0.5);
        for k in 1..6 {
            assert!((l.legend_rows[k].top() - l.legend_rows[k - 1].bottom()).abs() < 0.01);
        }
        assert!(
            (l.hint.top() - (l.legend_rows[5].bottom() + 8.0)).abs() <= 0.5,
            "{ctx}"
        );
        assert!(
            (l.hint.bottom() - (region.bottom() - 10.0)).abs() <= 0.5,
            "{ctx}"
        );
        let legend = l.legend_rows[0].union(l.legend_rows[5]);
        let rects = [l.tab, l.caption, l.pad, legend, l.hint];
        for r in rects {
            assert!(common::inside(r, region.shrink(10.0), 0.5), "{ctx}: {r:?}");
        }
        for (i, a) in rects.iter().enumerate() {
            for b in &rects[i + 1..] {
                assert!(
                    !a.intersects(*b) || a.intersect(*b).area() <= 0.0,
                    "{ctx}: {a:?} ∩ {b:?}"
                );
            }
        }
        // The hint has room for two lines at 1024 × 680.
        assert!(l.hint.height() >= 2.0 * 17.0, "{ctx}: {}", l.hint.height());
    }
}

/// AC 2: every control's rect or circle is the mock's plus the pad origin.
#[test]
fn g22_pad_geometry() {
    let o = pos2(990.0, 111.0);
    let g = PadGeometry::new(o);
    let rect = |x: f32, y: f32, w: f32, h: f32| Rect::from_min_size(o + vec2(x, y), vec2(w, h));
    let circle = |x: f32, y: f32, r: f32| (o + vec2(x, y), r);
    /// A rect and radius, or a circle.
    type Want = Result<(Rect, f32), (Pos2, f32)>;
    let want: Vec<(Control, Want)> = vec![
        (Control::Zl, Ok((rect(30.0, 8.0, 54.0, 15.0), 6.0))),
        (Control::L, Ok((rect(42.0, 28.0, 54.0, 13.0), 6.0))),
        (Control::R, Ok((rect(184.0, 28.0, 54.0, 13.0), 6.0))),
        (Control::Zr, Ok((rect(196.0, 8.0, 54.0, 15.0), 6.0))),
        (Control::Capture, Ok((rect(114.0, 84.0, 11.0, 11.0), 2.0))),
        (Control::DUp, Ok((rect(97.0, 103.0, 14.0, 14.0), 2.0))),
        (Control::DDown, Ok((rect(97.0, 127.0, 14.0, 14.0), 2.0))),
        (Control::DLeft, Ok((rect(85.0, 115.0, 14.0, 14.0), 2.0))),
        (Control::DRight, Ok((rect(109.0, 115.0, 14.0, 14.0), 2.0))),
        (Control::Minus, Err(circle(112.0, 64.0, 7.0))),
        (Control::Plus, Err(circle(168.0, 64.0, 7.0))),
        (Control::Home, Err(circle(160.0, 89.5, 6.5))),
        (Control::X, Err(circle(212.0, 70.0, 10.0))),
        (Control::Y, Err(circle(194.0, 88.0, 10.0))),
        (Control::A, Err(circle(230.0, 88.0, 10.0))),
        (Control::B, Err(circle(212.0, 106.0, 10.0))),
    ];
    assert_eq!(g.controls.len(), 16);
    for (c, w) in want {
        match (g.control(c), w) {
            (ControlShape::Rect(r, rad), Ok((wr, wrad))) => {
                assert!(common::rect_eq(r, wr, 0.5), "{c:?}: {r:?} vs {wr:?}");
                assert!((rad - wrad).abs() < 0.01, "{c:?}");
            }
            (ControlShape::Circle(k), Err((wc, wrad))) => {
                assert!(
                    k.center.distance(wc) <= 0.5 && (k.radius - wrad).abs() <= 0.5,
                    "{c:?}"
                );
            }
            (got, _) => panic!("{c:?}: wrong shape {got:?}"),
        }
    }
    assert!(g.well(Stick::Left).center.distance(o + vec2(70.0, 88.0)) <= 0.5);
    assert!(g.well(Stick::Right).center.distance(o + vec2(176.0, 122.0)) <= 0.5);
    assert_eq!(g.well(Stick::Left).radius, 17.0);
    assert_eq!(g.knob_radius, 11.0);
    assert!(
        g.dpad_well.center.distance(o + vec2(104.0, 122.0)) <= 0.5 && g.dpad_well.radius == 22.0
    );
    assert!(
        g.abxy_well.center.distance(o + vec2(212.0, 88.0)) <= 0.5 && g.abxy_well.radius == 31.0
    );
    assert_eq!(g.chips.len(), 11);
    assert!(common::rect_eq(
        g.chips[10].0,
        rect(66.0, 151.0, 76.0, 13.0),
        0.5
    ));
    assert_eq!(gamepad::knob_offset(255, 128), vec2(6.0, 0.0));
    assert_eq!(gamepad::knob_offset(0, 0), vec2(-6.0, -6.0));
}

/// AC 4: the circle hit test (the A/B/X/Y rects overlap at the corners).
#[test]
fn g22_hit_test() {
    let o = pos2(0.0, 0.0);
    let g = PadGeometry::new(o);
    // The corner where X's and Y's bounding squares overlap: (202..204,
    // 78..80); outside both circles at (203, 79)? distance to X (212,70) =
    // 12.7 > 10, to Y (194,88) = 12.7 > 10: neither.
    assert_eq!(g.hit(pos2(203.0, 79.0)), None);
    assert_eq!(g.hit(pos2(230.0, 88.0)), Some(Target::Control(Control::A)));
    assert_eq!(g.hit(pos2(239.0, 88.0)), Some(Target::Control(Control::A)));
    assert_eq!(g.hit(pos2(239.0, 97.0)), None, "A's corner");
    assert_eq!(g.hit(pos2(70.0, 88.0)), Some(Target::Stick(Stick::Left)));
    assert_eq!(g.hit(pos2(104.0, 122.0)), None, "the D-pad centre");
    assert_eq!(
        g.hit(pos2(104.0, 105.0)),
        Some(Target::Control(Control::DUp))
    );
    assert_eq!(g.hit(pos2(5.0, 5.0)), None);
}

/// AC 1: drawn at both sizes and densities, open and for every block:
/// every shape and every text's ink inside the gamepad region.
#[test]
fn g22_drawn_inside() {
    for (w, h) in SIZES {
        for ppp in PPPS {
            for (name, view) in views() {
                let hn = panel(w, h, ppp, view);
                let region = *hn.state();
                let ctx = format!("{w}x{h} ppp {ppp} {name}");
                let flat = common::shapes_flat(&hn.output().shapes);
                assert!(flat.len() > 80, "{ctx}: {}", flat.len());
                for f in &flat {
                    let b = f.shape.visual_bounding_rect();
                    // The harness's own panel fill covers the window.
                    let harness_bg = matches!(&f.shape, Shape::Rect(r) if r.fill == theme::FRAME);
                    if b.is_positive() && !harness_bg {
                        assert!(common::inside(b, region, 0.5), "{ctx}: {b:?} {:?}", f.shape);
                    }
                }
                for (t, ink, _) in common::texts(&flat) {
                    assert!(common::inside(ink, region, 0.5), "{ctx}: {t} {ink:?}");
                }
                let hint = view.hint.map(|_| ());
                let hint_texts = common::texts(&flat)
                    .into_iter()
                    .filter(|(t, _, _)| Some(t.as_str()) == view.hint)
                    .count();
                assert_eq!(hint_texts, usize::from(hint.is_some()), "{ctx}");
            }
        }
    }
}

/// AC 2: token colours, widths and chips; no mock hex anywhere.
#[test]
fn g22_pad_look() {
    for ppp in PPPS {
        let hn = panel(1280.0, 800.0, ppp, PadView::default());
        let region = *hn.state();
        let flat = common::shapes_flat(&hn.output().shapes);
        let mock = [
            0x2e21_16ff_u32,
            0x8a6a_3cff,
            0x120c_07ff,
            0x0b07_04ff,
            0x3a29_18ff,
            0xe8d6_abff,
            0xf2e6_c9ff,
            0xb49b_74ff,
        ];
        for f in &flat {
            for hex in mock {
                assert!(
                    !common::uses(&f.shape, theme::rgba(hex)),
                    "mock hex {hex:08x}: {:?}",
                    f.shape
                );
            }
        }
        // The body outline: one closed path, RIM, max(1.5, 1 px).
        let outlines: Vec<_> = flat
            .iter()
            .filter_map(|f| match &f.shape {
                Shape::Path(p)
                    if p.closed && p.fill == Color32::TRANSPARENT && p.points.len() > 50 =>
                {
                    Some(p.clone())
                }
                _ => None,
            })
            .collect();
        assert_eq!(outlines.len(), 1, "one body outline");
        assert_eq!(
            common::stroke_color(&Shape::Path(outlines[0].clone())),
            Some(theme::RIM)
        );
        assert!((outlines[0].stroke.width - 1.5_f32.max(1.0 / ppp)).abs() < 1e-3);
        // The body fill: one mesh of RULE.
        let meshes: Vec<_> = flat
            .iter()
            .filter_map(|f| match &f.shape {
                Shape::Mesh(m) => Some(m.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(meshes.len(), 1);
        assert!(meshes[0].vertices.iter().all(|v| v.color == theme::RULE));
        // Controls: CONTROL fill, RIM 1 pt border (circles and rects).
        let g = PadGeometry::new(
            gamepad::layout(region, TextHeights::measure(&hn.ctx))
                .pad
                .min,
        );
        for (c, s) in &g.controls {
            let found = flat.iter().any(|f| match (&f.shape, s) {
                (Shape::Circle(k), ControlShape::Circle(w)) => {
                    k.center.distance(w.center) < 0.01
                        && (k.radius - w.radius).abs() < 0.01
                        && k.fill == theme::CONTROL
                        && k.stroke.color == theme::RIM
                        && (k.stroke.width - 1.0_f32.max(1.0 / ppp)).abs() < 1e-3
                }
                (Shape::Rect(r), ControlShape::Rect(w, _)) => {
                    common::rect_eq(r.rect, *w, 0.01) && r.fill == theme::CONTROL
                }
                _ => false,
            });
            assert!(found, "{c:?} not drawn with the control tokens");
        }
        // Wells.
        assert!(flat.iter().any(
            |f| matches!(&f.shape, Shape::Circle(k) if k.radius == 31.0 && k.fill == theme::WELL)
        ));
        assert!(flat.iter().any(|f| matches!(&f.shape, Shape::Circle(k) if k.radius == 17.0 && k.fill == theme::WELL && k.stroke.color == theme::RIM)));
        // Chips: WELL + RIM, radius 3, height 13, text KEY_CHIP 10 pt.
        let chips: Vec<_> = flat
            .iter()
            .filter_map(|f| match &f.shape {
                Shape::Rect(r)
                    if r.fill == theme::WELL && r.corner_radius == theme::corner(3.0) =>
                {
                    Some(r.clone())
                }
                _ => None,
            })
            .collect();
        assert_eq!(chips.len(), 11, "{chips:#?}");
        for r in &chips {
            assert_eq!(r.rect.height(), 13.0);
            assert_eq!(r.corner_radius, theme::corner(3.0));
        }
        let chip_strokes = flat
            .iter()
            .filter(|f| matches!(&f.shape, Shape::Rect(r) if r.stroke.color == theme::RIM && r.rect.height() == 13.0 && r.corner_radius == theme::corner(3.0)))
            .count();
        assert_eq!(chip_strokes, 11);
        let texts: Vec<_> = flat
            .iter()
            .filter_map(|f| match &f.shape {
                Shape::Text(t) => Some(t.clone()),
                _ => None,
            })
            .collect();
        for chip in CHIP_TEXTS {
            let t = texts
                .iter()
                .find(|t| t.galley.text() == chip)
                .unwrap_or_else(|| panic!("{chip}"));
            let fmt = &t.galley.job.sections[0].format;
            assert_eq!(fmt.font_id.size, theme::KEY_CHIP.size, "{chip}");
            assert_eq!(fmt.color, theme::TEXT, "{chip}");
        }
        let mice: Vec<_> = texts
            .iter()
            .filter(|t| {
                t.galley.text() == MOUSE && t.galley.job.sections[0].format.font_id.size == 10.0
            })
            .collect();
        assert_eq!(mice.len(), 2);
        for m in mice {
            assert_eq!(m.galley.job.sections[0].format.color, theme::TEXT_MUTED);
        }
        // Letters inside their controls.
        for c in [
            Control::A,
            Control::B,
            Control::X,
            Control::Y,
            Control::L,
            Control::R,
            Control::Zl,
            Control::Zr,
        ] {
            let t = texts
                .iter()
                .find(|t| t.galley.text() == c.a11y_name())
                .unwrap();
            let ink = t.galley.rect.translate(t.pos.to_vec2());
            assert!(
                common::inside(ink, g.control(c).rect(), 0.5),
                "{c:?}: {ink:?}"
            );
        }
    }
}

/// AC 2: every filled path is convex; the one body outline follows the
/// mock path at every segment end point.
#[test]
fn g22_convex_and_outline() {
    let hn = panel(1280.0, 800.0, 1.0, PadView::default());
    let region = *hn.state();
    let flat = common::shapes_flat(&hn.output().shapes);
    let mut filled = 0;
    for f in &flat {
        if let Shape::Path(p) = &f.shape
            && p.fill != Color32::TRANSPARENT
        {
            filled += 1;
            assert!(
                paint::is_convex(&p.points).is_ok(),
                "concave fill {:?}",
                p.points
            );
        }
    }
    assert!(filled >= 3, "the triangles and the house");
    let o = gamepad::layout(region, TextHeights::measure(&hn.ctx))
        .pad
        .min;
    let g = PadGeometry::new(o);
    let outline = g.body_outline();
    let drawn = flat
        .iter()
        .find_map(|f| match &f.shape {
            Shape::Path(p) if p.closed && p.points.len() == outline.len() => Some(p.points.clone()),
            _ => None,
        })
        .expect("the body outline");
    assert_eq!(drawn, outline);
    // Segment end points of the mock path.
    let mut ends = vec![BODY_START];
    ends.extend(BODY_SEGMENTS.iter().map(|s| s.1));
    let mut k = 0;
    for (i, (ctrl, end)) in BODY_SEGMENTS.iter().enumerate() {
        k += if ctrl.is_some() {
            BODY_QUAD_SEGMENTS
        } else {
            1
        };
        let p = outline[k % outline.len()];
        let want = o + vec2(end.0, end.1);
        assert!(p.distance(want) <= 0.5, "segment {i}: {p:?} vs {want:?}");
    }
    // The fan: every triangle from the centre has the same orientation, so
    // the mesh pieces never overlap (the outline is star-shaped from it).
    let c = g.body_fan_centre();
    let n = outline.len();
    let signs: Vec<f32> = (0..n)
        .map(|i| {
            let (a, b) = (outline[i] - c, outline[(i + 1) % n] - c);
            a.x * b.y - a.y * b.x
        })
        .collect();
    assert!(
        signs.iter().all(|s| *s >= 0.0) || signs.iter().all(|s| *s <= 0.0),
        "{signs:?}"
    );
}

/// AC 3: six legend rows, their texts and fonts, one WELL divider each.
#[test]
fn g22_legend() {
    let hn = panel(1280.0, 800.0, 1.0, PadView::default());
    let flat = common::shapes_flat(&hn.output().shapes);
    for (name, keys) in LEGEND {
        let _ = hn.get_by_role_and_label(Role::Label, &gamepad::legend_row_label(name, keys));
        let find = |t: &str| {
            flat.iter()
                .find_map(|f| match &f.shape {
                    // 12.5 pt: the legend's (the stick's "mouse" is 10 pt).
                    Shape::Text(x)
                        if x.galley.text() == t
                            && x.galley.job.sections[0].format.font_id.size == 12.5 =>
                    {
                        Some(x.galley.job.sections[0].format.clone())
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("{t}"))
        };
        let n = find(name);
        assert_eq!(
            (n.font_id.size, n.color),
            (12.5, theme::PARCHMENT),
            "{name}"
        );
        let k = find(keys);
        assert_eq!((k.font_id.size, k.color), (12.5, theme::TEXT), "{keys}");
    }
    let dividers = flat
        .iter()
        .filter(|f| matches!(&f.shape, Shape::LineSegment { stroke, .. } if stroke.color == theme::WELL))
        .count();
    assert_eq!(dividers, 6);
    let _ = hn.get_by_role_and_label(Role::Label, TAB);
    let _ = hn.get_by_role_and_label(Role::Label, CAPTION);
}

/// AC 13, 14: 16 `Button` nodes with the exact names and `Click`, no
/// `Focus`; 2 `Slider` nodes without either; no node from chips or
/// symbols.
#[test]
fn g22_accesskit() {
    let hn = panel(1280.0, 800.0, 1.0, PadView::default());
    let buttons: Vec<String> = hn
        .query_all_by_role(Role::Button)
        .map(|n| n.accesskit_node().label().unwrap_or_default())
        .collect();
    // The exact names of AC 14 (in drawing order), not read back from the
    // code under test.
    let want: Vec<String> = [
        "ZL",
        "L",
        "R",
        "ZR",
        "Minus",
        "Plus",
        "Capture",
        "Home",
        "D-pad up",
        "D-pad down",
        "D-pad left",
        "D-pad right",
        "X",
        "Y",
        "A",
        "B",
    ]
    .map(str::to_owned)
    .to_vec();
    assert_eq!(buttons, want);
    for c in Control::ALL {
        let n = hn.get_by_role_and_label(Role::Button, c.a11y_name());
        let d = n.accesskit_node().data().clone();
        assert!(d.supports_action(Action::Click), "{c:?}");
        assert!(!d.supports_action(Action::Focus), "{c:?}: focusable");
    }
    for s in Stick::ALL {
        let n = hn.get_by_role_and_label(Role::Slider, s.a11y_name());
        let d = n.accesskit_node().data().clone();
        assert!(
            !d.supports_action(Action::Click) && !d.supports_action(Action::Focus),
            "{s:?}"
        );
    }
    assert_eq!(hn.query_all_by_role(Role::Slider).count(), 2);
    assert_eq!(hn.query_all_by_role(Role::Image).count(), 0);
    // (The chip "L" shares its text with the L button: not checked.)
    for chip in CHIP_TEXTS
        .iter()
        .filter(|c| **c != "L")
        .chain(&[MOUSE, "⌫", "↵"])
    {
        assert!(hn.query_by_label(chip).is_none(), "{chip} has a node");
    }
}

/// The fill of the circle drawn for A (radius 10 at A's centre).
fn a_fill(hn: &Harness<'_, Rect>) -> Color32 {
    let region = *hn.state();
    let g = PadGeometry::new(
        gamepad::layout(region, TextHeights::measure(&hn.ctx))
            .pad
            .min,
    );
    let ControlShape::Circle(a) = g.control(Control::A) else {
        panic!()
    };
    common::shapes_flat(&hn.output().shapes)
        .iter()
        .find_map(|f| match &f.shape {
            Shape::Circle(k) if k.center.distance(a.center) < 0.01 && k.radius == a.radius => {
                Some(k.fill)
            }
            _ => None,
        })
        .unwrap()
}

fn close(a: Color32, b: Color32) -> bool {
    a.to_array()
        .iter()
        .zip(b.to_array())
        .all(|(x, y)| x.abs_diff(y) <= 1)
}

/// AC 15: hovered A = the hover tint, held A = the pressed tint; with the
/// gate blocked neither.
#[test]
fn g22_states() {
    for enabled in [true, false] {
        let mut view = PadView {
            enabled,
            ..PadView::default()
        };
        let mut hn = panel(1280.0, 800.0, 1.0, view);
        let pos = hn.get_by_role_and_label(Role::Button, "A").rect().center();
        hn.input_mut().events.push(Event::PointerMoved(pos));
        hn.run();
        let want = if enabled {
            ButtonState::Hovered.tint(theme::CONTROL)
        } else {
            theme::CONTROL
        };
        assert!(close(a_fill(&hn), want), "hover enabled={enabled}");
        view.held = Some(Control::A);
        let hn = panel(1280.0, 800.0, 1.0, view);
        let want = if enabled {
            ButtonState::Pressed.tint(theme::CONTROL)
        } else {
            theme::CONTROL
        };
        assert!(close(a_fill(&hn), want), "held enabled={enabled}");
    }
}

/// AC 11 (4.6 finding 15): a long hint in a 30 pt-high rect never paints
/// or reports a node outside it.
#[test]
fn g22_inline_hint_respects_rect() {
    let text = "Gamepad clicks are ignored: Bonooru's USB check is not responding. And a much longer tail that wraps onto several more lines than the rect can hold.";
    let rect = Rect::from_min_size(pos2(40.0, 40.0), vec2(120.0, 30.0));
    let mut hn = Harness::builder()
        .with_size(vec2(400.0, 300.0))
        .with_context_setup(ocb_app::setup)
        .build_ui(move |ui| {
            let _ = ocb_app::ui::widgets::inline_hint(ui, rect, text);
        });
    hn.run();
    let flat = common::shapes_flat(&hn.output().shapes);
    let t = flat
        .iter()
        .find(|f| matches!(&f.shape, Shape::Text(x) if x.galley.text() == text))
        .unwrap();
    assert!(common::inside(t.clip, rect, 0.01), "clip {:?}", t.clip);
    let Shape::Text(ts) = &t.shape else { panic!() };
    assert!(
        ts.galley.rect.height() > 30.0,
        "the text is taller than the rect"
    );
    let node = hn.get_by_role_and_label(Role::Label, text);
    assert!(
        common::inside(node.rect(), rect, 0.5),
        "node {:?}",
        node.rect()
    );
}

/// AC 12: no banned word in any gamepad string; the caption is checked
/// without "free play" only.
#[test]
fn g22_no_online_wording() {
    let mut strings: Vec<&str> = vec![TAB, MOUSE];
    strings.extend(CHIP_TEXTS);
    for (n, k) in LEGEND {
        strings.push(n);
        strings.push(k);
    }
    strings.extend(Control::ALL.iter().map(|c| c.a11y_name()));
    strings.extend(Stick::ALL.iter().map(|s| s.a11y_name()));
    strings.extend(Block::samples().iter().map(Block::gamepad_hint));
    for s in strings {
        assert_eq!(banned_word(s), None, "{s}");
    }
    assert_eq!(
        banned_word(CAPTION),
        Some("free play"),
        "self-test: the exemption is needed"
    );
    let rest = CAPTION.replace(CAPTION_EXEMPT_PHRASE, "");
    assert_eq!(banned_word(&rest), None, "{CAPTION}");
}

/// The pure model names no GUI crate (as G.17's check of `gating.rs`).
#[test]
fn g22_model_has_no_egui() {
    fn hits(src: &str) -> Vec<String> {
        let code = common::code_only(src);
        code.split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .filter(|w| matches!(*w, "egui" | "eframe" | "epaint" | "egui_kittest"))
            .map(str::to_owned)
            .collect()
    }
    assert!(hits("// egui\nlet s = \"egui\";").is_empty());
    assert_eq!(hits("use egui::Ui;"), vec!["egui".to_owned()]);
    let src = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/gamepad.rs"),
    )
    .unwrap();
    assert!(src.contains("pub struct Pad"), "the right file");
    assert_eq!(hits(&src), Vec::<String>::new());
}

/// The fixed line heights the layout uses (`TAB_TEXT_HEIGHT`, …) hold every
/// galley within half a point at ppp 1–2 (G.1: the layout never moves with
/// the density).
#[test]
fn g22_text_heights_fit() {
    for ppp in [1.0_f32, 1.25, 1.5, 1.75, 2.0] {
        let mut hn = Harness::builder()
            .with_pixels_per_point(ppp)
            .with_context_setup(ocb_app::setup)
            .build_ui(move |ui| {
                let ctx = ui.ctx().clone();
                let h = |style, text: &str| {
                    paint::layout_line(&ctx, style, text, Color32::WHITE)
                        .rect
                        .height()
                };
                let tol = 0.5;
                assert!(
                    h(&theme::PANEL_TAB, TAB) <= gamepad::TAB_TEXT_HEIGHT + tol,
                    "ppp {ppp}"
                );
                assert!(
                    h(&theme::SMALL, CAPTION) <= gamepad::CAPTION_HEIGHT + tol,
                    "ppp {ppp}"
                );
                for (n, k) in LEGEND {
                    assert!(
                        h(&theme::LEGEND_NAME, n) <= gamepad::LEGEND_TEXT_HEIGHT + tol,
                        "ppp {ppp}"
                    );
                    assert!(
                        h(&theme::SMALL, k) <= gamepad::LEGEND_TEXT_HEIGHT + tol,
                        "ppp {ppp}"
                    );
                }
            });
        hn.run();
    }
}
