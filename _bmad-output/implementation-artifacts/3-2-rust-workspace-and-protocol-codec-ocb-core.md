---
baseline_commit: b5e0da02888fcbde32109924b23cdc1bf3556fe9
---

# Story 3.2: Rust workspace and protocol codec (`ocb-core`)

Status: review

<!-- Ultimate context engine analysis completed - comprehensive developer guide created (gds-create-story, 2026-11-24) -->

## Story

As the owner,
I want the app's protocol code in Rust, checked against the same vectors as the firmware,
so that the PC side and the firmware speak exactly the same wire format.

## Acceptance Criteria

1. **Workspace and core crate.** Given `app/` as a Cargo workspace with `app/rust-toolchain.toml` pinning Rust 1.100.0, when the owner runs `cargo test`, then crate `ocb-core` (lib) builds with no egui/eframe dependency, no threads and no I/O (AD-18), and its `protocol` module provides COBS, CRC-16/CCITT-FALSE, frame encode/decode with the AD-3 receive order, every message type, the canonical state and `NEUTRAL` (AD-10).
2. **Conformance vectors.** Given `firmware/common/proto/vectors/`, when the core tests run, then they read every vector file and pass all of them, and the `PROTO_VERSION` constant equals the vector header (AD-14); and `proptest` round-trips COBS and frames for random payloads.
3. **CI.** Given a push, when the GitHub Actions app job runs on ubuntu, windows and macos, then it runs `cargo test`, `cargo clippy -D warnings` and a `cargo-deny` licence check against the NFR-07 allow-list (AD-21); and the app version comes from `git describe` with fallback `0.0.0-dev`.

No bench part: this story is host-only (no H-rows, no hardware, no retro action #27 step).

## Tasks / Subtasks

- [x] **Task 1: Cargo workspace skeleton (AC: 1)**
  - [x] 1.1 `app/Cargo.toml`: `[workspace]` with `resolver = "3"`, `members = ["core"]`; `[workspace.package]` with `edition = "2024"`, `license = "MIT"`, `publish = false`, `rust-version = "1.100"`, `version = "0.0.0"` (the Cargo version is a placeholder; the real app version comes from D5). `[workspace.lints]` (see D9).
  - [x] 1.2 `app/rust-toolchain.toml`: `channel = "1.100.0"`, `components = ["clippy", "rustfmt"]`, `profile = "minimal"`.
  - [x] 1.3 `app/core/Cargo.toml`: package `ocb-core`, lib only, `[dependencies]` **empty**, `[dev-dependencies] proptest = "=1.12.0"`, `lints.workspace = true`.
  - [x] 1.4 Commit `app/Cargo.lock` (the CI uses `--locked`). Add `app/target/` to `.gitignore`.
  - [x] 1.5 `app/README.md`: replace the "becomes a Cargo project with the first app story" sentence with the real layout and the three commands (`cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo deny check licenses`), all run from `app/`.
- [x] **Task 2: `protocol` module (AC: 1)** — under `app/core/src/protocol/`; see "Module design" below.
  - [x] 2.1 `consts.rs`: every wire constant of `constants.txt` (same names, no `OCB_` prefix), plus a table `ALL: &[(&str, u32)]` used by the vector test in both directions. `PROTO_VERSION: u8 = 1`.
  - [x] 2.2 `cobs.rs`: `encode(&[u8]) -> Vec<u8>` and `decode(&[u8]) -> Result<Vec<u8>, CobsError>` (`Empty`, `Zero`, `Truncated`), the exact rules of `cobs.c` (0xFF block without implied zero, no final code byte after a terminal 0xFF block, empty input → `01`).
  - [x] 2.3 `crc16.rs`: CRC-16/CCITT-FALSE (poly 0x1021, init 0xFFFF, no reflection, no xorout), `crc16(&[u8]) -> u16`; little-endian on the wire. Check value `0x29B1`.
  - [x] 2.4 `frame.rs`: `RxSide { Controller, App }`, `type_in_range(side, ty)`, `encode_frame(ty, seq, body) -> Result<Vec<u8>, FrameError>` (returns COBS + trailing `0x00`; `body.len() > 255` → `Err(BodyTooLong)`), `decode_frame(side, enc_without_delimiter) -> Result<Frame, FrameError>` with the AD-3 order: `Empty` (len 0), `Overlong` (> 262), COBS (malformed always `Cobs`, even when the output would also be too long), length consistency (`n < 5`, `n != 3 + len + 2`, or decoded > 260 → `Crc`), CRC → `Crc`, type range → `Range`. `Frame { ty, seq, body: Vec<u8> }`.
  - [x] 2.5 `rx.rs`: streaming `Splitter` (one per receiving side, no I/O): `push(byte) -> RxEvent` (`None`, `Frame(Frame)`, `Dropped(FrameError)`) with `RxStats { ok, empty, cobs, crc, range, overlong }`, exactly the semantics of `rx.c` and story 2.2 D12 (overlong counted once at the 263rd byte, the rest of the run discarded up to and including the next `0x00`, which returns `Dropped` without counting again; an empty run counts in `empty`). A convenience `push_slice(&[u8]) -> Vec<Frame>` is fine.
  - [x] 2.6 `state.rs`: `State { buttons: u16, hat: u8, lx, ly, rx, ry, flags: u8 }`, `pub const NEUTRAL: State`, `to_bytes() -> [u8; 8]`, `from_bytes(&[u8; 8])`, predicates `is_neutral`, `is_neutral_pose`, `has_end`, `reserved_bits_ok` (same meaning as `state.h`). No validation of hat or bits 14/15 in the codec (as in C).
  - [x] 2.7 `msgs.rs`: one struct per message body (HELLO, INFO, SET_STATE, QUEUE_EVENT, START, STOP, PING, HEARTBEAT, STATUS, ACK, NAK, EVENT) with `encode() -> Vec<u8>` and `decode(&[u8]) -> Result<Self, MsgError::BodyLen>`, using the **exact C length rules** (table below). STATUS keeps its trailing bytes as `extra: Vec<u8>`. NAK `reason` and EVENT `code` decode any byte (see D3). Plus a `Message` enum and `Message::decode(ty, body)` / `Message::ty()` so Story 3.3 can match on typed frames; an unassigned type is `Err(MsgError::UnknownType(ty))`, never a panic.
  - [x] 2.8 `protocol/mod.rs` re-exports; `lib.rs` has `pub mod protocol; pub mod version;` and `#![forbid(unsafe_code)]`.
- [x] **Task 3: Vector conformance test C.1 (AC: 2)** — `app/core/tests/vectors.rs`
  - [x] 3.1 Vector directory: `concat!(env!("CARGO_MANIFEST_DIR"), "/../../firmware/common/proto/vectors")`. Fail loudly if it is missing (never skip).
  - [x] 3.2 A strict parser that follows `vectors/README.md` exactly: blank / comment / `key: value`; split at the first `:`; first key/value line must be `proto_version: XX` and must equal `consts::PROTO_VERSION`; cases grouped by `<case>.<field>` split at the **last** `.`; reject tabs, `#` inside a value, unknown fields, duplicate fields, missing required fields, duplicate case names, bad hex. Report failures as `file:line: case: message` and collect them all before failing (one run shows every problem, like F.9).
  - [x] 3.3 The same file list and minimum case counts as `kFiles[]` in `firmware/tests/test_vectors.c`: `constants.txt` ≥ 100 lines, `cobs.txt` ≥ 14, `crc16.txt` ≥ 5, `frames.txt` ≥ 18, `messages.txt` ≥ 18, `streams.txt` ≥ 10, `state.txt` ≥ 17. Fail if the directory holds a `.txt` not in the list (D16 of story 2.2).
  - [x] 3.4 Per-file checks exactly as the README table: cobs (encode and decode, or decode error), crc16, frames (`encode == wire` and `decode(wire, rx) == payload`, or the named `err`, with optional `payload` encode check; `rx` default `CONTROLLER`), messages (`encode(fields) + trailing == body`, `decode(body) == fields`, `encode_frame(type, seq, body) == wire` + `0x00`; or `BODY_LEN`), streams (feed `in` byte by byte through `Splitter`, frames and all six stats equal), state (both directions; `neutral` == `NEUTRAL`), constants (every line equals the Rust constant of that name **and** every entry of `consts::ALL` appears in the file).
  - [x] 3.5 Every message type has at least one ok case in `messages.txt` (same as F.9).
  - [x] 3.6 Red proof (retro takeaway, "shown failing once"): temporarily change one Rust constant, one CRC byte of a vector copy in a temp dir, and the `proto_version` header of a copy; each must fail with a clear message. Record the three red runs in the Dev Agent Record. Use a temp dir copy, never edit the committed vectors.
- [x] **Task 4: Property and API tests C.2, C.3 (AC: 2)** — `app/core/tests/props.rs`, unit tests in modules
  - [x] 4.1 proptest: `cobs::decode(cobs::encode(x)) == x` for `x` of 0..=600 random bytes (biased toward zeros and 0xFF-length runs, e.g. a strategy mixing `0u8` and `1..=255`); encoded output never contains `0x00` and has length ≤ `n + ceil(n/254) + 1`.
  - [x] 4.2 proptest: for random `ty` in each side's range, random `seq`, body 0..=255: `decode_frame(side, encode_frame(..) minus delimiter) == Frame`; encoded length ≤ 263 with the delimiter.
  - [x] 4.3 proptest: a random sequence of frames, garbage runs and `0x00` runs, fed byte by byte vs in random chunks into two `Splitter`s gives identical frames and stats; every valid frame that follows a `0x00` is delivered.
  - [x] 4.4 No-panic property: `decode_frame` and every `Message::decode` on arbitrary bytes (0..=600) return `Ok` or `Err`, never panic; every prefix of a valid frame decodes to an error without panicking.
  - [x] 4.5 Unit tests for what the vectors do not pin: `encode_frame` with 256-byte body → `Err`; STATUS encode with `extra` longer than 244 → `Err` (255 − 11); `State` predicates (NEUTRAL|END is a neutral pose but not neutral; reserved flag bits).
- [x] **Task 5: Core purity gate C.4 (AC: 1)** — `app/core/tests/purity.rs`
  - [x] 5.1 Scan every `.rs` under `app/core/src/` (recursively) for banned paths and tokens: `std::thread`, `std::sync::mpsc`, `std::fs`, `std::net`, `std::process`, `std::env`, `std::io`, `std::time::Instant`, `std::time::SystemTime`, `thread::spawn`, `unsafe`, `extern "C"`, `println!`, `eprintln!`, `print!`, `dbg!`, `include_bytes!`, `include_str!` (D10). Comments are not exempt (simpler and stricter, like the firmware gates).
  - [x] 5.2 Parse `app/core/Cargo.toml`: `[dependencies]` must be empty (D2); no `egui`, `eframe`, `serialport` anywhere in it.
  - [x] 5.3 Self-test with fixtures under `app/core/tests/fixtures/purity/`: one bad file per banned token is flagged, one good file passes, and the real tree passes. Fixtures are text files with a non-`.rs` extension (e.g. `.rs.txt`) so Cargo never compiles them.
- [x] **Task 6: App version from `git describe` C.5 (AC: 3)**
  - [x] 6.1 `app/core/build.rs` runs `git describe --tags --match "v[0-9]*.[0-9]*.[0-9]*" --long --dirty` from the repo root (`CARGO_MANIFEST_DIR/../..`), and parses it with **the same rule as `firmware/cmake/ocb_version.cmake`**: `^v(\d+)\.(\d+)\.(\d+)-(\d+)-(g[0-9a-f]+)(-dirty)?$` → `X.Y.Z`, plus `-N-gHASH` when N > 0, plus `-dirty`; anything else (no git, no tag, no `.git`, a failing command) → `0.0.0-dev` with major/minor/patch 0. Emit `cargo:rustc-env=OCB_APP_VERSION=…` and `OCB_APP_VERSION_MAJOR/MINOR/PATCH`.
  - [x] 6.2 Put the parser in `app/core/src/version_parse.rs` (pure function, no regex crate: a hand-written parser) and use it from both `build.rs` (`include!` or `#[path]` module) and the unit tests, so the parsing rule is tested: `v0.2.3-0-gabc1234` → `0.2.3`; `v0.2.3-5-gabc1234` → `0.2.3-5-gabc1234`; `v0.2.3-0-gabc1234-dirty` → `0.2.3-dirty`; `v1.2-0-gabc`, `0.2.3-0-gabc`, `v1.2.3-x-gabc`, empty → fallback; major/minor/patch > 255 → fallback (they are `u8` on the wire, AD-14 conventions).
  - [x] 6.3 `ocb_core::version`: `pub const APP_VERSION: &str = env!("OCB_APP_VERSION");` and `pub const APP_VERSION_TRIPLE: [u8; 3]` (Story 3.3 compares it with `INFO.fw_ver` / `bridge_ver`, AD-13).
  - [x] 6.4 `cargo:rerun-if-changed` on the repo's `.git/HEAD`, `.git/refs/tags` and `.git/packed-refs` when they exist (best effort; CI always builds fresh). The build script must never fail the build: every git error → fallback.
- [x] **Task 7: CI workflow `app.yml` (AC: 3)**
  - [x] 7.1 New `.github/workflows/app.yml`, name `app`, on `push`, `pull_request`, `workflow_dispatch` (no path filter, like `firmware.yml`, so it can become a required check). `permissions: contents: read`; concurrency group per ref.
  - [x] 7.2 Matrix `ubuntu-24.04`, `windows-latest`, `macos-latest`, `fail-fast: false`; `defaults.run.working-directory: app`; `actions/checkout@v7` with `fetch-depth: 0` (tags for `git describe`).
  - [x] 7.3 Toolchain from `app/rust-toolchain.toml`: `rustup toolchain install` (no argument: rustup reads the file) then `rustc --version` must print `1.100.0` (fail otherwise). No `dtolnay/rust-toolchain@stable` (that would ignore the pin).
  - [x] 7.4 `Swatinem/rust-cache@v2` with `workspaces: app -> target`.
  - [x] 7.5 Steps: `cargo fmt --all --check`; `cargo test --locked --workspace`; `cargo clippy --locked --workspace --all-targets -- -D warnings`; install cargo-deny (`taiki-e/install-action` with `tool: cargo-deny@0.22.0`, D7) and `cargo deny --locked check licenses` on **every** OS (the AC names all three).
  - [x] 7.6 Job summary: OS, `rustc --version`, app version (print `OCB_APP_VERSION` from a tiny test or `cargo test -- --nocapture version_prints`; simplest: a `#[test]` that prints it, run with `--nocapture` and grep). Keep it simple; the summary is a nice-to-have, the gates are not.
  - [x] 7.7 Do not touch `spike-app-egui.yml` or `firmware.yml`.
- [x] **Task 8: Licence gate config (AC: 3)** — `app/deny.toml`
  - [x] 8.1 `[licenses] allow = ["MIT", "Apache-2.0", "Apache-2.0 WITH LLVM-exception", "BSD-2-Clause", "BSD-3-Clause", "ISC", "Zlib", "MPL-2.0"]`, `confidence-threshold = 0.93`, `unused-allowed-license = "allow"` (the list is the policy, not the current graph). Header comment: NFR-07 as amended 2026-11-19, AD-21, and that OFL-1.1 / Ubuntu-font-1.0 join when egui arrives (Epic 4, AD-18).
  - [x] 8.2 Dev-dependencies stay in the graph (stricter). If `proptest`'s tree needs a licence outside the list (e.g. `Unicode-3.0`, `BSL-1.0`, `CC0-1.0`), **do not add it silently**: record it as a decision and flag it to the owner in the completion notes (NFR-07 names "MIT/BSD/Apache/ISC/Zlib" plus the weak-copyleft amendment).
  - [x] 8.3 Red proof: add a temporary dependency or an `allow` list without `MIT` in a scratch copy and show `cargo deny check licenses` failing; record it.
- [x] **Task 9: Line endings for the vectors (AC: 2, windows)** — new `/.gitattributes` with `firmware/common/proto/vectors/*.txt text eol=lf` (D8). The Rust parser stays strict on `\n` (README rule 1).
- [x] **Task 10: Docs (AC: 1–3)**
  - [x] 10.1 `docs/test-plan.md`: new section "App core tests (`cargo test`, no hardware)" after the firmware host tests, rows C.1 to C.5 (see Testing requirements).
  - [x] 10.2 `firmware/common/proto/vectors/README.md`: the first paragraph already names "the app's Rust codec tests (Story 3.2)": add the test path `app/core/tests/vectors.rs` and that adding a vector file also means adding it to the Rust list.
  - [x] 10.3 Dev Agent Record: decisions, red proofs, CI run link, file list.

## Dev Notes

### What exists today (read before writing)

- `app/` holds only `README.md` and `profiles/oot-switch2-1.1.1.toml` (Story 1.5). There is no Cargo project yet; `spikes/app-egui/` is a throwaway spike with its own Cargo project and workflow, **do not reuse or modify it** (it is not part of the workspace).
- The C reference implementation is `firmware/common/proto/` (Story 2.2): `cobs.c`, `crc16.c`, `frame.c`, `rx.c`, `msgs.c`, `state.c`, headers in `include/ocb_proto/`. The Rust codec must behave identically on every vector and on the edge cases listed in the headers. Read `frame.h` and `rx.c` comments before writing `frame.rs` / `rx.rs`.
- The C vector test `firmware/tests/test_vectors.c` (F.9) is the model for the Rust test: same file list, same minimum case counts, same both-direction constant check, same "every message type has an ok case".
- `firmware/tools/link_probe.py` also carries a Python codec checked against the vectors (F.19). It is a bench tool, not a reference; do not copy from it.

### Module design (binding where it says "must")

| Item | Rule |
|---|---|
| Crate | `ocb-core`, lib, path `app/core/` (Structural seed of the spine: `app/core/`). Module `protocol` now; `link` arrives in Story 3.3 (do not stub it). |
| Purity | Must have no I/O, no threads, no clocks, no `unsafe`, no printing. `std` is allowed (Vec, Result); `alloc` is fine. |
| Errors | Typed enums (`CobsError`, `FrameError`, `MsgError`), `#[derive(Debug, Clone, Copy, PartialEq, Eq)]`, implement `std::fmt::Display` and `std::error::Error`. No `thiserror` (no dependencies, D2). Decoders never panic on any input. |
| Integers | Little-endian on the wire, byte by byte (`u16::from_le_bytes` on a 2-byte array is fine). Versions are `[u8; 3]`. Times are `u32` µs. |
| Naming | Rust case of the C names: `MSG_HELLO`, `NAK_BAD_STATE`, `EVT_LINK_LOST`, `BTN_A`, `HAT_NEUTRAL`, `MAX_ENCODED_FRAME` … (`consts` module, same names as `constants.txt`). |

**Message body length rules** (copied from `msgs.c`; the vectors pin them, D10 of Story 2.2):

| Message | Type | v1 body | Decode accepts | Notes |
|---|---|---|---|---|
| HELLO | `0x01` | 1 | `len >= 1` | trailing ignored (frozen prefix, AD-7) |
| SET_STATE | `0x02` | 8 | `== 8` | |
| QUEUE_EVENT | `0x03` | 12 | `== 12` | `t_us` u32 + state |
| START | `0x04` | 4 | `== 4` | `lead_us` u32 |
| STOP | `0x05` | 0 | `== 0` | |
| PING | `0x06` | 0 | `== 0` | |
| HEARTBEAT | `0x40` | 4 | `len >= 4` | `bridge_fw_ver[3], flags`; trailing ignored |
| INFO | `0x81` | 14 | `len >= 14` | `proto_ver, fw_ver[3], bridge_ver[3], hid_profile, queue_cap u16, app_timeout_ms u16, report_period_us u16`; trailing ignored |
| STATUS | `0x83` | 11 | `len >= 11` | `free u16, now_us u32, last_seq, mode, abort_count, last_event, flags` + `extra` (debug counters; up to 244 bytes) |
| ACK | `0x84` | 1 | `== 1` | |
| NAK | `0x85` | 3 | `== 3` | `reason, expected, got` |
| EVENT | `0x86` | 1 | `== 1` | `code` |

Note: STOP decoding is strict (`== 0`) at codec level although the firmware link executes a STOP with a body anyway (F.5); that is the controller's semantics, not the codec's, and the app never receives STOP.

### Decisions (made autonomously, 2026-11-24; the owner can overrule any of them)

- **D1. Crate path and name.** `app/core/` with package `ocb-core` (lib name `ocb_core`), per the spine's Structural seed and `app-stack-options.md`. No `ocb-app` binary in this story (Epic 4 adds it; Story 3.4 adds `app/tools/ocb-dev`).
- **D2. Zero runtime dependencies in `ocb-core`.** COBS, CRC and the codec are small; no `crc`, `cobs`, `thiserror`, `byteorder`, `regex`. Only `proptest = "=1.12.0"` as a dev-dependency (`app-stack-options.md`, checked on crates.io 2026-11-24: 1.12.0 is current). The purity gate enforces the empty `[dependencies]`; a later story that needs a dependency edits the gate on purpose.
- **D3. Unknown NAK reasons and EVENT codes.** AD-19 says they are never fatal and Story 3.3 must report them as typed "unknown" events. The codec therefore decodes any byte: `NakReason` and `EventCode` are enums with an `Unknown(u8)` variant and lossless `from_u8` / `to_u8` (the vectors `nak_unknown_reason` 0x7E and `event_unknown` 0xEE must round-trip). Same for `Mode` and `HidProfile` (`Unknown(u8)`). `NakReason::is_transport()` / `is_semantic()` follow `NAK_TRANSPORT_MIN..=MAX` and `NAK_SEMANTIC_MIN..=MAX` (AD-6).
- **D4. Frame encode includes the delimiter**, like `ocb_frame_encode`; the vectors' `wire` is without it, so the test compares `wire + [0x00]`. `decode_frame` takes the run without its delimiter, like `ocb_frame_decode`.
- **D5. App version at build time in `ocb-core`'s `build.rs`**, same `git describe` command and regex as `firmware/cmake/ocb_version.cmake` (AD-14: "read through `git describe` (fallback `0.0.0-dev`) by the firmware CMake and the app build only"). A build script is compile-time, so it does not break the "no I/O" rule of the crate's runtime code. The Cargo `version` stays `0.0.0`.
- **D6. Test IDs `C.n`** ("app core") in a new test-plan section, next to `F.n` (firmware host tests). C.1 vectors, C.2 property round trips, C.3 codec API edges, C.4 purity gate, C.5 version parsing.
- **D7. cargo-deny pinned at 0.22.0** (latest on 2026-11-24) through `taiki-e/install-action`, run on all three OS. `EmbarkStudios/cargo-deny-action` is Linux-only (Docker) and the AC names all three OS.
- **D8. `.gitattributes` forces LF on the vector files.** The windows runner checks out with `core.autocrlf=true`; CRLF would break README rule 1. Forcing LF keeps the parser strict instead of making it lenient.
- **D9. Lints.** `[workspace.lints.rust] unsafe_code = "forbid"`, `missing_debug_implementations = "warn"`; `[workspace.lints.clippy] unwrap_used = "deny"`, `expect_used = "deny"`, `panic = "deny"`, `indexing_slicing = "deny"` for library code. Tests may opt out locally with `#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]` at the top of each test file. Rationale: decoders run on bytes from a cable and must never panic. If `indexing_slicing` makes the codec unreadable, drop that one lint (a `warn` would be fatal anyway under `-D warnings`) and rely on the no-panic property test C.2; record the choice.
- **D10. Purity gate as a Rust test** (`tests/purity.rs`), not a shell script: it runs on all three OS with `cargo test` and needs no extra CI step. Fixture self-test as in the firmware gates (F.13, F.18, F.30).

### Testing requirements (rows for `docs/test-plan.md`)

| ID | Test | Pass criterion | Binds |
|---|---|---|---|
| C.1 | Shared vectors (Story 3.2, `app/core/tests/vectors.rs`): COBS, CRC, frames (min/max length, CRC, length and range errors per receiver, `EMPTY`, `OVERLONG`), every message (fields, body, frame; body-length errors; trailing bytes), streams (splitter byte by byte, frames and six counters), canonical state and `NEUTRAL`, constants in both directions; the seven files are all present (and no other `.txt`), minimum case counts as F.9, every message type has an ok case, every `proto_version` header equals `ocb_core::protocol::consts::PROTO_VERSION`; strict parser | all pass on ubuntu, windows, macos | AD-14, AD-3, AD-10 |
| C.2 | Property round trips (proptest): COBS encode/decode for 0..=600 random bytes (no `00` in the output, size bound), frames for every type of both receive ranges with bodies 0..=255, splitter byte by byte equal to random chunking, no panic on arbitrary input to `decode_frame` and every message decoder | all pass | AD-3, AD-14 |
| C.3 | Codec API edges: body of 256 bytes refused, STATUS `extra` over 244 bytes refused, every prefix of a valid frame is an error (no panic), state predicates, unknown NAK reasons / EVENT codes / modes / profile ids round-trip | all pass | AD-6, AD-19 |
| C.4 | Core purity gate (`app/core/tests/purity.rs`): no I/O, thread, clock, process, env, `unsafe`, FFI or printing token in `app/core/src/`; `[dependencies]` empty, no egui/eframe/serialport; every bad fixture flagged, good fixture and the real tree pass | all pass | AD-18 |
| C.5 | App version (`version_parse`): the `git describe` forms of `ocb_version.cmake` give the same strings (`X.Y.Z`, `X.Y.Z-N-gHASH`, `-dirty`), malformed or absent → `0.0.0-dev`, components > 255 → fallback | all pass | AD-14 |

CI gates that are not `cargo test` rows: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo deny check licenses` (all three OS).

### Architecture compliance (checklist)

- AD-3: receive order COBS → length (as CRC) → CRC → type range. Seq and semantics are **not** in the codec (Story 3.3).
- AD-7 frozen items: frame layout, COBS/CRC rules, types `0x01`/`0x81`, HELLO body, INFO byte 0, HEARTBEAT prefix. HELLO, INFO and HEARTBEAT decoders accept trailing bytes.
- AD-10: one `NEUTRAL` in the Rust codebase (`protocol::state::NEUTRAL`), checked against `state.txt` `neutral`.
- AD-14: vectors win over `docs/link-protocol.md`; a wire change edits the vectors first. This story **must not edit any vector file** (if a vector looks wrong, stop and ask the owner, per the epic instructions).
- AD-18: core crate has no egui/eframe; enforced by C.4.
- AD-21: licence gate in CI; the app opens no network connections (nothing in this story does).
- Conventions: little-endian, µs in u32, versions `[u8; 3]`, core returns typed results.

### File structure

```text
.gitattributes                         NEW  (vectors eol=lf)
.gitignore                             UPDATE (app/target/)
.github/workflows/app.yml              NEW
app/Cargo.toml                         NEW  workspace
app/Cargo.lock                         NEW  committed
app/rust-toolchain.toml                NEW  1.100.0
app/deny.toml                          NEW  NFR-07 allow-list
app/README.md                          UPDATE
app/core/Cargo.toml                    NEW  ocb-core
app/core/build.rs                      NEW  git describe → OCB_APP_VERSION
app/core/src/lib.rs                    NEW
app/core/src/version.rs                NEW
app/core/src/version_parse.rs          NEW  (shared with build.rs)
app/core/src/protocol/{mod,consts,cobs,crc16,frame,rx,state,msgs}.rs  NEW
app/core/tests/{vectors,props,purity}.rs                               NEW
app/core/tests/fixtures/purity/*.rs.txt                                NEW
docs/test-plan.md                      UPDATE (C.1–C.5)
firmware/common/proto/vectors/README.md UPDATE (Rust test path)
```

`app/profiles/` stays where it is (bundled profiles, AD-17); it is not a crate.

### Library / framework requirements

- Rust **1.100.0** (released 2026-11-09 per `rustc +1.100.0 --version`: `rustc 1.100.0 (0ef1bc386 2026-11-09)`), edition 2024, resolver 3. Installed locally with `rustup toolchain install 1.100.0` (2026-11-24). On this Mac rustup is Homebrew's: its proxies are in `/opt/homebrew/opt/rustup/bin`, which is **not on PATH** by default; prepend it (`export PATH=/opt/homebrew/opt/rustup/bin:$PATH`) in each shell command.
- `proptest = "=1.12.0"` (dev only).
- `cargo-deny` 0.22.0; config format per current docs (`[licenses] allow`, `confidence-threshold`, `unused-allowed-license`; checked with ctx7 2026-11-24).
- Never add egui/eframe/serialport here (Epic 4 / Story 3.4).

### Previous story intelligence (Epic 2, retro 2026-11-24)

- **Red proof before trust**: every new gate and test was shown failing once. Do it for C.1 (three red cases), C.4 (fixtures) and the licence gate.
- **Numbers and claims are checked against logs before writing** (Story 2.7 H.12 was recorded wrong once). Paste the actual `cargo test` summary lines into the Dev Agent Record.
- Story 2.2 D12/D13/D18 define the splitter and direct-decode edge cases (overlong counted once at the 263rd byte; malformed COBS wins over overflow; `EMPTY`/`OVERLONG` are vector error names). Story 2.2 D14: `trailing` and STATUS `extra=`.
- Open issue #20 (C purity gates miss `exit`, `abort`, `alloca`, …): the Rust gate should not repeat that gap — ban `std::process` entirely (covers `exit` and `abort`).
- Decisions are recorded as numbered D-entries the owner can overrule; keep that style (D1–D10 above, continue from D11 during implementation).

### Git intelligence

Recent work is docs only (Epic 2 retro, devlog #2). The last code story (2.5, PR #24) set the pattern: story file + code + `docs/test-plan.md` rows + CI steps in one PR, D-entries in the story. Firmware CI is `firmware.yml` (ubuntu only); this story adds `app.yml` as a separate workflow, so a firmware-only change still runs the app job (the vectors are shared, which is the point).

### Project context rules

No `project-context.md` exists. Rules from memory and repo practice:
- All repo content in English; chat with the owner in Italian.
- Never commit `_bmad/` or `bmad-*`/`gds-*` skills; only `_bmad-output/`.
- Branch `story/3-2-rust-workspace-and-protocol-codec-ocb-core`; PR to `main`; the owner merges.

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 3.2]
- [Source: _bmad-output/planning-artifacts/architecture/architecture-bonooru-2026-11-19/ARCHITECTURE-SPINE.md#AD-3, AD-7, AD-10, AD-14, AD-18, AD-19, AD-21, Structural Seed]
- [Source: _bmad-output/planning-artifacts/architecture/architecture-bonooru-2026-11-19/app-stack-options.md#Test strategy, crate table]
- [Source: firmware/common/proto/vectors/README.md]
- [Source: firmware/common/proto/{cobs,crc16,frame,rx,msgs,state}.c, include/ocb_proto/*.h]
- [Source: firmware/tests/test_vectors.c#kFiles]
- [Source: firmware/cmake/ocb_version.cmake]
- [Source: _bmad-output/implementation-artifacts/2-2-protocol-library-and-conformance-vectors.md#D11–D19]
- [Source: _bmad-output/implementation-artifacts/epic-2-retro-2026-11-24.md#2, #6, #9]
- [Source: _bmad-output/specs/spec-bonooru/requirements.md#NFR-07]

## Dev Agent Record

### Agent Model Used

Claude Opus 5.5 (`claude-opus-5-5`), gds-dev-story, 2026-11-24.

### Debug Log References

- `cargo test` (macOS arm64, rustc 1.100.0): 27 tests in 5 targets, all pass (lib 4, `props` 11, `purity` 4, `vectors` 7, `version` 1). `c1_conformance_vectors` prints `vectors: 239 cases in 7 files ok (proto_version 01)`.
- `cargo clippy --all-targets -- -D warnings`: clean. `cargo fmt --all --check`: clean.
- `cargo deny --locked check licenses` (cargo-deny 0.22.0): `licenses ok`, with proptest's tree included; no licence outside the NFR-07 list was needed (Task 8.2: nothing to flag).
- Red proofs (each run once, then reverted; outputs checked in the terminal):
  - Rust constant `ACK_TIMEOUT_MS` 0x32 → 0x33: `1 vector failure(s): constants.txt:142: ACK_TIMEOUT_MS: vector 0x32 != Rust 0x33`.
  - COBS decoder bug (implied zero after a `0xFF` block): 6 failures (`cobs.txt` `nonzero_255`, `nonzero_254_then_zero`, `max_raw_nonzero`, `frames.txt` `max_nonzero_body`, …).
  - HELLO decoder made strict (`== 1`): `messages.txt:18: hello_trailing: decode: message body has the wrong length`.
  - Vector-side breakage is now permanent negative tests in `tests/vectors.rs` (`c1_red_*`: CRC byte, `proto_version` header, constant changed, constant removed, unknown field plus an unlisted `.txt`, message wire byte), each on a temp copy of the vectors.
  - Purity gate: its first run on the real tree flagged the `println!` of the version test in `src/version.rs` (moved to `tests/version.rs`); fixtures: 18 bad (one per banned token) flagged, the good one passes.
  - Licence gate: a scratch copy of the workspace with `MIT` removed from `allow` → `error[rejected]: failed to satisfy license requirements`, `licenses FAILED`, exit 4.
- Version: no `v*` tag exists, so `OCB_APP_VERSION=0.0.0-dev`. With a temporary local tag `v0.3.0` (deleted right after): `OCB_APP_VERSION=0.3.0-dirty` (uncommitted work in the tree), and back to `0.0.0-dev` after the delete, so `build.rs` reruns on tag changes.

### Decisions made during implementation (D1 to D10 followed; additions below)

- **D11. `consts.rs` generated once from `constants.txt`, cross-checked with `consts.h`.** A throwaway script wrote the 117 constants; a second script compared every value with the `#define`s of `consts.h` (all equal). Both-direction checking by C.1 then guards any later drift. The `wire_consts!` macro defines each constant and the `ALL` table in one place, so the two cannot disagree.
- **D12. `HidProfile::NotKnown` for id 0x00** (`HID_PROFILE_UNKNOWN`), because `Unknown(u8)` is the "value this app does not know" variant shared by all code enums (D3).
- **D13. `Splitter::push` returns `Dropped(Overlong)` at the delimiter that closes an overlong run** (without counting it again), mirroring `ocb_rx_push`'s `OCB_RX_DROPPED` + `last_err`.
- **D14. `MsgError::TooLong`** added for the STATUS `extra` limit (encode side); `Message::encode_frame` maps a frame-level `BodyTooLong` to it.
- **D15. `version_parse` is compiled into the library only under `cfg(test)`**; `build.rs` includes the same file through `#[path]`. The runtime crate exposes only `version::APP_VERSION` and `APP_VERSION_TRIPLE` (parsed by a `const fn` from the `build.rs` env values). The print-the-version test lives in `tests/version.rs` because the purity gate bans `println!` in `src/`.
- **D16. Clippy panic lints allowed per test file** (`#![allow(clippy::unwrap_used, …)]` at the top of each `tests/*.rs`) and the in-module unit tests use `assert!` only; no `clippy.toml`.
- **D17. Purity gate token matching on identifier boundaries**, so `#![forbid(unsafe_code)]` is not a hit for `unsafe` and `reprint!` is not a hit for `print!`; checked by `c4_good_fixture_passes`.

### Completion Notes List

- AC1: `app/` is a Cargo workspace (resolver 3, edition 2024) pinned to Rust 1.100.0; `ocb-core` has no runtime dependency and no egui/eframe, threads or I/O (C.4). `protocol` provides COBS, CRC-16/CCITT-FALSE, frame encode/decode in the AD-3 order, the receive splitter, every message type (typed, with `Unknown(u8)` codes), the canonical state and `NEUTRAL`.
- AC2: C.1 reads all seven vector files and passes 239 cases; `PROTO_VERSION` equals every header; proptest round-trips COBS, frames, the splitter and messages (C.2); edges in C.3.
- AC3: `.github/workflows/app.yml` runs fmt, `cargo test`, clippy `-D warnings` and `cargo deny check licenses` on ubuntu-24.04, windows-latest and macos-latest; the app version comes from `git describe` with fallback `0.0.0-dev` (C.5). `.gitattributes` keeps the vectors LF on Windows.
- No vector file was edited. No AC mismatch found.

### File List

- `.gitattributes` (new)
- `.gitignore` (modified)
- `.github/workflows/app.yml` (new)
- `app/Cargo.toml` (new)
- `app/Cargo.lock` (new)
- `app/rust-toolchain.toml` (new)
- `app/deny.toml` (new)
- `app/README.md` (modified)
- `app/core/Cargo.toml` (new)
- `app/core/build.rs` (new)
- `app/core/src/lib.rs` (new)
- `app/core/src/version.rs` (new)
- `app/core/src/version_parse.rs` (new)
- `app/core/src/protocol/mod.rs` (new)
- `app/core/src/protocol/consts.rs` (new)
- `app/core/src/protocol/cobs.rs` (new)
- `app/core/src/protocol/crc16.rs` (new)
- `app/core/src/protocol/frame.rs` (new)
- `app/core/src/protocol/rx.rs` (new)
- `app/core/src/protocol/state.rs` (new)
- `app/core/src/protocol/msgs.rs` (new)
- `app/core/tests/vectors.rs` (new)
- `app/core/tests/props.rs` (new)
- `app/core/tests/purity.rs` (new)
- `app/core/tests/version.rs` (new)
- `app/core/tests/fixtures/purity/good.rs.txt` (new)
- `app/core/tests/fixtures/purity/bad/*.rs.txt` (new, 18 files)
- `docs/test-plan.md` (modified: C.1 to C.5)
- `firmware/common/proto/vectors/README.md` (modified: Rust test path and list)
- `_bmad-output/implementation-artifacts/sprint-status.yaml` (modified)
- `_bmad-output/implementation-artifacts/3-2-rust-workspace-and-protocol-codec-ocb-core.md` (this file)

## Change Log

- 2026-11-24: Story 3.2 implemented (workspace, `ocb-core` protocol codec, C.1 to C.5, app CI, licence gate). Status → review.
