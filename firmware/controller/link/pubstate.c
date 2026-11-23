/*
 * SPDX-License-Identifier: MIT
 *
 * Published-state handoff, core 1 to core 0: a sequence-locked double
 * buffer (AD-11; story 2.3 D35, story 2.5 D4, D22). Rules in pubstate.h.
 */
#include <stdbool.h>
#include <stdint.h>

#include "ocb_link/config.h"
#include "ocb_link/pubstate.h"
#include "ocb_proto/state.h"

static OCB_CORE_RAMFUNC uint32_t sat_inc(uint32_t v)
{
    return v == UINT32_MAX ? v : v + 1u;
}

void ocb_pubstate_init(ocb_pubstate *ps)
{
    for (int k = 0; k < 2; k++) {
        ps->buttons[k] = OCB_NEUTRAL.buttons;
        ps->b[k][0] = OCB_NEUTRAL.hat;
        ps->b[k][1] = OCB_NEUTRAL.lx;
        ps->b[k][2] = OCB_NEUTRAL.ly;
        ps->b[k][3] = OCB_NEUTRAL.rx;
        ps->b[k][4] = OCB_NEUTRAL.ry;
        ps->b[k][5] = OCB_NEUTRAL.flags;
    }
    ps->gen = 0u;
    ps->written_count = 0u;
    ps->written = false;
    ps->retries = 0u;
    ps->fallbacks = 0u;
}

OCB_CORE_RAMFUNC void ocb_pubstate_write_begin(ocb_pubstate *ps)
{
    ps->gen = ps->gen + 1u; /* odd: slot ((gen + 1) >> 1) & 1 is being filled */
    OCB_LINK_BARRIER();
}

OCB_CORE_RAMFUNC void ocb_pubstate_write_fields(ocb_pubstate *ps, const ocb_state *s, int part)
{
    /* The slot of the publish in progress: gen is odd (2c + 1), the new
     * state is publish c + 1, slot (c + 1) & 1. */
    const uint32_t k = ((ps->gen >> 1) + 1u) & 1u;
    if (part == 0) {
        ps->buttons[k] = s->buttons;
        ps->b[k][0] = s->hat;
        ps->b[k][1] = s->lx;
    } else {
        ps->b[k][2] = s->ly;
        ps->b[k][3] = s->rx;
        ps->b[k][4] = s->ry;
        ps->b[k][5] = s->flags;
    }
}

OCB_CORE_RAMFUNC void ocb_pubstate_write_end(ocb_pubstate *ps)
{
    OCB_LINK_BARRIER();
    ps->gen = ps->gen + 1u; /* even: the slot is published */
}

OCB_CORE_RAMFUNC void ocb_pubstate_write(ocb_pubstate *ps, const ocb_state *s)
{
    ocb_pubstate_write_begin(ps);
    ocb_pubstate_write_fields(ps, s, 0);
    ocb_pubstate_write_fields(ps, s, 1);
    ocb_pubstate_write_end(ps);
}

OCB_CORE_RAMFUNC bool ocb_pubstate_publish(ocb_pubstate *ps, const ocb_state *s, uint32_t publish_count)
{
    if (ps->written && publish_count == ps->written_count) {
        return false;
    }
    ocb_pubstate_write(ps, s);
    ps->written_count = publish_count;
    ps->written = true;
    return true;
}

OCB_CORE_RAMFUNC uint32_t ocb_pubstate_try_begin(const ocb_pubstate *ps)
{
    const uint32_t g0 = ps->gen;
    OCB_LINK_BARRIER();
    return g0;
}

OCB_CORE_RAMFUNC void ocb_pubstate_try_copy(const ocb_pubstate *ps, uint32_t g0, ocb_state *c, int part)
{
    const uint32_t k = (g0 >> 1) & 1u; /* the slot of the last completed publish */
    if (part == 0) {
        c->buttons = ps->buttons[k];
        c->hat = ps->b[k][0];
        c->lx = ps->b[k][1];
    } else {
        c->ly = ps->b[k][2];
        c->rx = ps->b[k][3];
        c->ry = ps->b[k][4];
        c->flags = ps->b[k][5];
    }
}

OCB_CORE_RAMFUNC bool ocb_pubstate_try_end(const ocb_pubstate *ps, uint32_t g0)
{
    OCB_LINK_BARRIER();
    const uint32_t g1 = ps->gen;
    /* Unsigned difference: wrap-safe. <= 2 half steps from the even floor
     * of g0 means the slot read was not rewritten during the copy. */
    return (uint32_t)(g1 - (g0 & ~1u)) <= 2u;
}

OCB_CORE_RAMFUNC bool ocb_pubstate_read(ocb_pubstate *ps, ocb_state *out)
{
    for (uint32_t t = 0u; t < OCB_PUBSTATE_TRIES; t++) {
        ocb_state c;
        const uint32_t g0 = ocb_pubstate_try_begin(ps);
        ocb_pubstate_try_copy(ps, g0, &c, 0);
        ocb_pubstate_try_copy(ps, g0, &c, 1);
        if (ocb_pubstate_try_end(ps, g0)) {
            ocb_pubstate_note_read(ps, t + 1u, true);
            *out = c;
            return true;
        }
    }
    ocb_pubstate_note_read(ps, OCB_PUBSTATE_TRIES, false);
    *out = OCB_NEUTRAL;
    return false;
}

OCB_CORE_RAMFUNC void ocb_pubstate_note_read(ocb_pubstate *ps, uint32_t tries, bool ok)
{
    if (tries > 1u || !ok) {
        ps->retries = sat_inc(ps->retries);
    }
    if (!ok) {
        ps->fallbacks = sat_inc(ps->fallbacks);
    }
}

uint32_t ocb_pubstate_retries(const ocb_pubstate *ps)
{
    return ps->retries;
}

uint32_t ocb_pubstate_fallbacks(const ocb_pubstate *ps)
{
    return ps->fallbacks;
}
