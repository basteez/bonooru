# Review: currency and reality check

- **Reviewed:** `ARCHITECTURE-SPINE.md`, `app-stack-options.md` (2026-11-19)
- **Lens:** were the committed technical facts (versions, existence, API behaviour) checked against reality rather than asserted from memory?
- **Method:** GitHub API (`raspberrypi/pico-sdk`, `hathach/tinyusb`, `rust-lang/rust`, `serialport/serialport-rs`, `pyserial/pyserial`, `raspberrypi/picotool`, `crabnebula-dev/cargo-packager`), a shallow clone of pico-sdk tag `2.3.1` (source grep), the crates.io API (with a User-Agent header), the PyPI JSON API, and HEAD requests to developer.arm.com.

## Verdict

**Pass, with one substantive correction and some clarifications.** Every version number in both documents matches the registry or upstream state on 2026-11-19. There are no stale or invented versions. One API fact the spine relies on is wrong on Windows (product-string discovery, AD-13 and `app-stack-options.md`). A few other claims are correct but incomplete in ways that will matter when the code is built (UART break, picotool pin, the PySide6 platform floor, the PortAudio runtime dependency).

## 1. Firmware facts

| Claim | Result | Evidence |
|---|---|---|
| pico-sdk `2.3.1` is current | **Verified.** It is the latest release (2026-09-04); 2.3.0 came out 2026-07-03 | `gh api repos/raspberrypi/pico-sdk/releases` |
| `lib/tinyusb` at 2.3.1 = TinyUSB 0.18.0 | **Verified.** The submodule SHA `86ad6e56c1700e85f1c5678607a762cfe3aa2f47` is exactly tag `0.18.0` (compare: `identical`). Upstream is now at 0.21.0 (2026-06-30), so the SDK pin is about 1000 commits behind. That is fine because AD-1 says never to override `PICO_TINYUSB_PATH`, but the spine should say that 0.18.0 is the SDK's choice, not the latest | `gh api repos/raspberrypi/pico-sdk/contents/lib?ref=2.3.1`; `compare/0.18.0...86ad6e5` |
| `PICO_BOARD=waveshare_rp2040_zero` | **Verified.** `src/boards/include/boards/waveshare_rp2040_zero.h` exists at 2.3.1: rp2040, 2 MB flash, W25Q080 boot2, default UART0 on GP0/GP1, WS2812 on GP16 (no plain LED pin) | SDK source |
| `pico_usb_reset` + `PICO_ENABLE_USB_RESET_VIA_BAUD_RATE=1` | **Verified, with a nuance worth writing down.** The library was split out of `pico_stdio_usb` in 2.3.0 (release notes). `usb_reset.c` defines a strong `tud_cdc_line_coding_cb` under `#if PICO_ENABLE_USB_RESET_VIA_BAUD_RATE`. When the app uses TinyUSB directly (`LIB_TINYUSB_DEVICE`), `usb_reset_config.h` **defaults the define to 0**, so the explicit `=1` is required, as the spine says. It must be a `target_compile_definitions` on `bridge`, because `usb_reset.c` is an INTERFACE source compiled with the target's flags. The bridge must also `target_link_libraries(... pico_usb_reset)`. The baud-rate path does not need the vendor interface steps 2–5 from `usb_reset.h` | `src/rp2_common/pico_usb_reset/{usb_reset.c,include/pico/usb_reset_config.h,include/pico/usb_reset.h}` |
| "bridge code must not define `tud_cdc_line_coding_cb`" | **Correct, and there is no real conflict for this design.** In TinyUSB 0.18.0 the callback is declared `TU_ATTR_WEAK` (`cdc_device.h:204`), and the SDK provides the only strong definition. A second definition would be a duplicate-symbol link error, not a silent override. The bridge's UART baud is a fixed build constant (460800), so it never needs the host's line coding. DTR (AD-4 flag bit0) comes from `tud_cdc_line_state_cb` / `tud_cdc_connected()`, a separate weak callback with no clash. **Limitation to record:** if a later need arises to react to line coding (for example, a mirrored baud), the SDK callback has no chaining hook. You would set the define to 0 and copy its 10-line body (`rom_reset_usb_boot_extra(...)` on 1200) into the bridge | TinyUSB `0.18.0` `src/class/cdc/cdc_device.h` |
| Arm GNU Toolchain `15.2.Rel1`, supported by the SDK | **Verified.** The download URL `.../15.2.rel1/binrel/arm-gnu-toolchain-15.2.rel1-x86_64-arm-none-eabi.tar.xz` returns 200. There is no 15.3 or 16.1 yet. The SDK CI matrix `.github/workflows/multi-gcc.yml` at 2.3.1 includes "GCC 15.2.1", and the 2.3.0 notes say that `-fanalyzer` warnings from GCC 15.2 were fixed | developer.arm.com; SDK source |
| RP2040 UART break detection (AD-5) | **Hardware: yes. SDK API: no. The spine understates this.** The PL011 has `UARTDR.BE` (bit 10), `UARTRIS.BERIS` and `UARTIMSC.BEIM`. The SDK has `uart_set_break()` (TX only) but no RX-break query. `uart_getc`/`uart_read_blocking` keep only the low 8 bits, so they drop BE. A break also reaches the FIFO as a `0x00` byte, which is a COBS delimiter, so it is harmless to the framer. `ocb_hw` must either read `uart_get_hw(u)->dr` raw (≥16-bit) or enable `BEIM`. This also means the Deferred item "DMA vs IRQ: AD-4/AD-11 do not depend on it" is only true if break detection uses the BEIM interrupt or the RIS poll. An 8-bit DREQ DMA of DR discards BE. Add one line to AD-5 or to `ocb_hw` | `hardware/regs/uart.h` (rp2040); `hardware/uart.h:568` |
| Multicore FIFO, core 0 → core 1 | **Verified.** The API exists: `multicore_fifo_wready/rvalid/push_timeout_us/pop_timeout_us`. Two facts to note: the RP2040 SIO FIFO is 8 × 32-bit per direction, and `pico_multicore` reuses it for `multicore_launch_core1` and `multicore_lockout`. Core 0 should push with `wready` or a timeout and never block, because core 1 drains it in its non-blocking loop | `pico/multicore.h` |
| `__not_in_flash_func` | **Verified** (`pico/platform/sections.h:273`; host build gets a no-op stub). It moves only the annotated function into RAM. SDK helpers it calls that are not inlined still run from XIP flash. This matters only if flash is ever erased or written at run time, but "super-loop in RAM" should not be read as "all code it touches is in RAM". The 2.3.0 `pico_set_not_in_flash_placement()` can relocate these sections if needed | SDK source; 2.3.0 notes |
| CMake ≥ 3.25 / presets | **Consistent.** The SDK requires `cmake_minimum_required(VERSION 3.13...3.27)`. Preset schema v6 is the highest that 3.25 reads. Write `"version": 6` in `CMakePresets.json` (v7 needs 3.27; v10 needs 3.31) | SDK `CMakeLists.txt` |
| AD-1 "no works-on-my-machine builds" | **Gap:** SDK 2.x needs **picotool** to produce UF2s (`tools/Findpicotool.cmake`). If none is installed, it *fetches and builds picotool from git at configure time*. Latest picotool is 2.3.1 (2026-09-05). Pin it in CI: install the release binary or set `PICOTOOL_FETCH_FROM_GIT_PATH`, and record it in the Stack table | SDK `tools/Findpicotool.cmake`; picotool releases |

## 2. App stack versions (`app-stack-options.md`)

All of these match the latest stable release on 2026-11-19.

**Option A (crates.io):**

| Crate | Spine | Registry (date, licence, MSRV) | OK |
|---|---|---|---|
| Rust toolchain | 1.100.0 | 1.100.0 (2026-11-12, `channel-rust-stable.toml`) | yes |
| egui / eframe | 0.37.0 | 0.37.0 (2026-10-15, MIT/Apache, MSRV 1.95) | yes |
| serialport | 4.10.1 MPL-2.0 | 4.10.1 (2026-09-08, **MPL-2.0**) | yes |
| midly | 0.5.3 | 0.5.3 (2023-01-01, Unlicense). Old but latest | yes |
| rodio | 0.23.0 | 0.23.0 (2026-10-11); optional dep `cpal ^0.18.2` (latest cpal 0.18.2) | yes |
| sysinfo | 0.40.4 | 0.40.4 (2026-11-16, MSRV 1.95) | yes |
| rfd | 0.18.0 | 0.18.0 (2026-11-07) | yes |
| crossbeam-channel | 0.5.17 | 0.5.17 (2026-09-05) | yes |
| toml | 1.2.1 | 1.2.1+spec-1.1.0 (2026-11-06) | yes |
| serde | 1.0.229 | 1.0.229 (2026-07-18) | yes |
| proptest | 1.12.0 | 1.12.0 (2026-10-10) | yes |
| cargo-packager | 0.11.8 | 0.11.8 (2025-11-27). Latest, but **no release in about 12 months**: treat as a maintenance risk, since a script fallback is already listed | yes, flag |
| directories | 6.0.0 | 6.0.0 (2025-01-12). Latest | yes |
| midir | 0.11.1 | 0.11.1 (2026-10-10, MIT) | yes |
| serial2 / nusb (alternative) | unversioned | 0.2.39 (BSD-2/Apache) / 0.2.9 (Apache/MIT) | n/a |

**Option B (PyPI):**

| Package | Spine | PyPI (date, notes) | OK |
|---|---|---|---|
| PySide6 | 6.12.0 | 6.12.0 (2026-10-08), abi3 cp310+, LGPL-3.0-only OR GPL | yes, but see §3 |
| PyInstaller | 6.22.3 | 6.22.3 (2026-09-12), Python <3.16 | yes |
| pyserial | 3.5 | 3.5 (2020-11-23). Latest release, though the repo had commits in 2026-02 | yes, flag (stale releases) |
| mido | 1.3.3 | 1.3.3 (2024-10-25) | yes |
| sounddevice | 0.5.6 | 0.5.6 (2026-08-17) | yes, but see §3 |
| psutil | 7.2.2 | 7.2.2 (2026-01-28) | yes |
| platformdirs | 4.14.7 | 4.14.7 | yes |
| import-linter | 2.17 | 2.17 (2026-11-18, released the day before the spine) | yes |
| pytest | 9.2.0 | 9.2.0 | yes |
| hypothesis | 6.170.15 | 6.170.15 (MPL-2.0; test-only, not shipped, so no NFR-07 impact) | yes |
| tomli-w | 1.2.0 | 1.2.0 | yes |
| python-rtmidi | 1.5.8, "wheels only up to 3.12" | 1.5.8 (2023-11-20) wheels: cp38–cp312 for win_amd64, macOS x86_64 and arm64, manylinux_2_28 x86_64 and aarch64. **No cp313+ and no win_arm64.** The claim and the 3.12 pin are correct | yes |

## 3. Findings

### F1 (substantive, AD-13 + app-stack "Serial + discovery"): matching by product string does not work on Windows with either library

- **serialport 4.10.1, Windows:** `info.product = self.property(SPDRP_FRIENDLYNAME)` (`src/windows/enumerate.rs`). For a CDC-ACM device bound to the inbox `usbser.sys`, that value is "USB Serial Device (COMn)", not the iProduct string `Ocarina Bot Bridge`. macOS (IOKit "USB Product Name") and Linux (udev `ID_MODEL`) do return iProduct.
- **pyserial 3.5, Windows:** `list_ports_windows.py` sets `description = FriendlyName` and never sets `product`.
- **Effect:** on Windows the "product string first" rule always falls through to VID:PID. That is exactly the per-OS heuristic split AD-13 says it prevents.
- **Fix:** make the primary key VID:PID **plus** a unique PID or `serial_number`. Both libraries parse the serial number from the parent instance ID on Windows. Alternatively, read `DEVPKEY_Device_BusReportedDeviceDesc` in the Windows adapter. Update both AD-13 and the app-stack row ("`available_ports()` gives VID/PID/product/serial" holds only on macOS and Linux).

### F2 (moderate, AD-5 / `ocb_hw` / Deferred DMA item)

UART break detection needs register-level code (BE flag or BEIM). The SDK read helpers drop the flag, and 8-bit DMA loses it. See §1.

### F3 (moderate, AD-1 / Stack)

picotool is an unpinned build dependency, fetched from git when it is not installed. Pin picotool 2.3.1 in CI.

### F4 (minor, Option B platform floor)

- PySide6 6.12.0 wheels are `macosx_14_0_universal2`, `manylinux_2_34_x86_64` and `manylinux_2_39_aarch64`. So Option B needs **macOS 14+** and **glibc ≥ 2.34** (for example Ubuntu 22.04+) at both build and run time.
- The note "build on an old-glibc base image" therefore has a hard floor of 2.34 and cannot go older.
- State the OS minimums, because they constrain NFRs on supported platforms.

### F5 (minor, Option B Linux)

- sounddevice ships a pure `any` wheel on Linux. It needs the system `libportaudio2` at run time (bundled on Windows and macOS only).
- Add this to the "Linux build note" row and to the PyInstaller bundle.
- Likewise, python-rtmidi needs ALSA or JACK `.so` files at run time on Linux.

### F6 (informational)

- **Python 3.12** has been security-only since April 2025, with end of life in October 2028. The pin is forced by python-rtmidi, which has not been released since 2023. This is acceptable, but record it as a risk, since live MIDI is M10 only. An alternative is to drop python-rtmidi and allow 3.13+.
- **cargo-packager** has had no release in 12 months.
- **pyserial** has had no release since 2020.
- **TinyUSB 0.18.0** is three minors behind upstream, by SDK choice.

## 4. Not verifiable from public sources

- The "research report exit criteria (a)–(e)" and "spikes/app-egui" are internal. They are not checked here.
- The 460800-baud and 20 ms / 100 ms timing budgets are engineering claims for the spikes, not currency facts.

## Sources

- https://github.com/raspberrypi/pico-sdk/releases/tag/2.3.1 and /2.3.0 (release notes: `pico_usb_reset` split, GCC 15.2)
- pico-sdk@2.3.1: `src/rp2_common/pico_usb_reset/*`, `src/boards/include/boards/waveshare_rp2040_zero.h`, `src/rp2_common/hardware_uart/include/hardware/uart.h`, `src/rp2040/hardware_regs/include/hardware/regs/uart.h`, `src/rp2_common/pico_multicore/include/pico/multicore.h`, `src/rp2_common/pico_platform_sections/include/pico/platform/sections.h`, `tools/Findpicotool.cmake`, `.github/workflows/multi-gcc.yml`
- https://github.com/hathach/tinyusb tag 0.18.0 (`86ad6e5`), `src/class/cdc/cdc_device.h`
- https://developer.arm.com/-/media/Files/downloads/gnu/15.2.rel1/...
- https://static.rust-lang.org/dist/channel-rust-stable.toml
- crates.io API `/api/v1/crates/<name>`; PyPI `/pypi/<name>/json`
- serialport-rs v4.10.1 `src/windows/enumerate.rs`; pyserial v3.5 `serial/tools/list_ports_windows.py`
