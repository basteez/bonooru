/*
 * SPDX-License-Identifier: MIT
 *
 * The 0x30 input report (story 2.3 D8): the canonical state (AD-10) mapped
 * to the standard input report layout of S2 (button bytes, 12-bit stick
 * packing), with S7's fixed fields and the stick range pinned to the
 * calibration the profile serves (D7). Clean-room from S2, S5, S7; no code
 * of any source was used.
 */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include "ocb_p2/config.h"
#include "ocb_p2/internal.h"
#include "ocb_p2/p2.h"
#include "ocb_proto/consts.h"
#include "ocb_proto/state.h"

/* S2 "Standard input report - buttons": byte 3 right, 4 shared, 5 left. */
#define R_Y 0x01u
#define R_X 0x02u
#define R_B 0x04u
#define R_A 0x08u
#define R_R 0x40u
#define R_ZR 0x80u
#define S_MINUS 0x01u
#define S_PLUS 0x02u
#define S_RSTICK 0x04u
#define S_LSTICK 0x08u
#define S_HOME 0x10u
#define S_CAPTURE 0x20u
#define L_DOWN 0x01u
#define L_UP 0x02u
#define L_RIGHT 0x04u
#define L_LEFT 0x08u
#define L_L 0x40u
#define L_ZL 0x80u

/* Hat (AD-10, clockwise from up) to the D-pad bits of byte 5; 8 and
 * above: none. */
static uint8_t hat_bits(uint8_t hat)
{
    static const uint8_t k[8] = {
        L_UP,          L_UP | L_RIGHT,  L_RIGHT, L_DOWN | L_RIGHT,
        L_DOWN,        L_DOWN | L_LEFT, L_LEFT,  L_UP | L_LEFT,
    };
    return hat < 8u ? k[hat] : 0u;
}

/* D8: raw = 0x800 + (v - 0x80) * 14, clamped to 0x001..0xFFF. */
static uint16_t axis(uint8_t v, bool invert)
{
    const int32_t d = invert ? (int32_t)OCB_STICK_CENTER - (int32_t)v : (int32_t)v - (int32_t)OCB_STICK_CENTER;
    int32_t raw = (int32_t)OCB_P2_STICK_CENTER + d * (int32_t)OCB_P2_STICK_SCALE;
    if (raw < 1) {
        raw = 1;
    }
    if (raw > 0xFFF) {
        raw = 0xFFF;
    }
    return (uint16_t)raw;
}

/* S2 stick packing: x = b0 | (b1 & 0x0F) << 8, y = b1 >> 4 | b2 << 4. */
static void pack_stick(uint8_t *o, uint16_t x, uint16_t y)
{
    o[0] = (uint8_t)(x & 0xFFu);
    o[1] = (uint8_t)(((x >> 8) & 0x0Fu) | ((y & 0x0Fu) << 4));
    o[2] = (uint8_t)((y >> 4) & 0xFFu);
}

void ocb_p2_header_(ocb_p2 *p, const ocb_state *s, uint8_t *out)
{
    const uint16_t b = s->buttons;
    uint8_t right = 0u;
    uint8_t shared = (uint8_t)OCB_P2_SHARED_FIXED;
    uint8_t left = hat_bits(s->hat);
    /* Canonical bits 0..13 (AD-10); bits 14 and 15 are never reported. */
    if ((b & OCB_BTN_Y) != 0u) {
        right |= R_Y;
    }
    if ((b & OCB_BTN_B) != 0u) {
        right |= R_B;
    }
    if ((b & OCB_BTN_A) != 0u) {
        right |= R_A;
    }
    if ((b & OCB_BTN_X) != 0u) {
        right |= R_X;
    }
    if ((b & OCB_BTN_L) != 0u) {
        left |= L_L;
    }
    if ((b & OCB_BTN_R) != 0u) {
        right |= R_R;
    }
    if ((b & OCB_BTN_ZL) != 0u) {
        left |= L_ZL;
    }
    if ((b & OCB_BTN_ZR) != 0u) {
        right |= R_ZR;
    }
    if ((b & OCB_BTN_MINUS) != 0u) {
        shared |= S_MINUS;
    }
    if ((b & OCB_BTN_PLUS) != 0u) {
        shared |= S_PLUS;
    }
    if ((b & OCB_BTN_L3) != 0u) {
        shared |= S_LSTICK;
    }
    if ((b & OCB_BTN_R3) != 0u) {
        shared |= S_RSTICK;
    }
    if ((b & OCB_BTN_HOME) != 0u) {
        shared |= S_HOME;
    }
    if ((b & OCB_BTN_CAPTURE) != 0u) {
        shared |= S_CAPTURE;
    }
    out[1] = p->timer;
    p->timer = (uint8_t)(p->timer + 1u);
    out[2] = (uint8_t)OCB_P2_BATTERY_CONN;
    out[3] = right;
    out[4] = shared;
    out[5] = left;
    /* Canonical ly 0x00 = up; the Pro Controller reports up as the larger
     * value (D8), so Y is inverted. */
    pack_stick(&out[6], axis(s->lx, false), axis(s->ly, true));
    pack_stick(&out[9], axis(s->rx, false), axis(s->ry, true));
    out[12] = (uint8_t)OCB_P2_VIBRATOR;
}

size_t ocb_p2_build_report(ocb_p2 *p, const ocb_state *s, uint8_t *out, size_t cap)
{
    if (out == NULL || s == NULL || cap < OCB_P2_REPORT_LEN) {
        return 0u;
    }
    memset(out, 0, OCB_P2_REPORT_LEN); /* IMU [13..48] zero (D7), padding zero */
    out[0] = (uint8_t)OCB_P2_RID_IN_STANDARD;
    ocb_p2_header_(p, s, out);
    return OCB_P2_REPORT_LEN;
}
