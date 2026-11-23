/*
 * SPDX-License-Identifier: MIT
 *
 * F.34 (story 2.5 D6, D7): the liveness rules of the two controller cores,
 * on a fake clock. The stale boundary (20000 us alive, 20001 stale), a
 * counter moving every 19999 us never goes stale, the 32-bit counter wrap,
 * recovery (not latched), the first-observation grace window, the AC4 sum
 * of the constants re-added by hand, the decision functions and the
 * scratch encoding of the reboot cause and count.
 */
#include <stdint.h>
#include <stdio.h>

#include "ocb_core/live.h"
#include "ocb_test.h"

static void test_boundary(void)
{
    ocb_live_peer p;
    ocb_live_peer_init(&p, 5u, 1000u);
    OCB_CHECK(!ocb_live_peer_stale(&p, 6u, 2000u), "a change is alive");
    OCB_CHECK(!ocb_live_peer_stale(&p, 6u, 2000u + 20000u), "exactly 20000 us unchanged is alive");
    OCB_CHECK(ocb_live_peer_stale(&p, 6u, 2000u + 20001u), "20001 us unchanged is stale");
    OCB_CHECK(ocb_live_peer_stale(&p, 6u, 2000u + 50000u), "still stale later");
    /* Not latched: the peer resumes. */
    OCB_CHECK(!ocb_live_peer_stale(&p, 7u, 2000u + 50001u), "alive again after a change");
    OCB_CHECK(!ocb_live_peer_stale(&p, 7u, 2000u + 50001u + 20000u), "and the window restarts");
    /* A clock that goes back is no time. */
    OCB_CHECK(!ocb_live_peer_stale(&p, 7u, 10u), "clock back: not stale");
}

static void test_steady(void)
{
    ocb_live_peer p;
    uint64_t t = 0u;
    uint32_t c = 0u;
    ocb_live_peer_init(&p, c, t);
    int stale = 0;
    for (int i = 0; i < 1000000; i++) {
        t += 19999u;
        c++;
        stale += ocb_live_peer_stale(&p, c, t) ? 1 : 0;
        /* A second look at the same counter just before the next step. */
        stale += ocb_live_peer_stale(&p, c, t + 19998u) ? 1 : 0;
    }
    OCB_CHECK(stale == 0, "a counter moving every 19999 us went stale %d times", stale);
}

static void test_wrap(void)
{
    ocb_live_peer p;
    ocb_live_peer_init(&p, 0xFFFFFFFEu, 0u);
    OCB_CHECK(!ocb_live_peer_stale(&p, 0xFFFFFFFFu, 15000u), "before the wrap");
    OCB_CHECK(!ocb_live_peer_stale(&p, 0u, 30000u), "the wrap to 0 is a change");
    OCB_CHECK(!ocb_live_peer_stale(&p, 1u, 45000u), "after the wrap");
    OCB_CHECK(ocb_live_peer_stale(&p, 1u, 45000u + 20001u), "a stall after the wrap is seen");
}

static void test_grace(void)
{
    ocb_live_peer p;
    ocb_live_peer_init(&p, 42u, 100000u);
    OCB_CHECK(!ocb_live_peer_moved(&p), "not moved at init");
    OCB_CHECK(!ocb_live_peer_stale(&p, 42u, 100000u + 20000u), "grace: 20000 us without a first change");
    OCB_CHECK(ocb_live_peer_stale(&p, 42u, 100000u + 20001u), "grace expired: never moved is stale");
    OCB_CHECK(ocb_live_core1_cause(&p) == OCB_LIVE_CAUSE_BOOT_NOT_ALIVE, "never moved: BOOT_NOT_ALIVE");
    OCB_CHECK(!ocb_live_peer_stale(&p, 43u, 130000u) && ocb_live_peer_moved(&p), "moved");
    OCB_CHECK(ocb_live_peer_stale(&p, 43u, 130000u + 20001u), "stale after moving");
    OCB_CHECK(ocb_live_core1_cause(&p) == OCB_LIVE_CAUSE_CORE1_STALE, "moved then stale: CORE1_STALE");
}

static void test_constants(void)
{
    /* Hand-written: 20 ms + 60 ms + 5 ms. */
    OCB_CHECK(OCB_LIVE_STALE_US == 20000u, "stale %u", OCB_LIVE_STALE_US);
    OCB_CHECK(OCB_WDT_TIMEOUT_MS == 60u, "watchdog %u", OCB_WDT_TIMEOUT_MS);
    OCB_CHECK(OCB_LIVE_LOOP_BOUND_US == 5000u, "loop bound %u", OCB_LIVE_LOOP_BOUND_US);
    const uint32_t sum = 20000u + 60u * 1000u + 5000u;
    OCB_CHECK(OCB_LIVE_WORST_CASE_US == sum, "worst case %u, by hand %u", OCB_LIVE_WORST_CASE_US, sum);
    OCB_CHECK(sum <= 100000u, "AC4: %u us > 100 ms", sum);
    OCB_CHECK(OCB_LIVE_REBOOT_LIMIT_US == 100000u, "AC4 limit");
}

static void test_rules(void)
{
    OCB_CHECK(ocb_live_core0_may_feed(false), "core 1 fresh: feed");
    OCB_CHECK(!ocb_live_core0_may_feed(true), "core 1 stale: stop feeding");
    OCB_CHECK(ocb_live_core1_must_reboot(true), "core 0 stale: reboot");
    OCB_CHECK(!ocb_live_core1_must_reboot(false), "core 0 fresh: no reboot");
}

static void test_scratch(void)
{
    const uint8_t causes[4] = {OCB_LIVE_CAUSE_NONE, OCB_LIVE_CAUSE_CORE0_STALE, OCB_LIVE_CAUSE_CORE1_STALE,
                               OCB_LIVE_CAUSE_BOOT_NOT_ALIVE};
    for (int i = 0; i < 4; i++) {
        for (int j = i + 1; j < 4; j++) {
            OCB_CHECK(causes[i] != causes[j], "causes %d and %d equal", i, j);
        }
        OCB_CHECK(ocb_live_cause_decode(ocb_live_cause_encode(causes[i])) == causes[i], "cause %u round trip",
                  causes[i]);
    }
    OCB_CHECK(ocb_live_cause_encode(OCB_LIVE_CAUSE_CORE1_STALE) == 0xB0E50002u, "encoding by hand");
    OCB_CHECK(ocb_live_cause_decode(0u) == OCB_LIVE_CAUSE_NONE, "power-on zero: NONE");
    OCB_CHECK(ocb_live_cause_decode(0x12340002u) == OCB_LIVE_CAUSE_NONE, "wrong magic: NONE");
    OCB_CHECK(ocb_live_cause_decode(0xB0E50102u) == OCB_LIVE_CAUSE_NONE, "bits 8..15 set: NONE");
    OCB_CHECK(ocb_live_cause_decode(0xB0E50004u) == OCB_LIVE_CAUSE_NONE, "cause above max: NONE");
    OCB_CHECK(ocb_live_cause_decode(0xFFFFFFFFu) == OCB_LIVE_CAUSE_NONE, "all ones: NONE");
    for (uint32_t v = 0u; v < 256u; v++) {
        if (ocb_live_scratch_decode(ocb_live_scratch_encode((uint8_t)v)) != v) {
            OCB_CHECK(0, "count %u round trip", v);
        }
    }
    OCB_CHECK(ocb_live_scratch_decode(0u) == 0u, "power-on count 0");
}

/* Story 2.5 D31: the stall-to-reset span word and the observer's age. */
static void test_span(void)
{
    OCB_CHECK(ocb_live_span_encode(21340u) == 0xB0E50856u, "21340 us -> 2134 (0x856) units: %08x",
              ocb_live_span_encode(21340u));
    OCB_CHECK(ocb_live_span_decode(ocb_live_span_encode(21340u)) == 21340u, "round trip 21340");
    OCB_CHECK(ocb_live_span_decode(ocb_live_span_encode(80009u)) == 80000u, "truncated to 10 us");
    OCB_CHECK(ocb_live_span_decode(ocb_live_span_encode(0xFFFFFFFFu)) == 655350u, "saturates at 655350 us");
    OCB_CHECK(ocb_live_span_decode(0u) == 0u && ocb_live_span_decode(0x12340855u) == 0u, "no magic: 0");
    ocb_live_peer p;
    ocb_live_peer_init(&p, 1u, 5000u);
    OCB_CHECK(ocb_live_peer_since_us(&p, 25001u) == 20001u, "since init");
    (void)ocb_live_peer_stale(&p, 2u, 30000u);
    OCB_CHECK(ocb_live_peer_since_us(&p, 30500u) == 500u, "since the last change");
    OCB_CHECK(ocb_live_peer_since_us(&p, 100u) == 0u, "clock back: 0");
}

int main(void)
{
    test_span();
    test_boundary();
    test_steady();
    test_wrap();
    test_grace();
    test_constants();
    test_rules();
    test_scratch();
    const int f = ocb_test_failures();
    if (f != 0) {
        fprintf(stderr, "test_live (F.34): %d failure(s)\n", f);
        return 1;
    }
    printf("test_live (F.34): ok\n");
    return 0;
}
