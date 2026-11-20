# Ocarina of Time (Switch 2): ocarina input mechanics

*Converted from the author's org-roam notes on 2026-11-19. This file is now the reference in the repo.*

Topic: Ocarina Bot (author's notes)

*The Legend of Zelda: Ocarina of Time* remake, Switch 2 exclusive, released 2026-11-05. Collected from community sources, then checked by hand in game on 2026-11-20 (Story 1.1; Switch 2 system 23.5.0, OoT 1.1.1, Joy-Con 2, phone tuner). Measured data: [Test plan](test-plan.md), "Spike 1 results". Sections still marked `HYPOTHESIS` were not measured.

## Note buttons **[VERIFIED 2026-11-20]**

| Button | Pitch (sounding, measured) | Community chart name | N64 equivalent |
|--------|----------------------------|----------------------|----------------|
| L      | D5 (587 Hz)                | D4                   | A              |
| R      | F5 (698 Hz)                | F4                   | C-down         |
| A      | A5 (880 Hz)                | A4                   | C-right        |
| Y      | B5 (987 Hz)                | B4                   | C-left         |
| X      | D6 (1174 Hz)               | D5                   | C-up           |

The button → note pairing of the community chart is right, but the game **sounds one octave above** the chart's names. The project uses the **sounding pitch** everywhere (profile, UI, docs); the chart names are kept only for cross-reference with community guides.

## Pitch modifiers (free play) **[VERIFIED 2026-11-20]**

| Modifier         | Shift             |
|------------------|-------------------|
| D-pad ↑ / ↓      | +1 / −1 semitone  |
| Left stick ↑ / ↓ | +2 / −2 semitones |
| both together    | up to ±3          |

- Hold the modifier **before** pressing the note button (verified; the minimum lead is U5, still open)
- Changing a modifier while a note sounds retunes it: the D-pad steps at once, the left stick glides (by ear). So the player must change modifiers only between notes, unless a bend is wanted
- Songs are recognised **by pitch**, not by button: an alternative fingering of a song note still triggers the song (Spike 1.8)
- Range: B4–F6 sounding (B3–F5 in the community chart), chromatic, 19 pitches; all 19 verified. The count of 45 combinations and the full set of alternative fingerings are **[HYPOTHESIS]**, for Story 1.3
- Bends outside free play: not tested **[HYPOTHESIS]**

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

All 19 fingerings above measured with a tuner (Spike 1.2). Alternatives exist, e.g. C6 = X + stick↓ **[HYPOTHESIS]**, to measure in Story 1.3; two were heard to work inside a song (B5 = A + stick↑, D6 = Y + D↑ + stick↑, Spike 1.8, no tuner reading). The full set goes into the mapping profile, see [Score format and note mapping](score-format-and-mapping.md).

## Unknowns to measure

| ID  | Question                                                               | Impact                     | Answer (2026-11-20, by hand) |
|-----|------------------------------------------------------------------------|----------------------------|------------------------------|
| U1  | Does holding the button sustain the note? Max length?                  | note durations, legato     | Yes, while held; the tone wavers from ~2 s and the note stops by itself at ~6 s even if still held. Longer notes are re-pressed (`max_sustain`, FR-05a) |
| U2  | Same button twice: minimum release gap to retrigger?                   | repeated notes             | open (Story 1.4)             |
| U3  | Pressing a new button while one is held: cut, or ignored?              | legato phrasing            | Cut: the last pressed button sounds. Releasing it while the first is still held makes the first note sound again, so the old button must be released |
| U4  | Modifier changed while a note sounds: bend (glide) or no effect?       | possible portamento effect | D-pad: instant step (A5 → A♯5, 932 Hz). Left stick: gradual glide (by ear). Portamento is possible with the stick |
| U5  | Minimum lead of a modifier before the button                           | fingering cost             | open (Story 1.4)             |
| U6  | Stick threshold: how much deflection counts as ↑/↓? Diagonals?         | stick values in profile    | open (Story 1.3)             |
| U7  | Minimum press length that always sounds                                | fast passages              | open (Story 1.4)             |
| U8  | Free play limits: note count, auto-exit, timeouts                      | long pieces                | None seen: 100 notes in a row and 1 min idle, free play stays open until the player exits. An official song played in free play is recognised (by pitch) |
| U9  | How to enter/leave free play (item button, quick-swap, B to exit)      | navigation macro           | Enter: D-pad ← (fixed binding, not an item slot); note input is accepted only after ~2 s. Exit: B or D-pad ← (D-pad ← toggles) |
| U10 | Frame rate while playing (30/60 fps)                                   | quantisation grid          | open (Story 1.4)             |
| U11 | Does the game ever use a 2nd octave or other buttons (ZL/ZR, R-stick)? | range                      | No other note input or octave. ZL opens the known-songs menu (ZL + A previews a song), ZR does nothing, the right stick moves the camera. AD-9 allow-list (L, R, A, X, Y, hat, left stick) **confirmed** as is |

Tests: [Test plan](test-plan.md).

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
