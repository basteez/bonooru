/*
 * SPDX-License-Identifier: MIT
 *
 * Host test F.17 (story 2.6 Task 7): byte-stream fuzz and a lossy-channel
 * property test of the controller link, under ASan and UBSan.
 *
 * Part 1, fuzz (3 fixed seeds x 200 000 steps): random bytes, valid frames
 * with bit flips, truncations, junk insertions and later duplicates, 0x00
 * runs, 300-byte non-zero runs, error flags, ring overflows, clock jumps,
 * core aborts, USB mailbox events, TX-gate releases, then valid HELLO and
 * traffic again. Invariants after every step: core fields in range (mode,
 * latch implies IDLE and an empty queue, IDLE implies NEUTRAL, queue count
 * <= cap); `expected` changes only when a frame was delivered, and then to
 * that frame's seq + 1; the published state in PLAYING is inside the
 * allow-list; every TX frame is a valid controller frame of an allowed
 * type with a valid body; at most one transport NAK per value of
 * `expected` between re-arms; counters never decrease; and a valid frame
 * sent right after a 0x00 or an error byte is always delivered (resync
 * property, D4).
 *
 * Part 2, lossy channel: a reference sender implementing AD-6 as
 * docs/link-protocol.md describes it (HELLO first, resent every 100 ms
 * until INFO; window min(credit, 64 - unacked); credit = STATUS.free -
 * QUEUE_EVENTs sent after STATUS.last_seq; resend from the oldest unACKed
 * frame after 50 ms; rewind only on a NAK whose `expected` is ahead of the
 * last rewind point) talks to the real link through a FIFO wire at 460800
 * baud that drops frames, flips bits, duplicates frames, inserts junk and
 * adds latency, both ways, at loss rates up to 10 %. Where the doc is
 * silent the model follows story 2.6 D5 to D8: a NAK's `expected` is a
 * cumulative ACK of everything before it; CRC NAKs carry got = 0; any
 * semantic NAK is a test failure here. A test consumer pops the queue (as
 * playback would) so credits come back. Property: every QUEUE_EVENT is
 * enqueued exactly once and in order, the queue never exceeds its
 * capacity, and the run ends within a bounded fake time. Queue 512 and 5.
 *
 * Seeds can be overridden with OCB_FUZZ_SEED (decimal or 0x hex); a failure
 * prints the seed and the step.
 */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#include "link_fixture.h"

#define FUZZ_STEPS 200000u
#define HAND_ALLOWED 0x003Du /* Y | A | X | L | R, by hand */

static uint32_t g_rng;
static uint32_t rnd(void)
{
    g_rng ^= g_rng << 13;
    g_rng ^= g_rng >> 17;
    g_rng ^= g_rng << 5;
    return g_rng;
}
static uint32_t rnd_n(uint32_t n)
{
    return rnd() % n;
}
static bool chance(uint32_t per_10000)
{
    return rnd_n(10000u) < per_10000;
}

/* ---------------------------------------------------------------- part 1 */

typedef struct {
    uint32_t c[13];
} cnt_snap;

static void snap_counters(const lh *h, cnt_snap *s)
{
    const ocb_link_counters *c = ocb_link_counters_get(&h->link);
    s->c[0] = c->rx_bytes;
    s->c[1] = c->rx_err_bytes;
    s->c[2] = c->rx_overflow;
    s->c[3] = c->rx_err_runs;
    s->c[4] = c->breaks;
    s->c[5] = c->overlong_bytes;
    s->c[6] = c->app_frames;
    s->c[7] = c->heartbeats;
    s->c[8] = c->heartbeat_bad;
    s->c[9] = c->nak_suppressed;
    s->c[10] = c->tx_frames;
    s->c[11] = c->tx_dropped;
    s->c[12] = h->link.ring.dropped;
}

typedef struct {
    uint32_t seed;
    uint32_t step;
    bool failed;
    uint8_t last_wire[OCB_MAX_WIRE_FRAME];
    size_t last_len;
    bool rearmed;     /* expected moved or a sync point since the last transport NAK */
    bool have_nak_e;
    uint8_t last_nak_e; /* `expected` field of the last transport NAK */
    uint32_t delivered_checks;
    uint32_t tx_seen;
} fuzz_ctx;

static void fail(fuzz_ctx *z, const char *what)
{
    if (!z->failed) {
        fprintf(stderr, "test_link_fuzz: seed 0x%08x step %u: %s\n", z->seed, z->step, what);
    }
    z->failed = true;
    OCB_CHECK(false, "fuzz invariant: %s", what);
}

static bool body_ok(const lh_out *o)
{
    switch (o->type) {
    case OCB_MSG_INFO: {
        ocb_msg_info m;
        return ocb_msg_info_decode(o->body, o->len, &m) == OCB_OK;
    }
    case OCB_MSG_STATUS: {
        ocb_msg_status m;
        return ocb_msg_status_decode(o->body, o->len, &m) == OCB_OK && (m.flags & OCB_STATUS_RESERVED_MASK) == 0u;
    }
    case OCB_MSG_ACK: {
        ocb_msg_ack m;
        return ocb_msg_ack_decode(o->body, o->len, &m) == OCB_OK;
    }
    case OCB_MSG_NAK: {
        ocb_msg_nak m;
        return ocb_msg_nak_decode(o->body, o->len, &m) == OCB_OK &&
               (OCB_NAK_IS_TRANSPORT(m.reason) || OCB_NAK_IS_SEMANTIC(m.reason));
    }
    case OCB_MSG_EVENT: {
        ocb_msg_event m;
        return ocb_msg_event_decode(o->body, o->len, &m) == OCB_OK && m.code >= OCB_EVT_PLAYBACK_DONE &&
               m.code <= OCB_EVT_USB_SUSPENDED;
    }
    default:
        return false;
    }
}

static void drain_and_check(lh *h, fuzz_ctx *z)
{
    uint8_t wire[OCB_MAX_WIRE_FRAME];
    for (int k = 0; k < 64; k++) {
        const size_t cap = chance(500u) ? (size_t)rnd_n(30u) : sizeof wire; /* small caps keep frames pending */
        const size_t n = ocb_link_tx_next(&h->link, h->now, wire, cap);
        if (n == 0u) {
            if (cap == sizeof wire) {
                break;
            }
            continue;
        }
        if (n > cap) {
            fail(z, "tx_next wrote more than cap");
        }
        lh_out o;
        if (!lh_decode_out(wire, n, &o) || !body_ok(&o)) {
            fail(z, "TX frame invalid or of a type/body the app does not accept");
            continue;
        }
        z->tx_seen++;
        if (o.type == OCB_MSG_NAK && OCB_NAK_IS_TRANSPORT(o.body[0])) {
            if (z->have_nak_e && z->last_nak_e == o.body[1] && !z->rearmed) {
                fail(z, "two transport NAKs for one value of expected");
            }
            z->have_nak_e = true;
            z->last_nak_e = o.body[1];
            z->rearmed = false;
        }
    }
}

static void check_core(const lh *h, fuzz_ctx *z)
{
    const ocb_core *c = &h->core;
    if (c->mode > OCB_MODE_PLAYING) {
        fail(z, "mode out of range");
    }
    if (c->aborted && (c->mode != OCB_MODE_IDLE || ocb_queue_count(&c->queue) != 0u)) {
        fail(z, "latch without IDLE and an empty queue");
    }
    if (c->mode == OCB_MODE_IDLE && !ocb_state_is_neutral(&c->published)) {
        fail(z, "IDLE without NEUTRAL");
    }
    if (ocb_queue_count(&c->queue) > c->queue.cap) {
        fail(z, "queue over capacity");
    }
    if (c->mode == OCB_MODE_PLAYING &&
        ((c->published.buttons & (uint16_t)~HAND_ALLOWED) != 0u || c->published.rx != OCB_STICK_CENTER ||
         c->published.ry != OCB_STICK_CENTER || c->published.flags != 0u)) {
        fail(z, "PLAYING published a state outside the allow-list");
    }
}

static size_t random_body(uint8_t type, uint8_t *body)
{
    if (chance(1500u)) {
        const size_t n = rnd_n(16u);
        for (size_t i = 0; i < n; i++) {
            body[i] = (uint8_t)rnd();
        }
        return n;
    }
    ocb_state s = OCB_NEUTRAL;
    s.buttons = (uint16_t)rnd();
    s.hat = (uint8_t)rnd_n(10u);
    s.lx = (uint8_t)rnd();
    s.rx = (uint8_t)rnd();
    if (chance(500u)) {
        s = lst_end();
    }
    switch (type) {
    case OCB_MSG_HELLO:
        body[0] = chance(8000u) ? (uint8_t)OCB_PROTO_VERSION : (uint8_t)rnd();
        return 1u;
    case OCB_MSG_SET_STATE: {
        ocb_msg_set_state m = {s};
        return (size_t)ocb_msg_set_state_encode(&m, body, OCB_MAX_BODY);
    }
    case OCB_MSG_QUEUE_EVENT:
        return lh_body_queue_event(rnd_n(3000000u), s, body);
    case OCB_MSG_START: {
        ocb_msg_start m = {rnd_n(20000u)};
        return (size_t)ocb_msg_start_encode(&m, body, OCB_MAX_BODY);
    }
    case OCB_MSG_HEARTBEAT: {
        ocb_msg_heartbeat m = {{(uint8_t)rnd(), (uint8_t)rnd(), (uint8_t)rnd()}, (uint8_t)rnd_n(2u)};
        return (size_t)ocb_msg_heartbeat_encode(&m, body, OCB_MAX_BODY);
    }
    default:
        return 0u;
    }
}

static size_t random_frame(const lh *h, uint8_t *wire)
{
    static const uint8_t types[] = {OCB_MSG_HELLO,     OCB_MSG_SET_STATE, OCB_MSG_QUEUE_EVENT, OCB_MSG_QUEUE_EVENT,
                                    OCB_MSG_QUEUE_EVENT, OCB_MSG_START,   OCB_MSG_STOP,        OCB_MSG_PING,
                                    0x07u,             0x3Fu,             OCB_MSG_HEARTBEAT,   0x55u};
    const uint8_t type = types[rnd_n(sizeof types)];
    uint8_t seq;
    const uint32_t r = rnd_n(100u);
    if (r < 55u) {
        seq = h->core.expected;
    } else if (r < 75u) {
        seq = (uint8_t)(h->core.expected - 1u - rnd_n(64u));
    } else {
        seq = (uint8_t)rnd();
    }
    uint8_t body[OCB_MAX_BODY];
    const size_t blen = random_body(type, body);
    return lh_wire(type, seq, body, blen, wire);
}

static void feed_wire(lh *h, const uint8_t *w, size_t n, bool via_ring)
{
    if (!via_ring) {
        lh_feed(h, w, n);
        return;
    }
    for (size_t i = 0; i < n; i++) {
        (void)ocb_rxring_push(ocb_link_rx_ring(&h->link), w[i]);
    }
    (void)ocb_link_rx_drain(&h->link, h->now, 100000u);
}

static void fuzz_run(uint32_t seed)
{
    static lh h;
    fuzz_ctx z;
    memset(&z, 0, sizeof z);
    z.seed = seed;
    g_rng = seed != 0u ? seed : 1u;
    const uint16_t caps[] = {512u, 64u, 5u, 1u};
    lh_init_cfg(&h, caps[seed % 4u], (seed & 1u) != 0u);
    lh_open_gate(&h);
    cnt_snap prev;
    snap_counters(&h, &prev);
    uint32_t frames_delivered_checks = 0;

    for (z.step = 0; z.step < FUZZ_STEPS && !z.failed; z.step++) {
        const uint8_t exp0 = h.core.expected;
        const uint32_t ok0 = h.link.rx.stats.frames_ok;
        uint8_t w[OCB_MAX_WIRE_FRAME];
        size_t n;
        bool resync_check = false;
        uint8_t want_type = 0u;
        uint8_t want_seq = 0u;
        bool sync_seen = false;

        const uint32_t a = rnd_n(100u);
        if (a < 30u) {
            /* A clean frame right after a delimiter, or right after an
             * error byte (D4: it closes the run): always delivered. */
            n = random_frame(&h, w);
            if (chance(5000u)) {
                const uint8_t z0 = 0u;
                feed_wire(&h, &z0, 1u, chance(3000u));
            } else {
                ocb_link_rx_entry(&h.link, (uint16_t)((rnd_n(15u) + 1u) << 8 | (rnd() & 0xFFu)), h.now);
            }
            /* A NAK CRC for the run the prefix closed leaves before the
             * frame is handled (it may be a sync point that re-arms). */
            drain_and_check(&h, &z);
            feed_wire(&h, w, n, chance(3000u));
            ocb_frame f;
            (void)ocb_frame_decode(OCB_RX_CONTROLLER, w, n - 1u, &f);
            resync_check = true;
            want_type = f.type;
            want_seq = f.seq;
            memcpy(z.last_wire, w, n);
            z.last_len = n;
        } else if (a < 40u) {
            /* A damaged frame: flips, truncation or junk inside. */
            n = random_frame(&h, w);
            const uint32_t kind = rnd_n(3u);
            if (kind == 0u) {
                const uint32_t flips = 1u + rnd_n(3u);
                for (uint32_t i = 0; i < flips; i++) {
                    const size_t p = rnd_n((uint32_t)n);
                    w[p] = (uint8_t)(w[p] ^ (1u << rnd_n(8u)));
                }
                feed_wire(&h, w, n, false);
            } else if (kind == 1u) {
                feed_wire(&h, w, 1u + rnd_n((uint32_t)n - 1u), false);
            } else {
                const size_t p = rnd_n((uint32_t)n - 1u);
                feed_wire(&h, w, p, false);
                const uint8_t junk = (uint8_t)(1u + rnd_n(255u));
                feed_wire(&h, &junk, 1u, false);
                feed_wire(&h, &w[p], n - p, false);
            }
        } else if (a < 45u) {
            /* A later duplicate of the last clean frame. */
            if (z.last_len > 0u) {
                feed_wire(&h, z.last_wire, z.last_len, false);
            }
        } else if (a < 52u) {
            const uint32_t k = 1u + rnd_n(24u);
            for (uint32_t i = 0; i < k; i++) {
                ocb_link_rx_entry(&h.link, (uint16_t)(rnd() & 0xFFu), h.now);
            }
        } else if (a < 56u) {
            const uint32_t k = 1u + rnd_n(8u);
            for (uint32_t i = 0; i < k; i++) {
                ocb_link_rx_entry(&h.link, 0u, h.now);
            }
        } else if (a < 58u) {
            for (uint32_t i = 0; i < 300u; i++) {
                ocb_link_rx_entry(&h.link, (uint16_t)(1u + rnd_n(255u)), h.now);
            }
        } else if (a < 63u) {
            ocb_link_rx_entry(&h.link, (uint16_t)((rnd_n(15u) + 1u) << 8 | (rnd() & 0xFFu)), h.now);
        } else if (a < 64u) {
            /* Ring overflow: push past capacity, then drain. */
            for (uint32_t i = 0; i < OCB_LINK_RX_RING_CAP + 20u; i++) {
                (void)ocb_rxring_push(ocb_link_rx_ring(&h.link), (uint16_t)(1u + rnd_n(255u)));
            }
            (void)ocb_link_rx_drain(&h.link, h.now, 100000u);
        } else if (a < 72u) {
            h.now += chance(1000u) ? rnd_n(300000u) : rnd_n(3000u);
        } else if (a < 74u) {
            ocb_core_out out;
            ocb_core_abort(&h.core, (ocb_core_abort_cause)rnd_n(OCB_ABORT_CAUSE_COUNT), &out);
            ocb_link_queue_event(&h.link, out.event);
        } else if (a < 80u) {
            switch (rnd_n(6u)) {
            case 0:
                ocb_usb_mailbox_handshake_done(&h.mb);
                break;
            case 1:
            case 2:
                ocb_usb_mailbox_in_report(&h.mb);
                break;
            case 3:
                ocb_usb_mailbox_suspend(&h.mb);
                break;
            case 4:
                ocb_usb_mailbox_unmount(&h.mb);
                break;
            default:
                ocb_usb_mailbox_bus_reset(&h.mb);
                break;
            }
        } else if (a < 81u) {
            ocb_txgate_release(ocb_link_gate(&h.link));
        } else if (a < 84u) {
            (void)ocb_txgate_sample(ocb_link_gate(&h.link), chance(7000u), h.now);
        } else if (a < 86u) {
            /* Recovery: delimiter, valid HELLO, then a PING in order. */
            const uint8_t z0 = 0u;
            feed_wire(&h, &z0, 1u, false);
            drain_and_check(&h, &z);
            const uint8_t pv = (uint8_t)OCB_PROTO_VERSION;
            n = lh_wire(OCB_MSG_HELLO, (uint8_t)rnd(), &pv, 1u, w);
            feed_wire(&h, w, n, false);
            if (!ocb_link_session_open(&h.link)) {
                fail(&z, "a valid HELLO after a delimiter did not open the session");
            }
            sync_seen = true;
        } else {
            ocb_link_poll(&h.link, h.now);
        }

        /* Re-arm of the transport NAK budget seen from outside. */
        if (h.link.rx.stats.frames_ok != ok0 &&
            (h.link.frame.type == OCB_MSG_STOP || h.link.frame.type == OCB_MSG_HELLO)) {
            sync_seen = true;
        }
        if (h.core.expected != exp0 || sync_seen) {
            z.rearmed = true;
        }
        drain_and_check(&h, &z);

        if (resync_check) {
            frames_delivered_checks++;
            if (h.link.rx.stats.frames_ok == ok0 || h.link.frame.type != want_type || h.link.frame.seq != want_seq) {
                fail(&z, "a valid frame after a 0x00 or an error byte was not delivered (resync)");
            }
        }
        if (h.core.expected != exp0) {
            if (h.link.rx.stats.frames_ok == ok0) {
                fail(&z, "expected changed without a delivered frame");
            } else if (h.link.rx.stats.frames_ok - ok0 == 1u && h.core.expected != (uint8_t)(h.link.frame.seq + 1u)) {
                fail(&z, "expected is not the delivered frame's seq + 1");
            }
        }
        check_core(&h, &z);
        cnt_snap now;
        snap_counters(&h, &now);
        for (int i = 0; i < 13; i++) {
            if (now.c[i] < prev.c[i]) {
                fail(&z, "a counter decreased");
            }
        }
        prev = now;
    }
    printf("fuzz seed 0x%08x: %u steps, %u resync checks, %u TX frames, %u frames ok, %u dropped runs\n", seed,
           z.step, frames_delivered_checks, z.tx_seen, h.link.rx.stats.frames_ok,
           h.link.rx.stats.cobs_errors + h.link.rx.stats.crc_errors + h.link.rx.stats.range_errors);
}

/* ---------------------------------------------------------------- part 2 */

#define WIRE_CAP 65536u
#define BYTES_PER_TICK 46u /* 460800 baud, 10 bits per byte, 100 us tick */
#define TICK_US 100u

typedef struct {
    uint8_t b[WIRE_CAP];
    uint64_t at[WIRE_CAP];
    uint32_t head;
    uint32_t count;
} wire_q;

typedef struct {
    uint32_t loss; /* per 10 000, per frame, for each impairment */
} impair;

static void wq_push(wire_q *q, uint8_t b, uint64_t at)
{
    if (q->count >= WIRE_CAP) {
        OCB_CHECK(false, "wire buffer overflow");
        return;
    }
    const uint32_t i = (q->head + q->count) % WIRE_CAP;
    q->b[i] = b;
    q->at[i] = at;
    q->count++;
}

/* Sends one frame through an impaired FIFO wire. */
static void wire_send(wire_q *q, const uint8_t *w, size_t n, uint64_t now, const impair *im, uint64_t *last_at)
{
    if (chance(im->loss)) {
        return; /* dropped */
    }
    uint64_t at = now + 50u + (chance(im->loss) ? rnd_n(5000u) : 0u);
    if (at < *last_at) {
        at = *last_at; /* FIFO: latency never reorders */
    }
    *last_at = at;
    uint8_t buf[OCB_MAX_WIRE_FRAME];
    memcpy(buf, w, n);
    if (chance(im->loss)) {
        const size_t p = rnd_n((uint32_t)n);
        buf[p] = (uint8_t)(buf[p] ^ (1u << rnd_n(8u)));
    }
    if (chance(im->loss)) {
        const uint32_t k = 1u + rnd_n(6u);
        for (uint32_t i = 0; i < k; i++) {
            wq_push(q, (uint8_t)rnd(), at);
        }
    }
    for (size_t i = 0; i < n; i++) {
        wq_push(q, buf[i], at);
    }
    if (chance(im->loss)) {
        for (size_t i = 0; i < n; i++) {
            wq_push(q, buf[i], at);
        }
    }
}

typedef struct {
    uint32_t total;
    uint8_t hello_seq;
    bool session;
    uint64_t last_hello_us;
    uint32_t base;       /* oldest unACKed event */
    uint32_t next;       /* next event to (re)send */
    uint32_t high;       /* events ever sent: max index + 1 */
    uint32_t rewind_at;  /* last rewind point (event index), or UINT32_MAX */
    uint64_t base_timer; /* time the base was (re)sent or advanced */
    bool have_status;
    uint16_t free;
    uint32_t accepted_by_status;
    uint32_t resends;
    uint32_t rewinds;
    uint32_t naks;
    uint32_t semantic_naks;
    ocb_rx rx;
} sender;

static uint8_t seq_of(const sender *s, uint32_t idx)
{
    return (uint8_t)(s->hello_seq + 1u + idx);
}

static uint32_t ev_t(uint32_t idx)
{
    return (idx + 1u) * 7u;
}

/* Event index of `seq` relative to base - 1, if within [base - 1, base + 64]. */
static bool seq_to_idx(const sender *s, uint8_t seq, uint32_t *idx_after)
{
    /* idx_after = number of events up to and including seq. */
    const uint8_t ref = (uint8_t)(seq_of(s, s->base) - 1u); /* seq of base - 1 (HELLO for base 0) */
    const uint8_t d = (uint8_t)(seq - ref);
    if (d > 64u) {
        return false;
    }
    *idx_after = s->base + d;
    return true;
}

static void sender_ack_upto(sender *s, uint32_t count, uint64_t now)
{
    if (count > s->high) {
        OCB_CHECK(false, "ACK for an event never sent");
        return;
    }
    if (count > s->base) {
        s->base = count;
        s->base_timer = now;
        if (s->next < s->base) {
            s->next = s->base;
        }
    }
}

static void sender_rx_frame(sender *s, const ocb_frame *f, uint64_t now)
{
    uint32_t idx;
    if (f->type == OCB_MSG_INFO) {
        if (!s->session) {
            s->session = true;
            s->base_timer = now;
        }
        return;
    }
    if (!s->session) {
        return;
    }
    if (f->type == OCB_MSG_ACK) {
        ocb_msg_ack m;
        if (ocb_msg_ack_decode(f->body, f->len, &m) == OCB_OK && seq_to_idx(s, m.seq, &idx)) {
            sender_ack_upto(s, idx, now);
        }
    } else if (f->type == OCB_MSG_NAK) {
        ocb_msg_nak m;
        if (ocb_msg_nak_decode(f->body, f->len, &m) != OCB_OK) {
            return;
        }
        s->naks++;
        if (!OCB_NAK_IS_TRANSPORT(m.reason)) {
            s->semantic_naks++;
            return;
        }
        /* expected - 1 is accepted: a cumulative ACK (D7). */
        if (seq_to_idx(s, (uint8_t)(m.expected - 1u), &idx)) {
            sender_ack_upto(s, idx, now);
            if (s->rewind_at == UINT32_MAX || idx > s->rewind_at) {
                s->rewind_at = idx;
                s->next = idx;
                s->rewinds++;
                s->base_timer = now;
            }
        }
    } else if (f->type == OCB_MSG_STATUS) {
        ocb_msg_status m;
        if (ocb_msg_status_decode(f->body, f->len, &m) == OCB_OK && seq_to_idx(s, m.last_seq, &idx)) {
            s->have_status = true;
            s->free = m.free;
            s->accepted_by_status = idx;
            sender_ack_upto(s, idx, now);
        }
    }
}

static void sender_tick(sender *s, wire_q *q, uint64_t now, const impair *im, uint64_t *last_at, uint32_t budget)
{
    uint8_t w[OCB_MAX_WIRE_FRAME];
    if (!s->session) {
        if (now - s->last_hello_us >= (uint64_t)OCB_HELLO_RESEND_MS * 1000u || s->last_hello_us == 0u) {
            const uint8_t pv = (uint8_t)OCB_PROTO_VERSION;
            const size_t n = lh_wire(OCB_MSG_HELLO, s->hello_seq, &pv, 1u, w);
            wire_send(q, w, n, now, im, last_at);
            s->last_hello_us = now;
        }
        return;
    }
    if (s->base < s->high && now - s->base_timer >= (uint64_t)OCB_ACK_TIMEOUT_MS * 1000u) {
        s->next = s->base; /* go back N */
        s->base_timer = now;
        s->resends++;
        s->rewind_at = UINT32_MAX;
    }
    uint32_t sent_bytes = 0;
    while (s->next < s->total && sent_bytes < budget) {
        const uint32_t unacked = s->next - s->base;
        if (unacked >= (uint32_t)OCB_ACK_WINDOW) {
            break;
        }
        if (s->next >= s->high) {
            /* A new event needs credit. */
            if (!s->have_status) {
                break;
            }
            const uint32_t in_flight = s->high > s->accepted_by_status ? s->high - s->accepted_by_status : 0u;
            if ((uint32_t)s->free <= in_flight) {
                break;
            }
        }
        uint8_t body[OCB_MAX_BODY];
        const size_t bl = lh_body_queue_event(ev_t(s->next), lst_buttons(OCB_BTN_A), body);
        const size_t n = lh_wire(OCB_MSG_QUEUE_EVENT, seq_of(s, s->next), body, bl, w);
        wire_send(q, w, n, now, im, last_at);
        sent_bytes += (uint32_t)n;
        if (s->next == s->base) {
            s->base_timer = now;
        }
        s->next++;
        if (s->next > s->high) {
            s->high = s->next;
        }
    }
}

static bool lossy_run(uint32_t seed, uint16_t cap, uint32_t total, uint32_t loss)
{
    static lh h;
    static wire_q fwd;
    static wire_q rev;
    static sender s;
    static uint32_t got[4096];
    g_rng = seed != 0u ? seed : 1u; /* xorshift never leaves 0 */
    lh_init(&h, cap);
    memset(&fwd, 0, sizeof fwd);
    memset(&rev, 0, sizeof rev);
    memset(&s, 0, sizeof s);
    s.total = total;
    s.hello_seq = (uint8_t)rnd();
    s.rewind_at = UINT32_MAX;
    ocb_rx_init(&s.rx, OCB_RX_APP);
    const impair im = {loss};
    uint64_t fwd_last = 0;
    uint64_t rev_last = 0;
    uint32_t ngot = 0;
    bool ok = true;
    const uint64_t t_end = h.now + 600000000u; /* 600 s of fake time */
    uint16_t prev_count = 0;
    uint64_t next_pop = h.now;
    uint32_t ticks = 0;

    while (ok && !(s.session && s.base >= total && ngot >= total)) {
        if (h.now > t_end) {
            fprintf(stderr, "lossy seed 0x%08x cap %u loss %u: deadlock (base %u of %u, enqueued %u)\n", seed, cap,
                    loss, s.base, total, ngot);
            OCB_CHECK(false, "lossy channel did not finish");
            ok = false;
            break;
        }
        ticks++;
        h.now += TICK_US;
        (void)ocb_txgate_sample(ocb_link_gate(&h.link), true, h.now);
        sender_tick(&s, &fwd, h.now, &im, &fwd_last, BYTES_PER_TICK);

        /* Forward wire: at most one tick's worth of bytes that are due. */
        for (uint32_t k = 0; k < BYTES_PER_TICK && fwd.count > 0u && fwd.at[fwd.head] <= h.now; k++) {
            ocb_link_rx_entry(&h.link, fwd.b[fwd.head], h.now);
            fwd.head = (fwd.head + 1u) % WIRE_CAP;
            fwd.count--;
        }
        /* Probe: what entered the queue (no pop happens while feeding). */
        const uint16_t count = ocb_queue_count(&h.core.queue);
        if (count > h.core.queue.cap) {
            OCB_CHECK(false, "queue over capacity");
            ok = false;
        }
        if (count > prev_count) {
            const uint16_t added = (uint16_t)(count - prev_count);
            for (uint16_t i = 0; i < added; i++) {
                const uint16_t pos =
                    (uint16_t)((h.core.queue.head + prev_count + i) % h.core.queue.cap);
                if (ngot < 4096u) {
                    got[ngot] = h.core.queue.buf[pos].t_us;
                }
                ngot++;
            }
        } else if (count < prev_count) {
            OCB_CHECK(false, "queue shrank while feeding (cleared?)");
            ok = false;
        }
        ocb_link_poll(&h.link, h.now);
        /* The consumer: one event per 2 ms, as playback would. */
        if (h.now >= next_pop && ocb_queue_count(&h.core.queue) > 0u) {
            (void)ocb_queue_pop(&h.core.queue, NULL);
            next_pop = h.now + 2000u;
        }
        prev_count = ocb_queue_count(&h.core.queue);

        /* Reverse wire. */
        uint8_t w[OCB_MAX_WIRE_FRAME];
        for (int k = 0; k < 8; k++) {
            const size_t n = ocb_link_tx_next(&h.link, h.now, w, sizeof w);
            if (n == 0u) {
                break;
            }
            wire_send(&rev, w, n, h.now, &im, &rev_last);
        }
        for (uint32_t k = 0; k < BYTES_PER_TICK && rev.count > 0u && rev.at[rev.head] <= h.now; k++) {
            ocb_frame f;
            if (ocb_rx_push(&s.rx, rev.b[rev.head], &f) == OCB_RX_FRAME) {
                sender_rx_frame(&s, &f, h.now);
            }
            rev.head = (rev.head + 1u) % WIRE_CAP;
            rev.count--;
        }
        if (s.semantic_naks > 0u) {
            fprintf(stderr, "lossy seed 0x%08x cap %u loss %u: semantic NAK\n", seed, cap, loss);
            OCB_CHECK(false, "semantic NAK in the lossy run");
            ok = false;
        }
    }
    if (ok) {
        OCB_CHECK(ngot == total, "lossy seed 0x%08x cap %u loss %u: %u enqueued, wanted %u", seed, cap, loss, ngot, total);
        for (uint32_t i = 0; i < ngot && i < 4096u && i < total; i++) {
            if (got[i] != ev_t(i)) {
                fprintf(stderr, "lossy seed 0x%08x cap %u loss %u: enqueue %u is t=%u, wanted %u\n", seed, cap, loss,
                        i, got[i], ev_t(i));
                OCB_CHECK(false, "enqueued out of order or twice");
                ok = false;
                break;
            }
        }
    }
    printf("lossy seed 0x%08x cap %3u loss %4.1f%%: %u events in %.2f s fake, %u resends, %u rewinds, %u NAKs, "
           "crc %u, dropped bytes %u\n",
           seed, cap, (double)loss / 100.0, ngot, (double)ticks * TICK_US / 1e6, s.resends, s.rewinds, s.naks,
           h.link.rx.stats.crc_errors + h.link.rx.stats.cobs_errors, ocb_link_dropped_bytes(&h.link));
    return ok;
}

int main(void)
{
    uint32_t seeds[3] = {0x12345678u, 0xC0FFEE01u, 0x0BADF00Du};
    const char *env = getenv("OCB_FUZZ_SEED");
    if (env != NULL && env[0] != '\0') {
        const uint32_t s = (uint32_t)strtoul(env, NULL, 0);
        seeds[0] = s;
        seeds[1] = s ^ 0x5A5A5A5Au;
        seeds[2] = s ^ 0xA5A5A5A5u;
        printf("OCB_FUZZ_SEED=%s\n", env);
    }
    for (int i = 0; i < 3; i++) {
        fuzz_run(seeds[i]);
    }
    const uint32_t losses[] = {0u, 100u, 500u, 1000u};
    for (size_t i = 0; i < sizeof losses / sizeof losses[0]; i++) {
        (void)lossy_run(seeds[i % 3u] ^ (uint32_t)i, 512u, 3000u, losses[i]);
        (void)lossy_run(seeds[i % 3u] ^ (uint32_t)(i + 77u), 5u, 600u, losses[i]);
    }
    const int f = ocb_test_failures();
    if (f != 0) {
        fprintf(stderr, "test_link_fuzz (F.17): %d failure(s); seeds 0x%08x 0x%08x 0x%08x (OCB_FUZZ_SEED to replay)\n",
                f, seeds[0], seeds[1], seeds[2]);
        return 1;
    }
    printf("test_link_fuzz (F.17): ok\n");
    return 0;
}
