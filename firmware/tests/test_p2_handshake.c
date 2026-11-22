/*
 * SPDX-License-Identifier: MIT
 *
 * F.25 (story 2.3 Task 3.3): the reactive P2 handshake (D5, D6). Frames and
 * replies hand-written from S1 (USB-HID-Notes: 80 01 -> 81 01 00 <type>
 * <MAC reversed>, 80 02 -> 81 02, 80 03 -> 81 03, 80 04 / 80 05 no reply)
 * and S5 / S6 (the host order 01, 02, 03, 02, 04).
 */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include "ocb_p2/p2.h"
#include "ocb_test.h"
#include "p2_fixture.h"

static void expect_usb_reply(ocb_p2 *p, const uint8_t *want, size_t n, const char *what)
{
    uint8_t got[64];
    uint8_t full[64] = {0};
    memcpy(full, want, n);
    const size_t len = fx_in(p, got);
    OCB_CHECK_MEM(got, len, full, sizeof full, what);
}

static void full_sequence(ocb_p2 *p, int round)
{
    /* MAC 11 22 33 44 55 66 reversed: 66 55 44 33 22 11; type 03 = Pro. */
    static const uint8_t r01[] = {0x81, 0x01, 0x00, 0x03, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11};
    static const uint8_t r02[] = {0x81, 0x02};
    static const uint8_t r03[] = {0x81, 0x03};
    uint8_t out[64];

    OCB_CHECK(!ocb_p2_ready(p), "round %d: ready before the handshake", round);
    OCB_CHECK(fx_in(p, out) == 0u, "round %d: nothing to send before any request", round);
    OCB_CHECK(fx_usb(p, 0x01, NULL) == OCB_P2_RX_HANDLED, "80 01");
    OCB_CHECK(fx_usb(p, 0x02, NULL) == OCB_P2_RX_HANDLED, "80 02");
    OCB_CHECK(fx_usb(p, 0x03, NULL) == OCB_P2_RX_HANDLED, "80 03");
    OCB_CHECK(fx_usb(p, 0x02, NULL) == OCB_P2_RX_HANDLED, "80 02 again");
    OCB_CHECK(!ocb_p2_ready(p), "round %d: ready before 80 04", round);
    expect_usb_reply(p, r01, sizeof r01, "reply to 80 01");
    expect_usb_reply(p, r02, sizeof r02, "reply to 80 02");
    expect_usb_reply(p, r03, sizeof r03, "reply to 80 03");
    expect_usb_reply(p, r02, sizeof r02, "reply to the second 80 02");
    OCB_CHECK(fx_in(p, out) == 0u, "round %d: no report before 80 04", round);

    ocb_p2_log log;
    OCB_CHECK(fx_usb(p, 0x04, &log) == OCB_P2_RX_HANDLED, "80 04");
    OCB_CHECK(!ocb_p2_reply_pending(p), "80 04 has no reply");
    OCB_CHECK(ocb_p2_ready(p), "round %d: not ready after 80 04", round);
    OCB_CHECK(log.n == 2u && log.kind[0] == 0x01 && log.id[0] == 0x04 && log.kind[1] == 0x06,
              "log of 80 04: n %u", log.n);
    OCB_CHECK(ocb_p2_take_handshake(p), "handshake event once");
    OCB_CHECK(!ocb_p2_take_handshake(p), "handshake event only once");
    OCB_CHECK(fx_in(p, out) == 64u && out[0] == 0x30, "round %d: 0x30 stream after 80 04", round);
}

/* S12: the whole init a real Switch 1 sent to a real wired Pro Controller
 * (mzyy94's bypass_procon_log.txt), replayed in order. Each request must
 * get the reply the real controller gave (ack and id; for 01, 03 and 21
 * the data too), no request may be refused (story 2.3 D34: a refusal made
 * the Switch 2 repeat 0x01 forever), and the stream must be on at the end.
 * Output reports and expected replies are copied from the capture. */
typedef struct {
    const char *out;   /* the output report, hex, zero padded to 64 */
    uint8_t ack;       /* expected [13] of the 0x21 reply, or 0 for none */
    uint8_t usb;       /* expected [1] of a 0x81 reply, or 0 */
    const char *data;  /* expected reply bytes from [15], hex, or NULL */
} s12_step;

static size_t hex_bytes(const char *h, uint8_t *out, size_t cap)
{
    size_t n = 0;
    while (h[0] != 0 && h[1] != 0 && n < cap) {
        unsigned v = 0;
        for (int k = 0; k < 2; k++) {
            const char c = h[k];
            v = (v << 4) | (unsigned)(c >= 'a' ? c - 'a' + 10 : c >= 'A' ? c - 'A' + 10 : c - '0');
        }
        out[n++] = (uint8_t)v;
        h += 2;
    }
    return n;
}

static void test_s12_replay(void)
{
    static const s12_step steps[] = {
        {"0000", 0, 0, NULL},
        {"0000", 0, 0, NULL},
        {"8005", 0, 0, NULL},
        {"0000", 0, 0, NULL},
        {"8001", 0, 0x01, NULL},
        {"8002", 0, 0x02, NULL},
        {"01000000000000000000033000", 0x80, 0, NULL},
        {"8004", 0, 0, NULL},
        {"01080000000000000000480000", 0x80, 0, NULL},
        {"01090000000000000000020000", 0x82, 0, NULL},
        {"010a0000000000000000020000", 0x82, 0, NULL},
        {"010b0000000000000000080000", 0x80, 0, NULL},
        {"010c00000000000000001000600000100000", 0x90, 0, "0060000010"},
        {"010d000000000000000010506000000d0000", 0x90, 0, "506000000d"},
        {"010e00000000000000000104185395d6030400043c4e696e74656e646f2053776974636800000000006800ac90113f5900",
         0x81, 0, "03"},
        {"010f0001404000014040033000", 0x80, 0, NULL},
        {"01000000000000000000040000", 0x83, 0, NULL},
        {"01010000000000000000108060000018", 0x90, 0, "8060000018"},
        {"01020000000000000000109860000012", 0x90, 0, "9860000012"},
        {"01030001404000014040101080000018", 0x90, 0, "1080000018"},
        {"01040000000000000000103d60000019", 0x90, 0, "3d60000019"},
        {"01050000000000000000102880000018", 0x90, 0, "2880000018"},
        {"01060000000000000000400100", 0x80, 0, NULL},
        {"10070001404000014040", 0, 0, NULL},
        {"010d0000000000000000480100", 0x80, 0, NULL},
        {"010ec218037200014040212100", 0xA0, 0,
         "0100ff0003000501000000000000000000000000000000000000000000000000005c"},
        {"100f0001404000000000", 0, 0, NULL},
        {"01000000000000000000300100", 0x80, 0, NULL},
    };
    ocb_p2 p;
    fx_init(&p);
    int handshakes = 0;
    for (size_t i = 0; i < sizeof steps / sizeof steps[0]; i++) {
        uint8_t b[64] = {0};
        const size_t len = hex_bytes(steps[i].out, b, sizeof b);
        const uint8_t sc = b[0] == 0x01 ? b[10] : 0u;
        const ocb_p2_rx r = ocb_p2_on_output(&p, b, len < 2u ? len : sizeof b, NULL);
        OCB_CHECK(r != OCB_P2_RX_REFUSED && r != OCB_P2_RX_QUEUE_FULL, "S12 step %zu (%s): result %d", i,
                  steps[i].out, (int)r);
        handshakes += ocb_p2_take_handshake(&p) ? 1 : 0;
        uint8_t got[64];
        int replies = 0;
        while (ocb_p2_reply_pending(&p)) {
            (void)fx_in(&p, got);
            replies++;
            if (steps[i].usb != 0u) {
                OCB_CHECK(got[0] == 0x81 && got[1] == steps[i].usb, "S12 step %zu: 81 %02x", i, steps[i].usb);
            } else {
                OCB_CHECK(got[0] == 0x21 && got[13] == steps[i].ack && got[14] == sc,
                          "S12 step %zu: reply %02x %02x %02x, want 21 .. %02x %02x", i, got[0], got[13], got[14],
                          steps[i].ack, sc);
                if (steps[i].data != NULL) {
                    uint8_t want[64];
                    const size_t n = hex_bytes(steps[i].data, want, sizeof want);
                    OCB_CHECK_MEM(&got[15], n, want, n, "S12 reply data");
                }
            }
        }
        const int want_replies = (steps[i].ack != 0u || steps[i].usb != 0u) ? 1 : 0;
        OCB_CHECK(replies == want_replies, "S12 step %zu: %d replies, want %d", i, replies, want_replies);
    }
    OCB_CHECK(ocb_p2_ready(&p) && handshakes == 1, "S12 init: stream on (%d), one handshake (%d)", ocb_p2_ready(&p),
              handshakes);
    OCB_CHECK(p.cnt.refusals == 0u, "S12 init: %u refusals", p.cnt.refusals);
}

int main(void)
{
    ocb_p2 p;
    uint8_t out[64];

    /* 1. The full host sequence of S5 and S6, then restart and again, 100 times. */
    fx_init(&p);
    for (int round = 0; round < 100; round++) {
        full_sequence(&p, round);
        /* A second 80 04 changes nothing and posts no second handshake. */
        OCB_CHECK(fx_usb(&p, 0x04, NULL) == OCB_P2_RX_HANDLED && !ocb_p2_take_handshake(&p), "repeat 80 04");
        ocb_p2_restart(&p);
        OCB_CHECK(!ocb_p2_ready(&p) && fx_in(&p, out) == 0u, "restart clears ready");
    }

    /* 2. A host that skips steps: 80 02 then 80 04. */
    fx_init(&p);
    (void)fx_usb(&p, 0x02, NULL);
    (void)fx_usb(&p, 0x04, NULL);
    OCB_CHECK(ocb_p2_ready(&p), "80 02, 80 04 reaches ready");

    /* 3. Only subcommands: 03 with 0x30 (D5). Other modes do not start it. */
    fx_init(&p);
    {
        const uint8_t m3f[1] = {0x3F};
        const uint8_t m30[1] = {0x30};
        (void)fx_sub(&p, 0x03, m3f, 1, NULL);
        OCB_CHECK(!ocb_p2_ready(&p), "03 3F does not start the stream");
        ocb_p2_log log;
        (void)fx_sub(&p, 0x03, m30, 1, &log);
        OCB_CHECK(ocb_p2_ready(&p), "03 30 starts the stream");
        OCB_CHECK(log.n == 3u && log.kind[0] == 0x02 && log.id[0] == 0x03 && log.kind[1] == 0x08 &&
                      log.id[1] == 0x30 && log.kind[2] == 0x06,
                  "log of 03 30 (subcommand, argument, handshake)");
        OCB_CHECK(ocb_p2_take_handshake(&p), "03 30 posts the handshake");
    }

    /* 4. 80 05 stops the stream and clears ready; 80 04 restores it without a
     * second handshake event (one per restart). */
    (void)fx_usb(&p, 0x05, NULL);
    while (fx_in(&p, out) > 0u && out[0] != 0x30) {
    }
    OCB_CHECK(!ocb_p2_ready(&p) && fx_in(&p, out) == 0u, "80 05 stops the stream");
    (void)fx_usb(&p, 0x04, NULL);
    OCB_CHECK(ocb_p2_ready(&p) && !ocb_p2_take_handshake(&p), "80 04 after 80 05");

    /* 5. Every ordering of the five commands 01..05: never crashes; ready
     * iff 04 came after the last 05; replies only for 01, 02, 03. */
    {
        uint8_t perm[5] = {1, 2, 3, 4, 5};
        int count = 0;
        /* Heap's algorithm, iterative. */
        int c[5] = {0};
        int i = 0;
        for (;;) {
            fx_init(&p);
            bool want = false;
            int replies = 0;
            for (int k = 0; k < 5; k++) {
                (void)fx_usb(&p, perm[k], NULL);
                if (perm[k] == 4) {
                    want = true;
                } else if (perm[k] == 5) {
                    want = false;
                } else {
                    replies++;
                }
                OCB_CHECK(ocb_p2_ready(&p) == want, "ordering %d step %d", count, k);
            }
            int got = 0;
            while (fx_in(&p, out) > 0u && out[0] == 0x81) {
                got++;
            }
            OCB_CHECK(got == replies, "ordering %d: %d replies, expected %d", count, got, replies);
            count++;
            while (i < 5 && c[i] >= i) {
                c[i] = 0;
                i++;
            }
            if (i >= 5) {
                break;
            }
            const int j = (i % 2 == 0) ? 0 : c[i];
            const uint8_t t = perm[j];
            perm[j] = perm[i];
            perm[i] = t;
            c[i]++;
            i = 0;
        }
        OCB_CHECK(count == 120, "%d orderings", count);
    }

    /* 6. Restart after any step returns to the start (queue emptied). */
    for (int step = 0; step < 5; step++) {
        fx_init(&p);
        static const uint8_t seq[5] = {1, 2, 3, 2, 4};
        for (int k = 0; k <= step; k++) {
            (void)fx_usb(&p, seq[k], NULL);
        }
        ocb_p2_restart(&p);
        OCB_CHECK(!ocb_p2_ready(&p) && !ocb_p2_reply_pending(&p) && fx_in(&p, out) == 0u,
                  "restart after step %d", step);
        full_sequence(&p, 1000 + step);
    }

    /* 7. Queue of 4: in order, the 5th is QUEUE_FULL and the queue survives. */
    fx_init(&p);
    OCB_CHECK(fx_usb(&p, 0x01, NULL) == OCB_P2_RX_HANDLED, "q1");
    OCB_CHECK(fx_usb(&p, 0x02, NULL) == OCB_P2_RX_HANDLED, "q2");
    OCB_CHECK(fx_usb(&p, 0x03, NULL) == OCB_P2_RX_HANDLED, "q3");
    OCB_CHECK(fx_usb(&p, 0x02, NULL) == OCB_P2_RX_HANDLED, "q4");
    OCB_CHECK(fx_usb(&p, 0x03, NULL) == OCB_P2_RX_QUEUE_FULL, "q5 must be QUEUE_FULL");
    OCB_CHECK(p.cnt.queue_full == 1u, "queue_full counted");
    {
        static const uint8_t order[4] = {0x01, 0x02, 0x03, 0x02};
        for (int k = 0; k < 4; k++) {
            OCB_CHECK(fx_in(&p, out) == 64u && out[0] == 0x81 && out[1] == order[k], "queue order %d: %02x", k,
                      out[1]);
        }
        OCB_CHECK(fx_in(&p, out) == 0u, "queue empty");
    }

    /* 8. Replies come before reports while streaming. */
    fx_init(&p);
    (void)fx_usb(&p, 0x04, NULL);
    (void)fx_usb(&p, 0x02, NULL);
    OCB_CHECK(fx_in(&p, out) == 64u && out[0] == 0x81, "reply first");
    OCB_CHECK(fx_in(&p, out) == 64u && out[0] == 0x30, "then the report");

    /* 9. Unknown 0x80 command: IGNORED, logged as a USB command; a bare 0x80
     * byte is MALFORMED; an empty report is MALFORMED. */
    fx_init(&p);
    {
        ocb_p2_log log;
        OCB_CHECK(fx_usb(&p, 0x7E, &log) == OCB_P2_RX_IGNORED, "unknown 80 7E");
        OCB_CHECK(log.n == 1u && log.kind[0] == 0x01 && log.id[0] == 0x7E, "log of 80 7E");
        const uint8_t one[1] = {0x80};
        OCB_CHECK(ocb_p2_on_output(&p, one, 1, NULL) == OCB_P2_RX_MALFORMED, "80 alone");
        OCB_CHECK(ocb_p2_on_output(&p, NULL, 0, NULL) == OCB_P2_RX_MALFORMED, "empty");
        OCB_CHECK(!ocb_p2_reply_pending(&p) && !ocb_p2_ready(&p), "nothing changed");
    }

    /* 10. Short 0x80 reports (2 bytes) are answered like 64-byte ones. */
    fx_init(&p);
    {
        const uint8_t two[2] = {0x80, 0x02};
        OCB_CHECK(ocb_p2_on_output(&p, two, 2, NULL) == OCB_P2_RX_HANDLED, "2-byte 80 02");
        OCB_CHECK(fx_in(&p, out) == 64u && out[0] == 0x81 && out[1] == 0x02, "reply to 2-byte 80 02");
    }

    /* 11. The real Switch 1 init of the S12 capture, replayed (D34). */
    test_s12_replay();

    return ocb_test_failures() == 0 ? 0 : 1;
}
