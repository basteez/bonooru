# Roadmap

Milestones in order; each depends on the previous one unless noted. Test IDs refer to `docs/test-plan.md`.

| M   | Title                                         | State | Done when |
|-----|-----------------------------------------------|-------|-----------|
| M0  | Switch 2 recognises a Zero and presses A      | DONE 2026-11-18 | The owner's Zero pair runs RiiDraw (P2) on both console USB-C ports. With P2 the console sees a real Pro Controller and passes normalised input to every game, so no separate OoT check; spike 1 runs inside OoT anyway. Spike 0 (P1) matrix optional since ADR-002. |
| M1  | Controller profile decided                    | DONE 2026-11-18 | ADR-002 (P2) and ADR-006 (console USB-C) accepted. |
| M2  | Ocarina mechanics verified                    | DONE 2026-11-21 | Spikes 1 and 2 done; ocarina mechanics have no HYPOTHESIS (unverified) marker left on pitches/modifiers; the profile has measured timings; timings are measured except `start_lead_ms` (not measured), labelled in the profile (`app/profiles/oot-switch2-1.1.1.toml`, Story 1.5); `max_sustain_ms` measured on A in Story 1.6 (6000 ms, other buttons assumed equal by owner decision). |
| M3  | Controller firmware                           | DONE 2026-11-23 | Own firmware enumerates with the chosen profile, accepts `SET_STATE` / `QUEUE_EVENT` / `START` / `STOP` over UART, plays a hard-coded test timeline with jitter ≤ 1 frame, watchdog goes neutral in ≤ 100 ms; abort latch (AD-8) and STATUS `mode` (AD-19) implemented. Epic 2 (Stories 2.1 to 2.7). Jitter: IOI p95 1.088 frames (18.14 ms), above one frame and EXPLAINED as Story 2.5 AC3 allows (firmware bound 8.01 ms, game floor 12.94 ms, residual +2.92 ms within the 4 ms allowance; test plan H.19). Watchdog: core 0 hang 20.0 ms, core 1 hang 80.0 ms (H.2a, H.2b). |
| M4  | Bridge firmware                               | TODO  | Frame-aware store-and-forward PC→UART, best-effort UART→PC, at 460800 with no byte loss over 10 min; heartbeat (`HEARTBEAT` `0x40`) every 20 ms by hardware alarm (AD-4); 1200-baud touch reboots into BOOTSEL; VID:PID (+ product string where reported) lets the app find it on all 3 OS (AD-13, Q1). Story 3.4 soak (H.26, 2026-11-24): 600 000 events over 10 minutes at 460800, 0 transport NAKs, no dropped byte (debug counters unchanged), so 921600 is not needed; cross-OS discovery is Epic 4's. |
| M5  | App core + GUI shell + virtual gamepad        | TODO  | Rust + egui/eframe app (ADR-008, accepted 2026-11-20); the GUI app starts on Windows, Linux and macOS, finds the bridge by itself, shows connection/version status, and its on-screen gamepad drives the console (mouse + keyboard). |
| M6  | Score pipeline in the GUI                     | TODO  | Text score and `.mid` open from the GUI, track/transpose can be chosen, the timeline is shown with fingerings and an audio preview; the reference piece plays with 0 wrong notes (A.1). |
| M7  | Timing validation phase (NFR-04)              | TODO  | T.1–T.5 pass on all 3 OS, or NFR-04 is revised with the measured numbers written in the requirements; profile timing margins updated from the results. |
| M8  | Flashing from the GUI                         | TODO  | A user with a fresh Zero flashes both roles from a guided GUI wizard on all 3 OS (A.4), no other tool installed. |
| M9  | Release 1.0                                   | TODO  | Pre-built GUI app for 3 OS + both UF2s; README with wiring photo; acceptance A.1–A.5 pass. |
| M10 | Extras (optional, any order after M7)         | TODO  | Per picked extra: live MIDI keyboard (ADR-007) — playable latency judged OK by 3 people; calibration wizard in the GUI (FR-14); single-board variant (spike 3). |

## Notes

- Risk R4 mitigation: a walking skeleton after M2 — the spike-2 script plays a whole melody through the 2wiCC queue; 3-OS hardware tests run as smoke tests on 2 of 3 OS (timing is host-independent).
- Release/CI (from repo README): one version `vX.Y.Z` for the whole repo; a release ships the app for 3 OS plus `bridge.uf2` and `controller.uf2` (bundled in the app for its flasher). CI (build + test) arrives with the first story that produces code; the release workflow lands with M8–M9.
