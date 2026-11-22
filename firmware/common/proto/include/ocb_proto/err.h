/*
 * SPDX-License-Identifier: MIT
 *
 * ocb_proto error codes. Negative so message encoders can return a body
 * length or an error in one int. Not wire values.
 */
#ifndef OCB_PROTO_ERR_H
#define OCB_PROTO_ERR_H

typedef enum {
    OCB_OK = 0,
    OCB_ERR_EMPTY = -1,    /* empty frame: a zero-length run between delimiters (D10) */
    OCB_ERR_COBS = -2,     /* COBS decoding failed */
    OCB_ERR_CRC = -3,      /* length inconsistency or CRC mismatch (AD-3) */
    OCB_ERR_RANGE = -4,    /* type outside the receiver's range (AD-3) */
    OCB_ERR_BODY_LEN = -5, /* message body length wrong for its type (D10) */
    OCB_ERR_OVERLONG = -6, /* more than OCB_MAX_ENCODED_FRAME bytes without a delimiter (AD-4) */
    OCB_ERR_BUF = -7       /* output buffer too small, or an argument out of range */
} ocb_err;

#endif /* OCB_PROTO_ERR_H */
