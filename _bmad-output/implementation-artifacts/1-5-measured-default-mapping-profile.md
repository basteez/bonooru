---
baseline_commit: 6065ade85886afeb0250bffbd5be117e4dfe3556
---

# Story 1.5: Measured default mapping profile

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As the owner,
I want the measured mapping and timing values written into the bundled default profile,
so that the app pipeline (Epic 5) starts from real data.

## Acceptance Criteria

Copied from `_bmad-output/planning-artifacts/epics.md`, Story 1.5. These are the definition of done.

1. **Given** the results of Stories 1.1, 1.3 and 1.4
   **When** the owner writes `app/profiles/` default profile for the game version tested, following the TOML shape in `docs/score-format-and-mapping.md`
   **Then** it has an integer `schema` field, `[profile]` (name, frame_rate), `[buttons]` (sounding MIDI pitches), `[modifiers]` (including stick value), every alternative fingering, `[timing]` (measured value + a stated safety margin, including `max_sustain` from U1), `[range]` (B4-F6) and the fingering cost weights
2. **And** every value has a comment pointing to the spike row it came from
3. **And** roadmap M2 is marked DONE only when no HYPOTHESIS marker is left on pitches/modifiers and `[timing]` is filled

Added by this story file (derived from the owner's brief, not from the epic): no value may claim more than the data proves; estimates, owner choices and margins are labelled as such in the comment next to the value.

## Tasks / Subtasks

No Switch is needed for any task: the profile is written from values already measured. **The console stays off.** The dev agent never opens a serial port. Write no Rust, no firmware, no app code.

- [x] Task 1: Settle the profile shape and its units (AC: #1)
  - [x] 1.1 Use the shape in "Profile design" below (the owner decisions in "Owner decisions" are applied). Where it differs from the TOML example in `docs/score-format-and-mapping.md` (game version field, two stick values, timing in frames, `[[fingerings]]`, `[cost]`), update that doc in Task 5 so the doc and the file agree.
  - [x] 1.2 If the owner answers an open question differently before the dev run, apply the answer instead; do not guess a third option.
- [x] Task 2: Write the default profile `app/profiles/oot-switch2-1.1.1.toml` (AC: #1, #2, and the "no overclaiming" rule)
  - [x] 2.1 Header comment: what the file is, the tested environment (Switch 2 system 23.5.0, OoT 1.1.1, `docs/test-plan.md`, "Test environment"), that it targets game version 1.1.1 only, that pitches are **sounding** pitches (one octave above the community chart), and the legend of tags used in comments: `MEASURED`, `OWNER CHOICE`, `MARGIN`, `ESTIMATE`, `DESIGN DEFAULT`.
  - [x] 2.2 `schema = 1` (integer), `[profile]` with `name`, `game_version = "1.1.1"`, `frame_rate = 60` (U10, test plan row 2.5, caveat: sustained 60 images per second not shown, frame period 16.67 ms measured when the picture changes).
  - [x] 2.3 `[buttons]` L=74, R=77, A=81, Y=83, X=86 (test plan 1.1 and 1.9, `docs/ocarina-input-mechanics.md` "Note buttons").
  - [x] 2.4 `[modifiers]` semitone offsets `dpad_up = 1`, `dpad_down = -1`, `stick_up = 2`, `stick_down = -2` (test plan 1.2, 1.9) and the stick values as canonical state bytes (AD-10): `stick_up_ly = 0x00`, `stick_down_ly = 0xFF`, `stick_lx = 0x80` (U6, test plan 1.6). Comments: full travel only gives the full +-2 (50 % about +-105 c, 75 % about +-160 c); wire values 0xE00 / 0x20C; up and down mirrored within a few cents; any X component reduces the bend (diagonals about +-142 c), so `lx` stays centred; the bend still glides at the note start (first 30 ms about -29 c on Y + stick up, row 2.3), which is why stick modifiers keep a lead.
  - [x] 2.5 `[range]` `low = 71` (B4), `high = 89` (F6) (test plan 1.2; all 19 pitches verified).
  - [x] 2.6 `[[fingerings]]`: **all 45** button x modifier combinations from the table "Alternative fingerings (all 45 combinations)" in `docs/ocarina-input-mechanics.md` (test plan 1.9). One row each: `button`, `dpad` (`none`/`up`/`down`), `stick` (`none`/`up`/`down`), `pitch` (MIDI number), `chart` (true for the 19 chart fingerings, the **bold** ones). A trailing comment gives the measured Hz from the table. Transcribe, do not derive: the file is the data, the consistency check in Task 3 compares it with the model.
  - [x] 2.7 `[timing]` with the values in "Timing values and margins" below, each with its tag, its source row, the measured value, the pass counts around it and the margin applied.
  - [x] 2.8 `[cost]` fingering cost weights = the "first idea" table in `docs/score-format-and-mapping.md`, tagged `DESIGN DEFAULT (not measured)`; say in a comment what the measurements do and do not support (see "Fingering cost weights").
  - [x] 2.9 Comment rule: every key (or its table) has a comment with the spike row or doc section. No comment may state a value as measured when it is an estimate, a choice or a margin.
- [x] Task 3: Consistency check without hardware (AC: #1; justification in "Validator decision")
  - [x] 3.1 `spikes/test_default_profile.py`: throwaway spike test (MIT line, standard library `tomllib` plus `pytest`; needs Python 3.11 or newer, run with `uv run --python 3.12 --with pytest pytest spikes/test_default_profile.py`). It is **not** the AD-17 loader and must say so in its header. Checks only:
    - `schema` is an int; `frame_rate` is 60; required tables and keys exist;
    - exactly 45 distinct (button, dpad, stick) rows; every row's `pitch` equals `buttons[button] + dpad offset + stick offset`; exactly 19 distinct pitches covering `range.low` to `range.high` with no gaps and none outside; at least one `chart = true` row per pitch and exactly 19 chart rows;
    - the chart fingerings equal the 19-row table in `docs/ocarina-input-mechanics.md` ("Fingering chart") for a handful of anchor pitches written in the test (B4, G5, A5, F6);
    - `min_press_frames` and `min_gap_same_button_frames` are at least 2 (AD-15: not below `2 x max(report_period_us, frame time)`, the report period is unknown until Epic 2, so the frame-time bound is checked here and Story 5.1 adds the rest); `max_sustain_ms` is below 6000;
    - `stick_up_ly` is `0x00`, `stick_down_ly` is `0xFF`, `stick_lx` is `0x80`;
    - every non-blank, non-header line holding a `key = value` outside `[[fingerings]]` rows has a `#` comment on the same line or a comment block directly above (cheap guard for AC 2).
  - [x] 3.2 Run it; run `uvx ruff check spikes/` ad hoc. The existing 483 spike tests are not touched.
- [x] Task 4: Check the data for the "no HYPOTHESIS left on pitches/modifiers" condition (AC: #3)
  - [x] 4.1 `grep -rn HYPOTHESIS docs/ _bmad-output/specs/` and record in the Dev Agent Record each remaining marker with the decision from "HYPOTHESIS markers: what can go" below. Confirm none is on a pitch or modifier.
- [x] Task 5: Doc updates (only where data supports; no new claims)
  - [x] 5.1 `docs/score-format-and-mapping.md`: replace the placeholder TOML example with the final shape (short, pointing to `app/profiles/oot-switch2-1.1.1.toml` as the source of truth), fix the `frame_rate` and `max_sustain` placeholder comments, keep the cost table and say the weights now live in `[cost]` and are unmeasured. Keep all other content.
  - [x] 5.2 `docs/test-plan.md`: Spike 1 "Done when" note (lines 108-109) and Spike 2 "Done when" paragraph: say the profile was written in Story 1.5 with a pointer, and that `max_sustain` is an estimate. Add a short "Story 1.5" line under Spike 2 or in the Test environment note that the profile targets OoT 1.1.1 / system 23.5.0. Do not add trial results (there are none).
  - [x] 5.3 `docs/ocarina-input-mechanics.md`: only if a statement is now wrong or a pointer helps: add "(profile: `app/profiles/oot-switch2-1.1.1.toml`)" near the intro sentence of "Fingering chart". U1 keeps its text; add "(hand-timed, not measured with a tool; see profile `max_sustain_ms`)" if not already clear.
  - [x] 5.4 `docs/usb-controller-emulation.md` line 85 ("The game samples input once per frame at 60 fps"): split as described in "HYPOTHESIS markers: what can go". Leave every other marker.
  - [x] 5.5 `app/README.md`: one line saying `profiles/` holds the bundled default mapping profiles (TOML, AD-17), first file from Story 1.5. `spikes/README.md`: add `test_default_profile.py` to the index.
- [x] Task 6: Roadmap M2 (AC: #3)
  - [x] 6.1 In `_bmad-output/specs/spec-ocarina-bot/roadmap.md` (the in-repo roadmap; **never touch `~/Documents/org/`**) set M2 State to `DONE 2026-11-21` only after Task 4 shows no HYPOTHESIS on pitches/modifiers and the `[timing]` section has all five keys filled. In the "Done when" cell keep the original sentence and append one clause: timings are measured except `max_sustain_ms` (estimate) and `start_lead_ms` (not measured), both labelled in the profile. If either condition fails, leave M2 TODO and say why in the Completion Notes.
- [x] Task 7: Close-out
  - [x] 7.1 Fill the Dev Agent Record (file list, tests run, decisions applied, open questions answered by the owner). Set `1-5-measured-default-mapping-profile` to `review` in `sprint-status.yaml` when done.
  - [x] 7.2 Housekeeping: no temp files, no recordings, no UF2/2wiCC binaries, no game screenshots, nothing under `ux-designs/*/imports/`, `_bmad/` and skill folders untouched. Do not commit or push unless the owner asks.

## Dev Notes

### What this story is

A data-transcription story. The measurements exist (Stories 1.1, 1.3, 1.4); this story turns them into one TOML file plus honest margins and doc updates. The code that reads the file is Story 5.1 (Rust, `ocb-core`); the first consumer is Story 1.6 (spike code that builds a melody by hand from the profile). Nothing here touches firmware, `app/` Rust code or the Switch.

### Profile design (shape, with the reasons)

Target path: `app/profiles/oot-switch2-1.1.1.toml` (bundled default, one per game version, AD-17; `app/profiles/` is in the architecture tree; only `app/README.md` exists today).

```toml
schema = 1                       # integer, AD-17

[profile]
name = "OoT Switch 2"            # the pair (name, game_version) is what a user copy overrides (AD-17)
game_version = "1.1.1"
frame_rate = 60

[buttons]    # L, R, A, Y, X = 74, 77, 81, 83, 86
[modifiers]  # dpad_up/down, stick_up/down (semitones); stick_up_ly, stick_down_ly, stick_lx (state bytes)
[range]      # low = 71, high = 89
[timing]     # *_frames integers for the frame-grid values, *_ms for the others
[cost]       # fingering cost weights
[[fingerings]]  # 45 rows: button, dpad, stick, pitch, chart
```

Reasons for the choices that differ from the placeholder in `docs/score-format-and-mapping.md`:

- **`game_version`**: AD-17 says a user copy overrides the bundled default "with the same name and game version", so the version must be a field, not only part of `name`. The old name "OoT Switch 2 v1.0.0" is wrong: the tested game version is 1.1.1.
- **Two stick values instead of `stick_value`**: the data needs different canonical bytes for up and down (up `ly 0x00`, down `ly 0xFF`, neutral 0x80; AD-10 says `ly 0x00` = up). One `stick_value = 0x00` cannot say it, and deriving down as `0xFF - up` hides a fact measured in test plan 1.6 (down max is 0xFF = wire 0x20C, mirrored within a few cents). Explicit bytes mean the pipeline copies them into the canonical state without arithmetic. `lx = 0x80` is explicit because any X component reduces the bend.
- **Timing in frames** for `min_press`, `min_gap_same_button`, `modifier_lead`: every measurement is in frames, the pipeline snaps to the frame grid, and 2 or 5 frames in ms is not an integer (33.33, 83.33), so ms values risk off-by-one rounding at the snap. Put the ms in the comment. `max_sustain_ms` and `start_lead_ms` stay in ms (not on the frame grid).
- **`[[fingerings]]` rows**: "every alternative fingering" is explicit data (45 measured rows), not derived. The pitch of each row equals button + D-pad + stick offsets for all 45 (verified in Story 1.3), which is exactly what the Task 3 check asserts; it also gives Story 5.1 a ready validation rule.
- Story 5.1 may adjust the shape (it owns the schema and the migration). If it does, it bumps or keeps `schema` as appropriate and updates this file; Story 1.5 does not try to guess 5.1's types.

### Timing values and margins

All frames at 60 Hz (16.67 ms). "Measured" comes from test plan "Story 1.4 field trials" > "Values for Story 1.5" (rows 2.1 to 2.5). Caveat to copy into the file header: the analyser definitions behind 1-frame U7 counts, U2 at 2 to 3 frames and the jitter sigma were revised after seeing the recordings; the final numbers should be confirmed on fresh recordings in Epic 7 (Story 7.4 updates these margins from data).

| Key | Value in profile | Tag | Measured | Margin applied | Source |
|---|---|---|---|---|---|
| `min_press_frames` | **3** (50 ms) | MARGIN | 2 frames (33.3 ms) 20/20 on A, L, R, X, Y; 1 frame 16 to 18/20 | +1 frame. 3 to 10 frames were 20/20 on A only; 3 frames on L, R, X, Y was not run, the choice assumes monotonic behaviour | row 2.1, U7 |
| `min_gap_same_button_frames` | **5** (83.3 ms) | OWNER CHOICE | not a measured limit: 4 frames 20/20 on A, L, Y; R 5 frames 19/20 pooled (20/20 with its own file's threshold); X unresolved (best 4 frames 18/20, mic level); R and X not run above 5 frames | none added: 5 is already 1 frame above the A, L, Y limit; "retrigger" is a threshold on a continuous dip, so the value is definition-dependent | row 2.2, U2 |
| `modifier_lead_frames` | **1** (16.7 ms) | MARGIN | 0 frames for D-up, D-down, stick up, stick down (no wrong pitch longer than about 35 ms / 2 frames; 20/20 at every lead 0 to 5 on A; L, R, X, Y at 0 frames 20/20 except X + stick down 19/20 and Y + stick up 19/20) | +1 frame, as the Story 1.4 results require (at least 1 frame, because audio cannot resolve a 1 to 2 frame lateness). One value for all four modifiers: all measured 0, so no per-type split | row 2.3, U5 |
| `max_sustain_ms` | **5000** | ESTIMATE | about 6 s, hand-timed with no tool (1 s, wavers from about 2 s, stops by itself at about 6 s while held) | about 1 s (17 %) below the hand-timed ~6 s, to absorb hand-timing error and a possible per-note variation | test plan 1.3, U1 (owner decision 1; short U1 test deferred to Story 1.6) |
| `start_lead_ms` | **500** | DESIGN DEFAULT | not measured | none; carried over from the placeholder; Story 2.5 / Story 6.x use it as `START lead_us`, and the countdown the user sees is separate | `docs/score-format-and-mapping.md`, AD-15 |

- AD-15 rule check: min press and min gap must not fall below `2 x max(report_period_us, frame time)`. The frame-time part is 2 frames = 33.3 ms; 3 and 5 frames pass. The `report_period_us` part is measured in Epic 2; Story 5.1 enforces the full rule.
- Wording rule for comments: a margin line says "measured X, profile Y, margin Z" so a reader never takes Y for a measurement. Do not write "20/20" next to a value that was not run.
- Jitter (row 2.4, sigma 0.48 to 0.70 frame) is not a profile key; it is the basis for Epic 2 / 7 targets. A one-line comment in the header may cite it.

### Fingering cost weights

The cost table (first idea) in `docs/score-format-and-mapping.md`: no modifier change 0, D-pad change 1, stick change 2, stick passes through centre (up to down) +1, same button retriggered +1, non-natural fingering 0.1. Evidence check for what the data supports:

- Supported qualitatively: the D-pad steps at once (U4), the stick glides (U4, and the first 30 ms of a stick note read about -29 c, row 2.3), so a stick change is a worse choice than a D-pad change: order 0 < D-pad < stick holds.
- Not supported by any measurement: the exact numbers (1, 2, +1, +1, 0.1). No test compares two fingerings of the same note for audible quality.
- X + D-pad shifts about +-115 c instead of 100 c (about 30 c between two X variants of C-sharp6 / D-sharp6): this is a tuning difference between fingerings, not a transition cost. Do not encode it in weights; mention it in the `[cost]` comment as a possible later refinement.
- Therefore: carry the first-idea numbers into `[cost]` as `DESIGN DEFAULT (not measured)`, keys named after the table rows (`dpad_change`, `stick_change`, `stick_through_centre`, `same_button_retrigger`, `non_natural`). Tuning is a later task (Story 1.6 listening result, Story 5.5 golden tests, Epic 7). "Non-natural" means `chart = false` in `[[fingerings]]`.

### Validator decision

Decision: add one small consistency test, `spikes/test_default_profile.py`, and **no loader**.

- The AC does not ask for a test and AD-17 gives loading, validation and migration of profiles to the core `profile` module (Story 5.1, Rust). So no parser or validator goes into `app/`, and the test is not a second profile loader (AD-17 "prevents several parsers with different defaults"): it only reads the TOML with `tomllib` and asserts facts about the data.
- Why it is still worth its few lines: the file is 45 hand-transcribed rows, nobody can re-measure them in this story, and Stories 1.6 and 5.1 will trust it. The model "pitch = button + D-pad + stick" holds for all 45 measured combinations, so a typo shows up as a mismatch immediately.
- It lives in `spikes/` (throwaway Python, like the other 483 tests) and is replaced by Story 5.1's Rust tests. The owner chose to keep it (owner decision 4).

### HYPOTHESIS markers: what can go

Condition for M2 (AC 3): none on pitches/modifiers, `[timing]` filled. Current state from `grep`:

- `docs/ocarina-input-mechanics.md`: no marker; pitches and modifiers are `[VERIFIED 2026-11-20]` (Story 1.3). `docs/test-plan.md` lines 108-109 already record this.
- Remaining markers, none about pitches or modifiers:
  - `docs/score-format-and-mapping.md:101` live keyboard latency "3 to 6 frames": **keep**. Story 1.4 measured about 123 to 139 ms (about 7.4 to 8.3 frames) from the scheduled press to sound, but through the 2wiCC sender and a microphone, not the live-keyboard chain.
  - `docs/usb-controller-emulation.md:81` Switch 2 polls HID every 1 ms: **keep**, not measured (Epic 2).
  - `docs/usb-controller-emulation.md:85` "The game samples input once per frame at 60 fps": **split, do not delete**. Row 2.5 measured a 16.67 ms frame period when the picture changes (U10) and row 2.4 shows latency clusters one frame apart, consistent with per-frame sampling at 60 Hz. The input sampling rate itself is not directly measured and a sustained 60 images per second is not shown. Suggested text: a measured sentence for the frame period (with pointers to rows 2.5 and 2.4) and the remaining claim "input is sampled once per frame" still marked HYPOTHESIS, noted as "consistent with row 2.4". "Could be 30 fps in some scenes" stays as a caveat (only one scene type tested).
  - `docs/link-protocol.md:27` 460800 baud: **keep** (Epic 3).
  - `_bmad-output/specs/spec-ocarina-bot/requirements.md:53` (NFR-04 depends on frame rate and input sampling) and `decisions.md` lines 21, 44, 52, 60: **leave unchanged**; they are spec statements outside this story's data, and the input-sampling part is still unproven.
- Do not remove a marker because it looks outdated; remove or split only with a pointer to the data row.

### Developer guardrails

- English everywhere in the repo; chat with the owner is Italian.
- Pitches in the profile are the **sounding** pitch (MIDI), not the community chart name. Check: A = 81 (A5), not 69.
- Allow-list (AD-9): the profile may only name L, R, A, Y, X, D-pad up/down and the left stick. Do not add other buttons, the right stick or ZL/ZR (ZL opens the song menu, U11).
- The file never claims 20/20 where only A was run; never calls `min_gap_same_button` a measured limit; never calls `max_sustain` measured.
- Do not copy trial data tables into the profile; one comment line per value, with row id and doc section.
- No recorded audio or video, no game screenshots, no UF2/2wiCC binaries, never reference anything under `ux-designs/*/imports/`. Do not touch `_bmad/` or skill folders. Never read-for-edit or modify anything under `~/Documents/org/`.
- Do not change `send_2wicc.py`, `listen_trial.py` or any existing spike test.

### Architecture compliance

- **AD-17**: profile = TOML with integer `schema`; one bundled default per game version, read-only; mapping and timing values live only here, never in code. Story 5.1 owns loading and migration.
- **AD-10**: stick values are canonical state bytes (`ly 0x00` = up, centre 0x80); the hat is not stored as a number in the profile (D-pad up/down as names), the pipeline maps to the hat via the canonical state.
- **AD-9**: the fingerings use only allow-listed inputs.
- **AD-15**: minimum durations not below `2 x max(report_period_us, frame time)`; the profile's frame grid is `frame_rate`. `t_us` events are snapped to this grid by the pipeline, not here.
- Repo layout (architecture tree): `app/profiles/` bundled default profiles (TOML). No Cargo workspace exists yet; do not create one.

### Library and framework requirements

None added. The test uses the Python standard library (`tomllib`, Python 3.11+) and `pytest`. Latest-tech check: `tomllib` is in the standard library since Python 3.11 and reads hex integers (`0x00`) and inline tables as TOML 1.0 specifies; the system `python3` on the owner's Mac is 3.9, so run the test through `uv run --python 3.12`. For Story 5.1 (not this story): Rust `toml` + `serde` as the epic says.

### File structure requirements

| File | Action |
|---|---|
| `app/profiles/oot-switch2-1.1.1.toml` | NEW |
| `spikes/test_default_profile.py` | NEW (throwaway consistency test) |
| `docs/score-format-and-mapping.md` | UPDATE (profile shape, placeholders) |
| `docs/test-plan.md` | UPDATE (done-when notes, profile pointer) |
| `docs/ocarina-input-mechanics.md` | UPDATE only if a pointer helps (small) |
| `docs/usb-controller-emulation.md` | UPDATE (split the line 85 marker) |
| `app/README.md`, `spikes/README.md` | UPDATE (one line each) |
| `_bmad-output/specs/spec-ocarina-bot/roadmap.md` | UPDATE (M2) |
| `_bmad-output/implementation-artifacts/sprint-status.yaml` | UPDATE (status) |

### Testing requirements

- `uv run --python 3.12 --with pytest pytest spikes/test_default_profile.py` green; the older run `uv run --with pytest --with pyserial --with numpy pytest spikes/` must stay green (483 tests; they do not touch the profile). `uvx ruff check spikes/` clean.
- There are no field results to record; nothing in this story is a measurement. The only manual check is reading the file against the three sources (test plan 1.1/1.9, `ocarina-input-mechanics.md`, "Values for Story 1.5").

### Consumers (what they need from this file)

- **Story 1.6** (walking skeleton, needs the Switch, owner's session): reads `[buttons]`, `[modifiers]`, `[[fingerings]]` (use `chart = true` rows, or the lowest-cost alternative) and `[timing]` to hand-build timed states for a short melody (for example Ode to Joy) for `send_2wicc.py`. It needs durations in frames: the sender already works in frames. A note longer than `max_sustain_ms` must be re-pressed after `min_gap_same_button_frames`; Ode to Joy has none, so the estimate is not exercised there.
- **Story 5.1** loads and validates it; **5.5** uses the cost weights and timing; **2.5 / 7.4** use margins and may revise them.
- **Story 1.6 follow-up (owner decision 1):** the owner's Story 1.6 Switch session runs a short U1 test (holds of about 4 to 7 s, microphone onset and offset) to replace the hand-timed estimate behind `max_sustain_ms = 5000`; Story 1.6 records the result and updates the profile value and its tag if the data supports it.

### Previous story intelligence (Story 1.4, done)

- Results live in `docs/test-plan.md`, "Story 1.4 field trials" > "Values for Story 1.5": that table is the primary input here. Raw per-run logs are in the Story 1.4 file (Dev Agent Record), not needed except for pass counts.
- Honesty lessons from 1.4's reviews: mark owner-chosen values as such (U2), state analysis limits (about 35 ms lead resolution; analyser definitions), never present an unrun case as passed. The same wording discipline applies to the comments in the profile.
- U2 5 frames is an owner choice; U5 "0 frames" means "at the 2-frame resolution"; U10 "60 Hz" means "frame period 16.67 ms when the picture changes".
- Findings that do not change the profile but are good to know: the first injected input after a controller connect is ignored (relevant to Story 1.6 readiness, not to the profile); on X the D-pad shifts about +-115 c; stick start glide about -29 c in the first 30 ms.
- Story 1.4 also asked for fresh-recording confirmation of 1-frame U7 and U2 counts "in Story 1.5 / Epic 7"; Story 1.5 has no Switch, so this is deferred to Epic 7 (Story 7.4 revises the margins), recorded in the file header.

### Git intelligence

Recent commits: `3c872c5` Story 1.4 (spike generator and analysers, results in docs), `4020d09` Story 1.3, `7097e94` Story 1.2 (merge). Pattern: one branch per story (this one is `story/1-5-default-mapping-profile`), story files in `_bmad-output/implementation-artifacts/`, results and facts in `docs/`, python spike tests next to the spike scripts, no CI for Python yet (the first CI arrives with the first code story).

### Project Context Rules

- No `project-context.md` exists in the repo.
- Repo content (files, comments, commit text) is English; chat with the owner is Italian.
- Never commit `_bmad/`, `bmad-*` or `gds-*` skills; only `_bmad-output/`.
- Never commit or reference anything under `ux-designs/*/imports/`, game screenshots, recorded audio or video, UF2 or other binaries.
- Do not edit `~/Documents/org/`. Do not commit or push unless the owner asks.
- Ask decisions one at a time and keep them short (owner preference); search online or upstream before raising an open question.
- Do not re-check breadboard wiring; the Switch stays off in this story.

### Project Structure Notes

- `app/profiles/` is in the architecture tree and does not exist yet; create it with the profile file. No Rust code, no Cargo files.
- A Python test under `spikes/` is a variance from "tests in `app/tests/`" on purpose (no Cargo workspace yet; throwaway; see "Validator decision").

### Owner decisions (2026-11-21, replace the former open questions)

The owner answered the four open questions before the dev run; the tasks above apply these decisions.

1. **`max_sustain_ms = 5000`, tagged ESTIMATE.** Source: U1 hand-timed at about 6 s with no tool and no error bar (test plan 1.3). A short U1 test (holds of about 4 to 7 s, for example 4, 5, 5.5, 6, 6.5 and 7 s with the existing sender, microphone onset and offset) is **deferred to the owner's Story 1.6 Switch session**; the result may revise this value. The note is recorded in the profile comment and in "Consumers" (Story 1.6 follow-up) below.
2. **`min_press_frames = 3` (50 ms), tagged MARGIN.** Measured limit 2 frames (20/20 on all five buttons, test plan 2.1); 3 frames and above were run on A only; +1 frame margin.
3. **Units:** `min_press_frames`, `min_gap_same_button_frames`, `modifier_lead_frames` as integer frames (ms in comments); `max_sustain_ms` and `start_lead_ms` in ms. `docs/score-format-and-mapping.md` is updated to match.
4. **Consistency test kept:** `spikes/test_default_profile.py` (Task 3), with tests that pass.

Already settled in this story (unchanged): `min_gap_same_button_frames = 5` (OWNER CHOICE, measured limit 4 frames on A, L, Y), `modifier_lead_frames = 1` (MARGIN over measured 0), `start_lead_ms = 500` (DESIGN DEFAULT, not measured), cost weights DESIGN DEFAULT, explicit stick bytes `stick_up_ly = 0x00`, `stick_down_ly = 0xFF`, `stick_lx = 0x80`, file `app/profiles/oot-switch2-1.1.1.toml` with `game_version = "1.1.1"`.

## Dev Agent Record

### Agent Model Used

Claude Opus 5.5 (claude-opus-5-5), gds-dev-story workflow.

### Debug Log References

- Red phase: `uv run --python 3.12 --with pytest pytest spikes/test_default_profile.py` before the profile existed: 13 errors (FileNotFoundError).
- Mutation check: changing the F6 row pitch to 88 and adding an uncommented key made 6 tests fail; restored.
- Extra one-off cross-check (not committed): all 19 `chart = true` rows equal the full "Fingering chart" table in `docs/ocarina-input-mechanics.md` (19/19).
- Final: `uv run --python 3.12 --with pytest pytest spikes/test_default_profile.py`: 13 passed. `uv run --with pytest --with pyserial --with numpy pytest spikes/`: 496 passed (483 existing + 13 new). `uvx ruff check spikes/`: all checks passed.

### Completion Notes List

- Owner decisions applied (recorded in "Owner decisions" above): `max_sustain_ms = 5000` ESTIMATE with the short U1 test deferred to the Story 1.6 Switch session; `min_press_frames = 3` MARGIN (measured 2 + 1); units frames for the three grid values, ms for `max_sustain_ms` / `start_lead_ms`; consistency test kept.
- Profile `app/profiles/oot-switch2-1.1.1.toml`: `schema = 1`, `[profile]` (name, game_version 1.1.1, frame_rate 60), `[buttons]`, `[modifiers]` (offsets + `stick_up_ly 0x00`, `stick_down_ly 0xFF`, `stick_lx 0x80`), `[range]` 71-89, `[timing]` (5 keys), `[cost]` (5 keys), 45 `[[fingerings]]` rows (19 chart). The fingering rows were transcribed mechanically from the 45-combination table in `docs/ocarina-input-mechanics.md` (button, modifiers, measured Hz, bold = chart), not derived from the model; the test then checks them against the model. Every value has a comment with its row / doc section and a tag; no value is tagged MEASURED unless it is.
- Re-check against `docs/test-plan.md` and `docs/ocarina-input-mechanics.md`: buttons (rows 1.1, 1.9), offsets (1.2, 1.9), stick bytes and wire values (1.6), range (1.2, 1.9), timing (2.1, 2.2, 2.3, 1.3), frame rate (2.5, caveat kept). `min_press` comment says 3+ frames were run on A only; `min_gap` is an owner choice, not a measured limit; `max_sustain` and `start_lead` are not called measured; cost weights are DESIGN DEFAULT.
- HYPOTHESIS markers (`grep -rn HYPOTHESIS docs/ _bmad-output/specs/`), none on a pitch or modifier, none in the profile:
  - `docs/usb-controller-emulation.md` "The game samples input once per frame at 60 fps": **split** into a VERIFIED heading for the measured frame period (16.67 ms when the picture changes, row 2.5, with the sustained-60-images and one-scene caveats) and a remaining HYPOTHESIS "The game samples input once per frame" (consistent with row 2.4, not directly measured; 30 fps caveat kept).
  - Kept unchanged: `docs/score-format-and-mapping.md` live keyboard latency 3 to 6 frames; `docs/usb-controller-emulation.md` Switch 2 polls every 1 ms; `docs/link-protocol.md` 460800 baud; `requirements.md:53` (NFR-04); `decisions.md` lines 21, 44, 52, 60; `architecture.md:77` (marker definition) and `.memlog.md` (history). `docs/test-plan.md:108-109` only mention the marker.
- Stale profile key `stick_value` in `docs/ocarina-input-mechanics.md` (two places) replaced by the new keys `stick_up_ly` / `stick_down_ly` / `stick_lx` (no change of fact).
- Roadmap M2 set to `DONE 2026-11-21` in `_bmad-output/specs/spec-ocarina-bot/roadmap.md` only: no HYPOTHESIS on pitches/modifiers and `[timing]` has all five keys; "Done when" keeps the original sentence plus the clause on `max_sustain_ms` (estimate) and `start_lead_ms` (not measured). `~/Documents/org/` not touched.
- Story 1.6 follow-up recorded in "Consumers" (short U1 test, holds of about 4 to 7 s). No Story 1.6 file exists yet; `epics.md` not edited.
- No Switch used, no serial port opened, no Rust/firmware/app code, no recordings, binaries or screenshots added; no temp files left in the repo. Not committed.

### File List

- `app/profiles/oot-switch2-1.1.1.toml` (new)
- `spikes/test_default_profile.py` (new)
- `docs/score-format-and-mapping.md` (modified)
- `docs/test-plan.md` (modified)
- `docs/ocarina-input-mechanics.md` (modified)
- `docs/usb-controller-emulation.md` (modified)
- `app/README.md` (modified)
- `spikes/README.md` (modified)
- `_bmad-output/specs/spec-ocarina-bot/roadmap.md` (modified)
- `_bmad-output/planning-artifacts/epics.md` (modified: key names `max_sustain_ms` / `start_lead_ms` only, code review fix)
- `_bmad-output/specs/spec-ocarina-bot/SPEC.md` (modified: frame period measured, input sampling still unverified, code review fix)
- `_bmad-output/implementation-artifacts/sprint-status.yaml` (modified)
- `_bmad-output/implementation-artifacts/1-5-measured-default-mapping-profile.md` (this story file)

## Change Log

- 2026-11-21: story created (ready-for-dev).
- 2026-11-21: owner decisions recorded (max_sustain 5000 ms ESTIMATE, min_press 3 frames, frame/ms units, test kept); default profile, consistency test and doc updates implemented; roadmap M2 DONE; status review.
- 2026-11-21: code review fixes (owner-approved 1, 2, 3, 5): epics.md key names renamed to `max_sustain_ms` / `start_lead_ms` (lines 67, 226, 1113, 1122, 1308, 1389; line 455, the Story 1.5 AC text, left unchanged); SPEC.md unverified list now says the frame period is measured and once-per-frame input sampling stays unverified; usb-controller-emulation.md frame-period evidence limits (one scene type, 240 fps capture about ±4 ms); profile header tag rule reworded (measured or tunable values carry a tag).
