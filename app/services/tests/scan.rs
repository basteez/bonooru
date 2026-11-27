//! C.25: source scans (Story 4.4 AC 1, 7). The app never escalates
//! privileges: no `sudo`, `pkexec`, `doas`, `setuid`, `polkit` or
//! `Command::new` in the code of `ocb-services`, `ocb-app` and `ocb-probe`
//! (comments and string literals are skipped: the permission message tells
//! the owner to run `sudo usermod` himself). And `ocb-services` has no GUI
//! dependency. The scanner is self-tested on good and bad snippets.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::fs;
use std::path::{Path, PathBuf};

const BANNED: &[&str] = &["sudo", "pkexec", "doas", "setuid", "polkit"];

/// The code of a Rust source with comments, strings and char literals
/// replaced by spaces.
fn code_only(src: &str) -> String {
    let b: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        let next = b.get(i + 1).copied();
        if c == '/' && next == Some('/') {
            while i < b.len() && b[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && next == Some('*') {
            let mut depth = 0;
            while i < b.len() {
                if b[i] == '/' && b.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                } else if b[i] == '*' && b.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            out.push(' ');
            continue;
        }
        // Raw strings: r"..", r#".."#, br"..".
        let raw_start = if c == 'r' && matches!(next, Some('"' | '#')) {
            Some(i + 1)
        } else if c == 'b' && next == Some('r') && matches!(b.get(i + 2), Some('"' | '#')) {
            Some(i + 2)
        } else {
            None
        };
        let prev_ident = i > 0 && (b[i - 1].is_alphanumeric() || b[i - 1] == '_');
        if let Some(mut j) = raw_start.filter(|_| !prev_ident) {
            let mut hashes = 0;
            while b.get(j) == Some(&'#') {
                hashes += 1;
                j += 1;
            }
            if b.get(j) == Some(&'"') {
                j += 1;
                'raw: while j < b.len() {
                    if b[j] == '"' && (0..hashes).all(|k| b.get(j + 1 + k) == Some(&'#')) {
                        j += 1 + hashes;
                        break 'raw;
                    }
                    j += 1;
                }
                i = j;
                out.push(' ');
                continue;
            }
        }
        if c == '"' {
            i += 1;
            while i < b.len() && b[i] != '"' {
                if b[i] == '\\' {
                    i += 1;
                }
                i += 1;
            }
            i += 1;
            out.push(' ');
            continue;
        }
        // A char literal ('a', '\n'), not a lifetime ('a).
        if c == '\'' {
            let close = if next == Some('\\') {
                b.iter()
                    .skip(i + 2)
                    .position(|&x| x == '\'')
                    .map(|p| i + 2 + p)
            } else if b.get(i + 2) == Some(&'\'') {
                Some(i + 2)
            } else {
                None
            };
            if let Some(end) = close {
                i = end + 1;
                out.push(' ');
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

fn tokens(code: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = code.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_alphanumeric() || c == '_' {
            cur.push(c);
            continue;
        }
        if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        if c == ':' && chars.peek() == Some(&':') {
            chars.next();
            out.push("::".to_owned());
        } else if !c.is_whitespace() {
            out.push(c.to_string());
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// The privilege-escalation hits in `src`.
fn hits(src: &str) -> Vec<String> {
    let t = tokens(&code_only(src));
    let mut out = Vec::new();
    for (i, tok) in t.iter().enumerate() {
        if BANNED.iter().any(|b| tok.eq_ignore_ascii_case(b)) {
            out.push(tok.clone());
        }
        if tok == "Command"
            && t.get(i + 1).map(String::as_str) == Some("::")
            && t.get(i + 2).map(String::as_str) == Some("new")
        {
            out.push("Command::new".to_owned());
        }
    }
    out
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

fn app_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

#[test]
fn c25_scanner_self_test() {
    let good = r##"
        // sudo in a comment is fine; so is /* pkexec */ here.
        /// doc: run `sudo usermod -aG dialout $USER`
        let m = "Run: sudo usermod -aG {group} $USER";
        let r = r#"pkexec "quoted" setuid"#;
        let c = '"'; let l: &'static str = "doas";
        let commander = 1; fn new_command() {}
    "##;
    assert_eq!(hits(good), Vec::<String>::new());
    let bad: &[(&str, &str)] = &[
        ("fn f() { sudo(); }", "sudo"),
        ("let x = pkexec;", "pkexec"),
        ("use doas::run;", "doas"),
        ("libc::setuid(0);", "setuid"),
        ("mod polkit {}", "polkit"),
        ("std::process::Command::new(\"ls\")", "Command::new"),
        ("Command\n  ::\n  new(x)", "Command::new"),
        ("let s = \"a\"; sudo()", "sudo"),
        ("let c = '\\''; doas()", "doas"),
    ];
    for (src, want) in bad {
        assert_eq!(hits(src), vec![(*want).to_owned()], "{src}");
    }
}

#[test]
fn c25_no_privilege_escalation() {
    let mut files = Vec::new();
    for d in ["services/src", "gui/src", "tools/ocb-probe/src"] {
        rs_files(&app_dir().join(d), &mut files);
    }
    assert!(files.len() >= 10, "found only {} files", files.len());
    let mut problems = Vec::new();
    for f in &files {
        for h in hits(&fs::read_to_string(f).unwrap()) {
            problems.push(format!("{}: {h}", f.display()));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn c25_services_has_no_gui_dependency() {
    let toml = fs::read_to_string(app_dir().join("services/Cargo.toml")).unwrap();
    for line in toml.lines() {
        let key = line.split('#').next().unwrap().trim();
        for banned in ["egui", "eframe", "winit", "wgpu"] {
            assert!(
                !key.starts_with(banned),
                "ocb-services depends on {banned}: {line}"
            );
        }
    }
}
