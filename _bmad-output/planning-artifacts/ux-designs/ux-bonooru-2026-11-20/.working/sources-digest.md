# Sources digest — bonooru app UX (2026-11-20)

Factual extract for the UX session. Every item cites its source. Abbreviations:
SPEC = `_bmad-output/specs/spec-bonooru/SPEC.md`; REQ = `requirements.md`; DEC = `decisions.md`;
ARCH = `architecture.md`; DIAG = `architecture-diagrams.md`; ROAD = `roadmap.md`; RISK = `risk-register.md`
(all in the spec folder); SPINE = `ARCHITECTURE-SPINE.md`; SCORE = `docs/score-format-and-mapping.md`;
LINK = `docs/link-protocol.md`; TEST = `docs/test-plan.md`; MECH = `docs/ocarina-input-mechanics.md`;
USB = `docs/usb-controller-emulation.md`; SPIKE = `spikes/app-egui/SPIKE-REPORT.md` (+ `spikes/app-egui/src/ui/*`).
No design decisions here; only what the sources say. Status at time of writing: M0, M1 DONE; M2–M10 TODO; no app code yet beyond the spike (SPEC Assumptions; ROAD).

---

## 1. Product in one paragraph

A fun, personal project that plays arbitrary melodies (MIDI or hand-written text scores) on Link's ocarina in *The Legend of Zelda: Ocarina of Time* (Switch 2 remake, 2026) by emulating a wired controller — not for game progress (SPEC Why; README). Chain: `PC (desktop app) ──USB──▶ bridge (RP2040-Zero) ──UART──▶ controller (RP2040-Zero) ──USB──▶ Switch 2` (README). The **bridge** (Zero #1, PC side) is a frame-aware USB-CDC↔UART pipe that injects a `HEARTBEAT` every 20 ms and reboots to BOOTSEL on a 1200-baud touch; it never decodes app frames (ARCH Components; SPINE AD-4). The **controller** (Zero #2, Switch side) emulates profile **P2, a wired Switch 1 Pro Controller (057E:2009)**, buffers a timeline and plays it from its own 1 MHz clock (DEC ADR-002, ADR-003; ARCH). The **Switch 2 sees a wired Pro Controller**; the user must enable "Pro Controller Wired Communication" (DEC ADR-002). The controller plugs into a console USB-C port, top preferred (DEC ADR-006). Boards are wired GP0/GP1 crossed + GND only, 3 jumpers (ARCH Hardware). The user imports a score, the app reduces it to one line, fits it to the ocarina range (B3–F5, 19 pitches), picks fingerings, compiles a µs timeline, and streams it; the user plays it in the game's **free play** ocarina mode, which they reach manually (via the on-screen gamepad) (SPEC CAP-1..CAP-4, CAP-8; MECH; ARCH Glossary). Non-goals: official songs/progress automation, online/multiplayer, modding, Bluetooth, polyphony, mic input, MusicXML/ABC (SPEC Non-goals).

## 2. Users & context

- Audience: "the owner and first-time users with two Zeros, three jumper wires and a stock Switch 2" (SPEC Why). Personal project, not distributed as a product (REQ Open points, NFR-10 vs R5).
- Skill assumption: first-time user, not the developer; GUI only, no command line, follows on-screen steps (FR-15, NFR-10; SPEC CAP-10). Inspired by / "like RiiDraw" (REQ Open points "Host UI").
- **A.5**: "First-time user (not the developer) unblocks the unsigned app with the README, then flashes and plays a song with the GUI only, no other help" — related FR-15, NFR-10 (TEST Acceptance). SPEC CAP-10 success restates it; A.5 runs on all 3 OS with the README (SPINE Q3).
- **Unsigned app**: no code signing/notarisation (SPINE Q3; RISK R5, accepted 2026-11-19). README documents the first-run unblock with screenshots: macOS System Settings → Privacy & Security → Open Anyway; Windows SmartScreen: More info → Run anyway; "right-click → Open" no longer works on macOS 15+ (REQ Open points; RISK R5). The unblock happens **outside** the app (README), before the GUI is reached.
- Platforms: Windows, Linux, macOS, pre-built, no toolchain (NFR-01). Packaging: macOS universal `.app`; Linux AppImage (+ tarball if cheap) (SPINE Q6). Release assets `bonooru-<ver>-<os>-<arch>.<ext>` + `bridge.uf2` + `controller.uf2`; UF2s embedded in the app (SPINE Structural Seed).
- Physical context: controller board sits on the Switch (can only be flashed when moved to the PC) (DEC ADR-005 Consequences). Handheld/tabletop or dock; "don't dock mid-song" (DEC ADR-006; RISK R13). Switch on dock/stand, short cable Switch side, slack PC side (TEST Hardware safety; RISK C2).
- Offline only, single player (NFR-06); app opens no network connections — no update check, no telemetry (SPINE AD-21).

## 3. Requirements relevant to the app

All verbatim IDs from REQ; MoSCoW in brackets.

- **FR-01** [M] Import `.mid`; choose track/channel. CAP-1: both formats open from the GUI and yield the same `(pitch, start, duration)` list; track/channel selectable (M6).
- **FR-02** [M] Import hand-written text score (custom format) (see §6).
- **FR-03** [M] Reduce polyphony to one line (chords → one note). Rule: skyline = highest note; overlaps cut the earlier note (SCORE Steps).
- **FR-04** [M] Fit into playable range: auto transpose + manual override; "report what doesn't fit". CAP-2: chosen transposition minimises out-of-range notes; every note still not fitting is reported to the user. RISK R11 mitigation: "fit report in the GUI; curated example songs".
- **FR-05** [M] Pick an input combination (button + modifiers) for every pitch. CAP-3: reference piece (all 19 pitches) compiles with a fingering for every note.
- **FR-06** [M] Mapping and timing parameters in an editable profile, not hard-coded. CAP-3: changing a value in the profile file changes the compiled timeline, no code change.
- **FR-07** [M] Playback: start (with countdown), stop, tempo scale; stop = release every input. CAP-4 success: T.1–T.5 pass on 3 OS (or NFR-04 revised, M7); A.1 0 wrong notes.
- **FR-08** [M] Detect the bridge automatically, no manual serial-port choice. CAP-6: on Win/Linux/macOS the app shows connection and version status with no port selection (M5).
- **FR-09** [M] Flash firmware on both Zeros from the app (bridge and controller role). CAP-7 / A.4: a fresh Zero flashed in both roles from a guided GUI wizard on each OS, no other tool (M8).
- **FR-10** [S] Check firmware/protocol version and offer an update on mismatch. CAP-6: mismatched firmware triggers an update offer. Rules in SPINE AD-13 (§5).
- **FR-11** [S] Virtual gamepad: manual input to reach ocarina free play. CAP-8: mouse + keyboard input moves through the game into free play (M5).
- **FR-12** [S] Dry run: show the input timeline and play a local audio preview. CAP-9: timeline with fingerings and audio preview shown before playing (M6).
- **FR-13** [C] Live play from a MIDI keyboard — after v1, M10 extra; latency ≈ 3–6 frames [HYPOTHESIS]; "playable latency judged OK by 3 people" (SPEC CAP-11; SCORE Live MIDI; DEC ADR-007).
- **FR-14** [C] Calibration: guided tests measuring timing on the user's console; wizard writes measured values into the user's profile (SPEC CAP-12; M10).
- **FR-15** [M] Graphical UI for every user function (setup/flash, song import, preview, play, gamepad); no command line.
- **NFR-01** [M] Runs on Windows, Linux, macOS; pre-built binaries.
- **NFR-04** [M] Inter-onset interval error vs score: p95 ≤ 1 game frame, max ≤ 1.5 frames; drift < 1 frame over 5 min; validated in M7 (restated 2026-11-19). 1 frame = 16.7 ms at 60 fps (DEC ADR-003). Game frame rate (30/60) unverified (U10).
- **NFR-05** [M] Link drop → controller neutral within 100 ms (A.3). Firmware-side, but the app sees `EVENT LINK_LOST` (§8).
- **NFR-06** [M] Offline only, single player only.
- **NFR-08** [S] Wrong-note rate 0 on the reference piece once calibrated.
- **NFR-09** [S] Firmware upgradable by users with BOOT button + app only.
- **NFR-10** [M] Ease of use: first-time user, following the README for the one-time OS unblock, then flashes both boards and plays a song with the GUI only, following on-screen steps (restated 2026-11-19).
- Constraints touching UI: GUI only, no user CLI (SPEC Constraints); queue mode never emits Home / Capture / + / − (C6; SPINE AD-9).
- Success signal: Release 1.0 (M9) — first-time user wires two Zeros, flashes both from the GUI, plays the 1-minute reference piece (all 19 pitches) with 0 wrong notes; A.1–A.5 pass (SPEC Success signal).
- Milestones that ship UI: M5 (GUI shell, discovery, connection/version status, gamepad), M6 (score pipeline in GUI: open text + `.mid`, choose track/transpose, timeline with fingerings, audio preview), M8 (flashing wizard), M9 (release + README with wiring photo), M10 (live MIDI, calibration wizard) (ROAD).

## 4. ADR-008 screen-flow idea and other UI-touching ADRs

**ADR-008** (DEC; accepted 2026-11-20; = SPINE AD-18): app is Rust + egui/eframe. Context lists UI needs verbatim-ish: "GUI only, easy for first-time users (FR-15, NFR-10); 3 OS, pre-built (NFR-01). Needs native file dialogs, timeline/piano-roll view, audio preview, step-by-step flashing wizard, on-screen gamepad, background serial I/O without freezing the UI. Screen flow: `architecture-diagrams.md`."

**Screen flow, "first idea"** (DIAG "GUI screens (first idea, Rust + egui per ADR-008)"), transcribed:
- `Start` → decision `Bridge found? Firmware OK?`
- **no** → `Setup wizard`: "BOOT + plug, flash bridge, flash controller, wiring photo" → back to the check.
- **yes** → `Song`: "open .mid / text, pick track, transpose, piano roll + preview" → `Play`: "countdown, progress, STOP".
- **yes** → `Gamepad`: "reach free play" → `Play`.
- `Song` -.-> `Settings`: "profile, timing, advanced" (dotted edge).

UI module list in the structural seed: `ui/  # egui: setup wizard, song, play, gamepad, settings, advanced` (SPINE Structural Seed).

Other ADR/AD items affecting UI:
- **ADR-008 Consequences / AD-18**: macOS shows and uses only `/dev/cu.*`; audio open failure → "clear error with Retry", re-open on default-device change, "no silent fallback". Not yet verified: GUI on Windows/Linux.
- **ADR-002**: "the GUI setup wizard shows this step [Pro Controller Wired Communication] with a screenshot (FR-15, NFR-10)". Pairing reset via "Disconnect controllers" (RISK C7).
- **ADR-005**: flasher flow (§5); Linux: `RPI-RP2` may need auto-mount (udisks) or manual mount → "detect and explain"; serial needs `dialout` group → "check and explain".
- **ADR-006**: "document 'don't dock mid-song'".
- **ADR-007**: `.mid` + text in v1; MusicXML/ABC not planned, "users export to MIDI".
- **ADR-003**: app never times notes; tempo scale = recompile (SPINE AD-15).
- **SPINE Consistency Conventions — "UI safety content"** (verbatim): "the setup wizard shows wiring rules 1–4 (risk register), the 'Pro Controller Wired Communication' step and a wiring photo; the play screen says 'don't dock mid-song'; Linux `dialout` and udisks problems are detected by adapters and explained, never fixed by escalating privileges".
- **SPINE Conventions — Logging**: rotating log in OS log dir, every EVENT/NAK logged; "a hidden 'advanced' panel shows live frames". LINK Framing: "hidden 'advanced' panel in the GUI that shows and sends frames for debugging".
- **SPINE Conventions — Errors**: core returns typed results; services turn them into user-facing events.
- **AD-16**: UI thread never blocks on I/O; UI ↔ serial worker by messages only.

## 5. Device / state model

**Discovery** (SPINE AD-13; RISK R8; DEC ADR-005):
- Bridge USB identity: VID:PID generic Raspberry Pi `2E8A:000A` (Q1, "for now"); product string `Bonooru Bridge`; serial = flash unique ID.
- Match VID:PID first; product string as extra filter where the OS reports it; `HELLO` probe only to VID:PID matches.
- Windows: `serialport` gets no product string; Windows usually shows only "USB Serial Device" (RISK R8).
- Other Pico boards share the VID:PID (e.g. RiiDraw firmware listed as `2e8a:000a`, product "RiiDraw" — SPIKE owner test 1). 1200-baud touch only to a port **confirmed** as our bridge: product string, "or an explicit pick by the user in the setup wizard when the OS doesn't report the string" (AD-13).
- macOS: each device listed as `/dev/cu.*` and `/dev/tty.*`; show/open only `cu.*` (AD-18).
- Spike: worker auto-rescans every 1 s; hot-plug picked up (SPIKE owner test 1b).

**Session / what the app can know and when** (SPINE AD-7, AD-19; LINK):
- On open: app asserts DTR, writes `0x00`, sends `HELLO` every 100 ms until `INFO`. HELLO has STOP semantics (a restarted app never inherits a running piece).
- `INFO v1`: `proto_ver, fw_ver[3], bridge_ver[3]` (from last HEARTBEAT; **zeros if none**), `hid_profile, queue_cap` (default 512), `app_timeout_ms` (default 1000), `report_period_us` (median IN interval, 0 if unknown).
- So: bridge-only (no controller wired / powered) → **no INFO**, the app cannot read the bridge version over the link (bridge version reaches the app only via `INFO.bridge_ver`, AD-13). With bridge found but no answer, the app knows only the USB-level identity.
- `STATUS` every 50 ms (+ after ACKs, ≤ 1 per 10 ms): `free, now_us` (µs since t0; 0 when IDLE or during lead), `last_seq, mode, abort_count, last_event, link_ok`. App derives mode from STATUS; EVENT is a fast path (AD-19).
- Console handshake readiness: until the P2 profile completes the handshake, `START`/`SET_STATE` get `NAK NOT_READY` (AD-8; USB doc). This is how "controller not yet accepted by Switch" surfaces (e.g. Wired Communication OFF is a plausible cause — not stated explicitly in sources).
- Link up/down: `link_ok` in STATUS; `EVENT LINK_LOST`; `NAK NO_LINK` on START/SET_STATE while down (AD-5).

**Controller modes** (SPINE AD-8; LINK Controller modes; state diagram in SPINE):
- IDLE → DIRECT on `SET_STATE`; IDLE → PLAYING on `START`; DIRECT → PLAYING on `START` (real-time state → NEUTRAL first).
- DIRECT → IDLE: STOP, HELLO, app timeout, link loss, USB event. PLAYING → IDLE: STOP, HELLO, END, underrun, LATE, link loss, USB event.
- In PLAYING, `SET_STATE` and `START` get `NAK BUSY` → gamepad cannot act during playback.
- **Abort latch**: any IDLE transition the app didn't request (END applied, underrun, LATE, link loss, app timeout, USB suspend/unmount/bus reset, core stall) sets `aborted`; QUEUE_EVENT/START/SET_STATE get `NAK ABORTED` until `STOP` or `HELLO`.
- DIRECT mode: app sends `PING` when idle for `app_timeout_ms/4`; DTR falling edge also counts as app timeout (AD-5, AD-7).
- Safety mask in PLAYING: only L, R, A, X, Y, hat, left stick; DIRECT unmasked (AD-9).

**Update-offer rules** (SPINE AD-13 "Update offer", verbatim): "With no controller wired, the setup wizard always offers to reflash the bridge. If the protocol versions don't match, the update is required. If `fw_ver` or `bridge_ver` ≠ the app version, the update is recommended." App flashes only the UF2s bundled at its own version (AD-14). `NAK VERSION` mid-session = controller rebooted → app re-sends HELLO (AD-7).

**Flasher flow** (DEC ADR-005 "Flashing, RiiDraw style"; SPINE AD-16):
1. User holds BOOT and plugs a Zero into the PC.
2. App finds the `RPI-RP2` drive (all mounted volumes, label + `INFO_UF2.TXT`). Flasher worker snapshots volumes before the touch / BOOTSEL prompt and accepts exactly one new volume containing `INFO_UF2.TXT`, checks UF2 family is RP2040.
3. User picks role (bridge / controller); app copies the UF2, waits for the drive to vanish.
4. App verifies: bridge via USB product string; controller via HELLO through the bridge once wired.
- Bridge update without BOOT: 1200-baud touch → BOOTSEL → same copy (only to a confirmed bridge, AD-13). The touch and post-flash verification are serial-worker commands (AD-16).
- Controller can only be flashed when moved from the Switch to the PC (ADR-005). Flashing is "one board at a time" (DIAG System).
- picotool not required; fallback PICOBOOT only if M8 shows mass-storage unreliable (ADR-005; SPINE Deferred; RISK R9).
- Linux: auto-mount (udisks) / manual mount and `dialout` group → detect and explain (ADR-005; SPINE Conventions).
- Spike flasher states: Copied → WaitingForReboot → Done (volume gone after ~310 ms); progress bar; log ending "flash OK" (SPIKE owner test 4).

**BOOT procedure**: hold BOOT while plugging USB (ADR-005 step 1; SPIKE test 4b: "Zero plugged with BOOT held: `/Volumes/RPI-RP2` detected, label 'RPI-RP2', model 'Raspberry Pi RP2'"). NFR-09: upgrade with BOOT + app only.

**Wiring rules** (RISK "Wiring rules (README + GUI setup wizard)"): 1. connect only GP0, GP1, GND — never 5V, never 3V3; 2. rewire only with both USB cables unplugged; 3. keep the verified assembly as is, experiments on spare Zeros; 4. breadboard: nothing on the rows of 5V/3V3 pins, don't use power rails. Pinout: bridge GP0(TX)→controller GP1(RX), bridge GP1(RX)←controller GP0(TX), GND–GND (ARCH). Header order GP29, 3V3, GND, 5V: GND next to 5V, off-by-one puts it on 5V (RISK Facts). Method A (3 F-F jumpers) or method B (RiiDraw adapter board) (ADR-001). RISK Next action 6: "Write the wiring rules into the README and the setup wizard (C1)" — open.

**Switch setting**: "Pro Controller Wired Communication" ON, System Settings → Controllers & Accessories (ADR-002; USB doc table P2). Wizard shows it with a screenshot.

**Port**: console USB-C, top preferred so bottom stays free for charging; in dock bottom is taken → top or dock USB-A (dock unverified) (ADR-006).

## 6. Score / music facts

- **Internal model**: list of `(midi_pitch, start_s, duration_s)` (SCORE); integer µs downstream of import (SPINE Conventions).
- **Pipeline** (SCORE): `.mid`/text → note list → mono reduction (track/channel, skyline) → range fit (transpose) → fingering (min-cost path) → timing rules → timeline `(t_us, state)`; mapping profile feeds fit, fingering, timing.
- **MIDI import**: user picks track/channel (FR-01; SCORE Steps). Spike: `.mid` via `midly` with tempo map and skyline reduction (SPIKE criterion 2).
- **Text score (draft)** (SCORE): header comments `# title: …`; `tempo 120` (quarter notes per minute, changeable anywhere); `transpose 0`; notes `<name><accidental?><octave>/<denominator>[.]` with `#`/`b`; rests `r/4`; `|` bar lines optional and only checked; raw ms durations `E4:250`. Grammar details deferred, owned by `score` (SPINE Deferred).
- **Range**: B3–F5, chromatic, 19 pitches, 45 combinations (MECH, HYPOTHESIS). Profile `[range] low = 59 (B3), high = 77 (F5)` (SCORE).
- **Buttons/pitches** [HYPOTHESIS] (MECH): L=D4, R=F4, A=A4, Y=B4, X=D5; modifiers D-pad ↑/↓ ±1, left stick ↑/↓ ±2, both up to ±3; hold modifier before button; bends only in free play; songs recognised from the 5 natural notes only.
- **Transpose / range fit** (SCORE Steps): try every shift −24…+24 semitones; keep the fewest out-of-range (tie: smallest shift, then most natural notes); left-overs → **octave-fold or drop, always reported**. Manual override (FR-04). M6: "track/transpose can be chosen" (ROAD).
- **Official-song detection** (R2; SPINE AD-15): pipeline detects sequences of the 12 official songs → alternative fingering, else a warning. Risk: in-game effect ends free play mid-piece.
- **Fingering**: lowest total cost path (Viterbi/DP); cost weights in profile (SCORE).
- **Timing rules** (SCORE; AD-15): snap to the profile frame grid; min press, min gap on same button, modifier lead; steal time from previous note, never shift onsets. Validation rejects min press/gap below `2 × max(report_period_us, frame time)`. Profile defaults (to be measured): `min_press = 50`, `min_gap_same_button = 34`, `modifier_lead = 17`, `start_lead = 500` ms; `frame_rate = 60` (U10).
- **Unknowns affecting what's playable** (MECH U1–U11): sustain/max length (U1), retrigger gap (U2), legato (U3), bend glide (U4), free-play limits incl. note count/auto-exit/timeouts (U8), how to enter/leave free play (U9).
- **Timeline limits**: `t_us` u32 strictly increasing, < 2^31 (≈ 35.8 min — derived, not stated). Last event `NEUTRAL|END` (AD-8). Test lengths: 1-min reference piece, 5-min piece (TEST).
- **Tempo scale**: recompiling; firmware has no notion of tempo (AD-15).
- **Countdown**: "the UI plus `START lead_us`" (AD-15); playback clock starts at now + lead (LINK START). Profile `start_lead = 500` ms (SCORE).
- **Buffering**: queue cap from INFO (default 512 events); app prefills, sends START, then greedy as credits allow; never edits/drops/reorders (LINK Flow control; AD-15).
- **Underrun**: judged at apply time; non-END event applied with queue then empty → NEUTRAL same pass + `EVENT UNDERRUN` ("a stuck note is worse than a gap"); sets abort latch (AD-8; LINK).
- **LATE**: head event overdue > 1 report period → abort, `EVENT LATE`, no backlog fired (AD-8).
- **STOP semantics**: clear queue, go neutral, → IDLE, clear abort latch; neutral within 100 ms (LINK Messages, Watchdog). FR-07 "stop = release every input". After a semantic NAK on QUEUE_EVENT/START the app abandons the piece and sends STOP (AD-6).
- **End**: `EVENT PLAYBACK_DONE` when END is applied (AD-8).
- **Progress**: STATUS `now_us` = µs since t0 (0 during lead / IDLE) (AD-19) — available to drive a progress display.
- **Export**: timeline → CSV (`t_us`, pitch, fingering), M7 ground truth (AD-15). Whether user-facing is not stated.
- **Audio preview**: core renders into a buffer, audio adapter plays (AD-16). Spike: sine or square, 4 ms ramp, playhead driven by audio sample counter (SPIKE criterion 3).

## 7. Virtual gamepad

- FR-11 [S]: manual input to reach ocarina free play; CAP-8: mouse + keyboard on the on-screen gamepad moves through the game into free play (M5).
- Path: ui → worker → `SET_STATE` in DIRECT mode, not masked (all buttons possible incl. Home/Capture/+/−, AD-9 applies only to PLAYING) (SPINE Capability map; AD-8; AD-9).
- Canonical state buttons: Y, B, A, X, L, R, ZL, ZR, −, +, L3, R3, Home, Capture; hat 0–7 / 8 neutral; left + right stick (AD-10). **Which subset the gamepad exposes is not specified** in the spec.
- Spike stub: 10 buttons (ABXY, L, R, D-pad); keys Q/E = L/R, J/K/I/U = A/B/X/Y, arrows = D-pad; key auto-repeat ignored; mouse and key tracked separately; release-all on window focus loss; key-ups read even on other tabs so nothing sticks (SPIKE Gamepad stub; `ui/gamepad.rs`).
- Safety: DIRECT app timeout `app_timeout_ms` (default 1000 until W.1) + DTR drop → neutral; app PINGs when idle (AD-5, AD-7).
- Blocked during PLAYING (`NAK BUSY`); `NAK NOT_READY` before console handshake; `NAK NO_LINK` while link is down (AD-8, AD-5).
- Latency: no numeric requirement for the gamepad. Only reference: live MIDI key→sound ≈ 3–6 frames [HYPOTHESIS] (SCORE Live MIDI). Spike: HELLO poll ~1–8 ms host poll (LINK sequence).
- Live MIDI (M10) reuses `SET_STATE` in DIRECT (AD-15).

## 8. Error conditions in the sources

| Condition | Detection | What sources say the app does |
|---|---|---|
| Bridge not found | no VID:PID match during discovery | Flow goes to Setup wizard (DIAG). CAP-6: show connection status. |
| Other Pico board on `2E8A:000A` (e.g. RiiDraw) | VID:PID match, product string ≠ `Bonooru Bridge` / no INFO | Never 1200-baud-touch unconfirmed ports; user may pick explicitly in wizard when OS lacks product string (AD-13; SPIKE test 1). |
| Windows: no product string | `serialport` limitation | VID:PID + HELLO probe; explicit user pick before touch (R8; AD-13). |
| Linux: no serial permission | adapter detects (`dialout`) | "check and explain", never escalate privileges (ADR-005; Conventions). |
| Linux: RPI-RP2 not mounted | adapter detects (udisks) | "detect and explain" (ADR-005; Conventions). |
| Wrong protocol version | `INFO.proto_ver` ≠ app | Update **required** (AD-13); other frames get `NAK VERSION` (AD-7). |
| Firmware version ≠ app version | `fw_ver` / `bridge_ver` | Update **recommended** (AD-13). |
| Controller not wired / not powered | bridge answers no INFO (HELLO unanswered) | Wizard "always offers to reflash the bridge" (AD-13). Explicit UI message not specified. |
| Controller rebooted mid-session | `NAK VERSION` mid-session | Re-send HELLO (AD-7). |
| Console handshake not done (e.g. Wired Comm. setting) | `NAK NOT_READY` on START/SET_STATE | Not specified beyond the NAK; wizard shows the setting step (ADR-002). |
| Link lost (bridge unplugged, UART wire pulled) | `EVENT LINK_LOST`, `link_ok`, `NAK NO_LINK` | Controller goes neutral ≤ 100 ms (NFR-05, A.3); app-side UI not specified. |
| App frozen in DIRECT | controller app timeout | Controller → IDLE neutral; latch set (AD-5). |
| Underrun | `EVENT UNDERRUN` | Controller neutral; latch until STOP/HELLO (AD-8). App UI not specified. |
| LATE | `EVENT LATE` | Abort, no backlog (AD-8). |
| USB suspend / unmount / bus reset (e.g. docking) | `EVENT USB_SUSPENDED` etc. | Abort to IDLE; "don't dock mid-song" on play screen (AD-8; Conventions; R13). |
| Semantic NAK on QUEUE_EVENT / START (BUSY, ORDER, BAD_STATE, ABORTED …) | NAK | App abandons the piece, sends STOP (AD-6). |
| Unknown EVENT / NAK codes | — | Logged, never fatal (AD-19). |
| Audio output open fails (Bluetooth switch) | adapter error | Clear error + Retry; re-open on default-device change; no silent fallback (AD-18; SPIKE). |
| Flash: no / multiple new volumes, wrong UF2 family | flasher worker snapshot diff | Accepts exactly one new volume with `INFO_UF2.TXT`, RP2040 family (AD-16). UI text not specified. |
| Flash: drive doesn't vanish | timeout (spike) | Spike has a timeout case (SPIKE criterion 4). |
| Flash verify fails | bridge product string / controller HELLO | Step 4 of ADR-005; outcome UI not specified. |
| macOS xattr copy failure on RPI-RP2 | — | Implementation: plain read/write loop, not `fs::copy` (SPIKE). |
| Unmigratable user profile | profile module | "refused with a message" (AD-17). |
| Out-of-range notes | range fit | Octave-fold or drop, always reported (SCORE; FR-04). |
| Official song in melody | pipeline R2 | Alternative fingering, else warning (AD-15). |
| Timing below minimums | validation | Rejects min press/gap below 2 × max(report period, frame) (AD-15). |
| Unsigned app blocked by OS | before launch | README unblock; antivirus false positives possible (R5). |

## 9. Settings / profile

- **Mapping profile**: TOML, integer `schema` field; one bundled read-only default per game version; user copies (incl. calibration output) in the OS config dir override the bundled default with same name + game version; unmigratable copy refused with a message (SPINE AD-17). Sections: `[profile] name, frame_rate`, `[buttons]`, `[modifiers]` (incl. `stick_value`), `[timing] min_press, min_gap_same_button, modifier_lead, start_lead`, `[range] low, high` (SCORE). Fingering cost weights also in profile (SCORE).
- **App settings**: "live in a separate file" (AD-17). Contents not enumerated.
- **Settings screen** (DIAG): "profile, timing, advanced", reached from Song.
- **Advanced**: hidden panel showing live frames / sending frames for debugging (SPINE Conventions; LINK Framing).
- **Log**: rotating log in OS log dir (Conventions).
- **Calibration** (FR-14, M10): writes measured timing into the user's profile.
- **Runtime values from INFO**, never hard-coded: queue_cap, app_timeout_ms, report_period_us, hid_profile (AD-7).
- Not stated anywhere: persistence of last song, recent files, window layout, preferred transpose/tempo per song, chosen port.

## 10. egui spike findings

- Verdict: viable, no exit criterion failed; ADR-008 accepted (SPIKE Verdict; SPINE Q7). Spike author wrote it "without seeing the window"; owner hand test on macOS arm64 passed 2026-11-20 (SPIKE).
- Layout: top panel with `selectable_value` tabs "1 · Devices", "2-3 · Piano roll + audio", "4 · UF2 flasher", "Gamepad stub", plus monospace frame stats right-aligned; `CentralPanel` content (`ui/mod.rs`). Spike tab layout is a test harness, not a design.
- Widgets used: `heading`, `label`, `colored_label`, `button`, `text_edit_singleline`/`TextEdit`, `Grid` (ports table), `selectable_value` (tabs, sine/square), `checkbox` ("follow playhead"), `Slider` (zoom px/s 1–1200), `ProgressBar::show_percentage` (flash), custom `Painter` piano roll (`ui/*.rs`).
- Devices tab: product filter text box, "Rescan now", auto-rescan every 1 s; colour classification green = product matches filter, yellow = RP2040 VID `0x2E8A`; columns port, VID:PID, manufacturer, product, serial; "Open (read-only)" at 115200 (`ui/devices.rs`).
- Piano roll: 2,000 notes; drag/scroll pans; Ctrl/Cmd+scroll or pinch zooms around cursor; click seeks; follow playhead; black-key rows shaded, naturals labelled; ▶ Play (audio) / ■ Stop / ⏮; `.mid` loaded from a typed path (no file dialog in spike); "t = x.xx s" readout (`ui/piano.rs`; SPIKE).
- Performance: frame CPU p50 1.2 / p95 1.5 / max 1.8 ms with ~1,987 notes on screen; 120 Hz vsync held; idle CPU 0.2 % (egui repaints only on change); 18 % of a core when animating at 120 Hz; RSS 82 MB idle; cold start ~115 ms (530 ms first launch of new binary); first frame outlier 60–266 ms (wgpu start-up). Binary 10.9 MB / 4.8 MB zipped (SPIKE Numbers).
- Audio: playhead from audio thread sample counter, no drift; audio 9–20 ms behind wall clock; output latency (~10–20 ms) not compensated; owner judged playhead/audio offset "very small by ear" (SPIKE criterion 3, owner test 3).
- Gotchas: audio open failed first time on AirPods ("Error opening the stream with the OS"), worked on Retry → Retry + re-open on default-device change, no silent fallback; macOS duplicates `cu.*`/`tty.*` → show only `cu.*` (SPIKE Findings). Immediate-mode borrow pattern: collect an action, apply after the closure (SPIKE Pain points 2). egui 0.37 API churn (`App::ui`, `Panel::top`) — pin exact versions (SPIKE Pain points 1).
- Flasher sim: real RPI-RP2 volumes listed but not selectable in spike; safety banner in orange (`ui/flasher.rs`).
- Gamepad: release-on-focus-loss, auto-repeat ignored (owner test: 1 press, 1 release, 101 repeats ignored).
- **Not tested**: GUI on Windows/Linux (HiDPI, Wayland vs X11, Windows COM names, `dialout`), "what the window looks like, and how the UX feels: zoom, pan, button feel and audio quality" (SPIKE Not tested). Real flashing never done; real serial read from bridge not done.
- Fonts: egui default fonts OFL-1.1 / Ubuntu-font-1.0 — ship licence texts or replace the fonts (AD-18; R12).
- Third-party notices file ships with releases (AD-21) — an About/licences view is not specified.

## 11. Open questions / ambiguities left for UX

1. Is the app a linear wizard, a tabbed/sidebar app, or a hybrid? DIAG is a "first idea" only.
2. How does the user move from Setup wizard to Song/Gamepad/Play, and can they revisit setup (e.g. reflash) later from the main UI?
3. Order of Gamepad vs Song: DIAG has both branching from the check and both leading to Play — is gamepad a separate screen, a panel beside Play, or available during Song?
4. How is the "bridge found, controller not wired / no INFO" state shown, given the app cannot then read `bridge_ver`?
5. How is the explicit user port pick (AD-13, Windows without product string) presented without breaking FR-08 "no manual serial-port choice"?
6. How to present "required" vs "recommended" updates, and can a recommended update be skipped/deferred?
7. Wizard ordering: flash bridge first or controller first; wiring step before or after flashing; where the Wired Communication screenshot step and wiring rules 1–4 sit.
8. How does the wizard tell the user to move the controller board from the Switch to the PC (and back) for flashing?
9. How is `NAK NOT_READY` (console handshake not complete) surfaced — as a hint to enable Wired Communication?
10. What does the user see on underrun, LATE, link loss, USB suspend during Play, and how does recovery (STOP/HELLO clears latch) map to a button?
11. Countdown: length (profile `start_lead` 500 ms vs a visible user countdown), visual form, and whether it is user-configurable.
12. Progress during Play: what is shown (time, notes, piano-roll playhead driven by `now_us`)?
13. Tempo scale: range, control type, and whether recompile is instantaneous/visible.
14. Track/channel picker for `.mid`: how tracks are described (names, note counts, preview).
15. Transpose: how auto result and manual override are shown; how the fit report (folded/dropped notes) is displayed, per note or summary.
16. Official-song detection warning: where and how it is shown and whether the user can override.
17. Timing-validation rejection (notes too fast): how reported and whether the user can do anything.
18. Text score: is there an in-app editor, or only file open? (Only "open" is specified.)
19. Does the piano roll show fingerings (button + modifiers) inline, and in what notation?
20. Audio preview controls: waveform choice, volume, seek; sync with the piano roll.
21. Virtual gamepad: which buttons are exposed (spike: 10; DIRECT allows all incl. Home/+/−/sticks), keyboard mapping, and whether it can be used while a song is loaded.
22. Gamepad safety: should Home/Capture/+/− be exposed given C6 (queue mode only) and that DIRECT is unmasked?
23. Settings: which profile/timing fields are editable in-GUI vs only by file (FR-06 says "editable profile"; CAP-3 says "profile file")? How are user copies vs bundled defaults shown?
24. App settings file: what it contains (not enumerated).
25. Advanced panel: how hidden/revealed; what it shows; who it is for.
26. Is the CSV timeline export a user-facing feature?
27. Linux `dialout` / udisks explanations: wording and placement.
28. Where "don't dock mid-song" appears on the Play screen, and whether other safety reminders (cable, stand) appear.
29. Is there an About / third-party licences view (fonts OFL/Ubuntu, MPL-2.0 serialport notice)?
30. Window sizing, HiDPI, light/dark theme, and fonts (default egui fonts or replacements) — unspecified and untested on Windows/Linux.
31. Bundled curated example songs (R11 mitigation): are they in the app, and how surfaced?
32. Persistence: last opened song, chosen track/transpose/tempo per song, recent files — not specified.
33. Free-play navigation: sources leave U9 (how to enter/leave free play) unknown; does the app give any guidance text for reaching free play?
34. Calibration wizard (M10) and live MIDI (M10): should v1 UX reserve a place for them?
35. Wording/terminology: "bridge" / "controller" vs user-friendly names; "app" vs "host".
