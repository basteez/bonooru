---
baseline_commit: 16df35b4e701b38853706ce1299f9f13b7ea3534
---

# Story 1.4: Timing limits

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As the owner,
I want to measure the minimum press, retrigger gap, modifier lead and frame rate,
so that the timing rules and the frame grid come from data, not guesses.

## Acceptance Criteria

Copied from `_bmad-output/planning-artifacts/epics.md`, Story 1.4. These are the definition of done.

1. **Given** the spike sender and a recording setup (Switch video capture, capture card, or a phone on the console speaker; R10)
   **When** the owner runs Spike 2 rows 2.1, 2.2 and 2.3 with 20 tries per step
   **Then** U7 (min press), U2 (min gap same button) and U5 (modifier lead) each have the smallest value that passes 20/20 recorded in frames and ms
2. **Given** a 60 fps capture while playing
   **When** frames are counted (row 2.5)
   **Then** U10 (frame rate while playing) is recorded
3. **Given** a 120 BPM 16ths passage for 30 s (row 2.4)
   **When** onsets are detected (e.g. aubio)
   **Then** the onset jitter sigma is recorded and compared with the 1-frame target, noting that 2wiCC's per-frame queue is not our scheduler

## Tasks / Subtasks

Order matters. **Part A (tasks 1 to 6) needs no Switch and no hardware.** **Part B (tasks 7 to 11) needs the console and is run by the owner, not by the dev agent.** The dev agent does Part A, then **STOPS** and hands back the trial list (Task 6.3). The agent never plugs anything in, never opens a serial port, never records from the real console and never claims a trial result.

### Part A: no console needed [no-Switch]

- [x] **Task 1: Trial generator `spikes/gen_trials_1_4.py`** (AC: #1, #3)
  - [x] 1.1 Single file, PEP 723 header (`requires-python = ">=3.10"`, no dependencies), MIT line and "throwaway spike code" comment like `gen_trials_1_3.py`. Writes into `spikes/samples/story-1-4/`. Deterministic output (fixed seed for the order inside rounds), generated files **are committed**. **Reuse** `gen_trials_1_3.py` (`check_allowed`, `Stick`, `ALLOWED_*`, `BUTTON_MIDI`, `GuardError`) by loading it with importlib, as `listen_trial.py` does; do not copy the guard.
  - [x] 1.2 **All times in frames** (`12f` row times), never ms, so the sender's ms-to-frame rounding cannot blur a 1-frame step. The frame rate of the grid is the sender's 60 Hz (2wiCC timer, 16.666 ms). Every row has a `# label`; scored attempts start with `TRY`, so the analyser can find them.
  - [x] 1.3 **Attempt layout (shared)**: neutral `--pre` frames (default 30) so the analyser has a quiet window; (modifier set, if any); the press; release; neutral `--post` frames (default 90, 1.5 s, long enough for the note to die away and for the next attempt to start clean). Defaults are options.
  - [x] 1.4 **Rounds, not blocks.** Each file is a list of *rounds*; a round holds one attempt per sweep value (plus the controls), in a seeded shuffled order. 20 rounds = 20 tries per step. Reason: the sub-frame phase between the sender's 60 Hz grid and the game's frame grid is almost constant inside one run (the 2wiCC timer and the game clock are independent and differ by a small fraction, so the phase only drifts slowly). Twenty tries in a row at one step would be one phase tested twenty times. Interleaving gives every step the same phase mix, and the session length must be at least several minutes (see Dev Notes, "Phase diversity"). Split the rounds over several files (default 5 rounds each) to keep a failed run cheap; the label carries `round r/20`.
  - [x] 1.5 **Warm-up attempt**: every file starts with one sacrificial attempt labelled `WARMUP` (not scored). Known quirk: after every Joy-Con connect or disconnect the first injected input is ignored (seen three times in Story 1.3).
  - [x] 1.6 **U7 file(s) `u7-minpress-*.txt` (row 2.1)**: button A, no modifiers, press length `d` = 1..10 frames (`--press-frames 1-10`), 20 rounds. Each round also holds one **blank** attempt (no press at all) as a negative control: the analyser must report 0 sounds on blanks.
  - [x] 1.7 **U2 file(s) `u2-gap-*.txt` (row 2.2)**: button A twice: press `H` frames (default 12, safely above U7 once known; option `--hold-frames`), release `g` frames, press `H` frames again; `g` = 1..10 (`--gap-frames 1-10`), 20 rounds. Controls in every round: `g = 0` (one continuous press of length 2H, the "no retrigger" reference) and `g = 30` (clear retrigger, the "retrigger" reference). The analyser uses them to set the dip threshold (Task 3.3).
  - [x] 1.8 **U5 files `u5-lead-<mod>-*.txt` (row 2.3)**: one family per modifier, four of them: `dup` (hat up), `ddown` (hat down), `sup` (left stick `ly 0x00`, full travel), `sdown` (`ly 0xFF`), base button A, `lx 0x80`. The modifier is set `k` frames **before** the button press (`k` = 0..5 frames for the AC, `--leads 0-5`), kept until `--tail` frames after the release; hold the note `--note-frames` (default 36, 0.6 s) so the steady pitch can be read after the start. 20 rounds. Stick is its own family because U4/U6 say it glides and is graded: its lead may be much longer than the D-pad's. **Extension round** (only run when a family has no 20/20 by `k = 5`): `--leads 0-5,6,8,10,12,15,20,30`, the generator must produce it from the same command.
  - [x] 1.9 **Confirmation files `confirm-*.txt`**: once U7/U2/U5 are known on A, the same attempt at the found threshold plus one step below, on L, R, Y and X, 20 tries each, one file per value, built with the same options (`--button`). Default option values are the A thresholds from the results; the dev agent only provides the options, the owner supplies the numbers after Part B round 1.
  - [x] 1.10 **Row 2.4 file `jitter-120bpm-16ths.txt`**: 30 s of 120 BPM sixteenth notes (240 notes, 125 ms = 7.5 frames each, so the sender grid alternates 7 and 8 frames: this is a **known, deterministic** quantisation that the analysis compares against, not noise). The pattern cycles through the five buttons (L R A Y X Y A R ...) with no modifiers so every onset is also a pitch change. Press length `P` comes from `--press-frames` (default 4) and the release gap is the remainder of each 7 or 8-frame slot (so `P + gap = 7.5 frames` on average); both must be at or above the measured U7/U2, so this file is **generated after Part B round 1**, and the generator refuses `P` or the smallest gap below `--min-press` / `--min-gap` when given. Neutral lead-in 2 s and tail 1 s.
  - [x] 1.11 **Frame-rate scene** is not a trial file (see Task 5). The generator does not send anything for U10.
  - [x] 1.12 **Allow-list guard (hardware safety, AD-9)**: only `L R A X Y`, hat neutral/up/down, left stick, never B, ZL, ZR, a left or diagonal D-pad, right stick, Capture, Home, `+`, `-`, L3, R3. Every `Note`/row goes through the 1.3 `check_allowed`. The generator raises `GuardError` for anything else and for press/gap/hold below 1 frame, and `--press-frames` above 120.
  - [x] 1.13 Console output: per file, name, number of attempts, total duration; `--key` prints the key (attempt, time, expected outcome as "model", never as a result).

- [x] **Task 2: Tests for the generator `spikes/test_gen_trials_1_4.py`** (AC: #1, #3)
  - [x] 2.1 pytest, no hardware; load the generator and `send_2wicc.py` by importlib like `test_gen_trials_1_3.py`.
  - [x] 2.2 Every committed file parses with `send_2wicc.parse_lines`, schedules, encodes, and is byte-identical to a fresh generator run.
  - [x] 2.3 Allow-list property test over every parsed state and scheduled frame (as 1.3 Task 3.3), plus "the right stick is never set".
  - [x] 2.4 AC coverage: U7 has press lengths 1..10 frames, 20 tries each, plus blanks; U2 has gaps 1..10 plus controls 0 and 30, 20 tries each; U5 has leads 0..5 for each of the 4 modifiers, 20 tries each; each U5 attempt sets the modifier exactly `k` frames before the button (count frames on the scheduled states, not on the labels); no attempt starts with a modifier already held from the previous attempt; the round order is deterministic and every round holds every step once; the jitter file (built in memory with the default options; it is committed only after Part B round 1) has 240 notes over 30 s and the frame lengths sum to the scheduled total.
  - [x] 2.5 `uv run --with pytest --with pyserial --with numpy pytest spikes/` must pass (292 existing tests included).

- [x] **Task 3: Analysers in `spikes/listen_trial.py`** (AC: #1, #3) -- the 1.3 analyser reads one pitch in a window from press + 0.3 s to release - 0.2 s, which is empty for a 1 to 10 frame press, so it cannot score 2.1 to 2.3
  - [x] 3.1 Add `--mode {pitch,detect,retrigger,lead,jitter}` (default `pitch`, behaviour and tests unchanged). Attempts are found from the `TRY` rows of the trial (parser as in `presses_from_rows`), not from the press length.
  - [x] 3.2 **`detect` (row 2.1)**: per attempt, sound yes/no = band-limited level (same 250 to 4000 Hz band-pass and quiet-floor SNR as now, `--min-snr-db` 10) in `[t_press - 0.05 s, t_press + 0.8 s]` clearly above the pre-press noise; also reports onset delay (ms, informative only) and audible length (time above peak - 20 dB). A pitch is reported when enough voiced frames exist (short windows, same YIN) and a wrong pitch counts as **fail**. Blank attempts must give no sound. **[Superseded by the final analyser (2026-11-21): see Debug Log "Fixes in `listen_trial.py`" and `spikes/README.md`, "`listen_trial.py` timing modes".]**
  - [x] 3.3 **`retrigger` (row 2.2)**: per attempt, count the onsets in the attempt window from a short-hop envelope (hop 2 ms, smoothed 6 ms); a retrigger = the level falls at least `--dip-db` below the first note's plateau and then rises by at least `--rise-db` again, with the second rise within the attempt. `--dip-db` / `--rise-db` default to values taken from the `g=0` and `g=30` controls of the same recording (report them), so no threshold is invented in advance. Output per attempt: onsets (1 or 2), dip depth. **[Superseded by the final analyser (2026-11-21): see Debug Log "Fixes in `listen_trial.py`" and `spikes/README.md`, "`listen_trial.py` timing modes".]**
  - [x] 3.4 **`lead` (row 2.3)**: per attempt, pitch track of the first part of the note with 30 ms YIN windows, hop 5 ms, from the detected onset; pass = the median pitch of the windows that start in `[onset, onset + 40 ms]` is within `--lead-cents` (default 50) of the steady pitch of the same attempt (median of the last half of the hold). Also report the cents error at the first window. For the stick the steady pitch is the glide's end value, so a glide that has not finished shows as a fail; that is the intended meaning (modifier fully applied at note start). Resolution limit: wrong pitches shorter than about 30 ms cannot be seen (see "Measurement method"). **[Superseded by the final analyser (2026-11-21): see Debug Log "Fixes in `listen_trial.py`" and `spikes/README.md`, "`listen_trial.py` timing modes".]** **[Also superseded: the onset is found on the level of the whole 0.8 to 1.25 x band (`band_onset`, code review F1); resolution about 35 ms (2 frames).]**
  - [x] 3.5 **`jitter` (row 2.4)**: detect onsets over the whole file (same envelope, onset = first crossing of 50 % of the local peak, to be insensitive to slow attacks), match them in order to the scheduled press frames (and to the pitch of each button as a sanity check), compute the **inter-onset interval error** = measured IOI minus scheduled IOI **in sender frames** (not ideal ms), then mean, sigma, p95, max (ms and frames) and the drift (slope of cumulative error). Also report sigma of the schedule's own quantisation (the 7/8 frame pattern) so it is not mistaken for jitter. IOI cancels the constant latency, the offset and, to first order, the capture scale. **[Superseded by the final analyser (2026-11-21): see Debug Log "Fixes in `listen_trial.py`" and `spikes/README.md`, "`listen_trial.py` timing modes".]**
  - [x] 3.6 **Lossless capture** (see "Measurement method"): the ffmpeg avfoundation capture drops about 11 % of the samples in 1024-sample packets (about 21 ms steps, more than a frame), which is acceptable for per-attempt yes/no but not for IOI. Add a second backend `--backend sounddevice` (PortAudio, callback with a lock-free queue, `sounddevice` added to the PEP 723 dependencies, optional import so the existing ffmpeg path and its tests still work), writing the same WAV and the same `(arrival, samples)` chunk list. It must report any xrun/overflow status flag from the callback and print the number; a run with any overflow is marked **invalid** in the report. Keep `--backend ffmpeg` as the fallback and report the lost-sample scale as today.
  - [x] 3.7 **Capture integrity self-test (no console)**: `--selftest-tone` records `--seconds` (default 60) of a steady 1 kHz tone played from any other device (a phone or a laptop), finds phase discontinuities (a step larger than 0.2 ms in the fitted phase), and prints their count and the delivered-sample ratio. Pass for the sounddevice backend: 0 discontinuities and ratio within 0.1 % of 1.0. Run it once per session before Part B (Task 7.4).
  - [x] 3.8 Tests in `spikes/test_listen_trial.py` (numpy synthetic audio, no microphone, no sleeping, skipped without numpy like now): synthetic A5 bursts of 1, 2, 3 ... frames with a decay give "sound" from the right length and "no sound" for silence and for a noise-only blank; a two-burst signal with a 1-frame dip is one onset when the dip is shallower than the threshold and two when deeper; the `lead` analyser fails a synthetic note that starts 50 cents off for 60 ms and passes one that starts right; the jitter analyser recovers a known sigma from synthetic onsets with a known IOI pattern (including the 7/8 frame pattern) within 1 ms and is unaffected by a constant shift and a 0.5 % scale error; the tone self-test counts a synthetic 2-discontinuity signal as 2; the `pitch` mode results are unchanged on the existing fixtures.

- [x] **Task 4: Sender: nothing, unless proven necessary** (AC: #1)
  - [x] 4.1 Do **not** change `encode`, `schedule`, `Pacer` or the stop sequence (verified on hardware). The `Nf` row times and `--announce` already exist. If a trial needs something the sender cannot do (for example a sub-frame step, which the 60 Hz queue cannot express), say so in the Completion Notes instead of working around it.
  - [x] 4.2 Check by dry run that a 1-frame press really produces exactly one `QD` frame with the button and the surrounding neutral frames: `uv run spikes/send_2wicc.py --dry-run spikes/samples/story-1-4/<file>`; put the lines for press length 1 and for gap 1 into the Completion Notes. The sender's own warning for a row that gets zero frames must not appear for any generated file (add that as a test: parse, schedule, assert `dropped == []`).

- [x] **Task 5: Frame-rate tool `spikes/count_frames.py`** (AC: #2)
  - [x] 5.1 Why: U10 needs the number of **distinct** images per second the game produces (30 or 60, possibly a lower logic rate), counted in a video of the screen that has more frames per second than the game. PEP 723, dependency `numpy`; video frames are decoded with `ffmpeg` (rawvideo gray, downscaled) through a subprocess, kept thin so the counting logic is a pure function on a numpy array of frames.
  - [x] 5.2 Output: capture fps (from the file), number of frames, number of **changes** (mean absolute difference between consecutive frames above a threshold derived from the noise of the static parts, reported), unique frames per second over each 1 s window (min, median, max) and the **run-length histogram** of identical consecutive frames (for example 60 fps game in a 240 fps phone video gives runs of 4; a 30 fps game gives runs of 8; a 20 fps logic gives runs of 12). The median run length times 1/capture-fps is the game's frame period; also print the capture fps as an assumption, not a result.
  - [x] 5.3 Tests (numpy, no ffmpeg): synthetic frame stacks (a moving square) at known duplication factors give the right unique-frame rate and run histogram; sensor noise below the threshold does not create fake changes; a variable-rate sequence reports min and max. One optional test with `ffmpeg` is skipped when the binary is missing.
  - [x] 5.4 Videos are **never** committed: `spikes/recordings/` is git-ignored (already). Screenshots of the game are never committed either.

- [x] **Task 6: Docs, dry runs and the hand-over list** (AC: #1 to #3)
  - [x] 6.1 Dry run every generated file (exit 0, no zero-frame warning); record frames and duration per file in the Completion Notes.
  - [x] 6.2 Update `spikes/README.md` (index rows for the new files, a "Story 1.4 trials" section: files, how to regenerate, free-play entry steps, forbidden inputs, the capture options). Do not rewrite the 1.2/1.3 content.
  - [x] 6.3 **Hand-over (the dev agent's last step)**: fill the "Field trial list" table in the Dev Agent Record with the exact commands (port left as `<port>`), file, attempts, duration and the run order of Part B; set the story to `in-progress` (not `review`) and **stop**. Say: "Part A done; Part B needs the Switch, here is the list". Do not tick Part B tasks or any AC.

### Part B: needs the Switch, run by the owner (the dev agent stops before this) [Switch]

- [x] **Task 7: Hardware safety and session setup** (AC: #1 to #3) (7.5 optional, not done)
  - [x] 7.1 **Hardware safety first** (`docs/test-plan.md` "Hardware safety (before every session)"): wiring is only GP0, GP1, GND, nothing on 5V or 3V3; rewire only with both USB cables unplugged; **GND to GND continuity with a multimeter, expect about 0 ohm (200 range), before powering anything** (breadboard rails are not connected unless bridged); Switch on dock or stand, short cable on the Switch side, slack on the PC side. Rules for these trials, all about not leaving free play: keep the real controller within reach to take over; start a file only after the ocarina is open and the game has accepted input for 2 s (enter with D-pad left on the real controller); dismiss the "press A when the controller is ready" prompt by hand first (the sender never presses A on its own); stop with a **real Ctrl+C in a terminal**; never edit a trial file to add B, ZL, ZR, a left or diagonal D-pad, or the right stick. **[Skipped by owner decision (2026-11-21): wiring unchanged since the GND↔GND 0.1 Ω measurement recorded in `docs/test-plan.md`; bridge probes `+ID`/`+VER`/`+GQS` verified each time the controller was connected.]**
  - [x] 7.2 Environment as in Story 1.3 (write the actual values into the results; changed versions invalidate comparisons): macOS host, port (check with `--list-ports`), cable, console bottom port, 2wiCC v2.2 + 2wiCC_Comms v1.1, Pro Controller Wired Communication ON, Switch 2 system version, OoT version. Also record docked or handheld and the video mode: they can change the frame rate. **[Switch 2 23.5.0, OoT 1.1.1, ports and host recorded; console handheld (built-in screen, not docked) for all trials, so there is no TV video mode.]**
  - [x] 7.3 **Quirk: after every Joy-Con connect or disconnect the first injected input is ignored.** Do not detach or reattach Joy-Con inside a session. After any connect or disconnect: dismiss prompts by hand, then run `calibration-A.txt` from Story 1.3 (the first A is ignored, so expect the second to sound) before the next trial file. Each file's `WARMUP` attempt also absorbs one lost input. A file whose first scored attempt fails with no sound at all is re-run, not counted.
  - [x] 7.4 Capture setup: Mac microphone near the console speaker, **same place, same volume, quiet room** for the whole session (note them in the results). Run the tone self-test (Task 3.7), then `calibration-A.txt` in `--mode detect`: the A5 must be detected with SNR well above 10 dB.
  - [ ] 7.5 Optional, if the owner has one: a capture card or the console's own capture as a second observer (Open question 1, resolved: no card planned). Not required. **[Not done: optional; no capture card (Open question 1).]**

- [x] **Task 8: Round 1: rows 2.1, 2.2, 2.3** (AC: #1)
  - [x] 8.1 Run order: `u7-minpress-*` (`--mode detect`), `u2-gap-*` (`--mode retrigger`), then the four `u5-lead-*` families (`--mode lead`). Total about 30 to 45 min of playback; take breaks between families, but keep the session continuous across all files of one row (phase diversity). Keep the audio level and mic position unchanged; if anything is moved, say so in the results.
  - [x] 8.2 Per row, record **per step** the number of passes out of 20 (table, rows = steps in frames and ms). Result = the **smallest value for which that step and every larger step passed 20/20** (as in 1.3), in frames and in ms (frames x 16.667). A blank attempt that sounds, or a control (`g=0`, `g=30`) that does not behave, **invalidates** the row: find out why before using the number.
  - [x] 8.3 U5 per modifier: D-pad up, D-pad down, stick up, stick down each get their own smallest lead; the profile's single `modifier_lead` is decided in Story 1.5 (the largest of the four, or per type).
  - [x] 8.4 If any step shows a partial result (for example 17/20) record it as a finding with the pattern (random, or tied to the round number or position, which would point to the phase): do not round it up.
  - [x] 8.5 If any run leaves free play, the console in a menu, or a note stuck: Ctrl+C, take over with the real controller, record what happened, do not continue the file blindly.

- [x] **Task 9: Confirmation on the other buttons** (AC: #1)
  - [x] 9.1 Generate and run the `confirm-*` files at the found thresholds (and one step below) for L, R, Y and X. If a button needs a larger value than A, that value is the profile value and the difference is a finding.

- [x] **Task 10: Row 2.5 frame rate (U10) and row 2.4 jitter** (AC: #2, #3)
  - [x] 10.1 **U10**: record the screen while the game runs and shows continuous motion (free play with Link idle, the camera or a flag moving; the sender is not needed). Use a camera with more frames per second than the game (a phone in 240 or 120 fps slow motion pointed at the screen; or a 60 fps capture card). The Switch 2's own clip is limited to 30 s and its frame rate is not documented, so it is not the primary tool. Record a few minutes in total, in the same dock/handheld mode as the trials, and also while playing notes (not just idle). Run `count_frames.py`; U10 = unique frames per second (and the logic rate if different). Record how the video was made. The frame rate of the **capture** is not the answer: a 60 fps recording of a 30 fps game shows 30 distinct images per second. **[Partial: 41 s video, not a few minutes, and the scene was not in continuous motion; the frame period (16.67 ms) is shown, sustained 60 images per second is not.]**
  - [x] 10.2 **Row 2.4**: generate `jitter-120bpm-16ths.txt` from the Round 1 values (Task 1.10), run it with `--mode jitter --backend sounddevice` **at least 3 times** (separate runs show different phase), record the sigma per run. Compare with 1 frame (16.7 ms at 60 fps) and write the sentence: "2wiCC's per-frame queue is not our scheduler (the sender grid is quantised at 7/8 frames here, the real firmware scheduler is Story 2.4/2.5)".
  - [x] 10.3 Cross-check the audio onset precision once: the same jitter file with the ffmpeg backend shows how much the lost samples inflate sigma (this documents why the lossless backend is used).

- [x] **Task 11: Record the results** (AC: #1 to #3); see "Where results go"
  - [x] 11.1 `docs/test-plan.md`: fill the Result cells of rows 2.1 to 2.5; add a "Story 1.4 field trials" section under Spike 2 (environment, capture, per-step tables, controls, confirmation on other buttons, frame-rate method and count, jitter table).
  - [x] 11.2 `docs/ocarina-input-mechanics.md`: answers for U2, U5, U7, U10 (frames and ms), same style as U6.
  - [x] 11.3 `docs/test-plan.md` Spike 2 "Done when: the `[timing]` section of the profile is filled with measured values + safety margin": add a pointer that the measured values exist (Story 1.5 fills the profile).
  - [x] 11.4 `spikes/README.md`: OQ-3 (frame period) measured result; keep the rest.
  - [x] 11.5 Set the story to `review` only when every AC is true on recorded data, not on the model. **[Set to `review` on the owner's instruction (2026-11-21); AC status in the Completion Notes.]**

## Dev Notes

### What this story is

A measurement story, not a feature. Part A makes trial files and the small analysis tools; Part B is measurements by the owner, recorded as numbers in docs. Nothing is shipped; nothing goes into `app/` or `firmware/`. The sender is throwaway spike code (MIT); 2wiCC binaries are testing use only and never committed.

### What Story 1.5 needs from here

Story 1.5 writes `[timing]` in the default profile (`docs/score-format-and-mapping.md`: `min_press`, `min_gap_same_button`, `modifier_lead`, `start_lead`, `max_sustain`) and `[profile] frame_rate`, each with the spike row it came from, plus a stated safety margin. From this story it takes:

| Profile field | From | Needed form |
|---|---|---|
| `min_press` | U7, row 2.1 | smallest passing press, frames and ms, on A and confirmed on L R Y X |
| `min_gap_same_button` | U2, row 2.2 | smallest passing release gap, frames and ms |
| `modifier_lead` | U5, row 2.3 | per modifier (D-up, D-down, stick up, stick down), frames and ms; the profile has one field, so give the maximum and the per-type values |
| `[profile] frame_rate` | U10, row 2.5 | distinct frames per second while playing, and the capture method |
| jitter sigma | row 2.4 | sigma, p95, max in ms and frames; basis for Story 2.5 and the Epic 7 targets |

The placeholders in the TOML (50, 34, 17 ms) are guesses; the measured values replace them. `start_lead` (500 ms) is not measured here. Write results so each is findable by row number (2.1 to 2.5).

### Measurement method: audio or video? (decision for this story)

The question is whether the audio onset precision is enough for frame-level numbers (16.7 ms at 60 fps, 33 ms at 30 fps).

What the existing tool can and cannot do (from `spikes/listen_trial.py` and the Story 1.3 results):
- It captures with ffmpeg avfoundation, which loses about 11 % of the samples in 1024-sample packets (about 21 ms steps, more than one frame). This is a known upstream bug (ffmpeg trac #11398, "avfoundation audio capture had missing samples randomly", gaps every few thousand samples; older reports back to 2020). Compensating with a scale factor fixes the average rate but not the position of the individual gaps.
- Its onset detector has a 10 ms hop and takes the alignment from `--announce` lines (accurate to about half a frame plus host jitter) and chunk arrival times, so the **absolute** latency of an onset is good to roughly +-20 ms at best. Within one attempt the relative timing is better, but a lost packet inside the attempt still shifts it by up to 21 ms.
- It analyses a window from press + 0.3 s, which does not exist for a 1 to 10 frame press.

Recommendation (reasoning in short):
1. **Rows 2.1 and 2.2 (U7, U2): audio is sufficient, because the pass/fail per attempt is binary** (a note sounded; a second note sounded). The result is a count out of 20 per step, so onset precision does not matter; only "is there sound in a 0.8 s window" and "is there a dip and a re-attack" matter. Dropped samples cannot create or hide a note. Controls (blank attempts, `g=0` and `g=30`) calibrate false positives and the dip threshold. Audio is the right observable anyway: what counts is that the game **played** a note.
2. **Row 2.3 (U5): audio with a short-window pitch track**, binary per attempt (correct pitch at note start or not). Limit stated up front: a wrong pitch shorter than about 30 ms cannot be resolved with a 30 ms YIN window; a one-frame (17 ms) blip is below the resolution. Open question 3 asks whether that limit is acceptable.
3. **Row 2.4 (jitter): audio can reach a frame only with a lossless capture and an IOI metric.** Use PortAudio (`sounddevice`) instead of ffmpeg, a 2 ms onset hop, the 50 %-of-peak crossing, and inter-onset intervals against the sender's own frame schedule. That gives a measurement noise of a few ms. It still includes the game's audio output buffering (the jitter we want to know is the heard jitter, which is what matters musically), so the sigma is "input path plus game plus audio output", not the input path alone.
4. **Row 2.5 (U10): video, not audio.** The frame rate is a property of the picture. Count distinct images in a recording with more fps than the game (phone slow-motion at 240 or 120 fps, or a 60 fps capture card). A 60 fps recording only proves 60 if the images change every frame.
5. Video of the screen is **not** needed for rows 2.1 to 2.4; a capture card or the console's own clip would give clean digital audio, but the console clip is limited to 30 s and its frame rate is not documented, so it is an optional second observer (Open question 1), not the plan.

### Phase diversity (why rounds and long sessions)

The sender queue runs on the 2wiCC 60 Hz timer (16666 us, 60.0024 Hz). The game runs on its own clock. The sub-frame offset between the two is constant inside one short run and drifts slowly. If one step were tested 20 times in a row, the 20 tries would mostly share a phase, so 20/20 could mean "passes at this phase", not "always passes". Mitigations in the design: interleaved rounds (all steps see the same phase mix), sessions of several minutes per row, the sacrificial warm-up, and a report of pass counts against round number (a pattern tied to the round points to a phase effect). The sender cannot do sub-frame steps (the queue is one state per frame), so phase cannot be set directly; do not try to add dithering to the sender.

### Trial timing (defaults, all options, frames at 60 Hz)

| Parameter | Default | Why |
|---|---|---|
| pre | 30 frames | quiet window before the press for the SNR reference |
| post | 90 frames | the note dies away, the next attempt starts clean |
| U7 press | 1..10 frames, 20 rounds | row 2.1 |
| U2 hold H | 12 frames | above the expected U7, so the first and second note are both fully pressed |
| U2 gap | 1..10 frames, plus 0 and 30 as controls | row 2.2 |
| U5 lead | 0..5 frames (extension up to 30) | row 2.3 |
| U5 note | 36 frames | enough to read the steady pitch after the start |
| rounds per file | 5 | a failed run costs about 1.5 to 4 min |

Rough totals (dev agent computes the exact numbers in the dry run): U7 about 6 min, U2 about 9 min, U5 about 4 x 4 min.

### Known facts to build on (do not re-measure)

- A5 about 894 to 896 Hz in the game (+27 to +29 cents vs A4 = 440 Hz); compare pitch with the same attempt's own steady pitch, not with the nominal 880 Hz (test plan, "Story 1.3 field trials").
- Buttons L R A Y X sound D5 F5 A5 B5 D6; D-pad steps instantly (U4), the left stick is a graded bend, about linear in cents, full travel only (`ly 0x00` up, `ly 0xFF` down) for the +-2 semitones (U6). On X the D-pad shifts about +-115 cents.
- A held note wavers from about 2 s and stops by itself at about 6 s (U1). A new button cuts the held one (U3). Free play: enter with D-pad left, input accepted after about 2 s, B or D-pad left exits (U8, U9). ZL opens the song menu.
- After every Joy-Con connect or disconnect the first injected input is ignored (test plan and `docs/ocarina-input-mechanics.md`).
- The OoT version in use is the Switch 2 release. Press reports say it runs at 60 fps (compared with 20 fps on the N64), but no source states the logic or input rate; U10 is therefore measured, not assumed.

### Developer guardrails

- **Free play is fragile.** The generator must make forbidden inputs impossible to emit (Task 1.12) and the tests must prove it (Task 2.3). The spike sender has no AD-9 mask, so the generator is the only guard.
- The input format and encoder are Story 1.2's: do **not** change `encode`, `schedule`, `Pacer` timing or the stop sequence. A 1-frame press is one `QD` (or `QF` with the stick) frame; QD/QF mixing is already exercised by Story 1.3.
- `listen_trial.py` changes must keep the 1.3 behaviour and all its tests green (`--mode pitch` is the default). The ffmpeg backend stays available.
- Python only; standard library plus `pyserial`, `numpy`, and (optional, for the lossless capture) `sounddevice`. Tests run with `uv run --with pytest --with pyserial --with numpy pytest spikes/`; `uvx ruff check spikes/` ad hoc.
- No recorded audio or video, no game screenshots, no UF2/2wiCC binaries in the repo (`spikes/recordings/` is git-ignored), never reference anything under `ux-designs/*/imports/`. Everything written to the repo is English.
- New thresholds (dip depth, 50 cents, 10 dB SNR) are options with defaults taken from the controls, and the report must print the values used.

### Architecture compliance

- **AD-9** (`ARCHITECTURE-SPINE.md`): the trial files stay inside the allow-list (L, R, A, X, Y, hat up/down, left stick). The measured values feed the profile and the future firmware scheduler; the firmware mask is not touched.
- **AD-10**: trial files use the canonical state (`ly 0x00` up, hat 8 neutral); only the sender's `encode()` knows the 2wiCC layout.
- The 60 Hz grid here is the 2wiCC timer's, not our scheduler (Story 2.4 and 2.5 and Epic 7 measure ours). Say so in the results (AC 3).
- `docs/test-plan.md` is the single place for test results; `docs/ocarina-input-mechanics.md` holds the measured facts; `docs/link-protocol.md` is not touched.

### Files to touch

| File | Action | Notes |
|---|---|---|
| `spikes/gen_trials_1_4.py` | NEW | generator, standard library only, reuses the 1.3 guard |
| `spikes/test_gen_trials_1_4.py` | NEW | tests, no hardware |
| `spikes/samples/story-1-4/*.txt` | NEW | generated, committed: `u7-minpress-*`, `u2-gap-*`, `u5-lead-{dup,ddown,sup,sdown}-*`; `jitter-120bpm-16ths` and `confirm-*` after Round 1 |
| `spikes/listen_trial.py` | UPDATE | `--mode`, new analysers, `--backend sounddevice`, `--selftest-tone` |
| `spikes/test_listen_trial.py` | UPDATE | synthetic-audio tests, keep the 292-test run green |
| `spikes/count_frames.py` | NEW | U10 frame counting |
| `spikes/test_count_frames.py` | NEW | synthetic frame stacks |
| `spikes/send_2wicc.py` | UNCHANGED | only if proven necessary, see Task 4 |
| `spikes/README.md` | UPDATE | index, "Story 1.4 trials", OQ-3 (Part B) |
| `docs/test-plan.md` | UPDATE (Part B only) | rows 2.1 to 2.5, "Story 1.4 field trials" |
| `docs/ocarina-input-mechanics.md` | UPDATE (Part B only) | U2, U5, U7, U10 |

`spikes/app-egui/` and everything outside `spikes/`, `docs/` and `_bmad-output/` stay untouched.

### Where results go (Story 1.5 reads these)

| What | Where | Form |
|---|---|---|
| Rows 2.1, 2.2, 2.3 result (one line each) | `docs/test-plan.md`, Spike 2 table, Result cell (add the column like Spike 1) | smallest 20/20 value in frames and ms; for 2.3 per modifier |
| Per-step data | `docs/test-plan.md` "Story 1.4 field trials" | table per row: step (frames, ms), passes of 20, controls, notes on partial results |
| Row 2.4 | same section | sigma, p95, max, drift per run; schedule quantisation; backend; the sentence about 2wiCC vs our scheduler |
| Row 2.5 | same section | method (camera, fps, how counted), unique fps, run-length histogram summary |
| U2, U5, U7, U10 answers | `docs/ocarina-input-mechanics.md`, Unknowns table | the smallest passing value in frames and ms and a pointer to the test-plan row |

### Testing requirements

- Part A: all tests green without a serial port, microphone, ffmpeg or console; no real `sleep` in tests. `uv run --with pytest --with pyserial --with numpy pytest spikes/` and ruff clean.
- Part B results are measurements, not tests. The only automated guard is that the files sent are what the generator promised and that the analysers recover known values from synthetic audio.

### Previous story intelligence (Story 1.3, done)

- Procedure that worked: Part A (generator, tests, committed files, dry runs, hand-over table) then a hard stop; the owner runs Part B with `listen_trial.py`; a recording agent wrote values into the test plan; the dev agent then did the docs. Repeat it; keep the "Field trial list" table.
- What went wrong and was fixed: the first live calibration reported NO SOUND on a clearly audible A5 (sub-200 Hz noise in the SNR, plus about 9 to 12 % samples lost by ffmpeg avfoundation that shifted the windows). Fixes were the 250 to 4000 Hz band-pass, a quiet-floor cap on the noise, `-thread_queue_size 4096` and the measured `scale`. Do a calibration run first, always.
- The phone tuner reading was impractical; the microphone helper replaced it. A tuner cross-check showed the game's pitch is about +27 cents sharp, so compare against the game's own pitch.
- Quirk seen three times: after a Joy-Con connect or disconnect the first injected input is ignored; the controller prompt reappears after detaching Joy-Con.
- Review lessons from 1.2 and 1.3: strict parsing, no silent clamping, no silent fallback for bad option values, tests for the signal paths; keep story files honest (historical notes marked as such); keep the results where the previous stories put them (row number findable).
- The 1.3 deviations (results in a separate "field trials" subsection, ranges instead of per-press columns) are acceptable; do the same but say where.
- Generator learnings: files are deterministic and committed; labels on every row; key printed with `--key`, not written as a file; extra safety checks (holds under the 6 s auto-stop, bad options exit 1 before any file is written).

### Git intelligence

Recent: `4020d09` (Story 1.3 trials, `listen_trial.py`, results), `19f0e6e` (2wiCC sender, Story 1.2), `1c02e55` planning follow-ups. Pattern: one branch per story (`spike/1-x-...`), samples under `spikes/samples/story-1-x/`, results in `docs/test-plan.md`, story files in this folder, no CI for Python (tests run by hand).

### Latest tech information (research, read only; nothing copied)

- ffmpeg avfoundation audio capture drops samples (ffmpeg trac #11398, mailing list thread "5% of audio samples missing when capturing audio on a mac", 2020, and a 2025 report); a proposed FIFO patch exists, not in a release the owner has. No documented `audio_buffer_size` option. So the remedy here is a different capture path (PortAudio), not ffmpeg tuning.
- Switch 2 built-in capture keeps the last 30 s (Notebookcheck, May 2025); videos and screenshots can be copied over USB (MTP) from the console's own USB-C port, not the dock (Nintendo support page 68343). Frame rate of the clip is not documented. Elgato lists Switch 2 output up to 60 fps (120 fps at 1080p or 1440p) for capture cards.
- OoT: the N64 original ran at 20 fps; the Switch 2 release is reported at 60 fps (GamesRadar headline; it does not state the logic or input rate); an earlier Switch Online N64 test measured input lag of about 1 to 2 frames after an update (Kotaku, 2022). Nothing found about the Switch 2 release's input buffering or note-release timing.
- The 2wiCC repositories: no extra timing information was found by search beyond what Story 1.2 recorded in `spikes/README.md` (60 Hz timer, 16666 us, one queued state per frame, VSYNC off by default).
- `aubio` (named in the epic as an example) is not required: the numpy detector in `listen_trial.py` is used so there is no extra build dependency; aubio may be used as an optional cross-check only.

### Project Context Rules

- No `project-context.md` exists in the repo.
- Repo content (files, comments, commit text) is English; chat with the owner is Italian.
- Never commit `_bmad/`, `bmad-*` or `gds-*` skills; only `_bmad-output/`.
- Never commit or reference anything under `ux-designs/*/imports/`, game screenshots, recorded audio or video, UF2 or other binaries.
- Do not edit `~/Documents/org/`. Do not commit or push unless the owner asks.
- Ask decisions one at a time and keep them short (owner preference); search online or upstream before raising an open question.

### Project Structure Notes

- Trial files in `spikes/samples/story-1-4/` (as `story-1-3/`); generator, analysers and their tests sit next to `send_2wicc.py`. No path conflicts.
- `spikes/recordings/` (git-ignored) holds WAVs, videos and reports.

### Open questions (all resolved: owner-approved defaults, 2026-11-21)

1. **Second observer: capture card?** Research: the console's own clip is 30 s and its fps is undocumented; a capture card gives clean digital audio and 60 fps video (Elgato lists up to 60 fps). Whether the owner has a card is not in the repo. Recommended default: no card; Mac microphone with the lossless backend for rows 2.1 to 2.4, phone slow-motion for U10. If a card exists, use it as a cross-check on row 2.4 only. **Resolved (decision: owner-approved default, 2026-11-21): no capture card; Mac microphone with the lossless backend for rows 2.1 to 2.4, phone slow-motion video for U10.**
2. **Which button for the sweeps?** Research: nothing in game docs. Recommended default: A for the full sweeps, then L, R, Y, X confirmation at the found threshold and one step below (Task 9). If the owner wants all five swept fully, it multiplies Part B time by five. **Resolved (decision: owner-approved default, 2026-11-21): full sweeps on A; confirmation on L, R, Y, X at the found threshold and one step below.**
3. **How strict is "correct pitch" for the modifier lead (U5)?** Research: audio pitch tracking cannot resolve a wrong pitch shorter than about 30 ms. Recommended default: pass = correct pitch within 50 cents from the onset (first 40 ms window); a sub-30 ms blip is accepted and the limit is written in the results. Alternative: also video the screen note icons (not planned). **Resolved (decision: owner-approved default, 2026-11-21): U5 pass = correct pitch within 50 cents in the first 40 ms; the ~30 ms audio visibility limit is stated in the results.**
4. **Definition of "retrigger" (U2).** Research: no source on how the game handles a re-press within a few frames. Recommended default: a retrigger is a second onset in the audio (dip then re-rise relative to the `g=0` and `g=30` controls); if the game restarts the note without any audible dip, it counts as retriggered only when the level rises again by the control's amount. Record the dip depth per gap so Story 1.5 can pick a margin. **Resolved (decision: owner-approved default, 2026-11-21): a retrigger is a second audio onset (dip then re-rise), thresholds calibrated from the `g=0` and `g=30` controls.**
5. **Where to put the safety margin.** The epics say 1.5 adds "a stated safety margin". Recommended default: this story records only the measured value, 20/20 and the next-lower step's pass count; Story 1.5 chooses the margin. **Resolved (decision: owner-approved default, 2026-11-21): Story 1.4 records the measured values and the pass counts around them only; Story 1.5 picks the safety margin.**
6. **Is the 2.4 jitter target "sigma <= 1 frame" still meaningful on a 60 Hz sender grid with 7.5-frame notes?** Research: the sender's rounding alternates 7 and 8 frames, so the file itself carries a quantisation of sigma about 0.5 frame. Recommended default: report sigma against the sender's frame schedule (the IOI error), and separately the quantisation, as in Task 3.5; do not judge against 1 frame without that split. **Resolved (decision: owner-approved default, 2026-11-21): report the inter-onset-interval error against the sender's actual frame schedule; report the 7/8-frame quantisation (about 0.5 frame) separately.**

## Dev Agent Record

### Agent Model Used

Claude Opus 5.5 (claude-opus-5-5), gds-dev-story, 2026-11-21. Part A only.

### Field trial list (filled by the dev agent at the end of Part A)

Run from the repo root. `<port>` is the bridge port (`uv run spikes/send_2wicc.py --list-ports`; it was `/dev/cu.usbmodem101` in Stories 1.2 and 1.3). Every `listen_trial.py` run plays the file through the unchanged sender, records, analyses and writes `<file>-<date>.wav/.csv/.md` into `spikes/recordings/` (git-ignored). Before every trial file: free play open (D-pad left on the real controller), prompts dismissed by hand, 2 s wait, real controller within reach, Ctrl+C in a real terminal if anything leaves free play.

| Order | File | Command | Notes | Duration | Result |
|---|---|---|---|---|---|
| 0 | - | (no command) | Hardware safety (Task 7.1): GND to GND about 0 ohm before powering; environment values (Task 7.2); mic position, volume, quiet room fixed for the session | - | - |
| 1 | - | `uv run spikes/listen_trial.py --selftest-tone --backend sounddevice --seconds 60` | Play a steady 1 kHz tone from a phone near the Mac mic. Report: discontinuities, delivered-sample ratio, overflows, PASS/FAIL. Must PASS before going on | 60 s | PASS (2026-11-21, see log) |
| 2 | `samples/story-1-3/calibration-A.txt` | `uv run spikes/listen_trial.py --port <port> --backend sounddevice spikes/samples/story-1-3/calibration-A.txt` | Default `pitch` mode (this 1.3 file has no TRY rows). One A5 (about 894 Hz). Report: note, cents, SNR (well above 10 dB), scale (about 1.000), offset | 4 s | PASS (2026-11-21, run 2; see log) |
| 3 | `u7-minpress-part1..4.txt` | `uv run spikes/listen_trial.py --port <port> --backend sounddevice --mode detect spikes/samples/story-1-4/u7-minpress-partN.txt` (N = 1 to 4), then `uv run spikes/listen_trial.py --combine spikes/recordings/u7-minpress-part*-??????.csv` (one report per recording) | Row 2.1. Short A blips of 1 to 10 frames, silence on blanks. Report: the combined step table (passes/20 per step, fail rounds), the smallest step line, any BLANK SOUNDED / INVALID, overflows | 4 x 117 s | DONE (2026-11-21, re-scored): U7 = 2f (33.3 ms) on every button; A 1f 18/20, 2f-10f 20/20 |
| 4 | `u2-gap-part1..4.txt` | same with `--mode retrigger` and `u2-gap-partN.txt`; then `--combine spikes/recordings/u2-gap-part*-??????.csv` (one report per recording) | Row 2.2. Two A notes per attempt (one long note for gap 0). Report per part: `dip_db`, `controls_g0_valley_max_db`, `controls_g30_valley_min_db`, `controls_separated`; combined step table and smallest step; any CONTROL WRONG | 4 x 153 s | DONE (2026-11-21, re-scored, pooled controls): U2 = 4f (66.7 ms) on A, L, Y; R needs 5f (4f 17/40, 5f 19/20); X limited by mic level (4f 18/20); dev recommendation 5f; definition-dependent |
| 5 | `u5-lead-dup-part1..4.txt` | same with `--mode lead` and `u5-lead-dup-partN.txt`; then `--combine spikes/recordings/u5-lead-dup-part*-??????.csv` (one report per recording) | Row 2.3, A with D-up (A#5). Report: combined step table, smallest lead, typical `lead_cents` of failures | 4 x 84 s | DONE (2026-11-21, re-scored): D-up U5 = 0f, 20/20 at every lead 0-5f |
| 6 | `u5-lead-ddown-part1..4.txt` | as 5 with `ddown` | A with D-down (G#5). Same values | 4 x 84 s | DONE (2026-11-21, re-scored): D-down U5 = 0f, 20/20 at every lead 0-5f |
| 7 | `u5-lead-sup-part1..4.txt` | as 5 with `sup` | A with stick up (B5, a glide). Same values | 4 x 84 s | DONE (2026-11-21, re-scored): stick-up U5 = 0f, 20/20 at every lead 0-5f |
| 8 | `u5-lead-sdown-part1..4.txt` | as 5 with `sdown` | A with stick down (G5). Same values | 4 x 84 s | DONE (2026-11-21, re-scored): stick-down U5 = 0f, 20/20 at every lead 0-5f (part 4 = run 123328; run 123150 also 5/5 every step after re-score) |
| 9 (only if a U5 family has no 20/20 step by 5f) | extension | `uv run spikes/gen_trials_1_4.py --leads 0-5,6,8,10,12,15,20,30 --mods <mod> --out-dir spikes/samples/story-1-4-ext`, then as 5 with `spikes/samples/story-1-4-ext/u5-lead-<mod>-partN.txt` | Report the same values for the extension | 4 x about 150 s | - |
| 10 | `confirm-*.txt` | `uv run spikes/gen_trials_1_4.py --set confirm --u7 <U7> --u2 <U2> --u5 dup=<k>,ddown=<k>,sup=<k>,sdown=<k>`, then each file with the mode of its row (`confirm-u7-*`: detect, `confirm-u2-*`: retrigger, `confirm-u5-*`: lead) | Task 9: L R Y X at the threshold and one step below. Report passes/20 per file | about 48 files, 1 to 2.6 min each | DONE (2026-11-21, re-scored): confirm files 20/20 except sdown-X-0f 19/20 and sup-Y-0f 19/20; U7 2f and U5 0f on every button; U2 per button in log |
| 11 | `jitter-120bpm-16ths.txt` | `uv run spikes/gen_trials_1_4.py --set jitter --press-frames <P> --min-press <U7> --min-gap <U2>`, then 3 times `uv run spikes/listen_trial.py --port <port> --backend sounddevice --mode jitter spikes/samples/story-1-4/jitter-120bpm-16ths.txt` | Row 2.4: 30 s of fast L R A Y X Y A R. Report per run: matched, pitch matches, IOI error mean/sigma/p95/max (ms and frames), latency, drift, quantisation sigma, overflows | 3 x 33 s | DONE (2026-11-21, re-scored): 3 valid runs, IOI sigma 0.48 / 0.70 / 0.70 f (8.07 / 11.67 / 11.65 ms) |
| 12 | `jitter-120bpm-16ths.txt` | once more with `--backend ffmpeg` | Task 10.3 cross-check. Report sigma and the scale warning | 33 s | DONE (2026-11-21, re-scored): ffmpeg cross-check invalid (capture delivered 78 % of real time), sigma 47.7 ms; documents capture loss |
| 13 | video | record the screen with the phone in 240 (or 120) fps slow motion for a few minutes (idle and while playing notes), copy to `spikes/recordings/`, then `uv run spikes/count_frames.py spikes/recordings/<clip>.mov` (`--fps` if the file's rate is wrong) | Row 2.5 (U10). Report: capture fps, distinct images per 1 s window (min/median/max), run-length histogram, frame period, threshold/noise, dock or handheld, video mode, phone model and slow-motion setting | a few minutes | DONE (2026-11-21): frame period 16.67 ms (60 Hz) from 240 fps video; continuous-motion caveat |

### Field trial results log

#### Trial 1: capture self-test (2026-11-21), PASS

- Command: `uv run spikes/listen_trial.py --selftest-tone --backend sounddevice --seconds 60`
- Setup: 1 kHz 70 s WAV generated with ffmpeg, played via `afplay` from the MacBook Pro speakers (volume about 50%), captured by 'MacBook Pro Microphone'.
- Reported values: tone level -19.4 dBFS; phase discontinuities 0; delivered-sample ratio 1.00000; not judged first 0.5 s (start-up grace); low-level skipped start 0.500 s, end 0.000 s; mid dropouts 0; PortAudio overflows 0; self-test PASS.
- Recording: `spikes/recordings/selftest-sounddevice-20261121-111947.wav` (git-ignored).
- History: two earlier runs FAILED due to a detector false positive (startup near-silence counted as phase discontinuities / dropout: 2 with an ffplay tone at -29.7 dBFS, 1 with afplay at -19.7 dBFS; ratio 1.00000 and 0 overflows both times). Fixed in `listen_trial.py` (low-level block masking + `STARTUP_GRACE_S` = 0.5) and re-run, which gave the PASS above.

#### Trial 2: calibration (2026-11-21), PASS (run 2; run 1 NO SOUND)

- Setup: bridge `/dev/cu.usbmodem101` probes `+ID ` -> `+2wiCC`, `+VER ` -> `+VER 2.2`, `+GQS ` -> `+GQS 0400` (no replies before the Switch was on: the controller is powered by the console). Controller on Switch 2 bottom USB-C, Apple cable. Game in ocarina free play.
- Run 1: `uv run spikes/listen_trial.py --port /dev/cu.usbmodem101 --backend sounddevice spikes/samples/story-1-3/calibration-A.txt` -> NO SOUND (-61.8 dBFS, SNR 1), offset 0.375 s, scale 1.0000. Reported cause: console had not yet accepted the injected controller (first-input-after-connect quirk). Fix: sent L+R then A (ad-hoc file, not committed); controller accepted, back in free play.
- Run 2 (same command): A5 match, 894.2 Hz, +28 cents, -41.0 dBFS, SNR 17 dB, onset 130 ms after scheduled press (includes USB, console and audio latency), offset 0.363 s, scale 1.0000. PASS.
- Recording/report: `spikes/recordings/calibration-A-20261121-112432.{wav,csv,md}` (git-ignored).

#### Trial 3a: Row 2.1 U7 min press, part 1/4 (2026-11-21), partial (rounds 1-5 of 20)

- Command: `uv run spikes/listen_trial.py --port /dev/cu.usbmodem101 --backend sounddevice --mode detect spikes/samples/story-1-4/u7-minpress-part1.txt`
- Sender: 164 rows, 7008 frames, 116.800 s, exit 0. Recorded 118.2 s, offset 0.372 s, scale 1.0000. Rounds 1-5 of 20.
- Step table (passes/tries, failing rounds): 1f 4/5 (round 1 FAIL no sound, SNR 7.2); 2f..10f all 5/5. Blank controls: 5/5 silent (ok).
- Part-1 "smallest step passing every try (and every larger step)": 2f = 33.3 ms (partial, only 5 of 20 rounds).
- Observations (raw, not conclusions): onsets mostly 114-132 ms after scheduled press; two outliers at -36 ms (n=2, press=9f round 1) and -16 ms (n=29, press=7f round 3); one PASS with note "-" (n=7, press=5f round 1, SNR 21.0). Audible length 104-732 ms, not proportional to press length (e.g. 1f presses sounded 104-214 ms).
- Reports: `spikes/recordings/u7-minpress-part1-20261121-112505.{wav,csv,md}` (git-ignored).

#### Trial 3b: Row 2.1 U7 min press, part 2/4 (2026-11-21), partial (rounds 6-10 of 20)

- Command: `uv run spikes/listen_trial.py --port /dev/cu.usbmodem101 --backend sounddevice --mode detect spikes/samples/story-1-4/u7-minpress-part2.txt`
- Sender: 164 rows, 7008 frames, 116.800 s, exit 0. Recorded 118.2 s, offset 0.368 s, scale 1.0000. Rounds 6-10 of 20.
- Step table: 1f..10f all 5/5, no failing rounds. Blank controls rounds 6-10: 5/5 silent (ok).
- Part-2 smallest step: 1f = 16.7 ms. (Only non-PASS rows were inspected individually; all scored rows PASS.)
- Reports: `spikes/recordings/u7-minpress-part2-20261121-112740.{wav,csv,md}` (git-ignored).

#### Trial 3c: Row 2.1 U7 min press, part 3/4 (2026-11-21), partial (rounds 11-15 of 20)

- Environment note: before this part the owner moved the Switch 2 next to the Mac to bring it closer to the microphone (mic position changed vs parts 1-2).
- Command: `uv run spikes/listen_trial.py --port /dev/cu.usbmodem101 --backend sounddevice --mode detect spikes/samples/story-1-4/u7-minpress-part3.txt`
- Sender: 164 rows, 7008 frames, 116.800 s, exit 0. Recorded 118.2 s, offset 0.354 s, scale 1.0000. Rounds 11-15.
- Step table: 1f 4/5 (round 15 FAIL no sound, SNR 5.6; it was the last attempt of the file, n=55 at 115.267 s); 2f..10f all 5/5. Blank controls rounds 11-15: 5/5 silent (ok).
- Part-3 smallest step: 2f = 33.3 ms.
- Reports: `spikes/recordings/u7-minpress-part3-20261121-113027.{wav,csv,md}` (git-ignored).

#### Trial 3d: Row 2.1 U7 min press, part 4/4 and combined result (2026-11-21), DONE

- Before part 4: owner moved the in-game character to a quieter location; controller re-accepted with L+R then A, then one more A (ad-hoc, not committed).
- Re-calibration: `calibration-A.txt` -> A5 match 894.2 Hz, +28 cents, -22.6 dBFS, SNR 23 dB, onset 140 ms, offset 0.367 s (report `spikes/recordings/calibration-A-20261121-113353.*`).
- Part 4/4 command: `uv run spikes/listen_trial.py --port /dev/cu.usbmodem101 --backend sounddevice --mode detect spikes/samples/story-1-4/u7-minpress-part4.txt`
- Sender: 164 rows, 7008 frames, 116.800 s, exit 0; recorded 118.2 s; offset 0.376 s; scale 1.0000. Rounds 16-20: 1f..10f all 5/5; blanks 5/5 silent (ok); part smallest step 1f. Reports: `spikes/recordings/u7-minpress-part4-20261121-113419.*` (git-ignored).
- Combined command: `uv run spikes/listen_trial.py --combine spikes/recordings/u7-minpress-part*.csv` (4 files).
- Combined step table: 1f 18/20 (fail rounds 1 and 15, both "no sound"); 2f..10f 20/20 each. Blank controls 20/20 silent. No BLANK SOUNDED, no INVALID.
- Smallest step passing every try (and every larger step): U7 = 2f = 33.3 ms.
- Result: U7 = 2 frames (33.3 ms) 20/20; 1 frame 18/20.

#### Trial 4a: Row 2.2 U2 same-button gap, part 1/4 (2026-11-21), partial (rounds 1-5 of 20)

- Command: `uv run spikes/listen_trial.py --port /dev/cu.usbmodem101 --backend sounddevice --mode retrigger spikes/samples/story-1-4/u2-gap-part1.txt`
- Sender: 294 rows, 9198 frames, 153.300 s, exit 0; recorded 154.7 s; offset 0.373 s; scale 1.0000. Rounds 1-5, hold 12f.
- Thresholds: dip_db 10.8, rise_db 10.8, threshold_source controls, controls_g0_valley_max_db 4.8, controls_g30_valley_min_db 20.3, controls_separated True.
- Controls: g=0 5/5 one onset (dips 2.8-4.8 dB); g=30 5/5 two onsets (dips 20.3-20.9 dB); all "control ok".
- Step table: 1f 0/5 (fail rounds 1-5; dips 4.4, 7.7, 5.1, 5.0, 4.5 dB); 2f 1/5 (fail rounds 1, 2, 3, 5; failing dips 8.4, 5.1, 8.6, 9.4 dB); 3f..10f 5/5. Part smallest step: 3f = 50.0 ms.
- Raw observation (as reported, no conclusion): the 2f dips (5.1-9.4 dB) sit between the g=0 controls (up to 4.8) and the threshold (10.8), so 2f is a borderline audio case.
- Reports: `spikes/recordings/u2-gap-part1-20261121-113642.*` (git-ignored).

#### Trial 4b: Row 2.2 U2 same-button gap, part 2/4 (2026-11-21), partial (rounds 6-10 of 20)

- Command: same as part 1 with `u2-gap-part2.txt` (`uv run spikes/listen_trial.py --port /dev/cu.usbmodem101 --backend sounddevice --mode retrigger spikes/samples/story-1-4/u2-gap-part2.txt`).
- Sender: 294 rows, 9198 frames, 153.300 s, exit 0; recorded 154.7 s; offset 0.363 s; scale 1.0000. Rounds 6-10.
- Thresholds: dip_db 8.1, rise_db 8.1, source controls, controls_g0_valley_max_db 2.1, controls_g30_valley_min_db 17.8, controls_separated True. All controls ok (g=0 dips 1.2-2.1 dB).
- Step table: 1f 0/5 (rounds 6-10; dips 4.7-5.5 dB); 2f 4/5 (fail round 7, dip 5.5); 3f..10f 5/5. Part smallest step: 3f = 50.0 ms.
- Per-attempt dips, parts 1+2 (from the CSVs): g=0 1.2-4.8 dB; 1f 4.4-7.7 dB (all 1 onset); 2f part 1: 5.1, 8.4, 8.6, 9.4 (FAIL), 19.2 (PASS); 2f part 2: 5.5 (FAIL), 8.9, 9.2, 9.7, 11.5 (PASS); 3f 14.1-19.6 dB (all PASS).
- CAVEAT (raw, flagged for analysis/review): the retrigger threshold is derived per part from that part's controls (10.8 dB in part 1, 8.1 dB in part 2), so 2f attempts with dips of 8.4-9.4 dB FAIL in part 1 while 8.9-9.7 dB PASS in part 2. The 2f verdict is threshold-sensitive; 3f is unambiguous (14.1 dB or more) in both parts.
- Reports: `spikes/recordings/u2-gap-part2-20261121-114351.*` (git-ignored).

#### Trial 4c: Row 2.2 U2 same-button gap, part 3/4 (2026-11-21), partial (rounds 11-15 of 20)

- Command: same as part 1 with `u2-gap-part3.txt` (`uv run spikes/listen_trial.py --port /dev/cu.usbmodem101 --backend sounddevice --mode retrigger spikes/samples/story-1-4/u2-gap-part3.txt`).
- Sender: 294 rows, 9198 frames, 153.300 s, exit 0; recorded 154.7 s; offset 0.372 s; scale 1.0000. Rounds 11-15. No CONTROL WRONG, no INVALID.
- Thresholds: dip_db 8.9, controls_g0_valley_max_db 2.9, controls_g30_valley_min_db 16.8, controls_separated True.
- Step table: 1f 0/5 (rounds 11-15); 2f 3/5 (fail rounds 11, 15); 3f 4/5 (fail round 11); 4f..10f 5/5. Part smallest step: 4f = 66.7 ms.
- Per-attempt dips: g=0 1.9-2.9 dB; 1f 5.4-7.2 (all 1 onset); 2f 6.0, 6.7 (FAIL), 9.5, 9.9, 10.9 (PASS); 3f 8.5 (FAIL, round 11), 13.4, 13.5, 16.4, 16.5 (PASS).
- Note (as reported): the 3f failure (dip 8.5 dB) is the first 3f miss across parts 1-3; it is close to this part's 8.9 dB threshold (same threshold-sensitivity caveat as part 2).
- Reports: `spikes/recordings/u2-gap-part3-20261121-114703.*` (git-ignored).

#### Trial 4d: Row 2.2 U2 same-button gap, part 4/4 and combined result (2026-11-21), DONE (provisional, threshold re-analysis pending)

- Part 4 command: same as part 1 with `u2-gap-part4.txt` (`uv run spikes/listen_trial.py --port /dev/cu.usbmodem101 --backend sounddevice --mode retrigger spikes/samples/story-1-4/u2-gap-part4.txt`).
- Sender: 294 rows, 9198 frames, 153.300 s, exit 0; recorded 154.7 s; offset 0.378 s; scale 1.0000. Rounds 16-20.
- Thresholds: dip_db 9.0, controls_g0_valley_max_db 3.0, controls_g30_valley_min_db 17.0, controls_separated True; all controls ok.
- Step table: 1f 0/5; 2f..10f 5/5; part smallest step 2f = 33.3 ms.
- Dips: g=0 1.5-3.0; 1f 5.8-7.1 (1 onset); 2f 9.4, 11.1, 11.5, 11.7, 11.8 (all PASS); 3f 13.6-16.9; 4f 16.2-17.3.
- Reports: `spikes/recordings/u2-gap-part4-20261121-114959.*` (git-ignored).
- Combined command: `uv run spikes/listen_trial.py --combine spikes/recordings/u2-gap-part*.csv` (4 files).
- Combined step table: 1f 0/20; 2f 13/20 (fail rounds 1, 2, 3, 5, 7, 11, 15); 3f 19/20 (fail round 11); 4f..10f 20/20.
- Smallest step passing every try (and every larger step): U2 = 4f = 66.7 ms, with per-part thresholds (10.8 / 8.1 / 8.9 / 9.0 dB).
- Dip ranges across all 20 rounds: g=0 controls 1.2-4.8 dB; 1f 4.4-7.7; 2f 5.1-19.2; 3f 8.5-19.6 (only one below 13.4: 8.5 in round 11); 4f 16.2 or more in part 4 (all pass in every part). g=30 controls 16.8-20.9.
- Result: U2 = 4 frames (66.7 ms) 20/20 with per-part thresholds; 3f 19/20; 2f 13/20; 1f 0/20.
- CAVEAT (kept, flagged for analysis/review): the retrigger threshold is derived per part from that part's controls (10.8, 8.1, 8.9, 9.0 dB), so the 2f verdict (and the one 3f miss at 8.5 dB) is threshold-sensitive; the result is provisional pending threshold re-analysis.

#### Trial 5-dup-a: Row 2.3 U5 modifier lead, D-up family (A#5), part 1/4 (2026-11-21), partial (rounds 1-5 of 20)

- Command: `uv run spikes/listen_trial.py --port /dev/cu.usbmodem101 --backend sounddevice --mode lead spikes/samples/story-1-4/u5-lead-dup-part1.txt`
- Sender: 149 rows, 5068 frames, 84.467 s, exit 0; recorded 85.9 s; offset 0.370 s; scale 1.0000. lead_cents threshold 50. Rounds 1-5, note length 36f.
- Step table: 0f..5f all 5/5. Part smallest lead: 0f = 0.0 ms.
- Every attempt detected A#5 with lead_cents -11 to -14 and SNR 20.2-21.1 dB (including lead 0f, i.e. modifier and button on the same frame).
- Reports: `spikes/recordings/u5-lead-dup-part1-20261121-115304.*` (git-ignored).

#### Trial 5-dup-b: Row 2.3 U5 modifier lead, D-up family (A#5), part 2/4 (2026-11-21), partial (rounds 6-10 of 20)

- Command: same as part 1 with `u5-lead-dup-part2.txt` (`uv run spikes/listen_trial.py --port /dev/cu.usbmodem101 --backend sounddevice --mode lead spikes/samples/story-1-4/u5-lead-dup-part2.txt`).
- Sender: 149 rows, 5068 frames, 84.467 s, exit 0; recorded 85.9 s; offset 0.364 s; scale 1.0000; lead_cents threshold 50. Rounds 6-10.
- Step table: 0f..5f all 5/5; part smallest lead 0f = 0.0 ms. All attempts A#5, lead_cents -10 to -13. No FAIL, no INVALID.
- Reports: `spikes/recordings/u5-lead-dup-part2-20261121-115449.*` (git-ignored).

#### Trial 5-dup-c: Row 2.3 U5 modifier lead, D-up family (A#5), part 3/4 (2026-11-21), partial (rounds 11-15 of 20)

- Command: same as part 1 with `u5-lead-dup-part3.txt` (`uv run spikes/listen_trial.py --port /dev/cu.usbmodem101 --backend sounddevice --mode lead spikes/samples/story-1-4/u5-lead-dup-part3.txt`).
- Sender: 149 rows, 5068 frames, 84.467 s, exit 0; recorded 85.9 s; offset 0.364 s; scale 1.0000; lead_cents threshold 50. Rounds 11-15.
- Step table: 0f..5f all 5/5; part smallest lead 0f. All 30 attempts A#5, lead_cents -14 to -11. No FAIL, no INVALID.
- Reports: `spikes/recordings/u5-lead-dup-part3-20261121-115629.*` (git-ignored).

#### Trial 5-dup-d: Row 2.3 U5 modifier lead, D-up family (A#5), part 4/4 and combined result (2026-11-21), D-up DONE

- Part 4 command: same as part 1 with `u5-lead-dup-part4.txt` (`uv run spikes/listen_trial.py --port /dev/cu.usbmodem101 --backend sounddevice --mode lead spikes/samples/story-1-4/u5-lead-dup-part4.txt`).
- Sender: 149 rows, 5068 frames, 84.467 s, exit 0; recorded 85.9 s; offset 0.360 s; scale 1.0000; threshold 50 cents. Rounds 16-20: 0f..5f 5/5; all 30 A#5, lead_cents -14 to -12.
- Reports: `spikes/recordings/u5-lead-dup-part4-20261121-115809.*` (git-ignored).
- Combined command: `uv run spikes/listen_trial.py --combine spikes/recordings/u5-lead-dup-part*.csv` (4 files).
- Combined step table: 0f..5f all 20/20.
- Smallest lead: D-up = 0f (0.0 ms); modifier on the same frame as the A press is enough. Across all 120 attempts the note was A#5 with lead_cents between -14 and -10. No U5 extension needed for D-up.
- Result: D-up DONE (0f, 20/20 at every lead 0-5f); ddown, sup, sdown pending.

#### Trial 5-ddown a-d: Row 2.3 U5 modifier lead, D-down family (G#5), parts 1-4 and combined result (2026-11-21), D-down DONE

- Owner approved running the 4 parts of each family back to back.
- Command per part: `uv run spikes/listen_trial.py --port /dev/cu.usbmodem101 --backend sounddevice --mode lead spikes/samples/story-1-4/u5-lead-ddown-partN.txt` (N = 1..4).
- Each part: sender 149 rows, 5068 frames, 84.467 s, exit 0; recorded 85.9 s; scale 1.0000; threshold 50 cents; offsets 0.371 / 0.363 / 0.369 / 0.370 s.
- Every part: 0f..5f 5/5, no fails, no INVALID; all 30 attempts per part G#5, lead_cents -8 to -7.
- Reports (git-ignored): `spikes/recordings/u5-lead-ddown-part1-20261121-120024.*`, `part2-20261121-120150.*`, `part3-20261121-120317.*`, `part4-20261121-120444.*` (same prefix `u5-lead-ddown-`).
- Combined command: `uv run spikes/listen_trial.py --combine spikes/recordings/u5-lead-ddown-part*.csv` (4 files).
- Combined step table: 0f..5f all 20/20. Smallest lead D-down = 0f (0.0 ms). No extension needed.
- Result: D-down DONE (0f, 20/20 at every lead 0-5f); sup, sdown pending.

#### Trial 5-sup a-d: Row 2.3 U5 modifier lead, stick-up family (B5), parts 1-4 and combined result (2026-11-21), stick-up DONE

- Command per part (run back to back): `uv run spikes/listen_trial.py --port /dev/cu.usbmodem101 --backend sounddevice --mode lead spikes/samples/story-1-4/u5-lead-sup-partN.txt` (N = 1..4).
- Each part: sender exit 0; recorded 85.8 / 85.9 / 85.9 / 85.9 s; scale 1.0000; threshold 50 cents; offsets 0.353 / 0.373 / 0.370 / 0.372 s.
- Every part: 0f..5f 5/5, no fails, no INVALID; all attempts B5; lead_cents ranges per part: -11..-3, -10..-5, -8..-5, -10..-5.
- Reports (git-ignored): `spikes/recordings/u5-lead-sup-part1-20261121-120637.*`, `part2-20261121-120804.*`, `part3-20261121-120931.*`, `part4-20261121-121057.*` (same prefix `u5-lead-sup-`).
- Combined (4 files): 0f..5f all 20/20. Smallest lead stick-up = 0f (0.0 ms). No extension needed.
- Result: stick-up DONE (0f, 20/20 at every lead 0-5f); sdown pending.

#### Trial 5-sdown a-c: Row 2.3 U5 modifier lead, stick-down family (G5), parts 1-3 of 4 (2026-11-21), partial (rounds 1-15 of 20)

- Command per part: `uv run spikes/listen_trial.py --port /dev/cu.usbmodem101 --backend sounddevice --mode lead spikes/samples/story-1-4/u5-lead-sdown-partN.txt`.
- Parts 1-3: sender exit 0; recorded 85.9 s each; scale 1.0000; threshold 50 cents; offsets 0.362 / 0.373 / 0.369 s.
- Every part: 0f..5f 5/5, no fails, no INVALID; all attempts G5; lead_cents ranges -8..-5, -7..-6, -7..-5.
- Reports (git-ignored): `spikes/recordings/u5-lead-sdown-part1-20261121-121241.*`, `part2-20261121-121408.*`, `part3-20261121-121535.*` (same prefix `u5-lead-sdown-`).
- Part 4 ABORTED (not scored): the owner put the Switch into standby during the run believing the session was over; the run hung and was stopped by the coordinator. Only a partial WAV exists (`spikes/recordings/u5-lead-sdown-part4-20261121-121702.wav`), no CSV/report; it must not be used. Part 4 (rounds 16-20) is to be re-run in a later session.
- Status: parts 1-3/4 all 15/15 per step (0f..5f), smallest lead 0f so far; part 4 pending.

#### Trial 5-sdown-d: Row 2.3 U5 stick-down family (G5), part 4, two runs, after session resume (2026-11-21), stick-down DONE

- Session resumed after console standby: controller re-accepted (probe `+ID ` -> `+2wiCC`, `+GQS ` -> `+GQS 0400`; L+R then A, then A).
- Re-calibration: A5 match 893.8 Hz, +27 cents, -27.0 dBFS, SNR 26 dB, onset 130 ms, offset 0.371 s (`spikes/recordings/calibration-A-20261121-123139.*`).
- The owner then asked the coordinator to run all remaining console trials without interruption.
- Run 1 (`u5-lead-sdown-part4.txt`, `--mode lead`): sender exit 0, recorded 85.9 s, offset 0.366 s, scale 1.0000. 0f 4/5 (round 16 FAIL "pitch at start": n=1, the first scored attempt of the file, start_cents/lead_cents -1232, onset_ms 394 vs 118-148 for the others, SNR 30.1, note G5 802.6 Hz); 1f..5f 5/5. Reports: `spikes/recordings/u5-lead-sdown-part4-20261121-123150.*`.
- Combined parts 1-3 + run 1: 0f 19/20, 1f..5f 20/20, smallest 1f.
- Reported by the coordinator: the single failure is an outlier (late onset, -1232 cents = detector reading an octave+ off, first attempt of the file), so part 4 was repeated immediately.
- Run 2: sender exit 0, recorded 85.9 s, offset 0.374 s, scale 1.0000; 0f..5f all 5/5, all G5, lead_cents -8..-5, onset 118-134 ms. Reports: `spikes/recordings/u5-lead-sdown-part4-20261121-123328.*`.
- Result: stick-down = 0f with parts 1-3 + part 4 run 2 (20/20 at every lead 0-5f); part 4 run 1 had one 0f outlier (pitch-at-start, onset 394 ms), 19/20 at 0f if counted.
- Row 2.3 U5 overall: all four modifiers (D-up, D-down, stick-up, stick-down) work at 0f lead.
- The aborted part-4 WAV (`u5-lead-sdown-part4-20261121-121702.wav`) remains unscored and unused.

#### Trial 10 / Task 9: confirmation trials on L/R/X/Y, RAW (2026-11-21), analysis pending

- Files generated: `uv run spikes/gen_trials_1_4.py --set confirm --u7 2 --u2 4 --u5 dup=0,ddown=0,sup=0,sdown=0` (32 files: confirm-u7-{L,R,X,Y}-{1f,2f}, confirm-u2-{L,R,X,Y}-{3f,4f}, confirm-u5-{dup,ddown,sup,sdown}-{L,R,X,Y}-0f).
- Each run: `uv run spikes/listen_trial.py --port /dev/cu.usbmodem101 --backend sounddevice --mode <detect|retrigger|lead> <file>`, back to back (owner absent, console left on).
- Reports (git-ignored): `spikes/recordings/confirm-*-20261121-*.*`.
- Results below are RAW as reported (passes/20); no conclusions drawn here.
- U7 detect:
  - L-1f 18 (fail r3, r16 no sound).
  - L-2f 20 but INVALID (1 BLANK SOUNDED, r2, SNR 13.9).
  - R-1f 18 (r1, r14 no sound).
  - R-2f 20.
  - X-1f 6 (no sound x12, pitch-fail G4 x2; SNR about 4.5-11).
  - X-2f 11 INVALID (1 BLANK SOUNDED r16; fails: no sound x5, pitch G4/D5 x4).
  - Y-1f 16 (r2, 8, 10, 16 no sound).
  - Y-2f 20 but INVALID (3 BLANK SOUNDED r10, 18, 20, SNR 10.6-14.1).
- U5 lead 0f:
  - dup: L 20, R 20, X 20, Y 20.
  - ddown: L 20, R 20, X 6 (mostly "no pitch", one C#6 -1196), Y 20.
  - sup: L 20, R 20, X 20, Y 7 (pitch at start F#4/C#5/C#6 with +/-711 to 1909 cents, and "no pitch").
  - sdown: L 20, R 20, X 17 (r2 no pitch, r11 C6 -1909, r17 C5 +1180), Y 20.
- U2 retrigger:
  - L-3f 18 (dip_db 12.9; g0 max 6.9, g30 min 22.0; fails dips 11.4, 10.6).
  - L-4f 20 (dip_db 12.4).
  - R-3f 5 (dip_db 16.3; g0 max 14.9; g30 min 17.7; fail dips 11.2-16.0).
  - R-4f 18 (dip_db 12.0; fails 10.7, 11.7).
  - X-3f 14 INVALID (7 CONTROL WRONG: g30 controls 1 onset, one g0 control 2 onsets; dip_db 6.0; g0 max 6.6 > g30 min 5.5).
  - X-4f 19 INVALID (4 CONTROL WRONG; dip_db 5.3).
  - Y-3f 20 (dip_db 8.3).
  - Y-4f 20 (dip_db 7.6).
- Observation (as reported): X attempts have SNR about 9-14 dB vs 18-28 on other buttons.
- Status: RUN, analysis pending (flagged items under analysis by the dev agent).

#### Trial 11-12 / Row 2.4: jitter, RAW (2026-11-21), analysis pending (suspected analysis artefact)

- File: `uv run spikes/gen_trials_1_4.py --set jitter --press-frames 3 --min-press 2 --min-gap 4` -> `jitter-120bpm-16ths.txt`, 240 notes, 1976 frames, 32.9 s (P = 3 chosen so the 4/5-frame release gaps stay at or above provisional U2 = 4).
- Command: `uv run spikes/listen_trial.py --port /dev/cu.usbmodem101 --backend <sounddevice|ffmpeg> --mode jitter spikes/samples/story-1-4/jitter-120bpm-16ths.txt`.
- Sender each run: 482 rows, 1977 frames, 32.950 s, exit 0.
- Run 1 (sounddevice): recorded 34.3 s, offset 0.354; matched 196/240, pitch matches 25/175; latency 350.7 ms; drift +6.985 ms/s; IOI err mean 2.59 ms, sigma 20.27 ms (1.216 f), p95 36.72 ms, max 39.90 ms.
- Run 2 (sounddevice): 34.3 s, offset 0.355; matched 199, pitch 33/168; latency 354.3 ms; drift +7.150; mean 1.26, sigma 21.31 (1.279 f), p95 35.69, max 39.64.
- Run 3 (sounddevice): 34.3 s, offset 0.365; matched 205, pitch 124/173; latency 87.3 ms; drift -10.223; mean -1.85, sigma 21.17 (1.270 f), p95 36.94, max 39.70.
- Run 4 (ffmpeg cross-check): warning capture delivered 78.3 % of real time, scale 0.7826, recorded 26.9 s; matched 198, pitch 30/179; latency 293.8; drift -0.322; mean 0.50, sigma 22.83 (1.370 f), p95 38.56, max 50.77.
- Schedule quantisation (7/8-frame) sigma 8.35 ms (0.501 f) in all runs. Overflows: not printed.
- Reports (git-ignored): `spikes/recordings/jitter-120bpm-16ths-20261121-132343.*`, `-132418.*`, `-132453.*`, `-132527.*`.
- Coordinator note: low pitch-match counts, latencies inconsistent with the 130-140 ms calibration onset, and the bounded about 40 ms max error suggest the onset-to-note matching is wrong; under analysis by the dev agent. No sigma conclusion is recorded.
- Subjective owner observation (not a measurement): "Owner listening impression during the runs: the played notes sounded right (by ear). Not passed to the analysis to avoid biasing it."
- Status: RUN, analysis pending.

#### Offline re-analysis of confirmation and jitter recordings, PROPOSAL (2026-11-21), SUPERSEDED by the FINAL re-scored results below

Status: SUPERSEDED (kept for history). Originally: dev-agent offline analysis, proposal; analyser code not yet changed. The raw results above stay as reported. The reported figures below are not final.

- Reported finding: the X / Y+stick-up / BLANK SOUNDED / jitter anomalies are analyser artefacts: the broadband 250-4000 Hz SNR measure sits in noise for quiet X (about D6, 1186-1190 Hz); YIN octave/subharmonic locking gave G4 = 1190/3, D5 = 1190/2 and +/-700-1909 cent "pitch at start"; jitter onset matching locked onto the wrong latency.
- U7 confirmation re-scored offline (pitch-band level + spectral-peak pitch): L 1f 18/20, 2f 20/20; R 1f 18/20, 2f 20/20; X 1f 17/20, 2f 20/20; Y 1f 16/20, 2f 20/20; no blank sounds. U7 = 2f on every button.
- U5 confirmation re-scored: all files 20/20, including X+ddown, X+sdown, Y+sup. Y+sup first 30 ms window about -50 cents: stick bend still gliding (reported as a finding, not a fail).
- Jitter re-analysed (button-band envelopes, one latency per run):
  - Run 1: matched 237/240, latency 127.3 ms, drift +0.01 ms/s, IOI sigma 8.1 ms (0.48 f), p95 18.7, max 21.8.
  - Run 2: 240/240, 138.9 ms, drift +0.22, sigma 11.7 ms (0.70 f), p95 21.0, max 22.1.
  - Run 3: 239/240, 123.3 ms, drift +0.05, sigma 11.6 ms (0.70 f), p95 20.9, max 23.0.
  - Run 4 (ffmpeg, 78 % samples): invalid, sigma 62 ms.
  - Per-note latency is bimodal, two clusters about 1 frame (12-17 ms) apart (run 3: 95 vs 75 notes); reported as likely game frame quantisation.
- U2 pooled-control re-score (fundamental band; threshold = highest gap-0 dip + min(6 dB, half distance to lowest gap-30 dip)):
  - A: threshold 11.8 dB; 1f 0/20, 2f 4/20, 3f 19/20, 4f-10f 20/20 (U2 = 4f on A).
  - L: 16.0 dB; 3f 18/20, 4f 20/20.
  - R: 13.0 dB; 3f 13/20, 4f 18/20.
  - X: 8.6 dB; 3f 20/20, 4f 20/20.
  - Y: 9.3 dB; 3f 20/20, 4f 20/20.
  - 0 controls wrong.

#### Trial 10b: R U2 console re-runs (current analyser) (2026-11-21), pending offline re-score with the pooled rule

- `confirm-u2-R-4f.txt` -> 19/20 (fail round 1, the first attempt, dip 10.2 vs threshold 10.7; g0 max 5.7, g30 min 15.8), recorded 159.0 s. Reports: `spikes/recordings/confirm-u2-R-4f-20261121-132945.*`.
- `spikes/samples/story-1-4-ext/confirm-u2-R-5f.txt` (generated with `uv run spikes/gen_trials_1_4.py --set confirm --u2 5 --buttons R --out-dir spikes/samples/story-1-4-ext`) -> 20/20 (dip_db 11.2; g0 max 5.2; g30 min 17.4). Reports: `spikes/recordings/confirm-u2-R-5f-20261121-133225.*`.
- Pending offline re-score with the pooled rule.

#### Trial 13 / Row 2.5: U10 frame rate from slow-motion video (2026-11-21), DONE with caveat

- Setup change before this trial: the owner had to handle the Switch, the controller was unplugged and re-plugged into the Switch 2 TOP USB-C port (to stand the console upright); probe `+ID ` -> `+2wiCC`, `+GQS ` -> `+GQS 0400`; re-accepted with L+R then A; back in free play. Console in handheld mode, standing; brightness raised for the video.
- Method: iPhone Slo-mo 240 fps, phone fixed, framing the Switch screen; while recording, the coordinator played `spikes/samples/story-1-4/jitter-120bpm-16ths.txt` via `uv run spikes/send_2wicc.py --port /dev/cu.usbmodem101 ...` (482 rows, 1977 frames, 32.950 s, exit 0) so the screen animated. Transfer via USB + Image Capture (original file; AirDrop did not find the Mac).
- File: `spikes/recordings/u10-framerate-IMG5861.mov` (git-ignored): h264 1280x720, r_frame_rate 240/1, 9929 frames, 41.37 s.
- Command: `uv run spikes/count_frames.py spikes/recordings/u10-framerate-IMG5861.mov`.
- Output: capture fps 239.978 (from file, assumption); changes 620; threshold 1.49 grey levels (static-part noise 0.37); distinct images per 1 s window min 0, median 15, max 33 (41 windows); run lengths (identical consecutive capture frames): 1:15, 3:14, 4:464, 8:31, 12:15, 16:10, 19:1, 20:8, 24:6, 28:3, then longer static runs up to 184; median run 4 frames -> game frame period 16.67 ms, 59.99 distinct images per second.
- Coordinator note (raw interpretation, flagged for review): the dominant run of 4 capture frames at 240 fps means that when the picture changes it changes every 16.67 ms (60 Hz); runs of 8/12 are frames where nothing moved or repeated frames. The scene was not in continuous motion for a whole second (max 33 distinct per window), so "60 fps sustained" is not directly shown; the frame period is. No result for the game's logic/input rate (audio jitter shows about 1-frame bimodal latency, consistent with 60 Hz).
- Result: DONE: frame period 16.67 ms (60 Hz) from 240 fps video; continuous-motion caveat.
- Environment details recorded here: handheld mode, standing, controller on the Switch 2 TOP USB-C port for this trial (bottom port in earlier trials). Switch system version, OoT version and video mode are still not reported.

#### Environment for all Story 1.4 field trials (2026-11-21)

Taken from the `docs/test-plan.md` "Test environment" table (owner confirmed these were already given; unchanged).

- Switch 2 system 23.5.0; OoT version 1.1.1.
- Host: macOS (Darwin 27.2).
- Bridge: `/dev/cu.usbmodem101` "2wiCC COM"; firmware 2wiCC v2.2 (controller) + 2wiCC_Comms v1.1 (bridge); Apple USB-C cable.
- Console handheld (not docked), bottom USB-C port for all trials except the U10 video (top USB-C port, console standing).
- Microphone: MacBook Pro built-in microphone; Switch 2 next to the Mac from U7 part 3 on.
- In-game location changed to a quieter spot before U7 part 4.

#### FINAL re-scored results (2026-11-21), supersede the PROPOSAL above

Analyser fixes applied and tested (478 tests pass). All 63 saved recordings re-analysed offline with their live offsets/scales; reports `spikes/recordings/*-rescored.{csv,md}` (git-ignored). The stray partial WAV `u5-lead-sdown-part4-...-121702` was skipped.

- U7 (row 2.1): A round 1: 1f 18/20 (rounds 1, 15), 2f-10f 20/20; confirm L 1f 18/20, 2f 20/20; R 18/20, 20/20; X 17/20, 20/20; Y 16/20, 20/20. No blank sounded. U7 = 2f (33.3 ms) on every button.
- U5 (row 2.3): on A, dup/ddown/sup/sdown 20/20 at every lead 0-5f (sdown part 4 = run 123328; run 123150 also 5/5 every step after re-score). U5 = 0f for all four. Confirm files 20/20 except sdown-X-0f 19/20 and sup-Y-0f 19/20 (one failure each).
- U2 (row 2.2), pooled controls per button, rule: deepest gap-0 dip + min(6 dB, half distance to shallowest gap-30 dip):
  - A: 11.8 dB -> 1f 0/20, 2f 4/20, 3f 19/20, 4f-10f 20/20 (4f = 66.7 ms).
  - L: 17.7 dB -> 3f 17/20, 4f 20/20.
  - Y: 10.8 dB -> 3f 19/20, 4f 20/20.
  - X: 10.2 dB -> 3f 12/20, 4f 18/20 (X gap-30 full dip only about 13 dB; 4f median dip 12.8 dB; reported as measurement limited by X's low level at the mic).
  - R: 16.9 dB (3f, two 4f runs, 5f) -> 3f 5/20, 4f 17/40, 5f 19/20 (one held-note control dipped 14.7 dB, raising the threshold).
  - Median dips R: 3f 13.9, 4f 16.1, 5f 20.3, g30 21.2 dB; A: 3f 16.0, 4f 19.9.
  - With a 95th-percentile held-note rule: R 3f 13/20, 4f 37/40, 5f 20/20; X 3f 16/20, 4f 18/20.
  - Dev recommendation for Story 1.5: min_gap_same_button = 5f (83 ms) to cover R. U2 is definition-dependent (dip grows smoothly with gap).
- Jitter (row 2.4):
  - Run 1: 237/240, latency 127.3 ms (sd 5.8), drift +0.011 ms/s, IOI sigma 8.07 ms (0.48 f), p95 18.8, max 21.8.
  - Run 2: 240/240, 138.9 (sd 7.5), drift +0.215, sigma 11.67 ms (0.70 f), p95 21.1, max 22.2.
  - Run 3: 239/240, 123.3 (sd 7.1), drift +0.054, sigma 11.65 ms (0.70 f), p95 20.9, max 22.9.
  - Run 4 ffmpeg: invalid (78 % of real time), sigma 47.7 ms.
  - Quantisation 8.35 ms (0.50 f) separate.
  - Per-note latency bimodal about 1 frame apart (run 3: 95 at -6 ms / 76 at +6 ms; run 1: 154 at -2 / 25 at -18); reported as jitter about +/-1 game frame; sigma of 1 frame or less met in all three valid runs.
- Table rows 3-8 and 10-12 updated to these results.

#### Remaining field trials (as of 2026-11-21)

Only optional items remain:

- X U2 re-run with the mic closer.
- Extra R gap-0 control run.

### Debug Log References

- Retrigger threshold: a midpoint between the `g=0` and `g=30` controls was too conservative in a synthetic check (a real 4-frame retrigger with an 18 dB dip read as one note, threshold 22 dB). Changed to the deepest `g=0` valley plus 6 dB, capped at half way to the `g=30` valley.
- Jitter onsets: a threshold relative to the 99th percentile alone picked noise peaks in the 2 s lead-in; the threshold is now median + 0.3 x (p99 - median). **[Superseded: the spectral-flux onset detector was replaced by per-button envelopes, see "Fixes in `listen_trial.py`" below.]**
- Field trial 1 (2026-11-21, owner run): the tone self-test reported 1 and 2 phase discontinuities with ratio 1.00000 and 0 overflows. All flagged blocks were in the first 0.01 to 0.17 s (input start-up silence, -75 to -134 dBFS, phase is noise). Fix: `tone_discontinuities` only compares blocks whose tone amplitude is within 20 dB of the median; leading/trailing low-level time is reported as skipped, a low-level stretch in the middle is reported as a dropout and fails the self-test. Tests added for both cases; the self-test must be re-run. Second live recording: 10 ms of tone, about 150 ms near-silence, then steady (macOS input start-up) read as a dropout; the first `STARTUP_GRACE_S` = 0.5 s are now never judged (printed in the self-test). Both real self-test WAVs re-analysed: 0 discontinuities, 0 dropouts.
- Confirmation and jitter analysis (2026-11-21, offline on the saved WAVs): most failures were analyser artefacts. (1) Broadband peak-vs-RMS SNR: noise alone scored 5 to 14 dB, so blanks "sounded" and quiet X (D6) notes (9 to 11 dB) did not; at the expected pitch (+-3 %) X notes are 14 to 23 dB, blanks <= 6.8 dB. (2) Short-window YIN locked onto f/2, f/3, 2f (X read G4/D5; U5 X/Y start errors of +-1170 to 1910 cents). (3) Jitter: about 385 spectral-flux onsets for 240 notes; the latency search locked onto wrong offsets (87 to 354 ms) and the per-note latency tracking made a fake drift; errors were bounded by the 40 ms match window. (4) U2: the dip depth grows smoothly with the gap (the note decays while released), so the threshold is a definition; one deep held-note control (R, 14.7 dB) raises a per-file threshold.
- Fixes in `listen_trial.py`: detect = power at the expected pitch (game tuning +27 c) vs the same band before the press (`--band-snr-db` 8), pitch from the spectral peak near the expected pitch, a strong peak elsewhere = FAIL pitch; lead = onset on the note's own pitch **[superseded: onset on the level of the whole 0.8 to 1.25 x band, `band_onset`, code review F1]**, all pitches from spectral peaks within 0.8 to 1.25 x the steady pitch; retrigger = envelope of the fundamental band (0.8 to 1.25 x), `--combine --pool-controls` re-scores several reports with one threshold from all their controls and prints the deepest held-note dips; jitter = one envelope per button pitch (20 ms), one latency per run, onsets at the -6 dB crossing with a 3 dB dominance check, per-note latency histogram. `--report-tag` keeps the live-run reports when re-analysing. Synthetic tests for each live trap (low-SNR X, octave traps with a strong 2nd harmonic, an outlier held-note control, A5/B5 close pitches with a quiet X, frame-quantised latency).
- `spikes/samples/story-1-4-ext/` (R U2 5f and 4f, inputs of recorded trials) is committed; a test regenerates it with its command and checks the allow-list. The U5 extension example now writes to `story-1-4-u5ext` so it cannot collide.
- `uvx ruff check spikes/` clean.

### Completion Notes List

- Open questions 1 to 6 resolved with the owner-approved defaults (2026-11-21), recorded in Dev Notes. Story sanity pass (no checklist run): fixed the self-test task reference (3.7 -> 7.4), clarified that the jitter file is tested in memory before it is committed (2.4) and that its release gap is the remainder of each 7/8-frame slot, not a separate `--gap-frames` value (1.10).
- `gen_trials_1_4.py`: 24 committed files in `spikes/samples/story-1-4/` (U7 4 parts, U2 4 parts, U5 4 families x 4 parts), every time in frames, `TRY`/`WARMUP` labels, seeded interleaved rounds, 5 rounds per file. `--set confirm` and `--set jitter` build the Part B files from the round 1 numbers; the extension round comes from `--leads`. Guard: every row through `gen_trials_1_3.check_allowed`.
- Dry run (`send_2wicc.py --dry-run`, exit 0, no zero-frame warning for any file): `u7-minpress-part1..4` 164 rows, 7008 frames, 116.8 s each; `u2-gap-part1..4` 294 rows, 9198 frames, 153.3 s each; `u5-lead-*-part1..4` 149 rows, 5068 frames, 84.5 s each. In memory: `jitter-120bpm-16ths` 1977 frames, 33 s (240 notes); `confirm-*` 56 to 157 s each.
- 1-frame press (u7-minpress-part1, round 1): `1415 23583.3ms +QD 000000` / `1416 23600.0ms +QD 080000 # TRY u7 A press=1f round 1/20` / `1417 23616.7ms +QD 000000 # release`: exactly one A frame.
- 1-frame gap (u2-gap-part1, round 1): `938 15633.3ms +QD 080000` / `939 15650.0ms +QD 000000 # release, gap 1f` / `940 15666.7ms +QD 080000 # second press`: exactly one neutral frame.
- `send_2wicc.py` unchanged (Task 4): nothing needed a sub-frame step.
- `listen_trial.py`: `--mode detect|retrigger|lead|jitter` (default `pitch`, unchanged), `--backend sounddevice` (optional import; `sounddevice>=0.5` added to its PEP 723 dependencies, the only new dependency), `--selftest-tone`, `--combine` (merges the step tables of the parts of one row; added so the owner does not add counts by hand).
- Interpretations to check in review: (a) **[superseded: per-button envelopes, one latency per run]** jitter onsets use the positive spectral flux as the detection function (with the 50 %-of-local-peak crossing on it) instead of the level envelope, because consecutive 16ths of different buttons may not dip in level; (b) the `lead` test uses notes that start 60 and 100 cents off (fail) and 0 and 20 cents off (pass) instead of exactly 50 cents, which sits on the 50-cent threshold; (c) `calibration-A.txt` (Task 7.4) runs in the default `pitch` mode because it has no `TRY` rows; (d) **[superseded: detect onset is now the -6 dB crossing of the level at the expected pitch, informative only]** detect onset = first 2 ms block at peak - 6 dB.
- `count_frames.py`: pure counting on a numpy frame stack (static/moving pixels split by their variance over the clip; threshold 4 x the static noise, at least 0.5 grey levels), ffmpeg decode with `-fps_mode passthrough`.
- Tests: `uv run --with pytest --with pyserial --with numpy pytest spikes/` 483 passed (gen_trials_1_4 140, listen_trial 119, count_frames 11, gen_trials_1_3 96, send_2wicc 117; the count_frames ffmpeg test runs only where ffmpeg is installed). Earlier counts in this file (434, 436, 437, 478) are historical.
- (Historical, end of Part A:) **Part A done; Part B needs the Switch, here is the list** (Field trial list above). No Part B task or AC is ticked.
- Part B done by the owner on 2026-11-21 (raw log above). Results written to `docs/test-plan.md` (Spike 2 Result column; "Story 1.4 field trials" with "Values for Story 1.5"), `docs/ocarina-input-mechanics.md` (U2, U5, U7, U10, the stick start glide) and `spikes/README.md` (OQ-3).
- Values for Story 1.5: `min_press` 2 frames (33.3 ms); `min_gap_same_button` 5 frames (83.3 ms), an owner-chosen value, not a measured limit (measured 20/20: A, L, Y at 4 frames; R 5 frames 19/20 pooled, 20/20 with its own file's threshold; X unresolved, best 4 frames 18/20; R and X not run above 5 frames); `modifier_lead` 0 frames for all four modifiers, meaning no wrong pitch longer than about 35 ms (2 frames) (a 1 to 2 frame lateness is not resolvable by audio; keep at least 1 frame of margin); frame period 16.67 ms when the picture changes (sustained 60 images per second not shown); jitter IOI sigma 0.48 to 0.70 frame.
- AC status: **AC1 met**: smallest 20/20 values recorded in frames and ms for U7 (2f), U2 (4f on A; 5f for the profile, with the definition and X limit stated) and U5 (0f for each modifier), 20 tries per step. **AC2 met with a caveat**: U10 recorded as a 16.67 ms (60 Hz) frame period from a 240 fps video (not a 60 fps capture; the AC's method was superseded by the story's own Dev Notes); the scene did not move continuously, so sustained 60 images per second is not shown. **AC3 met**: sigma recorded for 3 runs (0.48 to 0.70 frame) and compared with the 1-frame target, with the sentence about 2wiCC's queue and the 7/8-frame quantisation reported separately.
- Code review (2026-11-21, approve with minor changes), fixes applied: F1 `lead` onset now found on the whole 0.8 to 1.25 x band (before, a start 200 c off, a missing stick modifier, was skipped as "before the onset" and never failed); resolution measured on synthetic notes: a wrong start of 33 ms passes, 36 ms or more fails, i.e. about 35 ms (2 frames); U5 re-scored, results unchanged; the Y + stick-up first window is −25 to −36 c (median −29 c) with the final analyser (the −50 c was an earlier method). F2 U2 worded as an owner-chosen value. F3 the final numbers come from analysers revised after seeing these recordings (synthetic test per trap); the 1-frame U7 counts, U2 at 2 to 3 frames and jitter sigma depend on analysis definitions and should be confirmed on fresh recordings with the frozen analyser in Story 1.5 / Epic 7. F4 `--combine` refuses two reports of one recording and notes repeat runs. F5 OQ-3 wording and the `--set jitter` examples. F6 `count_frames.py` says "frame period when the picture changes" and prints a sustained-rate caveat. F7 this bookkeeping.
- Deviations during Part B: Task 9 and the later runs (confirmation, jitter, R re-runs) were run back to back by the coordinator with the owner away from the console, against the Task 7.1 rule "keep the real controller within reach"; no run left free play. Before U5 stick-down part 4 the console went into standby and the session had to be resumed (controller re-accepted, re-calibrated); one part-4 recording (`...-121702`) was aborted and is unused.
- Not done: Task 7.5 (optional capture card, none). Task 7.1: per-session wiring/GND check skipped by owner decision (wiring unchanged since the 0.1 Ω measurement; bridge probes verified at every connect). Task 7.2: handheld for all trials, so no TV video mode. Partial: Task 10.1 (41 s video). Follow-ups dropped by the owner: X U2 with the mic closer, an extra R gap-0 run, a longer continuous-motion U10 video.

### File List

- `spikes/gen_trials_1_4.py` (new)
- `spikes/test_gen_trials_1_4.py` (new)
- `spikes/samples/story-1-4/*.txt` (new: 24 round 1 files, plus the 32 `confirm-*` files and `jitter-120bpm-16ths.txt` generated during Part B)
- `spikes/samples/story-1-4-ext/confirm-u2-R-4f.txt`, `confirm-u2-R-5f.txt` (new, generated during Part B)
- `spikes/listen_trial.py` (modified)
- `spikes/test_listen_trial.py` (modified)
- `spikes/count_frames.py` (new)
- `spikes/test_count_frames.py` (new)
- `spikes/README.md` (modified)
- `docs/test-plan.md` (modified: Spike 2 results, Story 1.4 field trials)
- `docs/ocarina-input-mechanics.md` (modified: U2, U5, U7, U10, modifier timing)
- `_bmad-output/implementation-artifacts/1-4-timing-limits.md` (modified)
- `_bmad-output/implementation-artifacts/sprint-status.yaml` (modified)

## Change Log

- 2026-11-21: story created (ready-for-dev).
- 2026-11-21: open questions resolved (owner-approved defaults); Part A implemented (generator, analysers, lossless capture, frame counter, tests, README); status in-progress, Part B pending on the console.
- 2026-11-21: self-test start-up fixes; analyser fixes after the first field trials (detect/lead at the expected pitch, retrigger on the fundamental band with pooled controls, per-button jitter onsets), `--report-tag`, `story-1-4-ext` committed and tested.
- 2026-11-21: Part B results recorded (test plan, input mechanics, spikes README OQ-3); U2 = 5 frames by owner decision; status review.
- 2026-11-21: code review fixes F1 to F7 (lead onset band and resolution, U2/U5 wording, analyser-revision caveat, `--combine` duplicate check, OQ-3 and examples, count_frames caveat, bookkeeping).
- 2026-11-21: second review fixes (lead resolution stated as about 35 ms everywhere, redundant second onset call dropped, U5 single failures listed, README lead entry).
