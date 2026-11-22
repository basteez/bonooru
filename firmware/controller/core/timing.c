/*
 * SPDX-License-Identifier: MIT
 *
 * Self-timing of the controller: lateness statistics and the loop-time
 * helper (story 2.5 D11, Task 11). Rules in ocb_core/timing.h.
 */
#include <stdint.h>
#include <string.h>

#include "ocb_core/timing.h"

static OCB_CORE_RAMFUNC uint32_t sat_add(uint32_t a, uint32_t b)
{
    return a > UINT32_MAX - b ? UINT32_MAX : a + b;
}

void ocb_core_timing_reset(ocb_core_timing_stats *t)
{
    memset(t, 0, sizeof *t);
}

OCB_CORE_RAMFUNC void ocb_core_timing_note(ocb_core_timing_stats *t, uint32_t late_us)
{
    unsigned b;
    if (late_us < OCB_CORE_LATE_EDGE_0) {
        b = 0u;
    } else if (late_us < OCB_CORE_LATE_EDGE_1) {
        b = 1u;
    } else if (late_us < OCB_CORE_LATE_EDGE_2) {
        b = 2u;
    } else if (late_us < OCB_CORE_LATE_EDGE_3) {
        b = 3u;
    } else if (late_us < OCB_CORE_LATE_EDGE_4) {
        b = 4u;
    } else {
        b = 5u;
    }
    t->hist[b] = sat_add(t->hist[b], 1u);
    t->n = sat_add(t->n, 1u);
    t->late_sum_us = sat_add(t->late_sum_us, late_us);
    ocb_core_max_u32(&t->late_max_us, late_us);
}

OCB_CORE_RAMFUNC uint32_t ocb_core_span_us(uint32_t start_us, uint32_t end_us)
{
    return end_us - start_us; /* unsigned: modulo 2^32 */
}

OCB_CORE_RAMFUNC void ocb_core_max_u32(uint32_t *max, uint32_t v)
{
    if (v > *max) {
        *max = v;
    }
}
