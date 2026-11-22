---
baseline_commit: 75e138208a308614c82018c211d8008572440910
---

# Story 2.2: Protocol library and conformance vectors

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As the owner,
I want one pure C protocol library tested against language-neutral vectors,
so that firmware and app can never disagree about the wire format.

## Acceptance Criteria

Copied verbatim from `_bmad-output/planning-artifacts/epics.md`, Story 2.2. These are the definition of done.

**AC1.**
**Given** `firmware/common/proto/` (pure C99, no pico-sdk includes)
**When** it is built with the `host-tests` preset
**Then** it provides COBS encode/decode, CRC-16/CCITT-FALSE, frame encode/decode (`type | seq | len | body | crc16le`), every message struct of `docs/link-protocol.md`, the canonical 8-byte state and `NEUTRAL` (AD-10), `PROTO_VERSION`, `OCB_MAX_ENCODED_FRAME`, the link timing constants and the 460800 baud constant
**And** the decoder checks in the AD-3 order (COBS → len consistency treated as CRC error → CRC → type range → seq → semantics) and counts and drops empty frames and out-of-range types

**AC2.**
**Given** this story assigns the numeric values of NAK reasons, EVENT codes, `mode` and `hid_profile`
**When** they are defined as `OCB_NAK_*`, `OCB_EVT_*` and `OCB_MSG_*` constants
**Then** the same values appear in `firmware/common/proto/vectors/` (one text file per area, `name: hex` lines, `proto_version` header)

**AC3.**
**Given** the vector files
**When** test F.9 runs
**Then** every vector passes: COBS, CRC, every message, a maximum-length frame, an empty frame, back-to-back frames, a leading delimiter, a bad CRC, an overlong run and `NEUTRAL`
**And** CI fails if `PROTO_VERSION` differs from the vector header

### Reading notes on the AC (what the story file adds, none of it changes the AC)

- **"seq → semantics" in the AD-3 order**: `ocb_proto` implements the first four checks (COBS, length consistency, CRC, type range) and hands `type`, `seq` and `body` to the caller. The seq check (go-back-N, Stories 2.6 and 3.3) and the semantic checks (mode table, Stories 2.4 and 2.6) need session state and live in the consumers. Message-body parsing (`ocb_msg_*_decode`) is available to them and reports a wrong body length as its own error (a semantic matter, D10).
- **"every message struct of `docs/link-protocol.md`"**: HELLO, INFO, SET_STATE, QUEUE_EVENT, START, STOP, PING, HEARTBEAT, STATUS, ACK, NAK, EVENT (12 messages).
- **"`OCB_MSG_*` constants including `mode` and `hid_profile`"**: the AC lists three prefixes. Message types are `OCB_MSG_*`; `mode` and `hid_profile` are field values and get `OCB_MODE_*` and `OCB_HID_PROFILE_*` (D1). The numeric values are what the AC cares about and they are in the vectors.
- **"`name: hex` lines"**: D2 refines this to `case.field: value` lines (still one line per `name: hex`, one parser rule, no escaping).

## Tasks / Subtasks

All tasks are for the dev agent and need no hardware. There is no owner-at-the-bench task in this story. The dev agent does not commit or push; CI cannot be proven without a push (see guardrails).

- [x] **Task 1: `ocb_proto` skeleton and constants header** (AC: #1, #2, Decisions D1, D5, D6)
  - [x] 1.1 `firmware/common/proto/CMakeLists.txt`: `ocb_proto` becomes `STATIC` with the sources of Tasks 2 to 5, public include dir `include/` (so `#include "ocb_proto/frame.h"` works, same layout as `ocb_hw`). Delete `include/.gitkeep`. Set `C_STANDARD 99`, `C_EXTENSIONS OFF` on the target; own sources get `ocb_strict_warnings()` plus `-Wpedantic -Wshadow -Wconversion` (drop `-Wconversion` only if it forces ugly casts, and say so in the Debug Log). Link nothing: no `pico_*`, no `ocb_hw`. Verify `bridge` and `controller` still configure, build and link with `fw-release` and `fw-debug` without warnings (they already link `ocb_proto`; unused code is fine).
  - [x] 1.2 `include/ocb_proto/consts.h`: every wire constant, the only place a wire number is written in C (AD-2). Content per D1, D5, D6 below: `OCB_PROTO_VERSION` (plain `1`, no suffix, so `tools/check-proto-version.sh` can parse it), `OCB_MSG_*`, `OCB_NAK_*`, `OCB_EVT_*`, `OCB_MODE_*`, `OCB_HID_PROFILE_*`, flag bits, button bits, `OCB_HAT_NEUTRAL`, size constants, timing constants, `OCB_LINK_BAUD`. Plain `#define`s with comments naming the AD or doc section. No GPIO tokens in comments (pin grep rule 1 scans this tree).
  - [x] 1.3 Compile-time checks in `consts.h` or a `.c` file using the C99 negative-array idiom (no `_Static_assert`, the target is C99): `OCB_LINK_TIMEOUT_MS <= OCB_LINK_TIMEOUT_MAX_MS`; `OCB_HEARTBEAT_PERIOD_MS + ceil((OCB_MAX_ENCODED_FRAME + 1) * 10 * 1000 / OCB_LINK_BAUD) <= OCB_HEARTBEAT_MAX_GAP_MS` (20 + 6 = 26 <= 27, D6); `OCB_MAX_ENCODED_FRAME == OCB_COBS_MAX_ENCODED(OCB_MAX_RAW_FRAME)`.
  - [x] 1.4 `include/ocb_proto/ocb_proto.h`: umbrella header including the others. Headers use include guards, `extern "C"` is not needed (C only).

- [x] **Task 2: CRC-16 and COBS, from scratch** (AC: #1, Decision D9)
  - [x] 2.1 `crc16.h/.c`: `uint16_t ocb_crc16(const uint8_t *data, size_t len)` (and an incremental `ocb_crc16_update(uint16_t crc, ...)` if the frame code wants it). CRC-16/CCITT-FALSE = RevEng `CRC-16/IBM-3740`: width 16, poly `0x1021`, init `0xFFFF`, refin false, refout false, xorout `0x0000`, check `0x29B1` over ASCII `123456789`. Bitwise loop or a 256-entry `const uint16_t` table (`static const`, goes in flash on the RP2040; the table is 512 bytes, either is fine; pick the bitwise loop unless profiling in a later story says otherwise, because it needs no table to get wrong and the 2.5 jitter budget is dominated by USB, not by a 260-byte CRC).
  - [x] 2.2 `cobs.h/.c`: `size_t ocb_cobs_encode(const uint8_t *in, size_t in_len, uint8_t *out, size_t out_cap)` returning the encoded length, 0 when `out_cap` is too small (never writes past `out_cap`, never appends the delimiter: the caller adds `0x00`); `ocb_cobs_decode(const uint8_t *in, size_t in_len, uint8_t *out, size_t out_cap, size_t *out_len)` returning `OCB_COBS_OK`, or an error for: a `0x00` inside the input, a code byte that points past the end of the input, `out_cap` too small. Macro `OCB_COBS_MAX_ENCODED(n)` = `n + ceil(n / 254)`, minimum 1 for `n = 0` (Cheshire and Baker: overhead at most one byte per 254, rounded up). Rules to implement: block code `1..0xFE` = that many bytes follow, a zero is implied after them unless the block ends the input; code `0xFF` = 254 non-zero bytes follow and **no** zero is implied; empty input encodes to the single byte `01`.
  - [x] 2.3 Write both from the published descriptions, not from any pasted code; cite the paper and the RevEng entry in the file header comments (references below). No dynamic allocation, no recursion, no libc beyond `string.h` (`memcpy`).

- [x] **Task 3: Canonical state and `NEUTRAL`** (AC: #1, Decision D1)
  - [x] 3.1 `state.h/.c`: `typedef struct { uint16_t buttons; uint8_t hat, lx, ly, rx, ry, flags; } ocb_state;` (8 bytes on the wire, in this order, `buttons` little-endian). `#define OCB_NEUTRAL_INIT { 0u, OCB_HAT_NEUTRAL, 0x80u, 0x80u, 0x80u, 0x80u, 0u }`; `extern const ocb_state OCB_NEUTRAL;` or an inline accessor. `ocb_state_encode(const ocb_state*, uint8_t out[8])`, `ocb_state_decode(const uint8_t in[8], ocb_state*)`, `ocb_state_is_neutral(const ocb_state*)` (every field equal to `NEUTRAL`, `flags` included, so `NEUTRAL|END` is not neutral), `ocb_state_is_neutral_pose()` (ignores `flags`; the AD-8 rule "END only with a neutral state"), `ocb_state_has_end()`, `ocb_state_reserved_bits_ok()` (flags bits 1 to 7 zero, buttons are not validated). The codec does **not** validate or normalise field ranges (hat 9 to 255, button bits 14 and 15): the scheduler and the profiles decide (Stories 2.4, 2.3); say so in the header comment.
  - [x] 3.2 Neutral on the wire: `00 00 08 80 80 80 80 00`.

- [x] **Task 4: Frame encode/decode, receive splitter and counters** (AC: #1, Decisions D5, D10)
  - [x] 4.1 `frame.h/.c`: `ocb_frame_encode(uint8_t type, uint8_t seq, const uint8_t *body, size_t body_len, uint8_t *out, size_t out_cap)` writes the COBS-encoded frame **plus the trailing `0x00`** and returns the total length (<= `OCB_MAX_WIRE_FRAME`), 0 on `body_len > OCB_MAX_BODY` or a too-small `out_cap`. Layout before COBS: `type | seq | len | body[len] | crc16 LE`, the CRC covers `type, seq, len, body`. It does not check the type range (the bridge builds `0x40` frames, the app `0x01..0x3F`, the controller `0x80..0xFF`).
  - [x] 4.2 `ocb_frame_decode(ocb_rx_side side, const uint8_t *enc, size_t enc_len, ocb_frame *out)`: `enc` is one frame **without** its delimiter. `ocb_frame` = `{ uint8_t type, seq, len; uint8_t body[OCB_MAX_BODY]; }` (copy, so the input buffer can be reused). Checks in exactly the AD-3 order and returns the first failure:
    1. COBS decode fails → `OCB_ERR_COBS`;
    2. decoded length `< 5` (`OCB_MIN_RAW_FRAME`) or `!= 3 + len + 2` → `OCB_ERR_CRC` (a length inconsistency **is** a CRC error, AC1; the same code, not a new one);
    3. CRC mismatch → `OCB_ERR_CRC`;
    4. type outside the receiver's range → `OCB_ERR_RANGE`, with `side` = `OCB_RX_CONTROLLER` (accepts `0x01..0x7F`: app frames and bridge heartbeats) or `OCB_RX_APP` (accepts `0x80..0xFF`); type `0x00` is out of range for both;
    5. seq and semantics: **not here** (Reading notes). `enc_len == 0` returns `OCB_ERR_EMPTY` (an empty frame, D10).
  - [x] 4.3 Streaming splitter `ocb_rx` (struct with a fixed `uint8_t buf[OCB_MAX_ENCODED_FRAME]`, no heap): `ocb_rx_init(ocb_rx*, ocb_rx_side)`, `ocb_rx_push(ocb_rx*, uint8_t byte, ocb_frame *out)` returning `OCB_RX_NONE` (byte consumed, nothing ready), `OCB_RX_FRAME` (a valid frame is in `out`) or `OCB_RX_DROPPED` (a delimiter ended a run that was dropped and counted). Behaviour: `0x00` ends the current run; a zero-length run is an **empty frame** (counted, dropped); a run longer than `OCB_MAX_ENCODED_FRAME` non-zero bytes is **overlong**: counted **once**, the rest of the run is discarded up to and including the next `0x00`, then decoding resumes; every other run goes through `ocb_frame_decode`. A convenience `ocb_rx_push_bytes(...)` loop is optional.
  - [x] 4.4 Counters `ocb_rx_stats` (all `uint32_t`, saturating is not needed): `frames_ok`, `empty`, `cobs_errors`, `crc_errors`, `range_errors`, `overlong`. Each dropped run increments exactly one counter. `empty` also counts the resync `0x00` that opens a session (AD-7 "writes one `0x00`"): it is a diagnostic counter, **not an error**, and consumers must not alarm on it (D10).
  - [x] 4.5 Nothing in this task blocks, allocates or prints.

- [x] **Task 5: Message structs and body codecs** (AC: #1, Decisions D1, D10)
  - [x] 5.1 `msgs.h/.c`: one struct and one `_encode`/`_decode` pair per message, exactly the layouts of D1's table, fields named as in `docs/link-protocol.md`: `ocb_msg_hello`, `ocb_msg_info`, `ocb_msg_set_state`, `ocb_msg_queue_event`, `ocb_msg_start`, (STOP and PING have no body: encode/decode check `len == 0`), `ocb_msg_heartbeat`, `ocb_msg_status`, `ocb_msg_ack`, `ocb_msg_nak`, `ocb_msg_event`. Encode: `int ocb_msg_X_encode(const ocb_msg_X*, uint8_t *body, size_t cap)` returns the body length or a negative error; decode: `int ocb_msg_X_decode(const uint8_t *body, size_t len, ocb_msg_X*)` returns 0 or `OCB_ERR_BODY_LEN`.
  - [x] 5.2 **Never cast a byte buffer to a struct and never assume alignment or host endianness**: read and write every multi-byte field with explicit byte shifts (Cortex-M0+ hard-faults on unaligned access, and the host tests may run on a big-endian machine in principle). Little-endian everywhere.
  - [x] 5.3 Body length rules (D10): HELLO, INFO, HEARTBEAT, STATUS accept `len >=` the v1 size and ignore trailing bytes (their prefixes are frozen or carry debug counters); SET_STATE (8), QUEUE_EVENT (12), START (4), STOP (0), PING (0), ACK (1), NAK (3), EVENT (1) require the exact length. STATUS exposes the trailing bytes as `extra`/`extra_len` (pointer into the caller's body plus length) so debug counters can be read later without a codec change.
  - [x] 5.4 `ocb_msg_status.flags` and `ocb_msg_heartbeat.flags` are plain bytes; the bit macros are in `consts.h`; bits 2 to 7 of STATUS flags are not validated on receive ("the app ignores them", AD-19).

- [x] **Task 6: Vector files** (AC: #2, #3, Decision D2)
  - [x] 6.1 Create `firmware/common/proto/vectors/` with `README.md` (the format of D2 in a page, and "a wire change edits these first, AD-14"), and the seven files `constants.txt`, `cobs.txt`, `crc16.txt`, `frames.txt`, `messages.txt`, `streams.txt`, `state.txt`. Every file starts with comment lines then the header `proto_version: 01`. No tabs, `\n` line endings, ASCII only, lowercase hex.
  - [x] 6.2 **Generate the hex independently of the C code** with a throwaway pure-Python implementation kept in the scratchpad (not committed; Python is not a repo dependency of firmware): otherwise the C tests only prove the C code agrees with itself. Cross-check against three external anchors before trusting it: CRC of `123456789` = `29B1`; the Cheshire and Baker / Wikipedia COBS examples (D2 lists the small ones, verified during story creation); and that the C library then reproduces every vector. Write the hex into the files by hand-reviewed paste, with a comment above each case saying what it exercises.
  - [x] 6.3 Required cases (names are free, content is not):
    - `constants.txt`: every `MSG_*`, `NAK_*`, `EVT_*`, `MODE_*`, `HID_PROFILE_*`, flag and button bit, `HAT_NEUTRAL`, size constants, timing constants, `LINK_BAUD`, all from D1, D5, D6 (the file is the cross-language table the Rust codec asserts, Story 3.2).
    - `cobs.txt`: the empty input, `00`, `00 00`, `00 11 00`, `11 22 00 33`, `11 22 33 44`, `11 00 00 00` (small ones in D2), 254 non-zero bytes (`FF` block), 255 non-zero bytes (`FF` block then `02 FF`), 255 bytes `00` then `01..FE`, 254 non-zero bytes then `00` (`FF` block then `01 01`); decode-only failures: a code byte past the end (`05 11 22`), a zero inside the input (`03 11 00 22`), the empty input to decode (`.err: COBS`, an empty `.enc` is not a frame).
    - `crc16.txt`: `123456789` → `b1 29` (as on the wire, LE), the empty input → `ff ff`, `00`, `ff`, a 260-byte buffer (the max raw frame), and the HELLO header `01 00 01 01` → `64 d1`.
    - `frames.txt`: minimum frame (STOP, empty body), maximum frame (body 255 bytes of `00`, encoded length 261) **and** maximum frame with 255 non-zero body bytes (encoded length 262 = `OCB_MAX_ENCODED_FRAME`), a frame whose CRC contains a `00` byte, all-zero type/seq, bad CRC (last bit flipped), CRC bytes in the wrong order, `len` larger than the body, `len` smaller than the body, truncated frame, decoded length 0 (`01` → `OCB_ERR_CRC`), type `00`, type `80` as seen by the controller (range), type `01` and `40` as seen by the app (range), boundary types `7f` and `80` accepted by the right side.
    - `messages.txt`: at least one case per message with `fields`, `body`, `seq` and `wire` (D2), plus: QUEUE_EVENT with `t_us = 01020304` (proves the LE order: body starts `04 03 02 01`), QUEUE_EVENT with `NEUTRAL|END`, INFO with `queue_cap`, `app_timeout_ms`, `report_period_us` at `ffff`, STATUS base (11 bytes) and STATUS with 3 trailing debug bytes, HELLO with trailing bytes, HEARTBEAT with DTR set and cleared, and body-length errors (`err: BODY_LEN`) for SET_STATE with 7 bytes, QUEUE_EVENT with 13, STOP with 1, INFO with 13.
    - `streams.txt`: back-to-back frames (three frames, one buffer), a leading delimiter, an empty frame (`00`), two empties in a row, a bad CRC followed by a good frame (recovery), garbage then delimiter then a good frame (resync), exactly `OCB_MAX_ENCODED_FRAME` (262) non-zero bytes forming a valid max frame (accepted), 263 non-zero bytes (overlong, counted once, the **next** valid frame still decodes), an overlong run of 1000 bytes, a range error, a COBS error (all by `OCB_RX_CONTROLLER` unless a case sets `rx`).
    - `state.txt`: `NEUTRAL` (`00 00 08 80 80 80 80 00`), each of the 14 button bits alone (fields `buttons=0001` is Y; the bytes are little-endian, so `01 00 ...`), END flag on neutral, a diagonal hat, sticks at 0x00 and 0xFF (`ly 00` = up).
  - [x] 6.4 `firmware/tools/check-pins.sh` already excludes `common/proto/vectors/`; do not change that.

- [x] **Task 7: Test F.9 (vectors) and the library unit tests in ctest** (AC: #3, Decisions D3, D4, D7, D8)
  - [x] 7.1 `firmware/tests/test_vectors.c`: plain C, uses `ocb_test.h` (D7). The vectors directory comes from the compile definition `OCB_VECTORS_DIR` set in `tests/CMakeLists.txt` (D3). Parser rules in D2/D3. Runs the checks listed in D3. Registers as `add_test(NAME test_vectors ...)` (F.9).
  - [x] 7.2 `firmware/tests/test_proto_api.c` (F.11): behaviours the vectors cannot express: `ocb_cobs_encode` with a too-small buffer returns 0 and writes nothing past `out_cap` (use a guard-byte pattern; ASan catches the rest); `ocb_frame_encode` rejects `body_len = 256`; decode of every prefix of a valid frame never reads past `enc_len` (ASan); the counters add up (`ok + empty + cobs + crc + range + overlong` equals the number of delimiters seen); `ocb_state_is_neutral*` semantics; a randomised round trip (fixed-seed LCG, 10 000 frames, mixed lengths 0 to 255, zeros, 0xFF runs): `decode(encode(x)) == x`; a byte-at-a-time splitter run over a concatenation equals the one-shot decodes; `sizeof`/offset sanity of the 8-byte state encode.
  - [x] 7.3 `firmware/tests/CMakeLists.txt`: add both tests, link `ocb_proto`; sanitizers per D8 for the whole host-tests build; `ocb_strict_warnings()` on the new test sources.
  - [x] 7.4 `firmware/tools/check-proto-version.sh` (bash, POSIX `grep`/`sed` only, BSD and GNU compatible like `check-pins.sh`): extracts `#define OCB_PROTO_VERSION <n>` from `consts.h`, formats it as two lowercase hex digits, and fails (exit 1, message names the file) if any `vectors/*.txt` has a different or missing `proto_version:` header; exit 2 on a usage error; optional arguments `[HEADER] [VECTORS_DIR]` so a self-test can point it at copies. Register `proto_version_check` in ctest with a self-test: run on the real tree (pass), on a temp copy with the header changed (must fail), on a temp copy of one vector file with a wrong header (must fail), on a temp copy with a missing header (must fail) (D4). The temp copies are made in the build directory, never in the source tree.
  - [x] 7.5 `firmware/tools/check-proto-pure.sh` (same portability rules): scans `firmware/common/proto/` `*.c` and `*.h` and fails on any `#include` that is not on the allow-list: `<stdint.h> <stddef.h> <stdbool.h> <string.h> <limits.h>` and quoted includes starting with `ocb_proto/`; it also fails on the substrings `pico`, `hardware/`, `tusb`, `ocb_hw` and `ocb_version.h` inside an `#include` line. Self-test in ctest with one fixture under `firmware/tests/proto_pure_fixtures/bad/` (an `#include "pico/stdlib.h"` and an `#include "ocb_hw/pins.h"` file, each must be flagged) and `.../good/` (allowed includes only, must pass). Make sure the fixtures contain no GPIO token or `PICO_*_PIN` macro so `check-pins.sh` stays green.
  - [x] 7.6 Confirm the real tree has no GPIO tokens in the new sources and comments (`firmware/tools/check-pins.sh` passes).

- [x] **Task 8: CI** (AC: #3, Decision D4)
  - [x] 8.1 `.github/workflows/firmware.yml`: add two steps next to "Pin grep (AD-20)": "Proto version vs vectors (AD-14)" running `firmware/tools/check-proto-version.sh` and "Proto library is pico-sdk free (AD-2)" running `firmware/tools/check-proto-pure.sh`. Each is its own red step even though ctest runs them too (same pattern as the pin grep). No other change to the workflow; artifact name and UF2 paths stay as they are. `actionlint` clean if available.
  - [x] 8.2 Do not push. State in the Completion Notes that CI is unverified until the owner pushes; give the local command sequence that reproduces every CI step (Task 10.2).

- [x] **Task 9: Docs** (AC: #1, #2)
  - [x] 9.1 `docs/link-protocol.md`: add the assigned numeric values (D1) without changing any decision: a value column in the NAK and EVENT lists (turn the two prose lists into small tables), the `mode` and `hid_profile` values, STATUS and INFO field sizes (`last_seq u8, mode u8, abort_count u8, last_event u8, flags u8`; STATUS base body 11 bytes, INFO 14), the body-length rules (D10), the definition of an empty frame, `OCB_MAX_BODY`, `OCB_MAX_RAW_FRAME`, `OCB_MAX_ENCODED_FRAME` and why (D5), the hat values, and the timing constants table (D6). Keep the precedence note; replace "Numeric values are set in `ocb_proto` and the vectors" with "Numeric values: `firmware/common/proto/include/ocb_proto/consts.h`, mirrored in `vectors/constants.txt`; those two win on a disagreement". Do not touch the Watchdog, Reliability or Controller modes sections beyond adding a number where one was missing.
  - [x] 9.2 `docs/usb-controller-emulation.md`: only the sentence "numeric values set in `ocb_proto`" may gain the values `P1 = 1, P2 = 2, P3 = 3`; no other change.
  - [x] 9.3 `docs/test-plan.md`, "Firmware host tests": update row **F.9** to say what it now covers (vectors: COBS, CRC, frames, every message, streams, state, constants; headers all equal `OCB_PROTO_VERSION`; the file set is complete) and add row **F.11** (`ocb_proto` API tests: buffer limits, counters, randomised round trip, sanitizer-clean; related AD-2, AD-3). Leave every other row alone, including F.0 and H.0.
  - [x] 9.4 `firmware/common/README.md`: `proto/` no longer "an empty target"; list the headers and the vectors README; `firmware/README.md`: one line each for the new scripts and for `OCB_SANITIZE`; `firmware/THIRD_PARTY_NOTICES.md`: add a line "ocb_proto: COBS and CRC-16 implemented from the published descriptions, no third-party code (references in the source headers)", nothing else.

- [x] **Task 10: Local verification** (AC: #1 to #3)
  - [x] 10.1 With the Story 2.1 setup (`firmware/tools/install-tools.sh`, the exported `OCB_ARM_TOOLCHAIN` and `OCB_PICOTOOL_DIR`, the two submodule commands) run from `firmware/`: `cmake --preset host-tests && cmake --build --preset host-tests && ctest --preset host-tests` green (F.0 tests, `test_vectors`, `test_proto_api`, `proto_version_check`, `proto_pure_check`, ASan and UBSan active); `cmake --preset fw-release && cmake --build --preset fw-release` and the same for `fw-debug`: both `bridge` and `controller` build and link `ocb_proto` with zero warnings; `firmware/tools/check-pins.sh` passes; `shellcheck` clean on the new scripts; `actionlint` clean on `firmware.yml` when available.
  - [x] 10.2 Red checks, each reverted afterwards: change `OCB_PROTO_VERSION` to 2 → `proto_version_check` and `test_vectors` fail; flip one hex digit in `frames.txt` → `test_vectors` fails naming file, line and case; add `#include "pico/stdlib.h"` to a proto source → `check-proto-pure.sh` fails (and the host build fails too); change one NAK value in `consts.h` → `test_vectors` fails on the constants file.
  - [x] 10.3 Record in the Completion Notes: the exact commands, the ctest summary line, the `ocb_proto` code size in the release ELF (`arm-none-eabi-size` of the archive objects, informational), whether ASan/UBSan ran on macOS clang, and that CI (ubuntu gcc with sanitizers) is unverified until pushed.

### Review Follow-ups (AI)

- [x] [AI-Review][High] AD-3 order: `ocb_cobs_decode` stopped at the first output overflow (`SPACE`, mapped to `OCB_ERR_CRC`) before validating the rest of the input, so `01`x261 + `05` and `01`x261 + `00` gave CRC instead of COBS. Fixed: the decoder validates the whole input and returns `SPACE` only for a well-formed input; vectors `cobs_overflow_truncated`, `cobs_overflow_zero` (COBS) and `decoded_too_long` (CRC) added; `frame.c` comment updated (`common/proto/cobs.c`, `frame.c`, `frames.txt`)
- [x] [AI-Review][Low] `OCB_COBS_MAX_ENCODED(n)` evaluates `n` more than once: documented in `cobs.h`
- [x] [AI-Review][Low] `ocb_msg_status_encode` silently dropped `extra_len` when `extra == NULL`: now returns `OCB_ERR_BUF`, documented in `msgs.h`, tested in F.11
- [x] [AI-Review][Low] `frame.h`: documented that `ocb_frame_decode` leaves `*out` untouched on error and that `out` must be non-NULL (and `out_len` in `cobs.h`); F.9 now checks `*out` is untouched on every error case
- [x] [AI-Review][Low] Direct `frames.txt` vectors for `EMPTY` (`empty_wire`) and `OVERLONG` (`overlong_wire`); the two error names added to the format (`vectors/README.md`)

## Dev Notes

### What this story is

The wire format becomes real, once, in C, and is pinned by language-neutral vectors that the Rust codec of Story 3.2 will read unchanged. It writes no USB, UART, multicore, scheduler, session or watchdog code: it is a pure library plus its tests. Everything later builds on it: the controller receive path and the go-back-N logic (2.6), the core (2.4), the bridge framer and heartbeat builder (3.1), and the Rust `ocb-core` codec (3.2). If a number or a rule here is wrong, all of them inherit it, so the values are fixed here with rationale (D1 to D10) and checked in two languages.

### Consumers (what they need from this story)

- **Story 2.4 (core)**: `ocb_state`, `OCB_NEUTRAL`, `ocb_state_is_neutral_pose()`, `ocb_state_has_end()`, `ocb_state_reserved_bits_ok()`, the `OCB_MODE_*` values, `OCB_T_US_LIMIT` (2^31), message structs for `SET_STATE`, `QUEUE_EVENT`, `START`. The core receives already-decoded structs; it never touches COBS.
- **Story 2.6 (controller link)**: `ocb_rx` with `OCB_RX_CONTROLLER` fed byte by byte from the UART IRQ ring (the splitter does no I/O), `ocb_rx_stats` for the debug counters, `ocb_frame_encode` for ACK/NAK/EVENT/STATUS, `ocb_msg_*` encoders, `OCB_NAK_*`, `OCB_EVT_*`, `OCB_ACK_WINDOW`, `OCB_STATUS_*`. The seq arithmetic (stale window of 64, wrap) is **2.6's**, not here.
- **Story 2.7 (watchdog)**: `OCB_LINK_TIMEOUT_MS`, `OCB_LINK_TIMEOUT_MAX_MS`, the `HEARTBEAT` struct with `OCB_HB_FLAG_DTR`.
- **Story 3.1 (bridge)**: `OCB_MAX_ENCODED_FRAME`, `OCB_MAX_WIRE_FRAME` (buffer sizes), `ocb_frame_encode` for `HEARTBEAT`, `OCB_HEARTBEAT_PERIOD_MS`, `OCB_HEARTBEAT_MAX_GAP_MS`, `OCB_LINK_BAUD`. The bridge never decodes (AD-4); it only needs the constants and the encoder.
- **Story 3.2 (Rust `ocb-core`)**: the vector files, byte for byte. It reads `proto_version`, every `case.field` line and `constants.txt`. This is why D2 is line-oriented with no escaping and no quoting.
- **Story 3.3 (sans-IO link)**: `NAK`/`EVENT` values and the unknown-value rule (AD-19) through the vectors' constants.

### Decisions (made autonomously, 2026-11-22; the owner can overrule any of them)

Each was researched first (see "Latest tech information") and the recommended option picked. Where the architecture or `docs/link-protocol.md` already fixed a value, the decision only records it.

**D1. Numeric values and where they live: one header, `ocb_proto/consts.h`; mirrored in `docs/link-protocol.md` and `vectors/constants.txt`; the vectors win on a conflict.**
All values are chosen so that `0x00` is never a valid value (a zeroed buffer or an uninitialised field can never look like a real reason, event or type) and so that classes are decidable by range.

| Group | Constants (value) | Why |
|---|---|---|
| Message types (AD-3 ranges; `OCB_MSG_*`) | `HELLO 0x01`, `SET_STATE 0x02`, `QUEUE_EVENT 0x03`, `START 0x04`, `STOP 0x05`, `PING 0x06`; `HEARTBEAT 0x40`; `INFO 0x81`, `STATUS 0x83`, `ACK 0x84`, `NAK 0x85`, `EVENT 0x86` | exactly the codes already in `docs/link-protocol.md`; `0x82` is left unassigned on purpose (reserved; it would be `UNKNOWN_TYPE` territory) |
| NAK reasons (`OCB_NAK_*`) transport | `CRC 0x01`, `SEQ 0x02`, `FULL 0x03` | transport class = `0x01..0x0F` |
| NAK reasons semantic | `VERSION 0x10`, `BUSY 0x11`, `ORDER 0x12`, `BAD_STATE 0x13`, `UNKNOWN_TYPE 0x14`, `NO_LINK 0x15`, `NOT_READY 0x16`, `ABORTED 0x17` | semantic class = `0x10..0x7F`; a macro `OCB_NAK_IS_TRANSPORT(r)`/`OCB_NAK_IS_SEMANTIC(r)` gives the AD-6 split (transport never advances `expected`, semantic does) in one place; the order follows the order in AD-6; room is left to add reasons without renumbering. An **unknown** reason (>= `0x18`, or `0x04..0x0F`) is logged and never fatal (AD-19); Story 3.3 decides its effect on rewind logic and this story gives it no class |
| EVENT codes (`OCB_EVT_*`) | `NONE 0x00` (only in `STATUS.last_event`: no event yet), `PLAYBACK_DONE 0x01`, `LINK_LOST 0x02`, `LATE 0x03`, `UNDERRUN 0x04`, `USB_SUSPENDED 0x05` | the same numbers serve `STATUS.last_event` and `EVENT.code`, so the app maps both with one table; order as in AD-5/AD-8 |
| `mode` (`OCB_MODE_*`) | `IDLE 0x00`, `DIRECT 0x01`, `PLAYING 0x02` | here `0` is legitimately a value: IDLE is the power-up state, and the app derives the mode from STATUS (AD-19) |
| `hid_profile` (`OCB_HID_PROFILE_*`) | `UNKNOWN 0x00`, `P1 0x01`, `P2 0x02`, `P3 0x03` | one id per profile of `docs/usb-controller-emulation.md` (P1 generic HID, P2 Pro Controller wired, P3 Switch 2 Pro Controller); the number equals the profile's name |
| `HEARTBEAT.flags` | `OCB_HB_FLAG_DTR 0x01` | AD-4 |
| `STATUS.flags` | `OCB_STATUS_LINK_OK 0x01`, `OCB_STATUS_CONSOLE_READY 0x02`, `OCB_STATUS_RESERVED_MASK 0xFC` | AD-19 |
| state `flags` | `OCB_STATE_FLAG_END 0x01`, `OCB_STATE_RESERVED_MASK 0xFE` | AD-8, AD-10 |
| buttons (`OCB_BTN_*`) | `Y 1<<0, B 1<<1, A 1<<2, X 1<<3, L 1<<4, R 1<<5, ZL 1<<6, ZR 1<<7, MINUS 1<<8, PLUS 1<<9, L3 1<<10, R3 1<<11, HOME 1<<12, CAPTURE 1<<13` | AD-10 order |
| hat | `OCB_HAT_UP 0 ... OCB_HAT_UP_LEFT 7` clockwise as in `docs/usb-controller-emulation.md` (0 up, 1 up-right, 2 right, 3 down-right, 4 down, 5 down-left, 6 left, 7 up-left), `OCB_HAT_NEUTRAL 8` | AD-10 |
| `PROTO_VERSION` | `1` (`OCB_PROTO_VERSION`) | the first shipped wire format: INFO v1; there is no earlier version to be compatible with |

Message bodies (all little-endian), sizes checked by the vectors:

| Message | Body | Bytes | Length rule (D10) |
|---|---|---|---|
| HELLO | `proto_ver u8` | 1 | `>= 1`, rest ignored |
| INFO | `proto_ver u8, fw_ver[3], bridge_ver[3], hid_profile u8, queue_cap u16, app_timeout_ms u16, report_period_us u16` | 14 | `>= 14` |
| SET_STATE | `state[8]` | 8 | exact |
| QUEUE_EVENT | `t_us u32, state[8]` | 12 | exact |
| START | `lead_us u32` | 4 | exact |
| STOP, PING | none | 0 | exact |
| HEARTBEAT | `bridge_fw_ver[3], flags u8` | 4 | `>= 4` |
| STATUS | `free u16, now_us u32, last_seq u8, mode u8, abort_count u8, last_event u8, flags u8` (+ debug counters) | 11 (+ n) | `>= 11` |
| ACK | `seq u8` | 1 | exact |
| NAK | `reason u8, expected u8, got u8` | 3 | exact |
| EVENT | `code u8` | 1 | exact |

`abort_count` is a u8 that wraps (a counter of aborts, not a measurement); `last_seq` is `expected - 1` (AD-19). Why a single header rather than an X-macro table or a generated file: the AD says wire constants exist only in `ocb_proto`, and the Rust side cannot include a C header; the three artefacts (header, docs, vectors) are kept equal by F.9 (header vs vectors, both directions, D3) and by review (docs). Rejected: generating the header from the vectors (a build step and a script dependency in a story whose point is that the vectors are an independent check); generating the vectors from the header (the check would be circular).

**D2. Vector file format: line-oriented `key: value`, one file per area, `case.field` keys, hex only, header `proto_version: 01`.**
Rules (also written into `vectors/README.md`):
1. UTF-8/ASCII, `\n` lines. A line is blank, a **comment** (first non-blank character is `#`), or `key: value`. Split at the **first** `:`; trim spaces around key and value. There is no escaping, no quoting, no multi-line value, no trailing comments (a `#` inside a value is an error).
2. The **first** `key: value` line of every file is the header `proto_version: 01` (a u8 as two hex digits, like everything else). It must equal `OCB_PROTO_VERSION` (D4).
3. **Values are hex bytes** in wire order: pairs of hex digits (`0-9a-f`, upper case also accepted), optional single spaces between bytes for readability, empty value = zero bytes (`raw:` with nothing after the colon). Exceptions, stated per field below: error names (`err`), side names (`rx`), and `fields`.
4. A **case** is the set of lines whose key is `<case>.<field>` with the same `<case>`; the fields of one case are consecutive; case names are `[a-z0-9_]+`, unique per file. Parsers must reject unknown fields, duplicate fields, and cases missing a required field (typo guard), and the test asserts a minimum case count per file so an empty or misparsed file cannot pass vacuously.
5. **Error names** are exactly the C names without prefix: `COBS`, `CRC`, `RANGE`, `BODY_LEN`. `rx` is `CONTROLLER` or `APP` (default `CONTROLLER` when absent).
6. `fields` is a space-separated list of `name=value`. A number is written as **big-endian hex of its full width, as in C** (`t_us=000003e8`, `queue_cap=0200`; the wire order is little-endian, which is the point of the check); a byte array is hex in wire order (`fw_ver=010203`). Field names are those of `docs/link-protocol.md`; a nested state is `state=<16 hex digits>` in wire order.

Per area (required fields):

| File | Case fields | What a test does |
|---|---|---|
| `constants.txt` | `NAME: <big-endian hex number>` lines, **no** dotted keys (the one file where hex is a number, not bytes; e.g. `LINK_BAUD: 070800`, `NAK_CRC: 01`, `OCB_` and group prefixes dropped) | every line must match a macro of the same name and value, and every macro on the C side's list must be present (both directions) |
| `cobs.txt` | `raw`, `enc` (no delimiter), or `enc` + `err` for decode-only failures | encode(raw) == enc; decode(enc) == raw; or decode(enc) fails with `err` |
| `crc16.txt` | `data`, `crc` (**2 bytes as on the wire, little-endian**) | `crc16(data)` as LE bytes equals `crc` |
| `frames.txt` | `payload` (`type seq body`, i.e. before len/CRC/COBS) + `wire` (COBS-encoded, no delimiter); or `wire` + `err` (+ `rx`) | encode(type, seq, body) == wire, decode(wire) == payload; or decode(wire, rx) fails with `err`; ok cases also decode with both sides when the type is in both ranges (none are) |
| `messages.txt` | `type`, `seq`, `fields`, `body`, `wire`; or `type`, `body`, `err` (`BODY_LEN`) | encode(fields) == body; decode(body) gives fields; encode_frame(type, seq, body) == wire; or decode(body) fails with `err` |
| `streams.txt` | `in` (bytes incl. delimiters), optional `rx`, `frames` (`;`-separated payload hex, in order, may be empty), `stats` (`ok=1 empty=0 cobs=0 crc=0 range=0 overlong=0`, all six keys, in that order) | push `in` byte by byte; collected frames and final stats must equal; then push the same `in` in one call if offered, same result |
| `state.txt` | `bytes` (8 bytes), `fields` (`buttons=0001 hat=08 lx=80 ly=80 rx=80 ry=80 flags=00`) | decode(bytes) == fields; encode(fields) == bytes; the `neutral` case also equals `OCB_NEUTRAL` |

Example snippet (every hex value below was computed with an independent implementation during story creation; the CRC/COBS algorithms were checked against the RevEng check value and the Wikipedia COBS table):

```text
# firmware/common/proto/vectors/messages.txt
# One case per message. type, seq, fields, body, wire (COBS, no delimiter).
proto_version: 01

# HELLO, seq 0: crc16("01 00 01 01") = d164, sent LE as "64 d1"
hello.type: 01
hello.seq: 00
hello.fields: proto_ver=01
hello.body: 01
hello.wire: 02 01 05 01 01 64 d1

# STOP, seq 0, empty body
stop.type: 05
stop.seq: 00
stop.fields:
stop.body:
stop.wire: 02 05 01 03 6c 27

# QUEUE_EVENT, seq 2, t_us = 1000 us, state = NEUTRAL (little-endian t_us = e8 03 00 00)
queue_event_neutral.type: 03
queue_event_neutral.seq: 02
queue_event_neutral.fields: t_us=000003e8 state=0000088080808000
queue_event_neutral.body: e8 03 00 00 00 00 08 80 80 80 80 00
queue_event_neutral.wire: 06 03 02 0c e8 03 01 01 01 06 08 80 80 80 80 03 a8 fd

# SET_STATE with the wrong body length: a semantic error, not a frame error
set_state_short.type: 02
set_state_short.body: 00 00 08 80 80 80 80
set_state_short.err: BODY_LEN
```

```text
# firmware/common/proto/vectors/cobs.txt
proto_version: 01
empty.raw:
empty.enc: 01
two_zeros.raw: 00 00
two_zeros.enc: 01 01 01
paper_ex4.raw: 11 22 00 33
paper_ex4.enc: 03 11 22 02 33
code_past_end.enc: 05 11 22
code_past_end.err: COBS
```

```text
# firmware/common/proto/vectors/streams.txt
proto_version: 01
# a leading delimiter (session open, AD-7) is one counted empty frame, then a good frame
leading_delimiter.in: 00 02 05 01 03 6c 27 00
leading_delimiter.frames: 05 00
leading_delimiter.stats: ok=1 empty=1 cobs=0 crc=0 range=0 overlong=0
```

The `hello`, `stop` and `queue_event_neutral` wire values and the CRC bytes above come from the independent generator and can be used as first anchors. The dev still regenerates every other value with that generator; if a value in this story ever disagrees with the generator plus the external anchors (CRC check `29B1`, the paper's COBS examples), the generator wins and the story is corrected in the Debug Log.

Why this shape: a parser is "read lines, skip `#` and blank, split at the first `:`, group by the text before the last `.`": about 30 lines in C and in Rust, no dependency, no JSON/TOML crate in firmware tests (the repo's firmware has no parser dependency and `cargo-deny` licence review is avoided for the vectors). It keeps AD-14's "one text file per area, one `name: hex` line each" literally true: each line is `name: hex`, with `name` = `case.field`. Rejected: JSON (needs a library in C or a hand parser with escaping rules; hex in strings anyway); TOML (extra crate for Rust, nothing for C); one-line-per-case with `->` separators (breaks "one `name: hex` per line" and makes error expectations awkward); CSV (positional fields, error-prone).

**D3. F.9 loads vectors with a small C parser inside `test_vectors.c`, the directory passed as a compile definition.**
`tests/CMakeLists.txt`: `target_compile_definitions(test_vectors PRIVATE OCB_VECTORS_DIR="${OCB_FW_DIR}/common/proto/vectors")` (absolute path, so ctest can run from any working directory; the test also accepts the directory as `argv[1]` for ad-hoc runs). The test: (1) opens each of the seven expected files by name (a missing file fails; a **new** file that is not in the list also fails, via a directory scan avoided for portability: the list is the single array `kFiles[]` in the test and the README says "adding an area means editing it"); (2) reads lines with `fgets` into a 4096-byte buffer and fails (not truncates) on a longer line (the longest legitimate value is a 263-byte frame as hex with spaces, about 800 characters); (3) parses per D2, reporting `file:line: message` on any malformed line and continuing to the end so one run shows every problem; (4) runs the per-area checks of D2's table; (5) prints `test_vectors: N cases in M files ok` and enforces minimum case counts (`cobs >= 14`, `crc16 >= 5`, `frames >= 18`, `messages >= 18`, `streams >= 10`, `state >= 17`; adjust only upward). Hex decoding and the line splitter are the only helpers; no `scanf` family on untrusted widths, no `strtok` (not re-entrant, mutates), no dynamic allocation needed (fixed buffers sized by `OCB_MAX_*`; a case holds at most a few arrays). Rejected: pre-processing the vectors into a generated C header at configure time (a second representation that can drift; the vectors would no longer be read as shipped); a vendored parser library (a dependency for 30 lines).

**D4. CI fails on a `PROTO_VERSION` mismatch twice, by two independent mechanisms.**
(a) `test_vectors` asserts that each file's header equals `OCB_PROTO_VERSION` and that all seven headers are present; that is the ctest gate. (b) `firmware/tools/check-proto-version.sh` runs as its own red step in `firmware.yml` (before the build matrix is needed, it needs only `grep`/`sed`), reading the header text and the vector headers without compiling anything; ctest also runs it with a self-test so the script itself is a tested gate (the Story 2.1 `check-pins.sh` pattern). Why both: (a) proves the compiled constant and parse path; (b) is a textual check that Story 3.2's app job can reuse unchanged for the Rust constant and that survives a build break. AD-14 says CI checks that **both codebases'** `PROTO_VERSION` equal the header: the Rust half is added by Story 3.2 (its `PROTO_VERSION` test is already in that story's AC); this story does not build that hook: Story 3.2 adds its own Rust test and may extend the script.

**D5. `OCB_MAX_ENCODED_FRAME = 262`, derived, not chosen.**
`len` is a u8, so the body is at most `OCB_MAX_BODY = 255`. Raw frame = `type, seq, len` (3) + body (255) + CRC (2) = `OCB_MAX_RAW_FRAME = 260`. COBS overhead is at most `ceil(n / 254)` bytes, so `260 + ceil(260 / 254) = 260 + 2 = 262` = `OCB_MAX_ENCODED_FRAME`, the number of non-delimiter bytes of the largest legal frame (confirmed by the independent generator: a 255-byte non-zero body gives 262 encoded bytes, an all-zero body 261). With the closing delimiter a frame occupies `OCB_MAX_WIRE_FRAME = 263` bytes on the wire. The AD-4 rule "a run longer than `OCB_MAX_ENCODED_FRAME` bytes without a delimiter is dropped" therefore means **more than 262 non-zero bytes**, and `ocb_rx` and the vectors use exactly that (a run of 262 is accepted, 263 is overlong). Also `OCB_MIN_RAW_FRAME = 5`. Why not a smaller body limit (the real messages are at most 14 bytes plus debug counters): the `len` field is already a byte, a smaller cap would be an arbitrary second rule to freeze, and the largest frame costs 263 bytes of buffer per direction, trivial on the RP2040. The cost shows up elsewhere and is checked in Task 1.3: at 460800 baud (10 bits per byte) 263 bytes take 5.7 ms, so a heartbeat due while a maximum app frame is in flight still meets `20 ms + 6 ms = 26 ms <= 27 ms` (AD-4's 27 ms bound has 1 ms of slack); the bridge story relies on this.

**D6. Link timing constants (named, in `consts.h`, mirrored in `constants.txt`).**
All from the ADs and `docs/link-protocol.md`; nothing new is invented:

| Constant | Value | Source |
|---|---|---|
| `OCB_LINK_BAUD` | 460800 | AD-4, link doc (a build constant; 921600 stays an option for Story 3.4's soak) |
| `OCB_HEARTBEAT_PERIOD_MS` | 20 | AD-4 |
| `OCB_HEARTBEAT_MAX_GAP_MS` | 27 | AD-4, F.7 |
| `OCB_LINK_TIMEOUT_MS` | 80 | AD-5 default |
| `OCB_LINK_TIMEOUT_MAX_MS` | 100 | AD-5 upper bound, NFR-05 (compile-time check `OCB_LINK_TIMEOUT_MS <= OCB_LINK_TIMEOUT_MAX_MS`) |
| `OCB_ACK_TIMEOUT_MS` | 50 | AD-6 |
| `OCB_ACK_WINDOW` | 64 | AD-6 (max unACKed frames, and the stale-seq window size) |
| `OCB_HELLO_RESEND_MS` | 100 | AD-7 |
| `OCB_APP_TIMEOUT_MS_DEFAULT` | 1000 | AD-5, until W.1 |
| `OCB_APP_PING_DIVISOR` | 4 | AD-7 (`PING` after `app_timeout_ms / 4` idle) |
| `OCB_STATUS_PERIOD_MS` | 50 | AD-19 |
| `OCB_STATUS_MIN_GAP_MS` | 10 | AD-19 (coalescing) |
| `OCB_CONSOLE_POLL_STALL_MS` | 100 | AD-19 (no IN report taken for more than this clears `console_ready`) |
| `OCB_QUEUE_CAP_DEFAULT` | 512 | AD-6/AD-7 default of `INFO.queue_cap` (never hard-coded in the app) |
| `OCB_T_US_LIMIT` | `0x80000000` | AD-8 (`t_us` < 2^31) |
| `OCB_STOP_NEUTRAL_MS` | 100 | NFR-05 / link doc ("after a `STOP`: neutral within 100 ms") |

Units are in the name (`_MS`); all are plain integers (no `UL`/`u` suffix problems for the shell and the Rust parser: `constants.txt` carries them as hex numbers). Rejected: enums for the values (the macros are used in preprocessor conditions and static checks), floating constants.

**D7. Test framework: none. Plain C with a 25-line shared header `tests/ocb_test.h` (our own, MIT).**
Story 2.1's F.0 tests use plain `assert`-style code with a non-zero exit (no framework), and its notes left the choice to this story. Unity (MIT) or Greatest (ISC) would be licence-compatible (NFR-07) but add a vendored dependency and notices for what is here a few hundred checks of `memcmp`; the vector test needs custom reporting (`file:line: case: field`) that a framework's assert macros do not give for free; ctest already provides discovery, selection, `--output-on-failure` and the CI gate. `ocb_test.h` holds `OCB_CHECK(cond, ...)` (prints `file:line` and a printf-style message, counts failures, continues), `OCB_CHECK_MEM(a, b, n, what)` (hex dump of both on mismatch) and a `return ocb_test_failures() ? 1 : 0` helper. Existing tests (`test_pins.c`, `test_version.c`) are not rewritten. If a later story needs mocking or fixtures at scale (the core's fake clock tests, 2.4), it can revisit this with evidence.

**D8. ASan and UBSan are on in the `host-tests` preset (CMake option `OCB_SANITIZE`, default ON for host tests only).**
Why: `ocb_proto` is exactly the code where a one-byte overrun in COBS or the splitter would corrupt a frame silently on a 2 MB-flash board with no debugger attached; sanitizers turn it into a failing test for free, and the randomised round trip and every-prefix tests in `test_proto_api` are written to exploit them. Supported: AddressSanitizer and UBSan ship with Apple clang (macOS, the owner's machine) and with GCC on `ubuntu-24.04` (libasan/libubsan come with the gcc package). Implementation: in `firmware/CMakeLists.txt`, `if(OCB_HOST_TESTS AND OCB_SANITIZE)` add `-fsanitize=address,undefined -fno-sanitize-recover=undefined -fno-omit-frame-pointer` to the compile and link flags (`CMAKE_C_FLAGS`, `CMAKE_EXE_LINKER_FLAGS`) before the subdirectories; the preset sets `"OCB_SANITIZE": "ON"`; firmware presets never see it (no effect on the Arm builds). Known caveats the dev handles: LeakSanitizer is not available on macOS arm64, so set `ASAN_OPTIONS=detect_leaks=1` only on Linux, through the ctest test property `ENVIRONMENT` guarded by `CMAKE_SYSTEM_NAME STREQUAL "Linux"` (nothing allocates anyway); the pin-grep self-test and other bash tests are unaffected; if a sanitizer runtime is missing on a machine, `-DOCB_SANITIZE=OFF` is the documented escape hatch (README). CI uses the same preset, so the first push proves gcc with sanitizers; until then it is unverified, and a gcc-only warning or sanitizer difference would be a CI finding to fix, not a reason to drop the option.

**D9. COBS and CRC are written from scratch (clean-room from the published descriptions), no third-party code.**
Both algorithms are 20 to 30 lines and fully specified publicly (the Cheshire and Baker paper; the RevEng catalogue entry gives the CRC parameters and the check value). Copying a library would bring an attribution and licence-compatibility question (Wikipedia's example C is CC BY-SA text; embedded COBS libraries vary in licence) for no saving, and the implementation must obey this repo's rules anyway (no heap, explicit bounds, no unaligned casts). Correctness is established by external anchors instead of by the source of the code: the CRC check value `0x29B1`, the paper's published encodings, and vectors generated by a separate Python implementation (Task 6.2). The source headers cite the paper and the catalogue as references (not as origin of code). `THIRD_PARTY_NOTICES.md` gets one line saying so (Task 9.4). Rejected: vendoring a COBS C library (licence review plus API mismatch with the no-delimiter, bounded-buffer interface we want); a table-driven CRC for now (see Task 2.1).

**D10. Decoder semantics the AD leaves open (decided here so Rust and C agree).**
1. **Empty frame** = a run of zero bytes between two delimiters (`00 00`, or a leading `00`). Counted in `empty`, dropped. A run whose COBS decoding is zero bytes (`01`) is **not** "empty": it is a too-short frame, i.e. `ERR_CRC` by the length rule. The distinction keeps `empty` meaning "nothing but a delimiter".
2. The session-opening `0x00` (AD-7) and idle-line `0x00` repeats count as `empty`; `empty` is informational only and never triggers a NAK or an alarm.
3. A COBS failure is its own error and counter (`cobs_errors`), not folded into `crc_errors`; AD-3 only says that a *length* inconsistency is treated as a CRC error. How the controller maps `cobs`/`crc` to `NAK CRC` is Story 2.6's rule (both are transport-level corruption).
4. Out-of-range types (including `0x00`) are counted in `range_errors` and dropped **after** the CRC passed (the AD-3 order), so line noise is classed as CRC, not as a range error, and no NAK `UNKNOWN_TYPE` is produced for it. A type that is in range but unassigned (for example `0x07` or `0x82`) is **not** a decoder error: the frame decodes, and the semantic layer answers `NAK UNKNOWN_TYPE` (AD-6).
5. Body length rules per message (D1 table): frozen-prefix and extensible messages accept longer bodies (so a newer app's `HELLO` still reaches the version check and gets `NAK VERSION` or an INFO), fixed messages are exact. A length mismatch is `OCB_ERR_BODY_LEN`, returned by the message decoder, a semantic error: the consumer decides the NAK (`BAD_STATE`-class reply is Story 2.4's call; this story only reports it).
6. The decoder never trusts `len` before the length-consistency check and never reads past the supplied buffer.

### Developer guardrails

- English everywhere in the repo; chat with the owner is Italian.
- **Pure C99, no pico-sdk, no `ocb_hw`, no `ocb_version.h`, nothing platform specific** in `common/proto/`; allowed includes are the five standard headers of `check-proto-pure.sh` and `ocb_proto/*`. No `printf`, no `malloc`, no `assert`, no global mutable state (the splitter keeps its state in its struct, so two instances can coexist: bridge heartbeat builder and controller RX in the same host test).
- **Unaligned access and endianness**: byte-wise loads and stores only (Task 5.2).
- **Never block, never allocate**, never loop unboundedly on input data: every loop is bounded by a length argument or an `OCB_MAX_*` constant.
- **No GPIO tokens**: `check-pins.sh` scans `common/proto/` (only the `vectors/` subtree is excluded). Words like "GPIO" or `gp` followed by digits must not appear, comments included.
- **Do not commit or push**; do not claim CI green for what never ran.
- **No UF2, ELF or any binary committed**; no 2wiCC code (NFR-07, ADR-004); no game screenshots; never reference anything under `ux-designs/*/imports/`; do not touch `_bmad/`, `bmad-*` or `gds-*` skill folders; do not modify `~/Documents/org/`.
- Scratch work (the independent vector generator, temp copies for red checks) goes in a scratch directory outside the repo or in the build directory; leave no `*.temp.*` files.
- Naming: C prefix `ocb_`, `snake_case.c`, macros `OCB_*`, messages `OCB_MSG_<NAME>`, `OCB_EVT_<NAME>`, `OCB_NAK_<NAME>` (architecture convention table).
- A wire change edits the **vectors first** (AD-14). In this story there is nothing to change after the fact, but if a vector and the doc disagree while working, the vectors and `consts.h` win and the doc is fixed.
- Memory-backed preferences: ask the owner one decision at a time and keep it short; research before asking. None of this story's decisions needs the owner now.

### Architecture compliance

- **AD-2**: wire constants exist only in `ocb_proto` (`consts.h`); no wire number appears in `bridge/` or `controller/`; `ocb_proto` is pure C99 and builds in `host-tests` without pico-sdk (the preset never includes `pico_sdk_init.cmake`, which is itself a proof, and `check-proto-pure.sh` is the explicit gate).
- **AD-3**: receive order COBS, length consistency (as a CRC error), CRC, type range implemented in `ocb_frame_decode` in that order; seq and semantics are the consumers'. Empty frames and out-of-range types are counted and dropped. Type ranges per receiver side.
- **AD-4**: the max-run rule uses `OCB_MAX_ENCODED_FRAME`; the heartbeat frame is a normal COBS+CRC frame built by `ocb_frame_encode` with body `bridge_fw_ver[3], flags`; the gap budget is checked at compile time (D5).
- **AD-5, AD-6, AD-7, AD-19**: constants and NAK/EVENT/STATUS/INFO layouts only; the behaviour is in later stories. The unknown-value rule (AD-19) is respected by giving unknown NAK reasons and EVENT codes no meaning in the codec (they decode as plain bytes).
- **AD-10**: canonical state, P1 bit order, hat 0 to 7 with 8 neutral, sticks centred at 0x80, `ly 0x00` = up, `NEUTRAL` defined once in C and once (Story 3.2) in Rust, and checked equal through `state.txt`.
- **AD-14**: `firmware/common/proto/vectors/` created with one text file per area and the `proto_version` header; F.9 and the CI scripts enforce it; vectors take precedence over the docs.
- **AD-21 / NFR-07**: own code under MIT, no third-party code in `ocb_proto` (D9). **NFR-06**: nothing here has a network path.
- Respected by omission: AD-8 mode logic, AD-9 mask, AD-11 core split, AD-12 profile interface, AD-13 USB identity, AD-20 pins (no pin is touched; no `ocb_hw` include).

### Library and framework requirements

- None. Standard C headers only (`stdint.h`, `stddef.h`, `stdbool.h`, `string.h`, `limits.h`). No test framework (D7). Host tests use the system C compiler (Apple clang or gcc) with `-fsanitize=address,undefined` (D8). `bash`, `grep`, `sed`, `cmake`, `ninja` for the scripts and build, as in Story 2.1. Python 3 is used **only** as a scratch tool to generate vector hex independently (not committed, not a repo dependency).
- pico-sdk 2.3.1 and the Arm GNU Toolchain 15.2.Rel1 are involved only because `fw-release` and `fw-debug` must still compile `ocb_proto` warning-free as part of the existing executables.

### File structure requirements

All paths under `firmware/` unless noted.

| File | Action | Notes |
|---|---|---|
| `common/proto/CMakeLists.txt` | UPDATE | currently an `INTERFACE` library with an include dir, no sources; becomes `STATIC` (Task 1.1); keep the include dir and the "never includes pico-sdk" comment |
| `common/proto/include/.gitkeep` | DELETE | replaced by real headers |
| `common/proto/include/ocb_proto/{consts,crc16,cobs,state,msgs,frame,ocb_proto}.h` | NEW | Tasks 1 to 5 |
| `common/proto/{crc16,cobs,state,msgs,frame,rx}.c` (flat, next to the CMake file, as `ocb_hw` keeps `led.c`) | NEW | `rx.c` holds the splitter and counters |
| `common/proto/vectors/{README.md,constants.txt,cobs.txt,crc16.txt,frames.txt,messages.txt,streams.txt,state.txt}` | NEW | Task 6 |
| `tests/test_vectors.c`, `tests/test_proto_api.c`, `tests/ocb_test.h` | NEW | Task 7 |
| `tests/proto_pure_fixtures/{bad,good}/*.c` | NEW | self-test of `check-proto-pure.sh` |
| `tests/proto_pure_selftest.sh`, `tests/proto_version_selftest.sh` | NEW | follow `tests/pin_grep_selftest.sh` |
| `tests/CMakeLists.txt` | UPDATE | currently registers `test_pins`, `test_pins_debug`, `test_version`, `pin_grep_selftest`; add the new tests, keep these four unchanged |
| `tools/check-proto-version.sh`, `tools/check-proto-pure.sh` | NEW | executable bit set; `shellcheck` clean |
| `CMakeLists.txt` | UPDATE | `OCB_SANITIZE` option and flags in the host-tests branch only (D8); nothing else changes |
| `CMakePresets.json` | UPDATE | `host-tests` cache variable `OCB_SANITIZE=ON` |
| `README.md`, `common/README.md`, `THIRD_PARTY_NOTICES.md` | UPDATE | Task 9.4 |
| `../.github/workflows/firmware.yml` | UPDATE | two steps (Task 8) |
| `../docs/link-protocol.md`, `../docs/usb-controller-emulation.md` (one sentence), `../docs/test-plan.md` | UPDATE | Task 9 |
| `../_bmad-output/implementation-artifacts/sprint-status.yaml` | UPDATE | story status as work moves |

Existing files that must not change: `bridge/` and `controller/` sources (they already link `ocb_proto`; if a one-line CMake change is unavoidable, record it), `common/hw/`, `tools/check-pins.sh` (it must keep passing and already excludes `vectors/`), `tools/toolchain.env`, `cmake/*`, `spikes/`, `app/`, the planning artifacts, `.github/workflows/spike-app-egui.yml`.

### Testing requirements

- `ctest --preset host-tests` (with `noTestsAction: error`) is the CI gate. After this story it runs: `test_pins`, `test_pins_debug`, `test_version`, `pin_grep_selftest` (F.0, unchanged), `test_vectors` (F.9), `test_proto_api` (F.11), `proto_version_check` (with self-test), `proto_pure_check` (with self-test).
- Every vector area of AC3 is covered by a named case: COBS, CRC, every message, a maximum-length frame, an empty frame, back-to-back frames, a leading delimiter, a bad CRC, an overlong run and `NEUTRAL`.
- Sanitizer-clean on macOS clang locally; CI gcc unverified until pushed.
- `fw-release` and `fw-debug` build both executables with no warnings (`-Wall -Wextra -Werror` on our sources, plus `-Wpedantic -Wshadow -Wconversion` on `ocb_proto`).
- Red checks of Task 10.2 prove the gates fail when they should.
- Not testable here and not claimed: the Rust codec (Story 3.2) reading these files; the bridge and controller behaviour on hardware; CI on ubuntu until the first push.

### Previous story intelligence (Story 2.1, done; PR #15)

- `ocb_proto` already exists as an `INTERFACE` target with `include/` and no sources; `bridge` and `controller` already link it. Story 2.1's D9 fixed the boundary: this story adds the sources and makes it `STATIC`; `vectors/` was **not** created there.
- `tests/CMakeLists.txt` builds host executables and registers them with `add_test`; `ocb_strict_warnings()` is a per-source property (`cmake/ocb_warnings.cmake`) because pico-sdk sources compile inside each executable; apply it to the new sources and keep `-Werror` off SDK code.
- The `host-tests` preset is Debug with the native compiler and no pico-sdk (`OCB_HOST_TESTS=ON`); the test preset has `noTestsAction: error`. `firmware/tools/check-pins.sh` excludes `common/proto/vectors/` (hex data) but **does** scan the rest of `common/proto/`: keep GPIO tokens out of code and comments.
- Scripts must behave the same with BSD grep (macOS) and GNU grep (CI): POSIX `grep -E`, `[[:space:]]`, `[0-9]`, no `\b`/`\d`/`-P`. Self-tests for scripts live in `tests/` and are registered in ctest (`pin_grep_selftest.sh` is the pattern to copy, including its usage-error check).
- `firmware.yml` (`ubuntu-24.04`, checkout `v7`, `fetch-depth: 0`, cache `v6`, upload-artifact `v7`) builds `fw-release`, `fw-debug`, `host-tests` and runs the pin grep as its own step; the workflow has never run on GitHub for the Story 2.1 follow-up commits beyond run 44946942547 (green for the UF2 artifact). Add steps in the same style; the concurrency group and the `firmware-uf2` artifact are used by later release work, so do not rename them.
- Learnings: reviews caught evidence overclaims and CI-unverified statements; keep the Completion Notes honest ("unverified until pushed"). The pico-sdk submodule is in place; the local toolchain is in `firmware/.tools` (git-ignored). `bash -n` and `shellcheck` were part of the Story 2.1 checks.
- Epic 1 decisions that matter here: the 2wiCC protocol is "reference only" (ASCII); Bonooru's binary framing, CRC and seq are our own design, so nothing in this story comes from 2wiCC.

### Git intelligence

Recent commits: `75e1382` merge of PR #15 (Story 2.1), `394c06b` LED board check recorded, `588d76e` review follow-ups, `25aa066` the 2.1 implementation, `bd6205f` merge of the Bonooru rename. Pattern: one story branch (`story/2-2-protocol-library-and-vectors`), implementation commits plus a review follow-up commit, merge via PR; the owner commits and pushes. `docs/link-protocol.md` was last touched by the rename and the spec commits, so it has no unrelated in-flight edits. Use `bonooru`/`ocb_` naming only.

### Latest tech information

Researched 2026-11-22 (read-only):

- **CRC-16/CCITT-FALSE** is catalogued by RevEng as **CRC-16/IBM-3740** (aliases CRC-16/AUTOSAR and CRC-16/CCITT-FALSE): width 16, poly `0x1021`, init `0xFFFF`, refin false, refout false, xorout `0x0000`, check `0x29B1`, residue `0x0000` (https://reveng.sourceforge.io/crc-catalogue/16.htm). Do not confuse it with CRC-16/KERMIT (reflected, init 0, also called "CCITT") or CRC-16/XMODEM (init 0). The residue `0x0000` is **not** used by the frame check (the CRC is computed over `type..body` and compared with the two trailing bytes in LE order).
- **COBS**: Stuart Cheshire and Mary Baker, "Consistent Overhead Byte Stuffing", IEEE/ACM Transactions on Networking, vol. 7, no. 2, pp. 159-172, April 1999, DOI 10.1109/90.769765 (https://www.stuartcheshire.org/papers/COBSforToN.pdf; the PDF is compressed and was not text-extracted; the facts below were taken from the Wikipedia article, https://en.wikipedia.org/wiki/Consistent_Overhead_Byte_Stuffing, which cites it). Worst-case overhead is `ceil(n / 254)` bytes, minimum 1; code `0xFF` means 254 non-zero bytes follow with no implied zero; the empty input encodes to the single byte `01` (the article does not state this case; it follows from the block method and the reference C encoder). Published example: `11 22 00 33` encodes to `03 11 22 02 33` (then the `00` delimiter).
- **Independent check performed during story creation** (a scratch Python implementation, not committed): CRC of `123456789` = `0x29B1`; the seven small COBS encodings of Task 6.3 (`01`, `01 01`, `01 01 01`, `01 02 11 01`, `03 11 22 02 33`, `05 11 22 33 44`, `02 11 01 01 01`); a 255-byte all-zero body frame is 260 raw / 261 encoded bytes, a 255-byte non-zero body frame is 260 raw / 262 encoded. These numbers underpin D5.
- The COBS and CRC algorithms are public descriptions; no code was copied from any source (D9).
- **Sanitizers**: ASan and UBSan are available in Apple clang and in GCC 13 on `ubuntu-24.04`; LeakSanitizer is Linux-only for practical purposes on the owner's machine (D8).
- No library, SDK or API version changes in this story: pico-sdk 2.3.1, TinyUSB 0.18.0, Arm GNU Toolchain 15.2.Rel1 and picotool 2.3.1 stay as pinned in Story 2.1.
- The user's global rule to use `ctx7` applies to library, framework, SDK and API documentation lookups; this story has no library dependency, so none was needed.

### References

- COBS: https://www.stuartcheshire.org/papers/COBSforToN.pdf ; https://en.wikipedia.org/wiki/Consistent_Overhead_Byte_Stuffing
- CRC-16/IBM-3740 (CCITT-FALSE): https://reveng.sourceforge.io/crc-catalogue/16.htm
- Epics: `_bmad-output/planning-artifacts/epics.md` (Stories 2.2 to 2.7, 3.1, 3.2)
- Architecture spine: `_bmad-output/planning-artifacts/architecture/architecture-bonooru-2026-11-19/ARCHITECTURE-SPINE.md` (AD-2 to AD-8, AD-10, AD-14, AD-19, AD-21)
- `docs/link-protocol.md`, `docs/usb-controller-emulation.md`, `docs/test-plan.md` ("Firmware host tests")
- `_bmad-output/specs/spec-bonooru/decisions.md` (ADR-002, ADR-004), `requirements.md` (NFR-06, NFR-07)
- Previous story: `_bmad-output/implementation-artifacts/2-1-firmware-build-skeleton-and-ci.md`

### Project Context Rules

- No `project-context.md` exists in the repo.
- Repo content (files, comments, commit text) is English; chat with the owner is Italian.
- Never commit `_bmad/`, `bmad-*` or `gds-*` skills; only `_bmad-output/`.
- Never commit or reference anything under `ux-designs/*/imports/`, game screenshots, recorded audio or video, UF2 or other binaries; no 2wiCC code.
- Do not edit `~/Documents/org/`. Do not commit or push unless the owner asks.
- Ask decisions one at a time and keep them short; search online or upstream before raising an open question.

### Open points for the owner (none block the story)

- D1 numeric values are the first assignment; they are frozen only once Story 2.6/3.2 consume them and a release exists, so the owner can still renumber now at no cost.
- D5 accepts a 255-byte body limit (262-byte encoded frame); lowering it later is a wire change (bump `PROTO_VERSION`) unless done before the first release.
- D8 turns sanitizers on in CI: if gcc on the runner behaves differently from clang locally, the first push will show it.

## Dev Agent Record

### Agent Model Used

Claude Opus 5.5 (claude-opus-5-5), gds-dev-story workflow, non-interactive.

### Implementation Plan

1. Independent vector generator first (scratch Python, never committed), anchored on the RevEng check `29b1`, the seven small COBS encodings, Wikipedia COBS examples 7 to 11 and the story's `hello`/`stop`/`queue_event_neutral` wires (all asserted in the generator; none disagreed).
2. `ocb_proto` in C99 from the published descriptions: `consts.h`, `err.h`, `crc16`, `cobs`, `state`, `frame` (encode/decode + compile-time checks), `rx` (splitter + counters), `msgs`.
3. F.9 `test_vectors.c` (own line parser per D2/D3, descriptor table per message), F.11 `test_proto_api.c`, `ocb_test.h` (D7).
4. Scripts `check-proto-version.sh`, `check-proto-pure.sh` with ctest self-tests; CI steps; `OCB_SANITIZE` (D8).
5. Docs, then the local verification and red checks of Task 10.

### Debug Log References

- First ctest run: `test_proto_api` failed twice because of two wrong expectations in the test itself (a 255-byte zero body frame is 261 encoded bytes **plus** the delimiter = 262 returned; the 12-byte-body frame needs 19 bytes, so the too-small loop must stop at 18). Fixed the test, not the library. `test_vectors` passed on the first run; its ability to fail was then proven by the red checks below.
- `-Wconversion` kept on `ocb_proto`: it needed only the usual `(uint8_t)`/`(uint16_t)` narrowing casts in byte packing, nothing ugly.
- gcc check without CI: Homebrew `gcc-16` (macOS arm64) built the host-tests with and without `OCB_SANITIZE` (scratch build dirs outside the repo, deleted afterwards): zero warnings, 8/8 tests pass. Docker was not running, so Ubuntu gcc 13 itself was not tried.
- Sanitizer effectiveness: temporarily disabling the truncation check in `ocb_cobs_decode` made `test_proto_api` abort with `AddressSanitizer: heap-buffer-overflow ... cobs.c:77 in ocb_cobs_decode` (every-prefix test); reverted.

### Decisions made during implementation (D1 to D10 followed; additions below)

- **D11. Separate `ocb_proto/err.h`** for `ocb_err` (negative codes `OCB_ERR_EMPTY, COBS, CRC, RANGE, BODY_LEN, OVERLONG, BUF`), included by `frame.h` and `msgs.h`. The story's file list had no home for error codes shared by the frame and message layers; putting them in `consts.h` would mix non-wire values into the wire-constant header.
- **D12. Overlong counting in `ocb_rx`**: counted once when the first excess byte (the 263rd) arrives; the run is discarded and the delimiter that closes it returns `OCB_RX_DROPPED` without counting again. Same final counters as counting at the delimiter for any delimiter-terminated stream (the vectors), and the counter is visible before the line resyncs. `ocb_rx` also keeps `last_err` (reason of the last dropped run) so Story 2.6 can choose `NAK CRC` vs nothing without diffing counters. The optional `ocb_rx_push_bytes` was not added (nothing needs it yet).
- **D13. Direct `ocb_frame_decode` edge cases**: `enc_len > OCB_MAX_ENCODED_FRAME` returns `OCB_ERR_OVERLONG` (the AD-4 rule, consistent with the splitter); a **well-formed** COBS input that decodes to more than `OCB_MAX_RAW_FRAME` bytes cannot satisfy the length rule and is reported as `OCB_ERR_CRC` (length inconsistency, AD-3), while a malformed one is always `OCB_ERR_COBS` because `ocb_cobs_decode` validates the whole input before reporting `SPACE` (review fix). `ocb_cobs_decode` reports an empty input as its own `OCB_COBS_ERR_EMPTY`.
- **D18 (review). `EMPTY` and `OVERLONG` are vector error names** in `frames.txt` (direct `ocb_frame_decode` cases), so the Rust codec pins the same classification; streams keep counting them as before.
- **D19 (review). `ocb_msg_status_encode` refuses `extra == NULL` with `extra_len > 0`** (`OCB_ERR_BUF`) instead of silently encoding the base body: a caller bug should not produce a valid-looking frame.
- **D14. Two optional vector fields** (documented in `vectors/README.md`): `messages.txt` `trailing` (hex bytes after the v1 body that the decoder ignores; the check is `encode(fields) + trailing == body`), used by `hello_trailing`, `info_trailing`, `heartbeat_trailing`, so the encode direction is still checked; STATUS exposes its trailing bytes as `extra=` in `fields` instead. `frames.txt` error cases may carry `payload` to also check the encoder (`all_zero_type_seq`, `type_00`).
- **D15. Extra constants beyond the D1/D6 lists**, all wire facts the Rust codec needs, in `consts.h` and `constants.txt`: `TYPE_{APP,BRIDGE,CTRL}_{MIN,MAX}`, `NAK_{TRANSPORT,SEMANTIC}_{MIN,MAX}` (plus the `OCB_NAK_IS_TRANSPORT/SEMANTIC` macros), `STICK_CENTER`, `STATE_SIZE`, `FRAME_DELIMITER`, `FRAME_HEADER_SIZE`, `FRAME_CRC_SIZE`, `BODY_LEN_*` (one per message), `CRC16_POLY/INIT/CHECK`.
- **D16. Unregistered area files are caught**: `tests/CMakeLists.txt` globs `vectors/*.txt` (`CONFIGURE_DEPENDS`) and passes the count as `OCB_VECTORS_TXT_COUNT`; `test_vectors` fails if it differs from `kFiles[]`. Also `constants.txt` has a minimum of 100 lines and every message type needs at least one ok case.
- **D17. Compile-time checks live in `frame.c`** (C99 negative-array typedefs), keeping `consts.h` plain `#define`s; the heartbeat-gap check uses `OCB_MAX_WIRE_FRAME` (= 262 + 1, the formula of Task 1.3).
- Vector failures are reported as `file:line: case: message` with the case's first line (not always the exact field line).

### Completion Notes List

- Review follow-up (2026-11-22): 5 findings resolved (1 High, 4 Low), see "Review Follow-ups (AI)". Red first: the new `frames.txt` cases failed against the old decoder (`frames.txt:117: cobs_overflow_truncated: frame_decode returned -3, expected COBS (-2)`, same for `cobs_overflow_zero`, and `EMPTY`/`OVERLONG` unknown names), then green after the fix. Vectors regenerated with the scratch generator (also stripped two trailing spaces on empty values in `cobs.txt` and `crc16.txt`). Re-verified: host-tests from a clean build dir `100% tests passed out of 8`, `test_vectors: 239 cases in 7 files ok (proto_version 01)`, no warnings, ASan/UBSan on; `fw-release` and `fw-debug` rebuilt the changed `ocb_proto` objects, rc 0, 0 warnings (`ocb_proto` text now 2008 bytes); `check-pins.sh`, `check-proto-version.sh`, `check-proto-pure.sh` ok; `shellcheck` clean.

- `ocb_proto` is a pure C99 `STATIC` library (`C_STANDARD 99`, `C_EXTENSIONS OFF`, `-Wall -Wextra -Werror -Wpedantic -Wshadow -Wconversion`), no heap, no I/O, no global mutable state, byte-wise LE access only. COBS and CRC written from the published descriptions (references in the source headers; `THIRD_PARTY_NOTICES.md` line added).
- Vectors: 7 files, 234 cases/lines checked (constants 117 entries, cobs 17, crc16 6, frames 25, messages 35, streams 14, state 20); every AC3 case is named (max-length frames 261/262, empty frame, back-to-back, leading delimiter, bad CRC, overlong 263 and 1000, `NEUTRAL`).
- Commands run from `firmware/` (with `OCB_ARM_TOOLCHAIN=$PWD/.tools/arm-gnu-toolchain-15.2.Rel1`, `OCB_PICOTOOL_DIR=$PWD/.tools/picotool-2.3.1`):
  - `cmake --preset host-tests && cmake --build --preset host-tests && ctest --preset host-tests` → `100% tests passed out of 8` (test_pins, test_pins_debug, test_version, pin_grep_selftest, test_vectors, test_proto_api, proto_version_check, proto_pure_check); `test_vectors: 234 cases in 7 files ok (proto_version 01)`; build output had no warnings. ASan and UBSan ran on macOS Apple clang 21 (`-fsanitize=address,undefined` on all 22 compile/link lines, `___asan_init` linked).
  - `cmake --preset fw-release && cmake --build --preset fw-release` and the same for `fw-debug`: both `bridge` and `controller` built, linking `libocb_proto.a`, zero warnings.
  - From the repo root: `firmware/tools/check-pins.sh` ok, `firmware/tools/check-proto-version.sh` ok (01, 7 files), `firmware/tools/check-proto-pure.sh` ok (14 files); `shellcheck` clean on all `firmware/tools/*.sh` and `firmware/tests/*.sh`; `actionlint .github/workflows/firmware.yml` clean.
- Red checks (each reverted, tree back to green afterwards): `OCB_PROTO_VERSION 2` → `test_vectors` fails on all 7 headers, `proto_version_check` fails, `check-proto-version.sh` exit 1 naming each file; `min_stop.wire` last digit flipped in `frames.txt` → `test_vectors` fails with `frames.txt:10: min_stop: frame_encode(payload) ... differs` plus hex dumps; `#include "pico/stdlib.h"` in `crc16.c` → `check-proto-pure.sh` exit 1 naming file:line and the host build fails (`'pico/stdlib.h' file not found`); `OCB_NAK_BUSY 0x19` → `constants.txt:39: NAK_BUSY: vector value 00000011, OCB_NAK_BUSY = 00000019`; a `GP0` comment in `rx.c` → `check-pins.sh` exit 1; COBS truncation check disabled → ASan heap-buffer-overflow in `test_proto_api`.
- `ocb_proto` size in the release build (`arm-none-eabi-size` of the archive objects, informational): text 1988 bytes total (crc16 204, cobs 272, state 200, frame 288, rx 196, msgs 828), no data/bss.
- CI: two steps added after "Pin grep (AD-20)": "Proto version vs vectors (AD-14)" and "Proto library is pico-sdk free (AD-2)". **CI is unverified until the owner pushes** (ubuntu-24.04 gcc with sanitizers and LeakSanitizer via `ASAN_OPTIONS=detect_leaks=1` on Linux). Local proxy: Homebrew gcc-16 with sanitizers, green. Local reproduction of every CI step: the command list above plus the existing submodule and install steps of `firmware/README.md`.
- `.gitkeep` in `common/proto/include/` deleted from the working tree (not staged). No bridge/controller source changed, no new dependency, no file under `_bmad/` or skill folders touched.

### File List

New:
- `firmware/common/proto/crc16.c`
- `firmware/common/proto/cobs.c`
- `firmware/common/proto/state.c`
- `firmware/common/proto/frame.c`
- `firmware/common/proto/rx.c`
- `firmware/common/proto/msgs.c`
- `firmware/common/proto/include/ocb_proto/consts.h`
- `firmware/common/proto/include/ocb_proto/err.h`
- `firmware/common/proto/include/ocb_proto/crc16.h`
- `firmware/common/proto/include/ocb_proto/cobs.h`
- `firmware/common/proto/include/ocb_proto/state.h`
- `firmware/common/proto/include/ocb_proto/frame.h`
- `firmware/common/proto/include/ocb_proto/msgs.h`
- `firmware/common/proto/include/ocb_proto/ocb_proto.h`
- `firmware/common/proto/vectors/README.md`
- `firmware/common/proto/vectors/constants.txt`
- `firmware/common/proto/vectors/cobs.txt`
- `firmware/common/proto/vectors/crc16.txt`
- `firmware/common/proto/vectors/frames.txt`
- `firmware/common/proto/vectors/messages.txt`
- `firmware/common/proto/vectors/streams.txt`
- `firmware/common/proto/vectors/state.txt`
- `firmware/tests/ocb_test.h`
- `firmware/tests/test_vectors.c`
- `firmware/tests/test_proto_api.c`
- `firmware/tests/proto_pure_selftest.sh`
- `firmware/tests/proto_version_selftest.sh`
- `firmware/tests/proto_pure_fixtures/bad/includes_sdk.c`
- `firmware/tests/proto_pure_fixtures/bad/includes_hw.c`
- `firmware/tests/proto_pure_fixtures/bad/includes_stdio.h`
- `firmware/tests/proto_pure_fixtures/bad/includes_version.c`
- `firmware/tests/proto_pure_fixtures/good/allowed.c`
- `firmware/tools/check-proto-version.sh`
- `firmware/tools/check-proto-pure.sh`

Modified:
- `firmware/common/proto/CMakeLists.txt`
- `firmware/tests/CMakeLists.txt`
- `firmware/CMakeLists.txt`
- `firmware/CMakePresets.json`
- `firmware/README.md`
- `firmware/common/README.md`
- `firmware/THIRD_PARTY_NOTICES.md`
- `.github/workflows/firmware.yml`
- `docs/link-protocol.md`
- `docs/usb-controller-emulation.md`
- `docs/test-plan.md`
- `_bmad-output/implementation-artifacts/sprint-status.yaml`
- `_bmad-output/implementation-artifacts/2-2-protocol-library-and-conformance-vectors.md`

Deleted:
- `firmware/common/proto/include/.gitkeep`

Changed by the review follow-up: `firmware/common/proto/cobs.c`, `firmware/common/proto/frame.c`, `firmware/common/proto/msgs.c`, `firmware/common/proto/include/ocb_proto/{cobs,frame,msgs}.h`, `firmware/common/proto/vectors/{README.md,cobs.txt,crc16.txt,frames.txt}`, `firmware/tests/test_vectors.c`, `firmware/tests/test_proto_api.c`, this story file.

## Change Log

- 2026-11-22: story created (ready-for-dev). Decisions D1 to D10 recorded; CRC-16/IBM-3740 parameters and COBS facts confirmed upstream; vector format and numeric values assigned.
- 2026-11-22: implemented (review). `ocb_proto` (COBS, CRC-16, frames, splitter and counters, 12 message codecs, canonical state, constants), 7 vector files generated independently, F.9 and F.11 in ctest with ASan/UBSan, `check-proto-version.sh` and `check-proto-pure.sh` with self-tests and CI steps, docs updated. Decisions D11 to D17 added. CI unverified until pushed.
- 2026-11-22: Addressed code review findings - 5 items resolved (AD-3 COBS-before-length fix in `ocb_cobs_decode`, new frame vectors incl. EMPTY/OVERLONG, header docs, STATUS extra NULL refused). Decisions D18, D19 added, D13 refined.
