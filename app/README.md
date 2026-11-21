# app

Cross-platform desktop app in **Rust + egui/eframe** (ADR-008, accepted 2026-11-20): bridge auto-detection, score import (text + MIDI), timeline preview, playback, virtual gamepad, built-in flasher. `app/` becomes a Cargo project (workspace with a GUI-free core crate and the app binary) with the first app story; crates and layout: AD-18 and `app-stack-options.md` in `_bmad-output/planning-artifacts/architecture/architecture-ocarina-bot-2026-11-19/`. Feasibility evidence: `spikes/app-egui/SPIKE-REPORT.md`.

`profiles/` holds the bundled default mapping profiles (TOML, one per game version, AD-17); the first, `oot-switch2-1.1.1.toml`, comes from Story 1.5.
