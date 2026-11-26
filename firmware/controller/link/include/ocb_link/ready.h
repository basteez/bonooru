/*
 * SPDX-License-Identifier: MIT
 *
 * Console readiness (AD-19, story 2.6 D11): the pure `ready` predicate and
 * the cross-core mailbox that feeds it.
 *
 * Mailbox: core 0 (the USB stack, Story 2.3) posts events by incrementing
 * one counter each; every counter has a single writer (core 0) and core 1
 * only reads them. Cortex-M0+ has no atomic read-modify-write, but aligned
 * 32-bit loads and stores are atomic, so a single writer per counter is
 * enough. Core 1 keeps its own last-seen copies and compares; counters wrap
 * at 2^32 without a false event.
 *
 * Predicate, evaluated by core 1 with its own 64-bit clock:
 *   ready = handshake seen
 *        && no USB-down event (suspend, unmount, bus reset) since then
 *        && now - (core 1 time at which `in_reports` last changed)
 *              <= OCB_CONSOLE_POLL_STALL_MS * 1000
 * Exactly 100 ms without an IN report is still ready; 1 us more is not.
 * The stall clears the flag only (no abort, AD-19); a new IN report
 * restores it while the handshake stands. A USB-down event clears the
 * handshake: only a new handshake restores `ready`. A handshake restarts
 * the IN clock (grace period). When a down event and a handshake are seen
 * in the same poll, the down event is handled first.
 */
#ifndef OCB_LINK_READY_H
#define OCB_LINK_READY_H

#include <stdbool.h>
#include <stdint.h>

#include "ocb_core/ramfunc.h"

typedef struct {
    volatile uint32_t handshakes; /* core 0 only */
    volatile uint32_t in_reports; /* core 0 only */
    volatile uint32_t suspends;   /* core 0 only */
    volatile uint32_t unmounts;   /* core 0 only */
    volatile uint32_t bus_resets; /* core 0 only */
    /* Story 2.3 D13: the median IN interval measured by core 0, us; 0 =
     * unknown (fewer than 64 samples, or a USB-down event since). Not a
     * counter: core 1 applies it to INFO and the LATE threshold when it
     * changes (ocb_link_poll). */
    volatile uint32_t report_period_us; /* core 0 only */
    /* Story 2.5 D12, D13: core 0's self-timing for the debug STATUS v4
     * (not counters; each is one aligned store by core 0, read by core 1;
     * the fields are independent, a reader may see a mix of two updates,
     * which a debug report accepts). us unless stated. */
    volatile uint32_t in_min_us;         /* IN interval minimum of the window (0 = unknown) */
    volatile uint32_t in_p95_us;         /* IN interval 95th percentile of the window */
    volatile uint32_t in_max_us;         /* IN interval maximum of the window */
    volatile uint32_t in_slow;           /* intervals above 1.5 x the median since the restart */
    volatile uint32_t core0_pass_max_us; /* longest core 0 pass since the restart (or boot) */
    volatile uint32_t seq_retries;       /* published-state reads that needed a retry */
    volatile uint32_t seq_fallbacks;     /* published-state reads that fell back to NEUTRAL */
    /* Story 4.0: the P2 replies dropped because the profile's reply queue
     * was full, since boot (the profile's cnt.queue_full), for the debug
     * STATUS v6. A value, not an event counter: one aligned store by core
     * 0, core 1 only copies it into the STATUS. */
    volatile uint32_t p2_queue_full; /* core 0 only */
} ocb_usb_mailbox;

/* Zeroes the counters, the report period, the self-timing fields and
 * p2_queue_full. Call
 * once, before either core uses the mailbox. */
void ocb_usb_mailbox_init(ocb_usb_mailbox *m);

/* Core 0 (Story 2.3): one increment each. */
void ocb_usb_mailbox_handshake_done(ocb_usb_mailbox *m);
void ocb_usb_mailbox_in_report(ocb_usb_mailbox *m);
void ocb_usb_mailbox_suspend(ocb_usb_mailbox *m);
void ocb_usb_mailbox_unmount(ocb_usb_mailbox *m);
void ocb_usb_mailbox_bus_reset(ocb_usb_mailbox *m);
/* Core 0 (Story 2.3 D13): one aligned store. */
void ocb_usb_mailbox_set_report_period(ocb_usb_mailbox *m, uint32_t report_period_us);
/* Core 0 (Story 2.5 D12): the IN interval spread, one store per field. */
void ocb_usb_mailbox_set_in_spread(ocb_usb_mailbox *m, uint32_t min_us, uint32_t p95_us, uint32_t max_us,
                                   uint32_t slow);
/* Core 0 (Story 2.5 D4, D12): the longest core 0 pass and the
 * published-state reader's counters, one store per field. */
void ocb_usb_mailbox_set_core0(ocb_usb_mailbox *m, uint32_t pass_max_us, uint32_t seq_retries,
                               uint32_t seq_fallbacks);
/* Core 0 (Story 4.0): the profile's reply-queue overflows, one store. */
void ocb_usb_mailbox_set_p2_queue_full(ocb_usb_mailbox *m, uint32_t n);

/* USB-down events returned by ocb_ready_poll (a bit mask). */
#define OCB_READY_DOWN_SUSPEND 0x01u
#define OCB_READY_DOWN_UNMOUNT 0x02u
#define OCB_READY_DOWN_BUS_RESET 0x04u

/* Core 1 state. */
typedef struct {
    uint32_t seen_handshakes;
    uint32_t seen_in_reports;
    uint32_t seen_suspends;
    uint32_t seen_unmounts;
    uint32_t seen_bus_resets;
    bool handshake;            /* a handshake with no USB-down event after it */
    uint64_t last_in_change_us; /* core 1 time of the last IN-report change or handshake */
} ocb_ready;

/* Starts not ready; the current mailbox values are taken as already seen. */
void ocb_ready_init(ocb_ready *r, const ocb_usb_mailbox *m, uint64_t now_us);

/* Reads the mailbox, updates the state and returns the USB-down events
 * seen since the last poll (OCB_READY_DOWN_* bits, 0 if none). */
OCB_CORE_RAMFUNC uint8_t ocb_ready_poll(ocb_ready *r, const ocb_usb_mailbox *m, uint64_t now_us);

/* The predicate at `now_us` (no mailbox read). */
OCB_CORE_RAMFUNC bool ocb_ready_is(const ocb_ready *r, uint64_t now_us);

#endif /* OCB_LINK_READY_H */
