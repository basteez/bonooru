/*
 * SPDX-License-Identifier: MIT
 *
 * UART driver of the board link (story 2.6 D14, D15). Plain pico-sdk API
 * (hardware_uart, hardware_gpio, hardware_irq), no code copied from
 * elsewhere. Facts from the pinned SDK 2.3.1 sources and the RP2040
 * datasheet (UART chapter):
 *  - the PL011 has a 32-entry TX FIFO (8 bits) and a 32-entry RX FIFO
 *    (12 bits: data plus FE bit 8, PE bit 9, BE bit 10, OE bit 11);
 *  - uart_set_irqs_enabled(rx = true) enables the RX interrupt at
 *    RXIFLSEL 0 (FIFO at least 1/8 full, i.e. 4 characters) and the RX
 *    timeout interrupt (at least one character, nothing new for 32 bit
 *    periods, about 70 us at 460800 baud);
 *  - a break loads a single 0x00 character with BE set; the next character
 *    is received only after the line goes high again;
 *  - uart_init also sets the DMA request enables, but no DMA channel is
 *    claimed here, so no DMA runs (AC1: no 8-bit DMA).
 */
#include "ocb_hw/uart_link.h"

#include "hardware/gpio.h"
#include "hardware/irq.h"
#include "hardware/uart.h"
#include "ocb_hw/pins.h"
#include "ocb_proto/consts.h"

#define LINK_UART uart0
#define LINK_UART_IRQ UART0_IRQ
#define TX_MASK (OCB_UART_LINK_TX_RING - 1u)

/* One writer per index: the step writes s_tx_head (and drains, so also
 * s_tx_tail); no interrupt touches the TX ring. */
static uint8_t s_tx_buf[OCB_UART_LINK_TX_RING];
static uint32_t s_tx_head;
static uint32_t s_tx_tail;
static ocb_uart_link_rx_sink s_rx_sink;

typedef char ocb_uart_tx_ring_pow2[((OCB_UART_LINK_TX_RING & TX_MASK) == 0u) ? 1 : -1];

uint32_t ocb_uart_link_init(void)
{
    /* TX: a high-Z input until the gate opens (AD-20). */
    gpio_init(OCB_PIN_UART_TX);
    gpio_set_dir(OCB_PIN_UART_TX, GPIO_IN);
    gpio_disable_pulls(OCB_PIN_UART_TX);
    /* RX: pull-down, so an unpowered peer reads as low (a break), never as
     * a floating idle-high. */
    gpio_set_pulls(OCB_PIN_UART_RX, false, true);

    const uint baud = uart_init(LINK_UART, OCB_LINK_BAUD);
    uart_set_format(LINK_UART, 8, 1, UART_PARITY_NONE);
    uart_set_hw_flow(LINK_UART, false, false);
    uart_set_fifo_enabled(LINK_UART, true);
    gpio_set_function(OCB_PIN_UART_RX, UART_FUNCSEL_NUM(LINK_UART, OCB_PIN_UART_RX));
    s_tx_head = 0u;
    s_tx_tail = 0u;
    return (uint32_t)baud;
}

/* Drains the RX FIFO until empty: every data-register value goes to the
 * sink with its error bits; reading DR pops the FIFO. */
static void __not_in_flash_func(on_uart_rx)(void)
{
    uart_hw_t *hw = uart_get_hw(LINK_UART);
    while ((hw->fr & UART_UARTFR_RXFE_BITS) == 0u) {
        const uint16_t dr = (uint16_t)(hw->dr & 0x0FFFu);
        s_rx_sink(dr);
    }
    hw->icr = UART_UARTICR_RXIC_BITS | UART_UARTICR_RTIC_BITS;
}

void ocb_uart_link_rx_irq_enable(ocb_uart_link_rx_sink sink)
{
    s_rx_sink = sink;
    irq_set_exclusive_handler(LINK_UART_IRQ, on_uart_rx);
    irq_set_enabled(LINK_UART_IRQ, true);
    uart_set_irqs_enabled(LINK_UART, true, false);
}

bool __not_in_flash_func(ocb_uart_link_rx_level)(void)
{
    return gpio_get(OCB_PIN_UART_RX);
}

void ocb_uart_link_tx_enable(bool enable)
{
    if (enable) {
        gpio_set_function(OCB_PIN_UART_TX, UART_FUNCSEL_NUM(LINK_UART, OCB_PIN_UART_TX));
    } else {
        gpio_init(OCB_PIN_UART_TX);
        gpio_set_dir(OCB_PIN_UART_TX, GPIO_IN);
        gpio_disable_pulls(OCB_PIN_UART_TX);
        s_tx_tail = s_tx_head;
    }
}

size_t __not_in_flash_func(ocb_uart_link_tx_space)(void)
{
    return (size_t)(OCB_UART_LINK_TX_RING - (s_tx_head - s_tx_tail));
}

bool __not_in_flash_func(ocb_uart_link_tx_write)(const uint8_t *data, size_t n)
{
    if (n > ocb_uart_link_tx_space()) {
        return false;
    }
    for (size_t i = 0; i < n; i++) {
        s_tx_buf[s_tx_head & TX_MASK] = data[i];
        s_tx_head++;
    }
    return true;
}

void __not_in_flash_func(ocb_uart_link_tx_pump)(void)
{
    uart_hw_t *hw = uart_get_hw(LINK_UART);
    while (s_tx_tail != s_tx_head && (hw->fr & UART_UARTFR_TXFF_BITS) == 0u) {
        hw->dr = s_tx_buf[s_tx_tail & TX_MASK];
        s_tx_tail++;
    }
}
