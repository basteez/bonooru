/*
 * SPDX-License-Identifier: MIT
 *
 * Consistent Overhead Byte Stuffing, without the 0x00 delimiter: the caller
 * appends or strips it. Bounded buffers only, no allocation.
 */
#ifndef OCB_PROTO_COBS_H
#define OCB_PROTO_COBS_H

#include <stddef.h>
#include <stdint.h>

/* Largest encoding of n input bytes: n + ceil(n / 254), at least 1. */
#define OCB_COBS_MAX_ENCODED(n) ((n) + ((n) + 253) / 254 + ((n) == 0 ? 1 : 0))

typedef enum {
    OCB_COBS_OK = 0,
    OCB_COBS_ERR_EMPTY,     /* empty input: not a valid encoding */
    OCB_COBS_ERR_ZERO,      /* a 0x00 byte inside the input */
    OCB_COBS_ERR_TRUNCATED, /* a code byte points past the end of the input */
    OCB_COBS_ERR_SPACE      /* the decoded data does not fit in out_cap */
} ocb_cobs_result;

/* Encodes `in_len` bytes into `out`. Returns the encoded length (>= 1), or 0
 * if `out_cap` is too small. Never writes at or past out[out_cap]. `in` may
 * be NULL when `in_len` is 0. */
size_t ocb_cobs_encode(const uint8_t *in, size_t in_len, uint8_t *out, size_t out_cap);

/* Decodes `in_len` bytes (no delimiter) into `out`. On OCB_COBS_OK,
 * *out_len is the decoded length; on an error it is 0 and the contents of
 * `out` are unspecified. Never reads past in[in_len], never writes at or past
 * out[out_cap]. */
ocb_cobs_result ocb_cobs_decode(const uint8_t *in, size_t in_len, uint8_t *out, size_t out_cap,
                                size_t *out_len);

#endif /* OCB_PROTO_COBS_H */
