/*
 * SPDX-License-Identifier: MIT
 *
 * TinyUSB configuration of the controller (story 2.3 Task 8.2): device mode
 * only, one HID interface (the P2 profile), 64-byte reports. Written for
 * this firmware (the shape follows the TinyUSB documentation of
 * tusb_config.h); TinyUSB 0.18.0 from the pinned pico-sdk submodule (AD-1).
 */
#ifndef OCB_TUSB_CONFIG_H
#define OCB_TUSB_CONFIG_H

#ifndef CFG_TUSB_MCU
#define CFG_TUSB_MCU OPT_MCU_RP2040
#endif

/* Device stack on the only root port; no host stack. */
#define CFG_TUSB_RHPORT0_MODE OPT_MODE_DEVICE
#define CFG_TUD_ENABLED 1
#define CFG_TUH_ENABLED 0

/* No TinyUSB log output: the UART pins carry the board link. The SDK sets
 * CFG_TUSB_DEBUG on the command line in Debug builds; this file overrides
 * it for every TinyUSB source (it is included by tusb_option.h). */
#undef CFG_TUSB_DEBUG
#define CFG_TUSB_DEBUG 0

#define CFG_TUD_ENDPOINT0_SIZE 64

/* Classes: HID only (P2 profile, AD-12). */
#define CFG_TUD_HID 1
#define CFG_TUD_CDC 0
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

/* Every P2 report is 64 bytes, the report id included. */
#define CFG_TUD_HID_EP_BUFSIZE 64

#endif /* OCB_TUSB_CONFIG_H */
