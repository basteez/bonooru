---
baseline_commit: 1f206f78c86c158d47269bb371a0609126254939
---

# Story 2.7: Controller link watchdog

Status: review

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As the owner,
I want the controller to go neutral on its own when the link or the app disappears,
so that no input can stay pressed on my console (NFR-05).

## Acceptance Criteria

Copied verbatim from `_bmad-output/planning-artifacts/epics.md`, Story 2.7. These are the definition of done. The marking in brackets is added by this story file: **[HOST]** = verifiable by `host-tests` (ctest, fake clock, ASan and UBSan), **[HW-BENCH]** = needs the RP2040-Zero boards on the bench (controller Zero, second Zero running 2wiCC_Comms as the transparent peer, a multimeter), **never the Switch 2**: nothing in this story needs the console (D20). The console-side proof "no input stays pressed on the Switch within 100 ms" is Story 3.5 / A.3 and Story 2.5 / H.2, not here.

**AC1.** **[HOST + HW-BENCH]**
**Given** host tests with a fake clock
**When** no valid `HEARTBEAT` arrives within `OCB_LINK_TIMEOUT_MS` (default 80, ≤ 100) or a UART break is seen
**Then** the controller aborts to IDLE (NEUTRAL, latch set), sends `EVENT LINK_LOST`, clears `link_ok`, and releases TX until RX is idle-high again (AD-5, AD-20)

**AC2.** **[HOST; HW-BENCH only on the bench build of D17]**
**Given** DIRECT mode
**When** no app frame arrives for `app_timeout_ms` (default 1000) or `HEARTBEAT.flags` shows a DTR falling edge
**Then** the controller aborts to IDLE
**And** neither rule applies in PLAYING, where the underrun rule covers a dead app

**AC3.** **[HOST + HW-BENCH]**
**Given** the link is down
**When** `START` or `SET_STATE` arrives
**Then** it gets `NAK NO_LINK`

### Reading notes on the AC (what the story file adds, none of it changes the AC)

- **Three rules, one new NAK, one new sequence.** The rules are pure C and are proven on the PC with a fake clock. What the PC cannot prove is the silicon: a real break on the pad, the TX pad going high-Z after the EVENT left, and the board's behaviour with a real peer. Those are owner bench tests H.8 to H.12 (Task 10), each started by one probe command (D16).
- **"The link is down"** (AC3) needs a definition the earlier stories left open: before the first HEARTBEAT there has never been a link, so it is down (D3). Story 2.6 D10 set `link_ok` on the first heartbeat and never cleared it; this story is the only place that clears it.
- **"releases TX until RX is idle-high again"** is the 2.6 TX gate (`ocb_txgate_release()`, reserved for this story) followed by the existing 1 ms idle-high rule. The EVENT must leave **before** the release (D6).
- **AD-20 amendment.** Story 2.6 D25 found that AD-20 as written deadlocks when both boards are powered. This story applies the pending amendment to the spine (D14, Task 8.1); it is the story that implements the controller half of the rule ("releases TX").
- **No wire change.** `NO_LINK` (0x15) and `LINK_LOST` (0x02) already exist in `consts.h` and the vectors; STATUS debug byte 2 (`watchdog_trips`) already exists and is filled now. No new message, no `PROTO_VERSION` bump, vectors not edited (D19).

## Scope boundaries (what is NOT in this story)

| Not here | Where it is |
|---|---|
| The bridge's HEARTBEAT sender (20 ms alarm), the bridge's TX probe pulses of D25, F.7 and F.8 | Story 3.1 (this story's probe tool emulates the heartbeat on the bench only, D15) |
| USB, the real `ready()`, `build_report`, the measured `report_period_us` | Story 2.3 |
| The real core 1 super-loop, hardware watchdog reboot, core-stall abort, H.2 | Story 2.5 (this story keeps calling `ocb_ctrl_link_step()`; the core-stall abort cause stays 2.5's) |
| The console-side proof (neutral on the Switch within 100 ms, 20 unplugs, A.3), `console_ready` measurements H.3, the STATUS staleness rule | Stories 3.5 and 2.3; Epic 4 |
| Rust sans-IO link and the app's reaction to `LINK_LOST` / `NAK NO_LINK` | Story 3.3 (this story writes the rules it must follow into `docs/link-protocol.md`) |
| Setting the final `app_timeout_ms` from the 30-minute soak (W.1), 0 false trips over 30 minutes of playback under load (W.2) | Epic 7 (Story 7.4); this story only runs short soaks (H.10) |
| The both-boards-powered case of H.1 (D25's bridge probe) | Story 3.1 |

## Tasks / Subtasks

Tasks 1 to 9 are for the dev agent and need **no hardware**; they end with everything green on the host and the ARM presets compiling. Task 10 is the **owner's bench session**. The dev agent does not commit or push; the owner commits and pushes, and CI is verified afterwards by the owner's push (as in Stories 2.1, 2.2, 2.4 and 2.6). Keep Completion Notes honest: anything CI-only is "unverified until pushed", anything on hardware is "unverified on hardware" until Task 10 is recorded.

Local build, as in Story 2.6: from `firmware/`, `export OCB_PICOTOOL_DIR=firmware/.tools/picotool-2.3.1 OCB_ARM_TOOLCHAIN=firmware/.tools/arm-gnu-toolchain-15.2.Rel1` (paths relative to the repo root; use absolute paths if the preset needs them), then `cmake --preset host-tests && cmake --build --preset host-tests && ctest --preset host-tests`, and `cmake --preset fw-release && cmake --build --preset fw-release` (same for `fw-debug`, `fw-debug-bench`).

- [x] **Task 1: `NO_LINK` in the core** (AC: #3; D11)
  - [x] 1.1 `firmware/controller/core/include/ocb_core/core.h` (UPDATE): add `bool link_ok;` to `ocb_core_env` (third field, comment: the bridge link is up; false makes `START` and `SET_STATE` get `NAK NO_LINK`). Positive polarity on purpose: a zero-initialised env means "no link", which is the safe default (D11).
  - [x] 1.2 `modes.c` (UPDATE): in `handle_set_state` and `handle_start` insert `else if (!env->link_ok) reply_nak(out, OCB_NAK_NO_LINK);` **after** the `aborted` check and **before** the `PLAYING` (`BUSY`) check. Precedence becomes `ABORTED` → `NO_LINK` → `BUSY` → `NOT_READY` → `BAD_STATE` → `ORDER` → `FULL` (the documented slot, 2.4 D3). `QUEUE_EVENT`, `PING`, `STOP`, `HELLO` and unassigned types are not touched. Update the header comment of `ocb_core_env` and the "NAK precedence" comment; no other core change.
  - [x] 1.3 Existing core tests: `tests/core_fixture.h` `h_env()` and the literal `{0u, true}` in `test_core_edge.c` must set `link_ok = true` by default (a new harness field `link_ok`, default true) so the 2.4 matrices keep meaning what they meant. Extend F.3 (`test_core_modes.c`) with a `NO_LINK` row for `SET_STATE` and `START` in IDLE, DIRECT, PLAYING (link down: IDLE and DIRECT give `NO_LINK`; PLAYING with the link down gives `NO_LINK` too, because the check precedes `BUSY`, D11), the precedence pairs (`ABORTED` over `NO_LINK`, `NO_LINK` over `BUSY` and over `NOT_READY` and over `BAD_STATE`), and "`NO_LINK` is semantic: `expected = seq + 1`, no effect on mode, queue or latch". F.12 (`test_core_edge.c`): `QUEUE_EVENT`, `PING`, `STOP`, `HELLO` are unchanged with the link down. `check-core-pure.sh` stays green.

- [x] **Task 2: the watchdog rules as a pure module** (AC: #1, #2; D1 to D4, D9, D10)
  - [x] 2.1 `firmware/controller/link/include/ocb_link/watchdog.h` and `watchdog.c` (NEW, inside `ocb_link`, so the existing purity gate F.18 covers them; no gate change, no new gate). SPDX `MIT`, same warning flags as the library, allowed includes only `<stdint.h> <stdbool.h> <stddef.h>` and `"ocb_proto/..."`, `"ocb_core/..."`, `"ocb_link/..."`. No heap, no recursion, no global state, no clock read, **no `/` or `%` on `uint64_t`** (Cortex-M0+ has no 64-bit divide; the limits are converted to microseconds once, at init, with a `uint64_t` multiply).
  - [x] 2.2 State (suggested): `ocb_wd { bool armed; uint64_t link_limit_us, app_limit_us; uint64_t last_hb_us; bool dtr_prev; uint32_t breaks_seen; uint8_t trips; }` and a `ocb_wd_init(wd, link_timeout_ms, app_timeout_ms)`. `link_timeout_ms` 0 selects `OCB_LINK_TIMEOUT_MS` (80) and anything above `OCB_LINK_TIMEOUT_MAX_MS` (100) is clamped to 100 (NFR-05 cap, AC1 "≤ 100"); `app_timeout_ms` 0 selects `OCB_APP_TIMEOUT_MS_DEFAULT` (1000). Both effective values are readable (`ocb_wd_link_timeout_ms()`, `ocb_wd_app_timeout_ms()`) because INFO reports the effective `app_timeout_ms` (D9).
  - [x] 2.3 One wrap-safe helper, used by every comparison and nothing else: `static OCB_CORE_RAMFUNC uint64_t ocb_wd_elapsed_us(uint64_t now_us, uint64_t then_us)` returning `now_us - then_us` when `now_us >= then_us`, else **0** (a clock that steps backwards, or a stale timestamp, never produces a huge elapsed value and never a false trip). Every deadline test is `elapsed > limit` (strictly greater, D4); the code never writes `now > then + limit` (that form overflows near the top of the range, D1).
  - [x] 2.4 Pure decision functions (names a suggestion, behaviour is not): `ocb_wd_on_heartbeat(wd, now_us, dtr, mode)` returns whether a DTR falling edge fired in DIRECT (D10) and arms the watchdog; `ocb_wd_check(wd, now_us, mode, last_app_frame_us, have_app_frame, breaks_total)` returns a verdict bit set `OCB_WD_LINK_TIMEOUT | OCB_WD_BREAK | OCB_WD_APP_TIMEOUT`, evaluated in that order of priority and **at most one loss episode per arming** (after a loss the watchdog is disarmed until the next heartbeat, D3, D7). `ocb_wd_note_loss(wd)` disarms and resets `dtr_prev`. `trips` increments once per verdict that fired (D12).
  - [x] 2.5 Hot functions carry `OCB_CORE_RAMFUNC` (the macro of `ocb_core/ramfunc.h`; do not invent another).

- [x] **Task 3: wire the watchdog into `ocb_link`** (AC: #1, #2, #3; D3 to D12, D23)
  - [x] 3.1 `link.h` / `link.c` (UPDATE): `ocb_link_cfg` gains `uint16_t link_timeout_ms` (the glue passes `OCB_LINK_TIMEOUT_MS`; tests pass anything). `ocb_link_init` runs `ocb_wd_init` and stores the **effective** `app_timeout_ms` back into `cfg.app_timeout_ms`, so INFO and the rule always agree. `make_env` passes `link_ok` to the core (Task 1).
  - [x] 3.2 `handle_bridge` (heartbeat): keep the 2.6 behaviour (store `bridge_ver`, flags, `last_heartbeat_us`, count) and add: `link_ok = true` (re-arms after a loss, D7), `ocb_wd_on_heartbeat(...)`; when it reports the DTR falling edge in DIRECT, call `ocb_core_abort(core, OCB_ABORT_APP_TIMEOUT, &out)` (no EVENT, D10) and count a trip. Only bit 0 of `HEARTBEAT.flags` is read; bits 1 to 7 are ignored. Heartbeats never reach the core, are never ACKed or NAKed (unchanged).
  - [x] 3.3 `note_app_frame` keeps being the single place where the app-frame time is recorded (admission step 1, D9): any valid app frame (0x01..0x3F) of any type, seq, session state or outcome counts, including stale frames and frames that get a NAK. HEARTBEATs never count.
  - [x] 3.4 `ocb_link_poll` order (D23): USB mailbox and `ready` aborts (2.6), **then the watchdog check**, **then** `ocb_core_tick`. A dead link therefore never applies an event that is due in the same pass. On `OCB_WD_LINK_TIMEOUT` or `OCB_WD_BREAK` (armed): `ocb_core_abort(core, OCB_ABORT_LINK_LOST, &out)`, queue `out.event` through the event ring (the core returns `LINK_LOST` only when the latch was not already set; an already-latched core gives no second event, AD-8), `link_ok = false`, `ocb_wd_note_loss`, start the TX release sequence (3.6). On `OCB_WD_APP_TIMEOUT`: `ocb_core_abort(core, OCB_ABORT_APP_TIMEOUT, &out)` (no EVENT; the app learns from STATUS `mode` and `abort_count`, AD-8), count a trip; `link_ok` is unchanged.
  - [x] 3.5 Break detection (D5): a break is a data-register entry with `BE` set (`cnt.breaks` already counts them; FE, PE, OE alone are not breaks). The watchdog compares `ocb_link_breaks()` with `breaks_seen` at each poll. A break seen **while armed** is a link loss (3.4). A break seen **while not armed** (no heartbeat yet, or already lost) only starts the TX release (3.6): there is no link to lose and nothing to abort, so a boot-time break (H.4 read `rx_be = 1` after power-up) never latches the core (D3, D8).
  - [x] 3.6 TX release sequence (D6, D23). New fields in `ocb_link`: `release_pending`, `release_deadline_us`. New constant in `config.h`: `OCB_LINK_RELEASE_WAIT_US` 5000 (local, not a wire value). While `release_pending`: `ocb_link_tx_next` returns **only the EVENT ring** (`LINK_LOST` is among the first because the abort has just queued it; anything older goes first) and discards ACK, NAK, INFO and STATUS (counted in `tx_dropped`, with the 2.6 D27 budget rule). New API `void ocb_link_tx_settled(ocb_link *link, uint64_t now_us, bool tx_hw_idle)`: called by the glue every pass after it pumped TX; when `release_pending` and the EVENT ring is empty and `tx_hw_idle`, **or** `now_us` has passed `release_deadline_us` (elapsed `>`, wrap-safe helper), it calls `ocb_txgate_release()` and clears `release_pending`. If the gate is already closed (a break at boot, a second break) the sequence completes at once. A break or loss that arrives while a release is pending does not extend the deadline.
  - [x] 3.7 STATUS (`tx.c`, UPDATE): `link_ok` bit 0 follows `link->link_ok` (already so); debug extra byte 2 `watchdog_trips` is now `wd.trips` (u8, wraps; layout and the 21-byte block unchanged, D19). Update the comment that says "Story 2.7".
  - [x] 3.8 The hooks of 2.6 D10: `ocb_link_set_link_ok()` is no longer needed by anything outside the library: remove it from the public API if no test depends on it, otherwise keep it with a comment "test only". `ocb_link_breaks()`, `ocb_link_last_heartbeat_us()`, `ocb_link_link_ok()` stay (probe shim, tests). Replace the `link.h` "placeholder until Story 2.7" comment by the real rule.
  - [x] 3.9 Do not change `ocb_proto`, `consts.h`, the vectors or `ocb_core` beyond Task 1.

- [x] **Task 4: host tests** (AC: #1, #2, #3; test F.21 `test_link_watchdog`, updates to F.2, F.3, F.12, F.16, F.17)
  - [x] 4.1 `firmware/tests/link_fixture.h` (UPDATE): `lh` gains `auto_hb` (default **true**), `hb_period_us` (20 000), `last_hb_sent`, and a helper `lh_advance(h, us)` that moves `h->now` in slices of at most `hb_period_us` and injects one `HEARTBEAT` (DTR bit per `h->dtr`) per period while the link has been brought up with `lh_link_up(h)`. `lh_init` leaves the link **down** (nothing sent) so session tests still start from the boot state; tests that need START / SET_STATE accepted call `lh_link_up`. Existing tests that move `h->now` directly across more than 60 ms (the `ready` 100 ms stall cases, the STATUS cadence, the fuzz and lossy-channel models) must call `lh_advance` or set `h->auto_hb`; run the suite once after Task 3 and fix every test that now sees an unexpected `LINK_LOST`: each such failure names the test that moves the clock without a heartbeat. The lossy-channel model sends its heartbeats **unimpaired** (the impairments apply to app frames: heartbeat loss is the subject of F.21, go-back-N is the subject of F.17).
  - [x] 4.2 `test_link_watchdog.c` (**F.21**, NEW). Fake clock, real link, hand-written expectations, frames built with the real encoder, outputs decoded with `ocb_frame_decode(OCB_RX_APP, ...)`. Cases:
    - **Timeout boundary (AC1).** Heartbeats every 20 ms for 10^6 periods: no trip. Gaps of 27 ms (the maximum wire spacing), 60 ms (two lost) and exactly 80 ms: no trip. 80 ms + 1 µs since the last heartbeat: trip, exactly one `LINK_LOST`. `link_timeout_ms` 0 gives 80, 100 gives 100 (exactly 100 ms alive, +1 µs trips), 101, 250 and 65535 are clamped to 100, 50 is honoured (cfg 50: trips after 50 ms + 1 µs).
    - **Arming (D3).** No heartbeat ever: any silence (hours of fake time) causes no abort, no latch, no event, `link_ok` 0, `watchdog_trips` 0; `START` and `SET_STATE` get `NAK NO_LINK`; `QUEUE_EVENT`, `PING`, `STOP`, `HELLO` behave as before. The first valid heartbeat sets `link_ok` and arms; a heartbeat with a short body (`heartbeat_bad`) does not.
    - **Loss effects (AC1).** In IDLE, DIRECT and PLAYING (held note, queue not empty): after the trip the published state is `NEUTRAL`, mode IDLE, queue empty (`free` = capacity), latch set, `abort_count` + 1, `last_event` `LINK_LOST`, STATUS `flags` bit 0 = 0, and the next `START` / `SET_STATE` / `QUEUE_EVENT` get `NAK ABORTED` (the latch comes before `NO_LINK`); `STOP` or `HELLO` clears the latch and then `START` / `SET_STATE` get `NAK NO_LINK`. A core already latched (END applied earlier) trips with **no** second EVENT but still clears `link_ok` and releases TX. A due event in the same pass as the trip is not applied (poll order).
    - **Break (AC1, D5, D8).** A `BE` entry while armed: abort at the next `ocb_link_poll`, same effects, one `LINK_LOST`, `watchdog_trips` + 1. 29 further `BE` entries (the H.6 burst) while the link is down: nothing more (no second EVENT, no trip, `abort_count` unchanged). `FE`, `PE`, `OE` alone, in any number, abort nothing. A `BE` entry while not armed (boot): no abort, no latch, the TX release sequence still runs. A break and a fresh heartbeat in the same drain: the loss stands, the heartbeat then sets `link_ok` again (D7).
    - **TX release sequence (AC1, D6).** With ACK, NAK, STATUS pending and the loss: the first `ocb_link_tx_next` returns the `LINK_LOST` EVENT frame; the next calls return 0 and the others are counted in `tx_dropped`; the gate stays open until `ocb_link_tx_settled(..., tx_hw_idle = true)`; with `tx_hw_idle` false it closes by itself when 5 ms + 1 µs have passed (exactly 5 ms: still open); once closed `ocb_link_tx_next` returns 0 and discards; the gate reopens only after RX has been high for 1 ms (a low sample restarts the wait) and STATUS then resumes with `link_ok` 0; a full EVENT ring still completes the release by the deadline; a release while the gate is already closed completes at once; EVENT is **never** reordered behind a STATUS.
    - **Recovery (D7).** A heartbeat after the loss sets `link_ok` again and re-arms; the latch stays (`NAK ABORTED`) until `STOP` / `HELLO`; the session stays open (no `NAK VERSION`); a second loss gives a second `LINK_LOST` and another trip.
    - **App timeout (AC2, D9).** In DIRECT: exactly `app_timeout_ms` (1000) since the last valid app frame: alive; +1 µs: abort with `APP_TIMEOUT` (no EVENT, latch set, `abort_count` + 1, NEUTRAL, mode IDLE, `link_ok` unchanged, trip + 1). Every kind of valid app frame keeps it alive: `PING`, `SET_STATE`, a stale duplicate, a frame that gets a NAK (`BUSY`, `ORDER`, `VERSION`), `HELLO`, `STOP` (STOP leaves DIRECT, so nothing is pending); a HEARTBEAT does not; a frame with a bad CRC does not. Entering DIRECT starts the clock (the entering `SET_STATE` is the last app frame). Custom `app_timeout_ms` (250, 1) honoured, 0 gives 1000 and INFO reports 1000. **Not in IDLE** (an hour of silence: nothing) and **not in PLAYING** (a piece with no app frames for 10 s of fake time, longer than `app_timeout_ms`, stays PLAYING until END or underrun; the underrun rule is what ends a dead app's piece).
    - **DTR edge (AC2, D10).** Heartbeats with DTR 1 then 0 in DIRECT: abort (`APP_TIMEOUT`, no EVENT, trip + 1) at that heartbeat. The same edge in IDLE or in PLAYING: nothing. `0→0`, `0→1`, `1→1`: nothing. The first heartbeat ever with DTR 0: no edge. A DTR 1 heartbeat, a loss, then a DTR 0 heartbeat: no edge (memory reset by the loss). `HEARTBEAT.flags` bits 1 to 7 set or clear never matter. DIRECT entered while DTR is already 0 is not aborted by the DTR rule (the app timeout covers it).
    - **`NAK NO_LINK` through the link (AC3).** Semantic: `expected = seq + 1`, NAK fields `reason, expected (after), got = seq`, cached response on a duplicate of that seq, no effect on mode or queue; after the first heartbeat the same `START` / `SET_STATE` get `NAK NOT_READY` (normal profile) or are accepted (ready).
    - **Wrap and clock hygiene (D1).** The whole timeout, app-timeout and release scenarios repeated with the fake clock starting at `2^32 − 40 ms` in microseconds (the u32 µs wrap, about 71.6 minutes), at `2^32 × 1000 − 40 ms` µs (a u32 millisecond counter's wrap, about 49.7 days) and at `2^53 − 40 ms`; each crosses the boundary during the scenario and gives the same results as the same scenario at time 1 s. A clock that steps **backwards** by 1 s and by 2^40 µs (the glue never does it; the library must not trip): no trip, no abort, the elapsed time is treated as 0.
    - **STATUS debug counters.** `watchdog_trips` (extra byte 2) is 0 at boot, +1 per rule firing (loss by timeout, loss by break, app timeout, DTR edge), wraps at 256; `abort_count` agrees; STATUS flags bits 2 to 7 stay 0 throughout.
    - **Independence.** Two links and two cores side by side: a loss on one changes nothing on the other.
  - [x] 4.3 `test_link_nak.c` (F.2, UPDATE): the comment "NO_LINK is not produced yet" becomes a real test (the link-level semantic NAK above, with the `expected`/`got` fields and the cached-response repeat). `test_link_tx.c` (F.16, UPDATE): `link_ok` bit follows the loss and the recovery; `watchdog_trips` byte; the closed-gate discard rule is unchanged when the closure comes from a release. `test_link_session.c` (F.15): a loss does not close the session (2.6 D8 stays).
  - [x] 4.4 `test_link_fuzz.c` (F.17, UPDATE): add heartbeats (random DTR), breaks, clock jumps across the timeouts, and the glue's `ocb_link_tx_settled` to the random operations; extra invariants checked after **every** step: a loss is always followed by NEUTRAL, IDLE and an empty queue (a link with `link_ok` false is never PLAYING); PLAYING never survives a loss; no `APP_TIMEOUT` abort ever happens in PLAYING or IDLE; at most one `LINK_LOST` EVENT per loss episode; `watchdog_trips` never decreases except by u8 wrap; the TX gate never closes without a release or a reset; every output frame still decodes. The 2.6 lossy-channel property (exactly once, in order) still holds with unimpaired heartbeats.
  - [x] 4.5 `tests/link_sim_shim.c` (UPDATE, used by the probe self-test, Task 6): add `sim_heartbeat(flags, now)` (or let `sim_rx` carry them), `sim_break(now)` (one `BE` entry), `sim_rx_level(high, now)` (feeds the gate), `sim_tx_settled(now, idle)`, `sim_usb_ready(now)` (posts the mailbox like Story 2.3 will, so DIRECT and PLAYING are reachable in the model), and counters for `watchdog_trips`, `abort_count`, `link_ok`, gate open. `sim_tx` must return 0 while the gate is closed (it already does through `ocb_link_tx_next`).
  - [x] 4.6 Every new rule is **proven red once** (Task 9.2) and each red run is recorded in the Debug Log.

- [x] **Task 5: controller glue, driver and the bench-only build** (AC: #1, #2, #3 on silicon; D6, D17, D23)
  - [x] 5.1 `firmware/common/hw/include/ocb_hw/uart_link.h` and `uart_link.c` (UPDATE): add `bool ocb_uart_link_tx_idle(void)`: the software TX ring is empty **and** `UARTFR.TXFE` is set **and** `UARTFR.BUSY` is clear (the shift register has sent the last stop bit; `BUSY` per the pinned SDK's `hardware/regs/uart.h`, "remains set until the complete byte, including all the stop bits, has been sent"). Nothing else in the driver changes: `ocb_uart_link_tx_enable(false)` already returns the pad to a high-Z input and empties the software ring. Note in a comment why no hardware FIFO flush is needed on release: the PL011 keeps shifting its FIFO out whatever the pad mux says, so the at most 32 residual bytes (0.7 ms at 460800) are gone before the gate can reopen (reopening needs 1 ms of idle-high RX), D6.
  - [x] 5.2 `firmware/controller/platform/link_runner.c` (UPDATE): `cfg.link_timeout_ms = OCB_LINK_TIMEOUT_MS;` (the existing `cfg.app_timeout_ms = OCB_APP_TIMEOUT_MS_DEFAULT`). In `ocb_ctrl_link_step()` keep the order poll, then RX drain (2.6 D28), then TX (`ocb_link_tx_next` into `ocb_uart_link_tx_write`), then `ocb_uart_link_tx_pump()`, and **then** `ocb_link_tx_settled(&s_link, now, ocb_uart_link_tx_idle())`. The next pass sees the closed gate and calls `ocb_uart_link_tx_enable(false)` through the existing `open != s_tx_enabled` code. Never block, never wait for the FIFO (existing guardrail). Time stays `time_us_64()`.
  - [x] 5.3 `firmware/controller/main.c` and `firmware/controller/CMakeLists.txt` (UPDATE) and `firmware/CMakePresets.json` (UPDATE): the **bench-only build** of D17. CMake cache option `OCB_BENCH_FAKE_READY` (default OFF, defined as a compile definition only when ON). In `main.c` (core 0 loop, after `multicore_launch_core1`): when defined, post `ocb_usb_mailbox_handshake_done(ocb_ctrl_usb_mailbox())` once and `ocb_usb_mailbox_in_report(...)` about every 1 ms (wrap-safe signed difference on `time_us_32()`, like the LED edge code), so `console_ready` reads 1 and DIRECT and PLAYING become reachable without a console. `#if defined(OCB_BENCH_FAKE_READY) && !defined(OCB_DEBUG_BUILD)` → `#error "bench build is debug only"`. New configure preset `fw-debug-bench` (inherits `fw-debug`, adds `OCB_BENCH_FAKE_READY=ON`, own `binaryDir` so it never overwrites `build/fw-debug`) and the matching build preset. The LED rhythm of Story 2.1 is unchanged (a different colour for the bench build is optional; if added it goes through `ocb_hw/led` only). Story 2.3 (real `ready`) removes this option or keeps it for the same purpose; say so in a comment.
  - [x] 5.4 `.github/workflows/firmware.yml` (UPDATE): after "Build fw-debug" add a step "Build fw-debug-bench (compile gate for the bench-only build)" running `cmake --preset fw-debug-bench && cmake --build --preset fw-debug-bench`. Nothing else changes (artifact `firmware-uf2` and its paths stay; the bench UF2 is **not** uploaded). `actionlint` clean when available.
  - [x] 5.5 `fw-release`, `fw-debug`, `fw-debug-bench` build with `-Wall -Wextra -Werror -Wpedantic -Wshadow -Wconversion` clean on our sources, both UF2s still come out for release and debug; `check-pins.sh` stays green (no GPIO number, no `PICO_DEFAULT_*` outside `pins.h`).

- [x] **Task 6: `link_probe.py`: bridge emulation and one command per bench step** (AC: #1 to #3 on the bench; D15, D16; test F.23)
  - [x] 6.1 **Heartbeat service in `Port`** (`firmware/tools/link_probe.py`, UPDATE). `Port` (and the fake ports of the self-test) gain `hb_start(period_ms, flags)`, `hb_stop()`, `hb_set_dtr(bool)` and `service()`. `service()` is called from `Port.poll()` and from `Port.sleep()` (which now sleeps in slices of at most 1 ms), so every wait loop in every subcommand keeps the heartbeat alive without threads. The schedule is absolute (next due = previous due + period); a late tick sends **one** heartbeat and re-bases on now, it never sends a burst (the real bridge's alarm does not stack either). Frame: `HEARTBEAT` type 0x40, own rolling seq, body `bridge_fw_ver[3] = 0.0.1` (so INFO's `bridge_ver` is visibly not zeros after the first heartbeat), `flags` bit 0 = DTR (default 1). It records `hb_sent`, the maximum gap between two sends (ms) and the count of gaps above `OCB_HEARTBEAT_MAX_GAP_MS` (27). Heartbeat writes bypass the pacer and the in-flight cap (11 bytes every 20 ms are about 0.5 KB/s). Written from `docs/link-protocol.md`, not from the C sources.
  - [x] 6.2 Global options: `--heartbeat` / `--no-heartbeat` (default **off** for the 2.6 subcommands, so H.4 to H.7 keep their meaning; default **on** for the new subcommands below), `--hb-period-ms` (default 20, argparse-validated to 5..40 like the other guards of 2.6 D36: exit 2 before a port is opened). `soak`, `fill`, `watch`, `noise`, `try` accept `--heartbeat`.
  - [x] 6.3 New subcommands. Each runs the whole step in **one process with one open session** (the limit of D38: `fill --no-hello` cannot continue a session), prints one line per check, ends with a single `PASS` or `FAIL <reason>` line and exits 0 or 1 (2 for a skipped step, see `direct`), and saves nothing the owner must handle. None needs the keyboard: where a physical action is needed, the probe prints one prominent line and then waits for the **evidence** (an event, a counter, STATUS stopping or resuming), with a timeout.
    - `linkup [--seconds 20] [--expect-fresh] [--expect-ready auto|no|yes]` (H.8). HELLO until INFO (prints `bridge_ver`, expected 0.0.0), then `try` (`SET_STATE` and `START`): both `NAK NO_LINK`; start heartbeats; time from the first heartbeat written to STATUS `link_ok = 1` (expected ≤ 100 ms); a second HELLO shows `bridge_ver` 0.0.1; `try` again: `NAK NOT_READY` on the normal build (`--expect-ready no`) or `SET_STATE` ACKed / `START` `BAD_STATE` on the bench build (`yes`); `auto` takes it from STATUS `flags` bit 1; then hold `--seconds` seconds printing STATUS statistics: `link_ok` stays 1, `abort_count` and `watchdog_trips` do not change, the probe's own heartbeat statistics (max send gap, gaps above 27 ms). `--expect-fresh` additionally requires, before the first HELLO of the run, `abort_count` 0 and `last_event` NONE (a controller power-cycled just before: the boot break did not latch, D3), and prints the boot counters.
    - `silence` (H.9). HELLO, heartbeats, `fill --count 5`, wait until stable; stop the heartbeat at a recorded time t0 (the time of the last heartbeat write); wait for `EVENT LINK_LOST`: elapsed from t0 is printed and must be in **[80, 120] ms** (below 80 is an early trip; above 120 is a failure: 80 ms timeout plus two USB hops and the loop); then STATUS must show `link_ok` 0, mode IDLE, `abort_count` + 1, `last_event` LINK_LOST, `free` back to 512 (queue cleared); `SET_STATE` gives `NAK ABORTED`; `STOP` ACKed; `SET_STATE` gives `NAK NO_LINK`; resume heartbeats: STATUS `link_ok` 1 within 100 ms; `SET_STATE` `NAK NOT_READY` (or accepted on the bench build); no `NAK VERSION` anywhere (session stayed open). No physical action.
    - `direct [--keepalive-seconds 5]` (H.11, **bench build only**; on a build whose STATUS `flags` bit 1 is 0 it prints `SKIP: console_ready is 0, flash the bench build (fw-debug-bench)` and exits 2). HELLO, heartbeats (DTR 1); (a) `SET_STATE` (neutral, accepted → mode DIRECT) then `PING` every `app_timeout_ms / 4` (the AD-7 rule, taken from INFO) for `--keepalive-seconds`: mode stays DIRECT, no abort; (b) stop all app frames (heartbeats continue): the abort must be seen in STATUS (mode IDLE, `abort_count` + 1, **no** EVENT, `link_ok` still 1) between `app_timeout_ms` and `app_timeout_ms + 150 ms` after the last app frame write; (c) `STOP`, `SET_STATE` again, then heartbeats flip DTR 1 → 0: abort seen within 100 ms of the first DTR-0 heartbeat write, no EVENT; (d) `STOP`, `SET_STATE`, heartbeats keep DTR 0 and PINGs continue for 3 s: no abort (the DTR rule is an edge, not a level).
    - `playing [--seconds 3]` (H.11, bench build only, same skip rule). Phase 1: queue a `--seconds` piece (a few neutral/A events, last `NEUTRAL|END`), `START` with a 100 ms lead, then **no app frames at all** for longer than `app_timeout_ms` while heartbeats continue: mode stays PLAYING until the piece ends, then `EVENT PLAYBACK_DONE` (not an app-timeout abort: AC2 "neither rule applies in PLAYING"); the same with a DTR 1 → 0 flip mid-piece: still PLAYING. Phase 2: a 10 s piece, `START`, stop the heartbeat 1.5 s in: `EVENT LINK_LOST` within [80, 120] ms, STATUS mode IDLE, `free` 512 (AC1 in PLAYING).
    - `breaks [--rounds 10] [--mode idle|playing] [--no-heartbeat] [--meter-seconds 15] [--count 5]` (H.12; the follow-up named in D38). One HELLO, heartbeats (unless `--no-heartbeat`), then per round: print `ROUND n/N: UNPLUG the bridge-TX wire (GP1 side) now`, wait up to 60 s for the evidence (armed: `EVENT LINK_LOST`; unarmed: the break counter `rx_be` rising in the next STATUS, or STATUS going silent), print how long STATUS has been silent after the event (armed only: expected: silent from the event on, the TX release) and, **in round 1 only**, `READ the multimeter on GP0 now (expect about 0 V); waiting <meter-seconds> s`; then print `REPLUG the wire now`, wait up to 30 s for STATUS to resume (the gate reopens after 1 ms of idle-high) and for `link_ok` 1 (armed; heartbeats resume), and check: armed: `abort_count` + 1, mode IDLE, `free` 512, `last_event` LINK_LOST, then `QUEUE_EVENT` gets `NAK ABORTED`, `STOP` ACKed, `--count` events accepted, and no `NAK VERSION`; unarmed: `abort_count` unchanged, mode IDLE, the session continues (the next `--count` events continue the open session's seq and are ACKed, no `NAK VERSION`, no `NAK SEQ`: this is exactly the H.6 check, now in one command). `--mode playing` (bench build) queues a 60 s piece and `START`s it before each unplug: after the loss mode must be IDLE and `free` 512. Round summary `round n: PASS ...`; final `PASS` only if every round passed.
  - [x] 6.4 `soak` with `--heartbeat`: the run must end with `watchdog_trips` and `abort_count` unchanged and `link_ok` 1 in the last STATUS (a false trip during 20 000 paced events fails the run); the heartbeat statistics are printed. Everything else of 2.6 D30 to D36 stays (pacer, in-flight cap, option guards).
  - [x] 6.5 `firmware/tests/test_link_probe.py` (UPDATE, **F.23**, `link_probe_selftest`): the new subcommands run **unmodified** against the real link through the shim on the fake clock, through the 2.6 `Bridge2wiCCPort` model extended with: the heartbeat path (bytes into the controller's RX), the gate (controller bytes are dropped while the gate is closed and `sim_rx_level` follows the model's "wire plugged" flag), and a scripted "user" that performs the unplug and replug when the probe prints its prompt (a hook on the printed line; no real keyboard). Required results: `linkup` PASS with `--expect-fresh`; `silence` PASS with the measured time in [80, 100] ms exactly on the fake clock; `direct` and `playing` PASS with `sim_usb_ready` (and `SKIP`, exit 2, without it); `breaks --rounds 3` PASS in armed, unarmed and `playing` modes; `soak --heartbeat --cycles 5` PASS with trips 0. **Negative controls** (each must make the command FAIL, which proves the checks are not vacuous): a heartbeat period of 100 ms through the same path (the controller trips and `linkup`'s hold fails); a fake port that drops 4 heartbeats in a row (a gap of 100 ms: `linkup`'s hold fails); a scripted user who never replugs (`breaks` times out with its explicit message). The probe's constants (`HEARTBEAT_PERIOD_MS` 20, `HEARTBEAT_MAX_GAP_MS` 27, `LINK_TIMEOUT_MS` 80, `APP_TIMEOUT_MS_DEFAULT` 1000, `APP_PING_DIVISOR` 4) are compared with `common/proto/vectors/constants.txt` in the self-test (D18). Stdlib only, no pyserial, `python3 -I`.
  - [x] 6.6 Header docstring of `link_probe.py`: list the new subcommands and options; keep SPDX `MIT`; no copied code; the heartbeat frame is built with the probe's own codec.

- [x] **Task 7: one value, one place: constants across header, vectors and docs** (AC: #1, #2; D18; test F.22 `docs_constants_check`)
  - [x] 7.1 The values already live in `common/proto/include/ocb_proto/consts.h` and `common/proto/vectors/constants.txt` and `test_vectors` already compares the two: **no new constant is introduced** (`OCB_LINK_TIMEOUT_MS` 80, `OCB_LINK_TIMEOUT_MAX_MS` 100, `OCB_APP_TIMEOUT_MS_DEFAULT` 1000, `OCB_HEARTBEAT_PERIOD_MS` 20, `OCB_HEARTBEAT_MAX_GAP_MS` 27, `OCB_APP_PING_DIVISOR` 4). `OCB_LINK_RELEASE_WAIT_US` (5000) is local to `ocb_link/config.h` (like `OCB_LINK_TX_IDLE_HIGH_US`), is documented in `docs/link-protocol.md`, and is not a wire value.
  - [x] 7.2 `firmware/tests/test_docs_constants.py` (NEW, stdlib, run as ctest `docs_constants_check` with `python3 -I`): parses the "Timing constants" table of `docs/link-protocol.md` (`| OCB_NAME | value | ... |` rows) and checks that **every** row whose name is in `constants.txt` has the same value (the file stores big-endian hex, the doc decimal), and that `OCB_LINK_TIMEOUT_MS` ≤ `OCB_LINK_TIMEOUT_MAX_MS` ≤ 100 and `OCB_HEARTBEAT_MAX_GAP_MS` < `OCB_LINK_TIMEOUT_MS`. It also greps the prose sentences of the "Watchdog" section and the new "Rules the controller link applies (Story 2.7)" section for the literal numbers `80`, `100`, `1000`, `20`, `27` next to their names and fails on a mismatch (a small regex per sentence is enough; the point is that no number can drift unnoticed). Proven red once (change the doc table's 80 to 81).
  - [x] 7.3 The C side needs no new test: `test_link_watchdog` uses the macros from `consts.h`, not literals, **and** one test asserts the literal values 80, 100 and 1000 (so a change of the header fails a test that a human must then update on purpose).

- [x] **Task 8: Docs and the AD-20 amendment** (AC: all; D14, D24)
  - [x] 8.1 **AD-20 amendment (text only, applies the pending amendment of Story 2.6 D25).** In `_bmad-output/planning-artifacts/architecture/architecture-bonooru-2026-11-19/ARCHITECTURE-SPINE.md`, section AD-20:
    - Heading line: append ` [AMENDED 2026-11-22: story 2.6 D25, applied by story 2.7]` after the title, in the style of the `[ADOPTED: ...]` tags of AD-5 and AD-9.
    - Replace the bullet whose text is "**TX release.** Both boards enable an internal pull-down on RX. Each keeps TX high-Z until RX sees idle-high, and releases TX again on a break or on link loss." with exactly this bullet (inner code spans kept): "- **TX release.** Both boards enable an internal pull-down on RX. The controller keeps TX high-Z until its RX has been idle-high for 1 ms (`OCB_LINK_TX_IDLE_HIGH_US`), and releases TX again on a break or on link loss, after the `EVENT LINK_LOST` has left. The bridge leads: while its RX does not show a live controller it drives TX idle-high in short probe pulses (about 5 ms every 500 ms, at most 1 % duty) and returns to high-Z in between; once its RX shows the controller (idle-high, or a valid frame) it keeps TX enabled, and after a break or link loss it returns to probing. A rule that made both boards wait for idle-high would deadlock with both powered."
    - Replace the bullet "- **Verification.** Measure on hardware in M3/M4." with "- **Verification.** Measure on hardware in M3/M4: H.1 (both boards, both power orders, and the unpowered cases), H.7 and H.12 (controller side, stories 2.6 and 2.7), A.3."
    - Nothing else in AD-20 or in the spine changes. Add one line to `docs-and-spec-edits.md`? No: that file records the edits the architecture run left for docs; this amendment is recorded in the story only.
  - [x] 8.2 Same sentence elsewhere, text only: `_bmad-output/planning-artifacts/epics.md` line 119 (the AD-20 summary: "both boards pull down RX, keep TX high-Z until RX idle-high, release TX on break or link loss" becomes "both boards pull down RX; the controller keeps TX high-Z until RX idle-high and the bridge leads with short TX probe pulses; both release TX on break or link loss") and the last Given/When/Then of **Story 3.1** (line 681: "TX stays high-Z until RX sees idle-high" becomes "the bridge probes with TX idle-high pulses of about 5 ms every 500 ms until RX shows the controller, then keeps TX enabled, and returns to probing on a break or link loss; F-test for the probe state machine"); Story 2.6's AC (line 620) stays as it is (it is the controller's rule, still true). `_bmad-output/specs/spec-bonooru/risk-register.md` row C5: replace the mitigation clause "both boards enable an internal pull-down on RX, keep TX high-Z until RX sees idle-high, and release TX again on a break or link loss" with "both boards enable an internal pull-down on RX; the controller keeps TX high-Z until RX sees idle-high and releases it on a break or link loss, the bridge leads with short TX probe pulses (AD-20 as amended)"; keep the status `open`. Say in the Completion Notes that these two files are normally outside the dev agent's remit and that the edit is text-only and follows D14.
  - [x] 8.3 `docs/link-protocol.md` (UPDATE; keep every existing sentence except where noted): after "Rules the controller link applies (Story 2.6)" add **"Rules the controller link applies (Story 2.7)"**: arming (first valid HEARTBEAT; before it the link is down, `NAK NO_LINK`, no timeout) (D3); the two limits and the strict-greater boundary (exactly 80 ms and exactly 1000 ms are still alive) and the clamp to 100 (D4); what a break is (BE only) and that it acts at the next poll (D5); the loss sequence in order: abort (latch, NEUTRAL, queue cleared), `EVENT LINK_LOST` (only if the latch was not already set), `link_ok` 0, EVENT sent, TX released after the hardware has drained or 5 ms, reopened after 1 ms of idle-high RX (D6); breaks while the link is already down only release TX (D8); recovery by the next HEARTBEAT, latch stays until STOP/HELLO, session stays open (D7); the app-timeout rule: DIRECT only, any valid app frame counts, `APP_TIMEOUT` has no EVENT (D9); the DTR rule: edge between consecutive heartbeats, DIRECT only, memory reset on loss (D10); `NAK NO_LINK` position in the precedence and that it is semantic (D11); `watchdog_trips` (D12); NFR-05 budget: 80 ms + one pass + one report period ≤ 100 ms (D22). Fix the three sentences that say "Story 2.7" later: the `link_ok` bullet (line "is set by the first valid HEARTBEAT. Clearing it ... is the link watchdog, Story 2.7"), the debug counter v1 line ("`watchdog_trips u8` (0 until Story 2.7)") and the TX gate bullet ("The link watchdog (Story 2.7) releases TX again..."). Add the app-side consequences for Story 3.3: after `EVENT LINK_LOST` (or STATUS `link_ok` 0 and `abort_count` + 1) the app abandons the piece and must `STOP` (or `HELLO`) before anything else; it must not send `START` / `SET_STATE` while `link_ok` is 0 (it gets `NAK NO_LINK`, a semantic NAK that advances `expected`); after `abort_count` rises without an EVENT (app timeout) the same recovery; a DIRECT-mode app must `PING` every `app_timeout_ms / 4` and the heartbeat's DTR must be asserted. State that **no wire value changed**. Update the "Who drives first" paragraph only by pointing to the amended AD-20.
  - [x] 8.4 `docs/test-plan.md` (UPDATE): "Firmware host tests": rewrite **F.2**, **F.3**, **F.12**, **F.16**, **F.17** rows where Task 4 changed them (keep IDs and keep the old content), add **F.21** (`test_link_watchdog`), **F.22** (`docs_constants_check`), **F.23** (probe commands on a fake clock). "Hardware-in-the-loop checks": add **H.8** to **H.12** exactly as written in "Hardware checks" below (empty Result column); amend **H.4** step 6 expectation (without a heartbeat `try` now gives `NAK NO_LINK`; `NAK NOT_READY` needs `--heartbeat`; the recorded 2026-11-22 PASS stays as history of the 2.6 build) and **H.7** step 5 (with Story 2.7 the wire pull releases TX: GP0 reads about 0 V; the recorded PASS stays as history); H.6 gets a one-line pointer to `breaks` (the follow-up of D38). Add the heartbeat emulation to the "Bench rig for the controller link" block (the probe sends the bridge's HEARTBEAT; the 2wiCC_Comms peer forwards it unchanged; host jitter caveat, D15). Leave H.0 to H.3 untouched (H.1 stays Story 3.1's).
  - [x] 8.5 `firmware/controller/README.md`: one paragraph on the watchdog (where it lives, the bench build and why it is debug-only); `firmware/README.md`: the new ctest names, `fw-debug-bench`, and the probe subcommands; `firmware/THIRD_PARTY_NOTICES.md`: append "watchdog, bench-ready hook, probe heartbeat: written from `docs/link-protocol.md` and the pico-sdk API, no third-party code".
  - [x] 8.6 Do **not** edit `docs/ocarina-input-mechanics.md`, `common/proto/**` (vectors, `consts.h`), `_bmad/`, `~/Documents/org/`; the only files under `_bmad-output/planning-artifacts` and `_bmad-output/specs` that change are the three of 8.1 and 8.2, text only.

- [x] **Task 9: Local verification, red checks and notes** (AC: all host parts)
  - [x] 9.1 From `firmware/` with the 2.1 setup: `cmake --preset host-tests && cmake --build --preset host-tests && ctest --preset host-tests` all green (the 23 existing tests plus `test_link_watchdog` and `docs_constants_check`: recount before writing the number; `link_probe_selftest` grows but stays one test); `fw-release`, `fw-debug`, `fw-debug-bench` build with zero warnings and the UF2s come out; `check-pins.sh`, `check-proto-pure.sh`, `check-proto-version.sh`, `check-core-pure.sh`, `check-link-pure.sh` pass; `shellcheck` and `actionlint` when available. Confirm ASan and UBSan are active (`-fsanitize=address,undefined` in the compile commands).
  - [x] 9.2 **Red checks**, each reverted afterwards, each recorded in the Debug Log with the failing output line: (a) timeout test `>` changed to `>=` (80 ms exactly trips): `test_link_watchdog` fails on the exact-boundary case; (b) timeout limit 100 → 101 ms (or drop the clamp): fails the clamp case; (c) arm at boot instead of at the first heartbeat: the "no heartbeat ever" case fails; (d) ignore `BE` (treat FE as a break): the FE-only case fails; (e) do not clear `link_ok` on loss: the STATUS and `NO_LINK` cases fail; (f) release TX **before** the EVENT is sent (swap the order): the EVENT-before-release case fails; (g) never release (drop `ocb_txgate_release`): the gate case fails; (h) `app_timeout` applies in PLAYING: the PLAYING case fails; (i) `app_timeout` applies in IDLE: the IDLE case fails; (j) DTR rule fires on a level instead of an edge, or in PLAYING: the DTR cases fail; (k) place the `NO_LINK` check after `BUSY` or before `ABORTED`: the F.3 precedence rows fail; (l) `elapsed` without the `now >= then` guard (a backwards clock step): the backwards-clock case fails; (m) compute the deadline as `then + limit` in `uint32_t`: the 2^32 µs wrap scenario fails; (n) run the watchdog **after** `ocb_core_tick` in `ocb_link_poll`: the "due event in the same pass" case fails; (o) have the heartbeat timer measure from the heartbeat's *arrival at poll* instead of its receipt: covered by (a) if it moves the boundary, otherwise skip with a note; (p) flip one number in `docs/link-protocol.md`'s timing table: `docs_constants_check` fails; (q) break `LINK_TIMEOUT_MS` in the probe's constants: `link_probe_selftest` fails; (r) the probe's `silence` negative control (period 100 ms) actually fails the command.
  - [x] 9.3 Completion Notes: exact commands, the ctest summary line, ASan/UBSan status on the local compiler, `arm-none-eabi-size` and stack figures (`-fstack-usage` of `ocb_ctrl_link_step` and `ocb_link_poll`, informational for Story 2.5's budget; 2.6 M-6 recorded 304 B for the step), the CI status ("verified green after the owner's push" or "unverified until pushed") and, for each of H.8 to H.12, "unverified on hardware" until Task 10 is recorded. Mention the three planning/spec files edited in Task 8.2.

- [ ] **Task 10: Owner bench session on hardware** (AC: #1 to #3 on silicon; **owner, not the dev agent**; D2, D16, D17)
  - [ ] 10.1 Follow "Hardware checks" below exactly (about 15 physical actions in all: 2 flashes, 1 power-cycle, 10 unplug/replug pairs, 1 multimeter reading). Record the results in `docs/test-plan.md` (H.8 to H.12 Result column and the rig block), then set the story to `done` after the code review (or write the deferral). If a result is a FAIL, open a follow-up and keep the story in `review`. If the owner declines the bench build (D17), H.11 is deferred to Story 2.3's H.3 run in writing and AC2 stays "host only" until then.

## Dev Notes

### What this story is

The controller's fail-safe: the three rules that return the board to NEUTRAL by itself, and the one NAK that tells the app why `START` / `SET_STATE` are refused while the link is down. As with Stories 2.4 and 2.6 the rules live in pure C that the PC tests with a fake clock; the silicon part is thin glue (one driver predicate, one call in the step, a debug-only bench hook). Everything that can leave a key pressed (a heartbeat timeout that never fires, a boot-time break that latches the core, a TX release that drops the `LINK_LOST` EVENT, an app timeout that kills a running piece, a 32-bit time wrap that trips the link) is decided here, so each open point has a recorded decision (D1 to D25).

### Design in one page

```text
 core 1 loop (2.6): ocb_ctrl_link_step()
   gate sample -> TX pad on/off
   ocb_link_poll(now):
       USB mailbox / ready aborts                        (2.6)
       watchdog check (NEW, before the scheduler):
           armed && elapsed(now, last_hb) > link_limit   -> LINK LOSS
           armed && new BE since last poll               -> LINK LOSS
           not armed && new BE                           -> TX release only
           DIRECT && elapsed(now, last_app_frame) > app_limit -> abort(APP_TIMEOUT), no EVENT
       ocb_core_tick                                     (scheduler)
   ocb_link_rx_drain:
       HEARTBEAT: store, link_ok = 1, arm; DTR 1->0 in DIRECT -> abort(APP_TIMEOUT)
       app frame: note_app_frame(now); core gets env.link_ok (NO_LINK for START / SET_STATE)
   ocb_link_tx_next -> uart ring -> pump
   ocb_link_tx_settled(now, uart_tx_idle):               (NEW)
       release_pending && EVENT sent && hw idle  (or 5 ms deadline) -> ocb_txgate_release()

 LINK LOSS = ocb_core_abort(LINK_LOST) [NEUTRAL, queue cleared, latch, EVENT unless latched]
             + link_ok = 0 + disarm + release_pending
 Recovery  = next valid HEARTBEAT (link_ok = 1, re-arm); latch stays until STOP / HELLO.
 Gate      = reopens after RX idle-high for 1 ms (2.6), independent of link_ok.
```

### Decisions (made autonomously, 2026-11-22; the owner can overrule any of them)

Each was researched in the repo first (AD-4, AD-5, AD-8, AD-19, AD-20, `docs/link-protocol.md` "Watchdog" and "Rules the controller ... applies", `docs/test-plan.md`, Stories 2.4 and 2.6 and their decisions and hand-offs, `consts.h` and the vectors, the 2.6 code) and, for the silicon, in the pinned pico-sdk 2.3.1 sources (UART register definitions: `UARTFR.BUSY`, `TXFE`, break behaviour already confirmed in 2.6 D20). Every open point was answerable from these, so no external source was needed; nothing from outside is copied.

**D1. The watchdog keeps the 64-bit microsecond clock of the rest of the library; 32-bit wrap is handled by subtraction and proven by tests, not by a second 32-bit millisecond clock.**
`ocb_link` and `ocb_core` take `now_us` as `uint64_t` (the glue passes `time_us_64()`, which does not wrap in practice). A u32 millisecond timestamp would need `now_us / 1000` on a `uint64_t`, which the library rules ban (2.6 Task 2.4: no 64-bit divide on Cortex-M0+), or a second clock that could disagree with the first. Instead every comparison is `elapsed > limit` with `elapsed = now - then` computed by one helper that returns 0 for `now < then` (D4), and the limits are converted to microseconds once at init. The only real 32-bit hazards in the codebase are `STATUS.now_us` (D13 of 2.6, accepted) and `time_us_32()` in `main.c`'s LED loop (already a signed difference). The required wrap tests run the same scenarios with the fake clock starting just below 2^32 µs (71.6 minutes), 2^32 ms (49.7 days of a u32 millisecond counter) and 2^53, and with a clock that steps backwards. *Alternative rejected:* `uint32_t` milliseconds with `(uint32_t)(now_ms - then_ms)`: correct for gaps below 49.7 days but it adds a divide, a truncation and a second time base; if the owner prefers it the change is local to `watchdog.c` and the wrap tests already exist.

**D2. The watchdog is a new pure module inside `ocb_link`, not part of the core and not a new library.**
The core stays what 2.4 made it (modes, queue, latch, mask); its only change is the `NO_LINK` input (D11). The rules need the heartbeat time, the app-frame time, the break counter and the TX gate, all of which live in `ocb_link`. A new file `watchdog.c` there is covered by the existing purity gate (`check-link-pure.sh`, F.18, scans the directory) and the existing CI step, so no gate is added or extended. *Alternative rejected:* a separate `ocb_watchdog` library (one more CMake target, one more gate copy, and the drift test of 2.6 D16 grows for no gain).

**D3. The watchdog is armed by the first valid HEARTBEAT. Before it, the link is down, nothing times out, and a boot break does not latch the core.**
AD-5 says "no valid heartbeat within the timeout", which has no reference point at power-up. Choosing "since boot" would latch the core (`abort_count` 1, `LINK_LOST` event into a closed gate) on every power-up with no bridge, and H.4 measured exactly one break at boot (`rx_be = 1`, the line settling when the UART takes the pin). Choosing "armed by the first heartbeat" keeps the boot state clean: `link_ok` 0, so START and SET_STATE get `NAK NO_LINK` (AC3) and nothing can be PLAYING or DIRECT without a link. The fail-safe is not weakened: a controller that never saw a heartbeat has no session worth protecting, and the moment it sees one the clock runs. The 2.6 bench (no heartbeat sender) keeps working: H.4 to H.6 are unaffected except the `try` expectation (D24).

**D4. Both timeouts trip strictly after the limit: exactly 80 ms and exactly 1000 ms are still alive.**
Same convention as the 2.6 `ready` stall ("exactly 100 ms is still ready, 100 ms + 1 µs is not"). The AC's wording "within" supports it. `link_timeout_ms` 0 selects 80 and values above 100 are clamped to 100, so the AC's "≤ 100" cannot be exceeded by a build constant (`OCB_LINK_TIMEOUT_MAX_MS`); a value below the heartbeat's maximum wire gap (27 ms) is not clamped (it would trip falsely, but it is the build's explicit choice, and the tests use 50 as the low case). `app_timeout_ms` 0 selects 1000; the effective value is what INFO reports.

**D5. A break is a data-register entry with `BE`; it is acted on at the next poll.**
AD-5: "the UART reports a break". 2.6 already counts `BE` entries (`cnt.breaks`) and keeps FE, PE and OE as separate counters; H.6 saw FE counts far above BE counts (garbage on a pulled wire), so FE alone is line noise, not a break, and must not abort a running piece. The watchdog compares the break counter with its own last-seen copy at every `ocb_link_poll`; the glue polls before it drains RX (2.6 D28), so a break drained in pass N acts in pass N+1 (microseconds). A break and the heartbeat that follows it in the same drain: the loss stands (a break in the middle of a link is a fail-safe event) and the heartbeat re-establishes `link_ok` (D7).

**D6. The loss sequence is: abort, EVENT, then release TX; the release waits for the EVENT and the hardware, with a 5 ms bound.**
AC1: "sends `EVENT LINK_LOST`, clears `link_ok`, and releases TX". If the gate closed first, 2.6's `ocb_link_tx_next` would discard the EVENT; if it never closed, TX would stay driven (AD-20 breached). So: the abort queues the EVENT (priority first), `release_pending` is set, and while it is set only the EVENT ring is emitted (stale ACK, NAK, INFO and STATUS are discarded and counted, like a closed gate would). The glue then reports `tx_hw_idle` (software ring empty, FIFO empty, shifter idle) through `ocb_link_tx_settled`; the gate is released when the EVENT has been handed out **and** the hardware is idle, or after `OCB_LINK_RELEASE_WAIT_US` = 5000 µs, whichever comes first. 5 ms covers the worst backlog (128-byte ring plus 32-byte FIFO = 3.5 ms at 460800) with margin and still ends long before anyone cares; it is local (not a wire value). The EVENT frame is 8 bytes (174 µs), so the one-off pulse into a peer that is gone (the C5 case: bridge unplugged, controller on the Switch) is that long; this is exactly what AC1 asks, and the H.1 measurement in Story 3.1 is where its back-power effect on an unpowered bridge is read. After the release the 2.6 rule applies unchanged: TX stays closed until RX has been idle-high for 1 ms. If RX is already high (a stalled but powered bridge) the gate reopens 1 ms later and STATUS resumes with `link_ok` 0, which is what the app needs to learn. No hardware FIFO flush is needed: the PL011 keeps shifting its FIFO out whatever the pad mux says, and at most 32 residual bytes take 0.7 ms, less than the 1 ms the gate needs to reopen. *Alternative rejected:* closing the gate in `ocb_link_poll` and sending the EVENT out of band (a second TX path breaks "core 1 is the only writer through one step").

**D7. Recovery is the next valid HEARTBEAT; the latch and the session are not touched by it.**
`handle_bridge` sets `link_ok` on every valid heartbeat (2.6 D10, now also after a loss) and re-arms the watchdog. The abort latch stays set until the app sends `STOP` or `HELLO` (AD-8), so START / SET_STATE / QUEUE_EVENT get `NAK ABORTED` first (precedence). The session stays open (2.6 D8: link loss and aborts never close it), so the app's recovery is `STOP`, not a new `HELLO`. A second loss gives a second EVENT and another trip.

**D8. A break releases TX whether or not the link was armed; only an armed link aborts.**
AD-20: "releases TX again on a break or on link loss". H.7 step 5 of Story 2.6 recorded that, without Story 2.7, pulling the peer's TX wire left GP0 at 3.28 V; that is the behaviour to fix, and it should not depend on a heartbeat having been seen (the same bench board, no heartbeat, should show about 0 V on GP0 after a wire pull). So a break always starts the release sequence (no EVENT is pending, so it completes as soon as the hardware is idle). Abort, latch and EVENT happen only when armed (D3). A break while the link is already down (the 30 further `BE` entries of a bouncing wire) does nothing more than a release that is already complete.

**D9. The app timeout counts any valid app frame, runs only in DIRECT, and aborts with no EVENT.**
AC2: "no app frame arrives". The reference is the time recorded in admission step 1 (`note_app_frame`), so every valid frame 0x01..0x3F counts, whatever its seq, session state or outcome (a stale duplicate or a NAKed frame proves the app is alive; a corrupt frame, which never reaches admission, does not; a HEARTBEAT is not an app frame). Entering DIRECT needs no special reset because the entering `SET_STATE` is itself an app frame. It applies in DIRECT only (AD-5): IDLE has nothing pressed, and PLAYING is covered by the underrun rule (a dead app's queue plays out and the controller goes neutral on its own). The cause is `OCB_ABORT_APP_TIMEOUT`, which AD-8 defines with no EVENT; the app learns from STATUS `mode` and `abort_count`. `app_timeout_ms` comes from `cfg` (the value INFO already reports, default 1000 until W.1).

**D10. The DTR rule is an edge between two consecutive valid heartbeats, in DIRECT only, memory reset by a loss.**
AC2: "a DTR falling edge". The previous DTR bit starts as "unknown" (0): the first heartbeat ever, with DTR 0, is not an edge. The edge is evaluated when the heartbeat is received and acts only if the mode is DIRECT at that moment; it is not remembered (a DIRECT entered later with DTR already low is still covered by the app timeout, and the app is told by the bridge's own rule to assert DTR on open). A loss resets the memory so a replugged bridge's first heartbeat is never an edge. Flags bits 1 to 7 are ignored (AD-4 freezes only the body prefix and bit 0). The abort uses `OCB_ABORT_APP_TIMEOUT` (AD-5: "counts as an app timeout in DIRECT mode") and so has no EVENT.

**D11. `NO_LINK` is decided by the core from a new `env.link_ok`; positive polarity; slot between `ABORTED` and `BUSY`.**
2.4 D3 put the NAK precedence in the core and reserved the `NO_LINK` slot "between `ABORTED` and `BUSY`" for this story, and 2.6 noted it needs a core input. `ocb_core_env` gains `bool link_ok`. Polarity: `link_ok` (true = up) so that a zero-initialised env means "no link": the unsafe omission (a caller that forgets the field) refuses START and SET_STATE instead of accepting them. The cost is that the 2.4 harness and the one literal initialiser must set it to true (Task 1.3). `NO_LINK` is only for `SET_STATE` and `START` (AC3, AD-5 "Gating"), is semantic (advances `expected`, no effect), and sits before `BUSY`, so a PLAYING controller whose link is down answers `NO_LINK`, not `BUSY` (it will already have aborted: the latch comes first, so in practice the app sees `ABORTED`).

**D12. `watchdog_trips` counts each rule firing, once per episode, u8 wrapping.**
It is the debug counter W.2 reads ("0 false watchdog trips"): it increments when the heartbeat timeout, a break (armed), the app timeout or the DTR edge fires, whether or not the latch was already set (a rule firing is a trip even when the core had nothing left to abort). One loss episode is one trip (the watchdog disarms until the next heartbeat). Layout of the 21-byte debug block is unchanged (byte 2).

**D13. The test fixtures keep the link up by default through a heartbeat helper.**
Existing tests move the fake clock by hundreds of milliseconds (the `ready` stall, STATUS cadence over 1000 periods, the fuzz and lossy-channel models). With a heartbeat watchdog armed they would all see `LINK_LOST`. The fixture gets `lh_advance()` (moves time in ≤ 20 ms slices and sends heartbeats) and `lh_link_up()`; `lh_init` still starts with the link down so the 2.6 boot-state tests stay as they are. The lossy-channel model sends its heartbeats unimpaired: heartbeat loss is F.21's subject, and four consecutive losses in 10 % random loss would make the exactly-once property depend on the watchdog.

**D14. This story applies the AD-20 amendment of 2.6 D25 to the spine, to epics (AD summary and Story 3.1), and to the C5 row of the risk register: text only.**
2.6 D25 recorded the amendment text and left the spine unedited. 2.7 is the story that implements the controller half of "releases TX", and leaving a spine rule that deadlocks as written would let Story 3.1 implement it literally. Deciding now costs three small text edits (Task 8.1 and 8.2 give the exact replacement strings); deferring it to 3.1 leaves the same defect in place for several stories. The `[AMENDED ...]` tag follows the spine's own `[ADOPTED ...]` / "amended" convention. The risk-register row keeps status `open` (H.1 with the real bridge is still pending). Normally `_bmad-output/specs/` is outside the dev agent's remit; this edit is one clause, mirrors the spine and is flagged in the Completion Notes for the owner. *Alternative rejected:* leaving all three to Story 3.1 (it would then have to change the controller-side wording it did not write).

**D15. The probe emulates the bridge's heartbeat on the bench; host jitter is a bench limit, not a firmware finding.**
There is no bridge firmware (Story 3.1) and the bench peer is the 2wiCC_Comms Zero, a transparent USB-CDC to UART pipe. The only way to exercise the watchdog on silicon is for the Mac to write `HEARTBEAT` frames through it (they pass unchanged, 2.6 D32; 11 bytes every 20 ms is 0.5 KB/s, far below the pipe's drop threshold). The schedule is absolute with no stacking (like the real bridge's alarm), it runs inside `Port.poll()` and `Port.sleep()` so every wait loop keeps it alive without threads, and the probe records its own send gaps. A Mac scheduler or USB stall of more than 60 ms between two heartbeat writes would trip the controller correctly and look like a false trip: the pass criteria therefore read `watchdog_trips` **together with** the probe's gap statistics, and a trip with a probe gap above 60 ms is recorded as "host jitter", not as a firmware failure. Mitigations, in order: close other heavy apps, `--hb-period-ms 10`, `nice -n -10`. The jitter immunity of the real design (hardware alarm in the bridge, F.7) is Story 3.1's proof, not this one's. The probe's heartbeat carries `bridge_fw_ver` 0.0.1 so the INFO `bridge_ver` path is checked on silicon, and its DTR bit is under the probe's control, which is how the DTR rule is exercised without a real CDC port.

**D16. One probe command per bench step, no keyboard, evidence instead of "press Enter".**
Story 2.6's H.6 needed a scratchpad harness (D38). Here every step that needs a persistent session is one subcommand (`linkup`, `silence`, `direct`, `playing`, `breaks`), runs in one process with one open session, prints one line per check and one final `PASS` or `FAIL <reason>`, and where the owner must act it prints a single prominent line and then waits for the **evidence** (an EVENT, a counter, STATUS stopping or resuming) with a timeout. The owner's physical actions are: flash a UF2 (BOOTSEL), power-cycle the controller, unplug and replug one wire (the bridge-TX to GP1 wire, as in H.6 and H.7, never a jumper to GND), and read one multimeter value. `breaks` replaces the follow-up named in D38. The old subcommands keep their behaviour and defaults. Each new command is run unmodified against the real link code on a fake clock in F.23, with negative controls, so a green self-test means the command can fail.

**D17. A bench-only debug build fakes `console_ready`, because without it DIRECT and PLAYING cannot be reached on silicon.**
Story 2.3 (USB) is not done, so the controller's `ready` is always 0 and `SET_STATE` and `START` get `NAK NOT_READY`: AC2 (DIRECT rules, "neither applies in PLAYING") and the PLAYING half of AC1 could then only be proven on the host. The 2.6 mailbox contract already says who posts: core 0, through five single-writer counters. A debug-only compile option posts `handshake_done` once and an IN report every millisecond from the core 0 loop (about 15 lines in `main.c`), nothing else. Guards: default OFF; `#error` unless `OCB_DEBUG_BUILD`; its own preset (`fw-debug-bench`) and build directory so the UF2 can never be mistaken for the plain debug one; CI builds it as a compile gate and never uploads it; it touches no pin and no protocol rule (it exercises the real `ready` predicate, the real core and the real link). Story 2.3 replaces it with the real profile (the option is then removed or kept for bench use, 2.3's call). *Alternatives rejected:* proving AC2 only on the host and deferring the silicon to Story 2.3's H.3 (an unnecessary gap while the boards are on the bench; this stays the written fallback if the owner declines the option); a probe-side trick (impossible: `ready` is the controller's own state).

**D18. One value, one place; docs and probe are checked against `constants.txt`.**
The six timing constants already exist in `consts.h` and `constants.txt`, and `test_vectors` already compares them. This story adds no constant to the wire set. It adds the two copies that could still drift unnoticed under a check: the numbers in `docs/link-protocol.md` (table and watchdog prose, `docs_constants_check`, F.22) and the probe's own constants (self-test, F.23). The one new value, the release bound `OCB_LINK_RELEASE_WAIT_US` (5000), is local to `ocb_link/config.h` like the 1 ms gate constant, and is written once in the docs.

**D19. No wire change.**
`NO_LINK` 0x15, `LINK_LOST` 0x02, HEARTBEAT 0x40 and `WATCHDOG` debug byte 2 already exist; no message, NAK reason, EVENT code, constant or `PROTO_VERSION` changes, the vectors are not edited. If the dev finds a rule that seems to need one, stop and record an open point (AD-14).

**D20. No Switch 2 is needed; the console-side proof is later.**
Every AC is a controller-side behaviour that the controller itself reports (STATUS `mode`, `abort_count`, `link_ok`, EVENT). The statement "no input stays pressed on the console within 100 ms" additionally needs the USB report path (2.3, 2.5) and the real bridge (3.1) and is measured in 3.5 and A.3. This story proves the controller decision and its reporting.

**D21. Reading of 2.6 D39: `max_app_gap_ms` counts app frames only.**
D39 attributed the saturation at 65535 to the missing heartbeat sender; the counter measures the gap between valid **app** frames (HEARTBEATs never update it, by construction), so heartbeats will not change it. It saturates whenever the bench waits idle without app frames. It is W.1's counter and is not a pass criterion of any 2.7 step; the new probe commands send HELLO at the start of each run, which resets it.

**D22. NFR-05 budget.**
Link loss by silence: 80 ms from the last heartbeat received, plus one loop pass (well under 1 ms), plus one HID report period for the console to see NEUTRAL (8 ms is the 2.4 fallback hypothesis; 2.3 measures it): about 89 ms against the 100 ms cap, a margin of about 11 ms, which is why the default is 80 and the clamp is 100. A break acts at the next pass, so it is far inside the budget. The controller-visible latency is what H.9 measures ([80, 120] ms including the Mac's USB hops and the bridge pipe); the console-side figure is Story 3.5's.

**D23. Poll order, and the release lives behind one extra call.**
`ocb_link_poll`: USB aborts, then the watchdog, then the scheduler, so a dead link never applies a due event. The glue calls `ocb_link_tx_settled()` once per pass after the TX pump with `ocb_uart_link_tx_idle()`; the pure library never reads hardware, so the hardware state arrives as a bool, which keeps the sequence testable on the host (F.21 drives it with fake `tx_hw_idle` values).

**D24. Two recorded bench expectations of Story 2.6 change; the recorded results stay as history.**
H.4 step 6: without a heartbeat, `try` now gives `NAK NO_LINK` (the link was never up); `NAK NOT_READY` needs `--heartbeat` (this is H.8). H.7 step 5: with this story, pulling the peer-TX wire releases TX, so GP0 returns to about 0 V; Story 2.6's recorded "stays at 3.28 V" was correct for the 2.6 firmware and is kept in the test plan as history (the row text gains the new expectation for builds from this story on). H.6 is unchanged for the unarmed case (D8) and gets a pointer to `breaks`.

**D25. Research and external facts.**
No outside source was needed. Facts used: the PL011 `UARTFR.BUSY` bit "remains set until the complete byte, including all the stop bits, has been sent from the shift register" and `TXFE` "does not indicate if there is data in the transmit shift register" (pinned SDK `hardware/regs/uart.h`), which is why `ocb_uart_link_tx_idle()` tests both; break behaviour and the data-register error bits (2.6 D20, SDK `uart.h`); the H.6 and H.7 measurements of Story 2.6 (a pulled wire shows as `BE` plus many `FE`; a controller whose gate is open stays at 3.28 V without a release).

### Behaviour reference (the tables the tests encode)

**Watchdog rules** (all comparisons on the 64-bit µs clock, `elapsed = now >= then ? now - then : 0`, trip when `elapsed > limit`):

| Rule | Armed? | Mode | Condition | Effect | EVENT | Trip |
|---|---|---|---|---|---|---|
| Heartbeat timeout | yes | any | `elapsed(now, last_hb) > link_limit` (80 ms; ≤ 100) | `abort(LINK_LOST)`, `link_ok` 0, disarm, TX release | `LINK_LOST` (unless latched) | yes |
| Break | yes | any | new `BE` entry since last poll | same as above | `LINK_LOST` (unless latched) | yes |
| Break | no | any | new `BE` entry | TX release only | none | no |
| App timeout | n/a (needs an app frame) | DIRECT | `elapsed(now, last_app_frame) > app_limit` (1000 ms) | `abort(APP_TIMEOUT)` | none | yes |
| DTR edge | yes (a heartbeat just arrived) | DIRECT | previous DTR 1, this heartbeat DTR 0 | `abort(APP_TIMEOUT)` | none | yes |
| any of the above | | IDLE | app timeout, DTR | nothing | | |
| app timeout, DTR | | PLAYING | | nothing (underrun covers it) | | |

**NAK precedence for `SET_STATE` / `START`** (first match wins): `ABORTED` → `NO_LINK` → `BUSY` → `NOT_READY` → `BAD_STATE` → (`ORDER`, `FULL` for `QUEUE_EVENT` only). `QUEUE_EVENT` skips `NO_LINK` and `NOT_READY`.

**Loss sequence on the wire** (armed, gate open): `t`: trip → core NEUTRAL, queue empty, latch → EVENT `LINK_LOST` first in TX → hardware drains (or 5 ms) → gate released, TX pad high-Z → RX idle-high for 1 ms → gate open → STATUS with `link_ok` 0 → heartbeat → `link_ok` 1 → app `STOP`.

**States of `link_ok` and arming:**

| Event | `link_ok` | Armed |
|---|---|---|
| boot | 0 | no |
| valid HEARTBEAT | 1 | yes |
| loss (timeout or break) | 0 | no |
| app timeout, DTR edge | unchanged | unchanged |
| HELLO, STOP, abort of any other cause | unchanged | unchanged |

### Consumers (what later stories need from this one)

- **Story 2.3**: nothing new to call. `console_ready` and the USB aborts are untouched; the bench option of D17 is removed or kept by 2.3; `H.3` can reuse the `linkup --expect-ready yes` and `direct` commands.
- **Story 2.5**: `ocb_ctrl_link_step()` now ends with `ocb_link_tx_settled()`; the loop budget gains one comparison per pass; the core-stall abort cause stays 2.5's; the hardware watchdog reboot is separate from this link watchdog.
- **Story 3.1 (bridge)**: the heartbeat period (20 ms) and maximum gap (27 ms) the 80 ms timeout assumes; the DTR bit semantics (bit 0 = CDC DTR asserted); the asymmetric TX rule of AD-20 as amended (the bridge leads with probe pulses, H.1); the controller releases TX after `EVENT LINK_LOST` and reopens after 1 ms idle-high.
- **Story 3.3 (Rust link)**: the Task 8.3 text: recovery by `STOP`, no `START` / `SET_STATE` while `link_ok` is 0, `NAK NO_LINK` is semantic, `PING` every `app_timeout_ms / 4` in DIRECT, assert DTR on open.
- **Story 3.5**: the console-side measurement (20 unplugs, audio stop) builds on H.12.
- **Story 4.x**: STATUS `flags` bit 0 is now the real `link_ok`; the status LED "bridge to controller" may use it directly.
- **Story 7.4 / W.1, W.2**: `watchdog_trips` and `max_app_gap_ms` in the debug STATUS.

### Developer guardrails

- **Read before editing (UPDATE files):** `firmware/controller/core/include/ocb_core/core.h`, `core/modes.c`; `firmware/controller/link/include/ocb_link/{link.h,config.h,txgate.h,internal.h}`, `link/link.c`, `link/tx.c`, `link/txgate.c`; `firmware/controller/platform/link_runner.{c,h}`; `firmware/controller/main.c`, `controller/CMakeLists.txt`; `firmware/common/hw/uart_link.c` and its header; `firmware/CMakePresets.json`; `firmware/tests/{CMakeLists.txt,link_fixture.h,core_fixture.h,link_sim_shim.c,test_link_*.c,test_core_*.c,test_link_probe.py}`; `firmware/tools/link_probe.py`; `.github/workflows/firmware.yml`; `docs/link-protocol.md` ("Watchdog", "Controller modes", "Rules the controller core applies", "Rules the controller link applies (Story 2.6)"), `docs/test-plan.md` (F, H and the bench block). Preserve everything not named here.
- The system must keep working end to end: `controller` and `bridge` still build in `fw-release` and `fw-debug`, both UF2s still come out, the LED still blinks as in Story 2.1 (H.0 behaviour must not regress; check by eye at the bench), H.4 to H.6 still run as recorded (unarmed behaviour is unchanged except the `try` reply, D24).
- Use `ocb_msg_*` encoders and decoders and `ocb_frame_*` only; never cast bytes to structs; never hand-code a frame in the library or in the tests (the probe is the only independent implementation).
- `-Wconversion` is on: cast `uint8_t` arithmetic explicitly; never use `uint32_t` for a time in the watchdog; no `/` or `%` on `uint64_t`.
- Never block: nothing in `ocb_ctrl_link_step()` waits on a FIFO, a flag or the clock. No GPIO number and no `PICO_DEFAULT_UART*` outside `pins.h`; the bench option touches no pin.
- No `printf` or stdio on the controller (the UART pins are the link). Debug information leaves only as STATUS debug counters or EVENT.
- Do not use 2wiCC code or text, do not copy from pico-examples or other firmware. The probe's heartbeat is written from `docs/link-protocol.md`.
- The core is modified only by Task 1. If a core rule looks wrong, record an open point.
- Do not create temp files in the repo; do not commit or push; do not edit `_bmad/`.

### Architecture compliance

- AD-2: pure C99 `ocb_link/watchdog` next to `ocb_proto` and `ocb_core`, no SDK, covered by the existing purity gate; `ocb_hw` holds the one new hardware predicate.
- AD-4: the 20 ms heartbeat and the 27 ms maximum gap are the assumptions behind the 80 ms timeout (documented, checked by `docs_constants_check`, `HEARTBEAT_MAX_GAP_MS` < `LINK_TIMEOUT_MS`).
- AD-5: heartbeat timeout (≤ 100, default 80), break, app timeout in DIRECT only, DTR edge, no app timeout in PLAYING, `NO_LINK` gating; D3 to D12 fix what AD-5 leaves open (arming, boundaries, break definition).
- AD-6 and AD-7: unchanged; `NO_LINK` is a semantic NAK; HELLO and STOP stay sync points and clear the latch.
- AD-8: `LINK_LOST` and `APP_TIMEOUT` abort causes and their EVENT rules exactly as written; the `NO_LINK` precedence slot of 2.4.
- AD-11: the link still runs on core 1, the only TX writer; the new call sits in the same step.
- AD-19: `link_ok` (bit 0) is now live; STATUS debug counters (byte 2) filled; reserved bits stay 0.
- AD-20: TX released on a break and on link loss, EVENT first; the spine text amended (D14).

### Library and framework requirements

None external for the firmware. C99 for the pure libraries (`C_STANDARD 99`, `C_EXTENSIONS OFF`); SDK 2.3.1 `hardware_uart` register access in the glue and driver only (pinned submodule, AD-1; nothing fetched). No test framework (Story 2.2 D7). Python 3 standard library for the probe's tests; `pyserial` only on the owner's bench machine. Cortex-M0+: no hardware divide, no 64-bit atomics (D1).

### File structure requirements

```text
firmware/controller/link/
  include/ocb_link/watchdog.h            (NEW)
  watchdog.c                              (NEW)
  (UPDATE) link.c  tx.c  include/ocb_link/link.h  config.h
firmware/controller/core/
  (UPDATE) include/ocb_core/core.h  modes.c
firmware/controller/platform/link_runner.c            (UPDATE)
firmware/controller/main.c  CMakeLists.txt            (UPDATE: bench option)
firmware/common/hw/uart_link.c  include/ocb_hw/uart_link.h   (UPDATE: tx_idle)
firmware/CMakePresets.json                            (UPDATE: fw-debug-bench)
firmware/tests/
  test_link_watchdog.c                    (NEW, F.21)
  test_docs_constants.py                  (NEW, F.22)
  (UPDATE) link_fixture.h  core_fixture.h  link_sim_shim.c  CMakeLists.txt
           test_link_nak.c  test_link_tx.c  test_link_session.c  test_link_fuzz.c
           test_link_ready.c (and any test the first run flags)
           test_core_modes.c  test_core_edge.c  test_link_probe.py
firmware/tools/link_probe.py                          (UPDATE)
.github/workflows/firmware.yml                        (UPDATE: one compile-gate step)
docs/link-protocol.md  docs/test-plan.md              (UPDATE)
firmware/controller/README.md  firmware/README.md  firmware/THIRD_PARTY_NOTICES.md  (UPDATE)
_bmad-output/planning-artifacts/architecture/architecture-bonooru-2026-11-19/ARCHITECTURE-SPINE.md   (UPDATE, AD-20 text)
_bmad-output/planning-artifacts/epics.md                (UPDATE, two sentences)
_bmad-output/specs/spec-bonooru/risk-register.md        (UPDATE, row C5 clause)
```

Not touched: `common/proto/**` (consts, vectors), `common/hw/include/ocb_hw/pins.h`, `bridge/**`, `_bmad/**`, `docs/ocarina-input-mechanics.md`, the purity gate scripts and their fixtures (no new banned word is needed: `time_us` already bans `time_us_32` and `time_us_64`).

### Testing requirements

All under ctest `host-tests`, ASan and UBSan on, hand-written expectations, frames built with the real encoder.

| Test (ctest name) | Test-plan ID | Must cover |
|---|---|---|
| `test_link_watchdog` | **F.21** (new) | Task 4.2 |
| `docs_constants_check` | **F.22** (new) | Task 7.2 |
| `link_probe_selftest` | **F.19, F.20, F.23** | Task 6.5 (the new commands on a fake clock, negative controls, constants) |
| `test_core_modes`, `test_core_edge` | **F.3, F.12** | Task 1.3 |
| `test_link_nak`, `test_link_tx`, `test_link_session` | **F.2, F.16, F.15** | Task 4.3 |
| `test_link_fuzz` | **F.17** | Task 4.4 |
| `link_pure_check`, `core_pure_check` | **F.18, F.13** | unchanged, must stay green with `watchdog.c` in the tree |

CI mapping: "Host tests" runs all of it; "Build fw-debug-bench" compiles the bench build; the purity steps are unchanged. The ARM builds compile the link and glue with the strict warnings.

### Hardware checks (owner bench session; Task 10)

**Rig (record in the test-plan bench block):** the Story 2.6 rig, unchanged and **not rechecked** (wiring is as left after 2.6: controller Zero powered from the Mac's USB-C; the second Zero running the official 2wiCC_Comms v1.1 as the transparent peer at `/dev/cu.usbmodem101`; crossed TX/RX and GND; nothing on 5V or 3V3; no USB-UART adapter; a multimeter). Controller UF2s are local builds with `OCB_PICOTOOL_DIR=firmware/.tools/picotool-2.3.1` and `OCB_ARM_TOOLCHAIN=firmware/.tools/arm-gnu-toolchain-15.2.Rel1`: **fw-debug** (`build/fw-debug/controller/controller.uf2`) for H.8, H.9, H.10, H.12, and **fw-debug-bench** (`build/fw-debug-bench/controller/controller.uf2`) for H.11 and for H.12 `--mode playing`. Flash by BOOTSEL. Probe: `firmware/tools/link_probe.py` (pyserial), global options before the subcommand: `link_probe.py --port /dev/cu.usbmodem101 <subcommand>`. The Switch 2 is **not** needed. The probe paces writes (2.6 D30); breaks are made by unplugging the bridge-TX to controller-GP1 wire (the RX pull-down turns the open line into a break), never by jumpering to GND (2.6 D37). Record for the session: the commit, the UF2s' build commit, the multimeter model (not recorded in 2.6), and the probe's heartbeat statistics.

| ID | Test | Exact steps (owner actions in **bold**) | Expected | Record |
|---|---|---|---|---|
| H.8 | Link-up, `NAK NO_LINK`, boot state (AC1, AC3) | 1. Flash fw-debug. 2. **Power-cycle the controller** (unplug and replug its USB-C). 3. `link_probe.py --port P linkup --seconds 20 --expect-fresh --expect-ready no` (the command does everything else). | `abort_count` 0 and `last_event` NONE at the start (the boot break did not latch, D3); INFO `bridge_ver` 0.0.0; `try` gives `NAK NO_LINK` twice; `link_ok` 1 within 100 ms of the first heartbeat; a second HELLO shows `bridge_ver` 0.0.1; `try` gives `NAK NOT_READY` twice; for 20 s `link_ok` stays 1, `abort_count` and `watchdog_trips` unchanged; the final line is `PASS`. LED still amber about 2 Hz. | the printed lines, the boot counters, the probe's heartbeat statistics (max send gap, gaps above 27 ms), the LED observation |
| H.9 | Heartbeat timeout and recovery, no physical action (AC1, AC3) | 1. fw-debug. 2. `link_probe.py --port P silence`. | `EVENT LINK_LOST` between 80 and 120 ms after the last heartbeat written (record the number; below 80 is an early trip); STATUS: `link_ok` 0, mode IDLE, `abort_count` + 1, `last_event` LINK_LOST, `free` 512; `SET_STATE` `NAK ABORTED`; `STOP` ACKed; `SET_STATE` `NAK NO_LINK`; heartbeats resume: `link_ok` 1 within 100 ms; `SET_STATE` `NAK NOT_READY`; no `NAK VERSION`; `PASS`. Run it 3 times. | the three elapsed times, the pass lines |
| H.10 | No false trips under traffic (AC1, W.2 in miniature) | 1. fw-debug. 2. `link_probe.py --port P --seed 1 soak --heartbeat --cycles 40 --events 500 --corrupt 0.02 --drop 0.02 --garbage`. 3. `link_probe.py --port P linkup --seconds 300 --expect-ready no`. | Both end in `PASS`; `watchdog_trips` and `abort_count` unchanged and `link_ok` 1 in the last STATUS; the heartbeat statistics are printed. If a trip occurs, the probe's max send gap decides: above 60 ms = host jitter (record it, retry with `--hb-period-ms 10`), otherwise a firmware finding. | the PASS lines, trips before/after, max send gaps, any retry |
| H.11 | DIRECT and PLAYING rules on silicon (AC2, AC1 in PLAYING), bench build | 1. **Flash fw-debug-bench.** 2. `link_probe.py --port P linkup --seconds 5 --expect-ready yes`. 3. `link_probe.py --port P direct`. 4. `link_probe.py --port P playing`. | `console_ready` 1 (the bench build), `SET_STATE` accepted. `direct`: keepalive PINGs keep DIRECT 5 s; with no app frame the abort is seen between 1000 and 1150 ms with no EVENT and `link_ok` 1; the DTR 1 to 0 flip aborts within 100 ms; DTR held at 0 with PINGs does not abort. `playing`: no app frame for longer than 1000 ms and a DTR flip mid-piece do not end PLAYING; the piece ends with `PLAYBACK_DONE`; stopping the heartbeat mid-piece gives `LINK_LOST` in [80, 120] ms and mode IDLE. All `PASS`. | the printed lines and times; confirm the bench UF2 was not left on the board afterwards (re-flash fw-debug) |
| H.12 | Real wire pulls and the TX release (AC1, AD-20) | 1. fw-debug (or the bench build for `--mode playing`). 2. Have the multimeter ready (DC V, red probe on the controller's GP0, black on GND; never on 3V3 or 5V). 3. `link_probe.py --port P breaks --rounds 10 --meter-seconds 15`. The probe prints `UNPLUG ...`: **unplug the bridge-TX to controller-GP1 wire**; in round 1 it prints `READ the multimeter ...`: **read GP0** (about 0 V expected); it prints `REPLUG ...`: **plug the wire back**. Repeat for 10 rounds. 4. Same with `--no-heartbeat --rounds 5` (the unarmed case: no abort, TX still released, session continues). 5. Optional: `--mode playing --rounds 3` on the bench build. | Armed rounds: `EVENT LINK_LOST` within about 100 ms of the unplug, then STATUS silent (TX released); GP0 about 0 V while unplugged (**not** a solid 3.3 V: the 2.6 H.7 step 5 reading of 3.28 V must now be gone); after the replug STATUS resumes and `link_ok` 1 within 1 s; `abort_count` + 1 per round, mode IDLE, `free` 512, `QUEUE_EVENT` `NAK ABORTED`, `STOP` ACKed, 5 events accepted, no `NAK VERSION`. Unarmed rounds: no EVENT, `abort_count` unchanged, GP0 about 0 V while unplugged, session continues (the H.6 result, in one command). Each round `PASS`; final `PASS`. | the multimeter model and reading, per-round times (unplug to EVENT, replug to `link_ok`), counters `rx_be`/`rx_fe` before and after, the optional step |

Safety: the meter in H.12 is the only thing connected to GP0; never connect a probe to 3V3 or 5V as a "reference". The wiring is not rechecked (as left after 2.6); if the owner rewires, the GND continuity check of the test plan applies again. The bench build must not be used for anything but H.11 and H.12 `--mode playing`.

What to record, once, in `docs/test-plan.md`: rows H.8 to H.12 (Result column) and the bench block (commit, UF2 build commits, multimeter model, heartbeat statistics).

### Previous story intelligence (Story 2.6, done; PRs up to #18, merged)

- The pure-library pattern is established and the watchdog follows it without new machinery: a module under `controller/link/`, covered by the existing gate and CI step. Do not add a gate.
- Hand-offs from 2.6 addressed here: HEARTBEAT stored but not acted on (D10 of 2.6) → arming and rules (D3 to D12); `ocb_link_breaks()` sticky counter → D5; `ocb_txgate_release()` reserved for 2.7 → D6, D23; `NAK NO_LINK` slot and the core input → D11; `watchdog_trips` placeholder byte → D12; `max_app_gap_ms` saturation (D39) → D21; AD-20 amendment (D25) → D14; TX gate released only on link loss (D40: an open gate stays open until 2.7) → D6, D8, H.12; H.6's `fill --no-hello` limit (D38) → `breaks` (D16); the 1 ms gate constant stays (D41).
- Review lessons from 2.6 that apply here: (1) every new rule needs a red proof recorded in the Debug Log (Task 9.2 lists them); (2) probe option values that would stall a run are rejected by argparse before a port opens (D36), so the new options use the same guard; (3) bench-tool and firmware behaviour are reproduced on the host with a fake clock (D31): F.23 does it for the new commands; (4) honest Completion Notes: "unverified until pushed", "unverified on hardware"; (5) the bench peer has no flow control and drops silently, so everything the probe writes stays paced; heartbeats are tiny and bypass the pacer deliberately.
- Facts to rely on, not to re-derive: the core's `ocb_core_abort` returns the EVENT only when the latch was not already set (2.4 D5); HELLO and STOP clear the latch and go to IDLE; the session is not closed by aborts; `ocb_link_q_event_` drops the new EVENT when the ring (8) is full and counts it; `ocb_link_tx_next` discards pending output when the gate is closed; the gate opens after RX has been high continuously for 1 ms.
- The pin grep prunes only `tests/pin_check_fixtures` and `common/proto/vectors`: keep GPIO tokens and `PICO_*PIN` macros out of every new file, including the bench hook.

### Git intelligence

Recent commits on `main`: `1f206f7` merge of PR #18 (Story 2.6), `c43592a` record H.4 to H.7 and mark 2.6 done, `fe74037` guard probe CLI options, `892dbe7` cap probe in-flight bytes and split debug drop counters, `bb3bacd` soak deadline and write pacing. Pattern: one story branch (this one: `story/2-7-controller-link-watchdog`), an implementation commit plus review follow-up commits, merge by PR; the owner commits and pushes. `firmware/controller/` today holds `core/` (2.4), `link/` and `platform/` (2.6), `main.c` (LED on core 0, bring-up core 1 loop). Stories 2.3 (USB) and 2.5 (core 1 super-loop, H.2) are still backlog, so this story must not assume either exists (D17, D23).

### Latest tech information

Read from the pinned pico-sdk 2.3.1 submodule: `UARTFR.BUSY` (bit 3) stays set until the last stop bit of the last byte has left the shift register and is set as soon as the TX FIFO is non-empty; `UARTFR.TXFE` (bit 7) reports an empty TX FIFO but not an empty shift register; so "TX idle" is `TXFE && !BUSY`. The data register carries FE, PE, BE, OE with each character; a break loads one zero character with `BE` set and the next character only after the line returns high (2.6 D20); a pad switched away from the UART function keeps the peripheral shifting its FIFO out, only the pin is disconnected (the basis of D6's "no flush"; to be seen at the bench as GP0 staying ≈ 0 V after the release, H.12). Toolchain pins unchanged (Arm GNU Toolchain 15.2.Rel1, CMake ≥ 3.25, pico-sdk 2.3.1, TinyUSB 0.18.0).

### Project Context Rules

No `project-context.md` exists in the repo. Project rules that apply, from the owner's notes and the repo: all repo files in English (chat in Italian); never commit `_bmad/` or `bmad-*`/`gds-*` skills (only `_bmad-output/`); never edit `~/Documents/org/`; no game screenshots, never reference `ux-designs/*/imports/`; no binaries and no 2wiCC code (NFR-07, ADR-004); do not create temp files in the repo; the dev agent does not commit or push; breadboard GP0/GP1/GND wiring is not rechecked each session unless rewired; ask one decision at a time (the owner has ADHD: keep the bench steps few, simple and one command each).

### References

- [Source: `_bmad-output/planning-artifacts/epics.md`#Story 2.7, #Story 2.6, #Story 2.3, #Story 2.5, #Story 3.1, #Story 3.5, #Epic 2 Notes]
- [Source: `_bmad-output/planning-artifacts/architecture/architecture-bonooru-2026-11-19/ARCHITECTURE-SPINE.md`#AD-4, AD-5, AD-6, AD-7, AD-8, AD-11, AD-19, AD-20]
- [Source: `docs/link-protocol.md`#Watchdog, #Reliability (AD-6), #Controller modes (AD-8), #Rules the controller core applies (Story 2.4), #Rules the controller link applies (Story 2.6), #Timing constants]
- [Source: `docs/test-plan.md`#Hardware safety, #Test environment, #Bench rig for the controller link, #Watchdog tests W.1 W.2, #Firmware host tests F.0 to F.20, #Hardware-in-the-loop checks H.0 to H.7, A.3]
- [Source: `_bmad-output/specs/spec-bonooru/requirements.md` (NFR-05), `decisions.md`, `risk-register.md` (C5)]
- [Source: `_bmad-output/implementation-artifacts/2-6-controller-link-uart-reliability-session-and-status.md`#Decisions D1 to D42, #Consumers, #Hardware checks, #Review Findings]
- [Source: `_bmad-output/implementation-artifacts/2-4-controller-core-modes-queue-scheduler-and-safety-mask.md`#Decisions (D3 precedence, D5 abort causes)]
- [Source: `firmware/common/proto/include/ocb_proto/{consts,frame,msgs,err}.h`, `common/proto/vectors/constants.txt`; `firmware/controller/core/{include/ocb_core/core.h,modes.c}`; `firmware/controller/link/{include/ocb_link/*.h,link.c,tx.c,txgate.c}`; `firmware/controller/platform/link_runner.c`; `firmware/controller/main.c`; `firmware/common/hw/uart_link.c`; `firmware/tools/link_probe.py`; `firmware/tests/{link_fixture.h,link_sim_shim.c,test_link_probe.py,CMakeLists.txt}`; `.github/workflows/firmware.yml`]
- [Source: pico-sdk 2.3.1, `src/rp2040/hardware_regs/include/hardware/regs/uart.h` (UARTFR.BUSY, TXFE), `src/rp2_common/hardware_uart/include/hardware/uart.h` (read from the submodule)]

### Open points for the owner (none block the story)

- **D3:** the watchdog is armed by the first heartbeat (a controller that never saw one does not time out). If you want "since boot", the cost is a latch and an `abort_count` of 1 at every power-up without a bridge.
- **D14:** this story edits the spine (AD-20), epics (two sentences) and the C5 row of the risk register, text only. Say if you would rather leave the risk-register row alone.
- **D17:** the bench-only build (`fw-debug-bench`) fakes `console_ready` so DIRECT and PLAYING can be tested on the boards today. If you decline it, H.11 moves to Story 2.3 and AC2 is host-only until then.
- **D15:** the Mac sends the bridge's heartbeat; if its scheduler stalls more than 60 ms the controller trips correctly and the run looks like a firmware failure. The probe prints its own gap statistics so this can be told apart.
- **D1:** 64-bit microseconds instead of a 32-bit millisecond timer; the wrap tests cover both boundaries. Say if you want the 32-bit form.
- **Risk:** the EVENT pulse on release (174 µs into a peer that may be gone) is what the AC asks for; its effect on an unpowered bridge is read in H.1 (Story 3.1).

## Dev Agent Record

### Agent Model Used

Claude Opus (claude-opus-5-5), gds-dev-story workflow, run autonomously on 2026-11-22. Open points D1, D3, D14, D15, D17 and the 174 µs EVENT pulse accepted as recommended in the story.

### Debug Log References

**First run after Task 3 (fixture fallout, Task 4.1).** With the watchdog wired in and `link_ok` reaching the core, 6 link tests failed: `test_link_session`, `test_link_gbn`, `test_link_nak`, `test_link_tx`, `test_link_ready` (START / SET_STATE now `NAK NO_LINK`: no heartbeat in the fixture; UBSan first flagged the uninitialised `env.link_ok` in `make_env`), then `test_link_tx` (STATUS cases moving the clock 50 ms at a time) and `test_link_ready` (the 100 ms `ready` stall and 300 × 1 ms loops). Fixed by `lh_link_up()` in `lh_session()` / the ready setup and by replacing every `h.now +=` in those tests with `lh_advance()`. `test_link_fuzz` passed unchanged but only because the link never came up; Task 4.4 then added heartbeats and the new invariants. One own test bug found on the way: `get_status()` in `test_link_watchdog.c` kept `extra` pointing into a stack buffer (fixed: static buffer); the 10 s PLAYING case first polled every 10 ms, above the 8 ms LATE threshold (fixed: 1 ms polls).

**Red checks (Task 9.2)**, each a temporary edit through a scratchpad script (backup, edit, rebuild, ctest, restore), first failing line quoted:

- (a) link timeout `>` → `>=`: `test_link_watchdog.c:203: FAIL: gap 80000 us tripped`.
- (b) clamp removed: `test_link_watchdog.c:229: FAIL: cfg 101: effective 101, wanted 100`.
- (c) armed at boot: `test_link_watchdog.c:195: FAIL: 10^6 heartbeat periods: trips 1 aborts 1 link_ok 1` (the hours-of-silence case fails too).
- (d) FE counted as the break instead of BE: `test_link_watchdog.c:428: FAIL: FE/PE/OE aborted something`.
- (e) `link_ok` not cleared on loss: `test_link_session.c:280: FAIL: link loss closed the session` (the loss check), `test_link_tx.c:487: FAIL: after the loss: STATUS link_ok 0, watchdog_trips 1`, plus `test_link_fuzz` and `test_link_watchdog`.
- (f) TX released before the EVENT (gate closed in the watchdog step): `test_link_watchdog.c:214: FAIL: 80 ms + 1 us: 0 LINK_LOST in 0 frames`.
- (g) never released: `test_link_watchdog.c:385: FAIL: latched core: TX not released`, and `link_probe_selftest` fails (the model's multimeter check).
- (h) app timeout in PLAYING too: `test_link_watchdog.c:779: FAIL: PLAYING: aborted before END (mode 0 trips 1)`.
- (i) app timeout in IDLE too: `test_link_fuzz.c:122: FAIL: fuzz invariant: an app-timeout or DTR trip outside DIRECT`, and `test_link_watchdog` fails.
- (j) DTR on a level: `test_link_watchdog.c:827: FAIL: non-edge DTR patterns aborted`; DTR also in PLAYING: `test_link_fuzz.c:122: ... DTR trip outside DIRECT` and `test_link_watchdog` fails.
- (k) `NO_LINK` after `BUSY` (SET_STATE): `test_core_modes.c:400: FAIL: SET_STATE/PLAYING link down: kind 2 NAK 0x11`; `NO_LINK` before `ABORTED` (START): `test_core_modes.c:424: FAIL: START: ABORTED must precede NO_LINK (0x15)` and `test_link_watchdog.c:135: ... wanted one NAK 0x17 ... reason 0x15`.
- (l) elapsed without the `now >= then` guard: `test_link_watchdog.c:175: FAIL: backwards is 0`, then `:975: FAIL: backwards step 1000000 us: tripped`.
- (m) deadline as `(uint32_t)now > (uint32_t)(then + limit)`: `test_link_watchdog.c:195: FAIL: 10^6 heartbeat periods: trips 16` (the u32 wrap inside the 20 000 s run) and the wrap scenarios.
- (n) watchdog after `ocb_core_tick`: `test_link_watchdog.c:406: FAIL: due event applied in the tripping pass (last_event 1, publishes 2)`.
- (o) skipped with a note: the link records the heartbeat time at receipt (`ocb_link_rx_entry` time); in the tests and on the target the drain and the poll of one pass share the same `now`, so a "time at poll" variant cannot move the boundary; covered by (a).
- (p) doc table 80 → 81: `docs_constants_check: FAIL timing table: OCB_LINK_TIMEOUT_MS is 81 in the doc, 80 in constants.txt`; also a prose sentence (80 → 81: `'exactly 80 ms is still alive, 81 ms + 1 µs trips' states 81`) and the controller-only value (5000 → 4000: `` `OCB_LINK_RELEASE_WAIT_US` written as 4000, the value is 5000``).
- (q) probe `LINK_TIMEOUT_MS = 81`: `AssertionError: 81 != 80 : probe LINK_TIMEOUT_MS differs from constants.txt`.
- (r) the negative controls are not vacuous: `silence`'s loss-window check forced true → `test_silence_negative_control` fails (`0 != 1`); `linkup`'s hold check forced true → `test_linkup_negative_controls` fails (the 100 ms period run returned 0); `breaks`' replug-timeout message changed → `test_breaks_never_replugged` fails.
- Extra: release while EVENTs are pending: `test_link_watchdog.c:536: FAIL: released before the EVENT left`.
- Purity gate on the new file: `#include "hardware/timer.h"` appended to `watchdog.c` → `check-link-pure.sh` rc 1 naming `watchdog.c:96`; a comment naming `time_us_64()` → rc 1. Restored: `check-link-pure: ok (15 files)`.
- Bench build guards: `cmake --preset fw-release -B <scratch> -DOCB_BENCH_FAKE_READY=ON` → `CMake Error at controller/CMakeLists.txt:15: OCB_BENCH_FAKE_READY needs OCB_DEBUG_BUILD=ON`; compiling `controller/main.c` with the fw-release command plus `-DOCB_BENCH_FAKE_READY=1` → `main.c:39:2: error: #error "bench build is debug only"`.

All edits restored; afterwards the full suite is green again (below).

### Completion Notes List

**What was built.** The AD-5 link watchdog as a pure C99 module `ocb_link/watchdog` (arming, 80 ms timeout clamped to 100, break, DIRECT app timeout, DTR edge, wrap-safe elapsed helper, `trips`), wired into `ocb_link` (poll order USB → watchdog → scheduler; loss = abort LINK_LOST + EVENT + `link_ok` 0 + TX release; EVENT-only output while the release is pending; `ocb_link_tx_settled()`), `NAK NO_LINK` in the core from `env.link_ok`, STATUS `watchdog_trips`, the driver predicate `ocb_uart_link_tx_idle()`, the glue call, the bench-only build `fw-debug-bench`, the probe's heartbeat service and the five one-command bench steps, the docs (link-protocol rules, test-plan F/H rows and bench block, READMEs, notices) and the AD-20 amendment (spine, epics, risk register).

**Local verification (2026-11-22, macOS arm64, Apple clang, from `firmware/` with `OCB_PICOTOOL_DIR=$PWD/.tools/picotool-2.3.1 OCB_ARM_TOOLCHAIN=$PWD/.tools/arm-gnu-toolchain-15.2.Rel1`):**

- `cmake --preset host-tests && cmake --build --preset host-tests && ctest --preset host-tests` (fresh build dir): `100% tests passed out of 25` (the 23 of Story 2.6 plus `test_link_watchdog` and `docs_constants_check`); host build 0 warnings. ASan and UBSan active: `-fsanitize=address,undefined -fno-sanitize-recover=undefined` in every host-test compile command (the ctypes module `ocb_link_sim` is built without sanitizers, as in 2.6). `link_probe_selftest` now runs 25 Python tests (was 14), still one ctest.
- `fw-release`, `fw-debug`, `fw-debug-bench` (`--clean-first`): 0 warnings each with the strict flags; `controller.uf2` and `bridge.uf2` out for all three. The bench define reaches `controller/main.c` only in `fw-debug-bench` (checked in the ninja commands).
- `check-pins.sh`, `check-proto-pure.sh`, `check-proto-version.sh`, `check-core-pure.sh`, `check-link-pure.sh` (15 files): all ok. `actionlint` and `shellcheck` clean.
- Sizes (`arm-none-eabi-size controller.elf`, text/bss): fw-release 22092 / 11488, fw-debug 31408 / 11484, fw-debug-bench 31456 / 11484. Stack (`-fstack-usage`, fw-release, scratch build): `ocb_ctrl_link_step` 288 B (2.6 M-6: 304 B), `ocb_link_poll` 88 B, `ocb_link_tx_next` 160 B, `ocb_link_tx_settled` 24 B, `ocb_wd_check` 24 B; informational for Story 2.5.
- CI: one new step "Build fw-debug-bench (compile gate for the bench-only build)"; host tests already run every ctest (so F.21, F.22, F.23 run in CI); no artifact change. **Unverified until pushed.**
- Hardware: H.8 to H.12 **unverified on hardware** until Task 10 is recorded (Task 10 left unchecked, owner's bench session).

**Files outside the dev agent's usual remit (Task 8.2, D14):** `_bmad-output/planning-artifacts/architecture/architecture-bonooru-2026-11-19/ARCHITECTURE-SPINE.md` (AD-20 heading tag, TX-release and Verification bullets, exactly the story's text), `_bmad-output/planning-artifacts/epics.md` (the AD-20 summary line and Story 3.1's last Then), `_bmad-output/specs/spec-bonooru/risk-register.md` (row C5 clause; status stays `open`). Text only.

**Decisions made during development (D26+):**

- **D26. Break and heartbeat in the same drain.** Breaks act at the next poll (D5) and the heartbeat is handled during the drain, before that poll; so the loss is decided at the poll and `link_ok` returns with the *following* heartbeat (20 ms later), not the one in the same drain. Making the heartbeat handler process a pending break first would move loss handling into the RX path; not worth it. F.21 tests this order.
- **D27. `ocb_wd_elapsed_us()` is public** (declared in `watchdog.h`, not `static`): Task 3.6 asks the TX-release deadline in `link.c` to use the same wrap-safe helper, so it is the one time-arithmetic function of both files.
- **D28. Verdict shape.** `ocb_wd_check()` returns only the loss bit when a loss fires (the loss aborts the core anyway; evaluating the app timeout in the same call could count a second trip), adds `OCB_WD_RELEASE` for a break while not armed, and `ocb_wd_init()` takes the link's break counter so a break counted before init is not a new one.
- **D29. Release when the gate is already closed** calls `ocb_txgate_release()` at once (it also restarts a 1 ms high run in progress, harmless: a break means RX was low) and leaves nothing pending.
- **D30. API.** `ocb_link_set_link_ok()` removed (nothing used it); added read-only views `ocb_link_watchdog_trips()` and `ocb_link_release_pending()` (tests, probe shim); `cfg.link_timeout_ms` also stores the effective value back.
- **D31. Fixture heartbeat body** `bridge_fw_ver` 0.0.0 (the probe uses 0.0.1): keeps every 2.6 INFO expectation; `lh_session()` brings the link up after the HELLO; `lh_init()` leaves it down as required.
- **D32. Fuzz break density.** The 2.6 fuzz's random error entries carry BE in 8 of 15 flag combinations, which kept the link almost always down once armed (DIRECT reached a handful of times). BE is now kept in 1 of 10 random error entries, and an explicit break operation is added; each seed must see both losses and app/DTR trips (about 2700 losses and 8 to 11 app/DTR trips per seed).
- **D33. Probe structure.** The heartbeat is a mixin `HeartbeatService` used by `Port` (and called explicitly by the test model, because the probe module is loaded at run time); new steps share a small `Link` session (seq numbering, every STATUS / EVENT / ACK / NAK recorded) and `Checks` (one line per check, one `PASS` / `FAIL`). `main()` starts the heartbeat for the 2.6 subcommands only with `--heartbeat`; the new subcommands start it themselves at the right moment (`linkup` after its first `try`). `breaks --no-heartbeat` is a subcommand option (dest distinct from the global `--no-heartbeat`). `silence` sends its 5 events one by one, each waiting for its ACK (the content of `fill --count 5` without a second session). `breaks`' wait limits (60 s, 30 s) are module constants so the self-test can shorten the never-replug control. `--seconds`, `--rounds`, `--count` are range-checked by argparse like `--hb-period-ms`.
- **D34. Extra checks and controls.** `linkup`'s hold also requires at least 10 STATUS per second; F.23 adds two negative controls beyond the story's three (`--expect-fresh` against a latched controller, a narrowed loss window in `silence`) and a by-hand test of the heartbeat schedule.
- **D35. Bridge model for F.23.** The 2.6 model gains the probe's heartbeat (serviced every 100 µs tick and at each poll), a "wire plugged" flag (unplug = one BE entry, bytes to the controller lost, RX level low), `sim_rx_level` / `sim_tx_settled` each tick, the driver's ring emptied when the gate closes (as `ocb_uart_link_tx_enable(false)` does), an optional boot break and an optional ready console. The scripted user acts 200 ms after each prompt and, at the multimeter prompt, records whether the gate is open (GP0 driven).
- **D36. Text edits, as worded.** Story 3.1's Then: the whole clause "TX stays high-Z until RX sees idle-high, and TX is released again on a break or link loss" was replaced by the story's new text (which already says "returns to probing on a break or link loss"), to avoid a repeated clause. Risk register C5: only the named clause was replaced, so the cell still begins "symmetric (AD-20):", which no longer describes the amended rule; left for the owner.
- **D37. Two guards for the bench build**: CMake refuses `OCB_BENCH_FAKE_READY` without `OCB_DEBUG_BUILD` at configure time, and `main.c` keeps the `#error` the story asks for (proven red with a direct compile).
- **D38. F.22 also checks controller-only values** written in the 2.7 section (`OCB_LINK_RELEASE_WAIT_US`) against `ocb_link/config.h`, so the one new value cannot drift either.

**Bench commands for Task 10 (owner):** exactly the H.8 to H.12 rows of `docs/test-plan.md` (port `/dev/cu.usbmodem101`).

### File List

Added:

- `firmware/controller/link/include/ocb_link/watchdog.h`
- `firmware/controller/link/watchdog.c`
- `firmware/tests/test_link_watchdog.c`
- `firmware/tests/test_docs_constants.py`

Modified:

- `.github/workflows/firmware.yml`
- `_bmad-output/implementation-artifacts/2-7-controller-link-watchdog.md`
- `_bmad-output/implementation-artifacts/sprint-status.yaml`
- `_bmad-output/planning-artifacts/architecture/architecture-bonooru-2026-11-19/ARCHITECTURE-SPINE.md`
- `_bmad-output/planning-artifacts/epics.md`
- `_bmad-output/specs/spec-bonooru/risk-register.md`
- `docs/link-protocol.md`
- `docs/test-plan.md`
- `firmware/CMakeLists.txt`
- `firmware/CMakePresets.json`
- `firmware/README.md`
- `firmware/THIRD_PARTY_NOTICES.md`
- `firmware/common/hw/include/ocb_hw/uart_link.h`
- `firmware/common/hw/uart_link.c`
- `firmware/controller/CMakeLists.txt`
- `firmware/controller/README.md`
- `firmware/controller/core/include/ocb_core/core.h`
- `firmware/controller/core/modes.c`
- `firmware/controller/link/CMakeLists.txt`
- `firmware/controller/link/include/ocb_link/config.h`
- `firmware/controller/link/include/ocb_link/link.h`
- `firmware/controller/link/link.c`
- `firmware/controller/link/tx.c`
- `firmware/controller/main.c`
- `firmware/controller/platform/link_runner.c`
- `firmware/tests/CMakeLists.txt`
- `firmware/tests/core_fixture.h`
- `firmware/tests/link_fixture.h`
- `firmware/tests/link_sim_shim.c`
- `firmware/tests/test_core_edge.c`
- `firmware/tests/test_core_modes.c`
- `firmware/tests/test_link_fuzz.c`
- `firmware/tests/test_link_nak.c`
- `firmware/tests/test_link_probe.py`
- `firmware/tests/test_link_ready.c`
- `firmware/tests/test_link_session.c`
- `firmware/tests/test_link_tx.c`
- `firmware/tools/link_probe.py`

## Change Log

- 2026-11-22: Story 2.7 created (ready-for-dev) by the create-story workflow, run autonomously. Decisions D1 to D25 recorded; AD-20 amendment of Story 2.6 D25 scheduled as Task 8.1.
- 2026-11-22: Tasks 1 to 9 implemented by the dev-story workflow (watchdog module, link wiring, NAK NO_LINK, TX release, bench-only build, probe heartbeat and one-command bench steps, F.21 to F.23, docs, AD-20 amendment); decisions D26 to D38 recorded; host tests 25/25 green, three ARM presets with 0 warnings; status review. Task 10 (owner bench session, H.8 to H.12) open.
