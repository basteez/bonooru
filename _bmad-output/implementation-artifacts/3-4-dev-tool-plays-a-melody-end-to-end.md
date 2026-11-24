---
baseline_commit: 3dadb62
---

# Story 3.4: Dev tool plays a melody end to end

Status: review

<!-- Ultimate context engine analysis completed - comprehensive developer guide created (gds-create-story, 2026-11-24) -->

## Story

As the owner,
I want a small command-line dev tool that opens the bridge and streams a timeline,
so that I hear the full chain (PC → bridge → controller → Switch) play a melody.

## Acceptance Criteria

1. **Plays a melody.** Given `app/tools/ocb-dev` (a dev-only binary, not shipped to users; FR-15 applies to the user app) with exactly two arguments: the serial port and a timeline file (`t_us,state` CSV), when the owner runs it with a melody built from the Story 1.5 profile, both boards wired and the console in free play, then it opens the port (DTR asserted), runs the session, prefills, sends `START` with the profile's start lead, streams on credit and the melody plays with every note at the right pitch; and on macOS it opens only `/dev/cu.*` ports.
2. **First input before START.** Given the console shows a wired controller only after its first input (Story 2.3 H.15), when `ocb-dev` has opened the session and before it sends `START`, then it sends D-pad down three times in DIRECT (never A), prompts the owner to dismiss OoT's controller connection screen with the Joy-Con and reopen the ocarina, keeps the link alive with `PING` while waiting, and starts only after the owner presses Enter (Story 2.5 D37).
3. **Ctrl+C.** Given Ctrl+C during playback, when the tool exits, then it sends STOP and the console is neutral.
4. **Soak.** Given the tool in a soak mode (same binary, a long generated timeline) at 460800 baud for 10 minutes, on a controller build without the built-in timeline (that build loads its table at every HELLO and answers every `QUEUE_EVENT` with `NAK BAD_STATE`, Story 2.5 D38), when the run ends, then the controller's debug counters show no dropped bytes and no CRC NAKs; the result is recorded (M4 exit; if bytes are lost, the 921600 option is evaluated as a build-constant change).

**Bench part: yes.** H.24 (melody on the Switch), H.25 (Ctrl+C mid-piece), H.26 (10-minute soak), H.27 (port rules on the Mac). Retro action #27 applies: before the first bench result the story records a review of the bench tools and H-row commands and a dry run of every new or changed tool against the simulated model (Task 8). Owner instruction for this epic: propose running the Story 3.4 and 3.5 bench parts in one session if the rig allows (3.5 needs `ocb-dev` playing a melody for A.3).

## Tasks / Subtasks

- [x] **Task 1: `ocb-core` hook for lazy feeding (AC: 1, 4)**
  - [x] 1.1 `Session::events_unsent(&self) -> usize`: QUEUE_EVENTs pushed with `queue_event` and not yet handed to `poll_transmit` (the send buffer's `qe` queue length; add `SendBuffer::qe_len`). The tool keeps a bounded backlog with it instead of pushing 600 000 soak events at once (D4).
  - [x] 1.2 Test in `app/sim/tests/` (or `app/core` unit test): the count rises with `queue_event`, falls as frames go out on credit, is 0 after STOP abandons the piece. Run `cargo test -p ocb-core --test purity` (the link module is scanned; prose rules of Story 3.3 apply).
- [x] **Task 2: `ocb-sim` debug counters (AC: 4, dry run)**
  - [x] 2.1 `FakeController::set_debug_extra(Option<Vec<u8>>)`: when set, every STATUS carries these bytes as `extra` (the debug tail, v1/v2 layout of `docs/link-protocol.md`). Default `None` (today's behaviour, C.6–C.12 unchanged).
- [x] **Task 3: crate `app/tools/ocb-dev` (AC: 1–4)** — see "Crate layout"
  - [x] 3.1 New workspace member `tools/ocb-dev` (package `ocb-dev`, `publish = false`, `[lints] workspace = true`, description "dev-only bench tool, never shipped"). Dependencies exactly: `ocb-core` (path), `serialport = { version = "=4.10.1", default-features = false }`, `ctrlc = "=3.5.3"`. Dev-dependencies: `ocb-sim` (path). Commit the updated `app/Cargo.lock` (CI runs `--locked`).
  - [x] 3.2 `cargo deny --locked check licenses` passes with the new graph (MPL-2.0 `serialport` is on the allow list; check every new crate; adding a licence is an owner decision, so HALT if one is missing).
- [x] **Task 4: timeline file (AC: 1)**
  - [x] 4.1 `timeline.rs` (pure): parses the CSV of "Timeline file format" into `Timeline { lead_us: u32, events: Vec<(u32, State)> }`. Errors name the line: header missing, bad hex, wrong length, `t_us` not strictly increasing or ≥ 2^31, last row not `NEUTRAL|END`, END before the last row, a state outside the AD-9 allow-list (buttons other than L R A X Y, hat other than up / down / neutral, right stick off centre, reserved flag bits), no `# start_lead_us:` line, more than 100 000 rows. Checked **before** the port is opened.
  - [x] 4.2 `firmware/tools/gen_test_timeline.py` (UPDATE): new option `--out-csv FILE` (default `app/tools/ocb-dev/timelines/ode-to-joy-90bpm.csv`), same events as `testtl_data.c`, `# start_lead_us: 500000` taken from the source header's `start_lead_ms 500` (the Story 1.5 profile value; fail if the header lacks it). Generate and commit the CSV (header comment "GENERATED by …; do not edit").
  - [x] 4.3 `firmware/tests/test_gen_test_timeline.py` (UPDATE, F.38): the committed CSV equals a fresh generation, and its events equal the C table's.
- [x] **Task 5: port rules and wire adapter (AC: 1)**
  - [x] 5.1 `port_rule(path: &str, macos: bool) -> Result<(), String>` (pure): on macOS only `/dev/cu.*` (a `/dev/tty.*` path is refused with a message naming the `cu.` twin); elsewhere any path. `main` passes `cfg!(target_os = "macos")`.
  - [x] 5.2 `serial.rs`: opens the port at 460800 8N1, no flow control, read timeout 1 ms, then `write_data_terminal_ready(true)` (AD-7: DTR asserted on open). Implements the `Wire` trait (read available bytes, write all, set DTR). Only this file touches `serialport`.
- [x] **Task 6: the runner (AC: 1–4)** — `runner.rs`, generic over `Wire`, `Clock` and `Owner` (see "Runner")
  - [x] 6.1 The I/O loop of Story 3.3's adapter rule: per pass read bytes → `handle_rx`; `handle_timeout` only when `poll_timeout` is due; `poll_transmit` until `None` → write; drain `poll_event`. A tap `Splitter(RxSide::App)` on the same received bytes decodes STATUS `extra` (debug counters) and counts NAKs by reason (the session does not expose either).
  - [x] 6.2 Phases: connect (INFO within 5 s, else FAIL naming the likely cause: no bridge, no controller, incompatible firmware); wait for STATUS `link_ok` 1 (bridge heartbeat) and `console_ready` 1 held for 2 s (up to 120 s, prompt "PLUG THE CONTROLLER INTO THE SWITCH …"); print INFO (`fw_ver`, `bridge_ver`, `queue_cap`, `report_period_us`) and the host hint (8000 µs: the console; 2000 µs: a Mac).
  - [x] 6.3 Melody mode first input (AC2): D-pad down ×3 by `set_state` (held 150 ms, 300 ms apart, as `link_probe.py selfplay`), never A; then the prompt "Dismiss the controller connection screen with the Joy-Con, reopen the ocarina, press Enter"; the session stays in DIRECT, so its own PING keeps the link alive (no PING code in the tool); Enter from a stdin thread (the `Owner` trait). Timeout 300 s → STOP, FAIL.
  - [x] 6.4 Play: queue events up to `queue_cap` (the prefill), `start(lead_us)`, then keep `events_unsent() ≤ 256` by feeding more as frames go out (D4); wait for `PlaybackDone` then `Stopped` (the session sends STOP itself). Budget: piece length + lead + 10 s. `PieceAbandoned` / `Refused` / `ControllerRebooted` / `RequiredUpdate` → FAIL with the reason.
  - [x] 6.5 Ctrl+C (AC3): a `ctrlc` handler sets an `AtomicBool`; the loop sees it in any phase after connect, calls `session.stop()`, waits up to 2 s for `Stopped`, prints the last STATUS mode (`IDLE` expected) and exits 130. A second Ctrl+C exits at once (exit 130, no STOP: the DTR drop on close still aborts DIRECT on the controller).
  - [x] 6.6 Summary line at the end of every run: events sent and applied (from STATUS `free` / `PlaybackDone`), resends, rewinds, NAKs by reason, debug counters first → last (when present), then `PASS` or `FAIL <reason>`; exit 0 / 1. Usage errors exit 2.
- [x] **Task 7: soak mode (AC: 4)**
  - [x] 7.1 Second argument `soak` (D3): a generated timeline of 10 minutes, one event every 1000 µs (D6), states alternating NEUTRAL and a sub-deadzone stick nudge (`lx 0x84`, everything else neutral, D7), last event `NEUTRAL|END`; generated lazily (iterator), `lead_us` 500 000. No first input and no Enter prompt in soak mode (D8).
  - [x] 7.2 Soak verdict: needs debug counters (STATUS `extra` ≥ 21 bytes, the v2 split), else FAIL "soak needs a debug controller build (fw-debug)". `PASS` when `PlaybackDone` arrived, no `PieceAbandoned`, transport NAKs (CRC, SEQ, FULL) = 0, and from the first to the last STATUS `dropped_bytes`, `rx_err_bytes`, `rx_overflow_bytes`, `overlong_bytes`, `tx_dropped` and `watchdog_trips` unchanged. Print the soak line: duration, events, bytes written, mean line load (% of 46 080 B/s), counters first → last.
  - [x] 7.3 A refused `QUEUE_EVENT` with `NAK BAD_STATE` right after connect (a timeline build, D38) → FAIL "this controller build has the built-in timeline; flash fw-debug".
- [x] **Task 8: host tests and dry run (AC: 1–4; retro action #27)** — `app/tools/ocb-dev/tests/`, the runner against `ocb-sim`'s `FakeController` through a `SimWire` on a fake clock (see "Dry run")
  - [x] 8.1 C.13 timeline parser: the committed ode-to-joy CSV parses (lead 500 000 µs, the event count of `testtl_data.c`, last `NEUTRAL|END`); every refusal of 4.1 with its line number.
  - [x] 8.2 C.14 port rule: `/dev/cu.usbmodem101` ok and `/dev/tty.usbmodem101` refused on macOS; `COM5`, `/dev/ttyACM0` ok elsewhere.
  - [x] 8.3 C.15 melody dry run: connect, first input (exactly 3 DIRECT presses of D-pad down, no A, released between, `FakeController` `log.direct`), waiting for Enter 5 s of fake time with no app timeout (PINGs), START with `lead_us` 500 000, every event applied in order, `PlaybackDone`, `Stopped`, exit PASS; also through a channel with 2 % drop and corruption.
  - [x] 8.4 C.16 Ctrl+C dry run: the flag raised mid-piece → STOP on the wire, `FakeController` IDLE with a NEUTRAL state, exit 130; raised while waiting for Enter → the same.
  - [x] 8.5 C.17 soak dry run (a 20 s soak through the same code path, `set_debug_extra`): PASS on clean counters; FAIL when `dropped_bytes` rises between the first and last STATUS; FAIL without debug counters; FAIL on a controller that refuses `QUEUE_EVENT` with `NAK BAD_STATE`; bounded backlog (`events_unsent` never above 256 + one batch).
  - [x] 8.6 Refusals: INFO never arrives (FAIL within 5 s of fake time); `console_ready` never 1 (FAIL after 120 s); `link_ok` 0.
  - [x] 8.7 The whole workspace stays green: `cargo fmt --all --check`, `cargo test --locked --workspace`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, `cargo deny --locked check licenses`; `ctest` for F.38.
- [x] **Task 9: docs and bench prep (AC: 1–4)**
  - [x] 9.1 `docs/test-plan.md`: rows C.13–C.17 (App core tests), F.38 extended (CSV), H.24–H.27 (finalised from "H rows" below), and an "`ocb-dev` bench rig (Story 3.4)" block (firmware per row, timeline file, host).
  - [x] 9.2 `app/tools/ocb-dev/README.md`: what it is (dev-only), the two arguments, the CSV format, the soak mode, exit codes, the bench rows.
  - [x] 9.3 `.github/workflows/app.yml`: nothing to add unless the new crate needs it (it builds and tests with `--workspace`; serialport without `libudev` needs no apt package). Confirm on all 3 OS.
  - [x] 9.4 Bench tool and H-row review recorded in the Dev Agent Record **before** the first bench result (retro action #27): what each command does, what it needs from the owner, the expected output, the host (console or Mac) of each row. Remember the Story 3.1 bench lesson: macOS completes the Pro Controller handshake by itself (`console_ready` 1 on the Mac, Story 2.5 D36).
  - [ ] 9.5 After the bench: results in `docs/test-plan.md` and here (numbers checked against the logs), the soak result in `_bmad-output/specs/spec-bonooru/roadmap.md` row M4 (append the result; do not change the M4 state, which also needs cross-OS discovery from Epic 4).

### Review Findings

gds-code-review, 2026-11-24: three parallel reviewers (Blind Hunter, Edge Case Hunter, Acceptance Auditor). 12 IMPORTANT and 7 MINOR fixed, 5 MINOR recorded. One Blind Hunter claim (the CSV not committed) was wrong: the review diff excluded `timelines/`.

IMPORTANT (all fixed, each with a test that fails on the old code):

- **I-1 An Enter typed before the prompt started the piece** (all three). The stdin thread queues lines from launch, and the first `enter_pressed` took a stale one, so the melody could start on the connection screen. Fix: `Owner::discard_input` right before the PROMPT; the test owner is edge-triggered. Test: `an_enter_before_the_prompt_does_not_start_the_piece`.
- **I-2 The soak baseline was the session's first STATUS, and NAKs were counted from port open** (Blind Hunter, Edge Case Hunter). A bounce or a CRC NAK while waiting for the console failed a clean soak. Fix: the baseline is the last STATUS before START and NAKs are counted from START; one more STATUS is read after the final STOP. Test: `the_baseline_is_taken_right_before_the_soak`.
- **I-3 Any `NAK BAD_STATE` was reported as the timeline build** (all three); a START refusal sent the owner to reflash. Fix: the build is recognised by its signature, STATUS `free` < `queue_cap` in IDLE before anything was queued (checked after the ready wait, so no input is sent either); a BAD_STATE abandon only gets a hint. `ocb-sim` now models the real build (`set_table_on_sync`: the table reloads at HELLO and STOP). Tests: `the_timeline_build_is_refused_before_any_event`, `soak_refuses_the_timeline_build`.
- **I-4 A controller that went silent passed the 2 s window and the first input** (Blind Hunter, Edge Case Hunter).
  - Fix: "ready" also needs a STATUS within 150 ms. Every wait fails after 1 s with no STATUS ("stopped answering"), and `pump` wakes for that moment. The first-input SET_STATEs must all be ACKed.
  - Tests: `a_controller_that_goes_silent_fails_the_ready_wait`, `…_mid_piece_fails_fast`.
- **I-5 After a failure, a STOP the session had already started was not waited for** (Edge Case Hunter). This left the abort latch set. Fix: `stop_quietly` / `finish_stop` also handle `Phase::Stopping`. Test: `a_refusal_mid_piece_stops_and_clears_the_latch`.
- **I-6 Task 6.6: the summary was printed only on success** (Acceptance Auditor). Fix: a `SUMMARY:` line ends every run. It shows events queued, `PLAYBACK_DONE`, frames sent, resends, rewinds, NAKs by reason, and the debug counters first and last.
- **I-7 C.16 never checked that the console was neutral** (Acceptance Auditor). Fix: `FakeController::published()`, asserted NEUTRAL after Ctrl+C mid-piece and at the prompt, and after a refusal.
- **I-8 A soak without debug counters failed only after 10 minutes** (Acceptance Auditor). Fix: checked before START (no QUEUE_EVENT is sent). Test: `soak_needs_debug_counters`.
- **I-9 The README claimed a second Ctrl+C stops the controller through DTR** (Edge Case Hunter). DTR aborts DIRECT, not PLAYING, so a queued piece plays on. Fix: README and the handler comment say so. A STOP that is not ACKed now tells the owner to unplug the bridge.
- **I-10 The parser accepted signs and spaces the format forbids** (Blind Hunter, Acceptance Auditor). Fix: digits only for `t_us`, hex digits only for `state`; tests.
- **I-11 The generator took `start_lead_ms` from anywhere, and `500.5` as 500** (all three). Fix: comment lines only, integer only; two F.38 refusals added.
- **I-12 Docs** (Acceptance Auditor).
  - The ready-wait hint told the soak to plug into the Switch, and asked for a plug-in although the controller must be powered before the tool starts. It is now the console handshake hint, or the Mac for the soak.
  - H.25 said "press Ctrl+C" while the bench review sends SIGINT; the row now matches the review.
  - C.13 claimed every refusal names its line.

MINOR fixed:

- The `end2` bound in `on_interrupt` was a no-op. Replaced by `one_more_status`.
- The interrupt message said "before the session opened" for any phase.
- Failure messages hard-coded the limits.
- The prefill test only bounded from above (now exactly 16).
- The steady backlog ≤ 256 was not checked (`Outcome.max_unsent_streaming`).
- L/R byte order was untested (the R row at 5 833 333 µs).
- C.16 did not assert "no STOP before INFO".

MINOR recorded (not fixed):

- **M-1 `SimIo` records only chunks that are exactly one frame** (Blind Hunter). It is exact today, because `poll_transmit` hands out one frame or a lone `0x00` per chunk. It would hide frames if the session ever batched them.
- **M-2 The dry run cannot model write errors, partial writes or the line rate** (Edge Case Hunter). `SerialIo`'s retry loop and the real throughput are exercised only at the bench (H.24–H.26).
- **M-3 `Soak` for more than about 71 minutes would stop before END** (Blind Hunter). `u32` `t_us` cannot reach it; the bench soak is fixed at 10 minutes.
- **M-4 The soak baseline STATUS can be up to 50 ms older than START**. Counters that move in that window count against the soak (a false FAIL, never a false PASS).
- **M-5 A second Ctrl+C leaves a piece already queued on the controller playing** (Edge Case Hunter). By design: it is the way out of a STOP that is never ACKed. Documented; unplugging the bridge makes the controller go neutral.

## Dev Notes

### What exists today (read before writing)

- **`app/core` (`ocb-core`)**: the codec (`protocol::{State, NEUTRAL, Message, Splitter, RxSide, decode_frame, …}`, constants in `protocol::consts`) and the sans-IO `link::Session` (Story 3.3): `new(SessionConfig { hello_seq, app_version })`, `open(now)`, `handle_rx(bytes, now)`, `handle_timeout(now)`, `poll_timeout()`, `poll_transmit(now) -> Option<Vec<u8>>` (one frame with its `0x00`, or a lone `0x00`), `poll_event() -> Option<LinkEvent>`, `queue_event(t_us, state)`, `start(lead_us)`, `set_state(state)`, `stop()`, `info()`, `status() -> Option<&StatusView>`, `phase()`, `credit()`, `unacked()`, `send_stats()`, `rx_stats()`, `is_direct()`, `is_playing()`. Events: `Connected(Info)`, `RequiredUpdate`, `RecommendedUpdate`, `ControllerRebooted`, `Status(StatusView)`, `ControllerEvent(EventCode)`, `PlaybackDone`, `PieceAbandoned { reason }`, `Refused { reason, ty }`, `UnknownNak`, `Stopped`. `queue_event` already refuses what the controller would (order, `t_us ≥ 2^31`, hat > 8, reserved flags, END on a pose, anything after END); it does **not** check the AD-9 allow-list (the tool does, 4.1). `StatusView` has no debug counters: read them from the tap (6.1). The app version is `ocb_core::version` (build.rs, `git describe`, AD-14).
- **`app/sim` (`ocb-sim`)**: `FakeController` (firmware-like replies, `set_console_ready`, `set_link_ok`, `force_abort`, `reboot`, `log()` with `enqueued`, `applied`, `direct`, `app_timeouts`), `Channel` (clean or impaired, latency), `Sim` driver (`run.rs`). The tool's dry run reuses `FakeController` and `Channel` but not `Sim` (the runner is the driver under test).
- **Bench tools**: `firmware/tools/link_probe.py` (`selfplay` is the Python ancestor of this tool: first input, Enter prompt, PING while waiting, 2 s stable `console_ready`; `soak`, `--bridge own`), `p2_probe.py`, `gen_test_timeline.py`. The STATUS debug layout is in `docs/link-protocol.md` "STATUS debug counters" (v1 bytes 0–6, v2 bytes 7–20) and `link_probe.debug_counters`.
- **Firmware**: `fw-debug` (no built-in timeline, debug STATUS v1–v3, reports the published state, D35) is the build for every 3.4 row. `fw-debug-timeline` must **not** be used (it reloads its table at HELLO and STOP, D38). The bridge runs the Story 3.1 firmware (`fw-release`, heartbeat every 20 ms, DTR in its heartbeat flags).
- **Melody source**: `spikes/samples/story-1-6/ode-to-joy-90bpm.txt` (62 notes, 90 BPM, generated from the Story 1.5 profile `app/profiles/oot-switch2-1.1.1.toml`, first press at frame 30, header `start_lead_ms 500`), converted by `gen_test_timeline.py` into `testtl_data.c` (158 events). The same converter writes the CSV (4.2), so the bench melody is the one H.18/H.19 measured.

### Decisions (made autonomously, 2026-11-24; the owner can overrule any of them)

- **D1. One crate, `app/tools/ocb-dev`**, workspace member `tools/ocb-dev`, bin `ocb-dev`, `publish = false`, never bundled (FR-15 is about the user app). Its modules: `main.rs` (args, ctrlc, stdin thread, exit codes), `serial.rs` (the only `serialport` user), `timeline.rs`, `soak.rs`, `runner.rs`, `tap.rs` (debug counters, NAK tally), `port_rule.rs`. `lib.rs` exposes them to the tests.
- **D2. `serialport` without default features** (`=4.10.1`, the AD-18 version): the default `libudev` feature only serves port enumeration on Linux, which the tool never does (it opens the path it is given), so CI needs no `libudev-dev`. `ctrlc =3.5.3` (MIT/Apache-2.0) for SIGINT / console Ctrl+C on all 3 OS.
- **D3. Exactly two arguments, soak by keyword**: `ocb-dev <port> <timeline.csv | soak>`. `soak` is reserved (a file literally named `soak` is passed as `./soak`). Anything else (no args, three args, `--help`) prints usage and exits 2.
- **D4. Bounded backlog**: the runner keeps at most 256 unsent QUEUE_EVENTs in the session (`events_unsent`, Task 1). A melody (158 events) is queued whole; the soak (600 000 events) is fed as frames go out. Memory stays flat; the session's go-back-N and credit rules are untouched.
- **D5. Timeline CSV with a lead directive** (format below): the profile's `start_lead_ms` reaches the tool through the file (`# start_lead_us:`), so the tool needs no TOML parser and keeps two arguments.
- **D6. Soak rate 1 event per 1000 µs**: about 1000 QUEUE_EVENTs/s, about 18 B each on the wire, so about 40 % of the 46 080 B/s line from the PC alone, plus ACK/STATUS back: 100× a melody's rate, without depending on the host keeping a 256 ms queue full at line rate. 10 minutes = 600 000 events, last `t_us` 600 000 000 < 2^31.
- **D7. Soak states are harmless on any host**: NEUTRAL and a left-stick X of `0x84` (well inside any deadzone), both on the AD-9 allow-list. The soak row runs with the controller on the Mac (H.26), not the console.
- **D8. Soak skips the first input and the Enter prompt**; it still waits for `link_ok` and `console_ready` (START needs them).
- **D9. One driver, three traits**: `Wire` (read with ≤ 1 ms wait, write all, set DTR), `Clock` (`now_us`, `sleep_until`) and `Owner` (prompts and the Enter flag). Real: `SerialWire`, `MonoClock` (`Instant`), `TerminalOwner` (stdin thread). Tests: `SimWire` (a `FakeController` and two `Channel`s, the fake clock advanced by `sleep_until`), `ScriptedOwner`. The same runner code runs on the bench and in the dry run.
- **D10. Interrupt flag is an `Arc<AtomicBool>`** passed to the runner, so tests raise it at a chosen fake time.
- **D11. Exit codes**: 0 PASS, 1 FAIL, 2 usage or file error (before the port opens), 130 interrupted (after STOP or the second Ctrl+C).

### Timeline file format (`t_us,state`)

```
# GENERATED by firmware/tools/gen_test_timeline.py from spikes/samples/story-1-6/ode-to-joy-90bpm.txt; do not edit
# start_lead_us: 500000
t_us,state
0,0000088080808000
500000,0100088080808000
900000,0000088080808000
...
44166667,0000088080808001
```

(The first three and the last of the 158 events of `testtl_data.c`: neutral lead-in, NOTE 01 B5 on Y, release, … END at 44.17 s.)

- Lines starting with `#` are comments; `# start_lead_us: N` is required once (decimal µs, ≤ 2^32 − 1).
- The first non-comment line is exactly `t_us,state`.
- `t_us`: decimal, strictly increasing, < 2^31. `state`: 16 hex digits, the 8 wire bytes of AD-10 in order (`buttons` LE, `hat`, `lx`, `ly`, `rx`, `ry`, `flags`), i.e. `State::to_bytes` as hex. Case-insensitive; no spaces.
- The last row is `NEUTRAL|END` (`0000088080808001`); END nowhere else. At most 100 000 rows.
- AD-9 allow-list (the tool refuses anything else, before opening the port): buttons ⊆ {Y 0x0001, A 0x0004, X 0x0008, L 0x0010, R 0x0020}, hat ∈ {0 up, 4 down, 8 neutral}, `rx = ry = 0x80`, flags ⊆ {END}.

### Runner

```
open port (DTR 1) ─ session.open ─ INFO? (5 s) ─ link_ok 1 and console_ready 1 for 2 s? (120 s)
  melody: SET_STATE hat-down ×3 (150 ms held, 300 ms apart) ─ prompt ─ Enter? (300 s, PINGs by the session)
  queue ≤ queue_cap ─ START lead ─ feed while events_unsent ≤ 256 ─ PlaybackDone ─ Stopped ─ summary ─ PASS
  Ctrl+C anywhere after open: STOP ─ Stopped? (2 s) ─ exit 130
```

The loop never busy-waits: it sleeps until the earliest of `poll_timeout`, the next press edge, 1 ms (serial read timeout) and the phase deadline. It is the adapter Story 3.3 describes: `handle_timeout` only when due, `poll_transmit` drained after every input.

### Dry run (Task 8, retro action #27)

`SimWire` couples the runner to `FakeController` exactly as `ocb-sim::Sim` couples the session: `write` → up channel → `ctrl.handle_rx`; each `read` delivers due bytes of the down channel after `ctrl.tick` and `ctrl.poll_transmit`; `Clock::sleep_until` advances the fake time (no real sleeping). `ScriptedOwner` presses Enter at a set fake time. Raising the interrupt flag at a fake time tests Ctrl+C. This is the dry run of the new tool against the simulated model before any bench (as F.20, F.23, F.42 did for the Python probes).

### Testing requirements (rows for `docs/test-plan.md`)

App core tests (C rows, `cargo test --workspace`):

| ID | Test | Pass |
|---|---|---|
| C.13 | `ocb-dev` timeline parser: the committed CSV and every refusal with its line | all pass |
| C.14 | `ocb-dev` port rule (macOS `cu.` only) | all pass |
| C.15 | `ocb-dev` melody dry run against `ocb-sim` (first input, Enter, PINGs, START lead, every event applied, clean and impaired) | all pass |
| C.16 | `ocb-dev` Ctrl+C dry run (mid-piece and at the prompt: STOP, IDLE, NEUTRAL, exit 130) | all pass |
| C.17 | `ocb-dev` soak dry run (PASS, FAIL on a counter rise, no debug counters, the timeline build; bounded backlog) | all pass |

F.38 extended: the CSV is regenerated and compared, and its events equal the C table.

H rows (bench, owner present; draft, finalised by Task 9.4):

| ID | Steps (draft) | Pass |
|---|---|---|
| H.24 | Melody on the Switch (AC1, AC2): controller `fw-debug` in the console's top USB-C port (unplugged from the Mac), bridge on the Mac, console in OoT ocarina free play opened with the Joy-Con; `app/target/release/ocb-dev "$P" app/tools/ocb-dev/timelines/ode-to-joy-90bpm.csv`; at the prompt the owner dismisses the connection screen, reopens the ocarina, presses Enter; listens. Optional: the H.19 analyser with `--driver` set to the same command | `PASS`; the melody plays, every note at the right pitch (owner); 3 D-pad presses seen, no A; INFO `report_period_us` 8000 (the console) |
| H.25 | Ctrl+C mid-piece (AC3): as H.24, Ctrl+C after about 5 s of melody | the sound stops (no held note), the tool prints STOP ACKed and mode IDLE, exit 130; a second run then plays normally |
| H.26 | Soak (AC4): controller `fw-debug` on the Mac (macOS gives `console_ready` 1), bridge wired; `ocb-dev "$P" soak` (10 min) | `PASS`: `dropped_bytes` and the v2 split unchanged, 0 CRC (transport) NAKs, `watchdog_trips` unchanged, 600 000 events applied; record the soak line. If bytes are lost: evaluate 921600 (build constant) |
| H.27 | Port rules on the Mac (AC1): `ocb-dev /dev/tty.usbmodemXXXX …` | refused with the `cu.` hint, exit 2, the port not opened |

### Architecture compliance

- AD-7: DTR asserted on open, a lone `0x00`, HELLO every 100 ms until INFO (the session does it); runtime values from INFO only (`queue_cap` for the prefill, never hard-coded 512).
- AD-6: go-back-N, window and credits are the session's; the tool only feeds events and never sends frames itself.
- AD-8 / AD-9: START in DIRECT turns the real-time state NEUTRAL and plays; the allow-list is checked by the tool before anything is sent.
- AD-15: the streamer prefills, starts and sends on credit; it never edits, drops or reorders events.
- AD-18: macOS shows and opens only `cu.*`; `serialport` (MPL-2.0) unmodified.
- AD-21 / NFR-07: `cargo deny` licence gate over the new graph.
- AD-14: the app version from `ocb_core::version` (no new version source).

### Library / framework requirements

- Rust 1.100.0 (pinned), edition 2024. PATH on this Mac: `export PATH=/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH`.
- `serialport =4.10.1` (default features off: `libudev` is only for enumeration), `ctrlc =3.5.3` (latest on crates.io 2026-11-24). Workspace lints apply to the tool too: no `unwrap`, `expect`, `panic!`, slice indexing in non-test code; tests opt out locally as in `app/sim/tests`.
- `serialport` API used: `serialport::new(path, 460_800).data_bits(Eight).parity(None).stop_bits(One).flow_control(None).timeout(Duration::from_millis(1)).open()`, `write_data_terminal_ready(true)`, `read` (a `TimedOut` error means no bytes), `write_all`. `ctrlc::set_handler(move || …)` once in `main`.

### File structure

- `app/Cargo.toml` (UPDATE: member `tools/ocb-dev`), `app/Cargo.lock` (UPDATE)
- `app/core/src/link/mod.rs`, `app/core/src/link/send.rs` (UPDATE: `events_unsent`, `qe_len`)
- `app/sim/src/controller.rs` (UPDATE: `set_debug_extra`)
- `app/tools/ocb-dev/Cargo.toml`, `README.md`, `src/{main,lib,serial,timeline,soak,runner,tap,port_rule}.rs`, `tests/*.rs`, `timelines/ode-to-joy-90bpm.csv` (NEW)
- `firmware/tools/gen_test_timeline.py`, `firmware/tests/test_gen_test_timeline.py` (UPDATE)
- `docs/test-plan.md` (UPDATE), `_bmad-output/specs/spec-bonooru/roadmap.md` (UPDATE after the bench)

### Previous story intelligence

- **3.3**: the session's adapter contract (strict timers, drain `poll_transmit`); START never waits behind QEs without credit; `NotPlaying` covers a lost START refusal; a reboot seen from STATUS triggers re-HELLO. The sim's checks are wire-only; the dry run here is the first real adapter.
- **3.1 bench (2026-11-24)**: macOS completes the Pro Controller handshake by itself, so with the controller on the Mac `console_ready` is 1 (my 3.1 H-row review missed it; retro 3.2 already said so, Story 2.5 D36). H.26 uses that on purpose; H.24/H.25 need the controller in the console and **unplugged from the Mac**. The bridge's port is `/dev/cu.usbmodem101` (or `…1101` when plugged after the controller); serial `53032847189C2E9C`. The own bridge passed a 300 s hold and the 2.6 soaks with 0 NAKs clean.
- **3.2**: workspace lints (no panics), purity gate on `app/core` (the new accessor must not use banned identifiers, comments included), cargo-deny 0.22.0 in CI, `cargo fmt` after the last edit (3.3 CI failure).
- **Epic 2 retro**: the bench tools are code too (review and dry run first); five of six bench failures were tool bugs. OoT shows its controller connection screen after the first input; the Enter prompt exists for it (D37).

### Git intelligence

Recent: `3dadb62` merge of PR #33 (Story 3.1 bridge firmware and bench), `ce79ac9` PR #32 (Story 3.3 session), Story 3.2 workspace. Conventions: one commit for the implementation, one for review fixes, one for the bench record; PR per story, never merged by the agent unless told.

### Project context rules

- Repo content in English; chat with the owner in Italian. One question at a time.
- Never commit `_bmad/` or the bmad-*/gds-* skills; `_bmad-output/` is committed.
- Game screenshots and recordings never go on GitHub (`spikes/recordings/` is git-ignored).
- Never edit `~/Documents/org/`.
- If an AC does not match the console or the code: stop and ask the owner; do not rewrite ACs.

### References

- `_bmad-output/planning-artifacts/epics.md` Story 3.4 (and 3.5 for the shared bench)
- `_bmad-output/planning-artifacts/architecture/architecture-bonooru-2026-11-19/ARCHITECTURE-SPINE.md` AD-6, AD-7, AD-8, AD-9, AD-13, AD-15, AD-18, AD-19, AD-21; `app-stack-options.md`
- `_bmad-output/implementation-artifacts/epic-2-retro-2026-11-24.md` (sections 3.2, 6, 7)
- `_bmad-output/implementation-artifacts/3-1-…`, `3-2-…`, `3-3-…`; `2-5-…` (D36, D37, D38, `selfplay`)
- `docs/link-protocol.md` (STATUS debug counters; rules the app link applies); `docs/test-plan.md` (H.18, H.19, H.22, own-bridge rig block)

## Dev Agent Record

### Agent Model Used

Claude Opus 5.5 (claude-opus-5-5), gds-dev-story, 2026-11-24.

### Debug Log References

- Red first: `app/sim/tests/backlog.rs` did not compile before `events_unsent` existed.
- Red proofs on the runner (each reverted): the first input pressing A → C.15 fails (2 tests); no wait for Enter → C.15 fails; Ctrl+C without STOP → C.16 fails (2); the soak ignoring moved counters → C.17 fails; an unbounded backlog → C.17 fails; START with lead 0 → passed at first, so `ControllerLog.starts` was added to `ocb-sim` and C.15 / C.17 now check the lead (red again: C.15 fails). F.38: one changed CSV row → `drift: app/tools/ocb-dev/timelines/ode-to-joy-90bpm.csv differs`.
- `cargo deny` rejected `unicode-ident` (`(MIT OR Apache-2.0) AND Unicode-3.0`, reached through serialport → unescaper → thiserror-impl → proc-macro2). HALT per Task 3.2; owner decision: allow `Unicode-3.0` (D12).
- Gates on the final tree: `cargo fmt --all --check`, `cargo test --locked --workspace` (all pass), `cargo clippy --locked --workspace --all-targets -- -D warnings` (clean), `cargo deny --locked check licenses` (ok), `ctest --preset host-tests` 44/44, F.38 ok.

### Decisions made during implementation (D1 to D11 followed; additions below)

- **D12. `Unicode-3.0` allowed in `app/deny.toml`** (owner decision, 2026-11-24, NFR-07): a permissive OSI licence, reached through `unicode-ident`; egui will need it too.
- **D13. `ocb-sim` logs START leads** (`ControllerLog.starts`): the only way the dry run can see the lead the tool sent; added after the red proof above showed the lead was unchecked.
- **D14. The prefill is `queue_cap`, the steady backlog 256**: right after START up to `queue_cap` events can be unsent at once, so the bounded-backlog check (8.5) is `max_unsent ≤ queue_cap` (512), not 256 + one batch as drafted.
- **D15. Serial writes retry on the 1 ms timeout** until the whole chunk is out or 1 s passes (a full USB buffer is back-pressure, a stuck port is an error); reads treat `TimedOut`, `WouldBlock` and `Interrupted` (Ctrl+C's EINTR) as "no bytes".
- **D16. The runner reports, it does not decide the host**: INFO `report_period_us` is printed with a hint (8000 µs the console, 2000 µs a computer), but a melody on the Mac is not refused (unlike `selfplay`, D36): the soak relies on the Mac host, and a melody on the Mac is harmless.

### Bench tool and H-row review (Task 9.4, retro action #27; written before any bench result)

Tools: `app/target/release/ocb-dev` (new; `cargo build --release -p ocb-dev` in `app/`), `firmware/tools/gen_test_timeline.py` (changed: CSV output; its dry run is F.38), `link_probe.py` and `p2_probe.py` (unchanged, not needed). Dry run of the new tool against the simulated model: C.15 (melody: first input, prompt, PINGs, START lead, every event, clean and impaired), C.16 (Ctrl+C mid-piece and at the prompt), C.17 (soak verdicts, timeline build, bounded backlog), all through the same `runner::run` the bench uses. Lesson of the Story 3.1 bench applied: macOS completes the Pro Controller handshake by itself, so the host of each row is stated.

- **H.24 (console).** Owner: unplug the controller Zero from the Mac and plug it into the console's top USB-C port; console on, Wired Communication ON, OoT ocarina free play opened with the Joy-Con. Agent: `ocb-dev "$P" app/tools/ocb-dev/timelines/ode-to-joy-90bpm.csv` with stdin from a FIFO in the scratchpad (the agent cannot type into the tool's terminal: it writes one line into the FIFO when the owner says the screen is dismissed and the ocarina reopened). Expected: INFO `report_period_us` about 8000, `link_ok 1 and console_ready 1`, 3 `first input` lines, the PROMPT; then START, `PLAYBACK_DONE: 158 events`, `PASS`. Owner: listen and say whether every note is right. Risk: the 2 s stable window needs the controller to stay plugged; the console's connection screen appears after the first input (expected).
- **H.25 (console).** As H.24; the agent sends SIGINT (`kill -INT`) to the tool about 5 s after START (the same handler as Ctrl+C). Expected: `INTERRUPTED: STOP ACKed, controller mode Idle (neutral)`, exit 130. Owner: say whether the sound stopped with no held note. Then one more H.24 run.
- **H.26 (Mac).** Owner: unplug the controller from the console and plug it into the Mac. Agent: `ocb-dev "$P" soak` in the background (10 min + lead). Expected: `SOAK:` line, both debug lines equal in the six counters, `transport NAKs 0`, `PASS`. Needs nothing else from the owner.
- **H.27 (agent alone).** `ocb-dev /dev/tty.usbmodemXXXX <csv>`: refused with the `/dev/cu.` name, exit 2.
- **Shared session with Story 3.5** (owner instruction): 3.5's A.3 needs `ocb-dev` playing a melody; H.24's setup is the same, so the two bench parts can run back to back once 3.5 is implemented. To be proposed to the owner at the bench.

### Completion Notes List

- AC1: `ocb-dev <port> <timeline.csv | soak>` (D3); DTR asserted on open (`serial.rs`); the session prefills up to INFO `queue_cap`, START with the file's `start_lead_us` (500 000, the Story 1.5 profile value through `gen_test_timeline.py`), streams on credit with a bounded backlog (`Session::events_unsent`); macOS `/dev/cu.*` only (`port_rule`). Dry run C.15; bench H.24, H.27.
- AC2: D-pad down ×3 in DIRECT (150 ms held, 300 ms apart, no button), the prompt, PINGs by the session while waiting, START only after Enter. Dry run C.15; bench H.24.
- AC3: Ctrl+C → STOP, wait for its ACK, mode IDLE printed, exit 130 (a second Ctrl+C exits at once). Dry run C.16; bench H.25.
- AC4: `soak` = 10 minutes, 1 event/ms, harmless states, needs fw-debug's v2 debug counters; PASS only with 0 transport NAKs and six counters unchanged; the timeline build is named. Dry run C.17; bench H.26.
- Task 9.5 (results, roadmap M4) waits for the bench.

### File List

- `app/Cargo.toml`, `app/Cargo.lock`, `app/deny.toml` (modified)
- `app/core/src/link/mod.rs`, `app/core/src/link/send.rs` (modified: `events_unsent`, `qe_len`)
- `app/sim/src/controller.rs` (modified: `set_debug_extra`, `ControllerLog.starts`, `set_table_on_sync`, `published`)
- `app/sim/tests/backlog.rs` (new)
- `app/tools/ocb-dev/Cargo.toml`, `README.md`, `src/{main,lib,port_rule,runner,serial,soak,tap,timeline}.rs`, `tests/common/mod.rs`, `tests/{timeline,port_rule,melody,interrupt,soak}.rs`, `timelines/ode-to-joy-90bpm.csv` (new)
- `firmware/tools/gen_test_timeline.py`, `firmware/tests/test_gen_test_timeline.py` (modified)
- `docs/test-plan.md` (modified: F.38, C.13–C.17, H.24–H.27, `ocb-dev` bench rig)
- `_bmad-output/implementation-artifacts/sprint-status.yaml`, this file

## Change Log

- 2026-11-24: Story created (gds-create-story).
- 2026-11-24: Story 3.4 implemented (`ocb-dev`, `events_unsent`, sim debug counters, CSV generator, C.13–C.17, F.38 extended); `Unicode-3.0` allowed (owner). Bench part pending.
- 2026-11-24: Code review: 12 IMPORTANT and 7 MINOR fixed, 5 MINOR recorded (Review Findings); C.13, C.15–C.17 and H.25 rows updated.
