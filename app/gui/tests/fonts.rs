//! G.4: bundled variable fonts, default weights, explicit weights, label
//! tracking and no `strong()` as bold (UX-DR2, AC 4). Widths are measured
//! on galleys laid out by the real font setup, CPU only.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

use egui::epaint::text::{LayoutJob, TextFormat, VariationCoords};
use egui::{Align, FontData, FontFamily, FontId, FontSelection};
use egui_kittest::Harness;
use ocb_app::ui::fonts;
use ocb_app::ui::theme::{self, Family};

const SAMPLE: &str = "Hamburgefonstiv Song of Storms";
const RAW: &str = "raw-nunito";

/// A harness with the app's font setup plus a `raw-nunito` family: the same
/// Nunito Sans bytes without the `wght` 400 tweak.
fn harness() -> Harness<'static> {
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            ocb_app::setup(ctx);
            let mut defs = fonts::definitions();
            defs.font_data.insert(
                RAW.to_owned(),
                FontData::from_static(fonts::NUNITO_SANS_TTF).into(),
            );
            defs.families
                .insert(FontFamily::Name(RAW.into()), vec![RAW.to_owned()]);
            ctx.set_fonts(defs);
        })
        .build_ui(|_ui| {});
    h.run();
    h
}

fn width(h: &Harness<'_>, family: FontFamily, wght: Option<f32>) -> f32 {
    let format = TextFormat {
        font_id: FontId::new(20.0, family),
        coords: wght.map_or_else(VariationCoords::default, |w| {
            VariationCoords::new([("wght", w)])
        }),
        ..Default::default()
    };
    let job = LayoutJob::simple_format(SAMPLE.to_owned(), format);
    h.ctx.fonts_mut(|f| f.layout_job(job)).size().x
}

fn display() -> FontFamily {
    Family::Display.egui_family()
}

#[test]
fn g4_axis_ranges_of_the_bundled_files() {
    let axis = |bytes: &'static [u8]| {
        let axes = FontData::from_static(bytes).variation_axes();
        let w = axes
            .iter()
            .find(|a| a.tag.to_be_bytes() == *b"wght")
            .expect("wght axis");
        (w.range.min, w.range.max, w.default)
    };
    assert_eq!(axis(fonts::CORMORANT_GARAMOND_TTF), (300.0, 700.0, 300.0));
    assert_eq!(axis(fonts::NUNITO_SANS_TTF), (200.0, 1000.0, 200.0));
}

#[test]
fn g4_no_weight_is_400_for_nunito_not_the_file_default() {
    let h = harness();
    let none = width(&h, FontFamily::Proportional, None);
    let w400 = width(&h, FontFamily::Proportional, Some(400.0));
    let raw = width(&h, FontFamily::Name(RAW.into()), None);
    println!("nunito: no weight {none}, 400 {w400}, raw file default {raw}");
    assert_eq!(none, w400, "text with no weight must render at wght 400");
    assert!(
        w400 > raw,
        "400 ({w400}) must be wider than the file default 200 ({raw})"
    );
}

#[test]
fn g4_explicit_weights_override_the_default() {
    let h = harness();
    let w400 = width(&h, FontFamily::Proportional, Some(400.0));
    let w600 = width(&h, FontFamily::Proportional, Some(600.0));
    let w700 = width(&h, FontFamily::Proportional, Some(700.0));
    println!("nunito: 400 {w400}, 600 {w600}, 700 {w700}");
    assert!(w700 > w600 && w600 > w400, "{w400} {w600} {w700}");
}

#[test]
fn g4_display_with_no_weight_is_700() {
    let h = harness();
    let none = width(&h, display(), None);
    let w700 = width(&h, display(), Some(700.0));
    let w300 = width(&h, display(), Some(300.0));
    println!("cormorant: no weight {none}, 700 {w700}, 300 {w300}");
    assert_eq!(none, w700);
    assert_ne!(none, w300);
}

#[test]
fn g4_text_styles_map_to_the_tokens() {
    let h = harness();
    let style = h.ctx.global_style();
    let get = |s: egui::TextStyle| style.text_styles.get(&s).cloned().unwrap();
    assert_eq!(get(egui::TextStyle::Body), FontId::proportional(13.5));
    assert_eq!(get(egui::TextStyle::Small), FontId::proportional(12.5));
    assert_eq!(get(egui::TextStyle::Button), FontId::proportional(13.5));
    assert_eq!(get(egui::TextStyle::Heading), FontId::new(21.0, display()));
    assert_eq!(get(egui::TextStyle::Monospace), FontId::monospace(13.0));
}

#[test]
fn g4_families_never_load_an_os_font() {
    let defs = fonts::definitions();
    let fam = |f: FontFamily| defs.families.get(&f).cloned().unwrap();
    assert_eq!(
        fam(FontFamily::Proportional),
        ["NunitoSans", "Ubuntu-Light", "egui-icons"]
    );
    assert_eq!(
        fam(display()),
        [
            "CormorantGaramond",
            "NunitoSans",
            "Ubuntu-Light",
            "egui-icons"
        ]
    );
    assert_eq!(
        fam(FontFamily::Monospace),
        ["Hack", "Ubuntu-Light", "egui-icons"]
    );
    let mut names: Vec<_> = defs.font_data.keys().cloned().collect();
    names.sort();
    assert_eq!(
        names,
        [
            "CormorantGaramond",
            "Hack",
            "NunitoSans",
            "Ubuntu-Light",
            "egui-icons"
        ]
    );
}

#[test]
fn g4_label_is_uppercase_with_0_06_em_tracking() {
    let h = harness();
    let label = theme::label("abc");
    assert_eq!(label.text(), "ABC");
    assert!((theme::LABEL.letter_spacing_pt() - 0.69).abs() < 1e-6);

    let style = h.ctx.global_style();
    let galley_width = |rt: egui::RichText| {
        let mut job = LayoutJob::default();
        rt.append_to(&mut job, &style, FontSelection::Default, Align::Min);
        h.ctx.fonts_mut(|f| f.layout_job(job)).size().x
    };
    let tracked = galley_width(theme::label("abc"));
    let plain = galley_width(theme::label("abc").extra_letter_spacing(0.0));
    let diff = tracked - plain;
    println!("label ABC: tracked {tracked}, plain {plain}, diff {diff}");
    // egui adds the extra spacing between glyphs, not after the last one:
    // n - 1 = 2 for 3 glyphs (measured 1.375 = 2 x 0.69 within the galley's
    // rounding; n = 3 would give 2.07).
    assert!((diff - 0.69 * 2.0).abs() <= 0.01, "diff {diff}");
}

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            rs_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// Calls of `strong` in Rust source: the identifier `strong` (whole word)
/// followed by optional whitespace (line breaks included) and `(`. Catches
/// `.strong()`, `.strong ()`, a line break before or after `.`, and
/// `RichText::strong(x)`. Line comments are skipped (prose may name the
/// method); string literals are not, which errs on the strict side.
fn strong_calls(source: &str) -> usize {
    let code: String = source
        .lines()
        .map(|l| l.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    let bytes = code.as_bytes();
    let ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let word = b"strong";
    let mut hits = 0;
    let mut i = 0;
    while let Some(pos) = code[i..].find("strong") {
        let at = i + pos;
        let end = at + word.len();
        let before_ok = at == 0 || !ident(bytes[at - 1]);
        let after_ok = end == bytes.len() || !ident(bytes[end]);
        let mut j = end;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if before_ok && after_ok && bytes.get(j) == Some(&b'(') {
            hits += 1;
        }
        i = end;
    }
    hits
}

#[test]
fn g4_strong_scan_self_test() {
    for bad in [
        "t.strong()",
        "t.strong ()",
        "t\n    .strong()",
        "t.\n    strong\n    ()",
        "egui::RichText::strong(t)",
        "RichText :: strong (t)",
    ] {
        assert_eq!(strong_calls(bad), 1, "missed: {bad:?}");
    }
    for good in [
        "// never .strong() here",
        "let strongest = 1;",
        "fn strong_calls() {}",
        "t.strongly()",
        "let s = strong;",
    ] {
        assert_eq!(strong_calls(good), 0, "false hit: {good:?}");
    }
}

/// `strong()` only changes the colour in egui; weights come from `wght`.
#[test]
fn g4_no_strong_in_the_sources() {
    let mut files = Vec::new();
    rs_files(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    assert!(files.len() >= 5, "scan found too few files: {files:?}");
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap();
        assert_eq!(strong_calls(&text), 0, "{} calls strong()", f.display());
    }
}
