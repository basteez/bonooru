# Bonooru firmware: one CMake project for both boards (AD-2).
#
# Firmware builds (presets fw-release, fw-debug) use the pico-sdk submodule
# with the pinned Arm GNU Toolchain and picotool (AD-1). The host-tests preset
# (OCB_HOST_TESTS=ON) builds only host-side tests with the native compiler and
# never includes pico-sdk.
cmake_minimum_required(VERSION 3.25)

option(OCB_HOST_TESTS "Build host-side tests only (native compiler, no pico-sdk)" OFF)
option(OCB_DEBUG_BUILD "Define OCB_DEBUG_BUILD=1 for our targets (debug-only pins and aids)" OFF)
option(OCB_BENCH_FAKE_READY "controller, debug builds only: fake a ready console for bench tests (story 2.7 D17)" OFF)
option(OCB_ALLOW_OTHER_TOOLCHAIN "Accept a compiler other than the pinned one (local experiments only)" OFF)
option(OCB_SANITIZE "host-tests only: build with AddressSanitizer and UBSan" OFF)

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

if(NOT OCB_HOST_TESTS)
    add_subdirectory(bridge)
    add_subdirectory(controller)
else()
    enable_testing()
    add_subdirectory(tests)
endif()
