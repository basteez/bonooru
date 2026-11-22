# SPDX-License-Identifier: MIT
"""Self-test of tools/link_probe.py (story 2.6 Task 10.2, test F.19).

Run by ctest as `python3 -I test_link_probe.py FIRMWARE_DIR`. Checks the
probe's codec against the shared conformance vectors (cobs, crc16, frames,
messages; parsed exactly as common/proto/vectors/README.md says, every
vector must pass) and its AD-6 sender against an in-process fake link
over a lossy wire (no serial port, no pyserial).
"""

import importlib.util
import os
import random
import sys
import unittest

FW_DIR = None
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

    def __init__(self, cap, now):
        self.cap = cap
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
        if self.queue and random.random() < 0.5:
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


if __name__ == "__main__":
    if len(sys.argv) < 2:
        sys.exit("usage: test_link_probe.py FIRMWARE_DIR")
    FW_DIR = sys.argv.pop(1)
    probe = load_probe(FW_DIR)
    unittest.main(verbosity=1)
