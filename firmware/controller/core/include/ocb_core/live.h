/*
 * SPDX-License-Identifier: MIT
 *
 * ocb_live: the liveness rules of the two controller cores (AD-11, story
 * 2.5 D6, D7), as pure C99. Each core increments a pass counter once per
 * pass of its loop; the other core observes it with an ocb_live_peer and
 * the time of its own clock. The firmware glue (controller/platform/
 * liveness.c) owns the counters, the hardware watchdog and the scratch
 * registers; this file owns the decisions and the numbers.
 *
 * Rules:
 *  - a peer is stale when its counter has not changed for MORE than
 *    OCB_LIVE_STALE_US (exactly OCB_LIVE_STALE_US is still alive);
 *  - a counter wrap at 2^32 is a change, not a stall;
 *  - the first observation (ocb_live_peer_init) starts a grace window of
 *    the same length: a peer that never moves after it is stale too (the
 *    glue reports it as OCB_LIVE_CAUSE_BOOT_NOT_ALIVE);
 *  - the observer is not latched: a peer that moves again is alive again
 *    (the glue latches its own reaction: it reboots the board);
 *  - core 0 feeds the hardware watchdog only while core 1 is not stale;
 *    core 1 forces a reboot when core 0 is stale.
 *
 * Worst case from a stall to the reset: OCB_LIVE_STALE_US of detection,
 * then at most OCB_WDT_TIMEOUT_MS of hardware watchdog (core 1 stalled:
 * core 0 stops feeding), plus one loop pass: asserted at compile time
 * below to stay within the 100 ms of AC4 (NFR-05).
 */
#ifndef OCB_CORE_LIVE_H
#define OCB_CORE_LIVE_H

#include <stdbool.h>
#include <stdint.h>

/* A peer counter unchanged for more than this is stale (D6). */
#define OCB_LIVE_STALE_US 20000u
/* The hardware watchdog timeout, ms (D6: not 100, so detection plus the
 * timeout stays within 100 ms). */
#define OCB_WDT_TIMEOUT_MS 60u
/* The longest legitimate loop pass, us, with a safety factor (story 2.5
 * Task 11: to be confirmed by the measured core0_pass_max_us of H.18). */
#define OCB_LIVE_LOOP_BOUND_US 5000u
/* AC4: stall to reset, worst case. */
#define OCB_LIVE_WORST_CASE_US (OCB_LIVE_STALE_US + OCB_WDT_TIMEOUT_MS * 1000u + OCB_LIVE_LOOP_BOUND_US)
#define OCB_LIVE_REBOOT_LIMIT_US 100000u

typedef char ocb_live_worst_case_within_ac4[(OCB_LIVE_WORST_CASE_US <= OCB_LIVE_REBOOT_LIMIT_US) ? 1 : -1];

/* Reboot causes stored in a watchdog scratch register (D7). */
#define OCB_LIVE_CAUSE_NONE 0u
#define OCB_LIVE_CAUSE_CORE0_STALE 1u
#define OCB_LIVE_CAUSE_CORE1_STALE 2u
#define OCB_LIVE_CAUSE_BOOT_NOT_ALIVE 3u
#define OCB_LIVE_CAUSE_MAX 3u

/* Scratch word encoding: magic in the high 16 bits, bits 8..15 zero, the
 * value (a cause, or a reboot count) in the low 8 bits. */
#define OCB_LIVE_SCRATCH_MAGIC 0xB0E5u

typedef struct {
    uint32_t last_counter;   /* the counter value last seen */
    uint64_t last_change_us; /* observer time of the last change (or of init) */
    bool moved;              /* the counter changed at least once since init */
} ocb_live_peer;

/* First observation: counter as seen at now_us; starts the grace window. */
void ocb_live_peer_init(ocb_live_peer *p, uint32_t counter, uint64_t now_us);

/* One observation. Returns true when the counter has not changed for more
 * than OCB_LIVE_STALE_US. A clock that went back counts as no time. */
bool ocb_live_peer_stale(ocb_live_peer *p, uint32_t counter, uint64_t now_us);

/* The counter changed at least once since init (a stale peer that never
 * moved is OCB_LIVE_CAUSE_BOOT_NOT_ALIVE). */
bool ocb_live_peer_moved(const ocb_live_peer *p);

/* Core 0's rule: feed the hardware watchdog only while core 1 is fresh. */
bool ocb_live_core0_may_feed(bool core1_stale);

/* Core 1's rule: force a reboot when core 0 is stale. */
bool ocb_live_core1_must_reboot(bool core0_stale);

/* The cause core 0 records when core 1 is stale. */
uint8_t ocb_live_core1_cause(const ocb_live_peer *core1);

/* Scratch encoding of a value 0..255 (a cause or a count). */
uint32_t ocb_live_scratch_encode(uint8_t value);
/* Decodes a scratch word; a word without the magic, with bits 8..15 set,
 * decodes to 0 (no cause, count 0). */
uint8_t ocb_live_scratch_decode(uint32_t word);

/* Story 2.5 D31: the device-side stall-to-reset interval, us, stored in a
 * third scratch word before the reset: magic in the high 16 bits, the
 * interval in units of OCB_LIVE_SPAN_UNIT_US in the low 16 (saturating at
 * 65535 units, 655 ms). Decodes to 0 without the magic. */
#define OCB_LIVE_SPAN_UNIT_US 10u
uint32_t ocb_live_span_encode(uint32_t span_us);
uint32_t ocb_live_span_decode(uint32_t word);

/* The observer's time since the peer's counter last changed (or since
 * init), us; 0 when the clock went back. */
uint64_t ocb_live_peer_since_us(const ocb_live_peer *p, uint64_t now_us);

/* Story 2.5 D34: the stall-to-reset interval when core 1 stalled and core 0
 * stopped feeding the hardware watchdog: the watchdog fires
 * OCB_WDT_TIMEOUT_MS after the last feed (the SDK doubles the load for
 * RP2040-E1, so the programmed time is the real one), so the interval from
 * core 1's last pass (as core 0 saw it) to the reset is
 * last_feed + OCB_WDT_TIMEOUT_MS * 1000 - peer_last_change, us, 0 if
 * negative, saturating at UINT32_MAX. Both times on core 0's clock. The
 * watchdog's own TIME field is not used: on the RP2040 it does not hold the
 * remaining time (pico-sdk watchdog.h, watchdog_get_time_remaining_us). */
uint32_t ocb_live_watchdog_stall_to_reset_us(uint64_t peer_last_change_us, uint64_t last_feed_us);

/* Cause helpers: decode refuses values above OCB_LIVE_CAUSE_MAX (NONE). */
uint32_t ocb_live_cause_encode(uint8_t cause);
uint8_t ocb_live_cause_decode(uint32_t word);

#endif /* OCB_CORE_LIVE_H */
