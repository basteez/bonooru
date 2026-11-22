# common

Code shared by bridge and controller: link protocol definitions, version, UART helpers.

- `proto/`: `ocb_proto`, the pure C99 codec (static library, no pico-sdk, no heap). Headers in `proto/include/ocb_proto/`: `consts.h` (every wire constant: `PROTO_VERSION`, message types, NAK reasons, EVENT codes, modes, profile ids, flag and button bits, sizes, link timing, baud rate), `err.h`, `crc16.h` (CRC-16/CCITT-FALSE), `cobs.h`, `state.h` (canonical 8-byte state, `NEUTRAL`), `frame.h` (frame encode/decode, the `ocb_rx` receive splitter and its counters), `msgs.h` (one struct and codec per message), and the umbrella `ocb_proto.h`
- `proto/vectors/`: language-neutral conformance vectors shared by the firmware host tests and the app core tests (AD-14); a wire change edits these first. Format: `proto/vectors/README.md`
- `hw/`: `ocb_hw`, board drivers on pico-sdk; today the non-blocking WS2812 LED driver (`hw/led.c`, `hw/include/ocb_hw/led.h`), later the UART helpers
- `hw/include/ocb_hw/pins.h`: the only pin definitions (GP0 TX, GP1 RX, GP16 LED, plus a debug-only toggle pin, GP15, in `fw-debug` builds); a CI grep forbids other pin numbers (AD-20)
