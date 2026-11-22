# Fixture for check-link-pure.sh: must be flagged (links the SDK UART library).
add_library(ocb_fixture STATIC fixture.c)
target_link_libraries(ocb_fixture PUBLIC ocb_core hardware_uart)
