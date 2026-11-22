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
 *
 * Bench H.15 attempt 3 (press 4) showed one report of NEUTRAL in the middle
 * of a held press: core 1 rewrote the state on every link pass, so core 0
 * could find a write in progress 3 times in a row and fell back to
 * NEUTRAL. Since then the writer publishes only when the core published a
 * new state (ocb_pubstate_publish, keyed on core->publish_count), and a
 * reader that gets no consistent copy keeps the last good state it read
 * (never a torn one, at most one report old) instead of NEUTRAL.
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
    uint32_t written_count;  /* writer only: the publish_count last written */
    bool written;            /* writer only: written_count is valid */
} ocb_pubstate;

/* NEUTRAL, seq 0. Before either core uses it. */
void ocb_pubstate_init(ocb_pubstate *ps);

/* The single writer (core 1): writes unconditionally. */
void ocb_pubstate_write(ocb_pubstate *ps, const ocb_state *s);

/* The single writer (core 1), once per pass: writes only when
 * publish_count (the core's counter, incremented at every publish) differs
 * from the one last written. Returns true when it wrote. */
bool ocb_pubstate_publish(ocb_pubstate *ps, const ocb_state *s, uint32_t publish_count);

/* The single reader (core 0). Returns true with a consistent copy in *out,
 * or false with *out unchanged (the caller's last good state) when no
 * consistent copy was obtained in OCB_PUBSTATE_TRIES tries. */
bool ocb_pubstate_read(const ocb_pubstate *ps, ocb_state *out);

/* Test hooks (one thread can model the other core): the first and second
 * half of a write. */
void ocb_pubstate_write_begin(ocb_pubstate *ps);
void ocb_pubstate_write_end(ocb_pubstate *ps);

#endif /* OCB_LINK_PUBSTATE_H */
