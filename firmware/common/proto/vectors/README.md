# Conformance vectors

Language-neutral test vectors for the Bonooru link protocol (AD-14). The firmware host
test F.9 (`firmware/tests/test_vectors.c`) and the app's Rust codec tests (Story 3.2) read
these files as shipped. **A wire change edits these files first**, then the code.

Numeric values live in `firmware/common/proto/include/ocb_proto/consts.h` and are mirrored
in `constants.txt`; those two win over `docs/link-protocol.md` on a disagreement.

## Files

One file per area. The list is fixed: adding an area means adding its file **and** its
entry in `kFiles[]` in `firmware/tests/test_vectors.c` (the test also fails if the number
of `.txt` files here differs from that list).

| File | What it pins |
|---|---|
| `constants.txt` | every wire constant (message types, NAK reasons, EVENT codes, modes, profile ids, flag and button bits, hat values, sizes, CRC parameters, timing) |
| `cobs.txt` | COBS encode/decode, including the `0xFF` block cases and decode failures |
| `crc16.txt` | CRC-16/CCITT-FALSE (check `29b1`), sent little-endian |
| `frames.txt` | whole frames: min/max length, CRC and length errors, type ranges per receiver |
| `messages.txt` | every message body, its fields and its frame; body-length errors |
| `streams.txt` | the receive splitter on byte streams: delimiters, empty frames, resync, overlong runs, counters |
| `state.txt` | the canonical 8-byte state and `NEUTRAL` |

## Format

1. ASCII, `\n` line endings, no tabs. A line is blank, a **comment** (first non-blank
   character `#`), or `key: value`. Split at the **first** `:` and trim spaces around key
   and value. No escaping, no quoting, no multi-line values, no trailing comments (a `#`
   inside a value is an error).
2. The **first** `key: value` line of every file is `proto_version: 01` (two hex digits).
   It must equal `OCB_PROTO_VERSION` (checked by F.9 and by
   `firmware/tools/check-proto-version.sh`, also a CI step).
3. Values are **hex bytes in wire order**: pairs of hex digits (lower case preferred),
   optional single spaces between bytes, an empty value means zero bytes. Exceptions are
   named per field below: `err`, `rx`, `fields`, `frames`, `stats`, and the whole of
   `constants.txt`.
4. A **case** is the group of consecutive lines whose key is `<case>.<field>` (split at the
   last `.`). Case names are `[a-z0-9_]+` and unique per file. Parsers reject unknown
   fields, duplicate fields and missing required fields; F.9 also enforces a minimum
   number of cases per file.
5. `err` is one of `COBS`, `CRC`, `RANGE`, `BODY_LEN` (the C names without `OCB_ERR_`).
   `rx` is `CONTROLLER` (accepts types `01..7f`, the default) or `APP` (accepts `80..ff`).
6. `fields` is a space-separated list of `name=value`: numbers are **big-endian hex of
   their full width** (u8 = 2 digits, u16 = 4, u32 = 8), byte arrays and the nested
   `state` are hex in wire order without spaces. Names follow `docs/link-protocol.md`.

## Per-file fields

| File | Fields | Check |
|---|---|---|
| `constants.txt` | `NAME: <big-endian hex number>`, no dots; `NAME` is the C macro without `OCB_` | every line matches the macro of that name, and every macro on F.9's list appears |
| `cobs.txt` | `raw`, `enc` (no delimiter); or `enc`, `err` | `encode(raw) == enc`, `decode(enc) == raw`; or `decode(enc)` fails |
| `crc16.txt` | `data`, `crc` (2 bytes, little-endian) | `crc16(data)` as LE bytes equals `crc` |
| `frames.txt` | `payload` (`type seq body`), `wire` (COBS, no delimiter), optional `rx`; or `wire`, `err`, optional `rx`, optional `payload` | `encode(payload) == wire` and `decode(wire, rx) == payload`; or `decode(wire, rx)` fails with `err` (and, with `payload`, `encode(payload) == wire`) |
| `messages.txt` | `type`, `seq`, `fields`, `body`, `wire`, optional `trailing`; or `type`, `body`, `err` | `encode(fields) + trailing == body`, `decode(body) == fields`, `encode_frame(type, seq, body) == wire`; or `decode(body)` fails with `BODY_LEN`. `trailing` = bytes after the v1 body that the decoder ignores (STATUS exposes them as `extra=` instead) |
| `streams.txt` | `in` (bytes including delimiters), optional `rx`, `frames` (`;`-separated payloads `type seq body`, may be empty), `stats` (`ok=N empty=N cobs=N crc=N range=N overlong=N`, decimal, all six in this order) | feed `in` byte by byte; delivered frames and final counters must match |
| `state.txt` | `bytes` (8 bytes), `fields` (`buttons=XXXX hat=XX lx=XX ly=XX rx=XX ry=XX flags=XX`) | `decode(bytes) == fields`, `encode(fields) == bytes`; the `neutral` case also equals `NEUTRAL` |

## Receive rules the vectors pin

- Check order (AD-3): COBS, then length consistency (`decoded >= 5` and
  `decoded == 3 + len + 2`, a failure is a **CRC** error), then CRC, then type range. Seq
  and semantic checks belong to the consumers.
- An **empty frame** is a zero-length run between two delimiters (or a leading `00`): it
  is counted in `empty` and dropped, never an error. An encoded `01` decodes to zero bytes:
  that is a too-short frame, a CRC error.
- A run of more than 262 (`MAX_ENCODED_FRAME`) non-delimiter bytes is **overlong**: counted
  once, the rest of the run is discarded up to and including the next `00`.
- Every delimiter that ends a run increments exactly one of `ok`, `empty`, `cobs`, `crc`,
  `range` (an overlong run is counted in `overlong` instead).

## How the hex was produced

Every value was computed by an implementation written separately from the C library (a
throwaway script, not part of the repo) and anchored on external references: the RevEng
CRC check value `29b1` and the published COBS examples (Cheshire and Baker; the Wikipedia
table). The C library then reproduces every vector.
