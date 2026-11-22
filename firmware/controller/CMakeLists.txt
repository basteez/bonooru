# controller firmware: the LED blink of Story 2.1 on core 0 and, on core 1,
# the board link of Story 2.6 (platform/link_runner.c: ocb_hw UART glue,
# ocb_link, ocb_core).
add_executable(controller main.c platform/link_runner.c)
target_include_directories(controller PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/platform")
ocb_strict_warnings(main.c platform/link_runner.c)
set_property(SOURCE platform/link_runner.c APPEND PROPERTY COMPILE_OPTIONS -Wpedantic -Wshadow -Wconversion)
target_link_libraries(controller PRIVATE ocb_hw ocb_proto ocb_core ocb_link ocb_version pico_multicore)

# No stdio: keeps the UART pins untouched and TinyUSB unlinked (AC2).
pico_enable_stdio_uart(controller 0)
pico_enable_stdio_usb(controller 0)

pico_set_program_name(controller "bonooru-controller")
pico_set_program_version(controller "${OCB_VERSION_STRING}")

# UF2 + ELF + map + bin + hex + dis: build/<preset>/controller/controller.uf2
pico_add_extra_outputs(controller)
