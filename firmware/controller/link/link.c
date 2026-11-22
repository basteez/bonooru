/*
 * SPDX-License-Identifier: MIT
 *
 * Controller link: RX error policy, admission, session, go-back-N front,
 * NAK budget, heartbeat store, USB-ready polling (story 2.6 D4 to D11).
 * Frame bodies are read only through the ocb_proto decoders.
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
#include "ocb_link/rxring.h"
#include "ocb_link/txgate.h"
#include "ocb_proto/consts.h"
#include "ocb_proto/err.h"
#include "ocb_proto/frame.h"
#include "ocb_proto/msgs.h"

void ocb_link_init(ocb_link *link, ocb_core *core, const ocb_usb_mailbox *mailbox, const ocb_link_cfg *cfg,
                   uint64_t now_us)
{
    memset(link, 0, sizeof *link);
    link->core = core;
    link->mailbox = mailbox;
    link->cfg = *cfg;
    link->report_period_us = 0u;
    ocb_rxring_init(&link->ring);
    ocb_rx_init(&link->rx, OCB_RX_CONTROLLER);
    ocb_ready_init(&link->ready, mailbox, now_us);
    ocb_txgate_init(&link->gate);
    link->session_open = false;
    link->link_ok = false;
    link->cache_valid = false;
    link->nak_spent = false;
    link->status_sent = false;
    link->have_app_frame = false;
}

void ocb_link_set_report_period(ocb_link *link, uint16_t report_period_us)
{
    link->report_period_us = report_period_us;
    ocb_core_set_report_period(link->core, report_period_us);
}

ocb_rxring *ocb_link_rx_ring(ocb_link *link)
{
    return &link->ring;
}

ocb_txgate *ocb_link_gate(ocb_link *link)
{
    return &link->gate;
}

OCB_CORE_RAMFUNC bool ocb_link_console_ready(const ocb_link *link, uint64_t now_us)
{
    return ocb_ready_is(&link->ready, now_us);
}

static OCB_CORE_RAMFUNC ocb_core_env make_env(const ocb_link *link, uint64_t now_us)
{
    ocb_core_env e;
    e.now_us = now_us;
    e.ready = ocb_ready_is(&link->ready, now_us);
    return e;
}

/* Transport NAK (CRC, SEQ, FULL) through the shared budget (D6): at most
 * one per value of `expected`. */
static OCB_CORE_RAMFUNC void q_transport_nak(ocb_link *link, uint8_t reason, uint8_t got)
{
    const uint8_t e = link->core->expected;
    if (link->nak_spent && link->nak_expected == e) {
        link->cnt.nak_suppressed++;
        return;
    }
    link->nak_spent = true;
    link->nak_expected = e;
    ocb_link_q_nak_(link, reason, e, got);
}

/* A new value of `expected` re-arms the budget. */
static OCB_CORE_RAMFUNC void expected_moved(ocb_link *link, uint8_t before)
{
    if (link->core->expected != before) {
        link->nak_spent = false;
    }
}

static OCB_CORE_RAMFUNC void reset_app_gap(ocb_link *link)
{
    link->max_app_gap_us = 0u;
}

static OCB_CORE_RAMFUNC void note_app_frame(ocb_link *link, uint64_t now_us)
{
    link->cnt.app_frames++;
    if (link->have_app_frame && now_us >= link->last_app_frame_us) {
        const uint64_t gap = now_us - link->last_app_frame_us;
        const uint32_t g = gap > (uint64_t)UINT32_MAX ? UINT32_MAX : (uint32_t)gap;
        if (g > link->max_app_gap_us) {
            link->max_app_gap_us = g;
        }
    }
    link->have_app_frame = true;
    link->last_app_frame_us = now_us;
}

/* Sync point bookkeeping: cache emptied, budget re-armed (D5, D6). */
static OCB_CORE_RAMFUNC void after_sync_point(ocb_link *link)
{
    link->cache_valid = false;
    link->nak_spent = false;
}

static OCB_CORE_RAMFUNC void handle_bridge(ocb_link *link, const ocb_frame *f, uint64_t now_us)
{
    if (f->type != OCB_MSG_HEARTBEAT) {
        link->cnt.bridge_other++;
        return;
    }
    ocb_msg_heartbeat hb;
    if (ocb_msg_heartbeat_decode(f->body, f->len, &hb) != OCB_OK) {
        link->cnt.heartbeat_bad++;
        return;
    }
    link->cnt.heartbeats++;
    memcpy(link->bridge_ver, hb.bridge_fw_ver, sizeof link->bridge_ver);
    link->hb_flags = hb.flags;
    link->last_heartbeat_us = now_us;
    link->link_ok = true; /* cleared by the Story 2.7 watchdog only */
}

static OCB_CORE_RAMFUNC void handle_hello(ocb_link *link, const ocb_frame *f, const ocb_core_env *env)
{
    ocb_msg_hello m;
    ocb_core_out out;
    const bool match =
        ocb_msg_hello_decode(f->body, f->len, &m) == OCB_OK && m.proto_ver == (uint8_t)OCB_PROTO_VERSION;
    if (match) {
        ocb_core_frame(link->core, env, f, &out);
        link->session_open = true;
    } else {
        /* D8: fail safe. The core goes neutral through a synthetic STOP with
         * the HELLO's seq (expected stays in step); its ACK is discarded. */
        ocb_frame stop;
        stop.type = OCB_MSG_STOP;
        stop.seq = f->seq;
        stop.len = 0u;
        ocb_core_frame(link->core, env, &stop, &out);
        link->session_open = false;
    }
    after_sync_point(link);
    reset_app_gap(link);
    ocb_link_q_info_(link);
}

/* seq == expected: the core decides (D7 for the NAK fields). */
static OCB_CORE_RAMFUNC void handle_in_order(ocb_link *link, const ocb_frame *f, const ocb_core_env *env)
{
    const uint8_t before = link->core->expected;
    ocb_core_out out;
    ocb_core_frame(link->core, env, f, &out);
    if (out.kind == OCB_CORE_REPLY_ACK) {
        ocb_link_q_ack_(link, f->seq);
        link->cache_valid = true;
        link->cache_seq = f->seq;
        link->cache_nak = 0u;
    } else if (out.kind == OCB_CORE_REPLY_NAK) {
        if (OCB_NAK_IS_TRANSPORT(out.nak_reason)) {
            q_transport_nak(link, out.nak_reason, f->seq);
        } else {
            ocb_link_q_nak_(link, out.nak_reason, link->core->expected, f->seq);
            link->cache_valid = true;
            link->cache_seq = f->seq;
            link->cache_nak = out.nak_reason;
        }
    }
    ocb_link_q_event_(link, out.event);
    expected_moved(link, before);
}

static OCB_CORE_RAMFUNC void handle_frame(ocb_link *link, uint64_t now_us)
{
    const ocb_frame *f = &link->frame;
    if (f->type >= (uint8_t)OCB_TYPE_BRIDGE_MIN && f->type <= (uint8_t)OCB_TYPE_BRIDGE_MAX) {
        handle_bridge(link, f, now_us);
        return;
    }
    if (f->type < (uint8_t)OCB_TYPE_APP_MIN || f->type > (uint8_t)OCB_TYPE_APP_MAX) {
        return; /* the controller-side splitter never delivers these */
    }
    note_app_frame(link, now_us);
    const ocb_core_env env = make_env(link, now_us);

    if (f->type == OCB_MSG_STOP) {
        ocb_core_out out;
        ocb_core_frame(link->core, &env, f, &out);
        after_sync_point(link);
        ocb_link_q_ack_(link, f->seq);
        return;
    }
    if (f->type == OCB_MSG_HELLO) {
        handle_hello(link, f, &env);
        return;
    }
    if (!link->session_open) {
        /* D5: the session gate comes before the seq check. */
        const uint8_t before = link->core->expected;
        ocb_core_advance_expected(link->core, f->seq);
        ocb_link_q_nak_(link, OCB_NAK_VERSION, link->core->expected, f->seq);
        expected_moved(link, before);
        return;
    }
    const uint8_t expected = link->core->expected;
    const uint8_t d = (uint8_t)(expected - f->seq);
    if (d == 0u) {
        handle_in_order(link, f, &env);
    } else if (d == 1u) {
        if (link->cache_valid && link->cache_seq == f->seq && link->cache_nak != 0u) {
            ocb_link_q_nak_(link, link->cache_nak, expected, f->seq);
        } else {
            ocb_link_q_ack_(link, (uint8_t)(expected - 1u));
        }
    } else if (d <= (uint8_t)OCB_ACK_WINDOW) {
        ocb_link_q_ack_(link, (uint8_t)(expected - 1u));
    } else {
        q_transport_nak(link, OCB_NAK_SEQ, f->seq);
    }
}

/* D4: an error entry or a ring gap ends the run in progress as a dropped
 * frame, so the next bytes start a fresh run even without a leading 0x00.
 * The splitter's run is reset directly (its fields are public): feeding it
 * a literal 0x00 would decode a complete but undelimited run as a frame,
 * and the zero byte a break loads must never act as a delimiter (D19). */
static OCB_CORE_RAMFUNC void abandon_run(ocb_link *link)
{
    if (link->rx.len == 0u && !link->rx.discarding) {
        return;
    }
    link->rx.len = 0u;
    link->rx.discarding = false;
    link->cnt.rx_err_runs++;
    q_transport_nak(link, OCB_NAK_CRC, 0u);
}

OCB_CORE_RAMFUNC void ocb_link_rx_entry(ocb_link *link, uint16_t entry, uint64_t now_us)
{
    if ((entry & OCB_LINK_RX_OVERFLOW_MARK) != 0u) {
        link->cnt.rx_overflow++;
        abandon_run(link);
        return;
    }
    if ((entry & OCB_LINK_RX_ERR_MASK) != 0u) {
        link->cnt.rx_err_bytes++;
        if ((entry & OCB_LINK_RX_BE) != 0u) {
            link->cnt.breaks++;
        }
        abandon_run(link);
        return;
    }
    const uint8_t byte = (uint8_t)(entry & OCB_LINK_RX_DATA_MASK);
    link->cnt.rx_bytes++;
    const bool was_discarding = link->rx.discarding;
    const size_t len_before = link->rx.len;
    const ocb_rx_result r = ocb_rx_push(&link->rx, byte, &link->frame);
    if (byte != OCB_FRAME_DELIMITER && link->rx.discarding) {
        /* Overlong: the buffered bytes and the first excess byte at once,
         * then every byte up to the delimiter. */
        link->cnt.overlong_bytes += was_discarding ? 1u : (uint32_t)len_before + 1u;
    }
    if (r == OCB_RX_FRAME) {
        handle_frame(link, now_us);
    } else if (r == OCB_RX_DROPPED) {
        const ocb_err e = link->rx.last_err;
        if (e == OCB_ERR_COBS || e == OCB_ERR_CRC || e == OCB_ERR_OVERLONG) {
            q_transport_nak(link, OCB_NAK_CRC, 0u);
        }
        /* EMPTY and RANGE: counted by the splitter, no reply (AD-3). */
    }
}

OCB_CORE_RAMFUNC uint32_t ocb_link_rx_drain(ocb_link *link, uint64_t now_us, uint32_t max)
{
    uint32_t n = 0u;
    uint16_t entry;
    while (n < max && ocb_rxring_pop(&link->ring, &entry)) {
        ocb_link_rx_entry(link, entry, now_us);
        n++;
    }
    return n;
}

OCB_CORE_RAMFUNC void ocb_link_poll(ocb_link *link, uint64_t now_us)
{
    ocb_core_out out;
    const uint8_t down = ocb_ready_poll(&link->ready, link->mailbox, now_us);
    if ((down & OCB_READY_DOWN_SUSPEND) != 0u) {
        ocb_core_abort(link->core, OCB_ABORT_USB_SUSPEND, &out);
        ocb_link_q_event_(link, out.event);
    }
    if ((down & OCB_READY_DOWN_UNMOUNT) != 0u) {
        ocb_core_abort(link->core, OCB_ABORT_USB_UNMOUNT, &out);
        ocb_link_q_event_(link, out.event);
    }
    if ((down & OCB_READY_DOWN_BUS_RESET) != 0u) {
        ocb_core_abort(link->core, OCB_ABORT_USB_BUS_RESET, &out);
        ocb_link_q_event_(link, out.event);
    }
    const ocb_core_env env = make_env(link, now_us);
    ocb_core_tick(link->core, &env, &out);
    ocb_link_q_event_(link, out.event);
}

uint32_t ocb_link_breaks(const ocb_link *link)
{
    return link->cnt.breaks;
}

uint64_t ocb_link_last_heartbeat_us(const ocb_link *link)
{
    return link->last_heartbeat_us;
}

bool ocb_link_link_ok(const ocb_link *link)
{
    return link->link_ok;
}

void ocb_link_set_link_ok(ocb_link *link, bool ok)
{
    link->link_ok = ok;
}

void ocb_link_queue_event(ocb_link *link, uint8_t code)
{
    ocb_link_q_event_(link, code);
}

bool ocb_link_session_open(const ocb_link *link)
{
    return link->session_open;
}

const ocb_link_counters *ocb_link_counters_get(const ocb_link *link)
{
    return &link->cnt;
}

uint32_t ocb_link_dropped_bytes(const ocb_link *link)
{
    return link->cnt.rx_err_bytes + link->ring.dropped + link->cnt.overlong_bytes;
}

uint16_t ocb_link_max_app_gap_ms(const ocb_link *link)
{
    const uint32_t ms = link->max_app_gap_us / 1000u;
    return ms > 0xFFFFu ? (uint16_t)0xFFFFu : (uint16_t)ms;
}
