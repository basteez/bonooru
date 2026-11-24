/*
 * SPDX-License-Identifier: MIT
 *
 * Bridge-only tuning values (Story 3.1). Not wire values: they never leave
 * the bridge, so they live here and not in ocb_proto/consts.h.
 */
#ifndef OCB_BRIDGE_CONFIG_H
#define OCB_BRIDGE_CONFIG_H

/* AD-20 probe: drive TX idle-high this long ... */
#define OCB_BRIDGE_PROBE_PULSE_US 5000u
/* ... every this long (duty 1 %) while RX does not show the controller. */
#define OCB_BRIDGE_PROBE_PERIOD_US 500000u
/* After a link loss TX stays high-Z this long before the next probe pulse,
 * so the controller sees a break and releases its TX (review fix I-1). */
#define OCB_BRIDGE_PROBE_RELEASE_US 10000u
/* RX continuously high this long = the controller drives its TX: linked.
 * The controller enables TX 1 ms after it sees our pulse
 * (OCB_LINK_TX_IDLE_HIGH_US), so a 5 ms pulse leaves about 4 ms. */
#define OCB_BRIDGE_RX_HIGH_US 1000u
/* RX low (sampled once per pass) with no character received for more than
 * this while linked = link loss. Received characters reset it, so data bits
 * never trip it (review fix I-8); a line held low also gives a break. */
#define OCB_BRIDGE_RX_LOW_US 2000u
/* At most this many bytes waiting in the UART software ring (plus the
 * 32-entry hardware FIFO): bounds the delay of a due heartbeat to one
 * maximum frame plus 48 bytes, so every heartbeat gap stays <= 27 ms (F.7). */
#define OCB_BRIDGE_UART_AHEAD_MAX 8u
/* UART -> PC bytes waiting for CDC space (power of two). */
#define OCB_BRIDGE_RXFWD_CAP 512u

#endif /* OCB_BRIDGE_CONFIG_H */
