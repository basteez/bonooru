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
   switched off) that must make it print FAIL;
 - story 2.5 (Task 10.4): its `stall` command against the real ocb_p2
   through a scripted hang on the same fake transport: a note pressed, then
   (core 0) the reports stop and the device vanishes 20 ms later, or (core
   1) the reports turn NEUTRAL 20 ms after the hang and the device vanishes
   60 ms after that, macOS noticing 110 ms later (the H.2a bench shape);
   the board comes back neutral; --drive ends selfplay when the device
   vanishes and takes the AC4 number from hangcheck's board-side
   stall-to-reset (D31, D32); --runs N gives the macOS removal latency
   (median, max); for core 1 the host's hang-to-last-report must agree with
   the board's interval within 15 ms (D34). Negative controls: the device never vanishing, a board
   interval above 100 ms, host and board disagreeing (core 0, and the
   270 ms of bench H.2b attempt 1 for core 1), coming back with a
   note pressed, core 1's reports never turning NEUTRAL, hangcheck failing,
   --runs without --drive.
"""

import argparse
import contextlib
import ctypes
import importlib.util
import io
import json
import os
import re
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


NEUTRAL8 = bytes([0x00, 0x00, 0x08, 0x80, 0x80, 0x80, 0x80, 0x00])
NOTE_Y8 = bytes([0x01, 0x00, 0x08, 0x80, 0x80, 0x80, 0x80, 0x00])  # AD-10: Y


class HangBoard:
    """A hang flavour on the Mac, on a fake clock (story 2.5 D6, D18, D31):
    the melody's first note at t_press; the hang `delay` later. core0: the
    reports stop, core 1 sees core 0 stale 20 ms later and reboots at once;
    core1: the reports go on, NEUTRAL 20 ms after the hang (core 0 sees core
    1 stale), the watchdog resets 60 ms after that. macOS notices the
    removal `removal_s` after the reset. The device is absent for
    `absent_s`, then a fresh board (NEUTRAL, or `back_pressed`). Each
    selfplay spawned after a reboot starts a new cycle (--runs). `run`
    answers hangcheck with `device_us` (default: the modelled interval)."""

    def __init__(self, lib, clock, expect, delay_s=0.050, gone_after_s=None, absent_s=0.3, back_pressed=False,
                 neutral_after_s=0.020, never_gone=False, never_neutral=False, removal_s=0.110, device_us=None):
        self.lib = lib
        self.clock = clock
        self.expect = expect
        self.delay_s = delay_s
        if gone_after_s is None:
            gone_after_s = 0.020 if expect == "core0" else 0.020 + 0.060
        self.reset_after_s = gone_after_s
        self.removal_s = removal_s
        self.absent_s = absent_s
        self.never_gone = never_gone
        self.neutral_after_s = neutral_after_s
        self.never_neutral = never_neutral
        self.back_pressed = back_pressed
        self.device_us = device_us
        self.spawned = []
        self.children = []
        self.ran = []
        self.run_rc = 0
        self.cycles = 0
        self.t_back = None
        self.start_cycle()

    def start_cycle(self):
        self.cycles += 1
        self.t_press = self.clock.t + 0.5
        self.t_hang = self.t_press + self.delay_s
        self.t_gone = None if self.never_gone else self.t_hang + self.reset_after_s + self.removal_s
        self.t_back = None if self.never_gone else self.t_gone + self.absent_s
        self.boot_done = False

    def gone(self):
        return self.t_gone is not None and self.t_gone <= self.clock.t < self.t_back

    def present(self):
        return not self.gone()

    def rebooted(self):
        return self.t_back is not None and self.clock.t >= self.t_back

    def state(self):
        t = self.clock.t
        if self.rebooted():
            return NOTE_Y8 if self.back_pressed else NEUTRAL8
        if t < self.t_press:
            return NEUTRAL8
        if self.expect == "core1" and not self.never_neutral and t >= self.t_hang + self.neutral_after_s:
            return NEUTRAL8
        return NOTE_Y8

    def reporting(self):
        if self.gone():
            return False
        if self.rebooted():
            return True
        if self.expect == "core0" and self.clock.t >= self.t_hang:
            return False  # core 0 hung: no more reports
        # The board reset: nothing reports, macOS has not noticed yet.
        return self.clock.t < self.t_hang + self.reset_after_s

    # The backend interface of p2_probe.run_stall.
    def open(self):
        if self.gone():
            raise OSError("no device")
        return HangHid(self)

    def spawn(self, argv):
        if self.rebooted():
            self.start_cycle()  # the next run of --runs: a new press, a new hang
        self.spawned.append(argv)
        self.children.append(FakeChild(self.clock))
        return self.children[-1]

    def run(self, argv):
        self.ran.append(argv)
        us = self.device_us if self.device_us is not None else int(self.reset_after_s * 1e6)
        return self.run_rc, "   0.000  ok   fresh boot\nHANGCHECK stall_to_reset_us=%d\n%s\n" % (
            us, "PASS" if self.run_rc == 0 else "FAIL boot_cause")


class FakeChild:
    def __init__(self, clock):
        self.clock = clock
        self.terminated = 0
        self.t_end = None

    def terminate(self):
        self.terminated += 1
        self.t_end = self.clock.t

    def communicate(self):
        return ("selfplay output", None)


class HangHid:
    """The HID transport of one boot of a HangBoard (the real ocb_p2)."""

    def __init__(self, board):
        self.board = board
        self.boot_gone = board.t_gone if not board.rebooted() else None
        self.buf = ctypes.create_string_buffer(64)
        board.lib.p2sim_init(bytes([0x02, 0x11, 0x22, 0x33, 0x44, 0x55]))

    def write(self, data):
        data = p2.pad(data)
        self.board.lib.p2sim_out(data, len(data))

    def read(self, timeout_ms):
        b = self.board
        end = b.clock.t + timeout_ms / 1000.0
        while True:
            b.clock.t += 0.002  # the Mac polls every 2 ms
            if self.boot_gone is not None and b.clock.t >= self.boot_gone:
                raise OSError("read error")  # hidapi: the device went away
            if b.reporting():
                b.lib.p2sim_set_state(b.state())
                n = b.lib.p2sim_in(self.buf, 64)
                if n:
                    return self.buf.raw[:n]
            if b.clock.t >= end:
                return None

    def close(self):
        pass


def stall_args(**kw):
    a = dict(expect="core0", hang_delay_ms=50, wait_s=10.0, drive=False, port=None, runs=1)
    a.update(kw)
    return argparse.Namespace(**a)


def drive(**kw):
    return stall_args(drive=True, port="/dev/cu.test", **kw)


class Stall(unittest.TestCase):
    """Story 2.5 F.32 extension: `stall` against a scripted hang (D18, D31 to D33)."""

    def setUp(self):
        if SIM_LIB is None:
            self.skipTest("no p2 simulation library")
        self.lib = load_sim()

    def stall(self, expect="core0", args=None, **kw):
        clock = FakeClock()
        board = HangBoard(self.lib, clock, expect, **kw)
        with fake_time(clock):
            rc, text = run(p2.run_stall, board, args or stall_args(expect=expect))
        return rc, text, board

    def test_core0(self):
        # The H.2a bench shape: the reset 20 ms after the hang, macOS sees it 110 ms later.
        rc, text, _ = self.stall("core0")
        self.assertEqual((rc, last_line(text)), (0, "PASS"), text)
        self.assertIn("first note seen: Y", text)
        self.assertIn("core 0 hung: the reports stopped", text)
        self.assertIn("information, not the AC4 number", text)
        self.assertIn("after the reboot:", text)
        self.assertIn("hangcheck --expect core0` now for the AC4 number", text)

    def test_core1(self):
        rc, text, _ = self.stall("core1")
        self.assertEqual((rc, last_line(text)), (0, "PASS"), text)
        self.assertIn("turned NEUTRAL before the reset", text)
        self.assertIn("press to NEUTRAL", text)

    def test_drive_and_the_board_number(self):
        rc, text, board = self.stall("core0", args=drive())
        self.assertEqual((rc, last_line(text)), (0, "PASS"), text)
        self.assertEqual(board.spawned[0][2:5], ["--port", "/dev/cu.test", "selfplay"])
        self.assertEqual(board.ran[0][2:], ["--port", "/dev/cu.test", "hangcheck", "--expect", "core0"])
        self.assertIn("AC4: the board's own stall-to-reset 20.0 ms <= 100 ms", text)
        m = re.search(r"macOS removal latency this run: ([0-9.]+) ms", text)
        self.assertIsNotNone(m, text)
        self.assertLess(abs(float(m.group(1)) - 110.0), 4.0, text)
        # D32: selfplay ended when the device vanished, before it came back.
        child = board.children[0]
        self.assertEqual(child.terminated, 1)
        self.assertLess(child.t_end, board.t_gone + 0.01)
        rc, text, board = self.stall("core1", args=drive(expect="core1"))
        self.assertEqual((rc, last_line(text)), (0, "PASS"), text)
        self.assertIn("AC4: the board's own stall-to-reset 80.0 ms <= 100 ms", text)
        self.assertRegex(text, r"ok   host and board agree: hang to the last report 7[89]\.[0-9] ms vs the board's 80\.0 ms")
        self.assertRegex(text, r"macOS removal latency this run: 11[0-2]\.[0-9] ms \(last report to gone")
        # D34, bench H.2b attempt 1: the board reported 270 ms for a reset the
        # host saw about 79 ms after the hang. The cross-check catches it even
        # under a loose board limit.
        rc, text, board = self.stall("core1", args=drive(expect="core1"), device_us=270000)
        self.assertEqual(rc, 1, text)
        self.assertIn("AC4: the board's own stall-to-reset 270.0 ms", text)
        self.assertIn("FAIL host and board agree: hang to the last report", text)
        rc, text, board = self.stall("core0", args=stall_args(drive=True, port=None))
        self.assertEqual(rc, 1, text)
        self.assertIn("--drive needs --port", last_line(text))

    def test_calibration_runs(self):
        rc, text, board = self.stall("core0", args=drive(runs=3))
        self.assertEqual((rc, last_line(text)), (0, "PASS"), text)
        self.assertEqual((board.cycles, len(board.ran), [c.terminated for c in board.children]), (3, 3, [1, 1, 1]))
        self.assertRegex(text, r"macOS removal latency .* over 3 run\(s\): median 1[01][0-9]\.[0-9] ms, max")
        self.assertIn("board stall-to-reset over 3 run(s): max 20.0 ms", text)
        rc, text, _ = self.stall("core0", args=stall_args(runs=2))
        self.assertEqual(rc, 1, text)
        self.assertIn("--runs above 1 needs --drive", last_line(text))

    def test_negative_controls(self):
        rc, text, _ = self.stall("core0", never_gone=True)
        self.assertEqual(rc, 1, text)
        self.assertIn("vanished within", last_line(text))
        # The --drive child is ended on every path, also this one.
        rc, text, board = self.stall("core0", never_gone=True, args=drive())
        self.assertEqual(rc, 1, text)
        self.assertEqual([c.terminated for c in board.children], [1])
        # The board's own interval above 100 ms (the AC4 failure).
        rc, text, _ = self.stall("core0", args=drive(), device_us=120000)
        self.assertEqual(rc, 1, text)
        self.assertIn("AC4: the board's own stall-to-reset 120.0 ms", last_line(text))
        # Host and board disagree: the board claims 60 ms, the host saw 20.
        rc, text, _ = self.stall("core0", args=drive(), removal_s=0.0, device_us=60000)
        self.assertEqual(rc, 1, text)
        self.assertIn("host and board agree", last_line(text))
        rc, text, _ = self.stall("core1", back_pressed=True)
        self.assertEqual(rc, 1, text)
        self.assertIn("no note pressed (pressed: Y)", last_line(text))
        rc, text, _ = self.stall("core1", never_neutral=True)
        self.assertEqual(rc, 1, text)
        self.assertIn("turned NEUTRAL", last_line(text))
        clock = FakeClock()
        board = HangBoard(self.lib, clock, "core0")
        board.run_rc = 1
        with fake_time(clock):
            rc, text = run(p2.run_stall, board, drive())
        self.assertEqual(rc, 1, text)
        self.assertIn("hangcheck --expect core0: FAIL", last_line(text))


if __name__ == "__main__":
    if len(sys.argv) < 2:
        sys.exit("usage: test_p2_probe.py FIRMWARE_DIR [P2_SIM_LIB]")
    FW_DIR = sys.argv.pop(1)
    if len(sys.argv) >= 2:
        SIM_LIB = sys.argv.pop(1)
    p2 = load_probe(FW_DIR)
    unittest.main(verbosity=1)
