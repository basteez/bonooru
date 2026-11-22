# Fixture for check-core-pure.sh: the core must not link an SDK library.
add_library(ocb_fixture STATIC fixture.c)
target_link_libraries(ocb_fixture PUBLIC ocb_proto pico_stdlib)
