//! Mapping profiles (Story 5.1; AD-17, AD-15, FR-06): the only code that
//! parses a mapping profile. Text in, a validated [`Profile`] or a typed
//! [`ProfileError`] out; no I/O (the services crate reads the files and
//! embeds the bundled default).
//!
//! Steps of [`parse`]: TOML syntax → the integer `schema` → the migration
//! chain up to [`SCHEMA`] → the shape (every key required, unknown keys
//! refused) → the value rules (ranges, the AD-15 timing floor, fingering
//! coverage). [`resolve`] picks between the bundled default and the user
//! copies (same `name` and `game_version` override, AD-17).

use std::collections::BTreeSet;
use std::fmt;

use serde::Deserialize;

/// The schema this version reads and writes.
pub const SCHEMA: u32 = 1;

/// The message shown when a user copy is refused (Story 5.13 shows it).
pub const USER_REFUSED_MESSAGE: &str =
    "This mapping profile can't be read by this version. Defaults are in use.";

/// One migration step: a table at schema `from` becomes schema `from + 1`.
/// The step edits the table; the chain sets `schema` itself.
#[derive(Clone, Copy, Debug)]
pub struct Migration {
    pub from: u32,
    pub apply: fn(&mut toml::Table) -> Result<(), ProfileError>,
}

/// The real migration chain: empty, schema 1 is the first.
pub const MIGRATIONS: &[Migration] = &[];

/// Why a profile text cannot be used.
#[derive(Clone, Debug, PartialEq)]
pub enum ProfileError {
    /// The file could not be read (the adapter's message).
    Io { message: String },
    /// Not TOML.
    Parse {
        message: String,
        line: Option<usize>,
    },
    /// No integer `schema`, or one this version cannot migrate.
    Schema { found: Option<i64> },
    /// A key missing, unknown or of the wrong type (`field` empty, the
    /// parser's message in `rule`), or a value breaking a rule.
    Invalid {
        field: String,
        value: String,
        rule: String,
        line: Option<usize>,
    },
}

impl ProfileError {
    /// The log reason: `read`, `parse`, `schema` or `invalid`.
    #[must_use]
    pub fn reason(&self) -> &'static str {
        match self {
            Self::Io { .. } => "read",
            Self::Parse { .. } => "parse",
            Self::Schema { .. } => "schema",
            Self::Invalid { .. } => "invalid",
        }
    }

    fn invalid(
        field: impl Into<String>,
        value: impl fmt::Display,
        rule: impl Into<String>,
    ) -> Self {
        Self::Invalid {
            field: field.into(),
            value: value.to_string(),
            rule: rule.into(),
            line: None,
        }
    }
}

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let with_line = |f: &mut fmt::Formatter<'_>, line: &Option<usize>, m: &str| match line {
            Some(l) => write!(f, "line {l}: {m}"),
            None => write!(f, "{m}"),
        };
        match self {
            Self::Io { message } => write!(f, "{message}"),
            Self::Parse { message, line } => with_line(f, line, message),
            Self::Schema { found: Some(n) } => write!(f, "schema {n}, this version reads {SCHEMA}"),
            Self::Schema { found: None } => write!(f, "no integer schema"),
            Self::Invalid {
                field, rule, line, ..
            } if field.is_empty() => with_line(f, line, rule),
            Self::Invalid {
                field, value, rule, ..
            } => write!(f, "{field} = {value}: {rule}"),
        }
    }
}

impl std::error::Error for ProfileError {}

/// A note button.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
pub enum Button {
    L,
    R,
    A,
    Y,
    X,
}

/// The D-pad modifier of a fingering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Dpad {
    None,
    Up,
    Down,
}

/// The stick modifier of a fingering (full travel, AD-10 bytes in
/// [`Modifiers`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StickDir {
    None,
    Up,
    Down,
}

/// The natural sounding pitch (MIDI) of each note button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Buttons {
    pub l: u8,
    pub r: u8,
    pub a: u8,
    pub y: u8,
    pub x: u8,
}

impl Buttons {
    /// The pitch of `b`.
    #[must_use]
    pub fn pitch(&self, b: Button) -> u8 {
        match b {
            Button::L => self.l,
            Button::R => self.r,
            Button::A => self.a,
            Button::Y => self.y,
            Button::X => self.x,
        }
    }
}

/// Modifier offsets (semitones) and stick bytes (canonical state, AD-10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Modifiers {
    pub dpad_up: i8,
    pub dpad_down: i8,
    pub stick_up: i8,
    pub stick_down: i8,
    pub stick_up_ly: u8,
    pub stick_down_ly: u8,
    pub stick_lx: u8,
}

/// The sounding range (MIDI), inclusive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Range {
    pub low: u8,
    pub high: u8,
}

/// Timing values as in the file: frames on the frame grid, or ms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timing {
    pub min_press_frames: u32,
    pub min_gap_same_button_frames: u32,
    pub modifier_lead_frames: u32,
    pub max_sustain_ms: u32,
    pub start_lead_ms: u32,
}

/// Fingering cost weights (finite, ≥ 0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cost {
    pub dpad_change: f64,
    pub stick_change: f64,
    pub stick_through_centre: f64,
    pub same_button_retrigger: f64,
    pub non_natural: f64,
}

/// One measured fingering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fingering {
    pub button: Button,
    pub dpad: Dpad,
    pub stick: StickDir,
    pub pitch: u8,
    /// One of the 19 chart fingerings.
    pub chart: bool,
}

/// A validated mapping profile.
#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    pub schema: u32,
    pub name: String,
    pub game_version: String,
    pub frame_rate: u32,
    pub buttons: Buttons,
    pub modifiers: Modifiers,
    pub range: Range,
    pub timing: Timing,
    pub cost: Cost,
    pub fingerings: Vec<Fingering>,
}

/// Upper bounds that keep every duration inside `u32` µs with room
/// (`t_us` < 2^31, AD-15).
const MAX_FRAMES: i64 = 600;
const MAX_MS: i64 = 600_000;

impl Profile {
    /// One frame in µs, rounded down (16 666 at 60 Hz).
    #[must_use]
    pub fn frame_us(&self) -> u32 {
        1_000_000 / self.frame_rate.max(1)
    }

    fn frames_us(&self, frames: u32) -> u32 {
        frames.saturating_mul(self.frame_us())
    }

    /// The minimum press, µs.
    #[must_use]
    pub fn min_press_us(&self) -> u32 {
        self.frames_us(self.timing.min_press_frames)
    }

    /// The minimum gap before the same button is pressed again, µs.
    #[must_use]
    pub fn min_gap_same_button_us(&self) -> u32 {
        self.frames_us(self.timing.min_gap_same_button_frames)
    }

    /// How long a modifier leads its note button, µs.
    #[must_use]
    pub fn modifier_lead_us(&self) -> u32 {
        self.frames_us(self.timing.modifier_lead_frames)
    }

    /// The longest press before the game cuts a note, µs.
    #[must_use]
    pub fn max_sustain_us(&self) -> u32 {
        self.timing.max_sustain_ms.saturating_mul(1000)
    }

    /// The lead before a timeline's first event (START `lead_us`), µs.
    #[must_use]
    pub fn start_lead_us(&self) -> u32 {
        self.timing.start_lead_ms.saturating_mul(1000)
    }

    /// The fingerings that sound `pitch`, in file order.
    pub fn fingerings_for(&self, pitch: u8) -> impl Iterator<Item = &Fingering> {
        self.fingerings.iter().filter(move |f| f.pitch == pitch)
    }

    /// The AD-15 floor with a known report period: min press and min gap
    /// same button must be at least `2 × max(report_period_us, frame)`.
    /// `0` means unknown (INFO v1), so the frame time alone counts, as at
    /// load. Epic 6 calls it once `INFO.report_period_us` is known.
    pub fn check_report_period(&self, report_period_us: u16) -> Result<(), ProfileError> {
        let floor = 2 * u32::from(report_period_us).max(self.frame_us());
        check_floor(
            "timing.min_press_frames",
            self.timing.min_press_frames,
            self.min_press_us(),
            floor,
        )?;
        check_floor(
            "timing.min_gap_same_button_frames",
            self.timing.min_gap_same_button_frames,
            self.min_gap_same_button_us(),
            floor,
        )
    }
}

fn check_floor(field: &str, frames: u32, us: u32, floor: u32) -> Result<(), ProfileError> {
    if us < floor {
        return Err(ProfileError::invalid(
            field,
            frames,
            format!("{us} us is below 2 x max(report period, frame) = {floor} us"),
        ));
    }
    Ok(())
}

// The file's shape: every key required, unknown keys refused (D5).
// Integers as i64 so a value out of range gets this module's error, with
// its field, instead of a deserialiser overflow.

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    #[serde(rename = "schema")]
    _schema: toml::Value,
    profile: RawMeta,
    buttons: RawButtons,
    modifiers: RawModifiers,
    range: RawRange,
    timing: RawTiming,
    cost: RawCost,
    fingerings: Vec<RawFingering>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMeta {
    name: String,
    game_version: String,
    frame_rate: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(non_snake_case)]
struct RawButtons {
    L: i64,
    R: i64,
    A: i64,
    Y: i64,
    X: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawModifiers {
    dpad_up: i64,
    dpad_down: i64,
    stick_up: i64,
    stick_down: i64,
    stick_up_ly: i64,
    stick_down_ly: i64,
    stick_lx: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRange {
    low: i64,
    high: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTiming {
    min_press_frames: i64,
    min_gap_same_button_frames: i64,
    modifier_lead_frames: i64,
    max_sustain_ms: i64,
    start_lead_ms: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCost {
    dpad_change: f64,
    stick_change: f64,
    stick_through_centre: f64,
    same_button_retrigger: f64,
    non_natural: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFingering {
    button: Button,
    dpad: Dpad,
    stick: StickDir,
    pitch: i64,
    chart: bool,
}

/// The 1-based line of byte offset `at` in `text`.
fn line_of(text: &str, at: usize) -> usize {
    text.get(..at)
        .map_or(0, |s| s.bytes().filter(|b| *b == b'\n').count())
        + 1
}

/// Parses, migrates and validates a profile text with the real chain.
pub fn parse(text: &str) -> Result<Profile, ProfileError> {
    parse_with(text, MIGRATIONS)
}

/// [`parse`] with a given migration chain (tests prove the mechanism with
/// a test-only step).
pub fn parse_with(text: &str, steps: &[Migration]) -> Result<Profile, ProfileError> {
    let mut table: toml::Table = toml::from_str(text).map_err(|e| ProfileError::Parse {
        message: e.message().trim().to_owned(),
        line: e.span().map(|s| line_of(text, s.start)),
    })?;
    let found = match table.get("schema") {
        Some(toml::Value::Integer(n)) => *n,
        _ => return Err(ProfileError::Schema { found: None }),
    };
    let current = i64::from(SCHEMA);
    if found > current || found < 0 {
        return Err(ProfileError::Schema { found: Some(found) });
    }
    let raw: RawFile = if found == current {
        // From the text: the parser's error then has a line.
        toml::from_str(text).map_err(|e| shape_error(&e, Some(text)))?
    } else {
        let mut v = found;
        while v < current {
            let step = steps
                .iter()
                .find(|s| i64::from(s.from) == v)
                .ok_or(ProfileError::Schema { found: Some(found) })?;
            (step.apply)(&mut table)?;
            v += 1;
        }
        table.insert("schema".to_owned(), toml::Value::Integer(current));
        table.try_into().map_err(|e| shape_error(&e, None))?
    };
    validate(raw)
}

fn shape_error(e: &toml::de::Error, text: Option<&str>) -> ProfileError {
    ProfileError::Invalid {
        field: String::new(),
        value: String::new(),
        rule: e.message().trim().to_owned(),
        line: text.and_then(|t| e.span().map(|s| line_of(t, s.start))),
    }
}

fn int_in(field: &str, v: i64, lo: i64, hi: i64) -> Result<i64, ProfileError> {
    if (lo..=hi).contains(&v) {
        Ok(v)
    } else {
        Err(ProfileError::invalid(
            field,
            v,
            format!("must be {lo} to {hi}"),
        ))
    }
}

fn u8_in(field: &str, v: i64, lo: i64, hi: i64) -> Result<u8, ProfileError> {
    let v = int_in(field, v, lo, hi)?;
    u8::try_from(v).map_err(|_| ProfileError::invalid(field, v, "out of range"))
}

fn i8_in(field: &str, v: i64) -> Result<i8, ProfileError> {
    let v = int_in(field, v, -12, 12)?;
    i8::try_from(v).map_err(|_| ProfileError::invalid(field, v, "out of range"))
}

fn u32_in(field: &str, v: i64, lo: i64, hi: i64) -> Result<u32, ProfileError> {
    let v = int_in(field, v, lo, hi)?;
    u32::try_from(v).map_err(|_| ProfileError::invalid(field, v, "out of range"))
}

fn weight(field: &str, v: f64) -> Result<f64, ProfileError> {
    if v.is_finite() && v >= 0.0 {
        Ok(v)
    } else {
        Err(ProfileError::invalid(
            field,
            v,
            "must be a finite number >= 0",
        ))
    }
}

fn text_field(field: &str, v: String) -> Result<String, ProfileError> {
    if v.trim().is_empty() {
        Err(ProfileError::invalid(
            field,
            format!("{v:?}"),
            "must not be empty",
        ))
    } else {
        Ok(v)
    }
}

fn validate(raw: RawFile) -> Result<Profile, ProfileError> {
    let name = text_field("profile.name", raw.profile.name)?;
    let game_version = text_field("profile.game_version", raw.profile.game_version)?;
    let frame_rate = u32_in("profile.frame_rate", raw.profile.frame_rate, 1, 240)?;
    let b = &raw.buttons;
    let buttons = Buttons {
        l: u8_in("buttons.L", b.L, 0, 127)?,
        r: u8_in("buttons.R", b.R, 0, 127)?,
        a: u8_in("buttons.A", b.A, 0, 127)?,
        y: u8_in("buttons.Y", b.Y, 0, 127)?,
        x: u8_in("buttons.X", b.X, 0, 127)?,
    };
    let m = &raw.modifiers;
    let modifiers = Modifiers {
        dpad_up: i8_in("modifiers.dpad_up", m.dpad_up)?,
        dpad_down: i8_in("modifiers.dpad_down", m.dpad_down)?,
        stick_up: i8_in("modifiers.stick_up", m.stick_up)?,
        stick_down: i8_in("modifiers.stick_down", m.stick_down)?,
        stick_up_ly: u8_in("modifiers.stick_up_ly", m.stick_up_ly, 0, 255)?,
        stick_down_ly: u8_in("modifiers.stick_down_ly", m.stick_down_ly, 0, 255)?,
        stick_lx: u8_in("modifiers.stick_lx", m.stick_lx, 0, 255)?,
    };
    let low = u8_in("range.low", raw.range.low, 0, 127)?;
    let high = u8_in("range.high", raw.range.high, 0, 127)?;
    if low >= high {
        return Err(ProfileError::invalid(
            "range.low",
            low,
            format!("must be below range.high ({high})"),
        ));
    }
    let range = Range { low, high };
    let t = &raw.timing;
    let timing = Timing {
        min_press_frames: u32_in("timing.min_press_frames", t.min_press_frames, 1, MAX_FRAMES)?,
        min_gap_same_button_frames: u32_in(
            "timing.min_gap_same_button_frames",
            t.min_gap_same_button_frames,
            1,
            MAX_FRAMES,
        )?,
        modifier_lead_frames: u32_in(
            "timing.modifier_lead_frames",
            t.modifier_lead_frames,
            0,
            MAX_FRAMES,
        )?,
        max_sustain_ms: u32_in("timing.max_sustain_ms", t.max_sustain_ms, 1, MAX_MS)?,
        start_lead_ms: u32_in("timing.start_lead_ms", t.start_lead_ms, 0, MAX_MS)?,
    };
    let c = &raw.cost;
    let cost = Cost {
        dpad_change: weight("cost.dpad_change", c.dpad_change)?,
        stick_change: weight("cost.stick_change", c.stick_change)?,
        stick_through_centre: weight("cost.stick_through_centre", c.stick_through_centre)?,
        same_button_retrigger: weight("cost.same_button_retrigger", c.same_button_retrigger)?,
        non_natural: weight("cost.non_natural", c.non_natural)?,
    };
    let mut fingerings = Vec::with_capacity(raw.fingerings.len());
    let mut seen = BTreeSet::new();
    for (i, f) in raw.fingerings.iter().enumerate() {
        let pitch = u8_in(
            &format!("fingerings[{i}].pitch"),
            f.pitch,
            i64::from(low),
            i64::from(high),
        )?;
        if !seen.insert((f.button, f.dpad, f.stick)) {
            return Err(ProfileError::invalid(
                format!("fingerings[{i}]"),
                format!("{:?}/{:?}/{:?}", f.button, f.dpad, f.stick),
                "duplicate button, dpad and stick",
            ));
        }
        fingerings.push(Fingering {
            button: f.button,
            dpad: f.dpad,
            stick: f.stick,
            pitch,
            chart: f.chart,
        });
    }
    for p in low..=high {
        if !fingerings.iter().any(|f| f.pitch == p) {
            return Err(ProfileError::invalid(
                "fingerings",
                p,
                "a pitch of the range has no fingering",
            ));
        }
    }
    let profile = Profile {
        schema: SCHEMA,
        name,
        game_version,
        frame_rate,
        buttons,
        modifiers,
        range,
        timing,
        cost,
        fingerings,
    };
    // AD-15 at load: the report period is unknown, the frame time counts.
    profile.check_report_period(0)?;
    if profile.max_sustain_us() <= profile.min_press_us() {
        return Err(ProfileError::invalid(
            "timing.max_sustain_ms",
            profile.timing.max_sustain_ms,
            format!(
                "must be above the min press ({} us)",
                profile.min_press_us()
            ),
        ));
    }
    Ok(profile)
}

/// A user profile file as the adapter read it: its file name (never the
/// path) and its text, or the read error's message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserFile {
    pub name: String,
    pub text: Result<String, String>,
}

/// Where the profile in use came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    Bundled,
    User { file: String },
}

/// Why a valid user file is not used (D8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IgnoreReason {
    /// Another `name` or `game_version`.
    OtherIdentity,
    /// A second valid copy with the bundled identity.
    Duplicate,
}

impl IgnoreReason {
    /// The log reason.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OtherIdentity => "other_identity",
            Self::Duplicate => "duplicate",
        }
    }
}

/// What happened to a user file.
#[derive(Clone, Debug, PartialEq)]
pub enum ProfileEvent {
    /// Might be the user copy but cannot be used: [`USER_REFUSED_MESSAGE`].
    Refused { file: String, error: ProfileError },
    /// Valid, not used.
    Ignored { file: String, reason: IgnoreReason },
}

/// The profile in use, where it came from, and what happened to each user
/// file (in file-name order).
#[derive(Clone, Debug, PartialEq)]
pub struct Resolved {
    pub profile: Profile,
    pub source: Source,
    pub events: Vec<ProfileEvent>,
}

/// Picks the profile: the first valid user copy (by file name) with the
/// bundled default's `name` and `game_version`, else the bundled default.
/// `Err` only when the bundled text itself is invalid (a build bug).
pub fn resolve(bundled: &str, user: &[UserFile]) -> Result<Resolved, ProfileError> {
    let default = parse(bundled)?;
    let mut files: Vec<&UserFile> = user.iter().collect();
    files.sort_by(|a, b| a.name.cmp(&b.name));
    let mut chosen: Option<(Profile, String)> = None;
    let mut events = Vec::new();
    for f in files {
        let parsed = match &f.text {
            Ok(t) => parse(t),
            Err(message) => Err(ProfileError::Io {
                message: message.clone(),
            }),
        };
        match parsed {
            Err(error) => events.push(ProfileEvent::Refused {
                file: f.name.clone(),
                error,
            }),
            Ok(p) if p.name != default.name || p.game_version != default.game_version => {
                events.push(ProfileEvent::Ignored {
                    file: f.name.clone(),
                    reason: IgnoreReason::OtherIdentity,
                });
            }
            Ok(_) if chosen.is_some() => events.push(ProfileEvent::Ignored {
                file: f.name.clone(),
                reason: IgnoreReason::Duplicate,
            }),
            Ok(p) => chosen = Some((p, f.name.clone())),
        }
    }
    Ok(match chosen {
        Some((profile, file)) => Resolved {
            profile,
            source: Source::User { file },
            events,
        },
        None => Resolved {
            profile: default,
            source: Source::Bundled,
            events,
        },
    })
}
