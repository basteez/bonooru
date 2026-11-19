# Review: rubric and reconciliation of ARCHITECTURE-SPINE.md

- Reviewed: `ARCHITECTURE-SPINE.md` and `app-stack-options.md` (2026-11-19)
- Checked against: `SPEC.md` and its companions (requirements, roadmap, decisions, architecture, architecture-diagrams, risk-register), `docs/link-protocol.md`, `docs/usb-controller-emulation.md`, `docs/score-format-and-mapping.md`, `docs/ocarina-input-mechanics.md`, `docs/test-plan.md`. I also read `.memlog.md` to see what the author decided but left out of the spine.
- Not reopened: ADR-001 to ADR-007 (except the language part of ADR-005) and the watchdog decision in `docs/link-protocol.md`.

## Verdict

**Strong spine, not ready to build from yet.** The firmware layering, frame-type ranges, heartbeat design, mode table, canonical state and threading model are good, and most AD rules can be enforced. Four problems remain:

1. The link-session contract has gaps that let the app and the firmware diverge: NAK semantics under go-back-N, credits versus the window, STATUS content, and HELLO during playback.
2. C5 is only half covered, and the miss is a rule-zero issue.
3. There is no Open Questions section. Owner decisions are either lost or hidden under Deferred, and NFR-07 is stated inconsistently.
4. Two spec constraints (C6, and R8 bridge identity) are quietly changed or dropped.

None of these needs a redesign. Each one is a rule to add or tighten.

Severity: **H** = two units will diverge, or a console or spec constraint is broken. **M** = a likely gap or rework. **L** = clarification.

---

## Part 1: Rubric

### 1.1 Does the spine fix the real divergence points, and miss none?

Fixed well:
- The frame-type ranges (AD-3) and ownership.
- The heartbeat encoding and the bridge's store-and-forward framer (AD-4).
- Version gating (AD-7).
- The mode table (AD-8).
- The canonical state and `NEUTRAL` (AD-10).
- One writer per resource across the two cores (AD-11).
- The profile interface (AD-12).
- One version source and shared vectors (AD-14).
- The timeline contract (AD-15).
- App threading (AD-16).
- Profile ownership (AD-17).

Divergence points that are still open:

| # | Sev | Gap | Why units diverge | Fix |
|---|---|---|---|---|
| D1 | **H** | **What a semantic NAK does to `seq`.** AD-6 defines `NAK(SEQ)` and `NAK(CRC)`. AD-7 and AD-8 add `NAK VERSION`, `BUSY` and `ORDER`, and link-protocol lists "queue full" and "bad state". Nothing says whether these NAKs advance `expected`. | If the controller does not advance `expected`, the app's go-back-N resends the rejected frame forever. If the controller does advance it but the app treats every NAK as "resend from expected", the next frame is a duplicate and the stream desyncs. Firmware and app will each pick one. | Add to AD-6: *transport NAKs* (`CRC`, `SEQ`) do not consume the seq, and the app resends from `expected`. *Semantic NAKs* (`VERSION`, `BUSY`, `ORDER`, `FULL`, `BAD_STATE`) do consume it (`expected++`). The app never resends after a semantic NAK; it surfaces an error instead. The NAK body carries both the rejected `seq` and `expected`. Also state which app frames carry seq and go through the window: all of them except HELLO, PING included. |
| D2 | **H** | **Credit flow versus the 64-frame window.** Link-protocol says the host "sends at most `free` events from the last STATUS". The spine adds a 64-frame unacked window but never says how the two combine. | `free` in a STATUS does not count `QUEUE_EVENT`s that are still in flight. One app counts them, another doesn't; the second one overruns the queue and hits a FULL NAK (see D1). | Rule: `credits = STATUS.free − (QUEUE_EVENTs sent with seq after STATUS.last_seq)`. Sending is limited by `min(credits, 64 − unacked)`. ACK and `last_seq` are cumulative. |
| D3 | **H** | **STATUS body, cadence and mode.** Link-protocol fixes STATUS as `free:u16, now_us:u32, last_seq, flags`, sent every 50 ms and on every ACK. The spine drops STATUS entirely and defers the `STATUS.flags` encoding. Only `.memlog` says what `now_us` means (playback clock, 0 when IDLE or during the lead). | The app needs the controller's mode (IDLE/DIRECT/PLAYING) to recover when an `EVENT` is lost. Controller→app frames are not reliable: an `EVENT PLAYBACK_DONE` or `UNDERRUN` can be destroyed by noise. If STATUS has no mode, the UI can get stuck showing "playing". The cadence matters to the app's liveness detection and to W.1. | New AD "STATUS": the body as in link-protocol, sent every 50 ms and after every ACK. Name the meaning of the flag bits now and defer only their numeric values: mode (2 bits), `last_event_code`, `link_ok`. `now_us` means what `.memlog` says it does. The app derives playback state from STATUS; `EVENT` is only a fast path. |
| D4 | **M** | **HELLO while PLAYING or DIRECT.** AD-6 says HELLO resets `expected`. It says nothing about the mode. | After case B (the app crashed during playback), a restarted app sends HELLO while the controller is still playing out the queue. Does playback continue, so the new app sees a mystery stream, or stop? The firmware and the app will assume different answers. | Rule: HELLO in any mode equals STOP (neutral, clear the queue, IDLE) and then INFO. |
| D5 | **M** | **What the controller holds after END.** AD-8 says that applying the END event takes the controller to IDLE. It does not say whether the END state is still applied, or whether IDLE forces neutral. | If the pipeline emits END on a note-on state, either the note stays held or it gets cut. | Two rules. The pipeline makes the END event's state `NEUTRAL | END`; the vectors and pipeline properties check this. Entering IDLE always publishes `NEUTRAL`, as defence in depth. |
| D6 | **M** | **How far ahead to stream (the 2 s rule) and the prefill policy.** The rule is missing (see R4). | The streamer's behaviour decides whether the underrun rule fires falsely under case C (an OS hiccup). | Add to AD-15 or a new streamer AD: prefill `min(piece, queue_cap)` events before `START`, then keep the queue at least 2 s ahead of `now_us`. |
| D7 | **M** | **The bridge's VID:PID** is not decided (AD-13 says only "VID:PID second"). `.memlog` has it as an open question; the spine has neither the question nor a value. | The firmware descriptor and the app's matcher must agree. The pico-sdk default `2E8A:000A` is shared by every pico-sdk CDC device, so a 1200-baud touch on a false match can reboot someone else's board. | Decide the value, or record an Open Question with an interim value. Require product string **and** VID:PID to match before any 1200-baud touch. See R8 in Part 2. |
| D8 | **L** | **A late `QUEUE_EVENT`.** While PLAYING, an event can arrive whose `t_us` is already in the past, for example after a long resend storm. | The firmware chooses between applying it late and dropping it. The app's progress and wrong-note reasoning assume one of the two. | Rule: a late event is applied immediately, and STATUS counts it in a `late` counter. Or it is treated as an underrun. Pick one. |
| D9 | **L** | **Format of the vector files.** AD-14 says "language-neutral hex" but gives no file grammar. | The C and app harnesses get written against different layouts. | Fix the format: one file per message, lines like `name: hex`, plus a `proto_version` header. Add a CI check that the vectors' `proto_version` equals `PROTO_VERSION` in both codebases. |
| D10 | **L** | **The HEARTBEAT DTR bit** (AD-4 bit0) has no consumer, and nothing tells the app to assert DTR. | One reading has the controller ignore it. Another uses "DTR dropped" as a fast way to detect that the app has gone in DIRECT mode. Some ports open with DTR low. | Either drop the bit, or rule: the app asserts DTR on open, and the controller treats a DTR edge from 1 to 0 in DIRECT as an app timeout. |
| D11 | **L** | **Leftover partial frame across sessions.** If the app dies in the middle of a write, the bridge holds a partial frame. | The first frame of the next session is corrupted. Recovery is fine, but the first HELLO can be lost. | Rule: the app writes a leading `0x00` on every connect. |

### 1.2 Can each AD rule be enforced, and does it prevent the divergence it names?

| AD | Enforceable? | Prevents its stated divergence? | Note |
|---|---|---|---|
| AD-1 | Yes: submodule pin, CI | Yes | The local toolchain is not pinned (CI only). That is acceptable. A `firmware/README` or devcontainer note would help. |
| AD-2 | Yes: the `host-tests` preset fails if core code includes pico-sdk | Yes | Good mechanical guard. |
| AD-3 | Yes: vectors | Yes | |
| AD-4 | Yes: framer unit tests (listed in the companion's test strategy) | Yes | The 300-byte cap is correct: the largest frame is 260 bytes before COBS. |
| AD-5 | Yes | Mostly | The settled watchdog is unchanged. A timer of exactly 100 ms leaves no margin for USB poll latency in A.3's "neutral within 100 ms". This is a measurement risk, not a reopening: a firmware constant ≤ 100 ms such as 80 ms fits the settled envelope. Recovery after a trip is also undefined. Add: the controller stays IDLE, keeps `expected`, and needs no new HELLO. |
| AD-6 | Yes: vectors and the sans-IO fake controller | **No, as written.** See D1 and D2. | |
| AD-7 | Yes | Partly | To make "HELLO body and INFO first byte never change" work, the **frame header layout, the COBS and CRC rules, and the type codes `0x01`/`0x81`** must be frozen too. Say so. |
| AD-8 | Yes: a table-driven firmware test | Mostly | See D4 and D5. Suspend and unmount are handled only in PLAYING. DIRECT mode with a held gamepad state during a dock or undock should also go neutral (see R13). `.memlog` allows `QUEUE_EVENT` only in IDLE and PLAYING, while the spine also allows prefill in DIRECT. That is fine, but check that it is intended. |
| AD-9 | Yes | It prevents what it names, **but it contradicts C6** (see Part 2). | |
| AD-10 | Yes: vectors check `NEUTRAL` | Yes | |
| AD-11 | Partly. "< 50 µs loop" and "fed only while both cores are alive" can only be checked on hardware. | Yes | Add a debug-build loop-time max reported through STATUS or EVENT, so the target can be checked. |
| AD-12 | Yes: link-time vtable | Yes | Say what "refuse" means for C7: ACK with an error, or ignore. It is profile-internal, so low priority. |
| AD-13 | Yes | Partly | VID:PID is undecided (D7). The R8 HELLO-probe fallback was dropped. The `pico_usb_reset` claim needs checking: Context7 confirms the `pico_usb_reset` library exists in pico-sdk, but not the exact macro name or that it owns `tud_cdc_line_coding_cb`. Add "verify in M4". C5 is only one-sided (Part 2). |
| AD-14 | Yes | Yes | Add the `proto_version` equality CI check (D9). |
| AD-15 | Yes: pipeline property tests | Yes | Missing pieces: END-state neutrality (D5), R2 song detection, and timeline export for M7 (Part 2). |
| AD-16 | Partly: review only | Yes | Acceptable. |
| AD-17 | Yes: one module | Yes | Add the precedence rule: a user copy overrides the bundled default with the same name and game version, and an unmigratable user copy is refused with a message. |
| AD-18 | Yes: crate split or import-linter | Yes | **States NFR-07 inconsistently:** it says only Option B needs NFR-07 relaxed, but the companion says Option A (`serialport`, MPL-2.0) needs it too. It also does not say *when* the spike verdict must land. M5's "done when" requires ADR-008 accepted, so the verdict gates M5. |

### 1.3 Could anything under Deferred let two units diverge?

| Deferred item | Safe? | Reason |
|---|---|---|
| Encodings of `STATUS.flags`, EVENT codes and NAK reasons | **Not fully** | Deferring the *numbers* to the first firmware story is safe because the vectors bind both sides. Deferring the *meaning* is not, because the app is built in parallel: which NAKs consume a seq (D1), which flags exist (D3), which events are terminal. Move the meaning into the spine and keep only the numbers deferred. |
| `app_timeout_ms` value | Safe | It reaches the app through INFO. |
| Baud rate | Safe | A build constant in `ocb_proto`, and both boards come from one tag. |
| UART DMA or IRQ | Safe | |
| Fingering weights and timing values | Safe | Profile data. |
| Text-score grammar | Safe | One owning module. |
| PICOBOOT fallback | Safe | |
| P1 and P3 profiles | Safe | |
| Single-board variant | Safe | |
| Signing, macOS arch, Linux packaging | Safe for divergence, **wrong section** | These are owner decisions that gate NFR-10 and A.5 (R5). They belong in Open Questions. |

### 1.4 Are every capability and every relevant NFR covered?

| Item | Covered? | Gap |
|---|---|---|
| CAP-1 | Yes | |
| CAP-2 | Yes | The FR-04 "report what doesn't fit" lives only in the test table. Name the range-fit report as a pipeline output in AD-15. |
| CAP-3 | Partly | **R2 is missing:** the compiler must detect official-song sequences and either use another fingering or warn. R2 scores 6. Add it to the pipeline in AD-15 and to the capability map. |
| CAP-4 | Partly | The 2 s rule, credits and STATUS are missing (D2, D3, D6). Tempo scale and countdown are covered. |
| CAP-5 | Yes | Plus the C5 issue (Part 2). |
| CAP-6 | Partly | The bridge's version cannot be seen without a wired controller running matching firmware. See FR-10 in Part 2. |
| CAP-7 | Yes | The flasher flow is in the Structural Seed. The "controller must be moved to the PC" step is implicit. |
| CAP-8 | Yes | |
| CAP-9 | Yes | |
| CAP-10 | Partly | Wizard content that the spec requires is not tied to anything: wiring rules 1–4 (risk register), the "Pro Controller Wired Communication" step with a screenshot (ADR-002), and "don't dock mid-song" (ADR-006). |
| CAP-11 | Yes | |
| CAP-12 | Yes | |
| NFR-01 | Yes (CI on 3 OS) | |
| NFR-02, NFR-03 | Implicit | Fine. |
| NFR-04 | Partly | Not bound to a mechanism for the M7 *measurement*: no timeline export for ground truth, and no debug GPIO for the cross-check (test plan, "Method"). |
| NFR-05 | Yes | See the AD-5 margin note. |
| NFR-06 | Not addressed | Add one line: the app makes no network connections (no update check, no telemetry). This stops a well-meaning "check for updates" feature. |
| NFR-07 | **Inconsistent** | See AD-18. No licence CI gate (`cargo-deny` or `pip-licenses`). Our own licence is not decided either: `.memlog` notes that no LICENSE file exists. |
| NFR-08 | Not bound | Ties to R2 and pipeline correctness. |
| NFR-09 | Yes | |
| NFR-10 | Yes (wizard, AD-18) | Blocked by R5 signing, which is an open question. |

### 1.5 Is every structural dimension decided, deferred or an open question?

| Dimension | Status in spine | Comment |
|---|---|---|
| Repo layout | Decided | Matches the README. |
| Firmware build | Decided (AD-1, AD-2) | |
| App build | Delegated to the companion per option | Fine. |
| CI | Decided (GitHub Actions, jobs sketched) | `.memlog` says no git remote exists yet. Note it as an assumption. Nothing says when CI arrives: the roadmap says "with the first code story". Add that. |
| Release | Decided (tag, assets, names) | Add the publishing channel explicitly (GitHub Releases), and the rule that app bundles embed the UF2s from the same workflow run. |
| Distribution | Partly | Linux format and macOS arch are left open. Fine, but they belong in Open Questions. |
| Environments | **Missing** | Dev (local toolchain), CI, and the HIL bench (test-plan environment table) are not named. One line will do. |
| Signing | Deferred | Should be an **Open Question** tied to R5, NFR-10 and A.5. `.memlog` has a recommendation (Apple notarytool, SignPath) that did not land. |
| Licence or compliance | **Missing** | NFR-07 gate, our own licence, attribution files, LGPL notice (Option B). |
| Observability | Decided (logging) | Add the W.1 max-gap statistic and the M7 timeline export (Part 2). |
| **Open Questions section** | **Missing entirely** | `.memlog` lists 7 questions (VID:PID, NFR-07 wording, signing, NFR-04 restatement, `app_timeout_ms`, own licence, macOS/Linux packaging). Only some reach the Deferred list, and none appear as questions. Add the section. |

---

## Part 2: Reconciliation with the inputs

### 2.1 Load-bearing items that didn't land, or that the spine contradicts

| # | Sev | Input | Status in spine | Fix |
|---|---|---|---|---|
| R1 | **H** | **C5 (rule zero).** "Unpowered board back-powered through UART". | AD-13 protects only the **bridge's** TX. The A.3 case is the reverse: the bridge is unplugged, the controller is still powered from the **Switch's VBUS**, and the controller's idle-high TX drives the unpowered bridge's GP1 through its clamp diodes. The Switch is then back-powering a board. | Make it symmetric. Each board keeps UART TX high-Z until RX sees idle-high, and goes back to high-Z on break or link loss. The controller's AD-5 trip also releases TX. Both boards use an internal **pull-down** on RX for idle detection, so a floating line from an unpowered peer reads low (break), not high. Update the C5 mitigation in the risk register to match. |
| R2 | **H** | **Rule zero: "firmware never touches the power path. Only GP0, GP1, GND."** | No firmware rule limits which pins may be driven. The M7 cross-check and spike 2 call for GPIO toggles. | New AD "Pin allow-list". Firmware configures only GP0 (UART TX), GP1 (UART RX) and the onboard LED (GP16). A debug toggle pin is debug-build only, is fixed in the board header, and is never on a header pin next to 5V or 3V3. Enforce it with one `pins.h` in `ocb_hw` and a grep in CI. |
| R3 | **H/M** | **C6:** "firmware whitelist in queue mode: ocarina inputs only, never Home / Capture / + / −". | AD-9 uses a **deny-list** of 4 buttons, which is weaker: B, ZL/ZR, the right stick and L3/R3 are allowed in PLAYING. The reasoning in `.memlog` is sound (a profile-independent list can't be weakened by a game patch), but this changes a console-risk mitigation without the owner. | Either get the owner to sign off and update C6 in the risk register, or combine the two: a firmware deny-list (fixed) **and** an allow-list derived from the loaded profile, sent in a session frame and intersected in firmware. The minimum is to record it as a deliberate deviation. |
| R4 | **M** | **2 s queue-ahead rule** (link-protocol, Flow control and Watchdog). The watchdog decision depends on it: "the queue is ≥ 2 s ahead". | Missing. | See D6. Put it in AD-15 or a streamer AD, because the settled watchdog design rests on it. |
| R5 | **M** | **STATUS cadence** (every 50 ms and on every ACK) and body. | Missing. | See D3. |
| R6 | **M** | **Credit flow** (ADR-003, link-protocol). | Only implied, by "credit flow" in the companion's test table. | See D2. |
| R7 | **M** | **FR-10 update offer** and the bridge version. | Bridge version only through `INFO.bridge_ver`, so it needs a wired controller running *matching* firmware: a mismatched controller still answers INFO, which is fine, but the controller must be present. In the setup wizard the bridge is flashed first and the controller isn't wired yet, so the bridge version can't be seen. `.memlog`'s "the app may always offer a reflash" did not land. | Choose one. (a) Encode the bridge version in a USB descriptor the app can read on all 3 OS, such as the interface string or a serial-number suffix (`bcdDevice` is not exposed by pyserial or serialport). (b) Allow one bridge→PC link-local frame, `BRIDGE_INFO`, sent on DTR assert, as an exception to AD-3. (c) Write the "always offer reflash" rule into the spine and the wizard. Also define the offer policy: proto mismatch means an update is required; same proto but `fw_ver` ≠ app version means an update is optional. |
| R8 | **M** | **R8 mitigation:** own VID:PID plus serial, and a HELLO probe as fallback. "pyserial usually shows only 'USB Serial Device' on Windows." | AD-13 matches the product string first, and that is the unreliable path on Windows. The HELLO-probe fallback is gone. VID:PID is undecided. | Match on VID:PID (own PID) plus a product-string check where the OS gives one. Use the HELLO probe only on candidates that already match VID:PID. Never send a 1200-baud touch to an unconfirmed port. |
| R9 | **M** | **R2:** the compiler detects official-song sequences. | Missing. | Add it to the AD-15 pipeline responsibilities and the CAP-3 row. |
| R10 | **M** | **NFR-07.** | AD-18 says only Option B needs it relaxed. The companion says Option A (MPL-2.0 `serialport`) does as well. The `binds:` list includes NFR-07 but no AD enforces it. | Fix the AD-18 wording. Add a licence gate to CI (`cargo-deny` licences, or `pip-licenses --fail-on`). Put the NFR-07 amendment wording from `.memlog` in Open Questions. |
| R11 | **M** | **R13 and ADR-006 docking:** "firmware re-enumerates cleanly on resume/reset; document 'don't dock mid-song'". | Only the PLAYING abort is covered (AD-8). Clean re-enumeration, DIRECT-mode behaviour and the UI warning are missing. (`.memlog` had "resume re-enumerates cleanly".) | Add to AD-8 or AD-12: unmount or suspend in **any** mode means neutral, IDLE and an EVENT. On mount, the profile restarts its handshake. The play screen shows "don't dock mid-song". |
| R12 | **L/M** | **Linux `dialout` and udisks** (ADR-005). | Covered in Conventions → Platform quirks. Fine. Could also say where the explanation appears (the wizard and the connection status) and that the app does not escalate privileges. | Low. |
| R13 | **L/M** | **W.1 needs "max gap between app frames at the controller"**, and W.2 counts false trips. | The firmware has no way to report this. | Add a debug statistic (max inter-frame gap and trip counters) to STATUS or to an `EVENT`/`STATS` frame. |
| R14 | **L/M** | **M7 method:** the ground truth is "the compiled timeline, exported by the host". | No export is defined. | Add a timeline export (CSV of `t_us`, pitch, fingering) to the advanced panel or settings, owned by core `pipeline` or `preview`. |
| R15 | **L** | **CAP-10 wizard content:** wiring rules 1–4 (risk register, "README + GUI setup wizard"), the Wired Communication setting with a screenshot (ADR-002), and the wiring photo. | Not listed. | One line in the capability map for CAP-10. |
| R16 | **L** | **1200-baud touch.** | Covered (AD-13 and AD-16). Verify the `pico_usb_reset` and `tud_cdc_line_coding_cb` claim in M4 (see AD-13 above). | |
| R17 | **L** | **NFR-06** (offline only). | Not stated. | One line: no network I/O in the app. |
| R18 | **L** | **The SPEC's Open Question on "controller core split"** | The spine decides it (AD-11). Good, but the SPEC still lists it as open. | Edit the SPEC (see 2.2). |
| R19 | **L** | **`queue target 512`** in link-protocol | The spine moves it to `INFO.queue_cap`, a build constant. Consistent, but say that the default is 512. | |

### 2.2 Facts the spine introduces that need edits to `docs/` or the spec

**`docs/link-protocol.md`**
1. **Frame-type ranges** `0x01–0x3F`, `0x40–0x7F` and `0x80–0xFF`, with the link-local rules (AD-3).
2. **New message: `HEARTBEAT` `0x40`.** Bridge→controller, with its own seq. Body `bridge_fw_ver[3], flags` (bit0 = DTR). Replace "exact encoding (e.g. an empty COBS frame) is an architecture decision" in the Watchdog section with this. The watchdog *decision* stays unchanged.
3. **Bridge description.** "Copies bytes both ways (DMA ring buffers, no parsing)" becomes store-and-forward per frame on PC→UART: it scans for delimiters, keeps a 300-byte cap, and drops overlong runs. The Layers table ("nobody parses it") needs the same nuance.
4. **INFO body changed:** `proto_ver u8, fw_ver[3], bridge_ver[3], profile u8, queue_cap u16, app_timeout_ms u16` (it was `proto_ver, fw_ver, profile, queue_cap`). Also update the sequence diagram's `INFO (fw, profile, queue_cap)`.
5. **New flag bit:** `state.flags` bit0 = END. The table says "flags reserved". Other bits must be 0.
6. **NAK reasons:** `CRC`, `SEQ`, `VERSION`, `BUSY`, `ORDER` (plus the existing "queue full" and "bad state"). The NAK body becomes `seq/expected, reason`, depending on how D1 is resolved.
7. **New EVENT code:** `PLAYBACK_DONE`. Existing: underrun, watchdog, USB suspended.
8. **Reliability:** go-back-N, a 64-frame window, duplicate re-ACK, and HELLO resetting `expected`. None of this is in the doc today.
9. **Controller modes** IDLE, DIRECT and PLAYING, the mode × frame table, and `NAK ORDER` for a decreasing `t_us`.
10. **PING policy:** "the app sends PING while connected" becomes "in DIRECT, when idle ≥ `app_timeout_ms/4`". Compatible, but tighter.
11. **Queue-mode safety mask** (C6 as implemented).
12. **Queue capacity:** "queue target 512" becomes `INFO.queue_cap`.

**`docs/usb-controller-emulation.md`**
13. The P1 button order becomes the **canonical link state** for every profile (AD-10). Add a note.
14. The profile interface (AD-12), `INFO.profile` ids, and the debug-build EVENT logging of subcommands.

**`_bmad-output/specs/spec-ocarina-bot/`**
15. **SPEC Constraints and ADR-005 / architecture.md Stack:** "App: Python 3.12+, PyInstaller" becomes conditional, per AD-18. **ADR-008** must be rewritten: the options are now Rust+egui or PySide6. CustomTkinter, the fallback recorded in decisions.md and the SPEC Open Questions, is dropped without a record.
16. **NFR-07** needs amending whichever option wins (MPL-2.0 or LGPL). Today the spec says "possible LGPL relaxation" only.
17. **architecture.md:** "Core 0: TinyUSB…; core 1: UART + scheduler — to decide in implementation" is now decided (AD-11). "Bridge stays a dumb byte pipe" needs the store-and-forward and heartbeat nuance. The SPEC's Open Question on the core split should be closed.
18. **risk-register.md:** C5's mitigation should become symmetric (R1). C6 changes from whitelist to deny-list, or to the combined form (R3). R8's mitigation should list the identification order the spine picks.
19. **requirements.md** "NFR-05 vs watchdog" note: the values stay. Add a pointer to HEARTBEAT `0x40` as the chosen encoding.
20. **roadmap.md M4:** "transparent CDC↔UART" becomes "transparent at the byte level, framed store-and-forward on PC→UART".

**`docs/test-plan.md`**
21. Add test rows for the new behaviour: the go-back-N and NAK matrix, the mode table, END and underrun, HELLO during PLAYING, symmetric C5 high-Z (measure the bridge's GP1 voltage with the bridge unpowered and the controller on the Switch), and the safety mask. Firmware unit tests cover part of this. A.3 should also check that the controller's TX is released.

---

## Top findings (priority order)

1. **H: The link-session contract is incomplete (D1–D4).** Define what semantic NAKs do to `seq`, the credit formula combined with the window, the STATUS body, cadence and mode bits, and HELLO forcing STOP. Put these meanings in the spine, and leave only their numeric encodings under Deferred.
2. **H: C5 covers only one direction, and a pin allow-list is missing (R1, R2).** The controller must release TX on break or link loss, both boards need an RX pull-down for idle detection, and a firmware pin allow-list must be added (rule zero).
3. **H: There is no Open Questions section, and NFR-07 is stated inconsistently.** Restore the `.memlog` questions (VID:PID, NFR-07 wording for *both* options, signing/R5, NFR-04/R3, own licence, packaging, spike deadline before M5), and add a licence CI gate.
4. **H/M: The spine quietly changes C6** from a whitelist to a deny-list. Get the owner's sign-off and update the risk register, or add a profile-derived allow-list.
5. **M: Bridge identity and FR-10 (D7, R7, R8).** Decide the VID:PID. Match VID:PID first on Windows, restore the HELLO-probe fallback, and touch at 1200 baud only on a confirmed port. Make the bridge's version readable without a controller, or write "always offer reflash" into the spine.

Also worth fixing: the 2 s rule and prefill policy (D6), END-state neutrality (D5), R2 song detection in the pipeline, docking in any mode, the W.1 statistic and M7 timeline export, NFR-06 "no network", and the environments line.
