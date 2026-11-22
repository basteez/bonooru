/*
 * SPDX-License-Identifier: MIT
 *
 * F.36 (story 2.5 D11, Task 11.2): the scheduler's lateness statistics and
 * the loop-time helper, on a fake clock, against hand-written numbers.
 * Events applied on time, 49/50 us (first edge), 99/100, 249/250,
 * 999/1000, 3999/4000 us late, exactly late_us late (applied, counted) and
 * late_us + 1 (LATE abort, not counted); the statistics reset at START and
 * survive PLAYBACK_DONE; 10^5 events do not overflow late_sum_us and the
 * sum saturates exactly at its documented limit; the wrap-safe span.
 */
#include <stdint.h>
#include <stdio.h>

#include "core_fixture.h"
#include "ocb_core/timing.h"

#define PERIOD 8000u
#define SPACING 20000u

static void test_buckets_and_late(void)
{
    harness h;
    h_init(&h, 64u, PERIOD);
    /* Lateness of each applied event, us (hand-written). */
    static const uint32_t late[] = {0u, 49u, 50u, 99u, 100u, 249u, 250u, 999u, 1000u, 3999u, 4000u, 8000u};
    const size_t n = sizeof late / sizeof late[0];
    for (size_t i = 0; i <= n; i++) {
        const ocb_core_out o = h_queue_event(&h, (uint32_t)(SPACING * (i + 1u)), st_buttons(OCB_BTN_A));
        OCB_CHECK(o.kind == OCB_CORE_REPLY_ACK, "setup: event %zu refused", i);
    }
    OCB_CHECK(h_start(&h, 0u).kind == OCB_CORE_REPLY_ACK, "setup: START");
    const uint64_t t0 = h.now;
    OCB_CHECK(h.core.timing.n == 0u, "reset at START");
    for (size_t i = 0; i < n; i++) {
        h.now = t0 + SPACING * (i + 1u) + late[i];
        const ocb_core_out o = h_tick(&h);
        OCB_CHECK(o.event == OCB_EVT_NONE && h.core.mode == OCB_MODE_PLAYING, "event %zu (late %u) not applied", i,
                  late[i]);
    }
    /* The 13th event, late_us + 1 late: LATE, not counted. */
    h.now = t0 + SPACING * (n + 1u) + PERIOD + 1u;
    const ocb_core_out o = h_tick(&h);
    OCB_CHECK(o.event == OCB_EVT_LATE, "late_us + 1: event %u, wanted LATE", o.event);

    const ocb_core_timing_stats *t = &h.core.timing;
    OCB_CHECK(t->n == 12u, "n %u, wanted 12", t->n);
    OCB_CHECK(t->late_max_us == 8000u, "max %u, wanted 8000", t->late_max_us);
    /* 0+49+50+99+100+249+250+999+1000+3999+4000+8000 = 18795 */
    OCB_CHECK(t->late_sum_us == 18795u, "sum %u, wanted 18795", t->late_sum_us);
    static const uint32_t want[OCB_CORE_LATE_BUCKETS] = {2u, 2u, 2u, 2u, 2u, 2u};
    for (int b = 0; b < OCB_CORE_LATE_BUCKETS; b++) {
        OCB_CHECK(t->hist[b] == want[b], "bucket %d: %u, wanted %u", b, t->hist[b], want[b]);
    }
    OCB_CHECK(h.core.mode == OCB_MODE_IDLE && t->n == 12u, "statistics survive the LATE abort");
}

static void test_reset_and_done(void)
{
    harness h;
    h_init(&h, 8u, PERIOD);
    (void)h_queue_event(&h, 1000u, st_buttons(OCB_BTN_X));
    (void)h_queue_event(&h, 2000u, st_end());
    OCB_CHECK(h_start(&h, 500u).kind == OCB_CORE_REPLY_ACK, "START");
    const uint64_t t0 = h.now + 500u;
    h.now = t0 + 1000u + 7u;
    (void)h_tick(&h);
    h.now = t0 + 2000u + 30u;
    const ocb_core_out o = h_tick(&h);
    OCB_CHECK(o.event == OCB_EVT_PLAYBACK_DONE, "PLAYBACK_DONE");
    const ocb_core_timing_stats *t = &h.core.timing;
    OCB_CHECK(t->n == 2u && t->late_max_us == 30u && t->late_sum_us == 37u && t->hist[0] == 2u,
              "after DONE: n %u max %u sum %u b0 %u", t->n, t->late_max_us, t->late_sum_us, t->hist[0]);
    /* Still there in IDLE, after more ticks and a STOP. */
    h.now += 100000u;
    (void)h_tick(&h);
    (void)h_stop(&h);
    OCB_CHECK(t->n == 2u && t->late_sum_us == 37u, "statistics survive DONE, ticks and STOP");
    /* A new START resets them. */
    (void)h_queue_event(&h, 1000u, st_buttons(OCB_BTN_Y));
    OCB_CHECK(h_start(&h, 0u).kind == OCB_CORE_REPLY_ACK, "second START");
    OCB_CHECK(t->n == 0u && t->late_max_us == 0u && t->late_sum_us == 0u && t->hist[0] == 0u, "reset at START");
    /* A refused START does not reset (BUSY while playing). */
    h.now += 1000u + 3u;
    (void)h_tick(&h);
    OCB_CHECK(h_start(&h, 0u).kind == OCB_CORE_REPLY_NAK, "START refused");
    OCB_CHECK(t->n == 1u && t->late_sum_us == 3u, "a refused START keeps the statistics");
}

static void test_saturation(void)
{
    ocb_core_timing_stats t;
    ocb_core_timing_reset(&t);
    for (int i = 0; i < 100000; i++) {
        ocb_core_timing_note(&t, 8000u);
    }
    OCB_CHECK(t.n == 100000u && t.late_sum_us == 800000000u && t.hist[5] == 100000u, "10^5 events: sum %u",
              t.late_sum_us);
    /* The documented limit: 65537 events at 65535 us = 2^32 - 1 exactly. */
    ocb_core_timing_reset(&t);
    for (int i = 0; i < 65537; i++) {
        ocb_core_timing_note(&t, 65535u);
    }
    OCB_CHECK(t.late_sum_us == 0xFFFFFFFFu, "65537 x 65535 = 2^32 - 1: %u", t.late_sum_us);
    ocb_core_timing_note(&t, 1u);
    OCB_CHECK(t.late_sum_us == 0xFFFFFFFFu, "saturated, no wrap: %u", t.late_sum_us);
    OCB_CHECK(t.n == 65538u && t.late_max_us == 65535u && t.hist[0] == 1u, "n %u max %u", t.n, t.late_max_us);
    t.n = 0xFFFFFFFFu;
    t.hist[0] = 0xFFFFFFFFu;
    ocb_core_timing_note(&t, 0u);
    OCB_CHECK(t.n == 0xFFFFFFFFu && t.hist[0] == 0xFFFFFFFFu, "n and buckets saturate");
}

static void test_span(void)
{
    OCB_CHECK(ocb_core_span_us(100u, 150u) == 50u, "plain span");
    OCB_CHECK(ocb_core_span_us(0xFFFFFFF0u, 0x10u) == 0x20u, "span across the wrap");
    OCB_CHECK(ocb_core_span_us(7u, 7u) == 0u, "zero span");
    uint32_t m = 0u;
    ocb_core_max_u32(&m, 12u);
    ocb_core_max_u32(&m, 5u);
    ocb_core_max_u32(&m, 40u);
    ocb_core_max_u32(&m, 39u);
    OCB_CHECK(m == 40u, "max %u", m);
}

int main(void)
{
    test_buckets_and_late();
    test_reset_and_done();
    test_saturation();
    test_span();
    const int f = ocb_test_failures();
    if (f != 0) {
        fprintf(stderr, "test_core_timing (F.36): %d failure(s)\n", f);
        return 1;
    }
    printf("test_core_timing (F.36): ok\n");
    return 0;
}
