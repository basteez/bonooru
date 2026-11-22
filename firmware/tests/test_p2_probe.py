# SPDX-License-Identifier: MIT
"""Self-test of tools/p2_probe.py (story 2.3 Task 9.3, test F.32, p2_probe
part; the link_probe.py part is in test_link_probe.py).

Run by ctest as `python3 -I test_p2_probe.py FIRMWARE_DIR P2_SIM_LIB`.
 - the probe's own expected bytes (the 203-byte report descriptor, the SPI
   regions) against hand-written values from the documents;
 - its `enum` parsers and command on fixtures in tests/fixtures/ shaped as
   macOS prints them (SPUSBHostDataType, SPUSBDataType, ioreg), including a
   plug bounce where the device vanishes once, on a fake clock;
 - its `handshake` command against the real ocb_p2 (P2_SIM_LIB, built from
   the firmware sources, loaded with ctypes) through a fake HID transport on
   a fake clock: a clean run, a lossy one that drops one reply (the resend
   path), and negative controls (a reply with one wrong byte, a refusal
   switched off) that must make it print FAIL.
"""

import argparse
import contextlib
import ctypes
import importlib.util
import io
import json
import os
import sys
import unittest

FW_DIR = None
SIM_LIB = None
p2 = None


def load_probe(fw_dir):
    sys.dont_write_bytecode = True
    path = os.path.join(fw_dir, "tools", "p2_probe.py")
    spec = importlib.util.spec_from_file_location("p2_probe", path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def fixture(name):
    with open(os.path.join(FW_DIR, "tests", "fixtures", name), encoding="utf-8") as f:
        return f.read()


class FakeClock:
    def __init__(self, on_sleep=None):
        self.t = 1000.0
        self.on_sleep = on_sleep

    def monotonic(self):
        return self.t

    def sleep(self, dt):
        if self.on_sleep:
            self.on_sleep(dt)
        self.t += dt


@contextlib.contextmanager
def fake_time(clock):
    saved = p2.time
    p2.time = clock
    try:
        yield
    finally:
        p2.time = saved


def run(fn, *a, **kw):
    out = io.StringIO()
    rc = fn(*a, out=lambda s: out.write(s + "\n"), **kw)
    return rc, out.getvalue()


def last_line(text):
    lines = [x for x in text.splitlines() if x.strip()]
    return lines[-1] if lines else ""


# S7 usbhid-dump, written again here as the independent reference.
S7_HEX = ("05 01 15 00 09 04 A1 01 85 30 05 01 05 09 19 01 29 0A 15 00 25 01 75 01 95 0A 55 00 65 00 81 02 "
          "05 09 19 0B 29 0E 15 00 25 01 75 01 95 04 81 02 75 01 95 02 81 03 0B 01 00 01 00 A1 00 0B 30 00 "
          "01 00 0B 31 00 01 00 0B 32 00 01 00 0B 35 00 01 00 15 00 27 FF FF 00 00 75 10 95 04 81 02 C0 0B "
          "39 00 01 00 15 00 25 07 35 00 46 3B 01 65 14 75 04 95 01 81 02 05 09 19 0F 29 12 15 00 25 01 75 "
          "01 95 04 81 02 75 08 95 34 81 03 06 00 FF 85 21 09 01 75 08 95 3F 81 03 85 81 09 02 75 08 95 3F "
          "81 03 85 01 09 03 75 08 95 3F 91 83 85 10 09 04 75 08 95 3F 91 83 85 80 09 05 75 08 95 3F 91 83 "
          "85 82 09 06 75 08 95 3F 91 83 C0")


class Constants(unittest.TestCase):
    def test_report_descriptor(self):
        self.assertEqual(len(p2.REPORT_DESC), 203)
        self.assertEqual(p2.REPORT_DESC, bytes.fromhex(S7_HEX))
        if SIM_LIB:
            lib = load_sim()
            buf = ctypes.create_string_buffer(256)
            n = lib.p2sim_report_desc(buf, 256)
            self.assertEqual(buf.raw[:n], p2.REPORT_DESC, "firmware and probe disagree")

    def test_identity(self):
        self.assertEqual((p2.VID, p2.PID, p2.BCD_DEVICE), (0x057E, 0x2009, 0x0210))
        self.assertEqual((p2.MANUFACTURER, p2.PRODUCT, p2.SERIAL),
                         ("Nintendo Co., Ltd.", "Pro Controller", "000000000001"))

    def test_spi_expectations(self):
        # S9 calibration: centre 0x800, range 0x700, S4 field orders.
        self.assertEqual(p2.SPI_EXPECT[(0x603D, 18)][:9].hex(), "000770000880000770")
        self.assertEqual(p2.SPI_EXPECT[(0x603D, 18)][9:].hex(), "000880000770000770")
        self.assertEqual(p2.SPI_EXPECT[(0x8010, 22)], b"\xff" * 22)


class FakeUsb:
    """system_profiler / ioreg answers from fixtures; `present` is a list of
    booleans, one per profiler call (the plug bounce), the last repeats."""

    def __init__(self, profiler_doc, ioreg_usb="", ioreg_hid="", present=None):
        self.doc = json.loads(profiler_doc)
        self.empty = json.loads(fixture("sp_usbhost_empty.json"))
        self.usb_text = ioreg_usb
        self.hid_text = ioreg_hid
        self.present = list(present) if present else [True]
        self.calls = 0

    def _on(self):
        i = min(self.calls, len(self.present) - 1)
        return self.present[i]

    def profiler(self):
        on = self._on()
        self.calls += 1
        return p2.profiler_devices(self.doc if on else self.empty)

    def ioreg_usb(self):
        return p2.ioreg_devices(self.usb_text) if self._on() else []

    def ioreg_hid(self):
        return self.hid_text


class Enum(unittest.TestCase):
    def test_parsers(self):
        for name in ("sp_usbhost_p2.json", "sp_usb_p2.json"):
            devs = [d for d in p2.profiler_devices(json.loads(fixture(name))) if d.get("vid") == 0x057E]
            self.assertEqual(len(devs), 1, name)
            d = devs[0]
            self.assertEqual((d["vid"], d["pid"], d["product"], d["manufacturer"], d["serial"], d["bcd"]),
                             (0x057E, 0x2009, "Pro Controller", "Nintendo Co., Ltd.", "000000000001", 0x0210), name)
        self.assertEqual(p2.profiler_devices(json.loads(fixture("sp_usbhost_empty.json"))), [])
        d = p2.ioreg_devices(fixture("ioreg_usb_p2.txt"))
        self.assertEqual(len(d), 1)
        self.assertEqual((d[0]["vid"], d[0]["pid"], d[0]["bcd"], d[0]["product"], d[0]["serial"]),
                         (0x057E, 0x2009, 0x0210, "Pro Controller", "000000000001"))
        self.assertEqual(p2.ioreg_report_descriptor(fixture("ioreg_hid_p2.txt")), bytes.fromhex(S7_HEX))

    def enum(self, usb, wait_s=20.0):
        with fake_time(FakeClock()):
            return run(p2.cmd_enum, argparse.Namespace(wait_s=wait_s), usb=usb)

    def test_enum_pass(self):
        for name in ("sp_usbhost_p2.json", "sp_usb_p2.json"):
            rc, text = self.enum(FakeUsb(fixture(name), fixture("ioreg_usb_p2.txt"), fixture("ioreg_hid_p2.txt")))
            self.assertEqual((rc, last_line(text)), (0, "PASS"), text)
            self.assertIn("equal to the 203 expected", text)
        # system_profiler only (no ioreg data): still PASS, the descriptor SKIPped.
        rc, text = self.enum(FakeUsb(fixture("sp_usb_p2.json")))
        self.assertEqual((rc, last_line(text)), (0, "PASS"), text)
        self.assertIn("SKIP HID report descriptor", text)

    def test_enum_bounce(self):
        # Plugged, gone, back (a bouncing plug): the stable window restarts.
        usb = FakeUsb(fixture("sp_usbhost_p2.json"), fixture("ioreg_usb_p2.txt"),
                      present=[False, True, True, False, True, True, True])
        rc, text = self.enum(usb)
        self.assertEqual((rc, last_line(text)), (0, "PASS"), text)
        self.assertGreaterEqual(usb.calls, 6)

    def test_enum_negative_controls(self):
        rc, text = self.enum(FakeUsb(fixture("sp_usbhost_empty.json")), wait_s=5.0)
        self.assertEqual(rc, 1, text)
        self.assertIn("FAIL 057E:2009 seen stable", last_line(text))
        bad = fixture("ioreg_usb_p2.txt").replace('"USB Serial Number" = "000000000001"',
                                                   '"USB Serial Number" = "000000000002"')
        rc, text = self.enum(FakeUsb(fixture("sp_usb_p2.json"), bad))
        self.assertEqual(rc, 1, text)
        self.assertIn("serial", last_line(text))
        badrd = fixture("ioreg_hid_p2.txt").replace("8582090675", "8583090675")
        rc, text = self.enum(FakeUsb(fixture("sp_usb_p2.json"), fixture("ioreg_usb_p2.txt"), badrd))
        self.assertEqual(rc, 1, text)
        self.assertIn("HID report descriptor", last_line(text))


def load_sim():
    lib = ctypes.CDLL(SIM_LIB)
    lib.p2sim_init.argtypes = [ctypes.c_char_p]
    lib.p2sim_out.argtypes = [ctypes.c_char_p, ctypes.c_size_t]
    lib.p2sim_in.argtypes = [ctypes.c_char_p, ctypes.c_size_t]
    lib.p2sim_in.restype = ctypes.c_size_t
    lib.p2sim_refusals.restype = ctypes.c_uint32
    lib.p2sim_report_desc.argtypes = [ctypes.c_char_p, ctypes.c_size_t]
    lib.p2sim_report_desc.restype = ctypes.c_size_t
    lib.p2sim_set_state.argtypes = [ctypes.c_char_p]
    return lib


class FakeHid:
    """The controller's HID endpoints on a fake clock: the host polls the IN
    endpoint every 1 ms (bInterval 1); writes go to ocb_p2_on_output.
    `drop` drops the first IN report matching it (a lost reply); `mutate`
    returns a changed report (a wrong byte) or None."""

    def __init__(self, lib, clock, drop=None, mutate=None):
        self.lib = lib
        self.clock = clock
        self.drop = drop
        self.mutate = mutate
        self.buf = ctypes.create_string_buffer(64)
        self.dropped = 0
        lib.p2sim_init(bytes([0x02, 0x11, 0x22, 0x33, 0x44, 0x55]))

    def write(self, data):
        data = p2.pad(data)
        self.lib.p2sim_out(data, len(data))

    def read(self, timeout_ms):
        end = self.clock.t + timeout_ms / 1000.0
        while True:
            self.clock.t += 0.001
            n = self.lib.p2sim_in(self.buf, 64)
            if n:
                r = self.buf.raw[:n]
                if self.drop is not None and self.dropped == 0 and self.drop(r):
                    self.dropped += 1
                else:
                    if self.mutate is not None:
                        r = self.mutate(r) or r
                    return r
            if self.clock.t >= end:
                return None


class Handshake(unittest.TestCase):
    def setUp(self):
        if SIM_LIB is None:
            self.skipTest("no p2 simulation library")
        self.lib = load_sim()

    def handshake(self, **kw):
        clock = FakeClock()
        dev = FakeHid(self.lib, clock, **kw)
        with fake_time(clock):
            rc, text = run(p2.run_handshake, dev, argparse.Namespace(seconds=2.0, hold=0.0))
        return rc, text, dev

    def test_clean(self):
        rc, text, _ = self.handshake()
        self.assertEqual((rc, last_line(text)), (0, "PASS"), text)
        self.assertIn("MAC locally administered", text)
        self.assertIn("11 SPI write 0x8000: write protected", text)
        self.assertIn("host-side median interval 1000 us", text)
        self.assertEqual(self.lib.p2sim_refusals(), 1)

    def test_lossy_reply_is_resent(self):
        rc, text, dev = self.handshake(drop=lambda r: r[0] == 0x81 and r[1] == 0x03)
        self.assertEqual(dev.dropped, 1)
        self.assertEqual((rc, last_line(text)), (0, "PASS"), text)
        rc, text, dev = self.handshake(drop=lambda r: r[0] == 0x21 and r[14] == 0x10)
        self.assertEqual(dev.dropped, 1)
        self.assertEqual((rc, last_line(text)), (0, "PASS"), text)

    def test_negative_controls(self):
        # One wrong byte in the stick calibration reply.
        def flip(i):
            return lambda r: r[:i] + bytes([r[i] ^ 0x01]) + r[i + 1:]

        rc, text, _ = self.handshake(
            mutate=lambda r: flip(22)(r) if r[0] == 0x21 and r[14] == 0x10 and r[15:17] == b"\x3d\x60" else None)
        self.assertEqual(rc, 1, text)
        self.assertIn("0x603d", last_line(text))
        # The refusal switched off: an SPI write acknowledged with status 00.
        rc, text, _ = self.handshake(
            mutate=lambda r: r[:15] + b"\x00" + r[16:] if r[0] == 0x21 and r[14] == 0x11 else None)
        self.assertEqual(rc, 1, text)
        self.assertIn("write protected", last_line(text))

    def test_watch(self):
        """Story 2.3 D35: a D-pad press published 0.5 s into the watch is
        printed and passes --expect Down; with nothing pressed it fails."""
        neutral = bytes([0x00, 0x00, 0x08, 0x80, 0x80, 0x80, 0x80, 0x00])
        down = bytes([0x00, 0x00, 0x04, 0x80, 0x80, 0x80, 0x80, 0x00])  # AD-10: hat 4 = down
        for press, want in ((True, 0), (False, 1)):
            clock = FakeClock()
            dev = FakeHid(self.lib, clock)
            t_press = clock.t + 0.5
            orig = dev.read

            def read(timeout_ms, _orig=orig, _press=press, _t=t_press, _clock=clock):
                if _press and _t <= _clock.t < _t + 0.15:
                    self.lib.p2sim_set_state(down)
                else:
                    self.lib.p2sim_set_state(neutral)
                return _orig(timeout_ms)

            dev.read = read
            with fake_time(clock):
                rc, text = run(p2.run_watch, dev, argparse.Namespace(seconds=2.0, expect="Down"))
            self.assertEqual(rc, want, text)
            if press:
                self.assertIn("Down", text)
                self.assertEqual(last_line(text), "PASS")
        self.assertEqual(p2.pressed(bytes([0x30, 0, 0x91, 0x08, 0x80, 0x01]) + bytes(58)), ["A", "Down"])

    def test_cli_guards(self):
        for argv in (["enum", "--wait-s", "0"], ["handshake", "--seconds", "-1"], ["nope"]):
            with contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit) as cm:
                    p2.main(argv)
            self.assertEqual(cm.exception.code, 2, argv)


if __name__ == "__main__":
    if len(sys.argv) < 2:
        sys.exit("usage: test_p2_probe.py FIRMWARE_DIR [P2_SIM_LIB]")
    FW_DIR = sys.argv.pop(1)
    if len(sys.argv) >= 2:
        SIM_LIB = sys.argv.pop(1)
    p2 = load_probe(FW_DIR)
    unittest.main(verbosity=1)
