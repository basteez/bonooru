/*
 * SPDX-License-Identifier: MIT
 *
 * Constants of the P2 profile (wired Pro Controller, story 2.3). Written
 * clean-room from public reverse-engineering documentation (D2); no code of
 * any source was used, and no 2wiCC code. Every value names its source:
 *
 *   S1  dekuNukem, Nintendo_Switch_Reverse_Engineering, USB-HID-Notes.md
 *   S2  same repository, bluetooth_hid_notes.md
 *   S3  same repository, bluetooth_hid_subcommands_notes.md
 *   S4  same repository, spi_flash_notes.md
 *   S5  Chromium, device/gamepad/nintendo_controller.cc
 *   S6  SDL, src/joystick/hidapi/SDL_hidapi_switch.c
 *   S7  mzyy94, "Nintendo Switch Pro Controller USB gadget" (2020-03-20)
 *   S8  tmk, lsusb -v dump of a Pro Controller (GitHub gist 9725596c...)
 *   S9  joycontrol (mart1nro), values only
 *   S12 mzyy94, bypass_procon_log.txt (gist 60ae253a..., the capture behind
 *       S7): every output report a real Switch 1 sent to a real wired Pro
 *       Controller during init, and the controller's real replies (data)
 *   S13 riking's Joy-Con replies to subcommand 0x01 in dekuNukem's issue #7
 *       (ack 81, id 01, one status byte; data)
 * Full links: docs/usb-controller-emulation.md, "P2 wire protocol".
 */
#ifndef OCB_P2_CONFIG_H
#define OCB_P2_CONFIG_H

/* USB identity (D3): exactly the real wired controller's. */
#define OCB_P2_VID 0x057Eu        /* S8 idVendor */
#define OCB_P2_PID 0x2009u        /* S8 idProduct */
#define OCB_P2_BCD_DEVICE 0x0210u /* S8 bcdDevice 2.10 */
#define OCB_P2_BCD_USB 0x0200u    /* S8 bcdUSB 2.00 */
#define OCB_P2_BCD_HID 0x0111u    /* S8 bcdHID 1.11 */
#define OCB_P2_EP0_SIZE 64u       /* S8 bMaxPacketSize0 */
#define OCB_P2_EP_IN 0x81u        /* S8 bEndpointAddress, EP 1 IN */
#define OCB_P2_EP_OUT 0x01u       /* S8 bEndpointAddress, EP 1 OUT */
#define OCB_P2_EP_SIZE 64u        /* S8 wMaxPacketSize 0x0040 */
#define OCB_P2_CONFIG_ATTR 0xA0u  /* S8 bmAttributes: bus powered, remote wakeup */
#define OCB_P2_CONFIG_POWER 0xFAu /* S8 MaxPower 500 mA (2 mA units) */

/* Endpoint interval (D4): the real device declares 8 (S8); ours defaults to
 * 1 ms, a build constant in 1..16 (OCB_P2_BINTERVAL in CMake). */
#define OCB_P2_BINTERVAL_REAL 8u
#define OCB_P2_BINTERVAL_DEFAULT 1u
#define OCB_P2_BINTERVAL_MIN 1u
#define OCB_P2_BINTERVAL_MAX 16u

/* Descriptor lengths. */
#define OCB_P2_DEVICE_DESC_LEN 18u  /* S8 bLength 18 */
#define OCB_P2_CONFIG_DESC_LEN 41u  /* S8 wTotalLength 0x0029 */
#define OCB_P2_REPORT_DESC_LEN 203u /* S7, S8 wDescriptorLength 203 */
#define OCB_P2_STRING_DESC_MAX 64u  /* bytes, the longest string fits */

/* String descriptors (S7 hex dump, S8): index 1, 2, 3. */
#define OCB_P2_STR_MANUFACTURER "Nintendo Co., Ltd."
#define OCB_P2_STR_PRODUCT "Pro Controller"
#define OCB_P2_STR_SERIAL "000000000001"
#define OCB_P2_LANGID 0x0409u /* S7 String Zero: 04 03 09 04 */

/* Report ids (S7 report descriptor, S8). Every report is 64 bytes on the
 * wire, the id byte included (S6: USB packets are 64 bytes zero padded). */
#define OCB_P2_REPORT_LEN 64u
#define OCB_P2_RID_IN_STANDARD 0x30u /* S2 INPUT 0x30, standard full mode */
#define OCB_P2_RID_IN_REPLY 0x21u    /* S2 INPUT 0x21, subcommand reply */
#define OCB_P2_RID_IN_USB 0x81u      /* S1 replies to 0x80 commands */
#define OCB_P2_RID_OUT_SUBCMD 0x01u  /* S2 OUTPUT 0x01, rumble and subcommand */
#define OCB_P2_RID_OUT_RUMBLE 0x10u  /* S2 OUTPUT 0x10, rumble only */
#define OCB_P2_RID_OUT_USB 0x80u     /* S1 USB commands */
#define OCB_P2_RID_OUT_82 0x82u      /* S7, S8: declared, meaning not documented */
#define OCB_P2_RID_OUT_MCU_FW 0x03u  /* S2 OUTPUT 0x03, NFC/IR MCU FW update packet: refused (D26) */

/* USB 0x80 commands (S1; S5 and S6 name them the same way). */
#define OCB_P2_USB_STATUS 0x01u     /* reply 81 01 00 <type> <MAC reversed> */
#define OCB_P2_USB_HANDSHAKE 0x02u  /* reply 81 02 */
#define OCB_P2_USB_BAUD 0x03u       /* reply 81 03 */
#define OCB_P2_USB_NO_TIMEOUT 0x04u /* no reply; USB HID only, no timeout */
#define OCB_P2_USB_TIMEOUT 0x05u    /* no reply; allow the timeout again */
#define OCB_P2_USB_RESET 0x06u      /* "possibly resets": refused (D9) */
#define OCB_P2_USB_UART_PRE 0x91u   /* UART pass-through: refused (D9) */
#define OCB_P2_USB_UART_ANY 0x92u   /* UART pass-through: refused (D9) */

/* 0x01 output report layout (S2): [1] counter, [2..9] rumble, [10] id,
 * [11..] arguments. */
#define OCB_P2_SUBCMD_ID_OFFSET 10u
#define OCB_P2_SUBCMD_ARG_OFFSET 11u

/* Subcommand ids (S3). */
#define OCB_P2_SC_BT_PAIRING 0x01u
#define OCB_P2_SC_DEVICE_INFO 0x02u
#define OCB_P2_SC_INPUT_MODE 0x03u
#define OCB_P2_SC_ELAPSED 0x04u
#define OCB_P2_SC_HCI_STATE 0x06u
#define OCB_P2_SC_RESET_PAIRING 0x07u
#define OCB_P2_SC_SHIPMENT 0x08u
#define OCB_P2_SC_SPI_READ 0x10u
#define OCB_P2_SC_SPI_WRITE 0x11u
#define OCB_P2_SC_SPI_ERASE 0x12u
#define OCB_P2_SC_MCU_RESET 0x20u
#define OCB_P2_SC_MCU_CONFIG 0x21u
#define OCB_P2_SC_MCU_STATE 0x22u
#define OCB_P2_SC_PLAYER_LIGHTS 0x30u
#define OCB_P2_SC_HOME_LIGHT 0x38u
#define OCB_P2_SC_IMU_ENABLE 0x40u
#define OCB_P2_SC_IMU_SENS 0x41u
#define OCB_P2_SC_VIBRATION 0x48u

#define OCB_P2_INPUT_MODE_FULL 0x30u     /* S3: standard full mode */
#define OCB_P2_MCU_STATE_UPDATE 0x02u    /* S3: "resume for update" */

/* Ack bytes of the 0x21 reply (S2: ACK has the MSB set, NACK is 00). */
#define OCB_P2_ACK 0x80u
#define OCB_P2_NACK 0x00u
#define OCB_P2_ACK_DEVICE_INFO 0x82u /* S9 */
#define OCB_P2_ACK_ELAPSED 0x83u     /* S9 */
#define OCB_P2_ACK_SPI_READ 0x90u    /* S3 "x9010 ack" */
#define OCB_P2_ACK_PAIRING 0x81u     /* S12: 21 .. 81 01 03 (story 2.3 D34); S13 */
#define OCB_P2_PAIRING_STATUS 0x03u  /* S12: the real controller's status byte */
#define OCB_P2_ACK_MCU_CONFIG 0xA0u  /* S3 "ACK xA0"; S12: A0 21 + 34 bytes */
#define OCB_P2_MCU_STATE_LEN 34u     /* S3 "34 bytes of data" */
#define OCB_P2_UNUSED_DATA 0x03u     /* S3: unused subcommands reply 80 <id> 03 */
#define OCB_P2_WRITE_PROTECTED 0x01u /* S3: 0x11 and 0x12 status, write protected */

/* 0x21 reply layout (S2): [13] ack, [14] id, [15..49] data (35 bytes). */
#define OCB_P2_REPLY_ACK_OFFSET 13u
#define OCB_P2_REPLY_DATA_OFFSET 15u
#define OCB_P2_REPLY_DATA_MAX 35u

/* Device info (S3 subcommand 0x02; D10). */
#define OCB_P2_DEVICE_TYPE_PRO 0x03u /* S3, S4 0x6012: Pro Controller */
#define OCB_P2_FW_MAJOR 0x04u        /* S9 default; above S3's 3.89 (D10) */
#define OCB_P2_FW_MINOR 0x00u        /* S9 default */
#define OCB_P2_INFO_CONST_02 0x02u   /* S3 "always 02" */
#define OCB_P2_INFO_CONST_01 0x01u   /* S3 "always 01" */
#define OCB_P2_COLOUR_FLAG 0x00u     /* S3, S4 0x601B: default colours */

/* SPI flash reads (S3 subcommand 0x10, D7). */
#define OCB_P2_SPI_READ_MAX 0x1Du /* S3 "max size is x1D" */
#define OCB_P2_SPI_ERASED 0xFFu   /* S4: unused flash reads FF */

/* Report 0x30 fixed fields (D8, starting values from S7's capture; S2 for
 * their meaning). */
#define OCB_P2_BATTERY_CONN 0x91u  /* S2: battery full + charging, Pro, USB powered */
#define OCB_P2_SHARED_FIXED 0x80u  /* S2 "Charging Grip" bit, set in S7's sample */
#define OCB_P2_VIBRATOR 0x0Au      /* S7 sample byte 12 */
#define OCB_P2_STICK_CENTER 0x800u /* S9 calibration centre */
#define OCB_P2_STICK_SCALE 14u     /* D8: 0x80 steps * 14 = 0x700, the calibrated range */
#define OCB_P2_IMU_OFFSET 13u      /* S2: 6-axis data at 13..48 */
#define OCB_P2_IMU_LEN 36u

/* Reply queue (D6). */
#define OCB_P2_QUEUE_DEPTH 4u

/* IN interval measurement (D13). */
#define OCB_P2_IV_BIN_US 50u
#define OCB_P2_IV_MAX_US 20000u
#define OCB_P2_IV_BINS 401u /* bins 0..400: 50 us each, centred on k * 50 */
#define OCB_P2_IV_WINDOW 256u
#define OCB_P2_IV_MIN_SAMPLES 64u

/* Debug log record kinds (D14). The same numbers as OCB_DBGLOG_* in
 * ocb_link/dbglog.h (the glue checks that they agree at compile time). */
#define OCB_P2_LOG_USB_CMD 0x01u
#define OCB_P2_LOG_SUBCMD 0x02u
#define OCB_P2_LOG_OTHER_REPORT 0x03u
#define OCB_P2_LOG_REFUSAL 0x04u
#define OCB_P2_LOG_HANDSHAKE 0x06u
#define OCB_P2_LOG_RESTART 0x07u
#define OCB_P2_LOG_ARG 0x08u /* the first argument of 01, 03, 21, 22 (story 2.3 D34) */
/* 0x09 (repeat) is added by the link's window only (ocb_link/dbglog.h). */

#endif /* OCB_P2_CONFIG_H */
