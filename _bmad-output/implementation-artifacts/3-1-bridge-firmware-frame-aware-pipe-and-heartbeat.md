---
baseline_commit: ce79ac91c8efee181196d32fc17af7527d4dbf1f
---

# Story 3.1: Bridge firmware: frame-aware pipe and heartbeat

Status: done

<!-- Ultimate context engine analysis completed - comprehensive developer guide created (gds-create-story, 2026-11-24) -->

## Story

As the owner,
I want the bridge board to forward whole frames to the controller and inject its own heartbeat,
so that the controller has a reliable link and a fail-safe signal independent of the PC.

## Acceptance Criteria

1. **Identity.** Given the bridge UF2, when it is plugged into the PC, then it enumerates as USB CDC with VID:PID `2E8A:000A`, product string `Bonooru Bridge` and the flash unique ID as serial number (AD-13); and its WS2812 LED shows the bridge colour from Story 2.1.
2. **PC → UART.** Given bytes from the PC, when they arrive on CDC, then the bridge only scans for `0x00` delimiters (never decodes), writes whole frames to the UART at 460800 with at most one app frame in flight plus one queued, drops and counts a run longer than `OCB_MAX_ENCODED_FRAME` without a delimiter, and discards a partial frame when DTR falls or CDC unmounts (AD-4); and F.8 passes in host tests.
3. **Heartbeat.** Given a hardware alarm every 20 ms, whatever the USB state, when a heartbeat is due, then a `HEARTBEAT` (0x40) COBS+CRC frame with its own rolling seq and body `bridge_fw_ver[3], flags` (bit0 = DTR) goes out at the next frame boundary, ahead of queued app frames; and F.7 passes: during a 512-frame burst with a fake clock every heartbeat gap is ≤ 27 ms.
4. **UART → PC.** Given bytes from the controller, when they arrive on UART, then they are copied to CDC as they come and dropped (and counted) when the CDC IN FIFO is full; no code path blocks.
5. **BOOTSEL touch.** Given the PC sets 1200 baud on the port, when the line coding changes, then the bridge reboots into BOOTSEL via pico-sdk `pico_usb_reset` with `PICO_ENABLE_USB_RESET_VIA_BAUD_RATE=1`, with no own `tud_cdc_line_coding_cb`; and any other baud rate the PC sets is ignored.
6. **Power-up (AD-20).** Given power-up, when the board starts, then RX has an internal pull-down, the bridge probes with TX idle-high pulses of about 5 ms every 500 ms until RX shows the controller, then keeps TX enabled, and returns to probing on a break or link loss; F-test for the probe state machine.

**Bench part: yes.** H.21 (identity, LED, BOOTSEL touch), H.22 (link and soak through the own bridge), H.23 (DTR edge in DIRECT through the real CDC DTR), H.1 (both boards powered, both orders; unpowered controller while the bridge probes). Retro action #27 applies: before the first bench result, the story records a review of the bench tools and H-row commands and a dry run of every new or changed tool against the simulated model (Task 9).

## Tasks / Subtasks

- [x] **Task 1: Layout, pure core library, purity gate (AC: 2, 3, 4, 6)**
  - [x] 1.1 `firmware/bridge/core/` — library `ocb_bridge_core`, pure C99 like `controller/core` and `controller/link`: no pico-sdk, no TinyUSB, no `hardware/*`, no clock reads, no allocation, no stdio. Depends only on `ocb_proto`. Files: `framer.c` (PC→UART), `heartbeat.c`, `probe.c`, `rxfwd.c` (UART→PC), `bridge.c` (the per-pass orchestration that ties them together, fed by the platform), headers in `include/ocb_bridge/`.
  - [x] 1.2 `firmware/bridge/platform/` — the thin glue: `main.c` (super-loop, TinyUSB `tud_task`, CDC read/write, UART pump, LED), `usb_descriptors.c`, `tusb_config.h`, the repeating hardware alarm. Only these files include pico-sdk / TinyUSB.
  - [x] 1.3 `firmware/tools/check-bridge-pure.sh` — same rules and fixtures shape as `check-link-pure.sh` (its banned list must equal the core/link gates' list, as F.18 checks for link), plus `tusb.h`, `class/cdc/*`, `pico/unique_id.h`; good fixture, bad fixtures, the real `bridge/core` tree; ctest `bridge_pure_check` and a CI step "Bridge core is pico-sdk and TinyUSB free (AD-2)".
  - [x] 1.4 `firmware/bridge/CMakeLists.txt`: the `bridge` executable links `ocb_bridge_core`, `ocb_hw`, `ocb_proto`, `ocb_version`, `pico_stdlib`, `pico_unique_id`, `tinyusb_device` (not `tinyusb_board`), `pico_usb_reset`; `PICO_ENABLE_USB_RESET_VIA_BAUD_RATE=1`, `PICO_ENABLE_USB_RESET_VIA_VENDOR_INTERFACE=0`; stdio off (as today).
- [x] **Task 2: USB identity (AC: 1, 5)**
  - [x] 2.1 Device descriptor: `bcdUSB 0x0200`, class from IAD (CDC), **VID `0x2E8A`, PID `0x000A`**, `bcdDevice` = firmware version (`OCB_VERSION_MAJOR << 8 | MINOR << 4 | PATCH` capped, or simply `0x0100`; record the choice), manufacturer `Bonooru`, product **`Bonooru Bridge`**, serial = the flash unique ID as 16 upper-case hex digits (`pico_get_unique_board_id_string`).
  - [x] 2.2 One CDC interface pair (`TUD_CDC_DESCRIPTOR`), 64-byte endpoints, no vendor reset interface, no MSC, no HID.
  - [x] 2.3 `pico_usb_reset` provides `tud_cdc_line_coding_cb` (1200 baud → BOOTSEL); the bridge defines **no** `tud_cdc_line_coding_cb` and ignores every other line coding (the UART stays at `OCB_LINK_BAUD`). A CI grep (`check-bridge-usb.sh` or a line in an existing step) fails if `tud_cdc_line_coding_cb` appears in `firmware/bridge/`.
  - [x] 2.4 LED: the Story 2.1 bridge colour and rhythm (blue 0,0,16, 1 Hz), unchanged; the blink runs from the super-loop's clock, never blocks.
- [x] **Task 3: Framer, PC → UART store-and-forward (AC: 2) — F.8**
  - [x] 3.1 `ocb_bridge_framer`: an assembly buffer of `OCB_MAX_ENCODED_FRAME` bytes, and **two frame slots** of `OCB_MAX_WIRE_FRAME` bytes: *in flight* (being pumped to the UART) and *queued* (complete, waiting). `feed(const uint8_t*, n) -> consumed`: scans for `0x00` only, never decodes; it stops consuming (returns early) when a frame completes and both slots are busy, so the rest stays in the caller's CDC staging buffer (USB backpressure is the flow control, AD-4).
  - [x] 3.2 A completed run of ≥ 1 byte becomes a frame (bytes + its `0x00`); an empty run (`00` with nothing before it) is forwarded as a lone `0x00` (D3) through the same slots.
  - [x] 3.3 A run longer than `OCB_MAX_ENCODED_FRAME` (262) without a delimiter: counted once (`overlong`), the rest of the run discarded up to and including the next `0x00` (the same rule as `ocb_rx`, Story 2.2 D12).
  - [x] 3.4 `discard_partial()`: called by the platform when DTR falls or CDC unmounts; drops the assembly buffer (counted `partial_discards`) and any overlong discard state; complete frames already in the slots still go out.
  - [x] 3.5 `next_chunk(max)`: the bytes the platform may write to the UART TX ring now (`ocb_uart_link_tx_space()`), always from the in-flight slot; when it empties, the next frame boundary is reached (Task 4 decides what goes next).
- [x] **Task 4: Heartbeat (AC: 3) — F.7**
  - [x] 4.1 The platform arms a **repeating hardware alarm** (pico-sdk `add_repeating_timer_us(-20000, …)` on the default alarm pool, or a raw `hardware_alarm`; record which) whose callback only increments an atomic `hb_ticks` counter. It runs whatever the USB state (mounted or not, DTR or not).
  - [x] 4.2 `ocb_bridge_heartbeat`: when `hb_ticks` moved since the last heartbeat, a heartbeat is **due**; several missed ticks give one heartbeat, never a burst. The frame: type `OCB_MSG_HEARTBEAT` (0x40), its own rolling u8 seq, body `bridge_fw_ver[3]` = `OCB_VERSION_MAJOR, MINOR, PATCH` and `flags` = `OCB_HB_FLAG_DTR` when CDC DTR is asserted; built with `ocb_frame_encode` (COBS + CRC + `00`).
  - [x] 4.3 At a frame boundary (in-flight slot empty) a due heartbeat goes first, **ahead of the queued app frame**; it never splits an app frame.
  - [x] 4.4 Heartbeats are sent only while the probe state machine has TX enabled (LINKED); while probing nothing is written (TX is high-Z) and due heartbeats are dropped, not queued (the first heartbeat after link-up goes at the next tick).
  - [x] 4.5 The compile-time check in `frame.c` already proves `20 + ceil(263 × 10 / 460.8) ≤ 27` ms; F.7 proves the code meets it: a 512-frame burst of maximum-size frames from the PC, the UART drained at exactly 460800 baud (10 bits per byte) on a fake clock, a tick every 20 ms: every gap between the first bytes of consecutive heartbeats on the UART is ≤ 27 ms, none is skipped, and app frames come out whole and in order.
- [x] **Task 5: UART → PC forwarder (AC: 4)**
  - [x] 5.1 `ocb_uart_link_rx_irq_enable(sink)`: the sink stores data-register values in an SPSC ring (IRQ → super-loop, drops and counts when full, like the controller's `rxring`); a value with FE, PE or OE set is not data: dropped and counted; BE is a **break**: counted and reported to the probe (Task 6), never forwarded.
  - [x] 5.2 `ocb_bridge_rxfwd`: each pass, take as many clean bytes as `tud_cdc_write_available()` allows, write them, `tud_cdc_write_flush()`; bytes that do not fit are **dropped and counted** (`cdc_drops`), never waited for (AD-4 "best effort"). The core decides how many to drop given the space; the platform does the TinyUSB calls.
- [x] **Task 6: Probe state machine, AD-20 (AC: 6) — F.39**
  - [x] 6.1 States `PROBING` and `LINKED`. At boot: `PROBING`, TX high-Z (`ocb_uart_link_init` already leaves it high-Z with the RX pull-down).
  - [x] 6.2 `PROBING`: every `OCB_BRIDGE_PROBE_PERIOD_US` (500 000) drive TX idle-high for `OCB_BRIDGE_PROBE_PULSE_US` (5 000), then high-Z (duty 1 %). During a pulse (and between pulses), if RX has been continuously high for `OCB_BRIDGE_RX_HIGH_US` (1 000), or a valid frame was received, → `LINKED` and TX stays enabled (if the pulse is still on, it simply never ends).
  - [x] 6.3 `LINKED`: TX enabled, heartbeats and app frames flow. A **break** (BE seen) or RX continuously low for more than `OCB_BRIDGE_RX_LOW_US` (2 000; record the value) → link loss → `PROBING`, TX high-Z at once, the framer's in-flight frame is abandoned (it would be cut anyway), queued frames are kept.
  - [x] 6.4 Pure C with a fake clock and fake RX level; the platform samples `ocb_uart_link_rx_level()` once per pass and calls `ocb_uart_link_tx_enable()` on state changes. Bridge-only constants live in `bridge/core/include/ocb_bridge/config.h`, not in `consts.h` (no wire value).
  - [x] 6.5 F.39: boot → pulses exactly 5 ms every 500 ms (duty ≤ 1 %), no pulse ever longer than 5 ms while RX stays low; RX high for 999 µs → still probing, 1000 µs → linked; a valid frame during probing → linked; linked → break → probing at once; linked → RX low 2000 µs stays, 2001 µs → probing; repeated link-up/loss cycles; heartbeats only in LINKED (with Task 4).
- [x] **Task 7: Platform super-loop (AC: 1–6)**
  - [x] 7.1 One core (core 0), non-blocking super-loop: `tud_task()`; CDC read into a 64-byte staging buffer kept across passes; `framer_feed`; heartbeat/boundary decision; `ocb_uart_link_tx_pump()`; RX ring → `rxfwd`; probe step; LED. No `sleep_*`, no busy wait, no `tud_cdc_write` that waits.
  - [x] 7.2 DTR edges and unmount: `tud_cdc_line_state_cb` (allowed; it is not the line-coding callback) and `tud_umount_cb` call `framer_discard_partial()`; the heartbeat DTR flag follows `tud_cdc_connected()`.
  - [x] 7.3 Counters in RAM (`overlong`, `partial_discards`, `cdc_drops`, `rx_err_bytes`, `breaks`, `rx_ring_drops`, `hb_sent`, `probe_cycles`); no way to read them out in this story (D11).
  - [x] 7.4 0 warnings in every preset that builds the bridge; the pin grep (AD-20) still passes (only GP0, GP1, GP16).
- [x] **Task 8: Host tests and docs**
  - [x] 8.1 `firmware/tests/test_bridge_framer.c` (F.8), `test_bridge_heartbeat.c` (F.7), `test_bridge_probe.c` (F.39), `test_bridge_rxfwd.c` (F.41), `bridge_pure_check` (F.40) in `tests/CMakeLists.txt`, ASan and UBSan on like the others.
  - [x] 8.2 F.8 cases: whole frames only, byte by byte and in random chunks; one in flight + one queued (a third complete frame is not consumed until a slot frees); empty run → lone `00`; overlong 263..5000 bytes counted once and the next frame delivered; DTR fall / unmount mid-frame discards only the partial; the framer never decodes (a frame with a bad CRC is forwarded unchanged); a 262-byte run is a frame, 263 is overlong.
  - [x] 8.3 F.41: clean bytes forwarded in order; FE/PE/OE values dropped and counted; BE counted as a break and not forwarded; CDC space 0 / partial: the excess dropped and counted, never blocked; ring overflow counted.
  - [x] 8.4 `docs/test-plan.md`: fill F.7 and F.8 (real test names), add F.39, F.40, F.41, F.42 (bench dry run), H.21–H.23, and the bridge rig block (labelled cable, retro action 6; 2wiCC_Comms v1.1 fallback).
  - [x] 8.5 `docs/link-protocol.md`: a "Rules the bridge applies (Story 3.1)" section (empty run forwarded, heartbeat dropped while probing, probe values, link-loss rule, RX error bytes, counters). No wire value changes; vectors unchanged.
  - [x] 8.6 `firmware/bridge/README.md`: what the bridge does now.
- [x] **Task 9: Bench tools, review and dry run (retro action #27) — F.42**
  - [x] 9.1 `firmware/tools/link_probe.py`: a global `--bridge {emulated,own}` (default `emulated`, today's behaviour). With `own`: never send emulated heartbeats (whatever `--heartbeat` says, with a warning), drive the real CDC DTR line (`serial.dtr`) where the emulated DTR flag was used (`direct`'s DTR step), and expect `INFO.bridge_ver` = `--expect-bridge-ver X.Y.Z` (default: the version `git describe` gives, same rule as the firmware) instead of `0.0.1`.
  - [x] 9.2 A host model of the own bridge for the dry run: `ocb_bridge_sim` (MODULE library built from `bridge/core` + a shim, like `ocb_link_sim`), loaded with ctypes in `link_probe_selftest`: probe → fake CDC → bridge core → fake UART at 460800 → `ocb_link_sim` (the real controller link) and back, fake clock, the bridge's 20 ms ticks.
  - [x] 9.3 F.42 dry run: `linkup --seconds 20 --expect-ready no`, `soak --cycles 40 --events 500` (with and without impairments), `direct` (DTR step through the modelled DTR), `breaks`-like link loss (the model drops the controller's TX: the bridge sees a break, re-probes, the controller trips and recovers) all PASS through the own-bridge model with `--bridge own`; the same commands with the bridge's heartbeat removed in the model must FAIL (red proof that the probe no longer relies on its own heartbeat).
  - [x] 9.4 Review of the bench tools and of every H-row command the session uses (H.21–H.23, H.1), recorded in the Dev Agent Record **before** the first bench result (retro action #27): what each command does, what it needs from the owner, expected output.

### Review Findings

Code review 2026-11-24 (gds-code-review: Blind Hunter, Edge Case Hunter, Acceptance Auditor; the Edge Case Hunter verified its main findings with C experiments against the real `bridge/core` sources). The framer, the heartbeat frame, the descriptors, the RX ring barriers and the 1200-baud path were found correct. Owner's rule: IMPORTANT fixed, MINOR fixed when cheap, else recorded.

- [x] [Review][Patch][IMPORTANT] **I-1 A link loss kept TX driven and sent the cut frame's leftovers** (all three; verified) — `enter_probing` started the next pulse in the same step, so TX never went high-Z and the UART ring was never emptied. Fixed: a loss returns TX high-Z at once (the platform's `tx_enable(false)` empties the ring) and the next pulse waits `OCB_BRIDGE_PROBE_RELEASE_US` (10 ms) so the controller sees a break and releases. F.39 checks TX and the 10 ms. [probe.c, config.h]
- [x] [Review][Patch][IMPORTANT] **I-2 No resync after a cut** (blind) — after a relink the next frame landed on the controller's partial run. Fixed: every link-up starts with a lone `0x00`. [bridge.c]
- [x] [Review][Patch][IMPORTANT] **I-3 A break and a valid frame in the same pass left the bridge linked** (edge; verified) — fixed: the break wins. [probe.c]
- [x] [Review][Patch][IMPORTANT] **I-4 The bridge's view of the controller's run was not reset on an error character** (edge; verified) — fixed: any FE/PE/OE/BE value resets the splitter, as the controller's receiver does. [bridge.c]
- [x] [Review][Patch][IMPORTANT] **I-5 DTR fall cut the wrong run** (auditor+edge+blind) — the staging buffer and the framer's run were dropped at once, though they hold what the PC sent before the fall, and TinyUSB's RX FIFO kept the real tail. Fixed: the platform keeps forwarding everything received before the fall and cuts the partial run once all of it is fed; a reopen first lets the app's opening `0x00` end it. Unmount still drops at once (TinyUSB resets its FIFOs). [platform/main.c, bridge.h/.c]
- [x] [Review][Patch][IMPORTANT] **I-6 USB suspend read as DTR 0; a DTR fall shorter than 20 ms was invisible** (edge) — DTR now comes from `tud_cdc_get_line_state()`; a fall is latched into the next heartbeat. [platform/main.c, bridge.c]
- [x] [Review][Patch][IMPORTANT] **I-7 The UART → PC ring dropped whatever did not fit in one pass** (blind) — fixed: leftovers wait; a byte is dropped only when the 512-byte ring is full. [rxfwd.c]
- [x] [Review][Patch][IMPORTANT] **I-8 The RX-low link-loss rule aliased on data bits** (blind+auditor) — the pad is sampled once per pass; fixed: received characters reset the low timer. [probe.c]
- [x] [Review][Patch][IMPORTANT] **I-9 Heartbeat margin 0.25 ms, no build guard** (blind+edge) — `OCB_BRIDGE_UART_AHEAD_MAX` 8 (worst delay about 6.6 ms) and a compile-time check in `bridge.c` (red: 64 fails to build). F.7 still 17.4 to 23.1 ms. [config.h, bridge.c]
- [x] [Review][Patch][IMPORTANT] **I-10 Bench tool gaps (retro #27 class)** (auditor+blind) — `bridgedtr` waited 1 s for console_ready (the P2 handshake takes longer) → now 10 s with the hint, like `direct`; it now waits 80 ms after raising DTR before STOP (the controller learns DTR from the next heartbeats). The dry run now also goes through `main()` (guards, `--expect-bridge-ver`, `--heartbeat` warning). [link_probe.py, test_link_probe.py]
- [x] [Review][Patch][IMPORTANT] **I-11 The Python model was kinder than the hardware** (all three) — the bridge's RX now reads low while controller characters are on the wire, a break and the last data arrive in the same pass; red control "bridge heartbeat removed" added for `bridgedtr`; the link-loss case asserts the controller's watchdog trip. [test_link_probe.py]
- [x] [Review][Patch][MINOR] **M-1 Purity gate CMake rule missed `tinyusb*`, `ocb_link`, `ocb_core`, `cmsis`** — added, with two bad fixtures (16 now). [check-bridge-pure.sh]
- [x] [Review][Patch][MINOR] **M-2 No ELF check for the 1200-baud touch** — CI step: `tud_cdc_line_coding_cb` from `pico_usb_reset/usb_reset.c` in the fw-release and fw-debug bridge ELFs. [firmware.yml]
- [ ] [Review][Minor][recorded] **M-3 `bridge_ver` 0.0.0 proves little while there is no `v*` tag** — H.22's real proof is link_ok 1 from the bridge's heartbeat; the version check becomes meaningful with the first tag.
- [ ] [Review][Minor][recorded] **M-4 A queued app frame survives a long probing period** and goes out after relink; the resync `0x00` now precedes it, and the controller refuses it if the session or link state no longer fits (`NAK NO_LINK` / `VERSION`). Holding app data until the first heartbeat is a possible later refinement.
- [ ] [Review][Minor][recorded] **M-5 "One in flight plus one queued" counts bridge slots**: up to 8 + 32 bytes of the previous frame may still be in the UART ring and FIFO.
- [ ] [Review][Minor][recorded] **M-6 F.7's skip check tolerates one heartbeat at the very end of a burst** (`hbs + 1 >= expected`): the gap check covers every heartbeat but the last.
- [ ] [Review][Minor][recorded] **M-7 VID:PID 2E8A:000A is the generic Pico pair** (AD-13, Q1): discovery confirms by product string, as designed.
- [ ] [Review][Minor][recorded] **M-8 `fw_version_triple` reads the checkout's version, not the flashed binary's** — the bench builds and flashes from the same checkout; pass `--expect-bridge-ver` otherwise.

## Dev Notes

### What exists today (read before writing)

- `firmware/bridge/main.c` (Story 2.1) only blinks the LED blue at 1 Hz; `CMakeLists.txt` links `ocb_hw ocb_proto ocb_version`, stdio off. Keep the LED code and colour.
- `firmware/common/hw/`: `uart_link.c` (register-level UART0 at `OCB_LINK_BAUD`, RX pull-down, TX high-Z until `ocb_uart_link_tx_enable(true)`, IRQ RX sink with the data-register error bits, 128-byte software TX ring, `tx_write` all-or-nothing, `tx_pump`, `tx_idle`, `rx_level`), `led.c`, `pins.h` (GP0, GP1, GP16 only). Reuse as is; the bridge is its second user.
- `firmware/common/proto/` (`ocb_frame_encode`, `ocb_rx` splitter for "a valid frame was received" in the probe, consts incl. `OCB_MSG_HEARTBEAT`, `OCB_HB_FLAG_DTR`, `OCB_MAX_ENCODED_FRAME` 262, `OCB_MAX_WIRE_FRAME` 263, `OCB_HEARTBEAT_PERIOD_MS` 20, `OCB_HEARTBEAT_MAX_GAP_MS` 27, `OCB_LINK_BAUD` 460800, and the heartbeat-gap compile-time check in `frame.c`).
- The controller (Stories 2.4–2.7) is the reference for structure: pure `core/` and `link/` libraries, purity gates (`check-*-pure.sh`, F.13/F.18/F.30), the RX SPSC ring (`link/rxring.c`), the TX gate (`link/txgate.c`, the controller side of AD-20), the watchdog (`link/watchdog.c`: armed by the first valid HEARTBEAT, 80 ms, break while armed = loss, DTR edge in DIRECT), and `ocb_link_sim` for the Python dry runs (`tests/link_sim_shim.c`).
- `firmware/tools/link_probe.py` (Stories 2.6/2.7): emulates the bridge heartbeat itself (`bridge_fw_ver` 0.0.1, DTR in flags) because there was no bridge firmware; its `selftest` (F.19, F.20, F.23) runs against `ocb_link_sim` through a 2wiCC_Comms model. Story 3.1 adds the own-bridge path (Task 9) without breaking the 2.x commands.
- pico-sdk 2.3.1 (`firmware/external/pico-sdk`): `src/rp2_common/pico_usb_reset/` — linking `pico_usb_reset` with `PICO_ENABLE_USB_RESET_VIA_BAUD_RATE=1` defines `tud_cdc_line_coding_cb` (1200 → `rom_reset_usb_boot`); `PICO_ENABLE_USB_RESET_VIA_VENDOR_INTERFACE` must be 0 (no vendor interface in our descriptors). Checked 2026-11-24: the AC's names exist in this SDK.

### Decisions (made autonomously, 2026-11-24; the owner can overrule any of them)

- **D1. Bridge split like the controller**: `bridge/core` pure C (`ocb_bridge_core`, testable on the host and loadable in Python), `bridge/platform` glue. A purity gate keeps the split honest (F.40).
- **D2. Two frame slots plus an assembly buffer** (≈ 790 bytes of RAM). The UART TX ring (128 bytes) is smaller than a maximum frame, so "one in flight" is the bridge's own slot being pumped into the ring; "one queued" is the second slot. A third frame stays in the CDC staging buffer / TinyUSB FIFO (backpressure).
- **D3. An empty run is forwarded as a lone `0x00`.** The app writes one at session open and after wire faults (Story 3.3) to end any partial run on the controller; the bridge only sends whole frames, so the controller's receiver could hold a partial run only after a wire fault, and a forwarded `0x00` costs one byte and an `empty` count on the controller.
- **D4. Missed heartbeat ticks give one heartbeat** (no burst), like the controller's rule for STATUS and the probe's emulation (Story 2.7 D15).
- **D5. Heartbeat body**: `bridge_fw_ver` = the firmware's `OCB_VERSION_MAJOR/MINOR/PATCH` (`0.0.0` while there is no `v*` tag, which the app reads as "unknown": Story 3.3 D5 — the controller still arms on it, AD-5 only needs a valid HEARTBEAT); `flags` bit 0 = `tud_cdc_connected()` (DTR); bits 1–7 written 0.
- **D6. RX error characters are not forwarded**: FE/PE/OE bytes are garbage, BE is a break (the probe's link-loss signal); both counted. The app's splitter would drop them as CRC errors anyway.
- **D7. Probe values**: pulse 5 ms every 500 ms (AD-20: "about 5 ms every 500 ms, ≤ 1 %"); link-up when RX is high for 1 ms continuously (the controller enables its TX 1 ms after it sees the pulse, `OCB_LINK_TX_IDLE_HIGH_US`, so a 5 ms pulse leaves ~4 ms to see it) or on a valid frame; link loss on a break or RX low for > 2 ms (longer than a few characters of `00` at 460800, so data never trips it). H.1 measures the result.
- **D8. No heartbeat while probing**: TX is high-Z, so nothing can be written; due ticks are dropped, not queued, so no stale heartbeat leaves at link-up.
- **D9. UART → PC drops instead of waiting** (AD-4: no bridge code path may block); the app's go-back-N repairs lost replies.
- **D10. One core.** The bridge needs no second core: USB, UART and the probe all run in one non-blocking super-loop; the heartbeat comes from a hardware alarm IRQ that only counts.
- **D11. Counters are not exported yet.** There is no bridge → app message for them (no wire change in this story); they are visible to host tests and a debugger. A later story can add a debug path if the bench needs it.
- **D12. Bench tool change, not a new tool**: `link_probe.py --bridge own` (Task 9) so the proven 2.x commands keep working; default `emulated` so 2.x bench rows stay valid.
- **D13. Dry-run model = the bridge's own C core** via ctypes (`ocb_bridge_sim`), between the probe and the real controller link (`ocb_link_sim`): the dry run exercises the real bridge logic, not a Python re-implementation (lesson of F.20/F.23).

### Testing requirements (rows for `docs/test-plan.md`)

| ID | Test | Pass criterion | Binds |
|---|---|---|---|
| F.7 | Heartbeat gap during a 512-frame burst (Story 3.1, `test_bridge_heartbeat`): maximum-size frames from the PC, UART drained at 460800 baud (10 bits/byte) on a fake clock, a tick every 20 ms: gaps between heartbeat starts ≤ 27 ms, none skipped, app frames whole and in order; missed ticks give one heartbeat; none while probing | every gap ≤ 27 ms | AD-4 |
| F.8 | Bridge framer (Story 3.1, `test_bridge_framer`): whole frames only (byte by byte, random chunks), one in flight + one queued, the third not consumed, empty run → lone `00`, 262-byte run a frame and 263 overlong (counted once, next frame delivered), DTR fall / unmount discards only the partial, never decodes (bad CRC forwarded unchanged) | matches AD-4 | AD-4 |
| F.39 | Bridge probe state machine (`test_bridge_probe`), fake clock: 5 ms pulses every 500 ms (≤ 1 %), RX high 999 µs / 1000 µs, valid frame → linked, break → probing at once, RX low 2000 / 2001 µs, cycles; heartbeats only when linked | matches AD-20 | AD-20, C5 |
| F.40 | Bridge purity gate (`bridge_pure_check`): `check-bridge-pure.sh` flags every bad fixture (SDK, TinyUSB, `hardware/*`, stdio, stdlib, clock read, allocation, `pico/unique_id.h`), passes the good fixture and `bridge/core`, banned list equal to the link gate's; no `tud_cdc_line_coding_cb` in `firmware/bridge/` | all pass | AD-2, AD-13 |
| F.41 | UART → PC forwarder (`test_bridge_rxfwd`): clean bytes in order; FE/PE/OE dropped and counted; BE a break, not forwarded; CDC space short → excess dropped and counted, never blocks; ring overflow counted | all pass | AD-4 |
| F.42 | Bench dry run with the own bridge (`link_probe_selftest`): `linkup`, `soak` (clean and impaired), `direct` (DTR through the modelled CDC DTR) and a link loss through `ocb_bridge_sim` (the bridge core) and `ocb_link_sim` with `--bridge own` PASS; with the model's heartbeat removed they FAIL | all as stated | retro #27 |

H rows (bench, owner present):

| ID | Steps (draft; finalised by Task 9.4) | Pass |
|---|---|---|
| H.21 | Flash `bridge.uf2` on the bridge Zero (first time from 2wiCC_Comms: BOOT button + plug, the owner); `system_profiler SPUSBDataType` / `ioreg`: VID:PID, product, serial = flash unique ID (from `picotool info -d` in BOOTSEL before flashing); LED blue 1 Hz (the owner looks); open the port at 1200 baud → `RPI-RP2` mounts; flash again with `picotool load`; open at 9600 / 115200 / 460800 → no reboot | all as stated |
| H.22 | Controller fw-debug wired as in H.4; `link_probe.py --bridge own --port "$P" linkup --seconds 20 --expect-ready no` (link_ok 1 without probe heartbeats, `bridge_ver` = the bridge's version); then `--seed 1 soak --cycles 40 --events 500 --corrupt 0.02 --drop 0.02 --garbage` and the clean soak; then `linkup --seconds 300` | PASS each; `watchdog_trips` and `abort_count` unchanged, `link_ok` 1 |
| H.23 | DTR edge through the real CDC DTR: `p2_probe.py handshake --hold 120 &` (controller USB on the Mac, console_ready 1) and `link_probe.py --bridge own --port "$P" direct` | PASS: DIRECT app timeout and the DTR-edge abort as in H.11 |
| H.1 | Both boards powered: bridge first then controller, and the reverse, 5 times each (`linkup --seconds 5` per run, the owner plugs); controller unpowered, bridge powered: meter on the controller's 3V3 and GP1 (pulses) | link-up in either order within 1 s; 3V3 ≈ 0 V; GP1 pulses visible or near 0 V on a meter; record whether 1 kΩ resistors are needed |

### Architecture compliance

- AD-2: bridge core pure; wire constants only in `ocb_proto`. AD-4: everything in the AC, no blocking path. AD-13: descriptors, 1200-baud touch via `pico_usb_reset`, no own `tud_cdc_line_coding_cb`. AD-20: pins, RX pull-down, the bridge leads with probe pulses. AD-1: pico-sdk 2.3.1 / TinyUSB 0.18.0 as pinned.
- No wire change: no message, constant or `PROTO_VERSION` change; vectors unchanged. Bridge-only tuning values in `ocb_bridge/config.h`.
- If an AC turns out not to match the hardware or the controller's documented behaviour, stop and ask the owner (epic instructions).

### File structure

```text
firmware/bridge/CMakeLists.txt                 UPDATE
firmware/bridge/README.md                      UPDATE
firmware/bridge/main.c                         MOVE → platform/main.c (rewritten; LED kept)
firmware/bridge/platform/{main.c,usb_descriptors.c,tusb_config.h}            NEW
firmware/bridge/core/{CMakeLists.txt,framer.c,heartbeat.c,probe.c,rxfwd.c,bridge.c}  NEW
firmware/bridge/core/include/ocb_bridge/{framer.h,heartbeat.h,probe.h,rxfwd.h,bridge.h,config.h}  NEW
firmware/tools/check-bridge-pure.sh            NEW (+ fixtures under firmware/tests/fixtures/bridge_pure/)
firmware/tests/test_bridge_{framer,heartbeat,probe,rxfwd}.c   NEW
firmware/tests/bridge_sim_shim.c               NEW (ocb_bridge_sim)
firmware/tests/CMakeLists.txt                  UPDATE
firmware/tools/link_probe.py                   UPDATE (--bridge own, own-bridge model in selftest)
firmware/tests/test_link_probe.py              UPDATE (F.42)
.github/workflows/firmware.yml                 UPDATE (bridge purity gate)
docs/test-plan.md, docs/link-protocol.md       UPDATE
```

### Library / framework requirements

- pico-sdk 2.3.1, TinyUSB 0.18.0 (pinned, AD-1), Arm GNU 15.2.Rel1, picotool 2.3.1 (`firmware/.tools/`). Build: `cmake --preset fw-release && cmake --build --preset fw-release` with `OCB_PICOTOOL_DIR=firmware/.tools/picotool-2.3.1 OCB_ARM_TOOLCHAIN=firmware/.tools/arm-gnu-toolchain-15.2.Rel1` (test-plan line 776 pattern).
- TinyUSB device CDC: `tud_cdc_available/read/write_available/write/write_flush/connected`, `tud_cdc_line_state_cb`, `tud_mount_cb/tud_umount_cb`. Check TinyUSB 0.18.0's CDC API in `lib/tinyusb/src/class/cdc/cdc_device.h` before use (do not rely on memory).
- Python side: the existing `link_probe.py` conventions (stdlib only for the self-test, `python3 -I`; pyserial only at the bench).

### Previous story intelligence

- Epic 2 retro §3.1: five of six bench failures were the test tools. Task 9 is that lesson: the tool change is reviewed and dry-run against the real C cores before the bench, and the red proof shows the probe really depends on the bridge's heartbeat.
- Story 2.6/2.7 bench: macOS "Allow accessories to connect" must be "Always"; the port name changes on replug (`ls /dev/cu.usbmodem*`); a replug bounces (D42); macOS USB removal latency ≈ 110 ms.
- Story 3.2/3.3 reviews: tests that pass for the wrong reason were the main finding class; every new gate gets a red proof; numbers are checked against logs before being written.
- Retro action 6: a labelled known-good USB cable for flashing (BOOTSEL trouble on 2026-11-23) — the owner is reminded before the bench; 2wiCC_Comms v1.1 stays at hand as the fallback (retro action #28).

### Git intelligence

Last merges: PR #31 (3.2) and PR #32 (3.3, merged by the agent on the owner's request). Firmware CI: `firmware.yml` (ubuntu), all presets 0 warnings, ctest host-tests, gate steps.

### Project context rules

No `project-context.md`. English in the repo, Italian in chat; never commit `_bmad/` or skills; branch `story/3-1-bridge-firmware-frame-aware-pipe-and-heartbeat`; the owner merges. At the bench: physical actions asked one at a time with the exact command or step and what to observe; everything the agent can do itself (picotool, 1200-baud reset, ioreg/system_profiler, the tools) it does without asking.

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 3.1]
- [Source: ARCHITECTURE-SPINE.md#AD-1, AD-2, AD-4, AD-5, AD-13, AD-20]
- [Source: docs/link-protocol.md#Framing, Watchdog, Rules the controller link applies (Story 2.6) "Who drives first", (Story 2.7)]
- [Source: docs/test-plan.md#H.1, H.4–H.12, F.7, F.8, F.18–F.23]
- [Source: _bmad-output/implementation-artifacts/epic-2-retro-2026-11-24.md#3.1, #6, #7]
- [Source: firmware/external/pico-sdk/src/rp2_common/pico_usb_reset/]

## Dev Agent Record

### Agent Model Used

Claude Opus 5.5 (`claude-opus-5-5`), gds-dev-story, 2026-11-24.

### Debug Log References

- Host tests (`ctest --preset host-tests`): 44/44 pass (ASan + UBSan as configured), new: `test_bridge_framer` (F.8), `test_bridge_heartbeat` (F.7), `test_bridge_probe` (F.39), `test_bridge_rxfwd` (F.41), `bridge_pure_check` (F.40); `link_probe_selftest` now also runs class `OwnBridge` (F.42, 7 tests).
- F.7 output: `F.7: 3394 heartbeats in 23 bursts of 512 maximum frames, gaps 17.362 to 23.069 ms (limit 27 ms)`.
- F.7 model bug found and fixed while writing it: the first model quantised every character to the 5 µs step (25 µs instead of 21.7 µs); heartbeat + 3 maximum frames then made exactly 800 characters = 20.000 ms, so every gap read 20.000 ms and the worst case was never exercised. Exact character times (with the FIFO arrival time) and 23 alarm phases fixed it; a guard now requires a max gap above 20.5 ms (no vacuous pass).
- Red proofs (each reverted, `git status` clean afterwards): heartbeat not ahead of the queued frame → `FAIL: no heartbeat skipped: 1 over 2922.5 ms`; `OCB_BRIDGE_UART_AHEAD_MAX` 128 instead of 16 → `FAIL: max heartbeat gap 28.777 ms > 27 ms`. F.42: the bridge's heartbeat removed → `bridgeup` FAIL on link_ok; wrong `--expect-bridge-ver` → FAIL; DTR not reaching the bridge → `bridgedtr` FAIL. F.40: the line-coding check sees a definition in a temporary file.
- F.42 sample (`bridgeup --seconds 2` through the model): link_ok 1 at 53 ms, INFO `bridge_ver` 0.3.1, hold 2 s with 40 STATUS, 103 bridge heartbeats in 2.06 s, one probe pulse, one link-up.
- Firmware builds: fw-release, fw-debug, fw-debug-o3, fw-debug-timeline, fw-debug-hang0, fw-debug-hang1, fw-release-xip: 0 warnings, 0 errors. `bridge.elf`: `tud_cdc_line_coding_cb` is the weak symbol from `pico_usb_reset/usb_reset.c:147`; `check-pins.sh` ok; `check-bridge-pure.sh` ok (12 files).

### Bench tool and H-row review (Task 9.4, retro action #27; written before any bench result)

Tools: `firmware/tools/link_probe.py` (changed: `--bridge own`, `bridgeup`, `bridgedtr`), `firmware/tools/p2_probe.py` (unchanged since Story 2.5, used by H.23 for console_ready), picotool 2.3.1 (`firmware/.tools/picotool-2.3.1/picotool/picotool`), a bench venv with pyserial 3.5 and hidapi in the session scratchpad. Dry run: F.42 (9 tests, through `main()` too) against the bridge's real core and the controller's real link.

- **H.21.** Owner, once: hold BOOT on the bridge Zero and plug it in with the labelled cable (it runs 2wiCC_Comms, which has no reset path). Agent: `picotool info -d` (flash ID), `picotool load -x firmware/build/fw-release/bridge/bridge.uf2`, `system_profiler SPUSBDataType` and `ioreg -p IOUSB -l` (VID, PID, product, manufacturer, serial = flash ID), `ls /dev/cu.usbmodem*`. Owner: look at the LED (blue, ~1 Hz). Agent: open at 1200 baud with pyserial → RPI-RP2 within 2 s (`picotool info` sees it), reflash; open at 9600 / 115200 / 460800 → the port stays. Risk: the port name changes after every reboot; re-list it.
- **H.22.** Controller Zero on fw-debug: it has no touch, so flashing it needs BOOT held by the owner (one action), then plugged into the Mac; wiring of H.4 unchanged. Agent: `bridgeup --seconds 20 --expect-ready no`; `--seed 1 soak --cycles 40 --events 500 --corrupt 0.02 --drop 0.02 --garbage`; clean soak; `--no-pace` soak; `bridgeup --seconds 300 --expect-ready no`. Expected: PASS each; `bridge_ver` 0.0.0 (no tag, M-3). Needs nothing from the owner.
- **H.23.** Needs console_ready 1: the controller's USB-C on the Mac (as in H.22) and `p2_probe.py handshake --hold 60` in the same shell line; `bridgedtr` waits up to 10 s for console_ready. If macOS asks for Input Monitoring, the owner allows it once (Story 2.3 H.14). Expected PASS: abort within 200 ms of the DTR drop, NAK ABORTED, recovery.
- **H.1 (Story 3.1 part).** Owner: power orders (unplug / replug USB-C of one board, then the other), 5 times each way; agent runs `bridgeup --seconds 5 --expect-ready no` after each (port re-listed). Owner, with the controller unplugged and the bridge plugged: multimeter on the controller's 3V3 and GP1 (pulses of 5 ms every 500 ms read as ≈ 0.03 V average on a DC meter). The bridge-unpowered measurement belongs to Story 3.5's AC.

### Bench results (2026-11-24; numbers checked against the session logs)

Rig: bridge fw-release `69f4d2c` (version 0.0.0), controller fw-debug from Epic 2 (not reflashed, controller code unchanged), labelled cable, macOS (Darwin 27.2), DT-830B. Full rows in `docs/test-plan.md` (H.21–H.23, H.1, own-bridge rig block).

- **H.21 PASS.** Flash id `0x53032847189C2E9C`; ioreg 2E8A:000A, bcdDevice 0x0100, `Bonooru` / `Bonooru Bridge`, serial = flash id; LED blue about 1 Hz (owner); 1200 baud → RPI-RP2 in 1.60 s (Python start included), reflashed; 9600 / 115200 / 460800 → port stays.
- **H.22 PASS.** 20 s hold: 400 STATUS, `link_ok` 1 from the bridge's heartbeat at 0.014 s, `bridge_ver` 0.0.0. Impaired soak 20000 events once and in order (resends 41, rewinds 1814, transport NAKs 1814, semantic 0); clean and `--no-pace` soaks 0 NAKs; 300 s hold 5999 STATUS; `watchdog_trips` 0 throughout, `dropped_bytes` 1 = the boot break.
- **H.23 PASS.** DTR drop in DIRECT → abort seen 50 ms later, no `LINK_LOST`, `NAK ABORTED`, recovery; the same with `p2_probe.py handshake --hold 60` alongside (p2_probe PASS, 38164 reports).
- **H.1 (Story 3.1 part) PASS.** Bridge first: link_ok 1 after 1, 108, 0, 25, 0 ms from the controller's enumeration; controller first: 39, 43, 43, 36, 34 ms from the bridge's port appearing; `bridgeup --seconds 5` PASS 10/10; no deadlock. Controller unplugged, bridge probing: controller 3V3 0 V, GP1 0.08–0.14 V fluctuating (5 ms pulses read by a slow meter); no back-powering, 1 kΩ series resistors not needed. The bridge-unpowered GP1 measurement stays with Story 3.5.
- **Deviation from the Task 9.4 review (H-row command, not an AC).** The review assumed `console_ready` 0 with the controller on the Mac; macOS 27.2 completes the Pro Controller handshake by itself, so STATUS shows `console_ready` 1 with no probe running. The first `bridgeup --expect-ready no` printed one FAIL (`SET_STATE ACK, START BAD_STATE`, the correct replies for a ready console); H.22 and H.1 now use `--expect-ready auto`, recorded in the test plan. H.23 needs no `p2_probe.py` on this host.
- Link-up times are measured from USB enumeration (scratch helper `powerup_timer.py`, not in the repo), not from power-up: the bridge probes and the controller's UART run before either enumerates.

### Decisions made during implementation (D1 to D13 followed; additions below)

- **D14. At most 16 bytes ahead in the UART software ring** (`OCB_BRIDGE_UART_AHEAD_MAX`). With the 128-byte ring and the 32-byte FIFO full, a due heartbeat could wait one maximum frame (5.7 ms) plus 160 bytes (3.5 ms): a gap up to about 29 ms, over 27. With 16 + 32 bytes ahead the bound is about 26.8 ms; F.7 measures 23.1 ms and fails at 28.8 ms with 128.
- **D15. `bcdDevice` fixed at 0x0100**; the firmware version reaches the app through the heartbeat and INFO.bridge_ver (AD-13).
- **D16. New bench commands instead of changing the 2.x ones**: `--bridge own` refuses `linkup`, `silence`, `direct`, `playing`, `breaks` (they drive the emulated heartbeat) and adds `bridgeup` (H.22) and `bridgedtr` (H.23); `--bridge emulated` (default) keeps every 2.x command as it was. `--expect-bridge-ver` defaults to the `git describe` version (same rule as the firmware build). `Port.set_dtr()` drives the real DTR line.
- **D17. DTR and unmount by polling** in the super-loop (`tud_cdc_connected()` and `tud_mounted()` each pass) instead of `tud_cdc_line_state_cb` / `tud_umount_cb`: same effect, one place, and the staging buffer is dropped with the partial frame.
- **D18. The RX ring from the IRQ to the loop is local to the bridge platform** (1024 entries, drops counted), not the controller's `ocb_rxring` (the bridge must not depend on the controller's libraries; F.40 forbids `ocb_link` / `ocb_core`).
- **D19. The heartbeat alarm is `add_repeating_timer_us(-20000)`** on the SDK's default alarm pool (a hardware alarm IRQ); its callback only increments a counter.

### Completion Notes List

- AC1, AC5: CDC ACM with 2E8A:000A, "Bonooru Bridge", serial from the flash unique ID; 1200-baud touch from `pico_usb_reset`, no own line-coding callback (F.40); LED blue 1 Hz kept. Bench: H.21.
- AC2: framer with one frame in flight plus one queued, overlong and partial-run rules (F.8). AC3: heartbeat from a 20 ms hardware alarm, ahead of the queued frame, gaps ≤ 27 ms (F.7). AC4: UART → PC best effort (F.41). AC6: probe state machine (F.39) and both power-up orders in the dry run (F.42). Bench: H.22, H.23, H.1.
- Task 9.4: bench tool and H-row review recorded above, before any bench result.
- Bench 2026-11-24: H.21, H.22, H.23 and H.1 (Story 3.1 part) PASS; results above and in `docs/test-plan.md`.

### File List

- `firmware/CMakeLists.txt` (modified: `bridge/core`)
- `firmware/bridge/CMakeLists.txt`, `firmware/bridge/README.md` (modified)
- `firmware/bridge/main.c` → `firmware/bridge/platform/main.c` (moved, rewritten)
- `firmware/bridge/platform/usb_descriptors.c`, `firmware/bridge/platform/tusb_config.h` (new)
- `firmware/bridge/core/CMakeLists.txt`, `framer.c`, `heartbeat.c`, `probe.c`, `rxfwd.c`, `bridge.c` (new)
- `firmware/bridge/core/include/ocb_bridge/config.h`, `framer.h`, `heartbeat.h`, `probe.h`, `rxfwd.h`, `bridge.h` (new)
- `firmware/tools/check-bridge-pure.sh` (new)
- `firmware/tools/link_probe.py` (modified: `--bridge`, `--expect-bridge-ver`, `bridgeup`, `bridgedtr`, `Port.set_dtr`)
- `firmware/tests/CMakeLists.txt` (modified)
- `firmware/tests/test_bridge_framer.c`, `test_bridge_heartbeat.c`, `test_bridge_probe.c`, `test_bridge_rxfwd.c`, `bridge_sim_shim.c`, `bridge_pure_selftest.sh` (new)
- `firmware/tests/bridge_pure_fixtures/` (new: 14 bad, 2 good)
- `firmware/tests/test_link_probe.py` (modified: `OwnBridgePort`, class `OwnBridge`)
- `.github/workflows/firmware.yml` (modified: bridge purity step)
- `docs/test-plan.md` (modified: F.7, F.8, F.39–F.42, H.21–H.23, bridge rig block)
- `docs/link-protocol.md` (modified: "Rules the bridge applies (Story 3.1)")
- `_bmad-output/implementation-artifacts/sprint-status.yaml`, this file

## Change Log

- 2026-11-24: Story 3.1 implemented (bridge firmware, host tests F.7, F.8, F.39–F.42, bench tool `--bridge own`). Bench part pending.
- 2026-11-24: Code review: 11 IMPORTANT and 2 MINOR fixed, 6 MINOR recorded (Review Findings); Task 9.4 review recorded.
- 2026-11-24: Bench: H.21, H.22, H.23, H.1 (3.1 part) PASS; H.22 and H.1 commands use `--expect-ready auto` (macOS completes the handshake). Status done.
