/*
 * SPDX-License-Identifier: MIT
 *
 * Self-timing of the controller (story 2.5 D11, Task 11): the lateness
 * statistics of the scheduler and the loop-time helper. Pure C99, no
 * clock: the caller passes the numbers. Always compiled (no #ifdef in a
 * pure library, story 2.3 D21); only the debug STATUS reports them.
 *
 * Lateness is `now - due` at the moment ocb_core_tick applies an event (an
 * event later than the LATE threshold is never applied, so it is never
 * counted). Buckets, us: [0,50) [50,100) [100,250) [250,1000) [1000,4000)
 * [4000, inf). Every counter saturates at UINT32_MAX: late_sum_us reaches
 * it after 65537 events at the largest possible lateness (65535 us, the
 * u16 report period), or after about 85 million events at 50 us, far more
 * than a piece holds (the queue cap is 512, a run is one piece).
 */
#ifndef OCB_CORE_TIMING_H
#define OCB_CORE_TIMING_H

#include <stdint.h>

#include "ocb_core/ramfunc.h"

#define OCB_CORE_LATE_BUCKETS 6
/* Upper edges (exclusive) of the first five buckets, us. */
#define OCB_CORE_LATE_EDGE_0 50u
#define OCB_CORE_LATE_EDGE_1 100u
#define OCB_CORE_LATE_EDGE_2 250u
#define OCB_CORE_LATE_EDGE_3 1000u
#define OCB_CORE_LATE_EDGE_4 4000u

typedef struct {
    uint32_t n;           /* events applied in PLAYING this run */
    uint32_t late_max_us; /* the largest lateness at apply */
    uint32_t late_sum_us; /* the sum of the lateness (the host divides by n) */
    uint32_t hist[OCB_CORE_LATE_BUCKETS];
} ocb_core_timing_stats;

/* All zero (at init and at every accepted START). */
void ocb_core_timing_reset(ocb_core_timing_stats *t);

/* One applied event, `late_us` after its due time. No division. */
OCB_CORE_RAMFUNC void ocb_core_timing_note(ocb_core_timing_stats *t, uint32_t late_us);

/* Loop-time helper: the elapsed microseconds between two reads of a
 * free-running 32-bit microsecond counter, wrap-safe (end may have wrapped
 * past start once). */
OCB_CORE_RAMFUNC uint32_t ocb_core_span_us(uint32_t start_us, uint32_t end_us);

/* *max = the larger of *max and v. */
OCB_CORE_RAMFUNC void ocb_core_max_u32(uint32_t *max, uint32_t v);

#endif /* OCB_CORE_TIMING_H */
