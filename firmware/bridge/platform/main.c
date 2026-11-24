/*
 * SPDX-License-Identifier: MIT
 *
 * Bridge firmware (Story 3.1, AD-4, AD-13, AD-20): USB CDC <-> UART at
 * 460800 with a frame-aware PC -> UART path, its own heartbeat every 20 ms
 * and the AD-20 probe. One core, one non-blocking super-loop; the logic is
 * in bridge/core (ocb_bridge_core, pure C), this file is the glue: TinyUSB,
 * the UART driver of ocb_hw, a repeating hardware alarm for the heartbeat
 * and the Story 2.1 LED blink (blue, 1 Hz).
 */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "hardware/sync.h"
#include "ocb_bridge/bridge.h"
#include "ocb_hw/led.h"
#include "ocb_hw/uart_link.h"
#include "ocb_proto/consts.h"
#include "ocb_version.h"
#include "pico/stdlib.h"
#include "tusb.h"

/* Story 2.1 colour and rhythm (Decisions D3, D5): about 6 % of full scale. */
#define BLINK_R 0u
#define BLINK_G 0u
#define BLINK_B 16u
#define BLINK_HALF_PERIOD_US 500000u

/* UART -> loop: data-register values from the RX interrupt (SPSC). */
#define RX_RING_CAP 1024u
#define RX_RING_MASK (RX_RING_CAP - 1u)
static volatile uint16_t s_rx_ring[RX_RING_CAP];
static volatile uint32_t s_rx_head; /* IRQ only */
static volatile uint32_t s_rx_tail; /* loop only */
static volatile uint32_t s_rx_ring_drops;

/* The heartbeat alarm's free-running tick counter (IRQ only writes it). */
static volatile uint32_t s_hb_ticks;

static ocb_bridge s_bridge;

static void rx_sink(uint16_t dr)
{
    const uint32_t head = s_rx_head;
    if (head - s_rx_tail >= RX_RING_CAP) {
        s_rx_ring_drops++;
        return;
    }
    s_rx_ring[head & RX_RING_MASK] = dr;
    __dmb();
    s_rx_head = head + 1u;
}

static bool hb_alarm(repeating_timer_t *t)
{
    (void)t;
    s_hb_ticks++;
    return true; /* keep repeating */
}

static void led_step(uint64_t now, bool *on, uint64_t *next_edge)
{
    if (now < *next_edge) {
        return;
    }
    const bool want_on = !*on;
    const bool queued = want_on ? ocb_led_set(BLINK_R, BLINK_G, BLINK_B) : ocb_led_set(0u, 0u, 0u);
    if (queued) {
        *on = want_on;
        /* From now, not from the missed edge: two writes are never closer
         * than a half period (WS2812 latch time). */
        *next_edge = now + BLINK_HALF_PERIOD_US;
    }
}

int main(void)
{
    ocb_led_init();
    (void)ocb_uart_link_init(); /* RX pull-down, TX high-Z (AD-20) */
    ocb_uart_link_rx_irq_enable(rx_sink);

    const uint8_t ver[3] = {(uint8_t)OCB_VERSION_MAJOR, (uint8_t)OCB_VERSION_MINOR,
                            (uint8_t)OCB_VERSION_PATCH};
    ocb_bridge_init(&s_bridge, ver);

    (void)tud_init(0u);

    /* A hardware alarm every OCB_HEARTBEAT_PERIOD_MS, whatever the USB state
     * (negative delay: period measured start to start). */
    static repeating_timer_t hb_timer;
    (void)add_repeating_timer_us(-(int64_t)OCB_HEARTBEAT_PERIOD_MS * 1000, hb_alarm, NULL, &hb_timer);

    static uint8_t staging[CFG_TUD_CDC_EP_BUFSIZE];
    size_t st_len = 0;
    size_t st_pos = 0;
    static uint8_t buf[OCB_MAX_WIRE_FRAME];
    bool tx_enabled = false;
    bool mounted = false;
    bool led_on = false;
    uint64_t led_edge = 0;

    for (;;) {
        tud_task();
        const uint64_t now = time_us_64();

        /* USB state: unmount and DTR falls drop the partial frame (and what
         * was read but not consumed: it belongs to that frame). */
        const bool m = tud_mounted();
        if (mounted && !m) {
            ocb_bridge_cdc_unmount(&s_bridge);
            st_len = st_pos = 0;
        }
        mounted = m;
        const bool dtr = m && tud_cdc_connected();
        if (s_bridge.dtr && !dtr) {
            st_len = st_pos = 0;
        }
        ocb_bridge_set_dtr(&s_bridge, dtr);

        /* PC -> framer (USB backpressure: read only when the staging buffer
         * is used up). */
        if (st_pos == st_len && m && tud_cdc_available() > 0u) {
            st_len = tud_cdc_read(staging, sizeof staging);
            st_pos = 0;
        }
        if (st_pos < st_len) {
            st_pos += ocb_bridge_cdc_in(&s_bridge, &staging[st_pos], st_len - st_pos);
        }

        ocb_bridge_hb_ticks(&s_bridge, s_hb_ticks);

        /* UART -> bridge (breaks and valid frames reach the probe). */
        while (s_rx_tail != s_rx_head) {
            const uint32_t tail = s_rx_tail;
            __dmb();
            const uint16_t dr = s_rx_ring[tail & RX_RING_MASK];
            s_rx_tail = tail + 1u;
            ocb_bridge_uart_rx(&s_bridge, dr);
        }

        /* The AD-20 probe decides whether TX is driven. */
        const bool tx = ocb_bridge_step(&s_bridge, now, ocb_uart_link_rx_level());
        if (tx != tx_enabled) {
            ocb_uart_link_tx_enable(tx);
            tx_enabled = tx;
        }

        /* Bridge -> UART: whole frames, heartbeats first at a boundary. */
        if (tx_enabled) {
            const size_t space = ocb_uart_link_tx_space();
            const size_t used = OCB_UART_LINK_TX_RING - space;
            const size_t n = ocb_bridge_uart_out(&s_bridge, buf, space, used);
            if (n > 0u) {
                (void)ocb_uart_link_tx_write(buf, n);
            }
            ocb_uart_link_tx_pump();
        }

        /* Bridge -> PC: best effort, never waits (AD-4). */
        const size_t cdc_space = m ? (size_t)tud_cdc_write_available() : 0u;
        const size_t k = ocb_bridge_cdc_out(&s_bridge, buf, cdc_space < sizeof buf ? cdc_space : sizeof buf);
        if (k > 0u) {
            (void)tud_cdc_write(buf, (uint32_t)k);
            (void)tud_cdc_write_flush();
        }

        led_step(now, &led_on, &led_edge);
    }
}
