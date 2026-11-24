# bridge

PC-side board (Story 3.1): USB CDC ↔ UART at 460800 baud between the app and the controller board.

- **PC → controller:** frame-aware store-and-forward. The bridge scans for `0x00` delimiters (it never decodes), writes whole frames to the UART, one in flight plus one queued, and lets USB backpressure do the flow control (AD-4).
- **Heartbeat:** its own `HEARTBEAT` (0x40) every 20 ms from a hardware alarm, whatever the USB state, at the next frame boundary ahead of queued app frames; body: firmware version and the CDC DTR flag. The controller's link watchdog (AD-5) depends on it.
- **Controller → PC:** best effort, bytes dropped (and counted) when CDC has no room; never blocks.
- **AD-20 probe:** TX high-Z at power-up; 5 ms idle-high pulses every 500 ms until the controller answers; back to probing on a break or link loss.
- **USB identity:** `2E8A:000A`, product `Bonooru Bridge`, serial = flash unique ID; 1200 baud → BOOTSEL (`pico_usb_reset`).
- **LED:** blue, 1 Hz (Story 2.1).

Layout: `core/` (`ocb_bridge_core`, pure C, host-tested: F.7, F.8, F.39–F.42; purity gate `tools/check-bridge-pure.sh`), `platform/` (TinyUSB, UART, alarm, LED glue). Rules in `docs/link-protocol.md` "Rules the bridge applies (Story 3.1)".
