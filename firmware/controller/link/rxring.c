/*
 * SPDX-License-Identifier: MIT
 *
 * SPSC RX ring (story 2.6 Task 3.1, D19). Free-running u16 indices; the
 * capacity is a power of two that divides 65536, so `head - tail` is the
 * fill level across the index wrap.
 */
#include <stdbool.h>
#include <stdint.h>

#include "ocb_link/config.h"
#include "ocb_link/rxring.h"

typedef char ocb_rxring_cap_is_pow2[((OCB_LINK_RX_RING_CAP & (OCB_LINK_RX_RING_CAP - 1)) == 0 &&
                                     OCB_LINK_RX_RING_CAP >= 4 && OCB_LINK_RX_RING_CAP <= 32768)
                                        ? 1
                                        : -1];

#define RING_MASK ((uint16_t)(OCB_LINK_RX_RING_CAP - 1))

void ocb_rxring_init(ocb_rxring *r)
{
    for (uint16_t i = 0; i < (uint16_t)OCB_LINK_RX_RING_CAP; i++) {
        r->buf[i] = 0u;
    }
    r->head = 0u;
    r->tail = 0u;
    r->dropped = 0u;
    r->gap_pending = false;
    r->overflowed = false;
}

OCB_CORE_RAMFUNC bool ocb_rxring_push(ocb_rxring *r, uint16_t entry)
{
    uint16_t h = r->head;
    const uint16_t used = (uint16_t)(h - r->tail);
    if (r->gap_pending) {
        /* The marker and the entry go in together, or the entry is lost too. */
        if ((uint32_t)used + 2u > (uint32_t)OCB_LINK_RX_RING_CAP) {
            r->dropped = r->dropped + 1u;
            return false;
        }
        r->buf[h & RING_MASK] = (uint16_t)OCB_LINK_RX_OVERFLOW_MARK;
        h = (uint16_t)(h + 1u);
        r->gap_pending = false;
    } else if (used >= (uint16_t)OCB_LINK_RX_RING_CAP) {
        r->dropped = r->dropped + 1u;
        r->gap_pending = true;
        r->overflowed = true;
        return false;
    }
    r->buf[h & RING_MASK] = entry;
    OCB_LINK_BARRIER();
    r->head = (uint16_t)(h + 1u);
    return true;
}

OCB_CORE_RAMFUNC bool ocb_rxring_pop(ocb_rxring *r, uint16_t *entry)
{
    const uint16_t t = r->tail;
    if (t == r->head) {
        return false;
    }
    *entry = r->buf[t & RING_MASK];
    OCB_LINK_BARRIER();
    r->tail = (uint16_t)(t + 1u);
    return true;
}

OCB_CORE_RAMFUNC uint16_t ocb_rxring_count(const ocb_rxring *r)
{
    return (uint16_t)(r->head - r->tail);
}
