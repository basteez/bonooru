---
baseline_commit: 7b13971d93d867e66da5694026e4604af05aebfa
---

# Story 3.3: Sans-IO link session

Status: review

<!-- Ultimate context engine analysis completed - comprehensive developer guide created (gds-create-story, 2026-11-24) -->

## Story

As the owner,
I want a link session that handles HELLO, ACKs, retries and credits without touching a port,
so that reliability logic is fully tested on the PC before it meets real hardware.

## Acceptance Criteria

1. **Session open.** Given the `link` module in `ocb-core`, which takes bytes plus a clock and returns bytes plus typed events, when a session opens, then it emits one `0x00`, then only `HELLO` every 100 ms until `INFO` arrives, and exposes INFO's values (`queue_cap`, `app_timeout_ms`, `report_period_us`, versions) without hard-coding them (AD-7); and a `proto_ver` mismatch is reported as a typed "required update" event; `fw_ver`/`bridge_ver` ≠ app version as "recommended update" (AD-13).
2. **Go-back-N and credits.** Given an in-test fake controller with injected loss, corruption and reordering, when a stream of `QUEUE_EVENT`s is sent, then go-back-N recovers every frame in order (cumulative ACK, resend after 50 ms, rewind only on a NAK whose `expected` is ahead of the last rewind point), credits follow `STATUS.free − QUEUE_EVENTs sent after last_seq`, and at most `min(credit, 64 − unacked)` frames are in flight (AD-6).
3. **Semantic NAK.** Given a semantic NAK on `QUEUE_EVENT` or `START`, when the session receives it, then it abandons the piece, emits STOP and a typed event naming the reason.
4. **Controller rebooted.** Given a mid-session `NAK VERSION`, when it arrives, then the session re-sends HELLO and reports "controller rebooted".
5. **PING.** Given DIRECT mode, when the session has sent nothing for `app_timeout_ms/4`, then it emits `PING`.
6. **Unknown codes.** Given unknown EVENT codes or NAK reasons, when they arrive, then they are reported as typed "unknown" events, never as errors that end the session (AD-19).

No bench part: host-only story (no H-row, no retro action #27 step). Story 3.4 is the first to put this session on the wire.

## Tasks / Subtasks

- [x] **Task 1: `link` module skeleton and API (AC: 1–6)** — `app/core/src/link/` (see "API shape")
  - [x] 1.1 `link/mod.rs`: `Session`, `SessionConfig`, `LinkEvent`, `Command` errors, re-exports. `lib.rs` gets `pub mod link;`.
  - [x] 1.2 Sans-IO contract: no clock read, no port. Time is a `u64` in µs passed by the caller (monotonic, from the adapter; the purity gate bans `Instant`). Inputs: `handle_rx(&[u8], now_us)`, `handle_timeout(now_us)`, commands. Outputs: `poll_transmit() -> Option<Vec<u8>>` (whole frames, already COBS-encoded with the `0x00`), `poll_event() -> Option<LinkEvent>`, `poll_timeout() -> Option<u64>` (the next instant `handle_timeout` must be called).
  - [x] 1.3 The receive side uses `protocol::Splitter` with `RxSide::App` and `Message::decode`; a body that fails to decode (`BodyLen`, `UnknownType`) is counted and reported as `LinkEvent::Malformed { ty }`, never fatal.
- [x] **Task 2: Session start and versions (AC: 1)**
  - [x] 2.1 `open(now_us)`: queue one lone `0x00`, then HELLO (`proto_ver = PROTO_VERSION`) with the session's HELLO seq; resend the **same** HELLO (same seq, D3) every `HELLO_RESEND_MS` (100 ms) until an INFO arrives. Nothing else is sent before INFO (commands return `Err(NotConnected)`).
  - [x] 2.2 On INFO with `proto_ver == PROTO_VERSION`: state `Connected`, `expected_next_seq = hello_seq + 1`, store `Info` (the session's runtime values: `queue_cap`, `app_timeout_ms`, `report_period_us`, `hid_profile`, `fw_ver`, `bridge_ver`), emit `LinkEvent::Connected(Info)`.
  - [x] 2.3 On INFO with another `proto_ver`: emit `LinkEvent::RequiredUpdate { controller_proto: u8, app_proto: u8, info }`, state `Incompatible`, stop resending HELLO (D4). Only `open()` again leaves this state.
  - [x] 2.4 After a matching INFO, compare `fw_ver` and `bridge_ver` with `SessionConfig::app_version` (default `version::APP_VERSION_TRIPLE`): one `LinkEvent::RecommendedUpdate { fw_ver, bridge_ver, app_ver, fw_differs, bridge_differs }` if either differs (D5). `bridge_ver` `[0,0,0]` means "no heartbeat seen yet" (AD-7) and is reported as unknown (`bridge_differs: false`, D5).
  - [x] 2.5 Never hard-code the INFO values: credits use `INFO.queue_cap` only through STATUS `free`; PING uses `INFO.app_timeout_ms`; the session exposes `info()` for the streamer and the UI.
- [x] **Task 3: Reliable send path, go-back-N (AC: 2)**
  - [x] 3.1 One seq space for every app frame after HELLO (`QUEUE_EVENT`, `START`, `SET_STATE`, `PING`): u8, wraps, `seq = hello_seq + 1 + n`. Keep a send buffer of unACKed frames (type, seq, body, is_queue_event).
  - [x] 3.2 Cumulative ACK: `ACK(s)` retires every unACKed frame up to `s` if `s` is in `[base−1, base+63]` (mod 256); anything else is stale and ignored. A NAK's `expected` also retires everything before it (Story 2.6 D7, as the bench probe does). STATUS `last_seq` retires the same way.
  - [x] 3.3 Resend: if the oldest unACKed frame has waited `ACK_TIMEOUT_MS` (50 ms) since it (or its last resend) was sent, go back and resend from it; clear the rewind point.
  - [x] 3.4 Rewind on a transport NAK (`CRC`, `SEQ`, `FULL`) only if its `expected` is ahead of the last rewind point (AD-6): next = the frame with seq `expected`; record the rewind point. A NAK whose `expected` is not ahead is ignored (it only retires).
  - [x] 3.5 Credits: `credit = STATUS.free − (QUEUE_EVENTs sent with seq after STATUS.last_seq)`; before the first STATUS after connecting, credit is 0 for **new** QUEUE_EVENTs (D6). Window: a new frame goes out only if `unacked < ACK_WINDOW` (64) and, for a QUEUE_EVENT, `credit > 0`. Non-queue frames (`START`, `SET_STATE`, `PING`) need no credit but count in the 64 window.
  - [x] 3.6 `poll_transmit` hands out at most what the window allows; frames are produced in order, never edited, dropped or reordered (AD-15).
  - [x] 3.7 After a link loss (see Task 5) or a `0x00`-requiring recovery, the next transmitted frame is preceded by one lone `0x00` (Story 2.7 "What the app must do").
- [x] **Task 4: Piece commands and local mirrors of the controller's refusals (AC: 2, 3)**
  - [x] 4.1 `queue_event(t_us, State) -> Result<(), CommandError>` appends to the piece. Refuse locally (never sent), mirroring the controller core (Story 2.4 "Rules the controller core applies"): `t_us` not strictly increasing → `Order`; `t_us ≥ T_US_LIMIT` → `Order`; `hat > 8` or a reserved flag bit → `BadState`; END on a state that is not the neutral pose → `BadState`; anything after a queued END → `BadState` (retro: the two extra refusals); not connected → `NotConnected`; aborted/abandoned piece not yet cleared by STOP → `Aborted`.
  - [x] 4.2 `start(lead_us) -> Result<(), CommandError>`: refuse with `EmptyQueue` if nothing was queued (mirrors `NAK BAD_STATE` on `START` with an empty queue), `NoLink` if the last STATUS says `link_ok` 0 (never send START while `link_ok` is 0, Story 2.7), `NotConnected`. START goes into the seq stream **after** every QUEUE_EVENT queued before it (the prefill is not required to be ACKed first; order on the wire is the guarantee).
  - [x] 4.3 `set_state(State)`: DIRECT input. Refused with `NoLink` while `link_ok` is 0, `BadState` for `hat > 8`, any flag set. Enters the session's DIRECT tracking (PING timer, Task 6).
  - [x] 4.4 `stop()`: always allowed once connected. Discards every unACKed and unsent app frame (the piece is abandoned), sends `STOP` with the next seq (a sync point: `expected` becomes `seq+1` on the controller), and sends **nothing else until STOP's ACK** (Story 2.6 "What the app must do": a late duplicate STOP would clear frames sent after it). STOP is resent every 50 ms until ACKed. After the ACK: state back to `Connected` idle, piece cleared, rewind point cleared.
  - [x] 4.5 Semantic NAK (`0x10–0x7F`, `NakReason::is_semantic`) whose `got` is a QUEUE_EVENT or START of the current piece: abandon the piece and issue `stop()` internally; emit `LinkEvent::PieceAbandoned { reason: AbandonReason::Nak(NakReason), seq }` (AC3). A semantic NAK on SET_STATE or PING: emit `LinkEvent::Refused { reason, ty }`, no STOP (D7). `NAK VERSION` is handled by Task 5.2, not here.
- [x] **Task 5: Recovery rules (AC: 4, 6)**
  - [x] 5.1 `EVENT LINK_LOST`, or a STATUS with `link_ok` 0 and `abort_count` higher than the last seen, or `abort_count` rising with no EVENT (app timeout, DTR edge): abandon the piece and `stop()` (Story 2.7 "What the app must do"); emit `LinkEvent::PieceAbandoned { reason: AbandonReason::LinkLost | AbandonReason::ControllerAborted }` when a piece or DIRECT was active, else only the status change. Set the "lone `0x00` before the next frame" flag after `LINK_LOST`.
  - [x] 5.2 Mid-session `NAK VERSION` (session was `Connected`): emit `LinkEvent::ControllerRebooted`, drop all send state, go back to `Opening` and send HELLO again (with a lone `0x00` first) every 100 ms until INFO (AC4). A piece in progress is abandoned (`PieceAbandoned { reason: ControllerRebooted }`).
  - [x] 5.3 `EVENT PLAYBACK_DONE`: emit `LinkEvent::PlaybackDone`; the piece is finished; the controller has latched (AD-8), so the session sends `STOP` to clear the latch before the next piece (D8).
  - [x] 5.4 Every EVENT is also forwarded as `LinkEvent::ControllerEvent(EventCode)`; an unknown code is `EventCode::Unknown(c)` and changes nothing else (AC6). An unknown NAK reason (`NakReason::Unknown(r)`) is `LinkEvent::UnknownNak { reason, expected, got }`; it still retires frames before `expected` but never rewinds and never ends the session (AC6, D9).
  - [x] 5.5 STATUS: keep the last snapshot (`mode`, `free`, `last_seq`, `abort_count`, `last_event`, `link_ok`, `console_ready`, `now_us`) and emit `LinkEvent::Status(StatusView)`; the mode the app shows comes from STATUS (AD-19). Reserved flag bits are ignored.
- [x] **Task 6: PING in DIRECT (AC: 5)**
  - [x] 6.1 While the session believes the controller is in DIRECT (a `set_state` was sent and no STOP/abort since; or STATUS mode DIRECT), if no app frame was sent for `app_timeout_ms / APP_PING_DIVISOR` (from INFO; 250 ms with the default 1000), emit `PING`. Any app frame (SET_STATE, PING, a resend) resets the timer. No PING outside DIRECT.
  - [x] 6.2 `poll_timeout` includes the PING deadline.
- [x] **Task 7: Fake controller and impaired channel, crate `ocb-sim` (AC: 2–6)** — `app/sim/`, dev-only, not shipped (D2)
  - [x] 7.1 `FakeController`: the controller link and core rules the app relies on, from `docs/link-protocol.md` (AD-6, AD-7, AD-8 and the Story 2.4 / 2.6 / 2.7 sections): session gate (`NAK VERSION` before HELLO), HELLO with matching / mismatched `proto_ver` → INFO (configurable `fw_ver`, `bridge_ver`, `queue_cap`, `app_timeout_ms`, `report_period_us`, `proto_ver`), STOP/HELLO sync points, `seq == expected` only, stale window → `ACK(expected−1)`, cached response for `d = 1`, `NAK SEQ` beyond, one transport NAK per `expected`, `NAK CRC` on a damaged frame through that budget, queue with capacity and `NAK FULL`, NAK precedence `ABORTED → NO_LINK → BUSY → NOT_READY → BAD_STATE → ORDER → FULL`, modes IDLE/DIRECT/PLAYING with the abort latch, STATUS every 50 ms plus after ACKs (≥ 10 ms apart) with `free`, `last_seq`, `mode`, `abort_count`, `last_event`, flags, and a scheduler that consumes queued events by time after START so `free` grows (enough for credits; no USB).
  - [x] 7.2 Knobs for tests: `link_ok`, `console_ready`, `reboot()` (session closed, `expected` 0, `abort_count` 0), `inject_event(code)`, `inject_nak(reason)` on the next frame of a type, `force_abort(cause)`.
  - [x] 7.3 `Channel`: a byte pipe each way with a seeded PRNG (own xorshift, no `rand` dependency) and per-direction knobs: drop a whole frame, flip a bit, insert junk, duplicate, reorder adjacent frames, latency in µs. Byte-accurate wire timing at 460800 baud is optional (D10).
  - [x] 7.4 `Sim::run(session, controller, channel, until_us)`: steps a shared fake clock (1 ms step or event-driven via `poll_timeout`), moves bytes, ticks the controller. The invariants of Task 8.3 are checked after every step.
  - [x] 7.5 Purity is not required in `ocb-sim` (not the core), but no threads and no real clock: everything runs on the fake clock so tests are deterministic and replayable by seed (`OCB_SIM_SEED` env var replays one seed, like `OCB_FUZZ_SEED` in F.17).
- [x] **Task 8: Tests (AC: 1–6)** — `app/sim/tests/`, rows C.6 to C.11 (see Testing requirements)
  - [x] 8.1 Open (C.6): first bytes are exactly `00` then one HELLO frame; then HELLO every 100 ms (101 ms timestamps checked, never more than one per 100 ms), nothing else, until INFO; INFO values exposed (non-default values in the fake: `queue_cap` 7, `app_timeout_ms` 400, `report_period_us` 8000); mismatch → `RequiredUpdate`, no more HELLO; `fw_ver` / `bridge_ver` ≠ app version → one `RecommendedUpdate`; equal → none; `bridge_ver` zeros → not "differs".
  - [x] 8.2 GBN (C.7): 2000 QUEUE_EVENTs through the channel at 0, 1, 5, 10 % drop, bit-flip, duplicate and reorder (each alone and all together), 3 seeds each, queue capacity 512 and 5: the fake controller's queue receives every event exactly once and in order; a rewind happens only on a transport NAK whose `expected` is ahead of the last rewind point (count rewinds and check against a log of NAKs); resend after exactly 50 ms of silence (fake clock).
  - [x] 8.3 Invariants after every step: `unacked ≤ 64`; QUEUE_EVENTs in flight ≤ `credit` computed from the last STATUS the session saw (never more than `free`); the controller's queue never exceeds its capacity (so `NAK FULL` only appears with a lossy STATUS path, and is recovered); nothing is sent between STOP and its ACK; nothing but HELLO (and the lone `0x00`) before INFO.
  - [x] 8.4 Semantic NAK (C.8): `ORDER`, `BAD_STATE`, `ABORTED`, `NOT_READY`, `BUSY`, `NO_LINK` injected on a QUEUE_EVENT and on START → `PieceAbandoned(Nak(reason))`, a STOP on the wire, nothing else until its ACK, then idle and a new piece works; on SET_STATE → `Refused`, no STOP.
  - [x] 8.5 Reboot (C.9): `controller.reboot()` mid-piece → the next app frame gets `NAK VERSION` → `ControllerRebooted`, HELLO again (with a lone `00`), reconnected, `PieceAbandoned(ControllerRebooted)`; a NAK VERSION before INFO is not a reboot.
  - [x] 8.6 PING (C.10): DIRECT with `app_timeout_ms` 1000 → PING every 250 ms of idle (exact fake-clock times), reset by each `set_state`; 400 → 100 ms; no PING in IDLE, PLAYING or after STOP; the fake controller never trips its app timeout during 10 s of idle DIRECT.
  - [x] 8.7 Unknown and recovery (C.11): EVENT `0xEE` → `ControllerEvent(Unknown(0xEE))`, session still connected and streaming; NAK reason `0x7E` → `UnknownNak`, no rewind, no STOP; `LINK_LOST` mid-piece → `PieceAbandoned(LinkLost)`, STOP, lone `00` before it; `abort_count` rising without EVENT → `PieceAbandoned(ControllerAborted)`; `link_ok` 0 → `start` and `set_state` return `NoLink`; local refusals of Task 4.1/4.2 (each one); `PLAYBACK_DONE` → event + STOP.
  - [x] 8.8 A short property test (proptest, already a dev-dependency) of the send buffer: random interleavings of `queue_event`, ACK/NAK/STATUS with random seqs (including stale and out-of-window), timeouts: no panic, invariants of 8.3 hold.
  - [x] 8.9 Red proof (record in the Dev Agent Record): break the rewind rule (rewind on every NAK) and the credit rule (ignore credit) one at a time and show C.7 failing; revert.
- [x] **Task 9: Workspace, CI, docs**
  - [x] 9.1 `app/Cargo.toml` members `["core", "sim"]`. `ocb-sim` depends on `ocb-core` (path) and nothing else at runtime; dev-dependency `proptest = "=1.12.0"` if 8.8 lives there. `publish = false`. The app CI runs `cargo test --workspace` already; check that `cargo deny` still passes.
  - [x] 9.2 The purity gate (C.4) must still pass on `app/core/src/link/` (see "Purity gate words" below).
  - [x] 9.3 `docs/test-plan.md`: rows C.6 to C.11 in the "App core tests" section.
  - [x] 9.4 `docs/link-protocol.md`: a short section "Rules the app link applies (Story 3.3)" listing D3–D9 as concrete app behaviour (no wire change: no message, NAK reason, EVENT, constant or `PROTO_VERSION` change, vectors unchanged).
  - [x] 9.5 `app/README.md`: mention `link` and `sim/`.

### Review Findings

Code review 2026-11-24 (gds-code-review: Blind Hunter, Edge Case Hunter, Acceptance Auditor; the Edge Case Hunter and the Auditor reproduced their main findings with scratch tests). The owner's rule: IMPORTANT findings fixed, MINOR fixed when cheap, otherwise recorded here. Every fix has a test in `app/sim/tests/regress.rs` (C.12); the first nine of those fail on the code before the fixes (checked by restoring the old `link/mod.rs` and `link/send.rs`).

- [x] [Review][Patch][IMPORTANT] **I-1 STOP could take the seq of a frame in flight** (all three; reproduced) — with more than 256 frames pushed, `next_free_seq` counted unsent frames, so an old ACK was taken as STOP's ACK: `Stopped` while the controller never stopped, and later frames lost silently. Fixed by giving seqs at first send (I-2): STOP takes the seq after the last frame sent. [app/core/src/link/send.rs, mod.rs]
- [x] [Review][Patch][IMPORTANT] **I-2 A QUEUE_EVENT waiting for credit blocked START, SET_STATE and PING** (blind+edge; reproduced) — a piece longer than the controller's queue never started (START queued behind events with no credit), and in DIRECT the gamepad froze until the app timeout fired. Fixed: QUEUE_EVENTs wait in their own queue; control frames go first; START waits only for the events queued before it or for the credit to run out. [send.rs]
- [x] [Review][Patch][IMPORTANT] **I-3 `poll_timeout` stayed in the past** (blind+edge) — a PING that could not be pushed kept `ping_due` behind now (busy loop for an adapter that sleeps on it). Fixed: PING is due only when the window has room and no control frame waits; `poll_transmit` no longer runs the timers (the adapter calls `handle_timeout` at `poll_timeout`). [mod.rs]
- [x] [Review][Patch][IMPORTANT] **I-4 `abort_rise_expected` could get stuck** (all three; reproduced) — an abort EVENT before the first STATUS left the flag set and the next abort without EVENT (app timeout) was ignored: no STOP, gamepad dead. Fixed: the EVENT advances the abort baseline by one; a STATUS behind the baseline only resyncs. [mod.rs]
- [x] [Review][Patch][IMPORTANT] **I-5 A lost semantic NAK on START went unnoticed** (all three; reproduced) — the next cumulative ACK covered the refused START; the app believed it was playing. Fixed with AD-19: once a STATUS covers START, a mode other than PLAYING with no abort rise abandons the piece (`AbandonReason::NotPlaying`). [mod.rs, events.rs]
- [x] [Review][Patch][IMPORTANT] **I-6 A reboot seen only in STATUS was taken for an abort** (blind+edge; reproduced) — `abort_count` dropping to 0 read as a rise, STOP acknowledged by a closed controller session, next piece died on `NAK VERSION`. Fixed: a STATUS with `abort_count` 0 and `last_event` NONE after a non-fresh one is a reboot (HELLO again); it is not applied to the send buffer. [mod.rs]
- [x] [Review][Patch][IMPORTANT] **I-7 No lone `0x00` after a wire fault** (auditor+edge) — Story 2.7 duty. Fixed: after a rewinding transport NAK, a 50 ms resend, and a link loss learned in any phase or from STATUS. [mod.rs]
- [x] [Review][Patch][IMPORTANT] **I-8 The simulator could not catch these bugs** (all three) — the driver read the session's own phase and `unacked`, called the timers every 1 ms, reordered only inside one step, and the fake controller was kinder than the firmware. Fixed: invariants from the wire only (window from confirmations, resends must be unconfirmed frames, credit from delivered STATUS, phase rules from HELLO/INFO/STOP/ACK on the wire), strict timers with a busy-loop check, reorder across steps (3 ms hold), and a fake controller with the firmware's TX priority, ACK and STATUS replacement, NAK and EVENT rings, replies discarded on link loss and the 8000 µs LATE fallback; knobs `lose_next_nak` / `lose_next_event`. [app/sim/src/run.rs, controller.rs, channel.rs]
- [x] [Review][Patch][IMPORTANT] **I-9 No lossy test of START, STOP, semantic NAKs; impairment matrix narrower than Task 8.2** (auditor+blind) — C.7 now runs every impairment at 1, 5 and 10 %; C.12 adds a lossy long piece with START and a lossy STOP. [app/sim/tests/gbn.rs, regress.rs]
- [x] [Review][Patch][MINOR] **M-1 Refused SET_STATE left DIRECT on** — `direct` cleared on a refused SET_STATE. [mod.rs]
- [x] [Review][Patch][MINOR] **M-2 Stale `last_event` reported a later abort as `PlaybackDone`** (blind+edge) — STATUS reports done only when `last_event` has just become `PLAYBACK_DONE`. [mod.rs]
- [x] [Review][Patch][MINOR] **M-3 `HISTORY` 256 let a seq map both ways** — now 64 (forward ≤ 64 + backward ≤ 64 < 256); unit test `a_seq_maps_at_most_once`. [send.rs]
- [x] [Review][Patch][MINOR] **M-4 Unchecked time additions** — `saturating_add` everywhere. [mod.rs, send.rs]
- [x] [Review][Patch][MINOR] **M-5 No PING with `app_timeout_ms < 4`** — interval computed in µs. [mod.rs]
- [x] [Review][Patch][MINOR] **M-6 Unbounded event queue** — consecutive unread STATUS events coalesce to the newest. [mod.rs]
- [x] [Review][Patch][MINOR] **M-7 Weak tests** — PING refusal tested; C.9 checks the `00` right before HELLO; the property test asserts it really sends QUEUE_EVENTs (not vacuous); Malformed bodies counted (`Session::malformed`). [regress.rs, props.rs, mod.rs]
- [ ] [Review][Minor][recorded] **M-8 A lost semantic NAK on a QUEUE_EVENT can still drop that event silently** — after the fixes the only semantic NAKs a QUEUE_EVENT can get are `ABORTED` (caught by the abort rise) and `ORDER` / `BAD_STATE`, which the app refuses locally before sending; a refusal of a valid event is therefore not expected. Detecting a missing event from STATUS `free` would need the controller's queue fill per event; not done.
- [ ] [Review][Minor][recorded] **M-9 An abort EVENT delayed behind STOP's ACK** (reordering only) can trigger one more STOP after `Stopped`; harmless (an idle STOP), and it can only abandon a piece started inside that few-ms window. The firmware sends EVENT before every other reply, so this needs the channel's reorder.
- [ ] [Review][Minor][recorded] **M-10 An idle session learns of a reboot only from STATUS or its next frame** — a reboot right after connecting (both STATUS "fresh") is seen at the next app frame (`NAK VERSION`).
- [ ] [Review][Minor][recorded] **M-11 A delayed transport NAK after a timeout resend rewinds once more** — a duplicate burst of at most 64 frames; correctness unaffected (go-back-N).
- [ ] [Review][Minor][recorded] **M-12 API differences from the story's sketch** — `PieceAbandoned` has no `seq`; `credit()` is `Option<i64>` (it can be negative right after a STATUS); `Sim::run` became `run_until` / `run_for` — recorded as D18–D20.

## Dev Notes

### What exists (read before writing)

- `app/core/src/protocol/` (Story 3.2): `encode_frame`, `decode_frame`, `Splitter` (`RxSide::App`), `Message` and every body struct, `NakReason`/`EventCode`/`Mode`/`HidProfile` with `Unknown(u8)`, `is_transport()`/`is_semantic()`, `State`, `NEUTRAL`, `consts::*` (`ACK_TIMEOUT_MS` 50, `ACK_WINDOW` 64, `HELLO_RESEND_MS` 100, `APP_PING_DIVISOR` 4, `APP_TIMEOUT_MS_DEFAULT`, `T_US_LIMIT`, `STATE_FLAG_END`, `STATE_RESERVED_MASK`, `HAT_NEUTRAL`, `STATUS_LINK_OK`, `STATUS_CONSOLE_READY`). Reuse them; never re-type a wire number (C.1 checks only `consts`).
- `app/core/src/version.rs`: `APP_VERSION_TRIPLE` for the recommended-update check.
- `app/core/tests/purity.rs` (C.4): scans `app/core/src/` on a token stream. See "Purity gate words".
- The reference behaviour of the controller is `docs/link-protocol.md` sections "Reliability (AD-6)", "Session start and versioning (AD-7)", "Controller modes (AD-8)", "Rules the controller core applies (Story 2.4)", "Rules the controller link applies (Story 2.6)" and "(Story 2.7)", "Controller → app reporting (AD-19)". Each has a "What the app must do" paragraph written for this story; every one of those duties is a task above.
- The bench probe `firmware/tools/link_probe.py` class `GbnSender` (Story 2.6) is a working AD-6 sender for QUEUE_EVENTs only, proven on silicon in H.5 / H.10. Read it for the window, `_count_after` (seq → index within `[base−1, base+64]`), the "NAK `expected` also acknowledges" rule and the rewind point. Do **not** port its `Pacer`/`max_inflight`/hold logic: that was for the 2wiCC_Comms 1 KB ring (Story 2.6 D30–D36); the own bridge (Story 3.1) is store-and-forward with USB backpressure (AD-4), so flow control is the serial adapter's job in Story 3.4.

### API shape (binding where it says "must")

```rust
pub struct SessionConfig {
    pub hello_seq: u8,          // seq used by every HELLO of one open (D3)
    pub app_version: [u8; 3],   // default version::APP_VERSION_TRIPLE
}

pub struct Session { /* state, send buffer, splitter, timers, out queues */ }

impl Session {
    pub fn new(cfg: SessionConfig) -> Self;
    pub fn open(&mut self, now_us: u64);
    pub fn handle_rx(&mut self, bytes: &[u8], now_us: u64);
    pub fn handle_timeout(&mut self, now_us: u64);
    pub fn poll_transmit(&mut self, now_us: u64) -> Option<Vec<u8>>;
    pub fn poll_event(&mut self) -> Option<LinkEvent>;
    pub fn poll_timeout(&self) -> Option<u64>;

    pub fn queue_event(&mut self, t_us: u32, state: State) -> Result<(), CommandError>;
    pub fn start(&mut self, lead_us: u32) -> Result<(), CommandError>;
    pub fn set_state(&mut self, state: State) -> Result<(), CommandError>;
    pub fn stop(&mut self) -> Result<(), CommandError>;

    pub fn info(&self) -> Option<&Info>;
    pub fn status(&self) -> Option<&StatusView>;
    pub fn phase(&self) -> Phase;      // Closed, Opening, Connected, Stopping, Incompatible
    pub fn credit(&self) -> Option<u32>;
    pub fn unacked(&self) -> usize;
}
```

- **Must:** no I/O, no clock, no threads (C.4). Deterministic: the same inputs and times give the same outputs.
- **Must:** a malformed or unexpected frame never panics and never ends the session; only `open()` / `NAK VERSION` / an INFO change the phase.
- `poll_transmit` takes `now_us` because sending a frame starts its 50 ms ACK timer. The adapter (Story 3.4) calls `poll_transmit` until `None`, writes the bytes, then sleeps until `poll_timeout` or the next read.
- Names may change if a better one appears (record it), but keep the shape: commands + `handle_*` in, `poll_*` out (the quinn-proto / sans-IO style).

### LinkEvent (typed, never a fatal error)

`Connected(Info)`, `RequiredUpdate { controller_proto, app_proto, info }`, `RecommendedUpdate { fw_ver, bridge_ver, app_ver, fw_differs, bridge_differs }`, `ControllerRebooted`, `Status(StatusView)`, `ControllerEvent(EventCode)`, `PlaybackDone`, `PieceAbandoned { reason: AbandonReason }` (`Nak(NakReason)`, `LinkLost`, `ControllerAborted`, `ControllerRebooted`), `Refused { reason: NakReason, ty: u8 }`, `UnknownNak { reason: u8, expected: u8, got: u8 }`, `Stopped` (STOP ACKed), `Malformed { ty: u8 }`.

### Decisions (made autonomously, 2026-11-24; the owner can overrule any of them)

- **D1. Session lives in `ocb-core::link`**, sans-IO, with `u64` µs time from the caller (AD-16: one serial worker will own the port and the session in Epic 4; Story 3.4's `ocb-dev` is that worker's first, CLI-only form).
- **D2. The fake controller is its own dev-only crate `ocb-sim` (`app/sim/`)**, not a `#[cfg(test)]` module of `ocb-core`: Story 3.4 needs it for the retro action #27 dry run of `ocb-dev` against a simulated controller, and it would break the core's purity gate (it is allowed to be a bigger, test-shaped piece of code). `ocb-core` must not depend on it; the session tests live in `app/sim/tests/`. It is never shipped (Epic 4's app binary will not depend on it).
- **D3. Every HELLO of one `open` uses the same seq** (`SessionConfig::hello_seq`, default 0). INFO carries no seq, so a single HELLO seq is the only way to know what `expected` the controller now has (`hello_seq + 1`). Same as the bench probe.
- **D4. A protocol mismatch stops the session** (`Incompatible`, no more HELLO): the controller answers every HELLO with INFO anyway and closes its session; resending forever would only spam. The UI (Epic 4) shows the required-update dialog; the session restarts only with `open()`.
- **D5. Recommended update compares exact triples** with the app version. The fallback `0.0.0-dev` app (triple `0.0.0`) against firmware `0.0.0` is "equal"; a dev app against tagged firmware reports "differs", which is accurate. `bridge_ver` `0.0.0` from INFO means "no HEARTBEAT seen" (AD-7) and is never reported as differing; a real bridge 0.0.0 is indistinguishable and harmless.
- **D6. No QUEUE_EVENT before the first STATUS of a session.** Credits need `STATUS.free`; the controller sends its first STATUS within 50 ms. The alternative (assume `INFO.queue_cap` free) is wrong after a reboot race or a previous piece. Non-queue frames do not wait.
- **D7. A semantic NAK on SET_STATE or PING does not send STOP.** AC3 names QUEUE_EVENT and START only; refusing a live gamepad state (`NOT_READY`, `NO_LINK`, `BUSY`) is reported as `Refused` and the UI decides.
- **D8. After `PLAYBACK_DONE` the session sends STOP** to clear the controller's abort latch (END applied sets `aborted`, AD-8), so the next piece or DIRECT input is not refused with `NAK ABORTED`.
- **D9. An unknown NAK reason (`0x18–0x7F` or `0x80–0xFF`) never rewinds and never abandons**: the app cannot know whether it is transport or semantic, so it only takes the cumulative information (`expected` retires older frames) and reports it. Unknown reasons in `0x04–0x0F` (future transport) are treated the same: the 50 ms resend repairs any loss.
- **D10. Channel timing.** The `Channel` delivers bytes with a fixed latency per direction; byte-accurate 460800-baud pacing is optional. The ACK timeout must still be exercised (a dropped frame must cost exactly one 50 ms resend).

### Purity gate words (C.4, Story 3.2 review fix I-1)

`app/core/src/link/` is scanned on a token stream. These identifiers are banned **anywhere, comments included**: `fs io net os thread spawn mpsc process Command File env Instant SystemTime stdin stdout stderr unsafe extern asm`; macros `print! println! eprint! eprintln! dbg! include! include_bytes! include_str! option_env!`; `std::`/`core::`/`alloc::` paths only to `fmt error collections vec string boxed borrow ops convert iter cmp num mem array slice str char hash marker default option result`. Watch prose: write "handles", "steps", not "process"; "the link", not "I/O" is fine (`I`/`O` are not banned). `std::collections::VecDeque` is allowed. Run `cargo test -p ocb-core --test purity` early.

### Testing requirements (rows for `docs/test-plan.md`)

| ID | Test | Pass criterion | Binds |
|---|---|---|---|
| C.6 | Session open and versions (Story 3.3, `app/sim/tests/`): first bytes `00` + HELLO, HELLO every 100 ms with the same seq and nothing else until INFO; INFO values exposed (non-default); mismatch → `RequiredUpdate`, HELLO stops; `fw_ver`/`bridge_ver` ≠ app → one `RecommendedUpdate`; zero `bridge_ver` not a difference | all pass | AD-7, AD-13 |
| C.7 | Go-back-N and credits through the fake controller and an impaired channel (drop, bit flip, duplicate, reorder, junk at 0–10 %, 3 seeds, capacity 512 and 5): every QUEUE_EVENT enqueued exactly once and in order; resend after 50 ms; rewind only on a transport NAK ahead of the last rewind point; invariants after every step (`unacked ≤ 64`, in-flight QUEUE_EVENTs ≤ credit, controller queue never over capacity, nothing sent between STOP and its ACK, only HELLO before INFO); `OCB_SIM_SEED` replays | all pass | AD-6, AD-15 |
| C.8 | Semantic NAKs: on QUEUE_EVENT and START each semantic reason → `PieceAbandoned(reason)` + STOP, silence until its ACK, a new piece then works; on SET_STATE → `Refused`, no STOP | all pass | AD-6 |
| C.9 | Controller reboot: mid-session `NAK VERSION` → `ControllerRebooted`, `00` + HELLO, reconnect, piece abandoned; `NAK VERSION` before INFO is not a reboot | all pass | AD-7 |
| C.10 | PING: DIRECT idle → PING every `app_timeout_ms/4` (1000 → 250 ms, 400 → 100 ms), reset by any app frame, none outside DIRECT; the fake controller's app timeout never trips in 10 s | all pass | AD-5, AD-7 |
| C.11 | Unknown codes and recovery: unknown EVENT / NAK reason → typed events, session goes on, no rewind or STOP for an unknown NAK; `LINK_LOST`, `abort_count` rise without EVENT → piece abandoned + STOP (+ lone `00` after `LINK_LOST`); `link_ok` 0 → `NoLink`; local refusals (order, `t_us ≥ 2^31`, hat, reserved flags, END on a pose, after END, START on an empty queue); `PLAYBACK_DONE` → event + STOP; send-buffer property test | all pass | AD-6, AD-8, AD-19 |

### Architecture compliance

- AD-6, AD-7, AD-8, AD-19 as listed; AD-15: the streamer never edits, drops or reorders events (an abandoned piece is dropped as a whole, by STOP).
- AD-16: the session is the worker's state; no thread here. AD-18: core purity (C.4).
- AD-14 / conventions: µs as integers, versions `[u8; 3]`, typed results, no wire constant outside `protocol::consts`.
- No wire change. If an AC turns out not to match the controller's documented behaviour, stop and ask the owner (epic instructions); do not rewrite ACs.

### File structure

```text
app/Cargo.toml                       UPDATE members += "sim"
app/Cargo.lock                       UPDATE
app/core/src/lib.rs                  UPDATE pub mod link
app/core/src/link/mod.rs             NEW  Session, config, phases
app/core/src/link/events.rs          NEW  LinkEvent, AbandonReason, StatusView, CommandError
app/core/src/link/send.rs            NEW  send buffer, window, credits, rewind (unit-testable)
app/sim/Cargo.toml                   NEW  ocb-sim (dev-only)
app/sim/src/lib.rs                   NEW
app/sim/src/controller.rs            NEW  FakeController
app/sim/src/channel.rs               NEW  impaired channel, xorshift PRNG
app/sim/src/run.rs                   NEW  Sim driver, invariant checks
app/sim/tests/{open,gbn,naks,reboot,ping,recovery}.rs   NEW
app/README.md                        UPDATE
docs/test-plan.md                    UPDATE C.6–C.11
docs/link-protocol.md                UPDATE "Rules the app link applies (Story 3.3)"
```

(The split of `link/` into files is a suggestion; keep `send.rs` separate so the window logic has its own unit tests.)

### Library / framework requirements

- No new runtime dependency in `ocb-core` (C.4 checks `[dependencies]` is empty). `ocb-sim` depends only on `ocb-core`; `proptest = "=1.12.0"` as a dev-dependency. No `rand`: a xorshift64* PRNG in `ocb-sim` is enough and keeps seeds stable across versions.
- Rust 1.100.0 (pinned). PATH on this Mac: `export PATH=/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH` (rustup proxies and cargo-deny).

### Previous story intelligence

- Story 3.2: decoders never panic; `Unknown(u8)` codes round-trip; `Message::decode(ty, body)`; `Splitter::push` gives `RxEvent::Dropped(err)` for every dropped run, `Frame` for a good one. Lints deny `unwrap`, `expect`, `panic`, indexing in library code (tests opt out per file). C.4 token rules (above).
- Story 3.2 review: tests that "can pass for the wrong reason" were the main finding class. Assert exact outcomes (exact times on the fake clock, exact frame sequences), and show the key tests failing once (Task 8.9).
- Epic 2: the bench probe's sender needed three bench rounds of fixes (H.5, H.12; Story 2.6 D30–D36, Story 2.7 D40–D42) because it was tested only at the bench. This story is the antidote: everything here is proven on the fake clock first. The probe's failure modes to cover in C.7: a soak-wide deadline (use per-piece completion, not wall time), unpaced bursts (the window), single-shot sends after a replug (every frame must be retried until ACKed or the piece is abandoned), a replug bounce read as the next unplug (the session must not double-count one `LINK_LOST`).
- Retro §6: mirror the two extra `NAK BAD_STATE` refusals (START on an empty queue, QUEUE_EVENT after END) and HELLO's version check — Tasks 4.1, 4.2 and 2.3.

### Git intelligence

Last merge: PR #31 (Story 3.2), commit `7b13971`. Pattern: story file + code + test-plan rows + docs in one PR, CI `app` (3 OS) and `firmware` green, D-entries in the story, review findings recorded in the story.

### Project context rules

No `project-context.md`. Repo content in English; chat in Italian. Never commit `_bmad/` or skills. Branch `story/3-3-sans-io-link-session`; the owner merges.

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 3.3]
- [Source: ARCHITECTURE-SPINE.md#AD-6, AD-7, AD-8, AD-13, AD-15, AD-16, AD-18, AD-19]
- [Source: docs/link-protocol.md#Reliability, Session start, Controller modes, Rules … (Story 2.4), (Story 2.6), (Story 2.7), Controller → app reporting]
- [Source: firmware/tools/link_probe.py#GbnSender]
- [Source: _bmad-output/implementation-artifacts/epic-2-retro-2026-11-24.md#3.1, #6]
- [Source: _bmad-output/implementation-artifacts/3-2-rust-workspace-and-protocol-codec-ocb-core.md#Review Findings, D-entries]

## Dev Agent Record

### Agent Model Used

Claude Opus 5.5 (`claude-opus-5-5`), gds-dev-story, 2026-11-24.

### Debug Log References

- `cargo test --workspace` (macOS arm64, rustc 1.100.0): **66 tests, all pass**. `ocb-core`: lib 11 (4 from Story 3.2 + 7 `link::send` unit tests), `props` 11, `purity` 4, `vectors` 14, `version` 1. `ocb-sim`: `gbn` 5, `naks` 4, `open` 4, `ping` 3, `props` 1, `reboot` 2, `recovery` 6. `gbn` takes about 1.8 s in a debug build.
- `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all --check`: clean. `cargo deny --locked check licenses`: `licenses ok` (no new crate). C.4 purity passes on `app/core/src/link/`. F.22 (`test_docs_constants.py`) still ok after the `docs/link-protocol.md` addition.
- C.7 sample (`OCB_SIM_VERBOSE=1 OCB_SIM_SEED=2`, capacity 5): clean run 2000 frames sent, 0 resends, 0 rewinds, 4.16 s of fake time; drop 10 %: 291 frames dropped up, 50 resends, 178 rewinds, 8.95 s; all impairments at 10 %: 577 dropped, 498 flipped, 534 junk, 510 duplicated, 513 reordered up, 480 resends, 554 rewinds, 32.0 s; every event enqueued exactly once, in order, no `NAK FULL`.
- Red proofs (Task 8.9), each reverted (`git status` clean on `send.rs` afterwards):
  - Credit rule removed (`next_to_send` ignores credit): C.7 fails at once, e.g. `t=104300 µs: 15 QUEUE_EVENTs after last_seq 53 with free 0`.
  - Rewind rule removed (rewind on every transport NAK): the first C.7 matrix did **not** catch it (5 % duplicates, bound loosened by resends). Added `c7_a_duplicated_nak_rewinds_once` (every controller frame duplicated): with the rule removed it fails with `seed 1: 198 rewinds for 99 NAK points (198 NAKs delivered)`; with the rule, it passes.
- Test-harness bugs found and fixed while writing the tests (not session bugs): the driver's credit check took the sync seq for `last_seq` after a 256-frame wrap; the NAK-point count collapsed `expected` values across wraps (now absolute frame positions); the unknown-code tests ended before the injected frame arrived.

### Decisions made during implementation (D1 to D10 followed; additions below)

- **D11. No `CommandError::Aborted`.** Every abandon sends STOP at once, so a command after an abandon gets `Stopping` until the ACK, then works. A separate "aborted" refusal would never be reachable.
- **D12. `CommandError::Busy` mirrors `NAK BUSY`**: START twice and SET_STATE while a piece is started are refused locally (the controller would NAK BUSY in PLAYING, and a semantic NAK on START abandons the piece).
- **D13. STOP after any controller abort, even when idle.** An abort rise (EVENT or STATUS) sends STOP even if no piece was active, so the controller's latch never stays set; `PieceAbandoned` is reported only when a piece or DIRECT input was active. An `abort_rise_expected` flag keeps one abort from being handled twice (EVENT then STATUS).
- **D14. `START` and `SET_STATE` need a STATUS with `link_ok` 1**; before the first STATUS of a session they return `NoLink` (the first STATUS comes within 50 ms; QUEUE_EVENTs wait for it anyway, D6).
- **D15. Fake controller test knob `sink_every_us`**: outside PLAYING it pops one queued event every N µs, so C.7 can stream 2000 events through a capacity of 5 without START and without the underrun rule ending the run.
- **D16. The driver checks invariants independently** of the session: it decodes every chunk the session sends, numbers new frames itself, and checks the credit rule against the session's last STATUS; the session's own counters are never trusted for C.7.
- **D17. `SendBuffer` is public** (`ocb_core::link::SendBuffer`) so the property test lives with the other session tests in `ocb-sim`; the session is the only intended user.
- **D18. `PieceAbandoned` carries only the reason**: the refused seq is in the NAK and means nothing to the UI (review M-12).
- **D19. `credit()` is `Option<i64>`**: right after a STATUS that saw a fuller queue the credit is negative, and hiding that as 0 would hide a bug (review M-12).
- **D20. `Sim::run_until(max, done)` and `run_for(us)`** instead of one `run(…)`: tests read better with a stop condition (review M-12).
- **D21. Code review fixes (2026-11-24)**: I-1 to I-9 and M-1 to M-7 fixed, M-8 to M-12 recorded (Review Findings above). The send buffer gives seqs at first send, with separate queues for QUEUE_EVENTs and control frames; `poll_transmit` no longer runs timers; `AbandonReason::NotPlaying` is new. Tests after the fixes: 83 (`ocb-core` 44: lib 14, props 11, purity 4, vectors 14, version 1; `ocb-sim` 39: gbn 5, naks 4, open 4, ping 3, props 2, reboot 2, recovery 6, regress 13).

### Completion Notes List

- AC1: `Session::open` emits a lone `00` then HELLO (fixed seq) every 100 ms until INFO; `info()` exposes INFO's values; `RequiredUpdate` on a `proto_ver` mismatch (HELLO stops), `RecommendedUpdate` on a `fw_ver` / `bridge_ver` difference (C.6).
- AC2: go-back-N with cumulative ACK (also from NAK `expected` and STATUS `last_seq`), 50 ms resend, rewind only ahead of the last rewind point, credits `free − QUEUE_EVENTs sent after last_seq`, window 64; proven through the fake controller and impaired channels (C.7).
- AC3: semantic NAK on QUEUE_EVENT / START → `PieceAbandoned(Nak(reason))` + STOP, silence until its ACK (C.8).
- AC4: mid-session `NAK VERSION` → `ControllerRebooted`, lone `00` + HELLO, reconnect (C.9).
- AC5: PING after `app_timeout_ms/4` of silence in DIRECT (C.10).
- AC6: unknown EVENT codes and NAK reasons are typed events; the session goes on (C.11).
- No wire change; no vector edited; no AC mismatch with the documented controller behaviour.

### File List

- `app/Cargo.toml` (modified: members += `sim`)
- `app/Cargo.lock` (modified)
- `app/README.md` (modified)
- `app/core/src/lib.rs` (modified)
- `app/core/src/link/mod.rs` (new)
- `app/core/src/link/events.rs` (new)
- `app/core/src/link/send.rs` (new)
- `app/sim/Cargo.toml` (new)
- `app/sim/src/lib.rs` (new)
- `app/sim/src/controller.rs` (new)
- `app/sim/src/channel.rs` (new)
- `app/sim/src/run.rs` (new)
- `app/sim/tests/common/mod.rs` (new)
- `app/sim/tests/open.rs`, `gbn.rs`, `naks.rs`, `reboot.rs`, `ping.rs`, `recovery.rs`, `props.rs`, `regress.rs` (new)
- `docs/test-plan.md` (modified: C.6 to C.11)
- `docs/link-protocol.md` (modified: "Rules the app link applies (Story 3.3)")
- `_bmad-output/implementation-artifacts/sprint-status.yaml` (modified)
- `_bmad-output/implementation-artifacts/3-3-sans-io-link-session.md` (this file)

## Change Log

- 2026-11-24: Story 3.3 implemented (`ocb-core::link` sans-IO session, `ocb-sim` fake controller and impaired channel, C.6 to C.11). Status → review.
- 2026-11-24: Code review: 9 IMPORTANT and 7 MINOR findings fixed, 5 MINOR recorded (Review Findings); C.12 added. CI: `app` run 45372030897 green before the review.
