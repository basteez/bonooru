/*
 * SPDX-License-Identifier: MIT
 *
 * ocb_link: the controller side of the PC <-> controller link (story 2.6).
 * Pure C99, no clock, no I/O, no allocation, no global state: the glue
 * feeds RX entries and the time, and pulls whole wire frames to send.
 *
 *   RX:  ocb_link_rx_entry() per data-register value (or ocb_link_rx_drain()
 *        from the embedded ring) -> error-byte policy -> ocb_rx splitter
 *        -> admission -> ocb_core_frame -> replies.
 *   Poll: ocb_link_poll() -> USB mailbox / ready -> aborts -> link watchdog
 *        (story 2.7) -> ocb_core_tick.
 *   TX:  ocb_link_tx_next() -> one whole frame, EVENT > NAK > INFO > ACK >
 *        STATUS; then ocb_link_tx_settled() completes a pending TX release.
 *
 * Admission (D5), per frame the splitter delivers (COBS, length, CRC and
 * type range already checked):
 *   1. any app frame (0x01..0x3F) updates the app-gap counter;
 *   2. bridge frames (0x40..0x7F): HEARTBEAT is stored (D10), sets link_ok
 *      and arms the watchdog (story 2.7), never ACKed, never reaches the
 *      core; other bridge types are counted and ignored;
 *   3. sync points: STOP (any seq) -> core, ACK; HELLO with a matching
 *      proto_ver -> core, session open, INFO; HELLO with another proto_ver
 *      or an empty body -> INFO, session closed, synthetic STOP to the core
 *      (its ACK discarded) (D8);
 *   4. session closed: NAK VERSION (semantic, expected = seq + 1);
 *   5. seq: d = (expected - seq) mod 256; d == 0 -> core; d == 1 -> the
 *      cached response; 2 <= d <= 64 -> ACK(expected - 1); else NAK SEQ;
 *   6. core semantics.
 * Transport NAKs (CRC, SEQ, FULL) share one budget: at most one per value
 * of `expected`, re-armed when `expected` changes and at a sync point (D6).
 * NAK fields (D7): expected = the controller's expected after handling,
 * got = the frame's seq (0 for CRC).
 *
 * `link_ok` (story 2.7 D3, D7): false at boot; set by every valid HEARTBEAT
 * (which also arms the link watchdog); cleared by a link loss (heartbeat
 * timeout, or a break while armed). While it is false, START and SET_STATE
 * get NAK NO_LINK. A loss aborts the core with LINK_LOST, queues EVENT
 * LINK_LOST (unless the latch was already set) and releases TX after the
 * EVENT has left (ocb_link_tx_settled). In DIRECT, the app timeout and the
 * HEARTBEAT DTR falling edge abort with APP_TIMEOUT (no EVENT). Rules in
 * ocb_link/watchdog.h and docs/link-protocol.md.
 *
 * STATUS.now_us (D13) is a u32 of microseconds since t0 from the core and
 * wraps after about 71 minutes of one piece; accepted, the wire width is
 * frozen.
 *
 * Every pointer argument must be non-NULL. One owner per instance (core 1),
 * except the embedded RX ring whose producer is the UART interrupt.
 */
#ifndef OCB_LINK_LINK_H
#define OCB_LINK_LINK_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "ocb_core/core.h"
#include "ocb_core/ramfunc.h"
#include "ocb_link/config.h"
#include "ocb_link/ready.h"
#include "ocb_link/rxring.h"
#include "ocb_link/txgate.h"
#include "ocb_link/watchdog.h"
#include "ocb_proto/frame.h"
#include "ocb_proto/msgs.h"

/* Values the glue passes in; the library reads no version header and no
 * build macro (tests pass anything). */
typedef struct {
    uint8_t fw_ver[3];       /* INFO.fw_ver: major, minor, patch */
    uint8_t hid_profile;     /* INFO.hid_profile, OCB_HID_PROFILE_* */
    uint16_t app_timeout_ms; /* INFO.app_timeout_ms and the DIRECT app timeout; 0 = default (story 2.7 D9) */
    uint16_t link_timeout_ms; /* heartbeat timeout; 0 = OCB_LINK_TIMEOUT_MS, clamped to OCB_LINK_TIMEOUT_MAX_MS */
    bool debug_counters;     /* append the 7 STATUS debug bytes (D12) */
} ocb_link_cfg;

/* Counters, u32, wrap. dropped_bytes (STATUS debug counter) =
 * rx_err_bytes + ring.dropped + overlong_bytes, saturated to u16. */
typedef struct {
    uint32_t rx_bytes;        /* error-free data bytes fed to the splitter */
    uint32_t rx_err_bytes;    /* entries with FE, PE, BE or OE: discarded */
    uint32_t rx_overflow;     /* ring overflow markers seen (gaps, not bytes) */
    uint32_t rx_err_runs;     /* runs in progress abandoned by an error or a gap */
    uint32_t breaks;          /* entries with BE (the link watchdog's break input) */
    uint32_t rx_fe;           /* entries with FE (framing error) */
    uint32_t rx_pe;           /* entries with PE (parity error) */
    uint32_t rx_oe;           /* entries with OE (overrun); an entry may set several bits */
    uint32_t overlong_bytes;  /* bytes of overlong runs discarded by the splitter */
    uint32_t app_frames;      /* valid app frames (0x01..0x3F) */
    uint32_t heartbeats;      /* valid HEARTBEATs */
    uint32_t heartbeat_bad;   /* HEARTBEATs with a short body (ignored) */
    uint32_t bridge_other;    /* other bridge types (0x41..0x7F), ignored */
    uint32_t nak_suppressed;  /* transport NAKs not sent (budget used) */
    uint32_t tx_frames;       /* frames returned by ocb_link_tx_next */
    uint32_t tx_dropped;      /* EVENT/NAK lost to a full ring or discarded by the closed gate */
} ocb_link_counters;

typedef struct {
    ocb_core *core;
    const ocb_usb_mailbox *mailbox;
    ocb_link_cfg cfg;
    uint16_t report_period_us;

    ocb_rxring ring;
    ocb_rx rx;
    ocb_frame frame; /* scratch for the splitter output */
    ocb_ready ready;
    ocb_txgate gate;

    bool session_open;

    /* Bridge heartbeat (D10). */
    bool link_ok;
    uint8_t bridge_ver[3];
    uint8_t hb_flags;
    uint64_t last_heartbeat_us;

    /* Link watchdog and the TX release sequence (story 2.7 D6). */
    ocb_wd wd;
    bool release_pending;      /* only EVENTs leave until the gate is released */
    uint64_t release_start_us; /* released at the latest OCB_LINK_RELEASE_WAIT_US after this */

    /* Cached response of the last core-handled non-sync frame (D6). */
    bool cache_valid;
    uint8_t cache_seq;
    uint8_t cache_nak; /* 0 = ACK, else the semantic NAK reason */

    /* Transport NAK budget (D6). */
    bool nak_spent;       /* a transport NAK was sent for nak_expected */
    uint8_t nak_expected; /* the value of expected the budget was spent on */

    /* TX slots (D12). */
    uint8_t ev_buf[OCB_LINK_TX_EVENT_CAP];
    uint8_t ev_head;
    uint8_t ev_count;
    ocb_msg_nak nak_buf[OCB_LINK_TX_NAK_CAP];
    uint8_t nak_head;
    uint8_t nak_count;
    bool info_pending;
    bool ack_pending;
    uint8_t ack_seq;
    bool status_requested;
    bool status_sent; /* false until the first STATUS: due at once */
    uint64_t last_status_us;
    uint8_t tx_seq; /* rolling seq of controller -> app frames, diagnostic */

    /* App-gap counter (D12). */
    bool have_app_frame;
    uint64_t last_app_frame_us;
    uint32_t max_app_gap_us;

    ocb_link_counters cnt;
} ocb_link;

/* Session closed, gate closed, link_ok 0, watchdog disarmed, everything
 * empty; the core is not touched (it must be initialised by the caller).
 * `ready` starts at 0 with the mailbox's current values taken as seen.
 * The effective app and link timeouts are stored back into link->cfg, so
 * INFO reports what the watchdog applies. */
void ocb_link_init(ocb_link *link, ocb_core *core, const ocb_usb_mailbox *mailbox, const ocb_link_cfg *cfg,
                   uint64_t now_us);

/* Re-takes the `ready` tracker's baseline from `baseline` instead of the
 * mailbox's values at init (D28): the firmware passes an all-zero mailbox so
 * that events core 0 posted before core 1 initialised the link are seen. */
void ocb_link_ready_rebase(ocb_link *link, const ocb_usb_mailbox *baseline, uint64_t now_us);

/* INFO.report_period_us, and the core's LATE threshold (0 = unknown). */
void ocb_link_set_report_period(ocb_link *link, uint16_t report_period_us);

/* The RX ring the UART interrupt pushes into (producer side). */
ocb_rxring *ocb_link_rx_ring(ocb_link *link);

/* The TX gate (sampled by the glue; released by the link watchdog). */
ocb_txgate *ocb_link_gate(ocb_link *link);

/* One RX entry: a data-register value (bits 0-7 data, 8 FE, 9 PE, 10 BE,
 * 11 OE) or OCB_LINK_RX_OVERFLOW_MARK. Error entries are never data (D4). */
OCB_CORE_RAMFUNC void ocb_link_rx_entry(ocb_link *link, uint16_t entry, uint64_t now_us);

/* Pops up to `max` entries from the embedded ring into ocb_link_rx_entry.
 * Returns the number handled. */
OCB_CORE_RAMFUNC uint32_t ocb_link_rx_drain(ocb_link *link, uint64_t now_us, uint32_t max);

/* USB mailbox and ready, the resulting aborts, the link watchdog (story
 * 2.7 D23: before the scheduler, so a dead link never applies a due event),
 * one scheduler pass. */
OCB_CORE_RAMFUNC void ocb_link_poll(ocb_link *link, uint64_t now_us);

/* The next whole wire frame (COBS plus the 0x00 delimiter) into out, or 0
 * when nothing is due, the gate is closed (pending output is then
 * discarded), or cap is smaller than the frame (it stays pending). While a
 * TX release is pending only EVENTs leave; everything else is discarded. */
OCB_CORE_RAMFUNC size_t ocb_link_tx_next(ocb_link *link, uint64_t now_us, uint8_t *out, size_t cap);

/* Called by the glue once per pass after it pumped TX (story 2.7 D6, D23).
 * tx_hw_idle: the driver's software ring, the TX FIFO and the shifter are
 * empty. A pending release completes (ocb_txgate_release) when the EVENT
 * ring is empty and tx_hw_idle, or when more than OCB_LINK_RELEASE_WAIT_US
 * have elapsed since it started. */
OCB_CORE_RAMFUNC void ocb_link_tx_settled(ocb_link *link, uint64_t now_us, bool tx_hw_idle);

/* The ready predicate the core and STATUS use. */
OCB_CORE_RAMFUNC bool ocb_link_console_ready(const ocb_link *link, uint64_t now_us);

/* Link state views (tests, the probe shim). */
uint32_t ocb_link_breaks(const ocb_link *link);
uint64_t ocb_link_last_heartbeat_us(const ocb_link *link);
bool ocb_link_link_ok(const ocb_link *link);
/* STATUS debug byte 2 (story 2.7 D12). */
uint8_t ocb_link_watchdog_trips(const ocb_link *link);
/* A TX release is pending (story 2.7 D6). */
bool ocb_link_release_pending(const ocb_link *link);
/* Queues an EVENT (for aborts detected outside the library). */
void ocb_link_queue_event(ocb_link *link, uint8_t code);

/* Read-only views. */
bool ocb_link_session_open(const ocb_link *link);
const ocb_link_counters *ocb_link_counters_get(const ocb_link *link);
/* rx_err_bytes + ring drops + overlong bytes (u32, wraps). */
uint32_t ocb_link_dropped_bytes(const ocb_link *link);
/* Largest gap between two consecutive app frames since the last session
 * open or close, ms, saturating at 65535. */
uint16_t ocb_link_max_app_gap_ms(const ocb_link *link);

#endif /* OCB_LINK_LINK_H */
