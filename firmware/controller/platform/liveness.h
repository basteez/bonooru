/*
 * SPDX-License-Identifier: MIT
 *
 * Liveness glue of the two controller cores and the hardware watchdog
 * (AD-11, story 2.5 D6, D7). The rules and numbers are in the pure
 * ocb_core/live.h; this file owns the two pass counters, the watchdog and
 * its scratch registers. In every build (release included): only the hang
 * injectors are debug-only (debug_hang.h).
 *
 *  - core 0 counts one pass per usb_p2_task() pass (the report pump ran;
 *    a console that stops polling is not a stalled core 0) and feeds the
 *    watchdog only while core 1's counter is fresh; when core 1 is stale
 *    it stops feeding, records the cause once, and reports NEUTRAL until
 *    the reset (at most OCB_WDT_TIMEOUT_MS later);
 *  - core 1 counts one pass per super-loop pass; when core 0 is stale it
 *    aborts the playback core (OCB_ABORT_CORE_STALL: NEUTRAL, latch),
 *    records the cause and reboots at once (watchdog_reboot(0, 0, 0)).
 *
 * Scratch registers (they survive a watchdog reset, not a power loss):
 * scratch[0] = magic | cause, read and cleared once at boot; scratch[1] =
 * magic | reboot count since power-on. scratch[4..7] belong to the SDK.
 */
#ifndef OCB_CTRL_LIVENESS_H
#define OCB_CTRL_LIVENESS_H

#include <stdbool.h>
#include <stdint.h>

/* Core 0, once, before core 1 is launched: reads and clears the boot
 * cause, then enables the hardware watchdog (OCB_WDT_TIMEOUT_MS), so a
 * core 1 that never starts is caught (BOOT_NOT_ALIVE). */
void liveness_boot(void);

/* The boot cause and reboot count read by liveness_boot (OCB_LIVE_CAUSE_*). */
uint8_t liveness_boot_cause(void);
uint8_t liveness_reboot_count(void);

/* Core 0, once per pass, after usb_p2_task(): counts the pass, observes
 * core 1, feeds the watchdog while core 1 is fresh. */
void liveness_core0_step(void);

/* Core 0: core 1 was seen stale (the USB report must be NEUTRAL). */
bool liveness_core1_stale(void);

/* Core 1, once, at the start of its entry function. */
void liveness_core1_start(void);

/* Core 1, once per super-loop pass: counts the pass, observes core 0,
 * reboots when core 0 is stale. */
void liveness_core1_step(void);

#endif /* OCB_CTRL_LIVENESS_H */
