/*
 * SPDX-License-Identifier: MIT
 *
 * Message body codecs (docs/link-protocol.md, story 2.2 D1 and D10).
 * Little-endian, byte by byte: never cast a byte buffer to a struct.
 */
#include "ocb_proto/msgs.h"

#include <string.h>

static void put_u16(uint8_t *p, uint16_t v)
{
    p[0] = (uint8_t)(v & 0xFFu);
    p[1] = (uint8_t)(v >> 8);
}

static void put_u32(uint8_t *p, uint32_t v)
{
    p[0] = (uint8_t)(v & 0xFFu);
    p[1] = (uint8_t)((v >> 8) & 0xFFu);
    p[2] = (uint8_t)((v >> 16) & 0xFFu);
    p[3] = (uint8_t)(v >> 24);
}

static uint16_t get_u16(const uint8_t *p)
{
    return (uint16_t)((uint16_t)p[0] | (uint16_t)((uint16_t)p[1] << 8));
}

static uint32_t get_u32(const uint8_t *p)
{
    return (uint32_t)p[0] | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24);
}

/* ---------------------------------------------------------------- HELLO */

int ocb_msg_hello_encode(const ocb_msg_hello *m, uint8_t *body, size_t cap)
{
    if (cap < (size_t)OCB_BODY_LEN_HELLO) {
        return OCB_ERR_BUF;
    }
    body[0] = m->proto_ver;
    return OCB_BODY_LEN_HELLO;
}

int ocb_msg_hello_decode(const uint8_t *body, size_t len, ocb_msg_hello *m)
{
    if (len < (size_t)OCB_BODY_LEN_HELLO) {
        return OCB_ERR_BODY_LEN;
    }
    m->proto_ver = body[0];
    return OCB_OK;
}

/* ---------------------------------------------------------------- INFO */

int ocb_msg_info_encode(const ocb_msg_info *m, uint8_t *body, size_t cap)
{
    if (cap < (size_t)OCB_BODY_LEN_INFO) {
        return OCB_ERR_BUF;
    }
    body[0] = m->proto_ver;
    memcpy(&body[1], m->fw_ver, 3);
    memcpy(&body[4], m->bridge_ver, 3);
    body[7] = m->hid_profile;
    put_u16(&body[8], m->queue_cap);
    put_u16(&body[10], m->app_timeout_ms);
    put_u16(&body[12], m->report_period_us);
    return OCB_BODY_LEN_INFO;
}

int ocb_msg_info_decode(const uint8_t *body, size_t len, ocb_msg_info *m)
{
    if (len < (size_t)OCB_BODY_LEN_INFO) {
        return OCB_ERR_BODY_LEN;
    }
    m->proto_ver = body[0];
    memcpy(m->fw_ver, &body[1], 3);
    memcpy(m->bridge_ver, &body[4], 3);
    m->hid_profile = body[7];
    m->queue_cap = get_u16(&body[8]);
    m->app_timeout_ms = get_u16(&body[10]);
    m->report_period_us = get_u16(&body[12]);
    return OCB_OK;
}

/* ---------------------------------------------------------------- SET_STATE */

int ocb_msg_set_state_encode(const ocb_msg_set_state *m, uint8_t *body, size_t cap)
{
    if (cap < (size_t)OCB_BODY_LEN_SET_STATE) {
        return OCB_ERR_BUF;
    }
    ocb_state_encode(&m->state, body);
    return OCB_BODY_LEN_SET_STATE;
}

int ocb_msg_set_state_decode(const uint8_t *body, size_t len, ocb_msg_set_state *m)
{
    if (len != (size_t)OCB_BODY_LEN_SET_STATE) {
        return OCB_ERR_BODY_LEN;
    }
    ocb_state_decode(body, &m->state);
    return OCB_OK;
}

/* ---------------------------------------------------------------- QUEUE_EVENT */

int ocb_msg_queue_event_encode(const ocb_msg_queue_event *m, uint8_t *body, size_t cap)
{
    if (cap < (size_t)OCB_BODY_LEN_QUEUE_EVENT) {
        return OCB_ERR_BUF;
    }
    put_u32(&body[0], m->t_us);
    ocb_state_encode(&m->state, &body[4]);
    return OCB_BODY_LEN_QUEUE_EVENT;
}

int ocb_msg_queue_event_decode(const uint8_t *body, size_t len, ocb_msg_queue_event *m)
{
    if (len != (size_t)OCB_BODY_LEN_QUEUE_EVENT) {
        return OCB_ERR_BODY_LEN;
    }
    m->t_us = get_u32(&body[0]);
    ocb_state_decode(&body[4], &m->state);
    return OCB_OK;
}

/* ---------------------------------------------------------------- START */

int ocb_msg_start_encode(const ocb_msg_start *m, uint8_t *body, size_t cap)
{
    if (cap < (size_t)OCB_BODY_LEN_START) {
        return OCB_ERR_BUF;
    }
    put_u32(&body[0], m->lead_us);
    return OCB_BODY_LEN_START;
}

int ocb_msg_start_decode(const uint8_t *body, size_t len, ocb_msg_start *m)
{
    if (len != (size_t)OCB_BODY_LEN_START) {
        return OCB_ERR_BODY_LEN;
    }
    m->lead_us = get_u32(&body[0]);
    return OCB_OK;
}

/* ---------------------------------------------------------------- STOP, PING */

int ocb_msg_stop_encode(uint8_t *body, size_t cap)
{
    (void)body;
    (void)cap;
    return OCB_BODY_LEN_STOP;
}

int ocb_msg_stop_decode(const uint8_t *body, size_t len)
{
    (void)body;
    return len == (size_t)OCB_BODY_LEN_STOP ? OCB_OK : OCB_ERR_BODY_LEN;
}

int ocb_msg_ping_encode(uint8_t *body, size_t cap)
{
    (void)body;
    (void)cap;
    return OCB_BODY_LEN_PING;
}

int ocb_msg_ping_decode(const uint8_t *body, size_t len)
{
    (void)body;
    return len == (size_t)OCB_BODY_LEN_PING ? OCB_OK : OCB_ERR_BODY_LEN;
}

/* ---------------------------------------------------------------- HEARTBEAT */

int ocb_msg_heartbeat_encode(const ocb_msg_heartbeat *m, uint8_t *body, size_t cap)
{
    if (cap < (size_t)OCB_BODY_LEN_HEARTBEAT) {
        return OCB_ERR_BUF;
    }
    memcpy(&body[0], m->bridge_fw_ver, 3);
    body[3] = m->flags;
    return OCB_BODY_LEN_HEARTBEAT;
}

int ocb_msg_heartbeat_decode(const uint8_t *body, size_t len, ocb_msg_heartbeat *m)
{
    if (len < (size_t)OCB_BODY_LEN_HEARTBEAT) {
        return OCB_ERR_BODY_LEN;
    }
    memcpy(m->bridge_fw_ver, &body[0], 3);
    m->flags = body[3];
    return OCB_OK;
}

/* ---------------------------------------------------------------- STATUS */

int ocb_msg_status_encode(const ocb_msg_status *m, uint8_t *body, size_t cap)
{
    const size_t extra_len = m->extra != NULL ? m->extra_len : 0u;

    if (extra_len > (size_t)OCB_MAX_BODY - (size_t)OCB_BODY_LEN_STATUS ||
        cap < (size_t)OCB_BODY_LEN_STATUS + extra_len) {
        return OCB_ERR_BUF;
    }
    put_u16(&body[0], m->free);
    put_u32(&body[2], m->now_us);
    body[6] = m->last_seq;
    body[7] = m->mode;
    body[8] = m->abort_count;
    body[9] = m->last_event;
    body[10] = m->flags;
    if (extra_len > 0u) {
        memcpy(&body[OCB_BODY_LEN_STATUS], m->extra, extra_len);
    }
    return (int)(OCB_BODY_LEN_STATUS + extra_len);
}

int ocb_msg_status_decode(const uint8_t *body, size_t len, ocb_msg_status *m)
{
    if (len < (size_t)OCB_BODY_LEN_STATUS) {
        return OCB_ERR_BODY_LEN;
    }
    m->free = get_u16(&body[0]);
    m->now_us = get_u32(&body[2]);
    m->last_seq = body[6];
    m->mode = body[7];
    m->abort_count = body[8];
    m->last_event = body[9];
    m->flags = body[10];
    m->extra_len = len - (size_t)OCB_BODY_LEN_STATUS;
    m->extra = m->extra_len > 0u ? &body[OCB_BODY_LEN_STATUS] : NULL;
    return OCB_OK;
}

/* ---------------------------------------------------------------- ACK, NAK, EVENT */

int ocb_msg_ack_encode(const ocb_msg_ack *m, uint8_t *body, size_t cap)
{
    if (cap < (size_t)OCB_BODY_LEN_ACK) {
        return OCB_ERR_BUF;
    }
    body[0] = m->seq;
    return OCB_BODY_LEN_ACK;
}

int ocb_msg_ack_decode(const uint8_t *body, size_t len, ocb_msg_ack *m)
{
    if (len != (size_t)OCB_BODY_LEN_ACK) {
        return OCB_ERR_BODY_LEN;
    }
    m->seq = body[0];
    return OCB_OK;
}

int ocb_msg_nak_encode(const ocb_msg_nak *m, uint8_t *body, size_t cap)
{
    if (cap < (size_t)OCB_BODY_LEN_NAK) {
        return OCB_ERR_BUF;
    }
    body[0] = m->reason;
    body[1] = m->expected;
    body[2] = m->got;
    return OCB_BODY_LEN_NAK;
}

int ocb_msg_nak_decode(const uint8_t *body, size_t len, ocb_msg_nak *m)
{
    if (len != (size_t)OCB_BODY_LEN_NAK) {
        return OCB_ERR_BODY_LEN;
    }
    m->reason = body[0];
    m->expected = body[1];
    m->got = body[2];
    return OCB_OK;
}

int ocb_msg_event_encode(const ocb_msg_event *m, uint8_t *body, size_t cap)
{
    if (cap < (size_t)OCB_BODY_LEN_EVENT) {
        return OCB_ERR_BUF;
    }
    body[0] = m->code;
    return OCB_BODY_LEN_EVENT;
}

int ocb_msg_event_decode(const uint8_t *body, size_t len, ocb_msg_event *m)
{
    if (len != (size_t)OCB_BODY_LEN_EVENT) {
        return OCB_ERR_BODY_LEN;
    }
    m->code = body[0];
    return OCB_OK;
}
