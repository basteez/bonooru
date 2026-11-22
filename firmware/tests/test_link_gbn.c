/*
 * SPDX-License-Identifier: MIT
 *
 * Host test F.1 (story 2.6 Task 5.6): go-back-N through the link (AD-6,
 * D5, D6). In-order accept across the seq wrap; seq == expected is the
 * only path to the core (probe: queue count, expected); stale frames at
 * d = 1, 2, 63, 64 and the first values outside the window; the cached
 * response of d == 1 (ACK or the same semantic NAK); the window across the
 * wrap; the cache emptied by a sync point; sync points executed at any d.
 * Frames are built with ocb_frame_encode and fed byte by byte.
 */
#include <stdint.h>
#include <stdio.h>

#include "link_fixture.h"

static uint32_t g_t = 0u; /* strictly increasing t_us for QUEUE_EVENT */

static void qe_seq(lh *h, uint8_t seq, uint32_t t_us)
{
    uint8_t body[OCB_MAX_BODY];
    const size_t n = lh_body_queue_event(t_us, lst_buttons(OCB_BTN_A), body);
    lh_send_seq(h, OCB_MSG_QUEUE_EVENT, seq, body, n);
}

/* In-order QUEUE_EVENTs with h->seq; each ACKed. */
static void accept_n(lh *h, uint32_t n)
{
    for (uint32_t i = 0; i < n; i++) {
        const uint8_t seq = h->seq;
        g_t += 10u;
        qe_seq(h, seq, g_t);
        h->seq = (uint8_t)(seq + 1u);
        lh_expect_ack(h, seq, "in-order QUEUE_EVENT");
    }
}

static void test_in_order_wrap(void)
{
    static lh h;
    lh_session(&h, 512u);
    g_t = 0u;
    const uint8_t first = h.seq;
    accept_n(&h, 300u);
    OCB_CHECK(ocb_queue_count(&h.core.queue) == 300u, "300 in order: queue %u", ocb_queue_count(&h.core.queue));
    OCB_CHECK(h.core.expected == (uint8_t)(first + 300u), "300 in order: expected %u", h.core.expected);
    ocb_core_status_view v;
    ocb_core_status(&h.core, h.now, &v);
    OCB_CHECK(v.last_seq == (uint8_t)(first + 299u), "last_seq %u", v.last_seq);
}

/* Stale and out-of-window frames at a given expected value E. */
static void check_window_at(lh *h, const char *where)
{
    const uint8_t E = h->core.expected;
    const uint16_t q0 = ocb_queue_count(&h->core.queue);
    const uint8_t mode0 = h->core.mode;

    /* d in the window: ACK(E-1), no effect (d == 1 has a cached ACK here). */
    const uint8_t win[] = {1u, 2u, 63u, 64u};
    for (size_t i = 0; i < sizeof win; i++) {
        const uint8_t seq = (uint8_t)(E - win[i]);
        qe_seq(h, seq, 1u); /* t_us 1 would be ORDER if the core saw it */
        lh_expect_ack(h, (uint8_t)(E - 1u), where);
        OCB_CHECK(h->core.expected == E && ocb_queue_count(&h->core.queue) == q0 && h->core.mode == mode0,
                  "%s: d %u had an effect (expected %u queue %u)", where, win[i], h->core.expected,
                  ocb_queue_count(&h->core.queue));
    }
    /* Outside the window (behind and ahead): NAK SEQ, once per expected;
     * an in-order PING before each one moves expected and re-arms it. */
    const uint8_t out[] = {65u, 66u, 128u, 191u, 255u};
    for (size_t i = 0; i < sizeof out; i++) {
        const uint8_t ps = h->core.expected;
        lh_send_seq(h, OCB_MSG_PING, ps, NULL, 0u);
        lh_expect_ack(h, ps, "PING re-arm");
        const uint8_t e = h->core.expected;
        const uint8_t seq = (uint8_t)(e - out[i]);
        qe_seq(h, seq, 1u);
        lh_expect_nak(h, OCB_NAK_SEQ, e, seq, where);
        OCB_CHECK(h->core.expected == e && ocb_queue_count(&h->core.queue) == q0, "%s: d %u had an effect", where,
                  out[i]);
        /* The same frame again: the budget for this expected is spent. */
        qe_seq(h, seq, 1u);
        lh_expect_none(h, "second NAK SEQ for the same expected");
    }
    h->seq = h->core.expected;
}

static void test_stale(void)
{
    static lh h;
    lh_session(&h, 512u);
    g_t = 0u;
    accept_n(&h, 70u);
    check_window_at(&h, "window mid-range");

    /* Across the wrap: expected = 2, so 255, 200 and 194 are stale
     * (d = 3, 58, 64) and 193 (d = 65) is outside. */
    lh_session(&h, 512u);
    g_t = 0u;
    accept_n(&h, (uint32_t)(256u + 2u - h.seq));
    OCB_CHECK(h.core.expected == 2u, "wrap setup: expected %u", h.core.expected);
    const uint8_t stale[] = {255u, 200u, 194u, 1u};
    for (size_t i = 0; i < sizeof stale; i++) {
        qe_seq(&h, stale[i], 1u);
        lh_expect_ack(&h, 1u, "stale across the wrap");
    }
    qe_seq(&h, 193u, 1u);
    lh_expect_nak(&h, OCB_NAK_SEQ, 2u, 193u, "d 65 across the wrap");
    OCB_CHECK(h.core.expected == 2u, "wrap: expected moved");
    check_window_at(&h, "window at the wrap");
}

static void test_cached_nak(void)
{
    static lh h;
    lh_session(&h, 512u);
    g_t = 1000u;
    accept_n(&h, 3u);
    /* An out-of-order t_us: NAK ORDER (semantic), expected advances. */
    const uint8_t k = h.seq;
    qe_seq(&h, k, 5u);
    h.seq = (uint8_t)(k + 1u);
    lh_expect_nak(&h, OCB_NAK_ORDER, (uint8_t)(k + 1u), k, "ORDER");
    const uint16_t q0 = ocb_queue_count(&h.core.queue);
    /* The duplicate (d == 1) gets the same NAK, even with a body that would
     * now be accepted: the core never sees it. */
    qe_seq(&h, k, g_t + 100u);
    lh_expect_nak(&h, OCB_NAK_ORDER, (uint8_t)(k + 1u), k, "duplicate of a NAKed frame");
    OCB_CHECK(ocb_queue_count(&h.core.queue) == q0 && h.core.expected == (uint8_t)(k + 1u), "duplicate had an effect");
    /* d == 2: plain ACK(expected - 1). */
    qe_seq(&h, (uint8_t)(k - 1u), 1u);
    lh_expect_ack(&h, k, "d 2 after a NAK");

    /* A sync point empties the cache: STOP with the NAKed frame's seq, then
     * the same seq again as a QUEUE_EVENT (d == 1): ACK, not the NAK. */
    lh_send_seq(&h, OCB_MSG_STOP, k, NULL, 0u);
    lh_expect_ack(&h, k, "STOP");
    OCB_CHECK(h.core.expected == (uint8_t)(k + 1u), "STOP: expected %u", h.core.expected);
    qe_seq(&h, k, 1u);
    lh_expect_ack(&h, k, "d 1 after a sync point (cache empty)");
    OCB_CHECK(ocb_queue_count(&h.core.queue) == 0u, "d 1 after STOP enqueued");

    /* Cached ACK: a duplicate QUEUE_EVENT never enqueues twice. */
    h.seq = h.core.expected;
    accept_n(&h, 1u);
    const uint16_t q1 = ocb_queue_count(&h.core.queue);
    for (int i = 0; i < 5; i++) {
        qe_seq(&h, (uint8_t)(h.seq - 1u), g_t + 50u);
        lh_expect_ack(&h, (uint8_t)(h.seq - 1u), "duplicate of an ACKed QUEUE_EVENT");
    }
    OCB_CHECK(ocb_queue_count(&h.core.queue) == q1, "duplicate QUEUE_EVENT enqueued (%u vs %u)",
              ocb_queue_count(&h.core.queue), q1);
}

/* Sync points are executed with any seq, also inside the stale window. */
static void test_sync_points_any_d(void)
{
    static lh h;
    const uint8_t ds[] = {1u, 2u, 30u, 64u, 65u, 200u};
    for (size_t i = 0; i < sizeof ds; i++) {
        lh_session(&h, 512u);
        g_t = 0u;
        accept_n(&h, 80u);
        lh_start(&h, 1000000u);
        lh_discard_tx(&h);
        OCB_CHECK(h.core.mode == OCB_MODE_PLAYING, "setup PLAYING");
        const uint8_t seq = (uint8_t)(h.core.expected - ds[i]);
        lh_send_seq(&h, OCB_MSG_STOP, seq, NULL, 0u);
        lh_expect_ack(&h, seq, "STOP at a stale d");
        OCB_CHECK(h.core.mode == OCB_MODE_IDLE && ocb_queue_count(&h.core.queue) == 0u &&
                      h.core.expected == (uint8_t)(seq + 1u),
                  "STOP at d %u not executed (mode %u expected %u)", ds[i], h.core.mode, h.core.expected);

        h.seq = h.core.expected;
        g_t = 0u;
        accept_n(&h, 10u);
        const uint8_t hseq = (uint8_t)(h.core.expected - ds[i]);
        uint8_t pv = (uint8_t)OCB_PROTO_VERSION;
        lh_send_seq(&h, OCB_MSG_HELLO, hseq, &pv, 1u);
        lh_out o[4];
        const size_t n = lh_replies(&h, o, 4u);
        OCB_CHECK(n == 1u && o[0].type == OCB_MSG_INFO, "HELLO at d %u: %zu replies", ds[i], n);
        OCB_CHECK(ocb_queue_count(&h.core.queue) == 0u && h.core.expected == (uint8_t)(hseq + 1u),
                  "HELLO at d %u not executed", ds[i]);
    }
}

int main(void)
{
    test_in_order_wrap();
    test_stale();
    test_cached_nak();
    test_sync_points_any_d();
    const int f = ocb_test_failures();
    if (f != 0) {
        fprintf(stderr, "test_link_gbn (F.1): %d failure(s)\n", f);
        return 1;
    }
    printf("test_link_gbn (F.1): ok\n");
    return 0;
}
