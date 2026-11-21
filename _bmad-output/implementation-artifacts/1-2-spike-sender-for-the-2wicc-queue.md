---
baseline_commit: 1c02e5533bcd6a9c6c9f4e58322724a9865818e9
---

# Story 1.2: Spike sender for the 2wiCC queue

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As the owner,
I want a tiny script that sends a list of timed controller states to the 2wiCC bridge,
so that the scripted spikes (Stories 1.3, 1.4) and the walking skeleton (Story 1.6) can run without any firmware of ours.

## Acceptance Criteria

Copied from `_bmad-output/planning-artifacts/epics.md`, Story 1.2. These are the definition of done.

1. **Given** the 2wiCC / 2wiCC_Comms serial command set is not recorded in this repo (only `+QD` appears in Spike 0b)
   **When** the owner reads the upstream 2wiCC and RiiDraw sources and docs (reading only, no code copied)
   **Then** `spikes/README.md` documents the subset the script uses: command syntax, the state encoding (buttons, hat, sticks), how queued states are timed (per frame or timestamp), the queue size and how the script avoids overflowing it, with links to the sources
2. **Given** the 2wiCC pair wired as in Spike 0b and the bridge port at 460800 8N1
   **When** the owner runs the script in `spikes/` (Python 3 with `pyserial`, single file, run with `uv run`) with a small input file of `(time, buttons, hat, stick)` rows
   **Then** the console receives the presses in order using the 2wiCC queue commands
   **And** the script and its README say it is throwaway spike code, licensed MIT, and that 2wiCC binaries are used for testing only and never committed
3. **Given** a stop request (Ctrl+C) or the end of the file
   **When** the script exits
   **Then** it sends a neutral state so no input stays pressed

## Tasks / Subtasks

Order matters: tasks 1 to 4 need no console and no hardware. Task 5 (hardware) comes last and is the only place where the owner plugs things in.

- [x] **Task 1: Read the upstream docs and write `spikes/README.md`** (AC: #1, #2)
  - [x] 1.1 Read (read only, never copy code or paste long passages) the 2wiCC README, the 2wiCC_Comms repo, and RiiDraw. Re-read the 2wiCC README yourself: the facts in "Research findings" below came from a summarised fetch and must be checked against the page
  - [x] 1.2 Write `spikes/README.md` sections: purpose and throwaway notice; licence (MIT) and "2wiCC binaries/UF2 are test use only, never committed"; **command subset used** (syntax, line terminator); **state encoding** (button bytes, D-pad bits, stick packing) plus the table mapping our canonical state (AD-10) to it; **timing and queue size** (per frame, no timestamps, default queue 1024); **anti-overflow strategy** (see Technical Requirements); **input file format** with examples; **how to run** (`uv run`, `--dry-run`, `--port`); links to the sources
  - [x] 1.3 Add an "Open questions" section to the README that mirrors the list in this story; every question marked "blocks implementation" or "answer by hardware test". Do not state a guess as fact: mark each unverified line `[UNVERIFIED]` until a hardware result replaces it
  - [x] 1.4 Keep the existing two-line intro of `spikes/README.md` and add an index entry for the new script (the file currently only describes the folder)
- [x] **Task 2: Write the sender script `spikes/send_2wicc.py`** (AC: #2, #3)
  - [x] 2.1 File header: PEP 723 inline metadata (`requires-python`, `dependencies = ["pyserial"]`), then a docstring/comment block with: MIT licence line (project licence, Q5 in the spine), "throwaway spike code, not shipped, not a template for the app", "2wiCC and 2wiCC_Comms binaries/UF2 are used for testing only and are never committed to this repo", and the upstream links
  - [x] 2.2 Input parser (pure function, no I/O): whitespace-separated rows, `#` comments, see "Input file format"; clear error messages with line numbers; rows must be in non-decreasing time order
  - [x] 2.3 State encoder (pure function): canonical state (buttons, hat, lx, ly) to the 2wiCC command string; named constants for every bit, each with a comment citing the source; neutral defined once
  - [x] 2.4 Scheduler (pure function): rows to a per-frame list of states (one entry per frame, repeated while a state is held); frame rate is an option (default 60, `[UNVERIFIED]`, see OQ-3); the last row is held for one frame, then the run ends
  - [x] 2.5 Pacer and serial output: opens the port (`--port`, `--baud` default 460800, 8N1, no flow control), prefills a bounded number of frames, then sends further frames on a wall-clock schedule so that the sent-but-not-yet-played lead never exceeds `--max-lead-frames` (default 30) and never approaches the queue size; the pacer takes an injectable clock/sleep and an injectable writer so tests need no hardware
  - [x] 2.6 `--dry-run`: no port is opened, no `pyserial` import is needed at run time; prints the exact lines that would be sent, with frame index and time, to stdout; exit code 0
  - [x] 2.7 Safe exit: end of file, Ctrl+C (`KeyboardInterrupt`), serial errors and any other exception all end in a `finally` that sends the neutral state (and the stop sequence from OQ-8 once known) and closes the port; a second Ctrl+C during the final send must not skip it; exit code non-zero on error or interrupt
  - [x] 2.8 Clear usage text (`--help`) and one printed summary line at the end (rows, frames, duration, port)
- [x] **Task 3: Tests that run without hardware** (AC: #2, #3)
  - [x] 3.1 Create `spikes/test_send_2wicc.py`, run as `uv run --with pytest --with pyserial pytest spikes/test_send_2wicc.py` (put the command in the README); import the script by path with `importlib` (single-file script, no package)
  - [x] 3.2 Parser tests: valid file, comments and blank lines, button names (`L+A`, `-` = none), hat names/numbers, stick forms, unknown button name, out-of-order time, bad hex, error line numbers
  - [x] 3.3 Encoder tests: neutral, A alone equals the string `+QD 080000` seen in Spike 0b (when the digital-only form is used), each of L, R, A, Y, X, each hat direction, 8-bit to 12-bit stick conversion with the centre `0x80` equal to the neutral
  - [x] 3.4 Scheduler tests: frame count for known times, state held between rows, a 1-frame press, last row held one frame then end, time-to-frame rounding and the `Nf` frame-suffix form
  - [x] 3.5 Pacer tests with a fake clock and fake writer: lead never exceeds the limit, total command count equals frame count, long files (more than the 1024 queue size, e.g. 5000 frames) never put more than the lead on the wire ahead of the fake clock
  - [x] 3.6 Exit tests: neutral is the last thing written after normal end, after a simulated `KeyboardInterrupt` mid-run, and after a writer error; `--dry-run` writes nothing to a port
  - [x] 3.7 Run the tests and the dry run on a sample file and paste the dry-run output of the minimal file in the Dev Agent Record
- [x] **Task 4: Housekeeping without the console** (AC: #1, #2)
  - [x] 4.1 Add two sample input files in `spikes/samples/`: `a-press.txt` (A pressed then released) and `scale-lrayx.txt` (L R A Y X, each about 500 ms with 500 ms gaps, neutral at the end); both documented in the README
  - [x] 4.2 Check that `git status` shows only `spikes/README.md`, `spikes/send_2wicc.py`, `spikes/test_send_2wicc.py`, `spikes/samples/*`, and (after Task 5) `docs/test-plan.md`; no UF2/ELF/BIN, nothing under `ux-designs/*/imports/`, no game screenshots
  - [x] 4.3 Add a "Related" pointer from `docs/test-plan.md` (Spike 0b and Spike 2 mention "a scripted sender") to `spikes/README.md` only if it takes one line; do not rewrite the plan
- [x] **Task 5: Hardware tasks (console needed, do these last, owner present)** (AC: #2, #3)
  - [x] 5.1 Safety first, per `docs/test-plan.md` "Hardware safety": only GP0, GP1, GND wired, nothing on 5V/3V3, rewire only with both USB cables unplugged, Switch on dock/stand with a short cable on the Switch side. Record the test environment (Switch 2 system version, game version, port, cable, firmware + version of 2wiCC and 2wiCC_Comms, host OS) in the "Test environment" table of `docs/test-plan.md` (host OS and cable rows are empty today)
  - [x] 5.2 **Spike 0b `+QD` test from a terminal**: bridge port at 460800 8N1; send `+QD 080000` then `+QD 000000`; A press/release seen on the console. Record the result in the Spike 0b "Result" section of `docs/test-plan.md` (the 2026-11-18 note says RiiDraw already works with this assembly; this is the first direct terminal test of the bare commands)
  - [x] 5.3 **Protocol probes from the terminal** (answers the open questions marked "hardware"): `ID`, `VER`, `GQS`, `GQR` (reply formats, queue size, remaining space), `SPM BUF` and `SPM RT`, queue a few `QD`/`QF` states and watch the cadence, fill the queue to see what happens when full, let it run empty to see what the controller does (holds last state or neutral), find a way to flush or stop (OQ-8), check whether `QD` and `QF` can be mixed. Write every answer into the "Open questions" section of `spikes/README.md` and remove its `[UNVERIFIED]` marks. If a probe changes the design (stop sequence, feedback polling), update the script and its tests, and re-run Task 3
  - [x] 5.4 **Minimal file**: run `uv run spikes/send_2wicc.py --port <bridge port> spikes/samples/a-press.txt`; A is pressed and released on the console; run again and press Ctrl+C in the middle of the press: the neutral state arrives and A is released (AC3); record both in `docs/test-plan.md`
  - [x] 5.5 **L R A Y X scale in free play**: with OoT 1.1.1 in ocarina free play (enter with D-pad left, wait about 2 s, see U9), run `spikes/samples/scale-lrayx.txt`; each note must sound in order and with the pitches of Spike 1.1 (D5 F5 A5 B5 D6). Note wrong or missed notes. Record in `docs/test-plan.md` (a short "Spike sender check" subsection under Spike 2 or after "Spike 1 results", with date and environment)
  - [x] 5.6 Stop in the game the same way: Ctrl+C during the scale leaves no sounding note; leave free play with B afterwards

## Dev Notes

### What this story is and is not

- A throwaway tool (Epic 1 intro: "Spikes use existing firmware only (2wiCC + 2wiCC_Comms, test use, nothing redistributed)"). It is **not** our link protocol (`docs/link-protocol.md`, COBS frames, `QUEUE_EVENT`) and nothing here is reused by `app/` or `firmware/`. Do not add it to any Cargo workspace or CI.
- 2wiCC is a **per-frame queue, not our scheduler** (Story 1.4 AC says so). The script is a best-effort sender; its timing quality is measured in Story 1.4, not promised here.
- All repo content in English. No code copied from 2wiCC, 2wiCC_Comms, RiiDraw or GLaMS2: write your own from the documented behaviour. Cite them as links only.

### Research findings (starting point, verify in Task 1)

Source: 2wiCC README on GitHub (https://github.com/knflrpn/2wiCC), read through a summarising fetch on 2026-11-20. Mark all as `[UNVERIFIED]` in the README until you re-read the page or run the hardware probes.

- Command syntax: `+` then an alphanumeric command type, a **required space** (even without parameter), an optional parameter, and a newline (`\r`, `\n` or both) to commit.
- `QD <6 hex digits>`: queue a digital-only state (3 bytes). `QF <18 hex digits>`: full state (3 digital bytes + 6 analog bytes). `QFI`: QF plus 12 IMU bytes (not needed).
- Button bytes (bit 7 to 0), from the README bit order (masks derived, not printed as a table upstream):
  - byte 1: ZR 0x80, R 0x40, right SL 0x20, right SR 0x10, A 0x08, B 0x04, X 0x02, Y 0x01
  - byte 2: Capture 0x20, Home 0x10, L3 0x08, R3 0x04, + 0x02, - 0x01
  - byte 3: ZL 0x80, L 0x40, left SL 0x20, left SR 0x10, D-pad left 0x08, right 0x04, up 0x02, down 0x01
  - `+QD 080000` = A (matches Spike 0b)
- D-pad is **four separate bits**, not a hat value. Our canonical hat (AD-10: 0 up, 1 up-right, 2 right, 3 down-right, 4 down, 5 down-left, 6 left, 7 up-left, 8 neutral) must be converted (diagonals set two bits).
- Sticks: last 6 bytes of QF = left X, left Y, right X, right Y, Pro Controller 12-bit format, centre 0x800, swing +-0x600 (0x200 to 0xE00). Packing is defined upstream only by a link to an external reverse-engineering doc (see `docs/usb-controller-emulation.md`: dekuNukem). The README example `+QF 000000000880000880` ("no buttons, neutral sticks") is consistent with two axes packed in 3 bytes (`00 08 80` = 0x800 and 0x800). See OQ-2.
- Timing: **per frame**. In `SPM BUF` one queued state plays per frame (60 FPS, or paced by VSYNC if `VSYNC 1`); `SPM RT` uses the latest received state. No timestamps. No repeat counts: one `QD`/`QF` = one frame, so a 500 ms press at 60 fps is 30 queued entries.
- Queue: default size 1024 entries (`GQS` total, `GQR` remaining). Other commands: `VSD` (frame delay, 4 hex digits, microseconds), `REC`, `GR`, `ID` (returns `+2wiCC`), `VER`, `GCS`, `LED`, `RMBL`, `ECHO`. Device messages come back as `+TYPE data\r\n` (`+ERR WATCHDOG`, `+WRN TIMEOUT`, ...).
- Serial link: 460800, 8N1, no flow control; the bridge board runs 2wiCC_Comms (https://github.com/knflrpn/2wiCC_Comms). Also in the same account: GLaMS2 (JS tool, MIT) and gif2mm2 (Rust, MIT) exist; reading them is allowed, copying is not.
- RiiDraw: `https://github.com/knflrpn/RiiDraw` returned 404 on 2026-11-20. Find the real upstream location (see OQ-10).

### Canonical state to 2wiCC mapping (design)

The input file speaks the project's canonical state (AD-10, `docs/usb-controller-emulation.md` P1 table), so files written now keep working when the real streamer arrives: bit order `Y B A X L R ZL ZR - + L3 R3 Home Capture`, hat 0 to 7 with 8 = neutral, sticks centred at 0x80, `ly 0x00` = up. The script converts that to the 2wiCC layout above. Only L, R, A, Y, X, the hat and the left stick are needed (AD-9 allow-list, confirmed by U11); the parser may accept all names but the README documents just those plus B (menu exit) and ZL/ZR/+ for probes. Sending right-stick or system buttons is allowed in the spike but the README must warn that this bypasses the allow-list the real firmware enforces.

### Input file format (needed by Stories 1.3, 1.4, 1.6)

One state change per line, whitespace-separated: `time buttons hat stick`. Trailing columns may be omitted and default to none/neutral.

```
# time    buttons  hat    stick
0         -        -      -          # neutral
500       A        -      -          # press A at 500 ms
1000      -        -      -          # release
```

- `time`: milliseconds from the start (float), or frames with an `f` suffix (`3f`), so Story 1.4 can sweep 1 to 10 frames exactly; a row means "this state is in effect from this time until the next row"
- `buttons`: names joined by `+` (`L+A`), `-` for none; optional hex form `0x0014` (canonical bit order)
- `hat`: `0` to `8`, or `up down left right`, `-` = neutral (8)
- `stick`: `lx:ly` as two hex digits each (`80:40`), `-` = centred; Story 1.3 row 1.6 needs `ly = 0x40, 0x20, 0x00` and the mirrored down values (`0xC0, 0xE0, 0xFF`). A form with three hex digits per axis (raw 12-bit, e.g. `800:5A0`) is useful for threshold sweeps while OQ-2 is open; implement it if it is cheap
- Story 1.6 will generate these files by hand or with a few lines of spike code: keep the format trivial and line-based; no header row

### CLI (needed by Story 1.3 and 1.6)

`uv run spikes/send_2wicc.py [--port PORT] [--baud 460800] [--frame-rate 60] [--max-lead-frames 30] [--dry-run] [--list-ports] FILE`

- `--port` is required unless `--dry-run`; `--list-ports` prints serial ports (pyserial's list tool) so the owner can find the bridge on macOS, Windows and Linux
- Exit codes: 0 normal end, 1 error, 130 Ctrl+C (neutral was sent in every case)
- Output stays human-readable on stdout; errors on stderr

### Anti-overflow and stop strategy (design, to be verified)

- Per-frame queue of 1024 entries, no documented acknowledgements, no documented full-queue behaviour (OQ-4, OQ-5, OQ-6). So the script **never relies on feedback**: it paces by wall clock (frame rate x elapsed time) and keeps the lead (frames sent minus frames the console has played) at most `--max-lead-frames` (default 30, about 0.5 s), far below 1024. Even a 5-minute file therefore never fills the queue.
- Small lead also bounds the Ctrl+C problem: if no queue-flush command exists (OQ-8), states already queued would still play after the neutral; with a 30-frame lead the worst case is half a second of already-queued input, and the neutral must be queued after it. If a flush or `SPM RT` switch turns out to work (hardware task 5.3), use it first and keep the bound as a safety net.
- Clock drift between the PC and the console's 60 Hz (OQ-3) makes the lead slowly grow or shrink on long files; the optional improvement is polling `GQR` (needs its reply format, OQ-5). Implement only if the hardware probes show it is needed; otherwise note the drift in the README for Story 1.4.
- The first frames: prefill up to the lead, then start the queue (how playback starts is OQ-7). Document what you found.

### Files to touch

| File | Action | Notes |
|---|---|---|
| `spikes/README.md` | UPDATE | Today: a 3-line folder intro ("Throwaway experiments, mainly for milestone M2... Nothing here is shipped; findings go back into docs or the ADRs"). Keep it, add the script section described in Task 1. Must not break the sentence that findings go back into docs |
| `spikes/send_2wicc.py` | NEW | Single file, PEP 723 metadata, `uv run` |
| `spikes/test_send_2wicc.py` | NEW | pytest, no hardware |
| `spikes/samples/a-press.txt`, `spikes/samples/scale-lrayx.txt` | NEW | Inputs for Task 5 |
| `docs/test-plan.md` | UPDATE (Task 5 only) | Test environment rows (host OS, cable), Spike 0b "Result" subsection, a short sender check section. Keep its table styles. Do not renumber existing rows |

`spikes/app-egui/` is a Rust spike; leave it alone. `.gitignore` already ignores `*.uf2`, `*.bin`, `__pycache__/`, `.venv/`, `.pytest_cache/`: nothing to add.

### Architecture compliance

- **AD-10 canonical state**: input files and the internal state use it; only the encoder knows the 2wiCC layout. Define NEUTRAL once (buttons 0, hat 8, sticks 0x80).
- **AD-9 allow-list** [`ARCHITECTURE-SPINE.md` AD-9]: the real firmware masks everything outside L, R, A, X, Y, hat, left stick in queue mode. This spike has no mask: say so in the README and keep the sample files inside the allow-list (B appears only if a probe needs it).
- **Spine, AD-14/AD-15 and the link protocol** are about our own firmware. This script is deliberately outside them: no COBS, no CRC, no credits.
- NFR-06/07 (offline, licences): `pyserial` is BSD-3-Clause; it is used by a throwaway script, not shipped. The script is MIT like the rest of the repo (`LICENSE`). 2wiCC has no licence: no code from it enters the repo.
- Hardware safety is in the story as Task 5.1, not optional.

### Library and framework requirements

- Python 3.10 or newer, `uv run` with inline metadata (PEP 723): `# /// script` block with `requires-python = ">=3.10"` and `dependencies = ["pyserial>=3.5"]`. No other runtime dependency. Standard library only for the rest (`argparse`, `time`, `sys`, `dataclasses`).
- Tests: `pytest` supplied with `uv run --with pytest`, not added to the script's metadata.
- Check the current pyserial docs (`ctx7` per the user's global rule) for `serial.Serial` timeouts, `write`, `flush` and `list_ports` before coding; open the port with DTR/RTS handled explicitly if it matters (OQ-9).

### Testing requirements

- Everything in Task 3 must pass on a machine with no serial port. The pacer and writer are injected; no `time.sleep` in tests (fake clock).
- Hardware results are not automated: they are recorded by hand in `docs/test-plan.md` (Task 5).
- Verify by hand once: `uv run spikes/send_2wicc.py --dry-run spikes/samples/a-press.txt` prints the expected lines and exits 0.

### Open questions (updated 2026-11-20 from a read-only reading of the upstream sources)

Sources read (read only, no code copied): the 2wiCC README, `src/2wiCC.c` and `src/2wiCC.h` (https://github.com/knflrpn/2wiCC), `2wiCC_Comms.c` (https://github.com/knflrpn/2wiCC_Comms), and dekuNukem `bluetooth_hid_notes.md` (https://github.com/dekuNukem/Nintendo_Switch_Reverse_Engineering/blob/master/bluetooth_hid_notes.md). Verified by the coordinator and re-checked by the dev agent on 2026-11-20.

Marker: **RESOLVED-BY-SOURCE** = the upstream source settles it; **CONFIRM ON HARDWARE** = still checked in Task 5; **OPEN** = only a hardware probe answers it.

| ID | Question | Answer from the sources | Status |
|----|----------|-------------------------|--------|
| OQ-1 | Exact masks of the three button bytes | README bit order, identical to the Pro Controller standard report (dekuNukem bytes 3/4/5). Byte 1: Y 0x01, X 0x02, B 0x04, A 0x08, SR 0x10, SL 0x20, R 0x40, ZR 0x80. Byte 2: Minus 0x01, Plus 0x02, RStick 0x04, LStick 0x08, Home 0x10, Capture 0x20. Byte 3: DDown 0x01, DUp 0x02, DRight 0x04, DLeft 0x08, SR 0x10, SL 0x20, L 0x40, ZL 0x80. D-pad is four bits: canonical hat 0..8 is converted to bits (diagonals = two bits). Spike 0b `+QD 080000` = A agrees | RESOLVED-BY-SOURCE; **CONFIRMED ON HARDWARE 2026-11-20** (L R A Y X scale in order in free play; A in Spike 0b). Other buttons and D-pad bits: source only |
| OQ-2 | Stick encoding in `QF` | 12 bits per axis, centre 0x800, full swing +-0x600 (README). Packing (dekuNukem): 3 bytes per stick, X = b0 \| (b1 & 0x0F) << 8, Y = (b1 >> 4) \| (b2 << 4); neutral bytes `00 08 80` match the README neutral `000880`. QF analog order: left stick 3 bytes, then right stick 3 bytes. **Y direction (is higher Y = up?) is not settled** | Packing RESOLVED-BY-SOURCE; **Y direction CONFIRMED 2026-11-20**: `+QF 0000000008E0000880` (Y 0xE00) = stick up, `HIGHER_Y_IS_UP = True`. Y 0x200 and 8-bit scaling not tested (Story 1.3) |
| OQ-3 | Frame period | Internal 60 Hz timer, 16666 us per frame. VSYNC is off by default and needs an external signal on GP14 (not wired). `VSD` only matters with VSYNC | RESOLVED-BY-SOURCE (frame rate measured in Story 1.4) |
| OQ-4 | Queue size and unit | Ring buffer of 1024 slots, usable capacity 1023 states (one per frame). Separate serial command ring buffer: 64 commands, max 64 characters each; commands beyond it are dropped | RESOLVED-BY-SOURCE; **CONFIRMED 2026-11-20**: `+GQS 0400`; idle in RT `+GQR 03FE` (one slot holds the current state, 1022 free) |
| OQ-5 | Replies | `QD`/`QF` send no reply. `ID` -> `+2wiCC`, `VER` -> `+VER <version>`, `GQR` -> `+GQR <n>`, `GQS` -> `+GQS <n>` (n printed as 4 upper-case hex digits, so 1024 = `0400`), `GCS` -> `+GCS 0` or `+GCS 1`; replies end with `\r\n`. Malformed commands are ignored silently. Unsolicited lines exist (`+ERR WATCHDOG`, `+RMBL ..`, `+LGHT ..`, `+WRN TIMEOUT ..`): any reader must tolerate them | RESOLVED-BY-SOURCE; **CONFIRMED 2026-11-20**: `+2wiCC`, `+VER 2.2`, `+GQS 0400`, `+GQR 03FE`, `+GCS 1`, ending `\r\n` |
| OQ-6 | Queue full | The new state is dropped silently | RESOLVED-BY-SOURCE (avoided by design, not probed) |
| OQ-7 | Playback start / empty queue / default mode | **Power-up mode is RT (real-time), not BUF.** The sender must send `+SPM BUF` before queuing. In BUF one state is consumed per frame; if the queue runs dry the last state is held, and after about 20 empty frames (about 333 ms) the firmware queues neutral and switches back to RT. The sender must keep the queue fed (lead > 0) for the whole file; the README documents the held-input risk | RESOLVED-BY-SOURCE; BUF playback **CONFIRMED 2026-11-20** (120 queued A frames held A continuously); empty-queue fallback not probed |
| OQ-8 | Immediate stop | `+SPM RT` then `+QD 000000`: in RT the latest state plays at the next frame, which discards the backlog. Used for Ctrl+C, errors and the end-of-file neutral | RESOLVED-BY-SOURCE; **CONFIRMED 2026-11-20**: A released at once, `+GQR 03FE` right after (backlog discarded); script Ctrl+C stopped a sounding note at once |
| OQ-9 | Serial port details | 460800 8N1, no flow control. The bridge forwards host bytes only while the CDC port is "connected" (DTR asserted); pyserial asserts DTR on open by default, so do not disable it. Commands start with `+` and are committed by `\r` or `\n` | RESOLVED-BY-SOURCE; **CONFIRMED on macOS 2026-11-20** (`/dev/cu.usbmodem101`, "2wiCC COM", 2E8A:000A); other hosts not checked |
| OQ-10 | RiiDraw source | RiiDraw is a closed web app (riidraw.com) with no public source. AC1 is met from the 2wiCC sources and the README says so | RESOLVED |
| OQ-11 | May `QD` and `QF` be mixed? | Yes: `QD` writes the same queue slot as `QF` and sets the analog part to neutral | RESOLVED-BY-SOURCE (mixing in one queue not probed on hardware) |

Design consequences (supersede the "Anti-overflow and stop strategy" notes above where they differ):

- Preamble `+SPM BUF` before the first queued state.
- The pacer keeps the lead (frames sent minus frames played, by wall clock) above 0 at all times while playing and at most `--max-lead-frames`, which must stay well under 1023 and under the 64-command serial buffer per burst. If the host falls behind (lead reaches 0), the script stops with an error and sends the stop sequence, instead of letting the firmware hold the last state.
- The neutral on every exit path is the stop sequence `+SPM RT` then `+QD 000000`.
- `QD` is used for frames with both sticks centred, `QF` otherwise (mixing is allowed).

### Previous story intelligence (Story 1.1, done)

- Story 1.1 has no story file (it was a by-hand spike). Its results are in `docs/test-plan.md` "Spike 1 results" and `docs/ocarina-input-mechanics.md`:
  - **Chart (sounding pitches, one octave above community chart names): L=D5, R=F5, A=A5, Y=B5, X=D6** (tuner measured; 1.1). All 19 fingerings B4..F6 verified (1.2): modifiers D-pad up/down = +-1, left stick up/down = +-2, both = +-3, held **before** the button.
  - Expected result of the scale check in Task 5.5: D5, F5, A5, B5, D6 in that order.
  - U1: a held note sounds up to about 6 s then stops itself (wavers from about 2 s). U3: a new button cuts the held one. U4: D-pad changes retune instantly, stick glides. U8/U9: free play enter with D-pad left, input only after about 2 s; exit with B or D-pad left; no limits seen. U11: ZL opens the song menu, right stick moves the camera: the sample files must not use them.
  - Environment: Switch 2 system 23.5.0, OoT 1.1.1, Joy-Con 2 pair; measurement with the Soundcorset tuner on the console speakers (hand-held, not on a table: resonances).
- Spike 0b: wiring and profile P2 already work in practice (RiiDraw, 2026-11-18); known-good cable is the Apple iPhone USB-C cable, 1 m; "Pro Controller Wired Communication" must be ON; console port top and bottom both work.
- Not yet recorded: host OS, 2wiCC/2wiCC_Comms firmware versions, the direct `+QD` terminal test.

### Git intelligence

Recent work: `0e756d5` applied Spike 1 findings to the planning docs, `09168b1` verified buttons by hand (Story 1.1). Pattern: one branch per story (`spike/1-x-...`), docs edits in `docs/test-plan.md`, status kept in this folder. The only code spike so far is `spikes/app-egui/` (Rust). There is no Python in the repo yet, so there is no test config to follow: keep the test file next to the script.

### Latest tech information

- `uv` runs scripts with PEP 723 inline metadata (`uv run script.py`); pyserial 3.5 is the current stable release. Re-check with `ctx7` before coding.
- 2wiCC / 2wiCC_Comms are published as UF2 releases upstream; they are **never** committed here (`.gitignore` has `*.uf2`).

### Project Context Rules

- No `project-context.md` exists in the repo.
- Repo content (files, comments, commit text) is in English; chat with the owner in Italian.
- Never commit `_bmad/`, `bmad-*` or `gds-*` skills; only `_bmad-output/`.
- Never commit or reference anything under `ux-designs/*/imports/`, game screenshots, UF2 or other binaries.
- Do not edit `~/Documents/org/`. Do not commit or push unless the owner asks.

### Project Structure Notes

- `spikes/` holds throwaway experiments; the new files sit directly in it (`spikes/send_2wicc.py`), next to `spikes/app-egui/`. No conflicts.
- `docs/test-plan.md` is the single place for test results; `docs/ocarina-input-mechanics.md` is only touched by Stories 1.3/1.4.

### References

- [Source: `_bmad-output/planning-artifacts/epics.md`#Story 1.2] acceptance criteria; #Story 1.3 first consumer (row 1.6 stick sweeps `ly = 0x40, 0x20, 0x00` and mirrored); #Story 1.4 (frame-exact sweeps 1..10 frames, 20 tries each; "2wiCC's per-frame queue is not our scheduler"); #Story 1.6 (melody files, result recorded in `docs/test-plan.md`)
- [Source: `docs/test-plan.md`#Hardware safety, #Spike 0b, #Test environment, #Spike 1 results, #Spike 2]
- [Source: `docs/usb-controller-emulation.md`#Candidate profiles (P2 and 2wiCC), #P1: HID report (canonical state)]
- [Source: `docs/link-protocol.md`#Why 460800 baud (hypothesis, "proven by 2wiCC"), #Framing (2wiCC is the ASCII-line reference)]
- [Source: `docs/ocarina-input-mechanics.md`#Note buttons, #Pitch modifiers, #Fingering chart, #Unknowns U1 U3 U4 U6 U11]
- [Source: `_bmad-output/planning-artifacts/architecture/architecture-bonooru-2026-11-19/ARCHITECTURE-SPINE.md`#AD-9, #AD-10, Q5 (licence MIT)]
- External, read only: https://github.com/knflrpn/2wiCC, https://github.com/knflrpn/2wiCC_Comms, https://github.com/knflrpn/GLaMS2, dekuNukem Nintendo_Switch_Reverse_Engineering (Pro Controller stick format)

## Dev Agent Record

### Agent Model Used

Claude Opus 5.5 (claude-opus-5-5), via gds-dev-story, 2026-11-20. Tasks 1 to 4 by the agent; Task 5 run on hardware by the owner with the coordinator, results recorded by the agent.

### Implementation Plan

- Pure pipeline in one file: `parse_lines` (rows) -> `schedule` (one canonical `State` per frame) -> `encode` (2wiCC command per frame) -> `Pacer.run` (I/O injected: `write`, `clock`, `sleep`) -> `play` (always ends with `send_stop`).
- Wire format from the 2wiCC README + `2wiCC.c`/`2wiCC.h` + dekuNukem notes, written from the documented behaviour (no code copied). Named constants per bit with source comments; `NEUTRAL` defined once.
- Preamble `+SPM BUF` (power-up mode is RT, OQ-7). Stop sequence on every exit path: `+SPM RT` then `+QD 000000` (OQ-8). `QD` when the left stick is centred, `QF` otherwise (OQ-11).
- Pacer: prefill `max_lead` frames with the preamble in one write, then top up every half frame so 0 < lead <= `max_lead`; if the lead reaches 0 it raises `Underrun` (exit 1 + stop sequence) instead of letting 2wiCC hold the last state. `--max-lead-frames` is capped at 48 so every write fits the 64-command serial buffer; at the end it waits n+1 frames for the queue to play out, then sends the stop sequence.
- Ctrl+C during the stop sequence: SIGINT is ignored while it is sent, and `send_stop` retries if a `KeyboardInterrupt` still reaches the write.

### Debug Log References

- First test run hung: the end-of-playback wait loop could sleep a sub-ulp amount on the fake clock (float time never advanced). Fixed with a 1 ms minimum sleep in the drain loop; the test recorder was also made O(1) per write.
- `ruff check` (not configured in the repo, run ad hoc with `uvx ruff`) flagged 4 style issues (unused variable, `collections.abc` import, redundant `int()`, list literal); fixed, now clean.
- Upstream re-check (read only): `GQR`/`GQS` replies print the number as 4 upper-case hex digits (`+GQS 0400`), recorded in the README and in OQ-5.

### Completion Notes List

- Tasks 1 to 5 done. Task 5 run by the owner on 2026-11-20 (results reported by the coordinator, recorded as given): environment macOS (Darwin 27.2), `/dev/cu.usbmodem101` ("2wiCC COM", 2E8A:000A), Apple USB-C cable, console bottom port, Wired Comm. ON, official 2wiCC v2.2 + 2wiCC_Comms v1.1 (UF2s kept outside the repo), Switch 2 23.5.0, OoT 1.1.1.
- Hardware finding: first session had the two GNDs on unbridged breadboard rails (no common ground): corrupted replies and inconsistent stop/queue probes. Rails bridged (0.1 Ω); earlier probe results discarded. Added a GND continuity check to "Hardware safety" in `docs/test-plan.md` and a note in the README.
- Results: probes `+2wiCC`, `+VER 2.2`, `+GQS 0400`, `+GQR 03FE`, `+GCS 1`; Spike 0b `+QD` A registered; `+SPM RT` + `+QD 000000` is an immediate stop (backlog discarded); stick Y 0xE00 = up (`HIGHER_Y_IS_UP = True` confirmed, Y 0x200 not tested); `a-press.txt` exit 0; scale L R A Y X in order in free play with the expected pitches D5 F5 A5 B5 D6, checked with a tuner (2nd run; 1st run hit a "connect controller" prompt); Ctrl+C in free play stopped the note at once, exit 130. Recorded in `docs/test-plan.md` (Test environment, Spike 0b Result, new "Spike sender check (Story 1.2)") and in the README Open questions.
- Not recorded: Y 0x200 (stick down); queue-full and empty-queue fallback behaviour; QD/QF mixing in one queue; port names on Windows/Linux. These keep `[UNVERIFIED]` or "source only" in the README.
- Post-hardware change: the summary line now also prints `(N frames queued before stop)`, because on Ctrl+C the rows/frames/duration are the planned values; README documents this plus the idle "press A when ready" prompt (no auto press added) and the SIGINT-to-background-`uv run` limitation. `HIGHER_Y_IS_UP` comment updated to "confirmed".
- AC1: `spikes/README.md` documents command subset, state encoding (tables), timing, queue size, anti-overflow and stop strategy, input format, how to run, links; RiiDraw noted as closed (OQ-10).
- AC2/AC3: script and tests written and confirmed on hardware (minimal file, scale in free play, Ctrl+C in free play).
- Defaults chosen without the owner: `\n` line terminator; `QD` for centred-stick frames; right stick always neutral; `HIGHER_Y_IS_UP = True` (OQ-2 Y direction; confirmed on hardware: Y 0xE00 = up); `--max-lead-frames` 1..48 (default 30); underrun = hard stop with exit 1; no `GQR` polling; dry run prints every frame; `docs/test-plan.md` got only the one-line "Related" pointer (Task 4.3).
- Tests: `uv run --with pytest --with pyserial pytest spikes/test_send_2wicc.py` -> `64 passed in 0.04s` after Tasks 1 to 4; `64 passed` again after the Task 5 changes (two exit tests now also check the frames-queued count). After the code-review fixes: `100 passed`, `uvx ruff check` clean.
- Code-review fixes (approved by the owner, 2026-11-20): SIGTERM/SIGHUP handlers route into the same stop path as Ctrl+C (exit 128+signal), and are ignored together with SIGINT while the stop sequence is sent; strict pure-hex parsing for buttons and sticks, ASCII-only digits for time and hat (no `isdigit()`), input read as `utf-8-sig` with a clean error on undecodable bytes; schedule capped at `MAX_FRAMES` = 216000 (1 hour at 60 Hz) with a ParseError naming the line; `--frame-rate` kept, help and README now say it must match the firmware's fixed 16666 us timer and only models other rates; BMad task references removed from `spikes/README.md` and the samples; README documents the extra frame (up to about 16 ms) the last state may play before the stop, what SIGKILL/power loss leaves to the firmware fallback, and the preamble limitation (no queue clear before `+SPM BUF`, unchanged). Bytes sent over serial on the normal and Ctrl+C paths are unchanged (dry-run output of `a-press.txt` identical before and after).
- Second review pass fixes (approved by the owner, 2026-11-20): `play()` ignores SIGINT/SIGTERM/SIGHUP as the first statement of its `finally`, so a second signal cannot skip the stop (regression test fails with the old order); original handlers of all stop signals are captured once before the run and restored at the end; real-signal tests skip on Windows and off the main thread, and also check that signals are ignored during the stop and that SIGINT is restored; `--frame-rate` must be finite and positive; time tokens over 32 characters, non-finite ms and over-long frame counts give a ParseError instead of a traceback; README notes that the SIGTERM/SIGHUP handling and the 129/143 codes are POSIX only (Windows console close and Ctrl+Break not handled), the `nohup` SIGHUP behaviour and the partial-line-on-signal case (both left unchanged). Tests: `110 passed`; `uvx ruff check`: all checks passed; dry-run output of `a-press.txt` identical to the original.
- Dry run `uv run spikes/send_2wicc.py --dry-run spikes/samples/a-press.txt` (exit 0; repeated frames elided here):

```
   pre            +SPM BUF
     0     0.0ms  +QD 000000
     1    16.7ms  +QD 000000
     2    33.3ms  +QD 000000
   ...  (frames 3..27 identical: +QD 000000)
    28   466.7ms  +QD 000000
    29   483.3ms  +QD 000000
    30   500.0ms  +QD 080000
    31   516.7ms  +QD 080000
   ...  (frames 32..58 identical: +QD 080000)
    59   983.3ms  +QD 080000
    60  1000.0ms  +QD 000000
  stop            +SPM RT
  stop            +QD 000000
summary: 3 rows, 61 frames, 1.017 s, port (dry run)
```

- `scale-lrayx.txt` dry run: `summary: 11 rows, 301 frames, 5.017 s, port (dry run)`.

### File List

- `spikes/README.md` (modified)
- `spikes/send_2wicc.py` (new)
- `spikes/test_send_2wicc.py` (new)
- `spikes/samples/a-press.txt` (new)
- `spikes/samples/scale-lrayx.txt` (new)
- `docs/test-plan.md` (modified: "Related" line, Hardware safety GND rule, Test environment rows, Spike 0b Result, "Spike sender check (Story 1.2)")
- `_bmad-output/implementation-artifacts/1-2-spike-sender-for-the-2wicc-queue.md` (this story: Dev Notes Open questions updated from upstream sources, tasks, record)
- `_bmad-output/implementation-artifacts/sprint-status.yaml` (story 1-2 -> review)

## Change Log

- 2026-11-20: Open questions resolved from the upstream sources (2wiCC README/.c/.h, 2wiCC_Comms, dekuNukem); design updated (SPM BUF preamble, lead > 0, SPM RT stop).
- 2026-11-20: Tasks 1 to 4 implemented (README, sender script, 64 hardware-free tests, samples, test-plan pointer). Task 5 pending hardware.
- 2026-11-20: Task 5 hardware results recorded (test plan, README, Open questions); summary line shows frames queued before stop; status -> review.
- 2026-11-20: Code-review fixes applied (signals, input validation, schedule cap, --frame-rate docs, README cleanup); scale pitches confirmed with a tuner. Status stays review.
- 2026-11-20: Second review pass fixes (signal ordering and restore, test portability, clean errors for frame rate and huge times, README POSIX note). Status stays review.
