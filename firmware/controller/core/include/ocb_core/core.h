/*
 * SPDX-License-Identifier: MIT
 *
 * ocb_core: the playback logic of the controller board (AD-8, AD-9), as a
 * pure C99 state machine (story 2.4). It owns the mode (IDLE, DIRECT,
 * PLAYING), the event queue, the abort latch, the go-back-N `expected`
 * counter at the sync points, the ordering baseline and the published
 * state. It owns no clock, no I/O and no allocation:
 *
 *  - time and console readiness come in with every call (ocb_core_env);
 *  - frames come in already decoded and sequence-checked (ocb_core_frame);
 *  - aborts detected outside the core come in through ocb_core_abort;
 *  - the scheduler is ocb_core_tick, called by whoever owns the timer;
 *  - the result of each call is the reply / event in ocb_core_out and the
 *    state in core->published (copied out by the caller after each call).
 *
 * Rules (precedence, validation, abort causes, scheduler) are documented in
 * docs/link-protocol.md, section "Controller modes (AD-8)".
 *
 * Every pointer argument must be non-NULL. Single-threaded by design: one
 * caller owns an instance. Instances are fully independent.
 */
#ifndef OCB_CORE_CORE_H
#define OCB_CORE_CORE_H

#include <stdbool.h>
#include <stdint.h>

#include "ocb_core/mask.h"
#include "ocb_core/queue.h"
#include "ocb_core/ramfunc.h"
#include "ocb_proto/consts.h"
#include "ocb_proto/frame.h"
#include "ocb_proto/state.h"

/* LATE threshold when the report period is unknown (0): the 8 ms poll
 * interval commonly seen for the Pro Controller on Switch 1
 * (docs/usb-controller-emulation.md), a hypothesis until the IN interval
 * is measured (story 2.4 D6). */
#define OCB_CORE_LATE_FALLBACK_US 8000u

/* Inputs of every call (D1). now_us is a monotonic 1 MHz count (the RP2040
 * 64-bit timer); ready is the HID profile's ready() at the time of the call;
 * link_ok is the bridge link state decided by the link watchdog (story 2.7
 * D11): false makes START and SET_STATE get NAK NO_LINK. Positive polarity
 * on purpose: a zero-initialised env means "no link", the safe default. */
typedef struct {
    uint64_t now_us;
    bool ready;
    bool link_ok;
} ocb_core_env;

typedef enum {
    OCB_CORE_REPLY_NONE = 0, /* nothing to send (non-app frame type) */
    OCB_CORE_REPLY_ACK,      /* ACK(frame seq) */
    OCB_CORE_REPLY_NAK,      /* NAK(nak_reason, ...) */
    OCB_CORE_REPLY_HELLO     /* the frame was HELLO: the link layer answers INFO */
} ocb_core_reply_kind;

/* Result of a call. event is an OCB_EVT_* to send as EVENT, or OCB_EVT_NONE. */
typedef struct {
    ocb_core_reply_kind kind;
    uint8_t nak_reason; /* OCB_NAK_*, valid when kind == OCB_CORE_REPLY_NAK */
    uint8_t event;
} ocb_core_out;

/* Unrequested transitions to IDLE (AD-8, story 2.4 D5). The first six are
 * detected outside the core (USB, link watchdog, stall detector) and passed
 * to ocb_core_abort; the last three are the scheduler's own. */
typedef enum {
    OCB_ABORT_LINK_LOST = 0,  /* EVENT LINK_LOST */
    OCB_ABORT_APP_TIMEOUT,    /* no EVENT */
    OCB_ABORT_USB_SUSPEND,    /* EVENT USB_SUSPENDED */
    OCB_ABORT_USB_UNMOUNT,    /* EVENT USB_SUSPENDED */
    OCB_ABORT_USB_BUS_RESET,  /* EVENT USB_SUSPENDED */
    OCB_ABORT_CORE_STALL,     /* no EVENT */
    OCB_ABORT_END,            /* END applied: EVENT PLAYBACK_DONE */
    OCB_ABORT_UNDERRUN,       /* EVENT UNDERRUN */
    OCB_ABORT_LATE,           /* EVENT LATE */
    OCB_ABORT_CAUSE_COUNT
} ocb_core_abort_cause;

typedef struct {
    uint8_t mode;        /* OCB_MODE_* */
    bool aborted;        /* the abort latch */
    uint8_t abort_count; /* latch set count, wraps, never reset */
    uint8_t last_event;  /* last EVENT emitted, OCB_EVT_NONE before the first */
    uint8_t expected;    /* next app seq the link layer accepts (D2) */

    ocb_state published;    /* the state the HID profile must report */
    uint32_t publish_count; /* incremented at every publish, wraps */

    uint64_t t0_us;    /* PLAYING: time origin of the events, now + lead */
    uint32_t last_t_us; /* ordering baseline: last accepted t_us */
    bool have_last_t;   /* false after entering IDLE: any first t_us is fine */
    bool end_queued;    /* an END event is queued: nothing may follow it */
    uint32_t late_us;   /* LATE threshold, the report period or the fallback */

    ocb_queue queue;
} ocb_core;

/* Snapshot for STATUS (AD-19, D13); flags (link_ok, console_ready) are not
 * core state and are added by the link layer. */
typedef struct {
    uint16_t free;
    uint32_t now_us; /* us since t0 (wraps after about 71 minutes); 0 unless PLAYING after the lead */
    uint8_t last_seq; /* expected - 1 */
    uint8_t mode;
    uint8_t abort_count;
    uint8_t last_event;
} ocb_core_status_view;

/* Initial state: IDLE, latch clear, abort_count 0, last_event NONE,
 * expected 0, NEUTRAL published, empty queue over storage[cap]. Returns 0,
 * or -1 when storage is NULL or cap is 0 (the core is then initialised with
 * a zero-capacity queue: every QUEUE_EVENT gets NAK FULL). */
int ocb_core_init(ocb_core *core, ocb_core_event *storage, uint16_t cap, uint16_t report_period_us);

/* LATE threshold = one report period (INFO.report_period_us); 0 selects
 * OCB_CORE_LATE_FALLBACK_US. */
void ocb_core_set_report_period(ocb_core *core, uint16_t report_period_us);

/* Handles one app frame that the link layer accepted (seq == expected, or a
 * sync point HELLO / STOP). Sets *out to the reply. HELLO and STOP are
 * never refused; types outside 0x01..0x3F give OCB_CORE_REPLY_NONE and
 * change nothing. HELLO's proto_ver is not read here: every HELLO resets
 * the core, so the link layer checks the version (AD-7) before calling. */
OCB_CORE_RAMFUNC void ocb_core_frame(ocb_core *core, const ocb_core_env *env, const ocb_frame *frame,
                                     ocb_core_out *out);

/* Scheduler pass: applies at most one due event (D6). out->kind is always
 * NONE; out->event is the EVENT to send, if any. */
OCB_CORE_RAMFUNC void ocb_core_tick(ocb_core *core, const ocb_core_env *env, ocb_core_out *out);

/* Unrequested transition to IDLE, in any mode (D5): publishes NEUTRAL,
 * clears the queue, sets the latch. A cause arriving while the latch is
 * already set changes nothing and returns no event. A cause value outside
 * the enum is handled like OCB_ABORT_CORE_STALL (latched, no event). */
OCB_CORE_RAMFUNC void ocb_core_abort(ocb_core *core, ocb_core_abort_cause cause, ocb_core_out *out);

void ocb_core_status(const ocb_core *core, uint64_t now_us, ocb_core_status_view *v);

/* Sets expected = seq + 1 and changes nothing else (story 2.6 D5). For the
 * link layer only, after it has refused a frame itself with a semantic NAK
 * (NAK VERSION before the session is open): the core owns `expected` but
 * never saw that frame. */
OCB_CORE_RAMFUNC void ocb_core_advance_expected(ocb_core *core, uint8_t seq);

#endif /* OCB_CORE_CORE_H */
