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


def main():
    global REPO
    if len(sys.argv) != 2:
        sys.exit("usage: test_docs_constants.py REPO_DIR")
    REPO = sys.argv[1]
    errors = check(read("docs", "link-protocol.md"), constants(), config_values())
    for e in errors:
        print("docs_constants_check: FAIL %s" % e)
    if errors:
        return 1
    print("docs_constants_check (F.22): ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
