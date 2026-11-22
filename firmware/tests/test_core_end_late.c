/*
 * SPDX-License-Identifier: MIT
 *
 * Host test F.4 (story 2.4): end of piece (END), reserved flag bits,
 * underrun judged at apply time, LATE (no backlog), the lead, ordering of
 * t_us, one event per scheduler pass (AD-8, D4, D6).
 */
#include <stdint.h>
#include <stdio.h>

#include "core_fixture.h"

#define PERIOD 1000u

/* Queues the given events and starts with `lead`; returns t0. */
static uint64_t start_with(harness *h, const uint32_t *t, const ocb_state *s, size_t n, uint32_t lead)
{
    for (size_t i = 0; i < n; i++) {
        const ocb_core_out o = h_queue_event(h, t[i], s[i]);
        OCB_CHECK(o.kind == OCB_CORE_REPLY_ACK, "setup: event %zu refused (NAK 0x%02x)", i, o.nak_reason);
    }
    const ocb_core_out o = h_start(h, lead);
    OCB_CHECK(o.kind == OCB_CORE_REPLY_ACK, "setup: START refused (NAK 0x%02x)", o.nak_reason);
    return h->now + lead;
}

static void expect_nak(harness *h, ocb_core_out o, uint8_t reason, const char *what)
{
    (void)h;
    OCB_CHECK(o.kind == OCB_CORE_REPLY_NAK && o.nak_reason == reason, "%s: kind %d NAK 0x%02x, wanted NAK 0x%02x",
              what, (int)o.kind, o.nak_reason, reason);
}

static void test_end_applied(void)
{
    harness h;
    h_init(&h, 8u, PERIOD);
    const uint32_t t[] = {1000u, 2000u};
    const ocb_state s[] = {st_buttons(OCB_BTN_A), st_end()};
    const uint64_t t0 = start_with(&h, t, s, 2, 0u);
    h.now = t0 + 1000u;
    ocb_core_out o = h_tick(&h);
    OCB_CHECK(o.event == OCB_EVT_NONE && h.core.published.buttons == OCB_BTN_A, "note not applied");
    const uint32_t pc = h.core.publish_count;
    h.now = t0 + 2000u;
    o = h_tick(&h);
    OCB_CHECK(o.kind == OCB_CORE_REPLY_NONE, "tick reply kind %d", (int)o.kind);
    OCB_CHECK(o.event == OCB_EVT_PLAYBACK_DONE, "END: event %u, wanted PLAYBACK_DONE", o.event);
    OCB_CHECK(h.core.mode == OCB_MODE_IDLE && h_published_neutral(&h), "END: not IDLE/NEUTRAL");
    OCB_CHECK(h.core.aborted && h.core.abort_count == 1u, "END applied must set the latch and count");
    OCB_CHECK(h.core.last_event == OCB_EVT_PLAYBACK_DONE, "last_event %u", h.core.last_event);
    OCB_CHECK(h.core.publish_count == pc + 2u, "END: publish_count +%u, wanted +2", h.core.publish_count - pc);
    OCB_CHECK(h.core.published.flags == 0u, "END flag published");
}

static void test_end_on_non_neutral(void)
{
    ocb_state bad[5];
    const char *names[5] = {"button A", "hat up", "lx off-centre", "ry off-centre", "buttons 0x8000"};
    for (int i = 0; i < 5; i++) {
        bad[i] = st_end();
    }
    bad[0].buttons = OCB_BTN_A;
    bad[1].hat = OCB_HAT_UP;
    bad[2].lx = 0x00;
    bad[3].ry = 0xFF;
    bad[4].buttons = 0x8000u;
    for (int i = 0; i < 5; i++) {
        harness h;
        h_init(&h, 8u, PERIOD);
        const uint8_t seq = h.seq;
        expect_nak(&h, h_queue_event(&h, 100u, bad[i]), OCB_NAK_BAD_STATE, names[i]);
        OCB_CHECK(h.core.expected == (uint8_t)(seq + 1u), "%s: BAD_STATE must advance expected", names[i]);
        OCB_CHECK(ocb_queue_count(&h.core.queue) == 0u && !h.core.have_last_t, "%s: refused event stored",
                  names[i]);
    }
}

static void test_reserved_flags(void)
{
    const uint8_t flags[] = {0x02, 0x80, 0xFE, 0x03};
    for (size_t i = 0; i < sizeof flags; i++) {
        harness h;
        h_init(&h, 8u, PERIOD);
        ocb_state s = st_neutral();
        s.flags = flags[i];
        char what[64];
        snprintf(what, sizeof what, "QUEUE_EVENT flags 0x%02x", flags[i]);
        expect_nak(&h, h_queue_event(&h, 100u, s), OCB_NAK_BAD_STATE, what);
        snprintf(what, sizeof what, "SET_STATE flags 0x%02x", flags[i]);
        expect_nak(&h, h_set_state(&h, s), OCB_NAK_BAD_STATE, what);
        OCB_CHECK(h.core.mode == OCB_MODE_IDLE, "%s changed the mode", what);
    }
    harness h;
    h_init(&h, 8u, PERIOD);
    expect_nak(&h, h_set_state(&h, st_end()), OCB_NAK_BAD_STATE, "SET_STATE with END");
    OCB_CHECK(h.core.mode == OCB_MODE_IDLE && h.core.publish_count == 0u, "SET_STATE with END applied");
}

static void test_nothing_after_end(void)
{
    harness h;
    h_init(&h, 8u, PERIOD);
    OCB_CHECK(h_queue_event(&h, 100u, st_end()).kind == OCB_CORE_REPLY_ACK, "END refused");
    expect_nak(&h, h_queue_event(&h, 200u, st_buttons(OCB_BTN_A)), OCB_NAK_BAD_STATE, "event after END");
    expect_nak(&h, h_queue_event(&h, 300u, st_end()), OCB_NAK_BAD_STATE, "second END");
    OCB_CHECK(ocb_queue_count(&h.core.queue) == 1u, "event after END stored");
    (void)h_stop(&h);
    OCB_CHECK(h_queue_event(&h, 200u, st_buttons(OCB_BTN_A)).kind == OCB_CORE_REPLY_ACK,
              "END marker not reset by entering IDLE");
}

static void test_underrun(void)
{
    /* The last event is a note: in the same tick the note is published,
     * then NEUTRAL, and UNDERRUN is returned. */
    harness h;
    h_init(&h, 8u, PERIOD);
    const uint32_t t[] = {1000u, 2000u};
    const ocb_state s[] = {st_buttons(OCB_BTN_A), st_buttons(OCB_BTN_X)};
    const uint64_t t0 = start_with(&h, t, s, 2, 0u);
    h.now = t0 + 1000u;
    ocb_core_out o = h_tick(&h);
    OCB_CHECK(o.event == OCB_EVT_NONE && h.core.mode == OCB_MODE_PLAYING,
              "note with more queued must not underrun");
    OCB_CHECK(h.core.published.buttons == OCB_BTN_A, "first note not published");
    const uint32_t pc = h.core.publish_count;
    h.now = t0 + 2000u;
    o = h_tick(&h);
    OCB_CHECK(o.event == OCB_EVT_UNDERRUN, "underrun: event %u", o.event);
    OCB_CHECK(h.core.publish_count == pc + 2u, "underrun: publish_count +%u, wanted +2 (note then NEUTRAL)",
              h.core.publish_count - pc);
    OCB_CHECK(h.core.mode == OCB_MODE_IDLE && h_published_neutral(&h) && h.core.aborted, "underrun state");
    OCB_CHECK(h.core.last_event == OCB_EVT_UNDERRUN && h.core.abort_count == 1u, "underrun bookkeeping");
}

static void test_late(void)
{
    /* Overdue by exactly one report period: applied. */
    {
        harness h;
        h_init(&h, 8u, PERIOD);
        const uint32_t t[] = {1000u, 900000u};
        const ocb_state s[] = {st_buttons(OCB_BTN_A), st_end()};
        const uint64_t t0 = start_with(&h, t, s, 2, 0u);
        h.now = t0 + 1000u + PERIOD;
        const ocb_core_out o = h_tick(&h);
        OCB_CHECK(o.event == OCB_EVT_NONE && h.core.published.buttons == OCB_BTN_A,
                  "overdue by exactly late_us must apply (event %u)", o.event);
    }
    /* Overdue by one report period + 1 us: LATE, nothing fired, ever. */
    {
        harness h;
        h_init(&h, 8u, PERIOD);
        const uint32_t t[] = {1000u, 2000u, 3000u, 4000u};
        const ocb_state s[] = {st_buttons(OCB_BTN_A), st_buttons(OCB_BTN_X), st_buttons(OCB_BTN_Y), st_end()};
        const uint64_t t0 = start_with(&h, t, s, 4, 0u);
        const uint32_t pc = h.core.publish_count;
        h.now = t0 + 1000u + PERIOD + 1u;
        ocb_core_out o = h_tick(&h);
        OCB_CHECK(o.event == OCB_EVT_LATE, "LATE: event %u", o.event);
        OCB_CHECK(h.core.mode == OCB_MODE_IDLE && h_published_neutral(&h) && h.core.aborted, "LATE state");
        OCB_CHECK(ocb_queue_count(&h.core.queue) == 0u, "LATE: queue not cleared");
        OCB_CHECK(h.core.publish_count == pc + 1u, "LATE: something other than NEUTRAL was published");
        for (int i = 0; i < 10; i++) {
            h.now += 1000000000u;
            o = h_tick(&h);
            OCB_CHECK(o.event == OCB_EVT_NONE && h.core.publish_count == pc + 1u, "backlog fired after LATE");
        }
    }
    /* An overdue END ends the piece with LATE, not PLAYBACK_DONE. */
    {
        harness h;
        h_init(&h, 8u, PERIOD);
        const uint32_t t[] = {1000u};
        const ocb_state s[] = {st_end()};
        const uint64_t t0 = start_with(&h, t, s, 1, 0u);
        h.now = t0 + 1000u + PERIOD + 1u;
        const ocb_core_out o = h_tick(&h);
        OCB_CHECK(o.event == OCB_EVT_LATE, "overdue END: event %u, wanted LATE", o.event);
    }
}

static void test_late_fallback(void)
{
    for (int via_setter = 0; via_setter <= 1; via_setter++) {
        for (uint32_t over = OCB_CORE_LATE_FALLBACK_US; over <= OCB_CORE_LATE_FALLBACK_US + 1u; over++) {
            harness h;
            h_init(&h, 8u, via_setter ? PERIOD : 0u);
            if (via_setter) {
                ocb_core_set_report_period(&h.core, 0u);
            }
            OCB_CHECK(h.core.late_us == 8000u, "fallback late_us %u", (unsigned)h.core.late_us);
            const uint32_t t[] = {1000u, 900000u};
            const ocb_state s[] = {st_buttons(OCB_BTN_A), st_end()};
            const uint64_t t0 = start_with(&h, t, s, 2, 0u);
            h.now = t0 + 1000u + over;
            const ocb_core_out o = h_tick(&h);
            const uint8_t want = over > 8000u ? OCB_EVT_LATE : OCB_EVT_NONE;
            OCB_CHECK(o.event == want, "fallback, overdue %u: event %u, wanted %u", (unsigned)over, o.event, want);
        }
    }
    harness h;
    h_init(&h, 8u, 0u);
    ocb_core_set_report_period(&h.core, 1234u);
    OCB_CHECK(h.core.late_us == 1234u, "set_report_period: %u", (unsigned)h.core.late_us);
}

static void test_lead_and_modes(void)
{
    harness h;
    h_init(&h, 8u, PERIOD);
    const uint32_t t[] = {0u, 10000u};
    const ocb_state s[] = {st_buttons(OCB_BTN_A), st_end()};
    const uint64_t t0 = start_with(&h, t, s, 2, 5000u);
    const uint32_t pc = h.core.publish_count;
    h.now = t0 - 1u;
    ocb_core_out o = h_tick(&h);
    OCB_CHECK(o.event == OCB_EVT_NONE && h.core.publish_count == pc, "applied during the lead");
    h.now = t0;
    o = h_tick(&h);
    OCB_CHECK(h.core.published.buttons == OCB_BTN_A, "t_us 0 not applied at t0");

    /* Nothing is ever applied in IDLE or DIRECT. */
    for (int direct = 0; direct <= 1; direct++) {
        harness g;
        h_init(&g, 8u, PERIOD);
        OCB_CHECK(h_queue_event(&g, 0u, st_buttons(OCB_BTN_A)).kind == OCB_CORE_REPLY_ACK, "prefill");
        if (direct) {
            OCB_CHECK(h_set_state(&g, st_buttons(OCB_BTN_B)).kind == OCB_CORE_REPLY_ACK, "DIRECT");
        }
        const uint32_t gpc = g.core.publish_count;
        for (int i = 0; i < 5; i++) {
            g.now += 100000000u;
            o = h_tick(&g);
            OCB_CHECK(o.event == OCB_EVT_NONE && g.core.publish_count == gpc, "%s: tick applied an event",
                      direct ? "DIRECT" : "IDLE");
        }
        OCB_CHECK(ocb_queue_count(&g.core.queue) == 1u, "%s: queue consumed", direct ? "DIRECT" : "IDLE");
    }
}

static void test_ordering(void)
{
    harness h;
    h_init(&h, 8u, PERIOD);
    OCB_CHECK(h_queue_event(&h, 0u, st_neutral()).kind == OCB_CORE_REPLY_ACK, "first t_us 0 refused");
    expect_nak(&h, h_queue_event(&h, 0u, st_neutral()), OCB_NAK_ORDER, "equal t_us");
    OCB_CHECK(h_queue_event(&h, 500u, st_neutral()).kind == OCB_CORE_REPLY_ACK, "500 refused");
    expect_nak(&h, h_queue_event(&h, 499u, st_neutral()), OCB_NAK_ORDER, "lower t_us");
    expect_nak(&h, h_queue_event(&h, 500u, st_neutral()), OCB_NAK_ORDER, "equal t_us again");
    expect_nak(&h, h_queue_event(&h, 0x80000000u, st_neutral()), OCB_NAK_ORDER, "t_us 2^31");
    expect_nak(&h, h_queue_event(&h, 0xFFFFFFFFu, st_neutral()), OCB_NAK_ORDER, "t_us 2^32-1");
    OCB_CHECK(h.core.last_t_us == 500u && ocb_queue_count(&h.core.queue) == 2u, "ORDER changed the baseline");
    OCB_CHECK(h_queue_event(&h, 0x7FFFFFFFu, st_neutral()).kind == OCB_CORE_REPLY_ACK, "t_us 2^31-1 refused");
    expect_nak(&h, h_queue_event(&h, 0x7FFFFFFFu, st_neutral()), OCB_NAK_ORDER, "t_us 2^31-1 twice");

    /* The ORDER NAK is semantic: it advances expected. */
    const uint8_t seq = h.seq;
    expect_nak(&h, h_queue_event(&h, 1u, st_neutral()), OCB_NAK_ORDER, "lower t_us");
    OCB_CHECK(h.core.expected == (uint8_t)(seq + 1u), "ORDER did not advance expected");

    /* The baseline resets on entering IDLE. */
    (void)h_stop(&h);
    OCB_CHECK(h_queue_event(&h, 1u, st_neutral()).kind == OCB_CORE_REPLY_ACK, "baseline not reset by STOP");

    /* A first event may be 2^31 - 1. */
    harness g;
    h_init(&g, 8u, PERIOD);
    OCB_CHECK(h_queue_event(&g, 0x7FFFFFFFu, st_neutral()).kind == OCB_CORE_REPLY_ACK, "first 2^31-1 refused");
    harness k;
    h_init(&k, 8u, PERIOD);
    expect_nak(&k, h_queue_event(&k, 0x80000000u, st_neutral()), OCB_NAK_ORDER, "first t_us 2^31");
}

static void test_one_event_per_tick(void)
{
    harness h;
    h_init(&h, 8u, 50000u);
    const uint32_t t[] = {1000u, 1001u, 1002u, 1003u};
    const ocb_state s[] = {st_buttons(OCB_BTN_A), st_buttons(OCB_BTN_X), st_buttons(OCB_BTN_Y), st_end()};
    const uint64_t t0 = start_with(&h, t, s, 4, 0u);
    h.now = t0 + 2000u; /* all four are due */
    const uint16_t want_buttons[] = {OCB_BTN_A, OCB_BTN_X, OCB_BTN_Y};
    for (int i = 0; i < 3; i++) {
        const uint32_t pc = h.core.publish_count;
        const ocb_core_out o = h_tick(&h);
        OCB_CHECK(o.event == OCB_EVT_NONE && h.core.publish_count == pc + 1u, "tick %d applied more than one", i);
        OCB_CHECK(h.core.published.buttons == want_buttons[i], "tick %d: buttons 0x%04x", i,
                  h.core.published.buttons);
        OCB_CHECK(ocb_queue_count(&h.core.queue) == (uint16_t)(3 - i), "tick %d: queue count", i);
    }
    const ocb_core_out o = h_tick(&h);
    OCB_CHECK(o.event == OCB_EVT_PLAYBACK_DONE, "last tick: event %u", o.event);
}

int main(void)
{
    test_end_applied();
    test_end_on_non_neutral();
    test_reserved_flags();
    test_nothing_after_end();
    test_underrun();
    test_late();
    test_late_fallback();
    test_lead_and_modes();
    test_ordering();
    test_one_event_per_tick();
    const int f = ocb_test_failures();
    if (f != 0) {
        fprintf(stderr, "test_core_end_late (F.4): %d failure(s)\n", f);
        return 1;
    }
    printf("test_core_end_late (F.4): ok\n");
    return 0;
}
