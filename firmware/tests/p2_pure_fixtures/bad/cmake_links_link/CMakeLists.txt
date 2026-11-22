# Fixture for check-p2-pure.sh: must be flagged (links the controller link).
add_library(ocb_fixture STATIC fixture.c)
target_link_libraries(ocb_fixture PUBLIC ocb_proto ocb_link)
