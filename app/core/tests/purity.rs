//! C.4: the `ocb-core` purity gate (AD-18, Story 3.2 D10). The core has no
//! GUI, no threads, no I/O, no clocks, no `unsafe` and no printing; its
//! `[dependencies]` stay empty. Checked on the real tree and, as a self-test,
//! on fixtures (every bad fixture flagged, the good one passes), like the
//! firmware gates F.13, F.18 and F.30. Comments are not exempt.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::fs;
use std::path::{Path, PathBuf};

/// Banned tokens, matched on identifier boundaries.
const BANNED: &[&str] = &[
    "std::thread",
    "std::sync::mpsc",
    "std::fs",
    "std::net",
    "std::process",
    "std::env",
    "std::io",
    "std::time::Instant",
    "std::time::SystemTime",
    "thread::spawn",
    "unsafe",
    "extern \"C\"",
    "println!",
    "eprintln!",
    "print!",
    "dbg!",
    "include_bytes!",
    "include_str!",
];

/// Crates the core must never name in its manifest.
const BANNED_CRATES: &[&str] = &["egui", "eframe", "serialport"];

fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// The banned tokens found in `text`, with their 1-based line numbers.
fn scan(text: &str) -> Vec<(usize, &'static str)> {
    let mut hits = Vec::new();
    for (i, line) in text.lines().enumerate() {
        for &tok in BANNED {
            let mut from = 0;
            while let Some(pos) = line[from..].find(tok) {
                let start = from + pos;
                let end = start + tok.len();
                let before_ok = !line[..start].chars().next_back().is_some_and(is_ident);
                let last_is_ident = tok.chars().next_back().is_some_and(is_ident);
                let after_ok = !last_is_ident || !line[end..].chars().next().is_some_and(is_ident);
                if before_ok && after_ok {
                    hits.push((i + 1, tok));
                    break;
                }
                from = start + 1;
            }
        }
    }
    hits
}

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            rs_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// Problems in a crate manifest: a non-empty `[dependencies]`, any
/// target-specific dependency table, or a banned crate name anywhere.
fn manifest_problems(toml: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let mut section = String::new();
    for (i, raw) in toml.lines().enumerate() {
        let line = raw.trim();
        if line.starts_with('[') {
            section = line.to_owned();
            if section.starts_with("[target.") && section.contains("dependencies") {
                problems.push(format!("line {}: target dependency table {section}", i + 1));
            }
            continue;
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if section == "[dependencies]" || section.starts_with("[dependencies.") {
            problems.push(format!("line {}: runtime dependency `{line}`", i + 1));
        }
    }
    for c in BANNED_CRATES {
        if toml
            .lines()
            .any(|l| !l.trim_start().starts_with('#') && l.contains(c))
        {
            problems.push(format!("banned crate `{c}` named"));
        }
    }
    problems
}

fn core_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn c4_real_tree_is_pure() {
    let mut files = Vec::new();
    rs_files(&core_dir().join("src"), &mut files);
    assert!(files.len() >= 8, "found only {} source files", files.len());
    let mut problems = Vec::new();
    for f in &files {
        for (line, tok) in scan(&fs::read_to_string(f).unwrap()) {
            problems.push(format!("{}:{line}: `{tok}`", f.display()));
        }
    }
    let manifest = fs::read_to_string(core_dir().join("Cargo.toml")).unwrap();
    problems.extend(manifest_problems(&manifest));
    assert!(
        problems.is_empty(),
        "ocb-core is not pure:\n{}",
        problems.join("\n")
    );
}

#[test]
fn c4_every_bad_fixture_is_flagged() {
    let dir = core_dir().join("tests/fixtures/purity/bad");
    let mut n = 0;
    for e in fs::read_dir(&dir).unwrap().flatten() {
        let text = fs::read_to_string(e.path()).unwrap();
        // The first line is a comment naming the fixture; scan the code only.
        let code: String = text.lines().skip(1).collect::<Vec<_>>().join("\n");
        assert!(
            !scan(&code).is_empty(),
            "not flagged: {}",
            e.path().display()
        );
        n += 1;
    }
    assert_eq!(n, BANNED.len(), "one bad fixture per banned token");
}

#[test]
fn c4_good_fixture_passes() {
    let text = fs::read_to_string(core_dir().join("tests/fixtures/purity/good.rs.txt")).unwrap();
    assert!(scan(&text).is_empty());
    // Identifier boundaries: these are not hits.
    assert!(scan("#![forbid(unsafe_code)]\nlet my_std::io = 1; let reprint!() = 0;").is_empty());
}

#[test]
fn c4_manifest_rules() {
    assert!(manifest_problems("[package]\nname = \"x\"\n\n[dependencies]\n# none\n\n[dev-dependencies]\nproptest = \"1\"\n").is_empty());
    assert!(!manifest_problems("[dependencies]\ncrc = \"3\"\n").is_empty());
    assert!(!manifest_problems("[dependencies.crc]\nversion = \"3\"\n").is_empty());
    assert!(!manifest_problems("[target.'cfg(unix)'.dependencies]\nlibc = \"0.2\"\n").is_empty());
    assert!(!manifest_problems("[dev-dependencies]\negui = \"0.37\"\n").is_empty());
}
