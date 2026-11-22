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

if(OCB_TEST_ENV)
    set_tests_properties(test_vectors test_proto_api PROPERTIES ENVIRONMENT "${OCB_TEST_ENV}")
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
