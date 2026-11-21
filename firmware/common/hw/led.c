/*
 * SPDX-License-Identifier: MIT
 *
 * Non-blocking WS2812 LED driver for the RP2040-Zero onboard LED.
 *
 * Derived in part from the pico-examples `pio/ws2812` example
 * (https://github.com/raspberrypi/pico-examples/tree/sdk-2.3.1/pio/ws2812),
 * Copyright (c) 2020 Raspberry Pi (Trading) Ltd., BSD-3-Clause; modified:
 * non-blocking write, GRB word packing, own claim/init wrapper.
 * The PIO program itself is vendored verbatim in
 * firmware/third_party/pico-examples/ws2812/ws2812.pio. See
 * firmware/THIRD_PARTY_NOTICES.md.
 */
#include "ocb_hw/led.h"

#include "hardware/pio.h"
#include "ocb_hw/pins.h"
#include "ws2812.pio.h"

/* WS2812 bit rate (Hz), as in the pico-examples ws2812 example. */
#define OCB_LED_BITRATE_HZ 800000.0f

static PIO s_pio;
static uint s_sm;
static bool s_ready;

void ocb_led_init(void)
{
    uint offset;
    s_ready = pio_claim_free_sm_and_add_program_for_gpio_range(
        &ws2812_program, &s_pio, &s_sm, &offset, OCB_PIN_LED, 1, true);
    if (!s_ready) {
        return; /* no free state machine: the LED stays dark, nothing blocks */
    }
    ws2812_program_init(s_pio, s_sm, offset, OCB_PIN_LED, OCB_LED_BITRATE_HZ, false);
}

bool ocb_led_set(uint8_t r, uint8_t g, uint8_t b)
{
    if (!s_ready || pio_sm_is_tx_fifo_full(s_pio, s_sm)) {
        return false;
    }
    /* GRB order, 24 bits left-aligned for the program's 24-bit out_shift. */
    const uint32_t grb = ((uint32_t)g << 16) | ((uint32_t)r << 8) | (uint32_t)b;
    pio_sm_put(s_pio, s_sm, grb << 8);
    return true;
}
