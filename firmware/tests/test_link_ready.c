/*
 * SPDX-License-Identifier: MIT
 *
 * Host test F.10 (story 2.6 Task 4.4): `console_ready` with a fake profile
 * (AD-19, AD-8, D11). The fake profile posts to the USB mailbox exactly as
 * Story 2.3's profile will; the clock is fake. STATUS `flags` bit 1 and the
 * NOT_READY gate are sampled after every event and must agree: 0 before
 * the handshake, 1 after; suspend, unmount and bus reset clear it and abort
 * the core with EVENT USB_SUSPENDED (once) in IDLE, DIRECT and PLAYING;
 * only a new handshake restores it; the IN-report stall (> 100 ms) clears
 * the flag only and never aborts; reserved bits stay 0; mailbox counters
 * wrap without a false event; two trackers are independent.
 */
#include <stdint.h>
#include <stdio.h>

#include "link_fixture.h"

/* The fake HID profile: what Story 2.3 will call on core 0. */
typedef struct {
    ocb_usb_mailbox *mb;
} fake_profile;

static void fp_handshake(fake_profile *p)
{
    ocb_usb_mailbox_handshake_done(p->mb);
}
static void fp_in_report(fake_profile *p)
{
    ocb_usb_mailbox_in_report(p->mb);
}

/* STATUS flags now (a STATUS is forced due by clearing status_sent, the
 * one white-box poke of this test). */
static uint8_t status_flags(lh *h)
{
    h->link.status_sent = false;
    lh_out o[16];
    const size_t n = lh_drain(h, o, 16u);
    for (size_t i = 0; i < n && i < 16u; i++) {
        ocb_msg_status m;
        if (lh_status_decode(&o[i], &m)) {
            OCB_CHECK((m.flags & OCB_STATUS_RESERVED_MASK) == 0u, "reserved STATUS bits set: 0x%02x", m.flags);
            return m.flags;
        }
    }
    OCB_CHECK(false, "no STATUS");
    return 0xFFu;
}

/* NOT_READY gate now: a SET_STATE in IDLE (session open, latch clear) is
 * ACKed when ready, NAK NOT_READY otherwise; a STOP restores IDLE. */
static bool gate_ready(lh *h)
{
    const uint8_t seq = h->seq;
    lh_set_state(h, lst_buttons(OCB_BTN_A));
    lh_out o[8];
    const size_t n = lh_replies(h, o, 8u);
    bool ready = false;
    if (n == 1u && o[0].type == OCB_MSG_ACK && o[0].body[0] == seq) {
        ready = true;
    } else {
        ocb_msg_nak m;
        OCB_CHECK(n == 1u && o[0].type == OCB_MSG_NAK && ocb_msg_nak_decode(o[0].body, o[0].len, &m) == OCB_OK &&
                      m.reason == OCB_NAK_NOT_READY,
                  "SET_STATE: neither ACK nor NAK NOT_READY");
    }
    lh_stop(h);
    lh_discard_tx(h);
    return ready;
}

/* Samples both views and checks they agree with `want`. */
static void expect_ready(lh *h, bool want, const char *what)
{
    const bool pred = ocb_link_console_ready(&h->link, h->now);
    const uint8_t flags = status_flags(h);
    const bool bit = (flags & OCB_STATUS_CONSOLE_READY) != 0u;
    const bool gate = gate_ready(h);
    OCB_CHECK(pred == want && bit == want && gate == want, "%s: predicate %d STATUS bit1 %d NOT_READY gate %d, wanted %d",
              what, (int)pred, (int)bit, (int)gate, (int)want);
}

static void poll(lh *h)
{
    ocb_link_poll(&h->link, h->now);
}

static void setup(lh *h, fake_profile *p)
{
    lh_init(h, 16u);
    lh_open_gate(h);
    lh_hello(h);
    lh_discard_tx(h);
    lh_link_up(h); /* story 2.7: SET_STATE / START need the link */
    p->mb = &h->mb;
}

static void test_handshake_and_down(void)
{
    static lh h;
    fake_profile p;
    setup(&h, &p);
    expect_ready(&h, false, "before the handshake");
    fp_handshake(&p);
    expect_ready(&h, false, "posted but not polled yet");
    poll(&h);
    expect_ready(&h, true, "after the handshake");

    void (*downs[3])(ocb_usb_mailbox *) = {ocb_usb_mailbox_suspend, ocb_usb_mailbox_unmount,
                                           ocb_usb_mailbox_bus_reset};
    for (int d = 0; d < 3; d++) {
        for (uint8_t mode = OCB_MODE_IDLE; mode <= OCB_MODE_PLAYING; mode++) {
            setup(&h, &p);
            fp_handshake(&p);
            poll(&h);
            lh_queue_event(&h, 0u, lst_buttons(OCB_BTN_A));
            lh_queue_event(&h, 500000u, lst_end());
            if (mode == OCB_MODE_DIRECT) {
                lh_set_state(&h, lst_buttons(OCB_BTN_B));
            } else if (mode == OCB_MODE_PLAYING) {
                lh_start(&h, 0u);
                poll(&h); /* the t=0 note is held */
            }
            lh_discard_tx(&h);
            OCB_CHECK(h.core.mode == mode, "setup mode %u", mode);
            const uint8_t aborts = h.core.abort_count;
            downs[d](&h.mb);
            poll(&h);
            lh_out o[8];
            const size_t n = lh_replies(&h, o, 8u);
            OCB_CHECK(n == 1u && o[0].type == OCB_MSG_EVENT && o[0].body[0] == OCB_EVT_USB_SUSPENDED,
                      "down %d in mode %u: %zu frames, wanted one EVENT USB_SUSPENDED", d, mode, n);
            OCB_CHECK(h.core.aborted && h.core.mode == OCB_MODE_IDLE && ocb_state_is_neutral(&h.core.published) &&
                          h.core.abort_count == (uint8_t)(aborts + 1u),
                      "down %d in mode %u: core not aborted", d, mode);
            /* A second down event while latched: no EVENT. */
            downs[(d + 1) % 3](&h.mb);
            poll(&h);
            lh_expect_none(&h, "second down event while aborted");
            lh_stop(&h); /* clears the latch; readiness stays 0 */
            lh_discard_tx(&h);
            expect_ready(&h, false, "after a down event");
            lh_advance(&h, 10000u);
            fp_in_report(&p);
            poll(&h);
            expect_ready(&h, false, "IN report after a down event (no handshake)");
            fp_handshake(&p);
            poll(&h);
            expect_ready(&h, true, "new handshake after a down event");
        }
    }

    /* Down and handshake in the same poll: down first, so ready. */
    setup(&h, &p);
    fp_handshake(&p);
    poll(&h);
    ocb_usb_mailbox_suspend(&h.mb);
    fp_handshake(&p);
    poll(&h);
    lh_discard_tx(&h);
    lh_stop(&h);
    lh_discard_tx(&h);
    expect_ready(&h, true, "down and handshake in one poll");
}

static void test_stall(void)
{
    static lh h;
    fake_profile p;
    setup(&h, &p);
    fp_handshake(&p);
    poll(&h);
    /* IN reports every 1 ms keep it 1 (500 ms). */
    for (int i = 0; i < 500; i++) {
        lh_advance(&h, 1000u);
        fp_in_report(&p);
        poll(&h);
        if (!ocb_link_console_ready(&h.link, h.now)) {
            OCB_CHECK(false, "lost ready with IN reports every 1 ms (step %d)", i);
            break;
        }
    }
    expect_ready(&h, true, "IN reports every 1 ms");
    /* Exactly 100 ms of silence: still 1; 1 us more: 0. */
    fp_in_report(&p);
    poll(&h);
    const uint64_t last = h.now;
    lh_advance(&h, last + 100000u - h.now); /* heartbeats continue (story 2.7) */
    poll(&h);
    expect_ready(&h, true, "100 ms without an IN report");
    lh_advance(&h, last + 100001u - h.now);
    poll(&h);
    expect_ready(&h, false, "100 ms + 1 us without an IN report");
    fp_in_report(&p);
    poll(&h);
    expect_ready(&h, true, "one IN report restores it");

    /* The stall never aborts PLAYING (AD-19); START and SET_STATE get
     * NOT_READY while it lasts. */
    lh_queue_event(&h, 0u, lst_buttons(OCB_BTN_A));
    lh_queue_event(&h, 1000u, lst_buttons(OCB_BTN_X));
    lh_queue_event(&h, 900000u, lst_end());
    lh_start(&h, 0u);
    lh_discard_tx(&h);
    OCB_CHECK(h.core.mode == OCB_MODE_PLAYING, "PLAYING");
    const uint8_t aborts = h.core.abort_count;
    for (int i = 0; i < 300; i++) {
        lh_advance(&h, 1000u); /* no IN reports */
        poll(&h);
    }
    OCB_CHECK(!ocb_link_console_ready(&h.link, h.now), "not stalled");
    OCB_CHECK(h.core.abort_count == aborts && !h.core.aborted && h.core.mode == OCB_MODE_PLAYING,
              "the stall aborted the piece");
    lh_set_state(&h, lst_buttons(OCB_BTN_B));
    lh_expect_nak(&h, OCB_NAK_BUSY, h.seq, (uint8_t)(h.seq - 1u), "SET_STATE in PLAYING: BUSY before NOT_READY");
    lh_out o[16];
    const size_t n = lh_drain(&h, o, 16u);
    for (size_t i = 0; i < n && i < 16u; i++) {
        OCB_CHECK(!(o[i].type == OCB_MSG_EVENT && o[i].body[0] == OCB_EVT_USB_SUSPENDED), "stall sent USB_SUSPENDED");
    }
    lh_stop(&h);
    lh_discard_tx(&h);
    expect_ready(&h, false, "stalled, after STOP");
    lh_queue_event(&h, 0u, lst_buttons(OCB_BTN_A));
    lh_discard_tx(&h);
    lh_start(&h, 0u);
    lh_expect_nak(&h, OCB_NAK_NOT_READY, h.seq, (uint8_t)(h.seq - 1u), "START while stalled");
    fp_in_report(&p);
    poll(&h);
    lh_start(&h, 0u);
    lh_expect_ack(&h, (uint8_t)(h.seq - 1u), "START once IN reports resume");
}

static void test_flags_and_wrap(void)
{
    static lh h;
    fake_profile p;
    setup(&h, &p);
    lh_heartbeat(&h, 1u, 0u, 0u, OCB_HB_FLAG_DTR);
    uint8_t f = status_flags(&h);
    OCB_CHECK(f == OCB_STATUS_LINK_OK, "flags 0x%02x, wanted link_ok only", f);
    fp_handshake(&p);
    poll(&h);
    f = status_flags(&h);
    OCB_CHECK(f == (OCB_STATUS_LINK_OK | OCB_STATUS_CONSOLE_READY), "flags 0x%02x, wanted 0x03", f);

    /* Counters at 2^32 - 1 when the tracker starts: no false event; the
     * increment that wraps to 0 is one event. */
    lh_init(&h, 16u);
    h.mb.handshakes = 0xFFFFFFFFu;
    h.mb.in_reports = 0xFFFFFFFFu;
    h.mb.suspends = 0xFFFFFFFFu;
    h.mb.unmounts = 0xFFFFFFFEu;
    h.mb.bus_resets = 0xFFFFFFFFu;
    ocb_ready_init(&h.link.ready, &h.mb, h.now);
    lh_open_gate(&h);
    lh_hello(&h);
    lh_discard_tx(&h);
    lh_link_up(&h);
    p.mb = &h.mb;
    for (int i = 0; i < 5; i++) {
        lh_advance(&h, 1000u);
        poll(&h);
    }
    lh_expect_none(&h, "no false event at 2^32 - 1");
    expect_ready(&h, false, "not ready at start");
    fp_handshake(&p); /* wraps to 0 */
    poll(&h);
    OCB_CHECK(h.mb.handshakes == 0u, "handshake counter did not wrap");
    expect_ready(&h, true, "handshake across the wrap");
    ocb_usb_mailbox_suspend(&h.mb); /* wraps to 0 */
    poll(&h);
    lh_out o[8];
    const size_t n = lh_replies(&h, o, 8u);
    OCB_CHECK(n == 1u && o[0].type == OCB_MSG_EVENT, "suspend across the wrap: %zu frames", n);

    /* Two independent trackers. */
    ocb_usb_mailbox ma;
    ocb_usb_mailbox mb;
    ocb_usb_mailbox_init(&ma);
    ocb_usb_mailbox_init(&mb);
    ocb_ready ra;
    ocb_ready rb;
    ocb_ready_init(&ra, &ma, 0u);
    ocb_ready_init(&rb, &mb, 0u);
    ocb_usb_mailbox_handshake_done(&ma);
    (void)ocb_ready_poll(&ra, &ma, 10u);
    (void)ocb_ready_poll(&rb, &mb, 10u);
    OCB_CHECK(ocb_ready_is(&ra, 10u) && !ocb_ready_is(&rb, 10u), "trackers shared state");
    ocb_usb_mailbox_bus_reset(&mb);
    OCB_CHECK(ocb_ready_poll(&rb, &mb, 20u) == OCB_READY_DOWN_BUS_RESET && ocb_ready_poll(&ra, &ma, 20u) == 0u,
              "down event leaked");
    OCB_CHECK(ocb_ready_is(&ra, 20u), "a's ready lost");
}

/* Posts made before the link is initialised are kept when the tracker is
 * rebased on a zero baseline, as the firmware does (D28). */
static void test_rebase(void)
{
    static lh h;
    fake_profile p;
    lh_init(&h, 16u);
    p.mb = &h.mb;
    /* Without the rebase, a handshake posted before init is taken as seen. */
    fp_handshake(&p);
    ocb_ready_init(&h.link.ready, &h.mb, h.now); /* what ocb_link_init does */
    ocb_link_poll(&h.link, h.now);
    OCB_CHECK(!ocb_link_console_ready(&h.link, h.now), "snapshot baseline: pre-init handshake counted");
    static const ocb_usb_mailbox zero = {0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u};
    ocb_link_ready_rebase(&h.link, &zero, h.now);
    ocb_link_poll(&h.link, h.now);
    OCB_CHECK(ocb_link_console_ready(&h.link, h.now), "zero baseline: pre-init handshake lost");
}

/* Story 2.3 D13: the mailbox also carries the measured report period; it
 * starts at 0 and never touches `ready` (not a counter, not an event). */
static void test_report_period_field(void)
{
    static lh h;
    fake_profile p;
    setup(&h, &p);
    OCB_CHECK(h.mb.report_period_us == 0u, "mailbox init: report period %u", h.mb.report_period_us);
    ocb_usb_mailbox_set_report_period(&h.mb, 1000u);
    poll(&h);
    expect_ready(&h, false, "a report period is not a handshake");
    fp_handshake(&p);
    poll(&h);
    ocb_usb_mailbox_set_report_period(&h.mb, 0u);
    poll(&h);
    expect_ready(&h, true, "a period of 0 is not a USB-down event");
}

/* Story 2.5 D12, D13: the self-timing fields start at 0, are stored as
 * given, and never touch `ready`. */
static void test_self_timing_fields(void)
{
    static lh h;
    fake_profile p;
    setup(&h, &p);
    const ocb_usb_mailbox *m = &h.mb;
    OCB_CHECK(m->in_min_us == 0u && m->in_p95_us == 0u && m->in_max_us == 0u && m->in_slow == 0u &&
                  m->core0_pass_max_us == 0u && m->seq_retries == 0u && m->seq_fallbacks == 0u,
              "mailbox init: self-timing fields not zero");
    ocb_usb_mailbox_set_in_spread(&h.mb, 7950u, 8050u, 16000u, 3u);
    ocb_usb_mailbox_set_core0(&h.mb, 412u, 9u, 1u);
    OCB_CHECK(m->in_min_us == 7950u && m->in_p95_us == 8050u && m->in_max_us == 16000u && m->in_slow == 3u,
              "in spread stored: %u %u %u %u", m->in_min_us, m->in_p95_us, m->in_max_us, m->in_slow);
    OCB_CHECK(m->core0_pass_max_us == 412u && m->seq_retries == 9u && m->seq_fallbacks == 1u,
              "core 0 fields stored: %u %u %u", m->core0_pass_max_us, m->seq_retries, m->seq_fallbacks);
    poll(&h);
    expect_ready(&h, false, "self-timing fields are not a handshake");
    fp_handshake(&p);
    poll(&h);
    ocb_usb_mailbox_set_in_spread(&h.mb, 0u, 0u, 0u, 0u);
    ocb_usb_mailbox_set_core0(&h.mb, 0u, 0u, 0u);
    poll(&h);
    expect_ready(&h, true, "zeroed self-timing fields are not a USB-down event");
    ocb_usb_mailbox_init(&h.mb);
    OCB_CHECK(m->in_max_us == 0u && m->seq_fallbacks == 0u, "init zeroes the self-timing fields");
}

int main(void)
{
    test_handshake_and_down();
    test_report_period_field();
    test_self_timing_fields();
    test_stall();
    test_flags_and_wrap();
    test_rebase();
    const int f = ocb_test_failures();
    if (f != 0) {
        fprintf(stderr, "test_link_ready (F.10): %d failure(s)\n", f);
        return 1;
    }
    printf("test_link_ready (F.10): ok\n");
    return 0;
}
