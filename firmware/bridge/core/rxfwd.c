/*
 * SPDX-License-Identifier: MIT
 *
 * UART -> PC forwarder (AD-4, Story 3.1). See ocb_bridge/rxfwd.h.
 */
#include "ocb_bridge/rxfwd.h"

#include <string.h>

#define RXFWD_MASK (OCB_BRIDGE_RXFWD_CAP - 1u)

typedef char ocb_ct_rxfwd_pow2[((OCB_BRIDGE_RXFWD_CAP & RXFWD_MASK) == 0u) ? 1 : -1];

void ocb_bridge_rxfwd_init(ocb_bridge_rxfwd *r)
{
    memset(r, 0, sizeof *r);
}

int ocb_bridge_rxfwd_push(ocb_bridge_rxfwd *r, uint16_t dr, uint8_t *byte)
{
    if ((dr & OCB_BRIDGE_DR_BE) != 0u) {
        r->stats.breaks++;
        return -1;
    }
    if ((dr & (OCB_BRIDGE_DR_FE | OCB_BRIDGE_DR_PE | OCB_BRIDGE_DR_OE)) != 0u) {
        r->stats.err_bytes++;
        return 0;
    }
    const uint8_t b = (uint8_t)(dr & 0xFFu);
    *byte = b;
    if (r->head - r->tail >= OCB_BRIDGE_RXFWD_CAP) {
        r->stats.ring_drops++;
        return 1;
    }
    r->buf[r->head & RXFWD_MASK] = b;
    r->head++;
    return 1;
}

size_t ocb_bridge_rxfwd_take(ocb_bridge_rxfwd *r, uint8_t *out, size_t space)
{
    size_t n = 0;
    while (r->tail != r->head && n < space) {
        out[n++] = r->buf[r->tail & RXFWD_MASK];
        r->tail++;
    }
    const uint32_t left = r->head - r->tail;
    r->stats.cdc_drops += left;
    r->tail = r->head;
    r->stats.forwarded += (uint32_t)n;
    return n;
}
