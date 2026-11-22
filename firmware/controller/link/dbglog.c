/*
 * SPDX-License-Identifier: MIT
 *
 * Debug log ring and STATUS window (story 2.3 D14).
 */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include "ocb_link/config.h"
#include "ocb_link/dbglog.h"

typedef char ocb_dbglog_cap_pow2[((OCB_DBGLOG_CAP & (OCB_DBGLOG_CAP - 1u)) == 0u) ? 1 : -1];

void ocb_dbglog_ring_init(ocb_dbglog_ring *r)
{
    for (size_t i = 0; i < OCB_DBGLOG_CAP; i++) {
        r->buf[i] = 0u;
    }
    r->head = 0u;
    r->tail = 0u;
    r->drops = 0u;
}

void ocb_dbglog_window_init(ocb_dbglog_window *w)
{
    memset(w, 0, sizeof *w);
}

bool ocb_dbglog_push(ocb_dbglog_ring *r, uint8_t kind, uint8_t id)
{
    const uint32_t head = r->head;
    if (head - r->tail >= OCB_DBGLOG_CAP) {
        r->drops = r->drops + 1u;
        return false;
    }
    r->buf[head & (OCB_DBGLOG_CAP - 1u)] = (uint16_t)(((uint16_t)kind << 8) | id);
    OCB_LINK_BARRIER(); /* the slot before the index (as the RX ring) */
    r->head = head + 1u;
    return true;
}

bool ocb_dbglog_pop(ocb_dbglog_ring *r, uint16_t *rec)
{
    const uint32_t tail = r->tail;
    if (r->head == tail) {
        return false;
    }
    *rec = r->buf[tail & (OCB_DBGLOG_CAP - 1u)];
    OCB_LINK_BARRIER();
    r->tail = tail + 1u;
    return true;
}

void ocb_dbglog_window_add(ocb_dbglog_window *w, uint16_t rec)
{
    if (w->n < OCB_DBGLOG_WINDOW) {
        w->rec[(uint8_t)((w->start + w->n) % OCB_DBGLOG_WINDOW)] = rec;
        w->n++;
    } else {
        w->rec[w->start] = rec;
        w->start = (uint8_t)((w->start + 1u) % OCB_DBGLOG_WINDOW);
    }
    w->total = (uint16_t)(w->total + 1u);
}

uint32_t ocb_dbglog_drain(ocb_dbglog_window *w, ocb_dbglog_ring *r)
{
    uint32_t added = 0u;
    uint16_t rec;
    /* Bounded: at most one ring's worth per call. */
    for (uint32_t i = 0u; i < OCB_DBGLOG_CAP && ocb_dbglog_pop(r, &rec); i++) {
        ocb_dbglog_window_add(w, rec);
        added++;
    }
    /* Records dropped by a full ring came after the ones it held. */
    const uint32_t drops = r->drops;
    if (drops != w->seen_drops) {
        const uint32_t lost = drops - w->seen_drops;
        w->seen_drops = drops;
        const uint8_t id = lost > 0xFFu ? (uint8_t)0xFFu : (uint8_t)lost;
        ocb_dbglog_window_add(w, (uint16_t)(((uint16_t)OCB_DBGLOG_OVERFLOW << 8) | id));
        added++;
    }
    return added;
}
