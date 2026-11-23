/*
 * SPDX-License-Identifier: MIT
 *
 * Hang injectors of the debug hang flavours (story 2.5 D9, D18, Task 9.4),
 * compiled only with the CMake option OCB_DEBUG_HANG=CORE0 or CORE1 (which
 * needs OCB_DEBUG_BUILD and OCB_DEBUG_TIMELINE). The chosen core hangs with
 * a note held, so AC4's "no input stays pressed" is meaningful:
 *  - debug_hang_note(state) is called by the hanging core with each state
 *    it handles (core 1: the published state after each pass; core 0: the
 *    state the report pump read); the K-th state with a note button down
 *    (K = 1, the melody's first note) arms the injector;
 *  - debug_hang_poll() is called once per pass by the same core; once
 *    OCB_DEBUG_HANG_DELAY_US have passed since the arming (a non-blocking
 *    wait: the loop keeps running, so the liveness detector does not see
 *    the wait as a stall, D23) it spins forever.
 * A release ELF carries none of it (check-release-nolog.sh).
 */
#ifndef OCB_CTRL_DEBUG_HANG_H
#define OCB_CTRL_DEBUG_HANG_H

#include "ocb_proto/state.h"

#if !defined(OCB_DEBUG_BUILD) || !(defined(OCB_DEBUG_HANG_CORE0) || defined(OCB_DEBUG_HANG_CORE1))
#error "debug_hang.h: only in a debug build with OCB_DEBUG_HANG=CORE0 or CORE1 (story 2.5 D9)"
#endif

void debug_hang_note(const ocb_state *st);
void debug_hang_poll(void);

#endif /* OCB_CTRL_DEBUG_HANG_H */
