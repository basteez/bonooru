# Strict warnings on our own sources only, never on pico-sdk sources.
# pico-sdk libraries are INTERFACE libraries whose sources compile inside each
# executable, so the flags go on our source files, not on the executable target.
set(OCB_STRICT_WARNINGS -Wall -Wextra -Werror)

function(ocb_strict_warnings)
    set_property(SOURCE ${ARGN} APPEND PROPERTY COMPILE_OPTIONS ${OCB_STRICT_WARNINGS})
endfunction()
