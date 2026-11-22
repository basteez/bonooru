---
baseline_commit: 9af451e7027804e86cc8800c7a897931eba7fd64
---

# Story 2.3: Controller enumerates as a wired Pro Controller

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As the owner,
I want the controller board to complete the P2 handshake with my Switch 2,
so that the console sees our firmware as a real wired Pro Controller.

## Acceptance Criteria

Copied from `_bmad-output/planning-artifacts/epics.md`, Story 2.3 (the five Given/When/Then blocks, numbered). These are the definition of done. The marking in brackets is added by this story file: **[HOST]** = verifiable by `host-tests` (ctest, ASan and UBSan, no hardware), **[CI]** = verified by a CI step after the owner pushes, **[HW-MAC]** = needs the controller Zero plugged into the Mac (no console, no second board), **[HW-CONSOLE]** = needs the Switch 2 (and, for the readouts, the second Zero running 2wiCC_Comms as the transparent bridge, the Spike 0b chain). The console-side proof is the owner's bench session (Task 11); the dev agent's Tasks 1 to 10 need no hardware.

**AC1.** **[HOST + HW-MAC + HW-CONSOLE]**
**Given** the controller UF2 built with `OCB_PROFILE=P2` (default) and "Pro Controller Wired Communication" ON
**When** the board is plugged into the console's top USB-C port
**Then** the console lists a wired Pro Controller (VID:PID `057E:2009`) on a stock Switch 2 with only the two Zeros and three wires (NFR-02, NFR-03)
**And** the profile implements the AD-12 interface: descriptors, control/OUT handler, `build_report(const ocb_state*)`, `tick()`, `ready()`, reading only the published state

**AC2.** **[HOST + review; HW-MAC shows the replies in use]**
**Given** the P2 handshake (USB `0x80` commands, subcommands for device info, SPI flash reads with stick calibration, player lights, IMU enable with zero IMU data, `0x30` input reports)
**When** it is implemented
**Then** it is written clean-room from public reverse-engineering docs, with no code copied from 2wiCC (NFR-07, ADR-004), and sources are cited in comments

**AC3.** **[HOST; HW-CONSOLE only to watch it happen]**
**Given** a controller firmware-update subcommand from the console
**When** the profile receives it
**Then** it refuses it (C7)

**AC4.** **[HOST + HW-MAC + HW-CONSOLE]**
**Given** a bus reset, suspend or unmount
**When** it happens
**Then** `ready()` becomes false and the handshake restarts after the reset; the board re-enumerates cleanly after dock/undock or console wake (ADR-006, R13)

**AC5.** **[HOST + CI for 5a; HOST for the algorithm and HW-CONSOLE for the number in 5b]**
**Given** a debug build
**When** the console sends subcommands
**Then** each one is logged through the SPSC ring (not in release builds) **(5a)**
**And** the board measures the median IN interval for `INFO.report_period_us` and records the measured Switch 2 polling rate in `docs/usb-controller-emulation.md` (closes the ADR-002 polling hypothesis) **(5b)**

### Reading notes on the AC (what the story file adds, none of it changes the AC)

- **"Lists a wired Pro Controller" is two checks, not one.** Enumeration (the descriptors are right and the device stays on the bus) is proven on the Mac first, without the console, by a one-command host script (D18). The console accepting it (it keeps polling, `console_ready` reads 1, the HOME screen shows a controller) is proven on the Switch 2 and read back through the bridge link, so the owner reads numbers, not screens (D19). The only thing read by eye is "a controller icon appears".
- **"Handshake restarts after the reset" and "re-enumerates cleanly"** need the console to be the thing that sleeps, wakes, docks and has its Wired Communication setting changed. Those are owner actions (H.16), each prompted by one script that checks the result.
- **"Each one is logged through the SPSC ring"**: the ring is the AD-11 core 0 to core 1 debug path. The story adds the consumer that makes the log readable without any stdio (the UART pins carry the link): core 1 drains the ring into a window at the end of the debug STATUS (D14). No new message, no `PROTO_VERSION` change.
- **"Records the measured Switch 2 polling rate"** is a documentation edit with a number that only the console can give (H.15, H.17). The dev agent writes the section with the rows empty; the owner's session fills them. Issue #21 (the LATE threshold and the core 1 stack) is **not** closed here; this story only produces the number Story 2.5 needs (D13).
- **`fw-debug-bench` is superseded** (D15): the real `ready()` exists now, so the fake one is removed, with its preset, CI step and docs.

## Scope boundaries (what is NOT in this story)

| Not here | Where it is |
|---|---|
| The core 1 super-loop, the seqlock double buffer, the 1 MHz timer ownership, hardware watchdog reboot, H.2, the hard-coded timeline and the jitter proof | Story 2.5 (this story reads the published state through one stub function, `ocb_ctrl_published_read()`, which returns NEUTRAL until 2.5 replaces it with the seqlock reader, D12) |
| Deciding the LATE fallback value (`OCB_CORE_LATE_FALLBACK_US`) and re-measuring the core 1 stack | Story 2.5 (issue #21). This story measures the IN interval and feeds it to INFO (D13) |
| Pressing buttons on the console from the app side (a note actually playing) | Stories 2.5 and 3.4. Here the report is always NEUTRAL, the console just sees a connected controller |
| The bridge firmware, its USB identity, heartbeat | Story 3.1 (the bench still uses 2wiCC_Comms and the probe's emulated heartbeat) |
| The setup wizard text about "Pro Controller Wired Communication" | Story 8.5 |
| P1 and P3 profiles | Not planned (AD-12 interface only; R6). `OCB_PROFILE` accepts only `P2` (D21) |
| Rumble, IMU data, NFC, home-light effects, pairing | Accepted and ignored or acknowledged only (D6) |
| Issue #20 (purity gates miss some identifiers) | Untouched. The new gate copies the established list and is drift-guarded against it (D16) |
| STATUS staleness rule, Epic 4 indicators | Epic 4 |

## Tasks / Subtasks

Tasks 1 to 10 are for the dev agent and need **no hardware**; they end with everything green on the host and the three ARM presets compiling. Task 11 is the **owner's bench session**. The dev agent does not commit or push; the owner commits and pushes, and CI is verified afterwards by the owner's push (as in Stories 2.1 to 2.7). Keep Completion Notes honest: anything CI-only is "unverified until pushed", anything on hardware is "unverified on hardware" until Task 11 is recorded.

Local build, as in Story 2.7: from `firmware/`, `export OCB_PICOTOOL_DIR=firmware/.tools/picotool-2.3.1 OCB_ARM_TOOLCHAIN=firmware/.tools/arm-gnu-toolchain-15.2.Rel1` (paths relative to the repo root; use absolute paths if the preset needs them), then `cmake --preset host-tests && cmake --build --preset host-tests && ctest --preset host-tests`, and `cmake --preset fw-release && cmake --build --preset fw-release` (same for `fw-debug`).

- [x] **Task 1: Re-verify the research and write the reference section** (AC: #2; D2, D25)
  - [x] 1.1 Open every source of the table "Sources and what each one gives" in Dev Notes (WebFetch, read-only) and re-verify each row of the three behaviour tables against the source named in its **Src** column. A row marked **UNVERIFIED** needs a second public source, or stays as the documented starting value and gets a line in Completion Notes ("set from the console observation in H.15"). Do not rely on memory or on this story's wording for any byte.
  - [x] 1.2 `docs/usb-controller-emulation.md` (UPDATE): new section "P2 wire protocol (Story 2.3)" with the descriptor values, the 0x80 commands, the subcommand table, the SPI map, the 0x30 layout and the canonical-state mapping, each with its source link; a "Measured on the Switch 2" table with empty rows (VID:PID listed, the handshake sequence the console really sends, median IN interval for `bInterval` 1 and 8, report period jitter, sleep and wake behaviour). Replace the sentence "P2 only: 0x80 0x01/0x02/0x03/0x04 handshake" in the Enumeration diagram with "reactive: the console sends them, we answer". Leave the ADR-002 hypothesis marked **[HYPOTHESIS]** until Task 11 fills it (AC5b).
  - [x] 1.3 Decide nothing new here: where the sources disagree (for example the `0x80 0x04` and `0x80 0x05` wording), implement the cautious reading of D5 and record the disagreement in the section.

- [x] **Task 2: The P2 library, part 1: descriptors and constants** (AC: #1, #2; D1, D3, D4)
  - [x] 2.1 `firmware/controller/profile_p2/` (NEW, pure C99, library `ocb_p2`, include path `ocb_p2/`). `include/ocb_p2/config.h`: `OCB_P2_VID 0x057E`, `OCB_P2_PID 0x2009`, `OCB_P2_BCD_DEVICE 0x0210`, string texts, `OCB_P2_REPORT_DESC_LEN 203`, endpoint addresses, packet size 64, `OCB_P2_BINTERVAL_DEFAULT` (1, D4), the SPI and report constants. Every value carries a one-line comment naming its source (S-number of the table).
  - [x] 2.2 `include/ocb_p2/desc.h`, `desc.c`: `const uint8_t` arrays for the device descriptor (18 bytes), the configuration descriptor with its HID and two endpoint descriptors (41 bytes, `bInterval` passed as a parameter of a small builder, or an array patched once in the glue: the pure library must be able to produce the exact bytes for any `bInterval` 1 to 16 and a test must read them back), the 203-byte HID report descriptor (written as commented items, not as an opaque blob, so a reviewer can read it; D3), the three strings as UTF-16LE data. No pico or TinyUSB header (the glue wraps them).
  - [x] 2.3 `tests/test_p2_desc.c` (**F.24**): an independent walker in the test (not the library's) parses the three descriptors and checks: lengths and `wTotalLength` agree, one interface, class `0x03`, two interrupt endpoints `0x81` and `0x01` of 64 bytes, `bcdUSB 2.00`, `bMaxPacketSize0 64`, the HID descriptor's `wDescriptorLength` equals the report descriptor's real length (203), the report descriptor is a well-formed item stream (every item's size fits, collections balance), the report IDs declared are exactly {0x30 in; 0x21, 0x81 in; 0x01, 0x10, 0x80, 0x82 out} with the sizes of the table, the strings round-trip. For every `bInterval` 1..16 the endpoint bytes are right. Hand-written expected bytes for the device descriptor and the configuration descriptor (from the sources, not from the library).

- [x] **Task 3: The P2 library, part 2: handshake, subcommands, replies** (AC: #2, #3, #4; D5 to D11, D17)
  - [x] 3.1 `include/ocb_p2/p2.h`, `p2.c`: the state `ocb_p2` (handshake step, `streaming`, timer byte, player-lights byte, vibration and IMU flags, reply queue of depth 4, counters, the interval measurer of Task 5) and:
    - `void ocb_p2_init(ocb_p2 *p, const ocb_p2_cfg *cfg)`: `cfg` carries the 6-byte MAC and the serial seed (the glue fills them from the RP2040 unique id, D10).
    - `void ocb_p2_restart(ocb_p2 *p)`: bus reset, unmount, suspend or re-configuration (AD-12 "restarts its handshake"): back to the start, queue cleared, `streaming` false, timer kept running.
    - `ocb_p2_rx ocb_p2_on_output(ocb_p2 *p, const uint8_t *buf, size_t len, ocb_p2_log *log)`: the control/OUT handler. `buf` is the OUT report as TinyUSB delivers it, including the report-ID byte (D6). It never reads past `len`, never writes outside `p`, accepts every length from 0 to a few hundred and every byte value, and returns what happened (`HANDLED`, `REFUSED`, `IGNORED`, `MALFORMED`, `QUEUE_FULL`); `*log` (may be NULL) gets one record for the debug log (kind and id, D14). No global state, no assert, no allocation.
    - `size_t ocb_p2_next_in(ocb_p2 *p, const ocb_state *published, uint8_t out[OCB_P2_REPORT_LEN], size_t cap)`: the next IN report: a queued reply if any, else (when `streaming`) the `0x30` report built from `published` (`ocb_p2_build_report`), else 0. This is the AD-12 `build_report` entry with the state passed in, so the profile reads nothing else.
    - `size_t ocb_p2_build_report(ocb_p2 *p, const ocb_state *s, uint8_t *out, size_t cap)`: the `0x30` builder alone (Task 4).
    - `void ocb_p2_tick(ocb_p2 *p, uint64_t now_us)`: the AD-12 `tick()`: advances the timer byte rate and the interval measurer's window. Pure: time is an argument.
    - `bool ocb_p2_ready(const ocb_p2 *p)`: the AD-12 `ready()`: the console handshake is complete (D5). The 100 ms IN-stall part of `console_ready` stays in `ocb_link/ready.c` (Story 2.6 D11), fed by the mailbox; this predicate does not duplicate it.
  - [x] 3.2 Behaviour exactly as the three tables of Dev Notes ("Behaviour reference"). Every response is built by one function per message with a bounds check against the 64-byte buffer; SPI reads clamp `len` to the table's maximum and never index outside the 4 KB-style image (the image is a small function of the address, not a stored 128 KB array, D7). Refusals (D9) leave the state untouched except a counter and a log record.
  - [x] 3.3 `tests/test_p2_handshake.c` (**F.25**), frames hand-written from the sources: the full sequence `80 01`, `80 02`, `80 03`, `80 02`, `80 04` gives exactly `81 01 00 03 <mac reversed>`, `81 02`, `81 03`, `81 02` and no reply to `80 04`, with `ready()` false before `80 04` and true after; a host that skips steps (`80 02` then `80 04`) or sends only subcommands with `03 30` (D5) reaches ready; every ordering of the five commands never crashes and `ready()` follows the rule; `80 05` stops streaming and clears ready; `ocb_p2_restart` after any step returns to the start and the whole sequence works again, 100 times in a row; replies queue in order and the 5th queued reply reports `QUEUE_FULL` without corrupting the queue; `0x80` with an unknown command id is `IGNORED`.
  - [x] 3.4 `tests/test_p2_subcmd.c` (**F.26**): every subcommand of the table through output report `0x01` with an 8-byte rumble field and through the 0x10 rumble-only report (no reply); reply bytes compared with hand-written expectations (ack byte, subcommand echo, data); device info with the test MAC; SPI reads: every documented region byte for byte, an unknown region returns `0xFF`, the length limit (`0x1D`, `0x1E`, `0xFF`) and the 32-bit address edge (`0xFFFFFFF0` + 0x1D does not wrap into data), zero length; the player-lights argument is stored; refusals (C7): SPI write `0x11`, sector erase `0x12`, BT pairing `0x01`, reset pairing `0x07`, MCU firmware-update path `0x21` and `0x22` with argument `0x02`, `0x80 0x06`, `0x80 0x91`, `0x80 0x92` get the documented refusal (D9) and **change nothing**: a before and after snapshot of the whole `ocb_p2` state plus every SPI region read back; the refusal count rises; an unlisted subcommand id gets the generic ack of D6. A negative control: a build of the test with the refusal switched off (a compile definition in the test only) must fail, recorded once in the Debug Log.

- [x] **Task 4: The input report** (AC: #1; D8, D12)
  - [x] 4.1 `report.c` (in `ocb_p2`): `ocb_p2_build_report` as the "Report 0x30" and "Canonical state to report" tables of Dev Notes: header bytes, the three button bytes, the hat to D-pad bits, the 12-bit stick packing with the D8 scaling and the Y inversion, the fixed fields, 36 IMU bytes of zero, zero padding to 64 bytes, the timer byte incremented once per report. Reserved bits (flags, button bits 14 and 15, hat 9 to 255) never reach the report.
  - [x] 4.2 `tests/test_p2_report.c` (**F.27**): NEUTRAL gives the exact 64 bytes with `30 <timer> 91 00 80 00 00 08 80 00 08 80 <vib> 00...`; each of the 14 button bits alone sets exactly its one bit in its one byte (hand-written table, not derived from the library's); all 65 536 `buttons` values: bits 14 and 15 never appear, the 14 others map one to one; every `hat` value 0 to 255 (9 to 255 neutral, diagonals set two D-pad bits, no illegal pair such as Up+Down); all 256 values of each stick axis: the 12-bit value is monotonic, 0x80 gives exactly 0x800, 0x00 gives centre − 0x700 and 0xFF stays inside the calibrated range of D7, Y is the mirror of X (canonical `ly 0x00` = up = the larger raw value); packing round-trips through an independent unpack in the test; the IMU bytes are zero; the timer byte advances by 1 per report and wraps; `build_report` on a NULL or short buffer returns 0 and writes nothing.

- [x] **Task 5: IN interval measurement and the fuzz test** (AC: #5b, #2; D13, D17)
  - [x] 5.1 `interval.c` (in `ocb_p2`): a bounded-cost median of the time between two IN completions (D13): `ocb_p2_interval_note(p, now_us)` O(1), a published value `ocb_p2_report_period_us(p)` that is 0 until 64 samples are in, then the median of the last 256 (histogram of 50 µs bins up to 20 ms, or any structure with the same behaviour and a bounded cost; the choice is the dev's, the worst-case cost per call is asserted by the test as an operation count and written in the notes). Samples above 20 ms (the console stopped polling) and samples after a restart are not counted; a restart clears the value to 0.
  - [x] 5.2 `tests/test_p2_interval.c` (**F.29**): a steady 1000 µs train gives 1000 (±25); 8000 µs gives 8000; a mixed train (80 % 1000 µs, 20 % 2000 µs) gives the median, not the mean; one outlier of 15 ms does not move it; wrap of a 32-bit and a 64-bit clock; a 25 ms gap is dropped and not counted; fewer than 64 samples gives 0; restart gives 0 and starts a new window; a backwards clock is ignored.
  - [x] 5.3 `tests/test_p2_fuzz.c` (**F.28**), 3 seeds × 200 000 iterations under ASan and UBSan: random OUT reports (length 0 to 300, random report ids with 80 % weight on the six real ones, random subcommand ids, SPI reads at random 32-bit addresses and lengths 0 to 255, rumble bytes random), interleaved with `restart`, `next_in`, `build_report` with random states and short buffers; after every step: every produced report has a valid header and length ≤ 64, no write outside the output buffer (guard bytes), the queue depth never exceeds 4, the counters are monotonic, `ready()` is true only after a handshake step was seen since the last restart. Optional, outside CI: `tests/fuzz/fuzz_p2.c`, a libFuzzer entry (`LLVMFuzzerTestOneInput`) over `ocb_p2_on_output`, built only with `-DOCB_FUZZ=ON` and clang; not in the default presets.

- [x] **Task 6: The purity gate for `ocb_p2`** (AC: #1; D16)
  - [x] 6.1 `firmware/tools/check-p2-pure.sh` (NEW): a copy of `check-link-pure.sh` with the allowed includes `<stdint.h> <stddef.h> <stdbool.h> <string.h> <limits.h>`, `"ocb_proto/<name>.h"`, `"ocb_p2/<name>.h"` (no `ocb_core`, no `ocb_link`: the profile reads only `ocb_state`), the same `banned_word_re` line byte for byte, and the same CMake rule (no `pico_*`, `hardware_*`, `ocb_hw`, `tinyusb*`, `tusb`). Banned include substrings add `tusb` (already there) and `class/hid`.
  - [x] 6.2 `tests/p2_pure_selftest.sh` and `tests/p2_pure_fixtures/{bad,good}` (NEW, **F.30**), as `link_pure_selftest.sh`: every bad fixture flagged (SDK, TinyUSB `tusb.h`, `class/hid/hid_device.h`, `<stdio.h>`, `<stdlib.h>`, a clock read, an allocation, `multicore_`, a CMake linking `tinyusb_device`, `ocb_core` or `ocb_link` include), the good one and the real `controller/profile_p2` tree pass, exit 2 on a usage error, and the banned list equals the core gate's (drift guard, as 2.6 D16). Registered in `tests/CMakeLists.txt` as `p2_pure_check`.
  - [x] 6.3 `.github/workflows/firmware.yml` (UPDATE): one step "Controller P2 profile is pico-sdk and TinyUSB free (AD-2)" running the script, after the link step.

- [x] **Task 7: Mailbox report period, debug log ring and STATUS window** (AC: #5; D13, D14, D24)
  - [x] 7.1 `controller/link/include/ocb_link/ready.h`, `ready.c` (UPDATE): add `volatile uint32_t report_period_us;` to `ocb_usb_mailbox` (single writer: core 0; initialised to 0 by `ocb_usb_mailbox_init`) and `ocb_usb_mailbox_set_report_period()`. `ocb_link_poll` (UPDATE, `link.c`) reads it once per pass; when it differs from the last applied value it calls `ocb_link_set_report_period()` (which sets INFO and the core's LATE threshold). Values above 65535 saturate. A USB-down event leaves the value to core 0, which sets 0 (D13).
  - [x] 7.2 `controller/link/include/ocb_link/dbglog.h`, `dbglog.c` (NEW, pure, in `ocb_link`): an SPSC ring of `uint16_t` records (kind in the high byte, id in the low byte; 64 entries; core 0 push only, core 1 pop only; volatile indices; `push` returns false and counts a drop when full, never blocks; D14), and a window of the newest 16 records plus a `u16` total, filled by `ocb_dbglog_drain(win, ring)`. Kinds: `0x01` USB `0x80` command id, `0x02` subcommand id, `0x03` other output report id, `0x04` refusal (the refused subcommand or command id), `0x05` ring overflow marker, `0x06` handshake complete, `0x07` restart.
  - [x] 7.3 `tx.c` (UPDATE): STATUS debug counters v3: append to the 21 bytes of v2 the window: `p2_log_total u16`, `p2_log_n u8` (0 to 16), then `n` records of 2 bytes (oldest first). Body ≤ 11 + 21 + 3 + 32 = 67 bytes: raise `TX_BODY_MAX` and `TX_WIRE_MAX` accordingly (and the compile-time size check). The window is only built when `cfg.debug_counters` is true; keep the release stack unchanged (build the scratch only in the debug branch, or size it by `cfg`) and measure with `-fstack-usage` on the ARM release build before and after; put both numbers in the Completion Notes (hand-off to issue #21, which stays open).
  - [x] 7.4 `link_runner.c` (UPDATE): `ocb_ctrl_link_step()` drains the ring into the window once per pass (debug builds only, behind `OCB_DEBUG_BUILD`; the call and the ring do not exist in release). Add `ocb_ctrl_dbglog()` (the ring, for core 0) and `ocb_ctrl_published_read(ocb_state *out)` (D12).
  - [x] 7.5 Tests: `tests/test_link_dbglog.c` (**F.31**): ring order, full ring drops and counts (never overwrites), the window keeps the newest 16 in order, `total` wraps, the STATUS debug tail carries total, n and the records exactly (compared with hand-written bytes), the decode by the probe's independent decoder is in F.32, release configuration (`debug_counters` false) produces the 11-byte STATUS with no tail; the mailbox field: after `set_report_period(1000)` the next poll makes INFO report 1000 and the core's LATE threshold 1000, a later 0 returns both to unknown (`INFO` 0, fallback), a value above 65535 saturates, a restart of the link keeps counting from zero (the D28 rebase rule of 2.6). Update `test_link_ready.c` (**F.10**), `test_link_session.c` (**F.15**, INFO), `test_link_tx.c` (**F.16**, STATUS length and cadence unchanged), `test_link_fuzz.c` (**F.17**, longer frames) and the `ocb_link_sim` shim only as far as they must change; every update keeps its old meaning.

- [x] **Task 8: TinyUSB glue, CMake, `main.c`, the removal of the bench build** (AC: #1, #4, #5; D1, D11, D12, D15, D20, D21)
  - [x] 8.1 Read before writing: `firmware/external/pico-sdk/lib/tinyusb/src/class/hid/hid_device.{c,h}` (OUT data arrives through `tud_hid_set_report_cb` with `report_id` 0 and type OUTPUT and **the report-ID byte as `buffer[0]`**; `tud_hid_report(id, p, len)` prepends `id` when it is non-zero, so the glue passes a non-zero id and the payload without it, or id 0 and the whole 64 bytes: pick one and test it on the bench, D6), `src/device/usbd.{c,h}` (which callback fires on a bus reset: `tud_umount_cb`, `tud_event_hook_cb` with `DCD_EVENT_BUS_RESET`, or both; `tud_suspend_cb`; `tud_mount_cb`) and `examples/device/hid_composite/src/{tusb_config.h,usb_descriptors.c}` as the shape of the glue (read only; nothing copied). Record in the Debug Log which callback each of the three USB-down events really uses in 0.18.0.
  - [x] 8.2 `firmware/controller/platform/tusb_config.h` (NEW): device mode only (`CFG_TUSB_MCU OPT_MCU_RP2040`, `CFG_TUSB_RHPORT0_MODE OPT_MODE_DEVICE`), `CFG_TUD_HID 1`, `CFG_TUD_HID_EP_BUFSIZE 64`, every other class 0, `CFG_TUD_ENDPOINT0_SIZE 64`, no logging.
  - [x] 8.3 `firmware/controller/platform/usb_p2.c`, `usb_p2.h` (NEW): the glue, core 0 only (AD-11 "the only core that touches TinyUSB"): the four descriptor callbacks (`tud_descriptor_device_cb`, `_configuration_cb`, `_string_cb`, `tud_hid_descriptor_report_cb`) returning the library's bytes; `tud_mount_cb` (calls `ocb_p2_restart`), `tud_umount_cb`, `tud_suspend_cb`, the bus-reset hook: each calls `ocb_p2_restart`, posts the matching `ocb_usb_mailbox_*` event, sets the mailbox report period to 0 and pushes the debug restart record; `tud_hid_set_report_cb` calls `ocb_p2_on_output`, pushes the returned log record to the SPSC ring (**debug builds only**, `#ifdef OCB_DEBUG_BUILD`), and when the call made the handshake complete posts `ocb_usb_mailbox_handshake_done` exactly once; the report pump: `tud_hid_report_complete_cb` posts `ocb_usb_mailbox_in_report`, notes the interval, publishes the median to the mailbox when it changes, and queues the next IN report (`ocb_p2_next_in` with the state from `ocb_ctrl_published_read`); `usb_p2_task()` called by `main()` each pass: `tud_task()`, `ocb_p2_tick(time_us_64())`, and the kick: if streaming or a reply is pending and `tud_hid_ready()`, send. Never blocks, never sleeps, no `printf`. Build the MAC (locally administered bit set) and serial seed from `pico_get_unique_board_id` here (the pure library gets them as bytes, D10).
  - [x] 8.4 `firmware/controller/main.c` (UPDATE): remove the `OCB_BENCH_FAKE_READY` code and its comment block; call `usb_p2_init()` after `ocb_ctrl_usb_mailbox_init()` and before `multicore_launch_core1`, and `usb_p2_task()` in the loop next to the unchanged LED blink (the LED must still blink at 2 Hz amber: H.0 must not regress, check at the bench, D20). Update the file comment (Story 2.3 is done, 2.5 still replaces `core1_entry`).
  - [x] 8.5 `firmware/controller/CMakeLists.txt` (UPDATE): add `platform/usb_p2.c`, link `ocb_p2` and `tinyusb_device` (and whatever the SDK needs for `pico_unique_id`), strict warnings on the glue as on `link_runner.c`, the `OCB_P2_BINTERVAL` cache variable (1 to 16, default 1, D4) passed as a compile definition and guarded with a `message(FATAL_ERROR)` outside the range, `OCB_PROFILE` (D21), and remove the `OCB_BENCH_FAKE_READY` block. Fix the comment "TinyUSB unlinked": it is linked now; stdio over USB and UART stay off (`pico_enable_stdio_*` 0). Add `firmware/controller/profile_p2/CMakeLists.txt` (C99, `C_EXTENSIONS OFF`, `-Wpedantic -Wshadow -Wconversion`, no SDK) and `add_subdirectory(controller/profile_p2)` in the top-level `CMakeLists.txt` next to `controller/link` (built in both modes). In `link_runner.c`, take `cfg.hid_profile` from the profile constant (`OCB_HID_PROFILE_P2` from `OCB_PROFILE`).
  - [x] 8.6 Remove the bench build (D15): `OCB_BENCH_FAKE_READY` option in the top-level `CMakeLists.txt`, the `fw-debug-bench` configure and build presets, the CI step "Build fw-debug-bench", and every doc sentence that offers it (list in Task 10). `firmware/build/fw-debug-bench` is untracked build output: leave it, delete nothing the owner has not asked to delete.
  - [x] 8.7 `firmware/tools/check-release-nolog.sh` (NEW) and a CI step "Release has no debug log (AC5a)": runs `arm-none-eabi-nm` on `build/fw-release/controller/controller.elf` and fails if any symbol of the debug log (`ocb_dbglog_`, `ocb_ctrl_dbglog`) exists; run on `fw-debug`'s ELF it must find them (the script's own self-test is a second mode `--expect-present`, run in CI on the debug ELF, so a rename cannot make the gate vacuous).
  - [x] 8.8 Build all three ARM presets and the host preset with 0 warnings; record the `arm-none-eabi-size` of `controller.elf` before and after (RAM and flash) in the Completion Notes.

- [x] **Task 9: Host tools** (AC: #1, #4, #5; D18, D19)
  - [x] 9.1 `firmware/tools/p2_probe.py` (NEW; standard library only for `enum`; `hidapi` (`pip install hidapi`, import `hid`) only when a HID device is opened, as `link_probe.py` needs pyserial only when a port is opened). Written from the cited sources and `docs/usb-controller-emulation.md`, not from the C code. One-shot subcommands, each printing one line per check and ending with `PASS` or `FAIL <reason>` (exit 0 or 1), every wait with a stated time budget and a prompt line that says what the human has to do:
    - `enum [--wait-s 90]` (H.13): waits for VID:PID `057E:2009` to appear in `system_profiler SPUSBHostDataType -json` (fallback `SPUSBDataType`, then `ioreg -p IOUSB -l -w0`; the dev reads the JSON key names on the owner's macOS first and writes the parser for them, with a recursive walk over `_items`), absorbs plug bounces (the device must be present in 3 consecutive 1 s polls before the checks run, and a disappearance during the checks restarts the stable window once), checks vendor and product id, the vendor string "Nintendo Co., Ltd.", the product string "Pro Controller", the serial "000000000001", `bcdDevice` 2.10, and the HID report descriptor bytes against the 203 expected ones when `ioreg` exposes them (otherwise that single line says `SKIP`, never `FAIL`).
    - `handshake [--seconds 10] [--hold N]` (H.14): opens the HID device (retry up to 30 s), plays the host side exactly as the table "Host sequence used by the probe" (including a harmless refusal probe, D9) with each request resent after 300 ms up to 3 times, checks every reply byte for byte against its own hand-written expectations, reads the `0x30` stream for `--seconds`, prints the host-side median interval and the count, checks that a SPI region read before and after the refused write is unchanged. `--hold N` keeps reading reports for N more seconds so the controller's `console_ready` stays 1 for another tool (D15). If the open fails it prints the macOS "Input Monitoring" hint, once.
  - [x] 9.2 `firmware/tools/link_probe.py` (UPDATE): new one-command steps on the existing session, heartbeat and trace machinery:
    - `p2check [--wait-s 120]` (H.15): prompts "PLUG THE CONTROLLER INTO THE SWITCH TOP USB-C PORT (Wired Communication ON)" and waits up to the budget for INFO and `console_ready` 1 (HELLO is resent while the controller boots, as in `linkup`); then prints the handshake sequence decoded from the debug STATUS window (every record in order: `80 01`, `80 02`, ..., subcommand ids, refusals, restarts), INFO `hid_profile` (must be 2) and `report_period_us`; after 5 s of stable polling checks `report_period_us` in 500 to 20000 and `abort_count` unchanged; PASS or FAIL with the reason. Prints the median in one line meant to be pasted into the test plan.
    - `p2watch` (H.16): the scripted cycle with one prompt per human action and a stated time per step: sleep the console (accepts either a `console_ready` drop with `EVENT USB_SUSPENDED`, or STATUS stopping because the port lost power; prints which), wake it (ready back within 30 s, handshake log shows a restart then a new handshake), switch Wired Communication OFF (flag 0 within 150 ms of the last IN report, measured), back ON (ready 1 again), optional `--dock` (undock, dock). Each transition checks `abort_count` and `last_event` and absorbs the settle window of Story 2.7 D42.
    - `direct` and `playing` (UPDATE): drop the "not the bench build" skip (exit code 2); they now require `console_ready` 1 from a real host and say so (`start p2_probe.py handshake --hold 120 in the same shell line, or plug into the console`). Their rules and numbers are unchanged.
    - the STATUS decoder learns the v3 window (D14) and prints it in `watch`.
  - [x] 9.3 `tests/test_p2_probe.py` (**F.32**, ctest `p2_probe_selftest`, `python3 -I`): `p2_probe.py`'s own expected bytes, its `enum` parser on three captured-shape JSON fixtures (SPUSBHostDataType, SPUSBDataType, a bounce where the device vanishes once; hand-written fixtures in `tests/fixtures/`), and its `handshake` logic run against the real `ocb_p2` through a ctypes shared module built like `ocb_link_sim` (`ocb_p2_sim`, without sanitizers) with a fake HID transport, a fake clock, a lossy variant that drops one reply (the resend path) and negative controls (a reply with one wrong byte must make it print FAIL). The `link_probe.py` side: `p2check` and `p2watch` run unmodified against `ocb_link_sim` extended with the v3 window and a scripted "console" (sleep, wake, wired off and on), with negative controls (no `console_ready` within the budget, an interval of 100 µs, a missing handshake record). Registered in `tests/CMakeLists.txt`; update the existing F.19, F.20, F.23 fixtures only as far as the removal of the bench build requires (the fake "ready console for the bench build" of the model becomes "a host that completed the handshake").

- [x] **Task 10: Documentation** (D22)
  - [x] 10.1 `docs/test-plan.md` (UPDATE): rows **F.24 to F.32** (Firmware host tests table), rows **H.13 to H.17** and the changed **H.11** (Hardware table), a "Bench rig for the P2 profile (Story 2.3)" block (Mac-only part, console part, the commands with `"$P"`, the recorded fields), and the Watchdog tests untouched. Update F.10 text (now fed by the real profile as well), F.15, F.16, F.17 (STATUS tail), F.19 to F.23 where the bench build is named. Test environment table: no value changes (system 23.5.0, OoT 1.1.1 are read, not asked); add the line "Console USB: top USB-C port, Pro Controller Wired Communication ON".
  - [x] 10.2 `docs/usb-controller-emulation.md`: finish Task 1.2; update "Profile interface (AD-12)" with the final function names and the D5 ready rule; the IN-interval hypothesis header gets the measured number only after Task 11.
  - [x] 10.3 `docs/link-protocol.md` (UPDATE, small): the STATUS debug counters list gets "v3, bytes 21 and on (Story 2.3 D14)": the P2 log window; "Rules the controller link applies" gets a "Story 2.3" paragraph: mailbox `report_period_us`, INFO reflects the measured value once known (0 before, and again after a USB-down event), the LATE threshold follows it (Story 2.5 confirms or replaces the fallback). No wire value changes, no `PROTO_VERSION` bump, no vector edit (D24). Keep `test_docs_constants.py` (F.22) green; extend it with the new values only if a docs table is added (VID, PID, descriptor length, default `bInterval`, STATUS tail layout) so the doc and `ocb_p2/config.h` cannot drift.
  - [x] 10.4 `firmware/README.md`, `firmware/controller/README.md` (new paragraphs for `profile_p2/` and the USB glue, the `OCB_P2_BINTERVAL` and `OCB_PROFILE` options, the removal of the bench build, the gate), `firmware/THIRD_PARTY_NOTICES.md` (TinyUSB now "linked into the controller UF2"; a line for `ocb_p2`: "written from public reverse-engineering documentation (list), no third-party code"; one sentence that the USB identifiers 057E:2009 and the strings are those of the device the console expects, used for interoperability with the owner's console, no third-party code or assets).
  - [x] 10.5 `_bmad-output/specs/spec-bonooru/risk-register.md` (UPDATE, status column only, and only after Task 11): C7 (refusal implemented and observed), R13 (re-enumeration result). Do not edit before the evidence exists. Done after the bench (2026-11-22): C7 "mitigated (Story 2.3)" with what was and was not observed; R13 evidence added to its mitigation cell (that table has no status column; dock and undock not run).
  - [x] 10.6 Remove every offer of the bench build: Story 2.7's text stays as history; `docs/test-plan.md` H.11, `firmware/controller/README.md`, `link_probe.py` docstring and help, `CMakePresets.json`, `.github/workflows/firmware.yml`.

- [x] **Task 11: Owner's bench session** (AC: #1, #3, #4, #5; D19). Rows in "Hardware checks" below. Needs: controller Zero, the Apple USB-C cable (reference cable), the Switch 2 (top USB-C port), the second Zero with 2wiCC_Comms wired as in Spike 0b (the Story 2.6 rig, wiring not rechecked) for H.15 to H.17, `pip install pyserial hidapi` in the bench venv.
  - [x] 11.1 H.13, H.14 on the Mac (no console). 11.2 H.15 on the console, then H.16, then H.17 (optional second flash). 11.3 Re-run H.11 on the real stack. 11.4 Fill the test plan rows, the "Measured on the Switch 2" table in `docs/usb-controller-emulation.md` (AC5b), then risk-register C7 and R13. 11.5 Mark the story done only when H.13 to H.16 are PASS or each FAIL has an explained decision.

## Dev Notes

### What this story is

The controller board stops being "a Zero that blinks and talks UART" and becomes a USB device the console accepts. Everything about the P2 conversation (descriptors, the `0x80` commands, the subcommand answers, the SPI flash image, the `0x30` report) lives in one new **pure C99 library**, `ocb_p2`, that knows nothing about TinyUSB or the RP2040, so it is tested on the PC with fixed bytes and fuzzed. A thin **TinyUSB glue** on core 0 wires it to the SDK: descriptor callbacks, the OUT handler, the IN pump, the USB events that feed the existing `ocb_usb_mailbox` of Story 2.6, so `console_ready`, `NAK NOT_READY` and the USB aborts become real without touching `ocb_link` rules. The debug subcommand log and the measured IN interval cross to core 1 through the mailbox and one SPSC ring, and leave the board inside the debug STATUS, so the owner can read the handshake the Switch 2 really performs.

### Design in one page

```text
 Switch 2 --USB--> TinyUSB (core 0)
   usb_p2.c  (glue: callbacks, IN pump, events)            profile_p2/ = ocb_p2 (pure C99)
     tud_hid_set_report_cb --OUT report--> ocb_p2_on_output --reply queue (4)--+
     tud_hid_report_complete_cb --> ocb_p2_next_in(published) -> 0x81/0x21/0x30 -+--> tud_hid_report
     tud_mount/umount/suspend/bus reset --> ocb_p2_restart + mailbox event
        |                                  |
        | mailbox (single writer = core 0) | SPSC ring (debug builds only)
        v                                  v
   core 1: ocb_link_poll -> ocb_ready (console_ready), aborts USB_SUSPENDED, INFO.report_period_us
           ocb_dbglog_drain -> STATUS debug v3 window  --UART--> bridge --> app / link_probe.py
   ocb_ctrl_published_read(): NEUTRAL until Story 2.5's seqlock
```

Timing of a poll: the host takes the IN report that is already in the endpoint buffer; in `tud_hid_report_complete_cb` we immediately queue the next one, so a report is at most one poll interval old when the console reads it (a property of interrupt endpoints, measured by Story 2.5, not fixed here).

### Sources and what each one gives (clean-room, D2)

Read on 2026-11-22 (read-only web fetches). **Used for facts** (offsets, ids, values, sequences), never for code. The implementer re-opens each (Task 1.1).

| S | Source | Licence seen | What it gives |
|---|---|---|---|
| S1 | dekuNukem, Nintendo_Switch_Reverse_Engineering, `USB-HID-Notes.md` https://github.com/dekuNukem/Nintendo_Switch_Reverse_Engineering/blob/master/USB-HID-Notes.md | repo notes (no licence file needed to cite facts) | the `0x80` commands: `01` status (reply `81 01 00 <type> <MAC reversed>`, sample `81 01 00 02 57 30 ea 8a bb 7c`), `02` handshake (reply `81 02`), `03` baud (reply `81 03`, must be followed by another `02`), `04` no timeout / stay in USB HID mode, `05` allow timeout, `06` possible reset, `91`/`92` UART pass-through; handshake order `02, 03, 02`; the Pro Controller has a single interface |
| S2 | same repo, `bluetooth_hid_notes.md` https://github.com/dekuNukem/Nintendo_Switch_Reverse_Engineering/blob/master/bluetooth_hid_notes.md | same | report `0x30` layout (timer, battery/connection, 3 button bytes, 2 × 3 stick bytes, vibrator byte, 36 bytes of IMU), button bit assignment per byte, stick packing, `0x21` subcommand-reply header (ack byte, subcommand id, data), output report `0x01` (counter, 8 rumble bytes, subcommand id, args), `0x10` rumble only, neutral rumble `00 01 40 40 00 01 40 40` |
| S3 | same repo, `bluetooth_hid_subcommands_notes.md` https://github.com/dekuNukem/Nintendo_Switch_Reverse_Engineering/blob/master/bluetooth_hid_subcommands_notes.md | same | subcommand ids and layouts: `02` device info (firmware 2 bytes, type `03` = Pro, constant `02`, MAC, constant `01`, colour flag), `03` input mode (`30` standard full), `04` elapsed times, `08` shipment, `10` SPI read (address LE32, length ≤ `0x1D`, reply `90 10` + address + length + data), `11` write (status byte), `30` player lights (bitfield), `38`, `40` IMU, `41`, `48` vibration, `21`/`22` MCU config and state (`02` = resume for update); unused ids ack `80 <id> 03` |
| S4 | same repo, `spi_flash_notes.md` https://github.com/dekuNukem/Nintendo_Switch_Reverse_Engineering/blob/master/spi_flash_notes.md | same | the address map (`0x6000` serial, `0x6012` type, `0x601B` colour flag, `0x6020` IMU calibration, `0x603D` left stick, `0x6046` right stick, `0x6050` colours, `0x6080` offsets, `0x6086`/`0x6098` stick parameters, `0x8010`/`0x801B` user stick calibration with magic `B2 A1`), the 9-byte stick calibration packing and its field order (left: max, centre, min as X,Y pairs in the order max-X, max-Y, centre-X, centre-Y, min-X, min-Y; right: centre, min, max) |
| S5 | Chromium `device/gamepad/nintendo_controller.cc` https://github.com/chromium/chromium/blob/main/device/gamepad/nintendo_controller.cc | BSD-style | the init sequence a real host runs on a wired Pro Controller (`80 01`, `80 02`, `80 03`, `80 02`, `80 04`, then subcommands `30`, `40`, `41`, SPI reads at `0x6020` (24), `0x6080` (6), `0x603D` (18), `0x6086` (18), then `48`, `03`), the `0x21` SPI reply layout (ack `90`, subcommand, address LE16 at offsets 14 to 15 of the data block, length, data), stick packing formula, button bytes |
| S6 | SDL `SDL_hidapi_switch.c` https://github.com/libsdl-org/SDL/blob/main/src/joystick/hidapi/SDL_hidapi_switch.c | zlib | command ids (status `01`, handshake `02`, high speed `03`, force USB `04`, clear USB `05`, reset MCU `06`), the host order (handshake, high speed, handshake, force USB, calibration reads at `0x603D` (18) and `0x8010` (22), vibration enable, force USB again), status reply parsing (byte 3 type, bytes 4 to 9 MAC reversed), `0x21` parsing (ack byte bit `0x80`, subcommand id, data from byte 15), USB packets are 64 bytes zero padded |
| S7 | mzyy94, "Nintendo Switch Pro Controller USB gadget" https://mzyy94.com/blog/2020/03/20/nintendo-switch-pro-controller-usb-gadget/ | blog (data only) | a `usbhid-dump` of the 203-byte HID report descriptor of a real controller (hex), report ids in and out, a sample `0x30` packet from a real wired controller (`30 df 91 00 80 00 31 28 7c dc 37 7b 0a ...`), stream of about 80 reports per second after `80 04`, "the Switch recognises the device only if the initialisation completes" |
| S8 | tmk, `lsusb -v` dump of a Pro Controller https://gist.github.com/tmk/9725596c6726b561ad2bf1f1e07e1523 | gist (data only) | device descriptor (`057E:2009`, `bcdDevice 2.10`, USB 2.00, `bMaxPacketSize0 64`), configuration (41 bytes, `bmAttributes 0xA0`, 500 mA), interface (HID, 2 endpoints), HID descriptor (`bcdHID 1.11`, report descriptor 203 bytes), endpoints `0x81` IN and `0x01` OUT, 64 bytes, `bInterval 8`, strings "Nintendo Co., Ltd." / "Pro Controller" / "000000000001" |
| S9 | joycontrol `memory.py`, `protocol.py` https://github.com/mart1nro/joycontrol (files under `joycontrol/`) | none stated in the pages read: **values only, never code** | default stick calibration bytes (`00 07 70 00 08 80 00 07 70` for both sticks: centre `0x800`, range `0x700`), acks used (`82` device info, `83` elapsed times, `90` SPI, `80` others), the `0x03` handling of modes `30` and `31` |
| S10 | espp `switch_pro` component https://components.espressif.com/components/espp/switch_pro | MIT | confirms the shape of a software emulation: OUT reports `80`, `01`, `10` in, replies `81`, `21`, `30` out, `80 04` then streaming, a serial number and calibration emulated from SPI banks, VID/PID `057E:2009`, `bcdDevice 2.10` |
| S11 | TinyUSB 0.18.0 sources in the pinned submodule `firmware/external/pico-sdk/lib/tinyusb` | MIT | `hid_device.c` (OUT data to `tud_hid_set_report_cb` with the report-ID byte in the buffer, `tud_hid_report` prepending a non-zero id, `tud_hid_report_complete_cb`), `usbd.c` (events), `examples/device/hid_composite` (shape of `tusb_config.h` and descriptor callbacks) |

**Not used on purpose:** 2wiCC (no licence: NFR-07, ADR-004; the owner's earlier reading of it is not a source and nothing is copied or paraphrased from it), the Linux `hid-nintendo` driver and `nscon` (GPL: not read for the implementation), and any Nintendo material. Web search found no source that fixes the exact order of the `0x80` steps on a Switch 2; S5 and S6 give what a host does, and what the console sends is unknown until H.15 (D5, D25).

### Decisions (made autonomously, 2026-11-22; the owner can overrule any of them)

Each was researched in the repo first (AD-1, AD-2, AD-7, AD-11, AD-12, AD-19, ADR-002, ADR-004, ADR-006, C7, R13, `docs/usb-controller-emulation.md`, `docs/link-protocol.md`, `docs/test-plan.md`, Stories 2.4, 2.6, 2.7, issues #20 and #21, the pinned TinyUSB 0.18.0 sources) and then online (S1 to S10). Open questions that the sources could not answer are marked and resolved by the cautious option.

**D1. One new pure library `ocb_p2` in `controller/profile_p2/`, one glue file on core 0.**
The structural seed of the spine names `controller/profile_p2/` ("P2 descriptors + handshake + report") and `controller/platform/` ("core0 usb"). The profile library holds every byte and rule and is pure C99 (C99, `C_EXTENSIONS OFF`, strict warnings, built in both modes), the glue (`platform/usb_p2.c`, `tusb_config.h`) is the only code that includes TinyUSB or the SDK. This is the AD-12 interface: descriptors (`desc.h`), control/OUT handler (`ocb_p2_on_output`), `build_report` (`ocb_p2_build_report`, taking `const ocb_state *`), `tick()` (`ocb_p2_tick`), `ready()` (`ocb_p2_ready`). *Alternative rejected:* the protocol inside the glue (untestable on the PC, no fuzzing, and the AC wants a purity gate); putting it into `ocb_link` (the link must not know about a USB profile, and the profile must not see `ocb_core`).

**D2. Clean-room rule: facts from public documents only, each cited; GPL sources and 2wiCC are not consulted for the code.**
Allowed: S1 to S11. Every constant, offset or sequence in `ocb_p2` has a one-line comment naming its source (S-number and, where a document has sections, the section), and `docs/usb-controller-emulation.md` carries the full links. The HID report descriptor and the USB descriptors are interoperability data copied as values from hardware dumps (S7, S8; there is no other way to be the device the console binds to), re-expressed as commented items and cross-checked between the two dumps; no source code is copied from any of the sources, and the library's structure (tables, queue, state machine) is our own. Review check for AC2: grep of the new files for `2wiCC` finds only the sentence "no 2wiCC code", and every `0x` value in `desc.c`, `p2.c`, `report.c` has a neighbouring citation comment (a review checklist item, not a script).

**D3. Identity: exactly the descriptors of the real wired controller, except the endpoint interval (D4).**
`057E:2009`, `bcdDevice 2.10`, USB 2.00, EP0 64, one configuration (`bmAttributes 0xA0` bus powered with remote wakeup, 500 mA), one HID interface (class 3, no boot subclass), IN `0x81` and OUT `0x01`, 64 bytes, strings "Nintendo Co., Ltd." / "Pro Controller" / "000000000001", report descriptor of 203 bytes with IDs 0x30, 0x21, 0x81 (in) and 0x01, 0x10, 0x80, 0x82 (out). Matching the real device is what lets the console bind its Pro Controller driver; deviating invites a rejection that cannot be debugged without a sniffer. Remote wakeup is declared and never used. A serial shared by all units is fine: one controller per console. The identifiers are used for interoperability with the owner's console; a note goes into `THIRD_PARTY_NOTICES.md` (Task 10.4). *Alternative rejected:* our own VID/PID or strings (the console would not treat it as a Pro Controller; ADR-002 chose P2 for exactly that).

**D4. `bInterval` is a build constant, default 1 ms; the real device's 8 ms is the documented fallback and the comparison build.**
The real controller declares 8 (S8) and a Switch 1 polls it every 8 ms. The timing chain of `docs/usb-controller-emulation.md` assumes the Switch 2 may poll faster, and ADR-002 notes that RiiDraw has a "Switch 2, faster" option. 8 ms (125 Hz) is already within one 60 fps frame, but 1 ms halves the added jitter, so the default is **1**, passed as the CMake cache variable `OCB_P2_BINTERVAL` (1 to 16). The bench measures both builds (H.15 with the default, H.17 with 8) and the section "Measured on the Switch 2" records whether the console follows the descriptor or caps the rate; if the console refuses to enumerate with 1, the fallback is a rebuild with 8 and the finding is recorded (no code change needed). *Open point for the owner:* none blocks.

**D5. The handshake is reactive and `ready()` means "the host asked for the stream".**
The console drives; the profile answers (S1, S5, S6). `ready()` becomes true when streaming is enabled, which is the first of: `80 04` (S1, S7: the real controller starts the `0x30` stream after it), or subcommand `03` with argument `30` (the input-mode request, S5, S6, S3), whichever comes first. It is cleared by `80 05`, by `ocb_p2_restart` and by nothing else (the 100 ms IN-stall rule stays in `ocb_link`, 2.6 D11). The first `ready()` posts `ocb_usb_mailbox_handshake_done` once per restart. The order `80 02, 80 03, 80 02` is *answered* in whatever order and count it arrives (each gets its documented reply), never enforced, because the Switch 2's real sequence is unknown (no source) and rejecting an unexpected order would be the one way to fail the console silently. Reports are never sent before streaming is enabled (a real controller sends the `0x81` replies only). **Caution rule (Task 11):** the story's two triggers are the starting values; if H.15 shows the console never sends either and still polls, the dev changes the trigger to the observed first step, with the log as evidence in the Debug Log and in the "Measured on the Switch 2" table (and F.25 follows). *Alternative rejected:* ready at `SET_CONFIGURATION` plus the first IN poll (the P1 rule): it would report ready before the console has asked for anything, and `START` would be accepted into a console that is still sending subcommands.

**D6. Replies: exact table, one generic rule, a reply queue of 4, replies before reports.**
The three behaviour tables below are the contract. Output reports are processed as TinyUSB delivers them (report-ID byte first). `0x80`: `01` → `81 01 00 03 <MAC reversed>`; `02` → `81 02`; `03` → `81 03`; `04` → no reply, streaming on; `05` → no reply, streaming off; `06`, `91`, `92` → no reply, logged as refused or ignored (D9). `0x01`: the subcommand id at byte 10, arguments from byte 11; the reply is a `0x21` report with the current (neutral) header and the ack and data of the table. `0x10`: rumble only, consumed, no reply, no log (it would flood the ring). An id that is neither in the table nor in the refusal list gets the notes' generic reply `80 <id> 03`, so an unknown probe by the console never stalls it. The queue of 4 replies (each 64 bytes) bounds RAM; a host that sends more without reading gets `QUEUE_FULL` (dropped, counted, logged), never a stall of the stream logic. IN priority: queued reply first, then the `0x30` report. *Open (resolved cautiously):* whether the report-ID byte is passed to `tud_hid_report` as `report_id` or inside the buffer is settled by reading S11 in Task 8.1 and proven on the bench in H.14 (the reply's first byte must be `0x81`/`0x21`/`0x30` on the wire).

**D7. The emulated SPI flash is a function of the address, not an image.**
Regions served (S3, S4, S5, S9), everything else reads `0xFF` (erased flash; in particular the user calibration magic `B2 A1` is absent, so a host falls back to the factory calibration): serial at `0x6000` (16 bytes: the ASCII serial `000000000001` padded with `0xFF`), device type `0x6012` = `03`, colour flag `0x601B` = `00` (colours not stored, defaults used), IMU factory calibration `0x6020` (24 bytes: the documented default values if S4 or another public source lists them, else `00` scale-neutral values noted in the Debug Log; the IMU is disabled in effect, it reports zeros), IMU offsets `0x6080` (6 bytes), **left stick factory calibration at `0x603D` and right at `0x6046`: `00 07 70 00 08 80 00 07 70` and `00 08 80 00 07 70 00 07 70`** (centre `0x800`, range `0x700` each side, field orders of S4, values from S9), stick parameters at `0x6086` and `0x6098` (18 bytes each: the bytes of a real Pro Controller dump if S4 or a second public source shows them, otherwise the documented structure with a dead zone of about 10 % of the range and the range ratio of S4 and a Debug Log line; **UNVERIFIED**: this is the one region whose exact bytes the sources read for this story did not give, Task 1.1 settles it, H.15 shows whether the console cares). Reads are bounds-checked (length ≤ `0x1D`; D6), 32-bit addresses never wrap. *Alternative rejected:* a 128 KB constant image (flash and review cost for no gain: the sparse map is the whole truth).

**D8. Canonical state to `0x30` report: a fixed mapping, with the stick range pinned to the served calibration.**
Buttons, hat and sticks as the "Canonical state to report" table. Sticks: the console normalises with the calibration of D7, so the report must use the same range. `raw = 0x800 + (v - 0x80) * 14` per axis (so `0x00` gives `0x100`, exactly centre minus `0x700`, and `0xFF` gives `0xEF2`, 99.2 % of the range), clamped to `0x001..0xFFF`; **Y is inverted** (`raw_y = 0x800 + (0x80 - ly) * 14`) because the canonical state has `ly 0x00` = up and the Pro Controller reports up as the larger value. NEUTRAL gives `00 08 80` for each stick, the same bytes as the calibration centre. The fixed fields start from what a real wired controller was seen to send (S7): battery/connection byte `0x91`, the shared button byte carries the `0x80` bit ("charging grip") that S7's sample shows, vibrator byte `0x0A`; these three are the least certain bytes of the report, so they are listed as starting values and changed only with console evidence. The timer byte increments once per report. Reserved flag bits, button bits 14 and 15 and hat values above 8 never reach the report (the profile decides, as `state.h` says). The full-deflection claim matters: Story 1.3 measured the stick threshold for the ocarina's up/down modifier, and a range that stopped at 50 % would silently break it later.

**D9. Refusal set (C7): anything that writes, erases, pairs, resets or updates is refused and changes nothing.**
Refused with a `NACK` (ack byte `0x00`, the subcommand echoed; S2 says a NACK is always `00`), except the SPI write which answers with its documented "write protected" status (S3: `80 11 01`): SPI write `0x11`, sector erase `0x12`, BT manual pairing `0x01`, reset pairing `0x07`, HCI state `0x06` (a reboot or disconnect request must not act on us), NFC/IR MCU reset `0x20`, config `0x21` (it carries MCU firmware data), MCU state `0x22` with argument `0x02` ("resume for update", argument `00` and `01` get a plain ack). Of the USB commands: `80 06` (reset), `80 91`, `80 92` (UART pass-through to a chip we do not have) get no reply and a refusal record. There is no firmware-update subcommand in S3 beyond these; if the console sends an id the log shows as unknown (D14), the generic ack applies and the id is recorded for the next story. The refusal never touches the emulated flash (it has no writable state at all), the stream or the handshake, and increments one counter. *Alternative rejected:* silence for refused commands (a console that waits for an ack could retry forever and look hung).

**D10. Device info and the MAC come from the board, the firmware version bytes from the documents.**
Reply `82 02 <fw major> <fw minor> 03 02 <MAC> 01 <colour flag>` (S3). The MAC is derived from the RP2040 unique board id with the locally-administered bit set and the multicast bit clear, so two boards differ and none collides with a vendor MAC; the glue computes it and passes 6 bytes to `ocb_p2_init`. The firmware version is the newest value S3 documents (3.89, encoded as two bytes `major`, `minor`; Task 1.1 re-verifies the byte values against the dump in S3 before any is written), chosen so that a console comparing versions has no reason to offer an update (C7 first line of defence, D9 the second). The colour flag is `00` (D7).

**D11. The glue only translates; it adds no protocol rule and does not abort anything.**
USB suspend, unmount and bus reset post their mailbox counter; `ocb_link_poll` already turns them into `USB_SUSPENDED` aborts and a `ready` drop (2.6). The glue also restarts the profile (`ocb_p2_restart`: handshake back to the start, queue cleared, streaming off, so the next host starts clean) and sets the mailbox report period to 0. `tud_mount_cb` (SET_CONFIGURATION) also restarts, so a host that re-selects the configuration after a resume starts clean. Which TinyUSB callback fires for which event in 0.18.0 is read from the source (Task 8.1) and recorded; every one of the three events must be covered by at least one callback and none may post twice for the same event (a double post is a false second abort; the host cannot test the glue, so the Debug Log carries the reading and H.16 the proof).

**D12. The profile reads the published state through one stub function until Story 2.5.**
Story 2.4 and 2.6 deliberately left the seqlock to 2.5 (the owner of the core 1 super-loop). So `ocb_ctrl_published_read(ocb_state *out)` (in `link_runner.c`) returns `OCB_NEUTRAL` in this story, with a comment that 2.5 replaces the body with the seqlock reader (retries 3, then NEUTRAL) and `ocb_p2_next_in` already takes the state as a parameter, so 2.5 changes one function. Consequence for the bench: the console sees a connected controller with nothing pressed; pressing notes is 2.5. *Alternative rejected:* reading `core->published` directly across cores (a torn read could show a wrong note for one frame: exactly what AD-11 forbids) or building the seqlock here (scope that 2.5's AC owns, and 2.5's tests are written around it).

**D13. The median IN interval is measured on core 0, crosses in the mailbox, and feeds INFO and the LATE threshold; issue #21 stays open.**
Core 0 stamps every IN completion (the dev reads in S11 whether the timestamp can be taken in the event hook, in interrupt context, or only in `tud_task()`; the more precise one wins and the choice is recorded, because a stamp taken in the task loop includes the loop's own jitter). `ocb_p2` keeps a bounded-cost median (Task 5.1) and publishes it only when it changes, through `ocb_usb_mailbox.report_period_us`. `ocb_link_poll` applies it with the existing `ocb_link_set_report_period()` (INFO and the core's LATE threshold together, as 2.4 D6 designed: "one report period"). Until 64 samples exist, and after any USB-down event, the value is 0 (unknown, AD-7) and the LATE fallback applies. **Not done here:** confirming `OCB_CORE_LATE_FALLBACK_US = 8000`, deciding the margin of LATE against the measured period, measuring jitter (not just the median) under playback, and the core 1 stack: Story 2.5, issue #21 (this story gives it the number and one more reason to re-measure the stack, because STATUS grew, Task 7.3). The test plan row H.15 records the median, and H.17 compares the two `bInterval` builds.

**D14. The debug log leaves the board in the debug STATUS, so no stdio and no new message are needed.**
The AC says each subcommand is "logged through the SPSC ring (not in release builds)". The ring (AD-11: debug logs go core 0 to core 1 through an SPSC ring that drops when full) carries `uint16_t` records (kind, id); core 1 drains it into a window of the newest 16 records and a `u16` total, which is appended to the debug STATUS as **v3** counters (`p2_log_total u16`, `p2_log_n u8`, `n` × 2 bytes). A reader sampling STATUS every 50 ms plus after ACKs sees the whole handshake unless more than 16 records arrive between two STATUS frames (the total shows a gap). Kinds are listed in Task 7.2. In release builds the ring, the drain call, the window and the push in the glue do not exist (`#ifdef OCB_DEBUG_BUILD` in the glue, `debug_counters` false in the link), proven by a symbol check on the release ELF (Task 8.7, AC5a). No wire value changes: STATUS already allows trailing debug counters that every decoder ignores, and AD-19 says a later debug layout may append more. *Alternatives rejected:* a new message type `LOG` (touches `ocb_proto`, the vectors and the Rust codec for a debug aid), abusing EVENT codes (pollutes `STATUS.last_event`), and stdio over USB (it is the device being debugged).

**D15. `fw-debug-bench` is superseded and removed.**
Story 2.7 D17 added a debug-only build that fakes `console_ready` so DIRECT and PLAYING could be tested without a console, and said Story 2.3 "removes this option or keeps it for the same bench purpose". The real `ready()` is now available on the bench itself: the controller Zero plugged into the Mac is a USB host that can complete the handshake, either by itself (macOS has a Pro Controller driver; H.13 observes whether it does) or through `p2_probe.py handshake --hold N`, one command in the same shell line as the probe step. So the fake is no longer needed, and two sources of `ready` is a hazard (a build that claims a ready console without one must never exist; the bench build already needed a "never flashed for real" rule and a CI gate). Removed: `OCB_BENCH_FAKE_READY` (CMake option, main.c code, comment), the preset, the CI step, the docs; `link_probe.py direct` and `playing` stop skipping with exit 2 and instead need `console_ready` 1 from a real host. H.11 is re-run on the real stack (Task 11.3). *Alternative rejected:* keep it "for convenience" (it duplicates the real path, the rule that guards it costs a CI step, and the real path is now one command away). *Fallback:* if `hidapi` cannot open the device on the owner's Mac and macOS does not handshake by itself, H.11 is re-run on the console chain (H.15 setup), where the real console provides the ready; no code is kept for that case.

**D16. A fourth purity gate for `ocb_p2`, copied from the link gate, drift-guarded; issue #20 is not touched.**
The profile must stay free of the SDK and TinyUSB or it cannot be tested on the PC (AD-2; the AC set asks for the module to run in ctest). The gate follows the established pattern: allowed includes narrower than the link's (`ocb_proto` and `ocb_p2` only: the profile must never see the core or the link), the same banned-identifier line byte for byte, a self-test with fixtures and a drift guard against the core gate's line. Issue #20 (some identifiers such as `exit`, `abort`, `fopen`, inline `asm` are not matched by any gate) is a change of the shared list that must hit all gates at once; doing it here would mix a maintenance fix into a hardware story. Because the new self-test compares the list with the core gate's, fixing #20 later in one place cannot leave this gate behind. Not closed, not changed.

**D17. Fuzz-friendliness is a design rule: one entry point, lengths checked first, no state outside the struct.**
`ocb_p2_on_output` takes `(buf, len)`, checks `len` against each field's offset before reading it, treats missing fields as zero or as `MALFORMED` (a documented table), never indexes with a host value without a bound, and writes only into the reply slot (fixed 64 bytes, filled by a builder that takes its capacity). There are no globals, no asserts, no `static` buffers, no allocation. The ctest fuzz of Task 5.3 (**F.28**) is deterministic (3 seeds, as 2.6 F.17) and runs under ASan and UBSan; a libFuzzer entry is provided but optional and outside CI (a clang-only target would make the pinned-toolchain CI depend on a second compiler).

**D18. Host-side tools: `p2_probe.py` (the Mac as the USB host) and new `link_probe.py` steps (the console, read over the link).**
The Mac side needs no console and no second board and answers "do the bytes work". `enum` uses only the macOS tools (`system_profiler`, fallback `ioreg`), stdlib only, so it runs anywhere the owner has Python 3 (3.9.6 on the bench Mac). `handshake` needs `hidapi` (`pip install hidapi`), the same kind of bench-machine dependency as `pyserial`. Both are written from the cited sources (the tool is the one independent implementation of the host side, as `link_probe.py` is of the link), have no hard-coded serial port or device path (the HID device is found by VID:PID; ports stay `"$P"`), and are tested in ctest against the real `ocb_p2` through a shared module (**F.32**), so a wrong byte in either implementation is caught on the PC. Known macOS caveats, to be recorded and not "fixed": the first HID open may need the Terminal to have Input Monitoring permission (one prompt); macOS may itself talk to a Pro Controller (its own handshake and subcommands will then interleave with the probe's, visible in the debug log; the probe says so if it sees unexpected traffic). *Alternatives rejected:* `pyusb` (needs libusb and a driver dance on macOS for a device already claimed by the HID driver), a shell-only check with `ioreg` (cannot do the handshake).

**D19. The bench goes Mac first, console second, one command and few physical actions each.**
H.13 (plug, nothing else), H.14 (nothing else), then the console: H.15 needs one plug-in, H.16 four actions (sleep, wake, setting off, setting on; plus two with `--dock`), H.17 one reflash. Every script prints what to do in one prominent line, states the time it will wait (90 to 120 s: the human is not hurried), absorbs bounces (a stable window before checking, as 2.7 D42), prints `PASS` or `FAIL <reason>` and never asks the owner to read a number off a screen: the numbers come back over the link. The environment (Switch 2 system 23.5.0, OoT 1.1.1, top USB-C port, Wired Communication ON, Apple reference cable) is read from `docs/test-plan.md`, not asked.

**D20. Core 0 stays a plain loop; TinyUSB is polled, not driven by an RTOS; the LED keeps blinking.**
`main()` keeps its LED half-period logic and adds one `usb_p2_task()` per pass (`tud_task()`, `ocb_p2_tick()`, the kick). No sleeps, no blocking calls (AD-11 forbids work on core 0 that could starve the report pump). The LED write is a non-blocking PIO FIFO push (2.1), so one pass stays short; if the bench shows the LED blink drifting or an enumeration failing because the loop is slow, the finding is recorded (Story 2.5 rebuilds core 0 around the report pump anyway). TinyUSB code runs from flash (XIP); only the SDK's USB interrupt handler is in RAM; Story 2.5 decides whether to move more.

**D21. `OCB_PROFILE` exists and accepts only `P2`.**
The AC names `OCB_PROFILE=P2 (default)`. A CMake cache STRING `OCB_PROFILE` (default `P2`) selects the profile library and the `INFO.hid_profile` value; `P1` and `P3` stop configuration with a message saying they are AD-12 slots not built in v1 (R6). No `#ifdef` in the pure libraries.

**D22. Documentation updates are part of the story, with the evidence-first rule for the status columns.**
The reference section and the measured table (Task 1.2, Task 11.4), the test plan rows (Task 10.1), `link-protocol.md` (the STATUS v3 tail and the report period), the READMEs and notices (Task 10.4). The risk register's C7 and R13 status cells change only with evidence from the bench (Task 10.5), never before.

**D23. Rumble is consumed and ignored.**
Output report `0x10` and the rumble bytes of `0x01` are parsed for length only. No haptics, nothing stored, no log record for `0x10` (it can arrive at the stream rate).

**D24. No protocol change.**
`PROTO_VERSION` stays 1; no message, NAK reason, EVENT code or constant is added; `common/proto/**` and the conformance vectors are untouched. STATUS gets a longer debug tail (D14), which the format has always allowed; `TX_BODY_MAX` grows because of it. `docs/link-protocol.md` documents the tail.

**D25. What the console really sends is an unknown that the story measures instead of guessing.**
No public source fixes the Switch 2's sequence (the dekuNukem and SDL notes describe hosts, and the Switch 1 sequence is documented only by what a controller sees). So the reply logic is reactive and tolerant (D5, D6), the debug log records the real sequence (D14), `p2check` prints it, and Task 11 writes it into "Measured on the Switch 2". The only allowed late change is D5's caution rule (change the ready trigger to the observed step) and the three uncertain report bytes of D8; anything larger is an open point for the owner, not a silent rewrite.

### Behaviour reference (the tables the tests encode)

Each row: what the profile does. **Src** = the S-number (Sources table). **UNVERIFIED** rows are the ones Task 1.1 must settle.

**USB `0x80` commands** (OUT report id `0x80`, byte 1 = command; replies are IN report id `0x81` zero padded to 64 bytes):

| Cmd | Meaning | Reply | Effect | Src |
|---|---|---|---|---|
| `01` | status / MAC request | `81 01 00 03 <MAC reversed, 6 bytes>` | none | S1, S6 |
| `02` | handshake | `81 02` | none (any count, any order) | S1, S5 |
| `03` | baud switch | `81 03` | none (the line stays USB; the host's follow-up `02` is answered) | S1, S5 |
| `04` | stay in USB HID mode, no timeout | none | streaming on (D5), handshake complete | S1, S5, S7 |
| `05` | allow timeout | none | streaming off, `ready()` false | S1, S7 (**the wording differs between sources**; cautious reading) |
| `06` | reset | none | refused, log `0x04` | S1 |
| `91`, `92` | UART pass-through | none | refused, log `0x04` | S1 |
| other | unknown | none | ignored, log `0x01` | |

**Subcommands** (OUT report id `0x01`: `[1]` counter, `[2..9]` rumble, `[10]` id, `[11..]` arguments; replies are IN report id `0x21`: `[1]` timer, `[2]` `0x91`, `[3..5]` buttons, `[6..11]` sticks (all neutral), `[12]` vibrator byte, `[13]` ack, `[14]` id, `[15..]` data up to 35 bytes, zero padded to 64):

| Id | Name | Ack | Data | Effect | Src |
|---|---|---|---|---|---|
| `01` | BT manual pairing | `00` (NACK) | none | refused (C7) | S3 |
| `02` | device info | `82` | `<fw major> <fw minor> 03 02 <MAC, 6 bytes> 01 <colour flag 00>` | none | S3 (**fw bytes UNVERIFIED**, D10) |
| `03` | input report mode | `80` | none | any mode: ack; `30` (or any) starts the `0x30` stream (D5) | S3, S5 |
| `04` | trigger elapsed time | `83` | 14 bytes (seven u16, all 0) | none | S3, S9 |
| `06` | HCI state | `00` (NACK) | none | refused | S3 |
| `07` | reset pairing | `00` (NACK) | none | refused (C7) | S3 |
| `08` | shipment low power | `80` | none | none | S3 |
| `10` | SPI read | `90` | `<addr LE32> <len> <len bytes>`, `len ≤ 0x1D` (above: NACK) | none | S3, S4, S5 |
| `11` | SPI write | `80` | `01` (write protected) | refused (C7) | S3 |
| `12` | SPI sector erase | `00` (NACK) | none | refused (C7) | S3 |
| `20` | reset MCU | `00` (NACK) | none | refused | S3 |
| `21` | MCU config | `00` (NACK) | none | refused (firmware-update path) | S3 |
| `22` | MCU state | `80` for argument `00` and `01`; `00` (NACK) for `02` | none | `02` refused (resume for update) | S3 |
| `30` | player lights | `80` | none | argument stored, log | S3 |
| `38` | HOME light | `80` | none | none | S3 |
| `40` | IMU enable | `80` | none | flag stored; the report carries zeros either way | S3 |
| `41` | IMU sensitivity | `80` | none | none | S3 |
| `48` | vibration enable | `80` | none | flag stored; rumble ignored (D23) | S3 |
| other | unused | `80` | `03` after the id as the generic reply (the reply carries the id) | none | S3 |

`0x10` (rumble only): no reply. A reply is only built when the whole subcommand header (11 bytes) is present; a shorter `0x01` report is `MALFORMED` (counted, no reply).

**SPI regions served** (D7; reads outside are `0xFF`):

| Address | Len | Content | Src |
|---|---|---|---|
| `0x6000` | 16 | ASCII serial, `0xFF` padded | S4 |
| `0x6012` | 1 | `03` (Pro) | S4 |
| `0x601B` | 1 | `00` (no colours stored) | S4 |
| `0x6020` | 24 | IMU factory calibration (documented defaults, **UNVERIFIED**) | S4, S5 |
| `0x603D` | 9 | left stick: `00 07 70 00 08 80 00 07 70` | S4 (layout), S9 (values) |
| `0x6046` | 9 | right stick: `00 08 80 00 07 70 00 07 70` | S4, S9 |
| `0x6080` | 6 | IMU offsets (documented defaults) | S4 |
| `0x6086`, `0x6098` | 18 | stick parameters (**UNVERIFIED**, D7) | S4, S5 |
| `0x8010`..`0x8025` | | `FF` (user calibration absent: no magic) | S4, S6 |

**Report `0x30`** (IN, 64 bytes): `[0]` `30`; `[1]` timer (+1 per report); `[2]` `91`; `[3]` right buttons; `[4]` shared buttons; `[5]` left buttons; `[6..8]` left stick (two 12-bit values, X then Y); `[9..11]` right stick; `[12]` vibrator byte; `[13..48]` IMU, 36 zero bytes; `[49..63]` zero. Stick packing: `b0 = x & 0xFF`, `b1 = ((x >> 8) & 0x0F) | ((y & 0x0F) << 4)`, `b2 = y >> 4` (S2, S5). Src: S2, S5, S7.

**Canonical state to report** (D8; canonical bit order of AD-10, P1 order):

| Canonical | Pro Controller | Byte and bit |
|---|---|---|
| bit 0 Y | Y | right `[3]` `0x01` |
| bit 1 B | B | right `[3]` `0x04` |
| bit 2 A | A | right `[3]` `0x08` |
| bit 3 X | X | right `[3]` `0x02` |
| bit 4 L | L | left `[5]` `0x40` |
| bit 5 R | R | right `[3]` `0x40` |
| bit 6 ZL | ZL | left `[5]` `0x80` |
| bit 7 ZR | ZR | right `[3]` `0x80` |
| bit 8 Minus | Minus | shared `[4]` `0x01` |
| bit 9 Plus | Plus | shared `[4]` `0x02` |
| bit 10 L3 | left stick click | shared `[4]` `0x08` |
| bit 11 R3 | right stick click | shared `[4]` `0x04` |
| bit 12 Home | Home | shared `[4]` `0x10` |
| bit 13 Capture | Capture | shared `[4]` `0x20` |
| bits 14, 15 | none | never reported |
| hat 0 up, 1 up-right, 2 right, 3 down-right, 4 down, 5 down-left, 6 left, 7 up-left | D-pad | left `[5]`: Down `0x01`, Up `0x02`, Right `0x04`, Left `0x08` (diagonals set two bits); 8 and 9 to 255: none |
| lx, rx | stick X | `raw = clamp(0x800 + (v - 0x80) * 14, 1, 0xFFF)` |
| ly, ry | stick Y | `raw = clamp(0x800 + (0x80 - v) * 14, 1, 0xFFF)` (up = larger) |
| fixed | | `[2] = 0x91`, shared `[4] \|= 0x80`, `[12] = 0x0A` (D8 starting values from S7) |

Src: S2 for the bit assignment, S7 for the fixed fields, D8 for the scaling.

**Host sequence used by `p2_probe.py handshake`** (the probe's own, from S5 and S6; each step waits for its reply, resend after 300 ms, up to 3 times):

| # | Sends | Expects |
|---|---|---|
| 1 | `80 01` | `81 01 00 03` + 6 bytes |
| 2 | `80 02` | `81 02` |
| 3 | `80 03` | `81 03` |
| 4 | `80 02` | `81 02` |
| 5 | `80 04` | nothing; then `0x30` reports begin within 1 s |
| 6 | `01` subcommand `02` | `21 .. 82 02 ...` device info with type `03` |
| 7 | subcommand `10` at `0x6020` (24), `0x6080` (6), `0x603D` (18), `0x6086` (18), `0x6000` (16), `0x8010` (22) | `90 10`, same address and length, data of the table |
| 8 | subcommands `30` (`01`), `40` (`00`), `41`, `48` (`01`), `03` (`30`) | ack `80` and the id |
| 9 | refusal probe: `11` write at `0x8000`, then SPI read `0x8000` | write: ack `80 11 01`; the read is unchanged (`FF`) |
| 10 | unlisted id `7F` | ack `80 7F 03` |
| 11 | read `0x30` reports for `--seconds` | every one starts `30`, timer +1 each, NEUTRAL fields, count and median interval printed |

### Consumers (what later stories need from this one)

- **Story 2.5**: replaces `ocb_ctrl_published_read()` with the seqlock reader and the core 1 loop (still calling `ocb_ctrl_link_step()`); confirms or replaces `OCB_CORE_LATE_FALLBACK_US` with the measured period from H.15 and H.17 and re-measures the core 1 stack (issue #21: STATUS grew by up to 35 bytes in debug builds, release unchanged, Task 7.3); builds the jitter measurement on top of the IN-completion stamps and the log.
- **Story 3.1 and 3.4**: nothing new; `console_ready` is now real, so `START` works when a console is attached. The STATUS tail is ignored by readers that know only v1.
- **Story 3.5**: A.3 and the `console_ready` checks use H.15 and H.16.
- **Story 4.x**: `STATUS.flags.console_ready` is now fed by a real profile; the "controller to Switch" indicator can use it.
- **Story 8.5**: the wizard's "Wired Communication" step can quote H.16 (what happens when the setting is off: ready falls within about 100 ms of the last poll).
- **Story 11.2**: the notices file lists `ocb_p2`'s documentation sources.

### Developer guardrails

- **Read before editing (UPDATE files):** `firmware/CMakeLists.txt`, `firmware/CMakePresets.json`, `firmware/controller/CMakeLists.txt`, `controller/main.c`, `controller/platform/link_runner.{c,h}`, `controller/link/include/ocb_link/{ready.h,link.h,config.h,internal.h}`, `link/{ready.c,link.c,tx.c}`, `firmware/tests/{CMakeLists.txt,link_fixture.h,link_sim_shim.c,test_link_ready.c,test_link_session.c,test_link_tx.c,test_link_fuzz.c,test_link_probe.py,test_docs_constants.py}`, `firmware/tools/link_probe.py`, `firmware/tools/check-link-pure.sh` (as the template, not edited), `.github/workflows/firmware.yml`, `docs/{usb-controller-emulation,link-protocol,test-plan}.md`, `firmware/{README.md,controller/README.md,THIRD_PARTY_NOTICES.md}`. Preserve everything not named here.
- The system must keep working end to end: both UF2s still build in `fw-release` and `fw-debug`, the UART link and watchdog behave exactly as in Story 2.7 (H.8 to H.10, H.12 must still pass when re-run), the LED still blinks amber at 2 Hz (H.0).
- Every byte the profile sends is built by a bounds-checked builder; never cast a byte buffer to a struct (alignment, endianness); `-Wconversion` is on, cast `uint8_t` arithmetic explicitly.
- No `printf` or stdio on the controller (the UART pins are the link). Debug information leaves only through the debug STATUS.
- The pin grep (`check-pins.sh`) prunes only `tests/pin_check_fixtures` and `common/proto/vectors`: write no `GP<n>` or `GPIO<n>` token in any comment of the new files (USB uses dedicated pads; no pin is configured by this story).
- Core 0 is the only core that touches TinyUSB; core 1 never includes a TinyUSB header (the link and the glue talk through the mailbox and the ring only).
- Do not read 2wiCC code or text; do not copy from the sources, cite them (D2). Do not copy from `third_party/pico-examples` or from the TinyUSB examples (reading the example for the shape of `tusb_config.h` is fine, the file is yours).
- The profile never calls into the scheduler or the core; it reads only the `ocb_state` it is given (AD-12).
- Do not create temp files in the repo; do not commit or push; do not edit `_bmad/`.

### Architecture compliance

- AD-1: TinyUSB 0.18.0 from the pinned submodule, `PICO_TINYUSB_PATH` not set, no new dependency.
- AD-2: `ocb_p2` pure C99 with its own gate (D16) and ctest coverage; the glue is thin.
- AD-7: `INFO.hid_profile` from `OCB_PROFILE` (2 for P2); `report_period_us` is the measured median, 0 if unknown; no version bump.
- AD-10: the canonical 8-byte state is mapped by the profile only (D8).
- AD-11: core 0 = TinyUSB + profile + report sending; core 0 to core 1 through the mailbox (single writer per counter) and the debug SPSC ring (drops when full, never blocks); the state read is the stub of D12 until 2.5's seqlock.
- AD-12: the five-part interface (D1); one profile per build; reads only the published state; refuses firmware-update subcommands (D9); restarts its handshake after a bus reset (D11); debug logging through the ring only (D14).
- AD-19: `console_ready` is the profile's `ready()` through the mailbox and the stall rule (unchanged); the 100 ms stall clears the flag only.
- ADR-002, ADR-004, NFR-07: clean-room (D2), no 2wiCC code.
- ADR-006, R13: re-enumeration after dock and wake (AC4, H.16).

### Library and framework requirements

Firmware: pico-sdk 2.3.1 and TinyUSB 0.18.0 as pinned (nothing fetched; `tinyusb_device` and `pico_unique_id` from the SDK), C99 for the pure libraries, no test framework (Story 2.2 D7). Host tools: Python 3 standard library for `enum` and for all tests (the bench Mac has 3.9.6; no f-string `=` specifier, no `match`, no `X | Y` type syntax), `hidapi` only for `p2_probe.py handshake` on the owner's bench machine, `pyserial` as before for `link_probe.py`. macOS tools used by `enum`: `system_profiler` and `ioreg`.

### File structure requirements

```text
firmware/controller/profile_p2/                 (NEW, pure C99, library ocb_p2)
  CMakeLists.txt
  include/ocb_p2/config.h  desc.h  p2.h
  desc.c  p2.c  report.c  interval.c
firmware/controller/platform/
  usb_p2.c  usb_p2.h  tusb_config.h             (NEW, glue, core 0)
  (UPDATE) link_runner.c  link_runner.h
firmware/controller/main.c  CMakeLists.txt      (UPDATE; bench build removed)
firmware/controller/link/
  include/ocb_link/dbglog.h  dbglog.c           (NEW, pure)
  (UPDATE) include/ocb_link/ready.h  ready.c  link.c  tx.c  config.h  internal.h
firmware/CMakeLists.txt  CMakePresets.json      (UPDATE: profile_p2, OCB_PROFILE, no bench option or preset)
firmware/tools/
  check-p2-pure.sh  check-release-nolog.sh  p2_probe.py   (NEW)
  (UPDATE) link_probe.py
firmware/tests/
  test_p2_desc.c  test_p2_handshake.c  test_p2_subcmd.c  test_p2_report.c
  test_p2_interval.c  test_p2_fuzz.c  test_link_dbglog.c  test_p2_probe.py   (NEW)
  p2_pure_selftest.sh  p2_pure_fixtures/{bad,good}/...  fixtures/*.json     (NEW)
  p2_sim_shim.c                                                              (NEW, ctypes module)
  fuzz/fuzz_p2.c                                                             (NEW, optional, not built by default)
  (UPDATE) CMakeLists.txt  link_fixture.h  link_sim_shim.c  test_link_ready.c  test_link_session.c
           test_link_tx.c  test_link_fuzz.c  test_link_probe.py  test_docs_constants.py
.github/workflows/firmware.yml                  (UPDATE: p2 gate, release-nolog gate; bench step removed)
docs/usb-controller-emulation.md  link-protocol.md  test-plan.md        (UPDATE)
firmware/README.md  controller/README.md  THIRD_PARTY_NOTICES.md        (UPDATE)
_bmad-output/specs/spec-bonooru/risk-register.md   (UPDATE after Task 11: C7, R13 status cells)
```

Not touched: `common/proto/**` (consts, vectors), `common/hw/include/ocb_hw/pins.h`, `bridge/**`, `controller/core/**` (the core is not modified by this story), `_bmad/**`, the three existing purity gate scripts and their fixtures, `docs/ocarina-input-mechanics.md`.

### Testing requirements

All under ctest `host-tests`, ASan and UBSan on, hand-written expectations from the cited sources (never derived from the code under test), frames built by the tests' own tables.

| Test (ctest name) | Test-plan ID | Must cover |
|---|---|---|
| `test_p2_desc` | **F.24** (new) | Task 2.3 |
| `test_p2_handshake` | **F.25** (new) | Task 3.3 |
| `test_p2_subcmd` | **F.26** (new) | Task 3.4 (includes AC3) |
| `test_p2_report` | **F.27** (new) | Task 4.2 |
| `test_p2_fuzz` | **F.28** (new) | Task 5.3 |
| `test_p2_interval` | **F.29** (new) | Task 5.2 (AC5b algorithm) |
| `p2_pure_check` | **F.30** (new) | Task 6.2 |
| `test_link_dbglog` | **F.31** (new) | Task 7.5 (AC5a, report period path) |
| `p2_probe_selftest` | **F.32** (new) | Task 9.3 |
| `test_link_ready`, `test_link_session`, `test_link_tx`, `test_link_fuzz` | **F.10, F.15, F.16, F.17** | Task 7.5 (updated, meaning kept) |
| `link_probe_selftest`, `docs_constants_check`, `link_pure_check`, `core_pure_check`, `proto_pure_check` | F.19 to F.23, F.18, F.13 | must stay green; F.23's "ready console for the bench build" becomes a completed-handshake host |

Red proofs to record in the Debug Log: the refusal switched off (F.26), a wrong stick scaling (F.27), a wrong `bInterval` byte (F.24), a missing `restart` clearing the queue (F.25), a push in a release configuration (F.31), a wrong reply byte in `p2_probe.py` (F.32). CI mapping: "Host tests" runs all of it; the two new steps run the gates; the three ARM presets compile the glue with the strict warnings (`-Wpedantic -Wshadow -Wconversion` on `usb_p2.c` too, except where TinyUSB headers trigger them: scope the flags to our file, not to the SDK headers, with `#pragma GCC diagnostic` around the includes if needed).

### Hardware checks (owner bench session; Task 11)

**Rig (record in the test-plan bench block):** Mac (macOS, Darwin 27.x); the controller Zero with the new `fw-debug` UF2 (`build/fw-debug/controller/controller.uf2`; the release UF2 for the last re-check of AC5a is not needed on the bench); the Apple USB-C reference cable; for H.15 to H.17 the Spike 0b chain: controller Zero in the console's top USB-C port, second Zero with 2wiCC_Comms as the bridge on the Mac (its port name changes at replug: `P=/dev/cu.usbmodemXXXX`), crossed TX/RX and GND as left after 2.6, nothing on 5V or 3V3, the console with Wired Communication ON. Wiring is **not rechecked** (breadboard GND/GP0/GP1 rule); if rewired, the GND continuity check applies again. Controller builds are local, `OCB_PICOTOOL_DIR=firmware/.tools/picotool-2.3.1`, `OCB_ARM_TOOLCHAIN=firmware/.tools/arm-gnu-toolchain-15.2.Rel1`. Flash by BOOTSEL. Python: a bench venv with `pip install pyserial hidapi`. All commands run from the repo root; `P` is set once with `export P=/dev/cu.usbmodemXXXX` and never typed into a command.

| ID | Test | Exact steps (owner actions in **bold**) | Expected | Record |
|---|---|---|---|---|
| H.13 | The Mac sees the Pro Controller (AC1, AC4, no console) | 1. Flash fw-debug on the controller Zero, then **unplug it**. 2. `python3 firmware/tools/p2_probe.py enum --wait-s 90`. 3. It prompts: **plug the controller Zero into the Mac with the Apple cable**. | `PASS`: `057E:2009` seen stable for 3 s, strings "Nintendo Co., Ltd." / "Pro Controller" / "000000000001", `bcdDevice 2.10`, HID report descriptor equal (or one `SKIP` line if `ioreg` does not expose it); LED amber 2 Hz still blinks (H.0 not regressed) | the printed lines, the macOS version, whether macOS shows a controller in System Information, the LED observation |
| H.14 | Host-driven handshake on the Mac (AC2, AC3, AC5a) | 1. Same board, plugged in. 2. `python3 firmware/tools/p2_probe.py handshake --seconds 10`. 3. If macOS asks for Input Monitoring for the Terminal: **allow it once** and rerun. 4. `python3 firmware/tools/link_probe.py --port "$P" watch --seconds 5` only if the bridge Zero is wired (optional here; the controller's UART peer is not needed for the handshake). | `PASS` for steps 1 to 11 of the host sequence table: every reply byte-exact, the refused write leaves `0x8000` unchanged, `0x30` reports with the timer +1 each, count about 10 s at the measured rate, host-side median interval printed. If macOS talks by itself, the probe prints that it saw unexpected replies and what they were; that is a finding to record, not a failure, unless a reply is wrong | the printed lines, the host-side median interval and count, whether macOS did its own handshake |
| H.15 | Console enumerates and handshakes; the polling rate (AC1, AC5a, AC5b) | 1. Controller on fw-debug with the default `bInterval` (1). 2. Bridge Zero wired and on the Mac (`P` set). 3. Console on, Wired Communication ON. 4. `python3 firmware/tools/link_probe.py --port "$P" p2check --wait-s 120`. 5. It prompts: **plug the controller into the console's top USB-C port**. 6. Look at the console: **a controller icon appears** (one glance, no menu). | `PASS`: INFO `hid_profile` 2, `console_ready` 1, the decoded handshake log printed in order (this is the console's real sequence: record it whole), `report_period_us` in 500 to 20000 after 5 s of polling, `abort_count` unchanged; one pasteable line with the median | the printed sequence, the median, the console and system version (23.5.0), the cable and port, the build commit |
| H.16 | Re-enumeration: sleep, wake, Wired Communication off and on, dock (AC4, R13) | 1. After H.15, `python3 firmware/tools/link_probe.py --port "$P" p2watch`. 2. It prompts one action at a time with its time budget (90 s each): **put the console to sleep**; **wake it**; **set Pro Controller Wired Communication OFF**; **set it back ON**. 3. Optional `p2watch --dock`: **undock**, **dock**. | `PASS` per step: on sleep either `console_ready` 0 with `EVENT USB_SUSPENDED` or STATUS stops (port power cut; the script prints which); after wake the log shows a restart then a new handshake and `console_ready` 1 within 30 s; Wired Communication OFF drops the flag within 150 ms of the last IN report (printed), ON restores it; dock and undock the same; `abort_count` rises by 1 per USB-down event and nothing hangs (no reflash or unplug needed) | the printed lines and times, which sleep behaviour was seen |
| H.17 | The comparison build, `bInterval` 8 (AC5b, optional) | 1. `cmake --preset fw-debug -DOCB_P2_BINTERVAL=8 && cmake --build --preset fw-debug`. 2. **Flash it by BOOTSEL**, plug into the console. 3. `python3 firmware/tools/link_probe.py --port "$P" p2check`. 4. Re-flash the default build afterwards. | `PASS`; the median differs from H.15 (the console follows the descriptor) or equals it (the console caps the rate): both are results | the median for 8 and for 1, what it means for the LATE threshold (hand-off to Story 2.5) |
| H.11 (re-run, changed) | DIRECT and PLAYING rules on the real stack (Story 2.7 AC2) | 1. fw-debug on the controller plugged into the Mac, bridge Zero wired. 2. In one shell line: `python3 firmware/tools/p2_probe.py handshake --hold 120 & python3 firmware/tools/link_probe.py --port "$P" linkup --seconds 5 --expect-ready yes && python3 firmware/tools/link_probe.py --port "$P" direct && python3 firmware/tools/link_probe.py --port "$P" playing; wait`. | all `PASS` as in Story 2.7; `console_ready` 1 comes from the real handshake | the printed lines; this retires the bench build |

Safety: nothing on 3V3 or 5V; USB cables only as above; the console's firmware-update and pairing screens are not opened except as H.16's optional last step ("Update Controllers", read-only look: expected no update offered or a refused one; record what the screen says, one sentence).

What to record, once, in `docs/test-plan.md`: rows F.24 to F.32 (Result) and H.13 to H.17 (Result column) with the bench block (commit, UF2 build commit, macOS version, hidapi version, Python version, cable, port, the console's system version), and in `docs/usb-controller-emulation.md` "Measured on the Switch 2" (sequence, medians, sleep behaviour).

### Previous story intelligence (Story 2.7 done; PRs up to #19, merged)

- The pure-library pattern, the purity gate copy with a drift guard, the probe as the independent implementation tested through a ctypes shared module, and the one-command bench steps with `PASS` and `FAIL` lines are all established; this story reuses them rather than inventing new machinery (D16, D18).
- Hand-offs from 2.7 addressed here: `fw-debug-bench` "removed or kept by 2.3" → D15; `console_ready` and the USB aborts "untouched" → D11; `H.3` can reuse `linkup --expect-ready yes` → used in H.11, H.15; `report_period_us` always 0 → D13.
- Hand-offs from 2.6: the USB mailbox (single writer per counter), `ocb_ready` and the 100 ms stall rule are ready and unchanged; `ocb_ctrl_usb_mailbox()` "nothing posts yet" is now the glue; `ocb_link_ready_rebase` already handles posts made before core 1 is up (so `usb_p2_init()` may post before `multicore_launch_core1`).
- Hand-offs from 2.4: the profile calls nothing in the core; `core->published` + `publish_count` are 2.5's seqlock input (D12); the LATE threshold is one report period (D13).
- Review lessons that apply: (1) every new rule needs a red proof in the Debug Log (Testing requirements lists them); (2) probe options that could stall a run are rejected before anything opens (argparse guards, as 2.6 D36); (3) the bench tool and the firmware behaviour are reproduced on the host with a fake clock (F.32); (4) honest Completion Notes ("unverified until pushed", "unverified on hardware"); (5) the wire and the bench peer drop silently, so the Mac-side tools resend requests (300 ms, 3 times) instead of assuming delivery; (6) the owner has ADHD: few steps, one command each, the prompts say what to do and how long the script waits.
- Facts to rely on, not re-derive: `link_ok` and the watchdog are independent of USB; `START` and `SET_STATE` precedence is `ABORTED → NO_LINK → BUSY → NOT_READY → ...`; `ocb_ready_is` allows exactly 100 ms between IN reports; the mailbox counters wrap without a false event; the STATUS debug tail is ignored by every decoder.

### Git intelligence

Recent commits on `main`: `9af451e` merge of PR #19 (Story 2.7), `98bec54` final H.12 PASS recorded, `2fdac9e` settle window after each replug, `b41d65e` paced resends after NAKs, `a828b85` resend link_probe requests. Pattern: one story branch (this one: `story/2-3-controller-enumerates-as-a-wired-pro-controller`), an implementation commit plus review follow-up commits, bench sessions recorded in later commits, merge by PR; the owner commits and pushes. `firmware/controller/` today holds `core/` (2.4), `link/` and `platform/` (2.6, 2.7), `main.c` (LED on core 0, link loop on core 1, the bench-only fake ready). The tree has no USB code yet; `.claude/` is untracked and not part of the story.

### Latest tech information

Read from the pinned TinyUSB 0.18.0 sources (`hid_device.c`): OUT endpoint data reaches `tud_hid_set_report_cb(instance, 0, HID_REPORT_TYPE_OUTPUT, buffer, bufsize)` with `buffer` still holding the report-ID byte first; `tud_hid_n_report(instance, report_id, report, len)` writes `report_id` as the first byte of the IN buffer when it is non-zero, so a payload that already starts with its id must go with `report_id` 0 or the glue must strip it (decide by reading and by the bench: the first byte on the wire must be `81`, `21` or `30`); `CFG_TUD_HID_EP_BUFSIZE` must be 64 for these reports; `tud_hid_report_complete_cb` marks the end of an IN transfer (the poll that took the report); `usbd.c` handles `DCD_EVENT_BUS_RESET`, suspend and resume; `tud_event_hook_cb` exists for events, and which of the callbacks fires on a reset is to be read, not assumed. pico-sdk 2.3.1 provides `tinyusb_device`; the 1200-baud reset helper of AD-13 is for the bridge, not here. Toolchain pins unchanged (Arm GNU Toolchain 15.2.Rel1, CMake ≥ 3.25, pico-sdk 2.3.1, TinyUSB 0.18.0). macOS: `system_profiler SPUSBHostDataType -json` works on the bench Mac (Darwin 27.2) and lists USB buses with nested `_items`; `SPUSBDataType` is the older name; the dev reads the real key names (vendor and product id fields, strings) before writing the parser.

### Project Context Rules

No `project-context.md` exists in the repo. Project rules that apply, from the owner's notes and the repo: all repo files in English (chat in Italian); never commit `_bmad/` or `bmad-*`/`gds-*` skills (only `_bmad-output/`); never edit `~/Documents/org/`; no game screenshots, never reference `ux-designs/*/imports/`; no binaries and no 2wiCC code (NFR-07, ADR-004); do not create temp files in the repo; the dev agent does not commit or push; breadboard GP0/GP1/GND wiring is not rechecked each session unless rewired; the test environment (Switch 2 system version, OoT version, console port, Wired Communication setting) is read from `docs/test-plan.md`, never asked; research before raising open questions; ask one decision at a time (the owner has ADHD: keep the bench steps few, simple and one command each).

### References

- [Source: `_bmad-output/planning-artifacts/epics.md`#Story 2.3, #Story 2.5, #Story 2.7, #Story 3.1, #Story 3.5, #Epic 2 Notes]
- [Source: `_bmad-output/planning-artifacts/architecture/architecture-bonooru-2026-11-19/ARCHITECTURE-SPINE.md`#AD-1, AD-2, AD-7, AD-10, AD-11, AD-12, AD-19, Structural Seed]
- [Source: `_bmad-output/specs/spec-bonooru/decisions.md`#ADR-002, ADR-004, ADR-006; `requirements.md` (NFR-02, NFR-03, NFR-07); `risk-register.md` (C7, R13, R6)]
- [Source: `docs/usb-controller-emulation.md`#Profile interface (AD-12), #Enumeration, #Timing chain, #Risks]
- [Source: `docs/link-protocol.md`#Messages, #Controller → app reporting (STATUS debug counters), #Rules the controller link applies]
- [Source: `docs/test-plan.md`#Test environment, #Bench rig for the controller link, #Spike 0b, #Firmware host tests F.0 to F.23, #Hardware-in-the-loop checks H.0 to H.12, A.3]
- [Source: `_bmad-output/implementation-artifacts/2-7-controller-link-watchdog.md`#Decisions D15, D17, #Consumers, #Hardware checks, #Previous story intelligence]
- [Source: `_bmad-output/implementation-artifacts/2-6-controller-link-uart-reliability-session-and-status.md`#D11 (ready and the mailbox), #D35 (STATUS debug counters), #D16 (purity gate)]
- [Source: `_bmad-output/implementation-artifacts/2-4-controller-core-modes-queue-scheduler-and-safety-mask.md`#D6 (LATE), #Consumers]
- [Source: `firmware/controller/{main.c,CMakeLists.txt,platform/link_runner.{c,h},link/include/ocb_link/ready.h,link/{ready.c,link.c,tx.c}}`; `firmware/tools/{check-link-pure.sh,link_probe.py}`; `firmware/tests/{CMakeLists.txt,link_pure_selftest.sh}`; `.github/workflows/firmware.yml`; `firmware/CMakePresets.json`]
- [Source: S1 to S10 of the Sources table (dekuNukem notes, Chromium, SDL, mzyy94, tmk gist, joycontrol, espp); S11 TinyUSB 0.18.0 in `firmware/external/pico-sdk/lib/tinyusb/src/class/hid/hid_device.c`, `src/device/usbd.c`, `examples/device/hid_composite`]
- [Source: GitHub issues #20 (purity gates) and #21 (LATE threshold and core 1 stack, Story 2.5)]

### Open points for the owner (none block the story)

- **D4:** the default `bInterval` is 1 ms (the real controller declares 8). If the console refuses it, rebuild with 8 and record; if you would rather ship the real 8 from the start, change one default.
- **D5:** `ready()` means "the host asked for the stream" (`80 04` or `03 30`); the Switch 2's real sequence is unknown until H.15. The story allows one correction of the trigger with evidence.
- **D8:** three bytes of the `0x30` report (battery/connection `0x91`, shared bit `0x80`, vibrator `0x0A`) are copied from one real capture and may need a change after H.15.
- **D7:** the stick-parameter bytes at `0x6086` and `0x6098` were not in the sources read; Task 1.1 settles them, and the console's reaction is the proof.
- **D15:** the bench-only build is removed. If `hidapi` cannot open the controller on your Mac and macOS does not handshake by itself, H.11 is re-run on the console chain instead.
- **D12:** until Story 2.5 the console sees a controller with nothing pressed; the first pressed button on the console is 2.5's.
- **D18:** the Mac may handshake with the controller by itself; if so, `p2_probe.py handshake` and macOS interleave and the probe says so.

## Dev Agent Record

### Agent Model Used

Claude (Opus), gds-dev-story workflow, run autonomously on 2026-11-22.

### Debug Log References

**Task 1.1, sources re-read (read-only fetches, raw files saved outside the repo).** S1 to S4 (raw markdown of dekuNukem's repo), S5 (Chromium raw source), S6 (SDL raw source), S7 (mzyy94 page), S8 (tmk gist) were re-read; S9 (joycontrol) only for the values already named (a second attempt to read its code was refused by the session's permission classifier, which fits D2: values only). Findings that settle the UNVERIFIED rows or change a starting value:
- Report descriptor: S7's usbhid-dump (203 bytes, hex) and S8's lsusb item decode agree item by item; device descriptor `12 01 00 02 00 00 00 40 7E 05 09 20 10 02 01 02 03 01` and configuration `09 02 29 00 ...` from S8's hex lines; strings from S7's hex.
- `0x6086` / `0x6098` stick parameters (UNVERIFIED): S4 gives a sample of the 12 values (`019 4CD AE E14 2EE 2EE 2EE 2EE AB4 AB4 496 496`) and says parameters 2 are "normally the same as 1, even in Pro Controller"; packed with S4's 12-bit pair packing: `19 D0 4C AE 40 E1 EE E2 2E EE E2 2E B4 4A AB 96 64 49`, served at both addresses. Settled from S4 (H.15 shows whether the console cares).
- `0x6020` IMU calibration (UNVERIFIED): S4 gives the default sensitivity coefficients `0x4000` (acc, ±8 G) and `0x343B` (gyro, ±2000 dps); origins set to 0 (the IMU reports zeros). `0x6080` offsets: S4's Pro Controller defaults X −688 (`FD50`), Y 0, Z 4038 (`0FC6`).
- S4 also states the console's sanity check for a Pro Controller: Y max, X min, Y min deltas ≥ `0x542`, X max ≥ `0x4D5`; the served calibration (`0x700` each) passes.
- Device-info firmware bytes (UNVERIFIED): S3's sample is `03 48` with "latest is 3.89" (encoding of 3.89 not given); S9 uses `04 00`. Decision D26: `04 00`.
- Sector erase `0x12`: S3 says it replies `x8012` ack plus a status byte (`01` = write protected), like the write. The story table had a NACK; implemented per S3 (`80 12 01`), still refused and counted.
- S2 adds two firmware-update paths the story did not list: output report `0x03` ("NFC/IR MCU FW Update packet") and feature reports `0x70`–`0x74` (OTA enable, memory read, erase, write). Both refused (D26).
- S7's capture between a real Switch 1 and a real Pro Controller shows the console's own init: `80 05`, `80 01`, `80 02`, `03` (mode), `80 04`, `48`, `02`, `08`, SPI reads, **`01` (BT manual pairing)**, `03`, `04`, SPI reads, `40`, `48`, **`21` (MCU config)**, `30`; about 80 `0x30` reports per second after `80 04`. The console itself sends two ids the refusal set NACKs (D26 records the risk).
- `80 05` wording: S1 "allows the controller to time out", S5 names it `kSubTypeEnableUsbTimeout`; nothing says the stream stops. Cautious reading kept (D5).
- Button bits, stick packing, battery byte meaning (`0x91` = full + charging, Pro, USB powered) re-verified in S2. NACK = `00` in S2.

**Task 8.1, TinyUSB 0.18.0 read** (`src/device/usbd.c`, `src/class/hid/hid_device.c`): bus reset → `DCD_EVENT_BUS_RESET` → `usbd_reset()` only, **no application callback** (`tud_umount_cb` is not called); the only hook is `tud_event_hook_cb(rhport, eventid, in_isr)`, called from the interrupt when the event is queued, without the endpoint. Unmount → `tud_umount_cb` (unplug event, or `SET_CONFIGURATION 0`). Suspend → `tud_suspend_cb` (only while connected). Mount → `tud_mount_cb` at `SET_CONFIGURATION != 0`. Interrupt OUT → `tud_hid_set_report_cb(instance, 0, OUTPUT, buf, len)` with the id in `buf[0]`; control `SET_REPORT` → id from `wValue`, stripped from the buffer when it leads it. `tud_hid_n_report(id, ...)` prepends a non-zero id; with id 0 our 64 bytes go out as they are. `tud_hid_report_complete_cb` runs inside `tud_task()` (task context). GET_REPORT returning 0 stalls (`TU_ASSERT(xferlen > 0)`). Choices: id 0 with the full 64 bytes (D6); bus reset counted in the hook and handled in `usb_p2_task()` (D28); the IN stamp taken in `tud_hid_report_complete_cb`, task context, so it includes core 0's loop latency (D13; the hook cannot tell which endpoint completed).

**Build issue found and fixed:** `CFG_TUSB_DEBUG` is set on the command line by the SDK in Debug builds; `tusb_config.h` now `#undef`s it and sets 0 (no TinyUSB log output; stdio off). `DCD_EVENT_BUS_RESET` needs `device/dcd.h`.

**Red proofs (each new rule shown failing once, then reverted; real output excerpts):**
- F.26, refusal switched off (`nack_refuse` answering `80` with no refusal): `test_p2_subcmd.c:261: FAIL: subcommand 01 must be refused (got 0)`, `...:117: FAIL: refused subcommand: refusal count 3 -> 3`.
- F.27, stick scale 13 instead of 14: `test_p2_report.c:149: FAIL: axis 0 at 0x00: 180`, `...:165: FAIL: pack x 0x100 y 0xF00: 80 01 e8`.
- F.24, OUT endpoint `bInterval` fixed at 8: `test_p2_desc.c:211: FAIL: configuration descriptor differs`, `...:91: FAIL: bInterval 8 expected 1`.
- F.25, `restart` not emptying the queue: `test_p2_handshake.c:158: FAIL: restart after step 0`, `...:36: FAIL: round 1000: nothing to send before any request`.
- F.28, the `80` length check relaxed to 1 byte (reads `buf[1]` of a 1-byte report): `ERROR: AddressSanitizer: heap-buffer-overflow ... p2.c:226 in on_usb`.
- F.30 / CI gate, `#include "pico/stdlib.h"` injected into `p2.c`: `check-p2-pure.sh` printed `p2.c:17:#include "pico/stdlib.h"` and its error line, exit 1; reverted: `check-p2-pure: ok (9 files)`.
- F.31, the v3 tail sent in a release configuration: `test_link_dbglog.c:148: FAIL: release STATUS: extra 26`.
- F.32, one wrong expected byte in `p2_probe.py` (`0x6080` reply): `AssertionError: Tuples differ: (1, 'FAIL 10 SPI read 0x6080 (6) -> 90 10 80 60 00 00 06 50 fd 00 00 c6 0f') != (0, 'PASS')`. The test's own negative controls (a wrong reply byte, a refusal switched off, absent device, wrong serial, one wrong descriptor byte, no console_ready, 100 µs interval, missing handshake record, console never wakes) are permanent tests.
- F.22 extension, a doc drift (`at most 58`): `docs_constants_check: FAIL link doc: STATUS v3 sizes differ from ocb_link/config.h (24, 56, 67)`.
- CI gate "Release has no debug log", a push injected in the release glue: the release ELF listed `ocb_dbglog_drain`, `_pop`, `_push`, ... and `check-release-nolog: FAIL: debug-log symbols in build/fw-release/controller/controller.elf`, exit 1; reverted and rebuilt: `ok, no debug-log symbol`. Also `--expect-present` on the release ELF: `FAIL: no debug-log symbol ... (expected in a debug build; renamed?)`, exit 1.

**Final local verification (real output):** fresh `cmake --preset host-tests && cmake --build --preset host-tests` (0 warnings), `ctest --preset host-tests`: `100% tests passed out of 34`. Fresh `fw-release` and `fw-debug` builds: 0 warnings each (`fw-debug-bench` no longer exists: `cmake --list-presets` shows fw-release, fw-debug, host-tests). `fw-debug -DOCB_P2_BINTERVAL=8` builds with 0 warnings; `-DOCB_P2_BINTERVAL=17` and `-DOCB_PROFILE=P1` stop the configuration with their messages. Gates: `check-pins: ok`, `check-proto-version: ok (proto_version 01, 7 files)`, `check-proto-pure: ok (14 files)`, `check-core-pure: ok (11 files)`, `check-link-pure: ok (18 files)`, `check-p2-pure: ok (9 files)`, `check-release-nolog: ok, no debug-log symbol in build/fw-release/...`, `--expect-present: ok, debug build carries the debug log (7 symbols)`.

**Sizes (`arm-none-eabi-size controller.elf`, text / data / bss):** release before (baseline `9af451e`) 22156 / 0 / 11488, after 38940 / 0 / 15260 (+16784 flash, +3772 RAM: TinyUSB, the profile, the 1.3 KB interval histogram, the reply queue). Debug before 31544 / 0 / 11484, after 48420 / 0 / 15396.

**Stack (`-fstack-usage`, ARM release build, same flags before and after):** `ocb_link_tx_next` 160 → 144 bytes (the 21-byte debug scratch left the release path; `build_status` is still inlined); `ocb_ctrl_link_step` 304 → 304; `ocb_link_poll` 88 → 88. The debug STATUS path is a separate function, `ocb_link_tx_status_debug_` 248 bytes (+ `ocb_link_status_body_` 80), reached only when `debug_counters` is set: worst core 1 TX chain in debug builds ≈ 144 + 248 + 80 bytes. Hand-off to issue #21 (stays open): Story 2.5 re-measures the core 1 stack with these numbers.

### Decisions added during development (numbered after D25; the owner can overrule any)

- **D26. Refusal set completed from the sources; firmware version `04 00`.** Added to C7's refusals: output report `0x03` (S2: MCU firmware update packet) and every feature report (S2: `0x70`–`0x74` unlock, read, erase and write the controller's memory), both with no reply, counted and logged (`ocb_p2_on_feature`, used by the glue for every FEATURE SET_REPORT). Sector erase answers as S3 documents it (`80 12 01`, write protected) instead of a NACK. Device info carries firmware `04 00` (S9; newer than S3's 3.89 under any encoding, so no update is offered). Risk recorded: S7 shows a Switch 1 sending `01` (pairing) and `21` (MCU config) during its normal init; both get a NACK. If H.15 shows the Switch 2 stalling after them, the D5-style caution rule applies: answer them with an ack that changes nothing (the "changes nothing" property of C7 is kept either way), with the log as evidence.
- **D27. Subcommand `03` starts the stream only with argument `0x30`.** D5's text says "`03` with argument `30`", the table said "30 (or any)". S7's capture shows the console sending `03` with another mode before `80 04`, so "any" would start the stream early. `03` without an argument is acked and changes nothing.
- **D28. USB-down posting: once per down episode, only after a mount; bus reset through the event hook.** TinyUSB 0.18.0 has no application callback for a bus reset (Debug Log), so `tud_event_hook_cb` counts it in the interrupt and `usb_p2_task()` handles it before and after `tud_task()`. Every suspend, unmount and bus reset restarts the profile, sets the report period to 0 and logs a restart; the mailbox counter is posted only if the device was mounted since the last post. Without that, the one or two bus resets of every enumeration would abort the core and send `EVENT USB_SUSPENDED` at each plug-in, and a sleep followed by a reset would count twice. A race remains if a bus reset and the next `SET_CONFIGURATION` are processed in the same `tud_task()` call (needs a whole re-enumeration inside one call; not expected).
- **D29. The debug STATUS is built in its own function and file (`controller/link/tx_debug.c`).** So the release TX path keeps its stack (measured smaller, Debug Log) and no `ocb_dbglog_*` function is referenced from `link.c` or `tx.c`: the window lives in `ocb_link` as plain data, filled by the firmware's drain (debug only) and read by `tx_debug.c`. That is what makes the release ELF symbol gate meaningful. `ocb_link_poll` applies the mailbox period only when the mailbox value changes (field `mb_period_seen`), so a direct `ocb_link_set_report_period` (tests, F.15) is not overwritten every pass.
- **D30. `p2_probe.py enum` parses loosely and prefers ioreg for the field checks.** No USB device could be attached to the Mac during development, so the real `SPUSBHostDataType` key names for a device were not captured (only the empty bus list was). The parser walks `_items` recursively and matches keys by suffix (`...VendorID`, `...ProductID`, manufacturer / vendor name, product name or `_name`, serial, bcd / version); the field checks use `ioreg -p IOUSB -l -w0` first (IOKit keys `idVendor`, `idProduct`, `USB Vendor Name`, `USB Product Name`, `USB Serial Number`, `bcdDevice`), system_profiler second. The JSON fixtures in `tests/fixtures/` are hand-written in the shape expected, not captured; if H.13 fails on a field the probe prints the parsed fields, and the fixture and parser are corrected with the owner's real output.
- **D31. No serial seed.** The USB serial (D3) and the SPI serial (D7) are fixed; only the MAC comes from the board (`ocb_p2_cfg` carries the MAC alone).
- **D32. `direct`, `playing` and `breaks --mode playing` FAIL (exit 1) without `console_ready`.** After waiting up to 5 s for it, with the hint to start `p2_probe.py handshake --hold N &` in the same shell line or to plug into the console. The exit code 2 "skipped" is gone with the bench build.
- **D33. Median details.** 401 bins of 50 µs centred on k × 50 µs (a steady 1000 µs train reads exactly 1000), lower median of the window; recomputed in `ocb_p2_tick` only after new samples (≤ 401 bin visits), O(1) per note; 64-bit and 32-bit clocks both work because only the low 32 bits are used (a backwards step becomes a huge interval and is dropped).

### Completion Notes List

- **Status: done (2026-11-22).** All tasks complete, Task 11 included. Every AC has host evidence and bench evidence; see "Acceptance criteria: evidence" below.
- **Tests:** `ctest --preset host-tests` reports 100% passed out of 35 (25 before the story, plus F.24 to F.33), under ASan and UBSan.
- **Builds and gates:** fw-release and fw-debug build with 0 warnings. All gates are green: `check-pins`, `check-proto-version`, `check-proto-pure` (14), `check-core-pure` (11), `check-link-pure` (20), `check-p2-pure` (9), `check-release-nolog` (release ELF, and `--expect-present` on the debug ELF).
- **CI:** green on `a4900b0`, the commit that records the bench: https://github.com/basteez/bonooru/actions/runs/45045132603 (`firmware-uf2` artifact present). Earlier green runs: `573630f` (45043965394), `f8900f1` (45039970542) and 45039390009.
- **Bench on the owner's Switch 2** (system 23.5.0, top USB-C port, Wired Communication ON; bridge 2wiCC_Comms v1.1 on `/dev/cu.usbmodem1101`), all recorded in `docs/test-plan.md`:

| Check | Result |
|---|---|
| H.13 | PASS |
| H.14 | PASS |
| H.11 | PASS ×3, on the real `ready()` |
| H.15 | attempt 1 FAILED (the pairing loop, fixed by D34); attempt 2 nothing visible (the report was always NEUTRAL, D12); attempt 3 PASS (the first input acted on, D35) |
| H.16 | PASS |
| H.17 | skipped |

- **Risk register:** C7 and R13 updated with the bench evidence.
- **Optional libFuzzer target** `tests/fuzz/fuzz_p2.c` (`-DOCB_FUZZ=ON`, Homebrew LLVM): not run in CI. The deterministic F.28 fuzz covers CI. The story lead also ran a local random-input fuzz of 6,000,000 inputs with no sanitizer report.
- **D15 done:** the `fw-debug-bench` build is gone. The untracked `firmware/build/fw-debug-bench/` directory was left as it was.

### Acceptance criteria: evidence

- [x] **AC1** (a wired Pro Controller on a stock Switch 2; the AD-12 interface).
  - Host: F.24 (descriptors), F.25 (handshake), F.27 (report).
  - Bench: H.13 (the Mac sees `057E:2009` with the right strings), H.14 (host handshake byte-exact), H.15 attempt 3 (the Switch 2 accepts the controller; the first A launched the game from HOME).
- [x] **AC2** (clean-room, sources cited).
  - Every constant cites S1 to S13. The sources table is in `docs/usb-controller-emulation.md`.
  - No 2wiCC or GPL code. A review found no IMPORTANT issue.
- [x] **AC3** (firmware update refused, C7).
  - Host: F.26 and F.28. Every refusal leaves the state byte-for-byte unchanged.
  - Bench: no update was offered in the whole session; the logged console sequences had no refused request.
- [x] **AC4** (`ready()` false and a new handshake after reset, suspend or unmount; clean re-enumeration).
  - Host: F.25 (restart), F.10 and F.31.
  - Bench: H.16. Sleep cuts USB power; wake gives a new handshake with ready 1.7 s after the console's first request; Wired Communication off and on recovers. Dock was not run (R13 notes it).
- [x] **AC5a** (debug log through the SPSC ring, absent in release).
  - Host: F.31, F.32.
  - CI: `check-release-nolog`.
  - Bench: the console's real sequence was read from the debug STATUS on every H.15 attempt.
- [x] **AC5b** (median IN interval in `INFO.report_period_us`; the polling rate recorded).
  - Host: F.29, F.31.
  - Bench: H.15 measured **8000 µs**, recorded in `docs/usb-controller-emulation.md`, where the ADR-002 1 ms hypothesis is now marked refuted.

### Learnings

- **macOS "Allow accessory to connect".** With the setting on "Ask", a pending prompt left the controller suspended and gone from the USB tree. That made the first H.11 attempt fail until a replug. Bench setup: "Allow accessories to connect: Always". The `p2_probe.py` failure messages now hint at it.
- **The Switch 2 shows a wired controller only after its first input.** Before it there is no icon and no Controllers-menu entry, even though the console assigned a player slot (`30`). The H.15 criterion is now "first input acted on".
- **The Switch 2 cuts USB power in sleep.** The controller board is off while the console sleeps and boots again on wake. "Sleep" is therefore a power cycle for the controller, not a USB suspend.
- **The Switch 2 polls at 8 ms** whatever `bInterval` says (our descriptor asks for 1 ms; the Mac polls the same build at about 2 ms).
- **The console's own init needs two answers a refusal policy would block.** The pairing request `01` needs `81 01 03` (a NACK makes the console repeat it forever), and the MCU configuration `21` needs `A0 21` with the standby state report. Both come from a real capture (S12) and change nothing. The rest of the init matches S12's Switch 1 order.
- **The published-state handoff needs care.** A reader with a short retry budget against a writer that rewrites on every pass produced a NEUTRAL blip. Writing only on change, and keeping the last good copy on the reader, fixed it.

### Hand-off to Story 2.5

- **`report_period_us`:** 8000 µs measured on the Switch 2 (H.15). Issue #21 stays open: Story 2.5 sets the LATE threshold against it, replaces or confirms `OCB_CORE_LATE_FALLBACK_US`, measures the jitter, and re-measures the core 1 stack (debug STATUS path: `ocb_link_tx_status_debug_` 248 bytes + `ocb_link_status_body_` 80).
- **The published-state path** (`controller/link/pubstate.c`, D35) is debug-only today. Story 2.5 keeps it or replaces it with its seqlock, and makes it the release path; release builds still report NEUTRAL.
- **Barriers:** `OCB_LINK_BARRIER()` is empty on the target. The handoff relies on volatile accesses and the in-order Cortex-M0+. Story 2.5 should define it as `__dmb()` for the firmware (compile definition) when it owns the seqlock.
- **The S12 data rows left unaligned:** device-info firmware version `04 00`, byte 10 `01`, colour flag `00`, the serial at `0x6000`, colours, the stick parameters at `0x6086` and `0x6098`, the stick calibration values. The console accepts us as we are; align them only if a later story finds a reason.

### Review follow-ups (code review, no IMPORTANT findings; MINOR fixes applied 2026-11-22)

Review outcome: a fresh-context code review found no IMPORTANT findings. Its MINOR findings were fixed in commit `f8900f1`, except the three listed under "Review findings not fixed" below.

Fixed in `f8900f1`:

- `THIRD_PARTY_NOTICES.md`: the Story 2.7 "bench-ready hook" line now says it was removed by Story 2.3.
- `docs/usb-controller-emulation.md`: S10 (espp) dropped from the sources table, because it was not read for this implementation. The S-list in `ocb_p2/config.h` and the notices already matched that.
- `profile_p2/p2.c`: a subcommand whose reply is dropped (`QUEUE_FULL`) now has no effect. Player lights, IMU, vibration and input mode are not stored, and `03 30` does not start the stream; the host's resend applies them. New F.26 case in `test_p2_subcmd.c`. Red proof: with the guard removed for player lights the test printed `test_p2_subcmd.c:339: FAIL: a dropped request changed a setting`; reverted, green.
- `docs/usb-controller-emulation.md`: the queue of 4 and its silent drop on overflow are documented. A dropped request is counted in `cnt.queue_full` (no new log kind) and changes nothing.
- `platform/usb_p2.c`: only `0x30` report completions feed the interval median, so handshake replies do not skew it. Every IN completion still feeds `console_ready`. The docs (usb-controller-emulation, link-protocol, controller README) and the `p2.h` comment now say "0x30 completions".
- `firmware/README.md`: `-DOCB_FUZZ=ON` needs clang with libFuzzer (Homebrew LLVM on macOS; Apple clang has none).
- `platform/usb_p2.c`: a comment explains how the MAC is derived. Bytes 2..7 of the unique id are XORed with bytes 0/1, then the first byte is made locally administered and unicast.
- Re-verified (real output): host-tests 0 warnings, `100% tests passed out of 34`. fw-release and fw-debug build with 0 warnings. `check-pins`, `check-proto-version`, `check-proto-pure`, `check-core-pure`, `check-link-pure`, `check-p2-pure` all ok. `check-release-nolog: ok, no debug-log symbol` on the release ELF, `--expect-present: ok (7 symbols)` on the debug ELF.

#### Review findings not fixed (MINOR, recorded for a later story)

- (a) `firmware/tools/check-p2-pure.sh`: the banned list lacks `asm`, `__asm`, `volatile` and `abort`. None is used today, and `<assert.h>` is already blocked by the include allow-list.
- (b) `tests/fuzz/fuzz_p2.c` (libFuzzer target) is not run in CI (Apple clang lacks libFuzzer). The deterministic `test_p2_fuzz` (F.28) under ASan and UBSan covers CI.
- (c) A reply-queue overflow is counted in `cnt.queue_full` but is not surfaced in the STATUS debug log.

### Bench H.15 attempt 1 (FAILED) and the fix (D34), 2026-11-22

**What happened** (story lead's bench run: fw-debug of the branch, top USB-C port, Wired Communication ON). `link_probe.py p2check` printed PASS: `console_ready` 1 after 24 s, `report_period_us` 8000, no NAK VERSION. But no controller icon appeared on the console.

The P2 log held 77 records: restart bus-reset, restart mount, report 00, report 00, usb 80 05, report 00, usb 80 01, usb 80 02, sub 03, HANDSHAKE, usb 80 04, sub 02, sub 08, sub 10, sub 10, sub 01, REFUSED 01. Then `sub 01, REFUSED 01` repeated about 30 times until the log was full. This is the D26 risk: the console retries the refused pairing request (subcommand 01) forever and never finishes its init.

**Research (read-only, clean-room; no GPL source, no 2wiCC).** Two new data sources, used for values only:
- **S12:** mzyy94's `bypass_procon_log.txt`, the capture behind S7 (gist 60ae253a…). It holds every output report a real Switch 1 sent to a real wired Pro Controller during init, with the controller's real replies.
  - Pairing: the console sends `01` with type `04`, then its address and name (`04 18 53 95 d6 03 04 00 04 3c "Nintendo Switch" … 68 00 ac 90 11 3f 59 00`). The real reply is `21 .. 81 01 03`.
  - MCU: the console sends `21 21 00` (MCU to standby). The real reply is `A0 21` and 34 bytes, `01 00 FF 00 03 00 05 01`, 25 × `00`, `5C`.
  - The capture's `03` argument is `30`; mzyy94's own summary table says `48`, which is wrong.
- **S13:** riking's comment in dekuNukem's issue #7. A Joy-Con answers `01` with `81 01 0C` (and `0B`), the same shape: ack `81`, id `01`, one status byte.
- **S3** (dekuNukem subcommand notes) documents the pairing types 01/02/03 and "ACK xA0 x20 and 34 bytes" for `21`. S12 shows the real reply echoes id `21`; S12 is followed.
- **Not read:** the code of nxbt, GP2040-CE and joycontrol. The session's permission classifier refused those reads, consistent with D2.

**D34 (new; supersedes D26's NACK for `01` and `21`).** The console's pairing request and MCU configuration now get the real controller's replies. Neither has any effect.
- `01` (any argument) → ack `81`, id `01`, data `03`. Nothing is paired or stored; the host address and name are discarded.
- `21` (any argument) → ack `A0`, id `21`, the 34-byte standby state report above. There is no MCU; nothing is configured.
- Both are counted as handled, not refused.
- C7 still holds. Nothing writes flash or the emulated SPI (the SPI image has no writable state). The firmware-update path stays refused: SPI write (`80 11 01`), SPI erase (`80 12 01`), `22 02` (NACK), output report `0x03`, feature reports `0x70`–`0x74`. Also still refused with a NACK: `06` (HCI), `07` (reset pairing), `20` (MCU reset); none of them appears in S12's init.
- Every other id S12's init sends gets the real controller's ack and id:

| Request | Reply |
|---|---|
| `80 05`, `80 04` | none |
| `80 01`, `80 02` | `81 01`, `81 02` |
| `03 30` | `80 03` |
| `08` | `80 08` |
| `02` | `82 02` |
| `04` | `83 04` |
| `10` | `90 10` + the echo |
| `40` | `80 40` |
| `48` | `80 48` |
| `30` | `80 30` |
| `38`, `22 00`, `22 01` | `80` (no change) |

- A replay of the whole S12 init (28 output reports) is now a host test. It checks each reply's ack and id, the data for `01`, `21` and the SPI echoes, zero refusals, one handshake and the stream on.

**Debug log, for the next bench run.**
- New kind `0x08`: the first argument of subcommands `01`, `03`, `21` and `22`. A profile log can now hold 3 records per report (`OCB_P2_LOG_MAX`).
- New kind `0x09`: repeat. In the link's window, a record equal to the two before it, or continuing a two-record cycle (A B A B) from the second B, is not stored. One repeat record counts them, updated in place and saturating at 255; `p2_log_total` counts it once. The H.15 loop now shows as `sub 01, REFUSED 01, sub 01, (repeated 57 more)`.
- `link_probe.py`'s decoder and tracker know both kinds; the tracker refreshes an in-place repeat count.
- `docs/link-protocol.md` documents them. `test_docs_constants.py` is unaffected: the STATUS v3 sizes did not change.

**Tooling.** `p2_probe.py enum` and `handshake` print "check for a macOS 'Allow accessory' prompt, or replug" when 057E:2009 cannot be found or opened. `handshake` now also sends S12's `01` pairing request and `21 21 00`, and checks the D34 replies byte for byte.

**Red proofs** (real output, each reverted):
- `01` refused again (the attempt-1 firmware): `test_p2_handshake.c:130: FAIL: S12 step 14 (010e…5900): result 1`, then `…:142: FAIL: S12 step 14: reply 21 00 01, want 21 .. 81 01`.
- `21` refused again: `S12 step 25 (010ec218037200014040212100): result 1`, `reply 21 00 21, want 21 .. a0 21`.
- Repeat compression off: `test_link_dbglog.c:95: FAIL: pair loop: n 16 total 62`.
- The old expectations failed first against the new code too (`test_p2_subcmd.c:261: FAIL: subcommand 01 must be refused (got 0)`, `test_p2_fuzz.c:96: FAIL: subcommand 01: 0`). They were updated: `01` and `21` are now expected to be answered, and the fuzz test checks that they are never refused.

**Verification** (real output, fresh build directories):
- host-tests: 0 warnings; ctest `100% tests passed out of 34`.
- fw-release: build=0, warnings=0. fw-debug: build=0, warnings=0.
- `check-pins: ok`; `check-proto-version: ok (proto_version 01, 7 files)`; `check-proto-pure: ok (14 files)`; `check-core-pure: ok (11 files)`; `check-link-pure: ok (18 files)`; `check-p2-pure: ok (9 files)`.
- `check-release-nolog: ok, no debug-log symbol in build/fw-release/controller/controller.elf`; `--expect-present: ok, debug build carries the debug log (7 symbols)`.
- Release ELF: 39096 text / 15268 bss.

**Risk register note (not edited; evidence first, D22).** C7 holds by design and by test (F.26, F.28), but is not yet observed on the console; R13 is untested. Attempt 1 adds one fact for the register's next update: the Switch 2 does not tolerate a refused pairing request during its wired init. Neither cell is marked passed.

**Next bench step.** H.15 attempt 2 with this build, the same command. The owner's glance at the controller icon stays part of the pass criterion, because `p2check` alone printed PASS in attempt 1.

### Bench H.15 attempt 2 (no icon) and D35, 2026-11-22

**What happened** (story lead, `c020514`, fw-debug, Switch 2 top USB-C port, Wired Communication ON).
- `p2check` printed PASS: `console_ready` 1 after 8.3 s, `report_period_us` 8000.
- The P2 log held 26 records: restart bus-reset, restart mount, report 00 ×2, usb 80 05, report 00, usb 80 01, usb 80 02, sub 03, arg 30, HANDSHAKE, usb 80 04, sub 02, sub 08, sub 10 ×2, sub 01, arg 01, sub 03, arg 30, sub 04, sub 10 ×5, sub 40, sub 30. After that the console sent nothing more.
- No controller icon appeared. Five D-pad-down presses in DIRECT were all ACKed, but nothing moved on the HOME screen.

**Analysis.**
1. **Inputs could not work in that build: confirmed.** `ocb_ctrl_published_read()` returned NEUTRAL in every build (D12), so no SET_STATE reached the `0x30` report. The mapping from canonical state to report is right and host-tested in F.27: hat 4 (down) sets byte 5 bit `0x01`, each of the 14 buttons sets exactly its S2 bit, all 65 536 button values and all 256 hat values are covered. The new `p2_probe.py watch` test (F.32) also shows a published D-pad down arriving as `Down` in the real `ocb_p2` report.
2. **Hypothesis B (we answer something wrong): weak.** The Switch 2 sent S12's Switch 1 init almost request for request:
   - the same order, including the second `03 30` right after `01`, which S12 also shows;
   - the same five SPI reads after `04`;
   - then `40` and `30`, after which it fell silent. S12's console also sends only rumble after `30 01`.
   - The Switch 2 skipped `48` and `21`.
   - Sending `30` (player lights) means the console assigned the controller a player slot. A host that rejected the controller would not do that.
   - Our replies match S12's ack and id for every request. The data differences are listed below. None is known to block registration; they are the fallback if attempt 3 still shows nothing.
3. **Hypothesis A (no visible controller until the first input): plausible, not proven by a public source.** No public document I found states whether the HOME screen shows a newly attached wired controller before any input.
   - joycontrol's code comment (read earlier, value-level) says it fakes the trigger elapsed-time reply (`04`) "so the Switch assigns a player number". That is over Bluetooth. S12's real wired controller answers `04` with zeros and still got `30 01`.
   - Attempt 2 shows the console did assign a slot (`30`). So the missing icon is more likely a display or input matter than a rejection.
   - Cheapest separation: make a press reach the report, and check the console's Controllers menu.

**Reply bytes compared with S12** (real Switch 1 and real Pro Controller). ✓ = matches; ✗ = data differs (listed as a hypothesis B fallback, not changed).

| Request | Real reply (S12) | Ours | |
|---|---|---|---|
| `80 01` | `81 01 00 03` + MAC reversed | same shape (MAC from the board) | ✓ |
| `80 02` | `81 02` | `81 02` | ✓ |
| `03 30` (both times) | `80 03` | `80 03` | ✓ |
| `02` | `82 02 03 48 03 02 <MAC> 03 01` | `82 02 04 00 03 02 <MAC> 01 00` | ✗ firmware `04 00` vs `03 48` (D10/D26 choice); byte 10 `01` (S3 "always 01") vs real `03`; colour flag `00` vs real `01` |
| `08` | `80 08` | `80 08` | ✓ |
| `10 6000` (16) | 16 × `FF` (no serial) | `"000000000001"` + `FF` ×4 | ✗ |
| `10 6050` (13) | `32 32 32 FF FF FF` + `FF` ×7 (colours) | `FF` ×13 | ✗ (follows our colour flag `00`) |
| `01` | `81 01 03` | `81 01 03` | ✓ (D34). Switch 2 sent arg `01`, S12 sent `04`; one public reply shape for all |
| `04` | `83 04` + zeros | same | ✓ |
| `10 6080` (24) | `50 FD 00 00 C6 0F` + real stick params `0F 30 61 96 30 F3 D4 14 54 41 15 54 C7 79 9C 33 36 63` | offsets ✓; params from S4's sample `19 D0 4C AE 40 E1 …` | ✗ params |
| `10 6098` (18) | the same real params | S4 sample | ✗ |
| `10 8010` (24) | `FF` ×22, then `B2 A1` (the user-IMU-calibration magic at `0x8026`) | `FF` ×24 | ✗ (benign: no user calibration) |
| `10 603D` (25) | real factory stick calibration + `FF` + colours `32 32 32 FF FF FF` | S9 default calibration + `FF` ×7 | ✗ values (ours pass S4's sanity limits) |
| `10 8028` (24) | real user IMU calibration (magic was set) | `FF` ×24 | ✗ (benign) |
| `40 01` | `80 40` | `80 40` | ✓. The IMU block stays zero; S12's real report carries real motion data, which a host can live without (S7's notes) |
| `30 01` | `80 30` | `80 30` | ✓ |
| report byte 2 | `91` in every S12 reply | `91` | ✓ (`8E` is the Joy-Con value seen in issue #7) |
| byte 12 (vibrator) | varies `00` / `09`..`0C` | `0A` fixed | ~ |
| timer | +1 per report on our side; real counts time | | ~ (hosts read it only for latency) |
| vendor `80 xx` | only 05, 01, 02, 04 | answered as S1 | ✓ |

**D35 (new): debug builds report the published state.** This is the cheapest test of hypothesis A, and an end-to-end check of the input path.
- `ocb_ctrl_published_read()` now reads core 1's published state through a sequence lock, in debug builds only (`controller/link/pubstate.c`, pure C99, inside `ocb_link`).
- Core 1 writes it once per link pass. Core 0 reads it with 3 tries, then NEUTRAL, so a torn state is never reported (AD-11).
- Release builds stay NEUTRAL until Story 2.5, which may keep or replace this as its seqlock. The scope is debug-only on purpose: the AC set of 2.5 owns the real path, and the hot-path timing is unmeasured.
- No race is accepted: it is not a plain cross-core copy.
- Host test F.33 (`test_link_pubstate`). Red proof: with the "odd counter" check removed, `test_link_pubstate.c:53: FAIL: read during a write: NEUTRAL, false`.

**New bench aids.**
- `link_probe.py press --hat {up,down,left,right} | --button {A,B,X,Y,L,R,ZL,ZR,PLUS,MINUS,HOME} [--count 5 --hold-ms 150 --gap-ms 500]`: SET_STATE in DIRECT with PINGs during the hold, then NEUTRAL; PASS when every SET_STATE is ACKed.
- `p2_probe.py watch [--seconds 30] [--expect Down]`: holds the stream from the Mac and prints the pressed buttons as they change.
- F.32 covers both, with negative controls: nothing pressed fails `--expect`; no `console_ready` fails `press`.

**Proposed attempt 3** (fw-debug of this change, flash the controller Zero only):
1. Mac, no console. Controller Zero on the Mac, bridge wired, `export P=/dev/cu.usbmodemXXXX`. One line: `python3 firmware/tools/p2_probe.py watch --seconds 30 --expect Down & sleep 3; python3 firmware/tools/link_probe.py --port "$P" press --hat down --count 5; wait` → `Down` printed five times, `PASS`. This proves the press reaches the USB report, independent of the console.
2. Console. `python3 firmware/tools/link_probe.py --port "$P" p2check`, plug into the top USB-C port. Then, on the HOME screen, `python3 firmware/tools/link_probe.py --port "$P" press --button A --count 3` and `... press --hat right --count 5`. Look: does an icon or "controller connected" banner appear after the first press, and does the HOME cursor move? Also open System Settings > Controllers and Accessories: is a Pro Controller listed?
3. If presses move the cursor: hypothesis A is confirmed and Story 2.3 passes H.15 with that note.
   If step 1 passes but the console still shows nothing and does not react: hypothesis B. Next build aligns the ✗ rows with S12: device info `03 48 … 03 01`, serial `FF`, colours, real stick parameters.

**Verification** (real output, fresh build directories):
- host-tests: 0 warnings; ctest `100% tests passed out of 35` (new: `test_link_pubstate`).
- fw-release: build=0, warnings=0. fw-debug: build=0, warnings=0.
- `check-pins: ok`; `check-proto-version: ok (proto_version 01, 7 files)`; `check-proto-pure: ok (14 files)`; `check-core-pure: ok (11 files)`; `check-link-pure: ok (20 files)`; `check-p2-pure: ok (9 files)`.
- `check-release-nolog: ok, no debug-log symbol in build/fw-release/controller/controller.elf`; `--expect-present: ok (7 symbols)`.
- `nm` shows no `ocb_pubstate_*` symbol in the release ELF; the debug ELF has `ocb_pubstate_init`, `_read`, `_write`, `_write_begin`, `_write_end`.

### Bench complete (2026-11-22) and the H.15 attempt 3 blip

**Results** (owner's bench; full rows in `docs/test-plan.md`):
- **H.13 PASS** on the first build (the enumeration code is unchanged since). `057E:2009`, manufacturer, product, serial and `bcdDevice` 2.10 ok. SKIP for the report descriptor, because ioreg does not expose it.
- **H.14 PASS** on `c020514`. `01` → `81 01 03`; `21` → `A0 21` + standby report. 5452 `0x30` reports, 0 timer discontinuities, host-side median 1994 µs.
- **H.11 PASS ×3** on the real `ready()`, held by `p2_probe.py handshake --hold`. `playing` saw `LINK_LOST` at 81.5 ms. The first attempt failed on the macOS accessory prompt (see Learnings).
- **H.15:**
  - Attempt 1 FAILED: the pairing loop, fixed by D34.
  - Attempt 2: nothing visible, because the report was always NEUTRAL (D12).
  - **Attempt 3 PASS** on `573630f`.
    - Mac pre-check: `watch --expect Down` with `press --hat down --count 5` PASS, 16356 reports, Down seen 5 times.
    - Console: `p2check` PASS, `console_ready` 1 after 9.2 s, `report_period_us` 8000, 27 records including sub `48`.
    - No icon and no Controllers-menu entry before the first input. The first `press --button A` launched Zelda from HOME, and `press --hat right --count 5` was all ACKed. The owner confirmed.
- **H.16 PASS.** Sleep cuts USB power. Wake: ready 1.7 s after the console's first request, with a new handshake. Wired Communication OFF: ready fell within one STATUS period, `abort_count` 0. ON: ready 0.0 s after the first request.
- **H.17** skipped: the console polls at 8 ms regardless of the 1 ms `bInterval`.

**The press-4 blip** (found by code reading, then fixed and host-tested). The `watch` stream during press 4 showed:

| Time | Report |
|---|---|
| 5.051 s | Down |
| 5.172 s | (nothing) |
| 5.174 s | Down |
| 5.205 s | (nothing) |
| 5.710 s | Down (press 5) |

Reading it: one NEUTRAL report about 121 ms into a 150 ms press. The next report, 2 ms later, was Down again, and the press ended on time. So it is about 2 ms of NEUTRAL, not 30 ms.

Causes ruled out:
- **A watch-tool artefact.** `run_watch` skips timeouts and non-`0x30` reads. "(nothing)" is printed only for a real `0x30` report with no button.
- **The link or DIRECT path.** An app timeout (1000 ms; PINGs every ≤ 50 ms) or any abort latches the core. It publishes NEUTRAL and stays NEUTRAL until STOP or HELLO, so Down could not come back 2 ms later.

The cause: the `573630f` handoff. Core 1 rewrote the state on every link pass, a few microseconds apart, so the sequence counter was odd for a noticeable share of the time. Core 0's reader gave up after 3 consecutive odd or changed reads and returned NEUTRAL. With about 75 Mac polls per press, a fallback rate below 1% gives about one blip in 5 presses, which matches what was seen.

**Fix (D35 amended):**
- The writer publishes only when the core's `publish_count` moves (`ocb_pubstate_publish`), so a held state is written once.
- A reader that gets no consistent copy keeps the last good state it read (`ocb_ctrl_published_read` keeps it on core 0) instead of NEUTRAL. It is still never torn and at most one report old.
- F.33 extended. Shown failing once each:
  - the old NEUTRAL fallback: `test_link_pubstate.c:57: FAIL: read during a write: false, last good state kept`;
  - a writer rewriting every pass: `test_link_pubstate.c:67: FAIL: same count rewrote at pass 0`.
- Not re-run on hardware. It needs no bench step before Story 2.5, which owns this path.

### File List

New:
- `firmware/controller/profile_p2/CMakeLists.txt`
- `firmware/controller/profile_p2/include/ocb_p2/config.h`
- `firmware/controller/profile_p2/include/ocb_p2/desc.h`
- `firmware/controller/profile_p2/include/ocb_p2/p2.h`
- `firmware/controller/profile_p2/include/ocb_p2/internal.h`
- `firmware/controller/profile_p2/desc.c`
- `firmware/controller/profile_p2/p2.c`
- `firmware/controller/profile_p2/report.c`
- `firmware/controller/profile_p2/interval.c`
- `firmware/controller/platform/tusb_config.h`
- `firmware/controller/platform/usb_p2.c`
- `firmware/controller/platform/usb_p2.h`
- `firmware/controller/link/include/ocb_link/dbglog.h`
- `firmware/controller/link/dbglog.c`
- `firmware/controller/link/tx_debug.c`
- `firmware/tools/check-p2-pure.sh`
- `firmware/tools/check-release-nolog.sh`
- `firmware/tools/p2_probe.py`
- `firmware/tests/p2_fixture.h`
- `firmware/tests/test_p2_desc.c`
- `firmware/tests/test_p2_handshake.c`
- `firmware/tests/test_p2_subcmd.c`
- `firmware/tests/test_p2_report.c`
- `firmware/tests/test_p2_interval.c`
- `firmware/tests/test_p2_fuzz.c`
- `firmware/tests/test_link_dbglog.c`
- `firmware/tests/test_p2_probe.py`
- `firmware/tests/p2_sim_shim.c`
- `firmware/tests/p2_pure_selftest.sh`
- `firmware/tests/p2_pure_fixtures/` (bad: `includes_sdk.c`, `includes_tusb.c`, `includes_hid_device.c`, `includes_stdio.c`, `includes_stdlib.c`, `includes_core.c`, `includes_link.c`, `reads_clock.c`, `allocates.c`, `multicore.c`, `cmake_links_tinyusb/CMakeLists.txt`, `cmake_links_link/CMakeLists.txt`; good: `allowed.c`, `CMakeLists.txt`)
- `firmware/tests/fixtures/` (`sp_usbhost_p2.json`, `sp_usb_p2.json`, `sp_usbhost_empty.json`, `ioreg_usb_p2.txt`, `ioreg_hid_p2.txt`)
- `firmware/tests/fuzz/fuzz_p2.c`
- `firmware/controller/link/include/ocb_link/pubstate.h`, `firmware/controller/link/pubstate.c`, `firmware/tests/test_link_pubstate.c` (D35)

Modified:
- `.github/workflows/firmware.yml`
- `firmware/CMakeLists.txt`, `firmware/CMakePresets.json`
- `firmware/controller/CMakeLists.txt`, `firmware/controller/main.c`
- `firmware/controller/platform/link_runner.c`, `firmware/controller/platform/link_runner.h`
- `firmware/controller/link/CMakeLists.txt`, `firmware/controller/link/link.c`, `firmware/controller/link/ready.c`, `firmware/controller/link/tx.c`
- `firmware/controller/link/include/ocb_link/config.h`, `.../internal.h`, `.../link.h`, `.../ready.h`
- `firmware/tests/CMakeLists.txt`, `firmware/tests/link_fixture.h`, `firmware/tests/link_sim_shim.c`, `firmware/tests/test_docs_constants.py`, `firmware/tests/test_link_fuzz.c`, `firmware/tests/test_link_probe.py`, `firmware/tests/test_link_ready.c`, `firmware/tests/test_link_tx.c`
- `firmware/tools/link_probe.py`
- `firmware/README.md`, `firmware/controller/README.md`, `firmware/THIRD_PARTY_NOTICES.md`
- `docs/usb-controller-emulation.md`, `docs/link-protocol.md`, `docs/test-plan.md`
- `firmware/tests/p2_sim_shim.c`, `firmware/tests/test_p2_probe.py` (D35 watch)
- `_bmad-output/specs/spec-bonooru/risk-register.md` (C7, R13 after the bench)
- `_bmad-output/implementation-artifacts/sprint-status.yaml`, this story file

## Change Log

- 2026-11-22: CI verified on `f8900f1` (run 45039970542, success, `firmware-uf2` artifact; earlier run 45039390009 success). Completion Notes updated: "unverified until pushed" removed for CI, PR opened against `main` (Refs #21, issue stays open). Status stays `review`; hardware ACs H.11 (re-run), H.13 to H.17 pending the owner's bench.
- 2026-11-22: Story 2.3 created (ready-for-dev) by the create-story workflow, run autonomously. Decisions D1 to D25 recorded; sources S1 to S11 cited; `fw-debug-bench` superseded (D15); issues #20 and #21 left open by design (D13, D16).
- 2026-11-22: Implementation (dev-story): `ocb_p2` profile, TinyUSB glue, P2 debug log and STATUS v3 tail, measured report period through the mailbox, purity and release-no-log gates, `p2_probe.py`, `p2check` / `p2watch`, 9 new ctest tests (F.24 to F.32; 34 in all, green), docs; `fw-debug-bench` removed; decisions D26 to D33 added. Status review; hardware ACs pending the owner's bench (Task 11).
- 2026-11-22: Review follow-ups: 7 MINOR fixes (no effect for dropped requests plus a test, median fed by 0x30 completions only, notices and docs consistency, fuzz note, MAC comment); 34 ctest tests green.
- 2026-11-22: Bench H.15 attempt 1 FAILED (the console repeated the refused pairing request 01 forever). D34: `01` and `21` get the real controller's replies from the S12 capture and change nothing; the log records arguments and counts repeats; `p2_probe.py` checks the new replies and hints at the macOS accessory prompt. 34 ctest tests green. H.15 attempt 2 pending.
- 2026-11-22: Bench H.15 attempt 2: handshake as S12, player slot assigned (`30`), but no icon; inputs could not reach the report (D12). D35: debug builds report the published state through a sequence lock (F.33); `link_probe.py press` and `p2_probe.py watch` added; S12 reply comparison recorded. 35 ctest tests green. Attempt 3 pending.
- 2026-11-22: Bench complete. H.13, H.14, H.11 ×3 PASS; H.15 attempt 3 PASS (first input acted on, new criterion); H.16 PASS; H.17 skipped (the console polls at 8 ms). The press-4 blip was fixed: publish on change, reader keeps its last good copy (D35 amended, F.33). Measured values recorded; risk register C7 and R13 updated. 35 ctest tests green. Status done.
