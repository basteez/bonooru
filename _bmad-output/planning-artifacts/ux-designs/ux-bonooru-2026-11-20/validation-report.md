# Validation Report — bonooru

- **DESIGN.md:** `_bmad-output/planning-artifacts/ux-designs/ux-bonooru-2026-11-20/DESIGN.md`
- **EXPERIENCE.md:** `_bmad-output/planning-artifacts/ux-designs/ux-bonooru-2026-11-20/EXPERIENCE.md`
- **Run at:** 2026-11-20T14:36+01:00

## Overall verdict

Adequate. A downstream architect or story developer can source-extract the pair cleanly: every token reference resolves, every source path resolves, memlog overrides are reflected, and the Hardware & Protocol Coupling table is a strong bridge to the firmware contract. The rubric found no critical findings and four high ones (A.5 flow, import failures, Windows wizard step 1, countdown contrast).

The egui feasibility review (egui/epaint 0.37, winit 0.30) found nothing that forces leaving egui: 40 of 52 elements are feasible, 11 costly, 1 infeasible by arithmetic (split-view height). Its five high items were all spine wording or interaction-model gaps.

After this update (memlog entries 77–82 applied, including Flow 4, Play gating, app-level undo, the draggable split and the 12 approved review-fix defaults), all high findings from both reviews are fixed. What remains is owner input: status hexes, staleness timeout, Windows bridge-alone, selection model confirmation, update flow, AccessKit, draft copy, key-screen mocks and the ocarina auto-open question.

## Category verdicts

- Flow coverage — adequate
- Token completeness — adequate
- Component coverage — adequate
- State coverage — adequate
- Visual reference coverage — thin
- Bloat & overspecification — adequate
- Inheritance discipline — adequate
- Shape fit — strong
- Extra reviewer: egui feasibility — 40 feasible · 11 costly · 1 infeasible

## Status after this update

- fixed: 44
- open-question: 6
- deferred: 1

## Findings by severity

### Critical (0)

None.

### High (9)

**[Rubric · Flow coverage]** — No flow for CAP-10 / A.5 (first-time non-developer) (§ EXPERIENCE › Key Flows)  
All three flows used the owner; the spec's release signal had no flow.  
Fix: Add Flow 4 with a non-developer protagonist on Windows.  
Status: **fixed** — Flow 4 "Giulia" added: Windows, SmartScreen via README, wizard, wrong wiring and Wired Communication off as failure beats, climax "You played Ode to Joy!".

**[Rubric · Token completeness]** — Countdown overlay contrast fails (amber 2.75:1, cream sub-line 4.02:1) (§ DESIGN › Components › Countdown)  
Veil at 55 % over parchment composites to ~#776d57.  
Fix: Raise the veil or switch numeral colour; state the ratio.  
Status: **fixed** — Veil 80 % (#16110ccc, composite ~#413a2d); numerals cream with 2 px frame outline and amber halo; cream 8.85:1 for numeral and sub-line.

**[Rubric · State coverage]** — Song import failures missing (§ EXPERIENCE › State Patterns)  
Corrupt/unsupported file, .mid with no notes, 0 % in range, missing last song / Recent.  
Fix: Add rows with copy and fallback.  
Status: **fixed** — Rows added per memlog default 6; banner "Couldn't open <file>: <reason>", current song kept, missing Recent removed.

**[Rubric · State coverage]** — Wizard step 1 cannot complete on Windows (§ EXPERIENCE › IA › Setup wizard)  
No product string on Windows; INFO needs the controller.  
Fix: Complete on drive-vanish + new port.  
Status: **fixed** — Step 1 done when RPI-RP2 vanishes and a new 2E8A:000A port appears; product string checked where supplied (memlog default 4).

**[egui feasibility]** — Split view cannot fit 19 lanes + zones at 1024 × 680 (#39, infeasible by arithmetic) (§ DESIGN › Layout; EXPERIENCE › Responsive)  
Fix: See review-egui-feasibility.md.  
Status: **fixed** — Draggable divider (persisted), editor min 4 lines, lane area scrolls vertically, empty zones collapse to a thin strip.

**[egui feasibility]** — Gamepad keyboard focus model (#23) (§ EXPERIENCE › Interaction Primitives)  
Fix: See review-egui-feasibility.md.  
Status: **fixed** — Panel-level egui focus, focus-lock filter for arrows/Tab, children not focusable, keys by physical position (memlog default 2).

**[egui feasibility]** — Space activates a focused button (#27) (§ EXPERIENCE › Context shortcuts)  
Fix: See review-egui-feasibility.md.  
Status: **fixed** — Space consumed at frame start for preview everywhere except the editor; buttons activate with Enter (memlog default 1).

**[egui feasibility]** — Undo and roll-to-text rewrite (#29) (§ EXPERIENCE › Undo / redo)  
Fix: See review-egui-feasibility.md.  
Status: **fixed** — One app-level history; TextEdit undoer not used; roll edits rewrite only affected tokens, caret kept.

**[egui feasibility]** — Linux file drop missing on Wayland (#42/#45) (§ EXPERIENCE › Responsive & Platform)  
Fix: See review-egui-feasibility.md.  
Status: **fixed** — Build without eframe wayland feature (X11/XWayland); Open… always available; architecture note in Upstream edits required (memlog default 3).

### Medium (18)

**[Rubric · Flow coverage]** — No flow for CAP-6 update offers (§ EXPERIENCE › State Patterns (Required / Recommended update))  
Path from "Update now" back to playing is defined only by table rows.  
Fix: Add a short update flow.  
Status: **open-question** — Open Question 8: add a Key Flow or accept states-only coverage.

**[Rubric · Token completeness]** — Six tokens still [ASSUMPTION] (status-warning/error/unknown, amber-ink, scrim, typography.code) (§ DESIGN frontmatter)  
Fix: Approve or replace.  
Status: **open-question** — Open Question 7. The error red itself is decided (memlog default 8); the hexes still need approval.

**[Rubric · Token completeness]** — Out-of-range label has two candidate values (3.3:1 vs 4.7:1) (§ DESIGN › Colors, Do's and Don'ts)  
Fix: Commit one value.  
Status: **fixed** — out-of-range-ink = #55504a, 4.71:1 on the zone (memlog default 5).

**[Rubric · Component coverage]** — No visual spec for the generic focus ring (§ DESIGN › Components)  
Accessibility Floor requires a ring on every focusable control.  
Fix: Add a focus-ring component and parchment variant.  
Status: **fixed** — focus-ring (2 px amber, 2 px offset) and focus-ring-on-parchment (amber-ink) as one shared helper.

**[Rubric · Component coverage]** — Components used in EXPERIENCE with no DESIGN row (§ DESIGN › Components)  
Progress bar, checkbox, context menu, log view, Advanced fields, port pick list, inline hint, song rail.  
Fix: Add short visual rows.  
Status: **fixed** — All added as tokens + prose (memlog default 12).

**[Rubric · Component coverage]** — Note selection model unspecified (§ EXPERIENCE › Note editing)  
Fix: Add a selection row.  
Status: **fixed** — Selection model added as [ASSUMPTION]: click, Shift+click, marquee, drag-move with lane/frame-grid snap, edge resize, Delete/Backspace when note view focused, context menu. Confirmation: Open Question 6.

**[Rubric · State coverage]** — No state for a link or console drop during the countdown (§ EXPERIENCE › State Patterns)  
Fix: Abort the countdown with the interruption banner.  
Status: **fixed** — Countdown cancelled, no START, interruption banner (memlog default 7).

**[Rubric · State coverage]** — STATUS staleness timeout and Windows bridge-alone presentation undecided (§ EXPERIENCE › Hardware coupling)  
Fix: Decide.  
Status: **open-question** — Open Questions 3 and 4 (candidate 150 ms recorded).

**[Rubric · Visual reference coverage]** — Mocks live in .working/ and are not linked at the sections they illustrate (§ both spines)  
Fix: Promote to mockups/ and add per-section reference lines.  
Status: **deferred** — Per-section "→ Reference" lines added (IA, Layout, Roll, out-of-range, official song, gamepad, banner, countdown). Promotion to mockups/ deferred to Finalize (Open Question 15).

**[Rubric · Visual reference coverage]** — No Hybrid mocks for wizard, confirm, Settings, empty state, end card, compact gamepad, 1024 px (§ DESIGN › Components)  
Fix: Render key-screen mocks before story writing.  
Status: **open-question** — Open Question 15.

**[Rubric · Inheritance discipline]** — Glossary drift: "timing profile" vs spec "mapping profile" (§ EXPERIENCE › IA, Settings; DESIGN › Song panel)  
Fix: Use "mapping profile" everywhere.  
Status: **fixed** — Glossary added; all copy uses "mapping profile" (memlog default 9).

**[Rubric · Inheritance discipline]** — DESIGN contradicts itself on red (§ DESIGN › Colors › Avoid)  
Fix: Scope the ban to ember / note view; name status-error.  
Status: **fixed** — status-error named as the one error red; ember reserved; no red for generic errors inside the note view (memlog default 8).

**[Rubric · Inheritance discipline]** — ADR-002 "screenshot" conflicts with no-screenshots constraint (§ decisions.md ADR-002)  
Fix: Text path + original diagram; log ADR amendment.  
Status: **fixed** — Spines use the original illustration + menu path; ADR-002 / ARCHITECTURE-SPINE edit listed in EXPERIENCE › Upstream edits required (not applied here).

**[egui feasibility]** — Concave silhouette fill (#8) (§ DESIGN › Virtual gamepad)  
Fix: See review-egui-feasibility.md.  
Status: **fixed** — Union of convex shapes + one stroked outline.

**[egui feasibility]** — Glyph coverage ⌫ ↵ ↑ ♯ ♪ (#5) (§ DESIGN › Typography)  
Fix: See review-egui-feasibility.md.  
Status: **fixed** — Painted as Painter shapes unless TTF coverage is verified.

**[egui feasibility]** — Focus ring spec (#24) (§ DESIGN › Components; EXPERIENCE › Accessibility)  
Fix: See review-egui-feasibility.md.  
Status: **fixed** — Shared helper, 2 px amber / amber-ink on parchment; shows after mouse use (accepted).

**[egui feasibility]** — Playhead steps at 20 Hz during Play (#33) (§ EXPERIENCE › Hardware coupling)  
Fix: See review-egui-feasibility.md.  
Status: **fixed** — Interpolate from last now_us + elapsed; visualizer uses the same time.

**[egui feasibility]** — Glow technique (#9, #10) (§ DESIGN › Elevation)  
Fix: See review-egui-feasibility.md.  
Status: **fixed** — Shadow / blur_width on rounded rects and circles only; text gets a halo; inset glow = stacked strokes.

### Low (24)

**[Rubric · Flow coverage]** — CAP-3 profile editing and AD-15 CSV export have no flow (§ EXPERIENCE › Settings modal)  
Fix: Accept as states only; resolve CSV.  
Status: **fixed** — States-only accepted for CAP-3; CSV export is not user-facing in v1 (memlog default 11).

**[Rubric · Flow coverage]** — Flow 2 step 3 relies on an assumed "accept" action (§ EXPERIENCE › Flow 2)  
Fix: Reword to "leaves the other as proposed".  
Status: **fixed** — Reworded; delete done via the context menu.

**[Rubric · Token completeness]** — button-stop-live foreground is a literal #ffffff (§ DESIGN frontmatter)  
Fix: Reuse on-ember.  
Status: **fixed** — Now {colors.on-ember}, 4.96:1.

**[Rubric · Token completeness]** — Component dimensions live only in prose (§ DESIGN › Components)  
Fix: Tokenise or declare prose the spec.  
Status: **fixed** — Key-chip size tokenised; Components intro states prose dimensions are the spec.

**[Rubric · Token completeness]** — Load-bearing pairs without stated ratio (ghost fill, playhead, on-ember) (§ DESIGN › Colors)  
Fix: Add one line to Colors.  
Status: **fixed** — Ghost fill 1.27:1 (border carries it, 4.71:1), playhead 3.98:1, on-ember 4.96:1, focus rings and error red ratios added.

**[Rubric · Component coverage]** — Gamepad visualizer "mirrors what the controller sends", but the protocol echoes no input (§ EXPERIENCE › Virtual gamepad)  
Fix: Derive from the compiled timeline at now_us.  
Status: **fixed** — Stated in Component Patterns and Hardware coupling.

**[Rubric · Component coverage]** — Song-rail collapse behaviour undefined (§ EXPERIENCE › Responsive)  
Fix: Add a behaviour row.  
Status: **fixed** — Song rail row added ([ASSUMPTION]: user toggle, never automatic).

**[Rubric · Component coverage]** — Tooltip behaviour undefined; LED fixes only on hover (§ EXPERIENCE › Status LED)  
Fix: Show tooltips on focus too.  
Status: **fixed** — Tooltips on hover and keyboard focus ([ASSUMPTION]); hover-only actions banned.

**[Rubric · State coverage]** — No state for unmigratable profile, gamepad in Link lost (idle), drop/Open during Play (§ EXPERIENCE › State Patterns)  
Fix: One row each.  
Status: **fixed** — Rows added (AD-17 message; clicks ignored with hint and drop/Open ignored, both [ASSUMPTION]).

**[Rubric · State coverage]** — About 14 operational strings are draft (§ EXPERIENCE › Voice and Tone)  
Fix: Run one approval pass.  
Status: **open-question** — Open Question 13.

**[Rubric · Visual reference coverage]** — Staff view cites "mock" without file or direction (§ DESIGN › Staff view)  
Fix: Cite direction 2.  
Status: **fixed** — Cites direction 2 "Parchment" of the directions file.

**[Rubric · Bloat & overspecification]** — Layout table and prose repeat token values inline (§ DESIGN › Layout & Spacing)  
Fix: Drop inline numbers.  
Status: **fixed** — Prose now names tokens only.

**[Rubric · Bloat & overspecification]** — Mood-reference paragraph describes seven screenshots (§ DESIGN › Brand & Style)  
Fix: Trim to lifted traits.  
Status: **fixed** — Trimmed to the five lifted traits.

**[Rubric · Inheritance discipline]** — Component names drift between files (§ both spines)  
Track picker/select, banners, readiness confirm, fit proposal, preview/dry run.  
Fix: Align names.  
Status: **fixed** — Names aligned; canonical list in EXPERIENCE › Foundation.

**[Rubric · Inheritance discipline]** — "No Nintendo names" contradicted by necessary strings (§ EXPERIENCE › Voice and Tone)  
Fix: Scope to franchise/game/character/logo names.  
Status: **fixed** — Rule rescoped; platform and settings names allowed.

**[Rubric · Inheritance discipline]** — "rescan every 1 s" and "Esc stops" credited to unsourced convention (§ EXPERIENCE › Coupling, Voice)  
Fix: Tag or cite.  
Status: **fixed** — Rescan interval tagged [ASSUMPTION]; safety note cites ARCHITECTURE-SPINE + memlog.

**[egui feasibility]** — Tabular figures (#4) (§ DESIGN › Typography)  
Fix: See review-egui-feasibility.md.  
Status: **fixed** — No tnum in egui shaper; fixed-width digit cells.

**[egui feasibility]** — Weights (#2) (§ DESIGN › Typography)  
Fix: See review-egui-feasibility.md.  
Status: **fixed** — wght axis or per-weight families; .strong() is not bold; 800 dropped.

**[egui feasibility]** — Tab order vs CentralPanel (#46) (§ DESIGN › Layout; EXPERIENCE › Context shortcuts)  
Fix: See review-egui-feasibility.md.  
Status: **fixed** — Manual region rects with scope_builder in reading order.

**[egui feasibility]** — Sub-pixel hairlines (#32) (§ DESIGN › Shapes)  
Fix: See review-egui-feasibility.md.  
Status: **fixed** — Strokes floored at 1 physical px.

**[egui feasibility]** — Minimum size in logical points (#38) (§ EXPERIENCE › Responsive)  
Fix: See review-egui-feasibility.md.  
Status: **fixed** — Stated; 1366×768 at 125 % needs 100 % scaling (accepted).

**[egui feasibility]** — Native pieces: rfd, Wayland decorations (#45) (§ EXPERIENCE › Foundation)  
Fix: See review-egui-feasibility.md.  
Status: **fixed** — rfd named; X11 build keeps system decorations.

**[egui feasibility]** — AccessKit widget_info (#52) (§ EXPERIENCE › Accessibility)  
Fix: See review-egui-feasibility.md.  
Status: **open-question** — Still undecided; note that helpers must emit widget_info if adopted.

**[egui feasibility]** — Gesture split drag-empty vs drag-note (#30) (§ EXPERIENCE › Note editing)  
Fix: See review-egui-feasibility.md.  
Status: **fixed** — Drag on empty = marquee; pan by wheel / Shift+wheel / middle-drag ([ASSUMPTION], Open Question 6).

## Reviewer files

- `review-rubric.md`
- `review-egui-feasibility.md`
