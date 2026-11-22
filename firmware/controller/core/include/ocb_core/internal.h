/*
 * SPDX-License-Identifier: MIT
 *
 * Internal helpers shared by modes.c and sched.c. Not part of the API:
 * callers outside controller/core use core.h only.
 */
#ifndef OCB_CORE_INTERNAL_H
#define OCB_CORE_INTERNAL_H

#include "ocb_core/core.h"

/* The only writer of core->published; increments publish_count. */
OCB_CORE_RAMFUNC void ocb_core_publish_(ocb_core *core, const ocb_state *s);

/* The only way to enter IDLE: publishes NEUTRAL, clears the queue, resets
 * the ordering baseline and the END marker, sets mode IDLE. */
OCB_CORE_RAMFUNC void ocb_core_enter_idle_(ocb_core *core);

#endif /* OCB_CORE_INTERNAL_H */
