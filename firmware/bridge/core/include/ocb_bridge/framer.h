/*
 * SPDX-License-Identifier: MIT
 *
 * PC -> UART framer (AD-4, Story 3.1): scans the CDC byte stream for 0x00
 * delimiters only, never decodes, and holds at most one complete frame
 * (the "queued" one; the "in flight" one belongs to ocb_bridge). A run
 * longer than OCB_MAX_ENCODED_FRAME is dropped and counted once; a partial
 * run is dropped when DTR falls or CDC unmounts. An empty run (a lone 0x00)
 * is a frame of zero bytes plus its delimiter (Story 3.1 D3).
 */
#ifndef OCB_BRIDGE_FRAMER_H
#define OCB_BRIDGE_FRAMER_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "ocb_proto/consts.h"

typedef struct {
    uint32_t frames;           /* complete runs queued (empty ones included) */
    uint32_t empty;            /* lone 0x00 runs */
    uint32_t overlong;         /* runs over OCB_MAX_ENCODED_FRAME, counted once */
    uint32_t partial_discards; /* partial runs dropped on DTR fall / unmount */
} ocb_bridge_framer_stats;

typedef struct {
    uint8_t run[OCB_MAX_ENCODED_FRAME];
    size_t run_len;
    bool discarding; /* inside an overlong run, until the next 0x00 */
    uint8_t queued[OCB_MAX_WIRE_FRAME];
    size_t queued_len; /* 0: the queued slot is free */
    ocb_bridge_framer_stats stats;
} ocb_bridge_framer;

void ocb_bridge_framer_init(ocb_bridge_framer *f);

/* Consumes bytes until all are used or a run completes while the queued
 * slot is full; returns how many were consumed. The caller keeps the rest
 * and offers them again later (USB backpressure). */
size_t ocb_bridge_framer_feed(ocb_bridge_framer *f, const uint8_t *data, size_t n);

/* DTR fell or CDC unmounted: drop the partial run (a queued complete
 * frame is kept). */
void ocb_bridge_framer_discard_partial(ocb_bridge_framer *f);

/* The queued frame (bytes and its 0x00), if any; take() frees the slot. */
bool ocb_bridge_framer_has_frame(const ocb_bridge_framer *f);
size_t ocb_bridge_framer_take(ocb_bridge_framer *f, uint8_t *out, size_t cap);

#endif /* OCB_BRIDGE_FRAMER_H */
