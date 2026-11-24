/*
 * SPDX-License-Identifier: MIT
 *
 * Host test F.41 (Story 3.1, AD-4): the bridge's UART -> PC path. Clean
 * bytes in order; FE, PE, OE values dropped and counted; BE a break, never
 * forwarded; short CDC space: the excess dropped and counted, never waited
 * for; a full ring counted.
 */
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "ocb_bridge/bridge.h"
#include "ocb_bridge/rxfwd.h"
#include "ocb_test.h"

static void clean_bytes_in_order_and_errors(void)
{
    ocb_bridge_rxfwd r;
    ocb_bridge_rxfwd_init(&r);
    uint8_t b = 0;
    const uint16_t in[] = {0x011, 0x022, 0x133, 0x244, 0x855, 0x466, 0x077, 0xF88, 0x000};
    int res[9];
    for (size_t i = 0; i < 9; i++) {
        res[i] = ocb_bridge_rxfwd_push(&r, in[i], &b);
    }
    OCB_CHECK(res[0] == 1 && res[1] == 1 && res[6] == 1 && res[8] == 1, "clean bytes");
    OCB_CHECK(res[2] == 0 && res[3] == 0 && res[4] == 0, "FE, PE, OE: not data");
    OCB_CHECK(res[5] == -1 && res[7] == -1, "BE (alone or with others): a break");
    uint8_t out[16];
    const size_t n = ocb_bridge_rxfwd_take(&r, out, sizeof out);
    const uint8_t want[4] = {0x11, 0x22, 0x77, 0x00};
    OCB_CHECK_MEM(out, n, want, sizeof want, "clean bytes in order");
    OCB_CHECK(r.stats.err_bytes == 3u && r.stats.breaks == 2u, "counters err %u breaks %u",
              r.stats.err_bytes, r.stats.breaks);
}

static void short_cdc_space_waits(void)
{
    ocb_bridge_rxfwd r;
    ocb_bridge_rxfwd_init(&r);
    uint8_t b = 0;
    for (uint16_t i = 0; i < 100; i++) {
        ocb_bridge_rxfwd_push(&r, (uint16_t)(i & 0xFFu), &b);
    }
    uint8_t out[128];
    size_t n = ocb_bridge_rxfwd_take(&r, out, 30);
    OCB_CHECK(n == 30u && out[0] == 0u && out[29] == 29u, "the first 30 bytes");
    OCB_CHECK(r.stats.cdc_drops == 0u, "the rest waits, nothing dropped");
    n = ocb_bridge_rxfwd_take(&r, out, sizeof out);
    OCB_CHECK(n == 70u && out[0] == 30u && out[69] == 99u, "the other 70 on the next pass, in order");
    ocb_bridge_rxfwd_push(&r, 0x42u, &b);
    n = ocb_bridge_rxfwd_take(&r, out, 0);
    OCB_CHECK(n == 0u && r.stats.cdc_drops == 0u, "space 0: kept, never waited for");
}

static void ring_overflow_counted(void)
{
    ocb_bridge_rxfwd r;
    ocb_bridge_rxfwd_init(&r);
    uint8_t b = 0;
    for (uint32_t i = 0; i < OCB_BRIDGE_RXFWD_CAP + 10u; i++) {
        ocb_bridge_rxfwd_push(&r, (uint16_t)(i & 0xFFu), &b);
    }
    OCB_CHECK(r.stats.cdc_drops == 10u, "10 dropped (ring full), got %u", r.stats.cdc_drops);
    static uint8_t out[OCB_BRIDGE_RXFWD_CAP];
    const size_t n = ocb_bridge_rxfwd_take(&r, out, sizeof out);
    OCB_CHECK(n == OCB_BRIDGE_RXFWD_CAP && out[0] == 0u, "the first CAP bytes kept");
}

static void bridge_breaks_reach_the_probe(void)
{
    ocb_bridge br;
    const uint8_t ver[3] = {0, 0, 0};
    ocb_bridge_init(&br, ver);
    ocb_bridge_step(&br, 0, true);
    ocb_bridge_step(&br, 1000, true);
    OCB_CHECK(ocb_bridge_probe_linked(&br.probe), "linked");
    ocb_bridge_uart_rx(&br, 0x100u); /* FE alone: noise, not a break */
    ocb_bridge_step(&br, 1100, true);
    OCB_CHECK(ocb_bridge_probe_linked(&br.probe), "a framing error is not a link loss");
    ocb_bridge_uart_rx(&br, 0x500u); /* BE (with FE): a break */
    ocb_bridge_step(&br, 1200, true);
    OCB_CHECK(!ocb_bridge_probe_linked(&br.probe), "a break is a link loss");
    uint8_t out[8];
    OCB_CHECK(ocb_bridge_cdc_out(&br, out, sizeof out) == 0u, "nothing forwarded");
}

int main(void)
{
    clean_bytes_in_order_and_errors();
    short_cdc_space_waits();
    ring_overflow_counted();
    bridge_breaks_reach_the_probe();
    if (ocb_test_failures() != 0) {
        fprintf(stderr, "test_bridge_rxfwd: %d failure(s)\n", ocb_test_failures());
        return 1;
    }
    printf("test_bridge_rxfwd: ok\n");
    return 0;
}
