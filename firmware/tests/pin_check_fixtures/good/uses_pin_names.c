/* Fixture (good): pin names only. Drives the LED pin and the UART pins by
 * their OCB_PIN_* names; numbers that are not pins are fine. */
#include "ocb_hw/pins.h"
void f(void)
{
    gpio_init(OCB_PIN_LED);
    gpio_put(OCB_PIN_LED, 1);
    gpio_set_function(OCB_PIN_UART_TX, GPIO_FUNC_UART);
    pio_sm_set_consecutive_pindirs(pio, sm, OCB_PIN_LED, 1, true);
    ws2812_program_init(pio, sm, offset, OCB_PIN_LED, 800000, false);
    uint32_t grb = 0x100600u << 8;
    sleep_us(280);
}
