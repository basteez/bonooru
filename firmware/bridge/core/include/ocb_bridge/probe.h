/*
 * SPDX-License-Identifier: MIT
 *
 * AD-20 probe state machine (Story 3.1): the bridge leads. While RX does not
 * show the controller it drives TX idle-high for OCB_BRIDGE_PROBE_PULSE_US
 * every OCB_BRIDGE_PROBE_PERIOD_US and is high-Z in between; RX high for
 * OCB_BRIDGE_RX_HIGH_US (or a valid frame) means the controller drives its
 * TX: LINKED, TX stays enabled. A break, or RX low with no character for
 * more than OCB_BRIDGE_RX_LOW_US, is a link loss: back to PROBING, TX high-Z
 * at once and for OCB_BRIDGE_PROBE_RELEASE_US before the next pulse. A break
 * in a step wins over a valid frame in the same step.
 * Pure C, fake clock: the platform samples the RX pad once per pass.
 */
#ifndef OCB_BRIDGE_PROBE_H
#define OCB_BRIDGE_PROBE_H

#include <stdbool.h>
#include <stdint.h>

typedef enum { OCB_BRIDGE_PROBING, OCB_BRIDGE_LINKED } ocb_bridge_probe_state;

typedef struct {
    ocb_bridge_probe_state state;
    bool started;
    bool pulsing;
    uint64_t next_pulse_us;
    uint64_t pulse_end_us;
    bool high_valid;
    uint64_t high_since_us;
    bool low_valid;
    uint64_t low_since_us;
    bool break_seen;
    bool frame_seen;
    bool activity;      /* a character was received since the last step */
    uint32_t pulses;    /* probe pulses started */
    uint32_t link_ups;  /* PROBING -> LINKED */
    uint32_t losses;    /* LINKED -> PROBING */
} ocb_bridge_probe;

void ocb_bridge_probe_init(ocb_bridge_probe *p);

/* A break character was received (BE). */
void ocb_bridge_probe_break(ocb_bridge_probe *p);

/* A valid frame from the controller was received. */
void ocb_bridge_probe_frame(ocb_bridge_probe *p);

/* Any character (clean or not) was received: the line is alive. */
void ocb_bridge_probe_activity(ocb_bridge_probe *p);

/* One pass at `now_us` with the RX pad level; returns whether TX must be
 * enabled (driven) after this pass. */
bool ocb_bridge_probe_step(ocb_bridge_probe *p, uint64_t now_us, bool rx_high);

/* TX may carry data (LINKED). */
bool ocb_bridge_probe_linked(const ocb_bridge_probe *p);

/* When the next pulse starts or ends (for a sleeping caller; informative). */
uint64_t ocb_bridge_probe_next_event_us(const ocb_bridge_probe *p);

#endif /* OCB_BRIDGE_PROBE_H */
