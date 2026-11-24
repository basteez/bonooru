# Bridge firmware (Story 3.1): USB CDC <-> UART with a frame-aware PC -> UART
# path, its own heartbeat and the AD-20 probe. The logic is in core/
# (ocb_bridge_core, pure C, added by the top-level CMakeLists.txt); this is
# the platform glue.
set(OCB_BRIDGE_PLATFORM_SOURCES platform/main.c platform/usb_descriptors.c)
add_executable(bridge ${OCB_BRIDGE_PLATFORM_SOURCES})
ocb_strict_warnings(${OCB_BRIDGE_PLATFORM_SOURCES})
target_include_directories(bridge PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/platform")
target_link_libraries(bridge PRIVATE ocb_bridge_core ocb_hw ocb_proto ocb_version pico_stdlib
    pico_unique_id tinyusb_device pico_usb_reset)
# AD-13: the 1200-baud touch reboots into BOOTSEL through pico_usb_reset's
# own tud_cdc_line_coding_cb; no vendor reset interface in our descriptors.
target_compile_definitions(bridge PRIVATE PICO_ENABLE_USB_RESET_VIA_BAUD_RATE=1
    PICO_ENABLE_USB_RESET_VIA_VENDOR_INTERFACE=0)

# No stdio: keeps the UART pins untouched (TinyUSB is used directly).
pico_enable_stdio_uart(bridge 0)
pico_enable_stdio_usb(bridge 0)

pico_set_program_name(bridge "bonooru-bridge")
pico_set_program_version(bridge "${OCB_VERSION_STRING}")

# UF2 + ELF + map + bin + hex + dis: build/<preset>/bridge/bridge.uf2
pico_add_extra_outputs(bridge)
