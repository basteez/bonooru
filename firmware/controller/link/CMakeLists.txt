# ocb_link: the controller side of the PC <-> controller link (story 2.6):
# RX error policy, session, go-back-N front, NAK budget, console ready,
# TX slots, STATUS, the TX gate and the link watchdog (story 2.7). Pure C99, never includes pico-sdk
# headers: it builds in host-tests and in the firmware presets, and
# tools/check-link-pure.sh enforces the include allow-list.
# Links only ocb_core and ocb_proto (no SDK, no board library, no version).
set(OCB_LINK_SOURCES rxring.c ready.c txgate.c watchdog.c link.c tx.c tx_debug.c dbglog.c pubstate.c)
add_library(ocb_link STATIC ${OCB_LINK_SOURCES})
target_include_directories(ocb_link PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}/include")
target_link_libraries(ocb_link PUBLIC ocb_core ocb_proto)
set_target_properties(ocb_link PROPERTIES C_STANDARD 99 C_STANDARD_REQUIRED ON C_EXTENSIONS OFF)
ocb_strict_warnings(${OCB_LINK_SOURCES})
set_property(SOURCE ${OCB_LINK_SOURCES} APPEND PROPERTY COMPILE_OPTIONS -Wpedantic -Wshadow -Wconversion)
