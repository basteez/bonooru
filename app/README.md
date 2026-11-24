# app

Cross-platform desktop app in **Rust + egui/eframe** (ADR-008, accepted 2026-11-20): bridge auto-detection, score import (text + MIDI), timeline preview, playback, virtual gamepad, built-in flasher. Crates and layout: AD-18 and `app-stack-options.md` in `_bmad-output/planning-artifacts/architecture/architecture-bonooru-2026-11-19/`. Feasibility evidence: `spikes/app-egui/SPIKE-REPORT.md`.

## Layout

A Cargo workspace (Story 3.2), Rust pinned to 1.100.0 by `rust-toolchain.toml`:

- `core/`: crate `ocb-core`, the GUI-free core (AD-18): no egui/eframe, no threads, no I/O, no runtime dependencies. Today it holds `protocol` (COBS, CRC-16/CCITT-FALSE, frames, the receive splitter, the canonical state and every message) and `version` (the app version from `git describe`, fallback `0.0.0-dev`). The sans-IO link session follows in Story 3.3.
- `profiles/`: the bundled default mapping profiles (TOML, one per game version, AD-17); the first, `oot-switch2-1.1.1.toml`, comes from Story 1.5. Not a crate.

## Checks

Run from `app/` (the same gates as the `app` CI workflow on ubuntu, windows and macos):

```sh
cargo fmt --all --check
cargo test                                   # C.1 to C.5 in docs/test-plan.md
cargo clippy --all-targets -- -D warnings
cargo deny check licenses                    # NFR-07 allow-list, deny.toml
```

`cargo test` reads the shared conformance vectors in `firmware/common/proto/vectors/` (AD-14): the app and the firmware codecs must agree on every one of them.
