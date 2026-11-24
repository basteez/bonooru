/*
 * SPDX-License-Identifier: MIT
 *
 * Host test F.39 (Story 3.1, AD-20): the bridge's probe state machine on a
 * fake clock (1 us steps). Pulses of exactly 5 ms every 500 ms (duty <= 1 %)
 * while RX stays low; RX high for 999 us is not enough, 1000 us links; a
 * valid frame links; once linked TX stays driven after the pulse; a break
 * unlinks at once; RX low for exactly 2000 us keeps the link, 2001 us loses
 * it; repeated link-up / loss cycles; heartbeats only when linked.
 */
#include <stdint.h>
#include <stdio.h>

#include "ocb_bridge/bridge.h"
#include "ocb_bridge/config.h"
#include "ocb_bridge/probe.h"
#include "ocb_proto/ocb_proto.h"
#include "ocb_test.h"

static void pulses_while_rx_low(void)
{
    ocb_bridge_probe p;
    ocb_bridge_probe_init(&p);
    uint64_t high_us = 0;
    uint64_t longest = 0;
    uint64_t run = 0;
    uint32_t rises = 0;
    bool prev = false;
    const uint64_t total = 10u * OCB_BRIDGE_PROBE_PERIOD_US;
    for (uint64_t t = 0; t < total; t++) {
        const bool tx = ocb_bridge_probe_step(&p, t, false);
        if (tx) {
            high_us++;
            run++;
            if (run > longest) {
                longest = run;
            }
            if (!prev) {
                rises++;
                OCB_CHECK(t % OCB_BRIDGE_PROBE_PERIOD_US == 0u, "pulse starts at %llu",
                          (unsigned long long)t);
            }
        } else {
            run = 0;
        }
        prev = tx;
    }
    OCB_CHECK(rises == 10u, "10 pulses in 5 s, got %u", rises);
    OCB_CHECK(longest == OCB_BRIDGE_PROBE_PULSE_US, "pulse %llu us", (unsigned long long)longest);
    OCB_CHECK(high_us * 100u <= total, "duty %.2f %%", 100.0 * (double)high_us / (double)total);
    OCB_CHECK(!ocb_bridge_probe_linked(&p), "never linked with RX low");
    OCB_CHECK(p.pulses == 10u, "pulses counted");
}

/* Probing from t=0; RX high from t=100 for `hold` us, then low. */
static bool linked_after_high(uint64_t hold)
{
    ocb_bridge_probe p;
    ocb_bridge_probe_init(&p);
    for (uint64_t t = 0; t < 100; t++) {
        ocb_bridge_probe_step(&p, t, false);
    }
    for (uint64_t t = 100; t < 100 + hold; t++) {
        ocb_bridge_probe_step(&p, t, true);
    }
    ocb_bridge_probe_step(&p, 100 + hold, false);
    return ocb_bridge_probe_linked(&p);
}

static void rx_high_threshold(void)
{
    /* A sample at t with RX high since t0 is "high for t - t0": 1000 us is
     * reached by the sample at t0 + 1000, i.e. 1001 samples. */
    OCB_CHECK(!linked_after_high(1000), "high for 999 us: not linked");
    OCB_CHECK(linked_after_high(1001), "high for 1000 us: linked");
}

static void linked_keeps_tx_after_the_pulse(void)
{
    ocb_bridge_probe p;
    ocb_bridge_probe_init(&p);
    bool tx = false;
    /* The controller answers the pulse 1 ms later and keeps its TX high. */
    for (uint64_t t = 0; t < 20000; t++) {
        tx = ocb_bridge_probe_step(&p, t, t >= 1000);
    }
    OCB_CHECK(ocb_bridge_probe_linked(&p), "linked during the pulse");
    OCB_CHECK(tx, "TX still driven 15 ms after the pulse would have ended");
    OCB_CHECK(p.link_ups == 1u, "one link-up");
}

static void a_valid_frame_links(void)
{
    ocb_bridge b;
    const uint8_t ver[3] = {0, 0, 0};
    ocb_bridge_init(&b, ver);
    ocb_bridge_step(&b, 0, false);
    uint8_t wire[16];
    const uint8_t body[1] = {5};
    const size_t n = ocb_frame_encode(OCB_MSG_ACK, 0, body, sizeof body, wire, sizeof wire);
    for (size_t i = 0; i < n; i++) {
        ocb_bridge_uart_rx(&b, wire[i]);
    }
    OCB_CHECK(ocb_bridge_step(&b, 10, false), "TX driven after a valid frame");
    OCB_CHECK(ocb_bridge_probe_linked(&b.probe), "linked on a valid frame");
    /* A damaged frame does not link. */
    ocb_bridge c;
    ocb_bridge_init(&c, ver);
    ocb_bridge_step(&c, 0, false);
    wire[2] ^= 0x10u;
    for (size_t i = 0; i < n; i++) {
        ocb_bridge_uart_rx(&c, wire[i]);
    }
    ocb_bridge_step(&c, 10, false);
    OCB_CHECK(!ocb_bridge_probe_linked(&c.probe), "a damaged frame does not link");
}

static void loss_rules(void)
{
    /* Break: back to probing at once, TX high-Z right after the next pulse
     * decision (the pulse starts at once, so check the state). */
    ocb_bridge_probe p;
    ocb_bridge_probe_init(&p);
    for (uint64_t t = 0; t <= 1000; t++) {
        ocb_bridge_probe_step(&p, t, true);
    }
    OCB_CHECK(ocb_bridge_probe_linked(&p), "linked");
    ocb_bridge_probe_break(&p);
    ocb_bridge_probe_step(&p, 2000, true);
    OCB_CHECK(!ocb_bridge_probe_linked(&p), "break: probing");
    OCB_CHECK(p.losses == 1u, "one loss");

    /* RX low: exactly 2000 us keeps the link, 2001 us loses it. */
    for (uint64_t low = 2000; low <= 2001; low++) {
        ocb_bridge_probe q;
        ocb_bridge_probe_init(&q);
        for (uint64_t t = 0; t <= 1000; t++) {
            ocb_bridge_probe_step(&q, t, true);
        }
        for (uint64_t t = 5000; t <= 5000 + low; t++) {
            ocb_bridge_probe_step(&q, t, false);
        }
        if (low == 2000) {
            OCB_CHECK(ocb_bridge_probe_linked(&q), "low for 2000 us: still linked");
        } else {
            OCB_CHECK(!ocb_bridge_probe_linked(&q), "low for 2001 us: lost");
        }
    }
    /* Short low glitches (data 0x00 characters) never trip it. */
    ocb_bridge_probe r;
    ocb_bridge_probe_init(&r);
    for (uint64_t t = 0; t <= 1000; t++) {
        ocb_bridge_probe_step(&r, t, true);
    }
    for (uint64_t t = 1001; t < 1001 + 100000; t++) {
        ocb_bridge_probe_step(&r, t, (t % 300u) > 20u);
    }
    OCB_CHECK(ocb_bridge_probe_linked(&r), "20 us lows every 300 us keep the link");
}

static void cycles(void)
{
    ocb_bridge_probe p;
    ocb_bridge_probe_init(&p);
    uint64_t t = 0;
    for (int c = 0; c < 20; c++) {
        /* probing for 1.2 s with RX low, then the controller answers */
        const uint64_t end_low = t + 1200000u;
        for (; t < end_low; t += 10) {
            ocb_bridge_probe_step(&p, t, false);
        }
        OCB_CHECK(!ocb_bridge_probe_linked(&p), "cycle %d: probing", c);
        const uint64_t end_high = t + 5000u;
        for (; t < end_high; t += 10) {
            ocb_bridge_probe_step(&p, t, true);
        }
        OCB_CHECK(ocb_bridge_probe_linked(&p), "cycle %d: linked", c);
        ocb_bridge_probe_break(&p);
        ocb_bridge_probe_step(&p, t, false);
    }
    OCB_CHECK(p.link_ups == 20u && p.losses == 20u, "20 cycles, ups %u losses %u", p.link_ups, p.losses);
}

static void heartbeats_only_when_linked(void)
{
    ocb_bridge b;
    const uint8_t ver[3] = {0, 0, 0};
    ocb_bridge_init(&b, ver);
    uint32_t ticks = 0;
    uint8_t out[64];
    size_t sent_probing = 0;
    for (uint64_t t = 0; t < 2000000u; t += 100) {
        if (t % 20000u == 0u) {
            ocb_bridge_hb_ticks(&b, ++ticks);
        }
        ocb_bridge_step(&b, t, false);
        sent_probing += ocb_bridge_uart_out(&b, out, sizeof out, 0);
    }
    OCB_CHECK(sent_probing == 0u, "no byte written while probing (%zu)", sent_probing);
    /* Link, then a loss abandons a heartbeat in flight. */
    ocb_bridge_step(&b, 2000000u, true);
    ocb_bridge_step(&b, 2001000u, true);
    ocb_bridge_hb_ticks(&b, ++ticks);
    OCB_CHECK(ocb_bridge_uart_out(&b, out, 3, 0) == 3u, "heartbeat started");
    ocb_bridge_uart_rx(&b, 0x400u); /* break */
    ocb_bridge_step(&b, 2001100u, false);
    OCB_CHECK(b.stats.inflight_abandoned == 1u, "the cut heartbeat counted");
    OCB_CHECK(ocb_bridge_uart_out(&b, out, sizeof out, 0) == 0u, "nothing after the loss");
}

int main(void)
{
    pulses_while_rx_low();
    rx_high_threshold();
    linked_keeps_tx_after_the_pulse();
    a_valid_frame_links();
    loss_rules();
    cycles();
    heartbeats_only_when_linked();
    if (ocb_test_failures() != 0) {
        fprintf(stderr, "test_bridge_probe: %d failure(s)\n", ocb_test_failures());
        return 1;
    }
    printf("test_bridge_probe: ok\n");
    return 0;
}
