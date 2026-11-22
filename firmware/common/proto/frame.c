/*
 * SPDX-License-Identifier: MIT
 *
 * Frame encode/decode (AD-3) and the compile-time checks of the sizing and
 * timing constants (story 2.2 D5, D6).
 */
#include "ocb_proto/frame.h"

#include <string.h>

#include "ocb_proto/cobs.h"
#include "ocb_proto/crc16.h"

/* C99 compile-time checks (negative array size on failure). */
#define OCB_CT_CHECK(name, cond) typedef char ocb_ct_##name[(cond) ? 1 : -1]

OCB_CT_CHECK(raw_frame_size,
             OCB_MAX_RAW_FRAME == OCB_FRAME_HEADER_SIZE + OCB_MAX_BODY + OCB_FRAME_CRC_SIZE);
OCB_CT_CHECK(min_raw_frame, OCB_MIN_RAW_FRAME == OCB_FRAME_HEADER_SIZE + OCB_FRAME_CRC_SIZE);
OCB_CT_CHECK(max_encoded_frame, OCB_MAX_ENCODED_FRAME == OCB_COBS_MAX_ENCODED(OCB_MAX_RAW_FRAME));
OCB_CT_CHECK(max_wire_frame, OCB_MAX_WIRE_FRAME == OCB_MAX_ENCODED_FRAME + 1);
OCB_CT_CHECK(link_timeout, OCB_LINK_TIMEOUT_MS <= OCB_LINK_TIMEOUT_MAX_MS);
/* A heartbeat due while a maximum frame is on the wire (10 bits per byte at
 * OCB_LINK_BAUD, rounded up to whole ms) still meets the AD-4 gap bound:
 * 20 + 6 <= 27. */
OCB_CT_CHECK(heartbeat_gap,
             OCB_HEARTBEAT_PERIOD_MS +
                     ((OCB_MAX_WIRE_FRAME * 10 * 1000 + OCB_LINK_BAUD - 1) / OCB_LINK_BAUD) <=
                 OCB_HEARTBEAT_MAX_GAP_MS);
OCB_CT_CHECK(state_size, OCB_BODY_LEN_SET_STATE == OCB_STATE_SIZE);

bool ocb_frame_type_in_range(ocb_rx_side side, uint8_t type)
{
    if (side == OCB_RX_CONTROLLER) {
        return type >= OCB_TYPE_APP_MIN && type <= OCB_TYPE_BRIDGE_MAX;
    }
    return type >= OCB_TYPE_CTRL_MIN;
}

size_t ocb_frame_encode(uint8_t type, uint8_t seq, const uint8_t *body, size_t body_len, uint8_t *out,
                        size_t out_cap)
{
    uint8_t raw[OCB_MAX_RAW_FRAME];

    if (body_len > (size_t)OCB_MAX_BODY || out_cap < 2u) {
        return 0;
    }
    raw[0] = type;
    raw[1] = seq;
    raw[2] = (uint8_t)body_len;
    if (body_len > 0u) {
        memcpy(&raw[OCB_FRAME_HEADER_SIZE], body, body_len);
    }
    const size_t n = OCB_FRAME_HEADER_SIZE + body_len;
    const uint16_t crc = ocb_crc16(raw, n);
    raw[n] = (uint8_t)(crc & 0xFFu);
    raw[n + 1u] = (uint8_t)(crc >> 8);

    /* Keep one byte for the delimiter. */
    const size_t enc = ocb_cobs_encode(raw, n + OCB_FRAME_CRC_SIZE, out, out_cap - 1u);
    if (enc == 0u) {
        return 0;
    }
    out[enc] = OCB_FRAME_DELIMITER;
    return enc + 1u;
}

ocb_err ocb_frame_decode(ocb_rx_side side, const uint8_t *enc, size_t enc_len, ocb_frame *out)
{
    uint8_t raw[OCB_MAX_RAW_FRAME];
    size_t n = 0;

    if (enc_len == 0u) {
        return OCB_ERR_EMPTY;
    }
    if (enc_len > (size_t)OCB_MAX_ENCODED_FRAME) {
        return OCB_ERR_OVERLONG;
    }
    /* 1. COBS: any malformed input is OCB_ERR_COBS (ocb_cobs_decode validates
     * the whole input before reporting SPACE). A well-formed input that
     * decodes to more than the largest raw frame cannot satisfy the length
     * rule below, so SPACE is a length inconsistency: a CRC error (AD-3). */
    const ocb_cobs_result cr = ocb_cobs_decode(enc, enc_len, raw, sizeof raw, &n);
    if (cr == OCB_COBS_ERR_SPACE) {
        return OCB_ERR_CRC;
    }
    if (cr != OCB_COBS_OK) {
        return OCB_ERR_COBS;
    }
    /* 2. Length consistency, treated as a CRC error (AD-3). */
    if (n < (size_t)OCB_MIN_RAW_FRAME ||
        n != (size_t)OCB_FRAME_HEADER_SIZE + raw[2] + (size_t)OCB_FRAME_CRC_SIZE) {
        return OCB_ERR_CRC;
    }
    /* 3. CRC over type, seq, len, body; sent little-endian. */
    const size_t crc_at = n - OCB_FRAME_CRC_SIZE;
    const uint16_t got = (uint16_t)((uint16_t)raw[crc_at] | (uint16_t)((uint16_t)raw[crc_at + 1u] << 8));
    if (ocb_crc16(raw, crc_at) != got) {
        return OCB_ERR_CRC;
    }
    /* 4. Type range of this receiver. */
    if (!ocb_frame_type_in_range(side, raw[0])) {
        return OCB_ERR_RANGE;
    }
    out->type = raw[0];
    out->seq = raw[1];
    out->len = raw[2];
    if (raw[2] > 0u) {
        memcpy(out->body, &raw[OCB_FRAME_HEADER_SIZE], raw[2]);
    }
    return OCB_OK;
}
