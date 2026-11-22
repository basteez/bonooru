#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""link_probe: bench sender for the Bonooru controller link (story 2.6, D18).

A small, readable bench tool, not the app and not a library. It speaks the
binary link protocol of docs/link-protocol.md over a serial port (a 3.3 V
USB-UART adapter wired to the controller's UART pins): COBS framing,
CRC-16/CCITT-FALSE, the v1 messages, and an AD-6 go-back-N sender. The
codec is written from docs/link-protocol.md, not from the C sources, and
is checked against the shared conformance vectors by ctest
(link_probe_selftest, tests/test_link_probe.py).

Needs Python 3 and, only when a port is opened, pyserial
(pip install pyserial). Nothing is sent until `hello` has been answered,
except what a subcommand says it sends (`noise`, `fill --no-hello`,
`loopback`).

Subcommands: hello, watch, fill, soak, noise, try, loopback. Run with -h.
"""

import argparse
import random
import struct
import sys
import time

# --------------------------------------------------------------- constants
# Values from docs/link-protocol.md ("Messages", "Timing constants").
PROTO_VERSION = 1
HELLO, SET_STATE, QUEUE_EVENT, START, STOP, PING = 0x01, 0x02, 0x03, 0x04, 0x05, 0x06
HEARTBEAT = 0x40
INFO, STATUS, ACK, NAK, EVENT = 0x81, 0x83, 0x84, 0x85, 0x86
TYPE_NAMES = {
    HELLO: "HELLO", SET_STATE: "SET_STATE", QUEUE_EVENT: "QUEUE_EVENT", START: "START",
    STOP: "STOP", PING: "PING", HEARTBEAT: "HEARTBEAT", INFO: "INFO", STATUS: "STATUS",
    ACK: "ACK", NAK: "NAK", EVENT: "EVENT",
}
NAK_NAMES = {
    0x01: "CRC", 0x02: "SEQ", 0x03: "FULL", 0x10: "VERSION", 0x11: "BUSY", 0x12: "ORDER",
    0x13: "BAD_STATE", 0x14: "UNKNOWN_TYPE", 0x15: "NO_LINK", 0x16: "NOT_READY", 0x17: "ABORTED",
}
EVENT_NAMES = {0x01: "PLAYBACK_DONE", 0x02: "LINK_LOST", 0x03: "LATE", 0x04: "UNDERRUN",
               0x05: "USB_SUSPENDED"}
MAX_BODY = 255
MAX_RAW_FRAME = 260
MAX_ENCODED_FRAME = 262
DEFAULT_BAUD = 460800
ACK_WINDOW = 64
ACK_TIMEOUT_S = 0.050
HELLO_RESEND_S = 0.100
NEUTRAL = bytes([0x00, 0x00, 0x08, 0x80, 0x80, 0x80, 0x80, 0x00])
BTN_A = 0x0004


class ProtoError(Exception):
    """A receive check failed; `kind` is COBS, CRC, RANGE, EMPTY, OVERLONG or BODY_LEN."""

    def __init__(self, kind):
        super().__init__(kind)
        self.kind = kind


# -------------------------------------------------------------------- COBS
def cobs_encode(raw):
    """COBS (Cheshire and Baker): no 0x00 in the output, no delimiter added."""
    out = bytearray([0])
    code_at = 0
    code = 1
    for i, b in enumerate(raw):
        if b == 0:
            out[code_at] = code
            code_at = len(out)
            out.append(0)
            code = 1
            continue
        out.append(b)
        code += 1
        if code == 0xFF and i != len(raw) - 1:
            out[code_at] = code
            code_at = len(out)
            out.append(0)
            code = 1
    out[code_at] = code
    return bytes(out)


def cobs_decode(enc):
    if len(enc) == 0:
        raise ProtoError("COBS")
    out = bytearray()
    i = 0
    while i < len(enc):
        code = enc[i]
        i += 1
        if code == 0 or code - 1 > len(enc) - i:
            raise ProtoError("COBS")
        block = enc[i:i + code - 1]
        if 0 in block:
            raise ProtoError("COBS")
        out += block
        i += code - 1
        if code != 0xFF and i < len(enc):
            out.append(0)
    return bytes(out)


# -------------------------------------------------------------------- CRC
def crc16(data):
    """CRC-16/CCITT-FALSE: poly 0x1021, init 0xFFFF, no reflection, no xorout."""
    crc = 0xFFFF
    for b in data:
        crc ^= b << 8
        for _ in range(8):
            crc = ((crc << 1) ^ 0x1021) if crc & 0x8000 else (crc << 1)
            crc &= 0xFFFF
    return crc


# ------------------------------------------------------------------ frames
def frame_encode(ftype, seq, body=b""):
    """Wire bytes of one frame, including the trailing 0x00 delimiter."""
    if len(body) > MAX_BODY:
        raise ValueError("body too long")
    raw = bytes([ftype & 0xFF, seq & 0xFF, len(body)]) + bytes(body)
    raw += struct.pack("<H", crc16(raw))
    return cobs_encode(raw) + b"\x00"


def type_in_range(ftype, side):
    if side == "CONTROLLER":
        return 0x01 <= ftype <= 0x7F
    return 0x80 <= ftype <= 0xFF


def frame_decode(enc, side="APP"):
    """Decodes one frame without its delimiter: (type, seq, body) or ProtoError.
    Check order: empty, overlong, COBS, length (a CRC error), CRC, type range."""
    if len(enc) == 0:
        raise ProtoError("EMPTY")
    if len(enc) > MAX_ENCODED_FRAME:
        raise ProtoError("OVERLONG")
    raw = cobs_decode(enc)
    if len(raw) < 5 or len(raw) > MAX_RAW_FRAME or len(raw) != 3 + raw[2] + 2:
        raise ProtoError("CRC")
    if struct.unpack("<H", raw[-2:])[0] != crc16(raw[:-2]):
        raise ProtoError("CRC")
    if not type_in_range(raw[0], side):
        raise ProtoError("RANGE")
    return raw[0], raw[1], bytes(raw[3:-2])


class Splitter:
    """Streaming receiver: feed bytes, get decoded frames; bad runs are counted."""

    def __init__(self, side="APP"):
        self.side = side
        self.buf = bytearray()
        self.discarding = False
        self.errors = {}

    def feed(self, data):
        frames = []
        for b in data:
            if b != 0:
                if self.discarding:
                    continue
                if len(self.buf) >= MAX_ENCODED_FRAME:
                    self.errors["OVERLONG"] = self.errors.get("OVERLONG", 0) + 1
                    self.discarding = True
                    self.buf.clear()
                    continue
                self.buf.append(b)
                continue
            if self.discarding:
                self.discarding = False
                continue
            try:
                frames.append(frame_decode(bytes(self.buf), self.side))
            except ProtoError as e:
                self.errors[e.kind] = self.errors.get(e.kind, 0) + 1
            self.buf.clear()
        return frames


# ---------------------------------------------------------------- messages
# Field layouts of docs/link-protocol.md, protocol v1, little-endian.
# (name, kind): u8, u16, u32, or bN = N raw bytes.
FIELDS = {
    HELLO: [("proto_ver", "u8")],
    INFO: [("proto_ver", "u8"), ("fw_ver", "b3"), ("bridge_ver", "b3"), ("hid_profile", "u8"),
           ("queue_cap", "u16"), ("app_timeout_ms", "u16"), ("report_period_us", "u16")],
    SET_STATE: [("state", "b8")],
    QUEUE_EVENT: [("t_us", "u32"), ("state", "b8")],
    START: [("lead_us", "u32")],
    STOP: [],
    PING: [],
    HEARTBEAT: [("bridge_fw_ver", "b3"), ("flags", "u8")],
    STATUS: [("free", "u16"), ("now_us", "u32"), ("last_seq", "u8"), ("mode", "u8"),
             ("abort_count", "u8"), ("last_event", "u8"), ("flags", "u8")],
    ACK: [("seq", "u8")],
    NAK: [("reason", "u8"), ("expected", "u8"), ("got", "u8")],
    EVENT: [("code", "u8")],
}
# Messages whose body may be longer than v1 (trailing bytes ignored;
# STATUS exposes them as `extra`, the debug counters).
EXTENSIBLE = {HELLO, INFO, HEARTBEAT, STATUS}
_SIZES = {"u8": 1, "u16": 2, "u32": 4}


def _size(kind):
    return _SIZES[kind] if kind in _SIZES else int(kind[1:])


def body_size(ftype):
    return sum(_size(k) for _, k in FIELDS[ftype])


def msg_encode(ftype, fields):
    out = bytearray()
    for name, kind in FIELDS[ftype]:
        v = fields[name]
        if kind == "u8":
            out += struct.pack("<B", v)
        elif kind == "u16":
            out += struct.pack("<H", v)
        elif kind == "u32":
            out += struct.pack("<I", v)
        else:
            if len(v) != _size(kind):
                raise ValueError(name)
            out += bytes(v)
    if ftype == STATUS and fields.get("extra"):
        out += bytes(fields["extra"])
    return bytes(out)


def msg_decode(ftype, body):
    size = body_size(ftype)
    if len(body) < size or (ftype not in EXTENSIBLE and len(body) != size):
        raise ProtoError("BODY_LEN")
    fields = {}
    pos = 0
    for name, kind in FIELDS[ftype]:
        n = _size(kind)
        chunk = body[pos:pos + n]
        if kind == "u8":
            fields[name] = chunk[0]
        elif kind == "u16":
            fields[name] = struct.unpack("<H", chunk)[0]
        elif kind == "u32":
            fields[name] = struct.unpack("<I", chunk)[0]
        else:
            fields[name] = bytes(chunk)
        pos += n
    if ftype == STATUS:
        fields["extra"] = bytes(body[size:])
    return fields


def queue_event_body(t_us, state=None):
    return msg_encode(QUEUE_EVENT, {"t_us": t_us, "state": state or button_state(BTN_A)})


def button_state(buttons):
    return struct.pack("<H", buttons) + NEUTRAL[2:]


def describe(ftype, seq, body):
    name = TYPE_NAMES.get(ftype, "0x%02x" % ftype)
    try:
        f = msg_decode(ftype, body) if ftype in FIELDS else {}
    except ProtoError:
        return "%s seq=%d (bad body %s)" % (name, seq, body.hex())
    if ftype == NAK:
        return "NAK %s expected=%d got=%d" % (NAK_NAMES.get(f["reason"], hex(f["reason"])),
                                               f["expected"], f["got"])
    if ftype == EVENT:
        return "EVENT %s" % EVENT_NAMES.get(f["code"], hex(f["code"]))
    if ftype == INFO:
        return ("INFO proto_ver=%d fw_ver=%s bridge_ver=%s hid_profile=%d queue_cap=%d "
                "app_timeout_ms=%d report_period_us=%d" % (
                    f["proto_ver"], ".".join(str(x) for x in f["fw_ver"]),
                    ".".join(str(x) for x in f["bridge_ver"]), f["hid_profile"], f["queue_cap"],
                    f["app_timeout_ms"], f["report_period_us"]))
    if ftype == STATUS:
        s = ("STATUS free=%d now_us=%d last_seq=%d mode=%d abort_count=%d last_event=%d flags=0x%02x"
             % (f["free"], f["now_us"], f["last_seq"], f["mode"], f["abort_count"],
                f["last_event"], f["flags"]))
        d = debug_counters(f["extra"])
        if d:
            s += " | max_app_gap_ms=%d watchdog_trips=%d dropped_bytes=%d tx_dropped=%d" % d
        return s
    return "%s %s (tx seq %d)" % (name, " ".join("%s=%s" % kv for kv in f.items()), seq)


def debug_counters(extra):
    """STATUS debug counters v1 (7 bytes), or None."""
    if len(extra) < 7:
        return None
    gap, trips, dropped, txd = struct.unpack("<HBHH", extra[:7])
    return gap, trips, dropped, txd


# ------------------------------------------------------------------ sender
class GbnSender:
    """The AD-6 sender (docs/link-protocol.md "Reliability", "Flow control").

    HELLO first, resent every 100 ms until INFO. QUEUE_EVENTs then take seqs
    HELLO.seq + 1, + 2, ... Window min(credit, 64 - unacked); credit =
    STATUS.free - QUEUE_EVENTs sent after STATUS.last_seq. Resend from the
    oldest unACKed frame after 50 ms; rewind only on a transport NAK whose
    `expected` is ahead of the last rewind point. A NAK's `expected` also
    acknowledges everything before it (story 2.6 D7).

    `write(bytes)` sends; `now()` is the clock; `impair(bytes)` may drop,
    corrupt or pad a frame (returns the bytes to write); `pacer`, if given,
    limits the bytes written to the UART's rate (see Pacer)."""

    def __init__(self, write, now, t_list, hello_seq=0, impair=None, pacer=None):
        self.write = write
        self.now = now
        self.pacer = pacer
        self.t_list = list(t_list)
        self.hello_seq = hello_seq & 0xFF
        self.impair = impair or (lambda b: b)
        self.info = None
        self.last_hello = None
        self.base = 0
        self.next = 0
        self.high = 0
        self.rewind_at = None
        self.base_timer = 0.0
        self.free = None
        self.accepted_by_status = 0
        self.last_status = None
        self.stats = {"sent": 0, "resends": 0, "rewinds": 0, "naks": 0, "semantic_naks": 0,
                      "nak_reasons": {}}

    def seq_of(self, idx):
        return (self.hello_seq + 1 + idx) & 0xFF

    def _count_after(self, seq):
        """Events up to and including `seq`, if `seq` is in [base - 1, base + 64]."""
        d = (seq - ((self.seq_of(self.base) - 1) & 0xFF)) & 0xFF
        return self.base + d if d <= ACK_WINDOW else None

    def _ack_upto(self, count):
        if count is not None and self.base < count <= self.high:
            self.base = count
            self.base_timer = self.now()
            self.next = max(self.next, self.base)

    def _send(self, ftype, seq, body):
        self.write(self.impair(frame_encode(ftype, seq, body)))
        self.stats["sent"] += 1

    def on_frame(self, ftype, seq, body):
        try:
            f = msg_decode(ftype, body) if ftype in FIELDS else None
        except ProtoError:
            return
        if ftype == INFO:
            if self.info is None:
                self.base_timer = self.now()
            self.info = f
            return
        if self.info is None or f is None:
            return
        if ftype == ACK:
            self._ack_upto(self._count_after(f["seq"]))
        elif ftype == NAK:
            self.stats["naks"] += 1
            name = NAK_NAMES.get(f["reason"], hex(f["reason"]))
            self.stats["nak_reasons"][name] = self.stats["nak_reasons"].get(name, 0) + 1
            if not 0x01 <= f["reason"] <= 0x0F:
                self.stats["semantic_naks"] += 1
                return
            count = self._count_after((f["expected"] - 1) & 0xFF)
            self._ack_upto(count)
            if count is not None and (self.rewind_at is None or count > self.rewind_at):
                self.rewind_at = count
                self.next = count
                self.stats["rewinds"] += 1
                self.base_timer = self.now()
        elif ftype == STATUS:
            count = self._count_after(f["last_seq"])
            if count is not None:
                self.free = f["free"]
                self.accepted_by_status = count
                self._ack_upto(count)
            self.last_status = f

    def done(self):
        return self.info is not None and self.base >= len(self.t_list)

    def step(self, budget=8):
        t = self.now()
        if self.info is None:
            if self.last_hello is None or t - self.last_hello >= HELLO_RESEND_S:
                self._send(HELLO, self.hello_seq, bytes([PROTO_VERSION]))
                self.last_hello = t
            return
        if self.base < self.high and t - self.base_timer >= ACK_TIMEOUT_S:
            self.next = self.base
            self.base_timer = t
            self.rewind_at = None
            self.stats["resends"] += 1
        sent = 0
        while self.next < len(self.t_list) and sent < budget:
            if self.next - self.base >= ACK_WINDOW:
                break
            if self.next >= self.high:
                if self.free is None:
                    break
                if self.free <= self.high - self.accepted_by_status:
                    break
            frame = frame_encode(QUEUE_EVENT, self.seq_of(self.next), queue_event_body(self.t_list[self.next]))
            if self.pacer is not None and not self.pacer.take(len(frame)):
                break
            if self.next == self.base:
                self.base_timer = t
            self.write(self.impair(frame))
            self.stats["sent"] += 1
            self.next += 1
            self.high = max(self.high, self.next)
            sent += 1


class Pacer:
    """Token bucket at the link's byte rate (story 2.6 D30).

    A USB-CDC to UART bridge without flow control (2wiCC_Comms: a 1 KB ring
    that silently drops on overflow) loses bytes when the host writes faster
    than the UART drains (460800 baud = 46 080 bytes/s). The sender may write
    at most `burst` bytes ahead of the line, refilled at 95 % of the line
    rate."""

    def __init__(self, now, baud=DEFAULT_BAUD, burst=256, share=0.95):
        self.now = now
        self.rate = baud / 10.0 * share
        self.burst = float(burst)
        self.tokens = float(burst)
        self.last = now()

    def take(self, n):
        t = self.now()
        self.tokens = min(self.burst, self.tokens + (t - self.last) * self.rate)
        self.last = t
        if self.tokens < n:
            return False
        self.tokens -= n
        return True


# -------------------------------------------------------------- serial I/O
class Port:
    """A serial port (pyserial imported only here) with a frame splitter.
    `now()` and `sleep()` are the only clock the commands use, so a test can
    replace the port with a simulated one on a fake clock."""

    def __init__(self, name, baud):
        try:
            import serial  # pylint: disable=import-outside-toplevel
        except ImportError:
            sys.exit("link_probe: pyserial is needed to open a port (pip install pyserial)")
        self.ser = serial.Serial(name, baud, timeout=0)
        self.rx = Splitter("APP")
        self.ser.reset_input_buffer()

    @staticmethod
    def now():
        return time.monotonic()

    @staticmethod
    def sleep(seconds):
        time.sleep(seconds)

    def write(self, data):
        self.ser.write(data)

    def poll(self):
        data = self.ser.read(4096)
        return self.rx.feed(data) if data else []

    def read_raw(self, n):
        return self.ser.read(n)


def _log(port, frame, t0):
    ftype, seq, body = frame
    print("%9.3f  %s" % (port.now() - t0, describe(ftype, seq, body)))


def do_hello(port, timeout=3.0, quiet=False, seq=0):
    """HELLO every 100 ms until INFO (AD-7). Returns the INFO fields or None."""
    t0 = port.now()
    last = None
    while port.now() - t0 < timeout:
        if last is None or port.now() - last >= HELLO_RESEND_S:
            port.write(frame_encode(HELLO, seq, bytes([PROTO_VERSION])))
            last = port.now()
        for fr in port.poll():
            if fr[0] == INFO:
                if not quiet:
                    _log(port, fr, t0)
                return msg_decode(INFO, fr[2])
        port.sleep(0.001)
    return None


def _percentile(xs, p):
    xs = sorted(xs)
    return xs[min(len(xs) - 1, int(round(p / 100.0 * (len(xs) - 1))))]


def cmd_watch(port, args):
    t0 = port.now()
    status_t = []
    last = None
    while port.now() - t0 < args.seconds:
        for fr in port.poll():
            if fr[0] == STATUS:
                status_t.append(port.now())
                last = fr
                if args.verbose:
                    _log(port, fr, t0)
            else:
                _log(port, fr, t0)
        port.sleep(0.0005)
    if not args.verbose and last is not None:
        _log(port, last, t0)
    gaps = [(b - a) * 1000.0 for a, b in zip(status_t, status_t[1:])]
    if gaps:
        print("STATUS: %d frames, interval median %.1f ms, p95 %.1f ms, min %.1f ms, max %.1f ms"
              % (len(status_t), _percentile(gaps, 50), _percentile(gaps, 95), min(gaps), max(gaps)))
    else:
        print("STATUS: %d frames" % len(status_t))
    return 0


def _run_sender(port, sender, timeout, log_naks=True):
    """Runs one sender until done or until `timeout` seconds from now (the
    deadline is per run, never the whole command's, story 2.6 D30)."""
    start = port.now()
    while not sender.done() and port.now() - start < timeout:
        for fr in port.poll():
            if fr[0] in (NAK, EVENT) and log_naks:
                _log(port, fr, start)
            sender.on_frame(*fr)
        sender.step()
        port.sleep(0.0002)
    return sender.done()


def _wait_status(port, sender, seconds=0.2):
    end = port.now() + seconds
    while port.now() < end:
        for fr in port.poll():
            sender.on_frame(*fr)
        port.sleep(0.001)
    return sender.last_status


def _stop(port, seq, timeout=1.0):
    port.write(frame_encode(STOP, seq))
    end = port.now() + timeout
    while port.now() < end:
        for fr in port.poll():
            if fr[0] == ACK and fr[2] and fr[2][0] == seq:
                return True
        port.sleep(0.001)
    return False


def make_pacer(port, args):
    return None if getattr(args, "no_pace", False) else Pacer(port.now, args.baud)


def cmd_fill(port, args):
    t0 = port.now()
    if args.no_hello:
        # H.4 step 7: no HELLO after a power cycle: expect NAK VERSION.
        for i in range(args.count):
            port.write(frame_encode(QUEUE_EVENT, i, queue_event_body((i + 1) * 1000)))
        end = port.now() + 0.5
        while port.now() < end:
            for fr in port.poll():
                if fr[0] != STATUS:
                    _log(port, fr, t0)
            port.sleep(0.001)
        return 0
    info = do_hello(port)
    if info is None:
        print("no INFO")
        return 1
    sender = GbnSender(port.write, port.now, [(i + 1) * 1000 for i in range(args.count)],
                       pacer=make_pacer(port, args))
    sender.on_frame(INFO, 0, msg_encode(INFO, info))
    ok = _run_sender(port, sender, 10.0)
    st = _wait_status(port, sender)
    print("fill: %s, %d events, stats %s" % ("done" if ok else "TIMEOUT", args.count, sender.stats))
    if st:
        print("STATUS after fill: free=%d last_seq=%d (expected free=%d last_seq=%d)" % (
            st["free"], st["last_seq"], info["queue_cap"] - args.count, sender.seq_of(args.count - 1)))
    if args.stop:
        print("STOP ACKed" if _stop(port, sender.seq_of(args.count)) else "STOP not ACKed")
        st = _wait_status(port, sender, 0.1)
        if st:
            print("STATUS after STOP: free=%d" % st["free"])
    return 0 if ok else 1


def make_impair(rng, corrupt, drop, garbage):
    def impair(frame):
        if drop and rng.random() < drop:
            return b""
        b = bytearray(frame)
        if corrupt and rng.random() < corrupt:
            p = rng.randrange(len(b) - 1)
            b[p] ^= 1 << rng.randrange(8)
        if garbage and rng.random() < 0.05:
            b = bytearray(rng.randrange(1, 256) for _ in range(rng.randrange(1, 8))) + b"\x00" + b
        return bytes(b)
    return impair


def soak(port, cycles, events, seed=1, corrupt=0.0, drop=0.0, garbage=False, pace=True,
         baud=DEFAULT_BAUD, cycle_timeout=30.0, out=print):
    """H.5: `cycles` x (HELLO, `events` QUEUE_EVENTs through the AD-6 sender,
    check STATUS free / last_seq, STOP). Returns (passed, totals)."""
    rng = random.Random(seed)
    impair = make_impair(rng, corrupt, drop, garbage)
    pacer = Pacer(port.now, baud) if pace else None
    t0 = port.now()
    totals = {"events": 0, "resends": 0, "rewinds": 0, "naks": 0, "semantic_naks": 0}
    first_dbg = last_dbg = None
    failures = 0
    hello_seq = 0
    for cycle in range(cycles):
        info = do_hello(port, quiet=True, seq=hello_seq)
        if info is None:
            out("cycle %d: no INFO" % cycle)
            failures += 1
            continue
        t_list = [(i + 1) * 1000 for i in range(events)]
        sender = GbnSender(port.write, port.now, t_list, hello_seq=hello_seq, impair=impair, pacer=pacer)
        sender.on_frame(INFO, 0, msg_encode(INFO, info))
        ok = _run_sender(port, sender, cycle_timeout, log_naks=False)
        st = _wait_status(port, sender, 0.15)
        want_free = info["queue_cap"] - events
        want_last = sender.seq_of(events - 1)
        good = ok and st is not None and st["free"] == want_free and st["last_seq"] == want_last \
            and sender.stats["semantic_naks"] == 0
        if st is not None:
            d = debug_counters(st["extra"])
            if d and last_dbg and d[2] != last_dbg[2]:
                # dropped_bytes moved during this cycle: worth a line at the bench.
                out("cycle %d: dropped_bytes %d -> %d" % (cycle, last_dbg[2], d[2]))
            first_dbg = first_dbg or d
            last_dbg = d or last_dbg
        if not good:
            failures += 1
            out("cycle %d: FAIL done=%s status=%s want free=%d last_seq=%d stats=%s" % (
                cycle, ok, st, want_free, want_last, sender.stats))
        totals["events"] += events if good else 0
        for k in ("resends", "rewinds", "naks", "semantic_naks"):
            totals[k] += sender.stats[k]
        stop_seq = sender.seq_of(events)
        if not _stop(port, stop_seq):
            _stop(port, stop_seq)
        hello_seq = (stop_seq + 1) & 0xFF
    elapsed = port.now() - t0
    out("soak: %d cycles x %d events, %.1f s, pacing %s, totals %s" % (
        cycles, events, elapsed, "on" if pace else "off", totals))
    if first_dbg or last_dbg:
        out("debug counters first %s last %s (max_app_gap_ms, watchdog_trips, dropped_bytes, tx_dropped)"
            % (first_dbg, last_dbg))
    out("%s: %d events enqueued exactly once and in order" % ("PASS" if failures == 0 else "FAIL",
                                                              totals["events"]))
    return failures == 0, totals


def cmd_soak(port, args):
    passed, _ = soak(port, args.cycles, args.events, seed=args.seed, corrupt=args.corrupt, drop=args.drop,
                     garbage=args.garbage, pace=not args.no_pace, baud=args.baud)
    return 0 if passed else 1


def cmd_noise(port, args):
    rng = random.Random(args.seed)
    port.write(bytes(rng.randrange(1, 256) for _ in range(args.count)))
    info = do_hello(port)
    print("noise: %d bytes, then %s" % (args.count, "INFO received" if info else "NO INFO"))
    return 0 if info else 1


def cmd_try(port, args):
    t0 = port.now()
    if do_hello(port) is None:
        print("no INFO")
        return 1
    port.write(frame_encode(SET_STATE, 1, NEUTRAL))
    port.write(frame_encode(QUEUE_EVENT, 2, queue_event_body(1000)))
    port.write(frame_encode(START, 3, struct.pack("<I", 100000)))
    end = port.now() + 0.3
    while port.now() < end:
        for fr in port.poll():
            if fr[0] != STATUS:
                _log(port, fr, t0)
        port.sleep(0.001)
    _stop(port, 4)
    return 0


def cmd_loopback(port, args):
    """Rig check R0: the port's own TX wired to its RX; every byte value comes back."""
    pattern = bytes(range(256))
    bad = 0
    for _ in range(args.repeat):
        port.write(pattern)
        got = bytearray()
        end = port.now() + 1.0
        while len(got) < 256 and port.now() < end:
            got += port.read_raw(256 - len(got))
        if bytes(got) != pattern:
            bad += 1
    print("loopback: %d of %d rounds of 256 byte values unchanged: %s" % (
        args.repeat - bad, args.repeat, "PASS" if bad == 0 else "FAIL"))
    return 0 if bad == 0 else 1


def cmd_hello(port, args):
    info = do_hello(port, timeout=args.seconds)
    if info is None:
        print("no INFO")
        return 1
    return 0


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--port", required=True)
    ap.add_argument("--baud", type=int, default=DEFAULT_BAUD)
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--no-pace", action="store_true",
                    help="write QUEUE_EVENTs as fast as the window allows (default: paced to the UART rate)")
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("hello", help="HELLO until INFO, print INFO")
    p.add_argument("--seconds", type=float, default=3.0)
    p = sub.add_parser("watch", help="print frames and STATUS interval statistics")
    p.add_argument("--seconds", type=float, default=5.0)
    p.add_argument("--verbose", action="store_true", help="print every STATUS")
    p = sub.add_parser("fill", help="prefill N QUEUE_EVENTs, check free and last_seq")
    p.add_argument("--count", type=int, default=10)
    p.add_argument("--stop", action="store_true")
    p.add_argument("--no-hello", action="store_true", help="send without HELLO (expect NAK VERSION)")
    p = sub.add_parser("soak", help="repeated fill / STOP cycles with optional impairments")
    p.add_argument("--cycles", type=int, default=40)
    p.add_argument("--events", type=int, default=500)
    p.add_argument("--corrupt", type=float, default=0.0)
    p.add_argument("--drop", type=float, default=0.0)
    p.add_argument("--garbage", action="store_true")
    p = sub.add_parser("noise", help="N random non-zero bytes, then HELLO until INFO")
    p.add_argument("--count", type=int, default=2000)
    p = sub.add_parser("try", help="one SET_STATE and one START, print the replies")
    p = sub.add_parser("loopback", help="rig check R0: TX wired to RX of the same port")
    p.add_argument("--repeat", type=int, default=100)
    args = ap.parse_args(argv)
    port = Port(args.port, args.baud)
    handler = {"hello": cmd_hello, "watch": cmd_watch, "fill": cmd_fill, "soak": cmd_soak,
               "noise": cmd_noise, "try": cmd_try, "loopback": cmd_loopback}[args.cmd]
    return handler(port, args)


if __name__ == "__main__":
    sys.exit(main())
