# Adversarial review: ARCHITECTURE-SPINE.md (bonooru, 2026-11-19)

Reviewer lens: for each hole, two units one level down each follow every AD to the letter and still build incompatibly, or build something unsafe. Sources: `ARCHITECTURE-SPINE.md`, `app-stack-options.md`, `docs/link-protocol.md`, `docs/usb-controller-emulation.md`.

## Verdict

**Not build-safe yet. Conditional pass after tightening.** The paradigm is right: compile ahead, one clock on the device, a sans-IO link and a pure-C core. The ownership splits (core 0/core 1, serial worker/UI) are mostly sound. The gaps are in the link's **reliability semantics** (AD-6), the **abort/resync lifecycle** (AD-5, AD-7, AD-8) and **liveness on the bridge and controller** (AD-4, AD-11). The two firmware boards and the two app stacks can each meet every AD and still:

- leave an input held on the console for up to 500 ms (H-10) or for good (H-7)
- trip the link watchdog during normal playback (H-1, H-2)
- false-trip underrun on a long rest (H-6)
- replay part of an aborted piece at the start of the next one (H-5)
- fall into a NAK/retransmit storm after one UART bit error (H-3)

Nineteen holes follow. H-1 to H-7 are blockers: fix them before the first firmware story. H-8 to H-14 should go into the spine now. H-15 to H-19 can be deferred to `ocb_proto` or the vectors, but name them.

---

## Blockers

### H-1. The bridge's UART→CDC path can block the heartbeat (false watchdog trip during playback)

- **Units:** bridge main loop vs controller watchdog (AD-4, AD-5).
- **Build A (bridge):** single core, `tud_task()` plus a loop that copies the UART RX ring to `tud_cdc_write()`. If the CDC IN FIFO is full, it waits for room (`tud_cdc_write_flush` until `tud_cdc_write_available()`), which is the natural "don't lose bytes" choice. Nothing in AD-4 forbids it; AD-4 only says "UART→PC is a transparent byte copy".
- **Trigger:** case C. The host app freezes for 300 ms while PLAYING, or the app has closed the port. The controller keeps sending STATUS every 50 ms plus ACKs, the CDC FIFO (64–256 B) fills in under a second, and the bridge loop stalls. **The heartbeat stops and the controller aborts a healthy piece.** That is exactly what NFR-05 case C forbids. When no app has the port open, the controller gets no heartbeat at all, so it sits permanently tripped.
- **Proposed rule (AD-4, add):** *"No bridge code path may block. The UART→PC copy is best-effort: when the CDC IN FIFO is full, the bridge drops the newest bytes and increments `drop_tx`. HEARTBEAT is driven by a hardware alarm, not by loop progress, and its wire spacing is ≤ 20 ms + one max-frame time (≤ 27 ms) whatever the USB state (unmounted, suspended, DTR low, host not reading)."*

### H-2. The heartbeat queues behind a full UART TX ring during prefill (false trip)

- **Units:** bridge framer vs controller watchdog vs app streamer.
- **Build A (bridge):** a large UART TX ring (say 4 KB, for throughput, as the Windows small-write risk suggests). HEARTBEAT is injected "between frames", that is, appended to the ring at the next boundary.
- **Build B (app):** prefills 512 QUEUE_EVENTs in one write, as the link-protocol doc recommends ("batch frames per write").
- **Result:** a 4 KB backlog at 46 kB/s is about 87 ms. Add 20 ms of heartbeat phase and the controller sees a heartbeat gap of more than 100 ms, so it trips, clears the prefilled queue and goes to IDLE. Both builds are AD-compliant.
- **Proposed rule (AD-4, add):** *"HEARTBEAT has priority at the UART: the bridge holds at most one app frame committed to the UART (the frame in flight) plus `OCB_BRIDGE_TX_AHEAD_MAX` = 1 further frame. A due HEARTBEAT goes out at the next frame boundary, ahead of any queued app frame. The larger buffering stays in CDC (USB backpressure). Test: a 512-frame burst keeps every heartbeat gap ≤ 27 ms (host test of the framer with a fake UART)."*

### H-3. NAK semantics cause a retransmit storm and an ambiguous NAK body

- **Units:** controller decoder vs app link session (AD-6). Rust vs C codec.
- **Facts:** the window is 64, frames are pipelined, and AD-6 says "on a NAK … the app resends everything from `expected`".
  1. One bit error on frame *n* gives `NAK(CRC)`. The 63 frames behind it (*n+1…*) each get `NAK(SEQ, n)`. That is 64 NAKs, each of which tells the app to resend the window again. Each resend produces another wave of NAK(SEQ) for frames still in flight. The result is a livelock that keeps the link saturated while the queue drains toward underrun.
  2. A corrupted **HEARTBEAT** or line noise (for example, the bridge rebooting during a 1200-baud touch) fails CRC. The controller cannot know it was not an app frame, so it NAKs to the app, and the app resends for no reason.
  3. The link-protocol doc gives the NAK body as `seq, reason`. AD-6 writes `NAK(SEQ, expected)`. **Build A (C)** puts the *received* seq in byte 0. **Build B (Rust/Python)** reads byte 0 as `expected` and rewinds to the wrong frame. Both match "a NAK carries seq and reason".
  4. ACK timeout: AD-6 says "resend everything from `expected`", but on a timeout the app does not *know* `expected`. Build A resends from its oldest unACKed frame; build B waits for STATUS. They behave differently but both comply.
- **Proposed rule (AD-6, replace the NAK/timeout sentences):** *"NAK body = `reason:u8, expected:u8, got:u8`, and the frame header seq is unused. The controller sends at most **one** `NAK(SEQ|CRC)` per value of `expected`. It suppresses further transport NAKs until a frame with `seq == expected` is accepted or 20 ms pass. A CRC-failed frame is NAKed only if its decoded length is ≥ the minimum app frame and the link is in session. The app rewinds on a NAK only if `nak.expected` is ahead of its last rewind point; otherwise it ignores the NAK. On an ACK timeout (`ack_timeout_ms` = 50, a constant in `ocb_proto`) the app resends from its oldest unACKed frame. Stale frames with seq in `[expected−64, expected−1]` are answered with a cumulative `ACK(expected−1)`, never with NAK."*

### H-4. Semantic NAKs: does `expected` advance? (livelock vs silent loss)

- **Units:** controller mode machine vs app streamer.
- AD-6 defines only transport outcomes. AD-8 adds `NAK ORDER`, `NAK BUSY` and `NAK VERSION`; credit overrun gives `NAK FULL`; reserved flag bits set gives `BAD_STATE`.
- **Build A (controller):** a semantic NAK consumes the seq (`expected++`). The app's go-back-N sees a NAK and rewinds to `expected` (now past the event), so **the event is lost** while the piece keeps playing with a gap.
- **Build B (controller):** a semantic NAK does not advance. The app resends the same invalid frame forever (ORDER never fixes itself), which is a **livelock**, and every frame behind it, **including STOP**, is blocked (see H-7).
- **Proposed rule (AD-6, add):** *"NAKs are of two kinds. Transport NAKs (`CRC`, `SEQ`) never advance `expected`. Semantic NAKs advance `expected` and the frame has no effect; they are `ORDER`, `BUSY`, `BAD_STATE`, `UNKNOWN_TYPE`, `VERSION` and `ABORTED`. `FULL` is transport-like: it does not advance, and the app retries after the next STATUS. On any semantic NAK for a QUEUE_EVENT or START, the app abandons the piece and sends STOP. The response to `expected−1` is cached, so a duplicate gets the same ACK or NAK it got the first time."*

### H-5. A controller-initiated abort crosses in-flight frames, so the tail of an aborted piece is replayed

- **Units:** controller core (AD-5, AD-8) vs app streamer.
- **Trigger:** an underrun, link watchdog, USB suspend or END-applied abort takes the controller to IDLE with the queue cleared. The app has up to 64 QUEUE_EVENTs (and possibly a START) already in flight. The EVENT that reports the abort may also be lost (H-11).
- **AD table result:** in IDLE, `QUEUE_EVENT` means "enqueue (prefill)". The tail of the aborted piece becomes the prefill of the next session. The next START (even for a different piece) plays the old notes first, or gets `NAK ORDER` because the ORDER baseline is the old piece's last `t_us`. A late in-flight `START` would restart the remains of the piece by itself.
- **Proposed rule (AD-8, add):** *"Every transition to IDLE that the app did not request (END applied, underrun, link loss, USB suspend or unmount, core stall) sets the latch `aborted`. While `aborted` is set, `QUEUE_EVENT`, `START` and `SET_STATE` get `NAK ABORTED` (semantic, see H-4). Only `STOP` or `HELLO` clears the latch. Every entry to IDLE also resets the ORDER baseline (`last_t_us = 0`) and clears the queue. STATUS carries `mode` and an `abort_count:u8` (see H-11)."* (END-applied is included because frames after END, such as an early next-piece prefill, would otherwise land in the IDLE queue.)

### H-6. The "queue empty = underrun" rule false-trips on long rests with a time-window streamer

- **Units:** app streamer vs controller scheduler (AD-8, AD-15).
- **Build A (streamer):** "keep the queue ≥ 2 s ahead" is implemented as a *time window*: send events with `t_us ≤ playhead + 2 s`. This is a common and reasonable reading of the link-protocol doc.
- **Score:** a 3 s rest (fermata, or a pause between phrases in a text score).
- **Result:** the release event at the start of the rest is applied, the next event (+3 s) is not yet sent, and the queue is empty without END, so the controller declares an **underrun**. The piece aborts on a perfectly healthy link. **Build B** (credit-greedy streamer) never hits this. Both comply.
- Note also that with a credit-greedy streamer and a short piece, the queue never underruns, so dead-app detection during PLAYING depends on the piece having more than `queue_cap` events. That matches the intent ("queue plays out").
- **Proposed rule (AD-15, add to streamer):** *"The streamer is credit-greedy: it sends the next event as soon as credit allows, with no time-window throttling."* And **(AD-8, clarify):** *"Underrun is judged at apply time: when the scheduler applies an event without END and the queue is then empty, that is an underrun, and the controller applies NEUTRAL in the same loop iteration, not the event's state."* The second sentence makes the last pre-underrun state never linger.

### H-7. STOP can be blocked or lost, and END/IDLE does not force neutral

- **Units:** app link session vs controller mode machine (AD-6, AD-7, AD-8).
  1. STOP rides the go-back-N stream. If any earlier frame is NAKed with `FULL` (credit miscount, H-8) or semantically blocked (H-4 build B), STOP waits behind it. The doc's "after STOP: neutral within 100 ms" has no bound.
  2. AD-7: "Until a matching HELLO arrives it NAKs (VERSION) every other app frame." After a controller reboot (hardware watchdog, console USB re-enumeration), the app's STOP gets NAK VERSION. That is harmless *after* a reboot, but the same rule also applies when a mismatching HELLO arrives mid-session (H-9). The controller then keeps PLAYING and refuses STOP.
  3. The END transition reads "→ IDLE, `EVENT PLAYBACK_DONE`" and does not say neutral. **Build A** applies the END event's state (which the pipeline might have generated with a button still down; AD-15 does not require END to be neutral) and stays in IDLE with that state published. In IDLE there is no app timeout, so the **input is held indefinitely** while the bridge is healthy.
- **Proposed rule (AD-8, add):** *"`STOP` is a sync point like HELLO: it is accepted with **any** seq and in any version or abort state, sets `expected = seq+1`, publishes NEUTRAL, clears the queue and the `aborted` latch, and goes to IDLE. It is ACKed. Entering IDLE always publishes NEUTRAL. The END event's state must equal NEUTRAL with END set: the pipeline guarantees it, and the firmware NAKs `BAD_STATE` otherwise. The app sends STOP ahead of any unsent frames (it flushes its own send queue first)."*

---

## Tighten now

### H-8. Credit accounting is undefined (STATUS.free vs frames in flight)

- **Units:** app streamer vs controller queue.
- **Build A (app):** credit = `STATUS.free`. **Build B (app):** credit = `STATUS.free − QUEUE_EVENTs sent after STATUS.last_seq`. Build A over-sends by up to the window size, gets `NAK FULL` and goes back N each time (under H-3 this is a storm).
- Also, `last_seq` is ambiguous: is it the last accepted seq (`expected−1`) or the seq of the frame that triggered this STATUS?
- **Bandwidth:** "STATUS on every ACK" plus a separate ACK is about 24 B controller→PC per 19 B QUEUE_EVENT in the other direction, so during prefill **the C→H direction saturates first**. Core 1 is non-blocking, so it must drop something, but which frame is unspecified. If it drops ACKs, timeouts follow, then retransmits.
- **Proposed rule (AD-6, add):** *"`STATUS.last_seq` = `expected−1` (cumulative). ACK(seq) is cumulative. Credit = `STATUS.free − (QUEUE_EVENTs sent with seq after STATUS.last_seq)`. The controller coalesces: at most one STATUS per 10 ms (plus the 50 ms periodic), and at most one pending ACK, which is replaced by a newer one. The controller TX ring has priority EVENT > NAK > ACK > STATUS and drops the lowest priority on overflow, never blocking."*

### H-9. HELLO mid-session: mode effect and the pipelined-HELLO duplicate

- **Units:** controller mode machine vs app link session (AD-7).
- AD-7 does not say what HELLO does to mode or queue. **Build A (controller):** HELLO only resets `expected`, and PLAYING continues ("resume after app restart"). **Build B:** HELLO aborts. With build A, an app restarted mid-piece finds the controller PLAYING a piece it knows nothing about. Its SET_STATE gets NAK BUSY, and the UI shows "connected, idle" while the console plays.
- **Duplicates:** the app sends `HELLO(seq=s)` and pipelines frames `s+1…` before INFO arrives. If INFO is lost (C→H is unreliable) and the app resends HELLO, then `expected = s+1` again and frames `s+1…` are **accepted twice**, so QUEUE_EVENTs are duplicated (or NAK ORDER in the best case).
- **Mismatching HELLO** mid-session: does the controller drop back to the "unversioned" state while PLAYING? See H-7.2.
- **Proposed rule (AD-7, add):** *"HELLO has STOP semantics (H-7) plus a version check. The app sends nothing after HELLO until it gets INFO, and it resends HELLO on a 100 ms timeout. A HELLO with a mismatched `proto_ver` still performs the STOP part, then answers INFO and leaves the controller unversioned. On `NAK VERSION` mid-session, the app treats the session as lost (controller rebooted), reports it to the user and re-HELLOs. Before HELLO, the app writes one `0x00` to flush any partial frame left in the bridge (H-12)."*

### H-10. Core stalls: a seqlock reader can hang, a core-0 hang leaves the last report latched for ~500 ms, and watchdog ownership is ambiguous

- **Units:** controller platform core 0 vs core 1 (AD-11).
  1. **Seqlock:** if core 1 stalls *mid-write* (odd sequence), a textbook seqlock reader on core 0 spins forever. Core 0 is then hung inside report building, so it neither sends neutral nor runs TinyUSB, which is the opposite of what AD-11 intends.
  2. **Core 0 hang:** nothing checks core 0. The USB IN endpoint holds the last armed report, so the console sees it repeatedly (or NAKs, and the game keeps the last state) until the **~500 ms** hardware watchdog fires. That breaks NFR-05 (≤ 100 ms).
  3. "Fed only while both cores are alive": **Build A** feeds from core 0 after checking core 1's counter. **Build B** feeds from core 1 after checking core 0's counter. Each misses one case.
  4. **Recovery after a core 1 stall** of more than 20 ms: core 0 forced neutral, but core 1 comes back in PLAYING with several overdue events and fires them in one burst. A burst of notes is a wrong melody, and the late-event policy is undefined.
- **Proposed rule (AD-11, replace the last two sentences):** *"The seqlock reader retries at most 3 times, then uses NEUTRAL. Core 0 feeds the hardware watchdog iff core 1's liveness counter is less than 20 ms old. Core 1 checks core 0's report counter, and if it is more than 20 ms old, core 1 calls an immediate watchdog reboot, which detaches USB so the console releases all inputs. The hardware watchdog timeout is ≤ 100 ms. The scheduler aborts (EVENT `LATE`, same path as underrun) if the head event is overdue by more than `late_tol_us` = one report period; it never fires a backlog."*

### H-11. Controller→app frames are unreliable, so the app's view of the mode can diverge

- **Units:** controller EVENT emitter vs app session state.
- EVENT has no seq or ACK. A lost `PLAYBACK_DONE`, `UNDERRUN` or `USB_SUSPENDED` (UART bit error on the way back, or a bridge drop under H-1) leaves the app UI in "playing" while the controller is IDLE. With H-5 unfixed, the app then keeps streaming into the IDLE queue. `STATUS.flags` is deferred, but **mode** in STATUS is an architectural need, not an encoding detail.
- The header `seq` of C→H frames is unspecified. **Build A** sends 0. **Build B** sends a TX counter, and an app built against B logs "gaps" that build A never produces.
- **Proposed rule (AD-8, add):** *"STATUS always carries `mode:u8`, `abort_count:u8` and `last_event:u8`. The app derives mode only from STATUS and EVENT, and reconciles it every 50 ms. C→H header seq is a controller TX counter (wrapping u8), and the app logs gaps."*

### H-12. Bridge partial-frame carry-over and DTR semantics

- **Units:** bridge framer vs app serial adapter (Rust `serialport` vs Python `pyserial`).
  1. The app crashes mid-write, so the bridge keeps a partial frame (store-and-forward waits for `0x00`). The next session's first bytes are appended, the merged frame fails CRC, and HELLO is lost. That is only a delay, but it is avoidable.
  2. HEARTBEAT `flags.bit0 = DTR` is defined, but **no consumer is specified**. **Build A (controller)** ignores DTR. **Build B** treats DTR low as "app gone" and goes neutral in DIRECT, or also aborts PLAYING. DTR defaults differ by stack and OS: pyserial asserts DTR on open, `serialport` behaviour depends on the platform and builder. The same firmware therefore behaves differently with app option A and option B.
- **Proposed rule (AD-4/AD-16, add):** *"On DTR falling edge and on CDC unmount, the bridge discards its partial PC→UART frame. The app explicitly asserts DTR on open and writes a leading `0x00`. The controller uses `HEARTBEAT.flags.DTR` only in DIRECT mode: a falling edge counts as an app timeout. In PLAYING it is ignored, because underrun covers it."*

### H-13. Lost intermediate states between the scheduler and the report (short presses dropped)

- **Units:** controller core 1 scheduler vs P2 profile (`tick`, report period) vs app pipeline (min press/gap).
- The seqlock is "latest wins". If core 1 applies press then release (or two events with **equal** `t_us`, which AD-8's "must not decrease" allows) between two P2 report sends (the Pro Controller pushes `0x30` reports every ~8 ms, not every 1 ms poll), **the press is never reported**. The pipeline's min press and min gap live in the app TOML profile (AD-17) and know nothing about the firmware's report period. Snapping to the frame grid can collapse a gap to 0.
- **Proposed rule (AD-15 + AD-12, add):** *"Timeline `t_us` is strictly increasing. INFO carries `report_period_us:u16` from the active HID profile. Profile validation (AD-17) rejects `min_press_us` or `min_gap_us` below `2 × max(report_period_us, game_frame_us)`. The firmware NAKs `ORDER` on `t_us ≤ last_t_us`."*

### H-14. USB suspend and unmount: mode coverage and the FIFO blocking path

- **Units:** controller core 0 (TinyUSB callbacks) vs core 1 (mode machine).
  1. AD-8 covers suspend only in PLAYING. In **DIRECT**, a suspend leaves SET_STATE running into a suspended bus, and on resume the console sees the held state. In **IDLE** with a prefilled queue, a later START plays to a console that re-enumerated and may not have finished the P2 handshake again.
  2. AD-11 sends USB events "over the multicore FIFO" (8 × u32). `multicore_fifo_push_blocking` from core 0 while core 1 is stalled blocks core 0, which is H-10 again. Debug-build profile logging "as EVENT" (AD-12) also has to cross from core 0 to core 1 (only core 1 writes UART TX), and a log line does not fit the FIFO.
- **Proposed rule (AD-8/AD-11/AD-12):** *"USB suspend, unmount or bus reset in **any** mode means NEUTRAL, clear the queue, IDLE, set `aborted`, and EVENT. START and SET_STATE get `NAK NOT_READY` until the profile reports `ready` (P2 handshake complete) through the published flags. Core 0→core 1 traffic uses non-blocking latched atomic flags for USB state and a RAM SPSC ring for debug logs, dropping on full with a counter. Core 0 never calls a blocking FIFO push."*

---

## Defer to ocb_proto or the vectors, but name them now

### H-15. Receive check order and frame limits

- **Build A** checks seq before type and NAKs a `0x90` echo (a loopback with TX shorted to RX). **Build B** checks type first and ignores it.
- **Bridge vs controller:** the bridge drops runs longer than **300** bytes, but the controller RX buffer is sized independently. A 256-byte buffer overflows on legitimate 257–262-byte encoded frames.
- **Rule:** *"Receive order: COBS decode, then length ≥ 5 and `len` field consistent with the frame (else treat as CRC), then CRC, then type range (out-of-range or empty frames are silently counted), then seq, then semantics. `OCB_MAX_ENCODED_FRAME` (262) lives in `ocb_proto` and sizes every buffer on both boards. The vectors include a max-length frame, an empty frame (`00 00`), back-to-back frames and a leading delimiter."*

### H-16. Time domains: t0, now_us and u32 wrap

- `t0 = now + lead`, but which "now"? Decode time on core 1, or the timer at START frame arrival? STATUS `now_us:u32` is the low half of a 64-bit timer and wraps every 71.6 min. The UI countdown ("UI plus START lead_us") starts when the app *sends* START, which may be tens of ms before the controller acts (retransmits, USB buffering), so the play cursor drifts.
- **Rule:** *"t0 = 64-bit timer value when the START frame is accepted, plus `lead_us`. The controller answers START with `EVENT STARTED` carrying `t0` as u32 in the `now_us` domain. The app derives its cursor from STATUS.now_us minus t0 using wrapping u32 arithmetic. Pieces are limited to `t_us < 2^31` (35 min), and the pipeline enforces this."*

### H-17. Link-watchdog trip with a mismatched or old bridge is invisible, and the mode flaps

- The HEARTBEAT layout has no version gate. A controller built against a new HEARTBEAT body with an old bridge never sees a valid heartbeat. It accepts HELLO, but every SET_STATE goes to DIRECT and trips at once (flapping), and the user sees "connected" with a gamepad that does nothing. `INFO.bridge_ver` = zeros is the only clue.
- **Rule:** *"The HEARTBEAT body prefix (`bridge_fw_ver[3], flags`) is frozen like HELLO and INFO byte 0. While no valid heartbeat has arrived in the last 100 ms, the controller stays in IDLE and NAKs SET_STATE and START with `NO_LINK`. STATUS has a `link_ok` bit, and the app offers to reflash the bridge when `link_ok = 0` or `bridge_ver ≠ app version`."*

### H-18. Flasher vs serial worker: two owners of "the device" and ambiguous volumes

- AD-16 gives the port to the serial worker and the volumes to the flasher worker. The flasher's "verify controller: HELLO through the bridge" needs the port, so **build A** opens it from the flasher thread, which breaks AD-16, and a second owner races the worker's auto-reconnect. With two `RPI-RP2` volumes mounted (both boards in BOOTSEL), "find a volume with INFO_UF2.TXT, family RP2040" matches both, so the controller UF2 can land on the bridge board.
- **Rule:** *"The flasher worker never touches a serial port. Verification is a serial-worker command whose result is awaited by the service layer. The flasher snapshots volumes before the 1200-baud touch (or before asking the user to press BOOTSEL), accepts exactly one *new* `RPI-RP2` volume, and refuses if there are none or more than one."*

### H-19. Naming collision and doc drift

- "profile" means the HID profile (AD-12, `INFO.profile`) **and** the mapping profile (AD-17). Two app modules will both want `profile`.
- The INFO body in `docs/link-protocol.md` (`proto_ver, fw_ver, profile, queue_cap`) differs from AD-7's v1 body. The doc's STATUS `last_seq` and NAK `seq` conflict with AD-6 (H-3, H-8).
- **Rule:** *"Rename the wire field to `hid_profile`. AD-7 and the vectors are authoritative over `docs/link-protocol.md`. Update the doc tables in the first firmware story, and CI checks that the doc message table matches the `ocb_proto` constants."*

---

## Edge-case checklist (requested)

| Edge case | Status | Hole |
|---|---|---|
| seq wrap vs 64-frame window vs u8 | Window < 128, so it is unambiguous, but stale-frame handling and the NAK storm are undefined | H-3 |
| NAK on CRC error | NAK storm, ambiguous body, heartbeat CRC errors cause spurious NAKs | H-3 |
| STATUS/ACK credit accounting | In-flight frames not subtracted, last_seq ambiguous, C→H saturates | H-8 |
| HELLO mid-playback | Mode effect undefined, pipelined-HELLO duplicates | H-9, H-7 |
| START lead and t0 | "now" undefined, u32 wrap, UI cursor drift, START with an empty queue | H-16, H-6 |
| END flag + underrun | END state may be non-neutral, long rests false-trip, frames after END land in the IDLE queue | H-7, H-6, H-5 |
| Heartbeat during store-and-forward | Can queue behind the TX backlog; the bridge can block on CDC | H-2, H-1 |
| UART break vs unpowered bridge | Break is a one-shot fast path, and the 100 ms timeout is the real guard. **Add:** the controller enables the RX pull-up so a pulled wire idles high instead of floating into noise, and fits ~1 kΩ series resistors on both TX lines so a powered board cannot back-power an unpowered one through its RX pin (hardware note for `docs/`) | — |
| Core-1 stall detection | Seqlock hang, core-0 hang unguarded, overdue burst | H-10 |
| USB suspend | Only PLAYING covered, re-handshake not covered, FIFO can block | H-14 |

## What holds up

- The AD-3 type ranges, the frozen HELLO/INFO byte 0 in AD-7, the AD-9 deny-list in firmware, the single writer of current state and UART TX (AD-11), sans-IO plus shared vectors (AD-14), and "one owner of the port" (AD-16) are the right primitives. Every fix above is an added clause, not a redesign.
- The decision to have no app timeout in PLAYING is correct, provided H-6 and H-11 make underrun and mode reporting dependable.
