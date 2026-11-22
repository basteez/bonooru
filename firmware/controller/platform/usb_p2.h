/*
 * SPDX-License-Identifier: MIT
 *
 * TinyUSB glue of the P2 profile (story 2.3 D1, D11, D20), core 0 only
 * (AD-11: the only core that touches TinyUSB). It translates: descriptor
 * callbacks return the bytes of ocb_p2, OUT reports go to ocb_p2_on_output,
 * the IN pump sends ocb_p2_next_in, and the USB events restart the profile
 * and post to the USB mailbox that core 1's link reads. It adds no protocol
 * rule and aborts nothing itself.
 */
#ifndef OCB_CTRL_USB_P2_H
#define OCB_CTRL_USB_P2_H

/* Builds the profile (MAC from the board's unique id) and starts TinyUSB.
 * Call once in main() on core 0, after ocb_ctrl_usb_mailbox_init(). */
void usb_p2_init(void);

/* One pass of core 0's loop, never blocks: tud_task(), the USB-down events
 * seen by the interrupt hook, ocb_p2_tick(), the measured report period
 * into the mailbox, and the IN kick when the endpoint is free. */
void usb_p2_task(void);

#endif /* OCB_CTRL_USB_P2_H */
