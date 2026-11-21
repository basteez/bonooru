---
baseline_commit: ea8d5a7c4f5894ef91cddb1808d80a28b0b64a7b
---

# Story 1.6: Walking skeleton plays a whole melody

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As the owner,
I want a whole melody to play on the Switch from a hand-written note list,
so that I hear the project work end to end early (R4).

## Acceptance Criteria

Copied from `_bmad-output/planning-artifacts/epics.md`, Story 1.6. These are the definition of done.

1. **Given** the measured profile from Story 1.5 and the spike sender
   **When** the owner converts a short public-domain melody (e.g. Ode to Joy) by hand or with a few lines of spike code into timed states using the measured fingerings and timing
   **Then** the melody plays in free play with every note audible and the right pitch
2. **And** the result (including any wrong or missed notes) is recorded in `docs/test-plan.md`

Added by this story file (derived from the owner's brief in Story 1.5, "Owner decisions" 1, not from the epic):

3. **And** the short U1 trial (held notes of about 4 to 7 s, microphone onset and offset) is run in the same Switch session and recorded in `docs/test-plan.md`; `max_sustain_ms` in `app/profiles/oot-switch2-1.1.1.toml` is revised (value, tag and comment) if the data supports another value, or re-tagged from ESTIMATE to what the data supports if it does not change.

## Acceptance criteria verdict (2026-11-21)

1. **Melody (AC 1): met at 90 BPM, for the final committed file only.** The final file is the simplified rhythm (owner decision 6) with the 400 ms staccato press cap (owner decision 5). Three official runs, each 62/62 notes audible and at the right pitch; owner's ear "it was fine" on each. Run 3/3 has a raw analyser cut flag on 52 D6 (X sounded 300 of 400 ms), judged a borderline fade and not a missed note. Not shown at 60 or 120 BPM with the final file (60 BPM was run only on the earlier uncapped, syncopated file; 120 BPM was not run). Not shown with legato presses: a held note is cut about every 7th sounded note (unexplained; hypothesis, mechanism unknown), which is why the press cap is used.
2. **Results recorded (AC 2): met.** All runs, wrong and missed notes and the owner's remarks are recorded in `docs/test-plan.md`, section "Walking skeleton (Story 1.6)".
3. **U1 sustain (AC 3): met for button A.** `max_sustain_ms = 6000` in `app/profiles/oot-switch2-1.1.1.toml` (tag MARGIN: measured 6500 ms on A, margin 500 ms). L, R, Y and X were not measured; they are assumed equal to A by owner decision 7, so their value is an assumption, not a measurement.

Tasks 7.4 (120 BPM stress run), 7.5 and 8.2 (U1 confirmation on L, R, Y, X) remain unchecked: not run, 8.2 skipped by owner decision 7.

## Tasks / Subtasks

The story has three parts. **Part A needs no console** (the dev agent does it; the console stays off, the dev agent never opens a serial port). **Part B needs the Switch** and is a separate later phase run by the coordinator with the owner. **Part C** writes the results down after Part B.

### Part A: spike code and tests [no-Switch]

- [x] **Task 1: Melody source file** (AC: #1)
  - [x] 1.1 `spikes/samples/story-1-6/ode-to-joy.melody`: the public-domain theme (Beethoven, Symphony no. 9, "Ode to Joy"; the melody is public domain, transcribe it from a public-domain edition such as IMSLP and name the source in the file header). Plain text, one note per line: `<sounding pitch name> <beats>` (for example `B5 1`, `B5 1.5`), `R <beats>` for a rest, `#` comments. Pitches are the **sounding** pitch (profile convention; A5 = MIDI 81), so no octave shift is needed. English, MIT, no copied third-party transcription text beyond the notes themselves.
  - [x] 1.2 Owner decision 2 (2026-11-21): the whole theme (A, A', B, A') in **G major with the tonic at G5**. Scale degrees 1 to 5 plus the low 5th become G5, A5, B5, C6, D6 and D5; all six are chart fingerings in the profile, the range is 74 to 86 (inside B4 to F6, 71 to 89), and all five buttons are used. See "Melody and range check" below.

- [x] **Task 2: Generator `spikes/gen_trials_1_6.py`** (AC: #1, #3), PEP 723 header, `requires-python >= 3.11` (needs `tomllib`; run with `uv run --python 3.12`), standard library only, MIT, throwaway spike code like `gen_trials_1_3.py` / `gen_trials_1_4.py`
  - [x] 2.1 Read the timing and fingerings from the profile TOML `app/profiles/oot-switch2-1.1.1.toml` (`[buttons]`, `[modifiers]` incl. `stick_up_ly` / `stick_down_ly` / `stick_lx`, `[[fingerings]]`, `[timing]`, `[range]`, `[profile] frame_rate`). Never hard-code a profile value. Do not parse the comments.
  - [x] 2.2 Fingering choice: for every pitch the `chart = true` row (exactly one per pitch). A pitch with no chart row, or outside `[range]`, is an error that names the note (no silent transpose, no octave fold). Out of scope: the cost-based fingering search (Story 5.5).
  - [x] 2.3 Frame grid: tempo as an option `--bpm` (default 90, see "Tempo versus the timing limits"); note onsets are snapped to the frame grid from **cumulative** time (round the onset, take durations as differences), so rounding error does not accumulate. The first press is at `start_lead_ms` (rounded to frames) after the start of the file.
  - [x] 2.4 Transition rules, in this order (implements the "steal time from the previous note, never shift onsets" rule of `docs/score-format-and-mapping.md`, "Timing rules"; a spike simplification, the real compiler is Story 5.5). Let `T` be the onset frame of the next note:
    - same button as the previous note: release the previous note at `T - min_gap_same_button_frames` (also when the pitch differs because only the modifiers differ: U2 is about the button);
    - different button and the D-pad or stick state changes: release the previous note and set the new modifiers at `T - modifier_lead_frames` (a modifier change during a sounding note bends it, U4), press the new button at `T`;
    - different button, same modifiers: new state at `T` releases the old button and presses the new one in the same state (U3: only one note button at a time, and the old button must be released);
    - a rest is neutral from the end of the previous press until the next onset.
  - [x] 2.5 Guards, raising an error with the note index: press shorter than `min_press_frames`; a gap on the same button shorter than `min_gap_same_button_frames`; two note buttons in one state (U3); a note longer than `max_sustain_ms` is re-pressed after `min_gap_same_button_frames` instead of being held (FR-05a; Ode to Joy has none, so this is covered by a unit test only); only L, R, A, X, Y, hat up/down/neutral and the left stick are emitted (reuse or mirror the allow-list of `gen_trials_1_3.py`; AD-9). Do not change `gen_trials_1_3.py`.
  - [x] 2.6 Output: `spikes/samples/story-1-6/ode-to-joy-90bpm.txt` in the `send_2wicc.py` input format (frames with the `f` suffix, one state change per line). Every note gets a label that the analyser parses: `# NOTE 07 B5 83 fingering=Y` (index, name, MIDI, fingering). A header comment lists the profile file, the profile values used and the option values. Deterministic, committed, "regenerate, do not edit by hand". Also a slow variant `ode-to-joy-60bpm.txt` (for listening and as the fallback if the analyser cannot judge the short notes) and a fast stress variant `ode-to-joy-120bpm.txt` (optional run, see Task 7.4).
  - [x] 2.7 U1 trial files (see "U1 trial protocol"): `u1-sustain-A-part{1..N}.txt` and, after Round 1 of the session, `u1-confirm-<L|R|Y|X>.txt` generated with `--u1-confirm-ms <value>`. `TRY` rows in the Story 1.4 style (`attempt`, `hold_ms`, round, shuffled interleaved rounds, a warm-up attempt that is not scored). This generator deliberately holds notes **longer than 6 s**, which `gen_trials_1_3.py` forbids (its `AUTO_STOP_MS` guard); it must have its own bound (hold at most 12 s, only buttons L, R, A, Y, X, no modifiers) and say so in a comment.
  - [x] 2.8 `--key` prints the note list (index, name, MIDI, fingering, onset frame, release frame, frames pressed) as a reading aid, never a result.

- [x] **Task 3: Tests for the generator** (AC: #1, #3) `spikes/test_gen_trials_1_6.py`, no hardware, no real sleeps
  - [x] 3.1 Every committed file under `spikes/samples/story-1-6/` equals what the generator writes (drift test, like Stories 1.3 / 1.4).
  - [x] 3.2 Every file parses with `send_2wicc.parse_lines` (the sender's own parser) and encodes with `encode()` without error; nothing outside the allow-list; no state with two note buttons.
  - [x] 3.3 Timing guards: all presses at least `min_press_frames`, all same-button gaps at least `min_gap_same_button_frames`, modifiers set `modifier_lead_frames` before the press, onsets exactly on the cumulative-rounded grid (a hand-computed small example).
  - [x] 3.4 Pitch check: the pitch the profile gives for each note's (button, dpad, stick) (button pitch + D-pad offset + stick offset) equals the note's pitch, for all notes of the melody. This is the check that the melody and the fingerings agree.
  - [x] 3.5 Errors: pitch out of range, pitch without a chart row, press too short at an absurd `--bpm` (for example 400), note longer than `max_sustain_ms` is re-pressed and the re-press gap is respected, `--bpm` below a sane bound is rejected.
  - [x] 3.6 The profile is read from the real file; if a profile value changes in Part C the tests follow it (no copied constants), except the documented melody facts.

- [x] **Task 4: Analysers in `spikes/listen_trial.py`** (AC: #1, #3) new `--mode melody` and `--mode sustain`; keep `--mode pitch` (default) and all existing tests green (483 tests at Story 1.5); `--backend sounddevice` (lossless) is the one used
  - [x] 4.1 `--mode melody`: reads the `# NOTE` labels of the trial file. Per note: window from the heard onset (scheduled press + `--latency`, default 0.13 s from the Story 1.4 calibration, measured per session with `calibration-A.txt`) plus 40 ms (skips the stick start glide: first 30 ms of a stick note about -29 c, test plan row 2.3) to the next heard onset minus 20 ms (or 0.5 s after the press for the last note). Reports per note: **audible** (tone at the expected pitch at least `--min-snr-db`, 10 dB, above the local noise; the 1.4 lesson: measure at the expected pitch, not broadband), **pitch** (spectral peak searched within +-3 semitones of the expected pitch, so a wrong neighbouring note is found, not forced onto the expected one; nearest note and cents), **verdict** (`ok`, `WRONG PITCH`, `NO SOUND`), **onsets in the window** (an extra onset is an unwanted second note; a repeated pitch needs a dip and re-attack, reuse `dip_and_rise` and `band_onset`; zero onsets for a note that should start is a missed note). Summary: notes, audible, right pitch, missed, extra, cents spread. A run with a PortAudio overflow is invalid (as in 1.4).
  - [x] 4.2 "In tune" has a defined meaning here (owner decision 3): the nearest note is the expected note (the 1.3 `match` rule) **and** the note is within 50 c of the run's own median offset for that pitch class. The game's A5 is about +28 c of equal temperament A4 = 440 Hz (test plan, Story 1.3), so absolute 440 Hz equal temperament is not the reference. The report prints the thresholds used.
  - [x] 4.3 `--mode sustain`: attempts are the `TRY` rows (`hold_ms` in the label). Per attempt: heard onset, then the tone envelope at the expected pitch (reuse `tone_env_db`); **interrupted** = the tone drops to less than `--min-snr-db` above the local noise, or 20 dB below the note's own level in the first second after the attack, for at least 150 ms, at least 100 ms before the scheduled release. Reports per attempt: interrupted yes/no, time of the drop after the heard onset (the auto-stop time, ms), level and pitch (c) in each full second of the hold relative to the first second (the "wavering", numbers only). Per hold duration: interrupted count out of N. A scheduled release that is later than the auto-stop is the point of the long holds, so "interrupted" is not a failure of the run, only of that duration.
  - [x] 4.4 Synthetic-audio tests in `spikes/test_listen_trial.py` (no microphone, no port): a melody with known notes recovers pitch and audible; a missing note is `NO SOUND`; a wrong-pitch note is `WRONG PITCH`; an extra onset is reported; a repeated pitch with and without a dip; a tone that stops at a known time is reported as interrupted with that time (+- 20 ms) and a tone held to the release is not; wavering in level is reported in numbers. Each analyser rule has a test for its trap (1.4 style).

- [x] **Task 5: Docs and README** (AC: #1 to #3)
  - [x] 5.1 `spikes/README.md`: index rows for `gen_trials_1_6.py`, its test, `samples/story-1-6/`, the new `listen_trial.py` modes; a short "Story 1.6" section with the exact commands of Part B (the same style as "Story 1.3 trials").
  - [x] 5.2 `docs/test-plan.md`: add the empty result tables to be filled in Part C (see "Where results go"): new section "Walking skeleton (Story 1.6)" and a row 1.10 "U1 sustain, measured" in the Spike 1 table with the Result cell empty until Part B. Do not write any result before it is measured.
  - [x] 5.3 Tests: `uv run --python 3.12 --with pytest --with pyserial --with numpy pytest spikes/` green (the profile test needs 3.11+, so run the whole folder with 3.12), `uvx ruff check spikes/` clean, `--dry-run` of every new trial file with `send_2wicc.py` prints without error.

### Part B: the Switch session, run by the coordinator with the owner (the dev agent stops before this) [Switch]

- [x] **Task 6: Session setup** (AC: #1, #3) *(done 2026-11-21; wiring check skipped by owner decision, `--selftest-tone` not run, calibration SNR used as the mic check)*
  - [x] 6.1 Hardware safety first (`docs/test-plan.md`, "Hardware safety (before every session)"). The GND-to-GND check is skipped when the wiring is unchanged (owner decision on 2026-11-21, `breadboard wiring`); keep the real controller within reach; start a file only after the ocarina is open and the game has accepted input for 2 s (enter free play with D-pad left on the real controller); dismiss the "press A when the controller is ready" prompt by hand first.
  - [x] 6.2 Environment: do **not** ask the owner for it, take it from `docs/test-plan.md` "Test environment" (Switch 2 23.5.0, OoT 1.1.1, 2wiCC v2.2 + 2wiCC_Comms v1.1, macOS, bridge `/dev/cu.usbmodem101`, handheld). Write the actual values of the session into the results; any changed version invalidates comparisons with Stories 1.3 / 1.4. Microphone: MacBook Pro built-in through PortAudio (`--backend sounddevice`), same place and volume for the whole session, quiet room; run `--selftest-tone` once, then `calibration-A.txt` (Story 1.3) in `--mode detect` or `pitch`: the A5 must have SNR well above 10 dB, and note the onset latency for `--latency`.
  - [x] 6.3 **Quirk: after every Joy-Con connect or disconnect the first injected input is ignored.** Do not detach or reattach Joy-Con inside the session. After any connect: dismiss prompts, run `calibration-A.txt` (the first A is ignored, the second sounds) before the next file. A run whose first note has no sound at all while the rest is fine is re-run, not counted.

- [x] **Task 7: Melody run** (AC: #1) *(done 2026-11-21; optional 120 BPM run not run; 60 BPM runs only on the earlier uncapped, syncopated file, none with the final file; three official 90 BPM runs on the final file)*
  - [ ] 7.1 Dry run, then play `ode-to-joy-60bpm.txt` once by ear (does it sound like the tune?), then `ode-to-joy-90bpm.txt` with `listen_trial.py --mode melody --backend sounddevice` **three times** (separate runs, different phase).
  - [x] 7.2 The owner listens to every run and writes down by ear any wrong, missing, doubled or cut-off note (the AC is about what is heard; the analyser is the second observer, the two are recorded side by side).
  - [x] 7.3 Any wrong or missed note is recorded with its index, fingering and the analyser row; do not round it up. Look first for the known suspects: stick notes (G5 = R + stick up: start glide), same-button repeats (B5 B5 on Y, D6 D6 on X, G5 G5 on R, A5 A5 on A: gap 5 frames, U2; X and R were not run above 5 frames in Story 1.4), Y-to-Y with a D-pad change (B5 to C6), X (D6) at the microphone level. If a wrong note traces to a profile value (for example `min_gap_same_button_frames`, the cost-free chart choice, `modifier_lead_frames`), it is a **finding with a profile change proposal**, to be raised to the owner, not changed silently.
  - [ ] 7.4 Optional stress run, only if time allows and 7.1 passed: `ode-to-joy-120bpm.txt` once (shortest note 15 frames), to see where the limit really is. Not required for the AC.
  - [ ] 7.5 If any run leaves free play, opens a menu or leaves a note stuck: Ctrl+C, take over with the real controller, record what happened, do not continue the file blindly.

- [x] **Task 8: U1 trial** (AC: #3), see "U1 trial protocol" *(round 1 on A done; L, R, Y, X confirmation (8.2) skipped by owner decision 7: assumed equal to A)*
  - [x] 8.1 Round 1, button A: `u1-sustain-A-part*.txt` with `--mode sustain --backend sounddevice` (holds of 4, 5, 5.5, 6, 6.5, 7 s, 5 attempts each, plus 10 s controls).
  - [ ] 8.2 Generate `u1-confirm-*.txt` with the candidate limit from Round 1 and run them on L, R, Y, X (5 attempts each).
  - [x] 8.3 Record per duration the interrupted count out of N, the auto-stop times of the interrupted attempts and the wavering numbers; then apply the profile update rule (Task 9).

### Part C: record the results [after Part B]

- [x] **Task 9: U1 result into the profile** (AC: #3)
  - [x] 9.1 Compute the candidate by the rule in "U1 trial protocol" (owner decision 1). Edit `app/profiles/oot-switch2-1.1.1.toml`: `max_sustain_ms`, the tag (`ESTIMATE` becomes `MARGIN`: "measured X ms, profile Y ms, margin Z"), and the comment above it (row id, how many attempts, buttons covered, what is not covered). **Also when the value stays 5000**: the tag changes from ESTIMATE to what the data supports, and the comment says the estimate was confirmed. Keep the header's tag legend consistent (ESTIMATE stays defined if another value still uses it; none does after this story, so say so or drop the line only if no value uses it: check with a grep).
  - [x] 9.2 If the new value is 6000 ms or more, the bound in `spikes/test_default_profile.py` (`0 < max_sustain_ms < 6000`, derived from the hand-timed 6 s) must be changed to the measured basis; the test must still guard against a value at or above the measured auto-stop. Run the profile test.
  - [x] 9.3 Update the places that quote the value: `docs/score-format-and-mapping.md` (the TOML excerpt comment "estimate: U1 hand-timed ~6 s ... deferred to Story 1.6", line about `max_sustain_ms`), `docs/ocarina-input-mechanics.md` row U1 (measured auto-stop time, pointer to the test-plan row), `docs/test-plan.md` row 1.3 (pointer to row 1.10) and the "Story 1.5" paragraph under Spike 2 ("a short U1 test is deferred ..."), and `_bmad-output/specs/spec-bonooru/roadmap.md` row M2 (the sentence "timings are measured except `max_sustain_ms` (estimate) and `start_lead_ms`" becomes "except `start_lead_ms`" once U1 is measured). Do not touch `~/Documents/org/`.

- [x] **Task 10: Melody result into `docs/test-plan.md`** (AC: #2): the table of "Where results go"; the environment of the session; every wrong or missed note; the sentence that this is the 2wiCC queue and the PC host clock, not our firmware scheduler (Epic 2, Epic 7 measure that); a "Lessons" line for anything the analysers got wrong. Set the story to `review` only when every AC is true on recorded data. *(results written 2026-11-21; story status and AC verdicts not set by the recorder)*

## Dev Notes

### What this story is

The walking skeleton of risk R4: the first time a melody goes from a note list, through the measured profile, through the spike sender and 2wiCC, to sound in OoT free play. It proves the chain (profile -> fingerings -> frame grid -> sender -> console -> ear) before the app, the firmware and the real scheduler exist. It is **not** a design for the pipeline of Epic 5: the generator is throwaway spike code. Nothing here touches `firmware/`, `app/` Rust code or the link protocol.

The Switch is needed for Part B only. The dev agent does Part A (code, tests, the committed trial files, the empty doc tables); the coordinator runs Part B with the owner; Part C writes the numbers.

### Melody and range check

Owner decision 2 (2026-11-21): Ode to Joy (Beethoven), G major, tonic G5. The theme uses scale degrees 1 to 5 plus the low 5th, so it spans 12 semitones. It fits B4 to F6 (71 to 89) for any tonic from E5 (76) to A#5 (82); checked against the chart fingerings in the profile (tonics E5, F5, F#5, G5, G#5, A5 and A#5 pass, all others fail on range or on a missing chart row). G5 was chosen because it uses all five buttons:

| Degree | Note (sounding) | MIDI | Chart fingering (profile) |
|---|---|---:|---|
| low 5 | D5 | 74 | L |
| 1 | G5 | 79 | R + stick up |
| 2 | A5 | 81 | A |
| 3 | B5 | 83 | Y |
| 4 | C6 | 84 | Y + D-pad up |
| 5 | D6 | 86 | X |

The F major alternative (tonic F5: C5 L + stick down, F5 R, G5 R + stick up, A5 A, A#5 A + D-pad up, C6 Y + D-pad up) has no X and no natural-only notes but exercises the stick down. The C major version at C5 (72 to 79) cannot play the low 5th (67, below B4), so it can only play the A and A' phrases.

Draft of the theme in the chosen key (**verify against a public-domain edition; the transcription is the dev agent's job**; durations in beats, `.` = dotted):

- A: B B C D | D C B A | G G A B | B. A A(2)
- A': B B C D | D C B A | G G A B | A. G G(2)
- B: A A B G | A B(.5) C(.5) B G | A B(.5) C(.5) B A | G A D5(2)
- A' again.

About 62 notes. The shortest notes are the paired eighths (B to C in phrase B, and the eighth after a dotted quarter).

### Tempo versus the timing limits

Profile values (frames at 60 Hz, 16.67 ms): `min_press_frames = 3`, `min_gap_same_button_frames = 5`, `modifier_lead_frames = 1`, `max_sustain_ms = 5000`, `start_lead_ms = 500`. Computed with the cumulative-rounding rule, beat = 3600 / BPM frames:

| BPM | Beat (frames) | Eighth (frames) | Eighth followed by the same button: press = eighth - 5 | Longest note (half, frames / s) |
|---:|---:|---:|---:|---:|
| 60 | 60 | 30 | 25 | 120 / 2.0 |
| 90 (default) | 40 | 20 | 15 | 80 / 1.33 |
| 120 (stress) | 30 | 15 | 10 | 60 / 1.0 |
| 225 (theoretical limit) | 16 | 8 | 3 | 32 / 0.53 |

(An eighth is half a beat; a dotted quarter plus an eighth is two beats.) The hard floor is: shortest note >= `min_press_frames` + `min_gap_same_button_frames` = 8 frames when the next note is on the same button, so beat >= 16 frames, 225 BPM. The default 90 BPM keeps the shortest press at 15 frames (250 ms), far from the 3-frame floor, so a failure at 90 BPM is a bug or a wrong profile value, not a tempo problem. Every note is far below `max_sustain_ms`, so the re-press rule is not exercised by the melody. (If the dev agent finds the arithmetic above wrong when implementing, the generator's computed numbers win; fix this table.)

### How the notes are evaluated ("every note audible and in tune")

Same method family as Story 1.4 (`docs/test-plan.md`, "Story 1.4 field trials" > "Environment and capture"; `1-4-timing-limits.md` "Measurement method"): the Mac microphone near the console speaker, **lossless** capture through PortAudio (`--backend sounddevice`, the ffmpeg capture loses about 11 % of the samples), analysis at the expected pitch (the broadband SNR and generic YIN were the 1.4 artefacts: room noise "sounded", X read as f/2), controls and a calibration at the start. What is different from Story 1.3's `--mode pitch`: that mode analyses from press + 0.3 s to release - 0.2 s, which does not exist for notes of 250 to 700 ms, so melody notes need their own windows (Task 4.1). A note is judged by three observers in this order: the analyser (audible, pitch, onsets), the owner's ear (the AC is about what is heard), and optionally the phone tuner on a held note if the two disagree. All three are recorded side by side in the results; a disagreement is a finding.

### U1 trial protocol

Question (U1, test plan row 1.3, `docs/ocarina-input-mechanics.md`): the game stops a held note by itself at about 6 s (hand-timed, no tool), with wavering from about 2 s. The profile's `max_sustain_ms = 5000` is an ESTIMATE with a margin of about 1 s. Notes longer than it are re-pressed after the min gap (FR-05a).

Design:

- **What is measured**: for a button held for D seconds, does the tone stay audible until the scheduled release (D), and when does it stop by itself if it does not. Observable: the tone at the expected pitch of the button (A5 for A), from the microphone, onset and offset.
- **Durations**: 4, 5, 5.5, 6, 6.5 and 7 s (the owner's list from Story 1.5), plus 10 s controls (3 to 5, they give the auto-stop time directly because the note ends well before the release; this also settles whether the stop depends on time since the press). Release after each hold, then 3 s of neutral before the next attempt.
- **Repetitions**: 5 attempts per duration on **A** in Round 1 (6 x 5 + 5 controls = 35 attempts, about 6 min of playback), in interleaved, shuffled rounds (phase diversity as in Story 1.4), with one unscored warm-up attempt per file (absorbs the first-input-after-connect quirk). One file per about 10 attempts so a file stays under a few minutes and the owner can watch it.
- **Confirmation on L, R, Y, X** (as Story 1.4 did for U7 and U2): 5 attempts each at the **candidate limit** and at the next longer tested duration (about 2 min per button). If a button interrupts earlier than A, the earlier value is the profile value and the difference is a finding.
- **Criterion "note interrupted"** (owner decision 1): the tone at the expected pitch is less than 10 dB above the local noise, or 20 dB below the note's own level in the first second after the attack, for at least 150 ms, at least 100 ms before the scheduled release. The time of the drop after the heard onset is the **auto-stop time**. Wavering (level and pitch per second, relative to the first second) is reported as numbers, not as a pass/fail, because a note that wavers but sounds is still a note; the owner also says by ear from which second it sounds bad (recorded as an observation).
- **Controls**: blank attempts (no press) must not read as a tone (the false-positive level); a 1 s hold must read as not interrupted.
- **Update rule for `max_sustain_ms`** (owner decision 1): let `D_ok` be the largest tested duration where every attempt on every tested button was uninterrupted, and `T_stop` the shortest measured auto-stop time among all interrupted attempts. Candidate = `floor100(min(D_ok, T_stop) - 500 ms)`. Margin 500 ms (30 frames) replaces the former 1 s hand-timing margin because the data now has an error bar. If the candidate is within +-100 ms of 5000, keep 5000 (no churn) and change only the tag and comment. Never raise the value above what a measured all-pass duration supports, never lower it on a single stray attempt without recording the pattern (a partial result such as 4/5 is a finding with its pattern: random, or tied to the round or position, which would point to the phase; do not round it up). The profile tag becomes `MARGIN`, with "measured X, profile Y, margin Z" and the row id.
- **Where it goes**: see "Where results go". The auto-stop time and the wavering numbers also go into `docs/ocarina-input-mechanics.md` U1.

### Known facts to build on (do not re-measure)

- The first injected input after a controller connect is ignored (Story 1.4; seen again in Story 1.4 calibration). Plan the first A of every session as a sacrifice.
- A5 plays about +28 c sharp of equal temperament A4 = 440 Hz; compare pitches with the run's own median, not with 880 Hz (test plan, Story 1.3).
- A 1-frame press sounds about 0.1 to 0.2 s; the sounded length is not proportional to the press (U7). A new button cuts the held one; the old one must be released (U3).
- D-pad steps at once, the stick glides (U4); a stick note starts about -29 c in its first 30 ms (row 2.3). On X the D-pad shifts about +-115 c, not 100 c (row 1.9): the melody's X note (D6) has no D-pad, so this is not exercised.
- Heard latency about 123 to 139 ms; the onset jitter through 2wiCC was 0.48 to 0.70 frame sigma (row 2.4). The result here is the 2wiCC queue plus the PC host clock, **not** our firmware scheduler.
- Free play is fragile: ZL opens the song menu, B and D-pad left exit (U8, U9, U11). The generator is the only guard because the spike sender has no AD-9 mask.

### Known issue: about every 7th note is cut (found in Part B, 2026-11-21)

With full-length (legato) presses, the game ends a held note by itself 0.54 to 0.91 s after its onset, about every 7th sounded note (07, 15, 23, ... in the 60 BPM runs; 07 and 14 even with 17 identical A5 notes, harmless there), and ignores input for up to about 1 s: a note that follows by hand-over or within that time is lost (08, 16, ...). The audio is truly silent (not an analyser artefact). No evidence for a sender or 2wiCC queue cause (later notes stay on schedule; 0 firmware lines in one diagnostic run); the dev agent's offline reading of the OoT decompilation (not verified on hardware) found no song recognition or input lock to explain it. Hypothesis, mechanism unknown. With presses of at most 400 ms there was no audible event in 1 full 60 BPM run and 1 short run, and the 3 official 90 BPM runs were 62/62. Workaround: owner decision 5 below (`DEFAULT_MAX_PRESS_MS = 400` in `spikes/gen_trials_1_6.py`, spike-level, not a profile value). Evidence: `docs/test-plan.md`, "Walking skeleton (Story 1.6)" (filled by the session recorder), and the Part B session notes below. Effect on U1: each attempt is one note, so roughly every 7th attempt (warm-up included) may be ended about 0.8 s in; check short auto-stop times against the attempt position before using them.

Follow-ups (not in this story):
- Real pipeline (Epic 5, Story 5.5 compiler): re-press long notes (split into presses of at most about 400 ms with the min gap) instead of the staccato cap, so durations stay audible (owner's preferred direction, 2026-11-21); confirm on the console.
- Whether a press cap (for example a `max_hold_ms` key) belongs in the profile TOML: open owner call (Part C or later), with its own test-plan row.
- Find the cause of the event (game or emulator); possible double attacks on X (D6) re-presses seen in every run (U2 on X unresolved since Story 1.4).
- Sustain summary: define D_ok as the largest hold below the smallest failing hold, scored holds only (today: the largest hold whose attempts all held, controls included).
- Sustain summary: the short-control INVALID branch is nearly dead code (a short control almost never reads as interrupted); make the control check explicit or drop it.
- Melody summary: a WRONG PITCH note is also listed under "missed" (it has no attack at the expected pitch); list it only once.
- Generator CLI: `--notes` and `--extra-lead-ms` are silently ignored for `--set all|u1|confirm`; refuse them there.

### Review findings (2026-11-21)

- First review (owner approved): wording and evidence fixes, all done. README "Known issue" reworded as a hypothesis (no evidence for a sender/queue cause; the cap result stated per run); `docs/test-plan.md` decompilation and queue-underrun exclusions marked as the dev agent's offline reading, not verified on hardware; Calibration and controls row for the first-input quirk; 60 BPM run 2 row note on the old "right pitch" counting (checked against the run-2 CSV: 52 ok, 6 CUT SHORT, 4 NO SOUND, pitch right on all 58 audible notes); 60/120 BPM files marked as not run in their final form.
- Second review (line by line, no blocking defects; owner approved): fixed: melody comments now start only at a token (`C#6 1` parses; test); a modifier set before the start of the file is refused (test); `split_sustain` guards against a press length under one frame / `max_sustain_ms` under `min_press_frames` (tests); `serial_rx_log.py` reports a port error as `PORT ERROR ...` with the normal end summary and exits 2, and closes the port if the log file cannot be created (tests). Deferred: the four follow-ups above (D_ok definition, short-control INVALID branch, WRONG PITCH listed as missed, `--notes`/`--extra-lead-ms` ignored outside `--set melody`).

### Developer guardrails

- English everywhere in the repo; chat with the owner is Italian.
- Read the values from the profile TOML; never copy a value into code, tests or the trial files' meaning (a header comment may show them). Pitches are sounding pitches.
- Do not change `send_2wicc.py` (`encode`, `schedule`, `Pacer`, the stop sequence), `gen_trials_1_3.py`, `gen_trials_1_4.py` or their tests. `listen_trial.py` changes only add modes; `--mode pitch` stays the default and all existing tests stay green.
- Allow-list only (AD-9): L, R, A, X, Y, hat up/down/neutral, left stick. No ZL, no ZR, no right stick, no B. States use the canonical encoding (AD-10: `ly 0x00` = up, hat 8 neutral); only the sender's `encode()` knows the 2wiCC layout.
- Python only; standard library plus `pyserial`, `numpy` and (optional) `sounddevice`. The generator needs Python 3.11+ for `tomllib`: use `uv run --python 3.12`, and put that in the file header and the README.
- **No UF2 or 2wiCC binaries, no game screenshots, no recorded audio or video in the repo** (`spikes/recordings/` is git-ignored: WAV, CSV, Markdown reports and any video stay there), never reference anything under `ux-designs/*/imports/`. Do not touch `_bmad/`, `bmad-*` or `gds-*` skill folders. Do not modify `~/Documents/org/`.
- The dev agent never opens a serial port and never runs the sender without `--dry-run` in Part A.
- Never claim more than the data proves: "20/20" or "5/5" only for what was run; a button not run is named as not run.

### Architecture compliance

- **AD-9** allow-list and **AD-10** canonical state as above. The firmware mask is not touched; the spike has none, so the generator guards.
- **AD-15**: the real timeline producer is the pipeline (Epic 5). The generator mimics three of its rules (frame grid, min press / min gap, modifier lead) and the "steal time from the previous note, never shift onsets" principle, only to play one melody. It is not a precursor of the code and is not to be reused: Story 5.5 writes the real one with golden tests. The AD-15 minimum is `2 x max(report_period_us, frame time)`; the profile's 3 and 5 frames are above the frame-time part (2 frames).
- `docs/test-plan.md` is the single place for results; `docs/ocarina-input-mechanics.md` holds the measured facts (U1); the profile holds the value.

### Files to touch

| File | Action | Notes |
|---|---|---|
| `spikes/gen_trials_1_6.py` | NEW | generator (melody files, U1 files), reads the profile |
| `spikes/test_gen_trials_1_6.py` | NEW | tests, no hardware |
| `spikes/samples/story-1-6/ode-to-joy.melody` | NEW | the note list (source) |
| `spikes/samples/story-1-6/*.txt` | NEW | generated, committed: `ode-to-joy-{60,90,120}bpm`, `u1-sustain-A-part*`, later `u1-confirm-*` |
| `spikes/listen_trial.py` | UPDATE | `--mode melody`, `--mode sustain` |
| `spikes/test_listen_trial.py` | UPDATE | synthetic-audio tests; the whole existing suite stays green |
| `spikes/README.md` | UPDATE | index, "Story 1.6" section with the Part B commands |
| `docs/test-plan.md` | UPDATE | A: empty tables; C: results; row 1.3 pointer, row 1.10 |
| `docs/ocarina-input-mechanics.md` | UPDATE (Part C) | U1 answer |
| `docs/score-format-and-mapping.md` | UPDATE (Part C) | the `max_sustain_ms` comment in the TOML excerpt |
| `app/profiles/oot-switch2-1.1.1.toml` | UPDATE (Part C) | `max_sustain_ms`, tag, comment; nothing else |
| `spikes/test_default_profile.py` | UPDATE (Part C, only if the value reaches 6000) | the `< 6000` bound |
| `_bmad-output/specs/spec-bonooru/roadmap.md` | UPDATE (Part C) | M2 row wording |

Everything else stays untouched, in particular `spikes/send_2wicc.py`, `spikes/app-egui/`, `firmware/`.

### Where results go

| What | Where | Form |
|---|---|---|
| Environment of the session | `docs/test-plan.md` "Walking skeleton (Story 1.6)" | the real values (system, OoT, firmware, host, port, mic position, handheld/docked), taken from "Test environment" and checked |
| Melody run, per run | same section | table: run id, BPM, notes, audible, right pitch, missed, extra, cents spread, invalid? ; owner's ear column next to it |
| Wrong or missed notes | same section | index, note, fingering, analyser row, owner's remark, suspected cause, finding/profile proposal |
| Calibration and controls | same section | A5 Hz and SNR, latency used, blank/1 s controls |
| U1 per duration | `docs/test-plan.md` row 1.10 + "U1 sustain, measured" table | duration, attempts, interrupted count, auto-stop times, wavering numbers, buttons covered |
| U1 answer | `docs/ocarina-input-mechanics.md` U1, and row 1.3 pointer | the measured auto-stop time and the profile value |
| New `max_sustain_ms` | profile (value, tag, comment) | rule in "U1 trial protocol" |
| Session recordings, reports | `spikes/recordings/` | git-ignored, never committed |

### Testing requirements

- Part A: all tests green without a serial port, microphone, ffmpeg or console, no real sleeps: `uv run --python 3.12 --with pytest --with pyserial --with numpy pytest spikes/`, `uvx ruff check spikes/`. New generator and analyser rules each have a test; the drift test guards the committed files.
- Part B results are measurements, not tests. The automated guard is that the files sent are what the generator promised and that the analysers recover known values from synthetic audio.

### Previous story intelligence (Stories 1.3 to 1.5, done)

- Story 1.4 (timing): the analyser definitions of the U7 1-frame counts, U2 at 2 to 3 frames and the jitter sigma were revised after the recordings; each artefact fix has a synthetic test. Reuse the fixed analysers (tone-band SNR at the expected pitch, spectral peaks, per-button envelopes), do not reintroduce broadband SNR or plain YIN. Follow-ups dropped by the owner (X with the mic closer, another R gap-0 run): X is the most likely microphone problem here (D6 is quieter at the mic: 9 to 11 dB broadband, 14 to 23 dB at the pitch).
- Story 1.5 (profile): every value has a tag; honesty rule: ESTIMATE, OWNER CHOICE, MARGIN, DESIGN DEFAULT, MEASURED; `max_sustain_ms` was left ESTIMATE only because this story measures it. `min_gap_same_button_frames = 5` is an owner choice with R and X not run above 5 frames, so the same-button repeats of this melody (Y, X, R and A all repeat a note; L does not) are the real test of that value, and X is the weakest case (U2 on X was unresolved).
- Owner preferences (memory): chat in Italian, repo in English; decisions asked one at a time and short; research before asking; do not re-check the breadboard wiring each session; game screenshots never on GitHub.

### Git intelligence

Recent commits: `ea8d5a7` merge of Story 1.5 (`app/profiles/oot-switch2-1.1.1.toml`, `spikes/test_default_profile.py`), `3c872c5` Story 1.4 (generator and analysers in `spikes/`, results in `docs/`), `7097e94` Story 1.2. Pattern: one branch per story (this one is `story/1-6-walking-skeleton`), generators write committed deterministic files with a drift test, results in `docs/test-plan.md`, Python spike tests next to the scripts, no CI for Python yet.

### Latest tech information

No new library. `tomllib` is in the standard library from Python 3.11 (hence `uv run --python 3.12`, as for `test_default_profile.py`). `sounddevice` and `numpy` come with the PEP 723 header of `listen_trial.py` as in Story 1.4. Upstream firmware behaviour (2wiCC v2.2: 60 Hz queue, 1023 usable slots, `+SPM BUF` / `+SPM RT` stop sequence) is documented in `spikes/README.md`; the sender already paces the queue, a 60 s melody is far below any limit.

### Project Context Rules

- No `project-context.md` exists in the repo.
- Repo content (files, comments, commit text) is English; chat with the owner is Italian.
- Never commit `_bmad/`, `bmad-*` or `gds-*` skills; only `_bmad-output/`.
- Never commit or reference anything under `ux-designs/*/imports/`, game screenshots, recorded audio or video, UF2 or other binaries.
- Do not edit `~/Documents/org/`. Do not commit or push unless the owner asks.
- Ask decisions one at a time and keep them short; search online or upstream before raising an open question.

### Project Structure Notes

- All new code is under `spikes/` (throwaway), results under `docs/`. No Rust, no `app/` code, no firmware. `spikes/samples/story-1-6/` follows `story-1-3/` and `story-1-4/`.
- A Python 3.11+ requirement for the generator is a variance from the 3.10 of the older generators, on purpose (reads the TOML profile directly instead of copying it).

### Owner decisions (2026-11-21, replace the former open questions)

The owner accepted all four recommendations; the tasks above apply them.

1. **U1 criterion and update rule**: "interrupted" = tone under 10 dB SNR or 20 dB below its own level for 150 ms, at least 100 ms before the release; candidate = `floor100(min(D_ok, T_stop) - 500 ms)` with a **500 ms margin** (not 1000 ms); keep 5000 if the candidate is within +-100 ms of it; tag ESTIMATE becomes MARGIN. 5 attempts per duration on A, 5 per button at the candidate on L, R, Y, X.
2. **Melody**: the full Ode to Joy theme (A, A', B, A') in G major at G5, about 62 notes.
3. **In tune**: the nearest note equals the expected note and the note is within +-50 c of the run's own per-pitch-class median; the owner's ear is recorded alongside.
4. **Tempo**: 90 BPM main run (3 runs with the analyser), 60 BPM by ear first, 120 BPM optional stress run.
5. **Press cap (2026-11-21, after the Part B diagnosis)**: (a) staccato with a 400 ms press cap is the spike default for the Story 1.6 melody files (60/90/120 BPM), a workaround for the every-~7th-note cut (see "Known issue"); the U1 files keep their long holds; not added to the profile now. (b) Re-pressing long notes is the preferred direction for the real pipeline later (follow-up, not implemented now).
6. **Rhythm of bar 12 (2026-11-21, after the 90 BPM run 1)**: the simplified common arrangement, `G5 1, A5 1, D5 2 | B5 1, B5 1, C6 1, D6 1` (two B attacks on beats 1 and 2 of bar 13), instead of Beethoven's syncopated entry (D5 quarter, then a B5 on beat 4 tied over the bar line), which under the 400 ms cap sounded like a missing note. Same 62 notes and 64 beats; the original is noted in the melody file header. Owner's check from a scratch copy at 90 BPM: 62/62 audible, "now it sounds as I remember it".
7. **U1 confirmation skipped (2026-11-21, after U1 round 1 on A)**: the confirmation on L, R, Y, X (Task 8.2) is not run; all buttons are assumed equal to A. `max_sustain_ms` is therefore measured on A only (D_ok 6500 ms, T_stop 6599 ms, candidate 6000 ms by decision 1), and the profile comment says the other buttons are an assumption.

## Dev Agent Record

### Agent Model Used

Claude Opus 5.5 (claude-opus-5-5), gds-dev-story, Part A only.

### Debug Log References

- `uv run --python 3.12 --with pytest --with pyserial --with numpy pytest spikes/`: 570 passed, 1 skipped (the skip is the `u1-confirm-*` drift test, which has no file until the session). 483 at Story 1.5.
- `uvx ruff check spikes/`: all checks passed.
- `uv run spikes/send_2wicc.py --dry-run` on all 8 new trial files: no error, no zero-frame row.
- First synthetic run of `--mode melody` with a missing note after a hand-over (A5 on A, then B5 on Y ignored) reported `WRONG PITCH A5` (the decaying tail of the previous note); fixed: another pitch counts only if it is still there in the second half of the window; trap test added.

### Completion Notes List

**Part A done (2026-11-21). Part B (Switch session) done 2026-11-21 (session over); results written into `docs/test-plan.md`; `max_sustain_ms = 6000` set by the dev agent (Task 9). The Part A text below was written before the session.**

- Task 1: `ode-to-joy.melody`, 62 notes, 64 beats, G major at G5, D5 to D6. Verified against Beethoven's theme: the draft in Dev Notes ended phrase B with `G A D5(2)`; Beethoven's score has the syncopated entry (bar 12: G A D5 on beats 1 to 3, then the B5 of the last phrase on beat 4 tied over the bar line). The file follows the score: `... G5 1, A5 1, D5 1, B5 2, B5 1, C6 1, D6 1 ...` (same 62 notes, same 64 beats). Checked against the G major excerpt of the English Wikipedia article "Ode to Joy" (it keeps the syncopation); the public-domain edition named in the header (Breitkopf & Haertel, IMSLP) was not checked visually (PDF score images). Only the notes are taken.
- Task 2: `gen_trials_1_6.py` reads every value from the profile TOML (no constants copied); chart fingering only; onsets from cumulative beats with exact fractions (round half up); transition rules as 2.4 (one rule: the previous press ends at min(next item onset, the next note's modifier frame, next onset - min gap if same button)); rests are neutral and reset the modifiers; FR-05a re-press split into the fewest presses of at most `max_sustain_ms`; guards with the note index; an independent `check_rows` re-checks every rule on the final rows. Computed numbers match the "Tempo versus the timing limits" table (shortest press 25 / 15 / 10 frames at 60 / 90 / 120 BPM). `--set all|melody|u1|confirm`, `--bpm`, `--key`.
- Task 2.7 (U1): each round also holds one blank and one 1 s control (the "Controls" of the U1 protocol), so round 1 is 5 x (6 holds + 10 s + 1 s + blank) = 45 attempts in 5 files of 80.2 s (one round per file, about 10 attempts each with the warm-up), about 6.7 min. Confirmation: candidate and the next longer tested hold (candidate + 500 ms beyond 7 s), 5 each, per button.
- Task 3: 50 generator tests (drift incl. later `u1-confirm-*` files via their header, sender parser and encoder, allow-list, U3, U4, timing rules from the profile, cumulative grid with a hand-computed example, pitch check of every note against the profile model and chart, all errors of 3.5, U1 design and guards).
- Task 4: `--mode melody` and `--mode sustain` added; `--mode pitch` stays the default and the existing tests are unchanged and green. `--latency` defaults to 0.13 s for these two modes only (0 for the others, as before). Levels for "audible" are per second of window (`band_mean_db`), so windows of 0.2 to 1.3 s compare with the 0.4 s lead-in. Sustain reports and `--combine` print per (button, hold) the interrupted count, the auto-stop times, D_ok, T_stop and the rule's candidate for the rows given. 22 synthetic tests (all notes ok at 60/90/120 BPM with the +27 c game tuning, missing note, missing note after a hand-over, wrong neighbour, nearest-note-right but out of tune, extra onset, repeated pitch with and without a dip, stick glide skipped, quiet X, invalid run, auto-stop time within 20 ms, held notes, wavering numbers, 12 dB sag / 100 ms dropout / drop at the release not interrupted, sounding blank invalidates, combine and candidate, latency defaults).
- Task 5: README index and "Story 1.6 trials" section with the Part B commands; `docs/test-plan.md` row 1.10 (Result empty) and the section "Walking skeleton (Story 1.6)" with empty tables.

### Part B session notes (2026-11-21)

- Setup: controller connected; bridge probes on `/dev/cu.usbmodem101`: `+ID ` -> `+2wiCC`, `+VER ` -> `+VER 2.2`, `+GQS ` -> `+GQS 0400`. Wiring check skipped (breadboard unchanged, owner decision). Console already in ocarina free play.
- After connect: L+R sent, then A sent (real-time mode). Owner reported the controller needed another A to connect (first-input-after-connect quirk again): the first L+R + A did not connect; a second A was sent, then 2 s wait.
- Mic self-test (`--selftest-tone`) not run this session; the calibration SNR is used as the mic check.
- Calibration `calibration-A.txt` (default pitch mode, MacBook Pro Microphone, sounddevice): A5 893.4 Hz, +26 c, -28.5 dBFS, SNR 19 dB, onset 120 ms after the scheduled press, verdict match; sender exit 0, 253 frames. Report `spikes/recordings/calibration-A-20261121-184633.{csv,md}` (git-ignored). Latency 0.12 s will be used for melody/sustain analysis.

- Melody 60 BPM run, `ode-to-joy-60bpm.txt`, analysed with `--mode melody --latency 0.12`: sender exit 0, 3931 frames, 65.517 s; recorded 66.9 s; offset 0.369 s. Notes 62, audible 56/62, right pitch 56/62, missed 6, extra onsets 18, cents spread -3 to +3 c (sd 0.7 c) vs pitch-class medians A +29, B +24, C +26, D +23, G +3 (c vs A4=440). Every audible note was the expected pitch. Missed (NO SOUND): 08 A5 A (t 7.5 s), 16 B5 Y (16.5 s), 24 G5 R+stick-up (24.5 s), 39 G5 R+stick-up (39.5 s), 40 A5 A (40.5 s), 41 B5 Y (41.5 s). Extra onsets: 08 A5 (3), 15 A5 (4), 33 B5 (3), 38 B5 (7), 40 A5 (5), 43 B5 (2). Report `spikes/recordings/ode-to-joy-60bpm-20261121-184708.{csv,md}` (git-ignored). Owner's ear: "generally good, some notes were missing" (translated from Italian; the owner did not say which notes, not mapped onto the six indexes). Cause not interpreted.

- 60 BPM offline diagnosis (dev agent), a hypothesis not a conclusion: all 6 misses are real; held notes stopped by themselves about 0.55 to 1.45 s after onset (stops 07 B5 7.10 s, 15 A5 15.05 s, 23 A5 24.05 s, 30 G5 31.95 s, 38 B5 39.05 s, 48 B5 48.35 s), then silence about 1 to 3 s with presses lost inside it (08; 16; 24; 39, 40, 41; none after 30 and 48), playback resuming on schedule; 6 notes cut short were marked `ok` by the analyser. Unknown loud sounds in the recording at 39.1 to 39.8 s and 41.0 to 41.5 s. Ruled out: analyser windows/thresholds, OoT song recognition, sender underrun. Working hypothesis (unproven): controller link drops out about 1 to 3 s roughly every 7 to 9 s. Analyser fix `count_reattacks`: extra onsets 18 -> 2 on re-analysis. Next: re-run 60 BPM with the owner watching the screen and the Story 1.4 control `jitter-120bpm-16ths.txt`, before the 90 BPM runs and U1.

- Melody 60 BPM run 2 (owner watching the screen), `ode-to-joy-60bpm.txt`, updated analyser, `--mode melody --latency 0.12`: sender exit 0, 3931 frames, 65.517 s; recorded 66.9 s; offset 0.371 s. Notes 62, audible 58/62, right pitch 52/62, missed 4, extra onsets 1, cents spread -2 to +1 c (sd 0.6 c); the summary line printed no "cut N" field, the per-note table shows 6 CUT SHORT. Missed (NO SOUND): 08 A5 A, 16 B5 Y, 47 D5 L, 48 B5 Y. CUT SHORT: 07 B5 Y, 15 A5 A, 30 G5 R+stick-up, 38 B5 Y, 46 A5 A, 62 G5 R+stick-up. Extra onsets: 15 A5 (2). Report `spikes/recordings/ode-to-joy-60bpm-20261121-185906.{csv,md}` (git-ignored). Owner: "nothing appears on screen during the silent notes, as if the button were not pressed" (translated from Italian). Same in both 60 BPM runs: 07 cut + 08 missed, 15 cut + 16 missed. Run 1 re-analysed with the new analyser: missed 6, cut 6 (07 670/1000 ms, 15 730/2000, 23 610/983, 30 1720/1983, 48 1070/1917, 62 825/2000), extra onsets 2 (38 B5: 3); note 38 was cut in run 1 by manual inspection (-16 dB at 39.03 s) but not flagged because the loud non-ocarina sound masks the B5 band.

- Melody 60 BPM run 3, `ode-to-joy-60bpm.txt`, `send_2wicc.py` only (not recorded, not analysed), to check whether recording / `listen_trial.py` was involved. Owner's ear: "no, notes missing anyway" (translated from Italian); which notes not said. Context, not a result: the owner remarked that informal non-test melodies played earlier in the session with the same generator and sender (not recorded, not part of the story) sounded fine by ear.

- Diagnostic run 4 (not one of the story's melody runs): notes 01 to 16 of the 60 BPM melody shifted 4 s later (`spikes/recordings/diag/ode-to-joy-60bpm-n01-16-lead4000ms.txt`, `gen_trials_1_6.py --set melody --bpm 60 --notes 1-16 --extra-lead-ms 4000`, 22.5 s, git-ignored). Read-only `spikes/serial_rx_log.py` during the trial: 0 lines received (19:10:21 to 19:10:54; `spikes/recordings/rx-20261121-191021.log`). Analyser (`--mode melody --latency 0.12`, offset 0.371): notes 16, audible 15/16, right pitch 15/16, missed 1, cut 1, extra onsets 1, cents spread -2 to +2 c (sd 0.8 c). Missed: 08 A5 A (NO SOUND, scheduled 11.5 s). Cut short: 07 B5 Y, 885 of 1000 ms (scheduled 10.5 s). Extra onsets: 05 D6 (2). Notes 15/16 played fine. Report `spikes/recordings/ode-to-joy-60bpm-n01-16-lead4000ms-20261121-191026.{csv,md}` (git-ignored). Owner did not comment on this run. Factual reading: the 07-cut / 08-missed event moved with the notes (about 7.3 s file time before, about 11.3 s now), not with elapsed time; no firmware messages received. Context: the owner listened to an informal non-test ragtime (fast, short notes), said it sounded fine, and believes the problem lies in the Ode to Joy file.

- Diagnostic run 5 (bisect, not one of the story's melody runs): `spikes/recordings/diag/ode-to-joy-60bpm-n06-08-lead2000ms.txt` (notes 06 C6, 07 B5, 08 A5, 2 s lead, 6.5 s; `gen_trials_1_6.py --set melody --bpm 60 --notes 6-8 --extra-lead-ms 2000`), played 3 times with `--mode melody --latency 0.12`. All three: notes 3, audible 3/3, right pitch 3/3, missed 0, cut 0, extra onsets 0. Reports `spikes/recordings/ode-to-joy-60bpm-n06-08-lead2000ms-20261121-191541`, `-191550`, `-191600` (`.csv`/`.md`, git-ignored). Factual reading: notes 06 to 08 alone do not reproduce the 07-cut / 08-missed event. Owner did not comment. Dev agent, offline: `serial_rx_log.py` exit code 1 on Ctrl+C was unintended and is fixed; the run 4 log was complete (0 lines). New diagnostic generator option `--max-press-ms N`.

- Diagnostic run 6 (not one of the story's melody runs): 17 identical A5 notes (A only, about 55 frames, 5-frame same-button gaps), 60 BPM, 2 s lead, 20.5 s; `spikes/recordings/diag/a5x17-60bpm-lead2000ms.txt` (`a5x17.melody`; `gen_trials_1_6.py --set melody --bpm 60 --extra-lead-ms 2000 --melody spikes/recordings/diag/a5x17.melody --out-dir spikes/recordings/diag`). Analyser (`--mode melody --latency 0.12`): notes 17, audible 17/17, right pitch 17/17, missed 0, cut 0, extra onsets 0, cents spread -2 to +0 c, sd 0.5 c. Report `spikes/recordings/a5x17-60bpm-lead2000ms-20261121-191849.{csv,md}` (git-ignored). Factual reading: a run of 17 long identical notes does not reproduce the event. Owner: "there is no staff on screen to look at" (translated from Italian; no note staff is displayed in this free-play setup). Dev agent, offline: an every-8-notes rule fits the data only loosely; the OoT decompilation shows no input lock or note cut tied to the 8-note free-play buffer. Owner's ear on this run: "anyway, between the 7th and the 8th note I heard a different pause" (translated from Italian); the analyser did not flag it. Gap measurement (dev agent, offline; offset 0.368 s, 5 ms window, gap = level more than 10 dB below the note's level): 15 normal pairs: tone falls 1071 to 1124 ms after the press, gap 4 to 51 ms (median about 31 ms). 07 -> 08: tone falls at 931 ms, gap 176 ms (deepest -22.9 dB). 14 -> 15: tone falls at 922 ms, gap 183 ms (-22.2 dB). Earliest audible release about 1040 ms (55-frame press plus about 120 to 130 ms latency), so notes 07 and 14 stopped before the key release. Onset intervals all 988 to 1013 ms (07 -> 08: 1001 ms); nothing was lost. The owner's remark is consistent with the 07 -> 08 gap; the analyser's cut-short rule did not flag it because it only checks up to 100 ms before the scheduled release. Dev agent's reading (hypothesis, mechanism unconfirmed): the game ends a held note on roughly every 7th sounded note, about 0.8 s after its onset; the next note is lost when it arrives by hand-over or within about 1 s; the pattern is not exact (run 1 group 25 to 30 had 6 notes; some expected events skipped).

- Diagnostic run 7 (bisect, not one of the story's melody runs): `spikes/recordings/diag/ode-to-joy-60bpm-n01-08-lead2000ms.txt` (notes 01 to 08 of the 60 BPM melody, 2 s lead, 11.5 s; generated with `gen_trials_1_6.py --set melody --bpm 60 --notes 1-8 --extra-lead-ms 2000`; git-ignored location), played twice with `--mode melody --latency 0.12`. Run A (report `spikes/recordings/ode-to-joy-60bpm-n01-08-lead2000ms-20261121-192010`): notes 8, audible 7/8, right pitch 7/8, missed 1, cut 1, extra onsets 3 (04 D6: 3 onsets, 05 D6: 2); 07 B5 Y CUT SHORT, sounded 615 ms; 08 A5 A NO SOUND. Run B (report `...-20261121-192024`): notes 8, audible 7/8, right pitch 7/8, missed 1, cut 1, extra onsets 2 (04: 2, 05: 2); 07 CUT SHORT, sounded 710 ms; 08 NO SOUND. D6 (X) notes 04 and 05 had SNR about 13 to 14 dB versus about 28 to 30 for the others. Factual reading: notes 01 to 08 alone reproduce the 07-cut / 08-missed event (2/2), while notes 06 to 08 alone did not (0/3). Owner did not comment.

- Diagnostic run 8 (candidate fix, not one of the story's melody runs): `spikes/recordings/diag/ode-to-joy-60bpm-n01-08-lead2000ms-maxpress400ms.txt` (notes 01 to 08, 2 s lead, 11.5 s, every press capped at 400 ms with a neutral gap before every note, same onsets; generated with `gen_trials_1_6.py --set melody --bpm 60 --notes 1-8 --extra-lead-ms 2000 --max-press-ms 400`; git-ignored location), played once with `--mode melody --latency 0.12`: notes 8, audible 8/8, right pitch 8/8, missed 0, cut 0, extra onsets 2 (04 D6: 2, 05 D6: 2), cents spread -1 to +1 c, sd 0.3 c. Report `spikes/recordings/ode-to-joy-60bpm-n01-08-lead2000ms-maxpress400ms-20261121-192119.{csv,md}` (git-ignored). Factual reading: on the same notes that failed 2/2 with normal articulation, capped presses played all 8 notes in this single run (07 not flagged cut, 08 sounded). One run only; the analyser's cut rule may miss late stops (being tightened). Owner did not comment.

- Diagnostic run 9 (candidate fix on the full melody, not one of the story's melody runs): `spikes/recordings/diag/ode-to-joy-60bpm-maxpress400ms.txt` (62 notes, 65.5 s, every press capped at 400 ms with a neutral gap before every note, same onsets; generated with `gen_trials_1_6.py --set melody --bpm 60 --max-press-ms 400 --out-dir spikes/recordings/diag`; git-ignored location), `--mode melody --latency 0.12`, sender exit 0, 3931 frames, offset 0.376 s. Analyser: notes 62, audible 61/62, right pitch 61/62, missed 1, cut 0, extra onsets 5, cents spread -6 to +1 c, sd 1.1 c. Missed: 30 G5 R + stick up (NO SOUND, SNR 9.9 dB, just under the 10 dB audibility threshold). Extra onsets: 09 G5 (2), 24 G5 (2), 25 G5 (2), 45 G5 (2), 51 D6 (2). Report `spikes/recordings/ode-to-joy-60bpm-maxpress400ms-20261121-192320.{csv,md}` (git-ignored). Owner's ear: "when it took up the theme again at the end, the second note was silent" (translated from Italian). Discrepancy, stated as such: the owner's description points to the start of the final phrase (around note 48/49), while the analyser flagged note 30 and passed 48 and 49. Offline check (dev agent), after two analyser fixes (staccato notes judged up to heard release + 100 ms; extra attacks counted only before the heard release) and a tightened early-stop cut rule (tone at least 6 dB below its start, staying down until the heard release, fall starting 75 to 300 ms before it): re-analysis of the full capped run gives notes 62, audible 62/62, right pitch 62/62, missed 0, cut 0, extra onsets 1 (51 D6: 2). 30 G5 sounded normally (peak 25 dB, 410 ms); the live "NO SOUND" was an analyser artefact (a 400 ms note averaged over its 2 s slot). 48 B5 and 49 B5 both sounded at full level (31.6 / 31.4 dB, 444 / 430 ms); 48 is the 2-beat tied note, capped to about 0.4 s then about 1.6 s of silence, which plausibly explains the owner hearing a "silent second note" (a musical effect of the cap, not a drop). G5 extra onsets 09, 24, 25, 45 were release-tail artefacts. D6 (X) extra onsets (04, 05, 51) persist in every run including staccato: possibly real double attacks on X (X re-press gap unresolved in Story 1.4), unconfirmed. Re-analysis of earlier recordings with the tightened rule: a5x17 cut 2 (07 sounded 800/917 ms, 14 780/917 ms); n01-08 `-192010`: missed 08, cut 07 (615/1000), extra 04 (3), 05 (2); n01-08 `-192024`: missed 08, cut 07 (710/1000), extra 04 (2), 05 (2); n01-08 maxpress400 `-192119`: 8/8, cut 0, extra 05 (2); full run 1 (`-184708`): cut 8 (07, 15, 23, 30, 38, 48, 55, 62); full run 2 (`-185906`): cut 6 (07, 15, 30, 38, 46, 62). Working description (hypothesis, mechanism unknown): about every 7th sounded note the game ends a held note 0.54 to 0.91 s after its onset, then ignores input for up to about 1 s; capping presses at 400 ms avoided the audible effects in the runs so far (1 full run + 1 short run). Warning for Task 8 (U1): an early auto-stop near 0.8 s on attempts at positions 7, 14, and so on could be this event, not the sustain limit. Generator default (cap value, staccato vs re-press) was pending an owner decision. Owner decision (2026-11-21), replacing the pending generator default: for Story 1.6 the spike default is (a) staccato with presses capped at 400 ms (the tested variant); (b) splitting long notes into repeated presses of at most 400 ms is the preferred direction for the real pipeline later (follow-up, untested). The dev agent is regenerating the committed melody files with the cap; the U1 files stay uncapped.

- Melody 90 BPM run 1 (story melody run, Task 7): committed `spikes/samples/story-1-6/ode-to-joy-90bpm.txt` (now with the 400 ms press cap), `--mode melody --latency 0.12`, sender exit 0, 2651 frames, 44.183 s, recorded 45.6 s, offset 0.373 s. Report `spikes/recordings/ode-to-joy-90bpm-20261121-193350.{csv,md}` (git-ignored). Numbers are in the "Melody runs" row. Owner's ear: "good, but the second note of the theme was missing when it takes it up again at the end" (translated from Italian). The analyser reports all notes sounded; The analyser reports all notes sounded. Offline check (dev agent, replaces "pending"): notes 44 to 53 in `...-193350.wav` all sounded with clean attacks (peaks 21.7 to 31.7 dB above noise, 240 to 470 ms); 48 B5 and 49 B5 are two separate Y presses (frames 1910 and 1990, 0.93 s silence between), both sounded (428 / 424 ms). Nothing missing. Explanation (dev agent): the file used Beethoven's syncopated entry (B5 on beat 4 of bar 12 tied over the bar line); with the 400 ms cap it becomes a short early B5 then about 0.93 s of silence over the downbeat, heard as a missing note. D6 (51/52) late "onsets" are a measurement effect of the quiet, dipping X note; the X presses were on time. Diagnostic run 10 (trial of the alternative, not a story run): simplified common rhythm (`D5 2`, `B5 1` giving two B attacks on beats 1 and 2 of bar 13; same 62 notes, 64 beats), scratch file `ode-simplified-90bpm.txt` generated from a scratch copy of the melody, 90 BPM, capped: notes 62, audible 62/62, right pitch 62/62, missed 0, cut 0, extra onsets 1 (52 D6: 2), cents spread -3 to +2 c, sd 0.7 c. Report `spikes/recordings/ode-simplified-90bpm-20261121-193703.{csv,md}` (git-ignored). Owner's ear: "ok, now it sounds as I remember it, I heard all the notes" (translated from Italian). Owner decision (2026-11-21): adopt the simplified rhythm for the committed melody; the dev agent is updating the file. 90 BPM run 1 stays recorded as the syncopated version.

- Melody 90 BPM official run 1/3 (Task 7): final committed `spikes/samples/story-1-6/ode-to-joy-90bpm.txt` (simplified rhythm, owner decision 6; 400 ms cap, owner decision 5), `--mode melody --latency 0.12`, sender exit 0, 2651 frames, 44.183 s, offset 0.375 s, no INVALID flag. Numbers are in the "Melody runs" row. Report `spikes/recordings/ode-to-joy-90bpm-20261121-193926.{csv,md}` (git-ignored). Owner's ear: "it was fine" (translated from Italian). The earlier run on the syncopated version (`...-193350`) stays in the table, labelled superseded. Analyser: notes 62, audible 62/62, right pitch 62/62, missed 0, cut 0, extra onsets 2 (04 D6: 2, 20 D6: 2), cents spread -3 to +2 c, sd 0.7 c.

- Melody 90 BPM official run 2/3 (Task 7): same committed file and command, sender exit 0, 2651 frames, 44.183 s, offset 0.354 s, no INVALID flag. Analyser: notes 62, audible 62/62, right pitch 62/62, missed 0, cut 0, extra onsets 0, cents spread -3 to +1 c, sd 0.6 c. Report `spikes/recordings/ode-to-joy-90bpm-20261121-194033.{csv,md}` (git-ignored). Owner's ear: "it was fine" (translated from Italian).

- Melody 90 BPM official run 3/3 (Task 7): same committed file and command, sender exit 0, 2651 frames, 44.183 s, offset 0.354 s, no INVALID flag. Analyser: notes 62, audible 62/62, right pitch 62/62, missed 0, cut 1, extra onsets 0, cents spread -3 to +1 c, sd 0.6 c. Cut short: 52 D6 X sounded 300 of 400 ms. Report `spikes/recordings/ode-to-joy-90bpm-20261121-194134.{csv,md}` (git-ignored). Owner's ear: "it was fine" (translated from Italian). The cut flag on 52 D6 (the quiet X note at the microphone) is unverified: Offline check (dev agent, replaces "pending"): note 52 D6 in `...-194134` fades (-3 to -4 dB at about 0.28 s, about -10 dB by the release at 0.40 s) rather than stopping abruptly; in `...-193926` and `...-194033` note 52 holds within about 2 dB until release; 52 is not at a 7-note-event position; D6 is the quiet, dipping note in every run; the owner heard nothing wrong. Dev agent's reading: borderline measurement effect of the 6 dB early-stop threshold on a soft X fade, not the 7-note event. Run 3/3 is recorded as 62/62 sounded with this remark; the analyser's raw "cut 1" stays visible. Also: sustain mode and `--combine` now list interruptions less than 2 s after onset as EARLY STOPS (treated as the 7-note event, excluded from counts, D_ok, T_stop and the control check; those holds are to be re-run).

- U1 round 1 (Task 8.1) started 2026-11-21 about 19:47, 5 parts on button A. Room noise for the U1 recordings: owner reported (translated from Italian) "if notes with a different pitch and sound show up in the recording, it was the air fryer"; an air fryer was running in the room, exact times not given.

- U1 round 1 on A, Task 8.1 (2026-11-21, about 19:46 to 19:53): 5 files `spikes/samples/story-1-6/u1-sustain-A-part1..5.txt`, `--mode sustain --latency 0.12`, all sender exits 0. Combined with `--combine spikes/recordings/u1-sustain-A-part*-??????.csv` (5 files). 1000 ms (control) 0/5 interrupted; 4000, 5000, 5500, 6000, 6500 ms 0/5 each; 7000 ms 3/5 interrupted (auto-stop 6599, 6663, 6832 ms); 10000 ms (control) 5/5 interrupted (auto-stop 6810, 6831, 6874, 6937, 7157 ms). No-sound 0 everywhere; blanks silent (ok) in all 5 files; no EARLY STOPS listed. Combine line: D_ok = 6500 ms, T_stop = 6599 ms, candidate floor100(min(D_ok, T_stop) - 500) = 6000 ms for these rows only ("the rule needs every button"); confirmation on L, R, Y, X is still to run. Per file: part1 (194606) 10 s stop 6874; part2 (194728) 10 s 7157; part3 (194851) 7 s 6832, 10 s 6937; part4 (195014) 7 s 6599, 10 s 6831; part5 (195137) 7 s 6663, 10 s 6810 (ms). SNR range about 19.5 to 29.6 dB (lower in parts 4 and 5). Wavering numbers: not reported. Owner did not comment on the result. The new `max_sustain_ms` is not final.

- Owner decision (2026-11-21): skip the U1 confirmation on L, R, Y, X. Owner's words (translated from Italian): "let's take it for granted that the values are the same for all buttons". Recorded as: L, R, Y, X not measured, owner decision: assumed equal to A. `max_sustain_ms` is measured on A only; the other buttons are an assumption. The dev agent is updating the profile (Part C) with 6000 ms; the final value is recorded when reported.

- Session close (2026-11-21): optional 120 BPM run not run (coordinator decision, session ended). 60 BPM runs only on the earlier uncapped, syncopated file; no 60 BPM run with the final file. Final `max_sustain_ms = 6000` (tag MARGIN; measured 6500 ms on A, margin 500 ms); L, R, Y, X not measured, assumed equal to A (owner decision 7). Open: the early-stop mechanism is a hypothesis; U1 wavering numbers not reported; 2wiCC_Comms version not probed; capture self-test not run.

### File List

- `spikes/gen_trials_1_6.py` (new)
- `spikes/test_gen_trials_1_6.py` (new)
- `spikes/samples/story-1-6/ode-to-joy.melody` (new)
- `spikes/samples/story-1-6/ode-to-joy-60bpm.txt` (new, generated)
- `spikes/samples/story-1-6/ode-to-joy-90bpm.txt` (new, generated)
- `spikes/samples/story-1-6/ode-to-joy-120bpm.txt` (new, generated)
- `spikes/samples/story-1-6/u1-sustain-A-part1.txt` to `u1-sustain-A-part5.txt` (new, generated)
- `spikes/listen_trial.py` (modified: `--mode melody`, `--mode sustain`, `--latency` default per mode, sustain `--combine`)
- `spikes/test_listen_trial.py` (modified: Story 1.6 synthetic-audio tests appended)
- `spikes/serial_rx_log.py` (new: read-only firmware message logger, Part B diagnosis)
- `spikes/test_serial_rx_log.py` (new)
- `spikes/README.md` (modified)
- `app/profiles/oot-switch2-1.1.1.toml` (modified, Part C: `max_sustain_ms`)
- `spikes/test_default_profile.py` (modified, Part C: sustain bound)
- `docs/score-format-and-mapping.md`, `docs/ocarina-input-mechanics.md` (modified, Part C: value quotes)
- `_bmad-output/specs/spec-bonooru/roadmap.md` (modified, Part C: M2 wording)
- `docs/test-plan.md` (modified: row 1.10, "Walking skeleton (Story 1.6)" empty tables)
- `_bmad-output/implementation-artifacts/sprint-status.yaml` (modified: 1-6 in-progress)

## Change Log

- 2026-11-21: story created (ready-for-dev). owner decisions recorded (U1 rule with 500 ms margin, G major full theme, in-tune criterion, 90 BPM main run).
- 2026-11-21: Part A implemented (generator, melody and U1 trial files, analysers, tests, README, empty result tables). Status in-progress; Part B (Switch session) and Part C pending.
- 2026-11-21: Part B diagnosis support: analyser `CUT SHORT` verdict (incl. early stops near the release), staccato and release-tail fixes, fewer false extra onsets; `serial_rx_log.py`; generator diagnostic options (`--notes`, `--extra-lead-ms`, `--max-press-ms`). Owner decision 5: committed melody files regenerated with the 400 ms press cap (`DEFAULT_MAX_PRESS_MS`); known issue and follow-ups recorded in Dev Notes.
- 2026-11-21: Owner decision 6: `ode-to-joy.melody` uses the simplified common rhythm in bar 12 (D5 half, then B B on beats 1-2); committed melody files regenerated. Recordings made before this change used the syncopated version.
- 2026-11-21: Owner decision 7 (U1 confirmation on L/R/Y/X skipped, buttons assumed equal to A). Task 9 (Part C): `max_sustain_ms` 5000 (ESTIMATE) -> 6000 (MARGIN: measured 6500 on A, margin 500; test plan row 1.10) in the profile; value quotes updated in `docs/score-format-and-mapping.md`, `docs/ocarina-input-mechanics.md` (U1), `docs/test-plan.md` (rows 1.3, 1.10 result cell, Story 1.5 paragraph), roadmap M2; `spikes/test_default_profile.py` bound now from the measured D_ok/T_stop minus the 500 ms margin; committed Story 1.6 trial files regenerated (headers quote the profile values; frames unchanged).
- 2026-11-21: Code review fixes (two reviews, owner approved): wording/evidence in README, test plan and story; generator sharps-vs-comments parsing, negative modifier frame guard, `split_sustain` guard; `serial_rx_log.py` port-error and log-file handling. Deferred items recorded as follow-ups; "Review findings" added to Dev Notes.
- 2026-11-21: Part B session run with the owner; results recorded (see Dev Agent Record, Part B session notes, and `docs/test-plan.md`).
- 2026-11-21: Acceptance criteria verdict recorded (AC 1 met at 90 BPM with the final file; AC 2 met; AC 3 met for button A, other buttons assumed). Status set to done; Epic 1 done.
