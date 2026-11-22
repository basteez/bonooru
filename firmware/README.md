# Firmware

Single CMake project on pico-sdk + TinyUSB with two targets, `bridge` and `controller`,
both for the Waveshare RP2040-Zero. Shared code lives in `common/`.

At this stage (Story 2.1) both targets only blink the onboard RGB LED: `bridge` blue at
1 Hz, `controller` amber at 2 Hz.

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
- `tools/check-core-pure.sh`: `controller/core/` includes only the same standard headers,
  `ocb_proto/*` and `ocb_core/*`, and names no heap, I/O, clock, sleep or multicore identifier
  (CI step; ctest `core_pure_check` with a self-test).

## CI

`.github/workflows/firmware.yml` runs on every push and pull request: it checks the
submodule pins, installs the pinned tools, builds `fw-release` and `fw-debug`, runs the
host tests and the pin check, and uploads `bridge.uf2` and `controller.uf2` as the
`firmware-uf2` artifact. UF2s are never committed.

Third-party notices: `THIRD_PARTY_NOTICES.md`.
