---
baseline_commit: fb27406bb3ca80ce5b169ab739182affcc7d63c4
---

# Story 2.4: Controller core: modes, queue, scheduler and safety mask

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As the owner,
I want the controller's playback logic as plain C tested on my PC,
so that the rules that keep inputs safe are proven before they touch the console.

## Acceptance Criteria

Copied verbatim from `_bmad-output/planning-artifacts/epics.md`, Story 2.4. These are the definition of done.

**AC1.**
**Given** `firmware/controller/core/` with no pico-sdk includes and a fake clock
**When** the `host-tests` preset runs
**Then** F.3 passes: every frame × IDLE/DIRECT/PLAYING follows the AD-8 table; entering IDLE publishes NEUTRAL, clears the queue and resets ordering; the abort latch is set by every unrequested cause and cleared only by STOP/HELLO; `NAK ABORTED` while set
**And** F.4 passes: `NEUTRAL|END` → `PLAYBACK_DONE`; END on non-neutral → `NAK BAD_STATE`; reserved flag bits must be 0; underrun judged at apply time → NEUTRAL in the same pass + `UNDERRUN`; head event overdue > one report period → `LATE`, no backlog fired; `t_us` strictly increasing and < 2^31 else `NAK ORDER`
**And** F.5 passes: HELLO/STOP during PLAYING with any seq → NEUTRAL, queue cleared, IDLE, `expected = seq+1`
**And** F.6 passes: in PLAYING every non-allowed button is cleared and the right stick centred (allow-list L, R, A, X, Y, hat, left stick, plus any input added from Story 1.1's U11 result); DIRECT is unmasked
**And** `START`/`SET_STATE` get `NAK NOT_READY` while the profile's `ready()` is false

### Reading notes on the AC (what the story file adds, none of it changes the AC)

- **There is one AC with five "And" clauses.** For the dev agent they are five independently verifiable blocks: F.3 (mode table and abort latch), F.4 (end of piece, underrun, LATE, ordering, flags), F.5 (sync points in PLAYING), F.6 (safety mask) and the NOT_READY gate. Each block gets its own test executable (D10) so a failure names the block.
- **"plus any input added from Story 1.1's U11 result"**: the U11 result exists and adds nothing. Story 1.1 recorded it by hand on 2026-11-20 in `docs/ocarina-input-mechanics.md` (table "Unknowns", row U11) and `docs/test-plan.md` (section "U11 other buttons in free play"): ZL opens the song menu (ZL + A previews a song), ZR does nothing, the right stick moves the camera, no other button or octave is a note input, and the AD-9 allow-list (L, R, A, X, Y, hat, left stick) is **confirmed as is**; "masking ZL and the right stick is desirable". So the allow-list of this story is exactly AD-9, there is no TODO to add an input, and the test list must prove ZL, ZR and the right stick are masked (D7).
- **"a fake clock"**: the core never reads a clock. Every call receives `now_us` (D1), so a test advances time by passing a larger number. There is nothing to fake inside the core, only a tiny helper in the tests.
- **"`expected = seq+1`" (F.5) lives in the core, the rest of go-back-N does not.** AD-8 and the AC tie `expected` to the sync points, so the core keeps the `expected` counter and updates it (D2). Accepting only `seq == expected`, the stale window, cached responses and the transport NAKs `CRC`/`SEQ` are Story 2.6 (F.1, F.2).
- **"the abort latch is set by every unrequested cause"**: the causes that come from outside the core (link loss, app timeout, USB suspend/unmount/bus reset, core stall) are *detected* by Stories 2.3 (USB), 2.6, 2.7 and 2.5. This story provides the one entry point they call, `ocb_core_abort(cause)`, and proves it for every cause (D5). The causes that the core itself detects (END applied, underrun, LATE) are implemented here.
- **"the profile's `ready()`"** is an input of every core call (`env.ready`), not a function the core calls (D1). Story 2.3 writes the real `ready()`; here a test sets the boolean.
- **"scheduler"**: in this story the scheduler is `ocb_core_tick(now_us)`: pure, called by whoever owns the 1 MHz timer. The core 1 super-loop, the seqlock double buffer and the hardware timer are Story 2.5.

## Scope boundaries (what is NOT in this story)

| Not here | Where it is |
|---|---|
| USB, TinyUSB, `build_report`, the profile's real `ready()` | Story 2.3 |
| Core 1 super-loop, 1 MHz hardware timer, seqlock double buffer, RAM placement of the code, hard-coded timeline, jitter self-timing, H.2 stall test | Story 2.5 |
| UART, `ocb_rx` wiring, go-back-N (accept only `seq == expected`, stale window, cached response, NAK `CRC`/`SEQ` at most once per `expected`), HELLO `proto_ver` check, INFO, `NAK VERSION` before HELLO, building STATUS (every 50 ms, coalescing, `flags` bits), TX priority, F.1, F.2, F.10 | Story 2.6 |
| Link watchdog (`HEARTBEAT` timeout, UART break, app timeout in DIRECT, DTR edge), `NAK NO_LINK`, `link_ok` | Story 2.7 (it calls `ocb_core_abort`; D5) |
| Bridge, Rust codec, app | Epics 3 to 6 |
| Any wire change: no new message, constant or `PROTO_VERSION` bump; `consts.h` and the vectors are not edited (D11) | n/a |

## Tasks / Subtasks

All tasks are for the dev agent. **No task needs hardware** (D15): every AC is verifiable on the host with `host-tests`. The dev agent does not commit or push; the owner commits and pushes, and CI is verified afterwards by the owner's push (as in Stories 2.1 and 2.2). Keep the Completion Notes honest: anything CI-only is "unverified until pushed".

- [x] **Task 1: `ocb_core` library skeleton and build wiring** (AC: all; Decisions D1, D11, D12)
  - [x] 1.1 Create `firmware/controller/core/CMakeLists.txt`: `add_library(ocb_core STATIC queue.c mask.c modes.c sched.c)`, public include dir `include/` (so `#include "ocb_core/core.h"` works), `target_link_libraries(ocb_core PUBLIC ocb_proto)`, `C_STANDARD 99`, `C_EXTENSIONS OFF`, `ocb_strict_warnings(...)` plus `-Wpedantic -Wshadow -Wconversion` exactly as in `common/proto/CMakeLists.txt`. It links nothing else: no `pico_*`, no `ocb_hw`, no `ocb_version`.
  - [x] 1.2 `firmware/CMakeLists.txt` (UPDATE; today it does `add_subdirectory(common/proto)`, `common/hw`, then `bridge` and `controller` in firmware builds or `tests` in host builds): add `add_subdirectory(controller/core)` right after `common/hw`, **in both build modes**, so the host tests see it and the ARM build compiles it too. Do not touch the sanitizer, toolchain or preset logic.
  - [x] 1.3 `firmware/controller/CMakeLists.txt` (UPDATE; today `controller` links `ocb_hw ocb_proto ocb_version`): add `ocb_core` to `target_link_libraries`. `main.c` stays the LED blinker (Story 2.1); nothing calls the core yet. The point is that `ocb_core` is compiled by `arm-none-eabi-gcc` with `-Wall -Wextra -Werror -Wpedantic -Wshadow -Wconversion` in `fw-release` and `fw-debug` with zero warnings (catches, for example, an implicit 64-bit division, D1).
  - [x] 1.4 Headers under `controller/core/include/ocb_core/`: `core.h` (the `ocb_core` struct, `ocb_core_env`, `ocb_core_out`, the API of D1, `OCB_CORE_*` constants and the `OCB_CORE_RAMFUNC` macro of D12), `queue.h`, `mask.h`, plus an umbrella `ocb_core.h`. Allowed includes only: `<stdint.h> <stddef.h> <stdbool.h> <string.h> <limits.h>`, `"ocb_proto/..."`, `"ocb_core/..."`. SPDX header `MIT`, include guards, no GPIO tokens in code or comments (the pin grep scans `firmware/controller/`).
  - [x] 1.5 Rules for every source: no heap, no recursion, no global mutable state (two `ocb_core` instances must be fully independent: a test proves it, 7.5), no `printf`, no libc beyond `string.h`, no `/` or `%` on `uint64_t` (libgcc 64-bit division is slow on Cortex-M0+; only `+`, `-`, comparisons), no dependence on host endianness or struct layout.

- [x] **Task 2: Event queue** (AC: #1 F.3 "clears the queue", F.4 "strictly increasing"; D1, D3, D4)
  - [x] 2.1 `queue.h/.c`: `typedef struct { uint32_t t_us; ocb_state state; } ocb_core_event;` and a fixed-capacity ring over caller-provided storage: `int ocb_queue_init(ocb_queue *q, ocb_core_event *storage, uint16_t cap)` (returns 0, or negative for `cap == 0` or `storage == NULL`; `cap` is a `uint16_t` because `INFO.queue_cap` and `STATUS.free` are u16), `bool ocb_queue_push(...)` (false when full, nothing written), `const ocb_core_event *ocb_queue_peek(...)` (NULL when empty), `bool ocb_queue_pop(ocb_queue *q, ocb_core_event *out)`, `void ocb_queue_clear(...)`, `uint16_t ocb_queue_count(...)`, `uint16_t ocb_queue_free(...)`. Index wrap by compare-and-reset, not `%`.
  - [x] 2.2 Production capacity is `OCB_QUEUE_CAP_DEFAULT` (512, `consts.h`): 512 events x 12 bytes = 6 KB of RAM, owned by Story 2.5/2.6 when they instantiate the core. This story does not allocate it; tests use their own arrays (cap 1, 2, 4 and 512).

- [x] **Task 3: Safety mask (F.6)** (AC: #1 F.6; Decision D7)
  - [x] 3.1 `mask.h/.c`: `#define OCB_CORE_ALLOWED_BUTTONS (OCB_BTN_L | OCB_BTN_R | OCB_BTN_A | OCB_BTN_X | OCB_BTN_Y)` (compile-time, from the `OCB_BTN_*` of `consts.h`; **no other place lists the allow-list**), and `ocb_state ocb_core_mask_state(const ocb_state *in)` returning a copy with `buttons &= OCB_CORE_ALLOWED_BUTTONS`, `hat` unchanged, `lx`/`ly` unchanged, `rx = ry = OCB_STICK_CENTER`, `flags = 0` (flags are never published, D8). A C99 negative-array compile-time check that `OCB_CORE_ALLOWED_BUTTONS` contains no bit outside `L|R|A|X|Y` is not needed (the macro is the list); do add a comment citing AD-9 and the U11 result (`docs/ocarina-input-mechanics.md`, row U11).
  - [x] 3.2 Adding an input later is a one-line firmware change plus its test row; the header comment says so (AD-9 "takes a firmware change").

- [x] **Task 4: Modes, frame handling and the abort latch (F.3, F.5, NOT_READY)** (AC: #1 F.3, F.5, NOT_READY; Decisions D1 to D5, D8, D14)
  - [x] 4.1 `core.h`: `ocb_core_init(ocb_core*, ocb_core_event *storage, uint16_t cap, uint16_t report_period_us)`; fields (names are a suggestion, behaviour is not): `mode`, `aborted`, `abort_count`, `last_event`, `expected` (u8), `published` (`ocb_state`), `publish_count` (u32), `t0_us` (u64), `last_t_us`, `have_last_t`, `end_queued`, `late_us`, the queue. Initial state: IDLE, not aborted, `abort_count 0`, `last_event OCB_EVT_NONE`, `expected 0`, `published = OCB_NEUTRAL`, empty queue. `ocb_core_set_report_period(core, us)`: 0 selects `OCB_CORE_LATE_FALLBACK_US` (8000), D6.
  - [x] 4.2 `ocb_core_env { uint64_t now_us; bool ready; }` and `ocb_core_out { ocb_core_reply_kind kind; uint8_t nak_reason; uint8_t event; }` with `kind` in `OCB_CORE_REPLY_NONE | ACK | NAK | HELLO` and `event` an `OCB_EVT_*` (0 = none to send). `void ocb_core_frame(ocb_core*, const ocb_core_env*, const ocb_frame*, ocb_core_out*)` implements exactly the table and the check order of D3 and D14. Bodies are parsed with the existing `ocb_msg_*_decode` of `ocb_proto/msgs.h`; never re-read body bytes by hand. A frame handled by the core must also leave `expected` right: it advances to `seq+1` after an ACK or a **semantic** NAK, stays after the transport NAK `FULL`, and is set to `seq+1` by HELLO and STOP (D2).
  - [x] 4.3 One private `enter_idle(core)` used by STOP, HELLO and every abort: publishes `OCB_NEUTRAL`, clears the queue, resets the ordering baseline (`have_last_t = false`, `end_queued = false`), sets `mode = IDLE`. Nothing else may set `mode = IDLE`; the F.3 and F.12 invariant checks (IDLE implies NEUTRAL published, empty queue) catch a path that skips it.
  - [x] 4.4 `void ocb_core_abort(ocb_core*, ocb_core_abort_cause, ocb_core_out*)`: causes `OCB_ABORT_LINK_LOST`, `OCB_ABORT_APP_TIMEOUT`, `OCB_ABORT_USB_SUSPEND`, `OCB_ABORT_USB_UNMOUNT`, `OCB_ABORT_USB_BUS_RESET`, `OCB_ABORT_CORE_STALL`, plus the two internal ones used by the scheduler (`END`, `UNDERRUN`, `LATE`; same internal helper). Behaviour per D5.
  - [x] 4.5 Read-only snapshot for STATUS (Story 2.6 formats it): `void ocb_core_status(const ocb_core*, uint64_t now_us, ocb_core_status_view *v)` with `free`, `now_us32` (µs since t0 as u32, 0 when IDLE or during the lead), `last_seq` (`expected - 1`), `mode`, `abort_count`, `last_event`. No flags: `link_ok` and `console_ready` come from outside (2.6, 2.7).

- [x] **Task 5: Scheduler tick (F.4)** (AC: #1 F.4; Decision D6)
  - [x] 5.1 `sched.c`: `void ocb_core_tick(ocb_core*, const ocb_core_env*, ocb_core_out*)` per D6. Does nothing unless `mode == PLAYING` and `now_us >= t0_us` (the lead). Applies **at most one** event per call; the caller loops as fast as it can (Story 2.5: < 50 µs per pass).
  - [x] 5.2 LATE check before apply, END and underrun handling after apply, all as in D6; every outcome is deterministic for a given `(state, env, frame)` sequence (no randomness, no time reads).
  - [x] 5.3 Applied states are `ocb_core_mask_state(state)` in PLAYING, and `published` is updated through one private `publish(core, const ocb_state*)` that also increments `publish_count`. No other code writes `published`.

- [x] **Task 6: Host tests, F.3 to F.6 and F.12** (AC: #1 all; Decision D10)
  - [x] 6.1 `firmware/tests/core_fixture.h` (plain C, `static` helpers, no framework, includes `ocb_test.h` of Story 2.2): `struct harness { ocb_core core; ocb_core_event storage[N]; uint64_t now; bool ready; uint8_t seq; }`, `h_init(cap, report_period_us)`, `h_advance(us)`, and one helper per frame (`h_hello`, `h_stop`, `h_ping`, `h_set_state`, `h_queue_event`, `h_start`, `h_raw(type, body, len)`) that builds the body with `ocb_msg_*_encode`, fills an `ocb_frame`, calls `ocb_core_frame`, returns the `ocb_core_out`, and keeps a `seq` that increments by one per call (a test may override it). `h_tick()` calls `ocb_core_tick`. Reusing `ocb_msg_*_encode` keeps the tests tied to the Story 2.2 wire format.
  - [x] 6.2 `firmware/tests/test_core_modes.c` (**F.3**): the full matrix of D3/D14: frame in {SET_STATE, QUEUE_EVENT, START, STOP, HELLO, PING, unassigned `0x07`, bridge type `0x40`} x mode {IDLE, DIRECT, PLAYING} x aborted {no, yes} x ready {yes, no}; expected reply and expected resulting mode per cell are written as a table in the test (not computed by the code under test). Plus: entering IDLE by each of STOP, HELLO and every abort cause publishes NEUTRAL, empties the queue and resets ordering (an event with a `t_us` below the old baseline is accepted again); the latch is set by **each** cause of D5 from each of the three modes, makes `QUEUE_EVENT`, `START` and `SET_STATE` return `NAK ABORTED`, is **not** cleared by `PING`, by a rejected frame, by `ocb_core_tick` or by a second abort, and is cleared only by STOP and HELLO; `abort_count` rules of D5; `last_event` per cause.
  - [x] 6.3 `firmware/tests/test_core_end_late.c` (**F.4**): END, flags, underrun, LATE, ordering: see the case list in "Testing requirements".
  - [x] 6.4 `firmware/tests/test_core_sync.c` (**F.5**): HELLO and STOP in PLAYING (mid-piece, with a non-empty queue, with a held note published) with seq values 0, 1, 127, 128, 255 and the wrap (`seq = 255` gives `expected = 0`) all give: published NEUTRAL, queue empty, mode IDLE, `aborted` cleared (also when it was set), `expected = seq+1` (u8 wrap), reply ACK (STOP) or HELLO (HELLO), no event. Same from IDLE and DIRECT. A STOP frame with a body longer than 0 bytes still stops (D4).
  - [x] 6.5 `firmware/tests/test_core_mask.c` (**F.6**): every one of the 16 button bits alone and all 65536 `buttons` values (loop; fast) through `ocb_core_mask_state` and through the full path (QUEUE_EVENT in PLAYING, tick, read `published`): only L, R, A, X, Y survive; ZL, ZR, B, Minus, Plus, L3, R3, Home, Capture and the undefined bits 14 and 15 are cleared; `rx`, `ry` forced to `0x80` for all 256 x 256 values; `lx`, `ly` and every hat value 0 to 8 unchanged; `flags` never published. DIRECT (`SET_STATE`) is **unmasked**: B, Home, Capture, ZL, ZR, the right stick and the undefined bits are published as sent. The ZL, ZR and right-stick cases carry a comment citing the U11 result (D7).
  - [x] 6.6 `firmware/tests/test_core_edge.c` (**F.12**): queue capacity and `NAK FULL`, validation (hat 9, reserved flag bits, body lengths), `START` with an empty queue, the NAK precedence of D3, `ocb_core_status` values, two independent cores, report-period fallback, 64-bit clock (a `now_us` above 2^32), and a fixed-seed random property test (D10 list).
  - [x] 6.7 `firmware/tests/CMakeLists.txt` (UPDATE; today it registers `test_pins`, `test_pins_debug`, `test_version`, `pin_grep_selftest`, `test_vectors`, `test_proto_api`, `proto_version_check`, `proto_pure_check`): add the five executables linking `ocb_core` (which brings `ocb_proto`), `ocb_strict_warnings` on each source, `add_test(NAME test_core_modes ...)` and so on, and the same `OCB_TEST_ENV` (`ASAN_OPTIONS=detect_leaks=1` on Linux) as `test_vectors`. ASan and UBSan stay on through the `host-tests` preset (`OCB_SANITIZE=ON`, Story 2.2 D8).

- [x] **Task 7: Purity gate for the core, red and green** (AC: #1 "no pico-sdk includes"; Decision D9)
  - [x] 7.1 `firmware/tools/check-core-pure.sh`, same style, portability rules, exit codes and usage handling as `check-proto-pure.sh` (POSIX `grep -E`, `sed`, `find`, no `\b`/`\d`/`-P`, exit 0 clean, 1 violation printed as `file:line:text`, 2 usage). Default root `firmware/controller/core`; optional single argument = a directory or one file. Two rules: (a) every `#include` must be `<stdint.h> <stddef.h> <stdbool.h> <string.h> <limits.h>`, `"ocb_proto/<name>.h"` or `"ocb_core/<name>.h"`, and no include line may contain `pico`, `hardware/`, `tusb`, `ocb_hw`, `ocb_version.h`; (b) no line of code may mention `malloc`, `calloc`, `realloc`, `free(`, `printf`, `time_us`, `get_absolute_time`, `sleep_`, `busy_wait`, `multicore_`, `mutex_`, `spin_lock` or `__not_in_flash_func` (the core takes the time as an argument and has no heap or I/O; the RAM placement is the `OCB_CORE_RAMFUNC` macro of D12). Comment text that mentions a banned word is a violation too: write comments without them (simple and reviewable).
  - [x] 7.2 `firmware/tests/core_pure_selftest.sh CHECK FW_DIR` (pattern of `proto_pure_selftest.sh`, including the usage-error check) and fixtures in `firmware/tests/core_pure_fixtures/bad/` (one file per violation class: an `#include "pico/stdlib.h"`, an `#include "hardware/timer.h"`, an `#include "ocb_hw/pins.h"`, `#include <stdio.h>`, `#include <stdlib.h>`, `time_us_64()` in code, a `malloc(` call) and `core_pure_fixtures/good/` (allowed includes only, one trailing comment, one `"ocb_core/core.h"` include). Fixtures must contain no GPIO token or `PICO_*_PIN` macro (so `check-pins.sh` stays green; note the pin grep prunes only `tests/pin_check_fixtures`, not these).
  - [x] 7.3 ctest: `add_test(NAME core_pure_check COMMAND bash core_pure_selftest.sh ...)`. The self-test runs on the bad fixtures (each must be flagged, exit 1, non-empty output), the good fixture (must pass) and the **real** `controller/core` tree (must pass): that is the persistent red-and-green proof, and it runs in CI inside "Host tests".
  - [x] 7.4 `.github/workflows/firmware.yml` (UPDATE; today the steps after "Host tests" are "Pin grep (AD-20)", "Proto version vs vectors (AD-14)", "Proto library is pico-sdk free (AD-2)", "Job summary", upload): add a step **"Controller core is pico-sdk free (AD-2)"** running `firmware/tools/check-core-pure.sh`, right after the proto-pure step, as its own red step. Nothing else changes; the `firmware-uf2` artifact name and its paths stay. `actionlint` clean if available.
  - [x] 7.5 The "two instances are independent" test of 6.6 is part of the purity story: it proves there is no hidden global.

- [x] **Task 8: Docs** (AC: #1)
  - [x] 8.1 `docs/test-plan.md`, "Firmware host tests" (UPDATE; rows F.3 to F.6 exist and say what AD they match): keep their IDs; replace the vague pass criterion by what is now checked (F.3: matrix of 8 frame kinds x 3 modes x latch x ready, latch causes, IDLE entry; F.4: END, flags, underrun, LATE, ordering; F.5: sync points with any seq including wrap; F.6: 16 button bits, right stick, hat, DIRECT unmasked, "allow-list confirmed by U11, Story 1.1"). Add row **F.12** (core edge cases and property test, Story 2.4; related AD-6, AD-8) and **F.13** (core purity gate self-test, Story 2.4; related AD-2). Leave F.0, F.1, F.2, F.7 to F.11 and H.0 to H.3 untouched.
  - [x] 8.2 `docs/link-protocol.md`, section "Controller modes (AD-8)" (UPDATE; keep every existing sentence): add a short "Rules the controller core applies (Story 2.4)" list with the D3 precedence (`ABORTED`, `BUSY`, `NOT_READY`, validation `BAD_STATE`, `ORDER`, `FULL`), the D4 validation rules, the D5 abort-cause table (which cause gives which EVENT) and the D6 scheduler details (one event per tick, LATE threshold, END applied counts as an abort, the last non-END note of an underrun is zero-length). Say that body-length errors are `NAK BAD_STATE`. No change to the tables of message types, NAK values or EVENT values (D11). This is the source Story 3.3 (Rust sans-IO link) must follow.
  - [x] 8.3 `firmware/controller/README.md`: one paragraph on `core/` (what it is, purity, how to run its tests) and a note that `main.c` still only blinks the LED. `firmware/README.md`: one bullet under "Protocol checks" for `tools/check-core-pure.sh` and the `core_pure_check` ctest. `firmware/THIRD_PARTY_NOTICES.md`: add "ocb_core: written from the project's own architecture decisions (AD-8, AD-9), no third-party code".
  - [x] 8.4 Do **not** edit `docs/ocarina-input-mechanics.md` or the U11 rows (Story 1.1 owns them), `_bmad-output/specs/` (the C6 row of the risk register stays "open" until the hardware proof of Story 2.5; mention it in the Completion Notes as a suggestion), or anything under `_bmad/`.

- [x] **Task 9: Local verification, red checks and notes** (AC: #1)
  - [x] 9.1 From `firmware/` with the Story 2.1 setup (`firmware/tools/install-tools.sh`, `OCB_ARM_TOOLCHAIN`, `OCB_PICOTOOL_DIR`, the two submodule commands): `cmake --preset host-tests && cmake --build --preset host-tests && ctest --preset host-tests` all green (the 8 existing tests plus `test_core_modes`, `test_core_end_late`, `test_core_sync`, `test_core_mask`, `test_core_edge`, `core_pure_check`: 14); `cmake --preset fw-release && cmake --build --preset fw-release`, same for `fw-debug`, zero warnings and `bridge.uf2`/`controller.uf2` still produced; `firmware/tools/check-pins.sh`, `check-proto-pure.sh`, `check-proto-version.sh`, `check-core-pure.sh` pass; `shellcheck` clean on the new scripts and `actionlint` on `firmware.yml` when available.
  - [x] 9.2 **Red checks**, each reverted afterwards, each recorded in the Debug Log with the failing output line: (a) add `#include "pico/stdlib.h"` to `core/modes.c`: `check-core-pure.sh` exits 1 naming the file and line, and the host build also fails; (b) add a `malloc(` call to `core/queue.c`: the gate exits 1; (c) in `mask.c` add `OCB_BTN_ZL` to the allow-list: `test_core_mask` fails naming ZL; (d) change `NAK_ORDER` strictness (`<=` to `<`) in the ordering check: `test_core_end_late` fails; (e) remove the `enter_idle` call from the HELLO path: `test_core_sync` fails; (f) make the LATE comparison `>=`: the exact-threshold case fails; (g) make the abort latch clear on `PING`: `test_core_modes` fails. Also run once with `-DOCB_SANITIZE=ON` (default in the preset) and confirm ASan and UBSan really are active (look for `-fsanitize=address,undefined` in the compile commands, as in Story 2.2).
  - [x] 9.3 Completion Notes: exact commands, the ctest summary line, ASan/UBSan status on the local compiler, `arm-none-eabi-size` of the `ocb_core` archive objects (informational, for Story 2.5's RAM budget), and the CI status ("verified green after the owner's push" or "unverified until pushed"; never claim more).

## Dev Notes

### What this story is

The playback brain of the controller board as a library that runs on the PC: the three modes of AD-8, the event queue, the scheduler that releases events at `t0 + t_us`, the abort latch, and the AD-9 safety mask. It touches no USB, UART, timer or second core. Everything that can hurt a Switch input (a stuck note, a pressed Home, a backlog fired after a stall, a timeline that keeps going after the app died) is decided here, in plain C, with tests that run in seconds under ASan and UBSan. Stories 2.5, 2.6 and 2.7 wrap it with hardware and the link; if a rule is wrong here, all of them inherit it, so each open point is decided below with a reason (D1 to D15).

### Design in one page

```text
 UART RX -> ocb_rx -> go-back-N (2.6) -> ocb_core_frame(core, env, frame, out) -> reply (ACK / NAK / HELLO)
 watchdogs, USB events (2.3, 2.7) ----> ocb_core_abort(core, cause, out)   -> EVENT to send
 super-loop (2.5), every pass --------> ocb_core_tick(core, env, out)     -> EVENT to send
                                          |
                                          +--> core->published (ocb_state, masked in PLAYING) --> seqlock (2.5) --> profile
```

`env = { now_us (uint64), ready (bool) }`. The core owns: `mode`, the event queue, `aborted`, `abort_count`, `last_event`, `expected`, `t0`, the ordering baseline and `published`. It owns no clock, no I/O and no allocation.

### Decisions (made autonomously, 2026-11-22; the owner can overrule any of them)

Each was researched in the repo first (AD-8, AD-9, AD-6, AD-5, AD-19, `docs/link-protocol.md`, Story 2.2) and the recommended option picked. Where an AD already fixed a rule, the decision only records it. No external research was needed: the behaviour is the project's own design, nothing in this story copies external code, and none of it comes from 2wiCC.

**D1. A pure state machine with injected time and readiness, caller-provided storage, 64-bit clock.**
`ocb_core` is a struct; all functions take the instance, an `ocb_core_env { uint64_t now_us; bool ready; }` and an output struct, and return nothing else. No globals, no clock read, no callbacks. The queue storage is passed to `ocb_core_init`. `now_us` is 64-bit because the RP2040 hardware timer is 64-bit at 1 MHz and a 64-bit comparison removes every wrap bug (`due = t0 + lead + t_us` can exceed 2^32 because `t_us < 2^31` and `lead_us` is a u32); the code uses only `+`, `-` and comparisons on it (division is slow on Cortex-M0+). `ready` is an input per call so the NOT_READY gate and the future `STATUS.console_ready` read the same predicate (AD-8, AD-19, F.10). *Alternative rejected:* the core calling `ready()`/`now()` function pointers (hides time, harder to test, risks a hidden dependency on the SDK).

**D2. The core keeps `expected` and applies it at sync points; the rest of go-back-N is Story 2.6.**
The AC (F.5) demands `expected = seq+1` after HELLO/STOP and AD-8 binds it to the mode logic, so the core owns the u8 `expected`. Rules: HELLO and STOP set `expected = seq+1` (u8 wrap); every other frame the core handles advances it to `seq+1` after an ACK or a **semantic** NAK, and leaves it after the transport NAK `FULL` (AD-6: transport NAKs never advance `expected`); frames the core ignores (bridge types, D14) do not touch it. The caller (2.6) is responsible for calling `ocb_core_frame` only with `seq == expected` or a sync point; the core does not check it, and `STATUS.last_seq` is `expected - 1`. *Alternative rejected:* leaving `expected` to 2.6 (F.5 could then not be proven here).

**D3. One precedence order for refusing a frame.**
AD-8 and AD-6 list the NAK reasons but not which wins when several apply. The order, fixed here and recorded in `docs/link-protocol.md` for Story 3.3, is, for `SET_STATE`, `QUEUE_EVENT` and `START` (the table gives which reasons apply to which frame):

1. `ABORTED` (the latch is set; sticky, so the app must `STOP` first; it also hides every other reason because the app has to abandon the piece anyway)
2. `BUSY` (`SET_STATE`/`START` in PLAYING, the AD-8 table)
3. `NOT_READY` (`SET_STATE`/`START` when `env.ready` is false; not for `QUEUE_EVENT`, as AD-8 says; Story 2.7 inserts `NO_LINK` between 1 and 2 when it adds `link_ok`)
4. `BAD_STATE` (malformed or forbidden content, D4)
5. `ORDER` (`QUEUE_EVENT` only: `t_us` not strictly increasing, or `>= 2^31`)
6. `FULL` (`QUEUE_EVENT` only, the only **transport** reason in the core: the frame is fine, retry after credits; it never advances `expected`)

Content errors come before `FULL` on purpose: a malformed frame must never turn into a retry loop. `ABORTED`, `BUSY`, `NOT_READY`, `BAD_STATE` and `ORDER` are semantic (they advance `expected`, the frame has no effect, AD-6).

**D4. Content validation, all answered with `NAK BAD_STATE`; STOP and HELLO are never refused.**
AD-8 only names "END on a non-neutral state" and "reserved flag bits must be 0"; the codec of Story 2.2 deliberately leaves the rest to the core. Rules:
- a body of the wrong length (`ocb_msg_*_decode` returns `OCB_ERR_BODY_LEN`) is `BAD_STATE` (Story 2.2 D10 left the choice to this story; `BAD_STATE` is semantic, so a bad frame advances `expected` and cannot make the app resend the same bytes forever, unlike a transport NAK);
- `QUEUE_EVENT` and `SET_STATE`: `flags` bits 1 to 7 must be 0 (`ocb_state_reserved_bits_ok`); `hat` must be 0 to 8 (a hat of 9 to 255 would reach the HID profile as an undefined value; rejecting is safer than normalising); button bits 14 and 15 are **not** rejected: the mask clears them in PLAYING and the profile ignores them in DIRECT;
- `SET_STATE` must have `flags == 0` (END is a queue concept; "any other state carrying END" is `BAD_STATE`);
- `QUEUE_EVENT` with END requires `ocb_state_is_neutral_pose` (AD-8), otherwise `BAD_STATE`;
- a `QUEUE_EVENT` after an END event is already queued is `BAD_STATE` (it would be dropped when END is applied; refusing it tells the app about the bug; the flag is reset on entering IDLE);
- `START` with an empty queue is `BAD_STATE` (AD-8 does not say; with an empty queue nothing is ever applied, so the underrun rule can never fire and PLAYING would last until a `STOP`, with no app timeout in PLAYING (AD-5); the app prefills before START per `docs/link-protocol.md`);
- `PING` with a non-empty body is `BAD_STATE`;
- **`STOP` and `HELLO` act whatever the body length** (fail safe: a malformed body must never stop the controller from going neutral). HELLO's body (`proto_ver`) is not parsed here: the version check and INFO are Story 2.6.
*Open point for the owner:* the empty-queue START and the after-END rule are the two places where this story adds a refusal AD-8 does not list; both are one-line removals if the owner prefers.

**D5. Abort causes: one entry point, every cause latches, the event follows the cause.**
`ocb_core_abort(core, cause, out)` for causes coming from outside; the scheduler uses the same private helper for END, underrun and LATE. Rules: (1) it applies in **every** mode including IDLE (AD-8 lists USB suspend, unmount and bus reset "in any mode"; this story applies the same to every cause so "the latch is set by every unrequested cause" has no exceptions); (2) it runs `enter_idle`, sets `aborted`, `last_event` and returns the event in `out->event`; (3) `abort_count` (u8, wraps, never reset) increments only on the transition `aborted` clear to set, and a cause that arrives while the latch is already set changes nothing and returns no event (the app is already told; this also stops a flapping link from flooding EVENTs); (4) END applied **does** count as an abort (AD-8 lists it as a cause), so `abort_count` counts every latch set, normal piece ends included; the app compares it between STATUS frames; (5) event per cause: `END` gives `PLAYBACK_DONE`, `UNDERRUN` gives `UNDERRUN`, `LATE` gives `LATE`, `LINK_LOST` gives `LINK_LOST`, the three USB causes give `USB_SUSPENDED` (v1 has one code for "the console went away"; no new wire value, D11), `APP_TIMEOUT` and `CORE_STALL` give **no** EVENT (the app is dead or the board is rebooting; the app learns it from STATUS `mode`/`abort_count`) but still set the latch and count the abort; they leave `last_event` unchanged; (6) `last_event` keeps the last EVENT emitted and is not cleared by STOP or HELLO (it lets the app recover a lost EVENT from STATUS, AD-19).

**D6. Scheduler: one event per tick, LATE strictly greater than one report period, END and underrun after apply.**
`ocb_core_tick`: only in PLAYING and once `now_us >= t0_us` (`t0_us = now_us + lead_us` at START). Let `head` be the oldest event and `due = t0_us + head.t_us`. If `now_us < due`: nothing. Else, in this order:
1. **LATE:** if `now_us - due > late_us`, abort with `LATE` (D5): NEUTRAL, queue cleared, nothing fired. This applies to an overdue END too (it ends the piece with `LATE` instead of `PLAYBACK_DONE`; the outcome is neutral either way). `late_us` is the report period given to `ocb_core_init`/`ocb_core_set_report_period` (the measured median IN interval, `INFO.report_period_us`); 0 means unknown and selects `OCB_CORE_LATE_FALLBACK_US = 8000` (the 8 ms the Switch 1 Pro Controller is commonly polled at, `docs/usb-controller-emulation.md`; a threshold of 0 would abort every event). Exactly one report period late is **not** late (`>` not `>=`).
2. Otherwise pop the head, publish `ocb_core_mask_state(head.state)`.
3. **END:** if the event had END, run the abort helper with `PLAYBACK_DONE` (publishes NEUTRAL, IDLE, latch set).
4. **Underrun:** else if the queue is now empty, run the abort helper with `UNDERRUN` (publishes NEUTRAL in the same call, so "in the same loop pass" holds: the note event is published, then NEUTRAL, with no tick in between).
At most one event is applied per call, so a stall of a few hundred µs never fires two events inside one loop pass and the order of published states is always the queue order; "no backlog" after LATE is guaranteed by clearing the queue. The last non-END note before an underrun therefore lasts zero time: it is the price of AD-8's rule and acceptable (a stuck note is worse than a gap, `docs/link-protocol.md`).

**D7. The mask is AD-9 as written, confirmed by U11; the hat is not narrowed.**
Allow-list = buttons L, R, A, X, Y, the hat, the left stick (`lx`, `ly`). Everything else is cleared and the right stick centred (`0x80`, `0x80`); `flags` become 0. U11 (Story 1.1, `docs/ocarina-input-mechanics.md` table "Unknowns" row U11, and `docs/test-plan.md` section "U11 other buttons in free play") found no other note input: ZL opens the song menu, ZR does nothing, the right stick moves the camera, so the AD-9 list is confirmed unchanged and masking ZL and the right stick is "desirable" (the tests assert it). B is also masked: it exits free play (U9). The hat passes with **all** its values 0 to 8 because AD-9 allows the hat and the core cannot know which direction is harmful for the current profile; the app's timeline generator already restricts itself to up, down and neutral (Stories 1.3 to 1.5) and a fully hat-aware mask would need the profile. *Open point for the owner:* the D-pad left (hat 5, 6, 7) toggles free play (U9); if the owner wants the firmware to refuse it too, that is an AD-9 change (narrow the hat in `mask.c`, one test row), not a change in this story's scope. The mask runs at publish time in PLAYING only; DIRECT (`SET_STATE`) is unmasked by AD-9 (the virtual gamepad must be able to press anything).

**D8. What "published" means: the final `ocb_state` after each call, flags always 0.**
`core->published` is the state the profile must report; `publish_count` increments at every publish. After any call, the contract is: IDLE implies `published == OCB_NEUTRAL`; PLAYING implies the published state is masked; the END flag is never published. Underrun publishes two states within one call (the note, then NEUTRAL); a consumer that copies `published` after the call (Story 2.5's seqlock write) sees only the final one, which is fine. The core never exposes a pointer a reader could hold across calls.

**D9. A new gate `check-core-pure.sh` instead of loosening `check-proto-pure.sh`.**
The AC asks for no pico-sdk in the core and that the purity is enforced by CI. The proto gate must keep refusing `ocb_core/` (the proto library must not depend on the core), so the core gets an analogous script with its own include allow-list (`ocb_proto/`, `ocb_core/` and the same five standard headers) and an extra rule banning heap, I/O and clock identifiers. It has the same shape, exit codes and self-test pattern as Story 2.2's, plus its own CI step and ctest `core_pure_check`. The host-tests build is a second, independent barrier (it has no SDK include path, so any `pico/` include fails to compile). The dev agent may factor shared shell code into `tools/` only if both self-tests stay green. Red and green proof: fixtures in ctest (persistent, runs in CI) and the manual red checks of Task 9.2.

**D10. Test layout, IDs and the property test.**
One executable per F-ID block, each named after the block, so a CI failure says which rule broke: `test_core_modes` (F.3), `test_core_end_late` (F.4), `test_core_sync` (F.5), `test_core_mask` (F.6); `test_core_edge` (F.12: everything that is part of the AC's intent but not a numbered F row: queue capacity, `NAK FULL`, validation, precedence, status view, independence, report-period fallback, 64-bit clock, property test); `core_pure_check` (F.13). All use the Story 2.2 `ocb_test.h` (no framework, D7 there) and the shared `core_fixture.h`. The property test is a fixed-seed LCG (seed constant, 20 000 steps, three seeds) of random frames (any type, random bodies, random seq), random ticks, random clock advances (including jumps larger than the report period) and random aborts; after every step it asserts the invariants: IDLE implies published NEUTRAL; PLAYING implies the mask holds; `aborted` implies IDLE; queue count never exceeds capacity; `expected` changed only as D2 allows; a STOP leaves IDLE, not aborted, empty queue, NEUTRAL; `free + count == cap`. Expected values in the matrix tests are written by hand from the AD-8 table, not computed by the code under test, so the tests do not just agree with themselves.

**D11. No wire change.**
The core only uses constants that already exist (`OCB_NAK_*`, `OCB_EVT_*`, `OCB_MODE_*`, `OCB_BTN_*`, `OCB_QUEUE_CAP_DEFAULT`, `OCB_T_US_LIMIT`). Its own constants (`OCB_CORE_LATE_FALLBACK_US`, `OCB_CORE_ALLOWED_BUTTONS`) are core-private and live in `ocb_core/`, not in `consts.h`; `PROTO_VERSION` stays 1 and the vectors are not touched. The app-visible *behaviour* (precedence, validation, abort rules) is documented in `docs/link-protocol.md` (Task 8.2) because the app cannot read it from vectors; Story 3.3 follows that text.

**D12. `OCB_CORE_RAMFUNC`: a macro, empty by default, so Story 2.5 can place the hot path in RAM without editing every function.**
AD-11 runs the super-loop from RAM. The core cannot include the SDK macro `__not_in_flash_func` (the gate bans it). `core.h` defines `OCB_CORE_RAMFUNC` as empty unless the build defines it (for example Story 2.5 adds `target_compile_definitions(ocb_core PUBLIC OCB_CORE_RAMFUNC=__attribute__((section(".time_critical.ocb_core"))))`); `ocb_core_frame`, `ocb_core_tick`, `ocb_core_abort`, `ocb_core_mask_state` and the queue functions are declared and defined with it. No behaviour change in this story; 2.5 decides the exact attribute.

**D13. A read-only status view is part of the core.**
`ocb_core_status` returns `free`, `now_us` (u32, µs since t0, 0 in IDLE and during the lead), `last_seq`, `mode`, `abort_count`, `last_event`. It is cheap and all of it is core state; Story 2.6 only formats the STATUS body and ORs in `link_ok`/`console_ready`. Covered by F.12.

**D14. Which frames the core handles.**
`HELLO`, `STOP`, `PING`, `SET_STATE`, `QUEUE_EVENT`, `START` as in the table of D3/D4. `PING` is acknowledged in every mode with no effect on state (it is the app keep-alive; the app-timeout clock is Story 2.7), including while aborted (AD-8 lists only the three frames above as NAKed by the latch). A type in `0x01..0x3F` that is not assigned (`0x07` to `0x3F`) gets `NAK UNKNOWN_TYPE` (semantic: `expected` advances, AD-6). A type in `0x40..0x7F` (bridge, link-local, never ACKed, outside the app `seq` space, AD-3), `0x00` or `0x80..0xFF` must never reach the core; if one does, the core returns `OCB_CORE_REPLY_NONE` and changes nothing (including `expected`), and a test pins that.

**D15. No hardware is needed.**
Every AC is verifiable on the host (`host-tests`). The Switch, the RP2040-Zero boards and the test-plan environment are not used by this story. What the host cannot prove (the core running at 1 MHz resolution from RAM next to TinyUSB, the real IN poll interval behind `late_us`, the mask on the console) is proven by Story 2.5 (debug self-timing, the console melody) and the H-tests (H.2), not claimed here.

### Behaviour reference (the table the tests encode)

Reply and effect for each frame, in the order of D3 (the first matching row wins; "advance" = `expected = seq+1`). Rows are listed by frame; modes are I(DLE), D(IRECT), P(LAYING). `aborted` and `ready` are inputs.

| Frame | Condition (in order) | Reply | Effect |
|---|---|---|---|
| `HELLO` | always | `HELLO` (2.6 sends INFO) | `enter_idle`, clear `aborted`, `expected = seq+1`, no event |
| `STOP` | always (any body length) | `ACK` | same as HELLO |
| `PING` | body length != 0 | `NAK BAD_STATE` | advance |
| `PING` | else | `ACK` | advance, no state change |
| `SET_STATE` | `aborted` | `NAK ABORTED` | advance |
| | mode P | `NAK BUSY` | advance |
| | `!ready` | `NAK NOT_READY` | advance |
| | bad length, flags != 0, hat > 8 | `NAK BAD_STATE` | advance |
| | else (mode I or D) | `ACK` | mode D, publish the state unmasked, advance |
| `START` | `aborted` | `NAK ABORTED` | advance |
| | mode P | `NAK BUSY` | advance |
| | `!ready` | `NAK NOT_READY` | advance |
| | bad length, or queue empty | `NAK BAD_STATE` | advance |
| | else (I or D) | `ACK` | publish NEUTRAL, mode P, `t0 = now + lead`, advance |
| `QUEUE_EVENT` | `aborted` | `NAK ABORTED` | advance |
| | bad length, reserved flag bits, hat > 8, END on a non-neutral pose, an END already queued | `NAK BAD_STATE` | advance |
| | `t_us >= 2^31`, or `t_us <= last accepted t_us` (when one exists) | `NAK ORDER` | advance |
| | queue full | `NAK FULL` (transport) | **no** advance |
| | else (any mode) | `ACK` | enqueue, remember `t_us`, END flag, advance |
| `0x07..0x3F` | always | `NAK UNKNOWN_TYPE` | advance |
| other types | always | none | none |

Abort causes (D5), all modes: `LINK_LOST` gives `EVENT LINK_LOST`; `USB_SUSPEND`, `USB_UNMOUNT`, `USB_BUS_RESET` give `EVENT USB_SUSPENDED`; `APP_TIMEOUT` and `CORE_STALL` give no EVENT; scheduler: END gives `PLAYBACK_DONE`, underrun `UNDERRUN`, late head `LATE`. Each: `enter_idle`, `aborted = true`, `abort_count++` (only when the latch was clear).

### Consumers (what later stories need from this one)

- **Story 2.5**: `ocb_core_init`, `ocb_core_frame`, `ocb_core_tick`, `ocb_core_mask_state`, `core->published` + `publish_count` (copied into the seqlock after each call), `OCB_CORE_RAMFUNC`, the queue storage ownership (512 x 12 bytes), `ocb_core_set_report_period` when the profile has measured the IN interval. It also loads its hard-coded timeline through `ocb_core_frame` QUEUE_EVENT frames or a small `ocb_core_load` it may add; the END and underrun rules apply unchanged.
- **Story 2.6**: `ocb_core_frame` replies (it builds ACK, NAK(reason, `expected`, `got = frame seq`), INFO), `ocb_core_status`, `core->expected`, the D3 precedence for its NAK matrix test (F.2), `NAK FULL` as the only transport NAK that comes from the core, the HELLO reply kind, `ready` supplied in `env`.
- **Story 2.7**: `ocb_core_abort(LINK_LOST | APP_TIMEOUT)`, the slot for `NAK NO_LINK` in the D3 order, `abort_count`/`last_event` rules; DIRECT-only app timeout is its own timer (the core records no app-frame time).
- **Story 2.3**: nothing from the core except the `ready()` contract: the profile reads `core->published` through 2.5's seqlock and calls `ocb_core_abort(USB_*)` on suspend, unmount and bus reset.
- **Story 3.3 (Rust link)**: the behaviour text of Task 8.2.

### Developer guardrails

- **Read before editing (UPDATE files):** `firmware/CMakeLists.txt` (adds `common/proto`, `common/hw`, then `bridge` and `controller` in firmware mode or `tests` in host mode; sanitizer flags set only under `OCB_HOST_TESTS`), `firmware/controller/CMakeLists.txt` (executable `controller` from `main.c`, links `ocb_hw ocb_proto ocb_version`, stdio disabled, UF2 output), `firmware/tests/CMakeLists.txt` (plain executables, `ocb_strict_warnings` per source, `OCB_TEST_ENV` for Linux leak detection, glob of vector files), `.github/workflows/firmware.yml` (one job, steps in the order listed in Task 7.4), `docs/test-plan.md` F table, `docs/link-protocol.md` "Controller modes". Preserve all of it.
- The core must leave the system working end to end: `controller` and `bridge` must still build and link in `fw-release` and `fw-debug`, `bridge.uf2` and `controller.uf2` still come out, and the artifact name `firmware-uf2` is unchanged.
- Never cast a byte buffer to a struct, never assume alignment. Use `ocb_msg_*_decode` and `ocb_frame` from Story 2.2.
- `uint8_t` arithmetic: `expected + 1` wraps as u8; cast explicitly (`-Wconversion` is on).
- Do not invent wire values. If a rule seems to need a new NAK reason or EVENT code, stop and record it as an open point instead (it would be a `PROTO_VERSION` and vector change, AD-14).
- Do not put a GPIO number or `PICO_*` pin macro in any file under `firmware/` (pin grep).
- Do not use any 2wiCC code or text; this logic is original and comes from AD-8 and AD-9.

### Architecture compliance

- AD-2: pure C99 core next to `ocb_proto`, no SDK; `host-tests` builds it with the native compiler. The seed's `controller/core/ # queue, scheduler, modes, watchdog (pure C)` is followed (watchdog files arrive in 2.7).
- AD-8: modes, table, entering IDLE, abort latch, ordering, END, underrun, LATE, NOT_READY (D3 to D6).
- AD-9: compile-time allow-list, DIRECT unmasked, U11 confirmed (D7).
- AD-6: semantic vs transport NAK effect on `expected` (D2, D3).
- AD-10: `NEUTRAL`, `ocb_state_is_neutral_pose`, `ocb_state_reserved_bits_ok` from `ocb_proto`; the core does not redefine neutral.
- AD-11: the core is what core 1 runs; single-threaded by construction, no atomics, `OCB_CORE_RAMFUNC` hook (D12). Seqlock and timer are 2.5.
- AD-12: the core never touches a profile; `ready` is an input.
- AD-19: `abort_count`, `last_event`, `last_seq` semantics (D5, D13).
- AD-20: no pins in the core.

### Library and framework requirements

None external. C99 (`-std=c99`-equivalent via `C_STANDARD 99`, `C_EXTENSIONS OFF`), the standard headers on the allow-list, `ocb_proto` from Story 2.2. No test framework (Story 2.2 D7). Compile-time checks use the C99 negative-array typedef idiom, not `_Static_assert`. Cortex-M0+ facts that matter: no hardware divide and no 64-bit atomics (so no `/` or `%` on `uint64_t`, and the core is single-writer by design); `uint64_t` add, subtract and compare are cheap.

### File structure requirements

```text
firmware/controller/core/
  CMakeLists.txt
  include/ocb_core/core.h  queue.h  mask.h  ocb_core.h
  queue.c  mask.c  modes.c  sched.c
firmware/tests/
  core_fixture.h
  test_core_modes.c  test_core_end_late.c  test_core_sync.c  test_core_mask.c  test_core_edge.c
  core_pure_selftest.sh
  core_pure_fixtures/bad/*   core_pure_fixtures/good/*
firmware/tools/check-core-pure.sh
```

Updated: `firmware/CMakeLists.txt`, `firmware/controller/CMakeLists.txt`, `firmware/tests/CMakeLists.txt`, `.github/workflows/firmware.yml`, `docs/test-plan.md`, `docs/link-protocol.md`, `firmware/controller/README.md`, `firmware/README.md`, `firmware/THIRD_PARTY_NOTICES.md`. Not touched: `common/proto/**` (including the vectors and `consts.h`), `common/hw/**`, `bridge/**`, `_bmad/**`, `_bmad-output/specs/**`.

### Testing requirements

All under ctest `host-tests`, ASan and UBSan on, hand-written expectations. Mapped to `docs/test-plan.md`:

| Test (ctest name) | Test-plan ID | Must cover |
|---|---|---|
| `test_core_modes` | **F.3** | the 8 frame kinds x 3 modes x latch x ready matrix; IDLE entry (neutral, queue cleared, ordering reset); latch set by every cause (`LINK_LOST`, `APP_TIMEOUT`, three USB causes, `CORE_STALL`, END, underrun, LATE) from each mode; `NAK ABORTED` for the three frames; latch cleared only by STOP/HELLO; `abort_count` increments only on clear-to-set, wraps at 256; event per cause; `last_event` kept |
| `test_core_end_late` | **F.4** | `NEUTRAL\|END` applied: `PLAYBACK_DONE`, IDLE, NEUTRAL published, latch set; END with a pressed button, with hat != 8, with a stick off-centre, with `buttons` 0x8000 only: `NAK BAD_STATE`; flags 0x02, 0x80, 0xFE, 0x03: `NAK BAD_STATE` (queue and SET_STATE); SET_STATE with END: `BAD_STATE`; a frame after a queued END: `BAD_STATE`; underrun: non-END last event leaves the queue empty: in the same `ocb_core_tick` call NEUTRAL is published and `UNDERRUN` returned (`publish_count` +2), a non-END event with more queued does not underrun; LATE: head overdue by exactly `late_us` is applied, by `late_us + 1` aborts (`LATE`, NEUTRAL, queue empty, nothing from the backlog fires on later ticks even after `now` jumps far past all events); fallback `late_us` when the period is 0; no tick before `t0` (lead), nothing applied in IDLE/DIRECT; ordering: first event any `t_us` in [0, 2^31), equal to the last is `ORDER`, lower is `ORDER`, `2^31` is `ORDER`, `2^31 - 1` is accepted, the baseline resets after IDLE; one event per tick |
| `test_core_sync` | **F.5** | HELLO and STOP from PLAYING (and I, D) with seq 0, 1, 127, 128, 255: NEUTRAL, queue cleared, IDLE, latch cleared, `expected = seq+1` with wrap; replies; no event; STOP with a body still stops |
| `test_core_mask` | **F.6** | all 65536 `buttons` values and the 16 single bits; right stick all values; `lx`/`ly`/hat unchanged; flags never published; DIRECT unmasked (B, Home, Capture, ZL, ZR, right stick, bits 14/15 pass); START from DIRECT publishes NEUTRAL |
| `test_core_edge` | **F.12** | queue cap 1, 2, 4, 512: FULL at the right frame, `free` correct, FULL does not advance `expected` and is not latched; `NAK` precedence pairs (ABORTED over BUSY over NOT_READY over BAD_STATE over ORDER over FULL); NOT_READY for START and SET_STATE only, QUEUE_EVENT accepted while not ready; NOT_READY follows `env.ready` live; START with an empty queue; hat 9, 255; body lengths wrong for every frame (`BAD_STATE`; STOP/HELLO unaffected); unassigned types `0x07`, `0x3F` get `UNKNOWN_TYPE`; types `0x00`, `0x40`, `0x7F`, `0x80`, `0xFF` change nothing (`expected` too); `ocb_core_status` in each mode (`now_us` 0 in IDLE and during the lead, correct afterwards, `last_seq = expected - 1`); `now_us` above 2^32 works; two independent cores; the property test of D10 |
| `core_pure_check` | **F.13** | every bad fixture flagged (exit 1, output), good fixture passes, real `controller/core` passes, usage error exits 2 |

CI mapping: "Host tests" step runs all of it; the separate step "Controller core is pico-sdk free (AD-2)" runs the gate on the real tree.

### Previous story intelligence (Story 2.2, done; PR #16, merged)

- `ocb_proto` is a pure C99 `STATIC` library with `ocb_strict_warnings` plus `-Wpedantic -Wshadow -Wconversion` set per source; copy that CMake pattern for `ocb_core`. It provides everything this story needs: `ocb_state` and `OCB_NEUTRAL`, `ocb_state_is_neutral_pose()`, `ocb_state_has_end()`, `ocb_state_reserved_bits_ok()`, `ocb_msg_*_decode`/`_encode` (decode returns `OCB_ERR_BODY_LEN` for a wrong length), `ocb_frame`, `OCB_NAK_*`, `OCB_EVT_*`, `OCB_MODE_*`, `OCB_BTN_*`, `OCB_T_US_LIMIT`, `OCB_QUEUE_CAP_DEFAULT`. Read `consts.h`, `state.h`, `msgs.h` and `frame.h` before coding.
- Decoder semantics decided there (D10 of Story 2.2) that this story builds on: the codec reports a wrong body length and leaves the NAK to the consumer (answered here: `BAD_STATE`, D4); the codec does not validate hat, button bits or flags (answered here, D4); the seq check and the semantic checks are the consumers' (answered here for the semantic part, 2.6 for go-back-N).
- The scripts pattern (`check-proto-pure.sh`, `check-proto-version.sh`, `check-pins.sh`): POSIX `grep -E`, `[[:space:]]`, no `\b`/`\d`/`-P` (BSD grep on macOS and GNU grep on CI must agree); self-tests live in `firmware/tests/*.sh`, are registered in ctest, and include a usage-error check (exit 2). The tests must not write into the source tree.
- Review lessons from 2.2: a High finding was a check-order mistake against the AD-3 order that a vector then pinned; here, the precedence of D3 is the equivalent trap, so pin every pair in F.12. Low findings were undocumented behaviours (`NULL` handling, multiple evaluation): document `NULL` rules in the headers (`out` and `core` non-NULL; `frame` non-NULL) and avoid macros that evaluate arguments twice.
- Completion Notes must stay honest: reviews caught overclaims and CI-unverified statements. Use the exact words "unverified until pushed" for anything only CI shows.
- Story 2.2 left `check-pins.sh` pruning only `tests/pin_check_fixtures` and `common/proto/vectors`; any new fixture directory is scanned by it, so keep GPIO tokens out of `core_pure_fixtures/`.
- Local setup is in `firmware/.tools` (git-ignored) with `OCB_ARM_TOOLCHAIN` and `OCB_PICOTOOL_DIR`; Homebrew gcc was used once as a second compiler for the host tests, a nice extra check for the property test.

### Git intelligence

Recent commits: `fb27406` merge of PR #16 (Story 2.2), `73fd536` and `1a03f53` bookkeeping, `94a1286` review follow-ups, `25dab61` the 2.2 implementation. Pattern: one story branch (this one: `story/2-4-controller-core-modes-queue-scheduler-and-safety-mask`), an implementation commit plus a review follow-up commit, merge by PR; the owner commits and pushes. No firmware file under `controller/` other than `main.c`, `CMakeLists.txt` and `README.md` exists yet. Use the `ocb_` prefix and `OCB_` constants; SPDX `MIT` headers; English everywhere.

### Latest tech information

No external research was needed: the logic is the project's own (AD-8, AD-9), the toolchain is pinned (Arm GNU Toolchain 15.2.Rel1, pico-sdk 2.3.1, CMake >= 3.25; Story 2.1) and unchanged. Relevant platform facts, from the RP2040 design: the 64-bit microsecond timer (`TIMERAWH`/`TIMERAWL`) is the 1 MHz clock Story 2.5 will feed into `env.now_us`; Cortex-M0+ has no hardware divider for general code, which is why D1 forbids `/` and `%` on 64-bit values. The 8 ms fallback of D6 is a hypothesis taken from `docs/usb-controller-emulation.md` ("8 ms is common on Switch 1 for the Pro Controller"), to be replaced by the measured `report_period_us` in Story 2.3/2.5.

### Hardware checks

None in this story (D15). For the record, what the owner will see later: Story 2.5 plays a hard-coded timeline through this core on the Switch (test-plan "Hardware-in-the-loop checks", H.2 for the stall rule), and H.3 (`console_ready`) belongs to 2.3 and 2.6. Nothing needs to be recorded in `docs/test-plan.md` result cells for this story; the F.x rows have no "Result" column.

### Project Context Rules

No `project-context.md` exists in the repo. Project rules that apply, from the owner's notes and the repo: all repo files in English (chat in Italian); never commit `_bmad/` or `bmad-*`/`gds-*` skills (only `_bmad-output/`); never edit `~/Documents/org/`; UX reference screenshots stay out of git and are not used here; no 2wiCC code (NFR-07, ADR-004); the dev agent does not commit or push.

### References

- [Source: `_bmad-output/planning-artifacts/epics.md`#Story 2.4, #Epic 2 Notes (sync point with U11), Stories 2.3, 2.5, 2.6, 2.7 (scope boundaries)]
- [Source: `_bmad-output/planning-artifacts/architecture/architecture-bonooru-2026-11-19/ARCHITECTURE-SPINE.md`#AD-2, AD-3, AD-5, AD-6, AD-8, AD-9, AD-10, AD-11, AD-12, AD-19, AD-20]
- [Source: `docs/link-protocol.md`#Controller modes (AD-8), #Reliability (AD-6), #Controller to app reporting (AD-19), #Body sizes and length rules, #Timing constants]
- [Source: `docs/test-plan.md`#Firmware host tests F.0 to F.11, #Hardware-in-the-loop checks, #U11 other buttons in free play]
- [Source: `docs/ocarina-input-mechanics.md`#Unknowns, row U11 (allow-list confirmed)]
- [Source: `docs/usb-controller-emulation.md`#Profile interface (AD-12), polling-rate hypothesis]
- [Source: `_bmad-output/specs/spec-bonooru/risk-register.md` C6; `requirements.md` FR-07, NFR-05; `decisions.md` ADR-003 (timing on the controller board)]
- [Source: `_bmad-output/implementation-artifacts/2-2-protocol-library-and-conformance-vectors.md`#Decisions D1 to D19, #Previous story intelligence]
- [Source: `firmware/common/proto/include/ocb_proto/{consts,state,msgs,frame,err}.h`; `firmware/tools/check-proto-pure.sh`; `firmware/tests/{CMakeLists.txt,proto_pure_selftest.sh,ocb_test.h}`; `.github/workflows/firmware.yml`]

### Open points for the owner (none block the story)

- D4: two refusals that AD-8 does not list (`START` with an empty queue, `QUEUE_EVENT` after a queued END) are `NAK BAD_STATE`; both are easy to drop.
- D7: the hat is not narrowed; D-pad left (free-play toggle, U9) passes the mask as AD-9 is written.
- D5: normal piece end increments `abort_count` (AD-8 lists END as an abort cause); the app should treat `PLAYBACK_DONE` as the discriminator.
- D6: the last non-END note before an underrun is zero-length (AD-8's rule); the 8000 µs LATE fallback is a hypothesis until 2.3 measures the IN interval.
- The risk register row C6 (still "open") can move to "mitigated in code, hardware proof pending" after this story; left untouched here (`_bmad-output/specs/` is not edited by this story).

## Review Findings

Code review of commit `3664f57` (fresh context): APPROVE, no important findings. Randomized fuzz harness (ASan+UBSan, about 7.2M iterations plus 1.2M with implicit-conversion and local-bounds checks): no crashes, no sanitizer reports, no invariant failures. Minor findings M1 to M4 fixed in `7b9f060`; M5 to M7 deferred:

- [x] **M1 (fixed)** `check-core-pure.sh` scanned only `*.c`/`*.h` and missed some I/O and clock names. Now it also scans `*.cpp *.cc *.S *.inc`, bans `clock_gettime`, `gettimeofday`, `stdio_`, `puts`, `putchar` (word-bounded), and fails when a `CMakeLists.txt` in the tree names `pico_*`, `hardware_*` or `ocb_hw` on a non-comment line. 11 new bad fixtures (18 in all) and a good `CMakeLists.txt` fixture (the comment may name them, the link line may not); red proven on the real tree, real tree green.
- [x] **M2 (fixed)** The static hot-path helpers of `modes.c` (`out_reset`, `cause_event`, `reply_ack`, `reply_nak`, `hat_ok`, `sync_point`, `handle_set_state`, `handle_start`, `handle_queue_event`) are now `static OCB_CORE_RAMFUNC`; `queue.c`, `mask.c`, `sched.c` have no static helpers. `ramfunc.h` and `firmware/controller/README.md` state that Story 2.5 must review the RAM placement of the `ocb_proto` functions the core calls, of `OCB_NEUTRAL` and of compiler-generated read-only tables.
- [x] **M3 (fixed, doc)** `docs/link-protocol.md` states explicitly that `START` on an empty queue and `QUEUE_EVENT` after a queued END are `NAK BAD_STATE`, refusals AD-8 does not list, which Story 3.3 must mirror.
- [x] **M4 (fixed, doc)** `docs/link-protocol.md` and the `ocb_core_frame` header comment state that the link layer (Story 2.6) checks HELLO's `proto_ver` **before** passing HELLO to `ocb_core_frame`, because the core resets on every HELLO (D22).
- [ ] **M5 (deferred)** The 8000 µs LATE fallback is a hypothesis (D6); re-measure the IN interval in Story 2.3/2.5 on hardware and feed `report_period_us`. Not fixable on the host.
- [ ] **M6 (deferred)** `ready()` is sampled only at `START` / `SET_STATE` (AD-8); losing readiness mid-PLAYING is handled by the USB abort causes. To be confirmed against Story 2.6/2.3's bus-reset / suspend abort wiring; no core change needed if those call `ocb_core_abort`.
- [ ] **M7 (deferred)** `ocb_core_status_view.now_us` is a u32 and wraps after about 71 minutes of one piece. It is the wire width of `STATUS.now_us` (no wire change allowed, D11), and pieces are minutes long; documented in `core.h`.

## Dev Agent Record

### Agent Model Used

Claude Opus 5.5 (claude-opus-5-5), gds-dev-story workflow, 2026-11-22.

### Decisions taken during implementation (numbered after D15; the owner can overrule any of them)

- **D16. The IDLE invariant is "IDLE implies NEUTRAL; the latch implies IDLE with an empty queue", not "IDLE implies an empty queue".** Task 4.3 and D10 state "IDLE implies NEUTRAL published, empty queue", but AD-8 accepts `QUEUE_EVENT` in IDLE (prefill), so an IDLE core can legitimately hold events; the first test run failed exactly there. The tests therefore check: IDLE implies NEUTRAL published; `aborted` implies IDLE and an empty queue; and *entering* IDLE (STOP, HELLO, every abort cause) empties the queue, checked at the transition (F.3, F.5, F.12). Rationale: follows AD-8's table, which wins over the story text; no behaviour change.
- **D17. Two extra headers: `ocb_core/ramfunc.h` and `ocb_core/internal.h`.** `OCB_CORE_RAMFUNC` (D12) is needed by `queue.h` and `mask.h` too, and `core.h` includes both, so the macro lives in `ramfunc.h` (included by `core.h`, so "core.h defines it" still holds for users). The private helpers shared by `modes.c` and `sched.c` (`ocb_core_publish_`, `ocb_core_enter_idle_`, the only writer of `published` and the only way into IDLE) are declared in `ocb_core/internal.h`, marked internal; keeping them under `ocb_core/` keeps the include allow-list of the gate simple. Rationale: no circular includes, gate unchanged.
- **D18. The aborted cells of the F.3 matrix.** An aborted core is always IDLE (every abort enters IDLE, and the latch NAKs the frames that leave it), so the cells "aborted × DIRECT/PLAYING" are set up by entering that mode and then aborting (`LINK_LOST`); their expected rows are the aborted behaviour, resulting mode IDLE. The precedence pair `ABORTED` over `BUSY` is unreachable in a consistent core; F.12 pins it white-box by forcing `mode = PLAYING` with the latch set, and `ABORTED` still wins. Rationale: covers the spec's 96 cells without inventing an impossible state in the black-box test.
- **D19. Fail-safe handling of impossible inputs.** An abort cause outside the enum is handled like `CORE_STALL` (latched, no EVENT); a scheduler pass in PLAYING with an empty queue (unreachable: START needs a queued event and an empty queue after an apply is an underrun) aborts with `UNDERRUN`. Rationale: any unexpected path ends neutral, never stuck.
- **D20. Gate details.** `check-core-pure.sh` spells the word boundary out (`(^|[^A-Za-z0-9_])`, POSIX ERE has no `\b`), so `ocb_queue_free(` is legal while a bare `free(` is not; the `printf` rule covers the whole `*printf` family (`snprintf`, `fprintf`). The good fixture contains an identifier with `free` in its name to pin this.
- **D22. HELLO's version is checked by the link layer before the core (review M4).** The core treats every HELLO as a sync point and never reads `proto_ver`; Story 2.6 checks the version first and answers a mismatch itself, deciding there whether a mismatched HELLO still forces NEUTRAL. Rationale: keeps the core free of session state (D2, D14) and makes the order explicit for 2.6 and 3.3.
- **D21. `ocb_test.h` helpers are `static inline`.** Including `ocb_test.h` from a test that does not use `OCB_CHECK_MEM` made Apple clang fail with `-Wunused-function` under `-Werror`; `ocb_test_hexdump` and `ocb_test_failures` became `static inline` (no behaviour change; Story 2.2's tests unaffected).

### Debug Log References

- First host run: `test_core_modes` and `test_core_edge` failed on the invariant "IDLE implies an empty queue" (cells with an IDLE prefill, property steps after a prefill); resolved by D16 (test-side correction, the core was right).
- First property-test version was too shallow (10 % aborts, 1 in 8 random states an END: few steps in PLAYING); rebalanced and a depth check added (each seed must reach > 500 PLAYING steps and see END, underrun and LATE at least once). Final per-seed figures: `0x12345678`: 3535 ACK, 4099 NAK, 1376 steps in PLAYING, 7 END / 39 underrun / 24 LATE; `0xC0FFEE01`: 3457 / 4232 / 1398, 2 / 29 / 45; `0x0BADF00D`: 3501 / 4142 / 1358, 3 / 23 / 35.
- Red checks (Task 9.2), each applied to a copy-restored tree and reverted (restored tree `diff -r` identical, then 14/14 green again):
  - (a) `#include "pico/stdlib.h"` in `core/modes.c`: `controller/core/modes.c:12:#include "pico/stdlib.h"`, gate rc=1; host build: `modes.c:12:10: fatal error: 'pico/stdlib.h' file not found`.
  - (b) `malloc(` in `core/queue.c`: `controller/core/queue.c:82:void *ocb_red(void) { return malloc(4); }`, gate rc=1.
  - (c) `OCB_BTN_ZL` added to `OCB_CORE_ALLOWED_BUTTONS` (`mask.h`): `test_core_mask.c:57: FAIL: button ZL (0x0040): masked to 0x0040, wanted 0x0000` (and `OCB_CORE_ALLOWED_BUTTONS 0x007d, wanted 0x003d`).
  - (d) ORDER `<=` to `<`: `test_core_end_late.c:31: FAIL: equal t_us: kind 1 NAK 0x00, wanted NAK 0x12`.
  - (e) HELLO path without `enter_idle`: `test_core_sync.c:31: FAIL: HELLO from PLAYING mid-piece seq 0: mode PLAYING` (also not NEUTRAL, queue not cleared).
  - (f) LATE `>=`: `test_core_end_late.c:149: FAIL: overdue by exactly late_us must apply (event 3)` and the 8000 µs fallback case.
  - (g) latch cleared on PING: `test_core_modes.c:215: FAIL: cell 66 PING/IDLE/ab1/rdy1: aborted 0, wanted 1`.
- Review fixes (M1): with `pico_stdlib` added to the real `controller/core/CMakeLists.txt`, an `extra.cpp` including `pico/stdlib.h` and a `gettimeofday(` call in `sched.c`, the gate printed `controller/core/CMakeLists.txt:9:target_link_libraries(ocb_core PUBLIC ocb_proto pico_stdlib)`, `controller/core/extra.cpp:1:#include "pico/stdlib.h"` and `controller/core/sched.c:57:...gettimeofday(0, 0);`, rc=1; reverted (tree `diff -r` identical), gate `ok (11 files)`. Each of the 18 bad fixtures exits 1 with output; `core_pure_selftest: ok (18 bad fixtures)`.
- Review fixes (M2): `modes.c` compiled by hand for ARM with `-DOCB_CORE_RAMFUNC=__attribute__((section(".time_critical.ocb_core")))`: every function of the frame path, static helpers included, is in `.time_critical.ocb_core` (0x59c bytes); only `ocb_core_init`, `ocb_core_set_report_period` and `ocb_core_status` stay in `.text`.
- Sanitizers: the host-tests compile command of the core and tests contains `-fsanitize=address,undefined` (checked with `ninja -t commands`); no sanitizer report in any run.

### Completion Notes List

- Implemented `ocb_core` (`firmware/controller/core/`): queue (`queue.c`), PLAYING mask (`mask.c`, allow-list macro in `mask.h`), modes / frame handling / abort latch / status view (`modes.c`), scheduler pass (`sched.c`), exactly per D1 to D15 and the behaviour table; bodies parsed only with `ocb_msg_*_decode`. No heap, no recursion, no global mutable state (`arm-none-eabi-size`: data 0, bss 0), no 64-bit division (no `__aeabi_uldivmod` among the archive's undefined symbols).
- `OCB_CORE_RAMFUNC` hook checked once by hand: compiling the four sources with `-DOCB_CORE_RAMFUNC=__attribute__((section(".time_critical.ocb_core")))` and the strict flags puts `ocb_core_tick` in `.time_critical.ocb_core` (not a build setting; Story 2.5 decides the attribute).
- Tests: `test_core_modes` (F.3, 96-cell hand-written matrix + latch per cause per mode + count wrap + IDLE entry), `test_core_end_late` (F.4), `test_core_sync` (F.5), `test_core_mask` (F.6, all 65 536 button values through the mask, the full PLAYING path and DIRECT), `test_core_edge` (F.12, including the property test), `core_pure_check` (F.13).
- Commands run from `firmware/` (with `OCB_ARM_TOOLCHAIN=$PWD/.tools/arm-gnu-toolchain-15.2.Rel1`, `OCB_PICOTOOL_DIR=$PWD/.tools/picotool-2.3.1`): `cmake --preset fw-release && cmake --build --preset fw-release --clean-first`, same for `fw-debug` and `host-tests`: all rc 0, **0 warnings** in each build log; `bridge.uf2` and `controller.uf2` produced; the ARM compile of `ocb_core` uses `-std=c99 -Wall -Wextra -Werror -Wpedantic -Wshadow -Wconversion`. `ctest --preset host-tests`: **`100% tests passed out of 14`** (8 existing + 6 new). `tools/check-pins.sh`, `check-proto-version.sh`, `check-proto-pure.sh`, `check-core-pure.sh`: all ok. `shellcheck` clean on the two new scripts, `actionlint` clean on `firmware.yml`.
- ASan/UBSan: active locally with Apple clang (macOS arm64; LeakSanitizer is Linux-only, set through `OCB_TEST_ENV` for the new tests too). Second compiler: Homebrew `gcc-16` host build in the scratchpad (ASan/UBSan on), 14/14 passed, no warnings.
- `arm-none-eabi-size` of the `ocb_core` objects (fw-release, informational for Story 2.5): `queue.c.o` 204, `mask.c.o` 28, `modes.c.o` 913, `sched.c.o` 188 bytes text; total 1333 text, 0 data, 0 bss. The queue storage (512 × 12 = 6 KB) is not allocated by this story.
- CI: green on `3664f57` (run 45005942303, artifact `firmware-uf2`) and on `7b9f060` (run 45006755419, artifact `firmware-uf2`). The step "Controller core is pico-sdk free (AD-2)" and the six new ctest entries run in "Host tests".
- Suggestions for the owner (not done here, per Task 8.4): the risk-register row C6 (`_bmad-output/specs/`) can move to "mitigated in code, hardware proof pending"; the open points of the story (D4 extra refusals, D7 hat not narrowed, D5 END counted in `abort_count`, D6 8000 µs fallback) stand as written.
- After the review fixes (M1 to M4), re-run from `firmware/`: `fw-release`, `fw-debug`, `host-tests` with `--clean-first`: rc 0, 0 warnings each; `ctest --preset host-tests`: `100% tests passed out of 14`; `check-pins`, `check-proto-version`, `check-proto-pure`, `check-core-pure`: ok; `shellcheck` and `actionlint` clean; UF2s produced; `ocb_core` text unchanged at 1333 bytes (default empty `OCB_CORE_RAMFUNC`). CI for these fixes: green on `7b9f060` (run 45006755419).
- Not touched: `common/proto/**` (vectors and `consts.h` unchanged, no wire change), `common/hw/**`, `bridge/**`, `docs/ocarina-input-mechanics.md`, `_bmad/**`, `_bmad-output/specs/**`. No 2wiCC or third-party code.

### File List

Added:
- `firmware/controller/core/CMakeLists.txt`
- `firmware/controller/core/include/ocb_core/core.h`
- `firmware/controller/core/include/ocb_core/queue.h`
- `firmware/controller/core/include/ocb_core/mask.h`
- `firmware/controller/core/include/ocb_core/ramfunc.h`
- `firmware/controller/core/include/ocb_core/internal.h`
- `firmware/controller/core/include/ocb_core/ocb_core.h`
- `firmware/controller/core/queue.c`
- `firmware/controller/core/mask.c`
- `firmware/controller/core/modes.c`
- `firmware/controller/core/sched.c`
- `firmware/tests/core_fixture.h`
- `firmware/tests/test_core_modes.c`
- `firmware/tests/test_core_end_late.c`
- `firmware/tests/test_core_sync.c`
- `firmware/tests/test_core_mask.c`
- `firmware/tests/test_core_edge.c`
- `firmware/tests/core_pure_selftest.sh`
- `firmware/tests/core_pure_fixtures/bad/includes_sdk.c`
- `firmware/tests/core_pure_fixtures/bad/includes_hardware.c`
- `firmware/tests/core_pure_fixtures/bad/includes_ocb_hw.c`
- `firmware/tests/core_pure_fixtures/bad/includes_stdio.c`
- `firmware/tests/core_pure_fixtures/bad/includes_stdlib.c`
- `firmware/tests/core_pure_fixtures/bad/reads_clock.c`
- `firmware/tests/core_pure_fixtures/bad/allocates.c`
- `firmware/tests/core_pure_fixtures/good/allowed.c`
- `firmware/tests/core_pure_fixtures/bad/calls_puts.c` (review M1)
- `firmware/tests/core_pure_fixtures/bad/calls_putchar.c` (review M1)
- `firmware/tests/core_pure_fixtures/bad/calls_stdio_init.c` (review M1)
- `firmware/tests/core_pure_fixtures/bad/reads_clock_gettime.c` (review M1)
- `firmware/tests/core_pure_fixtures/bad/reads_gettimeofday.c` (review M1)
- `firmware/tests/core_pure_fixtures/bad/ext_cpp/sdk.cpp` (review M1)
- `firmware/tests/core_pure_fixtures/bad/ext_cc/sdk.cc` (review M1)
- `firmware/tests/core_pure_fixtures/bad/ext_asm/sdk.S` (review M1)
- `firmware/tests/core_pure_fixtures/bad/ext_inc/table.inc` (review M1)
- `firmware/tests/core_pure_fixtures/bad/cmake_links_sdk/CMakeLists.txt` (review M1)
- `firmware/tests/core_pure_fixtures/bad/cmake_links_board/CMakeLists.txt` (review M1)
- `firmware/tests/core_pure_fixtures/good/CMakeLists.txt` (review M1)
- `firmware/tools/check-core-pure.sh`

Modified:
- `firmware/CMakeLists.txt`
- `firmware/controller/CMakeLists.txt`
- `firmware/tests/CMakeLists.txt`
- `firmware/tests/ocb_test.h`
- `.github/workflows/firmware.yml`
- `docs/test-plan.md`
- `docs/link-protocol.md`
- `firmware/controller/README.md`
- `firmware/README.md`
- `firmware/THIRD_PARTY_NOTICES.md`
- `_bmad-output/implementation-artifacts/sprint-status.yaml`
- `_bmad-output/implementation-artifacts/2-4-controller-core-modes-queue-scheduler-and-safety-mask.md`

### Change Log

- 2026-11-22: Story 2.4 implemented: `ocb_core` library (modes, queue, scheduler, abort latch, safety mask), host tests F.3 to F.6, F.12, F.13, purity gate `check-core-pure.sh` with CI step, docs (test plan, link protocol rules, READMEs, notices). Decisions D16 to D21 recorded. Status: review.
- 2026-11-22: Story 2.4 done: code review APPROVE, review follow-ups M1 to M4 fixed in `7b9f060`, CI green on `7b9f060`, PR opened against `main` (not merged).
- 2026-11-22: Addressed code review findings - 4 items resolved (M1 to M4: gate extended to more file types, names and CMake links; static hot-path helpers carry `OCB_CORE_RAMFUNC`; link-protocol text on the two extra refusals and the HELLO version order). M5 to M7 deferred with reasons. Decision D22 added. Status stays review.
