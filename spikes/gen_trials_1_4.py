# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""Generate the Story 1.4 trial files (timing limits: U7 min press, U2 min gap, U5 modifier lead, row 2.4 jitter).

SPDX-License-Identifier: MIT (the project licence, see LICENSE at the repo root).

THROWAWAY SPIKE CODE. Not shipped, not part of app/ or firmware/. It writes
input files for spikes/send_2wicc.py into spikes/samples/story-1-4/. The output
is deterministic and committed, so the owner can run the trials without running
this script.

All times are in frames of the sender's 60 Hz grid (`12f`), never ms, so the
ms-to-frame rounding cannot blur a 1-frame step. Every row has a `# label`;
scored attempts start with `TRY` (machine-readable `key=value` fields that
listen_trial.py parses), the first attempt of every file is a `WARMUP`.

Hardware safety: the spike sender has no AD-9 mask, so the generators are the
only guard. Every row goes through gen_trials_1_3.check_allowed (L R A X Y,
D-pad up/down/neutral, left stick); anything else raises GuardError.

Usage: uv run spikes/gen_trials_1_4.py                       # round 1 files (U7, U2, U5)
       uv run spikes/gen_trials_1_4.py --key                 # also print the key
       uv run spikes/gen_trials_1_4.py --leads 0-5,6,8,10,12,15,20,30 --mods sup --out-dir DIR   # extension
       uv run spikes/gen_trials_1_4.py --set confirm --u7 3 --u2 2 --u5 dup=1,ddown=1,sup=4,sdown=4
       uv run spikes/gen_trials_1_4.py --set jitter --press-frames 3 --min-press 2 --min-gap 4
"""

from __future__ import annotations

import argparse
import importlib.util
import itertools
import pathlib
import random
import sys
from collections.abc import Sequence
from dataclasses import dataclass, field, replace

HERE = pathlib.Path(__file__).resolve().parent
DEFAULT_OUT = HERE / "samples" / "story-1-4"


def _load(name: str):
    if name in sys.modules:
        return sys.modules[name]
    spec = importlib.util.spec_from_file_location(name, HERE / f"{name}.py")
    mod = importlib.util.module_from_spec(spec)
    sys.modules[name] = mod
    spec.loader.exec_module(mod)
    return mod


g = _load("gen_trials_1_3")  # the allow-list guard, Stick and the pitch model are reused, not copied
GuardError = g.GuardError

FRAME_RATE = 60
FRAME_MS = 1000.0 / FRAME_RATE
MAX_PRESS_FRAMES = 120
AUTO_STOP_FRAMES = g.AUTO_STOP_MS * FRAME_RATE // 1000  # 360: a held note stops by itself (U1)
SEED = 1404

# The four modifiers of row 2.3 (name, hat, stick), base button A, lx 0x80.
MODS = {
    "dup": (g.HAT_UP, g.CENTRE),
    "ddown": (g.HAT_DOWN, g.CENTRE),
    "sup": (g.HAT_NEUTRAL, g.Stick(0x80, 0x00)),
    "sdown": (g.HAT_NEUTRAL, g.Stick(0x80, 0xFF)),
}
MOD_TEXT = {"dup": "D-up", "ddown": "D-down", "sup": "stick up 80:00", "sdown": "stick down 80:FF"}
MOD_MIDI = {"dup": 1, "ddown": -1, "sup": 2, "sdown": -2}
JITTER_CYCLE = ("L", "R", "A", "Y", "X", "Y", "A", "R")  # never the same button twice in a row


# ---------------------------------------------------------------------------
# Rows and attempts
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class Row:
    frame: int
    button: str  # "-" or one allowed name
    hat: int = g.HAT_NEUTRAL
    stick: g.Stick = g.CENTRE
    label: str = ""

    def __post_init__(self):
        # check_allowed needs a button; for a row without one only the hat and stick are checked.
        g.check_allowed(self.button if self.button != "-" else "A", self.hat, self.stick)
        if self.frame < 0:
            raise GuardError("negative frame")


@dataclass(frozen=True)
class Attempt:
    rows: tuple[Row, ...]  # frames relative to the attempt start
    length: int            # frames until the next attempt starts
    scored: bool
    expect: str            # model reading aid for the key, never a result


def _try_label(kind: str, button: str, fields: Sequence[str], rnd: tuple[int, int] | None, control: bool) -> str:
    parts = ["TRY", kind, button, *fields]
    if rnd is not None:
        parts.append(f"round {rnd[0]}/{rnd[1]}")
    if control:
        parts.append("control")
    return " ".join(parts)


def _note(button: str, offset: int = 0) -> str:
    return g.pitch_text(g.BUTTON_MIDI[button] + offset)


def u7_attempt(button: str, d: int | None, pre: int, post: int, rnd=None) -> Attempt:
    """Row 2.1: neutral `pre`, press `d` frames, neutral `post`. d=None is a blank (no press)."""
    if d is None:
        rows = (Row(0, "-", label="neutral"), Row(pre, "-", label=_try_label("u7", button, ["blank"], rnd, True)))
        return Attempt(rows, pre + post, True, "silence (blank control)")
    _min_frames(d, "press")
    rows = (Row(0, "-", label="neutral"),
            Row(pre, button, label=_try_label("u7", button, [f"press={d}f"], rnd, False)),
            Row(pre + d, "-", label="release"))
    return Attempt(rows, pre + d + post, True, f"one {_note(button)} if a {d}-frame press counts")


def u2_attempt(button: str, hold: int, gap: int, pre: int, post: int, rnd=None, control: bool = False) -> Attempt:
    """Row 2.2: press `hold`, release `gap`, press `hold` again. gap=0 is one press of 2*hold."""
    _min_frames(hold, "hold")
    label = _try_label("u2", button, [f"gap={gap}f", f"hold={hold}f"], rnd, control)
    if gap == 0:
        rows = (Row(0, "-", label="neutral"), Row(pre, button, label=label), Row(pre + 2 * hold, "-", label="release"))
        return Attempt(rows, pre + 2 * hold + post, True, "one note, no retrigger (control)")
    _min_frames(gap, "gap")
    rows = (Row(0, "-", label="neutral"), Row(pre, button, label=label),
            Row(pre + hold, "-", label=f"release, gap {gap}f"),
            Row(pre + hold + gap, button, label="second press"),
            Row(pre + 2 * hold + gap, "-", label="release"))
    return Attempt(rows, pre + 2 * hold + gap + post, True,
                   "two notes (control)" if control else f"two notes if a {gap}-frame gap retriggers")


def u5_attempt(button: str, mod: str, lead: int, note: int, tail: int, pre: int, post: int, rnd=None) -> Attempt:
    """Row 2.3: modifier set `lead` frames before the press, kept until `tail` frames after the release."""
    if lead < 0:
        raise GuardError("lead must be >= 0 frames")
    _min_frames(note, "note")
    _min_frames(tail, "tail")
    hat, stick = MODS[mod]
    label = _try_label("u5", button, [f"mod={mod}", f"lead={lead}f", f"note={note}f"], rnd, False)
    rows = [Row(0, "-", label="neutral")]
    if lead > 0:
        rows.append(Row(pre, "-", hat, stick, f"mod {mod} set, lead {lead}f"))
    press = pre + lead
    rows += [Row(press, button, hat, stick, label),
             Row(press + note, "-", hat, stick, "release, modifier kept"),
             Row(press + note + tail, "-", label="neutral")]
    m = g.BUTTON_MIDI[button] + MOD_MIDI[mod]
    return Attempt(tuple(rows), press + note + tail + post, True,
                   f"{g.pitch_text(m)} from the start if a {lead}-frame lead is enough")


def warmup(button: str, pre: int, post: int) -> Attempt:
    rows = (Row(0, "-", label="neutral"), Row(pre, button, label=f"WARMUP {button} press 12f (not scored)"),
            Row(pre + 12, "-", label="release"))
    return Attempt(rows, pre + 12 + post, False, "may be ignored (first input after a connect)")


def _min_frames(v: int, what: str) -> None:
    if not isinstance(v, int) or v < 1:
        raise GuardError(f"{what} must be at least 1 frame (got {v!r})")


# ---------------------------------------------------------------------------
# Options and files
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class Options:
    pre: int = 30
    post: int = 90
    rounds: int = 20
    rounds_per_file: int = 5
    button: str = "A"
    press_frames: tuple[int, ...] = tuple(range(1, 11))
    hold_frames: int = 12
    gap_frames: tuple[int, ...] = tuple(range(1, 11))
    gap_controls: tuple[int, int] = (0, 30)
    leads: tuple[int, ...] = tuple(range(6))
    mods: tuple[str, ...] = tuple(MODS)
    note_frames: int = 36
    tail: int = 6
    seed: int = SEED
    # confirm set
    buttons: tuple[str, ...] = ("L", "R", "Y", "X")
    confirm_u7: int | None = None
    confirm_u2: int | None = None
    confirm_u5: dict[str, int] = field(default_factory=dict)
    # jitter set
    jitter_press: int = 4
    min_press: int | None = None
    min_gap: int | None = None

    def validate(self) -> None:
        if self.pre < 30:
            raise GuardError("--pre must be >= 30 frames (the analyser needs a 0.45 s quiet window)")
        if self.post < 30:
            raise GuardError("--post must be >= 30 frames (the note must die away)")
        if self.rounds < 1 or self.rounds_per_file < 1:
            raise GuardError("--rounds and --rounds-per-file must be >= 1")
        for b in (self.button, *self.buttons):
            g.check_allowed(b, g.HAT_NEUTRAL, g.CENTRE)
        for d in (*self.press_frames, self.jitter_press, self.hold_frames):
            _min_frames(d, "press/hold")
            if d > MAX_PRESS_FRAMES:
                raise GuardError(f"press/hold {d}f is above {MAX_PRESS_FRAMES} frames")
        for v in self.gap_frames:
            _min_frames(v, "gap")
        if not self.press_frames or not self.gap_frames or not self.leads or not self.mods:
            raise GuardError("--press-frames, --gap-frames, --leads and --mods need at least one value")
        if any(not 0 <= k <= 60 for k in self.leads):
            raise GuardError("--leads must be 0..60 frames")
        if any(m not in MODS for m in self.mods):
            raise GuardError(f"--mods: only {','.join(MODS)}")
        _min_frames(self.tail, "tail")
        if not 1 <= self.note_frames < AUTO_STOP_FRAMES:
            raise GuardError(f"--note-frames must be 1..{AUTO_STOP_FRAMES - 1} (U1 auto-stop at about 6 s)")
        if 2 * self.hold_frames + max(self.gap_controls) >= AUTO_STOP_FRAMES:
            raise GuardError("--hold-frames too long (U1 auto-stop)")


@dataclass(frozen=True)
class TrialFile:
    name: str
    title: str
    attempts: tuple[Attempt, ...]


def _rounds(steps: Sequence, opts: Options, family: str, make) -> list[list[Attempt]]:
    """`opts.rounds` rounds; each holds every step once in a seeded shuffled order."""
    rng = random.Random(f"{opts.seed}-{family}")
    out = []
    for r in range(1, opts.rounds + 1):
        order = list(steps)
        rng.shuffle(order)
        out.append([make(step, (r, opts.rounds)) for step in order])
    return out


def _split(prefix: str, title: str, rounds: list[list[Attempt]], opts: Options, button: str) -> list[TrialFile]:
    files = []
    chunks = [rounds[i: i + opts.rounds_per_file] for i in range(0, len(rounds), opts.rounds_per_file)]
    for k, chunk in enumerate(chunks, start=1):
        first = (k - 1) * opts.rounds_per_file + 1
        attempts = [warmup(button, opts.pre, opts.post)] + [a for rnd in chunk for a in rnd]
        files.append(TrialFile(f"{prefix}-part{k}.txt",
                               f"{title}, rounds {first}-{first + len(chunk) - 1} of {len(rounds)}", tuple(attempts)))
    return files


def u7_files(opts: Options, button: str | None = None, prefix: str = "u7-minpress",
             presses: Sequence[int] | None = None) -> list[TrialFile]:
    b = button or opts.button
    steps = [*(presses or opts.press_frames), None]
    rounds = _rounds(steps, opts, f"{prefix}-{b}", lambda d, rnd: u7_attempt(b, d, opts.pre, opts.post, rnd))
    return _split(prefix, f"row 2.1 U7 min press: {b}, press 1 step per attempt plus a blank control", rounds, opts, b)


def u2_files(opts: Options, button: str | None = None, prefix: str = "u2-gap",
             gaps: Sequence[int] | None = None) -> list[TrialFile]:
    b = button or opts.button
    controls = set(opts.gap_controls)
    steps = [*(gaps or opts.gap_frames), *opts.gap_controls]
    rounds = _rounds(steps, opts, f"{prefix}-{b}",
                     lambda gap, rnd: u2_attempt(b, opts.hold_frames, gap, opts.pre, opts.post, rnd, gap in controls))
    return _split(prefix, f"row 2.2 U2 min gap same button: {b}, hold {opts.hold_frames}f, gap controls "
                          f"{opts.gap_controls[0]} and {opts.gap_controls[1]}", rounds, opts, b)


def u5_files(opts: Options, mod: str, button: str | None = None, prefix: str | None = None,
             leads: Sequence[int] | None = None) -> list[TrialFile]:
    b = button or opts.button
    prefix = prefix or f"u5-lead-{mod}"
    rounds = _rounds(list(leads if leads is not None else opts.leads), opts, f"{prefix}-{b}",
                     lambda k, rnd: u5_attempt(b, mod, k, opts.note_frames, opts.tail, opts.pre, opts.post, rnd))
    return _split(prefix, f"row 2.3 U5 modifier lead: {b} with {MOD_TEXT[mod]}, note {opts.note_frames}f",
                  rounds, opts, b)


def round1(opts: Options) -> list[TrialFile]:
    files = u7_files(opts) + u2_files(opts)
    for mod in opts.mods:
        files += u5_files(opts, mod)
    return files


def confirm(opts: Options) -> list[TrialFile]:
    """Task 1.9: threshold and one step below on each button, one file per value, 20 tries each."""
    if opts.confirm_u7 is None and opts.confirm_u2 is None and not opts.confirm_u5:
        raise GuardError("--set confirm needs at least one of --u7, --u2, --u5 (the thresholds found on A)")
    one_file = replace(opts, rounds_per_file=opts.rounds)
    files: list[TrialFile] = []
    for b in opts.buttons:
        if opts.confirm_u7 is not None:
            for d in (opts.confirm_u7, opts.confirm_u7 - 1):
                if d >= 1:
                    files += _single(u7_files(one_file, b, f"confirm-u7-{b}-{d}f", [d]))
        if opts.confirm_u2 is not None:
            for gap in (opts.confirm_u2, opts.confirm_u2 - 1):
                if gap >= 1 and gap not in opts.gap_controls:
                    files += _single(u2_files(one_file, b, f"confirm-u2-{b}-{gap}f", [gap]))
        for mod, k0 in opts.confirm_u5.items():
            for k in (k0, k0 - 1):
                if k >= 0:
                    files += _single(u5_files(one_file, mod, b, f"confirm-u5-{mod}-{b}-{k}f", [k]))
    return files


def _single(files: list[TrialFile]) -> list[TrialFile]:
    (tf,) = files
    return [replace(tf, name=tf.name.replace("-part1.txt", ".txt"))]


def jitter_onsets(n: int = 240) -> list[int]:
    """Onset frames of 120 BPM sixteenths (7.5 frames): 0, 8, 15, 23, 30, ... (the 7/8 sender grid)."""
    return [(15 * k + 1) // 2 for k in range(n)]


def jitter_file(opts: Options, lead_in: int = 120, tail: int = 60) -> TrialFile:
    """Row 2.4: 30 s of 120 BPM 16ths cycling L R A Y X Y A R, no modifiers."""
    on = jitter_onsets()
    p = opts.jitter_press
    gaps = [b - a - p for a, b in itertools.pairwise(on)]
    if min(gaps) < 1:
        raise GuardError(f"--press-frames {p}: no release gap left in a 7-frame slot")
    if opts.min_press is not None and p < opts.min_press:
        raise GuardError(f"press {p}f is below --min-press {opts.min_press}f (U7)")
    if opts.min_gap is not None and min(gaps) < opts.min_gap:
        raise GuardError(f"release gap {min(gaps)}f is below --min-gap {opts.min_gap}f (U2)")
    rows = [Row(0, "-", label="neutral lead-in")]
    for k, f in enumerate(on):
        b = JITTER_CYCLE[k % len(JITTER_CYCLE)]
        rows.append(Row(lead_in + f, b, label=f"TRY jit {b} n={k + 1}/{len(on)} press={p}f"))
        rows.append(Row(lead_in + f + p, "-", label="release"))
    end = rows[-1].frame + tail
    rows.append(Row(end, "-", label="neutral end"))
    att = Attempt(tuple(rows), end, True, "")
    return TrialFile("jitter-120bpm-16ths.txt",
                     f"row 2.4 jitter: 120 BPM 16ths for 30 s ({len(on)} notes, slots of 7/8 frames), press {p}f",
                     (att,))


def build(opts: Options, which: str = "round1") -> list[TrialFile]:
    opts.validate()
    if which == "round1":
        return round1(opts)
    if which == "confirm":
        return confirm(opts)
    if which == "jitter":
        return [jitter_file(opts)]
    raise GuardError(f"unknown set {which!r}")


# ---------------------------------------------------------------------------
# Rendering
# ---------------------------------------------------------------------------


def file_rows(tf: TrialFile) -> list[Row]:
    """Absolute rows of a file; a final neutral row closes the last attempt's `post`."""
    out, t = [], 0
    for a in tf.attempts:
        out += [replace(r, frame=t + r.frame) for r in a.rows]
        t += a.length
    if out[-1].frame < t:
        out.append(Row(t, "-", label="end"))
    return out


def duration_frames(tf: TrialFile) -> int:
    return file_rows(tf)[-1].frame


def scored(tf: TrialFile) -> int:
    return sum(1 for r in file_rows(tf) if r.label.startswith("TRY"))


def render(tf: TrialFile) -> str:
    rows = file_rows(tf)
    secs = duration_frames(tf) / FRAME_RATE
    out = [
        f"# Story 1.4 trial: {tf.title}.",
        "# GENERATED by spikes/gen_trials_1_4.py; do not edit by hand, regenerate instead.",
        "# Run in ocarina free play only (open it with D-pad left on the real controller, wait 2 s).",
        "# Allow-list: buttons L R A X Y, D-pad up/down, left stick. Never add B, ZL, ZR, D-pad left",
        "# or a diagonal D-pad, or the right stick. Stop with Ctrl+C in a real terminal.",
        f"# {scored(tf)} scored attempts (TRY rows) plus a WARMUP, {duration_frames(tf)} frames, about {secs:.1f} s.",
        "# Times are frames of the sender's 60 Hz grid.",
        "# time   buttons  hat      stick    label",
    ]
    for r in rows:
        out.append(f"{str(r.frame) + 'f':<8} {r.button:<8} {g.ALLOWED_HATS[r.hat]:<8} {r.stick.token():<8} # {r.label}")
    return "\n".join(out) + "\n"


def key_lines(tf: TrialFile) -> list[str]:
    out = [f"{tf.name}: {scored(tf)} attempts, {duration_frames(tf) / FRAME_RATE:.1f} s"]
    t = 0
    for a in tf.attempts:
        for r in a.rows:
            if r.label.startswith(("TRY", "WARMUP")) and a.expect:
                out.append(f"  t={(t + r.frame) / FRAME_RATE:7.2f}s  {r.label:<52}  model: {a.expect}")
        t += a.length
    return out


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------


def frame_list(tok: str) -> tuple[int, ...]:
    """'1-10' or '0-5,6,8' -> ints (frames)."""
    out: list[int] = []
    try:
        for part in tok.split(","):
            a, sep, b = part.partition("-")
            lo, hi = int(a), int(b) if sep else int(a)
            if hi < lo:
                raise ValueError
            out += range(lo, hi + 1)
    except ValueError:
        raise argparse.ArgumentTypeError(f"bad frame list {tok!r} (like 1-10 or 0-5,6,8)") from None
    if any(v < 0 for v in out):
        raise argparse.ArgumentTypeError(f"negative value in {tok!r}")
    return tuple(out)


def name_list(tok: str) -> tuple[str, ...]:
    return tuple(p.strip() for p in tok.split(",") if p.strip())


def mod_values(tok: str) -> dict[str, int]:
    out = {}
    for part in tok.split(","):
        k, sep, v = part.partition("=")
        if not sep or k not in MODS or not v.isdigit():
            raise argparse.ArgumentTypeError(f"bad --u5 {tok!r} (like dup=1,ddown=1,sup=4,sdown=4)")
        out[k] = int(v)
    return out


def build_parser() -> argparse.ArgumentParser:
    d = Options()
    p = argparse.ArgumentParser(prog="gen_trials_1_4.py",
                                description="Throwaway spike: write the Story 1.4 trial files for send_2wicc.py.")
    p.add_argument("--set", default="round1", choices=("round1", "confirm", "jitter"),
                   help="round1: U7/U2/U5 sweeps (default); confirm: other buttons (Task 9); jitter: row 2.4")
    p.add_argument("--out-dir", default=str(DEFAULT_OUT), help="output folder (default spikes/samples/story-1-4)")
    p.add_argument("--key", action="store_true", help="also print the key (attempt, time, model expectation)")
    p.add_argument("--pre", type=int, default=d.pre, help="neutral frames before each attempt (30)")
    p.add_argument("--post", type=int, default=d.post, help="neutral frames after each attempt (90)")
    p.add_argument("--rounds", type=int, default=d.rounds, help="tries per step, one per round (20)")
    p.add_argument("--rounds-per-file", type=int, default=d.rounds_per_file, help="rounds per file (5)")
    p.add_argument("--button", default=d.button, help="sweep button, round1 (A)")
    p.add_argument("--press-frames", type=frame_list,
                   help="round1: U7 press lengths (1-10); jitter: the single press length (4)")
    p.add_argument("--hold-frames", type=int, default=d.hold_frames, help="U2 hold of each press (12)")
    p.add_argument("--gap-frames", type=frame_list, default=d.gap_frames, help="U2 release gaps (1-10)")
    p.add_argument("--leads", type=frame_list, default=d.leads, help="U5 modifier leads (0-5)")
    p.add_argument("--mods", type=name_list, default=d.mods, help="U5 families (dup,ddown,sup,sdown)")
    p.add_argument("--note-frames", type=int, default=d.note_frames, help="U5 note length (36)")
    p.add_argument("--tail", type=int, default=d.tail, help="U5 modifier kept after the release (6)")
    p.add_argument("--buttons", type=name_list, default=d.buttons, help="confirm: buttons (L,R,Y,X)")
    p.add_argument("--u7", type=int, help="confirm: U7 threshold found on A (frames)")
    p.add_argument("--u2", type=int, help="confirm: U2 threshold found on A (frames)")
    p.add_argument("--u5", type=mod_values, default={}, help="confirm: U5 thresholds, like dup=1,ddown=1,sup=4,sdown=4")
    p.add_argument("--min-press", type=int, help="jitter: refuse a press below this (measured U7)")
    p.add_argument("--min-gap", type=int, help="jitter: refuse a release gap below this (measured U2)")
    return p


def options_from_args(a: argparse.Namespace) -> Options:
    kw = {"pre": a.pre, "post": a.post, "rounds": a.rounds, "rounds_per_file": a.rounds_per_file,
          "button": a.button, "hold_frames": a.hold_frames, "gap_frames": a.gap_frames, "leads": a.leads,
          "mods": a.mods, "note_frames": a.note_frames, "tail": a.tail, "buttons": a.buttons,
          "confirm_u7": a.u7, "confirm_u2": a.u2, "confirm_u5": a.u5, "min_press": a.min_press,
          "min_gap": a.min_gap}
    if a.press_frames is not None:
        if a.set == "jitter":
            if len(a.press_frames) != 1:
                raise GuardError("--set jitter takes one --press-frames value")
            kw["jitter_press"] = a.press_frames[0]
        else:
            kw["press_frames"] = a.press_frames
    return Options(**kw)


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        files = build(options_from_args(args), args.set)
    except GuardError as e:
        print(f"error: {e}", file=sys.stderr)
        return 1
    out_dir = pathlib.Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    for tf in files:
        (out_dir / tf.name).write_text(render(tf), encoding="utf-8", newline="\n")
        n = duration_frames(tf)
        print(f"{tf.name:<32} {scored(tf):>4} attempts  {n:>6} frames  {n / FRAME_RATE:6.1f} s")
    if args.key:
        print("\nKey (model expectations are a reading aid, not results):")
        for tf in files:
            print()
            print("\n".join(key_lines(tf)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
