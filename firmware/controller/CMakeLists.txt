# controller firmware: at this stage it only blinks the onboard LED (Story 2.1).
add_executable(controller main.c)
ocb_strict_warnings(main.c)
# ocb_core is linked so the ARM build compiles it with the strict warnings;
# main.c does not call it yet (Story 2.5 does).
target_link_libraries(controller PRIVATE ocb_hw ocb_proto ocb_core ocb_version)

# No stdio: keeps the UART pins untouched and TinyUSB unlinked (AC2).
pico_enable_stdio_uart(controller 0)
pico_enable_stdio_usb(controller 0)

pico_set_program_name(controller "bonooru-controller")
pico_set_program_version(controller "${OCB_VERSION_STRING}")

# UF2 + ELF + map + bin + hex + dis: build/<preset>/controller/controller.uf2
pico_add_extra_outputs(controller)
