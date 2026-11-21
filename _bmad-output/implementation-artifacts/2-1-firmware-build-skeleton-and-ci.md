---
baseline_commit: bd6205fc93358645cac6385ad481175b70524586
---

# Story 2.1: Firmware build skeleton and CI

Status: review

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As the owner,
I want a pinned firmware toolchain that builds two flashable UF2s in CI,
so that both boards are always built the same way from day one.

## Acceptance Criteria

Copied verbatim from `_bmad-output/planning-artifacts/epics.md`, Story 2.1. These are the definition of done.

**AC1.**
**Given** a fresh clone
**When** the owner runs `git submodule update --init` and configures with the `fw-release` preset
**Then** pico-sdk is a submodule at `firmware/external/pico-sdk` on tag `2.3.1` with only `lib/tinyusb` initialised (TinyUSB 0.18.0), `PICO_TINYUSB_PATH` is not overridden, and `PICO_BOARD=waveshare_rp2040_zero` (AD-1)
**And** `firmware/CMakeLists.txt` (CMake ≥ 3.25) defines `ocb_proto`, `ocb_hw` and the executables `bridge` and `controller`, with presets `fw-release`, `fw-debug` and `host-tests` (AD-2)

**AC2.**
**Given** the two executables at this stage
**When** each UF2 is flashed on a Zero
**Then** it blinks the onboard WS2812 RGB LED (GP16, the small square between BOOT and RESET) through a PIO driver in `ocb_hw` (based on the pico-sdk `ws2812` PIO example, BSD-3), with a different colour per target (bridge vs controller), and configures no other pin
**And** the LED driver is non-blocking (PIO FIFO write, no busy wait), so later timing-critical loops can use it
**And** GP0, GP1 and GP16 (plus a debug-only toggle pin) are defined once in `ocb_hw/pins.h` (AD-20)

**AC3.**
**Given** a push to any branch
**When** the GitHub Actions firmware job runs on ubuntu
**Then** it uses the pinned Arm GNU Toolchain 15.2.Rel1 and picotool 2.3.1, uploads `bridge.uf2` and `controller.uf2`, runs `ctest --no-tests=error` for `host-tests` (this story adds one trivial host test, e.g. that `pins.h` defines GP0, GP1 and GP16, so the gate is real from day one), and fails if a grep finds a GPIO number outside `pins.h`
**And** the firmware version comes from `git describe` with fallback `0.0.0-dev` (AD-14)

### Reading notes on the AC (what the story file adds, none of it changes the AC)

- **AC1, "runs `git submodule update --init`"**: a plain `git submodule update --init` initialises only `firmware/external/pico-sdk`, not its nested `lib/tinyusb` (git does not recurse without `--recursive`, and `--recursive` would also pull cyw43-driver, lwip, mbedtls and btstack, which we do not want). The AC is met by a second documented command plus a CMake guard that stops with the exact command if `lib/tinyusb` is empty. See Decision D8.
- **AC2, "GP16 ... small square between BOOT and RESET"**: verified in the SDK 2.3.1 board header (`PICO_DEFAULT_WS2812_PIN 16`) below. The LED is the only thing the firmware drives in this story; GP0 and GP1 are defined in `pins.h` but not configured.
- **AC3, "ubuntu"**: pin the runner to `ubuntu-24.04` (not `ubuntu-latest`) so the runner image does not drift under a pinned toolchain.

## Tasks / Subtasks

Tasks 1 to 9 are for the dev agent and need no hardware. **Task 10 needs the owner at the bench** and stays unchecked until the owner has verified on the boards. The dev agent never marks it done.

- [x] **Task 1: pico-sdk submodule** (AC: #1)
  - [x] 1.1 `git submodule add https://github.com/raspberrypi/pico-sdk firmware/external/pico-sdk`, check out tag `2.3.1`, commit the gitlink at that tag (verified: tag `2.3.1`, SDK 2.3.1 gitlinks `lib/tinyusb` to `86ad6e56c1700e85f1c5678607a762cfe3aa2f47` = TinyUSB tag `0.18.0`).
  - [x] 1.2 Initialise **only** the nested TinyUSB: `git -C firmware/external/pico-sdk submodule update --init --depth 1 lib/tinyusb`. Do not run `--recursive`. Add `shallow = true` to the top-level submodule entry in `.gitmodules` (clone speed only).
  - [x] 1.3 Do not commit anything inside the submodule and never set `PICO_TINYUSB_PATH` (CMake guard in Task 3).
  - [x] 1.4 Add `.tools/` (see Task 2) and any new build dirs to `.gitignore` if not already covered (`build/`, `firmware/build*/`, `*.uf2` already are).

- [x] **Task 2: Toolchain pins and installer** (AC: #3, Decision D1)
  - [x] 2.1 `firmware/tools/toolchain.env`: the single source of every pin, `KEY=value` lines readable by bash (`set -a; . file`) and by CMake (`file(STRINGS ... REGEX)`). Keys: `ARM_TOOLCHAIN_VERSION=15.2.Rel1`, `PICOTOOL_VERSION=2.3.1`, the URL and SHA256 for each of the four artefacts in "Pinned artefacts" below.
  - [x] 2.2 `firmware/tools/install-tools.sh` (bash, `set -euo pipefail`, runs on Linux x86_64 and macOS arm64): detects OS/arch, downloads with `curl -fsSL --retry 3`, verifies SHA256 (`sha256sum` or `shasum -a 256`) **before** unpacking, unpacks to `${OCB_TOOLS_DIR:-firmware/.tools}/arm-gnu-toolchain-<ver>/` and `.../picotool/`, is idempotent (skips when the target dir and a `.ok` stamp for the same pins exist), exits non-zero with a clear message on a checksum mismatch or an unsupported OS/arch, and prints the two paths to export (`OCB_ARM_TOOLCHAIN`, `OCB_PICOTOOL_DIR`). `shellcheck` clean.
  - [x] 2.3 Do not execute downloaded binaries in the installer other than `arm-none-eabi-gcc --version` as a final sanity print.

- [x] **Task 3: Top-level CMake and presets** (AC: #1, Decisions D2, D4, D8)
  - [x] 3.1 `firmware/CMakeLists.txt`, `cmake_minimum_required(VERSION 3.25)`. Before `project()`: if `OCB_HOST_TESTS` is OFF, run the guards, then `include(external/pico-sdk/pico_sdk_init.cmake)` directly (verified: it is the file `pico_sdk_import.cmake` ends up including, so no vendored copy of the import file is needed and no SDK fetch-from-git fallback is reachable). Guards, each a `FATAL_ERROR` with the fix in the message:
    - `external/pico-sdk/CMakeLists.txt` missing: "run `git submodule update --init`";
    - `external/pico-sdk/lib/tinyusb/tusb.h` missing: "run `git -C firmware/external/pico-sdk submodule update --init --depth 1 lib/tinyusb`";
    - `PICO_TINYUSB_PATH` defined as a CMake variable or an environment variable (AD-1);
    - `PICO_BOARD` not `waveshare_rp2040_zero` (set it in the preset and force it here);
    - `picotool_DIR` unset (AD-1: picotool is "never fetched at build time"; verified in SDK 2.3.1 `tools/CMakeLists.txt` that without `picotool_DIR` the SDK would download and build picotool from git, so the guard is what keeps AD-1 true locally). Also set `FETCHCONTENT_FULLY_DISCONNECTED=ON` in `fw-release` and `fw-debug` as a second layer.
  - [x] 3.2 Toolchain guard after `project()`: run `${CMAKE_C_COMPILER} --version` and require the banner to contain `Arm GNU Toolchain <ARM_TOOLCHAIN_VERSION>` read from `tools/toolchain.env`, else `FATAL_ERROR` unless `-DOCB_ALLOW_OTHER_TOOLCHAIN=ON` (for local experiments, never in presets or CI). The exact banner of 15.2.Rel1 is expected to read `arm-none-eabi-gcc (Arm GNU Toolchain 15.2.Rel1 (Build arm-15.NN)) 15.2.1 ...`; confirm on the first run and adjust the match, not the pin.
  - [x] 3.3 `project(bonooru_fw C CXX ASM)` for firmware builds, `project(bonooru_fw C)` for `host-tests`. C standard 11 for firmware targets, pure C99-compatible code in `ocb_proto` (Story 2.2 owns the content). Warnings `-Wall -Wextra -Werror` on our own targets only (not on SDK targets); if arm-gcc 15 warns inside SDK headers included by our sources, narrow with `-isystem` on the SDK include dirs rather than dropping `-Werror`.
  - [x] 3.4 Subdirectories: `common/proto` (target `ocb_proto`, an `INTERFACE` library with include dir `common/proto/include` and no sources yet, so Story 2.2 only adds sources and turns it `STATIC`), `common/hw` (see Task 4), `bridge`, `controller` (firmware builds only), `tests` (host builds only; `enable_testing()` there).
  - [x] 3.5 `firmware/CMakePresets.json`, `"version": 6` (needs CMake 3.25), build dirs under `${sourceDir}/build/<preset>` (already git-ignored). Configure presets: `fw-release` (`CMAKE_BUILD_TYPE=Release`), `fw-debug` (`Debug`, defines `OCB_DEBUG_BUILD=1` for our targets), `host-tests` (`OCB_HOST_TESTS=ON`, native compiler, no pico-sdk). `fw-*` cache variables: `PICO_BOARD=waveshare_rp2040_zero`, `PICO_TOOLCHAIN_PATH=$env{OCB_ARM_TOOLCHAIN}`, `picotool_DIR=$env{OCB_PICOTOOL_DIR}/picotool`, `FETCHCONTENT_FULLY_DISCONNECTED=ON`. Matching build and test presets (`host-tests` test preset runs `ctest --no-tests=error` with `--output-on-failure`).
  - [x] 3.6 `firmware/cmake/ocb_version.cmake` and `ocb_version.h.in` (Task 5).

- [x] **Task 4: `ocb_hw` with `pins.h` and the non-blocking WS2812 driver** (AC: #2, Decisions D3, D5, D6, D7)
  - [x] 4.1 `firmware/common/hw/include/ocb_hw/pins.h`: pure C, **no SDK includes** (host tests include it). The only place a GPIO number may appear (AD-20): `OCB_PIN_UART_TX 0u`, `OCB_PIN_UART_RX 1u`, `OCB_PIN_LED 16u`, and, only `#ifdef OCB_DEBUG_BUILD`, `OCB_PIN_DEBUG_TOGGLE 15u` (Decision D4). Comment each with its role, never write other GPIO numbers anywhere else, comments included.
  - [x] 4.2 Target `ocb_hw_pins` (`INTERFACE`, include dir only) exists in every build, including `host-tests`; `ocb_hw` (`STATIC`, firmware builds only) links it plus `pico_stdlib`, `hardware_pio`, `hardware_clocks`.
  - [x] 4.3 `firmware/third_party/pico-examples/ws2812/ws2812.pio`: **verbatim** copy of `pio/ws2812/ws2812.pio` from pico-examples tag `sdk-2.3.1` (BSD-3-Clause, `Copyright (c) 2020 Raspberry Pi (Trading) Ltd.`, SPDX line kept as is), plus a small `README.md` in that folder with the source URL, tag and "not modified". `pico_generate_pio_header(ocb_hw <that file> OUTPUT_DIR ${CMAKE_CURRENT_BINARY_DIR}/generated)`, so nothing generated is committed (the example writes into its source tree; we do not).
  - [x] 4.4 `firmware/common/hw/include/ocb_hw/led.h` and `led.c` (our own code, MIT, header comment per Decision D7). API: `void ocb_led_init(void);` and `bool ocb_led_set(uint8_t r, uint8_t g, uint8_t b);`. Init claims a free PIO state machine with `pio_claim_free_sm_and_add_program_for_gpio_range` (or the non-`for_gpio_range` variant) and calls the generated `ws2812_program_init(pio, sm, offset, OCB_PIN_LED, 800000, false)`; it is the only code that configures a pin in this story. `ocb_led_set` writes the pixel word `((g << 16) | (r << 8) | b) << 8` (GRB, 24-bit, left-aligned for the example's `out_shift`) with `pio_sm_is_tx_fifo_full()` checked first and `pio_sm_put()` (non-blocking); returns `false` if the FIFO is full (caller retries on its next pass), never calls `pio_sm_put_blocking` and never sleeps. **Do not copy the example's `put_pixel`**: it uses `pio_sm_put_blocking`, which is exactly what AC2 forbids.
  - [x] 4.5 Write only on a colour change or on a blink edge, never in a tight loop: the WS2812 needs a low period of at least ~280 us between frames to latch, and two words sent closer together than that merge. At 1 to 2 Hz this is a non-issue; say it in a comment so later callers keep it that way.
  - [x] 4.6 `ocb_hw` pulls in no UART, TinyUSB or stdio code in this story. Call neither `stdio_init_all()` nor any `uart_*`/`tusb_*`; in each exe CMake `pico_enable_stdio_uart(<t> 0)` and `pico_enable_stdio_usb(<t> 0)` so GP0/GP1 stay untouched (AC2 "configures no other pin") and TinyUSB is not linked yet.

- [x] **Task 5: Version from `git describe`** (AC: #3, Decision D6b)
  - [x] 5.1 `firmware/cmake/ocb_version.cmake`: `execute_process(git describe --tags --match "v[0-9]*.[0-9]*.[0-9]*" --long --dirty ...)` in the repo root, no `--always`. On success strip the leading `v` and shape `X.Y.Z[-N-gHASH][-dirty]`; on any failure (no tag, no `.git` as in a source tarball, git not installed) use `0.0.0-dev`. Set `OCB_VERSION_STRING` and numeric `OCB_VERSION_MAJOR/MINOR/PATCH` (parsed from the leading `X.Y.Z`, `0.0.0` for the fallback). Message the result at configure time.
  - [x] 5.2 Generate `ocb_version.h` into the build dir from `ocb_version.h.in` (`OCB_VERSION_STRING`, `OCB_VERSION_MAJOR`, `OCB_VERSION_MINOR`, `OCB_VERSION_PATCH`) on an `INTERFACE` target `ocb_version` that both exes link; also `pico_set_program_version(<exe> "${OCB_VERSION_STRING}")` and `pico_set_program_name` so `picotool info` shows it. The version is computed at configure time (CI configures fresh; say it in the file comment). Nothing else in firmware may compute a version (AD-14).
  - [x] 5.3 CI checks out with `fetch-depth: 0` so tags are visible; without a `v*` tag in the repo today the CI result is `0.0.0-dev`, which is expected.

- [x] **Task 6: Executables `bridge` and `controller`** (AC: #2, Decisions D5, D6)
  - [x] 6.1 `firmware/bridge/main.c` and `firmware/controller/main.c` plus a `CMakeLists.txt` each: `add_executable`, link `ocb_hw`, `ocb_proto`, `ocb_version`, `pico_add_extra_outputs` (UF2 + ELF + map; UF2 named `bridge.uf2` / `controller.uf2` in `build/<preset>/bridge/` and `.../controller/`). Each `main()` is `ocb_led_init();` then a super-loop that reads `time_us_32()` and toggles on a signed-difference compare (`(int32_t)(now - next_edge) >= 0`), advancing `next_edge` only after `ocb_led_set()` returned `true`. No `sleep_*`, no busy wait.
  - [x] 6.2 Colours and rhythm, per Decision D5: bridge = blue, 1 Hz (500 ms on, 500 ms off); controller = amber, 2 Hz (250 ms on, 250 ms off). Put the RGB and the half-period in named constants at the top of each `main.c` (the owner tunes them at the bench, Task 10).
  - [x] 6.3 Keep `main.c` thin: the later stories replace the super-loop with the real firmware (bridge: Story 3.1; controller: core 0 USB + core 1 loop, Stories 2.3 and 2.4). Do not add queue, UART, USB or multicore code now.

- [x] **Task 7: Host test, pin grep and its self-test** (AC: #3, Decision D6)
  - [x] 7.1 `firmware/tests/CMakeLists.txt` and `test_pins.c`: plain C, no framework (Story 2.2 may add one), `assert`-style with a non-zero exit code. Checks: `OCB_PIN_UART_TX == 0`, `OCB_PIN_UART_RX == 1`, `OCB_PIN_LED == 16`; compiled a second time with `-DOCB_DEBUG_BUILD=1`, `OCB_PIN_DEBUG_TOGGLE` exists and is none of 0, 1, 16; compiled without it, `OCB_PIN_DEBUG_TOGGLE` is **not** defined (two executables or two test registrations). Also a `test_version.c` that includes the generated `ocb_version.h` and checks the string is non-empty and the numeric macros agree with its leading `X.Y.Z` (`0.0.0-dev` included). Register with `add_test`; the preset runs `ctest --no-tests=error`.
  - [x] 7.2 `firmware/tools/check-pins.sh` (bash, POSIX `grep -E` only, must behave the same with macOS BSD grep and GNU grep: use `[[:space:]]` and `[0-9]`, avoid `\b`/`\d`/`-P`): scans `firmware/` per Decision D6, prints `file:line:text` for each hit, exits 1 on any hit, 0 on none, 2 on usage error. Takes an optional root dir argument so the self-test can point it at fixtures.
  - [x] 7.3 Self-test registered in ctest (`pin_grep_selftest`): runs the script on `firmware/tests/pin_check_fixtures/bad/*` (one file per pattern in D6, each must be reported) and `.../good/*` (uses `OCB_PIN_*`, mentions "the LED pin", must pass), and on the real tree (must pass). The fixture dir is excluded from the real scan (D6). This is what makes "fails if a grep finds a GPIO number outside `pins.h`" a tested gate and not a hope.

- [x] **Task 8: CI workflow `firmware.yml`** (AC: #3, Decisions D1, D2, D9)
  - [x] 8.1 `.github/workflows/firmware.yml`, style of `spike-app-egui.yml` (header comment, `permissions: contents: read`, `concurrency` group with cancel-in-progress, `actions/checkout@v7`, `actions/upload-artifact@v7`). Triggers: `push` (all branches, no path filter: AC says "a push to any branch"), `pull_request`, `workflow_dispatch`, and `workflow_call` (so the later app/release workflow can reuse the UF2 build, Story 8.1 and Epic 11).
  - [x] 8.2 Job `firmware` on `ubuntu-24.04` (CMake 3.31 and Ninja on the image; confirm CMake ≥ 3.25 with a `cmake --version` step): checkout with `fetch-depth: 0` and `submodules: false`; init submodules with the two commands of Task 1 (`git submodule update --init --depth 1 firmware/external/pico-sdk`, then the nested `lib/tinyusb`); **pin check step**: the SDK HEAD equals the commit of tag `2.3.1` and the TinyUSB HEAD equals `86ad6e56c1700e85f1c5678607a762cfe3aa2f47` (record the SDK commit SHA of tag 2.3.1 in `toolchain.env` when adding the submodule) (compare the gitlink SHAs, `git rev-parse HEAD` in each, to the two constants in `toolchain.env`; this needs no tags in the shallow clones), and `git submodule status` shows no other initialised nested submodule.
  - [x] 8.3 Cache: `actions/cache@v6` on `firmware/.tools`, key `fw-tools-${{ runner.os }}-${{ runner.arch }}-${{ hashFiles('firmware/tools/toolchain.env', 'firmware/tools/install-tools.sh') }}` (Decision D1); then run `firmware/tools/install-tools.sh` every time (it verifies or skips), then export `OCB_ARM_TOOLCHAIN`, `OCB_PICOTOOL_DIR` via `$GITHUB_ENV` and print `arm-none-eabi-gcc --version` and `picotool version` (expected `2.3.1`). The prebuilt picotool is expected to run without extra packages; if it needs `libusb-1.0-0` at load time, apt-install it and note it in this story's Debug Log (do not build picotool from source).
  - [x] 8.4 Build and test steps: `cmake --preset fw-release && cmake --build --preset fw-release`; the same for `fw-debug` (compile gate for the debug-only toggle pin, not uploaded); `cmake --preset host-tests && cmake --build --preset host-tests && ctest --preset host-tests` (includes `--no-tests=error`); `firmware/tools/check-pins.sh` as its own step (so a violation shows as its own red step even though ctest also runs it).
  - [x] 8.5 Upload step: `actions/upload-artifact@v7`, name `firmware-uf2`, `path:` the two files `firmware/build/fw-release/bridge/bridge.uf2` and `firmware/build/fw-release/controller/controller.uf2`, `if-no-files-found: error`, `retention-days: 14` like the spike workflow. UF2s are CI artefacts only; they are never committed (`*.uf2` is already in `.gitignore`), and no step may `git add` them.
  - [x] 8.6 Log the resolved firmware version in the job summary (`$GITHUB_STEP_SUMMARY`: version, SDK and TinyUSB SHAs, toolchain banner, picotool version), so a reviewer sees what was built.
  - [x] 8.7 Local check before pushing: `actionlint` if available. The workflow itself can only be proven by a push; the dev agent does not push (see guardrails), so say in the Completion Notes that CI is unverified until the owner pushes, and give the exact local command sequence that reproduces every CI step on macOS (Task 9.3).

- [x] **Task 9: Docs, notices, test plan** (AC: #1 to #3)
  - [x] 9.1 `firmware/THIRD_PARTY_NOTICES.md` (Decision D7): the pico-examples ws2812 entry (copyright line, source URL, tag, which file, what we changed: nothing in the `.pio`, our driver code is derived from the `ws2812_program_init` usage pattern), the pico-sdk entry (BSD-3, linked into both UF2s, with the full BSD-3-Clause text and `Copyright (c) 2020 Raspberry Pi (Trading) Ltd.`), the TinyUSB entry (MIT, `Copyright (c) 2018 hathach`, "initialised and pinned, not linked until Story 2.3/3.1"), and a line "no 2wiCC code in this tree (NFR-07, ADR-004)". Story 11.2 later merges it into the release notices; keep the format a plain Markdown list so it can be concatenated.
  - [x] 9.2 Update `firmware/README.md` (replace "Build setup is added by the first firmware story" with the real setup: one-time tools install, the two submodule commands, the three presets, where the UF2s land, how to run the pin check), `firmware/common/README.md` (pins and LED paths now `hw/include/ocb_hw/pins.h`, `hw/led.c`), `firmware/bridge/README.md` and `firmware/controller/README.md` (one line: currently only blinks; colour and rhythm). Root `README.md`: change the "CI (build + test) is added with the first story that produces code" line to say firmware CI exists (`firmware.yml`) and app CI arrives with the app. English only.
  - [x] 9.3 In `firmware/README.md`, a "Local build" section that works on macOS arm64 and Linux x86_64 from a fresh clone: `firmware/tools/install-tools.sh`, `export OCB_ARM_TOOLCHAIN=... OCB_PICOTOOL_DIR=...` (as printed), the two submodule commands, `cmake --preset fw-release && cmake --build --preset fw-release`, `cmake --preset host-tests && cmake --build --preset host-tests && ctest --preset host-tests`, `firmware/tools/check-pins.sh`. Say how to flash by hand (hold BOOT, plug, copy the UF2 to `RPI-RP2`), and that Homebrew's `picotool` 2.3.1 is an acceptable local alternative to the downloaded one but Homebrew's toolchain is not (cask is 15.3.rel2, not the pin; the CMake guard rejects it).
  - [x] 9.4 `docs/test-plan.md`: in "Firmware host tests" add row `F.0` (skeleton smoke: `pins.h` allow-list, version macros, pin grep self-test; pass = `ctest --no-tests=error` green; Related AD-1, AD-14, AD-20); in "Hardware-in-the-loop checks (M3/M4)" add row `H.0` "Skeleton LED check (Story 2.1): flash `bridge.uf2`, then `controller.uf2`, on a Zero each" with the pass criterion "bridge: blue, 1 Hz; controller: amber, 2 Hz; the two are told apart at a glance; LED not dazzling at arm's length; nothing else on the board changes (no other pin)" and an **empty Result cell**. Do not fill the Result cell: Task 10 is the owner's. Leave every other row and table alone.

- [ ] **Task 10: Manual board check: flash both UF2s, confirm LED colour/rhythm** (AC: #2, test plan row H.0) **[owner at the bench; unchecked until verified on hardware]**
  - [ ] 10.1 Download the `firmware-uf2` artifact of the green CI run (or use the local build), flash `bridge.uf2` on one Zero and `controller.uf2` on the other (BOOT held while plugging, copy to `RPI-RP2`), with the GND-continuity and "rewire only unplugged" rules of `docs/test-plan.md` "Hardware safety" in mind (the boards are flashed on the PC, not wired, so the rules matter only if they are on the breadboard).
  - [ ] 10.2 Confirm the LED colour and rhythm for each board (blue 1 Hz vs amber 2 Hz, or whatever the owner tuned), that the two are easy to tell apart, and that the brightness is comfortable. Record the result, the environment (host OS, board, firmware version from `picotool info` or the CI summary) in the `H.0` Result cell. If the colour order is swapped (some Zero clones use RGB, not GRB), fix the order constant in `led.c`, rebuild, re-check, and note it in Decision D5.
  - [ ] 10.3 Only the owner ticks this task and the Result cell. The dev agent leaves both empty and the story stays in `review` until then.

## Dev Notes

### What this story is

The first firmware story and the first CI story: a pinned, reproducible build that produces two blinking-LED UF2s, an empty-but-real protocol library target for Story 2.2 to fill, and the gates (host tests, version, pin grep) that every later firmware story runs through. It writes no protocol, USB, UART, multicore or scheduler code. If a later story needs something that is easy to get wrong here (pins, version, UF2 names, preset names), this story fixes it once.

### What Stories 2.2 and 2.3 (and later) need from the skeleton

- **Story 2.2** (`ocb_proto` + vectors): `ocb_proto` already exists as a CMake target in both firmware and `host-tests` builds, so 2.2 adds sources under `firmware/common/proto/` and flips it to `STATIC`; `firmware/tests/` already has a CMake that builds host executables and registers them with `add_test`, so F.9 is "one more test directory" (vectors live in `firmware/common/proto/vectors/`, test-plan F.9; AC there: CI fails if `PROTO_VERSION` differs from the vector header, which will be an extra CI step added next to the pin grep). The `host-tests` preset must never see pico-sdk, so `ocb_proto` must not include SDK headers (already the AD-2 rule). Keep tests framework-free or add one small vendored framework in 2.2; do not pick it here.
- **Story 2.3** (P2 profile, TinyUSB): TinyUSB is initialised and pinned but not linked; 2.3 adds `tinyusb_device` to the controller only. `PICO_TINYUSB_PATH` stays unset (guard). The debug toggle pin (D4) is first used there for the IN-transfer timing measurement.
- **Story 2.4/2.5** (core, scheduler): the LED driver is non-blocking by AC so it can be used from the core 1 loop; `ocb_hw` stays free of blocking calls. `pico_set_program_version` is how the firmware version later reaches `INFO.fw_ver` (AD-14); `OCB_VERSION_*` is the macro set to use.
- **Story 3.1** (bridge): the bridge links TinyUSB CDC; `bridge/CMakeLists.txt` already follows the pattern.
- **Story 8.1/Epic 11** (flasher, release): the UF2 names `bridge.uf2` and `controller.uf2` and the `firmware-uf2` artifact name are the interface; the release workflow calls this workflow through `workflow_call` instead of rebuilding differently (AD-14: UF2s bundled at the app's own version).

### Decisions (made autonomously, 2026-11-21; the owner can overrule any of them)

Each was researched first (see "Latest tech information") and the recommended option picked.

**D1. Pinning the toolchain and picotool in CI: download + SHA256 verify + `actions/cache`, driven by one script.**
Chosen: `firmware/tools/toolchain.env` holds URL and SHA256 for every artefact; `firmware/tools/install-tools.sh` downloads, verifies the checksum before unpacking and installs under `firmware/.tools/`; CI wraps it in `actions/cache` keyed on the hash of those two files. The same script is what the owner runs locally, so CI and local builds cannot drift. Why: the pins are then in the repo, reviewable, and an SDK/toolchain upgrade is a one-file diff (AD-1: "each SDK upgrade is its own PR"). Rejected: a marketplace action such as `carlosperate/arm-none-eabi-gcc-action` (a third-party action in the trust path of firmware we flash on hardware, its version lags Arm releases, and it does not cover picotool); `apt install gcc-arm-none-eabi` (Ubuntu's 13.x, not the pin); building picotool from source (adds libusb/mbedtls build, minutes per cold run, and is what the SDK silently does if `picotool_DIR` is missing, exactly what AD-1 forbids). picotool: the official prebuilt `picotool-2.3.1-x86_64-lin.tar.gz` from `raspberrypi/pico-sdk-tools` release `v2.3.1-0`, same version as the SDK tag, and the tarball ships the `picotoolConfig.cmake` that `-Dpicotool_DIR` needs. Cache note: the cache holds the extracted tools (restored without re-hashing); the installer still runs and re-verifies when it has to download, and a cache is scoped to this repository.

**D2. Where the firmware CI lives: a new workflow `.github/workflows/firmware.yml`, with `workflow_call`.**
The architecture sketch says `ci.yml (from the first code story)`; this story deviates on purpose: the app jobs (3 OSes, Rust) arrive in Epic 3/4 with different triggers and caches, and a failing firmware pin check should not be hidden in a 6-job file. `firmware.yml` also exposes `workflow_call`, so the later `ci.yml`/release workflow depends on it (`needs:`) and downloads the `firmware-uf2` artifact instead of rebuilding (epics: "app bundles embed UF2s built in the same run"). Not added to `spike-app-egui.yml`: that workflow is throwaway, path-filtered to `spikes/app-egui/**`, and will be deleted. No path filter on `firmware.yml` push (AC says any push); the cold run is a couple of minutes, a warm one under a minute.

**D3. The two LED colours: bridge = blue, controller = amber (R 16, G 6, B 0), both at about 6 % of full scale.**
Why these: blue and amber are the classic colour-vision-deficiency-safe pair (they differ on the blue-yellow axis, which red-green deficiencies keep), they are far apart in hue so they read at a glance even through the Zero's LED diffuser, and they match "bridge = cool = PC side, controller = warm = console side". The rhythm differs as well (D5) so identification does not rely on colour alone. Brightness (Hardware safety, eye and power): the WS2812 takes up to about 60 mA at full white and is blinding at close range; at a channel value of 16 of 255 it draws a few mA (negligible on a USB port or the Switch's), and is comfortable at arm's length. Amber uses G 6 because the green die is more efficient than red, so R 16 + G 6 looks amber rather than yellow-green. Both are named constants for the owner to tune at the bench (Task 10). No white (it lights all three dies, three times the current).

**D4. Debug toggle pin: GP15, defined only under `OCB_DEBUG_BUILD`.**
Constraints: the pin must exist on the Waveshare RP2040-Zero (the board in `docs/test-plan.md` "Test environment" and `PICO_BOARD=waveshare_rp2040_zero`), must not be GP0, GP1 or GP16, and must not be one of GP26 to GP29 (analogue-capable, shared with ADC, unnecessary risk). GP15 is on the opposite side of the board from GP0/GP1 on the Zero's castellated edge, so a probe clip on it cannot bridge to the UART wires, and it carries no special function on the Zero (the board's other defaults: I2C1 on GP6/GP7, SPI1 on GP10 to GP13, per the SDK board header). It is only a measurement aid (the IN-transfer toggle of Story 2.3, firmware self-timing of Story 2.5); with no logic analyser available (epic note) it is probed with the owner's scope or multimeter-class tools later, or just left unused. Release builds do not define it, so `pins.h` cannot even name it there (test F.0 checks both). **Dev check (Task 4.1)**: confirm in the Waveshare RP2040-Zero pinout image (https://www.waveshare.com/wiki/RP2040-Zero, "Pinout Definition") that GP15 is a broken-out pad; the page gives the pinout only as an image, so this could not be verified in text during story creation. If it is not, take the nearest broken-out GPIO that is not adjacent to GP0/GP1, update this decision and the F.0 expectation.

**D5. Blink rhythm: bridge 1 Hz, controller 2 Hz.** (Companion to D3.) Time-based on `time_us_32()` with a signed-difference compare; no `sleep_ms`, no timer alarms (alarms and cores belong to later stories). Wrap-safe, as the 32-bit µs counter wraps every ~71 min. The colour order is GRB (WS2812 on the Zero as in the pico-examples `urgb_u32`); if a given Zero turns out to be RGB, Task 10.2 fixes the order.

**D6. GPIO-number grep rule** (AD-20: "a CI grep forbids other pin numbers").
`firmware/tools/check-pins.sh` scans files under `firmware/` with extensions `.c .h .cpp .hpp .S .pio .cmake .in .json` and files named `CMakeLists.txt`. A **violation** is any of:
1. a token `GP<n>`, `GPIO<n>`, `GP_<n>`, `GPIO_<n>` or `gpio <n>` (case-insensitive, one or two digits), in code **or comments** (comments say "the LED pin", not "GP16"); this also catches numbers in strings and `#define`s;
2. a `gpio_*(` call whose first argument is a numeric literal (`gpio_init(5)`, `gpio_set_function(0x10, ...)`, `gpio_put(7, 1)`);
3. a numeric literal in the pin position of `pio_gpio_init`, `pio_sm_set_consecutive_pindirs`, `sm_config_set_{in,out,set,sideset,jmp}_pins`, `ws2812_program_init` (and `PIO` pin-taking helpers a later story adds: extend the list there, with a fixture);
4. any use of an SDK board default pin macro outside `pins.h`: `PICO_DEFAULT_[A-Z0-9_]*` containing `PIN`, `UART`, `I2C`, `SPI` or `WS2812`, and any `PICO_[A-Z0-9_]*_PIN` (so the board's defaults cannot sneak in as an unreviewed pin).
**Excluded**: `firmware/external/` (pico-sdk, TinyUSB), `firmware/third_party/` (vendored example, no pin numbers, verbatim by rule), `firmware/build/` and `firmware/build-*/`, `firmware/.tools/`, `firmware/common/hw/include/ocb_hw/pins.h` (the one allowed place), `firmware/tests/pin_check_fixtures/` (the script's own test inputs), `firmware/common/proto/vectors/` (hex data, not code). Markdown and docs are not scanned (the READMEs legitimately say "GP0, GP1, GP16"). **How CI runs it**: its own step in `firmware.yml` after the build, and again inside ctest as `pin_grep_selftest` (which also proves the script flags each pattern on fixtures). **Known limit** (stated, not hidden): a grep cannot see an indirect number (`#define MY_PIN 5` then `gpio_init(MY_PIN)`); rule 2 and 3 catch only literals. The mitigations are rule 1 (the `#define` that would hold the number has to be written without a GP token, so reviewers still see it) and the F.0 test pinning the exact set of `OCB_PIN_*` values. Review catches the rest. Rejected: banning every digit near `gpio` (false positives on `gpio_put(OCB_PIN_LED, 1)`); a clang-tidy or AST check (toolchain cost out of proportion for three pins).

**D6b. Version embedding (AD-14).** `git describe --tags --match "v[0-9]*.[0-9]*.[0-9]*" --long --dirty`, no `--always`: with no matching tag git exits non-zero and the fallback `0.0.0-dev` applies, which is also the answer when `.git` is missing (source tarball) or git is not installed. With a tag it gives `1.2.3-4-gabc1234[-dirty]`, `v` stripped; numeric `MAJOR/MINOR/PATCH` come from the leading `X.Y.Z` (these are the `[major, minor, patch]` bytes of `INFO.fw_ver` later). Computed at CMake configure time into a generated `ocb_version.h` in the build dir (never committed), set as the program version for `picotool info`. Why not `--always`: a bare hash would pass as a version and break the `X.Y.Z` parse; the spec says fallback `0.0.0-dev`. CI needs `fetch-depth: 0` or the tags are missing and every build says `0.0.0-dev`.

**D7. BSD-3 attribution for the ws2812 example (AD-21, NFR-07).**
Three layers: (a) the `.pio` is vendored **verbatim** in `firmware/third_party/pico-examples/ws2812/ws2812.pio` with its original `Copyright (c) 2020 Raspberry Pi (Trading) Ltd.` + `SPDX-License-Identifier: BSD-3-Clause` header untouched, plus a folder README with source URL and tag; (b) `led.c` carries a header comment: "Derived in part from the pico-examples `pio/ws2812` example (https://github.com/raspberrypi/pico-examples/tree/sdk-2.3.1/pio/ws2812), Copyright (c) 2020 Raspberry Pi (Trading) Ltd., BSD-3-Clause; modified: non-blocking write, GRB word packing, own claim/init wrapper"; (c) `firmware/THIRD_PARTY_NOTICES.md` with the full BSD-3 text and entries for pico-sdk and TinyUSB. Story 11.2 AC ("attributions for reused MIT/BSD material ... are in `firmware/`") is satisfied by this file and extended there. Not vendoring the example's `ws2812.c` (it is a demo program; we take only the PIO program and the init pattern, and nothing from it blocks). SPDX tags: use `SPDX-License-Identifier: BSD-3-Clause` on the verbatim `.pio`, and `MIT` on our own files with the derived-from comment on `led.c`. BSD-3 conditions we meet: source redistribution keeps the copyright, conditions and disclaimer (verbatim file + notices), binary redistribution reproduces them "in documentation" (the notices file, shipped with releases by Story 11.2), no endorsement use of the names.

**D8. Submodule init: two documented steps + guard.** As in the AC reading notes: `git submodule update --init` then `git -C firmware/external/pico-sdk submodule update --init --depth 1 lib/tinyusb`. The CMake guard (Task 3.1) fails fast with that exact second command, and `firmware/README.md` and CI use both. Rejected: `git submodule update --init --recursive` (initialises cyw43-driver, lwip, mbedtls, btstack, which breaks "only `lib/tinyusb` initialised" and costs minutes); a configure-time auto-init (a build system that mutates the git checkout is surprising and CI must stay explicit); a helper script (two commands do not need one). The CI pin check compares the SDK and TinyUSB gitlink SHAs to the values in `toolchain.env` and fails if any other nested submodule is initialised.

**D9. Scope boundary with Story 2.2.** `ocb_proto` is created here as an empty `INTERFACE` target with an include dir only, so the AC ("defines `ocb_proto`") holds and nothing about the wire format is decided here. `firmware/common/proto/vectors/` is not created here.

### Pinned artefacts (verified 2026-11-21, checksums are the published ones)

| Item | Pin | Source and checksum |
|---|---|---|
| pico-sdk | tag `2.3.1` (released 2026-09-05; `pico_sdk_version.cmake` = 2.3.1) | https://github.com/raspberrypi/pico-sdk/releases/tag/2.3.1 ; `.gitmodules` of the SDK lists `lib/tinyusb` among its five nested submodules |
| TinyUSB | `0.18.0`, gitlink `86ad6e56c1700e85f1c5678607a762cfe3aa2f47` at SDK 2.3.1 (equals the `0.18.0` tag) | https://github.com/hathach/tinyusb/releases/tag/0.18.0 |
| Arm GNU Toolchain | `15.2.Rel1`, x86_64 Linux host, `arm-none-eabi` | https://developer.arm.com/-/media/Files/downloads/gnu/15.2.rel1/binrel/arm-gnu-toolchain-15.2.rel1-x86_64-arm-none-eabi.tar.xz (155,499,480 bytes), SHA256 `597893282ac8c6ab1a4073977f2362990184599643b4c5ee34870a8215783a16` (the `.sha256asc` published by Arm; the file was downloaded and hashed during story creation and matches) |
| Arm GNU Toolchain, local macOS | `15.2.Rel1`, darwin-arm64 host | https://developer.arm.com/-/media/Files/downloads/gnu/15.2.rel1/binrel/arm-gnu-toolchain-15.2.rel1-darwin-arm64-arm-none-eabi.tar.xz (141,961,944 bytes), SHA256 `1938a84b7105c192e3fb4fa5e893ba25f425f7ddab40515ae608cd40f68669a8` (published `.sha256asc`) |
| picotool, CI | `2.3.1`, prebuilt x86_64 Linux, release `v2.3.1-0` of `raspberrypi/pico-sdk-tools` | https://github.com/raspberrypi/pico-sdk-tools/releases/download/v2.3.1-0/picotool-2.3.1-x86_64-lin.tar.gz , SHA256 `878a1a1e7d8336b8be60300c7218f366d9790c0d3a211b181d6d172e80b1248e` (GitHub release digest; downloaded and re-hashed during story creation, matches; the archive unpacks to `picotool/` with `picotool`, `picotoolConfig.cmake`, `picotoolTargets*.cmake`) |
| picotool, local macOS | `2.3.1`, prebuilt | https://github.com/raspberrypi/pico-sdk-tools/releases/download/v2.3.1-0/picotool-2.3.1-mac.zip , SHA256 `7bb3cea5d9f1b000fc758cf4b54774fda5c16a9899d342ef6b0c30138988793b` (GitHub release digest). Homebrew's `picotool` formula is also 2.3.1 (bottled for arm64) and is an acceptable local alternative |
| CMake / Ninja on CI | 3.31.6 / 1.13.2 on `ubuntu-24.04` | runner image readme (https://github.com/actions/runner-images/blob/main/images/ubuntu/Ubuntu2404-Readme.md) |

Not pinned by us on purpose: Homebrew's `gcc-arm-embedded` cask is now `15.3.rel2`, and `arm-none-eabi-gcc` formula is 16.2.0; neither is the 15.2.Rel1 pin, so local macOS builds use the downloaded toolchain and the CMake guard rejects the others.

### Developer guardrails

- English everywhere in the repo; chat with the owner is Italian.
- **No UF2, ELF, BIN, map or any build output is ever committed** (`*.uf2`, `*.elf`, `*.bin`, `*.hex`, `*.map`, `build/` are git-ignored; keep it that way). No 2wiCC code or binaries (NFR-07, ADR-004). No game screenshots, no recorded audio; never reference anything under `ux-designs/*/imports/`. Do not touch `_bmad/`, `bmad-*` or `gds-*` skill folders, do not modify `~/Documents/org/`.
- **Do not commit or push** (the owner asks for that). CI cannot be proven without a push: say so in the Completion Notes and do not claim "CI green" for something that never ran.
- Never fetch anything at build time. The only network use is the installer script (checksum-verified) and CI checkout; the build itself must work with the network off once the tools and submodules are in place.
- Never set `PICO_TINYUSB_PATH`, never initialise other SDK submodules, never use `--recursive`.
- Configure only GP16 (the LED) in this story. GP0, GP1 and the debug toggle are defined, not configured. Do not call `stdio_init_all()` or any `uart_*` (it would claim GP0/GP1).
- No blocking calls in `ocb_hw`: no `sleep_*`, no `*_blocking`, no busy-wait on a hardware flag.
- Do not execute downloaded archives beyond `arm-none-eabi-gcc --version` / `picotool version` as sanity checks; use scratch directories, not the repo, for experiments.
- Never claim more than the data proves: a colour or rhythm is "confirmed" only by the owner at the bench (Task 10); the dev agent's checks are build, host tests, and the pin grep.
- Naming per the architecture: C prefix `ocb_`, `snake_case.c`, macros `OCB_*`.
- Memory-backed preferences: ask the owner one decision at a time and keep it short; research before asking; do not re-check breadboard wiring unless it was changed.

### Architecture compliance

- **AD-1**: SDK submodule at `firmware/external/pico-sdk` tag 2.3.1, only `lib/tinyusb`, `PICO_TINYUSB_PATH` never overridden, picotool 2.3.1 installed and pinned, never fetched at build time (guard + `FETCHCONTENT_FULLY_DISCONNECTED`), `PICO_BOARD=waveshare_rp2040_zero`, Arm toolchain version pinned (15.2.Rel1), each SDK upgrade its own PR (note it in `firmware/README.md`).
- **AD-2**: one CMake project, CMake ≥ 3.25, `ocb_proto` (pure C99, empty now), `ocb_hw`, executables `bridge` and `controller`, presets `fw-release`, `fw-debug`, `host-tests` (native compiler + ctest, **no pico-sdk**: the `host-tests` configure must not include `pico_sdk_init.cmake`). Wire constants exist only in `ocb_proto` (none yet).
- **AD-14**: version from `git describe`, fallback `0.0.0-dev`, read by the firmware CMake only here. `PROTO_VERSION` and vectors are Story 2.2's.
- **AD-20**: only GP0, GP1, GP16 (+ debug toggle) in `pins.h`; CI grep; nothing else configured. The TX-release and RX pull-down rules arrive with the UART in Epic 2/3 (Stories 2.6, 3.1), not here.
- **AD-21 / NFR-07**: BSD-3 attribution (D7); licence of our own code MIT (repo LICENSE); no network in firmware (there is none).
- **NFR-06** (offline): the firmware has no network path; the build uses none after setup.
- **AD-11** (core split) is respected by omission: the blink runs on core 0 in a plain loop now; it is replaced by the real loops later. `ocb_hw` stays non-blocking so it is usable from the core 1 loop.

### Library and framework requirements

- pico-sdk 2.3.1 (`pico_stdlib`, `hardware_pio`, `hardware_clocks`, `pico_time`), PIO program from pico-examples (verbatim, BSD-3), pioasm built by the SDK from its own tree (needs only host `cmake` and a host C++ compiler; both present on `ubuntu-24.04` and macOS with Xcode CLT). No other library. No unit-test framework yet.
- The generated PIO header provides `ws2812_program`, `ws2812_program_init(PIO pio, uint sm, uint offset, uint pin, float freq, bool rgbw)` (shown in the example's `ws2812.pio`, SDK 2.3.1 era). The example passes `freq = 800000` and `rgbw = false`; the SDK's WS2812 timing constants (T1=3, T2=3, T3=4) are for WS2812/WS2812B/SK6812 and are fine for the Zero's LED.
- Pin and board facts from the SDK board header `src/boards/include/boards/waveshare_rp2040_zero.h` at 2.3.1: `PICO_DEFAULT_WS2812_PIN 16`, `PICO_DEFAULT_UART_TX_PIN 0`, `PICO_DEFAULT_UART_RX_PIN 1`, flash 2 MB, `PICO_BOOT_STAGE2_CHOOSE_W25Q080`. Our `pins.h` values agree with them; the board macros themselves must not be used by our code (D6 rule 4).

### File structure requirements

New files (all under `firmware/` unless noted):

| File | Action | Notes |
|---|---|---|
| `external/pico-sdk` (+ `../.gitmodules`) | NEW submodule | tag 2.3.1, only `lib/tinyusb` nested init |
| `CMakeLists.txt`, `CMakePresets.json` | NEW | AD-2 |
| `cmake/ocb_version.cmake`, `cmake/ocb_version.h.in` | NEW | D6b |
| `tools/toolchain.env`, `tools/install-tools.sh`, `tools/check-pins.sh` | NEW | D1, D6 |
| `common/proto/CMakeLists.txt`, `common/proto/include/` (placeholder, e.g. `.gitkeep`) | NEW | empty `ocb_proto` for Story 2.2 |
| `common/hw/CMakeLists.txt`, `common/hw/led.c`, `common/hw/include/ocb_hw/pins.h`, `.../led.h` | NEW | Task 4 |
| `third_party/pico-examples/ws2812/ws2812.pio`, `.../README.md` | NEW | verbatim, D7 |
| `bridge/CMakeLists.txt`, `bridge/main.c`, `controller/CMakeLists.txt`, `controller/main.c` | NEW | Task 6 |
| `tests/CMakeLists.txt`, `tests/test_pins.c`, `tests/test_version.c`, `tests/pin_check_fixtures/{good,bad}/*` | NEW | Task 7 |
| `THIRD_PARTY_NOTICES.md` | NEW | D7 |
| `README.md`, `common/README.md`, `bridge/README.md`, `controller/README.md` | UPDATE | Task 9.2; existing text states the planned structure |
| `../.github/workflows/firmware.yml` | NEW | Task 8 |
| `../.gitignore` | UPDATE | add `firmware/.tools/` |
| `../README.md` | UPDATE | CI line |
| `../docs/test-plan.md` | UPDATE | rows `F.0`, `H.0` (Result empty) |
| `../_bmad-output/implementation-artifacts/sprint-status.yaml` | UPDATE | story status as work moves |

Existing files that must not change: `spikes/`, `app/`, `docs/link-protocol.md` and the other `docs/*` except `test-plan.md`, the planning artifacts, `.github/workflows/spike-app-egui.yml`. The `firmware/` tree today holds only the four README files, so nothing in it is "UPDATE" apart from those.

### Testing requirements

- `ctest --no-tests=error` for `host-tests` is the CI gate; it runs `test_pins`, `test_version` and `pin_grep_selftest` (F.0). All three pass on macOS (local, from a fresh clone) and on `ubuntu-24.04` (CI).
- The pin grep must pass on the real tree and fail on every `bad` fixture, with both GNU grep (CI) and BSD grep (macOS). Add a fixture for each D6 rule.
- Build gates: `fw-release` and `fw-debug` both compile warning-free for both targets; `fw-debug` additionally proves `OCB_PIN_DEBUG_TOGGLE` exists only there.
- Not testable without hardware, and not claimed: the LED colours and rhythm (Task 10, row H.0); that GP15 is broken out on the owner's Zero (D4 dev check against the pinout image); UF2 flashing.
- No Rust, no Python tests are touched; `cargo`/`uv` are not involved.

### Previous story intelligence (Story 1.6 and Epic 1, done)

- Epic 1 was all spikes and Python; no firmware exists yet, so there is no firmware code pattern to follow, only repo conventions: one branch per story (`story/2-1-firmware-build-skeleton`), results in `docs/test-plan.md` and nowhere else, hardware results filled **only** by the owner from real runs ("never claim more than the data proves"), tables added empty first (Story 1.6 Part A did the same for its result tables).
- The 1.6 story split work into a dev-agent part and a part that needs the board "run by the coordinator with the owner (the dev agent stops before this)". Task 10 follows the same rule: the dev agent stops there and does not tick it.
- Owner preference carried over from 1.6: decisions that matter are recorded in the story with rationale (done here, D1 to D9), repo text is English, chat Italian, nothing generated or binary committed (1.6: no UF2s or 2wiCC binaries; 2wiCC UF2s used in spikes are kept outside the repo, `docs/test-plan.md` environment row).
- Hardware facts to keep: both boards are Waveshare RP2040-Zero; wiring is GP0/GP1/GND only; breadboard rails not bridged caused corrupted replies in Story 1.2 (relevant when the owner re-wires after flashing, not for this story's flash-and-look check); the reference cable is the Apple 1 m USB-C. The Switch is **not** needed for this story.
- Story 1.6's change log shows reviews caught wording and evidence overclaims (e.g. a hypothesis written as fact). Keep Decision D4 (GP15 broken out) and the CI "unverified until pushed" statements honest in the Completion Notes.

### Git intelligence

Recent commits: `bd6205f` merge of the Bonooru rename (`chore/rename-to-bonooru`), `5ffe0f4` rename, `2f90b00` merge of Story 1.6, `9a251f6` Story 1.6, `ea8d5a7` merge of 1.5. Pattern: one story branch, a squash-like single story commit, merge commits via PR; Python spike tests live next to scripts; the only CI is the throwaway `spike-app-egui.yml` (action versions in use there: `actions/checkout@v7`, `actions/upload-artifact@v7`; `permissions: contents: read`, `concurrency` with cancel-in-progress, 14-day artifact retention). Reuse that style. The project was renamed from `ocarina-bot` to Bonooru: use `bonooru`/`ocb_` naming only.

### Latest tech information

Researched 2026-11-21 (read-only):

- **pico-sdk 2.3.1** (2026-09-05, latest): https://github.com/raspberrypi/pico-sdk/releases/tag/2.3.1. Its `.gitmodules` lists tinyusb, cyw43-driver, lwip, mbedtls and btstack; `lib/tinyusb` is gitlinked at `86ad6e56...` = TinyUSB `0.18.0` (newest TinyUSB tag today is 0.21.0; stay on the SDK's pin, AD-1 forbids overriding). `pico_sdk_init.cmake` can be included directly before `project()`; `external/pico_sdk_import.cmake` only adds PICO_SDK_PATH / fetch-from-git handling around it. SDK 2.3.1 `tools/CMakeLists.txt` requires picotool ≥ 2.3.0 and, when `picotool_DIR` is not set and nothing is installed beside the build, **downloads and builds picotool from git**: that is the behaviour the guard prevents.
- **picotool 2.3.1** (2026-09-05, latest): prebuilt binaries are published by https://github.com/raspberrypi/pico-sdk-tools/releases/tag/v2.3.1-0 (assets for x86_64 and aarch64 Linux, macOS, Windows, with GitHub-computed SHA256 digests, listed above). Building from source (https://github.com/raspberrypi/picotool) needs libusb and more build time; not needed.
- **Arm GNU Toolchain 15.2.Rel1** exists; Arm publishes a `.sha256asc` next to each archive (URLs above). Note that Arm's newest release moves on (Homebrew has 15.3.rel2), so the pin is deliberately older than the newest.
- **pico-examples ws2812** (tag `sdk-2.3.1`): https://github.com/raspberrypi/pico-examples/tree/sdk-2.3.1/pio/ws2812 ; `ws2812.pio` is `Copyright (c) 2020 Raspberry Pi (Trading) Ltd.`, `SPDX-License-Identifier: BSD-3-Clause`, program `ws2812` with `.side_set 1` and T1=3, T2=3, T3=4, and a `ws2812_program_init` helper that joins the TX FIFO. The example `ws2812.c` sends pixels with `pio_sm_put_blocking(pio, sm, pixel_grb << 8u)`: our driver replaces that with a FIFO-full check and `pio_sm_put`. GRB order via `urgb_u32`.
- **Waveshare RP2040-Zero**: https://www.waveshare.com/wiki/RP2040-Zero (pinout is an image on the page). SDK board header confirms the WS2812 on GP16.
- **GitHub Actions**: `actions/checkout` v7.0.1, `actions/upload-artifact` v7.0.2 (both already used by the spike workflow), `actions/cache` v6.1.0 latest. `ubuntu-24.04` image: CMake 3.31.6, Ninja 1.13.2.
- **Not verified (flagged for the dev agent)**: that the prebuilt Linux picotool runs on the runner without extra packages (the SDK only needs it for UF2 conversion, not USB; confirm in the first CI run and apt-install `libusb-1.0-0` only if it fails to load); the exact banner string of the 15.2.Rel1 compiler (the story states the expected form and Task 3.2 says to confirm). No tool downloaded in story creation was executed.

### Project Context Rules

- No `project-context.md` exists in the repo.
- Repo content (files, comments, commit text) is English; chat with the owner is Italian.
- Never commit `_bmad/`, `bmad-*` or `gds-*` skills; only `_bmad-output/`.
- Never commit or reference anything under `ux-designs/*/imports/`, game screenshots, recorded audio or video, UF2 or other binaries.
- Do not edit `~/Documents/org/`. Do not commit or push unless the owner asks.
- Ask decisions one at a time and keep them short; search online or upstream before raising an open question.
- The user's global rule to use `ctx7` for library docs applies to documentation lookups; the research above used the upstream repositories and release metadata directly, and the dev agent should do the same for any SDK function signature it is unsure of (read the pinned `firmware/external/pico-sdk` headers, which are the authority for 2.3.1).

### Project Structure Notes

- The structure follows the architecture's Structural Seed, with two small refinements recorded here: `ocb_hw` headers live under `common/hw/include/ocb_hw/` so `#include "ocb_hw/pins.h"` matches the AC's name; `third_party/` holds the vendored example (outside `external/`, which is the git submodule area). `common/hw/README` line "hw/pins.h" is updated to the new path.
- The structural seed lists `ci.yml`; this story creates `firmware.yml` instead (D2). Whoever creates the app CI later composes the two.
- `controller/core`, `controller/platform`, `controller/profile_p2` and `bridge` internals from the seed are **not** created now (later stories).

### Open points for the owner (none block the story)

- D3/D5 colours and rhythm are a first guess; Task 10 is where they get tuned.
- D4 pin GP15 awaits the pinout-image check by the dev agent.
- If the owner prefers a single `ci.yml` over `firmware.yml` (D2), it is a file rename plus the `needs:` wiring later.

## Dev Agent Record

### Agent Model Used

Claude Opus 5.5 (claude-opus-5-5), gds-dev-story, 2026-11-21.

### Debug Log References

- Toolchain banner confirmed on first run (macOS arm64 download of 15.2.Rel1): `arm-none-eabi-gcc (Arm GNU Toolchain 15.2.Rel1 (Build arm-15.86)) 15.2.1 20251203`. The guard matches the substring `(Arm GNU Toolchain 15.2.Rel1 ` (version read from `toolchain.env`); tested with a fake wrapper printing a 15.3.Rel2 banner: configure fails with the fix message.
- pico-sdk tag `2.3.1` = commit `079c6f39023649b154152db30f1d781e884879bc` (recorded as `PICO_SDK_COMMIT` in `toolchain.env`); `lib/tinyusb` checked out at `86ad6e56c1700e85f1c5678607a762cfe3aa2f47`; the other four nested submodules stay uninitialised.
- Task 3.1 path correction: TinyUSB's `tusb.h` is at `lib/tinyusb/src/tusb.h`, not `lib/tinyusb/tusb.h`; the guard checks the real path.
- picotool Linux prebuilt (`picotool-2.3.1-x86_64-lin.tar.gz`, SHA256 re-verified locally): `readelf -d` shows `NEEDED libusb-1.0.so.0` (plus libstdc++, libm, libgcc_s, libc; highest symbol version GLIBC_2.34, runner has 2.39). It was not executed (not a Linux host). The workflow therefore has a step that apt-installs `libusb-1.0-0` only if `ldconfig -p` does not list `libusb-1.0.so.0`. The macOS zip bundles its own `libusb-1.0.0.dylib` (universal binary, `picotool version` → `picotool v2.3.1`).
- Guards exercised locally: `OCB_PICOTOOL_DIR` unset → fails (no silent picotool fetch); `PICO_TINYUSB_PATH` in env → fails; `-DPICO_BOARD=pico` → fails; wrong toolchain banner → fails. Configure with Homebrew picotool 2.3.1 (`OCB_PICOTOOL_DIR=$(brew --prefix picotool)/lib/cmake`) also works.
- `picotool info -a build/fw-release/bridge/bridge.uf2`: name `bonooru-bridge`, version `0.0.0-dev`, board `waveshare_rp2040_zero`, sdk 2.3.1, Fixed Pin Information "none". `nm` on the ELF: no `uart_*`, no `tud_*` symbols.
- Version logic tested in a scratch clone (deleted afterwards): tag `v1.2.3` → `1.2.3`; dirty → `1.2.3-dirty`; one commit later → `1.2.3-1-g2d1f8c2`; `.git` removed → `0.0.0-dev`.
- Pin-grep red check: a temporary `firmware/bridge/zz_violation.c` with `gpio_init(3)` made `pin_grep_selftest` fail; file removed.

### Implementation decisions taken during dev (the owner can overrule)

- **D4 confirmed, GP15 kept.** The Waveshare wiki gives the pinout only as an image; the Apache NuttX board page for the Waveshare RP2040-Zero (https://nuttx.apache.org/docs/latest/platforms/arm/rp2040/boards/waveshare-rp2040-zero/index.html) lists GPIO15 on edge pad 8 (pads numbered anticlockwise from the USB connector) and GPIO0/GPIO1 on pads 23/22, i.e. opposite edges. No special function listed for GPIO15. Physical check on the owner's board is still Task 10's (only if the pin is ever probed).
- **`ocb_hw` stays `STATIC` but compiles against the SDK `*_headers` targets** (`pico_stdlib_headers`, `hardware_pio_headers`, `hardware_clocks_headers`) and links the full SDK libraries `INTERFACE` only. Why: pico-sdk libraries are `INTERFACE` libraries with sources; linking them normally to a `STATIC` library would compile SDK sources into the archive and again into each executable.
- **Strict warnings via a source property** (`ocb_strict_warnings()` in `cmake/ocb_warnings.cmake`) on our own `.c` files only, because SDK sources compile inside each executable target; verified with `ninja -t commands` that `-Werror` appears only on our sources. No `-isystem` narrowing was needed (arm-gcc 15 is warning-free on our code + SDK headers).
- **Version shape on an exact tag**: `--long` always prints `-0-gHASH`; on an exact tag it is dropped so a release builds as `X.Y.Z` (or `X.Y.Z-dirty`); `-N-gHASH` is kept for N > 0.
- **Blink edge scheduling**: `next_edge = now + half_period` (not `next_edge += ...`) so two LED writes are never closer than a half period, even after a missed edge (WS2812 latch time).
- **`firmware.yml` concurrency group** uses the literal prefix `firmware-` instead of `${{ github.workflow }}`: under `workflow_call` the latter is the caller's name, and sharing the caller's group would deadlock or cancel it.
- **Host tests**: `test_pins` (release pin set), `test_pins_debug` (same source with `OCB_DEBUG_BUILD=1`), `test_version`, `pin_grep_selftest` (bash script `tests/pin_grep_selftest.sh`, 11 bad fixtures covering rules 1 to 4, one good fixture, the real tree, and the usage-error exit 2).
- **THIRD_PARTY_NOTICES.md** also carries the TinyUSB MIT text now (copyright line as in its LICENSE: `Copyright (c) 2018, hathach (tinyusb.org)`), so Story 2.3 has nothing to add when it links TinyUSB.
- Local tools: `ninja` and `actionlint` installed with Homebrew (Homebrew `picotool` 2.3.1 also installed to verify the README alternative). The pinned Arm toolchain and picotool were installed by `install-tools.sh` into `firmware/.tools/` (git-ignored).

### Completion Notes List

- Tasks 1 to 9 done and verified locally on macOS arm64 (CMake 4.4.5, Ninja, pinned Arm GNU Toolchain 15.2.Rel1, picotool 2.3.1): `fw-release` and `fw-debug` build both targets warning-free with `-Wall -Wextra -Werror` on our sources; `bridge.uf2` and `controller.uf2` land in `firmware/build/fw-release/{bridge,controller}/`; `ctest --preset host-tests` 4/4 passed (`noTestsAction: error`); `firmware/tools/check-pins.sh` ok; `shellcheck` clean on all scripts; `actionlint` clean on `firmware.yml`.
- **CI is unverified until the owner pushes**: the workflow has never run. First-run points to watch: the libusb step (expected to find the library on the image), the cache, and the `firmware-uf2` artifact.
- Local reproduction of every CI step (repo root, macOS arm64 or Linux x86_64): `firmware/tools/install-tools.sh`; export the two printed variables; `git submodule update --init --depth 1 firmware/external/pico-sdk`; `git -C firmware/external/pico-sdk submodule update --init --depth 1 lib/tinyusb`; `cd firmware && cmake --preset fw-release && cmake --build --preset fw-release && cmake --preset fw-debug && cmake --build --preset fw-debug && cmake --preset host-tests && cmake --build --preset host-tests && ctest --preset host-tests && cd .. && firmware/tools/check-pins.sh`.
- Firmware version today is `0.0.0-dev` (no `v*` tag in the repo), as expected.
- **Task 10 and the `H.0` Result cell are the owner's** (flash both UF2s, confirm colour and rhythm). The LED colours/rhythm are not confirmed by the dev agent; the story stays in `review` until then.
- Not committed (coordinator's job). The submodule gitlink and `.gitmodules` were staged by `git submodule add` (gitlink re-staged at the tag commit); nothing else staged.

### File List

- `.gitmodules` (new)
- `firmware/external/pico-sdk` (new submodule, gitlink at tag 2.3.1)
- `.gitignore` (modified: `firmware/.tools/`)
- `.github/workflows/firmware.yml` (new)
- `README.md` (modified: CI line)
- `docs/test-plan.md` (modified: rows F.0, H.0 with empty Result)
- `firmware/CMakeLists.txt`, `firmware/CMakePresets.json` (new)
- `firmware/cmake/ocb_guards.cmake`, `firmware/cmake/ocb_warnings.cmake`, `firmware/cmake/ocb_version.cmake`, `firmware/cmake/ocb_version.h.in` (new)
- `firmware/tools/toolchain.env`, `firmware/tools/install-tools.sh`, `firmware/tools/check-pins.sh` (new)
- `firmware/common/proto/CMakeLists.txt`, `firmware/common/proto/include/.gitkeep` (new)
- `firmware/common/hw/CMakeLists.txt`, `firmware/common/hw/led.c`, `firmware/common/hw/include/ocb_hw/pins.h`, `firmware/common/hw/include/ocb_hw/led.h` (new)
- `firmware/third_party/pico-examples/ws2812/ws2812.pio`, `firmware/third_party/pico-examples/ws2812/README.md` (new)
- `firmware/bridge/CMakeLists.txt`, `firmware/bridge/main.c`, `firmware/controller/CMakeLists.txt`, `firmware/controller/main.c` (new)
- `firmware/tests/CMakeLists.txt`, `firmware/tests/test_pins.c`, `firmware/tests/test_version.c`, `firmware/tests/pin_grep_selftest.sh` (new)
- `firmware/tests/pin_check_fixtures/bad/` (11 files: `rule1_gp_comment.c`, `rule1_gpio_space.cmake`, `rule1_gpio_underscore.h`, `rule2_gpio_init.c`, `rule2_gpio_set_function_hex.c`, `rule3_pindirs.c`, `rule3_pio_gpio_init.c`, `rule3_sideset_pins.c`, `rule3_ws2812_init.c`, `rule4_pico_default.c`, `rule4_pico_pin.h`), `firmware/tests/pin_check_fixtures/good/uses_pin_names.c` (new)
- `firmware/THIRD_PARTY_NOTICES.md` (new)
- `firmware/README.md`, `firmware/common/README.md`, `firmware/bridge/README.md`, `firmware/controller/README.md` (modified)
- `_bmad-output/implementation-artifacts/2-1-firmware-build-skeleton-and-ci.md`, `_bmad-output/implementation-artifacts/sprint-status.yaml` (modified: status)

## Change Log

- 2026-11-21: story created (ready-for-dev). Decisions D1 to D9 recorded; versions, URLs and checksums verified against upstream (all pinned versions exist). Task 10 (manual board check) reserved for the owner.
- 2026-11-21: Tasks 1 to 9 implemented (dev agent); local builds, host tests and pin grep green; CI unverified until pushed; D4 (GP15) confirmed against the NuttX board pad table; status → review. Task 10 (owner, bench) open.
