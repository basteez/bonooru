/*
 * SPDX-License-Identifier: MIT
 *
 * ocb_p2: the P2 profile (wired Pro Controller) of the controller board,
 * story 2.3. The AD-12 interface, pure C99 (no clock, no I/O, no
 * allocation, no global state):
 *   descriptors   ocb_p2/desc.h
 *   control/OUT   ocb_p2_on_output() (and ocb_p2_on_feature(): refused)
 *   build_report  ocb_p2_next_in() / ocb_p2_build_report(), from a const
 *                 ocb_state * only
 *   tick()        ocb_p2_tick()
 *   ready()       ocb_p2_ready()
 *
 * The handshake is reactive (D5): the host drives, the profile answers each
 * USB 0x80 command and each subcommand by the tables of
 * docs/usb-controller-emulation.md ("P2 wire protocol"), in any order and
 * count. ready() is true once the host asked for the input stream (USB
 * command 0x80 0x04, or subcommand 0x03 with argument 0x30, whichever comes
 * first) and false again after 0x80 0x05 or a restart. Anything that writes,
 * erases, pairs, resets or updates is refused and changes nothing (C7, D9).
 *
 * Every pointer argument but `log` must be non-NULL unless stated. One
 * owner per instance (core 0 on the board).
 */
#ifndef OCB_P2_P2_H
#define OCB_P2_P2_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "ocb_p2/config.h"
#include "ocb_proto/state.h"

/* What the board gives the profile (D10): the MAC in display (big-endian)
 * order, built by the glue from the board's unique id. */
typedef struct {
    uint8_t mac[6];
} ocb_p2_cfg;

/* Result of one OUT report. */
typedef enum {
    OCB_P2_RX_HANDLED = 0,  /* understood; a reply was queued if one is due */
    OCB_P2_RX_REFUSED,      /* refused (C7, D9): the reply says so, nothing changed */
    OCB_P2_RX_IGNORED,      /* unknown report id or USB command: no reply */
    OCB_P2_RX_MALFORMED,    /* too short to carry its header: no reply */
    OCB_P2_RX_QUEUE_FULL    /* a reply was due but the queue was full: dropped */
} ocb_p2_rx;

/* Up to two debug-log records per OUT report (kind, id), OCB_P2_LOG_*. */
typedef struct {
    uint8_t n;
    uint8_t kind[2];
    uint8_t id[2];
} ocb_p2_log;

/* Counters, u32, wrap (monotonic between wraps). */
typedef struct {
    uint32_t out_reports;  /* calls of ocb_p2_on_output */
    uint32_t replies;      /* replies queued */
    uint32_t refusals;     /* refused requests (C7) */
    uint32_t ignored;
    uint32_t malformed;
    uint32_t queue_full;
    uint32_t in_reports;   /* reports returned by ocb_p2_next_in */
    uint32_t restarts;
    uint32_t handshakes;   /* streaming enabled after a restart */
} ocb_p2_counters;

/* IN interval measurer (D13): a histogram of 50 us bins over the last 256
 * intervals between IN completions, the median published when at least 64
 * are in. */
typedef struct {
    uint16_t bins[OCB_P2_IV_BINS];
    uint16_t ring[OCB_P2_IV_WINDOW]; /* bin index of each sample in the window */
    uint16_t ring_head;
    uint16_t count;          /* samples in the window (<= 256) */
    uint32_t last_us;        /* low 32 bits of the previous completion */
    bool have_last;
    bool dirty;
    uint32_t median_us;      /* 0 until OCB_P2_IV_MIN_SAMPLES */
    uint32_t ops_note;       /* work of the last note (test: bounded cost) */
    uint32_t ops_median;     /* work of the last median computation */
} ocb_p2_interval;

typedef struct {
    ocb_p2_cfg cfg;
    bool streaming;          /* the 0x30 stream is on: ready() */
    bool handshake_pending;  /* streaming was enabled since the restart, not yet taken */
    bool handshake_seen;     /* streaming was enabled at least once since the restart */
    uint8_t timer;           /* report byte 1, +1 per 0x21 or 0x30 report */
    uint8_t player_lights;   /* subcommand 0x30 argument */
    uint8_t input_mode;      /* subcommand 0x03 argument (0 until set) */
    bool imu_enabled;        /* subcommand 0x40 */
    bool vibration_enabled;  /* subcommand 0x48 */
    uint8_t q[OCB_P2_QUEUE_DEPTH][OCB_P2_REPORT_LEN];
    uint8_t q_head;
    uint8_t q_count;
    ocb_p2_counters cnt;
    ocb_p2_interval iv;
} ocb_p2;

/* Everything cleared, streaming off, timer 0. */
void ocb_p2_init(ocb_p2 *p, const ocb_p2_cfg *cfg);

/* Bus reset, unmount, suspend or a new SET_CONFIGURATION (AD-12 "restarts
 * its handshake", D11): streaming off, ready false, queue emptied, the
 * interval measurer cleared (report period 0). The timer, the stored
 * settings and the counters are kept. */
void ocb_p2_restart(ocb_p2 *p);

/* One OUT report as TinyUSB delivers it on the interrupt endpoint: buf[0]
 * is the report id. Any len (0 included) and any bytes; buf may be NULL
 * when len is 0. Never reads past len. `log` (may be NULL) receives the
 * debug records (D14). */
ocb_p2_rx ocb_p2_on_output(ocb_p2 *p, const uint8_t *buf, size_t len, ocb_p2_log *log);

/* A SET_REPORT of type FEATURE (S2: features 0x70 to 0x74 unlock and write
 * the controller's memory, the OTA update path): always refused, nothing
 * changes, no reply (D26). */
ocb_p2_rx ocb_p2_on_feature(ocb_p2 *p, uint8_t report_id, ocb_p2_log *log);

/* The next IN report into out (64 bytes): the oldest queued reply (a 0x21
 * reply gets the current header from `published` and the next timer value),
 * else, while streaming, the 0x30 report of `published`, else nothing.
 * Returns 64, or 0 with nothing written and nothing consumed when there is
 * nothing to send, out or published is NULL, or cap < 64. */
size_t ocb_p2_next_in(ocb_p2 *p, const ocb_state *published, uint8_t *out, size_t cap);

/* The 0x30 report of `s` (D8), timer +1. Returns 64, or 0 with nothing
 * written when out or s is NULL or cap < 64. */
size_t ocb_p2_build_report(ocb_p2 *p, const ocb_state *s, uint8_t *out, size_t cap);

/* AD-12 tick(): recomputes the published report period when new interval
 * samples arrived (bounded cost, D13). `now_us` is not stored. */
void ocb_p2_tick(ocb_p2 *p, uint64_t now_us);

/* AD-12 ready(): the host asked for the input stream since the last
 * restart and did not stop it (D5). */
bool ocb_p2_ready(const ocb_p2 *p);

/* True once per restart, the first time streaming was enabled: the glue
 * posts ocb_usb_mailbox_handshake_done (D5). */
bool ocb_p2_take_handshake(ocb_p2 *p);

/* A reply is waiting in the queue. */
bool ocb_p2_reply_pending(const ocb_p2 *p);

/* IN interval measurement (D13). note: one IN completion at now_us, O(1);
 * only the low 32 bits are used, so 32-bit and 64-bit clocks wrap alike.
 * Intervals above 20 ms (the host stopped polling) and backwards steps are
 * not counted. report_period_us: the median of the last 256 intervals once
 * 64 are in (after ocb_p2_tick), else 0; saturated to 65535 by the caller. */
void ocb_p2_interval_note(ocb_p2 *p, uint64_t now_us);
uint32_t ocb_p2_report_period_us(const ocb_p2 *p);

#endif /* OCB_P2_P2_H */
