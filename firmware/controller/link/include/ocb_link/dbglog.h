/*
 * SPDX-License-Identifier: MIT
 *
 * Debug log of the USB profile (story 2.3 D14, AC5a): core 0 pushes one
 * record per USB request into an SPSC ring (AD-11: debug logs go core 0 to
 * core 1, drop when full, never block); core 1 drains it into a window of
 * the newest 16 records, which the debug STATUS carries as its v3 tail
 * (docs/link-protocol.md). Debug builds only: the firmware creates the ring,
 * pushes and drains behind OCB_DEBUG_BUILD, so none of the functions below
 * is linked into a release ELF (tools/check-release-nolog.sh).
 *
 * Record: a uint16_t, kind in the high byte, id in the low byte.
 */
#ifndef OCB_LINK_DBGLOG_H
#define OCB_LINK_DBGLOG_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/* Ring entries, a power of two (index masking). */
#define OCB_DBGLOG_CAP 64u
/* Records kept in the STATUS window. */
#define OCB_DBGLOG_WINDOW 16u

/* Kinds (the same numbers as OCB_P2_LOG_* in ocb_p2/config.h). */
#define OCB_DBGLOG_USB_CMD 0x01u      /* id: the 0x80 command */
#define OCB_DBGLOG_SUBCMD 0x02u       /* id: the subcommand */
#define OCB_DBGLOG_OTHER_REPORT 0x03u /* id: another output (or feature) report id */
#define OCB_DBGLOG_REFUSAL 0x04u      /* id: the refused command, subcommand or report */
#define OCB_DBGLOG_OVERFLOW 0x05u     /* id: records dropped by a full ring (saturated at 255) */
#define OCB_DBGLOG_HANDSHAKE 0x06u    /* id: 0; the profile became ready */
#define OCB_DBGLOG_RESTART 0x07u      /* id: the cause, OCB_DBGLOG_RESTART_* */

#define OCB_DBGLOG_RESTART_MOUNT 0x01u
#define OCB_DBGLOG_RESTART_UNMOUNT 0x02u
#define OCB_DBGLOG_RESTART_SUSPEND 0x03u
#define OCB_DBGLOG_RESTART_BUS_RESET 0x04u

/* SPSC ring: head and drops written by the producer (core 0) only, tail by
 * the consumer (core 1) only; aligned 32-bit loads and stores are atomic on
 * Cortex-M0+. */
typedef struct {
    volatile uint16_t buf[OCB_DBGLOG_CAP];
    volatile uint32_t head;  /* producer */
    volatile uint32_t tail;  /* consumer */
    volatile uint32_t drops; /* producer: records lost to a full ring */
} ocb_dbglog_ring;

/* Window of the newest records (core 1 only). */
typedef struct {
    uint16_t rec[OCB_DBGLOG_WINDOW]; /* circular, oldest at `start` */
    uint8_t start;
    uint8_t n;          /* 0..16 */
    uint16_t total;     /* records ever added, wraps at 65536 */
    uint32_t seen_drops; /* ring drops already reported */
} ocb_dbglog_window;

void ocb_dbglog_ring_init(ocb_dbglog_ring *r);
void ocb_dbglog_window_init(ocb_dbglog_window *w);

/* Core 0. Returns false (and counts a drop) when the ring is full; never
 * overwrites, never blocks. */
bool ocb_dbglog_push(ocb_dbglog_ring *r, uint8_t kind, uint8_t id);

/* Core 1. Returns false when the ring is empty. */
bool ocb_dbglog_pop(ocb_dbglog_ring *r, uint16_t *rec);

/* Core 1: pops every record into the window, then an OVERFLOW record when
 * the ring dropped some since the last drain. Returns the records added. */
uint32_t ocb_dbglog_drain(ocb_dbglog_window *w, ocb_dbglog_ring *r);

/* Adds one record to the window (the newest 16 are kept). */
void ocb_dbglog_window_add(ocb_dbglog_window *w, uint16_t rec);

#endif /* OCB_LINK_DBGLOG_H */
