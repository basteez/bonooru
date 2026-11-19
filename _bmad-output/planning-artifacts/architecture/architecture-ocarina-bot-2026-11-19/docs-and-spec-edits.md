# Edits needed in docs/ and the spec after ARCHITECTURE-SPINE.md

None of these edits has been applied. The architecture run did not modify docs/ or the spec. Each item names the AD that causes it.

## docs/link-protocol.md

1. **Layers table and bridge bullets.** The bridge is no longer a pure byte copier. PC→UART is store-and-forward per COBS frame: the bridge scans for delimiters but never decodes, keeps one frame in flight plus one queued, drops overlong runs, and discards a partial frame when DTR falls. UART→PC is best-effort and drops bytes when full. (AD-4)
2. **Watchdog section.** Replace "the exact encoding (e.g. an empty COBS frame) is an architecture decision" with: the `HEARTBEAT` type is `0x40`, bridge→controller, with body `bridge_fw_ver[3], flags` (bit0 = DTR). It is driven by a hardware alarm every 20 ms, with wire spacing ≤ 27 ms. The link timeout is ≤ 100 ms (default 80). A DTR falling edge counts as an app timeout in DIRECT mode. The decision itself is unchanged. (AD-4, AD-5)
3. **Type ranges.** Add the type ranges `0x01–0x3F` / `0x40–0x7F` / `0x80–0xFF` and the receive-check order. (AD-3)
4. **INFO body.** It becomes `proto_ver, fw_ver[3], bridge_ver[3], hid_profile, queue_cap u16, app_timeout_ms u16, report_period_us u16`, and its byte 0 is frozen. Update the sequence diagram label to match. (AD-7)
5. **Controller state flags.** Bit0 = END. The other bits are reserved and must be 0. (AD-8)
6. **NAK body.** It becomes `reason, expected, got`. The reasons fall into two groups: transport (`CRC`, `SEQ`, `FULL`) and semantic (`VERSION`, `BUSY`, `ORDER`, `BAD_STATE`, `UNKNOWN_TYPE`, `NO_LINK`, `NOT_READY`, `ABORTED`). (AD-6)
7. **EVENT codes.** Add `PLAYBACK_DONE`, `LINK_LOST`, `LATE`, `UNDERRUN` and `USB_SUSPENDED`. (AD-5, AD-8)
8. **STATUS body and cadence.** The body is `free, now_us, last_seq, mode, abort_count, last_event, link_ok`, plus counters in debug builds. STATUS is sent every 50 ms, plus after ACKs coalesced to at most one per 10 ms. Add the TX priority order. (AD-19)
9. **New section: reliability.** Covers go-back-N, the 64-frame window, cumulative ACK, the 50 ms ACK timeout, `credit = free − in-flight QUEUE_EVENTs`, and HELLO/STOP as sync points accepted with any seq. (AD-6)
10. **New section: controller modes.** The IDLE/DIRECT/PLAYING table, the abort latch, strictly increasing `t_us` < 2^31, `END = NEUTRAL|END`, underrun judged at apply time, and the LATE rule. (AD-8)
11. **Flow control.** "Keeps the queue ≥ 2 s ahead" becomes "prefill, then send greedily whenever there is credit". "Queue target 512" becomes `INFO.queue_cap` (default 512). The PING rule becomes "DIRECT mode only, when idle ≥ `app_timeout_ms/4`". (AD-7, AD-15)
12. **Precedence note.** The spine ADs and the vectors in `firmware/common/proto/vectors/` take precedence over this doc. (AD-14)

## docs/usb-controller-emulation.md

13. **Canonical link state.** The P1 button order is the canonical link state for every profile. (AD-10)
14. **Profile interface.** Document the profile interface, including `ready()`, the handshake restart after a bus reset, and the `hid_profile` ids. Add `report_period_us`, measured on the device: this also answers the polling-rate HYPOTHESIS. (AD-12, AD-7)

## docs/test-plan.md

15. **New firmware host-test rows.** Go-back-N and the NAK matrix, the mode table and abort latch, END and underrun, HELLO/STOP during PLAYING, the queue-mode allow-list, a heartbeat gap ≤ 27 ms during a 512-frame burst, and the bridge framer.
16. **New HIL rows.** Measure the voltage at the bridge's GP1 with the bridge unpowered and the controller on the Switch (expected: TX released). Also check the core-stall reboot. (AD-20, AD-11)
17. **A.3.** Also check that the controller releases TX. **W.1.** Read the max-gap counter from the debug STATUS. **M7 method.** The ground truth is the CSV timeline export. (AD-19, AD-15)

## _bmad-output/specs/spec-ocarina-bot/

18. **SPEC Constraints, ADR-005 and architecture.md Stack.** The app language becomes conditional per ADR-008 (Rust + egui if the spike passes, otherwise Python 3.12 + PySide6). In decisions.md, mark ADR-005's language part as reopened by ADR-008, and rewrite ADR-008: CustomTkinter is dropped, per the research report. (AD-18)
19. **SPEC Open Questions.** Close "controller core split" (decided, AD-11). Replace the ADR-008 question with the conditional decision and spike deadline (Q7). Add Q1 (VID:PID) and Q5 (project licence).
20. **architecture.md.** Rewrite the "Core 0 / core 1 — to decide" line per AD-11. Change "bridge stays a dumb byte pipe" to "frame-aware pipe plus heartbeat" (AD-4). Add the safety mask and the abort latch to the controller internals diagram.
21. **risk-register.md.**
    - C5's mitigation becomes symmetric: an RX pull-down, and both boards release TX (AD-20).
    - C6 now names the allow-list: L, R, A, X, Y, hat, left stick (AD-9).
    - R8's mitigation now reads: own VID:PID first, product string where available, HELLO probe on VID:PID candidates only (AD-13).
    - R5's text: "right-click → Open" no longer works on macOS 15+.
    - Add rule zero: firmware drives only GP0, GP1 and GP16 (AD-20).
22. **requirements.md.** Update the "NFR-07 vs ADR-008" note: both stack options need an amendment (Q2). In the "NFR-05 vs watchdog" note, add a pointer to HEARTBEAT `0x40`.
23. **roadmap.md.**
    - M4: "transparent CDC↔UART" becomes "frame-aware store-and-forward PC→UART, best-effort UART→PC, heartbeat by hardware alarm".
    - M5: ADR-008 is decided by the spike verdict.
    - M3: add the abort latch and STATUS mode.

## Repo files outside docs/ (for the owner or the first stories)

24. `README.md` and `app/README.md` say "Desktop app (Python)". Make them stack-neutral until ADR-008 is decided.
25. `firmware/common/README.md`: add `proto/vectors/` and `hw/pins.h`.
