# app

Cross-platform desktop app in **Rust + egui/eframe** (ADR-008, accepted 2026-11-20): bridge auto-detection, score import (text + MIDI), timeline preview, playback, virtual gamepad, built-in flasher. Crates and layout: AD-18 and `app-stack-options.md` in `_bmad-output/planning-artifacts/architecture/architecture-bonooru-2026-11-19/`. Feasibility evidence: `spikes/app-egui/SPIKE-REPORT.md`.

## Layout

A Cargo workspace (Story 3.2), Rust pinned to 1.100.0 by `rust-toolchain.toml`:

- `core/`: crate `ocb-core`, the GUI-free core (AD-18): no egui/eframe, no threads, no I/O, no runtime dependencies. Today it holds `protocol` (COBS, CRC-16/CCITT-FALSE, frames, the receive splitter, the canonical state and every message) and `version` (the app version from `git describe`, fallback `0.0.0-dev`). `link` is the sans-IO session (Story 3.3): HELLO and INFO, go-back-N with credits, STOP, PING and typed events; bytes and a µs clock in, bytes and events out.
- `sim/`: crate `ocb-sim`, dev-only and never shipped: a fake controller that follows `docs/link-protocol.md`, an impaired channel (drop, bit flip, junk, duplicate, reorder) and a driver on a fake clock. The session tests C.6 to C.11 live in `sim/tests/`; Story 3.4 uses it for the dry run of `ocb-dev`.
- `gui/`: crate `ocb-app`, the desktop app (Story 4.1), binary `bonooru`; the only crate that depends on egui. eframe and egui are pinned at `=0.37.0` with default features off (`accesskit`, `clipboard`, `default_fonts`, `wgpu`, `x11`: no `wayland`, no `system_fonts`); on Linux the app forces winit's X11 backend (AD-18). Modules: `ui::theme` (every `DESIGN.md` token, the single source), `ui::fonts`, `ui::layout` and `ui::main_screen` (the five main-screen regions), `ui::paint` (painter-level helpers that handle egui's limits once: focus ring, glow, text halo, dashed rects, convex-only fills, stroke floor, digit cells, corner brackets, the dialog flourish; no accessibility nodes), `ui::symbols` (the 12 painted symbols), `ui::widgets` (themed button, standalone symbol and time, each one accessible node), `ui::modal` (the modal dialog and `Modals`, the app's one-at-a-time dialog host), `ui::status_strip` (`src/ui/status_strip.rs`, Story 4.5: the brand and the three status LEDs, each focusable with its tooltip and one `Role::Status` node), `leds` (`src/leds.rs`, Story 4.5: the LED decisions as plain data, no egui: the latest worker snapshot, the time since it arrived and the worker's health → three LEDs, their copy and the `led` log lines), `gating` (`src/gating.rs`, Story 4.6: the gate as plain data, no egui: whether a required firmware update blocks the gamepad and Play, the once-per-connection trigger of the firmware dialog, its copy and the `gate` log lines), `ui::firmware` (`src/ui/firmware.rs`, Story 4.6: the OK-only "Firmware update needed" dialog), `stop` (`src/stop.rs`, Story 4.7: STOP as plain data, no egui: who asked, what became of the worker command, the `stop requested` line and the bottom bar's copy), `ui::keys` (`src/ui/keys.rs`, Story 4.7: the keys read at the top of every frame: Esc read for STOP without being consumed, Space consumed so it never clicks a focused button), `ui::bottom_bar` (`src/ui/bottom_bar.rs`, Story 4.7: the bottom bar's pure layout, the safety note and STOP, `widgets::stop_button`, with the three Play/Listen slots reserved and empty), `ui::gallery` (the hidden debug gallery), `cli`, `smoke`.
- `services/`: crate `ocb-services` (Story 4.4), what the app does besides drawing; no egui. `port` (the traits the worker talks to and the real `serialport` 4.10.1 adapter: 460800 8N1, DTR asserted then the input cleared, writes that drop instead of blocking, the handle dropped on the first hard error), `log` (the rotating `bonooru.log`, 1 MiB × 4 files, on its own thread behind a bounded queue), `worker` (the serial worker: one thread named `bonooru-serial` owns the port and the link session, lists ports every second, opens only `Ours`/`Unconfirmed` bridges by the `ocb_core::device` rules, follows the bridge by its serial number; the UI gets events and immutable snapshots over `crossbeam-channel`) and, with the dev-only feature `sim`, `sim` (fake backends for tests and dry runs, never in a shipped build).
- `tools/ocb-probe/`: crate `ocb-probe` (Story 4.4), dev-only and never shipped: the discovery bench tool. It runs the same worker and log headless and prints machine-checkable lines (`list`, `run`, `run --expect-permission-denied`, `alone`; see `--help` and `docs/test-plan.md` H.28 to H.31). CI builds it in release mode for Windows x64, Linux x64 and arm64 and macOS arm64 (job `probe-artifacts`, artifacts `ocb-probe-<os>-<arch>`), so a bench machine needs no Rust.
- `profiles/`: the bundled default mapping profiles (TOML, one per game version, AD-17); the first, `oot-switch2-1.1.1.toml`, comes from Story 1.5. Not a crate.

## Running the app

From `app/`:

```sh
cargo run -p ocb-app                    # the main screen
cargo run -p ocb-app -- --gallery       # the hidden debug gallery (design review)
cargo run -p ocb-app -- --smoke-test    # 3 frames, one OCB_SMOKE ok/fail line: exit 0 ok, 4 fail, 3 watchdog (60 s)
```

If `cargo` is not on your `PATH` (rustup installed without `~/.cargo/bin` in the shell profile), put the toolchain first, e.g. `export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"` on an Apple Silicon Mac, or build once and run the binary directly: `cargo build -p ocb-app` then `target/debug/bonooru --gallery` (from `app/`).

The gallery shows every token, then one section per painting helper (focus ring, glow, halo, dashed rects, convex fills, stroke floor, digit cells, the 12 symbols, corner brackets) and buttons that open two sample dialogs.

The app starts the serial worker at launch (Story 4.4): it finds the bridge by itself (USB `2E8A:000A`, product `Bonooru Bridge` where the OS reports it) and never asks for a port. The smoke line ends with `worker=<scans>` (at least 1). The log is `bonooru.log` in the OS log directory: macOS `~/Library/Logs/Bonooru/`, Windows `%LOCALAPPDATA%\Bonooru\logs\`, Linux `$XDG_STATE_HOME/bonooru/logs/` (default `~/.local/state/bonooru/logs/`); it rotates at 1 MiB and keeps three old files (`bonooru.1.log` to `bonooru.3.log`). On Linux, opening the bridge needs the user in the group of the device node (`dialout` on Debian, Ubuntu and Fedora, `uucp` on Arch); the app says so and never asks for privileges. Every change of a status LED is written to the same log as one `led` line (`led <link>=<state> name="<name>" reason=<reason> status_age_us=<n|-> session_age_us=<n|->`, ages in µs, one line per change of state, name or reason, Story 4.5), next to the worker's port and version lines. Every change of the gate (Story 4.6) is one `gate` line (`gate gamepad=<on|off> play=<on|off> reason=<none|firmware_update> controller_proto=<n|-> app_proto=<n|->`), and the firmware dialog writes `dialog shown id=firmware_update_needed connected_us=<n> …` when it is requested (once per mismatching connection: again after a bridge replug or a restart) and `dialog closed id=firmware_update_needed outcome=<primary|dismissed>` when OK or Esc closes it. STOP requests and their handling appear as `stop` lines in `bonooru.log` (Story 4.7): the app writes `stop requested source=<esc|button> dialog=<open|none> command=<queued|no_worker|worker_gone|queue_full>` for each Esc press or STOP activation (at most one per frame, key repeats ignored), the worker `stop sent|pending port=<p>`, `stop skipped reason=<no_port|opening|incompatible|closed> port=<p|->`, `stop acked port=<p> after_us=<n|-> by=<command|session>` and `stop after port=<p> mode=<idle|direct|playing> abort_count=<n>` (the first STATUS after the ACK).

## Running the probe (bench)

From `app/` (or the unpacked CI artifact on Windows and Linux):

```sh
cargo run --release -p ocb-probe -- list          # every serial port, its USB ids and class; never opens a port
cargo run --release -p ocb-probe -- run           # connect, unplug, replug, log check (prompts on screen)
cargo run --release -p ocb-probe -- alone         # the bridge without a controller: HELLO only
cargo run --release -p ocb-probe -- run --expect-permission-denied   # Linux, H.30
```

Exit codes: 0 PASS, 1 FAIL, 2 usage, 130 interrupted. Every verdict line has a dry run (`cargo test -p ocb-probe -- --nocapture run_pass` prints what the bench shows).

`gui/ci/check-smoke.sh <file> <backend-regex>` re-checks a smoke line (CI); `gui/ci/check-accesskit.sh [target...]` checks that ocb-app enables eframe's `accesskit` feature in the normal dependency graph (CI, G.8). On Linux the build needs `pkg-config` and `libudev-dev` (Fedora: `systemd-devel`; serialport's port enumeration, run time `libudev.so.1`) and the X11 development packages listed in the `app` CI workflow (`.github/workflows/app.yml`, step "Linux system dependencies").

## Bundled fonts

Cormorant Garamond and Nunito Sans ship unmodified (no subsetting, no renaming) in `gui/assets/fonts/`, from [google/fonts](https://github.com/google/fonts) at commit `5026800714d07231937d92eb4cceabf06e05d74e`:

| File | Source path | Size | sha256 |
|---|---|---|---|
| `CormorantGaramond-VF.ttf` | `ofl/cormorantgaramond/CormorantGaramond[wght].ttf` | 1 195 560 B | `b20b7d9626dd956b2c5e558692ad328b1f19e3275e2782db4fa07670d83f35e0` |
| `NunitoSans-VF.ttf` | `ofl/nunitosans/NunitoSans[YTLC,opsz,wdth,wght].ttf` | 571 240 B | `f934d7142fb4784bf828da485b7dcbd90c0c80d514e9d49a5da0ed3a1ae2491d` |

Both are OFL-1.1. Hack, Ubuntu-Light and the `egui-icons` subset come from egui's `default_fonts`. The licence texts of every font in the binary are in `gui/assets/licenses/` (the two `OFL.txt` files from the same google/fonts commit; Hack, UFL-1.0 and the emoji-icon-font MIT licence from `epaint_default_fonts` 0.37.0), for the About view and the release notices (AD-21).

## Checks

Run from `app/` (the same gates as the `app` CI workflow on ubuntu, windows and macos):

```sh
cargo fmt --all --check
cargo test --workspace                       # C.1 to C.25, G.1 to G.6 and G.10 to G.13 in docs/test-plan.md
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check licenses bans               # NFR-07 allow-list and the AD-21 network-crate ban, deny.toml
```

`cargo test` reads the shared conformance vectors in `firmware/common/proto/vectors/` (AD-14): the app and the firmware codecs must agree on every one of them.
