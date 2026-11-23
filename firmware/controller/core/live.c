/*
 * SPDX-License-Identifier: MIT
 *
 * Liveness rules of the two controller cores (AD-11, story 2.5 D6, D7).
 * O(1), no division, no 64-bit division.
 */
#include <stdbool.h>
#include <stdint.h>

#include "ocb_core/live.h"

void ocb_live_peer_init(ocb_live_peer *p, uint32_t counter, uint64_t now_us)
{
    p->last_counter = counter;
    p->last_change_us = now_us;
    p->moved = false;
}

bool ocb_live_peer_stale(ocb_live_peer *p, uint32_t counter, uint64_t now_us)
{
    if (counter != p->last_counter) {
        /* Any change is progress, a wrap included. */
        p->last_counter = counter;
        p->last_change_us = now_us;
        p->moved = true;
        return false;
    }
    if (now_us <= p->last_change_us) {
        return false;
    }
    return now_us - p->last_change_us > (uint64_t)OCB_LIVE_STALE_US;
}

bool ocb_live_peer_moved(const ocb_live_peer *p)
{
    return p->moved;
}

bool ocb_live_core0_may_feed(bool core1_stale)
{
    return !core1_stale;
}

bool ocb_live_core1_must_reboot(bool core0_stale)
{
    return core0_stale;
}

uint8_t ocb_live_core1_cause(const ocb_live_peer *core1)
{
    return core1->moved ? (uint8_t)OCB_LIVE_CAUSE_CORE1_STALE : (uint8_t)OCB_LIVE_CAUSE_BOOT_NOT_ALIVE;
}

uint32_t ocb_live_scratch_encode(uint8_t value)
{
    return ((uint32_t)OCB_LIVE_SCRATCH_MAGIC << 16) | (uint32_t)value;
}

uint8_t ocb_live_scratch_decode(uint32_t word)
{
    if ((word >> 16) != (uint32_t)OCB_LIVE_SCRATCH_MAGIC || (word & 0xFF00u) != 0u) {
        return 0u;
    }
    return (uint8_t)(word & 0xFFu);
}

uint32_t ocb_live_cause_encode(uint8_t cause)
{
    return ocb_live_scratch_encode(cause > (uint8_t)OCB_LIVE_CAUSE_MAX ? (uint8_t)OCB_LIVE_CAUSE_NONE : cause);
}

uint8_t ocb_live_cause_decode(uint32_t word)
{
    const uint8_t c = ocb_live_scratch_decode(word);
    return c > (uint8_t)OCB_LIVE_CAUSE_MAX ? (uint8_t)OCB_LIVE_CAUSE_NONE : c;
}

uint32_t ocb_live_span_encode(uint32_t span_us)
{
    const uint32_t units = span_us / OCB_LIVE_SPAN_UNIT_US;
    return ((uint32_t)OCB_LIVE_SCRATCH_MAGIC << 16) | (units > 0xFFFFu ? 0xFFFFu : units);
}

uint32_t ocb_live_span_decode(uint32_t word)
{
    if ((word >> 16) != (uint32_t)OCB_LIVE_SCRATCH_MAGIC) {
        return 0u;
    }
    return (word & 0xFFFFu) * OCB_LIVE_SPAN_UNIT_US;
}

uint64_t ocb_live_peer_since_us(const ocb_live_peer *p, uint64_t now_us)
{
    return now_us > p->last_change_us ? now_us - p->last_change_us : 0u;
}
