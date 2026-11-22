/*
 * SPDX-License-Identifier: MIT
 *
 * Controller link runner (story 2.6 D3): connects the pure ocb_link and
 * ocb_core to the UART driver of ocb_hw, on core 1. Story 2.5's core 1
 * super-loop keeps calling ocb_ctrl_link_step() once per pass.
 */
#ifndef OCB_CTRL_LINK_RUNNER_H
#define OCB_CTRL_LINK_RUNNER_H

#include "ocb_core/core.h"
#include "ocb_link/ready.h"

/* Builds the core (512-event static queue), the link and the TX gate, and
 * initialises the UART (TX high-Z, RX pull-down). Core 1. */
void ocb_ctrl_link_init(void);

/* Installs and enables the UART RX interrupt. Must run on core 1 (the
 * NVIC enable is per core). */
void ocb_ctrl_link_start_rx(void);

/* One pass, core 1 only, never blocks: RX level into the TX gate (TX pad
 * follows it), RX ring into the link, link poll (USB events, scheduler),
 * then at most one frame into the UART TX ring and a FIFO top-up. */
void ocb_ctrl_link_step(void);

/* The core, for Story 2.5 (core->published is read after each step). */
const ocb_core *ocb_ctrl_core(void);

/* The USB mailbox: core 0 posts to it (Story 2.3) with the
 * ocb_usb_mailbox_* functions; nothing posts yet, so console_ready is 0. */
ocb_usb_mailbox *ocb_ctrl_usb_mailbox(void);

#endif /* OCB_CTRL_LINK_RUNNER_H */
