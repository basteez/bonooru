# Bonooru firmware: one CMake project for both boards (AD-2).
#
# Firmware builds (presets fw-release, fw-debug) use the pico-sdk submodule
# with the pinned Arm GNU Toolchain and picotool (AD-1). The host-tests preset
# (OCB_HOST_TESTS=ON) builds only host-side tests with the native compiler and
# never includes pico-sdk.
cmake_minimum_required(VERSION 3.25)

option(OCB_HOST_TESTS "Build host-side tests only (native compiler, no pico-sdk)" OFF)
option(OCB_DEBUG_BUILD "Define OCB_DEBUG_BUILD=1 for our targets (debug-only pins and aids)" OFF)
option(OCB_ALLOW_OTHER_TOOLCHAIN "Accept a compiler other than the pinned one (local experiments only)" OFF)
option(OCB_SANITIZE "host-tests only: build with AddressSanitizer and UBSan" OFF)
# Story 2.5 D9: the debug flavours of the controller. Each needs
# OCB_DEBUG_BUILD=ON; the hang flavours also need the timeline (the melody
# supplies the held note). Use the presets fw-debug-timeline,
# fw-debug-hang0, fw-debug-hang1 rather than typing these.
option(OCB_DEBUG_TIMELINE "controller: load the built-in test timeline after HELLO (debug builds only)" OFF)
set(OCB_DEBUG_HANG "NONE" CACHE STRING "controller: hang core 0 or core 1 on the first note (debug builds only)")
set_property(CACHE OCB_DEBUG_HANG PROPERTY STRINGS NONE CORE0 CORE1)
set(OCB_DEBUG_HANG_DELAY_US "50000" CACHE STRING "controller: the hang starts this long after the first note, us")
# Story 2.5 D2: the controller program runs from RAM (copy_to_ram); OFF
# builds the XIP variant, for the comparison H.20 and the RAM gate's
# self-test only.
option(OCB_CONTROLLER_RUN_FROM_RAM "controller: copy the whole program to RAM at boot" ON)

if(OCB_DEBUG_TIMELINE AND NOT OCB_DEBUG_BUILD)
    message(FATAL_ERROR "OCB_DEBUG_TIMELINE=ON needs OCB_DEBUG_BUILD=ON (a release build never carries the "
        "test melody). Use the preset fw-debug-timeline, or add -DOCB_DEBUG_BUILD=ON.")
endif()
if(NOT OCB_DEBUG_HANG MATCHES "^(NONE|CORE0|CORE1)$")
    message(FATAL_ERROR "OCB_DEBUG_HANG=${OCB_DEBUG_HANG}: must be NONE, CORE0 or CORE1")
endif()
if(NOT OCB_DEBUG_HANG STREQUAL "NONE")
    if(NOT OCB_DEBUG_BUILD)
        message(FATAL_ERROR "OCB_DEBUG_HANG=${OCB_DEBUG_HANG} needs OCB_DEBUG_BUILD=ON (a release build never "
            "carries a hang injector). Use the preset fw-debug-hang0 or fw-debug-hang1.")
    endif()
    if(NOT OCB_DEBUG_TIMELINE)
        message(FATAL_ERROR "OCB_DEBUG_HANG=${OCB_DEBUG_HANG} needs OCB_DEBUG_TIMELINE=ON (the built-in melody "
            "supplies the held note). Use the preset fw-debug-hang0 or fw-debug-hang1.")
    endif()
endif()
if(NOT OCB_DEBUG_HANG_DELAY_US MATCHES "^[0-9]+$" OR OCB_DEBUG_HANG_DELAY_US GREATER 1000000)
    message(FATAL_ERROR "OCB_DEBUG_HANG_DELAY_US=${OCB_DEBUG_HANG_DELAY_US}: an integer in 0..1000000 (us)")
endif()

set(OCB_FW_DIR "${CMAKE_CURRENT_SOURCE_DIR}")
list(APPEND CMAKE_MODULE_PATH "${OCB_FW_DIR}/cmake")

# Pins of the toolchain, read from the single source of truth.
file(STRINGS "${OCB_FW_DIR}/tools/toolchain.env" _ocb_arm_line REGEX "^ARM_TOOLCHAIN_VERSION=")
string(REPLACE "ARM_TOOLCHAIN_VERSION=" "" OCB_ARM_TOOLCHAIN_VERSION "${_ocb_arm_line}")

if(NOT OCB_HOST_TESTS)
    include(ocb_guards)
    ocb_pre_project_guards()
    # Include pico_sdk_init.cmake directly: no import file, no fetch-from-git fallback.
    include("${OCB_FW_DIR}/external/pico-sdk/pico_sdk_init.cmake")
    project(bonooru_fw C CXX ASM)
    ocb_toolchain_guard("${OCB_ARM_TOOLCHAIN_VERSION}")
    set(CMAKE_C_STANDARD 11)
    set(CMAKE_C_STANDARD_REQUIRED ON)
    pico_sdk_init()
else()
    project(bonooru_fw C)
    set(CMAKE_C_STANDARD 11)
    set(CMAKE_C_STANDARD_REQUIRED ON)
    # ASan + UBSan for every host-test target (story 2.2 D8); never in the
    # firmware presets. -DOCB_SANITIZE=OFF if a sanitizer runtime is missing.
    if(OCB_SANITIZE)
        set(_ocb_san "-fsanitize=address,undefined -fno-sanitize-recover=undefined -fno-omit-frame-pointer")
        string(APPEND CMAKE_C_FLAGS " ${_ocb_san}")
        string(APPEND CMAKE_EXE_LINKER_FLAGS " ${_ocb_san}")
    endif()
endif()

include(ocb_warnings)
include(ocb_version)

add_subdirectory(common/proto)
add_subdirectory(common/hw)
# Controller playback core (story 2.4): pure C99, built in both modes so the
# host tests and the ARM toolchain both compile it.
add_subdirectory(controller/core)
# Controller link layer (story 2.6): pure C99, built in both modes.
add_subdirectory(controller/link)
# Controller P2 profile (story 2.3): pure C99, built in both modes.
add_subdirectory(controller/profile_p2)

if(NOT OCB_HOST_TESTS)
    add_subdirectory(bridge)
    add_subdirectory(controller)
else()
    enable_testing()
    add_subdirectory(tests)
endif()
