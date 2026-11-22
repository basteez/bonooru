/*
 * SPDX-License-Identifier: MIT
 *
 * Host test F.11: ocb_proto API behaviour the vectors cannot express (buffer
 * limits, every-prefix decoding, counters, randomised round trips, splitter
 * equivalence, state predicates). Written to be run under ASan and UBSan
 * (OCB_SANITIZE, story 2.2 D8).
 */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "ocb_proto/ocb_proto.h"
#include "ocb_test.h"

#define GUARD 0xA5

/* Fixed-seed LCG (Numerical Recipes constants): reproducible everywhere. */
static uint32_t g_rng = 0x12345678u;
static uint32_t rnd(void)
{
    g_rng = g_rng * 1664525u + 1013904223u;
    return g_rng >> 8;
}

static void test_cobs_buffer_limits(void)
{
    uint8_t in[300];
    uint8_t out[400];
    for (size_t i = 0; i < sizeof in; i++) {
        in[i] = (uint8_t)(i % 7 == 0 ? 0 : i);
    }
    const size_t full = ocb_cobs_encode(in, sizeof in, out, sizeof out);
    OCB_CHECK(full > 0 && full <= OCB_COBS_MAX_ENCODED(sizeof in), "encode length %zu", full);

    /* Every too-small cap returns 0 and never writes at or past out_cap. */
    for (size_t cap = 0; cap < full; cap++) {
        uint8_t *buf = malloc(cap + 16);
        OCB_CHECK(buf != NULL, "malloc");
        if (buf == NULL) {
            return;
        }
        memset(buf, GUARD, cap + 16);
        const size_t r = ocb_cobs_encode(in, sizeof in, buf, cap);
        OCB_CHECK(r == 0, "cap %zu: encode returned %zu, expected 0", cap, r);
        for (size_t k = cap; k < cap + 16; k++) {
            OCB_CHECK(buf[k] == GUARD, "cap %zu: byte %zu past out_cap written", cap, k);
        }
        free(buf);
    }

    /* Decoding into a too-small buffer fails with SPACE, no overrun. */
    uint8_t raw[300];
    size_t n = 99;
    for (size_t cap = 0; cap < sizeof in; cap += 37) {
        memset(raw, GUARD, sizeof raw);
        const ocb_cobs_result r = ocb_cobs_decode(out, full, raw, cap, &n);
        OCB_CHECK(r == OCB_COBS_ERR_SPACE && n == 0, "decode cap %zu: %d", cap, (int)r);
        for (size_t k = cap; k < sizeof raw; k++) {
            OCB_CHECK(raw[k] == GUARD, "decode cap %zu wrote byte %zu", cap, k);
        }
    }
    /* A malformed input is reported as malformed even when the output also
     * overflows (COBS before length, AD-3). */
    {
        uint8_t bad[12];
        memset(bad, 0x01, sizeof bad);
        bad[11] = 0x05; /* code past the end */
        OCB_CHECK(ocb_cobs_decode(bad, sizeof bad, raw, 4, &n) == OCB_COBS_ERR_TRUNCATED,
                  "truncated input with a full output");
        bad[11] = 0x00; /* zero inside */
        OCB_CHECK(ocb_cobs_decode(bad, sizeof bad, raw, 4, &n) == OCB_COBS_ERR_ZERO,
                  "zero inside with a full output");
        bad[11] = 0x01;
        memset(raw, GUARD, sizeof raw);
        OCB_CHECK(ocb_cobs_decode(bad, sizeof bad, raw, 4, &n) == OCB_COBS_ERR_SPACE && n == 0 &&
                      raw[4] == GUARD,
                  "well-formed input, full output: SPACE, no write past out_cap");
    }
    OCB_CHECK(ocb_cobs_decode(out, full, raw, sizeof raw, &n) == OCB_COBS_OK && n == sizeof in &&
                  memcmp(raw, in, n) == 0,
              "decode round trip");
}

static void test_frame_encode_limits(void)
{
    uint8_t body[256] = {0};
    uint8_t out[OCB_MAX_WIRE_FRAME + 16];

    OCB_CHECK(ocb_frame_encode(0x03, 0, body, 256, out, sizeof out) == 0, "body_len 256 accepted");
    OCB_CHECK(ocb_frame_encode(0x03, 0, body, 255, out, sizeof out) == 262, "255 zero body: 261 + delimiter");
    for (size_t i = 0; i < 255; i++) {
        body[i] = (uint8_t)(i % 255 + 1);
    }
    /* Find a seq whose raw frame has no zero, as the vector generator does. */
    size_t maxlen = 0;
    for (unsigned s = 0; s < 256u; s++) {
        const size_t n = ocb_frame_encode(0x03, (uint8_t)s, body, 255, out, sizeof out);
        if (n > maxlen) {
            maxlen = n;
        }
    }
    OCB_CHECK(maxlen == OCB_MAX_WIRE_FRAME, "largest wire frame %zu", maxlen);
    /* Too-small output: 0, no write past out_cap. */
    for (size_t cap = 0; cap < 19; cap++) { /* the 12-byte body frame needs 19 */
        memset(out, GUARD, sizeof out);
        const size_t n = ocb_frame_encode(0x01, 0, body, 12, out, cap);
        OCB_CHECK(n == 0, "cap %zu: %zu", cap, n);
        for (size_t k = cap; k < sizeof out; k++) {
            OCB_CHECK(out[k] == GUARD, "cap %zu wrote byte %zu", cap, k);
        }
    }
    OCB_CHECK(ocb_frame_encode(0x05, 0, NULL, 0, out, sizeof out) == 7, "STOP with NULL body");
}

static void test_decode_prefixes(void)
{
    /* Every prefix of a valid frame, in an exactly-sized heap block (ASan
     * reports any read past enc_len), never decodes OK unless complete. */
    uint8_t body[40];
    uint8_t wire[OCB_MAX_WIRE_FRAME];
    ocb_frame fr;
    for (size_t i = 0; i < sizeof body; i++) {
        body[i] = (uint8_t)(i * 13);
    }
    const size_t n = ocb_frame_encode(0x03, 7, body, sizeof body, wire, sizeof wire);
    OCB_CHECK(n > 0, "encode");
    const size_t enc_n = n - 1;
    for (size_t k = 0; k <= enc_n; k++) {
        uint8_t *p = malloc(k > 0 ? k : 1);
        if (p == NULL) {
            OCB_CHECK(0, "malloc");
            return;
        }
        memcpy(p, wire, k);
        const ocb_err e = ocb_frame_decode(OCB_RX_CONTROLLER, p, k, &fr);
        if (k == enc_n) {
            OCB_CHECK(e == OCB_OK && fr.len == sizeof body && memcmp(fr.body, body, fr.len) == 0,
                      "full frame: %d", (int)e);
        } else {
            OCB_CHECK(e != OCB_OK, "prefix %zu decoded OK", k);
        }
        free(p);
    }
    /* Overlong input to the frame decoder. */
    uint8_t big[OCB_MAX_ENCODED_FRAME + 1];
    memset(big, 0x11, sizeof big);
    OCB_CHECK(ocb_frame_decode(OCB_RX_CONTROLLER, big, sizeof big, &fr) == OCB_ERR_OVERLONG,
              "overlong direct decode");
    /* Random garbage of every length never crashes and never exceeds the
     * receive range rules. */
    for (int iter = 0; iter < 20000; iter++) {
        const size_t len = rnd() % (OCB_MAX_ENCODED_FRAME + 1);
        uint8_t *p = malloc(len > 0 ? len : 1);
        if (p == NULL) {
            OCB_CHECK(0, "malloc");
            return;
        }
        for (size_t i = 0; i < len; i++) {
            p[i] = (uint8_t)rnd();
        }
        const ocb_err e = ocb_frame_decode((iter & 1) ? OCB_RX_APP : OCB_RX_CONTROLLER, p, len, &fr);
        if (e == OCB_OK) {
            OCB_CHECK(ocb_frame_type_in_range((iter & 1) ? OCB_RX_APP : OCB_RX_CONTROLLER, fr.type),
                      "accepted type %02x out of range", fr.type);
        }
        free(p);
    }
}

static size_t random_body(uint8_t *body)
{
    const size_t len = rnd() % (OCB_MAX_BODY + 1);
    const uint32_t style = rnd() % 4u;
    for (size_t i = 0; i < len; i++) {
        switch (style) {
        case 0:
            body[i] = (uint8_t)rnd();
            break;
        case 1:
            body[i] = 0; /* all zeros */
            break;
        case 2:
            body[i] = (uint8_t)(rnd() % 255u + 1u); /* no zeros: 0xFF blocks */
            break;
        default:
            body[i] = (rnd() % 8u == 0u) ? 0 : 0xFF; /* sparse zeros in 0xFF runs */
            break;
        }
    }
    return len;
}

static void test_random_round_trip_and_splitter(void)
{
    enum { N = 10000 };
    static uint8_t stream[N * OCB_MAX_WIRE_FRAME / 16]; /* filled in chunks below */
    uint8_t body[OCB_MAX_BODY];
    uint8_t wire[OCB_MAX_WIRE_FRAME];
    ocb_frame fr;
    ocb_rx rx;
    int mismatches = 0;

    ocb_rx_init(&rx, OCB_RX_CONTROLLER);
    size_t sn = 0;
    unsigned long frames_pushed = 0, frames_seen = 0;
    for (int i = 0; i < N; i++) {
        const uint8_t type = (uint8_t)(rnd() % 0x7Fu + 1u);
        const uint8_t seq = (uint8_t)rnd();
        const size_t len = random_body(body);
        const size_t n = ocb_frame_encode(type, seq, body, len, wire, sizeof wire);
        if (n == 0 || n > OCB_MAX_WIRE_FRAME || wire[n - 1] != 0) {
            mismatches++;
            continue;
        }
        for (size_t k = 0; k + 1 < n; k++) {
            if (wire[k] == 0) {
                mismatches++;
            }
        }
        /* One-shot decode. */
        if (ocb_frame_decode(OCB_RX_CONTROLLER, wire, n - 1, &fr) != OCB_OK || fr.type != type ||
            fr.seq != seq || fr.len != len || (len > 0 && memcmp(fr.body, body, len) != 0)) {
            mismatches++;
        }
        /* Byte-at-a-time through the splitter gives the same frame. */
        for (size_t k = 0; k < n; k++) {
            const ocb_rx_result r = ocb_rx_push(&rx, wire[k], &fr);
            if (k + 1 < n && r != OCB_RX_NONE) {
                mismatches++;
            }
            if (k + 1 == n) {
                if (r != OCB_RX_FRAME || fr.type != type || fr.seq != seq || fr.len != len ||
                    (len > 0 && memcmp(fr.body, body, len) != 0)) {
                    mismatches++;
                }
                frames_seen++;
            }
        }
        frames_pushed++;
        /* Keep a concatenation of the first frames for the batch check. */
        if (sn + n <= sizeof stream) {
            memcpy(stream + sn, wire, n);
            sn += n;
        }
    }
    OCB_CHECK(mismatches == 0, "%d round-trip mismatches", mismatches);
    OCB_CHECK(rx.stats.frames_ok == frames_pushed && frames_seen == frames_pushed,
              "splitter delivered %u of %lu", (unsigned)rx.stats.frames_ok, frames_pushed);

    /* A second splitter over the concatenation yields the same count. */
    ocb_rx rx2;
    unsigned long n2 = 0, delims = 0;
    ocb_rx_init(&rx2, OCB_RX_CONTROLLER);
    for (size_t k = 0; k < sn; k++) {
        delims += stream[k] == 0;
        n2 += ocb_rx_push(&rx2, stream[k], &fr) == OCB_RX_FRAME;
    }
    OCB_CHECK(n2 == delims && rx2.stats.frames_ok == n2, "concatenation: %lu frames, %lu delims",
              n2, delims);
}

static void test_counters_add_up(void)
{
    /* A noisy stream: random bytes with frequent delimiters, plus valid
     * frames, plus overlong runs. Every delimiter ends exactly one run, so
     * the six counters add up to the number of delimiters (each overlong run
     * here is closed by a delimiter). */
    ocb_rx rx;
    ocb_frame fr;
    uint8_t wire[OCB_MAX_WIRE_FRAME];
    uint8_t body[8] = {1, 2, 3, 4, 5, 6, 7, 8};
    unsigned long delims = 0, frames = 0, dropped = 0;
    ocb_rx_init(&rx, OCB_RX_CONTROLLER);
    for (int i = 0; i < 5000; i++) {
        const uint32_t what = rnd() % 5u;
        if (what == 0) {
            const size_t n = ocb_frame_encode((uint8_t)(rnd() % 0x80u), (uint8_t)i, body, sizeof body,
                                              wire, sizeof wire);
            for (size_t k = 0; k < n; k++) {
                const ocb_rx_result r = ocb_rx_push(&rx, wire[k], &fr);
                frames += r == OCB_RX_FRAME;
                dropped += r == OCB_RX_DROPPED;
            }
            delims++;
        } else if (what == 1) {
            const size_t len = 263u + rnd() % 400u;
            for (size_t k = 0; k < len; k++) {
                const ocb_rx_result r = ocb_rx_push(&rx, (uint8_t)(rnd() % 255u + 1u), &fr);
                OCB_CHECK(r == OCB_RX_NONE, "non-delimiter byte returned %d", (int)r);
            }
            dropped += ocb_rx_push(&rx, 0, &fr) == OCB_RX_DROPPED;
            delims++;
        } else {
            const size_t len = rnd() % 20u;
            for (size_t k = 0; k < len; k++) {
                const uint8_t b = (uint8_t)(rnd() % 255u + 1u);
                (void)ocb_rx_push(&rx, b, &fr);
            }
            const ocb_rx_result r = ocb_rx_push(&rx, 0, &fr);
            frames += r == OCB_RX_FRAME;
            dropped += r == OCB_RX_DROPPED;
            delims++;
        }
    }
    const ocb_rx_stats *s = &rx.stats;
    const unsigned long sum = (unsigned long)s->frames_ok + s->empty + s->cobs_errors +
                              s->crc_errors + s->range_errors + s->overlong;
    OCB_CHECK(sum == delims, "counters sum %lu, delimiters %lu", sum, delims);
    OCB_CHECK(frames == s->frames_ok, "FRAME results %lu vs frames_ok %u", frames,
              (unsigned)s->frames_ok);
    OCB_CHECK(frames + dropped == delims, "FRAME + DROPPED = %lu, delimiters %lu",
              frames + dropped, delims);
    OCB_CHECK(s->overlong > 0 && s->range_errors > 0 && s->empty > 0, "every class exercised");

    /* Two splitters coexist (no global state). */
    ocb_rx a, b;
    ocb_rx_init(&a, OCB_RX_CONTROLLER);
    ocb_rx_init(&b, OCB_RX_APP);
    const size_t n = ocb_frame_encode(OCB_MSG_ACK, 1, body, 1, wire, sizeof wire);
    ocb_rx_result ra = OCB_RX_NONE, rb = OCB_RX_NONE;
    for (size_t k = 0; k < n; k++) {
        ra = ocb_rx_push(&a, wire[k], &fr);
        rb = ocb_rx_push(&b, wire[k], &fr);
    }
    OCB_CHECK(ra == OCB_RX_DROPPED && a.last_err == OCB_ERR_RANGE && a.stats.range_errors == 1,
              "controller side drops ACK");
    OCB_CHECK(rb == OCB_RX_FRAME && b.stats.frames_ok == 1, "app side accepts ACK");
}

static void test_state(void)
{
    uint8_t out[OCB_STATE_SIZE];
    static const uint8_t neutral_wire[OCB_STATE_SIZE] = {0x00, 0x00, 0x08, 0x80,
                                                         0x80, 0x80, 0x80, 0x00};
    ocb_state s = OCB_NEUTRAL;

    ocb_state_encode(&OCB_NEUTRAL, out);
    OCB_CHECK_MEM(out, sizeof out, neutral_wire, sizeof neutral_wire, "NEUTRAL on the wire");
    OCB_CHECK(ocb_state_is_neutral(&s) && ocb_state_is_neutral_pose(&s) && !ocb_state_has_end(&s),
              "NEUTRAL predicates");
    s.flags = OCB_STATE_FLAG_END;
    OCB_CHECK(!ocb_state_is_neutral(&s) && ocb_state_is_neutral_pose(&s) && ocb_state_has_end(&s) &&
                  ocb_state_reserved_bits_ok(&s),
              "NEUTRAL|END: pose neutral, not neutral, END set");
    s.flags = 0x02;
    OCB_CHECK(!ocb_state_reserved_bits_ok(&s) && !ocb_state_has_end(&s), "reserved flag bit");
    s = OCB_NEUTRAL;
    s.buttons = OCB_BTN_A;
    OCB_CHECK(!ocb_state_is_neutral_pose(&s), "a pressed button is not a neutral pose");
    s = OCB_NEUTRAL;
    s.ly = 0x7F;
    OCB_CHECK(!ocb_state_is_neutral_pose(&s), "a moved stick is not a neutral pose");
    s = OCB_NEUTRAL;
    s.buttons = 0xC000; /* bits 14 and 15: not validated by the codec */
    ocb_state_encode(&s, out);
    OCB_CHECK(out[0] == 0x00 && out[1] == 0xC0 && ocb_state_reserved_bits_ok(&s),
              "unassigned button bits pass through");
    /* Byte order and field offsets of the encoding. */
    const ocb_state t = {0x1234, 0x05, 0x10, 0x20, 0x30, 0x40, 0x01};
    static const uint8_t t_wire[OCB_STATE_SIZE] = {0x34, 0x12, 0x05, 0x10, 0x20, 0x30, 0x40, 0x01};
    ocb_state_encode(&t, out);
    OCB_CHECK_MEM(out, sizeof out, t_wire, sizeof t_wire, "state field order");
    ocb_state u;
    ocb_state_decode(t_wire, &u);
    OCB_CHECK(u.buttons == 0x1234 && u.hat == 5 && u.lx == 0x10 && u.ly == 0x20 && u.rx == 0x30 &&
                  u.ry == 0x40 && u.flags == 1,
              "state decode");
}

static void test_msgs_limits(void)
{
    uint8_t buf[OCB_MAX_BODY + 4];
    const ocb_msg_info info = {1, {0, 2, 3}, {0, 2, 1}, OCB_HID_PROFILE_P2, 512, 1000, 8000};
    OCB_CHECK(ocb_msg_info_encode(&info, buf, OCB_BODY_LEN_INFO - 1) == OCB_ERR_BUF,
              "INFO into 13 bytes");
    OCB_CHECK(ocb_msg_info_encode(&info, buf, OCB_BODY_LEN_INFO) == OCB_BODY_LEN_INFO, "INFO");

    static const uint8_t extra[3] = {0x10, 0x20, 0x30};
    ocb_msg_status st = {500, 0x123456, 0x41, OCB_MODE_PLAYING, 3, OCB_EVT_PLAYBACK_DONE,
                         OCB_STATUS_LINK_OK, extra, sizeof extra};
    OCB_CHECK(ocb_msg_status_encode(&st, buf, 13) == OCB_ERR_BUF, "STATUS + extra into 13 bytes");
    OCB_CHECK(ocb_msg_status_encode(&st, buf, 14) == 14, "STATUS + 3 extra");
    st.extra = NULL; /* extra_len > 0 without bytes: refused, not silently dropped */
    OCB_CHECK(ocb_msg_status_encode(&st, buf, sizeof buf) == OCB_ERR_BUF, "STATUS extra NULL");
    st.extra = extra;
    st.extra_len = OCB_MAX_BODY; /* would exceed the frame body */
    OCB_CHECK(ocb_msg_status_encode(&st, buf, sizeof buf) == OCB_ERR_BUF, "STATUS extra too big");
    ocb_msg_status d;
    OCB_CHECK(ocb_msg_status_decode(buf, 11, &d) == OCB_OK && d.extra == NULL && d.extra_len == 0,
              "STATUS base: no extra");
    OCB_CHECK(ocb_msg_status_decode(buf, 14, &d) == OCB_OK && d.extra == &buf[11] &&
                  d.extra_len == 3,
              "STATUS extra points into the body");

    /* Unaligned bodies: decode from odd addresses (UBSan alignment check). */
    uint8_t odd[32];
    const ocb_msg_queue_event qe = {0x01020304u, OCB_NEUTRAL_INIT};
    OCB_CHECK(ocb_msg_queue_event_encode(&qe, odd + 1, sizeof odd - 1) == 12, "QE encode odd");
    OCB_CHECK(odd[1] == 0x04 && odd[2] == 0x03 && odd[3] == 0x02 && odd[4] == 0x01,
              "t_us little-endian");
    ocb_msg_queue_event q2;
    OCB_CHECK(ocb_msg_queue_event_decode(odd + 1, 12, &q2) == OCB_OK && q2.t_us == 0x01020304u &&
                  ocb_state_is_neutral(&q2.state),
              "QE decode odd");
    OCB_CHECK(ocb_msg_stop_decode(NULL, 0) == OCB_OK && ocb_msg_ping_decode(odd, 1) == OCB_ERR_BODY_LEN,
              "STOP and PING bodies");
    OCB_CHECK(OCB_NAK_IS_TRANSPORT(OCB_NAK_FULL) && !OCB_NAK_IS_TRANSPORT(OCB_NAK_VERSION) &&
                  OCB_NAK_IS_SEMANTIC(OCB_NAK_ABORTED) && !OCB_NAK_IS_SEMANTIC(OCB_NAK_CRC) &&
                  !OCB_NAK_IS_TRANSPORT(0x00) && !OCB_NAK_IS_SEMANTIC(0x80),
              "NAK classes");
}

static void test_crc(void)
{
    static const uint8_t check[] = {'1', '2', '3', '4', '5', '6', '7', '8', '9'};
    OCB_CHECK(ocb_crc16(check, sizeof check) == OCB_CRC16_CHECK, "CRC check value");
    const uint16_t part = ocb_crc16_update(OCB_CRC16_INIT, check, 4);
    OCB_CHECK(ocb_crc16_update(part, check + 4, 5) == OCB_CRC16_CHECK, "incremental CRC");
    OCB_CHECK(ocb_crc16(NULL, 0) == OCB_CRC16_INIT, "empty CRC");
}

int main(void)
{
    test_crc();
    test_cobs_buffer_limits();
    test_frame_encode_limits();
    test_decode_prefixes();
    test_random_round_trip_and_splitter();
    test_counters_add_up();
    test_state();
    test_msgs_limits();
    if (ocb_test_failures() != 0) {
        fprintf(stderr, "test_proto_api: %d failure(s)\n", ocb_test_failures());
        return 1;
    }
    printf("test_proto_api: ok\n");
    return 0;
}
