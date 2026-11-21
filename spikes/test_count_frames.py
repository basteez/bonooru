# SPDX-License-Identifier: MIT
# Throwaway spike tests for spikes/count_frames.py (Story 1.4, U10). Synthetic frames; ffmpeg only in one optional test.
# Run: uv run --with pytest --with numpy pytest spikes/test_count_frames.py
from __future__ import annotations

import importlib.util
import pathlib
import shutil
import subprocess
import sys

import pytest

np = pytest.importorskip("numpy")

HERE = pathlib.Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("count_frames", HERE / "count_frames.py")
cf = importlib.util.module_from_spec(_spec)
sys.modules["count_frames"] = cf
_spec.loader.exec_module(cf)

RNG = np.random.default_rng(99)


def scene(images: int, h: int = 48, w: int = 64) -> np.ndarray:
    """`images` distinct pictures: a static textured background, a HUD bar and a moving square."""
    bg = RNG.integers(40, 200, (h, w)).astype(np.float32)
    out = np.repeat(bg[None], images, axis=0)
    out[:, :6, :] = 230  # HUD
    for k in range(images):
        x = (3 * k) % (w - 8)
        out[k, 20:28, x: x + 8] = 255
    return out


def captured(images: np.ndarray, dup: list | int, noise: float = 0.0) -> np.ndarray:
    reps = [dup] * images.shape[0] if isinstance(dup, int) else list(dup)
    frames = np.repeat(images, reps, axis=0)
    if noise:
        frames = frames + RNG.normal(0, noise, frames.shape)
    return np.clip(frames, 0, 255).astype(np.uint8)


@pytest.mark.parametrize("dup,game_fps", [(4, 60), (8, 30), (12, 20), (2, 120)])
def test_duplication_factor_gives_game_rate(dup, game_fps):
    fc = cf.count_frames(captured(scene(240 * 3 // dup), dup), fps=240)
    assert fc.median_run == dup and abs(fc.game_fps - game_fps) < 1e-9
    assert set(fc.runs) == {dup}
    assert set(fc.per_second) == {game_fps}


def test_sensor_noise_does_not_create_changes():
    fc = cf.count_frames(captured(scene(30), 8, noise=1.5), fps=240)
    assert fc.changes == 29 and fc.runs == {8: 28}
    assert fc.noise > 0.5 and fc.threshold > fc.noise  # derived from the static parts and reported
    lines = "\n".join(cf.report_lines(fc))
    assert "threshold" in lines and "frame period when the picture changes 33.33 ms (30.00 Hz)" in lines
    assert "assumption" in lines and "caveat" not in lines


def test_capture_rate_equal_to_game_rate():
    fc = cf.count_frames(captured(scene(120), 1, noise=1.0), fps=60)
    assert fc.runs == {1: 118} and fc.per_second == (60, 60)


def test_variable_rate_reports_min_and_max():
    # 1 s at 60 fps then 1 s at 30 fps, in a 240 fps capture
    reps = [4] * 60 + [8] * 30
    fc = cf.count_frames(captured(scene(90), reps, noise=1.0), fps=240)
    assert min(fc.per_second) == 30 and max(fc.per_second) == 60
    assert set(fc.runs) == {4, 8}


def test_explicit_threshold_and_errors():
    frames = captured(scene(10), 4)
    assert cf.count_frames(frames, 240, threshold=1e9).changes == 0
    with pytest.raises(ValueError):
        cf.count_frames(frames[:1], 240)


@pytest.mark.skipif(shutil.which("ffmpeg") is None or shutil.which("ffprobe") is None, reason="ffmpeg not installed")
def test_decode_with_ffmpeg(tmp_path, capsys):
    # a 30 fps test pattern stored as a 120 fps lossless video: every image repeated 4 times
    video = tmp_path / "clip.mkv"
    subprocess.run(["ffmpeg", "-v", "error", "-f", "lavfi", "-i", "testsrc=size=128x96:rate=30:duration=2",
                    "-vf", "fps=120", "-c:v", "ffv1", str(video)], check=True)
    fps, w, h = cf.probe(str(video))
    assert (fps, w, h) == (120.0, 128, 96)
    frames = cf.decode(str(video), 64, w, h)
    assert frames.shape == (240, 48, 64)
    assert cf.main([str(video), "--width", "64"]) == 0
    out = capsys.readouterr().out
    assert "(30.00 Hz)" in out


def test_missing_video_is_an_error(tmp_path, capsys):
    assert cf.main([str(tmp_path / "none.mov")]) == 1
    assert "error" in capsys.readouterr().err


def test_sustained_rate_caveat_when_the_picture_pauses():
    # 60 fps game in a 240 fps capture, but the picture only moves for half of each second
    reps = []
    for _ in range(3):
        reps += [4] * 30 + [120]
    fc = cf.count_frames(captured(scene(len(reps)), reps, noise=1.0), fps=240)
    lines = "\n".join(cf.report_lines(fc))
    assert fc.median_run == 4 and "(60.00 Hz)" in lines
    assert "caveat: sustained rate not shown" in lines
