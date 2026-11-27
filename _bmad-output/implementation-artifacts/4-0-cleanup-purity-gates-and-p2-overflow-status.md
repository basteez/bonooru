---
baseline_commit: f3ffb63369055b640edb38bc272ec795ed5c40e7
---

# Story 4.0: Cleanup: purity gates and P2 overflow status

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As the firmware developer,
I want the purity gates to refuse the forbidden identifiers they still miss, and the debug STATUS to report the P2 reply-queue overflows,
so that the pure C99 libraries cannot quietly grow a libc, assembly or register dependency, and a dropped P2 reply is visible on the bench instead of invisible.

Origin: Epic 3 retrospective action item 4 (`epic-3-retro-2026-11-26.md` §5, §6: "a cleanup story before Epic 4 ... host tests only", owner decision 2026-11-26), which carries Epic 2 action items E2-A4 (issue #20) and E2-A5 (issue #25). Not in `epics.md`: this is a cleanup story ahead of Story 4.1. The PR closes #20 and #25.

**Owner constraint: host tests only.** No bench, no Switch, no flashing. Every check is host-side: ctest (`host-tests` preset), the Python self-tests run by ctest, `cargo test` / `clippy` / `fmt` in `app/`, and the ELF gates on locally built firmware images (if the ARM toolchain is installed; CI builds them anyway).

## Acceptance Criteria

1. **New identifier rules (#20).** The shared deny list used by all five purity gates (`check-proto-pure.sh`, `check-core-pure.sh`, `check-link-pure.sh`, `check-p2-pure.sh`, `check-bridge-pure.sh`) also refuses, word-bounded as in Story 2.4 D20: calls to `exit`, `_exit`, `_Exit`, `quick_exit` and `abort`; the names `fopen`, `strdup`, `strndup`, `alloca` and `__builtin_alloca`; inline assembly (`asm`, `__asm`, `__asm__`); and raw peripheral addresses (a hex literal cast straight to a pointer, or an RP2040 peripheral-window literal `0x40xxxxxx`, `0x50xxxxxx`, `0xd0xxxxxx`, `0xe0xxxxxx`). Names that only contain these words (`aborted`, `ocb_exit_code`, the word "abort" in a comment, `0x80000000`, `0xFFFFFFFFu`, `0xEDB88320u`) stay legal.
2. **`section(` only in `ramfunc.h` (#20).** A `section(` or `__section__(` attribute is refused in every scanned source file except a file named `ramfunc.h` (the hook of `OCB_CORE_RAMFUNC`, Story 2.4 D12). The comment words "section attribute" or `section "Controller modes"` stay legal.
3. **One list, all five gates.** The deny list lives in one place that all five gates use, and no gate keeps a private copy that could drift. `check-proto-pure.sh` gains the identifier rules too (today it checks includes only). The real trees (`common/proto`, `controller/core`, `controller/link`, `controller/profile_p2`, `bridge/core`) still pass every gate, and the CI steps call the gates unchanged.
4. **Fixtures and red proof (#20).** Each new rule has at least one bad fixture in `core_pure_fixtures/bad/`, and every other gate's fixtures have at least one new-rule bad fixture that proves the gate uses the shared list. The good fixtures contain the legal look-alikes of AC1 and AC2 (a `ramfunc.h` with a section attribute included). Before the rules are added, the self-tests are run with only the new fixtures in place and report each new bad fixture as not reported (red); after the change they pass (green). Both runs are in the Debug Log.
5. **P2 reply-queue overflows in the debug STATUS (#25).** A debug build's STATUS carries a new v6 block of 2 bytes right after the v5 block: `p2_queue_full u16`, the P2 replies dropped because the 4-entry reply queue was full, counted since boot, saturating at 65535. Core 0 posts the profile's counter through the USB mailbox. Release builds are unchanged: no debug tail, and the release TX path does not grow. No wire value changes: no `PROTO_VERSION` bump, the conformance vectors are unchanged.
6. **Sizes still fit.** The debug tail is 72 bytes with an empty window and at most 104; the largest debug STATUS body is 115 bytes, 122 bytes on the wire, and still fits the controller's 128-byte UART TX ring in one frame, never split. The compile-time checks in `tx_debug.c` and `platform/link_runner.c` hold.
7. **Decoders (#25).** `firmware/tools/link_probe.py` decodes the v5 block (it does not today) and the v6 block, and shows both on the STATUS line. `p2check` prints `p2_queue_full` at the end of its run, or says the build does not report it (a tail without v6). `ocb-dev`'s `DebugCounters` (`app/tools/ocb-dev/src/tap.rs`) parses `p2_queue_full` as `Option<u16>`, `None` for a shorter tail, and `ocb-sim`'s fake controller fills the v6 block of a tail long enough to hold it. `ocb-core` keeps STATUS `extra` opaque and does not change.
8. **Docs and test plan.** `docs/link-protocol.md` documents the v6 block and the new sizes (checked by `docs_constants_check`, F.22). `firmware/README.md` and `firmware/controller/README.md` describe the gate rules and v6 where they describe the gates and the debug STATUS. `docs/test-plan.md` updates F.13, F.16, F.18, F.19, F.22, F.30, F.31, F.32, F.40, the C rows touched, and adds a row for the proto purity gate (it has none today).
9. **Everything green on the host.** `ctest --preset host-tests` passes in full; `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass in `app/`; `check-pins.sh` and `check-proto-version.sh` still pass; when the ARM toolchain is installed, the fw-debug, fw-debug-o3 and fw-release builds compile and their ELF gates (`check-release-nolog.sh`, `check-stack.sh`, `check-ram-hot-path.sh`, `check-barrier.sh`) pass.

### AC → measurement (retro action E3-A1)

| AC | Measured by (host) | Field / result |
|---|---|---|
| 1, 2 | `core_pure_check` (`core_pure_selftest.sh`) | every new bad fixture rc 1 with output; good fixture rc 0 |
| 3 | `proto_pure_check`, `core_pure_check`, `link_pure_check`, `p2_pure_check`, `bridge_pure_check` + the five CI steps | real tree rc 0; the "shared list" check of each self-test |
| 4 | the same self-tests, run before and after (Debug Log) | red run lists each new fixture as `FAIL: bad fixture not reported`; green run ok |
| 5 | `test_link_tx` (F.16) new v6 test; `link_probe_selftest` (F.32 part) via `sim_set_p2_queue_full` | `extra[70..71]` = posted value, 65535 when posted > 65535; release-config STATUS has no tail |
| 6 | `test_link_tx` largest-STATUS test; `test_link_dbglog` full window; `_Static_assert`s at build | first frame 122 bytes; cap 121 writes nothing; extra 104 with 16 records |
| 7 | `link_probe_selftest` (decoder by hand, p2check output); `cargo test -p ocb-dev` (tap parse) | v5 and v6 dicts; p2check note line; `p2_queue_full` `Some`/`None` |
| 8 | `docs_constants_check` (F.22) | v4 / v5 / v6 sizes and offsets equal `ocb_link/config.h` |
| 9 | full ctest, cargo, ELF gates | all pass |

## Tasks / Subtasks

- [x] **Task 1: Red fixtures first (AC: 4)**
  - [x] 1.1 Add the new bad fixtures (list in Dev Notes, "Fixtures") under `firmware/tests/core_pure_fixtures/bad/`, one new-rule fixture under each of `proto_pure_fixtures/bad/`, `link_pure_fixtures/bad/`, `p2_pure_fixtures/bad/`, `bridge_pure_fixtures/bad/`, and the legal look-alikes in each `good/` (core `good/` also gets `ramfunc.h` with a section attribute).
  - [x] 1.2 Run the five self-tests on the unchanged gates; copy into the Debug Log the `FAIL: bad fixture not reported` line of every new fixture (the red proof). The good fixtures must still pass on the old gates (they are legal on both).
- [x] **Task 2: One shared rule file (AC: 1, 2, 3)**
  - [x] 2.1 Create `firmware/tools/pure-rules.sh` (sourced, not executed; no `set -e` of its own, no output). It defines `nb`, `banned_word_re` (the current list plus the AC1 words), `section_re`, `raw_addr_re` and one function, e.g. `pure_ident_hits FILE`, that prints the `grep -n` hits of all identifier rules for one file and applies the `ramfunc.h` exception to `section_re` by basename.
  - [x] 2.2 Make `check-core-pure.sh`, `check-link-pure.sh`, `check-p2-pure.sh` and `check-bridge-pure.sh` source it (`. "${script_dir}/pure-rules.sh"`) and drop their own `nb` / `banned_word_re` lines. Each keeps its own include allow-list, banned includes and CMake rule.
  - [x] 2.3 Make `check-proto-pure.sh` source it and add the identifier rules to its scan (it still scans `*.c` and `*.h` only). Update its header comment and its failure message.
  - [x] 2.4 Update the header comments of all five gates (rule (b) lists the new families; "identical to check-core-pure.sh" becomes "from tools/pure-rules.sh") and the failure messages (`no heap, I/O, clock, sleep, multicore, process-exit, assembly, section or register-address identifiers`, or similar).
- [x] **Task 3: Self-tests (AC: 3, 4)**
  - [x] 3.1 Replace the drift guards of `link_pure_selftest.sh`, `p2_pure_selftest.sh` and `bridge_pure_selftest.sh` (which compare `banned_word_re=` lines with the core gate) with a check that applies to all five self-tests: the gate sources `pure-rules.sh` and defines no `banned_word_re=` of its own. Red proof: a temporary copy of a gate with a private `banned_word_re=` line fails the check (in the Debug Log).
  - [x] 3.2 Run the five self-tests green; record the bad-fixture counts in the Debug Log; update the test names / comments in `firmware/tests/CMakeLists.txt` where they say "the banned list equals the core gate's".
- [x] **Task 4: Firmware, the v6 block (AC: 5, 6)**
  - [x] 4.1 `ocb_link/config.h`: describe v6 after v5 (`0 p2_queue_full u16`), `#define OCB_LINK_DEBUG_V6_LEN 2`, `OCB_LINK_DEBUG_TAIL_MAX_LEN` 104; update the "Debug tail: ... largest body" comment (72 / 104 / 115).
  - [x] 4.2 `ocb_link/ready.h` + `link/ready.c`: `volatile uint32_t p2_queue_full` in `ocb_usb_mailbox` (core 0 only, one aligned store, a value not a counter of events core 1 reacts to); zeroed by `ocb_usb_mailbox_init`; setter `ocb_usb_mailbox_set_p2_queue_full(ocb_usb_mailbox *m, uint32_t n)`.
  - [x] 4.3 `link/tx_debug.c`: `put_v6()` (saturating `put_u16` of `link->mailbox->p2_queue_full`), appended after `put_v5`; extend the `ocb_link_tx_dbg_fits` static assert with `OCB_LINK_DEBUG_V6_LEN`; update the file comment. `tx.c` (release path) is not touched.
  - [x] 4.4 `controller/platform/usb_p2.c`: post `s_p2.cnt.queue_full` from `post_self_timing()` when it changed (a `s_qfull_posted` copy, as `s_spread_posted`), update the file comment. Nothing else in the glue changes.
  - [x] 4.5 Confirm `platform/link_runner.c`'s `_Static_assert` (largest STATUS vs `OCB_UART_LINK_TX_RING`) still holds with 122 bytes; no edit expected beyond its comment.
- [x] **Task 5: Firmware host tests (AC: 5, 6)**
  - [x] 5.1 `tests/test_link_tx.c`: the 70-byte expectations become 72 (`test_v5_block`, the v1/v2/v4/v5 checks); a new `test_v6_block`: mailbox value 0 → `extra[70..71]` 0; 3 → 3; 70000 → 65535; a release-config link (`debug_counters` false) sends an 11-byte STATUS. The largest-STATUS test expects 122 bytes and refuses a cap of 121. Red proof: each new check fails on the code before Task 4 (Debug Log).
  - [x] 5.2 `tests/test_link_dbglog.c`: the extra lengths `21 + tail + 41 + 5` and 102 become `+ 2` and 104; update the comments. `tests/test_link_ready.c`: `ocb_usb_mailbox_init` zeroes `p2_queue_full`.
  - [x] 5.3 `tests/link_sim_shim.c`: `sim_set_p2_queue_full(uint32_t n)` (the core 0 post), header comment updated.
  - [x] 5.4 Update comments that state old sizes (`test_link_fuzz.c` "67-byte", `link_sim_shim.c`) only where they would now be wrong.
- [x] **Task 6: `link_probe.py` and `p2check` (AC: 7)**
  - [x] 6.1 `debug_counters()`: after v4, `d["v5"] = {"loss_age_us": age10 * 10, "loss_kind": k, "in_age_ms": a}` when the tail holds the 5 bytes, then `d["v6"] = {"p2_queue_full": n}` when it holds 2 more; constants `DEBUG_V5_LEN = 5`, `DEBUG_V6_LEN = 2`, `LOSS_KINDS = {0: "none", 1: "timeout", 2: "break"}`; `v5_str()` / `v6_str()`; the STATUS line (`frame_str`) appends `| v5: ...` and `| v6: p2_queue_full=N`. Update the module docstring.
  - [x] 6.2 `cmd_p2check`: at the end, a `c.note(...)`: `P2 reply-queue overflows (p2_queue_full): N` from the last STATUS, or `p2_queue_full: not reported (a debug build before Story 4.0, or a release build)` when there is no v6 block. A note, not a check: p2check's PASS / FAIL verdicts do not change (issue #25 asks to print it).
  - [x] 6.3 `tests/test_link_probe.py`: `test_debug_v5_v6` by hand (bytes after an empty window and after a 3-record window; a 70-byte tail has v5 and no v6; a 69-byte tail neither); `test_p2check` asserts the `p2_queue_full: 0` note; a new p2check run where the scripted console makes the sim post 3 (`sim_set_p2_queue_full`) shows `p2_queue_full): 3`; a unit test of the "not reported" text on a v5-only STATUS dict. Red proof per new output branch (value, not reported) in the Debug Log (retro action E3-A2).
- [x] **Task 7: Rust side (AC: 7)**
  - [x] 7.1 `app/tools/ocb-dev/src/tap.rs`: `pub p2_queue_full: Option<u16>` ("v6 (Story 4.0): P2 replies dropped on a full reply queue, since boot"), parsed at `v5 + 5` when `extra.len() >= v5 + 5 + 2`. `line()` is unchanged (it prints v1 / v2 only). Unit tests: a 72-byte tail gives `Some(n)`, a 70-byte tail `None`, a 3-record window shifts the offset.
  - [x] 7.2 `app/sim/src/controller.rs` `debug_tail()`: when the tail is at least 72 bytes, write a `p2_queue_full` value (default 0, settable with `set_p2_queue_full(u16)`) at 70..72; a test in `app/sim/tests/` reads it back through a STATUS. `app/tools/ocb-dev/tests/common/mod.rs` `debug_tail()` grows to 72 bytes ("a Story 4.0 debug tail"); keep one test that runs on a 70-byte tail (a Story 3.5 build) so the shorter tail stays covered.
  - [x] 7.3 `ocb-core`: no change (STATUS `extra` is opaque; C.3 already covers its limit). Confirm no vector change: `check-proto-version.sh` passes, `app/core/tests/vectors.rs` unchanged.
- [x] **Task 8: Docs and test plan (AC: 8)**
  - [x] 8.1 `docs/link-protocol.md` (rules, "STATUS debug counters"): the v5 closing sentence becomes "With v5 the debug tail is 70 bytes with an empty window, at most 102." and a new bullet "v6, right after the v5 block (Story 4.0): one fixed block of 2 bytes" with a one-row table (`| 0 | \`p2_queue_full u16\` | ...`) and "With v6 the debug tail is 72 bytes with an empty window, at most 104; the largest debug STATUS body is 115 bytes (122 bytes on the wire), which fits the controller's 128-byte UART TX ring in one frame."
  - [x] 8.2 `tests/test_docs_constants.py` `check_story_2_5`: the sum includes `V6_LEN`; block sizes `[v4, v5, v6]`; a v5 tail sentence check and the "largest" check moved to the v6 sentence; offsets list gains the v6 `0`. Red proof: the old doc fails the new check.
  - [x] 8.3 `firmware/README.md` (gate list, lines ~96–109: the new rule families, `tools/pure-rules.sh`, proto now also checks identifiers) and `firmware/controller/README.md` (debug STATUS mentions: v6 next to v4 / v5).
  - [x] 8.4 `docs/test-plan.md`: F.13, F.18, F.30, F.40 (new rule families, fixture counts, "uses tools/pure-rules.sh" instead of "equals the core gate's"); new row F.43 for `proto_pure_check` (includes and identifiers); F.16 (v6, 122 bytes); F.19 / F.32 (v5 / v6 decoding, p2check note); F.22 (v6 sizes); F.31 (sizes); the C row for the ocb-dev tap parse and the sim (C.17 or a new C.20). H.15's procedure may say p2check now prints `p2_queue_full` (text only; no bench run in this story).
- [x] **Task 9: Verify and close (AC: 9)**
  - [x] 9.1 `cmake --preset host-tests && cmake --build --preset host-tests && ctest --preset host-tests` (from `firmware/`); the five gates and `check-pins.sh`, `check-proto-version.sh` from the repo root.
  - [x] 9.2 In `app/`: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
  - [x] 9.3 If `arm-none-eabi-gcc` is installed (`firmware/tools/toolchain.env`): build fw-release, fw-debug, fw-debug-o3 and run the ELF gates as CI does; otherwise say so in the Completion Notes and rely on CI.
  - [x] 9.4 `sprint-status.yaml`: story to `review`; action items E2-A4, E2-A5 and E3-A4 to `done` when the PR merges (they track #20, #25 and this story). PR body: `Closes #20`, `Closes #25`.

### Review Findings

Code review 2026-11-26 (gds-code-review, `main...HEAD` at 2032bb8; layers: Blind Hunter, Edge Case Hunter, Acceptance Auditor). Severity: IMPORTANT = a verdict that cannot go red or an AC gap that hides a real regression; MINOR = everything else. Verified green on the host: ctest 44/44 (sanitized), `cargo test --workspace`, the five gates on the real trees, `docs_constants_check`. The size arithmetic (72 / 104 / 115 / 122 bytes), the `put_u16` saturation, the static asserts and the v4-only / v5 / v6 / truncated tail handling in `link_probe.py`, `tap.rs` and the C tests all check out. One rule was mutation-tested at a time (each deleted from `pure-rules.sh`): every alternative turns a self-test red except `__builtin_alloca`.

- [x] [Review][Patch] IMPORTANT: the `__builtin_alloca` fixture cannot fail for its own rule [firmware/tests/core_pure_fixtures/bad/builtin_alloca.c:1]. Line 1 is the comment "(the compiler builtin of alloca)". Its bare `alloca)` already matches the plain `alloca` alternative. Code line 5 (`__builtin_alloca(4)`) is not matched by `alloca`, because of the `_` before it and `nb`. Failure scenario (reproduced): delete `|__builtin_alloca` from `pure-rules.sh:19`, and all five self-tests stay green, so a real `__builtin_alloca(n)` in `controller/core` passes the gate. Fix: drop the word "alloca" from the comment, and assert that each bad fixture is flagged on a non-comment line (or on an expected line number). **Fixed: the fixture comment no longer contains the word; every self-test now fails a bad fixture that is flagged only on a comment line (red proof R2 in the Debug Log).**
- [x] [Review][Patch] IMPORTANT: the shared-list guard checks only `banned_word_re=`, and the proto, p2 and bridge gates have no fixture for the section or register rules (link has no register fixture) [firmware/tests/core_pure_selftest.sh:42-51, link_pure_selftest.sh:42-51, p2_pure_selftest.sh:42-51, proto_pure_selftest.sh:41-50, bridge_pure_selftest.sh:56-65]. The guard only needs a `. "${script_dir}/pure-rules.sh"` line and no line starting with `banned_word_re=`. Failure scenario (reproduced by the Edge Case Hunter): in `check-p2-pure.sh`, `check-proto-pure.sh` or `check-bridge-pure.sh`, replace `pure_ident_hits "${f}"` with a grep of `banned_word_re` only. Or reassign `raw_addr_re='^$'` / `section_re` after sourcing, or use `banned_word_re+=` / `readonly banned_word_re=`. In every case the self-test stays green while that tree stops enforcing two of the three rule families. This defeats D1/AC3 ("cannot drift"). Fix: give each gate a `section_attr.c` and a `register_*.c` bad fixture; make the guard require a `pure_ident_hits "${f}"` call and refuse any assignment of `nb`, `banned_word_re`, `section_re`, `raw_addr_re` or a `pure_ident_hits()` definition in the gate. **Fixed: `section_attr.c` and `register_cast.c` bad fixtures in every gate; the guard requires the `pure_ident_hits "${f}"` call and refuses any `=` / `+=` of `nb`, `banned_word_re`, `section_re`, `raw_addr_re` (also behind `readonly`) and a local `pure_ident_hits` (red proof R3).**
- [x] [Review][Patch] MINOR: p2check skips the `p2_queue_full` note on its early-return failure paths [firmware/tools/link_probe.py:2067, :2074]. The `c.note(p2_queue_full_note(link.status))` at :2098 is reached only when the run gets to the end. Failure scenario: the console never reaches `console_ready 1`, but STATUS frames with a v6 block did arrive. p2check prints FAIL with no overflow count, even though that is the run where dropped replies matter most (issue #25). Fix: emit the note before every `return c.finish()`; it stays a note, not a check (D5). **Fixed: the note is printed before both early `return c.finish()` (no INFO, no `console_ready`) (red proof R4).**
- [x] [Review][Patch] MINOR: the alloca deny list misses `__builtin_alloca_with_align` (and `__builtin_alloca_with_align_and_max`) [firmware/tools/pure-rules.sh:19]. Both are followed by `_`, so neither `alloca` nor `__builtin_alloca` matches under the `([^A-Za-z0-9_]|$)` tail. Failure scenario: `return __builtin_alloca_with_align(4, 8);` passes all five gates (checked with BSD grep). Fix: `__builtin_alloca[A-Za-z0-9_]*` plus a fixture. **Fixed: `__builtin_alloca[A-Za-z0-9_]*` and fixture `builtin_alloca_align.c` (red proof R1).**
- [x] [Review][Patch] MINOR: the good fixtures lack the AC2 look-alike `section "Controller modes"` and any prefixed call-form look-alike (`ocb_exit(`, `ocb_core_abort(`, `my_asm(`) [firmware/tests/*_pure_fixtures/good/allowed.c:19-28]. `ocb_fixture_aborted` and `ocb_exit_code` have no `(`, so they never reach the call-form alternatives. Failure scenario: someone loosens or removes `nb` in front of the exit group, every good fixture still passes, and the false positive is found only when a real tree breaks. AC4 lists these look-alikes explicitly. **Fixed: every `good/allowed.c` has `section "Controller modes"` in a comment, `ocb_exit(`, `ocb_core_abort(`, `my_asm(`, `ocb_fixture_section(` and `0x1FFFFFFFu` (red proof R5: they catch a rule with the word boundary dropped).**
- [x] [Review][Patch] MINOR: `firmware/controller/README.md` does not describe the new gate rules where it describes the core and link gates [firmware/controller/README.md:15-18, :46-48]. Those paragraphs still say only "allow-list / no clock / no I/O". AC8 asks for the gate rules there; Task 8.3 narrowed this to v6 only, so the task and the AC disagree. **Fixed: the core and link paragraphs describe the shared rules.**
- [x] [Review][Patch] MINOR: the test plan keeps stale STATUS sizes [docs/test-plan.md:903 (F.31 "its largest form (67-byte body)"), :889 (F.17 "debug STATUS frames reach their 67-byte maximum")]. F.31 now says both 67 and 115 bytes. F.17 was not updated although `test_link_fuzz.c`'s matching comment was (now 104). A reader takes the wrong maximum from the row. **Fixed: F.17 and F.31 give the 104-byte tail / 115-byte body.**
- [x] [Review][Decision] MINOR: the scope of the register-address rule — XIP registers and qualified casts are not caught [firmware/tools/pure-rules.sh:29]. The windows are 0x40/0x50/0xd0/0xe0 only, so `XIP_CTRL_BASE 0x14000000` and `XIP_SSI_BASE 0x18000000` (RP2040 datasheet 2.2, also peripheral registers) pass. The cast branch needs `*` right before `)`, so `*(volatile uint32_t * const)0x20041000`, `(volatile uint32_t *)(uintptr_t)0x20041000` and typedef'd pointer casts also pass when the address is outside the windows. This matches AC1 to the letter, but the README and gate comments say "a hex literal cast to a pointer" without that caveat. Owner decision: add the 0x14/0x18 windows and a `*[[:space:]]*(const|volatile)?[[:space:]]*\)` variant, or state the limits in the docs. **Fixed (owner: widen): qualified casts (`const`, `volatile`, `restrict` after `*`), one extra cast (`(T *)(uintptr_t)0x...`) and the XIP windows 0x10-0x15 and 0x18; fixtures `register_cast_const.c`, `register_cast_double.c`, `register_flash.c`, `register_xip.c`, `register_xip_ssi.c`; look-alikes stay legal; `pure-rules.sh`, the gate headers, `firmware/README.md` and F.13 say exactly what is matched, and name the typedef-cast gap (red proof R1).**
- [x] [Review][Decision] MINOR: false positives the "call form only" wording does not warn about [firmware/tools/pure-rules.sh:9-13, :19, :23, :29]. `[[:space:]]*\(` lets prose such as `/* on abort (D7) */` or `/* see section (2.2) */` trip the gates. The bare `asm` alternative flags any comment with the word "asm". The window literal flags legitimate high-bit masks `0x40000000u` (bit 30), `0xE0000000u` and `0x50000000`. The real trees pass today. Owner decision: accept this and document it in the header (rephrase the comment, or use `(1u << 30)`), or tighten the rules (for example no space before `(`, or no window match on masks). **Fixed (owner: document, no matching change): the `pure-rules.sh` header lists `abort (` / `exit (` / `section (` in a comment, the word `asm`, and window masks (`0x40000000u`, `0x10000000u`, `0x12345678`), each with its workaround; `firmware/README.md` points to it.**
- [x] [Review][Defer] MINOR: the fake controller writes v6 at the fixed offset 70 and ignores the P2 log window [app/sim/src/controller.rs:641-644]. It copies the pre-existing v5 assumption (fixed offset 65, Story 3.5). A sim test whose `set_debug_extra` tail has `t[23] = n > 0` gets `p2_queue_full` written into the v4 block, while `tap.rs` reads v6 at `24 + 2n + 46` and decodes the caller's bytes. No current test uses n > 0. Deferred, pre-existing; fix v5 and v6 together by reading byte 23. **Deferred (owner, 2026-11-26): code left as is.**

Dismissed (2): "a profile restart resets `p2_queue_full`": false, because `ocb_p2_restart` (`profile_p2/p2.c:170-179`) does not touch `cnt`, so "since boot" holds. "The `ramfunc.h` exemption works by basename in any tree": this is what AC2 / Story 2.4 D12 specify.

## Dev Notes

### Scope in one paragraph

Two small host-only changes, tied together only by the retro: (a) the purity gates learn the identifiers review finding M-5 of Story 2.6 listed (issue #20); (b) the debug STATUS grows a 2-byte v6 block with the P2 reply-queue overflow counter, decoded by the bench probe and the Rust dev tools (issue #25). No wire value, no release-build behaviour and no bench procedure changes.

### Part A: purity gates (#20)

**Current state (read before editing).**
- Five gates, not three: `firmware/tools/check-proto-pure.sh` (Story 2.2: includes only, `*.c *.h`), `check-core-pure.sh` (Story 2.4 D9: includes, identifiers, CMake), `check-link-pure.sh` (Story 2.6 D16), `check-p2-pure.sh` (Story 2.3 D16), `check-bridge-pure.sh` (Story 3.1). The last four scan `*.c *.h *.cpp *.cc *.S *.inc` and `CMakeLists.txt`.
- There is **no shared file** today: core, link, p2 and bridge each carry an identical `banned_word_re=` line, and `link_pure_selftest.sh`, `p2_pure_selftest.sh`, `bridge_pure_selftest.sh` fail if their line differs from the core gate's (the drift guard of Story 2.6 D16). The proto gate has no identifier rule.
- The current list: `nb='(^|[^A-Za-z0-9_])'`, `banned_word_re="${nb}(malloc|calloc|realloc|free[[:space:]]*\\(|[a-z]*printf|time_us|get_absolute_time|clock_gettime|gettimeofday|sleep_|busy_wait|multicore_|mutex_|spin_lock|__not_in_flash_func|stdio_|(puts|putchar)([^A-Za-z0-9_]|\$))"`. POSIX ERE, no `\b` (Story 2.4 D20); portable to GNU and BSD grep (macOS here; note the shell `grep` may be `ugrep`; the gates call `grep -E` from bash and must keep working with BSD, GNU and ugrep).
- The rules match **code and comments** alike. That is why `abort` and `exit` must be matched in call form (`abort[[:space:]]*\(`), like `free(`: today the real trees contain the word "abort" ten times in comments and identifiers (`aborted`, "the abort latch", `core.h`, `modes.c`, `watchdog.h`, `ready.h`, `link.h`), and none as a call. Measured on 2026-11-26: zero call-form hits of `exit`/`abort`, zero hits of `fopen|strdup|alloca|asm|__asm|__asm__`, zero `section(`, zero pointer-cast hex literals and zero `0x[45de]0xxxxxx` literals in all five trees; the only word "section" hits are two comments (`ramfunc.h:7` "a section attribute", `core.h:18` `section "Controller modes`), neither followed by `(`.
- `OCB_CORE_RAMFUNC` is defined empty in `controller/core/include/ocb_core/ramfunc.h`; a build may define it by compile definition (Story 2.4 D12, Story 2.5 D2: no build does). The `section(` exception is about that file: if a later build gives the macro a section attribute, it is written there.

**Decision D1: one sourced rule file, `firmware/tools/pure-rules.sh`.** The issue says "the shared deny list"; five copies with three drift guards (the core vs. proto gap included) make each new rule a five-file edit. A sourced file is the single list; each gate keeps what differs (include allow-list, banned includes, CMake rule). The self-test guard changes from "same line as core" to "sources the file, no private `banned_word_re=`". Keep the gates runnable from any cwd (`script_dir` is already computed in each).

**Decision D2: regexes (start here; adjust only with a fixture proving why).**
```bash
nb='(^|[^A-Za-z0-9_])'
banned_word_re="${nb}(malloc|calloc|realloc|free[[:space:]]*\\(|[a-z]*printf|time_us|get_absolute_time|clock_gettime|gettimeofday|sleep_|busy_wait|multicore_|mutex_|spin_lock|__not_in_flash_func|stdio_|(puts|putchar)([^A-Za-z0-9_]|\$)|(exit|_exit|_Exit|quick_exit|abort)[[:space:]]*\\(|(fopen|strdup|strndup|alloca|__builtin_alloca|asm|__asm|__asm__)([^A-Za-z0-9_]|\$))"
section_re="${nb}(section|__section__)[[:space:]]*\\("      # refused unless the basename is ramfunc.h
raw_addr_re="\\*[[:space:]]*\\)[[:space:]]*\\(?[[:space:]]*0[xX][0-9A-Fa-f]+|${nb}0[xX][45dDeE]0[0-9A-Fa-f]{6}([uUlL]|[^0-9A-Za-z_]|\$)"
```
Checked at story creation (bash + the machine's `grep -E`) on sample lines: flagged `exit(1);`, `_exit(1);`, `abort ();`, `fopen(`, `strdup(s)`, `alloca(4)`, `__builtin_alloca(4)`, `asm volatile("nop");`, `__asm__ __volatile__(...)`, `*(volatile uint32_t *)0x4001c000`, `0x40014000u`, `0xd0000000`, `__attribute__((section(".x")))`, `__attribute__((__section__(".x")))`; legal: `bool aborted;`, `/* abort latch, exit path */`, `int ocb_exit_code;`, `0x80000000u`, `0xFFFFFFFFu`, `0xEDB88320u`, `(void *)0`, `0x400140001` (9 digits), ` * a section attribute`, `section "Controller`. Re-run this on GNU grep too (CI is ubuntu).
- `asm` as a bare word catches `asm volatile(`, `asm(`; `__asm__` and `__asm` are listed because `_` is a word character (the same reason `__builtin_alloca` is listed apart from `alloca`).
- Raw addresses: (a) any hex literal right after a pointer cast, `(T *)0x...` or `(volatile uint32_t *)(0x...)`; (b) an 8-digit literal in the RP2040 peripheral windows: APB `0x40`, AHB-Lite (USB, PIO) `0x50`, SIO `0xd0`, Cortex-M0+ PPB `0xe0` (RP2040 datasheet §2.2 address map), with or without a `u` / `l` suffix. (b) leaves `0x80000000` (`OCB_T_US_LIMIT`), `0xFFFFFFFFu` and a CRC-32 polynomial `0xEDB88320u` legal. Do not ban `(void *)0` (decimal), and do not ban all 8-digit hex literals.
- The `section(` exception goes by basename (`[ "$(basename "$f")" = ramfunc.h ]`), not by path, so the fixture `good/ramfunc.h` proves it.

**Fixtures (Task 1).** Under `core_pure_fixtures/bad/` (names are suggestions; one rule each, comment line first as the existing fixtures do: `/* Fixture for check-core-pure.sh: must be flagged (...). */`):
`calls_exit.c`, `calls_underscore_exit.c` (`_exit(1)`), `calls_abort.c`, `opens_file.c` (`fopen`), `dup_string.c` (`strdup`), `stack_alloc.c` (`alloca`), `builtin_alloca.c` (`__builtin_alloca`), `inline_asm.c` (`asm volatile("nop");`), `inline_asm_gnu.c` (`__asm__ __volatile__("cpsid i");`), `register_cast.c` (`*(volatile uint32_t *)0x4001c000 = 0;`), `register_addr_suffix.c` (`#define OCB_FIXTURE_REG 0x40014000u`), `section_attr.c` (`__attribute__((section(".time_critical"))) void f(void);`), `section_dunder.c` (`__attribute__((__section__(".data")))`).
One sentinel each (e.g. `calls_abort.c`) in `proto_pure_fixtures/bad/`, `link_pure_fixtures/bad/`, `p2_pure_fixtures/bad/`, `bridge_pure_fixtures/bad/`, plus a `section_attr.c` in the link set (link includes `ramfunc.h`, the most likely place to misuse a section).
Good fixtures: extend each `good/allowed.c` with `bool aborted;`, a comment "abort latch, exit path", `int ocb_exit_code;`, `#define OCB_FIXTURE_LIMIT 0x80000000u`, `0xFFFFFFFFu`, `0xEDB88320u`; core `good/ramfunc.h` with `#define OCB_CORE_RAMFUNC __attribute__((section(".time_critical.ocb")))` (include guard, no includes). The proto good fixture only gets the `.c` look-alikes (its gate scans `*.c *.h`, so a `ramfunc.h` there also works if you want it).

**Must be preserved.** Every existing bad fixture still fails and every good fixture still passes; the exit codes (0 clean, 1 violation printed as `file:line:text`, 2 usage); `LC_ALL=C sort -t: -k1,1n -u` de-duplication (one line hit by two rules prints once); the per-gate include allow-lists and CMake rules; `check-pins.sh` and the other tools are not touched. The real trees must pass unchanged: if a rule hits real code, the rule is wrong (or tell the owner), not the code.

### Part B: P2 reply-queue overflows in the debug STATUS (#25)

**Current state.**
- `ocb_p2` (`controller/profile_p2/p2.c`): `reply_usb()` and `reply_sub()` increment `p->cnt.queue_full` (u32) and return `OCB_P2_RX_QUEUE_FULL` when the 4-entry queue (`OCB_P2_QUEUE_DEPTH`) is full. `ocb_p2_init` zeroes it; `ocb_p2_restart` does **not** (only `restarts++`), so the value is "since boot". Tested by F.25 (`test_p2_handshake` "queue_full counted") and F.26 (`test_p2_subcmd`, 4 drops). `ocb_p2` itself does not change.
- Nobody reads `cnt.queue_full` today. Core 0's glue `controller/platform/usb_p2.c` owns `static ocb_p2 s_p2` and posts the Story 2.5 self-timing to the cross-core mailbox in `post_self_timing()` only when values change (`s_spread_posted`, `s_core0_posted`). That function runs every pass (line ~236) in all builds; follow the same pattern for `queue_full`.
- Mailbox: `ocb_usb_mailbox` in `controller/link/include/ocb_link/ready.h`, single writer (core 0), aligned 32-bit stores, core 1 reads. Setters in `controller/link/ready.c` (`ocb_usb_mailbox_set_core0` is the model). `ocb_usb_mailbox_init` zeroes every field explicitly (add the new one there).
- Debug STATUS: `controller/link/tx_debug.c` builds the tail when `cfg.debug_counters` is set (debug builds: `OCB_LINK_DEBUG_COUNTERS`, `platform/link_runner.c:105`): v1+v2 (21 bytes), v3 header (3) + window (2n, n ≤ 16), v4 (41), v5 (5, Story 3.5). Layout comments in `ocb_link/config.h`. The tail is positional with no version byte: a reader detects a block by the tail length (`len >= off + block_len`), which is how Story 3.5 added v5 without touching any reader that stops earlier. "v6" follows that convention.
- Size limits: `TX_DBG_UART_RING 128` restated in `tx_debug.c` (a pure library cannot include `ocb_hw/uart_link.h`) and checked against `OCB_UART_LINK_TX_RING` by the `_Static_assert` in `platform/link_runner.c`. With v6: body 11 + 104 = 115, raw 3 + 115 + 2 = 120, COBS +1, delimiter +1 = **122 ≤ 128**. Also ≤ `OCB_MAX_BODY` / `OCB_MAX_WIRE_FRAME`. COBS overhead stays 1 byte (frame < 254).
- **Gap found while preparing:** `link_probe.py` decodes v1 to v4 only; Story 3.5 put the v5 decoding in Rust (`ocb-dev` `tap.rs`) and in `ocb-sim`, not in the probe. The probe must skip past v5 to reach v6, so decode v5 as well (AC7).
- Rust: `ocb-core` `StatusMsg.extra` is an opaque `Vec<u8>` (`app/core/src/protocol/msgs.rs`), the vectors have one opaque `status_extra` case (`messages.txt`); neither changes. `ocb-dev` `tap.rs` `DebugCounters::parse` reads v4 at `24 + 2n` and v5 at `+41` with `>=` length checks (forward compatible). `ocb-sim` `controller.rs` `debug_tail()` writes v5 at 65..70 when the tail is ≥ 70 bytes. The ocb-dev tests build a 70-byte tail in `tests/common/mod.rs` (`debug_tail`, `debug_tail_v4`).

**Decision D3: block and field.** v6 = one u16, `p2_queue_full`, saturating at 65535 (`put_u16` already saturates), right after v5. u16 like every other debug counter; a 4-entry queue that overflows 65535 times is a broken console anyway. No reset at profile restart (as the profile's counter); documented as "since boot".

**Decision D4: always posted, shown in debug builds only.** The mailbox post runs in every build like the Story 2.5 self-timing posts (one compare and at most one store per pass on core 0); only the debug STATUS shows it, which is what #25 means by "debug builds only, like the P2 log". The release TX path (`tx.c`) and the release STATUS (11 bytes) do not change. Do not add `#ifdef OCB_DEBUG_BUILD` around the post unless `check-release-nolog.sh` objects (it looks for the debug log, the test melody and the hang injector, not the mailbox).

**Decision D5: p2check prints, it does not judge.** #25: "have p2check print it". A `c.note()` line, so H.15's verdict is unchanged and no bench re-run is needed. Both output branches (a value, "not reported") get a test that fails on the old code (E3-A2).

**Decision D6: no wire change.** Debug tail is not a wire value (`config.h` header: "None of them is a wire value"; `link-protocol.md`: "These are debug aids, not wire values: no `PROTO_VERSION` change, and a later debug layout may append more"). No `consts.h`, `constants.txt`, vector or `PROTO_VERSION` edit; `check-proto-version.sh` must pass untouched.

**Must be preserved.** All v1–v5 offsets; the 50 ms STATUS cadence and the "never split" rule (`wl > cap` → stays due); `put_v4` / `put_v5` behaviour (Story 3.5's F.16 checks keep passing at the same offsets); the release image's stack and RAM-hot-path figures (`check-stack.sh` limits on `ocb_link_tx_status_debug_`: its buffers grow by 2 bytes, re-run the gate on fw-debug-o3 if the toolchain is present); `ocb-dev` `failsafe` / `watch` verdicts (they read v5 only).

### Files to touch

| File | Change |
|---|---|
| `firmware/tools/pure-rules.sh` | NEW: shared identifier rules |
| `firmware/tools/check-{proto,core,link,p2,bridge}-pure.sh` | source the rules; comments; messages; proto gains rule (b) |
| `firmware/tests/{proto,core,link,p2,bridge}_pure_selftest.sh` | shared-list guard replaces the drift guard |
| `firmware/tests/*_pure_fixtures/{bad,good}/` | new fixtures |
| `firmware/tests/CMakeLists.txt` | comments of the pure checks |
| `firmware/controller/link/include/ocb_link/config.h` | v6 layout, `V6_LEN`, `TAIL_MAX_LEN` 104 |
| `firmware/controller/link/include/ocb_link/ready.h`, `firmware/controller/link/ready.c` | mailbox field + setter |
| `firmware/controller/link/tx_debug.c` | `put_v6`, static assert |
| `firmware/controller/platform/usb_p2.c` | post `queue_full` |
| `firmware/controller/platform/link_runner.c` | comment only, if any |
| `firmware/tests/test_link_tx.c`, `test_link_dbglog.c`, `test_link_ready.c`, `link_sim_shim.c` | tests, sim setter |
| `firmware/tools/link_probe.py`, `firmware/tests/test_link_probe.py` | v5 / v6 decode, p2check note, tests |
| `firmware/tests/test_docs_constants.py` | v6 sizes |
| `app/tools/ocb-dev/src/tap.rs`, `app/tools/ocb-dev/tests/common/mod.rs` | parse, tests, 72-byte tail |
| `app/sim/src/controller.rs`, `app/sim/tests/…` | v6 in the fake tail, setter, test |
| `docs/link-protocol.md`, `docs/test-plan.md`, `firmware/README.md`, `firmware/controller/README.md` | docs |
| `_bmad-output/implementation-artifacts/sprint-status.yaml`, this file | status |

Not touched: `common/proto/**` (sources, vectors), `app/core/**`, `controller/profile_p2/**`, `bridge/**` sources, `.github/workflows/firmware.yml` (the gates are called the same way), `_bmad/`, any `bmad-*` / `gds-*` skill folder.

### Testing standards

- Firmware host tests: C99 with `ocb_test.h` (`OCB_CHECK`, `OCB_CHECK_MEM`), ASan/UBSan in the `host-tests` preset, `ocb_strict_warnings` on every new C file. Shell self-tests: bash, `set -euo pipefail`, rc conventions above. Python: stdlib only, run with `python3 -I` by ctest.
- Red proofs are part of done (retro E3-A2 and #20's "show each one fails before the fix"): each new fixture, each new C check, each new Python output branch, the docs check and the shared-list guard. Record command, the failing line, then the passing run, in the Debug Log.
- Rust: `cargo test --workspace`, `clippy -D warnings`, `fmt --check` (the `app` CI runs them on ubuntu, windows and macos).

### Previous story intelligence (Story 3.5, Epic 3 retro)

- Story 3.5 added v5 the same way this story adds v6: `config.h` layout + length, `tx_debug.c` writer + static assert, `test_link_tx.c` (exact extra length, saturation, largest frame), `test_link_dbglog.c` (window sizes), `test_docs_constants.py` (sizes, offsets list), `link-protocol.md` (table + size sentence), then the Rust readers (`tap.rs`, `ocb-sim` `controller.rs`). Mirror that file set; it is the checklist.
- Story 3.5's review found ACs that could not be measured; here every AC maps to a host test (table above).
- The retro's lesson "bench tools need a red proof per verdict branch" applies to the p2check note as an output branch.
- Shell environment: macOS, zsh as the interactive shell; run gate experiments with `bash -c` (zsh does not word-split unquoted variables, which silently breaks loops over directory lists).

### Git intelligence

Recent work is docs only (Epic 3 retro, Devlog #3, sprint-status action items, commits `f3ffb63`..`6a03123`). The last code story is 3.5 (`b1d9377`, PR #35): firmware v5 block, `ocb-dev failsafe` / `watch`, `ocb-sim` changes. Branch `story/4-0-cleanup-purity-gates-and-p2-overflow-status` is checked out from `main`.

### Latest tech information

No new library or version. Tools in use: bash + POSIX ERE grep (GNU / BSD / ugrep compatible), CMake presets, arm-none-eabi-gcc from `firmware/tools/toolchain.env`, pico-sdk and TinyUSB 0.18.0 (unchanged), Rust toolchain pinned in `app/rust-toolchain.toml`. RP2040 address map (datasheet §2.2): ROM `0x0`, XIP `0x10000000`, SRAM `0x20000000`, APB peripherals `0x40000000`, AHB-Lite peripherals `0x50000000`, SIO `0xd0000000`, Cortex-M0+ internal registers `0xe0000000`.

### Project Context Rules

No `project-context.md` exists. Rules that apply, from the repo and the owner:
- All repo content in English (chat with the owner in Italian).
- AD-2: `ocb_proto`, `ocb_core`, `ocb_link`, `ocb_p2`, `ocb_bridge_core` stay pure C99 that builds on the host; the gates are its enforcement.
- Never edit or commit `_bmad/` or `bmad-*` / `gds-*` skill folders; only `_bmad-output/` is committed.
- Test rows live in `docs/test-plan.md`; host rows are F.* (firmware) and C.* (app).
- Host tests only for this story (owner, retro action 4).

### Open questions for the owner

None. The choices that are not spelled out in #20 / #25 (one sourced rule file, call-form matching for `exit` / `abort`, the raw-address heuristic, v6 as one saturating u16 since boot, p2check printing a note rather than a verdict) are decided above (D1 to D6) from the code and the issue text; the code review can overturn any of them.

### References

- [Source: _bmad-output/implementation-artifacts/epic-3-retro-2026-11-26.md#5. Action items] item 4; §6 "The cleanup story (action 4) comes first."
- [Source: GitHub issue #20] Purity gates miss some forbidden identifiers
- [Source: GitHub issue #25] Show P2 reply-queue overflows in the debug STATUS
- [Source: _bmad-output/implementation-artifacts/2-6-controller-link-uart-reliability-session-and-status.md] M-5 (deferred)
- [Source: _bmad-output/implementation-artifacts/2-4-controller-core-modes-queue-scheduler-and-safety-mask.md] D9, D12, D20
- [Source: _bmad-output/implementation-artifacts/3-5-hardware-fail-safe-and-console_ready-checks.md] debug STATUS v5, File List
- [Source: firmware/controller/link/include/ocb_link/config.h] debug tail layouts
- [Source: firmware/controller/link/tx_debug.c] tail writer, size asserts
- [Source: firmware/controller/link/include/ocb_link/ready.h] USB mailbox
- [Source: firmware/controller/platform/usb_p2.c] `post_self_timing()`
- [Source: firmware/controller/profile_p2/p2.c] `reply_usb`, `reply_sub`, `ocb_p2_restart`
- [Source: docs/link-protocol.md] "Rules the controller link applies (Story 2.6)", STATUS debug counters
- [Source: docs/test-plan.md] F.13, F.16, F.18, F.19, F.22, F.30, F.31, F.32, F.40, C.17, H.15
- [Source: firmware/tests/test_docs_constants.py] `check_story_2_5`
- [Source: app/tools/ocb-dev/src/tap.rs] `DebugCounters::parse`
- [Source: app/sim/src/controller.rs] `debug_tail()`

## Dev Agent Record

### Agent Model Used

Claude Opus 5.5 (claude-opus-5-5), gds-dev-story, run non-interactively.

### Debug Log References

All runs on the host (macOS, BSD grep 2.6.0 under bash; GNU grep 3.11 in an `ubuntu:24.04` container for the gates). No bench, no Switch, no flashing.

**Red proof 1: new gate fixtures on the unchanged gates (Task 1.2, AC4).** Command: `bash tests/<g>_pure_selftest.sh tools/check-<g>-pure.sh .` from `firmware/`, gates as on `f3ffb63`, new fixtures in place. Every new bad fixture is reported as not flagged; no good fixture is reported (the look-alikes are legal on both gates):
```
### proto_pure_selftest.sh (gates unchanged)
FAIL: bad fixture not reported (rc=0): ./tests/proto_pure_fixtures/bad/calls_abort.c
### core_pure_selftest.sh (gates unchanged)
FAIL: bad fixture not reported (rc=0): ./tests/core_pure_fixtures/bad/builtin_alloca.c
FAIL: bad fixture not reported (rc=0): ./tests/core_pure_fixtures/bad/calls_abort.c
FAIL: bad fixture not reported (rc=0): ./tests/core_pure_fixtures/bad/calls_capital_exit.c
FAIL: bad fixture not reported (rc=0): ./tests/core_pure_fixtures/bad/calls_exit.c
FAIL: bad fixture not reported (rc=0): ./tests/core_pure_fixtures/bad/calls_quick_exit.c
FAIL: bad fixture not reported (rc=0): ./tests/core_pure_fixtures/bad/calls_underscore_exit.c
FAIL: bad fixture not reported (rc=0): ./tests/core_pure_fixtures/bad/dup_string_n.c
FAIL: bad fixture not reported (rc=0): ./tests/core_pure_fixtures/bad/dup_string.c
FAIL: bad fixture not reported (rc=0): ./tests/core_pure_fixtures/bad/inline_asm_gnu.c
FAIL: bad fixture not reported (rc=0): ./tests/core_pure_fixtures/bad/inline_asm_short.c
FAIL: bad fixture not reported (rc=0): ./tests/core_pure_fixtures/bad/inline_asm.c
FAIL: bad fixture not reported (rc=0): ./tests/core_pure_fixtures/bad/opens_file.c
FAIL: bad fixture not reported (rc=0): ./tests/core_pure_fixtures/bad/register_addr_suffix.c
FAIL: bad fixture not reported (rc=0): ./tests/core_pure_fixtures/bad/register_ahb.c
FAIL: bad fixture not reported (rc=0): ./tests/core_pure_fixtures/bad/register_cast_paren.c
FAIL: bad fixture not reported (rc=0): ./tests/core_pure_fixtures/bad/register_cast.c
FAIL: bad fixture not reported (rc=0): ./tests/core_pure_fixtures/bad/register_ppb.c
FAIL: bad fixture not reported (rc=0): ./tests/core_pure_fixtures/bad/register_sio.c
FAIL: bad fixture not reported (rc=0): ./tests/core_pure_fixtures/bad/section_attr.c
FAIL: bad fixture not reported (rc=0): ./tests/core_pure_fixtures/bad/section_dunder.c
FAIL: bad fixture not reported (rc=0): ./tests/core_pure_fixtures/bad/stack_alloc.c
### link_pure_selftest.sh (gates unchanged)
FAIL: bad fixture not reported (rc=0): ./tests/link_pure_fixtures/bad/calls_abort.c
FAIL: bad fixture not reported (rc=0): ./tests/link_pure_fixtures/bad/section_attr.c
### p2_pure_selftest.sh (gates unchanged)
FAIL: bad fixture not reported (rc=0): ./tests/p2_pure_fixtures/bad/calls_abort.c
### bridge_pure_selftest.sh (gates unchanged)
FAIL: bad fixture not reported (rc=0): ./tests/bridge_pure_fixtures/bad/calls_abort.c
```
Green after Tasks 2 and 3 (same command; identical with GNU grep 3.11 in `ubuntu:24.04`):
```
proto_pure_selftest: ok (5 bad fixtures, rules from tools/pure-rules.sh)
core_pure_selftest: ok (39 bad fixtures, rules from tools/pure-rules.sh)
link_pure_selftest: ok (14 bad fixtures, rules from tools/pure-rules.sh)
p2_pure_selftest: ok (13 bad fixtures, rules from tools/pure-rules.sh)
bridge_pure_selftest: ok (17 bad fixtures, rules from tools/pure-rules.sh)
```
Real trees: `check-proto-pure: ok (14 files)`, `check-core-pure: ok (18 files)`, `check-link-pure: ok (20 files)`, `check-p2-pure: ok (9 files)`, `check-bridge-pure: ok (12 files)`.

**Red proof 2: the shared-list guard (Task 3.1).** Temporary copies of the gates in the scratchpad, run through the real self-tests:
- `check-link-pure.sh` with an added private `banned_word_re="${nb}(malloc)"` after the source line: `FAIL: check-link-pure.sh defines a private banned_word_re` (plus the fixtures the narrowed list misses: `calls_abort.c`, `multicore.c`, `ram_section.c`, `reads_clock.c`).
- `check-p2-pure.sh` with the source line removed and an inline private list: `FAIL: check-p2-pure.sh does not source tools/pure-rules.sh` (plus `calls_abort.c`, `multicore.c`, `reads_clock.c` not reported).
- `check-bridge-pure.sh`, `check-core-pure.sh`, `check-proto-pure.sh` each with a line `banned_word_re="${banned_word_re}"` added (same rules, so only the guard can fail): `FAIL: check-bridge-pure.sh defines a private banned_word_re`, `FAIL: check-core-pure.sh defines a private banned_word_re`, `FAIL: check-proto-pure.sh defines a private banned_word_re`.

**Red proof 3: firmware v6 (Task 5.1, AC5, AC6).** New and updated C checks, built and run with the mailbox field and setter in place but before `config.h`, `put_v6` and the init zeroing (`./build/host-tests/tests/<t>`):
```
test_link_tx.c:318: FAIL: debug: 70 extra bytes
test_link_tx.c:337: FAIL: debug: second STATUS
test_link_tx.c:592: FAIL: largest debug STATUS on the wire: 120 bytes, wanted 122
test_link_tx.c:596: FAIL: the 122-byte STATUS split into 121
test_link_tx.c:597: FAIL: then sent whole
test_link_tx.c:480: FAIL: link up: STATUS link_ok 1, watchdog_trips 0
test_link_tx.c:500: FAIL: after the loss: STATUS link_ok 0, watchdog_trips 1
test_link_tx.c:540: FAIL: v5: STATUS with 72 extra bytes
test_link_tx.c:555: FAIL: v5 saturates
test_link_tx.c:617: FAIL: v6: STATUS with 72 extra bytes (posted 0)
test_link_tx.c:617: FAIL: v6: STATUS with 72 extra bytes (posted 3)
test_link_tx.c:617: FAIL: v6: STATUS with 72 extra bytes (posted 70000)
test_link_tx (F.16): 12 failure(s)
test_link_dbglog.c:259: FAIL: n=0: extra 70, wanted 72
test_link_dbglog.c:259: FAIL: n=1: extra 72, wanted 74
test_link_dbglog.c:259: FAIL: n=16: extra 102, wanted 104
test_link_dbglog.c:162: FAIL: extra 88
test_link_dbglog.c:174: FAIL: full window: extra 102
test_link_dbglog (F.31): 5 failure(s)
test_link_ready.c:370: FAIL: mailbox init: p2_queue_full 2779096485, wanted 0
test_link_ready (F.10): 1 failure(s)
```
The release-config check of `test_v6_block` (11-byte STATUS, no tail) passes on both: it guards behaviour that must not change. Green after Task 4: `test_link_tx (F.16): ok`, `test_link_dbglog (F.31): ok`, `test_link_ready (F.10): ok`.

**Red proof 4: `link_probe.py` decoders and the p2check note (Task 6.3, retro E3-A2).** The new tests run against `link_probe.py` of `f3ffb63` (`git show HEAD:firmware/tools/link_probe.py`), with the new `link_sim_shim.c`:
```
ERROR: test_debug_v5_v6 (__main__.Vectors)           KeyError: 'v5'
ERROR: test_p2_queue_full_note (__main__.Vectors)    AttributeError: module 'link_probe' has no attribute 'p2_queue_full_note'
FAIL: test_p2check (__main__.P2BenchSteps)            'P2 reply-queue overflows (p2_queue_full): 0' not found in '... no NAK VERSION\nPASS\n'
FAIL: test_p2check_prints_p2_queue_full (...)         'P2 reply-queue overflows (p2_queue_full): 3' not found in '... no NAK VERSION\nPASS\n'
Ran 6 tests ... FAILED (failures=2, errors=2)
```
Output branches: value (`test_p2check` 0, `test_p2check_prints_p2_queue_full` 3, end to end through the real link) and "not reported" (`test_p2_queue_full_note` on a v5-only STATUS dict, an empty tail and None). Green with the new probe: `Ran 6 tests ... OK`; full `link_probe_selftest` 60 tests OK.

**Red proof 5: Rust parse and fake tail (Task 7).** With the field and setter in place but the v6 read (`tap.rs`) and write (`controller.rs`) removed:
```
test tap::tests::v6_offset_follows_the_p2_log_window ... FAILED   left: None  right: Some(3)
test tap::tests::v6_p2_queue_full_after_v5 ... FAILED             left: None  right: Some(12345)
test tap::tests::v6_absent_from_a_story_3_5_tail ... ok
test c20_v6_p2_queue_full_read_back_through_a_status ... FAILED   left: [0, 0]  right: [52, 18]
test c20_a_story_3_5_tail_has_no_v6 ... ok
```
Green after restoring both.

**Red proof 6: docs check (Task 8.2).** New `check_story_2_5` against the old `docs/link-protocol.md`:
```
docs_constants_check: FAIL link doc: the v4 / v5 / v6 block sizes [41, 5] differ from config.h (41, 5, 2)
docs_constants_check: FAIL link doc: the v5 tail sizes differ from config.h (70, 102)
docs_constants_check: FAIL link doc: STATUS v6 sizes differ from ocb_link/config.h (72, 104, 115, 122)
docs_constants_check: FAIL link doc: v4 / v5 / v6 table offsets [0, 2, 4, 8, 20, 22, 24, 30, 32, 34, 35, 37, 38, 39, 0, 2, 3] differ from the layouts of config.h
```
After the v6 bullet: `docs_constants_check (F.22): ok`.

**Final verification (Task 9).**
- `cmake --preset host-tests && cmake --build --preset host-tests && ctest --preset host-tests` (from `firmware/`): `100% tests passed out of 44`.
- From the repo root: the five purity gates ok (above), `check-pins: ok`, `check-proto-version: ok (proto_version 01, 7 files)`.
- In `app/` (toolchain 1.100.0): `cargo fmt --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo test --workspace` 131 passed, 0 failed.
- ARM toolchain present (`firmware/.tools/arm-gnu-toolchain-15.2.Rel1`, picotool 2.3.1): fw-release, fw-debug, fw-debug-o3 built with no warning; `check-release-nolog.sh` ok on fw-release and `--expect-present` ok on fw-debug and fw-debug-o3; `check-barrier.sh --self-test` and all three ELFs ok; `check-ram-hot-path.sh` ok (23 hot functions in RAM) on all three; `check-stack.sh` ok: fw-release 1240, fw-debug 1196, fw-debug-o3 1248 bytes (limit 1536; `ocb_link_tx_status_debug_` frame 408 / 392 / 408 bytes).

**Review fixes (2026-11-26), red proofs R1 to R5.** Pre-fix code = commit 2032bb8; mutated copies of the gates and of `pure-rules.sh` live in the scratchpad, the self-tests and fixtures are the repo's.

R1, new core fixtures on the 2032bb8 rules (new self-tests in place):
```
### core_pure_selftest.sh (rules of 2032bb8)
FAIL: bad fixture not reported (rc=0): ./tests/core_pure_fixtures/bad/builtin_alloca_align.c
FAIL: bad fixture not reported (rc=0): ./tests/core_pure_fixtures/bad/register_cast_const.c
FAIL: bad fixture not reported (rc=0): ./tests/core_pure_fixtures/bad/register_cast_double.c
FAIL: bad fixture not reported (rc=0): ./tests/core_pure_fixtures/bad/register_flash.c
FAIL: bad fixture not reported (rc=0): ./tests/core_pure_fixtures/bad/register_xip_ssi.c
FAIL: bad fixture not reported (rc=0): ./tests/core_pure_fixtures/bad/register_xip.c
```
(The proto, link, p2 and bridge `section_attr.c` / `register_cast.c` fixtures are flagged by the 2032bb8 rules, as they should be; their red is in R3, where a gate stops applying the shared rules.) The first draft of the comment-line check treated a dereference line `*(volatile ...)` as a comment continuation and failed every `register_cast.c`; the detector was narrowed to `/*`, `//` or `*` followed by a space, `/` or end of line.

R2, the `__builtin_alloca` fixture and the comment-line check, with `|__builtin_alloca|` deleted from a copy of `pure-rules.sh`:
- 2032bb8 fixture comment, 2032bb8 self-test: no failure for `builtin_alloca.c` (the comment's word `alloca` satisfied the gate: the finding).
- 2032bb8 fixture comment, new self-test: `FAIL: bad fixture flagged only on a comment line: .../builtin_alloca.c`.
- new fixture comment, new self-test: `FAIL: bad fixture not reported (rc=0): .../builtin_alloca.c`.

R3, the tightened guard. Each mutation on a copy of the gate (beside a copy of `pure-rules.sh`): `call` = `pure_ident_hits "${f}"` replaced by a grep of `banned_word_re` only; `raw` = `raw_addr_re='ocb_never_matches'` after the source line; `plus` = `banned_word_re+='|ocb_never_matches'`; `readonly` = `readonly section_re='ocb_never_matches'`; `func` = a local `pure_ident_hits() { grep banned_word_re; }`. The 2032bb8 self-test with the 2032bb8 fixtures stays green on every mutation of the proto, link, p2 and bridge gates (core: green on `plus` and `raw`; its `call`, `readonly` and `func` already lost `section_dunder.c`). The new self-test fails each one:
```
proto/link/p2/bridge/core  call      FAIL: check-<g>-pure.sh does not call pure_ident_hits "${f}"   (+ register_cast.c and section_attr.c not reported)
proto/link/p2/bridge/core  raw       FAIL: check-<g>-pure.sh assigns a shared rule (nb, banned_word_re, section_re or raw_addr_re)   (+ register_cast.c not reported)
proto/link/p2/bridge/core  plus      FAIL: check-<g>-pure.sh assigns a shared rule (nb, banned_word_re, section_re or raw_addr_re)
proto/link/p2/bridge/core  readonly  FAIL: check-<g>-pure.sh assigns a shared rule (nb, banned_word_re, section_re or raw_addr_re)   (+ section_attr.c not reported)
proto/link/p2/bridge/core  func      FAIL: check-<g>-pure.sh defines its own pure_ident_hits   (+ register_cast.c and section_attr.c not reported)
```
(An earlier draft of `raw` / `readonly` used `'^$'`; that matches blank lines, so the old self-test also failed on it, and the mutation was replaced by a never-matching value.)

R4, the p2check note on the early FAIL paths, new tests against the 2032bb8 `cmd_p2check`:
```
FAIL: test_p2check_negative_controls       'P2 reply-queue overflows (p2_queue_full): 0' not found in '... FAIL console_ready 1 within ...'
FAIL: test_p2check_no_info_still_notes_p2_queue_full   'p2_queue_full: not reported' not found in '... FAIL INFO received ...'
```
Green after the fix; `link_probe_selftest` 61 tests OK.

R5, the good look-alikes, against a copy of `pure-rules.sh` with the word boundary dropped (`banned_word_re` += `|(exit|abort)[[:space:]]*\(|asm`, `section_re` without `nb`): the 2032bb8 `good/allowed.c` passes (`check-core-pure: ok (1 files)`), the new one is reported on `ocb_exit(`, `ocb_core_abort(`, `my_asm(`, `ocb_fixture_section(` (8 lines). On the real rules both pass.

Green after the review fixes: `proto_pure_selftest: ok (7 bad fixtures ...)`, `core ... (45 ...)`, `link ... (15 ...)`, `p2 ... (15 ...)`, `bridge ... (19 ...)`, identical with GNU grep 3.11 (`ubuntu:24.04`). Spot checks of the widened rule: flagged `__builtin_alloca_with_align_and_max(4,8,16)`, `*(io_rw_32 *const)0x40014000`, `(volatile uint32_t*)(unsigned long)(0x20041000)`; legal `(void *)0`, `0x400140001`, `0x1FFFFFFFu`, `0x80000000u`, `0xEDB88320u`, `(uint32_t)0x22345678` (`(uint32_t)0x12345678` is flagged: 0x12 is an XIP alias window, a documented false positive).

Re-verification after the review fixes: ctest `100% tests passed out of 44`; the five gates ok on the real trees (14 / 18 / 20 / 9 / 12 files), `check-pins: ok`, `check-proto-version: ok`, `docs_constants_check (F.22): ok`; `cargo fmt --check` clean, `cargo clippy --workspace --all-targets -- -D warnings` clean, `cargo test --workspace` 131 passed, 0 failed; fw-release, fw-debug, fw-debug-o3 rebuilt, `check-release-nolog.sh`, `check-barrier.sh` (with `--self-test`), `check-ram-hot-path.sh` and `check-stack.sh` ok (1240 / 1196 / 1248 bytes, limit 1536; no C source changed in the fixes).

### Completion Notes List

- Ultimate context engine analysis completed - comprehensive developer guide created.
- Part A (#20): `firmware/tools/pure-rules.sh` is the single deny list (D1, D2 regexes as written in Dev Notes, unchanged): `nb`, `banned_word_re` (old list + process exit in call form, `fopen`, `strdup`, `strndup`, `alloca`, `__builtin_alloca`, `asm`, `__asm`, `__asm__`), `section_re` (refused unless the basename is `ramfunc.h`), `raw_addr_re`, and `pure_ident_hits FILE`. All five gates source it; `check-proto-pure.sh` gained rule (b) (still `*.c *.h` only). Exit codes, `file:line:text` output and the sort/de-duplication are unchanged; the real trees pass unchanged (no rule needed adjusting). Each self-test now checks "sources `pure-rules.sh`, no private `banned_word_re=`" in place of the old core-vs-gate line comparison (also added to the core and proto self-tests).
- Fixtures: 21 new core bad fixtures (one per rule family plus each spelling: the 13 suggested plus `_Exit`, `quick_exit`, `strndup`, `__asm`, a parenthesised cast, and AHB / SIO / PPB window literals), one `calls_abort.c` sentinel per other gate, `section_attr.c` for link, legal look-alikes appended to every `good/allowed.c`, and `core_pure_fixtures/good/ramfunc.h` with a section attribute.
- Part B (#25): v6 = `p2_queue_full u16` right after v5 (D3). `ocb_usb_mailbox.p2_queue_full` + `ocb_usb_mailbox_set_p2_queue_full`, zeroed by init; `put_v6` in `tx_debug.c` (saturating `put_u16`), static assert extended; `usb_p2.c` posts `s_p2.cnt.queue_full` from `post_self_timing()` when it changed (`s_qfull_posted`, reset with the others at init), in every build (D4; no `#ifdef`, `check-release-nolog.sh` passes). `tx.c` untouched; release STATUS still 11 bytes. Sizes: tail 72 / 104, body 115, 122 bytes on the wire; `link_runner.c` assert holds (comment only).
- `link_probe.py` decodes v5 and v6 (`DEBUG_V5_LEN`, `DEBUG_V6_LEN`, `LOSS_KINDS`, `v5_str`, `v6_str`), shows both on the STATUS line; `p2_queue_full_note()` gives p2check's closing `c.note` (D5: no verdict change).
- Rust: `DebugCounters.p2_queue_full: Option<u16>` with three unit tests in `tap.rs`; `FakeController::set_p2_queue_full` writes bytes 70..72 of a tail of at least 72 bytes (new `app/sim/tests/debug_tail.rs`); the ocb-dev test tail is 72 bytes, `debug_tail_3_5()` keeps a 70-byte tail and a new `failsafe` test passes on it. `ocb-core` and the vectors are unchanged; `check-proto-version.sh` passes.
- Docs: `link-protocol.md` v5 sentence and v6 bullet/table; `test_docs_constants.py` checks v4/v5/v6 sizes, the v5 and v6 sentences and the offsets; `firmware/README.md` gate section rewritten around `tools/pure-rules.sh` (also lists the bridge gate, which it lacked); `firmware/controller/README.md` mentions v6; `docs/test-plan.md` F.13, F.16, F.18, F.19, F.22, F.30, F.31, F.32, F.40 updated, new F.43 (proto purity gate) and C.20 (v6 in the Rust tools), H.15 procedure text notes the p2check line.
- Small deviations: `test_link_ready.c` needed a 14th zero in the positional `ocb_usb_mailbox` initializer of `test_rebase` (`-Wmissing-field-initializers`); `test_link_fuzz.c`'s "67-byte maximum" comment was already stale and now says 104 bytes. The p2check STATUS line is not printed by `p2check` itself, so the end-to-end test asserts the note only; the `| v6:` STATUS text is covered by `test_debug_v5_v6`.
- Task 9.4: story set to `review` here; the action items E2-A4, E2-A5 and E3-A4 stay `open` in `sprint-status.yaml` until the PR merges, as the task says. PR body must carry `Closes #20` and `Closes #25`.
- Not done here (owner's call): no PR opened, no push.

- Review fixes (2026-11-26): findings 1 to 9 fixed (8 widened and 9 documented as the owner decided), finding 10 deferred; see Review Findings and red proofs R1 to R5. Status stays `review`.

### File List

New:
- firmware/tools/pure-rules.sh
- firmware/tests/core_pure_fixtures/bad/builtin_alloca.c
- firmware/tests/core_pure_fixtures/bad/calls_abort.c
- firmware/tests/core_pure_fixtures/bad/calls_capital_exit.c
- firmware/tests/core_pure_fixtures/bad/calls_exit.c
- firmware/tests/core_pure_fixtures/bad/calls_quick_exit.c
- firmware/tests/core_pure_fixtures/bad/calls_underscore_exit.c
- firmware/tests/core_pure_fixtures/bad/dup_string.c
- firmware/tests/core_pure_fixtures/bad/dup_string_n.c
- firmware/tests/core_pure_fixtures/bad/inline_asm.c
- firmware/tests/core_pure_fixtures/bad/inline_asm_gnu.c
- firmware/tests/core_pure_fixtures/bad/inline_asm_short.c
- firmware/tests/core_pure_fixtures/bad/opens_file.c
- firmware/tests/core_pure_fixtures/bad/register_addr_suffix.c
- firmware/tests/core_pure_fixtures/bad/register_ahb.c
- firmware/tests/core_pure_fixtures/bad/register_cast.c
- firmware/tests/core_pure_fixtures/bad/register_cast_paren.c
- firmware/tests/core_pure_fixtures/bad/register_ppb.c
- firmware/tests/core_pure_fixtures/bad/register_sio.c
- firmware/tests/core_pure_fixtures/bad/section_attr.c
- firmware/tests/core_pure_fixtures/bad/section_dunder.c
- firmware/tests/core_pure_fixtures/bad/stack_alloc.c
- firmware/tests/core_pure_fixtures/good/ramfunc.h
- firmware/tests/link_pure_fixtures/bad/calls_abort.c
- firmware/tests/link_pure_fixtures/bad/section_attr.c
- firmware/tests/p2_pure_fixtures/bad/calls_abort.c
- firmware/tests/proto_pure_fixtures/bad/calls_abort.c
- firmware/tests/bridge_pure_fixtures/bad/calls_abort.c
- firmware/tests/core_pure_fixtures/bad/builtin_alloca_align.c
- firmware/tests/core_pure_fixtures/bad/register_cast_const.c
- firmware/tests/core_pure_fixtures/bad/register_cast_double.c
- firmware/tests/core_pure_fixtures/bad/register_flash.c
- firmware/tests/core_pure_fixtures/bad/register_xip.c
- firmware/tests/core_pure_fixtures/bad/register_xip_ssi.c
- firmware/tests/proto_pure_fixtures/bad/register_cast.c
- firmware/tests/proto_pure_fixtures/bad/section_attr.c
- firmware/tests/link_pure_fixtures/bad/register_cast.c
- firmware/tests/p2_pure_fixtures/bad/register_cast.c
- firmware/tests/p2_pure_fixtures/bad/section_attr.c
- firmware/tests/bridge_pure_fixtures/bad/register_cast.c
- firmware/tests/bridge_pure_fixtures/bad/section_attr.c
- app/sim/tests/debug_tail.rs

Modified:
- firmware/tools/check-proto-pure.sh
- firmware/tools/check-core-pure.sh
- firmware/tools/check-link-pure.sh
- firmware/tools/check-p2-pure.sh
- firmware/tools/check-bridge-pure.sh
- firmware/tools/link_probe.py
- firmware/tests/proto_pure_selftest.sh
- firmware/tests/core_pure_selftest.sh
- firmware/tests/link_pure_selftest.sh
- firmware/tests/p2_pure_selftest.sh
- firmware/tests/bridge_pure_selftest.sh
- firmware/tests/proto_pure_fixtures/good/allowed.c
- firmware/tests/core_pure_fixtures/good/allowed.c
- firmware/tests/link_pure_fixtures/good/allowed.c
- firmware/tests/p2_pure_fixtures/good/allowed.c
- firmware/tests/bridge_pure_fixtures/good/allowed.c
- firmware/tests/CMakeLists.txt
- firmware/tests/test_link_tx.c
- firmware/tests/test_link_dbglog.c
- firmware/tests/test_link_ready.c
- firmware/tests/test_link_fuzz.c
- firmware/tests/link_sim_shim.c
- firmware/tests/test_link_probe.py
- firmware/tests/test_docs_constants.py
- firmware/controller/link/include/ocb_link/config.h
- firmware/controller/link/include/ocb_link/ready.h
- firmware/controller/link/ready.c
- firmware/controller/link/tx_debug.c
- firmware/controller/platform/usb_p2.c
- firmware/controller/platform/link_runner.c
- firmware/README.md
- firmware/controller/README.md
- app/tools/ocb-dev/src/tap.rs
- app/tools/ocb-dev/tests/common/mod.rs
- app/tools/ocb-dev/tests/failsafe.rs
- app/sim/src/controller.rs
- docs/link-protocol.md
- docs/test-plan.md
- _bmad-output/implementation-artifacts/sprint-status.yaml
- _bmad-output/implementation-artifacts/4-0-cleanup-purity-gates-and-p2-overflow-status.md

## Change Log

- 2026-11-26: Story created (gds-create-story), from Epic 3 retro action 4 and issues #20, #25.
- 2026-11-26: Implemented (gds-dev-story): shared purity rules in `tools/pure-rules.sh` used by all five gates, 26 new bad fixtures and legal look-alikes; debug STATUS v6 (`p2_queue_full`) in the firmware, `link_probe.py` (v5 and v6 decoding, p2check note), `ocb-dev` and `ocb-sim`; docs and test plan (F.43, C.20 new). Host verification green; status review.
- 2026-11-26: Code review fixes: comment-free `__builtin_alloca` fixture and a code-line check in every purity self-test; a stronger shared-list guard and section / register fixtures in every gate; `__builtin_alloca*`; the register rule widened to qualified and double casts and the XIP windows; the accepted false positives documented in `pure-rules.sh`; AC4 look-alikes; p2check's note on its early FAIL paths; controller README and stale F.17 / F.31 sizes. The fake controller's fixed v6 offset is deferred. Status stays review.
