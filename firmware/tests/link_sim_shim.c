/*
 * SPDX-License-Identifier: MIT
 *
 * A tiny C API over the real controller link (ocb_link + ocb_core +
 * ocb_proto), built as a shared library for the bench-sender simulation in
 * tests/test_link_probe.py (story 2.6 D31). Python drives it with ctypes
 * on a fake clock: the same code that runs on core 1, without the UART.
 * One instance, gate open, queue 512, debug counters on, default link and
 * app timeouts. Story 2.7 adds what the bench steps need (F.23): a break
 * entry, the RX level the TX gate samples, the glue's tx_settled, the USB
 * mailbox posts of a ready console (as Story 2.3 will post them), and the
 * watchdog counters. Heartbeats arrive as ordinary RX bytes (sim_rx).
 * Story 2.3 (F.32): every USB mailbox post one by one (a scripted console),
 * the measured report period, and the P2 debug log ring that sim_poll
 * drains into the debug STATUS window, as the firmware's core 1 does.
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
static ocb_dbglog_ring s_dbglog;

void sim_init(uint16_t cap, uint64_t now_us);
void sim_rx(const uint8_t *bytes, size_t n, uint64_t now_us);
void sim_poll(uint64_t now_us);
size_t sim_tx(uint64_t now_us, uint8_t *out, size_t cap);
uint32_t sim_counter(int which);
void sim_break(uint64_t now_us);
void sim_rx_level(int high, uint64_t now_us);
void sim_tx_settled(uint64_t now_us, int idle);
void sim_usb_ready(uint64_t now_us);
void sim_usb_event(int which);
void sim_report_period(uint32_t us);
int sim_dbglog(uint8_t kind, uint8_t id);

static int s_handshake_posted;
static uint32_t s_hat_seen; /* story 2.3 D35: bit per published hat value seen at a poll */

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
    cfg.link_timeout_ms = OCB_LINK_TIMEOUT_MS;
    cfg.debug_counters = true;
    s_handshake_posted = 0;
    s_hat_seen = 0u;
    ocb_dbglog_ring_init(&s_dbglog);
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
    (void)ocb_dbglog_drain(ocb_link_dbglog_window(&s_link), &s_dbglog);
    if (s_core.published.hat < 32u) {
        s_hat_seen |= 1u << s_core.published.hat;
    }
}

size_t sim_tx(uint64_t now_us, uint8_t *out, size_t cap)
{
    return ocb_link_tx_next(&s_link, now_us, out, cap);
}

/* One data-register entry with BE: what a pulled wire gives (2.6 D20). */
void sim_break(uint64_t now_us)
{
    ocb_link_rx_entry(&s_link, (uint16_t)OCB_LINK_RX_BE, now_us);
}

/* The RX pin level, as the glue samples it every pass for the TX gate. */
void sim_rx_level(int high, uint64_t now_us)
{
    (void)ocb_txgate_sample(ocb_link_gate(&s_link), high != 0, now_us);
}

/* The glue's call after the TX pump; idle = driver ring, FIFO and shifter
 * empty. */
void sim_tx_settled(uint64_t now_us, int idle)
{
    ocb_link_tx_settled(&s_link, now_us, idle != 0);
}

/* A host that completed the handshake (a console, or p2_probe.py
 * handshake --hold on the bench Mac): the handshake once, then an IN report
 * per call. */
void sim_usb_ready(uint64_t now_us)
{
    (void)now_us;
    if (!s_handshake_posted) {
        ocb_usb_mailbox_handshake_done(&s_mailbox);
        s_handshake_posted = 1;
    }
    ocb_usb_mailbox_in_report(&s_mailbox);
}

/* Story 2.3: one mailbox post, as the P2 glue makes it: 0 handshake done,
 * 1 IN report, 2 suspend, 3 unmount, 4 bus reset. */
void sim_usb_event(int which)
{
    switch (which) {
    case 0:
        ocb_usb_mailbox_handshake_done(&s_mailbox);
        break;
    case 1:
        ocb_usb_mailbox_in_report(&s_mailbox);
        break;
    case 2:
        ocb_usb_mailbox_suspend(&s_mailbox);
        break;
    case 3:
        ocb_usb_mailbox_unmount(&s_mailbox);
        break;
    default:
        ocb_usb_mailbox_bus_reset(&s_mailbox);
        break;
    }
}

/* Story 2.3 D13: the measured median IN interval, as core 0 posts it. */
void sim_report_period(uint32_t us)
{
    ocb_usb_mailbox_set_report_period(&s_mailbox, us);
}

/* Story 2.3 D14: one P2 debug log record (core 0's push); 0 if dropped. */
int sim_dbglog(uint8_t kind, uint8_t id)
{
    return ocb_dbglog_push(&s_dbglog, kind, id) ? 1 : 0;
}

/* 0 queue count, 1 dropped_bytes, 2 COBS+CRC drops, 3 overlong runs,
 * 4 NAKs suppressed, 5 tx_dropped; story 2.7: 6 watchdog_trips,
 * 7 abort_count, 8 link_ok, 9 TX gate open, 10 mode, 11 release pending;
 * story 2.3: 12 the published hat values seen (a bit each). */
uint32_t sim_counter(int which)
{
    const ocb_rx_stats *s = &s_link.rx.stats;
    switch (which) {
    case 6:
        return ocb_link_watchdog_trips(&s_link);
    case 7:
        return s_core.abort_count;
    case 8:
        return ocb_link_link_ok(&s_link) ? 1u : 0u;
    case 9:
        return ocb_txgate_is_open(ocb_link_gate(&s_link)) ? 1u : 0u;
    case 10:
        return s_core.mode;
    case 11:
        return ocb_link_release_pending(&s_link) ? 1u : 0u;
    case 12:
        return s_hat_seen;
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
