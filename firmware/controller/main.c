/*
 * SPDX-License-Identifier: MIT
 *
 * controller firmware, Story 2.1 skeleton: blinks the onboard LED amber at
 * 2 Hz (250 ms on, 250 ms off), so the two boards are told apart at a
 * glance (Decisions D3, D5).
 * Later stories replace this super-loop with the real firmware.
 */
#include <stdbool.h>
#include <stdint.h>

#include "ocb_hw/led.h"
#include "pico/stdlib.h"

/* Colour and rhythm, tuned at the bench (Story 2.1 Task 10). About 6 % of full
 * scale: comfortable at arm's length and a few mA of LED current. */
#define BLINK_R 16u
#define BLINK_G 6u
#define BLINK_B 0u
#define BLINK_HALF_PERIOD_US 250000u

int main(void)
{
    ocb_led_init();

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
