# Ocarina Bot: Test plan

*Converted from the author's org-roam notes on 2026-11-19. This file is now the reference in the repo.*

Topic: Ocarina Bot (author's notes)

Spikes first, all with **existing** firmware (no code of ours yet). Record for every run: Switch 2 system version, game version, port/cable, firmware + version, result.

## Hardware safety (before every session)

- Wiring = RiiDraw's (ADR-001 (author's notes)): only GP0, GP1, GND; nothing on 5V or 3V3
- Rewire only with both USB cables unplugged
- Switch on dock or stand, short cable on the Switch side, slack on the PC side
- Details: Risk register (author's notes)

## Test environment

| Item            | Value                                                                                                                           |
|-----------------|---------------------------------------------------------------------------------------------------------------------------------|
| Switch 2 system | 23.5.0 (Spike 1 by hand, 2026-11-20)                                                                                            |
| OoT version     | 1.1.1                                                                                                                           |
| Controller      | Joy-Con 2 pair (Spike 1 by hand; D-pad = Joy-Con 2 (L) direction buttons)                                                       |
| Measurement     | Soundcorset tuner (iOS), A4 = 440 Hz; Switch 2 and phone hand-held (table caused resonances), mic ~2 cm from the right speaker  |
| Host OS(es)     |                                                                                                                                 |
| Cable(s)        |                                                                                                                                 |
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

## Spike 1: ocarina mapping

Uses spike 0 or 0b in real-time mode (terminal or GLaMS web UI). Rows 1.1–1.5, 1.7, 1.8 can run **first, by hand with a real controller**: no hardware needed, and they settle the riskiest unknowns (Risk register (author's notes) R1, R2).

|  ID | Check (see [unknowns](ocarina-input-mechanics.md))                   | Method                                                                                                                                               | Result |
|----:|----------------------------------------|------------------------------------------------------------------------------------------------------------------------------------------------------|--------|
| 1.1 | 5 natural notes and their pitch        | press each, tuner app on the phone / record                                                                                                          | pairing confirmed; sounds one octave above the chart (L = D5 … X = D6), see results below |
| 1.2 | all 19 fingerings of the chart         | modifier first, then button                                                                                                                          | all 19 confirmed, one octave above the chart (B4–F6 sounding); modifiers ±1 / ±2 / ±3 confirmed |
| 1.3 | U1 sustain                             | hold 1 s, 3 s, 10 s                                                                                                                                  | sustains while held; wavers from ~2 s; auto-stops at ~6 s |
| 1.4 | U3 new button while holding            | L held, then press R                                                                                                                                 | new button cuts the held note (F5 starts, D5 stops); releasing R with L held brings D5 back |
| 1.5 | U4 bend while sounding                 | hold A, then D-pad ↑                                                                                                                                 | D-pad: instant step to A♯5 (932 Hz); left stick: gradual glide (by ear) |
| 1.6 | U6 stick threshold                     | ly = 0x40, 0x20, 0x00                                                                                                                                |        |
| 1.7 | U8, U9 free play enter/exit and limits | play 100 notes in a row                                                                                                                              | enter D-pad ← (input after ~2 s), exit B or D-pad ←; 100 notes and 1 min idle: no limit, no auto-exit |
| 1.8 | official song recognised in free play? | play a song's natural notes (e.g. Zelda's Lullaby) in free play; then the same with one note on an alternative fingering (e.g. D4 = R + D↓ + stick↓) | yes; matched by pitch, not button: alternative fingerings (B, then D) still trigger it, even very slow |

Done when: the mapping profile has no `HYPOTHESIS` markers left for pitches and modifiers.

### Spike 1 results (by hand, 2026-11-20)

Setup: see Test environment (Switch 2 23.5.0, OoT 1.1.1, Joy-Con 2 pair, Soundcorset on the Switch 2 speakers). Pitches are the sounding pitch read by the tuner (Hz shown to confirm the octave).

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

## Spike 2: timing limits

Needs a scripted sender (any tiny script over the serial port is fine for a spike).

|  ID | Measure                             | Sweep                                                      | Pass threshold      |
|----:|-------------------------------------|------------------------------------------------------------|---------------------|
| 2.1 | U7 min press that always sounds     | 1…10 frames (16.7 ms steps), 20 tries each                 | 20/20 sound         |
| 2.2 | U2 min gap to retrigger same button | 1…10 frames                                                | 20/20 retrigger     |
| 2.3 | U5 modifier lead                    | 0…5 frames                                                 | 20/20 correct pitch |
| 2.4 | Onset jitter                        | 120 BPM 16ths, 30 s, record + onset detection (e.g. aubio) | σ ≤ 1 frame         |
| 2.5 | U10 frame rate                      | count frames in a 60 fps capture                           | –                   |

Done when: the `[timing]` section of the profile is filled with measured values + safety margin.

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
