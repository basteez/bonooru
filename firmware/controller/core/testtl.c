/*
 * SPDX-License-Identifier: MIT
 *
 * The built-in test timeline loader (story 2.5 D8, D10). Rules in
 * ocb_core/testtl.h.
 */
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include "ocb_core/core.h"
#include "ocb_core/internal.h"
#include "ocb_core/testtl.h"
#include "ocb_proto/consts.h"
#include "ocb_proto/frame.h"
#include "ocb_proto/msgs.h"

size_t ocb_testtl_count(void)
{
    return ocb_testtl_table_len;
}

const ocb_testtl_event *ocb_testtl_get(size_t i)
{
    return i < ocb_testtl_table_len ? &ocb_testtl_table[i] : NULL;
}

int ocb_testtl_load_table(ocb_core *core, const ocb_core_env *env, const ocb_testtl_event *table, size_t n)
{
    if (core->mode != OCB_MODE_IDLE || core->aborted || ocb_queue_count(&core->queue) != 0u) {
        return OCB_TESTTL_ERR_STATE;
    }
    const uint8_t saved = core->expected;
    ocb_frame f;
    for (size_t i = 0; i < n; i++) {
        ocb_msg_queue_event m;
        m.t_us = table[i].t_us;
        m.state = table[i].state;
        const int len = ocb_msg_queue_event_encode(&m, f.body, sizeof f.body);
        f.type = OCB_MSG_QUEUE_EVENT;
        f.seq = core->expected;
        f.len = len > 0 ? (uint8_t)len : 0u;
        ocb_core_out out;
        ocb_core_frame(core, env, &f, &out);
        if (out.kind != OCB_CORE_REPLY_ACK) {
            /* Leave nothing half-loaded: back to an empty IDLE queue. */
            ocb_core_enter_idle_(core);
            ocb_core_advance_expected(core, (uint8_t)(saved - 1u));
            return out.kind == OCB_CORE_REPLY_NAK ? (int)out.nak_reason : OCB_TESTTL_ERR_STATE;
        }
    }
    ocb_core_advance_expected(core, (uint8_t)(saved - 1u)); /* expected = saved */
    return 0;
}

int ocb_testtl_load(ocb_core *core, const ocb_core_env *env)
{
    return ocb_testtl_load_table(core, env, ocb_testtl_table, ocb_testtl_table_len);
}
