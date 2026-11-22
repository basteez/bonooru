# ocb_p2: the P2 profile of the controller (wired Pro Controller, story 2.3):
# USB descriptors, the 0x80 handshake, subcommands, the emulated SPI flash,
# the 0x30 report and the IN interval median (AD-12). Pure C99, never
# includes pico-sdk or TinyUSB headers: it builds in host-tests and in the
# firmware presets, and tools/check-p2-pure.sh enforces the include
# allow-list. Links only ocb_proto (for ocb_state); the TinyUSB glue lives
# in controller/platform/usb_p2.c.
set(OCB_P2_SOURCES desc.c p2.c report.c interval.c)
add_library(ocb_p2 STATIC ${OCB_P2_SOURCES})
target_include_directories(ocb_p2 PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}/include")
target_link_libraries(ocb_p2 PUBLIC ocb_proto)
set_target_properties(ocb_p2 PROPERTIES C_STANDARD 99 C_STANDARD_REQUIRED ON C_EXTENSIONS OFF)
ocb_strict_warnings(${OCB_P2_SOURCES})
set_property(SOURCE ${OCB_P2_SOURCES} APPEND PROPERTY COMPILE_OPTIONS -Wpedantic -Wshadow -Wconversion)
