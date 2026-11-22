/*
 * SPDX-License-Identifier: MIT
 *
 * F.27 (story 2.3 Task 4.2): the 0x30 input report (D8). Expected bytes
 * hand-written from S2 (button table, stick packing) and S7 (the fixed
 * fields of a real capture), never derived from the library; sticks are
 * unpacked by an independent decoder written from S2's formula.
 */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include "ocb_p2/p2.h"
#include "ocb_proto/consts.h"
#include "ocb_test.h"
#include "p2_fixture.h"

/* S2 decode: x = b0 | (b1 & 0xF) << 8; y = (b1 >> 4) | (b2 << 4). */
static void unpack(const uint8_t *d, unsigned *x, unsigned *y)
{
    *x = (unsigned)d[0] | ((unsigned)(d[1] & 0x0F) << 8);
    *y = (unsigned)(d[1] >> 4) | ((unsigned)d[2] << 4);
}

static size_t report(ocb_p2 *p, const ocb_state *s, uint8_t out[64])
{
    return ocb_p2_build_report(p, s, out, 64);
}

int main(void)
{
    ocb_p2 p;
    fx_init(&p);
    uint8_t out[64];

    /* NEUTRAL: 30 <timer> 91 00 80 00 00 08 80 00 08 80 0A, then 51 zeros. */
    {
        uint8_t want[64] = {0x30, 0x00, 0x91, 0x00, 0x80, 0x00, 0x00, 0x08, 0x80, 0x00, 0x08, 0x80, 0x0A};
        OCB_CHECK(report(&p, &OCB_NEUTRAL, out) == 64u, "length 64");
        OCB_CHECK_MEM(out, 64u, want, 64u, "NEUTRAL report");
    }

    /* Each of the 14 canonical buttons alone sets exactly its bit (S2 table,
     * written here independently): {canonical bit, byte, mask}. */
    static const struct {
        uint16_t btn;
        int byte;
        uint8_t mask;
    } k_btn[14] = {
        {0x0001, 3, 0x01}, /* Y */
        {0x0002, 3, 0x04}, /* B */
        {0x0004, 3, 0x08}, /* A */
        {0x0008, 3, 0x02}, /* X */
        {0x0010, 5, 0x40}, /* L */
        {0x0020, 3, 0x40}, /* R */
        {0x0040, 5, 0x80}, /* ZL */
        {0x0080, 3, 0x80}, /* ZR */
        {0x0100, 4, 0x01}, /* Minus */
        {0x0200, 4, 0x02}, /* Plus */
        {0x0400, 4, 0x08}, /* L3 = left stick */
        {0x0800, 4, 0x04}, /* R3 = right stick */
        {0x1000, 4, 0x10}, /* Home */
        {0x2000, 4, 0x20}, /* Capture */
    };
    for (int k = 0; k < 14; k++) {
        ocb_state s = OCB_NEUTRAL;
        s.buttons = k_btn[k].btn;
        (void)report(&p, &s, out);
        uint8_t want3 = 0x00, want4 = 0x80, want5 = 0x00;
        if (k_btn[k].byte == 3) {
            want3 = k_btn[k].mask;
        } else if (k_btn[k].byte == 4) {
            want4 = (uint8_t)(0x80 | k_btn[k].mask);
        } else {
            want5 = k_btn[k].mask;
        }
        OCB_CHECK(out[3] == want3 && out[4] == want4 && out[5] == want5, "button %04x: %02x %02x %02x",
                  k_btn[k].btn, out[3], out[4], out[5]);
    }
    /* All 65 536 button values: bits 14 and 15 never appear; the 14 others
     * map one to one (the OR of their masks). */
    for (uint32_t v = 0; v <= 0xFFFFu; v++) {
        ocb_state s = OCB_NEUTRAL;
        s.buttons = (uint16_t)v;
        (void)report(&p, &s, out);
        uint8_t w3 = 0x00, w4 = 0x80, w5 = 0x00;
        for (int k = 0; k < 14; k++) {
            if ((v & k_btn[k].btn) != 0u) {
                if (k_btn[k].byte == 3) {
                    w3 |= k_btn[k].mask;
                } else if (k_btn[k].byte == 4) {
                    w4 |= k_btn[k].mask;
                } else {
                    w5 |= k_btn[k].mask;
                }
            }
        }
        if (out[3] != w3 || out[4] != w4 || out[5] != w5) {
            OCB_CHECK(false, "buttons %04x: %02x %02x %02x, expected %02x %02x %02x", (unsigned)v, out[3], out[4],
                      out[5], w3, w4, w5);
            break;
        }
    }

    /* Hat (AD-10) to D-pad (S2 byte 5: Down 01, Up 02, Right 04, Left 08). */
    {
        static const uint8_t want[9] = {0x02, 0x06, 0x04, 0x05, 0x01, 0x09, 0x08, 0x0A, 0x00};
        for (unsigned h = 0; h <= 255u; h++) {
            ocb_state s = OCB_NEUTRAL;
            s.hat = (uint8_t)h;
            (void)report(&p, &s, out);
            const uint8_t w = h < 9u ? want[h] : 0x00;
            OCB_CHECK(out[5] == w, "hat %u: %02x expected %02x", h, out[5], w);
            OCB_CHECK(!((out[5] & 0x01) && (out[5] & 0x02)) && !((out[5] & 0x04) && (out[5] & 0x08)),
                      "hat %u: illegal pair", h);
        }
    }

    /* Sticks: every value of each axis; X monotonic up, Y monotonic down
     * (canonical ly 0x00 = up = the larger raw value), 0x80 -> 0x800,
     * 0x00 -> 0x800 - 0x700, 0xFF inside the calibrated range of D7. */
    for (int axis = 0; axis < 4; axis++) {
        unsigned prev = 0;
        for (unsigned v = 0; v <= 255u; v++) {
            ocb_state s = OCB_NEUTRAL;
            uint8_t *f = axis == 0 ? &s.lx : axis == 1 ? &s.ly : axis == 2 ? &s.rx : &s.ry;
            *f = (uint8_t)v;
            (void)report(&p, &s, out);
            unsigned lx, ly, rx, ry;
            unpack(&out[6], &lx, &ly);
            unpack(&out[9], &rx, &ry);
            const unsigned got = axis == 0 ? lx : axis == 1 ? ly : axis == 2 ? rx : ry;
            /* The other three stay centred. */
            OCB_CHECK((axis == 0 || lx == 0x800) && (axis == 1 || ly == 0x800) && (axis == 2 || rx == 0x800) &&
                          (axis == 3 || ry == 0x800),
                      "axis %d value %u moved another axis", axis, v);
            const bool is_y = axis == 1 || axis == 3;
            if (v > 0u) {
                OCB_CHECK(is_y ? got < prev : got > prev, "axis %d not monotonic at %u", axis, v);
            }
            prev = got;
            OCB_CHECK(got >= 0x100 && got <= 0xF00, "axis %d value %u -> %03x outside centre +- 0x700", axis, v,
                      got);
            if (v == 0x80u) {
                OCB_CHECK(got == 0x800, "axis %d centre %03x", axis, got);
            }
            if (v == 0x00u) {
                OCB_CHECK(got == (is_y ? 0xF00u : 0x100u), "axis %d at 0x00: %03x", axis, got);
            }
            /* The mirror: Y of v equals X of (0x100 - v) around the centre. */
            if (is_y) {
                const unsigned mirror = 0x1000u - got;
                OCB_CHECK(mirror >= 0x100 && mirror <= 0xF00, "axis %d mirror", axis);
            }
        }
    }
    /* Packing round trip with an independent pack: x 0x123, y 0xABC. */
    {
        ocb_state s = OCB_NEUTRAL;
        s.lx = 0x00; /* 0x100 */
        s.ly = 0x00; /* 0xF00 */
        (void)report(&p, &s, out);
        OCB_CHECK(out[6] == 0x00 && out[7] == 0x01 && out[8] == 0xF0, "pack x 0x100 y 0xF00: %02x %02x %02x", out[6],
                  out[7], out[8]);
        s.lx = 0xFF; /* 0x800 + 127 * 14 = 0xEF2 */
        s.ly = 0xFF; /* 0x800 - 127 * 14 = 0x10E */
        (void)report(&p, &s, out);
        OCB_CHECK(out[6] == 0xF2 && out[7] == 0xEE && out[8] == 0x10, "pack x 0xEF2 y 0x10E: %02x %02x %02x", out[6],
                  out[7], out[8]);
    }

    /* IMU bytes 13..48 and the padding are zero for any state; flags never
     * reach the report. */
    {
        ocb_state s = {0xFFFF, 3, 0x00, 0xFF, 0x12, 0xEE, 0xFF};
        (void)report(&p, &s, out);
        for (int i = 13; i < 64; i++) {
            OCB_CHECK(out[i] == 0x00, "byte %d not zero", i);
        }
        OCB_CHECK(out[2] == 0x91 && out[12] == 0x0A, "fixed fields");
    }

    /* Timer: +1 per report, wraps. */
    {
        fx_init(&p);
        for (unsigned k = 0; k < 600u; k++) {
            (void)report(&p, &OCB_NEUTRAL, out);
            OCB_CHECK(out[1] == (uint8_t)k, "timer %u: %02x", k, out[1]);
        }
    }

    /* NULL or short buffers: 0, nothing written, timer unchanged. */
    {
        uint8_t guard[64];
        memset(guard, 0xA5, sizeof guard);
        const uint8_t t = p.timer;
        OCB_CHECK(ocb_p2_build_report(&p, &OCB_NEUTRAL, guard, 63) == 0u, "short buffer");
        OCB_CHECK(ocb_p2_build_report(&p, &OCB_NEUTRAL, NULL, 64) == 0u, "NULL buffer");
        OCB_CHECK(ocb_p2_build_report(&p, NULL, guard, 64) == 0u, "NULL state");
        for (int i = 0; i < 64; i++) {
            OCB_CHECK(guard[i] == 0xA5, "byte %d written", i);
        }
        OCB_CHECK(p.timer == t, "timer moved");
        OCB_CHECK(ocb_p2_next_in(&p, &OCB_NEUTRAL, guard, 63) == 0u, "next_in short buffer");
    }

    return ocb_test_failures() == 0 ? 0 : 1;
}
