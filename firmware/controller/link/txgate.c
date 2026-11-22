/*
 * SPDX-License-Identifier: MIT
 *
 * TX gate state machine (AD-20, story 2.6 D15).
 */
#include <stdbool.h>
#include <stdint.h>

#include "ocb_link/config.h"
#include "ocb_link/txgate.h"

void ocb_txgate_init(ocb_txgate *g)
{
    g->open = false;
    g->high_run = false;
    g->high_since_us = 0u;
}

OCB_CORE_RAMFUNC bool ocb_txgate_sample(ocb_txgate *g, bool rx_high, uint64_t now_us)
{
    if (g->open) {
        return true;
    }
    if (!rx_high) {
        g->high_run = false;
        return false;
    }
    if (!g->high_run) {
        g->high_run = true;
        g->high_since_us = now_us;
    }
    if (now_us >= g->high_since_us && now_us - g->high_since_us >= (uint64_t)OCB_LINK_TX_IDLE_HIGH_US) {
        g->open = true;
        g->high_run = false;
    }
    return g->open;
}

void ocb_txgate_release(ocb_txgate *g)
{
    g->open = false;
    g->high_run = false;
}

OCB_CORE_RAMFUNC bool ocb_txgate_is_open(const ocb_txgate *g)
{
    return g->open;
}
