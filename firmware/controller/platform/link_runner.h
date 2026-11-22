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
#include "ocb_link/dbglog.h"
#include "ocb_link/ready.h"
#include "ocb_proto/state.h"

/* Zeroes the USB mailbox (and, in debug builds, the P2 debug log ring).
 * Call once in main() on core 0, before core 1 is launched and before
 * anything posts to it (story 2.3). */
void ocb_ctrl_usb_mailbox_init(void);

/* Builds the core (512-event static queue), the link and the TX gate, and
 * initialises the UART (TX high-Z, RX pull-down). Core 1. */
void ocb_ctrl_link_init(void);

/* Installs and enables the UART RX interrupt. Must run on core 1 (the
 * NVIC enable is per core). */
void ocb_ctrl_link_start_rx(void);

/* One pass, core 1 only, never blocks: RX level into the TX gate (TX pad
 * follows it), link poll (USB events, ready, scheduler), RX ring into the
 * link, then at most one frame into the UART TX ring and a FIFO top-up. */
void ocb_ctrl_link_step(void);

/* The core, for Story 2.5 (core->published is read after each step). */
const ocb_core *ocb_ctrl_core(void);

/* The USB mailbox: core 0 (the P2 glue, story 2.3) posts to it with the
 * ocb_usb_mailbox_* functions, any time after ocb_ctrl_usb_mailbox_init()
 * (posts made before core 1 initialises the link are kept, D28). */
ocb_usb_mailbox *ocb_ctrl_usb_mailbox(void);

#ifdef OCB_DEBUG_BUILD
/* The P2 debug log ring (story 2.3 D14): core 0 pushes, core 1 drains it
 * into the debug STATUS. Debug builds only. */
ocb_dbglog_ring *ocb_ctrl_dbglog(void);
#endif

/* The state the USB profile reports (AD-12 build_report input, story 2.3
 * D12, D35): in debug builds core 1's published state through a sequence
 * lock (NEUTRAL when no consistent copy in 3 tries); in release builds
 * NEUTRAL until Story 2.5. Core 0. */
void ocb_ctrl_published_read(ocb_state *out);

#endif /* OCB_CTRL_LINK_RUNNER_H */
