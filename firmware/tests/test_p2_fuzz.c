/*
 * SPDX-License-Identifier: MIT
 *
 * F.28 (story 2.3 Task 5.3, D17): deterministic fuzz of the P2 profile
 * under ASan and UBSan, 3 seeds x 200 000 steps. Every OUT report lives in
 * a heap block of exactly its length, so a read past `len` is an ASan
 * error; output buffers carry guard bytes.
 */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include "ocb_p2/p2.h"
#include "ocb_test.h"
#include "p2_fixture.h"

static uint64_t g_rng;

static uint32_t rnd(void)
{
    /* xorshift64*, deterministic per seed. */
    g_rng ^= g_rng >> 12;
    g_rng ^= g_rng << 25;
    g_rng ^= g_rng >> 27;
    return (uint32_t)((g_rng * 0x2545F4914F6CDD1Dull) >> 32);
}

static uint32_t below(uint32_t n)
{
    return n == 0u ? 0u : rnd() % n;
}

static const uint8_t k_ids[6] = {0x01, 0x10, 0x80, 0x82, 0x03, 0x11};
static const uint8_t k_subs[] = {0x00, 0x01, 0x02, 0x03, 0x04, 0x06, 0x07, 0x08, 0x10, 0x11,
                                 0x12, 0x20, 0x21, 0x22, 0x30, 0x38, 0x40, 0x41, 0x48};

#define GUARD 16u

static bool counters_monotonic(const ocb_p2_counters *a, const ocb_p2_counters *b)
{
    return b->out_reports >= a->out_reports && b->replies >= a->replies && b->refusals >= a->refusals &&
           b->ignored >= a->ignored && b->malformed >= a->malformed && b->queue_full >= a->queue_full &&
           b->in_reports >= a->in_reports && b->restarts >= a->restarts && b->handshakes >= a->handshakes;
}

static void run(uint64_t seed, int steps)
{
    g_rng = seed;
    ocb_p2 p;
    fx_init(&p);
    bool stream_asked = false; /* a stream-on step since the last restart (test's own model) */
    uint64_t t = 1000;
    for (int step = 0; step < steps; step++) {
        const ocb_p2_counters before = p.cnt;
        const uint32_t op = below(100);
        if (op < 60u) {
            /* An OUT report of 0..300 bytes in a block of exactly that size. */
            const size_t len = below(301);
            uint8_t *b = (uint8_t *)malloc(len > 0u ? len : 1u);
            if (b == NULL) {
                OCB_CHECK(false, "malloc");
                return;
            }
            for (size_t i = 0; i < len; i++) {
                b[i] = (uint8_t)rnd();
            }
            if (len > 0u && below(100) < 80u) {
                b[0] = k_ids[below(6)];
            }
            if (len > 10u && b[0] == 0x01 && below(100) < 70u) {
                b[10] = k_subs[below((uint32_t)sizeof k_subs)];
                if (b[10] == 0x10 && len > 15u && below(2) == 0u) {
                    b[15] = (uint8_t)below(0x30); /* lengths around the 0x1D limit */
                }
            }
            if (len > 1u && b[0] == 0x80 && below(100) < 60u) {
                b[1] = (uint8_t)(1u + below(6));
            }
            ocb_p2_log log;
            memset(&log, 0xEE, sizeof log);
            const ocb_p2_rx r = ocb_p2_on_output(&p, len > 0u ? b : NULL, len, below(4) == 0u ? NULL : &log);
            OCB_CHECK(r <= OCB_P2_RX_QUEUE_FULL, "result %d", (int)r);
            if (len >= 2u && b[0] == 0x80 && b[1] == 0x04) {
                stream_asked = true;
            }
            if (len >= 12u && b[0] == 0x01 && b[10] == 0x03 && b[11] == 0x30) {
                stream_asked = true;
            }
            if (len >= 2u && b[0] == 0x80 && b[1] == 0x05) {
                OCB_CHECK(!ocb_p2_ready(&p), "80 05 left the stream on");
            }
            /* Refused requests are never reported as handled. */
            if (len >= 11u && b[0] == 0x01 && (b[10] == 0x11 || b[10] == 0x12 || b[10] == 0x07)) {
                OCB_CHECK(r == OCB_P2_RX_REFUSED || r == OCB_P2_RX_QUEUE_FULL, "subcommand %02x: %d", b[10], (int)r);
            }
            /* D34: the pairing request and the MCU configuration are
             * answered, never refused (a refusal stalls the console). */
            if (len >= 11u && b[0] == 0x01 && (b[10] == 0x01 || b[10] == 0x21)) {
                OCB_CHECK(r == OCB_P2_RX_HANDLED || r == OCB_P2_RX_QUEUE_FULL, "subcommand %02x: %d", b[10], (int)r);
            }
            free(b);
        } else if (op < 85u) {
            /* next_in with random capacity; guard bytes around the buffer. */
            uint8_t buf[GUARD + 64 + GUARD];
            memset(buf, 0x5A, sizeof buf);
            const size_t cap = below(4) == 0u ? below(64) : 64u;
            ocb_state s = {(uint16_t)rnd(), (uint8_t)rnd(), (uint8_t)rnd(), (uint8_t)rnd(),
                           (uint8_t)rnd(), (uint8_t)rnd(), (uint8_t)rnd()};
            const size_t n = ocb_p2_next_in(&p, &s, &buf[GUARD], cap);
            OCB_CHECK(n == 0u || n == 64u, "length %zu", n);
            if (n > 0u) {
                const uint8_t id = buf[GUARD];
                OCB_CHECK(id == 0x21 || id == 0x30 || id == 0x81, "report id %02x", id);
                if (id == 0x30) {
                    for (size_t i = 13; i < 64u; i++) {
                        if (buf[GUARD + i] != 0u) {
                            OCB_CHECK(false, "0x30 byte %zu not zero", i);
                            break;
                        }
                    }
                }
            } else {
                for (size_t i = 0; i < 64u; i++) {
                    if (buf[GUARD + i] != 0x5A) {
                        OCB_CHECK(false, "next_in wrote with result 0");
                        break;
                    }
                }
            }
            for (size_t i = 0; i < GUARD; i++) {
                OCB_CHECK(buf[i] == 0x5A && buf[GUARD + 64 + i] == 0x5A, "guard byte overwritten");
            }
            if (n > 0u) {
                ocb_p2_interval_note(&p, t);
            }
        } else if (op < 90u) {
            uint8_t buf[GUARD + 64 + GUARD];
            memset(buf, 0x5A, sizeof buf);
            const size_t cap = below(2) == 0u ? below(70) : 64u;
            ocb_state s = {(uint16_t)rnd(), (uint8_t)rnd(), (uint8_t)rnd(), (uint8_t)rnd(),
                           (uint8_t)rnd(), (uint8_t)rnd(), (uint8_t)rnd()};
            const size_t n = ocb_p2_build_report(&p, below(10) == 0u ? NULL : &s, &buf[GUARD], cap);
            OCB_CHECK(n == 0u || (n == 64u && buf[GUARD] == 0x30), "build_report %zu", n);
            for (size_t i = 0; i < GUARD; i++) {
                OCB_CHECK(buf[i] == 0x5A && buf[GUARD + 64 + i] == 0x5A, "guard byte overwritten");
            }
        } else if (op < 92u) {
            ocb_p2_restart(&p);
            stream_asked = false;
        } else if (op < 94u) {
            (void)ocb_p2_on_feature(&p, (uint8_t)rnd(), NULL);
        } else {
            t += below(3) == 0u ? (uint64_t)rnd() : (uint64_t)below(30000);
            ocb_p2_interval_note(&p, t);
            ocb_p2_tick(&p, t);
            OCB_CHECK(p.iv.ops_median <= 401u && p.iv.count <= 256u, "interval bounds");
        }
        OCB_CHECK(p.q_count <= 4u && p.q_head < 4u, "queue %u at %u", p.q_count, p.q_head);
        OCB_CHECK(counters_monotonic(&before, &p.cnt), "counters went back at step %d", step);
        OCB_CHECK(!ocb_p2_ready(&p) || stream_asked, "ready without a stream request at step %d", step);
        if (ocb_test_failures() > 20) {
            return;
        }
    }
}

int main(void)
{
    static const uint64_t seeds[3] = {0x2300000000000001ull, 0x9E3779B97F4A7C15ull, 0x0123456789ABCDEFull};
    for (int i = 0; i < 3; i++) {
        run(seeds[i], 200000);
    }
    return ocb_test_failures() == 0 ? 0 : 1;
}
