---
baseline_commit: 9f5a167
---

# Story 4.4: Serial worker, bridge discovery and file log

Status: review

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As the owner,
I want the app to find my bridge by itself and keep the link in a background worker,
so that the UI never freezes and I never pick a serial port (FR-08).

Origin: `epics.md` Epic 4, Story 4.4 (FR-08, AD-13, AD-16, AD-18, UX-DR65, the "Logging" convention of `ARCHITECTURE-SPINE.md`). Carries the Epic 3 retro actions E3-A1 (AC → measurement), E3-A2 (a red proof per verdict branch), E3-A3 (bench start checklist), E3-A5 (cross-OS discovery facts and the `SerialIo` lessons) and E3-A6 (M4 stays TODO until discovery is verified on all three OS). It builds on Stories 3.1 (bridge identity `2E8A:000A`, product `Bonooru Bridge`, serial = flash unique ID), 3.3 (`ocb-core`'s sans-IO `Session`), 3.4/3.5 (`ocb-dev`'s `SerialIo` and its bench lessons, `ocb-sim`) and 4.1–4.3 (`ocb-app`, smoke run, kittest conventions, CI `app.yml`).

**Scope: host code + a three-OS bench.** No firmware change. No LED drawing (Story 4.5), no version modal (4.6), no STOP (4.7), no gamepad (4.8). This story delivers the worker, the discovery rules, the rotating log, a small dev-only probe binary built by CI for the owner's Windows and Linux machines, and the bench on macOS, Windows and Linux.

## Acceptance Criteria

Numbers marked *(story)* are set by this story (the epic says only "about every 1 s"); each is measured as listed in the AC → measurement table.

1. **One worker owns the port and the session (AD-16).** A new crate `ocb-services` (`app/services/`, no egui/eframe dependency) holds the serial worker. Exactly one worker thread (named `bonooru-serial`) owns the `serialport` handle and the `ocb_core::link::Session`. The UI and the worker exchange only messages over `crossbeam-channel`: `WorkerCommand` in; `WorkerEvent` (discrete events) and `Arc<LinkSnapshot>` (immutable snapshots) out. They share no mutable state. After each publish the worker calls a waker the app passed in (the app passes `ctx.request_repaint()`). Nothing in `app/gui/src/` uses `serialport`, opens a file for the log, or calls a blocking receive (`recv()`, `recv_timeout`, `join` outside `on_exit`): the UI only `try_recv`s.
2. **The UI never waits on I/O.** With a fake backend whose port listing and open each block for 5 s, 10 consecutive app frames in a kittest harness finish in under 1 s of wall time *(story)*. On window close (`eframe::App::on_exit`) the app sends `Shutdown` and the worker thread ends within 500 ms *(story)*, closing the port (DTR falls).
3. **Discovery rules, pure (AD-13, AD-18).** `ocb_core::device` classifies every listed port, with no I/O, into exactly one of: `Ours` (USB `2E8A:000A` and product exactly `Bonooru Bridge`), `Unconfirmed` (USB `2E8A:000A` and no usable product string: always on Windows, elsewhere when the OS gives none), `Foreign { product }` (USB `2E8A:000A` with another product string) and `Ignored` (any other VID:PID, non-USB ports, and on macOS every name outside `/dev/cu.*`, so the `/dev/tty.*` twin is never seen). On Windows the `product` field `serialport` returns is the driver's friendly name (e.g. "USB Serial Device (COM5)", Dev Notes › Cross-OS discovery) and is never compared: a Windows bridge is `Unconfirmed` until INFO.
4. **Rescan and probe (AD-13).** The worker lists ports every 1000 ms *(story; epic: "about every 1 s")*, while idle and while connected. It opens and sends the HELLO probe only to `Ours` and `Unconfirmed` ports. Choice of port: `Ours` before `Unconfirmed`; among those, the serial number of the last connected bridge first, then the lowest port name. With more than one `Unconfirmed` port and no `Ours` (Windows with several Pico boards), each gets a probe window of 2 s *(story)* without INFO before the next is tried; a single candidate stays open. A port is opened at 460800 8N1, no flow control, DTR asserted, the OS input buffer cleared, and a fresh `Session` is opened on it (lone `00`, HELLO every 100 ms, AD-7).
5. **Foreign boards are never written to (AD-13, UX-DR11).** A `Foreign` port produces one `WorkerEvent::ForeignBoard { port, product }` per appearance and appears in the snapshot; it is never opened (so not even DTR is asserted on it). `Ignored` ports are never opened either.
6. **Port lifecycle carries the `SerialIo` lessons (E3-A5).** A listed candidate whose open fails with a transient error is retried every 250 ms *(story)* while it stays listed. Any read or write error other than timeout/would-block/interrupted drops the handle at once (the exclusive lock is released; on Linux a kept handle makes the kernel renumber the replugged port to `ttyACM1`) and emits `PortLost`; with the bridge unplugged, `PortLost` comes within 1500 ms *(story)* of the last byte received. When a port with the same serial number reappears (under the same or another name: macOS renames with the plug order, Linux may renumber), the worker reconnects to it and logs the rename; INFO arrives within 1500 ms *(story)* of the scan that first lists the port again (controller wired and powered). Writes never block: more than 4096 bytes waiting in the OS output queue, or 1 s without progress, drops the chunk and counts it (the session resends what matters).
7. **Linux serial permission (UX-DR65).** Opening a port that fails with permission denied emits one typed `WorkerEvent::PermissionDenied { port, group, message }` per appearance of that port (deduplicated; retried only at later scans, never in a loop) within one scan (≤ 1000 ms) of the port being listed. `message` is exactly: `Bonooru can't open <port>: your user is not in the <group> group. Run: sudo usermod -aG <group> $USER — then log out and back in.`, with `<group>` the group that owns the device node (`dialout` on Debian, Ubuntu and Fedora; `uucp` on Arch), `dialout` when it cannot be read. The app never escalates privileges: no `sudo`, `pkexec`, `doas`, setuid or polkit call anywhere in the code (source scan). A port held by another program gives `WorkerEvent::PortBusy { port }`, also deduplicated.
8. **Rotating file log (Logging convention, AD-18 `directories`).** The app writes `bonooru.log` in the OS log directory: macOS `~/Library/Logs/Bonooru/`, Windows `%LOCALAPPDATA%\Bonooru\logs\`, Linux `$XDG_STATE_HOME/bonooru/logs/` (default `~/.local/state/bonooru/logs/`). When a write would take the file past 1 MiB *(story)*, it rotates: `bonooru.2.log` → `bonooru.3.log`, `bonooru.1.log` → `bonooru.2.log`, `bonooru.log` → `bonooru.1.log` (at most 3 old files, the oldest deleted). One line per entry: UTC timestamp to the millisecond, ms since app start, a kind tag, the text. Logged: app start (app version, OS, arch, PID); every port change (listed, vanished, opened, closed, lost, renamed, foreign, permission denied, busy); every EVENT the controller sends (known and unknown codes, `PLAYBACK_DONE`); every NAK the app sees (semantic NAKs as `Refused`/`PieceAbandoned`, unknown reasons, and transport NAKs as the change in the session's rewind count); every version (INFO fields on connect, required and recommended update); a controller reboot. The worker never waits on the disk: entries go to a dedicated log thread over a bounded channel of 1024 entries *(story)*; when it is full the entry is dropped and counted, and the count is written as soon as there is room. If the log directory cannot be created or written, the app keeps running and emits one `WorkerEvent::LogUnavailable { reason }`.
9. **A dev-only probe for the bench (E3-A2, E3-A5).** A new crate `ocb-probe` (`app/tools/ocb-probe/`, never shipped) runs the same `ocb-services` worker and log code headless and prints machine-checkable lines: `ocb-probe list` (every port with VID, PID, serial, manufacturer, product and its class), `ocb-probe run` (connect, unplug, replug, log check, with the numbers of ACs 4, 6 and 8), `ocb-probe run --expect-permission-denied` (AC 7 on a real Linux port) and `ocb-probe alone` (bridge without controller). Each verdict branch has a dry run against `ocb-sim` and a red proof. CI builds it in release mode and uploads it as an artifact for Windows x64, Linux x64, Linux arm64 and macOS arm64, so the owner installs no toolchain on Windows or Linux.
10. **The real app runs the worker on three OS.** `bonooru` starts the worker at launch with the real backend and the real log; the smoke line (G.7) gains `worker=<scans>`, and the CI smoke check requires at least 1 scan on ubuntu, windows and macos. Tests never start the real backend or write to the real log directory.
11. **CI.** `serialport` `=4.10.1` with its default `libudev` feature in `ocb-services`; the Linux job installs `libudev-dev`; `cargo fmt`, `cargo test --locked --workspace`, `cargo clippy -D warnings`, `cargo deny --locked check licenses bans`, the eframe pin, G.8 and the smoke run stay green on ubuntu, windows and macos; a check proves `ocb-sim` is not in `ocb-app`'s normal dependency graph. Every new check has a red proof in the Debug Log.
12. **Three-OS bench, then M4 (E3-A6).** H.28 to H.32 (Dev Notes › H-rows) pass on macOS, Windows and Linux, run after the bench-tool review and the dry run (Tasks 11–12). Only when all of them pass on all three OS does `_bmad-output/specs/spec-bonooru/roadmap.md` set M4 to DONE, citing the H-rows. Until then M4 stays TODO. M5 stays TODO (it needs Stories 4.5–4.9).
    *Amended by owner decision 2026-11-27:* AC 12 passes on macOS (H.28, H.29, H.31, H.32); the Windows and Linux rows (H.28, H.29, H.31 on both, H.30 on Linux) are deferred because the owner has no Windows/Linux machine yet. They stay listed in this story and in `docs/test-plan.md`, marked "deferred: owner has no Windows/Linux machine yet (2026-11-27)", and are tracked as GitHub issue #43 (sprint action S4.4-A1). M4 stays TODO until they pass (condition written in `roadmap.md`). macOS bench: PASS 2026-11-27. The `probe-artifacts` CI job is still required, so the binaries are ready.

### AC → measurement (retro action E3-A1)

Every number in the ACs, the tool and the field that measure it. Host rows run in `cargo test`; bench rows are printed by `ocb-probe` on the owner's machine.

| AC | Number | Measured by | Field / result |
|---|---|---|---|
| 2 | 10 frames < 1 s while the backend blocks 5 s | `app/gui/tests/worker.rs` (G.13) `g13_ui_never_waits`: kittest harness with `App::with_link` on a `BlockingBackend` (list and open sleep 5 s) | `Instant` around 10 × `harness.run_steps(1)`: < 1 s |
| 2 | worker ends ≤ 500 ms after `Shutdown` | `ocb-services` `tests/worker_thread.rs` (C.23) `shutdown_joins_quickly`, real thread, fake backend with a port open | `Instant` from `send(Shutdown)` to `join()` returning: ≤ 500 ms; the fake port's `closed` flag set, its DTR line false |
| 4 | rescan every 1000 ms | C.23 `rescan_period` on the fake clock | the fake backend's `list` call times: consecutive differences exactly 1 000 000 µs (idle and connected) |
| 4 | rescan on the bench | `ocb-probe run` (H.29) | `scan_p50_ms` in 900–1100, `scan_max_ms` ≤ 1500 (worker `ScanDone { at_us, took_us }` events); `enum_max_ms` recorded, no verdict |
| 4 | 2 s probe window per `Unconfirmed` port | C.23 `probe_round_robin`: three `Unconfirmed` fakes, only the third answers | open times of port 1, 2, 3 are 2 000 000 µs apart (± one 1 ms loop tick); no writes to 1 and 2 after their window; connected on 3 |
| 4 | HELLO every 100 ms after `00` | C.23 `open_sequence` (fake port records writes) | first write a lone `00`, then HELLO frames 100 000 µs apart until INFO; DTR true before the first write; `clear(Input)` called before the first read |
| 5 | foreign / ignored never opened | C.23 `foreign_never_touched` | the fake backend's `open` count for the `Foreign` and `Ignored` ports is 0 for 30 s of fake time; one `ForeignBoard` event per appearance |
| 6 | open retry every 250 ms | C.23 `open_retry` (open fails 3 times with `NoDevice`, port still listed) | open attempt times 250 000 µs apart; connected at the 4th |
| 6 | `PortLost` ≤ 1500 ms after the last byte (host) | C.23 `unplug_detected` (read error at t; and a silent-port variant where the port only vanishes from the list) | read-error case: `PortLost` at t (same loop pass); vanish case: `PortLost` at the next scan, ≤ 1 000 000 µs after the vanish |
| 6 | `PortLost` ≤ 1500 ms after the last byte (bench) | `ocb-probe run` step "unplug" (H.29) | `lost_after_last_rx_ms` ≤ 1500 |
| 6 | INFO ≤ 1500 ms after the port is listed again (bench) | `ocb-probe run` step "replug" (H.29) | `info_after_listed_ms` ≤ 1500; `renamed` field printed (macOS replug into another USB socket) |
| 6 | reconnect by serial, rename | C.23 `rename_same_serial` | port `A` (serial S) vanishes, `B` (serial S) appears: connected on `B`, one `PortRenamed { from: A, to: B }` |
| 6 | write drop at 4096 B / 1 s | `ocb-services` serial adapter unit tests with a fake `SerialPort`-like trait object | `bytes_to_write` 4097 → chunk dropped, `dropped_chunks` + 1, no `write` call; a write making no progress → returns after 1 s of fake time with `dropped_chunks` + 1 |
| 7 | one event per appearance, within one scan | C.23 `permission_denied` (fake open → `Io(PermissionDenied)`) | exactly one `PermissionDenied` for 10 scans; emitted in the scan that lists the port; vanish + reappear → a second one |
| 7 | message text, group | `ocb-services` unit test `permission_message` | exact string for `/dev/ttyACM0` + `dialout`, + `uucp`, fallback `dialout` |
| 7 | real permission denial (bench) | `ocb-probe run --expect-permission-denied` (H.30) | `PASS` only if `PermissionDenied` arrived ≤ 1000 ms after the port was listed, the message is printed, and the port was never opened |
| 7 | no privilege escalation | `ocb-services` `tests/scan.rs` (C.25) source scan of `app/services/src`, `app/gui/src`, `app/tools/ocb-probe/src` | no `sudo`, `pkexec`, `doas`, `setuid`, `polkit`, `Command::new` outside comments; self-tested on good and bad snippets |
| 8 | rotate at 1 MiB, 3 old files | `ocb-services` `tests/log.rs` (C.22) on a temp dir | with the real limit: after writing 4.5 MiB of 200-byte lines, files `bonooru.log` and `.1`–`.3` exist, each ≤ 1 048 576 B, no `.4`; the newest line is last in `bonooru.log`; a line is never split across files |
| 8 | line format | C.22 `line_format` | regex `^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d\.\d{3}Z \+\d+ms [a-z_]+ .+$`; the UTC date of a known `SystemTime` (2026-11-27T10:15:02.123Z) formatted exactly |
| 8 | bounded 1024, drop + count | C.22 `full_channel_drops_and_counts` (log thread paused) | 1030 entries sent → 1024 written + one `log_dropped 6` line after resuming |
| 8 | every EVENT/NAK/version/port change logged | C.23 `everything_logged`: scripted fake controller (EVENT `0xEE`, `NAK NOT_READY` on SET_STATE, `NAK VERSION` mid-session, transport NAK via a corrupted frame, INFO with `fw_ver` ≠ app) + port vanish/reappear/rename | an in-memory log sink holds one entry per item, with the kinds listed in AC 8 |
| 8 | log on the real OS dir (bench) | `ocb-probe run` final step (H.29) | `LOG <path>` equals the AC 8 path for the OS; the run's PID-tagged lines include ≥ 1 `version`, ≥ 2 `port_opened`, ≥ 1 `port_lost` |
| 9 | each probe verdict branch | `app/tools/ocb-probe/tests/dry_run.rs` (C.24) | one test per branch (Dev Notes › Probe verdict branches) passing; red proofs in the Debug Log |
| 10 | `worker=` ≥ 1 on three OS | CI "Smoke run" (G.7), `gui/ci/check-smoke.sh` | the `OCB_SMOKE ok … worker=<n>` field with n ≥ 1 on ubuntu, windows, macos |
| 11 | `ocb-sim` not shipped | CI step | `cargo tree --locked -p ocb-app -e normal -i ocb-sim` fails with "nothing to print" / non-zero (no path) |
| 12 | three OS | H.28–H.32 rows in `docs/test-plan.md` | PASS lines and the numbers above per OS |

No AC number lacks a measurement. The Windows enumeration time (`enum_max_ms`) is recorded without a verdict: it is information for the owner, not an AC.

## Tasks / Subtasks

- [x] **Task 1: `ocb_core::device` — pure discovery rules (AC 3, 4).**
  - [x] 1.1 `app/core/src/device.rs` (+ `pub mod device;` in `lib.rs`): `PortInfo { name: String, usb: Option<UsbIds> }`, `UsbIds { vid: u16, pid: u16, serial: Option<String>, manufacturer: Option<String>, product: Option<String> }`, `Rules { product_reported: bool, callout_only: bool }` (Windows: `product_reported: false`; macOS: `callout_only: true`), `Class::{Ours, Unconfirmed, Foreign { product }, Ignored }`, `classify(&PortInfo, Rules) -> Class`, and `choose(candidates, last_serial) -> Option<…>` implementing the order of AC 4. Constants `BRIDGE_VID = 0x2E8A`, `BRIDGE_PID = 0x000A`, `BRIDGE_PRODUCT = "Bonooru Bridge"`. Product compared after `trim()`, case-sensitive. An empty product string counts as "not given".
  - [x] 1.2 Purity: no identifier `os`, `fs`, `io`, `env` etc. (C.4 bans the token `os`: name fields `host`, `platform`, never `os`). No `cfg(target_os)` in core: the caller passes `Rules`.
  - [x] 1.3 Tests `app/core/tests/device.rs` (C.21): every class with Windows/macOS/Linux rules; `/dev/tty.usbmodem101` ignored and `/dev/cu.usbmodem101` ours on macOS; Windows `product = "USB Serial Device (COM5)"` → `Unconfirmed`; Linux `"Bonooru Bridge"` → `Ours`, `"2wiCC COM"` → `Foreign`, `None` → `Unconfirmed`; other VID:PID (`2E8A:0003` BOOTSEL, `057E:2009`) → `Ignored`; choice order (ours > unconfirmed, last serial, lowest name; `COM10` vs `COM9` ordered by the number, not as text).
- [x] **Task 2: crate `ocb-services`, dependencies, CI packages (AC 1, 11).**
  - [x] 2.1 `app/services/Cargo.toml` (package `ocb-services`, lib): `ocb-core` (path), `serialport = "=4.10.1"` (default features, i.e. `libudev`), `crossbeam-channel = "=0.5.17"`, `directories = "=6.0.0"`; optional `ocb-sim` (path) behind feature `sim` (the fake backend for tests and dry runs, never enabled by a shipped build). Workspace lints. Add `"services"` and `"tools/ocb-probe"` to `[workspace] members`.
  - [x] 2.2 `.github/workflows/app.yml` "Linux system dependencies": add `libudev-dev` and update the comment ("No Wayland or ALSA: ALSA returns with Story 5.11"). `cargo deny` must stay green: check the new licences (`directories` → `dirs-sys` → `option-ext` is MPL-2.0, already allowed; `libudev`/`libudev-sys` MIT; `unescaper`). An unlisted licence is an owner decision (NFR-07): stop and ask, do not add it.
  - [x] 2.3 Note: Cargo unifies features across the workspace, so `ocb-dev`'s `serialport` (declared with `default-features = false`) also gets `libudev` in a workspace build. Harmless; update the comment in `app/tools/ocb-dev/Cargo.toml` to say so. Do not otherwise change `ocb-dev`.
- [x] **Task 3: the port adapter (AC 4, 6, 7).** `app/services/src/port.rs`.
  - [x] 3.1 Traits so the worker is testable without hardware: `Backend { fn list(&mut self) -> Result<Vec<PortInfo>, String>; fn open(&mut self, name: &str) -> Result<Box<dyn Port>, OpenError>; }` and `Port { read(&mut self, buf) -> Result<usize, PortError>, write_chunk, bytes_to_write, set_dtr, clear_input, … }`. `OpenError::{PermissionDenied, Busy, Gone, Other(String)}`.
  - [x] 3.2 `SerialBackend` (real): `serialport::available_ports()` mapped to `PortInfo` (USB fields copied; on Windows `product` set to `None` at the adapter, with a comment citing Dev Notes › Cross-OS discovery; `manufacturer` kept for the log only). Open: `serialport::new(name, 460_800)`, 8N1, `FlowControl::None`, `timeout(1 ms)`, then `write_data_terminal_ready(true)`, then `clear(ClearBuffer::Input)` (a stale STATUS from a controller since unplugged is not this session's, Story 3.5 H.3). Error mapping: `ErrorKind::Io(PermissionDenied)` → `PermissionDenied` (Linux EACCES); `ErrorKind::NoDevice` → `Busy` if the port is still in the last listing, else `Gone` (Linux/macOS EBUSY from `TIOCEXCL` and Windows `ERROR_ACCESS_DENIED` both map to `NoDevice`); anything else `Other`.
  - [x] 3.3 Writes, ported from `app/tools/ocb-dev/src/serial.rs` (read it first): skip and count the chunk when `bytes_to_write() > 4096`; loop `write` with a 1 s no-progress deadline, then drop and count; any non-timeout error → the port is closed (`Option<Box<dyn SerialPort>>` set to `None`) and `PortError::Lost`. Reads: one read of at most the 1 ms timeout; `TimedOut | WouldBlock | Interrupted` → 0 bytes.
  - [x] 3.4 Linux group of the node: `std::os::unix::fs::MetadataExt::gid()` on the device path, name looked up in `/etc/group` (plain parse, no libc crate); `cfg(target_os = "linux")` only. Message builder `permission_message(port, group)` (AC 7 exact text) as a pure function.
  - [x] 3.5 `FakeBackend` (feature `sim`, `app/services/src/sim.rs`): scripted ports appearing/vanishing at fake times, open results per attempt, each open port backed by an `ocb_sim::FakeController` through `ocb_sim::Channel`s (as `app/tools/ocb-dev/tests/common/mod.rs` `SimIo` does); records every open, write, DTR change and `clear_input` with the fake time. A `BlockingBackend` (list/open sleep a given duration, real time) for G.13.
- [x] **Task 4: the rotating log (AC 8).** `app/services/src/log.rs`.
  - [x] 4.1 `log_dir()`: macOS `BaseDirs::home_dir()/Library/Logs/Bonooru`; Windows `ProjectDirs::from("", "", "Bonooru")` → `data_local_dir().parent()/logs` (`data_local_dir` is `%LOCALAPPDATA%\Bonooru\data`); Linux `ProjectDirs::state_dir()` (`$XDG_STATE_HOME/bonooru`, or `~/.local/state/bonooru`) + `/logs`. `None` (no home) → `LogUnavailable`.
  - [x] 4.2 `RotatingFile { dir, max_bytes: 1 MiB, keep: 3 }`: append mode; size checked before each line; rotation by renames as in AC 8 (on Windows `rename` onto an existing file fails: remove the target first). A line is never split.
  - [x] 4.3 Line format of AC 8; UTC from `SystemTime::now()` with a hand-written days-to-civil conversion (no `chrono`/`time` crate); ms since start from an `Instant` taken at `Logger::start`.
  - [x] 4.4 `Logger` = a thread (`bonooru-log`) reading a `crossbeam_channel::bounded(1024)`; `LogHandle::log(kind, text)` uses `try_send`, counts drops (`AtomicU64`), and the thread writes `log_dropped <n>` when it next has room. Flush after each line (the log is for diagnosing crashes). A `MemorySink` for tests. `Logger::start` writes the start line (app version from `ocb_core::version::APP_VERSION`, `std::env::consts::{OS, ARCH}`, PID).
- [x] **Task 5: the worker (AC 1, 4–8).** `app/services/src/worker.rs`.
  - [x] 5.1 `WorkerCore<B: Backend, C: Clock>`: a steppable state machine, `step(&mut self)` does one pass: drain commands (`try_recv`); every 1000 ms list and classify (`Rules` for the host from `cfg!`), emit port-change events; pick/open per AC 4 and 6; while open: read → `session.handle_rx`, `handle_timeout` only when `poll_timeout` is due, drain `poll_transmit` → `write_chunk`, drain `poll_event` → map to `WorkerEvent` + log entries; publish a snapshot when anything changed. `Clock::now_us()`; the real clock is an `Instant` since start; the fake clock moves only when the test says so.
  - [x] 5.2 `LinkSnapshot` (immutable, `Clone`, behind `Arc`): ports seen with their class; the chosen port (name, serial, class, open or not); `Phase`; `Option<Info>`; last `StatusView` with its receive time (µs on the worker clock, plus the worker clock's "now" at publish, so the UI can judge the 150 ms staleness of Story 4.5 without its own clock reads mattering); counters (frames written, dropped chunks, rewinds, log drops); last error (`PermissionDenied`/`Busy`/`LogUnavailable`). Keep it data only; Story 4.5 maps it to LEDs.
  - [x] 5.3 `WorkerEvent`: `ScanDone { at_us, took_us }`, `PortListed`, `PortVanished`, `PortOpened`, `PortClosed`, `PortLost`, `PortRenamed`, `ForeignBoard`, `PermissionDenied`, `PortBusy`, `Link(LinkEvent)`, `LogUnavailable`. Events channel bounded(1024), `try_send`, drops counted in the snapshot. Snapshot channel bounded(1): the worker keeps a clone of the receiver and pops the stale snapshot before sending the new one (crossbeam channels are MPMC), so the UI always gets the latest and memory never grows while the window is hidden.
  - [x] 5.4 `WorkerCommand::Shutdown` (close the port, end the thread). Later stories add `SetState`, `Stop`, the 1200-baud touch and flashing checks (AD-16); keep the enum open for them but do not add them now.
  - [x] 5.5 `spawn(backend, log, waker: Box<dyn Fn() + Send>) -> WorkerHandle { commands: Sender, events: Receiver, snapshots: Receiver, join }`; the thread loop is `loop { core.step(); if shutdown { break } }` with the 1 ms read timeout as the only pacing when a port is open, and a 10 ms sleep per pass when none is (no busy loop while idle).
- [x] **Task 6: app integration (AC 1, 2, 10).**
  - [x] 6.1 `app/gui/Cargo.toml`: depend on `ocb-services`; dev-dependency `ocb-services = { path = "../services", features = ["sim"] }`.
  - [x] 6.2 `App::new` spawns the real worker (`SerialBackend`, real `Logger`) with `ctx.request_repaint()` as the waker; new `App::with_link(cc, mode, WorkerHandle)` for tests. `App` keeps the latest `Arc<LinkSnapshot>` and drains events with `try_recv` once per frame (Story 4.5 draws them; this story only stores them; keep the last 64 events for 4.5/4.6). `on_exit`: send `Shutdown`, join with a 500 ms bound (spawn-and-wait on a helper or `JoinHandle::is_finished` polling; never hang the exit).
  - [x] 6.3 Tests must never use the real backend or the real log dir: change `tests/window.rs` (it calls `App::new` today) to `App::with_link` with a `FakeBackend`; add a source scan in G.13 that fails if any file in `app/gui/tests/` calls `App::new(` or `SerialBackend`.
  - [x] 6.4 Smoke (G.7): `Smoke` waits for the first worker snapshot (bounded by the existing 60 s watchdog) before finishing, and adds `worker=<scans>` to the line; `gui/ci/check-smoke.sh` requires `worker=` ≥ 1; update the `smoke` unit tests and the line format doc.
- [x] **Task 7: `ocb-probe` (AC 9).** `app/tools/ocb-probe/` (package `ocb-probe`, bin), depends on `ocb-services` and `ocb-core`; dev-dependency `ocb-services` with `sim`, `ocb-sim`.
  - [x] 7.1 CLI (hand-written, like `ocb-app`'s `cli.rs`): `list`, `run [--step-timeout-s N]` (default 120), `run --expect-permission-denied`, `alone [--seconds N]` (default 10), `--help`. Exit codes: 0 PASS (or `list` ok), 1 FAIL, 2 usage, 130 interrupted (Ctrl+C via the `ctrlc` crate `=3.5.3` already in the lock file).
  - [x] 7.2 A runner generic over a `Driver` (production: the real worker thread and its channels with `recv_timeout`; dry run: `WorkerCore` stepped on a fake clock with `FakeBackend`), so the bench and the dry run share every verdict line (Story 3.4 D9 pattern).
  - [x] 7.3 Output lines (exact formats in Dev Notes › Probe output) and the verdict branches of Dev Notes › Probe verdict branches. Prompts are plain lines starting `PROMPT:`; the probe reacts to worker events, so the owner never presses Enter.
  - [x] 7.4 Dry runs `tests/dry_run.rs` (C.24): one test per verdict branch, each also asserting the exact `OCB_PROBE` line and exit code. Red proof per branch in the Debug Log (break the code path, show the test fail for the stated reason, restore).
- [x] **Task 8: CI (AC 9, 10, 11).**
  - [x] 8.1 `app.yml`: `libudev-dev` (Task 2.2); a step "ocb-sim never ships": `cargo tree --locked -p ocb-app -e normal -i ocb-sim` must fail to find it (and `-p ocb-services -e normal` without `--features sim` likewise); red proof: add `sim` to the normal dependency and see the step fail.
  - [x] 8.2 New job `probe-artifacts` in `app.yml` (needs nothing from `app`, runs in parallel), matrix `ubuntu-22.04` (x64; glibc 2.35, so the binary runs on older distros than a 24.04 build), `ubuntu-22.04-arm` (arm64; the repo is public, so the runner is free), `windows-latest` (x64; also runs under emulation on Windows 11 on Arm), `macos-latest` (arm64): toolchain from `rust-toolchain.toml` (same rustc 1.100.0 guard), `libudev-dev pkg-config` on Linux, `cargo build --locked --release -p ocb-probe`, `ocb-probe --help` as a run check, Linux `ldd target/release/ocb-probe` into the job summary (expect only `libudev.so.1`, libc, libm, libgcc_s), then package (Linux/macOS: `tar czf` to keep the exec bit, which upload-artifact's zip drops; Windows: the `.exe`) and `actions/upload-artifact@v7`, names `ocb-probe-linux-x64`, `ocb-probe-linux-arm64`, `ocb-probe-windows-x64`, `ocb-probe-macos-arm64`, `retention-days: 14`, `if-no-files-found: error` (pattern of `spike-app-egui.yml`).
  - [x] 8.3 Header comment of `app.yml` updated (C rows to C.25, G.13, the new job).
- [x] **Task 9: docs.**
  - [x] 9.1 `docs/test-plan.md` › App core tests: rows C.21 (device rules), C.22 (log), C.23 (worker on the fake clock and the thread tests), C.24 (`ocb-probe` dry runs), C.25 (source scans); › App GUI tests: G.13 (UI never waits, test isolation, `on_exit` join) and G.7 amended (`worker=`); › Hardware-in-the-loop: H.28–H.32 (Dev Notes › H-rows), Result column empty; a new rig block "Discovery bench rig (Story 4.4)" with the bench start checklist (Dev Notes › Rig block) and one record table per OS; › Test environment: rows for the Windows and Linux hosts (filled at the bench).
  - [x] 9.2 `app/README.md`: `services/` (`ocb-services`) and `tools/ocb-probe/` in the layout; how to run the probe; the log location per OS; Linux build needs `libudev-dev` and `pkg-config`.
  - [x] 9.3 `docs/link-protocol.md`: no change (no wire change). `ARCHITECTURE-SPINE.md`: no change unless a rule turns out wrong on the bench (then note it in the Debug Log and ask the owner).
- [x] **Task 10: verify on the host (AC 11).** In `app/` with `/opt/homebrew/opt/rustup/bin` first on `PATH`: `cargo fmt --all --check`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, `cargo test --locked --workspace`, `cargo deny --locked check licenses bans`, the G.8 scripts, `cargo run -p ocb-app -- --smoke-test` (with no bridge plugged, `worker=` ≥ 1), `cargo run -p ocb-probe -- list`. Red proofs recorded. Push the branch; the `app` workflow green on three OS; download each `ocb-probe-*` artifact once and run `--help` on the Mac for the macOS one.
- [x] **Task 11: bench-tool review (retro action #27, E3-A2), written in the Debug Log before any bench result.** For `ocb-probe` (new): every verdict branch listed with its dry-run test and red proof; every prompt read as the owner will read it; the expected lines per OS written down; the bench start checklist (Dev Notes › Rig block) checked against the plan; the lessons of Stories 3.1–3.5 applied (Dev Notes › Previous story intelligence). Also re-read H.28–H.32 against the tool's actual output format.
- [x] **Task 12: dry run against `ocb-sim` before each bench session.** `cargo test -p ocb-probe` (C.24) green on the commit that the bench uses, plus one manual `ocb-probe run` dry invocation through the dry-run driver (`cargo test -p ocb-probe -- --nocapture run_pass` to read the exact lines the owner will see). Record the commit in the rig block.
- [x] **Task 13 (BENCH, macOS): H.28, H.29, H.31, H.32.** Agent runs the tools on the Mac; the owner does the physical actions. Runbook: Dev Notes › Owner runbook › macOS.
- [ ] **Task 14 (BENCH, Windows): H.28, H.29, H.31.** *(deferred: owner has no Windows/Linux machine yet (2026-11-27); GitHub issue #43, sprint action S4.4-A1)* The owner runs the CI artifact; runbook: Dev Notes › Owner runbook › Windows. The agent reads the pasted output and the log file and fills the rows.
- [ ] **Task 15 (BENCH, Linux): H.28, H.30, H.29, H.31** *(deferred: owner has no Windows/Linux machine yet (2026-11-27); GitHub issue #43, sprint action S4.4-A1)* (in that order: the permission row first, while the user may not yet be in the group). Runbook: Dev Notes › Owner runbook › Linux.
- [x] **Task 16: results and M4 (AC 12, E3-A6).** Fill the H-rows, the rig block tables and the test environment rows with the numbers. If, and only if, H.28–H.32 pass on all three OS: set M4 to DONE in `_bmad-output/specs/spec-bonooru/roadmap.md` with one sentence citing H.26 (soak, Story 3.4) and H.28–H.32 (discovery on three OS, this story). If any OS fails: M4 stays TODO, the failure is a finding in this story, and the owner decides. Record, for Story 4.5, what each OS showed for the bridge alone (H.31) and for the product string (H.28): the inputs to OQ-5 and OQ-6, which Story 4.5 decides (not this story).

### Review Findings

*All findings below fixed on 2026-11-27 (review-fix commit after `c946377`); the decision item resolved by the owner: `alone` judges the session's HELLO cadence and reports drops as `hello_dropped=`. Red proofs and details: Debug Log › Review fixes.*

Code review 2026-11-27 (gds-code-review, diff `main...HEAD`, layers: Blind Hunter, Edge Case Hunter, Acceptance Auditor). Benches (Tasks 13–16) and the Windows/Linux deferral are out of scope. "Pre-bench" = fix before the macOS bench (H.28, H.29, H.31, H.32).

- [x] [Review][Decision] `alone` measures HELLOs the OS accepted, not HELLOs sent at the cadence — severity low/medium. `snap.counters.hello_frames` counts only chunks `write_chunk` accepted, and AC 6's drop policy deliberately drops chunks while the bridge does not read USB (no controller link). So `alone` and the drop policy work against each other: on a longer run (`--seconds 60`), or wherever the OS queue fills, the count stops and the verdict is FAIL even though the session sends HELLO every 100 ms. The 10 s default probably stays below the 4096 B limit. On Windows/macOS, where the queue figure may never pass 4096, the blocked writes of the next finding slow the cadence instead. Choose: count `poll_transmit` HELLOs (cadence) plus dropped chunks separately, or keep "accepted" and fix `--seconds` at 10. [app/tools/ocb-probe/src/runner.rs:613-624, app/services/src/worker.rs:903-916]
- [x] [Review][Patch] **Pre-bench.** `run` connect step spins without reading events once the busy deadline passes while a port is open (HIGH) — `until = min(deadline, open+5 s, busy+5 s)`, `busy` is never cleared, and the busy FAIL needs `opened.is_none()`. Once `now >= busy+5 s` with a port open, `wait()` returns `Timeout` from its `now >= deadline_us` guard without calling `d.next()`. It hot-loops at 100 % CPU, the queued `Connected` is never read, and at open+5 s it prints a false `FAIL (no INFO within 5 s of opening …)`. In the dry-run driver (clock moves only inside `next`) the same path never ends. Scenario: a terminal/monitor still closing the bridge gives `PortBusy` at 0.25 s, the open succeeds at 4 s, INFO comes at 4.1 s, verdict FAIL. No C.24 test covers busy-then-open. [app/tools/ocb-probe/src/runner.rs:280-315, runner.rs:210-219]
- [x] [Review][Patch] **Pre-bench.** `run` unplug/replug steps can PASS without a physical unplug or relisting (medium; verdict measures the wrong event) — unplug accepts any `PortLost` on the port, including a transient read error before the owner touches the cable. Replug accepts the next `Connected` without requiring `PortVanished` and then `PortListed`, and does not check the serial of the reconnected port. The base falls back to `opened_at` or even `e.at_us` (0 ms by construction), and the final line still says `info_after_listed_ms` with no marker of the fallback. Scenario: one read glitch, then an immediate reopen (see the `lose()` finding): both steps PASS, and H.29 records a number that is not the AC 6 "after the scan that lists the port again". Require vanish+listed (or print `base=listed|opened` and FAIL when it is not `listed`). [app/tools/ocb-probe/src/runner.rs:367-464]
- [x] [Review][Patch] **Pre-bench.** `write_chunk` can block the worker for 1 s per chunk, which breaks the AC 2 500 ms shutdown and the scan period (medium) — the chunk is dropped at once only when `bytes_to_write() > 4096`; an `Err` from `bytes_to_write` skips the check. Otherwise 1 ms writes repeat until 1 s passes with no progress, and the `poll_transmit` loop can do this for several chunks in one pass. Shutdown is checked only at the top of `step()`. Linux cdc-acm reports at most about 1 KiB queued and Windows usbser `cbOutQue` is not a real queue, so the 4096 test may never trip while the device NAKs. Scenario: bridge without a controller link (H.31/H.32 with the controller unpowered) means one HELLO blocks about 1 s. `on_exit`'s 500 ms join returns false, the logger is not stopped (queued lines lost), scans slip (`scan_max_ms > 1500`), and snapshots arrive about once per second. Bound total write time per pass (e.g. a few ms) and keep shutdown responsive. [app/services/src/port.rs:212-241, app/services/src/worker.rs:903-926]
- [x] [Review][Patch] **Pre-bench (recommended).** `lose()` retries the lost port with no backoff, so an open/fail loop floods the log and events (medium) — `set_retry(&c.name, now)`, whereas a failed `start()` uses `OPEN_RETRY_US`. If a node stays listed and opens but every read fails (Linux cdc-acm while disconnecting before udev removes the node; a Windows device in an error state), each 10 ms pass does open → DTR up → read error → `PortLost`. That is about 100 cycles/s with 2 log lines and 2 events each and DTR toggling: it rotates away 4 MiB of history in minutes and fills the 1024 event queue. Use `now + OPEN_RETRY_US`. [app/services/src/worker.rs:864-878]
- [x] [Review][Patch] Windows: a port gone since the last listing is reported as `PortBusy` (medium, Windows deferred) — serialport 4.10.1 `windows/error.rs:17` maps `ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND | ERROR_ACCESS_DENIED` all to `NoDevice`. `map_open_error` turns `NoDevice` + still listed into `Busy`. After an unplug, the immediate reopen (previous finding) fails with FILE_NOT_FOUND before the next scan, giving a spurious `PortBusy`, `last_error = Busy` ("held by another program"), a `port_busy` log line, and in the probe the start of the 5 s busy timer. Map using the raw OS error (only ACCESS_DENIED means busy). On POSIX the mapping is right: EBUSY (TIOCEXCL) and flock EWOULDBLOCK are `NoDevice`, and ENOENT is `Io(NotFound)`. [app/services/src/port.rs:397-407]
- [x] [Review][Patch] A failed rotation deletes the old logs one line at a time and drops every later line (medium) — `rotate()` removes `.3` and shifts `.2→.3` and `.1→.2` before renaming `bonooru.log→.1`. If that rename or the reopen fails, `write_line` returns before writing and `size` stays above the limit, so every later line rotates again. Scenario: the user deletes `bonooru.log` while the app runs, or on Windows a viewer holds it without FILE_SHARE_DELETE. Within 3 lines `.1`–`.3` are gone, every later entry is lost, and this is reported once (`failed` is sticky). [app/services/src/log.rs:94-135]
- [x] [Review][Patch] The UI event queue fills with `ScanDone` and drops the newest discrete events (medium) — `ScanDone` is emitted every second forever. `emit` uses `try_send`, so on a full queue the newest event is lost, and the UI drains only in `logic()`, which eframe may not run while the window is minimized or occluded. After about 17 min hidden the 1024 queue is all `ScanDone`, and `PortLost`, `PermissionDenied` and `Link(Connected/RequiredUpdate)` are dropped. Visible, `EVENTS_KEPT = 64` lets `ScanDone` push a `PermissionDenied` out of the app's history in about a minute (Story 4.5/4.6 read it). Keep scan timing in the snapshot, or filter `ScanDone` out of the UI ring. [app/services/src/worker.rs:452-457, worker.rs:620-628, app/gui/src/app.rs:53-64]
- [x] [Review][Patch] Round robin closes a bridge that answered with `RequiredUpdate` every 2 s (medium, Windows/multi-Unconfirmed) — `window_over` tests `!c.connected`, and `connected` (and `last_serial`) are set only on `Connected`. An incompatible bridge (INFO received, phase Incompatible) is closed after `PROBE_WINDOW_US`; the worker rotates to the other Pico (HELLO and DTR on it), comes back, and logs "required update" every cycle. Story 4.6's required-update screen would flicker. [app/services/src/worker.rs:665-673, worker.rs:950-990]
- [x] [Review][Patch] The top-ranked candidate waiting to retry blocks every other candidate outside round robin (low) — `try_open` returns when `target()`'s port has a future `retry_at_us` and never tries the next one. `rotate_after` applies only with no `Ours` and more than one `Unconfirmed`. Scenario: two `Ours` bridges, or `Ours` + `Unconfirmed`, with the first held by another program or permission-denied: the free one is never opened. [app/services/src/worker.rs:643-663, worker.rs:676-686]
- [x] [Review][Patch] Busy and permission dedup flags never reset while the port stays listed (low) — `busy_reported`/`perm_reported` clear only when the port leaves the listing; a successful open does not reset them. A later hold or denial on the same never-vanished name (COM5/ttyACM0 after a fast replug) raises no event and no `last_error`. [app/services/src/worker.rs:559-566, worker.rs:785-833]
- [x] [Review][Patch] A dead or stalled worker is invisible to the UI (low) — `Link::drain` ignores `Disconnected`, nobody checks `WorkerHandle::is_finished`, and `LinkSnapshot.now_us` freezes with the worker. So after a panic in `step` (or a stall, see `write_chunk`) the last snapshot keeps showing Connected with a "fresh" STATUS relative to its own frozen clock (Story 4.5's 150 ms staleness rule cannot fire). [app/gui/src/app.rs:53-64, app/services/src/worker.rs:1036-1046, worker.rs:1145-1157]
- [x] [Review][Patch] DTR is never cleared explicitly on close, and the "DTR falls" test is tautological (low) — `OpenPort::close` only drops the handle. serialport 4.10.1 `Drop` is `close(fd)`/`CloseHandle` and never touches HUPCL, so DTR falls only through the OS default (HUPCL) or the usbser driver. C.23 `shutdown_joins_quickly`/`shutdown_closes` assert DTR false, but `FakeOpenPort::drop` pushes `dtr=false` itself, so AC 2's "(DTR falls)" is not checked for the real path. Call `set_dtr(false)` before dropping (best effort), and record it in the fake only when called. [app/services/src/port.rs:160-163, app/services/src/sim.rs:485-492, app/services/tests/worker_thread.rs:57]
- [x] [Review][Patch] The log thread stop is unbounded on exit, and lines are lost when the worker join times out (low) — `Link::shut_down` calls `Logger::stop()` (an unbounded `join`) on the UI thread after the bounded worker join, so a slow or hung log disk freezes window close, against AC 2's "never hang". When the worker join times out, the logger is not stopped and queued lines (`port_closed`) are lost; in the probe, `finish()` then reads an incomplete file, so the log check can FAIL falsely. [app/gui/src/app.rs:67-74, app/services/src/log.rs:420-425, app/tools/ocb-probe/src/real.rs:110-124]
- [x] [Review][Patch] The probe log check breaks across rotation or with a concurrent writer (low) — `lines_of_pid` reads only the current `bonooru.log` from this PID's `start` line on. The file accumulates across app runs, so if it crosses 1 MiB mid-run the `start` line is in `.1`, the list is empty, and the run gives a false `log check: version 0 < 1 …`. Lines of a concurrently running `bonooru` that land after the start line are counted too, and each process rotates with its own `size`. Filter by PID per line, or read `.1` as well. [app/tools/ocb-probe/src/real.rs:57-74, app/services/src/log.rs:122-135]
- [x] [Review][Patch] `alone` fails on Windows when more than one `2E8A:000A` port is present (low, Windows deferred) — with no INFO by design, round robin moves to the other Unconfirmed port after 2 s, giving `FAIL (wrote to another port: …)` even though that is the AC 4 behaviour. [app/tools/ocb-probe/src/runner.rs:597-618, app/services/src/worker.rs:633-641]
- [x] [Review][Patch] Checks that cannot fail (low; E3-A2) — (a) smoke `worker < 1` is unreachable: `due()` needs a snapshot, and the first snapshot always follows a scan (`scans += 1` even on a listing error). A worker that fails to spawn shows only as the watchdog timeout, never as `OCB_SMOKE fail (worker=0)`. (b) CI "Linked libraries (Linux)" prints `ldd` but asserts nothing (a `not found` or an extra library passes). (c) `worker_thread.rs` `h.snapshots.len() <= 1` is guaranteed by `bounded(1)` itself. (d) The C.25 scan is evaded by `use std::process::Command as C; C::new(...)` (only the literal `Command :: new` sequence and the bare tokens are banned; strings are skipped). [app/gui/src/smoke.rs:201-203, .github/workflows/app.yml:246-256, app/services/tests/worker_thread.rs:77, app/services/tests/scan.rs:147-161]

## Dev Notes

### Scope in one paragraph

Build the plumbing every later Epic 4 story uses: a GUI-free `ocb-services` crate with a steppable serial worker (one thread, owns the port and the `Session`), the real `serialport` adapter carrying every `SerialIo` lesson, the pure discovery rules in `ocb_core::device`, and a rotating log on its own thread. The app starts the worker and stores its snapshots; nothing is drawn yet. A small headless `ocb-probe` binary runs the same code on the bench, so the owner can verify discovery on Windows and Linux from a CI artifact without installing Rust.

### Decisions taken at story creation (change only with a reason in the Debug Log; the owner can overrule any)

- **D1. New crate `ocb-services` (no egui).** AD-16/AD-18 allow "adapters in `ocb-app` or a separate crate". A separate crate lets `ocb-probe` reuse the worker without linking egui/wgpu/winit (small artifact, no X11 libraries on Linux, fast CI build) and makes "no egui in services" checkable. Adapters and the worker live together in it (modules `port`, `log`, `worker`, `sim`).
- **D2. Discovery rules in `ocb_core::device`.** The architecture names a core `device` module (CAP-6). The rules are pure, so they get the purity gate and fast tests; the OS differences enter only as `Rules`.
- **D3. Windows product string ignored.** `serialport` 4.10.1 on Windows fills `product` with `SPDRP_FRIENDLYNAME` (`src/windows/enumerate.rs:417`), the driver's display name ("USB Serial Device (COM5)" with the inbox `usbser.sys`), not the USB iProduct. Comparing it would call our own bridge "foreign". AD-13 and `EXPERIENCE.md` ("not on Windows") already say the string is unavailable there; the adapter sets it to `None`.
- **D4. Never open a `Foreign` or `Ignored` port.** AD-13 says "never written to"; not opening at all also avoids asserting DTR on someone else's board.
- **D5. Reconnect by serial number, not by name.** The flash unique ID is the bridge's serial on all three OS (macOS IOKit, Linux `ID_SERIAL_SHORT`, Windows parent HWID). This replaces `ocb-dev`'s "the single `/dev/cu.usbmodem*`" heuristic, which fails with two USB serial devices.
- **D6. Linux group from the device node.** The AC text names `dialout`; Arch uses `uucp` (`EXPERIENCE.md` marks the group name "[ASSUMPTION] per distro"). The message uses the node's actual group, which is `dialout` on Debian, Ubuntu and Fedora, and falls back to `dialout`.
- **D7. Log rotation 1 MiB × (1 + 3) files, own writer.** About 4 MiB at most; no logging crate (`log`/`tracing` are in the registry but not needed for one file writer; fewer licences, no global logger state). Story 8 (Log view, Copy diagnostics) reads these files.
- **D8. Log dir per OS** as AC 8. `directories` 6.0.0 has no log directory: macOS uses the platform convention `~/Library/Logs/<App>` (Console.app shows it), Linux `state_dir` (XDG says logs belong in `$XDG_STATE_HOME`), Windows `%LOCALAPPDATA%\Bonooru\logs` (non-roaming).
- **D9. The probe is a separate binary, not a `bonooru` flag.** It needs no window, no GPU, no X11, so it runs on any Linux box and over SSH, and its artifact is a few MB. The real app's worker is still exercised on three OS by the smoke run (`worker=`) and on the Mac bench by H.32.
- **D10. Artifacts for x64 and arm64 Linux, x64 Windows, arm64 macOS,** built on `ubuntu-22.04` (glibc 2.35) rather than 24.04 (glibc 2.39), so the owner's distro is unlikely to be too old. See Open questions 1.
- **D11. Port choice when several bridges are plugged** (same order as AC 4): `Ours` before `Unconfirmed`; within a class the last-connected serial first, then the lowest port name (numeric for `COMn`). No UI for it (the main screen never asks, `EXPERIENCE.md` › Port pick list is wizard-only, Epic 8).
- **D12. `ocb-dev` is not refactored onto `ocb-services`.** Its bench evidence (H.24–H.27, A.3, H.3) is tied to its code; the lessons are ported, not shared. A later cleanup may merge them.

### Current state of the files this story touches (read before editing)

- `app/Cargo.toml`: workspace members `core`, `gui`, `sim`, `tools/ocb-dev`; lints deny `unwrap_used`, `expect_used`, `panic`, `indexing_slicing`; `unsafe_code = "forbid"`. Add `services`, `tools/ocb-probe`. Must preserve: the lints (test files opt out with a file-level `#![allow(...)]`, as today).
- `app/core/src/lib.rs`: `pub mod link; pub mod protocol; pub mod version;` (+ `version_parse`). Add `pub mod device;`. Must preserve: `Cargo.toml` `[dependencies]` stays empty (C.4 checks it); `tests/purity.rs` bans tokens `fs io net os thread spawn mpsc process Command File env Instant SystemTime stdin stdout stderr unsafe extern asm` and `std::` modules outside an allow-list.
- `app/core/src/link/`: `Session::{new, open, handle_rx, handle_timeout, poll_timeout, poll_transmit, poll_event, phase, info, status, send_stats, rx_stats}`; `LinkEvent` (`Connected(Info)`, `RequiredUpdate`, `RecommendedUpdate`, `ControllerRebooted`, `Status`, `ControllerEvent`, `PlaybackDone`, `PieceAbandoned`, `Refused`, `UnknownNak`, `Stopped`, `Malformed`); `SendStats { sent, resends, rewinds }` (rewinds = transport NAKs). Used unchanged. The adapter contract: pass µs on a monotonic clock, call `handle_timeout` only when `poll_timeout` ≤ now, drain `poll_transmit` until `None` after every input (module doc of `link/mod.rs`).
- `app/tools/ocb-dev/src/serial.rs` (`SerialIo`): the lessons to port: DTR after open, `clear(Input)` on open, the `Option` handle dropped on any error, the 4096-byte / 1 s write drop, reads of at most 1 ms. Its `single_usbmodem` rename heuristic is replaced by D5. `port_rule.rs` (macOS `cu.` only) is replaced by `Rules::callout_only`. Not modified (D12) except the Cargo comment of Task 2.3.
- `app/gui/src/app.rs`: `App { mode, smoke, modals }`, `App::new`, `App::with_smoke`, `eframe::App::{logic, ui, clear_color}`; `ui()` draws the screen, then `modals.show`, then smoke bookkeeping; the comment where Story 4.7 reads Esc must stay at the top of `ui()`. Add the worker handle, snapshot, events and `on_exit`. Must preserve: dark-only `logic`, the modal order, the smoke flow.
- `app/gui/src/main.rs`: `run_native` with `App::new` / `App::with_smoke`; exit codes 0/1/2/3/4; console subsystem on Windows (no `windows_subsystem` attribute, so stdout works). Keep it.
- `app/gui/src/smoke.rs`: `OCB_SMOKE ok frames=3 ppp=… inner=… min=… theme=dark backend=…`; `Smoke::due(completed_frames)`; watchdog 60 s. `gui/ci/check-smoke.sh` parses every field. Add `worker=`.
- `app/gui/tests/window.rs`: builds `App::new` in kittest. Must change (Task 6.3).
- `.github/workflows/app.yml`: matrix `ubuntu-24.04`, `windows-latest`, `macos-latest`; Linux deps without udev ("those return with Stories 4.4 and 5.11"); steps fmt, test, clippy, pin check, G.8 ×2, build, smoke, deny, summary. Add libudev-dev, the `ocb-sim` check, the `probe-artifacts` job.
- `app/deny.toml`: allow-list (MIT, Apache-2.0, BSD-2/3, ISC, Zlib, MPL-2.0, Unicode-3.0, OFL-1.1, Ubuntu-font-1.0, BSL-1.0); network crates banned. Adding a licence is an owner decision.
- `_bmad-output/specs/spec-bonooru/roadmap.md` line 11: M4 TODO with the H.26 soak note. Changed only by Task 16.

### Cross-OS discovery (E3-A5; checked in `~/.cargo/registry/src/*/serialport-4.10.1` and with `ctx7` `/serialport/serialport-rs/v4.10.1`)

| | macOS | Windows (10/11) | Linux (glibc, `libudev` feature) |
|---|---|---|---|
| Enumeration | IOKit `IOSerialBSDServiceValue`; each device listed twice: `IOCalloutDevice` `/dev/cu.*` and `IODialinDevice` `/dev/tty.*` (`posix/enumerate.rs:573`) | SetupAPI over the Ports class GUIDs + registry `PortName` | libudev, subsystem `tty`, devnodes with a parent device |
| Port name | `/dev/cu.usbmodem101`; the number follows the USB location, so it changes with the socket and the plug order (Story 3.1 H.1: `usbmodem101` / `usbmodem1101`) | `COM3`, `COM5`…; stays with the device instance (VID, PID, serial), so the same bridge usually keeps its number in any socket; `COM10`+ works because `serialport` prefixes `\\.\` | `/dev/ttyACM0` (CDC ACM); renumbered to `ttyACM1` if the old node is still held open at replug; stable alias `/dev/serial/by-id/usb-Bonooru_Bonooru_Bridge_<serial>-if00` (not listed by `serialport`) |
| VID / PID | IOKit `idVendor` / `idProduct` | parsed from the HWID `USB\VID_2E8A&PID_000A&MI_00`; the bridge is a composite device (CDC with an IAD), so `serialport` reads the **parent** HWID for the serial | udev `ID_VENDOR_ID` / `ID_MODEL_ID` |
| Serial | IOKit `USB Serial Number` = flash ID (`53032847189C2E9C` for the owner's bridge, Story 3.1 rig) | parent HWID tail = flash ID | `ID_SERIAL_SHORT` = flash ID |
| Product | `USB Product Name` = `Bonooru Bridge` | `SPDRP_FRIENDLYNAME` = driver display name, **not** the iProduct (D3) | `ID_MODEL_ENC` unescaped (`Bonooru\x20Bridge` → `Bonooru Bridge`), else `ID_MODEL` with `_` → space |
| Manufacturer | `Bonooru` | `SPDRP_MFG` (driver vendor, e.g. "Microsoft") | `ID_VENDOR_ENC` / `ID_VENDOR` |
| Driver | built-in CDC ACM (`AppleUSBACMData`) | inbox `usbser.sys` binds CDC ACM automatically (no INF on 10/11; Windows 7/8 out of scope) | `cdc_acm` |
| Open errors (`serialport` mapping) | `EBUSY` (another `TIOCEXCL` holder) → `NoDevice`; `EACCES` → `Io(PermissionDenied)` | `ERROR_ACCESS_DENIED` (port open in another program) and `ERROR_FILE_NOT_FOUND` → `NoDevice` (`windows/error.rs`) | as macOS; `EACCES` is the `dialout` case |
| Permissions | none needed | none needed | node `root:dialout 0660` (Debian, Ubuntu, Fedora; Arch `root:uucp`); the user must be in the group and log in again; never fix by escalation (UX-DR65) |
| Exclusive open | `TIOCEXCL` + `flock` by default | share mode 0 | `TIOCEXCL` + `flock` |

More facts the code relies on:

- **libudev feature (E3-A5).** `serialport`'s default `libudev` feature needs `pkg-config` and `libudev-dev` at build time (Ubuntu; Fedora `systemd-devel`) and `libudev.so.1` at run time (present on every systemd distro). Without it, Linux enumeration falls back to `/sys/class/tty` and "won't expose as much information and may return ports that don't exist physically" (README 4.10.1); that fallback also `expect`s `/sys/class/tty` to exist. With libudev, if `libudev::Context::new()` fails, `available_ports()` returns an empty list silently: the probe's `list` prints the count so an empty list on a machine with the bridge plugged is visible.
- **ModemManager.** On Ubuntu it probes new serial devices; with its default "strict" filter it skips CDC ACM interfaces whose protocol is not AT commands. The bridge's CDC uses `CDC_COMM_PROTOCOL_NONE` (TinyUSB `TUD_CDC_DESCRIPTOR`), so it should be left alone. If H.29 on Linux shows a `PortBusy` right after plug-in, ModemManager is the first suspect (record it; the known fix is a udev rule `ENV{ID_MM_DEVICE_IGNORE}="1"`, documented, never installed by the app).
- **Windows read timing.** `COMMTIMEOUTS` with `ReadTotalTimeoutConstant = 1 ms`, but the Windows timer granularity is about 15.6 ms by default, so one read may wait that long. The loop then runs at about 60 Hz on Windows: fine for HELLO (100 ms), PING (250 ms), ACK resend (50 ms) and STATUS (50 ms). The probe records `scan_*` and loop stats per OS.
- **Windows enumeration cost.** SetupAPI enumeration can take tens of ms; it runs once per second inside the worker loop. `ScanDone.took_us` makes it visible (`enum_max_ms` in H.29, no verdict).

### SerialIo lessons (E3-A5; Stories 3.1, 3.4, 3.5)

1. **Reopen after unplug, and drop the handle at once.** Story 3.5 review: a kept descriptor held the exclusive lock and blocked the reopen. On Linux a kept handle also renumbers the node. The worker sets the port to `None` on the first hard error.
2. **Clear stale input on open.** Story 3.5 H.3: the OS kept a STATUS from a controller since unplugged in the input buffer. `clear(ClearBuffer::Input)` after DTR, before the first read; the session is new, so nothing older belongs to it.
3. **Non-blocking writes, drop while unlinked.** While the bridge has no controller link it stops reading USB (Story 3.1), the OS output queue fills and a blocking write stalls the tool. Drop the chunk above 4096 queued bytes or after 1 s without progress; HELLO, STOP and go-back-N resend what matters.
4. **macOS renames the port with the plug order** (`usbmodem101` / `usbmodem1101`, Story 3.1 H.1; Story 3.5 `failsafe`). The worker follows the serial number (D5) and logs the rename.
5. **Only `/dev/cu.*` on macOS** (opening `/dev/tty.*` waits for carrier detect; Story 3.4 C.14/H.27).
6. **The node can be listed before it opens** (`ocb-dev` `reopen`: "a failed open is not yet"): retry every 250 ms while listed.

### Probe output (Task 7; exact formats, so H-rows and dry runs check the same text)

- `list`: one line per port, then a summary:
  `PORT name=<name> usb=<VID>:<PID> serial=<s|-> manufacturer="<m|->" product="<p|->" class=<ours|unconfirmed|foreign|ignored>` (non-USB: `usb=-`), then `OCB_PORTS n=<all> ours=<k> unconfirmed=<k> foreign=<k> host=<macos|windows|linux>`. Exit 0; an enumeration error prints `OCB_PORTS error <text>` and exits 1. On Windows the raw product (friendly name) is printed with `product_raw=` and `product="-"`, so H.28 records what Windows reports.
- `run`: event lines `EVT +<ms> <kind> <fields>` as the worker reports them; `PROMPT: UNPLUG the bridge's USB cable now.` / `PROMPT: PLUG the bridge back in (on a Mac, into another USB socket if you have one).`; one `STEP <name> PASS|FAIL <fields>` per step (`connect`, `unplug`, `replug`, `log`); final `OCB_PROBE PASS host=<h> port=<p> serial=<s> renamed=<yes|no> info_after_open_ms=<n> lost_after_last_rx_ms=<n> info_after_listed_ms=<n> scan_p50_ms=<n> scan_max_ms=<n> enum_max_ms=<n> fw=<x.y.z> bridge=<x.y.z> proto=<n>` or `OCB_PROBE FAIL (<reason>) <same fields so far>`; then `LOG <absolute path of bonooru.log>`.
- `run --expect-permission-denied`: `OCB_PROBE PASS permission port=<p> group=<g> after_listed_ms=<n>` + the message line `MSG <message>`; or `OCB_PROBE FAIL (<reason>)`.
- `alone`: `OCB_PROBE PASS alone port=<p>[,<p2>…] class=<ours|unconfirmed> hello_frames=<n> hello_dropped=<d> seconds=<s> other_ports_written=0` or `OCB_PROBE FAIL (<reason>)` (`hello_frames` = HELLOs the session handed out, its cadence; `hello_dropped` = those the write rules dropped; review decision 2026-11-27).
- Ctrl+C anywhere: `OCB_PROBE INTERRUPTED step=<name>`, exit 130.

### Probe verdict branches (E3-A2: one dry-run test and one red proof each)

`run`: PASS; FAIL `no bridge within <n> s` (no `Ours`/`Unconfirmed` port); FAIL `only foreign boards: <products>`; FAIL `permission denied on <port>`; FAIL `port busy: <port>`; FAIL `no INFO within 5 s of opening <port>` (controller not wired or not powered); FAIL `protocol mismatch: controller <n>, app <n>` (`RequiredUpdate`); FAIL `unplug not seen within <n> s`; FAIL `loss detected <n> ms after the last byte (limit 1500)`; FAIL `port lost but still listed after <n> s (no physical unplug)`; FAIL `reconnected without leaving the port list (no physical unplug)`; FAIL `reconnected to another bridge: <p> serial <s> (not the one unplugged, listed again)`; FAIL `replug not seen within <n> s`; FAIL `listed again but no INFO within <n> s`; FAIL `INFO <n> ms after the port was listed (limit 1500)`; FAIL `rescan period p50 <n> ms / max <n> ms (limits 900–1100 / 1500)`; FAIL `log check: <what is missing>`; INTERRUPTED.
`run --expect-permission-denied`: PASS; FAIL `port opened: the user can already open <port>` (the run cannot prove the message); FAIL `no permission event within <n> s`; FAIL `permission event <n> ms after the port was listed (limit 1000)` (added at dev-story: the AC 7 bound of the AC table).
`alone`: PASS (several unconfirmed ports probed in turn are all named); FAIL `INFO received: the controller is powered` ; FAIL `no bridge port`; FAIL `wrote to a port that is not a bridge: <p>`; FAIL `only <n> HELLO frames in <s> s (expected ≥ 9 per s)`.
`list`: ok (exit 0) and `error` (exit 1).

### H-rows (for `docs/test-plan.md` › Hardware-in-the-loop; Result empty until the bench)

- **H.28 Port report per OS (Story 4.4 AC 3).** Both boards plugged and wired (controller powered by the same PC or the console), `ocb-probe list`. Pass: the bridge appears exactly once with `usb=2E8A:000A` and `serial=53032847189C2E9C` (the owner's bridge); macOS: only its `/dev/cu.*` name, `product="Bonooru Bridge"`, `class=ours`; Linux: `/dev/ttyACM<n>`, `product="Bonooru Bridge"`, `class=ours`; Windows: `COM<n>`, `product="-"` with `product_raw=` recorded, `class=unconfirmed`. The controller (`057E:2009`) is not a serial port and is absent. Record every printed line.
- **H.29 Discovery, connect, unplug, replug, log (AC 4, 6, 8).** Same rig, `ocb-probe run`, follow the two prompts. Pass: `OCB_PROBE PASS` with `info_after_open_ms`, `lost_after_last_rx_ms` ≤ 1500, `info_after_listed_ms` ≤ 1500, scan p50 900–1100 / max ≤ 1500, `LOG` at the AC 8 path for the OS. macOS: replug into another socket and record `renamed`. Record all fields and `enum_max_ms`.
- **H.30 Linux permission message (AC 7).** Linux only, before the user is added to the group: `ocb-probe run --expect-permission-denied` (or, if the user is already in the group, through `setpriv` as in the runbook). Pass: `OCB_PROBE PASS permission` and the `MSG` line with the node's group. Then the user is added, logs in again, and H.28/H.29 run normally.
- **H.31 Bridge alone (input to OQ-5/OQ-6, decided in Story 4.5).** The controller unpowered (its USB unplugged), the bridge plugged, `ocb-probe alone`. Pass: `OCB_PROBE PASS alone` (the bridge port opened, ≥ 9 HELLO frames per second for 10 s, no INFO, no other port written). Record `class` (Windows `unconfirmed`, macOS/Linux `ours`).
- **H.32 The GUI app runs the worker and logs (AC 1, 8, 10).** macOS required; Windows/Linux optional. `app/target/release/bonooru` (Mac build), both boards plugged; after 5 s unplug the bridge, replug it, close the window. Pass: the window stayed responsive (it can be moved/resized during the unplug), the process exits within 1 s of closing, and the log at the AC 8 path has, after the app's `start` line, `port_opened`, `version`, `port_lost`, `port_opened` lines in that order. The agent checks the log.

### Rig block (for `docs/test-plan.md`, "Discovery bench rig (Story 4.4)"; E3-A3)

Rig: the own-bridge rig (Story 3.1 bridge firmware, wiring GP0/GP1/GND unchanged, not rechecked). The controller only needs power (from the same PC or the console) and the wires: INFO comes back without the Switch. No `ocb-dev` or other serial tool running at the same time (the port is exclusive: it would show as `PortBusy`).

**Bench start checklist (every session, every OS):**

1. If the controller is in the console: console auto-sleep off (System Settings › Sleep Mode); if OoT's connection screen returns after a Joy-Con hand-back, close it with A from our controller (`python3 firmware/tools/link_probe.py --bridge own --port "$P" press --button A`, Mac only; on Windows/Linux power the controller from the PC instead, so the console is not involved).
2. Boards plugged (controller powered and wired, bridge on the PC) **before** any tool starts.
3. To interrupt a tool, send SIGINT to the tool's own PID: `kill -INT "$(pgrep -x ocb-probe)"` (macOS/Linux); Windows: Ctrl+C in its own console. Never to a wrapper shell.
4. Record once per session: host OS and version, machine (real/VM, CPU arch), `ocb-probe` commit or CI run id, bridge firmware (Story 3.1, `69f4d2c` `fw-release` unless reflashed), controller firmware, cable, USB socket(s) used.

### Owner runbook (what the owner runs on each OS, and what to send back)

**macOS (Task 13).** The agent builds `cargo build --locked --release -p ocb-probe -p ocb-app` in `app/` and runs the tools; the owner plugs, unplugs and watches the window. Commands: `app/target/release/ocb-probe list` (H.28); `app/target/release/ocb-probe run` (H.29; replug into another socket at the second prompt); controller USB out, then `app/target/release/ocb-probe alone` (H.31); controller back, then `app/target/release/bonooru` (H.32; owner moves the window during the unplug, closes it at the end; agent reads `~/Library/Logs/Bonooru/bonooru.log`).

**Windows (Task 14).** No Rust needed.

1. Download: GitHub › basteez/bonooru › Actions › workflow "app" › the run for the story branch head (the agent gives the link) › Artifacts › `ocb-probe-windows-x64` (needs a GitHub login). Extract the zip to `C:\bonooru-probe`.
2. Open PowerShell: `cd C:\bonooru-probe`, then `Unblock-File .\ocb-probe.exe` (removes the downloaded-file mark, so SmartScreen does not stop it).
3. Plug the controller Zero into the PC (it shows up as a game controller; nothing to install) and the bridge into another socket. Wires as on the Mac.
4. `.\ocb-probe.exe list` → expect one `PORT name=COM<n> usb=2E8A:000A serial=53032847189C2E9C … class=unconfirmed` line and `OCB_PORTS … unconfirmed=1`.
5. `.\ocb-probe.exe run` → follow the two `PROMPT:` lines (unplug the bridge, plug it back) → last lines `OCB_PROBE PASS …` and `LOG C:\Users\<you>\AppData\Local\Bonooru\logs\bonooru.log`.
6. Unplug the controller's USB, then `.\ocb-probe.exe alone` → `OCB_PROBE PASS alone … class=unconfirmed`.
7. Send back: the whole PowerShell output (select all, copy) and the file `bonooru.log`; Windows version (`winver`).

Fallback only if the exe does not run: install Rust with `winget install Rustlang.Rustup` plus the "Desktop development with C++" workload of Visual Studio Build Tools, clone the repo, `cd app`, `cargo run --release -p ocb-probe -- list` (same commands after `--`).

**Linux (Task 15).** No Rust needed.

1. Download `ocb-probe-linux-x64` (or `ocb-probe-linux-arm64` on an ARM machine) from the same Actions run (browser), or `gh run download <run-id> -R basteez/bonooru -n ocb-probe-linux-x64`. Then `tar xzf ocb-probe-linux-x64.tar.gz` and `./ocb-probe --help` (expect the usage text; if it says `GLIBC_2.35 not found`, send that line).
2. Plug both boards. `ls -l /dev/ttyACM*` (expect `crw-rw---- 1 root dialout …`) and `id -nG` (is `dialout`, or the group shown by `ls`, listed?). Send both outputs.
3. H.30 first. If the group is **not** listed: `./ocb-probe run --expect-permission-denied` → `OCB_PROBE PASS permission …` and a `MSG Bonooru can't open /dev/ttyACM0: …` line. If it **is** listed: `sudo setpriv --reuid="$(id -u)" --regid="$(id -g)" --clear-groups ./ocb-probe run --expect-permission-denied` (runs the probe as you but without your extra groups; `sudo` only starts the test, the probe itself never asks for privileges).
4. If not yet in the group: `sudo usermod -aG dialout $USER` (or the group from step 2), log out and back in (or reboot), check `id -nG`.
5. `./ocb-probe list` → `class=ours`, `product="Bonooru Bridge"`. `./ocb-probe run` → follow the prompts → `OCB_PROBE PASS …`, `LOG /home/<you>/.local/state/bonooru/logs/bonooru.log`. Unplug the controller's USB, `./ocb-probe alone` → `OCB_PROBE PASS alone … class=ours`.
6. Send back: the whole terminal output, the log file, `cat /etc/os-release` (first lines), `uname -m`.

Fallback only if the binary does not run: `curl https://sh.rustup.rs -sSf | sh`, `sudo apt install build-essential pkg-config libudev-dev` (Fedora: `gcc pkgconf-pkg-config systemd-devel`), clone, `cd app`, `cargo run --release -p ocb-probe -- list`.

### Testing standards

- Host tests only touch fakes: `FakeBackend` on a fake clock for every timing number (deterministic, no sleeps); real threads only in `worker_thread.rs` (shutdown, waker called, snapshot channel never grows) and G.13, with generous wall-time bounds.
- Test files: file-level `#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]`, as today.
- A source scan (C.25) uses the token approach of `app/core/tests/purity.rs` (comments and strings skipped, self-tested on a good and a bad snippet).
- Red proofs (E3-A2, AC 11): for each new check and each probe verdict branch, the Debug Log names the code change that makes the test fail and the failure message seen. Minimum list: C.21 Windows product compared (bridge becomes foreign), C.23 rescan period 900 ms, open retry removed, handle kept after error, foreign port opened, permission event repeated each scan, C.22 rotation off by one (`.4` appears), drop counter not written, G.13 a blocking `recv()` in `App::ui`, the `App::new(` scan, the `ocb-sim` CI step, `check-smoke.sh` with `worker=0`, and every probe branch.
- `cargo test --locked --workspace` must stay free of hardware: a developer may have the bridge plugged while running it; no test may call `SerialBackend::open`.

### Architecture compliance

- AD-16: one worker, messages only, UI never blocks; connect/disconnect live in the worker (later the touch and post-flash checks join them).
- AD-13: VID:PID first, product as an extra filter where reported, HELLO only to VID:PID matches, foreign never written; no 1200-baud touch in this story (Epic 8, wizard, and only to a confirmed port).
- AD-18: `serialport` 4.10.1 (MPL-2.0 unmodified), `crossbeam-channel` 0.5.17, `directories` 6.0.0, exact pins; macOS `cu.*` only; Linux runtime dep `libudev.so.1`; `dialout` explained, never fixed by escalation.
- AD-7: session open = DTR asserted, lone `00`, HELLO every 100 ms (done by `Session`, the adapter asserts DTR and writes what it returns).
- AD-21: no network crate (the deny gate); the log is local.
- Hexagonal: `ocb-core` stays pure (device rules); adapters and the worker in `ocb-services`; `ocb-app` depends on services; nothing depends on `ui`.

### Library / framework requirements

- `serialport = "=4.10.1"` (default features: `libudev`). API: `available_ports() -> Result<Vec<SerialPortInfo>>`, `SerialPortType::UsbPort(UsbPortInfo { vid, pid, serial_number, manufacturer, product })`; `serialport::new(path, baud)…open()`; `SerialPort::{write_data_terminal_ready, clear(ClearBuffer::Input), bytes_to_write, read, write}`; `Error { kind: ErrorKind::{NoDevice, InvalidInput, Unknown, Io(io::ErrorKind)}, description }`.
- `crossbeam-channel = "=0.5.17"`: `bounded`, `try_send`/`TrySendError::Full`, `try_recv`, `recv_timeout`; channels are MPMC (the snapshot pop trick of Task 5.3).
- `directories = "=6.0.0"`: `ProjectDirs::from("", "", "Bonooru")` (qualifier and organization empty), `state_dir() -> Option<&Path>` (Linux only), `data_local_dir()`; `BaseDirs::new()?.home_dir()`. No log dir in the crate (D8).
- `ctrlc = "=3.5.3"` for the probe (same as `ocb-dev`).
- No `log`, `tracing`, `chrono`, `time`, `libc`, `nix` as direct dependencies.

### File structure

New: `app/core/src/device.rs`, `app/core/tests/device.rs`; `app/services/{Cargo.toml, src/lib.rs, src/port.rs, src/log.rs, src/worker.rs, src/sim.rs, tests/log.rs, tests/worker.rs, tests/worker_thread.rs, tests/scan.rs}`; `app/tools/ocb-probe/{Cargo.toml, src/main.rs, src/lib.rs, src/cli.rs, src/runner.rs, tests/dry_run.rs}`; `app/gui/tests/worker.rs`.
Changed: `app/Cargo.toml`, `app/Cargo.lock`, `app/core/src/lib.rs`, `app/gui/{Cargo.toml, src/app.rs, src/main.rs (only if needed), src/smoke.rs, ci/check-smoke.sh, tests/window.rs}`, `app/tools/ocb-dev/Cargo.toml` (comment), `.github/workflows/app.yml`, `docs/test-plan.md`, `app/README.md`, and at the end `_bmad-output/specs/spec-bonooru/roadmap.md` (Task 16, conditional).
Never: `_bmad/`, skill folders, firmware, `ocb-dev` code, game screenshots.

### Previous story intelligence (3.1–3.5, 4.1–4.3)

- 3.1: bridge identity `2E8A:000A` / `Bonooru Bridge` / manufacturer `Bonooru` / serial = flash ID (`53032847189C2E9C`); port renamed by plug order; the labelled cable for flashing.
- 3.3: the sans-IO contract (`handle_timeout` only when due, drain `poll_transmit` after every input); typed events; a new `Session` per connection.
- 3.4: the runner shared by bench and dry run (D9) caught bugs before the bench (retro action #27); SIGINT reached a wrapper shell, not the tool (use `pgrep -x`); a `SUMMARY` count fix found at the bench: print every number the H-row needs, from the wire, not from internal counters.
- 3.5: stale input in the OS buffer; a second SIGINT was needed to release the port (the shutdown path must close the port promptly: AC 2); the console needed A to close its connection screen; two AC numbers were found unmeasurable only at review (hence the table above).
- 4.1: crate `ocb-app`, the smoke run and its checker, tests on the CPU, `cargo-deny` licence decisions are the owner's.
- 4.3: review found tests that could not fail; every check here needs a red proof; source scans must be self-tested.
- Epic 3 retro: bench surprises live in tools and in the console's behaviour: everything known is in the checklist above.

### Git intelligence

Last commits: `9f5a167` (Story 4.3 done), PR #42 merged; Story 4.3's helpers (`ui::paint`, `ui::modal`, `ui::widgets`) are not needed here. Story 4.0 (`4-0-cleanup…`) extended the C rows to C.20. Commit style: one long descriptive sentence per commit, story number in parentheses.

### Project Structure Notes

- The structural seed lists `app/adapters/` and `app/services/` as folders; D1 puts both in one crate at `app/services/` (crate `ocb-services`). Variance noted; no other crate layout changes.
- Dev-only crates live under `app/tools/` (`ocb-dev`, now `ocb-probe`) and `app/sim/`; they are never bundled (release packaging, Epic 11, ships `bonooru` only).

### Project Context Rules

No `project-context.md` exists. Rules from memory and practice: all repo content in English; never commit `_bmad/` or skill folders; game screenshots never on GitHub; the test environment lives in `docs/test-plan.md` (read it, do not ask); one question at a time to the owner; do not recheck GP0/GP1/GND wiring each session unless rewired; research (upstream source, docs) before raising a question.

### Open questions for the owner

1. **Your Windows and Linux machines.** Real PCs or VMs, CPU (x64 or ARM), Windows version, Linux distro and version. The plan assumes x64 Windows 10/11 and an x64 or arm64 Linux with glibc ≥ 2.35 (Ubuntu 22.04+, Debian 12+, Fedora 36+); CI builds all four artifacts, so only an older distro or a musl distro (Alpine) would need a change. Needed before Task 14/15, not before dev-story. **Answer (owner, 2026-11-27):** the Windows and Linux benches run on separate physical x64 PCs, not VMs; the exact Windows version and Linux distro/version are collected at the bench from `winver` and `/etc/os-release`. Artifact targets unchanged (Windows x64, Linux x64 + arm64, macOS arm64; arm64 kept because the runner costs nothing extra on a public repo). Then (owner decision, same day): Windows and Linux are not available for now; their bench rows are deferred (see AC 12).
2. **Foreign board on hardware (optional).** *(Default kept: host tests only.)* The foreign-board path (AC 5) is covered by host tests only. If you have a spare RP2040 board you are willing to flash with 2wiCC_Comms v1.1 (it reports `2wiCC COM` on `2E8A:000A`), one extra `ocb-probe list` + `run` on the Mac would show it as `foreign` and never opened. Default: host tests only.

(OQ-5 "Windows, bridge alone" and OQ-6 "bridge label without INFO" belong to Story 4.5; this story only collects the data in H.28 and H.31.)

### References

- `_bmad-output/planning-artifacts/epics.md` › Epic 4 › Story 4.4; Requirements Inventory (FR-08, AD-13, AD-16, AD-18 "Discovery", "Logging", "Linux"); UX-DR11, UX-DR65; OQ-5, OQ-6.
- `_bmad-output/planning-artifacts/architecture/architecture-bonooru-2026-11-19/ARCHITECTURE-SPINE.md` › AD-13, AD-16, AD-18 platform notes, Conventions (Errors, Logging), Structural Seed; `app-stack-options.md` (serialport, crossbeam-channel, directories, Linux runtime deps).
- `_bmad-output/planning-artifacts/ux-designs/ux-bonooru-2026-11-20/EXPERIENCE.md` › State Patterns (Bridge not found, Foreign Pico, Windows no product string, Bridge only, Linux no serial permission), Port pick list, Log view, Open Questions 4–5.
- `_bmad-output/implementation-artifacts/epic-3-retro-2026-11-26.md` › 3 Challenges, 5 Action items (1, 2, 3, 5, 6), 6 Epic 4 preview.
- `_bmad-output/implementation-artifacts/3-1-…`, `3-3-…`, `3-4-…`, `3-5-…` (bench rigs, `SerialIo`, D7), `4-1-…`, `4-3-…` (conventions, AC → measurement format).
- `docs/test-plan.md` › Test environment, Bench rig for the own bridge (Story 3.1), `ocb-dev` bench rig (3.4), A.3 / H.3 bench rig (3.5), App core tests (C.1–C.20), App GUI tests (G.1–G.12), Hardware-in-the-loop (H.0–H.27).
- `docs/link-protocol.md` (session open, HELLO cadence; unchanged).
- `_bmad-output/specs/spec-bonooru/roadmap.md` › M4, M5.
- `app/tools/ocb-dev/src/serial.rs`, `port_rule.rs`; `app/core/src/link/{mod.rs, events.rs, send.rs}`; `app/sim/src/{controller.rs, channel.rs}`; `app/tools/ocb-dev/tests/common/mod.rs` (SimIo pattern).
- `serialport` 4.10.1 sources: `src/windows/enumerate.rs` (HWID parse, parent HWID, `SPDRP_FRIENDLYNAME`, `SPDRP_MFG`), `src/windows/error.rs`, `src/windows/com.rs`, `src/posix/enumerate.rs` (IOKit callout/dial-in, udev properties, sysfs fallback), `src/posix/error.rs` (`EBUSY` → `NoDevice`, `EACCES` → `PermissionDenied`), README (libudev, pkg-config).
- `firmware/bridge/platform/usb_descriptors.c` (VID/PID, strings, `TUD_CDC_DESCRIPTOR` with `CDC_COMM_PROTOCOL_NONE`).

## Dev Agent Record

### Agent Model Used

Claude Opus 5.5 (`claude-opus-5-5`), gds-dev-story, 2026-11-27.

### Debug Log References

**Scope of this session.** Host part only: Tasks 1–9, the local part of Task 10, Task 11 (bench-tool review) and Task 12 (dry run). Tasks 13–16 (benches, M4) not started; no serial port was opened on the Mac for real (no bridge or controller was plugged: `/dev/cu.*` showed only `Bluetooth-Incoming-Port` and `debug-console`, both non-USB and `ignored`). The story's "choices you may want to overrule" stand as written (libudev kept, Linux group read from the device node, `ocb-dev` not refactored). Open question 1 answered by the owner (separate physical x64 PCs, not VMs), then the owner deferred Windows and Linux (AC 12 amended, see Change Log). Open question 2: default (host tests only for the foreign path).

**Implementation choices (within the tasks; noted for review).**
- `Port` (Task 3.1) is the small `SerialPort`-like trait (`read`, `write`, `bytes_to_write`, `set_dtr`, `clear_input`, `io::Result`); the `SerialIo` rules (drop the handle on a hard error, 4096 B / 1 s write drop, soft read errors as 0 bytes, DTR then clear on open) live in the wrapper `OpenPort`, so the worker asserts DTR and clears input itself and the fake records the order (C.23 `open_sequence`). `OpenError::{PermissionDenied, Busy, Gone, Other}` as specified; `SerialBackend` remembers its last listing to tell `Busy` (listed) from `Gone`.
- Events travel as `TimedEvent { at_us, event: WorkerEvent }` (the probe needs the worker-clock time of every event, e.g. `PortListed` → INFO). `ScanDone` keeps its own `at_us`/`took_us` as specified. `PortLost` carries `last_rx_us` so `lost_after_last_rx_ms` comes from the worker's receive time, not a counter. STATUS is not forwarded as an event (it is in the snapshot, with its receive time), so the bounded event channel never fills at 20 Hz.
- A dropped `WorkerHandle` (all command senders gone) is a shutdown too, so a worker never outlives its app (kittest harnesses drop the app without `on_exit`).
- Round robin (AC 4) applies only with no `Ours` and more than one `Unconfirmed`; the window check runs before the session pass, so nothing is written to a port in the pass that ends its window (found by C.23 `probe_round_robin`: a HELLO at exactly 2.000 s went out before the close). Permission denials are retried at the next scan only; `Busy`/`Gone`/`Other` every 250 ms.
- The snapshot is also published after every successful write (frame counters never stale: `alone` reads `hello_frames` from it).
- `LogUnavailable`: the log thread records the first write failure once; `Logger::start` with an unusable directory records the reason and discards entries silently (no second report).
- Session commands of later stories (SET_STATE) are reachable in tests only through `WorkerCore::session_mut()`, compiled with the dev-only feature `sim`; C.23 `everything_logged` uses it to produce `Refused NotReady` and a transport NAK (a corrupted frame), which this story's worker would never send by itself.
- `App::with_smoke` keeps its meaning (real worker, used by `main`); tests build `App::with_link(...).smoke_mode(smoke)`. The G.13 scan bans `App::new(`, `App::with_smoke(`, `SerialBackend`, `Logger::start(` and `worker::spawn(` in `app/gui/tests`.
- `ocb-probe run --expect-permission-denied` waits 10 s by default (`--step-timeout-s` overrides): the event comes in the first scan. The AC 7 bound (≤ 1000 ms after the listing) is a verdict branch of its own (`permission event <n> ms after the port was listed (limit 1000)`), added to the Dev Notes branch list because the AC table requires it.
- **CI check correction (AC 11 table):** `cargo tree -i ocb-sim` exits 0 with "warning: nothing to print" when no path exists, so "fails / non-zero" is not a usable signal. The step greps the inverted tree for the crate's own `^ocb-sim v` line, with `--target all`, for `ocb-app`, `ocb-services` and `ocb-probe`. Positive control: `cargo tree -p ocb-probe -e normal,dev --target all -i ocb-sim` prints `ocb-sim v0.0.0 … └── ocb-services … └── ocb-probe`.
- Dependencies: `crossbeam-channel 0.5.17` (MIT OR Apache-2.0), `directories 6.0.0` (MIT OR Apache-2.0) → `dirs-sys 0.5.0` (MIT OR Apache-2.0) → `option-ext 0.2.0` (MPL-2.0) and `redox_users 0.5.3` (MIT, Redox only); `libudev 0.3.0` / `libudev-sys 0.1.4` (MIT) were already reachable through serialport's default feature. All on the existing allow-list: `cargo deny --locked check licenses bans` → `bans ok, licenses ok`. No new licence.

**Red proofs (E3-A2, AC 11).** Each: the code change, the test that failed and how; then restored and green. Run by a scratch script that applies one mutation, runs the named tests, restores the file.
- C.21 Windows product compared (`if !rules.product_reported` disabled): `c21_windows_product_never_compared` `left: Foreign { product: "USB Serial Device (COM5)" } right: Unconfirmed`.
- C.21 class priority dropped from `ranked`: `c21_choice_order` fails (an unconfirmed port ranked before ours).
- C.23 rescan period 900 ms (`next + 900_000`): `c23_rescan_period` `left: 7 right: 6` list calls in 5.5 s.
- C.23 open retry removed (retry at the next scan): `c23_open_retry` `left: [0, 1000000] right: [0, 250000, 500000, 750000]`.
- Handle kept after error (`self.port = None` removed from `OpenPort::lost`): `port::tests::hard_errors_drop_the_handle` fails (`is_open()` still true). C.23 `unplug_detected` alone does not catch it (the worker drops the whole `OpenPort` on loss), so the unit test is the guard.
- C.23 foreign port opened (`Class::is_candidate` includes `Foreign`): `c23_foreign_never_touched` fails (`opens` not empty).
- C.23 permission event repeated each scan: `c23_permission_denied` `one for 10 scans: left: 10 right: 1`.
- C.23 busy event repeated: `c23_port_busy_deduplicated` `left: 20 right: 1`.
- C.23 probe window 1 ms late: `c23_probe_round_robin` `left: 2001000 right: 2000000`.
- C.23 input not cleared on open: `c23_open_sequence` `left: ["open", "dtr_on", "write", …] right: ["open", "dtr_on", "clear", …]`.
- C.23 rename not reported: `c23_rename_same_serial` `left: 0 right: 1`.
- C.23 shutdown leaves the port open: `c23_shutdown_closes` `left: 0 right: 1` closes.
- C.23 stale snapshot not popped: `c23_snapshot_channel_holds_one` fails (the snapshot read is the oldest, not the newest).
- C.23 transport NAK not logged (`new_rewinds > 999`): `c23_everything_logged` fails on `has("nak", "transport")`.
- C.23 thread never ends (the `break` on shutdown removed): `c23_shutdown_joins_quickly` fails at `assert!(h.shutdown(500 ms), "joined")`.
- C.22 rotation off by one (old `.3` not removed, `1..=keep` renames): `c22_rotation_keeps_three_old_files` fails (`bonooru.4.log` exists).
- C.22 drop counter not written: `c22_full_channel_drops_and_counts` `left: 0 right: 1` `log_dropped` lines.
- C.22 unavailable reported on every failure: `c22_unwritable_log_reported_once` `reported once: left: Some("disk full") right: None`.
- Write drop at 4097 B removed (`> OUT_QUEUE_LIMIT + 1`): `full_queue_drops_without_writing` `left: Ok(true) right: Ok(false)`.
- Write stall bound 2 s: `stalled_write_drops_after_one_second` `left: 2000000 right: 1000000`.
- Permission message text changed: `port::tests::permission_message` fails on the exact string.
- C.25 `std::process::Command::new("pkexec")` added to `services/src/port.rs`: `c25_no_privilege_escalation` fails naming the file; the scanner's self-test covers each banned token and the comment/string/raw-string/char skipping.
- G.13 blocking `recv()` in `App::ui`: both `g13_ui_code_never_blocks` (`.recv()` in `gui/src/app.rs`) and `g13_ui_never_waits` (10 frames took far over 1 s; the run took 125 s) fail.
- G.13 `App::new(` in a test file (`tests/window.rs`): `g13_tests_never_start_the_real_worker` fails naming the file.
- G.7 smoke no longer waits for the worker (`worker_scans.is_some()` dropped from `Smoke::due`): `smoke::tests::due_once_after_three_completed_frames_and_a_worker_snapshot` fails.
- `check-smoke.sh` with `worker=0`: `::error::smoke check: worker=0 < 1 (the serial worker never scanned)`, exit 1; with the field missing: `not an ok line with every field`, exit 1.
- CI "ocb-sim never ships": `ocb-services = { path = "../services", features = ["sim"] }` in `gui/Cargo.toml` → the step prints `ocb-sim v0.0.0 … └── ocb-services … └── ocb-app` and `::error::ocb-sim is in ocb-app's normal dependency graph`.
- CI "serialport libudev feature on": `serialport = { version = "=4.10.1", default-features = false }` in `services/Cargo.toml` → `::error::serialport is built without its libudev feature` (run without `--locked`: with it the step already fails on the lock file, exit 101).
- `ocb-probe` verdict branches (C.24), one mutation each in `tools/ocb-probe/src/runner.rs`, each test failed:
  - `run` PASS: log check needs 3 `port_opened` → `run_pass` gets exit 1 (log check FAIL).
  - FAIL no bridge: the foreign message always → `run_fail_no_bridge` gets `OCB_PROBE FAIL (only foreign boards: ) …`.
  - FAIL only foreign: branch removed → `run_fail_only_foreign` gets `no bridge within 3 s`.
  - FAIL permission denied: arm removed → `run_fail_permission_denied` fails (no such line; the run ends on the step timeout).
  - FAIL port busy: busy never recorded → `run_fail_port_busy` fails (no `port busy` line).
  - FAIL no INFO: limit 6 s → `run_fail_no_info` fails its time bound (gave up after 5.001 s).
  - FAIL protocol mismatch: the `RequiredUpdate` return removed → `run_fail_protocol_mismatch` gets `no INFO within 5 s`.
  - FAIL unplug not seen: step deadline doubled → `run_fail_unplug_not_seen` fails its time bound.
  - FAIL loss late: limit doubled → `run_fail_loss_late` fails (the unplug step passes).
  - FAIL replug not seen: deadline doubled → `run_fail_replug_not_seen` fails its time bound.
  - FAIL INFO late after listed: limit ×10 → `run_fail_info_late_after_listed` gets exit 0.
  - FAIL rescan period: max limit doubled → `run_fail_rescan_period` gets exit 0.
  - FAIL log check: counts ignored → `run_fail_log_check` gets exit 0.
  - INTERRUPTED: wrong step name → `run_interrupted` `left: "OCB_PROBE INTERRUPTED step=connect" right: "… step=unplug"`.
  - permission PASS: `MSG` line dropped → `permission_pass` fails on the last line.
  - permission FAIL port opened: arm removed → `left: "OCB_PROBE FAIL (no permission event within 10 s)"`.
  - permission FAIL no event: timeout doubled → `permission_fail_no_event` fails its time bound.
  - permission FAIL late: limit doubled → `permission_fail_late` gets exit 0.
  - `alone` PASS: 11 HELLO/s required → `alone_pass` gets exit 1.
  - alone FAIL INFO: both INFO arms removed → `left: "OCB_PROBE FAIL (only 1 HELLO frames in 10 s …)"`.
  - alone FAIL no bridge: search 20 s → `alone_fail_no_bridge` fails its time bound.
  - alone FAIL other port: both guards removed → `alone_fail_other_port` gets exit 0.
  - alone FAIL few HELLOs: threshold 0 → `alone_fail_few_hellos` gets exit 0.
  - alone INTERRUPTED: the window ignores Ctrl+C → `alone_interrupted` gets exit 1.
  - `list` ok: `product_raw` not printed → `list_ok` fails on the Windows line; `list` error: exit 0 → `list_error` `left: 0`.

**Task 11: bench-tool review (written before any bench result).**
- *Verdict branches → dry run → red proof:* every branch of Dev Notes › Probe verdict branches has its C.24 test and its red proof above, plus the added `permission event > 1000 ms` branch and two `run` PASS variants (macOS rename; Windows same `COM5`, product not compared). `list` ok/error are pure (`list_lines`), tested on macOS and Windows listings.
- *Prompts as the owner reads them:* `PROMPT: UNPLUG the bridge's USB cable now.` / `PROMPT: PLUG the bridge back in (on a Mac, into another USB socket if you have one).` The probe reacts to worker events; nobody presses Enter. Each step waits 120 s by default (`--step-timeout-s`).
- *Expected lines per OS (from the dry runs):* macOS `run`: `EVT +0 listed port=/dev/cu.usbmodem101 class=ours serial=53032847189C2E9C`, `EVT … opened …`, `EVT … info proto=1 fw=… bridge=…`, `STEP connect PASS …`, the unplug prompt, `EVT … lost … reason="read: …"`, `STEP unplug PASS lost_after_last_rx_ms=<n>`, the plug prompt, `EVT … vanished`, `EVT … listed port=/dev/cu.usbmodem1101 …`, `EVT … renamed from=/dev/cu.usbmodem101 to=/dev/cu.usbmodem1101`, `STEP replug PASS … renamed=yes …`, `STEP log PASS`, `OCB_PROBE PASS host=macos …`, `LOG /Users/<you>/Library/Logs/Bonooru/bonooru.log`. The macOS `/dev/tty.usbmodem*` twin is listed as `ignored` (never in `EVT` lines as a candidate). Windows: `class=unconfirmed`, `product="-"`, `product_raw="USB Serial Device (COM<n>)"` in `list`, usually `renamed=no` (COM number follows the device). Linux: `/dev/ttyACM<n>`, `class=ours`, `LOG /home/<you>/.local/state/bonooru/logs/bonooru.log`.
- *Start checklist vs plan:* the four steps (console sleep / A from our controller on the Mac only; boards plugged before the tool; SIGINT to `pgrep -x ocb-probe`, not a wrapper; the once-per-session record) are in the rig block of `docs/test-plan.md`, with a per-OS record table; VM caveats dropped (owner: physical PCs).
- *Lessons 3.1–3.5 applied:* DTR then input clear on open (3.5 H.3); the handle dropped at the first hard error (3.5 review; Linux renumbering); writes drop instead of blocking (3.1/3.5); follow the serial, not the name (3.1 H.1, `usbmodem101`/`usbmodem1101`); only `/dev/cu.*` on macOS (3.4 H.27); a listed node that does not open yet is retried every 250 ms (`ocb-dev reopen`); the runner shared by bench and dry run (3.4 D9); the numbers of the verdict line come from worker events with their times (3.4 SUMMARY lesson); interrupt goes to the tool's own PID, and the shutdown path closes the port within 2 s before the INTERRUPTED line (3.5 second-SIGINT lesson).
- *Findings of the review (no code change needed, recorded for the bench):* (1) `alone`'s `hello_frames` counts frames the OS accepted, not frames seen on the wire; a bridge without a controller link stops reading USB (Story 3.1), but 10 s of HELLO is about 700 B, far below the 4096 B drop limit, so the count is the real cadence. (2) A replug faster than one scan period is never seen as vanished/listed; `info_after_listed_ms` then falls back to the reopen time (documented in the code); a human replug takes longer. (3) H.30 through `sudo setpriv …`: `sudo` may set `HOME=/root`, the log directory is then not writable and the probe prints `EVT … log_unavailable …`; the permission verdict does not depend on the log, so the row still holds. (4) On Windows the read timer granularity (~15.6 ms) slows the loop to about 60 Hz; `scan_*` and `lost_after_last_rx_ms` keep their margins (limits 1100/1500 ms).
- *H.28–H.32 re-read against the output format:* H.28 fields (`usb=`, `serial=`, `product=`, `product_raw=`, `class=`, `OCB_PORTS … host=`) match `list_lines`; H.29 fields match the final line (`info_after_open_ms`, `lost_after_last_rx_ms`, `info_after_listed_ms`, `scan_p50_ms`, `scan_max_ms`, `enum_max_ms`, `renamed`) and the `LOG` line; H.30 matches `OCB_PROBE PASS permission port= group= after_listed_ms=` + `MSG`; H.31 matches `OCB_PROBE PASS alone port= class= hello_frames= seconds= other_ports_written=0`; H.32 uses the log kinds the worker writes (`start`, `port_opened`, `version`, `port_lost`).

**Task 12: dry run.** `cargo test --locked -p ocb-probe` green: 27 dry runs (C.24) + 3 unit tests (`real::lines_of_pid`, CLI). Manual invocation `cargo test -p ocb-probe --test dry_run -- --nocapture --exact run_pass` printed exactly:
```
EVT +0 listed port=/dev/cu.usbmodem101 class=ours serial=53032847189C2E9C
EVT +0 opened port=/dev/cu.usbmodem101 class=ours serial=53032847189C2E9C
EVT +2 info proto=1 fw=0.0.0 bridge=0.0.0
STEP connect PASS port=/dev/cu.usbmodem101 serial=53032847189C2E9C info_after_open_ms=2
PROMPT: UNPLUG the bridge's USB cable now.
EVT +1503 lost port=/dev/cu.usbmodem101 last_rx=+1501 reason="read: Device not configured"
STEP unplug PASS lost_after_last_rx_ms=2
PROMPT: PLUG the bridge back in (on a Mac, into another USB socket if you have one).
EVT +2000 vanished port=/dev/cu.usbmodem101
EVT +4000 listed port=/dev/cu.usbmodem1101 class=ours serial=53032847189C2E9C
EVT +4000 renamed from=/dev/cu.usbmodem101 to=/dev/cu.usbmodem1101
EVT +4000 opened port=/dev/cu.usbmodem1101 class=ours serial=53032847189C2E9C
EVT +4002 info proto=1 fw=0.0.0 bridge=0.0.0
STEP replug PASS port=/dev/cu.usbmodem1101 renamed=yes info_after_listed_ms=2
STEP log PASS
OCB_PROBE PASS host=macos port=/dev/cu.usbmodem1101 serial=53032847189C2E9C renamed=yes info_after_open_ms=2 lost_after_last_rx_ms=2 info_after_listed_ms=2 scan_p50_ms=1000 scan_max_ms=1000 enum_max_ms=0 fw=0.0.0 bridge=0.0.0 proto=1
LOG <sim>/bonooru.log
```
(The fake channel has 100 µs latency each way and the worker steps 1 ms, hence the 2 ms figures.) Every branch's exact line is asserted in C.24. The bench commit goes into the rig block at the bench session (the dry run is re-run on it then).

**Host verification (Task 10, local part; `/opt/homebrew/opt/rustup/bin` first on `PATH`, macOS arm64).**
- `cargo fmt --all --check`: clean.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: clean; also `-p ocb-services` and `-p ocb-probe` alone (no `sim` feature): clean.
- `cargo test --locked --workspace`: 53 test binaries, 313 tests, all pass (new: C.21 8, C.22 7, C.23 15 + 2 thread + 8 port unit, C.25 3, C.24 27 + 3, G.13 6, smoke/window updates).
- `cargo deny --locked check licenses bans`: `bans ok, licenses ok`.
- G.8: `check-accesskit.sh` for the three targets ok; no eframe `wayland` feature; eframe/egui pins `v0.37.0`.
- New CI checks locally: no `ocb-sim` in the normal graphs of `ocb-app`, `ocb-services`, `ocb-probe`; `serialport feature "libudev"` present for `x86_64-unknown-linux-gnu`.
- Smoke: `target/debug/bonooru --smoke-test` → `OCB_SMOKE ok frames=3 ppp=2 inner=1280x800 min=1024x680 theme=dark backend=AppKit worker=1`, exit 0; `check-smoke.sh … AppKit` ok. The real log at `~/Library/Logs/Bonooru/bonooru.log` got the `start` line (`app=0.0.0-dev host=macos arch=aarch64 pid=…`) and four `port_listed … class=ignored` lines (debug-console and Bluetooth, both twins); nothing was opened.
- `cargo run -p ocb-probe -- list` (enumeration only, never opens): four `PORT … usb=- class=ignored` lines and `OCB_PORTS n=4 ours=0 unconfirmed=0 foreign=0 host=macos`, exit 0; `--help` prints the usage.
- Linux in Docker (`rust:1-bookworm`, aarch64, toolchain 1.100.0 from `rust-toolchain.toml`, `pkg-config libudev-dev` installed): `cargo build --locked --release -p ocb-probe` ok; `ocb-probe --help` ok; `ocb-probe list` → `OCB_PORTS n=4 ours=0 unconfirmed=0 foreign=0 host=linux`, exit 0 (the container's tty devices, non-USB); `ldd`: `libudev.so.1`, `libgcc_s.so.1`, `libc.so.6` and the loader only. Same container: `cargo test --locked --no-fail-fast -p ocb-core -p ocb-services -p ocb-probe` all pass (C.21–C.25 on Linux rules and the real `log_dir` → `~/.local/state/bonooru/logs`) and `cargo clippy … -D warnings` clean for those crates. `ocb-app` (needs the X11 packages) was not built in Docker; CI covers it.
- Not done here (after the coordinator's push): the `app` workflow on three OS, the `probe-artifacts` downloads and the `--help` run of the macOS artifact (Task 10 left unchecked for that).

**Review fixes (2026-11-27, after `c946377`; CI was green on `c946377` for every job, the four probe artifacts included).** One line per finding, then the red proofs. Each red proof: one mutation, the test that failed and how, then restored and green (the same scratch script as before).

- *Decision (owner):* `alone` judges the session's cadence. The worker counts `hello_sent` (every HELLO `poll_transmit` hands out) and `hello_dropped` (those the write rules dropped); `alone` PASS needs `hello_sent ≥ 9/s` and prints `hello_frames=<sent> hello_dropped=<d>`; `--seconds` stays.
- *1, busy then open (pre-bench):* the busy timer ends at `PortOpened`; the busy FAIL no longer needs "no port open" (it cannot be pending then); `wait()` always asks the driver for a queued event, also after the deadline, and returns `Timeout` only after that, so an expired timer never hides an event and the connect loop never re-waits on a past deadline. New dry run `run_pass_after_busy` (busy from 0 s, open at 4.75 s, INFO at 5.55 s, after the busy limit with the port open).
- *2, physical unplug (pre-bench):* the unplug step needs `PortLost` and then `PortVanished` of the port; a reconnection before the vanish fails (`reconnected without leaving the port list (no physical unplug)`, or `reconnected to another bridge: …` when the reopened serial differs); a lost port still listed at the step timeout fails (`port lost but still listed after <n> s (no physical unplug)`). The replug step needs a candidate with the same serial listed again, then INFO on a port with that serial; no fallback to the reopen or INFO time (`listed again but no INFO within <n> s`, `reconnected to another bridge: …`).
- *3, writes never block (pre-bench):* `OpenPort` keeps an own 4096 B queue (`queue`), so the limit holds where `bytes_to_write` is not a real queue (Linux cdc-acm, Windows usbser); `flush` writes at most `WRITE_BUDGET_US` = 2 ms per pass and stops at the first call without progress; "no progress for 1 s" is measured across passes (`last_progress_us`), then what is left is abandoned (one dropped chunk) and a lone `00` closes the cut frame. Scans, shutdown (≤ 500 ms) and the log stay on time with a port that never drains (fake-clock and real-thread tests).
- *4, `lose()` backoff (pre-bench):* the lost port's retry is `now + OPEN_RETRY_US`.
- *Windows `NoDevice`:* on Windows a `NoDevice` open error triggers a fresh listing; only a port still listed now is `Busy`, else `Gone` (`still_listed`, unit-tested; POSIX unchanged: `NoDevice` = EBUSY/flock, a missing node is `Io(NotFound)`).
- *Rotation failure:* the current file is first moved aside to `bonooru.0.log`; if that fails nothing else is touched, the line is written to the (reopened) current file, the next attempt waits for another 1/16 of the limit, and the log thread writes one `log_rotate_failed <reason>` line per failure; a deleted `bonooru.log` is recreated.
- *`ScanDone` flood:* scans are no longer events; the snapshot carries `scans` and `recent_scans` (the last 64 `(start, listing time)`), which the probe merges for its scan statistics. Deviation from the AC table's `ScanDone { at_us, took_us }` event: same data, in the snapshot.
- *Round robin vs `RequiredUpdate`:* the connection is "answered" on INFO of either kind; the round robin never closes an answered port.
- *A waiting candidate blocks others:* `target(now)` takes, in rank order (round robin: after the last one tried), the first candidate whose retry time has come.
- *Dedup flags:* reset for the port on a successful open; a busy result resets the permission flag and vice versa.
- *Dead or stalled worker:* `Link::drain` sees the snapshot channel disconnect (the worker's sender goes with its thread, panics included); `App::worker_health()` gives `Running`, `Stalled` (no snapshot for 3 s; an idle worker publishes every scan), `Ended` or `NotStarted`.
- *DTR on close:* `OpenPort::close` calls `set_dtr(false)` (best effort) before dropping; the fake records DTR only when `set_dtr` is called (no more `dtr=false` from its `Drop`), so the thread test's "DTR falls" can fail.
- *Logger stop:* `Logger::stop` sets a stop flag, writes what is queued (also while another handle lives) and waits at most 500 ms (`stop_within`); `on_exit` and the probe's `finish` stop the logger whether or not the worker ended.
- *Probe log check:* `log_problems` counts only lines with ` port=<p> ` of the ports this run opened (`version` lines: `connected port=`), and the real driver reads `bonooru.1.log` then `bonooru.log` from this PID's `start` line (`run_lines`).
- *`alone` with several bridges:* every unconfirmed port the worker probes in turn is named (`port=COM3,COM9`); the remaining "other port" guard fails only for a written port that is listed and not a candidate (`wrote to a port that is not a bridge: <p>`).
- *Checks that could not fail:* (a) the smoke run reports after `WORKER_WAIT` = 20 s without a snapshot, so `worker=0` is a fail line (unit test); (b) CI `ldd` output is checked by `tools/ocb-probe/ci/check-ldd.sh` (only vDSO, loader, libudev.so.1, libc, libm, libgcc_s; no `not found`; libudev required); (c) the snapshot test asserts the waiting snapshot is the newest (`scans ≥ 2`), not the channel length; (d) C.25 rejects `process::Command` paths and any `as` in a `use` item naming `process`.

Red proofs of the review fixes:
- `busy = None` removed at `PortOpened` → `run_pass_after_busy` exit 1 (`FAIL (port busy: …)` with the port open).
- The original bug (old `wait()` guard returning `Timeout` before reading + old busy check with `opened.is_none()`) → `run_pass_after_busy` never ends: killed by `timeout 60` (rc 124), the spin of the finding.
- Unplug: the reconnection arm disabled → `run_fail_glitch_reconnected` fails (`STEP unplug FAIL reconnected` missing); the lost-but-listed reason disabled → `run_fail_lost_still_listed` fails; the other-serial reason disabled → `run_fail_other_bridge` fails.
- Replug: the listed-without-INFO reason disabled → `run_fail_listed_without_info` fails; the old fallback (`listed_at.or(Some(e.at_us))`) → `run_fail_other_bridge_at_replug` gets exit 0.
- Log check counting every writer's lines → `run_fail_log_check_other_writer` gets exit 0; `run_lines` ignoring `bonooru.1.log` → `lines_across_a_rotation` `left: 0 right: 4`.
- `alone`: the old "another port" failure on the second unconfirmed port → `alone_pass_two_unconfirmed` exit 1; the non-bridge guard disabled → `alone_fail_not_a_bridge` exit 0; judging accepted frames (`sent − dropped`) → `alone_pass_with_drops` exit 1; cadence threshold 1 → `alone_fail_few_hellos` exit 0.
- Write budget 1 s → `flush_never_takes_more_than_its_budget` fails; own queue limit removed → `own_queue_bounds_writes_when_the_os_reports_nothing` `left: 50 right: 40`; stall measured inside one call → `stall_measured_across_passes_drops_after_one_second` panics `never dropped`; the old blocking write (keep writing on timeouts until 1 s inside one pass) → `c23_shutdown_quick_even_when_writes_stall` fails (join over 500 ms).
- `lose()` reopening at once → `c23_lost_port_retried_after_backoff` `left: 1000 right: 250000`.
- `still_listed` using the stale listing → `fresh_listing_decides_busy_or_gone` fails.
- Rotation in the old order (shift first, then move the current file) → `c22_failed_rotation_keeps_old_files_and_later_lines` `the old files are untouched` fails (`.1`–`.3` rewritten); a failed rotation returning the error → the same test fails (later lines lost). `c22_deleted_log_is_recreated` still passes under that mutation: the deleted file is also covered by the general failure path (reopen), so this test guards the outcome, not one code path.
- `stop_within` without its limit → `c22_stop_is_bounded_and_keeps_queued_lines` `!ended` fails after 5.1 s; the stop flag ignored while a handle lives → the same test fails at "ended although a handle is alive" (0.5 s).
- One event per scan re-added → `c23_no_scan_events` fails.
- `answered` not set on `RequiredUpdate` → `c23_round_robin_keeps_a_bridge_that_answered` `left: 3 right: 1` opens (`[(0, "COM3"), (2000000, "COM9"), (4000000, "COM3"), …]`).
- A waiting candidate blocking (`target` returning the first in rank) → `c23_waiting_candidate_does_not_block_others` `left: 0 right: 1`.
- Flags not reset on open → `c23_flags_reset_on_a_new_state` `left: 1 right: 2`.
- `set_dtr(false)` removed from `close` → `close_lowers_dtr_then_drops` `left: [true] right: [true, false]` and `c23_shutdown_quick_even_when_writes_stall` fails on "DTR lowered on close".
- Stale snapshot not popped → `c23_waker_called_and_snapshots_bounded` fails (`stale snapshot: scans 1`): the test can fail now.
- `worker_health()` never `Ended` → `g13_dead_worker_is_visible` fails after 5 s; stall limit 60 s → `g13_stalled_worker_is_visible` fails.
- Smoke without the 20 s wait → `no_worker_snapshot_reports_worker_0_after_the_wait` fails.
- C.25 alias rule removed → `c25_scanner_self_test` fails on `use std::process::{Command as C, Stdio};`.
- `check-ldd.sh`: real arm64 (Docker) and x64 outputs pass; an extra `libssl.so.3` → `::error::ldd: library not on the allowed list`, exit 1; `libudev.so.1 => not found` → `::error::ldd: missing library`, exit 1; no libudev → `::error::ldd: libudev.so.1 not linked`, exit 1.

**Task 12 again (all verdict branches):** `cargo test -p ocb-probe` green: 36 dry runs (C.24) + 4 unit tests. `run_pass` (`--nocapture --exact run_pass`) prints:
```
EVT +0 listed port=/dev/cu.usbmodem101 class=ours serial=53032847189C2E9C
EVT +0 opened port=/dev/cu.usbmodem101 class=ours serial=53032847189C2E9C
EVT +2 info proto=1 fw=0.0.0 bridge=0.0.0
STEP connect PASS port=/dev/cu.usbmodem101 serial=53032847189C2E9C info_after_open_ms=2
PROMPT: UNPLUG the bridge's USB cable now.
EVT +1503 lost port=/dev/cu.usbmodem101 last_rx=+1501 reason="read: Device not configured"
EVT +2000 vanished port=/dev/cu.usbmodem101
STEP unplug PASS lost_after_last_rx_ms=2
PROMPT: PLUG the bridge back in (on a Mac, into another USB socket if you have one).
EVT +5000 listed port=/dev/cu.usbmodem1101 class=ours serial=53032847189C2E9C
EVT +5000 renamed from=/dev/cu.usbmodem101 to=/dev/cu.usbmodem1101
EVT +5000 opened port=/dev/cu.usbmodem1101 class=ours serial=53032847189C2E9C
EVT +5002 info proto=1 fw=0.0.0 bridge=0.0.0
STEP replug PASS port=/dev/cu.usbmodem1101 renamed=yes info_after_listed_ms=2
STEP log PASS
OCB_PROBE PASS host=macos port=/dev/cu.usbmodem1101 serial=53032847189C2E9C renamed=yes info_after_open_ms=2 lost_after_last_rx_ms=2 info_after_listed_ms=2 scan_p50_ms=1000 scan_max_ms=1000 enum_max_ms=0 fw=0.0.0 bridge=0.0.0 proto=1
LOG <sim>/bonooru.log
```
`alone_pass`:
```
EVT +0 listed port=/dev/cu.usbmodem101 class=ours serial=53032847189C2E9C
EVT +0 opened port=/dev/cu.usbmodem101 class=ours serial=53032847189C2E9C
OCB_PROBE PASS alone port=/dev/cu.usbmodem101 class=ours hello_frames=100 hello_dropped=0 seconds=10 other_ports_written=0
```

**Verification of the review fixes (macOS arm64, `/opt/homebrew/opt/rustup/bin` first):** `cargo fmt --all --check` clean; `cargo clippy --locked --workspace --all-targets -- -D warnings` clean, also `-p ocb-services -p ocb-probe` without `sim`; `cargo test --locked --workspace`: 53 binaries, 340 tests, all pass; `cargo deny --locked check licenses bans`: `bans ok, licenses ok`; G.8 accesskit for the three targets ok, no eframe `wayland`; "ocb-sim never ships" and "serialport libudev" checks ok; smoke `--smoke-test` and `--smoke-test --gallery`: `OCB_SMOKE ok frames=3 ppp=2 inner=1280x800 min=1024x680 theme=dark backend=AppKit worker=1`, checker ok; `ocb-probe list` (enumeration only): `OCB_PORTS n=4 ours=0 unconfirmed=0 foreign=0 host=macos`. No serial port was opened (only `cu.Bluetooth-Incoming-Port` and `cu.debug-console`, non-USB). Linux in Docker (`rust:1-bookworm`, aarch64, rustc 1.100.0, `pkg-config libudev-dev`): release build of `ocb-probe` ok, `--help` ok, `list` → four `/dev/ttyS*` `ignored` and `OCB_PORTS n=4 … host=linux`; `ldd` = vDSO, `libudev.so.1`, `libgcc_s.so.1`, `libc.so.6`, loader, and `check-ldd.sh` → `ldd check ok: 5 lines, all allowed`; `cargo test --locked --no-fail-fast -p ocb-core -p ocb-services -p ocb-probe` all pass; clippy `-D warnings` clean for those crates.

**macOS bench (Tasks 10, 13, 16; 2026-11-27).**
- *Task 10 (CI part):* the `app` workflow run for `5832ac1` was green on every job, the four `probe-artifacts` included; the macOS artifact's `--help` was checked on the Mac.
- *Rig:* macOS 27.2.1 (26B131), MacBook arm64; release builds of `ocb-probe` and `bonooru` at `5832ac1` from the repo; C.24 re-run on that commit just before the bench, 36 passed (Task 12). Firmware: bridge Story 3.1 `fw-release` `69f4d2c`, controller `fw-debug-o3` (owner confirmed: not reflashed since the Story 3.5 bench); INFO `proto=1 fw=0.0.0 bridge=0.0.0 hid=P2`. Controller powered from the Mac by USB, no console. The labelled cable; the bridge moved from socket `1101` to `2101` on replug. Checklist: boards plugged before the tools; no other serial tool running (checked with `pgrep`); console not involved.
- *H.28 `list`:* PASS. `PORT name=/dev/cu.usbmodem1101 usb=2E8A:000A serial=53032847189C2E9C manufacturer="Bonooru" product="Bonooru Bridge" class=ours`, its twin `/dev/tty.usbmodem1101` `class=ignored`, the debug-console and Bluetooth ports (both twins) `usb=- class=ignored`; `OCB_PORTS n=6 ours=1 unconfirmed=0 foreign=0 host=macos`.
- *H.29 `run`:* PASS. `info_after_open_ms=4`; unplug: `lost` at +52929 (`read: Broken pipe`, last byte +52774), `vanished` at +53007, `lost_after_last_rx_ms=155`; replug: `listed port=/dev/cu.usbmodem2101` at +74000, `renamed from=/dev/cu.usbmodem1101 to=/dev/cu.usbmodem2101`, `opened` +74001, `info` +74006, `info_after_listed_ms=5`; `STEP log PASS`; `OCB_PROBE PASS host=macos port=/dev/cu.usbmodem2101 serial=53032847189C2E9C renamed=yes info_after_open_ms=4 lost_after_last_rx_ms=155 info_after_listed_ms=5 scan_p50_ms=1000 scan_max_ms=1009 enum_max_ms=1 fw=0.0.0 bridge=0.0.0 proto=1`; `LOG` at `~/Library/Logs/Bonooru/bonooru.log` (the AC 8 path). All AC 4/6/8 limits held with wide margins (1500 ms vs 155 and 5 ms; scan p50 1000, max 1009).
- *H.31 `alone`* (controller USB unplugged): PASS. `OCB_PROBE PASS alone port=/dev/cu.usbmodem2101 class=ours hello_frames=100 hello_dropped=0 seconds=10 other_ports_written=0`.
- *H.32 app:* PASS. The window stayed fluid while the owner moved and resized it during the unplug; exit code 0, 64 ms after the log's `port_closed` (11:22:04.391Z → 11:22:04.455Z). After this run's `start` line, in order: `port_opened` 11:21:38.054, `version connected` 11:21:38.056 (hid=P2 queue_cap=512 app_timeout_ms=1000 report_period_us=2000), `port_lost` 11:21:49.576 (Broken pipe), `port_vanished` 11:21:50.058, `port_listed` 11:22:01.053, `port_opened` 11:22:01.056, `version connected` 11:22:01.058, `port_closed` 11:22:04.391.
- *Task 16:* AC 12 (as amended) holds on macOS. M4 stays TODO in `roadmap.md`, DONE only when the deferred Windows/Linux rows pass (GitHub issue #43; sprint action S4.4-A1). Windows/Linux rows H.28–H.31 stay deferred in the test plan with #43.

**Forward note for Story 4.5 (OQ-5, OQ-6; macOS data now, Windows/Linux with #43):** on macOS the bridge is listed with its product string (`class=ours`) before any INFO, so "Bridge – found" (OQ-6) can rest on `Ours` there. Alone (controller unplugged) the bridge accepted HELLO every 100 ms with none dropped (H.31 `hello_dropped=0`): it keeps reading USB with no controller attached, so "bridge found, controller not answering" is observable as open port + no INFO, with no write drops. Windows will report `unconfirmed` (no product string) and is the open part of OQ-5; its data comes with #43. The same note is in `epics.md` under Story 4.5.

### Completion Notes List

- Ultimate context engine analysis completed - comprehensive developer guide created.
- macOS bench 2026-11-27: H.28, H.29, H.31, H.32 PASS at `5832ac1`; Windows/Linux deferred to GitHub issue #43 (sprint action S4.4-A1); M4 stays TODO; OQ-5/OQ-6 macOS data handed to Story 4.5. Status set to `review`.
- Tasks 1–9, 11, 12 done; Task 10 done locally (CI after push pending, so unchecked); Tasks 13–16 not started (bench; Windows/Linux deferred by owner decision 2026-11-27). Story stays `in-progress`.
- `ocb_core::device`: pure discovery rules (`classify`, `ranked`, `choose`, natural port-name order), under the C.4 purity gate.
- `ocb-services` (new crate, no GUI): `port` (traits, `SerialBackend` on serialport 4.10.1 with libudev, `OpenPort` with the `SerialIo` lessons, permission message, `/etc/group` lookup), `log` (rotating `bonooru.log`, own thread, bounded 1024 queue with drop count, OS log dirs via `directories` 6.0.0, hand-written UTC), `worker` (`WorkerCore` steppable on a fake clock, `spawn` on a `bonooru-serial` thread, events + `Arc<LinkSnapshot>` over crossbeam, waker), `sim` (feature `sim`: `FakeBackend` with `ocb-sim` controllers, `BlockingBackend`, `SimRig`).
- `ocb-app`: starts the real worker and log at launch, drains events/snapshots with `try_recv` once per frame (keeps the last 64 events and the latest snapshot), bounded join in `on_exit`; smoke waits for the first worker snapshot and prints `worker=<scans>`; tests use `App::with_link` on fake backends only.
- `ocb-probe` (new dev-only crate): `list`, `run`, `run --expect-permission-denied`, `alone`, shared runner for bench and dry run; 27 dry runs.
- CI: `libudev-dev`, "ocb-sim never ships" (grep of the inverted tree; `cargo tree -i` exits 0 when nothing matches), "serialport libudev feature on", new `probe-artifacts` job (ubuntu-22.04 x64, ubuntu-22.04-arm, windows-latest, macos-latest; release build, `--help` run check, `ldd` in the summary, tar for Unix, upload-artifact@v7, 14 days).
- Review fixes (2026-11-27): every finding of the code review fixed (see Debug Log › Review fixes): busy-then-open, physical unplug/replug, non-blocking writes with a per-pass budget, lost-port backoff, Windows busy/gone, rotation failure, no scan events, round robin vs RequiredUpdate, retry not blocking, flags reset, worker health in the UI, DTR on close, bounded logger stop, robust probe log check, `alone` with several bridges and cadence + `hello_dropped=`, and four checks made able to fail (smoke `worker=0`, `check-ldd.sh`, snapshot newest, C.25 aliases).
- Docs: test plan rows C.21–C.25, G.7 amended, G.13, H.28–H.32 (Windows/Linux deferred), discovery bench rig with the start checklist, test environment rows for the two PCs; `app/README.md` (layout, probe, log location, libudev); roadmap M4 condition.

### File List

New:
- `app/core/src/device.rs`
- `app/core/tests/device.rs`
- `app/services/Cargo.toml`
- `app/services/src/lib.rs`
- `app/services/src/port.rs`
- `app/services/src/log.rs`
- `app/services/src/worker.rs`
- `app/services/src/sim.rs`
- `app/services/tests/log.rs`
- `app/services/tests/worker.rs`
- `app/services/tests/worker_thread.rs`
- `app/services/tests/scan.rs`
- `app/tools/ocb-probe/Cargo.toml`
- `app/tools/ocb-probe/src/main.rs`
- `app/tools/ocb-probe/src/lib.rs`
- `app/tools/ocb-probe/src/cli.rs`
- `app/tools/ocb-probe/src/real.rs`
- `app/tools/ocb-probe/src/runner.rs`
- `app/tools/ocb-probe/tests/dry_run.rs`
- `app/tools/ocb-probe/ci/check-ldd.sh`
- `app/gui/tests/worker.rs`
- `_bmad-output/implementation-artifacts/4-4-serial-worker-bridge-discovery-and-file-log.md`

Changed:
- `.github/workflows/app.yml`
- `app/Cargo.toml`
- `app/Cargo.lock`
- `app/core/src/lib.rs`
- `app/gui/Cargo.toml`
- `app/gui/src/app.rs`
- `app/gui/src/smoke.rs`
- `app/gui/ci/check-smoke.sh`
- `app/gui/tests/common/mod.rs`
- `app/gui/tests/window.rs`
- `app/gui/tests/main_screen.rs`
- `app/gui/tests/gallery.rs`
- `app/tools/ocb-dev/Cargo.toml` (comment only)
- `app/README.md`
- `docs/test-plan.md`
- `_bmad-output/specs/spec-bonooru/roadmap.md` (M4 condition text; M4 stays TODO)
- `_bmad-output/planning-artifacts/epics.md` (Story 4.5 forward note: macOS OQ-5/OQ-6 data)
- `_bmad-output/implementation-artifacts/sprint-status.yaml`

### Change Log

- 2026-11-27: Host implementation of Story 4.4 (Tasks 1–9, 11, 12; Task 10 locally): discovery rules, `ocb-services` (port adapter, rotating log, serial worker, fakes), app integration with `worker=` in the smoke line, `ocb-probe` with dry runs per verdict branch, CI (libudev, ocb-sim check, libudev feature check, `probe-artifacts`), docs. Status stays `in-progress` (benches pending).
- 2026-11-27: Owner answer to open question 1 recorded (separate physical x64 PCs, not VMs); VM caveats removed from the Dev Notes and the owner runbooks.
- 2026-11-27: Owner decision: Windows and Linux unavailable for now. AC 12 amended (passes on macOS, Windows/Linux rows deferred); Tasks 14 and 15 marked deferred; the H-rows and the rig block mark the Windows/Linux parts deferred; `roadmap.md` M4 stays TODO with the condition that the deferred rows pass.
- 2026-11-27: Fixed every Story 4.4 code review finding (Review Findings all checked; the decision resolved by the owner: `alone` judges the session's HELLO cadence and reports `hello_dropped=`); new and changed `ocb-probe` verdict branches dry-run and red-proved; `check-ldd.sh` added to the `probe-artifacts` job; test plan rows C.22–C.25, G.7, G.13, H.29, H.31 amended. Status stays `in-progress`.
- 2026-11-27: Recorded the macOS bench (H.28, H.29, H.31, H.32 PASS at `5832ac1`; firmware unchanged since the Story 3.5 bench, owner confirmed); Tasks 10, 13, 16 checked; Windows/Linux deferred to GitHub issue #43 (sprint action S4.4-A1); M4 stays TODO; OQ-5/OQ-6 forward note to Story 4.5; status `review`.
