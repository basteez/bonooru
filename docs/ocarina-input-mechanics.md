# Ocarina of Time (Switch 2): ocarina input mechanics

*Converted from the author's org-roam notes on 2026-11-19. This file is now the reference in the repo.*

Topic: Ocarina Bot (author's notes)

*The Legend of Zelda: Ocarina of Time* remake, Switch 2 exclusive, released 2026-11-05. Collected from community sources, then checked by hand in game on 2026-11-20 (Story 1.1; Switch 2 system 23.5.0, OoT 1.1.1, Joy-Con 2, phone tuner). Stick deflection (U6) and all 45 button × modifier combinations measured on 2026-11-20 (Story 1.3; 2wiCC spike sender, pitch from the MacBook microphone with `spikes/listen_trial.py`, cross-checked with a phone tuner). Measured data: [Test plan](test-plan.md), "Spike 1 results" and "Story 1.3 field trials".

## Note buttons **[VERIFIED 2026-11-20]**

| Button | Pitch (sounding, measured) | Measured Hz (Story 1.3, test-plan 1.9) | Community chart name | N64 equivalent |
|--------|----------------------------|----------------------------------------|----------------------|----------------|
| L      | D5 (587 Hz)                | 596.6                                  | D4                   | A              |
| R      | F5 (698 Hz)                | 702.6                                  | F4                   | C-down         |
| A      | A5 (880 Hz)                | 894.3                                  | A4                   | C-right        |
| Y      | B5 (987 Hz)                | 1003.7                                 | B4                   | C-left         |
| X      | D6 (1174 Hz)               | 1190.1                                 | D5                   | C-up           |

**Game tuning [VERIFIED 2026-11-20]:** the game sounds about **+27 to +29 cents** sharp of equal temperament with A4 = 440 Hz: A5 measures about 894 to 896 Hz (microphone, and a phone tuner's cents indicator read about +27 c). The Hz in brackets in the "Pitch" column are nominal note frequencies, not measurements. Per-note readings vary around it: L, A, Y about +18 to +35 c, R about +6 to +16 c, X +7 to +41 c (see the X D-pad note below); not investigated, it never changes a note name.

The button → note pairing of the community chart is right, but the game **sounds one octave above** the chart's names. The project uses the **sounding pitch** everywhere (profile, UI, docs); the chart names are kept only for cross-reference with community guides.

## Pitch modifiers (free play) **[VERIFIED 2026-11-20]**

| Modifier         | Shift             |
|------------------|-------------------|
| D-pad ↑ / ↓      | +1 / −1 semitone  |
| Left stick ↑ / ↓ | +2 / −2 semitones at full travel, graded below it (see U6) |
| both together    | up to ±3          |

- Hold the modifier **before** pressing the note button (verified; the minimum lead is U5, still open)
- Changing a modifier while a note sounds retunes it: the D-pad steps at once, the left stick glides (by ear). So the player must change modifiers only between notes, unless a bend is wanted
- Songs are recognised **by pitch**, not by button: an alternative fingering of a song note still triggers the song (Spike 1.8)
- Range: B4–F6 sounding (B3–F5 in the community chart), chromatic, 19 pitches; all 19 verified.
- 45 combinations (5 buttons × 9 modifier states: D-pad none/↑/↓ × stick none/↑/↓) **[VERIFIED 2026-11-20]**: all 45 give the model pitch (button + D-pad ±1 + stick ±2) by nearest note, for all five buttons. The nine states give 7 distinct offsets (−3 to +3), so the 45 inputs cover the 19 pitches with 1 to 4 fingerings each (table below). Test plan row 1.9.
- The left stick is a **graded bend**, not a switch: ±2 semitones only at full vertical travel (U6). Stick modifiers therefore always use full travel with `lx` centred: profile `stick_value` up = `ly 0x00` (wire Y 0xE00), down = `ly 0xFF` (wire Y 0x20C).
- On X the D-pad shifts by about ±115 c (D↑ +114 to +117 c, D↓ −115 to −119 c from the bare button) instead of about 100 c, so X + D↓ and X + D↑ + stick↓ (C♯6) differ by about 30 c, as do X + D↑ and X + D↓ + stick↑ (D♯6). Repeated and confirmed (test plan 1.9); each still reads as the right note. Cause not investigated; the fingering optimiser can prefer the variant closer in tune if it matters.
- Bends outside free play: not measured, out of scope (the bot only plays in free play).

## Fingering chart (one option per pitch) **[VERIFIED 2026-11-20]**

Pitches are sounding pitches (one octave above the community chart).

| Pitch | Input           | Pitch | Input           |
|-------|-----------------|-------|-----------------|
| B4    | L + D↓ + stick↓ | G♯5   | A + D↓          |
| C5    | L + stick↓      | A5    | A               |
| C♯5   | L + D↓          | A♯5   | A + D↑          |
| D5    | L               | B5    | Y               |
| D♯5   | L + D↑          | C6    | Y + D↑          |
| E5    | R + D↓          | C♯6   | X + D↓          |
| F5    | R               | D6    | X               |
| F♯5   | R + D↑          | D♯6   | X + D↑          |
| G5    | R + stick↑      | E6    | X + stick↑      |
|       |                 | F6    | X + D↑ + stick↑ |

All 19 fingerings above measured with a tuner (Spike 1.2) and again with the microphone (Story 1.3). Every alternative is listed in the next section; the full set goes into the mapping profile, see [Score format and note mapping](score-format-and-mapping.md).

## Alternative fingerings (all 45 combinations) **[VERIFIED 2026-11-20]**

Measured in free play with the stick at full travel (`ly` 0x00 up, 0xFF down, `lx` centred), one press each, pitch from the microphone (test plan, "Story 1.3 field trials", 1.9). Hz are measured medians (the game sounds about +27 c sharp of A4 = 440 Hz, see above). **Bold** = the chart fingering. All 45 matched the model by nearest note; no combination gave a different note, no sound, or a wrong octave.

| Pitch | Count | Inputs (measured Hz) |
|-------|------:|----------------------|
| B4 | 1 | **L + D↓ + stick↓** (499.9 Hz) |
| C5 | 1 | **L + stick↓** (530.1 Hz) |
| C♯5 | 2 | **L + D↓** (560.9 Hz); L + D↑ + stick↓ (564.2 Hz) |
| D5 | 2 | **L** (596.6 Hz); R + D↓ + stick↓ (592.8 Hz) |
| D♯5 | 3 | **L + D↑** (633.3 Hz); L + D↓ + stick↑ (628.9 Hz); R + stick↓ (624.4 Hz) |
| E5 | 3 | L + stick↑ (668.6 Hz); **R + D↓** (665.2 Hz); R + D↑ + stick↓ (663.5 Hz) |
| F5 | 2 | L + D↑ + stick↑ (710.6 Hz); **R** (702.6 Hz) |
| F♯5 | 3 | **R + D↑** (743.5 Hz); R + D↓ + stick↑ (744.6 Hz); A + D↓ + stick↓ (752.1 Hz) |
| G5 | 2 | **R + stick↑** (788.0 Hz); A + stick↓ (797.4 Hz) |
| G♯5 | 4 | R + D↑ + stick↑ (834.3 Hz); **A + D↓** (843.5 Hz); A + D↑ + stick↓ (845.5 Hz); Y + D↓ + stick↓ (845.5 Hz) |
| A5 | 2 | **A** (894.3 Hz); Y + stick↓ (893.6 Hz) |
| A♯5 | 4 | **A + D↑** (950.1 Hz); A + D↓ + stick↑ (946.8 Hz); Y + D↓ (951.3 Hz); Y + D↑ + stick↓ (949.7 Hz) |
| B5 | 3 | A + stick↑ (1001.9 Hz); **Y** (1003.7 Hz); X + D↓ + stick↓ (992.3 Hz) |
| C6 | 4 | A + D↑ + stick↑ (1067.5 Hz); **Y + D↑** (1065.9 Hz); Y + D↓ + stick↑ (1066.4 Hz); X + stick↓ (1061.7 Hz) |
| C♯6 | 3 | Y + stick↑ (1124.2 Hz); **X + D↓** (1113.5 Hz); X + D↑ + stick↓ (1135.2 Hz) |
| D6 | 2 | Y + D↑ + stick↑ (1194.3 Hz); **X** (1190.1 Hz) |
| D♯6 | 2 | **X + D↑** (1273.2 Hz); X + D↓ + stick↑ (1251.8 Hz) |
| E6 | 1 | **X + stick↑** (1338.1 Hz) |
| F6 | 1 | **X + D↑ + stick↑** (1428.2 Hz) |

19 pitches, 45 inputs. Two alternatives were also heard inside a song (B5 = A + stick↑, D6 = Y + D↑ + stick↑, Spike 1.8).

## Unknowns to measure

| ID  | Question                                                               | Impact                     | Answer (2026-11-20, by hand) |
|-----|------------------------------------------------------------------------|----------------------------|------------------------------|
| U1  | Does holding the button sustain the note? Max length?                  | note durations, legato     | Yes, while held; the tone wavers from ~2 s and the note stops by itself at ~6 s even if still held. Longer notes are re-pressed (`max_sustain`, FR-05a) |
| U2  | Same button twice: minimum release gap to retrigger?                   | repeated notes             | open (Story 1.4)             |
| U3  | Pressing a new button while one is held: cut, or ignored?              | legato phrasing            | Cut: the last pressed button sounds. Releasing it while the first is still held makes the first note sound again, so the old button must be released |
| U4  | Modifier changed while a note sounds: bend (glide) or no effect?       | possible portamento effect | D-pad: instant step (A5 → A♯5, 932 Hz). Left stick: gradual glide (by ear). Portamento is possible with the stick |
| U5  | Minimum lead of a modifier before the button                           | fingering cost             | open (Story 1.4)             |
| U6  | Stick threshold: how much deflection counts as ↑/↓? Diagonals?         | stick values in profile    | **No threshold: the stick is a graded bend** (Story 1.3, test plan 1.6). With the stick set before the press, the pitch is about linear in cents with vertical deflection, about 25 c per 0x10 step (about 2 c per percent of travel), no dead zone (12 % already bends about ±24 c), slight saturation in the last step to full travel; ±200 c (the full ±2 semitones) only at full travel: up `ly 0x00` = wire 0xE00 (A5 → 1002 Hz, +199 c), down `ly 0xFF` = wire 0x20C (→ 797 Hz, −197 c). 50 % (0x40 / 0xC0) gives about ±105 c, 75 % (0x20 / 0xE0) about ±160 c: none of them counts as a clean ±2. Up and down are mirrored within a few cents (recorded separately for Story 1.5). The X axis alone has no effect; diagonals and square corners clamp to about ±142 c (consistent with the vector being clamped to the unit circle, bend from its Y component), so use a pure vertical stick. Profile `stick_value`: up = `ly 0x00`, down = `ly 0xFF`, `lx` 0x80 |
| U7  | Minimum press length that always sounds                                | fast passages              | open (Story 1.4)             |
| U8  | Free play limits: note count, auto-exit, timeouts                      | long pieces                | None seen: 100 notes in a row and 1 min idle, free play stays open until the player exits. An official song played in free play is recognised (by pitch) |
| U9  | How to enter/leave free play (item button, quick-swap, B to exit)      | navigation macro           | Enter: D-pad ← (fixed binding, not an item slot); note input is accepted only after ~2 s. Exit: B or D-pad ← (D-pad ← toggles) |
| U10 | Frame rate while playing (30/60 fps)                                   | quantisation grid          | open (Story 1.4)             |
| U11 | Does the game ever use a 2nd octave or other buttons (ZL/ZR, R-stick)? | range                      | No other note input or octave. ZL opens the known-songs menu (ZL + A previews a song), ZR does nothing, the right stick moves the camera. AD-9 allow-list (L, R, A, X, Y, hat, left stick) **confirmed** as is |

Tests: [Test plan](test-plan.md).

Other observation (Story 1.3, seen several times): after the controller is connected or powered on, or after the Joy-Con are detached, the first injected input is ignored and the second works (on the Home menu, and on the "press A when the controller is ready" prompt). Relevant to the readiness flow, not to note mapping.

## Other input paths

- Microphone: humming or a real instrument into the Switch 2 mic is officially supported (Nintendo says it doesn't make progress easier). Out of scope, noted in Architecture (author's notes).

## Risks

### Mapping changes with a game update **[RISK]**

Mitigation: mapping lives in a profile file, never in code.

## Sources

- [jiaqhuan/zelda-ocarina-switch2](https://github.com/jiaqhuan/zelda-ocarina-switch2) (+ `docs/technical-analysis.md`)
- [Zelda Central: beginner's guide and controls](https://zeldacentral.com/games/ocarina-of-time-remake/beginners-guide/)
- [Questfern: all notes on Switch 2](https://questfern.com/oot/guides/ocarina-songs/)
- In-game check by hand, 2026-11-20: [Test plan](test-plan.md), "Spike 1 results"

## Related

- [Ocarina Bot: USB controller emulation](usb-controller-emulation.md)
