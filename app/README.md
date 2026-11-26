# app

Cross-platform desktop app in **Rust + egui/eframe** (ADR-008, accepted 2026-11-20): bridge auto-detection, score import (text + MIDI), timeline preview, playback, virtual gamepad, built-in flasher. Crates and layout: AD-18 and `app-stack-options.md` in `_bmad-output/planning-artifacts/architecture/architecture-bonooru-2026-11-19/`. Feasibility evidence: `spikes/app-egui/SPIKE-REPORT.md`.

## Layout

A Cargo workspace (Story 3.2), Rust pinned to 1.100.0 by `rust-toolchain.toml`:

- `core/`: crate `ocb-core`, the GUI-free core (AD-18): no egui/eframe, no threads, no I/O, no runtime dependencies. Today it holds `protocol` (COBS, CRC-16/CCITT-FALSE, frames, the receive splitter, the canonical state and every message) and `version` (the app version from `git describe`, fallback `0.0.0-dev`). `link` is the sans-IO session (Story 3.3): HELLO and INFO, go-back-N with credits, STOP, PING and typed events; bytes and a µs clock in, bytes and events out.
- `sim/`: crate `ocb-sim`, dev-only and never shipped: a fake controller that follows `docs/link-protocol.md`, an impaired channel (drop, bit flip, junk, duplicate, reorder) and a driver on a fake clock. The session tests C.6 to C.11 live in `sim/tests/`; Story 3.4 uses it for the dry run of `ocb-dev`.
- `gui/`: crate `ocb-app`, the desktop app (Story 4.1), binary `bonooru`; the only crate that depends on egui. eframe and egui are pinned at `=0.37.0` with default features off (`accesskit`, `clipboard`, `default_fonts`, `wgpu`, `x11`: no `wayland`, no `system_fonts`); on Linux the app forces winit's X11 backend (AD-18). Modules: `ui::theme` (every `DESIGN.md` token, the single source), `ui::fonts`, `ui::layout` and `ui::main_screen` (the five main-screen regions), `ui::gallery` (the token gallery), `cli`, `smoke`.
- `profiles/`: the bundled default mapping profiles (TOML, one per game version, AD-17); the first, `oot-switch2-1.1.1.toml`, comes from Story 1.5. Not a crate.

## Running the app

From `app/`:

```sh
cargo run -p ocb-app                    # the main screen
cargo run -p ocb-app -- --gallery       # the token gallery (design review)
cargo run -p ocb-app -- --smoke-test    # 3 frames, one OCB_SMOKE ok/fail line: exit 0 ok, 4 fail, 3 watchdog (60 s)
```

`gui/ci/check-smoke.sh <file> <backend-regex>` re-checks a smoke line (CI). On Linux the build needs the X11 development packages listed in the `app` CI workflow (`.github/workflows/app.yml`, step "Linux system dependencies").

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
cargo test --workspace                       # C.1 to C.20 and G.1 to G.6 in docs/test-plan.md
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check licenses bans               # NFR-07 allow-list and the AD-21 network-crate ban, deny.toml
```

`cargo test` reads the shared conformance vectors in `firmware/common/proto/vectors/` (AD-14): the app and the firmware codecs must agree on every one of them.
