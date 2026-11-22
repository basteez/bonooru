# controller

Switch-side board: wired Pro Controller emulation, on-device timeline scheduling, link watchdog (neutral within 100 ms) (milestone M3).

Currently core 0 blinks the onboard LED amber at 2 Hz (Story 2.1) and core 1 runs the board
link (Story 2.6, bring-up loop in `main.c`) with its link watchdog (Story 2.7).

## `core/`: the playback core (Story 2.4)

`core/` is `ocb_core`, the controller's playback logic as a pure C99 library: the modes
IDLE / DIRECT / PLAYING (AD-8), the event queue, the scheduler pass, the abort latch and the
PLAYING safety mask (AD-9). It reads no clock and does no I/O: every call gets the time and the
profile's `ready()` as arguments, so it runs unchanged on the PC. It includes only the standard
headers on its allow-list, `ocb_proto/` and `ocb_core/` (`tools/check-core-pure.sh`, ctest
`core_pure_check`, its own CI step). Its tests (F.3 to F.6, F.12 in `docs/test-plan.md`) run with
the host preset: `cmake --preset host-tests && cmake --build --preset host-tests && ctest --preset host-tests`.
Since Story 2.6 the link layer drives it on core 1; Stories 2.5, 2.3 and 2.7 wire it to the
timer, USB and the watchdog.

RAM placement (for Story 2.5): the hot functions of the core, including its private helpers, carry
the `OCB_CORE_RAMFUNC` macro (empty by default, `ocb_core/ramfunc.h`). It does not cover the
`ocb_proto` functions the core calls (message decoders, state predicates), the `OCB_NEUTRAL`
constant or compiler-generated read-only tables; Story 2.5 must review where those live before
claiming the hot path runs from RAM.

## `link/`: the link layer (Story 2.6)

`link/` is `ocb_link`, the controller side of the PC <-> controller link as a pure C99 library
next to `ocb_core`: the RX error-byte policy and resync on top of the `ocb_rx` splitter, the
session (HELLO / `NAK VERSION`), the go-back-N front (stale window, cached response, cumulative
ACK, one transport NAK per `expected`), the console `ready` predicate fed by a cross-core USB
mailbox, the TX slots (EVENT > NAK > INFO > ACK > STATUS), STATUS with its cadence and debug
counters, and the TX gate state machine (AD-20). It reads no clock, does no I/O and allocates
nothing: the glue passes RX entries and the time and pulls whole wire frames. Its rules are in
`docs/link-protocol.md`, "Rules the controller link applies (Story 2.6)". Purity:
`tools/check-link-pure.sh` (ctest `link_pure_check`, its own CI step). Tests F.1, F.2, F.10,
F.14 to F.17 in `docs/test-plan.md`, under the host preset.

## `platform/`: the glue on core 1 (Story 2.6)

`platform/link_runner.c` connects `ocb_link` and `ocb_core` to the UART driver
`common/hw/uart_link.c`: `ocb_ctrl_link_init()` (512-event static queue, version from the
generated header, profile P2), `ocb_ctrl_link_start_rx()` (RX interrupt enabled from core 1) and
`ocb_ctrl_link_step()` (one non-blocking pass: RX level into the TX gate, poll (USB mailbox,
`ready`, scheduler), RX ring into the link, at most one frame into the UART TX ring). `main.c`
zeroes the USB mailbox before core 1 starts, and the link counts mailbox events from zero, so
core-0 posts made before core 1 is up are not lost. `main.c` launches core 1 with a bring-up loop
that only calls `ocb_ctrl_link_step()`; Story 2.5 replaces that loop with the real super-loop
and keeps the call. Core 1 is the only UART TX writer (a debug-build assertion). Nothing posts
to the USB mailbox until Story 2.3, so on this firmware `console_ready` is always 0 and `START` /
`SET_STATE` get `NAK NOT_READY`. Bench sender: `tools/link_probe.py`.

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

Bench-only build: the preset `fw-debug-bench` (CMake option `OCB_BENCH_FAKE_READY`, its own
build directory) makes core 0 post a console handshake and an IN report every millisecond to
the USB mailbox, so `console_ready` reads 1 and DIRECT and PLAYING can be tested on the bench
before Story 2.3 brings real USB (test plan H.11). It is debug-only on purpose (`main.c` and
CMake refuse it otherwise): a build that claims a ready console without one must never be
flashed for real use; CI compiles it as a gate and never uploads its UF2. Story 2.3 removes the
option or keeps it for the same bench purpose.
