/*
 * SPDX-License-Identifier: MIT
 *
 * F.33 (story 2.3 D35): the published-state handoff from core 1 to core 0.
 * One thread plays both cores: round trips of many states, a read while a
 * write is in progress (never torn: false, the caller's last good state
 * kept), publication only when publish_count moves, the sequence counter
 * wrapping, and init to NEUTRAL.
 */
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "ocb_link/pubstate.h"
#include "ocb_proto/consts.h"
#include "ocb_test.h"

static int same(const ocb_state *a, const ocb_state *b)
{
    return a->buttons == b->buttons && a->hat == b->hat && a->lx == b->lx && a->ly == b->ly && a->rx == b->rx &&
           a->ry == b->ry && a->flags == b->flags;
}

int main(void)
{
    static ocb_pubstate ps;
    ocb_state got;
    ocb_pubstate_init(&ps);
    OCB_CHECK(ocb_pubstate_read(&ps, &got) && same(&got, &OCB_NEUTRAL) && ps.seq == 0u, "init is NEUTRAL");

    /* Round trips; D-pad down as the bench sends it: 00 00 04 80 80 80 80 00. */
    const ocb_state down = {0x0000u, OCB_HAT_DOWN, 0x80u, 0x80u, 0x80u, 0x80u, 0u};
    ocb_pubstate_write(&ps, &down);
    OCB_CHECK(ocb_pubstate_read(&ps, &got) && same(&got, &down), "D-pad down round trip");
    uint32_t x = 12345u;
    for (int i = 0; i < 100000; i++) {
        x = x * 1103515245u + 12345u;
        const ocb_state s = {(uint16_t)(x >> 8), (uint8_t)(x >> 3), (uint8_t)(x >> 11), (uint8_t)(x >> 17),
                             (uint8_t)(x >> 21), (uint8_t)(x >> 24), (uint8_t)(x >> 27)};
        ocb_pubstate_write(&ps, &s);
        if (!ocb_pubstate_read(&ps, &got) || !same(&got, &s)) {
            OCB_CHECK(false, "round trip %d", i);
            break;
        }
    }
    OCB_CHECK((ps.seq & 1u) == 0u, "seq even after writes");

    /* A read while core 1 is in the middle of a write: never the half-written
     * state; NEUTRAL and false after the tries. */
    const ocb_state a = {OCB_BTN_A, OCB_HAT_NEUTRAL, 0x80u, 0x80u, 0x80u, 0x80u, 0u};
    ocb_pubstate_write(&ps, &a);
    OCB_CHECK(ocb_pubstate_read(&ps, &got) && same(&got, &a), "A read");
    ocb_pubstate_write_begin(&ps);
    ps.buttons = OCB_BTN_B; /* half of a new state */
    /* Bench H.15 attempt 3, press 4: a failed read must keep the last good
     * state (A held), not drop to NEUTRAL. */
    OCB_CHECK(!ocb_pubstate_read(&ps, &got) && same(&got, &a), "read during a write: false, last good state kept");
    ocb_pubstate_write_end(&ps);
    OCB_CHECK(ocb_pubstate_read(&ps, &got) && got.buttons == OCB_BTN_B, "after the write ends");

    /* Publication only when the core's publish_count moves: a held state
     * is written once, so the reader rarely meets a write in progress. */
    ocb_pubstate_init(&ps);
    OCB_CHECK(ocb_pubstate_publish(&ps, &down, 7u), "first publish writes");
    const uint32_t seq_after = ps.seq;
    for (int i = 0; i < 1000; i++) {
        OCB_CHECK(!ocb_pubstate_publish(&ps, &down, 7u), "same count rewrote at pass %d", i);
    }
    OCB_CHECK(ps.seq == seq_after, "seq moved without a new publish");
    OCB_CHECK(ocb_pubstate_publish(&ps, &a, 8u) && ocb_pubstate_read(&ps, &got) && same(&got, &a), "new count");
    OCB_CHECK(ocb_pubstate_publish(&ps, &down, 0u), "a wrapped count still publishes");

    /* The counter wraps without a false in-progress reading. */
    ps.seq = 0xFFFFFFFEu;
    ocb_pubstate_write(&ps, &down);
    OCB_CHECK(ps.seq == 0u && ocb_pubstate_read(&ps, &got) && same(&got, &down), "seq wrap");

    const int f = ocb_test_failures();
    if (f != 0) {
        fprintf(stderr, "test_link_pubstate (F.33): %d failure(s)\n", f);
        return 1;
    }
    printf("test_link_pubstate (F.33): ok\n");
    return 0;
}
