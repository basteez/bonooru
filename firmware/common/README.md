# common

Code shared by bridge and controller: link protocol definitions, version, UART helpers.

- `proto/`: `ocb_proto`, the pure C99 codec (static library, no pico-sdk, no heap). Headers in `proto/include/ocb_proto/`: `consts.h` (every wire constant: `PROTO_VERSION`, message types, NAK reasons, EVENT codes, modes, profile ids, flag and button bits, sizes, link timing, baud rate), `err.h`, `crc16.h` (CRC-16/CCITT-FALSE), `cobs.h`, `state.h` (canonical 8-byte state, `NEUTRAL`), `frame.h` (frame encode/decode, the `ocb_rx` receive splitter and its counters), `msgs.h` (one struct and codec per message), and the umbrella `ocb_proto.h`
- `proto/vectors/`: language-neutral conformance vectors shared by the firmware host tests and the app core tests (AD-14); a wire change edits these first. Format: `proto/vectors/README.md`
- `hw/`: `ocb_hw`, board drivers on pico-sdk: the non-blocking WS2812 LED driver (`hw/led.c`, `hw/include/ocb_hw/led.h`) and the board-link UART driver `ocb_hw/uart_link` (`hw/uart_link.c`, Story 2.6: UART0 at `OCB_LINK_BAUD` 8N1, IRQ-driven RX handing each data-register value with its error bits to a sink, all-or-nothing TX into a 128-byte ring drained without waiting, RX pull-down, TX high-Z until enabled; no frame knowledge, shared with the bridge)
- `hw/include/ocb_hw/pins.h`: the only pin definitions (GP0 TX, GP1 RX, GP16 LED, plus a debug-only toggle pin, GP15, in `fw-debug` builds); a CI grep forbids other pin numbers (AD-20)
