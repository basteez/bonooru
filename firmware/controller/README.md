# controller

Switch-side board: wired Pro Controller emulation, on-device timeline scheduling, link watchdog (neutral within 100 ms) (milestone M3).

Core 0 blinks the onboard LED amber at 2 Hz (Story 2.1) and runs the USB stack, the wired Pro
Controller profile P2 (Story 2.3); core 1 runs the board link (Story 2.6) with its link watchdog
(Story 2.7) in the AD-11 super-loop of Story 2.5 (`main.c`). Both cores watch each other and a
hardware watchdog resets the board when one hangs (Story 2.5, below). The whole program runs from
RAM (`copy_to_ram`, Story 2.5 D2).

## `core/`: the playback core (Story 2.4)

`core/` is `ocb_core`, the controller's playback logic as a pure C99 library: the modes
IDLE / DIRECT / PLAYING (AD-8), the event queue, the scheduler pass, the abort latch and the
PLAYING safety mask (AD-9). It reads no clock and does no I/O: every call gets the time and the
profile's `ready()` as arguments, so it runs unchanged on the PC. It includes only the standard
headers on its allow-list, `ocb_proto/` and `ocb_core/` (`tools/check-core-pure.sh`, ctest
`core_pure_check`, its own CI step). Since Story 4.0 the gate also applies the shared rules of
`tools/pure-rules.sh`: no heap, I/O, clock, sleep or multicore name, no process exit in call form
(`exit(`, `abort(` and the like), no `fopen`, `strdup`, `strndup`, `alloca` or
`__builtin_alloca*`, no inline assembly, no raw register address (a hex literal after a pointer
cast, or an 8-digit literal in an RP2040 register or XIP window), and no `section(` attribute
except in `ramfunc.h`; the exact forms and the accepted false positives are in
`firmware/README.md` and the header of `tools/pure-rules.sh`. Its tests (F.3 to F.6, F.12 in `docs/test-plan.md`) run with
the host preset: `cmake --preset host-tests && cmake --build --preset host-tests && ctest --preset host-tests`.
Since Story 2.6 the link layer drives it on core 1; Stories 2.5, 2.3 and 2.7 wire it to the
timer, USB and the watchdog.

RAM placement: the hot functions of the core, including its private helpers, carry the
`OCB_CORE_RAMFUNC` macro (empty, `ocb_core/ramfunc.h`). Story 2.5 D2 links the controller with
the SDK's `copy_to_ram` binary type instead, so the whole program (the `ocb_proto` decoders,
`OCB_NEUTRAL`, the SDK, TinyUSB and the compiler's tables included) runs from SRAM;
`tools/check-ram-hot-path.sh` proves the hot functions are in RAM (CI). The CMake option
`OCB_CONTROLLER_RUN_FROM_RAM=OFF` (preset `fw-release-xip`) builds the XIP variant for the
comparison H.20 and the gate's self-test only.

Story 2.5 adds to `ocb_core`: `live.c` (the liveness rules: a peer counter unchanged for more than
20 ms is stale, the 60 ms hardware watchdog, the reboot-cause encoding), `timing.c` (the
scheduler's lateness statistics, per run, reported by the debug STATUS v4) and the built-in test
timeline (`testtl.c` and the generated `testtl_data.c`, a separate library `ocb_core_testtl`
linked only by the timeline flavour; regenerate with `python3 firmware/tools/gen_test_timeline.py`,
ctest `testtl_drift` fails on drift).

## `link/`: the link layer (Story 2.6)

`link/` is `ocb_link`, the controller side of the PC <-> controller link as a pure C99 library
next to `ocb_core`: the RX error-byte policy and resync on top of the `ocb_rx` splitter, the
session (HELLO / `NAK VERSION`), the go-back-N front (stale window, cached response, cumulative
ACK, one transport NAK per `expected`), the console `ready` predicate fed by a cross-core USB
mailbox, the TX slots (EVENT > NAK > INFO > ACK > STATUS), STATUS with its cadence and debug
counters, and the TX gate state machine (AD-20). It reads no clock, does no I/O and allocates
nothing: the glue passes RX entries and the time and pulls whole wire frames. Its rules are in
`docs/link-protocol.md`, "Rules the controller link applies (Story 2.6)". Purity:
`tools/check-link-pure.sh` (ctest `link_pure_check`, its own CI step), with the same allow-list
idea plus `ocb_link/` and the same shared identifier rules as the core gate (Story 4.0); a
section attribute belongs in `ocb_core/ramfunc.h` only (`OCB_CORE_RAMFUNC`). Tests F.1, F.2, F.10,
F.14 to F.17 in `docs/test-plan.md`, under the host preset.

## `platform/`: the glue on core 1 (Story 2.6)

`platform/link_runner.c` connects `ocb_link` and `ocb_core` to the UART driver
`common/hw/uart_link.c`: `ocb_ctrl_link_init()` (512-event static queue, version from the
generated header, profile P2), `ocb_ctrl_link_start_rx()` (RX interrupt enabled from core 1) and
`ocb_ctrl_link_step()` (one non-blocking pass: RX level into the TX gate, poll (USB mailbox,
`ready`, scheduler), RX ring into the link, at most one frame into the UART TX ring, then the
published state). `main.c` zeroes the USB mailbox before core 1 starts, and the link counts
mailbox events from zero, so core-0 posts made before core 1 is up are not lost. Core 1's
super-loop (Story 2.5, `main.c`): per pass the liveness step, `ocb_ctrl_link_step()` and, in debug
builds, `ocb_ctrl_link_pass_end()` (the loop maximum since `START` and one step of the stack
high-water scan, both into the debug STATUS v4); no sleep, no blocking call, no stdio, no divide;
the scheduler is the polled 1 MHz timer (no alarm interrupt). Core 1 is the only UART TX writer (a
debug-build assertion). Since Story 2.3 the P2 glue on core 0 posts to the USB mailbox
(handshake, IN reports, suspend / unmount / bus reset, the measured report period; Story 2.5: the
IN interval spread, its own longest pass and the handoff counters; Story 4.0: the P2 reply-queue
overflows, `p2_queue_full`, for the debug STATUS v6 after the v4 and Story 3.5 v5 blocks);
`ocb_ctrl_published_read()`
is the state the profile reports: core 1's published state through the seqlock double buffer of
`link/pubstate.c`, in every build (Story 2.5 D4: 3 tries, then NEUTRAL). In debug builds
`ocb_ctrl_link_step()` drains the P2 debug log into the debug STATUS. Bench sender:
`tools/link_probe.py`.

## Liveness and the hardware watchdog (Story 2.5)

`platform/liveness.c`, in every build: each core counts its passes; core 0 feeds the hardware
watchdog (60 ms) only while core 1's count moves (stale after more than 20 ms), and reports
NEUTRAL once core 1 is stale; core 1 aborts the core (`CORE_STALL`) and reboots the board at once
when core 0's count stops. Worst case stall to reset: 20 + 60 + 5 ms = 85 ms (AC4, NFR-05; a
compile-time check in `ocb_core/live.h`). The cause and a reboot count survive the reset in the
watchdog scratch registers 0 and 1 (`scratch[4..7]` are the SDK's) and are reported by the debug
STATUS v4. The cross-core barrier of `ocb_link` is a real `dmb sy` on the target
(`platform/ocb_link_barrier.h`, force-included; `tools/check-barrier.sh`, CI).

## Debug flavours (Story 2.5)

Each hardware check of Story 2.5 flashes one UF2 (presets in `firmware/CMakePresets.json`;
`Release` optimisation with the debug counters, so the measured timing is the release timing):

- `fw-debug-timeline` (`OCB_DEBUG_TIMELINE=ON`): after the host's `HELLO` (and after every `STOP`
  or `HELLO` that empties the queue) the built-in melody (the Story 1.6 Ode to Joy, 158 events) is
  loaded into the core through the real validation path; the host sends only `HELLO` and
  `START lead_us` (`link_probe.py selfplay`, H.18) and must not send its own `QUEUE_EVENT`s.
- `fw-debug-hang0`, `fw-debug-hang1` (`OCB_DEBUG_HANG=CORE0|CORE1`, with the timeline): the chosen
  core stops looping `OCB_DEBUG_HANG_DELAY_US` (50 ms) after the first note was published or read,
  with the note held (`p2_probe.py stall`, H.2a/H.2b).

Guards (configure stops with the fix in the message): `OCB_DEBUG_TIMELINE` and `OCB_DEBUG_HANG`
need `OCB_DEBUG_BUILD=ON`; `OCB_DEBUG_HANG` needs `OCB_DEBUG_TIMELINE=ON`. A release ELF is proven
free of the melody and the injectors by `tools/check-release-nolog.sh` (CI).

## The link watchdog (Story 2.7)

The fail-safe rules of AD-5 live in `link/watchdog.c` (`ocb_link/watchdog.h`), pure C99 inside
`ocb_link`, covered by the same purity gate and tested on a fake clock (F.21): armed by the
first valid bridge `HEARTBEAT`; more than 80 ms without one (`OCB_LINK_TIMEOUT_MS`, clamped to
100) or a UART break while armed is a link loss (abort to IDLE and NEUTRAL, `EVENT LINK_LOST`,
`link_ok` 0, then the TX pad released once that EVENT has left the UART, at most 5 ms later); in
DIRECT only, no app frame for `app_timeout_ms` (1000) or a DTR falling edge in the heartbeat
aborts with no EVENT; while the link is down `START` and `SET_STATE` get `NAK NO_LINK` (decided
by the core from `env.link_ok`). The glue adds one call per pass, `ocb_link_tx_settled()` with
the driver's `ocb_uart_link_tx_idle()`. Rules: `docs/link-protocol.md`, "Rules the controller
link applies (Story 2.7)".

The Story 2.7 bench-only build (`fw-debug-bench`, a faked `console_ready`) was removed by Story
2.3 D15: DIRECT and PLAYING are tested on the bench with a real host holding the handshake
(`tools/p2_probe.py handshake --hold N`, test plan H.11).

## `profile_p2/`: the wired Pro Controller profile (Story 2.3)

`profile_p2/` is `ocb_p2`, the P2 profile of AD-12 as a pure C99 library: the USB descriptors
(`desc.c`: the real controller's bytes, `bInterval` as a build constant), the reactive handshake
and subcommand answers with a reply queue of 4 (`p2.c`), the emulated SPI flash as a function of
the address, the refusals of C7 (SPI write and erase, reset pairing, resets, the MCU update
path, feature reports; the console's pairing request and MCU configuration get the real
controller's replies and change nothing, D34), the `0x30` report from a `const ocb_state *` (`report.c`) and the median IN
interval (`interval.c`). Clean-room from public reverse-engineering documents, every value cited
(sources and tables: `docs/usb-controller-emulation.md`, "P2 wire protocol"); no 2wiCC code.
Purity: `tools/check-p2-pure.sh` (no SDK, no TinyUSB, never `ocb_core` or `ocb_link`). Tests
F.24 to F.30 under the host preset, including a deterministic fuzz under ASan and UBSan.

## `platform/usb_p2.c`: the USB glue on core 0 (Story 2.3)

The only code that includes TinyUSB (0.18.0, `platform/tusb_config.h`: device mode, one HID
interface, 64-byte reports). It returns `ocb_p2`'s descriptors, passes every OUT report to
`ocb_p2_on_output()`, sends `ocb_p2_next_in()` whenever the IN endpoint is free, restarts the
profile and posts to the USB mailbox on mount, unmount, suspend and bus reset (the bus reset is
seen only through `tud_event_hook_cb`, in the interrupt; it is handled in the task), stamps
each 0x30 report completion for the median and puts it in the mailbox (Story 2.5: with the
minimum, 95th percentile, maximum and slow-interval count of the same window, and its own longest
pass, which bounds the stamp latency), reports NEUTRAL once core 1 is stale, and in debug builds
pushes the profile's log records into the SPSC ring. `usb_p2_task()` runs once per pass of core 0's plain
loop, next to the LED blink; it never blocks. The MAC is derived from the board's unique id
(locally administered). Build options: `OCB_PROFILE` (`P2` only), `OCB_P2_BINTERVAL` (1..16,
default 1). Bench tools: `tools/p2_probe.py` (Mac) and `link_probe.py p2check` / `p2watch`
(console), test plan H.13 to H.17.
