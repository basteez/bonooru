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
