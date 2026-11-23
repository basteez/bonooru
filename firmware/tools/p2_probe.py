#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""p2_probe: the Mac-side bench checks of the P2 profile (story 2.3, D18).

The controller Zero plugged into the Mac is a USB device; this tool is the
host. It is the one independent implementation of the host side of the P2
conversation, written from the public documents cited in
docs/usb-controller-emulation.md ("P2 wire protocol": dekuNukem's
Nintendo_Switch_Reverse_Engineering notes, Chromium's nintendo_controller,
SDL's hidapi Switch driver, the mzyy94 and tmk hardware dumps), not from
the firmware's C code. ctest checks it against the real ocb_p2 through a
shared module (tests/test_p2_probe.py, F.32).

Subcommands (each prints one line per check, then PASS or FAIL <reason>;
exit 0 or 1, 2 for a usage error):

  enum [--wait-s 90]      H.13: waits for 057E:2009 in system_profiler (then
                          ioreg), absorbs plug bounces (3 consecutive 1 s
                          polls), checks the identity, the strings,
                          bcdDevice 2.10 and, when ioreg exposes it, the
                          203-byte HID report descriptor (else one SKIP
                          line). macOS tools only, standard library only.
  handshake [--seconds 10] [--hold N]
                          H.14: opens the HID device (hidapi:
                          pip install hidapi), plays the host sequence of
                          the docs (80 01, 80 02, 80 03, 80 02, 80 04, the
                          subcommands, the SPI reads, a refused SPI write,
                          an unlisted id), each request resent after 300 ms
                          up to 3 times, checks every reply byte for byte,
                          then reads the 0x30 stream for --seconds and
                          prints the host-side median interval. --hold N
                          keeps reading N more seconds, so the controller's
                          console_ready stays 1 for another tool (H.11).
  watch [--seconds 30] [--expect Down]
                          story 2.3 D35: holds the stream (80 02, 80 04) and
                          prints the pressed buttons of the 0x30 reports as
                          they change, so a press sent with link_probe.py
                          press is seen end to end without a console.

  stall --expect core0|core1 [--drive --port "$P"]
                          story 2.5 H.2a/H.2b, a hang flavour on the Mac:
                          holds the stream, finds the first note, times the
                          hang to the device vanishing (host clock, the
                          assumptions printed), checks how the reports
                          end (core 0: they stop; core 1: they turn NEUTRAL),
                          waits for the board to come back with nothing
                          pressed; --drive runs link_probe.py selfplay and
                          hangcheck on the bridge port itself.

No serial port and no device path is ever hard-coded: the HID device is
found by VID:PID, the bridge port comes from --port.
"""

import argparse
import json
import os
import re
import subprocess
import sys
import time

# ----------------------------------------------------------------- constants
# docs/usb-controller-emulation.md "P2 wire protocol" (S7, S8 dumps).
VID = 0x057E
PID = 0x2009
BCD_DEVICE = 0x0210
MANUFACTURER = "Nintendo Co., Ltd."
PRODUCT = "Pro Controller"
SERIAL = "000000000001"
REPORT_LEN = 64

# S7: usbhid-dump of the report descriptor of a real wired Pro Controller.
REPORT_DESC = bytes.fromhex(
    "05 01 15 00 09 04 A1 01 85 30 05 01 05 09 19 01 "
    "29 0A 15 00 25 01 75 01 95 0A 55 00 65 00 81 02 "
    "05 09 19 0B 29 0E 15 00 25 01 75 01 95 04 81 02 "
    "75 01 95 02 81 03 0B 01 00 01 00 A1 00 0B 30 00 "
    "01 00 0B 31 00 01 00 0B 32 00 01 00 0B 35 00 01 "
    "00 15 00 27 FF FF 00 00 75 10 95 04 81 02 C0 0B "
    "39 00 01 00 15 00 25 07 35 00 46 3B 01 65 14 75 "
    "04 95 01 81 02 05 09 19 0F 29 12 15 00 25 01 75 "
    "01 95 04 81 02 75 08 95 34 81 03 06 00 FF 85 21 "
    "09 01 75 08 95 3F 81 03 85 81 09 02 75 08 95 3F "
    "81 03 85 01 09 03 75 08 95 3F 91 83 85 10 09 04 "
    "75 08 95 3F 91 83 85 80 09 05 75 08 95 3F 91 83 "
    "85 82 09 06 75 08 95 3F 91 83 C0")

# Header bytes [2..12] of a 0x21 or 0x30 report with nothing pressed (S2 for
# the layout, S7's capture for 91 / charging-grip bit / 0A).
NEUTRAL_HEADER = bytes([0x91, 0x00, 0x80, 0x00, 0x00, 0x08, 0x80, 0x00, 0x08, 0x80, 0x0A])
NEUTRAL_RUMBLE = bytes([0x00, 0x01, 0x40, 0x40, 0x00, 0x01, 0x40, 0x40])  # S2

# S12 (mzyy94's capture of a real Switch 1 and Pro Controller): the
# console's pairing request (type 04, its address and name) and the
# controller's MCU state report after 21 21 00 (story 2.3 D34).
PAIR_REQUEST = bytes.fromhex("04185395d6030400043c4e696e74656e646f2053776974636800000000006800ac90113f5900")
MCU_STANDBY = bytes.fromhex("0100ff0003000501" + "00" * 25 + "5c")

RESEND_S = 0.300
RESENDS = 3
OPEN_RETRY_S = 30.0

# SPI regions read by the probe and their expected bytes (S4 layout; values
# S4, S9; docs "SPI regions served").
SPI_EXPECT = {
    (0x6020, 24): bytes([0, 0, 0, 0, 0, 0, 0x00, 0x40, 0x00, 0x40, 0x00, 0x40,
                         0, 0, 0, 0, 0, 0, 0x3B, 0x34, 0x3B, 0x34, 0x3B, 0x34]),
    (0x6080, 6): bytes([0x50, 0xFD, 0x00, 0x00, 0xC6, 0x0F]),
    (0x603D, 18): bytes([0x00, 0x07, 0x70, 0x00, 0x08, 0x80, 0x00, 0x07, 0x70,
                         0x00, 0x08, 0x80, 0x00, 0x07, 0x70, 0x00, 0x07, 0x70]),
    (0x6086, 18): bytes([0x19, 0xD0, 0x4C, 0xAE, 0x40, 0xE1, 0xEE, 0xE2, 0x2E,
                         0xEE, 0xE2, 0x2E, 0xB4, 0x4A, 0xAB, 0x96, 0x64, 0x49]),
    (0x6000, 16): b"000000000001" + b"\xff" * 4,
    (0x8010, 22): b"\xff" * 22,
}


def pad(b):
    b = bytes(b)
    return b + bytes(REPORT_LEN - len(b))


# ------------------------------------------------------------------- enum
def _hex_int(v):
    """'0x057e', '0x057e  (Nintendo Co., Ltd.)', 1406, '1406' -> int or None."""
    if isinstance(v, int):
        return v
    if not isinstance(v, str):
        return None
    m = re.match(r"\s*(0x[0-9a-fA-F]+|[0-9]+)", v)
    if not m:
        return None
    return int(m.group(1), 0)


def _bcd(v):
    """'2.10', '0x0210', 528 -> 0x0210 (or None)."""
    if isinstance(v, int):
        return v
    if not isinstance(v, str):
        return None
    m = re.match(r"\s*(\d+)\.(\d\d)\s*$", v)
    if m:
        return (int(m.group(1)) << 8) | int(m.group(2), 16)
    return _hex_int(v)


def _norm(key):
    return re.sub(r"[^a-z]", "", key.lower())


def profiler_devices(doc):
    """Every USB device in a system_profiler JSON document (SPUSBHostDataType
    or the older SPUSBDataType), found by a recursive walk over `_items`.
    Keys are matched loosely (lower case, letters only), because their
    spelling differs between macOS versions: vendor id, product id,
    manufacturer / vendor name, product name or _name, serial, bcd / version."""
    out = []

    def walk(node):
        if isinstance(node, dict):
            dev = {}
            for k, v in node.items():
                n = _norm(k)
                if n.endswith("vendorid"):
                    dev["vid"] = _hex_int(v)
                elif n.endswith("productid"):
                    dev["pid"] = _hex_int(v)
                elif n in ("manufacturer", "usbdevicekeymanufacturername", "usbdevicekeyvendorname", "vendorname"):
                    dev["manufacturer"] = v
                elif n in ("usbdevicekeyproductname", "productname"):
                    dev["product"] = v
                elif n == "name" and "product" not in dev:
                    dev["name"] = v
                elif "serial" in n:
                    dev["serial"] = v
                elif n in ("bcddevice", "usbdevicekeybcddevice", "usbdevicekeyproductversion", "productversion"):
                    dev["bcd"] = _bcd(v)
            if dev.get("vid") is not None:
                if "product" not in dev and "name" in dev:
                    dev["product"] = dev["name"]
                dev.pop("name", None)
                out.append(dev)
            for v in node.values():
                walk(v)
        elif isinstance(node, list):
            for v in node:
                walk(v)

    walk(doc)
    return out


IOREG_PROP = re.compile(r'"([^"]+)" = (.*?)\s*$')


def ioreg_blocks(text):
    """Property blocks of `ioreg -l -w0` output, as dicts of raw strings."""
    blocks = []
    cur = None
    for line in text.splitlines():
        if "+-o " in line:
            cur = {}
            blocks.append(cur)
            continue
        m = IOREG_PROP.search(line)
        if m and cur is not None and not line.strip().startswith("+-o"):
            cur[m.group(1)] = m.group(2)
    return blocks


def _ioreg_str(v):
    if v is None:
        return None
    m = re.match(r'^"(.*)"$', v)
    return m.group(1) if m else v


def ioreg_devices(text):
    """USB devices of `ioreg -p IOUSB -l -w0` (keys idVendor, idProduct,
    "USB Vendor Name", "USB Product Name", "USB Serial Number", bcdDevice)."""
    out = []
    for b in ioreg_blocks(text):
        if "idVendor" not in b:
            continue
        out.append({"vid": _hex_int(b.get("idVendor")), "pid": _hex_int(b.get("idProduct")),
                    "manufacturer": _ioreg_str(b.get("USB Vendor Name")),
                    "product": _ioreg_str(b.get("USB Product Name")),
                    "serial": _ioreg_str(b.get("USB Serial Number") or b.get("kUSBSerialNumberString")),
                    "bcd": _hex_int(b.get("bcdDevice"))})
    return out


def ioreg_report_descriptor(text):
    """The ReportDescriptor of the HID node with our VendorID / ProductID in
    `ioreg -r -c IOHIDDevice -l -w0`, or None if macOS does not expose it."""
    for b in ioreg_blocks(text):
        if _hex_int(b.get("VendorID")) == VID and _hex_int(b.get("ProductID")) == PID and "ReportDescriptor" in b:
            m = re.match(r"^<([0-9a-fA-F]*)>$", b["ReportDescriptor"])
            if m:
                return bytes.fromhex(m.group(1))
    return None


class MacUsb:
    """The macOS tools enum uses (replaced by fixtures in the self-test)."""

    def run(self, argv):
        try:
            return subprocess.run(argv, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, timeout=30,
                                  check=False).stdout.decode("utf-8", "replace")
        except (OSError, subprocess.SubprocessError):
            return ""

    def profiler(self):
        for dt in ("SPUSBHostDataType", "SPUSBDataType"):
            text = self.run(["system_profiler", dt, "-json"])
            try:
                devs = profiler_devices(json.loads(text))
            except ValueError:
                continue
            if devs:
                return devs
        return []

    def ioreg_usb(self):
        return ioreg_devices(self.run(["ioreg", "-p", "IOUSB", "-l", "-w0"]))

    def ioreg_hid(self):
        return self.run(["ioreg", "-r", "-c", "IOHIDDevice", "-l", "-w0"])


def _ours(devs):
    return [d for d in devs if d.get("vid") == VID and d.get("pid") == PID]


def _present(usb):
    return bool(_ours(usb.profiler()) or _ours(usb.ioreg_usb()))


def cmd_enum(args, usb=None, out=print):
    usb = usb or MacUsb()
    fails = []

    def check(ok, text):
        out("%s %s" % ("ok  " if ok else "FAIL", text))
        if not ok:
            fails.append(text)
        return ok

    out(">>> PLUG THE CONTROLLER ZERO INTO THE MAC (Apple USB-C cable). Waiting up to %d s." % args.wait_s)
    t0 = time.monotonic()
    restarts = 0
    stable = 0
    while True:
        if time.monotonic() - t0 > args.wait_s:
            check(False, "057E:2009 seen stable for 3 s within %d s" % args.wait_s)
            out("     check for a macOS 'Allow accessory' prompt, or replug")
            out("FAIL %s" % fails[0])
            return 1
        stable = stable + 1 if _present(usb) else 0
        if stable >= 3:
            out("ok   057E:2009 present in 3 consecutive 1 s polls (%.0f s after the prompt)" % (
                time.monotonic() - t0))
            # A disappearance during the checks restarts the stable window once.
            devs = _ours(usb.ioreg_usb()) or _ours(usb.profiler())
            if not devs:
                if restarts == 0:
                    restarts += 1
                    stable = 0
                    out("     the device vanished during the checks (bounce): waiting again")
                    continue
            break
        time.sleep(1.0)

    ioreg = _ours(usb.ioreg_usb())
    prof = _ours(usb.profiler())
    src = "ioreg" if ioreg else "system_profiler"
    dev = (ioreg or prof)[0] if (ioreg or prof) else {}
    out("     fields from %s: %s" % (src, dev))
    check(dev.get("vid") == VID and dev.get("pid") == PID, "VID:PID %04X:%04X" % (dev.get("vid") or 0,
                                                                                  dev.get("pid") or 0))
    check(dev.get("manufacturer") == MANUFACTURER, "manufacturer %r (want %r)" % (dev.get("manufacturer"),
                                                                                  MANUFACTURER))
    check(dev.get("product") == PRODUCT, "product %r (want %r)" % (dev.get("product"), PRODUCT))
    check(dev.get("serial") == SERIAL, "serial %r (want %r)" % (dev.get("serial"), SERIAL))
    check(dev.get("bcd") == BCD_DEVICE, "bcdDevice %s (want 2.10)" % (
        ("%x.%02x" % (dev["bcd"] >> 8, dev["bcd"] & 0xFF)) if dev.get("bcd") is not None else None))
    rd = ioreg_report_descriptor(usb.ioreg_hid())
    if rd is None:
        out("SKIP HID report descriptor: ioreg does not expose it on this Mac")
    else:
        check(rd == REPORT_DESC, "HID report descriptor %d bytes, equal to the 203 expected" % len(rd))
    if fails:
        out("FAIL %s" % fails[0])
        return 1
    out("PASS")
    return 0


# -------------------------------------------------------------- handshake
class HidTransport:
    """hidapi (pip install hidapi, module `hid`), imported only here."""

    def __init__(self):
        import hid  # noqa: PLC0415 (bench-machine dependency, as pyserial)
        self.dev = hid.device()
        self.dev.open(VID, PID)
        self.dev.set_nonblocking(False)

    def write(self, data):
        self.dev.write(list(pad(data)))

    def read(self, timeout_ms):
        r = self.dev.read(REPORT_LEN, int(timeout_ms))
        return bytes(r) if r else None

    def close(self):
        self.dev.close()


def open_hid(out):
    """Retries for up to 30 s; the Input Monitoring hint once on failure."""
    t0 = time.monotonic()
    hinted = False
    while True:
        try:
            return HidTransport()
        except ImportError:
            out("FAIL hidapi is not installed: pip install hidapi")
            return None
        except (IOError, OSError, ValueError) as e:
            if not hinted:
                out("     cannot open 057E:2009 (%s); if macOS asks, allow Input Monitoring for the Terminal "
                    "(System Settings > Privacy & Security > Input Monitoring) and rerun. Retrying for %d s."
                    % (e, OPEN_RETRY_S))
                hinted = True
            if time.monotonic() - t0 > OPEN_RETRY_S:
                out("     check for a macOS 'Allow accessory' prompt, or replug")
                out("FAIL cannot open the HID device 057E:2009")
                return None
            time.sleep(1.0)


def usb_cmd(cmd):
    return bytes([0x80, cmd])


class Host:
    """The host side of the documents' sequence on one transport."""

    def __init__(self, dev, out):
        self.dev = dev
        self.out = out
        self.counter = 0
        self.unexpected = []
        self.reports = []  # (time, bytes) of 0x30 reports seen
        self.fails = []

    def check(self, ok, text):
        self.out("%s %s" % ("ok  " if ok else "FAIL", text))
        if not ok:
            self.fails.append(text)
        return ok

    def subcmd(self, sc, args=b""):
        b = bytes([0x01, self.counter & 0x0F]) + NEUTRAL_RUMBLE + bytes([sc]) + bytes(args)
        self.counter += 1
        return b

    def _take(self, r, match):
        if r[0] == 0x30:
            self.reports.append((time.monotonic(), r))
            return False
        if match(r):
            return True
        self.unexpected.append(r[:16].hex())
        return False

    def request(self, data, match):
        """Sends `data`, waits 300 ms for a reply that `match`es, resends up
        to 3 times. Returns the reply or None."""
        for _ in range(1 + RESENDS):
            self.dev.write(data)
            end = time.monotonic() + RESEND_S
            while time.monotonic() < end:
                r = self.dev.read(max(1, int((end - time.monotonic()) * 1000)))
                if r and self._take(r, match):
                    return r
        return None

    def usb(self, cmd, want):
        r = self.request(usb_cmd(cmd), lambda r: r[0] == 0x81 and r[1] == cmd)
        self.check(r is not None and r == pad(want), "80 %02x -> %s" % (
            cmd, r[:len(want) + 2].hex(" ") if r else "no reply"))
        return r

    def sub(self, sc, args, ack, data, what):
        r = self.request(self.subcmd(sc, args), lambda r: r[0] == 0x21 and r[14] == sc)
        if r is None:
            return self.check(False, "%s: no reply" % what)
        want = bytes([0x21, r[1]]) + NEUTRAL_HEADER + bytes([ack, sc]) + bytes(data)
        return self.check(r == pad(want), "%s -> %s" % (what, r[13:15 + len(data)].hex(" ")))


def spi_args(addr, n):
    return bytes([addr & 0xFF, (addr >> 8) & 0xFF, (addr >> 16) & 0xFF, (addr >> 24) & 0xFF, n])


def median(xs):
    s = sorted(xs)
    return s[(len(s) - 1) // 2] if s else 0


def run_handshake(dev, args, out=print):
    h = Host(dev, out)
    # 1-4: the USB commands (S1, S5, S6).
    r = h.request(usb_cmd(0x01), lambda r: r[0] == 0x81 and r[1] == 0x01)
    h.check(r is not None and r[2] == 0x00 and r[3] == 0x03 and r[10:] == bytes(REPORT_LEN - 10),
            "80 01 -> 81 01 00 03 + MAC %s" % (r[4:10][::-1].hex(":") if r else "no reply"))
    if r is not None:
        mac = r[4:10][::-1]
        h.check(mac[0] & 0x03 == 0x02, "MAC locally administered, unicast (%s)" % mac.hex(":"))
    else:
        mac = bytes(6)
    h.usb(0x02, [0x81, 0x02])
    h.usb(0x03, [0x81, 0x03])
    h.usb(0x02, [0x81, 0x02])
    # 5: 80 04, no reply, then the 0x30 stream within 1 s.
    n0 = len(h.reports)
    dev.write(usb_cmd(0x04))
    end = time.monotonic() + 1.0
    while time.monotonic() < end and len(h.reports) == n0:
        rr = dev.read(50)
        if rr:
            h._take(rr, lambda _r: False)
    h.check(len(h.reports) > n0, "80 04 -> 0x30 reports begin within 1 s")
    # 6: device info (S3): fw 04 00, type 03, 02, MAC, 01, colour 00.
    h.sub(0x02, b"", 0x82, bytes([0x04, 0x00, 0x03, 0x02]) + mac + bytes([0x01, 0x00]), "02 device info")
    # 7: SPI reads (S5, S6 addresses).
    for (addr, n), data in SPI_EXPECT.items():
        h.sub(0x10, spi_args(addr, n), 0x90, spi_args(addr, n) + data, "10 SPI read 0x%04x (%d)" % (addr, n))
    # 8: settings.
    for sc, a in ((0x30, b"\x01"), (0x40, b"\x00"), (0x41, b"\x03\x00\x00\x01"), (0x48, b"\x01"), (0x03, b"\x30")):
        h.sub(sc, a, 0x80, b"", "%02x ack" % sc)
    # 8b (story 2.3 D34): what a real console sends in its init (S12 capture):
    # the pairing request gets 81 01 03, the MCU configuration A0 21 and the
    # standby state report; both change nothing.
    h.sub(0x01, PAIR_REQUEST, 0x81, b"\x03", "01 pairing request -> 81 01 03, nothing paired")
    h.sub(0x21, b"\x21\x00", 0xA0, MCU_STANDBY, "21 MCU config -> A0 21 + standby state")
    # 9: refusal probe (C7): an SPI write is write-protected and changes nothing.
    h.sub(0x10, spi_args(0x8000, 1), 0x90, spi_args(0x8000, 1) + b"\xff", "10 SPI read 0x8000 before the write")
    h.sub(0x11, spi_args(0x8000, 1) + b"\xaa", 0x80, b"\x01", "11 SPI write 0x8000: write protected")
    h.sub(0x10, spi_args(0x8000, 1), 0x90, spi_args(0x8000, 1) + b"\xff", "10 SPI read 0x8000 unchanged")
    # 10: an unlisted id (S3: 80 <id> 03).
    h.sub(0x7F, b"", 0x80, b"\x03", "7f unlisted -> generic ack")
    # 11: the stream.
    start = len(h.reports)
    end = time.monotonic() + args.seconds + args.hold
    while time.monotonic() < end:
        rr = dev.read(100)
        if rr:
            h._take(rr, lambda _r: False)
    reps = h.reports[start:]
    timer_gaps = sum(1 for a, b in zip(reps, reps[1:]) if (b[1][1] - a[1][1]) & 0xFF != 1)
    bad = [r for _, r in reps if r[2:13] != NEUTRAL_HEADER or r[13:] != bytes(REPORT_LEN - 13)]
    iv = [(b[0] - a[0]) * 1e6 for a, b in zip(reps, reps[1:])]
    h.check(len(reps) > 0 and not bad, "%d 0x30 reports, every one 30 .. 91 00 80 00 neutral sticks 0A, IMU zero"
            % len(reps))
    h.check(len(reps) > 1 and timer_gaps <= len(reps) // 100,
            "timer +1 per report (%d discontinuities in %d)" % (timer_gaps, len(reps)))
    out("     host-side median interval %.0f us over %d reports (hidapi timing, not the console's)" % (
        median(iv), len(reps)))
    if h.unexpected:
        out("     note: %d replies the probe did not ask for (macOS talking to the controller?): %s" % (
            len(h.unexpected), ", ".join(h.unexpected[:5])))
    if h.fails:
        out("FAIL %s" % h.fails[0])
        return 1
    out("PASS")
    return 0


def cmd_handshake(args, out=print):
    dev = open_hid(out)
    if dev is None:
        return 1
    return run_handshake(dev, args, out)


# Report 0x30 decode (S2 button table), for `watch`.
BTN_BITS = ((3, 0x01, "Y"), (3, 0x02, "X"), (3, 0x04, "B"), (3, 0x08, "A"), (3, 0x40, "R"), (3, 0x80, "ZR"),
            (4, 0x01, "Minus"), (4, 0x02, "Plus"), (4, 0x04, "RStick"), (4, 0x08, "LStick"), (4, 0x10, "Home"),
            (4, 0x20, "Capture"), (5, 0x01, "Down"), (5, 0x02, "Up"), (5, 0x04, "Right"), (5, 0x08, "Left"),
            (5, 0x40, "L"), (5, 0x80, "ZL"))


def pressed(r):
    """The names of the buttons pressed in a 0x30 report (S2), sorted."""
    return sorted(name for i, mask, name in BTN_BITS if r[i] & mask)


def run_watch(dev, args, out=print):
    """Story 2.3 D35: the host holds the stream and prints every change of
    the pressed buttons, so a press sent over the link (`link_probe.py
    press`) can be seen arriving in the USB report, with no console."""
    h = Host(dev, out)
    h.usb(0x02, [0x81, 0x02])
    dev.write(usb_cmd(0x04))
    out(">>> STREAM ON. Watching the 0x30 buttons for %.0f s: send the press now (link_probe.py press)."
        % args.seconds)
    seen = set()
    last = None
    n = 0
    end = time.monotonic() + args.seconds
    while time.monotonic() < end:
        r = dev.read(100)
        if not r or r[0] != 0x30:
            continue
        n += 1
        now = pressed(r)
        if now != last:
            out("     %8.3f s  %s" % (args.seconds - (end - time.monotonic()), " ".join(now) or "(nothing)"))
            last = now
        seen.update(now)
    h.check(n > 0, "%d 0x30 reports read" % n)
    if args.expect:
        h.check(args.expect in seen, "%s seen in the 0x30 stream (seen: %s)" % (args.expect,
                                                                               " ".join(sorted(seen)) or "nothing"))
    if h.fails:
        out("FAIL %s" % h.fails[0])
        return 1
    out("PASS")
    return 0


def cmd_watch(args, out=print):
    dev = open_hid(out)
    if dev is None:
        return 1
    return run_watch(dev, args, out)


# ------------------------------------------------------------ stall (2.5)
# Story 2.5 D18, H.2a and H.2b: a hang flavour (fw-debug-hang0 or -hang1)
# plugged into the Mac. The host clock times the hang; the assumptions are
# printed with every result.
STALL_WAIT_S = 120.0       # time for the press to come (or the device to come back)
STALL_HANG_DELAY_MS = 50   # OCB_DEBUG_HANG_DELAY_US of the hang presets
STALL_REPORT_LATENCY_MS = 4  # a report reaches hidapi within one Mac poll (2 ms) plus one more
STALL_LIMIT_MS = 100       # AC4: stall to reset, the board's own interval (hangcheck, D31)
STALL_CONSISTENCY_MS = 5   # host last report to gone may undercut the board's interval by this much (2 ms polls)
STALL_CORE1_AGREE_MS = 15  # core 1: host hang-to-last-report vs the board's interval (polls, arming, D34)
STALL_GONE_BUDGET_S = 2.0  # after the press: the device must vanish by then
STALL_POLL_S = 0.005       # presence poll while the reports are silent
STALL_STOP_GAP_MS = 10     # core 0 hang: the reports stop at least this long before the device vanishes
STALL_NEUTRAL_READ_S = 1.0  # after it comes back: reports read to prove nothing is pressed
_HANGCHECK_RE = re.compile(r"HANGCHECK stall_to_reset_us=(\d+)")
NOTE_BUTTONS = ("Y", "X", "A", "L", "R")  # the AD-9 note buttons of the melody


def note_pressed(r):
    """The note buttons (AD-9) pressed in a 0x30 report."""
    return [b for b in pressed(r) if b in NOTE_BUTTONS]


class HidBackend:
    """hidapi and subprocess for `stall`, replaced by a fake in the tests."""

    def open(self):
        return HidTransport()

    def present(self):
        import hid  # noqa: PLC0415
        return bool(hid.enumerate(VID, PID))

    def spawn(self, argv):
        return subprocess.Popen(argv, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, universal_newlines=True)

    def run(self, argv):
        """(exit code, output) of a link_probe.py step."""
        p = subprocess.run(argv, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, universal_newlines=True)
        return p.returncode, p.stdout


def _link_probe_argv(args, *rest):
    tool = os.path.join(os.path.dirname(os.path.abspath(__file__)), "link_probe.py")
    return [sys.executable, tool, "--port", args.port] + list(rest)


def _open_retry(backend, budget_s):
    end = time.monotonic() + budget_s
    while True:
        try:
            return backend.open()
        except (IOError, OSError, ValueError, ImportError):
            if time.monotonic() >= end:
                return None
            time.sleep(0.2)


def _stream_on(dev, out):
    h = Host(dev, out)
    h.usb(0x02, [0x81, 0x02])
    dev.write(usb_cmd(0x04))
    return h


class _Child:
    """The --drive selfplay process: ended exactly once, on every path."""

    def __init__(self):
        self.proc = None

    def end(self):
        """Terminates it (if any, once) and returns the last lines it printed."""
        if self.proc is None:
            return []
        proc, self.proc = self.proc, None
        proc.terminate()
        return (proc.communicate()[0] or "").splitlines()[-5:]


def run_stall(backend, args, out=print):
    """H.2a/H.2b (story 2.5 D18, D31 to D33): times a hang from the first note
    to the device vanishing, checks how the reports end and that the board
    comes back with nothing pressed. With --drive it runs link_probe.py
    selfplay (ended as soon as the device vanishes, so nothing drives the
    rebooted board) and then hangcheck, whose board-side stall-to-reset
    interval is the AC4 number (<= 100 ms); the host-side times are
    information, and for core 0 they give the macOS removal latency
    (calibration). --runs N repeats the whole cycle (needs --drive)."""
    if args.runs > 1 and not args.drive:
        out("FAIL --runs above 1 needs --drive (each run restarts selfplay)")
        return 1
    results = []
    rc = 0
    for i in range(args.runs):
        if args.runs > 1:
            out(">>> run %d of %d" % (i + 1, args.runs))
        child = _Child()
        try:
            r, info = _run_stall(backend, args, out, child)
        finally:
            child.end()
        rc = rc or r
        results.append(info)
        if r != 0:
            break
    if args.runs == 1:
        return rc  # the run's own PASS / FAIL line is the last one
    lat = sorted(x["removal_ms"] for x in results if x.get("removal_ms") is not None)
    dev = [x["device_ms"] for x in results if x.get("device_ms") is not None]
    if dev:
        out("     board stall-to-reset over %d run(s): max %.1f ms (AC4 limit %d ms)" % (len(dev), max(dev),
                                                                                       STALL_LIMIT_MS))
    if lat:
        out("     macOS removal latency (host gone - last report - board stall-to-reset) over %d run(s): median "
            "%.1f ms, max %.1f ms" % (len(lat), lat[(len(lat) - 1) // 2], lat[-1]))
    out("PASS" if rc == 0 else "FAIL run %d of %d did not pass" % (len(results), args.runs))
    return rc


def _run_stall(backend, args, out, child):
    fails = []
    info = {"ok": False, "removal_ms": None, "device_ms": None}

    def check(ok, text):
        out("%s %s" % ("ok  " if ok else "FAIL", text))
        if not ok:
            fails.append(text)
        return ok

    def finish():
        if fails:
            out("FAIL %s" % fails[0])
            return 1, info
        info["ok"] = True
        out("PASS")
        return 0, info

    delay = args.hang_delay_ms
    out("     the AC4 number is the board's own stall-to-reset interval (hangcheck, D31); host times are "
        "information: hang delay %d ms (the preset), report latency %d ms" % (delay, STALL_REPORT_LATENCY_MS))
    dev = _open_retry(backend, args.wait_s)
    if not check(dev is not None, "the controller 057E:2009 is open (fw-debug-hang%s on the Mac)" %
                 ("0" if args.expect == "core0" else "1")):
        return finish()
    _stream_on(dev, out)
    if args.drive:
        if not check(bool(args.port), "--drive needs --port \"$P\" (the bridge's serial port)"):
            return finish()
        child.proc = backend.spawn(_link_probe_argv(args, "selfplay", "--wait-s", "%d" % args.wait_s))
        out(">>> STREAM ON. link_probe.py selfplay started on %s; waiting up to %.0f s for the first note." % (
            args.port, args.wait_s))
    else:
        out(">>> STREAM ON. Start `python3 firmware/tools/link_probe.py --port \"$P\" selfplay` now; waiting up to "
            "%.0f s for the first note." % args.wait_s)
    t_press = t_last = t_neutral = t_gone = None
    n = 0
    end = time.monotonic() + args.wait_s
    while t_gone is None:
        now = time.monotonic()
        if t_press is None and now >= end:
            break
        if t_press is not None and now - t_press > STALL_GONE_BUDGET_S:
            break
        try:
            r = dev.read(STALL_POLL_S * 1000.0)
        except (IOError, OSError, ValueError):
            t_gone = time.monotonic()  # the read failed: the device went away
            break
        if r and r[0] == 0x30:
            n += 1
            t = time.monotonic()
            notes = note_pressed(r)
            if t_press is None and notes:
                t_press = t
                out("     first note seen: %s" % " ".join(notes))
            elif t_press is not None and not notes and t_neutral is None and not pressed(r):
                t_neutral = t
            t_last = t
            continue
        if t_press is not None and not backend.present():
            t_gone = time.monotonic()
    # D32: the board is gone (or never pressed): stop selfplay now, so its
    # heartbeats never arm the rebooted board's link and hangcheck sees the
    # boot as it is.
    tail = child.end()
    if t_press is None:
        for line in tail:
            out("     selfplay: %s" % line)
    _close(dev)
    if not check(t_press is not None, "a note button pressed in the 0x30 stream within %.0f s (%d reports)" % (
            args.wait_s, n)):
        return finish()
    if not check(t_gone is not None, "the controller vanished within %.0f s of the first note (the reboot)"
                 % STALL_GONE_BUDGET_S):
        return finish()

    def ms(t):
        return (t - t_press) * 1000.0 if t is not None else float("nan")

    out("     raw: press 0 ms, last report %+.1f ms, neutral report %s, gone %+.1f ms (host monotonic)" % (
        ms(t_last), ("%+.1f ms" % ms(t_neutral)) if t_neutral is not None else "none", ms(t_gone)))
    if args.expect == "core0":
        check((t_gone - t_last) * 1000.0 >= STALL_STOP_GAP_MS,
              "core 0 hung: the reports stopped %.1f ms before the device vanished (want >= %d ms)" % (
                  (t_gone - t_last) * 1000.0, STALL_STOP_GAP_MS))
    else:
        check(t_neutral is not None and t_neutral < t_gone,
              "core 1 hung: the reports went on and turned NEUTRAL before the reset (core 0 saw core 1 stale)")
        if t_neutral is not None:
            out("     press to NEUTRAL %.1f ms = %d ms hang delay + %.1f ms (expected about 20 to 30 ms)" % (
                ms(t_neutral), delay, ms(t_neutral) - delay))
    out("     host: press to gone %.1f ms, minus the %d ms delay and %d ms report latency = %.1f ms (includes the "
        "macOS removal latency; information, not the AC4 number)" % (
            ms(t_gone), delay, STALL_REPORT_LATENCY_MS, ms(t_gone) - delay - STALL_REPORT_LATENCY_MS))
    out(">>> The controller rebooted; waiting up to %.0f s for it to come back." % args.wait_s)
    end = time.monotonic() + args.wait_s
    while not backend.present() and time.monotonic() < end:
        time.sleep(0.05)
    back = backend.present()
    if not check(back, "the controller came back (re-enumerated) within %.0f s" % args.wait_s):
        return finish()
    out("     back %.0f ms after it vanished" % ((time.monotonic() - t_gone) * 1000.0))
    dev = _open_retry(backend, 30.0)
    if not check(dev is not None, "reopened after the reboot"):
        return finish()
    _stream_on(dev, out)
    bad = []
    seen = 0
    end = time.monotonic() + STALL_NEUTRAL_READ_S
    while time.monotonic() < end:
        r = dev.read(50)
        if r and r[0] == 0x30:
            seen += 1
            if note_pressed(r):
                bad.append(" ".join(note_pressed(r)))
    _close(dev)
    check(seen > 0 and not bad, "after the reboot: %d reports, no note pressed%s" % (
        seen, (" (pressed: %s)" % bad[0]) if bad else ""))
    if args.drive:
        rc, text = backend.run(_link_probe_argv(args, "hangcheck", "--expect", args.expect))
        for line in (text or "").splitlines():
            out("     hangcheck: %s" % line)
        m = _HANGCHECK_RE.search(text or "")
        if check(rc == 0 and m is not None, "link_probe.py hangcheck --expect %s: %s" % (
                args.expect, "PASS" if rc == 0 else "FAIL")):
            device_ms = int(m.group(1)) / 1000.0
            info["device_ms"] = device_ms
            check(device_ms <= STALL_LIMIT_MS, "AC4: the board's own stall-to-reset %.1f ms <= %d ms" % (
                device_ms, STALL_LIMIT_MS))
            if args.expect == "core0":
                # The reports stop at the hang (within one 2 ms poll), so the
                # host's last-report-to-gone time is the board's stall-to-reset
                # plus the macOS removal latency.
                removal = (t_gone - t_last) * 1000.0 - device_ms
                info["removal_ms"] = removal
                out("     macOS removal latency this run: %.1f ms = %.1f ms (last report to gone) - %.1f ms (board)"
                    % (removal, (t_gone - t_last) * 1000.0, device_ms))
                check(removal >= -STALL_CONSISTENCY_MS, "host and board agree: the host saw at least the board's "
                      "interval (within %d ms)" % STALL_CONSISTENCY_MS)
            elif t_last is not None:
                # D34: core 0 keeps reporting (NEUTRAL) until the watchdog
                # resets the board, so the host's last report marks the reset
                # (within one 2 ms poll) and press + hang delay marks the hang
                # (the note is seen within one poll of its publication): an
                # independent estimate of the board's interval.
                host_ms = (t_last - t_press) * 1000.0 - delay
                check(abs(host_ms - device_ms) <= STALL_CORE1_AGREE_MS,
                      "host and board agree: hang to the last report %.1f ms vs the board's %.1f ms (within %d ms)"
                      % (host_ms, device_ms, STALL_CORE1_AGREE_MS))
                removal = (t_gone - t_last) * 1000.0
                info["removal_ms"] = removal
                out("     macOS removal latency this run: %.1f ms (last report to gone; core 0 reported until the "
                    "reset)" % removal)
    else:
        out("     run `python3 firmware/tools/link_probe.py --port \"$P\" hangcheck --expect %s` now for the AC4 "
            "number (the board's own stall-to-reset)" % args.expect)
    return finish()


def _close(dev):
    close = getattr(dev, "close", None)
    if close is not None:
        try:
            close()
        except (IOError, OSError, ValueError):
            pass


def cmd_stall(args, out=print):
    return run_stall(HidBackend(), args, out)


# -------------------------------------------------------------------- cli
def wait_arg(text):
    try:
        v = float(text)
    except ValueError:
        raise argparse.ArgumentTypeError("not a number: %r" % text) from None
    if not 1.0 <= v <= 600.0:
        raise argparse.ArgumentTypeError("must be in 1..600 s, got %s" % text)
    return v


def seconds_arg(text):
    try:
        v = float(text)
    except ValueError:
        raise argparse.ArgumentTypeError("not a number: %r" % text) from None
    if not 0.0 <= v <= 3600.0:
        raise argparse.ArgumentTypeError("must be in 0..3600 s, got %s" % text)
    return v


def runs_arg(text):
    try:
        v = int(text)
    except ValueError:
        raise argparse.ArgumentTypeError("not an integer: %r" % text) from None
    if not 1 <= v <= 50:
        raise argparse.ArgumentTypeError("must be in 1..50, got %d" % v)
    return v


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("enum", help="H.13: the Mac sees 057E:2009 with the right descriptors")
    p.add_argument("--wait-s", type=wait_arg, default=90.0, help="time to plug the board (default %(default)s)")
    p = sub.add_parser("handshake", help="H.14: the host-driven handshake and the 0x30 stream")
    p.add_argument("--seconds", type=seconds_arg, default=10.0, help="0x30 stream to read (default %(default)s)")
    p.add_argument("--hold", type=seconds_arg, default=0.0,
                   help="keep reading N more seconds so console_ready stays 1 (H.11)")
    p = sub.add_parser("watch", help="story 2.3 D35: hold the stream and print the pressed buttons as they change")
    p.add_argument("--seconds", type=seconds_arg, default=30.0, help="time to watch (default %(default)s)")
    p.add_argument("--expect", choices=sorted(name for _, _, name in BTN_BITS),
                   help="FAIL unless this button is seen pressed at least once")
    p = sub.add_parser("stall", help="story 2.5 H.2a/H.2b: time a hang flavour's reboot from the first note "
                                     "(Mac, no console)")
    p.add_argument("--expect", choices=("core0", "core1"), required=True, dest="expect")
    p.add_argument("--hang-delay-ms", type=int, default=STALL_HANG_DELAY_MS,
                   help="the preset's OCB_DEBUG_HANG_DELAY_US in ms (default %(default)s)")
    p.add_argument("--wait-s", type=wait_arg, default=STALL_WAIT_S,
                   help="time for the first note, and for the device to come back (default %(default)s)")
    p.add_argument("--drive", action="store_true",
                   help="run link_probe.py selfplay (and hangcheck at the end) on --port itself")
    p.add_argument("--port", default=None, help="the bridge's serial port, for --drive (pass \"$P\")")
    p.add_argument("--runs", type=runs_arg, default=1,
                   help="repeat the whole cycle N times (needs --drive): the calibration of the macOS removal "
                        "latency, median and max (default %(default)s)")
    args = ap.parse_args(argv)
    if args.cmd == "enum":
        return cmd_enum(args)
    if args.cmd == "watch":
        return cmd_watch(args)
    if args.cmd == "stall":
        return cmd_stall(args)
    return cmd_handshake(args)


if __name__ == "__main__":
    sys.exit(main())
