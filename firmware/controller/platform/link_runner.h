/*
 * SPDX-License-Identifier: MIT
 *
 * Controller link runner (story 2.6 D3): connects the pure ocb_link and
 * ocb_core to the UART driver of ocb_hw, on core 1. The core 1 super-loop
 * of story 2.5 (main.c) calls ocb_ctrl_link_step() once per pass.
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

/* The core (core->published is read after each step). */
const ocb_core *ocb_ctrl_core(void);

/* Core 1, story 2.5 D6: core 0 is stale. Aborts the core with
 * OCB_ABORT_CORE_STALL (NEUTRAL, latch, no EVENT) and publishes NEUTRAL. */
void ocb_ctrl_link_abort_stall(void);

#ifdef OCB_DEBUG_BUILD
/* Core 1, debug builds, at the end of each super-loop pass (story 2.5
 * Task 11, D13, D15): the pass time since pass_start_us (a 32-bit
 * microsecond read at the start of the pass) into the loop maximum (reset
 * at START), one step of the stack mark scan, both into the STATUS v4. */
void ocb_ctrl_link_pass_end(uint32_t pass_start_us);
#endif

/* The USB mailbox: core 0 (the P2 glue, story 2.3) posts to it with the
 * ocb_usb_mailbox_* functions, any time after ocb_ctrl_usb_mailbox_init()
 * (posts made before core 1 initialises the link are kept, D28). */
ocb_usb_mailbox *ocb_ctrl_usb_mailbox(void);

#ifdef OCB_DEBUG_BUILD
/* The P2 debug log ring (story 2.3 D14): core 0 pushes, core 1 drains it
 * into the debug STATUS. Debug builds only. */
ocb_dbglog_ring *ocb_ctrl_dbglog(void);
#endif

/* The state the USB profile reports (AD-12 build_report input; story 2.3
 * D12; story 2.5 D4): core 1's published state through the seqlock double
 * buffer, NEUTRAL when no consistent copy was read in 3 tries. Every
 * build. Core 0. */
void ocb_ctrl_published_read(ocb_state *out);

/* Core 0: the reader's counters of the handoff (story 2.5 D4). */
uint32_t ocb_ctrl_published_retries(void);
uint32_t ocb_ctrl_published_fallbacks(void);

#endif /* OCB_CTRL_LINK_RUNNER_H */
