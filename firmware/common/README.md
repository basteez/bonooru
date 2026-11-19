# common

Code shared by bridge and controller: link protocol definitions, version, UART helpers.

- `proto/`: `ocb_proto`, the pure C99 codec (COBS, CRC, frame types, messages, canonical state, `NEUTRAL`, `PROTO_VERSION`, link constants)
- `proto/vectors/`: language-neutral conformance vectors shared by the firmware host tests and the app core tests (AD-14); a wire change edits these first
- `hw/`: `ocb_hw`, UART helpers on pico-sdk
- `hw/pins.h`: the only pin definitions (GP0 TX, GP1 RX, GP16 LED, plus a debug-only toggle pin); a CI grep forbids other pin numbers (AD-20)
