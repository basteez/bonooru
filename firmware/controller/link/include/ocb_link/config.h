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

/* Compiler barrier between the slot write and the index publication of the
 * RX ring. The ring's storage and indices are volatile, which already keeps
 * the two stores in order on one core; a build may still define a stronger
 * barrier through a compile definition. Empty on the host. */
#ifndef OCB_LINK_BARRIER
#define OCB_LINK_BARRIER() ((void)0)
#endif

#endif /* OCB_LINK_CONFIG_H */
