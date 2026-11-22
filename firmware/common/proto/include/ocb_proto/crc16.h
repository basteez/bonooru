/*
 * SPDX-License-Identifier: MIT
 *
 * CRC-16/CCITT-FALSE (RevEng CRC-16/IBM-3740): width 16, poly 0x1021,
 * init 0xFFFF, no reflection, xorout 0x0000, check 0x29B1.
 */
#ifndef OCB_PROTO_CRC16_H
#define OCB_PROTO_CRC16_H

#include <stddef.h>
#include <stdint.h>

/* CRC of `len` bytes, starting from OCB_CRC16_INIT. `data` may be NULL when
 * `len` is 0. */
uint16_t ocb_crc16(const uint8_t *data, size_t len);

/* Continue a CRC over more bytes (start with OCB_CRC16_INIT). */
uint16_t ocb_crc16_update(uint16_t crc, const uint8_t *data, size_t len);

#endif /* OCB_PROTO_CRC16_H */
