# SPDX-License-Identifier: MIT
"""Host test F.22 (story 2.7 Task 7.2, D18): one value, one place.

Run by ctest as `python3 -I test_docs_constants.py REPO_DIR` (docs_constants_check).
The watchdog values live in firmware/common/proto/include/ocb_proto/consts.h and
firmware/common/proto/vectors/constants.txt (test_vectors compares those two).
This test makes the copies in docs/link-protocol.md fail loudly when they drift:

  1. every row `| `OCB_NAME` | value | ... |` of the "Timing constants" table
     whose NAME is in constants.txt has the same value (the file stores
     big-endian hex, the doc decimal or 2^N);
  2. OCB_LINK_TIMEOUT_MS <= OCB_LINK_TIMEOUT_MAX_MS <= 100 and
     OCB_HEARTBEAT_MAX_GAP_MS < OCB_LINK_TIMEOUT_MS;
  3. in the "Watchdog" section and the "Rules the controller link applies
     (Story 2.7)" section, every number written next to a constant's name
     (`` `OCB_NAME` (N ...`` or `` `OCB_NAME` = N``) equals the value, and the
     prose sentences of the Watchdog section that state the values in words
     (a regex per sentence) still state the right ones. Controller-only
     values of ocb_link/config.h written there are checked against config.h.
  4. story 2.3: the P2 descriptor table of docs/usb-controller-emulation.md
     (VID:PID, bcdDevice, configuration length, report descriptor length,
     default bInterval, report length) equals ocb_p2/config.h, and the STATUS
     v3 tail sizes written in docs/link-protocol.md (24 bytes with an empty
     window, at most 56, body at most 67) equal ocb_link/config.h.
  5. story 2.5: the STATUS v4 sizes (block 39 bytes, tail 63 with an empty
     window and at most 95, body at most 106, 113 bytes on the wire) and the
     v4 table's offsets equal ocb_link/config.h; the liveness numbers of
     "Rules the controller applies (Story 2.5)" (20 ms, 60 ms, 5 ms, 85 ms)
     equal ocb_core/live.h; the LATE fallback written in the docs (8000 us)
     equals ocb_core/core.h.
Stdlib only.
"""

import os
import re
import sys

REPO = None


def read(*parts):
    with open(os.path.join(REPO, *parts), "r", encoding="utf-8") as f:
        return f.read()


def constants():
    out = {}
    for line in read("firmware", "common", "proto", "vectors", "constants.txt").splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        k, _, v = line.partition(":")
        out[k.strip()] = int(v.strip(), 16)
    return out


def config_values():
    text = read("firmware", "controller", "link", "include", "ocb_link", "config.h")
    return {m.group(1): int(m.group(2)) for m in re.finditer(r"^#define OCB_([A-Z0-9_]+) (\d+)\s*$", text, re.M)}


def doc_value(text):
    text = text.strip()
    m = re.fullmatch(r"2\^(\d+)", text)
    if m:
        return 2 ** int(m.group(1))
    return int(text)


def section(doc, heading):
    """The text of a markdown section from its heading to the next heading
    of the same or a higher level."""
    m = re.search(r"^(#+) " + re.escape(heading) + r"\s*$", doc, re.M)
    if m is None:
        raise AssertionError("section not found: %s" % heading)
    level = len(m.group(1))
    rest = doc[m.end():]
    nxt = re.search(r"^#{1,%d} " % level, rest, re.M)
    return rest[:nxt.start()] if nxt else rest


# Watchdog-section prose (written in words, not next to a name): regex,
# then the constant name of each group, in order.
WATCHDOG_SENTENCES = [
    (r"A hardware alarm drives it every (\d+) ms; wire spacing ≤ (\d+) ms",
     ("HEARTBEAT_PERIOD_MS", "HEARTBEAT_MAX_GAP_MS")),
    (r"`OCB_LINK_TIMEOUT_MS` \(≤ (\d+) ms, default (\d+)\)", ("LINK_TIMEOUT_MAX_MS", "LINK_TIMEOUT_MS")),
    (r"default (\d+) ms until W\.1\), DIRECT mode only", ("APP_TIMEOUT_MS_DEFAULT",)),
    (r"when idle for `app_timeout_ms/(\d+)`", ("APP_PING_DIVISOR",)),
    (r"\| ≤ (\d+) ms \(NFR-05, A\.3\) \|", ("LINK_TIMEOUT_MAX_MS",)),
]
# Story 2.7 section prose stated in words.
RULES_SENTENCES = [
    (r"exactly (\d+) ms is still alive, \d+ ms \+ 1 µs trips", ("LINK_TIMEOUT_MS",)),
    (r"exactly \d+ ms is still alive, (\d+) ms \+ 1 µs trips", ("LINK_TIMEOUT_MS",)),
    (r"exactly (\d+) ms is still alive\. All times", ("APP_TIMEOUT_MS_DEFAULT",)),
    (r"A loss by silence is decided (\d+) ms after the last heartbeat", ("LINK_TIMEOUT_MS",)),
    (r"inside the (\d+) ms of NFR-05", ("LINK_TIMEOUT_MAX_MS",)),
]
NAMED = re.compile(r"`OCB_([A-Z0-9_]+)`\s*(?:\(|=\s*)(\d+)")


def check(doc, consts, cfg):
    errors = []
    # 1. The timing table.
    table = section(doc, "Timing constants")
    rows = re.findall(r"^\|\s*`OCB_([A-Z0-9_]+)`\s*\|\s*([^|]+?)\s*\|", table, re.M)
    if len(rows) < 10:
        errors.append("timing table: only %d rows found" % len(rows))
    seen = {}
    for name, value in rows:
        if name not in consts:
            continue
        try:
            v = doc_value(value)
        except ValueError:
            errors.append("timing table: OCB_%s has a non-numeric value %r" % (name, value))
            continue
        seen[name] = v
        if v != consts[name]:
            errors.append("timing table: OCB_%s is %d in the doc, %d in constants.txt" % (name, v, consts[name]))
    for name in ("HEARTBEAT_PERIOD_MS", "HEARTBEAT_MAX_GAP_MS", "LINK_TIMEOUT_MS", "LINK_TIMEOUT_MAX_MS",
                 "APP_TIMEOUT_MS_DEFAULT", "APP_PING_DIVISOR"):
        if name not in seen:
            errors.append("timing table: OCB_%s missing" % name)
    # 2. Relations.
    if not consts["LINK_TIMEOUT_MS"] <= consts["LINK_TIMEOUT_MAX_MS"] <= 100:
        errors.append("OCB_LINK_TIMEOUT_MS <= OCB_LINK_TIMEOUT_MAX_MS <= 100 does not hold")
    if not consts["HEARTBEAT_MAX_GAP_MS"] < consts["LINK_TIMEOUT_MS"]:
        errors.append("OCB_HEARTBEAT_MAX_GAP_MS < OCB_LINK_TIMEOUT_MS does not hold")
    # 3. Prose.
    for heading, sentences in (("Watchdog (decided 2026-11-19)", WATCHDOG_SENTENCES),
                               ("Rules the controller link applies (Story 2.7)", RULES_SENTENCES)):
        text = section(doc, heading)
        named = 0
        for m in NAMED.finditer(text):
            name, value = m.group(1), int(m.group(2))
            want = consts.get(name, cfg.get(name))
            if want is None:
                errors.append("%s: `OCB_%s` is neither a wire constant nor in ocb_link/config.h" % (heading, name))
            elif value != want:
                errors.append("%s: `OCB_%s` written as %d, the value is %d" % (heading, name, value, want))
            named += 1
        for pattern, names in sentences:
            m = re.search(pattern, text)
            if m is None:
                errors.append("%s: sentence not found (edited? update the check): %s" % (heading, pattern))
                continue
            for i, name in enumerate(names):
                if int(m.group(i + 1)) != consts[name]:
                    errors.append("%s: %r states %s, OCB_%s is %d" % (heading, m.group(0), m.group(i + 1), name,
                                                                     consts[name]))
        if heading.startswith("Rules") and named < 5:
            errors.append("%s: only %d values written next to a name" % (heading, named))
    return errors


def p2_config():
    text = read("firmware", "controller", "profile_p2", "include", "ocb_p2", "config.h")
    return {m.group(1): int(m.group(2), 0) for m in
            re.finditer(r"^#define OCB_P2_([A-Z0-9_]+) (0x[0-9A-Fa-f]+|\d+)u?\b", text, re.M)}


def check_p2(usb_doc, link_doc, p2, cfg):
    """Story 2.3: the P2 tables and the STATUS v3 sizes."""
    errors = []
    table = section(usb_doc, "Descriptors (D3, D4)")
    m = re.search(r"\| VID:PID \| `([0-9A-F]{4}):([0-9A-F]{4})`", table)
    if m is None or (int(m.group(1), 16), int(m.group(2), 16)) != (p2["VID"], p2["PID"]):
        errors.append("usb doc: VID:PID row differs from ocb_p2/config.h")
    m = re.search(r"\| `bcdDevice`, `bcdUSB`, `bMaxPacketSize0` \| (\d+)\.(\d+), (\d+)\.(\d+), (\d+) \|", table)
    if m is None or ((int(m.group(1)) << 8) | int(m.group(2), 16)) != p2["BCD_DEVICE"] or \
            int(m.group(5)) != p2["EP0_SIZE"]:
        errors.append("usb doc: bcdDevice / bMaxPacketSize0 row differs from ocb_p2/config.h")
    m = re.search(r"\| Configuration \| (\d+) bytes", table)
    if m is None or int(m.group(1)) != p2["CONFIG_DESC_LEN"]:
        errors.append("usb doc: configuration length differs from OCB_P2_CONFIG_DESC_LEN")
    m = re.search(r"report descriptor (\d+) bytes", table)
    if m is None or int(m.group(1)) != p2["REPORT_DESC_LEN"]:
        errors.append("usb doc: report descriptor length differs from OCB_P2_REPORT_DESC_LEN")
    m = re.search(r"\| `bInterval` \| \*\*(\d+)\*\* by default .*?declares (\d+)", table)
    if m is None or int(m.group(1)) != p2["BINTERVAL_DEFAULT"] or int(m.group(2)) != p2["BINTERVAL_REAL"]:
        errors.append("usb doc: bInterval row differs from OCB_P2_BINTERVAL_DEFAULT / _REAL")
    m = re.search(r"Every report is (\d+) bytes on the wire", table)
    if m is None or int(m.group(1)) != p2["REPORT_LEN"]:
        errors.append("usb doc: report length differs from OCB_P2_REPORT_LEN")
    rules = section(link_doc, "Rules the controller link applies (Story 2.6)")
    m = re.search(r"(\d+) bytes in all with an empty window, at most (\d+); a debug STATUS body is at most (\d+) "
                  r"bytes without the v4 block", rules)
    empty = cfg["LINK_DEBUG_COUNTERS_LEN"] + cfg["LINK_DEBUG_V3_HEADER_LEN"]
    if m is None or (int(m.group(1)), int(m.group(2)), int(m.group(3))) != (
            empty, cfg["LINK_DEBUG_V3_MAX_LEN"], 11 + cfg["LINK_DEBUG_V3_MAX_LEN"]):
        errors.append("link doc: STATUS v3 sizes differ from ocb_link/config.h (%d, %d, %d)" % (
            empty, cfg["LINK_DEBUG_V3_MAX_LEN"], 11 + cfg["LINK_DEBUG_V3_MAX_LEN"]))
    return errors


def live_values():
    text = read("firmware", "controller", "core", "include", "ocb_core", "live.h")
    return {m.group(1): int(m.group(2)) for m in re.finditer(r"^#define OCB_([A-Z0-9_]+) (\d+)u\b", text, re.M)}


def core_late_fallback():
    text = read("firmware", "controller", "core", "include", "ocb_core", "core.h")
    m = re.search(r"^#define OCB_CORE_LATE_FALLBACK_US (\d+)u\b", text, re.M)
    return int(m.group(1)) if m else None


def check_story_2_5(link_doc, cfg, live, late_fallback):
    """Story 2.5: the STATUS v4 sizes and table, the liveness numbers, the LATE fallback."""
    errors = []
    rules = section(link_doc, "Rules the controller link applies (Story 2.6)")
    v4 = cfg["LINK_DEBUG_V4_LEN"]
    empty3 = cfg["LINK_DEBUG_COUNTERS_LEN"] + cfg["LINK_DEBUG_V3_HEADER_LEN"]
    if cfg["LINK_DEBUG_V3_MAX_LEN"] + v4 != cfg["LINK_DEBUG_TAIL_MAX_LEN"]:
        errors.append("config.h: V3_MAX_LEN + V4_LEN != TAIL_MAX_LEN")
    m = re.search(r"one fixed block of (\d+) bytes", rules)
    if m is None or int(m.group(1)) != v4:
        errors.append("link doc: the v4 block size differs from OCB_LINK_DEBUG_V4_LEN (%d)" % v4)
    body = 11 + cfg["LINK_DEBUG_TAIL_MAX_LEN"]
    wire = 3 + body + 2 + 2  # header, CRC, one COBS byte (body < 254), the delimiter
    want = (empty3 + v4, cfg["LINK_DEBUG_TAIL_MAX_LEN"], body, wire)
    m = re.search(r"the debug tail is (\d+) bytes with an empty window, at most (\d+); the largest debug STATUS "
                  r"body is (\d+) bytes \((\d+) bytes on the wire\)", rules)
    if m is None or tuple(int(g) for g in m.groups()) != want:
        errors.append("link doc: STATUS v4 sizes differ from ocb_link/config.h (%d, %d, %d, %d)" % want)
    offsets = [int(x) for x in re.findall(r"^    \| (\d+) \| `", rules, re.M)]
    if offsets != [0, 2, 4, 8, 20, 22, 24, 30, 32, 34, 35, 37, 38]:
        errors.append("link doc: v4 table offsets %s differ from the 39-byte layout of config.h" % offsets)
    text = section(link_doc, "Rules the controller applies (Story 2.5)")
    checks = (
        (r"`OCB_LIVE_STALE_US` \((\d+) ms", live["LIVE_STALE_US"] // 1000, "OCB_LIVE_STALE_US"),
        (r"`OCB_WDT_TIMEOUT_MS`, (\d+) ms", live["WDT_TIMEOUT_MS"], "OCB_WDT_TIMEOUT_MS"),
        (r"one loop pass \(bound (\d+) ms\)", live["LIVE_LOOP_BOUND_US"] // 1000, "OCB_LIVE_LOOP_BOUND_US"),
        (r"\(bound \d+ ms\) = (\d+) ms", (live["LIVE_STALE_US"] + live["WDT_TIMEOUT_MS"] * 1000 +
                                         live["LIVE_LOOP_BOUND_US"]) // 1000, "the worst case sum"),
    )
    for rx, value, name in checks:
        m = re.search(rx, text)
        if m is None or int(m.group(1)) != value:
            errors.append("link doc (Story 2.5): %s written as %s, live.h gives %d" % (
                name, m.group(1) if m else "nothing", value))
    m = re.search(r"Story 2\.5 \(D14\) keeps the fallback at (\d+) µs", link_doc)
    if m is None or int(m.group(1)) != late_fallback:
        errors.append("link doc: the LATE fallback differs from OCB_CORE_LATE_FALLBACK_US (%s)" % late_fallback)
    return errors


def main():
    global REPO
    if len(sys.argv) != 2:
        sys.exit("usage: test_docs_constants.py REPO_DIR")
    REPO = sys.argv[1]
    errors = check(read("docs", "link-protocol.md"), constants(), config_values())
    errors += check_p2(read("docs", "usb-controller-emulation.md"), read("docs", "link-protocol.md"), p2_config(),
                       config_values())
    errors += check_story_2_5(read("docs", "link-protocol.md"), config_values(), live_values(), core_late_fallback())
    for e in errors:
        print("docs_constants_check: FAIL %s" % e)
    if errors:
        return 1
    print("docs_constants_check (F.22): ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
