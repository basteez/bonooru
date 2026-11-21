# Ocarina Bot: Test plan

*Converted from the author's org-roam notes on 2026-11-19. This file is now the reference in the repo.*

Topic: Ocarina Bot (author's notes)

Spikes first, all with **existing** firmware (no code of ours yet). Record for every run: Switch 2 system version, game version, port/cable, firmware + version, result.

## Hardware safety (before every session)

- Wiring = RiiDraw's (ADR-001 (author's notes)): only GP0, GP1, GND; nothing on 5V or 3V3
- Rewire only with both USB cables unplugged
- Before powering anything, check GND↔GND continuity between the two boards with a multimeter (Ω, 200 range, expect ~0 Ω). On a breadboard the top and bottom power rails are **not** connected unless bridged (Story 1.2: unbridged rails gave corrupted replies and inconsistent probes)
- Switch on dock or stand, short cable on the Switch side, slack on the PC side
- Details: Risk register (author's notes)

## Test environment

| Item            | Value                                                                                                                           |
|-----------------|---------------------------------------------------------------------------------------------------------------------------------|
| Switch 2 system | 23.5.0 (Spike 1 by hand and Story 1.2 sender, 2026-11-20)                                                                       |
| OoT version     | 1.1.1                                                                                                                           |
| Controller      | Joy-Con 2 pair (Spike 1 by hand; D-pad = Joy-Con 2 (L) direction buttons)                                                       |
| Measurement     | Soundcorset tuner (iOS), A4 = 440 Hz; Switch 2 and phone hand-held (table caused resonances), mic ~2 cm from the right speaker  |
| Host OS(es)     | macOS (Darwin 27.2), Story 1.2 sender; bridge port `/dev/cu.usbmodem101`, product "2wiCC COM", VID:PID 2E8A:000A                |
| Cable(s)        | Apple USB-C ↔ USB-C (Story 1.2), console bottom USB-C port                                                                      |
| Firmware        | 2wiCC v2.2 (`2wiCC_RP2040.uf2`, controller Zero), 2wiCC_Comms v1.1 (`2wiCC_Comms.uf2`, bridge Zero); official GitHub releases, kept outside the repo (Story 1.2, 2026-11-20). Previously RiiDraw firmware |
| Recording       | Switch 2 video capture (Capture button, 30 s) or capture card; T.2 (5 min) needs a capture card or a mic on the console speaker |

## Spike 0 (optional): the Switch 2 sees an RP2040-Zero and presses A

Optional since ADR-002 (author's notes) chose P2; run it only to evaluate P1 as a later simplification.
Goal: validate profile P1 (generic HID) on Switch 2 with zero code. See [USB controller emulation](usb-controller-emulation.md).

### Setup

- Flash GP2040-CE, `WaveshareZero` build (UF2), on one Zero
- Set input mode to Switch (web configurator or boot-time button, check current GP2040-CE docs)
- Find the GPIO mapped to A in that build; a jumper wire from it to GND = "press A"

### Steps

1. Switch 2 on, home menu
2. Plug the Zero into the connection under test (matrix below)
3. System Settings → Controllers → check the controller shows up
4. Touch the jumper → the A action happens (open a menu item, or use the controller test screen if present)

### Matrix

|  ID | Connection                       | Wired Comm. setting | Recognised? | A works? | Notes |
|----:|----------------------------------|---------------------|-------------|----------|-------|
| 0.1 | console USB-C (bottom), handheld | OFF                 |             |          |       |
| 0.2 | console USB-C (bottom), handheld | ON                  |             |          |       |
| 0.3 | console USB-C (top)              | OFF                 |             |          |       |
| 0.4 | dock USB-A                       | OFF                 |             |          |       |
| 0.5 | dock USB-A                       | ON                  |             |          |       |
| 0.6 | best of above, inside OoT        | –                   |             |          |       |

### Done when

At least one row passes 0.6: in OoT the Zero's A press is accepted.

## Spike 0b: two-board chain with the Pro Controller profile

Goal: validate wiring + bridge + profile P2 (what RiiDraw uses).
- Flash 2wiCC on the controller Zero and 2wiCC_Comms on the bridge Zero (published UF2s, test use only: no licence, nothing is redistributed)
- Wire GP0↔GP1 crossed + GND; enable "Pro Controller Wired Communication"
- From a serial terminal on the bridge port, 460800 8N1: send `+QD 080000` then `+QD 000000`
- Done when: A press/release is seen on the console

### Result

- 2026-11-18: wiring + P2 already work in practice: the owner ran RiiDraw (2wiCC-based) with this assembly on their Switch 2
- Known-good setup:

  | Item            | Value                                                     |
  |-----------------|-----------------------------------------------------------|
  | Switch 2 port   | console USB-C top and bottom: both work; dock not tried   |
  | Cable           | USB-C ↔ USB-C, Apple iPhone cable (data, USB 2.0), 1 m    |
  | Board-to-board  | both Zeros on a breadboard, loose jumper wires (method A) |
  | Wired Comm.     | ON (required by RiiDraw)                                  |
  | Switch 2 system | not recorded                                              |

- Label the Apple cable and keep it for the project: it is the reference cable
- No separate OoT check: with P2 the system handles the controller, games get normalised input; spike 1 runs inside OoT anyway
- Also measure here: P2 polling interval on Switch 2 (see ADR-002)
- 2026-11-20 (Story 1.2): first direct terminal test of the bare commands, official 2wiCC v2.2 + 2wiCC_Comms v1.1, macOS, Apple USB-C cable, console bottom port, Wired Comm. ON, Switch 2 system 23.5.0, OoT 1.1.1
  - First session: the two GND pins were on breadboard power rails that were not bridged (no common ground). PC→controller commands mostly worked, but replies were corrupted (e.g. `+2wicc��`, `+GQS<<48<�`, `+GQR<>3Fe`) and the stop/queue probes were inconsistent. Fix: rails bridged, GND↔GND measured 0.1 Ω. Those earlier probe results are discarded; everything below is after the fix
  - `+QD 080000`, 1 s, `+QD 000000` on "Test input devices → buttons": A registered. **Pass**
  - Note: that screen exits when A is held for about 1 s or less, and it keeps showing the last pressed button (also with a Joy-Con), so it shows presses, not releases

## Spike 1: ocarina mapping

Uses spike 0 or 0b in real-time mode (terminal or GLaMS web UI). Rows 1.1–1.5, 1.7, 1.8 can run **first, by hand with a real controller**: no hardware needed, and they settle the riskiest unknowns (Risk register (author's notes) R1, R2).

|  ID | Check (see [unknowns](ocarina-input-mechanics.md))                   | Method                                                                                                                                               | Result |
|----:|----------------------------------------|------------------------------------------------------------------------------------------------------------------------------------------------------|--------|
| 1.1 | 5 natural notes and their pitch        | press each, tuner app on the phone / record                                                                                                          | pairing confirmed; sounds one octave above the chart (L = D5 … X = D6), see results below |
| 1.2 | all 19 fingerings of the chart         | modifier first, then button                                                                                                                          | all 19 confirmed, one octave above the chart (B4–F6 sounding); modifiers ±1 / ±2 / ±3 confirmed |
| 1.3 | U1 sustain                             | hold 1 s, 3 s, 10 s                                                                                                                                  | sustains while held; wavers from ~2 s; auto-stops at ~6 s |
| 1.4 | U3 new button while holding            | L held, then press R                                                                                                                                 | new button cuts the held note (F5 starts, D5 stops); releasing R with L held brings D5 back |
| 1.5 | U4 bend while sounding                 | hold A, then D-pad ↑                                                                                                                                 | D-pad: instant step to A♯5 (932 Hz); left stick: gradual glide (by ear) |
| 1.6 | U6 stick threshold                     | ly = 0x40, 0x20, 0x00                                                                                                                                | no threshold: graded bend, about linear in cents, ±2 semitones only at full travel (up `ly 0x00` = wire 0xE00, +199 c; down `ly 0xFF` = wire 0x20C, −197 c); 0x40/0xC0 ≈ ±105 c, 0x20/0xE0 ≈ ±160 c; X axis no effect; diagonals and corners ≈ ±142 c. See Story 1.3 field trials below |
| 1.7 | U8, U9 free play enter/exit and limits | play 100 notes in a row                                                                                                                              | enter D-pad ← (input after ~2 s), exit B or D-pad ←; 100 notes and 1 min idle: no limit, no auto-exit |
| 1.8 | official song recognised in free play? | play a song's natural notes (e.g. Zelda's Lullaby) in free play; then the same with one note on an alternative fingering (e.g. D4 = R + D↓ + stick↓) | yes; matched by pitch, not button: alternative fingerings (B, then D) still trigger it, even very slow |
| 1.9 | 45 fingerings: 5 buttons × 9 modifier states (D-pad none/up/down × stick none/up/down), alternative fingerings | one file per button, stick at full deflection, `lx` centred; pitch from `listen_trial.py` | all 45 run: 45/45 match the model (button + D-pad ±1 + stick ±2) by nearest note; 19 pitches, 1 to 4 fingerings each; on X the D-pad shifts about ±115 c (repeated, confirmed). See Story 1.3 field trials below |

Done when: the mapping profile has no `HYPOTHESIS` markers left for pitches and modifiers.
Met for the measured data by Story 1.3 (2026-11-20): `docs/ocarina-input-mechanics.md` has no `HYPOTHESIS` marker left on pitches or modifiers. The profile was written in Story 1.5 (2026-11-21): `app/profiles/oot-switch2-1.1.1.toml` (buttons, modifiers, stick bytes, range and all 45 fingerings, each with its row in a comment); no `HYPOTHESIS` marker on pitches or modifiers.

### Spike 1 results (by hand, 2026-11-20)

Setup: see Test environment (Switch 2 23.5.0, OoT 1.1.1, Joy-Con 2 pair, Soundcorset on the Switch 2 speakers). Pitches are the sounding pitch read by the tuner (Hz shown to confirm the octave). The Hz shown are the tuner's nominal note frequencies (A5 = 880 Hz), not measured values: the game's A5 measures about 894 to 896 Hz (see the Story 1.3 field trials below, tuner cross-check).
Rows 1.6 and 1.9 were measured with the spike sender, not by hand: see "Story 1.3 field trials" below.

#### 1.1 Natural notes

| Button | Hypothesis | Measured     | Match            |
|--------|------------|--------------|------------------|
| L      | D4         | D5 (587 Hz)  | pitch class yes, one octave higher |
| R      | F4         | F5 (698 Hz)  | pitch class yes, one octave higher |
| A      | A4         | A5 (880 Hz)  | pitch class yes, one octave higher |
| Y      | B4         | B5 (987 Hz)  | pitch class yes, one octave higher |
| X      | D5         | D6 (1174 Hz) | pitch class yes, one octave higher |

Result: the button → note pairing of the chart is right, but every natural note sounds **one octave higher** than written (D5 F5 A5 B5 D6). Same values in a first run with the console on a table (discarded for resonances).


#### 1.2 Chart fingerings (modifier first, then button)

Expected = chart pitch one octave up, as found in 1.1.

| Chart pitch | Input            | Expected | Measured                 | Match |
|-------------|------------------|----------|--------------------------|-------|
| B3          | L + D↓ + stick↓  | B4       | B4 (493 Hz)              | yes   |
| C4          | L + stick↓       | C5       | C5 (523 Hz)              | yes   |
| C♯4         | L + D↓           | C♯5      | C♯5 (554 Hz)             | yes   |
| D4          | L                | D5       | D5 (587 Hz), from 1.1    | yes   |
| D♯4         | L + D↑           | D♯5      | D♯5 (622 Hz)             | yes   |
| E4          | R + D↓           | E5       | E5 (659 Hz)              | yes   |
| F4          | R                | F5       | F5 (698 Hz), from 1.1    | yes   |
| F♯4         | R + D↑           | F♯5      | F♯5 (739 Hz)             | yes   |
| G4          | R + stick↑       | G5       | G5 (783 Hz)              | yes   |
| G♯4         | A + D↓           | G♯5      | G♯5 (830 Hz)             | yes   |
| A4          | A                | A5       | A5 (880 Hz), from 1.1    | yes   |
| A♯4         | A + D↑           | A♯5      | A♯5 (932 Hz)             | yes   |
| B4          | Y                | B5       | B5 (987 Hz), from 1.1    | yes   |
| C5          | Y + D↑           | C6       | C6 (1046 Hz)             | yes   |
| C♯5         | X + D↓           | C♯6      | C♯6 (1108 Hz)            | yes   |
| D5          | X                | D6       | D6 (1174 Hz), from 1.1   | yes   |
| D♯5         | X + D↑           | D♯6      | D♯6 (1244 Hz)            | yes   |
| E5          | X + stick↑       | E6       | E6 (1318 Hz)             | yes   |
| F5          | X + D↑ + stick↑  | F6       | F6 (1396 Hz)             | yes   |

Result: all 19 chart fingerings give the chart pitch one octave up; range B4–F6 sounding (B3–F5 written). Modifiers confirmed: D-pad ↑/↓ = ±1, left stick ↑/↓ = ±2, both together = ±3 semitones, held before the button.

#### 1.3 U1 sustain (A held)

| Hold     | Observed                                                                 |
|----------|--------------------------------------------------------------------------|
| 1 s      | sounds for the whole press, stops on release                             |
| ~2 s on  | the tone starts to waver slightly (like running out of breath)           |
| ≥ 6 s    | the note stops by itself at about 6 s even with the button still held    |

Result: holding sustains the note, up to a maximum of about 6 s; audible wavering from about 2 s.

#### 1.4 U3 new button while holding

| Step                          | Observed                         |
|-------------------------------|----------------------------------|
| L held (D5), then R pressed   | F5 starts, D5 stops (cut)        |
| R released, L still held      | D5 sounds again                  |

Result: the last pressed button wins; releasing it falls back to the button still held (D5 again).

#### 1.5 U4 modifier changed while a note sounds

| Step                              | Observed                                            |
|-----------------------------------|-----------------------------------------------------|
| A held (A5, 880 Hz), then D-pad ↑ | pitch jumps at once to A♯5 (932 Hz), no glide       |
| Note held, then left stick moved  | pitch changes gradually (glide), by ear; no tuner reading |

A first reading of 311 Hz (D♯4) was the tuner locking on a subharmonic (932 / 3) during the change; the re-run held both for 2 s and read 932 Hz.

Result: a D-pad change while the note sounds retunes it immediately (a step); a left-stick change glides gradually (a bend). The stick glide was observed by ear only.

#### 1.7 U8, U9 free play enter/exit and limits

| Check                     | Observed                                                              |
|---------------------------|-----------------------------------------------------------------------|
| Enter free play           | D-pad ←; the ocarina takes note input only after about 2 s            |
| Exit free play            | B or D-pad ←                                                          |
| D-pad ← binding           | fixed: always opens/closes the ocarina, not a user-assigned item slot |
| 100 notes (L R A Y X × 20, ~1/s) | no note limit, no auto-exit: free play closes only when the player exits |
| Idle 1 min, then A        | still open, A sounds at once: no idle timeout                         |

Result: enter with D-pad ← (note input accepted after ~2 s), exit with B or D-pad ←; no note limit, no auto-exit, no idle timeout seen.

#### 1.8 Official song in free play (Zelda's Lullaby: B D A B D A written)

| Run                                   | Input                     | Observed                                  |
|---------------------------------------|---------------------------|-------------------------------------------|
| Natural notes                         | Y X A Y X A               | recognised, even at a very slow tempo     |
| 1st B on an alternative fingering     | (A + stick↑) X A Y X A    | recognised                                |
| 1st D on an alternative fingering     | Y (Y + D↑ + stick↑) A Y X A | recognised                              |

Result: the game matches official songs by **pitch**, not by button: an alternative fingering of the same pitch still triggers the song. Recognition does not depend on tempo (very slow still works).

#### U11 other buttons in free play (extra check, not a numbered row)

| Input              | Observed                                                         |
|--------------------|------------------------------------------------------------------|
| ZL                 | opens the menu of known songs                                    |
| ZL + ZR            | stays on that menu, nothing else happens                         |
| ZL + A (in menu)   | plays a preview of the highlighted song                          |
| ZR                 | nothing                                                          |
| Right stick        | moves the camera                                                 |

Result: no other button changes pitch or octave; the 19 chromatic pitches of the chart are the whole range. ZL (song menu, song preview with A) and the right stick (camera) act in free play but are not note inputs, so the AD-9 queue-mode allow-list (L, R, A, X, Y, hat, left stick) holds, and masking ZL and the right stick is desirable.

### Story 1.3 field trials (2026-11-20)

All trials run on 2026-11-20. Conclusions are in `docs/ocarina-input-mechanics.md` (U6, Pitch modifiers, Alternative fingerings).

Setup: Switch 2 23.5.0, OoT 1.1.1, controller on the Switch 2 bottom USB-C. Sender [`spikes/send_2wicc.py`](../spikes/README.md) through `spikes/listen_trial.py`, bridge `/dev/cu.usbmodem101`. Trial files in `spikes/samples/story-1-3/`.

Hardware safety (checklist above) done before power-on: cables unplugged, only GP0↔GP1 crossed + GND wired, nothing on 5V/3V3; GND↔GND measured 0.1 Ω (multimeter Ω 200 range). Bridge probes: `+ID ` → `+2wiCC`, `+VER ` → `+VER 2.2`, `+GQS ` → `+GQS 0400`.

Pitch method: `spikes/listen_trial.py` with the MacBook Pro microphone (a per-press phone-tuner reading was impractical for the owner). The ffmpeg capture loses about 11 to 12 % of the samples (delivers 88 to 89 % of real time); the tool rescales the note windows (scale per run below).

Calibration (`calibration-A.txt`, report `spikes/recordings/calibration-A-20261120-205016.md`, offset 0.348, scale 0.8828): A5 = 893.8 Hz, +27 cents vs equal temperament A4 = 440 Hz, -42.5 dBFS, SNR 17, onset 125 ms, verdict match.

Tuner cross-check (resolves the earlier open caveat about the +27 c reading): calibration A played twice with mic capture while the owner read the phone tuner. Run 1 (`spikes/recordings/calibration-A-20261120-210505.md`): mic 896.6 Hz (A5 +32 c); the owner first reported "A5 880 Hz". Run 2 (`spikes/recordings/calibration-A-20261120-210544.md`): mic 894.8 Hz (A5 +29 c); the owner read the tuner's cents indicator: about +27 cents. Result: the tuner agrees with the microphone, the game's A5 is about +27 to +29 c sharp vs A4 = 440 Hz (about 894 to 896 Hz). The "880 Hz" a phone tuner shows is its nominal note frequency, not a measured value. The cents in the tables below are therefore vs equal temperament A4 = 440 Hz, and the game's pitch is a constant offset of about +27 c from it; the offsets between notes within one run do not depend on this.

Observation (seen three times): after connecting or powering on, the first injected input is ignored and the second works (D-pad right on Home: 1st no effect, 2nd moved the selection; "press A when the controller is ready" prompt: 1st A ignored, 2nd A dismissed it). Again before the Y fingering run, after the owner detached the Joy-Con: the controller prompt reappeared, the first injected A was ignored and the second dismissed it.

#### 1.6 U6 stick threshold

##### Up (`u6-ly-up.txt`: A held, left stick up, 5 tries per value)

Report `spikes/recordings/u6-ly-up-20261120-205031.md` (offset 0.331, scale 0.8897). Hz = median per note.

| `ly` | wire 12-bit | % of full travel | Hz (5 tries) | Pitch (vs A4 = 440) | Counted as +2 | Onset median |
|-----:|------------:|-----------------:|--------------|---------------------|---------------|--------------|
| (rest) | 0x800 | 0 | not measured | A5 (calibration 893.8 Hz, +27 c) | no (baseline) | |
| 0x40 | 0xB00 | 50 | 950.3 to 951.5 | A♯5 +34 c | no: 5/5 OTHER, between A5 and B5 | 22 ms |
| 0x20 | 0xC80 | 75 | 979.9 to 980.6 | B5 -14 c | 5/5 | 67 ms |
| 0x00 | 0xE00 | 100 | 1001.7 to 1002.8 | B5 +25 c | 5/5 | 56 ms |

- Repeatability: ±1 Hz over 5 tries at every value.
- Full deflection (1002 Hz) is a whole tone above the 893.8 Hz A5 (893.8 × 2^(2/12) ≈ 1003.3 Hz). 75 % reaches 980 Hz, about 22 Hz (about 38 cents) short of that; 50 % reaches 951 Hz.
- Observation (not a final conclusion): the stick acts as a **graded bend**, not a threshold switch. Partial deflection gives an intermediate pitch, very repeatable; only full deflection gives the full +2 semitones. 0x40 is not a clean B5.
- Onsets over all 15 notes: median 56 ms.

##### Down (`u6-ly-down.txt`: A held, left stick down, 5 tries per value)

Report `spikes/recordings/u6-ly-down-20261120-205151.md` (offset 0.321, scale 0.8900, capture 89.0 % of real time). Hz = median per note.

| `ly` | wire 12-bit | % of full travel | Hz (5 tries) | Pitch (vs A4 = 440) | Counted as -2 | Onset median |
|-----:|------------:|-----------------:|--------------|---------------------|---------------|--------------|
| (rest) | 0x800 | 0 | not measured | A5 (calibration 893.8 Hz, +27 c) | no (baseline) | |
| 0xC0 | 0x500 | 50 | 841.2 to 841.8 | G♯5 +22 c | no: 5/5 OTHER, intermediate (not G5) | 56 ms |
| 0xE0 | 0x380 | 75 | 815.4 to 815.8 | G♯5 -31 c | no: 5/5 OTHER, intermediate | 67 ms |
| 0xFF | 0x20C | 99 | 797.0 to 797.8 | G5 +29 c | 5/5 | 67 ms |

- Repeatability: within ±0.5 Hz over 5 tries at every value.
- Full deflection (797 Hz) is about a whole tone below the 893.8 Hz A5 (893.8 × 2^(-2/12) ≈ 796.3 Hz).
- Observation (not a final conclusion): the same graded bend as up, mirrored. Only full deflection gives the full -2 semitones.
- `ly 0xFF` = wire Y 0x20C (8-bit 0xFF) works for down. This closes OQ-2 in `spikes/README.md` (down and the 8-bit scaling confirmed).
- Onsets over all 15 notes: median 67 ms.

##### Pitch vs deflection (`u6-sweep.txt`: A held, 8 steps per direction, 1 note per step)

Report `spikes/recordings/u6-sweep-20261120-205302.md` (offset 0.478, scale 0.8863, capture 88.6 % of real time). Cents are computed here as 1200·log2(Hz / 893.8), against the calibration A5; "step" is the change from the previous row. Up and down are recorded separately (Story 1.5 decides the profile shape).

Up:

| `ly` | wire 12-bit | % of full travel | Hz | nearest note (A4 = 440) | cents vs 893.8 Hz | step (c) | mark |
|-----:|------------:|-----------------:|---:|------|-----:|----:|------|
| 0x80 | 0x800 | 0 | 893.6 | A5 | 0 |  | **rest** |
| 0x70 | 0x8C0 | 12 | 906.4 | A♯5 | +24 | +25 |  |
| 0x60 | 0x980 | 25 | 919.7 | A♯5 | +49 | +25 |  |
| 0x50 | 0xA40 | 38 | 934.8 | A♯5 | +78 | +28 |  |
| 0x40 | 0xB00 | 50 | 949.7 | A♯5 | +105 | +27 |  |
| 0x30 | 0xBC0 | 62 | 965.8 | B5 | +134 | +29 |  |
| 0x20 | 0xC80 | 75 | 980.2 | B5 | +160 | +26 |  |
| 0x10 | 0xD40 | 88 | 994.6 | B5 | +185 | +25 |  |
| 0x00 | 0xE00 | 100 | 1002.8 | B5 | +199 | +14 | **full travel** |

Down:

| `ly` | wire 12-bit | % of full travel | Hz | nearest note (A4 = 440) | cents vs 893.8 Hz | step (c) | mark |
|-----:|------------:|-----------------:|---:|------|-----:|----:|------|
| 0x80 | 0x800 | 0 | 894.0 | A5 | 0 |  | **rest** |
| 0x90 | 0x740 | 12 | 881.6 | A5 | -24 | -24 |  |
| 0xA0 | 0x680 | 25 | 868.2 | A5 | -50 | -27 |  |
| 0xB0 | 0x5C0 | 38 | 855.3 | A5 | -76 | -26 |  |
| 0xC0 | 0x500 | 50 | 841.0 | G♯5 | -105 | -29 |  |
| 0xD0 | 0x440 | 62 | 828.1 | G♯5 | -132 | -27 |  |
| 0xE0 | 0x380 | 75 | 815.6 | G♯5 | -159 | -26 |  |
| 0xF0 | 0x2C0 | 88 | 803.5 | G5 | -184 | -26 |  |
| 0xFF | 0x20C | 99 | 797.7 | G5 | -197 | -13 | **full travel** |

Threshold: none found in either direction (graded from the first step); no step gives a jump to the full ±2.

Observations (not final conclusions):

- No dead zone detected at the first step: 12 % deflection already bends the pitch.
- The bend is close to linear in cents with deflection (about 25 c per 0x10 step) and reaches about ±2 semitones at full travel.
- Up, last step 0x10 → 0x00 is smaller (about +14 c): possible saturation near full travel. Down, last step 0xF0 → 0xFF is also smaller (about -13 c, and that step is 15 / 16 of the others in wire travel).
- Values agree with trials 1 and 2 (Up, Down above) within about 1.5 Hz.
- The tool's `counted` / `OTHER` verdict is nearest-note based and not meaningful for a graded bend: Hz and cents are the result.
- Caveat: the onset column in this run is negative (median -23 ms), which suggests a small window misalignment; the pitch values are still consistent with trials 1 and 2.

##### Diagonals (`u6-diagonals.txt`: A held, 3 tries per position)

Report `spikes/recordings/u6-diagonals-20261120-205454.md` (offset 0.353, scale 0.8885). Cents are computed as 1200·log2(Hz / 893.8).

| Position | stick `lx:ly` | wire `xxx:yyy` | Hz range (3 tries) | cents vs 893.8 Hz |
|----------|---------------|----------------|--------------------|-------------------|
| lx-only left | 00:80 | 200:800 | 894.0 to 894.6 | +0 to +2 |
| lx-only right | FF:80 | DF4:800 | 893.2 to 894.8 | -1 to +2 |
| circle up-left | 26:26 | 3C8:C38 | 970.9 to 971.3 | +143 to +144 |
| circle up-right | DA:26 | C38:C38 | 971.9 to 973.4 | +145 to +148 |
| circle down-left | 26:DA | 3C8:3C8 | 822.8 to 823.2 | -143 to -142 |
| circle down-right | DA:DA | C38:3C8 | 821.4 to 823.0 | -146 to -143 |
| corner up-left | 00:00 | 200:E00 | 971.2 to 972.8 | +144 to +147 |
| corner up-right | FF:00 | DF4:E00 | 971.2 to 972.3 | +144 to +146 |
| corner down-left | 00:FF | 200:20C | 823.9 to 824.3 | -141 to -140 |
| corner down-right | FF:FF | DF4:20C | 822.2 to 822.8 | -145 to -143 |

Observations (as observations / interpretation, not final conclusions):

- X axis alone has no effect: left and right with `ly` centred stay at A5 (about 894 Hz).
- Diagonals give a **partial** bend (about +142 c up, about -143 c down), not the full ±200 c.
- The corners (Y wire at full travel, 0xE00 / 0x20C) give the same pitch as the circle diagonals (Y about 70 % of travel). This is consistent with the game clamping the stick vector to the unit circle and bending by its Y component (0.707 × 200 c ≈ 141 c).
- Consequence for Story 1.5: use a pure vertical stick (`lx` centred) for stick modifiers; any X component reduces the bend.
- Left/right symmetric within about 2 Hz.
- Onset median 56 ms.

Result (1.6): there is no threshold. The left stick is a graded bend, about linear in cents (about 25 c per 0x10 step), no dead zone at 12 %, slight saturation in the last step, and only full travel gives the full ±2 semitones: up `ly 0x00` (wire 0xE00, +199 c), down `ly 0xFF` (wire 0x20C, −197 c). Of the AC values, 0x40 / 0xC0 give about ±105 c and 0x20 / 0xE0 about ±160 c (not counted as ±2), 0x00 / 0xFF count 5/5. Up and down are mirrored within a few cents. The X axis has no effect; diagonals and corners give about ±142 c, so stick modifiers use a pure vertical stick at full travel. No finer round needed (Task 7.1): the sweep already gives the curve.

#### 1.9 Alternative fingerings (45 combinations)

Stick at full deflection (`ly` 0x00 up, 0xFF down), `lx` centred. Cents are vs equal temperament, A4 = 440 Hz, as read by the tool (see the tuner cross-check above: the game sounds about +27 c sharp vs A4 = 440 Hz). "Model" = the button's natural note plus the modifier offsets; the verdict is nearest note against the model.

##### L (`fingerings-L.txt`)

Report `spikes/recordings/fingerings-L-20261120-205702.md` (offset 0.338, scale 0.8868). 9 of 9 matched the model pitch. Onset median 90 ms.

| D-pad | Stick | Model offset | Hz | Note | Cents |
|-------|-------|-------------:|---:|------|------:|
| none | none | 0 | 596.6 | D5 | +27 |
| up | none | +1 | 633.3 | D♯5 | +30 |
| down | none | -1 | 560.9 | C♯5 | +20 |
| none | up | +2 | 668.6 | E5 | +24 |
| none | down | -2 | 530.1 | C5 | +23 |
| up | up | +3 | 710.6 | F5 | +30 |
| up | down | -1 | 564.2 | C♯5 | +31 |
| down | up | +1 | 628.9 | D♯5 | +18 |
| down | down | -3 | 499.9 | B4 | +21 |

Alternative fingerings seen on L: C♯5 = D-pad down alone, or D-pad up + stick down (560.9 and 564.2 Hz); D♯5 = D-pad up alone, or D-pad down + stick up (633.3 and 628.9 Hz).

##### R (`fingerings-R.txt`)

Report `spikes/recordings/fingerings-R-20261120-205754.md` (offset 0.339, scale 0.8862). 9 of 9 matched the model pitch. Onset median 79 ms.

| D-pad | Stick | Model offset | Hz | Note | Cents |
|-------|-------|-------------:|---:|------|------:|
| none | none | 0 | 702.6 | F5 | +10 |
| up | none | +1 | 743.5 | F♯5 | +8 |
| down | none | -1 | 665.2 | E5 | +15 |
| none | up | +2 | 788.0 | G5 | +9 |
| none | down | -2 | 624.4 | D♯5 | +6 |
| up | up | +3 | 834.3 | G♯5 | +8 |
| up | down | -1 | 663.5 | E5 | +11 |
| down | up | +1 | 744.6 | F♯5 | +11 |
| down | down | -3 | 592.8 | D5 | +16 |

Alternative fingerings seen on R: E5 = D-pad down alone, or D-pad up + stick down (665.2 and 663.5 Hz); F♯5 = D-pad up alone, or D-pad down + stick up (743.5 and 744.6 Hz).

Observation (no conclusion): the cents offset on R is about +6 to +16, against about +18 to +31 on L and about +25 to +34 on A (see below). This may be a per-button tuning difference or a measurement effect; not investigated.

##### A (`fingerings-A.txt`)

Report `spikes/recordings/fingerings-A-20261120-205845.md` (offset 0.330, scale 0.8902). 9 of 9 matched the model pitch. Onset median 90 ms (one row at -11 ms: small window misalignment, caveat).

| D-pad | Stick | Model offset | Hz | Note | Cents |
|-------|-------|-------------:|---:|------|------:|
| none | none | 0 | 894.3 | A5 | +28 |
| up | none | +1 | 950.1 | A♯5 | +33 |
| down | none | -1 | 843.5 | G♯5 | +27 |
| none | up | +2 | 1001.9 | B5 | +25 |
| none | down | -2 | 797.4 | G5 | +29 |
| up | up | +3 | 1067.5 | C6 | +34 |
| up | down | -1 | 845.5 | G♯5 | +31 |
| down | up | +1 | 946.8 | A♯5 | +27 |
| down | down | -3 | 752.1 | F♯5 | +28 |

Alternative fingerings seen on A: G♯5 = D-pad down alone, or D-pad up + stick down (843.5 and 845.5 Hz); A♯5 = D-pad up alone, or D-pad down + stick up (950.1 and 946.8 Hz).

Observation (no conclusion): A reads about +25 to +34 c, like L (about +18 to +31) and unlike R (about +6 to +16).

##### Y (`fingerings-Y.txt`)

Report `spikes/recordings/fingerings-Y-20261120-210111.md` (offset 0.328, scale 0.8878). 9 of 9 matched the model pitch. Onset median 90 ms.

| D-pad | Stick | Model offset | Hz | Note | Cents |
|-------|-------|-------------:|---:|------|------:|
| none | none | 0 | 1003.7 | B5 | +28 |
| up | none | +1 | 1065.9 | C6 | +32 |
| down | none | -1 | 951.3 | A♯5 | +35 |
| none | up | +2 | 1124.2 | C♯6 | +24 |
| none | down | -2 | 893.6 | A5 | +27 |
| up | up | +3 | 1194.3 | D6 | +29 |
| up | down | -1 | 949.7 | A♯5 | +32 |
| down | up | +1 | 1066.4 | C6 | +33 |
| down | down | -3 | 845.5 | G♯5 | +31 |

Alternative fingerings seen on Y: A♯5 = D-pad down alone, or D-pad up + stick down (951.3 and 949.7 Hz); C6 = D-pad up alone, or D-pad down + stick up (1065.9 and 1066.4 Hz).

##### X (`fingerings-X.txt`)

Report `spikes/recordings/fingerings-X-20261120-210204.md` (offset 0.360, scale 0.8864). 9 of 9 matched the model pitch by nearest note. Onset median 45 ms.

| D-pad | Stick | Model offset | Hz | Note | Cents |
|-------|-------|-------------:|---:|------|------:|
| none | none | 0 | 1190.1 | D6 | +23 |
| up | none | +1 | 1273.2 | D♯6 | +39 |
| down | none | -1 | 1113.5 | C♯6 | +7 |
| none | up | +2 | 1338.1 | E6 | +26 |
| none | down | -2 | 1061.7 | C6 | +25 |
| up | up | +3 | 1428.2 | F6 | +38 |
| up | down | -1 | 1135.2 | C♯6 | +41 |
| down | up | +1 | 1251.8 | D♯6 | +10 |
| down | down | -3 | 992.3 | B5 | +8 |

Alternative fingerings seen on X: C♯6 = D-pad down alone, or D-pad up + stick down (1113.5 and 1135.2 Hz); D♯6 = D-pad up alone, or D-pad down + stick up (1273.2 and 1251.8 Hz).

Repeat of X (`fingerings-X.txt` run again, report `spikes/recordings/fingerings-X-20261120-210326.md`, offset 0.349, scale 0.8867). The owner moved the Switch to a different position between the runs. 9 of 9 matched the model by nearest note. Onset median 90 ms.

| D-pad | Stick | Model offset | Hz | Cents | Hz, first run |
|-------|-------|-------------:|---:|------:|--------------:|
| none | none | 0 | 1192.4 | +26 | 1190.1 |
| up | none | +1 | 1273.4 | +40 | 1273.2 |
| down | none | -1 | 1113.0 | +7 | 1113.5 |
| none | up | +2 | 1336.5 | +24 | 1338.1 |
| none | down | -2 | 1062.1 | +26 | 1061.7 |
| up | up | +3 | 1427.5 | +37 | 1428.2 |
| up | down | -1 | 1135.3 | +41 | 1135.2 |
| down | up | +1 | 1251.2 | +9 | 1251.8 |
| down | down | -3 | 991.7 | +7 | 992.3 |

Observation (measured, repeatable): every value of the repeat agrees with the first X run within about 2 Hz, so the wider cents spread on X is not a placement or measurement artefact. On X the D-pad shifts the pitch by about +114 to +117 c (up) and -115 to -119 c (down) from the bare button, instead of about 100 c, so two fingerings of the same pitch differ by about 30 c (C♯6: 1113 vs 1135 Hz; D♯6: 1273 vs 1251 Hz). Each is still identified as the model pitch by nearest note. The SNR on X was lower (11 to 18 dB) in the first run. Cause not investigated.

All 45 combinations run: 45 of 45 match the model by nearest note.

Result (1.9): every one of the 45 combinations sounds the model pitch (button + D-pad ±1 + stick ±2 at full travel), for all five buttons; no "no sound", no wrong octave. The nine modifier states give 7 distinct offsets, so the 45 inputs cover the 19 pitches with 1 to 4 fingerings each; the per-pitch list is in `docs/ocarina-input-mechanics.md`, "Alternative fingerings". Deviation in tune only: on X the D-pad shifts about ±115 c instead of about 100 c (repeated, confirmed).

## Spike 2: timing limits

Needs a scripted sender (any tiny script over the serial port is fine for a spike).

|  ID | Measure                             | Sweep                                                      | Pass threshold      | Result (Story 1.4, 2026-11-21) |
|----:|-------------------------------------|------------------------------------------------------------|---------------------|--------------------------------|
| 2.1 | U7 min press that always sounds     | 1…10 frames (16.7 ms steps), 20 tries each                 | 20/20 sound         | **2 frames (33.3 ms)**: 20/20 on A, L, R, X, Y; 1 frame sounds 16 to 18/20 |
| 2.2 | U2 min gap to retrigger same button | 1…10 frames                                                | 20/20 retrigger     | **5 frames (83.3 ms), owner-chosen value**. Measured 20/20: A, L, Y at 4 frames; R 5 frames 19/20 pooled (20/20 with its own file's threshold); X unresolved (mic level, best 4 frames 18/20); R and X not run above 5 frames. Definition-dependent, see below |
| 2.3 | U5 modifier lead                    | 0…5 frames                                                 | 20/20 correct pitch | **0 frames** for D-up, D-down, stick up, stick down: no wrong pitch longer than about 35 ms (2 frames) at lead 0 (20/20 at every lead 0 to 5 frames on A; confirmed on L, R, X, Y at 0 frames); a 1 to 2 frame lateness is not resolvable by audio |
| 2.4 | Onset jitter                        | 120 BPM 16ths, 30 s, record + onset detection (e.g. aubio) | σ ≤ 1 frame         | IOI error σ 8.1 / 11.7 / 11.7 ms (0.48 / 0.70 / 0.70 frame) in 3 runs: **σ ≤ 1 frame met**; latency 123 to 139 ms; per-note latency in two clusters one frame apart |
| 2.5 | U10 frame rate                      | count frames in a 60 fps capture                           | –                   | **frame period 16.67 ms (60 Hz)** from a 240 fps phone video; sustained 60 distinct images per second not shown (the scene did not move continuously) |

Done when: the `[timing]` section of the profile is filled with measured values + safety margin. The measured values are in "Story 1.4 field trials" below ("Values for Story 1.5"); Story 1.5 adds the safety margin and fills the profile.
Story 1.5 (2026-11-21): `[timing]` in `app/profiles/oot-switch2-1.1.1.toml` is filled: `min_press_frames = 3` (measured 2 + 1 frame margin), `min_gap_same_button_frames = 5` (owner choice, not a measured limit), `modifier_lead_frames = 1` (measured 0 + 1 frame margin). `max_sustain_ms = 5000` is an **estimate** (U1 hand-timed at about 6 s, row 1.3; a short U1 test is deferred to the Story 1.6 Switch session) and `start_lead_ms = 500` is a design default, not measured. The profile targets OoT 1.1.1 on Switch 2 system 23.5.0 (see "Test environment").

### Story 1.4 field trials (2026-11-21)

All trials run on 2026-11-21 by the owner, Part B of Story 1.4. Answers are also in `docs/ocarina-input-mechanics.md` (U2, U5, U7, U10). The raw per-run log (every command, offset, part table) is in the Story 1.4 file, Dev Agent Record.

#### Values for Story 1.5

| Profile field | Value | From | Notes |
|---|---|---|---|
| `min_press` | **2 frames = 33.3 ms** | row 2.1 | 20/20 on all five buttons; 1 frame sounds 16 to 18/20 (not reliable) |
| `min_gap_same_button` | **5 frames = 83.3 ms** (owner-chosen, not a measured limit) | row 2.2 (decision) | measured 20/20: A, L, Y at 4 frames; R 5 frames 19/20 pooled (20/20 with its own file's threshold); X unresolved (mic level, best 4 frames 18/20); R and X not run above 5 frames |
| `modifier_lead` | **0 frames** measured (per type: D-up 0, D-down 0, stick up 0, stick down 0) | row 2.3 | means: no wrong pitch longer than about 35 ms (2 frames) at lead 0; a 1 to 2 frame lateness is not resolvable by audio, so **keep a margin of at least 1 frame** in Story 1.5. The stick still glides at the start (Y + stick up: first 30 ms window −25 to −36 c, median −29 c; median of the first 40 ms −7 c) |
| `[profile] frame_rate` | **60 Hz (frame period 16.67 ms)** | row 2.5 | 240 fps phone video, median run of 4 identical capture frames; sustained-motion caveat |
| jitter | IOI error σ 8.1 to 11.7 ms (0.48 to 0.70 frame), p95 18.8 to 21.1 ms, max 21.8 to 22.9 ms; latency 123 to 139 ms (per run), drift ≤ 0.22 ms/s | row 2.4 | 2wiCC's per-frame queue is not our scheduler (the sender grid is quantised at 7/8 frames here; the real firmware scheduler is Story 2.4/2.5). Schedule quantisation σ 8.35 ms (0.50 frame) is separate |

All values except `min_gap_same_button` are measured limits (the smallest value that passed 20/20, and the pass counts around it); the safety margin is Story 1.5's.

#### Environment and capture

- Switch 2 system 23.5.0, OoT 1.1.1, console handheld (built-in screen, not docked, so no TV video mode) for all trials; controller on the Switch 2 bottom USB-C port (top port for the U10 video only, console standing). Per-session wiring/GND check skipped by owner decision (wiring unchanged since the GND↔GND 0.1 Ω measurement above); bridge probes `+ID`/`+VER`/`+GQS` verified at every controller connect.
- macOS (Darwin 27.2); bridge `/dev/cu.usbmodem101` ("2wiCC COM"), 2wiCC v2.2 + 2wiCC_Comms v1.1, Apple USB-C cable.
- Sender [`spikes/send_2wicc.py`](../spikes/README.md) through `spikes/listen_trial.py`; trial files `spikes/samples/story-1-4/` (round 1, `confirm-*`, `jitter-120bpm-16ths.txt`) and `spikes/samples/story-1-4-ext/` (R gap 4 and 5 frames), all generated by `spikes/gen_trials_1_4.py`: times in frames, 20 tries per step in interleaved shuffled rounds (phase diversity), a warm-up attempt per file, blank and gap controls.
- Capture: MacBook Pro built-in microphone through PortAudio (`--backend sounddevice`, lossless). Capture self-test (1 kHz tone from the Mac speakers, 60 s): 0 phase discontinuities, delivered-sample ratio 1.00000, 0 overflows, PASS. Every sounddevice run had scale 1.0000. The Switch 2 was next to the Mac from U7 part 3 on; the character was moved to a quieter spot before U7 part 4.
- Calibration (`calibration-A.txt`): A5 894.2 Hz, +28 c, onset 130 to 140 ms after the scheduled press. The first calibration run gave no sound (the console had not accepted the injected controller yet: the first-input-after-connect quirk); after L+R then A by hand it worked.
- Results below are the **final re-score** with the fixed analysers (reports `spikes/recordings/*-rescored.*`, git-ignored). The live-run numbers of the confirmation and jitter runs were wrong because of analyser artefacts (see "Lessons").

#### 2.1 U7 min press (`--mode detect`)

Sound = power within ±3 % of the expected pitch, 0.02 to 0.30 s after the press, at least 8 dB above the same band before the press.

| Press (frames / ms) | A (round 1) | L | R | X | Y |
|---|---|---|---|---|---|
| 1 / 16.7 | 18/20 (rounds 1, 15) | 18/20 | 18/20 | 17/20 | 16/20 |
| 2 / 33.3 | 20/20 | 20/20 | 20/20 | 20/20 | 20/20 |
| 3 to 10 / 50 to 167 | 20/20 each | – | – | – | – |

Blank controls (no press): 20/20 silent on A and in every confirmation file. Result: **U7 = 2 frames (33.3 ms)** on every button. The sounded length is not proportional to the press (a 1-frame press sounded about 100 to 210 ms in the part 1 live report).

#### 2.2 U2 min gap, same button (`--mode retrigger`)

A held 12 frames, released `gap` frames, pressed 12 frames again; controls gap 0 (one held note) and gap 30 (clear retrigger) in every round. The analyser measures the dip of the note's fundamental between the two presses; the threshold comes from the controls pooled per button: deepest gap-0 dip + min(6 dB, half the distance to the shallowest gap-30 dip).

| Gap (frames / ms) | A (thr 11.8 dB) | L (17.7) | R (16.9) | X (10.2) | Y (10.8) |
|---|---|---|---|---|---|
| 1 / 16.7 | 0/20 | – | – | – | – |
| 2 / 33.3 | 4/20 | – | – | – | – |
| 3 / 50.0 | 19/20 | 17/20 | 5/20 | 12/20 | 19/20 |
| 4 / 66.7 | 20/20 | 20/20 | 17/40 (two runs) | 18/20 | 20/20 |
| 5 / 83.3 | 20/20 | – | 19/20 (20/20 in its own file) | – | – |
| 6 to 10 | 20/20 each | – | – | – | – |

Controls: every gap-0 and gap-30 control behaves under the pooled threshold. Median dip per gap: A 1f 5.8, 2f 9.6, 3f 16.0, 4f 19.9, 5f+ about 21 dB; R 3f 13.9, 4f 16.1, 5f 20.3, gap 30 21.2 dB. The dip grows smoothly with the gap (the note decays while the button is up), so "retrigger" is a threshold on a continuous quantity, not a yes/no of the game. R's threshold is raised by one held note that dipped 14.7 dB; with a 95th-percentile rule instead of the deepest held-note dip R gives 3f 13/20, 4f 37/40, 5f 20/20. X's full dip (gap 30) is only about 13 dB above its noise floor at the mic, so its 4-frame dips (median 12.8 dB) sit on the threshold: a measurement limit, not a game result. Owner decision: **U2 = 5 frames (83.3 ms)** for the profile; this is a chosen value, not a measured limit (R and X were not run above 5 frames).

#### 2.3 U5 modifier lead (`--mode lead`)

Modifier set `k` frames before A (0 to 5), note 36 frames, modifier kept 6 frames after the release. Pass = median pitch of the 9 overlapping 30 ms windows starting in the first 40 ms of the note within 50 c of the steady pitch. Resolution (synthetic notes, ±100 c and −200 c): a wrong start of 33 ms passes, one of 36 ms or more fails, so a pass means no wrong pitch longer than about 35 ms (2 frames).

| Lead (frames) | D-up (A♯5) | D-down (G♯5) | stick up (B5) | stick down (G5) |
|---|---|---|---|---|
| 0 to 5 | 20/20 each | 20/20 each | 20/20 each | 20/20 each |

Confirmation at 0 frames on L, R, X, Y: 20/20 in every file except X + stick down 19/20 and Y + stick up 19/20 (one failure each). Result: **U5 = 0 frames** for all four modifiers, at the 2-frame resolution above; Story 1.5 should keep a margin of at least 1 frame. Finding: on Y + stick up the first 30 ms window reads −25 to −36 c (median −29 c) from the steady pitch (the stick bend still gliding; also −25 c on R + stick up), the 40 ms median −7 c, so it passes. An earlier analysis method read about −50 c for the same window.

#### 2.4 Onset jitter (`--mode jitter`)

`jitter-120bpm-16ths.txt`: 240 notes, L R A Y X Y A R cycle, press 3 frames (`--press-frames 3 --min-press 2 --min-gap 4`), 7/8-frame slots. Onsets per button pitch (20 ms window), one latency per run, onset at the −6 dB crossing; IOI error = measured inter-onset interval minus the sender's scheduled interval.

| Run | Backend | Matched | Latency (median, sd) | Drift | IOI σ | p95 | max |
|---|---|---|---|---|---|---|---|
| 1 | sounddevice | 237/240 | 127.3 ms (5.8) | +0.011 ms/s | 8.07 ms (0.48 f) | 18.8 ms | 21.8 ms |
| 2 | sounddevice | 240/240 | 138.9 ms (7.5) | +0.215 ms/s | 11.67 ms (0.70 f) | 21.1 ms | 22.2 ms |
| 3 | sounddevice | 239/240 | 123.3 ms (7.1) | +0.054 ms/s | 11.65 ms (0.70 f) | 20.9 ms | 22.9 ms |
| 4 | ffmpeg | invalid: 78 % of real time delivered | – | – | 47.7 ms | – | – |

Schedule quantisation (7/8-frame pattern vs ideal 125 ms), not jitter: σ 8.35 ms (0.50 frame). The per-note latency falls into two clusters about one frame apart (run 3: 95 notes at −6 ms and 76 at +6 ms from the median; run 1: 154 at −2 ms, 25 at −18 ms): consistent with the game sampling input once per 60 Hz frame while the sender's grid is not phase-locked to it. **σ ≤ 1 frame: met** in all three valid runs. 2wiCC's per-frame queue is not our scheduler (the sender grid is quantised at 7/8 frames here; the real firmware scheduler is Story 2.4/2.5). Run 4 shows why the lossless capture is needed (Task 10.3).

#### 2.5 U10 frame rate (`count_frames.py`)

iPhone slow motion 240 fps (h264 1280×720, 9929 frames, 41.4 s), phone fixed on the Switch screen, handheld console standing, while `jitter-120bpm-16ths.txt` played so the picture moved. `uv run spikes/count_frames.py spikes/recordings/u10-framerate-IMG5861.mov`: 620 changes, threshold 1.49 grey levels (static noise 0.37); run lengths of identical capture frames 4: 464, 8: 31, 12: 15, 3: 14, others few; median run 4 → **frame period 16.67 ms (60 Hz)**. Distinct images per 1 s window: min 0, median 15, max 33, because the scene was not in continuous motion for a whole second, so "60 distinct images every second" is not directly shown; when the picture changes it changes on a 16.67 ms grid. The logic/input rate is not measured by video; the one-frame latency clusters of row 2.4 are consistent with 60 Hz input. Video 41 s, not several minutes (the story asked for a few minutes).

#### Lessons (analysis artefacts found and fixed, 2026-11-21)

The final numbers come from analysers revised after seeing these recordings (each fix has a synthetic test for its trap). The 1-frame U7 counts, U2 at 2 to 3 frames and the jitter σ depend on analysis definitions and should be confirmed on fresh recordings with the frozen analyser in Story 1.5 / Epic 7.

- A broadband (250 to 4000 Hz) peak-vs-RMS SNR is biased: room noise alone scored 5 to 14 dB, so blanks "sounded" and X (D6, quieter at the mic: 9 to 11 dB broadband) failed. Measured at the expected pitch, X notes are 14 to 23 dB and blanks at most 6.8 dB.
- Short-window YIN on noisy notes locked onto f/2, f/3 or 2f (X read as G4 or D5; lead errors of ±1170 to 1910 c). Pitches are now spectral peaks searched near the expected pitch.
- Jitter: generic onset detection found about 385 onsets for 240 notes and the latency search locked onto wrong offsets (87 to 354 ms), with a fake drift and errors bounded by the 40 ms match window. Per-button envelopes with one latency per run fixed it.
- The U2 dip depth depends on the gap continuously and on the button's decay; one outlier held note changes a per-file threshold, so controls are pooled per button.
- The ffmpeg avfoundation capture is unusable for timing (78 % of real time delivered in the jitter run).
- After any controller connect, the first injected input is ignored (seen again: first calibration run, after every re-plug).

Possible follow-ups (dropped by the owner on 2026-11-21): U2 on X with the microphone closer to the speaker; another R gap-0 control run to see whether a held R note dips 14.7 dB again; a longer U10 video with continuous motion.

### Spike sender check (Story 1.2)

2026-11-20. Sender: [`spikes/send_2wicc.py`](../spikes/README.md). Environment: macOS (Darwin 27.2), `/dev/cu.usbmodem101`, Apple USB-C cable, console bottom port, 2wiCC v2.2 + 2wiCC_Comms v1.1, GND bridged (0.1 Ω). Switch 2 system 23.5.0, OoT 1.1.1.

| Check | Command / input | Result |
|---|---|---|
| Probes | `+ID `, `+VER `, `+GQS `, `+GQR `, `+GCS ` | `+2wiCC`, `+VER 2.2`, `+GQS 0400`, `+GQR 03FE` (idle in RT: one slot held by the current state, so 1022 free), `+GCS 1`; replies end with `\r\n` |
| Held queue, no stop | `+SPM BUF` + 120 × `+QD 080000` (2 s) | A held continuously (test screen exits) |
| Stop | same queue, then `+SPM RT` + `+QD 000000` after ~0.3 s | A released at once (screen stays open); `+GQR ` right after → `+GQR 03FE` (backlog discarded). Immediate stop confirmed |
| Stick Y | RT, `+QF 0000000008E0000880` (left X 0x800, Y 0xE00) | stick **up** on the stick test screen; Y 0x200 not tested |
| Minimal file | `uv run spikes/send_2wicc.py --port /dev/cu.usbmodem101 spikes/samples/a-press.txt` | A pressed and released, screen stayed open; `summary: 3 rows, 61 frames, 1.017 s, port /dev/cu.usbmodem101, exit 0` |
| L R A Y X scale, free play | `spikes/samples/scale-lrayx.txt` | 1st run: game showed a "connect controller" prompt, nothing played. 2nd run: all 5 notes L R A Y X in the right order; pitches checked with a tuner: D5 F5 A5 B5 D6, as expected; `summary: 11 rows, 301 frames, 5.017 s, ..., exit 0` |
| Ctrl+C, free play | file holding A from 0.5 s to 6 s, real Ctrl+C from the owner's terminal while the note sounded | note stopped at once; `^Cinterrupted: sending neutral`, `summary: 3 rows, 361 frames, 6.017 s, port /dev/cu.usbmodem101, exit 130` |

Observations:

- After the controller sits idle, the Switch may show "press A when the controller is ready"; one short A press (`+SPM RT`, `+QD 080000`, 0.15 s, `+QD 000000`) dismisses it. Relevant for future navigation macros. The sender does **not** press A automatically (it would play a note in free play)
- SIGINT sent to a background `uv run` from a non-interactive shell does not reach the script (uv does not forward it, and background jobs start with SIGINT ignored); a real Ctrl+C from a terminal works

## Spike 3 (optional): single board with Pico-PIO-USB

- Example `usb_device` of Pico-PIO-USB on a Zero, D+/D− on two GPIOs, 1.5 kΩ pull-up on D+
- Done when: the PC enumerates the PIO port while the native port drives the Switch 2

## Timing validation phase (NFR-04, roadmap M7)

Goal: prove the **whole** chain (host → bridge → controller → Switch 2 → game audio) meets NFR-04, not just the firmware.

### Method

- Reference material: metronome piece (120 BPM, 8ths and 16ths), the 1-min reference piece (A.1), a 5-min piece
- Ground truth: the CSV timeline export from the app (`t_us`, pitch, fingering of every event; AD-15)
- Measurement: record the console audio (capture card or Switch 2 video capture), detect onsets (e.g. aubio), align on the first note, compare each onset to the timeline
- Optional cross-check: controller board toggles a GPIO at every scheduled event → logic analyser vs audio
- Metrics: inter-onset interval (IOI) error = measured gap between consecutive onsets minus the score gap (ms and frames): p95, max, σ; drift = slope of cumulative onset error vs time. The fixed capture latency cancels out in IOI error.

### Tests

| ID  | Test                                                            | Pass criterion                              | Result |
|-----|-----------------------------------------------------------------|---------------------------------------------|--------|
| T.1 | Metronome piece, 60 s                                           | IOI error p95 ≤ 1 frame, max ≤ 1.5 frames   |        |
| T.2 | 5-min piece                                                     | drift < 1 frame over the piece              |        |
| T.3 | T.1 + T.2 on Windows, Linux, macOS                              | same results on all 3 OS (host-independent) |        |
| T.4 | T.1 with host under load (CPU stress, other USB traffic)        | no change vs T.1                            |        |
| T.5 | Fastest passage the profile allows (min press/gap from spike 2) | 0 missed or wrong notes in 5 runs           |        |

### Exit

- All pass → NFR-04 confirmed, M7 done
- Some fail → find the stage (firmware GPIO vs audio tells host/firmware from console/game), fix or revise NFR-04 with the measured numbers

## Watchdog tests

| ID  | Test                                                                                      | Pass criterion                                             | Result |
|-----|-------------------------------------------------------------------------------------------|------------------------------------------------------------|--------|
| W.1 | Soak: 30 min connected in direct mode on each OS; read the max-gap counter (max gap between app frames) from the debug-build STATUS | sets `app_timeout_ms` (max gap × safety margin, ≤ 1 s; default 1000 until then) |        |
| W.2 | 30 min of playback with the host under load (as T.4)                                       | 0 false watchdog trips                                      |        |

## Firmware host tests (`host-tests` preset, ctest, no hardware)

| ID  | Test                                                                                              | Pass criterion                                         | Related |
|-----|---------------------------------------------------------------------------------------------------|--------------------------------------------------------|---------|
| F.1 | Go-back-N: in-order accept, stale seq → cumulative ACK, duplicate of `expected−1` → cached response, wrap | matches AD-6                                     | AD-6    |
| F.2 | NAK matrix: each transport (`CRC`, `SEQ`, `FULL`) and semantic reason; transport never advances `expected`, at most one per `expected` | matches AD-6 | AD-6 |
| F.3 | Mode table: every frame × IDLE / DIRECT / PLAYING; abort latch set by every non-requested cause, cleared only by STOP / HELLO | matches AD-8 | AD-8 |
| F.4 | END and underrun: `NEUTRAL\|END` → `PLAYBACK_DONE`; END on non-neutral → `NAK BAD_STATE`; underrun judged at apply time → NEUTRAL in the same pass + `UNDERRUN`; LATE rule | matches AD-8 | AD-8 |
| F.5 | HELLO / STOP during PLAYING, with any seq                                                         | NEUTRAL, queue cleared, → IDLE, `expected = seq+1`     | AD-6, AD-8 |
| F.6 | Queue-mode allow-list: every non-allowed button cleared, right stick centred in PLAYING; DIRECT unmasked | matches AD-9                                   | AD-9    |
| F.7 | Heartbeat gap during a 512-frame burst (bridge, fake clock)                                       | every gap ≤ 27 ms                                      | AD-4    |
| F.8 | Bridge framer: whole frames only, one in flight + one queued, overlong run dropped, partial frame discarded on DTR fall | matches AD-4                    | AD-4    |
| F.9 | Shared vectors (`firmware/common/proto/vectors/`)                                                 | all pass; `PROTO_VERSION` = vector header              | AD-14   |
| F.10 | `console_ready` with a fake profile: STATUS `flags` bit1 follows `ready()`; 0 before handshake, 1 after, 0 on suspend / unmount / bus reset and after > 100 ms without an IN report; reserved bits 2–7 always 0; NOT_READY gating uses the same predicate | matches AD-19, AD-8 | AD-19, AD-12 |

## Hardware-in-the-loop checks (M3/M4)

| ID  | Test                                                                                                   | Pass criterion                                   | Result |
|-----|--------------------------------------------------------------------------------------------------------|--------------------------------------------------|--------|
| H.1 | Bridge unpowered, controller on the Switch: measure the voltage at the bridge's GP1                    | ≈ 0 V: the controller has released TX (no back-powering) | |
| H.2 | Core stall (debug build: hang core 0, then core 1)                                                     | watchdog reboot, USB detach, no input left pressed | |
| H.3 | `console_ready` on the Switch 2 (P2): plug in with Wired Communication ON, then put the console to sleep and wake it, then switch Wired Communication OFF and ON | flag 0 until the handshake, 1 after; back to 0 within 150 ms on sleep and on Wired Communication OFF; 1 again after wake / ON; `START` gets `NAK NOT_READY` whenever the flag is 0 | |

## Later: acceptance tests

| ID  | Test                                                                                    | Related        |
|-----|-----------------------------------------------------------------------------------------|----------------|
| A.1 | Reference piece (1 min, uses all 19 pitches) plays with 0 wrong notes                   | NFR-04, NFR-08 |
| A.2 | Same on Windows, Linux, macOS                                                           | NFR-01         |
| A.3 | Unplug the bridge mid-song, and separately pull the UART wire mid-song (20× each) → neutral within 100 ms every time; also check that the controller releases TX (high-Z) after the break / link loss | NFR-05, AD-5, AD-20 |
| A.4 | Flash both boards from the host app on each OS                                          | FR-09          |
| A.5 | First-time user (not the developer) unblocks the unsigned app with the README, then flashes and plays a song with the GUI only, no other help | FR-15, NFR-10  |

## Related

- Ocarina Bot: Roadmap (author's notes)
- Ocarina Bot: Requirements (author's notes)
- Scripted sender for Spike 0b and Spike 2: [`spikes/README.md`](../spikes/README.md)
