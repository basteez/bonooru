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
use std::io::{ErrorKind, Read as _};
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

/// Largest user profile read (the bundled one is about 10 KiB): a bigger
/// file is refused, never read whole (review 2026-11-28).
pub const MAX_PROFILE_BYTES: u64 = 1024 * 1024;

/// Whether a directory entry is a user profile candidate: a `.toml`
/// extension in any case, and not a dot-file (`._x.toml` from macOS,
/// `.#x.toml` editor locks).
#[must_use]
pub fn is_candidate(name: &str) -> bool {
    !name.starts_with('.')
        && Path::new(name)
            .extension()
            .is_some_and(|x| x.eq_ignore_ascii_case("toml"))
}

/// Reads one candidate: a regular file (symlinks followed) of at most
/// [`MAX_PROFILE_BYTES`], else an error message. Never blocks on a FIFO
/// or a device: the type is checked before the file is opened.
fn read_candidate(path: &Path) -> Result<String, String> {
    let meta = fs::metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_file() {
        return Err("not a regular file".to_owned());
    }
    if meta.len() > MAX_PROFILE_BYTES {
        return Err(format!("larger than {MAX_PROFILE_BYTES} bytes"));
    }
    let mut text = String::new();
    fs::File::open(path)
        .and_then(|f| f.take(MAX_PROFILE_BYTES + 1).read_to_string(&mut text))
        .map_err(|e| e.to_string())?;
    if text.len() as u64 > MAX_PROFILE_BYTES {
        return Err(format!("larger than {MAX_PROFILE_BYTES} bytes"));
    }
    Ok(text)
}

/// Loads the profile with the user copies in `dir` (candidates by
/// [`is_candidate`], by name). Only a missing dir means no user copy; a
/// dir or an entry that cannot be read is a refused candidate (`read`),
/// so a permission problem is visible in the log.
#[must_use]
pub fn load_from(dir: &Path) -> Resolved {
    let mut user = Vec::new();
    let dir_name = dir.file_name().map_or_else(
        || "profiles".to_owned(),
        |n| n.to_string_lossy().into_owned(),
    );
    match fs::read_dir(dir) {
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        Err(e) => user.push(UserFile {
            name: dir_name,
            text: Err(e.to_string()),
        }),
        Ok(entries) => {
            for entry in entries {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(e) => {
                        user.push(UserFile {
                            name: dir_name.clone(),
                            text: Err(e.to_string()),
                        });
                        continue;
                    }
                };
                let name = entry.file_name().to_string_lossy().into_owned();
                if !is_candidate(&name) {
                    continue;
                }
                let text = read_candidate(&entry.path());
                user.push(UserFile { name, text });
            }
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

/// A one-line, quotable text: every control character (newlines, tabs,
/// ESC, NUL …) and the Unicode line and paragraph separators as spaces,
/// quotes and backslashes escaped, at most 160 characters. Used for
/// `detail`, file names and the profile name.
#[must_use]
pub fn detail(text: &str) -> String {
    let mut out = String::new();
    let mut n = 0;
    for c in text.chars() {
        let piece = match c {
            c if c.is_control() || c == '\u{2028}' || c == '\u{2029}' => " ".to_owned(),
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
/// files in name order, then one `loaded` line. File names only, quoted
/// and cleaned like `detail` (a name cannot forge a line).
#[must_use]
pub fn log_lines(r: &Resolved) -> Vec<String> {
    let mut lines: Vec<String> = r
        .events
        .iter()
        .map(|e| match e {
            ProfileEvent::Refused { file, error } => format!(
                "refused file=\"{}\" reason={} detail=\"{}\"",
                detail(file),
                error.reason(),
                detail(&error.to_string())
            ),
            ProfileEvent::Ignored { file, reason } => {
                format!(
                    "ignored file=\"{}\" reason={}",
                    detail(file),
                    reason.as_str()
                )
            }
        })
        .collect();
    let (source, file) = match &r.source {
        Source::Bundled => ("bundled", "-".to_owned()),
        Source::User { file } => ("user", format!("\"{}\"", detail(file))),
    };
    let p = &r.profile;
    lines.push(format!(
        "loaded name=\"{}\" game_version=\"{}\" schema={} source={source} file={file}",
        detail(&p.name),
        detail(&p.game_version),
        p.schema
    ));
    lines
}
