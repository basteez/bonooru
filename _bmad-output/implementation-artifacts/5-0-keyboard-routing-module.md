---
baseline_commit: d731a7b9f4a6982814598e48624ae17d4ca37acd
---

# Story 5.0: Keyboard routing module

Status: review

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As the owner,
I want one module to own every keyboard rule of the app (Esc, Space, Enter, Tab and the arrows, the gamepad keys, Cmd/Ctrl shortcuts, text fields),
so that Epic 5's Listen on Space, Cmd/Ctrl+O and text fields are added as rules in one place instead of special cases spread over `app.rs`, `keys.rs`, `pad_keys.rs` and `modal.rs`.

Origin: Epic 4 retrospective action **E4-A2** (`epic-4-retro-2026-11-28.md` §5 insight 2, §6 risks "Space" and "Shortcuts", §7: "One module owns keyboard routing (Space, Enter, Esc, the gamepad keys, Cmd/Ctrl shortcuts, text fields); the G.20 and Story 4.9 rules move there. Done when the module exists with its tests; Listen on Space is a routing rule, not a special case"). Not in `epics.md`: a cleanup story ahead of Story 5.1 (owner, 2026-11-28).

**Owner constraint: tests only.** No bench, no Switch, no manual step. A pure refactor plus two new rules (Listen on Space, global shortcuts) that have no consumer yet; every existing behaviour stays exactly as Stories 4.7 and 4.9 left it.

## Acceptance Criteria

1. **One owner.** A new module `ocb_app::ui::keyboard` (`app/gui/src/ui/keyboard.rs`) holds every keyboard rule. `ui::keys` and `ui::pad_keys` are removed (their code moves in); the key scan inside `App::panel_focus` (Esc without modifiers, Tab / Shift+Tab, the four arrows, AccessKit `Focus`), the key-up extraction in `App::logic`, the focus-lock `EventFilter`, and the dialog's key rules in `ui::modal` (`ANSWER_KEYS`, the fresh-Esc check of the opening frame, the Esc consume) are called through it. Outside `ui/keyboard.rs`, no file under `app/gui/src/` matches the word-bounded tokens `\bKey::`, `egui::Key\b`, `\bKey\b` inside an `egui` `use` list, `Event::Key\b`, `consume_key`, `key_pressed`, `key_released`, `keys_down` or `set_focus_lock_filter` (a source-scan test enforces it; `gallery.rs` and the smoke run included). The pure model's `PadKey::…` and `Pad::key_down`/`key_up` are not egui input and stay legal (`\bKey::` does not match `PadKey::`; an `InputState::key_down(Key::…)` call is caught by its `Key::` argument).
2. **One table.** The module states its rules once, as data the tests iterate (a `const` table or a `match` with one arm per rule, whichever reads better) and in its module doc, in the order of `EXPERIENCE.md` › Interaction Primitives: for each key, the phase (frame start / gamepad pass / `logic`), the context (text edit focused, a widget focused, a dialog open, gamepad panel live), the action and whether the event is consumed, read only, or left in the input.
3. **Behaviour unchanged.** Every existing test of Stories 4.1–4.9 passes with no assertion changed: G.19, G.20, G.21, G.22, G.23, G.24, G.25, G.18, G.10, G.11. Allowed edits to existing tests: import paths and helper names (`keys::frame_start` → `keyboard::frame_start`, `pad_keys::pad_key` → `keyboard::pad_key`), and moving `g20_frame_start_keys` and G.24's `pad_key` cases into the new G.26 file (AC 8). Each such edit is listed in the Debug Log with its reason.
4. **Listen on Space is a rule.** `keyboard::frame_start` returns `FrameKeys { stop, listen, shortcuts }`. `listen` is true when the frame has a **fresh** Space press (`repeat == false`) with no Cmd/Ctrl/macOS Cmd held (Shift and Alt do not block it), no text edit focused and no dialog open (open or requested this frame; passed in by the caller); one press → one `listen` (a held Space's repeats never set it). It is set whether or not a widget or the gamepad panel has focus (Space is not a gamepad key). The Space consume rules of Story 4.7 are unchanged (consumed outside a text edit; Cmd/Ctrl+Space only while a widget has focus). `App` does nothing with `listen` yet: Story 5.11 wires it (a comment at the call site says so). No other code checks Space.
5. **Global shortcuts are rules.** `keyboard::shortcut(key, modifiers) -> Option<Shortcut>` maps the `EXPERIENCE.md` global shortcuts: Cmd/Ctrl+Enter → `PlayFromCursor`, Cmd/Ctrl+Shift+Enter → `PlayFromStart`, Cmd/Ctrl+O → `Open`, Cmd/Ctrl+S → `Save`, Cmd/Ctrl+Z → `Undo`, Cmd/Ctrl+Shift+Z → `Redo` (Cmd/Ctrl = `command_held`; Alt held → none; matched by logical `key`). `FrameKeys::shortcuts` lists the frame's fresh presses that match, in order, in every context (text edit, gamepad focus and dialogs included). In this story they are **reported, not consumed** and nothing acts on them: Cmd+L, Cmd+Enter and the rest stay in the input exactly as G.25 asserts today. The story that adds a consumer (5.7 Open, Epic 6 Play, Epic 9 Undo/Save) decides there whether its row consumes.
6. **Text fields.** One rule decides "a text edit has focus" (`ctx.text_edit_focused()`, read once per frame in `frame_start`): with it, Space and Enter repeats stay for the text edit, `listen` is false, Esc is still STOP (egui clears the text focus on Esc first, as today), shortcuts are still reported.
7. **Order and E4-A5.** The three phases keep their place and order in the frame: `frame_start` in the first pass before any widget (every mode), the gamepad pass after the main screen in the first pass (Main mode), the key-ups in `App::logic` in every pass. No release or STOP logic moves into `ui`-only code: the logic-only release path (`App::pad_background_keys`) gets its key-ups from `keyboard`, and `g25`'s logic-only key-up test still passes. The STOP request site and "STOP before any pad line of the frame" are unchanged.
8. **Tests (G.26, `app/gui/tests/keyboard.rs`).** (a) Pure table tests over `egui::Event` values, no `Context`: every rule of AC 2 with each context flag on and off, fresh vs repeat, each modifier (none, Shift, Alt, Ctrl, Cmd, macOS Cmd), including: Esc any modifiers → stop, repeat → none; Space → listen and consumed, Space repeat → consumed, no listen; Cmd+Space with nothing focused → left in input, no listen; Space in a text edit → left, no listen; Space with a dialog open → consumed, no listen; Space with the gamepad live → listen; Enter repeat outside a text edit → dropped, in a text edit → kept; each shortcut and its near misses (Alt+Cmd+O, Cmd+P, plain O); the 18 pad keys by physical key and `None` for Space, Tab, Escape, X, 1, F, Delete; a pad event without `physical_key` → nothing; Cmd+L in the pad pass → reported with `command`, left in input. (b) The kittest harness moved from `g20_frame_start_keys` (a `TextEdit`, Esc read not consumed, Space consumed / left), extended with the Listen and shortcut fields. (c) The source scan of AC 1. G.20's and G.24's rows in `docs/test-plan.md` say those cases live in G.26 now.
9. **Docs.** `app/README.md` module list: `ui::keyboard` replaces `ui::keys` and `ui::pad_keys`. `EXPERIENCE.md` › Interaction Primitives gains one sentence: the rules live in `ui::keyboard`; Listen on Space and the global shortcuts are recognised there since Story 5.0 and acted on by the stories that add the features. `docs/test-plan.md`: new row G.26, G.20 and G.24 updated, the G.* intro sentence names Story 5.0. `sprint-status.yaml`: E4-A2 `done` with the PR.
10. **Green, no new flake.** `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (two full runs locally), `cargo deny check` pass in `app/`; CI green on ubuntu, windows and macos at the PR head. Every new or changed test has a red proof in the Debug Log (E4-A3).

### AC → measurement (retro action E3-A1)

| AC | Measured by (host) | Field / result |
|---|---|---|
| 1 | G.26 `g26_one_owner` (source scan of `app/gui/src/**/*.rs`) | no forbidden token outside `ui/keyboard.rs`; `ui/keys.rs`, `ui/pad_keys.rs` absent |
| 2 | G.26 table tests iterate the table; review reads the module doc against `EXPERIENCE.md` | every rule has a test row |
| 3 | `cargo test --workspace` (G.10, G.11, G.18–G.25) | same test names pass; `git diff` on those files shows imports/moves only |
| 4 | G.26 table + harness (`listen`) | true once per fresh plain/Shift/Alt Space outside text edit and dialog; false otherwise |
| 5 | G.26 `shortcut` cases; G.25 Cmd+L / Cmd+Enter cases unchanged | six shortcuts, near misses none; events still in the input |
| 6 | G.26 harness with a focused `TextEdit` | Space kept, `listen` false, `stop` true on Esc, shortcuts reported |
| 7 | G.20, G.23, G.25 (incl. the logic-only key-up test) | unchanged pass |
| 8 | G.26 | all pass |
| 9 | review; `docs_constants`-style checks n/a | rows present |
| 10 | local runs + CI | all green |

## Tasks / Subtasks

- [x] **Task 1: Red-first tests for the new rules (AC: 4, 5, 6, 8, 10)**
  - [x] 1.1 Create `app/gui/tests/keyboard.rs` with the G.26 cases for `listen` and `shortcut` against a stub API (or behind the new names), run them, record the failing output (red proof) in the Debug Log.
- [x] **Task 2: The module (AC: 1, 2, 4, 5, 6)**
  - [x] 2.1 Create `app/gui/src/ui/keyboard.rs`: move `FrameKeys`, `frame_start`, `command_held` from `keys.rs`; `pad_key`, `KeyEdge`, `take` from `pad_keys.rs` (rename `take` → `pad_keys` or keep, your choice; note it).
  - [x] 2.2 Add the pure core (`route`-style functions over `&egui::Event` plus a small context struct) that `frame_start` and the pad pass use, so the table tests need no `Context`.
  - [x] 2.3 Add `listen`, `shortcuts`, `Shortcut`, `shortcut()`; `frame_start` takes `dialog_open: bool`.
  - [x] 2.4 Add `focus_scan(ctx) -> FocusKeys { esc_plain, lock, tab, accesskit_focus }` (the scan now inline in `App::panel_focus`), `lock_filter() -> EventFilter`, `key_ups(events) -> Vec<PadKey>` (the `logic` extraction), and the dialog helpers (`ANSWER_KEYS` or an `answer_keys_up(ctx)` check, `fresh_esc(ctx)`, `dialog_takes_esc(ctx)` doing the `consume_key(Modifiers::NONE, Key::Escape)`).
  - [x] 2.5 Delete `ui/keys.rs`, `ui/pad_keys.rs`; update `ui/mod.rs`.
- [x] **Task 3: Callers (AC: 1, 3, 7)**
  - [x] 3.1 `app.rs`: `ui()` calls `keyboard::frame_start(ctx, self.modals.is_open() || <requested this frame>)` — see Dev Notes › Dialog flag; `panel_focus` uses `focus_scan`; `pad_input` uses the pad pass and `lock_filter`; `logic` uses `key_ups`. Comment at the `listen` / `shortcuts` fields: "Story 5.11 / 5.7 act on these".
  - [x] 3.2 `modal.rs`: arming and Esc via the `keyboard` helpers; behaviour identical (G.11 / `tests/modal.rs` unchanged).
  - [x] 3.3 `gallery.rs` / `smoke.rs`: check for key handling; route any through `keyboard`.
- [x] **Task 4: Move and keep tests (AC: 3, 8)**
  - [x] 4.1 Move `g20_frame_start_keys` (and its harness) from `tests/stop_app.rs` and the `pad_key` cases from `tests/gamepad_focus.rs` into `tests/keyboard.rs`; update imports elsewhere. List every edited test file and why in the Debug Log.
  - [x] 4.2 Add `g26_one_owner` (scan) and the full table tests.
- [x] **Task 5: Red proofs for every new or changed test (AC: 10; E4-A3)** — see Dev Notes › Red proofs; record command, failing assertion, then the passing run.
- [x] **Task 6: Docs (AC: 9)** — README, EXPERIENCE.md sentence, test-plan G.20 / G.24 / G.26 and intro, sprint-status E4-A2.
- [x] **Task 7: Green (AC: 10)** — fmt, clippy, two `cargo test --workspace` runs, deny; push; CI green on three OS.

## Dev Notes

### Scope in one paragraph

Move, don't redesign. Today the keyboard rules live in four places: `ui/keys.rs` (Story 4.7: Esc read for STOP, Space consumed, Enter repeats dropped), `ui/pad_keys.rs` (Story 4.9: physical key → `PadKey`, Cmd/Ctrl left in the input, mapped keys consumed), `app.rs` (`panel_focus`'s scan of Esc/Tab/arrows/AccessKit Focus at ~l.606–660, the `EventFilter` lock at ~l.580, the key-up filter in `logic` at ~l.915–930) and `ui/modal.rs` (`ANSWER_KEYS` l.159, fresh Esc ~l.290–305, `consume_key(Escape)` ~l.324). They all go into `ui::keyboard`; the focus *decisions* (who gains/loses the panel, `keys=on/off` lines, `release_all`) stay in `App` and `gamepad` — only the reading of keys moves. Two rules are new and have no consumer yet: Listen on Space and the global shortcuts. Epic 5 then adds Listen (5.11) and Cmd/Ctrl+O (5.7) by acting on a field, and the text fields of Epic 9 by the one text-edit rule.

### Decisions taken at story creation (change only with a reason in the Debug Log)

- **D1 Placement.** `ui::keyboard` under `ui/` (AD-18: egui input under `ui/`; the pure policy crates at the root name no GUI crate). Its core functions take `&egui::Event` values and a plain context struct, no `Context`, so G.26 can test the table without a harness. Not a crate-root module: it needs `egui::Key`/`Event`.
- **D2 Listen with a dialog open → no Listen.** A dialog is modal (the scrim takes clicks; the Listen button under it cannot be clicked), and "every shortcut has a visible button" (EXPERIENCE). Space is still consumed (Story 4.7 rule, so it never clicks a dialog button).
- **D3 Listen modifiers.** Cmd/Ctrl/macOS Cmd block it (Cmd+Space is the OS's; Cmd/Ctrl combos are global shortcuts); Shift and Alt don't — the same rule as the gamepad keys (owner, Story 4.9, 2026-11-28).
- **D4 Shortcuts reported, not consumed, in 5.0.** Consuming keys for features that don't exist would change G.25's "Cmd+L / Cmd+Enter stay in the input" for nothing. Each consumer story flips its own row. Repeats of a held shortcut are not reported (fresh presses only), like Esc.
- **D5 Shortcut match by logical key.** Unlike the pad keys (physical, positional), Cmd+O / Cmd+S / Cmd+Z follow the layout's letters, as every desktop app does (on AZERTY Cmd+Z is the key labelled Z). `Event::Key::key` is egui-winit's logical key (falls back to physical).
- **D6 Listen and the gamepad.** With the gamepad panel focused, Space still sets `listen` (EXPERIENCE: "including while the gamepad has focus; Space is not a gamepad key"). `pad_key(Key::Space)` stays `None`.
- **D7 No behaviour change elsewhere.** If moving code exposes a latent bug, do not fix it silently: record it, keep the old behaviour, and raise it as a finding (or a GitHub issue) — this story's PR must be reviewable as a move.

### Dialog flag (AC 4)

`frame_start` runs first in `ui()`, before the firmware dialog request (Story 4.6, `ui()` ~l.960–980) and before `Modals::show`. Pass `self.modals.is_open()`; a dialog requested later in the same frame cannot be known at frame start. Accept that edge (a Space in the very frame the firmware dialog arrives sets `listen`); document it in the module doc as an accepted limit. Story 5.11 must re-check `modals.is_open()` before starting a preview anyway (it acts after the frame's dialogs are known). In Gallery mode pass `self.modals.is_open()` too.

### Current state of the files this story touches (read before editing)

- `app/gui/src/ui/keys.rs` (77 lines): `FrameKeys { stop }`; `pub(crate) command_held(Modifiers)` (Cmd, Ctrl or macOS Cmd); `frame_start(ctx)` reads a fresh Esc (any modifiers, never consumed), and unless `ctx.text_edit_focused()`: removes Space presses (all when a widget has focus; otherwise only those without Cmd/Ctrl) and Enter repeats. **Move whole. Preserve:** every rule; called once per frame in pass 0 only (input reaches pass 0 only, egui 0.37 `RawInput::take`).
- `app/gui/src/ui/pad_keys.rs` (94 lines): `pad_key(physical)` table (18 keys); `KeyEdge { key, pressed, repeat, command }`; `take(ctx)` retains every non-mapped event and every mapped event with Cmd/Ctrl, records the edges in order. **Move whole. Preserve:** physical key only (no logical fallback, D1 of 4.9); order; Cmd/Ctrl left in input; mapped ones consumed.
- `app/gui/src/app.rs` (1069 lines): `use crate::ui::keys::{self, FrameKeys}` (l.22); `pad_input` (l.482–604): `pad_keys::take(ctx)` at l.564 when `focused_now`, then the lock filter `EventFilter { tab, horizontal_arrows, vertical_arrows: true, escape: false }` at ~l.580; `panel_focus` (l.606–720): the input scan (l.609–660) → `(esc_plain, lock_event, tab_event, any_press, accesskit_focus)`; `any_press` is pointer, keep it in `App` or return it from the scan (your call, note it); `logic` (l.889–936): key-ups via `pad_keys::pad_key` on `i.raw.events` (raw, not `events`: the logic-only pass); `ui` (l.938+): `keys::frame_start` first pass only. **Change:** call sites only. **Preserve:** every comment's reasoning (move the key-related ones into `keyboard.rs`), the review-2026-11-28 rules (mapped keys consumed in every focused frame incl. focus-gain and Shift+Esc frames, applied only when `panel_focused && !ignore_new`; a key-up always honoured; repeats count in the lock scan).
- `app/gui/src/ui/modal.rs` (468 lines): `ANSWER_KEYS` (l.159) used for arming (every answer key up once since open); `fresh_esc` (l.290–305) when `opening_esc_dismisses`; Esc consumed with `Modifiers::NONE` when top modal or first frame and no popup (l.320–328). **Change:** read through `keyboard` helpers. **Preserve:** the Esc contract with Story 4.7 documented on `Modals`; Cmd/Ctrl+Esc ignored by the dialog.
- `app/gui/tests/stop_app.rs` (`g20_frame_start_keys`, l.379–~460) and `app/gui/tests/gamepad_focus.rs` (`pad_key` cases l.~690–715): move to G.26.
- `app/gui/tests/common/mod.rs`: key-event helpers (`physical_key` set, as winit) — reuse in G.26.
- Not touched: `app/core`, `app/services`, `app/sim`, `app/tools/*`, `firmware/`, `gamepad.rs` (pure model), `ui/gamepad.rs` (drawing; check it has no key code — grep showed none), `stop.rs`, `gating.rs`, `leds.rs`.

### egui 0.37 facts (from Story 4.9, still valid; `~/.cargo/registry/src/*/egui-0.37.0`)

- `Event::Key { key, physical_key: Option<Key>, pressed, repeat, modifiers }`; `repeat` set by egui from `keys_down`; kittest `Harness::key_press` pushes `physical_key: None` — push your own events.
- `Memory::begin_pass` clears focus on unmodified Esc before the app runs and applies the previous frame's focus-lock filter; `set_focus_lock_filter` only works from the widget's second focused frame.
- `Modifiers::command` is Cmd on macOS, Ctrl elsewhere; `mac_cmd` macOS only; `command_held` checks all three.
- eframe 0.37 `App::logic` (`eframe-0.37.0/src/epi.rs:150-170`): runs before every `ui`, and alone while the window is hidden; then the rest of `ctx.input` is "that of the last shown frame". The key-up path in `logic` reads `i.raw.events` (keep).

### Anti-flake rules (E4-A4; apply to every new or moved test)

- No tight wall-clock upper bound: none of the G.26 tests needs time at all; keep them clock-free.
- Time references taken before the action (only if a moved test uses a clock: keep its current order).
- Wait for the log line, not only the state (G.20/G.25 helpers already do; don't replace them).
- High kittest `max_steps` when a worker thread runs (G.26 runs none; moved app tests keep their rig).
- Unique log markers per test; no shared global state between tests.

### E4-A5 note

No release or STOP logic is added. The only `logic` code touched is the key-up extraction, which must keep reading `raw.events` so a key-up seen only by the logic-only pass still releases (G.25 review test). Reviewers check that no `release_all`, `request_stop` or `pad.key_up` call moved from `logic` into `ui`.

### Red proofs (E4-A3; minimum list for the Debug Log)

For each, break the rule, run the named test, paste the failing assertion, restore, paste the pass:
- `listen` set on a repeat; set with Cmd held; not set with Shift; set in a text edit; set with a dialog open; not set while the gamepad is live.
- `shortcut`: Cmd+Shift+Z mapped to `Undo`; Alt+Cmd+O mapped to `Open`; the physical key used instead of the logical one.
- Shortcuts consumed (G.25's "stays in the input" and G.26 fail).
- `g26_one_owner`: put a `Key::Escape` back in `app.rs` → fails naming the file and line.
- Table completeness: drop one table row → the iteration test fails.
- Moved `g20_frame_start_keys` cases: consume Esc in `frame_start` → fails; stop consuming Space → fails.
- Moved `pad_key` cases: map `Key::X` → fails.
- Logic key-ups: read `events` instead of `raw.events` → G.25's logic-only test fails (or explain why it can't in kittest and prove it another way).

### Testing standards

`cargo test --workspace` with `egui_kittest` 0.37 on the CPU; no GPU, no window, no real serial port, no real log (G.13). The G.26 pure tests construct `egui::Event::Key` values directly. Test names `g26_*`. Clippy `-D warnings` on all targets.

### Previous story intelligence (4.7, 4.9, Epic 4 retro)

- 4.7: Esc read never consumed, one STOP per frame, Space consumed, Enter repeats dropped; tests must use realistic event sequences (egui marks a press of a held key as repeat).
- 4.9: physical keys; Cmd/Ctrl left for shortcuts; the review found five keyboard bugs in the app glue (key-up in logic-only pass, dialog gating, lock fallback vs AccessKit, consume in every focused frame, right/middle click) — each has a G.25 test; they are the regression net for this move.
- Retro: the 4.9 work was where keyboard ownership got hard to follow (insight 2); E4-A3 red proofs for every test; E4-A4 flake rules.

### Git intelligence

Last commits on `main`: `d731a7b` (merge #54, E4-A1 Linux VM H.28 PASS), `1f9985c` (Devlog #4), `fb3eab2` (Epic 4 retro). Last code story 4.9 (PR #50). Branch `story/5-0-keyboard-routing-module` from up-to-date `main`. Commit style: one long descriptive sentence, story number in parentheses.

### Library / framework requirements

No new dependency (E4-A6 does not apply). egui/eframe/egui_kittest stay `=0.37.0`. `cargo deny` unchanged.

### Architecture compliance

- AD-18: egui input under `ui/`; pure policy at the crate root untouched.
- AD-16: the UI never waits; nothing here touches the worker.
- UX-DR41 (Esc = STOP), UX-DR61 (Space never activates a button; Enter once per press), UX-DR62, UX-DR37 (gamepad keys): preserved, now in one place.

### File structure

- New: `app/gui/src/ui/keyboard.rs`, `app/gui/tests/keyboard.rs`.
- Removed: `app/gui/src/ui/keys.rs`, `app/gui/src/ui/pad_keys.rs`.
- Changed: `app/gui/src/ui/mod.rs`, `app/gui/src/app.rs`, `app/gui/src/ui/modal.rs`, `app/gui/tests/stop_app.rs`, `app/gui/tests/gamepad_focus.rs`, other tests' imports as needed, `app/README.md`, `docs/test-plan.md`, `_bmad-output/planning-artifacts/ux-designs/ux-bonooru-2026-11-20/EXPERIENCE.md`, `_bmad-output/implementation-artifacts/sprint-status.yaml`.

### Project Structure Notes

Matches AD-18 and `app/README.md`. One variance from 4.9's note ("`ui::pad_keys` is separate from `ui::keys` because …"): the phase difference stays, but as two functions of one module, which is what E4-A2 asks.

### Project Context Rules

No `project-context.md`. Repo rules: every repo file in English; never stage `_bmad/`, `.claude/` or bmad-*/gds-* skill folders; one question at a time to the owner; test environment facts in `docs/test-plan.md`; search upstream (egui sources) before raising an open question.

### Open questions for the owner

None. D2–D6 are technical defaults drawn from `EXPERIENCE.md`; the code review can overturn any of them.

### References

- [Source: _bmad-output/implementation-artifacts/epic-4-retro-2026-11-28.md#5–7] E4-A2, E4-A3, E4-A4, E4-A5
- [Source: _bmad-output/planning-artifacts/ux-designs/ux-bonooru-2026-11-20/EXPERIENCE.md#Interaction Primitives] global and context shortcuts, gamepad focus and mapping
- [Source: _bmad-output/planning-artifacts/epics.md#Story 5.11] Space consumed at frame start, never activates a button
- [Source: _bmad-output/implementation-artifacts/4-7-stop-esc-and-the-bottom-bar.md] frame-start rules
- [Source: _bmad-output/implementation-artifacts/4-9-gamepad-keyboard-focus-and-compact-layout.md] pad keys, focus lock, review findings 2026-11-28
- [Source: app/gui/src/ui/keys.rs, app/gui/src/ui/pad_keys.rs, app/gui/src/app.rs, app/gui/src/ui/modal.rs]
- [Source: docs/test-plan.md] G.20, G.24, G.25

## Dev Agent Record

### Agent Model Used

Claude Opus 5.5 (claude-opus-5-5)

### Debug Log References

Red proofs (E4-A3). Each: the mutation applied to the green tree, `cargo test -q -p ocb-app --test <t>`, the failing assertion, then the file restored (the full green runs below are after every restore). Script: a scratch `red.py` (not committed) that applies one string replacement, runs the test and restores.

| # | Mutation | Test | Red result |
|---|---|---|---|
| R1 | Listen also on a Space repeat (`fresh &&` dropped) | `g26_rule_table`, `g26_frame_start_keys` | `left: Some(SpaceListen) right: Some(SpaceConsume)` at `repeat=true`; harness "a Space repeat is Listen" |
| R2 | Listen with Cmd/Ctrl held (`!command` dropped) | `g26_rule_table` | `Modifiers { ctrl: true }` widget focused → `SpaceListen` ≠ `SpaceConsume` |
| R3 | Shift blocks Listen | `g26_rule_table` | `Modifiers { shift: true }` → `SpaceConsume` ≠ `SpaceListen` |
| R4 | Listen in a text edit (the `SpaceText` arm disabled) | `g26_rule_table`, `g26_frame_start_keys` | `text_edit: true` → `SpaceListen` ≠ `SpaceText`; harness "Listen in a text edit" |
| R5 | Listen with a dialog open (`!cx.dialog_open` dropped) | `g26_rule_table`, `g26_frame_start_keys` | `dialog_open: true` → `SpaceListen` ≠ `SpaceConsume`; harness "Listen with a dialog open" |
| R6 | No Listen while a widget / the gamepad has focus | `g26_rule_table` | `widget_focused: true` → `SpaceConsume` ≠ `SpaceListen` |
| R7 | Cmd+Shift+Z → `Undo` | `g26_shortcuts` | `left: Some(Undo) right: Some(Redo)` |
| R8 | Alt no longer blocks shortcuts | `g26_shortcuts` | "Enter Modifiers { command: true }" + Alt → `Some(PlayFromCursor)` ≠ `None` |
| R9 | Shortcuts matched by the physical key | `g26_shortcuts` | the AZERTY case (`key: Z, physical: W`) panics at `keyboard.rs:371` (no shortcut) |
| R10 | Shortcuts consumed (G.26) | `g26_rule_table`, `g26_frame_start_keys` | `o.keep` false; harness "Cmd+O consumed" |
| R11 | Shortcuts consumed (G.25) | `g25_modifiers_and_consumption` | panics at `gamepad_keys_app.rs:990` (Cmd+Enter no longer in the input) |
| R12 | `const _K: egui::Key = egui::Key::Escape;` added to `app.rs` | `g26_one_owner` | "key input outside ui::keyboard: app.rs:24: Key::, app.rs:24: egui::Key" |
| R13 | A new `Rule::Extra` added to the enum, `ALL` and `index` but given no test row | `g26_rule_table` | "Extra has no test row" (first try: `Extra` added to the enum only stayed green, because `ALL` did not list it; fixed by `Rule::index`, an exhaustive `match`, so a rule cannot compile without a place in the table, and the table test checks `ALL[i].index() == i`) |
| R14 | `frame_start` consumes Esc | `g26_rule_table`, `g26_frame_start_keys` | `o.keep` false; harness "Esc consumed" |
| R15 | Space no longer consumed (`SpaceConsume` kept) | `g26_rule_table`, `g26_frame_start_keys` | `o.keep` true; harness "Space not consumed with a dialog open" |
| R16 | `pad_key` maps `Key::X` | `g26_pad_keys_table`, `g26_rule_table`, `g26_pad_keys_frame` | `X: left: Some(C) right: None` |
| R17 | Pad pass consumes Cmd keys and keeps plain ones | `g26_pad_keys_frame` | `left: [(L,false),(K,false),(X,false),(J,false)] right: [(L,true),(X,false),(J,false)]` |
| R18 | `App::logic` reads `i.events` instead of `i.raw.events` | `gamepad_keys_app` | **stays green.** Not provable on the host: kittest always runs `logic` then `ui` on the same input, and when `logic` runs nothing has been consumed yet, so `events` equals `raw.events`. The difference only exists in eframe's hidden-window `run_logic` path, which kittest cannot drive. Cover: the pure `key_ups` is tested in `g26_rule_table` (mapped releases only), the G.25 logic-only test drives `App::pad_background_keys` directly, and the call site still passes `&i.raw.events`, exactly as Story 4.9 left it (`git diff` of `app.rs`) |
| R19 | `ANSWER_KEYS` drops Space | `g26_dialog_helpers` | `left: [Enter, Enter, Escape] right: [Enter, Space, Escape]` (first try stayed green because the test looped over the mutated const itself; now it loops over a literal list and checks the const) |
| R20 | The dialog also takes Cmd+Esc | `g26_dialog_helpers` | panics at `keyboard.rs:501` ("Cmd+Esc taken by the dialog") |
| R21 | Alt+Tab counts as a focus Tab | `g26_rule_table` | `Tab Modifiers { alt: true }` → `Some(FocusTab)` ≠ `None` |
| R22 | `has_fresh_esc` counts repeats | `g26_rule_table` | `assertion failed: !keyboard::has_fresh_esc(&[esc(true)])` |
| R23 | Enter repeats kept outside a text edit | `g26_rule_table` | panics at `keyboard.rs:203` (`o.keep`) |
| R24 | A pad event with no physical key mapped by its logical key | `g26_rule_table`, `g26_pad_keys_frame` | `left: Some((PadKey, KeyEdge { key: L, .. })) right: None` |

Moved and changed tests (AC 3):
- `tests/stop_app.rs`: `g20_frame_start_keys` and its `KeysState` harness moved out to `tests/keyboard.rs` as `g26_frame_start_keys` (same assertions, plus the Listen, shortcut and dialog-flag cases); the `ui::keys` import dropped. No other G.20 test changed.
- `tests/gamepad_focus.rs`: `g24_pad_keys_table` moved out as `g26_pad_keys_table` (same assertions, `keyboard::pad_key`); `Key` and `PadKey` imports dropped. No other G.24 test changed.
- No assertion of G.10, G.11, G.18, G.19, G.21, G.22, G.23 or G.25 changed; those files are untouched.

Found on the way: the first `g26_one_owner` run failed on a real leftover, `set_focus_lock_filter` in `App::pad_input`; it moved into the module as `keyboard::lock_focus(ctx, id)`.

Green runs (macOS, 2026-11-28, after all restores): `cargo fmt --check` ok; `cargo clippy --workspace --all-targets -- -D warnings` ok; `cargo test --workspace` twice: 545 passed, 0 failed each (Story 4.9 ended at 539: +8 G.26, −2 moved); `cargo deny check`: advisories, bans, licenses, sources ok.

### Completion Notes List

- `ui::keyboard` (`app/gui/src/ui/keyboard.rs`) now holds every keyboard rule as one table, `Rule` / `Rule::ALL` with an exhaustive `Rule::index`, and the module doc mirrors `EXPERIENCE.md` › Interaction Primitives. The pure core is `frame_route`, `pad_route`, `focus_route`, `key_ups`, `has_fresh_esc` and `shortcut`; the egui adapters are `frame_start(ctx, dialog_open)`, `pad_keys`, `focus_scan`, `lock_focus`, `answer_key_down`, `fresh_esc` and `dialog_takes_esc`. `ui::keys` and `ui::pad_keys` are deleted.
- New rules with no consumer yet: `FrameKeys::listen` (Story 5.11) and `FrameKeys::shortcuts` (5.7, Epic 6, Epic 9). The shortcuts are reported, not consumed (D4). `App::ui` passes `self.modals.is_open()` as the dialog flag, with the accepted limit for a dialog requested later in the same frame written in the module doc.
- `App`'s focus decisions, its release logic and the STOP site are unchanged (E4-A5). Only the reading of keys moved: `panel_focus` uses `focus_scan` (the pointer `any_press` stays in `App`), `pad_input` uses `pad_keys` and `lock_focus`, and `logic` uses `key_ups(&i.raw.events)`.
- `ui::modal` reads its answer keys and Esc through the module. Its behaviour is identical: the G.11 modal tests are untouched and pass.
- Naming: `pad_keys::take` is now `keyboard::pad_keys`, and the focus-lock filter is set by `keyboard::lock_focus` (`lock_filter()` still returns the filter).
- One red proof cannot be shown on the host (R18, explained in the Debug Log).
- Task 1 order: the new tests could not compile before the new API existed, so the module was written first, and every new test was then proven red by mutating the finished code (R1–R24), not by a stub run.

### File List

- `app/gui/src/ui/keyboard.rs` (new)
- `app/gui/tests/keyboard.rs` (new, G.26)
- `app/gui/src/ui/keys.rs` (deleted)
- `app/gui/src/ui/pad_keys.rs` (deleted)
- `app/gui/src/ui/mod.rs`
- `app/gui/src/app.rs`
- `app/gui/src/ui/modal.rs`
- `app/gui/src/gamepad.rs` (doc comment)
- `app/gui/src/ui/widgets.rs` (doc comment)
- `app/gui/tests/stop_app.rs`
- `app/gui/tests/gamepad_focus.rs`
- `app/README.md`
- `docs/test-plan.md`
- `_bmad-output/planning-artifacts/ux-designs/ux-bonooru-2026-11-20/EXPERIENCE.md`
- `_bmad-output/implementation-artifacts/sprint-status.yaml`
- `_bmad-output/implementation-artifacts/5-0-keyboard-routing-module.md`

## Change Log

- 2026-11-28: story created (ready-for-dev).
- 2026-11-28: implemented: `ui::keyboard` owns every key rule (table `Rule::ALL`), Listen on Space and the global shortcuts recognised, G.26 added, G.20/G.24 cases moved; 24 red proofs (one not provable on the host, explained); status review.
