/*
 * SPDX-License-Identifier: MIT
 *
 * Hang injectors of the debug hang flavours (story 2.5 D9, D18). Rules in
 * debug_hang.h.
 */
#include "debug_hang.h"

#include <stdbool.h>
#include <stdint.h>

#include "ocb_core/mask.h"
#include "pico/stdlib.h"

#ifndef OCB_DEBUG_HANG_DELAY_US
#error "OCB_DEBUG_HANG_DELAY_US must be defined by CMake (story 2.5 Task 8.1)"
#endif

#define DEBUG_HANG_K 1u /* the K-th pressed state arms the injector (the first note) */

static bool s_was_pressed;
static uint32_t s_pressed_seen;
static bool s_armed;
static uint64_t s_armed_at_us;

void debug_hang_note(const ocb_state *st)
{
    const bool pressed = (st->buttons & (uint16_t)OCB_CORE_ALLOWED_BUTTONS) != 0u;
    if (pressed && !s_was_pressed && !s_armed) {
        s_pressed_seen++;
        if (s_pressed_seen >= DEBUG_HANG_K) {
            s_armed = true;
            s_armed_at_us = time_us_64();
        }
    }
    s_was_pressed = pressed;
}

void debug_hang_poll(void)
{
    if (!s_armed || time_us_64() - s_armed_at_us < (uint64_t)OCB_DEBUG_HANG_DELAY_US) {
        return;
    }
    /* The hang: no more passes on this core, interrupts still enabled. */
    for (;;) {
        __compiler_memory_barrier();
    }
}
