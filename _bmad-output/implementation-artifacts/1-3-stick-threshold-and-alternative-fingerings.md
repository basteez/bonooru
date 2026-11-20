---
baseline_commit: 7097e946c9c0766afeadb807123dd5d652778c00
---

# Story 1.3: Stick threshold and alternative fingerings

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As the owner,
I want to measure the stick deflection the game needs and check every alternative fingering,
so that the mapping profile can list all usable combinations per pitch.

## Acceptance Criteria

Copied from `_bmad-output/planning-artifacts/epics.md`, Story 1.3. These are the definition of done.

1. **Given** the spike sender from Story 1.2
   **When** the owner runs Spike 1 row 1.6 with `ly = 0x40, 0x20, 0x00` (and the mirrored values for down)
   **Then** the smallest deflection that always counts as up/down is recorded (U6), with diagonals noted
2. **Given** the 45 button x modifier combinations
   **When** each one is played in free play
   **Then** its pitch is recorded and the full list goes into `docs/ocarina-input-mechanics.md`
   **And** no HYPOTHESIS marker remains on pitches or modifiers in that doc (UX Open Question 2 can be closed for the chart data)

## Tasks / Subtasks

Order matters. **Part A (tasks 1 to 4) needs no Switch and no hardware.** **Part B (tasks 5 to 7) needs the console and is run by the owner, not by the dev agent.** The dev agent does Part A, then **STOPS** and returns the trial list (Task 4.3) to the owner. The agent never plugs anything in, never opens a serial port and never claims a trial result.

### Part A: no console needed

- [x] **Task 1: Trial input generator `spikes/gen_trials_1_3.py`** (AC: #1, #2)
  - [x] 1.1 Single file, PEP 723 header (`requires-python = ">=3.10"`, no dependencies, standard library only), MIT line and "throwaway spike code" comment like `send_2wicc.py`. Writes the trial files below into `spikes/samples/story-1-3/`. Output is deterministic (same bytes on every run) and the generated files **are committed**, so the owner can run them without running the generator first. The generator exists so that a second round (bisecting the threshold, Task 7.1) is one command.
  - [x] 1.2 Common row pattern for every trial note (all values are options with these defaults, see "Trial timing"):
    1. neutral for `--lead-in` (1000 ms) at the start of the file;
    2. set the modifiers (D-pad hat and/or stick), buttons none;
    3. `--mod-lead` (500 ms) later, press the note button with the modifiers still held;
    4. hold `--hold` (1500 ms);
    5. release the button, keep the modifiers `--tail` (200 ms), then everything neutral;
    6. neutral gap `--gap` (1500 ms) before the next note.
    Each row carries a `# label` comment (for example `# A ly=0x40 up try 3/5`) so the committed files are readable. The file ends with a neutral row.
  - [x] 1.3 **U6 up sweep**, file `u6-ly-up.txt`: base button **A** (A5 = 880 Hz; stick up should give B5 = 987 Hz, stick down G5 = 783 Hz, so both directions stay inside B4 to F6), hat neutral, `lx = 0x80`, `ly` in `0x40, 0x20, 0x00` (50 %, 75 %, 100 % of full swing; wire values 0xB00, 0xC80, 0xE00), `--repeats` (default 5) presses per value. Values in the order given in the AC.
  - [x] 1.4 **U6 down sweep**, file `u6-ly-down.txt`: same, with the mirrored values `ly` in `0xC0, 0xE0, 0xFF` (wire 0x500, 0x380, 0x20C; note that 8-bit 0xFF maps to 0x20C, not 0x200, which is how the sender scales 8-bit values; the full-swing raw value 0x200 is covered by the raw sweep in 1.6).
  - [x] 1.5 **Diagonals and controls**, file `u6-diagonals.txt`, base button A, 3 presses each: (a) lx-only controls `lx = 0x00` and `lx = 0xFF` with `ly = 0x80` (expect no pitch change; if A changes, horizontal counts, which would be a finding); (b) four diagonals on the circle at 100 % radius: `26:26` (up-left), `DA:26` (up-right), `26:DA` (down-left), `DA:DA` (down-right) (0x26 and 0xDA are 0x80 -/+ 0x5A, which is 0.707 x 0x80); (c) four diagonals at the square corner: `00:00`, `FF:00`, `00:FF`, `FF:FF`. Canonical `ly 0x00` is up, so `26:26` is up-left. A diagonal "counts" if the pitch moves by the same +-2 as the straight stick.
  - [x] 1.5a **Deflection sweep (pitch vs deflection curve)**, file `u6-sweep.txt`, base button A, hat neutral, `lx = 0x80`, **one press per step**, each step held `--sweep-hold` (3000 ms, release between notes, `--gap` 1500 ms) so the tuner reading is stable (read within the first 2 s, the tone wavers after that). Steps, up direction: rest `ly = 0x80` (control, 0 %), then `0x70, 0x60, 0x50, 0x40, 0x30, 0x20, 0x10, 0x00` (step 0x10, ending at full travel; the AC values 0x40, 0x20, 0x00 are on this grid). Down direction: rest again, then `0x90, 0xA0, 0xB0, 0xC0, 0xD0, 0xE0, 0xF0, 0xFF` (the AC values 0xC0, 0xE0, 0xFF are included; 0xFF is the last canonical value, not on the 0x10 grid). The step size is an option (`--sweep-step`, default 0x10). Label every row with `ly`, wire value and percent. It does **not** replace `u6-ly-up`/`u6-ly-down` (those give 5 repeats per AC value for "always counts"); the sweep gives the curve. `u6-raw-ly` (1.6) is kept as an optional finer second round, since the sweep covers most of what it was for.
  - [x] 1.6 **Optional raw sweep**, file `u6-raw-ly.txt`, base A, 3 presses each: raw 12-bit `ly` = `800` (control, must not move), `A00`, `B00`, `C00`, `D00`, `E00` and the down side `600`, `500`, `400`, `300`, `200`. Raw form is `xxx:yyy` with three hex digits per axis, sent unchanged by the sender (`lx` stays `800`). Meant only as a finer second round if the 0x10-step sweep still leaves the threshold uncertain (Task 7.1); the generator must produce it, the owner runs it only if needed.
  - [x] 1.7 **The 45 combinations**, five files `fingerings-L.txt`, `fingerings-R.txt`, `fingerings-A.txt`, `fingerings-Y.txt`, `fingerings-X.txt`, each with the 9 modifier states below (45 total), **one press each**, in this order: none, D-up, D-down, stick-up, stick-down, D-up+stick-up, D-up+stick-down, D-down+stick-up, D-down+stick-down. Stick values are full deflection: `80:00` for up, `80:FF` for down (options `--stick-up` / `--stick-down`, defaults `0x00` / `0xFF`). Owner decision: they stay at full deflection and are regenerated only if the sweep shows something odd. Hold each note `--hold` 2500 ms in these files (long enough to read the tuner; it must stay under the ~6 s auto-stop of U1) and gap 1500 ms. Nine notes take about 45 s per file.
  - [x] 1.8 **Allow-list guard (hardware safety, AD-9 spirit).** The generator must only ever emit: buttons from `L R A X Y`; hat from `neutral (8)`, `up (0)`, `down (4)`; left stick only. Never `B` (exits free play), never D-pad left or any diagonal hat (`5`, `6`, `7` include D-pad left, which closes the ocarina), never `ZL` (opens the song menu), `ZR`, `L3`, `R3`, `+`, `-`, `Home`, `Capture`, never a right stick (it is not even encodable in the input format). It raises an error if asked for anything else.
  - [x] 1.9 Console output of the generator: for each file, the file name, the number of notes and the total duration. It also writes (or prints with `--key`) a **key**: the ordered list of notes with start times and the **expected pitch** from the model below, so the owner can follow the tuner. Expected pitch comes from the model only as a reading aid and is never written into results as if it were measured.

- [x] **Task 2: Sender label support (small, optional-if-hard, see 2.4)** (AC: #1, #2)
  - [x] 2.1 Why: the owner reads a tuner while nine different notes play. The parser currently drops everything after `#`. Add `--announce`: when a row has a `# label`, the sender prints the label to stdout when that row's first frame is **played** (wall-clock estimate `Pacer.played()`, not when it is queued, which is up to `--max-lead-frames` = 0.5 s earlier). Print with `flush=True`, one line `t=12.3s  A ly=0x40 up try 3/5`.
  - [x] 2.2 Keep the verified byte sequence untouched: `parse_lines` still returns the same `Row` states; the label is an extra field on `Row` (default empty) or a parallel list. `--announce` is off by default; without it the output and the bytes sent are identical to Story 1.2 (add a test that proves it with a recorded writer).
  - [x] 2.3 Tests in `spikes/test_send_2wicc.py`, fake clock, no sleeping: label extracted (including labels containing `#`, hex and `=`); a row without a label announces nothing; announce order and times match the schedule; `--dry-run --announce` prints labels next to the frame lines; existing tests still pass.
  - [x] 2.4 If this grows beyond the pacer's existing hooks (more than a small, well-tested change), **do not force it**: drop `--announce`, rely on the key from Task 1.9, and say so in Completion Notes. The trials work without it.

- [x] **Task 3: Tests for the generator** (AC: #1, #2)
  - [x] 3.1 `spikes/test_gen_trials_1_3.py` (pytest, no hardware), loading the generator and `send_2wicc.py` like `test_send_2wicc.py` does (importlib).
  - [x] 3.2 Every committed file under `spikes/samples/story-1-3/` parses with `send_2wicc.parse_lines`, schedules, encodes, and is byte-identical to a fresh generator run (guards drift).
  - [x] 3.3 The allow-list guard of 1.8 as a property test over every generated state: buttons subset of `L R A X Y`, hat in `{0, 4, 8}`, no raw right stick, and a state that sets a button never sets the stick to centre by accident (every note row has its modifiers still set at the press).
  - [x] 3.4 Coverage of the AC: the up sweep has `ly` 0x40, 0x20, 0x00 and the down sweep 0xC0, 0xE0, 0xFF, each `repeats` times; the five fingering files together hold exactly 45 distinct (button, hat, stick) triples; each of the 45 notes starts with the modifiers set at least `mod-lead` before the button; the expected pitches from the model give 19 distinct pitches in B4..F6 (MIDI 71..89) and each pitch has at least one fingering (the 19-pitch coverage check).
  - [x] 3.4a Sweep: `u6-sweep.txt` has one press per step, ly 0x80 (rest control), 0x70 down to 0x00 in 0x10 steps, then 0x80 (rest), 0x90 up to 0xF0 and 0xFF, includes 0x40, 0x20, 0x00, 0xC0, 0xE0, 0xFF, holds each step 3000 ms with a release in between, and `--sweep-step` changes the step.
  - [x] 3.5 Wire values: through `send_2wicc.encode`, `ly 0x40` gives the left-stick Y of 0xB00, `0x20` 0xC80, `0x00` 0xE00, `0xC0` 0x500, `0xE0` 0x380, `0xFF` 0x20C (assert on the decoded 12-bit values, not on the hex string), `lx 0x80` gives 0x800. This pins the "higher Y = up" decision (`HIGHER_Y_IS_UP`) for these files.
  - [x] 3.6 Run: `uv run --with pytest --with pyserial pytest spikes/` must pass (the 110 Story 1.2 tests included).

- [x] **Task 4: Dry runs, docs and the hand-over list** (AC: #1, #2)
  - [x] 4.1 `uv run spikes/send_2wicc.py --dry-run spikes/samples/story-1-3/<file>` runs for every generated file with exit 0. Eyeball one `QF` line from `u6-ly-up.txt` against the wire values above and put the line in the Completion Notes.
  - [x] 4.2 Update `spikes/README.md`: index entries for `gen_trials_1_3.py`, `test_gen_trials_1_3.py`, `samples/story-1-3/`; a short "Story 1.3 trials" section (file list, how to regenerate, the `--announce` option if built, the free-play entry steps and the forbidden inputs of 1.8). Update the OQ-2 row: "8-bit scaling and Y 0x200 (down) `[UNVERIFIED]`" stays until Task 5 is done. Do not rewrite the 1.2 content.
  - [x] 4.3 **Hand-over (the dev agent's last step).** Fill the "Field trial list" table in the Dev Agent Record with the exact commands (port left as `<port>`), file, number of notes, duration and the run order of Part B, which is: `u6-ly-up`, `u6-ly-down`, `u6-sweep`, `u6-diagonals`, `fingerings-L/R/A/Y/X` (`u6-raw-ly` only if needed), set the story to `in-progress` (not `review`) and **stop**. Tell the owner: "Part A done; Part B needs the Switch, here is the list". Do not mark Part B tasks or any ACs as done.

### Part B: needs the Switch, run by the owner (the dev agent stops before this)

- [x] **Task 5: Field trials on the console** (AC: #1, #2)
  - [x] 5.1 **Hardware safety first** (docs/test-plan.md "Hardware safety (before every session)"): wiring is only GP0, GP1, GND, nothing on 5V or 3V3; rewire only with both USB cables unplugged; **GND to GND continuity with a multimeter, expect about 0 ohm**, before powering anything (breadboard rails are not connected unless bridged); Switch on dock or stand, short cable on the Switch side, slack on the PC side. Additional rules for this story, all about not leaving free play by accident: keep the real controller within reach to take over; run a trial file only after the ocarina is open and the game has accepted input for 2 s; dismiss the "press A when the controller is ready" prompt by hand first (it appears after the controller sat idle; the sender never presses A on its own); stop with a **real Ctrl+C in a terminal** (the sender then sends `+SPM RT` and `+QD 000000`); never edit a trial file to add B, ZL, ZR, a left or diagonal D-pad, or the right stick.
  - [x] 5.2 Environment as in Story 1.2: macOS host, `/dev/cu.usbmodem101` (check with `--list-ports`), Apple USB-C cable, console bottom port, 2wiCC v2.2 + 2wiCC_Comms v1.1, Pro Controller Wired Communication ON, Switch 2 23.5.0, OoT 1.1.1. Write the actual values into the results (changed versions invalidate comparisons).
  - [x] 5.3 Enter free play by hand (D-pad left on the real controller), wait 2 s, hold the phone with the tuner against the console speakers **in the hand, not on the table** (resonances, Spike 1 note). Optional `uv run spikes/send_2wicc.py --dry-run <file>` first to print the key.
  - [x] 5.4 **Row 1.6, U6 up and down:** run `u6-ly-up.txt` then `u6-ly-down.txt` (`uv run spikes/send_2wicc.py --port <port> --announce spikes/samples/story-1-3/<file>`; drop `--announce` if Task 2 was dropped). For every press write the pitch name and Hz from the tuner. The pass criterion is in "Stick threshold: what to record".
  - [x] 5.5 **Diagonals:** run `u6-diagonals.txt`, record per row: pitch, and whether it equals the straight-stick result.
  - [x] 5.5a **Deflection sweep:** run `u6-sweep.txt` right after the threshold files (field trial order: `u6-ly-up`, `u6-ly-down`, `u6-sweep`, `u6-diagonals`, `fingerings-*`). Read the tuner in Hz at every step and write it down; the curve per direction (rest, threshold, full travel) is what Story 1.5 needs.
  - [x] 5.6 **The 45 combinations (new row 1.9):** run the five `fingerings-*.txt` files, one tuner reading per note. Re-enter free play by hand if a file ended it. Record name and Hz; mark "no sound", "wrong octave" or "different from expected" explicitly.
  - [x] 5.7 If any run leaves free play, the console in a menu, or a note stuck: stop with Ctrl+C, take over with the real controller, record what happened, and do not continue the file blindly.

- [x] **Task 6: Record the results** (AC: #1, #2); see "Where results go" for the exact places
  - [x] 6.1 `docs/test-plan.md`: fill the **Result** cell of row 1.6; add row 1.9; add the sections "1.6 U6 stick threshold" and "1.9 Alternative fingerings (45 combinations)" to "Spike 1 results", between 1.5 and 1.7 (1.6) and after the 1.8 / U11 block or as 1.9 in numeric order (1.9), in the existing table style. The 1.6 section also holds the **pitch-vs-deflection curve** from the sweep, one table per direction (rows: `ly` canonical, wire 12-bit, % of full travel, Hz, pitch, counted yes/no) with three marked points: **rest** (no effect), **threshold** (first step that gives the full +-2), **full travel**. Up and down are recorded **separately**; Story 1.5 decides the profile shape.
  - [x] 6.2 `docs/ocarina-input-mechanics.md`: U6 answer, a new full fingering table (all 45), and **remove or resolve every HYPOTHESIS marker on pitches and modifiers**.
  - [x] 6.3 `docs/test-plan.md` "Spike 1: ocarina mapping" -> "Done when: the mapping profile has no HYPOTHESIS markers left for pitches and modifiers": add a one-line pointer that this is met by Story 1.3 (only when it is true).
  - [x] 6.4 `spikes/README.md` OQ-2 row: replace `[UNVERIFIED]` for Y 0x200 and the 8-bit scaling with the measured result.
  - [x] 6.5 Close UX Open Question 2 for the chart data: `_bmad-output/planning-artifacts/ux-designs/ux-ocarina-bot-2026-11-20/EXPERIENCE.md` Open Questions, add "resolved by Story 1.3 (chart data); copy/screenshot check stays with Epic 11 (OQ-2 in `epics.md`)". **Approved by the owner**; do it once the results are recorded. `epics.md` stays unchanged.

- [x] **Task 7: Follow-up round if needed** (AC: #1)
  - [x] 7.1 If the smallest passing value lies between two AC values (for example 0x40 fails and 0x20 passes) or the pitch depends on how far the stick is pushed, regenerate with finer values (`gen_trials_1_3.py --ly 0x38,0x30,...`, `--sweep-step 0x08`, or the raw file of 1.6), run it, record it. If the pitch is graded by deflection (not a step), record Hz against deflection and tell Story 1.5, because `stick_value` then matters beyond a threshold.
  - [x] 7.2 Set the story to `review` only when every AC is true on recorded data, not on the model.

## Dev Notes

### What this story is

A measurement story, not a feature. The product of Part A is a set of generated input files (and a tiny sender helper); the product of Part B is numbers in docs. Nothing here is shipped, and none of it goes into `app/` or `firmware/`. The sender is throwaway spike code, MIT; 2wiCC binaries are used for testing only and are never committed.

### Trial timing (defaults, all options)

| Parameter | Default | Why |
|---|---|---|
| lead-in | 1000 ms | free play takes input only after about 2 s from opening (Spike 1.7); the owner waits 2 s by hand, the file adds 1 s plus the 0.5 s prefill |
| mod-lead | 500 ms | modifier **before** the button is verified (Spike 1.2); the minimum lead (U5) is open until Story 1.4, so use a long, safe value |
| hold | 1500 ms (U6), 2500 ms (45 combos) | long enough for a tuner reading; well under the ~6 s auto-stop (U1), and the tone wavers from about 2 s, so read the pitch in the first 2 s |
| tail | 200 ms | keep the modifier past the button release so the release is clean |
| gap | 1500 ms | the note has stopped before the next press (also avoids U2, the unknown retrigger gap) |
| repeats | 5 (U6), 1 (45 combos) | "always counts" needs more than one try; Story 1.4 uses 20 for timing, 5 is a first screen |

Frame rate stays 60 (`--frame-rate`, U10 is Story 1.4). The sender rounds ms to frames; 33 ms steps do not matter here.

### The pitch model (reading aid and expected values)

Sounding pitches of the five buttons (verified, Story 1.1): L = D5 (MIDI 74), R = F5 (77), A = A5 (81), Y = B5 (83), X = D6 (86). Modifiers add: D-pad up +1, down -1, left stick up +2, down -2, both add (so up to +-3). The nine modifier states and their offsets:

| State | Offset | State | Offset |
|---|---:|---|---:|
| none | 0 | D-up + stick-up | +3 |
| D-up | +1 | D-up + stick-down | -1 |
| D-down | -1 | D-down + stick-up | +1 |
| stick-up | +2 | D-down + stick-down | -3 |
| stick-down | -2 | | |

So the nine states give only 7 distinct offsets (-3 to +3), as the community analysis also notes (see the Sources). Expected pitch = button pitch + offset. All 45 land inside B4 (71) to F6 (89): L-3 = 71 and X+3 = 89, so nothing needs clamping. The model is a **hypothesis until Part B measures each one** (the results table has a "model" and a "measured" column; they must be filled independently). Alternatives per pitch fall out of the model (for example C6 = Y + D-up = X + stick-down = A + D-up + stick-up), and some are already seen in songs: B5 = A + stick-up and D6 = Y + D-up + stick-up (Spike 1.8, no tuner reading yet; Part B gives the tuner reading).

### Stick threshold: what to record

- Question (U6): how much `ly` deflection counts as up or down, and do diagonals count?
- For every tested value record each of the 5 presses: pitch name and Hz. Classify each press as **up/down counted** (pitch moved by exactly +2 / -2 semitones, B5 987 Hz or G5 783 Hz for base A), **not counted** (A5 880 Hz), or **other** (anything else, for example an in-between pitch: write the Hz).
- **Threshold** = the smallest value for which all repeats counted **and** every larger deflection tested also counted all repeats. State it as canonical 8-bit (what the profile's `stick_value` holds, AD-10) and as the 12-bit wire value (the dry run prints it), plus the percentage of full swing. If even 0x40 counts every time, say so and say the true minimum is below the tested range (use the raw sweep, Task 7.1, for the lower bound; the story needs only "the smallest deflection that always counts" from the AC values unless the owner wants finer).
- Check for a graded response: U4 says the stick **glides** when moved while a note sounds (by ear). With the stick set before the press the expectation is a step, but if different values give different pitches, that is a finding, not noise.
- Up and down need not match. Record both; the profile (Story 1.5) currently has one `stick_value` field, so report whether one value serves both directions (the mirrored values 0xC0, 0xE0, 0xFF are not symmetric around 0x80: 0xFF is 127 steps, 0x00 is 128).
- Diagonals: the straight-stick result is the reference. Record which diagonals still count as up/down; note that a real thumb rarely reaches the square corners (circle gate), so the circle rows (`26:26` etc.) matter more than the corner rows for later use. The lx-only rows must not move the pitch.
- Y 0x200 (down) and the 8-bit scaling were never checked on hardware (Story 1.2 tested only Y 0xE00 = up, `HIGHER_Y_IS_UP` confirmed). The down sweep closes both: if `ly 0xFF` gives the pitch dropping and 0xC0 gives a different result than 0x40 mirrored, say so.

### Where results go (Story 1.5 reads these)

| What | Where | Form |
|---|---|---|
| Row 1.6 result (one line) | `docs/test-plan.md`, Spike 1 table, row 1.6 Result cell | threshold up, threshold down, diagonals in one sentence |
| Per-press data for U6 | `docs/test-plan.md` "Spike 1 results", new "#### 1.6 U6 stick threshold" before 1.7 | table: `ly` canonical, wire 12-bit, % swing, press 1..5 pitch+Hz, verdict; diagonals table; "Result:" sentence like the other sections |
| Row 1.9 (45 combinations) | `docs/test-plan.md`, new row 1.9 in the Spike 1 table and "#### 1.9 Alternative fingerings" | table: button, modifier state, model pitch, measured pitch + Hz, match |
| U6 answer | `docs/ocarina-input-mechanics.md`, Unknowns table, row U6, "Answer" column | threshold(s) in canonical and wire units, graded or step, diagonals verdict, pointer to test-plan 1.6 |
| All fingerings per pitch | `docs/ocarina-input-mechanics.md`, new section "Alternative fingerings (all 45 combinations) [VERIFIED date]" after the chart | table pitch -> all inputs that produce it (19 rows), with the measured one marked; also the list of any combination that did not match the model |
| HYPOTHESIS cleanup | `docs/ocarina-input-mechanics.md` | see below |
| Stick value for the profile | `docs/ocarina-input-mechanics.md` U6 answer and the "Pitch modifiers" table | a single sentence "profile `stick_value` up = ..., down = ..." so Story 1.5 can copy it |
| Spike sender facts | `spikes/README.md` OQ-2 row | Y 0x200 and 8-bit scaling confirmed or not |

HYPOTHESIS markers in `docs/ocarina-input-mechanics.md` today, on pitches and modifiers, to resolve in Task 6.2:

1. Chart section: "Alternatives exist, e.g. C6 = X + stick down **[HYPOTHESIS]**, to measure in Story 1.3" -> replaced by the measured list; remove the marker and the "to measure" text.
2. Pitch modifiers section: "The count of 45 combinations and the full set of alternative fingerings are **[HYPOTHESIS]**, for Story 1.3" -> replace with the measured fact (45 combinations, 19 pitches, number of distinct offsets).
3. Pitch modifiers section: "Bends outside free play: not tested **[HYPOTHESIS]**" -> this is not in scope for the project (the bot only plays in free play) and not part of Story 1.3's measurements. Owner decision: out of scope, no extra trial. Reword to "not measured, out of scope (the bot only plays in free play)" and drop the marker.
4. Run `grep -n HYPOTHESIS docs/ocarina-input-mechanics.md` at the end; it must print nothing for pitches and modifiers. Markers in other files (for example `docs/score-format-and-mapping.md`, the live-MIDI latency row) are other stories' business: do not touch them.

### What Story 1.5 needs from here

Story 1.5 writes the default profile (`app/profiles/`, TOML shape in `docs/score-format-and-mapping.md` "Mapping profile") and every value gets a comment pointing to the spike row it came from. From this story it takes: `[modifiers] stick_value` (and whether up and down differ; the TOML has one field and a down value `ly = 0xFF`), the **list of every alternative fingering** per pitch (the fingering optimiser needs all of them; the cost weights stay in the profile), and the confirmation that the modifier semitone offsets (+-1, +-2) hold for all five buttons. Write results so each is findable by row number (1.6, 1.9).

### Developer guardrails

- **Free play is fragile.** D-pad left toggles the ocarina, B exits it, ZL opens the song menu. The generator must make these impossible to emit (Task 1.8) and the tests must prove it (Task 3.3). This is the same idea as AD-9 (the real firmware masks everything but L, R, A, X, Y, hat, left stick in PLAYING); note that the spike sender has no mask, so the generator is the only guard.
- Input format and encoder are Story 1.2's: `time buttons hat stick`, canonical stick `lx:ly` (two hex digits each, `ly 00` = up) or raw `xxx:yyy` (three hex digits each). Do **not** change `encode`, `schedule`, `Pacer` timing or the stop sequence: the byte sequence there is verified on hardware. The only permitted change in `send_2wicc.py` is the optional label/`--announce` of Task 2, with a test proving that without it the bytes are unchanged.
- The sender sends `QF` (3 digital + 6 analog bytes) whenever the left stick is not centred, `QD` otherwise (mixing in one queue was not probed on hardware, OQ-11). These trials mix them constantly (stick rows, then neutral). If mixing misbehaves (a note stuck, a dropped frame) record it as a finding; do not work around it silently.
- Pacer/queue limits are unchanged: lead at most 30 frames (cap 48), the files can be any length; there is no per-file frame limit below 1 hour.
- One file per button for the 45 combinations (about 45 s each) keeps a failed run cheap and keeps the owner's tuner readings short.
- Python only, standard library plus `pyserial` as in 1.2; tests run with `uv run --with pytest --with pyserial pytest spikes/`. Check `uvx ruff check spikes/` as in 1.2 (not a repo gate, run ad hoc).
- Everything written to the repo is English. No screenshots of the game, no UF2 or binaries (`.gitignore` already covers `*.uf2`, `*.bin`), nothing under `ux-designs/*/imports/`. Tuner screenshots, if any, stay out of git.

### Architecture compliance

- **AD-10 canonical state**: trial files use the canonical state (button bit order Y B A X L R ..., hat 0 to 7 with 8 neutral, sticks 0x80 centre, `ly 0x00` up). Only the sender's `encode()` knows the 2wiCC layout.
- **AD-9 allow-list** (`ARCHITECTURE-SPINE.md`, AD-9): the trial files stay inside it (L, R, A, X, Y, hat up/down, left stick). The measured stick value feeds the profile, and the firmware mask will let the left stick through unchanged; U11 (Story 1.1) already confirmed the allow-list as is.
- Spike sender is deliberately outside the link protocol (no COBS/CRC/credits): `docs/link-protocol.md` is not touched.
- `docs/test-plan.md` is the single place for test results; `docs/ocarina-input-mechanics.md` holds the measured facts (this story and Story 1.4 are the only ones that edit it).

### Files to touch

| File | Action | Notes |
|---|---|---|
| `spikes/gen_trials_1_3.py` | NEW | generator, standard library only |
| `spikes/test_gen_trials_1_3.py` | NEW | tests, no hardware |
| `spikes/samples/story-1-3/*.txt` | NEW | generated, committed: `u6-ly-up`, `u6-ly-down`, `u6-sweep`, `u6-diagonals`, `u6-raw-ly`, `fingerings-L/R/A/Y/X` |
| `spikes/send_2wicc.py` | UPDATE (optional, Task 2) | `--announce` and label on `Row`; bytes sent unchanged |
| `spikes/test_send_2wicc.py` | UPDATE (with Task 2) | add tests, keep the existing 110 green |
| `spikes/README.md` | UPDATE | Task 4.2 and 6.4; keep the Story 1.2 content |
| `docs/test-plan.md` | UPDATE (Part B only) | rows 1.6 and 1.9 and their sections; keep the table styles; do not renumber rows |
| `docs/ocarina-input-mechanics.md` | UPDATE (Part B only) | U6, fingering list, HYPOTHESIS cleanup |
| `_bmad-output/.../EXPERIENCE.md` | UPDATE (Part B, approved by the owner) | close OQ-2 for chart data |

`spikes/app-egui/` and everything outside `spikes/`, `docs/` and `_bmad-output/` stay untouched.

### Testing requirements

- Part A: all tests green without a serial port; no real `sleep` in tests (fake clock, as in 1.2). `uv run --with pytest --with pyserial pytest spikes/` and ruff clean.
- Part B results are recorded by hand and are **measurements, not tests**; no automated check can tell whether the game heard a note. The only automated guard is that the files sent are what the generator promised.

### Previous story intelligence (Story 1.2, done)

- Sender facts to reuse: `uv run spikes/send_2wicc.py --port <port> <file>`; `--dry-run` prints every frame; bridge port on macOS `/dev/cu.usbmodem101`; neutral sent on every handled exit; Ctrl+C must come from a real terminal; the "press A when the controller is ready" prompt appears after idle and is dismissed by hand.
- Hardware finding of 1.2: without a common ground the replies are corrupted and probes inconsistent; the GND check is in "Hardware safety" now. Do it every session.
- Open from 1.2 and closed here: `ly` down (Y 0x200) and 8-bit stick scaling `[UNVERIFIED]`; `QD`/`QF` mixing in one queue not probed (these trials exercise it).
- Review lessons from 1.2: strict hex parsing, no silent clamping, tests for the signal paths. Keep new parsing strict; do not add silent fallbacks for bad option values.
- The scale file `spikes/samples/scale-lrayx.txt` proved the free-play pitches D5 F5 A5 B5 D6 through the sender with a tuner; trial rows use the same press shape, plus modifiers.
- Spike 1 notes that matter here: modifiers held **before** the button; D-pad steps at once, stick glides (U4); song recognition is by pitch (1.8); tuner hand-held, not on a table; the tuner can lock on a subharmonic during a pitch change (1.5: 311 Hz instead of 932 Hz): hold the note and re-read.

### Git intelligence

Recent: `19f0e6e` added the 2wiCC sender (Story 1.2: `spikes/send_2wicc.py`, 110 tests, samples, README, test-plan results), `0e756d5` applied Spike 1 findings to the planning docs, Spike 1 by hand before that. Pattern: one branch per story (`spike/1-x-...`), samples in `spikes/samples/`, results in `docs/test-plan.md`, story files in this folder. No CI for Python; tests are run by hand.

### Latest tech information

- Nothing new beyond Story 1.2: `uv` runs PEP 723 scripts; `pyserial` 3.5. The generator needs no dependency.
- Upstream (read only, no code copied): the community analysis `jiaqhuan/zelda-ocarina-switch2` `docs/technical-analysis.md` counts 45 inputs (5 buttons x 3 D-pad states x 3 stick states), says the nine modifier combinations give 7 distinct offsets, and lists only the easiest input per pitch; it does **not** state a stick threshold, a dead zone or diagonal behaviour. A web search (2026-11-20) for the in-game stick dead zone found nothing either. So U6 is measured here, not looked up. The same analysis points to the "LoZ Ocarina Note Charts" project as listing every alternative input per pitch; it can serve as a cross-check of the measured list, never as a replacement (the game is the reference).

### Project Context Rules

- No `project-context.md` exists in the repo.
- Repo content (files, comments, commit text) is in English; chat with the owner is in Italian.
- Never commit `_bmad/`, `bmad-*` or `gds-*` skills; only `_bmad-output/`.
- Never commit or reference anything under `ux-designs/*/imports/`, game screenshots, UF2 or other binaries.
- Do not edit `~/Documents/org/`. Do not commit or push unless the owner asks.
- Ask decisions one at a time and keep them short (owner preference); search online or upstream before raising an open question.

### Project Structure Notes

- Generated trial files live in a subfolder `spikes/samples/story-1-3/` (the 1.2 samples are two files directly in `spikes/samples/`; nine more files would bury them). No conflict with existing paths.
- Generator and its test sit next to `send_2wicc.py`, as the 1.2 files do.

### Open questions

All resolved by the owner on 2026-11-20:

1. Stick test base and units: base A, canonical 8-bit values plus raw file for finer rounds. Resolved.
2. "Bends outside free play" marker: out of scope; reword to "not measured, out of scope (the bot only plays in free play)", drop the marker, no extra trial. Resolved.
3. Closing UX Open Question 2 in `EXPERIENCE.md`: approved, done in Task 6.5 after the results are recorded; `epics.md` unchanged. Resolved.
4. Stick value: the intermediate deflection response is measured, not only a threshold (sweep file `u6-sweep.txt`, Tasks 1.5a, 5.5a, 6.1). The 45 combinations stay at full deflection (0x00 / 0xFF) and are regenerated only if the sweep shows something odd. Resolved.
5. Profile asymmetry (one `stick_value` field vs mirrored values not symmetric around 0x80): record up and down threshold and curve separately; Story 1.5 decides the profile shape. Resolved.

### References

- [Source: `_bmad-output/planning-artifacts/epics.md`#Story 1.3] acceptance criteria; #Story 1.5 consumer; #Story 1.4 timing (U5 modifier lead, U2 retrigger gap: not measured here); UX Open Question 2
- [Source: `_bmad-output/implementation-artifacts/1-2-spike-sender-for-the-2wicc-queue.md`] sender design, hardware findings, open items
- [Source: `spikes/README.md`#Input file format, #State encoding, #How to run, #Open questions] OQ-2 stick Y
- [Source: `spikes/send_2wicc.py`] `parse_lines`, `stick_axis_to_12` (8-bit to 12-bit, `HIGHER_Y_IS_UP = True`), `encode` (`QD`/`QF`), `Pacer`, `play`
- [Source: `docs/test-plan.md`#Hardware safety, #Spike 1: ocarina mapping (row 1.6), #Spike 1 results, #Spike sender check (Story 1.2)]
- [Source: `docs/ocarina-input-mechanics.md`#Pitch modifiers, #Fingering chart, #Unknowns U1 to U11 (U6 open)]
- [Source: `docs/score-format-and-mapping.md`#Mapping profile] the TOML shape Story 1.5 fills
- [Source: `_bmad-output/planning-artifacts/architecture/architecture-ocarina-bot-2026-11-19/ARCHITECTURE-SPINE.md`#AD-9, #AD-10]
- External, read only: https://github.com/jiaqhuan/zelda-ocarina-switch2 (`docs/technical-analysis.md`)

## Dev Agent Record

### Agent Model Used

Claude Opus 5.5 (claude-opus-5-5), gds-dev-story, Part A only.

### Field trial list (filled by the dev agent at the end of Part A)

| Order | File | Command | Notes | Duration |
|---|---|---|---|---|
| 0 | `calibration-A.txt` | `uv run spikes/listen_trial.py --port <port> spikes/samples/story-1-3/calibration-A.txt` | 1 (A, model A5); Session check: verdict `match`, A5 near 880 Hz, SNR well above 10 dB, onset a few tens of ms. If not: fix mic/device/volume before the trials | 4 s |
| 1 | `u6-ly-up.txt` | `uv run spikes/listen_trial.py --port <port> spikes/samples/story-1-3/u6-ly-up.txt` | 15 (A, ly 0x40/0x20/0x00 x5); Row 1.6 up: counted = B5 987.8 Hz, not counted = A5 880 Hz | 55 s |
| 2 | `u6-ly-down.txt` | `uv run spikes/listen_trial.py --port <port> spikes/samples/story-1-3/u6-ly-down.txt` | 15 (A, ly 0xC0/0xE0/0xFF x5); Row 1.6 down: counted = G5 784.0 Hz. Also closes Y 0x200 / 8-bit scaling (OQ-2) | 55 s |
| 3 | `u6-sweep.txt` | `uv run spikes/listen_trial.py --port <port> spikes/samples/story-1-3/u6-sweep.txt` | 18 (one per step); Curve: Hz per step; mark rest, threshold, full travel per direction | 93 s |
| 4 | `u6-diagonals.txt` | `uv run spikes/listen_trial.py --port <port> spikes/samples/story-1-3/u6-diagonals.txt` | 30 (10 positions x3); lx-only rows must stay A5 (`match`); diagonals `counted` or not | 111 s |
| 5 | `fingerings-L.txt` | `uv run spikes/listen_trial.py --port <port> spikes/samples/story-1-3/fingerings-L.txt` | 9; Row 1.9: Hz + note per press; `OTHER` / `NO SOUND` are findings | 42 s |
| 6 | `fingerings-R.txt` | `uv run spikes/listen_trial.py --port <port> spikes/samples/story-1-3/fingerings-R.txt` | 9; as above | 42 s |
| 7 | `fingerings-A.txt` | `uv run spikes/listen_trial.py --port <port> spikes/samples/story-1-3/fingerings-A.txt` | 9; as above | 42 s |
| 8 | `fingerings-Y.txt` | `uv run spikes/listen_trial.py --port <port> spikes/samples/story-1-3/fingerings-Y.txt` | 9; as above | 42 s |
| 9 | `fingerings-X.txt` | `uv run spikes/listen_trial.py --port <port> spikes/samples/story-1-3/fingerings-X.txt` | 9; as above | 42 s |
| (10, only if needed) | `u6-raw-ly.txt` | `uv run spikes/listen_trial.py --port <port> spikes/samples/story-1-3/u6-raw-ly.txt` | 33 (11 raw values x3); Task 7.1 finer round: raw 800 control must stay A5 | 122 s |

Each run records the Mac microphone (game audio from the speakers), plays the file through the unchanged sender (`--announce` added by the helper) and prints a per-press table (Hz, note, cents, dBFS, SNR, onset, verdict); CSV and Markdown reports land next to the WAV in the git-ignored `spikes/recordings/`. Copy the measured values into `docs/test-plan.md` by hand. Fallback without the helper: `uv run spikes/send_2wicc.py --port <port> --announce spikes/samples/story-1-3/<file>` with a hand-held tuner. Re-analyse a recording offline with the command the live run prints (`--analyse <wav> <trial> --offset <s>`).

Expected values in the table above (A5 near 880 Hz, B5 987.8 Hz, G5 784.0 Hz) are nominal equal-temperament pitches (A4 = 440 Hz) from the hand-over and are kept as a historical record. They are superseded by the measured tuning: the game sounds about +27 to +29 c sharp (A5 about 895 Hz, A bent up to B5 about 1002 Hz, bent down to G5 about 797 Hz); see `docs/test-plan.md`, "Story 1.3 field trials".

Before each file: free play open (D-pad left on the real controller), prompt dismissed by hand, 2 s wait, room quiet. Optional preview: `uv run spikes/send_2wicc.py --dry-run --announce spikes/samples/story-1-3/<file>`; key with press times and model pitches: `uv run spikes/gen_trials_1_3.py --key`. Total for the 9 required files about 8 min of playback.

### Debug Log References

- `uv run --with pytest --with pyserial --with numpy pytest spikes/`: 292 passed (sender 117 = 110 Story 1.2 + 7 new; generator 96; listen_trial 79). Without `--with numpy` the listen_trial tests are skipped. `uvx ruff check spikes/`: clean.
- `listen_trial.py` live mode checked once end to end with a fake ffmpeg (a script producing an 880 Hz tone) and a fake sender (announce lines on time) in the scratchpad: offset 0.342 s against a true ~0.33 s, onset -10 ms, verdict `match`. No real microphone, port or console was used.
- YIN fallback fix: in noise (about 10 dB SNR at F6) no dip fell under the 0.15 threshold and the global minimum sat on 3x the period (466 Hz); the fallback now takes the first local dip within 0.1 of the best one. Checked on all 19 pitches, sine and harmonic, 5 noise seeds: 0 errors.
- One test fix during development: the model gives four C6 fingerings, not three (Y + D-down + stick-up also gives 84); the story's list was "for example", the test now asserts all four.

### Completion Notes List

- Part A (at the time of the hand-over, superseded by the Part B notes below): Tasks 1 to 4 done; Part B (Tasks 5 to 7) needed the Switch and had not started, so no trial had been run, no result recorded, `docs/test-plan.md` and `docs/ocarina-input-mechanics.md` were untouched, and the story stayed `in-progress`.
- Generator `spikes/gen_trials_1_3.py`: standard library only, deterministic, 11 files (calibration, 9 required, optional `u6-raw-ly.txt`). Every row carries a `# label`; press rows start with `PRESS`, and fingering press labels carry the model pitch as `model X` (reading aid; never "measured"). The key is printed with `--key` (not written as a file, so every file in the samples folder is a valid sender input). Threshold values are set with `--ly-up` / `--ly-down` (comma-separated hex) rather than a single `--ly`, because up and down are separate files. Extra safety beyond the story: holds must be under 6000 ms (U1 auto-stop), raw stick values must stay in 0x200..0xE00, `--mod-lead` and `--gap` must be > 0, the up/down values must be on the right side of 0x80; bad options exit 1 before any file is written.
- Allow-list guard: `check_allowed()` runs on every `Note` construction (only L R A X Y, hat 8/0/4, left stick in range); the tests also check every parsed state and every scheduled frame of every committed file.
- `--announce` implemented (Task 2): `Row.label` (default empty, text after the first `#`, further `#` kept), `announcements()`, an optional `(frame, label)` list on `Pacer`/`play` printed via `print(..., flush=True)` once the host-clock estimate says the frame has played. Off by default; tests prove the bytes are identical with and without it (`play` with a recorder and `main` with a fake serial port and fake clock), and the plain dry-run output is unchanged. `encode`, `schedule`, pacing and the stop sequence are untouched.
- Dry runs: all 10 files exit 0 (`u6-ly-up`/`u6-ly-down` 3301 frames 55.0 s, `u6-sweep` 5587 frames 93.1 s, `u6-diagonals` 6631 frames 110.5 s, `fingerings-*` 2509 frames 41.8 s each, `u6-raw-ly` 7297 frames 121.6 s).
- Eyeballed `QF` line from `u6-ly-up.txt` (frame 90, first press): `+QF 0800000008B0000880` = A (byte 1 0x08), left stick bytes `00 08 B0` = X 0x800, Y 0xB00 (ly 0x40, 50 % up), right stick `00 08 80` neutral. Matches the wire table.
- README: index rows, an "Input file format" note about labels, `--announce` in the options line, a new "Story 1.3 trials" section at the end (files, regenerate, run steps, forbidden inputs), OQ-2 row still `[UNVERIFIED]` with a pointer to the files that will close it.
- Part B helper `spikes/listen_trial.py` (owner request, approved): records the Mac microphone with ffmpeg (avfoundation, mono 48 kHz, device by name or index), runs the unchanged sender with `--announce`, aligns the recording to the trial file's own schedule (offset from the announce lines matched to the labelled rows and the audio chunk arrival times), analyses each PRESS row (YIN 300 to 2000 Hz, median Hz, note, cents, dBFS, SNR, onset, verdict counted / not counted / match / OTHER / NO SOUND), writes CSV and Markdown next to the WAV in the git-ignored `spikes/recordings/`. `--analyse WAV TRIAL --offset S` re-analyses offline. Ctrl+C: the sender gets the terminal SIGINT itself (same process group) and sends its stop; ffmpeg runs in its own session and is stopped by the helper after the sender, so the WAV is complete.
- First live calibration (3 recordings, analysed offline only): the A5 was present (about 894 Hz, +28 cents, -40/-34 dBFS) but reported NO SOUND. Root cause: broadband SNR dominated by sub-200 Hz noise (7-8 dB < 10 dB threshold) plus about 9 % of samples lost by the ffmpeg avfoundation capture in 1024-sample packets, which made the windows drift late so the pre-press noise window contained the note. Fixed as in the Change Log; the two note recordings now give A5 894.6 / 894.1 Hz, `match`, SNR 22 / 28 dB; the Switch-off recording stays NO SOUND.
- Generator: added `calibration-A.txt` (one A press, 2.5 s, model A5) for a 4 s mic and alignment check before a session.
- Part B (2026-11-20): field trials run by the owner with `listen_trial.py`; results recorded in `docs/test-plan.md` "Story 1.3 field trials" by a recording agent (values not changed by the dev agent). Dev agent then did Task 6 and the consistency pass:
  - `docs/test-plan.md`: row 1.6 and 1.9 Result cells filled (were "in progress"), "Done when" pointer (6.3), pointer from "Spike 1 results" to the field-trials section, "work in progress" / "not run yet" / "OQ-2 not closed yet" lines finalised, a "Result (1.6)" and "Result (1.9)" sentence added. Measured values untouched.
  - `docs/ocarina-input-mechanics.md`: measured Hz per button, game tuning (+27 to +29 c, A5 about 894 to 896 Hz), stick = graded bend, 45-combination fact, X D-pad about ±115 c, U6 answer with profile `stick_value` (up `ly 0x00`, down `ly 0xFF`, `lx` 0x80), new "Alternative fingerings (all 45 combinations)" table (19 rows, generated from the test-plan tables and checked against the model: 45 inputs, 19 pitches, chart fingering bold), first-input-ignored observation. `grep HYPOTHESIS` on the file prints nothing.
  - `spikes/README.md` OQ-2: confirmed (down via wire 0x20C, 8-bit scaling linear); raw Y 0x200 itself not sent.
  - `EXPERIENCE.md` Open Question 2: alternative fingerings resolved by Story 1.3 (chart data); copy/screenshot check stays with Epic 11. `epics.md` unchanged.
- Task 7.1 decision: no finer round. The stick is a graded bend (no threshold to bisect); the 0x10-step sweep already gives the pitch vs deflection curve per direction, and the profile uses full travel. `u6-raw-ly.txt` was not run.
- Deviations from the story's "Where results go", left as recorded (structure only, no data change): the 1.6 and 1.9 sections live in a separate "Story 1.3 field trials" subsection of Spike 1 (after U11) instead of inside "Spike 1 results" between 1.5 and 1.7, with a pointer added; the 1.6 per-value tables give the Hz range of the 5 presses instead of one column per press (repeatability ±1 Hz stated); the environment line records Switch, OoT, port and 2wiCC v2.2 but not the 2wiCC_Comms version or the "Pro Controller Wired Communication" setting (unchanged from Story 1.2).
- AC #1 met: smallest deflection that always counts as ±2 = full travel (`ly 0x00` up, `ly 0xFF` down, 5/5 each; 0x40/0xC0 and 0x20/0xE0 never give the full ±2), graded response and diagonals recorded (test plan 1.6, mechanics U6).
- AC #2 met: all 45 played and recorded with Hz (test plan 1.9), full per-pitch list in `docs/ocarina-input-mechanics.md`, no HYPOTHESIS marker left on pitches or modifiers; UX Open Question 2 closed for the chart data.

### File List

- `spikes/gen_trials_1_3.py` (new)
- `spikes/test_gen_trials_1_3.py` (new)
- `spikes/samples/story-1-3/u6-ly-up.txt` (new, generated)
- `spikes/samples/story-1-3/u6-ly-down.txt` (new, generated)
- `spikes/samples/story-1-3/u6-sweep.txt` (new, generated)
- `spikes/samples/story-1-3/u6-diagonals.txt` (new, generated)
- `spikes/samples/story-1-3/u6-raw-ly.txt` (new, generated)
- `spikes/samples/story-1-3/fingerings-L.txt` (new, generated)
- `spikes/samples/story-1-3/fingerings-R.txt` (new, generated)
- `spikes/samples/story-1-3/fingerings-A.txt` (new, generated)
- `spikes/samples/story-1-3/fingerings-Y.txt` (new, generated)
- `spikes/samples/story-1-3/fingerings-X.txt` (new, generated)
- `spikes/samples/story-1-3/calibration-A.txt` (new, generated)
- `spikes/listen_trial.py` (new: mic capture + pitch detection for Part B)
- `spikes/test_listen_trial.py` (new)
- `.gitignore` (modified: `spikes/recordings/`)
- `docs/test-plan.md` (modified: Story 1.3 field trials, rows 1.6 and 1.9)
- `docs/ocarina-input-mechanics.md` (modified: U6, tuning, 45 fingerings, HYPOTHESIS cleanup)
- `_bmad-output/planning-artifacts/ux-designs/ux-ocarina-bot-2026-11-20/EXPERIENCE.md` (modified: Open Question 2 closed for chart data)
- `spikes/send_2wicc.py` (modified: row labels, `--announce`)
- `spikes/test_send_2wicc.py` (modified: 7 tests for labels and `--announce`)
- `spikes/README.md` (modified)
- `_bmad-output/implementation-artifacts/sprint-status.yaml` (modified: story review)
- `_bmad-output/implementation-artifacts/1-3-stick-threshold-and-alternative-fingerings.md` (this file)

## Change Log

- 2026-11-20: story created (ready-for-dev).
- 2026-11-20: Part A implemented (trial generator and files, `--announce`, tests, README, field trial list); status in-progress, Part B waits for the console.
- 2026-11-20: Part B helper `listen_trial.py` (mic capture and automatic pitch per press) and `calibration-A.txt` added on the owner's request; field trial list switched to it.
- 2026-11-20: `listen_trial.py` fix after the first live calibration (NO SOUND on a clearly audible A5): analysis band-passed to 250-4000 Hz, noise reference capped by the file's quiet floor, capture sample loss measured (`scale`) and compensated, ffmpeg `-thread_queue_size 4096` and warnings surfaced, `--scale` for `--analyse`; 9 regression tests from the recordings' shape.
- 2026-11-20: Part B recorded (test-plan 1.6 and 1.9), Task 6 done (mechanics doc U6 + 45 fingerings + HYPOTHESIS cleanup, OQ-2 closed in spikes README and UX EXPERIENCE.md), Task 7 not needed (graded bend, sweep covers it); all ACs met; status review.
- 2026-11-20: review nits fixed: Part A completion note marked as historical, File List shows sprint-status `review`, nominal vs measured pitch note under the field trial list, missing blank line in test-plan 1.6.
