# SPDX-License-Identifier: MIT
# Throwaway spike test (Story 1.5): consistency checks on the bundled default profile
# app/profiles/oot-switch2-1.1.1.toml. No hardware needed.
#
# This is NOT the profile loader of AD-17: it does not parse the profile for the app, has no defaults and
# no migration. It only reads the TOML with the standard library and asserts facts about the data
# (hand-transcribed from docs/test-plan.md and docs/ocarina-input-mechanics.md). Story 5.1 (Rust, ocb-core
# `profile` module) owns loading and validation and replaces this test.
#
# Needs Python 3.11+ (tomllib). Run:
#   uv run --python 3.12 --with pytest pytest spikes/test_default_profile.py
from __future__ import annotations

import pathlib
import re
import tomllib

import pytest

ROOT = pathlib.Path(__file__).resolve().parent.parent
PROFILE_PATH = ROOT / "app" / "profiles" / "oot-switch2-1.1.1.toml"

BUTTONS = ("L", "R", "A", "Y", "X")  # AD-9 allow-list: note buttons
DPAD_OFFSET_KEY = {"none": None, "up": "dpad_up", "down": "dpad_down"}
STICK_OFFSET_KEY = {"none": None, "up": "stick_up", "down": "stick_down"}

# Chart fingerings for a few anchor pitches, copied from docs/ocarina-input-mechanics.md, "Fingering chart".
# (pitch: (button, dpad, stick))
CHART_ANCHORS = {
    71: ("L", "down", "down"),  # B4 = L + D-down + stick down
    79: ("R", "none", "up"),  # G5 = R + stick up
    81: ("A", "none", "none"),  # A5 = A
    89: ("X", "up", "up"),  # F6 = X + D-up + stick up
}


@pytest.fixture(scope="module")
def text() -> str:
    return PROFILE_PATH.read_text(encoding="utf-8")


@pytest.fixture(scope="module")
def profile(text: str) -> dict:
    return tomllib.loads(text)


def model_pitch(profile: dict, row: dict) -> int:
    pitch = profile["buttons"][row["button"]]
    for key in (DPAD_OFFSET_KEY[row["dpad"]], STICK_OFFSET_KEY[row["stick"]]):
        if key is not None:
            pitch += profile["modifiers"][key]
    return pitch


def test_schema_and_profile_header(profile: dict) -> None:
    assert type(profile["schema"]) is int
    assert profile["profile"]["frame_rate"] == 60
    assert isinstance(profile["profile"]["name"], str) and profile["profile"]["name"]
    assert profile["profile"]["game_version"] == "1.1.1"


def test_required_tables_and_keys(profile: dict) -> None:
    for table in (
        "profile",
        "buttons",
        "modifiers",
        "range",
        "timing",
        "cost",
        "fingerings",
    ):
        assert table in profile, table
    assert set(profile["buttons"]) == set(BUTTONS)
    assert set(profile["modifiers"]) == {
        "dpad_up",
        "dpad_down",
        "stick_up",
        "stick_down",
        "stick_up_ly",
        "stick_down_ly",
        "stick_lx",
    }
    assert set(profile["timing"]) == {
        "min_press_frames",
        "min_gap_same_button_frames",
        "modifier_lead_frames",
        "max_sustain_ms",
        "start_lead_ms",
    }
    for key, value in profile["timing"].items():
        assert type(value) is int, key
    assert set(profile["cost"]) == {
        "dpad_change",
        "stick_change",
        "stick_through_centre",
        "same_button_retrigger",
        "non_natural",
    }
    assert set(profile["range"]) == {"low", "high"}


def test_measured_button_pitches_and_offsets(profile: dict) -> None:
    # test plan 1.1 / 1.9, sounding pitches (one octave above the community chart)
    assert profile["buttons"] == {"L": 74, "R": 77, "A": 81, "Y": 83, "X": 86}
    mods = profile["modifiers"]
    assert (
        mods["dpad_up"],
        mods["dpad_down"],
        mods["stick_up"],
        mods["stick_down"],
    ) == (1, -1, 2, -2)
    assert (profile["range"]["low"], profile["range"]["high"]) == (71, 89)


def test_fingerings_are_45_distinct_allow_listed_rows(profile: dict) -> None:
    rows = profile["fingerings"]
    assert len(rows) == 45
    keys = set()
    for row in rows:
        assert set(row) == {"button", "dpad", "stick", "pitch", "chart"}, row
        assert row["button"] in BUTTONS, row
        assert row["dpad"] in DPAD_OFFSET_KEY, row
        assert row["stick"] in STICK_OFFSET_KEY, row
        assert type(row["pitch"]) is int and type(row["chart"]) is bool, row
        keys.add((row["button"], row["dpad"], row["stick"]))
    assert len(keys) == 45
    assert keys == {
        (b, d, s) for b in BUTTONS for d in DPAD_OFFSET_KEY for s in STICK_OFFSET_KEY
    }


def test_every_row_matches_the_model(profile: dict) -> None:
    for row in profile["fingerings"]:
        assert row["pitch"] == model_pitch(profile, row), row


def test_pitches_cover_the_range_without_gaps(profile: dict) -> None:
    low, high = profile["range"]["low"], profile["range"]["high"]
    pitches = {row["pitch"] for row in profile["fingerings"]}
    assert len(pitches) == 19
    assert pitches == set(range(low, high + 1))


def test_one_chart_fingering_per_pitch(profile: dict) -> None:
    chart = [row for row in profile["fingerings"] if row["chart"]]
    assert len(chart) == 19
    assert {row["pitch"] for row in chart} == set(
        range(profile["range"]["low"], profile["range"]["high"] + 1)
    )


def test_chart_anchors_match_the_fingering_chart(profile: dict) -> None:
    chart = {
        row["pitch"]: (row["button"], row["dpad"], row["stick"])
        for row in profile["fingerings"]
        if row["chart"]
    }
    for pitch, combo in CHART_ANCHORS.items():
        assert chart[pitch] == combo, pitch


def test_timing_floor_and_sustain_bound(profile: dict) -> None:
    timing = profile["timing"]
    # AD-15: not below 2 x max(report_period_us, frame time); only the frame-time part (2 frames) is
    # checkable now, the report period is measured in Epic 2 and Story 5.1 enforces the full rule.
    assert timing["min_press_frames"] >= 2
    assert timing["min_gap_same_button_frames"] >= 2
    assert timing["modifier_lead_frames"] >= 0
    # U1 measured on A (Story 1.6, test plan row 1.10): every hold up to 6500 ms held (D_ok), shortest auto-stop
    # 6599 ms (T_stop). The value must keep at least the 500 ms margin of the update rule below both.
    d_ok_ms, t_stop_ms, margin_ms = 6500, 6599, 500
    assert 0 < timing["max_sustain_ms"] <= min(d_ok_ms, t_stop_ms) - margin_ms
    assert timing["start_lead_ms"] >= 0


def test_cost_weights_keep_the_supported_order(profile: dict) -> None:
    cost = profile["cost"]
    # Only the order "no change (0) < D-pad change < stick change" is supported by the data (U4, row 2.3).
    assert 0 < cost["dpad_change"] < cost["stick_change"]


def test_stick_state_bytes(profile: dict) -> None:
    mods = profile["modifiers"]
    assert mods["stick_up_ly"] == 0x00
    assert mods["stick_down_ly"] == 0xFF
    assert mods["stick_lx"] == 0x80


KEY_VALUE = re.compile(r"^\s*[A-Za-z0-9_]+\s*=")
TABLE_HEADER = re.compile(r"^\s*\[\[?\s*([A-Za-z0-9_.]+)\s*\]\]?")


def test_every_value_outside_fingerings_has_a_comment(text: str) -> None:
    """Cheap guard for AC 2: each key = value line has a comment on the line or directly above it."""
    in_fingerings = False
    previous = ""
    missing = []
    for number, line in enumerate(text.splitlines(), start=1):
        stripped = line.strip()
        header = TABLE_HEADER.match(line)
        if header:
            in_fingerings = (
                stripped.startswith("[[") and header.group(1) == "fingerings"
            )
        elif (
            KEY_VALUE.match(line)
            and not in_fingerings
            and "#" not in line
            and not previous.startswith("#")
        ):
            missing.append(f"{number}: {stripped}")
        if stripped:
            previous = stripped
    assert not missing, "values without a comment:\n" + "\n".join(missing)


def test_every_fingering_row_has_its_measured_hz(text: str) -> None:
    # Each [[fingerings]] header carries the measured Hz from the 45-combination table as a trailing comment.
    headers = [
        line for line in text.splitlines() if line.strip().startswith("[[fingerings]]")
    ]
    assert len(headers) == 45
    for line in headers:
        assert re.search(r"#.*\d+\.\d Hz", line), line
