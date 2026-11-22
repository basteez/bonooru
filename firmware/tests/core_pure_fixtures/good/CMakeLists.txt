# Fixture for check-core-pure.sh: must pass. A comment may name pico_stdlib,
# hardware_timer or ocb_hw; a link line may not.
add_library(ocb_fixture STATIC allowed.c)
target_link_libraries(ocb_fixture PUBLIC ocb_proto)
