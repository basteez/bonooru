---
baseline_commit: ce9c80e
---

# Story 5.1: Mapping profile module

Status: review

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As the owner,
I want the app to load, validate and migrate mapping profiles in one place,
so that mapping and timing values are never in code and every module gets a valid profile (FR-06).

Source: `epics.md` › Epic 5 › Story 5.1; AD-15, AD-17. First story of Epic 5 after the cleanup Story 5.0. No bench, no manual step: host tests only (the profile is data; nothing is sent to the Switch).

## Acceptance Criteria

1. **One parser (AD-17).** A new module `ocb_core::profile` is the only code that parses a mapping profile. It uses `toml` + `serde`, checks the integer `schema`, migrates, validates ranges and required fields, and returns a typed `Profile` or a typed `ProfileError`. No other crate or module deserialises profile TOML (a source scan enforces it).
2. **Purity gate amended, not dropped (C.4).** `ocb-core`'s `[dependencies]` may name exactly `toml` and `serde` (exact pins, D1); any other crate is still a hit, and the banned words and macros are unchanged (`include_str!` stays banned in the core). The purity self-test gains a bad fixture with another crate and a good one with the two allowed.
3. **Bundled default, embedded read-only.** `app/profiles/oot-switch2-1.1.1.toml` (Story 1.5) is embedded into the binary by `ocb-services` (`include_str!`, D2) and passed to the core as text. It parses and validates with no error, and a test pins its key values: name "OoT Switch 2", game version "1.1.1", schema 1, frame rate 60, 5 buttons, range 71–89, the six timing values, the five cost weights, 45 fingerings, 19 of them `chart`, every pitch 71–89 reachable.
4. **User copy overrides.** A user copy is a `*.toml` file in `<config dir>/profiles/` (D4). One with the same `name` and `game_version` as the bundled default, valid or migratable, overrides it. Files with another identity are ignored with a typed event. Two valid matching copies: the first by file name wins, a typed event names the other.
5. **User copy refused.** A user file that cannot be read, cannot be parsed, has a `schema` this version cannot migrate (newer, missing, not an integer, or older with no migration path), or fails validation is refused. The bundled default is used, and a typed event carries the file, the error kind and the exact message "This mapping profile can't be read by this version. Defaults are in use." (`profile::USER_REFUSED_MESSAGE`; Story 5.13 shows it). An unparseable file counts as a refused candidate, because it may be the user copy.
6. **Migration path.** Migration is a chain of steps `schema N → N + 1` up to `profile::SCHEMA` (1). It has no real step yet, so schema 1 loads as is. The mechanism is tested with a test-only step (schema 0 → 1, adding a field), and a schema above `SCHEMA` is refused, never guessed.
7. **Timing rule (AD-15).** Validation rejects `min_press_frames` or `min_gap_same_button_frames` whose duration is below `2 × max(report_period_us, frame time)`. At load the report period is unknown, so only the frame time counts (2 frames). `Profile::check_report_period(report_period_us)` applies the full rule once `INFO.report_period_us` is known; Epic 6 (Play gating) calls it, and it is not wired in this story. The frame time is `1_000_000 / frame_rate` µs, rounded down.
8. **Validation rules (D5).**
   - Unknown keys are refused (typos such as `min_pres_frames`).
   - `frame_rate` is 1–240.
   - Every button pitch and range bound is 0–127, and `low < high`.
   - Modifier offsets are −12…+12; stick bytes are `u8`.
   - Every timing value is > 0, except `modifier_lead_frames` ≥ 0 and `start_lead_ms` ≥ 0. `max_sustain_ms` must be > the min press in ms.
   - Cost weights are finite and ≥ 0.
   - Fingerings: button ∈ {L, R, A, Y, X}, dpad and stick ∈ {none, up, down}, each pitch inside the range, no duplicate (button, dpad, stick), and every pitch of the range has at least one fingering.
   - Each error names its field (dotted path, `fingerings[12].pitch`), the value and the rule.
9. **Settings stay apart.** The adapter defines `settings_path()` as `<config dir>/settings.toml`, a file separate from `<config dir>/profiles/` (a test asserts it is not inside the profiles dir). Settings contents come with Stories 5.12 and 5.13; nothing reads or writes the file yet.
10. **App wiring and log.**
    - `App::new` loads the profile through the adapter at start and keeps the `Profile` (`App::profile()`).
    - It writes one `profile` line per outcome:
      - `profile loaded name="<n>" game_version=<v> schema=<s> source=<bundled|user> file=<f|->`
      - `profile refused file=<f> reason=<read|parse|schema|invalid> detail="<short>"`
      - `profile ignored file=<f> reason=<other_identity|duplicate>`
    - Test constructors (`with_link`, `without_link`) use the bundled default only, with no file system access. A builder `.profile(resolved)` injects a resolution and writes its lines to the app's log.
    - The app never fails to start because of a user file.
11. **Fuzz (never panics).** A `proptest` case feeds arbitrary strings and random byte edits of the bundled text to `profile::parse`. It never panics, and always returns `Ok` or a typed error.
12. **Docs.**
    - `docs/score-format-and-mapping.md` › Mapping profile describes the schema, the validation rules, migration, where user copies live on each OS, and the refusal message. The "Story 5.1 owns" placeholder goes.
    - `app/README.md` lists `ocb_core::profile` and `ocb_services::profiles`.
    - `docs/test-plan.md`: C.4 updated; new C.30 (core profile), C.31 (adapter) and G.27 (app lines).
    - The profile file's header comment no longer says "Loading … belong to Story 5.1".
13. **Green, no new flake.** fmt, clippy `-D warnings`, `cargo test --workspace` (two runs) and `cargo deny check` all pass. CI is green on three OS. Every new or changed test and check has a red proof in the Debug Log (E4-A3).

### AC → measurement (retro action E3-A1)

| AC | Measured by (host) | Field / result |
|---|---|---|
| 1 | C.30 `c30_one_parser` (source scan of `app/*/src`) | `toml::` / `Deserialize` for profile types only under `core/src/profile` |
| 2 | C.4 (`core/tests/purity.rs`) real tree + fixtures | real tree passes; bad fixture with `regex = …` hit; good fixture with `toml`/`serde` passes |
| 3 | C.30 `c30_bundled_values` | pinned values equal |
| 4, 5 | C.30 `resolve` cases (pure, text in) + C.31 (temp config dir) | `source=user` / `bundled`, events with kind and message |
| 6 | C.30 migration cases | test step applied; schema 2 → `Schema { found: 2, .. }` |
| 7 | C.30 timing cases | 1 frame at 60 Hz refused, 2 frames accepted; `check_report_period(20_000)` refuses 2 frames (33 333 < 40 000) |
| 8 | C.30 one case per rule | each error names field and value |
| 9 | C.31 `c31_settings_apart` | `settings_path()` not under `user_profiles_dir()`, same parent |
| 10 | G.27 (`gui/tests/profile_app.rs`) | exact `profile` lines on a memory log; refused copy → app runs, bundled in use |
| 11 | C.30 proptest | no panic in 256+ cases each |
| 12 | review | text present |
| 13 | local + CI | all green |

## Tasks / Subtasks

- [x] **Task 1: Dependency check and purity gate (AC: 2; E4-A6)**
  - [x] 1.1 Record in the Debug Log `cargo info toml` / `cargo info serde`: version, licence, MSRV (Dev Notes › Dependencies has the values found at story creation; re-check).
  - [x] 1.2 Red first: add the purity fixtures (bad: a third crate in core `[dependencies]`; good: `toml` + `serde` exact pins), run C.4, record the failures; then amend `core/tests/purity.rs` (allow list for `[dependencies]` names) and get it green.
  - [x] 1.3 Add `toml = { version = "=1.2.3", default-features = false, features = ["std", "parse", "serde"] }` and `serde = { version = "=1.0.229", features = ["derive"] }` to `app/core/Cargo.toml`; update the "Must stay empty" comment. `cargo deny check` green.
- [x] **Task 2: `ocb_core::profile` (AC: 1, 3, 5–8, 11)**
  - [x] 2.1 Red first: write C.30 (`app/core/tests/profile.rs`) against the planned API; record the failing run.
  - [x] 2.2 Types: `Profile`, `Button`, `Dpad`, `Stick`, `Fingering`, `Timing`, `Cost`, `Range`, `Modifiers`; raw serde structs private (`#[serde(deny_unknown_fields)]`); `SCHEMA = 1`; `USER_REFUSED_MESSAGE`.
  - [x] 2.3 `parse(text) -> Result<Profile, ProfileError>`: TOML → `toml::Table` → read `schema` → migrate chain → deserialise → validate.
  - [x] 2.4 `ProfileError` (typed, `Display`, no panics): `Parse { message, line }`, `Schema { found: Option<i64>, supported: u32 }`, `Invalid { field, value, rule }`.
  - [x] 2.5 `resolve(bundled: &str, user: &[UserFile]) -> Result<Resolved, ProfileError>` (`Err` only if the bundled text itself is invalid); `UserFile { name, text: Result<String, String> }` (an I/O error as text); `Resolved { profile, source, events }`; `ProfileEvent::{Loaded, Refused, Ignored}`.
  - [x] 2.6 Helpers the pipeline will need: `frame_us()`, `min_press_us()`, `min_gap_same_button_us()`, `modifier_lead_us()`, `max_sustain_us()`, `start_lead_us()`, `fingerings_for(pitch)`; `check_report_period(u16)`.
  - [x] 2.7 Proptest (AC 11).
- [x] **Task 3: `ocb_services::profiles` adapter (AC: 3, 4, 5, 9)**
  - [x] 3.1 Red first: C.31 (`app/services/tests/profiles.rs`) with a temp config dir.
  - [x] 3.2 `BUNDLED` (`include_str!("../../profiles/oot-switch2-1.1.1.toml")`), `config_dir()`, `user_profiles_dir()`, `settings_path()` (via `directories::ProjectDirs::from("", "", "Bonooru")`), `load_from(dir: &Path) -> Resolved` (reads `*.toml` sorted by name; missing dir → none; unreadable → `UserFile { text: Err }`), `load() -> Resolved` (real config dir; `None` dir → bundled only), `bundled_only() -> Resolved`, `log_lines(&Resolved) -> Vec<String>`.
- [x] **Task 4: App (AC: 10)**
  - [x] 4.1 Red first: G.27 (`app/gui/tests/profile_app.rs`).
  - [x] 4.2 `App` field `profile: Resolved`; `App::new` → `profiles::load()`, lines into `app_log`; `with_link` / `without_link` → `bundled_only()`, no lines; `.profile(resolved)` builder writes the lines; `App::profile() -> &Profile`.
- [x] **Task 5: Docs (AC: 12)** — score-format doc, README, test plan (C.4, C.30, C.31, G.27), profile header comment.
- [x] **Task 6: Red proofs (AC: 13; E4-A3)** — Dev Notes › Red proofs.
- [ ] **Task 7: Green (AC: 13)** — fmt, clippy, two workspace runs, deny; push; CI green on three OS.

## Dev Notes

### Scope in one paragraph

The pure core gets a profile parser and validator (`ocb_core::profile`), fed with text. The services crate gets the I/O around it: it embeds the bundled file, finds the OS config dir, reads user copies and names the settings file. The app loads once at start and logs the outcome. No UI shows anything yet (Story 5.13 adds the Settings modal and the inline refusal message), and no pipeline uses the profile yet (Stories 5.4–5.6 do). Keep the API small and typed so those stories just call it.

### Decisions taken at story creation (change only with a reason in the Debug Log)

- **D1 Core dependencies.** The epic AC puts the parser in `ocb-core` with `toml` + `serde`, and AD-17 says only the core module loads profiles. Story 3.2 D2 kept core `[dependencies]` empty as a purity proxy. Both crates are pure (no I/O, threads or clocks; `toml` without `display` has no writer either). So the gate changes from "empty" to "exactly these two names, exact pins", and the banned words and macros stay as they are. `serde`'s derive pulls `serde_derive` / `syn` / `quote` / `proc-macro2`, which only run at build time. Version pins: the versions already in `Cargo.lock` through `egui_kittest` (dev graph), so the lock gains no new crate versions for `toml` (check `cargo tree -i toml` after the change).
- **D2 Embedding.** `include_str!` is banned in the core (purity: build-time file read), so the services crate embeds the file and the core only sees `&str`. The bundled file is also read by C.30 through `std::fs` in the test, which is allowed: tests are not scanned.
- **D3 One flat error type.** There are three kinds plus a field path; that is enough for the log, for Story 5.13's message and for Epic 9's editor. Do not add an error crate (`thiserror`): `Display` by hand.
- **D4 User copy location.** `<config dir>/profiles/*.toml`, with the config dir from `directories::ProjectDirs::from("", "", "Bonooru").config_dir()`: macOS `~/Library/Application Support/Bonooru`, Windows `%APPDATA%\Bonooru\config`, Linux `$XDG_CONFIG_HOME/bonooru` (default `~/.config/bonooru`). `directories` is already a pinned dependency of `ocb-services` (`=6.0.0`, the log dir). File names are free; identity is `(name, game_version)` (AD-17). The app never creates the folder in this story (Story 5.13 writes the user copy).
- **D5 Validation strictness.** `deny_unknown_fields` everywhere. A typo in a user copy should be refused loudly, never silently defaulted (AD-17 "Prevents: … several parsers with different defaults"). There are no defaults: every field is required.
- **D6 Floats.** `cost.non_natural = 0.1` makes the cost weights `f64` (finite, ≥ 0). Every timing value is an integer (frames or ms). `frame_us = 1_000_000 / frame_rate` (16 666 at 60). Durations in µs as `u32` (pipeline: `t_us` u32, AD-15).
- **D7 Report period.** Not known at load. The load-time rule uses the frame time only. `check_report_period` exists for Epic 6, and nothing calls it in this story. A report period of 0 means unknown (INFO v1 `report_period_us` 0) and falls back to the frame time.
- **D8 Refused vs ignored.** Refused: a candidate that might be the user copy but can't be used (unreadable, unparseable, bad schema, invalid). Ignored: a valid file with another identity, or a second valid match. Only refusals carry `USER_REFUSED_MESSAGE`.

### Dependencies (E4-A6; verified at story creation with `cargo info`, 2026-11-28)

| Crate | Version | Licence | MSRV | Notes |
|---|---|---|---|---|
| `toml` | 1.2.3+spec-1.1.0 | MIT OR Apache-2.0 | 1.85 | features used: `std`, `parse`, `serde` (default also has `display`: not needed). Deps: `toml_parser`, `winnow`, `toml_datetime`, `serde_spanned`, `serde_core`: all MIT OR Apache-2.0 |
| `serde` | 1.0.229 | MIT OR Apache-2.0 | 1.56 | feature `derive` (`serde_derive`, MIT OR Apache-2.0) |

Both licences are in `deny.toml`'s allow list; no network crate (AD-21). Workspace MSRV 1.100 ≥ both. `toml` 1.x API: `toml::from_str::<T>(&str)`, `toml::Table` (= `Map<String, Value>`), `Value::try_into::<T>()`; the error type `toml::de::Error` has `message()` and `span() -> Option<Range<usize>>` (byte offsets: compute the line from the span). Check these against `~/.cargo/registry/src/*/toml-1.2.3*/src` before use (ctx7 / source).

### Profile shape (from `app/profiles/oot-switch2-1.1.1.toml`, the source of truth)

`schema` (int) · `[profile] name, game_version, frame_rate` · `[buttons] L R A Y X` (MIDI pitch) · `[modifiers] dpad_up, dpad_down, stick_up, stick_down` (semitones), `stick_up_ly, stick_down_ly, stick_lx` (bytes, hex literals in the file) · `[range] low, high` · `[timing] min_press_frames, min_gap_same_button_frames, modifier_lead_frames, max_sustain_ms, start_lead_ms` · `[cost] dpad_change, stick_change, stick_through_centre, same_button_retrigger` (ints in the file) `, non_natural` (0.1) · `[[fingerings]] button, dpad, stick, pitch, chart`. Note: TOML ints into `f64` fields — serde `toml` accepts an integer for an `f64` field? **Check**; if not, deserialise cost as a small enum/number type that accepts both and say so in the Debug Log. Do not change the data file to `1.0` to work around it unless there is no other way (it is Story 1.5's measured record; a format change needs a reason).

### Log lines (AC 10; kind tag `profile`)

```
profile loaded name="OoT Switch 2" game_version=1.1.1 schema=1 source=bundled file=-
profile loaded name="OoT Switch 2" game_version=1.1.1 schema=1 source=user file=my-oot.toml
profile refused file=broken.toml reason=parse detail="line 3: expected `=`"
profile refused file=new.toml reason=schema detail="schema 2, this version reads 1"
profile refused file=bad.toml reason=invalid detail="timing.min_press_frames = 1: below 2 frames (33332 us)"
profile ignored file=other.toml reason=other_identity
profile ignored file=b.toml reason=duplicate
```
Order: refused / ignored lines first (in file-name order), then exactly one `loaded` line. `detail` is one line, quotes escaped, ≤ 160 chars. File names only, never the full path (privacy in shared logs).

### Current state of the files this story touches

- `app/core/Cargo.toml`: empty `[dependencies]` with the comment "Must stay empty (Story 3.2 D2; enforced by tests/purity.rs)". **Change:** two deps, comment.
- `app/core/tests/purity.rs` (C.4): token scan of `core/src` (banned words: `fs`, `io`, `os`, `env`, `File` …; banned macros incl. `include_str`); manifest check: anything in `[dependencies]` is a hit (`dependencies` tables, dotted keys, target tables), `BANNED_CRATES` egui/eframe/serialport everywhere. **Change:** an allow list for `[dependencies]` names. **Preserve:** every other rule, the fixtures, the dotted-key and target-table handling (a `[target.'cfg(…)'.dependencies]` with `toml` is still a hit: keep target tables empty).
- Watch the banned words in new core code: identifiers `fs`, `io`, `os`, `env`, `File`, `process`, `Command` are hits even in comments. "profile", "frames" are fine (token match, whole identifiers).
- `app/core/src/lib.rs`: modules `device`, `link`, `protocol`, `version`. **Change:** `pub mod profile;` + doc line.
- `app/services/Cargo.toml`: `directories = "=6.0.0"` already. **No new dep.**
- `app/services/src/log.rs`: `log_dir()` with `directories` (pattern to mirror for `config_dir()`; macOS uses `BaseDirs` for `~/Library/Logs`, config uses `ProjectDirs::config_dir()` on all three).
- `app/services/src/lib.rs`: modules. **Change:** `pub mod profiles;`.
- `app/gui/src/app.rs`: `App::new` (real logger, worker), `with_link`, `without_link`, `build`, `led_log` builder (sets `self.log`). **Change:** a `profile` field set in `build` (bundled only), loaded and logged in `new`; `.profile()` builder; accessor. **Preserve:** construction order, the real logger handle sharing.
- `app/gui/src/main.rs`: unchanged (it calls `App::new` / `App::with_smoke`; check `with_smoke` → `App::new` so the smoke run loads the profile too; the smoke line format stays as it is).

### Anti-flake rules (E4-A4)

No clock, no thread, no wall-clock bound in C.30 / C.31 / G.27. C.31 uses a fresh temp dir per test (`std::env::temp_dir()` + a unique name with the test name and process id; remove it at the end; no shared state). G.27 waits for nothing: the lines are written at construction. Keep any kittest `max_steps` default (no worker thread in G.27; use `without_link`).

### E4-A5

Not concerned: no release or STOP logic. The profile is loaded at construction, not in `ui`.

### Red proofs (E4-A3; minimum list for the Debug Log)

- C.4: a third crate accepted (drop the allow-list check) → bad fixture not reported; `include_str!` allowed → existing fixture fails to report.
- C.30: each validation rule disabled one at a time (unknown key accepted; frame_rate 0; low ≥ high; offset 13; negative cost; NaN cost; duplicate fingering; uncovered pitch; fingering out of range; min press 1 frame accepted; `check_report_period` using frame time only); schema 2 accepted; migration step not applied; a refused user copy overriding; an ignored identity overriding; the duplicate rule picking the last; the message text changed; a pinned bundled value changed.
- C.30 `c30_one_parser`: a `toml::from_str` in `services` → hit.
- C.31: missing dir treated as error; unreadable file skipped silently instead of refused; `settings_path()` inside the profiles dir; full path in the log line.
- G.27: lines not written by `.profile()`; `App::new` path → can't run real dirs in tests: prove the wiring with the builder plus a source assertion that `App::new` calls `profiles::load()` (as Story 5.0's `g26_logic_reads_raw_events`).
- Proptest: make `parse` panic on a rare input (e.g. `text.len() == 7`) → proptest finds it.

### Testing standards

Core: `cargo test -p ocb-core` (C.30 integration test reads the real bundled file with `std::fs`; tests are not purity-scanned). `proptest = "=1.12.0"` is already a core dev-dependency. Services: temp dirs, never the real config dir (like G.13 for the log). GUI: kittest only where needed; G.27 can be plain construction plus the memory log (`LogHandle` memory sink pattern from `status_app.rs` / `stop_app.rs`).

### Previous story intelligence

- 5.0: one owner per concern plus a source scan proves it (`g26_one_owner`) → reuse the pattern for "one parser" (`c30_one_parser`). Red proofs by mutation script; two of them stayed green at first because the test iterated the mutated const itself. Compare against literals, not against the code's own constants.
- 4.4: the log file and `directories` (log dir per OS), privacy of paths.
- 3.2: purity gate design (token scan, fixtures, self-test) → extend, don't rewrite.
- 1.5 / 1.6: the profile values and their tags; `max_sustain_ms` 6000; all 45 fingerings measured.

### Git intelligence

`main` at `ce9c80e` (merge of PR #55, Story 5.0). Branch `story/5-1-mapping-profile-module`. The 5.0 → done flip in `sprint-status.yaml` and the story file is carried by this PR. Commit style: one long sentence, story number in parentheses.

### Architecture compliance

AD-17 (one loader, bundled read-only, user copies in the config dir, separate settings file); AD-15 (minimum durations rule; µs integers downstream); AD-18 (core has no GUI/I/O; services do the I/O); AD-21 (licences, no network); conventions: module named `profile` in the core.

### File structure

- New: `app/core/src/profile.rs` (or `profile/mod.rs` + submodules if it grows past ~600 lines), `app/core/tests/profile.rs`, `app/core/tests/purity_fixtures/…` (follow the existing fixture layout), `app/services/src/profiles.rs`, `app/services/tests/profiles.rs`, `app/gui/tests/profile_app.rs`.
- Changed: `app/core/Cargo.toml`, `app/core/src/lib.rs`, `app/core/tests/purity.rs`, `app/services/src/lib.rs`, `app/gui/src/app.rs`, `app/Cargo.lock`, `app/profiles/oot-switch2-1.1.1.toml` (header comment only), `app/README.md`, `docs/score-format-and-mapping.md`, `docs/test-plan.md`, `_bmad-output/implementation-artifacts/sprint-status.yaml`, `_bmad-output/implementation-artifacts/5-0-keyboard-routing-module.md` (status done).

### Project Context Rules

No `project-context.md`. Repo rules: English in the repo; never stage `_bmad/`, `.claude/`, bmad-*/gds-* skills; one question at a time; search upstream before asking.

### Open questions for the owner

None. D1 (two pure crates in the core) follows the epic's own AC. D4 (location) and D5 (strict) follow AD-17. The review can overturn any of them.

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 5.1]
- [Source: _bmad-output/planning-artifacts/architecture/architecture-bonooru-2026-11-19/ARCHITECTURE-SPINE.md#AD-15, #AD-17, #AD-18]
- [Source: app/profiles/oot-switch2-1.1.1.toml] (Story 1.5)
- [Source: docs/score-format-and-mapping.md#Mapping profile]
- [Source: app/core/tests/purity.rs] (C.4, Story 3.2 D2/D10)
- [Source: app/services/src/log.rs#log_dir]
- [Source: _bmad-output/implementation-artifacts/epic-4-retro-2026-11-28.md] E4-A3, E4-A4, E4-A6

### Review Findings (2026-11-28)

Code review (gds-code-review: Blind Hunter, Edge Case Hunter, Acceptance Auditor) of `ce9c80e..866a0b1`, full review against this spec. Result: 1 decision-needed, 12 patch, 3 defer, 9 dismissed.

**Decision needed**

- [x] [Review][Decision] **The refusal message can be false, and the event does not carry it** (medium) [app/core/src/profile.rs:654-716]. Every unusable `*.toml` file becomes `ProfileEvent::Refused`, even when another file is the valid user copy in use. Example: `a.toml` is broken and `b.toml` is a valid copy, so `resolve` returns `source = User { b.toml }` plus one `Refused` (checked in a scratch crate). Story 5.13 would then show "… Defaults are in use.", which is false. The same happens with OS or editor junk (`._x.toml`, `.#x.toml`, a folder named `dir.toml`). AC 5 also says the event "carries … the exact message", but `Refused { file, error }` has no message field, and the test only compares the constant with a literal. Options: (a) keep the event as it is and add `Resolved::refusal_message() -> Option<&'static str>`, which gives `USER_REFUSED_MESSAGE` only when `source == Bundled` and a second text (or none) when a user copy is in use; (b) add a `message: &'static str` field to `Refused`, chosen by the outcome; (c) keep it as it is and reword AC 5 and the docs ("refused file(s) were skipped"). Whichever option is picked, add a test with one valid copy and one broken copy. → **Resolved (Claude, technical: option a).** `Resolved::refusal_message()` returns `USER_REFUSED_MESSAGE` only when a file was refused and the bundled default is in use. `c30_resolve` covers a broken file next to the valid copy in use (refused, no message) and a run with no refusal. Docs updated. Red proof R6.

**Patch**

- [x] [Review][Patch] **C.4 allow list can be bypassed** (medium) [app/core/tests/purity.rs:220-247]. The crate is taken from the key and `pinned` only looks for `="=`. These manifests all return no problem (the code was copied into a scratch binary): → Fixed. The runtime-dep check collects every key per crate (inline, table and dotted forms) and requires exactly one `version = "=X.Y.Z"`. `package`, `path`, `git`, `registry`, `registry-index`, `workspace`, `branch`, `tag` and `rev` are refused, and so is `=1` / `=1.2`. 12 new bad cases and 2 new good ones. Red proofs P1–P3, P5–P7.
  - `toml = { package = "regex", version = "=1.11.0" }`
  - `toml = { version = "=1.2.3", path = "../x" }` and the same with `git = …` or `registry = …`
  - `[dependencies.toml]` followed by `package = "regex"` or `registry = …` or `workspace = true`, or by no `version` line at all
  - `toml = "=1"` (Cargo reads `=1` as `>=1.0.0, <2.0.0`, so it is not an exact pin)

  This breaks AC 2 ("any other crate is still a hit"), and the C.4 row and the Completion Notes claim more than the code does ("path / git sources are refused"). Fix: refuse `package`, `path`, `git`, `registry`, `workspace`, `branch`, `tag` and `rev` in both the inline and the table form; require exactly one `version = "=MAJOR.MINOR.PATCH"` per allowed crate; drop the redundant `version="=` check; add these manifests as bad fixtures, with red proofs.
- [x] [Review][Patch] **A FIFO or a huge `*.toml` file hangs or bloats app start** (medium) [app/services/src/profiles.rs:86]. `fs::read_to_string` runs inside `App::new` with no file-type check and no size cap. A scratch run of `load_from` on a dir with a `mkfifo`'d `p.toml` was still blocked after 3 s. A 200 MB file took 17 s and 426 MB of RSS. This breaks AC 10 ("the app never fails to start because of a user file"). Fix: refuse as `read` anything that is not a regular file (`metadata().is_file()`), and cap the read (for example `File::take(1 MiB + 1)`, refused above 1 MiB). → Fixed. Candidates are checked with `metadata().is_file()` before opening, so a FIFO is refused as `read` without blocking. Files over 1 MiB (`MAX_PROFILE_BYTES`) are refused, and the read goes through `take()`. Red proofs A9 (the test hangs without the check) and A10.
- [x] [Review][Patch] **An unreadable profiles dir and failing entries are silent** (low) [app/services/src/profiles.rs:77-78]. Any `read_dir` error is treated like a missing dir: permission denied, or `profiles` being a regular file (checked). `entries.flatten()` also drops entry errors, although the doc comment says "an entry that cannot be read is a refused candidate". The only line written is `loaded … source=bundled`. Fix: only `ErrorKind::NotFound` means no user copy. Any other dir or entry error becomes a refused `read` candidate, or at least a `profile` line. → Fixed. Only `NotFound` means no user copy. Any other dir error, or an entry error, is a refused `read` candidate named after the dir. Red proof A13.
- [x] [Review][Patch] **File names go into the log unescaped** (low) [app/services/src/profiles.rs:135,140,150]. `file={file}` is raw. A legal Unix name containing `\n` forges a second `profile loaded … source=user` line (checked), and a name with a space breaks `key=value`. Fix: pass the name through `detail()` and quote it (`file="…"`), or escape spaces and control characters. Update the AC 10 formats and the docs to match. → Fixed. `file="…"` and `game_version="…"` are quoted and cleaned by `detail()`. A Unix name with a newline is tested. Red proof A14.
- [x] [Review][Patch] **`detail()` cleans only `\n`, `\r` and `\t`** (low) [app/services/src/profiles.rs:106-124]. ESC, NUL, VT/FF, U+0085 and U+2028/2029 reach the log, and toml messages can quote file content. Fix: map every `char::is_control()` character and U+2028/2029 to a space. → Fixed. Every `is_control()` character and U+2028/2029 becomes a space. Red proof A8.
- [x] [Review][Patch] **`.TOML` is skipped silently, and dot-files are read** (low) [app/services/src/profiles.rs:80]. `x != "toml"` is case-sensitive: `UPPER.TOML` holding a valid copy gave `source=bundled` and no line (checked). AppleDouble `._x.toml` files and emacs `.#x.toml` locks are read and refused. Fix: compare with `eq_ignore_ascii_case("toml")` and skip names starting with `.`. Document both rules in the score-format doc. → Fixed. `is_candidate`: `.toml` in any case, never a dot-file. Documented. Red proofs A6, A11, A12.
- [x] [Review][Patch] **Whitespace around `name` / `game_version` makes a copy silently "other identity"** (low) [app/core/src/profile.rs:487-497, 691]. `text_field` checks `trim().is_empty()` but stores the value untrimmed, and `resolve` compares exactly. So `game_version = "1.1.1 "` is valid but only `ignored other_identity`, and the user's edits never apply. Fix: refuse leading or trailing whitespace as `invalid`, which is consistent with D5 (strict, loud). → Fixed. Leading or trailing spaces are refused as `invalid`. Red proof V13.
- [x] [Review][Patch] **The G.27 source check does not pin the call inside `App::new`** (low) [app/gui/tests/profile_app.rs:90-108]. It counts `profiles::load()` anywhere in `app.rs`, comments included. If the call moved into `build`, every test constructor would read the real config dir and the check would still pass. Fix: slice the `pub fn new(` body (up to the next `fn `) and assert that the call is in it, and that `fn build` has `bundled_only()` and no `load()`. `profile_lines` should match the kind tag exactly (`" profile refused|ignored|loaded "`), not any `" profile "`. → Fixed. The check slices the `pub fn new` and `fn build` bodies (comments stripped). `profiles::load()` must be in `new` and nowhere else, and `bundled_only()` in `build`. `profile_lines` matches the kind field exactly. Red proofs G3, G4.
- [x] [Review][Patch] **The `c30_one_parser` exemption is wider than `profile.rs`** (low) [app/core/tests/profile.rs:493]. `rel.starts_with("core/src/profile")` also exempts any future `core/src/profile_x.rs` or `core/src/profiles/…`. Fix: exempt `core/src/profile.rs` exactly, or `core/src/profile/` if the module is split later. → Fixed. The exemption is exactly `core/src/profile.rs`.
- [x] [Review][Patch] **The doc says shape errors carry a line, but migrated files never do** (low) [app/core/src/profile.rs:434, docs/score-format-and-mapping.md › Loading and validation step 3]. `shape_error(&e, None)` runs on the migration path. Fix: change the doc to "with the line when the file needed no migration", or keep spans. → Fixed (doc wording). The `Invalid` doc comment and the score-format doc say the line exists only for a file at the current schema.
- [x] [Review][Patch] **Red proofs are missing or stale (AC 13, E4-A3)** (low) [Debug Log › Red proofs]. → Fixed. The missing red proofs were added (V14, L1, SH1–SH3, F2 for `c30_fuzz_edits`, B1 for `c31_bundled_is_the_repo_file`, G3 for `g27_test_constructor_uses_the_bundled_default`, A6–A8, A11–A13). Every old one was re-run on the final tests, with current line numbers in the Debug Log.
  - Missing for: `c30_fuzz_edits`, `c31_bundled_is_the_repo_file`, `g27_test_constructor_uses_the_bundled_default`, the empty `name` / `game_version` rule, the `line: Some(_)` assertions (parse and shape), the missing-key, wrong-type and unknown-enum cases of `c30_shape`, the `*.toml` filter, and the 160-character cap and newline flattening of `detail` (A5 covers only quotes).
  - Stale: S1 `:266`, S2 `:288`, V11 `:316` and O1 `:484` point to lines from before the final `c30_shape` change (now about 291, 313, 341 and 509). Re-run them on the committed tests.
  - Also cover the tests added by the patches above.
- [x] [Review][Patch] **The Debug Log and the deviation notes are inaccurate** (low) [story Debug Log and Completion Notes]. → Fixed in the Debug Log and the Completion Notes (`toml_writer`, ints into `f64`, the API deviations, the AC wording).
  - `toml_writer` does not come from `egui_kittest` turning on `display`. `egui_kittest` 0.37 enables only `parse` and `serde`; `cargo tree -i toml_writer` prints nothing; `toml_writer` and `indexmap` are weak (`?/std`) lock entries and are not compiled. `indexmap` is not mentioned at all.
  - The "TOML ints into `f64`" check that the Dev Notes asked for is not recorded (it works: serde accepts `visit_i64` for `f64`).
  - Undocumented deviations: `Schema` has no `supported` field (Task 2.4, AC table `Schema { found: 2, .. }`); `Invalid` has a `line` field and `Io` is a fourth kind (D3 says three); there is no `ProfileEvent::Loaded` (Task 2.5); the 600-frame and 600 000 ms caps and the non-empty name rule are not in AC 8.
  - AC 10 still says `App::profile()` (the code has `mapping_profile()`), and AC 3 says "six timing values" (there are five).

**Defer**

- [x] [Review][Defer] **Cross-field invariants are not validated** [app/core/src/profile.rs:499-615]: deferred. AC 8 does not ask for them; they belong to Story 5.4 (fingering) or Epic 9 (editor). These all pass today: offsets with the wrong sign or zero (`dpad_up = -1`), `stick_up_ly == stick_down_ly`, a fingering pitch that does not match button + offsets, zero or several `chart` fingerings per pitch, and a button pitch outside the range.
- [x] [Review][Defer] **Cost weights have no upper bound** [app/core/src/profile.rs:475]: deferred to Story 5.6 (cost function). `1e308` passes and can sum to `inf`.
- [x] [Review][Defer] **`c30_one_parser` bans every `Deserialize` / `serde::` outside the profile module** [app/core/tests/profile.rs:480-484]: deferred to Story 5.12/5.13. The settings file (`settings.toml`) will need its own TOML reading, so the scan must narrow to profile types, or settings must also live in the core.

**Dismissed (9):**
- `line_of` fallback: toml spans lie inside the text, on char boundaries.
- ProjectDirs macOS path: `directories` 6.0.0 drops empty qualifier and organisation, so the path is `…/Application Support/Bonooru` as documented.
- `bundled_only` panic: documented, with a local allow, and C.30/C.31 parse that text.
- C.4 fixtures as inline strings: the existing `c4_manifest_rules` style.
- `c31_settings_apart` early return without a home dir: CI runners have one.
- No test at the `max_sustain` boundary: a nit.
- BOM, CRLF, duplicate keys, overflow, migration termination: verified fine.
- Task 7 / CI not done yet: expected before the PR.
- The `version="=` dead check: folded into the C.4 patch.

## Dev Agent Record

### Agent Model Used

Claude Opus 5.5 (claude-opus-5-5)

### Debug Log References

**Correction (review 2026-11-28).** `toml_writer` 1.2.0 and `indexmap` are lock entries only, not compiled crates. Cargo records the optional dependencies that `toml`'s weak `?/std` features name, but `cargo tree -i toml_writer` prints nothing, so they are not built. The "egui_kittest turns on display" explanation below is wrong.

**TOML integers into `f64` (Dev Notes check).** serde's `f64` visitor accepts integers, so the cost weights `1`, `2`, `1`, `1` in the data file deserialise as `1.0`, `2.0`, `1.0`, `1.0`. `c30_bundled_values` pins them, and the data file did not change.

**Dependencies (E4-A6, Task 1.1)**, re-checked with `cargo info` on 2026-11-28:
- `toml` 1.2.3+spec-1.1.0: MIT OR Apache-2.0, MSRV 1.85. Used with `default-features = false` and features `std`, `parse`, `serde`. Its normal deps under ocb-core are `serde_core`, `serde_spanned` 1.1.3, `toml_datetime` 1.1.3, `toml_parser` 1.1.7 and `winnow` 1.0.5.
- `serde` 1.0.229: MIT OR Apache-2.0, MSRV 1.56. Used with feature `derive` (`serde_derive`).
- Both versions were already in `Cargo.lock` through `egui_kittest` (the dev graph). The lock gains the two dependency entries of `ocb-core` and one new crate, `toml_writer` 1.2.0 (MIT OR Apache-2.0). It comes in through `toml`'s `display` feature, which `egui_kittest` turns on; cargo features are additive across the workspace.
- `cargo deny check`: advisories, bans, licenses and sources ok.

**Red-first runs:**
- C.4 manifest cases before the gate change: `c4_manifest_rules` panicked "flagged: [dependencies] toml = { version = \"=1.2.3\" …".
- C.30 before the module existed: `error[E0432]: unresolved import ocb_core::profile`.
- C.31 before the adapter existed: `error[E0432]: unresolved import ocb_services::profiles`.
- G.27 before the App change: `error[E0599]: no method named profile / profile_source`.

**Red proofs (E4-A3).** Each mutation was applied to the green tree with a scratch script (not committed), the named test was run, and the file was restored. Every row failed as shown:

| # | Mutation | Test | Red result |
|---|---|---|---|
| P1 | The gate accepts any runtime crate | C.4 `c4_manifest_rules` | "not flagged: [dependencies] toml … regex …" |
| P2 | The gate accepts an unpinned `toml`/`serde` | C.4 `c4_manifest_rules` | "not flagged: [dependencies] toml = \"1.2.3\"" |
| P3 | The gate accepts a `path` source | C.4 `c4_manifest_rules` | "not flagged: [dependencies.toml] path = …" |
| P4 | `include_str!` dropped from the banned macros | C.4 `c4_every_bad_fixture_gives_its_own_hit` | "include_str.rs.txt: expected hit `include_str!`, got []" |
| V1 | `deny_unknown_fields` dropped on `RawTiming` | C.30 `c30_shape` | **first try green**: the case `min_pres_frames` also removed the required key, so the missing-key error hid the unknown one. Fixed by adding `zz_extra = 1` to every table and to the top level while keeping every required key; it now fails at `profile.rs:264` |
| V2 | `frame_rate` 0 accepted | C.30 `c30_validation_rules` | "expected Invalid, got Ok(Profile { … frame_rate: 0 …" |
| V3 | `low >= high` accepted | C.30 | `left: "fingerings[0].pitch" right: "range.low"` |
| V4 | Offset ±13 accepted | C.30 | "expected Invalid, got Ok(… dpad_up: 13 …" |
| V5 | Negative cost accepted | C.30 | "expected Invalid, got Ok(…" |
| V6 | NaN cost accepted | C.30 | "expected Invalid, got Ok(…" |
| V7 | Duplicate fingering accepted | C.30 | `left: "fingerings" right: "fingerings[44]"` |
| V8 | Uncovered pitch accepted | C.30 | "expected Invalid, got Ok(…" |
| V9 | Fingering pitch only 0–127 checked | C.30 | `left: "fingerings" right: "fingerings[44].pitch"` |
| V10 | The load-time AD-15 floor removed | C.30 `c30_resolve`, `c30_validation_rules` | `left: User { file: "d-invalid.toml" } right: Bundled` |
| V11 | `check_report_period` uses the frame time only | C.30 `c30_timing_floor` | panics at `profile.rs:316` (20 ms not refused) |
| V12 | Max sustain under the min press accepted | C.30 | "expected Invalid, got Ok(…" |
| S1 | Schema 2 accepted | C.30 `c30_schema_and_migration`, `c30_resolve` | panics at `:266`; `left: User { file: "c-schema.toml" }` |
| S2 | Migration step not applied | C.30 `c30_schema_and_migration` | panics at `:288` |
| R1 | A refused user copy overrides | C.30 `c30_resolve` | `left: User { file: "a-io.toml" } right: Bundled` |
| R2 | Another identity overrides | C.30 `c30_resolve` | `left: User { file: "n.toml" } right: Bundled` |
| R3 | The duplicate rule picks the last | C.30 `c30_resolve` | `left: User { file: "z.toml" } right: User { file: "a.toml" }` |
| R4 | Refusal message text changed | C.30 `c30_resolve` | `left: "This profile can't be read. …" right: "This mapping profile can't be read by this version. …"` |
| R5 | A pinned bundled value changed in the data file (6000 → 6100) | C.30 `c30_bundled_values` | `left: (3, 5, 1, 6100, 500)` |
| O1 | A `toml::from_str` added to `services` | C.30 `c30_one_parser` | panics at `:484` (offender listed) |
| F1 | `parse` panics when `len % 37 == 5` | C.30 `c30_fuzz_arbitrary` | proptest finds it (panic at `profile.rs:403`). The regression file that proptest wrote for the mutated code was deleted |
| A1 | A missing dir treated as refused | C.31 `c31_load_from_dir` | `assertion failed: r.events.is_empty()` |
| A2 | An unreadable file skipped silently | C.31 `c31_load_from_dir` | `left: 2 right: 3` (no `dir.toml` line) |
| A3 | Settings inside the profiles dir | C.31 `c31_settings_apart` | panics at `:132` |
| A4 | Full path instead of the file name | C.31 `c31_load_from_dir`, `c31_log_lines` | `left: User { file: "/var/folders/…/profiles/mine.toml" }` |
| A5 | Quotes not escaped in `detail` | C.31 `c31_log_lines` | panics at `:119` |
| G1 | `.profile()` writes no lines | G.27 | `left: [] right: ["refused file=mine.toml …", "loaded …"]` |
| G2 | `App::new` uses `bundled_only()` | G.27 `g27_new_loads_through_the_adapter` | "App::new must call profiles::load()": `left: 0 right: 1` |

**Green runs (macOS, 2026-11-28):**
- fmt ok; clippy `-D warnings` ok on all targets.
- `cargo test --workspace` twice: 561 passed, 0 failed (Story 5.0 ended at 545: +9 C.30, +4 C.31, +3 G.27).
- deny ok.
- Release smoke `bonooru --smoke-test`: `OCB_SMOKE ok … worker=1`, and the real log has `profile loaded name="OoT Switch 2" game_version=1.1.1 schema=1 source=bundled file=-` (no user dir on this Mac).

**Review fixes (2026-11-28): red proofs re-run on the final tests.** Same method. Line numbers are those of the final files.

| # | Mutation | Test | Red result |
|---|---|---|---|
| P1 | Any runtime crate allowed | C.4 `c4_manifest_rules` | "not flagged" at `purity.rs:472` |
| P2 | Pin not checked to be exact | C.4 | "not flagged" |
| P3 | `path` / `git` dropped from the forbidden keys | C.4 | "not flagged" |
| P5 | `package` dropped from the forbidden keys | C.4 | "not flagged" at `:472` |
| P6 | `=1` accepted as exact | C.4 | "not flagged" at `:473` |
| P7 | A missing `version` accepted | C.4 | "not flagged" at `:472` |
| V13 | Spaces around `name` / `game_version` accepted | C.30 `c30_validation_rules` | "expected Invalid, got Ok" at `profile.rs:45` |
| V14 | Empty name accepted | C.30 | "expected Invalid, got Ok" at `:45` |
| L1 | Shape errors lose their line | C.30 `c30_shape` | panics at `:248` (`line: Some(_)`) |
| SH1 | A missing key defaulted (`#[serde(default)]` on `start_lead_ms`) | C.30 `c30_shape` | panics at `:246` |
| SH2 | `ZL` accepted as a button (`alias`) | C.30 `c30_shape` | panics at `:246` |
| SH3 | `left` accepted as a D-pad direction | C.30 `c30_shape` | panics at `:248` |
| R6 | `refusal_message` ignores the source | C.30 `c30_resolve` | `left == right` failed at `:447` |
| F2 | `parse` panics on U+FFFD (what random byte edits produce) | C.30 `c30_fuzz_edits` | panic found by proptest |
| B1 | The embedded text gains a trailing newline | C.31 `c31_bundled_is_the_repo_file` | `left == right` failed at `profiles.rs:30` |
| A6 | Every file is a candidate | C.31 | panics at `:153` |
| A7 | `detail` not capped at 160 | C.31 `c31_log_lines` | panics at `:117` |
| A8 | Control characters kept | C.31 `c31_candidates_and_hostile_files` | panics at `:201` (a line contains `\n`) |
| A9 | FIFO opened without the type check | C.31 | **hangs** (killed after 120 s): the bug itself |
| A10 | No size cap (both checks and `take` removed) | C.31 | `big.toml` read whole: "reason=schema …" instead of "larger than 1048576 bytes" at `:186` |
| A11 | Dot-files read | C.31 | `!is_candidate("._mine.toml")` at `:153` |
| A12 | Extension compared case-sensitively | C.31 | `is_candidate("MINE.TOML")` at `:151` |
| A13 | An unreadable profiles dir is silent | C.31 | lines `["loaded …"]` only, at `:217` |
| A14 | File names unquoted | C.31 | fails on the quoted `file="…"` lines |
| G3 | `led_log` re-logs the profile | G.27 `g27_test_constructor_uses_the_bundled_default` | panics at `profile_app.rs:50` |
| G4 | `profiles::load()` moved from `new` into `build` | G.27 `g27_new_loads_through_the_adapter` | "only App::new loads" at `:125` |
| V1–V12, S1, S2, R1–R5, O1, F1, A1–A5, G1, G2, P4 | (above) | re-run | all still fail; new lines S1 `:301`, S2 `:323`, V11 `:351`, O1 `:536` |

The A9 hang left one test process, which was killed by hand. The temp dirs `ocb-profiles-*` and the proptest regression file written for the F2 mutation were removed.

Green runs after the review fixes (macOS): fmt ok, clippy `-D warnings` ok, `cargo test --workspace` twice 562 passed / 0 failed (+1 `c31_candidates_and_hostile_files`), deny ok.

### Completion Notes List

- **`ocb_core::profile`** (`app/core/src/profile.rs`):
  - `parse` / `parse_with` (migration chain `MIGRATIONS`, empty; `SCHEMA = 1`), `resolve`.
  - Typed `Profile` (`Buttons`, `Modifiers`, `Range`, `Timing`, `Cost`, `Fingering`, `Button` / `Dpad` / `StickDir`), with helpers for the µs durations, `fingerings_for` and `check_report_period` (Epic 6).
  - `ProfileError::{Io, Parse, Schema, Invalid}`, each with a `reason()`, plus `USER_REFUSED_MESSAGE`.
  - Every shape uses `deny_unknown_fields`, and integers are read as `i64` so a value out of range gets this module's own error naming its field.
- **Deviation from the story text:** the log reason for an unreadable file is `read`, not `io`. The core purity gate (C.4) bans the identifier `io` even inside strings, and the reason string lives in the core. The story, the README, the score-format doc and the test plan all say `read`.
- **Naming:** the accessor is `App::mapping_profile()`, because `App::profile(resolved)` is the builder. `App::profile_source()` returns the source.
- **Purity gate C.4:** an allow list with exact `=` pins. `path` / `git` sources and target tables are refused, and the banned words and macros are unchanged.
- **`ocb_services::profiles`:**
  - `BUNDLED` (`include_str!`); `config_dir`, `user_profiles_dir` and `settings_path`; `load_from`, `load` and `bundled_only`; `detail` and `log_lines`.
  - `bundled_only` panics only if the bundled text is invalid, a build bug that C.30 and C.31 rule out. This is documented, with a local `allow(clippy::panic)`.
- **App:** `App::new` calls `.profile(profiles::load())`. The test constructors keep the bundled default and write no line.
- **Docs:** the score-format doc (Loading and validation, user copy locations, log lines), README, test plan (C.4, C.30, C.31, G.27) and the profile header comment.

- **Review 2026-11-28, deviations from the story text, recorded for traceability:**
  - `ProfileError` has four kinds (`Io` for a refused read), not three (D3).
  - `Schema { found }` has no `supported` field; `SCHEMA` is the constant.
  - `Invalid` carries `line`.
  - There is no `ProfileEvent::Loaded`: the `loaded` line comes from `Resolved::source`.
  - The upper bounds (600 frames, 600 000 ms) and the non-empty, no-surrounding-spaces rule for `name` / `game_version` are additions to AC 8.
  - AC 5's "the event carries the message" became `Resolved::refusal_message()`, so the message is never false.
  - AC 10's accessor is `App::mapping_profile()`.
  - The log fields are quoted (`file="…"`, `game_version="…"`).
  - AC 3 says six timing values; there are five.
- **Deferred** (review, kept for later stories):
  - cross-field invariants (offset signs, distinct stick bytes, fingering pitch = button + offsets, one chart fingering per pitch): Story 5.4 / Epic 9;
  - an upper bound on the cost weights: Story 5.5;
  - TOML reading for the settings file, which `c30_one_parser` allows only in `core/src/profile.rs`: Stories 5.12 / 5.13 decide where it lives.

### File List

- `app/core/src/profile.rs` (new)
- `app/core/tests/profile.rs` (new, C.30)
- `app/services/src/profiles.rs` (new)
- `app/services/tests/profiles.rs` (new, C.31)
- `app/gui/tests/profile_app.rs` (new, G.27)
- `app/core/Cargo.toml`
- `app/core/src/lib.rs`
- `app/core/tests/purity.rs`
- `app/services/src/lib.rs`
- `app/gui/src/app.rs`
- `app/Cargo.lock`
- `app/profiles/oot-switch2-1.1.1.toml` (header comment)
- `app/README.md`
- `docs/score-format-and-mapping.md`
- `docs/test-plan.md`
- `_bmad-output/implementation-artifacts/5-1-mapping-profile-module.md`
- `_bmad-output/implementation-artifacts/sprint-status.yaml`
- `_bmad-output/implementation-artifacts/5-0-keyboard-routing-module.md` (status done)

## Change Log

- 2026-11-28: story created (ready-for-dev).
- 2026-11-28: implemented (`ocb_core::profile`, `ocb_services::profiles`, App wiring, purity gate allow list, docs); 32 red proofs; status review.
- 2026-11-28: code review (1 decision, 12 patches, 3 defers) handled: `refusal_message`, a strict C.4 pin check, hostile user files (FIFO, size, dir errors, dot-files, case), quoted log fields, whitespace in the identity, G.27 body check; red proofs re-run (58 in all).
