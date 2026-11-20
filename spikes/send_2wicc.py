# /// script
# requires-python = ">=3.10"
# dependencies = ["pyserial>=3.5"]
# ///
"""Send a file of timed controller states to the 2wiCC queue over the bridge port.

SPDX-License-Identifier: MIT (the project licence, see LICENSE at the repo root).

THROWAWAY SPIKE CODE. Not shipped, not part of app/ or firmware/, and not a
template for the app. It exists so Stories 1.3, 1.4 and 1.6 can drive the
console before our own firmware exists.

2wiCC and 2wiCC_Comms binaries/UF2 are used for testing only and are NEVER
committed to this repo. No upstream code is copied here: the behaviour below is
written from the documented protocol (see spikes/README.md).

Upstream sources (read only):
  https://github.com/knflrpn/2wiCC          (command set, state format)
  https://github.com/knflrpn/2wiCC_Comms    (USB-serial bridge)
  https://github.com/dekuNukem/Nintendo_Switch_Reverse_Engineering/blob/master/bluetooth_hid_notes.md
                                            (Pro Controller button bytes, stick packing)

Usage: uv run spikes/send_2wicc.py --port PORT FILE
       uv run spikes/send_2wicc.py --dry-run FILE
       uv run spikes/send_2wicc.py --list-ports
       uv run spikes/send_2wicc.py --port PORT --announce FILE   # print row labels as they play
"""

from __future__ import annotations

import argparse
import math
import re
import signal
import sys
import time
from collections.abc import Callable, Iterable, Sequence
from dataclasses import dataclass

# ---------------------------------------------------------------------------
# Canonical state (AD-10). Input files speak this; only the encoder knows 2wiCC.
# ---------------------------------------------------------------------------

# Canonical button bits (AD-10 / docs/usb-controller-emulation.md P1 table).
CANON_BUTTONS = {
    "Y": 1 << 0,
    "B": 1 << 1,
    "A": 1 << 2,
    "X": 1 << 3,
    "L": 1 << 4,
    "R": 1 << 5,
    "ZL": 1 << 6,
    "ZR": 1 << 7,
    "MINUS": 1 << 8,
    "PLUS": 1 << 9,
    "L3": 1 << 10,
    "R3": 1 << 11,
    "HOME": 1 << 12,
    "CAPTURE": 1 << 13,
}
CANON_BUTTON_MASK = (1 << 14) - 1

HAT_NEUTRAL = 8
HAT_NAMES = {
    "up": 0, "up-right": 1, "right": 2, "down-right": 3,
    "down": 4, "down-left": 5, "left": 6, "up-left": 7, "neutral": 8,
}

STICK_CENTRE_8 = 0x80


@dataclass(frozen=True)
class Stick:
    """A stick position.

    raw=False: x/y are canonical 8-bit values (0x80 centre, y 0x00 = up).
    raw=True:  x/y are 12-bit wire values sent unchanged (0x800 centre).
    """

    x: int = STICK_CENTRE_8
    y: int = STICK_CENTRE_8
    raw: bool = False


@dataclass(frozen=True)
class State:
    buttons: int = 0
    hat: int = HAT_NEUTRAL
    stick: Stick = Stick()


NEUTRAL = State()  # the one and only neutral (AD-10)

# ---------------------------------------------------------------------------
# 2wiCC wire format (2wiCC README "Controller State Format"; identical to the
# Pro Controller standard report bytes 3/4/5 in dekuNukem bluetooth_hid_notes.md)
# ---------------------------------------------------------------------------

# Byte 1 (right side): bit order [ZR, R, SL, SR, A, B, X, Y] (bits 7..0), 2wiCC README
W1_Y = 0x01
W1_X = 0x02
W1_B = 0x04
W1_A = 0x08
W1_SR = 0x10
W1_SL = 0x20
W1_R = 0x40
W1_ZR = 0x80
# Byte 2 (shared): [-, -, Capture, Home, L stick, R stick, +, -] (bits 7..0), 2wiCC README
W2_MINUS = 0x01
W2_PLUS = 0x02
W2_RSTICK = 0x04
W2_LSTICK = 0x08
W2_HOME = 0x10
W2_CAPTURE = 0x20
# Byte 3 (left side): [ZL, L, SL, SR, D-left, D-right, D-up, D-down] (bits 7..0), 2wiCC README
W3_DDOWN = 0x01
W3_DUP = 0x02
W3_DRIGHT = 0x04
W3_DLEFT = 0x08
W3_SR = 0x10
W3_SL = 0x20
W3_L = 0x40
W3_ZL = 0x80

# Canonical button -> (wire byte index 0..2, mask)
CANON_TO_WIRE = {
    "Y": (0, W1_Y), "B": (0, W1_B), "A": (0, W1_A), "X": (0, W1_X),
    "L": (2, W3_L), "R": (0, W1_R), "ZL": (2, W3_ZL), "ZR": (0, W1_ZR),
    "MINUS": (1, W2_MINUS), "PLUS": (1, W2_PLUS), "L3": (1, W2_LSTICK),
    "R3": (1, W2_RSTICK), "HOME": (1, W2_HOME), "CAPTURE": (1, W2_CAPTURE),
}

# Canonical hat 0..8 -> D-pad bits in byte 3 (diagonals set two bits).
HAT_TO_DPAD = {
    0: W3_DUP,
    1: W3_DUP | W3_DRIGHT,
    2: W3_DRIGHT,
    3: W3_DDOWN | W3_DRIGHT,
    4: W3_DDOWN,
    5: W3_DDOWN | W3_DLEFT,
    6: W3_DLEFT,
    7: W3_DUP | W3_DLEFT,
    8: 0,
}

# Sticks: 12 bits per axis, centre 0x800, full swing +-0x600 (2wiCC README).
STICK_CENTRE_12 = 0x800
STICK_SWING_12 = 0x600
STICK_MIN_12 = STICK_CENTRE_12 - STICK_SWING_12  # 0x200
STICK_MAX_12 = STICK_CENTRE_12 + STICK_SWING_12  # 0xE00
# OQ-2, confirmed on hardware 2026-11-20: higher 12-bit Y = stick up (Pro
# Controller convention), so canonical ly 0x00 (up) maps to a high Y.
# Raw 12-bit input is never flipped.
HIGHER_Y_IS_UP = True

# Commands (2wiCC README "Command Reference"): "+", type, a required space,
# optional parameter, newline to commit.
EOL = "\n"
CMD_BUFFERED = "+SPM BUF"
CMD_REALTIME = "+SPM RT"
CMD_NEUTRAL = "+QD 000000"
PREAMBLE = (CMD_BUFFERED,)
STOP_SEQUENCE = (CMD_REALTIME, CMD_NEUTRAL)  # OQ-8: RT drops the backlog, then neutral

MAX_FRAMES = 60 * 60 * 60  # 1 hour at 60 Hz: longer files are rejected, not built in memory
QUEUE_CAPACITY = 1023      # 1024-slot ring, one slot always free (2wiCC.c)
SERIAL_CMD_BUFFER = 64     # firmware serial command ring (2wiCC.h)
MAX_LEAD_LIMIT = 48        # prefill burst + preamble stays well under 64 commands


class ParseError(ValueError):
    pass


class Terminated(Exception):
    """SIGTERM or SIGHUP arrived; handled like Ctrl+C (stop sequence, then exit)."""

    def __init__(self, signum: int):
        super().__init__(f"signal {signum}")
        self.signum = signum


class Underrun(RuntimeError):
    """The host fell behind: the console would have run out of queued frames."""


# ---------------------------------------------------------------------------
# Parser (pure)
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class Time:
    value: float
    frames: bool  # True: value is a frame count ("3f"); False: milliseconds


@dataclass(frozen=True)
class Row:
    line: int
    time: Time
    state: State
    label: str = ""  # the row's `# comment`, shown only with --announce


_FRAMES_RE = re.compile(r"[0-9]+[fF]")
_MS_RE = re.compile(r"[0-9]+(\.[0-9]*)?")
_HEX_RE = re.compile(r"[0-9A-Fa-f]+")


def is_hex(s: str) -> bool:
    """Pure ASCII hex digits only: no sign, no 0x, no whitespace, no underscores."""
    return _HEX_RE.fullmatch(s) is not None


MAX_TIME_TOKEN = 32  # characters; far more than 1 hour needs


def parse_time(tok: str) -> Time:
    if len(tok) <= MAX_TIME_TOKEN:
        try:
            if _FRAMES_RE.fullmatch(tok):
                return Time(int(tok[:-1]), True)
            if _MS_RE.fullmatch(tok):
                v = float(tok)
                if math.isfinite(v):
                    return Time(v, False)
        except (ValueError, OverflowError):
            pass
    raise ParseError(f"bad time {tok!r} (milliseconds like 500 or 12.5, or frames like 3f)")


def parse_buttons(tok: str) -> int:
    if tok == "-":
        return 0
    if tok.lower().startswith("0x"):
        if not is_hex(tok[2:]):
            raise ParseError(f"bad button hex {tok!r}")
        v = int(tok, 0)  # prefix checked above: 0x + pure hex digits
        if v & ~CANON_BUTTON_MASK:
            raise ParseError(f"button hex {tok!r} sets bits above bit 13")
        return v
    mask = 0
    for name in tok.split("+"):
        key = name.strip().upper()
        if key not in CANON_BUTTONS:
            known = " ".join(CANON_BUTTONS)
            raise ParseError(f"unknown button {name!r} (known: {known})")
        mask |= CANON_BUTTONS[key]
    return mask


def parse_hat(tok: str) -> int:
    if tok == "-":
        return HAT_NEUTRAL
    if re.fullmatch(r"[0-9]+", tok):
        v = int(tok)
        if 0 <= v <= 8:
            return v
        raise ParseError(f"hat {tok!r} out of range 0..8")
    key = tok.lower()
    if key in HAT_NAMES:
        return HAT_NAMES[key]
    raise ParseError(f"unknown hat {tok!r} (0..8 or {' '.join(HAT_NAMES)})")


def parse_stick(tok: str) -> Stick:
    if tok == "-":
        return Stick()
    parts = tok.split(":")
    if len(parts) != 2 or len(parts[0]) != len(parts[1]) or len(parts[0]) not in (2, 3):
        raise ParseError(f"bad stick {tok!r} (lx:ly as 2 hex digits each, or raw 12-bit as 3 hex digits each)")
    if not (is_hex(parts[0]) and is_hex(parts[1])):
        raise ParseError(f"bad stick hex {tok!r}")
    x, y = int(parts[0], 16), int(parts[1], 16)
    return Stick(x, y, raw=len(parts[0]) == 3)


def parse_lines(lines: Iterable[str]) -> list[Row]:
    """Parse input rows: `time [buttons [hat [stick]]]`, `#` starts a comment."""
    rows: list[Row] = []
    for n, raw in enumerate(lines, start=1):
        text, _, comment = raw.partition("#")
        text = text.strip()
        if not text:
            continue
        toks = text.split()
        if len(toks) > 4:
            raise ParseError(f"line {n}: too many columns (time buttons hat stick)")
        toks += ["-"] * (4 - len(toks))
        try:
            t = parse_time(toks[0])
            state = State(parse_buttons(toks[1]), parse_hat(toks[2]), parse_stick(toks[3]))
        except ParseError as e:
            raise ParseError(f"line {n}: {e}") from None
        rows.append(Row(n, t, state, comment.strip()))
    return rows


# ---------------------------------------------------------------------------
# Encoder (pure)
# ---------------------------------------------------------------------------


def stick_axis_to_12(v8: int, invert: bool) -> int:
    """Canonical 8-bit axis (0x80 centre) -> 12-bit wire value in 0x200..0xE00."""
    d = v8 - STICK_CENTRE_8
    if invert:
        d = -d
    v = STICK_CENTRE_12 + round(d * STICK_SWING_12 / 0x80)
    return max(STICK_MIN_12, min(STICK_MAX_12, v))


def stick_to_12(stick: Stick) -> tuple[int, int]:
    if stick.raw:
        return stick.x & 0xFFF, stick.y & 0xFFF
    return stick_axis_to_12(stick.x, False), stick_axis_to_12(stick.y, HIGHER_Y_IS_UP)


def pack_stick(x12: int, y12: int) -> bytes:
    """Pro Controller packing (dekuNukem): X = b0 | (b1 & 0xF) << 8, Y = b1 >> 4 | b2 << 4."""
    return bytes((x12 & 0xFF, ((x12 >> 8) & 0x0F) | ((y12 & 0x0F) << 4), (y12 >> 4) & 0xFF))


def digital_bytes(state: State) -> bytes:
    b = [0, 0, 0]
    for name, bit in CANON_BUTTONS.items():
        if state.buttons & bit:
            idx, mask = CANON_TO_WIRE[name]
            b[idx] |= mask
    b[2] |= HAT_TO_DPAD[state.hat]
    return bytes(b)


def stick_is_centred(stick: Stick) -> bool:
    return stick_to_12(stick) == (STICK_CENTRE_12, STICK_CENTRE_12)


def encode(state: State) -> str:
    """Canonical state -> one 2wiCC queue command (no line terminator)."""
    digital = digital_bytes(state).hex().upper()
    if stick_is_centred(state.stick):
        return f"+QD {digital}"
    left = pack_stick(*stick_to_12(state.stick)).hex().upper()
    right = pack_stick(STICK_CENTRE_12, STICK_CENTRE_12).hex().upper()
    return f"+QF {digital}{left}{right}"


# ---------------------------------------------------------------------------
# Scheduler (pure)
# ---------------------------------------------------------------------------


def time_to_frame(t: Time, frame_rate: float) -> int:
    if t.frames:
        return int(t.value)
    return math.floor(t.value * frame_rate / 1000.0 + 0.5)


def schedule(rows: Sequence[Row], frame_rate: float) -> tuple[list[State], list[Row]]:
    """Rows -> one state per frame.

    A row is in effect from its frame until the next row's frame. Frames before
    the first row are neutral. The last row is held for one frame, then the run
    ends. Returns (frames, rows that got zero frames because of rounding).
    """
    if not rows:
        return [], []
    starts = [time_to_frame(r.time, frame_rate) for r in rows]
    for i in range(1, len(rows)):
        if starts[i] < starts[i - 1]:
            r = rows[i]
            raise ParseError(f"line {r.line}: time goes backwards (rows must be in non-decreasing time order)")
    for r, start in zip(rows, starts):
        if start >= MAX_FRAMES:
            raise ParseError(f"line {r.line}: time is beyond the {MAX_FRAMES}-frame limit (1 hour at 60 Hz)")
    frames: list[State] = [NEUTRAL] * starts[0]
    dropped: list[Row] = []
    for i, row in enumerate(rows):
        end = starts[i + 1] if i + 1 < len(rows) else starts[i] + 1
        if end == starts[i]:
            dropped.append(row)
        frames.extend([row.state] * (end - starts[i]))
    return frames, dropped


# ---------------------------------------------------------------------------
# Pacer (I/O injected: write, clock, sleep)
# ---------------------------------------------------------------------------


def lines_to_bytes(lines: Iterable[str]) -> bytes:
    return "".join(line + EOL for line in lines).encode("ascii")


class Pacer:
    """Keep 0 < (frames sent - frames played) <= max_lead while playing.

    Frames played is estimated from the host clock (frame_rate x elapsed).
    The estimate runs at most one frame ahead of the console (the first state
    plays at the next 2wiCC frame tick), so the real lead is never smaller.
    """

    def __init__(self, write: Callable[[bytes], object], clock: Callable[[], float],
                 sleep: Callable[[float], object], frame_rate: float = 60.0, max_lead: int = 30,
                 announce: Sequence[tuple[int, str]] = (), emit: Callable[[str], object] | None = None):
        if not 1 <= max_lead <= MAX_LEAD_LIMIT:
            raise ValueError(f"max_lead must be 1..{MAX_LEAD_LIMIT}")
        self.write = write
        self.clock = clock
        self.sleep = sleep
        self.frame_rate = frame_rate
        self.max_lead = max_lead
        self.sent = 0
        self.max_seen_lead = 0
        self.min_seen_lead: int | None = None
        # --announce: (frame, label) pairs in frame order, printed once the frame is played.
        self.announce = list(announce)
        self.emit = emit if emit is not None else (lambda text: print(text, flush=True))
        self._next_announce = 0

    def _announce_played(self, played: int) -> None:
        while self._next_announce < len(self.announce) and self.announce[self._next_announce][0] <= played:
            frame, label = self.announce[self._next_announce]
            self.emit(f"t={frame / self.frame_rate:.1f}s  {label}")
            self._next_announce += 1

    def played(self, t0: float) -> int:
        return int((self.clock() - t0) * self.frame_rate)

    def run(self, commands: Sequence[str]) -> None:
        n = len(commands)
        if n == 0:
            return
        period = 1.0 / self.frame_rate
        first = min(self.max_lead, n)
        t0 = self.clock()
        self.write(lines_to_bytes([*PREAMBLE, *commands[:first]]))
        self.sent = first
        self.max_seen_lead = first
        while self.sent < n:
            played = self.played(t0)
            if self.announce:
                self._announce_played(played)
            lead = self.sent - played
            self.min_seen_lead = lead if self.min_seen_lead is None else min(self.min_seen_lead, lead)
            if lead <= 0:
                raise Underrun(f"host fell behind at frame {self.sent}/{n} (lead {lead}); stopping")
            target = min(n, played + self.max_lead)
            if target > self.sent:
                self.write(lines_to_bytes(commands[self.sent:target]))
                self.sent = target
                self.max_seen_lead = max(self.max_seen_lead, self.sent - played)
            self.sleep(period / 2)
        # Let the queue drain: the last frame plus one tick of start offset.
        end = t0 + (n + 1) * period
        while (left := end - self.clock()) > 0:
            if self.announce:
                self._announce_played(self.played(t0))
            self.sleep(max(0.001, min(left, period)))
        if self.announce:
            self._announce_played(n)


def send_stop(write: Callable[[bytes], object], attempts: int = 3) -> bool:
    """Send the stop sequence; a Ctrl+C or write error during it is retried."""
    data = lines_to_bytes(STOP_SEQUENCE)
    for _ in range(attempts):
        try:
            write(data)
            return True
        except KeyboardInterrupt:
            continue
        except Exception as e:  # noqa: BLE001 - best effort on the way out
            print(f"warning: stop sequence write failed: {e}", file=sys.stderr)
    return False


STOP_SIGNALS = tuple(getattr(signal, n) for n in ("SIGINT", "SIGTERM", "SIGHUP") if hasattr(signal, n))


def _raise_terminated(signum, _frame):
    raise Terminated(signum)


def _set_handlers(handlers: dict) -> dict:
    """Install {signum: handler}; returns the previous handlers. No-op outside the main thread."""
    old = {}
    for signum, handler in handlers.items():
        if handler is None:  # handler not set from Python; nothing to restore
            continue
        try:
            old[signum] = signal.signal(signum, handler)
        except (ValueError, TypeError):  # not in the main thread
            pass
    return old


def play(commands: Sequence[str], write: Callable[[bytes], object], clock, sleep,
         frame_rate: float, max_lead: int, stats: dict | None = None,
         announce: Sequence[tuple[int, str]] = (), emit: Callable[[str], object] | None = None) -> int:
    """Run the pacer; always send the stop sequence. Returns the exit code.

    If `stats` is given, stats["sent"] is set to the frames actually queued.
    """
    code = 1
    pacer = Pacer(write, clock, sleep, frame_rate, max_lead, announce, emit)
    originals = {s: signal.getsignal(s) for s in STOP_SIGNALS}
    _set_handlers({s: _raise_terminated for s in STOP_SIGNALS if s != signal.SIGINT})
    try:
        pacer.run(commands)
        code = 0
    except KeyboardInterrupt:
        print("interrupted: sending neutral", file=sys.stderr)
        code = 130
    except Terminated as e:
        print(f"terminated ({signal.Signals(e.signum).name}): sending neutral", file=sys.stderr)
        code = 128 + e.signum
    except Exception as e:  # noqa: BLE001
        print(f"error: {e}", file=sys.stderr)
        code = 1
    finally:
        # First statement: ignore Ctrl+C, SIGTERM and SIGHUP so a second signal
        # cannot skip the stop sequence.
        _set_handlers({s: signal.SIG_IGN for s in STOP_SIGNALS})
        try:
            if stats is not None:
                stats["sent"] = pacer.sent
            if not send_stop(write):
                print("error: could not send the neutral state", file=sys.stderr)
                code = code or 1
        finally:
            _set_handlers(originals)
    return code


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(
        prog="send_2wicc.py",
        description="Throwaway spike: send timed controller states to the 2wiCC queue "
                    "(bridge port, 460800 8N1). Input rows: `time buttons hat stick`, "
                    "see spikes/README.md.",
    )
    p.add_argument("file", nargs="?", help="input file of timed states")
    p.add_argument("--port", help="bridge serial port (required unless --dry-run)")
    p.add_argument("--baud", type=int, default=460800, help="baud rate (default 460800)")
    p.add_argument("--frame-rate", type=float, default=60.0,
                   help="frames per second used to schedule and pace (default 60). Must match the "
                        "2wiCC firmware timer (fixed at 16666 us = 60 Hz); other values only model a "
                        "different rate and do not change the console's playback rate")
    p.add_argument("--max-lead-frames", type=int, default=30,
                   help=f"max frames queued ahead of playback, 1..{MAX_LEAD_LIMIT} (default 30)")
    p.add_argument("--dry-run", action="store_true", help="print the lines instead of opening a port")
    p.add_argument("--list-ports", action="store_true", help="list serial ports and exit")
    p.add_argument("--announce", action="store_true",
                   help="print each row's `# label` when its first frame plays (estimated from the host "
                        "clock); with --dry-run, show the labels next to the frame lines")
    return p


def announcements(rows: Sequence[Row], frame_rate: float) -> list[tuple[int, str]]:
    """(first frame, label) for every labelled row, in frame order (input to --announce)."""
    return [(time_to_frame(r.time, frame_rate), r.label) for r in rows if r.label]


def dry_run_lines(commands: Sequence[str], frame_rate: float,
                  announce: Sequence[tuple[int, str]] = ()) -> list[str]:
    labels: dict[int, list[str]] = {}
    for frame, label in announce:
        labels.setdefault(frame, []).append(label)
    out = [f"{'pre':>6} {'':>9}  {line}" for line in PREAMBLE]
    for i, cmd in enumerate(commands):
        line = f"{i:>6} {i * 1000.0 / frame_rate:>7.1f}ms  {cmd}"
        if i in labels:
            line += "  # " + " | ".join(labels[i])
        out.append(line)
    out += [f"{'stop':>6} {'':>9}  {line}" for line in STOP_SEQUENCE]
    return out


def open_serial(port: str, baud: int):
    import serial  # imported lazily so --dry-run needs no pyserial

    # 8N1, no flow control. DTR stays asserted (pyserial default): the bridge
    # only forwards host bytes while the CDC port is "connected" (OQ-9).
    return serial.Serial(port, baudrate=baud, bytesize=8, parity="N", stopbits=1,
                         timeout=0, write_timeout=2.0, xonxoff=False, rtscts=False, dsrdtr=False)


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)

    if args.list_ports:
        from serial.tools import list_ports

        for p in sorted(list_ports.comports(), key=lambda p: p.device):
            print(f"{p.device}\t{p.description}\t{p.hwid}")
        return 0
    if not args.file:
        print("error: FILE is required", file=sys.stderr)
        return 1
    if not args.dry_run and not args.port:
        print("error: --port is required unless --dry-run", file=sys.stderr)
        return 1
    if not 1 <= args.max_lead_frames <= MAX_LEAD_LIMIT:
        print(f"error: --max-lead-frames must be 1..{MAX_LEAD_LIMIT}", file=sys.stderr)
        return 1
    if not math.isfinite(args.frame_rate) or args.frame_rate <= 0:
        print("error: --frame-rate must be a positive finite number", file=sys.stderr)
        return 1

    try:
        with open(args.file, encoding="utf-8-sig") as fh:
            rows = parse_lines(fh)
        frames, dropped = schedule(rows, args.frame_rate)
    except UnicodeDecodeError as e:
        print(f"error: {args.file}: not UTF-8 text (byte {e.start})", file=sys.stderr)
        return 1
    except (OSError, ParseError) as e:
        print(f"error: {args.file}: {e}", file=sys.stderr)
        return 1
    for r in dropped:
        print(f"warning: line {r.line}: shorter than one frame, never played", file=sys.stderr)

    commands = [encode(s) for s in frames]
    duration = len(frames) / args.frame_rate
    announce = announcements(rows, args.frame_rate) if args.announce else []

    if args.dry_run:
        for line in dry_run_lines(commands, args.frame_rate, announce):
            print(line)
        print(f"summary: {len(rows)} rows, {len(frames)} frames, {duration:.3f} s, port (dry run)")
        return 0

    try:
        ser = open_serial(args.port, args.baud)
    except Exception as e:  # noqa: BLE001
        print(f"error: cannot open {args.port}: {e}", file=sys.stderr)
        return 1

    def write(data: bytes) -> None:
        ser.write(data)
        ser.flush()

    stats: dict = {"sent": 0}
    try:
        code = play(commands, write, time.monotonic, time.sleep, args.frame_rate, args.max_lead_frames, stats,
                    announce)
    finally:
        ser.close()
    print(f"summary: {len(rows)} rows, {len(frames)} frames, {duration:.3f} s, port {args.port}, exit {code}"
          f" ({stats['sent']} frames queued before stop)")
    return code


if __name__ == "__main__":
    sys.exit(main())
