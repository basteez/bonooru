# Decisions (ADR-001 … ADR-008)

Written 2026-11-18. ADR-001…006 **accepted** 2026-11-18; ADR-007 **accepted** 2026-11-19. ADR-005's language part reopened by ADR-008 (2026-11-19) and settled by it: **ADR-008 accepted 2026-11-20 — the app is Rust + egui/eframe**. Architecture ADs: `ARCHITECTURE-SPINE.md`.

## ADR-001 — Two RP2040-Zero boards (bridge + controller) — accepted

- **Context:** the Zero has one native USB port, which must be a USB device to the Switch 2; the PC needs its own data link. 4 Zeros on hand; RiiDraw/2wiCC prove the two-Zero layout on Switch 2.
- **Decision:** #1 bridge (USB-CDC ↔ UART), #2 controller (UART ↔ USB HID). GP0/GP1 crossed + GND, same as RiiDraw/2wiCC. Wiring method A (3 F-F jumpers) or method B (RiiDraw adapter board), one or the other. Verified by the owner with method A on a breadboard, console USB-C, Apple USB-C↔USB-C data cable 1 m. Firmware keeps the same UART pins and never touches the power path.
- **Consequences:** no parts to buy, no soldering, adapter-board compatible; two firmwares to build and flash; app must tell boards apart; bridge is a frame-aware pipe plus heartbeat (store-and-forward PC→UART per COBS frame, never decodes; best-effort UART→PC; link-local `HEARTBEAT` `0x40`, AD-4); the app protocol lives only in app and controller.
- **Rejected now:** 1 Zero + USB-TTL adapter (extra part, Windows driver quirks; fallback); 1 Zero + Pico-PIO-USB (hand-wired USB, clock constraints, CPU load near timing loop; spike 3); microphone path (out of scope).

## ADR-002 — Controller emulation profile — accepted

- **Decision:** **P2, wired Switch 1 Pro Controller** (057E:2009) — only profile proven on the owner's console, both USB-C ports. Firmware profile-agnostic: link protocol sends a neutral 8-byte state; a profile module turns it into USB reports. P1 (HORI Pokkén HID) off the critical path, possible later simplification; P3 (Switch 2 Pro Controller) only if Nintendo drops P2.
- **Consequences:**
  - Users must enable "Pro Controller Wired Communication" (System Settings → Controllers & Accessories); the GUI setup wizard shows this step with an original illustration plus the exact menu path (System Settings → Controllers & Accessories → Pro Controller Wired Communication → On); no console or game screenshots (copyright) (FR-15, NFR-10).
  - P2 handshake (USB `0x80` commands, subcommands, `0x30` input reports) written clean-room from public reverse-engineering docs (e.g. dekuNukem's Nintendo_Switch_Reverse_Engineering); 2wiCC read for behaviour only, no code copied (NFR-07, ADR-004).
  - Plausible answers to console subcommands: device info, SPI flash reads (stick calibration), player lights, IMU enable (IMU data = zeros).
  - Firmware refuses controller firmware-update requests (risk C7).
  - 2wiCC binaries + RiiDraw remain a debugging reference (same hardware, same console).
- **HYPOTHESIS (unverified):** Switch 2 report rate with P2 — Switch 1 polls ~8 ms; RiiDraw's "Switch 2, faster" option suggests faster. Measure in spike 2 (GPIO toggle per IN transfer); affects NFR-04 / R3.
- **Risks:** system update breaks profile (profile module behind one interface; P1/P3 addable without touching protocol or app); handshake bugs (implement step by step against a USB capture of 2wiCC; log every subcommand via EVENT frames during development).

## ADR-003 — Timing scheduled on the controller board — accepted

- **Context:** onsets within ~1 game frame (16.7 ms at 60 fps), no drift; PC jitter from OS scheduling (Windows ~1–15 ms), USB-CDC buffering, runtime pauses (e.g. garbage collection); 2wiCC's per-frame queue; RiiDraw had desync on large drawings.
- **Decision:** app compiles the whole piece into `(t_us, state)` events with absolute times from START; controller buffers them and applies each from a 1 MHz hardware timer; app streams ahead with credit flow control; real-time `SET_STATE` only for virtual gamepad / live mode.
- **Consequences:** timing independent of host OS; µs events work for 30 or 60 fps (app still snaps to the profile's frame grid); firmware needs queue + scheduler and underrun handling (neutral + event); live MIDI cannot use this path (ADR-007).
- **Rejected:** app sends each state at its time (OS jitter into the music); fixed 60 Hz frame queue (assumes 60 fps); VSYNC input (no video tap).

## ADR-004 — Own firmware on pico-sdk + TinyUSB — accepted

- **Decision:** both firmwares from scratch in C on pico-sdk + TinyUSB. Reuse MIT material (descriptors, handshake notes) with attribution. 2wiCC: ideas + binaries in spikes only; copy no code, keep our protocol different. Bridge starts from TinyUSB's CDC example (or official `debugprobe` UART bridge as reference).
- **Consequences:** clean licensing (NFR-07), code publishable under MIT; more work than flashing 2wiCC, but ADR-003 needs own firmware anyway.
- **Licences surveyed:** 2wiCC none; GP2040-CE MIT; ProBridge-S2 MIT; Pico-PIO-USB MIT; pico-sdk BSD-3; TinyUSB MIT.
- **Rejected:** 2wiCC as-is (no licence, frame queue only); fork GP2040-CE (too large); Arduino-Pico (less USB timing control); MicroPython/CircuitPython (HID descriptor + µs timing harder).

## ADR-005 — Cross-platform app with built-in flasher — accepted; language settled by ADR-008 (Rust)

- **Language (2026-11-20):** the app is **Rust + egui/eframe** (ADR-008). The original Python language, packaging and library choices below are superseded; the flashing flow, the no-toolchain rule and the GUI-only rule stand. Libraries: `serialport` (ports, VID/PID lookup), `midly` (`.mid`), `midir` (live MIDI, M10); UF2s bundled in the binary.
- **Decision (original, superseded language part):** Python 3.12+, packaged per OS with PyInstaller (no Python install for users). GUI only, no CLI for users (toolkit: ADR-008). Libraries: `pyserial` (ports, VID/PID lookup), `mido` + `python-rtmidi`; UF2s bundled in the app.
- **Flashing, RiiDraw style:** (1) user holds BOOT and plugs a Zero into the PC; (2) app finds the `RPI-RP2` drive (all mounted volumes, label + `INFO_UF2.TXT`); (3) user picks role (bridge / controller), app copies the UF2, waits for the drive to vanish; (4) app checks the new firmware (bridge: USB product string; controller: HELLO through the bridge once wired). Bridge update without BOOT: 1200-baud touch → BOOTSEL → same copy. picotool not required.
- **Consequences:** one code base, three OS; mass-storage copy is driverless. Linux: `RPI-RP2` may need auto-mount (udisks) or manual mount → detect and explain; serial access needs `dialout` group → check and explain. Controller board sits on the Switch, so it can only be flashed when moved to the PC.
- **HYPOTHESIS (unverified):** mass-storage UF2 copy is reliable on all three OS — verify in M8; fallback PICOBOOT (USB library or bundled picotool).
- **Rejected:** bundle picotool (native binary + Windows driver); web app/WebSerial (no Firefox/Safari support); Rust/Go (slower iteration; Python fine for a non-real-time app) — reversed 2026-11-20: Rust chosen in ADR-008.

## ADR-006 — Direct USB connection to the Switch 2 — accepted

- **Decision:** controller board on a console USB-C port with a USB-C↔USB-C data cable; top port preferred so the bottom stays free for charging. Dock USB-A as alternative for TV play (to verify).
- **Evidence:** RiiDraw (P2) works on both console ports with the Apple 1 m cable. Dock not tried.
- **Consequences:** works handheld and tabletop, no dock needed; in the dock the bottom port is taken → top port or dock USB-A.
- **HYPOTHESIS (unverified):** console port and dock behave the same — open for dock USB-A (spike 0 rows 0.4–0.5) and for P1.
- **Risk:** docking/undocking while connected hangs the board → firmware re-enumerates cleanly on resume/reset; document "don't dock mid-song".

## ADR-007 — Score input formats — accepted 2026-11-19

- **Candidates:** `.mid` (huge catalogue, exact timing; polyphonic/multi-track); text score (quick, diff-friendly; format to design); live MIDI keyboard (fun; no look-ahead, latency ≈ 3–6 frames, modifier lead adds delay); MusicXML/ABC (common; extra parsers, MIDI export covers them).
- **Decision (accepted as proposed):** (1) `.mid` + text score in v1, same note model; (2) live MIDI keyboard after v1 as extra (M10), real-time path with greedy fingering; (3) MusicXML/ABC not planned, users export to MIDI.
- **Consequences:** pipeline assumes look-ahead (global fingering optimisation); live mode reuses `SET_STATE`, not the queue.
- **HYPOTHESIS (unverified):** live latency acceptable for "fun" use — judged after M6 with a key-to-sound test; live mode stays an M10 extra.

## ADR-008 — App stack (language + GUI toolkit) — accepted 2026-11-20

- **Context:** GUI only, easy for first-time users (FR-15, NFR-10); 3 OS, pre-built (NFR-01). Needs native file dialogs, timeline/piano-roll view, audio preview, step-by-step flashing wizard, on-screen gamepad, background serial I/O without freezing the UI. Screen flow: `architecture-diagrams.md`. Research: `_bmad-output/planning-artifacts/research/app-stack-evaluation.md`. Evidence: `spikes/app-egui/SPIKE-REPORT.md`.
- **Decision (AD-18):** the app is **Rust + egui/eframe**. Crates from the spike: `eframe`/`egui` 0.37 (pin exactly), `serialport`, `midly`, `rodio`; full list and packaging in `app-stack-options.md`.
- **Reasons:** spike passed — no exit criterion failed; owner hand test (macOS arm64) passed discovery of a real Zero, hot-plug, piano roll, audio preview, simulated flash and real `RPI-RP2` detection. Release binary ~11 MB (4.8 MB zipped), cold start ~115 ms, frame time p95 1.5 ms with ~2,000 notes on screen, idle CPU 0.2 %. All dependencies permissive or MPL-2.0; no LGPL/GPL-only.
- **Rejected alternative:** Python 3.12 + PySide6 — bundle ~113 MB, LGPL Qt + FFmpeg obligations, PyInstaller antivirus false positives. Also dropped: CustomTkinter, Dear PyGui, Flet, Toga.
- **Licence:** NFR-07 as amended 2026-11-19 — `serialport` is MPL-2.0 (used unmodified, source pointed to); egui's default fonts are OFL-1.1 / Ubuntu-font-1.0, so their licence texts ship with the app (or the fonts are replaced). See R12.
- **Consequences:**
  - `app/` is a Cargo workspace: core crate (score, pipeline, profile, protocol, sans-IO link, device, flasher, preview) has no egui/eframe dependency, no threads, no I/O, and is unit-testable; one serial worker thread owns the port, the UI exchanges only messages with it (AD-16); AD-15–AD-17 apply.
  - macOS lists each serial device twice (`/dev/cu.*`, `/dev/tty.*`): the app shows and uses only `cu.*`.
  - Audio output open can fail transiently (e.g. Bluetooth profile switch): show a clear error with Retry and re-open the output when the default device changes; no silent fallback.
  - Cost: egui/rodio/sysinfo APIs churn between 0.x versions (pin exact versions); ~2 GB `target/`; Linux build needs `libudev` and ALSA dev packages.
  - Not yet verified: GUI on Windows/Linux, 3-OS CI run (M5).
