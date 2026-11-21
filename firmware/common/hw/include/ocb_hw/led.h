/*
 * SPDX-License-Identifier: MIT
 *
 * Non-blocking driver for the onboard WS2812 RGB LED (AC2, Story 2.1).
 */
#ifndef OCB_HW_LED_H
#define OCB_HW_LED_H

#include <stdbool.h>
#include <stdint.h>

/* Claim a free PIO state machine, load the ws2812 program and configure the
 * LED pin. The only pin this firmware configures at this stage. */
void ocb_led_init(void);

/* Queue one colour (8-bit per channel) without blocking.
 * Returns false if the PIO TX FIFO is full; the caller retries on its next
 * pass. Call only on a colour change or a blink edge, never in a tight loop:
 * the WS2812 latches a frame after a low period of at least ~280 us, and two
 * words sent closer together than that merge into one frame. */
bool ocb_led_set(uint8_t r, uint8_t g, uint8_t b);

#endif /* OCB_HW_LED_H */
