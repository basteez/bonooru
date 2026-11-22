/*
 * SPDX-License-Identifier: MIT
 *
 * TX gate (AD-20, story 2.6 D15): the controller must not drive its TX pad
 * until the peer is powered, which it learns from its own RX line being
 * idle-high. Pure state machine, closed -> open:
 *
 *  - starts closed;
 *  - ocb_txgate_sample(): opens once RX has been high continuously for
 *    OCB_LINK_TX_IDLE_HIGH_US; a low sample while closed restarts the wait;
 *    once open, low samples (data bits, a break) do not close it;
 *  - ocb_txgate_release(): closes at once (Story 2.7 calls it on a break
 *    and on link loss); a new 1 ms idle-high run is needed to reopen.
 *
 * The glue switches the TX pad between the UART function and a high-Z
 * input to follow ocb_txgate_is_open().
 */
#ifndef OCB_LINK_TXGATE_H
#define OCB_LINK_TXGATE_H

#include <stdbool.h>
#include <stdint.h>

#include "ocb_core/ramfunc.h"

typedef struct {
    bool open;
    bool high_run;         /* closed: RX seen high since high_since_us */
    uint64_t high_since_us;
} ocb_txgate;

void ocb_txgate_init(ocb_txgate *g);

/* Feeds one RX level sample; returns whether the gate is open after it. */
OCB_CORE_RAMFUNC bool ocb_txgate_sample(ocb_txgate *g, bool rx_high, uint64_t now_us);

void ocb_txgate_release(ocb_txgate *g);

OCB_CORE_RAMFUNC bool ocb_txgate_is_open(const ocb_txgate *g);

#endif /* OCB_LINK_TXGATE_H */
