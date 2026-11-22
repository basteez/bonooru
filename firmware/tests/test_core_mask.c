/*
 * SPDX-License-Identifier: MIT
 *
 * Host test F.6 (story 2.4): the PLAYING safety mask (AD-9, D7). Only L, R,
 * A, X, Y, the hat and the left stick pass; everything else is cleared and
 * the right stick centred; flags are never published. DIRECT is unmasked.
 *
 * The allow-list is confirmed unchanged by the U11 result of Story 1.1
 * (docs/ocarina-input-mechanics.md, table "Unknowns", row U11): ZL opens the
 * song menu, ZR does nothing, the right stick moves the camera; masking ZL
 * and the right stick is desirable. Expected values are written by hand.
 */
#include <stdint.h>
#include <stdio.h>

#include "core_fixture.h"

/* Y 0x0001 | A 0x0004 | X 0x0008 | L 0x0010 | R 0x0020, written by hand. */
#define HAND_ALLOWED 0x003Du

static const struct {
    uint16_t bit;
    const char *name;
    bool survives;
} bits[16] = {
    {0x0001u, "Y", true},
    {0x0002u, "B", false}, /* exits free play (U9) */
    {0x0004u, "A", true},
    {0x0008u, "X", true},
    {0x0010u, "L", true},
    {0x0020u, "R", true},
    {0x0040u, "ZL", false}, /* U11: opens the song menu */
    {0x0080u, "ZR", false}, /* U11: no note, masked anyway */
    {0x0100u, "Minus", false},
    {0x0200u, "Plus", false},
    {0x0400u, "L3", false},
    {0x0800u, "R3", false},
    {0x1000u, "Home", false},
    {0x2000u, "Capture", false},
    {0x4000u, "bit14", false},
    {0x8000u, "bit15", false},
};

static void test_allow_list_constant(void)
{
    OCB_CHECK((unsigned)OCB_CORE_ALLOWED_BUTTONS == HAND_ALLOWED, "OCB_CORE_ALLOWED_BUTTONS 0x%04x, wanted 0x%04x",
              (unsigned)OCB_CORE_ALLOWED_BUTTONS, HAND_ALLOWED);
}

static void test_single_bits(void)
{
    for (int i = 0; i < 16; i++) {
        const ocb_state in = st_buttons(bits[i].bit);
        const ocb_state out = ocb_core_mask_state(&in);
        const uint16_t want = bits[i].survives ? bits[i].bit : 0u;
        OCB_CHECK(out.buttons == want, "button %s (0x%04x): masked to 0x%04x, wanted 0x%04x", bits[i].name,
                  bits[i].bit, out.buttons, want);
    }
}

static void test_all_fields(void)
{
    for (uint32_t b = 0; b <= 0xFFFFu; b++) {
        const ocb_state in = st_buttons((uint16_t)b);
        const ocb_state out = ocb_core_mask_state(&in);
        if (out.buttons != (uint16_t)(b & HAND_ALLOWED)) {
            OCB_CHECK(false, "buttons 0x%04x masked to 0x%04x", (unsigned)b, out.buttons);
        }
    }
    for (uint32_t x = 0; x <= 0xFFu; x++) {
        for (uint32_t y = 0; y <= 0xFFu; y++) {
            ocb_state in = st_neutral();
            in.rx = (uint8_t)x;
            in.ry = (uint8_t)y;
            in.lx = (uint8_t)y;
            in.ly = (uint8_t)x;
            const ocb_state out = ocb_core_mask_state(&in);
            if (out.rx != 0x80u || out.ry != 0x80u) {
                OCB_CHECK(false, "right stick (%u,%u) not centred", (unsigned)x, (unsigned)y);
            }
            if (out.lx != (uint8_t)y || out.ly != (uint8_t)x) {
                OCB_CHECK(false, "left stick (%u,%u) changed", (unsigned)y, (unsigned)x);
            }
        }
    }
    for (uint32_t hat = 0; hat <= 8u; hat++) {
        ocb_state in = st_neutral();
        in.hat = (uint8_t)hat;
        const ocb_state out = ocb_core_mask_state(&in);
        OCB_CHECK(out.hat == (uint8_t)hat, "hat %u changed to %u", (unsigned)hat, out.hat);
    }
    for (uint32_t fl = 0; fl <= 0xFFu; fl++) {
        ocb_state in = st_neutral();
        in.flags = (uint8_t)fl;
        const ocb_state out = ocb_core_mask_state(&in);
        if (out.flags != 0u) {
            OCB_CHECK(false, "flags 0x%02x not cleared", (unsigned)fl);
        }
    }
}

static ocb_state value_state(uint32_t b)
{
    ocb_state s = st_buttons((uint16_t)b);
    s.hat = (uint8_t)(b % 9u);
    s.lx = (uint8_t)(b & 0xFFu);
    s.ly = (uint8_t)(b >> 8);
    s.rx = (uint8_t)(b >> 8);
    s.ry = (uint8_t)(b & 0xFFu);
    return s;
}

/* The full path: QUEUE_EVENT in PLAYING, tick, read published, for every
 * buttons value (sticks and hat varying alongside). One event of lookahead
 * stays queued so the apply never underruns; the piece ends with END. */
static void test_full_path_playing(void)
{
    static harness h;
    h_init(&h, 4u, 1000u);
    const uint32_t step = 10u;
    OCB_CHECK(h_queue_event(&h, step, value_state(0u)).kind == OCB_CORE_REPLY_ACK, "prefill");
    OCB_CHECK(h_start(&h, 0u).kind == OCB_CORE_REPLY_ACK, "START");
    const uint64_t t0 = h.now;
    for (uint32_t b = 0; b <= 0xFFFFu; b++) {
        const uint32_t t_next = (b + 2u) * step;
        const ocb_state next = b < 0xFFFFu ? value_state(b + 1u) : st_end();
        const ocb_core_out q = h_queue_event(&h, t_next, next);
        if (q.kind != OCB_CORE_REPLY_ACK) {
            OCB_CHECK(false, "value 0x%04x: QUEUE_EVENT NAK 0x%02x", (unsigned)(b + 1u), q.nak_reason);
            return;
        }
        h.now = t0 + (uint64_t)(b + 1u) * step;
        const ocb_core_out o = h_tick(&h);
        const ocb_state s = value_state(b);
        const ocb_state *p = &h.core.published;
        const bool ok = o.event == OCB_EVT_NONE && p->buttons == (uint16_t)(b & HAND_ALLOWED) && p->rx == 0x80u &&
                        p->ry == 0x80u && p->lx == s.lx && p->ly == s.ly && p->hat == s.hat && p->flags == 0u;
        if (!ok) {
            OCB_CHECK(false, "PLAYING buttons 0x%04x: published %04x hat %u l(%u,%u) r(%u,%u) flags %u event %u",
                      (unsigned)b, p->buttons, p->hat, p->lx, p->ly, p->rx, p->ry, p->flags, o.event);
            return;
        }
    }
    h.now = t0 + (uint64_t)0x10001u * step;
    const ocb_core_out o = h_tick(&h);
    OCB_CHECK(o.event == OCB_EVT_PLAYBACK_DONE && h_published_neutral(&h), "final END");
}

/* The 16 single bits through the full path, named. */
static void test_single_bits_playing(void)
{
    for (int i = 0; i < 16; i++) {
        harness h;
        h_init(&h, 4u, 1000u);
        OCB_CHECK(h_queue_event(&h, 100u, st_buttons(bits[i].bit)).kind == OCB_CORE_REPLY_ACK, "queue");
        OCB_CHECK(h_queue_event(&h, 200u, st_end()).kind == OCB_CORE_REPLY_ACK, "queue END");
        OCB_CHECK(h_start(&h, 0u).kind == OCB_CORE_REPLY_ACK, "START");
        h.now += 100u;
        (void)h_tick(&h);
        const uint16_t want = bits[i].survives ? bits[i].bit : 0u;
        OCB_CHECK(h.core.published.buttons == want, "PLAYING button %s: published 0x%04x, wanted 0x%04x",
                  bits[i].name, h.core.published.buttons, want);
    }
}

/* DIRECT (SET_STATE) is unmasked: every buttons value, the right stick and
 * the undefined bits are published as sent. */
static void test_direct_unmasked(void)
{
    static harness h;
    h_init(&h, 4u, 1000u);
    for (uint32_t b = 0; b <= 0xFFFFu; b++) {
        const ocb_state s = value_state(b);
        const ocb_core_out o = h_set_state(&h, s);
        if (o.kind != OCB_CORE_REPLY_ACK || !st_eq(&h.core.published, &s) || h.core.mode != OCB_MODE_DIRECT) {
            OCB_CHECK(false, "DIRECT buttons 0x%04x not published as sent", (unsigned)b);
            return;
        }
    }
    /* Named cases: B, Home, Capture, ZL (U11), ZR (U11), bits 14/15 and a
     * right stick pushed (U11: camera) all pass in DIRECT. */
    const uint16_t named[] = {OCB_BTN_B, OCB_BTN_HOME, OCB_BTN_CAPTURE, OCB_BTN_ZL, OCB_BTN_ZR, 0x4000u, 0x8000u};
    for (size_t i = 0; i < sizeof named / sizeof named[0]; i++) {
        ocb_state s = st_buttons(named[i]);
        s.rx = 0x00;
        s.ry = 0xFF;
        (void)h_set_state(&h, s);
        OCB_CHECK(st_eq(&h.core.published, &s), "DIRECT 0x%04x with right stick masked", named[i]);
    }

    /* START from DIRECT publishes NEUTRAL (AD-8). */
    OCB_CHECK(h_queue_event(&h, 1000u, st_end()).kind == OCB_CORE_REPLY_ACK, "queue END");
    OCB_CHECK(h_start(&h, 500u).kind == OCB_CORE_REPLY_ACK, "START from DIRECT");
    OCB_CHECK(h.core.mode == OCB_MODE_PLAYING && h_published_neutral(&h), "START from DIRECT not NEUTRAL");
}

int main(void)
{
    test_allow_list_constant();
    test_single_bits();
    test_all_fields();
    test_full_path_playing();
    test_single_bits_playing();
    test_direct_unmasked();
    const int f = ocb_test_failures();
    if (f != 0) {
        fprintf(stderr, "test_core_mask (F.6): %d failure(s)\n", f);
        return 1;
    }
    printf("test_core_mask (F.6): ok\n");
    return 0;
}
