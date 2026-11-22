# /// script
# requires-python = ">=3.10"
# dependencies = ["numpy>=1.26", "pyserial>=3.5", "sounddevice>=0.5"]
# ///
"""Play a trial file through send_2wicc.py while recording the game audio, then read the pitch of every note.

SPDX-License-Identifier: MIT (the project licence, see LICENSE at the repo root).

THROWAWAY SPIKE CODE (Story 1.3, Part B helper). Not shipped, not part of app/
or firmware/. It replaces reading a phone tuner by hand: ffmpeg records the
host microphone (macOS avfoundation), the unchanged sender plays the file, and
every PRESS row is analysed (YIN pitch, nearest note, cents, level, onset).

Recordings are game audio and are NEVER committed: they go to
spikes/recordings/ (git-ignored).

Usage: uv run spikes/listen_trial.py --port /dev/cu.usbmodem101 spikes/samples/story-1-3/calibration-A.txt
       uv run spikes/listen_trial.py --analyse REC.wav TRIAL.txt --offset 0.123
       uv run spikes/listen_trial.py --list-devices
       uv run spikes/listen_trial.py --port PORT --backend sounddevice --mode melody spikes/samples/story-1-6/ode-to-joy-90bpm.txt
       uv run spikes/listen_trial.py --port PORT --backend sounddevice --mode sustain spikes/samples/story-1-6/u1-sustain-A-part1.txt
       uv run spikes/listen_trial.py --backend sounddevice --mode melody \
           --driver 'python3 firmware/tools/link_probe.py --port "$P" selfplay --lead-ms 3000' \
           spikes/samples/story-1-6/ode-to-joy-90bpm.txt      (Story 2.5 H.19: the firmware plays, not 2wiCC)
"""

from __future__ import annotations

import argparse
import csv
import datetime as dt
import importlib.util
import math
import os
import pathlib
import queue
import re
import shlex
import signal
import subprocess
import sys
import threading
import time
import wave
from collections import Counter
from collections.abc import Sequence
from dataclasses import dataclass

import numpy as np

HERE = pathlib.Path(__file__).resolve().parent
SENDER = HERE / "send_2wicc.py"
DEFAULT_OUT = HERE / "recordings"


def _load(name: str):
    if name in sys.modules:
        return sys.modules[name]
    spec = importlib.util.spec_from_file_location(name, HERE / f"{name}.py")
    mod = importlib.util.module_from_spec(spec)
    sys.modules[name] = mod
    spec.loader.exec_module(mod)
    return mod


s = _load("send_2wicc")
g = _load("gen_trials_1_3")

FRAME_RATE = 60.0
F_MIN, F_MAX = 300.0, 2000.0  # the ocarina sounds B4 (494 Hz) to F6 (1397 Hz)

# ---------------------------------------------------------------------------
# Pitch (pure)
# ---------------------------------------------------------------------------


def hz_to_midi(f: float) -> float:
    return 69.0 + 12.0 * math.log2(f / 440.0)


def note_and_cents(f: float) -> tuple[int, float]:
    m = hz_to_midi(f)
    n = round(m)
    return n, (m - n) * 100.0


def dbfs(x: np.ndarray) -> float:
    if x.size == 0:
        return -math.inf
    rms = float(np.sqrt(np.mean(np.square(x, dtype=np.float64))))
    return 20.0 * math.log10(rms) if rms > 0 else -math.inf


def yin_frame(frame: np.ndarray, sr: int, fmin: float = F_MIN, fmax: float = F_MAX,
              threshold: float = 0.15) -> tuple[float, float]:
    """YIN on one frame -> (f0 Hz, aperiodicity 0..1, lower = clearer). frame length >= W + tau_max."""
    tau_min = max(2, int(sr / fmax))
    tau_max = math.ceil(sr / fmin)
    w = frame.size - tau_max - 1
    if w < tau_max:
        raise ValueError("frame too short for fmin")
    x = frame.astype(np.float64)
    lags = np.lib.stride_tricks.sliding_window_view(x, w)[: tau_max + 2]
    d = np.sum((lags[0] - lags) ** 2, axis=1)  # d[tau], tau = 0..tau_max+1
    cmnd = np.ones_like(d)
    csum = np.cumsum(d[1:])
    taus = np.arange(1, d.size)
    with np.errstate(divide="ignore", invalid="ignore"):
        cmnd[1:] = np.where(csum > 0, d[1:] * taus / csum, 1.0)
    # First dip below the threshold (YIN step 4): avoids octave-down (subharmonic) picks.
    tau = -1
    for t in range(tau_min, tau_max + 1):
        if cmnd[t] < threshold:
            while t + 1 <= tau_max and cmnd[t + 1] < cmnd[t]:
                t += 1
            tau = t
            break
    if tau < 0:
        # Noisy frame: the first local dip close to the best one, not the global minimum, which
        # in noise often falls on a multiple of the period (an octave-down error).
        best = float(np.min(cmnd[tau_min: tau_max + 1]))
        for t in range(tau_min, tau_max + 1):
            if cmnd[t] <= best + 0.1 and cmnd[t] <= cmnd[t - 1] and cmnd[t] <= cmnd[t + 1]:
                tau = t
                break
        if tau < 0:
            tau = tau_min + int(np.argmin(cmnd[tau_min: tau_max + 1]))
    # Parabolic interpolation around the dip.
    a, b, c = cmnd[tau - 1], cmnd[tau], cmnd[tau + 1]
    den = a - 2 * b + c
    shift = 0.5 * (a - c) / den if den > 0 else 0.0
    return sr / (tau + shift), float(b)


def frame_len(sr: int, fmin: float = F_MIN) -> int:
    tau_max = math.ceil(sr / fmin)
    return max(1024, 4 * tau_max) + tau_max + 1


@dataclass(frozen=True)
class Pitch:
    hz: float | None      # median f0 of the voiced frames, None if no sound
    voiced: float         # fraction of frames with aperiodicity < max_aperiodicity
    level_dbfs: float


def estimate_pitch(x: np.ndarray, sr: int, max_aperiodicity: float = 0.3, max_frames: int = 60) -> Pitch:
    """Median YIN f0 over evenly spaced frames of a window."""
    n = frame_len(sr)
    level = dbfs(x)
    if x.size < n:
        return Pitch(None, 0.0, level)
    starts = np.linspace(0, x.size - n, num=min(max_frames, max(1, (x.size - n) // (n // 4) + 1))).astype(int)
    f0s, voiced = [], 0
    for st in starts:
        f, ap = yin_frame(x[st: st + n], sr)
        if ap < max_aperiodicity and F_MIN <= f <= F_MAX:
            voiced += 1
            f0s.append(f)
    frac = voiced / len(starts)
    return Pitch(float(np.median(f0s)) if f0s else None, frac, level)


# ---------------------------------------------------------------------------
# Trial schedule (from the trial file itself, via the sender's parser)
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class Press:
    index: int
    line: int
    t_press: float   # schedule seconds (frame-exact, like the sender)
    t_release: float
    label: str
    button: str
    model_midi: int | None   # button + D-pad + stick (full model)
    base_midi: int | None    # button + D-pad, stick not counted
    stick_sign: int          # +1 up, -1 down, 0 centre (from the wire Y the sender sends)


def _button_name(mask: int) -> str:
    names = [n for n, bit in s.CANON_BUTTONS.items() if mask & bit]
    return "+".join(names)


def presses_from_rows(rows: Sequence, frame_rate: float = FRAME_RATE) -> list[Press]:
    out: list[Press] = []
    frames = [s.time_to_frame(r.time, frame_rate) for r in rows]
    for i, r in enumerate(rows):
        b = r.state.buttons
        if not b or (i > 0 and rows[i - 1].state.buttons == b):
            continue
        j = i + 1
        while j < len(rows) and rows[j].state.buttons == b:
            j += 1
        t_rel = frames[j] / frame_rate if j < len(rows) else (frames[i] + 1) / frame_rate
        name = _button_name(b)
        y12 = s.stick_to_12(r.state.stick)[1]
        sign = (y12 > s.STICK_CENTRE_12) - (y12 < s.STICK_CENTRE_12)
        base = model = None
        if name in g.BUTTON_MIDI and r.state.hat in g.HAT_OFFSET:
            base = g.BUTTON_MIDI[name] + g.HAT_OFFSET[r.state.hat]
            model = base + 2 * sign
        label = r.label.removeprefix("PRESS ")
        out.append(Press(len(out) + 1, r.line, frames[i] / frame_rate, t_rel, label, name, model, base, sign))
    return out


def load_presses(trial: pathlib.Path) -> tuple[list, list[Press]]:
    rows = s.parse_lines(trial.read_text(encoding="utf-8-sig").splitlines())
    return rows, presses_from_rows(rows)


# ---------------------------------------------------------------------------
# Analysis per press
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class Params:
    attack: float = 0.30       # skip after the press
    release_guard: float = 0.20  # stop before the release
    min_dbfs: float = -60.0
    min_snr_db: float = 10.0
    min_voiced: float = 0.5
    latency: float = 0.0       # extra shift (s) on top of the offset, e.g. measured game latency
    scale: float = 1.0         # WAV seconds per schedule second (< 1 when the capture loses samples)
    band: tuple[float, float] = (250.0, 4000.0)  # analysis band: drops room rumble and hiss


def bandpass(x: np.ndarray, sr: int, lo: float, hi: float) -> np.ndarray:
    """Zero-phase FFT band-pass with 50 Hz linear edges (numpy only)."""
    if x.size == 0:
        return x
    spec = np.fft.rfft(x)
    f = np.fft.rfftfreq(x.size, 1.0 / sr)
    gain = np.clip((f - (lo - 50.0)) / 50.0, 0.0, 1.0) * np.clip(((hi + 50.0) - f) / 50.0, 0.0, 1.0)
    return np.fft.irfft(spec * gain, n=x.size)


def noise_floor_dbfs(xb: np.ndarray, sr: int, block: float = 0.05, pct: float = 10.0) -> float:
    """Quiet level of the whole (band-passed) recording: a low percentile of 50 ms block levels."""
    n = int(sr * block)
    if xb.size < n:
        return -math.inf
    blocks = xb[: xb.size - xb.size % n].reshape(-1, n)
    rms = np.sqrt(np.mean(blocks ** 2, axis=1))
    v = float(np.percentile(rms, pct))
    return 20.0 * math.log10(v) if v > 0 else -math.inf


@dataclass(frozen=True)
class Result:
    press: Press
    hz: float | None
    note: str
    cents: float | None
    level_dbfs: float
    noise_dbfs: float
    voiced: float
    onset_ms: float | None
    verdict: str


def verdict_for(press: Press, midi: int | None) -> str:
    if midi is None:
        return "NO SOUND"
    if press.model_midi is None:
        return "-"
    if press.stick_sign == 0:
        return "match" if midi == press.model_midi else "OTHER"
    if midi == press.model_midi:
        return "counted"
    if midi == press.base_midi:
        return "not counted"
    return "OTHER"


def _seg(x: np.ndarray, sr: int, t0: float, t1: float) -> np.ndarray:
    a, b = max(0, round(t0 * sr)), min(x.size, round(t1 * sr))
    return x[a:b] if b > a else x[:0]


def onset_ms(x: np.ndarray, sr: int, t_press: float, note_level_dbfs: float) -> float | None:
    """First 10 ms block from press-0.2 s to press+1 s whose level reaches note level - 6 dB."""
    hop = int(sr * 0.01)
    start = t_press - 0.2
    seg = _seg(x, sr, start, t_press + 1.0)
    for k in range(0, seg.size - hop + 1, hop):
        if dbfs(seg[k: k + hop]) >= note_level_dbfs - 6.0:
            return (start + k / sr - t_press) * 1000.0
    return None


def wav_time(t_sched: float, offset: float, p: Params) -> float:
    """Schedule time (s) -> WAV time (s)."""
    return offset + p.latency + t_sched * p.scale


def analyse_press(xb: np.ndarray, sr: int, press: Press, offset: float, p: Params = Params(),
                  floor: float = -math.inf) -> Result:
    """Analyse one press on the band-passed recording `xb`; `floor` is the recording's quiet level."""
    tp, tr = wav_time(press.t_press, offset, p), wav_time(press.t_release, offset, p)
    pitch = estimate_pitch(_seg(xb, sr, tp + p.attack, tr - p.release_guard), sr)
    # Noise just before the press; the file's quiet floor caps it, so a note that starts a little
    # early (alignment error) cannot hide itself by raising its own noise reference.
    local = dbfs(_seg(xb, sr, tp - 0.45, tp - 0.05))
    noise = min(local, floor) if math.isfinite(floor) else local
    sound = (pitch.hz is not None and pitch.level_dbfs >= p.min_dbfs and pitch.voiced >= p.min_voiced
             and (not math.isfinite(noise) or pitch.level_dbfs - noise >= p.min_snr_db))
    if not sound:
        return Result(press, None, "-", None, pitch.level_dbfs, noise, pitch.voiced, None, "NO SOUND")
    midi, cents = note_and_cents(pitch.hz)
    on = onset_ms(xb, sr, tp, pitch.level_dbfs)
    return Result(press, pitch.hz, g.midi_name(midi), cents, pitch.level_dbfs, noise, pitch.voiced,
                  None if on is None else on / p.scale, verdict_for(press, midi))


def analyse(x: np.ndarray, sr: int, presses: Sequence[Press], offset: float, p: Params = Params()) -> list[Result]:
    dur = x.size / sr
    xb = bandpass(x, sr, *p.band)
    floor = noise_floor_dbfs(xb, sr)
    return [analyse_press(xb, sr, pr, offset, p, floor) for pr in presses
            if wav_time(pr.t_release, offset, p) - p.release_guard <= dur]


# ---------------------------------------------------------------------------
# Story 1.4: attempts (TRY rows) and the timing analysers (detect, retrigger, lead, jitter)
# ---------------------------------------------------------------------------

_FIELD_RE = re.compile(r"(\w+)=(\d+)f?")
_ROUND_RE = re.compile(r"round (\d+)/(\d+)")
FRAME_MS = 1000.0 / FRAME_RATE
MODES = ("pitch", "detect", "retrigger", "lead", "jitter", "melody", "sustain")
TIMING_MODES = ("detect", "retrigger", "lead", "jitter")


@dataclass(frozen=True)
class Attempt:
    index: int
    line: int
    t: float                 # schedule s of the TRY row (the press, or where a blank's press would be)
    t_end: float             # schedule s of the next attempt's TRY/WARMUP row (or the file end)
    kind: str                # u7, u2, u5, jit
    button: str
    fields: dict             # key=value fields of the label (frames), e.g. {"press": 3}
    round: int | None
    control: bool
    blank: bool
    model_midi: int | None   # button + D-pad + stick of the TRY row's state
    label: str

    @property
    def step(self) -> str:
        key = {"u7": "press", "u2": "gap", "u5": "lead"}.get(self.kind)
        if self.blank:
            return "blank"
        return f"{self.fields[key]}f" if key in self.fields else "-"

    @property
    def family(self) -> str:
        return f"{self.kind} {self.fields_mod}".strip()

    @property
    def fields_mod(self) -> str:
        m = re.search(r"mod=(\w+)", self.label)
        return m.group(1) if m else ""


def attempts_from_rows(rows: Sequence, frame_rate: float = FRAME_RATE) -> list[Attempt]:
    """Every `TRY` row of a Story 1.4 trial file (gen_trials_1_4.py labels), in order."""
    frames = [s.time_to_frame(r.time, frame_rate) for r in rows]
    marks = [i for i, r in enumerate(rows) if r.label.startswith(("TRY", "WARMUP"))]
    out: list[Attempt] = []
    for k, i in enumerate(marks):
        r = rows[i]
        if not r.label.startswith("TRY"):
            continue
        toks = r.label.split()
        nxt = marks[k + 1] if k + 1 < len(marks) else None
        t_end = (frames[nxt] if nxt is not None else frames[-1] + 1) / frame_rate
        rm = _ROUND_RE.search(r.label)
        model = None
        if toks[2] in g.BUTTON_MIDI and r.state.hat in g.HAT_OFFSET:
            y12 = s.stick_to_12(r.state.stick)[1]
            sign = (y12 > s.STICK_CENTRE_12) - (y12 < s.STICK_CENTRE_12)
            model = g.BUTTON_MIDI[toks[2]] + g.HAT_OFFSET[r.state.hat] + 2 * sign
        out.append(Attempt(len(out) + 1, r.line, frames[i] / frame_rate, t_end, toks[1], toks[2],
                           {k_: int(v) for k_, v in _FIELD_RE.findall(r.label)},
                           int(rm.group(1)) if rm else None, "control" in toks, "blank" in toks, model, r.label))
    return out


def envelope_db(x: np.ndarray, sr: int, hop: float = 0.002, smooth: float = 0.006) -> np.ndarray:
    """Level in dBFS per `hop` (RMS), power-averaged over `smooth`. Index k covers x[k*hop : (k+1)*hop]."""
    n = max(1, round(hop * sr))
    m = x.size // n
    if m == 0:
        return np.full(0, -math.inf)
    p = np.mean(np.square(x[: m * n].reshape(m, n)), axis=1)
    w = max(1, round(smooth / hop))
    if w > 1:
        p = np.convolve(p, np.ones(w) / w, mode="same")
    with np.errstate(divide="ignore"):
        return 10.0 * np.log10(p)


@dataclass(frozen=True)
class TimingParams:
    """Thresholds of the Story 1.4 analysers; every value used is printed in the report."""
    detect_from: float = 0.02     # detect window: press + 0.02 s ...
    detect_to: float = 0.30       # ... to press + 0.30 s (the game latency is about 0.13 s)
    band_snr_db: float = 8.0      # sound = level at the expected pitch at least this above the same band before
    pitch_tol_cents: float = 60.0  # a sounding note further than this from the expected pitch is a wrong note
    audible_db: float = 20.0      # audible length: time above peak - 20 dB
    dip_db: float | None = None   # retrigger thresholds; None = from the g=0 / g=30 controls
    rise_db: float | None = None
    lead_cents: float = 50.0
    lead_window: float = 0.040    # pitch windows that start within onset + 40 ms
    pitch_window: float = 0.030
    pitch_hop: float = 0.005


@dataclass(frozen=True)
class AttemptResult:
    attempt: Attempt
    sound: bool
    level_dbfs: float
    noise_dbfs: float
    onset_ms: float | None = None
    audible_ms: float | None = None
    hz: float | None = None
    onsets: int | None = None
    dip_db: float | None = None
    start_cents: float | None = None   # lead: first window vs the steady pitch
    lead_cents: float | None = None    # lead: median of the first 40 ms vs the steady pitch
    verdict: str = ""
    passed: bool | None = None         # None for controls and blanks (not scored)


# The game plays about +27 cents sharp of A4 = 440 Hz (Story 1.3); expected pitches include it.
GAME_CENTS = 27.0


def expected_hz(midi: int) -> float:
    return g.midi_hz(midi) * 2 ** (GAME_CENTS / 1200.0)


def cents(f: float, ref: float) -> float:
    return 1200.0 * math.log2(f / ref)


def band_power_db(x: np.ndarray, sr: int, t0: float, t1: float, f: float, width: float = 0.03) -> float:
    """Mean power (dB) of x[t0:t1] within +-`width` (relative) of `f` (Hann window, FFT).

    A narrow band around the expected pitch is far less sensitive to the room and console noise than
    the 250 to 4000 Hz band: on the live recordings X (D6) notes were 9 to 11 dB above the broadband
    noise but 14 to 23 dB above the same narrow band before the press (2026-11-21)."""
    seg = _seg(x, sr, t0, t1)
    if seg.size < 16:
        return -math.inf
    spec = np.abs(np.fft.rfft(seg * np.hanning(seg.size))) ** 2
    fr = np.fft.rfftfreq(seg.size, 1.0 / sr)
    sel = (fr > f * (1 - width)) & (fr < f * (1 + width))
    p = float(spec[sel].sum()) / seg.size
    return 10.0 * math.log10(p) if p > 0 else -math.inf


def spectral_peak(x: np.ndarray, sr: int, t0: float, t1: float, lo: float, hi: float,
                  nfft: int = 1 << 15) -> float | None:
    """Frequency of the strongest spectral peak of x[t0:t1] in [lo, hi] (zero-padded FFT, parabolic fit).

    Searching only near the expected pitch makes octave and fifth errors impossible (short-window YIN
    on noisy notes locked onto f/2, f/3 or 2f on the live recordings)."""
    seg = _seg(x, sr, t0, t1)
    if seg.size < 16:
        return None
    n = max(nfft, seg.size)
    mag = np.abs(np.fft.rfft(seg * np.hanning(seg.size), n))
    fr = np.fft.rfftfreq(n, 1.0 / sr)
    idx = np.flatnonzero((fr >= lo) & (fr <= hi))
    if idx.size < 3:
        return None
    k = int(idx[np.argmax(mag[idx])])
    if 0 < k < mag.size - 1 and mag[k] > 0:
        a, b, c = (math.log(max(v, 1e-30)) for v in mag[k - 1: k + 2])
        den = a - 2 * b + c
        shift = 0.5 * (a - c) / den if den < 0 else 0.0
    else:
        shift = 0.0
    return float((k + shift) * sr / n)


def tone_env_db(x: np.ndarray, sr: int, f: float, t0: float, t1: float, win: float = 0.010,
                hop: float = 0.002) -> tuple[np.ndarray, np.ndarray]:
    """Level (dBFS amplitude) of the component at `f`: Hann-windowed demodulation, window `win`,
    one value per `hop`. Returns (window centre times s, levels dB)."""
    n, h = round(win * sr), max(1, round(hop * sr))
    a0, a1 = max(0, round(t0 * sr)), min(x.size, round(t1 * sr))
    if a1 - a0 < n:
        return np.zeros(0), np.zeros(0)
    w = np.hanning(n)
    ker = w * np.exp(-2j * np.pi * f * np.arange(n) / sr) * 2.0 / w.sum()
    frames = np.lib.stride_tricks.sliding_window_view(x[a0:a1], n)[::h]
    amp = np.empty(frames.shape[0])
    for k in range(0, frames.shape[0], 4096):  # chunks keep memory bounded on long files
        amp[k: k + 4096] = np.abs(frames[k: k + 4096] @ ker)
    t = (a0 + np.arange(amp.size) * h + n / 2) / sr
    with np.errstate(divide="ignore"):
        return t, 20.0 * np.log10(amp)


def half_crossing(t: np.ndarray, env: np.ndarray, peak_idx: int, drop_db: float = 6.0) -> float:
    """Time of the rising crossing of (peak - drop_db) before `peak_idx`, linearly interpolated."""
    thr = env[peak_idx] - drop_db
    j = peak_idx
    while j > 0 and env[j - 1] >= thr:
        j -= 1
    if j == 0 or env[j] == env[j - 1]:
        return float(t[j])
    frac = (thr - env[j - 1]) / (env[j] - env[j - 1])
    return float(t[j - 1] + frac * (t[j] - t[j - 1]))


def band_onset(xb: np.ndarray, sr: int, lo: float, hi: float, tp: float) -> tuple[float | None, float, float]:
    """(onset s, peak dB, noise dB) of the level in the band [lo, hi] around a press at WAV time `tp`.

    The onset is the -6 dB crossing before the peak in [tp - 0.05 s, tp + 0.8 s] of the 2 ms envelope of
    the band. Used by `lead`: the band covers every pitch the note may start at (a missing modifier is
    up to 2 semitones away), so a wrong-pitch start is not skipped as "before the onset"."""
    t0 = max(0.0, tp - 0.5)
    y = bandpass(_seg(xb, sr, t0, tp + 0.9), sr, lo, hi)
    env = envelope_db(y, sr)
    if env.size == 0:
        return None, -math.inf, -math.inf
    times = t0 + (np.arange(env.size) + 0.5) * 0.002
    pre = env[(times >= tp - 0.45) & (times < tp - 0.05)]
    win = np.flatnonzero((times >= tp - 0.05) & (times <= tp + 0.8))
    if win.size == 0:
        return None, -math.inf, -math.inf
    k = int(win[np.argmax(env[win])])
    noise = float(np.median(pre)) if pre.size else -math.inf
    return half_crossing(times[win[0]: k + 1], env[win[0]: k + 1], k - win[0]), float(env[k]), noise


def detect_attempt(xb: np.ndarray, sr: int, a: Attempt, offset: float, p: Params, tp_: TimingParams,
                   floor: float = -math.inf) -> AttemptResult:
    """Row 2.1: sound yes/no at the expected pitch, onset, audible length, measured pitch.

    Sound = power within +-3 % of the expected pitch in [press + 0.02 s, press + 0.30 s] at least
    `band_snr_db` above the same band just before the press. If not, a strong peak elsewhere (another
    note) is a FAIL pitch. Pitch = strongest spectral peak between 0.75 and 1.33 x the expected pitch."""
    tp = wav_time(a.t, offset, p)
    f = expected_hz(a.model_midi if a.model_midi is not None else g.BUTTON_MIDI.get(a.button, 81))
    w0, w1 = tp + tp_.detect_from, tp + tp_.detect_to
    level = band_power_db(xb, sr, w0, w1, f)
    noise = band_power_db(xb, sr, tp - 0.45, tp - 0.05, f)
    sound = math.isfinite(level) and (not math.isfinite(noise) or level - noise >= tp_.band_snr_db)
    if a.blank:
        return AttemptResult(a, sound, level, noise, verdict="BLANK SOUNDED" if sound else "silent (ok)")
    if not sound:
        other = spectral_peak(xb, sr, w0, w1, F_MIN, F_MAX)
        if other is not None and abs(cents(other, f)) > tp_.pitch_tol_cents:
            lo, no = band_power_db(xb, sr, w0, w1, other), band_power_db(xb, sr, tp - 0.45, tp - 0.05, other)
            if math.isfinite(lo) and lo - no >= tp_.band_snr_db:
                return AttemptResult(a, True, level, noise, hz=other, verdict="FAIL pitch", passed=False)
        return AttemptResult(a, False, level, noise, verdict="FAIL no sound", passed=False)
    hz = spectral_peak(xb, sr, w0, w1, f * 0.75, f * 1.33)
    t, e = tone_env_db(xb, sr, f, tp - 0.05, tp + 0.8)
    on_ms = audible = None
    if e.size:
        k = int(np.argmax(e))
        on_ms = (half_crossing(t, e, k) - tp) * 1000.0 / p.scale
        _, en = tone_env_db(xb, sr, f, tp - 0.45, tp - 0.05)
        floor_db = float(np.median(en)) + 3.0 if en.size else -math.inf
        audible = float(np.sum(e >= max(e[k] - tp_.audible_db, floor_db))) * 2.0 / p.scale
    ok = hz is not None and abs(cents(hz, f)) <= tp_.pitch_tol_cents
    return AttemptResult(a, True, level, noise, on_ms, audible, hz, verdict="PASS" if ok else "FAIL pitch",
                         passed=ok)


def valley_depth(env: np.ndarray) -> float:
    """Deepest dip that is followed by a re-rise: max over k of min(peak before k, peak after k) - env[k]."""
    if env.size < 3:
        return 0.0
    before = np.maximum.accumulate(env)
    after = np.maximum.accumulate(env[::-1])[::-1]
    return float(np.max(np.minimum(before - env, after - env)))


def dip_and_rise(env: np.ndarray, dip_db: float, rise_db: float) -> bool:
    before = np.maximum.accumulate(env)
    after = np.maximum.accumulate(env[::-1])[::-1]
    return bool(np.any((before - env >= dip_db) & (after - env >= rise_db)))


RETRIGGER_BAND = (0.8, 1.25)  # x the expected pitch: the fundamental only (keeps X above the noise)


def _u2_env(xf: np.ndarray, sr: int, a: Attempt, offset: float, p: Params, floor: float) -> tuple[np.ndarray, float]:
    tp = wav_time(a.t, offset, p)
    noise = _local_noise(xf, sr, tp, floor)
    span = (2 * a.fields.get("hold", 12) + a.fields.get("gap", 0)) / FRAME_RATE
    end = min(wav_time(a.t + span + 0.5, offset, p), wav_time(a.t_end, offset, p) - 0.45)
    env = envelope_db(_seg(xf, sr, tp - 0.05, end), sr)
    if math.isfinite(noise):
        env = np.maximum(env, noise + 6.0)  # noise wobble below the floor + 6 dB is not a dip or a rise
    return env, noise


def _local_noise(xb: np.ndarray, sr: int, tp: float, floor: float) -> float:
    local = dbfs(_seg(xb, sr, tp - 0.45, tp - 0.05))
    return min(local, floor) if math.isfinite(floor) else local


RETRIGGER_MARGIN_DB = 6.0


def calibrate_retrigger(valleys0: Sequence[float], valleys30: Sequence[float],
                        margin: float = RETRIGGER_MARGIN_DB) -> tuple[float | None, bool]:
    """Dip/rise threshold from the controls: the deepest valley of a held note (g=0) plus `margin` dB,
    but at most half way to the shallowest valley of a clear retrigger (g=30). Valid if they are apart.

    The g=0 control measures how much a held note wobbles on its own; a short real retrigger dips far
    less than the g=30 one, so a plain midpoint would miss it. Pool the controls of every file of one
    button (`--combine --pool-controls`): one file's 20 controls can miss a rare deep wobble."""
    if not valleys0 or not valleys30:
        return None, False
    hi0, lo30 = max(valleys0), min(valleys30)
    return hi0 + min(margin, (lo30 - hi0) / 2.0), hi0 < lo30


def _retrigger_verdict(a_control: bool, gap: int | None, n_on: int | None) -> tuple[str, bool | None]:
    want = 1 if gap == 0 else 2
    if n_on is None:
        return "NO THRESHOLD", None
    if a_control:
        return ("control ok" if n_on == want else "CONTROL WRONG"), None
    return ("PASS" if n_on == 2 else f"FAIL {n_on} onset(s)"), n_on == 2


def retrigger_attempts(x: np.ndarray, sr: int, atts: Sequence[Attempt], offset: float, p: Params,
                       tp_: TimingParams) -> tuple[list[AttemptResult], dict[str, str]]:
    """Row 2.2: onsets (1 or 2) and the dip depth per attempt, on the band around the note's fundamental
    (`RETRIGGER_BAND`); thresholds from the controls unless given."""
    bands: dict[int, tuple[np.ndarray, float]] = {}
    envs = []
    for a in atts:
        m = a.model_midi if a.model_midi is not None else 81
        if m not in bands:
            f = expected_hz(m)
            xf = bandpass(x, sr, f * RETRIGGER_BAND[0], f * RETRIGGER_BAND[1])
            bands[m] = (xf, noise_floor_dbfs(xf, sr))
        envs.append(_u2_env(bands[m][0], sr, a, offset, p, bands[m][1]))
    valleys = [valley_depth(e) if e.size else 0.0 for e, _ in envs]
    v0 = [v for a, v in zip(atts, valleys) if a.control and a.fields.get("gap") == 0]
    v30 = [v for a, v in zip(atts, valleys) if a.control and a.fields.get("gap", 0) > 0]
    thr, separated = calibrate_retrigger(v0, v30)
    dip = tp_.dip_db if tp_.dip_db is not None else thr
    rise = tp_.rise_db if tp_.rise_db is not None else thr
    meta = {"band": f"{RETRIGGER_BAND[0]:g} to {RETRIGGER_BAND[1]:g} x the expected pitch",
            "dip_db": _fmt(dip, ".1f"), "rise_db": _fmt(rise, ".1f"),
            "threshold_source": "options" if tp_.dip_db is not None and tp_.rise_db is not None else "controls",
            "controls_g0_valley_top3_db": ", ".join(f"{v:.1f}" for v in sorted(v0)[-3:]) or "-",
            "controls_g30_valley_low3_db": ", ".join(f"{v:.1f}" for v in sorted(v30)[:3]) or "-",
            "controls_separated": str(separated)}
    out = []
    for a, (env, noise), v in zip(atts, envs, valleys):
        peak = float(np.max(env)) if env.size else -math.inf
        sound = env.size > 0 and peak >= p.min_dbfs and (not math.isfinite(noise) or peak - noise >= p.min_snr_db)
        if not sound:
            n_on = 0
        elif dip is None or rise is None:
            n_on = None
        else:
            n_on = 2 if dip_and_rise(env, dip, rise) else 1
        verdict, ok = _retrigger_verdict(a.control, a.fields.get("gap"), n_on)
        out.append(AttemptResult(a, sound, peak, noise, onsets=n_on, dip_db=v, verdict=verdict, passed=ok))
    return out, meta


def pooled_rescore(rows: Sequence[dict[str, str]]) -> tuple[list[dict[str, str]], list[str]]:
    """Re-score retrigger report rows with one threshold from the controls of all of them (same rule as
    `calibrate_retrigger`). Rows without sound (0 onsets) stay as they are."""
    def dip(r):
        return float(r["dip_db"]) if r["dip_db"] not in ("", "-") else 0.0
    g0 = [r for r in rows if r["control"] == "1" and r["step"] == "0f"]
    g30 = [r for r in rows if r["control"] == "1" and r["step"] != "0f"]
    thr, separated = calibrate_retrigger([dip(r) for r in g0], [dip(r) for r in g30])
    if thr is None:
        return list(rows), ["pooled controls: none (need gap 0 and gap 30 controls)"]
    out = []
    for r in rows:
        r = dict(r)
        if r["onsets"] not in ("0",):
            n_on = 2 if dip(r) >= thr else 1
            r["onsets"] = str(n_on)
            r["verdict"], ok = _retrigger_verdict(r["control"] == "1", int(r["step"][:-1]), n_on)
            r["pass"] = "" if ok is None else str(int(ok))
        out.append(r)
    top = sorted(g0, key=dip)[-3:]
    deepest = "; ".join(f"{dip(r):.1f} dB {r['label']}" for r in reversed(top))
    lines = [(f"pooled controls: {len(g0)} held (gap 0), {len(g30)} retrigger (gap 30); threshold {thr:.1f} dB "
              f"(deepest held-note dip + min(6 dB, half the distance to the shallowest gap-30 dip)); "
              f"separated {separated}"),
             f"deepest held-note dips (outliers raise the threshold): {deepest}"]
    return out, lines


def lead_attempt(xb: np.ndarray, sr: int, a: Attempt, offset: float, p: Params, tp_: TimingParams,
                 floor: float = -math.inf) -> AttemptResult:
    """Row 2.3: is the pitch right from the note start? Median of the windows in [onset, onset + 40 ms]
    vs the steady pitch (last half of the hold), within `lead_cents`.

    The onset is taken on the level of the band 0.8 to 1.25 x the pitch (every pitch the note may start
    at), and every pitch is the strongest spectral peak in that band: wide enough for a missing modifier
    (+-2 semitones), too narrow for an octave error. Resolution: the verdict is the median of 9 overlapping
    30 ms windows, so a wrong start shorter than about 35 ms (2 frames) passes (synthetic notes: 33 ms
    passes, 36 ms fails, see the tests); a 1 to 2 frame lateness of a modifier is not resolvable by audio."""
    tp = wav_time(a.t, offset, p)
    f = expected_hz(a.model_midi if a.model_midi is not None else 81)
    on, peak, noise = band_onset(xb, sr, f * 0.8, f * 1.25, tp)
    # sound check as in detect, on a band wide enough for a missing modifier (+-2 semitones)
    level = band_power_db(xb, sr, tp + tp_.detect_from, tp + tp_.detect_to, f, width=0.13)
    quiet = band_power_db(xb, sr, tp - 0.45, tp - 0.05, f, width=0.13)
    if on is None or not math.isfinite(level) or (math.isfinite(quiet) and level - quiet < tp_.band_snr_db):
        return AttemptResult(a, False, level, quiet, verdict="FAIL no sound", passed=False)
    note = a.fields.get("note", 36) / FRAME_RATE * p.scale
    steady = spectral_peak(xb, sr, on + note / 2, on + note - 0.01, f * 0.8, f * 1.25)
    if steady is None:
        return AttemptResult(a, True, peak, noise, verdict="FAIL no pitch", passed=False)
    start = []
    t = on
    while t <= on + tp_.lead_window + 1e-9:
        fs = spectral_peak(xb, sr, t, t + tp_.pitch_window, steady * 0.8, steady * 1.25)
        if fs is not None:
            start.append(fs)
        t += tp_.pitch_hop
    on_ms = (on - tp) * 1000.0 / p.scale
    if not start:
        return AttemptResult(a, True, peak, noise, on_ms, verdict="FAIL no pitch", passed=False)
    err = cents(float(np.median(start)), steady)
    ok = abs(err) <= tp_.lead_cents
    return AttemptResult(a, True, peak, noise, on_ms, hz=steady, start_cents=cents(start[0], steady),
                         lead_cents=err, verdict="PASS" if ok else "FAIL pitch at start", passed=ok)


def analyse_attempts(x: np.ndarray, sr: int, atts: Sequence[Attempt], offset: float, mode: str,
                     p: Params = Params(), tp_: TimingParams = TimingParams()) -> tuple[list[AttemptResult], dict]:
    dur = x.size / sr
    atts = [a for a in atts if wav_time(a.t, offset, p) + 0.8 <= dur]
    if mode == "retrigger":
        return retrigger_attempts(x, sr, atts, offset, p, tp_)
    xb = bandpass(x, sr, *p.band)
    if mode == "detect":
        return [detect_attempt(xb, sr, a, offset, p, tp_) for a in atts], {
            "band_snr_db": f"{tp_.band_snr_db:g}", "game_cents": f"{GAME_CENTS:+g}",
            "window_s": f"{tp_.detect_from:g} to {tp_.detect_to:g} after the press"}
    if mode == "lead":
        return [lead_attempt(xb, sr, a, offset, p, tp_) for a in atts], {
            "lead_cents": f"{tp_.lead_cents:g}", "band_snr_db": f"{tp_.band_snr_db:g}"}
    raise ValueError(f"mode {mode!r}")


@dataclass(frozen=True)
class StepStat:
    family: str
    step: str
    passes: int
    tries: int
    fail_rounds: tuple[int, ...]


def step_table(rows: Sequence[dict[str, str]]) -> list[StepStat]:
    """Pass counts per (family, step) from result rows (the CSV columns); controls/blanks are not scored."""
    acc: dict[tuple[str, str], list] = {}
    for r in rows:
        if r["pass"] not in ("0", "1"):
            continue
        e = acc.setdefault((r["family"], r["step"]), [0, 0, []])
        e[1] += 1
        if r["pass"] == "1":
            e[0] += 1
        elif r["round"] not in ("", "-"):
            e[2].append(int(r["round"]))

    def key(k):
        fam, step = k
        return fam, int(step[:-1]) if step.endswith("f") and step[:-1].isdigit() else 10 ** 6

    return [StepStat(f, st, e[0], e[1], tuple(sorted(e[2]))) for (f, st), e in sorted(acc.items(), key=lambda kv: key(kv[0]))]


def smallest_passing(stats: Sequence[StepStat]) -> dict[str, str]:
    """Per family: the smallest step for which that step and every larger step passed every try."""
    out: dict[str, str] = {}
    for fam in dict.fromkeys(st.family for st in stats):
        steps = [st for st in stats if st.family == fam]
        best = None
        for st in reversed(steps):
            if st.passes != st.tries:
                break
            best = st.step
        if best is None:
            out[fam] = "none (the largest step did not pass every try)"
        else:
            n = int(best[:-1])
            out[fam] = f"{best} = {n * FRAME_MS:.1f} ms"
    return out


ATTEMPT_COLUMNS = ("n", "line", "t_s", "label", "family", "step", "round", "control", "level_dbfs", "snr_db",
                   "onset_ms", "audible_ms", "hz", "note", "onsets", "dip_db", "start_cents", "lead_cents",
                   "verdict", "pass")


def attempt_row(r: AttemptResult) -> dict[str, str]:
    a = r.attempt
    return {
        "n": str(a.index), "line": str(a.line), "t_s": f"{a.t:.3f}", "label": a.label, "family": a.family,
        "step": a.step, "round": "-" if a.round is None else str(a.round), "control": "1" if a.control else "0",
        "level_dbfs": _fmt(r.level_dbfs, ".1f"), "snr_db": _fmt(r.level_dbfs - r.noise_dbfs, ".1f"),
        "onset_ms": _fmt(r.onset_ms, ".0f"), "audible_ms": _fmt(r.audible_ms, ".0f"), "hz": _fmt(r.hz, ".1f"),
        "note": g.midi_name(note_and_cents(r.hz)[0]) if r.hz else "-",
        "onsets": "-" if r.onsets is None else str(r.onsets), "dip_db": _fmt(r.dip_db, ".1f"),
        "start_cents": _fmt(r.start_cents, "+.0f"), "lead_cents": _fmt(r.lead_cents, "+.0f"),
        "verdict": r.verdict, "pass": "" if r.passed is None else str(int(r.passed)),
    }


def attempt_summary_lines(rows: Sequence[dict[str, str]], meta: dict[str, str]) -> list[str]:
    out = [f"{k}: {v}" for k, v in meta.items()]
    stats = step_table(rows)
    if stats:
        out.append(f"{'family':<10} {'step':>5} {'ms':>6}  passes  fail rounds")
        for st in stats:
            ms = f"{int(st.step[:-1]) * FRAME_MS:.1f}" if st.step[:-1].isdigit() else "-"
            out.append(f"{st.family:<10} {st.step:>5} {ms:>6}  {st.passes:>2}/{st.tries:<3}  "
                       f"{','.join(map(str, st.fail_rounds)) or '-'}")
        for fam, v in smallest_passing(stats).items():
            out.append(f"smallest step passing every try (and every larger step): {fam}: {v}")
    bad = [r for r in rows if r["verdict"] in ("BLANK SOUNDED", "CONTROL WRONG", "NO THRESHOLD")]
    if bad:
        out.append(f"INVALID: {len(bad)} control/blank attempt(s) did not behave "
                   f"({', '.join(sorted({r['verdict'] for r in bad}))}); find out why before using the numbers")
    return out


# ---- row 2.4: jitter (inter-onset intervals against the sender's frame schedule)


@dataclass(frozen=True)
class JitterResult:
    notes: int
    matched: int              # notes whose own button was the loudest (>= min_dom_db) at its peak
    ioi_err_ms: np.ndarray    # measured IOI (rescaled to schedule s) minus scheduled IOI, consecutive pairs
    drift_ms_per_s: float
    latency_ms: float         # median per-note latency (schedule t to onset)
    latency_sd_ms: float
    lag_hist: tuple[tuple[int, int], ...]  # (ms from the median, count) in 4 ms bins
    quant_sigma_ms: float     # sigma of the schedule's own IOI vs the ideal 125 ms (the 7/8 frame pattern)

    def stats(self) -> dict[str, float]:
        e = self.ioi_err_ms
        if e.size < 2:
            return {}
        a = np.abs(e)
        return {"mean_ms": float(np.mean(e)), "sigma_ms": float(np.std(e, ddof=1)),
                "p95_ms": float(np.percentile(a, 95)), "max_ms": float(np.max(a))}


def analyse_jitter(x: np.ndarray, sr: int, atts: Sequence[Attempt], offset: float, p: Params = Params(),
                   ideal_ioi: float = 0.125, win: float = 0.020, hop: float = 0.001, min_dom_db: float = 3.0,
                   max_latency: float = 0.5) -> JitterResult:
    """Onsets per button: one level envelope per button pitch (window `win`; 20 ms separates A5 and B5,
    110 Hz apart). One latency for the whole run: the shift at which each scheduled note's own button is
    loudest. Each onset = the -6 dB crossing of its button's envelope in [e + L - 70 ms, e + L + 100 ms],
    kept only if that button is at least `min_dom_db` louder than the others at its peak. No per-note
    latency tracking; the drift comes from the fit only."""
    xb = bandpass(x, sr, *p.band)
    mids = sorted({a.model_midi for a in atts if a.model_midi is not None})
    env = {}
    t = np.zeros(0)
    for m in mids:
        t, env[m] = tone_env_db(xb, sr, expected_hz(m), 0.0, x.size / sr, win, hop)
    if t.size == 0 or not mids:
        return JitterResult(len(atts), 0, np.zeros(0), math.nan, math.nan, math.nan, (), math.nan)
    stack = np.vstack([env[m] for m in mids])
    dom = {m: env[m] - np.max(np.delete(stack, i, axis=0), axis=0) if len(mids) > 1 else env[m]
           for i, m in enumerate(mids)}

    def idx(tt):
        return np.clip(np.round((np.asarray(tt) - t[0]) / hop).astype(int), 0, t.size - 1)

    exp = np.array([wav_time(a.t, offset, p) for a in atts])
    ms = [a.model_midi for a in atts]
    best_l, best_s = 0.0, -math.inf
    for lat in np.arange(0.0, max_latency, 0.002):
        sc = float(np.mean([dom[m][i] for m, i in zip(ms, idx(exp + lat + 0.04))]))
        if sc > best_s:
            best_l, best_s = float(lat), sc
    got: list[float | None] = []
    for e, m in zip(exp, ms):
        i0, i1 = idx(e + best_l - 0.07), idx(e + best_l + 0.10)
        if i1 - i0 < 3:
            got.append(None)
            continue
        k = i0 + int(np.argmax(env[m][i0:i1]))
        got.append(half_crossing(t, env[m], k) if dom[m][k] >= min_dom_db else None)
    sched = np.array([a.t for a in atts])
    pairs = [(i, i + 1) for i in range(len(atts) - 1) if got[i] is not None and got[i + 1] is not None]
    err = np.array([((got[j] - got[i]) / p.scale - (sched[j] - sched[i])) * 1000.0 for i, j in pairs])
    keep = [i for i in range(len(atts)) if got[i] is not None]
    lag = np.array([((got[i] - offset - p.latency) / p.scale - sched[i]) * 1000.0 for i in keep])
    drift = float(np.polyfit(sched[keep], lag, 1)[0]) if len(keep) >= 3 else math.nan
    med = float(np.median(lag)) if lag.size else math.nan
    hist: tuple[tuple[int, int], ...] = ()
    if lag.size:
        h, edges = np.histogram(lag - med, bins=np.arange(-30, 34, 4))
        hist = tuple((int(e0), int(c)) for e0, c in zip(edges[:-1], h) if c)
    q = np.diff(sched) - ideal_ioi
    return JitterResult(len(atts), len(keep), err, drift, med, float(np.std(lag)) if lag.size else math.nan, hist,
                        float(np.std(q, ddof=1)) * 1000.0 if q.size > 1 else math.nan)


def jitter_lines(j: JitterResult) -> list[str]:
    st = j.stats()
    out = [f"notes {j.notes}, matched {j.matched} (the note's own button the loudest at its onset)",
           (f"latency (median, includes USB, console and audio) {j.latency_ms:.1f} ms, per-note sd "
            f"{j.latency_sd_ms:.1f} ms; drift {j.drift_ms_per_s:+.3f} ms per s")]
    if st:
        out.append("IOI error vs the sender's frame schedule: " + ", ".join(
            f"{k[:-3]} {v:.2f} ms ({v / FRAME_MS:.3f} frames)" for k, v in st.items()))
    if j.lag_hist:
        out.append("per-note latency minus median (4 ms bins): "
                   + ", ".join(f"{a:+d}: {c}" for a, c in j.lag_hist))
    out.append(f"schedule quantisation (7/8-frame pattern vs ideal 125 ms), not jitter: sigma "
               f"{j.quant_sigma_ms:.2f} ms ({j.quant_sigma_ms / FRAME_MS:.3f} frames)")
    out.append("2wiCC's per-frame queue is not our scheduler (the sender grid is quantised at 7/8 frames here; "
               "the real firmware scheduler is Story 2.4/2.5)")
    return out


# ---- capture self-test (steady tone from another device)


@dataclass(frozen=True)
class ToneCheck:
    count: int            # phase steps larger than step_ms where the tone is present
    amp: float            # median block amplitude of the tone
    lead_s: float = 0.0   # low-level time skipped at the start (input start-up, before the tone)
    tail_s: float = 0.0   # low-level time skipped at the end
    dropouts: int = 0     # low-level stretches between the first and the last tone block


STARTUP_GRACE_S = 0.5  # the start of a capture is not judged (macOS input start-up, seen live 2026-11-21)


def tone_discontinuities(x: np.ndarray, sr: int, f0: float = 1000.0, block: float = 0.005,
                         step_ms: float = 0.2, min_rel_db: float = -20.0,
                         grace: float = STARTUP_GRACE_S) -> ToneCheck:
    """Count phase steps larger than `step_ms` in a steady tone.

    Phase per block from the projection on exp(-j 2 pi f0 t); the steady frequency offset of the other
    device is removed with the median phase advance. Each block's phase is compared with the one two
    blocks before, so a step inside a block is seen in full. Only blocks whose tone amplitude is within
    `min_rel_db` of the median take part: the phase of a block without the tone is noise (seen live:
    the first 0.17 s of a recording are the input's start-up silence). Low-level stretches at the start
    and end are reported as skipped time, those in the middle as dropouts. The first `grace` seconds are
    never judged: on macOS the input may deliver a first buffer of tone, then about 150 ms of near-silence
    while it starts (seen live), which is start-up, not a dropout. Dropping a whole number of
    tone periods is invisible (48 samples at 48 kHz for 1 kHz); a 1024-sample packet is not."""
    n = round(block * sr)
    m = x.size // n
    if m < 4:
        return ToneCheck(0, 0.0)
    t = np.arange(m * n) / sr
    z = (x[: m * n] * np.exp(-2j * np.pi * f0 * t)).reshape(m, n).sum(axis=1)
    mag = np.abs(z)
    amp = float(np.median(mag)) * 2.0 / n
    ok = mag >= float(np.median(mag)) * 10 ** (min_rel_db / 20.0)
    ok[: min(m, math.ceil(grace / block))] = False
    if not ok.any():
        return ToneCheck(0, amp)
    first, last = int(np.argmax(ok)), int(m - 1 - np.argmax(ok[::-1]))
    inner = ok[first: last + 1]
    dropouts = int(np.sum(~inner[1:] & inner[:-1]))
    ph = np.angle(z)
    step = np.angle(np.exp(1j * (ph[2:] - ph[:-2])))
    valid = ok[2:] & ok[1:-1] & ok[:-2]
    if not valid.any():
        return ToneCheck(0, amp, first * block, (m - 1 - last) * block, dropouts)
    adv = float(np.median(step[valid]))
    res = np.abs(np.angle(np.exp(1j * (step - adv))))
    bad = (res > 2 * np.pi * f0 * step_ms / 1000.0) & valid
    runs = int(np.sum(bad[1:] & ~bad[:-1]) + bad[0])
    return ToneCheck(runs, amp, first * block, (m - 1 - last) * block, dropouts)


# ---------------------------------------------------------------------------
# Output
# ---------------------------------------------------------------------------

COLUMNS = ("n", "line", "t_press_s", "label", "model", "measured_hz", "note", "cents",
           "level_dbfs", "noise_dbfs", "voiced_pct", "onset_ms", "verdict")


def _fmt(v: float | None, spec: str) -> str:
    return "-" if v is None or not math.isfinite(v) else format(v, spec)


def result_row(r: Result) -> dict[str, str]:
    pr = r.press
    return {
        "n": str(pr.index), "line": str(pr.line), "t_press_s": f"{pr.t_press:.3f}", "label": pr.label,
        "model": g.midi_name(pr.model_midi) if pr.model_midi is not None else "-",
        "measured_hz": _fmt(r.hz, ".1f"), "note": r.note, "cents": _fmt(r.cents, "+.0f"),
        "level_dbfs": _fmt(r.level_dbfs, ".1f"), "noise_dbfs": _fmt(r.noise_dbfs, ".1f"),
        "voiced_pct": f"{r.voiced * 100:.0f}", "onset_ms": _fmt(r.onset_ms, ".0f"), "verdict": r.verdict,
    }


def table_lines(results: Sequence[Result]) -> list[str]:
    head = f"{'n':>3} {'t(s)':>7}  {'label':<52} {'model':<5} {'Hz':>7} {'note':<4} {'cents':>5} " \
           f"{'dBFS':>6} {'SNR':>5} {'onset':>6}  verdict"
    out = [head, "-" * len(head)]
    for r in results:
        row = result_row(r)
        snr = r.level_dbfs - r.noise_dbfs
        out.append(f"{row['n']:>3} {row['t_press_s']:>7}  {row['label'][:52]:<52} {row['model']:<5} "
                   f"{row['measured_hz']:>7} {row['note']:<4} {row['cents']:>5} {row['level_dbfs']:>6} "
                   f"{_fmt(snr, '.0f'):>5} {row['onset_ms']:>6}  {r.verdict}")
    return out


def summary_lines(results: Sequence[Result], offset: float) -> list[str]:
    counts: dict[str, int] = {}
    for r in results:
        counts[r.verdict] = counts.get(r.verdict, 0) + 1
    onsets = [r.onset_ms for r in results if r.onset_ms is not None]
    lines = [f"offset {offset:.3f} s (WAV time of schedule t=0); "
             + ", ".join(f"{k}: {v}" for k, v in sorted(counts.items()))]
    if onsets:
        lines.append(f"onset after scheduled press: median {float(np.median(onsets)):.0f} ms "
                     f"(min {min(onsets):.0f}, max {max(onsets):.0f}); includes USB, console and audio latency")
    return lines


def write_reports(base: pathlib.Path, results: Sequence[Result], meta: dict[str, str]) -> tuple[pathlib.Path, pathlib.Path]:
    csv_path, md_path = base.with_suffix(".csv"), base.with_suffix(".md")
    with open(csv_path, "w", newline="", encoding="utf-8") as fh:
        w = csv.DictWriter(fh, fieldnames=COLUMNS)
        w.writeheader()
        for r in results:
            w.writerow(result_row(r))
    md = [f"# Trial analysis: {meta.get('trial', '')}", ""]
    md += [f"- {k}: {v}" for k, v in meta.items()]
    md += [("- pitch: YIN 300 to 2000 Hz, median over the held note (attack and release skipped); "
            "note names A4 = 440 Hz; model = button + D-pad + stick (reading aid)"), ""]
    md.append("| " + " | ".join(COLUMNS) + " |")
    md.append("|" + "---|" * len(COLUMNS))
    for r in results:
        md.append("| " + " | ".join(result_row(r)[c].replace("|", "/") for c in COLUMNS) + " |")
    md_path.write_text("\n".join(md) + "\n", encoding="utf-8")
    return csv_path, md_path


# ---------------------------------------------------------------------------
# WAV I/O
# ---------------------------------------------------------------------------


def read_wav(path: pathlib.Path) -> tuple[np.ndarray, int]:
    with wave.open(str(path), "rb") as w:
        sr, ch, width, n = w.getframerate(), w.getnchannels(), w.getsampwidth(), w.getnframes()
        raw = w.readframes(n)
    if width == 2:
        x = np.frombuffer(raw, dtype="<i2").astype(np.float64) / 32768.0
    elif width == 4:
        x = np.frombuffer(raw, dtype="<i4").astype(np.float64) / 2147483648.0
    else:
        raise ValueError(f"{path}: {8 * width}-bit WAV not supported (16 or 32-bit PCM)")
    if ch > 1:
        x = x.reshape(-1, ch).mean(axis=1)
    return x, sr


def write_wav(path: pathlib.Path, x: np.ndarray, sr: int) -> None:
    pcm = np.clip(np.round(x * 32767.0), -32768, 32767).astype("<i2")
    with wave.open(str(path), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(sr)
        w.writeframes(pcm.tobytes())


# ---------------------------------------------------------------------------
# Alignment (pure): recording clock vs sender clock, both on the host monotonic clock
# ---------------------------------------------------------------------------

_ANNOUNCE_RE = re.compile(r"t=([0-9.]+)s  (.*)")


def recording_start(chunks: Sequence[tuple[float, int]], sr: int) -> float:
    """Host time of sample 0: min over chunks of (arrival - samples so far / sr). Late chunks only add delay."""
    if not chunks:
        raise ValueError("no audio received")
    return min(arr - total / sr for arr, total in chunks)


def audio_rate(chunks: Sequence[tuple[float, int]], skip: float = 0.5) -> float | None:
    """Delivered samples per host second (least-squares slope), ignoring the first `skip` s of start-up."""
    if len(chunks) < 3:
        return None
    t = np.array([c[0] for c in chunks])
    n = np.array([c[1] for c in chunks], dtype=np.float64)
    keep = t >= t[0] + skip
    if keep.sum() < 3 or t[keep][-1] - t[keep][0] < 1.0:
        return None
    return float(np.polyfit(t[keep], n[keep], 1)[0])


def alignment(chunks: Sequence[tuple[float, int]], sr: int, t_send: float) -> tuple[float, float]:
    """(offset, scale): WAV time of schedule t=0, and WAV seconds per host second.

    If the capture loses samples (seen on 2026-11-20: ffmpeg avfoundation dropped about 9 % in
    1024-sample packets), the WAV runs short of real time; scale < 1 maps schedule times onto it.
    """
    rate = audio_rate(chunks) or float(sr)
    start = min(arr - total / rate for arr, total in chunks)
    scale = rate / sr
    return (t_send - start) * scale, scale


def sender_start(lines: Sequence[tuple[float, str]], announce: Sequence[tuple[int, str]],
                 frame_rate: float = FRAME_RATE) -> float | None:
    """Host time of schedule t=0 from --announce lines matched in order to the file's labelled rows."""
    best = None
    k = 0
    for arr, text in lines:
        m = _ANNOUNCE_RE.fullmatch(text.rstrip("\r\n"))
        if not m or k >= len(announce):
            continue
        frame, label = announce[k]
        if m.group(2) != label:
            break  # out of step: stop using lines rather than guess
        k += 1
        t0 = arr - frame / frame_rate
        best = t0 if best is None else min(best, t0)
    return best


# ---------------------------------------------------------------------------
# Live run: ffmpeg + sender
# ---------------------------------------------------------------------------


class Recorder:
    """ffmpeg avfoundation -> raw s16le on a pipe -> WAV file; arrival times kept for alignment."""

    def __init__(self, device: str, sr: int, wav_path: pathlib.Path, ffmpeg: str = "ffmpeg"):
        self.sr = sr
        self.wav_path = wav_path
        self.chunks: list[tuple[float, int]] = []
        self.total = 0
        # -thread_queue_size: a live device whose packets are not read in time loses them (ffmpeg docs);
        # the default queue is small. stderr is read continuously so a full pipe cannot stall ffmpeg.
        self.cmd = [ffmpeg, "-hide_banner", "-loglevel", "warning", "-nostdin", "-thread_queue_size", "4096",
                    "-f", "avfoundation", "-i", f":{device}", "-ac", "1", "-ar", str(sr), "-f", "s16le", "-"]
        self.errors: list[str] = []
        self.proc: subprocess.Popen | None = None
        self.thread: threading.Thread | None = None
        self.first = threading.Event()

    def start(self, timeout: float = 5.0) -> None:
        # Own session: a terminal Ctrl+C must not cut the recording before the sender's stop.
        self.wav = wave.open(str(self.wav_path), "wb")  # noqa: SIM115 - closed in stop()
        self.wav.setnchannels(1)
        self.wav.setsampwidth(2)
        self.wav.setframerate(self.sr)
        try:
            self.proc = subprocess.Popen(self.cmd, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                         stderr=subprocess.PIPE, start_new_session=True)
        except OSError:
            self.wav.close()
            raise
        self.thread = threading.Thread(target=self._read, daemon=True)
        self.thread.start()
        self.err_thread = threading.Thread(target=self._read_err, daemon=True)
        self.err_thread.start()
        if not self.first.wait(timeout):
            err = self.stop()
            raise RuntimeError(f"no audio from ffmpeg within {timeout:.0f} s (microphone permission for the "
                               f"terminal? device name?) {err}")

    def _read(self) -> None:
        fd = self.proc.stdout.fileno()
        odd = b""
        while True:
            data = os.read(fd, 8192)
            if not data:
                break
            now = time.monotonic()
            data = odd + data
            cut = len(data) - len(data) % 2
            data, odd = data[:cut], data[cut:]
            self.wav.writeframes(data)
            self.total += len(data) // 2
            self.chunks.append((now, self.total))
            self.first.set()

    def _read_err(self) -> None:
        for line in self.proc.stderr:
            self.errors.append(line.decode(errors="replace").rstrip())

    def stop(self) -> str:
        if self.proc is not None:
            if self.proc.poll() is None:
                self.proc.terminate()
            try:
                self.proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait()
        for th in (self.thread, getattr(self, "err_thread", None)):
            if th is not None:
                th.join(timeout=5)
        err = "\n".join(self.errors[-20:])
        if hasattr(self, "wav"):
            self.wav.close()
        return err


class SdRecorder:
    """PortAudio (sounddevice) input -> WAV, lossless unless PortAudio reports an overflow (Story 1.4).

    Same interface as Recorder (start, stop, chunks, total). The callback only copies the block and
    puts it on a SimpleQueue (no locks, no file I/O); a writer thread does the rest. Every callback
    status flag is kept; `overflows` counts input overflows, and any overflow makes the run invalid."""

    def __init__(self, device: str, sr: int, wav_path: pathlib.Path, sd=None):
        self.device = int(device) if str(device).isdigit() else device
        self.sr = sr
        self.wav_path = wav_path
        self.sd = sd
        self.chunks: list[tuple[float, int]] = []
        self.total = 0
        self.overflows = 0
        self.statuses: list[str] = []
        self.q: queue.SimpleQueue = queue.SimpleQueue()
        self.first = threading.Event()
        self.stream = None
        self.thread: threading.Thread | None = None

    def _callback(self, indata, frames, time_info, status) -> None:
        if status:
            if getattr(status, "input_overflow", False):
                self.overflows += 1
            self.statuses.append(str(status))
        self.q.put((time.monotonic(), bytes(indata)))

    def _write(self) -> None:
        while True:
            item = self.q.get()
            if item is None:
                break
            now, data = item
            self.wav.writeframes(data)
            self.total += len(data) // 2
            self.chunks.append((now, self.total))
            self.first.set()

    def start(self, timeout: float = 5.0) -> None:
        if self.sd is None:
            try:
                import sounddevice as sd  # optional: only this backend needs PortAudio
            except (ImportError, OSError) as e:
                raise RuntimeError(f"--backend sounddevice needs the sounddevice package ({e})") from None
            self.sd = sd
        self.wav = wave.open(str(self.wav_path), "wb")  # noqa: SIM115 - closed in stop()
        self.wav.setnchannels(1)
        self.wav.setsampwidth(2)
        self.wav.setframerate(self.sr)
        self.thread = threading.Thread(target=self._write, daemon=True)
        self.thread.start()
        try:
            self.stream = self.sd.RawInputStream(samplerate=self.sr, channels=1, dtype="int16", device=self.device,
                                                 callback=self._callback)
            self.stream.start()
        except Exception as e:  # noqa: BLE001 - PortAudio errors have their own types
            self.stop()
            raise RuntimeError(f"cannot open the input device {self.device!r}: {e}") from None
        if not self.first.wait(timeout):
            err = self.stop()
            raise RuntimeError(f"no audio from the input device within {timeout:.0f} s (microphone permission "
                               f"for the terminal? device name?) {err}")

    def stop(self) -> str:
        if self.stream is not None:
            try:
                self.stream.stop()
                self.stream.close()
            finally:
                self.stream = None
        if self.thread is not None:
            self.q.put(None)
            self.thread.join(timeout=5)
            self.thread = None
        if hasattr(self, "wav"):
            self.wav.close()
        if self.statuses:
            return f"PortAudio status flags in {len(self.statuses)} callback(s), {self.overflows} input overflow(s): " \
                   f"{sorted(set(self.statuses))}"
        return ""


def make_recorder(backend: str, device: str, sr: int, wav_path: pathlib.Path, ffmpeg: str = "ffmpeg"):
    if backend == "sounddevice":
        return SdRecorder(device, sr, wav_path)
    return Recorder(device, sr, wav_path, ffmpeg)


def run_sender(port: str, trial: pathlib.Path, extra: Sequence[str]) -> tuple[int, list[tuple[float, str]], bool]:
    """Run the unchanged sender with --announce; echo and timestamp its stdout. Returns (code, lines, interrupted)."""
    return run_streaming([sys.executable, "-u", str(SENDER), "--port", port, "--announce", *extra, str(trial)])


# Story 2.5 D16: the firmware's selfplay as the driver of a live run (instead of the 2wiCC sender).
_T0_RE = re.compile(r"T0_HOST_MONO=([0-9]+(?:\.[0-9]+)?)")
_PASTE_RE = re.compile(r"PASTE_LINE: run \d+ \| late max (\d+) us.*?\| IN median (\d+),")


def driver_argv(cmd: str) -> list[str]:
    """The driver command line: environment variables expanded (so '"$P"' inside single quotes works), then
    split as a shell would."""
    return shlex.split(os.path.expandvars(cmd))


def run_driver(cmd: str) -> tuple[int, list[tuple[float, str]], bool]:
    """Run the driver (e.g. link_probe.py selfplay); echo and timestamp its output."""
    return run_streaming(driver_argv(cmd), stderr_too=True)


def driver_t0(lines: Sequence[tuple[float, str]]) -> float | None:
    """Host monotonic time of the controller's t0, from the driver's first T0_HOST_MONO= line. The recorder
    stamps its chunks with the same clock (time.monotonic), so `alignment` turns it into a WAV offset."""
    for _arr, text in lines:
        m = _T0_RE.search(text)
        if m:
            return float(m.group(1))
    return None


def driver_self_timing(lines: Sequence[tuple[float, str]]) -> tuple[float, float] | None:
    """(late_max_us, report_period_us) from the driver's first PASTE_LINE, or None."""
    for _arr, text in lines:
        m = _PASTE_RE.search(text)
        if m:
            return float(m.group(1)), float(m.group(2))
    return None


def run_streaming(cmd: Sequence[str], stderr_too: bool = False) -> tuple[int, list[tuple[float, str]], bool]:
    """Run `cmd`, echo and timestamp its stdout. Returns (code, lines, interrupted)."""
    # Same process group: a real terminal Ctrl+C reaches the child directly (its own stop sequence).
    proc = subprocess.Popen(list(cmd), stdout=subprocess.PIPE, stderr=subprocess.STDOUT if stderr_too else None,
                            text=True, bufsize=1)
    lines: list[tuple[float, str]] = []

    def pump():
        for line in proc.stdout:
            lines.append((time.monotonic(), line))
            print(line, end="", flush=True)

    t = threading.Thread(target=pump, daemon=True)
    t.start()
    interrupted = False
    forwarded = False
    while True:
        try:
            code = proc.wait()
            break
        except KeyboardInterrupt:
            interrupted = True
            # The terminal already sent SIGINT to the sender; forward once only if it did not get it.
            try:
                proc.wait(timeout=0.5)
            except subprocess.TimeoutExpired:
                if not forwarded:
                    proc.send_signal(signal.SIGINT)
                    forwarded = True
            except KeyboardInterrupt:
                pass
    t.join(timeout=2)
    return code, lines, interrupted


def list_devices(ffmpeg: str) -> int:
    r = subprocess.run([ffmpeg, "-hide_banner", "-f", "avfoundation", "-list_devices", "true", "-i", ""],
                       check=False, capture_output=True, text=True)
    print(r.stderr)
    return 0


# ---------------------------------------------------------------------------
# Story 1.6: melody (NOTE rows) and sustain (U1 TRY rows)
# ---------------------------------------------------------------------------

STORY_1_6_LATENCY = 0.13  # default heard latency for melody/sustain, s (Story 1.4 calibration: 130 to 140 ms)
_NOTE_RE = re.compile(r"NOTE (\d+) (\S+) (\d+) fingering=(\S+)")


@dataclass(frozen=True)
class MelodyNote:
    index: int
    line: int
    t: float          # schedule s of the press
    name: str
    midi: int         # expected sounding pitch (from the label, the generator's profile fingering)
    fingering: str
    label: str
    t_release: float | None = None  # schedule s of the first release of the note's button (or hand-over)


def melody_notes_from_rows(rows: Sequence, frame_rate: float = FRAME_RATE) -> list[MelodyNote]:
    """Every `NOTE` row of a Story 1.6 melody file (gen_trials_1_6.py labels), in order."""
    out = []
    frames = [s.time_to_frame(r.time, frame_rate) for r in rows]
    for i, r in enumerate(rows):
        m = _NOTE_RE.match(r.label)
        if m:
            j = i + 1
            while j < len(rows) and rows[j].state.buttons == r.state.buttons:
                j += 1
            rel = frames[j] / frame_rate if j < len(rows) else None
            out.append(MelodyNote(int(m.group(1)), r.line, frames[i] / frame_rate, m.group(2), int(m.group(3)),
                                  m.group(4), r.label, rel))
    return out


def first_drop(t: np.ndarray, e: np.ndarray, t_from: float, t_to: float, thr: float, run: int) -> float | None:
    """Time of the first sample in [t_from, t_to] below `thr` that stays below it for `run` samples
    (the U1 interruption rule; also the melody "cut short" rule)."""
    for i in np.flatnonzero((t >= t_from) & (t <= t_to)):
        if e[i] < thr and i + run <= e.size and np.all(e[i: i + run] < thr):
            return float(t[i])
    return None


def band_mean_db(x: np.ndarray, sr: int, t0: float, t1: float, f: float, width: float = 0.03) -> float:
    """`band_power_db` divided by the window length: comparable between windows of different lengths
    (melody notes are 0.2 to 1.3 s, the quiet reference 0.4 s; band_power_db grows with the length)."""
    n = round((t1 - t0) * sr)
    v = band_power_db(x, sr, t0, t1, f, width)
    return v - 10.0 * math.log10(n) if n > 0 and math.isfinite(v) else -math.inf


def max_rise(env: np.ndarray) -> float:
    """Largest rise of `env` above the lowest value before it (an attack, from silence or from a dip)."""
    if env.size < 2:
        return 0.0
    return float(np.max(env - np.minimum.accumulate(env)))


def count_reattacks(env: np.ndarray, dip_db: float, rise_db: float, min_dip: int = 1,
                    reach_db: float = math.inf) -> int:
    """Dips of at least `dip_db` below the running peak that last at least `min_dip` samples, each followed
    by a rise of `rise_db` from the dip back to within `reach_db` of that peak: every one is a new attack
    (the `dip_and_rise` rule, counted instead of yes/no). A shorter dip (a click or a room transient over a
    held note) is not a new attack, nor is noise or another sound in the silence after a note stopped."""
    n, peak, low, below, in_dip = 0, -math.inf, math.inf, 0, False
    for v in env:
        if not in_dip:
            peak = max(peak, v)
            if peak - v >= dip_db:
                in_dip, low, below = True, v, 1
        else:
            low = min(low, v)
            if peak - v >= dip_db:
                below += 1
            if v - low >= rise_db and v >= peak - reach_db:
                if below >= min_dip:
                    n += 1
                    peak = v
                else:
                    peak = max(peak, v)
                in_dip = False
    return n


@dataclass(frozen=True)
class MelodyParams:
    """Thresholds of `--mode melody`; every value is printed in the report."""
    skip: float = 0.040        # window starts at the heard onset + this (stick start glide, row 2.3: ~30 ms)
    end_guard: float = 0.020   # ... and ends this before the next heard onset
    last_window: float = 0.5   # the last note: heard onset + this
    search_semitones: float = 3.0   # pitch = strongest peak within this of the expected pitch
    tune_cents: float = 50.0   # in tune: within this of the run's median for the pitch class
    dip_db: float = 8.0        # onsets: dip below the running peak ...
    rise_db: float = 8.0       # ... and rise from the dip (or from silence) for an attack
    start_span: float = 0.12   # the attack of a note is searched within +- this of its heard onset
    min_dip: float = 0.030     # an extra attack needs a dip at least this long (a same-button re-press: 83 ms)
    reach_db: float = 10.0     # ... and a re-rise to within this of the level before the dip
    cut_db: float = 20.0       # cut short: the tone falls this far below its own start level (or under the SNR)
    cut_min_s: float = 0.150   # ... for at least this long ...
    cut_before_s: float = 0.100  # ... starting more than this before the scheduled release (U1 rule)
    early_db: float = 6.0      # early stop near the release: the tone falls this far below its start ...
    early_margin_s: float = 0.075  # ... and stays down until the heard release, falling more than this before it
    release_tail: float = 0.100  # a note followed by a rest is judged up to its heard release + this
    early_window_s: float = 0.300  # ... but at most this before it (earlier falls: the 150 ms rule; a long sag is not a stop)
    env_win: float = 0.030     # tone envelope window (separates a semitone at the ocarina's pitches)
    env_hop: float = 0.005


@dataclass(frozen=True)
class MelodyResult:
    note: MelodyNote
    t_heard: float
    level_db: float
    noise_db: float
    audible: bool
    hz: float | None
    heard_midi: int | None
    cents: float | None        # vs equal temperament, A4 = 440 Hz
    dev_cents: float | None    # vs the run's median for the pitch class
    onsets: int
    verdict: str
    sounded_ms: float | None = None  # cut short: heard onset to the drop, schedule ms (None = not cut)


def melody_analyse(x: np.ndarray, sr: int, notes: Sequence[MelodyNote], offset: float, p: Params = Params(),
                   mp: MelodyParams = MelodyParams()) -> list[MelodyResult]:
    """Per note: audible at the expected pitch, pitch (peak within +-3 semitones, so a wrong neighbour is
    found), attacks around the onset and inside the note, then the in-tune rule on the whole run."""
    if not notes:
        return []
    xb = bandpass(x, sr, *p.band)
    heard = [wav_time(n.t, offset, p) for n in notes]
    q0, q1 = heard[0] - 0.45, heard[0] - 0.05  # quiet reference: the lead-in before the first note
    dur = x.size / sr
    raw = []
    for k, n in enumerate(notes):
        h = heard[k]
        w0 = h + mp.skip
        w1 = heard[k + 1] - mp.end_guard if k + 1 < len(notes) else h + mp.last_window
        if w1 > dur:
            break
        tr = wav_time(n.t_release, offset, p) if n.t_release is not None else None
        if tr is not None:
            # A short (staccato) press followed by a rest: judge the sounding part only, not the silence after
            # it (live 2026-11-21, --max-press-ms 400: a 0.4 s G5 in a 2 s slot read 9.9 dB and NO SOUND).
            w1 = max(w0 + 0.05, min(w1, tr + mp.release_tail))
        f = expected_hz(n.midi)
        level, noise = band_mean_db(xb, sr, w0, w1, f), band_mean_db(xb, sr, q0, q1, f)
        audible = math.isfinite(level) and (not math.isfinite(noise) or level - noise >= p.min_snr_db)
        span = 2 ** (mp.search_semitones / 12.0)
        peak = spectral_peak(xb, sr, w0, w1, f / span, f * span)
        hz = None
        if peak is not None:
            # Another pitch counts only if it is still there in the second half of the window: the decaying
            # tail of the previous note (a missed note after a hand-over) is not a wrong note.
            mid = (w0 + w1) / 2
            lp, npk = band_mean_db(xb, sr, mid, w1, peak), band_mean_db(xb, sr, q0, q1, peak)
            if audible or (math.isfinite(lp) and (not math.isfinite(npk) or lp - npk >= p.min_snr_db)):
                hz = peak
        # attacks, on the level of the tone at the expected pitch
        t, e = tone_env_db(xb, sr, f, h - mp.start_span - mp.env_win, w1 + mp.env_win, mp.env_win, mp.env_hop)
        _, en = tone_env_db(xb, sr, f, q0, q1, mp.env_win, mp.env_hop)
        if en.size:
            e = np.maximum(e, float(np.median(en)) + 6.0)  # noise wobble is not an attack
        start = e[(t >= h - mp.start_span) & (t <= h + mp.start_span)]
        # no new attack without a press: after the heard release only the release tail is left (a G5 released
        # with the stick still up dipped and bounced back 5 dB within 50 ms, live 2026-11-21)
        body_end = w1 - 0.04 if tr is None else min(w1 - 0.04, tr - 0.02)
        body = e[(t >= h + mp.start_span / 2) & (t <= body_end)]
        onsets = int(max_rise(start) >= mp.rise_db) + count_reattacks(body, mp.dip_db, mp.rise_db,
                                                                      max(1, round(mp.min_dip / mp.env_hop)), mp.reach_db)
        if not audible:
            onsets = 0  # attacks of the expected note only count if the note sounds at all
        midi, c = note_and_cents(hz) if hz is not None else (None, None)
        sounded = None
        if audible and n.t_release is not None:
            sounded = melody_cut(xb, sr, f, h, wav_time(n.t_release, offset, p), (q0, q1), p, mp)
        raw.append((n, h, level, noise, audible, hz, midi, c, onsets, sounded))
    by_pc: dict[int, list[float]] = {}
    for n, *_, midi, c, _o, _s in raw:
        if midi == n.midi:
            by_pc.setdefault(n.midi % 12, []).append(c)
    med = {pc: float(np.median(v)) for pc, v in by_pc.items()}
    out = []
    for n, h, level, noise, audible, hz, midi, c, onsets, sounded in raw:
        dev = c - med[n.midi % 12] if midi == n.midi else None
        if hz is None:
            verdict = "NO SOUND"
        elif midi != n.midi or abs(dev) > mp.tune_cents:
            verdict = "WRONG PITCH"
        elif not audible:
            verdict = "NO SOUND"
        elif sounded is not None:
            verdict = "CUT SHORT"
        else:
            verdict = "ok"
        out.append(MelodyResult(n, h, level, noise, audible, hz, midi, c, dev, onsets, verdict, sounded))
    return out


def melody_cut(xb: np.ndarray, sr: int, f: float, h: float, tr: float, quiet: tuple[float, float], p: Params,
               mp: MelodyParams) -> float | None:
    """Cut short (the sustain-mode interruption rule on a melody note): the tone at the expected pitch falls
    `cut_db` below its own level at the start of the note, or under `min_snr_db` above the lead-in, for at
    least `cut_min_s`, starting more than `cut_before_s` before the heard release. Returns how long the note
    sounded (heard onset to the drop, schedule ms), or None if it was held to its release. Also an early stop
    near the release (`early_stop`): it falls `early_db` and stays down from more than `early_margin_s` before
    the heard release."""
    t, e = tone_env_db(xb, sr, f, h - 0.05, tr + mp.cut_min_s + 0.05, mp.env_win, mp.env_hop)
    _, en = tone_env_db(xb, sr, f, *quiet, mp.env_win, mp.env_hop)
    head = e[(t >= h + mp.skip) & (t <= min(h + mp.skip + 0.2, tr - mp.cut_before_s))]
    if not head.size:
        return None
    ref = float(np.median(head))
    thr = ref - mp.cut_db
    if en.size:
        thr = max(thr, float(np.median(en)) + p.min_snr_db)
    drop = first_drop(t, e, h + mp.skip, tr - mp.cut_before_s, thr, max(1, round(mp.cut_min_s / mp.env_hop)))
    if drop is None:
        drop = early_stop(t, e, max(h + mp.skip, tr - mp.early_window_s - 0.05), tr, ref - mp.early_db,
                          mp.early_margin_s, mp.early_window_s)
    return None if drop is None else (drop - h) * 1000.0 / p.scale


def early_stop(t: np.ndarray, e: np.ndarray, t_from: float, tr: float, thr: float, margin: float,
               window: float = math.inf) -> float | None:
    """An early stop close to the release, which the 150 ms rule cannot see when the next note comes soon
    (seen live 2026-11-21: A5 x 17 at 60 BPM, notes 07 and 14 stopped about 110 ms before their heard
    release, other notes started to fall about 10 ms after it; with the room tail the 30 ms envelope only
    fell 10 to 14 dB). The time the tone falls below `thr` for good, i.e. it stays below until the heard
    release `tr`, if that is between `window` and `margin` before `tr`; else None."""
    sel = np.flatnonzero((t >= t_from) & (t <= tr))
    if sel.size == 0 or e[sel[-1]] >= thr:
        return None
    above = sel[e[sel] >= thr]
    if above.size == 0:
        return None  # never above: no sound to stop (a NO SOUND note is judged elsewhere)
    i = int(above[-1]) + 1  # first sample of the final stretch below thr
    return float(t[i]) if tr - window <= t[i] < tr - margin else None


MELODY_COLUMNS = ("n", "line", "t_s", "label", "expected", "fingering", "snr_db", "audible", "hz", "note",
                  "cents", "dev_cents", "onsets", "sounded_ms", "verdict")


def melody_row(r: MelodyResult) -> dict[str, str]:
    n = r.note
    return {"n": f"{n.index:02d}", "line": str(n.line), "t_s": f"{n.t:.3f}", "label": n.label, "expected": n.name,
            "fingering": n.fingering, "snr_db": _fmt(r.level_db - r.noise_db, ".1f"),
            "audible": "1" if r.audible else "0", "hz": _fmt(r.hz, ".1f"),
            "note": g.midi_name(r.heard_midi) if r.heard_midi is not None else "-",
            "cents": _fmt(r.cents, "+.0f"), "dev_cents": _fmt(r.dev_cents, "+.0f"), "onsets": str(r.onsets),
            "sounded_ms": _fmt(r.sounded_ms, ".0f"), "verdict": r.verdict}


def melody_summary_lines(results: Sequence[MelodyResult], p: Params, mp: MelodyParams,
                         meta: dict[str, str] | None = None) -> list[str]:
    n = len(results)
    audible = sum(r.audible for r in results)
    right = sum(r.verdict in ("ok", "CUT SHORT") for r in results)
    cut = [r for r in results if r.verdict == "CUT SHORT"]
    missed = [r for r in results if r.verdict == "NO SOUND" or r.onsets == 0]
    extra = [r for r in results if r.onsets > 1]
    devs = [r.dev_cents for r in results if r.dev_cents is not None]
    out = [(f"thresholds: latency {p.latency:.3f} s; window heard onset + {mp.skip * 1000:.0f} ms to next heard "
           f"onset - {mp.end_guard * 1000:.0f} ms (last note {mp.last_window:g} s); audible: level at the expected "
           f"pitch (+-3 %) >= {p.min_snr_db:g} dB above the lead-in; pitch: peak within +-{mp.search_semitones:g} "
           f"semitones; in tune: nearest note = expected and within +-{mp.tune_cents:g} c of the run's median for "
           f"the pitch class; attack: rise >= {mp.rise_db:g} dB within +-{mp.start_span * 1000:.0f} ms of the heard "
           f"onset, extra attack: dip >= {mp.dip_db:g} dB for >= {mp.min_dip * 1000:.0f} ms then rise >= {mp.rise_db:g} dB to within {mp.reach_db:g} dB of the level "
           f"before the dip, inside the note "
           f"(tone envelope {mp.env_win * 1000:.0f} ms); cut short: the tone falls >= {mp.cut_db:g} dB below its "
           f"start (or under {p.min_snr_db:g} dB above the lead-in) for >= {mp.cut_min_s * 1000:.0f} ms, starting "
           f"> {mp.cut_before_s * 1000:.0f} ms before the scheduled release, or it falls >= {mp.early_db:g} dB and stays "
           f"down from {mp.early_margin_s * 1000:.0f} to {mp.early_window_s * 1000:.0f} ms before the heard release")]
    meds: dict[str, list[float]] = {}
    for r in results:
        if r.dev_cents is not None:
            meds.setdefault(g.NOTE_NAMES[r.note.midi % 12], []).append(r.cents)
    out.append("pitch-class medians (c vs A4 = 440 Hz): " +
               (", ".join(f"{k} {float(np.median(v)):+.0f}" for k, v in sorted(meds.items())) or "-"))
    spread = f"{min(devs):+.0f} to {max(devs):+.0f} c, sd {float(np.std(devs)):.1f} c" if devs else "-"
    out.append(f"notes {n}, audible {audible}/{n}, right pitch {right}/{n}, missed {len(missed)}, cut {len(cut)}, "
               f"extra onsets {sum(r.onsets - 1 for r in extra)}, cents spread {spread}")
    if missed:
        out.append("missed: " + ", ".join(f"{r.note.index:02d} {r.note.name} {r.note.fingering}"
                                          f" ({r.verdict if r.verdict == 'NO SOUND' else 'no attack'})"
                                          for r in missed))
    if cut:
        out.append("cut short: " + ", ".join(
            f"{r.note.index:02d} {r.note.name} {r.note.fingering} sounded {r.sounded_ms:.0f} of "
            f"{(r.note.t_release - r.note.t) * 1000:.0f} ms" for r in cut))
    wrong = [r for r in results if r.verdict == "WRONG PITCH"]
    if wrong:
        out.append("wrong pitch: " + ", ".join(
            f"{r.note.index:02d} {r.note.name} {r.note.fingering} heard "
            f"{g.midi_name(r.heard_midi) if r.heard_midi is not None else '-'} {_fmt(r.dev_cents, '+.0f')}"
            for r in wrong))
    if extra:
        out.append("extra onsets: " + ", ".join(f"{r.note.index:02d} {r.note.name} ({r.onsets})" for r in extra))
    if meta and meta.get("valid", "yes").startswith("no"):
        out.append("INVALID RUN: PortAudio input overflow, samples were lost; run it again")
    return out


# ---- Story 2.5: inter-onset statistics of a melody, and the explanation of AC3


ONSET_WIN = 0.020   # tone envelope window for onsets (the Spike 2.4 jitter definition)
ONSET_HOP = 0.001
ONSET_BEFORE = 0.070  # an onset is searched in [heard - 70 ms, heard + 100 ms] (as analyse_jitter)
ONSET_AFTER = 0.100
# Story 2.5 D16, D26: the allowance for the analyser's own noise in an IOI p95. The Spike 2.4 onset method
# recovers a known onset sigma within 1 ms on synthetic notes (test_jitter_recovers_known_sigma); 1 ms per
# onset gives an IOI sigma of 1.4 ms and a p95 of |error| of about 2.8 ms; rounded up to 4 ms (one bin of the
# Spike 2.4 latency histogram).
ANALYSER_NOISE_MS = 4.0
GAME_FLOOR_FACTOR = 1.0 - math.sqrt(0.05)  # p95 of |U1 - U2| for U1, U2 uniform on [0, F]: 0.776 F


def melody_onsets(x: np.ndarray, sr: int, notes: Sequence[MelodyNote], offset: float, p: Params,
                  audible: Sequence[bool]) -> list[float | None]:
    """Onset of every audible note, WAV s: the -6 dB crossing before the peak of the tone envelope at the
    expected pitch within [heard - 70 ms, heard + 100 ms] (the Spike 2.4 jitter definition, per note pitch).
    None for a note that is not audible or has no peak in its window."""
    xb = bandpass(x, sr, *p.band)
    out: list[float | None] = []
    for n, ok in zip(notes, audible):
        if not ok:
            out.append(None)
            continue
        h = wav_time(n.t, offset, p)
        t, e = tone_env_db(xb, sr, expected_hz(n.midi), h - ONSET_BEFORE - ONSET_WIN, h + ONSET_AFTER + ONSET_WIN,
                           ONSET_WIN, ONSET_HOP)
        sel = np.flatnonzero((t >= h - ONSET_BEFORE) & (t <= h + ONSET_AFTER))
        if sel.size < 3:
            out.append(None)
            continue
        k = int(sel[0] + np.argmax(e[sel]))
        out.append(half_crossing(t, e, k))
    return out


@dataclass(frozen=True)
class IoiStats:
    """IOI error of consecutive heard notes (measured onset gap minus scheduled gap), the Spike 2.4 definitions."""
    pairs: int
    mean_ms: float
    sigma_ms: float
    p95_ms: float      # of |error|
    max_ms: float      # of |error|
    drift_ms_per_s: float
    missing: tuple[int, ...]  # note indices with no onset (excluded from the pairs)


def ioi_stats(onsets: Sequence[float | None], sched: Sequence[float], offset: float, p: Params,
              indices: Sequence[int] | None = None) -> IoiStats:
    """Pairs of consecutive notes that both have an onset; the error is independent of `offset` (it only
    locates windows) and of a constant latency; `p.scale` maps WAV s to schedule s."""
    idx = list(indices) if indices is not None else list(range(1, len(sched) + 1))
    sch = np.asarray(sched, dtype=float)
    pairs = [(i, i + 1) for i in range(len(sch) - 1) if onsets[i] is not None and onsets[i + 1] is not None]
    err = np.array([((onsets[j] - onsets[i]) / p.scale - (sch[j] - sch[i])) * 1000.0 for i, j in pairs])
    keep = [i for i in range(len(sch)) if onsets[i] is not None]
    missing = tuple(idx[i] for i in range(len(sch)) if onsets[i] is None)
    lag = np.array([((onsets[i] - offset - p.latency) / p.scale - sch[i]) * 1000.0 for i in keep])
    drift = float(np.polyfit(sch[keep], lag, 1)[0]) if len(keep) >= 3 else math.nan
    if err.size == 0:
        return IoiStats(0, math.nan, math.nan, math.nan, math.nan, drift, missing)
    a = np.abs(err)
    return IoiStats(int(err.size), float(np.mean(err)), float(np.std(err, ddof=1)) if err.size > 1 else math.nan,
                    float(np.percentile(a, 95)), float(np.max(a)), drift, missing)


def ioi_lines(st: IoiStats) -> list[str]:
    if st.pairs == 0:
        return ["IOI error: no pair of consecutive notes with onsets"]
    out = [f"IOI error (heard onset gap minus scheduled gap, {st.pairs} pairs): mean {st.mean_ms:+.2f} ms, "
           f"sigma {st.sigma_ms:.2f} ms ({st.sigma_ms / FRAME_MS:.3f} frames), p95 {st.p95_ms:.2f} ms "
           f"({st.p95_ms / FRAME_MS:.3f} frames), max {st.max_ms:.2f} ms ({st.max_ms / FRAME_MS:.3f} frames), "
           f"drift {st.drift_ms_per_s:+.3f} ms per s"]
    if st.missing:
        out.append("no onset (excluded from the pairs): " + ", ".join(f"{i:02d}" for i in st.missing))
    return out


def explain_ioi(ioi_p95_ms: float, late_max_us: float, in_period_us: float, frame_ms: float = FRAME_MS,
                analyser_noise_ms: float = ANALYSER_NOISE_MS) -> dict[str, object]:
    """AC3's second branch as a computation (Story 2.5 D16). firmware bound = the scheduler's largest
    lateness plus one whole polling window; game floor = the p95 of the difference of two onsets each
    quantised to the game's frame (two independent uniform errors: triangular, p95 = 0.776 frame); residual =
    the measured p95 minus the root-sum-square of the two. Verdict MET (p95 <= 1 frame), EXPLAINED (above,
    with the firmware bound <= half a frame and the residual within the analyser noise) or UNEXPLAINED."""
    fw = late_max_us / 1000.0 + in_period_us / 1000.0
    floor = GAME_FLOOR_FACTOR * frame_ms
    residual = ioi_p95_ms - math.sqrt(fw * fw + floor * floor)
    if ioi_p95_ms <= frame_ms:
        verdict = "MET"
    elif fw <= frame_ms / 2.0 and residual <= analyser_noise_ms:
        verdict = "EXPLAINED"
    else:
        verdict = "UNEXPLAINED"
    return {"firmware_bound_ms": fw, "game_floor_ms": floor, "residual_ms": residual, "verdict": verdict}


def explain_lines(ex: dict[str, object], ioi_p95_ms: float, late_max_us: float, in_period_us: float) -> list[str]:
    return [f"AC3 (Story 2.5 D16): IOI p95 {ioi_p95_ms:.2f} ms vs 1 frame {FRAME_MS:.2f} ms; firmware bound "
            f"{ex['firmware_bound_ms']:.2f} ms (late max {late_max_us:.0f} us + IN period {in_period_us:.0f} us), "
            f"game floor {ex['game_floor_ms']:.2f} ms (0.776 frame), residual {ex['residual_ms']:+.2f} ms "
            f"(allowance {ANALYSER_NOISE_MS:g} ms): {ex['verdict']}"
            + ("" if ex["verdict"] != "UNEXPLAINED" else " (a follow-up for Epic 7, not hidden)")]


def report_melody(x: np.ndarray, sr: int, notes: Sequence[MelodyNote], offset: float, p: Params,
                  mp: MelodyParams, base: pathlib.Path, meta: dict[str, str],
                  self_timing: tuple[float, float] | None = None) -> int:
    meta = {**meta, "mode": "melody", "offset_s": f"{offset:.3f}", "scale": f"{p.scale:.4f}",
            "latency_s": f"{p.latency:.3f}", "min_snr_db": f"{p.min_snr_db:g}"}
    results = melody_analyse(x, sr, notes, offset, p, mp)
    rows = [melody_row(r) for r in results]
    head = f"{'n':>3} {'t(s)':>7}  {'exp':<4} {'fingering':<12} {'SNR':>5} {'Hz':>7} {'note':<4} {'c':>4} " \
           f"{'dev':>4} {'on#':>3} {'sound':>5}  verdict"
    print()
    print(head)
    print("-" * len(head))
    for r in rows:
        print(f"{r['n']:>3} {r['t_s']:>7}  {r['expected']:<4} {r['fingering']:<12} {r['snr_db']:>5} {r['hz']:>7} "
              f"{r['note']:<4} {r['cents']:>4} {r['dev_cents']:>4} {r['onsets']:>3} {r['sounded_ms']:>5}  {r['verdict']}")
    summary = melody_summary_lines(results, p, mp, meta)
    # Story 2.5 D16: inter-onset statistics of the heard notes, and the AC3 explanation when the firmware's
    # self-timing is known (selfplay's PASTE_LINE through --driver, or --late-max-us and --period-us).
    done = [r.note for r in results]
    onsets = melody_onsets(x, sr, done, offset, p, [r.audible for r in results])
    st = ioi_stats(onsets, [n.t for n in done], offset, p, [n.index for n in done])
    summary += ioi_lines(st)
    if self_timing is not None and st.pairs > 0:
        late_max_us, period_us = self_timing
        summary += explain_lines(explain_ioi(st.p95_ms, late_max_us, period_us), st.p95_ms, late_max_us, period_us)
    print("\n".join(summary))
    if len(results) < len(notes):
        print(f"note: {len(notes) - len(results)} note(s) not analysed (recording ends before them)")
    csv_path, md_path = write_table(base, MELODY_COLUMNS, rows, meta, summary)
    print(f"reports: {csv_path}  {md_path}")
    return 0


# ---- U1 sustain


@dataclass(frozen=True)
class SustainParams:
    """Thresholds of `--mode sustain` (owner decision 1, Story 1.6); every value is printed in the report."""
    drop_db: float = 20.0          # interrupted: 20 dB below the note's own level in its first second ...
    min_drop_s: float = 0.150      # ... (or under min_snr_db above the noise) for at least this long ...
    before_release_s: float = 0.100  # ... starting at least this before the scheduled release
    env_win: float = 0.020
    env_hop: float = 0.005


@dataclass(frozen=True)
class SustainResult:
    attempt: Attempt
    hold_ms: int | None
    sound: bool
    level_db: float               # median level in the first second after the attack
    noise_db: float
    onset_ms: float | None        # heard onset after the scheduled press (incl. latency shift), schedule ms
    interrupted: bool | None
    stop_ms: float | None         # auto-stop: time of the drop after the heard onset, schedule ms
    level_per_s: tuple[float | None, ...] = ()   # each full second vs the first second, dB
    cents_per_s: tuple[float | None, ...] = ()   # each full second vs the first second, c
    verdict: str = ""


def sustain_attempt(xb: np.ndarray, sr: int, a: Attempt, offset: float, p: Params = Params(),
                    sp: SustainParams = SustainParams()) -> SustainResult:
    hold = a.fields.get("hold_ms")
    tp = wav_time(a.t, offset, p)
    tr = wav_time(a.t + (hold or 1000) / 1000.0, offset, p)
    f = expected_hz(a.model_midi if a.model_midi is not None else g.BUTTON_MIDI.get(a.button, 81))
    t, e = tone_env_db(xb, sr, f, tp - 0.5, tr + 0.5, sp.env_win, sp.env_hop)
    pre = e[(t >= tp - 0.45) & (t <= tp - 0.05)]
    noise = float(np.median(pre)) if pre.size else -math.inf
    if a.blank:
        body = e[(t >= tp + 0.05) & (t <= tp + 1.0)]
        level = float(np.median(body)) if body.size else -math.inf
        sound = math.isfinite(level) and level - noise >= p.min_snr_db
        return SustainResult(a, None, sound, level, noise, None, None, None,
                             verdict="BLANK SOUNDED" if sound else "silent (ok)")
    win = np.flatnonzero((t >= tp - 0.1) & (t <= tp + 0.5))
    if win.size == 0:
        return SustainResult(a, hold, False, -math.inf, noise, None, None, None, verdict="NO SOUND")
    k = int(win[np.argmax(e[win])])
    onset = half_crossing(t[win[0]: k + 1], e[win[0]: k + 1], k - win[0])
    first = e[(t >= onset + 0.1) & (t <= min(onset + 1.0, tr - 0.05))]
    level = float(np.median(first)) if first.size else -math.inf
    on_ms = (onset - tp) * 1000.0 / p.scale
    if not math.isfinite(level) or (math.isfinite(noise) and level - noise < p.min_snr_db):
        return SustainResult(a, hold, False, level, noise, on_ms, None, None, verdict="NO SOUND")
    thr = max(level - sp.drop_db, noise + p.min_snr_db if math.isfinite(noise) else -math.inf)
    run = max(1, round(sp.min_drop_s / sp.env_hop))
    drop = first_drop(t, e, onset + 0.1, tr - sp.before_release_s, thr, run)
    stop = None if drop is None else (drop - onset) * 1000.0 / p.scale
    levels, cents_ = [], []
    ref_hz = spectral_peak(xb, sr, onset, onset + 1.0, f * 0.94, f * 1.06)
    ref_lv = e[(t >= onset) & (t < onset + 1.0)]
    ref_lv = float(np.median(ref_lv)) if ref_lv.size else level
    sec = 0
    while onset + sec + 1.0 <= tr + 1e-9:
        seg = e[(t >= onset + sec) & (t < onset + sec + 1.0)]
        lv = float(np.median(seg)) if seg.size else None
        levels.append(None if lv is None else lv - ref_lv)
        hz = spectral_peak(xb, sr, onset + sec, onset + sec + 1.0, f * 0.94, f * 1.06)
        loud = lv is not None and (not math.isfinite(noise) or lv - noise >= p.min_snr_db)
        cents_.append(cents(hz, ref_hz) if loud and hz and ref_hz else None)
        sec += 1
    verdict = f"interrupted at {stop:.0f} ms" if stop is not None else "held to release"
    return SustainResult(a, hold, True, level, noise, on_ms, stop is not None, stop, tuple(levels), tuple(cents_),
                         verdict)


def sustain_analyse(x: np.ndarray, sr: int, atts: Sequence[Attempt], offset: float, p: Params = Params(),
                    sp: SustainParams = SustainParams()) -> list[SustainResult]:
    xb = bandpass(x, sr, *p.band)
    dur = x.size / sr
    keep = [a for a in atts if wav_time(a.t + a.fields.get("hold_ms", 1000) / 1000.0, offset, p) + 0.5 <= dur]
    return [sustain_attempt(xb, sr, a, offset, p, sp) for a in keep]


SUSTAIN_COLUMNS = ("n", "line", "t_s", "label", "button", "hold_ms", "round", "control", "blank", "snr_db",
                   "onset_ms", "interrupted", "stop_ms", "level_per_s_db", "cents_per_s", "verdict")


def _series(v: Sequence[float | None], spec: str) -> str:
    return " ".join("-" if x is None else format(x, spec) for x in v) or "-"


def sustain_row(r: SustainResult) -> dict[str, str]:
    a = r.attempt
    return {"n": str(a.index), "line": str(a.line), "t_s": f"{a.t:.3f}", "label": a.label, "button": a.button,
            "hold_ms": "-" if r.hold_ms is None else str(r.hold_ms),
            "round": "-" if a.round is None else str(a.round), "control": "1" if a.control else "0",
            "blank": "1" if a.blank else "0", "snr_db": _fmt(r.level_db - r.noise_db, ".1f"),
            "onset_ms": _fmt(r.onset_ms, ".0f"),
            "interrupted": "" if r.interrupted is None else str(int(r.interrupted)),
            "stop_ms": _fmt(r.stop_ms, ".0f"), "level_per_s_db": _series(r.level_per_s, "+.1f"),
            "cents_per_s": _series(r.cents_per_s, "+.0f"), "verdict": r.verdict}


def floor100(v: float) -> int:
    return int(math.floor(v / 100.0) * 100)


# A stop this soon after the onset is not the U1 auto-stop (about 6 s) but the Story 1.6 "every ~7th note"
# event (a held note ended 0.54 to 0.91 s after its onset, docs/test-plan.md "Walking skeleton (Story 1.6)"):
# such attempts are listed separately and left out of the interrupted counts, D_ok and T_stop.
EVENT_STOP_MS = 2000.0
_ATTEMPT_RE = re.compile(r"attempt=(\d+)")


def sustain_summary_lines(rows: Sequence[dict[str, str]], sp: SustainParams | None = None,
                          p: Params | None = None) -> list[str]:
    """Per (button, hold): interrupted count out of the attempts that sounded, auto-stop times; then the
    owner's update rule on these rows only: candidate = floor100(min(D_ok, T_stop) - 500 ms)."""
    out = []
    if sp is not None and p is not None:
        out.append(f"thresholds: latency {p.latency:.3f} s; interrupted = tone at the expected pitch "
                   f"< {p.min_snr_db:g} dB above the noise or > {sp.drop_db:g} dB below its first second, for "
                   f">= {sp.min_drop_s * 1000:.0f} ms, starting >= {sp.before_release_s * 1000:.0f} ms before the "
                   f"scheduled release (tone envelope {sp.env_win * 1000:.0f} ms)")
    acc: dict[tuple[str, int], list] = {}
    events = []
    for r in rows:
        if r["blank"] == "1" or r["hold_ms"] in ("", "-"):
            continue
        e = acc.setdefault((r["button"], int(r["hold_ms"])), [0, 0, 0, []])
        if r["interrupted"] == "":
            e[2] += 1  # no sound
            continue
        if r["interrupted"] == "1" and float(r["stop_ms"]) < EVENT_STOP_MS:
            events.append(r)  # the every-~7th-note event, not U1 data
            continue
        e[1] += 1
        if r["interrupted"] == "1":
            e[0] += 1
            e[3].append(float(r["stop_ms"]))
    out.append(f"{'button':<6} {'hold_ms':>7}  interrupted  no sound  auto-stop ms")
    for (b, h), (i, n, ns, stops) in sorted(acc.items()):
        out.append(f"{b:<6} {h:>7}  {i:>4}/{n:<6}  {ns:>8}  {', '.join(f'{v:.0f}' for v in sorted(stops)) or '-'}")
    holds: dict[int, list] = {}
    for (b, h), (i, n, ns, _s) in acc.items():
        holds.setdefault(h, []).append((i, n, ns))
    ok = [h for h, v in holds.items() if all(i == 0 and n > 0 and ns == 0 for i, n, ns in v)]
    stops = [v for e in acc.values() for v in e[3]]
    d_ok = max(ok) if ok else None
    t_stop = min(stops) if stops else None
    basis = [v for v in (d_ok, t_stop) if v is not None]
    cand = floor100(min(basis) - 500) if d_ok is not None else None
    if events:
        def where(r):
            m = _ATTEMPT_RE.search(r["label"])
            return f"attempt {m.group(1)} (note {int(m.group(1)) + 1} of its file, warm-up = 1)" if m else "attempt ?"
        out.append(f"EARLY STOPS (< {EVENT_STOP_MS:.0f} ms, the Story 1.6 every-~7th-note event, not the U1 auto-stop; "
                   "left out of the counts, D_ok and T_stop; re-run those holds): " + "; ".join(
                       f"{r['button']} {r['hold_ms']} ms {where(r)} stopped at {float(r['stop_ms']):.0f} ms"
                       for r in events))
    out.append(f"buttons: {', '.join(sorted({b for b, _ in acc}))}; D_ok (largest hold with every attempt held, "
               f"none silent) = {d_ok if d_ok is not None else '-'} ms; T_stop (shortest auto-stop) = "
               f"{f'{t_stop:.0f}' if t_stop is not None else '-'} ms; candidate floor100(min(D_ok, T_stop) - 500) = "
               f"{cand if cand is not None else '-'} ms (these rows only; the rule needs every button)")
    bad = [r for r in rows if r["verdict"] == "BLANK SOUNDED" or
           (r["control"] == "1" and r["hold_ms"] not in ("", "-") and int(r["hold_ms"]) <= 2000
            and r["interrupted"] != "0" and r not in events)]
    if bad:
        out.append(f"INVALID: {len(bad)} control attempt(s) did not behave (blank sounded, or a short hold was "
                   "not held to its release); find out why before using the numbers")
    return out


def report_sustain(x: np.ndarray, sr: int, atts: Sequence[Attempt], offset: float, p: Params, sp: SustainParams,
                   base: pathlib.Path, meta: dict[str, str]) -> int:
    meta = {**meta, "mode": "sustain", "offset_s": f"{offset:.3f}", "scale": f"{p.scale:.4f}",
            "latency_s": f"{p.latency:.3f}", "min_snr_db": f"{p.min_snr_db:g}"}
    results = sustain_analyse(x, sr, atts, offset, p, sp)
    rows = [sustain_row(r) for r in results]
    head = f"{'n':>3} {'t(s)':>8}  {'label':<44} {'SNR':>5} {'onset':>5} {'stop':>6}  verdict / level dB per s"
    print()
    print(head)
    print("-" * len(head))
    for r in rows:
        print(f"{r['n']:>3} {r['t_s']:>8}  {r['label'][:44]:<44} {r['snr_db']:>5} {r['onset_ms']:>5} "
              f"{r['stop_ms']:>6}  {r['verdict']} / {r['level_per_s_db']} / c {r['cents_per_s']}")
    summary = sustain_summary_lines(rows, sp, p)
    if meta.get("valid", "yes").startswith("no"):
        summary.append("INVALID RUN: PortAudio input overflow, samples were lost; run it again")
    print("\n".join(summary))
    if len(results) < len(atts):
        print(f"note: {len(atts) - len(results)} attempt(s) not analysed (recording ends before them)")
    csv_path, md_path = write_table(base, SUSTAIN_COLUMNS, rows, meta, summary)
    print(f"reports: {csv_path}  {md_path}")
    return 0


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------


def build_parser() -> argparse.ArgumentParser:
    d = Params()
    p = argparse.ArgumentParser(prog="listen_trial.py",
                                description="Throwaway spike: play a trial file with send_2wicc.py while "
                                            "recording the microphone, then detect the pitch of every PRESS row.")
    p.add_argument("trial", nargs="?", help="trial input file (the same file the sender plays)")
    p.add_argument("--port", help="bridge serial port, passed to send_2wicc.py")
    p.add_argument("--analyse", metavar="WAV", help="analyse an existing recording instead of playing")
    p.add_argument("--offset", type=float, help="with --analyse: recording time (s) of the schedule's t=0")
    p.add_argument("--scale", type=float, default=1.0,
                   help="with --analyse: WAV seconds per schedule second (printed by the live run; default 1)")
    p.add_argument("--device", default="MacBook Pro Microphone",
                   help="avfoundation audio device, name or index (default 'MacBook Pro Microphone')")
    p.add_argument("--sample-rate", type=int, default=48000, help="recording rate (default 48000)")
    p.add_argument("--out-dir", default=str(DEFAULT_OUT), help="where recordings and reports go (git-ignored)")
    p.add_argument("--ffmpeg", default="ffmpeg", help="ffmpeg binary")
    p.add_argument("--tail", type=float, default=1.0, help="seconds recorded after the sender exits (default 1)")
    p.add_argument("--attack", type=float, default=d.attack, help="skip after the press, s (0.30)")
    p.add_argument("--release-guard", type=float, default=d.release_guard, help="stop before the release, s (0.20)")
    p.add_argument("--latency", type=float, default=None,
                   help="extra shift, s, e.g. the measured onset median (default 0; melody and sustain: "
                        f"{STORY_1_6_LATENCY:g}, the Story 1.4 heard latency, measure it per session)")
    p.add_argument("--min-dbfs", type=float, default=d.min_dbfs, help="no sound below this level (-60)")
    p.add_argument("--min-snr-db", type=float, default=d.min_snr_db, help="no sound below this SNR (10)")
    p.add_argument("--list-devices", action="store_true", help="list avfoundation devices and exit")
    p.add_argument("--max-lead-frames", type=int, help="passed to the sender")
    # Story 1.4
    p.add_argument("--mode", default="pitch", choices=MODES,
                   help="pitch: Story 1.3 note pitch (default); detect: row 2.1; retrigger: row 2.2; "
                        "lead: row 2.3; jitter: row 2.4 (Story 1.4 files, attempts = TRY rows); "
                        "melody: Story 1.6 melody files (NOTE rows); sustain: Story 1.6 U1 files (TRY u1 rows)")
    p.add_argument("--backend", default="ffmpeg", choices=("ffmpeg", "sounddevice"),
                   help="capture: ffmpeg avfoundation (default; loses samples) or sounddevice (PortAudio, lossless)")
    p.add_argument("--dip-db", type=float, help="retrigger: dip below the first note (default: from the controls); "
                   f"melody: dip for an extra attack (default {MelodyParams().dip_db:g})")
    p.add_argument("--rise-db", type=float, help="retrigger: re-rise after the dip (default: from the controls); "
                   f"melody: rise of an attack (default {MelodyParams().rise_db:g})")
    p.add_argument("--lead-cents", type=float, default=TimingParams().lead_cents,
                   help="lead: pass if the first 40 ms are within this of the steady pitch (50)")
    p.add_argument("--selftest-tone", action="store_true",
                   help="capture self-test: record a steady tone from another device, count phase jumps; no trial")
    p.add_argument("--seconds", type=float, default=60.0, help="self-test length, s (60)")
    p.add_argument("--tone-hz", type=float, default=1000.0, help="self-test tone frequency (1000)")
    p.add_argument("--combine", nargs="+", metavar="CSV",
                   help="merge the step tables of several report CSVs (the parts of one row) and exit")
    p.add_argument("--pool-controls", action="store_true",
                   help="with --combine on retrigger reports: one threshold from the controls of all the files")
    p.add_argument("--band-snr-db", type=float, default=TimingParams().band_snr_db,
                   help="detect/lead: level at the expected pitch above the same band before the press (8)")
    p.add_argument("--report-tag", default="",
                   help="with --analyse: suffix for the report names (keeps the reports of the live run)")
    # Story 2.5 D16
    p.add_argument("--driver", metavar="CMD",
                   help="live run without 2wiCC: start CMD (e.g. 'python3 firmware/tools/link_probe.py --port \"$P\" "
                        "selfplay --lead-ms 3000') instead of the sender; the offset comes from its T0_HOST_MONO= line, "
                        "the AC3 explanation from its PASTE_LINE")
    p.add_argument("--late-max-us", type=float, help="melody, with --analyse: the firmware's late_max_us (selfplay)")
    p.add_argument("--period-us", type=float, help="melody, with --analyse: the console's report_period_us (selfplay)")
    return p


def params_from(a: argparse.Namespace) -> Params:
    latency = a.latency
    if latency is None:
        latency = STORY_1_6_LATENCY if a.mode in ("melody", "sustain") else Params().latency
    return Params(a.attack, a.release_guard, a.min_dbfs, a.min_snr_db, Params().min_voiced, latency, a.scale)


def melody_params_from(a: argparse.Namespace) -> MelodyParams:
    d = MelodyParams()
    return MelodyParams(dip_db=d.dip_db if a.dip_db is None else a.dip_db,
                        rise_db=d.rise_db if a.rise_db is None else a.rise_db)


def report_any(x: np.ndarray, sr: int, a: argparse.Namespace, rows_: Sequence, presses: Sequence[Press],
               offset: float, p: Params, tp_: TimingParams, base: pathlib.Path, meta: dict[str, str],
               self_timing: tuple[float, float] | None = None) -> int:
    """Run the analysis of `--mode` on a recording and write its reports."""
    if a.mode == "melody":
        if self_timing is None and getattr(a, "late_max_us", None) is not None and getattr(a, "period_us", None):
            self_timing = (a.late_max_us, a.period_us)
        return report_melody(x, sr, melody_notes_from_rows(rows_), offset, p, melody_params_from(a), base, meta,
                             self_timing)
    if a.mode == "sustain":
        return report_sustain(x, sr, attempts_from_rows(rows_), offset, p, SustainParams(), base, meta)
    if a.mode != "pitch":
        return report_timing(x, sr, attempts_from_rows(rows_), offset, p, tp_, a.mode, base, meta)
    report(x, sr, presses, offset, p, base, meta)
    return 0


def report(x: np.ndarray, sr: int, presses: Sequence[Press], offset: float, p: Params,
           base: pathlib.Path, meta: dict[str, str]) -> list[Result]:
    results = analyse(x, sr, presses, offset, p)
    print()
    print("\n".join(table_lines(results)))
    print("\n".join(summary_lines(results, offset)))
    if len(results) < len(presses):
        print(f"note: {len(presses) - len(results)} press(es) not analysed (recording ends before them)")
    csv_path, md_path = write_reports(base, results, {**meta, "offset_s": f"{offset:.3f}", "scale": f"{p.scale:.4f}"})
    print(f"reports: {csv_path}  {md_path}")
    return results


def write_table(base: pathlib.Path, columns: Sequence[str], rows: Sequence[dict[str, str]], meta: dict[str, str],
                extra: Sequence[str] = ()) -> tuple[pathlib.Path, pathlib.Path]:
    csv_path, md_path = base.with_suffix(".csv"), base.with_suffix(".md")
    with open(csv_path, "w", newline="", encoding="utf-8") as fh:
        w = csv.DictWriter(fh, fieldnames=columns)
        w.writeheader()
        w.writerows(rows)
    md = [f"# Trial analysis: {meta.get('trial', '')}", ""] + [f"- {k}: {v}" for k, v in meta.items()]
    md += ["", *(f"    {line}" for line in extra), ""] if extra else [""]
    md.append("| " + " | ".join(columns) + " |")
    md.append("|" + "---|" * len(columns))
    md += ["| " + " | ".join(r[c].replace("|", "/") for c in columns) + " |" for r in rows]
    md_path.write_text("\n".join(md) + "\n", encoding="utf-8")
    return csv_path, md_path


def report_timing(x: np.ndarray, sr: int, atts: Sequence[Attempt], offset: float, p: Params, tp_: TimingParams,
                  mode: str, base: pathlib.Path, meta: dict[str, str]) -> int:
    """Story 1.4 modes: print the per-attempt table and the step summary, write CSV + Markdown."""
    meta = {**meta, "mode": mode, "offset_s": f"{offset:.3f}", "scale": f"{p.scale:.4f}",
            "min_snr_db": f"{p.min_snr_db:g}", "min_dbfs": f"{p.min_dbfs:g}"}
    if not atts:
        print("error: no TRY rows in the trial file (Story 1.4 files only)", file=sys.stderr)
        return 1
    if mode == "jitter":
        j = analyse_jitter(x, sr, atts, offset, p)
        lines = jitter_lines(j)
        print()
        print("\n".join(lines))
        st = j.stats()
        rows = [{"pair": str(k + 1), "ioi_err_ms": f"{e:+.2f}"} for k, e in enumerate(j.ioi_err_ms)]
        meta.update({k: f"{v:.2f}" for k, v in st.items()})
        csv_path, md_path = write_table(base, ("pair", "ioi_err_ms"), rows, meta, lines)
        print(f"reports: {csv_path}  {md_path}")
        return 0
    results, extra_meta = analyse_attempts(x, sr, atts, offset, mode, p, tp_)
    meta.update(extra_meta)
    rows = [attempt_row(r) for r in results]
    head = f"{'n':>3} {'t(s)':>8}  {'label':<46} {'SNR':>5} {'onset':>5} {'len':>5} {'note':<4} " \
           f"{'on#':>3} {'dip':>5} {'lead':>5}  verdict"
    print()
    print(head)
    print("-" * len(head))
    for r in rows:
        print(f"{r['n']:>3} {r['t_s']:>8}  {r['label'][:46]:<46} {r['snr_db']:>5} {r['onset_ms']:>5} "
              f"{r['audible_ms']:>5} {r['note']:<4} {r['onsets']:>3} {r['dip_db']:>5} {r['lead_cents']:>5}  "
              f"{r['verdict']}")
    summary = attempt_summary_lines(rows, meta={k: v for k, v in extra_meta.items()})
    print("\n".join(summary))
    if len(results) < len(atts):
        print(f"note: {len(atts) - len(results)} attempt(s) not analysed (recording ends before them)")
    csv_path, md_path = write_table(base, ATTEMPT_COLUMNS, rows, meta, summary)
    print(f"reports: {csv_path}  {md_path}")
    return 0


_RECORDING_RE = re.compile(r"(.+-\d{8}-\d{6})(?:-.+)?$")


def recording_id(path: str) -> str:
    """The recording a report belongs to: `<trial>-<date>-<time>` without a `--report-tag` suffix."""
    stem = pathlib.Path(path).stem
    m = _RECORDING_RE.match(stem)
    return m.group(1) if m else stem


def combine(paths: Sequence[str], pool: bool = False) -> int:
    seen: dict[str, str] = {}
    for path in paths:
        rid = recording_id(path)
        if rid in seen:
            print(f"error: {path} and {seen[rid]} are reports of the same recording ({rid}); pass one report "
                  "per recording (e.g. only the *-rescored.csv files)", file=sys.stderr)
            return 1
        seen[rid] = path
    rows: list[dict[str, str]] = []
    for path in paths:
        try:
            with open(path, newline="", encoding="utf-8") as fh:
                rows += list(csv.DictReader(fh))
        except OSError as e:
            print(f"error: {e}", file=sys.stderr)
            return 1
    if rows and "interrupted" in rows[0]:
        print("\n".join([f"files: {len(paths)}", *sustain_summary_lines(rows)]))
        return 0
    if not rows or "pass" not in rows[0]:
        print("error: not a Story 1.4 attempt report (no 'pass' column)", file=sys.stderr)
        return 1
    extra: list[str] = []
    keys = Counter((r["label"], r["round"], r["family"], r["step"]) for r in rows)
    repeats = sum(n - 1 for n in keys.values() if n > 1)
    if repeats:
        extra.append(f"note: {repeats} attempt(s) appear in more than one recording (repeat runs of the same "
                     "trial file); they are counted once per run")
    if pool:
        rows, pooled = pooled_rescore(rows)
        extra += pooled
    print("\n".join([*extra, *attempt_summary_lines(rows, {"files": str(len(paths))})]))
    return 0


def selftest_lines(x: np.ndarray, sr: int, chunks, f0: float, backend: str, overflows: int | None) -> tuple[list[str], bool]:
    xb = bandpass(x, sr, f0 - 200.0, f0 + 200.0)
    tc = tone_discontinuities(xb, sr, f0)
    count, amp = tc.count, tc.amp
    rate = audio_rate(chunks)
    ratio = rate / sr if rate else math.nan
    level = 20 * math.log10(amp) if amp > 0 else -math.inf
    lines = [f"tone {f0:g} Hz level {level:.1f} dBFS; phase discontinuities (> 0.2 ms): {count}",
             f"delivered-sample ratio {ratio:.5f} (samples per real second / nominal rate)"]
    lines.append(f"not judged: first {STARTUP_GRACE_S:g} s (input start-up grace); "
                 f"low-level time skipped: start {tc.lead_s:.3f} s, end {tc.tail_s:.3f} s; "
                 f"tone dropouts in the middle: {tc.dropouts}")
    if overflows is not None:
        lines.append(f"PortAudio input overflows: {overflows}")
    ok = level > -50 and count == 0 and tc.dropouts == 0 and abs(ratio - 1.0) <= 0.001 and not overflows
    if level <= -50:
        lines.append("no tone found: play a steady tone near the microphone")
    if tc.dropouts:
        lines.append("the tone dropped out mid-recording (not checked there): keep the tone steady and run again")
    lines.append(f"self-test ({backend}): {'PASS' if ok else 'FAIL'} (pass = 0 discontinuities, ratio within 0.1 %"
                 ", no overflow)")
    return lines, ok


def run_selftest(a: argparse.Namespace) -> int:
    out = pathlib.Path(a.out_dir)
    out.mkdir(parents=True, exist_ok=True)
    wav = out / f"selftest-{a.backend}-{dt.datetime.now().strftime('%Y%m%d-%H%M%S')}.wav"
    rec = make_recorder(a.backend, a.device, a.sample_rate, wav, a.ffmpeg)
    print(f"recording {a.seconds:g} s of the test tone to {wav} ({a.backend}, '{a.device}')", flush=True)
    try:
        rec.start()
    except (OSError, RuntimeError) as e:
        print(f"error: {e}", file=sys.stderr)
        return 1
    try:
        time.sleep(a.seconds)
    except KeyboardInterrupt:
        pass
    finally:
        err = rec.stop()
    if err:
        print(f"{a.backend}: {err}", file=sys.stderr)
    x, sr = read_wav(wav)
    lines, ok = selftest_lines(x, sr, rec.chunks, a.tone_hz, a.backend, getattr(rec, "overflows", None))
    print("\n".join(lines))
    return 0 if ok else 2


def main(argv: Sequence[str] | None = None) -> int:
    a = build_parser().parse_args(argv)
    if a.list_devices:
        return list_devices(a.ffmpeg)
    if a.combine:
        return combine(a.combine, a.pool_controls)
    if a.selftest_tone:
        return run_selftest(a)
    if not a.trial:
        print("error: trial file is required", file=sys.stderr)
        return 1
    trial = pathlib.Path(a.trial)
    try:
        rows_, presses = load_presses(trial)
    except (OSError, s.ParseError) as e:
        print(f"error: {trial}: {e}", file=sys.stderr)
        return 1
    atts = attempts_from_rows(rows_)
    tp_ = TimingParams(dip_db=a.dip_db, rise_db=a.rise_db, lead_cents=a.lead_cents, band_snr_db=a.band_snr_db)
    if a.mode in (*TIMING_MODES, "sustain") and not atts:
        print(f"error: --mode {a.mode} needs a Story 1.4 / 1.6 trial file (TRY rows)", file=sys.stderr)
        return 1
    if a.mode == "melody" and not melody_notes_from_rows(rows_):
        print("error: --mode melody needs a Story 1.6 melody file (NOTE rows)", file=sys.stderr)
        return 1
    p = params_from(a)

    if a.analyse:
        if a.offset is None:
            print("error: --analyse needs --offset (printed by the live run and in its .md)", file=sys.stderr)
            return 1
        wav = pathlib.Path(a.analyse)
        try:
            x, sr = read_wav(wav)
        except (OSError, ValueError, wave.Error) as e:
            print(f"error: {e}", file=sys.stderr)
            return 1
        meta = {"trial": str(trial), "wav": str(wav)}
        base = wav.with_suffix("")
        if a.report_tag:
            base = base.with_name(f"{base.name}-{a.report_tag}")
        return report_any(x, sr, a, rows_, presses, a.offset, p, tp_, base, meta)

    if not a.port and not a.driver:
        print("error: --port is required (or --driver CMD, or --analyse WAV --offset S)", file=sys.stderr)
        return 1
    out = pathlib.Path(a.out_dir)
    out.mkdir(parents=True, exist_ok=True)
    stamp = dt.datetime.now().strftime("%Y%m%d-%H%M%S")
    wav = out / f"{trial.stem}-{stamp}.wav"
    rec = make_recorder(a.backend, a.device, a.sample_rate, wav, a.ffmpeg)
    print(f"recording {wav} from '{a.device}' ({a.backend})", flush=True)
    try:
        rec.start()
    except (OSError, RuntimeError) as e:
        print(f"error: {e}", file=sys.stderr)
        return 1
    code, lines, interrupted = 1, [], False
    try:
        time.sleep(0.3)  # let the input settle; it also gives a noise floor before the lead-in
        if a.driver:
            code, lines, interrupted = run_driver(a.driver)
        else:
            extra = ["--max-lead-frames", str(a.max_lead_frames)] if a.max_lead_frames else []
            code, lines, interrupted = run_sender(a.port, trial, extra)
        try:
            time.sleep(a.tail)
        except KeyboardInterrupt:
            interrupted = True
    finally:
        old = signal.signal(signal.SIGINT, signal.SIG_IGN)  # finish the WAV even on a second Ctrl+C
        try:
            err = rec.stop()
        finally:
            signal.signal(signal.SIGINT, old)
    if err:
        print(f"{a.backend}: {err}", file=sys.stderr)
    overflows = getattr(rec, "overflows", None)
    if overflows:
        print(f"INVALID RUN: {overflows} PortAudio input overflow(s), samples were lost", file=sys.stderr)
    who = "driver" if a.driver else "sender"
    print(f"{who} exit {code}{' (interrupted)' if interrupted else ''}; recorded {rec.total / a.sample_rate:.1f} s")
    self_timing = None
    if a.driver:
        t_send = driver_t0(lines)
        self_timing = driver_self_timing(lines)
        if t_send is None:
            print("error: the driver printed no T0_HOST_MONO= line (it never started the piece); the WAV is kept: "
                  f"{wav}", file=sys.stderr)
            return code or 1
    else:
        rows = s.parse_lines(trial.read_text(encoding="utf-8-sig").splitlines())
        t_send = sender_start(lines, s.announcements(rows, FRAME_RATE))
    if t_send is None or not rec.chunks:
        print("error: cannot align (no --announce lines matched the file, or no audio); the WAV is kept: "
              f"{wav}", file=sys.stderr)
        return code or 1
    offset, scale = alignment(rec.chunks, a.sample_rate, t_send)
    if abs(scale - 1.0) > 0.005:
        print(f"warning: the capture delivered {scale * 100:.1f} % of real time (samples lost); "
              "note windows are rescaled to match", file=sys.stderr)
    print(f"offset {offset:.3f} s, scale {scale:.4f}  (re-analyse: uv run spikes/listen_trial.py --analyse {wav} "
          f"{trial} --offset {offset:.3f} --scale {scale:.4f})")
    p = Params(p.attack, p.release_guard, p.min_dbfs, p.min_snr_db, p.min_voiced, p.latency, scale, p.band)
    x, sr = read_wav(wav)
    meta = {"trial": str(trial), "wav": str(wav), "device": a.device, "backend": a.backend,
            f"{who}_exit": str(code), "interrupted": str(interrupted)}
    if a.driver:
        meta["driver"] = a.driver
    if overflows is not None:
        meta["input_overflows"] = str(overflows)
        meta["valid"] = "no (input overflow)" if overflows else "yes"
    if self_timing is not None:
        print(f"re-analyse with the AC3 explanation: add --late-max-us {self_timing[0]:.0f} --period-us "
              f"{self_timing[1]:.0f}")
    report_any(x, sr, a, rows_, presses, offset, p, tp_, wav.with_suffix(""), meta, self_timing)
    return code


if __name__ == "__main__":
    sys.exit(main())
