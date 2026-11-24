/*
 * SPDX-License-Identifier: MIT
 *
 * ctypes shim of the bridge core (Story 3.1, test F.42): one ocb_bridge
 * instance driven by tests/test_link_probe.py on a fake clock, between the
 * bench probe and the real controller link (ocb_link_sim). Nothing here
 * re-implements bridge logic: every call goes to ocb_bridge_core's sources.
 */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "ocb_bridge/bridge.h"

/* Exported for ctypes; prototypes keep -Wmissing-prototypes quiet. */
void bs_init(uint8_t major, uint8_t minor, uint8_t patch);
size_t bs_cdc_in(const uint8_t *data, size_t n);
void bs_set_dtr(int dtr);
void bs_uart_rx(uint16_t dr);
size_t bs_cdc_out(uint8_t *out, size_t space);
void bs_hb_ticks(uint32_t ticks);
int bs_step(uint64_t now_us, int rx_high);
size_t bs_uart_out(uint8_t *out, size_t cap, size_t ring_used);
int bs_linked(void);
uint32_t bs_counter(int which);

static ocb_bridge s_b;

void bs_init(uint8_t major, uint8_t minor, uint8_t patch)
{
    const uint8_t ver[3] = {major, minor, patch};
    ocb_bridge_init(&s_b, ver);
}

size_t bs_cdc_in(const uint8_t *data, size_t n)
{
    return ocb_bridge_cdc_in(&s_b, data, n);
}

void bs_set_dtr(int dtr)
{
    ocb_bridge_set_dtr(&s_b, dtr != 0);
}

void bs_uart_rx(uint16_t dr)
{
    ocb_bridge_uart_rx(&s_b, dr);
}

size_t bs_cdc_out(uint8_t *out, size_t space)
{
    return ocb_bridge_cdc_out(&s_b, out, space);
}

void bs_hb_ticks(uint32_t ticks)
{
    ocb_bridge_hb_ticks(&s_b, ticks);
}

int bs_step(uint64_t now_us, int rx_high)
{
    return ocb_bridge_step(&s_b, now_us, rx_high != 0) ? 1 : 0;
}

size_t bs_uart_out(uint8_t *out, size_t cap, size_t ring_used)
{
    return ocb_bridge_uart_out(&s_b, out, cap, ring_used);
}

int bs_linked(void)
{
    return ocb_bridge_probe_linked(&s_b.probe) ? 1 : 0;
}

uint32_t bs_counter(int which)
{
    switch (which) {
    case 0:
        return s_b.stats.hb_sent;
    case 1:
        return s_b.stats.app_frames;
    case 2:
        return s_b.probe.link_ups;
    case 3:
        return s_b.probe.losses;
    case 4:
        return s_b.framer.stats.overlong;
    case 5:
        return s_b.rx.stats.breaks;
    case 6:
        return s_b.rx.stats.cdc_drops;
    case 7:
        return s_b.probe.pulses;
    default:
        return 0;
    }
}
