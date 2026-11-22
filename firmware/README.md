# Firmware

Single CMake project on pico-sdk + TinyUSB with two targets, `bridge` and `controller`,
both for the Waveshare RP2040-Zero. Shared code lives in `common/`.

Both targets blink the onboard RGB LED (Story 2.1): `bridge` blue at 1 Hz, `controller`
amber at 2 Hz. The controller also runs the board link on core 1 (Story 2.6).

## Pinned toolchain (AD-1)

| Tool | Pin | Where |
|---|---|---|
| pico-sdk | tag `2.3.1`, submodule `external/pico-sdk` | only its `lib/tinyusb` is initialised (TinyUSB 0.18.0) |
| Arm GNU Toolchain | `15.2.Rel1` | downloaded by `tools/install-tools.sh` |
| picotool | `2.3.1` (prebuilt) | downloaded by `tools/install-tools.sh` |
| CMake | ≥ 3.25 | host install |

Every pin (versions, URLs, SHA256, submodule SHAs) lives in `tools/toolchain.env`. Each
SDK or toolchain upgrade is its own PR. `PICO_TINYUSB_PATH` is never set, and picotool
is never fetched at build time: configure fails if `picotool_DIR` is missing.

## Local build

Works on macOS arm64 and Linux x86_64 from a fresh clone. Needs `cmake` (≥ 3.25),
`ninja`, a host C/C++ compiler (for `pioasm` and the host tests), `curl`, `git`.
Run from the repo root:

```sh
# One-time: download and verify the pinned tools into firmware/.tools (git-ignored)
firmware/tools/install-tools.sh
export OCB_ARM_TOOLCHAIN=...   # as printed by install-tools.sh
export OCB_PICOTOOL_DIR=...    # as printed by install-tools.sh

# Submodules: pico-sdk, then only its TinyUSB (never --recursive)
git submodule update --init
git -C firmware/external/pico-sdk submodule update --init --depth 1 lib/tinyusb

cd firmware
cmake --preset fw-release && cmake --build --preset fw-release
cmake --preset host-tests && cmake --build --preset host-tests && ctest --preset host-tests
cd ..
firmware/tools/check-pins.sh
```

Presets (`CMakePresets.json`):

- `fw-release`: release UF2s, `build/fw-release/bridge/bridge.uf2` and
  `build/fw-release/controller/controller.uf2`.
- `fw-debug`: debug build, defines `OCB_DEBUG_BUILD=1` (debug-only pins and aids).
- The Story 2.7 bench-only preset `fw-debug-bench` (a faked `console_ready`) was removed by
  Story 2.3 D15: `console_ready` now comes from the real P2 handshake (the console, or
  `tools/p2_probe.py handshake --hold N` on the bench Mac).
- Controller options (cache variables, any firmware preset): `OCB_PROFILE` (the HID profile,
  `P2` only in v1; `P1` / `P3` stop the configuration, R6) and `OCB_P2_BINTERVAL` (the P2
  endpoint interval in ms, 1..16, default 1; the real controller's 8 is the comparison build,
  `cmake --preset fw-debug -DOCB_P2_BINTERVAL=8`).
- `host-tests`: native compiler, ctest, no pico-sdk (`ctest --preset host-tests` fails
  if no test is found). Built with AddressSanitizer and UBSan (`OCB_SANITIZE=ON`); if a
  sanitizer runtime is missing on a machine, configure with `-DOCB_SANITIZE=OFF`.

Homebrew's `picotool` 2.3.1 is an acceptable local alternative to the downloaded one
(`export OCB_PICOTOOL_DIR="$(brew --prefix picotool)/lib/cmake"`: its `picotool/`
subfolder holds `picotoolConfig.cmake`). Homebrew's Arm toolchain is not: the cask is 15.3.rel2 and the
formula 16.x, not the pin, and configure rejects any compiler whose banner is not
`Arm GNU Toolchain 15.2.Rel1` (override with `-DOCB_ALLOW_OTHER_TOOLCHAIN=ON` only for a
local experiment, never in CI).

The firmware version comes from `git describe` on a `vX.Y.Z` tag at configure time,
fallback `0.0.0-dev` (AD-14); `picotool info <file>.uf2` shows it.

## Flashing by hand

Hold BOOT on the Zero while plugging it into USB, then copy the UF2 onto the `RPI-RP2`
drive that appears. The board reboots into the new firmware.

## Pin check (AD-20)

`common/hw/include/ocb_hw/pins.h` is the only file where a GPIO number may appear.
`tools/check-pins.sh` (also run by CI and by ctest as `pin_grep_selftest`) fails on a
pin number anywhere else in the firmware sources; use the `OCB_PIN_*` names.

## Protocol checks (AD-2, AD-14)

- `tools/check-proto-version.sh`: every `common/proto/vectors/*.txt` header equals
  `OCB_PROTO_VERSION` in `consts.h` (CI step; ctest `proto_version_check` with a self-test).
- `tools/check-proto-pure.sh`: `common/proto/` includes only `<stdint.h> <stddef.h>
  <stdbool.h> <string.h> <limits.h>` and `ocb_proto/*` headers (CI step; ctest
  `proto_pure_check` with a self-test).
- `tools/check-core-pure.sh`: `controller/core/` (`.c .h .cpp .cc .S .inc`) includes only the
  same standard headers, `ocb_proto/*` and `ocb_core/*`, names no heap, I/O, clock, sleep or
  multicore identifier, and its `CMakeLists.txt` links no `pico_*`, `hardware_*` or `ocb_hw`
  library (CI step; ctest `core_pure_check` with a self-test).
- `tools/check-link-pure.sh`: the same rules for `controller/link/` (`ocb_link`), which may also
  include `ocb_link/*`; its banned identifier list must equal the core gate's (CI step; ctest
  `link_pure_check` with a self-test and the drift check).
- `tools/check-p2-pure.sh` (Story 2.3): the same rules for `controller/profile_p2/` (`ocb_p2`),
  with a narrower allow-list (`ocb_proto/*` and `ocb_p2/*`, never `ocb_core` or `ocb_link`) and
  no TinyUSB (`tusb`, `class/hid`, `tinyusb*`); banned list in step with the core gate (CI step;
  ctest `p2_pure_check`).
- `tools/check-release-nolog.sh ELF` (Story 2.3 AC5a): the release controller ELF has no symbol
  of the P2 debug log (`ocb_dbglog_*`, `ocb_ctrl_dbglog`); `--expect-present` on the debug ELF
  proves the gate is not vacuous (both run in CI; needs `arm-none-eabi-nm` from
  `$OCB_ARM_TOOLCHAIN`).

## Host test names

Optional fuzz target (Story 2.3): `-DOCB_FUZZ=ON` on the host preset builds `fuzz_p2` with
libFuzzer; it needs clang with the libFuzzer runtime (on macOS Homebrew LLVM, e.g.
`CC=$(brew --prefix llvm)/bin/clang`; Apple clang has no libFuzzer). Not in CI.

ctest (`host-tests` preset): `test_pins`, `test_pins_debug`, `test_version`, `pin_grep_selftest`,
`test_vectors`, `test_proto_api`, `test_core_modes`, `test_core_end_late`, `test_core_sync`,
`test_core_mask`, `test_core_edge`, `proto_version_check`, `proto_pure_check`, `core_pure_check`,
`test_link_rx`, `test_link_session`, `test_link_gbn`, `test_link_nak`, `test_link_tx`,
`test_link_ready`, `test_link_fuzz`, `test_link_watchdog` (Story 2.7, F.21), `link_pure_check`,
`link_probe_selftest`, `docs_constants_check` (Story 2.7, F.22); Story 2.3: `test_p2_desc`,
`test_p2_handshake`, `test_p2_subcmd`, `test_p2_report`, `test_p2_interval`, `test_p2_fuzz`,
`p2_pure_check`, `test_link_dbglog`, `p2_probe_selftest` (F.24 to F.32).

## Bench sender (Story 2.6)

`tools/link_probe.py` speaks the binary link protocol over a serial port (a 3.3 V USB-UART
adapter on the controller's UART pins): `hello`, `watch`, `fill`, `soak`, `noise`, `try`,
`loopback`. Python 3 plus `pyserial` (`pip install pyserial`, needed only to open a port). Its
codec is written from `docs/link-protocol.md` and checked against the shared vectors by ctest
`link_probe_selftest`. Bench procedure: `docs/test-plan.md`, H.4 to H.7.

Story 2.7 adds the emulated bridge heartbeat (`--heartbeat`, `--no-heartbeat`,
`--hb-period-ms`) and one subcommand per bench step, each one process and one session ending
in `PASS` / `FAIL`: `linkup` (H.8), `silence` (H.9), `direct` and `playing` (H.11; since
Story 2.3 they need `console_ready` 1 from a real host), `breaks` (H.12); `--heartbeat soak` is H.10 (global options go before the subcommand). ctest runs them against the real link on a
fake clock (F.23).

Story 2.3 adds `p2check` (H.15: the console enumerates and handshakes, its sequence and polling
period) and `p2watch` (H.16: sleep, wake, Wired Communication off and on, `--dock`) to
`link_probe.py`, and `tools/p2_probe.py`, the Mac-side host of the P2 profile: `enum` (H.13,
`system_profiler` / `ioreg`, standard library only) and `handshake` (H.14, needs
`pip install hidapi`; `--hold N` keeps `console_ready` 1 for another tool). ctest: F.32.

## CI

`.github/workflows/firmware.yml` runs on every push and pull request: it checks the
submodule pins, installs the pinned tools, builds `fw-release` and `fw-debug`, runs the
host tests, the pin check, the purity gates (proto, core, link, P2) and the release-has-no-debug-log
gate, and uploads `bridge.uf2` and `controller.uf2` as the
`firmware-uf2` artifact. UF2s are never committed.

Third-party notices: `THIRD_PARTY_NOTICES.md`.
