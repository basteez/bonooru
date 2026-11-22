/*
 * SPDX-License-Identifier: MIT
 *
 * ocb_proto wire constants: the only place a wire number is written in C
 * (AD-2). Mirrored, value for value, in common/proto/vectors/constants.txt
 * (checked both ways by test F.9) and in docs/link-protocol.md; on a
 * disagreement this header and the vectors win over the doc.
 *
 * Plain integer macros without suffixes, so shell scripts and the Rust side
 * can parse the values. Units are part of the name.
 */
#ifndef OCB_PROTO_CONSTS_H
#define OCB_PROTO_CONSTS_H

/* Protocol version (AD-7): exact match required. Keep a plain decimal
 * integer: tools/check-proto-version.sh parses this line. */
#define OCB_PROTO_VERSION 1

/* Message types (AD-3, docs/link-protocol.md "Messages"). 0x82 is reserved. */
#define OCB_MSG_HELLO       0x01
#define OCB_MSG_SET_STATE   0x02
#define OCB_MSG_QUEUE_EVENT 0x03
#define OCB_MSG_START       0x04
#define OCB_MSG_STOP        0x05
#define OCB_MSG_PING        0x06
#define OCB_MSG_HEARTBEAT   0x40
#define OCB_MSG_INFO        0x81
#define OCB_MSG_STATUS      0x83
#define OCB_MSG_ACK         0x84
#define OCB_MSG_NAK         0x85
#define OCB_MSG_EVENT       0x86

/* Type ranges per direction (AD-3). Type 0x00 is never valid. */
#define OCB_TYPE_APP_MIN    0x01 /* app -> controller */
#define OCB_TYPE_APP_MAX    0x3F
#define OCB_TYPE_BRIDGE_MIN 0x40 /* bridge -> controller, link-local */
#define OCB_TYPE_BRIDGE_MAX 0x7F
#define OCB_TYPE_CTRL_MIN   0x80 /* controller -> app */
#define OCB_TYPE_CTRL_MAX   0xFF

/* NAK reasons (AD-6). Transport 0x01..0x0F never advance `expected`;
 * semantic 0x10..0x7F do. Unknown reasons have no class (AD-19). */
#define OCB_NAK_CRC          0x01
#define OCB_NAK_SEQ          0x02
#define OCB_NAK_FULL         0x03
#define OCB_NAK_VERSION      0x10
#define OCB_NAK_BUSY         0x11
#define OCB_NAK_ORDER        0x12
#define OCB_NAK_BAD_STATE    0x13
#define OCB_NAK_UNKNOWN_TYPE 0x14
#define OCB_NAK_NO_LINK      0x15
#define OCB_NAK_NOT_READY    0x16
#define OCB_NAK_ABORTED      0x17

#define OCB_NAK_TRANSPORT_MIN 0x01
#define OCB_NAK_TRANSPORT_MAX 0x0F
#define OCB_NAK_SEMANTIC_MIN  0x10
#define OCB_NAK_SEMANTIC_MAX  0x7F
#define OCB_NAK_IS_TRANSPORT(r) ((r) >= OCB_NAK_TRANSPORT_MIN && (r) <= OCB_NAK_TRANSPORT_MAX)
#define OCB_NAK_IS_SEMANTIC(r)  ((r) >= OCB_NAK_SEMANTIC_MIN && (r) <= OCB_NAK_SEMANTIC_MAX)

/* EVENT codes (AD-5, AD-8); the same numbers serve STATUS.last_event.
 * EVT_NONE appears only in STATUS.last_event (no event yet). */
#define OCB_EVT_NONE          0x00
#define OCB_EVT_PLAYBACK_DONE 0x01
#define OCB_EVT_LINK_LOST     0x02
#define OCB_EVT_LATE          0x03
#define OCB_EVT_UNDERRUN      0x04
#define OCB_EVT_USB_SUSPENDED 0x05

/* Controller modes, STATUS.mode (AD-8). */
#define OCB_MODE_IDLE    0x00
#define OCB_MODE_DIRECT  0x01
#define OCB_MODE_PLAYING 0x02

/* INFO.hid_profile: one id per profile of docs/usb-controller-emulation.md. */
#define OCB_HID_PROFILE_UNKNOWN 0x00
#define OCB_HID_PROFILE_P1      0x01
#define OCB_HID_PROFILE_P2      0x02
#define OCB_HID_PROFILE_P3      0x03

/* HEARTBEAT.flags (AD-4). */
#define OCB_HB_FLAG_DTR 0x01

/* STATUS.flags (AD-19); reserved bits are written 0 and ignored on receive. */
#define OCB_STATUS_LINK_OK       0x01
#define OCB_STATUS_CONSOLE_READY 0x02
#define OCB_STATUS_RESERVED_MASK 0xFC

/* State flags (AD-8, AD-10). */
#define OCB_STATE_FLAG_END      0x01
#define OCB_STATE_RESERVED_MASK 0xFE

/* Buttons, canonical P1 bit order for every profile (AD-10). */
#define OCB_BTN_Y       0x0001
#define OCB_BTN_B       0x0002
#define OCB_BTN_A       0x0004
#define OCB_BTN_X       0x0008
#define OCB_BTN_L       0x0010
#define OCB_BTN_R       0x0020
#define OCB_BTN_ZL      0x0040
#define OCB_BTN_ZR      0x0080
#define OCB_BTN_MINUS   0x0100
#define OCB_BTN_PLUS    0x0200
#define OCB_BTN_L3      0x0400
#define OCB_BTN_R3      0x0800
#define OCB_BTN_HOME    0x1000
#define OCB_BTN_CAPTURE 0x2000

/* Hat, clockwise from up; 8 = neutral (AD-10). Sticks: 0x80 = centre,
 * ly 0x00 = up. */
#define OCB_HAT_UP         0
#define OCB_HAT_UP_RIGHT   1
#define OCB_HAT_RIGHT      2
#define OCB_HAT_DOWN_RIGHT 3
#define OCB_HAT_DOWN       4
#define OCB_HAT_DOWN_LEFT  5
#define OCB_HAT_LEFT       6
#define OCB_HAT_UP_LEFT    7
#define OCB_HAT_NEUTRAL    8
#define OCB_STICK_CENTER   0x80

/* Sizes (story 2.2 D5). Raw frame = type, seq, len (3) + body + crc16 (2).
 * COBS adds at most ceil(n / 254) bytes, at least 1. */
#define OCB_STATE_SIZE        8
#define OCB_MAX_BODY          255
#define OCB_MIN_RAW_FRAME     5
#define OCB_MAX_RAW_FRAME     260
#define OCB_MAX_ENCODED_FRAME 262 /* non-delimiter bytes of the largest frame (AD-4) */
#define OCB_MAX_WIRE_FRAME    263 /* plus the 0x00 delimiter */
#define OCB_FRAME_DELIMITER   0x00
#define OCB_FRAME_HEADER_SIZE 3
#define OCB_FRAME_CRC_SIZE    2

/* Body lengths of protocol v1 (D10: HELLO, INFO, HEARTBEAT, STATUS accept
 * longer bodies; every other message requires the exact length). */
#define OCB_BODY_LEN_HELLO       1
#define OCB_BODY_LEN_INFO        14
#define OCB_BODY_LEN_SET_STATE   8
#define OCB_BODY_LEN_QUEUE_EVENT 12
#define OCB_BODY_LEN_START       4
#define OCB_BODY_LEN_STOP        0
#define OCB_BODY_LEN_PING        0
#define OCB_BODY_LEN_HEARTBEAT   4
#define OCB_BODY_LEN_STATUS      11
#define OCB_BODY_LEN_ACK         1
#define OCB_BODY_LEN_NAK         3
#define OCB_BODY_LEN_EVENT       1

/* CRC-16/CCITT-FALSE = RevEng CRC-16/IBM-3740: refin/refout false,
 * xorout 0x0000; sent little-endian after the body. */
#define OCB_CRC16_POLY  0x1021
#define OCB_CRC16_INIT  0xFFFF
#define OCB_CRC16_CHECK 0x29B1 /* CRC of ASCII "123456789" */

/* Link timing and sizing (story 2.2 D6). */
#define OCB_LINK_BAUD              460800 /* AD-4; a build constant */
#define OCB_HEARTBEAT_PERIOD_MS    20     /* AD-4 */
#define OCB_HEARTBEAT_MAX_GAP_MS   27     /* AD-4, test F.7 */
#define OCB_LINK_TIMEOUT_MS        80     /* AD-5 default */
#define OCB_LINK_TIMEOUT_MAX_MS    100    /* AD-5, NFR-05 */
#define OCB_ACK_TIMEOUT_MS         50     /* AD-6 */
#define OCB_ACK_WINDOW             64     /* AD-6: max unACKed frames, stale window */
#define OCB_HELLO_RESEND_MS        100    /* AD-7 */
#define OCB_APP_TIMEOUT_MS_DEFAULT 1000   /* AD-5, until W.1 */
#define OCB_APP_PING_DIVISOR       4      /* AD-7: PING after app_timeout_ms / 4 idle */
#define OCB_STATUS_PERIOD_MS       50     /* AD-19 */
#define OCB_STATUS_MIN_GAP_MS      10     /* AD-19 coalescing */
#define OCB_CONSOLE_POLL_STALL_MS  100    /* AD-19 */
#define OCB_QUEUE_CAP_DEFAULT      512    /* AD-6, AD-7: default INFO.queue_cap */
#define OCB_T_US_LIMIT             0x80000000 /* AD-8: t_us < 2^31 */
#define OCB_STOP_NEUTRAL_MS        100    /* NFR-05: neutral within 100 ms of STOP */

#endif /* OCB_PROTO_CONSTS_H */
