/*
 * SPDX-License-Identifier: MIT
 *
 * Host test F.5 (story 2.4): HELLO and STOP are sync points. From PLAYING
 * (mid-piece, queue non-empty, a note held) and from IDLE and DIRECT, with
 * any seq including the u8 wrap: NEUTRAL published, queue cleared, IDLE,
 * latch cleared, expected = seq + 1, reply ACK (STOP) or HELLO, no EVENT
 * (AD-6, AD-8, D2, D4).
 */
#include <stdint.h>
#include <stdio.h>

#include "core_fixture.h"

static void check_sync(harness *h, int use_hello, uint8_t seq, const char *from, bool body)
{
    const char *fn = use_hello ? "HELLO" : "STOP";
    h->seq = seq;
    ocb_core_out o;
    if (use_hello) {
        o = h_hello(h);
    } else if (body) {
        const uint8_t junk[5] = {1, 2, 3, 4, 5};
        o = h_raw(h, OCB_MSG_STOP, junk, (uint8_t)sizeof junk);
    } else {
        o = h_stop(h);
    }
    const ocb_core_reply_kind want = use_hello ? OCB_CORE_REPLY_HELLO : OCB_CORE_REPLY_ACK;
    OCB_CHECK(o.kind == want, "%s from %s seq %u: reply %d", fn, from, seq, (int)o.kind);
    OCB_CHECK(o.event == OCB_EVT_NONE, "%s from %s seq %u: event %u", fn, from, seq, o.event);
    OCB_CHECK(h->core.mode == OCB_MODE_IDLE, "%s from %s seq %u: mode %s", fn, from, seq, mode_name(h->core.mode));
    OCB_CHECK(h_published_neutral(h), "%s from %s seq %u: not NEUTRAL", fn, from, seq);
    OCB_CHECK(ocb_queue_count(&h->core.queue) == 0u, "%s from %s seq %u: queue not cleared", fn, from, seq);
    OCB_CHECK(!h->core.aborted, "%s from %s seq %u: latch not cleared", fn, from, seq);
    const uint8_t want_expected = (uint8_t)(seq + 1u);
    OCB_CHECK(h->core.expected == want_expected, "%s from %s seq %u: expected %u, wanted %u", fn, from, seq,
              h->core.expected, want_expected);
}

static void test_sync_points(void)
{
    const uint8_t seqs[] = {0u, 1u, 127u, 128u, 255u};
    const char *const setups[] = {"PLAYING mid-piece", "PLAYING aborted", "IDLE", "DIRECT"};
    for (int use_hello = 0; use_hello <= 1; use_hello++) {
        for (size_t si = 0; si < sizeof seqs; si++) {
            for (int setup = 0; setup < 4; setup++) {
                harness h;
                h_init(&h, 16u, 1000u);
                h.seq = 200u; /* setup frames use other seqs */
                if (setup == 0) {
                    h_enter_mode(&h, OCB_MODE_PLAYING);
                    h.now += 1000000u;
                    (void)h_tick(&h); /* note A held, one event still queued */
                    OCB_CHECK(h.core.published.buttons == OCB_BTN_A && ocb_queue_count(&h.core.queue) == 1u,
                              "setup: mid-piece");
                } else if (setup == 1) {
                    h_enter_mode(&h, OCB_MODE_PLAYING);
                    (void)h_abort(&h, OCB_ABORT_LINK_LOST);
                } else if (setup == 2) {
                    h_enter_mode(&h, OCB_MODE_IDLE);
                } else {
                    h_enter_mode(&h, OCB_MODE_DIRECT);
                }
                check_sync(&h, use_hello, seqs[si], setups[setup], false);
            }
        }
    }
    OCB_CHECK((uint8_t)(255u + 1u) == 0u, "u8 wrap");
}

static void test_stop_with_body(void)
{
    /* A malformed STOP body still stops (fail safe, D4). */
    harness h;
    h_init(&h, 16u, 1000u);
    h_enter_mode(&h, OCB_MODE_PLAYING);
    h.now += 1000000u;
    (void)h_tick(&h);
    check_sync(&h, 0, 9u, "PLAYING (STOP with a 5-byte body)", true);

    /* HELLO with an empty or long body also acts (version checks are 2.6). */
    harness g;
    h_init(&g, 16u, 1000u);
    h_enter_mode(&g, OCB_MODE_DIRECT);
    g.seq = 30u;
    ocb_core_out o = h_raw(&g, OCB_MSG_HELLO, NULL, 0u);
    OCB_CHECK(o.kind == OCB_CORE_REPLY_HELLO && g.core.mode == OCB_MODE_IDLE && g.core.expected == 31u,
              "HELLO with an empty body");
    uint8_t long_body[OCB_MAX_BODY];
    memset(long_body, 0xAB, sizeof long_body);
    h_enter_mode(&g, OCB_MODE_PLAYING);
    g.seq = 255u;
    o = h_raw(&g, OCB_MSG_HELLO, long_body, (uint8_t)sizeof long_body);
    OCB_CHECK(o.kind == OCB_CORE_REPLY_HELLO && g.core.mode == OCB_MODE_IDLE && g.core.expected == 0u,
              "HELLO with a 255-byte body");
}

int main(void)
{
    test_sync_points();
    test_stop_with_body();
    const int f = ocb_test_failures();
    if (f != 0) {
        fprintf(stderr, "test_core_sync (F.5): %d failure(s)\n", f);
        return 1;
    }
    printf("test_core_sync (F.5): ok\n");
    return 0;
}
