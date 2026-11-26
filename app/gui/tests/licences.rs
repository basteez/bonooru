//! G.5: the licence texts of every font in the binary are in
//! `assets/licenses/` (AD-21, for the About view and the release notices),
//! non-empty and naming their font.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::PathBuf;

/// File, and a text it must contain (the font's name or its copyright).
const LICENCES: &[(&str, &str)] = &[
    ("OFL-1.1-CormorantGaramond.txt", "Cormorant"),
    ("OFL-1.1-NunitoSans.txt", "Nunito Sans"),
    ("Hack.txt", "Hack"),
    ("UFL-1.0-Ubuntu.txt", "UBUNTU FONT LICENCE"),
    ("MIT-emoji-icon-font.txt", "John Slegers"),
];

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/licenses")
}

fn check(dir: &std::path::Path) -> Vec<String> {
    let mut errs = Vec::new();
    for (file, needle) in LICENCES {
        let path = dir.join(file);
        match std::fs::read_to_string(&path) {
            Err(e) => errs.push(format!("{file}: {e}")),
            Ok(text) if text.trim().is_empty() => errs.push(format!("{file}: empty")),
            Ok(text) if !text.contains(needle) => {
                errs.push(format!("{file}: does not name `{needle}`"));
            }
            Ok(_) => {}
        }
    }
    errs
}

#[test]
fn g5_licence_files_exist_and_name_their_font() {
    let errs = check(&dir());
    assert!(errs.is_empty(), "licence files:\n  {}", errs.join("\n  "));
}

/// The two OFL files carry their copyright lines (OFL-1.1 requires them).
#[test]
fn g5_ofl_files_carry_the_copyright() {
    for file in ["OFL-1.1-CormorantGaramond.txt", "OFL-1.1-NunitoSans.txt"] {
        let text = std::fs::read_to_string(dir().join(file)).unwrap();
        assert!(text.starts_with("Copyright "), "{file}");
        assert!(
            text.contains("SIL Open Font License, Version 1.1"),
            "{file}"
        );
    }
}

/// Self-test: an empty directory gives one error per licence.
#[test]
fn g5_check_fails_on_missing_files() {
    let empty = std::env::temp_dir().join("ocb-g5-no-such-dir");
    assert_eq!(check(&empty).len(), LICENCES.len());
}
