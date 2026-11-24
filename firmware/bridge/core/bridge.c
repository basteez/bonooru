/*
 * SPDX-License-Identifier: MIT
 *
 * The bridge's per-pass logic (Story 3.1). See ocb_bridge/bridge.h.
 */
#include "ocb_bridge/bridge.h"

#include <string.h>

#include "ocb_bridge/config.h"
#include "ocb_bridge/heartbeat.h"

/* A due heartbeat waits at most for one maximum frame in flight, the bytes
 * allowed ahead in the UART software ring, the 32-entry FIFO and the shift
 * register (10 bits each at OCB_LINK_BAUD): every gap must stay within
 * OCB_HEARTBEAT_MAX_GAP_MS (AD-4, F.7). */
#define OCB_BRIDGE_HB_WORST_US                                                                     \
    (((uint64_t)OCB_MAX_WIRE_FRAME + OCB_BRIDGE_UART_AHEAD_MAX + 33u) * 10u * 1000000u /           \
         OCB_LINK_BAUD +                                                                           \
     1u)
typedef char ocb_ct_bridge_hb_gap[((uint64_t)OCB_HEARTBEAT_PERIOD_MS * 1000u + OCB_BRIDGE_HB_WORST_US <=
                                   (uint64_t)OCB_HEARTBEAT_MAX_GAP_MS * 1000u)
                                      ? 1
                                      : -1];

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
        b->dtr_fell = true;
    }
    b->dtr = dtr;
}

void ocb_bridge_cut_partial(ocb_bridge *b)
{
    ocb_bridge_framer_discard_partial(&b->framer);
}

void ocb_bridge_cdc_unmount(ocb_bridge *b)
{
    ocb_bridge_framer_discard_partial(&b->framer);
    b->dtr = false;
}

void ocb_bridge_uart_rx(ocb_bridge *b, uint16_t dr)
{
    uint8_t byte = 0;
    ocb_bridge_probe_activity(&b->probe);
    const int r = ocb_bridge_rxfwd_push(&b->rx, dr, &byte);
    if (r <= 0) {
        /* An error character ends the run in progress (as the controller's
         * receiver does): the next valid frame is seen whole. */
        ocb_rx_init(&b->rxsplit, OCB_RX_APP);
        if (r < 0) {
            ocb_bridge_probe_break(&b->probe);
        }
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
    if (!was_linked && ocb_bridge_probe_linked(&b->probe)) {
        /* Whatever the controller's receiver holds from before (a cut
         * frame, probe noise) ends here. */
        b->need_delim = true;
    }
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
    if (b->need_delim) {
        b->need_delim = false;
        b->inflight[0] = OCB_FRAME_DELIMITER;
        b->inflight_len = 1;
        b->inflight_pos = 0;
        b->inflight_hb = false;
        return true;
    }
    if (b->hb_due) {
        const bool dtr = b->dtr && !b->dtr_fell;
        b->dtr_fell = false;
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
