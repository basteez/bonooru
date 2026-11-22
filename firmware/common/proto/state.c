/*
 * SPDX-License-Identifier: MIT
 *
 * Canonical controller state (AD-10).
 */
#include "ocb_proto/state.h"

const ocb_state OCB_NEUTRAL = OCB_NEUTRAL_INIT;

void ocb_state_encode(const ocb_state *s, uint8_t out[OCB_STATE_SIZE])
{
    out[0] = (uint8_t)(s->buttons & 0xFFu);
    out[1] = (uint8_t)(s->buttons >> 8);
    out[2] = s->hat;
    out[3] = s->lx;
    out[4] = s->ly;
    out[5] = s->rx;
    out[6] = s->ry;
    out[7] = s->flags;
}

void ocb_state_decode(const uint8_t in[OCB_STATE_SIZE], ocb_state *s)
{
    s->buttons = (uint16_t)((uint16_t)in[0] | (uint16_t)((uint16_t)in[1] << 8));
    s->hat = in[2];
    s->lx = in[3];
    s->ly = in[4];
    s->rx = in[5];
    s->ry = in[6];
    s->flags = in[7];
}

bool ocb_state_is_neutral_pose(const ocb_state *s)
{
    return s->buttons == OCB_NEUTRAL.buttons && s->hat == OCB_NEUTRAL.hat &&
           s->lx == OCB_NEUTRAL.lx && s->ly == OCB_NEUTRAL.ly && s->rx == OCB_NEUTRAL.rx &&
           s->ry == OCB_NEUTRAL.ry;
}

bool ocb_state_is_neutral(const ocb_state *s)
{
    return ocb_state_is_neutral_pose(s) && s->flags == OCB_NEUTRAL.flags;
}

bool ocb_state_has_end(const ocb_state *s)
{
    return (s->flags & OCB_STATE_FLAG_END) != 0u;
}

bool ocb_state_reserved_bits_ok(const ocb_state *s)
{
    return (s->flags & OCB_STATE_RESERVED_MASK) == 0u;
}
