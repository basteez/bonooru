/*
 * SPDX-License-Identifier: MIT
 *
 * COBS encoder and decoder, written from the published description, no
 * third-party code. Reference: S. Cheshire and M. Baker, "Consistent Overhead
 * Byte Stuffing", IEEE/ACM Transactions on Networking 7(2):159-172, 1999,
 * DOI 10.1109/90.769765; see also
 * https://en.wikipedia.org/wiki/Consistent_Overhead_Byte_Stuffing
 *
 * Rules: a code byte 0x01..0xFE is followed by code-1 non-zero bytes and an
 * implied 0x00 unless the block ends the input; 0xFF is followed by 254
 * non-zero bytes and no implied 0x00. A 0xFF block that ends the input is not
 * followed by a final code byte. The empty input encodes to 0x01.
 */
#include "ocb_proto/cobs.h"

size_t ocb_cobs_encode(const uint8_t *in, size_t in_len, uint8_t *out, size_t out_cap)
{
    size_t code_pos = 0; /* where the current block's code byte goes */
    size_t o = 1;        /* next free output position */
    uint8_t code = 1;    /* current block length + 1 */

    if (out_cap == 0u) {
        return 0;
    }
    for (size_t i = 0; i < in_len; i++) {
        const uint8_t b = in[i];
        if (b != 0u) {
            if (o >= out_cap) {
                return 0;
            }
            out[o++] = b;
            code++;
            if (code != 0xFFu) {
                continue;
            }
            /* 254 data bytes: close the block, no implied zero. */
            out[code_pos] = 0xFF;
            code = 1;
            if (i + 1u == in_len) {
                return o;
            }
        } else {
            out[code_pos] = code;
            code = 1;
        }
        /* Reserve the next block's code byte. */
        if (o >= out_cap) {
            return 0;
        }
        code_pos = o++;
    }
    out[code_pos] = code;
    return o;
}

ocb_cobs_result ocb_cobs_decode(const uint8_t *in, size_t in_len, uint8_t *out, size_t out_cap,
                                size_t *out_len)
{
    size_t i = 0;
    size_t o = 0;

    *out_len = 0;
    if (in_len == 0u) {
        return OCB_COBS_ERR_EMPTY;
    }
    while (i < in_len) {
        const uint8_t code = in[i++];
        if (code == 0u) {
            return OCB_COBS_ERR_ZERO;
        }
        const size_t n = (size_t)code - 1u;
        if (n > in_len - i) {
            return OCB_COBS_ERR_TRUNCATED;
        }
        for (size_t k = 0; k < n; k++) {
            const uint8_t b = in[i++];
            if (b == 0u) {
                return OCB_COBS_ERR_ZERO;
            }
            if (o >= out_cap) {
                return OCB_COBS_ERR_SPACE;
            }
            out[o++] = b;
        }
        if (code != 0xFFu && i < in_len) {
            if (o >= out_cap) {
                return OCB_COBS_ERR_SPACE;
            }
            out[o++] = 0;
        }
    }
    *out_len = o;
    return OCB_COBS_OK;
}
