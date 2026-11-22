/*
 * SPDX-License-Identifier: MIT
 *
 * CRC-16/CCITT-FALSE, bitwise (no table). Written from the published
 * parameters, no third-party code. Reference: Greg Cook, "Catalogue of
 * parametrised CRC algorithms", entry CRC-16/IBM-3740 (alias
 * CRC-16/CCITT-FALSE), https://reveng.sourceforge.io/crc-catalogue/16.htm
 */
#include "ocb_proto/crc16.h"

#include "ocb_proto/consts.h"

uint16_t ocb_crc16_update(uint16_t crc, const uint8_t *data, size_t len)
{
    for (size_t i = 0; i < len; i++) {
        crc = (uint16_t)(crc ^ (uint16_t)((uint16_t)data[i] << 8));
        for (unsigned bit = 0; bit < 8u; bit++) {
            if ((crc & 0x8000u) != 0u) {
                crc = (uint16_t)((uint16_t)(crc << 1) ^ (uint16_t)OCB_CRC16_POLY);
            } else {
                crc = (uint16_t)(crc << 1);
            }
        }
    }
    return crc;
}

uint16_t ocb_crc16(const uint8_t *data, size_t len)
{
    return ocb_crc16_update((uint16_t)OCB_CRC16_INIT, data, len);
}
