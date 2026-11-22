/*
 * SPDX-License-Identifier: MIT
 *
 * Host test F.2 (story 2.6 Task 5.7): the NAK matrix through the link
 * (AD-6, D6, D7). Transport reasons (CRC from a corrupted frame and from an
 * overlong run, SEQ, FULL) never advance `expected` and share one budget of
 * one NAK per value of `expected`; a new `expected` and a sync point
 * re-arm it; every semantic reason the controller produces advances
 * `expected` to seq + 1 and has no effect; NAK fields are `expected` after
 * handling and `got` = the frame's seq (0 for CRC); the core's precedence
 * still holds through the link. Story 2.7: NO_LINK (link down: no
 * heartbeat yet, or lost) is semantic for SET_STATE and START, with the same
 * fields and the cached response on a duplicate.
 */
#include <stdint.h>
#include <stdio.h>

#include "link_fixture.h"

static void qe_seq(lh *h, uint8_t seq, uint32_t t_us, ocb_state s)
{
    uint8_t body[OCB_MAX_BODY];
    const size_t n = lh_body_queue_event(t_us, s, body);
    lh_send_seq(h, OCB_MSG_QUEUE_EVENT, seq, body, n);
}

static void send_corrupted(lh *h, uint8_t seq)
{
    uint8_t wire[OCB_MAX_WIRE_FRAME];
    const size_t n = lh_wire(OCB_MSG_PING, seq, NULL, 0u, wire);
    wire[n - 2u] = (uint8_t)(wire[n - 2u] ^ 0x10u); /* a CRC byte */
    lh_feed(h, wire, n);
}

static void send_overlong(lh *h)
{
    for (uint32_t i = 0; i < 300u; i++) {
        ocb_link_rx_entry(&h->link, 0x42u, h->now);
    }
    ocb_link_rx_entry(&h->link, 0x00u, h->now);
}

static void test_crc_budget(void)
{
    static lh h;
    lh_session(&h, 512u);
    const uint8_t E = h.core.expected;

    /* A damaged frame, then the 10 frames already in flight: one NAK. */
    send_corrupted(&h, E);
    for (uint8_t i = 1u; i <= 10u; i++) {
        lh_send_seq(&h, OCB_MSG_PING, (uint8_t)(E + i), NULL, 0u);
    }
    lh_expect_nak(&h, OCB_NAK_CRC, E, 0u, "CRC then 10 in flight");
    OCB_CHECK(h.core.expected == E, "CRC advanced expected");
    OCB_CHECK(ocb_link_counters_get(&h.link)->nak_suppressed == 10u, "suppressed %u",
              ocb_link_counters_get(&h.link)->nak_suppressed);

    /* The resend of the damaged frame is accepted and re-arms the budget. */
    lh_send_seq(&h, OCB_MSG_PING, E, NULL, 0u);
    lh_expect_ack(&h, E, "resend after CRC");
    send_corrupted(&h, (uint8_t)(E + 1u));
    lh_expect_nak(&h, OCB_NAK_CRC, (uint8_t)(E + 1u), 0u, "CRC at the new expected");

    /* Overlong run: also NAK CRC (budget already spent here: none). */
    send_overlong(&h);
    lh_expect_none(&h, "overlong with the budget spent");

    /* A sync point re-arms even when expected does not change: STOP with
     * seq E (expected stays E + 1). */
    lh_send_seq(&h, OCB_MSG_STOP, E, NULL, 0u);
    lh_expect_ack(&h, E, "STOP");
    OCB_CHECK(h.core.expected == (uint8_t)(E + 1u), "STOP: expected %u", h.core.expected);
    send_overlong(&h);
    lh_expect_nak(&h, OCB_NAK_CRC, (uint8_t)(E + 1u), 0u, "overlong after a sync point");

    /* SEQ shares the same budget: spent by the CRC above. */
    lh_send_seq(&h, OCB_MSG_PING, (uint8_t)(E + 100u), NULL, 0u);
    lh_expect_none(&h, "SEQ with the budget spent by CRC");
    /* New expected: SEQ is sent, then a CRC at that expected is not. */
    lh_send_seq(&h, OCB_MSG_PING, (uint8_t)(E + 1u), NULL, 0u);
    lh_expect_ack(&h, (uint8_t)(E + 1u), "PING");
    lh_send_seq(&h, OCB_MSG_PING, (uint8_t)(E + 100u), NULL, 0u);
    lh_expect_nak(&h, OCB_NAK_SEQ, (uint8_t)(E + 2u), (uint8_t)(E + 100u), "SEQ");
    send_corrupted(&h, (uint8_t)(E + 2u));
    lh_expect_none(&h, "CRC with the budget spent by SEQ");
    OCB_CHECK(h.core.expected == (uint8_t)(E + 2u), "transport NAKs advanced expected");

    /* Range errors and empty runs: counted, no reply (AD-3). */
    uint8_t wire[OCB_MAX_WIRE_FRAME];
    lh_send_seq(&h, OCB_MSG_PING, (uint8_t)(E + 2u), NULL, 0u);
    lh_expect_ack(&h, (uint8_t)(E + 2u), "PING");
    const size_t n = lh_wire(OCB_MSG_STATUS, 0u, NULL, 0u, wire); /* controller type: range error */
    lh_feed(&h, wire, n);
    const uint8_t zeros[4] = {0u, 0u, 0u, 0u};
    lh_feed(&h, zeros, sizeof zeros);
    lh_expect_none(&h, "range error and empty runs");
    OCB_CHECK(h.link.rx.stats.range_errors == 1u, "range error not counted");
}

/* A transport NAK lost to a full NAK ring does not spend the budget (D27). */
static void test_ring_full_keeps_budget(void)
{
    static lh h;
    lh_session(&h, 8u);
    /* Fill the NAK ring with semantic NAKs (UNKNOWN_TYPE), not drained. */
    for (int i = 0; i < OCB_LINK_TX_NAK_CAP; i++) {
        lh_send_seq(&h, 0x07u, h.core.expected, NULL, 0u);
    }
    const uint8_t E = h.core.expected;
    send_corrupted(&h, E); /* NAK CRC lost: ring full */
    OCB_CHECK(!h.link.nak_spent, "a NAK lost to a full ring spent the budget");
    lh_discard_tx(&h);
    send_corrupted(&h, E);
    lh_expect_nak(&h, OCB_NAK_CRC, E, 0u, "CRC after a ring-full loss");
}

static void test_full(void)
{
    static lh h;
    lh_session(&h, 2u);
    qe_seq(&h, h.seq, 10u, lst_buttons(OCB_BTN_A));
    qe_seq(&h, (uint8_t)(h.seq + 1u), 20u, lst_buttons(OCB_BTN_X));
    h.seq = (uint8_t)(h.seq + 2u);
    lh_discard_tx(&h);
    const uint8_t E = h.core.expected;
    qe_seq(&h, E, 30u, lst_buttons(OCB_BTN_Y));
    lh_expect_nak(&h, OCB_NAK_FULL, E, E, "FULL");
    OCB_CHECK(h.core.expected == E, "FULL advanced expected");
    /* The resend and the frames behind it: no second NAK for this expected. */
    qe_seq(&h, E, 30u, lst_buttons(OCB_BTN_Y));
    qe_seq(&h, (uint8_t)(E + 1u), 40u, lst_buttons(OCB_BTN_Y));
    lh_expect_none(&h, "FULL twice for one expected");
    OCB_CHECK(ocb_queue_count(&h.core.queue) == 2u, "FULL enqueued");
}

static void test_full_retry(void)
{
    static lh h;
    lh_session(&h, 2u);
    qe_seq(&h, h.seq, 10u, lst_buttons(OCB_BTN_A));
    qe_seq(&h, (uint8_t)(h.seq + 1u), 20u, lst_buttons(OCB_BTN_X));
    h.seq = (uint8_t)(h.seq + 2u);
    lh_discard_tx(&h);
    const uint8_t E = h.core.expected;
    qe_seq(&h, E, 30u, lst_buttons(OCB_BTN_Y));
    lh_expect_nak(&h, OCB_NAK_FULL, E, E, "FULL");
    /* A slot frees: START (the app sends it at E instead of t=30), one
     * event applied, then the retry of t=30 is accepted. */
    lh_send_seq(&h, OCB_MSG_START, E, (const uint8_t[]){0u, 0u, 0u, 0u}, 4u);
    lh_expect_ack(&h, E, "START");
    lh_advance(&h, 10u);
    ocb_link_poll(&h.link, h.now); /* t=10 applied: one slot free */
    lh_discard_tx(&h);
    OCB_CHECK(ocb_queue_free(&h.core.queue) == 1u, "free %u", ocb_queue_free(&h.core.queue));
    const uint8_t E2 = h.core.expected;
    qe_seq(&h, E2, 30u, lst_buttons(OCB_BTN_Y));
    lh_expect_ack(&h, E2, "retry after a slot freed");
    OCB_CHECK(ocb_queue_count(&h.core.queue) == 2u, "retry not enqueued");
}

static void expect_semantic(lh *h, uint8_t reason, uint8_t seq, uint8_t mode0, uint16_t q0, const char *what)
{
    lh_expect_nak(h, reason, (uint8_t)(seq + 1u), seq, what);
    OCB_CHECK(h->core.expected == (uint8_t)(seq + 1u), "%s: expected %u", what, h->core.expected);
    OCB_CHECK(h->core.mode == mode0 && ocb_queue_count(&h->core.queue) == q0, "%s: had an effect (mode %u queue %u)",
              what, h->core.mode, ocb_queue_count(&h->core.queue));
}

static void test_semantic(void)
{
    static lh h;
    uint8_t ss[OCB_STATE_SIZE];
    ocb_state_encode(&OCB_NEUTRAL, ss);

    /* VERSION: session closed. */
    lh_init(&h, 8u);
    lh_open_gate(&h);
    lh_send_seq(&h, OCB_MSG_PING, 9u, NULL, 0u);
    expect_semantic(&h, OCB_NAK_VERSION, 9u, OCB_MODE_IDLE, 0u, "VERSION");

    /* ORDER, BAD_STATE, UNKNOWN_TYPE in IDLE. */
    lh_session(&h, 8u);
    qe_seq(&h, h.seq, 100u, lst_buttons(OCB_BTN_A));
    h.seq++;
    lh_discard_tx(&h);
    uint8_t s = h.seq;
    qe_seq(&h, s, 50u, lst_buttons(OCB_BTN_A));
    expect_semantic(&h, OCB_NAK_ORDER, s, OCB_MODE_IDLE, 1u, "ORDER");
    s = h.core.expected;
    ocb_state bad = OCB_NEUTRAL;
    bad.hat = 9u;
    qe_seq(&h, s, 200u, bad);
    expect_semantic(&h, OCB_NAK_BAD_STATE, s, OCB_MODE_IDLE, 1u, "BAD_STATE");
    s = h.core.expected;
    lh_send_seq(&h, 0x07u, s, NULL, 0u);
    expect_semantic(&h, OCB_NAK_UNKNOWN_TYPE, s, OCB_MODE_IDLE, 1u, "UNKNOWN_TYPE");
    /* Precedence through the link: BAD_STATE before ORDER (hat 9, old t). */
    s = h.core.expected;
    qe_seq(&h, s, 50u, bad);
    expect_semantic(&h, OCB_NAK_BAD_STATE, s, OCB_MODE_IDLE, 1u, "BAD_STATE over ORDER");

    /* BUSY: SET_STATE in PLAYING. */
    s = h.core.expected;
    lh_send_seq(&h, OCB_MSG_START, s, (const uint8_t[]){0x40u, 0x42u, 0x0Fu, 0u}, 4u);
    lh_expect_ack(&h, s, "START");
    s = h.core.expected;
    lh_send_seq(&h, OCB_MSG_SET_STATE, s, ss, sizeof ss);
    expect_semantic(&h, OCB_NAK_BUSY, s, OCB_MODE_PLAYING, 1u, "BUSY");

    /* ABORTED, and ABORTED before NOT_READY. */
    ocb_core_out out;
    ocb_core_abort(&h.core, OCB_ABORT_APP_TIMEOUT, &out);
    s = h.core.expected;
    qe_seq(&h, s, 300u, lst_buttons(OCB_BTN_A));
    expect_semantic(&h, OCB_NAK_ABORTED, s, OCB_MODE_IDLE, 0u, "ABORTED");
    ocb_usb_mailbox_suspend(&h.mb); /* not ready from now on */
    ocb_link_poll(&h.link, h.now);
    lh_discard_tx(&h);
    s = h.core.expected;
    lh_send_seq(&h, OCB_MSG_SET_STATE, s, ss, sizeof ss);
    expect_semantic(&h, OCB_NAK_ABORTED, s, OCB_MODE_IDLE, 0u, "ABORTED over NOT_READY");

    /* NOT_READY: latch cleared by STOP, console not ready. */
    s = h.core.expected;
    lh_send_seq(&h, OCB_MSG_STOP, s, NULL, 0u);
    lh_expect_ack(&h, s, "STOP");
    s = h.core.expected;
    lh_send_seq(&h, OCB_MSG_SET_STATE, s, ss, sizeof ss);
    expect_semantic(&h, OCB_NAK_NOT_READY, s, OCB_MODE_IDLE, 0u, "NOT_READY");
}

/* NO_LINK (story 2.7 D11): semantic, fields expected (after) and got = seq,
 * the cached response on a duplicate, no effect; after the first heartbeat
 * the same frames are NOT_READY (console not ready) or accepted (ready). */
static void test_no_link(void)
{
    static lh h;
    uint8_t ss[OCB_STATE_SIZE];
    ocb_state_encode(&OCB_NEUTRAL, ss);
    for (int ready = 0; ready <= 1; ready++) {
        lh_init(&h, 8u);
        lh_open_gate(&h);
        if (ready) {
            lh_make_ready(&h);
        }
        lh_hello(&h);
        qe_seq(&h, h.seq, 100u, lst_buttons(OCB_BTN_A));
        h.seq++;
        lh_discard_tx(&h);
        uint8_t s = h.core.expected;
        lh_send_seq(&h, OCB_MSG_SET_STATE, s, ss, sizeof ss);
        expect_semantic(&h, OCB_NAK_NO_LINK, s, OCB_MODE_IDLE, 1u, "NO_LINK SET_STATE");
        /* A duplicate of that seq gets the cached NAK, nothing else changes. */
        lh_send_seq(&h, OCB_MSG_SET_STATE, s, ss, sizeof ss);
        expect_semantic(&h, OCB_NAK_NO_LINK, s, OCB_MODE_IDLE, 1u, "NO_LINK SET_STATE duplicate");
        s = h.core.expected;
        lh_send_seq(&h, OCB_MSG_START, s, (const uint8_t[]){0u, 0u, 0u, 0u}, 4u);
        expect_semantic(&h, OCB_NAK_NO_LINK, s, OCB_MODE_IDLE, 1u, "NO_LINK START");
        lh_link_up(&h);
        s = h.core.expected;
        lh_send_seq(&h, OCB_MSG_SET_STATE, s, ss, sizeof ss);
        if (ready) {
            lh_expect_ack(&h, s, "SET_STATE after the first heartbeat, ready");
        } else {
            expect_semantic(&h, OCB_NAK_NOT_READY, s, OCB_MODE_IDLE, 1u, "SET_STATE after the first heartbeat");
        }
    }
}

int main(void)
{
    test_crc_budget();
    test_ring_full_keeps_budget();
    test_full();
    test_full_retry();
    test_semantic();
    test_no_link();
    const int f = ocb_test_failures();
    if (f != 0) {
        fprintf(stderr, "test_link_nak (F.2): %d failure(s)\n", f);
        return 1;
    }
    printf("test_link_nak (F.2): ok\n");
    return 0;
}
