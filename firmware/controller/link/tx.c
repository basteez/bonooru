/*
 * SPDX-License-Identifier: MIT
 *
 * Controller link TX (AD-19, story 2.6 D12, D15): slots instead of a byte
 * FIFO, one whole frame per call by priority EVENT > NAK > INFO > ACK >
 * STATUS, STATUS cadence and body, the closed-gate discard and the
 * EVENT-only output of a pending TX release (story 2.7 D6). Frames are
 * built with the ocb_proto encoders only.
 */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include "ocb_core/core.h"
#include "ocb_link/config.h"
#include "ocb_link/internal.h"
#include "ocb_link/link.h"
#include "ocb_link/ready.h"
#include "ocb_link/txgate.h"
#include "ocb_proto/consts.h"
#include "ocb_proto/frame.h"
#include "ocb_proto/msgs.h"

/* Controller -> app bodies are small (INFO, 14 bytes, is the largest of
 * this path; the debug STATUS with its counters is built by
 * ocb_link_tx_status_debug_ in tx_debug.c, with its own buffers, so the
 * release stack of this path does not grow with the debug tail, story 2.3
 * D14): keep the stack use of the TX path low. */
#define TX_BODY_MAX 32u
#define TX_WIRE_MAX 48u /* header 3 + body 32 + crc 2, COBS overhead 1, delimiter 1 */
#define STATUS_PERIOD_US ((uint64_t)OCB_STATUS_PERIOD_MS * 1000u)
#define STATUS_MIN_GAP_US ((uint64_t)OCB_STATUS_MIN_GAP_MS * 1000u)

typedef char ocb_link_tx_body_fits[(OCB_BODY_LEN_STATUS <= TX_BODY_MAX && OCB_BODY_LEN_INFO <= TX_BODY_MAX &&
                                    OCB_FRAME_HEADER_SIZE + TX_BODY_MAX + OCB_FRAME_CRC_SIZE + 2u <= TX_WIRE_MAX)
                                       ? 1
                                       : -1];

OCB_CORE_RAMFUNC void ocb_link_q_event_(ocb_link *link, uint8_t code)
{
    if (code == OCB_EVT_NONE) {
        return;
    }
    if (link->ev_count >= (uint8_t)OCB_LINK_TX_EVENT_CAP) {
        link->cnt.tx_dropped++;
        return;
    }
    const uint8_t i = (uint8_t)((link->ev_head + link->ev_count) % (uint8_t)OCB_LINK_TX_EVENT_CAP);
    link->ev_buf[i] = code;
    link->ev_count++;
}

OCB_CORE_RAMFUNC bool ocb_link_q_nak_(ocb_link *link, uint8_t reason, uint8_t expected, uint8_t got)
{
    if (link->nak_count >= (uint8_t)OCB_LINK_TX_NAK_CAP) {
        link->cnt.tx_dropped++;
        return false;
    }
    const uint8_t i = (uint8_t)((link->nak_head + link->nak_count) % (uint8_t)OCB_LINK_TX_NAK_CAP);
    link->nak_buf[i].reason = reason;
    link->nak_buf[i].expected = expected;
    link->nak_buf[i].got = got;
    link->nak_count++;
    return true;
}

OCB_CORE_RAMFUNC void ocb_link_q_ack_(ocb_link *link, uint8_t seq)
{
    link->ack_pending = true;
    link->ack_seq = seq;
}

OCB_CORE_RAMFUNC void ocb_link_q_info_(ocb_link *link)
{
    link->info_pending = true;
}

static OCB_CORE_RAMFUNC bool status_due(const ocb_link *link, uint64_t now_us)
{
    if (!link->status_sent) {
        return true;
    }
    if (now_us < link->last_status_us) {
        return false;
    }
    const uint64_t since = now_us - link->last_status_us;
    if (since >= STATUS_PERIOD_US) {
        return true;
    }
    return link->status_requested && since >= STATUS_MIN_GAP_US;
}

static OCB_CORE_RAMFUNC int build_status(const ocb_link *link, uint64_t now_us, const uint8_t *extra, size_t extra_len,
                                         uint8_t *body, size_t cap)
{
    ocb_core_status_view v;
    ocb_core_status(link->core, now_us, &v);
    uint8_t flags = 0u;
    if (link->link_ok) {
        flags |= (uint8_t)OCB_STATUS_LINK_OK;
    }
    if (ocb_ready_is(&link->ready, now_us)) {
        flags |= (uint8_t)OCB_STATUS_CONSOLE_READY;
    }
    ocb_msg_status m;
    m.free = v.free;
    m.now_us = v.now_us;
    m.last_seq = v.last_seq;
    m.mode = v.mode;
    m.abort_count = v.abort_count;
    m.last_event = v.last_event;
    m.flags = (uint8_t)(flags & (uint8_t)~OCB_STATUS_RESERVED_MASK);
    m.extra = extra_len > 0u ? extra : NULL;
    m.extra_len = extra_len;
    return ocb_msg_status_encode(&m, body, cap);
}

/* The same body for tx_debug.c (build_status stays static, so the release
 * path below inlines it as before story 2.3). */
OCB_CORE_RAMFUNC int ocb_link_status_body_(const ocb_link *link, uint64_t now_us, const uint8_t *extra,
                                           size_t extra_len, uint8_t *body, size_t cap)
{
    return build_status(link, now_us, extra, extra_len, body, cap);
}

OCB_CORE_RAMFUNC void ocb_link_status_sent_(ocb_link *link, uint64_t now_us)
{
    link->last_status_us = now_us;
    link->status_sent = true;
    link->status_requested = false;
    link->tx_seq = (uint8_t)(link->tx_seq + 1u);
    link->cnt.tx_frames++;
}

static OCB_CORE_RAMFUNC int build_info(const ocb_link *link, uint8_t *body, size_t cap)
{
    ocb_msg_info m;
    m.proto_ver = (uint8_t)OCB_PROTO_VERSION;
    memcpy(m.fw_ver, link->cfg.fw_ver, sizeof m.fw_ver);
    memcpy(m.bridge_ver, link->bridge_ver, sizeof m.bridge_ver);
    m.hid_profile = link->cfg.hid_profile;
    m.queue_cap = link->core->queue.cap;
    m.app_timeout_ms = link->cfg.app_timeout_ms;
    m.report_period_us = link->report_period_us;
    return ocb_msg_info_encode(&m, body, cap);
}

/* Discards everything but the EVENT ring: a pending TX release (story 2.7
 * D6) lets only EVENTs leave, and the closed gate below discards the rest.
 * A discarded transport NAK never reached the app, so the budget for the
 * current `expected` is re-armed (D27). */
static OCB_CORE_RAMFUNC void discard_non_event(ocb_link *link)
{
    if (link->nak_count > 0u) {
        link->nak_spent = false;
    }
    link->cnt.tx_dropped +=
        (uint32_t)link->nak_count + (link->info_pending ? 1u : 0u) + (link->ack_pending ? 1u : 0u);
    link->nak_count = 0u;
    link->nak_head = 0u;
    link->info_pending = false;
    link->ack_pending = false;
    link->status_requested = false;
}

/* Closed gate (D15): nothing leaves, and nothing stale may leave later. */
static OCB_CORE_RAMFUNC void discard_pending(ocb_link *link)
{
    discard_non_event(link);
    link->cnt.tx_dropped += (uint32_t)link->ev_count;
    link->ev_count = 0u;
    link->ev_head = 0u;
}

typedef enum { TX_NONE, TX_EVENT, TX_NAK, TX_INFO, TX_ACK, TX_STATUS } tx_kind;

OCB_CORE_RAMFUNC size_t ocb_link_tx_next(ocb_link *link, uint64_t now_us, uint8_t *out, size_t cap)
{
    if (!ocb_txgate_is_open(&link->gate)) {
        discard_pending(link);
        return 0u;
    }
    if (link->release_pending) {
        /* Story 2.7 D6: the LINK_LOST EVENT (and any older EVENT) leaves
         * before the release; nothing else is sent any more. */
        discard_non_event(link);
        if (link->ev_count == 0u) {
            return 0u;
        }
    }
    uint8_t body[TX_BODY_MAX];
    int n;
    uint8_t type;
    tx_kind kind;
    if (link->ev_count > 0u) {
        const ocb_msg_event m = {link->ev_buf[link->ev_head]};
        n = ocb_msg_event_encode(&m, body, sizeof body);
        type = OCB_MSG_EVENT;
        kind = TX_EVENT;
    } else if (link->nak_count > 0u) {
        n = ocb_msg_nak_encode(&link->nak_buf[link->nak_head], body, sizeof body);
        type = OCB_MSG_NAK;
        kind = TX_NAK;
    } else if (link->info_pending) {
        n = build_info(link, body, sizeof body);
        type = OCB_MSG_INFO;
        kind = TX_INFO;
    } else if (link->ack_pending) {
        const ocb_msg_ack m = {link->ack_seq};
        n = ocb_msg_ack_encode(&m, body, sizeof body);
        type = OCB_MSG_ACK;
        kind = TX_ACK;
    } else if (status_due(link, now_us)) {
        if (link->cfg.debug_counters) {
            /* Debug counters and the P2 log window (story 2.3 D14). */
            return ocb_link_tx_status_debug_(link, now_us, out, cap);
        }
        n = build_status(link, now_us, NULL, 0u, body, sizeof body);
        type = OCB_MSG_STATUS;
        kind = TX_STATUS;
    } else {
        return 0u;
    }
    if (n < 0) {
        return 0u; /* unreachable: every body fits TX_BODY_MAX */
    }
    uint8_t wire[TX_WIRE_MAX];
    const size_t len = ocb_frame_encode(type, link->tx_seq, body, (size_t)n, wire, sizeof wire);
    if (len == 0u || len > cap) {
        return 0u; /* stays pending; never split */
    }
    memcpy(out, wire, len);

    switch (kind) {
    case TX_EVENT:
        link->ev_head = (uint8_t)((link->ev_head + 1u) % (uint8_t)OCB_LINK_TX_EVENT_CAP);
        link->ev_count--;
        break;
    case TX_NAK:
        link->nak_head = (uint8_t)((link->nak_head + 1u) % (uint8_t)OCB_LINK_TX_NAK_CAP);
        link->nak_count--;
        break;
    case TX_INFO:
        link->info_pending = false;
        break;
    case TX_ACK:
        link->ack_pending = false;
        link->status_requested = true; /* STATUS after ACKs (AD-19) */
        break;
    case TX_STATUS:
    case TX_NONE:
    default:
        ocb_link_status_sent_(link, now_us);
        return len;
    }
    link->tx_seq = (uint8_t)(link->tx_seq + 1u);
    link->cnt.tx_frames++;
    return len;
}
