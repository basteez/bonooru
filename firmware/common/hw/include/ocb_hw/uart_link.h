/*
 * SPDX-License-Identifier: MIT
 *
 * Register-level UART driver for the PC <-> controller board link (AD-4,
 * AD-5, AD-20; story 2.6 D14, D15). Shared by both boards; it knows
 * nothing about frames: RX hands every data-register value (data byte
 * plus the FE, PE, BE, OE bits) to a sink, TX takes whole byte strings
 * all-or-nothing. UART0 at OCB_LINK_BAUD, 8N1, FIFOs on, IRQ-driven RX,
 * no DMA channel. Pins come from ocb_hw/pins.h only.
 *
 * Power-up rule (AD-20): the RX pad gets an internal pull-down and the TX
 * pad stays a high-Z input until ocb_uart_link_tx_enable(true); the
 * caller decides when (the controller: after RX was idle-high for
 * OCB_LINK_TX_IDLE_HIGH_US, ocb_link's TX gate).
 */
#ifndef OCB_HW_UART_LINK_H
#define OCB_HW_UART_LINK_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/* Software TX ring size: the step drains it into the 32-entry hardware
 * FIFO without ever waiting. */
#define OCB_UART_LINK_TX_RING 128u

/* Receives one data-register value (dr & 0x0FFF), in the RX interrupt. */
typedef void (*ocb_uart_link_rx_sink)(uint16_t dr);

/* UART0 at OCB_LINK_BAUD 8N1 with FIFOs, RX pin to the UART with a
 * pull-down, TX pin a high-Z input (no pull). Returns the achieved baud
 * rate. No interrupt is enabled yet. */
uint32_t ocb_uart_link_init(void);

/* Installs the RX handler and enables the RX and RX-timeout interrupts.
 * The NVIC enable acts on the calling core: call it from the core that
 * owns the link. */
void ocb_uart_link_rx_irq_enable(ocb_uart_link_rx_sink sink);

/* The RX pad level (true = high, idle). Readable while the UART owns the
 * pin: the SDK's function select keeps the pad's input enabled. */
bool ocb_uart_link_rx_level(void);

/* true: the TX pad becomes the UART's TX. false: back to a high-Z input
 * (no pull) and the software TX ring is emptied. */
void ocb_uart_link_tx_enable(bool enable);

/* Free bytes in the software TX ring. */
size_t ocb_uart_link_tx_space(void);

/* Copies all n bytes into the software TX ring, or nothing (returns
 * false) when they do not fit. Never blocks. */
bool ocb_uart_link_tx_write(const uint8_t *data, size_t n);

/* Moves bytes from the software ring into the hardware FIFO while it has
 * room. Never waits. Call once per loop pass. */
void ocb_uart_link_tx_pump(void);

/* true when everything written has left the pin: the software ring is
 * empty, the TX FIFO is empty (UARTFR.TXFE) and the shift register has sent
 * the last stop bit (UARTFR.BUSY clear). Story 2.7: the link watchdog
 * releases TX only then (or after its 5 ms bound). Never waits. */
bool ocb_uart_link_tx_idle(void);

#endif /* OCB_HW_UART_LINK_H */
