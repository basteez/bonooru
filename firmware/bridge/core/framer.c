/*
 * SPDX-License-Identifier: MIT
 *
 * PC -> UART framer (AD-4, Story 3.1). See ocb_bridge/framer.h.
 */
#include "ocb_bridge/framer.h"

#include <string.h>

void ocb_bridge_framer_init(ocb_bridge_framer *f)
{
    memset(f, 0, sizeof *f);
}

/* A run ended at a 0x00: queue it (the slot must be free). */
static void complete(ocb_bridge_framer *f)
{
    memcpy(f->queued, f->run, f->run_len);
    f->queued[f->run_len] = OCB_FRAME_DELIMITER;
    f->queued_len = f->run_len + 1u;
    if (f->run_len == 0u) {
        f->stats.empty++;
    }
    f->stats.frames++;
    f->run_len = 0;
}

size_t ocb_bridge_framer_feed(ocb_bridge_framer *f, const uint8_t *data, size_t n)
{
    size_t i = 0;
    while (i < n) {
        const uint8_t b = data[i];
        if (b != OCB_FRAME_DELIMITER) {
            i++;
            if (f->discarding) {
                continue;
            }
            if (f->run_len >= (size_t)OCB_MAX_ENCODED_FRAME) {
                /* First excess byte: count the overlong run once, drop the rest. */
                f->stats.overlong++;
                f->discarding = true;
                f->run_len = 0;
                continue;
            }
            f->run[f->run_len++] = b;
            continue;
        }
        if (f->discarding) {
            f->discarding = false;
            f->run_len = 0;
            i++;
            continue;
        }
        if (f->queued_len != 0u) {
            /* A run completes but the queued slot is busy: stop before the
             * delimiter; the caller offers it again later. */
            break;
        }
        i++;
        complete(f);
    }
    return i;
}

void ocb_bridge_framer_discard_partial(ocb_bridge_framer *f)
{
    if (f->run_len != 0u || f->discarding) {
        f->stats.partial_discards++;
    }
    f->run_len = 0;
    f->discarding = false;
}

bool ocb_bridge_framer_has_frame(const ocb_bridge_framer *f)
{
    return f->queued_len != 0u;
}

size_t ocb_bridge_framer_take(ocb_bridge_framer *f, uint8_t *out, size_t cap)
{
    const size_t n = f->queued_len;
    if (n == 0u || cap < n) {
        return 0;
    }
    memcpy(out, f->queued, n);
    f->queued_len = 0;
    return n;
}
