# ocb_core: the controller playback core (AD-8, AD-9; story 2.4): modes,
# event queue, scheduler, abort latch and safety mask. Pure C99, never
# includes pico-sdk headers: it builds in host-tests and in the firmware
# presets, and tools/check-core-pure.sh enforces the include allow-list.
# Links only ocb_proto (no pico_*, no ocb_hw, no ocb_version).
set(OCB_CORE_SOURCES queue.c mask.c modes.c sched.c live.c timing.c)
add_library(ocb_core STATIC ${OCB_CORE_SOURCES})
target_include_directories(ocb_core PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}/include")
target_link_libraries(ocb_core PUBLIC ocb_proto)
set_target_properties(ocb_core PROPERTIES C_STANDARD 99 C_STANDARD_REQUIRED ON C_EXTENSIONS OFF)
ocb_strict_warnings(${OCB_CORE_SOURCES})
set_property(SOURCE ${OCB_CORE_SOURCES} APPEND PROPERTY COMPILE_OPTIONS -Wpedantic -Wshadow -Wconversion)

# Story 2.5 D8, D9: the built-in test timeline (the generated table and its
# loader), a separate library so that only the OCB_DEBUG_TIMELINE flavour
# links it into the controller; built and tested on the host in every
# configuration. Same purity rules (tools/check-core-pure.sh scans it).
set(OCB_CORE_TESTTL_SOURCES testtl.c testtl_data.c)
add_library(ocb_core_testtl STATIC ${OCB_CORE_TESTTL_SOURCES})
target_link_libraries(ocb_core_testtl PUBLIC ocb_core)
set_target_properties(ocb_core_testtl PROPERTIES C_STANDARD 99 C_STANDARD_REQUIRED ON C_EXTENSIONS OFF)
ocb_strict_warnings(${OCB_CORE_TESTTL_SOURCES})
set_property(SOURCE ${OCB_CORE_TESTTL_SOURCES} APPEND PROPERTY COMPILE_OPTIONS -Wpedantic -Wshadow -Wconversion)
