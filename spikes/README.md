# spikes

Throwaway experiments, mainly for milestone M2 (ocarina mechanics and timing on the console). Nothing here is shipped; findings go back into docs or the ADRs.

## Index

| Path | What |
|---|---|
| `app-egui/` | Rust UI spike |
| `send_2wicc.py` | Sends a file of timed controller states to the 2wiCC queue over the bridge port (Story 1.2). Used by Stories 1.3, 1.4 and 1.6 |
| `test_send_2wicc.py` | Tests for `send_2wicc.py`, no hardware needed |
| `samples/` | Input files for `send_2wicc.py` |
| `gen_trials_1_3.py` | Generates the Story 1.3 trial files (stick threshold U6, the 45 fingerings) into `samples/story-1-3/` |
| `test_gen_trials_1_3.py` | Tests for `gen_trials_1_3.py` and the committed trial files, no hardware needed |
| `samples/story-1-3/` | Generated Story 1.3 trial files (committed; regenerate, do not edit by hand) |
| `listen_trial.py` | Plays a trial file with `send_2wicc.py` while recording the Mac microphone, then reports the pitch of every note (Story 1.3 Part B helper); timing modes for Story 1.4; `--mode melody` and `--mode sustain` for Story 1.6 |
| `test_listen_trial.py` | Tests for `listen_trial.py` with synthetic audio, no microphone or port |
| `gen_trials_1_4.py` | Generates the Story 1.4 trial files (timing limits: U7, U2, U5, row 2.4 jitter) into `samples/story-1-4/` |
| `test_gen_trials_1_4.py` | Tests for `gen_trials_1_4.py` and the committed Story 1.4 files, no hardware needed |
| `samples/story-1-4/` | Generated Story 1.4 trial files (committed; regenerate, do not edit by hand) |
| `samples/story-1-4-ext/` | Generated inputs of extra Story 1.4 trials (committed, checked against their generator command) |
| `gen_trials_1_6.py` | Generates the Story 1.6 trial files (walking skeleton melody from a note list and the profile TOML; U1 sustain trial) into `samples/story-1-6/`. Python 3.11+: `uv run --python 3.12 spikes/gen_trials_1_6.py` |
| `test_gen_trials_1_6.py` | Tests for `gen_trials_1_6.py` and the committed Story 1.6 files (drift, sender parser, timing rules, pitch check against the profile), no hardware needed. Python 3.11+ |
| `samples/story-1-6/` | Story 1.6 note list `ode-to-joy.melody` (source) and the generated trial files (committed; regenerate, do not edit by hand) |
| `serial_rx_log.py` | Read-only logger of what the 2wiCC firmware sends back during a run (`+ERR WATCHDOG`, `+WRN TIMEOUT`, `+LGHT`), with timestamps; start it before the trial, stop it after (Story 1.6 diagnosis) |
| `test_serial_rx_log.py` | Tests for `serial_rx_log.py` with a fake port |
| `count_frames.py` | Counts distinct images per second in a screen video (Story 1.4, U10) |
| `test_count_frames.py` | Tests for `count_frames.py` with synthetic frames (one optional ffmpeg test) |
| `test_default_profile.py` | Consistency checks on the bundled profile `app/profiles/oot-switch2-1.1.1.toml` (Story 1.5; not a profile loader, replaced by Story 5.1). Python 3.11+: `uv run --python 3.12 --with pytest pytest spikes/test_default_profile.py` |
| `recordings/` | Git-ignored output of `listen_trial.py` and the U10 videos (WAV, CSV, Markdown, video); never committed |

## `send_2wicc.py`: 2wiCC queue sender

### Purpose and status

**Throwaway spike code.** It lets the scripted spikes drive the console through existing firmware (2wiCC on the controller board, 2wiCC_Comms on the bridge board) before our own firmware exists. It is not shipped, not part of `app/` or `firmware/`, not our link protocol (`docs/link-protocol.md`), and not a template for the app. Its timing quality is measured in Story 1.4, not promised here: 2wiCC's per-frame queue is not our scheduler.

### Licence and binaries

- The script and its tests are MIT, like the rest of the repo (`LICENSE`). `pyserial` (BSD-3-Clause) is fetched by `uv` at run time, not vendored.
- **2wiCC and 2wiCC_Comms binaries/UF2 are used for testing only and are never committed to this repo.** 2wiCC has no licence; no upstream code was copied. Everything below describes documented behaviour in our own words.

### Sources (read only)

- 2wiCC README, `src/2wiCC.c`, `src/2wiCC.h`: <https://github.com/knflrpn/2wiCC>
- 2wiCC_Comms (USB-serial bridge firmware): <https://github.com/knflrpn/2wiCC_Comms>
- Pro Controller button bytes and stick packing (linked from the 2wiCC README): <https://github.com/dekuNukem/Nintendo_Switch_Reverse_Engineering/blob/master/bluetooth_hid_notes.md>
- RiiDraw is a closed web app (riidraw.com) with no public source, so it could not be read; the subset below comes from the 2wiCC sources alone.

### Command subset used

Each command is ASCII: `+`, the command type, **a space (always, even with no parameter)**, an optional parameter, then `\r` or `\n` to commit. The script ends lines with `\n`. Link: 460800 baud, 8N1, no flow control.

| Command | Used for | Reply |
|---|---|---|
| `+SPM BUF` | Preamble: switch to buffered playback before queuing. Power-up mode is RT | none |
| `+QD hhhhhh` | Queue one frame, digital only (3 bytes); analog set to neutral | none |
| `+QF hhhhhhhhhhhhhhhhhh` | Queue one frame, 3 digital + 6 analog bytes; used only when the left stick is not centred | none |
| `+SPM RT` | Stop: real-time mode plays the latest state at the next frame, so the backlog is discarded | none |
| `+QD 000000` | Neutral, sent right after `+SPM RT` | none |

Not used by the script but useful for the probes (replies measured on 2wiCC v2.2, 2026-11-20): `+ID ` -> `+2wiCC`, `+VER ` -> `+VER 2.2`, `+GQS ` -> `+GQS 0400` and `+GQR ` -> `+GQR 03FE` (numbers are 4 upper-case hex digits; idle in RT one slot holds the current state, so 1022 free), `+GCS ` -> `+GCS 1`. Replies end with `\r\n`. Malformed commands are ignored without a reply. The firmware may also send unsolicited lines (`+ERR WATCHDOG`, `+WRN TIMEOUT ...`, `+RMBL ...`, `+LGHT ...`); the script does not read the port.

### State encoding

Input files use the project's canonical state (AD-10, `docs/usb-controller-emulation.md` P1 table): button bits `Y B A X L R ZL ZR - + L3 R3 Home Capture` (bit 0 to 13), hat 0 to 7 with 8 = neutral, sticks centred at 0x80 with `ly 0x00` = up. The encoder (`encode()`) is the only place that knows the 2wiCC layout.

2wiCC digital bytes (same as the Pro Controller standard report bytes 3, 4, 5):

| Byte | 0x80 | 0x40 | 0x20 | 0x10 | 0x08 | 0x04 | 0x02 | 0x01 |
|---|---|---|---|---|---|---|---|---|
| 1 | ZR | R | SL (right) | SR (right) | A | B | X | Y |
| 2 | - | - | Capture | Home | L3 | R3 | + | - (minus) |
| 3 | ZL | L | SL (left) | SR (left) | D-left | D-right | D-up | D-down |

Canonical to 2wiCC (the buttons the ocarina needs, plus the ones a probe may need):

| Canonical | 2wiCC | Example command |
|---|---|---|
| A | byte 1 0x08 | `+QD 080000` (proven in Spike 0b) |
| B (menu exit only) | byte 1 0x04 | `+QD 040000` |
| X | byte 1 0x02 | `+QD 020000` |
| Y | byte 1 0x01 | `+QD 010000` |
| R | byte 1 0x40 | `+QD 400000` |
| L | byte 3 0x40 | `+QD 000040` |
| ZL / ZR / + (probes only) | byte 3 0x80 / byte 1 0x80 / byte 2 0x02 | |
| hat 0 up, 2 right, 4 down, 6 left | byte 3 0x02, 0x04, 0x01, 0x08 | `+QD 000002` = D-pad up |
| hat 1, 3, 5, 7 (diagonals) | two bits: 0x06, 0x05, 0x09, 0x0A | |
| hat 8 | no D-pad bit | |

Sticks: 12 bits per axis, centre 0x800, full swing +-0x600 (0x200 to 0xE00). Each stick is packed in 3 bytes: byte 0 = X low 8 bits, byte 1 = X high 4 bits (low nibble) and Y low 4 bits (high nibble), byte 2 = Y high 8 bits. Neutral is `00 08 80`. `QF` analog order: left stick 3 bytes, then right stick 3 bytes (the script always sends the right stick neutral). The canonical 8-bit value maps linearly: `0x80` to 0x800, `0x00` to 0x200 (or 0xE00 on Y, see below), `0xFF` to 0xDF4. The raw form `xxx:yyy` (three hex digits) is sent unchanged, for threshold sweeps.

**AD-9 warning:** the real firmware will mask everything outside L, R, A, X, Y, hat and left stick. This spike has no mask: any button in an input file is sent. The sample files stay inside the allow-list.

### Timing and queue size

- One queued state = one frame. No timestamps, no repeat counts: a 500 ms press at 60 Hz is 30 queued entries.
- 2wiCC advances the queue on its own 60 Hz timer (16666 us per frame). VSYNC sync is off by default and needs an external signal on GP14, which is not wired, so `VSD` does not matter here. The 2wiCC timer is not locked to the console's frame rate (U10, Story 1.4).
- Queue: a 1024-slot ring, usable capacity 1023 frames. Separately, the firmware buffers at most 64 serial commands (64 characters each); extra commands are dropped.
- Queue full: the new state is dropped without a reply.
- Queue empty in BUF mode: the last state stays held. After about 20 empty frames (about 333 ms) the firmware queues neutral and falls back to RT. **Risk:** if the sender stalls, the last pressed button stays pressed for up to a third of a second; the script prevents this as below.
- `QD` and `QF` can be mixed in one queue: `QD` writes the same slot with neutral analog.

### Anti-overflow and stop strategy

- The script never relies on replies. It paces by the host clock: frames played = elapsed time x `--frame-rate`. `--frame-rate` must match the firmware timer (fixed at 16666 us, 60 Hz); other values only model a different rate in scheduling and pacing and do not change how fast the console plays the queue.
- It sends `+SPM BUF` plus a prefill of `--max-lead-frames` frames (default 30, about 0.5 s) in one write, then tops the queue up every half frame so that the lead (frames sent minus frames played) stays **above 0 and at most `--max-lead-frames`**. The lead limit is capped at 48, so every write (preamble + prefill included) fits the firmware's 64-command serial buffer and stays far below the 1023-frame queue, however long the file is.
- If the host falls behind (lead reaches 0, for example a stalled machine), the script stops with an error instead of letting 2wiCC hold the last state.
- Exit, every path the script can handle (end of file, Ctrl+C, SIGTERM, SIGHUP, serial error, any other error): the stop sequence `+SPM RT` then `+QD 000000`. A second Ctrl+C (or SIGTERM/SIGHUP) during the stop sequence does not skip it. At the end of the file the script first waits until the queue has played out.
- Depending on where the firmware tick falls, the last queued state may still play for one more frame (up to about 16 ms) before the neutral takes over.
- What the script cannot handle: SIGKILL, a crash of the Python process itself, a pulled USB cable or power loss on the host side. Then no stop is sent and the firmware's own fallback applies: the frames already queued (at most `--max-lead-frames`) play out, the last state is held, and after about 20 empty frames (about 333 ms) 2wiCC queues neutral and returns to RT.
- Known limitation: the preamble does not clear the queue first (no `+SPM RT` before `+SPM BUF`). After a run that ended without the stop sequence (see above), states left in the queue could play before the new file; every handled exit leaves the queue empty. Left as is because it is the sequence verified on hardware.
- Clock drift: the host clock and the 2wiCC timer (16666 us, 60.0024 Hz) differ slightly. Over a 5-minute file the error is well under the 30-frame lead. Polling `GQR` was not needed and is not implemented.

### Input file format

One state change per line, whitespace separated: `time buttons hat stick`. `#` starts a comment; blank lines are ignored. Trailing columns may be omitted (none/neutral). The text after the first `#` of a state row is its label, printed only with `--announce`. A row is in effect from its time until the next row; frames before the first row are neutral; the last row is held for one frame, then the run ends. Rows must be in non-decreasing time order.

- `time`: milliseconds from the start (`500`, `12.5`), or frames with an `f` suffix (`3f`). Milliseconds are rounded to the nearest frame; a row that gets zero frames prints a warning.
- `buttons`: names joined by `+` (`L+A`), case-insensitive, `-` for none. Names: `Y B A X L R ZL ZR MINUS PLUS L3 R3 HOME CAPTURE`. Or hex in canonical bit order (`0x0014` = A+L).
- `hat`: `0` to `8`, or `up up-right right down-right down down-left left up-left neutral`; `-` = neutral.
- `stick` (left stick): `lx:ly` with two hex digits each (`80:40`, canonical, `ly 00` = up), or three hex digits each (`800:5A0`, raw 12-bit wire values); `-` = centred.

```
# time  buttons  hat  stick
0       -        -    -        # neutral
500     A        -    -        # press A at 500 ms
1000    -        -    -        # release
10f     -        up   80:00    # D-pad up and stick up from frame 10
```

Samples in `samples/`:

- `a-press.txt`: A pressed for 500 ms, then released; the first end-to-end check (also run with Ctrl+C during the press)
- `scale-lrayx.txt`: L R A Y X, each 500 ms with 500 ms gaps, neutral at the end; in ocarina free play it sounds D5 F5 A5 B5 D6 (checked with a tuner on 2026-11-20)

### How to run

```sh
uv run spikes/send_2wicc.py --list-ports                                  # find the bridge port
uv run spikes/send_2wicc.py --dry-run spikes/samples/a-press.txt          # print the lines, open nothing
uv run spikes/send_2wicc.py --port /dev/tty.usbmodemXXXX spikes/samples/a-press.txt
```

Options: `--port` (required unless `--dry-run`), `--baud` (460800), `--frame-rate` (60, must match the firmware), `--max-lead-frames` (30, 1 to 48), `--announce` (Story 1.3: print each row's `# label` as `t=12.3s  label` when its first frame plays, estimated from the host clock, not when it is queued up to `--max-lead-frames` earlier; with `--dry-run` the labels appear next to the frame lines). Without `--announce` the output and the bytes sent are unchanged. Exit codes: 0 normal end, 1 error, 130 Ctrl+C, 143 SIGTERM, 129 SIGHUP; the neutral is sent in all these cases once the port is open (not after SIGKILL or power loss, see above). The 129/143 codes and the SIGTERM/SIGHUP handling are POSIX only: on Windows, closing the console window does not deliver SIGTERM, and Ctrl+Break (SIGBREAK) is not handled, so neither sends the stop.
Under `nohup` the script still installs its own SIGHUP handler, so a hangup stops the run (with the neutral) instead of being ignored. If a signal interrupts a write half-way, a partial command line may go out just before the stop sequence; this is not handled, to keep the byte sequence verified on hardware, and was not seen in the hardware runs. A summary line is printed at the end: rows, frames and duration are the **planned** values from the file (also after Ctrl+C); the trailing `(N frames queued before stop)` is what was actually sent.

On macOS the bridge shows up as `/dev/cu.usbmodem101` (product "2wiCC COM", VID:PID 2E8A:000A); names on other hosts were not checked.

Notes from the hardware run (2026-11-20, see `docs/test-plan.md` "Spike sender check (Story 1.2)"):

- **Common ground first.** Check GND to GND between the two boards with a multimeter (expect ~0 Ω) before powering. Breadboard top and bottom power rails are not connected unless bridged; without a common ground commands mostly got through but replies were corrupted (`+2wicc��`) and queue/stop probes were inconsistent.
- After the controller sits idle, the Switch may show "press A when the controller is ready"; a short A press dismisses it (in a terminal: `+SPM RT`, `+QD 080000`, about 0.15 s, `+QD 000000`). The sender does not do this by itself, because in free play it would play a note.
- Ctrl+C must come from a real terminal. A SIGINT sent to a background `uv run` from a non-interactive shell does not reach the script (uv does not forward it, and background jobs start with SIGINT ignored); automated stop tests need another way to deliver it.

The port is opened 8N1, no flow control, with DTR asserted (pyserial default): 2wiCC_Comms only forwards host bytes while the USB serial port is "connected" (DTR set), so do not disable DTR.

Tests (no hardware):

```sh
uv run --with pytest --with pyserial pytest spikes/test_send_2wicc.py
```

### Open questions

Answered from the upstream sources on 2026-11-20, then checked on hardware the same day (2wiCC v2.2, 2wiCC_Comms v1.1, macOS, Switch 2 23.5.0, OoT 1.1.1). `[UNVERIFIED]` marks only what is still not checked.

| ID | Question | Answer | Status |
|---|---|---|---|
| OQ-1 | Button masks | Table above (2wiCC README bit order = Pro Controller report). A proven in Spike 0b; L R A Y X played in order in free play with the expected pitches D5 F5 A5 B5 D6 (tuner) | Confirmed on hardware (scale test, tuner). Other buttons (B, ZL, ZR, +, ...) and D-pad bits: source only `[UNVERIFIED]` |
| OQ-2 | Stick encoding | 12-bit, centre 0x800, +-0x600, 3-byte packing as above. `+QF 0000000008E0000880` (left Y 0xE00) moved the stick **up**: higher Y = up, `HIGHER_Y_IS_UP = True` | Confirmed on hardware (Story 1.3, 2026-11-20): Y up; down works with canonical `ly 0xFF` = wire Y 0x20C (full −2 semitones), and the linear 8-bit scaling is confirmed by `u6-ly-down.txt` and `u6-sweep.txt` (pitch about linear in cents across the 0x10 steps, mirrored up and down). Raw Y 0x200 itself was not sent (`u6-raw-ly.txt` not needed) |
| OQ-3 | Frame period | Internal 60 Hz timer, 16666 us; VSYNC off by default. Measured (Story 1.4, test plan 2.4 and 2.5): over 30 s of 7/8-frame notes the heard onsets follow the queued frame counts with no measurable drift (at most 0.22 ms per s); the game's frame period is 16.67 ms when the picture changes (sustained 60 images per second not shown) and the per-note latency falls into two clusters one frame apart, because the 2wiCC timer is not phase-locked to the game | Measured on hardware (2026-11-21) |
| OQ-4 | Queue size | 1024 slots, 1023 usable; `+GQS 0400`; idle in RT `+GQR 03FE` (one slot holds the current state). 64-command serial buffer | Confirmed on hardware (GQS/GQR) |
| OQ-5 | Replies | None for `QD`/`QF`; `+2wiCC`, `+VER 2.2`, `+GQS 0400`, `+GQR 03FE`, `+GCS 1`, all ending `\r\n`; malformed commands ignored; unsolicited lines exist | Replies confirmed on hardware; unsolicited lines: source only |
| OQ-6 | Queue full | New state dropped silently | Source (avoided by design, not probed) |
| OQ-7 | Start, empty queue, default mode | Power-up mode is RT; `+SPM BUF` is sent first. In BUF, 120 queued A frames held A continuously | BUF playback confirmed on hardware; empty-queue fallback (neutral + RT after about 20 frames): source only |
| OQ-8 | Immediate stop | `+SPM RT` then `+QD 000000` released A at once, and `+GQR` right after read `03FE` (backlog discarded). Ctrl+C from the script stopped a sounding note at once | Confirmed on hardware |
| OQ-9 | Serial details | 460800 8N1, no flow control, DTR asserted (pyserial default) works; `\n` commits. macOS port `/dev/cu.usbmodem101` | Confirmed on macOS; other hosts not checked |
| OQ-10 | RiiDraw source | Closed web app, no public source | Resolved |
| OQ-11 | Mixing `QD` and `QF` | Allowed per source; script uses `QD` with centred stick, `QF` otherwise | Source; mixing in one queue not probed `[UNVERIFIED]` |

## Story 1.3 trials (`gen_trials_1_3.py`)

Input files for the stick threshold (U6, Spike 1 row 1.6) and the 45 button x modifier combinations (row 1.9). Throwaway spike code, MIT, standard library only. The files in `samples/story-1-3/` are generated and committed; run them as they are. Each file starts with 1 s of neutral; every note sets the D-pad/stick first, presses the button 500 ms later, holds it, releases, keeps the modifiers 200 ms, goes neutral, then waits 1500 ms. Every row has a `# label`; `PRESS` rows are the notes.

| File | Notes | Duration | What |
|---|---:|---:|---|
| `calibration-A.txt` | 1 | 4 s | one A press (model A5), held 2.5 s: mic level and alignment check before a session |
| `u6-ly-up.txt` | 15 | 55 s | base A, `ly` 0x40, 0x20, 0x00 (wire 0xB00, 0xC80, 0xE00), 5 presses each, hold 1.5 s |
| `u6-ly-down.txt` | 15 | 55 s | base A, `ly` 0xC0, 0xE0, 0xFF (wire 0x500, 0x380, 0x20C), 5 presses each |
| `u6-sweep.txt` | 18 | 93 s | base A, one press per step, hold 3 s: rest 0x80, 0x70 to 0x00 (step 0x10); rest, 0x90 to 0xF0, 0xFF |
| `u6-diagonals.txt` | 30 | 111 s | base A, 3 presses each: lx-only controls `00:80`, `FF:80`; circle `26:26`, `DA:26`, `26:DA`, `DA:DA`; corners `00:00`, `FF:00`, `00:FF`, `FF:FF` |
| `fingerings-L/R/A/Y/X.txt` | 9 each | 42 s each | one button x the 9 modifier states (none, D-up, D-down, stick-up, stick-down, then the four D-pad + stick pairs), stick at full deflection `80:00` / `80:FF`, hold 2.5 s |
| `u6-raw-ly.txt` | 33 | 122 s | optional finer round: raw 12-bit `ly` 800 (control), A00 to E00, 600 to 200, 3 presses each |

Regenerate (same bytes every run) and print the key (press times and the model pitch, a reading aid only, never a result):

```sh
uv run spikes/gen_trials_1_3.py --key
uv run spikes/gen_trials_1_3.py --ly-up 38,30,28 --sweep-step 0x08 --out-dir /some/other/dir   # a finer round
```

Options: `--lead-in`, `--mod-lead`, `--hold` (U6 files), `--fingering-hold`, `--sweep-hold`, `--tail`, `--gap` (ms); `--repeats`, `--diag-repeats`; `--ly-up`, `--ly-down` (comma-separated hex); `--sweep-step`, `--stick-up`, `--stick-down` (hex); `--out-dir`. Holds must stay under 6 s (the note auto-stops at about 6 s, U1). A finer round written over the committed folder makes the drift test fail on purpose; write it elsewhere or restore the defaults.

Run a trial (free play first):

1. Do the "Hardware safety" checks in `docs/test-plan.md` (GND to GND continuity first).
2. On the real controller: open the ocarina in free play with D-pad left, dismiss any "press A when the controller is ready" prompt by hand, wait 2 s. Keep the real controller within reach.
3. `uv run spikes/listen_trial.py --port /dev/cu.usbmodem101 spikes/samples/story-1-3/u6-ly-up.txt` (records the mic and reads every note, see below), or the sender alone with a hand-held tuner: `uv run spikes/send_2wicc.py --port /dev/cu.usbmodem101 --announce spikes/samples/story-1-3/u6-ly-up.txt`.
4. Stop with Ctrl+C in a real terminal if anything leaves free play.

Forbidden inputs (the generator raises an error, the tests check every generated state): any button other than L R A X Y (never B, which exits free play, never ZL, which opens the song menu, never ZR, L3, R3, +, -, Home, Capture); any D-pad state other than neutral, up and down (left and the diagonals 5, 6, 7 contain D-pad left, which closes the ocarina); the right stick (not even writable in the input format). The sender has no mask of its own: never edit a trial file by hand to add these.

### `listen_trial.py`: pitch from the microphone

Reading a phone tuner for every press is slow, so this helper records the game audio with the host microphone and reads each note itself. Throwaway spike code, MIT; numpy and pyserial come through the PEP 723 header. macOS only (ffmpeg `avfoundation`); the terminal app needs microphone permission (System Settings, Privacy & Security, Microphone).

What it does:

1. Starts `ffmpeg -f avfoundation -i ":<device>"` (default device `MacBook Pro Microphone`; an index such as `0` also works; `--list-devices` lists them), mono 48 kHz, raw PCM on a pipe; the script writes `spikes/recordings/<trial>-<date>.wav` and timestamps every audio chunk on the host clock. ffmpeg runs in its own session, so a terminal Ctrl+C does not cut the recording.
2. After 0.3 s it runs the **unchanged** sender, `send_2wicc.py --port <port> --announce <trial>`, in the same process group (a real Ctrl+C reaches it directly and it sends its stop sequence; the helper forwards one SIGINT only if the sender is still running 0.5 s later). The bytes sent are the same as without the helper; `--announce` only adds stdout lines, which are echoed.
3. Alignment: the time of the schedule's t=0 is taken from the `--announce` lines matched in order to the file's labelled rows (each printed within about half a frame of its frame). The audio side comes from the chunk arrival times: the delivered sample rate is fitted (least squares, first 0.5 s skipped) and sample 0 is placed on the earliest arrival. `offset` = WAV time of t=0, `scale` = WAV seconds per real second (1.0 for a clean capture; below 1 when the capture loses samples, with a warning). Note windows come from the trial file itself (the sender's parser: PRESS row to release row), mapped as `offset + scale x t`.
4. After the sender exits, 1 s more is recorded (`--tail`), ffmpeg is stopped, and each PRESS row is analysed: window from press + 0.3 s to release - 0.2 s (`--attack`, `--release-guard`), the recording is band-passed to 250 to 4000 Hz first (room and console rumble sits below 200 Hz), then YIN pitch (300 to 2000 Hz, first dip under the threshold, which avoids octave-down picks), median Hz of the voiced frames, nearest note (A4 = 440 Hz) and cents, band level in dBFS, noise just before the press capped by the file's quiet floor (10th percentile of 50 ms blocks) for the SNR, onset after the scheduled press (USB + console + audio latency). "NO SOUND" when the level is under -60 dBFS, the SNR under 10 dB, or fewer than half the frames are voiced.
5. Verdict from the model (button + D-pad + stick from the row's state, the same model as the generator): stick rows give `counted` (pitch moved by 2), `not counted` (base pitch), or `OTHER`; rows without stick give `match` or `OTHER`. The model is a reading aid; the measured Hz is the result.

Output: a table on stdout, plus `<wav>.csv` and `<wav>.md` next to the WAV in `spikes/recordings/` (git-ignored: game audio is never committed). Copy the numbers into `docs/test-plan.md` by hand.

```sh
uv run spikes/listen_trial.py --list-devices
uv run spikes/listen_trial.py --port /dev/cu.usbmodem101 spikes/samples/story-1-3/calibration-A.txt   # 4 s check first
uv run spikes/listen_trial.py --port /dev/cu.usbmodem101 spikes/samples/story-1-3/u6-ly-up.txt
uv run spikes/listen_trial.py --analyse spikes/recordings/<file>.wav spikes/samples/story-1-3/u6-ly-up.txt --offset 0.342
```

The live run prints the exact `--analyse` command with its offset and scale (`--scale`, default 1).

First live calibration (2026-11-20, MacBook Pro microphone): the A5 was clearly in the WAV (about 894 Hz, +28 cents, about -40 dBFS, from about 1.77 s to 4.05 s) but every run said NO SOUND. Two causes: (1) the noise reference was broadband and dominated by sound below 200 Hz, so the SNR was 7 to 8 dB; (2) ffmpeg's avfoundation capture lost about 9 % of the samples in 1024-sample packets (a phase jump every 64 to 128 ms, the 2.5 s note lasted 2.25 to 2.28 s in the WAV), so the windows drifted late and the pre-press noise window already held the note. Fixes: band-pass before every measurement, the quiet-floor cap on the noise, `-thread_queue_size 4096` on the ffmpeg input (ffmpeg drops packets from a live device that are not read in time) with its warnings read and printed, and the measured `scale`. Those three recordings were made before the scale was measured; re-analyse them with `--scale 0.912 --offset <old offset x 0.912>` (or with the old offset and scale 1, which also works now thanks to the floor cap). If a live run still prints the "samples lost" warning, the scale keeps the windows right, but report it. Other options: `--device`, `--sample-rate`, `--out-dir`, `--tail`, `--latency` (extra shift, for example the median onset if the windows look late), `--min-dbfs`, `--min-snr-db`, `--max-lead-frames` (passed to the sender). The exit code is the sender's.

Tests:

```sh
uv run --with pytest --with pyserial --with numpy pytest spikes/
uv run --python 3.12 --with pytest --with pyserial --with numpy pytest spikes/   # the whole folder, incl. the 3.11+ tests
```

Without `--with numpy` the `listen_trial.py` tests are skipped and the others still run.

## Story 1.4 trials (`gen_trials_1_4.py`)

Input files for Spike 2 rows 2.1 (U7 min press), 2.2 (U2 min gap on the same button), 2.3 (U5 modifier lead) and 2.4 (onset jitter). Throwaway spike code, MIT, standard library only; it reuses the 1.3 allow-list guard (`check_allowed`) by import. All times are frames of the sender's 60 Hz grid (`130f`), so the ms-to-frame rounding cannot blur a 1-frame step. Every attempt is: 30 neutral frames (the analyser's quiet window), the press (and modifier), 90 neutral frames. Scored attempts are the `TRY` rows (machine-readable fields such as `press=3f`, `gap=5f`, `mod=dup lead=2f`); every file starts with one `WARMUP` attempt that is not scored (after a Joy-Con connect or disconnect the first injected input is ignored).

**Rounds, not blocks.** Each round holds every step once in a seeded shuffled order, so every step sees the same mix of sub-frame phases between the 2wiCC timer and the game clock; 20 rounds = 20 tries per step, split 5 rounds per file. Run all parts of one row in one session.

| Files | Row | Each round | Duration per file |
|---|---|---|---:|
| `u7-minpress-part1..4.txt` | 2.1 | A pressed 1 to 10 frames, plus one blank (no press, must stay silent) | 117 s |
| `u2-gap-part1..4.txt` | 2.2 | A 12 frames, release 1 to 10 frames, A 12 frames; controls gap 0 (one 24-frame press) and gap 30 | 153 s |
| `u5-lead-{dup,ddown,sup,sdown}-part1..4.txt` | 2.3 | modifier (D-up, D-down, stick `80:00`, stick `80:FF`) set 0 to 5 frames before A, note 36 frames, modifier kept 6 frames after the release | 84 s |

Totals: U7 about 8 min, U2 about 10 min, U5 about 22 min (5.6 min per modifier).

`samples/story-1-4-ext/` holds the inputs of extra recorded trials, committed for traceability: `confirm-u2-R-5f.txt` and `confirm-u2-R-4f.txt` from `uv run spikes/gen_trials_1_4.py --set confirm --u2 5 --buttons R --out-dir spikes/samples/story-1-4-ext` (the tests regenerate them and compare).

Generated later, from the round 1 results (then committed in the same folder):

```sh
uv run spikes/gen_trials_1_4.py --key                                       # (re)write round 1, print the key
uv run spikes/gen_trials_1_4.py --leads 0-5,6,8,10,12,15,20,30 --mods sup --out-dir spikes/samples/story-1-4-u5ext   # U5 extension (not needed: U5 = 0f)
uv run spikes/gen_trials_1_4.py --set confirm --u7 N --u2 N --u5 dup=N,ddown=N,sup=N,sdown=N   # L R Y X, threshold and one below
uv run spikes/gen_trials_1_4.py --set jitter --press-frames 3 --min-press 2 --min-gap 4        # row 2.4 (the measured U7/U2)
```

The extension writes the whole round 1 set with the new leads into its own folder (only the `u5-lead-*` files there are needed); never write a changed set over `samples/story-1-4/`, the tests compare it byte for byte. `--set confirm` writes one file per button and value (`confirm-u7-L-3f.txt`, ...), 20 tries each, with the blanks or gap controls of that row. `--set jitter` writes `jitter-120bpm-16ths.txt`: 240 notes of 120 BPM sixteenths (7.5 frames, so the sender grid alternates 7 and 8 frames, a known quantisation), cycling L R A Y X Y A R, press `--press-frames` frames, the rest of each slot released, 2 s lead-in; it refuses a press or gap below `--min-press` / `--min-gap`. Other options: `--pre`, `--post`, `--rounds`, `--rounds-per-file`, `--button`, `--press-frames`, `--hold-frames`, `--gap-frames`, `--leads`, `--mods`, `--note-frames`, `--tail`, `--buttons`, `--out-dir`. Unsafe values (press under 1 or over 120 frames, gap under 1, B/ZL/ZR or any other button, a left or diagonal D-pad, notes near the 6 s auto-stop) exit 1 before any file is written.

Run order, free-play entry and the forbidden inputs are the same as for Story 1.3 (above): hardware safety first, open free play with D-pad left on the real controller, dismiss prompts by hand, wait 2 s, stop with Ctrl+C in a real terminal, never edit a file to add a forbidden input.

### `listen_trial.py` timing modes and the lossless capture (Story 1.4)

Expected pitches include the game's tuning (+27 cents, Story 1.3). The first live run (2026-11-21) showed that broadband levels and short-window YIN are not enough for X (D6 is about 10 dB above the room noise in 250 to 4000 Hz, so blanks "sounded" and notes did not, and YIN read f/2 or f/3); the modes below measure at the expected pitch instead.

- `--mode detect` (row 2.1): sound = power within +-3 % of the expected pitch in press + 0.02 s to press + 0.30 s at least `--band-snr-db` (8) above the same band before the press; pitch = strongest spectral peak between 0.75 and 1.33 x the expected pitch (no octave errors); a strong peak at another pitch is a FAIL pitch; blanks must stay silent. Onset and audible length (above peak - 20 dB) come from the level at the expected pitch (10 ms window) and are informative only.
- `--mode retrigger` (row 2.2): onsets per attempt from the 2 ms envelope (6 ms smoothing) of the note's fundamental (0.8 to 1.25 x the expected pitch). A retrigger is a dip of at least `--dip-db` below the level before it followed by a rise of at least `--rise-db`. Defaults from the controls of the same file: the deepest valley of the `gap 0` controls plus 6 dB, at most half way to the shallowest valley of the `gap 30` controls. The dip depth grows smoothly with the gap (the note decays while released), so the threshold is a definition; pool the controls of all files of one button with `--combine ... --pool-controls`, which re-scores every attempt with one threshold and prints the deepest held-note dips (one outlier raises the threshold).
- `--mode lead` (row 2.3): the onset is the -6 dB crossing of the level of the whole search band, 0.8 to 1.25 x the expected pitch (`band_onset`), so a start at a wrong pitch (a missing modifier is up to 2 semitones away) is not skipped; the steady pitch is the strongest spectral peak in the last half of the note, the start pitch the median of the peaks of the 9 overlapping 30 ms windows (hop 5 ms) that start in the first 40 ms, all searched in that band (wide enough for a missing modifier, too narrow for an octave error); pass within `--lead-cents` (50). The first window's error is reported too (the stick bend glides at the start). A wrong pitch shorter than about 35 ms (2 frames) cannot be seen (synthetic notes: 33 ms passes, 36 ms fails).
- `--mode jitter` (row 2.4): one level envelope per button pitch (20 ms window, enough to separate A5 and B5, 110 Hz apart); one latency for the whole run (the shift at which every scheduled note's own button is loudest); each onset is the -6 dB crossing of its button's envelope near the expected time, kept only if that button is at least 3 dB louder than the others. Reports the inter-onset-interval error against the sender's frame schedule (mean, sigma, p95, max in ms and frames), the latency and its per-note spread with a histogram (two clusters one frame apart = the game's frame quantisation), the drift and, separately, the schedule's own 7/8-frame quantisation.
- Every mode writes a CSV and a Markdown report next to the WAV; with `--analyse`, `--report-tag NAME` writes `<wav>-NAME.csv/.md` instead, so the reports of the live run are kept. The step table shows passes per step, the rounds of the failures, and the smallest step for which it and every larger step passed every try. `--combine A.csv B.csv ...` merges the parts of one row into one table; it refuses two reports of the same recording (for example the live report and its `-rescored` re-analysis), and notes repeat runs of the same trial file.
- `--backend sounddevice` records through PortAudio (callback into a lock-free queue, a writer thread writes the WAV). Any input overflow flag is counted and printed; a run with an overflow is marked invalid. ffmpeg (`--backend ffmpeg`, the default) loses about 11 % of the samples in 1024-sample packets (ffmpeg trac #11398): acceptable for yes/no per attempt, not for jitter. `sounddevice` comes with the PEP 723 header of `listen_trial.py`; the tests do not need it.
- `--selftest-tone` (no trial, no port): records `--seconds` (60) of a steady `--tone-hz` (1000) tone played by another device near the microphone and counts phase jumps larger than 0.2 ms plus the delivered-sample ratio. Pass: 0 jumps, ratio within 0.1 %, no overflow. Run it once per session before the trials.

```sh
uv run spikes/listen_trial.py --selftest-tone --backend sounddevice --seconds 60
uv run spikes/listen_trial.py --port <port> --backend sounddevice --mode detect spikes/samples/story-1-4/u7-minpress-part1.txt
uv run spikes/listen_trial.py --combine spikes/recordings/u7-minpress-part*-??????.csv          # live reports only
uv run spikes/listen_trial.py --combine spikes/recordings/u2-gap-part*-rescored.csv --pool-controls
uv run spikes/listen_trial.py --analyse REC.wav TRIAL.txt --offset 0.354 --scale 1.0 --mode detect --report-tag rescored
```

## Story 1.6 trials (`gen_trials_1_6.py`)

Walking skeleton (risk R4): the first whole melody from a note list, through the measured profile and the spike sender, to sound in free play; plus the short U1 sustain trial of the same session. Throwaway spike code, MIT, standard library only, **Python 3.11+** (`tomllib`): run it with `uv run --python 3.12`. It is not a precursor of the Epic 5 pipeline (Story 5.5 writes the real timeline compiler).

**Melody files.** `samples/story-1-6/ode-to-joy.melody` holds the note list: the whole Ode to Joy theme (A, A', B, A'; Beethoven, public domain) in G major with the tonic at G5, 62 notes, 64 beats, D5 to D6, one note per line `<sounding pitch> <beats>` (`R <beats>` is a rest). The generator reads every value from `app/profiles/oot-switch2-1.1.1.toml` (`[buttons]`, `[modifiers]`, `[range]`, `[[fingerings]]`, `[timing]`, `[profile] frame_rate`) and:

- takes the `chart = true` fingering of each pitch (a pitch outside `[range]` or without a chart row is an error naming the note; no transpose);
- puts the onsets on the frame grid from **cumulative** beats (beat = 3600 / BPM frames at 60 Hz), first press at `start_lead_ms`;
- never moves an onset, it shortens the previous press instead: `min_gap_same_button_frames` before a note on the same button, `modifier_lead_frames` before a note whose D-pad or stick state changes (the new modifiers are set there), a hand-over at the onset otherwise (U3: one note button at a time); a rest is neutral;
- re-presses a note longer than `max_sustain_ms` after the min gap (FR-05a; Ode to Joy has none);
- **caps every press at 400 ms** (`DEFAULT_MAX_PRESS_MS`, staccato: a longer note is a 400 ms press, then neutral until the next onset). This is a Story 1.6 workaround, see "Known issue" below; `--max-press-ms 0` writes an uncapped `-uncapped.txt` variant;
- refuses a press under `min_press_frames`, a same-button gap under the min gap, a modifier change while a note sounds, and any input outside the allow-list.

Every note row has a label `NOTE 07 B5 83 fingering=Y` (index, sounding pitch, MIDI, fingering) that `listen_trial.py --mode melody` reads.

| File | Tempo | Shortest press | Longest press | Duration | Use |
|---|---:|---:|---:|---:|---|
| `ode-to-joy-60bpm.txt` | 60 BPM (beat 60 frames) | 24 frames | 24 frames (cap) | 65.5 s | by ear first; not run in its final (capped, simplified) form |
| `ode-to-joy-90bpm.txt` | 90 BPM (beat 40 frames) | 15 frames | 24 frames (cap) | 44.2 s | main run, 3 times with the analyser |
| `ode-to-joy-120bpm.txt` | 120 BPM (beat 30 frames) | 10 frames | 24 frames (cap) | 33.5 s | optional stress run; not run in its final (capped, simplified) form |

The shortest press is the eighth note before the same button (B5 then C6, both on Y): eighth minus the 5-frame gap (at 60 BPM that is 25 frames, so the cap sets every press to 24 frames).

**Known issue: about every 7th note is cut (2026-11-21, Part B).** With full-length (legato) presses, the game ends a held note by itself 0.54 to 0.91 s after its onset, about every 7th sounded note (07, 15, 23, ... in the 60 BPM runs; 07 and 14 even with 17 identical A5 notes), and ignores input for up to about 1 s, so a note that follows by hand-over or within that time is lost. This is a hypothesis; the mechanism is unknown. Not an analyser artefact (the audio is silent at those moments). No evidence for a sender or 2wiCC queue cause (later notes stay on schedule; 0 firmware lines in one diagnostic run with `serial_rx_log.py`), and the dev agent's offline reading of the OoT decompilation (read only, not verified on hardware) found no song recognition or input lock that would explain it. With the 400 ms cap there was no audible event in 1 full 60 BPM run (the owner heard one "silent" note there, attributed offline to the cap plus the then syncopated rhythm) and 1 short run (notes 1-8); the 3 official capped 90 BPM runs of the final (simplified) file were 62/62. Owner decision (2026-11-21): the 400 ms staccato cap is the spike default; re-pressing long notes is the preferred direction for the real pipeline (follow-up); whether a value belongs in the profile is an open owner call. Evidence: `docs/test-plan.md`, "Walking skeleton (Story 1.6)". The U1 files keep their long holds; mind the event when reading short auto-stop times there.

Diagnostic options for `--set melody` (files are labelled `DIAGNOSTIC`, write them to `spikes/recordings/diag/`, never over the committed folder): `--notes A-B` (only those notes, original numbering), `--extra-lead-ms N` (neutral before the first press), `--max-press-ms N` (another cap; `0` = uncapped). `serial_rx_log.py --port PORT` logs what the firmware sends back during a run (start it first, stop it after the run).

**U1 files.** `u1-sustain-A-part1..5.txt`: one round each (shuffled, a different order per round), 9 scored attempts plus a `WARMUP` per file: A held 4, 5, 5.5, 6, 6.5 and 7 s, a 10 s control (gives the auto-stop time directly), a 1 s control (must be held to its release) and a blank (no press, must stay silent). Every attempt is 30 neutral frames, the hold, 180 neutral frames (3 s). 80.2 s per file, about 6.7 min for the five. These holds are longer than the 6 s guard of the older generators on purpose; this generator has its own bound (at most 12 s, buttons L R A Y X only, no modifiers). `TRY` labels: `TRY u1 A hold_ms=5500 attempt=3 round 2/5` (`blank`, `control`).

`u1-confirm-<L|R|Y|X>.txt` are generated **in the session** after round 1 from the candidate limit: 5 attempts at the candidate and 5 at the next longer tested duration (candidate + 500 ms beyond 7 s), shuffled, plus a warm-up, on each of L, R, Y, X (96.2 s per file for a 5500 ms candidate). Commit them with the results; the tests regenerate them from the `# u1_confirm_ms:` header line.

```sh
uv run --python 3.12 spikes/gen_trials_1_6.py                                   # (re)write the committed set
uv run --python 3.12 spikes/gen_trials_1_6.py --set melody --bpm 90 --key       # one tempo, print the note list
uv run --python 3.12 spikes/gen_trials_1_6.py --set confirm --u1-confirm-ms 5500 # U1 confirmation files (L R Y X)
```

`--key` prints, per note, the index, name, MIDI, fingering, onset frame, release frame and frames pressed (or the attempt times of the U1 files): a reading aid, never a result. Other options: `--melody`, `--profile`, `--out-dir`, `--u1-button`, `--u1-buttons`, and the diagnostic options above. If the profile changes (Story 1.6 Part C may change `max_sustain_ms`), the drift test fails on purpose: regenerate and commit (the melody file headers list the profile values used).

### `listen_trial.py --mode melody` and `--mode sustain` (Story 1.6)

Both default to `--latency 0.13` (the Story 1.4 heard latency; measure it per session with `calibration-A.txt` and pass the median onset). Use `--backend sounddevice` (lossless); a run with a PortAudio overflow is reported as INVALID.

- `--mode melody`: reads the `NOTE` rows. Window per note from its heard onset (scheduled press + latency) + 40 ms (skips the stick start glide) to the next heard onset - 20 ms (the last note: + 0.5 s). **Audible** = the level within +-3 % of the expected pitch (with the game's +27 c) at least `--min-snr-db` (10) above the same band in the lead-in before the first note (levels per second, so windows of different lengths compare). **Pitch** = the strongest spectral peak within +-3 semitones of the expected pitch (a wrong neighbour is found, not forced onto the expected note); a peak at another pitch counts only if it is still there in the second half of the window (the decaying tail of the previous note is not a wrong note). **In tune** (owner decision 3) = the nearest note (A4 = 440 Hz) is the expected note and the cents are within +-50 c of the run's own median for that pitch class. **Onsets** from the tone envelope at the expected pitch (30 ms window): an attack is a rise of `--rise-db` (8) within +-120 ms of the heard onset (a repeated pitch only rises after a dip, so a merged repeat has 0 attacks = missed); each dip of `--dip-db` (8) lasting at least 30 ms and followed by such a rise back to within 10 dB of the level before the dip, inside the note, is an extra onset (a click over a held note or noise in the silence after a note stopped is not); a note that is not audible has 0 onsets. **Cut short** (the sustain-mode interruption rule): the tone at the expected pitch falls 20 dB below its level at the start of the note (or under `--min-snr-db` above the lead-in) for at least 150 ms, starting more than 100 ms before the note's scheduled release; the report gives how long it sounded (`sounded_ms`). Verdicts `ok`, `CUT SHORT`, `WRONG PITCH`, `NO SOUND`; summary: notes, audible, right pitch (`ok` + `CUT SHORT`), missed (no sound or no attack), cut, extra onsets, cents spread, per pitch class medians, and the list of every wrong/missed/cut/doubled note with its fingering.
- `--mode sustain`: reads the `TRY u1` rows. Per attempt: heard onset, the level of the tone at the expected pitch (20 ms window); **interrupted** (owner decision 1) = the tone falls below `--min-snr-db` above the noise or 20 dB below its own median level in the first second after the attack, for at least 150 ms, starting at least 100 ms before the scheduled release; the drop time after the heard onset is the **auto-stop time**. Wavering in numbers: the level (dB) and pitch (c) of every full second of the hold relative to the first second. Per (button, hold): interrupted count out of the attempts that sounded, the auto-stop times, then D_ok, T_stop and the candidate `floor100(min(D_ok, T_stop) - 500 ms)` for the rows given. The blank must stay silent and the 1 s control must be held, or the report says INVALID. A stop sooner than 2 s after the onset is the Story 1.6 every-~7th-note event, not the auto-stop: it is listed under `EARLY STOPS` with its attempt and position in the file (the warm-up is note 1) and left out of the counts, D_ok and T_stop; re-run those holds. `--combine` on sustain CSVs prints the same per-duration table over several files.

```sh
uv run spikes/listen_trial.py --port /dev/cu.usbmodem101 --backend sounddevice --mode melody --latency 0.13 spikes/samples/story-1-6/ode-to-joy-90bpm.txt
uv run spikes/listen_trial.py --port /dev/cu.usbmodem101 --backend sounddevice --mode sustain spikes/samples/story-1-6/u1-sustain-A-part1.txt
uv run spikes/listen_trial.py --combine spikes/recordings/u1-sustain-A-part*-??????.csv spikes/recordings/u1-confirm-*-??????.csv
```

### Story 1.6 session commands (Part B)

Do the "Hardware safety" checks (`docs/test-plan.md`; the GND check is skipped while the wiring is unchanged), open free play with D-pad left on the real controller, dismiss any prompt by hand, wait 2 s, keep the real controller within reach, stop with Ctrl+C in a real terminal. Do not detach or reattach a Joy-Con inside the session (the first injected input after a connect is ignored: after any connect, run `calibration-A.txt` before the next file; a run whose first note is silent while the rest is fine is re-run, not counted).

```sh
P=/dev/cu.usbmodem101
uv run spikes/listen_trial.py --selftest-tone --backend sounddevice --seconds 60                                 # once
uv run spikes/listen_trial.py --port $P --backend sounddevice spikes/samples/story-1-3/calibration-A.txt          # 4 s; A5 SNR well above 10 dB, onset ms -> L
uv run spikes/send_2wicc.py --dry-run spikes/samples/story-1-6/ode-to-joy-90bpm.txt                              # dry run
uv run spikes/send_2wicc.py --port $P --announce spikes/samples/story-1-6/ode-to-joy-60bpm.txt                   # by ear (65.5 s)
uv run spikes/listen_trial.py --port $P --backend sounddevice --mode melody --latency L spikes/samples/story-1-6/ode-to-joy-90bpm.txt   # x3 (44.2 s each)
uv run spikes/listen_trial.py --port $P --backend sounddevice --mode melody --latency L spikes/samples/story-1-6/ode-to-joy-120bpm.txt  # optional (33.5 s)
for k in 1 2 3 4 5; do uv run spikes/listen_trial.py --port $P --backend sounddevice --mode sustain --latency L spikes/samples/story-1-6/u1-sustain-A-part$k.txt; done   # 80.2 s each
uv run spikes/listen_trial.py --combine spikes/recordings/u1-sustain-A-part*-??????.csv                          # candidate from round 1
uv run --python 3.12 spikes/gen_trials_1_6.py --set confirm --u1-confirm-ms C                                     # C = the candidate hold
for b in L R Y X; do uv run spikes/listen_trial.py --port $P --backend sounddevice --mode sustain --latency L spikes/samples/story-1-6/u1-confirm-$b.txt; done
uv run spikes/listen_trial.py --combine spikes/recordings/u1-sustain-A-part*-??????.csv spikes/recordings/u1-confirm-*-??????.csv
```

`L` is the calibration onset in seconds (onset ms / 1000; default 0.13 s). Re-analyse any recording with the printed `--analyse REC.wav TRIAL.txt --offset S --scale 1.0 --mode melody|sustain --report-tag NAME` command. Recordings and reports stay in `spikes/recordings/` (git-ignored).

### `count_frames.py`: frame rate from a video (U10)

Counts the distinct images per second in a video of the screen recorded with more frames per second than the game (phone slow motion at 240 or 120 fps, or a 60 fps capture card). ffmpeg decodes every stored frame (grey, 160 px wide, no frame duplication or drop); a change is a mean absolute difference, over the moving part of the picture, above a threshold derived from the noise of the static parts (both printed). Output: capture fps (an assumption from the file, `--fps` to override), changes, distinct images per 1 s window (min, median, max), the run-length histogram of identical frames (60 fps game in a 240 fps video: runs of 4; 30 fps: 8; 20 fps: 12) and the game frame period from the median run. Videos stay in `spikes/recordings/` (git-ignored); never commit them or screenshots of the game.

```sh
uv run spikes/count_frames.py spikes/recordings/<clip>.mov
```
