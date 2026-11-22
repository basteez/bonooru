/*
 * SPDX-License-Identifier: MIT
 *
 * A tiny C API over the real controller link (ocb_link + ocb_core +
 * ocb_proto), built as a shared library for the bench-sender simulation in
 * tests/test_link_probe.py (story 2.6 D31). Python drives it with ctypes
 * on a fake clock: the same code that runs on core 1, without the UART.
 * One instance, gate open, queue 512, debug counters on.
 */
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include "ocb_core/ocb_core.h"
#include "ocb_link/ocb_link.h"
#include "ocb_proto/ocb_proto.h"

static ocb_core_event s_queue[512];
static ocb_core s_core;
static ocb_usb_mailbox s_mailbox;
static ocb_link s_link;

void sim_init(uint16_t cap, uint64_t now_us);
void sim_rx(const uint8_t *bytes, size_t n, uint64_t now_us);
void sim_poll(uint64_t now_us);
size_t sim_tx(uint64_t now_us, uint8_t *out, size_t cap);
uint32_t sim_counter(int which);

void sim_init(uint16_t cap, uint64_t now_us)
{
    if (cap > 512u) {
        cap = 512u;
    }
    (void)ocb_core_init(&s_core, s_queue, cap, 0u);
    ocb_usb_mailbox_init(&s_mailbox);
    ocb_link_cfg cfg;
    memset(&cfg, 0, sizeof cfg);
    cfg.hid_profile = OCB_HID_PROFILE_P2;
    cfg.app_timeout_ms = OCB_APP_TIMEOUT_MS_DEFAULT;
    cfg.debug_counters = true;
    ocb_link_init(&s_link, &s_core, &s_mailbox, &cfg, now_us);
    (void)ocb_txgate_sample(ocb_link_gate(&s_link), true, now_us);
    (void)ocb_txgate_sample(ocb_link_gate(&s_link), true, now_us + OCB_LINK_TX_IDLE_HIGH_US);
}

void sim_rx(const uint8_t *bytes, size_t n, uint64_t now_us)
{
    for (size_t i = 0; i < n; i++) {
        ocb_link_rx_entry(&s_link, bytes[i], now_us);
    }
}

void sim_poll(uint64_t now_us)
{
    ocb_link_poll(&s_link, now_us);
}

size_t sim_tx(uint64_t now_us, uint8_t *out, size_t cap)
{
    return ocb_link_tx_next(&s_link, now_us, out, cap);
}

/* 0 queue count, 1 dropped_bytes, 2 COBS+CRC drops, 3 overlong runs,
 * 4 NAKs suppressed, 5 tx_dropped. */
uint32_t sim_counter(int which)
{
    const ocb_rx_stats *s = &s_link.rx.stats;
    switch (which) {
    case 0:
        return ocb_queue_count(&s_core.queue);
    case 1:
        return ocb_link_dropped_bytes(&s_link);
    case 2:
        return s->cobs_errors + s->crc_errors;
    case 3:
        return s->overlong;
    case 4:
        return ocb_link_counters_get(&s_link)->nak_suppressed;
    default:
        return ocb_link_counters_get(&s_link)->tx_dropped;
    }
}
