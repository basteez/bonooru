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
`loopback`) and the emulated bridge HEARTBEAT (story 2.7).

Subcommands: hello, watch, fill, soak, noise, try, loopback (story 2.6), the
one-command bench steps of story 2.7: linkup (H.8), silence (H.9), direct
and playing (H.11), breaks (H.12), and of story 2.3: p2check (H.15),
p2watch (H.16) and press (D35: a D-pad direction or a button in DIRECT), and
of story 2.5: selfplay (H.18: the built-in melody of the fw-debug-timeline
build on the console, with the firmware's self-timing from the debug STATUS
v4) and hangcheck (H.2a/H.2b: after a liveness reboot, the boot cause the
firmware reports). Run with -h.

console_ready (story 2.3 D15): it now comes only from a real USB host that
completed the P2 handshake, the Switch 2 or, on the bench Mac,
`python3 firmware/tools/p2_probe.py handshake --hold 120 &` started in the
same shell line. direct, playing and `breaks --mode playing` need it; the
bench-only fake (fw-debug-bench) is gone.

Bridge heartbeat emulation (story 2.7 D15): until the bridge firmware exists
(Story 3.1) the probe itself sends the bridge's HEARTBEAT (type 0x40, body
bridge_fw_ver 0.0.1 and flags bit 0 = DTR) every --hb-period-ms (default 20)
through the transparent bench peer. The schedule is absolute; a late tick
sends one heartbeat and re-bases, never a burst. It runs inside the port's
poll() and sleep(), so every wait loop keeps it alive without threads; the
probe prints its own send statistics (max gap, gaps above 27 ms) so a host
stall can be told apart from a firmware trip. Options: --heartbeat (the
2.6 subcommands, default off), --no-heartbeat (the new ones, default on).
When a command ends its heartbeat stops: the controller then sees a link
loss 80 ms later (EVENT LINK_LOST, latch set), which the next command's
HELLO clears.

Every story 2.7 subcommand runs one whole bench step in one process with
one open session, prints one line per check and ends with `PASS` or
`FAIL <reason>` (exit 0 or 1). Where the owner must act it prints one prominent line and
waits for the evidence (an EVENT, a counter, STATUS stopping or resuming).
Their requests are resent with the same seq every 50 ms until answered (a
stop-and-wait AD-6 sender); `breaks` writes one 0x00 after each replug and
waits for the line to settle (1 s with no new break, framing error, link
loss or STATUS gap) before it checks the round and prompts the next unplug.
--trace FILE (those five subcommands only; the 2.6 ones ignore it) writes
every frame sent and received with a timestamp; `breaks` prints the tail of
the trace when it fails.
"""

import argparse
import os
import random
import re
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
# Watchdog timing (docs/link-protocol.md "Timing constants"; compared with
# common/proto/vectors/constants.txt by the self-test, story 2.7 D18).
HEARTBEAT_PERIOD_MS = 20
HEARTBEAT_MAX_GAP_MS = 27
LINK_TIMEOUT_MS = 80
APP_TIMEOUT_MS_DEFAULT = 1000
APP_PING_DIVISOR = 4
HB_FLAG_DTR = 0x01
PROBE_BRIDGE_VER = bytes([0, 0, 1])  # what the emulated heartbeat reports
STATUS_LINK_OK = 0x01
STATUS_CONSOLE_READY = 0x02
MODE_IDLE, MODE_DIRECT, MODE_PLAYING = 0, 1, 2
EVT_PLAYBACK_DONE, EVT_LINK_LOST = 0x01, 0x02
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
            s += " | " + dbg_str(d)
            if d.get("p2_log"):
                s += " | p2_log: " + ", ".join(p2_log_str(k, i) for k, i in d["p2_log"])
            if d.get("v4"):
                s += " | v4: " + v4_str(d["v4"])
        return s
    return "%s %s (tx seq %d)" % (name, " ".join("%s=%s" % kv for kv in f.items()), seq)


DEBUG_V1 = ("max_app_gap_ms", "watchdog_trips", "dropped_bytes", "tx_dropped")
DEBUG_V2 = ("rx_err_bytes", "rx_overflow_bytes", "overlong_bytes", "rx_fe", "rx_pe", "rx_be", "rx_oe")
# dropped_bytes = rx_err_bytes + rx_overflow_bytes + overlong_bytes (story 2.6 D35);
# rx_fe .. rx_oe count the error bits (one character may set several).
DROP_SPLIT = ("rx_err_bytes", "rx_overflow_bytes", "overlong_bytes", "rx_fe", "rx_pe", "rx_be", "rx_oe")


# v3 (story 2.3 D14): the P2 debug log window after the 21 bytes:
# p2_log_total u16, p2_log_n u8, then p2_log_n records (kind, id), oldest first.
P2_LOG_KINDS = {0x01: "usb", 0x02: "sub", 0x03: "report", 0x04: "REFUSED", 0x05: "overflow",
                0x06: "HANDSHAKE", 0x07: "restart", 0x08: "arg", 0x09: "repeat"}
P2_LOG_REPEAT = 0x09
P2_RESTART_CAUSES = {0x01: "mount", 0x02: "unmount", 0x03: "suspend", 0x04: "bus-reset"}


def p2_log_str(kind, ident):
    """One P2 debug log record as text: 'usb 80 04', 'sub 10', 'restart mount'."""
    if kind == 0x01:
        return "usb 80 %02x" % ident
    if kind == 0x07:
        return "restart %s" % P2_RESTART_CAUSES.get(ident, "%02x" % ident)
    if kind == 0x06:
        return "HANDSHAKE"
    if kind == 0x05:
        return "overflow (%d lost)" % ident
    if kind == P2_LOG_REPEAT:
        return "(repeated %d%s more)" % (ident, "+" if ident == 255 else "")
    return "%s %02x" % (P2_LOG_KINDS.get(kind, "kind%02x" % kind), ident)


# v4 (story 2.5 D13): one fixed 39-byte block after the v3 window, at offset
# 24 + 2 * p2_log_n of the debug tail, little-endian, every u16 saturating.
DEBUG_V4_LEN = 39
DEBUG_V4_FORMAT = "<HHI6HHHHHHHHBHBB"
DEBUG_V4 = ("late_n", "late_max_us", "late_sum_us", "late_hist", "loop_max_us", "core0_pass_max_us", "in_min_us",
            "in_p95_us", "in_max_us", "in_slow", "seq_retries", "seq_fallbacks", "stack_used_max", "boot_cause",
            "reboot_count")
LATE_BUCKETS_US = ("<50", "50-99", "100-249", "250-999", "1000-3999", ">=4000")
BOOT_CAUSES = {0: "NONE", 1: "CORE0_STALE", 2: "CORE1_STALE", 3: "BOOT_NOT_ALIVE"}


def debug_v4(block):
    """The 39-byte v4 block as a dict (late_hist is a list of 6)."""
    v = struct.unpack(DEBUG_V4_FORMAT, bytes(block[:DEBUG_V4_LEN]))
    d = dict(zip(DEBUG_V4[:3], v[:3]))
    d["late_hist"] = list(v[3:9])
    d.update(zip(DEBUG_V4[4:], v[9:]))
    return d


def debug_counters(extra):
    """STATUS debug counters (docs/link-protocol.md) as a dict: the v1 prefix
    (7 bytes), when present the v2 split of dropped_bytes (14 more bytes),
    the v3 P2 debug log window (p2_log_total and p2_log, a list of
    (kind, id)) and the v4 self-timing block ("v4", a dict, story 2.5);
    None without debug counters."""
    if len(extra) < 7:
        return None
    d = dict(zip(DEBUG_V1, struct.unpack("<HBHH", extra[:7])))
    if len(extra) >= 21:
        d.update(zip(DEBUG_V2, struct.unpack("<7H", extra[7:21])))
    if len(extra) >= 24:
        total, n = struct.unpack("<HB", extra[21:24])
        recs = extra[24:24 + 2 * n]
        d["p2_log_total"] = total
        d["p2_log"] = [(recs[i], recs[i + 1]) for i in range(0, len(recs) - 1, 2)]
        off = 24 + 2 * n
        if len(extra) >= off + DEBUG_V4_LEN:
            d["v4"] = debug_v4(extra[off:off + DEBUG_V4_LEN])
    return d


def v4_str(v):
    """The v4 block as one line."""
    mean = v["late_sum_us"] / float(v["late_n"]) if v["late_n"] else 0.0
    return ("late n=%d max=%d us mean=%.1f us hist=%s | loop_max=%d us core0_pass_max=%d us | IN min=%d p95=%d "
            "max=%d slow=%d | seq retries=%d fallbacks=%d | stack=%d B | boot=%s reboots=%d" % (
                v["late_n"], v["late_max_us"], mean, "/".join(str(x) for x in v["late_hist"]), v["loop_max_us"],
                v["core0_pass_max_us"], v["in_min_us"], v["in_p95_us"], v["in_max_us"], v["in_slow"],
                v["seq_retries"], v["seq_fallbacks"], v["stack_used_max"],
                BOOT_CAUSES.get(v["boot_cause"], "%d" % v["boot_cause"]), v["reboot_count"]))


def dbg_str(d):
    """The numeric debug counters as k=v (the P2 log and v4 are printed apart)."""
    return " ".join("%s=%d" % kv for kv in (d or {}).items() if isinstance(kv[1], int))


def drop_split(d):
    return " ".join("%s=%d" % (k, d[k]) for k in DROP_SPLIT if k in d)


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
    limits the bytes written to the UART's rate (see Pacer); `max_inflight`,
    if given, caps the bytes written for QUEUE_EVENTs the controller has not
    yet confirmed (ACK-clocked, independent of the bridge's drain rate,
    story 2.6 D33)."""

    def __init__(self, write, now, t_list, hello_seq=0, impair=None, pacer=None, max_inflight=None,
                 baud=DEFAULT_BAUD):
        self.write = write
        self.now = now
        self.pacer = pacer
        self.max_inflight = max_inflight
        self.writes = []  # [event index, bytes written], oldest first, not yet confirmed
        self.inflight = 0
        self.min_drain = baud / 10.0 * MIN_DRAIN_SHARE  # bytes/s, the slowest bridge assumed
        self.hold_until = 0.0
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
        self._retire(self.base)

    def _go_back(self):
        """A rewind or resend abandons the copies still on the wire. Nothing
        confirms them any more, so they are retired by time: no new
        QUEUE_EVENT is written until they have drained even from the slowest
        bridge assumed (MIN_DRAIN_SHARE of the line rate): at most
        max_inflight / (0.8 x 46 080) s, about 26 ms with the default cap.
        The hold also applies after a 50 ms resend timeout, because the
        frames written after the oldest unconfirmed one may be recent
        (story 2.6 D33, D36)."""
        if self.max_inflight is not None and self.inflight > 0:
            self.hold_until = max(self.hold_until, self.now() + self.inflight / self.min_drain)
        self.writes = []
        self.inflight = 0

    def _retire(self, count):
        """Writes of events below `count` have left the wire, in FIFO order:
        the controller has handled a copy of each of them."""
        while self.writes and self.writes[0][0] < count:
            self.inflight -= self.writes.pop(0)[1]

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
                self._go_back()
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
            self._go_back()
            self.next = self.base
            self.base_timer = t
            self.rewind_at = None
            self.stats["resends"] += 1
        if t < self.hold_until:
            return
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
            if self.max_inflight is not None and self.inflight + len(frame) > self.max_inflight:
                break
            if self.pacer is not None and not self.pacer.take(len(frame)):
                break
            if self.next == self.base:
                self.base_timer = t
            data = self.impair(frame)
            self.write(data)
            self.writes.append([self.next, len(data)])
            self.inflight += len(data)
            self.stats["sent"] += 1
            self.next += 1
            self.high = max(self.high, self.next)
            sent += 1


PACE_SHARE = 0.95
MAX_INFLIGHT = 960  # bytes: 50 QUEUE_EVENT frames, under the 1023-byte ring of 2wiCC_Comms
MIN_DRAIN_SHARE = 0.80  # slowest bridge drain assumed after a rewind (2wiCC_Comms: about 0.85)
GARBAGE_MAX = 7  # soak --garbage: at most this many junk bytes, then a 0x00, before a frame


class Pacer:
    """Token bucket at the link's byte rate (story 2.6 D30).

    A USB-CDC to UART bridge without flow control (2wiCC_Comms: a 1 KB ring
    that silently drops on overflow) loses bytes when the host writes faster
    than the UART drains (460800 baud = 46 080 bytes/s). The sender may write
    at most `burst` bytes ahead of the line, refilled at 95 % of the line
    rate."""

    def __init__(self, now, baud=DEFAULT_BAUD, burst=256, share=PACE_SHARE):
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


# --------------------------------------------------- heartbeat emulation
class HeartbeatService:
    """The bridge's HEARTBEAT, emulated by the probe (story 2.7 D15).

    Mixed into a port that has now() and write(). service() is called from
    the port's poll() and sleep(). Absolute schedule: next due = previous due
    + period; a late tick sends one heartbeat and re-bases on now (no burst,
    like the real bridge's alarm). Heartbeat writes bypass the pacer and the
    in-flight cap (11 bytes every 20 ms, about 0.5 KB/s)."""

    def _hb_init(self):
        self.hb_period = None
        self.hb_dtr = True
        self.hb_due = 0.0
        self.hb_seq = 0
        self.hb_sent = 0
        self.hb_last = None
        self.hb_max_gap_ms = 0.0
        self.hb_late = 0  # gaps above HEARTBEAT_MAX_GAP_MS

    def hb_start(self, period_ms=HEARTBEAT_PERIOD_MS, dtr=None):
        """Starts (or restarts) the heartbeat; the first one is due at once.
        The gap statistics ignore the pause before a restart."""
        if not hasattr(self, "hb_period"):
            self._hb_init()
        if dtr is not None:
            self.hb_dtr = dtr
        self.hb_period = period_ms / 1000.0
        self.hb_due = self.now()
        self.hb_last = None
        self.service()

    def hb_stop(self):
        """Stops the heartbeat; returns the time of the last heartbeat written."""
        if not hasattr(self, "hb_period"):
            self._hb_init()
        self.hb_period = None
        return self.hb_last

    def hb_set_dtr(self, on):
        if not hasattr(self, "hb_period"):
            self._hb_init()
        self.hb_dtr = bool(on)

    def hb_running(self):
        return getattr(self, "hb_period", None) is not None

    def hb_frame(self):
        flags = HB_FLAG_DTR if self.hb_dtr else 0
        return frame_encode(HEARTBEAT, self.hb_seq,
                            msg_encode(HEARTBEAT, {"bridge_fw_ver": PROBE_BRIDGE_VER, "flags": flags}))

    def hb_emit(self, frame):
        """Writes one heartbeat frame (a test port may override it)."""
        self.write(frame)

    def service(self):
        if getattr(self, "hb_period", None) is None:
            return
        t = self.now()
        if t < self.hb_due:
            return
        self.hb_emit(self.hb_frame())
        self.hb_seq = (self.hb_seq + 1) & 0xFF
        self.hb_sent += 1
        if self.hb_last is not None:
            gap = (t - self.hb_last) * 1000.0
            self.hb_max_gap_ms = max(self.hb_max_gap_ms, gap)
            if gap > HEARTBEAT_MAX_GAP_MS:
                self.hb_late += 1
        self.hb_last = t
        nxt = self.hb_due + self.hb_period
        self.hb_due = nxt if nxt > t else t + self.hb_period

    def hb_stats(self):
        if not hasattr(self, "hb_period"):
            self._hb_init()
        return "heartbeat: sent %d, max send gap %.1f ms, gaps above %d ms: %d" % (
            self.hb_sent, self.hb_max_gap_ms, HEARTBEAT_MAX_GAP_MS, self.hb_late)


# -------------------------------------------------------------- serial I/O
class Port(HeartbeatService):
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
        self._hb_init()

    @staticmethod
    def now():
        return time.monotonic()

    def sleep(self, seconds):
        """Sleeps in slices of at most 1 ms, servicing the heartbeat."""
        end = time.monotonic() + seconds
        while True:
            self.service()
            rem = end - time.monotonic()
            if rem <= 0:
                break
            time.sleep(min(rem, 0.001))

    def write(self, data):
        self.ser.write(data)

    def poll(self):
        self.service()
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
    if getattr(args, "no_pace", False):
        return None
    return Pacer(port.now, args.baud, share=getattr(args, "pace_share", PACE_SHARE))


def max_inflight_of(args):
    v = getattr(args, "max_inflight", MAX_INFLIGHT)
    return v if v and v > 0 else None


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
                       pacer=make_pacer(port, args), max_inflight=max_inflight_of(args), baud=args.baud)
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
            n = rng.randrange(1, GARBAGE_MAX + 1)
            b = bytearray(rng.randrange(1, 256) for _ in range(n)) + b"\x00" + b
        return bytes(b)
    return impair


def _status_now(port, timeout=1.0):
    """The next STATUS (decoded) within `timeout`, or None."""
    end = port.now() + timeout
    while port.now() < end:
        for fr in port.poll():
            if fr[0] == STATUS:
                try:
                    return msg_decode(STATUS, fr[2])
                except ProtoError:
                    pass
        port.sleep(0.001)
    return None


def _trips(st):
    d = debug_counters(st["extra"]) if st else None
    return d["watchdog_trips"] if d else None


def soak(port, cycles, events, seed=1, corrupt=0.0, drop=0.0, garbage=False, pace=True,
         baud=DEFAULT_BAUD, cycle_timeout=30.0, out=print, pace_share=PACE_SHARE, max_inflight=MAX_INFLIGHT,
         heartbeat=False, hb_period_ms=HEARTBEAT_PERIOD_MS):
    """H.5: `cycles` x (HELLO, `events` QUEUE_EVENTs through the AD-6 sender,
    check STATUS free / last_seq, STOP). Returns (passed, totals).
    heartbeat (story 2.7, H.10): the emulated bridge heartbeat runs for the
    whole soak, and the run fails unless the last STATUS shows link_ok 1 and
    the same abort_count and watchdog_trips as before the first cycle."""
    hb_base = None
    hb_fail = None
    if heartbeat:
        port.hb_start(hb_period_ms)
        end = port.now() + 1.0
        while port.now() < end:
            hb_base = _status_now(port, 0.2)
            if hb_base is not None and hb_base["flags"] & STATUS_LINK_OK:
                break
        if hb_base is None or not hb_base["flags"] & STATUS_LINK_OK:
            hb_fail = "link_ok never became 1 with the heartbeat running"
    rng = random.Random(seed)
    impair = make_impair(rng, corrupt, drop, garbage)
    pacer = Pacer(port.now, baud, share=pace_share) if pace else None
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
        sender = GbnSender(port.write, port.now, t_list, hello_seq=hello_seq, impair=impair, pacer=pacer,
                           max_inflight=max_inflight if max_inflight and max_inflight > 0 else None, baud=baud)
        sender.on_frame(INFO, 0, msg_encode(INFO, info))
        ok = _run_sender(port, sender, cycle_timeout, log_naks=False)
        st = _wait_status(port, sender, 0.15)
        want_free = info["queue_cap"] - events
        want_last = sender.seq_of(events - 1)
        good = ok and st is not None and st["free"] == want_free and st["last_seq"] == want_last \
            and sender.stats["semantic_naks"] == 0
        if st is not None:
            d = debug_counters(st["extra"])
            if d and last_dbg and d["dropped_bytes"] != last_dbg["dropped_bytes"]:
                # dropped_bytes moved during this cycle: worth a line at the bench, with its causes.
                out("cycle %d: dropped_bytes %d -> %d (%s)" % (cycle, last_dbg["dropped_bytes"],
                                                              d["dropped_bytes"], drop_split(d)))
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
    out("soak: %d cycles x %d events, %.1f s, pacing %s, max in flight %s, totals %s" % (
        cycles, events, elapsed, ("%.2f" % pace_share) if pace else "off",
        ("%d B" % max_inflight) if max_inflight and max_inflight > 0 else "off", totals))
    if first_dbg or last_dbg:
        out("debug counters first: %s" % dbg_str(first_dbg))
        out("debug counters last:  %s" % dbg_str(last_dbg))
    if heartbeat:
        out(port.hb_stats())
        last = _status_now(port, 0.5)
        if hb_fail is None:
            if last is None:
                hb_fail = "no STATUS at the end"
            elif not last["flags"] & STATUS_LINK_OK:
                hb_fail = "link_ok 0 at the end"
            elif last["abort_count"] != hb_base["abort_count"]:
                hb_fail = "abort_count %d -> %d (a watchdog trip?)" % (hb_base["abort_count"], last["abort_count"])
            elif _trips(last) != _trips(hb_base):
                hb_fail = "watchdog_trips %s -> %s" % (_trips(hb_base), _trips(last))
        if last is not None:
            out("watchdog: link_ok %d, abort_count %d -> %d, watchdog_trips %s -> %s" % (
                last["flags"] & STATUS_LINK_OK, hb_base["abort_count"] if hb_base else -1, last["abort_count"],
                _trips(hb_base), _trips(last)))
        if hb_fail is not None:
            failures += 1
            out("FAIL heartbeat: %s" % hb_fail)
    out("%s: %d events enqueued exactly once and in order" % ("PASS" if failures == 0 else "FAIL",
                                                              totals["events"]))
    return failures == 0, totals


def cmd_soak(port, args):
    passed, _ = soak(port, args.cycles, args.events, seed=args.seed, corrupt=args.corrupt, drop=args.drop,
                     garbage=args.garbage, pace=not args.no_pace, baud=args.baud,
                     pace_share=getattr(args, "pace_share", PACE_SHARE),
                     max_inflight=getattr(args, "max_inflight", MAX_INFLIGHT),
                     heartbeat=getattr(args, "hb_enabled", False),
                     hb_period_ms=getattr(args, "hb_period_ms", HEARTBEAT_PERIOD_MS))
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


# ------------------------------------------------- story 2.7 bench steps
BREAKS_UNPLUG_TIMEOUT_S = 60.0  # breaks: wait for the unplug evidence
BREAKS_REPLUG_TIMEOUT_S = 30.0  # breaks: wait for STATUS to resume after REPLUG
BREAKS_SETTLE_S = 1.0           # breaks: a quiet line this long after a replug = settled (D42)
BREAKS_SETTLE_TIMEOUT_S = 15.0  # breaks: give up if the line never settles
STATUS_GAP_S = 0.2              # breaks, settling: no STATUS for this long = TX released again
LOSS_WINDOW_MS = (80.0, 120.0)  # H.9: EVENT LINK_LOST after the last heartbeat written
STATUS_SILENT_S = 0.3           # breaks, unarmed: STATUS silent this long = TX released
TRACE_TAIL = 120                # breaks: trace lines printed when it fails


class P2Log:
    """The P2 debug log seen through the debug STATUS v3 window (story 2.3
    D14): every record once, in order, with the time its STATUS arrived. A
    jump of p2_log_total larger than the window is counted as a gap. A
    repeat record is counted up in place by the firmware (total unchanged),
    so the tracker refreshes its last record when both are repeats."""

    def __init__(self):
        self.total = None
        self.records = []  # [(time, kind, id)]
        self.gaps = 0

    def feed(self, d, t):
        if not d or "p2_log_total" not in d:
            return
        total, recs = d["p2_log_total"], d["p2_log"]
        if self.total is not None and self.records:
            k = (total - self.total) & 0xFFFF
            i = len(recs) - k - 1
            last = self.records[-1]
            if 0 <= i < len(recs) and last[1] == P2_LOG_REPEAT and recs[i][0] == P2_LOG_REPEAT:
                self.records[-1] = (last[0], P2_LOG_REPEAT, recs[i][1])
        if self.total is None:
            new = recs
        else:
            k = (total - self.total) & 0xFFFF
            if k > len(recs):
                self.gaps += k - len(recs)
            new = recs[len(recs) - min(k, len(recs)):] if k else []
        self.total = total
        self.records += [(t, kd, i) for kd, i in new]

    def since(self, t):
        return [(kd, i) for rt, kd, i in self.records if rt >= t]


class Link:
    """One open session for a whole bench step (story 2.7 D16): app seq
    numbering, every frame received recorded (STATUS, EVENT, ACK, NAK), and
    small request / wait helpers. Frames are written unpaced: the steps send
    a few frames at a time, each waiting for its reply."""

    def __init__(self, port, out=print, trace=None):
        self.port = port
        self.out = out
        self.trace_path = trace  # --trace FILE: every frame sent and received
        self.trace = []          # ["  t.tttttt TX|RX ..."], kept in memory, written by close()
        self.seq = 0
        self.info = None
        self.status = None      # last STATUS fields (with "dbg")
        self.status_t = None    # its arrival time
        self.status_count = 0
        self.events = []        # [(time, code)]
        self.replies = []       # [(kind, fields)]: ("ACK", seq) or ("NAK", fields)
        self.nak_version = 0
        self.nak_seq = 0
        self.last_app_write = None
        self.t0 = port.now()
        self.p2log = P2Log()  # story 2.3: the P2 debug log, from the debug STATUS

    def log(self, text):
        self.out("%8.3f  %s" % (self.port.now() - self.t0, text))

    def _trace(self, direction, text):
        self.trace.append("%11.6f %s %s" % (self.port.now() - self.t0, direction, text))

    def write(self, data, what):
        """Every app-side write goes through here (traced)."""
        self.port.write(data)
        self._trace("TX", what)

    def close(self, rc):
        """Writes the trace file (--trace); returns rc."""
        if self.trace_path:
            with open(self.trace_path, "w", encoding="ascii", errors="replace") as f:
                f.write("\n".join(self.trace) + "\n")
        return rc

    def dump_tail(self, n=TRACE_TAIL):
        self.out("trace (last %d of %d frames; --trace FILE keeps all):" % (min(n, len(self.trace)), len(self.trace)))
        for line in self.trace[-n:]:
            self.out("  " + line)

    def pump(self):
        for ftype, seq, body in self.port.poll():
            self._trace("RX", describe(ftype, seq, body))
            try:
                f = msg_decode(ftype, body) if ftype in FIELDS else None
            except ProtoError:
                continue
            if f is None:
                continue
            if ftype == STATUS:
                f["dbg"] = debug_counters(f["extra"])
                self.p2log.feed(f["dbg"], self.port.now())
                self.status = f
                self.status_t = self.port.now()
                self.status_count += 1
            elif ftype == EVENT:
                self.events.append((self.port.now(), f["code"]))
            elif ftype == ACK:
                self.replies.append(("ACK", f["seq"]))
            elif ftype == NAK:
                self.replies.append(("NAK", f))
                if f["reason"] == 0x10:
                    self.nak_version += 1
                elif f["reason"] == 0x02:
                    self.nak_seq += 1
            elif ftype == INFO:
                self.info = f

    def wait(self, seconds, until=None):
        """Pumps for up to `seconds`; returns True as soon as until() is true."""
        end = self.port.now() + seconds
        while True:
            self.pump()
            if until is not None and until():
                return True
            if self.port.now() >= end:
                return False
            self.port.sleep(0.001)

    def hello(self, timeout=3.0):
        """HELLO every 100 ms until INFO (AD-7). Returns the INFO fields or None."""
        hello_seq = self.seq
        self.info = None
        end = self.port.now() + timeout
        last = None
        while self.port.now() < end:
            if last is None or self.port.now() - last >= HELLO_RESEND_S:
                self.write(frame_encode(HELLO, hello_seq, bytes([PROTO_VERSION])), "HELLO seq=%d" % hello_seq)
                self.last_app_write = last = self.port.now()
            self.pump()
            if self.info is not None:
                self.seq = (hello_seq + 1) & 0xFF
                return self.info
            self.port.sleep(0.001)
        return None

    def send(self, ftype, body=b"", seq=None):
        """Writes one frame with the next seq (or `seq`, for a resend)."""
        fresh = seq is None
        if fresh:
            seq = self.seq
        self.write(frame_encode(ftype, seq, body), "%s seq=%d len=%d%s" % (
            TYPE_NAMES.get(ftype, hex(ftype)), seq, len(body), "" if fresh else " (resend)"))
        self.last_app_write = self.port.now()
        if fresh:
            self.seq = (seq + 1) & 0xFF
        return seq, len(self.replies)

    def delimiter(self):
        """One 0x00: ends whatever partial run the controller's receiver may
        hold (AD-7 writes one on open; story 2.7 D40 after a replug)."""
        self.write(b"\x00", "0x00 (delimiter)")

    def reply(self, ticket, timeout=0.5):
        """'ACK', a NAK name, or None: the reply to the frame of `ticket`."""
        seq, idx = ticket
        result = []

        def found():
            for kind, f in self.replies[idx:]:
                if kind == "ACK" and ((f - seq) & 0xFF) < ACK_WINDOW:
                    result.append("ACK")
                    return True
                # A NAK CRC carries got = 0 for any frame (the seq of a
                # damaged frame is unknown): it names no frame, not seq 0.
                if kind == "NAK" and f["reason"] != 0x01 and f["got"] == seq:
                    result.append(NAK_NAMES.get(f["reason"], hex(f["reason"])))
                    return True
            return False

        self.wait(timeout, found)
        return result[0] if result else None

    def request(self, ftype, body=b"", timeout=0.5):
        return self.reply(self.send(ftype, body), timeout)

    def request_reliable(self, ftype, body=b"", timeout=1.0):
        """Stop-and-wait AD-6 sender (window 1, story 2.7 D40): the frame is
        resent with the same seq every OCB_ACK_TIMEOUT_MS (50 ms) while it
        has no reply, or after a transport NAK, until `timeout`. A lost or
        damaged frame gets at most one NAK CRC (got = 0, which names no
        frame) and the frames after it nothing (one transport NAK per
        `expected`), so the resend is what recovers, exactly as the app's
        go-back-N does. Returns 'ACK', a semantic NAK name, or None."""
        ticket = self.send(ftype, body)
        seq = ticket[0]
        end = self.port.now() + timeout
        while True:
            slot_end = min(self.port.now() + ACK_TIMEOUT_S, end)
            r = self.reply(ticket, max(0.0, slot_end - self.port.now()))
            if r is not None and r not in ("SEQ", "FULL"):
                return r
            # A transport NAK came early: still wait out the 50 ms slot, so
            # resends are paced (at most one per OCB_ACK_TIMEOUT_MS).
            self.wait(max(0.0, slot_end - self.port.now()))
            if self.port.now() >= end:
                return r
            # A fresh ticket: only replies received after this resend count.
            ticket = self.send(ftype, body, seq=seq)

    def next_status(self, timeout=0.5, pred=None):
        """The next STATUS received after now (satisfying pred), or None."""
        count = self.status_count

        def ok():
            return self.status_count > count and (pred is None or pred(self.status))

        return self.status if self.wait(timeout, ok) else None

    def event_after(self, t, code):
        for et, c in self.events:
            if et >= t and c == code:
                return et
        return None


class Checks:
    """One line per check, a single PASS / FAIL line at the end."""

    def __init__(self, link):
        self.link = link
        self.failed = []

    def check(self, ok, text):
        self.link.log("%s %s" % ("ok  " if ok else "FAIL", text))
        if not ok:
            self.failed.append(text)
        return ok

    def note(self, text):
        self.link.log("     " + text)

    def finish(self):
        if self.failed:
            self.link.out("FAIL %s" % self.failed[0])
            return self.link.close(1)
        self.link.out("PASS")
        return self.link.close(0)


def _hb_period(args):
    return getattr(args, "hb_period_ms", HEARTBEAT_PERIOD_MS)


def _status_str(st):
    if st is None:
        return "no STATUS"
    d = st.get("dbg") or {}
    return "mode=%d free=%d abort_count=%d last_event=%d link_ok=%d console_ready=%d trips=%s rx_be=%s rx_fe=%s" % (
        st["mode"], st["free"], st["abort_count"], st["last_event"], st["flags"] & STATUS_LINK_OK,
        (st["flags"] & STATUS_CONSOLE_READY) >> 1, d.get("watchdog_trips", "-"), d.get("rx_be", "-"),
        d.get("rx_fe", "-"))


def _link_up(link, c, timeout=0.5):
    """Starts the heartbeat if needed and waits for STATUS link_ok 1."""
    t = link.port.now()
    st = link.next_status(timeout, lambda s: s["flags"] & STATUS_LINK_OK)
    return c.check(st is not None, "STATUS link_ok 1 within %d ms (%s)" % (
        timeout * 1000, ("%.0f ms" % ((link.status_t - t) * 1000)) if st else "never"))


def _try_pair(link):
    """SET_STATE (neutral) and START: their replies, then STOP if needed."""
    r1 = link.request_reliable(SET_STATE, NEUTRAL)
    r2 = link.request_reliable(START, struct.pack("<I", 100000))
    return r1, r2


def _expect_ready(link, args):
    mode = getattr(args, "expect_ready", "auto")
    if mode == "auto":
        st = link.status or link.next_status(0.5)
        return bool(st and st["flags"] & STATUS_CONSOLE_READY)
    return mode == "yes"


def cmd_linkup(port, args):
    """H.8: boot state, NAK NO_LINK, link-up time, bridge_ver, then a hold."""
    link = Link(port, trace=getattr(args, "trace", None))
    c = Checks(link)
    hb = getattr(args, "hb_enabled", True)
    if not c.check(hb, "the heartbeat is on (linkup needs it)"):
        return c.finish()
    if args.expect_fresh:
        st = link.next_status(1.0)
        c.check(st is not None, "STATUS before the first HELLO (gate open)")
        if st is not None:
            c.note("boot counters: %s" % _status_str(st))
            c.check(st["abort_count"] == 0 and st["last_event"] == 0,
                    "fresh boot: abort_count 0 and last_event NONE (the boot break did not latch)")
    info = link.hello()
    if not c.check(info is not None, "INFO received"):
        return c.finish()
    ver = ".".join(str(x) for x in info["bridge_ver"])
    c.note("INFO bridge_ver %s" % ver)
    if args.expect_fresh:
        c.check(info["bridge_ver"] == bytes(3), "bridge_ver 0.0.0 before any heartbeat")
    r1, r2 = _try_pair(link)
    c.check(r1 == "NO_LINK" and r2 == "NO_LINK", "link down: SET_STATE %s, START %s (want NO_LINK twice)" % (r1, r2))
    port.hb_start(_hb_period(args))
    t_hb = port.hb_last
    st = link.next_status(0.5, lambda s: s["flags"] & STATUS_LINK_OK)
    dt = (link.status_t - t_hb) * 1000.0 if st else None
    c.check(st is not None and dt <= 100.0, "link_ok 1 %s after the first heartbeat (want <= 100 ms)" % (
        ("%.1f ms" % dt) if dt is not None else "never"))
    info = link.hello()
    c.check(info is not None and info["bridge_ver"] == PROBE_BRIDGE_VER, "second HELLO: bridge_ver %s (want 0.0.1)" % (
        ".".join(str(x) for x in info["bridge_ver"]) if info else "no INFO"))
    ready = _expect_ready(link, args)
    r1, r2 = _try_pair(link)
    if ready:
        c.check(r1 == "ACK" and r2 == "BAD_STATE", "link up, ready: SET_STATE %s, START %s (want ACK, BAD_STATE)"
                % (r1, r2))
    else:
        c.check(r1 == "NOT_READY" and r2 == "NOT_READY",
                "link up, not ready: SET_STATE %s, START %s (want NOT_READY twice)" % (r1, r2))
    c.check(link.request_reliable(STOP) == "ACK", "STOP ACKed")
    base = link.next_status(0.5)
    if c.check(base is not None, "STATUS before the hold"):
        n0 = link.status_count
        bad = []

        def watch():
            st = link.status
            if link.status_count > n0 and not bad:
                if not st["flags"] & STATUS_LINK_OK:
                    bad.append("link_ok 0")
                elif st["abort_count"] != base["abort_count"]:
                    bad.append("abort_count %d -> %d" % (base["abort_count"], st["abort_count"]))
                elif _trips(st) != _trips(base):
                    bad.append("watchdog_trips %s -> %s" % (_trips(base), _trips(st)))
            return False

        link.wait(args.seconds, watch)
        n = link.status_count - n0
        c.check(not bad and n >= int(args.seconds * 10), "hold %.0f s: %d STATUS, link_ok stayed 1, abort_count and "
                "watchdog_trips unchanged%s" % (args.seconds, n, (" -- " + bad[0]) if bad else ""))
        c.note("last STATUS: %s" % _status_str(link.status))
    c.check(link.nak_version == 0, "no NAK VERSION")
    c.note(port.hb_stats())
    return c.finish()


def _fill(link, count, t_start=1000, step=1000):
    """Sends `count` QUEUE_EVENTs one at a time; stops at the first one that
    gets no ACK (the later seqs would only hit the transport NAK budget and
    blur the trace). Returns how many were ACKed."""
    ok = 0
    for i in range(count):
        r = link.request_reliable(QUEUE_EVENT, queue_event_body(t_start + i * step))
        if r != "ACK":
            link.log("     event %d of %d: %s, the remaining events are not sent" % (i + 1, count, r or "no reply"))
            break
        ok += 1
    return ok


def _stop_heartbeat_and_wait_loss(link, c, what, timeout=1.0):
    port = link.port
    t0 = port.hb_stop()
    et = None
    if link.wait(timeout, lambda: link.event_after(t0, EVT_LINK_LOST) is not None):
        et = link.event_after(t0, EVT_LINK_LOST)
    dt = (et - t0) * 1000.0 if et is not None else None
    lo, hi = LOSS_WINDOW_MS
    c.check(dt is not None and lo <= dt <= hi, "%s: EVENT LINK_LOST %s after the last heartbeat (want %d..%d ms)" % (
        what, ("%.1f ms" % dt) if dt is not None else "never", lo, hi))
    return dt


def cmd_silence(port, args):
    """H.9: heartbeat timeout and recovery, no physical action."""
    link = Link(port, trace=getattr(args, "trace", None))
    c = Checks(link)
    info = link.hello()
    if not c.check(info is not None, "INFO received"):
        return c.finish()
    port.hb_start(_hb_period(args))
    _link_up(link, c)
    cap = info["queue_cap"]
    c.check(_fill(link, 5) == 5, "5 QUEUE_EVENTs ACKed")
    st = link.next_status(0.5, lambda s: s["free"] == cap - 5)
    c.check(st is not None, "STATUS free %d" % (cap - 5))
    base = link.status or {"abort_count": 0}
    _stop_heartbeat_and_wait_loss(link, c, "silence")
    st = link.next_status(0.5)
    c.check(st is not None and not st["flags"] & STATUS_LINK_OK and st["mode"] == MODE_IDLE and
            st["abort_count"] == (base["abort_count"] + 1) & 0xFF and st["last_event"] == EVT_LINK_LOST and
            st["free"] == cap, "STATUS after the loss: %s" % _status_str(st))
    r = link.request_reliable(SET_STATE, NEUTRAL)
    c.check(r == "ABORTED", "SET_STATE after the loss: %s (want ABORTED)" % r)
    r = link.request_reliable(STOP)
    c.check(r == "ACK", "STOP: %s (want ACK)" % r)
    r = link.request_reliable(SET_STATE, NEUTRAL)
    c.check(r == "NO_LINK", "SET_STATE with the link down: %s (want NO_LINK)" % r)
    port.hb_start(_hb_period(args))
    t_hb = port.hb_last
    st = link.next_status(0.5, lambda s: s["flags"] & STATUS_LINK_OK)
    dt = (link.status_t - t_hb) * 1000.0 if st else None
    c.check(st is not None and dt <= 100.0, "heartbeats resumed: link_ok 1 %s later (want <= 100 ms)" % (
        ("%.1f ms" % dt) if dt is not None else "never"))
    ready = _expect_ready(link, args)
    r = link.request_reliable(SET_STATE, NEUTRAL)
    c.check(r == ("ACK" if ready else "NOT_READY"), "SET_STATE after the recovery: %s (want %s)" % (
        r, "ACK" if ready else "NOT_READY"))
    link.request_reliable(STOP)
    c.check(link.nak_version == 0, "no NAK VERSION (the session stayed open)")
    c.note(port.hb_stats())
    return c.finish()


READY_HINT = ("console_ready is 0: it needs a USB host that completed the P2 handshake. On the bench Mac start "
              "`python3 firmware/tools/p2_probe.py handshake --hold 120 &` in the same shell line, or plug the "
              "controller into the console (story 2.3 D15)")


def _need_ready(link, c, seconds=5.0):
    """console_ready 1 from a real host within `seconds` (story 2.3 D15: the
    bench-only fake is gone). False after a FAIL line with the hint."""
    st = link.next_status(seconds, lambda s: s["flags"] & STATUS_CONSOLE_READY)
    return c.check(st is not None, "console_ready 1 from a real host" if st is not None else READY_HINT)


def cmd_direct(port, args):
    """H.11: the DIRECT app timeout and the DTR edge (needs console_ready 1)."""
    link = Link(port, trace=getattr(args, "trace", None))
    c = Checks(link)
    info = link.hello()
    if not c.check(info is not None, "INFO received"):
        return c.finish()
    port.hb_start(_hb_period(args), dtr=True)
    _link_up(link, c)
    if not _need_ready(link, c):
        return c.finish()
    app_ms = info["app_timeout_ms"]
    ping_s = app_ms / APP_PING_DIVISOR / 1000.0

    def keepalive(seconds, until=None):
        end = port.now() + seconds
        while port.now() < end:
            link.send(PING)
            if link.wait(min(ping_s, end - port.now()), until):
                return True
        return False

    # (a) SET_STATE, then PING every app_timeout_ms / 4.
    r = link.request_reliable(SET_STATE, NEUTRAL)
    c.check(r == "ACK", "SET_STATE: %s (want ACK, mode DIRECT)" % r)
    base = link.next_status(0.5)
    aborts = base["abort_count"] if base else 0
    keepalive(args.keepalive_seconds)
    st = link.next_status(0.5)
    c.check(st is not None and st["mode"] == MODE_DIRECT and st["abort_count"] == aborts,
            "(a) PING every %d ms for %.0f s: DIRECT kept (%s)" % (ping_s * 1000, args.keepalive_seconds,
                                                                _status_str(st)))
    # (b) no app frame at all: the app timeout.
    t_last = link.last_app_write
    n_ev = len(link.events)
    st = link.next_status(app_ms / 1000.0 + 1.0, lambda s: s["mode"] == MODE_IDLE)
    dt = (link.status_t - t_last) * 1000.0 if st else None
    c.check(st is not None and app_ms <= dt <= app_ms + 150.0 and st["abort_count"] == (aborts + 1) & 0xFF and
            st["flags"] & STATUS_LINK_OK and len(link.events) == n_ev,
            "(b) no app frame: abort seen %s after the last one (want %d..%d ms), no EVENT, link_ok 1 (%s)" % (
                ("%.0f ms" % dt) if dt is not None else "never", app_ms, app_ms + 150, _status_str(st)))
    # (c) DTR 1 -> 0 while PINGs continue.
    c.check(link.request_reliable(STOP) == "ACK", "STOP ACKed")
    c.check(link.request_reliable(SET_STATE, NEUTRAL) == "ACK", "SET_STATE ACKed")
    aborts = link.status["abort_count"] if link.status else 0
    keepalive(0.2)
    port.hb_set_dtr(False)
    sent0 = port.hb_sent
    link.wait(0.1, lambda: port.hb_sent > sent0)
    t_flip = port.hb_last  # the first DTR-0 heartbeat written

    def dtr_abort_seen():
        return link.status_t is not None and link.status_t >= t_flip and link.status["mode"] == MODE_IDLE

    got = keepalive(0.5, dtr_abort_seen)
    dt = (link.status_t - t_flip) * 1000.0 if got else None
    st = link.status
    c.check(got and dt <= 100.0 and st["abort_count"] == (aborts + 1) & 0xFF and len(link.events) == n_ev,
            "(c) DTR 1 -> 0: abort seen %s after the first DTR-0 heartbeat (want <= 100 ms), no EVENT" % (
                ("%.0f ms" % dt) if dt is not None else "never"))
    # (d) DTR held at 0, PINGs continue: no abort (an edge, not a level).
    c.check(link.request_reliable(STOP) == "ACK", "STOP ACKed")
    c.check(link.request_reliable(SET_STATE, NEUTRAL) == "ACK", "SET_STATE ACKed with DTR 0")
    aborts = link.status["abort_count"] if link.status else 0
    keepalive(3.0)
    st = link.next_status(0.5)
    c.check(st is not None and st["mode"] == MODE_DIRECT and st["abort_count"] == aborts,
            "(d) DTR held at 0 for 3 s with PINGs: no abort (%s)" % _status_str(st))
    link.request_reliable(STOP)
    port.hb_set_dtr(True)
    c.check(link.nak_version == 0, "no NAK VERSION")
    c.note(port.hb_stats())
    return c.finish()


def _queue_piece(link, seconds, n_mid=3):
    """A neutral / A piece of `seconds`, last event NEUTRAL|END."""
    total_us = int(seconds * 1e6)
    ok = True
    for i in range(n_mid):
        t = i * total_us // (n_mid + 1)
        state = button_state(BTN_A) if i % 2 == 0 else NEUTRAL
        ok = link.request_reliable(QUEUE_EVENT, queue_event_body(t, state)) == "ACK" and ok
    end = NEUTRAL[:7] + bytes([0x01])
    ok = link.request_reliable(QUEUE_EVENT, queue_event_body(total_us, end)) == "ACK" and ok
    return ok


def cmd_playing(port, args):
    """H.11: no app-timeout and no DTR rule in PLAYING; a heartbeat loss in
    PLAYING (needs console_ready 1)."""
    link = Link(port, trace=getattr(args, "trace", None))
    c = Checks(link)
    info = link.hello()
    if not c.check(info is not None, "INFO received"):
        return c.finish()
    port.hb_start(_hb_period(args), dtr=True)
    _link_up(link, c)
    if not _need_ready(link, c):
        return c.finish()
    cap = info["queue_cap"]
    for flip in (False, True):
        what = "DTR flip mid-piece" if flip else "no app frames"
        c.check(link.request_reliable(STOP) == "ACK", "STOP ACKed")
        port.hb_set_dtr(True)
        c.check(_queue_piece(link, args.seconds), "%s: %.0f s piece queued" % (what, args.seconds))
        trips0 = _trips(link.status)
        t_start = port.now()
        c.check(link.request_reliable(START, struct.pack("<I", 100000)) == "ACK", "%s: START ACKed" % what)
        left_playing = []

        def watch():
            st = link.status
            if link.status_t and link.status_t > t_start + 0.2 and st["mode"] != MODE_PLAYING and \
                    link.event_after(t_start, EVT_PLAYBACK_DONE) is None and not left_playing:
                left_playing.append(_status_str(st))
            if flip and port.now() - t_start > args.seconds / 2 and port.hb_dtr:
                port.hb_set_dtr(False)
            return link.event_after(t_start, EVT_PLAYBACK_DONE) is not None

        done = link.wait(args.seconds + 2.0, watch)
        c.check(done and not left_playing and _trips(link.status) == trips0 and
                link.event_after(t_start, EVT_LINK_LOST) is None,
                "%s for %.0f s (> app_timeout_ms %d): PLAYING until EVENT PLAYBACK_DONE%s" % (
                    what, args.seconds, info["app_timeout_ms"],
                    (" -- left PLAYING early: " + left_playing[0]) if left_playing else ("" if done else
                                                                                         " -- no PLAYBACK_DONE")))
    port.hb_set_dtr(True)
    # Phase 2: a heartbeat loss in PLAYING.
    c.check(link.request_reliable(STOP) == "ACK", "STOP ACKed")
    c.check(_queue_piece(link, 10.0), "10 s piece queued")
    c.check(link.request_reliable(START, struct.pack("<I", 100000)) == "ACK", "START ACKed")
    link.wait(1.5)
    c.check(link.status is not None and link.status["mode"] == MODE_PLAYING, "PLAYING 1.5 s in")
    _stop_heartbeat_and_wait_loss(link, c, "PLAYING")
    st = link.next_status(0.5)
    c.check(st is not None and st["mode"] == MODE_IDLE and st["free"] == cap, "after the loss: %s (want mode 0, "
            "free %d)" % (_status_str(st), cap))
    port.hb_start(_hb_period(args))
    link.wait(0.1)
    link.request_reliable(STOP)
    c.check(link.nak_version == 0, "no NAK VERSION")
    c.note(port.hb_stats())
    return c.finish()


def cmd_breaks(port, args):
    """H.12: real wire pulls (bridge TX -> controller GP1) and the TX release.
    On a failure the tail of the frame trace is printed (all of it with
    --trace FILE)."""
    link = Link(port, trace=getattr(args, "trace", None))
    rc = _breaks(port, args, link)
    if rc == 1:
        link.dump_tail()
    return link.close(rc)


def _settle(link, armed):
    """Waits until the line has been quiet for BREAKS_SETTLE_S: STATUS keeps
    arriving (no gap above STATUS_GAP_S, i.e. TX was not released again),
    the break and framing-error counters do not move, no new EVENT
    LINK_LOST, and (armed) STATUS link_ok 1. Returns (seconds it took, a
    description of what was absorbed) or None after BREAKS_SETTLE_TIMEOUT_S
    (story 2.7 D42)."""
    port = link.port
    t0 = port.now()
    end = t0 + BREAKS_SETTLE_TIMEOUT_S

    def counters():
        d = (link.status or {}).get("dbg") or {}
        return d.get("rx_be"), d.get("rx_fe")

    first = last = counters()
    n_ev = len(link.events)
    lost = 0
    gaps = 0
    quiet_since = port.now()
    in_gap = False
    while port.now() < end:
        link.wait(0.01)
        now = port.now()
        disturbed = False
        gap = link.status_t is None or now - link.status_t > STATUS_GAP_S
        if gap:
            disturbed = True
            if not in_gap:
                gaps += 1
        in_gap = gap
        k = counters()
        if k != last:
            disturbed = True
            last = k
        new = link.events[n_ev:]
        if new:
            lost += sum(1 for _, code in new if code == EVT_LINK_LOST)
            disturbed = disturbed or any(code == EVT_LINK_LOST for _, code in new)
            n_ev = len(link.events)
        if armed and not (link.status and link.status["flags"] & STATUS_LINK_OK):
            disturbed = True
        if disturbed:
            quiet_since = now
        elif now - quiet_since >= BREAKS_SETTLE_S:
            parts = []
            if first[0] is not None and last[0] is not None:
                parts.append("rx_be %d -> %d, rx_fe %d -> %d while settling" % (first[0], last[0], first[1], last[1]))
            parts.append("%d STATUS gap(s), %d extra LINK_LOST absorbed" % (gaps, lost))
            return now - t0, "; ".join(parts)
    return None


def _breaks(port, args, link):
    c = Checks(link)
    armed = getattr(args, "hb_enabled", True) and not args.no_heartbeat
    if args.mode == "playing" and not armed:
        link.out("FAIL --mode playing needs the heartbeat (the link must be up to START)")
        return 2
    info = link.hello()
    if not c.check(info is not None, "INFO received"):
        return c.finish()
    cap = info["queue_cap"]
    if armed:
        port.hb_start(_hb_period(args))
        _link_up(link, c)
    if args.mode == "playing" and not _need_ready(link, c):
        return c.finish()
    t_next = [1000]
    round_fail = 0
    for rnd in range(1, args.rounds + 1):
        fails_before = len(c.failed)
        if args.mode == "playing":
            c.check(_queue_piece(link, 60.0), "round %d: 60 s piece queued" % rnd)
            c.check(link.request_reliable(START, struct.pack("<I", 100000)) == "ACK", "round %d: START ACKed" % rnd)
        base = link.next_status(0.5)
        if not c.check(base is not None, "round %d: STATUS before the unplug" % rnd):
            break
        link.out("ROUND %d/%d: UNPLUG the bridge-TX wire (GP1 side) now" % (rnd, args.rounds))
        t_prompt = port.now()
        if armed:
            ok = link.wait(BREAKS_UNPLUG_TIMEOUT_S, lambda: link.event_after(t_prompt, EVT_LINK_LOST) is not None)
            if not c.check(ok, "round %d: EVENT LINK_LOST after the unplug" % rnd):
                link.out("FAIL round %d: no EVENT LINK_LOST within %.0f s of the UNPLUG prompt" % (
                    rnd, BREAKS_UNPLUG_TIMEOUT_S))
                return 1
            t_ev = link.event_after(t_prompt, EVT_LINK_LOST)
            n = link.status_count
            link.wait(STATUS_SILENT_S)
            c.check(link.status_count == n, "round %d: STATUS silent for %.0f ms after the EVENT (TX released)" % (
                rnd, (port.now() - t_ev) * 1000.0))
        else:
            def silent():
                return link.status_t is not None and port.now() - max(link.status_t, t_prompt) > STATUS_SILENT_S

            ok = link.wait(BREAKS_UNPLUG_TIMEOUT_S, silent)
            if not c.check(ok, "round %d: STATUS went silent after the unplug (TX released)" % rnd):
                link.out("FAIL round %d: STATUS never went silent within %.0f s of the UNPLUG prompt" % (
                    rnd, BREAKS_UNPLUG_TIMEOUT_S))
                return 1
        if rnd == 1:
            link.out("READ the multimeter on GP0 now (expect about 0 V); waiting %.0f s" % args.meter_seconds)
            link.wait(args.meter_seconds)
        link.out("REPLUG the wire now")
        t_replug = port.now()
        want = (lambda s: s["flags"] & STATUS_LINK_OK) if armed else None
        st = link.next_status(BREAKS_REPLUG_TIMEOUT_S, want)
        if st is None:
            c.check(False, "round %d: STATUS resumed after the replug" % rnd)
            link.out("FAIL round %d: STATUS did not resume%s within %.0f s of the REPLUG prompt (wire still "
                     "unplugged?)" % (rnd, " with link_ok 1" if armed else "", BREAKS_REPLUG_TIMEOUT_S))
            return 1
        c.note("round %d: STATUS resumed %.0f ms after the REPLUG prompt: %s" % (
            rnd, (link.status_t - t_replug) * 1000.0, _status_str(st)))
        # D40: a bouncing contact can leave a partial run in the receiver.
        link.delimiter()
        # D42: a replugged contact bounces for a while; every break in that
        # time belongs to this round, never to the next one.
        settled = _settle(link, armed)
        if settled is None:
            c.check(False, "round %d: the line settled after the replug" % rnd)
            link.out("FAIL round %d: the line did not settle within %.0f s after the replug (breaks kept coming: "
                     "a loose contact? press the wire in and re-run)" % (rnd, BREAKS_SETTLE_TIMEOUT_S))
            return 1
        c.note("round %d: line settled %.0f ms after STATUS resumed (%s)" % (rnd, settled[0] * 1000.0, settled[1]))
        link.delimiter()
        st = link.next_status(0.5, want) or link.status
        bd, sd = base.get("dbg") or {}, st.get("dbg") or {}
        if bd and sd:
            c.note("round %d: rx_be %d -> %d, rx_fe %d -> %d" % (rnd, bd["rx_be"], sd["rx_be"], bd["rx_fe"],
                                                                 sd["rx_fe"]))
        if armed:
            c.check(st["abort_count"] == (base["abort_count"] + 1) & 0xFF and st["mode"] == MODE_IDLE and
                    st["free"] == cap and st["last_event"] == EVT_LINK_LOST,
                    "round %d: abort_count +1, mode IDLE, free %d, last_event LINK_LOST" % (rnd, cap))
            r = link.request_reliable(QUEUE_EVENT, queue_event_body(t_next[0]))
            c.check(r == "ABORTED", "round %d: QUEUE_EVENT %s (want ABORTED)" % (rnd, r))
            r = link.request_reliable(STOP)
            c.check(r == "ACK", "round %d: STOP %s (want ACK)" % (rnd, r))
            t_next[0] = 1000
        else:
            c.check(st["abort_count"] == base["abort_count"] and st["mode"] == MODE_IDLE,
                    "round %d: abort_count unchanged, mode IDLE" % rnd)
        ok = _fill(link, args.count, t_next[0])
        t_next[0] += args.count * 1000
        c.check(ok == args.count, "round %d: %d of %d events ACKed in the open session" % (rnd, ok, args.count))
        c.check(link.nak_version == 0 and (armed or link.nak_seq == 0), "round %d: no NAK VERSION%s" % (
            rnd, "" if armed else ", no NAK SEQ"))
        good = len(c.failed) == fails_before
        round_fail += 0 if good else 1
        link.out("round %d: %s" % (rnd, "PASS" if good else "FAIL"))
        if args.mode == "playing":
            link.request_reliable(STOP)
    if armed:
        c.note(port.hb_stats())
    return c.finish()


# ------------------------------------------------------------ story 2.3
P2_WAIT_S = 120.0      # p2check: time for the owner to plug the controller in
P2_STEP_S = 90.0       # p2watch: time per owner action
P2_STABLE_S = 5.0      # p2check: stable polling before the period is read
P2_SETTLE_S = 1.0      # p2watch: the state must hold this long (2.7 D42)
P2_WAKE_READY_S = 30.0  # p2watch: ready again within this after the controller is back
P2_PERIOD_RANGE_US = (500, 20000)
STATUS_PERIOD_S = 0.050  # docs/link-protocol.md: a STATUS every 50 ms
EVT_USB_SUSPENDED = 0x05
HID_PROFILE_P2 = 2


def _ready(st):
    return bool(st and st["flags"] & STATUS_CONSOLE_READY)


def _p2_sequence(recs):
    return ", ".join(p2_log_str(k, i) for k, i in recs) if recs else "(none)"


def cmd_p2check(port, args):
    """H.15: the console enumerates the controller and completes the P2
    handshake; prints the console's real sequence and the measured period."""
    link = Link(port, trace=getattr(args, "trace", None))
    c = Checks(link)
    if getattr(args, "hb_enabled", True):
        port.hb_start(_hb_period(args))
    link.out(">>> PLUG THE CONTROLLER INTO THE SWITCH TOP USB-C PORT (Pro Controller Wired Communication ON). "
             "Waiting up to %.0f s." % args.wait_s)
    t0 = port.now()
    info = link.hello(timeout=args.wait_s)  # HELLO every 100 ms while the controller boots
    if not c.check(info is not None, "INFO received (the controller is powered and the link answers)"):
        return c.finish()
    c.check(info["hid_profile"] == HID_PROFILE_P2, "INFO hid_profile %d (want 2, P2)" % info["hid_profile"])
    left = max(0.5, args.wait_s - (port.now() - t0))
    st = link.next_status(left, _ready)
    if not c.check(st is not None, "console_ready 1 within %.0f s of the prompt%s" % (
            args.wait_s, (" (%.1f s)" % (link.status_t - t0)) if st else "")):
        c.note("P2 log so far: %s" % _p2_sequence(link.p2log.since(t0)))
        return c.finish()
    base = link.status
    link.wait(P2_STABLE_S)
    st = link.next_status(0.5)
    c.check(st is not None and _ready(st) and st["abort_count"] == base["abort_count"],
            "%.0f s of stable polling: console_ready stayed 1, abort_count unchanged (%s)" % (
                P2_STABLE_S, _status_str(st)))
    info = link.hello()
    period = info["report_period_us"] if info else 0
    lo, hi = P2_PERIOD_RANGE_US
    c.check(info is not None and lo <= period <= hi, "INFO report_period_us %d (want %d..%d)" % (period, lo, hi))
    seq = link.p2log.since(t0)
    c.check(any(k == 0x06 for k, _ in seq), "the P2 log shows the handshake (HANDSHAKE record)")
    c.note("console sequence (%d records%s): %s" % (len(seq), (", %d lost" % link.p2log.gaps)
                                                    if link.p2log.gaps else "", _p2_sequence(seq)))
    refused = [i for k, i in seq if k == 0x04]
    if refused:
        c.note("refused by the profile (C7): %s" % " ".join("%02x" % i for i in refused))
    link.out("H.15 median IN interval: %d us (INFO.report_period_us)" % period)
    c.check(link.nak_version == 0, "no NAK VERSION")
    return c.finish()


def cmd_p2watch(port, args):
    """H.16: sleep, wake, Wired Communication off and on (optional undock and
    dock); one prompt per action, each transition checked over the link."""
    link = Link(port, trace=getattr(args, "trace", None))
    c = Checks(link)
    if getattr(args, "hb_enabled", True):
        port.hb_start(_hb_period(args))
    if not c.check(link.hello() is not None, "INFO received"):
        return c.finish()
    if not c.check(link.next_status(1.0, _ready) is not None,
                   "console_ready 1 at the start (run p2check first: the controller in the console's top port)"):
        return c.finish()
    steps = [("down", "sleep", "PUT THE CONSOLE TO SLEEP (press its power button once)"),
             ("up", "wake", "WAKE THE CONSOLE (power button; press A on a controller if it asks)"),
             ("off", "wired off",
              "SET 'Pro Controller Wired Communication' OFF (System Settings > Controllers and Accessories)"),
             ("up", "wired on", "SET 'Pro Controller Wired Communication' back ON")]
    if args.dock:
        steps += [("down", "undock", "UNDOCK THE CONSOLE (lift it out of the dock)"),
                  ("up", "dock", "DOCK THE CONSOLE AGAIN")]
    for kind, label, prompt in steps:
        base = link.status
        aborts0 = base["abort_count"] if base else 0
        link.out(">>> %s. Waiting up to %.0f s." % (prompt, args.step_s))
        t0 = port.now()
        n_ev = len(link.events)
        if kind in ("down", "off"):
            last_ready = [link.status_t]
            silent = []

            def dropped():
                if _ready(link.status):
                    last_ready[0] = link.status_t
                    return False
                return link.status_t is not None and link.status_t > t0

            def gone():
                return link.status_t is not None and port.now() - max(link.status_t, t0) > P2_SETTLE_S

            got = link.wait(args.step_s, lambda: dropped() or (gone() and not silent.append(1)))
            if not c.check(got, "%s: console_ready 0 or STATUS stopped" % label):
                return c.finish()
            if silent:
                c.note("STATUS stopped: the port lost power (the controller is off until the console wakes)")
            else:
                st = link.status
                ev = any(code == EVT_USB_SUSPENDED for _, code in link.events[n_ev:])
                c.note("console_ready 0 (STATUS %.0f ms after the last one with ready 1), last_event %d%s, "
                       "abort_count %d -> %d" % ((link.status_t - last_ready[0]) * 1000.0, st["last_event"],
                                                 ", EVENT USB_SUSPENDED" if ev else "", aborts0, st["abort_count"]))
                c.check(((st["abort_count"] - aborts0) & 0xFF) <= 1, "at most one abort for one USB-down event")
                if kind == "off":
                    c.check(link.status_t - last_ready[0] <= 0.150 + STATUS_PERIOD_S,
                            "ready fell within one STATUS period of 150 ms (%.0f ms between STATUS frames)" % (
                                (link.status_t - last_ready[0]) * 1000.0))
        else:
            silent_before = link.status_t is None or port.now() - link.status_t > P2_SETTLE_S

            def ready_again():
                return _ready(link.status) and link.status_t > t0

            got = link.wait(args.step_s, ready_again)
            if not c.check(got, "%s: console_ready 1 again within %.0f s" % (label, args.step_s)):
                c.note("P2 log of this step: %s" % _p2_sequence(link.p2log.since(t0)))
                return c.finish()
            # From the console's first request of this step (the first P2 log
            # record), else from the first STATUS after a power cut, else from
            # the prompt: the human's reaction time is not counted when the
            # controller tells us when the console came back.
            recs = [rt for rt, _, _ in link.p2log.records if rt > t0]
            ref, what = (recs[0], "the console's first request") if recs else (t0, "the prompt")
            if silent_before and not recs:
                what = "the prompt (STATUS had stopped)"
            dt = link.status_t - ref
            c.check(dt <= P2_WAKE_READY_S, "%s: ready %.1f s after %s (want <= %.0f s)" % (
                label, dt, what, P2_WAKE_READY_S))
            seq = link.p2log.since(t0)
            kinds = [k for k, _ in seq]
            if kind == "up" and any(k == 0x07 for k in kinds):
                first = kinds.index(0x07)
                c.check(0x06 in kinds[first:], "the P2 log shows a restart, then a new handshake")
            c.note("P2 log of this step: %s" % _p2_sequence(seq))
            link.hello()  # a rebooted controller needs the session again
        link.wait(P2_SETTLE_S)
    c.check(link.nak_version <= 1, "no unexpected NAK VERSION")
    return c.finish()



# Canonical inputs for `press` (AD-10: hat clockwise from up, 8 neutral).
PRESS_HATS = {"up": 0, "right": 2, "down": 4, "left": 6}
PRESS_BUTTONS = {"Y": 0x0001, "B": 0x0002, "A": 0x0004, "X": 0x0008, "L": 0x0010, "R": 0x0020, "ZL": 0x0040,
                 "ZR": 0x0080, "MINUS": 0x0100, "PLUS": 0x0200, "HOME": 0x1000}


def press_state(hat=None, button=None):
    """The 8-byte canonical state of one D-pad direction or one button."""
    b = PRESS_BUTTONS[button] if button else 0
    h = PRESS_HATS[hat] if hat else 8
    return struct.pack("<HBBBBBB", b, h, 0x80, 0x80, 0x80, 0x80, 0)


def cmd_press(port, args):
    """Story 2.3 D35 bench aid: presses one D-pad direction or button in
    DIRECT (SET_STATE, held --hold-ms with PINGs, then NEUTRAL), --count
    times, --gap-ms apart; needs console_ready 1 (a host holding the P2
    handshake) and a debug controller build (D35). PASS when every
    SET_STATE was ACKed."""
    link = Link(port, trace=getattr(args, "trace", None))
    c = Checks(link)
    if getattr(args, "hb_enabled", True):
        port.hb_start(_hb_period(args))
    if not c.check(link.hello() is not None, "INFO received"):
        return c.finish()
    _link_up(link, c)
    if not _need_ready(link, c):
        return c.finish()
    state = press_state(args.hat, args.button)
    what = ("D-pad %s" % args.hat) if args.hat else ("button %s" % args.button)
    acks = 0
    for i in range(args.count):
        r = link.request_reliable(SET_STATE, state)
        acks += r == "ACK"
        end = port.now() + args.hold_ms / 1000.0
        while port.now() < end:
            link.wait(min(0.05, max(0.0, end - port.now())))
            link.send(PING)
        r2 = link.request_reliable(SET_STATE, NEUTRAL)
        acks += r2 == "ACK"
        link.log("press %d/%d: %s held %d ms (%s, release %s)" % (i + 1, args.count, what, args.hold_ms, r, r2))
        end = port.now() + args.gap_ms / 1000.0
        while port.now() < end:
            link.wait(min(0.2, max(0.0, end - port.now())))
            link.send(PING)
    c.check(acks == 2 * args.count, "%d of %d SET_STATE ACKed" % (acks, 2 * args.count))
    c.check(link.request_reliable(STOP) == "ACK", "STOP ACKed")
    return c.finish()


# ------------------------------------------------------------ story 2.5
# selfplay (H.18) and hangcheck (H.2a, H.2b): docs/test-plan.md, "Bench rig
# for the timeline and watchdog (Story 2.5)".
SELFPLAY_WAIT_S = 120.0     # time to plug the controller into the console
SELFPLAY_STABLE_S = 2.0     # console_ready must hold this long (plug bounces)
SELFPLAY_LEAD_MS = 3000
SELFPLAY_TAIL_S = 10.0      # after the last event, before giving up on PLAYBACK_DONE
SELFPLAY_LOAD_S = 1.0       # the loader fills the queue within one pass of the STOP
FIRST_INPUT_PRESSES = 3     # D17: D-pad down, never A
FIRST_INPUT_HOLD_MS = 150
FIRST_INPUT_GAP_MS = 300
STACK_LIMIT_B = 1536        # D15, issue #21
LOOP_LIMIT_US = 5000        # OCB_LIVE_LOOP_BOUND_US (ocb_core/live.h)
HANGCHECK_WAIT_S = 120.0
MODE_IDLE = 0
EVT_PLAYBACK_DONE = 0x01
EVT_LATE = 0x03
EVT_UNDERRUN = 0x04
TIMELINE_DEFAULT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "tests", "testtl_expected.txt")
_TIMELINE_RE = re.compile(r"(\d+) notes in (\d+) events, the last event at (\d+) us")


def read_timeline(path):
    """(notes, events, last_event_us) from the header of the expected-notes
    file the generator writes (tools/gen_test_timeline.py), or None."""
    try:
        with open(path, "r", encoding="utf-8") as f:
            head = f.read(4096)
    except OSError:
        return None
    m = _TIMELINE_RE.search(head)
    return (int(m.group(1)), int(m.group(2)), int(m.group(3))) if m else None


def selfplay_verdict(run, n_events, period_us):
    """The first broken limit of one selfplay run (H.18), or None. `run` has
    the v4 fields, `events` (codes received during the run) and
    `abort_delta` (abort_count after minus before)."""
    if run["late_n"] != n_events:
        return "late_n %d, the table has %d events" % (run["late_n"], n_events)
    if run["late_max_us"] > period_us:
        return "late_max_us %d above one report period (%d us)" % (run["late_max_us"], period_us)
    bad = [EVENT_NAMES.get(e, hex(e)) for e in run["events"] if e in (EVT_LATE, EVT_UNDERRUN)]
    if bad:
        return "EVENT %s during the run" % bad[0]
    if run["abort_delta"] != 1 or EVT_PLAYBACK_DONE not in run["events"]:
        return "abort_count moved by %d (want 1, the END) or no PLAYBACK_DONE" % run["abort_delta"]
    if run["seq_fallbacks"] != 0:
        return "seq_fallbacks %d (the published state fell back to NEUTRAL)" % run["seq_fallbacks"]
    if run["core0_pass_max_us"] > LOOP_LIMIT_US:
        return "core0_pass_max_us %d above %d" % (run["core0_pass_max_us"], LOOP_LIMIT_US)
    if run["loop_max_us"] > LOOP_LIMIT_US:
        return "loop_max_us %d above %d" % (run["loop_max_us"], LOOP_LIMIT_US)
    if run["stack_used_max"] > STACK_LIMIT_B:
        return "stack_used_max %d B above %d B (issue #21: raise PICO_CORE1_STACK_SIZE)" % (
            run["stack_used_max"], STACK_LIMIT_B)
    return None


def paste_line(i, run, period_us):
    mean = run["late_sum_us"] / float(run["late_n"]) if run["late_n"] else 0.0
    return ("PASTE_LINE: run %d | late max %d us, mean %.1f us, hist %s | IN median %d, min %d, p95 %d, max %d us, "
            "slow %d | loop max %d us | core0 pass max %d us | stack %d B | seq retries %d, fallbacks %d" % (
                i, run["late_max_us"], mean, "/".join(str(x) for x in run["late_hist"]), period_us,
                run["in_min_us"], run["in_p95_us"], run["in_max_us"], run["in_slow"], run["loop_max_us"],
                run["core0_pass_max_us"], run["stack_used_max"], run["seq_retries"], run["seq_fallbacks"]))


def _v4_of(st):
    d = (st or {}).get("dbg") or {}
    return d.get("v4")


def _wait_ready_stable(link, deadline, stable_s):
    """console_ready 1 continuously for stable_s (absorbs plug bounces)."""
    since = [None]

    def ok():
        if not _ready(link.status):
            since[0] = None
            return False
        if since[0] is None:
            since[0] = link.status_t
        return link.status_t - since[0] >= stable_s

    return link.wait(max(0.0, deadline - link.port.now()), ok)


def _direct_presses(link, state, count, hold_ms, gap_ms):
    """Presses `state` count times in DIRECT (PINGs keep the app timeout
    away), NEUTRAL in between; returns the SET_STATEs ACKed."""
    acks = 0
    for _ in range(count):
        acks += link.request_reliable(SET_STATE, state) == "ACK"
        end = link.port.now() + hold_ms / 1000.0
        while link.port.now() < end:
            link.wait(min(0.05, max(0.0, end - link.port.now())))
            link.send(PING)
        acks += link.request_reliable(SET_STATE, NEUTRAL) == "ACK"
        end = link.port.now() + gap_ms / 1000.0
        while link.port.now() < end:
            link.wait(min(0.2, max(0.0, end - link.port.now())))
            link.send(PING)
    return acks


def _start_timed(link, lead_us, tries=3):
    """START with the host's timing: (reply, t_send, t_reply) of the try
    that got a reply (the same seq is resent every 50 ms, at most `tries`)."""
    ticket = link.send(START, struct.pack("<I", lead_us))
    seq = ticket[0]
    for i in range(tries):
        t_send = link.port.now()
        r = link.reply(ticket, ACK_TIMEOUT_S)
        if r is not None:
            return r, t_send, link.port.now()
        if i + 1 < tries:
            ticket = link.send(START, struct.pack("<I", lead_us), seq=seq)
    return None, None, None


def cmd_selfplay(port, args):
    """H.18: the built-in melody of a fw-debug-timeline controller plays on
    the console from the controller's own clock; the firmware's self-timing
    (debug STATUS v4) is checked against the limits of story 2.5."""
    link = Link(port, trace=getattr(args, "trace", None))
    c = Checks(link)
    tl = read_timeline(args.timeline)
    if not c.check(tl is not None, "the expected-notes file %s names the table (notes, events, last event)"
                   % args.timeline):
        return c.finish()
    n_notes, n_events, end_us = tl
    if getattr(args, "hb_enabled", True):
        port.hb_start(_hb_period(args))
    link.out(">>> PLUG THE CONTROLLER INTO THE SWITCH TOP USB-C PORT; the game must be in ocarina free play "
             "(open it with the Joy-Con first). Waiting up to %.0f s." % args.wait_s)
    t0 = port.now()
    deadline = t0 + args.wait_s
    info = link.hello(timeout=args.wait_s)
    if not c.check(info is not None, "INFO received (the controller is powered and the link answers)"):
        return c.finish()
    if not _link_up(link, c, timeout=1.0):
        return c.finish()
    if not c.check(_wait_ready_stable(link, deadline, SELFPLAY_STABLE_S),
                   "console_ready 1 for %.0f s in a row within %.0f s of the prompt" % (SELFPLAY_STABLE_S,
                                                                                       args.wait_s)):
        c.note("P2 log so far: %s" % _p2_sequence(link.p2log.since(t0)))
        return c.finish()
    period = 0
    while True:
        info = link.hello()
        period = info["report_period_us"] if info else 0
        if period or port.now() >= deadline:
            break
        link.wait(0.5)
    if not c.check(period > 0, "INFO report_period_us %d (the console's median IN interval)" % period):
        return c.finish()
    cap = info["queue_cap"]
    if args.first_input == "hat-down":
        acks = _direct_presses(link, press_state(hat="down"), FIRST_INPUT_PRESSES, FIRST_INPUT_HOLD_MS,
                               FIRST_INPUT_GAP_MS)
        c.check(acks == 2 * FIRST_INPUT_PRESSES, "first input: D-pad down x%d in DIRECT (%d of %d SET_STATE ACKed; "
                "the console shows a wired controller only after its first input)" % (
                    FIRST_INPUT_PRESSES, acks, 2 * FIRST_INPUT_PRESSES))
    lead_us = int(args.lead_ms) * 1000
    passes = 0
    for i in range(1, args.runs + 1):
        if not c.check(link.request_reliable(STOP) == "ACK", "run %d: STOP ACKed (IDLE: the timeline loads)" % i):
            break
        st = link.next_status(SELFPLAY_LOAD_S, lambda s: s["mode"] == MODE_IDLE and s["free"] != cap)
        free = st["free"] if st else cap
        if free == cap:
            c.check(False, "run %d: the queue is still empty after STOP: not a fw-debug-timeline build?" % i)
            break
        if not c.check(free == cap - n_events, "run %d: STATUS free %d = queue_cap %d - %d table events (a dirty "
                       "or different queue refuses the run)" % (i, free, cap, n_events)):
            break
        abort0 = st["abort_count"]
        n_ev = len(link.events)
        r, t_send, t_reply = _start_timed(link, lead_us)
        if not c.check(r == "ACK", "run %d: START lead %d ms ACKed (%s)" % (i, args.lead_ms, r)):
            break
        rtt = t_reply - t_send
        link.out("T0_HOST_MONO=%.6f" % (t_reply + lead_us / 1e6 - rtt / 2.0))
        link.log("run %d: playing %d notes, %.1f s, round trip %.1f ms" % (i, n_notes, end_us / 1e6,
                                                                          rtt * 1000.0))
        budget = lead_us / 1e6 + end_us / 1e6 + SELFPLAY_TAIL_S

        def ended():
            return any(code in (EVT_PLAYBACK_DONE, EVT_LATE, EVT_UNDERRUN) for _, code in link.events[n_ev:])

        if not c.check(link.wait(budget, ended), "run %d: an end EVENT within %.0f s" % (i, budget)):
            break
        st = link.next_status(0.5)
        v = _v4_of(st)
        if not c.check(v is not None, "run %d: the debug STATUS carries the v4 block (a story 2.5 debug build)" % i):
            break
        run = dict(v)
        run["events"] = [code for _, code in link.events[n_ev:]]
        run["abort_delta"] = (st["abort_count"] - abort0) & 0xFF
        link.log("run %d: events %s" % (i, ", ".join(EVENT_NAMES.get(e, hex(e)) for e in run["events"])))
        link.log("run %d: %s" % (i, v4_str(v)))
        link.out(paste_line(i, run, period))
        why = selfplay_verdict(run, n_events, period)
        if c.check(why is None, "run %d: %s" % (i, why or "late_n = %d, late_max %d <= %d us, no LATE/UNDERRUN, "
                                                       "fallbacks 0, loop and pass max <= %d us, stack <= %d B" % (
                                                           n_events, v["late_max_us"], period, LOOP_LIMIT_US,
                                                           STACK_LIMIT_B))):
            passes += 1
    link.request_reliable(STOP)
    c.note("%d of %d runs passed; listen: did the melody play, every note at the right pitch?" % (passes, args.runs))
    return c.finish()


def cmd_hangcheck(port, args):
    """H.2a, H.2b: after a liveness reboot the controller comes back fresh
    and its debug STATUS v4 names the cause (CORE0_STALE or CORE1_STALE)."""
    link = Link(port, trace=getattr(args, "trace", None))
    c = Checks(link)
    if getattr(args, "hb_enabled", True):
        port.hb_start(_hb_period(args))
    want = {"core0": 1, "core1": 2}[args.expect]
    link.out(">>> Waiting up to %.0f s for the controller to answer after its reboot (HELLO every 100 ms)."
             % args.wait_s)
    info = link.hello(timeout=args.wait_s)
    if not c.check(info is not None, "INFO received (the controller is back)"):
        return c.finish()
    st = link.next_status(1.0)
    v = _v4_of(st)
    if not c.check(v is not None, "the debug STATUS carries the v4 block (a story 2.5 debug build)"):
        return c.finish()
    c.check(v["boot_cause"] == want, "boot_cause %s (want %s)" % (
        BOOT_CAUSES.get(v["boot_cause"], v["boot_cause"]), BOOT_CAUSES[want]))
    if args.reboots_before is not None:
        c.check(v["reboot_count"] == (args.reboots_before + 1) & 0xFF, "reboot_count %d (want %d + 1)" % (
            v["reboot_count"], args.reboots_before))
    else:
        c.check(v["reboot_count"] >= 1, "reboot_count %d (at least 1 since power-on)" % v["reboot_count"])
    c.check(st["mode"] == MODE_IDLE and st["abort_count"] == 0, "fresh boot: mode IDLE, abort_count 0 (%s)"
            % _status_str(st))
    c.note("console_ready %d (0 until a USB host completes the handshake)" % (1 if _ready(st) else 0))
    return c.finish()


def cmd_hello(port, args):
    info = do_hello(port, timeout=args.seconds)
    if info is None:
        print("no INFO")
        return 1
    return 0


def min_inflight():
    """The largest single QUEUE_EVENT write: the frame plus the largest
    --garbage prefix (junk and its 0x00). A smaller cap could never send."""
    return len(frame_encode(QUEUE_EVENT, 0, queue_event_body(0xFFFFFFFF))) + GARBAGE_MAX + 1


def pace_share_arg(text):
    try:
        v = float(text)
    except ValueError:
        raise argparse.ArgumentTypeError("not a number: %r" % text) from None
    if not 0.0 < v <= 1.0:
        raise argparse.ArgumentTypeError("must be in (0, 1], got %s" % text)
    return v


def max_inflight_arg(text):
    try:
        v = int(text)
    except ValueError:
        raise argparse.ArgumentTypeError("not an integer: %r" % text) from None
    if v != 0 and v < min_inflight():
        raise argparse.ArgumentTypeError("must be 0 (no cap) or at least %d bytes (one QUEUE_EVENT plus the "
                                         "largest --garbage prefix), got %d" % (min_inflight(), v))
    return v


def hb_period_arg(text):
    try:
        v = int(text)
    except ValueError:
        raise argparse.ArgumentTypeError("not an integer: %r" % text) from None
    if not 5 <= v <= 40:
        raise argparse.ArgumentTypeError("must be in 5..40 ms (the 80 ms link timeout needs a few heartbeats), "
                                         "got %d" % v)
    return v


def seconds_arg(text):
    try:
        v = float(text)
    except ValueError:
        raise argparse.ArgumentTypeError("not a number: %r" % text) from None
    if not 0.0 <= v <= 3600.0:
        raise argparse.ArgumentTypeError("must be in 0..3600 s, got %s" % text)
    return v


def positive_int_arg(text):
    try:
        v = int(text)
    except ValueError:
        raise argparse.ArgumentTypeError("not an integer: %r" % text) from None
    if not 1 <= v <= 1000:
        raise argparse.ArgumentTypeError("must be in 1..1000, got %d" % v)
    return v


def lead_ms_arg(text):
    try:
        v = int(text)
    except ValueError:
        raise argparse.ArgumentTypeError("not an integer: %r" % text) from None
    if not 0 <= v <= 60000:
        raise argparse.ArgumentTypeError("must be in 0..60000 ms, got %d" % v)
    return v


# Story 2.7 subcommands: the heartbeat is on by default.
HEARTBEAT_COMMANDS = ("linkup", "silence", "direct", "playing", "breaks", "p2check", "p2watch", "press", "selfplay",
                      "hangcheck")


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--port", required=True)
    ap.add_argument("--baud", type=int, default=DEFAULT_BAUD)
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--no-pace", action="store_true",
                    help="write QUEUE_EVENTs as fast as the window allows (default: paced to the UART rate)")
    ap.add_argument("--pace-share", type=pace_share_arg, default=PACE_SHARE,
                    help="pacing rate as a fraction of baud/10 bytes/s, in (0, 1] (default %(default)s)")
    ap.add_argument("--max-inflight", type=max_inflight_arg, default=MAX_INFLIGHT,
                    help="max bytes of unconfirmed QUEUE_EVENTs on the wire, 0 = no cap, else at least "
                         "%d (default %%(default)s)" % min_inflight())
    ap.add_argument("--heartbeat", action="store_true",
                    help="send the emulated bridge HEARTBEAT (default for linkup, silence, direct, playing, breaks; "
                         "opt-in for the other subcommands)")
    ap.add_argument("--no-heartbeat", dest="no_heartbeat_global", action="store_true",
                    help="never send the emulated HEARTBEAT")
    ap.add_argument("--trace", metavar="FILE",
                    help="linkup, silence, direct, playing, breaks: write every frame sent and received, with "
                         "a timestamp, to FILE (breaks also prints the tail of it when it fails); the 2.6 "
                         "subcommands ignore it")
    ap.add_argument("--hb-period-ms", type=hb_period_arg, default=HEARTBEAT_PERIOD_MS,
                    help="heartbeat period, 5..40 ms (default %(default)s)")
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
    p = sub.add_parser("linkup", help="H.8: boot state, NAK NO_LINK, link-up time, bridge_ver, hold")
    p.add_argument("--seconds", type=seconds_arg, default=20.0, help="hold time (default %(default)s)")
    p.add_argument("--expect-fresh", action="store_true",
                   help="require abort_count 0 and last_event NONE before the first HELLO (just power-cycled)")
    p.add_argument("--expect-ready", choices=("auto", "no", "yes"), default="auto",
                   help="console_ready: no (no USB host), yes (a host completed the handshake: the console or "
                        "p2_probe.py handshake --hold), auto (from STATUS)")
    p = sub.add_parser("silence", help="H.9: stop the heartbeat, EVENT LINK_LOST in 80..120 ms, recovery")
    p.add_argument("--expect-ready", choices=("auto", "no", "yes"), default="auto")
    p = sub.add_parser("direct", help="H.11: DIRECT app timeout and DTR edge (needs console_ready 1)")
    p.add_argument("--keepalive-seconds", type=seconds_arg, default=5.0)
    p = sub.add_parser("playing", help="H.11: no app timeout or DTR rule in PLAYING; loss in PLAYING "
                                       "(needs console_ready 1)")
    p.add_argument("--seconds", type=seconds_arg, default=3.0, help="piece length (default %(default)s)")
    p = sub.add_parser("breaks", help="H.12: unplug / replug the bridge-TX wire, TX release, session continues")
    p.add_argument("--rounds", type=positive_int_arg, default=10)
    p.add_argument("--mode", choices=("idle", "playing"), default="idle",
                   help="playing (needs console_ready 1): a 60 s piece is playing at each unplug")
    p.add_argument("--no-heartbeat", action="store_true", help="the unarmed case: no heartbeat at all")
    p.add_argument("--meter-seconds", type=seconds_arg, default=15.0,
                   help="round 1: time to read the multimeter on GP0 (default %(default)s)")
    p.add_argument("--count", type=positive_int_arg, default=5, help="events sent after each replug")
    p = sub.add_parser("p2check", help="H.15: console enumerates and handshakes; sequence and polling period")
    p.add_argument("--wait-s", type=seconds_arg, default=P2_WAIT_S,
                   help="time to plug the controller into the console (default %(default)s)")
    p = sub.add_parser("p2watch", help="H.16: sleep, wake, Wired Communication off and on, re-enumeration")
    p.add_argument("--step-s", type=seconds_arg, default=P2_STEP_S, help="time per action (default %(default)s)")
    p.add_argument("--dock", action="store_true", help="also undock and dock")
    p = sub.add_parser("press", help="story 2.3 D35: press a D-pad direction or a button in DIRECT, N times")
    g = p.add_mutually_exclusive_group(required=True)
    g.add_argument("--hat", choices=sorted(PRESS_HATS))
    g.add_argument("--button", choices=sorted(PRESS_BUTTONS))
    p.add_argument("--hold-ms", type=positive_int_arg, default=150, help="1..1000 ms (default %(default)s)")
    p.add_argument("--gap-ms", type=positive_int_arg, default=500, help="1..1000 ms (default %(default)s)")
    p.add_argument("--count", type=positive_int_arg, default=5)
    p = sub.add_parser("selfplay", help="H.18 (story 2.5): the built-in melody of fw-debug-timeline on the console, "
                                        "with the firmware's self-timing")
    p.add_argument("--lead-ms", type=lead_ms_arg, default=SELFPLAY_LEAD_MS, help="START lead (default %(default)s)")
    p.add_argument("--wait-s", type=seconds_arg, default=SELFPLAY_WAIT_S,
                   help="time to plug the controller into the console (default %(default)s)")
    p.add_argument("--first-input", choices=("hat-down", "none"), default="hat-down",
                   help="D-pad down x3 in DIRECT before the first run (never A; default %(default)s)")
    p.add_argument("--runs", type=positive_int_arg, default=1, help="STOP, START repeated (default %(default)s)")
    p.add_argument("--timeline", default=TIMELINE_DEFAULT,
                   help="the expected-notes file of the table (default firmware/tests/testtl_expected.txt)")
    p = sub.add_parser("hangcheck", help="H.2a/H.2b (story 2.5): after a liveness reboot, the boot cause in STATUS v4")
    p.add_argument("--expect", choices=("core0", "core1"), required=True)
    p.add_argument("--wait-s", type=seconds_arg, default=HANGCHECK_WAIT_S,
                   help="time for the controller to come back (default %(default)s)")
    p.add_argument("--reboots-before", type=int, default=None,
                   help="the reboot_count before the hang, when known (then exactly one more is wanted)")
    args = ap.parse_args(argv)
    if args.cmd in HEARTBEAT_COMMANDS:
        args.hb_enabled = not args.no_heartbeat_global
    else:
        args.hb_enabled = args.heartbeat and not args.no_heartbeat_global
    if args.cmd != "breaks":
        args.no_heartbeat = False
    port = Port(args.port, args.baud)
    if args.hb_enabled and args.cmd not in HEARTBEAT_COMMANDS:
        port.hb_start(args.hb_period_ms)
    handler = {"hello": cmd_hello, "watch": cmd_watch, "fill": cmd_fill, "soak": cmd_soak,
               "noise": cmd_noise, "try": cmd_try, "loopback": cmd_loopback, "linkup": cmd_linkup,
               "silence": cmd_silence, "direct": cmd_direct, "playing": cmd_playing,
               "breaks": cmd_breaks, "p2check": cmd_p2check, "p2watch": cmd_p2watch, "press": cmd_press,
               "selfplay": cmd_selfplay, "hangcheck": cmd_hangcheck}[args.cmd]
    rc = handler(port, args)
    if port.hb_running() and args.cmd not in HEARTBEAT_COMMANDS:
        print(port.hb_stats())
    return rc


if __name__ == "__main__":
    sys.exit(main())
