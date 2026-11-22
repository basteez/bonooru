/*
 * SPDX-License-Identifier: MIT
 *
 * Host test F.14 (story 2.6 Task 3.5): the controller link's RX path.
 * Byte-at-a-time, chunked (through the ring) and idle-separated delivery;
 * resync after garbage; every corrupted byte and every single-bit flip of
 * a small frame drops that frame and never the next one; error flags (FE,
 * PE, BE, OE alone and combined) close the run and are never data; breaks
 * are counted; overlong runs; ring wrap, full, overflow marker and
 * counters. Expectations are written by hand.
 *
 * Delivery is observed through HEARTBEAT frames (no session needed: the
 * link stores each valid one and counts it) and through PING ACKs.
 */
#include <stdint.h>
#include <stdio.h>

#include "link_fixture.h"

static uint32_t g_rng = 1u;
static uint32_t rnd(void)
{
    g_rng = g_rng * 1664525u + 1013904223u;
    return g_rng >> 8;
}

static size_t hb_wire(uint8_t v, uint8_t *wire)
{
    ocb_msg_heartbeat m = {{v, (uint8_t)(v + 1u), (uint8_t)(v + 2u)}, 0u};
    uint8_t body[8];
    const int n = ocb_msg_heartbeat_encode(&m, body, sizeof body);
    return lh_wire(OCB_MSG_HEARTBEAT, v, body, (size_t)n, wire);
}

static uint32_t hb_count(const lh *h)
{
    return ocb_link_counters_get(&h->link)->heartbeats;
}

static void expect_bridge_ver(const lh *h, uint8_t v, const char *what)
{
    OCB_CHECK(h->link.bridge_ver[0] == v && h->link.bridge_ver[1] == (uint8_t)(v + 1u) &&
                  h->link.bridge_ver[2] == (uint8_t)(v + 2u),
              "%s: bridge_ver %u.%u.%u, wanted %u.x.x", what, h->link.bridge_ver[0], h->link.bridge_ver[1],
              h->link.bridge_ver[2], v);
}

static void feed_entry(lh *h, uint16_t e)
{
    ocb_link_rx_entry(&h->link, e, h->now);
}

static void test_delivery(void)
{
    static lh h;
    uint8_t wire[OCB_MAX_WIRE_FRAME];

    /* Byte at a time, back to back (each frame ends with its delimiter). */
    lh_init(&h, 8u);
    for (uint8_t v = 1u; v <= 50u; v++) {
        const size_t n = hb_wire(v, wire);
        lh_feed(&h, wire, n);
        OCB_CHECK(hb_count(&h) == v, "back to back: %u delivered after frame %u", hb_count(&h), v);
        expect_bridge_ver(&h, v, "back to back");
    }

    /* With idle 0x00 bytes in between (empty runs, no reply). */
    lh_init(&h, 8u);
    const uint8_t idle[5] = {0u, 0u, 0u, 0u, 0u};
    for (uint8_t v = 1u; v <= 50u; v++) {
        lh_feed(&h, idle, (size_t)(v % 5u) + 1u);
        const size_t n = hb_wire(v, wire);
        lh_feed(&h, wire, n);
    }
    OCB_CHECK(hb_count(&h) == 50u, "idle-separated: %u of 50", hb_count(&h));
    expect_bridge_ver(&h, 50u, "idle-separated");

    /* Random chunk sizes through the ring and the drain. */
    lh_init(&h, 8u);
    uint8_t stream[50u * 16u];
    size_t len = 0;
    for (uint8_t v = 1u; v <= 50u; v++) {
        len += hb_wire(v, &stream[len]);
    }
    size_t pos = 0;
    while (pos < len) {
        size_t chunk = (size_t)(rnd() % 40u) + 1u;
        if (chunk > len - pos) {
            chunk = len - pos;
        }
        for (size_t i = 0; i < chunk; i++) {
            OCB_CHECK(ocb_rxring_push(ocb_link_rx_ring(&h.link), stream[pos + i]), "ring push refused");
        }
        pos += chunk;
        const uint32_t drained = ocb_link_rx_drain(&h.link, h.now, (uint32_t)(rnd() % 64u) + 1u);
        (void)drained;
    }
    while (ocb_link_rx_drain(&h.link, h.now, 7u) > 0u) {
    }
    OCB_CHECK(hb_count(&h) == 50u, "chunked: %u of 50", hb_count(&h));
    expect_bridge_ver(&h, 50u, "chunked");
    OCB_CHECK(ocb_link_counters_get(&h.link)->rx_bytes == (uint32_t)len, "chunked: rx_bytes %u, wanted %zu",
              ocb_link_counters_get(&h.link)->rx_bytes, len);

    /* App frames through the session: PINGs are ACKed in order. */
    lh_session(&h, 8u);
    for (int i = 0; i < 10; i++) {
        const uint8_t seq = h.seq;
        lh_ping(&h);
        lh_expect_ack(&h, seq, "PING byte by byte");
    }
}

static void test_garbage_resync(void)
{
    static lh h;
    uint8_t wire[OCB_MAX_WIRE_FRAME];
    lh_init(&h, 8u);
    const uint8_t garbage[] = {0x11u, 0x22u, 0xFFu, 0x01u, 0x02u, 0x40u, 0x07u, 0x99u};

    /* garbage, delimiter, frame */
    lh_feed(&h, garbage, sizeof garbage);
    uint8_t z = 0u;
    lh_feed(&h, &z, 1u);
    size_t n = hb_wire(10u, wire);
    lh_feed(&h, wire, n);
    OCB_CHECK(hb_count(&h) == 1u, "garbage before: %u", hb_count(&h));

    /* frame, garbage (ended by a delimiter), frame */
    lh_feed(&h, garbage, sizeof garbage);
    lh_feed(&h, &z, 1u);
    n = hb_wire(11u, wire);
    lh_feed(&h, wire, n);
    OCB_CHECK(hb_count(&h) == 2u, "garbage between: %u", hb_count(&h));
    expect_bridge_ver(&h, 11u, "garbage between");

    /* garbage after the last frame, then a delimiter: dropped, no frame. */
    lh_feed(&h, garbage, sizeof garbage);
    lh_feed(&h, &z, 1u);
    OCB_CHECK(hb_count(&h) == 2u, "garbage after: %u", hb_count(&h));
    const ocb_rx_stats *s = &h.link.rx.stats;
    /* Three garbage runs: COBS or CRC errors, never a frame. */
    OCB_CHECK(s->cobs_errors + s->crc_errors == 3u, "garbage runs counted %u", s->cobs_errors + s->crc_errors);
    OCB_CHECK(s->frames_ok == 2u, "frames_ok %u", s->frames_ok);
}

/* Every byte position replaced, and every single-bit flip, of a small
 * frame: that frame is never delivered, the next one always is. */
static void test_corruption(void)
{
    static lh h;
    uint8_t good[OCB_MAX_WIRE_FRAME];
    uint8_t next[OCB_MAX_WIRE_FRAME];
    const size_t ng = hb_wire(20u, good);
    const size_t nn = hb_wire(30u, next);
    uint32_t cases = 0;
    for (size_t pos = 0; pos + 1u < ng; pos++) {
        for (unsigned bit = 0; bit < 9u; bit++) {
            uint8_t bad[OCB_MAX_WIRE_FRAME];
            memcpy(bad, good, ng);
            if (bit < 8u) {
                bad[pos] = (uint8_t)(bad[pos] ^ (1u << bit));
            } else {
                bad[pos] = (uint8_t)(bad[pos] + 0x37u); /* a replaced byte */
            }
            lh_init(&h, 8u);
            lh_feed(&h, bad, ng);
            OCB_CHECK(hb_count(&h) == 0u, "pos %zu bit %u: corrupted frame delivered", pos, bit);
            lh_feed(&h, next, nn);
            OCB_CHECK(hb_count(&h) == 1u, "pos %zu bit %u: next frame lost (%u)", pos, bit, hb_count(&h));
            expect_bridge_ver(&h, 30u, "after corruption");
            cases++;
        }
    }
    OCB_CHECK(cases == (uint32_t)(ng - 1u) * 9u, "corruption cases %u", cases);
}

/* Error flags close the run; the following frame is delivered even with
 * no delimiter in between; the bad byte is never data. */
static void test_error_flags(void)
{
    static lh h;
    uint8_t a[OCB_MAX_WIRE_FRAME];
    uint8_t b[OCB_MAX_WIRE_FRAME];
    const size_t na = hb_wire(40u, a);
    const size_t nb = hb_wire(50u, b);
    uint32_t combos = 0;
    for (uint16_t flags = 1u; flags <= 0x0Fu; flags++) {
        const uint16_t err = (uint16_t)(flags << 8);
        const bool brk = (err & OCB_LINK_RX_BE) != 0u;

        /* (1) Mid-frame: half of A, the error entry, then all of B. */
        lh_init(&h, 8u);
        lh_feed(&h, a, na / 2u);
        feed_entry(&h, (uint16_t)(err | 0x00u));
        lh_feed(&h, b, nb);
        OCB_CHECK(hb_count(&h) == 1u, "flags 0x%x mid-frame: %u delivered", flags, hb_count(&h));
        expect_bridge_ver(&h, 50u, "error mid-frame");
        const ocb_link_counters *c = ocb_link_counters_get(&h.link);
        OCB_CHECK(c->rx_err_bytes == 1u && c->rx_err_runs == 1u, "flags 0x%x: err bytes %u runs %u", flags,
                  c->rx_err_bytes, c->rx_err_runs);
        OCB_CHECK(c->breaks == (brk ? 1u : 0u), "flags 0x%x: breaks %u", flags, c->breaks);
        OCB_CHECK(c->rx_fe == ((flags & 1u) ? 1u : 0u) && c->rx_pe == ((flags & 2u) ? 1u : 0u) &&
                      c->rx_oe == ((flags & 8u) ? 1u : 0u),
                  "flags 0x%x: FE %u PE %u OE %u", flags, c->rx_fe, c->rx_pe, c->rx_oe);
        OCB_CHECK(c->rx_bytes == (uint32_t)(na / 2u + nb), "flags 0x%x: rx_bytes %u", flags, c->rx_bytes);

        /* (2) Before a frame, with a non-zero data byte: if it were data it
         * would prefix B's run and B would be lost. */
        lh_init(&h, 8u);
        feed_entry(&h, (uint16_t)(err | 0x5Au));
        lh_feed(&h, b, nb);
        OCB_CHECK(hb_count(&h) == 1u, "flags 0x%x before B: %u delivered", flags, hb_count(&h));
        OCB_CHECK(ocb_link_counters_get(&h.link)->rx_err_runs == 0u, "nothing in progress, no run abandoned");

        /* (3) Between the two halves of an otherwise valid frame: the frame
         * is dropped (the byte is not skipped). */
        lh_init(&h, 8u);
        lh_feed(&h, a, na / 2u);
        feed_entry(&h, (uint16_t)(err | a[na / 2u]));
        lh_feed(&h, &a[na / 2u], na - na / 2u);
        OCB_CHECK(hb_count(&h) == 0u, "flags 0x%x inside A: delivered", flags);
        combos++;
    }
    OCB_CHECK(combos == 15u, "flag combinations %u", combos);

    /* A break while idle: the 0x00 it loads is not a delimiter and not
     * data; counted once, nothing else. */
    lh_init(&h, 8u);
    feed_entry(&h, (uint16_t)(OCB_LINK_RX_BE | 0x00u));
    feed_entry(&h, (uint16_t)(OCB_LINK_RX_BE | OCB_LINK_RX_FE | 0x00u));
    OCB_CHECK(ocb_link_breaks(&h.link) == 2u, "breaks %u", ocb_link_breaks(&h.link));
    OCB_CHECK(h.link.rx.stats.empty == 0u, "a break byte was taken as a delimiter");
    OCB_CHECK(ocb_link_counters_get(&h.link)->rx_bytes == 0u, "a break byte was taken as data");
    lh_feed(&h, b, nb);
    OCB_CHECK(hb_count(&h) == 1u, "after breaks: %u", hb_count(&h));
}

static void test_overlong(void)
{
    static lh h;
    uint8_t b[OCB_MAX_WIRE_FRAME];
    const size_t nb = hb_wire(60u, b);
    const uint32_t lengths[] = {263u, 264u, 300u, 524u, 1000u, 5000u};
    for (size_t i = 0; i < sizeof lengths / sizeof lengths[0]; i++) {
        lh_init(&h, 8u);
        for (uint32_t k = 0; k < lengths[i]; k++) {
            feed_entry(&h, (uint16_t)((k % 250u) + 1u));
        }
        uint8_t z = 0u;
        lh_feed(&h, &z, 1u);
        lh_feed(&h, b, nb);
        OCB_CHECK(hb_count(&h) == 1u, "overlong %u: next frame %u", lengths[i], hb_count(&h));
        OCB_CHECK(h.link.rx.stats.overlong == 1u, "overlong %u: counted %u", lengths[i], h.link.rx.stats.overlong);
        OCB_CHECK(ocb_link_counters_get(&h.link)->overlong_bytes == lengths[i], "overlong %u: bytes %u",
                  lengths[i], ocb_link_counters_get(&h.link)->overlong_bytes);
        OCB_CHECK(ocb_link_dropped_bytes(&h.link) == lengths[i], "overlong %u: dropped_bytes %u", lengths[i],
                  ocb_link_dropped_bytes(&h.link));
    }
    /* 262 non-zero bytes are not overlong (a maximal frame's size). */
    lh_init(&h, 8u);
    for (uint32_t k = 0; k < 262u; k++) {
        feed_entry(&h, 0x55u);
    }
    OCB_CHECK(h.link.rx.stats.overlong == 0u && ocb_link_counters_get(&h.link)->overlong_bytes == 0u,
              "262 bytes counted as overlong");
}

static void test_ring(void)
{
    static ocb_rxring r;
    ocb_rxring_init(&r);
    uint16_t e = 0u;
    OCB_CHECK(!ocb_rxring_pop(&r, &e), "empty ring popped");

    /* Fill to 512, the 513th is dropped and flagged. */
    for (uint16_t i = 0; i < 512u; i++) {
        OCB_CHECK(ocb_rxring_push(&r, (uint16_t)(i & 0x0FFFu)), "push %u refused", i);
    }
    OCB_CHECK(ocb_rxring_count(&r) == 512u, "count %u", ocb_rxring_count(&r));
    OCB_CHECK(!ocb_rxring_push(&r, 0x0AAu), "push on full accepted");
    OCB_CHECK(r.overflowed && r.dropped == 1u, "overflow flag %d dropped %u", (int)r.overflowed, r.dropped);

    /* One slot free: the marker needs two, so this push is lost too. */
    OCB_CHECK(ocb_rxring_pop(&r, &e) && e == 0u, "pop 0");
    OCB_CHECK(!ocb_rxring_push(&r, 0x0ABu), "push with one free slot and a pending gap accepted");
    OCB_CHECK(r.dropped == 2u, "dropped %u", r.dropped);

    /* Two slots free: marker, then the entry. */
    OCB_CHECK(ocb_rxring_pop(&r, &e) && e == 1u, "pop 1");
    OCB_CHECK(ocb_rxring_push(&r, 0x0ACu), "push after the gap refused");
    for (uint16_t i = 2; i < 512u; i++) {
        OCB_CHECK(ocb_rxring_pop(&r, &e) && e == i, "pop %u got %u", i, e);
    }
    OCB_CHECK(ocb_rxring_pop(&r, &e) && e == OCB_LINK_RX_OVERFLOW_MARK, "marker missing (0x%x)", e);
    OCB_CHECK(ocb_rxring_pop(&r, &e) && e == 0x0ACu, "entry after the marker 0x%x", e);
    OCB_CHECK(!ocb_rxring_pop(&r, &e), "ring not empty");
    OCB_CHECK(r.overflowed, "overflow flag is sticky");

    /* Index wrap: 70 000 entries through a ring kept nearly full. */
    ocb_rxring_init(&r);
    uint32_t pushed = 0;
    uint32_t popped = 0;
    while (popped < 70000u) {
        while (ocb_rxring_count(&r) < 500u) {
            OCB_CHECK(ocb_rxring_push(&r, (uint16_t)(pushed & 0x0FFFu)), "wrap push refused");
            pushed++;
        }
        for (int k = 0; k < 37; k++) {
            OCB_CHECK(ocb_rxring_pop(&r, &e) && e == (uint16_t)(popped & 0x0FFFu), "wrap pop %u got %u", popped, e);
            popped++;
        }
    }
    OCB_CHECK(!r.overflowed && r.dropped == 0u, "wrap overflowed");

    /* The link treats a marker as one error: the run in progress is
     * dropped, the next frame is delivered, counters by hand. */
    static lh h;
    uint8_t a[OCB_MAX_WIRE_FRAME];
    uint8_t b[OCB_MAX_WIRE_FRAME];
    const size_t na = hb_wire(70u, a);
    const size_t nb = hb_wire(80u, b);
    lh_init(&h, 8u);
    ocb_rxring *ring = ocb_link_rx_ring(&h.link);
    /* Idle zeros, then the start of A, fill the ring exactly; the next 600
     * entries are lost while A's run is still in progress. */
    const uint32_t filler = 512u - (uint32_t)(na - 3u);
    for (uint32_t i = 0; i < filler; i++) {
        OCB_CHECK(ocb_rxring_push(ring, 0x00u), "filler push refused");
    }
    for (size_t i = 0; i < na - 3u; i++) {
        OCB_CHECK(ocb_rxring_push(ring, a[i]), "A push refused");
    }
    uint32_t lost = 0;
    for (uint32_t i = 0; i < 600u; i++) {
        if (!ocb_rxring_push(ring, 0x77u)) {
            lost++;
        }
    }
    OCB_CHECK(lost == 600u, "lost %u, wanted 600", lost);
    (void)ocb_link_rx_drain(&h.link, h.now, 1000u);
    for (size_t i = 0; i < nb; i++) {
        OCB_CHECK(ocb_rxring_push(ring, b[i]), "push of B refused");
    }
    (void)ocb_link_rx_drain(&h.link, h.now, 1000u);
    const ocb_link_counters *c = ocb_link_counters_get(&h.link);
    OCB_CHECK(hb_count(&h) == 1u, "frame after overflow: %u", hb_count(&h));
    expect_bridge_ver(&h, 80u, "after overflow");
    OCB_CHECK(c->rx_overflow == 1u && c->rx_err_runs == 1u, "rx_overflow %u runs %u", c->rx_overflow,
              c->rx_err_runs);
    OCB_CHECK(h.link.ring.dropped == lost, "ring dropped %u", h.link.ring.dropped);
    OCB_CHECK(ocb_link_dropped_bytes(&h.link) == lost, "dropped_bytes %u, wanted %u", ocb_link_dropped_bytes(&h.link),
              lost);
    OCB_CHECK(c->rx_bytes == 512u + (uint32_t)nb, "rx_bytes %u", c->rx_bytes);
}

/* Hand count of every counter over one scripted stream. */
static void test_counters(void)
{
    static lh h;
    uint8_t w[OCB_MAX_WIRE_FRAME];
    lh_init(&h, 8u);
    size_t bytes = 0;
    const uint8_t z = 0u;
    lh_feed(&h, &z, 1u); /* 1 empty run */
    bytes += 1u;
    size_t n = hb_wire(1u, w);
    lh_feed(&h, w, n); /* 1 heartbeat */
    bytes += n;
    feed_entry(&h, OCB_LINK_RX_FE | 0x12u); /* 1 err byte, nothing in progress */
    feed_entry(&h, OCB_LINK_RX_BE);         /* 1 err byte, 1 break */
    lh_feed(&h, w, 3u);                     /* a run in progress */
    bytes += 3u;
    feed_entry(&h, OCB_LINK_RX_OE | 0x01u); /* 1 err byte, 1 abandoned run */
    n = hb_wire(2u, w);
    lh_feed(&h, w, n); /* 1 heartbeat */
    bytes += n;
    const ocb_link_counters *c = ocb_link_counters_get(&h.link);
    OCB_CHECK(c->rx_bytes == (uint32_t)bytes, "rx_bytes %u, wanted %zu", c->rx_bytes, bytes);
    OCB_CHECK(c->rx_err_bytes == 3u, "rx_err_bytes %u", c->rx_err_bytes);
    OCB_CHECK(c->breaks == 1u, "breaks %u", c->breaks);
    OCB_CHECK(c->rx_err_runs == 1u, "rx_err_runs %u", c->rx_err_runs);
    OCB_CHECK(c->heartbeats == 2u, "heartbeats %u", c->heartbeats);
    OCB_CHECK(c->rx_overflow == 0u && c->overlong_bytes == 0u, "overflow/overlong");
    OCB_CHECK(h.link.rx.stats.empty == 1u && h.link.rx.stats.frames_ok == 2u, "splitter stats empty %u ok %u",
              h.link.rx.stats.empty, h.link.rx.stats.frames_ok);
    OCB_CHECK(ocb_link_dropped_bytes(&h.link) == 3u, "dropped_bytes %u", ocb_link_dropped_bytes(&h.link));
    OCB_CHECK(h.link.link_ok, "link_ok after a valid heartbeat");

    /* A short heartbeat is counted and ignored. */
    uint8_t short_body[2] = {9u, 9u};
    lh_send_seq(&h, OCB_MSG_HEARTBEAT, 0u, short_body, 2u);
    OCB_CHECK(c->heartbeat_bad == 1u && c->heartbeats == 2u, "short heartbeat");
    expect_bridge_ver(&h, 2u, "short heartbeat ignored");
}

int main(void)
{
    test_delivery();
    test_garbage_resync();
    test_corruption();
    test_error_flags();
    test_overlong();
    test_ring();
    test_counters();
    const int f = ocb_test_failures();
    if (f != 0) {
        fprintf(stderr, "test_link_rx (F.14): %d failure(s)\n", f);
        return 1;
    }
    printf("test_link_rx (F.14): ok\n");
    return 0;
}
