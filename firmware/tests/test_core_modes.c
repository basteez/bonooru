/*
 * SPDX-License-Identifier: MIT
 *
 * Host test F.3 (story 2.4): the AD-8 mode table as a hand-written matrix
 * (8 frame kinds x 3 modes x latch x ready), entering IDLE, and the abort
 * latch (every cause from every mode, NAK ABORTED, cleared only by STOP and
 * HELLO, abort_count and last_event rules of D5). Story 2.7 adds the
 * NAK NO_LINK rows (D11): link down in every mode, the precedence pairs, and
 * that NO_LINK is semantic with no effect. The matrix itself runs with the
 * link up (the harness default).
 */
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "core_fixture.h"

enum frame_kind { F_SET_STATE, F_QUEUE_EVENT, F_START, F_STOP, F_HELLO, F_PING, F_UNASSIGNED, F_BRIDGE };

static const char *const frame_names[] = {"SET_STATE", "QUEUE_EVENT", "START", "STOP",
                                          "HELLO",     "PING",        "0x07",  "0x40"};

#define I OCB_MODE_IDLE
#define D OCB_MODE_DIRECT
#define P OCB_MODE_PLAYING
#define R_NONE OCB_CORE_REPLY_NONE
#define R_ACK OCB_CORE_REPLY_ACK
#define R_NAK OCB_CORE_REPLY_NAK
#define R_HELLO OCB_CORE_REPLY_HELLO

typedef struct {
    enum frame_kind frame;
    uint8_t mode;    /* mode before the frame (and before the abort, when aborted) */
    bool aborted;    /* abort latch set (by LINK_LOST) before the frame */
    bool ready;      /* env.ready during the frame */
    ocb_core_reply_kind kind;
    uint8_t nak;     /* expected NAK reason, 0 when not a NAK */
    uint8_t mode_after;
    bool aborted_after;
    bool advance;    /* expected == seq + 1 afterwards */
} cell;

/* Written by hand from the AD-8 table, D3 precedence and D14. An aborted
 * core is always IDLE (the abort entered IDLE), whatever mode it was in. */
static const cell matrix[] = {
    /* SET_STATE */
    {F_SET_STATE, I, false, true, R_ACK, 0, D, false, true},
    {F_SET_STATE, I, false, false, R_NAK, OCB_NAK_NOT_READY, I, false, true},
    {F_SET_STATE, D, false, true, R_ACK, 0, D, false, true},
    {F_SET_STATE, D, false, false, R_NAK, OCB_NAK_NOT_READY, D, false, true},
    {F_SET_STATE, P, false, true, R_NAK, OCB_NAK_BUSY, P, false, true},
    {F_SET_STATE, P, false, false, R_NAK, OCB_NAK_BUSY, P, false, true},
    {F_SET_STATE, I, true, true, R_NAK, OCB_NAK_ABORTED, I, true, true},
    {F_SET_STATE, I, true, false, R_NAK, OCB_NAK_ABORTED, I, true, true},
    {F_SET_STATE, D, true, true, R_NAK, OCB_NAK_ABORTED, I, true, true},
    {F_SET_STATE, D, true, false, R_NAK, OCB_NAK_ABORTED, I, true, true},
    {F_SET_STATE, P, true, true, R_NAK, OCB_NAK_ABORTED, I, true, true},
    {F_SET_STATE, P, true, false, R_NAK, OCB_NAK_ABORTED, I, true, true},
    /* QUEUE_EVENT: accepted in every mode, ready or not */
    {F_QUEUE_EVENT, I, false, true, R_ACK, 0, I, false, true},
    {F_QUEUE_EVENT, I, false, false, R_ACK, 0, I, false, true},
    {F_QUEUE_EVENT, D, false, true, R_ACK, 0, D, false, true},
    {F_QUEUE_EVENT, D, false, false, R_ACK, 0, D, false, true},
    {F_QUEUE_EVENT, P, false, true, R_ACK, 0, P, false, true},
    {F_QUEUE_EVENT, P, false, false, R_ACK, 0, P, false, true},
    {F_QUEUE_EVENT, I, true, true, R_NAK, OCB_NAK_ABORTED, I, true, true},
    {F_QUEUE_EVENT, I, true, false, R_NAK, OCB_NAK_ABORTED, I, true, true},
    {F_QUEUE_EVENT, D, true, true, R_NAK, OCB_NAK_ABORTED, I, true, true},
    {F_QUEUE_EVENT, D, true, false, R_NAK, OCB_NAK_ABORTED, I, true, true},
    {F_QUEUE_EVENT, P, true, true, R_NAK, OCB_NAK_ABORTED, I, true, true},
    {F_QUEUE_EVENT, P, true, false, R_NAK, OCB_NAK_ABORTED, I, true, true},
    /* START */
    {F_START, I, false, true, R_ACK, 0, P, false, true},
    {F_START, I, false, false, R_NAK, OCB_NAK_NOT_READY, I, false, true},
    {F_START, D, false, true, R_ACK, 0, P, false, true},
    {F_START, D, false, false, R_NAK, OCB_NAK_NOT_READY, D, false, true},
    {F_START, P, false, true, R_NAK, OCB_NAK_BUSY, P, false, true},
    {F_START, P, false, false, R_NAK, OCB_NAK_BUSY, P, false, true},
    {F_START, I, true, true, R_NAK, OCB_NAK_ABORTED, I, true, true},
    {F_START, I, true, false, R_NAK, OCB_NAK_ABORTED, I, true, true},
    {F_START, D, true, true, R_NAK, OCB_NAK_ABORTED, I, true, true},
    {F_START, D, true, false, R_NAK, OCB_NAK_ABORTED, I, true, true},
    {F_START, P, true, true, R_NAK, OCB_NAK_ABORTED, I, true, true},
    {F_START, P, true, false, R_NAK, OCB_NAK_ABORTED, I, true, true},
    /* STOP: always ACK, IDLE, latch cleared */
    {F_STOP, I, false, true, R_ACK, 0, I, false, true},
    {F_STOP, I, false, false, R_ACK, 0, I, false, true},
    {F_STOP, D, false, true, R_ACK, 0, I, false, true},
    {F_STOP, D, false, false, R_ACK, 0, I, false, true},
    {F_STOP, P, false, true, R_ACK, 0, I, false, true},
    {F_STOP, P, false, false, R_ACK, 0, I, false, true},
    {F_STOP, I, true, true, R_ACK, 0, I, false, true},
    {F_STOP, I, true, false, R_ACK, 0, I, false, true},
    {F_STOP, D, true, true, R_ACK, 0, I, false, true},
    {F_STOP, D, true, false, R_ACK, 0, I, false, true},
    {F_STOP, P, true, true, R_ACK, 0, I, false, true},
    {F_STOP, P, true, false, R_ACK, 0, I, false, true},
    /* HELLO: always the HELLO reply (INFO by the link layer), IDLE, latch cleared */
    {F_HELLO, I, false, true, R_HELLO, 0, I, false, true},
    {F_HELLO, I, false, false, R_HELLO, 0, I, false, true},
    {F_HELLO, D, false, true, R_HELLO, 0, I, false, true},
    {F_HELLO, D, false, false, R_HELLO, 0, I, false, true},
    {F_HELLO, P, false, true, R_HELLO, 0, I, false, true},
    {F_HELLO, P, false, false, R_HELLO, 0, I, false, true},
    {F_HELLO, I, true, true, R_HELLO, 0, I, false, true},
    {F_HELLO, I, true, false, R_HELLO, 0, I, false, true},
    {F_HELLO, D, true, true, R_HELLO, 0, I, false, true},
    {F_HELLO, D, true, false, R_HELLO, 0, I, false, true},
    {F_HELLO, P, true, true, R_HELLO, 0, I, false, true},
    {F_HELLO, P, true, false, R_HELLO, 0, I, false, true},
    /* PING: ACK, no state change, does not clear the latch */
    {F_PING, I, false, true, R_ACK, 0, I, false, true},
    {F_PING, I, false, false, R_ACK, 0, I, false, true},
    {F_PING, D, false, true, R_ACK, 0, D, false, true},
    {F_PING, D, false, false, R_ACK, 0, D, false, true},
    {F_PING, P, false, true, R_ACK, 0, P, false, true},
    {F_PING, P, false, false, R_ACK, 0, P, false, true},
    {F_PING, I, true, true, R_ACK, 0, I, true, true},
    {F_PING, I, true, false, R_ACK, 0, I, true, true},
    {F_PING, D, true, true, R_ACK, 0, I, true, true},
    {F_PING, D, true, false, R_ACK, 0, I, true, true},
    {F_PING, P, true, true, R_ACK, 0, I, true, true},
    {F_PING, P, true, false, R_ACK, 0, I, true, true},
    /* Unassigned app type 0x07: NAK UNKNOWN_TYPE (semantic, advances) */
    {F_UNASSIGNED, I, false, true, R_NAK, OCB_NAK_UNKNOWN_TYPE, I, false, true},
    {F_UNASSIGNED, I, false, false, R_NAK, OCB_NAK_UNKNOWN_TYPE, I, false, true},
    {F_UNASSIGNED, D, false, true, R_NAK, OCB_NAK_UNKNOWN_TYPE, D, false, true},
    {F_UNASSIGNED, D, false, false, R_NAK, OCB_NAK_UNKNOWN_TYPE, D, false, true},
    {F_UNASSIGNED, P, false, true, R_NAK, OCB_NAK_UNKNOWN_TYPE, P, false, true},
    {F_UNASSIGNED, P, false, false, R_NAK, OCB_NAK_UNKNOWN_TYPE, P, false, true},
    {F_UNASSIGNED, I, true, true, R_NAK, OCB_NAK_UNKNOWN_TYPE, I, true, true},
    {F_UNASSIGNED, I, true, false, R_NAK, OCB_NAK_UNKNOWN_TYPE, I, true, true},
    {F_UNASSIGNED, D, true, true, R_NAK, OCB_NAK_UNKNOWN_TYPE, I, true, true},
    {F_UNASSIGNED, D, true, false, R_NAK, OCB_NAK_UNKNOWN_TYPE, I, true, true},
    {F_UNASSIGNED, P, true, true, R_NAK, OCB_NAK_UNKNOWN_TYPE, I, true, true},
    {F_UNASSIGNED, P, true, false, R_NAK, OCB_NAK_UNKNOWN_TYPE, I, true, true},
    /* Bridge type 0x40: never reaches the core; if it does, nothing happens */
    {F_BRIDGE, I, false, true, R_NONE, 0, I, false, false},
    {F_BRIDGE, I, false, false, R_NONE, 0, I, false, false},
    {F_BRIDGE, D, false, true, R_NONE, 0, D, false, false},
    {F_BRIDGE, D, false, false, R_NONE, 0, D, false, false},
    {F_BRIDGE, P, false, true, R_NONE, 0, P, false, false},
    {F_BRIDGE, P, false, false, R_NONE, 0, P, false, false},
    {F_BRIDGE, I, true, true, R_NONE, 0, I, true, false},
    {F_BRIDGE, I, true, false, R_NONE, 0, I, true, false},
    {F_BRIDGE, D, true, true, R_NONE, 0, I, true, false},
    {F_BRIDGE, D, true, false, R_NONE, 0, I, true, false},
    {F_BRIDGE, P, true, true, R_NONE, 0, I, true, false},
    {F_BRIDGE, P, true, false, R_NONE, 0, I, true, false},
};

/* The SET_STATE of the matrix: buttons and a right stick the mask would
 * clear, to show DIRECT publishes it as sent. */
static ocb_state matrix_set_state(void)
{
    ocb_state s = st_buttons((uint16_t)(OCB_BTN_A | OCB_BTN_ZL | OCB_BTN_HOME));
    s.rx = 0x10;
    s.ry = 0xF0;
    return s;
}

static ocb_core_out send_kind(harness *h, enum frame_kind k)
{
    switch (k) {
    case F_SET_STATE:
        return h_set_state(h, matrix_set_state());
    case F_QUEUE_EVENT:
        return h_queue_event(h, 3000000u, st_buttons(OCB_BTN_Y));
    case F_START:
        return h_start(h, 0u);
    case F_STOP:
        return h_stop(h);
    case F_HELLO:
        return h_hello(h);
    case F_PING:
        return h_ping(h);
    case F_UNASSIGNED:
        return h_raw(h, 0x07, NULL, 0u);
    case F_BRIDGE:
    default: {
        const uint8_t hb[OCB_BODY_LEN_HEARTBEAT] = {0, 1, 0, OCB_HB_FLAG_DTR};
        return h_raw(h, OCB_MSG_HEARTBEAT, hb, (uint8_t)sizeof hb);
    }
    }
}

static void test_matrix(void)
{
    const size_t n = sizeof matrix / sizeof matrix[0];
    OCB_CHECK(n == 8u * 3u * 2u * 2u, "matrix has %zu cells, wanted 96", n);
    for (size_t i = 0; i < n; i++) {
        const cell *c = &matrix[i];
        harness h;
        h_init(&h, 16u, 1000u);
        h.seq = (uint8_t)(40u + i); /* any seq: the core does not check it */
        h_enter_mode(&h, c->mode);
        if (c->aborted) {
            (void)h_abort(&h, OCB_ABORT_LINK_LOST);
        }
        const ocb_state before_pub = h.core.published;
        const uint16_t before_count = ocb_queue_count(&h.core.queue);
        const uint8_t before_expected = h.core.expected;
        h.ready = c->ready;
        const uint8_t seq = h.seq;
        const ocb_core_out o = send_kind(&h, c->frame);

        const char *fn = frame_names[c->frame];
        const char *mn = mode_name(c->mode);
        OCB_CHECK(o.kind == c->kind, "cell %zu %s/%s/ab%d/rdy%d: reply kind %d, wanted %d", i, fn, mn,
                  c->aborted, c->ready, (int)o.kind, (int)c->kind);
        if (c->kind == R_NAK) {
            OCB_CHECK(o.nak_reason == c->nak, "cell %zu %s/%s/ab%d/rdy%d: NAK 0x%02x, wanted 0x%02x", i, fn,
                      mn, c->aborted, c->ready, o.nak_reason, c->nak);
        }
        OCB_CHECK(o.event == OCB_EVT_NONE, "cell %zu %s/%s: a frame emitted event %u", i, fn, mn, o.event);
        OCB_CHECK(h.core.mode == c->mode_after, "cell %zu %s/%s/ab%d/rdy%d: mode %s, wanted %s", i, fn, mn,
                  c->aborted, c->ready, mode_name(h.core.mode), mode_name(c->mode_after));
        OCB_CHECK(h.core.aborted == c->aborted_after, "cell %zu %s/%s/ab%d/rdy%d: aborted %d, wanted %d", i,
                  fn, mn, c->aborted, c->ready, h.core.aborted, c->aborted_after);
        const uint8_t want_expected = c->advance ? (uint8_t)(seq + 1u) : before_expected;
        OCB_CHECK(h.core.expected == want_expected, "cell %zu %s/%s: expected %u, wanted %u", i, fn, mn,
                  h.core.expected, want_expected);
        OCB_CHECK(h_invariants_ok(&h), "cell %zu %s/%s: invariants broken", i, fn, mn);

        /* Published state per resulting mode. */
        if (c->mode_after == I || c->mode_after == P) {
            OCB_CHECK(h_published_neutral(&h), "cell %zu %s/%s: published not NEUTRAL", i, fn, mn);
        } else if (c->frame == F_SET_STATE && c->kind == R_ACK) {
            const ocb_state want = matrix_set_state();
            OCB_CHECK(st_eq(&h.core.published, &want), "cell %zu: SET_STATE not published unmasked", i);
        } else {
            OCB_CHECK(st_eq(&h.core.published, &before_pub), "cell %zu %s/%s: published changed", i, fn, mn);
        }
        /* Queue: one more after an accepted QUEUE_EVENT, empty in IDLE. */
        if (c->frame == F_QUEUE_EVENT && c->kind == R_ACK) {
            OCB_CHECK(ocb_queue_count(&h.core.queue) == before_count + 1u, "cell %zu: queue not grown", i);
        } else if (c->mode_after != I) {
            OCB_CHECK(ocb_queue_count(&h.core.queue) == before_count, "cell %zu %s/%s: queue changed", i, fn,
                      mn);
        }
    }
}

/* The EVENT per cause, by hand from D5. */
static const struct {
    ocb_core_abort_cause cause;
    const char *name;
    uint8_t event;
} causes[] = {
    {OCB_ABORT_LINK_LOST, "LINK_LOST", OCB_EVT_LINK_LOST},
    {OCB_ABORT_APP_TIMEOUT, "APP_TIMEOUT", OCB_EVT_NONE},
    {OCB_ABORT_USB_SUSPEND, "USB_SUSPEND", OCB_EVT_USB_SUSPENDED},
    {OCB_ABORT_USB_UNMOUNT, "USB_UNMOUNT", OCB_EVT_USB_SUSPENDED},
    {OCB_ABORT_USB_BUS_RESET, "USB_BUS_RESET", OCB_EVT_USB_SUSPENDED},
    {OCB_ABORT_CORE_STALL, "CORE_STALL", OCB_EVT_NONE},
    {OCB_ABORT_END, "END", OCB_EVT_PLAYBACK_DONE},
    {OCB_ABORT_UNDERRUN, "UNDERRUN", OCB_EVT_UNDERRUN},
    {OCB_ABORT_LATE, "LATE", OCB_EVT_LATE},
};

static void test_latch_every_cause_every_mode(void)
{
    const uint8_t modes[] = {I, D, P};
    OCB_CHECK(sizeof causes / sizeof causes[0] == (size_t)OCB_ABORT_CAUSE_COUNT, "cause table incomplete");
    for (size_t ci = 0; ci < sizeof causes / sizeof causes[0]; ci++) {
        for (size_t mi = 0; mi < sizeof modes; mi++) {
            const char *cn = causes[ci].name;
            const char *mn = mode_name(modes[mi]);
            harness h;
            h_init(&h, 16u, 1000u);
            h_enter_mode(&h, modes[mi]);
            if (modes[mi] == P) {
                /* A held note is published before the abort. */
                h.now += 1000000u;
                (void)h_tick(&h);
                OCB_CHECK(h.core.published.buttons == OCB_BTN_A, "%s/%s: setup note not published", cn, mn);
            }
            h.core.last_event = 0x77; /* marker: kept by the causes without an EVENT */
            ocb_core_out o = h_abort(&h, causes[ci].cause);
            OCB_CHECK(o.kind == R_NONE, "%s/%s: abort reply kind %d", cn, mn, (int)o.kind);
            OCB_CHECK(o.event == causes[ci].event, "%s/%s: event %u, wanted %u", cn, mn, o.event,
                      causes[ci].event);
            const uint8_t want_last = causes[ci].event == OCB_EVT_NONE ? 0x77 : causes[ci].event;
            OCB_CHECK(h.core.last_event == want_last, "%s/%s: last_event %u, wanted %u", cn, mn,
                      h.core.last_event, want_last);
            OCB_CHECK(h.core.aborted, "%s/%s: latch not set", cn, mn);
            OCB_CHECK(h.core.abort_count == 1u, "%s/%s: abort_count %u", cn, mn, h.core.abort_count);
            OCB_CHECK(h.core.mode == I, "%s/%s: not IDLE", cn, mn);
            OCB_CHECK(h_published_neutral(&h), "%s/%s: not NEUTRAL", cn, mn);
            OCB_CHECK(ocb_queue_count(&h.core.queue) == 0u, "%s/%s: queue not cleared", cn, mn);
            OCB_CHECK(!h.core.have_last_t && !h.core.end_queued, "%s/%s: ordering baseline not reset", cn, mn);

            /* NAK ABORTED for the three frames, ready or not. */
            o = h_queue_event(&h, 5u, st_neutral());
            OCB_CHECK(o.kind == R_NAK && o.nak_reason == OCB_NAK_ABORTED, "%s/%s: QUEUE_EVENT not ABORTED", cn,
                      mn);
            o = h_start(&h, 0u);
            OCB_CHECK(o.kind == R_NAK && o.nak_reason == OCB_NAK_ABORTED, "%s/%s: START not ABORTED", cn, mn);
            o = h_set_state(&h, st_neutral());
            OCB_CHECK(o.kind == R_NAK && o.nak_reason == OCB_NAK_ABORTED, "%s/%s: SET_STATE not ABORTED", cn,
                      mn);
            /* Not cleared by PING, a rejected frame, a tick or a second abort. */
            o = h_ping(&h);
            OCB_CHECK(o.kind == R_ACK && h.core.aborted, "%s/%s: PING cleared the latch", cn, mn);
            const uint8_t bad[3] = {1, 2, 3};
            o = h_raw(&h, OCB_MSG_SET_STATE, bad, 3u);
            OCB_CHECK(h.core.aborted, "%s/%s: rejected frame cleared the latch", cn, mn);
            o = h_raw(&h, 0x07, NULL, 0u);
            OCB_CHECK(h.core.aborted, "%s/%s: unknown type cleared the latch", cn, mn);
            h.now += 10000000u;
            o = h_tick(&h);
            OCB_CHECK(h.core.aborted && o.event == OCB_EVT_NONE, "%s/%s: tick changed the latch", cn, mn);
            for (size_t cj = 0; cj < sizeof causes / sizeof causes[0]; cj++) {
                const uint64_t pc = h.core.publish_count;
                o = h_abort(&h, causes[cj].cause);
                OCB_CHECK(o.event == OCB_EVT_NONE, "%s/%s then %s: second abort emitted %u", cn, mn,
                          causes[cj].name, o.event);
                OCB_CHECK(h.core.abort_count == 1u, "%s/%s then %s: second abort counted", cn, mn,
                          causes[cj].name);
                OCB_CHECK(h.core.last_event == want_last, "%s/%s then %s: last_event changed", cn, mn,
                          causes[cj].name);
                OCB_CHECK(h.core.publish_count == pc, "%s/%s then %s: second abort published", cn, mn,
                          causes[cj].name);
            }
            OCB_CHECK(h.core.aborted && h_invariants_ok(&h), "%s/%s: latch lost", cn, mn);

            /* STOP (or HELLO, alternately) clears it; last_event is kept. */
            if ((ci + mi) % 2u == 0u) {
                o = h_stop(&h);
                OCB_CHECK(o.kind == R_ACK, "%s/%s: STOP reply", cn, mn);
            } else {
                o = h_hello(&h);
                OCB_CHECK(o.kind == R_HELLO, "%s/%s: HELLO reply", cn, mn);
            }
            OCB_CHECK(!h.core.aborted, "%s/%s: STOP/HELLO did not clear the latch", cn, mn);
            OCB_CHECK(h.core.last_event == want_last, "%s/%s: STOP/HELLO changed last_event", cn, mn);
            OCB_CHECK(h.core.abort_count == 1u, "%s/%s: STOP/HELLO changed abort_count", cn, mn);
            o = h_queue_event(&h, 5u, st_neutral());
            OCB_CHECK(o.kind == R_ACK, "%s/%s: QUEUE_EVENT refused after the latch was cleared", cn, mn);
        }
    }
}

static void test_abort_count_wraps(void)
{
    harness h;
    h_init(&h, 4u, 1000u);
    for (unsigned i = 1; i <= 256u; i++) {
        (void)h_abort(&h, OCB_ABORT_CORE_STALL);
        OCB_CHECK(h.core.abort_count == (uint8_t)i, "abort %u: abort_count %u", i, h.core.abort_count);
        (void)h_stop(&h);
    }
    OCB_CHECK(h.core.abort_count == 0u, "abort_count did not wrap to 0");
    OCB_CHECK(h.core.last_event == OCB_EVT_NONE, "CORE_STALL set last_event");
}

/* Entering IDLE by STOP and HELLO from PLAYING with a held note: NEUTRAL,
 * empty queue, ordering baseline reset (an older t_us is accepted again). */
static void test_idle_entry_by_request(void)
{
    for (int use_hello = 0; use_hello <= 1; use_hello++) {
        harness h;
        h_init(&h, 16u, 1000u);
        h_enter_mode(&h, P);
        OCB_CHECK(h_queue_event(&h, 50000000u, st_buttons(OCB_BTN_R)).kind == R_ACK, "queue late event");
        h.now += 1000000u;
        (void)h_tick(&h);
        OCB_CHECK(h.core.published.buttons == OCB_BTN_A && ocb_queue_count(&h.core.queue) == 2u,
                  "setup: note held, two queued");
        const ocb_core_out o = use_hello ? h_hello(&h) : h_stop(&h);
        OCB_CHECK(o.event == OCB_EVT_NONE, "%s emitted an event", use_hello ? "HELLO" : "STOP");
        OCB_CHECK(h.core.mode == I && h_published_neutral(&h) && ocb_queue_count(&h.core.queue) == 0u,
                  "%s: not IDLE/NEUTRAL/empty", use_hello ? "HELLO" : "STOP");
        OCB_CHECK(!h.core.aborted && h.core.abort_count == 0u, "requested stop counted as an abort");
        OCB_CHECK(h_queue_event(&h, 10u, st_neutral()).kind == R_ACK,
                  "%s: t_us below the old baseline refused (ordering not reset)", use_hello ? "HELLO" : "STOP");
    }
}


/* NAK NO_LINK (story 2.7 D11): SET_STATE and START with env.link_ok false. */
static void test_no_link(void)
{
    const uint8_t modes[] = {I, D, P};
    for (size_t mi = 0; mi < sizeof modes; mi++) {
        for (int k = 0; k < 2; k++) {
            const char *mn = mode_name(modes[mi]);
            const char *fn = k == 0 ? "SET_STATE" : "START";
            harness h;
            h_init(&h, 16u, 1000u);
            h_enter_mode(&h, modes[mi]);
            h.link_ok = false;
            const ocb_state before_pub = h.core.published;
            const uint16_t before_count = ocb_queue_count(&h.core.queue);
            const uint32_t before_pc = h.core.publish_count;
            const uint8_t seq = 200u;
            h.seq = seq;
            const ocb_core_out o = k == 0 ? h_set_state(&h, matrix_set_state()) : h_start(&h, 0u);
            OCB_CHECK(o.kind == R_NAK && o.nak_reason == OCB_NAK_NO_LINK, "%s/%s link down: kind %d NAK 0x%02x",
                      fn, mn, (int)o.kind, o.nak_reason);
            OCB_CHECK(o.event == OCB_EVT_NONE, "%s/%s: NO_LINK emitted an event", fn, mn);
            OCB_CHECK(h.core.expected == (uint8_t)(seq + 1u), "%s/%s: NO_LINK is semantic, expected %u", fn, mn,
                      h.core.expected);
            OCB_CHECK(h.core.mode == modes[mi], "%s/%s: NO_LINK changed the mode", fn, mn);
            OCB_CHECK(!h.core.aborted && h.core.abort_count == 0u, "%s/%s: NO_LINK touched the latch", fn, mn);
            OCB_CHECK(ocb_queue_count(&h.core.queue) == before_count, "%s/%s: NO_LINK changed the queue", fn, mn);
            OCB_CHECK(st_eq(&h.core.published, &before_pub) && h.core.publish_count == before_pc,
                      "%s/%s: NO_LINK published", fn, mn);
            OCB_CHECK(h_invariants_ok(&h), "%s/%s: invariants", fn, mn);
        }
    }

    /* Precedence pairs: ABORTED over NO_LINK; NO_LINK over BUSY (PLAYING,
     * above), over NOT_READY and over BAD_STATE. */
    for (int k = 0; k < 2; k++) {
        const char *fn = k == 0 ? "SET_STATE" : "START";
        harness h;
        h_init(&h, 16u, 1000u);
        h_enter_mode(&h, D);
        (void)h_abort(&h, OCB_ABORT_LINK_LOST);
        h.link_ok = false;
        ocb_core_out o = k == 0 ? h_set_state(&h, st_neutral()) : h_start(&h, 0u);
        OCB_CHECK(o.kind == R_NAK && o.nak_reason == OCB_NAK_ABORTED, "%s: ABORTED must precede NO_LINK (0x%02x)",
                  fn, o.nak_reason);

        h_init(&h, 16u, 1000u);
        h_enter_mode(&h, I);
        h.link_ok = false;
        h.ready = false;
        o = k == 0 ? h_set_state(&h, st_neutral()) : h_start(&h, 0u);
        OCB_CHECK(o.kind == R_NAK && o.nak_reason == OCB_NAK_NO_LINK,
                  "%s: NO_LINK must precede NOT_READY (0x%02x)", fn, o.nak_reason);

        /* BAD_STATE: a body of the wrong length for SET_STATE, START with an
         * empty queue. */
        h_init(&h, 16u, 1000u);
        h.link_ok = false;
        if (k == 0) {
            const uint8_t bad[3] = {1, 2, 3};
            o = h_raw(&h, OCB_MSG_SET_STATE, bad, 3u);
        } else {
            o = h_start(&h, 0u);
        }
        OCB_CHECK(o.kind == R_NAK && o.nak_reason == OCB_NAK_NO_LINK,
                  "%s: NO_LINK must precede BAD_STATE (0x%02x)", fn, o.nak_reason);
        /* With the link up the same frame is BAD_STATE: the check above is
         * not vacuous. */
        h.link_ok = true;
        if (k == 0) {
            const uint8_t bad[3] = {1, 2, 3};
            o = h_raw(&h, OCB_MSG_SET_STATE, bad, 3u);
        } else {
            o = h_start(&h, 0u);
        }
        OCB_CHECK(o.kind == R_NAK && o.nak_reason == OCB_NAK_BAD_STATE, "%s: link up control (0x%02x)", fn,
                  o.nak_reason);
    }

    /* A zero-initialised env is "no link" (D11 polarity). */
    {
        harness h;
        h_init(&h, 16u, 1000u);
        h_enter_mode(&h, I);
        ocb_core_env e;
        memset(&e, 0, sizeof e);
        e.ready = true;
        ocb_frame f;
        memset(&f, 0, sizeof f);
        f.type = OCB_MSG_START;
        ocb_msg_start m = {0u};
        f.len = (uint8_t)ocb_msg_start_encode(&m, f.body, sizeof f.body);
        ocb_core_out o;
        ocb_core_frame(&h.core, &e, &f, &o);
        OCB_CHECK(o.kind == R_NAK && o.nak_reason == OCB_NAK_NO_LINK, "zeroed env: START not NO_LINK");
    }
}

int main(void)
{
    test_matrix();
    test_latch_every_cause_every_mode();
    test_abort_count_wraps();
    test_idle_entry_by_request();
    test_no_link();
    const int f = ocb_test_failures();
    if (f != 0) {
        fprintf(stderr, "test_core_modes (F.3): %d failure(s)\n", f);
        return 1;
    }
    printf("test_core_modes (F.3): ok\n");
    return 0;
}
