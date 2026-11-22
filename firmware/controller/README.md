# controller

Switch-side board: wired Pro Controller emulation, on-device timeline scheduling, link watchdog (neutral within 100 ms) (milestone M3).

Currently (Story 2.1) it only blinks the onboard LED amber at 2 Hz.

## `core/`: the playback core (Story 2.4)

`core/` is `ocb_core`, the controller's playback logic as a pure C99 library: the modes
IDLE / DIRECT / PLAYING (AD-8), the event queue, the scheduler pass, the abort latch and the
PLAYING safety mask (AD-9). It reads no clock and does no I/O: every call gets the time and the
profile's `ready()` as arguments, so it runs unchanged on the PC. It includes only the standard
headers on its allow-list, `ocb_proto/` and `ocb_core/` (`tools/check-core-pure.sh`, ctest
`core_pure_check`, its own CI step). Its tests (F.3 to F.6, F.12 in `docs/test-plan.md`) run with
the host preset: `cmake --preset host-tests && cmake --build --preset host-tests && ctest --preset host-tests`.
The firmware links it, but `main.c` still only blinks the LED; Stories 2.5 to 2.7 wire it to the
timer, USB and the link.
