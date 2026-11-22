/*
 * SPDX-License-Identifier: MIT
 *
 * Host test F.12 (story 2.4): controller core edge cases and a fixed-seed
 * property test. Queue capacity and NAK FULL, the D3 NAK precedence, the
 * NOT_READY gate, validation (hat, body lengths), unassigned and foreign
 * frame types, the STATUS view, a clock above 2^32, two independent cores,
 * and invariants over random frame / tick / abort sequences (D10).
 */
#include <stdint.h>
#include <stdio.h>

#include "core_fixture.h"

#define HAND_ALLOWED 0x003Du /* Y | A | X | L | R, by hand */

static void expect_nak(ocb_core_out o, uint8_t reason, const char *what)
{
    OCB_CHECK(o.kind == OCB_CORE_REPLY_NAK && o.nak_reason == reason, "%s: kind %d NAK 0x%02x, wanted NAK 0x%02x",
              what, (int)o.kind, o.nak_reason, reason);
}

static void expect_ack(ocb_core_out o, const char *what)
{
    OCB_CHECK(o.kind == OCB_CORE_REPLY_ACK, "%s: kind %d NAK 0x%02x, wanted ACK", what, (int)o.kind, o.nak_reason);
}

static void test_queue_capacity(void)
{
    const uint16_t caps[] = {1u, 2u, 4u, 512u};
    for (size_t ci = 0; ci < sizeof caps / sizeof caps[0]; ci++) {
        const uint16_t cap = caps[ci];
        static harness h;
        h_init(&h, cap, 1000u);
        for (uint32_t i = 0; i < cap; i++) {
            OCB_CHECK(ocb_queue_free(&h.core.queue) == cap - i, "cap %u: free %u before event %u", cap,
                      ocb_queue_free(&h.core.queue), (unsigned)i);
            const ocb_core_out o = h_queue_event(&h, (i + 1u) * 100u, st_buttons(OCB_BTN_A));
            if (o.kind != OCB_CORE_REPLY_ACK) {
                OCB_CHECK(false, "cap %u: event %u refused (NAK 0x%02x)", cap, (unsigned)i, o.nak_reason);
                break;
            }
        }
        OCB_CHECK(ocb_queue_free(&h.core.queue) == 0u && ocb_queue_count(&h.core.queue) == cap, "cap %u: not full",
                  cap);
        const uint8_t before = h.core.expected;
        const uint8_t seq = h.seq;
        const uint32_t t_full = ((uint32_t)cap + 1u) * 100u;
        expect_nak(h_queue_event(&h, t_full, st_buttons(OCB_BTN_X)), OCB_NAK_FULL, "event cap+1");
        OCB_CHECK(h.core.expected == before, "cap %u: FULL advanced expected", cap);
        OCB_CHECK(!h.core.aborted && h.core.mode == OCB_MODE_IDLE, "cap %u: FULL latched or changed mode", cap);
        OCB_CHECK(h.core.last_t_us == cap * 100u, "cap %u: FULL moved the ordering baseline", cap);

        /* Retry the same frame after a slot frees up: accepted. */
        if (cap >= 2u) {
            h.ready = true;
            expect_ack(h_start(&h, 0u), "START");
            h.now += 100u;
            (void)h_tick(&h);
            OCB_CHECK(ocb_queue_free(&h.core.queue) == 1u, "cap %u: free after one apply", cap);
            h.seq = seq;
            expect_ack(h_queue_event(&h, t_full, st_buttons(OCB_BTN_X)), "retry after FULL");
            OCB_CHECK(h.core.expected == (uint8_t)(seq + 1u), "cap %u: retry did not advance", cap);
        }
    }

    /* Zero capacity and NULL storage are refused at init; QUEUE_EVENT then
     * always gets FULL. */
    ocb_queue q;
    ocb_core_event one[1];
    OCB_CHECK(ocb_queue_init(&q, one, 0u) == -1 && ocb_queue_free(&q) == 0u, "queue cap 0");
    OCB_CHECK(ocb_queue_init(&q, NULL, 4u) == -1, "queue NULL storage");
    ocb_core c;
    OCB_CHECK(ocb_core_init(&c, NULL, 0u, 0u) == -1, "core with no storage");
    ocb_frame f;
    memset(&f, 0, sizeof f);
    ocb_msg_queue_event m = {10u, OCB_NEUTRAL_INIT};
    f.type = OCB_MSG_QUEUE_EVENT;
    f.len = (uint8_t)ocb_msg_queue_event_encode(&m, f.body, sizeof f.body);
    const ocb_core_env env = {0u, true};
    ocb_core_out out;
    ocb_core_frame(&c, &env, &f, &out);
    expect_nak(out, OCB_NAK_FULL, "zero-capacity core");

    /* FIFO order across many wraps of a 3-slot ring. */
    ocb_core_event st[3];
    OCB_CHECK(ocb_queue_init(&q, st, 3u) == 0, "init 3");
    uint32_t pushed = 0;
    uint32_t popped = 0;
    for (int round = 0; round < 50; round++) {
        const int n_push = 1 + round % 3;
        for (int k = 0; k < n_push; k++) {
            const ocb_core_event ev = {pushed, OCB_NEUTRAL_INIT};
            if (ocb_queue_push(&q, &ev)) {
                pushed++;
            }
        }
        const int n_pop = 1 + (round + 1) % 3;
        for (int k = 0; k < n_pop; k++) {
            const ocb_core_event *pk = ocb_queue_peek(&q);
            ocb_core_event ev;
            const bool got = ocb_queue_pop(&q, &ev);
            OCB_CHECK(got == (pk != NULL), "peek/pop disagree");
            if (got) {
                OCB_CHECK(ev.t_us == popped, "FIFO order: got %u, wanted %u", (unsigned)ev.t_us, (unsigned)popped);
                popped++;
            }
        }
        OCB_CHECK(ocb_queue_count(&q) + ocb_queue_free(&q) == 3u, "count + free != cap");
    }
    OCB_CHECK(pushed > 50u && popped + ocb_queue_count(&q) == pushed, "ring bookkeeping");
    ocb_queue_clear(&q);
    OCB_CHECK(ocb_queue_count(&q) == 0u && ocb_queue_peek(&q) == NULL && !ocb_queue_pop(&q, NULL), "clear");
}

static ocb_state st_hat(uint8_t hat)
{
    ocb_state s = OCB_NEUTRAL;
    s.hat = hat;
    return s;
}

/* D3: ABORTED > BUSY > NOT_READY > BAD_STATE > ORDER > FULL. */
static void test_precedence(void)
{
    const uint8_t bad3[3] = {0, 0, 0};
    /* ABORTED over NOT_READY, BAD_STATE, ORDER, FULL. */
    {
        harness h;
        h_init(&h, 1u, 1000u);
        (void)h_abort(&h, OCB_ABORT_LINK_LOST);
        h.ready = false;
        expect_nak(h_set_state(&h, st_neutral()), OCB_NAK_ABORTED, "ABORTED over NOT_READY (SET_STATE)");
        expect_nak(h_start(&h, 0u), OCB_NAK_ABORTED, "ABORTED over NOT_READY (START)");
        h.ready = true;
        expect_nak(h_raw(&h, OCB_MSG_SET_STATE, bad3, 3u), OCB_NAK_ABORTED, "ABORTED over BAD_STATE (SET_STATE)");
        expect_nak(h_raw(&h, OCB_MSG_QUEUE_EVENT, bad3, 3u), OCB_NAK_ABORTED, "ABORTED over BAD_STATE (QUEUE)");
        expect_nak(h_raw(&h, OCB_MSG_START, bad3, 3u), OCB_NAK_ABORTED, "ABORTED over BAD_STATE (START)");
        expect_nak(h_queue_event(&h, 0x80000000u, st_neutral()), OCB_NAK_ABORTED, "ABORTED over ORDER");
        harness z;
        memset(&z, 0, sizeof z);
        (void)ocb_core_init(&z.core, NULL, 0u, 0u); /* every push would be FULL */
        z.ready = true;
        (void)h_abort(&z, OCB_ABORT_CORE_STALL);
        expect_nak(h_queue_event(&z, 10u, st_neutral()), OCB_NAK_ABORTED, "ABORTED over FULL");
    }
    /* ABORTED over BUSY: unreachable (the latch implies IDLE); white-box,
     * with the invariant broken on purpose, ABORTED still wins. */
    {
        harness h;
        h_init(&h, 4u, 1000u);
        h_enter_mode(&h, OCB_MODE_PLAYING);
        h.core.aborted = true;
        expect_nak(h_set_state(&h, st_neutral()), OCB_NAK_ABORTED, "ABORTED over BUSY (SET_STATE)");
        expect_nak(h_start(&h, 0u), OCB_NAK_ABORTED, "ABORTED over BUSY (START)");
    }
    /* BUSY over NOT_READY and BAD_STATE. */
    {
        harness h;
        h_init(&h, 4u, 1000u);
        h_enter_mode(&h, OCB_MODE_PLAYING);
        h.ready = false;
        expect_nak(h_set_state(&h, st_neutral()), OCB_NAK_BUSY, "BUSY over NOT_READY (SET_STATE)");
        expect_nak(h_start(&h, 0u), OCB_NAK_BUSY, "BUSY over NOT_READY (START)");
        h.ready = true;
        expect_nak(h_raw(&h, OCB_MSG_SET_STATE, bad3, 3u), OCB_NAK_BUSY, "BUSY over BAD_STATE (SET_STATE)");
        expect_nak(h_raw(&h, OCB_MSG_START, bad3, 3u), OCB_NAK_BUSY, "BUSY over BAD_STATE (START)");
    }
    /* NOT_READY over BAD_STATE (including START with an empty queue). */
    {
        harness h;
        h_init(&h, 4u, 1000u);
        h.ready = false;
        expect_nak(h_raw(&h, OCB_MSG_SET_STATE, bad3, 3u), OCB_NAK_NOT_READY, "NOT_READY over BAD_STATE (SET)");
        expect_nak(h_set_state(&h, st_hat(9)), OCB_NAK_NOT_READY, "NOT_READY over hat 9");
        expect_nak(h_start(&h, 0u), OCB_NAK_NOT_READY, "NOT_READY over empty-queue START");
    }
    /* BAD_STATE over ORDER and FULL; ORDER over FULL. */
    {
        harness h;
        h_init(&h, 1u, 1000u);
        expect_ack(h_queue_event(&h, 500u, st_neutral()), "fill");
        expect_nak(h_queue_event(&h, 500u, st_hat(9)), OCB_NAK_BAD_STATE, "BAD_STATE over ORDER");
        ocb_state rs = st_neutral();
        rs.flags = 0x04;
        expect_nak(h_queue_event(&h, 600u, rs), OCB_NAK_BAD_STATE, "BAD_STATE over FULL");
        expect_nak(h_queue_event(&h, 400u, st_neutral()), OCB_NAK_ORDER, "ORDER over FULL");
        expect_nak(h_queue_event(&h, 0x80000000u, st_neutral()), OCB_NAK_ORDER, "ORDER (2^31) over FULL");
        expect_nak(h_queue_event(&h, 600u, st_neutral()), OCB_NAK_FULL, "FULL alone");
    }
}

static void test_not_ready(void)
{
    harness h;
    h_init(&h, 4u, 1000u);
    h.ready = false;
    expect_ack(h_queue_event(&h, 100u, st_buttons(OCB_BTN_A)), "QUEUE_EVENT while not ready");
    expect_ack(h_queue_event(&h, 200u, st_end()), "QUEUE_EVENT END while not ready");
    expect_ack(h_ping(&h), "PING while not ready");
    expect_nak(h_set_state(&h, st_neutral()), OCB_NAK_NOT_READY, "SET_STATE not ready");
    expect_nak(h_start(&h, 0u), OCB_NAK_NOT_READY, "START not ready");
    OCB_CHECK(h.core.mode == OCB_MODE_IDLE && h.core.publish_count == 0u, "NOT_READY had an effect");
    h.ready = true; /* follows env.ready live, no latch */
    expect_ack(h_set_state(&h, st_buttons(OCB_BTN_B)), "SET_STATE once ready");
    h.ready = false;
    expect_nak(h_set_state(&h, st_neutral()), OCB_NAK_NOT_READY, "SET_STATE not ready again");
    OCB_CHECK(h.core.published.buttons == OCB_BTN_B && h.core.mode == OCB_MODE_DIRECT, "DIRECT kept");
    expect_nak(h_start(&h, 0u), OCB_NAK_NOT_READY, "START from DIRECT not ready");
    h.ready = true;
    expect_ack(h_start(&h, 0u), "START once ready");
    /* Readiness does not matter once PLAYING (the USB causes abort instead). */
    h.ready = false;
    h.now += 100u;
    (void)h_tick(&h);
    OCB_CHECK(h.core.published.buttons == OCB_BTN_A, "tick depends on ready");
}

static void test_validation(void)
{
    harness h;
    h_init(&h, 8u, 1000u);
    expect_nak(h_start(&h, 0u), OCB_NAK_BAD_STATE, "START with an empty queue (IDLE)");
    expect_ack(h_set_state(&h, st_neutral()), "DIRECT");
    expect_nak(h_start(&h, 0u), OCB_NAK_BAD_STATE, "START with an empty queue (DIRECT)");
    OCB_CHECK(h.core.mode == OCB_MODE_DIRECT, "refused START changed the mode");

    const uint8_t hats[] = {9u, 10u, 128u, 255u};
    for (size_t i = 0; i < sizeof hats; i++) {
        char what[48];
        snprintf(what, sizeof what, "QUEUE_EVENT hat %u", hats[i]);
        expect_nak(h_queue_event(&h, 100u + (uint32_t)i, st_hat(hats[i])), OCB_NAK_BAD_STATE, what);
        snprintf(what, sizeof what, "SET_STATE hat %u", hats[i]);
        expect_nak(h_set_state(&h, st_hat(hats[i])), OCB_NAK_BAD_STATE, what);
    }
    for (uint8_t hat = 0; hat <= 8u; hat++) {
        expect_ack(h_set_state(&h, st_hat(hat)), "SET_STATE valid hat");
    }
    /* Undefined button bits 14 and 15 are not refused (masked or ignored). */
    expect_ack(h_set_state(&h, st_buttons(0xC000u)), "SET_STATE bits 14/15");
    expect_ack(h_queue_event(&h, 100u, st_buttons(0xC000u)), "QUEUE_EVENT bits 14/15");

    /* Wrong body lengths: BAD_STATE, semantic (expected advances). */
    static const struct {
        uint8_t type;
        uint8_t len;
    } bad_len[] = {
        {OCB_MSG_SET_STATE, 0u},   {OCB_MSG_SET_STATE, 7u},   {OCB_MSG_SET_STATE, 9u},
        {OCB_MSG_SET_STATE, 255u}, {OCB_MSG_QUEUE_EVENT, 0u}, {OCB_MSG_QUEUE_EVENT, 11u},
        {OCB_MSG_QUEUE_EVENT, 13u}, {OCB_MSG_START, 0u},      {OCB_MSG_START, 3u},
        {OCB_MSG_START, 5u},       {OCB_MSG_PING, 1u},        {OCB_MSG_PING, 255u},
    };
    uint8_t body[OCB_MAX_BODY];
    memset(body, 0, sizeof body);
    body[2] = OCB_HAT_NEUTRAL;
    for (size_t i = 0; i < sizeof bad_len / sizeof bad_len[0]; i++) {
        harness g;
        h_init(&g, 8u, 1000u);
        expect_ack(h_queue_event(&g, 100u, st_neutral()), "prefill");
        const uint8_t seq = g.seq;
        const ocb_core_out o = h_raw(&g, bad_len[i].type, body, bad_len[i].len);
        char what[48];
        snprintf(what, sizeof what, "type 0x%02x len %u", bad_len[i].type, bad_len[i].len);
        expect_nak(o, OCB_NAK_BAD_STATE, what);
        OCB_CHECK(g.core.expected == (uint8_t)(seq + 1u), "%s: expected not advanced", what);
        OCB_CHECK(g.core.mode == OCB_MODE_IDLE && ocb_queue_count(&g.core.queue) == 1u, "%s: had an effect", what);
    }
    /* STOP and HELLO are unaffected by their body length. */
    harness g;
    h_init(&g, 8u, 1000u);
    expect_ack(h_raw(&g, OCB_MSG_STOP, body, 8u), "STOP with a body");
    OCB_CHECK(h_raw(&g, OCB_MSG_HELLO, body, 0u).kind == OCB_CORE_REPLY_HELLO, "HELLO with no body");
}

static void test_types(void)
{
    harness h;
    h_init(&h, 8u, 1000u);
    h_enter_mode(&h, OCB_MODE_DIRECT);
    const uint8_t unassigned[] = {0x07, 0x20, 0x3F};
    for (size_t i = 0; i < sizeof unassigned; i++) {
        const uint8_t seq = h.seq;
        expect_nak(h_raw(&h, unassigned[i], NULL, 0u), OCB_NAK_UNKNOWN_TYPE, "unassigned type");
        OCB_CHECK(h.core.expected == (uint8_t)(seq + 1u), "type 0x%02x: expected not advanced", unassigned[i]);
    }
    const uint8_t foreign[] = {0x00, 0x40, 0x7F, 0x80, 0x81, 0x84, 0xFF};
    for (size_t i = 0; i < sizeof foreign; i++) {
        const ocb_core before = h.core;
        const ocb_core_out o = h_raw(&h, foreign[i], NULL, 0u);
        OCB_CHECK(o.kind == OCB_CORE_REPLY_NONE && o.event == OCB_EVT_NONE, "type 0x%02x: reply %d", foreign[i],
                  (int)o.kind);
        OCB_CHECK(h.core.expected == before.expected && h.core.mode == before.mode &&
                      h.core.publish_count == before.publish_count &&
                      ocb_queue_count(&h.core.queue) == ocb_queue_count(&before.queue) &&
                      h.core.aborted == before.aborted,
                  "type 0x%02x changed the core", foreign[i]);
    }
}

static void test_status_view(void)
{
    harness h;
    h_init(&h, 8u, 1000u);
    ocb_core_status_view v;
    ocb_core_status(&h.core, h.now, &v);
    OCB_CHECK(v.free == 8u && v.now_us == 0u && v.last_seq == 255u && v.mode == OCB_MODE_IDLE &&
                  v.abort_count == 0u && v.last_event == OCB_EVT_NONE,
              "initial status");
    h.seq = 10u;
    expect_ack(h_queue_event(&h, 1000u, st_buttons(OCB_BTN_A)), "q1");
    expect_ack(h_queue_event(&h, 2000u, st_end()), "q2");
    ocb_core_status(&h.core, h.now, &v);
    OCB_CHECK(v.free == 6u && v.last_seq == 11u && v.now_us == 0u, "IDLE with two queued: free %u last_seq %u",
              v.free, v.last_seq);
    expect_ack(h_set_state(&h, st_neutral()), "DIRECT");
    ocb_core_status(&h.core, h.now + 5000u, &v);
    OCB_CHECK(v.mode == OCB_MODE_DIRECT && v.now_us == 0u && v.last_seq == 12u, "DIRECT status");
    expect_ack(h_start(&h, 500u), "START lead 500");
    const uint64_t t0 = h.now + 500u;
    ocb_core_status(&h.core, h.now + 499u, &v);
    OCB_CHECK(v.mode == OCB_MODE_PLAYING && v.now_us == 0u, "lead: now_us %u", (unsigned)v.now_us);
    ocb_core_status(&h.core, t0, &v);
    OCB_CHECK(v.now_us == 0u, "at t0: now_us %u", (unsigned)v.now_us);
    ocb_core_status(&h.core, t0 + 1234u, &v);
    OCB_CHECK(v.now_us == 1234u, "after t0: now_us %u", (unsigned)v.now_us);
    h.now = t0 + 1000u;
    (void)h_tick(&h);
    ocb_core_status(&h.core, h.now, &v);
    OCB_CHECK(v.free == 7u && v.now_us == 1000u, "after one apply: free %u now %u", v.free, (unsigned)v.now_us);
    h.now = t0 + 2000u;
    (void)h_tick(&h);
    ocb_core_status(&h.core, h.now, &v);
    OCB_CHECK(v.mode == OCB_MODE_IDLE && v.now_us == 0u && v.abort_count == 1u &&
                  v.last_event == OCB_EVT_PLAYBACK_DONE && v.free == 8u,
              "after END");
    h.seq = 0u;
    (void)h_stop(&h);
    ocb_core_status(&h.core, h.now, &v);
    OCB_CHECK(v.last_seq == 0u && v.abort_count == 1u && v.last_event == OCB_EVT_PLAYBACK_DONE, "after STOP seq 0");
}

static void test_clock_above_2_32(void)
{
    harness h;
    h_init(&h, 8u, 1000u);
    h.now = 0x100000000ull - 3000u; /* the piece crosses 2^32 */
    expect_ack(h_queue_event(&h, 1000u, st_buttons(OCB_BTN_A)), "q1");
    expect_ack(h_queue_event(&h, 5000u, st_buttons(OCB_BTN_X)), "q2");
    expect_ack(h_queue_event(&h, 0x7FFFFFFFu, st_end()), "q3");
    expect_ack(h_start(&h, 1000u), "START");
    const uint64_t t0 = h.now + 1000u;
    h.now = t0 + 999u;
    (void)h_tick(&h);
    OCB_CHECK(h_published_neutral(&h), "applied early");
    h.now = t0 + 1000u;
    (void)h_tick(&h);
    OCB_CHECK(h.core.published.buttons == OCB_BTN_A, "first event");
    h.now = t0 + 5000u; /* above 2^32 */
    OCB_CHECK(h.now > 0xFFFFFFFFull, "test setup: now below 2^32");
    (void)h_tick(&h);
    OCB_CHECK(h.core.published.buttons == OCB_BTN_X, "event across 2^32");
    ocb_core_status_view v;
    ocb_core_status(&h.core, h.now, &v);
    OCB_CHECK(v.now_us == 5000u, "status across 2^32: %u", (unsigned)v.now_us);
    /* The last event is due at t0 + 2^31 - 1, beyond 2^32 + 2^31. */
    h.now = t0 + 0x7FFFFFFFull - 1u;
    ocb_core_out o = h_tick(&h);
    OCB_CHECK(o.event == OCB_EVT_NONE && h.core.mode == OCB_MODE_PLAYING, "END early");
    h.now += 1u;
    o = h_tick(&h);
    OCB_CHECK(o.event == OCB_EVT_PLAYBACK_DONE, "END at t0 + 2^31 - 1: event %u", o.event);
}

/* Two instances share nothing (no hidden global state, Task 7.5). */
static void test_two_cores(void)
{
    harness a;
    harness b;
    h_init(&a, 4u, 1000u);
    h_init(&b, 8u, 0u);
    a.seq = 10u;
    b.seq = 100u;
    h_enter_mode(&a, OCB_MODE_PLAYING);
    expect_ack(h_set_state(&b, st_buttons(OCB_BTN_HOME)), "b DIRECT");
    (void)h_abort(&a, OCB_ABORT_LINK_LOST);
    OCB_CHECK(a.core.aborted && !b.core.aborted, "abort leaked to the other core");
    OCB_CHECK(b.core.mode == OCB_MODE_DIRECT && b.core.published.buttons == OCB_BTN_HOME, "b changed");
    OCB_CHECK(a.core.expected == 13u && b.core.expected == 101u, "expected shared: %u %u", a.core.expected,
              b.core.expected);
    OCB_CHECK(a.core.late_us == 1000u && b.core.late_us == 8000u, "late_us shared");
    expect_nak(h_queue_event(&a, 1u, st_neutral()), OCB_NAK_ABORTED, "a aborted");
    expect_ack(h_queue_event(&b, 1u, st_neutral()), "b accepts");
    OCB_CHECK(ocb_queue_count(&a.core.queue) == 0u && ocb_queue_count(&b.core.queue) == 1u, "queues shared");
    OCB_CHECK(a.core.abort_count == 1u && b.core.abort_count == 0u, "abort_count shared");
}

/* Property test (D10): fixed-seed LCG, three seeds x 20 000 steps. */
static uint32_t g_rng;
static uint32_t rnd(void)
{
    g_rng = g_rng * 1664525u + 1013904223u;
    return g_rng >> 8;
}

static ocb_state rnd_state(void)
{
    ocb_state s;
    const uint32_t r = rnd();
    s.buttons = (uint16_t)rnd();
    s.hat = (uint8_t)((r & 7u) == 0u ? rnd() : rnd() % 9u);
    s.lx = (uint8_t)rnd();
    s.ly = (uint8_t)rnd();
    s.rx = (uint8_t)rnd();
    s.ry = (uint8_t)rnd();
    s.flags = (uint8_t)((r & 0x30u) == 0u ? rnd() : 0u);
    if ((r & 0x1F00u) == 0u) { /* 1 in 32: END */
        s = OCB_NEUTRAL;
        s.flags = OCB_STATE_FLAG_END;
    }
    return s;
}

static void random_frame(harness *h, ocb_core_out *out, uint8_t *seq_out)
{
    h->seq = (uint8_t)rnd();
    *seq_out = h->seq;
    const uint32_t kind = rnd() % 20u;
    if (kind < 7u) {
        /* Mostly valid, mostly in order. */
        uint32_t t = h->core.have_last_t ? h->core.last_t_us + 1u + rnd() % 3000u : rnd() % 3000u;
        if (rnd() % 16u == 0u) {
            t = rnd() << 1;
        }
        *out = h_queue_event(h, t, rnd_state());
    } else if (kind < 9u) {
        *out = h_start(h, rnd() % 4000u);
    } else if (kind < 11u) {
        *out = h_set_state(h, rnd_state());
    } else if (kind < 12u) {
        *out = h_stop(h);
    } else if (kind < 13u) {
        *out = h_hello(h);
    } else if (kind < 15u) {
        *out = h_ping(h);
    } else {
        uint8_t body[OCB_MAX_BODY];
        for (size_t i = 0; i < sizeof body; i++) {
            body[i] = (uint8_t)rnd();
        }
        const uint8_t type = (uint8_t)(kind < 18u ? 1u + rnd() % 8u : rnd());
        const uint8_t len = (uint8_t)(rnd() % 4u == 0u ? rnd() : rnd() % 14u);
        *out = h_raw(h, type, body, len);
    }
}

static bool masked_ok(const ocb_state *s)
{
    return (s->buttons & (uint16_t)~HAND_ALLOWED) == 0u && s->rx == 0x80u && s->ry == 0x80u && s->flags == 0u;
}

static void property_run(uint32_t seed)
{
    static harness h;
    g_rng = seed;
    h_init(&h, 8u, (uint16_t)(seed & 1u ? 1000u : 0u));
    int failures_before = ocb_test_failures();
    unsigned playing_steps = 0;
    unsigned outcome[8] = {0}; /* by EVENT code, from ticks */
    unsigned acks = 0;
    unsigned naks = 0;
    for (int step = 0; step < 20000; step++) {
        const uint8_t expected_before = h.core.expected;
        const uint32_t what = rnd() % 100u;
        ocb_core_out o;
        if (what < 45u) {
            uint8_t seq;
            random_frame(&h, &o, &seq);
            const bool frozen =
                o.kind == OCB_CORE_REPLY_NONE || (o.kind == OCB_CORE_REPLY_NAK && o.nak_reason == OCB_NAK_FULL);
            const uint8_t want = frozen ? expected_before : (uint8_t)(seq + 1u);
            OCB_CHECK(h.core.expected == want, "seed %u step %d: expected %u, wanted %u", (unsigned)seed, step,
                      h.core.expected, want);
            OCB_CHECK(o.event == OCB_EVT_NONE, "seed %u step %d: a frame emitted an event", (unsigned)seed, step);
            if (o.kind == OCB_CORE_REPLY_NAK) {
                OCB_CHECK(o.nak_reason == OCB_NAK_FULL || OCB_NAK_IS_SEMANTIC(o.nak_reason),
                          "seed %u step %d: NAK 0x%02x", (unsigned)seed, step, o.nak_reason);
            }
        } else if (what < 95u) {
            const uint32_t r = rnd();
            h.now += (r & 0xF0u) == 0u ? rnd() % 100000u : rnd() % 1500u;
            o = h_tick(&h);
            outcome[o.event & 7u]++;
            OCB_CHECK(h.core.expected == expected_before, "seed %u step %d: tick moved expected", (unsigned)seed,
                      step);
        } else if (what < 97u) {
            o = h_abort(&h, (ocb_core_abort_cause)(rnd() % (uint32_t)OCB_ABORT_CAUSE_COUNT));
            OCB_CHECK(h.core.aborted && h.core.mode == OCB_MODE_IDLE, "seed %u step %d: abort", (unsigned)seed,
                      step);
            OCB_CHECK(h.core.expected == expected_before, "seed %u step %d: abort moved expected", (unsigned)seed,
                      step);
        } else {
            h.ready = rnd() % 4u != 0u; /* mostly ready */
            o.kind = OCB_CORE_REPLY_NONE;
        }

        acks += o.kind == OCB_CORE_REPLY_ACK ? 1u : 0u;
        naks += o.kind == OCB_CORE_REPLY_NAK ? 1u : 0u;
        const ocb_core *c = &h.core;
        playing_steps += c->mode == OCB_MODE_PLAYING ? 1u : 0u;
        OCB_CHECK(h_invariants_ok(&h), "seed %u step %d: IDLE/latch/queue invariant", (unsigned)seed, step);
        if (c->mode == OCB_MODE_PLAYING) {
            OCB_CHECK(masked_ok(&c->published), "seed %u step %d: PLAYING state not masked", (unsigned)seed, step);
            OCB_CHECK(ocb_queue_count(&c->queue) > 0u, "seed %u step %d: PLAYING with an empty queue",
                      (unsigned)seed, step);
        }
        OCB_CHECK(ocb_queue_count(&c->queue) <= c->queue.cap, "seed %u step %d: count > cap", (unsigned)seed, step);
        if (ocb_test_failures() > failures_before + 5) {
            return; /* keep the log readable */
        }
    }
    printf("property seed 0x%08x: %u ACK, %u NAK, %u steps in PLAYING, ticks ended %u by END, %u by underrun, "
           "%u by LATE\n",
           (unsigned)seed, acks, naks, playing_steps, outcome[OCB_EVT_PLAYBACK_DONE], outcome[OCB_EVT_UNDERRUN],
           outcome[OCB_EVT_LATE]);
    /* The random walk must reach the interesting states, or it proves little. */
    OCB_CHECK(playing_steps > 500u && outcome[OCB_EVT_PLAYBACK_DONE] > 0u && outcome[OCB_EVT_UNDERRUN] > 0u &&
                  outcome[OCB_EVT_LATE] > 0u && acks > 1000u && naks > 1000u,
              "seed %u: property walk too shallow", (unsigned)seed);
    /* A final STOP always lands in a clean IDLE. */
    h.seq = 77u;
    const ocb_core_out o = h_stop(&h);
    OCB_CHECK(o.kind == OCB_CORE_REPLY_ACK && h.core.mode == OCB_MODE_IDLE && !h.core.aborted &&
                  ocb_queue_count(&h.core.queue) == 0u && h_published_neutral(&h) && h.core.expected == 78u,
              "seed %u: final STOP", (unsigned)seed);
}

static void test_property(void)
{
    const uint32_t seeds[] = {0x12345678u, 0xC0FFEE01u, 0x0BADF00Du};
    for (size_t i = 0; i < sizeof seeds / sizeof seeds[0]; i++) {
        property_run(seeds[i]);
    }
}

int main(void)
{
    test_queue_capacity();
    test_precedence();
    test_not_ready();
    test_validation();
    test_types();
    test_status_view();
    test_clock_above_2_32();
    test_two_cores();
    test_property();
    const int f = ocb_test_failures();
    if (f != 0) {
        fprintf(stderr, "test_core_edge (F.12): %d failure(s)\n", f);
        return 1;
    }
    printf("test_core_edge (F.12): ok\n");
    return 0;
}
