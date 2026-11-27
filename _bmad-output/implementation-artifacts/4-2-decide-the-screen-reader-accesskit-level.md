---
baseline_commit: 338dfa7
---

# Story 4.2: Decide the screen-reader (AccessKit) level

Status: review

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As the owner,
I want to decide how far the app supports screen readers before any widget helper exists,
so that accessibility is built in from the start or consciously left out.

Origin: `epics.md` Epic 4, Story 4.2 (OQ-9). A decision story: the owner answers one question (below), the dev agent records the answer and propagates it. It must be done before Story 4.3 builds the shared painting helpers, because adding `widget_info` to helpers later means touching every widget that uses them (`review-egui-feasibility.md` row 52: "cheap if done in the shared helpers, expensive later").

**Numbering, for the record.** The same question has three numbers: `epics.md` OQ-9, `EXPERIENCE.md` Open Questions item 8, and `review-egui-feasibility.md` row 52 ("Open Question 13", an older numbering). Story 4.1 met the same split for OQ-7 / item 6. Record the outcome under the `epics.md` and `EXPERIENCE.md` numbers.

**Owner constraint: host-side only.** No bench, no Switch, no board. Verification is the edit of the planning docs, plus (only for "none") `cargo tree`, `cargo deny`, `cargo test` and the CI smoke run.

## Acceptance Criteria

1. **Decision recorded (from `epics.md`).** Given UX Open Question 9 and egui 0.37's AccessKit support, when the owner decides between "none", "basic labels via `widget_info` in shared helpers" and "full", then the decision and its reason are recorded in `EXPERIENCE.md` (Accessibility Floor bullet "Screen-reader support" rewritten as the decision; Open Questions item 8 marked RESOLVED with date, owner, reason, and "Same question as `epics.md` OQ-9"), and the OQ-9 line in `epics.md` is marked RESOLVED the same way.
2. **Propagated to Story 4.3 (from `epics.md`).** If the level is not "none", Story 4.3 must emit `widget_info` from every shared helper: the Story 4.3 text in `epics.md` gets a concrete AC (which helpers, which `egui::Role`, where the label comes from) and a test row is scheduled in `docs/test-plan.md` for Story 4.3 (see Task 3). If the level is "none", the last AC line of Story 4.3 in `epics.md` ("shared helpers emit `widget_info` if Story 4.2 decided so") is replaced by "no `widget_info` beyond what egui's built-in widgets emit (Story 4.2: none)".
3. **Build matches the decision.** "none": eframe's `accesskit` feature is removed from `app/gui/Cargo.toml` and everything stays green (Task 4). "basic" or "full": `app/gui/Cargo.toml` is unchanged (the feature stays; Story 4.1 D2 kept it pending this story).
4. **Later stories told.** The ACs of the later stories that draw custom widgets name what they owe under the chosen level (Task 2 lists them per level). Nothing else in their scope changes.

No AC contains a number to measure, so the E3-A1 AC → measurement table is not needed (the only checks are the doc edits, and for "none" the existing G.7, G.8 and G.9 rows).

## Options

What egui 0.37 does in each case is in Dev Notes › Facts. Effort is for one developer working with an agent, summed over all affected stories.

| Level | What it means concretely | Cost | Host-side testability | Risk |
|---|---|---|---|---|
| **none** | Ship without the OS accessibility adapter: drop eframe's `accesskit` feature. Built-in widgets (buttons, text edit, combo box, labels) still build AccessKit nodes inside egui, but nothing talks to VoiceOver / Narrator / NVDA / Orca. Custom-painted widgets (LEDs, gamepad, Roll, Staff, painted-symbol buttons) get no `widget_info`. | ~1 h now (one feature line, comments, `Cargo.lock`, rerun G.7–G.9). 0 later. Linux graph shrinks from 229 to 161 crates (the `zbus` / `atspi` / async stack goes); macOS and Windows lose 3 crates each. The X11 hook (Story 4.1 D3) becomes redundant on its own: winit's Wayland backend is no longer compiled. Keep the hook as a harmless guard. | Tests still work: `egui_kittest` turns AccessKit on in its own context, and its `eframe` dev feature re-enables `eframe/accesskit` in test builds only. But custom widgets have **no node**, so tests cannot find them by label or role (`get_by_label`, `get_by_role_and_label`): LED, gamepad and symbol-button tests must use coordinates and `output().shapes`. kittest's accessibility check (on by default) still panics on built-in input widgets without a name, so icon-only built-in buttons need a name anyway. | A blind or low-vision user gets an app the OS sees as one blank window. Reversing later means a retrofit pass over every custom widget (the expensive path of review row 52). |
| **basic** (recommended) | Keep the feature. Every shared helper of Story 4.3 that draws something interactive or informative takes an accessible label and calls `Response::widget_info(\|\| WidgetInfo::labeled(role, enabled, label))` with the closest `egui::Role`. Custom widgets later reuse the helpers or add the same one call: LED = `Role::Status` with its visible label (+ detail) as the name; gamepad control = `Role::Button` named "A", "ZL", "D-pad up"...; the Roll and Staff = one node each with a short summary name (e.g. "Roll view, 42 notes, B4 to F6"), no node per note. No live regions, no keyboard path beyond what EXPERIENCE already specifies. | ~0.5 day in Story 4.3 (one `a11y` helper + its test); then about one line per custom widget: 4.5 LEDs, 4.7 STOP (if custom-painted), 4.8 gamepad controls, 4.9 gamepad panel (`Role::Group`, name says when keys are active), 5.8 Roll and 9.7 Staff (one summary node each), 9.2 editor free (built-in `TextEdit`, plus `labelled_by`). Total ~1–1.5 days over Epics 4, 5, 9. Zero cost at run time when no screen reader runs: egui builds the tree only after the OS adapter asks for it (`InitialTreeRequested` → `enable_accesskit`). | Best of the three: every custom widget becomes queryable by role + name in kittest, so the LED, gamepad and modal tests of Stories 4.5–4.9 read like the user's view (`get_by_role_and_label(Role::Status, "Bridge v0.3")`, `get_by_role_and_label(Role::Button, "A").click()`). A guard test per helper uses `Context::has_accesskit_node(id)` and the node's role and label. | Low. It does not make the app fully usable without sight (the Roll is one summary node, note editing stays mouse-only), and nobody tests it with a real screen reader. Names and roles are only checked host-side. |
| **full** | "basic" plus: one node per visible note in Roll and Staff with pitch, time, fingering; keyboard selection and editing of notes (EXPERIENCE says note drawing is mouse-only today, so a scope change); live-region announcements (`accesskit::Live::Polite`) for LED changes, countdown, interruption banner and end card; modal focus handling announced as dialogs; a manual screen-reader pass (VoiceOver, NVDA or Narrator, Orca) on the three OS before each release. | +1.5–2 weeks spread over Epics 4, 5, 6, 9, 11, plus new stories for keyboard note editing; a new manual test row per release on three OS (the owner has a Mac; Windows and Linux need machines or VMs). Per-note nodes for ~2,000 notes on screen put pressure on the Roll budget (p95 ≈ 1.5 ms, Story 5.8) while a screen reader is running. | Tree shape is testable in kittest; real speech is not: VoiceOver / NVDA / Orca behaviour can only be checked by hand. | High for a solo project: largest scope, an EXPERIENCE scope change, manual tests that cannot run in CI, and the app's core loop (eyes on the console while the Switch plays) is visual anyway. AccessKit's Linux (AT-SPI) path is the least mature of the three (see Facts). |

## Recommendation

**basic.** Reasons:

1. **Cheapest now, expensive later.** Done inside the Story 4.3 helpers it is one closure per helper (~0.5 day) and about one line per custom widget afterwards (~1–1.5 days in total). Retrofitting it later touches every widget, which is exactly what OQ-9 was raised to avoid.
2. **It pays for itself in tests.** In egui 0.37 only widgets that report `WidgetInfo` get an AccessKit node (`Context::has_accesskit_node` docs), and `egui_kittest` finds widgets only through those nodes. With "basic", the LED, gamepad and modal tests of Stories 4.5–4.9 query by role + name; with "none" they fall back to coordinates and shape lists. kittest already insists on names for built-in input widgets, so part of the work is due anyway.
3. **"none" saves little and "full" costs a lot.** "none" only trims the Linux dependency graph (229 → 161 crates) and makes the X11 hook redundant; nothing else in the app gets simpler. "full" means weeks of work, a scope change to note editing, and manual screen-reader testing on three OS that CI cannot do, for an app whose main task is visual.

## Open questions for the owner

1. **Screen-reader level (UX OQ-9 / EXPERIENCE item 8): "none", "basic" (recommended) or "full"?** One word is enough; add a reason if it differs from the recommendation's.

   **Answer (owner, 2026-11-27): basic.** Every shared helper emits `widget_info` with the closest `egui::Role` and the visible label as the accessible name; the Roll and the Staff expose one summary node each; eframe's `accesskit` feature stays on. Reason: the recommendation's (cheap inside the Story 4.3 helpers, expensive to retrofit; makes custom widgets queryable by role + name in kittest; "none" saves little, "full" costs weeks plus manual screen-reader tests).

## Tasks / Subtasks

All tasks start after the owner's answer. Repo content in English.

- [x] **Task 1: Record the decision (AC 1).**
  - [x] 1.1 `_bmad-output/planning-artifacts/ux-designs/ux-bonooru-2026-11-20/EXPERIENCE.md` › Accessibility Floor: replace the bullet "Screen-reader support: egui AccessKit coverage not decided (Open Question)..." with the decision. For "basic": "Screen-reader support: basic (owner, Story 4.2, <date>). Every shared widget helper emits `widget_info` (closest `egui::Role` + the visible label as the accessible name); custom widgets that do not go through a helper do the same; the Roll and the Staff expose one summary node each. No live regions, no per-note nodes, no screen-reader test on real hardware." For "none": state that the OS accessibility adapter is not shipped and why. For "full": list the parts of the Options row.
  - [x] 1.2 Same file › Open Questions item 8: `**Screen reader / AccessKit** — RESOLVED <date> (owner, Story 4.2): <level>.` + one-line reason + "Same question as `epics.md` OQ-9." (pattern of items 3 and 6).
  - [x] 1.3 `_bmad-output/planning-artifacts/epics.md` › "UX open questions": the OQ-9 line gets `RESOLVED <date> (Story 4.2): <level>.` + "Same question as `EXPERIENCE.md` Open Questions item 8."
  - [x] 1.4 `review-egui-feasibility.md` is a historical review: leave it as is.

- [x] **Task 2: Propagate to later stories (AC 2, AC 4).** Edit `epics.md` only; no story files exist yet for these.
  - [x] 2.1 **basic or full:** Story 4.3, replace the last AC line with: "**And** every shared helper that draws an interactive or informative element takes an accessible label and calls `Response::widget_info` with the closest `egui::Role` (buttons `Button`, LEDs `Status`, symbols on their own `Image` with a name, decorative ornaments such as corner brackets, glow and halos none); a kittest test checks each one (`docs/test-plan.md` G.10)". Add the convention to the Epic 4 notes paragraph in one sentence.
  - [x] 2.2 **basic or full:** one AC line each, appended to the existing ACs, no other change: 4.5 (each LED is a `Role::Status` node named with its visible label + detail, e.g. "Bridge v0.3"); 4.8 (each gamepad control is a `Role::Button` node named after the control, e.g. "A", "ZL", "D-pad up"; an AccessKit click is a tap); 4.9 (the panel is a `Role::Group` node whose name says whether keys are active); 5.8 and 9.7 (the view is one node with a summary name: view, note count, range); 9.2 (the editor `TextEdit` is `labelled_by` its visible label).
  - [x] 2.3 **full only:** also add to 4.5 and the Epic 6 stories for countdown, interruption banner and end card a live-region AC (`accesskit::Live::Polite`); add per-note nodes to 5.8 and 9.7; raise the keyboard note editing scope change with the owner (it contradicts EXPERIENCE › Accessibility Floor "note drawing/moving/resizing (mouse)") before writing it into Epic 9; add a manual screen-reader row (three OS) to Epic 11's release checklist. Stop and ask the owner before creating new stories.
  - [x] 2.4 **none:** Story 4.3, replace the last AC line as stated in AC 2. Nothing else changes.

- [x] **Task 3: Pin the decision in the test plan (AC 2).** `docs/test-plan.md` › App GUI tests.
  - [x] 3.1 **basic or full:** add row G.10 (implemented by Story 4.3, not here): "Accessible helpers (`tests/a11y.rs`): in a kittest harness each shared helper that draws an interactive or informative element produces an AccessKit node (`Context::has_accesskit_node(resp.id)`) with the expected `Role` and a non-empty name equal to its visible label; decorative helpers produce none; the harness accessibility check stays on (no `with_accessibility_check(false)` in `app/gui/tests/`, source scan) | all pass | Story 4.2, UX-DR3". Red proof in Story 4.3: remove the `widget_info` call from one helper, the test names it.
  - [x] 3.2 **none:** amend G.8 to also check `cargo tree --locked -p ocb-app -e features,normal -i winit --target x86_64-unknown-linux-gnu --edges no-dev` has no line `winit feature "wayland"` (with "none" this is true; it makes AD-18 hold at build time, not only through the run-time hook), and add to G.9 that `accesskit_winit` is not in the normal graph (`cargo tree --locked -p ocb-app -e normal -i accesskit_winit --target <each>` prints nothing / errors "did not match").

- [x] **Task 4: Build change, "none" only (AC 3).**
  - [x] 4.1 `app/gui/Cargo.toml`: remove `"accesskit",` from the eframe features; update the comment block above `[dependencies]` (D2: "accesskit off by Story 4.2"). Keep `egui_kittest`'s `eframe` feature: it re-enables `eframe/accesskit` in test builds only (resolver 3 does not unify dev features into `cargo build`), and kittest needs it.
  - [x] 4.2 `app/gui/src/lib.rs` (the `with_x11()` hook, comment at line ~35): keep the hook, rewrite the comment: winit's Wayland backend is no longer compiled (no `accesskit_winit`), the hook stays as a guard for AD-18 should any future crate turn `winit/wayland` back on.
  - [x] 4.3 `cargo update -p ocb-app --offline` or a plain build to refresh `app/Cargo.lock`; check that the 68 Linux-only crates are gone (`cargo tree --locked -p ocb-app -e normal --target x86_64-unknown-linux-gnu --prefix none | sort -u | wc -l` ≈ 161).
  - [x] 4.4 `app/README.md` (the `gui/` line lists the eframe features) and `docs/test-plan.md` G.8 / G.9 per Task 3.2.
  - [x] 4.5 Green: `cargo fmt --all --check`, `cargo test --locked --workspace`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, `cargo deny --locked check licenses bans`, and the CI smoke run on three OS (G.7). Debug Log: the crate counts before/after and the new G.8 output.

- [x] **Task 5: Close.** Story file: Completion Notes with the decision, File List; `sprint-status.yaml`: `4-2-decide-the-screen-reader-accesskit-level: review` (then `done` after review, as usual). No code review layers are needed for a docs-only outcome ("basic" / "full"); for "none" run the normal review.

## Dev Notes

### Facts (checked 2026-11-27 in `~/.cargo/registry/src/*/` and with `ctx7` on `/emilk/egui/0.37.0`)

- **Two layers.** `egui 0.37.0` always depends on `accesskit 0.25` (the tree types; not optional). The eframe feature `accesskit` = `egui-winit/accesskit` = optional dep `accesskit_winit 0.34.2`, which brings the OS adapters: `accesskit_macos 0.27.2` (NSAccessibility → VoiceOver), `accesskit_windows 0.35.2` (UI Automation → Narrator, NVDA, JAWS), `accesskit_unix 0.24.1` (AT-SPI over D-Bus → Orca), with `accesskit_consumer 0.39.2`. Dropping the eframe feature removes only the adapters; `widget_info` still compiles and still feeds egui's own tree (used by kittest).
- **Lazy at run time.** eframe enables egui's tree only when the adapter receives `InitialTreeRequested`, i.e. when an assistive technology asks (`eframe-0.37.0/src/native/winit_integration.rs:193` → `ctx.enable_accesskit()`; `egui-winit` sends updates with `update_if_active`). Without a screen reader running, `widget_info` costs a closure call: `Context::accesskit_node_builder` returns `None` when AccessKit is off.
- **Built-in widgets are covered.** `widget_info` is called by `Button`, `Checkbox`, `RadioButton`, `Slider`, `DragValue`, `TextEdit`, `Label`, `Hyperlink`, `Image`, `ProgressBar`, `Spinner`, `ColorPicker`, `RangeSlider` and the containers `Area`, `Window`, `Panel`, `ScrollArea`, `CollapsingHeader`, `ComboBox` (`egui-0.37.0/src/widgets`, `src/containers`). egui's `docs/accessibility.md` (0.37.0): "Most built-in widgets register their role and accessible name when shown through the usual egui APIs."
- **Custom widgets need one call.** `Response::widget_info(&self, make_info: impl Fn() -> WidgetInfo)` (`response.rs:951`), called after interaction. In 0.37 `WidgetInfo.role` is an `accesskit::Role` re-exported as `egui::Role` (no `WidgetType` any more); constructors `WidgetInfo::new(role)`, `labeled(role, enabled, label)`, `selected(role, enabled, selected, label)`, `slider(enabled, value, label)`, `drag_value`, `text_edit`. The node also gets bounds, `disabled`, and `Focus` / `Click` actions from the `Sense`. `Response::labelled_by(id)` ties a field to its label. Raw access: `Context::accesskit_node_builder(id, |node| ...)` (e.g. `node.set_live(...)` for "full"). The guide's own example: `allocate_exact_size(size, Sense::click())`, paint, then `response.widget_info(|| WidgetInfo::labeled(Role::Button, ui.is_enabled(), "Open color picker"))`.
- **No `widget_info`, no node.** `Context::has_accesskit_node(id)` (`context.rs`): "Only widgets that report `WidgetInfo` do; a plain `Ui::allocate_rect` does not." So a custom-painted widget without `widget_info` is invisible to screen readers **and** to kittest queries.
- **AccessKit click = egui click.** An AccessKit `Action::Click` on a widget that senses clicks sets `FAKE_PRIMARY_CLICKED` (`context.rs:1767`), so `clicked()` fires. For the gamepad (Story 4.8) that means a screen-reader activation is a tap, never a hold, which fits "click = tap". The same block shows egui treats Space like Enter on a focused button; EXPERIENCE bans Space activation, which Story 4.7 handles (independent of this decision).
- **kittest.** `egui_kittest 0.37.0` ("Testing library for egui based on kittest and AccessKit") calls `ctx.enable_accesskit()` itself (`src/lib.rs:231`) and queries the AccessKit tree. Its `eframe` feature = `dep:eframe` + `eframe/accesskit`. After every `run()` it checks that every visible input-role node has a name and panics otherwise (`src/accessibility.rs`; off with `with_accessibility_check(false)`). Story 4.1's gallery test already relies on labels (`query_all_by_label`).
- **Dependency graph, measured with `cargo tree --offline -p ocb-app -e normal,build` (dev edges excluded), current vs a probe copy without the `accesskit` feature:** Linux 229 → 161 crates (gone: `accesskit*`, `atspi*`, `zbus*`, `async-*`, `wayland-protocols-plasma`, `smithay-client-toolkit 0.19`), macOS 142 → 139, Windows 144 → 141. In the probe, `cargo tree -e features,normal -i winit --target x86_64-unknown-linux-gnu` has no `winit feature "wayland"`: Story 4.1 D3's cause (`accesskit_winit` default features enable `winit/wayland`) disappears. `wayland-client` stays through `smithay-clipboard` (eframe `clipboard`), which does not affect winit's backend choice. No licence changes either way: every crate of the accesskit stack is already under the allow-list (Story 4.1 probe).
- **Per OS.** AccessKit's README lists adapters for Windows (UIA), macOS and Unix (AT-SPI), with Windows and macOS roughly at feature parity including text edits ([AccessKit repo](https://github.com/AccessKit/accesskit), [accesskit 0.21.1 docs](https://docs.rs/crate/accesskit/0.21.1)). egui's own README still says AccessKit "currently implements the native accessibility APIs on Windows and macOS" (stale: `accesskit_unix` is in our Linux graph). The GNOME accessibility blog lists AT-SPI-side gaps of AccessKit as used by GTK: no sentence text boundaries (Orca's Say All falls back to lines), no text attributes, table properties not exposed ([blogs.gnome.org/a11y/?p=120](https://blogs.gnome.org/a11y/?p=120); date unclear, may be older than accesskit_unix 0.24). Linux handlers may be called from another thread (internal to accesskit_unix; eframe handles it). No egui-specific VoiceOver or Orca bug was found in this search; none of this was tried with a real screen reader. Those gaps matter mostly for "full" (long text, tables); "basic" uses names and roles only.

### Custom widgets planned (where the cost lands)

From `EXPERIENCE.md` › Component Patterns, `DESIGN.md` › Components and `epics.md`:

| Story | Custom-painted element | basic | full (in addition) |
|---|---|---|---|
| 4.3 | Shared helpers: focus ring, glow, halo, dashed rect, digit cells, painted symbols ▶ ■ ⏮ ▾ ▲ ⌫ ↵ ↑ ♯ ♪ ↶ ↷, corner brackets, modal dialog | `widget_info` in the helpers that make clickable or informative elements; ornaments none; modal = egui container (`Window`/`Area` nodes) + a named title | dialog role + focus moved into the modal and announced |
| 4.5 | Status LEDs (colour + symbol + label, tooltip on hover and focus) | `Role::Status`, name = visible label + detail | live region on state change |
| 4.6 | "Firmware update needed" modal, inline hint | helper from 4.3 | announced as alert dialog |
| 4.7 | STOP, bottom bar | name "Stop" if custom-painted (built-in button: free) | — |
| 4.8 | Gamepad silhouette and controls (not in the Tab ring) | `Role::Button` per control, named | sticks as sliders with value, locked state announced |
| 4.9 | Gamepad panel focus, "Keys active" chip | `Role::Group`, name includes "keys active" | announcement on focus lock |
| 5.8 / 5.9 | Roll view (lanes, note bars, ruler, fit proposal) | one node, summary name | one node per visible note (pitch, time, fingering), ruler as slider |
| 6.x | Countdown, interruption banner, end card | built-in labels / helper names | live regions |
| 9.2 | Text editor | free (`TextEdit`) + `labelled_by` | — |
| 9.3–9.7 | Selection, note drawing, context menu, Staff view | Staff: one summary node; context menu is egui's (free) | keyboard note selection and editing (scope change) |

### Current state of the files this story may touch

- `app/gui/Cargo.toml`: eframe `=0.37.0`, default features off, features `accesskit, clipboard, default_fonts, wgpu, x11`; Linux-only `winit =0.30.13` with `x11` for the hook; dev `egui_kittest =0.37.0` with `eframe`. Story 4.1 D2: "Kept: `accesskit` until Story 4.2 decides the screen-reader level (if 4.2 says "none", drop it there; the X11 hook stays harmless)". Only "none" changes it.
- `app/gui/src/lib.rs` ~line 35–43: `native_options()` with the `#[cfg(target_os = "linux")]` `with_x11()` hook and a comment naming `accesskit_winit` as the reason. Under "none" keep the hook, update the comment. G.2 asserts the hook is set exactly on Linux: must stay green.
- `app/deny.toml`: `[graph] all-features = true`; unchanged by any option.
- `docs/test-plan.md` › App GUI tests: G.1–G.9 exist; G.10 is new (basic/full), G.8/G.9 amended (none).
- `EXPERIENCE.md` › Accessibility Floor (line ~306) and Open Questions item 8 (line ~464); `epics.md` OQ-9 (line ~163), Story 4.3 last AC line (line ~861).
- Must be preserved: everything Story 4.1 built (G.1–G.9 green on three OS).

### Previous story intelligence (Story 4.1)

- Decisions in that story were recorded under both OQ numbers (OQ-7 / item 6) with date, owner and source; reuse that wording pattern.
- D2/D3 (eframe features, X11 hook) and the Debug Log's G.7/G.8 red proofs explain why AccessKit and Wayland are linked; the G.7 Docker reproduction (ubuntu 24.04 under Xvfb with a bogus `WAYLAND_DISPLAY`) is the way to re-check Linux if "none" changes the graph.
- kittest panics on missing glyphs: painted symbols are not text, so their accessible names must be plain words ("Play", "Stop"), never the symbol characters (Nunito Sans has no `▶`, `♯`).
- Comment in `tests/gallery.rs`: clipped rows stay in the AccessKit tree, so kittest queries see off-screen widgets; keep that in mind for G.10.

### Git intelligence

Last commits: `338dfa7 Mark Story 4.1 done`, `985a06a` (OQ-7 owner review, gallery overlaps), `7d48352` (4.1 review fixes), `e56dc0b` (ocb-app shell). No code touches AccessKit beyond the eframe feature and the X11 hook comment.

### Project Context Rules

No `project-context.md` exists. Rules from memory and the repo: all repo content in English; never edit or commit `_bmad/` or `bmad-*` / `gds-*` skill folders; ask the owner one question at a time; research before asking (done above); host-side only for this story.

### References

- `_bmad-output/planning-artifacts/epics.md` › Epic 4 (notes: "a small decision story for the AccessKit level (OQ-9)"), Stories 4.2, 4.3, 4.5, 4.8, 4.9, 5.8, 9.2, 9.7; UX open questions OQ-9; UX-DR3
- `_bmad-output/planning-artifacts/ux-designs/ux-bonooru-2026-11-20/EXPERIENCE.md` › Accessibility Floor; Component Patterns; Open Questions item 8
- `_bmad-output/planning-artifacts/ux-designs/ux-bonooru-2026-11-20/review-egui-feasibility.md` › row 52, recommendation 17; `validation-report.md` › "AccessKit widget_info (#52)"
- `_bmad-output/implementation-artifacts/4-1-app-shell-with-theme-fonts-and-main-screen-regions.md` › AC 7, D2, D3, Debug Log G.7/G.8; Open questions 2
- `app/gui/Cargo.toml`, `app/gui/src/lib.rs`, `app/deny.toml`, `app/README.md`, `docs/test-plan.md` › App GUI tests
- Crate sources: `egui-0.37.0/src/response.rs` (`widget_info`, `labelled_by`, `fill_accesskit_node_*`), `egui-0.37.0/src/context.rs` (`accesskit_node_builder`, `has_accesskit_node`, `enable_accesskit`, AccessKit click), `egui-0.37.0/src/data/output.rs` (`WidgetInfo`), `eframe-0.37.0/Cargo.toml` and `src/native/winit_integration.rs`, `egui-winit-0.37.0/src/lib.rs`, `egui_kittest-0.37.0/Cargo.toml`, `src/lib.rs`, `src/accessibility.rs`, `accesskit-0.25.2/src/lib.rs` (`Role::Status`, `Group`, `Canvas`, `Live`)
- `ctx7` `/emilk/egui/0.37.0`: `docs/accessibility.md` (Labels, Custom widgets, Testing), `crates/egui_kittest/README.md` (Accessibility check), `README.md` (FAQ, accessibility)
- Web: [AccessKit repository](https://github.com/AccessKit/accesskit), [accesskit 0.21.1 on docs.rs](https://docs.rs/crate/accesskit/0.21.1), [GNOME a11y blog on GTK + AccessKit](https://blogs.gnome.org/a11y/?p=120)

## Dev Agent Record

### Agent Model Used

Claude Opus 5.5 (claude-opus-5-5)

### Debug Log References

- AC 3 (build matches "basic"): `app/gui/Cargo.toml` and `app/Cargo.lock` unchanged (`git status app/` clean). `cargo tree --locked --offline -p ocb-app -e normal -i accesskit_winit --target <t> --depth 0` prints `accesskit_winit v0.34.2` for `x86_64-unknown-linux-gnu`, `aarch64-apple-darwin` and `x86_64-pc-windows-msvc`: the OS adapter stays in the normal graph on the three OS.
- E3-A2 (red proof for checks added now): no check is implementable in this story. G.10 tests the Story 4.3 helpers, which do not exist yet; its red proof is scheduled in Story 4.3 (row text). A guard that eframe's `accesskit` feature stays on was considered and not added: it is not a task of this story, and it cannot be a `cargo test` (the dev-dependency `egui_kittest` with its `eframe` feature re-enables `eframe/accesskit` in test builds, so a test would stay green even if the feature were removed from `[dependencies]`); it would have to be a `cargo tree -e normal` CI step like G.8. Left as a suggestion for Story 4.3 review.
- No code or test changed, so `cargo test`, `cargo fmt --check` and clippy were not rerun (no Rust input differs from `338dfa7`, which CI ran green).

### Completion Notes List

- Story created 2026-11-27: ultimate context engine analysis completed, comprehensive developer guide created. Decision open (one question to the owner).
- Owner decision 2026-11-27: **basic** (recorded under Open questions for the owner). Every shared helper emits `widget_info` with the closest `egui::Role` and the visible label; the Roll and the Staff are one summary node each; eframe's `accesskit` feature stays on.
- Task 1: `EXPERIENCE.md` Accessibility Floor bullet rewritten as the decision; Open Questions item 8 RESOLVED (date, owner, reason, "Same question as `epics.md` OQ-9"); `epics.md` OQ-9 RESOLVED with the cross-reference to item 8; UX-DR62 notes "(basic, Story 4.2)". `review-egui-feasibility.md` (row 52, "Open Question 13") left as is per Task 1.4: it is a historical review.
- Task 2: Story 4.3's last AC replaced by the concrete `widget_info` AC (roles per helper kind, G.10); the convention added to the Epic 4 notes in one sentence; one AC line appended to 4.5 (LED `Role::Status`), 4.8 (gamepad `Role::Button`, AccessKit click = tap), 4.9 (panel `Role::Group`, name says whether keys are active), 5.8 (Roll summary node), 9.2 (`TextEdit` `labelled_by`), 9.7 (Staff summary node). 2.3 (full) and 2.4 (none) do not apply.
- Task 3: `docs/test-plan.md` row G.10 added (implemented by Story 4.3) and the section intro says so. 3.2 (none) does not apply.
- Task 4 ("none" only) does not apply: `app/gui/Cargo.toml` unchanged, AC 3 checked with `cargo tree` (Debug Log).
- Task 5: story and `sprint-status.yaml` set to `review`. Docs-only outcome, no code review layers needed (Task 5).

### File List

- `_bmad-output/implementation-artifacts/4-2-decide-the-screen-reader-accesskit-level.md` (new: this story, decision recorded)
- `_bmad-output/implementation-artifacts/sprint-status.yaml` (4-2 → review)
- `_bmad-output/planning-artifacts/ux-designs/ux-bonooru-2026-11-20/EXPERIENCE.md` (Accessibility Floor, Open Questions item 8)
- `_bmad-output/planning-artifacts/epics.md` (OQ-9, UX-DR62, Epic 4 notes, Stories 4.3, 4.5, 4.8, 4.9, 5.8, 9.2, 9.7)
- `docs/test-plan.md` (App GUI tests: G.10, intro)

### Change Log

- 2026-11-27: Owner decided screen-reader level "basic"; recorded in EXPERIENCE.md and epics.md, propagated to Stories 4.3, 4.5, 4.8, 4.9, 5.8, 9.2, 9.7, test row G.10 scheduled for Story 4.3. Status → review.
