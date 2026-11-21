# ocb_proto: the PC <-> controller wire protocol library (AD-2).
# Empty INTERFACE target for now; Story 2.2 adds the sources and makes it
# STATIC. Pure C99, never includes pico-sdk headers (it also builds in host-tests).
add_library(ocb_proto INTERFACE)
target_include_directories(ocb_proto INTERFACE "${CMAKE_CURRENT_SOURCE_DIR}/include")
