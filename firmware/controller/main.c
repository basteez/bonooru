/*
 * SPDX-License-Identifier: MIT
 *
 * controller firmware (AD-11, story 2.5). Core 0: the Story 2.1 LED blink,
 * amber at 2 Hz (250 ms on, 250 ms off), so the two boards are told apart at
 * a glance (Decisions D3, D5), the USB stack (story 2.3: TinyUSB and the P2
 * profile, platform/usb_p2.c) polled once per pass of the same plain loop
 * (no sleep, no blocking call), and the liveness rule of core 0 (it feeds
 * the hardware watchdog only while core 1 is alive). Core 1: the super-loop
 * of story 2.5: non-blocking, no sleep, no stdio, no divide; per pass the
 * liveness rule of core 1, the board link (UART, ocb_link, ocb_core with the
 * 1 MHz polled scheduler, D3) and, in debug builds, the loop-time and stack
 * measurements of the debug STATUS. The whole program runs from RAM
 * (copy_to_ram, D2). console_ready comes from the console's real handshake
 * through the USB mailbox (story 2.3 D5, D11).
 */
#include <stdbool.h>
#include <stdint.h>

#include "link_runner.h"
#include "liveness.h"
#include "ocb_hw/led.h"
#include "pico/multicore.h"
#include "pico/stdlib.h"
#include "usb_p2.h"
#ifdef OCB_DEBUG_BUILD
#include "stack_mark.h"
#endif

/* Colour and rhythm, to be tuned at the bench (Story 2.1 Task 10). About 6 %
 * of full scale: comfortable at arm's length and a few mA of LED current. */
#define BLINK_R 16u
#define BLINK_G 6u
#define BLINK_B 0u
#define BLINK_HALF_PERIOD_US 250000u

/* Core 1: init, the RX interrupt enabled from this core (the NVIC enable
 * is per core), then the super-loop. Core 1 is the only UART TX writer and
 * the only writer of the published state. */
static void core1_entry(void)
{
    liveness_core1_start();
    ocb_ctrl_link_init();
    ocb_ctrl_link_start_rx();
    for (;;) {
#ifdef OCB_DEBUG_BUILD
        const uint32_t pass_start = time_us_32();
#endif
        liveness_core1_step();
        ocb_ctrl_link_step();
#ifdef OCB_DEBUG_BUILD
        ocb_ctrl_link_pass_end(pass_start);
#endif
    }
}

int main(void)
{
    ocb_led_init();
    ocb_ctrl_usb_mailbox_init(); /* before core 1 and before any core-0 post */
    usb_p2_init();               /* may post before core 1 is up (D28 rebase) */
#ifdef OCB_DEBUG_BUILD
    stack_mark_paint();          /* story 2.5 D15: before core 1 uses its stack */
#endif
    liveness_boot();             /* boot cause, then the hardware watchdog (D6, D7) */
    multicore_launch_core1(core1_entry);

    bool on = false;
    uint32_t next_edge = time_us_32();

    for (;;) {
        usb_p2_task();
        liveness_core0_step();
        const uint32_t now = time_us_32();
        /* Signed difference: wrap-safe across the 32-bit microsecond counter. */
        if ((int32_t)(now - next_edge) >= 0) {
            const bool want_on = !on;
            const bool queued = want_on ? ocb_led_set(BLINK_R, BLINK_G, BLINK_B)
                                        : ocb_led_set(0u, 0u, 0u);
            if (queued) {
                on = want_on;
                /* From now, not from the missed edge: two writes are never
                 * closer than a half period (WS2812 latch time). */
                next_edge = now + BLINK_HALF_PERIOD_US;
            }
        }
    }
}
