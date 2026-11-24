/*
 * SPDX-License-Identifier: MIT
 *
 * AD-20 probe state machine (Story 3.1). See ocb_bridge/probe.h.
 */
#include "ocb_bridge/probe.h"

#include <string.h>

#include "ocb_bridge/config.h"

void ocb_bridge_probe_init(ocb_bridge_probe *p)
{
    memset(p, 0, sizeof *p);
    p->state = OCB_BRIDGE_PROBING;
}

void ocb_bridge_probe_break(ocb_bridge_probe *p)
{
    p->break_seen = true;
}

void ocb_bridge_probe_frame(ocb_bridge_probe *p)
{
    p->frame_seen = true;
}

bool ocb_bridge_probe_linked(const ocb_bridge_probe *p)
{
    return p->state == OCB_BRIDGE_LINKED;
}

static void enter_probing(ocb_bridge_probe *p, uint64_t now_us)
{
    p->state = OCB_BRIDGE_PROBING;
    p->pulsing = false;
    p->next_pulse_us = now_us; /* probe again at once */
    p->high_valid = false;
    p->low_valid = false;
}

static void enter_linked(ocb_bridge_probe *p)
{
    p->state = OCB_BRIDGE_LINKED;
    p->pulsing = false;
    p->low_valid = false;
    p->link_ups++;
}

bool ocb_bridge_probe_step(ocb_bridge_probe *p, uint64_t now_us, bool rx_high)
{
    if (!p->started) {
        p->started = true;
        p->next_pulse_us = now_us;
    }
    const bool brk = p->break_seen;
    const bool frame = p->frame_seen;
    p->break_seen = false;
    p->frame_seen = false;

    if (p->state == OCB_BRIDGE_LINKED) {
        if (brk) {
            p->losses++;
            enter_probing(p, now_us);
        } else if (rx_high) {
            p->low_valid = false;
        } else {
            if (!p->low_valid) {
                p->low_valid = true;
                p->low_since_us = now_us;
            }
            if (now_us - p->low_since_us > OCB_BRIDGE_RX_LOW_US) {
                p->losses++;
                enter_probing(p, now_us);
            }
        }
        if (p->state == OCB_BRIDGE_LINKED) {
            return true;
        }
    }

    /* PROBING. A valid frame, or RX high long enough, shows the controller. */
    if (frame) {
        enter_linked(p);
        return true;
    }
    if (rx_high) {
        if (!p->high_valid) {
            p->high_valid = true;
            p->high_since_us = now_us;
        }
        if (now_us - p->high_since_us >= OCB_BRIDGE_RX_HIGH_US) {
            enter_linked(p);
            return true;
        }
    } else {
        p->high_valid = false;
    }
    if (p->pulsing && now_us >= p->pulse_end_us) {
        p->pulsing = false;
    }
    if (!p->pulsing && now_us >= p->next_pulse_us) {
        p->pulsing = true;
        p->pulses++;
        p->pulse_end_us = now_us + OCB_BRIDGE_PROBE_PULSE_US;
        p->next_pulse_us = now_us + OCB_BRIDGE_PROBE_PERIOD_US;
    }
    return p->pulsing;
}

uint64_t ocb_bridge_probe_next_event_us(const ocb_bridge_probe *p)
{
    if (p->state == OCB_BRIDGE_LINKED) {
        return UINT64_MAX;
    }
    return p->pulsing ? p->pulse_end_us : p->next_pulse_us;
}
