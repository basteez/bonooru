# SPDX-License-Identifier: MIT
"""Self-test of tools/link_probe.py (story 2.6 Task 10.2, tests F.19, F.20).

Run by ctest as `python3 -I test_link_probe.py FIRMWARE_DIR [LINK_SIM_LIB]`.
F.19: the probe's codec against the shared conformance vectors (cobs,
crc16, frames, messages; parsed exactly as common/proto/vectors/README.md
says, every vector must pass) and its AD-6 sender against an in-process
fake link over a lossy wire (no serial port, no pyserial).
F.20 (story 2.6 D30, D31): the probe's real `soak` command on a fake
clock, (a) for longer than its old 30 s budget against the fake link, and
(b) against the real controller link (LINK_SIM_LIB, built from the
firmware sources, loaded with ctypes) through a model of the 2wiCC_Comms
bridge used at the H.5 bench (no flow control, 1 KB USB-to-UART ring that
drops on overflow, UART at 460800 baud both ways).
F.23 (story 2.7 D16): the one-command bench steps (linkup, silence, direct,
playing, breaks, --heartbeat soak) run unmodified against the real link
through the same bridge model, extended with the probe's heartbeat path,
the TX gate (the controller's bytes stop while it is released; the RX level
follows the model's "wire plugged" flag), a USB host that completed the P2
handshake when one is modelled (story 2.3: the console or
`p2_probe.py handshake --hold`), and a scripted user who unplugs and
replugs the wire when the probe prints its prompt. Negative controls prove
each command can fail. The probe's watchdog constants are compared with
constants.txt.
F.32, link_probe.py part (story 2.3 D18, D19): the STATUS v3 decoder, and
`p2check` and `p2watch` unmodified against the real link through the same
bridge model plus a scripted console (plug, sleep, wake, Wired
Communication off and on) that posts to the USB mailbox, the measured
report period and the P2 debug log exactly as the controller's core 0
does; negative controls (no console_ready within the budget, a 100 us
interval, a missing handshake record).
F.19/F.23 style, story 2.5 (Task 10.4): the STATUS v4 decoder by hand, the
selfplay verdict on crafted runs, and `selfplay` and `hangcheck` unmodified
against the real link with the timeline loader of the fw-debug-timeline
flavour (a short 6-event table, and once the real 158-event melody on a
coarser tick) and a console polling every 8000 us; negative controls (a
LATE event, no timeline loaded, a different table, no console_ready within
the budget, a stack mark of 1600 bytes, a wrong boot cause).
"""

import argparse
import ast
import contextlib
import ctypes
import importlib.util
import io
import os
import random
import re
import sys
import unittest

FW_DIR = None
SIM_LIB = None
TMP_DIR = None  # a scratch directory for files a test writes (the build tree)
probe = None


def load_probe(fw_dir):
    sys.dont_write_bytecode = True  # no __pycache__ in the source tree
    path = os.path.join(fw_dir, "tools", "link_probe.py")
    spec = importlib.util.spec_from_file_location("link_probe", path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def parse_vectors(name):
    """{case: {field: value}} from `case.field: value` lines (README rules)."""
    path = os.path.join(FW_DIR, "common", "proto", "vectors", name)
    cases = {}
    first = True
    with open(path, "r", encoding="ascii") as f:
        for line in f:
            line = line.rstrip("\n")
            if not line.strip() or line.lstrip().startswith("#"):
                continue
            key, _, value = line.partition(":")
            key, value = key.strip(), value.strip()
            if first:
                assert key == "proto_version" and int(value, 16) == probe.PROTO_VERSION, name
                first = False
                continue
            case, _, field = key.rpartition(".")
            assert case and field, line
            assert field not in cases.setdefault(case, {}), "duplicate %s" % key
            cases[case][field] = value
    return cases


def hexb(value):
    return bytes.fromhex(value.replace(" ", ""))


def parse_fields(value):
    out = {}
    for item in value.split():
        name, _, v = item.partition("=")
        out[name] = v
    return out


class Vectors(unittest.TestCase):
    def test_cobs(self):
        cases = parse_vectors("cobs.txt")
        self.assertGreater(len(cases), 10)
        for name, c in cases.items():
            enc = hexb(c["enc"])
            if "err" in c:
                self.assertEqual(c["err"], "COBS", name)
                with self.assertRaises(probe.ProtoError, msg=name):
                    probe.cobs_decode(enc)
                continue
            raw = hexb(c["raw"])
            self.assertEqual(probe.cobs_encode(raw), enc, name)
            self.assertEqual(probe.cobs_decode(enc), raw, name)

    def test_crc16(self):
        cases = parse_vectors("crc16.txt")
        self.assertGreater(len(cases), 3)
        for name, c in cases.items():
            crc = probe.crc16(hexb(c["data"]))
            self.assertEqual(bytes([crc & 0xFF, crc >> 8]), hexb(c["crc"]), name)

    def test_frames(self):
        cases = parse_vectors("frames.txt")
        self.assertGreater(len(cases), 10)
        for name, c in cases.items():
            side = c.get("rx", "CONTROLLER")
            wire = hexb(c["wire"])
            if "payload" in c:
                p = hexb(c["payload"])
                self.assertEqual(probe.frame_encode(p[0], p[1], p[2:]), wire + b"\x00", name)
            if "err" in c:
                with self.assertRaises(probe.ProtoError, msg=name) as cm:
                    probe.frame_decode(wire, side)
                self.assertEqual(cm.exception.kind, c["err"], name)
            else:
                ftype, seq, body = probe.frame_decode(wire, side)
                self.assertEqual(bytes([ftype, seq]) + body, hexb(c["payload"]), name)

    def test_messages(self):
        cases = parse_vectors("messages.txt")
        self.assertGreater(len(cases), 20)
        for name, c in cases.items():
            ftype = int(c["type"], 16)
            body = hexb(c["body"])
            if "err" in c:
                self.assertEqual(c["err"], "BODY_LEN", name)
                with self.assertRaises(probe.ProtoError, msg=name):
                    probe.msg_decode(ftype, body)
                continue
            want = parse_fields(c["fields"])
            got = probe.msg_decode(ftype, body)
            fields = {}
            for fname, kind in probe.FIELDS[ftype]:
                v = want[fname]
                fields[fname] = hexb(v) if kind.startswith("b") else int(v, 16)
                self.assertEqual(got[fname], fields[fname], "%s.%s" % (name, fname))
            if ftype == probe.STATUS:
                self.assertEqual(got["extra"], hexb(want.get("extra", "")), name)
                fields["extra"] = got["extra"]
            trailing = hexb(c.get("trailing", ""))
            enc = probe.msg_encode(ftype, fields)
            if ftype == probe.STATUS:
                self.assertEqual(enc, body, name)
            else:
                self.assertEqual(enc + trailing, body, name)
            self.assertEqual(probe.frame_encode(ftype, int(c["seq"], 16), body), hexb(c["wire"]) + b"\x00", name)

    def test_debug_counters(self):
        """STATUS debug counters: v1 prefix (7 bytes) and the v2 split of
        dropped_bytes (21 bytes, story 2.6 D35), little-endian, by hand."""
        self.assertIsNone(probe.debug_counters(bytes([0x10, 0x20, 0x30])))
        v1 = probe.debug_counters(bytes([0xD2, 0x04, 0x00, 0x38, 0x01, 0x02, 0x00]))
        self.assertEqual(v1, {"max_app_gap_ms": 1234, "watchdog_trips": 0, "dropped_bytes": 312, "tx_dropped": 2})
        v2 = probe.debug_counters(bytes([0xD2, 0x04, 0x00, 0x38, 0x01, 0x02, 0x00, 4, 0, 8, 0, 0x2C, 0x01,
                                         2, 0, 1, 0, 1, 0, 1, 0]))
        self.assertEqual(v2["rx_err_bytes"], 4)
        self.assertEqual(v2["rx_overflow_bytes"], 8)
        self.assertEqual(v2["overlong_bytes"], 300)
        self.assertEqual((v2["rx_fe"], v2["rx_pe"], v2["rx_be"], v2["rx_oe"]), (2, 1, 1, 1))
        self.assertEqual(v2["rx_err_bytes"] + v2["rx_overflow_bytes"] + v2["overlong_bytes"], v2["dropped_bytes"])
        self.assertIn("rx_overflow_bytes=8", probe.drop_split(v2))
        # Story 2.3 D14: v3, the P2 log window after the 21 bytes.
        v3 = probe.debug_counters(bytes([0xD2, 0x04, 0x00, 0x38, 0x01, 0x02, 0x00, 4, 0, 8, 0, 0x2C, 0x01,
                                         2, 0, 1, 0, 1, 0, 1, 0, 0x2A, 0x01, 3, 0x07, 0x01, 0x01, 0x04,
                                         0x06, 0x00]))
        self.assertEqual(v3["p2_log_total"], 298)
        self.assertEqual(v3["p2_log"], [(0x07, 0x01), (0x01, 0x04), (0x06, 0x00)])
        self.assertEqual([probe.p2_log_str(k, i) for k, i in v3["p2_log"]],
                         ["restart mount", "usb 80 04", "HANDSHAKE"])
        empty = probe.debug_counters(bytes(21) + bytes([0, 0, 0]))
        self.assertEqual((empty["p2_log_total"], empty["p2_log"]), (0, []))

    def test_debug_v4(self):
        """Story 2.5 D13: the v4 block after the v3 window, at 24 + 2 n, by hand."""
        block = bytes([62, 0, 0x34, 0x12, 0x04, 0x03, 0x02, 0x01, 60, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0xFF, 0xFF,
                       37, 0, 0x9C, 0x01, 0x0E, 0x1F, 0x72, 0x1F, 0x80, 0x3E, 3, 0, 0xFF, 0xFF, 0xFF,
                       0x4C, 0x04, 2, 5])
        self.assertEqual(len(block), 39)
        for n in (0, 1, 16):
            extra = bytes(21) + bytes([n, 0, n]) + bytes(2 * n) + block
            d = probe.debug_counters(extra)
            v = d["v4"]
            self.assertEqual((v["late_n"], v["late_max_us"], v["late_sum_us"]), (62, 0x1234, 0x01020304), n)
            self.assertEqual(v["late_hist"], [60, 1, 0, 0, 0, 65535])
            self.assertEqual((v["loop_max_us"], v["core0_pass_max_us"]), (37, 412))
            self.assertEqual((v["in_min_us"], v["in_p95_us"], v["in_max_us"], v["in_slow"]), (7950, 8050, 16000, 3))
            self.assertEqual((v["seq_retries"], v["seq_fallbacks"], v["stack_used_max"]), (65535, 255, 1100))
            self.assertEqual((v["boot_cause"], v["reboot_count"]), (2, 5))
        self.assertNotIn("v4", probe.debug_counters(bytes(21) + bytes([0, 0, 0]) + block[:38]))
        self.assertIn("boot=CORE1_STALE reboots=5", probe.v4_str(v))
        self.assertNotIn("late_hist", probe.dbg_str(d))

    def test_selfplay_verdict(self):
        """Story 2.5: each limit of a selfplay run fails on its own."""
        good = dict(late_n=158, late_max_us=40, late_sum_us=1000, late_hist=[158, 0, 0, 0, 0, 0], loop_max_us=45,
                    core0_pass_max_us=120, in_min_us=7950, in_p95_us=8050, in_max_us=8100, in_slow=0,
                    seq_retries=0, seq_fallbacks=0, stack_used_max=1100, boot_cause=0, reboot_count=0,
                    events=[0x01], abort_delta=1)
        self.assertIsNone(probe.selfplay_verdict(good, 158, 8000))
        for change, needle in ((dict(late_n=157), "late_n 157"), (dict(late_max_us=9000), "above one report period"),
                               (dict(events=[0x03, 0x01]), "EVENT LATE"), (dict(events=[0x04]), "EVENT UNDERRUN"),
                               (dict(abort_delta=2), "abort_count moved by 2"),
                               (dict(seq_fallbacks=1), "seq_fallbacks 1"),
                               (dict(core0_pass_max_us=5001), "core0_pass_max_us 5001"),
                               (dict(loop_max_us=6000), "loop_max_us 6000"),
                               (dict(stack_used_max=1600), "stack_used_max 1600")):
            bad = dict(good)
            bad.update(change)
            self.assertIn(needle, probe.selfplay_verdict(bad, 158, 8000) or "", change)
        self.assertEqual(probe.read_timeline(os.path.join(FW_DIR, "tests", "testtl_expected.txt")),
                         (62, 158, 44166667))

    def test_p2log_tracker(self):
        """Every record once, in order; a jump beyond the window is a gap."""
        log = probe.P2Log()
        log.feed({"p2_log_total": 2, "p2_log": [(1, 1), (1, 2)]}, 0.0)
        log.feed({"p2_log_total": 2, "p2_log": [(1, 1), (1, 2)]}, 0.05)
        log.feed({"p2_log_total": 4, "p2_log": [(1, 1), (1, 2), (1, 3), (1, 4)]}, 0.10)
        log.feed({"p2_log_total": 24, "p2_log": [(2, i) for i in range(4, 20)]}, 0.15)
        self.assertEqual([i for _, _, i in log.records][:4], [1, 2, 3, 4])
        self.assertEqual(len(log.records), 4 + 16)
        self.assertEqual(log.gaps, 4)
        # Story 2.3: a repeat record counted up in place (total unchanged)
        # refreshes the tracker's last record; a new record after it is added.
        log = probe.P2Log()
        log.feed({"p2_log_total": 4, "p2_log": [(2, 1), (4, 1), (2, 1), (9, 1)]}, 0.0)
        log.feed({"p2_log_total": 4, "p2_log": [(2, 1), (4, 1), (2, 1), (9, 30)]}, 0.05)
        log.feed({"p2_log_total": 5, "p2_log": [(2, 1), (4, 1), (2, 1), (9, 57), (2, 0x30)]}, 0.10)
        self.assertEqual([(k, i) for _, k, i in log.records], [(2, 1), (4, 1), (2, 1), (9, 57), (2, 0x30)])
        self.assertEqual(probe.p2_log_str(9, 57), "(repeated 57 more)")
        self.assertEqual(probe.p2_log_str(8, 4), "arg 04")

    def test_splitter_resync(self):
        sp = probe.Splitter("APP")
        f1 = probe.frame_encode(probe.ACK, 1, b"\x05")
        f2 = probe.frame_encode(probe.ACK, 2, b"\x06")
        frames = sp.feed(b"\x11\x22\x00" + f1 + b"\x00\x00" + f2)
        self.assertEqual([(t, s) for t, s, _ in frames], [(probe.ACK, 1), (probe.ACK, 2)])
        frames = sp.feed(bytes([0x42] * 300) + b"\x00" + f1)
        self.assertEqual(len(frames), 1)
        self.assertEqual(sp.errors.get("OVERLONG"), 1)


class FakeLink:
    """A minimal controller written from docs/link-protocol.md (AD-6, AD-7,
    AD-19) for the sender test: session, seq == expected, stale window 64,
    cumulative ACK, transport NAKs at most once per expected, a queue with
    STATUS credits, and a consumer that frees one slot per tick."""

    def __init__(self, cap, now, consume=True):
        self.cap = cap
        self.consume = consume
        self.now = now
        self.rx = probe.Splitter("CONTROLLER")
        self.expected = 0
        self.session = False
        self.queue = []
        self.enqueued = []
        self.out = []
        self.nak_spent_for = None
        self.last_status = -1.0
        self.ack_pending = False

    def _nak(self, reason, got):
        if self.nak_spent_for == self.expected:
            return
        self.nak_spent_for = self.expected
        self.out.append(probe.frame_encode(probe.NAK, 0, bytes([reason, self.expected, got])))

    def feed(self, data):
        before = sum(self.rx.errors.values())
        for ftype, seq, body in self.rx.feed(data):
            self._frame(ftype, seq, body)
        if sum(self.rx.errors.values()) > before:
            self._nak(0x01, 0)

    def _frame(self, ftype, seq, body):
        if ftype == probe.HELLO:
            self.session = body[:1] == bytes([probe.PROTO_VERSION])
            self.expected = (seq + 1) & 0xFF
            self.queue.clear()
            self.nak_spent_for = None
            info = probe.msg_encode(probe.INFO, {
                "proto_ver": 1, "fw_ver": b"\x00\x00\x01", "bridge_ver": b"\x00\x00\x00", "hid_profile": 2,
                "queue_cap": self.cap, "app_timeout_ms": 1000, "report_period_us": 0})
            self.out.append(probe.frame_encode(probe.INFO, 0, info))
            return
        if ftype == probe.STOP:
            self.expected = (seq + 1) & 0xFF
            self.queue.clear()
            self.nak_spent_for = None
            self.out.append(probe.frame_encode(probe.ACK, 0, bytes([seq])))
            return
        if not self.session:
            self.expected = (seq + 1) & 0xFF
            self.out.append(probe.frame_encode(probe.NAK, 0, bytes([0x10, self.expected, seq])))
            return
        d = (self.expected - seq) & 0xFF
        if d == 0:
            if len(self.queue) >= self.cap:
                self._nak(0x03, seq)
                return
            t = probe.msg_decode(probe.QUEUE_EVENT, body)["t_us"]
            self.queue.append(t)
            self.enqueued.append(t)
            self.expected = (seq + 1) & 0xFF
            self.nak_spent_for = None
            self.ack_pending = True
        elif d <= 64:
            self.ack_pending = True
        else:
            self._nak(0x02, seq)

    def tick(self):
        if self.consume and self.queue and random.random() < 0.5:
            self.queue.pop(0)
        if self.ack_pending:
            self.out.append(probe.frame_encode(probe.ACK, 0, bytes([(self.expected - 1) & 0xFF])))
            self.ack_pending = False
        if self.session and self.now() - self.last_status >= 0.010:
            st = probe.msg_encode(probe.STATUS, {
                "free": self.cap - len(self.queue), "now_us": 0, "last_seq": (self.expected - 1) & 0xFF,
                "mode": 0, "abort_count": 0, "last_event": 0, "flags": 0})
            self.out.append(probe.frame_encode(probe.STATUS, 0, st))
            self.last_status = self.now()
        out, self.out = self.out, []
        return out


class Sender(unittest.TestCase):
    def run_pair(self, seed, cap, n, loss):
        rng = random.Random(seed)
        random.seed(seed)
        clock = [0.0]
        now = lambda: clock[0]  # noqa: E731
        link = FakeLink(cap, now)
        fwd = []

        def lossy(frame):
            if rng.random() < loss:
                return b""
            b = bytearray(frame)
            if rng.random() < loss:
                b[rng.randrange(len(b) - 1)] ^= 1 << rng.randrange(8)
            if rng.random() < loss:
                b = b + b
            return bytes(b)

        events = [(i + 1) * 10 for i in range(n)]
        s = probe.GbnSender(fwd.append, now, events, hello_seq=rng.randrange(256), impair=lossy)
        rx = probe.Splitter("APP")
        for _ in range(200000):
            if s.done() and len(link.enqueued) >= n:
                break
            clock[0] += 0.0005
            s.step()
            for chunk in fwd:
                link.feed(chunk)
            fwd.clear()
            for frame in link.tick():
                for fr in rx.feed(lossy(frame)):
                    s.on_frame(*fr)
            self.assertLessEqual(len(link.queue), cap)
        self.assertTrue(s.done(), "seed %d cap %d loss %.2f: not done (base %d)" % (seed, cap, loss, s.base))
        self.assertEqual(link.enqueued, events, "seed %d: not exactly once and in order" % seed)
        self.assertEqual(s.stats["semantic_naks"], 0)
        return s.stats

    def test_clean(self):
        st = self.run_pair(1, 512, 600, 0.0)
        self.assertEqual(st["resends"], 0)

    def test_lossy(self):
        for seed, cap, loss in ((2, 512, 0.02), (3, 512, 0.10), (4, 5, 0.05), (5, 5, 0.10)):
            self.run_pair(seed, cap, 400, loss)

    def test_hello_until_info(self):
        clock = [0.0]
        sent = []
        s = probe.GbnSender(sent.append, lambda: clock[0], [10], hello_seq=7)
        for _ in range(5):
            s.step()
            clock[0] += 0.05
        hellos = [probe.frame_decode(f[:-1], "CONTROLLER") for f in sent]
        self.assertTrue(all(t == probe.HELLO and q == 7 for t, q, _ in hellos))
        self.assertEqual(len(hellos), 3)  # t = 0, 0.10, 0.20


class FakeClock:
    """The probe's clock in a test: `now()` / `sleep()` on the port, and the
    same functions as the module's `time` (so any direct time call in the
    probe also runs on fake time)."""

    def __init__(self, on_sleep):
        self.t = 1000.0
        self.on_sleep = on_sleep

    def now(self):
        return self.t

    def monotonic(self):
        return self.t

    def sleep(self, dt):
        self.on_sleep(max(dt, 0.0))


def soak_args(**kw):
    a = dict(cycles=1, events=10, seed=1, corrupt=0.0, drop=0.0, garbage=False, no_pace=False,
             baud=probe.DEFAULT_BAUD, port="sim", pace_share=probe.PACE_SHARE, max_inflight=probe.MAX_INFLIGHT)
    a.update(kw)
    return argparse.Namespace(**a)


def run_soak(port, **kw):
    """Runs the probe's own `soak` subcommand; returns (rc, printed text, totals)."""
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf):
        rc = probe.cmd_soak(port, soak_args(**kw))
    text = buf.getvalue()
    m = re.search(r"totals (\{.*\})", text)
    totals = ast.literal_eval(m.group(1)) if m else {}
    return rc, text, totals


class FakePort:
    """FakeLink behind a port on a coarse fake clock (10 ms per sleep)."""

    def __init__(self, cap=512):
        self.clock = FakeClock(self._advance)
        self.link = FakeLink(cap, self.clock.now, consume=False)
        self.rx = probe.Splitter("APP")
        self.inbox = []

    def now(self):
        return self.clock.now()

    def sleep(self, dt):
        self.clock.sleep(dt)

    def _advance(self, dt):
        self.clock.t += max(dt, 0.01)
        self.inbox.extend(self.link.tick())

    def write(self, data):
        self.link.feed(data)

    def poll(self):
        frames = []
        for chunk in self.inbox:
            frames.extend(self.rx.feed(chunk))
        self.inbox = []
        return frames


class Bridge2wiCCPort:
    """The H.5 bench on a fake clock: probe -> USB -> 2wiCC_Comms -> UART ->
    the real controller link (ctypes) -> UART -> 2wiCC_Comms -> USB -> probe.

    2wiCC_Comms v1.1 (read-only source inspection, story 2.6 D30): it reads
    the CDC endpoint 64 bytes at a time in a 10 us loop whatever its ring's
    space, into a 1024-byte ring (1023 usable) that drops bytes silently
    when full, and drains it to UART0 at 460800 baud; no flow control, UART
    FIFO off. Model per 100 us tick: up to 120 bytes from the host into the
    ring (USB full speed), 4.608 bytes from the ring to the controller's RX,
    the controller's TX (128-byte ring, as the firmware's) drained at the
    same rate back to the host (no loss on that path)."""

    TICK = 100e-6
    USB_BYTES_PER_TICK = 120
    RING = 1023
    UART_BYTES_PER_TICK = 46080 * 100e-6
    CTRL_TX_RING = 128

    def __init__(self, lib, drain_share=1.0, usb_ready=False, boot_break=False):
        self.lib = lib
        self.usb_ready = usb_ready  # a host that completed the P2 handshake (story 2.3; was the 2.7 bench build)
        self.plugged = True         # the bridge-TX -> controller-GP1 wire
        self.replug_garbage = b""   # bytes a bouncing contact leaves at the replug (no error flags)
        self.flip_after_replug = False  # corrupt the first byte forwarded after a replug
        self.bounce_after_replug = ()   # seconds after a replug at which the contact bounces (a break)
        self.bounce_low_ticks = 0
        self.bounces = 0
        self.actions = []           # [(time, fn)]: the scripted user
        self.gate_closed_ticks = 0
        probe.HeartbeatService._hb_init(self)
        # The bridge's UART TX can be slower than the line rate (2wiCC_Comms
        # polls uart_putc_raw with the FIFO off between tud_task calls).
        self.fwd_bytes_per_tick = self.UART_BYTES_PER_TICK * drain_share
        self.clock = FakeClock(self._advance)
        self.host_out = bytearray()  # written by the probe, not yet read by the bridge
        self.ring = bytearray()      # the bridge's USB -> UART ring
        self.ctrl_tx = bytearray()   # the controller's TX ring
        self.host_in = bytearray()
        self.credit_fwd = 0.0
        self.credit_rev = 0.0
        self.bridge_drops = 0
        self.rx = probe.Splitter("APP")
        self.buf = ctypes.create_string_buffer(512)
        lib.sim_init(512, self._us())
        if boot_break:
            lib.sim_break(self._us())  # H.4: one BE entry when the UART takes the pin

    # The probe's heartbeat service (story 2.7), as on the real Port.
    def hb_start(self, period_ms=20, dtr=None):
        probe.HeartbeatService.hb_start(self, period_ms, dtr)

    def hb_stop(self):
        return probe.HeartbeatService.hb_stop(self)

    def hb_set_dtr(self, on):
        probe.HeartbeatService.hb_set_dtr(self, on)

    def hb_running(self):
        return probe.HeartbeatService.hb_running(self)

    def hb_stats(self):
        return probe.HeartbeatService.hb_stats(self)

    def hb_frame(self):
        return probe.HeartbeatService.hb_frame(self)

    def hb_emit(self, frame):
        self.write(frame)

    def service(self):
        probe.HeartbeatService.service(self)

    def at(self, t, fn):
        self.actions.append((t, fn))

    def unplug(self):
        """The wire is pulled: the RX pull-down turns the line into a break."""
        self.plugged = False
        self.lib.sim_break(self._us())

    def replug(self):
        """The wire is back. A bouncing contact can leave a few clean data
        bytes with no delimiter (a partial run the next frame lands on), or
        hit the first byte that follows (story 2.7 D40, bench H.12 step 3)."""
        self.plugged = True
        if self.replug_garbage:
            self.lib.sim_rx(self.replug_garbage, len(self.replug_garbage), self._us())
        self.flip_pending = self.flip_after_replug
        for dt in self.bounce_after_replug:
            self.at(self.clock.t + dt, self._bounce)

    def _bounce(self):
        """A contact bounce once the wire is back (story 2.7 D42, bench
        h12d / h12e): the line drops for 300 us, the UART sees a break."""
        if not self.plugged:
            return
        self.bounces += 1
        self.lib.sim_break(self._us())
        self.bounce_low_ticks = 3

    def gate_open(self):
        return self.lib.sim_counter(9) != 0

    def _us(self):
        return ctypes.c_uint64(int(round(self.clock.t * 1e6)))

    def now(self):
        return self.clock.now()

    def sleep(self, dt):
        self.clock.sleep(dt)

    def write(self, data):
        self.host_out += data

    def poll(self):
        self.service()
        data, self.host_in = bytes(self.host_in), bytearray()
        return self.rx.feed(data) if data else []

    def read_raw(self, n):
        return b""

    def _advance(self, dt):
        end = self.clock.t + dt
        while True:
            self._tick()
            self.clock.t += self.TICK
            if self.clock.t >= end:
                break

    def _tick(self):
        lib = self.lib
        now = self._us()
        self.service()
        if self.actions:
            due = [a for a in self.actions if a[0] <= self.clock.t]
            self.actions = [a for a in self.actions if a[0] > self.clock.t]
            for _, fn in due:
                fn()
        if self.usb_ready:
            lib.sim_usb_ready(now)
        console = getattr(self, "console", None)
        if console is not None:
            console.tick(self.clock.t)
        # Host -> bridge ring (drops when full).
        take = self.host_out[:self.USB_BYTES_PER_TICK]
        del self.host_out[:len(take)]
        room = self.RING - len(self.ring)
        self.ring += take[:room]
        self.bridge_drops += max(0, len(take) - room)
        # Bridge ring -> controller UART RX.
        self.credit_fwd += self.fwd_bytes_per_tick
        n = min(int(self.credit_fwd), len(self.ring))
        if n > 0:
            chunk = bytes(self.ring[:n])
            del self.ring[:n]
            self.credit_fwd -= n
            if self.plugged and self.bounce_low_ticks == 0:
                if getattr(self, "flip_pending", False):
                    chunk = bytes([chunk[0] ^ 0x01]) + chunk[1:]
                    self.flip_pending = False
                lib.sim_rx(chunk, len(chunk), now)
        if not self.ring:
            self.credit_fwd = min(self.credit_fwd, 1.0)
        low = not self.plugged or self.bounce_low_ticks > 0
        if self.bounce_low_ticks > 0:
            self.bounce_low_ticks -= 1
        lib.sim_rx_level(0 if low else 1, now)
        lib.sim_poll(now)
        if not self.gate_open():
            # Released: the pad is high-Z, the driver ring is emptied.
            self.ctrl_tx.clear()
            self.gate_closed_ticks += 1
        # Controller TX: whole frames into its ring, drained at the UART rate.
        while True:
            space = self.CTRL_TX_RING - len(self.ctrl_tx)
            k = lib.sim_tx(now, self.buf, space)
            if k == 0:
                break
            self.ctrl_tx += self.buf.raw[:k]
        self.credit_rev += self.UART_BYTES_PER_TICK
        n = min(int(self.credit_rev), len(self.ctrl_tx))
        if n > 0:
            self.host_in += self.ctrl_tx[:n]
            del self.ctrl_tx[:n]
            self.credit_rev -= n
        if not self.ctrl_tx:
            self.credit_rev = min(self.credit_rev, 1.0)
        lib.sim_tx_settled(now, 0 if self.ctrl_tx else 1)


def load_sim():
    lib = ctypes.CDLL(SIM_LIB)
    lib.sim_init.argtypes = [ctypes.c_uint16, ctypes.c_uint64]
    lib.sim_rx.argtypes = [ctypes.c_char_p, ctypes.c_size_t, ctypes.c_uint64]
    lib.sim_poll.argtypes = [ctypes.c_uint64]
    lib.sim_tx.argtypes = [ctypes.c_uint64, ctypes.c_char_p, ctypes.c_size_t]
    lib.sim_tx.restype = ctypes.c_size_t
    lib.sim_counter.argtypes = [ctypes.c_int]
    lib.sim_counter.restype = ctypes.c_uint32
    lib.sim_break.argtypes = [ctypes.c_uint64]
    lib.sim_rx_level.argtypes = [ctypes.c_int, ctypes.c_uint64]
    lib.sim_tx_settled.argtypes = [ctypes.c_uint64, ctypes.c_int]
    lib.sim_usb_ready.argtypes = [ctypes.c_uint64]
    lib.sim_usb_event.argtypes = [ctypes.c_int]
    lib.sim_report_period.argtypes = [ctypes.c_uint32]
    lib.sim_dbglog.argtypes = [ctypes.c_uint8, ctypes.c_uint8]
    lib.sim_dbglog.restype = ctypes.c_int
    lib.sim_timeline.argtypes = [ctypes.c_int]
    lib.sim_in_spread.argtypes = [ctypes.c_uint32] * 4
    lib.sim_core0.argtypes = [ctypes.c_uint32] * 3
    lib.sim_self_timing.argtypes = [ctypes.c_uint32] * 2
    lib.sim_set_boot.argtypes = [ctypes.c_uint8, ctypes.c_uint8]
    return lib


@contextlib.contextmanager
def fake_time(port):
    """Any direct time call inside the probe runs on the port's fake clock."""
    saved = probe.time
    probe.time = port.clock
    try:
        yield
    finally:
        probe.time = saved


class CliGuard(unittest.TestCase):
    """--pace-share and --max-inflight are refused by argparse (exit 2, no
    port opened) when they would make every soak cycle stall (D36)."""

    def bad(self, *argv):
        err = io.StringIO()
        with contextlib.redirect_stderr(err), self.assertRaises(SystemExit) as cm:
            probe.main(["--port", "none"] + list(argv) + ["soak"])
        self.assertEqual(cm.exception.code, 2, err.getvalue())
        return err.getvalue()

    def test_refused(self):
        for v in ("0", "-0.5", "1.01", "nan", "x"):
            self.assertIn("--pace-share", self.bad("--pace-share", v))
        low = probe.min_inflight() - 1
        for v in ("-1", "1", str(low), "x"):
            self.assertIn("--max-inflight", self.bad("--max-inflight", v))

    def test_hb_period_refused(self):
        """--hb-period-ms outside 5..40 is refused before a port opens (story 2.7)."""
        for v in ("4", "41", "100", "x"):
            err = io.StringIO()
            with contextlib.redirect_stderr(err), self.assertRaises(SystemExit) as cm:
                probe.main(["--port", "none", "--hb-period-ms", v, "linkup"])
            self.assertEqual(cm.exception.code, 2, err.getvalue())
            self.assertIn("--hb-period-ms", err.getvalue())
        self.assertEqual(probe.hb_period_arg("5"), 5)
        self.assertEqual(probe.hb_period_arg("40"), 40)

    def test_accepted(self):
        self.assertEqual(probe.pace_share_arg("1"), 1.0)
        self.assertEqual(probe.pace_share_arg("0.85"), 0.85)
        self.assertEqual(probe.max_inflight_arg("0"), 0)
        self.assertEqual(probe.max_inflight_arg(str(probe.min_inflight())), probe.min_inflight())
        self.assertEqual(probe.max_inflight_arg("960"), 960)
        # The minimum is one QUEUE_EVENT frame (19 bytes) plus the largest
        # --garbage prefix (7 junk bytes and a 0x00), by hand.
        self.assertEqual(probe.min_inflight(), 19 + 7 + 1)


class Soak(unittest.TestCase):
    def test_long_soak_keeps_sending(self):
        """F.20a: 150 short cycles take more than 30 s of (fake) time; every
        cycle must still run (H.5 first run: every cycle after 30 s sent 0)."""
        port = FakePort()
        with fake_time(port):
            rc, text, totals = run_soak(port, cycles=150, events=20)
        self.assertGreater(port.now() - 1000.0, 30.0, "the soak did not outlast 30 s")
        self.assertEqual(rc, 0, text[-600:])
        self.assertEqual(totals.get("events"), 150 * 20)

    def test_soak_through_2wicc_bridge(self):
        """F.20b: the real link behind the 2wiCC_Comms model. Clean: PASS, no
        NAK, no byte dropped by the bridge. Impaired (seeds 1 and 2): PASS.
        Unpaced (positive control): the bridge drops bytes and NAKs appear,
        which is what the first H.5 run showed."""
        if SIM_LIB is None:
            self.skipTest("no link simulation library")
        lib = load_sim()
        port = Bridge2wiCCPort(lib)
        with fake_time(port):
            rc, text, totals = run_soak(port, cycles=4, events=500)
        self.assertEqual(rc, 0, text[-800:])
        self.assertEqual(totals.get("naks"), 0, text[-800:])
        self.assertEqual(totals.get("resends"), 0, text[-800:])
        self.assertEqual(port.bridge_drops, 0)
        self.assertEqual(lib.sim_counter(2), 0, "COBS/CRC drops at the controller")

        for seed in (1, 2):
            port = Bridge2wiCCPort(lib)
            with fake_time(port):
                rc, text, totals = run_soak(port, cycles=3, events=500, seed=seed, corrupt=0.02, drop=0.02,
                                            garbage=True)
            self.assertEqual(rc, 0, "seed %d: %s" % (seed, text[-800:]))
            self.assertEqual(port.bridge_drops, 0, "seed %d" % seed)

        port = Bridge2wiCCPort(lib)
        with fake_time(port):
            rc, text, totals = run_soak(port, cycles=2, events=500, no_pace=True, max_inflight=0)
        self.assertGreater(port.bridge_drops, 0, "unpaced: the bridge model never overflowed")
        self.assertGreater(totals.get("naks", 0), 0, "unpaced: no NAK")

    def test_soak_through_slow_bridge(self):
        """F.20c (D33): a bridge that drains at 85 % of the line rate. The
        pacer alone (95 %) lets the backlog grow past the 1 KB ring; the
        ACK-clocked in-flight cap keeps it under, whatever the drain rate:
        clean PASS with 0 NAK and 0 bridge drop, impaired seed 1 PASS with 0
        bridge drop. Positive control: without the cap the bridge drops."""
        if SIM_LIB is None:
            self.skipTest("no link simulation library")
        lib = load_sim()
        port = Bridge2wiCCPort(lib, drain_share=0.85)
        with fake_time(port):
            rc, text, totals = run_soak(port, cycles=4, events=500)
        self.assertEqual(rc, 0, text[-800:])
        self.assertEqual(port.bridge_drops, 0, "85 %% drain: %d bytes dropped by the bridge" % port.bridge_drops)
        self.assertEqual(totals.get("naks"), 0, text[-800:])
        port = Bridge2wiCCPort(lib, drain_share=0.85)
        with fake_time(port):
            rc, text, totals = run_soak(port, cycles=3, events=500, seed=1, corrupt=0.02, drop=0.02, garbage=True)
        self.assertEqual(rc, 0, text[-800:])
        self.assertEqual(port.bridge_drops, 0, "85 %% drain, impaired: %d bytes dropped" % port.bridge_drops)
        port = Bridge2wiCCPort(lib, drain_share=0.85)
        with fake_time(port):
            run_soak(port, cycles=3, events=500, max_inflight=0)
        self.assertGreater(port.bridge_drops, 0, "85 %% drain without the cap: the bridge never overflowed")


# ------------------------------------------------------------ F.23 (2.7)

def cmd_args(**kw):
    """The defaults main() would give the story 2.7 subcommands."""
    a = dict(port="sim", baud=probe.DEFAULT_BAUD, seed=1, no_pace=False, pace_share=probe.PACE_SHARE,
             max_inflight=probe.MAX_INFLIGHT, hb_enabled=True, hb_period_ms=probe.HEARTBEAT_PERIOD_MS,
             seconds=2.0, expect_fresh=False, expect_ready="auto", keepalive_seconds=2.0, rounds=3, mode="idle",
             no_heartbeat=False, meter_seconds=1.0, count=5, trace=None, wait_s=10.0, step_s=10.0, dock=False,
             lead_ms=300, first_input="hat-down", runs=1, timeline=None, expect="core0", reboots_before=None)
    a.update(kw)
    return argparse.Namespace(**a)


class ScriptedUser(io.StringIO):
    """Reads the probe's output as it is printed and acts like the owner:
    unplugs or replugs the wire 200 ms after the prompt and, at the
    multimeter prompt, records whether the controller still drives GP0
    (the TX gate open would read 3.3 V)."""

    def __init__(self, port, replug=True, delay=0.2):
        super().__init__()
        self.port = port
        self.replug = replug
        self.delay = delay
        self.meter_gate_open = []

    def write(self, text):
        if "UNPLUG the bridge-TX wire" in text:
            self.port.at(self.port.now() + self.delay, self.port.unplug)
        elif "REPLUG the wire" in text and self.replug:
            self.port.at(self.port.now() + self.delay, self.port.replug)
        elif "READ the multimeter" in text:
            self.meter_gate_open.append(self.port.gate_open())
        return super().write(text)


def run_cmd(port, fn, user=None, **kw):
    """Runs one probe subcommand unmodified on the fake clock; (rc, text)."""
    buf = user if user is not None else io.StringIO()
    with fake_time(port), contextlib.redirect_stdout(buf):
        rc = fn(port, cmd_args(**kw))
    return rc, buf.getvalue()


def last_line(text):
    lines = [x for x in text.splitlines() if x.strip()]
    return lines[-1] if lines else ""


class DropHeartbeatsPort(Bridge2wiCCPort):
    """Negative control: drops 4 heartbeats in a row once the hold has begun."""

    def __init__(self, lib, start_after):
        super().__init__(lib)
        self.start_after = start_after
        self.dropped = 0

    def hb_emit(self, frame):
        if self.hb_sent >= self.start_after and self.dropped < 4:
            self.dropped += 1
            return
        self.write(frame)


def parse_constants():
    path = os.path.join(FW_DIR, "common", "proto", "vectors", "constants.txt")
    out = {}
    with open(path, "r", encoding="ascii") as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            k, _, v = line.partition(":")
            out[k.strip()] = int(v.strip(), 16)
    return out


class WatchdogConstants(unittest.TestCase):
    def test_probe_constants_match_vectors(self):
        """Story 2.7 D18: one value, one place."""
        c = parse_constants()
        for name in ("HEARTBEAT_PERIOD_MS", "HEARTBEAT_MAX_GAP_MS", "LINK_TIMEOUT_MS", "APP_TIMEOUT_MS_DEFAULT",
                     "APP_PING_DIVISOR"):
            self.assertIn(name, c, name)
            self.assertEqual(getattr(probe, name), c[name], "probe %s differs from constants.txt" % name)
        self.assertEqual(probe.HB_FLAG_DTR, c["HB_FLAG_DTR"])
        self.assertEqual(probe.STATUS_LINK_OK, c["STATUS_LINK_OK"])
        self.assertEqual(probe.STATUS_CONSOLE_READY, c["STATUS_CONSOLE_READY"])
        self.assertEqual(probe.EVT_LINK_LOST, c["EVT_LINK_LOST"])

    def test_heartbeat_schedule(self):
        """Absolute schedule; a late tick sends one heartbeat and re-bases."""
        class P(probe.HeartbeatService):
            def __init__(self):
                self.t = 0.0
                self.out = []

            def now(self):
                return self.t

            def write(self, data):
                self.out.append((self.t, data))

        p = P()
        p.hb_start(20)
        for _ in range(100):
            p.t += 0.001
            p.service()
        self.assertEqual([round(t, 3) for t, _ in p.out[:6]], [0.0, 0.02, 0.04, 0.06, 0.08, 0.1])
        n = len(p.out)
        p.t += 0.095  # a host stall of 95 ms: one heartbeat, no burst
        p.service()
        self.assertEqual(len(p.out), n + 1)
        self.assertGreater(p.hb_max_gap_ms, 90.0)
        self.assertEqual(p.hb_late, 1)
        t_last = p.out[-1][0]
        p.t += 0.019
        p.service()
        self.assertEqual(len(p.out), n + 1, "re-based on now")
        p.t += 0.001
        p.service()
        self.assertEqual(round(p.out[-1][0] - t_last, 3), 0.02)
        ftype, _, body = probe.frame_decode(p.out[-1][1][:-1], "CONTROLLER")
        self.assertEqual(ftype, probe.HEARTBEAT)
        f = probe.msg_decode(probe.HEARTBEAT, body)
        self.assertEqual((f["bridge_fw_ver"], f["flags"]), (bytes([0, 0, 1]), 1))
        p.hb_set_dtr(False)
        self.assertEqual(probe.msg_decode(probe.HEARTBEAT, probe.frame_decode(p.hb_frame()[:-1], "CONTROLLER")[2])
                         ["flags"], 0)
        self.assertEqual(p.hb_stop(), p.out[-1][0])


class ReliableRequest(unittest.TestCase):
    """Story 2.7 D40 review: request_reliable resends at most once per
    OCB_ACK_TIMEOUT_MS, whatever the controller answers, and a NAK CRC
    (got = 0) never names seq 0."""

    class OneNakPort:
        """Answers the first write with one NAK, then stays silent."""

        def __init__(self, reason, got):
            self.t = 0.0
            self.writes = []
            self.pending = []
            self.reason = reason
            self.got = got

        def now(self):
            return self.t

        def sleep(self, dt):
            self.t += dt

        def write(self, data):
            self.writes.append(self.t)
            if len(self.writes) == 1:
                body = bytes([self.reason, 0, self.got])
                self.pending.append(probe.frame_encode(probe.NAK, 0, body))

        def poll(self):
            self.t += 1e-5  # a poll costs time
            out = []
            for w in self.pending:
                out.extend(probe.Splitter("APP").feed(w))
            self.pending = []
            return out

    def run_one(self, reason, seq, got):
        port = self.OneNakPort(reason, got)
        link = probe.Link(port, out=lambda *a: None)
        link.seq = seq
        r = link.request_reliable(probe.STOP, timeout=1.0)
        return r, port.writes

    def test_paced_after_a_nak(self):
        limit = int(1.0 / probe.ACK_TIMEOUT_S) + 2
        for reason, seq, got, name in ((0x02, 5, 5, "NAK SEQ got=seq"), (0x01, 0, 0, "NAK CRC got=0, seq 0"),
                                       (0x03, 9, 9, "NAK FULL got=seq")):
            r, writes = self.run_one(reason, seq, got)
            self.assertIsNotNone(writes)
            self.assertLessEqual(len(writes), limit, "%s: %d writes in 1 s (limit %d)" % (name, len(writes), limit))
            gaps = [b - a for a, b in zip(writes, writes[1:])]
            self.assertTrue(all(g >= probe.ACK_TIMEOUT_S - 1e-6 for g in gaps), "%s: a resend closer than 50 ms" % name)
            self.assertNotEqual(r, "CRC", "%s: a NAK CRC was taken as the reply to seq %d" % (name, seq))


class BenchSteps(unittest.TestCase):
    """F.23: every story 2.7 bench command against the real link."""

    def setUp(self):
        if SIM_LIB is None:
            self.skipTest("no link simulation library")
        self.lib = load_sim()

    def test_linkup(self):
        port = Bridge2wiCCPort(self.lib, boot_break=True)
        rc, text = run_cmd(port, probe.cmd_linkup, expect_fresh=True, expect_ready="no", seconds=3.0)
        self.assertEqual((rc, last_line(text)), (0, "PASS"), text)
        self.assertIn("NO_LINK twice", text)
        self.assertEqual(self.lib.sim_counter(6), 0, "watchdog trips during linkup")
        port = Bridge2wiCCPort(self.lib, usb_ready=True)
        rc, text = run_cmd(port, probe.cmd_linkup, expect_ready="yes", seconds=1.0)
        self.assertEqual((rc, last_line(text)), (0, "PASS"), text)

    def test_linkup_negative_controls(self):
        # A 100 ms heartbeat period through the same path: the controller trips.
        port = Bridge2wiCCPort(self.lib)
        rc, text = run_cmd(port, probe.cmd_linkup, hb_period_ms=100, seconds=2.0)
        self.assertEqual(rc, 1, text)
        self.assertTrue(last_line(text).startswith("FAIL"), text)
        # 4 heartbeats dropped in a row during the hold (a 100 ms gap).
        port = DropHeartbeatsPort(self.lib, start_after=40)
        rc, text = run_cmd(port, probe.cmd_linkup, seconds=2.0)
        self.assertEqual(rc, 1, text)
        self.assertIn("FAIL hold", last_line(text), text)
        # A fresh-boot check against a latched controller fails.
        port = Bridge2wiCCPort(self.lib)
        rc, _ = run_cmd(port, probe.cmd_linkup, seconds=0.5)
        rc, text = run_cmd(port, probe.cmd_linkup, expect_fresh=True, seconds=0.5)
        self.assertEqual(rc, 1, text)

    def test_silence(self):
        port = Bridge2wiCCPort(self.lib)
        rc, text = run_cmd(port, probe.cmd_silence)
        self.assertEqual((rc, last_line(text)), (0, "PASS"), text)
        m = re.search(r"EVENT LINK_LOST ([0-9.]+) ms after the last heartbeat", text)
        self.assertIsNotNone(m, text)
        self.assertTrue(80.0 <= float(m.group(1)) <= 100.0, m.group(0))
        self.assertGreater(port.gate_closed_ticks, 0, "the TX gate was never released")

    def test_silence_negative_control(self):
        """The loss window is checked: a link timeout the probe does not
        expect (as if the firmware tripped at 40 ms) fails the command."""
        saved = probe.LOSS_WINDOW_MS
        probe.LOSS_WINDOW_MS = (100.0, 120.0)
        try:
            port = Bridge2wiCCPort(self.lib)
            rc, text = run_cmd(port, probe.cmd_silence)
        finally:
            probe.LOSS_WINDOW_MS = saved
        self.assertEqual(rc, 1, text)

    def test_direct_and_playing(self):
        for fn in (probe.cmd_direct, probe.cmd_playing):
            port = Bridge2wiCCPort(self.lib, usb_ready=True)
            rc, text = run_cmd(port, fn, seconds=3.0, keepalive_seconds=2.0)
            self.assertEqual((rc, last_line(text)), (0, "PASS"), "%s: %s" % (fn.__name__, text))
            # Story 2.3 D15: no host, no console_ready: FAIL with the hint
            # (the bench-only fake and its SKIP are gone).
            port = Bridge2wiCCPort(self.lib)
            rc, text = run_cmd(port, fn)
            self.assertEqual(rc, 1, text)
            self.assertIn("console_ready is 0", text)
            self.assertIn("p2_probe.py handshake --hold", text)

    def test_press(self):
        """Story 2.3 D35: `press --hat down` with a host holding the
        handshake: every SET_STATE ACKed and the core published hat 4 (the
        state the debug build's report pump now reads); without a host it
        fails with the hint."""
        port = Bridge2wiCCPort(self.lib, usb_ready=True)
        rc, text = run_cmd(port, probe.cmd_press, hat="down", button=None, hold_ms=150, gap_ms=200, count=3)
        self.assertEqual((rc, last_line(text)), (0, "PASS"), text)
        self.assertTrue(self.lib.sim_counter(12) & (1 << 4), "hat 4 never published")
        self.assertEqual(probe.press_state("down", None), bytes([0, 0, 4, 0x80, 0x80, 0x80, 0x80, 0]))
        self.assertEqual(probe.press_state(None, "A"), bytes([4, 0, 8, 0x80, 0x80, 0x80, 0x80, 0]))
        port = Bridge2wiCCPort(self.lib)
        rc, text = run_cmd(port, probe.cmd_press, hat="down", button=None, hold_ms=150, gap_ms=200, count=1)
        self.assertEqual(rc, 1, text)
        self.assertIn("console_ready is 0", text)

    def test_breaks(self):
        for kw in (dict(), dict(no_heartbeat=True), dict(mode="playing")):
            port = Bridge2wiCCPort(self.lib, usb_ready=kw.get("mode") == "playing")
            user = ScriptedUser(port)
            rc, text = run_cmd(port, probe.cmd_breaks, user=user, rounds=3, **kw)
            self.assertEqual((rc, last_line(text)), (0, "PASS"), "%s: %s" % (kw, text))
            self.assertEqual(len(re.findall(r"^round \d: PASS$", text, re.M)), 3, text)
            self.assertEqual(user.meter_gate_open, [False], "%s: GP0 still driven at the meter prompt" % kw)
            if kw.get("no_heartbeat"):
                self.assertEqual(self.lib.sim_counter(7), 0, "unarmed: the core was aborted")

    def test_breaks_unarmed_bouncing_replug(self):
        """Story 2.7 D40 (bench H.12 step 3, rounds 3 to 5 failed): with no
        heartbeat, nothing flushes the controller's receiver after a replug.
        (a) clean garbage bytes left by the contact make the first frame a
        CRC drop; (b) the first byte after the replug is hit. The controller
        answers with one NAK CRC (got = 0) and, by the shared transport-NAK
        budget, nothing to the frames after it: only a resend of the lost
        frame (AD-6) recovers. Every round must still PASS."""
        for garbage, flip in ((b"\x55\xaa\x13", False), (b"", True), (b"\x41" * 7, True)):
            port = Bridge2wiCCPort(self.lib)
            port.replug_garbage = garbage
            port.flip_after_replug = flip
            user = ScriptedUser(port)
            path = os.path.join(TMP_DIR, "breaks-trace.log") if TMP_DIR else None
            rc, text = run_cmd(port, probe.cmd_breaks, user=user, rounds=3, no_heartbeat=True, trace=path)
            self.assertEqual((rc, last_line(text)), (0, "PASS"), "garbage %r flip %s: %s" % (garbage, flip, text))
            self.assertEqual(self.lib.sim_counter(0), 15, "15 events must be queued exactly once")
            if path and not flip:
                # The delimiter written after each replug ends the partial run,
                # so no resend is even needed (the resend is the second guard).
                with open(path, "r", encoding="ascii") as f:
                    trace = f.read()
                # One right after the replug, one after the line settled (D42).
                self.assertEqual(trace.count("0x00 (delimiter)"), 6, "no delimiter after a replug")
                self.assertNotIn("(resend)", trace, "the garbage run cost a frame: delimiter missing?")
        # The armed case through the same bouncing contact.
        port = Bridge2wiCCPort(self.lib)
        port.replug_garbage = b"\x55\xaa\x13"
        port.flip_after_replug = True
        rc, text = run_cmd(port, probe.cmd_breaks, user=ScriptedUser(port), rounds=2)
        self.assertEqual((rc, last_line(text)), (0, "PASS"), text)

    def test_trace(self):
        """--trace: every frame sent and received, with a timestamp; breaks
        prints the tail of it when it fails."""
        port = Bridge2wiCCPort(self.lib)
        path = os.path.join(TMP_DIR, "trace.log") if TMP_DIR else None
        if path is None:
            self.skipTest("no scratch directory")
        rc, text = run_cmd(port, probe.cmd_silence, trace=path)
        self.assertEqual(rc, 0, text)
        with open(path, "r", encoding="ascii") as f:
            lines = f.read().splitlines()
        self.assertTrue(any(" TX QUEUE_EVENT seq=" in x for x in lines), lines[:5])
        self.assertTrue(any(" RX EVENT LINK_LOST" in x for x in lines), lines[:5])
        port = Bridge2wiCCPort(self.lib)
        saved = probe.BREAKS_REPLUG_TIMEOUT_S
        probe.BREAKS_REPLUG_TIMEOUT_S = 1.0
        try:
            rc, text = run_cmd(port, probe.cmd_breaks, user=ScriptedUser(port, replug=False), rounds=1,
                               no_heartbeat=True)
        finally:
            probe.BREAKS_REPLUG_TIMEOUT_S = saved
        self.assertEqual(rc, 1)
        self.assertIn("trace (last", text)
        self.assertIn(" TX HELLO seq=", text)

    def test_breaks_bounce_after_replug(self):
        """Story 2.7 D42 (bench h12d, h12e): the contact bounces 50 to 150 ms
        after the replug (and twice more, up to 1.25 s). The bounce must be absorbed while the line settles
        and never be taken as the next round's unplug, armed and unarmed;
        the last round settles too. Every round PASS, each bounce absorbed."""
        for kw in (dict(), dict(no_heartbeat=True)):
            port = Bridge2wiCCPort(self.lib)
            # A burst 50 to 150 ms after the reconnect (the bench pattern), then
            # two late bounces, so a settle window that ignores a break ends
            # too early.
            port.bounce_after_replug = (0.05, 0.09, 0.15, 0.7, 1.25)
            user = ScriptedUser(port)
            rc, text = run_cmd(port, probe.cmd_breaks, user=user, rounds=3, **kw)
            self.assertEqual((rc, last_line(text)), (0, "PASS"), "%s: %s" % (kw, text))
            self.assertEqual(len(re.findall(r"^round \d: PASS$", text, re.M)), 3, text)
            self.assertEqual(port.bounces, 15, "%s: the model did not bounce 5 times per round" % kw)
            self.assertEqual(text.count("UNPLUG the bridge-TX wire"), 3, kw)
            self.assertEqual(len(re.findall(r"round \d: line settled", text)), 3, text)
            if kw.get("no_heartbeat"):
                self.assertEqual(self.lib.sim_counter(7), 0, "unarmed: the core was aborted")
                self.assertEqual(self.lib.sim_counter(0), 15, "unarmed: 15 events queued exactly once")

    def test_breaks_line_never_settles(self):
        """A contact that keeps bouncing: breaks fails with its settle message."""
        saved = probe.BREAKS_SETTLE_TIMEOUT_S
        probe.BREAKS_SETTLE_TIMEOUT_S = 2.0
        try:
            port = Bridge2wiCCPort(self.lib)
            port.bounce_after_replug = tuple(0.05 + 0.3 * k for k in range(20))
            rc, text = run_cmd(port, probe.cmd_breaks, user=ScriptedUser(port), rounds=1)
        finally:
            probe.BREAKS_SETTLE_TIMEOUT_S = saved
        self.assertEqual(rc, 1, text)
        self.assertIn("the line did not settle", text)

    def test_breaks_never_replugged(self):
        saved = probe.BREAKS_REPLUG_TIMEOUT_S
        probe.BREAKS_REPLUG_TIMEOUT_S = 2.0
        try:
            port = Bridge2wiCCPort(self.lib)
            user = ScriptedUser(port, replug=False)
            rc, text = run_cmd(port, probe.cmd_breaks, user=user, rounds=2)
        finally:
            probe.BREAKS_REPLUG_TIMEOUT_S = saved
        self.assertEqual(rc, 1, text)
        self.assertIn("STATUS did not resume with link_ok 1 within 2 s of the REPLUG prompt", text)

    def test_soak_with_heartbeat(self):
        port = Bridge2wiCCPort(self.lib)
        with fake_time(port):
            rc, text, totals = run_soak(port, cycles=5, events=100, hb_enabled=True)
        self.assertEqual(rc, 0, text[-800:])
        self.assertIn("watchdog_trips 0 -> 0", text)
        self.assertEqual(self.lib.sim_counter(6), 0)
        self.assertEqual(port.bridge_drops, 0)


class ConsoleModel:
    """A scripted Switch 2 at the controller's USB port (story 2.3 F.32): it
    posts to the USB mailbox, the measured period and the P2 debug log the
    way platform/usb_p2.c does for the real console. `sequence` is the
    handshake it logs at each connect; `period_us` what core 0 measures."""

    def __init__(self, lib, sequence=None, period_us=1000, in_every_s=0.001):
        self.lib = lib
        self.plugged = False
        self.awake = True
        self.wired = True
        self.connected = False
        self.sequence = sequence if sequence is not None else [
            (0x01, 0x05), (0x01, 0x01), (0x01, 0x02), (0x02, 0x03), (0x01, 0x04), (0x06, 0x00),
            (0x02, 0x48), (0x02, 0x02), (0x02, 0x10), (0x02, 0x01), (0x08, 0x04), (0x02, 0x21), (0x08, 0x21),
            (0x02, 0x30)]
        self.period_us = period_us
        self.in_every_s = in_every_s
        self.next_in = 0.0
        self.n_in = 0
        self.handshake_post = True

    def tick(self, t):
        lib = self.lib
        up = self.plugged and self.awake
        if up and not self.connected:
            self.connected = True
            lib.sim_dbglog(0x07, 0x01)  # restart: mount
            for k, i in self.sequence:
                lib.sim_dbglog(k, i)
            if self.handshake_post and any(k == 0x06 for k, _ in self.sequence):
                lib.sim_usb_event(0)
            self.n_in = 0
            self.next_in = t
        elif not up and self.connected:
            self.connected = False
            lib.sim_usb_event(2)       # suspend
            lib.sim_dbglog(0x07, 0x03)
            lib.sim_report_period(0)
        if self.connected and self.wired and t >= self.next_in:
            self.next_in = t + self.in_every_s
            lib.sim_usb_event(1)
            self.n_in += 1
            if self.n_in == 64:
                lib.sim_report_period(self.period_us)


class ConsoleUser(io.StringIO):
    """The owner at the console: acts 0.5 s after each prompt."""

    def __init__(self, port, console, delay=0.5):
        super().__init__()
        self.port = port
        self.console = console
        self.delay = delay

    def _later(self, fn):
        self.port.at(self.port.now() + self.delay, fn)

    def write(self, text):
        c = self.console
        if "PLUG THE CONTROLLER INTO THE SWITCH" in text:
            self._later(lambda: setattr(c, "plugged", True))
        elif "PUT THE CONSOLE TO SLEEP" in text or "UNDOCK" in text:
            self._later(lambda: setattr(c, "awake", False))
        elif "WAKE THE CONSOLE" in text or "DOCK THE CONSOLE AGAIN" in text:
            self._later(lambda: setattr(c, "awake", True))
        elif "Wired Communication' OFF" in text:
            self._later(lambda: setattr(c, "wired", False))
        elif "Wired Communication' back ON" in text:
            self._later(lambda: setattr(c, "wired", True))
        return super().write(text)


class P2BenchSteps(unittest.TestCase):
    """F.32 (link_probe.py part): p2check and p2watch against the real link."""

    def setUp(self):
        if SIM_LIB is None:
            self.skipTest("no link simulation library")
        self.lib = load_sim()

    def port_with_console(self, **kw):
        port = Bridge2wiCCPort(self.lib)
        port.console = ConsoleModel(self.lib, **kw)
        return port, port.console

    def test_p2check(self):
        port, console = self.port_with_console()
        rc, text = run_cmd(port, probe.cmd_p2check, user=ConsoleUser(port, console), wait_s=10.0)
        self.assertEqual((rc, last_line(text)), (0, "PASS"), text)
        self.assertIn("H.15 median IN interval: 1000 us", text)
        self.assertIn("usb 80 05, usb 80 01, usb 80 02, sub 03, usb 80 04, HANDSHAKE", text)
        self.assertIn("sub 01, arg 04, sub 21, arg 21, sub 30", text)
        self.assertNotIn("refused by the profile", text)

    def test_p2check_negative_controls(self):
        # The console never answers (setting OFF before the plug): no ready.
        port, console = self.port_with_console()
        console.wired = False
        console.handshake_post = False
        rc, text = run_cmd(port, probe.cmd_p2check, user=ConsoleUser(port, console), wait_s=3.0)
        self.assertEqual(rc, 1, text)
        self.assertIn("FAIL console_ready 1 within", last_line(text))
        # A 100 us interval is outside 500..20000.
        port, console = self.port_with_console(period_us=100)
        rc, text = run_cmd(port, probe.cmd_p2check, user=ConsoleUser(port, console), wait_s=10.0)
        self.assertEqual(rc, 1, text)
        self.assertIn("report_period_us 100", last_line(text))
        # No HANDSHAKE record in the log (ready from another path).
        port, console = self.port_with_console(sequence=[(0x01, 0x01), (0x01, 0x02)])
        console.handshake_post = True
        console.sequence_post = True
        orig = console.tick

        def tick(t):
            was = console.connected
            orig(t)
            if console.connected and not was:
                self.lib.sim_usb_event(0)  # ready without the profile logging it

        console.tick = tick
        rc, text = run_cmd(port, probe.cmd_p2check, user=ConsoleUser(port, console), wait_s=10.0)
        self.assertEqual(rc, 1, text)
        self.assertIn("HANDSHAKE record", last_line(text))

    def test_p2watch(self):
        port, console = self.port_with_console()
        console.plugged = True
        rc, text = run_cmd(port, probe.cmd_p2watch, user=ConsoleUser(port, console), step_s=10.0)
        self.assertEqual((rc, last_line(text)), (0, "PASS"), text)
        self.assertIn("EVENT USB_SUSPENDED", text)
        self.assertIn("a restart, then a new handshake", text)
        port, console = self.port_with_console()
        console.plugged = True
        rc, text = run_cmd(port, probe.cmd_p2watch, user=ConsoleUser(port, console), step_s=10.0, dock=True)
        self.assertEqual((rc, last_line(text)), (0, "PASS"), text)

    def test_p2watch_negative_control(self):
        # The console never comes back after the sleep.
        port, console = self.port_with_console()
        console.plugged = True
        user = ConsoleUser(port, console)
        orig = user.write

        def write(text):
            if "WAKE THE CONSOLE" in text:
                return io.StringIO.write(user, text)
            return orig(text)

        user.write = write
        rc, text = run_cmd(port, probe.cmd_p2watch, user=user, step_s=3.0)
        self.assertEqual(rc, 1, text)
        self.assertIn("console_ready 1 again", last_line(text))
        # Not ready at the start: p2watch refuses to run the cycle.
        port, console = self.port_with_console()
        rc, text = run_cmd(port, probe.cmd_p2watch, user=ConsoleUser(port, console), step_s=3.0)
        self.assertEqual(rc, 1, text)
        self.assertIn("run p2check first", last_line(text))


# ------------------------------------------------------- story 2.5 (F.23)

SHORT_EXPECTED = ("# test table of link_sim_shim.c (k_short_table)\n"
                  "# Expected notes: 2 notes in 6 events, the last event at 500000 us.\n"
                  "01 B5 6 100000\n02 D6 18 300000\n")


class Switch2Console(ConsoleModel):
    """The console at the controller's port for selfplay: polls every
    8000 us, and core 0 posts the IN spread and its own numbers (story 2.5
    D12) when the period becomes known."""

    def __init__(self, lib, spread=(7950, 8000, 8100, 0), core0=(120, 0, 0), **kw):
        kw.setdefault("period_us", 8000)
        kw.setdefault("in_every_s", 0.008)
        super().__init__(lib, **kw)
        self.spread = spread
        self.core0 = core0

    def tick(self, t):
        n0 = self.n_in
        super().tick(t)
        if self.n_in == 64 and n0 != 64:
            self.lib.sim_in_spread(*self.spread)
            self.lib.sim_core0(*self.core0)


class StallPort(Bridge2wiCCPort):
    """Negative control: the controller's loop stalls 20 ms (no poll, so no
    scheduler pass) 90 ms into the lead of a 300 ms START: the event due at
    +100 ms is applied 10 ms late, more than one 8000 us period: LATE."""

    def _tick(self):
        if self.lib.sim_counter(10) == 2 and getattr(self, "playing_since", None) is None:
            self.playing_since = self.clock.t
        since = getattr(self, "playing_since", None)
        if since is not None and 0.390 <= self.clock.t - since < 0.410:
            saved = self.lib.sim_poll
            self.lib.sim_poll = lambda now: None
            try:
                super()._tick()
            finally:
                self.lib.sim_poll = saved
            return
        super()._tick()


class CoarsePort(Bridge2wiCCPort):
    """The same bench model on a 1 ms tick (for the 44 s melody)."""

    TICK = 1e-3
    USB_BYTES_PER_TICK = 1200
    UART_BYTES_PER_TICK = 46080 * 1e-3


class SelfPlay(unittest.TestCase):
    """Story 2.5 F.23 style: selfplay and hangcheck against the real link."""

    def setUp(self):
        if SIM_LIB is None:
            self.skipTest("no link simulation library")
        self.lib = load_sim()
        self.expected = os.path.join(TMP_DIR or ".", "short_testtl_expected.txt")
        with open(self.expected, "w", encoding="utf-8") as f:
            f.write(SHORT_EXPECTED)

    def port(self, timeline=2, stack=900, cls=Bridge2wiCCPort, **kw):
        port = cls(self.lib)
        port.console = Switch2Console(self.lib, **kw)
        self.lib.sim_timeline(timeline)
        self.lib.sim_self_timing(30, stack)
        return port, port.console

    def test_selfplay_short_table(self):
        port, console = self.port()
        rc, text = run_cmd(port, probe.cmd_selfplay, user=ConsoleUser(port, console), timeline=self.expected,
                           runs=2)
        self.assertEqual((rc, last_line(text)), (0, "PASS"), text)
        self.assertIn("run 1: STATUS free 506 = queue_cap 512 - 6 table events", text)
        self.assertIn("run 2: late_n = 6", text)
        self.assertIn("T0_HOST_MONO=", text)
        self.assertIn("PASTE_LINE: run 2 | late max", text)
        self.assertIn("IN median 8000, min 7950, p95 8000, max 8100 us, slow 0", text)
        self.assertIn("stack 900 B", text)
        self.assertIn("first input: D-pad down x3 in DIRECT (6 of 6", text)
        # The host's estimate of t0 is close to the controller's (fake clock).
        t0s = [float(x) for x in re.findall(r"T0_HOST_MONO=([0-9.]+)", text)]
        self.assertEqual(len(t0s), 2)

    def test_selfplay_real_melody(self):
        port, console = self.port(timeline=1, cls=CoarsePort)
        rc, text = run_cmd(port, probe.cmd_selfplay, user=ConsoleUser(port, console),
                           timeline=os.path.join(FW_DIR, "tests", "testtl_expected.txt"), lead_ms=1000,
                           first_input="none")
        self.assertEqual((rc, last_line(text)), (0, "PASS"), text)
        self.assertIn("STATUS free 354 = queue_cap 512 - 158 table events", text)
        self.assertIn("late_n = 158", text)

    def test_selfplay_negative_controls(self):
        # A LATE event: the controller's loop stalls 20 ms during the piece.
        port, console = self.port(cls=StallPort)
        rc, text = run_cmd(port, probe.cmd_selfplay, user=ConsoleUser(port, console), timeline=self.expected)
        self.assertEqual(rc, 1, text)
        self.assertIn("late_n 1, the table has 6 events", last_line(text))
        self.assertIn("events LATE", text)
        # Not the timeline flavour: the queue stays empty after STOP.
        port, console = self.port(timeline=0)
        rc, text = run_cmd(port, probe.cmd_selfplay, user=ConsoleUser(port, console), timeline=self.expected)
        self.assertEqual(rc, 1, text)
        self.assertIn("not a fw-debug-timeline build", last_line(text))
        # A different table than the expected file says.
        port, console = self.port(timeline=1)
        rc, text = run_cmd(port, probe.cmd_selfplay, user=ConsoleUser(port, console), timeline=self.expected)
        self.assertEqual(rc, 1, text)
        self.assertIn("STATUS free 354 = queue_cap 512 - 6", last_line(text))
        # No console_ready within the budget (never plugged).
        port, console = self.port()
        rc, text = run_cmd(port, probe.cmd_selfplay, timeline=self.expected, wait_s=3.0)
        self.assertEqual(rc, 1, text)
        self.assertIn("console_ready 1 for 2 s in a row", last_line(text))
        # A stack mark of 1600 bytes.
        port, console = self.port(stack=1600)
        rc, text = run_cmd(port, probe.cmd_selfplay, user=ConsoleUser(port, console), timeline=self.expected)
        self.assertEqual(rc, 1, text)
        self.assertIn("stack_used_max 1600 B above 1536 B", last_line(text))

    def test_hangcheck(self):
        port = Bridge2wiCCPort(self.lib)
        self.lib.sim_set_boot(1, 3)
        rc, text = run_cmd(port, probe.cmd_hangcheck, expect="core0")
        self.assertEqual((rc, last_line(text)), (0, "PASS"), text)
        self.assertIn("boot_cause CORE0_STALE (want CORE0_STALE)", text)
        rc, text = run_cmd(port, probe.cmd_hangcheck, expect="core0", reboots_before=2)
        self.assertEqual((rc, last_line(text)), (0, "PASS"), text)
        # Negative controls: the wrong cause, the wrong count, a power-on boot.
        port = Bridge2wiCCPort(self.lib)
        self.lib.sim_set_boot(1, 1)
        rc, text = run_cmd(port, probe.cmd_hangcheck, expect="core1")
        self.assertEqual(rc, 1, text)
        self.assertIn("boot_cause CORE0_STALE (want CORE1_STALE)", last_line(text))
        rc, text = run_cmd(port, probe.cmd_hangcheck, expect="core0", reboots_before=4)
        self.assertEqual(rc, 1, text)
        self.assertIn("reboot_count 1 (want 4 + 1)", last_line(text))
        port = Bridge2wiCCPort(self.lib)
        rc, text = run_cmd(port, probe.cmd_hangcheck, expect="core1")
        self.assertEqual(rc, 1, text)
        self.assertIn("boot_cause NONE", last_line(text))


if __name__ == "__main__":
    if len(sys.argv) < 2:
        sys.exit("usage: test_link_probe.py FIRMWARE_DIR [LINK_SIM_LIB]")
    FW_DIR = sys.argv.pop(1)
    if len(sys.argv) >= 2:
        SIM_LIB = sys.argv.pop(1)
    if len(sys.argv) >= 2:
        TMP_DIR = sys.argv.pop(1)
    probe = load_probe(FW_DIR)
    unittest.main(verbosity=1)
