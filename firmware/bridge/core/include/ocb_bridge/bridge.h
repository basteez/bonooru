/*
 * SPDX-License-Identifier: MIT
 *
 * The bridge's per-pass logic (Story 3.1, AD-4, AD-20), pure C: the platform
 * feeds it CDC bytes, UART data-register values, heartbeat alarm ticks, the
 * DTR line and the RX pad level, and asks it what to write to the UART and
 * to CDC and whether TX must be driven. Nothing here blocks.
 *
 * PC -> UART: one app frame in flight (being written to the UART) plus one
 * queued (in the framer); a due heartbeat goes at the next frame boundary,
 * ahead of the queued app frame. Data and heartbeats flow only while the
 * probe state machine says LINKED.
 */
#ifndef OCB_BRIDGE_BRIDGE_H
#define OCB_BRIDGE_BRIDGE_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "ocb_bridge/framer.h"
#include "ocb_bridge/probe.h"
#include "ocb_bridge/rxfwd.h"
#include "ocb_proto/consts.h"
#include "ocb_proto/frame.h"

typedef struct {
    uint32_t hb_sent;            /* heartbeats put in flight */
    uint32_t hb_dropped;         /* alarm ticks while not linked */
    uint32_t inflight_abandoned; /* frames cut by a link loss */
    uint32_t app_frames;         /* app frames put in flight */
} ocb_bridge_stats;

typedef struct {
    ocb_bridge_framer framer;
    ocb_bridge_probe probe;
    ocb_bridge_rxfwd rx;
    ocb_rx rxsplit; /* controller frames, to see "a valid frame" while probing */
    uint8_t inflight[OCB_MAX_WIRE_FRAME];
    size_t inflight_len;
    size_t inflight_pos;
    bool inflight_hb;
    uint8_t ver[3];
    bool dtr;
    bool dtr_fell; /* DTR fell since the last heartbeat: the next one says 0 */
    bool need_delim; /* a lone 0x00 goes first after a link-up (resync) */
    uint8_t hb_seq;
    uint32_t hb_ticks_seen;
    bool hb_due;
    bool tx_enabled;
    ocb_bridge_stats stats;
} ocb_bridge;

/* `ver` is the bridge firmware version sent in every heartbeat. */
void ocb_bridge_init(ocb_bridge *b, const uint8_t ver[3]);

/* PC -> bridge bytes; returns how many were consumed (keep the rest). */
size_t ocb_bridge_cdc_in(ocb_bridge *b, const uint8_t *data, size_t n);

/* The CDC DTR line. A fall is latched until the next heartbeat, so even a
 * fall shorter than one period reaches the controller as DTR 0. Dropping
 * the partial frame is the platform's call (ocb_bridge_cut_partial), once
 * everything the PC sent before the fall has been fed (review fix I-5). */
void ocb_bridge_set_dtr(ocb_bridge *b, bool dtr);

/* Drops the partial run (the bytes after the last delimiter fed). */
void ocb_bridge_cut_partial(ocb_bridge *b);

/* CDC unmounted: drops the partial frame, DTR off. */
void ocb_bridge_cdc_unmount(ocb_bridge *b);

/* One UART data-register value (data byte and error bits). */
void ocb_bridge_uart_rx(ocb_bridge *b, uint16_t dr);

/* Bytes for CDC: up to `space`; queued bytes that do not fit are dropped. */
size_t ocb_bridge_cdc_out(ocb_bridge *b, uint8_t *out, size_t space);

/* The heartbeat alarm's free-running tick counter (every 20 ms). A change
 * makes one heartbeat due (several missed ticks: still one). */
void ocb_bridge_hb_ticks(ocb_bridge *b, uint32_t ticks);

/* The probe step with the RX pad level; returns whether TX must be driven.
 * A link loss abandons the frame in flight. */
bool ocb_bridge_step(ocb_bridge *b, uint64_t now_us, bool rx_high);

/* Bytes to append to the UART software ring now: at most `cap`, and only
 * while fewer than OCB_BRIDGE_UART_AHEAD_MAX bytes wait in it (`ring_used`).
 * Nothing while not linked. */
size_t ocb_bridge_uart_out(ocb_bridge *b, uint8_t *out, size_t cap, size_t ring_used);

#endif /* OCB_BRIDGE_BRIDGE_H */
