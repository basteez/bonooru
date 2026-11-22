# Third-party notices (firmware)

Third-party material in `firmware/` and in the firmware binaries
(`bridge.uf2`, `controller.uf2`). Plain Markdown list, so the release notices
(Story 11.2) can concatenate it.

- **pico-examples, `pio/ws2812` example** (BSD-3-Clause)
  - Copyright (c) 2020 Raspberry Pi (Trading) Ltd.
  - Source: https://github.com/raspberrypi/pico-examples/tree/sdk-2.3.1/pio/ws2812 (tag `sdk-2.3.1`)
  - Used: `ws2812.pio`, vendored verbatim in `firmware/third_party/pico-examples/ws2812/ws2812.pio` (not modified); assembled into both UF2s.
  - Derived: `firmware/common/hw/led.c` is our own code (MIT) that follows the example's `ws2812_program_init` usage pattern; changed: non-blocking FIFO write instead of `pio_sm_put_blocking`, GRB word packing, own claim/init wrapper. The example's `ws2812.c` is not copied.
  - Licence text: BSD-3-Clause, below.
- **Raspberry Pi Pico SDK (pico-sdk) 2.3.1** (BSD-3-Clause)
  - Copyright (c) 2020 Raspberry Pi (Trading) Ltd.
  - Source: https://github.com/raspberrypi/pico-sdk (tag `2.3.1`), git submodule at `firmware/external/pico-sdk`, not modified.
  - Used: linked into both UF2s.
  - Licence text: BSD-3-Clause, below.
- **TinyUSB 0.18.0** (MIT)
  - Copyright (c) 2018, hathach (tinyusb.org)
  - Source: https://github.com/hathach/tinyusb (tag `0.18.0`), the pico-sdk's pinned `lib/tinyusb` submodule, not modified.
  - Used: initialised and pinned, not linked until Stories 2.3 / 3.1. Licence text: MIT, below.
- ocb_proto: COBS and CRC-16 implemented from the published descriptions, no third-party code (references in the source headers).
- ocb_core: written from the project's own architecture decisions (AD-8, AD-9), no third-party code.
- ocb_link, uart_link, link_probe (Story 2.6): written from `docs/link-protocol.md` and the pico-sdk API, no third-party code.
- No 2wiCC code in this tree (NFR-07, ADR-004).

## BSD-3-Clause licence text (pico-sdk and pico-examples)

    Copyright 2020 (c) 2020 Raspberry Pi (Trading) Ltd.
    
    Redistribution and use in source and binary forms, with or without modification, are permitted provided that the
    following conditions are met:
    
    1. Redistributions of source code must retain the above copyright notice, this list of conditions and the following
       disclaimer.
    
    2. Redistributions in binary form must reproduce the above copyright notice, this list of conditions and the following
       disclaimer in the documentation and/or other materials provided with the distribution.
    
    3. Neither the name of the copyright holder nor the names of its contributors may be used to endorse or promote products
       derived from this software without specific prior written permission.
    
    THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES,
    INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
    DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
    SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
    SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY,
    WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF
    THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

## MIT licence text (TinyUSB)

    The MIT License (MIT)
    
    Copyright (c) 2018, hathach (tinyusb.org)
    
    Permission is hereby granted, free of charge, to any person obtaining a copy
    of this software and associated documentation files (the "Software"), to deal
    in the Software without restriction, including without limitation the rights
    to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
    copies of the Software, and to permit persons to whom the Software is
    furnished to do so, subject to the following conditions:
    
    The above copyright notice and this permission notice shall be included in
    all copies or substantial portions of the Software.
    
    THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
    IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
    FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
    AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
    LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
    OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
    THE SOFTWARE.

