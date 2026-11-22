/*
 * SPDX-License-Identifier: MIT
 *
 * P2 USB descriptors (story 2.3 D3). Interoperability data: the values of
 * a real wired Pro Controller as published in two independent hardware
 * dumps, S7 (usbhid-dump of the report descriptor, device and string hex)
 * and S8 (lsusb -v, the same values decoded), cross-checked against each
 * other. Re-expressed here as commented items; no source code was copied.
 */
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include "ocb_p2/config.h"
#include "ocb_p2/desc.h"

#define LO(v) ((uint8_t)((v) & 0xFFu))
#define HI(v) ((uint8_t)(((v) >> 8) & 0xFFu))

/* S8 "Device Descriptor" and S8's hex line 12 01 00 02 ... 03 01. */
const uint8_t ocb_p2_device_desc[OCB_P2_DEVICE_DESC_LEN] = {
    18u,                                     /* bLength (S8) */
    0x01u,                                   /* bDescriptorType: DEVICE */
    LO(OCB_P2_BCD_USB), HI(OCB_P2_BCD_USB), /* bcdUSB 2.00 (S8) */
    0x00u,                                   /* bDeviceClass: per interface (S8) */
    0x00u,                                   /* bDeviceSubClass (S8) */
    0x00u,                                   /* bDeviceProtocol (S8) */
    OCB_P2_EP0_SIZE,                         /* bMaxPacketSize0 64 (S8) */
    LO(OCB_P2_VID), HI(OCB_P2_VID),         /* idVendor 057E (S8) */
    LO(OCB_P2_PID), HI(OCB_P2_PID),         /* idProduct 2009 (S8) */
    LO(OCB_P2_BCD_DEVICE), HI(OCB_P2_BCD_DEVICE), /* bcdDevice 2.10 (S8) */
    0x01u,                                   /* iManufacturer (S8) */
    0x02u,                                   /* iProduct (S8) */
    0x03u,                                   /* iSerialNumber (S8) */
    0x01u,                                   /* bNumConfigurations (S8) */
};

/* S7 usbhid-dump, item by item; S8 shows the same items decoded. One
 * application collection; report 0x30 is declared as 14 buttons, 4 x 16-bit
 * axes, a 4-bit hat, 4 more buttons and 52 constant bytes (63 bytes after
 * the id), although the real 0x30 payload has the layout of S2 (S7 notes
 * the mismatch; the console uses S2's layout). The vendor reports 0x21,
 * 0x81 (in) and 0x01, 0x10, 0x80, 0x82 (out) are 63 bytes each. */
const uint8_t ocb_p2_report_desc[OCB_P2_REPORT_DESC_LEN] = {
    0x05u, 0x01u,                   /* Usage Page (Generic Desktop) */
    0x15u, 0x00u,                   /* Logical Minimum (0) */
    0x09u, 0x04u,                   /* Usage (Joystick) */
    0xA1u, 0x01u,                   /* Collection (Application) */
    0x85u, 0x30u,                   /*   Report ID (0x30) */
    0x05u, 0x01u,                   /*   Usage Page (Generic Desktop) */
    0x05u, 0x09u,                   /*   Usage Page (Button) */
    0x19u, 0x01u,                   /*   Usage Minimum (1) */
    0x29u, 0x0Au,                   /*   Usage Maximum (10) */
    0x15u, 0x00u,                   /*   Logical Minimum (0) */
    0x25u, 0x01u,                   /*   Logical Maximum (1) */
    0x75u, 0x01u,                   /*   Report Size (1) */
    0x95u, 0x0Au,                   /*   Report Count (10) */
    0x55u, 0x00u,                   /*   Unit Exponent (0) */
    0x65u, 0x00u,                   /*   Unit (None) */
    0x81u, 0x02u,                   /*   Input (Data, Variable, Absolute) */
    0x05u, 0x09u,                   /*   Usage Page (Button) */
    0x19u, 0x0Bu,                   /*   Usage Minimum (11) */
    0x29u, 0x0Eu,                   /*   Usage Maximum (14) */
    0x15u, 0x00u,                   /*   Logical Minimum (0) */
    0x25u, 0x01u,                   /*   Logical Maximum (1) */
    0x75u, 0x01u,                   /*   Report Size (1) */
    0x95u, 0x04u,                   /*   Report Count (4) */
    0x81u, 0x02u,                   /*   Input (Data, Variable, Absolute) */
    0x75u, 0x01u,                   /*   Report Size (1) */
    0x95u, 0x02u,                   /*   Report Count (2) */
    0x81u, 0x03u,                   /*   Input (Constant, Variable, Absolute) */
    0x0Bu, 0x01u, 0x00u, 0x01u, 0x00u, /* Usage (Generic Desktop: Pointer), 4-byte form */
    0xA1u, 0x00u,                   /*   Collection (Physical) */
    0x0Bu, 0x30u, 0x00u, 0x01u, 0x00u, /*   Usage (X) */
    0x0Bu, 0x31u, 0x00u, 0x01u, 0x00u, /*   Usage (Y) */
    0x0Bu, 0x32u, 0x00u, 0x01u, 0x00u, /*   Usage (Z) */
    0x0Bu, 0x35u, 0x00u, 0x01u, 0x00u, /*   Usage (Rz) */
    0x15u, 0x00u,                   /*     Logical Minimum (0) */
    0x27u, 0xFFu, 0xFFu, 0x00u, 0x00u, /*  Logical Maximum (65535) */
    0x75u, 0x10u,                   /*     Report Size (16) */
    0x95u, 0x04u,                   /*     Report Count (4) */
    0x81u, 0x02u,                   /*     Input (Data, Variable, Absolute) */
    0xC0u,                          /*   End Collection */
    0x0Bu, 0x39u, 0x00u, 0x01u, 0x00u, /* Usage (Hat switch) */
    0x15u, 0x00u,                   /*   Logical Minimum (0) */
    0x25u, 0x07u,                   /*   Logical Maximum (7) */
    0x35u, 0x00u,                   /*   Physical Minimum (0) */
    0x46u, 0x3Bu, 0x01u,            /*   Physical Maximum (315) */
    0x65u, 0x14u,                   /*   Unit (English Rotation: Degrees) */
    0x75u, 0x04u,                   /*   Report Size (4) */
    0x95u, 0x01u,                   /*   Report Count (1) */
    0x81u, 0x02u,                   /*   Input (Data, Variable, Absolute) */
    0x05u, 0x09u,                   /*   Usage Page (Button) */
    0x19u, 0x0Fu,                   /*   Usage Minimum (15) */
    0x29u, 0x12u,                   /*   Usage Maximum (18) */
    0x15u, 0x00u,                   /*   Logical Minimum (0) */
    0x25u, 0x01u,                   /*   Logical Maximum (1) */
    0x75u, 0x01u,                   /*   Report Size (1) */
    0x95u, 0x04u,                   /*   Report Count (4) */
    0x81u, 0x02u,                   /*   Input (Data, Variable, Absolute) */
    0x75u, 0x08u,                   /*   Report Size (8) */
    0x95u, 0x34u,                   /*   Report Count (52) */
    0x81u, 0x03u,                   /*   Input (Constant, Variable, Absolute) */
    0x06u, 0x00u, 0xFFu,            /*   Usage Page (Vendor 0xFF00) */
    0x85u, 0x21u,                   /*   Report ID (0x21) */
    0x09u, 0x01u,                   /*   Usage (1) */
    0x75u, 0x08u,                   /*   Report Size (8) */
    0x95u, 0x3Fu,                   /*   Report Count (63) */
    0x81u, 0x03u,                   /*   Input (Constant, Variable, Absolute) */
    0x85u, 0x81u,                   /*   Report ID (0x81) */
    0x09u, 0x02u,                   /*   Usage (2) */
    0x75u, 0x08u,                   /*   Report Size (8) */
    0x95u, 0x3Fu,                   /*   Report Count (63) */
    0x81u, 0x03u,                   /*   Input (Constant, Variable, Absolute) */
    0x85u, 0x01u,                   /*   Report ID (0x01) */
    0x09u, 0x03u,                   /*   Usage (3) */
    0x75u, 0x08u,                   /*   Report Size (8) */
    0x95u, 0x3Fu,                   /*   Report Count (63) */
    0x91u, 0x83u,                   /*   Output (Constant, Variable, Absolute, Volatile) */
    0x85u, 0x10u,                   /*   Report ID (0x10) */
    0x09u, 0x04u,                   /*   Usage (4) */
    0x75u, 0x08u,                   /*   Report Size (8) */
    0x95u, 0x3Fu,                   /*   Report Count (63) */
    0x91u, 0x83u,                   /*   Output (Constant, Variable, Absolute, Volatile) */
    0x85u, 0x80u,                   /*   Report ID (0x80) */
    0x09u, 0x05u,                   /*   Usage (5) */
    0x75u, 0x08u,                   /*   Report Size (8) */
    0x95u, 0x3Fu,                   /*   Report Count (63) */
    0x91u, 0x83u,                   /*   Output (Constant, Variable, Absolute, Volatile) */
    0x85u, 0x82u,                   /*   Report ID (0x82) */
    0x09u, 0x06u,                   /*   Usage (6) */
    0x75u, 0x08u,                   /*   Report Size (8) */
    0x95u, 0x3Fu,                   /*   Report Count (63) */
    0x91u, 0x83u,                   /*   Output (Constant, Variable, Absolute, Volatile) */
    0xC0u,                          /* End Collection */
};

size_t ocb_p2_config_desc(uint8_t binterval, uint8_t *out, size_t cap)
{
    if (out == NULL || cap < OCB_P2_CONFIG_DESC_LEN || binterval < OCB_P2_BINTERVAL_MIN ||
        binterval > OCB_P2_BINTERVAL_MAX) {
        return 0u;
    }
    /* S8 configuration hex 09 02 29 00 ... and its decoded fields. */
    const uint8_t d[OCB_P2_CONFIG_DESC_LEN] = {
        /* Configuration (S8) */
        9u, 0x02u, LO(OCB_P2_CONFIG_DESC_LEN), HI(OCB_P2_CONFIG_DESC_LEN), /* wTotalLength 41 */
        0x01u,               /* bNumInterfaces */
        0x01u,               /* bConfigurationValue */
        0x00u,               /* iConfiguration */
        OCB_P2_CONFIG_ATTR,  /* bmAttributes 0xA0 */
        OCB_P2_CONFIG_POWER, /* bMaxPower 500 mA */
        /* Interface 0 (S8): HID, no boot subclass, 2 endpoints */
        9u, 0x04u, 0x00u, 0x00u, 0x02u, 0x03u, 0x00u, 0x00u, 0x00u,
        /* HID descriptor (S8): bcdHID 1.11, country 0, 1 descriptor, report, 203 bytes */
        9u, 0x21u, LO(OCB_P2_BCD_HID), HI(OCB_P2_BCD_HID), 0x00u, 0x01u, 0x22u,
        LO(OCB_P2_REPORT_DESC_LEN), HI(OCB_P2_REPORT_DESC_LEN),
        /* Endpoint IN 0x81 (S8): interrupt, 64 bytes, bInterval (D4) */
        7u, 0x05u, OCB_P2_EP_IN, 0x03u, LO(OCB_P2_EP_SIZE), HI(OCB_P2_EP_SIZE), binterval,
        /* Endpoint OUT 0x01 (S8): interrupt, 64 bytes, bInterval (D4) */
        7u, 0x05u, OCB_P2_EP_OUT, 0x03u, LO(OCB_P2_EP_SIZE), HI(OCB_P2_EP_SIZE), binterval,
    };
    memcpy(out, d, sizeof d);
    return sizeof d;
}

size_t ocb_p2_string_desc(uint8_t index, uint8_t *out, size_t cap)
{
    if (out == NULL) {
        return 0u;
    }
    if (index == 0u) {
        /* S7 "String Zero": 04 03 09 04 (US English). */
        if (cap < 4u) {
            return 0u;
        }
        out[0] = 4u;
        out[1] = 0x03u;
        out[2] = LO(OCB_P2_LANGID);
        out[3] = HI(OCB_P2_LANGID);
        return 4u;
    }
    const char *s;
    switch (index) {
    case 1u:
        s = OCB_P2_STR_MANUFACTURER;
        break;
    case 2u:
        s = OCB_P2_STR_PRODUCT;
        break;
    case 3u:
        s = OCB_P2_STR_SERIAL;
        break;
    default:
        return 0u;
    }
    const size_t n = strlen(s);
    const size_t len = 2u + 2u * n;
    if (len > cap || len > 0xFFu) {
        return 0u;
    }
    out[0] = (uint8_t)len;
    out[1] = 0x03u; /* STRING */
    for (size_t i = 0; i < n; i++) {
        out[2u + 2u * i] = (uint8_t)s[i]; /* ASCII: UTF-16LE low byte */
        out[3u + 2u * i] = 0x00u;
    }
    return len;
}
