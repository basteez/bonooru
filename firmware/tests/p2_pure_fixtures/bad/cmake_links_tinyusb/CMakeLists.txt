# Fixture for check-p2-pure.sh: must be flagged (links the TinyUSB device stack).
add_library(ocb_fixture STATIC fixture.c)
target_link_libraries(ocb_fixture PUBLIC ocb_proto tinyusb_device)
