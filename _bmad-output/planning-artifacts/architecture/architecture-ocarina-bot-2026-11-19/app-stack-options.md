# App stack options (companion to ARCHITECTURE-SPINE.md, AD-18)

Only one option is used. Which one depends on the `spikes/app-egui` verdict (research report §1, exit criteria (a)–(e)). Versions were checked on 2026-11-19 against crates.io and PyPI. Pin exact versions when the project is set up. The module names, boundaries and rules in the spine (AD-15–AD-17, conventions) apply to both options unchanged.

| Concern | Option A: Rust + egui/eframe (if spike passes) | Option B: Python 3.12 + PySide6 (otherwise) |
| --- | --- | --- |
| Toolchain | Rust 1.100.0, pinned in `app/rust-toolchain.toml` | CPython 3.12 (pinned because python-rtmidi wheels stop at 3.12); `uv` for envs and lock |
| Core/UI boundary enforcement | Cargo workspace: `ocb-core` (lib, no egui/eframe dependency) and `ocb-app` (bin). Adapters live in `ocb-app` or a separate `ocb-adapters` crate | Package `ocarina_bot`: `core/`, `adapters/`, `services/`, `ui/`; an `import-linter` 2.17 contract forbids `PySide6` and `adapters` imports from `core` |
| UI | egui/eframe 0.37.0 (pin exactly; 0.x API breaks on minor versions) | PySide6 6.12.0, QtWidgets only |
| Wizard (NFR-10) | hand-written state machine in `ui` | `QWizard` |
| Piano roll | egui `Painter` | `QGraphicsView` / `QPainter` |
| Worker ↔ UI messages (AD-16) | `std::thread` + `crossbeam-channel` 0.5.17; the worker calls `ctx.request_repaint()` | `threading.Thread` + `queue.Queue`, with a Qt signal relay into the UI thread (the core never sees Qt) |
| Serial + discovery | `serialport` 4.10.1 (MPL-2.0: needs NFR-07 amended) — `available_ports()` gives VID/PID/product/serial | `pyserial` 3.5 `list_ports` (not `QSerialPort`, so the core and adapters stay Qt-free) |
| `.mid` import | `midly` 0.5.3 | `mido` 1.3.3 |
| Profile TOML | `toml` 1.2.1 + `serde` 1.0.229 | stdlib `tomllib` (read); writing calibration output uses `tomli-w` 1.2.0 |
| Audio preview | `rodio` 0.23.0 / `cpal` | `sounddevice` 0.5.6 (not QtMultimedia, which avoids bundling LGPL FFmpeg) |
| Drives (`RPI-RP2`) | `sysinfo` 0.40.4 disks + `std::fs` | `psutil` 7.2.2 `disk_partitions` + `shutil` (keeps it Qt-free) |
| File dialogs | `rfd` 0.18.0 | `QFileDialog` |
| Config/log dirs | `directories` 6.0.0 | `platformdirs` 4.14.7 |
| Live MIDI (M10) | `midir` 0.11.1 | `python-rtmidi` 1.5.8 (wheels only up to 3.12) |
| Tests | `cargo test`; `proptest` 1.12.0 for COBS/pipeline properties; vectors read from `firmware/common/proto/vectors/` | `pytest` 9.2.0 + `hypothesis` 6.170.15; same vectors |
| Packaging | one binary per OS; macOS `.app` via `cargo-packager` 0.11.8 (or a script); Windows `.exe`/installer; Linux AppImage or tarball | PyInstaller 6.22.3 **onedir**, no UPX, version info embedded (LGPL replaceability + fewer antivirus hits) |
| Licence impact | NFR-07 + MPL-2.0 (unmodified `serialport`); alternatively `serial2` + `nusb` to stay literal | NFR-07 + LGPL-3 (Qt, dynamically linked, onedir); LGPL notice + source pointer shipped |
| Platform floor / runtime deps | Linux: `libudev-dev` at build time, `libudev.so.1` at run time | PySide6 6.12 needs macOS 14+ and glibc 2.34+; `sounddevice` needs `libportaudio2` on Linux; build on the oldest supported glibc |
| Maintenance risks | `cargo-packager` has had no release for about 12 months (fallback: a bundle script); egui is 0.x | `pyserial` has had no release since 2020; Python 3.12 receives security fixes only (until Oct 2028) |
| Licence gate (AD-21) | `cargo-deny` licences check | `pip-licenses --fail-on` |

## Test strategy (both options)

| Layer | What | Where |
| --- | --- | --- |
| Wire conformance | shared vectors (AD-14): COBS, CRC, every message, `NEUTRAL` | firmware `host-tests` + app core tests |
| Firmware core | queue, scheduler with a fake clock, mode table (AD-8), watchdogs (AD-5), safety mask (AD-9), bridge framer (AD-4) | `firmware/tests` via ctest |
| App core | score parsers, pipeline properties (sorted, onsets never shifted, min press/gap respected, END last), range-fit report, fingering DP, profile validation/migration | app unit + property tests |
| Golden | reference piece → timeline snapshot; text score and `.mid` of the same melody → identical note list (CAP-1) | app tests |
| Link session (sans-IO) | HELLO/INFO, version mismatch, go-back-N under injected loss/corruption, credit flow, PING cadence | app tests against an in-test fake controller |
| Hardware in the loop | spikes, T.1–T.5, W.1–W.2, A.1–A.5 | `docs/test-plan.md`, manual |
| GUI | starts headless in CI and exits (smoke test) | CI 3-OS matrix |
