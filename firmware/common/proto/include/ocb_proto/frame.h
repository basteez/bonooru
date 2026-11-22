/*
 * SPDX-License-Identifier: MIT
 *
 * Frames (AD-3): raw = type | seq | len | body[len] | crc16 LE, the CRC over
 * type, seq, len and body; COBS-encoded and closed by a 0x00 delimiter.
 *
 * ocb_frame_decode runs the first four receive checks of AD-3 in order:
 * COBS, length consistency (reported as a CRC error), CRC, type range. The
 * seq and semantic checks need session state and belong to the consumers.
 */
#ifndef OCB_PROTO_FRAME_H
#define OCB_PROTO_FRAME_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "ocb_proto/consts.h"
#include "ocb_proto/err.h"

/* Which end receives: decides the accepted type range. */
typedef enum {
    OCB_RX_CONTROLLER, /* accepts 0x01..0x7F: app frames and bridge heartbeats */
    OCB_RX_APP         /* accepts 0x80..0xFF: controller frames */
} ocb_rx_side;

typedef struct {
    uint8_t type;
    uint8_t seq;
    uint8_t len;
    uint8_t body[OCB_MAX_BODY];
} ocb_frame;

/* Writes the COBS-encoded frame plus the trailing 0x00 into `out`. Returns
 * the total length (<= OCB_MAX_WIRE_FRAME), or 0 when body_len >
 * OCB_MAX_BODY or out_cap is too small. The type range is not checked. `body`
 * may be NULL when body_len is 0. */
size_t ocb_frame_encode(uint8_t type, uint8_t seq, const uint8_t *body, size_t body_len, uint8_t *out,
                        size_t out_cap);

/* Decodes one frame, `enc` without its delimiter. Returns OCB_OK (and fills
 * *out, a copy) or the first failing check: OCB_ERR_EMPTY (enc_len 0),
 * OCB_ERR_OVERLONG (enc_len > OCB_MAX_ENCODED_FRAME), OCB_ERR_COBS (any
 * malformed COBS), OCB_ERR_CRC (decoded length inconsistent, including more
 * than OCB_MAX_RAW_FRAME bytes, or CRC mismatch), OCB_ERR_RANGE. On an error
 * *out is left untouched. `out` must not be NULL; `enc` may be NULL only
 * when enc_len is 0. Never reads past enc[enc_len]. */
ocb_err ocb_frame_decode(ocb_rx_side side, const uint8_t *enc, size_t enc_len, ocb_frame *out);

/* True if `type` is in the receive range of `side`. */
bool ocb_frame_type_in_range(ocb_rx_side side, uint8_t type);

/* Receive counters. Each dropped run increments exactly one of them.
 * `empty` also counts the session-opening and idle 0x00 bytes: informational,
 * never an error (D10). */
typedef struct {
    uint32_t frames_ok;
    uint32_t empty;
    uint32_t cobs_errors;
    uint32_t crc_errors;
    uint32_t range_errors;
    uint32_t overlong;
} ocb_rx_stats;

/* Streaming splitter: fed one byte at a time, no I/O, no allocation; any
 * number of instances can coexist. */
typedef struct {
    ocb_rx_side side;
    size_t len;
    bool discarding; /* inside an overlong run, until the next 0x00 */
    ocb_err last_err; /* reason of the last dropped run (OCB_OK if none yet) */
    ocb_rx_stats stats;
    uint8_t buf[OCB_MAX_ENCODED_FRAME];
} ocb_rx;

typedef enum {
    OCB_RX_NONE,    /* byte consumed, nothing ready */
    OCB_RX_FRAME,   /* a valid frame is in *out */
    OCB_RX_DROPPED  /* a delimiter ended a run that was dropped (see last_err, stats) */
} ocb_rx_result;

void ocb_rx_init(ocb_rx *rx, ocb_rx_side side);

/* Feeds one byte. A run longer than OCB_MAX_ENCODED_FRAME non-zero bytes is
 * counted in `overlong` once, when its first excess byte arrives; the rest of
 * the run up to and including the next 0x00 is discarded (that 0x00 returns
 * OCB_RX_DROPPED without counting again). */
ocb_rx_result ocb_rx_push(ocb_rx *rx, uint8_t byte, ocb_frame *out);

#endif /* OCB_PROTO_FRAME_H */
