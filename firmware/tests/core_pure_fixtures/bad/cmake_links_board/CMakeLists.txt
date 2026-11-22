# Fixture for check-core-pure.sh: the core must not link the board library.
add_library(ocb_fixture STATIC fixture.c)
target_link_libraries(ocb_fixture PRIVATE ocb_hw hardware_timer)
