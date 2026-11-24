/*
 * SPDX-License-Identifier: MIT
 *
 * Host test F.7 (Story 3.1, AD-4): heartbeat gaps during a 512-frame burst.
 * Maximum-size frames from the PC as fast as the bridge takes them; a model
 * of the platform: the UART software ring (128) and the 32-entry hardware
 * FIFO, the wire draining one 10-bit character every 1e10/460800 ns; the
 * heartbeat alarm ticking every 20 ms. Every gap between the starts of two
 * consecutive heartbeats on the wire must be <= 27 ms, none skipped, and
 * the app frames must come out whole and in order. Also: several missed
 * ticks give one heartbeat; none while probing; body and seq.
 */
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "ocb_bridge/bridge.h"
#include "ocb_proto/ocb_proto.h"
#include "ocb_test.h"

static const uint8_t kVer[3] = {1, 2, 3};

#define RING_CAP 128u
#define FIFO_CAP 32u
#define STEP_NS 5000u
/* 10 bits at OCB_LINK_BAUD, in ns (rounded up: never faster than the line). */
#define CHAR_NS ((10ull * 1000000000ull + OCB_LINK_BAUD - 1ull) / OCB_LINK_BAUD)
#define TICK_NS 20000000ull
#define BURST 512u

typedef struct {
    uint8_t ring[RING_CAP];
    size_t ring_len;
    uint8_t fifo[FIFO_CAP];
    uint64_t fifo_at[FIFO_CAP]; /* when each byte entered the FIFO */
    size_t fifo_len;
    uint64_t wire_free_ns;      /* when the shift register is free */
} uart_model;

static void pop_front(uint8_t *buf, size_t *len, size_t k)
{
    memmove(buf, &buf[k], *len - k);
    *len -= k;
}

/* One burst with the alarm's first tick at `phase_ns`; returns the largest
 * gap between heartbeat starts. */
static uint64_t burst_gaps(uint64_t phase_ns, uint64_t *min_out, uint32_t *hbs_out)
{
    static uint8_t stream[BURST * OCB_MAX_WIRE_FRAME];
    size_t stream_len = 0;
    uint8_t body[OCB_MAX_BODY];
    for (size_t i = 0; i < sizeof body; i++) {
        body[i] = (uint8_t)(1u + (i % 254u));
    }
    for (uint32_t i = 0; i < BURST; i++) {
        stream_len += ocb_frame_encode(OCB_MSG_QUEUE_EVENT, (uint8_t)i, body, sizeof body,
                                       &stream[stream_len], OCB_MAX_WIRE_FRAME);
    }
    OCB_CHECK(stream_len == BURST * (size_t)OCB_MAX_WIRE_FRAME, "512 maximum frames (%zu bytes)",
              stream_len);

    ocb_bridge b;
    ocb_bridge_init(&b, kVer);
    ocb_bridge_set_dtr(&b, true);
    uart_model u;
    memset(&u, 0, sizeof u);
    ocb_rx rx;
    ocb_rx_init(&rx, OCB_RX_CONTROLLER);

    size_t fed = 0;
    uint64_t t = 0;
    bool at_frame_start = true;
    uint64_t frame_start_ns = 0;
    uint64_t last_hb_ns = 0;
    uint64_t max_gap_ns = 0;
    uint64_t min_gap_ns = UINT64_MAX;
    uint32_t hbs = 0;
    uint32_t apps = 0;
    uint8_t hb_seq_expected = 0;
    bool order_ok = true;
    uint64_t end_ns = 0;

    /* Link up first (RX high for 1 ms). */
    ocb_bridge_step(&b, 0, true);
    ocb_bridge_step(&b, 1000, true);

    while (apps < BURST && t < 10000000000ull) {
        ocb_bridge_hb_ticks(&b, (uint32_t)((t + TICK_NS - phase_ns) / TICK_NS));
        ocb_bridge_step(&b, t / 1000u + 1000u, true);
        if (fed < stream_len) {
            size_t chunk = stream_len - fed;
            if (chunk > 64u) {
                chunk = 64u; /* one USB packet per pass at most */
            }
            fed += ocb_bridge_cdc_in(&b, &stream[fed], chunk);
        }
        u.ring_len += ocb_bridge_uart_out(&b, &u.ring[u.ring_len], RING_CAP - u.ring_len, u.ring_len);
        /* pump: ring -> FIFO */
        size_t k = FIFO_CAP - u.fifo_len;
        if (k > u.ring_len) {
            k = u.ring_len;
        }
        memcpy(&u.fifo[u.fifo_len], u.ring, k);
        for (size_t j = 0; j < k; j++) {
            u.fifo_at[u.fifo_len + j] = t;
        }
        u.fifo_len += k;
        pop_front(u.ring, &u.ring_len, k);
        /* wire: exact character times; a character starts when the shift
         * register is free and the byte is in the FIFO. */
        while (u.fifo_len > 0u) {
            const uint64_t start = u.wire_free_ns > u.fifo_at[0] ? u.wire_free_ns : u.fifo_at[0];
            if (start > t) {
                break;
            }
            const uint8_t byte = u.fifo[0];
            memmove(u.fifo_at, &u.fifo_at[1], (u.fifo_len - 1u) * sizeof u.fifo_at[0]);
            pop_front(u.fifo, &u.fifo_len, 1);
            u.wire_free_ns = start + CHAR_NS;
            if (at_frame_start) {
                frame_start_ns = start;
                at_frame_start = false;
            }
            if (byte == 0u) {
                at_frame_start = true;
            }
            ocb_frame f;
            if (ocb_rx_push(&rx, byte, &f) == OCB_RX_FRAME) {
                if (f.type == OCB_MSG_HEARTBEAT) {
                    if (hbs > 0u) {
                        const uint64_t gap = frame_start_ns - last_hb_ns;
                        if (gap > max_gap_ns) {
                            max_gap_ns = gap;
                        }
                        if (gap < min_gap_ns) {
                            min_gap_ns = gap;
                        }
                    }
                    last_hb_ns = frame_start_ns;
                    OCB_CHECK(f.seq == hb_seq_expected, "heartbeat seq %u, expected %u", f.seq,
                              hb_seq_expected);
                    hb_seq_expected++;
                    OCB_CHECK(f.len == 4u && memcmp(f.body, kVer, 3) == 0 &&
                                  f.body[3] == OCB_HB_FLAG_DTR,
                              "heartbeat body");
                    hbs++;
                } else {
                    if (f.type != OCB_MSG_QUEUE_EVENT || f.seq != (uint8_t)apps ||
                        f.len != sizeof body || memcmp(f.body, body, sizeof body) != 0) {
                        order_ok = false;
                    }
                    apps++;
                    end_ns = u.wire_free_ns;
                }
            }
        }
        t += STEP_NS;
    }
    OCB_CHECK(apps == BURST, "all %u app frames out, got %u", BURST, apps);
    OCB_CHECK(order_ok, "app frames whole and in order");
    OCB_CHECK(rx.stats.crc_errors == 0u && rx.stats.cobs_errors == 0u, "no damaged frame on the wire");
    const uint64_t expect_hbs = end_ns / TICK_NS;
    OCB_CHECK(hbs + 1u >= expect_hbs, "no heartbeat skipped: %u over %.1f ms", hbs,
              (double)end_ns / 1e6);
    OCB_CHECK(max_gap_ns <= (uint64_t)OCB_HEARTBEAT_MAX_GAP_MS * 1000000ull,
              "max heartbeat gap %.3f ms > %u ms", (double)max_gap_ns / 1e6,
              (unsigned)OCB_HEARTBEAT_MAX_GAP_MS);
    *min_out = min_gap_ns;
    *hbs_out = hbs;
    return max_gap_ns;
}

static void burst_gaps_all_phases(void)
{
    uint64_t worst = 0;
    uint64_t best = UINT64_MAX;
    uint32_t hbs_total = 0;
    /* 23 tick phases across one 20 ms period (a prime step, so the phase
     * against the frame boundaries varies). */
    for (uint64_t i = 0; i < 23u; i++) {
        uint64_t mn = 0;
        uint32_t hbs = 0;
        const uint64_t mx = burst_gaps(i * (TICK_NS / 23u), &mn, &hbs);
        if (mx > worst) {
            worst = mx;
        }
        if (mn < best) {
            best = mn;
        }
        hbs_total += hbs;
    }
    printf("F.7: %u heartbeats in 23 bursts of 512 maximum frames, gaps %.3f to %.3f ms (limit %u ms)\n",
           hbs_total, (double)best / 1e6, (double)worst / 1e6, (unsigned)OCB_HEARTBEAT_MAX_GAP_MS);
    OCB_CHECK(worst > 20500000u, "the worst case was exercised (max gap %.3f ms)", (double)worst / 1e6);
}

static size_t drain(ocb_bridge *b, uint8_t *out, size_t cap)
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

static void missed_ticks_give_one_heartbeat(void)
{
    ocb_bridge b;
    ocb_bridge_init(&b, kVer);
    ocb_bridge_step(&b, 0, true);
    ocb_bridge_step(&b, 1000, true);
    ocb_bridge_hb_ticks(&b, 5); /* five ticks at once */
    uint8_t out[64];
    size_t n = drain(&b, out, sizeof out);
    OCB_CHECK(n == 11u, "one heartbeat frame (11 bytes on the wire), got %zu", n);
    ocb_bridge_hb_ticks(&b, 5); /* no change: nothing */
    OCB_CHECK(drain(&b, out, sizeof out) == 0u, "no new tick, no heartbeat");
    ocb_bridge_hb_ticks(&b, 6);
    n = drain(&b, out, sizeof out);
    ocb_rx rx;
    ocb_rx_init(&rx, OCB_RX_CONTROLLER);
    ocb_frame f;
    ocb_rx_result r = OCB_RX_NONE;
    for (size_t i = 0; i < n; i++) {
        r = ocb_rx_push(&rx, out[i], &f);
    }
    OCB_CHECK(r == OCB_RX_FRAME && f.type == OCB_MSG_HEARTBEAT && f.seq == 1u, "second heartbeat, seq 1");
    OCB_CHECK(f.body[3] == 0u, "DTR off: flags 0");
}

static void none_while_probing(void)
{
    ocb_bridge b;
    ocb_bridge_init(&b, kVer);
    ocb_bridge_step(&b, 0, false);
    ocb_bridge_hb_ticks(&b, 1);
    ocb_bridge_hb_ticks(&b, 2);
    uint8_t out[64];
    OCB_CHECK(ocb_bridge_uart_out(&b, out, sizeof out, 0) == 0u, "nothing while probing");
    OCB_CHECK(b.stats.hb_dropped == 2u, "two ticks dropped");
    /* Link-up: the dropped ticks do not come back as a stale heartbeat. */
    ocb_bridge_step(&b, 10, true);
    ocb_bridge_step(&b, 1010, true);
    OCB_CHECK(drain(&b, out, sizeof out) == 0u, "no stale heartbeat at link-up");
    ocb_bridge_hb_ticks(&b, 3);
    OCB_CHECK(drain(&b, out, sizeof out) == 11u, "the next tick sends one");
}

static void heartbeat_goes_before_the_queued_frame(void)
{
    ocb_bridge b;
    ocb_bridge_init(&b, kVer);
    ocb_bridge_step(&b, 0, true);
    ocb_bridge_step(&b, 1000, true);
    uint8_t f1[8];
    uint8_t f2[8];
    const size_t n1 = ocb_frame_encode(OCB_MSG_PING, 1, NULL, 0, f1, sizeof f1);
    const size_t n2 = ocb_frame_encode(OCB_MSG_PING, 2, NULL, 0, f2, sizeof f2);
    ocb_bridge_cdc_in(&b, f1, n1);
    uint8_t out[64];
    size_t got = ocb_bridge_uart_out(&b, out, 2, 0); /* f1 in flight, 2 bytes out */
    ocb_bridge_cdc_in(&b, f2, n2);                   /* f2 queued */
    ocb_bridge_hb_ticks(&b, 1);                      /* heartbeat due */
    got += drain(&b, &out[got], sizeof out - got);
    /* f1 completes first (never split), then the heartbeat, then f2. */
    OCB_CHECK(got == n1 + 11u + n2, "f1 + heartbeat + f2, got %zu", got);
    OCB_CHECK_MEM(out, n1, f1, n1, "f1 whole");
    OCB_CHECK(out[n1 + 1u] == OCB_MSG_HEARTBEAT,
              "heartbeat next");
    OCB_CHECK_MEM(&out[n1 + 11u], n2, f2, n2, "f2 last");
}

int main(void)
{
    burst_gaps_all_phases();
    missed_ticks_give_one_heartbeat();
    none_while_probing();
    heartbeat_goes_before_the_queued_frame();
    if (ocb_test_failures() != 0) {
        fprintf(stderr, "test_bridge_heartbeat: %d failure(s)\n", ocb_test_failures());
        return 1;
    }
    printf("test_bridge_heartbeat: ok\n");
    return 0;
}
