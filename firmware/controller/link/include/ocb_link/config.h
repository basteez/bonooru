/*
 * SPDX-License-Identifier: MIT
 *
 * Local constants of the controller link layer (story 2.6 D12, D15, D17).
 * None of them is a wire value: they size buffers and tune timing on the
 * controller only, so they live here and not in ocb_proto/consts.h (no
 * vector, no Rust mirror, no PROTO_VERSION bump).
 */
#ifndef OCB_LINK_CONFIG_H
#define OCB_LINK_CONFIG_H

/* RX ring entries (u16 each, 1 KB). A power of two (index masking). At
 * 460800 baud a character takes 21.7 us, so 512 entries absorb a stalled
 * loop of more than 11 ms. */
#define OCB_LINK_RX_RING_CAP 512

/* TX slots (D12): EVENT ring and NAK ring capacities. */
#define OCB_LINK_TX_EVENT_CAP 8
#define OCB_LINK_TX_NAK_CAP 4

/* TX gate (AD-20, D15): RX must be high continuously this long before the
 * TX pad is driven. 1000 us = 460 bit times at 460800 baud: longer than any
 * run of ones inside a frame, shorter than the 20 ms heartbeat period. A
 * hypothesis to tune at the bench (test plan H.7). */
#define OCB_LINK_TX_IDLE_HIGH_US 1000

/* TX release bound (story 2.7 D6): after a link loss or a break the TX
 * gate is released once the EVENT LINK_LOST has left and the UART is idle,
 * or at the latest this long after the release started. 5000 us covers the
 * worst backlog (128-byte driver ring + 32-byte FIFO = 3.5 ms at 460800).
 * Local, not a wire value. */
#define OCB_LINK_RELEASE_WAIT_US 5000

/* STATUS debug counters (D12, D35), little-endian, after the 11 base bytes.
 * v1 prefix (7 bytes): max_app_gap_ms u16, watchdog_trips u8,
 * dropped_bytes u16, tx_dropped u16. v2 adds the causes of dropped_bytes
 * (14 bytes): rx_err_bytes u16, rx_overflow_bytes u16, overlong_bytes u16,
 * and the error bits seen, rx_fe u16, rx_pe u16, rx_be u16, rx_oe u16.
 * Every u16 saturates at 65535. */
#define OCB_LINK_DEBUG_COUNTERS_V1_LEN 7
#define OCB_LINK_DEBUG_COUNTERS_LEN 21
/* v3 (story 2.3 D14) appends the P2 debug log window: p2_log_total u16,
 * p2_log_n u8 (0..16), then p2_log_n records of 2 bytes (kind, id), oldest
 * first. v1 to v3: 24 bytes with an empty window, at most 56. */
#define OCB_LINK_DEBUG_V3_HEADER_LEN 3
#define OCB_LINK_DEBUG_V3_MAX_LEN 56
/* v4 (story 2.5 D13): one fixed block after the v3 window, at offset
 * 24 + 2 * p2_log_n of the tail, every u16 saturating:
 *   0 late_n u16, 2 late_max_us u16, 4 late_sum_us u32, 8 late_hist u16 x 6,
 *  20 loop_max_us u16, 22 core0_pass_max_us u16, 24 in_min_us u16,
 *  26 in_p95_us u16, 28 in_max_us u16, 30 in_slow u16, 32 seq_retries u16,
 *  34 seq_fallbacks u8, 35 stack_used_max u16, 37 boot_cause u8,
 *  38 reboot_count u8, 39 stall_reset_10us u16 (D31: the board's own
 *  stall-to-reset interval of the last liveness reboot, 10 us units).
 */
#define OCB_LINK_DEBUG_V4_LEN 41
/* v5 (story 3.5): one fixed block right after v4, the hardware checks'
 * timing, every u16 saturating:
 *   0 loss_age_10us u16 (the last link loss: last valid heartbeat to the
 *     abort, 10 us units; 0 before any loss), 2 loss_kind u8 (0 none,
 *     1 heartbeat timeout, 2 break), 3 in_age_ms u16 (time since the last
 *     IN report or handshake when the STATUS was built, ms: the clock of
 *     the 100 ms polling-stall rule).
 * v1 to v5: 70 bytes with an empty window, at most 102. */
#define OCB_LINK_DEBUG_V5_LEN 5
/* v6 (story 4.0): one fixed block right after v5:
 *   0 p2_queue_full u16 (the P2 replies dropped because the profile's
 *     4-entry reply queue was full, since boot, saturating; posted by core
 *     0 through the USB mailbox).
 * Debug tail: 72 bytes with an empty window, at most 104; the largest
 * debug STATUS body is 11 + 104 = 115 bytes. */
#define OCB_LINK_DEBUG_V6_LEN 2
#define OCB_LINK_DEBUG_TAIL_MAX_LEN 104

/* RX ring entry layout: the PL011 data register value (dr & 0x0FFF). */
#define OCB_LINK_RX_DATA_MASK 0x00FFu
#define OCB_LINK_RX_FE 0x0100u /* framing error */
#define OCB_LINK_RX_PE 0x0200u /* parity error */
#define OCB_LINK_RX_BE 0x0400u /* break */
#define OCB_LINK_RX_OE 0x0800u /* overrun */
#define OCB_LINK_RX_ERR_MASK 0x0F00u
/* Not a register bit: the ring inserts this marker where it had to drop
 * entries (D19). */
#define OCB_LINK_RX_OVERFLOW_MARK 0x1000u

/* Memory barrier between a slot write and the index (or generation)
 * publication of the RX ring, the debug log ring and the published-state
 * double buffer. Their storage and indices are volatile, which keeps the
 * stores in order for one core; across the two cores the firmware builds
 * force-include controller/platform/ocb_link_barrier.h, which defines it as
 * a real DMB (story 2.5 D5, D24; tools/check-barrier.sh checks the ELF).
 * Empty on the host (single-threaded tests). */
#ifndef OCB_LINK_BARRIER
#define OCB_LINK_BARRIER() ((void)0)
#endif

#endif /* OCB_LINK_CONFIG_H */
