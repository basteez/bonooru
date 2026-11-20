# /// script
# requires-python = ">=3.10"
# dependencies = ["numpy>=1.26", "pyserial>=3.5"]
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
"""

from __future__ import annotations

import argparse
import csv
import datetime as dt
import importlib.util
import math
import os
import pathlib
import re
import signal
import subprocess
import sys
import threading
import time
import wave
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


def run_sender(port: str, trial: pathlib.Path, extra: Sequence[str]) -> tuple[int, list[tuple[float, str]], bool]:
    """Run the unchanged sender with --announce; echo and timestamp its stdout. Returns (code, lines, interrupted)."""
    cmd = [sys.executable, "-u", str(SENDER), "--port", port, "--announce", *extra, str(trial)]
    # Same process group: a real terminal Ctrl+C reaches the sender directly (its own stop sequence).
    proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, text=True, bufsize=1)
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
    p.add_argument("--latency", type=float, default=d.latency,
                   help="extra shift, s, e.g. the measured onset median (default 0)")
    p.add_argument("--min-dbfs", type=float, default=d.min_dbfs, help="no sound below this level (-60)")
    p.add_argument("--min-snr-db", type=float, default=d.min_snr_db, help="no sound below this SNR (10)")
    p.add_argument("--list-devices", action="store_true", help="list avfoundation devices and exit")
    p.add_argument("--max-lead-frames", type=int, help="passed to the sender")
    return p


def params_from(a: argparse.Namespace) -> Params:
    return Params(a.attack, a.release_guard, a.min_dbfs, a.min_snr_db, Params().min_voiced, a.latency, a.scale)


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


def main(argv: Sequence[str] | None = None) -> int:
    a = build_parser().parse_args(argv)
    if a.list_devices:
        return list_devices(a.ffmpeg)
    if not a.trial:
        print("error: trial file is required", file=sys.stderr)
        return 1
    trial = pathlib.Path(a.trial)
    try:
        _, presses = load_presses(trial)
    except (OSError, s.ParseError) as e:
        print(f"error: {trial}: {e}", file=sys.stderr)
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
        report(x, sr, presses, a.offset, p, wav.with_suffix(""), {"trial": str(trial), "wav": str(wav)})
        return 0

    if not a.port:
        print("error: --port is required (or --analyse WAV --offset S)", file=sys.stderr)
        return 1
    out = pathlib.Path(a.out_dir)
    out.mkdir(parents=True, exist_ok=True)
    stamp = dt.datetime.now().strftime("%Y%m%d-%H%M%S")
    wav = out / f"{trial.stem}-{stamp}.wav"
    rec = Recorder(a.device, a.sample_rate, wav, a.ffmpeg)
    print(f"recording {wav} from '{a.device}'", flush=True)
    try:
        rec.start()
    except (OSError, RuntimeError) as e:
        print(f"error: {e}", file=sys.stderr)
        return 1
    code, lines, interrupted = 1, [], False
    try:
        time.sleep(0.3)  # let the input settle; it also gives a noise floor before the lead-in
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
        print(f"ffmpeg: {err}", file=sys.stderr)
    print(f"sender exit {code}{' (interrupted)' if interrupted else ''}; recorded {rec.total / a.sample_rate:.1f} s")
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
    report(x, sr, presses, offset, p, wav.with_suffix(""),
           {"trial": str(trial), "wav": str(wav), "device": a.device, "sender_exit": str(code),
            "interrupted": str(interrupted)})
    return code


if __name__ == "__main__":
    sys.exit(main())
