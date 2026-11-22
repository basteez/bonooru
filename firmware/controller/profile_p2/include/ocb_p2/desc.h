/*
 * SPDX-License-Identifier: MIT
 *
 * USB descriptors of the P2 profile (AD-12 "descriptors", story 2.3 D3,
 * D4): the bytes of a real wired Pro Controller (S7, S8 in config.h),
 * except bInterval, which is a build constant. Pure data and two small
 * builders; the TinyUSB glue (controller/platform/usb_p2.c) hands them to
 * the stack.
 */
#ifndef OCB_P2_DESC_H
#define OCB_P2_DESC_H

#include <stddef.h>
#include <stdint.h>

#include "ocb_p2/config.h"

/* Device descriptor, 18 bytes. */
extern const uint8_t ocb_p2_device_desc[OCB_P2_DEVICE_DESC_LEN];

/* HID report descriptor, 203 bytes. */
extern const uint8_t ocb_p2_report_desc[OCB_P2_REPORT_DESC_LEN];

/* Configuration descriptor (configuration, interface, HID, IN and OUT
 * endpoints; 41 bytes) with both endpoints at `binterval`. Returns 41, or 0
 * (nothing written) when out is NULL, cap < 41 or binterval is outside
 * OCB_P2_BINTERVAL_MIN..OCB_P2_BINTERVAL_MAX. */
size_t ocb_p2_config_desc(uint8_t binterval, uint8_t *out, size_t cap);

/* String descriptor `index` (0 = language list, 1 manufacturer, 2 product,
 * 3 serial) as bytes: bLength, 0x03, then UTF-16LE. Returns its length, or
 * 0 (nothing written) for an unknown index, a NULL out or a short cap. */
size_t ocb_p2_string_desc(uint8_t index, uint8_t *out, size_t cap);

#endif /* OCB_P2_DESC_H */
