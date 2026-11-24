/*
 * SPDX-License-Identifier: MIT
 *
 * USB descriptors of the bridge (Story 3.1, AD-13): one CDC ACM function,
 * VID:PID 2E8A:000A (the generic Raspberry Pi pair, Q1), manufacturer
 * "Bonooru", product "Bonooru Bridge", serial number = the flash unique ID
 * as 16 upper-case hex digits. Written for this firmware with TinyUSB's
 * descriptor macros (TinyUSB 0.18.0).
 */
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include "pico/unique_id.h"
#include "tusb.h"

#define OCB_BRIDGE_VID 0x2E8Au
#define OCB_BRIDGE_PID 0x000Au
/* bcdDevice: fixed (Story 3.1 D15); the firmware version travels in the
 * heartbeat and reaches the app as INFO.bridge_ver (AD-13). */
#define OCB_BRIDGE_BCD_DEVICE 0x0100u

enum { ITF_NUM_CDC = 0, ITF_NUM_CDC_DATA, ITF_NUM_TOTAL };
enum { STR_LANGID = 0, STR_MANUFACTURER, STR_PRODUCT, STR_SERIAL, STR_CDC, STR_COUNT };

#define EPNUM_CDC_NOTIF 0x81
#define EPNUM_CDC_OUT 0x02
#define EPNUM_CDC_IN 0x82

static const tusb_desc_device_t s_device = {
    .bLength = sizeof(tusb_desc_device_t),
    .bDescriptorType = TUSB_DESC_DEVICE,
    .bcdUSB = 0x0200,
    /* Interface Association Descriptor (CDC uses two interfaces). */
    .bDeviceClass = TUSB_CLASS_MISC,
    .bDeviceSubClass = MISC_SUBCLASS_COMMON,
    .bDeviceProtocol = MISC_PROTOCOL_IAD,
    .bMaxPacketSize0 = CFG_TUD_ENDPOINT0_SIZE,
    .idVendor = OCB_BRIDGE_VID,
    .idProduct = OCB_BRIDGE_PID,
    .bcdDevice = OCB_BRIDGE_BCD_DEVICE,
    .iManufacturer = STR_MANUFACTURER,
    .iProduct = STR_PRODUCT,
    .iSerialNumber = STR_SERIAL,
    .bNumConfigurations = 1,
};

#define CONFIG_TOTAL_LEN (TUD_CONFIG_DESC_LEN + TUD_CDC_DESC_LEN)

static const uint8_t s_config[] = {
    TUD_CONFIG_DESCRIPTOR(1, ITF_NUM_TOTAL, 0, CONFIG_TOTAL_LEN, 0x00, 100),
    TUD_CDC_DESCRIPTOR(ITF_NUM_CDC, STR_CDC, EPNUM_CDC_NOTIF, 8, EPNUM_CDC_OUT, EPNUM_CDC_IN, 64),
};

uint8_t const *tud_descriptor_device_cb(void)
{
    return (uint8_t const *)&s_device;
}

uint8_t const *tud_descriptor_configuration_cb(uint8_t index)
{
    (void)index;
    return s_config;
}

/* UTF-16LE string descriptor buffer: header + up to 31 characters. */
static uint16_t s_string[32];

uint16_t const *tud_descriptor_string_cb(uint8_t index, uint16_t langid)
{
    (void)langid;
    char serial[2u * PICO_UNIQUE_BOARD_ID_SIZE_BYTES + 1u];
    const char *str = NULL;
    size_t n = 0;
    switch (index) {
    case STR_LANGID:
        s_string[1] = 0x0409; /* English (United States) */
        n = 1;
        break;
    case STR_MANUFACTURER:
        str = "Bonooru";
        break;
    case STR_PRODUCT:
    case STR_CDC:
        str = "Bonooru Bridge";
        break;
    case STR_SERIAL:
        pico_get_unique_board_id_string(serial, sizeof serial);
        str = serial;
        break;
    default:
        return NULL;
    }
    if (str != NULL) {
        n = strlen(str);
        if (n > 31u) {
            n = 31u;
        }
        for (size_t i = 0; i < n; i++) {
            s_string[1u + i] = (uint16_t)(uint8_t)str[i];
        }
    }
    s_string[0] = (uint16_t)((TUSB_DESC_STRING << 8) | (2u * n + 2u));
    return s_string;
}
