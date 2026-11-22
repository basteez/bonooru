/*
 * SPDX-License-Identifier: MIT
 *
 * F.29 (story 2.3 Task 5.2, AC5b): the median IN interval (D13).
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

    return ocb_test_failures() == 0 ? 0 : 1;
}
