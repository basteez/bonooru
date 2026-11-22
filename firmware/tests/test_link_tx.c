/*
 * SPDX-License-Identifier: MIT
 *
 * Host test F.16 (story 2.6 Task 6.8): the controller link TX (AD-19,
 * D12, D15). Exact priority EVENT > NAK > INFO > ACK > STATUS; STATUS
 * cadence (first at once, every 50 ms with no drift, after an ACK but
 * never closer than 10 ms, at most one per 10 ms in a prefill burst);
 * STATUS fields in every mode and after every abort cause; reserved flag
 * bits; the 21 debug bytes (v1 prefix and the split of dropped_bytes); slot overflow; a too-small `cap`; the TX gate;
 * two independent links. Every output is decoded as a controller frame.
 */
#include <stdint.h>
#include <stdio.h>

#include "link_fixture.h"

static void send_corrupted(lh *h, uint8_t seq)
{
    uint8_t wire[OCB_MAX_WIRE_FRAME];
    const size_t n = lh_wire(OCB_MSG_PING, seq, NULL, 0u, wire);
    wire[1] = (uint8_t)(wire[1] ^ 0x01u);
    lh_feed(h, wire, n);
}

/* m->extra (debug counters) points into the decoded frame, so the frames
 * must outlive this call: static storage, valid until the next call. */
static bool get_status(lh *h, ocb_msg_status *m)
{
    static lh_out o[16];
    const size_t n = lh_drain(h, o, 16u);
    for (size_t i = 0; i < n && i < 16u; i++) {
        if (lh_status_decode(&o[i], m)) {
            return true;
        }
    }
    return false;
}

static void test_priority(void)
{
    static lh h;
    lh_session(&h, 8u);
    h.now += 50000u; /* STATUS due */
    lh_send_seq(&h, OCB_MSG_HELLO, 200u, (const uint8_t[]){1u}, 1u); /* INFO */
    const uint8_t ping_seq = 201u;
    lh_send_seq(&h, OCB_MSG_PING, ping_seq, NULL, 0u); /* ACK */
    send_corrupted(&h, 202u);                          /* NAK CRC */
    ocb_link_queue_event(&h.link, OCB_EVT_LATE);       /* EVENT */
    lh_out o[8];
    const size_t n = lh_drain(&h, o, 8u);
    const uint8_t want[] = {OCB_MSG_EVENT, OCB_MSG_NAK, OCB_MSG_INFO, OCB_MSG_ACK, OCB_MSG_STATUS};
    OCB_CHECK(n == sizeof want, "priority: %zu frames, wanted 5", n);
    for (size_t i = 0; i < n && i < sizeof want; i++) {
        OCB_CHECK(o[i].type == want[i], "priority: frame %zu is 0x%02x, wanted 0x%02x", i, o[i].type, want[i]);
    }
    OCB_CHECK(n >= 4u && o[3].body[0] == ping_seq, "ACK seq %u", o[3].body[0]);
    /* Rolling tx seq: one counter for every outbound frame. */
    for (size_t i = 1; i < n && i < 8u; i++) {
        OCB_CHECK(o[i].seq == (uint8_t)(o[0].seq + i), "tx seq not rolling at %zu", i);
    }
}

static void test_cadence(void)
{
    static lh h;
    lh_init(&h, 512u);
    lh_open_gate(&h);
    /* The first STATUS is due at once. */
    lh_out o[16];
    size_t n = lh_drain(&h, o, 16u);
    OCB_CHECK(n == 1u && o[0].type == OCB_MSG_STATUS, "first STATUS: %zu frames", n);
    /* Periodic: exactly every 50 ms over 1000 periods (1 ms steps). */
    const uint64_t t0 = h.now;
    uint32_t count = 0;
    bool drift = false;
    for (uint32_t ms = 1; ms <= 50000u; ms++) {
        h.now = t0 + (uint64_t)ms * 1000u;
        n = lh_drain(&h, o, 16u);
        if (n > 0u) {
            count++;
            if (ms % 50u != 0u || n != 1u || o[0].type != OCB_MSG_STATUS) {
                drift = true;
            }
        }
    }
    OCB_CHECK(count == 1000u && !drift, "periodic: %u STATUS, drift %d", count, (int)drift);

    /* ACK-triggered: 10 ms after the previous STATUS at the earliest. */
    lh_make_ready(&h);
    lh_hello(&h);
    h.now += 1000u;
    lh_discard_tx(&h); /* INFO; STATUS not due */
    uint64_t last_status = 0;
    n = 0;
    /* Find the time of the last STATUS: step until one appears. */
    for (int i = 0; i < 60 && n == 0u; i++) {
        h.now += 1000u;
        n = lh_drain(&h, o, 16u);
    }
    OCB_CHECK(n == 1u && o[0].type == OCB_MSG_STATUS, "setup periodic STATUS");
    last_status = h.now;
    h.now += 2000u;
    const uint8_t seq = h.seq;
    lh_ping(&h);
    n = lh_drain(&h, o, 16u);
    OCB_CHECK(n == 1u && o[0].type == OCB_MSG_ACK && o[0].body[0] == seq, "ACK alone at +2 ms (%zu)", n);
    uint64_t status_at = 0;
    for (uint32_t us = 0; us < 60000u && status_at == 0u; us += 100u) {
        h.now += 100u;
        n = lh_drain(&h, o, 16u);
        if (n > 0u && o[0].type == OCB_MSG_STATUS) {
            status_at = h.now;
        }
    }
    OCB_CHECK(status_at == last_status + 10000u, "ACK-triggered STATUS at +%llu us, wanted +10000",
              (unsigned long long)(status_at - last_status));

    /* Prefill burst: 64 frames within 1 ms, TX drained after each frame. */
    lh_session(&h, 512u);
    h.now += 50000u;
    n = lh_drain(&h, o, 16u); /* a periodic STATUS now */
    const uint64_t burst0 = h.now;
    uint32_t acks = 0;
    uint32_t statuses = 0;
    uint64_t last_s = burst0;
    uint64_t min_gap = UINT64_MAX;
    for (uint32_t i = 0; i < 64u; i++) {
        lh_queue_event(&h, (i + 1u) * 1000u, lst_buttons(OCB_BTN_A));
        h.now += 15u;
        lh_out b[8];
        const size_t m = lh_drain(&h, b, 8u);
        for (size_t k = 0; k < m && k < 8u; k++) {
            if (b[k].type == OCB_MSG_ACK) {
                acks++;
            } else if (b[k].type == OCB_MSG_STATUS) {
                statuses++;
            }
        }
    }
    OCB_CHECK(h.now - burst0 < 1000u, "burst longer than 1 ms");
    OCB_CHECK(acks == 64u && statuses == 0u, "burst: %u ACK, %u STATUS (wanted 64, 0)", acks, statuses);
    /* Then STATUS every >= 10 ms with ACK traffic continuing. */
    for (uint32_t i = 0; i < 400u; i++) {
        lh_queue_event(&h, 100000u + i * 1000u, lst_buttons(OCB_BTN_A));
        h.now += 500u;
        lh_out b[8];
        const size_t m = lh_drain(&h, b, 8u);
        for (size_t k = 0; k < m && k < 8u; k++) {
            if (b[k].type == OCB_MSG_STATUS) {
                if (h.now - last_s < min_gap) {
                    min_gap = h.now - last_s;
                }
                last_s = h.now;
            }
        }
    }
    OCB_CHECK(min_gap == 10000u, "STATUS min gap under ACK traffic %llu us", (unsigned long long)min_gap);

    /* Without draining: one cumulative ACK for the whole burst. */
    lh_session(&h, 512u);
    const uint8_t first = h.seq;
    for (uint32_t i = 0; i < 64u; i++) {
        lh_queue_event(&h, (i + 1u) * 1000u, lst_buttons(OCB_BTN_A));
    }
    lh_expect_ack(&h, (uint8_t)(first + 63u), "cumulative ACK");
}

static void expect_status(lh *h, uint16_t free, uint32_t now_us, uint8_t last_seq, uint8_t mode, uint8_t abort_count,
                          uint8_t last_event, uint8_t flags, const char *what)
{
    h->now += 50000u;
    ocb_msg_status m;
    memset(&m, 0, sizeof m);
    const bool ok = get_status(h, &m);
    OCB_CHECK(ok, "%s: no STATUS", what);
    if (!ok) {
        return;
    }
    OCB_CHECK(m.free == free && m.now_us == now_us && m.last_seq == last_seq && m.mode == mode &&
                  m.abort_count == abort_count && m.last_event == last_event && m.flags == flags,
              "%s: STATUS free %u now %u last_seq %u mode %u aborts %u last_event %u flags 0x%02x; wanted %u %u %u %u "
              "%u %u 0x%02x",
              what, m.free, m.now_us, m.last_seq, m.mode, m.abort_count, m.last_event, m.flags, free, now_us, last_seq,
              mode, abort_count, last_event, flags);
    OCB_CHECK((m.flags & OCB_STATUS_RESERVED_MASK) == 0u, "%s: reserved bits set", what);
    OCB_CHECK(m.extra_len == 0u, "%s: debug bytes without the flag", what);
}

static void test_fields(void)
{
    static lh h;
    lh_init(&h, 16u);
    lh_open_gate(&h);
    expect_status(&h, 16u, 0u, 255u, OCB_MODE_IDLE, 0u, OCB_EVT_NONE, 0u, "boot");
    lh_heartbeat(&h, 1u, 2u, 3u, 0u);
    expect_status(&h, 16u, 0u, 255u, OCB_MODE_IDLE, 0u, OCB_EVT_NONE, OCB_STATUS_LINK_OK, "heartbeat");
    lh_make_ready(&h);
    /* Keep IN reports flowing so `ready` holds across the 50 ms steps. */
    const uint8_t both = OCB_STATUS_LINK_OK | OCB_STATUS_CONSOLE_READY;
    lh_hello(&h); /* seq 0 */
    ocb_usb_mailbox_in_report(&h.mb);
    ocb_link_poll(&h.link, h.now);
    expect_status(&h, 16u, 0u, 0u, OCB_MODE_IDLE, 0u, OCB_EVT_NONE, both, "after HELLO");
    ocb_usb_mailbox_in_report(&h.mb);
    ocb_link_poll(&h.link, h.now);
    lh_queue_event(&h, 1000u, lst_buttons(OCB_BTN_A));  /* seq 1 */
    lh_queue_event(&h, 900000u, lst_buttons(OCB_BTN_X)); /* seq 2 */
    lh_queue_event(&h, 950000u, lst_end());              /* seq 3 */
    expect_status(&h, 13u, 0u, 3u, OCB_MODE_IDLE, 0u, OCB_EVT_NONE, both, "IDLE prefilled");
    ocb_usb_mailbox_in_report(&h.mb);
    ocb_link_poll(&h.link, h.now);
    lh_set_state(&h, lst_buttons(OCB_BTN_B)); /* seq 4 */
    expect_status(&h, 13u, 0u, 4u, OCB_MODE_DIRECT, 0u, OCB_EVT_NONE, both, "DIRECT");
    ocb_usb_mailbox_in_report(&h.mb);
    ocb_link_poll(&h.link, h.now);
    lh_start(&h, 100000u); /* seq 5; t0 = now + 100 ms */
    const uint64_t t0 = h.now + 100000u;
    ocb_usb_mailbox_in_report(&h.mb);
    ocb_link_poll(&h.link, h.now);
    expect_status(&h, 13u, 0u, 5u, OCB_MODE_PLAYING, 0u, OCB_EVT_NONE, both, "PLAYING in the lead");
    h.now = t0 + 1000u;
    ocb_usb_mailbox_in_report(&h.mb);
    ocb_link_poll(&h.link, h.now); /* applies t=1000 */
    lh_discard_tx(&h);
    ocb_usb_mailbox_in_report(&h.mb);
    ocb_link_poll(&h.link, h.now + 50000u);
    /* expect_status samples 50 ms later: 51 ms since t0. */
    expect_status(&h, 14u, 51000u, 5u, OCB_MODE_PLAYING, 0u, OCB_EVT_NONE, both, "PLAYING after the lead");

    /* After each abort cause (STOP between them clears the latch). */
    const struct {
        ocb_core_abort_cause cause;
        uint8_t event;
    } causes[] = {
        {OCB_ABORT_LINK_LOST, OCB_EVT_LINK_LOST},         {OCB_ABORT_APP_TIMEOUT, OCB_EVT_NONE},
        {OCB_ABORT_USB_SUSPEND, OCB_EVT_USB_SUSPENDED},   {OCB_ABORT_USB_UNMOUNT, OCB_EVT_USB_SUSPENDED},
        {OCB_ABORT_USB_BUS_RESET, OCB_EVT_USB_SUSPENDED}, {OCB_ABORT_CORE_STALL, OCB_EVT_NONE},
    };
    uint8_t last_event = OCB_EVT_NONE;
    for (size_t i = 0; i < sizeof causes / sizeof causes[0]; i++) {
        ocb_core_out out;
        ocb_core_abort(&h.core, causes[i].cause, &out);
        if (causes[i].event != OCB_EVT_NONE) {
            last_event = causes[i].event;
        }
        ocb_usb_mailbox_in_report(&h.mb);
        ocb_link_poll(&h.link, h.now);
        expect_status(&h, 16u, 0u, (uint8_t)(h.seq - 1u), OCB_MODE_IDLE, (uint8_t)(i + 1u), last_event, both,
                      "after an abort");
        ocb_usb_mailbox_in_report(&h.mb);
        ocb_link_poll(&h.link, h.now);
        lh_stop(&h);
    }
    /* Not ready again: bit1 clears, bit0 stays, reserved bits stay 0. */
    ocb_usb_mailbox_bus_reset(&h.mb);
    ocb_link_poll(&h.link, h.now);
    lh_discard_tx(&h);
    expect_status(&h, 16u, 0u, (uint8_t)(h.seq - 1u), OCB_MODE_IDLE, 7u, OCB_EVT_USB_SUSPENDED,
                  OCB_STATUS_LINK_OK, "bus reset");
}

static void test_debug_counters(void)
{
    static lh h;
    lh_init_cfg(&h, 8u, true);
    lh_open_gate(&h);
    lh_make_ready(&h);
    lh_hello(&h);
    h.now += 1234567u; /* a gap of 1234.567 ms */
    lh_ping(&h);
    h.now += 5000u;
    lh_ping(&h);
    /* 4 error bytes (FE, PE, BE, FE|OE), a 300-byte overlong run, and 8
     * bytes lost to a full RX ring (520 idle bytes pushed, then drained). */
    ocb_link_rx_entry(&h.link, OCB_LINK_RX_FE, h.now);
    ocb_link_rx_entry(&h.link, OCB_LINK_RX_PE, h.now);
    ocb_link_rx_entry(&h.link, OCB_LINK_RX_BE, h.now);
    ocb_link_rx_entry(&h.link, (uint16_t)(OCB_LINK_RX_FE | OCB_LINK_RX_OE | 0x42u), h.now);
    for (int i = 0; i < 300; i++) {
        ocb_link_rx_entry(&h.link, 0x11u, h.now);
    }
    ocb_link_rx_entry(&h.link, 0x00u, h.now);
    for (int i = 0; i < 520; i++) {
        (void)ocb_rxring_push(ocb_link_rx_ring(&h.link), 0x00u);
    }
    (void)ocb_link_rx_drain(&h.link, h.now, 1000u);
    /* 10 EVENTs: 2 dropped. */
    for (int i = 0; i < 10; i++) {
        ocb_link_queue_event(&h.link, OCB_EVT_LATE);
    }
    h.now += 50000u;
    lh_out o[32];
    const size_t n = lh_drain(&h, o, 32u);
    ocb_msg_status m;
    bool found = false;
    for (size_t i = 0; i < n && i < 32u; i++) {
        if (lh_status_decode(&o[i], &m)) {
            found = true;
            break;
        }
    }
    OCB_CHECK(found, "debug: no STATUS");
    if (!found) {
        return;
    }
    OCB_CHECK(m.extra_len == 21u, "debug: %zu extra bytes", m.extra_len);
    if (m.extra_len == 21u) {
        /* By hand: gap 1234 ms, trips 0, dropped 4 + 8 + 300 = 312, tx_dropped
         * 2, then rx_err 4, overflow 8, overlong 300, FE 2, PE 1, BE 1, OE 1. */
        const uint8_t want[21] = {0xD2u, 0x04u, 0u,  0x38u, 0x01u, 2u,  0u, 4u, 0u, 8u, 0u,
                                  0x2Cu, 0x01u, 2u, 0u,    1u,    0u, 1u, 0u, 1u, 0u};
        OCB_CHECK_MEM(m.extra, m.extra_len, want, sizeof want, "debug counters");
    }
    /* Saturation at 65535. */
    h.link.max_app_gap_us = 0xFFFFFFFFu;
    h.link.cnt.tx_dropped = 70000u;
    h.link.cnt.rx_err_bytes = 80000u;
    h.now += 50000u;
    OCB_CHECK(get_status(&h, &m) && m.extra_len == 21u, "debug: second STATUS");
    if (m.extra_len == 21u) {
        const uint8_t sat[21] = {0xFFu, 0xFFu, 0u,    0xFFu, 0xFFu, 0xFFu, 0xFFu, 0xFFu, 0xFFu, 8u, 0u,
                                 0x2Cu, 0x01u, 2u,    0u,    1u,    0u,    1u,    0u,    1u,    0u};
        OCB_CHECK_MEM(m.extra, m.extra_len, sat, sizeof sat, "saturated counters");
    }
    /* A HELLO restarts max_app_gap_ms. */
    lh_hello(&h);
    OCB_CHECK(ocb_link_max_app_gap_ms(&h.link) == 0u, "HELLO kept the app gap");
}

static void test_overflow_and_cap(void)
{
    static lh h;
    lh_session(&h, 8u);
    const uint32_t d0 = ocb_link_counters_get(&h.link)->tx_dropped;
    for (uint8_t i = 0; i < 9u; i++) {
        ocb_link_queue_event(&h.link, (uint8_t)(OCB_EVT_PLAYBACK_DONE + (i % 5u)));
    }
    OCB_CHECK(ocb_link_counters_get(&h.link)->tx_dropped - d0 == 1u, "9th EVENT not dropped");
    /* 5 NAKs: semantic NAK VERSION after closing the session. */
    lh_hello_v(&h, 7u);
    for (uint8_t i = 0; i < 5u; i++) {
        lh_ping(&h);
    }
    OCB_CHECK(ocb_link_counters_get(&h.link)->tx_dropped - d0 == 2u, "5th NAK not dropped (%u)",
              ocb_link_counters_get(&h.link)->tx_dropped - d0);
    lh_out o[32];
    const size_t n = lh_drain(&h, o, 32u);
    size_t ev = 0;
    size_t nak = 0;
    for (size_t i = 0; i < n && i < 32u; i++) {
        ev += o[i].type == OCB_MSG_EVENT ? 1u : 0u;
        nak += o[i].type == OCB_MSG_NAK ? 1u : 0u;
    }
    OCB_CHECK(ev == 8u && nak == 4u, "kept %zu EVENT, %zu NAK", ev, nak);
    OCB_CHECK(n >= 8u && o[0].body[0] == OCB_EVT_PLAYBACK_DONE && o[7].body[0] == OCB_EVT_LATE, "EVENT order");

    /* ACK replaced (not counted). */
    lh_session(&h, 8u);
    const uint32_t d1 = ocb_link_counters_get(&h.link)->tx_dropped;
    const uint8_t s = h.seq;
    lh_ping(&h);
    lh_ping(&h);
    lh_ping(&h);
    lh_expect_ack(&h, (uint8_t)(s + 2u), "replaced ACK");
    OCB_CHECK(ocb_link_counters_get(&h.link)->tx_dropped == d1, "a replaced ACK was counted");

    /* cap too small: 0 and pending; later it comes out whole. */
    lh_ping(&h);
    uint8_t wire[OCB_MAX_WIRE_FRAME];
    OCB_CHECK(ocb_link_tx_next(&h.link, h.now, wire, 3u) == 0u, "frame split into a small cap");
    OCB_CHECK(ocb_link_tx_next(&h.link, h.now, wire, 0u) == 0u, "cap 0");
    const size_t len = ocb_link_tx_next(&h.link, h.now, wire, sizeof wire);
    lh_out one;
    OCB_CHECK(len > 3u && lh_decode_out(wire, len, &one) && one.type == OCB_MSG_ACK && one.body[0] == (uint8_t)(s + 3u),
              "pending frame lost after a small cap");
    /* Exactly the frame's size is enough. */
    lh_ping(&h);
    uint8_t ref[OCB_MAX_WIRE_FRAME];
    const ocb_msg_ack a = {(uint8_t)(s + 4u)};
    uint8_t body[4];
    const int bn = ocb_msg_ack_encode(&a, body, sizeof body);
    const size_t need = ocb_frame_encode(OCB_MSG_ACK, h.link.tx_seq, body, (size_t)bn, ref, sizeof ref);
    OCB_CHECK(ocb_link_tx_next(&h.link, h.now, wire, need - 1u) == 0u, "cap need-1");
    OCB_CHECK(ocb_link_tx_next(&h.link, h.now, wire, need) == need, "cap == frame size");
}

static void test_gate(void)
{
    static lh h;
    lh_init(&h, 8u);
    ocb_txgate *g = ocb_link_gate(&h.link);
    uint8_t wire[OCB_MAX_WIRE_FRAME];
    /* Closed: nothing leaves, STATUS not due, pending output discarded. */
    ocb_link_queue_event(&h.link, OCB_EVT_LATE);
    lh_hello_v(&h, 9u); /* INFO pending, session closed */
    lh_stop(&h);        /* ACK pending */
    lh_stop(&h);        /* (replaces the ACK) */
    lh_ping(&h);        /* NAK VERSION */
    OCB_CHECK(ocb_link_tx_next(&h.link, h.now, wire, sizeof wire) == 0u, "closed gate sent");
    OCB_CHECK(ocb_link_counters_get(&h.link)->tx_dropped == 4u, "closed gate: tx_dropped %u (EVENT, NAK, INFO, ACK)",
              ocb_link_counters_get(&h.link)->tx_dropped);

    /* Opens only after 1000 us of continuous high; a low sample restarts. */
    const uint64_t t = h.now;
    OCB_CHECK(!ocb_txgate_sample(g, true, t), "open at once");
    OCB_CHECK(!ocb_txgate_sample(g, true, t + 999u), "open at 999 us");
    OCB_CHECK(!ocb_txgate_sample(g, false, t + 999u), "low sample");
    OCB_CHECK(!ocb_txgate_sample(g, true, t + 1000u), "open after a low sample");
    OCB_CHECK(!ocb_txgate_sample(g, true, t + 1999u), "open at 999 us of the second run");
    OCB_CHECK(ocb_txgate_sample(g, true, t + 2000u), "not open after 1000 us high");
    OCB_CHECK(ocb_txgate_sample(g, false, t + 2001u), "a low sample closed an open gate");
    h.now = t + 2001u;
    /* Nothing stale leaves on reopening: only the first STATUS. */
    lh_out o[8];
    size_t n = lh_drain(&h, o, 8u);
    OCB_CHECK(n == 1u && o[0].type == OCB_MSG_STATUS, "after opening: %zu frames, first 0x%02x", n,
              n > 0u ? o[0].type : 0u);
    /* release closes at once and discards. */
    ocb_link_queue_event(&h.link, OCB_EVT_LATE);
    ocb_txgate_release(g);
    OCB_CHECK(!ocb_txgate_is_open(g), "release did not close");
    h.now += 100000u;
    OCB_CHECK(ocb_link_tx_next(&h.link, h.now, wire, sizeof wire) == 0u, "released gate sent");
    OCB_CHECK(!ocb_txgate_sample(g, true, h.now), "reopened without a new idle-high run");
    OCB_CHECK(ocb_txgate_sample(g, true, h.now + 1000u), "did not reopen");
    h.now += 1000u;
    n = lh_drain(&h, o, 8u);
    OCB_CHECK(n == 1u && o[0].type == OCB_MSG_STATUS, "after release: stale EVENT left");
}

/* A transport NAK discarded by the closed gate never reached the app: the
 * budget for that `expected` is re-armed (D27). */
static void test_gate_rearms_nak(void)
{
    static lh h;
    lh_session(&h, 8u);
    const uint8_t E = h.core.expected;
    ocb_txgate_release(ocb_link_gate(&h.link));
    send_corrupted(&h, E);
    uint8_t wire[OCB_MAX_WIRE_FRAME];
    OCB_CHECK(ocb_link_tx_next(&h.link, h.now, wire, sizeof wire) == 0u, "closed gate sent");
    lh_open_gate(&h);
    lh_discard_tx(&h); /* STATUS */
    send_corrupted(&h, E);
    lh_expect_nak(&h, OCB_NAK_CRC, E, 0u, "CRC after the closed gate discarded the first one");
    send_corrupted(&h, E);
    lh_expect_none(&h, "budget spent again by the NAK that left");
}

static void test_two_links(void)
{
    static lh a;
    static lh b;
    lh_session(&a, 8u);
    lh_init(&b, 4u);
    ocb_link_queue_event(&a.link, OCB_EVT_LATE);
    lh_heartbeat(&a, 1u, 1u, 1u, 0u);
    OCB_CHECK(!b.link.link_ok && b.link.ev_count == 0u && !ocb_link_session_open(&b.link) &&
                  !ocb_txgate_is_open(ocb_link_gate(&b.link)),
              "state shared between links");
    lh_open_gate(&b);
    lh_out o[8];
    const size_t n = lh_drain(&b, o, 8u);
    OCB_CHECK(n == 1u && o[0].type == OCB_MSG_STATUS, "b sent a's output");
    ocb_msg_status m;
    OCB_CHECK(lh_status_decode(&o[0], &m) && m.free == 4u && m.flags == 0u, "b STATUS from a's state");
}

int main(void)
{
    test_priority();
    test_cadence();
    test_fields();
    test_debug_counters();
    test_overflow_and_cap();
    test_gate();
    test_gate_rearms_nak();
    test_two_links();
    const int f = ocb_test_failures();
    if (f != 0) {
        fprintf(stderr, "test_link_tx (F.16): %d failure(s)\n", f);
        return 1;
    }
    printf("test_link_tx (F.16): ok\n");
    return 0;
}
