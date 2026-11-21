# common

Code shared by bridge and controller: link protocol definitions, version, UART helpers.

- `proto/`: `ocb_proto`, the pure C99 codec (COBS, CRC, frame types, messages, canonical state, `NEUTRAL`, `PROTO_VERSION`, link constants); an empty target until Story 2.2
- `proto/vectors/`: language-neutral conformance vectors shared by the firmware host tests and the app core tests (AD-14); a wire change edits these first
- `hw/`: `ocb_hw`, board drivers on pico-sdk; today the non-blocking WS2812 LED driver (`hw/led.c`, `hw/include/ocb_hw/led.h`), later the UART helpers
- `hw/include/ocb_hw/pins.h`: the only pin definitions (GP0 TX, GP1 RX, GP16 LED, plus a debug-only toggle pin, GP15, in `fw-debug` builds); a CI grep forbids other pin numbers (AD-20)
