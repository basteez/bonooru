/*
 * SPDX-License-Identifier: MIT
 *
 * Liveness glue and the hardware watchdog (AD-11, story 2.5 D6, D7). Rules
 * in liveness.h and ocb_core/live.h. Written for this firmware from the
 * pico-sdk 2.3.1 headers and hardware_watchdog/watchdog.c (read for the
 * story: watchdog_enable and watchdog_reboot reset every block but ROSC
 * and XOSC, the USB controller included, and use scratch[4..7]); no code
 * copied.
 */
#include "liveness.h"

#include <stdbool.h>
#include <stdint.h>

#include "hardware/structs/watchdog.h"
#include "hardware/watchdog.h"
#include "link_runner.h"
#include "ocb_core/live.h"
#include "pico/stdlib.h"

/* Single writer each: core 0 writes s_core0_pass, core 1 s_core1_pass;
 * aligned 32-bit stores are atomic on Cortex-M0+. */
static volatile uint32_t s_core0_pass;
static volatile uint32_t s_core1_pass;

/* Core 0 only. */
static ocb_live_peer s_core1_peer;
static bool s_core1_peer_init;
static volatile bool s_core1_stale; /* written by core 0, read by core 0 (USB pump) */
static bool s_cause_written;
static uint8_t s_boot_cause;
static uint8_t s_reboot_count;

/* Core 1 only. */
static ocb_live_peer s_core0_peer;

static void record_cause(uint8_t cause)
{
    watchdog_hw->scratch[0] = ocb_live_cause_encode(cause);
    const uint8_t n = ocb_live_scratch_decode(watchdog_hw->scratch[1]);
    watchdog_hw->scratch[1] = ocb_live_scratch_encode(n == 0xFFu ? n : (uint8_t)(n + 1u));
}

void liveness_boot(void)
{
    s_boot_cause = ocb_live_cause_decode(watchdog_hw->scratch[0]);
    s_reboot_count = ocb_live_scratch_decode(watchdog_hw->scratch[1]);
    if (!watchdog_caused_reboot()) {
        /* Power-on or a reset by the RUN pin or BOOTSEL: whatever the scratch
         * holds is not ours to report. */
        s_boot_cause = (uint8_t)OCB_LIVE_CAUSE_NONE;
    }
    watchdog_hw->scratch[0] = 0u; /* read once, cleared */
    watchdog_hw->scratch[1] = ocb_live_scratch_encode(s_reboot_count);
    s_core0_pass = 0u;
    s_core1_pass = 0u;
    s_core1_peer_init = false;
    s_core1_stale = false;
    s_cause_written = false;
    watchdog_enable(OCB_WDT_TIMEOUT_MS, true);
}

uint8_t liveness_boot_cause(void)
{
    return s_boot_cause;
}

uint8_t liveness_reboot_count(void)
{
    return s_reboot_count;
}

void __not_in_flash_func(liveness_core0_step)(void)
{
    s_core0_pass = s_core0_pass + 1u;
    const uint64_t now = time_us_64();
    const uint32_t c1 = s_core1_pass;
    if (!s_core1_peer_init) {
        /* First observation: the grace window starts (BOOT_NOT_ALIVE). */
        ocb_live_peer_init(&s_core1_peer, c1, now);
        s_core1_peer_init = true;
    }
    const bool stale = ocb_live_peer_stale(&s_core1_peer, c1, now);
    if (ocb_live_core0_may_feed(stale) && !s_core1_stale) {
        watchdog_update();
        return;
    }
    /* Latched until the reset: NEUTRAL reports, no feeding. */
    s_core1_stale = true;
    if (!s_cause_written) {
        s_cause_written = true;
        record_cause(ocb_live_core1_cause(&s_core1_peer));
    }
}

bool __not_in_flash_func(liveness_core1_stale)(void)
{
    return s_core1_stale;
}

void liveness_core1_start(void)
{
    ocb_live_peer_init(&s_core0_peer, s_core0_pass, time_us_64());
}

void __not_in_flash_func(liveness_core1_step)(void)
{
    s_core1_pass = s_core1_pass + 1u;
    const bool stale = ocb_live_peer_stale(&s_core0_peer, s_core0_pass, time_us_64());
    if (ocb_live_core1_must_reboot(stale)) {
        ocb_ctrl_link_abort_stall(); /* NEUTRAL published, latch set */
        record_cause((uint8_t)OCB_LIVE_CAUSE_CORE0_STALE);
        watchdog_reboot(0u, 0u, 0u);
        for (;;) {
            tight_loop_contents(); /* the reset is immediate */
        }
    }
}
