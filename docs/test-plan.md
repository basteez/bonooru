# Bonooru: Test plan

*Converted from the author's org-roam notes on 2026-11-19. This file is now the reference in the repo.*

Topic: Bonooru (author's notes)

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

### Bench rig for the controller link (Story 2.6)

H.4 to H.7 need no Switch 2: the controller Zero is powered from a PC USB port through its own USB-C (it does not enumerate yet; that is expected) and wired to a peer that speaks UART at 460800. Peer: **(a)** a 3.3 V USB-UART adapter (never connect its VCC or 5 V pin), or **(b)** the second Zero with 2wiCC_Comms, only after rig check R0 (its own TX and RX jumpered together, `link_probe.py loopback --repeat 100`: every byte value returns unchanged). For the 2026-11-22 session R0 was done by source inspection instead (story 2.6 D32): 2wiCC_Comms v1.1 copies bytes unchanged between the CDC endpoint and UART0 at a fixed 460800 8N1, but it has no flow control and drops bytes silently when its 1 KB USB-to-UART ring is full, so `link_probe.py` paces its writes to the UART rate (on by default; `--no-pace` disables it). Wiring: peer TX → controller RX, controller TX → peer RX, GND↔GND, nothing on 5V or 3V3, GND continuity measured first. Sender: `firmware/tools/link_probe.py` (`pip install pyserial`). Record once for the session:

| Item | Value |
|---|---|
| Peer (type, model, chip) | |
| Rig check R0 (option b only) | 2026-11-22: by source inspection (story 2.6 D32), not run physically |
| `link_probe.py` commit | |
| Controller UF2 (CI run or local build commit; fw-debug for H.4–H.6, fw-release for H.7) | |
| Host OS and version, serial port name | |
| Cable(s) | |

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
| 1.3 | U1 sustain                             | hold 1 s, 3 s, 10 s                                                                                                                                  | sustains while held; wavers from ~2 s; auto-stops at ~6 s (by hand); measured in row 1.10 |
| 1.4 | U3 new button while holding            | L held, then press R                                                                                                                                 | new button cuts the held note (F5 starts, D5 stops); releasing R with L held brings D5 back |
| 1.5 | U4 bend while sounding                 | hold A, then D-pad ↑                                                                                                                                 | D-pad: instant step to A♯5 (932 Hz); left stick: gradual glide (by ear) |
| 1.6 | U6 stick threshold                     | ly = 0x40, 0x20, 0x00                                                                                                                                | no threshold: graded bend, about linear in cents, ±2 semitones only at full travel (up `ly 0x00` = wire 0xE00, +199 c; down `ly 0xFF` = wire 0x20C, −197 c); 0x40/0xC0 ≈ ±105 c, 0x20/0xE0 ≈ ±160 c; X axis no effect; diagonals and corners ≈ ±142 c. See Story 1.3 field trials below |
| 1.7 | U8, U9 free play enter/exit and limits | play 100 notes in a row                                                                                                                              | enter D-pad ← (input after ~2 s), exit B or D-pad ←; 100 notes and 1 min idle: no limit, no auto-exit |
| 1.8 | official song recognised in free play? | play a song's natural notes (e.g. Zelda's Lullaby) in free play; then the same with one note on an alternative fingering (e.g. D4 = R + D↓ + stick↓) | yes; matched by pitch, not button: alternative fingerings (B, then D) still trigger it, even very slow |
| 1.9 | 45 fingerings: 5 buttons × 9 modifier states (D-pad none/up/down × stick none/up/down), alternative fingerings | one file per button, stick at full deflection, `lx` centred; pitch from `listen_trial.py` | all 45 run: 45/45 match the model (button + D-pad ±1 + stick ±2) by nearest note; 19 pitches, 1 to 4 fingerings each; on X the D-pad shifts about ±115 c (repeated, confirmed). See Story 1.3 field trials below |
| 1.10 | U1 sustain, measured (Story 1.6)     | A held 4, 5, 5.5, 6, 6.5, 7 s (5 tries each) + 10 s and 1 s controls + blanks, microphone onset/offset (`listen_trial.py --mode sustain`); confirmation on L, R, Y, X at the candidate limit and the next longer hold | A held to the release up to 6.5 s (5/5 each at 4 to 6.5 s); auto-stop 6599 to 7157 ms (7 s 3/5, 10 s 5/5); L, R, Y, X not run (owner decision: assumed equal to A); profile `max_sustain_ms = 6000` (6500 − 500 ms). See "1.10 U1 sustain, measured" below |

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
Story 1.5 (2026-11-21): `[timing]` in `app/profiles/oot-switch2-1.1.1.toml` is filled: `min_press_frames = 3` (measured 2 + 1 frame margin), `min_gap_same_button_frames = 5` (owner choice, not a measured limit), `modifier_lead_frames = 1` (measured 0 + 1 frame margin). `max_sustain_ms` was an estimate of 5000 ms at Story 1.5 (U1 hand-timed at about 6 s, row 1.3); Story 1.6 measured U1 on A and set it to **6000 ms** (row 1.10: measured 6500 ms, margin 500 ms; L, R, Y, X assumed equal to A by owner decision); `start_lead_ms = 500` is a design default, not measured. The profile targets OoT 1.1.1 on Switch 2 system 23.5.0 (see "Test environment").

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

## Walking skeleton (Story 1.6)

The first whole melody from a hand-written note list through the measured profile, the spike sender and 2wiCC, to sound in OoT free play (risk R4), plus the short U1 sustain trial of the same session. Trial files: `spikes/samples/story-1-6/` (generated by `spikes/gen_trials_1_6.py` from `ode-to-joy.melody` and `app/profiles/oot-switch2-1.1.1.toml`); analysers `listen_trial.py --mode melody` and `--mode sustain`; commands in [`spikes/README.md`](../spikes/README.md), "Story 1.6 trials". The timing heard here is the 2wiCC queue and the PC host clock, **not** our firmware scheduler (Epic 2 and Epic 7 measure that).

Melody: Ode to Joy (Beethoven, public domain), the whole theme (A, A', B, A') in G major at G5, 62 notes, D5 to D6, chart fingerings of the profile (D5 = L, G5 = R + stick up, A5 = A, B5 = Y, C6 = Y + D-up, D6 = X). "In tune" = the nearest note is the expected note and within ±50 c of the run's own median for that pitch class (the game is about +27 c of A4 = 440 Hz).

Results: Part B session held 2026-11-21 (session over, the Switch was powered off). Every cell below is filled only from recorded data.

### Environment of the session

| Item | Value |
|---|---|
| Date | 2026-11-21 |
| Switch 2 system / OoT version | 23.5.0 / 1.1.1 (from "Test environment"; not re-checked this session) |
| Controller firmware (2wiCC / 2wiCC_Comms) | 2wiCC v2.2 (`+VER ` -> `+VER 2.2`; `+ID ` -> `+2wiCC`; `+GQS ` -> `+GQS 0400`); 2wiCC_Comms version not probed |
| Host, bridge port | macOS, `/dev/cu.usbmodem101` |
| Console mode (handheld / docked) | handheld (from "Test environment"); controller on the bottom USB-C port; per-session wiring check skipped by owner decision (breadboard unchanged) |
| Microphone, position, volume | MacBook Pro Microphone via PortAudio (`--backend sounddevice`); position and volume not reported |
| Capture self-test (`--selftest-tone`) | not run this session; the calibration SNR is used as the mic check |

### Calibration and controls

| Check | Result |
|---|---|
| `calibration-A.txt`: A5 Hz, cents, SNR | A5 893.4 Hz, +26 c, -28.5 dBFS, SNR 19 dB, verdict match (default pitch mode); sender exit 0, 253 frames; report `spikes/recordings/calibration-A-20261121-184633.{csv,md}` (git-ignored) |
| Onset latency used (`--latency`) | 0.12 s (calibration onset 120 ms after the scheduled press) |
| First input after connect | ignored (first-input quirk): the first A after connecting did not register; a second A was needed before calibration |
| U1 blanks (must stay silent) | round 1 on A: silent (ok) in all 5 files |
| U1 1 s controls (must be held to the release) | round 1 on A: 1000 ms 0/5 interrupted |

### Melody runs

| Run | File | BPM | Notes | Audible | Right pitch | Missed | Extra onsets | Cents spread | Invalid? | Owner's ear |
|---|---|---:|---:|---:|---:|---:|---:|---|---|---|
| | `ode-to-joy-60bpm.txt` (by ear only) | 60 | 62 | 56 | 56 | 6 | 18 (08 A5: 3, 15 A5: 4, 33 B5: 3, 38 B5: 7, 40 A5: 5, 43 B5: 2) | -3 to +3 c (sd 0.7 c) vs pitch-class medians A +29, B +24, C +26, D +23, G +3 (c vs A4=440) | not reported | "generally good, some notes were missing" (which notes not said) |
| 60 BPM run 2 (owner watching the screen) | `ode-to-joy-60bpm.txt` | 60 | 62 | 58 | 52 | 4 | 1 (15 A5: 2) | -2 to +1 c (sd 0.6 c) | not reported | "nothing appears on screen during the silent notes, as if the button were not pressed"; 6 CUT SHORT in the per-note table (summary line printed no "cut N" field). Note: the analyser version of this run counted the 6 CUT SHORT notes outside "right pitch" (report CSV: 52 ok, 6 CUT SHORT, 4 NO SOUND); the pitch itself was correct on all 58 audible notes |
| superseded (syncopated version, not official 1/3) | `ode-to-joy-90bpm.txt` as it was then (syncopated rhythm, 400 ms press cap, owner decision 5; report `...-20261121-193350`) | 90 | 62 | 62 | 62 | 0 (cut 0) | 2 (19 D6: 2, 52 D6: 2) | -3 to +1 c (sd 0.7 c); pitch-class medians A +27, B +22, C +24, D +24, G +3 | not reported | "good, but the second note of the theme was missing when it takes it up again at the end" (analyser reports all notes sounded; offline check below) |
| official 1/3 | `ode-to-joy-90bpm.txt` (final: simplified rhythm, owner decision 6; 400 ms cap, owner decision 5; report `...-20261121-193926`) | 90 | 62 | 62 | 62 | 0 (cut 0) | 2 (04 D6: 2, 20 D6: 2) | -3 to +2 c (sd 0.7 c) | no INVALID flag | "it was fine" |
| official 2/3 | `ode-to-joy-90bpm.txt` (report `...-20261121-194033`) | 90 | 62 | 62 | 62 | 0 (cut 0) | 0 | -3 to +1 c (sd 0.6 c) | no INVALID flag | "it was fine" |
| official 3/3 | `ode-to-joy-90bpm.txt` (report `...-20261121-194134`) | 90 | 62 | 62 | 62 | 0 (analyser raw cut 1: 52 D6 X sounded 300 of 400 ms; offline check: borderline measurement effect, see below) | 0 | -3 to +1 c (sd 0.6 c) | no INVALID flag | "it was fine" |
| optional | `ode-to-joy-120bpm.txt` | 120 | 62 | | | | | | | |

### Wrong or missed notes

| Run | Index | Note | Fingering | Analyser row | Owner's remark | Suspected cause | Finding / profile proposal |
|---|---:|---|---|---|---|---|---|
| 60 BPM (by ear only) | 08 | A5 | A | NO SOUND (t 7.5 s) | not mapped (owner did not say which notes) | not interpreted | not interpreted |
| 60 BPM (by ear only) | 16 | B5 | Y | NO SOUND (t 16.5 s) | not mapped (owner did not say which notes) | not interpreted | not interpreted |
| 60 BPM (by ear only) | 24 | G5 | R + stick up | NO SOUND (t 24.5 s) | not mapped (owner did not say which notes) | not interpreted | not interpreted |
| 60 BPM (by ear only) | 39 | G5 | R + stick up | NO SOUND (t 39.5 s) | not mapped (owner did not say which notes) | not interpreted | not interpreted |
| 60 BPM (by ear only) | 40 | A5 | A | NO SOUND (t 40.5 s) | not mapped (owner did not say which notes) | not interpreted | not interpreted |
| 60 BPM (by ear only) | 41 | B5 | Y | NO SOUND (t 41.5 s) | not mapped (owner did not say which notes) | not interpreted | not interpreted |
| 60 BPM run 2 | 08 | A5 | A | NO SOUND | owner: nothing on screen during the silent notes, as if the button were not pressed (note-level mapping not given) | not interpreted | not interpreted |
| 60 BPM run 2 | 16 | B5 | Y | NO SOUND | owner: nothing on screen during the silent notes, as if the button were not pressed (note-level mapping not given) | not interpreted | not interpreted |
| 60 BPM run 2 | 47 | D5 | L | NO SOUND | owner: nothing on screen during the silent notes, as if the button were not pressed (note-level mapping not given) | not interpreted | not interpreted |
| 60 BPM run 2 | 48 | B5 | Y | NO SOUND | owner: nothing on screen during the silent notes, as if the button were not pressed (note-level mapping not given) | not interpreted | not interpreted |
| 60 BPM run 2 | 07 | B5 | Y | CUT SHORT | not given | not interpreted | not interpreted |
| 60 BPM run 2 | 15 | A5 | A | CUT SHORT | not given | not interpreted | not interpreted |
| 60 BPM run 2 | 30 | G5 | R + stick up | CUT SHORT | not given | not interpreted | not interpreted |
| 60 BPM run 2 | 38 | B5 | Y | CUT SHORT | not given | not interpreted | not interpreted |
| 60 BPM run 2 | 46 | A5 | A | CUT SHORT | not given | not interpreted | not interpreted |
| 60 BPM run 2 | 62 | G5 | R + stick up | CUT SHORT | not given | not interpreted | not interpreted |

Run 2 (second 60 BPM run, owner watching the screen; analysed live with the updated analyser, cut-short verdict and fixed extra-onset rule; report `spikes/recordings/ode-to-joy-60bpm-20261121-185906.{csv,md}`, git-ignored): sender exit 0, 3931 frames, 65.517 s; recorded 66.9 s; offset 0.371 s. Missed (NO SOUND): 08 A5 A, 16 B5 Y, 47 D5 L, 48 B5 Y. CUT SHORT: 07 B5 Y, 15 A5 A, 30 G5 R+stick-up, 38 B5 Y, 46 A5 A, 62 G5 R+stick-up. Extra onsets: 15 A5 (2). Owner's observation: "nothing appears on screen during the silent notes, as if the button were not pressed" (translated from Italian). Factual: 07 cut + 08 missed and 15 cut + 16 missed occurred in both 60 BPM runs.

Run 1 re-analysed with the new analyser (dev agent): missed 6 (unchanged); cut 6: 07 (670/1000 ms), 15 (730/2000), 23 (610/983), 30 (1720/1983), 48 (1070/1917), 62 (825/2000); extra onsets 2 (38 B5: 3). Note 38 was cut in run 1 by manual inspection (-16 dB at 39.03 s) but not flagged, because the loud non-ocarina sound masks the B5 band.

Run 3 (third 60 BPM run of `ode-to-joy-60bpm.txt`, `send_2wicc.py` only, not recorded, not analysed; done to check whether recording / `listen_trial.py` was involved): owner's ear "no, notes missing anyway" (translated from Italian); the owner did not say which notes. Context, not a result: the owner remarked that informal non-test melodies played earlier in the session with the same generator and sender (not recorded, not part of the story) sounded fine by ear.

Run 4 (diagnostic, not one of the story's melody runs): notes 01 to 16 of the 60 BPM melody shifted 4 s later (`spikes/recordings/diag/ode-to-joy-60bpm-n01-16-lead4000ms.txt`, generated with `gen_trials_1_6.py --set melody --bpm 60 --notes 1-16 --extra-lead-ms 4000`, 22.5 s; git-ignored location). The read-only firmware log `spikes/serial_rx_log.py` ran during the whole trial: 0 lines received (19:10:21 to 19:10:54; log `spikes/recordings/rx-20261121-191021.log`). Analyser (`--mode melody --latency 0.12`, offset 0.371): notes 16, audible 15/16, right pitch 15/16, missed 1, cut 1, extra onsets 1, cents spread -2 to +2 c (sd 0.8 c). Missed: 08 A5 A (NO SOUND, scheduled 11.5 s). Cut short: 07 B5 Y sounded 885 of 1000 ms (scheduled 10.5 s). Extra onsets: 05 D6 (2). Notes 15 and 16 played fine this time. Report `spikes/recordings/ode-to-joy-60bpm-n01-16-lead4000ms-20261121-191026.{csv,md}` (git-ignored). Owner did not comment on this run. Factual reading: the 07-cut / 08-missed event moved with the notes (about 7.3 s file time before, about 11.3 s now), not with elapsed time; no firmware messages were received. Context, not a result: the owner listened to an informal non-test ragtime (fast, short notes) and said it sounded fine; the owner believes the problem lies in the Ode to Joy file.

Run 5 (diagnostic bisect, not one of the story's melody runs): `spikes/recordings/diag/ode-to-joy-60bpm-n06-08-lead2000ms.txt` (notes 06 C6, 07 B5, 08 A5 of the 60 BPM melody, 2 s lead, 6.5 s; generated with `gen_trials_1_6.py --set melody --bpm 60 --notes 6-8 --extra-lead-ms 2000`; git-ignored location), played 3 times with `--mode melody --latency 0.12`. All three runs: notes 3, audible 3/3, right pitch 3/3, missed 0, cut 0, extra onsets 0. Reports `spikes/recordings/ode-to-joy-60bpm-n06-08-lead2000ms-20261121-191541`, `-191550`, `-191600` (`.csv`/`.md`, git-ignored). Factual reading: notes 06 to 08 alone do not reproduce the 07-cut / 08-missed event. Owner did not comment on these runs. Dev agent, offline: `serial_rx_log.py` exiting with code 1 on Ctrl+C was unintended and is fixed; the run 4 log was complete (0 lines). New diagnostic generator option `--max-press-ms N`.

Run 6 (diagnostic, not one of the story's melody runs): 17 identical A5 notes (A only, held about 55 frames, 5-frame same-button gaps), 60 BPM, 2 s lead, 20.5 s; `spikes/recordings/diag/a5x17-60bpm-lead2000ms.txt` (melody file `a5x17.melody`, generated with `gen_trials_1_6.py --set melody --bpm 60 --extra-lead-ms 2000 --melody spikes/recordings/diag/a5x17.melody --out-dir spikes/recordings/diag`; git-ignored location). Analyser (`--mode melody --latency 0.12`): notes 17, audible 17/17, right pitch 17/17, missed 0, cut 0, extra onsets 0, cents spread -2 to +0 c, sd 0.5 c. Report `spikes/recordings/a5x17-60bpm-lead2000ms-20261121-191849.{csv,md}` (git-ignored). Factual reading: a run of 17 long identical notes does not reproduce the event. Owner: "there is no staff on screen to look at" (translated from Italian; in this free-play setup no note staff is displayed). Dev agent, offline: an every-8-notes rule fits the data only loosely, and the dev agent's reading of the OoT decompilation (offline, not verified on hardware) shows no input lock or note cut tied to the 8-note free-play buffer. Owner's ear on this run: "anyway, between the 7th and the 8th note I heard a different pause" (translated from Italian); the analyser did not flag it. Gap measurement (dev agent, offline; offset 0.368 s, 5 ms window, gap = level more than 10 dB below the note's level): 15 normal pairs: tone falls 1071 to 1124 ms after the press, gap 4 to 51 ms (median about 31 ms). 07 -> 08: tone falls at 931 ms, gap 176 ms (deepest -22.9 dB). 14 -> 15: tone falls at 922 ms, gap 183 ms (-22.2 dB). Earliest audible release about 1040 ms (55-frame press plus about 120 to 130 ms latency), so notes 07 and 14 stopped before the key release. Onset intervals all 988 to 1013 ms (07 -> 08: 1001 ms); nothing was lost. The owner's remark is consistent with the 07 -> 08 gap; the analyser's cut-short rule did not flag it because it only checks up to 100 ms before the scheduled release. Dev agent's reading (hypothesis, mechanism unconfirmed): the game ends a held note on roughly every 7th sounded note, about 0.8 s after its onset; the next note is lost when it arrives by hand-over or within about 1 s; the pattern is not exact (run 1 group 25 to 30 had 6 notes; some expected events skipped).

Run 7 (diagnostic bisect, not one of the story's melody runs): `spikes/recordings/diag/ode-to-joy-60bpm-n01-08-lead2000ms.txt` (notes 01 to 08 of the 60 BPM melody, 2 s lead, 11.5 s; generated with `gen_trials_1_6.py --set melody --bpm 60 --notes 1-8 --extra-lead-ms 2000`; git-ignored location), played twice with `--mode melody --latency 0.12`. Run A (report `spikes/recordings/ode-to-joy-60bpm-n01-08-lead2000ms-20261121-192010`): notes 8, audible 7/8, right pitch 7/8, missed 1, cut 1, extra onsets 3 (04 D6: 3 onsets, 05 D6: 2); 07 B5 Y CUT SHORT, sounded 615 ms; 08 A5 A NO SOUND. Run B (report `...-20261121-192024`): notes 8, audible 7/8, right pitch 7/8, missed 1, cut 1, extra onsets 2 (04: 2, 05: 2); 07 CUT SHORT, sounded 710 ms; 08 NO SOUND. D6 (X) notes 04 and 05 had SNR about 13 to 14 dB versus about 28 to 30 for the others. Factual reading: notes 01 to 08 alone reproduce the 07-cut / 08-missed event (2/2), while notes 06 to 08 alone did not (0/3). Owner did not comment.

Run 8 (diagnostic, candidate fix, not one of the story's melody runs): `spikes/recordings/diag/ode-to-joy-60bpm-n01-08-lead2000ms-maxpress400ms.txt` (notes 01 to 08, 2 s lead, 11.5 s, every press capped at 400 ms with a neutral gap before every note, same onsets; generated with `gen_trials_1_6.py --set melody --bpm 60 --notes 1-8 --extra-lead-ms 2000 --max-press-ms 400`; git-ignored location), played once with `--mode melody --latency 0.12`: notes 8, audible 8/8, right pitch 8/8, missed 0, cut 0, extra onsets 2 (04 D6: 2, 05 D6: 2), cents spread -1 to +1 c, sd 0.3 c. Report `spikes/recordings/ode-to-joy-60bpm-n01-08-lead2000ms-maxpress400ms-20261121-192119.{csv,md}` (git-ignored). Factual reading: on the same notes that failed 2/2 with normal articulation, capped presses played all 8 notes in this single run (07 not flagged cut, 08 sounded). One run only; the analyser's cut rule may miss late stops (being tightened). Owner did not comment.

Run 9 (diagnostic, candidate fix on the full melody, not one of the story's melody runs): `spikes/recordings/diag/ode-to-joy-60bpm-maxpress400ms.txt` (62 notes, 65.5 s, every press capped at 400 ms with a neutral gap before every note, same onsets; generated with `gen_trials_1_6.py --set melody --bpm 60 --max-press-ms 400 --out-dir spikes/recordings/diag`; git-ignored location), `--mode melody --latency 0.12`, sender exit 0, 3931 frames, offset 0.376 s. Analyser: notes 62, audible 61/62, right pitch 61/62, missed 1, cut 0, extra onsets 5, cents spread -6 to +1 c, sd 1.1 c. Missed: 30 G5 R + stick up (NO SOUND, SNR 9.9 dB, just under the 10 dB audibility threshold). Extra onsets: 09 G5 (2), 24 G5 (2), 25 G5 (2), 45 G5 (2), 51 D6 (2). Report `spikes/recordings/ode-to-joy-60bpm-maxpress400ms-20261121-192320.{csv,md}` (git-ignored). Owner's ear: "when it took up the theme again at the end, the second note was silent" (translated from Italian). Discrepancy, stated as such: the owner's description points to the start of the final phrase (around note 48/49), while the analyser flagged note 30 and passed 48 and 49. Offline check (dev agent), after two analyser fixes (staccato notes judged up to heard release + 100 ms; extra attacks counted only before the heard release) and a tightened early-stop cut rule (tone at least 6 dB below its start, staying down until the heard release, fall starting 75 to 300 ms before it): re-analysis of the full capped run gives notes 62, audible 62/62, right pitch 62/62, missed 0, cut 0, extra onsets 1 (51 D6: 2). 30 G5 sounded normally (peak 25 dB, 410 ms); the live "NO SOUND" was an analyser artefact (a 400 ms note averaged over its 2 s slot). 48 B5 and 49 B5 both sounded at full level (31.6 / 31.4 dB, 444 / 430 ms); 48 is the 2-beat tied note, capped to about 0.4 s then about 1.6 s of silence, which plausibly explains the owner hearing a "silent second note" (a musical effect of the cap, not a drop). G5 extra onsets 09, 24, 25, 45 were release-tail artefacts. D6 (X) extra onsets (04, 05, 51) persist in every run including staccato: possibly real double attacks on X (X re-press gap unresolved in Story 1.4), unconfirmed.

Re-analysis of earlier recordings with the tightened rule: a5x17 cut 2 (07 sounded 800/917 ms, 14 780/917 ms); n01-08 `-192010`: missed 08, cut 07 (615/1000), extra 04 (3), 05 (2); n01-08 `-192024`: missed 08, cut 07 (710/1000), extra 04 (2), 05 (2); n01-08 maxpress400 `-192119`: 8/8, cut 0, extra 05 (2); full run 1 (`-184708`): cut 8 (07, 15, 23, 30, 38, 48, 55, 62); full run 2 (`-185906`): cut 6 (07, 15, 30, 38, 46, 62).

Working description (hypothesis, mechanism unknown): about every 7th sounded note the game ends a held note 0.54 to 0.91 s after its onset, then ignores input for up to about 1 s; capping presses at 400 ms avoided the audible effects in the runs so far (1 full run + 1 short run). Warning for Task 8 (U1): an early auto-stop near 0.8 s on attempts at positions 7, 14, and so on could be this event, not the sustain limit. Generator default (cap value, staccato vs re-press) was pending an owner decision. Owner decision (2026-11-21), replacing the pending generator default: for Story 1.6 the spike default is (a) staccato with presses capped at 400 ms (the tested variant); (b) splitting long notes into repeated presses of at most 400 ms is the preferred direction for the real pipeline later (follow-up, untested). The dev agent is regenerating the committed melody files with the cap; the U1 files stay uncapped.

90 BPM run 1 (story melody run, Task 7): committed `spikes/samples/story-1-6/ode-to-joy-90bpm.txt` (now with the 400 ms press cap), `--mode melody --latency 0.12`, sender exit 0, 2651 frames, 44.183 s, recorded 45.6 s, offset 0.373 s. Report `spikes/recordings/ode-to-joy-90bpm-20261121-193350.{csv,md}` (git-ignored). Numbers are in the "Melody runs" row. Owner's ear: "good, but the second note of the theme was missing when it takes it up again at the end" (translated from Italian). The analyser reports all notes sounded. Offline check (dev agent, replaces "pending"): notes 44 to 53 in `...-193350.wav` all sounded with clean attacks (peaks 21.7 to 31.7 dB above noise, 240 to 470 ms); 48 B5 and 49 B5 are two separate Y presses (frames 1910 and 1990, 0.93 s silence between), both sounded (428 / 424 ms). Nothing missing. Explanation (dev agent): the file used Beethoven's syncopated entry (B5 on beat 4 of bar 12 tied over the bar line); with the 400 ms cap it becomes a short early B5 then about 0.93 s of silence over the downbeat, heard as a missing note. D6 (51/52) late "onsets" are a measurement effect of the quiet, dipping X note; the X presses were on time.

Run 10 (trial of the alternative, not a story run): simplified common rhythm (`D5 2`, `B5 1` giving two B attacks on beats 1 and 2 of bar 13; same 62 notes, 64 beats), scratch file `ode-simplified-90bpm.txt` generated from a scratch copy of the melody, 90 BPM, capped: notes 62, audible 62/62, right pitch 62/62, missed 0, cut 0, extra onsets 1 (52 D6: 2), cents spread -3 to +2 c, sd 0.7 c. Report `spikes/recordings/ode-simplified-90bpm-20261121-193703.{csv,md}` (git-ignored). Owner's ear: "ok, now it sounds as I remember it, I heard all the notes" (translated from Italian). Owner decision (2026-11-21): adopt the simplified rhythm for the committed melody; the dev agent is updating the file. 90 BPM run 1 stays recorded as the syncopated version.

90 BPM official run 1/3 (Task 7): final committed `spikes/samples/story-1-6/ode-to-joy-90bpm.txt` (simplified rhythm, owner decision 6; 400 ms cap, owner decision 5), `--mode melody --latency 0.12`, sender exit 0, 2651 frames, 44.183 s, offset 0.375 s, no INVALID flag. Numbers are in the "Melody runs" row. Report `spikes/recordings/ode-to-joy-90bpm-20261121-193926.{csv,md}` (git-ignored). Owner's ear: "it was fine" (translated from Italian). The earlier run on the syncopated version (`...-193350`) stays in the table, labelled superseded.

90 BPM official run 2/3 (Task 7): same committed file and command, sender exit 0, 2651 frames, 44.183 s, offset 0.354 s, no INVALID flag. Numbers are in the "Melody runs" row. Report `spikes/recordings/ode-to-joy-90bpm-20261121-194033.{csv,md}` (git-ignored). Owner's ear: "it was fine" (translated from Italian).

90 BPM official run 3/3 (Task 7): same committed file and command, sender exit 0, 2651 frames, 44.183 s, offset 0.354 s, no INVALID flag. Numbers are in the "Melody runs" row. Cut short: 52 D6 X sounded 300 of 400 ms. Report `spikes/recordings/ode-to-joy-90bpm-20261121-194134.{csv,md}` (git-ignored). Owner's ear: "it was fine" (translated from Italian). The cut flag on 52 D6 (the quiet X note at the microphone) was unverified live; offline check (dev agent): note 52 D6 in `...-194134` fades (-3 to -4 dB at about 0.28 s, about -10 dB by the release at 0.40 s) rather than stopping abruptly; in `...-193926` and `...-194033` note 52 holds within about 2 dB until release; 52 is not at a 7-note-event position; D6 is the quiet, dipping note in every run; the owner heard nothing wrong. Dev agent's reading: borderline measurement effect of the 6 dB early-stop threshold on a soft X fade, not the 7-note event. Run 3/3 is recorded as 62/62 sounded with this remark; the analyser's raw "cut 1" stays visible. Also: sustain mode and `--combine` now list interruptions less than 2 s after onset as EARLY STOPS (treated as the 7-note event, excluded from counts, D_ok, T_stop and the control check; those holds are to be re-run).

Session close (2026-11-21, coordinator): the optional 120 BPM run was not run (coordinator decision, session ended). The 60 BPM runs (runs 1 to 3, plus diagnostics) were done only with the earlier uncapped, syncopated file; there is no 60 BPM run with the final committed file. The three official 90 BPM runs are the final file. Final profile value: `max_sustain_ms = 6000` (tag MARGIN; measured 6500 ms on A, margin 500 ms); L, R, Y, X not measured, assumed equal to A by owner decision 7. Still open: the cause of the held-note early stop (about every 7th sounded note) is a hypothesis with the mechanism unknown; the wavering numbers of the U1 trial were not reported; the 2wiCC_Comms version was not probed; the capture self-test was not run.

Offline analysis of the 60 BPM recording (dev agent, 2026-11-21), recorded as a **hypothesis, not a conclusion**:

- All 6 misses are real (game silent): raw level at the expected pitch and broadband at the noise floor during each gap.
- Pattern: a held note stops by itself about 0.55 to 1.45 s after its onset, then silence for about 1 to 3 s; presses inside the silence are lost; playback resumes on schedule. Stops: 07 B5 at 7.10 s (silent to 8.5 s, lost 08); 15 A5 at 15.05 s (to 17.5 s, lost 16); 23 A5 at 24.05 s (to 25.5 s, lost 24); 30 G5 at 31.95 s (to 32.5 s, none lost); 38 B5 at 39.05 s (to 42.0 s, lost 39, 40, 41); 48 B5 at 48.35 s (to 49.5 s, none lost). So 6 notes were also cut short; the analyser marked those `ok` (it checks presence and pitch, not length).
- At 39.1 to 39.8 s a loud wideband non-ocarina sound and at 41.0 to 41.5 s a low 300 to 500 Hz sound are in the recording; source unknown.
- Ruled out (by the dev agent): analyser windows and thresholds. Excluded by the dev agent's offline reading, not verified on hardware: OoT free-play song recognition (read-only reading of the zeldaret/oot decompilation); sender queue underrun (exit 0, notes after each gap on schedule). The same transition types play correctly elsewhere in the run.
- Working hypothesis (unproven): the injected controller link drops out for about 1 to 3 s, roughly every 7 to 9 s, plus the known first-input-after-reconnect quirk. Not seen in Story 1.4.
- Next (as stated by the coordinator): re-run 60 BPM with the owner watching the screen, and the Story 1.4 control `jitter-120bpm-16ths.txt`, before the 90 BPM runs and U1 (a dropout would corrupt U1).

### 1.10 U1 sustain, measured

U1 round 1 on A (2026-11-21, about 19:46 to 19:53): 5 files `spikes/samples/story-1-6/u1-sustain-A-part1..5.txt`, `--mode sustain --latency 0.12`, all sender exits 0. Combined with `--combine spikes/recordings/u1-sustain-A-part*-??????.csv` (5 files). 1000 ms (control) 0/5 interrupted; 4000, 5000, 5500, 6000, 6500 ms 0/5 each; 7000 ms 3/5 interrupted (auto-stop 6599, 6663, 6832 ms); 10000 ms (control) 5/5 interrupted (auto-stop 6810, 6831, 6874, 6937, 7157 ms). No-sound 0 everywhere; blanks silent (ok) in all 5 files; no EARLY STOPS listed. Combine line: D_ok = 6500 ms, T_stop = 6599 ms, candidate floor100(min(D_ok, T_stop) - 500) = 6000 ms for these rows only ("the rule needs every button"); confirmation on L, R, Y, X is still to run. Per file: part1 (194606) 10 s stop 6874; part2 (194728) 10 s 7157; part3 (194851) 7 s 6832, 10 s 6937; part4 (195014) 7 s 6599, 10 s 6831; part5 (195137) 7 s 6663, 10 s 6810 (ms). SNR range about 19.5 to 29.6 dB (lower in parts 4 and 5). Wavering numbers: not reported. Owner did not comment on the result. The new `max_sustain_ms` is not final.

U1 round 1 session (2026-11-21, started about 19:47, 5 parts on button A). Room noise for the U1 recordings: owner reported (translated from Italian) "if notes with a different pitch and sound show up in the recording, it was the air fryer"; an air fryer was running in the room, exact times not given.

| Button | Hold | Attempts | Interrupted | Auto-stop times (ms after the heard onset) | Wavering (level dB / pitch c per second vs the first) |
|---|---:|---:|---:|---|---|
| A | 4 s | 5 | 0/5 | none | not reported |
| A | 5 s | 5 | 0/5 | none | not reported |
| A | 5.5 s | 5 | 0/5 | none | not reported |
| A | 6 s | 5 | 0/5 | none | not reported |
| A | 6.5 s | 5 | 0/5 | none | not reported |
| A | 7 s | 5 | 3/5 | 6599, 6663, 6832 | not reported |
| A | 10 s (control) | 5 | 5/5 | 6810, 6831, 6874, 6937, 7157 | not reported |
| L, R, Y, X | candidate and next longer hold | 5 each | not measured | not measured | not measured (owner decision: assumed equal to A) |

Owner decision (2026-11-21): skip the U1 confirmation on L, R, Y, X. Owner's words (translated from Italian): "let's take it for granted that the values are the same for all buttons". Recorded as: L, R, Y, X not measured, owner decision: assumed equal to A. `max_sustain_ms` is measured on A only; the other buttons are an assumption. The dev agent is updating the profile (Part C) with 6000 ms; the final value is recorded when reported.

Update rule (owner decision 1, Story 1.6): D_ok = the largest tested hold where every attempt on every tested button was uninterrupted; T_stop = the shortest measured auto-stop time; candidate `max_sustain_ms` = floor100(min(D_ok, T_stop) − 500 ms); keep 5000 if the candidate is within ±100 ms of it.

| D_ok | T_stop | Candidate | Profile value | Buttons covered |
|---|---|---|---|---|
| 6500 ms (A only) | 6599 ms (A only) | 6000 ms by the rule, computed from A only | `max_sustain_ms = 6000` in `app/profiles/oot-switch2-1.1.1.toml` (tag MARGIN; measured 6500 ms on A, margin 500 ms), set by the dev agent | A measured; L, R, Y, X not measured, assumed equal to A (owner decision) |

### Lessons

Session over (2026-11-21). Entries below are what the analysers got wrong or what was learned, with the fix where one was made.

- Melody analyser, 60 BPM run (2026-11-21): the live run counted 18 false extra onsets (room clicks, noise in silences, onsets on silent notes). Fix: `count_reattacks` now needs a dip of at least 30 ms and a re-rise to within 10 dB of the pre-dip level; non-audible notes report 0 onsets. Re-analysis: extra onsets 18 -> 2 (only 38 B5, 3, coinciding with the loud sound at 39.1 s); misses and pitches unchanged. The "Melody runs" row above keeps the live-run numbers. A "cut short" verdict for melody mode is being added (the analyser checks presence and pitch, not length).

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
| F.0 | Skeleton smoke (Story 2.1): `pins.h` allow-list (release and debug pin sets), version macros agree with the version string, pin grep self-test (every bad fixture flagged, good fixtures and the real tree pass) | `ctest --no-tests=error` green | AD-1, AD-14, AD-20 |
| F.1 | Go-back-N through the link (Story 2.6, `test_link_gbn`): frames built with the real encoder and fed byte by byte; 300 in-order `QUEUE_EVENT`s across the seq wrap; `seq == expected` is the only path to the core (queue count, `expected`, `last_seq`); stale `d` = 1, 2, 63, 64 → `ACK(expected−1)` and no effect (a stale `QUEUE_EVENT` never enqueues twice); `d` = 65, 66, 128, 191, 255 → `NAK SEQ`, once per `expected`; `d` = 1 repeats the cached response (ACK or the same semantic NAK, e.g. `ORDER`); the window across the wrap (`expected` = 2: 255, 200, 194, 1 stale, 193 outside); a sync point empties the cache; `STOP` / `HELLO` executed at any `d` | matches AD-6 and `docs/link-protocol.md` "Rules the controller link applies" | AD-6    |
| F.2 | NAK matrix through the link (Story 2.6, `test_link_nak`): `CRC` from a corrupted frame and from an overlong run, `SEQ`, `FULL` (queue of 2) never advance `expected` and share one budget of one NAK per value of `expected` (a damaged frame followed by 10 in-flight frames → exactly one NAK); a new `expected` and a sync point re-arm it; `FULL` then a retry after a slot frees is accepted; range errors and empty runs get no reply; `VERSION`, `BUSY`, `ORDER`, `BAD_STATE`, `UNKNOWN_TYPE`, `ABORTED`, `NOT_READY` advance `expected` to `seq+1` with no effect on mode or queue; NAK fields `expected` (after handling) and `got` (= seq, 0 for `CRC`); core precedence pairs still hold (`BAD_STATE` over `ORDER`, `ABORTED` over `NOT_READY`); `NO_LINK` is Story 2.7 | matches AD-6 | AD-6 |
| F.3 | Mode table (Story 2.4, `test_core_modes`): hand-written matrix of 8 frame kinds (`SET_STATE`, `QUEUE_EVENT`, `START`, `STOP`, `HELLO`, `PING`, unassigned `0x07`, bridge `0x40`) × IDLE / DIRECT / PLAYING × latch set / clear × `ready()` true / false (reply, NAK reason, resulting mode, latch, `expected`, published state); entering IDLE by STOP, HELLO and every abort cause publishes NEUTRAL, clears the queue and resets the ordering baseline; the abort latch is set by every unrequested cause (link loss, app timeout, USB suspend / unmount / bus reset, core stall, END applied, underrun, LATE) from each mode, gives `NAK ABORTED` to `QUEUE_EVENT` / `START` / `SET_STATE`, survives PING, rejected frames, ticks and further aborts, and is cleared only by STOP / HELLO; `abort_count` counts clear-to-set only and wraps at 256; EVENT and `last_event` per cause | matches AD-8 | AD-8 |
| F.4 | END, underrun, LATE, ordering (Story 2.4, `test_core_end_late`): `NEUTRAL\|END` applied → `PLAYBACK_DONE`, IDLE, NEUTRAL, latch set; END on a non-neutral pose (button, hat, sticks, bit 15) → `NAK BAD_STATE`; reserved flag bits (0x02, 0x80, 0xFE, 0x03) → `NAK BAD_STATE` for `QUEUE_EVENT` and `SET_STATE`; `SET_STATE` with END and anything queued after an END → `NAK BAD_STATE`; underrun judged at apply time → the note then NEUTRAL in the same scheduler pass + `UNDERRUN`; head overdue by exactly one report period is applied, by one more µs → `LATE`, queue cleared, no backlog ever fired; 8000 µs fallback when the period is 0; nothing applied during the lead or in IDLE / DIRECT; `t_us` strictly increasing and < 2^31 else `NAK ORDER`, baseline reset on entering IDLE; one event per scheduler pass | matches AD-8 | AD-8 |
| F.5 | HELLO / STOP during PLAYING, with any seq (Story 2.4, `test_core_sync`): mid-piece with a held note and a non-empty queue, aborted, and from IDLE / DIRECT; seq 0, 1, 127, 128, 255 (wrap: `expected` = 0); STOP with a body and HELLO with any body length still act | NEUTRAL, queue cleared, → IDLE, latch cleared, `expected = seq+1`, reply ACK (STOP) / INFO (HELLO), no EVENT | AD-6, AD-8 |
| F.6 | Queue-mode allow-list (Story 2.4, `test_core_mask`): the 16 button bits one by one and all 65 536 `buttons` values, through the mask and through the full path (`QUEUE_EVENT` in PLAYING, scheduler, published state): only L, R, A, X, Y survive (ZL, ZR, B, Minus, Plus, L3, R3, Home, Capture, bits 14 / 15 cleared); right stick centred for all 256 × 256 values; left stick and hat 0–8 unchanged; flags never published; DIRECT (`SET_STATE`) unmasked; START from DIRECT publishes NEUTRAL. Allow-list confirmed by U11 (Story 1.1) | matches AD-9 | AD-9 |
| F.7 | Heartbeat gap during a 512-frame burst (bridge, fake clock)                                       | every gap ≤ 27 ms                                      | AD-4    |
| F.8 | Bridge framer: whole frames only, one in flight + one queued, overlong run dropped, partial frame discarded on DTR fall | matches AD-4                    | AD-4    |
| F.9 | Shared vectors (`firmware/common/proto/vectors/`, Story 2.2): COBS, CRC, frames (min/max length, CRC, length and range errors per receiver), every message (fields, body, frame; body-length errors), streams (back-to-back, leading delimiter, empty frames, resync, overlong runs, counters), canonical state and `NEUTRAL`, constants (both directions); the seven files are all present and every `proto_version` header equals `OCB_PROTO_VERSION` (also checked by `check-proto-version.sh` as its own CI step) | all pass; `PROTO_VERSION` = vector header | AD-14 |
| F.10 | `console_ready` with a fake profile (Story 2.6, `test_link_ready`): the fake profile posts to the USB mailbox as Story 2.3 will; STATUS `flags` bit1, the link's predicate and the `NOT_READY` gate sampled after every event and equal: 0 before the handshake, 1 after; suspend, unmount and bus reset each clear it and abort the core with one `EVENT USB_SUSPENDED` in IDLE, DIRECT and PLAYING (none when already aborted), and only a new handshake restores it; IN reports every 1 ms keep it 1; exactly 100 ms of silence keeps it 1, 100 ms + 1 µs clears it, one IN report restores it; the stall never aborts PLAYING; `START` / `SET_STATE` get `NAK NOT_READY` while it is 0; bits 2–7 always 0 (also with `link_ok`); mailbox counters wrap at 2^32 without a false event; two independent trackers | matches AD-19, AD-8 | AD-19, AD-12 |
| F.11 | `ocb_proto` API (Story 2.2): buffer limits (COBS and frame encoders never write past `out_cap`, body length 256 refused), every prefix of a frame decodes without reading past its end, counters add up to the delimiters seen, 10 000-frame randomised round trip and byte-at-a-time splitter equal to one-shot decoding, state predicates, unaligned message bodies; under ASan and UBSan | all pass, sanitizer-clean | AD-2, AD-3 |
| F.12 | Controller core edge cases (Story 2.4, `test_core_edge`): queue capacity 1, 2, 4, 512 and `NAK FULL` (transport: does not advance `expected`, not latched, retry accepted after a slot frees); NAK precedence `ABORTED` > `BUSY` > `NOT_READY` > `BAD_STATE` > `ORDER` > `FULL` pair by pair; `NOT_READY` only for `START` / `SET_STATE`, follows `ready()` live; `START` with an empty queue; hat 9–255; wrong body length for every frame → `NAK BAD_STATE`; unassigned app types → `NAK UNKNOWN_TYPE`; types `0x00`, `0x40`–`0x7F`, `0x80`–`0xFF` change nothing; STATUS view (`free`, `now_us` 0 in IDLE and during the lead, `last_seq = expected−1`); clock above 2^32; two independent instances; fixed-seed property test (3 seeds × 20 000 random frames, ticks, clock jumps, aborts) with the invariants checked after every step; under ASan and UBSan | all pass, sanitizer-clean | AD-6, AD-8 |
| F.13 | Controller core purity gate (Story 2.4, `core_pure_check`): `tools/check-core-pure.sh` flags every bad fixture (SDK, `hardware/`, `ocb_hw`, `<stdio.h>`, `<stdlib.h>`, clock reads, an allocation, `puts` / `putchar` / SDK stdio calls, forbidden headers in `.cpp` / `.cc` / `.S` / `.inc` files, a `CMakeLists.txt` linking `pico_*` / `hardware_*` / `ocb_hw`), passes the good fixture and the real `controller/core` tree, exits 2 on a usage error; also its own CI step | all pass | AD-2 |
| F.14 | Link RX stream (Story 2.6, `test_link_rx`): the same frames byte by byte, back to back, idle-separated and in random chunks through the RX ring all delivered; garbage before, between and after frames resyncs at the next delimiter; every replaced byte and every single-bit flip of a frame drops that frame and never the next; FE, PE, BE, OE alone and in all 15 combinations end the run in progress (the next frame is delivered without a leading delimiter) and are never data; breaks counted and the break's zero byte is no delimiter; overlong runs of 263 to 5000 bytes dropped once, next frame delivered; ring fill, overflow, gap marker, sticky flag and index wrap; every counter by hand | all pass, sanitizer-clean | AD-3, AD-4, AD-5 |
| F.15 | Link session (Story 2.6, `test_link_session`): before the first matching HELLO every app frame (also with a wild seq) gets `NAK VERSION` before the seq check, `expected = seq+1`; `STOP` accepted before HELLO; a matching HELLO from IDLE, DIRECT, PLAYING and aborted acts like STOP and is answered with INFO (every field: `proto_ver`, `fw_ver`, `bridge_ver` zeros then the last HEARTBEAT's, `hid_profile`, `queue_cap`, `app_timeout_ms`, `report_period_us` 0 then the set value); trailing HELLO bytes ignored; `proto_ver` 0, 2, 255 or an empty body (and the vector `hello_trailing`) → INFO, session closed, core neutral (also with a held note), next frame `NAK VERSION`; duplicate HELLOs answered; a rebooted controller facing seq 120 sends only `NAK VERSION`, the NAK slots overflow and count; the session survives aborts and USB events | matches AD-7 and `docs/link-protocol.md` | AD-7 |
| F.16 | Link TX and STATUS (Story 2.6, `test_link_tx`): all five pending → exactly EVENT, NAK, INFO, ACK, STATUS; rolling controller tx seq; first STATUS at once, then exactly every 50 ms over 1000 periods; ACK-triggered STATUS exactly 10 ms after the previous one; a 64-frame prefill burst in < 1 ms gives 64 ACKs and no STATUS, then STATUS never closer than 10 ms; one cumulative ACK when not drained; STATUS fields in IDLE (prefilled), DIRECT, PLAYING in and after the lead, after each external abort cause, `link_ok` and `console_ready` bits, reserved bits 0; 21 debug bytes with the debug flag (v1 prefix and the v2 split of `dropped_bytes`: error characters, receive-buffer overflow, overlong bytes, FE/PE/BE/OE; values by hand and saturation), none without; 9th EVENT and 5th NAK dropped and counted, ACK replaced; a too-small `cap` keeps the frame pending; the TX gate (opens after 1000 µs of continuous high, a low sample restarts the wait, open stays open, `release` closes at once, closed discards and counts); two independent links | all pass | AD-19, AD-20 |
| F.17 | Link fuzz and lossy channel (Story 2.6, `test_link_fuzz`): 3 seeds × 200 000 steps of random bytes, damaged and duplicated frames, `0x00` runs, 300-byte runs, error flags, ring overflows, clock jumps, aborts, USB events and gate releases, with HELLO and traffic again; after every step: core fields in range, `expected` changes only with a delivered frame, PLAYING inside the allow-list, every TX frame valid, at most one transport NAK per `expected`, counters monotonic, a valid frame after a `0x00` or an error byte always delivered. Lossy channel: a reference AD-6 sender over a FIFO wire at 460800 baud with drops, bit flips, duplicates, junk and latency both ways at 0, 1, 5, 10 %; queue 512 and 5: every `QUEUE_EVENT` enqueued exactly once and in order, never over capacity, done within the time bound. `OCB_FUZZ_SEED` replays | all pass, sanitizer-clean | AD-3, AD-6 |
| F.18 | Controller link purity gate (Story 2.6, `link_pure_check`): `tools/check-link-pure.sh` flags every bad fixture (SDK, `hardware/uart.h`, `hardware/irq.h`, `ocb_hw/uart_link.h`, `<stdio.h>`, `<stdlib.h>`, a clock read, an allocation, `multicore_`, the SDK RAM macro, a `CMakeLists.txt` linking `hardware_uart`, an SDK include in a `.cpp`), passes the good fixture and the real `controller/link` tree, exits 2 on a usage error, and its banned identifier list equals the core gate's; also its own CI step | all pass | AD-2 |
| F.19 | Bench sender self-test (Story 2.6, `link_probe_selftest`): `tools/link_probe.py`'s COBS, CRC, frame and message codec against `cobs.txt`, `crc16.txt`, `frames.txt`, `messages.txt` (every vector), its splitter, and its AD-6 sender against an in-process fake link over a lossy wire (exactly once, in order); stdlib only, run with `python3 -I` | all pass | AD-6, AD-14 |
| F.20 | Bench soak on a fake clock (Story 2.6 D30, D31, `link_probe_selftest`): the probe's own `soak` command (a) for 150 cycles (> 30 s of fake time) against the in-process fake link: every cycle sends and passes; (b) against the real controller link (`ocb_link_sim`, built from the firmware sources and loaded with ctypes) through a model of the 2wiCC_Comms bridge (1 KB USB-to-UART ring dropping on overflow, no flow control, UART at 460800 both ways): clean soak PASS with 0 NAK, 0 resend and 0 byte dropped by the bridge; impaired soaks (seeds 1 and 2) PASS; unpaced positive control shows bridge drops and NAKs; (c) the same bridge draining at 85 % of the line rate: with the ACK-clocked in-flight cap (D33) clean PASS with 0 NAK and 0 bridge drop, impaired seed 1 PASS with 0 bridge drop, and without the cap the bridge drops; the probe's decoder of the debug counters (v1 and v2) by hand; out-of-range `--pace-share` / `--max-inflight` refused by argparse (exit 2) | all pass | AD-6 |

## Hardware-in-the-loop checks (M3/M4)

| ID  | Test                                                                                                   | Pass criterion                                   | Result |
|-----|--------------------------------------------------------------------------------------------------------|--------------------------------------------------|--------|
| H.0 | Skeleton LED check (Story 2.1): flash `bridge.uf2`, then `controller.uf2`, on a Zero each        | bridge: blue, 1 Hz; controller: amber, 2 Hz; the two are told apart at a glance; LED not dazzling at arm's length; nothing else on the board changes (no other pin) | PASS 2026-11-21 (UF2s from CI run 44946942547, commit 25aa066; flashed via BOOTSEL, boards off the breadboard). Bridge: blue, ~1 Hz. Controller: amber, ~2 Hz (20 flashes in 10 s); looks whitish head-on, clearly yellow-orange off-axis or through paper. |
| H.1 | Bridge unpowered, controller on the Switch: measure the voltage at the bridge's GP1. Then, both boards powered (Story 3.1 bridge firmware; story 2.6 D25): power the bridge first and the controller second, then the reverse order, 5 times each; and with the controller unpowered and the bridge powered, measure the controller's 3V3 pin and GP1 | ≈ 0 V: the controller has released TX (no back-powering). Both powered: the link comes up in either order within 1 s (`hello` answered, STATUS flowing), never a deadlock with both TX high-Z; unpowered controller: its 3V3 pin stays ≈ 0 V while the bridge probes (≤ 1 % duty pulses on GP1). Record: each voltage, the link-up time per order, whether the probe pulses are visible (scope or meter), and whether 1 kΩ series resistors are needed | |
| H.2 | Core stall (debug build: hang core 0, then core 1)                                                     | watchdog reboot, USB detach, no input left pressed | |
| H.3 | `console_ready` on the Switch 2 (P2): plug in with Wired Communication ON, then put the console to sleep and wake it, then switch Wired Communication OFF and ON | flag 0 until the handshake, 1 after; back to 0 within 150 ms on sleep and on Wired Communication OFF; 1 again after wake / ON; `START` gets `NAK NOT_READY` whenever the flag is 0 | |
| H.4 | UART bring-up (Story 2.6): HELLO, INFO, STATUS cadence, ACK, `NAK VERSION`, `NOT_READY`. 1. Power the controller (fw-debug); LED amber 2 Hz as in H.0. 2. `link_probe.py watch --port P` for 5 s before any HELLO: STATUS frames (or none yet), no crash. 3. `link_probe.py hello`: INFO arrives. 4. `watch --seconds 30`: STATUS interval statistics. 5. `fill --count 10 --stop`: ACKs, `free` = cap−10, `last_seq` correct, then `free` back to 512 after STOP. 6. `try`: `SET_STATE` and `START` both get `NAK NOT_READY` (no USB profile yet). 7. Power-cycle the controller, `fill --no-hello`: `NAK VERSION`; then `hello` and `fill` work | `dropped_bytes` may read 1 from boot (the line settling when the UART takes the pin); INFO `proto_ver` 1, `fw_ver` = the build's version, `bridge_ver` 0.0.0, `hid_profile` 2, `queue_cap` 512, `app_timeout_ms` 1000, `report_period_us` 0; STATUS interval median 50 ms, p95 ≤ 60 ms, never closer than 10 ms; `flags` 0 (no heartbeat sent, console not ready, bits 2–7 0); `NAK VERSION` at step 7; LED still blinking. Record: INFO bytes, STATUS interval stats (median, p95, min), the NAK lines, LED observation, the probe's output saved as a log | |
| H.5 | Reliability soak on the real wire (Story 2.6): `link_probe.py --port P --seed 1 soak --cycles 40 --events 500 --corrupt 0.02 --drop 0.02 --garbage`, then the same with `--seed 2`, then `link_probe.py --port P soak --cycles 40 --events 500` (no impairment). `--seed` and `--port` go before the subcommand; writes are paced to 95 % of the UART rate and at most 960 bytes of unconfirmed QUEUE_EVENTs are on the wire by default (story 2.6 D30, D33; `--pace-share`, `--max-inflight`, `--no-pace`); flash the fw-debug build so STATUS carries the debug counters including the v2 split (D35) | each run ends with the probe's PASS line (every event enqueued exactly once and in order, checked with `free` and `last_seq` after each cycle; no deadlock; 20 000 events per run) and no `cycle N: FAIL` line; the impairments are host-side, so they show as NAKs and resends, not as `dropped_bytes`: debug `dropped_bytes` does not rise in any run, and its causes `rx_err_bytes`, `rx_overflow_bytes`, `overlong_bytes`, `rx_fe`, `rx_pe`, `rx_be`, `rx_oe` stay at their boot values (a `cycle N: dropped_bytes a -> b (...)` line names the cause: error bits = UART errors on the wire, check GND continuity and the wiring; `rx_overflow_bytes` = the controller's receive buffer overflowed; `overlong_bytes` = a long run without a delimiter, i.e. bytes lost before the controller); record the cause line and the cycle; `tx_dropped` reported (not a failure by itself); `abort_count` unchanged; non-impaired run: 0 NAKs and 0 resends. First run (2026-11-22, unpaced probe): FAILED, see story 2.6 Debug Log. Record: the three PASS lines, the "debug counters first/last" lines (all eleven counters), any `cycle N: dropped_bytes` line, retransmission and NAK statistics, run time | |
| H.6 | UART error bits and break seen and survived (Story 2.6): 1. Session open (H.4 step 3). 2. Jumper the controller's RX pad to GND for about 50 ms, then remove it (RX is an input; no 3V3 anywhere). 3. `watch`: debug `dropped_bytes` rises by at least 1. 4. `fill --count 5` right after, no HELLO. 5. Repeat the jumper 10 times with a `fill` after each. 6. (Adapter only, optional) `noise --count 2000` and a break from the adapter while a `soak` runs | `dropped_bytes` rises on each break; the frame after the break is always accepted (ACK, `free` consistent); the session stays open (no `NAK VERSION`); STATUS keeps coming. Record: counter before/after each jumper, the 10 results, whether the optional step ran | |
| H.7 | Power-up TX high-Z and RX pull-down, controller-only subset of H.1 (Story 2.6, AD-20): 1. Both USB cables unplugged; only GND between controller and peer, a multimeter (DC V) on the controller's TX pad and GND, the controller's RX pad open. 2. Power the controller only: read TX. 3. Read RX. 4. Connect the powered, idle peer TX to the controller's RX: read the controller's TX within 1 s. 5. Disconnect the peer TX again: read TX. 6. Repeat 4–5 once. 7. Session open, jumper RX to GND for 50 ms (as H.6), read TX right after. Also, with the peer unpowered and wired to the controller's TX: the peer's RX reads about 0 V | step 2: TX about 0 V or a floating few tenths of a volt (high-Z), **not** a solid 3.3 V; step 3: RX about 0 V (pull-down); step 4: TX goes to about 3.3 V; step 5: TX stays high (the release on link loss is Story 2.7; not a failure here), record it; step 7 informational; unpowered peer's RX ≈ 0 V (no back-powering, risk C5). The adapter idles high, so it plays the leading role the bridge has in D25; the both-boards-powered case is H.1. Record: meter model, each voltage, a note per step, the delay at step 4; if 1 ms of idle-high is wrong, the value to set in `OCB_LINK_TX_IDLE_HIGH_US` | |

## Later: acceptance tests

| ID  | Test                                                                                    | Related        |
|-----|-----------------------------------------------------------------------------------------|----------------|
| A.1 | Reference piece (1 min, uses all 19 pitches) plays with 0 wrong notes                   | NFR-04, NFR-08 |
| A.2 | Same on Windows, Linux, macOS                                                           | NFR-01         |
| A.3 | Unplug the bridge mid-song, and separately pull the UART wire mid-song (20× each) → neutral within 100 ms every time; also check that the controller releases TX (high-Z) after the break / link loss | NFR-05, AD-5, AD-20 |
| A.4 | Flash both boards from the host app on each OS                                          | FR-09          |
| A.5 | First-time user (not the developer) unblocks the unsigned app with the README, then flashes and plays a song with the GUI only, no other help | FR-15, NFR-10  |

## Related

- Bonooru: Roadmap (author's notes)
- Bonooru: Requirements (author's notes)
- Scripted sender for Spike 0b and Spike 2: [`spikes/README.md`](../spikes/README.md)
