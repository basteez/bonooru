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
        // Story 4.3: dialog buttons, focus ring on dark, dialog title.
        ("ink", t::INK, "amber", t::AMBER, 4.5),
        ("text", t::TEXT, "well", t::WELL, 4.5),
        ("amber", t::AMBER, "panel", t::PANEL, 3.0),
        ("amber", t::AMBER, "panel-raised", t::PANEL_RAISED, 3.0),
        ("amber", t::AMBER, "well", t::WELL, 3.0),
        (
            "parchment",
            t::PARCHMENT,
            "panel-raised",
            t::PANEL_RAISED,
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

/// `fg` at `alpha` over `bg`, blended in gamma space like egui.
fn over(fg: Color32, alpha: f32, bg: Color32) -> Color32 {
    let mix = |f: u8, b: u8| (f32::from(f) * alpha + f32::from(b) * (1.0 - alpha)).round() as u8;
    Color32::from_rgb(
        mix(fg.r(), bg.r()),
        mix(fg.g(), bg.g()),
        mix(fg.b(), bg.b()),
    )
}

/// Owner eye check 2026-11-27 (Story 4.3): the disabled label must stay
/// readable. It is measured against what is really behind it: the faded fill
/// (`button-disabled.fill-opacity`) composited over each panel a button can
/// sit on. Floor 3:1 (a disabled control's label, WCAG large/UI floor).
#[test]
fn g6_disabled_button_labels() {
    use t::components::{
        BUTTON_DISABLED_FILL_OPACITY as A, BUTTON_DISABLED_TEXT, BUTTON_DISABLED_TEXT_ON_PRIMARY,
    };
    let backgrounds = [
        ("frame", t::FRAME),
        ("panel", t::PANEL),
        ("panel-raised", t::PANEL_RAISED),
        ("well", t::WELL),
    ];
    let mut errs = Vec::new();
    for (bg_name, bg) in backgrounds {
        for (kind, fill, text) in [
            ("primary", t::AMBER, BUTTON_DISABLED_TEXT_ON_PRIMARY),
            ("ghost", t::WELL, BUTTON_DISABLED_TEXT),
        ] {
            let behind = over(fill, A, bg);
            let r = ratio(text, behind);
            println!("disabled {kind} on {bg_name}: {r:.2}:1");
            if r < 3.0 {
                errs.push(format!(
                    "disabled {kind} label on {bg_name}: {r:.2}:1 < 3:1"
                ));
            }
        }
    }
    assert!(errs.is_empty(), "{}", errs.join("\n"));
    // The look the owner rejected: `ink` text and `amber` fill both at 45 %.
    let old = ratio(
        over(t::INK, 0.45, over(t::AMBER, 0.45, t::PANEL_RAISED)),
        over(t::AMBER, 0.45, t::PANEL_RAISED),
    );
    assert!(old < 3.0, "the rejected look measured {old:.2}:1");
}
