/*
 * SPDX-License-Identifier: MIT
 *
 * F.37 (story 2.5 D8, D10): the built-in test timeline and its loader.
 *  - the generated table: 158 events (hand-counted from the Story 1.6
 *    trial file), strictly increasing t_us below 2^31, inside the AD-9
 *    allow-list, the last event NEUTRAL|END and no other END;
 *  - the loader through the real link: a session is opened, the table is
 *    loaded into the real core (capacity 512), the queue holds the whole
 *    table, `expected` is unchanged, and a START with the host's next seq
 *    is ACKed (not NAK SEQ);
 *  - the melody plays on a fake clock ticked every 50 us: the applied
 *    states in order equal the table through the AD-9 mask, PLAYBACK_DONE
 *    is the only event, n equals the table length, lateness max < 50 us;
 *  - a 30 ms stall in the middle aborts LATE and fires no backlog;
 *  - loading twice, into a PLAYING core or into a latched core is refused
 *    and changes nothing; a table with a bad row is rejected by the real
 *    ocb_core_frame path and leaves an empty IDLE queue.
 */
#include <stdint.h>
#include <stdio.h>

#include "link_fixture.h"
#include "ocb_core/mask.h"
#include "ocb_core/testtl.h"

#define TL_LEN 158u           /* hand-counted rows of ode-to-joy-90bpm.txt */
#define TL_FIRST_NOTE_US 500000u /* frame 30 */
#define TL_LAST_US 44166667u     /* frame 2650: (2650 * 10^6 + 30) / 60 */

static ocb_core_env env_of(const lh *h)
{
    ocb_core_env e;
    e.now_us = h->now;
    e.ready = ocb_link_console_ready(&h->link, h->now);
    e.link_ok = ocb_link_link_ok(&h->link);
    return e;
}

static bool same(const ocb_state *a, const ocb_state *b)
{
    return a->buttons == b->buttons && a->hat == b->hat && a->lx == b->lx && a->ly == b->ly && a->rx == b->rx &&
           a->ry == b->ry && a->flags == b->flags;
}

static void test_table(void)
{
    const size_t n = ocb_testtl_count();
    OCB_CHECK(n == TL_LEN, "table length %zu, wanted %u", n, TL_LEN);
    OCB_CHECK(ocb_testtl_get(n) == NULL, "get past the end");
    if (n == 0u) {
        return;
    }
    OCB_CHECK(ocb_testtl_get(1u)->t_us == TL_FIRST_NOTE_US && ocb_testtl_get(1u)->state.buttons == OCB_BTN_Y,
              "first note: Y at 500000 us");
    int ends = 0;
    for (size_t i = 0; i < n; i++) {
        const ocb_testtl_event *e = ocb_testtl_get(i);
        if (i > 0u && e->t_us <= ocb_testtl_get(i - 1u)->t_us) {
            OCB_CHECK(0, "event %zu not after %zu", i, i - 1u);
        }
        OCB_CHECK(e->t_us < 0x80000000u, "event %zu t_us %u not below 2^31", i, e->t_us);
        OCB_CHECK((e->state.buttons & (uint16_t)~OCB_CORE_ALLOWED_BUTTONS) == 0u, "event %zu buttons 0x%04x", i,
                  e->state.buttons);
        OCB_CHECK(e->state.hat == OCB_HAT_UP || e->state.hat == OCB_HAT_DOWN || e->state.hat == OCB_HAT_NEUTRAL,
                  "event %zu hat %u outside up/down/neutral", i, e->state.hat);
        OCB_CHECK(e->state.rx == OCB_STICK_CENTER && e->state.ry == OCB_STICK_CENTER, "event %zu right stick", i);
        ends += ocb_state_has_end(&e->state) ? 1 : 0;
        OCB_CHECK((e->state.flags & (uint8_t)~OCB_STATE_FLAG_END) == 0u, "event %zu reserved flags", i);
    }
    const ocb_testtl_event *last = ocb_testtl_get(n - 1u);
    OCB_CHECK(ends == 1 && ocb_state_has_end(&last->state) && ocb_state_is_neutral_pose(&last->state),
              "the last event is NEUTRAL|END and the only END (%d)", ends);
    OCB_CHECK(last->t_us == TL_LAST_US, "last t_us %u, wanted %u", last->t_us, TL_LAST_US);
}

/* Session open, console ready, link up; the table loaded. Returns the
 * load result. */
static int session_and_load(lh *h)
{
    lh_session(h, 512u);
    const ocb_core_env e = env_of(h);
    return ocb_testtl_load(&h->core, &e);
}

static void test_load_and_start(void)
{
    static lh h;
    lh_session(&h, 512u);
    const uint8_t expected = h.core.expected;
    OCB_CHECK(expected == h.seq, "fixture: expected %u, next seq %u", expected, h.seq);
    const ocb_core_env e = env_of(&h);
    const int rc = ocb_testtl_load(&h.core, &e);
    OCB_CHECK(rc == 0, "load returned %d", rc);
    OCB_CHECK(ocb_queue_count(&h.core.queue) == TL_LEN, "queue holds %u events", ocb_queue_count(&h.core.queue));
    OCB_CHECK(h.core.expected == expected, "expected %u after the load, wanted %u", h.core.expected, expected);
    OCB_CHECK(h.core.mode == OCB_MODE_IDLE && !h.core.aborted, "still IDLE, unlatched");
    lh_expect_none(&h, "the load sends nothing to the host");
    const uint8_t start_seq = h.seq;
    lh_start(&h, 3000000u);
    lh_expect_ack(&h, start_seq, "START after the load");
    OCB_CHECK(h.core.mode == OCB_MODE_PLAYING, "PLAYING after START");
}

static void test_plays(void)
{
    static lh h;
    OCB_CHECK(session_and_load(&h) == 0, "load");
    const uint8_t start_seq = h.seq;
    lh_start(&h, 0u);
    lh_expect_ack(&h, start_seq, "START");
    const uint64_t t0 = h.core.t0_us;
    uint32_t applied = 0u;
    int events_seen = 0;
    uint8_t last_event = OCB_EVT_NONE;
    int bad_state = 0;
    const uint64_t end = t0 + TL_LAST_US + 1000u;
    while (h.now <= end) {
        ocb_core_env e = env_of(&h);
        e.link_ok = true;
        ocb_core_out out;
        ocb_core_tick(&h.core, &e, &out);
        if (out.event != OCB_EVT_NONE) {
            events_seen++;
            last_event = out.event;
        }
        if (h.core.timing.n != applied) {
            applied = h.core.timing.n;
            const ocb_state want = ocb_core_mask_state(&ocb_testtl_get(applied - 1u)->state);
            if (!same(&h.core.published, &want)) {
                bad_state++;
            }
        }
        h.now += 50u;
    }
    OCB_CHECK(bad_state == 0, "%d applied states differ from the masked table", bad_state);
    OCB_CHECK(events_seen == 1 && last_event == OCB_EVT_PLAYBACK_DONE, "events %d, last 0x%02x", events_seen,
              last_event);
    OCB_CHECK(h.core.timing.n == TL_LEN, "n %u, wanted %u", h.core.timing.n, TL_LEN);
    OCB_CHECK(h.core.timing.late_max_us < 50u, "late max %u us", h.core.timing.late_max_us);
    OCB_CHECK(h.core.mode == OCB_MODE_IDLE && ocb_state_is_neutral(&h.core.published), "IDLE, NEUTRAL at the end");
}

static void test_stall_late(void)
{
    static lh h;
    OCB_CHECK(session_and_load(&h) == 0, "load");
    lh_start(&h, 0u);
    lh_discard_tx(&h);
    const uint64_t t0 = h.core.t0_us;
    /* Apply the first 80 events on time. */
    for (size_t i = 0; i < 80u; i++) {
        h.now = t0 + ocb_testtl_get(i)->t_us;
        ocb_core_env e = env_of(&h);
        e.link_ok = true;
        ocb_core_out out;
        ocb_core_tick(&h.core, &e, &out);
    }
    OCB_CHECK(h.core.timing.n == 80u, "80 applied: %u", h.core.timing.n);
    /* The loop stalls 30 ms past event 80's due time. */
    h.now = t0 + ocb_testtl_get(80u)->t_us + 30000u;
    ocb_core_env e = env_of(&h);
    e.link_ok = true;
    ocb_core_out out;
    ocb_core_tick(&h.core, &e, &out);
    OCB_CHECK(out.event == OCB_EVT_LATE, "30 ms stall: event 0x%02x, wanted LATE", out.event);
    OCB_CHECK(h.core.timing.n == 80u, "no backlog fired: n %u", h.core.timing.n);
    OCB_CHECK(h.core.mode == OCB_MODE_IDLE && ocb_queue_count(&h.core.queue) == 0u &&
                  ocb_state_is_neutral(&h.core.published),
              "IDLE, empty, NEUTRAL after LATE");
}

static void test_refusals(void)
{
    static lh h;
    OCB_CHECK(session_and_load(&h) == 0, "load");
    ocb_core_env e = env_of(&h);
    /* Twice: the queue is not empty. */
    const uint32_t pc = h.core.publish_count;
    const uint8_t ex = h.core.expected;
    OCB_CHECK(ocb_testtl_load(&h.core, &e) == OCB_TESTTL_ERR_STATE, "second load accepted");
    OCB_CHECK(ocb_queue_count(&h.core.queue) == TL_LEN && h.core.expected == ex && h.core.publish_count == pc,
              "second load changed something");
    /* PLAYING. */
    lh_start(&h, 1000000u);
    lh_discard_tx(&h);
    OCB_CHECK(h.core.mode == OCB_MODE_PLAYING, "setup: PLAYING");
    {
        const uint16_t count = ocb_queue_count(&h.core.queue);
        const uint8_t ex2 = h.core.expected;
        e = env_of(&h);
        OCB_CHECK(ocb_testtl_load(&h.core, &e) == OCB_TESTTL_ERR_STATE, "load into PLAYING accepted");
        OCB_CHECK(ocb_queue_count(&h.core.queue) == count && h.core.expected == ex2 &&
                      h.core.mode == OCB_MODE_PLAYING,
                  "load into PLAYING changed something");
    }
    /* Latched (an abort). */
    ocb_core_out out;
    ocb_core_abort(&h.core, OCB_ABORT_LATE, &out);
    const uint8_t ex3 = h.core.expected;
    e = env_of(&h);
    OCB_CHECK(ocb_testtl_load(&h.core, &e) == OCB_TESTTL_ERR_STATE, "load into a latched core accepted");
    OCB_CHECK(ocb_queue_count(&h.core.queue) == 0u && h.core.expected == ex3 && h.core.aborted,
              "load into a latched core changed something");
    /* After STOP (the latch cleared) it loads again: the second run. */
    lh_stop(&h);
    lh_discard_tx(&h);
    e = env_of(&h);
    OCB_CHECK(ocb_testtl_load(&h.core, &e) == 0 && ocb_queue_count(&h.core.queue) == TL_LEN, "reload after STOP");
}

static void test_bad_table(void)
{
    static lh h;
    lh_session(&h, 512u);
    const uint8_t ex = h.core.expected;
    /* Negative control: the third row goes back in time (NAK ORDER from the
     * real ocb_core_frame path). */
    const ocb_testtl_event bad[3] = {
        {0u, OCB_NEUTRAL_INIT},
        {500000u, {OCB_BTN_Y, OCB_HAT_NEUTRAL, 0x80u, 0x80u, 0x80u, 0x80u, 0u}},
        {400000u, OCB_NEUTRAL_INIT},
    };
    ocb_core_env e = env_of(&h);
    OCB_CHECK(ocb_testtl_load_table(&h.core, &e, bad, 3u) == OCB_NAK_ORDER, "bad order not refused with NAK ORDER");
    OCB_CHECK(ocb_queue_count(&h.core.queue) == 0u && h.core.expected == ex && h.core.mode == OCB_MODE_IDLE &&
                  !h.core.aborted,
              "a refused table left something behind");
    /* END on a pressed state: NAK BAD_STATE. */
    const ocb_testtl_event bad_end[2] = {
        {0u, OCB_NEUTRAL_INIT},
        {500000u, {OCB_BTN_Y, OCB_HAT_NEUTRAL, 0x80u, 0x80u, 0x80u, 0x80u, OCB_STATE_FLAG_END}},
    };
    OCB_CHECK(ocb_testtl_load_table(&h.core, &e, bad_end, 2u) == OCB_NAK_BAD_STATE, "END on a note accepted");
    OCB_CHECK(ocb_queue_count(&h.core.queue) == 0u && h.core.expected == ex, "END refusal left something behind");
    /* Too many for the queue: NAK FULL. */
    static lh small;
    lh_session(&small, 16u);
    e = env_of(&small);
    OCB_CHECK(ocb_testtl_load(&small.core, &e) == OCB_NAK_FULL, "158 events into a 16-event queue");
    OCB_CHECK(ocb_queue_count(&small.core.queue) == 0u, "FULL left a partial table");
}

int main(void)
{
    test_table();
    test_load_and_start();
    test_plays();
    test_stall_late();
    test_refusals();
    test_bad_table();
    const int f = ocb_test_failures();
    if (f != 0) {
        fprintf(stderr, "test_testtl (F.37): %d failure(s)\n", f);
        return 1;
    }
    printf("test_testtl (F.37): ok\n");
    return 0;
}
