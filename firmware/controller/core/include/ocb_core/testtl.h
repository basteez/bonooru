/*
 * SPDX-License-Identifier: MIT
 *
 * ocb_testtl: the built-in test timeline of the debug timeline flavour
 * (story 2.5 D8, D10). Pure C99. The table (testtl_data.c) is generated
 * from the Story 1.6 trial file by firmware/tools/gen_test_timeline.py;
 * the loader feeds it to the real core through the real validation path:
 * one QUEUE_EVENT frame per event, built with the ocb_proto encoder and
 * handed to ocb_core_frame with seq = core->expected, so ordering, the END
 * rule, the flags and the queue capacity are checked by the code the app
 * uses; then `expected` is restored, so the host's next frame (START) has
 * the seq it expects. No wire change: the host sends only HELLO and START.
 *
 * Always built and tested on the host; linked into the controller only
 * when the CMake option OCB_DEBUG_TIMELINE is ON (a release ELF carries
 * none of it, story 2.5 Task 8.3).
 */
#ifndef OCB_CORE_TESTTL_H
#define OCB_CORE_TESTTL_H

#include <stddef.h>
#include <stdint.h>

#include "ocb_core/core.h"
#include "ocb_core/queue.h"

/* One event: applied at t0 + t_us (the queue's own event type). */
typedef ocb_core_event ocb_testtl_event;

/* The generated table (testtl_data.c). */
extern const ocb_testtl_event ocb_testtl_table[];
extern const size_t ocb_testtl_table_len;

/* The precondition of a load was not met (not IDLE, latched, or the queue
 * is not empty): nothing was changed. */
#define OCB_TESTTL_ERR_STATE (-1)

size_t ocb_testtl_count(void);
/* Event i, or NULL when i >= ocb_testtl_count(). */
const ocb_testtl_event *ocb_testtl_get(size_t i);

/* Loads the built-in table. Returns 0, OCB_TESTTL_ERR_STATE, or the first
 * NAK reason (OCB_NAK_*) the core gave; on a NAK the queue is emptied
 * again (IDLE, NEUTRAL) and `expected` is restored. Call only while the
 * core is IDLE, not latched, with an empty queue. */
int ocb_testtl_load(ocb_core *core, const ocb_core_env *env);

/* The same for any table (tests: a negative control with a bad row). */
int ocb_testtl_load_table(ocb_core *core, const ocb_core_env *env, const ocb_testtl_event *table, size_t n);

#endif /* OCB_CORE_TESTTL_H */
