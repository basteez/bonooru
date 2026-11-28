//! The mapping-profile adapter (Story 5.1, AD-17): the I/O around
//! `ocb_core::profile`. It embeds the bundled default, finds the OS config
//! dir, reads the user copies and names the settings file. Parsing and
//! validation stay in the core; this module never looks inside a profile.
//!
//! Locations (`directories::ProjectDirs::from("", "", "Bonooru")`, D4):
//! config dir macOS `~/Library/Application Support/Bonooru`, Windows
//! `%APPDATA%\Bonooru\config`, Linux `$XDG_CONFIG_HOME/bonooru` (default
//! `~/.config/bonooru`); user copies in `<config dir>/profiles/*.toml`; app
//! settings in `<config dir>/settings.toml` (a separate file, AD-17; its
//! contents come with Stories 5.12 and 5.13).

use std::fs;
use std::path::{Path, PathBuf};

use ocb_core::profile::{self, ProfileEvent, Resolved, Source, UserFile};

/// The bundled default for OoT 1.1.1 on Switch 2 (Story 1.5), read-only.
pub const BUNDLED: &str = include_str!("../../profiles/oot-switch2-1.1.1.toml");

/// Longest `detail` in a log line.
const DETAIL_MAX: usize = 160;

/// The OS config dir of the app, `None` without a home directory.
#[must_use]
pub fn config_dir() -> Option<PathBuf> {
    Some(
        directories::ProjectDirs::from("", "", "Bonooru")?
            .config_dir()
            .to_path_buf(),
    )
}

/// Where user profile copies live: `<config dir>/profiles`.
#[must_use]
pub fn user_profiles_dir() -> Option<PathBuf> {
    Some(config_dir()?.join("profiles"))
}

/// The app settings file: `<config dir>/settings.toml`, apart from the
/// profiles (AD-17). Nothing reads or writes it yet.
#[must_use]
pub fn settings_path() -> Option<PathBuf> {
    Some(config_dir()?.join("settings.toml"))
}

/// The bundled default alone (tests, and a platform with no config dir).
///
/// # Panics
///
/// If the bundled text is invalid: a build bug that C.30 and C.31 rule out
/// (they parse this exact text), so a shipped build never panics here.
#[must_use]
pub fn bundled_only() -> Resolved {
    resolve(&[])
}

fn resolve(user: &[UserFile]) -> Resolved {
    match profile::resolve(BUNDLED, user) {
        Ok(r) => r,
        // Only the bundled text itself can fail `resolve`: a build bug.
        Err(e) => unreachable_bundled(&e),
    }
}

#[allow(clippy::panic)]
fn unreachable_bundled(e: &profile::ProfileError) -> ! {
    panic!("the bundled mapping profile is invalid: {e}")
}

/// Loads the profile with the user copies in `dir` (`*.toml`, by name). A
/// missing dir means no user copy; an entry that cannot be read is a
/// refused candidate.
#[must_use]
pub fn load_from(dir: &Path) -> Resolved {
    let mut user = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for e in entries.flatten() {
            let path = e.path();
            if path.extension().is_none_or(|x| x != "toml") {
                continue;
            }
            let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
                continue;
            };
            let text = fs::read_to_string(&path).map_err(|err| err.to_string());
            user.push(UserFile { name, text });
        }
    }
    resolve(&user)
}

/// Loads the profile from the real config dir (bundled only when the
/// platform has none).
#[must_use]
pub fn load() -> Resolved {
    match user_profiles_dir() {
        Some(d) => load_from(&d),
        None => bundled_only(),
    }
}

/// A one-line `detail`: newlines and tabs as spaces, quotes and
/// backslashes escaped, at most 160 characters.
#[must_use]
pub fn detail(text: &str) -> String {
    let mut out = String::new();
    let mut n = 0;
    for c in text.chars() {
        let piece = match c {
            '\n' | '\r' | '\t' => " ".to_owned(),
            '"' => "\\\"".to_owned(),
            '\\' => "\\\\".to_owned(),
            c => c.to_string(),
        };
        let len = piece.chars().count();
        if n + len > DETAIL_MAX {
            break;
        }
        n += len;
        out.push_str(&piece);
    }
    out
}

/// The `profile` log lines (without the kind tag): the refused and ignored
/// files in name order, then one `loaded` line. File names only.
#[must_use]
pub fn log_lines(r: &Resolved) -> Vec<String> {
    let mut lines: Vec<String> = r
        .events
        .iter()
        .map(|e| match e {
            ProfileEvent::Refused { file, error } => format!(
                "refused file={file} reason={} detail=\"{}\"",
                error.reason(),
                detail(&error.to_string())
            ),
            ProfileEvent::Ignored { file, reason } => {
                format!("ignored file={file} reason={}", reason.as_str())
            }
        })
        .collect();
    let (source, file) = match &r.source {
        Source::Bundled => ("bundled", "-"),
        Source::User { file } => ("user", file.as_str()),
    };
    let p = &r.profile;
    lines.push(format!(
        "loaded name=\"{}\" game_version={} schema={} source={source} file={file}",
        detail(&p.name),
        p.game_version,
        p.schema
    ));
    lines
}
