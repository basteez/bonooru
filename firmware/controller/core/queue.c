/*
 * SPDX-License-Identifier: MIT
 *
 * Fixed-capacity FIFO ring (story 2.4 Task 2). Indices wrap by compare and
 * reset, never by a remainder operation.
 */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "ocb_core/queue.h"

int ocb_queue_init(ocb_queue *q, ocb_core_event *storage, uint16_t cap)
{
    q->head = 0u;
    q->count = 0u;
    if (storage == NULL || cap == 0u) {
        q->buf = NULL;
        q->cap = 0u;
        return -1;
    }
    q->buf = storage;
    q->cap = cap;
    return 0;
}

OCB_CORE_RAMFUNC bool ocb_queue_push(ocb_queue *q, const ocb_core_event *ev)
{
    if (q->count >= q->cap) {
        return false;
    }
    uint32_t idx = (uint32_t)q->head + (uint32_t)q->count;
    if (idx >= q->cap) {
        idx -= q->cap;
    }
    q->buf[idx] = *ev;
    q->count = (uint16_t)(q->count + 1u);
    return true;
}

OCB_CORE_RAMFUNC const ocb_core_event *ocb_queue_peek(const ocb_queue *q)
{
    if (q->count == 0u) {
        return NULL;
    }
    return &q->buf[q->head];
}

OCB_CORE_RAMFUNC bool ocb_queue_pop(ocb_queue *q, ocb_core_event *out)
{
    if (q->count == 0u) {
        return false;
    }
    if (out != NULL) {
        *out = q->buf[q->head];
    }
    uint32_t next = (uint32_t)q->head + 1u;
    if (next >= q->cap) {
        next = 0u;
    }
    q->head = (uint16_t)next;
    q->count = (uint16_t)(q->count - 1u);
    return true;
}

OCB_CORE_RAMFUNC void ocb_queue_clear(ocb_queue *q)
{
    q->head = 0u;
    q->count = 0u;
}

OCB_CORE_RAMFUNC uint16_t ocb_queue_count(const ocb_queue *q)
{
    return q->count;
}

OCB_CORE_RAMFUNC uint16_t ocb_queue_free(const ocb_queue *q)
{
    return (uint16_t)(q->cap - q->count);
}
