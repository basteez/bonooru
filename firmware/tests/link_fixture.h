/*
 * SPDX-License-Identifier: MIT
 *
 * Shared harness for the controller link host tests (story 2.6). A link,
 * its core (queue up to 512) and a USB mailbox on a fake clock (h->now).
 * Wire frames are built with the real ocb_frame_encode and fed one byte at
 * a time as RX entries; every output frame is checked as a whole
 * controller frame and decoded with ocb_frame_decode(OCB_RX_APP, ...).
 * Plain C, no framework.
 *
 * Story 2.7 (D13): the link starts down (no heartbeat, the boot state).
 * lh_link_up() sends a first HEARTBEAT; from then on lh_advance() moves the
 * clock in slices of at most hb_period_us and injects one HEARTBEAT (DTR
 * per h->dtr) per period while auto_hb is set, like the bridge would, so a
 * test that moves the clock by hundreds of milliseconds does not see a
 * link loss it is not about. lh_session() brings the link up.
 */
#ifndef OCB_LINK_FIXTURE_H
#define OCB_LINK_FIXTURE_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include "ocb_core/ocb_core.h"
#include "ocb_link/ocb_link.h"
#include "ocb_proto/ocb_proto.h"
#include "ocb_test.h"

#define LH_CAP_MAX 512u
#define LH_FW_MAJOR 0u
#define LH_FW_MINOR 2u
#define LH_FW_PATCH 3u
#define LH_APP_TIMEOUT 1000u

typedef struct {
    ocb_core core;
    ocb_core_event storage[LH_CAP_MAX];
    ocb_usb_mailbox mb;
    ocb_link link;
    uint64_t now;
    uint8_t seq; /* seq of the next app frame sent with an lh_* helper */
    /* Heartbeat emulation (story 2.7 D13). */
    bool auto_hb;          /* lh_advance injects heartbeats (default true) */
    bool hb_up;            /* lh_link_up was called */
    bool dtr;              /* DTR bit of the injected heartbeats (default true) */
    uint64_t hb_period_us; /* default OCB_HEARTBEAT_PERIOD_MS */
    uint64_t last_hb_sent;
} lh;

typedef struct {
    uint8_t type;
    uint8_t seq;
    uint8_t len;
    uint8_t body[OCB_MAX_BODY];
} lh_out;

static inline void lh_init_cfg(lh *h, uint16_t cap, bool debug_counters)
{
    memset(h, 0, sizeof *h);
    h->now = 1000000u;
    h->auto_hb = true;
    h->dtr = true;
    h->hb_period_us = (uint64_t)OCB_HEARTBEAT_PERIOD_MS * 1000u;
    const int rc = ocb_core_init(&h->core, h->storage, cap, 0u);
    OCB_CHECK(rc == 0, "ocb_core_init(cap %u) returned %d", (unsigned)cap, rc);
    ocb_usb_mailbox_init(&h->mb);
    ocb_link_cfg cfg;
    cfg.fw_ver[0] = LH_FW_MAJOR;
    cfg.fw_ver[1] = LH_FW_MINOR;
    cfg.fw_ver[2] = LH_FW_PATCH;
    cfg.hid_profile = OCB_HID_PROFILE_P2;
    cfg.app_timeout_ms = LH_APP_TIMEOUT;
    cfg.link_timeout_ms = (uint16_t)OCB_LINK_TIMEOUT_MS;
    cfg.debug_counters = debug_counters;
    cfg.boot_cause = 0u;   /* story 2.5 D7 */
    cfg.reboot_count = 0u;
    cfg.stall_reset_us = 0u;
    ocb_link_init(&h->link, &h->core, &h->mb, &cfg, h->now);
}

static inline void lh_init(lh *h, uint16_t cap)
{
    lh_init_cfg(h, cap, false);
}

/* RX high for OCB_LINK_TX_IDLE_HIGH_US: the gate opens (the clock moves). */
static inline void lh_open_gate(lh *h)
{
    (void)ocb_txgate_sample(ocb_link_gate(&h->link), true, h->now);
    h->now += OCB_LINK_TX_IDLE_HIGH_US;
    const bool open = ocb_txgate_sample(ocb_link_gate(&h->link), true, h->now);
    OCB_CHECK(open, "gate did not open");
}

/* Console handshake through the mailbox, as the P2 glue posts it (story 2.3). */
static inline void lh_make_ready(lh *h)
{
    ocb_usb_mailbox_handshake_done(&h->mb);
    ocb_link_poll(&h->link, h->now);
}

static inline void lh_feed(lh *h, const uint8_t *bytes, size_t n)
{
    for (size_t i = 0; i < n; i++) {
        ocb_link_rx_entry(&h->link, bytes[i], h->now);
    }
}

static inline size_t lh_wire(uint8_t type, uint8_t seq, const uint8_t *body, size_t len, uint8_t *wire)
{
    const size_t n = ocb_frame_encode(type, seq, body, len, wire, OCB_MAX_WIRE_FRAME);
    OCB_CHECK(n > 0u, "encode type 0x%02x len %zu failed", type, len);
    return n;
}

/* One frame with an explicit seq, fed byte by byte (delimiter included). */
static inline void lh_send_seq(lh *h, uint8_t type, uint8_t seq, const uint8_t *body, size_t len)
{
    uint8_t wire[OCB_MAX_WIRE_FRAME];
    const size_t n = lh_wire(type, seq, body, len, wire);
    lh_feed(h, wire, n);
}

/* One frame with h->seq, then h->seq++. */
static inline void lh_send(lh *h, uint8_t type, const uint8_t *body, size_t len)
{
    lh_send_seq(h, type, h->seq, body, len);
    h->seq = (uint8_t)(h->seq + 1u);
}

static inline void lh_hello_v(lh *h, uint8_t proto_ver)
{
    lh_send(h, OCB_MSG_HELLO, &proto_ver, 1u);
}

static inline void lh_hello(lh *h)
{
    lh_hello_v(h, (uint8_t)OCB_PROTO_VERSION);
}

static inline void lh_stop(lh *h)
{
    lh_send(h, OCB_MSG_STOP, NULL, 0u);
}

static inline void lh_ping(lh *h)
{
    lh_send(h, OCB_MSG_PING, NULL, 0u);
}

static inline size_t lh_body_queue_event(uint32_t t_us, ocb_state s, uint8_t *body)
{
    ocb_msg_queue_event m;
    m.t_us = t_us;
    m.state = s;
    return (size_t)ocb_msg_queue_event_encode(&m, body, OCB_MAX_BODY);
}

static inline void lh_queue_event(lh *h, uint32_t t_us, ocb_state s)
{
    uint8_t body[OCB_MAX_BODY];
    const size_t n = lh_body_queue_event(t_us, s, body);
    lh_send(h, OCB_MSG_QUEUE_EVENT, body, n);
}

static inline void lh_set_state(lh *h, ocb_state s)
{
    ocb_msg_set_state m;
    m.state = s;
    uint8_t body[OCB_MAX_BODY];
    const int n = ocb_msg_set_state_encode(&m, body, sizeof body);
    lh_send(h, OCB_MSG_SET_STATE, body, (size_t)n);
}

static inline void lh_start(lh *h, uint32_t lead_us)
{
    ocb_msg_start m = {lead_us};
    uint8_t body[OCB_MAX_BODY];
    const int n = ocb_msg_start_encode(&m, body, sizeof body);
    lh_send(h, OCB_MSG_START, body, (size_t)n);
}

static inline void lh_heartbeat(lh *h, uint8_t a, uint8_t b, uint8_t c, uint8_t flags)
{
    ocb_msg_heartbeat m = {{a, b, c}, flags};
    uint8_t body[OCB_MAX_BODY];
    const int n = ocb_msg_heartbeat_encode(&m, body, sizeof body);
    lh_send_seq(h, OCB_MSG_HEARTBEAT, 0x33u, body, (size_t)n);
}

/* One emulated bridge heartbeat now (bridge_fw_ver 0.0.0, so INFO expectations
 * of the 2.6 tests hold; DTR per h->dtr). */
static inline void lh_hb_now(lh *h)
{
    lh_heartbeat(h, 0u, 0u, 0u, h->dtr ? (uint8_t)OCB_HB_FLAG_DTR : 0u);
    h->last_hb_sent = h->now;
}

/* First heartbeat: link_ok 1, the watchdog armed; lh_advance keeps it up. */
static inline void lh_link_up(lh *h)
{
    lh_hb_now(h);
    h->hb_up = true;
}

/* Moves the clock by us, in slices of at most hb_period_us, injecting one
 * heartbeat per period while the link is up and auto_hb is set. */
static inline void lh_advance(lh *h, uint64_t us)
{
    while (us > 0u) {
        uint64_t step = us;
        if (h->hb_up && h->auto_hb) {
            const uint64_t due = h->last_hb_sent + h->hb_period_us;
            const uint64_t to_due = due > h->now ? due - h->now : 0u;
            if (step > to_due) {
                step = to_due;
            }
        }
        h->now += step;
        us -= step;
        if (h->hb_up && h->auto_hb && h->now >= h->last_hb_sent + h->hb_period_us) {
            lh_hb_now(h);
        }
    }
}

/* Checks one TX output as a whole, valid controller frame and decodes it. */
static inline bool lh_decode_out(const uint8_t *wire, size_t n, lh_out *o)
{
    if (n < 2u || n > (size_t)OCB_MAX_WIRE_FRAME || wire[n - 1u] != 0u) {
        OCB_CHECK(false, "TX output of %zu bytes is not one delimited frame", n);
        return false;
    }
    for (size_t i = 0; i + 1u < n; i++) {
        if (wire[i] == 0u) {
            OCB_CHECK(false, "TX output has a 0x00 inside the frame at %zu", i);
            return false;
        }
    }
    ocb_frame f;
    const ocb_err e = ocb_frame_decode(OCB_RX_APP, wire, n - 1u, &f);
    if (e != OCB_OK) {
        OCB_CHECK(false, "TX output does not decode as a controller frame (%d)", (int)e);
        return false;
    }
    o->type = f.type;
    o->seq = f.seq;
    o->len = f.len;
    memcpy(o->body, f.body, f.len);
    return true;
}

/* Pulls every due frame (unbounded cap). Returns how many. */
static inline size_t lh_drain(lh *h, lh_out *out, size_t max)
{
    size_t k = 0;
    uint8_t wire[OCB_MAX_WIRE_FRAME];
    for (;;) {
        const size_t n = ocb_link_tx_next(&h->link, h->now, wire, sizeof wire);
        if (n == 0u) {
            break;
        }
        lh_out tmp;
        if (!lh_decode_out(wire, n, &tmp)) {
            continue;
        }
        if (k < max) {
            out[k] = tmp;
        }
        k++;
        OCB_CHECK(k < 4096u, "TX never runs dry");
        if (k >= 4096u) {
            break;
        }
    }
    return k;
}

/* Pulls every due frame and keeps the replies only (STATUS dropped). */
static inline size_t lh_replies(lh *h, lh_out *out, size_t max)
{
    lh_out all[64];
    const size_t n = lh_drain(h, all, 64u);
    size_t k = 0;
    for (size_t i = 0; i < n && i < 64u; i++) {
        if (all[i].type != OCB_MSG_STATUS && k < max) {
            out[k++] = all[i];
        }
    }
    return k;
}

static inline void lh_discard_tx(lh *h)
{
    lh_out tmp[64];
    (void)lh_drain(h, tmp, 64u);
}

/* Expect exactly one reply, an ACK of seq. */
static inline void lh_expect_ack(lh *h, uint8_t seq, const char *what)
{
    lh_out o[8];
    const size_t n = lh_replies(h, o, 8u);
    OCB_CHECK(n == 1u && o[0].type == OCB_MSG_ACK && o[0].len == 1u && o[0].body[0] == seq,
              "%s: wanted one ACK(%u), got %zu frame(s), first type 0x%02x body0 %u", what, seq, n,
              n > 0u ? o[0].type : 0u, n > 0u ? o[0].body[0] : 0u);
}

/* Expect exactly one reply, NAK(reason, expected, got). */
static inline void lh_expect_nak(lh *h, uint8_t reason, uint8_t expected, uint8_t got, const char *what)
{
    lh_out o[8];
    const size_t n = lh_replies(h, o, 8u);
    ocb_msg_nak m = {0u, 0u, 0u};
    const bool ok = n == 1u && o[0].type == OCB_MSG_NAK && ocb_msg_nak_decode(o[0].body, o[0].len, &m) == OCB_OK;
    OCB_CHECK(ok && m.reason == reason && m.expected == expected && m.got == got,
              "%s: wanted one NAK(0x%02x, %u, %u), got %zu frame(s), type 0x%02x NAK(0x%02x, %u, %u)", what, reason,
              expected, got, n, n > 0u ? o[0].type : 0u, m.reason, m.expected, m.got);
}

static inline void lh_expect_none(lh *h, const char *what)
{
    lh_out o[8];
    const size_t n = lh_replies(h, o, 8u);
    OCB_CHECK(n == 0u, "%s: wanted no reply, got %zu (first type 0x%02x)", what, n, n > 0u ? o[0].type : 0u);
}

static inline ocb_state lst_buttons(uint16_t b)
{
    ocb_state s = OCB_NEUTRAL;
    s.buttons = b;
    return s;
}

static inline ocb_state lst_end(void)
{
    ocb_state s = OCB_NEUTRAL;
    s.flags = OCB_STATE_FLAG_END;
    return s;
}

/* Gate open, console ready, session open, replies drained, link up. */
static inline void lh_session(lh *h, uint16_t cap)
{
    lh_init(h, cap);
    lh_open_gate(h);
    lh_make_ready(h);
    lh_hello(h);
    lh_discard_tx(h);
    lh_link_up(h);
}

static inline bool lh_status_decode(const lh_out *o, ocb_msg_status *m)
{
    return o->type == OCB_MSG_STATUS && ocb_msg_status_decode(o->body, o->len, m) == OCB_OK;
}

#endif /* OCB_LINK_FIXTURE_H */
