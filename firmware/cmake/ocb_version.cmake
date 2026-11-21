# Firmware version from git describe (AD-14, Story 2.1 Decision D6b).
#
# Computed once at CMake configure time (CI always configures fresh); a local
# build tree picks up a new tag or commit on the next reconfigure. This is the
# only place in firmware/ that computes a version.
#
# Result: OCB_VERSION_STRING = X.Y.Z, X.Y.Z-N-gHASH (N > 0) and/or a -dirty
# suffix; fallback 0.0.0-dev when there is no matching v-tag, no .git (source
# tarball) or no git. OCB_VERSION_MAJOR/MINOR/PATCH are the leading X.Y.Z.

set(OCB_VERSION_STRING "0.0.0-dev")
set(OCB_VERSION_MAJOR 0)
set(OCB_VERSION_MINOR 0)
set(OCB_VERSION_PATCH 0)

find_package(Git QUIET)
if(GIT_FOUND)
    execute_process(
        COMMAND "${GIT_EXECUTABLE}" describe --tags --match "v[0-9]*.[0-9]*.[0-9]*" --long --dirty
        WORKING_DIRECTORY "${OCB_FW_DIR}/.."
        OUTPUT_VARIABLE _ocb_describe
        OUTPUT_STRIP_TRAILING_WHITESPACE
        ERROR_QUIET
        RESULT_VARIABLE _ocb_describe_rc)
    if(_ocb_describe_rc EQUAL 0 AND _ocb_describe MATCHES
            "^v([0-9]+)\\.([0-9]+)\\.([0-9]+)-([0-9]+)-(g[0-9a-f]+)(-dirty)?$")
        set(OCB_VERSION_MAJOR "${CMAKE_MATCH_1}")
        set(OCB_VERSION_MINOR "${CMAKE_MATCH_2}")
        set(OCB_VERSION_PATCH "${CMAKE_MATCH_3}")
        set(OCB_VERSION_STRING "${CMAKE_MATCH_1}.${CMAKE_MATCH_2}.${CMAKE_MATCH_3}")
        if(NOT CMAKE_MATCH_4 STREQUAL "0")
            string(APPEND OCB_VERSION_STRING "-${CMAKE_MATCH_4}-${CMAKE_MATCH_5}")
        endif()
        string(APPEND OCB_VERSION_STRING "${CMAKE_MATCH_6}")
    endif()
endif()

message(STATUS "Firmware version: ${OCB_VERSION_STRING}")

configure_file("${CMAKE_CURRENT_LIST_DIR}/ocb_version.h.in"
    "${CMAKE_BINARY_DIR}/generated/ocb_version/ocb_version.h" @ONLY)

add_library(ocb_version INTERFACE)
target_include_directories(ocb_version INTERFACE "${CMAKE_BINARY_DIR}/generated/ocb_version")
