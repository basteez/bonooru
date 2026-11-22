/*
 * SPDX-License-Identifier: MIT
 *
 * Single-producer single-consumer ring of RX entries (story 2.6 Task 3.1).
 * An entry is the PL011 data register value (dr & 0x0FFF): bits 0-7 data,
 * 8 FE, 9 PE, 10 BE, 11 OE. The producer is the UART RX interrupt, the
 * consumer the core 1 loop; both run on core 1, so no atomics are needed:
 * the producer writes the slot before it publishes `head`, the consumer
 * reads the slot before it publishes `tail`. Each index has one writer.
 *
 * Overflow (D19): a push on a full ring drops the new entry and counts it
 * in `dropped`. The next push that finds room first stores the marker
 * OCB_LINK_RX_OVERFLOW_MARK, so the consumer sees the gap at the exact
 * place in the stream where the bytes were lost and treats it like one
 * error byte. `overflowed` is a sticky flag (set by the producer, cleared
 * only by init).
 */
#ifndef OCB_LINK_RXRING_H
#define OCB_LINK_RXRING_H

#include <stdbool.h>
#include <stdint.h>

#include "ocb_core/ramfunc.h"
#include "ocb_link/config.h"

typedef struct {
    volatile uint16_t buf[OCB_LINK_RX_RING_CAP];
    volatile uint16_t head;     /* producer only: next slot to write (free-running) */
    volatile uint16_t tail;     /* consumer only: next slot to read (free-running) */
    volatile uint32_t dropped;  /* producer only: entries lost to a full ring (wraps) */
    volatile bool gap_pending;  /* producer only: a marker is owed before the next entry */
    volatile bool overflowed;   /* producer only: sticky, any overflow since init */
} ocb_rxring;

void ocb_rxring_init(ocb_rxring *r);

/* Producer (interrupt). Returns false when the entry was dropped. */
OCB_CORE_RAMFUNC bool ocb_rxring_push(ocb_rxring *r, uint16_t entry);

/* Consumer. Returns false when empty. */
OCB_CORE_RAMFUNC bool ocb_rxring_pop(ocb_rxring *r, uint16_t *entry);

/* Entries waiting (consumer side view). */
OCB_CORE_RAMFUNC uint16_t ocb_rxring_count(const ocb_rxring *r);

#endif /* OCB_LINK_RXRING_H */
