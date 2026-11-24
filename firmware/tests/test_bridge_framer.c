/*
 * SPDX-License-Identifier: MIT
 *
 * Host test F.8 (Story 3.1, AD-4): the bridge's PC -> UART path. Whole
 * frames only, byte by byte and in random chunks; one frame in flight plus
 * one queued (a third is not consumed until a slot frees); an empty run is a
 * lone 0x00; a 262-byte run is a frame, 263 is overlong (counted once, the
 * next frame delivered); DTR fall / unmount drops only the partial run; the
 * bridge never decodes (a frame with a bad CRC goes out unchanged).
 * Expectations are written by hand.
 */
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "ocb_bridge/bridge.h"
#include "ocb_bridge/framer.h"
#include "ocb_proto/ocb_proto.h"
#include "ocb_test.h"

static const uint8_t kVer[3] = {0, 3, 1};

static uint32_t g_rng = 7u;
static uint32_t rnd(void)
{
    g_rng = g_rng * 1664525u + 1013904223u;
    return g_rng >> 8;
}

/* A linked bridge (RX high long enough on a fake clock). */
/* A linked bridge (RX high long enough on a fake clock); the resync 0x00
 * every link-up starts with is taken off the wire here. */
static void link_up(ocb_bridge *b)
{
    ocb_bridge_init(b, kVer);
    ocb_bridge_step(b, 0, true);
    ocb_bridge_step(b, 1000, true);
    OCB_CHECK(ocb_bridge_probe_linked(&b->probe), "bridge linked for the test");
    uint8_t d[4];
    const size_t n = ocb_bridge_uart_out(b, d, sizeof d, 0);
    OCB_CHECK(n == 1u && d[0] == 0u, "link-up starts with a lone 00 (%zu bytes)", n);
}

/* Everything the bridge would write to the UART now (ring empty each call). */
static size_t drain_uart(ocb_bridge *b, uint8_t *out, size_t cap)
{
    size_t n = 0;
    for (;;) {
        const size_t k = ocb_bridge_uart_out(b, &out[n], cap - n, 0);
        if (k == 0u) {
            return n;
        }
        n += k;
    }
}

static size_t frame_of(uint8_t ty, uint8_t seq, const uint8_t *body, size_t n, uint8_t *wire)
{
    return ocb_frame_encode(ty, seq, body, n, wire, OCB_MAX_WIRE_FRAME);
}

static void whole_frames_byte_by_byte_and_chunked(void)
{
    uint8_t wire[4 * OCB_MAX_WIRE_FRAME];
    size_t total = 0;
    const uint8_t body[3] = {1, 0, 2};
    for (uint8_t s = 0; s < 4; s++) {
        total += frame_of(OCB_MSG_PING, s, NULL, 0, &wire[total]);
        total += frame_of(OCB_MSG_START, s, body, sizeof body, &wire[total]);
    }
    for (int mode = 0; mode < 2; mode++) {
        ocb_bridge b;
        link_up(&b);
        uint8_t out[sizeof wire];
        size_t got = 0;
        size_t pos = 0;
        while (pos < total) {
            size_t chunk = mode == 0 ? 1u : 1u + (size_t)(rnd() % 40u);
            if (chunk > total - pos) {
                chunk = total - pos;
            }
            pos += ocb_bridge_cdc_in(&b, &wire[pos], chunk);
            got += drain_uart(&b, &out[got], sizeof out - got);
        }
        got += drain_uart(&b, &out[got], sizeof out - got);
        OCB_CHECK_MEM(out, got, wire, total, mode == 0 ? "byte by byte" : "random chunks");
        OCB_CHECK(b.stats.app_frames == 8u, "8 frames, got %u", b.stats.app_frames);
    }
}

static void one_in_flight_one_queued(void)
{
    ocb_bridge b;
    link_up(&b);
    uint8_t three[3 * 8];
    size_t n = 0;
    for (uint8_t s = 0; s < 3; s++) {
        n += frame_of(OCB_MSG_PING, s, NULL, 0, &three[n]);
    }
    const size_t one = n / 3u;
    /* No UART pump yet: the framer queues frame 1, stops before frame 2's
     * delimiter. */
    size_t used = ocb_bridge_cdc_in(&b, three, n);
    OCB_CHECK(used == 2u * one - 1u, "consumed up to frame 2's last byte, got %zu", used);
    /* One write moves frame 1 in flight; frame 2 completes and queues; frame
     * 3 is still not consumed (one in flight + one queued). */
    uint8_t out[16];
    const size_t k = ocb_bridge_uart_out(&b, out, 1, 0);
    OCB_CHECK(k == 1u, "one byte of frame 1 written");
    used += ocb_bridge_cdc_in(&b, &three[used], n - used);
    OCB_CHECK(used == 3u * one - 1u, "frame 3 waits for a slot, consumed %zu", used);
    OCB_CHECK(ocb_bridge_framer_has_frame(&b.framer), "frame 2 queued");
}

static void empty_run_is_a_lone_zero(void)
{
    ocb_bridge b;
    link_up(&b);
    const uint8_t zero[1] = {0};
    OCB_CHECK(ocb_bridge_cdc_in(&b, zero, 1) == 1u, "consumed");
    uint8_t out[8];
    const size_t n = drain_uart(&b, out, sizeof out);
    OCB_CHECK(n == 1u && out[0] == 0u, "a lone 00 forwarded (%zu bytes)", n);
    OCB_CHECK(b.framer.stats.empty == 1u, "counted as empty");
}

static void run_of_262_is_a_frame_263_is_overlong(void)
{
    uint8_t run[300];
    for (size_t i = 0; i < sizeof run; i++) {
        run[i] = (uint8_t)(1u + (i % 250u));
    }
    for (size_t len = 262; len <= 263; len++) {
        ocb_bridge b;
        link_up(&b);
        uint8_t in[300 + 16];
        memcpy(in, run, len);
        in[len] = 0;
        size_t total = len + 1u;
        /* then a good frame */
        const size_t good = frame_of(OCB_MSG_PING, 9, NULL, 0, &in[total]);
        total += good;
        uint8_t out[600];
        size_t got = 0;
        size_t pos = 0;
        while (pos < total) {
            pos += ocb_bridge_cdc_in(&b, &in[pos], total - pos);
            got += drain_uart(&b, &out[got], sizeof out - got);
        }
        if (len == 262) {
            OCB_CHECK_MEM(out, got, in, total, "262-byte run forwarded whole, then the frame");
            OCB_CHECK(b.framer.stats.overlong == 0u, "not overlong");
        } else {
            OCB_CHECK_MEM(out, got, &in[len + 1u], good, "263: dropped, next frame delivered");
            OCB_CHECK(b.framer.stats.overlong == 1u, "counted once");
        }
    }
    /* A very long run (5000) is counted once. */
    ocb_bridge b;
    link_up(&b);
    uint8_t ones[100];
    memset(ones, 0x11, sizeof ones);
    for (int i = 0; i < 50; i++) {
        OCB_CHECK(ocb_bridge_cdc_in(&b, ones, sizeof ones) == sizeof ones, "consumed");
    }
    const uint8_t zero[1] = {0};
    ocb_bridge_cdc_in(&b, zero, 1);
    uint8_t out[8];
    OCB_CHECK(drain_uart(&b, out, sizeof out) == 0u, "nothing of the overlong run goes out");
    OCB_CHECK(b.framer.stats.overlong == 1u, "5000-byte run counted once");
}

static void dtr_fall_and_unmount_drop_only_the_partial(void)
{
    for (int which = 0; which < 2; which++) {
        ocb_bridge b;
        link_up(&b);
        ocb_bridge_set_dtr(&b, true);
        uint8_t f1[8];
        uint8_t f2[8];
        const size_t n1 = frame_of(OCB_MSG_PING, 1, NULL, 0, f1);
        const size_t n2 = frame_of(OCB_MSG_PING, 2, NULL, 0, f2);
        ocb_bridge_cdc_in(&b, f1, n1);            /* complete: queued */
        ocb_bridge_cdc_in(&b, f2, n2 - 2u);       /* partial */
        if (which == 0) {
            ocb_bridge_set_dtr(&b, false);
            ocb_bridge_cut_partial(&b); /* the platform, once the PC's bytes are fed */
        } else {
            ocb_bridge_cdc_unmount(&b);
        }
        /* The tail alone: a broken run of its own. */
        uint8_t out[32];
        size_t got = 0;
        size_t pos = n2 - 2u;
        while (pos < n2) {
            pos += ocb_bridge_cdc_in(&b, &f2[pos], n2 - pos);
            got += drain_uart(&b, &out[got], sizeof out - got);
        }
        got += drain_uart(&b, &out[got], sizeof out - got);
        /* f1 whole; the tail of f2 is a run of its own (1 byte + 00), never
         * joined to the dropped head. */
        OCB_CHECK(got >= n1, "f1 delivered");
        OCB_CHECK_MEM(out, n1, f1, n1, which == 0 ? "f1 after DTR fall" : "f1 after unmount");
        OCB_CHECK(got == n1 + 2u, "only the 2-byte tail run follows, got %zu", got);
        OCB_CHECK(b.framer.stats.partial_discards == 1u, "partial discarded once");
    }
    /* A DTR fall alone drops nothing: the platform cuts only once the
     * bytes sent before the fall are fed. */
    {
        ocb_bridge c;
        link_up(&c);
        ocb_bridge_set_dtr(&c, true);
        uint8_t g[8];
        const size_t ng = frame_of(OCB_MSG_PING, 3, NULL, 0, g);
        ocb_bridge_cdc_in(&c, g, ng - 1u);
        ocb_bridge_set_dtr(&c, false);
        ocb_bridge_cdc_in(&c, &g[ng - 1u], 1u);
        uint8_t o[16];
        const size_t got = drain_uart(&c, o, sizeof o);
        OCB_CHECK_MEM(o, got, g, ng, "a frame completed after the fall still goes out");
    }
    /* DTR staying high, or rising, drops nothing. */
    ocb_bridge b;
    link_up(&b);
    uint8_t f[8];
    const size_t n = frame_of(OCB_MSG_PING, 1, NULL, 0, f);
    ocb_bridge_cdc_in(&b, f, n - 1u);
    ocb_bridge_set_dtr(&b, true);
    ocb_bridge_cdc_in(&b, &f[n - 1u], 1u);
    uint8_t out[16];
    const size_t got = drain_uart(&b, out, sizeof out);
    OCB_CHECK_MEM(out, got, f, n, "DTR rising keeps the run");
}

static void never_decodes(void)
{
    ocb_bridge b;
    link_up(&b);
    uint8_t f[8];
    const size_t n = frame_of(OCB_MSG_PING, 1, NULL, 0, f);
    f[n - 2u] ^= 0x01u; /* damage the CRC */
    const uint8_t garbage[5] = {0x13, 0x37, 0xFF, 0xEE, 0x00};
    ocb_bridge_cdc_in(&b, f, n);
    uint8_t out[32];
    size_t got = drain_uart(&b, out, sizeof out);
    OCB_CHECK_MEM(out, got, f, n, "bad CRC forwarded unchanged");
    ocb_bridge_cdc_in(&b, garbage, sizeof garbage);
    got = drain_uart(&b, out, sizeof out);
    OCB_CHECK_MEM(out, got, garbage, sizeof garbage, "garbage forwarded as a run");
}

static void nothing_while_probing(void)
{
    ocb_bridge b;
    ocb_bridge_init(&b, kVer);
    ocb_bridge_step(&b, 0, false);
    uint8_t f[8];
    const size_t n = frame_of(OCB_MSG_PING, 1, NULL, 0, f);
    ocb_bridge_cdc_in(&b, f, n);
    uint8_t out[16];
    OCB_CHECK(ocb_bridge_uart_out(&b, out, sizeof out, 0) == 0u, "no data while probing");
    /* Linked later: the queued frame goes out. */
    ocb_bridge_step(&b, 10, true);
    ocb_bridge_step(&b, 1010, true);
    const size_t got = drain_uart(&b, out, sizeof out);
    OCB_CHECK(got == n + 1u && out[0] == 0u, "a lone 00 first (resync)");
    OCB_CHECK_MEM(&out[1], got - 1u, f, n, "queued frame sent once linked");
}

static void uart_ahead_limit(void)
{
    ocb_bridge b;
    link_up(&b);
    uint8_t body[200];
    memset(body, 0x55, sizeof body);
    uint8_t f[OCB_MAX_WIRE_FRAME];
    const size_t n = frame_of(OCB_MSG_QUEUE_EVENT, 1, body, sizeof body, f);
    ocb_bridge_cdc_in(&b, f, n);
    uint8_t out[OCB_MAX_WIRE_FRAME];
    OCB_CHECK(ocb_bridge_uart_out(&b, out, sizeof out, 0) == 8u, "at most 8 bytes ahead");
    OCB_CHECK(ocb_bridge_uart_out(&b, out, sizeof out, 5) == 3u, "tops up to 8");
    OCB_CHECK(ocb_bridge_uart_out(&b, out, sizeof out, 8) == 0u, "nothing when 8 wait");
}

int main(void)
{
    whole_frames_byte_by_byte_and_chunked();
    one_in_flight_one_queued();
    empty_run_is_a_lone_zero();
    run_of_262_is_a_frame_263_is_overlong();
    dtr_fall_and_unmount_drop_only_the_partial();
    never_decodes();
    nothing_while_probing();
    uart_ahead_limit();
    if (ocb_test_failures() != 0) {
        fprintf(stderr, "test_bridge_framer: %d failure(s)\n", ocb_test_failures());
        return 1;
    }
    printf("test_bridge_framer: ok\n");
    return 0;
}
