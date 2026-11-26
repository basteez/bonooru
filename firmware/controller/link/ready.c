/*
 * SPDX-License-Identifier: MIT
 *
 * Console readiness predicate and USB mailbox (AD-19, story 2.6 D11).
 */
#include <stdbool.h>
#include <stdint.h>

#include "ocb_link/ready.h"
#include "ocb_proto/consts.h"

#define STALL_US ((uint64_t)OCB_CONSOLE_POLL_STALL_MS * 1000u)

void ocb_usb_mailbox_init(ocb_usb_mailbox *m)
{
    m->handshakes = 0u;
    m->in_reports = 0u;
    m->suspends = 0u;
    m->unmounts = 0u;
    m->bus_resets = 0u;
    m->report_period_us = 0u;
    m->in_min_us = 0u;
    m->in_p95_us = 0u;
    m->in_max_us = 0u;
    m->in_slow = 0u;
    m->core0_pass_max_us = 0u;
    m->seq_retries = 0u;
    m->seq_fallbacks = 0u;
    m->p2_queue_full = 0u;
}

/* Single writer per counter: a plain load, add and store (no RMW atomics
 * on Cortex-M0+, none needed). */
void ocb_usb_mailbox_handshake_done(ocb_usb_mailbox *m)
{
    m->handshakes = m->handshakes + 1u;
}

void ocb_usb_mailbox_in_report(ocb_usb_mailbox *m)
{
    m->in_reports = m->in_reports + 1u;
}

void ocb_usb_mailbox_suspend(ocb_usb_mailbox *m)
{
    m->suspends = m->suspends + 1u;
}

void ocb_usb_mailbox_unmount(ocb_usb_mailbox *m)
{
    m->unmounts = m->unmounts + 1u;
}

void ocb_usb_mailbox_bus_reset(ocb_usb_mailbox *m)
{
    m->bus_resets = m->bus_resets + 1u;
}

void ocb_usb_mailbox_set_report_period(ocb_usb_mailbox *m, uint32_t report_period_us)
{
    m->report_period_us = report_period_us;
}

void ocb_usb_mailbox_set_in_spread(ocb_usb_mailbox *m, uint32_t min_us, uint32_t p95_us, uint32_t max_us,
                                   uint32_t slow)
{
    m->in_min_us = min_us;
    m->in_p95_us = p95_us;
    m->in_max_us = max_us;
    m->in_slow = slow;
}

void ocb_usb_mailbox_set_core0(ocb_usb_mailbox *m, uint32_t pass_max_us, uint32_t seq_retries,
                               uint32_t seq_fallbacks)
{
    m->core0_pass_max_us = pass_max_us;
    m->seq_retries = seq_retries;
    m->seq_fallbacks = seq_fallbacks;
}

void ocb_usb_mailbox_set_p2_queue_full(ocb_usb_mailbox *m, uint32_t n)
{
    m->p2_queue_full = n;
}

void ocb_ready_init(ocb_ready *r, const ocb_usb_mailbox *m, uint64_t now_us)
{
    r->seen_handshakes = m->handshakes;
    r->seen_in_reports = m->in_reports;
    r->seen_suspends = m->suspends;
    r->seen_unmounts = m->unmounts;
    r->seen_bus_resets = m->bus_resets;
    r->handshake = false;
    r->last_in_change_us = now_us;
}

OCB_CORE_RAMFUNC uint8_t ocb_ready_poll(ocb_ready *r, const ocb_usb_mailbox *m, uint64_t now_us)
{
    /* One load per counter. */
    const uint32_t hs = m->handshakes;
    const uint32_t in = m->in_reports;
    const uint32_t su = m->suspends;
    const uint32_t um = m->unmounts;
    const uint32_t br = m->bus_resets;
    uint8_t down = 0u;

    if (su != r->seen_suspends) {
        r->seen_suspends = su;
        down |= OCB_READY_DOWN_SUSPEND;
    }
    if (um != r->seen_unmounts) {
        r->seen_unmounts = um;
        down |= OCB_READY_DOWN_UNMOUNT;
    }
    if (br != r->seen_bus_resets) {
        r->seen_bus_resets = br;
        down |= OCB_READY_DOWN_BUS_RESET;
    }
    if (down != 0u) {
        r->handshake = false;
    }
    /* Down first, then a new handshake (D11). */
    if (hs != r->seen_handshakes) {
        r->seen_handshakes = hs;
        r->handshake = true;
        r->last_in_change_us = now_us; /* grace period */
    }
    if (in != r->seen_in_reports) {
        r->seen_in_reports = in;
        r->last_in_change_us = now_us;
    }
    return down;
}

OCB_CORE_RAMFUNC bool ocb_ready_is(const ocb_ready *r, uint64_t now_us)
{
    if (!r->handshake) {
        return false;
    }
    if (now_us < r->last_in_change_us) {
        return true; /* the clock never goes back; treat as no gap */
    }
    return now_us - r->last_in_change_us <= STALL_US;
}
