/*
 * SPDX-License-Identifier: MIT
 *
 * Internal helpers shared by link.c and tx.c. Not part of the API.
 */
#ifndef OCB_LINK_INTERNAL_H
#define OCB_LINK_INTERNAL_H

#include <stdbool.h>
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

#endif /* OCB_LINK_INTERNAL_H */
