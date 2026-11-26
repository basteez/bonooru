/*
 * SPDX-License-Identifier: MIT
 *
 * TinyUSB glue of the P2 profile (story 2.3 Task 8.3), core 0 only. Every
 * byte and rule is in ocb_p2 (controller/profile_p2); this file only
 * connects it to TinyUSB 0.18.0 and to the USB mailbox of core 1. Written
 * for this firmware from the TinyUSB headers (hid_device.h, usbd.h); no
 * example code copied.
 *
 * TinyUSB 0.18.0, read in src/device/usbd.c and src/class/hid/hid_device.c
 * (story 2.3 Task 8.1, D6, D11):
 *  - OUT reports on the interrupt endpoint reach tud_hid_set_report_cb with
 *    report_id 0, type OUTPUT and the report id still in buffer[0]; a
 *    control SET_REPORT gives the id from wValue and strips it from the
 *    buffer when the buffer starts with it. FEATURE reports come the same
 *    way with type FEATURE.
 *  - tud_hid_report(0, buf, 64) sends the 64 bytes as they are (the id is
 *    our buf[0]); a non-zero id would be prepended.
 *  - Bus reset: DCD_EVENT_BUS_RESET only resets the stack (no application
 *    callback, tud_umount_cb is NOT called); the one place that sees it is
 *    tud_event_hook_cb, called from the USB interrupt when the event is
 *    queued. Unmount: tud_umount_cb (unplug event, or SET_CONFIGURATION 0).
 *    Suspend: tud_suspend_cb (only while connected). Mount:
 *    tud_mount_cb at SET_CONFIGURATION != 0.
 *  - tud_hid_report_complete_cb runs in tud_task() (task context), once
 *    per IN transfer the host took: that is where the IN interval is
 *    stamped, for 0x30 reports only (D13; the hook cannot tell which endpoint completed, so the
 *    stamp includes the loop's own latency, recorded in the story).
 *
 * Story 2.5: the reported state is NEUTRAL as soon as core 1 is seen stale
 * (liveness.h, D6); the IN interval spread, the longest pass of this task
 * and the published-state reader's counters go to the mailbox for the
 * debug STATUS v4 (D12, D13), posted when they change. Story 4.0: the
 * profile's reply-queue overflows (cnt.queue_full, since boot) go to the
 * mailbox the same way, for the debug STATUS v6.
 *
 * USB-down posting (D11, D26): each of suspend, unmount and bus reset
 * restarts the profile and sets the report period to 0; it posts its
 * mailbox counter only when the device was mounted since the last post, so
 * the bus resets of an enumeration (before SET_CONFIGURATION) are no false
 * abort, and one sleep posts once.
 */
#include "usb_p2.h"

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include "link_runner.h"
#include "liveness.h"
#include "ocb_core/timing.h"
#include "ocb_link/dbglog.h"
#include "ocb_link/ready.h"
#include "ocb_p2/config.h"
#include "ocb_p2/desc.h"
#include "ocb_p2/p2.h"
#include "ocb_proto/state.h"
#include "pico/stdlib.h"
#include "pico/unique_id.h"
#ifdef OCB_DEBUG_HANG_CORE0
#include "debug_hang.h"
#endif

/* Strict warnings stay on for this file, not for the SDK's headers. */
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wconversion"
#pragma GCC diagnostic ignored "-Wshadow"
#pragma GCC diagnostic ignored "-Wpedantic"
#include "tusb.h"
#include "device/dcd.h" /* DCD_EVENT_BUS_RESET, for tud_event_hook_cb */
#pragma GCC diagnostic pop

#ifndef OCB_P2_BINTERVAL
#error "OCB_P2_BINTERVAL must be defined by CMake (1..16, story 2.3 D4)"
#endif

/* The profile's log kinds and the link's debug log kinds are one table. */
typedef char ocb_p2_log_kinds_agree[(OCB_P2_LOG_USB_CMD == OCB_DBGLOG_USB_CMD && OCB_P2_LOG_SUBCMD == OCB_DBGLOG_SUBCMD &&
                                     OCB_P2_LOG_OTHER_REPORT == OCB_DBGLOG_OTHER_REPORT &&
                                     OCB_P2_LOG_REFUSAL == OCB_DBGLOG_REFUSAL &&
                                     OCB_P2_LOG_HANDSHAKE == OCB_DBGLOG_HANDSHAKE &&
                                     OCB_P2_LOG_RESTART == OCB_DBGLOG_RESTART && OCB_P2_LOG_ARG == OCB_DBGLOG_ARG)
                                        ? 1
                                        : -1];
typedef char ocb_p2_binterval_ok[(OCB_P2_BINTERVAL >= 1 && OCB_P2_BINTERVAL <= 16) ? 1 : -1];
typedef char ocb_p2_ep_buf_ok[(CFG_TUD_HID_EP_BUFSIZE == OCB_P2_REPORT_LEN) ? 1 : -1];

static ocb_p2 s_p2;
static uint8_t s_config_desc[OCB_P2_CONFIG_DESC_LEN];
static uint16_t s_string[OCB_P2_STRING_DESC_MAX / 2u];
static uint8_t s_in[OCB_P2_REPORT_LEN];
static volatile uint32_t s_bus_resets_isr; /* written by the USB interrupt only */
static uint32_t s_bus_resets_seen;
static bool s_up;                 /* mounted since the last USB-down post */
static uint32_t s_period_posted;  /* the report period last put in the mailbox */
/* Story 2.5 D12, D13: the self-timing last put in the mailbox, and the
 * longest pass since the last restart (or boot). */
static uint32_t s_pass_max_us;
static uint32_t s_spread_posted[4]; /* min, p95, max, slow */
static uint32_t s_core0_posted[3];  /* pass max, seq retries, seq fallbacks */
static uint32_t s_qfull_posted;     /* story 4.0: the queue_full last posted */

static void log_push(uint8_t kind, uint8_t id)
{
#ifdef OCB_DEBUG_BUILD
    (void)ocb_dbglog_push(ocb_ctrl_dbglog(), kind, id);
#else
    (void)kind;
    (void)id;
#endif
}

static void log_records(const ocb_p2_log *log)
{
#ifdef OCB_DEBUG_BUILD
    for (uint8_t i = 0u; i < log->n && i < OCB_P2_LOG_MAX; i++) {
        log_push(log->kind[i], log->id[i]);
    }
#else
    (void)log;
#endif
}

static void post_period(uint32_t period)
{
    if (period != s_period_posted) {
        s_period_posted = period;
        ocb_usb_mailbox_set_report_period(ocb_ctrl_usb_mailbox(), period);
    }
}

/* Story 2.5 D12, D13: posts the spread and core 0's numbers when they
 * changed (one store per field, debug STATUS v4 only); story 4.0: the
 * reply-queue overflows the same way (debug STATUS v6 only). */
static void post_self_timing(void)
{
    const uint32_t sp[4] = {ocb_p2_interval_min_us(&s_p2), ocb_p2_interval_p95_us(&s_p2),
                            ocb_p2_interval_max_us(&s_p2), ocb_p2_interval_slow(&s_p2)};
    if (sp[0] != s_spread_posted[0] || sp[1] != s_spread_posted[1] || sp[2] != s_spread_posted[2] ||
        sp[3] != s_spread_posted[3]) {
        memcpy(s_spread_posted, sp, sizeof sp);
        ocb_usb_mailbox_set_in_spread(ocb_ctrl_usb_mailbox(), sp[0], sp[1], sp[2], sp[3]);
    }
    const uint32_t c0[3] = {s_pass_max_us, ocb_ctrl_published_retries(), ocb_ctrl_published_fallbacks()};
    if (c0[0] != s_core0_posted[0] || c0[1] != s_core0_posted[1] || c0[2] != s_core0_posted[2]) {
        memcpy(s_core0_posted, c0, sizeof c0);
        ocb_usb_mailbox_set_core0(ocb_ctrl_usb_mailbox(), c0[0], c0[1], c0[2]);
    }
    const uint32_t qf = s_p2.cnt.queue_full;
    if (qf != s_qfull_posted) {
        s_qfull_posted = qf;
        ocb_usb_mailbox_set_p2_queue_full(ocb_ctrl_usb_mailbox(), qf);
    }
}

/* Restart of the profile for any of the four USB events (D11). */
static void restart(uint8_t cause)
{
    ocb_p2_restart(&s_p2);
    post_period(0u);
    s_pass_max_us = 0u; /* story 2.5: the pass maximum is per restart */
    log_push(OCB_DBGLOG_RESTART, cause);
}

static void usb_down(uint8_t cause, void (*post)(ocb_usb_mailbox *))
{
    restart(cause);
    if (s_up) {
        s_up = false;
        post(ocb_ctrl_usb_mailbox());
    }
}

/* The next IN report, when the endpoint is free and one is due. */
static void pump(void)
{
    if (!tud_hid_ready()) {
        return;
    }
    ocb_state st;
    if (liveness_core1_stale()) {
        st = OCB_NEUTRAL; /* story 2.5 D6: core 1 hung, release everything */
    } else {
        ocb_ctrl_published_read(&st);
    }
#ifdef OCB_DEBUG_HANG_CORE0
    debug_hang_note(&st);
#endif
    const size_t n = ocb_p2_next_in(&s_p2, &st, s_in, sizeof s_in);
    if (n > 0u) {
        (void)tud_hid_report(0u, s_in, (uint16_t)n);
    }
}

static void bus_reset_check(void)
{
    const uint32_t n = s_bus_resets_isr;
    if (n != s_bus_resets_seen) {
        s_bus_resets_seen = n;
        usb_down(OCB_DBGLOG_RESTART_BUS_RESET, ocb_usb_mailbox_bus_reset);
    }
}

void usb_p2_init(void)
{
    /* MAC (D10): 6 bytes from the board's 8-byte unique id. Bytes 2..7 are
     * XORed with bytes 0 and 1 (alternating), so all 64 bits of the id
     * reach the MAC and two boards whose ids differ only in the first bytes
     * still get different MACs. Then the first byte is made locally
     * administered (bit 1 set) and unicast (bit 0 clear), so it never
     * collides with a vendor-assigned MAC. */
    pico_unique_board_id_t id;
    pico_get_unique_board_id(&id);
    ocb_p2_cfg cfg;
    for (size_t i = 0; i < 6u; i++) {
        cfg.mac[i] = (uint8_t)(id.id[i + 2u] ^ id.id[i % 2u]);
    }
    cfg.mac[0] = (uint8_t)((cfg.mac[0] & 0xFCu) | 0x02u);
    ocb_p2_init(&s_p2, &cfg);
    (void)ocb_p2_config_desc((uint8_t)OCB_P2_BINTERVAL, s_config_desc, sizeof s_config_desc);
    s_bus_resets_isr = 0u;
    s_bus_resets_seen = 0u;
    s_up = false;
    s_period_posted = 0u;
    s_pass_max_us = 0u;
    memset(s_spread_posted, 0, sizeof s_spread_posted);
    memset(s_core0_posted, 0, sizeof s_core0_posted);
    s_qfull_posted = 0u;
    (void)tud_init(0u);
}

void usb_p2_task(void)
{
    const uint32_t t_start = time_us_32();
    bus_reset_check();
    tud_task();
    bus_reset_check();
    ocb_p2_tick(&s_p2, time_us_64());
    post_period(ocb_p2_report_period_us(&s_p2));
    if (ocb_p2_ready(&s_p2) || ocb_p2_reply_pending(&s_p2)) {
        pump();
    }
    /* Story 2.5 D12: the longest pass bounds the IN stamp latency. */
    ocb_core_max_u32(&s_pass_max_us, ocb_core_span_us(t_start, time_us_32()));
    post_self_timing();
#ifdef OCB_DEBUG_HANG_CORE0
    debug_hang_poll();
#endif
}

/* ------------------------------------------------------------------ */
/* TinyUSB callbacks (strong definitions of the stack's weak ones).    */
/* ------------------------------------------------------------------ */

uint8_t const *tud_descriptor_device_cb(void)
{
    return ocb_p2_device_desc;
}

uint8_t const *tud_descriptor_configuration_cb(uint8_t index)
{
    (void)index;
    return s_config_desc;
}

uint16_t const *tud_descriptor_string_cb(uint8_t index, uint16_t langid)
{
    (void)langid;
    const size_t n = ocb_p2_string_desc(index, (uint8_t *)s_string, sizeof s_string);
    return n > 0u ? s_string : NULL;
}

uint8_t const *tud_hid_descriptor_report_cb(uint8_t instance)
{
    (void)instance;
    return ocb_p2_report_desc;
}

/* Interrupt context: count bus resets only; usb_p2_task handles them. */
void tud_event_hook_cb(uint8_t rhport, uint32_t eventid, bool in_isr)
{
    (void)rhport;
    (void)in_isr;
    if (eventid == (uint32_t)DCD_EVENT_BUS_RESET) {
        s_bus_resets_isr = s_bus_resets_isr + 1u;
    }
}

void tud_mount_cb(void)
{
    restart(OCB_DBGLOG_RESTART_MOUNT);
    s_up = true;
}

void tud_umount_cb(void)
{
    usb_down(OCB_DBGLOG_RESTART_UNMOUNT, ocb_usb_mailbox_unmount);
}

void tud_suspend_cb(bool remote_wakeup_en)
{
    (void)remote_wakeup_en;
    usb_down(OCB_DBGLOG_RESTART_SUSPEND, ocb_usb_mailbox_suspend);
}

uint16_t tud_hid_get_report_cb(uint8_t instance, uint8_t report_id, hid_report_type_t report_type, uint8_t *buffer,
                               uint16_t reqlen)
{
    /* No GET_REPORT in the P2 conversation (S1, S5, S6): stall. */
    (void)instance;
    (void)report_id;
    (void)report_type;
    (void)buffer;
    (void)reqlen;
    return 0u;
}

void tud_hid_set_report_cb(uint8_t instance, uint8_t report_id, hid_report_type_t report_type, uint8_t const *buffer,
                           uint16_t bufsize)
{
    (void)instance;
    ocb_p2_log log;
    log.n = 0u;
    if (report_type == HID_REPORT_TYPE_FEATURE) {
        (void)ocb_p2_on_feature(&s_p2, report_id, &log); /* refused (D26) */
    } else if (report_type == HID_REPORT_TYPE_OUTPUT) {
        if (report_id == 0u) {
            (void)ocb_p2_on_output(&s_p2, buffer, bufsize, &log);
        } else {
            /* Control SET_REPORT: put the id back in front (D6). */
            uint8_t b[OCB_P2_REPORT_LEN + 1u];
            const size_t n = bufsize < OCB_P2_REPORT_LEN ? (size_t)bufsize : (size_t)OCB_P2_REPORT_LEN;
            b[0] = report_id;
            if (n > 0u) {
                memcpy(&b[1], buffer, n);
            }
            (void)ocb_p2_on_output(&s_p2, b, n + 1u, &log);
        }
    }
    log_records(&log);
    if (ocb_p2_take_handshake(&s_p2)) {
        ocb_usb_mailbox_handshake_done(ocb_ctrl_usb_mailbox());
    }
    pump();
}

void tud_hid_report_complete_cb(uint8_t instance, uint8_t const *report, uint16_t len)
{
    (void)instance;
    /* Every IN the host took feeds console_ready (the stall rule). Only
     * 0x30 completions feed the interval median (review fix): the
     * handshake replies come at the console's request pace and would skew
     * the period Story 2.5 needs. */
    ocb_usb_mailbox_in_report(ocb_ctrl_usb_mailbox());
    if (report != NULL && len > 0u && report[0] == OCB_P2_RID_IN_STANDARD) {
        ocb_p2_interval_note(&s_p2, time_us_64());
    }
    pump();
}
