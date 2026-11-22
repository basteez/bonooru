/*
 * SPDX-License-Identifier: MIT
 *
 * Controller link runner on core 1 (story 2.6 D3, D14, D15; story 2.7: the
 * link watchdog's timeout and TX release; story 2.3: the P2 debug log
 * drain, the published-state read the USB profile uses, the HID profile
 * from OCB_PROFILE). Glue only:
 * every protocol rule lives in ocb_link / ocb_core, every register access
 * in ocb_hw/uart_link. Hot functions are placed in RAM with the SDK macro
 * (the pure libraries use OCB_CORE_RAMFUNC, empty for now: Story 2.5).
 */
#include "link_runner.h"

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "ocb_hw/uart_link.h"
#include "ocb_link/ocb_link.h"
#include "ocb_link/pubstate.h"
#include "ocb_proto/consts.h"
#include "ocb_proto/state.h"
#include "ocb_version.h"
#include "pico/stdlib.h"

#ifdef OCB_DEBUG_BUILD
#define OCB_LINK_DEBUG_COUNTERS true
#else
#define OCB_LINK_DEBUG_COUNTERS false
#endif

/* INFO.hid_profile from the build's profile (story 2.3 D21). */
#if defined(OCB_PROFILE_P2)
#define OCB_CTRL_HID_PROFILE OCB_HID_PROFILE_P2
#else
#error "OCB_PROFILE: only P2 is built (story 2.3 D21)"
#endif

/* Static storage (no heap): the queue is 512 x 12 bytes = 6 KB. */
static ocb_core_event s_queue[OCB_QUEUE_CAP_DEFAULT];
static ocb_core s_core;
static ocb_link s_link;
static ocb_usb_mailbox s_mailbox;
static bool s_tx_enabled;
#ifdef OCB_DEBUG_BUILD
/* Story 2.3 D14: the P2 debug log, core 0 pushes, core 1 drains. Debug
 * builds only (AC5a: a release ELF has none of it). */
static ocb_dbglog_ring s_dbglog;
/* Story 2.3 D35: the published state, core 1 writes, core 0 reads (debug
 * builds only, so the bench can prove a state reaches the console before
 * Story 2.5 owns the real seqlock). */
static ocb_pubstate s_pub;
#endif

typedef char ocb_queue_cap_fits_u16[(OCB_QUEUE_CAP_DEFAULT <= 0xFFFF) ? 1 : -1];

void ocb_ctrl_usb_mailbox_init(void)
{
    ocb_usb_mailbox_init(&s_mailbox);
#ifdef OCB_DEBUG_BUILD
    ocb_dbglog_ring_init(&s_dbglog);
    ocb_pubstate_init(&s_pub);
#endif
}

void ocb_ctrl_link_init(void)
{
    (void)ocb_core_init(&s_core, s_queue, (uint16_t)OCB_QUEUE_CAP_DEFAULT, 0u);
    ocb_link_cfg cfg;
    cfg.fw_ver[0] = (uint8_t)OCB_VERSION_MAJOR;
    cfg.fw_ver[1] = (uint8_t)OCB_VERSION_MINOR;
    cfg.fw_ver[2] = (uint8_t)OCB_VERSION_PATCH;
    cfg.hid_profile = (uint8_t)OCB_CTRL_HID_PROFILE;
    cfg.app_timeout_ms = (uint16_t)OCB_APP_TIMEOUT_MS_DEFAULT;
    cfg.link_timeout_ms = (uint16_t)OCB_LINK_TIMEOUT_MS; /* story 2.7: AD-5 default, clamped to 100 */
    cfg.debug_counters = OCB_LINK_DEBUG_COUNTERS;
    const uint64_t t = time_us_64();
    ocb_link_init(&s_link, &s_core, &s_mailbox, &cfg, t);
    /* D28: the mailbox was zeroed by main() before core 1 started, so count
     * from zero: anything core 0 posted in between is seen, not lost. */
    static const ocb_usb_mailbox k_zero_mailbox = {0u, 0u, 0u, 0u, 0u, 0u};
    ocb_link_ready_rebase(&s_link, &k_zero_mailbox, t);

    const uint32_t baud = ocb_uart_link_init();
#ifdef OCB_DEBUG_BUILD
    /* Within 1 % of the nominal rate (no stdio: an assert, not a print). */
    const uint32_t nominal = (uint32_t)OCB_LINK_BAUD;
    const uint32_t diff = baud > nominal ? baud - nominal : nominal - baud;
    hard_assert(diff * 100u <= nominal);
#else
    (void)baud;
#endif
    s_tx_enabled = false;
}

static void __not_in_flash_func(rx_sink)(uint16_t dr)
{
    (void)ocb_rxring_push(ocb_link_rx_ring(&s_link), dr);
}

void ocb_ctrl_link_start_rx(void)
{
    ocb_uart_link_rx_irq_enable(rx_sink);
}

void __not_in_flash_func(ocb_ctrl_link_step)(void)
{
    const uint64_t now = time_us_64();
    ocb_txgate *gate = ocb_link_gate(&s_link);

    /* AD-20: TX is driven only while the gate is open. */
    const bool open = ocb_txgate_sample(gate, ocb_uart_link_rx_level(), now);
    if (open != s_tx_enabled) {
        ocb_uart_link_tx_enable(open);
        s_tx_enabled = open;
    }

    /* Poll first (USB mailbox, ready, aborts, scheduler), so a START or
     * SET_STATE drained in this pass sees a fresh `ready` (D28). */
    ocb_link_poll(&s_link, now);
    (void)ocb_link_rx_drain(&s_link, now, (uint32_t)OCB_LINK_RX_RING_CAP);
#ifdef OCB_DEBUG_BUILD
    /* Story 2.3 D14: the P2 debug log into the debug STATUS window. */
    (void)ocb_dbglog_drain(ocb_link_dbglog_window(&s_link), &s_dbglog);
#endif

#ifdef OCB_DEBUG_BUILD
    hard_assert(get_core_num() == 1u); /* AC1, AD-11: core 1 is the only TX writer */
#endif
    uint8_t frame[OCB_MAX_WIRE_FRAME];
    const size_t space = open ? ocb_uart_link_tx_space() : sizeof frame;
    const size_t cap = space < sizeof frame ? space : sizeof frame;
    /* While closed this discards the pending output (no stale burst). */
    const size_t n = ocb_link_tx_next(&s_link, now, frame, cap);
    if (n > 0u && open) {
        (void)ocb_uart_link_tx_write(frame, n);
    }
    if (open) {
        ocb_uart_link_tx_pump();
    }
    /* Story 2.7 D6, D23: a TX release after a link loss or a break completes
     * once the EVENT has left and the UART is idle (or after 5 ms); the next
     * pass then sees the closed gate and returns the pad to high-Z above. */
    ocb_link_tx_settled(&s_link, now, ocb_uart_link_tx_idle());
#ifdef OCB_DEBUG_BUILD
    /* Story 2.3 D35: the state the core published in this pass (SET_STATE
     * in DIRECT, the scheduler in PLAYING), for the USB report on core 0. */
    ocb_pubstate_write(&s_pub, &s_core.published);
#endif
}

const ocb_core *ocb_ctrl_core(void)
{
    return &s_core;
}

ocb_usb_mailbox *ocb_ctrl_usb_mailbox(void)
{
    return &s_mailbox;
}

#ifdef OCB_DEBUG_BUILD
ocb_dbglog_ring *ocb_ctrl_dbglog(void)
{
    return &s_dbglog;
}
#endif

/* Story 2.3 D12, D35: debug builds read core 1's published state through
 * the sequence lock (3 tries, then NEUTRAL); release builds report NEUTRAL
 * until Story 2.5 makes the seqlock the real path for every build. */
void ocb_ctrl_published_read(ocb_state *out)
{
#ifdef OCB_DEBUG_BUILD
    (void)ocb_pubstate_read(&s_pub, out);
#else
    *out = OCB_NEUTRAL;
#endif
}
