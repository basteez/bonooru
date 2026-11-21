# Finalize — Pass 1 coverage check (2026-11-20)

Run by the distiller on `DESIGN.md` (542 lines) + `EXPERIENCE.md` (~390 lines), against `references/validate.md` Pass 1.

## 1. Flow coverage — adequate

Sources: SPEC CAP-1..CAP-12 (no named UJs in sources; flows are the 3 memlog-approved ones, protagonist Tiziano).

| CAP | Covered by | Note |
|---|---|---|
| CAP-1 import .mid / text | Flow 2 (.mid), Flow 3 (text) | ok |
| CAP-2 mono + range fit | Flow 2 (2 folded notes) | ok |
| CAP-3 fingering + profile | States/Component only | no flow touches profile editing (Advanced settings) — low |
| CAP-4 play, countdown, stop, tempo | Flows 1–3 | tempo change only as Flow 3 failure path |
| CAP-5 link drop | State table (Link lost playing) | no flow; firmware-side, acceptable |
| CAP-6 discovery + update offer | Flow 1 (discovery) | **miss: no flow for required / recommended update** — medium |
| CAP-7 flash both boards | Flow 1 | failure path present |
| CAP-8 virtual gamepad to free play | Flow 1 step 9, Flow 2 | ok |
| CAP-9 dry run / preview | Flow 3 | ok |
| CAP-10 / A.5 first-time user | — | **miss: protagonist is the developer-owner; A.5 requires a first-time non-developer** — high |
| CAP-11 live MIDI, CAP-12 calibration | Scope deltas (out of this pass) | ok by decision |

## 2. Token completeness — adequate

- 44 colour tokens, all valid hex (8-digit where alpha). 22 typography, 6 rounded, 30 spacing, 46 component tokens.
- 88 `{…}` refs in DESIGN.md, 20 in EXPERIENCE.md: all resolve (the one literal `{path.to.token}` in Foundation is the syntax example, not a ref).
- Dark only: no light pairs needed (decision).
- **[ASSUMPTION] tokens needing approval:** `status-warning #f08a4b`, `status-error #ef6a55`, `status-unknown #9a8a72` (only `status-ok` is in the mock); `amber-ink #8a5a12` (song name on parchment — amber itself is ~1.5:1 on parchment); `scrim`; `typography.code` (egui Hack monospace for the editor); `gamepad-compact-width 200`, `song-rail-width 48`, `lane-height-min 18`.
- Contrast stated for load-bearing pairs. **Below AA:** out-of-range label `#6f675b` on `#cfc6b4` = 3.3:1 (11 px text). Mock value kept; `#55504a` (4.7:1) proposed in Do's/Don'ts. Warning vs amber luminance only 1.3:1 apart — symbol carries meaning.
- Mock note: playhead and official-song warning share `ember` (as in the Hybrid mock); intentional but worth a look.

## 3. Component coverage — adequate

All DESIGN.md components have a behavioural row or state in EXPERIENCE.md, except purely decorative ones (panel-tab, corner-bracket, rim). Thin spots:
- Modal dialog and setup wizard have **no Hybrid mock**; their visual spec in DESIGN.md is [ASSUMPTION].
- Text editor visual (split ratio 40/60, monospace) [ASSUMPTION].
- End-card button styling on parchment [ASSUMPTION].
- Staff view: lane→staff-rule mapping and modifier glyph form [ASSUMPTION].
- Gamepad leader lines (vs chips only in mock) [ASSUMPTION].

## 4. State coverage — adequate

Every requested state is in the State Patterns table (first run, bridge not found, foreign Pico/RiiDraw, Windows no product string, bridge only, controller not ready, USB suspend/dock idle + playing, required/recommended update, link lost idle + playing, underrun, LATE, bridge unplugged, semantic NAK, flash failures ×4 + manual fallback, audio failure + Retry, unsaved draft, crash restore, empty, too-short, out-of-range, official song, Linux dialout / udisks, unsigned app via README, focus).
Gaps:
- **STATUS staleness timeout** (when LEDs drop if STATUS stops) — not in sources.
- **Windows + bridge alone**: no product string and no INFO → app cannot confirm the bridge. Undecided presentation.
- Bridge LED label with no INFO ("Bridge – found", no version) [ASSUMPTION].
- Settings modal: no error/empty states needed beyond profile "unmigratable copy refused with a message" (AD-17) — not yet surfaced in the spine — low.
- Microcopy for underrun / LATE / link lost / bridge unplugged / flash failures 2–4 / audio / required update / Linux commands is **draft** [ASSUMPTION].

## 5. Visual reference coverage — thin

- `.working/layouts-main-screen-2026-11-20.html` and `.working/directions-oot-2026-11-20.html` linked inline (EXPERIENCE intro, DESIGN Layout & Spacing); spines-win stated once in EXPERIENCE.md.
- Not yet promoted to `mockups/` (Finalize step "artifacts promoted" pending) — links use `.working/` for now.
- The 7 mood screenshots in `imports/` are described in words in DESIGN.md › Brand & Style, never linked (by decision: copyrighted, git-ignored). This is intentional, not an orphan.
- No mock for: setup wizard, settings modal, readiness confirm in Hybrid, empty state in Hybrid, staff view in Hybrid, compact gamepad, 1024 px layout. Candidates for key-screen mocks.
- Known mock drift superseded by the spine: "Switch linked" label, "?" Home/Capture/+/− buttons, spike key mapping J K I U.

## Memlog conflicts resolved (later entry wins)

- Gamepad set: "ocarina + navigation only" → superseded by override: **full Pro Controller set**, no extra protection.
- HJKL: "H=A J=B K=X L=Y" → superseded by override: **vim-positional H=Y J=B K=X L=A**.
- Out-of-range display: "only in the report" → corrected (change entry) → **dimmed zones in the note view, per-note editable**.
- Piano roll creation: OPEN assumption → resolved by decision: **text editor + piano-roll drawing** in v1.
- "Evoke, don't copy": facilitator proposal → confirmed by decision.

## Conflicts with sources surfaced (not resolved)

- **ADR-002 says the wizard shows the "Pro Controller Wired Communication" step with a screenshot**; the confirmed "evoke, don't copy" constraint forbids console/game screenshots. Needs a user call (Open Question 3).
- Spec FR-01/02/04/11/12 vs UX scope additions (drawing, editor, conversion, per-note edits, Staff view, full gamepad) — listed in EXPERIENCE.md › Scope deltas for spec/epics update.

## Items needing the user (priority order)

1. Wired Communication screenshot vs evoke-don't-copy.
2. A.5 first-time-user flow and update flow (add or accept the gap).
3. Play gated on all three LEDs ok?
4. Status warning/error/unknown hexes + `amber-ink`; out-of-range label contrast.
5. STATUS staleness timeout; Windows bridge-alone presentation.
6. Undo/redo; note-editing gestures.
7. Banner persistence; end-card auto-dismiss duration.
8. Example song list; text-score file extension; countdown tick sound; AccessKit level; CSV export.
