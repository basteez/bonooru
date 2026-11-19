# Requirements

Verbatim from the requirements note. Priority: M = must, S = should, C = could (MoSCoW). "Host" in the notes = the desktop **app** (`app/`).

## Functional

| ID    | Requirement                                                                                              | Prio | CAP    |
|-------|----------------------------------------------------------------------------------------------------------|------|--------|
| FR-01 | Import a `.mid` file; choose track/channel                                                               | M    | CAP-1  |
| FR-02 | Import a hand-written text score (custom format)                                                         | M    | CAP-1  |
| FR-03 | Reduce polyphony to one melodic line (chords → one note)                                                 | M    | CAP-2  |
| FR-04 | Fit the melody into the playable range (auto transpose + manual override); report what doesn't fit       | M    | CAP-2  |
| FR-05 | Pick an input combination (button + modifiers) for every pitch                                           | M    | CAP-3  |
| FR-06 | Mapping and timing parameters in an editable profile, not hard-coded                                     | M    | CAP-3  |
| FR-07 | Playback: start (with countdown), stop, tempo scale; stop = release every input                          | M    | CAP-4  |
| FR-08 | Detect the bridge board automatically (no manual serial-port choice)                                     | M    | CAP-6  |
| FR-09 | Flash firmware on both RP2040-Zero boards from the host app (bridge and controller role)                 | M    | CAP-7  |
| FR-10 | Check firmware/protocol version and offer an update when they don't match                                | S    | CAP-6  |
| FR-11 | Virtual gamepad: manual input to reach ocarina free play in the game                                     | S    | CAP-8  |
| FR-12 | Dry run: show the input timeline and play a local audio preview on the PC                                | S    | CAP-9  |
| FR-13 | Live play from a MIDI keyboard (to analyse, see ADR-007)                                                 | C    | CAP-11 |
| FR-14 | Calibration: guided tests that measure the timing parameters on the user's console                       | C    | CAP-12 |
| FR-15 | Graphical UI for every user function (setup/flash, song import, preview, play, gamepad); no command line | M    | CAP-10 |

## Non-functional

| ID     | Requirement                                                                                                                   | Prio |
|--------|-------------------------------------------------------------------------------------------------------------------------------|------|
| NFR-01 | Host runs on Windows, Linux and macOS; users install no toolchain (pre-built binaries)                                        | M    |
| NFR-02 | Hardware: 2× RP2040-Zero + 3 jumper wires, nothing else                                                                       | M    |
| NFR-03 | Stock Switch 2: no modding, no homebrew                                                                                       | M    |
| NFR-04 | Inter-onset interval error vs the score: p95 ≤ 1 game frame, max ≤ 1.5 frames; drift < 1 frame over a 5-minute piece; validated in a dedicated timing test phase (M7). Restated 2026-11-19 | M    |
| NFR-05 | Fail safe: if the link drops, the controller goes neutral within 100 ms                                                       | M    |
| NFR-06 | Offline only, single player only                                                                                              | M    |
| NFR-07 | Own code (MIT); dependencies under permissive licences (MIT/BSD/Apache/ISC/Zlib), or weak copyleft used unmodified (MPL-2.0 file-level; LGPL dynamically linked). Amended 2026-11-19 | M    |
| NFR-08 | Wrong-note rate 0 on the reference test piece once calibrated                                                                 | S    |
| NFR-09 | Firmware upgradable by users with BOOT button + host app only                                                                 | S    |
| NFR-10 | Ease of use: a first-time user, following the repo README for the one-time OS unblock of the unsigned app, then flashes both boards and plays a song with the GUI only, following on-screen steps. Restated 2026-11-19 | M    |

## Open points on requirements

- **Host UI** — decided: GUI only, like RiiDraw (FR-15, NFR-10). App stack conditional: ADR-008 (decided by the `spikes/app-egui` verdict before M5, Q7).
- **NFR-04 depends on the game's frame rate and input sampling** — HYPOTHESIS (unverified). The 1-frame threshold is a target. Spike 2 measures the limits; M7 (tests T.1–T.5) checks the full chain and may revise the threshold. See risk R3 and the inconsistency list in `risk-register.md`.
- **NFR-05 vs watchdog** — resolved 2026-11-19: two-level watchdog (bridge `HEARTBEAT` type `0x40` every 20 ms, link timeout ≤ 100 ms, default 80, AD-4/AD-5; app-level `app_timeout_ms`, default 1000 ms until W.1, DIRECT mode only). See `docs/link-protocol.md` (Watchdog) and tests W.1, W.2, A.3 in `docs/test-plan.md`.
- **NFR-07 vs ADR-008** — resolved 2026-11-19 (Q2): NFR-07 amended to allow weak copyleft used unmodified; both stack options now fit.
- **NFR-10 vs R5** — resolved 2026-11-19 (Q3): no code signing; this is a personal project, not distributed as a product. The README documents the first-run unblock (macOS: System Settings → Privacy & Security → Open Anyway; Windows SmartScreen: More info → Run anyway) with screenshots.

## Out of scope (verbatim)

- Playing or recognising the 12 official songs, any game-progress automation
- Online play, multiplayer, anything outside offline single player
- Console or game modding, save editing
- Bluetooth controller emulation
- Polyphony (the ocarina is monophonic)
- Microphone input path (humming into the Switch 2 mic): noted as an alternative in the architecture only
