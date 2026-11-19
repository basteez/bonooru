# Spike report: Ocarina Bot app in Rust + egui (ADR-008)

Date: 2026-11-19. Machine: macOS (Darwin 27.2), Apple Silicon, 120 Hz display. Toolchain: Rust 1.100.0.
Crates: eframe/egui 0.37.0 (wgpu backend), serialport 4.10.1, rodio 0.23.0 (playback only), sysinfo 0.40.4 (disk only), midly 0.5.3, crossbeam-channel 0.5.17.

**I wrote this spike without seeing the window.** All evidence comes from logs, unit tests and measurements, not from looking at the screen. Someone still has to look at it (see "Try by hand").

## Verdict

**Rust + egui is viable. No exit criterion failed.** Two criteria are PARTIAL because hardware and CI were not available, not because the stack fell short. Performance, size, start time and licences are all clearly fine. The real cost is API churn between crate versions (details below), not missing capability. Recommendation: go ahead with Rust + egui, provided the owner tries the window by hand and still enjoys writing Rust. Otherwise fall back to PySide6.

## Results per criterion

| # | Criterion | Result | Evidence |
|---|---|---|---|
| 1 | Device discovery | **PARTIAL** | `serialport::available_ports()` gives VID, PID, manufacturer, product and serial. `classify()` marks a product-filter match (green) above an RP2040 VID `0x2E8A` match (yellow), and both rules are unit-tested. A background worker thread re-scans every 1 s, sends events over a crossbeam channel and calls `ctx.request_repaint()`. A test drives it with a fake port and checks the events, the read path and that the UI is woken. Read-only open uses 115200 baud with DTR left as is. The code has no write call and no 1200-baud path (checked with grep). **Not tested with hardware: no USB device was attached during the run** (`ioreg` showed nothing). `--list` only found the Bluetooth and debug-console ports. |
| 2 | Piano roll | **PASS** | 2,000 synthetic monophonic notes are drawn with `Painter`. The visible range comes from a binary search. Drag or scroll pans; Ctrl/Cmd+scroll or pinch zooms around the cursor; a click seeks; the playhead can follow. `.mid` loading works through `midly`, with a tempo map and skyline reduction to one voice (unit-tested). Unattended stress run (`SPIKE_AUTOPLAY=1`), with zoom swinging between 1 and 600 px/s and up to **1,987 notes on screen**: eframe CPU time per frame was **p50 1.2 ms, p95 1.5 ms, max 1.8 ms**. Frame interval stayed at **8.3 ms (120 Hz vsync), p95 8.7 ms**. The only outlier was a single 60–266 ms first frame (wgpu start-up). Idle CPU was **0.2 %** (egui only repaints when something changes); animating at 120 Hz used 18 % of one core. RSS was 82 MB idle. |
| 3 | Audio preview | **PASS** (by log) | A custom `rodio::Source` turns the timeline into sine or square tones, with a 4 ms ramp so notes do not click. It owns the clock: the UI draws the playhead from the audio thread's sample counter (an atomic), so the two cannot drift. Over 8 s the audio clock stayed **9–20 ms behind wall clock with no growth**. Start/stop and seek-while-playing are implemented. Not compensated: device output latency (samples rendered ≠ samples heard, about 10–20 ms). **I could not hear it.** The default output device opened and pulled samples at the right rate, but whether it sounds right still needs a listen. |
| 4 | UF2 flasher logic | **PASS** | `app_core::uf2` has no GUI code. It finds volumes through `sysinfo`. A volume is accepted if it has `INFO_UF2.TXT` and either the label is `RPI-RP2` (macOS/Windows), the mount folder is named `RPI-RP2` (Linux udisks, where sysinfo reports `/dev/sdX` as the name), or the INFO file says RP2. Every UF2 block is checked (magic numbers, block numbering, RP2040 family ID). The copy is a plain read/write loop, not `fs::copy`, to avoid the macOS xattr failure on RPI-RP2. It then polls until the volume disappears, with a timeout. 6 unit tests cover this, including an end-to-end flash onto a fake drive with a fake bootloader thread that "reboots", a timeout case and a rejected-file case. The UI flashes only the simulated directory and refuses `/Volumes`, `/media`, `/run/media` and real mount points. **Never run against a real board** (on purpose). |
| 5 | CI for 3 OS | **PARTIAL** (not run) | `.github/workflows/spike-app-egui.yml` uses a matrix of ubuntu, windows and macos latest, and the path filter `spikes/app-egui/**`. Steps: apt deps, test, clippy `-D warnings`, release build, then upload (tar on Unix, because upload-artifact's zip drops the exec bit). It has not run on GitHub yet. Local checks instead: **Windows** `cargo clippy --target x86_64-pc-windows-msvc -D warnings` is clean (type-check only, not linked). **Linux** in Docker (Debian bookworm arm64, same apt list as CI): `cargo test` passes 16/16, the release build works (17 MB), and it needs `libudev.so.1` and `libasound.so.2` at run time. |
| – | Gamepad stub | **PASS** (logic) | 10 buttons (ABXY, L, R, D-pad), each with mouse press/release and a key mapping (Q/E = L/R, J/K/I/U = A/B/X/Y, arrows = D-pad). Key auto-repeat is ignored through egui's `repeat` flag and counted. Key and mouse are tracked separately, so letting go of one does not release a button the other still holds. Everything is released when the window loses focus. Key-ups are read even on other tabs, so a button cannot stick. Each change logs the 8-byte controller state from the link protocol. About 135 UI lines plus 170 core lines including tests: a small effort. |

## Numbers (macOS arm64)

| Metric | Value |
|---|---|
| Release binary (strip, thin LTO, 1 CGU) | **10.9 MB** (10,931,200 B); **4.8 MB zipped** (`zip -9`) |
| Linux arm64 release binary (Docker) | 17.1 MB |
| Cold start, process start → first frame | **~115 ms** warm (4 runs: 116–122 ms); **530 ms** on the very first launch of a new binary (Gatekeeper scan and Metal shader caches) |
| Clean release build (deps already downloaded) | 28.7 s; incremental release 9.0 s; incremental debug 3.8 s |
| `target/` size | 2.2 GB |
| Lines of code | 2,388 total. Core (GUI-free, includes tests) 1,345; UI 1,005; main 38. 16 unit tests |
| Dynamic deps (macOS) | system frameworks only |

## Licences (`cargo metadata`, per-platform resolve graphs, mac/win/linux)

229 crates (macOS), 209 (Windows), 314 (Linux). Almost all are MIT and/or Apache-2.0. Others: Zlib, BSD-2/3, ISC, 0BSD, Unlicense, BSL-1.0 (`clipboard-win`, `error-code`, Windows only) and Unicode-3.0 (ICU4X crates). These are all permissive. Flagged:
- **`serialport` 4.10.1: MPL-2.0** (expected, file-level copyleft). Fine if it is not modified and the source is pointed to. NFR-07 needs to say this explicitly.
- **`epaint_default_fonts`: `(MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0`**. The bundled fonts need their licence texts shipped with the app, or you replace the fonts.
- Dual licences where we pick the permissive side: `self_cell` (Apache-2.0 OR GPL-2.0), `unescaper` (MIT OR GPL-3.0, build-time).

No LGPL or GPL-only dependency.

## Pain points a Rust learner would hit

1. **API churn beats docs and AI memory.** egui 0.37 replaced `App::update(ctx)` with `App::ui(ui)` and `TopBottomPanel` with `Panel::top(..).show(ui)`. rodio 0.23 replaced `OutputStream`/`Sink` with `DeviceSinkBuilder`/`MixerDeviceSink` and `NonZero` sample rates. sysinfo 0.40 made `Disks::new_with_refreshed_list()` return a `Result`. Most tutorials and examples you find online show older APIs. Reading the crate source in `~/.cargo/registry` was the reliable fix.
2. **Borrow checker in immediate-mode closures.** You cannot change `self` from inside a `Grid`/`show` closure that already borrows it. The usual pattern is to collect an action ("open this port") and apply it after the closure. It is easy once you know it, confusing before.
3. **Threads and ownership.** The worker needs `Send` boxed closures, a cloned `egui::Context` and channels. Calling a module `core` clashes with the built-in `core` crate.
4. **Build footprint.** 2.2 GB `target/`, about 30 s clean build on a fast M-series Mac (expect minutes on older laptops). Incremental builds are fine.
5. **Not Rust-specific but bitten here.** Linux needs system `-dev` packages. On macOS `fs::copy` copies xattrs, which fails on RPI-RP2. upload-artifact drops exec bits. The audio clock tells you what was rendered, not what was heard.

Things that were easier than expected: almost everything compiled on the first try once the APIs were known. Frame time is a non-issue. Unit-testing the core needed nothing special.

## Not tested

- Any real hardware: serial enumeration of the Zeros (none attached) and reading from the bridge. Flashing a real RPI-RP2 was not done, on purpose.
- Running the GUI on Windows or Linux (Linux was built and tested headless only, Windows type-checked only). That leaves HiDPI, Wayland vs X11, Windows COM port names and the Linux `dialout` group unchecked.
- Actually running the CI workflow on GitHub. Signing and notarisation (R5) are out of scope.
- What the window looks like, and how the UX feels: zoom, pan, button feel and audio quality.

## Try by hand

```sh
export PATH=/opt/homebrew/opt/rustup/bin:$PATH
cd spikes/app-egui && cargo run --release
cargo run --release -- --list        # headless: ports + boot volumes, opens nothing
SPIKE_AUTOPLAY=1 SPIKE_AUDIO=1 cargo run --release   # stress roll + audio, frame stats on stderr
```

1. Plug in a Zero and check it shows up yellow (RP2040) on tab 1. Optionally type its product string into the filter and check it turns green. "Open (read-only)" is safe in this spike, but you can leave it alone.
2. Tab 2: pan, zoom, click to seek, Play/Stop, sine vs square. Listen for clicks, and check the playhead matches what you hear. Load a real `.mid`.
3. Tab 4: "Create fake RPI-RP2 + dummy UF2", "Scan volumes", "Flash simulated drive". The progress bar should fill and the log should end with "flash OK".
4. Gamepad tab: hold a key and check that `repeats ignored` rises while `presses` stays at 1. Switch away from the window while holding a key and check the button releases.
5. Push a branch and check the 3 CI artefacts download and start (Windows SmartScreen and macOS Gatekeeper will warn, because they are unsigned).
