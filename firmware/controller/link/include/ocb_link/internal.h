/*
 * SPDX-License-Identifier: MIT
 *
 * Internal helpers shared by link.c and tx.c. Not part of the API.
 */
#ifndef OCB_LINK_INTERNAL_H
#define OCB_LINK_INTERNAL_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "ocb_core/ramfunc.h"
#include "ocb_link/link.h"

/* EVENT ring; a full ring drops the new EVENT (tx_dropped). NONE is ignored. */
OCB_CORE_RAMFUNC void ocb_link_q_event_(ocb_link *link, uint8_t code);
/* NAK ring; a full ring drops the new NAK (tx_dropped) and returns false. */
OCB_CORE_RAMFUNC bool ocb_link_q_nak_(ocb_link *link, uint8_t reason, uint8_t expected, uint8_t got);
/* Cumulative ACK slot: the newer seq replaces the older one. */
OCB_CORE_RAMFUNC void ocb_link_q_ack_(ocb_link *link, uint8_t seq);
/* INFO slot (built when sent). */
OCB_CORE_RAMFUNC void ocb_link_q_info_(ocb_link *link);
/* A STATUS body with `extra_len` debug bytes after the 11 base bytes. */
OCB_CORE_RAMFUNC int ocb_link_status_body_(const ocb_link *link, uint64_t now_us, const uint8_t *extra,
                                           size_t extra_len, uint8_t *body, size_t cap);
/* Bookkeeping of a STATUS that left (cadence, seq, frame count). */
OCB_CORE_RAMFUNC void ocb_link_status_sent_(ocb_link *link, uint64_t now_us);
/* The debug STATUS (debug counters v1, v2 and the v3 P2 log window, story
 * 2.3 D14) as one wire frame, with its own buffers (tx_debug.c), so the
 * release TX path keeps its stack. 0 when it does not fit cap. */
OCB_CORE_RAMFUNC size_t ocb_link_tx_status_debug_(ocb_link *link, uint64_t now_us, uint8_t *out, size_t cap);

#endif /* OCB_LINK_INTERNAL_H */
