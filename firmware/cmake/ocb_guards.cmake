# Configure-time guards for firmware builds (AD-1, Story 2.1 Task 3.1).
# Each guard stops with the fix in the message.

function(ocb_pre_project_guards)
    set(_sdk "${OCB_FW_DIR}/external/pico-sdk")

    if(NOT EXISTS "${_sdk}/CMakeLists.txt")
        message(FATAL_ERROR
            "pico-sdk submodule is missing. Run from the repo root:\n"
            "  git submodule update --init")
    endif()

    if(NOT EXISTS "${_sdk}/lib/tinyusb/src/tusb.h")
        message(FATAL_ERROR
            "TinyUSB (pico-sdk lib/tinyusb) is not initialised. Run from the repo root:\n"
            "  git -C firmware/external/pico-sdk submodule update --init --depth 1 lib/tinyusb")
    endif()

    if(DEFINED PICO_TINYUSB_PATH OR DEFINED ENV{PICO_TINYUSB_PATH})
        message(FATAL_ERROR
            "PICO_TINYUSB_PATH must not be set (AD-1): the SDK's pinned lib/tinyusb is used. "
            "Unset the environment variable and remove the cache entry.")
    endif()

    if(DEFINED PICO_BOARD AND NOT PICO_BOARD STREQUAL "waveshare_rp2040_zero")
        message(FATAL_ERROR
            "PICO_BOARD is '${PICO_BOARD}', expected 'waveshare_rp2040_zero' (AD-1). "
            "Use the fw-release or fw-debug preset.")
    endif()
    set(PICO_BOARD "waveshare_rp2040_zero" CACHE STRING "Target board (AD-1)" FORCE)

    # Without picotool_DIR the SDK would download and build picotool from git
    # at build time, which AD-1 forbids.
    if(NOT picotool_DIR OR NOT EXISTS "${picotool_DIR}/picotoolConfig.cmake")
        message(FATAL_ERROR
            "picotool_DIR is unset or has no picotoolConfig.cmake (got '${picotool_DIR}'). "
            "Run firmware/tools/install-tools.sh and export OCB_PICOTOOL_DIR as it prints "
            "(the presets set picotool_DIR=$OCB_PICOTOOL_DIR/picotool).")
    endif()

    if(NOT PICO_TOOLCHAIN_PATH OR NOT EXISTS "${PICO_TOOLCHAIN_PATH}/bin")
        message(FATAL_ERROR
            "PICO_TOOLCHAIN_PATH is unset or invalid (got '${PICO_TOOLCHAIN_PATH}'). "
            "Run firmware/tools/install-tools.sh and export OCB_ARM_TOOLCHAIN as it prints.")
    endif()
endfunction()

# Require the pinned Arm GNU Toolchain; the banner of 15.2.Rel1 reads
# "arm-none-eabi-gcc (Arm GNU Toolchain 15.2.Rel1 (Build arm-15.86)) 15.2.1 20251203".
function(ocb_toolchain_guard version)
    execute_process(
        COMMAND "${CMAKE_C_COMPILER}" --version
        OUTPUT_VARIABLE _banner
        ERROR_QUIET
        RESULT_VARIABLE _rc)
    string(REGEX REPLACE "\n.*" "" _first_line "${_banner}")
    string(FIND "${_banner}" "(Arm GNU Toolchain ${version} " _pos)
    if(NOT _rc EQUAL 0 OR _pos EQUAL -1)
        if(OCB_ALLOW_OTHER_TOOLCHAIN)
            message(WARNING "Compiler is not Arm GNU Toolchain ${version} ('${_first_line}'); "
                "accepted because OCB_ALLOW_OTHER_TOOLCHAIN=ON. Never use this in CI.")
        else()
            message(FATAL_ERROR
                "Compiler '${CMAKE_C_COMPILER}' is not the pinned Arm GNU Toolchain ${version} "
                "(banner: '${_first_line}'). Run firmware/tools/install-tools.sh and export "
                "OCB_ARM_TOOLCHAIN, or pass -DOCB_ALLOW_OTHER_TOOLCHAIN=ON for a local experiment.")
        endif()
    else()
        message(STATUS "Toolchain: ${_first_line}")
    endif()
endfunction()
