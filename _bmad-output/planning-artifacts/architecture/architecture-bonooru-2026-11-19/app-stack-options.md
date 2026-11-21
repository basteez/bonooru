# App stack (companion to ARCHITECTURE-SPINE.md, AD-18)

**Decided 2026-11-20 (ADR-008): Rust + egui/eframe.** The `spikes/app-egui` spike passed and the owner's hand test passed (`spikes/app-egui/SPIKE-REPORT.md`). Versions were checked on 2026-11-19 against crates.io; the spike used the same versions. Pin exact versions when `app/` is set up. The module names, boundaries and rules in the spine (AD-15–AD-17, conventions) apply.

| Concern | Rust + egui/eframe |
| --- | --- |
| Toolchain | Rust 1.100.0, pinned in `app/rust-toolchain.toml` |
| Core/UI boundary enforcement | Cargo workspace: `ocb-core` (lib, no egui/eframe dependency) and `ocb-app` (bin). Adapters live in `ocb-app` or a separate `ocb-adapters` crate |
| UI | egui/eframe 0.37.0 (pin exactly; 0.x API breaks on minor versions, and online examples often show older APIs) |
| Wizard (NFR-10) | hand-written state machine in `ui` |
| Piano roll | egui `Painter`; visible range by binary search (spike: p95 1.5 ms/frame with ~2,000 notes on screen) |
| Worker ↔ UI messages (AD-16) | `std::thread` + `crossbeam-channel` 0.5.17; the worker calls `ctx.request_repaint()` |
| Serial + discovery | `serialport` 4.10.1 (MPL-2.0, unmodified) — `available_ports()` gives VID/PID/product/serial. **macOS:** each device is listed as `/dev/cu.*` and `/dev/tty.*`; show and open only `cu.*` |
| `.mid` import | `midly` 0.5.3 |
| Profile TOML | `toml` 1.2.1 + `serde` 1.0.229 |
| Audio preview | `rodio` 0.23.0 / `cpal`; the audio thread's sample counter is the preview clock. Opening the output can fail transiently (Bluetooth): show a clear error with Retry, re-open the output when the default device changes, never fall back to silence |
| Drives (`RPI-RP2`) | `sysinfo` 0.40.4 disks + `std::fs`; copy with a plain read/write loop, not `fs::copy` (macOS xattr copy fails on `RPI-RP2`) |
| File dialogs | `rfd` 0.18.0 |
| Config/log dirs | `directories` 6.0.0 |
| Live MIDI (M10) | `midir` 0.11.1 |
| Tests | `cargo test`; `proptest` 1.12.0 for COBS/pipeline properties; vectors read from `firmware/common/proto/vectors/` |
| Packaging | one binary per OS (~11 MB release on macOS arm64); macOS universal `.app` via `cargo-packager` 0.11.8 (or a script); Windows `.exe`/installer; Linux AppImage |
| Licence impact | NFR-07 as amended: MPL-2.0 `serialport` (notice + source pointer); egui default fonts OFL-1.1 / Ubuntu-font-1.0 (licence texts shipped, or fonts replaced); dual-licensed crates taken on the permissive side |
| Platform floor / runtime deps | Linux: `libudev-dev` + ALSA dev packages at build time, `libudev.so.1` + `libasound.so.2` at run time |
| Maintenance risks | `cargo-packager` has had no release for about 12 months (fallback: a bundle script); egui, rodio and sysinfo are 0.x with API churn |
| Licence gate (AD-21) | `cargo-deny` licences check |

## Rejected alternative

Python 3.12 + PySide6 (QtWidgets), packaged with PyInstaller: bundle ~113 MB vs ~11 MB for the Rust binary; LGPL Qt + FFmpeg obligations; PyInstaller bundles trigger antivirus false positives (NFR-10). CustomTkinter, Dear PyGui, Flet and Toga were dropped earlier by the research report.

## Test strategy

| Layer | What | Where |
| --- | --- | --- |
| Wire conformance | shared vectors (AD-14): COBS, CRC, every message, `NEUTRAL` | firmware `host-tests` + app core tests |
| Firmware core | queue, scheduler with a fake clock, mode table (AD-8), watchdogs (AD-5), safety mask (AD-9), bridge framer (AD-4) | `firmware/tests` via ctest |
| App core | score parsers, pipeline properties (sorted, onsets never shifted, min press/gap respected, END last), range-fit report, fingering DP, profile validation/migration | app unit + property tests |
| Golden | reference piece → timeline snapshot; text score and `.mid` of the same melody → identical note list (CAP-1) | app tests |
| Link session (sans-IO) | HELLO/INFO, version mismatch, go-back-N under injected loss/corruption, credit flow, PING cadence | app tests against an in-test fake controller |
| Hardware in the loop | spikes, T.1–T.5, W.1–W.2, A.1–A.5 | `docs/test-plan.md`, manual |
| GUI | starts headless in CI and exits (smoke test) | CI 3-OS matrix |
