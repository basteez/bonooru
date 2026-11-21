/*
 * SPDX-License-Identifier: MIT
 *
 * Pin allow-list for both boards (Waveshare RP2040-Zero), AD-20.
 * This is the ONLY file in firmware/ where a GPIO number may appear;
 * firmware/tools/check-pins.sh fails CI on a pin number anywhere else.
 * Pure C, no pico-sdk includes: host tests include it too.
 */
#ifndef OCB_HW_PINS_H
#define OCB_HW_PINS_H

/* GP0: UART0 TX of the PC <-> controller board link (bridge and controller). */
#define OCB_PIN_UART_TX 0u

/* GP1: UART0 RX of the board link. */
#define OCB_PIN_UART_RX 1u

/* GP16: the onboard WS2812 RGB LED (the small square between BOOT and RESET). */
#define OCB_PIN_LED 16u

#ifdef OCB_DEBUG_BUILD
/* GP15: debug-only toggle for timing measurements (scope probe); never in
 * release builds. Broken out on the Zero's edge, away from GP0/GP1. */
#define OCB_PIN_DEBUG_TOGGLE 15u
#endif

#endif /* OCB_HW_PINS_H */
