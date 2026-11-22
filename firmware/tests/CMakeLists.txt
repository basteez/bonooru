# Host tests (preset host-tests): native compiler, no pico-sdk (AD-2).
# Plain C, no framework: each test exits non-zero on failure.

# Release pin set: OCB_PIN_DEBUG_TOGGLE must not exist.
add_executable(test_pins test_pins.c)
target_link_libraries(test_pins PRIVATE ocb_hw_pins)
ocb_strict_warnings(test_pins.c)
add_test(NAME test_pins COMMAND test_pins)

# Debug pin set: OCB_PIN_DEBUG_TOGGLE exists and is none of the other pins.
add_executable(test_pins_debug test_pins.c)
target_link_libraries(test_pins_debug PRIVATE ocb_hw_pins)
target_compile_definitions(test_pins_debug PRIVATE OCB_DEBUG_BUILD=1 OCB_TEST_EXPECT_DEBUG_PINS=1)
add_test(NAME test_pins_debug COMMAND test_pins_debug)

add_executable(test_version test_version.c)
target_link_libraries(test_version PRIVATE ocb_version)
ocb_strict_warnings(test_version.c)
add_test(NAME test_version COMMAND test_version)

# The pin grep flags every bad fixture, passes the good one and the real tree.
add_test(NAME pin_grep_selftest
    COMMAND bash "${CMAKE_CURRENT_SOURCE_DIR}/pin_grep_selftest.sh"
            "${OCB_FW_DIR}/tools/check-pins.sh" "${OCB_FW_DIR}")

# Linux only: LeakSanitizer (not available on macOS arm64; nothing allocates
# in ocb_proto anyway).
set(OCB_TEST_ENV "")
if(OCB_SANITIZE AND CMAKE_SYSTEM_NAME STREQUAL "Linux")
    set(OCB_TEST_ENV "ASAN_OPTIONS=detect_leaks=1")
endif()

# F.9: ocb_proto against the conformance vectors (AD-14). The vectors are
# read as shipped from the source tree; the .txt count at configure time
# guards against an area file the test does not know.
set(OCB_VECTORS_DIR "${OCB_FW_DIR}/common/proto/vectors")
file(GLOB OCB_VECTOR_FILES CONFIGURE_DEPENDS "${OCB_VECTORS_DIR}/*.txt")
list(LENGTH OCB_VECTOR_FILES OCB_VECTORS_TXT_COUNT)
add_executable(test_vectors test_vectors.c)
target_link_libraries(test_vectors PRIVATE ocb_proto)
target_compile_definitions(test_vectors PRIVATE
    OCB_VECTORS_DIR="${OCB_VECTORS_DIR}" OCB_VECTORS_TXT_COUNT=${OCB_VECTORS_TXT_COUNT})
ocb_strict_warnings(test_vectors.c)
add_test(NAME test_vectors COMMAND test_vectors)

# F.11: ocb_proto API tests (buffer limits, counters, randomised round trip).
add_executable(test_proto_api test_proto_api.c)
target_link_libraries(test_proto_api PRIVATE ocb_proto)
ocb_strict_warnings(test_proto_api.c)
add_test(NAME test_proto_api COMMAND test_proto_api)

# Controller core (story 2.4, D10): one executable per test-plan block, so a
# failure names the rule. F.3 modes and latch, F.4 END/underrun/LATE/order,
# F.5 sync points, F.6 safety mask, F.12 edge cases and property test.
set(OCB_CORE_TESTS test_core_modes test_core_end_late test_core_sync test_core_mask test_core_edge test_live test_core_timing)
foreach(_t IN LISTS OCB_CORE_TESTS)
    add_executable(${_t} ${_t}.c)
    target_link_libraries(${_t} PRIVATE ocb_core)
    ocb_strict_warnings(${_t}.c)
    add_test(NAME ${_t} COMMAND ${_t})
endforeach()

if(OCB_TEST_ENV)
    set_tests_properties(test_vectors test_proto_api ${OCB_CORE_TESTS} PROPERTIES ENVIRONMENT "${OCB_TEST_ENV}")
endif()

# PROTO_VERSION in consts.h equals every vector header; the script's self-test
# works on copies in the build directory, never in the source tree (D4).
add_test(NAME proto_version_check
    COMMAND bash "${CMAKE_CURRENT_SOURCE_DIR}/proto_version_selftest.sh"
            "${OCB_FW_DIR}/tools/check-proto-version.sh" "${OCB_FW_DIR}"
            "${CMAKE_CURRENT_BINARY_DIR}/proto_version_selftest")

# ocb_proto includes only the allowed standard headers and ocb_proto/* (AD-2).
add_test(NAME proto_pure_check
    COMMAND bash "${CMAKE_CURRENT_SOURCE_DIR}/proto_pure_selftest.sh"
            "${OCB_FW_DIR}/tools/check-proto-pure.sh" "${OCB_FW_DIR}")

# F.13: ocb_core includes only the allowed headers and uses no heap, I/O or
# clock identifiers (AD-2, story 2.4 D9); bad fixtures flagged, good fixture
# and the real controller/core tree pass.
add_test(NAME core_pure_check
    COMMAND bash "${CMAKE_CURRENT_SOURCE_DIR}/core_pure_selftest.sh"
            "${OCB_FW_DIR}/tools/check-core-pure.sh" "${OCB_FW_DIR}")

# Controller link (story 2.6): one executable per test-plan block. F.14 RX
# stream, F.15 session, F.1 go-back-N, F.2 NAK matrix, F.16 TX and STATUS,
# F.10 console ready, F.17 byte-stream fuzz and lossy channel; story 2.7:
# F.21 link watchdog; story 2.3: F.31 the P2 debug log ring, the STATUS v3
# tail and the measured report period.
set(OCB_LINK_TESTS test_link_rx test_link_session test_link_gbn test_link_nak test_link_tx test_link_ready
    test_link_fuzz test_link_watchdog test_link_dbglog test_link_pubstate)
foreach(_t IN LISTS OCB_LINK_TESTS)
    add_executable(${_t} ${_t}.c)
    target_link_libraries(${_t} PRIVATE ocb_link)
    ocb_strict_warnings(${_t}.c)
    add_test(NAME ${_t} COMMAND ${_t})
endforeach()
if(OCB_TEST_ENV)
    set_tests_properties(${OCB_LINK_TESTS} PROPERTIES ENVIRONMENT "${OCB_TEST_ENV}")
endif()

# F.37 (story 2.5 D8, D10): the built-in test timeline, its loader through
# the real link and core, the melody played on a fake clock.
add_executable(test_testtl test_testtl.c)
target_link_libraries(test_testtl PRIVATE ocb_link ocb_core_testtl)
ocb_strict_warnings(test_testtl.c)
add_test(NAME test_testtl COMMAND test_testtl)
if(OCB_TEST_ENV)
    set_tests_properties(test_testtl PROPERTIES ENVIRONMENT "${OCB_TEST_ENV}")
endif()

# F.18: ocb_link includes only the allowed headers and uses no heap, I/O or
# clock identifiers (AD-2, story 2.6 D16); bad fixtures flagged, good
# fixture and the real controller/link tree pass, the banned list equals
# the core gate's.
add_test(NAME link_pure_check
    COMMAND bash "${CMAKE_CURRENT_SOURCE_DIR}/link_pure_selftest.sh"
            "${OCB_FW_DIR}/tools/check-link-pure.sh" "${OCB_FW_DIR}")

# F.19: tools/link_probe.py (the bench sender) against the shared vectors and
# its AD-6 sender against an in-process fake link; stdlib only, no pyserial.
# F.20 (story 2.6 D31): the probe's real `soak` against the real link code
# through a model of the 2wiCC_Comms bridge, on a fake clock. The link is
# loaded with ctypes from a shared module built from the same sources without
# sanitizers (a sanitized library cannot be loaded into a plain python3).
# F.23 (story 2.7 D16): the one-command bench steps (linkup, silence, direct,
# playing, breaks, soak --heartbeat) through the same model, with negative
# controls.
find_package(Python3 COMPONENTS Interpreter REQUIRED)
file(GLOB OCB_LINK_SIM_SOURCES CONFIGURE_DEPENDS
    "${OCB_FW_DIR}/controller/link/*.c" "${OCB_FW_DIR}/controller/core/*.c" "${OCB_FW_DIR}/common/proto/*.c")
add_library(ocb_link_sim MODULE link_sim_shim.c ${OCB_LINK_SIM_SOURCES})
target_include_directories(ocb_link_sim PRIVATE
    "${OCB_FW_DIR}/controller/link/include" "${OCB_FW_DIR}/controller/core/include"
    "${OCB_FW_DIR}/common/proto/include")
target_compile_options(ocb_link_sim PRIVATE -fno-sanitize=all)
target_link_options(ocb_link_sim PRIVATE -fno-sanitize=all)
set_target_properties(ocb_link_sim PROPERTIES C_STANDARD 99 C_EXTENSIONS OFF)
ocb_strict_warnings(link_sim_shim.c)
add_test(NAME link_probe_selftest
    COMMAND "${Python3_EXECUTABLE}" -I "${CMAKE_CURRENT_SOURCE_DIR}/test_link_probe.py" "${OCB_FW_DIR}"
            "$<TARGET_FILE:ocb_link_sim>" "${CMAKE_CURRENT_BINARY_DIR}")

# F.22 (story 2.7 D18): the watchdog values written in docs/link-protocol.md
# (timing table and prose) equal constants.txt and ocb_link/config.h.
add_test(NAME docs_constants_check
    COMMAND "${Python3_EXECUTABLE}" -I "${CMAKE_CURRENT_SOURCE_DIR}/test_docs_constants.py" "${OCB_FW_DIR}/..")

# Controller P2 profile (story 2.3): F.24 descriptors, F.25 handshake, F.26
# subcommands and refusals (C7), F.27 the 0x30 report, F.28 the fuzz test,
# F.29 the IN interval median.
set(OCB_P2_TESTS test_p2_desc test_p2_handshake test_p2_subcmd test_p2_report test_p2_interval test_p2_fuzz)
foreach(_t IN LISTS OCB_P2_TESTS)
    add_executable(${_t} ${_t}.c)
    target_link_libraries(${_t} PRIVATE ocb_p2)
    ocb_strict_warnings(${_t}.c)
    add_test(NAME ${_t} COMMAND ${_t})
endforeach()
if(OCB_TEST_ENV)
    set_tests_properties(${OCB_P2_TESTS} PROPERTIES ENVIRONMENT "${OCB_TEST_ENV}")
endif()

# F.30: ocb_p2 includes only the allowed headers and uses no heap, I/O or
# clock identifiers (AD-2, story 2.3 D16); bad fixtures flagged, good
# fixture and the real controller/profile_p2 tree pass, the banned list
# equals the core gate's.
add_test(NAME p2_pure_check
    COMMAND bash "${CMAKE_CURRENT_SOURCE_DIR}/p2_pure_selftest.sh"
            "${OCB_FW_DIR}/tools/check-p2-pure.sh" "${OCB_FW_DIR}")

# F.32 (story 2.3 D18): tools/p2_probe.py (the Mac-side host of the P2
# handshake) against its fixtures and the real ocb_p2 through a ctypes
# shared module built from the same sources without sanitizers, as
# ocb_link_sim; its link_probe.py part (p2check, p2watch) is in
# link_probe_selftest.
file(GLOB OCB_P2_SIM_SOURCES CONFIGURE_DEPENDS
    "${OCB_FW_DIR}/controller/profile_p2/*.c" "${OCB_FW_DIR}/common/proto/*.c")
add_library(ocb_p2_sim MODULE p2_sim_shim.c ${OCB_P2_SIM_SOURCES})
target_include_directories(ocb_p2_sim PRIVATE
    "${OCB_FW_DIR}/controller/profile_p2/include" "${OCB_FW_DIR}/common/proto/include")
target_compile_options(ocb_p2_sim PRIVATE -fno-sanitize=all)
target_link_options(ocb_p2_sim PRIVATE -fno-sanitize=all)
set_target_properties(ocb_p2_sim PROPERTIES C_STANDARD 99 C_EXTENSIONS OFF)
ocb_strict_warnings(p2_sim_shim.c)
add_test(NAME p2_probe_selftest
    COMMAND "${Python3_EXECUTABLE}" -I "${CMAKE_CURRENT_SOURCE_DIR}/test_p2_probe.py" "${OCB_FW_DIR}"
            "$<TARGET_FILE:ocb_p2_sim>")

# F.38 (story 2.5 D8): the generated table does not drift from its source
# (regenerated in a temporary directory, compared byte for byte), and the
# generator's own rules hold on hand-written bad inputs.
add_test(NAME testtl_drift
    COMMAND "${Python3_EXECUTABLE}" -I "${CMAKE_CURRENT_SOURCE_DIR}/test_gen_test_timeline.py" "${OCB_FW_DIR}")

# Optional (story 2.3 D17): a libFuzzer entry over ocb_p2_on_output, only
# with -DOCB_FUZZ=ON and clang; never in a preset or in CI.
option(OCB_FUZZ "host-tests only: build the libFuzzer target fuzz_p2 (clang)" OFF)
if(OCB_FUZZ)
    if(NOT CMAKE_C_COMPILER_ID MATCHES "Clang")
        message(FATAL_ERROR "OCB_FUZZ needs clang (libFuzzer)")
    endif()
    add_executable(fuzz_p2 fuzz/fuzz_p2.c)
    target_link_libraries(fuzz_p2 PRIVATE ocb_p2)
    target_compile_options(fuzz_p2 PRIVATE -fsanitize=fuzzer)
    target_link_options(fuzz_p2 PRIVATE -fsanitize=fuzzer)
endif()
