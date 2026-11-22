/*
 * SPDX-License-Identifier: MIT
 *
 * Published-state handoff from core 1 to core 0 (story 2.3 D35, debug
 * builds only until Story 2.5 owns it): a sequence lock with one writer
 * (core 1, after each link pass) and one reader (core 0, the USB report
 * pump). The writer makes `seq` odd, copies the 8 state bytes, makes it
 * even again; the reader copies between two equal even reads of `seq`,
 * tries 3 times and falls back to NEUTRAL, so a torn state is never
 * reported (AD-11). Cortex-M0+ has no caches and does not reorder stores;
 * every field is volatile, and OCB_LINK_BARRIER() (a compiler barrier on
 * the host, a build may define a DMB) separates the counter from the data.
 * Story 2.5 replaces or keeps this as the real seqlock.
 */
#ifndef OCB_LINK_PUBSTATE_H
#define OCB_LINK_PUBSTATE_H

#include <stdbool.h>
#include <stdint.h>

#include "ocb_proto/state.h"

#define OCB_PUBSTATE_TRIES 3u

typedef struct {
    volatile uint32_t seq;   /* even: stable; odd: a write is in progress */
    volatile uint16_t buttons;
    volatile uint8_t b[6];   /* hat, lx, ly, rx, ry, flags */
} ocb_pubstate;

/* NEUTRAL, seq 0. Before either core uses it. */
void ocb_pubstate_init(ocb_pubstate *ps);

/* The single writer (core 1). */
void ocb_pubstate_write(ocb_pubstate *ps, const ocb_state *s);

/* The single reader (core 0). Returns false and writes NEUTRAL into *out
 * when no consistent copy was obtained in OCB_PUBSTATE_TRIES tries. */
bool ocb_pubstate_read(const ocb_pubstate *ps, ocb_state *out);

/* Test hooks (one thread can model the other core): the first and second
 * half of a write. */
void ocb_pubstate_write_begin(ocb_pubstate *ps);
void ocb_pubstate_write_end(ocb_pubstate *ps);

#endif /* OCB_LINK_PUBSTATE_H */
