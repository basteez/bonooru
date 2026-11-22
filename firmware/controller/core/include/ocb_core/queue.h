/*
 * SPDX-License-Identifier: MIT
 *
 * Event queue of the controller core (AD-8, story 2.4 Task 2): a
 * fixed-capacity FIFO ring over storage owned by the caller. No allocation,
 * no global state; any number of queues can coexist.
 *
 * Every pointer argument must be non-NULL unless stated otherwise. The
 * capacity is a u16 because INFO.queue_cap and STATUS.free are u16 on the
 * wire; the firmware default is OCB_QUEUE_CAP_DEFAULT (512 events of
 * 12 bytes in memory, owned by whoever instantiates the core).
 */
#ifndef OCB_CORE_QUEUE_H
#define OCB_CORE_QUEUE_H

#include <stdbool.h>
#include <stdint.h>

#include "ocb_core/ramfunc.h"
#include "ocb_proto/state.h"

/* One scheduled state: applied at t0 + t_us (AD-8). */
typedef struct {
    uint32_t t_us;
    ocb_state state;
} ocb_core_event;

typedef struct {
    ocb_core_event *buf;
    uint16_t cap;
    uint16_t head;  /* index of the oldest event */
    uint16_t count; /* number of queued events, 0..cap */
} ocb_queue;

/* Returns 0, or -1 when storage is NULL or cap is 0 (the queue is then left
 * empty with capacity 0, so every push fails). */
int ocb_queue_init(ocb_queue *q, ocb_core_event *storage, uint16_t cap);

/* Appends a copy of *ev. Returns false when full (nothing is written). */
OCB_CORE_RAMFUNC bool ocb_queue_push(ocb_queue *q, const ocb_core_event *ev);

/* The oldest event, or NULL when empty. Valid until the next push, pop or
 * clear on the same queue. */
OCB_CORE_RAMFUNC const ocb_core_event *ocb_queue_peek(const ocb_queue *q);

/* Removes the oldest event and copies it to *out (out may be NULL to drop
 * it). Returns false when empty. */
OCB_CORE_RAMFUNC bool ocb_queue_pop(ocb_queue *q, ocb_core_event *out);

OCB_CORE_RAMFUNC void ocb_queue_clear(ocb_queue *q);
OCB_CORE_RAMFUNC uint16_t ocb_queue_count(const ocb_queue *q);
/* cap - count: the STATUS.free value. */
OCB_CORE_RAMFUNC uint16_t ocb_queue_free(const ocb_queue *q);

#endif /* OCB_CORE_QUEUE_H */
