/*
 * SPDX-License-Identifier: MIT
 *
 * The bridge's per-pass logic (Story 3.1). See ocb_bridge/bridge.h.
 */
#include "ocb_bridge/bridge.h"

#include <string.h>

#include "ocb_bridge/config.h"
#include "ocb_bridge/heartbeat.h"

void ocb_bridge_init(ocb_bridge *b, const uint8_t ver[3])
{
    memset(b, 0, sizeof *b);
    ocb_bridge_framer_init(&b->framer);
    ocb_bridge_probe_init(&b->probe);
    ocb_bridge_rxfwd_init(&b->rx);
    ocb_rx_init(&b->rxsplit, OCB_RX_APP);
    memcpy(b->ver, ver, 3);
}

size_t ocb_bridge_cdc_in(ocb_bridge *b, const uint8_t *data, size_t n)
{
    return ocb_bridge_framer_feed(&b->framer, data, n);
}

void ocb_bridge_set_dtr(ocb_bridge *b, bool dtr)
{
    if (b->dtr && !dtr) {
        ocb_bridge_framer_discard_partial(&b->framer);
    }
    b->dtr = dtr;
}

void ocb_bridge_cdc_unmount(ocb_bridge *b)
{
    ocb_bridge_framer_discard_partial(&b->framer);
    b->dtr = false;
}

void ocb_bridge_uart_rx(ocb_bridge *b, uint16_t dr)
{
    uint8_t byte = 0;
    const int r = ocb_bridge_rxfwd_push(&b->rx, dr, &byte);
    if (r < 0) {
        ocb_bridge_probe_break(&b->probe);
        return;
    }
    if (r == 0) {
        return;
    }
    ocb_frame frame;
    if (ocb_rx_push(&b->rxsplit, byte, &frame) == OCB_RX_FRAME) {
        ocb_bridge_probe_frame(&b->probe);
    }
}

size_t ocb_bridge_cdc_out(ocb_bridge *b, uint8_t *out, size_t space)
{
    return ocb_bridge_rxfwd_take(&b->rx, out, space);
}

void ocb_bridge_hb_ticks(ocb_bridge *b, uint32_t ticks)
{
    if (ticks == b->hb_ticks_seen) {
        return;
    }
    b->hb_ticks_seen = ticks;
    if (ocb_bridge_probe_linked(&b->probe)) {
        b->hb_due = true;
    } else {
        b->stats.hb_dropped++;
    }
}

bool ocb_bridge_step(ocb_bridge *b, uint64_t now_us, bool rx_high)
{
    const bool was_linked = ocb_bridge_probe_linked(&b->probe);
    const bool tx = ocb_bridge_probe_step(&b->probe, now_us, rx_high);
    if (was_linked && !ocb_bridge_probe_linked(&b->probe)) {
        if (b->inflight_pos < b->inflight_len) {
            b->stats.inflight_abandoned++;
        }
        b->inflight_len = 0;
        b->inflight_pos = 0;
        b->hb_due = false;
    }
    b->tx_enabled = tx;
    return tx;
}

/* At a frame boundary: a due heartbeat first, else the queued app frame. */
static bool load_next(ocb_bridge *b)
{
    if (b->hb_due) {
        const bool dtr = b->dtr;
        const size_t n = ocb_bridge_hb_build(b->hb_seq, b->ver, dtr, b->inflight, sizeof b->inflight);
        b->hb_due = false;
        if (n == 0u) {
            return false;
        }
        b->hb_seq++;
        b->inflight_len = n;
        b->inflight_pos = 0;
        b->inflight_hb = true;
        b->stats.hb_sent++;
        return true;
    }
    const size_t n = ocb_bridge_framer_take(&b->framer, b->inflight, sizeof b->inflight);
    if (n == 0u) {
        return false;
    }
    b->inflight_len = n;
    b->inflight_pos = 0;
    b->inflight_hb = false;
    b->stats.app_frames++;
    return true;
}

size_t ocb_bridge_uart_out(ocb_bridge *b, uint8_t *out, size_t cap, size_t ring_used)
{
    if (!ocb_bridge_probe_linked(&b->probe) || ring_used >= OCB_BRIDGE_UART_AHEAD_MAX) {
        return 0;
    }
    size_t allowed = (size_t)OCB_BRIDGE_UART_AHEAD_MAX - ring_used;
    if (allowed > cap) {
        allowed = cap;
    }
    size_t n = 0;
    while (n < allowed) {
        if (b->inflight_pos >= b->inflight_len && !load_next(b)) {
            break;
        }
        const size_t left = b->inflight_len - b->inflight_pos;
        size_t k = allowed - n;
        if (k > left) {
            k = left;
        }
        memcpy(&out[n], &b->inflight[b->inflight_pos], k);
        b->inflight_pos += k;
        n += k;
    }
    return n;
}
