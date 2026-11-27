---
status: final
stepsCompleted: [1, 2, 3, 4]
inputDocuments:
  - _bmad-output/specs/spec-bonooru/SPEC.md
  - _bmad-output/specs/spec-bonooru/requirements.md
  - _bmad-output/specs/spec-bonooru/decisions.md
  - _bmad-output/specs/spec-bonooru/roadmap.md
  - _bmad-output/specs/spec-bonooru/risk-register.md
  - _bmad-output/specs/spec-bonooru/architecture.md
  - _bmad-output/specs/spec-bonooru/architecture-diagrams.md
  - _bmad-output/planning-artifacts/architecture/architecture-bonooru-2026-11-19/ARCHITECTURE-SPINE.md
  - _bmad-output/planning-artifacts/architecture/architecture-bonooru-2026-11-19/app-stack-options.md
  - _bmad-output/planning-artifacts/ux-designs/ux-bonooru-2026-11-20/DESIGN.md
  - _bmad-output/planning-artifacts/ux-designs/ux-bonooru-2026-11-20/EXPERIENCE.md
  - _bmad-output/planning-artifacts/ux-designs/ux-bonooru-2026-11-20/review-egui-feasibility.md
  - docs/link-protocol.md
  - docs/usb-controller-emulation.md
  - docs/score-format-and-mapping.md
  - docs/ocarina-input-mechanics.md
  - docs/test-plan.md
  - spikes/app-egui/SPIKE-REPORT.md
---

# bonooru - Epic Breakdown

## Overview

This document provides the complete epic and story breakdown for bonooru, decomposing the requirements from the spec (`SPEC.md` + `requirements.md`, used as the PRD), the UX design contract (`DESIGN.md` + `EXPERIENCE.md`) and the architecture spine (`ARCHITECTURE-SPINE.md`, AD-1…AD-21) into implementable stories.

Precedence: architecture ADs and `firmware/common/proto/vectors/` win over `docs/link-protocol.md` (AD-14); the UX spines win over the mockups. Game reference screenshots in `ux-designs/*/imports/` are git-ignored and are never committed, embedded or referenced by any story.

## Requirements Inventory

### Functional Requirements

Priority in brackets (MoSCoW). FR-16…FR-25 come from the UX session; owner confirmed FR-23/FR-24 as Must and the rest as Should.

- FR-01 [M]: Import a `.mid` file and choose its track/channel.
- FR-02 [M]: Import a hand-written text score (custom format, `docs/score-format-and-mapping.md`); both formats produce the same `(pitch, start, duration)` note model.
- FR-03 [M]: Reduce polyphony to one melodic line (chords → highest note "skyline"; overlaps cut the earlier note).
- FR-04 [M]: Fit the melody into the playable range B4–F6, sounding pitch (Story 1.1) (auto transpose over −24…+24 with fewest out-of-range notes, tie-break smallest shift then most natural notes; manual override); octave-fold or drop left-over notes and always report what doesn't fit.
- FR-05 [M]: Pick an input combination (button + modifiers) for every pitch with a min-cost fingering path (Viterbi/DP, profile weights).
- FR-06 [M]: Mapping and timing parameters (pitches, modifiers, timing rules, range, cost weights) live in an editable TOML mapping profile, never in code.
- FR-07 [M]: Playback: start (with countdown), stop, tempo scale; stop releases every input.
- FR-08 [M]: Detect the bridge board automatically, no manual serial-port choice.
- FR-09 [M]: Flash firmware on both RP2040-Zero boards (bridge and controller roles) from the app.
- FR-10 [S]: Check firmware/protocol version and offer an update when they don't match.
- FR-11 [S]: Virtual gamepad for manual input to reach ocarina free play.
- FR-12 [S]: Dry run: show the input timeline and play a local audio preview on the PC.
- FR-13 [C]: Live play from a MIDI keyboard (M10 extra, ADR-007).
- FR-14 [C]: Calibration: guided tests that measure timing parameters on the user's console (M10 extra).
- FR-15 [M]: Graphical UI for every user function (setup/flash, song import, preview, play, gamepad); no command line.
- FR-16 [S]: In-app text-score editor with live note view and inline errors.
- FR-17 [S]: Draw notes in the note view (add/move/resize/delete); the text score is the source of truth, kept in two-way sync.
- FR-18 [S]: The first edit of a `.mid` converts it to a text score; the original file is never modified.
- FR-19 [S]: Per-note actions on out-of-range notes of the fit proposal: move by octave, delete, keep.
- FR-20 [S]: One app-level undo/redo for text, note, out-of-range, transpose and tempo edits.
- FR-21 [S]: Roll and Staff views of the song; Listen = PC audio preview from the cursor, nothing sent to the Switch.
- FR-22 [S]: Virtual gamepad has the full Pro Controller set; keyboard mapping only while it has focus.
- FR-23 [M]: Three live status LEDs: PC↔bridge, bridge↔controller, controller↔Switch (STATUS `console_ready`).
- FR-24 [M]: Play only when all three LEDs are ok, after a readiness confirm ("Don't ask again", resettable).
- FR-25 [S]: Persistence: last song + per-song track/transpose/tempo/cursor, Recent (last 5), draft autosave/restore.

Derived pipeline requirements (from CAP-3 / AD-15 / R2, stated in the spec but not numbered as FRs):

- FR-05a: Timing rules: snap to the profile frame grid; enforce min press, min gap on same button, modifier lead; steal time from the previous note, never shift onsets; reject min press/gap below `2 × max(report_period_us, frame time)`; at most one note button pressed in any state (U3: releasing a later button brings back a still-held one); a note longer than the profile `max_sustain_ms` is re-pressed (release for the min gap, press again) before the game cuts it (U1, ~6 s), and the count is reported.
- FR-05b: Official-song detection (R2): detect pitch sequences of the 12 official songs and warn (the game matches by pitch, so no fingering avoids it; Spike 1.8).
- FR-05c: Timeline export to CSV (`t_us`, pitch, fingering) as the M7 ground truth (not user-facing in v1).

### NonFunctional Requirements

- NFR-01 [M]: App runs on Windows, Linux and macOS; users install no toolchain (pre-built binaries).
- NFR-02 [M]: Hardware is 2× RP2040-Zero + 3 jumper wires, nothing else.
- NFR-03 [M]: Stock Switch 2: no modding, no homebrew.
- NFR-04 [M]: Inter-onset interval error vs the score: p95 ≤ 1 game frame, max ≤ 1.5 frames; drift < 1 frame over a 5-minute piece; validated in M7 (T.1–T.5), may be revised with measured numbers.
- NFR-05 [M]: Fail safe: if the link drops, the controller goes neutral within 100 ms (A.3).
- NFR-06 [M]: Offline only, single player only; the app opens no network connections (AD-21).
- NFR-07 [M]: Own code MIT; dependencies permissive (MIT/BSD/Apache/ISC/Zlib) or weak copyleft used unmodified (MPL-2.0 file-level; LGPL dynamically linked); enforced by `cargo-deny`.
- NFR-08 [S]: Wrong-note rate 0 on the reference test piece once calibrated (A.1).
- NFR-09 [S]: Firmware upgradable by users with BOOT button + app only.
- NFR-10 [M]: A first-time user, following the README for the one-time OS unblock of the unsigned app, flashes both boards and plays a song with the GUI only (A.5).
- NFR-11 (constraint, rule zero): console safety outranks every other concern — wiring GP0/GP1/GND only, firmware drives only GP0, GP1, GP16; queue mode emits only ocarina inputs (C6); firmware refuses controller firmware-update requests (C7).
- NFR-12 (constraint): Evoke, don't copy — nothing from Nintendo ships in the app, mocks, README or repo (logos, art, UI textures, fonts, sounds, screenshots).

### Additional Requirements

From `ARCHITECTURE-SPINE.md`, `app-stack-options.md`, `architecture.md`, `docs/*` and the spike report.

**Starter / scaffolding (affects Epic 1 Story 1):** no third-party starter template. The repo skeleton exists (`firmware/{bridge,controller,common}/README.md`, `app/README.md`, `spikes/app-egui`). Firmware is a new CMake project on pico-sdk; the app is a new Cargo workspace seeded from `spikes/app-egui` (proven crate versions and patterns, not its code structure). CI arrives with the first story that produces code; `release.yml` lands with M8–M9.

Firmware build and structure
- AD-1: pico-sdk git submodule at `firmware/external/pico-sdk` pinned to `2.3.1` (only `lib/tinyusb` = TinyUSB 0.18.0 initialised; `PICO_TINYUSB_PATH` never overridden); picotool 2.3.1 and Arm GNU Toolchain 15.2.Rel1 pinned in CI; `PICO_BOARD=waveshare_rp2040_zero`; each SDK upgrade is its own PR rebuilding both UF2s.
- AD-2: one CMake project (≥ 3.25) with `ocb_proto` (pure C99: COBS, CRC-16/CCITT-FALSE, frame types, message structs, canonical state, `NEUTRAL`, `PROTO_VERSION`, `OCB_MAX_ENCODED_FRAME`, link timing constants, baud 460800), `ocb_hw` (UART helpers, `pins.h`), executables `bridge` and `controller`; presets `fw-release`, `fw-debug`, `host-tests` (native compiler + ctest, no pico-sdk).
- Firmware core (queue, scheduler, modes, watchdog logic) is plain C with no pico-sdk includes and runs on a PC; platform code is thin.
- Naming: C prefix `ocb_`, `snake_case.c`, constants `OCB_MSG_*`, `OCB_EVT_*`, `OCB_NAK_*`.

Link protocol
- Framing: COBS, `0x00` delimiter; payload `type:u8 | seq:u8 | len:u8 | body | crc16:u16le`; little-endian; times µs u32; versions `[major, minor, patch]`.
- AD-3: type ranges `0x01–0x3F` app→controller, `0x40–0x7F` bridge→controller (link-local, never forwarded/ACKed, outside app seq), `0x80–0xFF` controller→app; receive order COBS → len check → CRC → type range → seq → semantics; empty/out-of-range frames counted and dropped.
- Messages: HELLO 0x01, SET_STATE 0x02, QUEUE_EVENT 0x03, START 0x04, STOP 0x05, PING 0x06, HEARTBEAT 0x40, INFO 0x81, STATUS 0x83, ACK 0x84, NAK 0x85, EVENT 0x86. Numeric NAK reasons / EVENT codes / STATUS field values are set by the first firmware story in `ocb_proto` and the vectors.
- AD-6: go-back-N with u8 wrapping seq, stale-frame window 64, cached response for duplicate `expected−1`, transport NAKs (CRC/SEQ/FULL) never advance `expected` (max one per `expected`), semantic NAKs advance it; HELLO/STOP sync points; cumulative ACK, resend after 50 ms; credits `STATUS.free − QUEUE_EVENTs after last_seq`, send ≤ `min(credit, 64 − unacked)`.
- AD-7: `PROTO_VERSION` u8 exact match; frozen forever: frame layout, COBS/CRC rules, codes 0x01/0x81, HELLO body, INFO byte 0, HEARTBEAT body prefix; session open = assert DTR, write `0x00`, HELLO every 100 ms until INFO; INFO v1 body; app learns runtime values (queue_cap, app_timeout_ms, report_period_us) from INFO, never hard-codes them; PING in DIRECT when idle `app_timeout_ms/4`; mid-session `NAK VERSION` → re-HELLO.
- AD-10: canonical 8-byte state in P1 bit order (Y, B, A, X, L, R, ZL, ZR, −, +, L3, R3, Home, Capture), hat 0–7 / 8 neutral, sticks centred 0x80 (`ly 0x00` = up), flags bit0 END; `NEUTRAL` defined once per codebase and checked by vectors.
- AD-14: repo version from `git describe` (fallback `0.0.0-dev`) read by firmware CMake and app build only; language-neutral vectors in `firmware/common/proto/vectors/` (COBS, CRC, every message, max-length/empty/back-to-back/leading delimiter/bad CRC/overlong run, `NEUTRAL`) run by both firmware host tests and app core tests; CI checks both `PROTO_VERSION`s equal the vector header; wire changes edit vectors first; app flashes only UF2s bundled at its own version.

Bridge firmware
- AD-4: `HEARTBEAT` 0x40 COBS+CRC frame, own rolling seq, body `bridge_fw_ver[3], flags` (bit0 DTR), hardware alarm every 20 ms regardless of USB state, wire spacing ≤ 27 ms; PC→UART store-and-forward per frame (never decodes; one app frame in flight + one queued; due heartbeat at next boundary ahead of queued frames; buffering stays in CDC); overlong run dropped and counted; partial frame discarded on DTR fall / CDC unmount; UART→PC best effort, drop and count on full CDC FIFO; no blocking code path.
- AD-13: VID:PID `2E8A:000A`, product string `Bonooru Bridge`, serial = flash unique ID; 1200-baud touch via pico-sdk `pico_usb_reset` with `PICO_ENABLE_USB_RESET_VIA_BAUD_RATE=1` (no own `tud_cdc_line_coding_cb`); bridge version reaches the app only via `INFO.bridge_ver`.
- M4 exit: 460800 baud with no byte loss over 10 min (baud is a build constant; 921600 only if loss measured).

Controller firmware
- AD-5: link watchdog — no valid HEARTBEAT within `OCB_LINK_TIMEOUT_MS` (≤ 100, default 80) or UART break → abort to IDLE + `EVENT LINK_LOST`; UART RX IRQ-driven reading error bits (no 8-bit DMA); `app_timeout_ms` (default 1000 until W.1) and DTR falling edge only in DIRECT; no app timeout in PLAYING; `NAK NO_LINK` on START/SET_STATE while link down.
- AD-8: modes IDLE/DIRECT/PLAYING per the mode table; entering IDLE publishes NEUTRAL, clears queue, resets ordering; abort latch set by every unrequested IDLE transition (END, underrun, LATE, link loss, app timeout, USB suspend/unmount/bus reset, core stall), cleared only by STOP/HELLO, `NAK ABORTED` while set; `t_us` strictly increasing and < 2^31 (`NAK ORDER`); last event `NEUTRAL|END` → `EVENT PLAYBACK_DONE`, END on non-neutral → `NAK BAD_STATE`; underrun judged at apply → NEUTRAL same pass + `EVENT UNDERRUN`; head event overdue > 1 report period → `EVENT LATE`, never fire a backlog; `NAK NOT_READY` while profile `ready()` is false.
- AD-9: in PLAYING mask every applied state to L, R, A, X, Y, hat, left stick; clear other buttons, centre right stick; DIRECT unmasked; adding an input needs a firmware change.
- AD-11: core 0 = TinyUSB task + profile + report sending (only TinyUSB toucher); core 1 = non-blocking RAM super-loop (`__not_in_flash_func`, < 50 µs) for UART, decode, modes, queue, 1 MHz scheduler, watchdogs; seqlock double buffer, reader retries ≤ 3 then NEUTRAL; core 1 sole UART TX writer; core0→core1 via latched atomic flags and SPSC log ring that drops; hardware watchdog ≤ 100 ms fed only while core 1 liveness < 20 ms old; core 1 forces reboot if core 0 report counter > 20 ms old; report sent on every host poll.
- AD-12: exactly one profile per build (`OCB_PROFILE`, default P2); interface = descriptors, control/OUT handler (handshake), `build_report(const ocb_state*)`, `tick()`, `ready()`; profile reads only the published state; P2 (057E:2009) clean-room handshake (USB `0x80` commands, subcommands: device info, SPI flash reads, player lights, IMU enable with zero data, `0x30` reports), refuses firmware-update subcommands (C7), restarts handshake after bus reset, logs subcommands in debug builds only; `INFO.hid_profile` = profile id; `INFO.report_period_us` = measured median IN interval.
- AD-19: STATUS body `free u16, now_us u32, last_seq, mode, abort_count, last_event, flags u8` (+ debug counters: max app-frame gap, watchdog trips, dropped bytes); flags bit0 `link_ok`, bit1 `console_ready`, bits 2–7 reserved 0; `console_ready` = profile `ready()` (1 after handshake; 0 on suspend/unmount/bus reset and > 100 ms without IN report — flag only, not abort); STATUS every 50 ms plus after ACKs, coalesced ≤ 1/10 ms; TX priority EVENT > NAK > ACK > STATUS, drop lowest on overflow, never block; unknown codes logged, never fatal.
- AD-20: firmware configures only GP0 (TX), GP1 (RX), GP16 (LED) from `ocb_hw/pins.h`; CI grep forbids other pin numbers; debug toggle pin only in debug builds; both boards pull down RX; the controller keeps TX high-Z until RX idle-high and the bridge leads with short TX probe pulses; both release TX on break or link loss (H.1, A.3).
- ADR-006 / R13: clean re-enumeration on dock/undock or resume; README says "don't dock mid-song".

App architecture
- AD-18 / app-stack-options: Rust 1.100.0 pinned in `app/rust-toolchain.toml`; Cargo workspace `ocb-core` (lib, no egui/eframe dependency, no threads, no I/O) + `ocb-app` (bin) (+ optional `ocb-adapters`); pinned crates: eframe/egui 0.37.0, serialport 4.10.1, midly 0.5.3, rodio 0.23.0, sysinfo 0.40.4, crossbeam-channel 0.5.17, toml 1.2.1, serde 1.0.229, rfd 0.18.0, directories 6.0.0, proptest 1.12.0 (tests), midir 0.11.1 (M10), egui_extras image loaders (wiring photo).
- Hexagonal: `ui` → `services` → `core` (score, pipeline, profile, protocol, link (sans-IO), device, flasher, preview); adapters (serial, drives+fs, audio, midi, config dir) implement core ports. Nothing depends on `ui`.
- AD-15: only the core pipeline produces a `Timeline` (events with `t_us` u32 strictly increasing < 2^31, full canonical state snapped to frame grid, last `NEUTRAL|END`; annotations never sent); pipeline also returns range-fit report and official-song detection (warning only: songs match by pitch); tempo scale = recompile; countdown = UI + `START lead_us`; streamer prefills, STARTs, sends greedily on credit, never edits/drops/reorders; live MIDI uses SET_STATE (M10).
- AD-16: one serial worker thread owns the port and link session; UI ↔ worker only via messages (commands in, events + immutable snapshots out); connect/disconnect/1200-baud touch/post-flash verification are worker commands; flasher worker never opens a serial port, snapshots volumes before touch/BOOT prompt, accepts exactly one new volume with `INFO_UF2.TXT`, checks RP2040 family, copies with a plain read/write loop (not `fs::copy`), waits for the volume to vanish; audio preview rendered in core, played by the adapter; UI thread never blocks on I/O.
- AD-17: profile = TOML with integer `schema`; only core `profile` loads/validates/migrates; bundled read-only defaults per game version in `app/profiles/`; user copies (incl. calibration output) in the OS config dir override the same-name default; unmigratable user copy refused with a message; app settings in a separate file.
- Time units: integer µs everywhere downstream of score import; float seconds only inside import. Core returns typed results; services turn them into user-facing events.
- Logging: rotating log in the OS log dir, every EVENT/NAK logged; hidden advanced panel shows live frames.
- Discovery (AD-13): match VID:PID first, product string as extra filter where the OS reports it (not Windows), HELLO probe only to VID:PID ports; 1200-baud touch only to a confirmed bridge (product string or explicit user pick); macOS shows/opens only `/dev/cu.*`; port rescan ~1 s.
- Update offer (AD-13): with no controller wired, the wizard always offers to reflash the bridge; proto mismatch → update required; `fw_ver`/`bridge_ver` ≠ app version → update recommended.
- Audio adapter: open failures show an error with Retry; re-open on default-device change; never silently fall back to no sound.
- Linux: eframe built without the `wayland` feature (X11/XWayland) so file drops work; File › Open… always available; runtime deps `libudev.so.1`, `libasound.so.2`; `dialout` and udisks problems detected and explained with exact commands, never fixed by privilege escalation.
- AD-21: no network connections; `cargo-deny` licence gate in CI; releases ship a third-party notices file (MPL-2.0 `serialport` notice + source pointer, OFL/Ubuntu/Hack font licence texts) and the MIT licence.

CI, release and test infrastructure
- GitHub Actions: firmware job (ubuntu: both UF2s, ctest host-tests, pin grep) → app jobs (win/linux/macos: core tests, vectors, licence gate, clippy, headless GUI smoke test, bundle app + both UF2s) → release on tag `vX.Y.Z`.
- Release assets `bonooru-<ver>-<os>-<arch>.<ext>`, `bridge.uf2`, `controller.uf2`; app bundles embed UF2s built in the same run; macOS universal `.app` (unsigned), Windows `.exe`, Linux AppImage (+ tarball if cheap); `cargo-packager` 0.11.8 or a bundle script.
- No code signing (Q3); README documents first-run unblock per OS (macOS Open Anyway; Windows SmartScreen More info › Run anyway).
- App test layers: score parsers, pipeline property tests (sorted, onsets never shifted, min press/gap respected, END last), range-fit, fingering DP, profile validation/migration; golden tests (reference piece → timeline snapshot; text and `.mid` of the same melody → identical note list); sans-IO link tests against an in-test fake controller (HELLO/INFO, version mismatch, go-back-N under injected loss/corruption, credits, PING cadence).
- Firmware host tests F.1–F.10 (go-back-N, NAK matrix, mode table + abort latch, END/underrun/LATE, HELLO/STOP in PLAYING, allow-list, heartbeat gap ≤ 27 ms under 512-frame burst, bridge framer, shared vectors, `console_ready` with a fake profile).
- Hardware-in-the-loop: H.1 (no back-powering), H.2 (core stall → watchdog reboot, nothing pressed), H.3 (`console_ready` on Switch 2 with sleep/wake and Wired Communication off/on, back to 0 within 150 ms on OFF; sleep cuts USB power, so the controller reboots), W.1 (30-min DIRECT soak sets `app_timeout_ms`), W.2 (30-min playback under load, 0 false trips), T.1–T.5 (timing phase), A.1–A.5 (acceptance).
- Every HIL run records the test environment (Switch 2 system version, game version, port, cable, firmware version, host OS).

Hardware-risk spikes and roadmap gates (from roadmap / test plan / risk register)
- M2 Spike 1 (ocarina mapping, rows 1.1–1.8; 1.1–1.5, 1.7, 1.8 can run by hand with a real controller first) and Spike 2 (timing limits 2.1–2.5) must remove all HYPOTHESIS markers on pitches/modifiers and fill `[timing]` with measured values + margin. Settles U1–U11, R1, R2, R3.
- R4: walking skeleton after M2 — a spike script plays a whole melody through an existing queue to keep motivation.
- Measure P2 polling interval on Switch 2 (ADR-002 hypothesis; `INFO.report_period_us` answers it later).
- Dock USB-A behaviour (spike 0 rows 0.4–0.5) unverified.

UX open questions to carry as spikes or explicit AC TODOs (not guesses)
- OQ-1 Ocarina auto-open (D-pad left before Play) vs leaving it to the user — ties to U9; resolve after Spike 1.
- OQ-2 Fingering chart unverified — all lane-header fingerings, staff glyphs, visualizer captions come from the community hypothesis; must be verified in game (Spike 1) before release copy/screenshots.
- OQ-3 STATUS staleness timeout (candidate 3 × 50 ms = 150 ms → LEDs `unknown`) — decide with H.3 measurements. RESOLVED 2026-11-26 (Story 3.5): 150 ms.
- OQ-4 `console_ready` 100 ms polling-stall rule unverified on hardware (H.3); Play gating to be revisited after hands-on testing. RESOLVED 2026-11-26 (Story 3.5): verified and kept at 100 ms.
- OQ-5 Windows, bridge alone, main screen: how to show an unconfirmed bridge (no product string, no INFO).
- OQ-6 Bridge LED label without INFO ("Bridge – found" assumed).
- OQ-7 Status warning/error/unknown hexes, `amber-ink`, `scrim`, editor font (proposed). RESOLVED 2026-11-27 (Story 4.1, owner gallery review): all kept as proposed (`#f08a4b`, `#ef6a55`, `#9a8a72`, `#8a5a12`, `#0b0806b3`, Hack 13 px). Same question as `EXPERIENCE.md` Open Questions item 6.
- OQ-8 Update flow key flow (required/recommended → wizard → reflash → Play re-enabled) — states-only coverage today.
- OQ-9 Screen reader / AccessKit support level — must be decided before the shared widget helpers are built. RESOLVED 2026-11-27 (Story 4.2, owner): basic — every shared helper emits `widget_info` with the closest `egui::Role` and the visible label, the Roll and the Staff are one summary node each, eframe's `accesskit` feature stays on. Same question as `EXPERIENCE.md` Open Questions item 8.
- OQ-10 Text score file extension and save-as defaults; grammar details owned by `score`.
- OQ-11 Final list of 3–5 bundled public-domain examples (Ode to Joy, Greensleeves, Frère Jacques, Twinkle Twinkle, B4–F6 scale).
- OQ-12 Wiring photo of the owner's assembly (not yet taken).
- OQ-13 Draft microcopy approval pass over every "draft" string.
- OQ-14 Fit proposal: explicit "Accept all" / "Reset to proposal" buttons, or Play-accepts only.

### UX Design Requirements

Source: `DESIGN.md` (tokens, visuals) and `EXPERIENCE.md` (behaviour). Items tagged [ASSUMPTION] in the spines stay assumptions and are flagged in acceptance criteria.

Foundations
- UX-DR1: Theme module with every `DESIGN.md` token (colors incl. alpha overlays via `Color32::from_rgba_unmultiplied`, spacing scale and named sizes, radii, typography scale) as the single source; dark only, OS light/dark ignored.
- UX-DR2: Bundle Cormorant Garamond (display: brand, panel tabs, dialog/card titles, countdown) and Nunito Sans (everything else) as TTF; weights via the variable `wght` axis (never `.strong()` for bold); uppercase labels with 0.06 em tracking; Hack monospace (egui `default_fonts`) for editor and log; all font licence texts in About.
- UX-DR3: Shared painting helpers: `paint_focus_ring(resp, radius, on_parchment)` used by every focusable widget (amber on dark, amber-ink on parchment, 2 px, 2 px offset); glow via `epaint::Shadow` on rounded rects/circles only; text halo as a blurred rounded rect; dashed rounded-rect helper (`dashed_line_many`); convex-only fills; strokes floored at 1 physical px; fixed-width digit cells for all times (no `tnum`); painted symbols ▶ ■ ⏮ ▾ ▲ ⌫ ↵ ↑ ♯ ♪ ↶ ↷ unless TTF coverage is verified.
- UX-DR4: Corner-bracket ornament (2 px L path with rounded elbow, 1.2 px inner quadratic curve, 1.6 px elbow dot, mirrored per corner, `bracket` colour; `rim` on dark overlays) on the note-view card, end card and illustration sheets only.
- UX-DR5: Main screen layout Variant A built from computed rects in reading order with `scope_builder` (not egui side panels) so Tab order is status → song → note view → gamepad → bottom bar; status strip 44 px, song panel 260 px, gamepad 300 px, bottom bar 88 px, all extra width to the note view; min window 1024 × 680 logical points; HiDPI via `pixels_per_point`.
- UX-DR6: Responsive rules: ≥ 1280 px full layout; 1024–1279 px compact gamepad (200 px, buttons only, key hints in tooltips) and user-collapsible song rail (48 px; never automatic; state persisted [ASSUMPTION]); safety note wraps.
- UX-DR7: Modal dialog component (440–460 px, centred, title over an original flourish rule, checklist rows with amber diamond bullets, actions right-aligned primary amber + ghost secondary, scrim backdrop); modal stack depth one; Esc closes the topmost dialog.

Status and connection
- UX-DR8: Status strip: brand (Cormorant, parchment), three Status LEDs with 20 px gaps, "Set up / Reflash…" link, gear (disabled during Play).
- UX-DR9: Status LED component: four states, each colour + symbol + label (+ detail): ok filled dot with glow (`status-ok`), warning filled triangle (`status-warning`), error cross (`status-error`), unknown hollow ring (`status-unknown`, label only); tooltip = detail + fix on hover and on keyboard focus; never says "linked to game" / "in free play"; click opens the wizard at the relevant step when not ok [ASSUMPTION]; copy per Voice table ("Bridge v0.3", "Switch – not ready", "Controller – not answering", "Bridge – not found", "Bridge – found").
- UX-DR10: LED state machine driven by the Hardware & Protocol Coupling table: USB enumeration (none → error, foreign product string → warning, ours → ok without version), INFO (Bridge↔Controller ok, versions in labels), no INFO/STATUS (Bridge↔Controller error, Switch unknown), `link_ok`, `console_ready` (incl. "Undock the Switch" tooltip on suspend), NAK NOT_READY / NO_LINK, mid-session NAK VERSION (briefly unknown, re-HELLO), port disappears; LEDs start `unknown` and resolve within ~1 s; STATUS staleness timeout = OQ-3 TODO.
- UX-DR11: Foreign Pico / RiiDraw board shown as PC↔Bridge warning naming the board when the OS gives a product string, with "Set up…"; never touched over serial.
- UX-DR12: Update banner (slim, 38 px, under the status strip) for a recommended update: "Firmware update available" [Update] [Later]; Update opens the wizard at the right flash step; Later hides it until next launch.
- UX-DR13: "Firmware update needed" blocking modal on `INFO.proto_ver` ≠ app: Update now (initial focus, Enter) → wizard at the right flash step; Not now / Esc → Play buttons and gamepad disabled (gamepad inline hint "The gamepad is off until both boards are updated.", Play tooltip "Play needs a firmware update."), Listen/preview/editing/loading/saving keep working; reshown on next launch or reconnect while the mismatch persists [ASSUMPTION]; LEDs keep showing current board versions.

Song panel
- UX-DR14: Song panel: parchment "Song" panel tab, filename + "Open…" pill, "No song loaded" state, mapping-profile footer hint "Mapping profile: Default · Switch 2" at 75 %.
- UX-DR15: Track picker (`.mid` only, hidden for text scores): auto-selects the likely melody track (most notes in range, least polyphony); rows "name · ch N · N notes · N % in range"; amber painted caret; change recompiles; 0 % in range track still loads with all ghost notes [ASSUMPTION].
- UX-DR16: Transpose stepper: large value ("−5 st"), −12/−1/+1/+12/Auto (Auto = best fit −24..+24), `auto` chip while value equals the automatic result, active button amber with glow; live fit summary "212 in range · 2 folded · 0 dropped", plus "N long notes re-pressed" when FR-05a re-presses any; each change one undo step.
- UX-DR17: Tempo control: −5/+5 % stepper, Reset, range 50–150 %, linked BPM field (scales the whole tempo map); every change recompiles and is one undo step; "N notes too short" state turns the value `status-error` and disables Play.
- UX-DR18: Official-song warning row in the song panel (ember wash, 3 px ember left border, ember-text): "1 official-song sequence: could end free play"; Play stays enabled.
- UX-DR19: Song rail (collapsed song panel): icons Open, Track, Transpose, Tempo with current-value tooltips and focus ring; clicking expands the panel with that control focused.

Note view
- UX-DR20: Note-view card: parchment, `rounded.xl`, `card-inset` frame, corner brackets; header with Roll/Staff segmented toggle (only place it appears), preview transport ▶ ■, time "0:12 / 0:31" in digit cells, "Preview · PC only" at 75 %, Undo ↶ / Redo ↷ history buttons (26 px ink circles, disabled at 45 % when empty, tooltips with shortcut).
- UX-DR21: Roll view: 19 lanes B4–F6, sounding pitch (21 px, min 18 px then vertical scroll with pinned ruler), accidental lanes in `-alt` fills, lane headers (96 px) with note name + fingering (once per lane), ruler corner label "note · input", ruler ticks every beat, labels every 2 s, ruler click sets the shared cursor; note bars with no text; selected = amber stroke + glow; resize affordance on last 6 px; 2 px ember playhead with convex pentagon handle; pan with wheel/trackpad (Shift = horizontal), middle-drag; Cmd/Ctrl+wheel zoom around pointer; follows playhead during preview and Play.
- UX-DR22: Staff view: same lanes as staff rules (natural 1.2 px ink, accidental 0.5 px ink 45 %), lane headers kept, notes as 5 px bars starting with a 17 px staff glyph (button letter) followed by modifier tags (e.g. "D↑", "S↑"); same selection, editing, playhead; switching views keeps scroll, zoom, cursor, selection; view choice persists [ASSUMPTION].
- UX-DR23: Out-of-range zones above F6 / below B4 (30 px, collapse to 6 px strip when empty), labelled in `out-of-range-ink`, dashed divider; ghost notes (dashed outline) with fold target outline in the target lane and dashed leader line; inline label "G6 · out of range — proposed: fold to G5"; ghost notes selectable.
- UX-DR24: Official-song group: rounded ember-wash rectangle with 1.6 px dashed ember outline around the notes, notes filled ember, label tab "Triggers <Song>"; shown for every detected sequence (no fingering avoids it: songs match by pitch).
- UX-DR25: Note selection model: click selects (replace), Shift+click toggles, drag on empty = marquee (amber-wash, dashed amber-ink) selecting touched notes, Shift+marquee adds, click empty clears; Esc never clears selection.
- UX-DR26: Note editing: double-click empty = add note at pointer lane/time (frame-grid snap) with last-used duration (one beat initially [ASSUMPTION]); drag selection = move (lane + frame-grid snap); drag right edge = resize; Delete/Backspace deletes only while the note view has focus; each gesture one undo step; edits rewrite only the affected tokens in the text score and keep the editor caret.
- UX-DR27: Note context menu (right-click, selects if needed; menu key / Shift+F10 [ASSUMPTION]): Move up an octave · Move down an octave · separator · Delete; out-of-range notes add "Reset to proposal" [ASSUMPTION]; items that would leave B4–F6 + zones disabled; styled per `context-menu`.
- UX-DR28: Text editor split (text scores only): editor on top (Well fill, Hack 13 px), draggable split divider (6 px, grip amber on hover/drag, initial 40/60 [ASSUMPTION], persisted, editor ≥ 4 lines, double-click reset [ASSUMPTION]); live parse updates the note view; inline straight `status-error` underline with hover message and fix line (e.g. "Unknown note length “q”." / "Use 1, 2, 4, 8 or 16, e.g. D5/4."); last valid parse stays drawn while invalid [ASSUMPTION]; egui's built-in TextEdit undoer disabled.
- UX-DR29: App-level undo/redo history (Cmd/Ctrl+Z, Cmd/Ctrl+Shift+Z everywhere incl. editor, plus header buttons): covers text, note-view, out-of-range, transpose, tempo edits; excludes track choice, view toggle, cursor, Settings; per song, cleared on loading another song [ASSUMPTION]; disabled during Play.
- UX-DR30: `.mid` conversion: first note-view edit converts the `.mid` (selected track, transpose applied) to a text score, opens the split editor, shows "Converted to text score – save to keep changes"; original never modified; undo of that edit keeps the converted score [ASSUMPTION].
- UX-DR31: Fit proposal semantics: pipeline fold/drop is a proposed default; Play accepts the current proposal [ASSUMPTION]; "Accept all" / "Reset to proposal" buttons = OQ-14 TODO.

Preview and Listen
- UX-DR32: Preview transport: PC audio only with an original synthesised ocarina-like timbre; ▶/■ with moving playhead; starts from the cursor; three triggers (header ▶, Listen, Space); volume in Settings; disabled during countdown/Play.
- UX-DR33: Listen button (bottom bar, secondary style, painted fairy-orb icon with one wing pair, label "Listen", sub-line "on this PC", tooltip "Preview the song on this PC (Space). Nothing is sent to the Switch."); toggles stop while playing [ASSUMPTION]; not gated by LEDs or firmware update; disabled with no song and during countdown/Play; with an invalid text score previews the last valid parse [ASSUMPTION]; no copy refers to the orb.
- UX-DR34: Audio failure: inline "Can't open the audio device." + Retry in the note-view header; re-open on default-device change; no silent fallback.

Virtual gamepad
- UX-DR35: Gamepad panel full layout: original Pro-Controller-like silhouette (union of convex fills + one stroked outline), ZL/ZR, L/R, two sticks, D-pad, ABXY, −, +, Capture, Home; key chips beside mapped controls (Z, Q, E, C, H, J, K, L, ⌫, ↵, "WASD · arrows", "mouse" by the sticks) with leader lines where needed [ASSUMPTION]; legend list; caption "Use it to reach ocarina free play."
- UX-DR36: Gamepad input: click = tap, press-and-hold = hold, sticks drag and spring back; controls use `Sense::CLICK | DRAG` without FOCUSABLE; key and mouse tracked separately; release everything on focus loss and window blur; key auto-repeat ignored.
- UX-DR37: Gamepad focus: one panel-level focus id; gained by clicking anywhere in the panel or tabbing in [ASSUMPTION]; focus-lock filter so arrows/Tab don't move egui focus; click elsewhere to leave [ASSUMPTION]; focused look = inset 2 px amber ring + stacked inset amber wash + "Keys active" chip; keys matched by `physical_key`; mapping Arrows/WASD = D-pad, H J K L = Y B X A, Q E = L R, Z C = ZL ZR, Enter = +, Backspace = −; Home, Capture, sticks mouse only; Cmd/Ctrl combos keep global meaning.
- UX-DR38: Gamepad states: during countdown/Play locked ("Locked · live view" chip, no input) and lit-key visualizer derived from the compiled timeline at the interpolated playback time, with caption "Now: Y + D↑ (C5)"; when the Switch is not ready, the link is down or an update is required, clicks are ignored with an inline hint [ASSUMPTION]; in DIRECT mode PING when idle; compact variant < 1280 px.

Bottom bar and play sequence
- UX-DR39: Bottom bar: Play from cursor (primary amber, painted ▶, sub-line cursor time), Play from start (secondary, ⏮, "0:00"), Listen, two-line safety note "Don't dock the Switch mid-song." / "Esc stops at any time.", STOP with sub-line "Esc" (idle dim 75 %, live ember with glow during countdown/Play); big buttons 60 px high, min 190 px wide, 14 px apart.
- UX-DR40: Play gating: both Play actions (and Resume) disabled unless all three LEDs are ok, and also while a required update is pending, no song is loaded, notes are too short, the text score is invalid [ASSUMPTION], or during Play; disabled tooltip names only the first blocking reason + fix in order Bridge › Controller › Switch › firmware update › no song › too-short › invalid score, with the Voice-table strings; preview always available; revisit after H.3 (OQ-4).
- UX-DR41: STOP: always enabled, Esc anywhere (read at frame start, not consumed, before any Modal consumes it) sends STOP and releases every input.
- UX-DR42: Readiness confirm modal: title "Ready to play?", checklist "The ocarina is out in free play." · "Don't touch the controller until the song ends." · "Don't dock the Switch.", [Cancel] [Play] (Play focused; Enter = Play; Esc = Cancel + harmless STOP), "Don't ask again" checkbox; shown on every Play/Resume unless suppressed; reset from Settings.
- UX-DR43: Countdown: 3·2·1 default (Settings 0–5 s), 80 % veil over the note view only, 128 px cream numerals with 2 px frame outline and amber halo, sub-line "Starting from 0:12 · hands off the controller", original wood-block tick per number; STOP live, Esc cancels; link/console drop or port loss cancels it with no START sent and shows the interruption banner; separate from the technical `start_lead_ms` (500 ms).
- UX-DR44: Playing state: playhead and time interpolate locally from the last STATUS `now_us` and resync each STATUS (0 during lead); song panel locked, note view read-only (scroll allowed) [ASSUMPTION]; drop and Open… ignored, Cmd/Ctrl+O disabled [ASSUMPTION]; gear disabled.
- UX-DR45: Interruption banner (dialogue style: translucent fill, cream-40 % border, name tab, centred actions, ▾ chevron, Details link): causes UNDERRUN, LATE, LINK_LOST, USB_SUSPENDED, bridge unplugged, semantic NAK ("Playback couldn't start" [ASSUMPTION]) with the Voice/State strings; actions Resume from <time> (= Play from cursor through confirm + countdown, gated), Dismiss, Details (log view at the entry); cursor stays at the stop point; persists until acted on; app sends STOP to clear the latch.
- UX-DR46: End-of-song card on PLAYBACK_DONE: parchment card ~440 px with corner brackets and frame ring, "You played <Song>!" (song name amber-ink), detail "1:48 · 62 notes · on time" (actual vs expected), Play again (ink fill) / Close (ink outline); auto-dismiss after 6 s, timer paused on hover; silent.

Empty state, files and persistence
- UX-DR47: Empty state: drop zone (~470 px, dashed parchment-grid, original drop icon, "Drop a .mid or score here", Open… + New song), "Or try an example" chips (3–5 bundled public-domain songs already in range, OQ-11), "Recent" chips (last 5); header transport and Undo/Redo dimmed, time "0:00 / 0:00"; Play/Listen disabled ("Load a song to enable Play"), gamepad usable.
- UX-DR48: File hovering: drop zone border turns amber-ink; outside the empty state the whole note-view card shows a drop outline [ASSUMPTION].
- UX-DR49: Import failure banner (interruption-banner variant without Resume): "Couldn't open <file>: <reason>." with reasons "not a MIDI file or score", "the file is damaged", "the MIDI file has no notes", "the file was moved or deleted"; unsupported formats (MusicXML/ABC) point to MIDI export; current song kept (or empty state).
- UX-DR50: Persistence: reopen last song with track, transpose, tempo, cursor; per-song memory on reopen; Recent last 5; missing last song / Recent entry → empty state + banner, entry removed.
- UX-DR51: Drafts and saving: continuous draft autosave to an internal app folder; title-bar dot while unsaved; Cmd/Ctrl+S saves (save-as for new/converted scores; extension = OQ-10); Unsaved changes dialog (Save / Discard / Cancel) on close or opening another song; "Restore unsaved draft?" (Restore / Discard) on launch after a crash when the draft is newer than the file; New song opens the split view with an empty score.

Setup wizard
- UX-DR52: Setup wizard frame: status strip with LEDs stays visible; 280 px step list of 6 clickable rows (number circle, title, check mark; current step amber ring) — "Flash the bridge", "Flash the controller", "Wire the two boards", "Plug both boards in", "Turn on Wired Communication", "Test" — plus an unnumbered Welcome ("A melody waits to be played."); step body with card-title, one instruction line, illustration on a parchment sheet with corner brackets, status line, Back/Next footer.
- UX-DR53: Wizard navigation: opens automatically only on first run (until a valid bridge has ever been seen), walked in order with Next enabled only when the step is done (step 3: Next is the confirmation); on re-entry any step clickable; steps auto-complete from detection; closing on first run returns to the main screen in "bridge not found" state [ASSUMPTION]; reachable from "Set up / Reflash…", Update now / Update, and "Set up…" on a foreign-board message.
- UX-DR54: Wizard step completion rules: step 1 done when the RPI-RP2 drive vanishes and a new `2E8A:000A` port appears (product string checked where supplied, not on Windows); step 2 done on drive vanish; step 3 on Next; step 4 when PC↔Bridge ok and INFO received, else "No answer from the controller. Check the wires: GP0 goes to GP1, GP1 to GP0."; step 5 when `console_ready` = 1; step 6 when the user confirms A reached the game.
- UX-DR55: Wizard flasher UI: "Waiting for the RPI-RP2 drive…" with painted spinner; progress bar "Copying… 64 %"; specific failure messages + Retry (no drive, two drives, wrong board/family, board not back after flashing); after 2 failures a "Flash manually" row (opens the folder with the `.uf2` and explains drag-and-drop); never 1200-baud-touches an unconfirmed port.
- UX-DR56: Wizard illustrations, all original Painter drawings on parchment: flash illustration (board top view, BOOT ringed amber-ink, cable to a generic laptop, callouts "1 · hold BOOT", "2 · plug into your PC"); wiring diagram (two board outlines, three ink wires "GP0 → GP1", dashed "GP1 ← GP0", "GND – GND", 5V and 3V3 circled in ember with "Circled pins: 5V and 3V3. Never connect them.") beside the owner's wiring photo (OQ-12) and wiring rules 1–4; Wired Communication schematic (generic rows, target row "Pro Controller Wired Communication" toggled On with amber-ink ring) with the exact menu path as text and the sub-line "Switch LED: “Switch – not ready” until the setting is On."; no console screenshots.
- UX-DR57: Port pick list (Windows only, reflash needing a 1200-baud touch without a product string): one row per `2E8A:000A` port (port name, VID:PID muted, selected amber border), "Use this port"; never shown on the main screen.

Settings, log, diagnostics
- UX-DR58: Settings modal (gear, disabled during Play): countdown length (0–5 s), preview volume, reset "Don't ask again", "Set up / Reflash…", About (versions + third-party licences: fonts OFL, Hack, serialport MPL-2.0, …), collapsed Advanced section (mapping-profile timing fields — min press, min gap same button, modifier lead, start lead, … — each with a one-line explanation and unit; Reset to defaults; Open profile file; Show log; Copy diagnostics); unmigratable user profile → inline "This mapping profile can't be read by this version. Defaults are in use." [Open profile file].
- UX-DR59: Log view: Well fill, monospace rows "12:04:31.220  EVENT  UNDERRUN  t=1:12", muted timestamps, error rows in `status-error`, newest at the bottom with auto-follow unless scrolled up; opened from Settings › Advanced › Show log or a banner's Details (scrolled to the entry, highlighted amber-wash); shows recent EVENT/NAK, versions, ports.
- UX-DR60: Copy diagnostics: clipboard summary with app + firmware versions, OS, recent events.

Interaction, accessibility, voice, platform
- UX-DR61: Global shortcuts consumed/read at frame start: Esc (STOP, also closes topmost dialog), Cmd/Ctrl+Enter (Play from cursor), Cmd/Ctrl+Shift+Enter (Play from start), Cmd/Ctrl+O, Cmd/Ctrl+S, Cmd/Ctrl+Z / Shift+Z; Space = preview play/stop everywhere except the text editor (consumed at frame start, never activates a button); buttons activate with Enter only; Tab/Shift+Tab move between regions in documented order outside editor and gamepad; Cmd on macOS, Ctrl elsewhere.
- UX-DR62: Accessibility floor: LEDs never colour alone; focus always visible on every focusable control; every function keyboard-reachable except sticks, Home, Capture and note drawing; no information conveyed only by sound; banned: stacked modals, hover-only actions, automatic reconnect that silently restarts a song, gamepad input during Play, Space activating buttons; AccessKit level = OQ-9 decision before widget helpers (basic, Story 4.2).
- UX-DR63: Voice and copy: operational copy plain, second person, action first, no exclamation marks, every error says what happened then what to do; light game flavour only in ambient moments; no franchise names except detected official song titles; boards named "bridge" / "controller", console "the Switch"; all strings from the EXPERIENCE Voice table; "draft" strings pending approval (OQ-13).
- UX-DR64: Brand constraint (evoke, don't copy): no Nintendo logos/marks, character/item art, ripped UI, game fonts, sounds or screenshots anywhere in app, mocks, README or repo; ornaments, silhouette, glyphs, illustrations, preview timbre and countdown tick are original; the fairy-orb icon never reproduces the game's companion and no catchphrase appears.
- UX-DR65: Platform rules: macOS shows/opens only `/dev/cu.*`, RPI-RP2 copy via read/write loop, universal unsigned `.app`; Windows: VID:PID + HELLO probe, step 1 on drive-vanish + new port, port pick before any 1200-baud touch; Linux: X11/XWayland build, `dialout` message with exact command (`sudo usermod -aG dialout $USER`, then log out and in), udisks message ("Open the RPI-RP2 drive in your file manager to mount it, then Retry." + `udisksctl mount` command), never escalate privileges; all: offline only.

### FR Coverage Map

| Requirement | Epic | Note |
|---|---|---|
| FR-01 | Epic 5 | `.mid` import + track/channel picker |
| FR-02 | Epic 5 | Text score parser, same note model |
| FR-03 | Epic 5 | Skyline mono reduction |
| FR-04 | Epic 5 | Range fit, auto/manual transpose, fit report |
| FR-05 | Epic 5 | Fingering DP (chart data verified in Epic 1) |
| FR-05a | Epic 5 | Timing rules; values measured in Epic 1, margins tuned in Epic 7 |
| FR-05b | Epic 5 | Official-song detection (Spike 1.8: songs match by pitch → warning only) |
| FR-05c | Epic 7 | Timeline CSV export (M7 ground truth) |
| FR-06 | Epic 1 (data), Epic 5 (profile module) | Measured TOML profile; `profile` load/validate/migrate (AD-17) |
| FR-07 | Epic 6 | Start with countdown, STOP, tempo scale via recompile |
| FR-08 | Epic 4 | Bridge discovery (VID:PID, product string, HELLO probe) |
| FR-09 | Epic 8 | Flash both roles from the wizard |
| FR-10 | Epic 4 (detect + gate), Epic 8 (update flow) | Version check in INFO; required/recommended update → reflash |
| FR-11 | Epic 4 | Virtual gamepad over SET_STATE (DIRECT) |
| FR-12 | Epic 5 | Timeline view with fingerings + PC audio preview |
| FR-13 | Epic 12 | Live MIDI keyboard (Could, M10) |
| FR-14 | Epic 12 | Calibration wizard (Could, M10) |
| FR-15 | Epics 4, 5, 6, 8 | GUI for every function; closed by A.5 in Epic 11 |
| FR-16 | Epic 9 | Text-score editor with live note view, inline errors |
| FR-17 | Epic 9 | Note drawing in Roll/Staff, two-way sync |
| FR-18 | Epic 9 | `.mid` → text score on first edit |
| FR-19 | Epic 5 (proposal shown), Epic 9 (per-note actions) | Ghost notes + fold targets; octave move/delete/keep |
| FR-20 | Epic 9 | App-level undo/redo |
| FR-21 | Epic 5 (Roll + Listen), Epic 9 (Staff) | Roll and Staff views; Listen from cursor |
| FR-22 | Epic 4 | Full Pro Controller set, keyboard only while focused |
| FR-23 | Epic 2 (`console_ready` in firmware), Epic 4 (LEDs in app) | Three live status LEDs |
| FR-24 | Epic 6 | Play gating + readiness confirm |
| FR-25 | Epic 10 | Last song, per-song settings, Recent, drafts |
| NFR-01 | Epics 4, 11 | 3-OS CI from Epic 4; packaged binaries in Epic 11 |
| NFR-02 | Epics 2, 3 | Two Zeros + 3 wires only |
| NFR-03 | All | Stock console, no modding |
| NFR-04 | Epic 7 | T.1–T.5 (firmware jitter checked in Epic 2) |
| NFR-05 | Epics 2, 3 | Controller watchdog (Epic 2), heartbeat + A.3 (Epic 3) |
| NFR-06 | Epic 4 | No network (AD-21), carried by all app epics |
| NFR-07 | Epics 4, 11 | `cargo-deny` gate from Epic 4; notices file in Epic 11 |
| NFR-08 | Epics 6, 11 | A.1 with 0 wrong notes |
| NFR-09 | Epic 8 | BOOT + app only, 1200-baud bridge update |
| NFR-10 | Epics 8, 11 | Wizard (Epic 8); README unblock + A.5 (Epic 11) |
| NFR-11 | Epics 2, 3, 8 | Pins, allow-list, C7, wiring rules in wizard |
| NFR-12 | Epics 4–11 | Original art only; no Nintendo assets |

## Epic List

Ordered along the roadmap, hardware risk first. Each epic builds only on earlier ones. Epics 9 and 10 depend only on Epic 5 and can move later if needed.

### Epic 1: Ocarina mechanics verified (M2)
The owner knows, from measurements on their own Switch 2, which buttons and modifiers play which pitches and with what timing limits, and a spike script already plays a whole melody through the existing 2wiCC queue (walking skeleton, R4).
**Covers:** FR-06 (measured profile data); Spike 1 (1.1–1.8), Spike 2 (2.1–2.5), P2 polling interval; R1, R2, R3, R4; U1–U11; OQ-1, OQ-2.

### Epic 2: Own controller firmware plays on the Switch (M3)
The owner's own controller firmware enumerates as a wired Pro Controller (P2), reports `console_ready`, enforces the safety rules and plays a hard-coded test timeline on the console with ≤ 1 frame jitter.
**Covers:** FR-23 (firmware side), NFR-02, NFR-05 (controller watchdog), NFR-11; AD-1, AD-2, AD-3, AD-5, AD-6, AD-7, AD-8, AD-9, AD-10, AD-11, AD-12, AD-14, AD-19, AD-20; F.1–F.6, F.9, F.10, H.2.
**Notes:** runs in parallel with Epic 1, with two sync points: Story 2.4 takes the AD-9 allow-list as confirmed by Story 1.1 (U11), and Story 2.5 needs the measured profile from Story 1.5. Stories 2.1–2.3 are independent of Epic 1. No logic analyser is available: jitter is checked in two layers — firmware self-timing in debug builds (apply time vs `t_us`, IN-poll intervals → `report_period_us`, reported via debug STATUS/EVENT; max lateness ≤ one report period) and end-to-end console-audio recording with onset detection (e.g. aubio; IOI p95 ≤ 1 frame).

### Epic 3: The PC drives the controller through the bridge (M4)
With the bridge firmware flashed, a GUI-free development tool on the PC streams frames end to end and plays a melody on the Switch; unplugging the bridge or a wire returns the controller to neutral within 100 ms.
**Covers:** NFR-05 (A.3), NFR-11; AD-4, AD-6, AD-7, AD-13 (firmware side), AD-14, AD-18 (workspace), AD-20; F.7, F.8, H.1, H.3; 460800-baud soak; OQ-3, OQ-4.
**Notes:** the PC side is not a throwaway script. This epic creates the `app/` Cargo workspace with the `ocb-core` crate holding only `protocol` and the sans-IO `link`, tested against `firmware/common/proto/vectors/`, plus a bare dev binary (`app/tools/ocb-dev`, two arguments: port and timeline file). App CI starts here and checks `PROTO_VERSION` against the vector header. Epic 4 reuses the crate.

### Epic 4: App shows the link state and drives the Switch with the virtual gamepad (M5)
The GUI app starts on Windows, Linux and macOS, finds the bridge by itself, shows the three status LEDs with versions, and its on-screen gamepad (mouse + keyboard) takes the user into ocarina free play; STOP and Esc always release every input.
**Covers:** FR-08, FR-10 (detection + gating), FR-11, FR-15 (partial), FR-22, FR-23, NFR-01 (CI), NFR-06, NFR-07 (licence gate); AD-7, AD-13, AD-16, AD-18, AD-19, AD-21; UX-DR1–11, UX-DR13 (detection + gating, OK-only dialog), UX-DR35–38, UX-DR41, UX-DR61, UX-DR62, UX-DR65; rotating file log (conventions); OQ-5, OQ-6, OQ-7, OQ-9.
**Notes:** reuses `ocb-core` from Epic 3. Kept lean: UI foundations (tokens, fonts, painting helpers, layout), status strip and LEDs, gamepad, STOP, file log, and a small decision story for the AccessKit level (OQ-9) before the widget helpers are built. Decided "basic" (Story 4.2): every helper or custom widget that draws an interactive or informative element calls `Response::widget_info` with the closest `egui::Role` and the label passed to it as the name (the visible label where there is one, plain words otherwise, never symbol glyphs); ornaments, and symbols painted inside another widget, get none. No forward links to the wizard: the required-update dialog has only "OK" with manual-flash instructions (BOOT + drag the matching `.uf2`); "Set up / Reflash…", LED click → wizard, "Update now" and the recommended-update banner arrive in Epic 8. Settings modal moves to Epic 5; Log view, Copy diagnostics and About move to Epic 8.

### Epic 5: Load a song and hear it on the PC (M6)
The user opens a `.mid` or text score (or an example), picks the track, transpose and tempo, sees the fit proposal and the Roll view with fingerings, and listens to it on the PC; nothing is sent to the Switch.
**Covers:** FR-01, FR-02, FR-03, FR-04, FR-05, FR-05a, FR-05b, FR-06 (profile module), FR-12, FR-19 (proposal display), FR-21 (Roll + Listen); AD-15, AD-17; UX-DR14–24, UX-DR31–34, UX-DR47–49, UX-DR58 (Settings modal: countdown, volume, Advanced mapping-profile fields; About excluded); OQ-11 (examples), OQ-14.

### Epic 6: Play the song on the Switch (M6)
The user plays a loaded song on the console from the cursor or the start, through Play gating, the readiness confirm and the countdown; interruptions show a banner with Resume, and the end card celebrates a finished song. The reference piece plays with 0 wrong notes (A.1).
**Covers:** FR-07, FR-24, NFR-08; AD-6, AD-8, AD-15, AD-19; UX-DR39–46.

### Epic 7: Timing holds up (M7)
NFR-04 is confirmed on all three OS with recorded measurements, or restated with the measured numbers; the profile timing margins and `app_timeout_ms` are set from data.
**Covers:** FR-05c, NFR-04; T.1–T.5, W.1, W.2.

### Epic 8: Set everything up from scratch inside the app (M8)
A user with fresh Zeros flashes the bridge and the controller from a guided wizard, wires them by the on-screen rules, turns on Wired Communication and tests with the gamepad; required and recommended firmware updates lead back to the wizard.
**Covers:** FR-09, FR-10 (update flow), FR-15, NFR-09, NFR-10 (wizard), NFR-11 (wiring rules); AD-13, AD-14, AD-16; UX-DR52–57, UX-DR12 (recommended-update banner), UX-DR13 (Update now → wizard), UX-DR9 (LED click → wizard step), UX-DR8 ("Set up / Reflash…" link), UX-DR58 (About + licences, "Set up / Reflash…" in Settings), UX-DR59 (Log view), UX-DR60 (Copy diagnostics), UX-DR65 (Linux/Windows); A.4; OQ-8, OQ-12 (photo slot).

### Epic 9: Compose and edit songs
The user writes a score in the in-app editor with a live note view, draws and edits notes in Roll and Staff, fixes out-of-range notes per note, edits a `.mid` (converted to a text score), and undoes or redoes any of it.
**Covers:** FR-16, FR-17, FR-18, FR-19 (per-note actions), FR-20, FR-21 (Staff); UX-DR22, UX-DR25–30.

### Epic 10: Pick up where I left off
The app reopens the last song with its settings, remembers per-song settings and recent files, saves scores, autosaves a draft and restores it after a crash.
**Covers:** FR-25; UX-DR50, UX-DR51; OQ-10.

### Epic 11: Release 1.0 (M9)
Pre-built app for Windows, Linux and macOS plus both UF2s on GitHub Releases; README with the first-run unblock and wiring photo; licences and notices shipped; acceptance tests A.1–A.5 pass with a first-time user.
**Covers:** NFR-01, NFR-07, NFR-08, NFR-10; AD-14, AD-21; release workflow; OQ-2 (copy check), OQ-11, OQ-12, OQ-13.

### Epic 12: Extras (optional, M10)
The user can play live from a MIDI keyboard and run a calibration wizard that writes measured timing into their profile; a single-board variant is explored.
**Covers:** FR-13, FR-14; Spike 3; AD-15 (live path), AD-17 (user profile).

## Epic 1: Ocarina mechanics verified (M2)

The owner knows, from measurements on their own Switch 2, which buttons and modifiers play which pitches and with what timing limits, and a spike script already plays a whole melody through the existing 2wiCC queue (walking skeleton, R4). Spikes use existing firmware only (2wiCC + 2wiCC_Comms, test use, nothing redistributed). Every run records the test environment (Switch 2 system version, game version, port, cable, firmware + version, host OS) in `docs/test-plan.md`.

### Story 1.1: Verify note buttons and modifiers by hand

As the owner,
I want to check the community ocarina chart in game with a real controller,
So that the riskiest unknowns (R1, R2) are settled before any code is written.

**Acceptance Criteria:**

**Given** a real controller, the game in ocarina free play and a phone tuner or recorder
**When** the owner runs Spike 1 rows 1.1, 1.2, 1.3, 1.4, 1.5 and 1.7 from `docs/test-plan.md`
**Then** the pitch of each of the 5 natural buttons and of every fingering in the chart is recorded in the Spike 1 table
**And** U1 (sustain), U3 (new button while holding), U4 (bend while sounding), U8 and U9 (free play enter/exit and limits) have recorded answers in `docs/ocarina-input-mechanics.md`

**Given** the results of row 1.8 (an official song played on natural notes, then with one note on an alternative fingering)
**When** the owner records whether the game matches songs by buttons or by pitches
**Then** R2's mitigation in `risk-register.md` is updated to "alternative fingering" or "warn only"

**Given** the answer to U9
**When** the owner decides UX Open Question 1 (should the app take the ocarina out itself before Play)
**Then** the decision is written in `EXPERIENCE.md` Open Questions as resolved, with the reason

**Given** the answer to U11 (other buttons such as ZL/ZR or the right stick)
**When** an input outside L, R, A, X, Y, hat and left stick is found
**Then** the AD-9 allow-list change is noted as a TODO for Story 2.4; otherwise the allow-list is confirmed as is

### Story 1.2: Spike sender for the 2wiCC queue

As the owner,
I want a tiny script that sends a list of timed controller states to the 2wiCC bridge,
So that the scripted spikes and the walking skeleton can run without any firmware of ours.

**Acceptance Criteria:**

**Given** the 2wiCC / 2wiCC_Comms serial command set is not recorded in this repo (only `+QD` appears in Spike 0b)
**When** the owner reads the upstream 2wiCC and RiiDraw sources and docs (reading only, no code copied)
**Then** `spikes/README.md` documents the subset the script uses: command syntax, the state encoding (buttons, hat, sticks), how queued states are timed (per frame or timestamp), the queue size and how the script avoids overflowing it, with links to the sources

**Given** the 2wiCC pair wired as in Spike 0b and the bridge port at 460800 8N1
**When** the owner runs the script in `spikes/` (Python 3 with `pyserial`, single file, run with `uv run`) with a small input file of `(time, buttons, hat, stick)` rows
**Then** the console receives the presses in order using the 2wiCC queue commands
**And** the script and its README say it is throwaway spike code, licensed MIT, and that 2wiCC binaries are used for testing only and never committed

**Given** a stop request (Ctrl+C) or the end of the file
**When** the script exits
**Then** it sends a neutral state so no input stays pressed

### Story 1.3: Stick threshold and alternative fingerings

As the owner,
I want to measure the stick deflection the game needs and check every alternative fingering,
So that the mapping profile can list all usable combinations per pitch.

**Acceptance Criteria:**

**Given** the spike sender from Story 1.2
**When** the owner runs Spike 1 row 1.6 with `ly = 0x40, 0x20, 0x00` (and the mirrored values for down)
**Then** the smallest deflection that always counts as ↑/↓ is recorded (U6), with diagonals noted

**Given** the 45 button × modifier combinations
**When** each one is played in free play
**Then** its pitch is recorded and the full list goes into `docs/ocarina-input-mechanics.md`
**And** no HYPOTHESIS marker remains on pitches or modifiers in that doc (UX Open Question 2 can be closed for the chart data)

### Story 1.4: Timing limits

As the owner,
I want to measure the minimum press, retrigger gap, modifier lead and frame rate,
So that the timing rules and the frame grid come from data, not guesses.

**Acceptance Criteria:**

**Given** the spike sender and a recording setup (Switch video capture, capture card, or a phone on the console speaker; R10)
**When** the owner runs Spike 2 rows 2.1, 2.2 and 2.3 with 20 tries per step
**Then** U7 (min press), U2 (min gap same button) and U5 (modifier lead) each have the smallest value that passes 20/20 recorded in frames and ms

**Given** a 60 fps capture while playing
**When** frames are counted (row 2.5)
**Then** U10 (frame rate while playing) is recorded

**Given** a 120 BPM 16ths passage for 30 s (row 2.4)
**When** onsets are detected (e.g. aubio)
**Then** the onset jitter σ is recorded and compared with the 1-frame target, noting that 2wiCC's per-frame queue is not our scheduler

### Story 1.5: Measured default mapping profile

As the owner,
I want the measured mapping and timing values written into the bundled default profile,
So that the app pipeline (Epic 5) starts from real data.

**Acceptance Criteria:**

**Given** the results of Stories 1.1, 1.3 and 1.4
**When** the owner writes `app/profiles/` default profile for the game version tested, following the TOML shape in `docs/score-format-and-mapping.md`
**Then** it has an integer `schema` field, `[profile]` (name, frame_rate), `[buttons]` (sounding MIDI pitches), `[modifiers]` (including stick value), every alternative fingering, `[timing]` (measured value + a stated safety margin, including `max_sustain` from U1), `[range]` (B4–F6) and the fingering cost weights
**And** every value has a comment pointing to the spike row it came from
**And** roadmap M2 is marked DONE only when no HYPOTHESIS marker is left on pitches/modifiers and `[timing]` is filled

### Story 1.6: Walking skeleton plays a whole melody

As the owner,
I want a whole melody to play on the Switch from a hand-written note list,
So that I hear the project work end to end early (R4).

**Acceptance Criteria:**

**Given** the measured profile from Story 1.5 and the spike sender
**When** the owner converts a short public-domain melody (e.g. Ode to Joy) by hand or with a few lines of spike code into timed states using the measured fingerings and timing
**Then** the melody plays in free play with every note audible and the right pitch
**And** the result (including any wrong or missed notes) is recorded in `docs/test-plan.md`

## Epic 2: Own controller firmware plays on the Switch (M3)

The owner's own controller firmware enumerates as a wired Pro Controller (P2), reports `console_ready`, enforces the safety rules and plays a hard-coded test timeline on the console with ≤ 1 frame jitter. Runs in parallel with Epic 1, except two sync points: Story 2.4 starts after Story 1.1 (U11 can change the AD-9 allow-list) and Story 2.5 starts after Story 1.5 (measured profile). Jitter is measured in two layers (firmware self-timing + console audio), because no logic analyser is available.

### Story 2.1: Firmware build skeleton and CI

As the owner,
I want a pinned firmware toolchain that builds two flashable UF2s in CI,
So that both boards are always built the same way from day one.

**Acceptance Criteria:**

**Given** a fresh clone
**When** the owner runs `git submodule update --init` and configures with the `fw-release` preset
**Then** pico-sdk is a submodule at `firmware/external/pico-sdk` on tag `2.3.1` with only `lib/tinyusb` initialised (TinyUSB 0.18.0), `PICO_TINYUSB_PATH` is not overridden, and `PICO_BOARD=waveshare_rp2040_zero` (AD-1)
**And** `firmware/CMakeLists.txt` (CMake ≥ 3.25) defines `ocb_proto`, `ocb_hw` and the executables `bridge` and `controller`, with presets `fw-release`, `fw-debug` and `host-tests` (AD-2)

**Given** the two executables at this stage
**When** each UF2 is flashed on a Zero
**Then** it blinks the onboard WS2812 RGB LED (GP16, the small square between BOOT and RESET) through a PIO driver in `ocb_hw` (based on the pico-sdk `ws2812` PIO example, BSD-3), with a different colour per target (bridge vs controller), and configures no other pin
**And** the LED driver is non-blocking (PIO FIFO write, no busy wait), so later timing-critical loops can use it
**And** GP0, GP1 and GP16 (plus a debug-only toggle pin) are defined once in `ocb_hw/pins.h` (AD-20)

**Given** a push to any branch
**When** the GitHub Actions firmware job runs on ubuntu
**Then** it uses the pinned Arm GNU Toolchain 15.2.Rel1 and picotool 2.3.1, uploads `bridge.uf2` and `controller.uf2`, runs `ctest --no-tests=error` for `host-tests` (this story adds one trivial host test, e.g. that `pins.h` defines GP0, GP1 and GP16, so the gate is real from day one), and fails if a grep finds a GPIO number outside `pins.h`
**And** the firmware version comes from `git describe` with fallback `0.0.0-dev` (AD-14)

### Story 2.2: Protocol library and conformance vectors

As the owner,
I want one pure C protocol library tested against language-neutral vectors,
So that firmware and app can never disagree about the wire format.

**Acceptance Criteria:**

**Given** `firmware/common/proto/` (pure C99, no pico-sdk includes)
**When** it is built with the `host-tests` preset
**Then** it provides COBS encode/decode, CRC-16/CCITT-FALSE, frame encode/decode (`type | seq | len | body | crc16le`), every message struct of `docs/link-protocol.md`, the canonical 8-byte state and `NEUTRAL` (AD-10), `PROTO_VERSION`, `OCB_MAX_ENCODED_FRAME`, the link timing constants and the 460800 baud constant
**And** the decoder checks in the AD-3 order (COBS → len consistency treated as CRC error → CRC → type range → seq → semantics) and counts and drops empty frames and out-of-range types

**Given** this story assigns the numeric values of NAK reasons, EVENT codes, `mode` and `hid_profile`
**When** they are defined as `OCB_NAK_*`, `OCB_EVT_*` and `OCB_MSG_*` constants
**Then** the same values appear in `firmware/common/proto/vectors/` (one text file per area, `name: hex` lines, `proto_version` header)

**Given** the vector files
**When** test F.9 runs
**Then** every vector passes: COBS, CRC, every message, a maximum-length frame, an empty frame, back-to-back frames, a leading delimiter, a bad CRC, an overlong run and `NEUTRAL`
**And** CI fails if `PROTO_VERSION` differs from the vector header

### Story 2.3: Controller enumerates as a wired Pro Controller

As the owner,
I want the controller board to complete the P2 handshake with my Switch 2,
So that the console sees our firmware as a real wired Pro Controller.

**Acceptance Criteria:**

**Given** the controller UF2 built with `OCB_PROFILE=P2` (default) and "Pro Controller Wired Communication" ON
**When** the board is plugged into the console's top USB-C port
**Then** the console lists a wired Pro Controller (VID:PID `057E:2009`) on a stock Switch 2 with only the two Zeros and three wires (NFR-02, NFR-03)
**And** the profile implements the AD-12 interface: descriptors, control/OUT handler, `build_report(const ocb_state*)`, `tick()`, `ready()`, reading only the published state

**Given** the P2 handshake (USB `0x80` commands, subcommands for device info, SPI flash reads with stick calibration, player lights, IMU enable with zero IMU data, `0x30` input reports)
**When** it is implemented
**Then** it is written clean-room from public reverse-engineering docs, with no code copied from 2wiCC (NFR-07, ADR-004), and sources are cited in comments

**Given** a controller firmware-update subcommand from the console
**When** the profile receives it
**Then** it refuses it (C7)

**Given** a bus reset, suspend or unmount
**When** it happens
**Then** `ready()` becomes false and the handshake restarts after the reset; the board re-enumerates cleanly after dock/undock or console wake (ADR-006, R13)

**Given** a debug build
**When** the console sends subcommands
**Then** each one is logged through the SPSC ring (not in release builds)
**And** the board measures the median IN interval for `INFO.report_period_us` and records the measured Switch 2 polling rate in `docs/usb-controller-emulation.md` (closes the ADR-002 polling hypothesis)

### Story 2.4: Controller core: modes, queue, scheduler and safety mask

As the owner,
I want the controller's playback logic as plain C tested on my PC,
So that the rules that keep inputs safe are proven before they touch the console.

**Acceptance Criteria:**

**Given** `firmware/controller/core/` with no pico-sdk includes and a fake clock
**When** the `host-tests` preset runs
**Then** F.3 passes: every frame × IDLE/DIRECT/PLAYING follows the AD-8 table; entering IDLE publishes NEUTRAL, clears the queue and resets ordering; the abort latch is set by every unrequested cause and cleared only by STOP/HELLO; `NAK ABORTED` while set
**And** F.4 passes: `NEUTRAL|END` → `PLAYBACK_DONE`; END on non-neutral → `NAK BAD_STATE`; reserved flag bits must be 0; underrun judged at apply time → NEUTRAL in the same pass + `UNDERRUN`; head event overdue > one report period → `LATE`, no backlog fired; `t_us` strictly increasing and < 2^31 else `NAK ORDER`
**And** F.5 passes: HELLO/STOP during PLAYING with any seq → NEUTRAL, queue cleared, IDLE, `expected = seq+1`
**And** F.6 passes: in PLAYING every non-allowed button is cleared and the right stick centred (allow-list L, R, A, X, Y, hat, left stick, plus any input added from Story 1.1's U11 result); DIRECT is unmasked
**And** `START`/`SET_STATE` get `NAK NOT_READY` while the profile's `ready()` is false

### Story 2.5: Hard-coded timeline plays on the Switch with measured jitter

As the owner,
I want the controller to play a built-in test melody from its own clock,
So that I hear my own firmware play the ocarina and know its timing is tight.

**Acceptance Criteria:**

**Given** the core split of AD-11 (core 0: TinyUSB task, profile, report on every host poll; core 1: non-blocking RAM super-loop with the 1 MHz scheduler, sole writer of the state via a seqlock double buffer, reader retries ≤ 3 then NEUTRAL)
**When** a debug build with a hard-coded timeline (a melody from the Story 1.5 profile, ending in `NEUTRAL|END`) runs on the console in free play
**Then** the melody plays with every note at the right pitch

**Given** the debug build's self-timing
**When** the timeline plays
**Then** the firmware records, for every applied event, its lateness vs `t_us` and the IN-poll intervals, and reports them (debug STATUS counters or EVENT)
**And** the maximum lateness is ≤ one report period

**Given** a recording of the console audio (phone or capture)
**When** onsets are detected (e.g. aubio) and compared with the timeline
**Then** inter-onset error p95 ≤ 1 frame is recorded, or the gap is explained (firmware vs console/game) using the self-timing data

**Given** a debug build that hangs core 0, then one that hangs core 1 (H.2)
**When** the stall happens
**Then** the liveness rules reboot the board through the hardware watchdog (≤ 100 ms), the USB detaches and no input stays pressed on the console

### Story 2.6: Controller link: UART, reliability, session and STATUS

As the owner,
I want the controller to speak the full link protocol over UART,
So that any PC-side sender can drive it reliably once the bridge exists.

**Acceptance Criteria:**

**Given** UART RX on GP1 at 460800, IRQ-driven and reading the data register's error bits (no 8-bit DMA)
**When** frames arrive
**Then** they are decoded by `ocb_proto` and fed to the core; core 1 is the only UART TX writer

**Given** host tests F.1 and F.2
**When** they run
**Then** go-back-N matches AD-6 (accept only `seq == expected`, stale window 64 → cumulative ACK, duplicate of `expected−1` → cached response, wrap) and the NAK matrix matches (transport NAKs never advance `expected`, at most one per `expected`; semantic NAKs advance it)

**Given** a HELLO with the matching `proto_ver`
**When** it arrives in any state
**Then** it has STOP semantics and is answered with INFO v1 (`proto_ver, fw_ver[3], bridge_ver[3]` from the last HEARTBEAT or zeros, `hid_profile, queue_cap, app_timeout_ms, report_period_us`); before a matching HELLO every app frame except STOP gets `NAK VERSION` (AD-7)

**Given** a running controller
**When** STATUS is built
**Then** it carries `free, now_us, last_seq, mode, abort_count, last_event, flags` (+ debug counters), every 50 ms plus after ACKs, coalesced to ≤ 1 per 10 ms; TX priority EVENT > NAK > ACK > STATUS with lowest dropped on overflow and no blocking (AD-19)
**And** F.10 passes with a fake profile: `flags` bit1 follows `ready()` (0 before handshake, 1 after, 0 on suspend/unmount/bus reset and after > 100 ms without an IN report); bits 2–7 always 0; NOT_READY uses the same predicate

**Given** power-up (AD-20)
**When** the board starts
**Then** RX has an internal pull-down and TX stays high-Z until RX sees idle-high

### Story 2.7: Controller link watchdog

As the owner,
I want the controller to go neutral on its own when the link or the app disappears,
So that no input can stay pressed on my console (NFR-05).

**Acceptance Criteria:**

**Given** host tests with a fake clock
**When** no valid `HEARTBEAT` arrives within `OCB_LINK_TIMEOUT_MS` (default 80, ≤ 100) or a UART break is seen
**Then** the controller aborts to IDLE (NEUTRAL, latch set), sends `EVENT LINK_LOST`, clears `link_ok`, and releases TX until RX is idle-high again (AD-5, AD-20)

**Given** DIRECT mode
**When** no app frame arrives for `app_timeout_ms` (default 1000) or `HEARTBEAT.flags` shows a DTR falling edge
**Then** the controller aborts to IDLE
**And** neither rule applies in PLAYING, where the underrun rule covers a dead app

**Given** the link is down
**When** `START` or `SET_STATE` arrives
**Then** it gets `NAK NO_LINK`

## Epic 3: The PC drives the controller through the bridge (M4)

With the bridge firmware flashed, a GUI-free development tool on the PC streams frames end to end and plays a melody on the Switch; unplugging the bridge or a wire returns the controller to neutral within 100 ms. The PC side is the real `ocb-core` crate (protocol + sans-IO link), not a throwaway script; Epic 4 reuses it.

### Story 3.1: Bridge firmware: frame-aware pipe and heartbeat

As the owner,
I want the bridge board to forward whole frames to the controller and inject its own heartbeat,
So that the controller has a reliable link and a fail-safe signal independent of the PC.

**Acceptance Criteria:**

**Given** the bridge UF2
**When** it is plugged into the PC
**Then** it enumerates as USB CDC with VID:PID `2E8A:000A`, product string `Bonooru Bridge` and the flash unique ID as serial number (AD-13)
**And** its WS2812 LED shows the bridge colour from Story 2.1

**Given** bytes from the PC
**When** they arrive on CDC
**Then** the bridge only scans for `0x00` delimiters (never decodes), writes whole frames to the UART at 460800 with at most one app frame in flight plus one queued, drops and counts a run longer than `OCB_MAX_ENCODED_FRAME` without a delimiter, and discards a partial frame when DTR falls or CDC unmounts (AD-4)
**And** F.8 passes in host tests

**Given** a hardware alarm every 20 ms, whatever the USB state
**When** a heartbeat is due
**Then** a `HEARTBEAT` (0x40) COBS+CRC frame with its own rolling seq and body `bridge_fw_ver[3], flags` (bit0 = DTR) goes out at the next frame boundary, ahead of queued app frames
**And** F.7 passes: during a 512-frame burst with a fake clock every heartbeat gap is ≤ 27 ms

**Given** bytes from the controller
**When** they arrive on UART
**Then** they are copied to CDC as they come and dropped (and counted) when the CDC IN FIFO is full; no code path blocks

**Given** the PC sets 1200 baud on the port
**When** the line coding changes
**Then** the bridge reboots into BOOTSEL via pico-sdk `pico_usb_reset` with `PICO_ENABLE_USB_RESET_VIA_BAUD_RATE=1`, with no own `tud_cdc_line_coding_cb`
**And** any other baud rate the PC sets is ignored

**Given** power-up (AD-20)
**When** the board starts
**Then** RX has an internal pull-down, the bridge probes with TX idle-high pulses of about 5 ms every 500 ms until RX shows the controller, then keeps TX enabled, and returns to probing on a break or link loss; F-test for the probe state machine

### Story 3.2: Rust workspace and protocol codec (`ocb-core`)

As the owner,
I want the app's protocol code in Rust, checked against the same vectors as the firmware,
So that the PC side and the firmware speak exactly the same wire format.

**Acceptance Criteria:**

**Given** `app/` as a Cargo workspace with `app/rust-toolchain.toml` pinning Rust 1.100.0
**When** the owner runs `cargo test`
**Then** crate `ocb-core` (lib) builds with no egui/eframe dependency, no threads and no I/O (AD-18)
**And** its `protocol` module provides COBS, CRC-16/CCITT-FALSE, frame encode/decode with the AD-3 receive order, every message type, the canonical state and `NEUTRAL` (AD-10)

**Given** `firmware/common/proto/vectors/`
**When** the core tests run
**Then** they read every vector file and pass all of them, and the `PROTO_VERSION` constant equals the vector header (AD-14)
**And** `proptest` round-trips COBS and frames for random payloads

**Given** a push
**When** the GitHub Actions app job runs on ubuntu, windows and macos
**Then** it runs `cargo test`, `cargo clippy -D warnings` and a `cargo-deny` licence check against the NFR-07 allow-list (AD-21)
**And** the app version comes from `git describe` with fallback `0.0.0-dev`

### Story 3.3: Sans-IO link session

As the owner,
I want a link session that handles HELLO, ACKs, retries and credits without touching a port,
So that reliability logic is fully tested on the PC before it meets real hardware.

**Acceptance Criteria:**

**Given** the `link` module in `ocb-core`, which takes bytes plus a clock and returns bytes plus typed events
**When** a session opens
**Then** it emits one `0x00`, then only `HELLO` every 100 ms until `INFO` arrives, and exposes INFO's values (`queue_cap`, `app_timeout_ms`, `report_period_us`, versions) without hard-coding them (AD-7)
**And** a `proto_ver` mismatch is reported as a typed "required update" event; `fw_ver`/`bridge_ver` ≠ app version as "recommended update" (AD-13)

**Given** an in-test fake controller with injected loss, corruption and reordering
**When** a stream of `QUEUE_EVENT`s is sent
**Then** go-back-N recovers every frame in order (cumulative ACK, resend after 50 ms, rewind only on a NAK whose `expected` is ahead of the last rewind point), credits follow `STATUS.free − QUEUE_EVENTs sent after last_seq`, and at most `min(credit, 64 − unacked)` frames are in flight (AD-6)

**Given** a semantic NAK on `QUEUE_EVENT` or `START`
**When** the session receives it
**Then** it abandons the piece, emits STOP and a typed event naming the reason

**Given** a mid-session `NAK VERSION`
**When** it arrives
**Then** the session re-sends HELLO and reports "controller rebooted"

**Given** DIRECT mode
**When** the session has sent nothing for `app_timeout_ms/4`
**Then** it emits `PING`

**Given** unknown EVENT codes or NAK reasons
**When** they arrive
**Then** they are reported as typed "unknown" events, never as errors that end the session (AD-19)

### Story 3.4: Dev tool plays a melody end to end

As the owner,
I want a small command-line dev tool that opens the bridge and streams a timeline,
So that I hear the full chain (PC → bridge → controller → Switch) play a melody.

**Acceptance Criteria:**

**Given** `app/tools/ocb-dev` (a dev-only binary, not shipped to users; FR-15 applies to the user app) with exactly two arguments: the serial port and a timeline file (`t_us,state` CSV)
**When** the owner runs it with a melody built from the Story 1.5 profile, both boards wired and the console in free play
**Then** it opens the port (DTR asserted), runs the session, prefills, sends `START` with the profile's start lead, streams on credit and the melody plays with every note at the right pitch
**And** on macOS it opens only `/dev/cu.*` ports

**Given** the console shows a wired controller only after its first input (Story 2.3 H.15)
**When** `ocb-dev` has opened the session and before it sends `START`
**Then** it sends D-pad down three times in DIRECT (never A), prompts the owner to dismiss OoT's controller connection screen with the Joy-Con and reopen the ocarina, keeps the link alive with `PING` while waiting, and starts only after the owner presses Enter (Story 2.5 D37)

**Given** Ctrl+C during playback
**When** the tool exits
**Then** it sends STOP and the console is neutral

**Given** the tool in a soak mode (same binary, a long generated timeline) at 460800 baud for 10 minutes, on a controller build without the built-in timeline (that build loads its table at every HELLO and answers every `QUEUE_EVENT` with `NAK BAD_STATE`, Story 2.5 D38)
**When** the run ends
**Then** the controller's debug counters show no dropped bytes and no CRC NAKs; the result is recorded (M4 exit; if bytes are lost, the 921600 option is evaluated as a build-constant change)

### Story 3.5: Hardware fail-safe and `console_ready` checks

As the owner,
I want the fail-safe and console-state behaviour measured on my real hardware,
So that I can trust the boards with my console and settle the open timing questions.

**Acceptance Criteria:**

**Given** a melody playing through `ocb-dev`
**When** the owner unplugs the bridge mid-song 20 times, and separately pulls a UART wire mid-song 20 times (A.3)
**Then** the console goes neutral within 100 ms every time (checked by audio stop or the controller's debug data) and the controller releases TX after the break or link loss

**Given** the bridge unpowered and the controller on the Switch (H.1)
**When** the owner measures the voltage at the bridge's GP1 with a multimeter
**Then** it is ≈ 0 V (no back-powering)

**Given** H.3 on the Switch 2 with P2
**When** the owner plugs in with Wired Communication ON, puts the console to sleep and wakes it, then switches Wired Communication OFF and ON
**Then** `console_ready` is 0 until the handshake and 1 after; back to 0 within 150 ms on OFF; on sleep the console cuts USB power (Story 2.3 H.16), so the controller powers off, STATUS stops and, after wake, the session reports "controller rebooted" and `console_ready` is 1 again after the new handshake; 1 again after ON; `START` gets `NAK NOT_READY` whenever it is 0
**And** results are recorded in `docs/test-plan.md`

**Given** the H.3 data
**When** the owner decides UX Open Questions 3 (STATUS staleness timeout; candidate 3 × 50 ms) and 4 (the 100 ms polling-stall rule)
**Then** both decisions are written into `EXPERIENCE.md` (Open Questions resolved, Hardware & Protocol Coupling gap closed) and, if the 100 ms rule changes, into AD-19 and the firmware constant

## Epic 4: App shows the link state and drives the Switch with the virtual gamepad (M5)

The GUI app starts on Windows, Linux and macOS, finds the bridge by itself, shows the three status LEDs with versions, and its on-screen gamepad (mouse + keyboard) takes the user into ocarina free play; STOP and Esc always release every input. Reuses `ocb-core` from Epic 3. No forward links to the wizard (Epic 8): the required-update dialog has only "OK".

### Story 4.1: App shell with theme, fonts and main-screen regions

As the owner,
I want the app window to open on all three OS with the Hybrid look and the main-screen regions in place,
So that every later feature lands in a consistent, themed layout.

**Acceptance Criteria:**

**Given** crate `ocb-app` (bin) in the workspace, depending on `ocb-core`, with eframe/egui pinned at exactly 0.37.0
**When** the app starts
**Then** it opens a dark-only window (OS light/dark ignored) with minimum size 1024 × 680 logical points and HiDPI via `pixels_per_point` (UX-DR5)
**And** a `theme` module holds every `DESIGN.md` token (colors incl. alpha overlays, spacing, named sizes, radii, typography) as the single source (UX-DR1)

**Given** the bundled TTFs Cormorant Garamond and Nunito Sans (variable `wght` axis) and egui's Hack monospace
**When** text is drawn
**Then** display styles use Cormorant, everything else Nunito Sans, weights come from `wght` (never `.strong()` as bold), labels are uppercase with 0.06 em tracking (UX-DR2)
**And** the font licence texts are stored in the repo for later display in About

**Given** the main screen
**When** it is laid out
**Then** regions are computed rects filled with `scope_builder` in reading order — status strip (44 px), song panel (260 px), note-view card (parchment, all remaining width), gamepad panel (300 px), bottom bar (88 px) — with empty placeholder content (UX-DR5)

**Given** Linux
**When** the app is built
**Then** eframe is built without the `wayland` feature (X11/XWayland) (AD-18)

**Given** CI on ubuntu, windows and macos
**When** the app job runs
**Then** a headless smoke test starts the GUI and exits cleanly
**And** `cargo-deny` bans HTTP/network client crates (e.g. `reqwest`, `ureq`, `hyper`) so no update check or telemetry can slip in (AD-21: no network)

**Given** UX Open Question 7 (proposed status hexes, `amber-ink`, `scrim`, editor font)
**When** the owner sees them in the running app
**Then** they are confirmed or changed in `DESIGN.md` and the theme module

### Story 4.2: Decide the screen-reader (AccessKit) level

As the owner,
I want to decide how far the app supports screen readers before any widget helper exists,
So that accessibility is built in from the start or consciously left out.

**Acceptance Criteria:**

**Given** UX Open Question 9 and egui 0.37's AccessKit support
**When** the owner decides between "none", "basic labels via `widget_info` in shared helpers" and "full"
**Then** the decision and its reason are recorded in `EXPERIENCE.md` (Accessibility Floor, question resolved)
**And** if the level is not "none", Story 4.3 must emit `widget_info` from every shared helper

### Story 4.3: Shared painting helpers and modal dialog

As the owner,
I want one set of painting helpers for focus, glow, dashes, brackets and symbols,
So that every widget looks consistent and the egui limits are handled once.

**Acceptance Criteria:**

**Given** the theme module
**When** any focusable themed widget is drawn
**Then** it calls `paint_focus_ring(resp, radius, on_parchment)` (2 px amber, 2 px offset; amber-ink on parchment) (UX-DR3)

**Given** the helpers module
**When** a widget needs it
**Then** it provides: glow via `epaint::Shadow` on rounded rects/circles only; a blurred rounded-rect halo behind text; a dashed rounded-rect helper on `dashed_line_many`; convex-only fills; strokes floored at 1 physical pixel; fixed-width digit cells for times; painted symbols ▶ ■ ⏮ ▾ ▲ ⌫ ↵ ↑ ♯ ♪ ↶ ↷; and the corner-bracket ornament (UX-DR3, UX-DR4)
**And** a hidden debug gallery screen shows each helper so they can be checked by eye

**Given** a modal dialog request
**When** it is shown
**Then** it uses the `modal-dialog` component (440–460 px, flourish rule, checklist rows, primary/ghost actions, scrim), only one modal is open at a time, and Esc closes it (UX-DR7)
**And** every shared helper that draws an interactive or informative element takes an accessible label and calls `Response::widget_info` with the closest `egui::Role` (buttons `Button`, a symbol shown on its own `Image` named with plain words, never the glyph); the symbol helper also has a variant without `widget_info` for symbols painted inside another widget (button, Roll, Staff); decorative ornaments (focus ring, glow, halo, dashed rect, corner brackets) add no node; a kittest test checks each one (`docs/test-plan.md` G.10)

### Story 4.4: Serial worker, bridge discovery and file log

As the owner,
I want the app to find my bridge by itself and keep the link in a background worker,
So that the UI never freezes and I never pick a serial port (FR-08).

**Acceptance Criteria:**

**Given** a services layer with one serial worker thread (AD-16)
**When** the app runs
**Then** the worker alone owns the port and the `ocb-core` link session; the UI exchanges only messages (commands in; events and immutable snapshots out) and calls nothing that blocks on I/O

**Given** the worker rescans ports about every 1 s
**When** ports are listed
**Then** it matches VID:PID `2E8A:000A`, uses the product string `Bonooru Bridge` as an extra filter where the OS reports it, sends the HELLO probe only to VID:PID matches, and on macOS considers only `/dev/cu.*` (AD-13, AD-18)
**And** a VID:PID match with a different product string is reported as a foreign board and is never written to

**Given** Linux without serial permission
**When** opening the port fails with a permission error
**Then** a typed event carries the plain message with the exact command (`sudo usermod -aG dialout $USER`, then log out and back in) and the app never escalates privileges (UX-DR65)

**Given** any EVENT, NAK, version or port change
**When** it happens
**Then** it is written to a rotating log file in the OS log dir (`directories`)

### Story 4.5: Status strip with three live LEDs

As the owner,
I want three always-visible LEDs for PC↔bridge, bridge↔controller and controller↔Switch,
So that I can see at a glance what is ready while my eyes are on the console (FR-23).

**Acceptance Criteria:**

**Given** the status strip
**When** it is drawn
**Then** it shows the brand, the three Status LEDs 20 px apart and the gear icon (Settings arrives in Epic 5; until then the gear is hidden) (UX-DR8)

**Given** a Status LED
**When** its state is ok, warning, error or unknown
**Then** it shows colour + symbol (filled dot with glow, triangle, cross, hollow ring) + label (+ detail), never colour alone; the tooltip gives detail + fix on hover and on keyboard focus; copy follows the Voice table ("Bridge v0.3", "Switch – not ready", "Controller – not answering", "Bridge – not found") (UX-DR9, UX-DR62)

**Given** the Hardware & Protocol Coupling table
**When** signals arrive
**Then** LEDs follow it: no VID:PID → PC↔Bridge error; foreign product string → warning naming the board; ours → ok without version; INFO → Bridge↔Controller ok with versions; no INFO/STATUS → Bridge↔Controller error, Switch unknown; `link_ok` 0 → Bridge↔Controller error; `console_ready` 0 → Switch warning (tooltip adds "Undock the Switch" after a suspend); NAK NOT_READY / NO_LINK treated like the flags; mid-session NAK VERSION → briefly unknown while HELLO is resent; port vanished → PC↔Bridge error (UX-DR10, UX-DR11)
**And** LEDs start unknown and resolve within about 1 s; when STATUS stops for longer than the staleness timeout decided in Story 3.5, the affected LEDs drop to unknown

**Given** UX Open Questions 5 (Windows, bridge alone) and 6 (bridge label without INFO)
**When** the owner checks the behaviour on Windows and with the bridge alone
**Then** both are decided and written into `EXPERIENCE.md`
**Note (Story 4.4 bench, macOS, 2026-11-27):** the bridge is listed as `ours` (product `Bonooru Bridge` reported) and, alone (controller unplugged), accepts HELLO every 100 ms with none dropped (H.31: `hello_frames=100 hello_dropped=0`): it keeps reading USB with no controller. Windows (`unconfirmed`, no product string) and Linux data follow with issue #43.

**Given** the app opens no network connections
**When** it runs
**Then** nothing in the status strip implies online features (NFR-06)
**And** each LED is a `Role::Status` AccessKit node named with its visible label + detail, e.g. "Bridge v0.3" (Story 4.2: basic)

### Story 4.6: Firmware version check and gating

As the owner,
I want the app to refuse to drive boards whose protocol doesn't match,
So that an old firmware can never misread my inputs (FR-10).

**Acceptance Criteria:**

**Given** INFO with `proto_ver` ≠ the app's `PROTO_VERSION`
**When** it arrives
**Then** a blocking "Firmware update needed" modal appears with one body line explaining that both boards must be reflashed with the `.uf2` files of this app version (hold BOOT, plug in, drag the file onto the RPI-RP2 drive) and a single "OK" button (no wizard link yet)
**And** after OK, Play and the gamepad stay disabled with the inline hint "The gamepad is off until both boards are updated." while the LEDs keep showing the boards' current versions (UX-DR13)

**Given** `fw_ver` or `bridge_ver` ≠ the app version with matching `proto_ver`
**When** INFO arrives
**Then** the mismatch is logged and nothing is blocked (the recommended-update banner arrives in Epic 8)

**Given** the mismatch persists
**When** the app is relaunched or the bridge reconnects
**Then** the modal shows again

### Story 4.7: STOP, Esc and the bottom bar

As the owner,
I want a big STOP that always works, also from the keyboard,
So that I can release every input instantly whatever the app is doing.

**Acceptance Criteria:**

**Given** the bottom bar
**When** it is drawn
**Then** it shows STOP with sub-line "Esc" (idle style, dim but clickable) and the two-line safety note "Don't dock the Switch mid-song." / "Esc stops at any time.", and reserves the slots for Play and Listen (empty until Epics 5–6) (UX-DR39)

**Given** any app state, including an open modal or a focused text field
**When** the user presses Esc or clicks STOP
**Then** Esc is read (not consumed) at frame start before any modal consumes it, and the worker sends STOP so the controller releases every input (UX-DR41, UX-DR61)
**And** with no bridge connected STOP is harmless
**And** STOP is a `Role::Button` AccessKit node named "Stop" (its visible label "STOP" in plain case, so screen readers do not spell it) with the description "Esc stops at any time." (the safety-note line; names from `DESIGN.md` › Bottom bar and `EXPERIENCE.md` › Microcopy); a kittest test finds it by role + name (Story 4.2: basic)

**Given** keyboard navigation
**When** the user presses Tab / Shift+Tab outside the gamepad
**Then** focus moves in the order status → song → note view → gamepad → bottom bar, buttons activate with Enter only and never with Space (UX-DR61)

### Story 4.8: Virtual gamepad with mouse

As the owner,
I want an on-screen Pro Controller I can click,
So that I can navigate the game to ocarina free play from the PC (FR-11, FR-22).

**Acceptance Criteria:**

**Given** the gamepad panel at ≥ 1280 px
**When** it is drawn
**Then** it shows an original Pro-Controller-like silhouette (union of convex fills + one stroked outline) with ZL/ZR, L/R, two sticks, D-pad, ABXY, −, +, Capture and Home, key chips, a legend list and the caption "Use it to reach ocarina free play." (UX-DR35, UX-DR64)

**Given** the link and console ready
**When** the user clicks a control, presses and holds it, or drags a stick
**Then** click = tap, hold = hold, sticks follow the drag and spring back on release; each change sends `SET_STATE` (DIRECT mode) and PING keeps the session alive when idle (UX-DR36, AD-7)
**And** the controls are not in the Tab ring (`Sense::CLICK | DRAG` without FOCUSABLE)

**Given** the window loses focus
**When** any input is held
**Then** every input is released

**Given** the Switch not ready, the link down, or a required update pending
**When** the user clicks the gamepad
**Then** nothing is sent and an inline hint explains why (e.g. "Gamepad clicks are ignored until the Switch is ready.") (UX-DR38)

**Given** the owner in game
**When** they use only the on-screen gamepad
**Then** they can reach ocarina free play (M5 exit)
**And** each button control (A, B, X, Y, D-pad up / down / left / right, L, R, ZL, ZR, −, +, Home, Capture) is a `Role::Button` AccessKit node named after the control ("A", "ZL", "D-pad up", "Minus", "Plus", "Home", "Capture"), and an AccessKit click is a tap, checked in kittest with `Node::click_accesskit()` (not `click()`, which simulates the pointer); each stick is one `Role::Slider` node named "Left stick" / "Right stick" (AccessKit 0.25 has no two-axis role), with no click or value requirement (Story 4.2: basic)

### Story 4.9: Gamepad keyboard focus and compact layout

As the owner,
I want to drive the gamepad from the keyboard when it has focus, and keep it usable on small windows,
So that navigating the game is quick and the keyboard never presses buttons by accident.

**Acceptance Criteria:**

**Given** the gamepad panel
**When** the user clicks anywhere in it or tabs into it
**Then** the panel takes focus as one widget, shows the inset amber ring + wash and the "Keys active" chip, and a focus-lock filter keeps arrows and Tab inside the panel (UX-DR37)

**Given** the panel has focus
**When** keys are pressed
**Then** keys are matched by `physical_key`: Arrows/WASD = D-pad, H J K L = Y B X A, Q E = L R, Z C = ZL ZR, Enter = +, Backspace = −; Home, Capture and sticks are mouse only; key auto-repeat is ignored; key and mouse holds are tracked separately; Cmd/Ctrl combos keep their global meaning

**Given** the panel loses focus or the window blurs
**When** keys are held
**Then** every input is released and keys no longer reach the gamepad

**Given** a window between 1024 and 1279 px wide
**When** the gamepad is drawn
**Then** it uses the compact variant (200 px, buttons only, key hints in tooltips) (UX-DR6)
**And** the panel is a `Role::Group` AccessKit node whose name says whether keys are active, and the Story 4.8 control nodes are its children: the panel node is created before the controls and their `Ui` is built with `UiBuilder::accessibility_parent(panel_id)` (egui otherwise parents a node to the nearest ancestor that already has one) (Story 4.2: basic)

## Epic 5: Load a song and hear it on the PC (M6)

The user opens a `.mid` or text score (or an example), picks the track, transpose and tempo, sees the fit proposal and the Roll view with fingerings, and listens to it on the PC; nothing is sent to the Switch. All pipeline code lives in `ocb-core` (integer µs downstream of import; float seconds only inside import).

### Story 5.1: Mapping profile module

As the owner,
I want the app to load, validate and migrate mapping profiles in one place,
So that mapping and timing values are never in code and every module gets a valid profile (FR-06).

**Acceptance Criteria:**

**Given** the `profile` module in `ocb-core` and the bundled default from Story 1.5 in `app/profiles/` (embedded read-only)
**When** the app loads profiles
**Then** only this module parses TOML (`toml` + `serde`), checks the integer `schema`, validates ranges and required fields, and returns a typed `Profile` or a typed error (AD-17)

**Given** a user copy with the same name and game version in the OS config dir
**When** it is valid or migratable
**Then** it overrides the bundled default

**Given** a user copy that cannot be migrated
**When** it is loaded
**Then** it is refused, the bundled default is used, and a typed event carries "This mapping profile can't be read by this version. Defaults are in use."

**Given** timing values
**When** they are validated
**Then** a min press or min gap below `2 × max(report_period_us, frame time)` is rejected (AD-15)
**And** app settings are stored in a separate file from profiles

### Story 5.2: Text score parser

As the owner,
I want to write a melody as a text score and have it parsed into notes,
So that I can play hand-written melodies (FR-02).

**Acceptance Criteria:**

**Given** the draft format in `docs/score-format-and-mapping.md` (`<name><accidental?><octave>/<denominator>[.]`, `#`/`b`, rests `r/N`, optional `|` bar checks, `tempo N` anywhere, `transpose N`, `# title:` comment, raw `ms` durations like `E4:250`)
**When** the `score` module parses a file
**Then** it returns the common note model `(midi_pitch, start, duration)` in µs plus title, tempo and transpose
**And** every error has a line, column span, message and fix line (e.g. "Unknown note length “q”." / "Use 1, 2, 4, 8 or 16, e.g. D5/4.") so the editor (Epic 9) can underline it

**Given** an invalid file
**When** it is parsed
**Then** the parser returns all errors found plus the last valid parse up to the first error, never panics, and a `proptest` fuzz case confirms it

**Given** the grammar
**When** the story is done
**Then** `docs/score-format-and-mapping.md` describes the final grammar (no longer "draft")

### Story 5.3: MIDI import with track list

As the owner,
I want to open a `.mid` file and see its tracks with useful stats,
So that I can pick the melody track of any MIDI I find (FR-01).

**Acceptance Criteria:**

**Given** a `.mid` file (format 0 or 1)
**When** the `score` module imports it with `midly`
**Then** it applies the tempo map and returns, per track/channel with notes: name, channel, note count and the note list in the common model
**And** percussion channel 10 is skipped

**Given** a file that is not MIDI, is damaged, has no notes, or has only percussion
**When** import runs
**Then** a typed error names the reason ("not a MIDI file or score", "the file is damaged", "the MIDI file has no notes")

**Given** the same melody as a text score and as a `.mid`
**When** both are imported
**Then** a golden test shows identical note lists (CAP-1)

### Story 5.4: Mono reduction and range fit

As the owner,
I want any melody reduced to one line and fitted into the ocarina's range,
So that polyphonic or wide songs still become playable, with a clear report (FR-03, FR-04).

**Acceptance Criteria:**

**Given** a note list with chords and overlaps
**When** mono reduction runs
**Then** chords keep the highest note (skyline) and overlaps cut the earlier note

**Given** the profile range (B4–F6, sounding pitch)
**When** auto transpose runs
**Then** it tries every shift in −24…+24 and keeps the one with fewest out-of-range notes (tie: smallest shift, then most natural notes)
**And** a manual transpose value overrides it

**Given** notes still out of range
**When** the fit proposal is computed
**Then** each is proposed as an octave fold (with its target pitch) or a drop, and the fit report counts in range / folded / dropped and lists every affected note
**And** the track-picker stat "% in range" uses the same logic

### Story 5.5: Fingering and timing rules compile a timeline

As the owner,
I want every note turned into button + modifier presses on the frame grid,
So that the app produces the exact timeline the controller will play (FR-05, FR-05a).

**Acceptance Criteria:**

**Given** a fitted mono note list and a `Profile`
**When** the `pipeline` compiles it
**Then** fingering picks the lowest-cost sequence over all profile fingerings (Viterbi/DP with the profile's cost weights)
**And** timing rules snap to the profile frame grid, enforce min press, min gap on the same button and modifier lead, steal time from the previous note and never shift onsets
**And** no state holds two note buttons (U3), and a note longer than the profile `max_sustain_ms` is re-pressed after the min gap before the game cuts it (U1); the number of re-pressed notes is reported

**Given** the compiled result
**When** it is returned
**Then** it is a `Timeline` of events with `t_us` (u32, strictly increasing, < 2^31) and full canonical states, ending with `NEUTRAL|END`, plus annotations (note index, pitch, fingering) that are never sent (AD-15)
**And** notes that cannot meet the minimum durations at the current tempo are reported as "N notes too short"

**Given** property tests
**When** they run
**Then** they confirm: events sorted, onsets never shifted, min press/gap respected, END last, only allow-list inputs used, at most one note button per state, no press longer than `max_sustain_ms`

**Given** the reference piece (all 19 pitches)
**When** it compiles
**Then** every note has a fingering and the timeline matches a golden snapshot

### Story 5.6: Official-song detection

As the owner,
I want the app to spot official song sequences in my melody,
So that a piece doesn't accidentally trigger an in-game effect and end free play (FR-05b, R2).

**Acceptance Criteria:**

**Given** the official song sequences stored as profile data, as pitch sequences (Story 1.1: the game matches by pitch, at any tempo)
**When** the pipeline compiles a melody containing one
**Then** it returns a warning with the song title and the affected notes
**And** it does not try alternative fingerings, since they do not break the match

**Given** the warning
**When** it is reported
**Then** song titles are the only game names used, and only for the detected song (UX-DR63)

### Story 5.7: Open a song in the GUI

As the owner,
I want to open or drop a song file, pick its track, or click an example,
So that I can load a melody without any command line.

**Acceptance Criteria:**

**Given** no song loaded
**When** the main screen shows
**Then** the note-view card shows the empty state: drop zone ("Drop a .mid or score here", Open…), "Or try an example" chips with 3–5 bundled public-domain examples already in range (final list decided now, UX Open Question 11), header transport dimmed and "0:00 / 0:00"; the song panel shows "No song loaded" with "Open…"; the gamepad stays usable; "New song" is not shown until Epic 9 adds the editor (UX-DR47)

**Given** File › Open… / Cmd/Ctrl+O (native dialog via `rfd`) or a file dropped on the window
**When** a `.mid` or text score is chosen
**Then** it is imported, the song panel shows the filename and the mapping-profile footer hint, and the track picker (`.mid` only) auto-selects the likely melody track (most notes in range, least polyphony) with rows "name · ch N · N notes · N % in range"; changing the track recompiles (UX-DR14, UX-DR15)
**And** while a file hovers, the drop zone border turns amber-ink (outside the empty state the card shows a drop outline) (UX-DR48)

**Given** an import failure or an unsupported format (MusicXML, ABC)
**When** it happens
**Then** the banner "Couldn't open <file>: <reason>." with Dismiss appears over the note view (unsupported formats suggest exporting to MIDI) and the current song (or empty state) is kept (UX-DR49)

**Given** a track with 0 % in range
**When** it is selected
**Then** it still loads, every note shows as a ghost note and the picker shows "0 % in range"

### Story 5.8: Roll view

As the owner,
I want to see the song as a piano roll on the 19 ocarina lanes with fingerings,
So that I understand what will be pressed before playing (FR-12, FR-21).

**Acceptance Criteria:**

**Given** a loaded song
**When** the note-view card draws the Roll
**Then** it shows the parchment card with corner brackets; the header with the Roll toggle segment (Staff disabled until Epic 9), transport placeholder, time in digit cells and "Preview · PC only"; 19 lanes B4–F6 with accidental lanes in `-alt` fills; lane headers (note name + fingering from the compiled timeline, ♯ and ↑ painted); ruler with ticks every beat and labels every 2 s; note bars with no text (UX-DR20, UX-DR21)

**Given** the Roll
**When** the user clicks the ruler, scrolls, Shift+scrolls, middle-drags or Cmd/Ctrl+scrolls
**Then** the click sets the shared cursor; scroll pans vertically/horizontally; Cmd/Ctrl+wheel zooms around the pointer
**And** when lanes don't fit, they shrink to 18 px and then the lane area scrolls with the ruler pinned

**Given** about 2,000 notes on screen
**When** panning and zooming
**Then** frame time stays near the spike's numbers (p95 ≈ 1.5 ms on the owner's Mac)
**And** the Roll is one `Role::Canvas` AccessKit node with a summary name (view, note count, range, e.g. "Roll view, 42 notes, B4 to F6"), no node per note (Story 4.2: basic)

### Story 5.9: Fit proposal and official-song group in the Roll

As the owner,
I want out-of-range notes and official-song sequences visible in the Roll,
So that I see exactly what the app changed or warns about.

**Acceptance Criteria:**

**Given** a song with out-of-range notes
**When** the Roll is drawn
**Then** two zones above F6 and below B4 (30 px, collapsing to a 6 px strip when empty) show ghost notes with dashed outlines, fold targets as dashed outlines in the target lane linked by a dashed leader, and labels like "G6 · out of range — proposed: fold to G5" (UX-DR23)

**Given** an unresolved official-song warning
**When** the Roll is drawn
**Then** the affected notes sit in an ember-wash group with a dashed ember outline, filled ember, and a label tab "Triggers <Song>" (UX-DR24)

**Given** UX Open Question 14
**When** the owner decides between Play-accepts-the-proposal only and explicit "Accept all" / "Reset to proposal" buttons
**Then** the decision is recorded in `EXPERIENCE.md` and, if buttons are chosen, they sit next to the fit summary (UX-DR31)

### Story 5.10: Transpose, tempo and song warnings

As the owner,
I want to change transpose and tempo and see the effect at once,
So that I can make a song fit and stay playable.

**Acceptance Criteria:**

**Given** the song panel
**When** the user uses the Transpose stepper (−12, −1, +1, +12, Auto)
**Then** the large value updates, the `auto` chip shows while the value equals the automatic result, the song recompiles and the fit summary "N in range · N folded · N dropped" (plus "N long notes re-pressed" when any) updates on every change (UX-DR16)

**Given** the Tempo control (−5/+5 %, Reset, 50–150 %, linked BPM field that scales the whole tempo map)
**When** the tempo changes
**Then** the song recompiles; if notes get shorter than the profile minimum the value turns `status-error` and "N notes too short" shows (Play gating uses this in Epic 6) (UX-DR17)

**Given** an official-song warning
**When** the song panel draws
**Then** the warning row shows "N official-song sequence: could end free play" in the ember style (UX-DR18)

**Given** these controls
**When** the story is done
**Then** each change is emitted as an edit command so Epic 9's undo history can wrap it without changing this code

### Story 5.11: Preview on the PC (Listen)

As the owner,
I want to hear the song on my PC from the cursor,
So that I can check it before sending anything to the Switch (FR-12, FR-21).

**Acceptance Criteria:**

**Given** a loaded song
**When** the user clicks the header ▶, the Listen button or presses Space (outside a text field)
**Then** the core renders an original synthesised ocarina-like timbre from the timeline into a buffer, the audio adapter (`rodio`) plays it from the cursor, and the playhead follows the audio clock (AD-16, UX-DR32)
**And** nothing is sent to the Switch

**Given** the bottom bar
**When** it is drawn
**Then** Listen sits in its slot: secondary style, painted fairy-orb icon (one wing pair, original), label "Listen", sub-line "on this PC", tooltip "Preview the song on this PC (Space). Nothing is sent to the Switch."; pressing it while playing stops the preview; it is disabled with no song; no copy refers to the orb (UX-DR33, UX-DR64)
**And** Space is consumed at frame start and never activates a focused button (UX-DR61)

**Given** the audio output cannot be opened (e.g. Bluetooth profile switch)
**When** preview starts
**Then** the note-view header shows "Can't open the audio device." with Retry, the output re-opens when the default device changes, and there is no silent fallback (UX-DR34)

### Story 5.12: Song rail for small windows

As the owner,
I want to collapse the song panel on a small laptop screen,
So that the note view keeps the space.

**Acceptance Criteria:**

**Given** a window narrower than 1280 px
**When** the user clicks the chevron in the song panel tab
**Then** the panel collapses to the 48 px song rail with painted icons (Open, Track, Transpose, Tempo), each with the focus ring and a tooltip showing the current value; it never collapses automatically and the state is persisted (UX-DR19, UX-DR6)

**Given** the rail
**When** the user clicks an icon
**Then** the panel expands with that control focused

### Story 5.13: Settings modal

As the owner,
I want a Settings dialog for countdown, volume and the mapping-profile timing fields,
So that I can tune the app without editing files by hand.

**Acceptance Criteria:**

**Given** the gear in the status strip (now visible)
**When** the user opens Settings
**Then** a modal shows countdown length (0–5 s, default 3; used from Epic 6), preview volume, and a collapsed Advanced section (UX-DR58)

**Given** Advanced
**When** it is expanded
**Then** each mapping-profile timing field (min press, min gap same button, modifier lead, start lead, …) shows a one-line explanation and its unit; edits are written to the user profile copy through the `profile` module; "Reset to defaults" and "Open profile file" are available
**And** a refused user profile shows the inline message from Story 5.1 with "Open profile file"

**Given** Settings
**When** Esc or × is pressed
**Then** it closes (and Esc still sends STOP)

## Epic 6: Play the song on the Switch (M6)

The user plays a loaded song on the console from the cursor or the start, through Play gating, the readiness confirm and the countdown; interruptions show a banner with Resume, and the end card celebrates a finished song. The reference piece plays with 0 wrong notes (A.1).

### Story 6.1: Play from start streams the song to the Switch

As the owner,
I want a Play from start button that sends the compiled song to the controller,
So that the song I loaded actually plays on the ocarina (FR-07).

**Acceptance Criteria:**

**Given** a streamer in the services layer on top of the `ocb-core` link session
**When** playback starts
**Then** the worker prefills the queue up to the available credit, sends `START` with the profile's `start_lead_ms` as `lead_us`, then sends the remaining `QUEUE_EVENT`s greedily whenever there is credit, never editing, dropping or reordering events (AD-15, AD-6)

**Given** the bottom bar
**When** a song is loaded and all three LEDs are ok
**Then** "Play from start" (secondary style, painted ⏮, sub-line "0:00") is enabled; otherwise disabled (full gating rules in Story 6.2) (UX-DR39)

**Given** STOP or Esc during playback
**When** it is pressed
**Then** STOP is sent, every input is released and the app returns to idle

**Given** a semantic NAK on `QUEUE_EVENT` or `START`
**When** it arrives
**Then** the piece is abandoned and STOP is sent (the user-facing banner arrives in Story 6.6; until then the event is logged)

**Given** the tempo
**When** the user changes it
**Then** the timeline is recompiled; the firmware has no notion of tempo (AD-15)

### Story 6.2: Play from cursor and full Play gating

As the owner,
I want to play from the cursor and see exactly why Play is disabled when it is,
So that I never start a song when something isn't ready (FR-24).

**Acceptance Criteria:**

**Given** the bottom bar
**When** it is drawn
**Then** "Play from cursor" is the primary amber button with painted ▶ and a sub-line showing the cursor position ("0:42"); it compiles from that offset (UX-DR39)

**Given** any of: a LED not ok, a required firmware update pending, no song loaded, notes too short, an invalid text score, or playback in progress
**When** the Play buttons are drawn
**Then** both are disabled, and the tooltip names only the first blocking reason and its fix, in the order Bridge › Controller › Switch › firmware update › no song › too-short notes › invalid score, with the Voice-table strings (e.g. "Play needs all three lights on." + "Switch: turn on Pro Controller Wired Communication.") (UX-DR40)
**And** Listen and the preview stay available

**Given** the global shortcuts
**When** the user presses Cmd/Ctrl+Enter or Cmd/Ctrl+Shift+Enter (also while the gamepad has focus)
**Then** they trigger Play from cursor and Play from start, subject to the same gating (UX-DR61)

**Given** UX Open Question 4's decision from Story 3.5
**When** gating is implemented
**Then** it uses the decided `console_ready` rule and staleness timeout

### Story 6.3: Readiness confirm

As the owner,
I want a short checklist before every Play,
So that I don't start a song when the ocarina isn't out or my hands are on the controller (FR-24).

**Acceptance Criteria:**

**Given** an enabled Play action
**When** the user triggers it
**Then** the "Ready to play?" modal shows the checklist "The ocarina is out in free play." · "Don't touch the controller until the song ends." · "Don't dock the Switch.", a "Don't ask again" checkbox, Cancel (ghost) and Play (primary, initial focus) (UX-DR42)
**And** Enter = Play; Esc = Cancel (and a harmless STOP)

**Given** "Don't ask again" was ticked
**When** the user plays again
**Then** the confirm is skipped
**And** Settings gains a "Reset 'Don't ask again'" action that brings it back

### Story 6.4: Countdown

As the owner,
I want a visible 3·2·1 countdown before the song starts,
So that I can take my eyes to the console in time.

**Acceptance Criteria:**

**Given** a confirmed Play
**When** the countdown runs (length from Settings, 0–5 s, default 3)
**Then** an 80 % veil covers the note view only, large cream numerals with a frame outline and amber halo count down, the sub-line reads "Starting from 0:12 · hands off the controller", and an original wood-block tick plays per number (UX-DR43)
**And** STOP turns live (ember) and the gamepad and song panel lock

**Given** Esc or STOP during the countdown
**When** pressed
**Then** the countdown is cancelled and no START is sent

**Given** `link_ok` or `console_ready` drops, or the port vanishes, during the countdown
**When** detected
**Then** the countdown is cancelled, no START is sent, and the cause is logged (shown in the banner from Story 6.6)
**And** the countdown is separate from the technical `start_lead_ms`

### Story 6.5: Playing state and live gamepad view

As the owner,
I want to see where the song is and what is being pressed while it plays,
So that I can follow along and trust what the console is receiving.

**Acceptance Criteria:**

**Given** STATUS mode PLAYING
**When** the screen updates
**Then** the playhead and time interpolate locally from the last STATUS `now_us` plus elapsed time and resync on each STATUS (0 during the lead); the Roll follows the playhead (UX-DR44)

**Given** playback
**When** the gamepad panel draws
**Then** it shows the "Locked · live view" chip, accepts no input, lights every control the timeline presses at the interpolated time, and shows a caption like "Now: Y + D↑ (C5)" (UX-DR38)

**Given** playback
**When** the user interacts with the app
**Then** STOP is live; the song panel is locked; the note view is read-only (scroll allowed); drop and Open… are ignored and Cmd/Ctrl+O is disabled; the gear is disabled (UX-DR44)

### Story 6.6: Interruption banner and Resume

As the owner,
I want a clear message when playback stops early and a way to resume from that point,
So that a sleeping Switch or a loose wire doesn't mean starting over.

**Acceptance Criteria:**

**Given** EVENT UNDERRUN, LATE, LINK_LOST or USB_SUSPENDED, the bridge port vanishing, or a cancelled countdown
**When** playback stops
**Then** the app sends STOP to clear the latch and shows the dialogue-style banner over the note view with a name tab ("Playback stopped"), the cause, the fix and the time, using the State-pattern strings (e.g. "Playback stopped at 1:12 – the Switch went to sleep. Wake it, take the ocarina out, then Resume.") (UX-DR45)
**And** the cursor stays at the stop point

**Given** a semantic NAK that abandons the piece
**When** it happens
**Then** the banner says "Playback couldn't start" with Details

**Given** the banner
**When** the user clicks "Resume from <time>"
**Then** it acts as Play from cursor (confirm + countdown) and is disabled by the same gating, with the same tooltip
**And** Dismiss closes it; "Details" opens the log file location until the Log view exists (Epic 8)
**And** the banner persists until acted on; the app never reconnects and restarts a song by itself

### Story 6.7: End-of-song card and reference piece

As the owner,
I want a small celebration when a song finishes, and proof the reference piece plays perfectly,
So that the moment the song plays on the Switch feels complete (A.1).

**Acceptance Criteria:**

**Given** EVENT PLAYBACK_DONE
**When** it arrives
**Then** a parchment end card (about 440 px, corner brackets, frame ring) shows "You played <Song>!" (song name in amber-ink) and "1:48 · 62 notes · on time" (actual vs expected duration), with Play again and Close; it auto-dismisses after 6 s, the timer pauses while hovered, and it makes no sound (UX-DR46)

**Given** the 1-minute reference piece (all 19 pitches) and the measured profile
**When** the owner plays it from the app on the console
**Then** it plays with 0 wrong notes (A.1, NFR-08) and the run is recorded in `docs/test-plan.md` with the test environment

## Epic 7: Timing holds up (M7)

NFR-04 is confirmed on all three OS with recorded measurements, or restated with the measured numbers; the profile timing margins and `app_timeout_ms` are set from data. Recording uses a capture card or a phone/mic on the console speaker (R10); relative timing is enough because IOI error cancels the fixed capture latency.

### Story 7.1: Timeline export and measurement tooling

As the owner,
I want to export the exact timeline of a song and compare it with a recording,
So that every timing test has a ground truth and a repeatable analysis.

**Acceptance Criteria:**

**Given** a compiled timeline
**When** the owner exports it (hidden Advanced action or `ocb-dev` flag; not user-facing in v1)
**Then** a CSV with `t_us`, pitch and fingering per event is written (FR-05c, AD-15)

**Given** a recording of the console audio and the CSV
**When** the analysis script in `spikes/` (or `app/tools/`) runs
**Then** it detects onsets (e.g. aubio), aligns on the first note, and reports IOI error (p95, max, σ in ms and frames), drift (slope of cumulative error vs time), and missed or extra notes

**Given** the test pieces
**When** the story is done
**Then** the metronome piece (120 BPM, 8ths and 16ths), the 1-minute reference piece and a 5-minute piece exist as text scores in the repo
**And** the debug-build GPIO toggle per scheduled event remains optional (no logic analyser available)

### Story 7.2: Timing tests on the main OS

As the owner,
I want T.1, T.2, T.4 and T.5 run on my main computer,
So that I know whether the whole chain meets NFR-04.

**Acceptance Criteria:**

**Given** the tooling from Story 7.1 and the app on the owner's main OS
**When** T.1 (metronome, 60 s) runs
**Then** IOI error p95 ≤ 1 frame and max ≤ 1.5 frames, or the failing numbers are recorded

**Given** T.2 (5-minute piece)
**When** it runs
**Then** drift < 1 frame over the piece, or the number is recorded

**Given** T.4 (T.1 with CPU stress and other USB traffic)
**When** it runs
**Then** results show no change vs T.1

**Given** T.5 (fastest passage the profile allows)
**When** it runs 5 times
**Then** there are 0 missed or wrong notes
**And** every result and the test environment are recorded in `docs/test-plan.md`; a failure is localised with the firmware self-timing data (firmware vs console/game)

### Story 7.3: Cross-OS timing and playback soak

As the owner,
I want the timing and the watchdog checked on Windows, Linux and macOS,
So that I know playback doesn't depend on the host OS.

**Acceptance Criteria:**

**Given** the app built for all three OS
**When** T.1 and T.2 run on each (T.3)
**Then** results match the main OS within the pass criteria (timing is host-independent)

**Given** 30 minutes of playback with the host under load (W.2)
**When** it runs
**Then** there are 0 false watchdog trips
**And** results are recorded

### Story 7.4: Set timeouts and margins from data

As the owner,
I want `app_timeout_ms` and the profile's timing margins set from the measurements,
So that the safety and timing values are tuned, not guessed.

**Acceptance Criteria:**

**Given** a debug build and 30 minutes connected in DIRECT mode on each OS (W.1)
**When** the max-gap counter from debug STATUS is read
**Then** `app_timeout_ms` is set to max gap × a safety margin, ≤ 1 s, as the firmware constant, and AD-5 / `docs/link-protocol.md` are updated with the value

**Given** the results of Stories 7.2 and 7.3
**When** the owner reviews them
**Then** the bundled profile's `[timing]` margins are updated
**And** NFR-04 is marked confirmed in `requirements.md`, or restated with the measured numbers, and roadmap M7 is marked DONE

## Epic 8: Set everything up from scratch inside the app (M8)

A user with fresh Zeros flashes the bridge and the controller from a guided wizard, wires them by the on-screen rules, turns on Wired Communication and tests with the gamepad; required and recommended firmware updates lead back to the wizard. All illustrations are original Painter drawings; no console or game screenshots.

### Story 8.1: Flasher core and bundled UF2s

As the owner,
I want the app to carry both firmwares and copy them safely to a board in BOOT mode,
So that flashing needs no other tool and never touches the wrong drive (FR-09).

**Acceptance Criteria:**

**Given** the CI app job
**When** it builds the app
**Then** it embeds `bridge.uf2` and `controller.uf2` built in the same workflow run at the same version, and the app flashes only these (AD-14)

**Given** the `flasher` module in `ocb-core` and a drives adapter (`sysinfo` + `std::fs`)
**When** a flash runs
**Then** a flasher worker (which never opens a serial port) snapshots mounted volumes before the BOOT prompt or 1200-baud touch, accepts exactly one new volume containing `INFO_UF2.TXT` (label `RPI-RP2`, or a Linux mount folder named `RPI-RP2`), checks the RP2040 family in every UF2 block, copies with a plain read/write loop (not `fs::copy`), reports progress, and waits for the volume to disappear with a timeout (AD-16)

**Given** no new volume, two new volumes, a non-RP2040 drive, or a board that doesn't come back
**When** detected
**Then** a typed error names the case
**And** unit tests cover each case with a fake drive and a fake bootloader, as in the spike

**Given** Linux with `RPI-RP2` not auto-mounted
**When** detected
**Then** a typed event carries "Open the RPI-RP2 drive in your file manager to mount it, then Retry." plus the exact `udisksctl mount` command; no privilege escalation (UX-DR65)

### Story 8.2: Setup wizard frame and first run

As a first-time user,
I want a step-by-step wizard that opens on first launch,
So that I know exactly what to do to get set up (NFR-10).

**Acceptance Criteria:**

**Given** no valid bridge has ever been seen
**When** the app launches
**Then** the wizard opens at Welcome ("A melody waits to be played."), then step 1; the status strip with LEDs stays visible (UX-DR52, UX-DR53)

**Given** the wizard
**When** it is drawn
**Then** it shows the 280 px list of 6 clickable steps ("Flash the bridge", "Flash the controller", "Wire the two boards", "Plug both boards in", "Turn on Wired Communication", "Test") with check marks and the current step ringed in amber, and a step body with title, one instruction line, a parchment illustration sheet with corner brackets, a status line, and Back / Next

**Given** first run
**When** the user moves through the steps
**Then** they are walked in order and Next is enabled only when the current step is done (on step 3 Next is the confirmation); closing the wizard returns to the main screen in the "bridge not found" state

**Given** a later visit
**When** the wizard is opened
**Then** any step can be opened directly and steps auto-complete from detection

### Story 8.3: Flash the bridge and the controller

As a first-time user,
I want to flash each board by holding BOOT and plugging it in,
So that both boards run the right firmware without any other tool (FR-09, NFR-09).

**Acceptance Criteria:**

**Given** step 1 ("Hold BOOT on board 1 and plug it into your PC.") with the original flash illustration (board outline, BOOT ringed in amber-ink, cable to a generic laptop, callouts "1 · hold BOOT", "2 · plug into your PC")
**When** one new `RPI-RP2` drive appears
**Then** the bridge UF2 is copied with the progress bar ("Copying… 64 %") and the step completes when the drive vanishes and a new `2E8A:000A` port appears (product string checked where the OS supplies it, not on Windows) (UX-DR54, UX-DR55, UX-DR56)

**Given** step 2 ("Take board 2 off the Switch, hold BOOT and plug it into your PC.")
**When** the drive appears
**Then** the controller UF2 is copied and the step completes when the drive vanishes (verification completes at step 4)

**Given** a failure
**When** it happens
**Then** the status line shows the specific message with Retry ("No RPI-RP2 drive appeared. Unplug the board, hold BOOT, plug it back in.", "Two boards are in BOOT mode. Unplug one, then Retry.", "This drive isn't an RP2040 board. Use an RP2040-Zero.", "The board didn't come back after flashing. Unplug it and plug it in again.")
**And** after two failures a "Flash manually" row opens the folder with the `.uf2` and explains drag-and-drop

### Story 8.4: Wire the boards and plug them in

As a first-time user,
I want clear wiring instructions and a check that the boards talk to each other,
So that I wire them safely and know it worked (NFR-11, C1).

**Acceptance Criteria:**

**Given** step 3
**When** it is shown
**Then** the original wiring diagram shows two board outlines ("Board 1 · bridge (PC)", "Board 2 · controller (Switch)"), three wires "GP0 → GP1", dashed "GP1 ← GP0", "GND – GND", the 5V and 3V3 pins circled in ember with "Circled pins: 5V and 3V3. Never connect them.", the owner's wiring photo beside it (loaded with `egui_extras`; a placeholder until the photo exists, UX Open Question 12), and wiring rules 1–4 as a numbered list (UX-DR56)
**And** the instruction reads "Connect GP0, GP1 and GND only. Never 5V or 3V3. Unplug both cables before you rewire."

**Given** step 4 ("Plug the bridge into your PC." / "Plug the controller into the Switch's top USB-C port.")
**When** the PC↔Bridge LED is ok and INFO arrives
**Then** the step completes and the bridge LED shows its version
**And** if INFO doesn't arrive, the status line reads "No answer from the controller. Check the wires: GP0 goes to GP1, GP1 to GP0."

**Given** the bridge is found but no controller answers
**When** the wizard is open
**Then** it offers to reflash the bridge (AD-13)

### Story 8.5: Wired Communication and the final test

As a first-time user,
I want to be guided to the console setting and to a final test press,
So that I finish setup knowing the Switch reacts to the app.

**Acceptance Criteria:**

**Given** step 5
**When** it is shown
**Then** the original Wired Communication schematic (generic rows as ink bars, one target row "Pro Controller Wired Communication" with a toggle drawn On and an amber-ink ring) shows with the sub-line "Switch LED: “Switch – not ready” until the setting is On." and the path "System Settings › Controllers & Accessories › Pro Controller Wired Communication › On"; no console screenshot (UX-DR56, UX-DR64)
**And** the step completes when `console_ready` = 1

**Given** step 6 ("All three lights are on. Press A on the gamepad and check the Switch reacts.")
**When** the user presses A on the virtual gamepad and confirms
**Then** the wizard is done and the main screen opens (empty state or last song)

### Story 8.6: Reflash the bridge without BOOT, and the Windows port pick list

As the owner,
I want to update the bridge without unplugging it to press BOOT,
So that updates are quick, and never reboot someone else's board.

**Acceptance Criteria:**

**Given** a bridge confirmed by product string `Bonooru Bridge`
**When** the user starts a bridge reflash from the wizard
**Then** the serial worker sends the 1200-baud touch, the flasher waits for the new `RPI-RP2` drive and copies the bridge UF2 (AD-13, AD-16)

**Given** Windows (no product string) and one or more `2E8A:000A` ports
**When** a reflash needs the touch
**Then** the port pick list shows one row per port (port name, "2E8A:000A" muted, selected row amber-bordered) and the touch is sent only after the user clicks "Use this port" (UX-DR57)
**And** the pick list never appears on the main screen, and an unconfirmed port is never touched

### Story 8.7: Update flows lead to the wizard

As the owner,
I want firmware updates to take me straight to the right wizard step,
So that keeping both boards current is one click away (FR-10).

**Acceptance Criteria:**

**Given** the "Firmware update needed" modal from Story 4.6
**When** it is upgraded in this story
**Then** it shows "This firmware is too old for this app. Update both boards to continue." with Not now (ghost) and Update now (primary, initial focus, Enter); Update now opens the wizard at the right flash step; Not now / Esc keep Play and the gamepad disabled (UX-DR13)

**Given** `fw_ver` or `bridge_ver` ≠ the app version with a matching protocol
**When** INFO arrives
**Then** the slim banner "Firmware update available" [Update] [Later] appears under the status strip; Update opens the wizard at the right flash step; Later hides it until the next launch (UX-DR12)

**Given** the status strip
**When** it is drawn
**Then** it shows the "Set up / Reflash…" link, and clicking a LED that is not ok opens the wizard at the relevant step (UX-DR8, UX-DR9)
**And** a foreign-board LED tooltip offers "Set up…"

**Given** UX Open Question 8
**When** the update path is implemented
**Then** a Key Flow "update → wizard → reflash → Play re-enabled" is added to `EXPERIENCE.md`

### Story 8.8: Log view, diagnostics and About

As the owner,
I want to see recent link events, copy a diagnostics summary and read the licences,
So that problems can be understood and reported, and credits are given.

**Acceptance Criteria:**

**Given** Settings › Advanced › Show log, or "Details" on any banner
**When** the log view opens
**Then** it shows monospace rows like "12:04:31.220  EVENT  UNDERRUN  t=1:12" with muted timestamps, error rows in `status-error`, newest at the bottom with auto-follow unless scrolled up; from "Details" it scrolls to the entry and highlights it (UX-DR59)
**And** Story 6.6's "Details" now opens this view

**Given** "Copy diagnostics"
**When** clicked
**Then** a summary with app and firmware versions, OS and recent events is copied to the clipboard (UX-DR60)

**Given** Settings
**When** About is opened
**Then** it shows app, firmware and protocol versions and the third-party licences (Cormorant Garamond and Nunito Sans OFL, Hack, egui default fonts OFL/Ubuntu, `serialport` MPL-2.0 with source pointer, …), and Settings also offers "Set up / Reflash…" (UX-DR58)

### Story 8.9: Flash from scratch on all three OS (A.4)

As the owner,
I want the whole wizard proven with fresh boards on Windows, Linux and macOS,
So that I know a new user can set up on any OS (M8).

**Acceptance Criteria:**

**Given** a spare Zero (#3 or #4) wiped or running other firmware, and the app on each OS
**When** the owner flashes both roles through the wizard, with no other tool installed
**Then** every step completes on Windows, Linux and macOS (A.4)
**And** any OS-specific problem (Linux mount or `dialout`, macOS Finder quirks, Windows COM names) is fixed or explained in the app
**And** if mass-storage copy proves unreliable, the PICOBOOT fallback (R9) is raised as a new story

## Epic 9: Compose and edit songs

The user writes a score in the in-app editor with a live note view, draws and edits notes in Roll and Staff, fixes out-of-range notes per note, edits a `.mid` (converted to a text score), and undoes or redoes any of it. The text score is the source of truth; undo/redo, two-way sync and editing live in the app (ui + core score editing) with no protocol impact (AD-18).

### Story 9.1: App-level undo and redo

As the owner,
I want one undo/redo history for my song edits,
So that I can try changes freely and step back (FR-20).

**Acceptance Criteria:**

**Given** the edit commands emitted by the transpose and tempo controls (Story 5.10)
**When** the user changes transpose or tempo
**Then** each change is one step in an app-level history

**Given** the history
**When** the user presses Cmd/Ctrl+Z or Cmd/Ctrl+Shift+Z anywhere (including later the editor), or clicks the ↶ Undo / ↷ Redo buttons at the right of the note-card header
**Then** the last step is undone or redone; each button is disabled when its stack is empty and its tooltip names the shortcut ("Undo (Cmd/Ctrl+Z)") (UX-DR29, UX-DR20)
**And** track choice, view toggle, cursor and Settings are not in the history; the history is per song and cleared when another song loads; undo/redo are disabled during Play

### Story 9.2: Text-score editor with live note view

As the owner,
I want to type a score and see the notes appear as I type,
So that I can write melodies inside the app (FR-16).

**Acceptance Criteria:**

**Given** a text score loaded, or "New song" in the empty state (now enabled)
**When** the note-view card draws
**Then** it splits into the text editor on top (Well fill, Hack 13 px) and the note view below, separated by a draggable 6 px divider (grip amber on hover/drag, initial 40/60, position persisted, editor never below 4 lines, double-click resets) (UX-DR28)

**Given** the user types
**When** the text changes
**Then** the parser from Story 5.2 runs, the note view and timeline update live, errors get a straight `status-error` underline with a hover message and fix line, and the last valid parse stays drawn while the text is invalid
**And** Play is disabled while the score is invalid (gating from Story 6.2); Listen previews the last valid parse

**Given** the editor
**When** the user types
**Then** plain Enter is a newline, Space types a space (not preview), egui's built-in TextEdit undoer is disabled and text edits go into the app history from Story 9.1
**And** the editor `TextEdit` is named with `Response::accessible_name("Score text")`, since no visible label names it (Story 4.2: basic)

### Story 9.3: Select notes in the Roll

As the owner,
I want to select one or many notes with the mouse,
So that I can act on them.

**Acceptance Criteria:**

**Given** the Roll
**When** the user clicks a note, Shift+clicks, drags on empty canvas, Shift+drags, or clicks empty canvas
**Then** click replaces the selection, Shift+click toggles, drag draws the marquee (amber wash, dashed amber-ink) and selects every touched note, Shift+marquee adds, and an empty click clears (UX-DR25)
**And** selected notes have the amber stroke and glow; ghost notes in the out-of-range zones are selectable; Esc never clears the selection (it is STOP)

### Story 9.4: Draw and edit notes with the mouse

As the owner,
I want to add, move, resize and delete notes in the Roll,
So that I can fix a melody without typing (FR-17).

**Acceptance Criteria:**

**Given** a text score in the Roll
**When** the user double-clicks an empty spot
**Then** a note is added at that lane and time (frame-grid snap) with the last-used duration (one beat before any note has been placed or resized) (UX-DR26)

**Given** a selection
**When** the user drags a note body, drags a note's right edge (resize cursor on its last 6 px), or presses Delete/Backspace while the note view has focus
**Then** the selection moves (pitch snaps to the 19 lanes, time to the frame grid), the note resizes, or the selection is deleted

**Given** any such gesture
**When** it ends
**Then** only the affected note tokens are rewritten in the text score, the editor keeps its caret, editor and note view stay in sync, and the gesture is one undo step

### Story 9.5: Edit a MIDI song by converting it to a text score

As the owner,
I want my first edit on a `.mid` to turn it into a text score,
So that I can edit any MIDI without touching the original file (FR-18).

**Acceptance Criteria:**

**Given** a `.mid` loaded
**When** the user makes the first note-view edit
**Then** the selected track, with transpose applied, is converted to a text score, the split editor opens, the notice "Converted to text score – save to keep changes" shows, and the edit is applied (UX-DR30)
**And** the original `.mid` file is never modified
**And** undoing that first edit keeps the converted score

### Story 9.6: Context menu and per-note out-of-range actions

As the owner,
I want a right-click menu to move notes by an octave or delete them, also for out-of-range notes,
So that I decide note by note how the melody fits (FR-19).

**Acceptance Criteria:**

**Given** a right-click on a note (selected if not already) or on a selection, or the menu key / Shift+F10 on a selection
**When** the context menu opens
**Then** it lists Move up an octave · Move down an octave · separator · Delete, styled per `context-menu`; items that would leave B4–F6 plus the zones are disabled (UX-DR27)

**Given** an out-of-range (ghost) note
**When** the menu opens
**Then** it also offers "Reset to proposal", and the user can move it by an octave, delete it, or leave the proposal; dragging works too (UX-DR23)
**And** each action writes back to the text score (converting a `.mid` first, Story 9.5) and is one undo step; the fit summary updates

### Story 9.7: Staff view

As the owner,
I want a second view that shows which button each note uses,
So that I can read the fingering along the melody (FR-21).

**Acceptance Criteria:**

**Given** the Roll / Staff toggle (Staff now enabled)
**When** the user switches to Staff
**Then** the same lanes are drawn as staff rules (natural 1.2 px ink, accidental 0.5 px ink at 45 %, floored at 1 physical px), lane headers stay, each note is a 5 px bar starting with a 17 px staff glyph (button letter) followed by modifier tags (e.g. "D↑", "S↑") (UX-DR22)
**And** selection, editing, context menu and playhead work as in Roll

**Given** a switch between views
**When** it happens
**Then** scroll, zoom, cursor and selection are kept, and the chosen view persists across launches
**And** the Staff is one `Role::Canvas` AccessKit node with a summary name (view, note count, range), no node per note; the staff glyphs use the symbol helper's variant without a node (Story 4.3) (Story 4.2: basic)

## Epic 10: Pick up where I left off

The app reopens the last song with its settings, remembers per-song settings and recent files, saves scores, autosaves a draft and restores it after a crash.

### Story 10.1: Save scores and protect unsaved changes

As the owner,
I want to save my score and be warned before I lose changes,
So that my work is never thrown away by accident.

**Acceptance Criteria:**

**Given** UX Open Question 10
**When** this story starts
**Then** the text-score file extension and save-as defaults are decided and recorded in `EXPERIENCE.md` and `docs/score-format-and-mapping.md`

**Given** a text score with edits since the last save
**When** the window is drawn
**Then** the title shows a dot (UX-DR51)

**Given** Cmd/Ctrl+S
**When** pressed
**Then** an existing text score is saved in place; a new or converted score opens save-as (native dialog via `rfd`); the original `.mid` is never overwritten

**Given** unsaved changes
**When** the user closes the window or opens another song
**Then** the Unsaved changes dialog offers Save / Discard / Cancel

### Story 10.2: Draft autosave and crash restore

As the owner,
I want my edits autosaved as a draft and offered back after a crash,
So that a crash never costs me a melody.

**Acceptance Criteria:**

**Given** a song with unsaved edits
**When** the user edits
**Then** a draft is saved continuously to an internal app folder (OS data dir) without blocking the UI

**Given** the app crashed or was killed with a draft newer than its file
**When** the app launches
**Then** "Restore unsaved draft?" offers Restore / Discard; Restore loads the draft as unsaved changes; Discard deletes it (UX-DR51)

**Given** a normal save or Discard
**When** it completes
**Then** the draft for that song is removed

### Story 10.3: Reopen the last song and Recent files

As the owner,
I want the app to reopen what I was working on, with its settings,
So that I continue right where I stopped (FR-25).

**Acceptance Criteria:**

**Given** a song was open when the app closed
**When** the app launches
**Then** it reopens that song with its track, transpose, tempo and cursor (UX-DR50)

**Given** a file opened before
**When** it is reopened
**Then** its track, transpose, tempo and cursor are restored from per-song memory in the app settings file

**Given** the empty state
**When** it is drawn
**Then** a "Recent" group lists the last 5 songs as chips; clicking one opens it

**Given** the last song or a Recent entry no longer exists
**When** it is opened
**Then** the empty state shows with the banner "Couldn't open <file>: the file was moved or deleted." and the entry is removed from Recent

## Epic 11: Release 1.0 (M9)

Pre-built app for Windows, Linux and macOS plus both UF2s on GitHub Releases; README with the first-run unblock and wiring photo; licences and notices shipped; acceptance tests A.1–A.5 pass with a first-time user. No code signing (Q3).

### Story 11.1: Release workflow

As the owner,
I want a tag to build and publish the whole release,
So that every version ships the same way with matching app and firmware.

**Acceptance Criteria:**

**Given** a pushed tag `vX.Y.Z`
**When** `release.yml` runs
**Then** the firmware job builds `bridge.uf2` and `controller.uf2`, the app jobs build pre-built binaries for Windows, Linux and macOS (NFR-01, no toolchain for users) embedding those UF2s from the same run, and a GitHub Release is created with `bonooru-<ver>-<os>-<arch>.<ext>`, `bridge.uf2` and `controller.uf2` (AD-14)
**And** firmware and app report the same version from the tag

**Given** packaging
**When** artifacts are built
**Then** macOS is a universal unsigned `.app` (zipped), Windows an `.exe` (or installer), Linux an AppImage (plus a tarball if cheap), via `cargo-packager` 0.11.8 or a bundle script; Unix exec bits survive (tar, not upload-artifact zip)

**Given** the Linux artifact on a clean machine
**When** it starts
**Then** it runs with only `libudev.so.1` and `libasound.so.2` as system libraries, on X11/XWayland

### Story 11.2: Licences and third-party notices

As the owner,
I want every licence obligation met in the shipped app,
So that the project can be shared openly (NFR-07, AD-21).

**Acceptance Criteria:**

**Given** a release
**When** it is built
**Then** it ships the project MIT licence and a third-party notices file generated from the dependency graph, including the MPL-2.0 `serialport` notice with a source pointer and the OFL / Ubuntu-font / Hack licence texts
**And** `cargo-deny` passes on all three platforms' dependency graphs
**And** the About screen lists the same entries as the notices file

**Given** firmware
**When** it is released
**Then** attributions for reused MIT/BSD material (pico-sdk, TinyUSB, ws2812 example, any descriptor notes) are in `firmware/` and no 2wiCC code is present

### Story 11.3: README for first-time users

As a first-time user,
I want a README that gets me from download to the first song,
So that I can set up alone (NFR-10, A.5).

**Acceptance Criteria:**

**Given** the repo README
**When** a new user reads it
**Then** it covers: what you need (2 RP2040-Zero, 3 jumper wires, a data USB-C cable, a stock Switch 2), download, the first-run unblock per OS with OS dialog screenshots (macOS System Settings › Privacy & Security › Open Anyway; Windows SmartScreen More info › Run anyway), the wiring rules 1–4 with the owner's wiring photo (UX Open Question 12 closed), "don't dock mid-song", and that the app wizard does the rest
**And** it contains no Nintendo logos, game art or console/game screenshots (NFR-12)

**Given** the wiring photo
**When** it is taken
**Then** the same image is used in the wizard (Story 8.4) and the README

### Story 11.4: Content freeze

As the owner,
I want all on-screen copy, fingerings and examples final before release,
So that nothing in the app is a guess.

**Acceptance Criteria:**

**Given** UX Open Question 2
**When** the release candidate is checked in game
**Then** every fingering shown (lane headers, staff glyphs, gamepad captions) matches the verified chart from Epic 1

**Given** UX Open Question 13
**When** the owner reviews every "draft" string in the Voice table and State rows
**Then** each is approved or rewritten, and the `[ASSUMPTION]` tags that were confirmed during the build are removed from the spines

**Given** UX Open Question 11
**When** examples are reviewed
**Then** the 3–5 bundled examples are public domain, in range and play correctly

### Story 11.5: Acceptance run and v1.0.0

As the owner,
I want the acceptance tests passed and the release published,
So that version 1.0 is done (M9).

**Acceptance Criteria:**

**Given** the release candidate
**When** A.1 (reference piece, 0 wrong notes), A.2 (same on Windows, Linux, macOS), A.3 (unplug and wire-pull fail-safe) and A.4 (flash both boards on each OS) are run
**Then** all pass and are recorded in `docs/test-plan.md`

**Given** A.5
**When** a first-time user who is not the developer gets the release and the README only
**Then** they unblock the app, flash both boards, wire them, and play a song with the GUI only, with no other help; observations are recorded and blocking issues fixed before tagging

**Given** all acceptance tests pass
**When** the owner tags `v1.0.0`
**Then** the release is published and roadmap M9 is marked DONE

## Epic 12: Extras (optional, M10)

The user can play live from a MIDI keyboard and run a calibration wizard that writes measured timing into their profile; a single-board variant is explored. Any order, after M7. Live mode and calibration need UX design before their GUI stories start (out of the current UX pass).

### Story 12.1: Live MIDI latency spike

As the owner,
I want to measure key-to-sound latency from a MIDI keyboard,
So that I know whether live play is fun before building it (ADR-007).

**Acceptance Criteria:**

**Given** `ocb-dev` extended with a live mode using `midir` 0.11.1 and greedy fingering, sending `SET_STATE` in DIRECT mode (never the queue) (AD-15)
**When** the owner plays notes on a MIDI keyboard with the console in free play
**Then** key-to-sound latency is measured (recording of key click and console audio) and recorded, in frames
**And** 3 people play it and judge whether the latency is OK for fun use; the verdict is recorded in ADR-007

### Story 12.2: Live MIDI keyboard mode in the app

As the owner,
I want to play the ocarina live from my MIDI keyboard through the app,
So that I can improvise on the Switch (FR-13).

**Acceptance Criteria:**

**Given** a positive verdict from Story 12.1 and a UX design for live mode added to the spines
**When** the user picks a MIDI input and starts live mode
**Then** each key press maps to the profile fingering (greedy, with modifier lead) and is sent as `SET_STATE` through the serial worker; key release returns to neutral; PING keeps the session alive
**And** live mode obeys the same gating as the gamepad (3 LEDs ok, no required update) and STOP/Esc releases everything

### Story 12.3: Calibration wizard

As the owner,
I want guided tests that measure the timing values on my console,
So that my profile matches my console without manual spikes (FR-14).

**Acceptance Criteria:**

**Given** a UX design for calibration added to the spines
**When** the user runs the calibration wizard (min press, min gap same button, modifier lead sweeps as in Spike 2, confirmed by the user's ear or a recording)
**Then** the measured values plus margin are written to the user profile copy in the OS config dir through the `profile` module (AD-17), overriding the bundled default
**And** the user can reset to the bundled default from Settings

### Story 12.4: Single-board spike (Pico-PIO-USB)

As the owner,
I want to know if one Zero can do both jobs,
So that a simpler hardware variant can be considered later.

**Acceptance Criteria:**

**Given** Spike 3 in `docs/test-plan.md` on a spare Zero (Pico-PIO-USB `usb_device` example, D+/D− on two GPIOs, 1.5 kΩ pull-up on D+)
**When** it runs
**Then** the result (does the PC enumerate the PIO port while the native port drives the Switch 2, and at what CPU cost) is recorded
**And** this spike runs only on a spare board and never on the verified assembly (wiring rule 3); any pin used is outside the production `pins.h` rule and lives only in `spikes/`
