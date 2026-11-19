# Architecture

Principle: the **app compiles** the score ahead of time into a timed list of controller states; the **controller board plays** it from its own clock. PC-side USB/serial jitter does not affect musical timing (ADR-003). Diagrams: `architecture-diagrams.md`. Binding decisions (AD-1…AD-21): `ARCHITECTURE-SPINE.md`; they win over this summary and over `docs/link-protocol.md`.

## Components

| Component        | Runs on                      | Responsibilities |
|------------------|------------------------------|------------------|
| App ("host" in the notes) | PC (Win/Linux/macOS) | import scores, arrange, pick fingerings, compile timeline, stream, virtual gamepad, flash firmware |
| Bridge board     | RP2040-Zero #1 (PC side)     | frame-aware pipe plus heartbeat: store-and-forward PC→UART per COBS frame (never decodes), best-effort UART→PC, `HEARTBEAT` `0x40` every 20 ms by hardware alarm (AD-4); identifies itself by VID:PID + product string (AD-13); 1200-baud reset to BOOTSEL |
| Controller board | RP2040-Zero #2 (Switch side) | UART protocol, modes IDLE/DIRECT/PLAYING + abort latch, event queue, timer-driven playback, safety mask, USB HID device, watchdogs |
| Switch 2         | console                      | sees a wired controller |

App modules (from the diagram): score import (.mid, text) → arranger (mono + range fit) → fingering (profile-driven) → timeline compiler (µs events) → streamer (credit flow control); virtual gamepad feeds the streamer; flasher (UF2 / BOOTSEL) is separate.

## Controller board internals

UART RX (IRQ, break flag read; no 8-bit DMA) → frame decoder (COBS + CRC, type range, go-back-N seq) → mode logic (AD-8): `SET_STATE` to real-time state, `QUEUE_EVENT` to event queue (ring buffer). 1 MHz timer (playback clock) drives the scheduler over the queue → safety mask (AD-9 allow-list in PLAYING) → published state (seqlock) ← real-time state. Profile builds the report → TinyUSB HID IN endpoint. Link/app watchdogs and USB events abort to IDLE (NEUTRAL) and set the abort latch until `STOP`/`HELLO`.

- Core split (decided, AD-11): **core 0** runs the TinyUSB task, the active profile and report sending (only core touching TinyUSB); **core 1** runs a non-blocking RAM super-loop (UART, decode, modes, queue, scheduler, watchdogs; target < 50 µs) and is the only writer of the current state (seqlock) and of UART TX. Cross-core liveness checks plus the hardware watchdog (≤ 100 ms) reboot a stalled board.
- Report sent on every host poll; the scheduler only changes its content.

## Repo layout (existing, from README)

| Path | What |
|------|------|
| `firmware/` | One CMake project (pico-sdk + TinyUSB), two targets `bridge` and `controller` |
| `firmware/common/` | Shared: link protocol, version, UART |
| `firmware/bridge/` | frame-aware USB CDC ↔ UART pipe + heartbeat, 1200-baud touch → BOOTSEL |
| `firmware/controller/` | Pro Controller emulation, scheduler, watchdog |
| `app/` | Desktop app (stack per ADR-008): score import, preview, playback, flasher |
| `spikes/` | Throwaway experiments (M2 timing / fingering checks) |
| `docs/` | Reference docs used during development |

## Stack

- Firmware: C, pico-sdk 2.3.1 (BSD-3) + TinyUSB 0.18.0 (MIT), picotool 2.3.1 (ADR-004, AD-1).
- App: conditional per ADR-008 (AD-18) — Rust + egui/eframe if the spike passes, otherwise Python 3.12 + PySide6; per-option libraries and packaging in `app-stack-options.md`. Verdict before M5 (Q7).
- Versioning: one repo version `vX.Y.Z`; firmware and app from the same tag agree on the protocol version (FR-10).

## Hardware and wiring

| Item | Qty | Role |
|------|-----|------|
| RP2040-Zero (Waveshare) | 2 | bridge (PC side), controller (Switch side) |
| Jumper wires | 3 | UART TX/RX crossed + GND (F-F, or on a breadboard) |
| USB-C cables (data) | 2 | PC ↔ bridge, controller ↔ Switch 2 |
| PC (Windows/Linux/macOS) | 1 | app |
| Nintendo Switch 2 | 1 | target, stock firmware, no modding |

| Bridge (#1) | Controller (#2) |
|-------------|-----------------|
| GP0 (TX)    | GP1 (RX)        |
| GP1 (RX)    | GP0 (TX)        |
| GND         | GND             |

- Same pinout as RiiDraw/2wiCC (method A jumpers or RiiDraw adapter board), verified on the owner's Switch 2.
- Do **not** link 5V/3V3: each board is powered by its own USB. On the Zero the 5V pin is VBUS with no diode, so on the controller board it is the Switch's VBUS; GND sits next to it.
- Rewire only with both cables unplugged. Full rules: `risk-register.md`.
- Reference cable: Apple USB-C↔USB-C data cable (USB 2.0), 1 m — labelled and kept for the project.

## Alternatives considered

| Option | Pros | Cons | Verdict |
|--------|------|------|---------|
| A. 2× RP2040-Zero (bridge + controller) | proven by RiiDraw; parts on hand; no soldering | 2 boards to flash; 2 firmwares | **chosen** (ADR-001) |
| B. 1× Zero + USB-TTL adapter (CP210x) | one firmware; PABotBase does this | extra part; Windows driver for some chips; no 1200-baud tricks | fallback |
| C. 1× Zero, 2nd USB port via Pico-PIO-USB | one board, no adapter | needs 1.5 kΩ pull-up on D+, hand-wired USB cable, 120/240 MHz clock, CPU load next to timing loop | spike only |
| D. Microphone: PC plays melody to the Switch 2 mic | no hardware; officially supported by the game | pitch detection unknown, room noise, no control of bends | out of scope |
| E. Bluetooth Pro Controller from PC | no hardware | Linux-only stacks, Switch 2 pairing unknown, not cross-platform | rejected |

## Glossary

- **Free play** — in-game ocarina mode where arbitrary notes (incl. bends) can be played.
- **P1 / P2 / P3** — controller emulation profiles (`docs/usb-controller-emulation.md`); P2 chosen.
- **Mapping profile** — TOML file per game version holding pitches, modifiers, timing, range (`docs/score-format-and-mapping.md`).
- **HYPOTHESIS (unverified)** — marker on claims not yet measured or checked in game; never treat as fact. **RISK** — marker on identified risks (see `risk-register.md`).
- **RiiDraw / 2wiCC** — reference projects (auto-drawing on Tomodachi Life; its open firmware, unlicensed).
