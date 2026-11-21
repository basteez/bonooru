# pico-examples ws2812 PIO program (vendored)

- File: `ws2812.pio`
- Source: https://github.com/raspberrypi/pico-examples/blob/sdk-2.3.1/pio/ws2812/ws2812.pio
- Tag: `sdk-2.3.1`
- Licence: BSD-3-Clause, Copyright (c) 2020 Raspberry Pi (Trading) Ltd.
- Not modified: a verbatim copy, header included. Do not edit it here; a change
  is a new verbatim copy from a newer tag.

`firmware/common/hw/CMakeLists.txt` runs `pioasm` on it at build time; the
generated header lands in the build tree and is never committed. Our driver
(`firmware/common/hw/led.c`) is our own code, derived in part from the
example's usage pattern. See `firmware/THIRD_PARTY_NOTICES.md`.
