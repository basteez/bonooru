//! G.3: the theme module and the `DESIGN.md` frontmatter hold the same
//! tokens (UX-DR1). Reads the frontmatter between the first two `---` lines,
//! parses `colors`, `rounded`, `spacing` and `typography` with a small line
//! parser (no YAML crate) and compares both ways: a token missing on either
//! side, or a different value, fails with the token's name.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use ocb_app::ui::theme::{self, Family};

fn design_md() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../_bmad-output/planning-artifacts/ux-designs/ux-bonooru-2026-11-20/DESIGN.md");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The frontmatter lines between the first two `---`.
fn frontmatter(doc: &str) -> Vec<&str> {
    let mut lines = doc.lines();
    assert_eq!(lines.next(), Some("---"), "DESIGN.md has no frontmatter");
    lines.take_while(|l| *l != "---").collect()
}

/// Strips a `# comment` that is outside quotes, then surrounding quotes.
fn value(raw: &str) -> String {
    let raw = raw.trim();
    if let Some(q) = raw.chars().next().filter(|c| *c == '\'' || *c == '"') {
        let rest = &raw[1..];
        let end = rest
            .find(q)
            .unwrap_or_else(|| panic!("unclosed quote: {raw}"));
        return rest[..end].to_owned();
    }
    raw.split('#').next().unwrap().trim().to_owned()
}

fn key(raw: &str) -> String {
    value(raw)
}

/// One top-level section: its two-space keys with their value (flat
/// sections) or with their four-space fields (typography).
#[derive(Debug, Default)]
struct Section {
    flat: BTreeMap<String, String>,
    nested: BTreeMap<String, BTreeMap<String, String>>,
}

fn section(lines: &[&str], name: &str) -> Section {
    let start = lines
        .iter()
        .position(|l| *l == format!("{name}:"))
        .unwrap_or_else(|| panic!("no `{name}:` section"));
    let mut out = Section::default();
    let mut current: Option<String> = None;
    for line in &lines[start + 1..] {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        if !line.starts_with(' ') {
            break; // next top-level key
        }
        let (k, v) = line
            .trim()
            .split_once(':')
            .unwrap_or_else(|| panic!("bad line: {line}"));
        if line.starts_with("    ") {
            let parent = current
                .clone()
                .unwrap_or_else(|| panic!("orphan field: {line}"));
            out.nested
                .entry(parent)
                .or_default()
                .insert(key(k), value(v));
        } else {
            let k = key(k);
            let v = value(v);
            if v.is_empty() {
                out.nested.entry(k.clone()).or_default();
                current = Some(k);
            } else {
                out.flat.insert(k, v);
                current = None;
            }
        }
    }
    out
}

fn px(v: &str, token: &str) -> f32 {
    v.strip_suffix("px")
        .unwrap_or_else(|| panic!("{token}: `{v}` is not px"))
        .parse()
        .unwrap_or_else(|e| panic!("{token}: {e}"))
}

fn num(v: &str, token: &str) -> f32 {
    v.parse().unwrap_or_else(|e| panic!("{token}: `{v}`: {e}"))
}

fn hex(v: &str, token: &str) -> u32 {
    let h = v
        .strip_prefix('#')
        .unwrap_or_else(|| panic!("{token}: `{v}` is not a hex"));
    let full = match h.len() {
        6 => format!("{h}ff"),
        8 => h.to_owned(),
        _ => panic!("{token}: `{v}` is not 6 or 8 hex digits"),
    };
    u32::from_str_radix(&full, 16).unwrap_or_else(|e| panic!("{token}: {e}"))
}

/// Compares the key sets both ways; returns the error lines.
fn missing<'a>(
    what: &str,
    design: impl IntoIterator<Item = &'a String>,
    theme: impl IntoIterator<Item = &'a str>,
) -> Vec<String> {
    let design: Vec<&str> = design.into_iter().map(String::as_str).collect();
    let theme: Vec<&str> = theme.into_iter().collect();
    let mut errs = Vec::new();
    for d in &design {
        if !theme.contains(d) {
            errs.push(format!("{what}.{d}: in DESIGN.md, missing in theme"));
        }
    }
    for t in &theme {
        if !design.contains(t) {
            errs.push(format!("{what}.{t}: in theme, missing in DESIGN.md"));
        }
    }
    errs
}

fn report(errs: &[String]) {
    assert!(errs.is_empty(), "token mismatch:\n  {}", errs.join("\n  "));
}

#[test]
fn g3_colors() {
    let doc = design_md();
    let lines = frontmatter(&doc);
    let s = section(&lines, "colors");
    let tokens = theme::color_tokens();
    let mut errs = missing("colors", s.flat.keys(), tokens.iter().map(|t| t.name));
    for t in tokens {
        if let Some(v) = s.flat.get(t.name) {
            let want = hex(v, t.name);
            if want != t.rgba {
                errs.push(format!(
                    "colors.{}: DESIGN.md #{want:08x}, theme #{:08x}",
                    t.name, t.rgba
                ));
            }
            // AC 3: the constant is `Color32::from_rgba_unmultiplied` of the hex.
            let [r, g, b, a] = t.rgba.to_be_bytes();
            assert_eq!(
                t.color,
                egui::Color32::from_rgba_unmultiplied(r, g, b, a),
                "{}",
                t.name
            );
        }
    }
    report(&errs);
}

fn compare_sizes(section_name: &str, theme_tokens: &[(&'static str, f32)]) {
    let doc = design_md();
    let lines = frontmatter(&doc);
    let s = section(&lines, section_name);
    let mut errs = missing(
        section_name,
        s.flat.keys(),
        theme_tokens.iter().map(|(n, _)| *n),
    );
    for (name, val) in theme_tokens {
        if let Some(v) = s.flat.get(*name) {
            let want = px(v, name);
            if want != *val {
                errs.push(format!(
                    "{section_name}.{name}: DESIGN.md {want}, theme {val}"
                ));
            }
        }
    }
    report(&errs);
}

#[test]
fn g3_rounded() {
    compare_sizes("rounded", theme::rounded_tokens());
}

#[test]
fn g3_spacing() {
    compare_sizes("spacing", theme::spacing_tokens());
}

#[test]
fn g3_typography() {
    let doc = design_md();
    let lines = frontmatter(&doc);
    let s = section(&lines, "typography");
    let tokens = theme::typography_tokens();
    let mut errs = missing(
        "typography",
        s.nested.keys(),
        tokens.iter().map(|(n, _)| *n),
    );
    for (name, style) in tokens {
        let Some(fields) = s.nested.get(*name) else {
            continue;
        };
        if *name == "code" {
            // Named exception: `typography.code` is a note, not fields
            // ("egui bundled monospace (Hack, ...); size 13px").
            let note = fields.get("note").map_or("", String::as_str);
            assert!(
                note.contains("Hack") && note.contains("13px"),
                "typography.code note: {note}"
            );
            assert_eq!(style.family, Family::Code, "typography.code family");
            assert_eq!(style.size, 13.0, "typography.code size");
            continue;
        }
        let field = |f: &str| fields.get(f).map(String::as_str);
        let mut check = |what: &str, design: Option<String>, ours: String| {
            let design = design.unwrap_or_else(|| "-".to_owned());
            if design != ours {
                errs.push(format!(
                    "typography.{name}.{what}: DESIGN.md {design}, theme {ours}"
                ));
            }
        };
        check(
            "fontFamily",
            field("fontFamily").map(str::to_owned),
            style.family.design_name().to_owned(),
        );
        check(
            "fontSize",
            field("fontSize").map(|v| px(v, name).to_string()),
            style.size.to_string(),
        );
        check(
            "fontWeight",
            field("fontWeight").map(|v| num(v, name).to_string()),
            style.weight.to_string(),
        );
        check(
            "lineHeight",
            field("lineHeight").map(|v| num(v, name).to_string()),
            style
                .line_height
                .map_or_else(|| "-".to_owned(), |f| f.to_string()),
        );
        let em = field("letterSpacing").map_or(0.0, |v| {
            num(
                v.strip_suffix("em")
                    .unwrap_or_else(|| panic!("{name}: `{v}` is not em")),
                name,
            )
        });
        check(
            "letterSpacing",
            Some(em.to_string()),
            style.letter_spacing_em.to_string(),
        );
        let known = [
            "fontFamily",
            "fontSize",
            "fontWeight",
            "lineHeight",
            "letterSpacing",
        ];
        for f in fields.keys() {
            if !known.contains(&f.as_str()) {
                errs.push(format!("typography.{name}.{f}: field unknown to the theme"));
            }
        }
    }
    report(&errs);
}

/// The parser itself: a comment after a quoted hex is stripped, a `#`
/// inside quotes is kept, nested fields land under their parent.
#[test]
fn g3_parser_self_test() {
    let lines = [
        "colors:",
        "  # a comment line",
        "  a: '#112233'   # [ASSUMPTION] note",
        "  b: '#11223344'",
        "typography:",
        "  t:",
        "    fontSize: 13px",
        "    fontWeight: '700'",
        "rounded:",
        "  '1': 4px",
    ];
    let c = section(&lines, "colors");
    assert_eq!(c.flat.len(), 2);
    assert_eq!(hex(&c.flat["a"], "a"), 0x1122_33ff);
    assert_eq!(hex(&c.flat["b"], "b"), 0x1122_3344);
    let t = section(&lines, "typography");
    assert_eq!(t.nested["t"]["fontWeight"], "700");
    assert_eq!(section(&lines, "rounded").flat["1"], "4px");
}
