# controller firmware: the LED blink of Story 2.1 and the USB stack of
# Story 2.3 (TinyUSB plus the P2 profile, platform/usb_p2.c) on core 0 and,
# on core 1, the board link of Story 2.6 (platform/link_runner.c: ocb_hw
# UART glue, ocb_link, ocb_core) in the super-loop of Story 2.5, with the
# liveness rules and the hardware watchdog (platform/liveness.c) on both.

# Story 2.3 D21: the HID profile of the build. Only P2 is built in v1; P1
# and P3 are AD-12 slots (R6).
set(OCB_PROFILE "P2" CACHE STRING "controller HID profile (story 2.3 D21): P2 only in v1")
set_property(CACHE OCB_PROFILE PROPERTY STRINGS P2)
if(NOT OCB_PROFILE STREQUAL "P2")
    message(FATAL_ERROR "OCB_PROFILE=${OCB_PROFILE}: only P2 is built in v1 (P1 and P3 are AD-12 slots, not built; R6)")
endif()

# Story 2.3 D4: the endpoint interval of the P2 descriptors, ms. 1 by
# default; the real controller declares 8 (H.17 builds both).
set(OCB_P2_BINTERVAL "1" CACHE STRING "P2 endpoint bInterval in ms, 1..16 (story 2.3 D4)")
if(NOT OCB_P2_BINTERVAL MATCHES "^[0-9]+$" OR OCB_P2_BINTERVAL LESS 1 OR OCB_P2_BINTERVAL GREATER 16)
    message(FATAL_ERROR "OCB_P2_BINTERVAL=${OCB_P2_BINTERVAL}: must be an integer in 1..16")
endif()

set(OCB_CONTROLLER_SOURCES main.c platform/link_runner.c platform/usb_p2.c platform/liveness.c
    platform/stack_mark.c)
set(OCB_CONTROLLER_STRICT platform/link_runner.c platform/usb_p2.c platform/liveness.c platform/stack_mark.c)
if(NOT OCB_DEBUG_HANG STREQUAL "NONE")
    list(APPEND OCB_CONTROLLER_SOURCES platform/debug_hang.c)
    list(APPEND OCB_CONTROLLER_STRICT platform/debug_hang.c)
endif()
add_executable(controller ${OCB_CONTROLLER_SOURCES})
target_include_directories(controller PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/platform")
ocb_strict_warnings(main.c ${OCB_CONTROLLER_STRICT})
set_property(SOURCE ${OCB_CONTROLLER_STRICT} APPEND PROPERTY COMPILE_OPTIONS -Wpedantic -Wshadow -Wconversion)
target_compile_definitions(controller PRIVATE OCB_PROFILE_P2=1 OCB_P2_BINTERVAL=${OCB_P2_BINTERVAL})
target_link_libraries(controller PRIVATE ocb_hw ocb_proto ocb_core ocb_link ocb_p2 ocb_version pico_multicore
    pico_unique_id tinyusb_device hardware_watchdog)

# Story 2.5 D9: the debug flavours (options and guards in firmware/CMakeLists.txt).
if(OCB_DEBUG_TIMELINE)
    target_link_libraries(controller PRIVATE ocb_core_testtl)
    target_compile_definitions(controller PRIVATE OCB_DEBUG_TIMELINE=1)
endif()
if(OCB_DEBUG_HANG STREQUAL "CORE0")
    target_compile_definitions(controller PRIVATE OCB_DEBUG_HANG_CORE0=1
        OCB_DEBUG_HANG_DELAY_US=${OCB_DEBUG_HANG_DELAY_US})
elseif(OCB_DEBUG_HANG STREQUAL "CORE1")
    target_compile_definitions(controller PRIVATE OCB_DEBUG_HANG_CORE1=1
        OCB_DEBUG_HANG_DELAY_US=${OCB_DEBUG_HANG_DELAY_US})
endif()

# Story 2.5 D5, D24: the cross-core barrier of ocb_link is a real `dmb sy` on
# the target, from a platform header force-included into the library's
# sources (no SDK include and no asm token in the pure library).
target_compile_options(ocb_link PRIVATE -include "${CMAKE_CURRENT_SOURCE_DIR}/platform/ocb_link_barrier.h")

# Story 2.5 D2: the whole program runs from RAM (no XIP cache miss on either
# core); OCB_CONTROLLER_RUN_FROM_RAM=OFF keeps the XIP variant for H.20.
if(OCB_CONTROLLER_RUN_FROM_RAM)
    pico_set_binary_type(controller copy_to_ram)
endif()

# Story 2.5 Task 12.2: per-function stack usage and call graph of our own
# sources and of the SDK sources compiled into the controller, for
# tools/check-stack.sh (the static lower bound of the core 1 stack).
target_compile_options(controller PRIVATE -fstack-usage -fcallgraph-info=su)
foreach(_lib ocb_core ocb_link ocb_proto ocb_hw)
    target_compile_options(${_lib} PRIVATE -fstack-usage -fcallgraph-info=su)
endforeach()
if(OCB_DEBUG_TIMELINE)
    target_compile_options(ocb_core_testtl PRIVATE -fstack-usage -fcallgraph-info=su)
endif()

# No stdio: keeps the UART pins untouched, and USB belongs to the P2
# profile (TinyUSB is linked for it, never for stdio).
pico_enable_stdio_uart(controller 0)
pico_enable_stdio_usb(controller 0)

pico_set_program_name(controller "bonooru-controller")
pico_set_program_version(controller "${OCB_VERSION_STRING}")

# UF2 + ELF + map + bin + hex + dis: build/<preset>/controller/controller.uf2
pico_add_extra_outputs(controller)
