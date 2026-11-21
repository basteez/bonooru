# /// script
# requires-python = ">=3.10"
# dependencies = ["numpy>=1.26"]
# ///
"""Count the distinct images per second in a video of the screen (Story 1.4, row 2.5, U10).

SPDX-License-Identifier: MIT (the project licence, see LICENSE at the repo root).

THROWAWAY SPIKE CODE. Not shipped. The game's frame rate is the number of
**distinct** images per second, counted in a recording with more frames per
second than the game (a phone in 240 or 120 fps slow motion, or a 60 fps
capture card). The capture fps is an assumption, not the answer: a 60 fps
recording of a 30 fps game shows runs of 2 identical frames.

ffmpeg decodes the video (grey, downscaled, every frame passed through); the
counting is a pure function on a numpy array of frames. Videos are never
committed (spikes/recordings/ is git-ignored).

Usage: uv run spikes/count_frames.py spikes/recordings/clip.mov
       uv run spikes/count_frames.py clip.mov --fps 240 --width 160 --threshold 2.5
"""

from __future__ import annotations

import argparse
import itertools
import json
import math
import subprocess
import sys
from collections import Counter
from collections.abc import Sequence
from dataclasses import dataclass

import numpy as np


@dataclass(frozen=True)
class FrameCount:
    fps: float                      # capture fps (assumed, from the file or --fps)
    frames: int
    changes: int
    threshold: float                # mean abs difference (grey levels) counted as a change
    noise: float                    # typical difference of the static parts between consecutive frames
    per_second: tuple[int, ...]     # changes in each whole 1 s window
    runs: dict[int, int]            # run length (identical consecutive frames) -> count

    @property
    def median_run(self) -> float:
        lengths = [k for k, n in self.runs.items() for _ in range(n)]
        return float(np.median(lengths)) if lengths else math.nan

    @property
    def game_fps(self) -> float:
        return self.fps / self.median_run if self.median_run else math.nan


def frame_differences(frames: np.ndarray, static_pct: float = 50.0, dynamic_pct: float = 90.0
                      ) -> tuple[np.ndarray, np.ndarray]:
    """(change metric, noise metric) for each pair of consecutive frames.

    Pixels are split by how much they vary over the whole clip: the least varying half are the static
    parts (HUD, background: only sensor and compression noise), the most varying tenth are where the
    picture moves. The change metric is the mean abs difference over the moving pixels, the noise
    metric the same over the static ones."""
    f = frames.astype(np.float32)
    d = np.abs(np.diff(f, axis=0))                  # (n-1, h, w)
    spread = f.std(axis=0)
    static = spread <= np.percentile(spread, static_pct)
    dynamic = spread >= np.percentile(spread, dynamic_pct)
    return d[:, dynamic].mean(axis=1), d[:, static].mean(axis=1)


def count_frames(frames: np.ndarray, fps: float, threshold: float | None = None, factor: float = 4.0,
                 min_threshold: float = 0.5) -> FrameCount:
    """Count changes between consecutive frames. Threshold: `factor` x the median static-part difference
    (the noise), at least `min_threshold` grey levels, unless given."""
    if frames.ndim != 3 or frames.shape[0] < 2:
        raise ValueError("need at least 2 frames of shape (n, h, w)")
    change, static = frame_differences(frames)
    noise = float(np.median(static))
    thr = threshold if threshold is not None else max(min_threshold, factor * noise)
    changed = change > thr
    n = frames.shape[0]
    idx = np.flatnonzero(changed) + 1               # frame index where a new image starts
    starts = np.concatenate([[0], idx])             # the first frame of the clip starts an image too
    per = round(fps)
    per_second = tuple(int(np.sum((starts >= s) & (starts < s + per))) for s in range(0, n - per + 1, per))
    bounds = [0, *idx.tolist(), n]
    lengths = [b - a for a, b in itertools.pairwise(bounds) if b > a]
    if len(lengths) > 2:
        lengths = lengths[1:-1]                     # the first and last runs are cut by the clip edges
    return FrameCount(fps, n, int(changed.sum()), thr, noise, per_second, dict(sorted(Counter(lengths).items())))


def report_lines(fc: FrameCount) -> list[str]:
    ps = fc.per_second
    out = [(f"capture fps {fc.fps:g} (assumption from the file or --fps, not a result); {fc.frames} frames, "
            f"{fc.frames / fc.fps:.1f} s"),
           f"changes {fc.changes}; threshold {fc.threshold:.2f} grey levels (static-part noise {fc.noise:.2f})"]
    if ps:
        out.append(f"distinct images per 1 s window: min {min(ps)}, median {float(np.median(ps)):g}, max {max(ps)} "
                   f"({len(ps)} windows)")
    runs = ", ".join(f"{k}: {v}" for k, v in fc.runs.items())
    out.append(f"run lengths (identical consecutive frames): {runs}")
    out.append(f"median run {fc.median_run:g} frames -> frame period when the picture changes "
               f"{fc.median_run / fc.fps * 1000:.2f} ms ({fc.game_fps:.2f} Hz)")
    if ps and float(np.median(ps)) < fc.game_fps - 1:
        out.append(f"caveat: sustained rate not shown (median {float(np.median(ps)):g} distinct images per 1 s "
                   f"window < {fc.game_fps:.2f}): the picture did not change on every game frame")
    return out


# ---------------------------------------------------------------------------
# ffmpeg I/O (thin)
# ---------------------------------------------------------------------------


def probe(path: str, ffprobe: str = "ffprobe") -> tuple[float, int, int]:
    r = subprocess.run([ffprobe, "-v", "error", "-select_streams", "v:0", "-show_entries",
                        "stream=avg_frame_rate,r_frame_rate,width,height", "-of", "json", path],
                       check=True, capture_output=True, text=True)
    st = json.loads(r.stdout)["streams"][0]
    num, _, den = (st.get("avg_frame_rate") or st["r_frame_rate"]).partition("/")
    fps = float(num) / float(den or 1) if float(den or 1) else float("nan")
    return fps, int(st["width"]), int(st["height"])


def decode(path: str, width: int, src_w: int, src_h: int, ffmpeg: str = "ffmpeg") -> np.ndarray:
    """All frames as grey uint8 (n, h, width); frames are passed through as stored (no duplication or drop)."""
    h = max(2, round(src_h * width / src_w / 2) * 2)
    cmd = [ffmpeg, "-v", "error", "-i", path, "-an", "-fps_mode", "passthrough",
           "-vf", f"scale={width}:{h},format=gray", "-f", "rawvideo", "-"]
    raw = subprocess.run(cmd, check=True, capture_output=True).stdout
    n = len(raw) // (width * h)
    return np.frombuffer(raw[: n * width * h], dtype=np.uint8).reshape(n, h, width)


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(prog="count_frames.py",
                                description="Throwaway spike: distinct images per second in a screen video (U10).")
    p.add_argument("video")
    p.add_argument("--fps", type=float, help="capture fps if the file's is wrong (some slow-motion exports)")
    p.add_argument("--width", type=int, default=160, help="decode width in pixels (160)")
    p.add_argument("--threshold", type=float, help="change threshold, grey levels (default: from the static parts)")
    p.add_argument("--ffmpeg", default="ffmpeg")
    p.add_argument("--ffprobe", default="ffprobe")
    return p


def main(argv: Sequence[str] | None = None) -> int:
    a = build_parser().parse_args(argv)
    try:
        fps, w, h = probe(a.video, a.ffprobe)
        frames = decode(a.video, a.width, w, h, a.ffmpeg)
    except (OSError, subprocess.CalledProcessError, KeyError, IndexError, ValueError) as e:
        print(f"error: cannot read {a.video}: {e}", file=sys.stderr)
        return 1
    fps = a.fps or fps
    if not fps or not math.isfinite(fps):
        print("error: unknown capture fps, pass --fps", file=sys.stderr)
        return 1
    try:
        fc = count_frames(frames, fps, a.threshold)
    except ValueError as e:
        print(f"error: {e}", file=sys.stderr)
        return 1
    print("\n".join(report_lines(fc)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
