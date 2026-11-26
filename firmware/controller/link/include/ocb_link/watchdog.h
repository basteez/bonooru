/*
 * SPDX-License-Identifier: MIT
 *
 * Link watchdog (AD-5, story 2.7 D1 to D12): the pure decision rules that
 * return the controller to NEUTRAL by itself when the bridge link or the
 * app disappears. Pure C99: no clock read, no I/O, no heap, no global
 * state, no 64-bit divide (Cortex-M0+). The link layer (link.c) feeds it
 * the time, the heartbeats, the app-frame time and the break counter, and
 * applies the verdicts (aborts, link_ok, TX release).
 *
 * Rules (docs/link-protocol.md, "Rules the controller link applies (Story
 * 2.7)"):
 *  - armed by the first valid HEARTBEAT (D3); before it nothing times out;
 *  - heartbeat timeout: armed and elapsed(now, last heartbeat) > link limit
 *    (default OCB_LINK_TIMEOUT_MS, clamped to OCB_LINK_TIMEOUT_MAX_MS, D4);
 *  - break: armed and a new BE entry since the last check (D5);
 *    a loss disarms until the next heartbeat (one episode per arming, D7);
 *  - break while not armed: TX release only, no abort, no trip (D8); this
 *    includes a break received before the arming heartbeat in the same loop
 *    pass (D39: the arming heartbeat takes the break counter as its
 *    baseline and owes the release);
 *  - app timeout: DIRECT only, elapsed(now, last app frame) > app limit
 *    (default OCB_APP_TIMEOUT_MS_DEFAULT, D9);
 *  - DTR falling edge between two consecutive valid heartbeats, DIRECT only
 *    (D10); the DTR memory is reset by a loss.
 * Every comparison is `elapsed > limit` (strictly greater: exactly the
 * limit is still alive, D4), with elapsed from ocb_wd_elapsed_us().
 *
 * Time must be monotonic (the glue passes the RP2040's 64-bit 1 MHz timer,
 * which never steps back and does not wrap in practice). If a caller ever
 * passes a time earlier than a stored stamp, elapsed is 0: no rule fires
 * until the clock has caught up with the stamp again (a timeout is
 * suppressed for that long, never fired early).
 *
 * Every pointer argument must be non-NULL. One owner per instance.
 */
#ifndef OCB_LINK_WATCHDOG_H
#define OCB_LINK_WATCHDOG_H

#include <stdbool.h>
#include <stdint.h>

#include "ocb_core/ramfunc.h"

/* Verdict bits of ocb_wd_check(). LINK_TIMEOUT and BREAK are a link loss
 * (at most one of the two per call, in that priority); APP_TIMEOUT is only
 * evaluated when no loss fired; RELEASE is a break seen while not armed
 * (TX release only). */
#define OCB_WD_LINK_TIMEOUT 0x01u
#define OCB_WD_BREAK 0x02u
#define OCB_WD_APP_TIMEOUT 0x04u
#define OCB_WD_RELEASE 0x08u
#define OCB_WD_LOSS (OCB_WD_LINK_TIMEOUT | OCB_WD_BREAK)

typedef struct {
    bool armed;             /* a valid heartbeat was seen since boot or the last loss */
    bool dtr_prev;          /* DTR bit of the previous heartbeat (false = unknown or low) */
    uint8_t trips;          /* rule firings, wraps (STATUS debug byte 2, D12) */
    uint16_t link_timeout_ms; /* effective values (D4, D9) */
    uint16_t app_timeout_ms;
    uint64_t link_limit_us; /* the same limits in us, computed once at init */
    uint64_t app_limit_us;
    uint64_t last_hb_us;    /* time of the last valid heartbeat */
    uint32_t breaks_seen;   /* the link's break counter at the last check or arming */
    bool release_owed;      /* a break before the arming heartbeat: release TX (D39) */
    /* Story 3.5: the last link loss, for the debug STATUS v5. The age is
     * the time from the last valid heartbeat to the verdict (the abort and
     * NEUTRAL happen in the same pass): an upper bound of "link gone to
     * neutral". kind: 0 none yet, OCB_WD_LINK_TIMEOUT or OCB_WD_BREAK. */
    uint32_t last_loss_age_us;
    uint8_t last_loss_kind;
} ocb_wd;

/* link_timeout_ms 0 selects OCB_LINK_TIMEOUT_MS; above OCB_LINK_TIMEOUT_MAX_MS
 * it is clamped to it. app_timeout_ms 0 selects OCB_APP_TIMEOUT_MS_DEFAULT.
 * breaks_now is the link's break counter at init. Starts disarmed. */
void ocb_wd_init(ocb_wd *wd, uint16_t link_timeout_ms, uint16_t app_timeout_ms, uint32_t breaks_now);

uint16_t ocb_wd_link_timeout_ms(const ocb_wd *wd);
uint16_t ocb_wd_app_timeout_ms(const ocb_wd *wd);

/* now - then when now >= then, else 0 (a clock that steps backwards or a
 * stale timestamp never yields a huge elapsed time, D1; the deadline is then
 * not reached until the clock passes `then` again). The only time
 * arithmetic of the watchdog and of the TX release deadline. */
OCB_CORE_RAMFUNC uint64_t ocb_wd_elapsed_us(uint64_t now_us, uint64_t then_us);

/* A valid HEARTBEAT was received at now_us with the given DTR bit while the
 * core was in `mode` (OCB_MODE_*); breaks_total is the link's BE entry
 * counter at that moment. Arms the watchdog. When it arms (was not armed),
 * breaks counted before it are not a loss: the counter becomes the
 * baseline, and a break among them still owes the TX release (returned as
 * OCB_WD_RELEASE by the next ocb_wd_check, D39). Returns true when it is a
 * DTR falling edge in DIRECT (the caller aborts with APP_TIMEOUT; the trip
 * is already counted). */
OCB_CORE_RAMFUNC bool ocb_wd_on_heartbeat(ocb_wd *wd, uint64_t now_us, bool dtr, uint8_t mode,
                                          uint32_t breaks_total);

/* One check per poll, before the scheduler. mode is the core mode,
 * last_app_frame_us / have_app_frame the app-frame time of admission step 1,
 * breaks_total the link's BE entry counter. Returns OCB_WD_* bits. A loss
 * disarms the watchdog and resets the DTR memory; every fired rule counts
 * one trip (RELEASE is not a rule firing). */
OCB_CORE_RAMFUNC uint8_t ocb_wd_check(ocb_wd *wd, uint64_t now_us, uint8_t mode, uint64_t last_app_frame_us,
                                      bool have_app_frame, uint32_t breaks_total);

/* Disarms and forgets the DTR bit (called by ocb_wd_check on a loss). */
OCB_CORE_RAMFUNC void ocb_wd_note_loss(ocb_wd *wd);

#endif /* OCB_LINK_WATCHDOG_H */
