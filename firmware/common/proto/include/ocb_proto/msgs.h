/*
 * SPDX-License-Identifier: MIT
 *
 * Message bodies of docs/link-protocol.md, protocol v1. All multi-byte fields
 * little-endian, read and written byte by byte (no struct casts, no alignment
 * or host-endianness assumption).
 *
 * Encoders: int ocb_msg_X_encode(const ocb_msg_X *m, uint8_t *body, size_t cap)
 * return the body length, or OCB_ERR_BUF when cap is too small.
 * Decoders: int ocb_msg_X_decode(const uint8_t *body, size_t len, ocb_msg_X *m)
 * return OCB_OK or OCB_ERR_BODY_LEN (D10): HELLO, INFO, HEARTBEAT and STATUS
 * accept len >= their v1 size and ignore (STATUS: expose) the trailing bytes;
 * every other message requires the exact length. Unknown NAK reasons and
 * EVENT codes decode as plain bytes (AD-19). Flag bytes are not validated.
 */
#ifndef OCB_PROTO_MSGS_H
#define OCB_PROTO_MSGS_H

#include <stddef.h>
#include <stdint.h>

#include "ocb_proto/consts.h"
#include "ocb_proto/err.h"
#include "ocb_proto/state.h"

typedef struct {
    uint8_t proto_ver;
} ocb_msg_hello;

typedef struct {
    uint8_t proto_ver; /* byte 0, frozen forever (AD-7) */
    uint8_t fw_ver[3];
    uint8_t bridge_ver[3];
    uint8_t hid_profile; /* OCB_HID_PROFILE_* */
    uint16_t queue_cap;
    uint16_t app_timeout_ms;
    uint16_t report_period_us;
} ocb_msg_info;

typedef struct {
    ocb_state state;
} ocb_msg_set_state;

typedef struct {
    uint32_t t_us;
    ocb_state state;
} ocb_msg_queue_event;

typedef struct {
    uint32_t lead_us;
} ocb_msg_start;

typedef struct {
    uint8_t bridge_fw_ver[3];
    uint8_t flags; /* OCB_HB_FLAG_DTR */
} ocb_msg_heartbeat;

typedef struct {
    uint16_t free;
    uint32_t now_us;
    uint8_t last_seq;
    uint8_t mode; /* OCB_MODE_* */
    uint8_t abort_count;
    uint8_t last_event; /* OCB_EVT_* */
    uint8_t flags;      /* OCB_STATUS_* */
    /* Trailing debug counters: on decode, a pointer into the caller's body;
     * on encode, appended after the 11 base bytes (may be NULL if 0). */
    const uint8_t *extra;
    size_t extra_len;
} ocb_msg_status;

typedef struct {
    uint8_t seq;
} ocb_msg_ack;

typedef struct {
    uint8_t reason; /* OCB_NAK_* */
    uint8_t expected;
    uint8_t got;
} ocb_msg_nak;

typedef struct {
    uint8_t code; /* OCB_EVT_* */
} ocb_msg_event;

int ocb_msg_hello_encode(const ocb_msg_hello *m, uint8_t *body, size_t cap);
int ocb_msg_hello_decode(const uint8_t *body, size_t len, ocb_msg_hello *m);
int ocb_msg_info_encode(const ocb_msg_info *m, uint8_t *body, size_t cap);
int ocb_msg_info_decode(const uint8_t *body, size_t len, ocb_msg_info *m);
int ocb_msg_set_state_encode(const ocb_msg_set_state *m, uint8_t *body, size_t cap);
int ocb_msg_set_state_decode(const uint8_t *body, size_t len, ocb_msg_set_state *m);
int ocb_msg_queue_event_encode(const ocb_msg_queue_event *m, uint8_t *body, size_t cap);
int ocb_msg_queue_event_decode(const uint8_t *body, size_t len, ocb_msg_queue_event *m);
int ocb_msg_start_encode(const ocb_msg_start *m, uint8_t *body, size_t cap);
int ocb_msg_start_decode(const uint8_t *body, size_t len, ocb_msg_start *m);
/* STOP and PING have no body: encode returns 0, decode checks len == 0. */
int ocb_msg_stop_encode(uint8_t *body, size_t cap);
int ocb_msg_stop_decode(const uint8_t *body, size_t len);
int ocb_msg_ping_encode(uint8_t *body, size_t cap);
int ocb_msg_ping_decode(const uint8_t *body, size_t len);
int ocb_msg_heartbeat_encode(const ocb_msg_heartbeat *m, uint8_t *body, size_t cap);
int ocb_msg_heartbeat_decode(const uint8_t *body, size_t len, ocb_msg_heartbeat *m);
int ocb_msg_status_encode(const ocb_msg_status *m, uint8_t *body, size_t cap);
int ocb_msg_status_decode(const uint8_t *body, size_t len, ocb_msg_status *m);
int ocb_msg_ack_encode(const ocb_msg_ack *m, uint8_t *body, size_t cap);
int ocb_msg_ack_decode(const uint8_t *body, size_t len, ocb_msg_ack *m);
int ocb_msg_nak_encode(const ocb_msg_nak *m, uint8_t *body, size_t cap);
int ocb_msg_nak_decode(const uint8_t *body, size_t len, ocb_msg_nak *m);
int ocb_msg_event_encode(const ocb_msg_event *m, uint8_t *body, size_t cap);
int ocb_msg_event_decode(const uint8_t *body, size_t len, ocb_msg_event *m);

#endif /* OCB_PROTO_MSGS_H */
