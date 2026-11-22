/*
 * SPDX-License-Identifier: MIT
 *
 * Published-state handoff, core 1 to core 0 (story 2.3 D35).
 */
#include <stdbool.h>
#include <stdint.h>

#include "ocb_link/config.h"
#include "ocb_link/pubstate.h"
#include "ocb_proto/state.h"

void ocb_pubstate_init(ocb_pubstate *ps)
{
    ps->seq = 0u;
    ocb_pubstate_write_begin(ps);
    ps->buttons = OCB_NEUTRAL.buttons;
    ps->b[0] = OCB_NEUTRAL.hat;
    ps->b[1] = OCB_NEUTRAL.lx;
    ps->b[2] = OCB_NEUTRAL.ly;
    ps->b[3] = OCB_NEUTRAL.rx;
    ps->b[4] = OCB_NEUTRAL.ry;
    ps->b[5] = OCB_NEUTRAL.flags;
    ocb_pubstate_write_end(ps);
    ps->seq = 0u;
}

void ocb_pubstate_write_begin(ocb_pubstate *ps)
{
    ps->seq = ps->seq + 1u; /* odd */
    OCB_LINK_BARRIER();
}

void ocb_pubstate_write_end(ocb_pubstate *ps)
{
    OCB_LINK_BARRIER();
    ps->seq = ps->seq + 1u; /* even */
}

void ocb_pubstate_write(ocb_pubstate *ps, const ocb_state *s)
{
    ocb_pubstate_write_begin(ps);
    ps->buttons = s->buttons;
    ps->b[0] = s->hat;
    ps->b[1] = s->lx;
    ps->b[2] = s->ly;
    ps->b[3] = s->rx;
    ps->b[4] = s->ry;
    ps->b[5] = s->flags;
    ocb_pubstate_write_end(ps);
}

bool ocb_pubstate_read(const ocb_pubstate *ps, ocb_state *out)
{
    for (uint32_t t = 0u; t < OCB_PUBSTATE_TRIES; t++) {
        const uint32_t s0 = ps->seq;
        if ((s0 & 1u) != 0u) {
            continue;
        }
        OCB_LINK_BARRIER();
        ocb_state c;
        c.buttons = ps->buttons;
        c.hat = ps->b[0];
        c.lx = ps->b[1];
        c.ly = ps->b[2];
        c.rx = ps->b[3];
        c.ry = ps->b[4];
        c.flags = ps->b[5];
        OCB_LINK_BARRIER();
        if (ps->seq == s0) {
            *out = c;
            return true;
        }
    }
    *out = OCB_NEUTRAL;
    return false;
}
