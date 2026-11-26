/*
 * SPDX-License-Identifier: MIT
 *
 * Link watchdog rules (AD-5, story 2.7 D1 to D12). See ocb_link/watchdog.h.
 */
#include <stdbool.h>
#include <stdint.h>

#include "ocb_link/watchdog.h"
#include "ocb_proto/consts.h"

void ocb_wd_init(ocb_wd *wd, uint16_t link_timeout_ms, uint16_t app_timeout_ms, uint32_t breaks_now)
{
    uint16_t link_ms = link_timeout_ms == 0u ? (uint16_t)OCB_LINK_TIMEOUT_MS : link_timeout_ms;
    if (link_ms > (uint16_t)OCB_LINK_TIMEOUT_MAX_MS) {
        link_ms = (uint16_t)OCB_LINK_TIMEOUT_MAX_MS; /* NFR-05 cap (D4) */
    }
    const uint16_t app_ms = app_timeout_ms == 0u ? (uint16_t)OCB_APP_TIMEOUT_MS_DEFAULT : app_timeout_ms;
    wd->armed = false;
    wd->dtr_prev = false;
    wd->trips = 0u;
    wd->link_timeout_ms = link_ms;
    wd->app_timeout_ms = app_ms;
    /* Converted once, with a multiply (no 64-bit divide anywhere, D1). */
    wd->link_limit_us = (uint64_t)link_ms * 1000u;
    wd->app_limit_us = (uint64_t)app_ms * 1000u;
    wd->last_hb_us = 0u;
    wd->breaks_seen = breaks_now;
    wd->release_owed = false;
    wd->last_loss_age_us = 0u;
    wd->last_loss_kind = 0u;
}

uint16_t ocb_wd_link_timeout_ms(const ocb_wd *wd)
{
    return wd->link_timeout_ms;
}

uint16_t ocb_wd_app_timeout_ms(const ocb_wd *wd)
{
    return wd->app_timeout_ms;
}

OCB_CORE_RAMFUNC uint64_t ocb_wd_elapsed_us(uint64_t now_us, uint64_t then_us)
{
    return now_us >= then_us ? now_us - then_us : 0u;
}

static OCB_CORE_RAMFUNC void count_trip(ocb_wd *wd)
{
    wd->trips = (uint8_t)(wd->trips + 1u);
}

OCB_CORE_RAMFUNC void ocb_wd_note_loss(ocb_wd *wd)
{
    wd->armed = false;
    wd->dtr_prev = false;
}

OCB_CORE_RAMFUNC bool ocb_wd_on_heartbeat(ocb_wd *wd, uint64_t now_us, bool dtr, uint8_t mode,
                                          uint32_t breaks_total)
{
    if (!wd->armed) {
        /* D39: a break drained before the arming heartbeat (same loop pass)
         * happened while the link was down: release only, never a loss. */
        if (breaks_total != wd->breaks_seen) {
            wd->release_owed = true;
        }
        wd->breaks_seen = breaks_total;
    }
    const bool edge = wd->dtr_prev && !dtr && mode == (uint8_t)OCB_MODE_DIRECT;
    wd->dtr_prev = dtr;
    wd->armed = true;
    wd->last_hb_us = now_us;
    if (edge) {
        count_trip(wd);
    }
    return edge;
}

OCB_CORE_RAMFUNC uint8_t ocb_wd_check(ocb_wd *wd, uint64_t now_us, uint8_t mode, uint64_t last_app_frame_us,
                                      bool have_app_frame, uint32_t breaks_total)
{
    uint8_t verdict = 0u;
    const bool new_break = breaks_total != wd->breaks_seen;
    wd->breaks_seen = breaks_total;
    if (wd->armed) {
        if (ocb_wd_elapsed_us(now_us, wd->last_hb_us) > wd->link_limit_us) {
            verdict = (uint8_t)OCB_WD_LINK_TIMEOUT;
        } else if (new_break) {
            verdict = (uint8_t)OCB_WD_BREAK;
        }
        if (verdict != 0u) {
            const uint64_t age = ocb_wd_elapsed_us(now_us, wd->last_hb_us);
            wd->last_loss_age_us = age > 0xFFFFFFFFu ? 0xFFFFFFFFu : (uint32_t)age;
            wd->last_loss_kind = verdict;
            count_trip(wd);
            ocb_wd_note_loss(wd);
            wd->release_owed = false; /* the loss releases TX anyway */
            return verdict; /* the loss aborts the core: nothing else to check */
        }
    } else if (new_break) {
        verdict = (uint8_t)OCB_WD_RELEASE;
    }
    if (wd->release_owed) {
        wd->release_owed = false;
        verdict = (uint8_t)(verdict | OCB_WD_RELEASE);
    }
    if (mode == (uint8_t)OCB_MODE_DIRECT && have_app_frame &&
        ocb_wd_elapsed_us(now_us, last_app_frame_us) > wd->app_limit_us) {
        count_trip(wd);
        verdict = (uint8_t)(verdict | OCB_WD_APP_TIMEOUT);
    }
    return verdict;
}
