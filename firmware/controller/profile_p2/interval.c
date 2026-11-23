/*
 * SPDX-License-Identifier: MIT
 *
 * Median IN interval (story 2.3 D13, AC5b): the time between two IN
 * completions, measured on core 0, published as INFO.report_period_us.
 * Story 2.5 D12, D35 add the jitter of issue #21: the 95th percentile of
 * the same window, and over one span (every interval since the restart
 * while a median is published) the minimum, the maximum and the count of
 * slow intervals (above 1.5 x the median).
 *
 * Structure: a histogram of 401 bins of 50 us (bin k holds intervals in
 * [50k - 25, 50k + 25) us, so k * 50 is the bin's centre) and a ring of the
 * bin index of the last 256 samples, so the oldest sample leaves the
 * histogram when a new one enters. Cost: ocb_p2_interval_note is O(1)
 * (one ring slot, two bin updates, one compare for `slow`); the median,
 * minimum, 95th percentile and maximum are recomputed in ocb_p2_tick only
 * when samples arrived, by one walk over the 401 bins (ops_median <=
 * OCB_P2_IV_BINS, asserted by the F.29 test). Memory: 802 + 512 bytes.
 */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include "ocb_p2/config.h"
#include "ocb_p2/internal.h"
#include "ocb_p2/p2.h"

void ocb_p2_interval_clear_(ocb_p2_interval *iv)
{
    memset(iv, 0, sizeof *iv);
}

void ocb_p2_interval_note(ocb_p2 *p, uint64_t now_us)
{
    ocb_p2_interval *iv = &p->iv;
    const uint32_t now = (uint32_t)(now_us & 0xFFFFFFFFu);
    iv->ops_note = 0u;
    if (!iv->have_last) {
        iv->have_last = true;
        iv->last_us = now;
        return;
    }
    /* Unsigned difference of the low 32 bits: wrap-safe for 32-bit and
     * 64-bit clocks; a backwards step gives a huge value and is dropped. */
    const uint32_t dt = now - iv->last_us;
    iv->last_us = now;
    /* Story 2.5 D35: min and max over every interval since the restart
     * (stalls above 20 ms too), slow over those since the median is
     * published: slow is a subset of the min..max span. A backwards step
     * (dt >= 2^31) is no interval. */
    if (dt < 0x80000000u) {
        if (iv->run_min_us == 0u || dt < iv->run_min_us) {
            iv->run_min_us = dt;
        }
        if (dt > iv->run_max_us) {
            iv->run_max_us = dt;
        }
        /* 2 dt > 3 median, no division (dt < 2^31, median <= 20000). */
        if (iv->median_us != 0u && (uint64_t)dt * 2u > (uint64_t)iv->median_us * 3u && iv->slow != UINT32_MAX) {
            iv->slow++;
        }
    }
    if (dt > OCB_P2_IV_MAX_US) {
        return; /* not in the window: the median must not move */
    }
    const uint16_t bin = (uint16_t)((dt + OCB_P2_IV_BIN_US / 2u) / OCB_P2_IV_BIN_US);
    if (iv->count >= OCB_P2_IV_WINDOW) {
        const uint16_t old = iv->ring[iv->ring_head];
        if (old < OCB_P2_IV_BINS && iv->bins[old] > 0u) {
            iv->bins[old]--;
        }
    } else {
        iv->count++;
    }
    iv->ring[iv->ring_head] = bin;
    iv->ring_head = (uint16_t)((iv->ring_head + 1u) % OCB_P2_IV_WINDOW);
    if (bin < OCB_P2_IV_BINS) {
        iv->bins[bin]++;
    }
    iv->dirty = true;
    iv->ops_note = 1u;
}

void ocb_p2_tick(ocb_p2 *p, uint64_t now_us)
{
    (void)now_us;
    ocb_p2_interval *iv = &p->iv;
    if (!iv->dirty) {
        return;
    }
    iv->dirty = false;
    iv->ops_median = 0u;
    if (iv->count < OCB_P2_IV_MIN_SAMPLES) {
        iv->median_us = 0u;
        iv->min_us = 0u;
        iv->p95_us = 0u;
        iv->max_us = 0u;
        return;
    }
    /* Lower median: the k-th smallest sample, k = (count + 1) / 2; 95th
     * percentile: the k95-th smallest, k95 = ceil(0.95 * count). */
    const uint32_t k = ((uint32_t)iv->count + 1u) / 2u;
    const uint32_t k95 = ((uint32_t)iv->count * 95u + 99u) / 100u;
    uint32_t seen = 0u;
    bool have_min = false;
    bool have_median = false;
    bool have_p95 = false;
    for (uint32_t b = 0u; b < OCB_P2_IV_BINS; b++) {
        iv->ops_median++;
        if (iv->bins[b] == 0u) {
            continue;
        }
        seen += iv->bins[b];
        if (!have_min) {
            have_min = true;
            iv->min_us = b * OCB_P2_IV_BIN_US;
        }
        if (!have_median && seen >= k) {
            have_median = true;
            iv->median_us = b * OCB_P2_IV_BIN_US;
        }
        if (!have_p95 && seen >= k95) {
            have_p95 = true;
            iv->p95_us = b * OCB_P2_IV_BIN_US;
        }
        iv->max_us = b * OCB_P2_IV_BIN_US;
    }
    if (!have_median) {
        iv->median_us = 0u; /* unreachable: count samples are in the bins */
    }
}

uint32_t ocb_p2_report_period_us(const ocb_p2 *p)
{
    return p->iv.median_us;
}

uint32_t ocb_p2_interval_min_us(const ocb_p2 *p)
{
    return p->iv.run_min_us;
}

uint32_t ocb_p2_interval_p95_us(const ocb_p2 *p)
{
    return p->iv.p95_us;
}

uint32_t ocb_p2_interval_max_us(const ocb_p2 *p)
{
    return p->iv.run_max_us;
}

uint32_t ocb_p2_interval_slow(const ocb_p2 *p)
{
    return p->iv.slow;
}
