# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Generate the Story 1.6 trial files (walking skeleton melody, U1 sustain trial).

SPDX-License-Identifier: MIT (the project licence, see LICENSE at the repo root).

THROWAWAY SPIKE CODE. Not shipped, not part of app/ or firmware/, and NOT a
precursor of the Epic 5 pipeline (Story 5.5 writes the real timeline compiler).
It writes input files for spikes/send_2wicc.py into spikes/samples/story-1-6/.
The output is deterministic and committed, so the owner can run the trials
without running this script.

Python 3.11+ (tomllib): every fingering and timing value is read from the
profile TOML (app/profiles/oot-switch2-1.1.1.toml), never copied into this code.

Melody files: a note list (`<sounding pitch> <beats>`, see the .melody file) is
turned into timed controller states on the profile's frame grid:
  - fingering = the profile's `chart = true` row for the pitch (no search, no transpose);
  - onsets are rounded from cumulative beats, so rounding error does not add up;
  - "steal time from the previous note, never shift onsets": the previous press
    ends min_gap_same_button_frames before a same-button note, modifier_lead_frames
    before a note whose D-pad/stick state changes, and at the onset otherwise;
  - a press longer than max_sustain_ms is re-pressed after the min gap (FR-05a).
Every note row is labelled `NOTE <index> <name> <midi> fingering=<...>` for
listen_trial.py --mode melody.

U1 files: long holds (up to 12 s) on one button, `TRY u1 ...` rows for
listen_trial.py --mode sustain. These holds are deliberately LONGER than the
6 s auto-stop guard of gen_trials_1_3.py / gen_trials_1_4.py: measuring the
auto-stop is the point. Their own bound: hold at most U1_MAX_HOLD_MS, buttons
L R A Y X only, no modifiers.

Hardware safety: the spike sender has no AD-9 mask, so the generators are the
only guard. Every state goes through gen_trials_1_3.check_allowed (L R A X Y,
D-pad up/down/neutral, left stick); anything else raises GuardError.

Usage: uv run --python 3.12 spikes/gen_trials_1_6.py                  # committed set: melody 60/90/120 BPM, U1 round 1
       uv run --python 3.12 spikes/gen_trials_1_6.py --set melody --bpm 90 --key
       uv run --python 3.12 spikes/gen_trials_1_6.py --set confirm --u1-confirm-ms 5500
"""

from __future__ import annotations

import argparse
import importlib.util
import math
import pathlib
import random
import re
import sys
import tomllib
from collections.abc import Sequence
from dataclasses import dataclass, field
from fractions import Fraction

HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parent
DEFAULT_OUT = HERE / "samples" / "story-1-6"
DEFAULT_PROFILE = REPO / "app" / "profiles" / "oot-switch2-1.1.1.toml"
DEFAULT_MELODY = DEFAULT_OUT / "ode-to-joy.melody"


def _load(name: str):
    if name in sys.modules:
        return sys.modules[name]
    spec = importlib.util.spec_from_file_location(name, HERE / f"{name}.py")
    mod = importlib.util.module_from_spec(spec)
    sys.modules[name] = mod
    spec.loader.exec_module(mod)
    return mod


g = _load("gen_trials_1_3")  # allow-list guard, Stick, hat constants, note names (reused, not copied)
GuardError = g.GuardError

# Story 1.6 WORKAROUND (owner decision 5, 2026-11-21), spike-level, NOT a profile value: no melody press is
# longer than this (staccato: a longer note is a 400 ms press, then silence until the next onset). Reason: an
# unexplained game-side event, about every 7th sounded note, ends a held note 0.54 to 0.91 s after its onset and
# ignores input for up to about 1 s (docs/test-plan.md, "Walking skeleton (Story 1.6)"; hypothesis, mechanism
# unknown). With presses of at most 400 ms no audible event in 1 full 60 BPM run and 1 short run (notes 1-8),
# and the 3 official 90 BPM runs were 62/62 (2026-11-21). Follow-up for the real pipeline: re-press
# long notes instead (owner's preferred direction); whether a value belongs in the profile is an owner call.
# The U1 files keep their long holds (their purpose). `--max-press-ms 0` writes an uncapped melody file.
DEFAULT_MAX_PRESS_MS = 400
COMMITTED_BPMS = (60, 90, 120)  # 60 by ear, 90 main run, 120 optional stress run (owner decision 4)
MIN_BPM = 30          # sanity bound: below it a half note is 4 s or more, near the U1 auto-stop
END_TAIL_FRAMES = 60  # neutral after the last release (file layout, not a profile value)
U1_MAX_HOLD_MS = 12000  # this generator's own bound for the U1 holds (see the module docstring)
U1_BUTTONS = ("L", "R", "A", "Y", "X")
SEED = 1606

# ---------------------------------------------------------------------------
# Profile (read from the TOML; comments are not parsed)
# ---------------------------------------------------------------------------

DPAD_HAT = {"none": g.HAT_NEUTRAL, "up": g.HAT_UP, "down": g.HAT_DOWN}


@dataclass(frozen=True)
class Fingering:
    button: str
    dpad: str   # none, up, down
    stick: str  # none, up, down
    pitch: int
    chart: bool

    def text(self) -> str:
        parts = [self.button]
        if self.dpad != "none":
            parts.append(f"D-{self.dpad}")
        if self.stick != "none":
            parts.append(f"stick-{self.stick}")
        return "+".join(parts)


@dataclass(frozen=True)
class Profile:
    path: pathlib.Path
    frame_rate: int
    buttons: dict
    modifiers: dict
    low: int
    high: int
    timing: dict
    fingerings: tuple[Fingering, ...]

    @property
    def min_press(self) -> int:
        return self.timing["min_press_frames"]

    @property
    def min_gap(self) -> int:
        return self.timing["min_gap_same_button_frames"]

    @property
    def lead(self) -> int:
        return self.timing["modifier_lead_frames"]

    @property
    def max_sustain_ms(self) -> int:
        return self.timing["max_sustain_ms"]

    @property
    def max_sustain_frames(self) -> int:
        """Longest press (frames) that is not longer than max_sustain_ms."""
        return self.max_sustain_ms * self.frame_rate // 1000

    @property
    def start_lead_frames(self) -> int:
        return round_half_up(Fraction(self.timing["start_lead_ms"] * self.frame_rate, 1000))

    def mods(self, f: Fingering) -> tuple[int, g.Stick]:
        """Canonical (hat, stick) of a fingering (AD-10)."""
        m = self.modifiers
        if f.stick == "up":
            stick = g.Stick(m["stick_lx"], m["stick_up_ly"])
        elif f.stick == "down":
            stick = g.Stick(m["stick_lx"], m["stick_down_ly"])
        else:
            stick = g.CENTRE
        return DPAD_HAT[f.dpad], stick

    def model_pitch(self, button: str, dpad: str, stick: str) -> int:
        """Button pitch + D-pad offset + stick offset, from the profile."""
        m = self.modifiers
        off = {"none": 0, "up": m["dpad_up"], "down": m["dpad_down"]}[dpad]
        off += {"none": 0, "up": m["stick_up"], "down": m["stick_down"]}[stick]
        return self.buttons[button] + off

    def chart_fingering(self, midi: int, what: str) -> Fingering:
        if not self.low <= midi <= self.high:
            raise GuardError(f"{what}: pitch {g.midi_name(midi)} ({midi}) is outside the profile range "
                             f"{g.midi_name(self.low)}..{g.midi_name(self.high)} ({self.low}..{self.high})")
        rows = [f for f in self.fingerings if f.pitch == midi and f.chart]
        if len(rows) != 1:
            raise GuardError(f"{what}: pitch {g.midi_name(midi)} ({midi}) has {len(rows)} chart fingerings in the "
                             "profile (need exactly one)")
        return rows[0]

    def values_text(self) -> str:
        t = self.timing
        return (f"frame_rate {self.frame_rate}, min_press_frames {t['min_press_frames']}, "
                f"min_gap_same_button_frames {t['min_gap_same_button_frames']}, "
                f"modifier_lead_frames {t['modifier_lead_frames']}, max_sustain_ms {t['max_sustain_ms']}, "
                f"start_lead_ms {t['start_lead_ms']}")


def load_profile(path: pathlib.Path = DEFAULT_PROFILE) -> Profile:
    with open(path, "rb") as fh:
        d = tomllib.load(fh)
    fings = tuple(Fingering(f["button"], f["dpad"], f["stick"], f["pitch"], f["chart"]) for f in d["fingerings"])
    return Profile(pathlib.Path(path), d["profile"]["frame_rate"], dict(d["buttons"]), dict(d["modifiers"]),
                   d["range"]["low"], d["range"]["high"], dict(d["timing"]), fings)


def rel(path: pathlib.Path) -> str:
    try:
        return pathlib.Path(path).resolve().relative_to(REPO).as_posix()
    except ValueError:
        return pathlib.Path(path).name


# ---------------------------------------------------------------------------
# Melody file
# ---------------------------------------------------------------------------

_PITCH_RE = re.compile(r"([A-Ga-g])([#b]?)(-?\d+)$")
_COMMENT_RE = re.compile(r"(?:^|\s)#")
_PC = {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}


def parse_pitch(tok: str) -> int:
    m = _PITCH_RE.fullmatch(tok)
    if not m:
        raise GuardError(f"bad pitch {tok!r} (like B5, C#6, Bb5)")
    pc = _PC[m.group(1).upper()] + {"": 0, "#": 1, "b": -1}[m.group(2)]
    return (int(m.group(3)) + 1) * 12 + pc


@dataclass(frozen=True)
class Item:
    line: int
    midi: int | None  # None = rest
    beats: Fraction


def parse_melody(text: str) -> list[Item]:
    items = []
    for n, raw in enumerate(text.splitlines(), start=1):
        body = _COMMENT_RE.split(raw, maxsplit=1)[0].strip()  # `#` starts a comment only at a token start (C#6)
        if not body:
            continue
        toks = body.split()
        if len(toks) != 2:
            raise GuardError(f"melody line {n}: need `<pitch> <beats>` or `R <beats>`")
        try:
            beats = Fraction(toks[1])
        except (ValueError, ZeroDivisionError):
            raise GuardError(f"melody line {n}: bad beats {toks[1]!r}") from None
        if beats <= 0:
            raise GuardError(f"melody line {n}: beats must be > 0")
        midi = None if toks[0].upper() == "R" else parse_pitch(toks[0])
        items.append(Item(n, midi, beats))
    if not any(i.midi is not None for i in items):
        raise GuardError("melody has no notes")
    return items


# ---------------------------------------------------------------------------
# Melody -> planned presses -> timeline
# ---------------------------------------------------------------------------


def round_half_up(x: Fraction) -> int:
    return math.floor(x + Fraction(1, 2))


def beat_frames(bpm: Fraction, frame_rate: int) -> Fraction:
    return Fraction(60 * frame_rate) / bpm


@dataclass(frozen=True)
class Planned:
    index: int              # 1-based among the sounding notes
    line: int               # melody file line
    midi: int
    fingering: Fingering
    hat: int
    stick: g.Stick
    onset: int              # frame of the press (on the cumulative grid)
    release: int            # frame of the final release
    mods_at: int            # frame the modifiers are set (onset - lead when they change, else onset)
    segments: tuple[tuple[int, int], ...]  # [start, end) of each press (more than one = re-press)
    contiguous: bool        # the next melody item is a note (no rest between): modifiers carry over

    @property
    def name(self) -> str:
        return g.midi_name(self.midi)

    @property
    def mods(self) -> tuple[int, g.Stick]:
        return self.hat, self.stick

    @property
    def label(self) -> str:
        return f"NOTE {self.index:02d} {self.name} {self.midi} fingering={self.fingering.text()}"

    def pressed_frames(self) -> int:
        return sum(b - a for a, b in self.segments)


NEUTRAL_MODS = (g.HAT_NEUTRAL, g.CENTRE)


def split_sustain(start: int, end: int, max_frames: int, gap: int) -> tuple[tuple[int, int], ...]:
    """Split a press [start, end) into the fewest presses of at most `max_frames`, `gap` frames apart (FR-05a)."""
    if max_frames < 1:
        raise GuardError(f"max_sustain_ms gives {max_frames} frame(s) per press; it must be at least one frame")
    length = end - start
    if length <= max_frames:
        return ((start, end),)
    n = 2
    while n * max_frames + (n - 1) * gap < length:
        n += 1
    total = length - (n - 1) * gap
    base, extra = divmod(total, n)
    segs, t = [], start
    for k in range(n):
        d = base + (1 if k < extra else 0)
        segs.append((t, t + d))
        t += d + gap
    return tuple(segs)


def plan(items: Sequence[Item], prof: Profile, bpm: Fraction, extra_lead: int = 0,
         first_index: int = 1, max_press: int | None = None) -> tuple[list[Planned], int]:
    """Planned presses and the frame where the melody ends (the cumulative grid). `extra_lead` frames are
    added before the first press and `first_index` numbers the notes (diagnostic subsets only)."""
    if bpm < MIN_BPM:
        raise GuardError(f"--bpm {bpm} is below the sane bound {MIN_BPM}")
    bf = beat_frames(bpm, prof.frame_rate)
    lead_in = prof.start_lead_frames + extra_lead
    cum, onsets = Fraction(0), []
    for it in items:
        onsets.append(lead_in + round_half_up(cum * bf))
        cum += it.beats
    end = lead_in + round_half_up(cum * bf)
    nexts = onsets[1:] + [end]

    raw = []  # (index, item, fingering, mods, onset, next item's onset, contiguous with the next note)
    for k, it in enumerate(items):
        if it.midi is None:
            continue
        idx = len(raw) + first_index
        f = prof.chart_fingering(it.midi, f"note {idx:02d} (melody line {it.line})")
        if prof.model_pitch(f.button, f.dpad, f.stick) != it.midi:
            raise GuardError(f"note {idx:02d}: profile fingering {f.text()} does not give pitch {it.midi}")
        g.check_allowed(f.button, *prof.mods(f))
        contiguous = k + 1 < len(items) and items[k + 1].midi is not None
        raw.append((idx, it, f, prof.mods(f), onsets[k], nexts[k], contiguous))

    # Modifier set frame of every note: lead frames early when they differ from the state before it
    # (the previous note's modifiers if contiguous, neutral after a rest or at the start).
    mods_at = []
    for j, (idx, it, f, mods, onset, _nxt, _c) in enumerate(raw):
        before = raw[j - 1][3] if j > 0 and raw[j - 1][6] else NEUTRAL_MODS
        mods_at.append(onset - prof.lead if mods != before else onset)
        if mods_at[-1] < 0:
            raise GuardError(f"note {idx:02d}: its modifiers would be set before the start of the file (lead-in "
                             f"{onset} frame(s) < modifier_lead_frames {prof.lead}); raise start_lead_ms or --extra-lead-ms")

    out = []
    for j, (idx, it, f, mods, onset, nxt, contiguous) in enumerate(raw):
        release = nxt
        if j + 1 < len(raw):
            _, _, nf, _, n_on, _, _ = raw[j + 1]
            release = min(release, mods_at[j + 1])
            if nf.button == f.button:
                release = min(release, n_on - prof.min_gap)
        if max_press is not None:
            release = min(release, onset + max_press)  # diagnostic: no press longer than this (staccato)
        what = f"note {idx:02d} ({g.midi_name(it.midi)}, {f.text()})"
        if mods_at[j] < (out[-1].release if out else 0):
            raise GuardError(f"{what}: modifiers would change while note {idx - 1:02d} still sounds (U4)")
        if prof.max_sustain_frames < prof.min_press:
            raise GuardError(f"max_sustain_ms {prof.max_sustain_ms} is shorter than min_press_frames {prof.min_press}")
        segs = split_sustain(onset, release, prof.max_sustain_frames, prof.min_gap)
        for a, b in segs:
            if b - a < prof.min_press:
                raise GuardError(f"{what}: press of {b - a} frame(s) is shorter than min_press_frames "
                                 f"{prof.min_press} (tempo too fast?)")
        if j + 1 < len(raw) and raw[j + 1][2].button == f.button and raw[j + 1][4] - release < prof.min_gap:
            raise GuardError(f"{what}: gap before the same button is shorter than min_gap_same_button_frames "
                             f"{prof.min_gap}")
        out.append(Planned(idx, it.line, it.midi, f, mods[0], mods[1], onset, release, mods_at[j], segs, contiguous))
    return out, end


@dataclass(frozen=True)
class Row:
    frame: int
    button: str  # "-" or one allowed name
    hat: int = g.HAT_NEUTRAL
    stick: g.Stick = g.CENTRE
    label: str = ""

    def __post_init__(self):
        g.check_allowed(self.button if self.button != "-" else "A", self.hat, self.stick)
        if self.frame < 0:
            raise GuardError("negative frame")


def timeline(notes: Sequence[Planned], end: int) -> list[tuple[str, int, g.Stick]]:
    """One (button, hat, stick) per frame up to `end` (exclusive)."""
    n = max(end, max(p.release for p in notes))
    mods = [NEUTRAL_MODS] * n
    btn = ["-"] * n
    for j, p in enumerate(notes):
        nxt = notes[j + 1] if j + 1 < len(notes) else None
        # modifiers stay set through a release gap until the next note's own set frame; a rest resets them
        mods_end = nxt.mods_at if nxt is not None and p.contiguous else p.release
        for f in range(p.mods_at, mods_end):
            mods[f] = p.mods
        for a, b in p.segments:
            for f in range(a, b):
                btn[f] = p.fingering.button
    return [(btn[f], *mods[f]) for f in range(n)]


def melody_rows(notes: Sequence[Planned], end: int) -> list[Row]:
    tl = timeline(notes, end)
    starts = {p.onset: p for p in notes}
    represses = {a: p for p in notes for a, _ in p.segments[1:]}
    rows = [Row(0, "-", label="neutral lead-in")]
    prev = ("-", *NEUTRAL_MODS)
    for f, st in enumerate(tl):
        if st == prev:
            continue
        b, hat, stick = st
        if f in starts:
            label = starts[f].label
        elif f in represses:
            label = f"REPRESS {represses[f].index:02d} {represses[f].name} (press longer than max_sustain_ms)"
        else:
            parts = []
            if prev[0] != "-" and b == "-":
                parts.append("release")
            if (hat, stick) != prev[1:]:
                nxt = next((p for p in notes if p.mods_at == f), None)
                parts.append(f"modifiers for note {nxt.index:02d}" if nxt else "modifiers neutral")
            label = ", ".join(parts) or "state change"
        rows.append(Row(f, b, hat, stick, label))
        prev = st
    rows.append(Row(len(tl), "-", label="release, neutral" if prev != ("-", *NEUTRAL_MODS) else "melody end"))
    rows.append(Row(len(tl) + END_TAIL_FRAMES, "-", label="end"))
    return rows


# ---------------------------------------------------------------------------
# Timeline checks (independent of the planner; the tests run them on every file)
# ---------------------------------------------------------------------------


def press_runs(rows: Sequence[Row]) -> list[tuple[str, int, int, tuple]]:
    """(button, start, end, modifiers) of every continuous press of one button."""
    runs = []
    for i, r in enumerate(rows[:-1]):
        nxt = rows[i + 1].frame
        if r.button == "-":
            continue
        if runs and runs[-1][0] == r.button and runs[-1][2] == r.frame:
            b, a, _, m = runs[-1]
            if m != (r.hat, r.stick):
                raise GuardError(f"frame {r.frame}: modifiers change while {b} is held (U4 bend)")
            runs[-1] = (b, a, nxt, m)
        else:
            runs.append((r.button, r.frame, nxt, (r.hat, r.stick)))
    return runs


def check_rows(rows: Sequence[Row], prof: Profile) -> None:
    """Raise GuardError if the rows break a profile timing rule or the allow-list."""
    for r in rows:
        g.check_allowed(r.button if r.button != "-" else "A", r.hat, r.stick)
        if "+" in r.button:
            raise GuardError(f"frame {r.frame}: two note buttons in one state (U3)")
    runs = press_runs(rows)
    mods_at = {}  # frame -> modifiers in effect
    for i, r in enumerate(rows[:-1]):
        for f in range(r.frame, rows[i + 1].frame):
            mods_at[f] = (r.hat, r.stick)
    for k, (b, a, e, m) in enumerate(runs):
        if e - a < prof.min_press:
            raise GuardError(f"frame {a}: {b} pressed {e - a} frame(s), below min_press_frames {prof.min_press}")
        if (e - a) * 1000 > prof.max_sustain_ms * prof.frame_rate:
            raise GuardError(f"frame {a}: {b} held longer than max_sustain_ms {prof.max_sustain_ms}")
        if k + 1 < len(runs) and runs[k + 1][0] == b and runs[k + 1][1] - e < prof.min_gap:
            raise GuardError(f"frame {runs[k + 1][1]}: {b} re-pressed after {runs[k + 1][1] - e} frame(s), "
                             f"below min_gap_same_button_frames {prof.min_gap}")
        for f in range(max(0, a - prof.lead), a):
            if mods_at.get(f, NEUTRAL_MODS) != m:
                raise GuardError(f"frame {a}: modifiers set less than modifier_lead_frames {prof.lead} "
                                 f"before the {b} press")


# ---------------------------------------------------------------------------
# Trial files
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class TrialFile:
    name: str
    header: tuple[str, ...]
    rows: tuple[Row, ...]
    notes: tuple[Planned, ...] = ()

    @property
    def frames(self) -> int:
        return self.rows[-1].frame


def bpm_text(bpm: Fraction) -> str:
    return str(bpm.numerator) if bpm.denominator == 1 else f"{float(bpm):g}"


def note_subset(items: Sequence[Item], first: int, last: int) -> list[Item]:
    """The melody items from sounding note `first` to `last` (1-based, inclusive), rests between them kept."""
    idx = [k for k, it in enumerate(items) if it.midi is not None]
    if not 1 <= first <= last <= len(idx):
        raise GuardError(f"--notes {first}-{last}: the melody has notes 1..{len(idx)}")
    return list(items[idx[first - 1]: idx[last - 1] + 1])


def melody_file(melody: pathlib.Path, prof: Profile, bpm: Fraction, notes_range: tuple[int, int] | None = None,
                extra_lead_ms: int = 0, max_press_ms: int | None = DEFAULT_MAX_PRESS_MS) -> TrialFile:
    items = parse_melody(melody.read_text(encoding="utf-8"))
    first = 1
    if notes_range is not None:
        first = notes_range[0]
        items = note_subset(items, *notes_range)
    if not 0 <= extra_lead_ms <= 60000:
        raise GuardError("--extra-lead-ms must be 0..60000")
    extra = round_half_up(Fraction(extra_lead_ms * prof.frame_rate, 1000))
    max_press = None
    if max_press_ms is not None:
        max_press = round_half_up(Fraction(max_press_ms * prof.frame_rate, 1000))
        if max_press < prof.min_press:
            raise GuardError(f"--max-press-ms {max_press_ms} is below min_press_frames {prof.min_press}")
    notes, end = plan(items, prof, bpm, extra, first, max_press)
    rows = melody_rows(notes, end)
    check_rows(rows, prof)
    bf = beat_frames(bpm, prof.frame_rate)
    beats = sum(i.beats for i in items)
    secs = rows[-1].frame / prof.frame_rate
    stem = melody.stem
    diag = notes_range is not None or extra_lead_ms or max_press_ms != DEFAULT_MAX_PRESS_MS
    suffix = ""
    if notes_range is not None:
        suffix += f"-n{notes_range[0]:02d}-{notes_range[1]:02d}"
    if extra_lead_ms:
        suffix += f"-lead{extra_lead_ms}ms"
    if max_press_ms != DEFAULT_MAX_PRESS_MS:
        suffix += "-uncapped" if max_press_ms is None else f"-maxpress{max_press_ms}ms"
    header = (
        f"# Story 1.6 walking skeleton: {stem}, {bpm_text(bpm)} BPM, {len(notes)} notes, {beats} beats.",
        "# GENERATED by spikes/gen_trials_1_6.py; do not edit by hand, regenerate instead.",
        f"# Melody: {rel(melody)}; profile: {rel(prof.path)}.",
        f"# Profile values used: {prof.values_text()}.",
        (f"# Options: bpm {bpm_text(bpm)} (beat {float(bf):g} frames); onsets rounded from cumulative beats; "
         f"first press at frame {prof.start_lead_frames}."),
        "# Fingering: the chart fingering of the profile for every pitch. The previous press ends",
        "# min_gap_same_button_frames before a same-button note, modifier_lead_frames before a note whose",
        "# D-pad/stick change, at the onset otherwise (onsets never move).",
        (f"# Press cap: {max_press_ms} ms (Story 1.6 workaround, staccato: see DEFAULT_MAX_PRESS_MS in the generator)."
         if max_press_ms is not None else "# Press cap: none (uncapped variant)."),
        "# Run in ocarina free play only (open it with D-pad left on the real controller, wait 2 s).",
        "# Allow-list: buttons L R A X Y, D-pad up/down, left stick. Never add B, ZL, ZR, D-pad left",
        "# or a diagonal D-pad, or the right stick. Stop with Ctrl+C in a real terminal.",
        f"# {len(notes)} NOTE rows (index, sounding pitch, MIDI, fingering), {rows[-1].frame} frames, about {secs:.1f} s.",
        "# Times are frames of the sender's 60 Hz grid.",
        *(((f"# DIAGNOSTIC variant: notes {first}..{notes[-1].index} of the melody (original numbering), "
            f"{extra_lead_ms} ms extra neutral before the first press, max press "
            f"{'-' if max_press_ms is None else f'{max_press_ms} ms'}. Not a committed trial file."),) if diag else ()),
    )
    return TrialFile(f"{stem}-{bpm_text(bpm)}bpm{suffix}.txt", header, tuple(rows), tuple(notes))


# --- U1 sustain trial -------------------------------------------------------


@dataclass(frozen=True)
class U1Options:
    holds_ms: tuple[int, ...] = (4000, 5000, 5500, 6000, 6500, 7000)  # the owner's list (Story 1.5)
    long_control_ms: int = 10000   # ends well after the auto-stop: gives the auto-stop time directly
    short_control_ms: int = 1000   # must read as not interrupted
    blank: bool = True             # no press: must not read as a tone
    rounds: int = 5
    rounds_per_file: int = 1
    pre: int = 30                  # neutral frames before each press (the analyser's quiet window)
    post: int = 180                # neutral frames after each release (3 s)
    button: str = "A"
    confirm_ms: int | None = None
    buttons: tuple[str, ...] = ("L", "R", "Y", "X")
    seed: int = SEED

    def validate(self, frame_rate: int) -> None:
        for b in (self.button, *self.buttons):
            if b not in U1_BUTTONS:
                raise GuardError(f"U1 button {b!r} is not allowed (only {' '.join(U1_BUTTONS)})")
        holds = [*self.holds_ms, self.long_control_ms, self.short_control_ms]
        if self.confirm_ms is not None:
            holds.append(self.confirm_ms)
        for ms in holds:
            hold_frames(ms, frame_rate)
        if self.rounds < 1 or self.rounds_per_file < 1:
            raise GuardError("rounds and rounds per file must be >= 1")
        if self.pre < 30 or self.post < 60:
            raise GuardError("pre must be >= 30 frames and post >= 60 frames")


def hold_frames(ms: int, frame_rate: int) -> int:
    if not isinstance(ms, int) or not 0 < ms <= U1_MAX_HOLD_MS:
        raise GuardError(f"U1 hold {ms!r} ms must be 1..{U1_MAX_HOLD_MS} ms")
    if (ms * frame_rate) % 1000:
        raise GuardError(f"U1 hold {ms} ms is not on the {frame_rate} Hz frame grid")
    return ms * frame_rate // 1000


@dataclass(frozen=True)
class U1Attempt:
    hold_ms: int | None  # None = blank
    control: bool
    scored: bool = True


def u1_attempt_rows(a: U1Attempt, button: str, t: int, attempt_no: int, rnd: tuple[int, int] | None,
                    o: U1Options, frame_rate: int) -> tuple[list[Row], int]:
    rows = [Row(t, "-", label="neutral")]
    press = t + o.pre
    if not a.scored:
        rows += [Row(press, button, label=f"WARMUP {button} press 12f (not scored)"),
                 Row(press + 12, "-", label="release")]
        return rows, press + 12 + o.post
    parts = ["TRY", "u1", button]
    parts.append("blank" if a.hold_ms is None else f"hold_ms={a.hold_ms}")
    parts.append(f"attempt={attempt_no}")
    if rnd:
        parts.append(f"round {rnd[0]}/{rnd[1]}")
    if a.control:
        parts.append("control")
    label = " ".join(parts)
    if a.hold_ms is None:
        rows.append(Row(press, "-", label=label))
        return rows, press + o.post
    n = hold_frames(a.hold_ms, frame_rate)
    rows += [Row(press, button, label=label), Row(press + n, "-", label="release")]
    return rows, press + n + o.post


def u1_file(name: str, title: str, button: str, rounds: Sequence[Sequence[U1Attempt]], first_round: int,
            total_rounds: int, o: U1Options, prof: Profile, extra_header: Sequence[str] = ()) -> TrialFile:
    rows, t = u1_attempt_rows(U1Attempt(12, False, scored=False), button, 0, 0, None, o, prof.frame_rate)
    k = 0
    for r, attempts in enumerate(rounds, start=first_round):
        for a in attempts:
            k += 1
            more, t = u1_attempt_rows(a, button, t, k, (r, total_rounds), o, prof.frame_rate)
            rows += more
    rows.append(Row(t, "-", label="end"))
    secs = t / prof.frame_rate
    header = (
        f"# Story 1.6 U1 sustain trial: {title}.",
        "# GENERATED by spikes/gen_trials_1_6.py; do not edit by hand, regenerate instead.",
        "# Holds up to 12 s on purpose (the U1 auto-stop is the measurement); no modifiers.",
        f"# Profile: {rel(prof.path)} (frame_rate {prof.frame_rate}, max_sustain_ms {prof.max_sustain_ms} when generated).",
        f"# Each attempt: {o.pre} neutral frames, the hold, {o.post} neutral frames. Shuffled interleaved rounds;",
        "# the first attempt is a WARMUP (not scored: the first input after a connect is ignored).",
        *extra_header,
        "# Run in ocarina free play only (open it with D-pad left on the real controller, wait 2 s).",
        "# Allow-list: buttons L R A X Y. Stop with Ctrl+C in a real terminal.",
        f"# {k} scored attempts (TRY rows) plus a WARMUP, {t} frames, about {secs:.1f} s.",
        "# Times are frames of the sender's 60 Hz grid.",
    )
    return TrialFile(name, header, tuple(rows))


def _shuffled_rounds(steps: Sequence[U1Attempt], o: U1Options, family: str) -> list[list[U1Attempt]]:
    rng = random.Random(f"{o.seed}-{family}")
    out = []
    for _ in range(o.rounds):
        order = list(steps)
        rng.shuffle(order)
        out.append(order)
    return out


def u1_round1(o: U1Options, prof: Profile) -> list[TrialFile]:
    o.validate(prof.frame_rate)
    steps = [U1Attempt(ms, False) for ms in o.holds_ms]
    steps += [U1Attempt(o.long_control_ms, True), U1Attempt(o.short_control_ms, True)]
    if o.blank:
        steps.append(U1Attempt(None, True))
    rounds = _shuffled_rounds(steps, o, f"u1-{o.button}")
    files = []
    for k in range(0, len(rounds), o.rounds_per_file):
        chunk = rounds[k: k + o.rounds_per_file]
        part = k // o.rounds_per_file + 1
        title = (f"button {o.button}, holds {', '.join(str(h) for h in o.holds_ms)} ms, controls "
                 f"{o.long_control_ms} ms, {o.short_control_ms} ms{' and a blank' if o.blank else ''}; "
                 f"rounds {k + 1}-{k + len(chunk)} of {len(rounds)}")
        files.append(u1_file(f"u1-sustain-{o.button}-part{part}.txt", title, o.button, chunk, k + 1, len(rounds),
                             o, prof))
    return files


def confirm_holds(o: U1Options) -> tuple[int, int]:
    v = o.confirm_ms
    longer = sorted(h for h in o.holds_ms if h > v)
    nxt = longer[0] if longer else v + 500
    return v, nxt


def u1_confirm(o: U1Options, prof: Profile) -> list[TrialFile]:
    if o.confirm_ms is None:
        raise GuardError("--set confirm needs --u1-confirm-ms (the candidate limit from round 1 on A)")
    o.validate(prof.frame_rate)
    v, nxt = confirm_holds(o)
    hold_frames(nxt, prof.frame_rate)
    files = []
    for b in o.buttons:
        rounds = _shuffled_rounds([U1Attempt(v, False), U1Attempt(nxt, False)], o, f"u1-confirm-{b}")
        files.append(u1_file(f"u1-confirm-{b}.txt", f"confirmation on {b}: holds {v} and {nxt} ms, "
                             f"{o.rounds} attempts each", b, rounds, 1, len(rounds), o, prof,
                             (f"# u1_confirm_ms: {v}",)))
    return files


# ---------------------------------------------------------------------------
# Rendering, key, CLI
# ---------------------------------------------------------------------------


def render(tf: TrialFile) -> str:
    out = [*tf.header, "# time   buttons  hat      stick    label"]
    for r in tf.rows:
        out.append(f"{str(r.frame) + 'f':<8} {r.button:<8} {g.ALLOWED_HATS[r.hat]:<8} {r.stick.token():<8} # {r.label}")
    return "\n".join(out) + "\n"


def key_lines(tf: TrialFile, frame_rate: int) -> list[str]:
    out = [f"{tf.name}: {tf.frames} frames, {tf.frames / frame_rate:.1f} s"]
    if tf.notes:
        out.append(f"  {'idx':>3}  {'note':<4} {'midi':>4}  {'fingering':<16} {'onset':>6} {'release':>7} {'pressed':>7}")
        for p in tf.notes:
            out.append(f"  {p.index:>3}  {p.name:<4} {p.midi:>4}  {p.fingering.text():<16} {p.onset:>5}f "
                       f"{p.release:>6}f {p.pressed_frames():>6}f" + (f"  ({len(p.segments)} presses)"
                                                                      if len(p.segments) > 1 else ""))
    else:
        for r in tf.rows:
            if r.label.startswith(("TRY", "WARMUP")):
                out.append(f"  t={r.frame / frame_rate:7.2f}s  {r.label}")
    return out


def bpm_value(tok: str) -> Fraction:
    try:
        v = Fraction(tok)
    except (ValueError, ZeroDivisionError):
        raise argparse.ArgumentTypeError(f"bad --bpm {tok!r}") from None
    if v <= 0:
        raise argparse.ArgumentTypeError("--bpm must be > 0")
    return v


def name_list(tok: str) -> tuple[str, ...]:
    return tuple(p.strip() for p in tok.split(",") if p.strip())


@dataclass(frozen=True)
class Options:
    which: str = "all"
    bpm: Fraction = Fraction(90)
    melody: pathlib.Path = DEFAULT_MELODY
    profile: pathlib.Path = DEFAULT_PROFILE
    u1: U1Options = field(default_factory=U1Options)
    notes_range: tuple[int, int] | None = None   # diagnostic melody subset (--set melody only)
    extra_lead_ms: int = 0                       # diagnostic extra lead (--set melody only)
    max_press_ms: int | None = DEFAULT_MAX_PRESS_MS  # press cap of the melody files (None = uncapped)


def build(o: Options) -> list[TrialFile]:
    prof = load_profile(o.profile)
    if o.which == "all":
        return [*(melody_file(o.melody, prof, Fraction(b), max_press_ms=o.max_press_ms) for b in COMMITTED_BPMS),
                *u1_round1(o.u1, prof)]
    if o.which == "melody":
        return [melody_file(o.melody, prof, o.bpm, o.notes_range, o.extra_lead_ms, o.max_press_ms)]
    if o.which == "u1":
        return u1_round1(o.u1, prof)
    if o.which == "confirm":
        return u1_confirm(o.u1, prof)
    raise GuardError(f"unknown set {o.which!r}")


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(prog="gen_trials_1_6.py",
                                description="Throwaway spike: write the Story 1.6 trial files for send_2wicc.py.")
    p.add_argument("--set", default="all", choices=("all", "melody", "u1", "confirm"),
                   help="all: the committed set (melody at 60, 90, 120 BPM and U1 round 1, default); "
                        "melody: one melody file at --bpm; u1: U1 round 1; confirm: U1 confirmation files")
    p.add_argument("--bpm", type=bpm_value, default=Fraction(90), help="melody tempo for --set melody (90)")
    p.add_argument("--melody", default=str(DEFAULT_MELODY), help="note list (default samples/story-1-6/ode-to-joy.melody)")
    p.add_argument("--profile", default=str(DEFAULT_PROFILE), help="profile TOML (default app/profiles/oot-switch2-1.1.1.toml)")
    p.add_argument("--out-dir", default=str(DEFAULT_OUT), help="output folder (default spikes/samples/story-1-6)")
    p.add_argument("--u1-button", default="A", help="U1 round 1 button (A)")
    p.add_argument("--u1-confirm-ms", type=int, help="confirm: the candidate limit from round 1 (ms)")
    p.add_argument("--u1-buttons", type=name_list, default=("L", "R", "Y", "X"), help="confirm: buttons (L,R,Y,X)")
    p.add_argument("--key", action="store_true", help="also print the note list / attempt times (reading aid)")
    p.add_argument("--notes", type=note_range, help="diagnostic, --set melody: only notes A-B (like 1-16)")
    p.add_argument("--extra-lead-ms", type=int, default=0,
                   help="diagnostic, --set melody: extra neutral before the first press, ms (0)")
    p.add_argument("--max-press-ms", type=int, default=DEFAULT_MAX_PRESS_MS,
                   help=f"melody files: release every note at most this long after its onset (staccato; default "
                        f"{DEFAULT_MAX_PRESS_MS}, the Story 1.6 workaround; 0 = uncapped)")
    return p


def note_range(tok: str) -> tuple[int, int]:
    a, sep, b = tok.partition("-")
    if not (sep and a.isdigit() and b.isdigit()):
        raise argparse.ArgumentTypeError(f"bad --notes {tok!r} (like 1-16)")
    return int(a), int(b)


def main(argv: Sequence[str] | None = None) -> int:
    a = build_parser().parse_args(argv)
    o = Options(a.set, a.bpm, pathlib.Path(a.melody), pathlib.Path(a.profile),
                U1Options(button=a.u1_button, confirm_ms=a.u1_confirm_ms, buttons=a.u1_buttons),
                a.notes, a.extra_lead_ms, a.max_press_ms or None)
    try:
        files = build(o)
        fr = load_profile(o.profile).frame_rate
    except (GuardError, OSError, KeyError, tomllib.TOMLDecodeError) as e:
        print(f"error: {e}", file=sys.stderr)
        return 1
    out_dir = pathlib.Path(a.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    for tf in files:
        (out_dir / tf.name).write_text(render(tf), encoding="utf-8", newline="\n")
        what = f"{len(tf.notes)} notes" if tf.notes else \
            f"{sum(1 for r in tf.rows if r.label.startswith('TRY'))} attempts"
        print(f"{tf.name:<28} {what:>12}  {tf.frames:>6} frames  {tf.frames / fr:6.1f} s")
    if a.key:
        print("\nKey (a reading aid, never a result):")
        for tf in files:
            print()
            print("\n".join(key_lines(tf, fr)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
