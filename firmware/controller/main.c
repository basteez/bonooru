/*
 * SPDX-License-Identifier: MIT
 *
 * controller firmware. Core 0: the Story 2.1 LED blink, amber at 2 Hz
 * (250 ms on, 250 ms off), so the two boards are told apart at a glance
 * (Decisions D3, D5). Core 1 (Story 2.6 bring-up): the board link, UART
 * plus ocb_link plus ocb_core, in a plain loop.
 * Story 2.5 replaces core1_entry with the real core 1 super-loop (liveness
 * counters, seqlock, 1 MHz timer) and keeps calling ocb_ctrl_link_step()
 * from it; Story 2.3 adds the USB stack on core 0 and posts to the USB
 * mailbox. Until then console_ready is always 0, so START and SET_STATE
 * get NAK NOT_READY.
 */
#include <stdbool.h>
#include <stdint.h>

#include "link_runner.h"
#include "ocb_hw/led.h"
#include "pico/multicore.h"
#include "pico/stdlib.h"

/* Colour and rhythm, to be tuned at the bench (Story 2.1 Task 10). About 6 %
 * of full scale: comfortable at arm's length and a few mA of LED current. */
#define BLINK_R 16u
#define BLINK_G 6u
#define BLINK_B 0u
#define BLINK_HALF_PERIOD_US 250000u

/* Core 1: init, the RX interrupt enabled from this core (the NVIC enable
 * is per core), then the link loop. Core 1 is the only UART TX writer. */
static void core1_entry(void)
{
    ocb_ctrl_link_init();
    ocb_ctrl_link_start_rx();
    for (;;) {
        ocb_ctrl_link_step();
    }
}

int main(void)
{
    ocb_led_init();
    ocb_ctrl_usb_mailbox_init(); /* before core 1 and before any core-0 post */
    multicore_launch_core1(core1_entry);

    bool on = false;
    uint32_t next_edge = time_us_32();

    for (;;) {
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
