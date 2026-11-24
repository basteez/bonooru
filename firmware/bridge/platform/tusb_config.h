/*
 * SPDX-License-Identifier: MIT
 *
 * TinyUSB configuration of the bridge (Story 3.1): device mode only, one CDC
 * interface. TinyUSB 0.18.0 from the pinned pico-sdk submodule (AD-1). The
 * 1200-baud BOOTSEL touch comes from pico_usb_reset (AD-13); no vendor
 * reset interface.
 */
#ifndef OCB_TUSB_CONFIG_H
#define OCB_TUSB_CONFIG_H

#ifndef CFG_TUSB_MCU
#define CFG_TUSB_MCU OPT_MCU_RP2040
#endif

#define CFG_TUSB_RHPORT0_MODE OPT_MODE_DEVICE
#define CFG_TUD_ENABLED 1
#define CFG_TUH_ENABLED 0

/* No TinyUSB log output: the UART pins carry the board link. */
#undef CFG_TUSB_DEBUG
#define CFG_TUSB_DEBUG 0

#define CFG_TUD_ENDPOINT0_SIZE 64

/* Classes: one CDC ACM interface (AD-13). */
#define CFG_TUD_CDC 1
#define CFG_TUD_HID 0
#define CFG_TUD_MSC 0
#define CFG_TUD_MIDI 0
#define CFG_TUD_VENDOR 0
#define CFG_TUD_AUDIO 0
#define CFG_TUD_VIDEO 0
#define CFG_TUD_DFU 0
#define CFG_TUD_DFU_RUNTIME 0
#define CFG_TUD_BTH 0
#define CFG_TUD_ECM_RNDIS 0
#define CFG_TUD_NCM 0
#define CFG_TUD_USBTMC 0

/* CDC FIFOs: the PC side of the backpressure (AD-4). */
#define CFG_TUD_CDC_RX_BUFSIZE 256
#define CFG_TUD_CDC_TX_BUFSIZE 256
#define CFG_TUD_CDC_EP_BUFSIZE 64

#endif /* OCB_TUSB_CONFIG_H */
