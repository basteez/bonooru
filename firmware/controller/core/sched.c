/*
 * SPDX-License-Identifier: MIT
 *
 * Scheduler pass of the controller core (AD-8, story 2.4 D6): at most one
 * due event per call; LATE is judged before the apply, END and underrun
 * after it. Only 64-bit addition, subtraction and comparison (no 64-bit
 * division on Cortex-M0+).
 */
#include <stddef.h>
#include <stdint.h>

#include "ocb_core/core.h"
#include "ocb_core/internal.h"
#include "ocb_core/mask.h"
#include "ocb_proto/consts.h"
#include "ocb_proto/state.h"

OCB_CORE_RAMFUNC void ocb_core_tick(ocb_core *core, const ocb_core_env *env, ocb_core_out *out)
{
    out->kind = OCB_CORE_REPLY_NONE;
    out->nak_reason = 0u;
    out->event = OCB_EVT_NONE;

    if (core->mode != OCB_MODE_PLAYING || env->now_us < core->t0_us) {
        return; /* not playing, or still in the lead */
    }
    const ocb_core_event *head = ocb_queue_peek(&core->queue);
    if (head == NULL) {
        /* Unreachable (START needs a queued event, an empty queue after an
         * apply is an underrun); fail safe anyway. */
        ocb_core_abort(core, OCB_ABORT_UNDERRUN, out);
        return;
    }
    const uint64_t due = core->t0_us + (uint64_t)head->t_us;
    if (env->now_us < due) {
        return;
    }
    if (env->now_us - due > (uint64_t)core->late_us) {
        /* Overdue by more than one report period: never fire a backlog. */
        ocb_core_abort(core, OCB_ABORT_LATE, out);
        return;
    }

    /* Story 2.5 D11: lateness at apply, <= late_us here (fits 32 bits). */
    ocb_core_timing_note(&core->timing, (uint32_t)(env->now_us - due));

    ocb_core_event ev;
    (void)ocb_queue_pop(&core->queue, &ev);
    const ocb_state masked = ocb_core_mask_state(&ev.state);
    ocb_core_publish_(core, &masked);

    if (ocb_state_has_end(&ev.state)) {
        ocb_core_abort(core, OCB_ABORT_END, out);
    } else if (ocb_queue_count(&core->queue) == 0u) {
        /* Underrun judged at apply time: NEUTRAL in the same pass. */
        ocb_core_abort(core, OCB_ABORT_UNDERRUN, out);
    }
}
