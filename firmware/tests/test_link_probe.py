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
playing, breaks, soak --heartbeat) run unmodified against the real link
through the same bridge model, extended with the probe's heartbeat path,
the TX gate (the controller's bytes stop while it is released; the RX level
follows the model's "wire plugged" flag), a ready console when the bench
build is modelled, and a scripted user who unplugs and replugs the wire
when the probe prints its prompt. Negative controls prove each command can
fail. The probe's watchdog constants are compared with constants.txt.
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
        self.usb_ready = usb_ready  # the bench build of story 2.7 D17
        self.plugged = True         # the bridge-TX -> controller-GP1 wire
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
        self.plugged = True

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
            if self.plugged:
                lib.sim_rx(chunk, len(chunk), now)
        if not self.ring:
            self.credit_fwd = min(self.credit_fwd, 1.0)
        lib.sim_rx_level(1 if self.plugged else 0, now)
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
             no_heartbeat=False, meter_seconds=1.0, count=5)
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
            port = Bridge2wiCCPort(self.lib)
            rc, text = run_cmd(port, fn)
            self.assertEqual(rc, 2, text)
            self.assertIn("SKIP: console_ready is 0", text)

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


if __name__ == "__main__":
    if len(sys.argv) < 2:
        sys.exit("usage: test_link_probe.py FIRMWARE_DIR [LINK_SIM_LIB]")
    FW_DIR = sys.argv.pop(1)
    if len(sys.argv) >= 2:
        SIM_LIB = sys.argv.pop(1)
    probe = load_probe(FW_DIR)
    unittest.main(verbosity=1)
