/*
 * SPDX-License-Identifier: MIT
 *
 * Modes, frame handling and the abort latch of the controller core (AD-8,
 * story 2.4 D2 to D5, D14). Frame bodies are read only through the
 * ocb_proto message decoders.
 */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "ocb_core/core.h"
#include "ocb_core/internal.h"
#include "ocb_proto/consts.h"
#include "ocb_proto/err.h"
#include "ocb_proto/msgs.h"
#include "ocb_proto/state.h"

int ocb_core_init(ocb_core *core, ocb_core_event *storage, uint16_t cap, uint16_t report_period_us)
{
    core->mode = OCB_MODE_IDLE;
    core->aborted = false;
    core->abort_count = 0u;
    core->last_event = OCB_EVT_NONE;
    core->expected = 0u;
    core->published = OCB_NEUTRAL;
    core->publish_count = 0u;
    core->t0_us = 0u;
    core->last_t_us = 0u;
    core->have_last_t = false;
    core->end_queued = false;
    ocb_core_timing_reset(&core->timing);
    ocb_core_set_report_period(core, report_period_us);
    return ocb_queue_init(&core->queue, storage, cap);
}

void ocb_core_set_report_period(ocb_core *core, uint16_t report_period_us)
{
    core->late_us = report_period_us == 0u ? OCB_CORE_LATE_FALLBACK_US : (uint32_t)report_period_us;
}

OCB_CORE_RAMFUNC void ocb_core_publish_(ocb_core *core, const ocb_state *s)
{
    core->published = *s;
    core->publish_count++;
}

OCB_CORE_RAMFUNC void ocb_core_enter_idle_(ocb_core *core)
{
    ocb_core_publish_(core, &OCB_NEUTRAL);
    ocb_queue_clear(&core->queue);
    core->have_last_t = false;
    core->last_t_us = 0u;
    core->end_queued = false;
    core->mode = OCB_MODE_IDLE;
}

static OCB_CORE_RAMFUNC void out_reset(ocb_core_out *out)
{
    out->kind = OCB_CORE_REPLY_NONE;
    out->nak_reason = 0u;
    out->event = OCB_EVT_NONE;
}

/* EVENT per cause (D5); OCB_EVT_NONE = latched without an EVENT. */
static OCB_CORE_RAMFUNC uint8_t cause_event(ocb_core_abort_cause cause)
{
    switch (cause) {
    case OCB_ABORT_LINK_LOST:
        return OCB_EVT_LINK_LOST;
    case OCB_ABORT_USB_SUSPEND:
    case OCB_ABORT_USB_UNMOUNT:
    case OCB_ABORT_USB_BUS_RESET:
        return OCB_EVT_USB_SUSPENDED;
    case OCB_ABORT_END:
        return OCB_EVT_PLAYBACK_DONE;
    case OCB_ABORT_UNDERRUN:
        return OCB_EVT_UNDERRUN;
    case OCB_ABORT_LATE:
        return OCB_EVT_LATE;
    case OCB_ABORT_APP_TIMEOUT:
    case OCB_ABORT_CORE_STALL:
    case OCB_ABORT_CAUSE_COUNT:
    default:
        return OCB_EVT_NONE;
    }
}

OCB_CORE_RAMFUNC void ocb_core_abort(ocb_core *core, ocb_core_abort_cause cause, ocb_core_out *out)
{
    out_reset(out);
    if (core->aborted) {
        /* Already latched, hence already IDLE, neutral and empty (D5 rule 3). */
        return;
    }
    ocb_core_enter_idle_(core);
    core->aborted = true;
    core->abort_count = (uint8_t)(core->abort_count + 1u);
    const uint8_t ev = cause_event(cause);
    if (ev != OCB_EVT_NONE) {
        core->last_event = ev;
        out->event = ev;
    }
}

static OCB_CORE_RAMFUNC void reply_ack(ocb_core_out *out)
{
    out->kind = OCB_CORE_REPLY_ACK;
}

static OCB_CORE_RAMFUNC void reply_nak(ocb_core_out *out, uint8_t reason)
{
    out->kind = OCB_CORE_REPLY_NAK;
    out->nak_reason = reason;
}

static OCB_CORE_RAMFUNC bool hat_ok(const ocb_state *s)
{
    return s->hat <= OCB_HAT_NEUTRAL;
}

/* HELLO and STOP: sync points, never refused, any body length (D4). */
static OCB_CORE_RAMFUNC void sync_point(ocb_core *core, const ocb_frame *frame)
{
    ocb_core_enter_idle_(core);
    core->aborted = false;
    core->expected = (uint8_t)(frame->seq + 1u);
}

/* NAK precedence for SET_STATE and START (story 2.4 D3, story 2.7 D11),
 * first match wins: ABORTED, NO_LINK, BUSY, NOT_READY, BAD_STATE. */
static OCB_CORE_RAMFUNC void handle_set_state(ocb_core *core, const ocb_core_env *env,
                                              const ocb_frame *frame, ocb_core_out *out)
{
    ocb_msg_set_state m;
    if (core->aborted) {
        reply_nak(out, OCB_NAK_ABORTED);
    } else if (!env->link_ok) {
        reply_nak(out, OCB_NAK_NO_LINK);
    } else if (core->mode == OCB_MODE_PLAYING) {
        reply_nak(out, OCB_NAK_BUSY);
    } else if (!env->ready) {
        reply_nak(out, OCB_NAK_NOT_READY);
    } else if (ocb_msg_set_state_decode(frame->body, frame->len, &m) != OCB_OK || m.state.flags != 0u ||
               !hat_ok(&m.state)) {
        reply_nak(out, OCB_NAK_BAD_STATE);
    } else {
        /* DIRECT is not masked (AD-9). */
        core->mode = OCB_MODE_DIRECT;
        ocb_core_publish_(core, &m.state);
        reply_ack(out);
    }
}

static OCB_CORE_RAMFUNC void handle_start(ocb_core *core, const ocb_core_env *env, const ocb_frame *frame,
                                          ocb_core_out *out)
{
    ocb_msg_start m;
    if (core->aborted) {
        reply_nak(out, OCB_NAK_ABORTED);
    } else if (!env->link_ok) {
        reply_nak(out, OCB_NAK_NO_LINK);
    } else if (core->mode == OCB_MODE_PLAYING) {
        reply_nak(out, OCB_NAK_BUSY);
    } else if (!env->ready) {
        reply_nak(out, OCB_NAK_NOT_READY);
    } else if (ocb_msg_start_decode(frame->body, frame->len, &m) != OCB_OK ||
               ocb_queue_count(&core->queue) == 0u) {
        reply_nak(out, OCB_NAK_BAD_STATE);
    } else {
        /* From DIRECT the real-time state goes NEUTRAL (AD-8). */
        ocb_core_publish_(core, &OCB_NEUTRAL);
        core->t0_us = env->now_us + (uint64_t)m.lead_us;
        core->mode = OCB_MODE_PLAYING;
        ocb_core_timing_reset(&core->timing); /* statistics per run (D11) */
        reply_ack(out);
    }
}

static OCB_CORE_RAMFUNC void handle_queue_event(ocb_core *core, const ocb_frame *frame,
                                                ocb_core_out *out)
{
    ocb_msg_queue_event m;
    if (core->aborted) {
        reply_nak(out, OCB_NAK_ABORTED);
        return;
    }
    if (ocb_msg_queue_event_decode(frame->body, frame->len, &m) != OCB_OK ||
        !ocb_state_reserved_bits_ok(&m.state) || !hat_ok(&m.state) ||
        (ocb_state_has_end(&m.state) && !ocb_state_is_neutral_pose(&m.state)) || core->end_queued) {
        reply_nak(out, OCB_NAK_BAD_STATE);
        return;
    }
    if (m.t_us >= (uint32_t)OCB_T_US_LIMIT || (core->have_last_t && m.t_us <= core->last_t_us)) {
        reply_nak(out, OCB_NAK_ORDER);
        return;
    }
    const ocb_core_event ev = {m.t_us, m.state};
    if (!ocb_queue_push(&core->queue, &ev)) {
        reply_nak(out, OCB_NAK_FULL);
        return;
    }
    core->last_t_us = m.t_us;
    core->have_last_t = true;
    if (ocb_state_has_end(&m.state)) {
        core->end_queued = true;
    }
    reply_ack(out);
}

OCB_CORE_RAMFUNC void ocb_core_frame(ocb_core *core, const ocb_core_env *env, const ocb_frame *frame,
                                     ocb_core_out *out)
{
    out_reset(out);
    switch (frame->type) {
    case OCB_MSG_HELLO:
        sync_point(core, frame);
        out->kind = OCB_CORE_REPLY_HELLO;
        return;
    case OCB_MSG_STOP:
        sync_point(core, frame);
        reply_ack(out);
        return;
    case OCB_MSG_PING:
        if (ocb_msg_ping_decode(frame->body, frame->len) != OCB_OK) {
            reply_nak(out, OCB_NAK_BAD_STATE);
        } else {
            reply_ack(out);
        }
        break;
    case OCB_MSG_SET_STATE:
        handle_set_state(core, env, frame, out);
        break;
    case OCB_MSG_START:
        handle_start(core, env, frame, out);
        break;
    case OCB_MSG_QUEUE_EVENT:
        handle_queue_event(core, frame, out);
        break;
    default:
        if (frame->type < OCB_TYPE_APP_MIN || frame->type > OCB_TYPE_APP_MAX) {
            /* Bridge, controller or invalid type: never reaches the core (D14). */
            return;
        }
        reply_nak(out, OCB_NAK_UNKNOWN_TYPE);
        break;
    }
    /* ACK and semantic NAKs advance expected; the transport NAK FULL does
     * not (AD-6, D2). */
    if (!(out->kind == OCB_CORE_REPLY_NAK && OCB_NAK_IS_TRANSPORT(out->nak_reason))) {
        core->expected = (uint8_t)(frame->seq + 1u);
    }
}

void ocb_core_status(const ocb_core *core, uint64_t now_us, ocb_core_status_view *v)
{
    v->free = ocb_queue_free(&core->queue);
    if (core->mode == OCB_MODE_PLAYING && now_us >= core->t0_us) {
        v->now_us = (uint32_t)(now_us - core->t0_us);
    } else {
        v->now_us = 0u;
    }
    v->last_seq = (uint8_t)(core->expected - 1u);
    v->mode = core->mode;
    v->abort_count = core->abort_count;
    v->last_event = core->last_event;
}

OCB_CORE_RAMFUNC void ocb_core_advance_expected(ocb_core *core, uint8_t seq)
{
    core->expected = (uint8_t)(seq + 1u);
}
