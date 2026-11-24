# Fixture for check-bridge-pure.sh: links the controller link, must fail.
add_library(ocb_fixture STATIC x.c)
target_link_libraries(ocb_fixture PUBLIC ocb_link)
