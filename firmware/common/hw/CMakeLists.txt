# ocb_hw_pins: the pin allow-list (AD-20), header only, in every build
# including host-tests (no SDK includes in pins.h).
add_library(ocb_hw_pins INTERFACE)
target_include_directories(ocb_hw_pins INTERFACE "${CMAKE_CURRENT_SOURCE_DIR}/include")
if(OCB_DEBUG_BUILD)
    target_compile_definitions(ocb_hw_pins INTERFACE OCB_DEBUG_BUILD=1)
endif()

if(OCB_HOST_TESTS)
    return()
endif()

# ocb_hw: board hardware drivers (firmware builds only). Compiled against the
# SDK *_headers targets so no SDK source is compiled into this archive; the
# full SDK libraries are linked by the consuming executable (INTERFACE).
# uart_link.c (story 2.6): the board-link UART driver, shared by both boards,
# no frame knowledge; ocb_proto is linked for OCB_LINK_BAUD only.
add_library(ocb_hw STATIC led.c uart_link.c)
target_include_directories(ocb_hw PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}/include")
target_link_libraries(ocb_hw
    PUBLIC ocb_hw_pins ocb_proto pico_stdlib_headers hardware_pio_headers hardware_clocks_headers
           hardware_uart_headers hardware_gpio_headers hardware_irq_headers
    INTERFACE pico_stdlib hardware_pio hardware_clocks hardware_uart hardware_gpio hardware_irq)
ocb_strict_warnings(led.c uart_link.c)

# The vendored pico-examples PIO program (BSD-3-Clause, verbatim); the header
# is generated into the build tree, never committed.
pico_generate_pio_header(ocb_hw
    "${OCB_FW_DIR}/third_party/pico-examples/ws2812/ws2812.pio"
    OUTPUT_DIR "${CMAKE_CURRENT_BINARY_DIR}/generated")
