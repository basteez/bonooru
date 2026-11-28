//! C.30 (Story 5.1): the mapping profile module `ocb_core::profile`. The
//! bundled default parses and keeps its measured values; every validation
//! rule refuses its bad value and names the field; schema and migration;
//! the user-copy resolution (override, refusal, ignore, duplicate); the
//! AD-15 timing floor; the one-parser source scan; a fuzz case. Pure: text
//! in, values out (the bundled file is read here with `std::fs`, tests are
//! not purity-scanned). No clock, no thread.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

use ocb_core::profile::{
    self, Button, Dpad, IgnoreReason, Migration, ProfileError, ProfileEvent, Source, StickDir,
    UserFile,
};
use proptest::prelude::*;

fn app_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn bundled() -> String {
    std::fs::read_to_string(app_dir().join("profiles/oot-switch2-1.1.1.toml")).unwrap()
}

/// The bundled text with one `from` → `to` replacement (must match once).
fn edit(from: &str, to: &str) -> String {
    let b = bundled();
    assert_eq!(b.matches(from).count(), 1, "`{from}` not unique");
    b.replacen(from, to, 1)
}

fn invalid_field(text: &str) -> String {
    match profile::parse(text) {
        Err(ProfileError::Invalid { field, .. }) => field,
        other => panic!("expected Invalid, got {other:?}"),
    }
}

fn user(name: &str, text: &str) -> UserFile {
    UserFile {
        name: name.to_owned(),
        text: Ok(text.to_owned()),
    }
}

/// AC 3: the bundled default parses, with its measured values (literals
/// here, never the module's own constants).
#[test]
fn c30_bundled_values() {
    let p = profile::parse(&bundled()).unwrap();
    assert_eq!(p.schema, 1);
    assert_eq!(p.name, "OoT Switch 2");
    assert_eq!(p.game_version, "1.1.1");
    assert_eq!(p.frame_rate, 60);
    assert_eq!(
        [Button::L, Button::R, Button::A, Button::Y, Button::X].map(|b| p.buttons.pitch(b)),
        [74, 77, 81, 83, 86]
    );
    assert_eq!((p.modifiers.dpad_up, p.modifiers.dpad_down), (1, -1));
    assert_eq!((p.modifiers.stick_up, p.modifiers.stick_down), (2, -2));
    assert_eq!(
        (
            p.modifiers.stick_up_ly,
            p.modifiers.stick_down_ly,
            p.modifiers.stick_lx
        ),
        (0x00, 0xFF, 0x80)
    );
    assert_eq!((p.range.low, p.range.high), (71, 89));
    let t = p.timing;
    assert_eq!(
        (
            t.min_press_frames,
            t.min_gap_same_button_frames,
            t.modifier_lead_frames,
            t.max_sustain_ms,
            t.start_lead_ms
        ),
        (3, 5, 1, 6000, 500)
    );
    let c = p.cost;
    assert_eq!(
        (
            c.dpad_change,
            c.stick_change,
            c.stick_through_centre,
            c.same_button_retrigger,
            c.non_natural
        ),
        (1.0, 2.0, 1.0, 1.0, 0.1)
    );
    assert_eq!(p.fingerings.len(), 45);
    assert_eq!(p.fingerings.iter().filter(|f| f.chart).count(), 19);
    for pitch in 71..=89 {
        assert!(p.fingerings_for(pitch).count() >= 1, "pitch {pitch}");
        assert_eq!(
            p.fingerings_for(pitch).filter(|f| f.chart).count(),
            1,
            "one chart fingering for {pitch}"
        );
    }
    let first = p.fingerings[0];
    assert_eq!(
        (
            first.button,
            first.dpad,
            first.stick,
            first.pitch,
            first.chart
        ),
        (Button::L, Dpad::Down, StickDir::Down, 71, true)
    );
    // Durations in µs (AD-15): frame 1_000_000 / 60 rounded down.
    assert_eq!(p.frame_us(), 16_666);
    assert_eq!(p.min_press_us(), 49_998);
    assert_eq!(p.min_gap_same_button_us(), 83_330);
    assert_eq!(p.modifier_lead_us(), 16_666);
    assert_eq!(p.max_sustain_us(), 6_000_000);
    assert_eq!(p.start_lead_us(), 500_000);
}

/// AC 8: each rule refuses its bad value and names the field.
#[test]
fn c30_validation_rules() {
    let cases: &[(&str, &str, &str)] = &[
        ("frame_rate = 60", "frame_rate = 0", "profile.frame_rate"),
        ("frame_rate = 60", "frame_rate = 241", "profile.frame_rate"),
        ("name = \"OoT Switch 2\"", "name = \"\"", "profile.name"),
        (
            "game_version = \"1.1.1\"",
            "game_version = \" \"",
            "profile.game_version",
        ),
        (
            "game_version = \"1.1.1\"",
            "game_version = \"1.1.1 \"",
            "profile.game_version",
        ),
        (
            "name = \"OoT Switch 2\"",
            "name = \" OoT Switch 2\"",
            "profile.name",
        ),
        ("L = 74", "L = 128", "buttons.L"),
        ("X = 86", "X = -1", "buttons.X"),
        ("dpad_up = 1 ", "dpad_up = 13 ", "modifiers.dpad_up"),
        (
            "stick_down = -2",
            "stick_down = -13",
            "modifiers.stick_down",
        ),
        ("stick_lx = 0x80", "stick_lx = 256", "modifiers.stick_lx"),
        ("low = 71", "low = 89", "range.low"),
        ("high = 89", "high = 128", "range.high"),
        (
            "min_press_frames = 3",
            "min_press_frames = 0",
            "timing.min_press_frames",
        ),
        (
            "min_press_frames = 3",
            "min_press_frames = 1",
            "timing.min_press_frames",
        ),
        (
            "min_gap_same_button_frames = 5",
            "min_gap_same_button_frames = 1",
            "timing.min_gap_same_button_frames",
        ),
        (
            "modifier_lead_frames = 1",
            "modifier_lead_frames = -1",
            "timing.modifier_lead_frames",
        ),
        (
            "max_sustain_ms = 6000",
            "max_sustain_ms = 40",
            "timing.max_sustain_ms",
        ),
        (
            "start_lead_ms = 500",
            "start_lead_ms = -1",
            "timing.start_lead_ms",
        ),
        ("dpad_change = 1", "dpad_change = -1", "cost.dpad_change"),
        ("non_natural = 0.1", "non_natural = nan", "cost.non_natural"),
        ("non_natural = 0.1", "non_natural = inf", "cost.non_natural"),
    ];
    for (from, to, field) in cases {
        assert_eq!(invalid_field(&edit(from, to)), *field, "{to}");
    }
    // A fingering outside the range.
    let out = edit(
        "# F6, measured 1428.2 Hz (chart)\nbutton = \"X\"\ndpad = \"up\"\nstick = \"up\"\npitch = 89",
        "# F6, measured 1428.2 Hz (chart)\nbutton = \"X\"\ndpad = \"up\"\nstick = \"up\"\npitch = 90",
    );
    assert_eq!(invalid_field(&out), "fingerings[44].pitch");
    // A duplicate (button, dpad, stick): the F6 row turned into the first
    // row's combination (L, down, down) with F6's pitch is out of place
    // and duplicates row 0.
    let dup = edit(
        "button = \"X\"\ndpad = \"up\"\nstick = \"up\"\npitch = 89",
        "button = \"L\"\ndpad = \"down\"\nstick = \"down\"\npitch = 71",
    );
    assert_eq!(invalid_field(&dup), "fingerings[44]");
    // A pitch of the range with no fingering: drop the only F6 row.
    let b = bundled();
    let at = b.rfind("[[fingerings]]").unwrap();
    let uncovered = b[..at].to_owned();
    assert_eq!(invalid_field(&uncovered), "fingerings");
    // Every error names its value and rule in its text.
    let e = profile::parse(&edit("min_press_frames = 3", "min_press_frames = 1")).unwrap_err();
    let text = e.to_string();
    assert!(text.contains("timing.min_press_frames"), "{text}");
    assert!(text.contains("= 1"), "{text}");
    assert!(text.contains("33332"), "{text}");
    assert_eq!(e.reason(), "invalid");
}

/// AC 8, D5: unknown and missing keys and wrong types are refused.
#[test]
fn c30_shape() {
    for (from, to) in [
        ("min_press_frames = 3", "min_pres_frames = 3"),
        (
            "pitch = 71\nchart = true",
            "pitch = 71\nchart = true\nextra = 1",
        ),
        ("start_lead_ms = 500", ""),
        ("frame_rate = 60", "frame_rate = \"60\""),
        ("button = \"L\"", "button = \"ZL\""),
        ("dpad = \"down\"", "dpad = \"left\""),
    ] {
        // The first occurrence (some keys repeat in every fingering).
        let text = bundled().replacen(from, to, 1);
        let e = profile::parse(&text).unwrap_err();
        assert_eq!(e.reason(), "invalid", "{to}: {e:?}");
        assert!(
            matches!(e, ProfileError::Invalid { line: Some(_), .. }),
            "{to}: {e:?}"
        );
    }
    // An extra key in every table, and at the top, keeping every required
    // key: refused (D5, `deny_unknown_fields` on each shape).
    let b = bundled();
    let mut spots: Vec<usize> = [
        "[profile]",
        "[buttons]",
        "[modifiers]",
        "[range]",
        "[timing]",
        "[cost]",
        "[[fingerings]]",
    ]
    .iter()
    .map(|h| {
        let at = b.find(&format!("\n{h}")).unwrap() + 1;
        at + b[at..].find('\n').unwrap() + 1
    })
    .collect();
    spots.push(b.find("schema = 1").unwrap());
    for at in spots {
        let text = format!("{}zz_extra = 1\n{}", &b[..at], &b[at..]);
        let e = profile::parse(&text).unwrap_err();
        assert_eq!(e.reason(), "invalid", "extra key at {at}: {e:?}");
        assert!(e.to_string().contains("zz_extra"), "{e}");
    }
    // Not TOML at all.
    let e = profile::parse("schema = = 1").unwrap_err();
    assert_eq!(e.reason(), "parse");
    assert!(
        matches!(e, ProfileError::Parse { line: Some(1), .. }),
        "{e:?}"
    );
    let e = profile::parse("\n\n[profile\n").unwrap_err();
    assert!(
        matches!(e, ProfileError::Parse { line: Some(3), .. }),
        "{e:?}"
    );
}

/// AC 6: schema check and the migration chain.
#[test]
fn c30_schema_and_migration() {
    assert_eq!(profile::SCHEMA, 1);
    for (to, found) in [
        ("schema = 2", Some(2)),
        ("schema = 0", Some(0)),
        ("schema = -1", Some(-1)),
    ] {
        let e = profile::parse(&edit("schema = 1", to)).unwrap_err();
        assert_eq!(e, ProfileError::Schema { found }, "{to}");
        assert_eq!(e.reason(), "schema");
    }
    for to in ["schema = \"1\"", "schema = 1.0", ""] {
        let e = profile::parse(&edit("schema = 1", to)).unwrap_err();
        assert_eq!(e, ProfileError::Schema { found: None }, "{to:?}");
    }
    // A test-only step 0 → 1: schema 0 had no `start_lead_ms`.
    fn add_start_lead(t: &mut toml::Table) -> Result<(), ProfileError> {
        let timing = t
            .get_mut("timing")
            .and_then(toml::Value::as_table_mut)
            .ok_or(ProfileError::Schema { found: Some(0) })?;
        timing.insert("start_lead_ms".to_owned(), toml::Value::Integer(250));
        Ok(())
    }
    let steps = [Migration {
        from: 0,
        apply: add_start_lead,
    }];
    let old = edit("schema = 1", "schema = 0").replacen("start_lead_ms = 500", "", 1);
    let p = profile::parse_with(&old, &steps).unwrap();
    assert_eq!(p.schema, 1);
    assert_eq!(p.timing.start_lead_ms, 250);
    // Without the step the same file is refused; schema 2 is never guessed.
    assert_eq!(
        profile::parse(&old).unwrap_err(),
        ProfileError::Schema { found: Some(0) }
    );
    assert_eq!(
        profile::parse_with(&edit("schema = 1", "schema = 2"), &steps).unwrap_err(),
        ProfileError::Schema { found: Some(2) }
    );
    // The real chain is empty today (schema 1 is the first).
    assert!(profile::MIGRATIONS.is_empty());
}

/// AC 7: the AD-15 floor, at load (frame time) and with a report period.
#[test]
fn c30_timing_floor() {
    // 2 frames at 60 Hz pass, 1 frame does not (above in c30_validation_rules).
    let two = edit("min_press_frames = 3", "min_press_frames = 2");
    let p = profile::parse(&two).unwrap();
    assert_eq!(p.min_press_us(), 33_332);
    // Unknown (0) or a fast report period: the frame time decides.
    assert!(p.check_report_period(0).is_ok());
    assert!(p.check_report_period(8_000).is_ok());
    assert!(p.check_report_period(16_666).is_ok());
    // 20 ms report period: floor 40 000 µs > 33 332.
    let e = p.check_report_period(20_000).unwrap_err();
    assert!(
        matches!(&e, ProfileError::Invalid { field, .. } if field == "timing.min_press_frames"),
        "{e:?}"
    );
    // The gap is checked too: min press 3 frames (49 998) fine at 24 ms
    // (floor 48 000), the gap 2 frames (33 332) not.
    let gap = edit(
        "min_gap_same_button_frames = 5",
        "min_gap_same_button_frames = 2",
    );
    let p = profile::parse(&gap).unwrap();
    let e = p.check_report_period(24_000).unwrap_err();
    assert!(
        matches!(&e, ProfileError::Invalid { field, .. } if field == "timing.min_gap_same_button_frames"),
        "{e:?}"
    );
    // The bundled default holds at 24 ms (min press 49 998 ≥ 48 000).
    assert!(
        profile::parse(&bundled())
            .unwrap()
            .check_report_period(24_000)
            .is_ok()
    );
}

/// AC 4, 5, D8: user copies.
#[test]
fn c30_resolve() {
    let b = bundled();
    // No user file: bundled.
    let r = profile::resolve(&b, &[]).unwrap();
    assert_eq!(r.source, Source::Bundled);
    assert!(r.events.is_empty());
    // A valid matching copy overrides.
    let mine = b.replacen("max_sustain_ms = 6000", "max_sustain_ms = 5000", 1);
    let r = profile::resolve(&b, &[user("mine.toml", &mine)]).unwrap();
    assert_eq!(
        r.source,
        Source::User {
            file: "mine.toml".to_owned()
        }
    );
    assert_eq!(r.profile.timing.max_sustain_ms, 5000);
    // Refused: unreadable, unparseable, schema, invalid. Bundled in use, one
    // event each, in file-name order.
    let files = [
        user(
            "d-invalid.toml",
            &b.replacen("min_press_frames = 3", "min_press_frames = 1", 1),
        ),
        user("b-parse.toml", "schema = = 1"),
        user("c-schema.toml", &b.replacen("schema = 1", "schema = 9", 1)),
        UserFile {
            name: "a-io.toml".to_owned(),
            text: Err("permission denied".to_owned()),
        },
    ];
    let r = profile::resolve(&b, &files).unwrap();
    assert_eq!(r.source, Source::Bundled);
    assert_eq!(r.profile.timing.min_press_frames, 3);
    let refused: Vec<(&str, &str)> = r
        .events
        .iter()
        .map(|e| match e {
            ProfileEvent::Refused { file, error } => (file.as_str(), error.reason()),
            ProfileEvent::Ignored { .. } => panic!("{e:?}"),
        })
        .collect();
    assert_eq!(
        refused,
        [
            ("a-io.toml", "read"),
            ("b-parse.toml", "parse"),
            ("c-schema.toml", "schema"),
            ("d-invalid.toml", "invalid")
        ]
    );
    assert_eq!(
        profile::USER_REFUSED_MESSAGE,
        "This mapping profile can't be read by this version. Defaults are in use."
    );
    assert_eq!(
        r.refusal_message(),
        Some("This mapping profile can't be read by this version. Defaults are in use.")
    );
    // A broken file next to the valid user copy in use: refused, but
    // "Defaults are in use" would be false, so no message.
    let r = profile::resolve(&b, &[user("a.toml", "schema = = 1"), user("b.toml", &mine)]).unwrap();
    assert_eq!(
        r.source,
        Source::User {
            file: "b.toml".to_owned()
        }
    );
    assert!(matches!(&r.events[..], [ProfileEvent::Refused { .. }]));
    assert_eq!(r.refusal_message(), None);
    // No refusal: no message.
    assert_eq!(profile::resolve(&b, &[]).unwrap().refusal_message(), None);
    // Another identity: ignored, not refused.
    let other = b.replacen("game_version = \"1.1.1\"", "game_version = \"1.0.0\"", 1);
    let named = b.replacen("name = \"OoT Switch 2\"", "name = \"Mine\"", 1);
    let r = profile::resolve(&b, &[user("o.toml", &other), user("n.toml", &named)]).unwrap();
    assert_eq!(r.source, Source::Bundled);
    assert_eq!(
        r.events,
        vec![
            ProfileEvent::Ignored {
                file: "n.toml".to_owned(),
                reason: IgnoreReason::OtherIdentity
            },
            ProfileEvent::Ignored {
                file: "o.toml".to_owned(),
                reason: IgnoreReason::OtherIdentity
            },
        ]
    );
    // Two valid matches: the first by name wins, the other is a duplicate.
    let five = b.replacen("max_sustain_ms = 6000", "max_sustain_ms = 5500", 1);
    let r = profile::resolve(&b, &[user("z.toml", &five), user("a.toml", &mine)]).unwrap();
    assert_eq!(
        r.source,
        Source::User {
            file: "a.toml".to_owned()
        }
    );
    assert_eq!(r.profile.timing.max_sustain_ms, 5000);
    assert_eq!(
        r.events,
        vec![ProfileEvent::Ignored {
            file: "z.toml".to_owned(),
            reason: IgnoreReason::Duplicate
        }]
    );
    // An invalid bundled text is the only error.
    assert!(profile::resolve("schema = 1", &[user("mine.toml", &mine)]).is_err());
}

/// AC 1: only `core/src/profile*` deserialises TOML.
#[test]
fn c30_one_parser() {
    fn rs(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                if p.file_name().is_some_and(|n| n == "target") {
                    continue;
                }
                rs(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    let hit = |code: &str| {
        ["toml::", "use toml", "Deserialize", "serde::"]
            .iter()
            .any(|t| code.contains(t))
    };
    assert!(hit("let t: toml::Table = toml::from_str(s)?;"));
    assert!(hit("#[derive(serde::Deserialize)]"));
    assert!(!hit("let profile = profile::parse(text);"));
    let mut files = Vec::new();
    for krate in ["core/src", "services/src", "gui/src", "sim/src", "tools"] {
        rs(&app_dir().join(krate), &mut files);
    }
    assert!(files.len() > 20);
    let mut offenders = Vec::new();
    for f in &files {
        let rel = f
            .strip_prefix(app_dir())
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if rel == "core/src/profile.rs" {
            continue;
        }
        let code = std::fs::read_to_string(f).unwrap();
        for (n, line) in code.lines().enumerate() {
            let line = line.split("//").next().unwrap_or("");
            if hit(line) {
                offenders.push(format!("{rel}:{}", n + 1));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "TOML/serde outside core/src/profile: {offenders:?}"
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// AC 11: arbitrary text never panics.
    #[test]
    fn c30_fuzz_arbitrary(text in ".{0,400}") {
        let _ = profile::parse(&text);
    }

    /// AC 11: byte edits of the bundled text never panic.
    #[test]
    fn c30_fuzz_edits(edits in prop::collection::vec((any::<prop::sample::Index>(), any::<u8>()), 1..8)) {
        let mut bytes = bundled().into_bytes();
        for (i, b) in edits {
            let at = i.index(bytes.len());
            bytes[at] = b;
        }
        let text = String::from_utf8_lossy(&bytes);
        let _ = profile::parse(&text);
    }
}
