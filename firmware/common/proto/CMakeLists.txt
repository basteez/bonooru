# ocb_proto: the PC <-> controller wire protocol library (AD-2, AD-3).
# Pure C99, never includes pico-sdk headers: it also builds in host-tests,
# and tools/check-proto-pure.sh enforces the include allow-list.
# Links nothing (no pico_*, no ocb_hw).
set(OCB_PROTO_SOURCES crc16.c cobs.c state.c frame.c rx.c msgs.c)
add_library(ocb_proto STATIC ${OCB_PROTO_SOURCES})
target_include_directories(ocb_proto PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}/include")
set_target_properties(ocb_proto PROPERTIES C_STANDARD 99 C_STANDARD_REQUIRED ON C_EXTENSIONS OFF)
ocb_strict_warnings(${OCB_PROTO_SOURCES})
set_property(SOURCE ${OCB_PROTO_SOURCES} APPEND PROPERTY COMPILE_OPTIONS -Wpedantic -Wshadow -Wconversion)
