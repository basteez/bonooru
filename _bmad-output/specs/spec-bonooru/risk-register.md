# Risk register

Assessment of 2026-11-18, design phase (no code yet). P = probability, I = impact, 1 (low) … 3 (high); score = P × I.

## Rule zero: the console comes first

P × I treats recoverable and irreversible losses alike. The Switch 2 costs more than the rest of the project, so console-damage risks (C*) sit above every other risk whatever their score: no residual risk to the console is accepted beyond the precautions below.

### Facts from the RP2040-Zero schematic (Waveshare)

- USB VBUS goes straight to the `5V` header pin (VSYS, pin 23): no diode, no fuse, no ESD/TVS. On the controller board that pin **is** the Switch's VBUS.
- CC1/CC2: 5.1 kΩ pull-downs (R5, R9) → plain 5 V sink, no USB-PD negotiation.
- Header order GP29, 3V3, GND, 5V: the GND wire sits next to 5V; an off-by-one puts it on 5V.

### Wiring rules (README + GUI setup wizard)

1. Connect **only** GP0, GP1 and GND. Never 5V, never 3V3.
2. Rewire only with both USB cables unplugged.
3. Keep the verified assembly as is; experiments go on the spare Zeros (#3, #4).
4. Breadboard: nothing else on the rows of the 5V and 3V3 pins; do not use the power rails. A wire one row off lands on 5V just as on a header.

Our firmware uses the same UART pins as RiiDraw and does not touch the power path, so swapping firmware adds no electrical risk.

**Firmware pins (rule zero, AD-20):** firmware drives **only** GP0 (TX), GP1 (RX) and GP16 (onboard LED), defined once in `ocb_hw/pins.h`; a CI grep forbids other pin numbers; a debug toggle pin exists only in debug builds (AD-20).

## Console risks

| ID | Risk | P | I | Mitigation | Status |
|----|------|---|---|------------|--------|
| C1 | Both 5V pins linked (miswire, other tutorials): PC 5V back-fed into the Switch VBUS / power path | 1 | 3 | RiiDraw wiring, rules 1–3; risk only when rewiring | open |
| C2 | Cable yanked: Switch falls or bottom USB-C port damaged (also the charging port) | 2 | 2 | prefer top USB-C, stand, short cable on Switch side, boards fixed to a base or adapter board, slack on PC side | open |
| C3 | 5V–GND short on the controller board (dropped wire, metal surface) | 1 | 2 | adapter board or insulated base; port has its own current limit (assumed) | open |
| C4 | ESD on the bare board reaches the Switch port | 1 | 1 | handle unplugged; console ports have their own ESD protection | accept |
| C5 | Unpowered board back-powered through UART (few mA) | 1 | 1 | asymmetric (AD-20 as amended): both boards enable an internal pull-down on RX; the controller keeps TX high-Z until RX sees idle-high and releases it on a break or link loss, the bridge leads with short TX probe pulses (AD-20 as amended); measured in M3/M4 (test plan H.1) | open |
| C6 | Timeline inputs reach the game outside free play (items, menus) | 2 | 1 | compile-time allow-list in PLAYING (AD-9): L, R, A, X, Y, hat, left stick; every other button cleared, right stick centred; DIRECT unmasked; adding an input needs a firmware change | open |
| C7 | P2 only: console offers a firmware update to the emulated Pro Controller, or stores bogus pairing data | 1 | 1 | refuse update commands; pairing reset via "Disconnect controllers". Story 2.3 (bench 2026-11-22): the profile refuses SPI write and erase, `22 02` (MCU update mode), output report `0x03` and every feature report, and stores nothing (host tests F.26, F.28). On the Switch 2 no firmware update was offered during the session and no refused request appeared in the logged console sequences. Not observed: the console's "Update Controllers" screen was not opened. The console's Bluetooth pairing request (`01`) is now acknowledged with the real controller's reply and stores nothing on the board (D34); whether the console keeps pairing data for the board's MAC is not observed, and "Disconnect controllers" remains the reset | mitigated (Story 2.3) |
| C8 | Nintendo ToS / EULA (2025: console can be made unusable for unauthorised use); controller emulation ≠ modding | 1 | 3 | offline single player only (NFR-06); no Nintendo marks in the product | accept |

Software cannot brick the console: HID (P1) and wired Pro Controller (P2) involve no persistent writes on the console side; worst case is a USB stack crash → reboot.

## Project risks

| ID  | Risk | P | I | Score | Mitigation / action |
|-----|------|---|---|-------|---------------------|
| R1  | Ocarina mechanics differ from the community chart (no sustain, min note length, cooldown, free play limits) | 2 | 3 | 6 | spike 1 by hand with a real controller first, no hardware needed |
| R2  | A melody accidentally contains one of the 12 official songs → in-game effect, free play ends mid-piece | 2 | 3 | 6 | **warn only** (spike 1.8, 2026-11-20: the game matches songs by pitch, not by button, at any tempo, so an alternative fingering does not avoid the song); the compiler detects official song pitch sequences and warns |
| R3  | NFR-04 (≤ 1 frame) unreachable as written: frame sampling (0–16.7 ms) + USB polling (0–8 ms) not synced to our clock; aubio ±5–10 ms | 3 | 2 | 6 | measure polling interval (GPIO toggle per IN); restate NFR-04 on inter-onset error (p95 ≤ 1 frame, max ≤ 1.5, drift < 1). Measured (Story 2.3 H.15, Story 2.5 H.18, 2026-11-23): the Switch 2 polls every 8000 µs (95th percentile 8000 µs, a few missed polls under 20 ms in a 44 s piece); the firmware applies every event within 14 µs of its time (mean about 5 µs), so the firmware's share of the jitter is negligible and the polling grid (0–8 ms) plus the game's frame (0–16.7 ms) remain; the audio result (H.19) and the explanation of AC3 decide the NFR-04 reading |
| R4  | Scope too large for a fun project: first song only at M6 → motivation loss | 2 | 3 | 6 | walking skeleton after M2 (spike-2 script plays a whole melody through the 2wiCC queue); 3-OS hardware tests as smoke tests on 2 of 3 OS |
| R5  | Unsigned app bundle blocked by Gatekeeper / SmartScreen, antivirus false positives → breaks NFR-10, A.5 | 3 | 2 | 6 | accepted 2026-11-19 (Q3): no signing; README documents the first-run unblock per OS with screenshots ("right-click → Open" no longer works on macOS 15+, so the docs use Open Anyway) |
| R6  | Switch 2 system update breaks the controller profile | 2 | 2 | 4 | profile-agnostic firmware: P1 or P3 behind the same interface (ADR-002) |
| R7  | P2 handshake written clean-room: bugs, more effort than P1 | 2 | 2 | 4 | profile proven on owner's console; compare against 2wiCC USB captures; log subcommands |
| R8  | Bridge not found by USB product string on Windows (`serialport` gets no product string there; Windows usually shows only "USB Serial Device") | 2 | 2 | 4 | generic VID:PID `2E8A:000A` first (Q1, decided 2026-11-19; shared with other Pico boards, so 1200-baud touch only on a confirmed bridge), product string as extra filter where the OS reports it, HELLO probe on VID:PID candidates only; 1200-baud touch only to VID:PID matches (AD-13) |
| R9  | UF2 mass-storage copy flaky (Linux auto-mount, macOS Finder quirks with `RPI-RP2`) | 2 | 2 | 4 | M8 tests; fallback PICOBOOT (ADR-005) |
| R10 | Measurement tools: Switch 2 video capture 30 s max (T.2 needs 5 min); logic analyser for GPIO cross-check | 3 | 1 | 3 | capture card, or record the console speaker with a mic (relative timing is enough) |
| R11 | Playable range is 19 semitones: many songs don't fit, octave folding hurts the result | 3 | 1 | 3 | fit report in the GUI; curated example songs |
| R12 | Licences: MPL-2.0 `serialport` (allowed by NFR-07 as amended, Q2 resolved); egui default fonts under OFL-1.1 / Ubuntu-font-1.0; HORI VID:PID and strings; 2wiCC unlicensed | 2 | 1 | 2 | NFR-07 amended 2026-11-19; CI licence gate `cargo-deny` (AD-21); the app ships the MPL-2.0 notice + source pointer for `serialport` and the OFL/Ubuntu font licence texts (or replaces the fonts); 2wiCC in spikes only |
| R13 | Docking/undocking hangs the controller board | 2 | 1 | 2 | clean re-enumeration; "don't dock mid-song" (ADR-006). Story 2.3 bench (H.16, 2026-11-22): sleep cuts USB power and wake re-enumerates cleanly (ready 1.7 s after the console's first request, new handshake), Wired Communication off/on recovers without a hang or reflash; dock and undock not run yet |
| R14 | Game patch changes the mapping | 2 | 1 | 2 | mapping in the profile file, never in code |

## Inconsistencies found in the notes (RISK, unresolved)

- **Old link-protocol watchdog timeout vs NFR-05 / A.3 (100 ms)** — resolved 2026-11-19 by the owner: two-level watchdog, see `docs/link-protocol.md` (Watchdog) and AD-4/AD-5. Story 2.5 (2026-11-23) measured the core-stall half of NFR-05 on the board: a hung core 0 resets the board 20.0 ms after its last pass (5 runs), a hung core 1 80.0 ms after (detection plus the 60 ms hardware watchdog), nothing left pressed (test plan H.2a, H.2b).
- **NFR-04 measures absolute per-note error** — resolved 2026-11-19 (Q4): NFR-04 restated on inter-onset intervals; T.1 rewritten.

## Next actions (from notes)

1. [x] Spike 1 by hand with a real controller + row 1.8 (R1, R2): done 2026-11-20, results in `docs/test-plan.md`
2. [x] Measure the USB polling interval (R3, R6): 8000 µs median (Story 2.3 H.15), jitter and scheduler lateness measured on the console (Story 2.5 H.18, 2026-11-23)
3. [ ] Walking skeleton after M2 (R4)
4. [ ] Fix the two inconsistencies above in link protocol and requirements
5. [x] Code signing: not signing (Q3, 2026-11-19, R5); NFR-07 amendment (Q2) done 2026-11-19 (R12)
6. [ ] Write the wiring rules into the README and the setup wizard (C1)
7. [x] Record port and cable used with RiiDraw (both console USB-C ports work)
