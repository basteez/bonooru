/*
 * SPDX-License-Identifier: MIT
 *
 * Host test F.0: ocb_hw/pins.h defines exactly the allowed pins (AD-20).
 * Built twice: release pin set, and with OCB_DEBUG_BUILD=1
 * (OCB_TEST_EXPECT_DEBUG_PINS=1) for the debug-only toggle pin.
 */
#include <stdio.h>

#include "ocb_hw/pins.h"

#if !defined(OCB_PIN_UART_TX) || !defined(OCB_PIN_UART_RX) || !defined(OCB_PIN_LED)
#error "pins.h must define OCB_PIN_UART_TX, OCB_PIN_UART_RX and OCB_PIN_LED"
#endif

#if defined(OCB_TEST_EXPECT_DEBUG_PINS)
#if !defined(OCB_PIN_DEBUG_TOGGLE)
#error "OCB_PIN_DEBUG_TOGGLE must exist when OCB_DEBUG_BUILD is defined"
#endif
#else
#if defined(OCB_PIN_DEBUG_TOGGLE)
#error "OCB_PIN_DEBUG_TOGGLE must not exist without OCB_DEBUG_BUILD"
#endif
#endif

static int failures;

static void check(int ok, const char *what)
{
    if (!ok) {
        fprintf(stderr, "FAIL: %s\n", what);
        failures++;
    }
}

int main(void)
{
    check(OCB_PIN_UART_TX == 0u, "OCB_PIN_UART_TX == 0");
    check(OCB_PIN_UART_RX == 1u, "OCB_PIN_UART_RX == 1");
    check(OCB_PIN_LED == 16u, "OCB_PIN_LED == 16");
#if defined(OCB_TEST_EXPECT_DEBUG_PINS)
    check(OCB_PIN_DEBUG_TOGGLE != OCB_PIN_UART_TX, "debug toggle is not the UART TX pin");
    check(OCB_PIN_DEBUG_TOGGLE != OCB_PIN_UART_RX, "debug toggle is not the UART RX pin");
    check(OCB_PIN_DEBUG_TOGGLE != OCB_PIN_LED, "debug toggle is not the LED pin");
    check(OCB_PIN_DEBUG_TOGGLE < 26u, "debug toggle is not an ADC-capable pin");
#endif
    if (failures == 0) {
        printf("test_pins: ok\n");
    }
    return failures == 0 ? 0 : 1;
}
