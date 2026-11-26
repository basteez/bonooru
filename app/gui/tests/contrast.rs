//! G.6: contrast floors of the colour tokens (WCAG 2.x relative luminance),
//! so any value changed at the owner review of UX Open Question 7 (AC 11)
//! still meets them.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use egui::Color32;
use ocb_app::ui::theme as t;

fn channel(c: u8) -> f64 {
    let s = f64::from(c) / 255.0;
    if s <= 0.040_45 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

fn luminance(c: Color32) -> f64 {
    assert_eq!(
        c.a(),
        255,
        "contrast is defined here for opaque colours only"
    );
    0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b())
}

fn ratio(a: Color32, b: Color32) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

#[test]
fn g6_contrast_floors() {
    let pairs: &[(&str, Color32, &str, Color32, f64)] = &[
        ("text", t::TEXT, "panel-raised", t::PANEL_RAISED, 4.5),
        (
            "text-muted",
            t::TEXT_MUTED,
            "panel-raised",
            t::PANEL_RAISED,
            4.5,
        ),
        (
            "status-ok",
            t::STATUS_OK,
            "panel-raised",
            t::PANEL_RAISED,
            4.5,
        ),
        (
            "status-warning",
            t::STATUS_WARNING,
            "panel-raised",
            t::PANEL_RAISED,
            4.5,
        ),
        (
            "status-error",
            t::STATUS_ERROR,
            "panel-raised",
            t::PANEL_RAISED,
            4.5,
        ),
        (
            "status-unknown",
            t::STATUS_UNKNOWN,
            "panel-raised",
            t::PANEL_RAISED,
            4.5,
        ),
        ("status-error", t::STATUS_ERROR, "panel", t::PANEL, 4.5),
        ("status-error", t::STATUS_ERROR, "well", t::WELL, 4.5),
        ("amber-ink", t::AMBER_INK, "parchment", t::PARCHMENT, 3.0),
        (
            "amber-ink",
            t::AMBER_INK,
            "parchment-header",
            t::PARCHMENT_HEADER,
            3.0,
        ),
        ("ink", t::INK, "parchment", t::PARCHMENT, 4.5),
        (
            "out-of-range-ink",
            t::OUT_OF_RANGE_INK,
            "out-of-range-zone",
            t::OUT_OF_RANGE_ZONE,
            4.5,
        ),
    ];
    let mut errs = Vec::new();
    for (fg_name, fg, bg_name, bg, floor) in pairs {
        let r = ratio(*fg, *bg);
        println!("{fg_name} on {bg_name}: {r:.2}:1 (floor {floor}:1)");
        if r < *floor {
            errs.push(format!("{fg_name} on {bg_name}: {r:.2}:1 < {floor}:1"));
        }
    }
    assert!(
        errs.is_empty(),
        "contrast below floor:\n  {}",
        errs.join("\n  ")
    );
}

/// Self-test of the formula: black on white is 21:1, a colour on itself 1:1,
/// and the documented 4.71:1 of `out-of-range-ink` on its zone.
#[test]
fn g6_formula_self_test() {
    assert!((ratio(Color32::BLACK, Color32::WHITE) - 21.0).abs() < 1e-9);
    assert!((ratio(t::AMBER, t::AMBER) - 1.0).abs() < 1e-9);
    let r = ratio(t::OUT_OF_RANGE_INK, t::OUT_OF_RANGE_ZONE);
    assert!((r - 4.71).abs() < 0.01, "{r}");
}
