---
id: SPEC-ocarina-bot
companions:
  - requirements.md
  - roadmap.md
  - decisions.md
  - architecture.md
  - architecture-diagrams.md
  - risk-register.md
  - ../../planning-artifacts/architecture/architecture-ocarina-bot-2026-11-19/ARCHITECTURE-SPINE.md
  - ../../planning-artifacts/architecture/architecture-ocarina-bot-2026-11-19/app-stack-options.md
  - ../../../docs/link-protocol.md
  - ../../../docs/usb-controller-emulation.md
  - ../../../docs/ocarina-input-mechanics.md
  - ../../../docs/score-format-and-mapping.md
  - ../../../docs/test-plan.md
sources:
  - ~/Documents/org/org-roam/20261118095901-ocarina_bot.org
  - ~/Documents/org/org-roam/20261118095902-ocarina_bot_requirements.org
  - ~/Documents/org/org-roam/20261118095903-ocarina_bot_architecture.org
  - ~/Documents/org/org-roam/20261118095904-ocarina_bot_link_protocol.org
  - ~/Documents/org/org-roam/20261118095905-ocarina_bot_usb_controller.org
  - ~/Documents/org/org-roam/20261118095906-ocarina_of_time_switch_2_ocarina_input.org
  - ~/Documents/org/org-roam/20261118095907-ocarina_bot_score_format_and_mapping.org
  - ~/Documents/org/org-roam/20261118095908-ocarina_bot_test_plan.org
  - ~/Documents/org/org-roam/20261118095909-ocarina_bot_roadmap.org
  - ~/Documents/org/org-roam/20261118095910-ocarina_bot_adr_001_two_board_topology.org
  - ~/Documents/org/org-roam/20261118095911-ocarina_bot_adr_002_controller_emulation_profile.org
  - ~/Documents/org/org-roam/20261118095912-ocarina_bot_adr_003_device_side_scheduling.org
  - ~/Documents/org/org-roam/20261118095913-ocarina_bot_adr_004_own_firmware_on_pico_sdk.org
  - ~/Documents/org/org-roam/20261118095914-ocarina_bot_adr_005_cross_platform_python_host.org
  - ~/Documents/org/org-roam/20261118095915-ocarina_bot_adr_006_direct_usb_connection.org
  - ~/Documents/org/org-roam/20261118095916-ocarina_bot_adr_007_score_input_formats.org
  - ~/Documents/org/org-roam/20261118095917-ocarina_bot_adr_008_gui_toolkit.org
  - ~/Documents/org/org-roam/20261118112325-ocarina_bot_risk_register.org
---

> **Canonical contract.** This SPEC and the files in `companions:` are the complete, preservation-validated contract for what to build, test, and validate. Source documents listed in frontmatter are for traceability — consult them only if you need narrative rationale or prose color this contract intentionally omits.

# Ocarina Bot

## Why

A vision to realize, for fun: play arbitrary melodies (MIDI or hand-written scores) on Link's ocarina in *The Legend of Zelda: Ocarina of Time* (Switch 2 remake, 2026) by emulating a wired controller — not for game progress. Inspired by RiiDraw (auto-drawing in Tomodachi Life), whose two-RP2040-Zero setup already works on the owner's console. Audience: the owner and first-time users with two Zeros, three jumper wires and a stock Switch 2. The notes call the desktop application "host"; this spec calls it the **app** (`app/`).

## Capabilities

IDs in brackets map to `requirements.md` (verbatim FR/NFR, MoSCoW). Reference docs (protocol, USB emulation, ocarina mechanics, score format, test plan) live in `docs/` and are adopted companions.

- **CAP-1** [FR-01 M, FR-02 M]
  - **intent:** User can import a `.mid` file (choosing track/channel) or a hand-written text score into one note model.
  - **success:** Both formats open from the GUI and yield the same `(pitch, start, duration)` list for the same melody; track/channel is selectable (M6).
- **CAP-2** [FR-03 M, FR-04 M]
  - **intent:** The app reduces the score to one melodic line and fits it into the playable range, with auto transpose and manual override.
  - **success:** Chords become one note; the chosen transposition minimises out-of-range notes; every note that still doesn't fit is reported to the user.
- **CAP-3** [FR-05 M, FR-06 M]
  - **intent:** The app picks an input combination (button + modifiers) for every pitch and applies timing rules, all driven by an editable mapping profile.
  - **success:** Changing a mapping or timing value in the profile file changes the compiled timeline with no code change; the reference piece (all 19 pitches) compiles with a fingering for every note.
- **CAP-4** [FR-07 M, NFR-04 M, NFR-08 S]
  - **intent:** User can play a compiled piece on the console with timing kept by the controller board: start with countdown, stop, tempo scale; stop releases every input.
  - **success:** Tests T.1–T.5 pass on all 3 OS (or NFR-04 is revised with measured numbers, M7); A.1 plays with 0 wrong notes.
- **CAP-5** [NFR-05 M]
  - **intent:** If the link drops, the controller board returns to neutral on its own.
  - **success:** A.3 — unplugging the bridge mid-song leaves the controller neutral within 100 ms.
- **CAP-6** [FR-08 M, FR-10 S]
  - **intent:** The app finds the bridge without a manual serial-port choice and checks firmware/protocol version, offering an update on mismatch.
  - **success:** On Windows, Linux and macOS the app shows connection and version status with no port selection (M5); a mismatched firmware triggers an update offer.
- **CAP-7** [FR-09 M, NFR-09 S]
  - **intent:** User can flash and later upgrade both boards (bridge and controller roles) from the app with only the BOOT button.
  - **success:** A.4 — a fresh Zero is flashed in both roles from a guided GUI wizard on each OS, no other tool installed (M8).
- **CAP-8** [FR-11 S]
  - **intent:** User can drive the console manually from an on-screen gamepad to reach ocarina free play.
  - **success:** Mouse + keyboard input on the on-screen gamepad moves through the game into free play (M5).
- **CAP-9** [FR-12 S]
  - **intent:** User can dry-run a piece: see the input timeline and hear a local audio preview on the PC.
  - **success:** The timeline with fingerings and an audio preview are shown before playing (M6).
- **CAP-10** [FR-15 M, NFR-10 M]
  - **intent:** Every user function (setup/flash, song import, preview, play, gamepad) is reachable from a graphical UI with on-screen steps; no command line.
  - **success:** A.5 — a first-time user (not the developer) unblocks the unsigned app using the README, then flashes both boards and plays a song with the GUI only, no other help.
- **CAP-11** [FR-13 C] — after v1, M10 extra (ADR-007 accepted); latency judged after M6
  - **intent:** User can play live from a MIDI keyboard.
  - **success:** Playable latency judged OK by 3 people (M10).
- **CAP-12** [FR-14 C]
  - **intent:** User can run guided tests that measure the timing parameters on their own console.
  - **success:** The calibration wizard writes measured timing values into the user's profile (M10).

## Constraints

- **Console first (rule zero):** console-damage risks outrank all others; no residual console risk accepted beyond the precautions in `risk-register.md`. Wiring is only GP0, GP1, GND — never 5V or 3V3; firmware never touches the power path.
- Stock Switch 2: no modding, no homebrew (NFR-03); offline single player only (NFR-06).
- Hardware: 2× RP2040-Zero + 3 jumper wires, nothing else (NFR-02); topology bridge + controller (ADR-001); controller on a console USB-C port, top preferred (ADR-006).
- Controller profile P2 (wired Switch 1 Pro Controller); firmware profile-agnostic behind one interface (ADR-002). Users must enable "Pro Controller Wired Communication".
- Timing is scheduled on the controller board from absolute-time events; the app never times notes itself (ADR-003).
- Own firmware in C on pico-sdk + TinyUSB; P2 handshake clean-room; 2wiCC code never copied (ADR-004). Own code; dependencies permissive, or weak copyleft used unmodified (NFR-07, amended 2026-11-19).
- App: language conditional per ADR-008 — Rust + egui/eframe if the `spikes/app-egui` spike passes, otherwise Python 3.12 + PySide6 (AD-18); pre-built for Windows/Linux/macOS, no toolchain for users (NFR-01, ADR-005); GUI only, no user CLI (FR-15).
- Architecture ADs (`ARCHITECTURE-SPINE.md`, AD-1…AD-21) bind all code; they and `firmware/common/proto/vectors/` take precedence over `docs/link-protocol.md` (AD-14).
- Mapping and timing values live in a profile file per game version, never in code (FR-06, R14). Ocarina mechanics are unverified until M2 (`docs/ocarina-input-mechanics.md`).
- Queue mode only emits ocarina inputs, never Home / Capture / + / − (C6); firmware refuses console firmware-update requests (C7).
- One repo version `vX.Y.Z`; firmware and app from the same tag agree on the protocol version. Repo layout fixed: `firmware/` (CMake, targets `bridge`, `controller`, shared `common/`), `app/`, `spikes/`, `docs/`.

## Non-goals

- Playing or recognising the 12 official songs; any game-progress automation.
- Online play, multiplayer, anything outside offline single player.
- Console or game modding, save editing.
- Bluetooth controller emulation.
- Polyphony (the ocarina is monophonic).
- Microphone input path (humming into the Switch 2 mic).
- MusicXML / ABC import; users export to MIDI (ADR-007).

## Success signal

Release 1.0 (M9): pre-built app for Windows, Linux and macOS plus both UF2s; a first-time user wires two Zeros, flashes both from the GUI, and plays the 1-minute reference piece (all 19 pitches) with 0 wrong notes — acceptance tests A.1–A.5 pass.

## Assumptions

- "App" (`app/`) = "host" in the notes.
- NFR-09 and NFR-10 are in the requirements note and kept verbatim, although the caller listed NFR-01…08.
- Spec status at derive time: M0 and M1 DONE; M2–M10 TODO; no code yet.

## Open Questions

- **Q7 / ADR-008 (conditional):** Rust + egui/eframe if `spikes/app-egui` meets exit criteria (a)–(e) of the research report, else Python 3.12 + PySide6; spike verdict due before M5 starts.
- Ocarina mechanics (unverified): button→pitch pairing, modifiers, fingering chart, unknowns U1–U11 — settled by spikes 1–2.
- Does the game match official songs by buttons or by pitches (spike 1.8, R2)?
- Unverified: Switch 2 polling rate with P2; game frame rate (30/60 fps); 460800 baud sufficiency; dock USB-A behaviour; UF2 mass-storage reliability on 3 OS; app library choices.
