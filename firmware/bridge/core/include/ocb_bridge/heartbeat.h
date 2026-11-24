/*
 * SPDX-License-Identifier: MIT
 *
 * The bridge heartbeat frame (AD-4, Story 3.1): HEARTBEAT (0x40), its own
 * rolling seq, body bridge_fw_ver[3] and flags (bit 0 = CDC DTR).
 */
#ifndef OCB_BRIDGE_HEARTBEAT_H
#define OCB_BRIDGE_HEARTBEAT_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/* Writes the whole wire frame (COBS, CRC, 0x00) into out; returns its
 * length, or 0 if cap is too small. */
size_t ocb_bridge_hb_build(uint8_t seq, const uint8_t ver[3], bool dtr, uint8_t *out, size_t cap);

#endif /* OCB_BRIDGE_HEARTBEAT_H */
