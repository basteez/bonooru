/*
 * SPDX-License-Identifier: MIT
 *
 * UART -> PC forwarder (AD-4, Story 3.1): clean bytes from the controller
 * wait in a ring and are handed to CDC as space allows; a byte is dropped
 * (and counted) only when the ring is full (best effort, never blocks). A received
 * value with FE, PE or OE is not data (dropped, counted); BE is a break
 * (counted, reported, never forwarded).
 */
#ifndef OCB_BRIDGE_RXFWD_H
#define OCB_BRIDGE_RXFWD_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "ocb_bridge/config.h"

/* Data-register error bits (RP2040 UARTDR: FE bit 8, PE 9, BE 10, OE 11). */
#define OCB_BRIDGE_DR_FE 0x100u
#define OCB_BRIDGE_DR_PE 0x200u
#define OCB_BRIDGE_DR_BE 0x400u
#define OCB_BRIDGE_DR_OE 0x800u

typedef struct {
    uint32_t forwarded;    /* bytes handed to CDC */
    uint32_t cdc_drops;    /* bytes dropped: the ring full (CDC had no space) */
    uint32_t err_bytes;    /* values with FE, PE or OE (not data) */
    uint32_t breaks;       /* values with BE */
} ocb_bridge_rxfwd_stats;

typedef struct {
    uint8_t buf[OCB_BRIDGE_RXFWD_CAP];
    uint32_t head;
    uint32_t tail;
    ocb_bridge_rxfwd_stats stats;
} ocb_bridge_rxfwd;

void ocb_bridge_rxfwd_init(ocb_bridge_rxfwd *r);

/* One data-register value. Returns 1 for a clean byte (stored in *byte),
 * 0 for an error value, -1 for a break. A clean byte is queued for CDC. */
int ocb_bridge_rxfwd_push(ocb_bridge_rxfwd *r, uint16_t dr, uint8_t *byte);

/* Copies up to `space` queued bytes to out (the CDC space this pass); the
 * rest waits. Returns the bytes copied. */
size_t ocb_bridge_rxfwd_take(ocb_bridge_rxfwd *r, uint8_t *out, size_t space);

#endif /* OCB_BRIDGE_RXFWD_H */
