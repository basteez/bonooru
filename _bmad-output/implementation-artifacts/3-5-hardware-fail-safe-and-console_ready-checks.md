---
baseline_commit: d631a08
---

# Story 3.5: Hardware fail-safe and `console_ready` checks

Status: review

<!-- Ultimate context engine analysis completed - comprehensive developer guide created (gds-create-story, 2026-11-24) -->

## Story

As the owner,
I want the fail-safe and console-state behaviour measured on my real hardware,
so that I can trust the boards with my console and settle the open timing questions.

## Acceptance Criteria

1. **A.3 fail-safe.** Given a melody playing through `ocb-dev`, when the owner unplugs the bridge mid-song 20 times, and separately pulls a UART wire mid-song 20 times, then the console goes neutral within 100 ms every time (checked by audio stop or the controller's debug data) and the controller releases TX after the break or link loss.
2. **H.1 (bridge unpowered).** Given the bridge unpowered and the controller on the Switch, when the owner measures the voltage at the bridge's GP1 with a multimeter, then it is ≈ 0 V (no back-powering).
3. **H.3 `console_ready`.** Given H.3 on the Switch 2 with P2, when the owner plugs in with Wired Communication ON, puts the console to sleep and wakes it, then switches Wired Communication OFF and ON, then:
   - `console_ready` is 0 until the handshake and 1 after;
   - it is back to 0 within 150 ms on OFF;
   - on sleep the console cuts USB power (Story 2.3 H.16), so the controller powers off and STATUS stops; after wake the session reports "controller rebooted" and `console_ready` is 1 again after the new handshake;
   - it is 1 again after ON;
   - `START` gets `NAK NOT_READY` whenever it is 0.

   The results are recorded in `docs/test-plan.md`.
4. **UX decisions.** Given the H.3 data, when the owner decides UX Open Questions 3 (STATUS staleness timeout; candidate 3 × 50 ms) and 4 (the 100 ms polling-stall rule), then both decisions are written into `EXPERIENCE.md` (Open Questions resolved, Hardware & Protocol Coupling gap closed). If the 100 ms rule changes, the change also goes into AD-19 and the firmware constant.

The numbering is that of the `epics.md` list ("UX open questions", OQ-3 and OQ-4). `EXPERIENCE.md` numbers its own list differently: its item 3 is the staleness timeout, and the 100 ms rule appears in its "Play gating" row and in the `console_ready` row of the coupling table. This is not an AC mismatch; both places get updated.

**Bench part: yes.** H.1 (bridge-unpowered part), H.3, A.3 (×20 unplug, ×20 wire), and the OQ-3/OQ-4 decision with the owner. Retro action #27 applies (Task 7): the story records a review of the bench tools and H-row commands, and a dry run of every new or changed tool against the simulated model, before the first bench result. Owner instruction: the decisions on OQ-3 and OQ-4 are the owner's; the agent presents the H.3 data and a recommendation, then asks.

## Tasks / Subtasks

- [x] **Task 1: `Io::reopen` and an unplug-aware runner (AC: 1)**
  - [x] 1.1 `Io` gains `fn reopen(&mut self) -> Result<bool, String>`: `Ok(true)` once the port is open again, `Ok(false)` while it is absent. `SerialIo` re-opens the same path at 460800 with DTR, and `false` while the node does not exist or will not open.
  - [x] 1.2 A read or write error is a **port loss** (the bridge unplugged), not a FAIL, in the modes that expect it (`failsafe`, `watch`).
- [x] **Task 2: `ocb-dev <port> failsafe` (AC: 1)** — keyword mode (Story 3.4 D3: still exactly two arguments)
  - [x] 2.1 The melody is embedded (`include_str!` of `timelines/ode-to-joy-90bpm.csv`, parsed with the 3.4 parser). Round 1: the first input and the Enter prompt as in the melody mode. Later rounds: START 3 s after the controller is ready again (no presses; the console already knows the controller).
  - [x] 2.2 Each round: print `ROUND n/20: playing — unplug the bridge or pull the bridge-TX wire now`. Wait for the loss: the port fails (**unplug**), or `PieceAbandoned(LinkLost)` with the port still open (**wire**). `PLAYBACK_DONE` before any loss is a round FAIL ("too late, the piece ended").
  - [x] 2.3 Recovery:
    - unplug: `reopen` every 100 ms (up to 120 s), then a new session (HELLO);
    - wire: wait for STATUS `link_ok` 1 (up to 120 s);
    - then STOP if the latch is set, and wait for `link_ok` and `console_ready` for 2 s.
  - [x] 2.4 Round checks, from the controller's own data (debug STATUS), compared with the last STATUS before the round's START:
    - `abort_count` + 1 (wraps);
    - `last_event` LINK_LOST;
    - `watchdog_trips` + 1;
    - the first STATUS after the recovery shows mode IDLE, so the piece did not continue on the console;
    - wire rounds: `rx_be` rose (the break).
    
    Print one `ROUND n: <kind> PASS|FAIL …` line with these numbers. The summary counts the rounds by kind. `PASS` when 20 rounds passed (`--` none) and none failed.
  - [x] 2.5 Ctrl+C at any time: STOP if a session is open, a summary of the rounds so far, exit 130.
- [x] **Task 3: `ocb-dev <port> watch` (AC: 3, 4)** — keyword mode, the H.3 recorder
  - [x] 3.1 Waits for INFO with no limit (the controller is powered by the console and may be unplugged at start). Then it logs every change of `console_ready` or `link_ok`, STATUS silences (no STATUS for 300 ms: "STATUS stopped"), STATUS resuming, `ControllerRebooted`, EVENTs and NAKs. Each line carries the time since start in ms and the gap from the previous STATUS.
  - [x] 3.2 The NOT_READY probe: while `console_ready` is 0 and `link_ok` is 1, every 2 s, queue one `NEUTRAL|END` at `t_us` 0 and send `START` with lead 0. The controller would have to be ready to play it, and even then it would only go neutral. Expected: `PieceAbandoned(Nak(NotReady))`, then STOP. Count probes and replies by reason.
  - [x] 3.3 After a STATUS silence, the first probe or the reboot STATUS lets the session see the reboot (mid-session `NAK VERSION` or a fresh STATUS): `ControllerRebooted`, re-HELLO. The probe also runs while `console_ready` is 1 and the session is not open, so a reboot is always seen.
  - [x] 3.4 The analysis at Ctrl+C (or after 30 minutes):
    - every `console_ready` 1→0 edge with STATUS still flowing, with the gap between the last ready-1 STATUS and the first ready-0 STATUS (the OFF criterion: ≤ 150 ms + one STATUS period, as `p2watch`);
    - every silence, with its length;
    - every 0→1 edge, with the time from the session's INFO, or from the first STATUS after a silence;
    - probes: n sent, n `NAK NOT_READY`, any other reply;
    - `ControllerRebooted` count;
    - the STATUS interval median, p99 and max while the link was up (the OQ-3 data);
    - when the debug tail has the v4 block, the controller's `in_max_us` and `in_slow` (the OQ-4 data: the longest gap between console polls).
    
    The verdict follows AC3: every 1→0 edge while STATUS flows is within the criterion, every probe while not ready got `NOT_READY`, and every silence was followed by `ControllerRebooted` and then `console_ready` 1.
  - [x] 3.5 The tap parses the debug tail v3 window length and the v4 block (`in_min/p95/max_us`, `in_slow`) when present (`docs/link-protocol.md`).
- [x] **Task 4: host tests and dry run (AC: 1, 3; retro action #27)** — the runner against `ocb-sim` through `SimIo`
  - [x] 4.1 `SimIo` models a bridge unplug (read/write fail, `reopen` false until replug; the fake controller sees `link_ok` false meanwhile) and a wire pull (`link_ok` false, port up).
  - [x] 4.2 C.18 failsafe dry run: 3 unplug rounds and 3 wire rounds → `PASS`, one `ROUND` line each with the right kind; a round where the controller does not latch LINK_LOST → FAIL; `PLAYBACK_DONE` before any loss → round FAIL; Ctrl+C mid-run → STOP, exit 130.
  - [x] 4.3 C.19 watch dry run: a scripted H.3 (start unplugged → INFO → ready 0 → handshake → 1; Wired OFF: ready 0 with STATUS flowing; probes get `NOT_READY`; ON: 1; sleep: STATUS stops; wake: the controller reboots → `ControllerRebooted` → 1) → `PASS` with the expected analysis lines; a 1→0 gap above the criterion → FAIL; a probe answered with something other than `NOT_READY` → FAIL. `ocb-sim` gains what this needs (a reboot that keeps the controller silent until a time, a debug tail with v4).
  - [x] 4.4 The whole workspace stays green (fmt, test, clippy, cargo-deny), and the Story 3.4 tests are unchanged.
- [x] **Task 5: docs (AC: 1–4)**
  - [x] 5.1 `docs/test-plan.md`: C.18–C.19; rows H.3, A.3 and H.1 finalised with the commands; an "A.3 / H.3 bench rig (Story 3.5)" block.
  - [x] 5.2 `app/tools/ocb-dev/README.md`: the two new modes.
- [ ] **Task 6: bench (AC: 1–3)** — after Task 7
  - [ ] 6.1 H.1 bridge-unpowered GP1, A.3 ×20 unplug and ×20 wire (the owner also reads GP0 once with the meter while a wire is pulled, as in H.12), H.3 with `watch`. Record the results in `docs/test-plan.md` and here, with the numbers checked against the logs.
- [x] **Task 7: bench tool and H-row review, recorded before the bench (retro action #27)**
- [ ] **Task 8: the OQ-3/OQ-4 decision (AC: 4)** — present the H.3 data and a recommendation, the owner decides; write `EXPERIENCE.md` (Open Question 3 resolved, the coupling-table gap closed, the "Play gating" row and the `console_ready` row updated for the 100 ms rule). If the owner changes the 100 ms rule: AD-19 in the spine, `OCB_CONSOLE_POLL_STALL_MS` in `firmware/common/proto/include/ocb_proto/consts.h`, its app twin and the docs.

## Dev Notes

### What exists today

- **`ocb-dev` (Story 3.4):**
  - `runner.rs` is a driver generic over `Io` and `Owner`. It has the phases connect → wait_ready → first_input → wait_enter → play, the tap with debug counters v1/v2, liveness (`status_silent_us` 1 s), `stop_quietly`/`finish_stop`, and `SUMMARY:`.
  - Modes: a timeline file, or `soak`. The dry run is `tests/common/mod.rs` (`SimIo` + `FakeController` + `ScriptedOwner`).
  - Liveness must be off in `watch` (STATUS stops on purpose at sleep) and turned into "port loss" handling in `failsafe`.
- **Session (`ocb-core`, Story 3.3):**
  - Reboot detection: a mid-session `NAK VERSION`, or a STATUS with `abort_count` 0 and `last_event` NONE while the previous one was not fresh. Then `ControllerRebooted` and re-HELLO.
  - Before a sleep in H.3 the controller may still be "fresh" (no abort yet), so the probe (3.3) is what makes the session see the reboot, through `NAK VERSION`.
  - `start()` refuses locally with `NoLink` when `link_ok` is 0, so wire probes only run with `link_ok` 1.
- **Firmware (no change planned):**
  - The link watchdog aborts with `EVENT LINK_LOST` 80 ms after the last heartbeat (H.9: 81.1–81.6 ms), or at once on a break, and NEUTRAL is published in the same pass (Story 2.7). The TX release follows the LINK_LOST EVENT (H.12: GP0 0 V while unplugged).
  - `console_ready` uses the 100 ms polling-stall rule (`OCB_CONSOLE_POLL_STALL_MS`, AD-19). STATUS goes out every 50 ms.
  - `fw-debug` has debug tail v1–v3. `fw-debug-o3` adds v4 (`in_max_us`, `in_slow`: the console's polling gaps) and has no built-in melody, so `ocb-dev` plays on it.
- **Bench history:**
  - H.16 (2026-11-22, `p2watch`, 2wiCC bridge): sleep cuts power and STATUS stops; wake gives ready 1.7 s after the first request; OFF drops within one STATUS period; ON 0.0 s.
  - H.12: wire pulls, 10 armed rounds PASS.
  - Story 3.1 bench: own bridge, macOS completes the handshake.
  - Story 3.4 bench: `ocb-dev` on the console; the connection screen appears once per Joy-Con hand-back.

### Decisions (made autonomously, 2026-11-24; the owner can overrule any of them)

- **D1. No firmware change.** "Neutral within 100 ms" is checked per round by the controller's own data and by ear:
  - the round's abort is the link watchdog's (`last_event` LINK_LOST, `watchdog_trips` + 1, `abort_count` + 1), and the piece was not still playing when the link came back (mode IDLE);
  - the watchdog's timing is fixed by design (80 ms, or immediate on a break) and was measured on silicon (H.9);
  - the owner listens for the stop.
  
  A per-round latency field in the debug STATUS was considered and rejected: it is a firmware and wire-layout change for a number the design already bounds. The owner can overrule.
- **D2. Two keyword modes in `ocb-dev`** (`failsafe`, `watch`). They keep the two-argument rule of Story 3.4 and reuse the runner, the tap and the dry run. Python was rejected: it would bypass the Rust session that AC3 names ("the session reports controller rebooted").
- **D3. `failsafe` tells unplug from wire by itself** (port lost or not), so one mode serves both series; the owner runs it twice (20 unplug rounds, then 20 wire rounds). A round of the wrong kind is reported, not failed.
- **D4. Controller build for the whole bench: `fw-debug-o3`.** It has v4 for the OQ-4 data and no built-in melody. One BOOT action by the owner. A.3 does not depend on the build.
- **D5. H.1 needs no tool.** It is a multimeter reading on the bridge's GP1 with the bridge's USB unplugged and the controller in the console.

### Testing requirements

| ID | Test | Pass |
|---|---|---|
| C.18 | `ocb-dev failsafe` dry run (unplug and wire rounds, a missing LINK_LOST, a piece that ends first, Ctrl+C) | all pass |
| C.19 | `ocb-dev watch` dry run (scripted H.3: ready edges, OFF gap, probes, sleep and reboot, verdicts) | all pass |

H rows (bench, draft, finalised by Task 7): H.1 bridge-unpowered part; A.3 (`ocb-dev "$P" failsafe` ×2); H.3 (`ocb-dev "$P" watch` with the five owner actions).

### Architecture compliance

AD-5 (watchdog, TX release), AD-7 (re-HELLO after a reboot), AD-8 (NOT_READY), AD-19 (`console_ready`, the 100 ms rule, STATUS every 50 ms), AD-20 (TX high-Z, H.1).

### File structure

- `app/tools/ocb-dev/src/{runner,serial,tap,main}.rs` (UPDATE), maybe `src/modes/{failsafe,watch}.rs` (NEW)
- `app/tools/ocb-dev/tests/{failsafe,watch}.rs` (NEW), `tests/common/mod.rs` (UPDATE)
- `app/sim/src/controller.rs` (UPDATE if the dry run needs it)
- `docs/test-plan.md`, `app/tools/ocb-dev/README.md`, after the bench `EXPERIENCE.md` (and AD-19 / constants only if the owner changes the rule)

### Previous story intelligence

- **3.4:**
  - The runner's adapter rules: strict timers; `pump` wakes for the silence deadline only while it is in the future.
  - The tap baseline is taken at START, not at session open.
  - The Enter prompt drains earlier input.
  - The test owner is edge-triggered.
  - `kill -INT` must target the tool's own PID, not the wrapper shell (bench slip at H.25).
- **3.1:** macOS completes the handshake. The bridge port is `/dev/cu.usbmodem101`, or `…1101` when plugged after the controller.
- **Epic 2 retro:** the bench tools are code too (review and dry run first).

### Project context rules

Repo content in English, chat in Italian, one question at a time. Never commit `_bmad/`. Never put game screenshots on GitHub. Never edit `~/Documents/org/`. If an AC does not match the console or the code, stop and ask the owner. The OQ-3/OQ-4 decisions are the owner's.

### References

`epics.md` Story 3.5 and the UX OQ list (lines 151–165); `EXPERIENCE.md` (Hardware & Protocol Coupling, Open Questions, Play gating); `ARCHITECTURE-SPINE.md` AD-5, AD-19, AD-20; `docs/link-protocol.md` (Story 2.7 rules, debug v4); `docs/test-plan.md` H.1, H.3, H.9, H.12, H.16, A.3; Story 3.4 file.

## Dev Agent Record

### Agent Model Used

Claude Opus 5.5 (claude-opus-5-5), gds-dev-story, 2026-11-24.

### Debug Log References

- Red proofs (each reverted):
  - no NOT_READY probes → C.19 FAIL;
  - the 200 ms fall limit ignored → C.19 FAIL;
  - reboots never marked after a silence → C.19 FAIL;
  - the round verdict ignored → C.18 FAIL.
  - The `watchdog_trips` round check has no test of its own: removing it leaves C.18 green, because the first-STATUS checks (IDLE, LINK_LOST, `abort_count`) fail first in the only dry-run scenario of a missed loss. Recorded as M-1 below.
- A first C.19 window for the slow fall (250 ms of lost STATUS) was classified as a silence (> 300 ms between STATUS frames), not as a late fall. The test now loses about 200 ms, so STATUS is late but not silent.
- Gates: `cargo fmt --all --check`, `cargo test --locked --workspace` (27 test binaries ok), clippy `-D warnings` clean, `cargo deny` ok.

### Decisions made during implementation (D1 to D5 followed; additions below)

- **D6. The runner became a module** (`runner/mod.rs`, `runner/failsafe.rs`, `runner/watch.rs`), so the new modes reach the runner's private state without widening its API.
- **D7. `Io` gained `reopen` and `idle_until`.** The real `reopen` takes the single `/dev/cu.usbmodem*` node when the old name is gone: macOS renames the bridge's port with the plug order (Story 3.1 H.1). The controller has no CDC port, so the only usbmodem is the bridge.
- **D8. `watch` ends normally with Ctrl+C**, then judges: exit 0 PASS / 1 FAIL, not 130. A probe answered by the reboot (`NAK VERSION`) does not count as a NOT_READY probe.
- **D9. Fall criterion: 200 ms** between the last ready STATUS and the first not-ready STATUS while STATUS flows. That is the 100 ms stall rule plus one STATUS period (150 ms, AD-19), plus one period of slack for the STATUS phase, as `p2watch` judged H.16. A gap above 300 ms is a silence (sleep or power), not a fall.
- **D10. `ocb-sim` counts link losses into the debug tail** (`watchdog_trips` byte 2, `rx_be` bytes 17-18), as the firmware does, so the failsafe dry run judges the same fields as the bench.

### Bench tool and H-row review (Task 7, retro action #27; written before any bench result)

Tools: `app/target/release/ocb-dev` (changed: `failsafe`, `watch`, `Io::reopen`); picotool for one flash; the multimeter. Dry run of the changed tool against the simulated model: C.18 and C.19 (the same `runner::run` the bench uses), and C.13–C.17 still green. Lessons applied: SIGINT goes to the tool's own PID (`pgrep -x ocb-dev`), not to a wrapper shell (3.4 H.25); macOS completes the handshake (3.1); the connection screen appears once per Joy-Con hand-back (3.4 H.24).

- **Flash `fw-debug-o3` on the controller.** Owner: hold BOOT on the controller Zero and plug it into the Mac. Agent: `picotool load -x build/fw-debug-o3/controller/controller.uf2` (built in `firmware/`), then the owner moves it to the console when asked.
- **H.1, bridge-unpowered part.** Owner: unplug the bridge's USB; the controller in the console (on); multimeter DC V on the bridge's GP1 and GND. Expected ≈ 0 V.
- **A.3, unplug run.**
  - Agent: `ocb-dev "$P" failsafe` with stdin from a FIFO; Enter is sent after the owner confirms the ocarina is open (round 1 only).
  - Owner, per round: when the log shows `ROUND n/20: playing` (the agent relays it), unplug the bridge's USB during the melody, listen, plug it back. Twenty times.
  - Expected: `ROUND n: unplug PASS` × 20, then `PASS`.
  - Risk: the port name changes on replug (handled by D7). If the bridge is replugged into another Mac port, the name may change again (also handled).
- **A.3, wire run.**
  - Same command. Owner, per round: pull the bridge-TX → controller-GP1 wire during the melody, listen, push it back.
  - Round 1: also read GP0 on the controller with the meter while the wire is out (expected ≈ 0 V, as H.12).
  - Expected: `ROUND n: wire PASS` × 20.
- **H.3.**
  - Agent: `ocb-dev "$P" watch` (the controller unplugged first: the owner unplugs it from the console).
  - Owner, one at a time and only when asked: plug the controller into the console; sleep; wake (power button, A if asked); Wired Communication OFF; ON.
  - Agent: SIGINT, then reads the `H.3:` analysis.
  - Expected PASS: first STATUS at 0, flowing fall ≤ 200 ms, one silence with the reboot seen and ready after it, probes all `NOT_READY`.
- **OQ-3 / OQ-4.** The agent presents the STATUS interval stats, the OFF fall time, `in_max_us` and `in_slow`, with a recommendation; the owner decides.

### Completion Notes List

- AC1 (tooling): `failsafe` (Task 2), dry run C.18. Bench pending (Task 6).
- AC2: no tool; a multimeter reading at the bench.
- AC3 (tooling): `watch` (Task 3), dry run C.19. Bench pending.
- AC4: after the bench, with the owner (Task 8).

### File List

- `app/tools/ocb-dev/src/runner.rs` → `src/runner/mod.rs` (moved, modified), `src/runner/failsafe.rs`, `src/runner/watch.rs` (new)
- `app/tools/ocb-dev/src/serial.rs`, `src/tap.rs`, `src/main.rs`, `README.md` (modified)
- `app/tools/ocb-dev/tests/common/mod.rs` (modified), `tests/failsafe.rs`, `tests/watch.rs` (new)
- `app/sim/src/controller.rs` (modified: link losses in the debug tail)
- `docs/test-plan.md` (modified: H.3, A.3, C.18, C.19, A.3 / H.3 bench rig)
- `_bmad-output/implementation-artifacts/sprint-status.yaml`, this file

## Change Log

- 2026-11-24: Story created (gds-create-story).
- 2026-11-24: Tooling implemented (`ocb-dev failsafe` and `watch`, dry runs C.18 and C.19); bench and the OQ decision pending.
