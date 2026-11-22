/*
 * SPDX-License-Identifier: MIT
 *
 * F.33 (story 2.3 D35, story 2.5 D4, D22): the published-state handoff
 * from core 1 to core 0, a sequence-locked double buffer. One thread plays
 * both cores:
 *  - init is NEUTRAL; 100 000 random states round-trip;
 *  - exhaustive interleavings: the writer's steps (begin, field part 0,
 *    field part 1, end) for 1 to 4 publishes against the reader's steps
 *    (begin, copy part 0, copy part 1, end; up to 3 tries) in every order:
 *    a returned state is always one that was published (never torn or
 *    mixed), it is the state current at the start of the read or a later
 *    one, NEUTRAL comes only after 3 failed tries and only when at least
 *    two publishes started inside the read; with at most 3 publishes it
 *    never happens, with 4 it can (the fallback path is reachable);
 *  - the bench-shaped case of Story 2.3 H.15 attempt 3 (a writer that
 *    rewrites every pass, one write inside every read) gives no fallback;
 *  - write on change (publish_count), the generation wrap, the reader's
 *    saturating counters.
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

/* ------------------------------------------------------------------ */
/* Exhaustive interleavings.                                          */
/* ------------------------------------------------------------------ */

#define MAXK 4

typedef struct {
    ocb_pubstate ps;
    int w;        /* writer steps done, 0..4k */
    int t;        /* reader try, 0..2 */
    int r;        /* reader step inside the try, 0..3 */
    uint32_t g0;  /* reader: generation read at the start of the try */
    ocb_state c;  /* reader: the copy in progress */
    int w0;       /* writer steps done when the reader's first try began */
} model;

static ocb_state s_pub[MAXK + 1]; /* s_pub[0]: current before the read; s_pub[i]: publish i */
static int s_k;
static unsigned long s_leaves;
static unsigned long s_fallbacks;
static unsigned long s_retried;
static int s_fail_reported;

static void fill_states(void)
{
    for (int i = 0; i <= MAXK; i++) {
        /* Every field distinct per publish, so a mix of two is no publish. */
        const uint8_t v = (uint8_t)(0x10u * (unsigned)(i + 1));
        s_pub[i].buttons = (uint16_t)(0x0101u * (unsigned)(i + 1));
        s_pub[i].hat = (uint8_t)i;
        s_pub[i].lx = v;
        s_pub[i].ly = (uint8_t)(v + 1u);
        s_pub[i].rx = (uint8_t)(v + 2u);
        s_pub[i].ry = (uint8_t)(v + 3u);
        s_pub[i].flags = (uint8_t)(i & 1);
    }
}

static void writer_step(model *m)
{
    const int pub = m->w / 4 + 1; /* publish 1..k */
    switch (m->w % 4) {
    case 0:
        ocb_pubstate_write_begin(&m->ps);
        break;
    case 1:
        ocb_pubstate_write_fields(&m->ps, &s_pub[pub], 0);
        break;
    case 2:
        ocb_pubstate_write_fields(&m->ps, &s_pub[pub], 1);
        break;
    default:
        ocb_pubstate_write_end(&m->ps);
        break;
    }
    m->w++;
}

static void report(const char *what, const model *m)
{
    if (s_fail_reported < 5) {
        s_fail_reported++;
        OCB_CHECK(0, "k=%d: %s (writer step %d, try %d)", s_k, what, m->w, m->t);
    } else {
        ocb_test_fail_count++;
    }
}

/* Reader finished with result ok/out; writer at step m->w. */
static void leaf(const model *m, int ok, const ocb_state *out)
{
    s_leaves++;
    const int completed_at_end = m->w / 4;
    const int started_at_end = (m->w + 3) / 4;
    if (ok) {
        if (m->t > 0) {
            s_retried++;
        }
        int j = -1;
        for (int i = 0; i <= s_k; i++) {
            if (same(out, &s_pub[i])) {
                j = i;
            }
        }
        if (j < 0) {
            report("torn or mixed state returned", m);
        } else if (j > completed_at_end) {
            report("a state not yet published returned", m);
        } else if (j < m->w0 / 4) {
            report("a state older than the one current at the start of the read", m);
        }
        if (started_at_end <= 1 && m->t > 0) {
            report("a retry with at most one publish in the window", m);
        }
    } else {
        s_fallbacks++;
        if (!same(out, &OCB_NEUTRAL)) {
            report("fallback is not NEUTRAL", m);
        }
        if (started_at_end < 2) {
            report("NEUTRAL with fewer than two publishes in the window", m);
        }
    }
}

static void explore(model m)
{
    const int writer_left = m.w < 4 * s_k;
    /* Option 1: the writer moves. */
    if (writer_left) {
        model n = m;
        writer_step(&n);
        explore(n);
    }
    /* Option 2: the reader moves. */
    switch (m.r) {
    case 0:
        if (m.t == 0) {
            m.w0 = m.w;
        }
        m.g0 = ocb_pubstate_try_begin(&m.ps);
        m.r = 1;
        break;
    case 1:
        ocb_pubstate_try_copy(&m.ps, m.g0, &m.c, 0);
        m.r = 2;
        break;
    case 2:
        ocb_pubstate_try_copy(&m.ps, m.g0, &m.c, 1);
        m.r = 3;
        break;
    default:
        if (ocb_pubstate_try_end(&m.ps, m.g0)) {
            leaf(&m, 1, &m.c);
            return;
        }
        m.t++;
        m.r = 0;
        if (m.t >= (int)OCB_PUBSTATE_TRIES) {
            leaf(&m, 0, &OCB_NEUTRAL);
            return;
        }
        break;
    }
    explore(m);
}

static void test_interleavings(void)
{
    fill_states();
    for (s_k = 1; s_k <= MAXK; s_k++) {
        model m;
        memset(&m, 0, sizeof m);
        ocb_pubstate_init(&m.ps);
        ocb_pubstate_write(&m.ps, &s_pub[0]); /* the state before the read */
        s_leaves = 0u;
        s_fallbacks = 0u;
        s_retried = 0u;
        explore(m);
        printf("  k=%d publishes in the read: %lu interleavings, %lu with a retry, %lu NEUTRAL\n", s_k, s_leaves,
               s_retried, s_fallbacks);
        if (s_k <= 3) {
            OCB_CHECK(s_fallbacks == 0u, "k=%d: %lu fallbacks (3 tries must absorb 3 publishes)", s_k, s_fallbacks);
        } else {
            OCB_CHECK(s_fallbacks > 0u, "k=4: the NEUTRAL fallback was never reached (test not exhaustive?)");
        }
        if (s_k >= 2) {
            OCB_CHECK(s_retried > 0u, "k=%d: no interleaving needed a retry (test not exhaustive?)", s_k);
        }
    }
}

/* ------------------------------------------------------------------ */
/* The rest.                                                          */
/* ------------------------------------------------------------------ */

static void test_round_trips(void)
{
    static ocb_pubstate ps;
    ocb_state got;
    ocb_pubstate_init(&ps);
    OCB_CHECK(ocb_pubstate_read(&ps, &got) && same(&got, &OCB_NEUTRAL) && ps.gen == 0u, "init is NEUTRAL");
    /* D-pad down as the bench sends it: 00 00 04 80 80 80 80 00. */
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
    OCB_CHECK((ps.gen & 1u) == 0u && ps.gen == 2u * 100001u, "gen counts two per publish: %u", ps.gen);
    OCB_CHECK(ocb_pubstate_retries(&ps) == 0u && ocb_pubstate_fallbacks(&ps) == 0u, "no retry without a writer");
}

/* Story 2.3 H.15 attempt 3, press 4: a writer that rewrites every pass, so
 * every read overlaps a write. The single slot fell back three times in a
 * row; the double buffer must never fall back or even retry when one write
 * overlaps each try. */
static void test_bench_blip(void)
{
    static ocb_pubstate ps;
    ocb_pubstate_init(&ps);
    const ocb_state a = {OCB_BTN_A, OCB_HAT_NEUTRAL, 0x80u, 0x80u, 0x80u, 0x80u, 0u};
    ocb_pubstate_write(&ps, &a);
    int bad = 0;
    for (int i = 0; i < 10000; i++) {
        /* The writer is in the middle of rewriting A when the read starts. */
        ocb_pubstate_write_begin(&ps);
        ocb_pubstate_write_fields(&ps, &a, 0);
        ocb_state c;
        const uint32_t g0 = ocb_pubstate_try_begin(&ps);
        ocb_pubstate_try_copy(&ps, g0, &c, 0);
        ocb_pubstate_write_fields(&ps, &a, 1);
        ocb_pubstate_write_end(&ps);
        ocb_pubstate_try_copy(&ps, g0, &c, 1);
        if (!ocb_pubstate_try_end(&ps, g0) || !same(&c, &a)) {
            bad++;
        }
        ocb_state got;
        if (!ocb_pubstate_read(&ps, &got) || !same(&got, &a)) {
            bad++;
        }
    }
    OCB_CHECK(bad == 0, "the held press blipped %d times", bad);
    OCB_CHECK(ocb_pubstate_fallbacks(&ps) == 0u && ocb_pubstate_retries(&ps) == 0u, "fallbacks %u retries %u",
              ocb_pubstate_fallbacks(&ps), ocb_pubstate_retries(&ps));
}

static void test_publish_and_counters(void)
{
    static ocb_pubstate ps;
    ocb_state got;
    const ocb_state down = {0x0000u, OCB_HAT_DOWN, 0x80u, 0x80u, 0x80u, 0x80u, 0u};
    const ocb_state a = {OCB_BTN_A, OCB_HAT_NEUTRAL, 0x80u, 0x80u, 0x80u, 0x80u, 0u};

    /* Write on change: 1000 passes with the same publish_count. */
    ocb_pubstate_init(&ps);
    OCB_CHECK(ocb_pubstate_publish(&ps, &down, 7u), "first publish writes");
    const uint32_t gen_after = ps.gen;
    int rewrote = 0;
    for (int i = 0; i < 1000; i++) {
        rewrote += ocb_pubstate_publish(&ps, &down, 7u) ? 1 : 0;
    }
    OCB_CHECK(rewrote == 0 && ps.gen == gen_after, "same count rewrote %d times", rewrote);
    OCB_CHECK(ocb_pubstate_publish(&ps, &a, 8u) && ocb_pubstate_read(&ps, &got) && same(&got, &a), "new count");
    OCB_CHECK(ocb_pubstate_publish(&ps, &down, 0u), "a wrapped count still publishes");

    /* The generation wraps without a false rejection. */
    ps.gen = 0xFFFFFFFCu;
    ocb_pubstate_write(&ps, &a);
    ocb_pubstate_write(&ps, &down);
    OCB_CHECK(ps.gen == 0u && ocb_pubstate_read(&ps, &got) && same(&got, &down), "gen wrap");
    /* A try that straddles the wrap. */
    ps.gen = 0xFFFFFFFEu;
    ocb_pubstate_write(&ps, &a);
    const uint32_t g0 = ocb_pubstate_try_begin(&ps);
    ocb_pubstate_write(&ps, &down); /* one publish across the wrap: still accepted */
    OCB_CHECK(ocb_pubstate_try_end(&ps, g0), "one publish across the wrap accepted");

    /* The reader's counters: a first-try read counts nothing, a retried
     * read counts a retry, a fallback counts both; both saturate. */
    ocb_pubstate_init(&ps);
    ocb_pubstate_note_read(&ps, 1u, true);
    OCB_CHECK(ocb_pubstate_retries(&ps) == 0u && ocb_pubstate_fallbacks(&ps) == 0u, "first try counts nothing");
    ocb_pubstate_note_read(&ps, 2u, true);
    OCB_CHECK(ocb_pubstate_retries(&ps) == 1u && ocb_pubstate_fallbacks(&ps) == 0u, "second try: one retry");
    ocb_pubstate_note_read(&ps, OCB_PUBSTATE_TRIES, false);
    OCB_CHECK(ocb_pubstate_retries(&ps) == 2u && ocb_pubstate_fallbacks(&ps) == 1u, "fallback: both");
    ps.retries = UINT32_MAX - 1u;
    ps.fallbacks = UINT32_MAX - 1u;
    for (int i = 0; i < 3; i++) {
        ocb_pubstate_note_read(&ps, OCB_PUBSTATE_TRIES, false);
    }
    OCB_CHECK(ocb_pubstate_retries(&ps) == UINT32_MAX && ocb_pubstate_fallbacks(&ps) == UINT32_MAX,
              "counters saturate, no wrap: %u %u", ocb_pubstate_retries(&ps), ocb_pubstate_fallbacks(&ps));
}

int main(void)
{
    test_round_trips();
    test_interleavings();
    test_bench_blip();
    test_publish_and_counters();
    const int f = ocb_test_failures();
    if (f != 0) {
        fprintf(stderr, "test_link_pubstate (F.33): %d failure(s)\n", f);
        return 1;
    }
    printf("test_link_pubstate (F.33): ok\n");
    return 0;
}
