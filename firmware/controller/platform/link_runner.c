/*
 * SPDX-License-Identifier: MIT
 *
 * Controller link runner on core 1 (story 2.6 D3, D14, D15; story 2.7: the
 * link watchdog's timeout and TX release; story 2.3: the P2 debug log
 * drain, the HID profile from OCB_PROFILE; story 2.5: the published state
 * through the seqlock double buffer in every build (D4), the boot cause in
 * the link config (D7), the self-timing of the debug STATUS v4 (D13: loop
 * maximum, stack mark), the built-in timeline loader of the timeline
 * flavour (D10) and the core 1 hang injector). Glue only: every protocol
 * rule lives in ocb_link / ocb_core, every register access in
 * ocb_hw/uart_link. The whole program runs from RAM (copy_to_ram, story
 * 2.5 D2); the SDK macro on the hot functions keeps them in RAM in the XIP
 * comparison variant too.
 */
#include "link_runner.h"

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "liveness.h"
#include "ocb_core/timing.h"
#include "ocb_hw/uart_link.h"
#include "ocb_link/ocb_link.h"
#include "ocb_link/pubstate.h"
#include "ocb_proto/consts.h"
#include "ocb_proto/state.h"
#include "ocb_version.h"
#include "pico/stdlib.h"
#ifdef OCB_DEBUG_BUILD
#include "stack_mark.h"
#endif
#ifdef OCB_DEBUG_TIMELINE
#include "ocb_core/testtl.h"
#endif
#ifdef OCB_DEBUG_HANG_CORE1
#include "debug_hang.h"
#endif

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
/* Story 2.5 D4: the published state, core 1 writes, core 0 reads, through
 * the seqlock double buffer, in every build. */
static ocb_pubstate s_pub;
#ifdef OCB_DEBUG_BUILD
/* Story 2.3 D14: the P2 debug log, core 0 pushes, core 1 drains. Debug
 * builds only (AC5a: a release ELF has none of it). */
static ocb_dbglog_ring s_dbglog;
/* Story 2.5 D13, Task 11: core 1's longest pass since START, and the mode
 * seen at the previous pass (a START is the move into PLAYING). */
static uint32_t s_loop_max_us;
static uint8_t s_mode_seen;
#endif
#ifdef OCB_DEBUG_TIMELINE
static bool s_tl_tried; /* a load failed since the core last left the loadable state */
#endif

typedef char ocb_queue_cap_fits_u16[(OCB_QUEUE_CAP_DEFAULT <= 0xFFFF) ? 1 : -1];
/* Story 2.5 D13 (review fix): the largest debug STATUS (header, 11-byte base
 * body, the full debug tail, CRC, one COBS byte, the delimiter) fits the UART
 * driver's TX ring in one frame; tx_debug.c restates the ring size because a
 * pure library cannot include ocb_hw, this is where both are visible. */
#define OCB_CTRL_STATUS_WIRE_MAX                                                                                     \
    (OCB_FRAME_HEADER_SIZE + OCB_BODY_LEN_STATUS + OCB_LINK_DEBUG_TAIL_MAX_LEN + OCB_FRAME_CRC_SIZE + 2)
_Static_assert(OCB_UART_LINK_TX_RING >= OCB_CTRL_STATUS_WIRE_MAX,
               "the UART TX ring must hold the largest debug STATUS frame (story 2.5 D13)");

void ocb_ctrl_usb_mailbox_init(void)
{
    ocb_usb_mailbox_init(&s_mailbox);
    ocb_pubstate_init(&s_pub);
#ifdef OCB_DEBUG_BUILD
    ocb_dbglog_ring_init(&s_dbglog);
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
    cfg.boot_cause = liveness_boot_cause();     /* story 2.5 D7 */
    cfg.reboot_count = liveness_reboot_count();
    const uint64_t t = time_us_64();
    ocb_link_init(&s_link, &s_core, &s_mailbox, &cfg, t);
    /* D28: the mailbox was zeroed by main() before core 1 started, so count
     * from zero: anything core 0 posted in between is seen, not lost. */
    static const ocb_usb_mailbox k_zero_mailbox = {0};
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
#ifdef OCB_DEBUG_BUILD
    s_loop_max_us = 0u;
    s_mode_seen = s_core.mode;
#endif
#ifdef OCB_DEBUG_TIMELINE
    s_tl_tried = false;
#endif
}

static void __not_in_flash_func(rx_sink)(uint16_t dr)
{
    (void)ocb_rxring_push(ocb_link_rx_ring(&s_link), dr);
}

void ocb_ctrl_link_start_rx(void)
{
    ocb_uart_link_rx_irq_enable(rx_sink);
}

#ifdef OCB_DEBUG_TIMELINE
/* Story 2.5 D10: the built-in timeline is loaded through the real core
 * whenever the core is loadable (session open, IDLE, not latched, empty
 * queue), so HELLO or STOP (which empty the queue) re-arm it; a failed
 * load is not retried every pass; the host sends only HELLO and START. */
static void timeline_loader(uint64_t now)
{
    const bool loadable = ocb_link_session_open(&s_link) && s_core.mode == OCB_MODE_IDLE && !s_core.aborted &&
                          ocb_queue_count(&s_core.queue) == 0u;
    if (!loadable) {
        s_tl_tried = false;
        return;
    }
    if (s_tl_tried) {
        return;
    }
    ocb_core_env env;
    env.now_us = now;
    env.ready = ocb_link_console_ready(&s_link, now);
    env.link_ok = ocb_link_link_ok(&s_link);
    /* A loaded queue is not empty, so the next loadable state can only come
     * from a HELLO or STOP that emptied it (even in the very next pass): no
     * flag to keep. A failed load (the queue shows it: STATUS free) is not
     * retried until the core leaves the loadable state. */
    s_tl_tried = ocb_testtl_load(&s_core, &env) != 0;
}
#endif

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
#ifdef OCB_DEBUG_TIMELINE
    timeline_loader(now);
#endif
    /* Story 2.3 D35, story 2.5 D4: a newly published state (SET_STATE in
     * DIRECT, the scheduler in PLAYING), for the USB report on core 0;
     * written only when publish_count moved. Every build. */
    (void)ocb_pubstate_publish(&s_pub, &s_core.published, s_core.publish_count);
#ifdef OCB_DEBUG_HANG_CORE1
    debug_hang_note(&s_core.published);
    debug_hang_poll();
#endif
}

#ifdef OCB_DEBUG_BUILD
void __not_in_flash_func(ocb_ctrl_link_pass_end)(uint32_t pass_start_us)
{
    /* Story 2.5 Task 11: the longest pass since START (wrap-safe). */
    if (s_core.mode == OCB_MODE_PLAYING && s_mode_seen != OCB_MODE_PLAYING) {
        s_loop_max_us = 0u;
    }
    s_mode_seen = s_core.mode;
    ocb_core_max_u32(&s_loop_max_us, ocb_core_span_us(pass_start_us, time_us_32()));
    /* Story 2.5 D15: a few words of the core 1 stack per pass. */
    stack_mark_scan_step();
    ocb_link_set_self_timing(&s_link, s_loop_max_us, stack_mark_used_bytes());
}
#endif

void __not_in_flash_func(ocb_ctrl_link_abort_stall)(void)
{
    ocb_core_out out;
    ocb_core_abort(&s_core, OCB_ABORT_CORE_STALL, &out); /* no EVENT (AD-8) */
    (void)ocb_pubstate_publish(&s_pub, &s_core.published, s_core.publish_count);
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

/* Story 2.5 D4: core 1's published state through the seqlock double
 * buffer (3 tries, else NEUTRAL), in every build. Core 0. */
void __not_in_flash_func(ocb_ctrl_published_read)(ocb_state *out)
{
    (void)ocb_pubstate_read(&s_pub, out);
}

uint32_t ocb_ctrl_published_retries(void)
{
    return ocb_pubstate_retries(&s_pub);
}

uint32_t ocb_ctrl_published_fallbacks(void)
{
    return ocb_pubstate_fallbacks(&s_pub);
}
