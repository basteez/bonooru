# ocb_bridge_core: the bridge's logic (Story 3.1): framer (PC -> UART), the
# heartbeat frame, the AD-20 probe, the UART -> PC forwarder and the per-pass
# orchestration. Pure C99, never includes pico-sdk or TinyUSB headers: it
# builds in host-tests and in the firmware presets, and
# tools/check-bridge-pure.sh enforces the include allow-list (F.40).
set(OCB_BRIDGE_CORE_SOURCES framer.c heartbeat.c probe.c rxfwd.c bridge.c)
add_library(ocb_bridge_core STATIC ${OCB_BRIDGE_CORE_SOURCES})
target_include_directories(ocb_bridge_core PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}/include")
target_link_libraries(ocb_bridge_core PUBLIC ocb_proto)
set_target_properties(ocb_bridge_core PROPERTIES C_STANDARD 99 C_STANDARD_REQUIRED ON C_EXTENSIONS OFF)
ocb_strict_warnings(${OCB_BRIDGE_CORE_SOURCES})
set_property(SOURCE ${OCB_BRIDGE_CORE_SOURCES} APPEND PROPERTY COMPILE_OPTIONS -Wpedantic -Wshadow -Wconversion)
