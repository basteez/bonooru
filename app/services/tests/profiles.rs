//! C.31 (Story 5.1 AC 3–5, 9, 10): the profile adapter
//! `ocb_services::profiles`. The embedded bundled default, user copies read
//! from a temp config dir (never the real one), the settings file kept
//! apart, and the `profile` log lines. No clock, no thread; a fresh temp
//! dir per test (E4-A4).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::fs;
use std::path::PathBuf;

use ocb_core::profile::Source;
use ocb_services::profiles::{self, BUNDLED, MAX_PROFILE_BYTES};

fn temp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ocb-profiles-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    d
}

/// AC 3: the embedded text is the repo file, byte for byte, and valid.
#[test]
fn c31_bundled_is_the_repo_file() {
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../profiles/oot-switch2-1.1.1.toml");
    assert_eq!(BUNDLED, fs::read_to_string(file).unwrap());
    let r = profiles::bundled_only();
    assert_eq!(r.source, Source::Bundled);
    assert!(r.events.is_empty());
    assert_eq!(r.profile.name, "OoT Switch 2");
}

/// AC 4, 5: a missing dir is no user copy; files are read in name order;
/// only `*.toml`; an unreadable entry is refused, not skipped.
#[test]
fn c31_load_from_dir() {
    let d = temp_dir("load");
    // Missing directory: bundled, no event.
    let r = profiles::load_from(&d.join("profiles"));
    assert_eq!(r.source, Source::Bundled);
    assert!(r.events.is_empty());

    let p = d.join("profiles");
    fs::create_dir_all(&p).unwrap();
    let mine = BUNDLED.replacen("max_sustain_ms = 6000", "max_sustain_ms = 5000", 1);
    fs::write(p.join("mine.toml"), &mine).unwrap();
    fs::write(p.join("broken.toml"), "schema = = 1").unwrap();
    fs::write(p.join("notes.txt"), "not a profile").unwrap();
    // A directory named like a profile cannot be read as a file.
    fs::create_dir_all(p.join("dir.toml")).unwrap();
    let r = profiles::load_from(&p);
    assert_eq!(
        r.source,
        Source::User {
            file: "mine.toml".to_owned()
        }
    );
    assert_eq!(r.profile.timing.max_sustain_ms, 5000);
    let lines = profiles::log_lines(&r);
    assert_eq!(lines.len(), 3, "{lines:?}");
    assert!(
        lines[0].starts_with("refused file=\"broken.toml\" reason=parse detail=\"line 1: "),
        "{lines:?}"
    );
    assert!(
        lines[1].starts_with("refused file=\"dir.toml\" reason=read detail=\""),
        "{lines:?}"
    );
    assert_eq!(
        lines[2],
        "loaded name=\"OoT Switch 2\" game_version=\"1.1.1\" schema=1 source=user file=\"mine.toml\""
    );
    // File names only, never a path.
    for l in &lines {
        assert!(!l.contains(&*d.to_string_lossy()), "{l}");
    }
    fs::remove_dir_all(&d).unwrap();
}

/// AC 10: the exact line formats, including ignored files and a long or
/// quoted detail.
#[test]
fn c31_log_lines() {
    let d = temp_dir("lines");
    let p = d.join("profiles");
    fs::create_dir_all(&p).unwrap();
    let other = BUNDLED.replacen("game_version = \"1.1.1\"", "game_version = \"1.0.0\"", 1);
    fs::write(p.join("a.toml"), &other).unwrap();
    fs::write(p.join("b.toml"), BUNDLED).unwrap();
    fs::write(p.join("c.toml"), BUNDLED).unwrap();
    let bad = BUNDLED.replacen("min_press_frames = 3", "min_press_frames = 1", 1);
    fs::write(p.join("d.toml"), &bad).unwrap();
    let r = profiles::load_from(&p);
    assert_eq!(
        profiles::log_lines(&r),
        vec![
            "ignored file=\"a.toml\" reason=other_identity".to_owned(),
            "ignored file=\"c.toml\" reason=duplicate".to_owned(),
            "refused file=\"d.toml\" reason=invalid detail=\"timing.min_press_frames = 1: 16666 us is below 2 x max(report period, frame) = 33332 us\"".to_owned(),
            "loaded name=\"OoT Switch 2\" game_version=\"1.1.1\" schema=1 source=user file=\"b.toml\"".to_owned(),
        ]
    );
    assert_eq!(
        profiles::log_lines(&profiles::bundled_only()),
        vec![
            "loaded name=\"OoT Switch 2\" game_version=\"1.1.1\" schema=1 source=bundled file=-"
                .to_owned()
        ]
    );
    // A detail is one line, quotes escaped, at most 160 characters.
    let long = format!("\"x\"\n{}", "a".repeat(300));
    let one = profiles::detail(&long);
    assert!(one.chars().count() <= 160, "{one}");
    assert!(!one.contains('\n'));
    assert!(one.contains("\\\"x\\\""), "{one}");
    fs::remove_dir_all(&d).unwrap();
}

/// AC 9, D4: the settings file sits next to the profiles dir, not in it.
#[test]
fn c31_settings_apart() {
    let (Some(profiles_dir), Some(settings)) =
        (profiles::user_profiles_dir(), profiles::settings_path())
    else {
        // No home directory on this runner: nothing to compare.
        return;
    };
    assert!(
        !settings.starts_with(&profiles_dir),
        "{settings:?} in {profiles_dir:?}"
    );
    assert_eq!(settings.file_name().unwrap(), "settings.toml");
    assert_eq!(profiles_dir.file_name().unwrap(), "profiles");
    assert_eq!(settings.parent(), profiles_dir.parent());
    assert_eq!(
        Some(profiles_dir.parent().unwrap().to_path_buf()),
        profiles::config_dir()
    );
}

/// Review 2026-11-28: candidates (any-case `.toml`, no dot-files), a file
/// too large, a profiles path that is not a directory, a FIFO (never
/// blocks), and a file name that cannot forge a log line.
#[test]
fn c31_candidates_and_hostile_files() {
    assert!(profiles::is_candidate("mine.toml"));
    assert!(profiles::is_candidate("MINE.TOML"));
    assert!(profiles::is_candidate("a.Toml"));
    assert!(!profiles::is_candidate("._mine.toml"));
    assert!(!profiles::is_candidate(".#mine.toml"));
    assert!(!profiles::is_candidate("mine.toml.bak"));
    assert!(!profiles::is_candidate("toml"));

    let d = temp_dir("hostile");
    let p = d.join("profiles");
    fs::create_dir_all(&p).unwrap();
    let mine = BUNDLED.replacen("max_sustain_ms = 6000", "max_sustain_ms = 5000", 1);
    fs::write(p.join("UPPER.TOML"), &mine).unwrap();
    fs::write(p.join("._junk.toml"), "\0\0").unwrap();
    let big = "#".repeat(usize::try_from(MAX_PROFILE_BYTES).unwrap() + 1);
    fs::write(p.join("big.toml"), big).unwrap();
    #[cfg(unix)]
    {
        let fifo = p.join("pipe.toml");
        let ok = std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success();
        assert!(ok, "mkfifo");
        // A name with a newline: it must not start a line of its own.
        fs::write(p.join("x\nprofile loaded source=user.toml"), "schema = = 1").unwrap();
    }
    let r = profiles::load_from(&p);
    assert_eq!(
        r.source,
        Source::User {
            file: "UPPER.TOML".to_owned()
        }
    );
    let lines = profiles::log_lines(&r);
    assert!(
        lines.iter().any(|l| l.starts_with(
            "refused file=\"big.toml\" reason=read detail=\"larger than 1048576 bytes\""
        )),
        "{lines:?}"
    );
    assert!(!lines.iter().any(|l| l.contains("._junk")), "{lines:?}");
    #[cfg(unix)]
    {
        assert!(
            lines.iter().any(
                |l| l == "refused file=\"pipe.toml\" reason=read detail=\"not a regular file\""
            ),
            "{lines:?}"
        );
        assert!(
            lines.iter().any(|l| l
                .starts_with("refused file=\"x profile loaded source=user.toml\" reason=parse")),
            "{lines:?}"
        );
    }
    for l in &lines {
        assert!(!l.contains('\n'), "{l:?}");
    }
    // `profiles` is a regular file: refused as `read`, never silent.
    let q = d.join("q");
    fs::create_dir_all(&q).unwrap();
    fs::write(q.join("profiles"), "x").unwrap();
    let r = profiles::load_from(&q.join("profiles"));
    assert_eq!(r.source, Source::Bundled);
    let lines = profiles::log_lines(&r);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(
        lines[0].starts_with("refused file=\"profiles\" reason=read detail=\""),
        "{lines:?}"
    );
    // Control characters and line separators become spaces.
    assert_eq!(profiles::detail("a\u{1b}b\0c\u{2028}d"), "a b c d");
    fs::remove_dir_all(&d).unwrap();
}
