/*
 * SPDX-License-Identifier: MIT
 *
 * F.29 (story 2.3 Task 5.2, AC5b): the median IN interval (D13); story
 * 2.5 D12: the minimum, 95th percentile and maximum of the same window and
 * the count of slow intervals (above 1.5 x the median), all reset by a
 * restart.
 */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include "ocb_p2/p2.h"
#include "ocb_test.h"
#include "p2_fixture.h"

static uint64_t g_t;

static void train(ocb_p2 *p, uint32_t step, int n)
{
    for (int i = 0; i < n; i++) {
        g_t += step;
        ocb_p2_interval_note(p, g_t);
        OCB_CHECK(p->iv.ops_note <= 1u, "note cost %u", p->iv.ops_note);
    }
    ocb_p2_tick(p, g_t);
    OCB_CHECK(p->iv.ops_median <= 401u, "median cost %u", p->iv.ops_median);
}

static bool near(uint32_t got, uint32_t want)
{
    return got + 25u >= want && got <= want + 25u;
}

static void spread_is(const ocb_p2 *p, uint32_t mn, uint32_t p95, uint32_t mx, const char *what)
{
    OCB_CHECK(ocb_p2_interval_min_us(p) == mn && ocb_p2_interval_p95_us(p) == p95 &&
                  ocb_p2_interval_max_us(p) == mx,
              "%s: min %u p95 %u max %u, wanted %u %u %u", what, ocb_p2_interval_min_us(p),
              ocb_p2_interval_p95_us(p), ocb_p2_interval_max_us(p), mn, p95, mx);
}

static void test_spread(ocb_p2 *p)
{
    /* Steady 8000 us: min = p95 = max = 8000, nothing slow. */
    fx_init(p);
    ocb_p2_interval_note(p, g_t);
    train(p, 8000, 63);
    spread_is(p, 8000u, 0u, 8000u, "under 64 samples: no p95 yet, min and max since the restart");
    train(p, 8000, 237);
    spread_is(p, 8000u, 8000u, 8000u, "steady 8000");
    OCB_CHECK(ocb_p2_interval_slow(p) == 0u, "steady: slow %u", ocb_p2_interval_slow(p));

    /* A missed poll: one 16000 us interval in the 8000 train is slow. */
    train(p, 16000, 1);
    OCB_CHECK(ocb_p2_interval_slow(p) == 1u, "16 ms in an 8 ms train: slow %u", ocb_p2_interval_slow(p));
    spread_is(p, 8000u, 8000u, 16000u, "with one missed poll");
    /* Jitter below 1.5 x is not slow: 11950 us (2 x 11950 = 23900 <= 24000)
     * and exactly 12000 (= 1.5 x); 12001 is slow. */
    train(p, 11950, 1);
    train(p, 12000, 1);
    OCB_CHECK(ocb_p2_interval_slow(p) == 1u, "jitter below 1.5 x counted: slow %u", ocb_p2_interval_slow(p));
    train(p, 12001, 1);
    OCB_CHECK(ocb_p2_interval_slow(p) == 2u, "12001 us: slow %u", ocb_p2_interval_slow(p));
    /* Short intervals (the host catching up) are not slow. */
    train(p, 1000, 1);
    OCB_CHECK(ocb_p2_interval_slow(p) == 2u, "a short interval counted as slow");

    /* p95 is a percentile, not the maximum: 95 x 1000, 4 x 3000, 1 x 5000
     * -> the 95th smallest of 100 is 1000. */
    fx_init(p);
    ocb_p2_interval_note(p, g_t);
    train(p, 1000, 95);
    train(p, 3000, 4);
    train(p, 5000, 1);
    spread_is(p, 1000u, 1000u, 5000u, "95/4/1 train");
    /* One more 3000: 96th smallest of 101 is k95 = ceil(95.95) = 96 -> 3000. */
    train(p, 3000, 1);
    spread_is(p, 1000u, 3000u, 5000u, "95/5/1 train");

    /* Mixed 80 % 1000, 20 % 2000: p95 2000. */
    fx_init(p);
    ocb_p2_interval_note(p, g_t);
    for (int r = 0; r < 60; r++) {
        train(p, 1000, 4);
        train(p, 2000, 1);
    }
    spread_is(p, 1000u, 2000u, 2000u, "mixed");
    OCB_CHECK(ocb_p2_interval_slow(p) > 0u, "2000 in a 1000 median train is slow");

    /* The window forgets: 256 x 8000 after an outlier. */
    fx_init(p);
    ocb_p2_interval_note(p, g_t);
    train(p, 8000, 100);
    train(p, 15000, 1);
    spread_is(p, 8000u, 8000u, 15000u, "outlier in the window");
    train(p, 8000, 256);
    /* D35: p95 is the window's, min and max cover the same span as slow
     * (since the restart), so the outlier stays the maximum. */
    spread_is(p, 8000u, 8000u, 15000u, "outlier left the window, still the maximum since the restart");
    OCB_CHECK(ocb_p2_interval_slow(p) == 1u, "slow is since the restart, not the window: %u",
              ocb_p2_interval_slow(p));

    /* Review fix: a 30 ms stall (above the 20 ms window limit) while a
     * median is published is slow and is the reported maximum, without
     * entering the window (the median and p95 do not move); a backwards
     * clock step is no stall. */
    {
        const uint32_t slow0 = ocb_p2_interval_slow(p);
        const uint16_t count0 = p->iv.count;
        train(p, 30000, 1);
        OCB_CHECK(ocb_p2_interval_slow(p) == slow0 + 1u, "30 ms stall not slow: %u", ocb_p2_interval_slow(p));
        OCB_CHECK(ocb_p2_interval_max_us(p) == 30000u, "30 ms stall not the max: %u", ocb_p2_interval_max_us(p));
        OCB_CHECK(p->iv.count == count0 && ocb_p2_interval_p95_us(p) == 8000u && ocb_p2_report_period_us(p) == 8000u,
                  "the stall entered the window");
        train(p, 8000, 300);
        OCB_CHECK(ocb_p2_interval_max_us(p) == 30000u, "the stall max is kept since the restart");
        g_t -= 5000u;
        ocb_p2_interval_note(p, g_t);
        OCB_CHECK(ocb_p2_interval_slow(p) == slow0 + 1u, "a backwards step counted as a stall");
    }

    /* Restart zeroes all. */
    ocb_p2_restart(p);
    spread_is(p, 0u, 0u, 0u, "after restart");
    OCB_CHECK(p->iv.run_max_us == 0u && p->iv.run_min_us == 0u, "restart: the span not cleared");
    OCB_CHECK(ocb_p2_interval_slow(p) == 0u, "restart: slow %u", ocb_p2_interval_slow(p));

    /* slow saturates. */
    p->iv.slow = UINT32_MAX;
    p->iv.median_us = 8000u;
    p->iv.have_last = true;
    p->iv.last_us = (uint32_t)g_t;
    g_t += 19000u;
    ocb_p2_interval_note(p, g_t);
    OCB_CHECK(ocb_p2_interval_slow(p) == UINT32_MAX, "slow saturates");
}

/* Story 2.5 D35, bench H.18: "IN max 8000, slow 5" was possible because
 * max covered the last 256 intervals and slow the whole time since the
 * restart. Now a slow interval is always within [min, max]. */
static void test_slow_within_span(ocb_p2 *p)
{
    fx_init(p);
    ocb_p2_interval_note(p, g_t);
    train(p, 8000, 300);
    for (int i = 0; i < 5; i++) {
        train(p, 8000, 50);
        train(p, 16000, 1); /* a missed poll */
        train(p, 4000, 1);  /* the next one catches up */
    }
    train(p, 8000, 300); /* every slow interval has left the window */
    OCB_CHECK(ocb_p2_interval_slow(p) == 5u, "slow %u", ocb_p2_interval_slow(p));
    spread_is(p, 4000u, 8000u, 16000u, "the bench shape: max and min keep the 5 missed polls");
    OCB_CHECK(p->iv.max_us == 8000u, "the window alone shows 8000 (the bench's in_max_us)");
    /* Raw microseconds, not bin centres. */
    train(p, 8013, 1);
    OCB_CHECK(ocb_p2_interval_max_us(p) == 16000u && ocb_p2_interval_min_us(p) == 4000u, "raw values kept");
    train(p, 3999, 1);
    OCB_CHECK(ocb_p2_interval_min_us(p) == 3999u, "raw min %u", ocb_p2_interval_min_us(p));
}

int main(void)
{
    ocb_p2 p;

    /* Steady 1000 us and 8000 us. */
    fx_init(&p);
    g_t = 5000;
    ocb_p2_interval_note(&p, g_t);
    train(&p, 1000, 300);
    OCB_CHECK(near(ocb_p2_report_period_us(&p), 1000), "1000 us train: %u", ocb_p2_report_period_us(&p));
    fx_init(&p);
    ocb_p2_interval_note(&p, g_t);
    train(&p, 8000, 300);
    OCB_CHECK(near(ocb_p2_report_period_us(&p), 8000), "8000 us train: %u", ocb_p2_report_period_us(&p));

    /* Fewer than 64 samples: 0; the 64th publishes. */
    fx_init(&p);
    ocb_p2_interval_note(&p, g_t);
    train(&p, 1000, 63);
    OCB_CHECK(ocb_p2_report_period_us(&p) == 0u, "63 samples must give 0");
    train(&p, 1000, 1);
    OCB_CHECK(near(ocb_p2_report_period_us(&p), 1000), "64 samples: %u", ocb_p2_report_period_us(&p));

    /* Mixed 80 % 1000, 20 % 2000: the median (1000), not the mean (1200). */
    fx_init(&p);
    ocb_p2_interval_note(&p, g_t);
    for (int r = 0; r < 60; r++) {
        train(&p, 1000, 4);
        train(&p, 2000, 1);
    }
    OCB_CHECK(near(ocb_p2_report_period_us(&p), 1000), "mixed train: %u", ocb_p2_report_period_us(&p));

    /* One 15 ms outlier does not move it. */
    train(&p, 15000, 1);
    OCB_CHECK(near(ocb_p2_report_period_us(&p), 1000), "outlier moved it: %u", ocb_p2_report_period_us(&p));

    /* The window is the last 256: 256 samples of 4000 replace the 1000s. */
    train(&p, 4000, 256);
    OCB_CHECK(near(ocb_p2_report_period_us(&p), 4000), "window: %u", ocb_p2_report_period_us(&p));

    /* A 25 ms gap is dropped and not counted. */
    fx_init(&p);
    ocb_p2_interval_note(&p, g_t);
    train(&p, 1000, 10);
    const uint16_t before = p.iv.count;
    train(&p, 25000, 1);
    OCB_CHECK(p.iv.count == before, "25 ms gap counted");
    train(&p, 1000, 60);
    OCB_CHECK(near(ocb_p2_report_period_us(&p), 1000), "after the gap: %u", ocb_p2_report_period_us(&p));

    /* Exactly 20 ms is counted, 20 ms + 1 us is not. */
    fx_init(&p);
    ocb_p2_interval_note(&p, g_t);
    train(&p, 20000, 1);
    OCB_CHECK(p.iv.count == 1u, "20 ms not counted");
    train(&p, 20001, 1);
    OCB_CHECK(p.iv.count == 1u, "20.001 ms counted");

    /* Wrap of a 32-bit clock and of a 64-bit clock. */
    fx_init(&p);
    g_t = 0xFFFFFFFFull - 50000u;
    ocb_p2_interval_note(&p, g_t);
    train(&p, 1000, 100);
    OCB_CHECK(near(ocb_p2_report_period_us(&p), 1000) && p.iv.count == 100u, "32-bit wrap: %u (%u)",
              ocb_p2_report_period_us(&p), p.iv.count);
    {
        /* A 32-bit clock passed as small values after the wrap. */
        ocb_p2 q;
        fx_init(&q);
        uint32_t t32 = 0xFFFFF000u;
        ocb_p2_interval_note(&q, t32);
        for (int i = 0; i < 100; i++) {
            t32 += 1000u;
            ocb_p2_interval_note(&q, t32);
        }
        ocb_p2_tick(&q, 0);
        OCB_CHECK(near(ocb_p2_report_period_us(&q), 1000) && q.iv.count == 100u, "32-bit clock values wrap");
    }
    fx_init(&p);
    g_t = 0xFFFFFFFFFFFFFFFFull - 30000u;
    ocb_p2_interval_note(&p, g_t);
    train(&p, 1000, 100);
    OCB_CHECK(near(ocb_p2_report_period_us(&p), 1000) && p.iv.count == 100u, "64-bit wrap: %u",
              ocb_p2_report_period_us(&p));

    /* A backwards clock is ignored. */
    fx_init(&p);
    g_t = 1000000;
    ocb_p2_interval_note(&p, g_t);
    train(&p, 1000, 10);
    g_t -= 5000;
    ocb_p2_interval_note(&p, g_t);
    OCB_CHECK(p.iv.count == 10u, "backwards step counted");

    /* Restart gives 0 and starts a new window. */
    fx_init(&p);
    ocb_p2_interval_note(&p, g_t);
    train(&p, 1000, 100);
    ocb_p2_restart(&p);
    OCB_CHECK(ocb_p2_report_period_us(&p) == 0u && p.iv.count == 0u, "restart clears");
    ocb_p2_interval_note(&p, g_t + 100000u); /* the first note after a restart is a start point */
    g_t += 100000u;
    train(&p, 8000, 63);
    OCB_CHECK(ocb_p2_report_period_us(&p) == 0u, "new window under 64");
    train(&p, 8000, 1);
    OCB_CHECK(near(ocb_p2_report_period_us(&p), 8000), "new window: %u", ocb_p2_report_period_us(&p));

    /* Bin edges: 1024 us -> 1000, 1025 us -> 1050 (centred 50 us bins). */
    fx_init(&p);
    ocb_p2_interval_note(&p, g_t);
    train(&p, 1024, 64);
    OCB_CHECK(ocb_p2_report_period_us(&p) == 1000u, "1024 -> %u", ocb_p2_report_period_us(&p));
    fx_init(&p);
    ocb_p2_interval_note(&p, g_t);
    train(&p, 1025, 64);
    OCB_CHECK(ocb_p2_report_period_us(&p) == 1050u, "1025 -> %u", ocb_p2_report_period_us(&p));

    /* Tick without new samples does no work. */
    ocb_p2_tick(&p, g_t);
    OCB_CHECK(p.iv.ops_median <= 401u, "idle tick");

    test_spread(&p);
    test_slow_within_span(&p);

    return ocb_test_failures() == 0 ? 0 : 1;
}
