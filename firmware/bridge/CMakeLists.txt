# bridge firmware: at this stage it only blinks the onboard LED (Story 2.1).
add_executable(bridge main.c)
ocb_strict_warnings(main.c)
target_link_libraries(bridge PRIVATE ocb_hw ocb_proto ocb_version)

# No stdio: keeps the UART pins untouched and TinyUSB unlinked (AC2).
pico_enable_stdio_uart(bridge 0)
pico_enable_stdio_usb(bridge 0)

pico_set_program_name(bridge "bonooru-bridge")
pico_set_program_version(bridge "${OCB_VERSION_STRING}")

# UF2 + ELF + map + bin + hex + dis: build/<preset>/bridge/bridge.uf2
pico_add_extra_outputs(bridge)
