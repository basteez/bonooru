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

/// Parses one top-level section. Fails loudly on any shape the comparison
/// would not see: a duplicate key or field, an indentation other than two
/// (key) or four (field) spaces, a field without a parent.
fn section(lines: &[&str], name: &str) -> Result<Section, String> {
    let start = lines
        .iter()
        .position(|l| *l == format!("{name}:"))
        .ok_or_else(|| format!("no `{name}:` section"))?;
    let mut out = Section::default();
    let mut current: Option<String> = None;
    for line in &lines[start + 1..] {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        if !line.starts_with(' ') {
            break; // next top-level key
        }
        let indent = line.len() - line.trim_start().len();
        let (k, v) = line
            .trim()
            .split_once(':')
            .ok_or_else(|| format!("{name}: bad line: {line}"))?;
        match indent {
            4 => {
                let parent = current
                    .clone()
                    .ok_or_else(|| format!("{name}: field without a group: {line}"))?;
                let group = out.nested.entry(parent.clone()).or_default();
                if group.insert(key(k), value(v)).is_some() {
                    return Err(format!("{name}.{parent}: duplicate field `{}`", key(k)));
                }
            }
            2 => {
                let k = key(k);
                let v = value(v);
                if out.flat.contains_key(&k) || out.nested.contains_key(&k) {
                    return Err(format!("{name}: duplicate key `{k}`"));
                }
                if v.is_empty() {
                    out.nested.insert(k.clone(), BTreeMap::new());
                    current = Some(k);
                } else {
                    out.flat.insert(k, v);
                    current = None;
                }
            }
            _ => {
                return Err(format!(
                    "{name}: unsupported indentation ({indent}): {line}"
                ));
            }
        }
    }
    Ok(out)
}

/// A section of `key: value` tokens only (colors, rounded, spacing): a
/// nested group (or a value the parser reads as empty, such as an unquoted
/// `#hex`) is an error, never silently skipped.
fn flat_section(lines: &[&str], name: &str) -> Result<BTreeMap<String, String>, String> {
    let s = section(lines, name)?;
    if let Some(k) = s.nested.keys().next() {
        return Err(format!(
            "{name}.{k}: nested group or empty value, not a token"
        ));
    }
    Ok(s.flat)
}

/// A section of groups only (typography, components): a one-line entry is
/// an error.
fn grouped_section(
    lines: &[&str],
    name: &str,
) -> Result<BTreeMap<String, BTreeMap<String, String>>, String> {
    let s = section(lines, name)?;
    if let Some(k) = s.flat.keys().next() {
        return Err(format!(
            "{name}.{k}: one-line entry, expected a group of fields"
        ));
    }
    Ok(s.nested)
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
    let flat = flat_section(&lines, "colors").unwrap_or_else(|e| panic!("{e}"));
    let tokens = theme::color_tokens();
    let mut errs = missing("colors", flat.keys(), tokens.iter().map(|t| t.name));
    for t in tokens {
        if let Some(v) = flat.get(t.name) {
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
    let flat = flat_section(&lines, section_name).unwrap_or_else(|e| panic!("{e}"));
    let mut errs = missing(
        section_name,
        flat.keys(),
        theme_tokens.iter().map(|(n, _)| *n),
    );
    for (name, val) in theme_tokens {
        if let Some(v) = flat.get(*name) {
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
    let groups = grouped_section(&lines, "typography").unwrap_or_else(|e| panic!("{e}"));
    let tokens = theme::typography_tokens();
    let mut errs = missing("typography", groups.keys(), tokens.iter().map(|(n, _)| *n));
    for (name, style) in tokens {
        let Some(fields) = groups.get(*name) else {
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

/// Component sizes the screens draw from `theme::components`: the LED size,
/// the corner-bracket inset, the disabled-button opacity and every
/// `components.modal-dialog` size, compared both ways for the modal group
/// (a px key without a constant, or a constant without a key, fails naming
/// it; the group's colour and typography references are not sizes).
#[test]
fn g3_component_sizes() {
    let doc = design_md();
    let lines = frontmatter(&doc);
    let groups = grouped_section(&lines, "components").unwrap_or_else(|e| panic!("{e}"));
    let field = |group: &str, key: &str| -> String {
        groups
            .get(group)
            .and_then(|g| g.get(key))
            .unwrap_or_else(|| panic!("components.{group}.{key} missing in DESIGN.md"))
            .clone()
    };
    let mut errs = Vec::new();
    let eq = |errs: &mut Vec<String>, what: &str, design: f32, ours: f32| {
        if design != ours {
            errs.push(format!(
                "components.{what}: DESIGN.md {design}, theme {ours}"
            ));
        }
    };
    eq(
        &mut errs,
        "status-led.size",
        px(&field("status-led", "size"), "status-led.size"),
        theme::components::STATUS_LED_SIZE,
    );
    // Story 4.5: the strip and LED spacing lifted from the mock.
    use theme::components::{status_led, status_strip};
    for (group, key, ours) in [
        ("status-led", "gap", status_led::GAP),
        ("status-led", "symbol-gap", status_led::SYMBOL_GAP),
        ("status-led", "detail-gap", status_led::DETAIL_GAP),
        ("status-strip", "padding-x", status_strip::PADDING_X),
        ("status-strip", "brand-gap", status_strip::BRAND_GAP),
    ] {
        let what = format!("{group}.{key}");
        eq(&mut errs, &what, px(&field(group, key), &what), ours);
    }
    let padding = field("status-led", "padding");
    let (py, px_) = padding
        .split_once(' ')
        .unwrap_or_else(|| panic!("status-led.padding: `{padding}` is not `<y>px <x>px`"));
    eq(
        &mut errs,
        "status-led.padding y",
        px(py, "status-led.padding"),
        status_led::PADDING_Y,
    );
    eq(
        &mut errs,
        "status-led.padding x",
        px(px_, "status-led.padding"),
        status_led::PADDING_X,
    );
    eq(
        &mut errs,
        "corner-bracket.inset",
        px(&field("corner-bracket", "inset"), "corner-bracket.inset"),
        theme::components::CORNER_BRACKET_INSET,
    );
    // `button-disabled` (owner decision 2026-11-27): colour references
    // `'{colors.<name>}'` (the border `'<w>px {colors.<name>}'`) must name
    // the token whose colour the theme constant holds.
    let colour = |key: &str, v: &str| -> egui::Color32 {
        let name = v
            .strip_prefix("{colors.")
            .and_then(|r| r.strip_suffix('}'))
            .unwrap_or_else(|| panic!("button-disabled.{key}: `{v}` is not a colour reference"));
        theme::color_tokens()
            .iter()
            .find(|t| t.name == name)
            .unwrap_or_else(|| panic!("button-disabled.{key}: no colour `{name}`"))
            .color
    };
    for (key, ours) in [
        ("background", theme::components::BUTTON_DISABLED_FILL),
        ("foreground", theme::components::BUTTON_DISABLED_TEXT),
    ] {
        let v = field("button-disabled", key);
        if colour(key, &v) != ours {
            errs.push(format!(
                "components.button-disabled.{key}: DESIGN.md {v}, theme {ours:?}"
            ));
        }
    }
    let border = field("button-disabled", "border");
    let (w, c) = border
        .split_once(' ')
        .unwrap_or_else(|| panic!("button-disabled.border: `{border}`"));
    eq(
        &mut errs,
        "button-disabled.border width",
        px(w, "button-disabled.border"),
        theme::components::BUTTON_DISABLED_BORDER_WIDTH,
    );
    if colour("border", c) != theme::components::BUTTON_DISABLED_BORDER {
        errs.push(format!(
            "components.button-disabled.border: DESIGN.md {c}, theme differs"
        ));
    }
    let modal = groups
        .get("modal-dialog")
        .expect("components.modal-dialog in DESIGN.md");
    let design_px: BTreeMap<&String, &String> =
        modal.iter().filter(|(_, v)| v.ends_with("px")).collect();
    let ours = theme::components::modal_dialog_tokens();
    errs.extend(missing(
        "components.modal-dialog",
        design_px.keys().copied(),
        ours.iter().map(|(n, _)| *n),
    ));
    for (name, val) in ours {
        if let Some(v) = design_px.get(&(*name).to_owned()) {
            eq(
                &mut errs,
                &format!("modal-dialog.{name}"),
                px(v, name),
                *val,
            );
        }
    }
    report(&errs);
}

/// The blur `<n>` of a glow string written `Shadow blur <n>px ...`, if any.
fn shadow_blur(glow: &str) -> Option<u8> {
    let rest = glow.strip_prefix("Shadow blur ")?;
    let n = rest.split("px").next()?;
    Some(n.parse().unwrap_or_else(|e| panic!("glow `{glow}`: {e}")))
}

/// Every `components.*.glow` string that names `Shadow blur <n>px` has a
/// `theme::glow` constant with the same blur, and every constant has its
/// string (Story 4.3 AC 2, AC 14). Glow strings without a Shadow blur (a bare
/// colour reference, `'none'`, the gamepad panel's inset wash) are not blur
/// tokens and are skipped by name below, so a new kind of glow string fails
/// until it is listed.
#[test]
fn g3_glow_tokens() {
    let doc = design_md();
    let lines = frontmatter(&doc);
    let groups = grouped_section(&lines, "components").unwrap_or_else(|e| panic!("{e}"));
    let errs = glow_mismatches(&groups, theme::glow_tokens());
    report(&errs);
}

/// Components whose `glow` string is not a Shadow blur.
const GLOW_WITHOUT_BLUR: &[&str] = &[
    "note-bar-selected",
    "gamepad-control-lit",
    "gamepad-panel-focused",
    "button-disabled",
];

fn glow_mismatches(
    groups: &BTreeMap<String, BTreeMap<String, String>>,
    ours: &[(&str, u8)],
) -> Vec<String> {
    let mut design = BTreeMap::new();
    let mut errs = Vec::new();
    for (name, fields) in groups {
        let Some(glow) = fields.get("glow") else {
            continue;
        };
        match shadow_blur(glow) {
            Some(n) => {
                design.insert(name.clone(), n);
            }
            None if GLOW_WITHOUT_BLUR.contains(&name.as_str()) => {}
            None => errs.push(format!(
                "components.{name}.glow: `{glow}` is neither a Shadow blur nor listed as one without"
            )),
        }
    }
    errs.extend(missing("glow", design.keys(), ours.iter().map(|(n, _)| *n)));
    for (name, blur) in ours {
        if let Some(n) = design.get(*name)
            && n != blur
        {
            errs.push(format!("glow.{name}: DESIGN.md blur {n}px, theme {blur}"));
        }
    }
    errs
}

/// The glow comparison fails on a changed blur, on a blur string without a
/// constant and on a constant without a string.
#[test]
fn g3_glow_self_test() {
    let group = |glow: &str| {
        let mut g = BTreeMap::new();
        g.insert("glow".to_owned(), glow.to_owned());
        g
    };
    let mut groups = BTreeMap::new();
    groups.insert("a".to_owned(), group("Shadow blur 8px, same hue"));
    groups.insert("button-disabled".to_owned(), group("none"));
    assert!(glow_mismatches(&groups, &[("a", 8)]).is_empty());
    let changed = glow_mismatches(&groups, &[("a", 9)]);
    assert!(
        changed[0].starts_with("glow.a: DESIGN.md blur 8px"),
        "{changed:?}"
    );
    let no_constant = glow_mismatches(&groups, &[]);
    assert!(
        no_constant[0].starts_with("glow.a: in DESIGN.md"),
        "{no_constant:?}"
    );
    let no_string = glow_mismatches(&groups, &[("a", 8), ("b", 3)]);
    assert!(
        no_string[0].starts_with("glow.b: in theme"),
        "{no_string:?}"
    );
    groups.insert("c".to_owned(), group("soft halo"));
    let unknown = glow_mismatches(&groups, &[("a", 8)]);
    assert!(unknown[0].contains("components.c.glow"), "{unknown:?}");
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
    let c = flat_section(&lines, "colors").unwrap();
    assert_eq!(c.len(), 2);
    assert_eq!(hex(&c["a"], "a"), 0x1122_33ff);
    assert_eq!(hex(&c["b"], "b"), 0x1122_3344);
    let t = grouped_section(&lines, "typography").unwrap();
    assert_eq!(t["t"]["fontWeight"], "700");
    assert_eq!(flat_section(&lines, "rounded").unwrap()["1"], "4px");
}

/// Shapes the comparison would not see fail loudly (review finding 7).
#[test]
fn g3_parser_rejects_unknown_shapes() {
    let cases: &[(&[&str], &str, &str)] = &[
        // A nested group inside a flat section.
        (
            &["colors:", "  a: '#112233'", "  group:", "    b: '#445566'"],
            "colors",
            "colors.group: nested group",
        ),
        // An unquoted hex reads as empty (YAML comment) and is not a token.
        (
            &["colors:", "  a: #112233"],
            "colors",
            "colors.a: nested group or empty value",
        ),
        // A duplicate key.
        (
            &["colors:", "  a: '#112233'", "  a: '#445566'"],
            "colors",
            "colors: duplicate key `a`",
        ),
        // A one-line typography entry.
        (
            &[
                "typography:",
                "  t:",
                "    fontSize: 13px",
                "  inline: 13px",
            ],
            "typography",
            "typography.inline: one-line entry",
        ),
        // A duplicate typography field.
        (
            &[
                "typography:",
                "  t:",
                "    fontSize: 13px",
                "    fontSize: 14px",
            ],
            "typography",
            "typography.t: duplicate field `fontSize`",
        ),
        // A deeper nesting level.
        (
            &["typography:", "  t:", "    font:", "      size: 13px"],
            "typography",
            "typography: unsupported indentation (6)",
        ),
    ];
    for (lines, name, want) in cases {
        let got = if *name == "typography" {
            grouped_section(lines, name).map(|_| ())
        } else {
            flat_section(lines, name).map(|_| ())
        };
        let err = got.expect_err(&format!("accepted: {lines:?}"));
        assert!(
            err.starts_with(want),
            "{lines:?}: got `{err}`, want `{want}...`"
        );
    }
}
