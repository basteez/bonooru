# Ocarina Bot

Plays arbitrary melodies (MIDI or hand-written scores) on Link's ocarina in
*The Legend of Zelda: Ocarina of Time* (Switch 2) by emulating a wired controller.
Two RP2040-Zero boards linked over UART, driven by a cross-platform desktop app.

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
| `app/`                 | Desktop app (stack per ADR-008): score import, preview, playback, flasher |
| `spikes/`              | Throwaway experiments (M2 timing / fingering checks)              |
| `docs/`                | Reference docs used during development                            |
| `_bmad/`, `_bmad-output/` | BMad Method tooling and planning artifacts                     |

## Versioning and releases

- One version for the whole repo, tagged `vX.Y.Z`. Firmware and app built from the
  same tag must agree on the protocol version (FR-10).
- A release ships: the desktop app for Windows, Linux and macOS, plus `bridge.uf2`
  and `controller.uf2` (the app bundles both UF2s for its built-in flasher).
- CI (build + test) is added with the first story that produces code; the release
  workflow lands with milestones M8–M9.
