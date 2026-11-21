# Bonooru

Plays arbitrary melodies (MIDI or hand-written scores) on Link's ocarina in
*The Legend of Zelda: Ocarina of Time* (Switch 2) by emulating a wired controller.
Two RP2040-Zero boards linked over UART, driven by a cross-platform desktop app written in Rust + egui/eframe.

```
PC (desktop app) ──USB──▶ bridge (RP2040-Zero) ──UART──▶ controller (RP2040-Zero) ──USB──▶ Switch 2
```

## Layout

| Path                   | What                                                              |
|------------------------|-------------------------------------------------------------------|
| `firmware/`            | One CMake project (pico-sdk + TinyUSB), two targets               |
| `firmware/common/`     | Code shared by both boards: link protocol, version, UART          |
| `firmware/bridge/`     | PC-side board: frame-aware USB CDC ↔ UART pipe + link heartbeat, 1200-baud touch → BOOTSEL |
| `firmware/controller/` | Switch-side board: Pro Controller emulation, scheduler, watchdog  |
| `app/`                 | Desktop app, Rust + egui/eframe (Cargo project, ADR-008): score import, preview, playback, flasher |
| `spikes/`              | Throwaway experiments (M2 timing / fingering checks)              |
| `docs/`                | Reference docs used during development                            |
| `_bmad/`, `_bmad-output/` | BMad Method tooling and planning artifacts                     |

## Versioning and releases

- One version for the whole repo, tagged `vX.Y.Z`. Firmware and app built from the
  same tag must agree on the protocol version (FR-10).
- A release ships: the desktop app for Windows, Linux and macOS, plus `bridge.uf2`
  and `controller.uf2` (the app bundles both UF2s for its built-in flasher).
- Firmware CI (`.github/workflows/firmware.yml`) builds both UF2s with the pinned
  toolchain and runs the firmware host tests on every push; app CI arrives with the
  app, and the release workflow lands with milestones M8–M9.

## Disclaimer

This is an unofficial fan project for personal, offline fun. It is not affiliated with,
endorsed or sponsored by Nintendo. *The Legend of Zelda* and *Nintendo Switch* are
trademarks of Nintendo. No game code, assets or console modifications are involved.
