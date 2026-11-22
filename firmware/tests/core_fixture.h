/*
 * SPDX-License-Identifier: MIT
 *
 * Shared harness for the controller core host tests (story 2.4 Task 6.1).
 * The "fake clock" is h->now: the core never reads time, every call gets
 * it in ocb_core_env, so a test advances time by changing h->now. Frame
 * bodies are built with the ocb_proto encoders, so the tests stay tied to
 * the Story 2.2 wire format. Plain C, no framework.
 */
#ifndef OCB_CORE_FIXTURE_H
#define OCB_CORE_FIXTURE_H

#include <stdbool.h>
#include <stdint.h>
#include <string.h>

#include "ocb_core/ocb_core.h"
#include "ocb_proto/ocb_proto.h"
#include "ocb_test.h"

#define H_CAP_MAX 512u

typedef struct {
    ocb_core core;
    ocb_core_event storage[H_CAP_MAX];
    uint64_t now;
    bool ready;
    uint8_t seq; /* seq of the next frame; incremented by every frame helper */
} harness;

static inline void h_init(harness *h, uint16_t cap, uint16_t report_period_us)
{
    memset(h, 0, sizeof *h);
    h->ready = true;
    h->now = 1000000u; /* start away from 0 so a lead and t0 are visible */
    const int rc = ocb_core_init(&h->core, h->storage, cap, report_period_us);
    OCB_CHECK(rc == 0, "ocb_core_init(cap %u) returned %d", (unsigned)cap, rc);
}

static inline ocb_core_env h_env(const harness *h)
{
    ocb_core_env e;
    e.now_us = h->now;
    e.ready = h->ready;
    return e;
}

static inline void h_advance(harness *h, uint64_t us)
{
    h->now += us;
}

/* Any frame, body as given (len may disagree with the message type). */
static inline ocb_core_out h_raw(harness *h, uint8_t type, const uint8_t *body, uint8_t len)
{
    ocb_frame f;
    memset(&f, 0, sizeof f);
    f.type = type;
    f.seq = h->seq;
    f.len = len;
    if (len > 0u && body != NULL) {
        memcpy(f.body, body, len);
    }
    h->seq = (uint8_t)(h->seq + 1u);
    const ocb_core_env e = h_env(h);
    ocb_core_out out;
    memset(&out, 0xEE, sizeof out); /* the core must overwrite every field */
    ocb_core_frame(&h->core, &e, &f, &out);
    return out;
}

static inline ocb_core_out h_hello(harness *h)
{
    ocb_msg_hello m = {OCB_PROTO_VERSION};
    uint8_t body[OCB_MAX_BODY];
    const int n = ocb_msg_hello_encode(&m, body, sizeof body);
    return h_raw(h, OCB_MSG_HELLO, body, (uint8_t)n);
}

static inline ocb_core_out h_stop(harness *h)
{
    return h_raw(h, OCB_MSG_STOP, NULL, 0u);
}

static inline ocb_core_out h_ping(harness *h)
{
    return h_raw(h, OCB_MSG_PING, NULL, 0u);
}

static inline ocb_core_out h_set_state(harness *h, ocb_state s)
{
    ocb_msg_set_state m;
    m.state = s;
    uint8_t body[OCB_MAX_BODY];
    const int n = ocb_msg_set_state_encode(&m, body, sizeof body);
    return h_raw(h, OCB_MSG_SET_STATE, body, (uint8_t)n);
}

static inline ocb_core_out h_queue_event(harness *h, uint32_t t_us, ocb_state s)
{
    ocb_msg_queue_event m;
    m.t_us = t_us;
    m.state = s;
    uint8_t body[OCB_MAX_BODY];
    const int n = ocb_msg_queue_event_encode(&m, body, sizeof body);
    return h_raw(h, OCB_MSG_QUEUE_EVENT, body, (uint8_t)n);
}

static inline ocb_core_out h_start(harness *h, uint32_t lead_us)
{
    ocb_msg_start m = {lead_us};
    uint8_t body[OCB_MAX_BODY];
    const int n = ocb_msg_start_encode(&m, body, sizeof body);
    return h_raw(h, OCB_MSG_START, body, (uint8_t)n);
}

static inline ocb_core_out h_tick(harness *h)
{
    const ocb_core_env e = h_env(h);
    ocb_core_out out;
    memset(&out, 0xEE, sizeof out);
    ocb_core_tick(&h->core, &e, &out);
    return out;
}

static inline ocb_core_out h_abort(harness *h, ocb_core_abort_cause cause)
{
    ocb_core_out out;
    memset(&out, 0xEE, sizeof out);
    ocb_core_abort(&h->core, cause, &out);
    return out;
}

/* States used across the tests. */
static inline ocb_state st_neutral(void)
{
    return OCB_NEUTRAL;
}

static inline ocb_state st_end(void)
{
    ocb_state s = OCB_NEUTRAL;
    s.flags = OCB_STATE_FLAG_END;
    return s;
}

static inline ocb_state st_buttons(uint16_t buttons)
{
    ocb_state s = OCB_NEUTRAL;
    s.buttons = buttons;
    return s;
}

static inline bool st_eq(const ocb_state *a, const ocb_state *b)
{
    return a->buttons == b->buttons && a->hat == b->hat && a->lx == b->lx && a->ly == b->ly &&
           a->rx == b->rx && a->ry == b->ry && a->flags == b->flags;
}

static inline bool h_published_neutral(const harness *h)
{
    return ocb_state_is_neutral(&h->core.published);
}

/* Invariants after every call (F.3, F.12): IDLE implies NEUTRAL published;
 * the latch implies IDLE with an empty queue (IDLE itself may hold a
 * prefill, AD-8, so "empty" is checked at the moment IDLE is entered);
 * flags are never published; free + count == cap. */
static inline bool h_invariants_ok(const harness *h)
{
    const ocb_core *c = &h->core;
    if (c->mode == OCB_MODE_IDLE && !ocb_state_is_neutral(&c->published)) {
        return false;
    }
    if (c->aborted && (c->mode != OCB_MODE_IDLE || ocb_queue_count(&c->queue) != 0u)) {
        return false;
    }
    if ((c->published.flags) != 0u) {
        return false;
    }
    return ocb_queue_free(&c->queue) + ocb_queue_count(&c->queue) == c->queue.cap;
}

/* Puts the core in `mode` with two events queued far in the future (when
 * the mode allows it): IDLE (prefilled), DIRECT (B pressed, prefilled) or
 * PLAYING (started with no lead, nothing due yet). */
static inline void h_enter_mode(harness *h, uint8_t mode)
{
    const bool ready = h->ready;
    h->ready = true;
    ocb_core_out o = h_queue_event(h, 1000000u, st_buttons(OCB_BTN_A));
    OCB_CHECK(o.kind == OCB_CORE_REPLY_ACK, "setup: queue event 1");
    o = h_queue_event(h, 2000000u, st_buttons(OCB_BTN_X));
    OCB_CHECK(o.kind == OCB_CORE_REPLY_ACK, "setup: queue event 2");
    if (mode == OCB_MODE_DIRECT) {
        o = h_set_state(h, st_buttons(OCB_BTN_B));
        OCB_CHECK(o.kind == OCB_CORE_REPLY_ACK, "setup: set state");
    } else if (mode == OCB_MODE_PLAYING) {
        o = h_start(h, 0u);
        OCB_CHECK(o.kind == OCB_CORE_REPLY_ACK, "setup: start");
    }
    OCB_CHECK(h->core.mode == mode, "setup: mode %u, wanted %u", (unsigned)h->core.mode, (unsigned)mode);
    h->ready = ready;
}

static inline const char *mode_name(uint8_t mode)
{
    switch (mode) {
    case OCB_MODE_IDLE:
        return "IDLE";
    case OCB_MODE_DIRECT:
        return "DIRECT";
    case OCB_MODE_PLAYING:
        return "PLAYING";
    default:
        return "?";
    }
}

#endif /* OCB_CORE_FIXTURE_H */
