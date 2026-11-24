//! C.4: the `ocb-core` purity gate (AD-18, Story 3.2 D10). The core has no
//! GUI, no threads, no I/O, no clocks, no `unsafe` and no printing; its
//! `[dependencies]` stay empty. Checked on the real tree and, as a self-test,
//! on fixtures (each bad fixture must produce the hit it names, the good one
//! passes), like the firmware gates F.13, F.18 and F.30.
//!
//! The scan works on a token stream (identifiers, `::`, single characters),
//! so whitespace, line breaks and grouped imports do not hide anything
//! (review fix of Story 3.2). Three rules:
//! - banned identifiers anywhere (comments and strings are not exempt);
//! - banned macros (`name!`);
//! - every `std`, `core` or `alloc` root must be followed by `::` and an
//!   allowed module, or by `::{` (whose contents the other rules see);
//!   `std as …`, `use std;` and a disallowed module (`std::fs`) are hits;
//!   the bare word in prose is not.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::fs;
use std::path::{Path, PathBuf};

/// Identifiers that name I/O, threads, processes, clocks or FFI.
const BANNED_WORDS: &[&str] = &[
    "fs",
    "io",
    "net",
    "os",
    "thread",
    "spawn",
    "mpsc",
    "process",
    "Command",
    "File",
    "env", // `env!` (compile-time, from build.rs) is allowed
    "Instant",
    "SystemTime",
    "stdin",
    "stdout",
    "stderr",
    "unsafe",
    "extern",
    "asm",
];

/// Macros that print or read files or the environment at build time.
const BANNED_MACROS: &[&str] = &[
    "print",
    "println",
    "eprint",
    "eprintln",
    "dbg",
    "include",
    "include_bytes",
    "include_str",
    "option_env",
];

/// The only modules a `std` / `core` / `alloc` path may name.
const ALLOWED_MODULES: &[&str] = &[
    "fmt",
    "error",
    "collections",
    "vec",
    "string",
    "boxed",
    "borrow",
    "ops",
    "convert",
    "iter",
    "cmp",
    "num",
    "mem",
    "array",
    "slice",
    "str",
    "char",
    "hash",
    "marker",
    "default",
    "option",
    "result",
];

const ROOTS: &[&str] = &["std", "core", "alloc"];

/// Crates the core must never depend on, in any dependency table.
const BANNED_CRATES: &[&str] = &["egui", "eframe", "serialport"];

fn tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_whitespace() {
            continue;
        }
        if c.is_ascii_alphanumeric() || c == '_' {
            let mut id = String::from(c);
            while let Some(&n) = chars.peek() {
                if n.is_ascii_alphanumeric() || n == '_' {
                    id.push(n);
                    chars.next();
                } else {
                    break;
                }
            }
            out.push(id);
        } else if c == ':' && chars.peek() == Some(&':') {
            chars.next();
            out.push("::".to_owned());
        } else {
            out.push(c.to_string());
        }
    }
    out
}

/// The rule hits in `text` (deduplicated, in order of first appearance).
fn scan(text: &str) -> Vec<String> {
    let t = tokens(text);
    let mut hits: Vec<String> = Vec::new();
    let mut hit = |h: String| {
        if !hits.contains(&h) {
            hits.push(h);
        }
    };
    for (i, tok) in t.iter().enumerate() {
        let next = t.get(i + 1).map(String::as_str);
        let after = t.get(i + 2).map(String::as_str);
        if BANNED_WORDS.contains(&tok.as_str()) && !(tok == "env" && next == Some("!")) {
            hit(tok.clone());
        }
        if BANNED_MACROS.contains(&tok.as_str()) && next == Some("!") {
            hit(format!("{tok}!"));
        }
        if ROOTS.contains(&tok.as_str()) {
            // A root inside a longer path (`crate::std`) is not a root.
            let prev = i.checked_sub(1).and_then(|p| t.get(p)).map(String::as_str);
            if prev == Some("::") {
                continue;
            }
            match (next, after) {
                (Some("::"), Some("{")) => {}
                (Some("::"), Some(m)) if ALLOWED_MODULES.contains(&m) => {}
                (Some("::"), Some(m)) => hit(format!("{tok}::{m}")),
                (Some("as"), _) => hit(format!("{tok} as")),
                (n, _) if prev == Some("use") => hit(format!("use {tok}{}", n.unwrap_or(""))),
                // Prose ("the GUI-free core of …") is not a path.
                _ => {}
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

/// A TOML table header, normalised: comment dropped, spaces and quotes
/// removed (`[ "dependencies" ] # x` → `dependencies`).
fn header(line: &str) -> Option<String> {
    let line = line.split('#').next().unwrap_or("").trim();
    let inner = line.strip_prefix('[')?.strip_suffix(']')?;
    Some(
        inner
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '"' && *c != '\'')
            .collect(),
    )
}

fn is_dep_table(name: &str) -> bool {
    name.split('.').any(|seg| seg.ends_with("dependencies"))
}

/// Problems in a crate manifest: anything in `[dependencies]` (or a dotted
/// `dependencies.*` key, or a target dependency table), and a banned crate
/// in any dependency table, dev and build included.
fn manifest_problems(toml: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let mut section = String::new();
    for (i, raw) in toml.lines().enumerate() {
        if let Some(h) = header(raw) {
            if h.starts_with("target.") && is_dep_table(&h) {
                problems.push(format!("line {}: target dependency table [{h}]", i + 1));
            }
            section = h;
            continue;
        }
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let key: String = line
            .split('=')
            .next()
            .unwrap_or("")
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '"' && *c != '\'')
            .collect();
        let runtime = section == "dependencies"
            || section.starts_with("dependencies.")
            || (section.is_empty() && key.starts_with("dependencies."));
        if runtime {
            problems.push(format!("line {}: runtime dependency `{line}`", i + 1));
        }
        let in_dep_table = is_dep_table(&section) || key.contains("dependencies.");
        let crate_name = if section.is_empty() || !is_dep_table(&section) {
            key.rsplit('.').next().unwrap_or("").to_owned()
        } else {
            key.split('.').next().unwrap_or("").to_owned()
        };
        let table_crate = section.rsplit('.').next().unwrap_or("");
        if (in_dep_table && BANNED_CRATES.contains(&crate_name.as_str()))
            || (section.contains("dependencies.") && BANNED_CRATES.contains(&table_crate))
        {
            problems.push(format!("line {}: banned crate in `{line}`", i + 1));
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
        for h in scan(&fs::read_to_string(f).unwrap()) {
            problems.push(format!("{}: `{h}`", f.display()));
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
fn c4_every_bad_fixture_gives_its_own_hit() {
    let dir = core_dir().join("tests/fixtures/purity/bad");
    let mut expected_hits = Vec::new();
    for e in fs::read_dir(&dir).unwrap().flatten() {
        let text = fs::read_to_string(e.path()).unwrap();
        let (first, code) = text.split_once('\n').unwrap();
        let want = first.strip_prefix("// expect: ").unwrap().trim().to_owned();
        let hits = scan(code);
        assert!(
            hits.contains(&want),
            "{}: expected hit `{want}`, got {hits:?}",
            e.path().display()
        );
        expected_hits.push(want);
    }
    // Every word and macro rule is proven by at least one fixture.
    for w in BANNED_WORDS {
        assert!(expected_hits.iter().any(|h| h == w), "no fixture for `{w}`");
    }
    for m in BANNED_MACROS {
        let h = format!("{m}!");
        assert!(expected_hits.contains(&h), "no fixture for `{h}`");
    }
    for root_rule in ["std as", "std::fs", "core::time"] {
        assert!(
            expected_hits.iter().any(|h| h == root_rule),
            "no fixture for `{root_rule}`"
        );
    }
}

#[test]
fn c4_good_fixture_passes() {
    let text = fs::read_to_string(core_dir().join("tests/fixtures/purity/good.rs.txt")).unwrap();
    assert_eq!(scan(&text), Vec::<String>::new());
    // Not hits: identifier boundaries, allowed modules, the compile-time env!.
    for ok in [
        "#![forbid(unsafe_code)]",
        "let my_fs = 1; let reprint = 2; let threads = 3;",
        "use std::fmt; use core::ops::Range; impl std::error::Error for X {}",
        "use std::{fmt, collections::BTreeMap};",
        "pub const V: &str = env!(\"OCB_APP_VERSION\");",
        "use crate::protocol::frame;",
        "//! the GUI-free core of the app, `ocb-core`",
    ] {
        assert_eq!(scan(ok), Vec::<String>::new(), "{ok}");
    }
}

#[test]
fn c4_manifest_rules() {
    let clean = "[package]\nname = \"x\"\ndescription = \"no egui here\"\n\n[dependencies]\n# none\n\n[dev-dependencies]\nproptest = \"1\"\n";
    assert_eq!(manifest_problems(clean), Vec::<String>::new());
    for bad in [
        "[dependencies]\ncrc = \"3\"\n",
        "[dependencies] # runtime\ncrc = \"3\"\n",
        "[ dependencies ]\ncrc = \"3\"\n",
        "[\"dependencies\"]\ncrc = \"3\"\n",
        "[dependencies.crc]\nversion = \"3\"\n",
        "dependencies.crc = \"3\"\n[package]\n",
        "[target.'cfg(unix)'.dependencies]\nlibc = \"0.2\"\n",
        "[dev-dependencies]\negui = \"0.37\"\n",
        "[build-dependencies]\neframe = \"0.37\"\n",
        "[dev-dependencies.serialport]\nversion = \"4\"\n",
    ] {
        assert!(!manifest_problems(bad).is_empty(), "not flagged:\n{bad}");
    }
}
