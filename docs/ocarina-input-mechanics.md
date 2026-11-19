# Ocarina of Time (Switch 2): ocarina input mechanics

*Converted from the author's org-roam notes on 2026-11-19. This file is now the reference in the repo.*

Topic: Ocarina Bot (author's notes)

*The Legend of Zelda: Ocarina of Time* remake, Switch 2 exclusive, released 2026-11-05. Everything below comes from community sources, **not** verified in game yet.

## Note buttons **[HYPOTHESIS — unverified]**

| Button | Pitch (written) | N64 equivalent |
|--------|-----------------|----------------|
| L      | D4              | A              |
| R      | F4              | C-down         |
| A      | A4              | C-right        |
| Y      | B4              | C-left         |
| X      | D5              | C-up           |

Most guides agree on the set L/R/A/Y/X; the button→pitch pairing comes from one source.

## Pitch modifiers (free play only) **[HYPOTHESIS — unverified]**

| Modifier         | Shift             |
|------------------|-------------------|
| D-pad ↑ / ↓      | +1 / −1 semitone  |
| Left stick ↑ / ↓ | +2 / −2 semitones |
| both together    | up to ±3          |

- Hold the modifier **before** pressing the note button
- Songs are recognised from the 5 natural notes only; bends work in free play only
- Range: B3–F5, chromatic, 19 pitches (45 combinations → many pitches have several fingerings)

## Fingering chart (one option per pitch) **[HYPOTHESIS — unverified]**

| Pitch | Input           | Pitch | Input           |
|-------|-----------------|-------|-----------------|
| B3    | L + D↓ + stick↓ | G♯4   | A + D↓          |
| C4    | L + stick↓      | A4    | A               |
| C♯4   | L + D↓          | A♯4   | A + D↑          |
| D4    | L               | B4    | Y               |
| D♯4   | L + D↑          | C5    | Y + D↑          |
| E4    | R + D↓          | C♯5   | X + D↓          |
| F4    | R               | D5    | X               |
| F♯4   | R + D↑          | D♯5   | X + D↑          |
| G4    | R + stick↑      | E5    | X + stick↑      |
|       |                 | F5    | X + D↑ + stick↑ |

Alternatives exist, e.g. C5 = X + stick↓. The full set goes into the mapping profile, see [Score format and note mapping](score-format-and-mapping.md).

## Unknowns to measure

| ID  | Question                                                               | Impact                     |
|-----|------------------------------------------------------------------------|----------------------------|
| U1  | Does holding the button sustain the note? Max length?                  | note durations, legato     |
| U2  | Same button twice: minimum release gap to retrigger?                   | repeated notes             |
| U3  | Pressing a new button while one is held: cut, or ignored?              | legato phrasing            |
| U4  | Modifier changed while a note sounds: bend (glide) or no effect?       | possible portamento effect |
| U5  | Minimum lead of a modifier before the button                           | fingering cost             |
| U6  | Stick threshold: how much deflection counts as ↑/↓? Diagonals?         | stick values in profile    |
| U7  | Minimum press length that always sounds                                | fast passages              |
| U8  | Free play limits: note count, auto-exit, timeouts                      | long pieces                |
| U9  | How to enter/leave free play (item button, quick-swap, B to exit)      | navigation macro           |
| U10 | Frame rate while playing (30/60 fps)                                   | quantisation grid          |
| U11 | Does the game ever use a 2nd octave or other buttons (ZL/ZR, R-stick)? | range                      |

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

## Related

- [Ocarina Bot: USB controller emulation](usb-controller-emulation.md)
