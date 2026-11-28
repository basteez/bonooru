---
baseline_commit: d62be7acce29099bf51fe44a2283c0c447d41212
---

# Story 4.9: Gamepad keyboard focus and compact layout

Status: review

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As the owner,
I want to drive the gamepad from the keyboard when it has focus, and keep it usable on small windows,
so that navigating the game is quick and the keyboard never presses buttons by accident.

Origin: `epics.md` Epic 4, Story 4.9, the last story of Epic 4 (UX-DR37 panel focus, focus lock, focused look and key mapping; UX-DR6 compact gamepad between 1024 and 1279 px; the Story 4.2 "basic" line: the panel is a `Role::Group` node, parent of the Story 4.8 control nodes; FR-22 "full Pro Controller set, keyboard only while focused"; M5 "on-screen gamepad drives the console (mouse + keyboard)"). It builds on Story 4.3 (painting helpers, `paint_focus_ring`, the modal and its Esc contract), 4.5 (tooltips with `theme::tooltip_frame`, the rig block format), 4.6 (input in the first pass only, the gate), 4.7 (`ui::keys::frame_start`: Esc read and never consumed, Space consumed, Enter repeats dropped; `ButtonState`; the forward note "after STOP the gamepad drops its own held set (mouse **and keys**)") and 4.8 (the pad model `ocb_app::gamepad`, `ui::gamepad`, the release rules a–d with the hidden-window part in `App::logic`, the `pad` log lines, the H.35 bench and its lessons: the console's connection screen took two A presses; the owner noticed at the eye check that the keyboard bindings do nothing yet). It carries the Epic 3 retro rules E3-A1 (AC → measurement table below), E3-A2 (no bench tool is new or changed; the new `pad` lines are the instrument and each verdict they can show has a host test proving both values) and E3-A3 (rig block with the bench start checklist; dry run against `ocb-sim` before the bench).

**Scope: host code + docs + a short macOS bench with OoT running (H.36, about 25 minutes) + an owner eye check. No firmware change, no wire change, no `ocb-core` change, no worker (`ocb-services`) change.** In scope: the panel-level focus (click, Tab, AccessKit), the focus-lock filter, the focused look (inset amber ring + wash, "Keys active" chip), the key mapping by physical key with its hold rules (fresh press, 50 ms minimum, auto-repeat ignored, chords, duplicates, opposites, Cmd/Ctrl combos left alone), the release rules extended to keys and to panel focus loss, the compact variant below 1280 pt (200 pt region, button grid, key hints in tooltips), the `Role::Group` panel node with every panel node as its child, `pad` log lines for keys and focus, host tests (G.24, G.25, updates to G.1, G.10, G.15, G.17, G.18, G.20–G.23), H.36 and its rig block, docs. Out of scope: the song rail (Story 5.12); Space = preview (Epic 5); the lit-key visualizer and "Locked · live view" (Epic 6); Cmd/Ctrl shortcuts themselves (Epic 5/6: this story only leaves them alone); Windows and Linux runs (issue #43); issue #45 itself (this story must not make it worse).

## Acceptance Criteria

Numbers marked *(story)* are set by this story; every number is measured as listed in the AC → measurement table.

1. **Panel focus (UX-DR37).** The gamepad panel has one panel-level focus id, `ui::gamepad::panel_id()`, sensed over the whole gamepad region with `Sense::FOCUSABLE` only (never `CLICK`: egui fake-clicks a focused clickable widget on Enter/Space, and Enter must stay "+"). The 18 controls stay unfocusable (Story 4.8 AC 13). The panel gains focus: (a) by a primary press anywhere in the panel that reaches it (a control, a stick or empty panel space; not through a dialog, which takes the press first), in the press frame; a press on a control still presses it in that same frame (Story 4.8 AC 4 unchanged), and the panel keeps focus through the press, the hold and the release (egui surrenders the focus of a widget that is not hovered at a click, and the controls lie above the panel: the app requests the panel's focus again in every frame where a press or a release lands on the panel); (b) by Tab in reading order (LED × 3 → panel); (c) by an AccessKit `Focus` action. It loses focus: by a click outside the panel (egui `SurrenderFocusOn::Clicks`), by Esc without modifiers (egui `Memory::begin_pass`; Esc is also STOP, AC 8), when a dialog opens (the dialog takes focus), and when the window loses focus or is hidden or minimized (the app surrenders it, D4). One `pad keys=on source=<click|tab|other>` line per gain and one `pad keys=off reason=<esc|dialog|window_blur|elsewhere>` line per loss (Dev Notes › Log lines).
2. **Focus lock (UX-DR37).** While the panel has focus, Tab, Shift+Tab and the four arrows never move egui focus out of it, also in the first frame after it gained focus: the app calls `Memory::set_focus_lock_filter(panel_id, EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: false })` in every frame the panel has focus, **after** any `request_focus` of that frame (a `request_focus` resets the filter); and because egui accepts the filter only from the second frame of focus (`had_focus_last_frame`), a frame in which the panel had focus at the end of the previous first pass and lost it with a Tab or arrow key event and no other loss cause (no click elsewhere, no Esc, no dialog, no blur) requests the panel's focus back, with no `keys=off` line. Esc is not locked: it leaves the panel (and is STOP). "Click elsewhere to leave" and Esc are the two ways out (D3).
3. **Focused look (UX-DR37, DESIGN.md › Virtual gamepad › Focused).** Only while the panel has focus, at both widths: an inset 2 pt `amber` ring on the gamepad region's edge plus an inset `amber-wash` wash of 6 stacked inset strokes *(story; DESIGN `gamepad-panel-focused.glow` "5–6 stacked inset strokes", mock `inset 0 0 22px #f0b43c40`; Dev Notes › Focused look)*, painted over the region fill and under the panel content; and the chip "Keys active" (`chip-keys-active`: `amber` fill, `ink` text, `rounded.full`, 11.5 pt weight 700, padding 1 × 8 pt, bottom 3 pt above the tab underline, right-aligned in the header, mock `.ph em`) beside the tab, disjoint from the tab at 200 pt and 300 pt region widths. The chip adds no AccessKit node (the group's name says it, AC 10). No ring and no chip without focus.
4. **Key mapping by physical key (UX-DR37).** Keys are matched by `Event::Key::physical_key` (positional on any layout; an event with `physical_key: None` does nothing): Arrow keys and W A S D → D-pad up/left/down/right (`W`/`ArrowUp` up, `A`/`ArrowLeft` left, `S`/`ArrowDown` down, `D`/`ArrowRight` right); H J K L → Y B X A; Q E → L R; Z C → ZL ZR; Enter → +; Backspace → −. Home, Capture and both sticks have no key (mouse only). The table lives once, in the pure model (`gamepad::PadKey`), and the chips and legend of Story 4.8 already show it (unchanged texts).
5. **Key hold semantics.** Only in the frame's first pass, and only while the panel had focus at the end of the previous first pass and still has it after `main_screen::show` (keys of the frame that gains or loses focus do nothing): a fresh press (`repeat == false` and that physical key not already held) presses its control in the frame it arrives; the key's release releases it, never earlier than **50 ms** after the press (`gamepad::TAP_MIN_US`, the Story 4.8 tap; a quick key tap is a 50 ms tap); OS key auto-repeat (`repeat == true`) presses nothing and writes nothing; any number of keys can be held at once (chords: the state is the union); two keys on the same control (W and ArrowUp) keep it pressed while either is held; opposite directions held together cancel on that axis (`gamepad::hat`, Story 4.8, e.g. ArrowLeft + D → no horizontal direction); key holds and the mouse hold are tracked separately (releasing the mouse never ends a key hold and the reverse) and the sent state is the union of both plus the sticks; nothing is sent while the union does not change. A press with Cmd, Ctrl or macOS Ctrl held (`Modifiers::command || ctrl || mac_cmd`) presses nothing and is left in the input for the app (Cmd/Ctrl combos keep their global meaning); Shift and Alt/Option do not block a press. A release is honoured whatever the modifiers; a release of a key that is not held is ignored. While the panel has focus the mapped key events (press and release, without Cmd/Ctrl) are consumed after they are read, so no later widget sees them.
6. **Release rules extended to keys (Story 4.8 AC 8; Story 4.7 forward note).** Every key hold ends with the mouse hold and the sticks, with its own `pad release … key=<k>` line, when: (a) the window loses focus or is hidden/minimized (also in a `logic`-only frame): one `SET_STATE` with the remaining state (NEUTRAL), and the panel's focus is surrendered (AC 1); (a′) the panel loses focus for any other reason (click elsewhere, a dialog): `release_all(FocusLost)`, one `SET_STATE` NEUTRAL, the pointer press spent if down; (b) STOP (Esc or the STOP button): no `SET_STATE` (STOP releases everything); (c) the gate turns blocked while the pad is active (a key hold counts): STOP (`source=gamepad`); (d) the controller leaves DIRECT by itself: nothing sent. A key still physically held after any of these is not pressed again by its auto-repeat (egui flags it `repeat`); only a fresh press does.
7. **Gated keys (UX-DR38).** While the gate blocks the gamepad and the panel has focus, each fresh mapped key press sends nothing and writes one `pad ignored control=<c> reason=<block> key=<k>` line (repeats and releases write nothing); the inline hint of the reason is shown as today.
8. **Global keys keep their meaning (UX-DR41, UX-DR61; Story 4.7).** Esc = STOP always, read at frame start and never consumed: with the panel focused and keys held, one Esc requests one STOP, releases every hold (`reason=stop`), and (without modifiers) the panel loses focus; with Shift/Alt held the STOP still happens and the panel keeps its focus (egui clears focus only on an unmodified Esc). Space never activates anything and is not a gamepad key (still consumed by `keys::frame_start`). Plain Enter presses + only while the panel has focus; Cmd/Ctrl+Enter never presses + (reserved for Play, Epic 6). Backspace presses − only while the panel has focus. With the panel not focused no key ever reaches the pad.
9. **Compact variant (UX-DR6, DESIGN.md › Compact variant).** When the content rect is narrower than **1280 pt** (the window between 1024 and 1279 pt, logical points), `layout::regions` gives the gamepad region `spacing.gamepad-compact-width` (**200 pt**) and the note view the rest; at 1280 pt and wider it stays 300 pt with the full panel unchanged. The compact panel (10 pt padding, 180 pt inner width), top to bottom: the tab "Gamepad" (and the "Keys active" chip when focused) over the 1 pt underline; the caption wrapped in at most two lines; the button grid (180 × 124 pt; geometry in Dev Notes › Compact geometry: ZL L R ZR row, − Capture Home + row, the D-pad in its well and the ABXY diamond in its well, the same control shapes, sizes, letters, tints and hit tests as the full pad); the inline hint 8 pt under the grid while gated. No silhouette, no sticks, no key chips, no legend, no "mouse" labels (buttons only). Each control shows a tooltip on hover (`theme::tooltip_frame`, egui's hover delay) with its key hint (Copy table, e.g. "A: L key", "Home: mouse only"). Mouse semantics, keys, gating, release rules and lines are the same at both widths. If the layout switches to compact while a stick is dragged, the drag ends at once (`pad stick=<s> end reason=layout`, the stick back to centre); a key or mouse hold of a button survives the switch (control ids are the same at both widths). At 1024 × 680 and 1279 × 800, ppp 1 and 2, everything stays inside the 200 pt region and nothing overlaps.
10. **AccessKit (Story 4.2: basic).** The panel is one `Role::Group` node over the gamepad region, named "Gamepad – keys active" while it has focus and "Gamepad – keys off" otherwise, with the `Focus` action (it is focusable). It is created before any other node of the panel, and every node of the panel (the tab, caption and legend `Label`s, the 16 `Button` and 2 `Slider` controls, the hint) is built in a child `Ui` with `UiBuilder::accessibility_parent(panel_id)`, so all of them are its children (egui otherwise parents a node to the nearest ancestor that already has one). The controls keep the Story 4.8 names and roles, without the `Focus` action; an AccessKit `Click` is still a tap. In the compact variant the group has the 16 `Button` nodes and the tab, caption and hint `Label`s (no `Slider`, no legend).
11. **Tab order.** From nothing focused: Tab → the three LEDs → the panel; there Tab and Shift+Tab stay on the panel (AC 2). Shift+Tab from nothing focused (or from the first LED) reaches STOP, as before; Esc or a click elsewhere leaves the panel. Every test and runbook step that reached STOP with "Tab four times" now uses Shift+Tab once (from nothing focused) or an AccessKit click (Dev Notes › Regressions to expect).
12. **Log lines.** The app writes the new and extended `pad` lines (`press … source=keyboard key=<k>`, `release … reason=<key_up|tap|focus_lost|window_blur|stop|gate|controller_idle> key=<k>`, `stick=<s> end reason=layout`, `ignored … key=<k>`, `keys=on|off`) in the exact formats of Dev Notes › Log lines through the existing `LogHandle` (non-blocking); a `press` line is written before the `try_send` of the state it causes; no line while nothing happens (100 idle frames with the panel focused and no key → 0 `pad` lines). The Story 4.8 mouse lines are unchanged.
13. **No regressions, CI green.** `cargo fmt --all --check`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, `cargo test --locked --workspace`, `cargo deny --locked check licenses bans`, the G.8 scripts, the smoke run (`worker=` ≥ 1) and "ocb-sim never ships" stay green on ubuntu, windows and macos. G.1, G.3, G.10–G.23, C.23–C.29 keep passing, changed only where this story changes a number or an order (the 1024 pt layout, the Tab order, a press taking panel focus, the new nodes), each change with its reason in the Debug Log. Every new check has a red proof in the Debug Log.
14. **Docs, bench, eye check, M5.** `EXPERIENCE.md`, `DESIGN.md`, `docs/test-plan.md` (G.24, G.25, the changed rows, H.36 and its rig block, the M5 keyboard line), `epics.md` (Story 4.9 bench line → H.36), `_bmad-output/specs/spec-bonooru/roadmap.md` (M5 Done-when cell gets the macOS evidence; **M5 stays TODO**, D16) and `app/README.md` updated as in Task 10. H.36 runs on macOS with the dry run green on the bench commit (owner present, OoT running); Windows/Linux go to issue #43. The owner eye check of the focused look and the compact variant is recorded.

### AC → measurement (retro action E3-A1)

Host rows run in `cargo test`; fake clocks for every time number (worker `FakeClock`, UI `TestClock` through `App::ui_clock`); waits on worker threads are bounded polls (≤ 2 s) as in G.23 (the starvation-safe `Rig::frame`). Key events in tests are pushed as real winit would send them: `Event::Key { key, physical_key: Some(..), pressed, repeat: false, modifiers }` (kittest's `key_press` sets `physical_key: None`, which this story ignores by design; egui itself sets `repeat` from its `keys_down`, so a second press event of a held key arrives flagged). The bench rows (H.36) read `bonooru.log` and what the owner sees and hears on the Switch.

| AC | Number / claim | Measured by | Field / result |
|---|---|---|---|
| 1 | press on a control, on a stick or on empty panel space → focus, same-frame press; focus kept through press/hold/release; click elsewhere, unmodified Esc, a dialog, blur → focus lost; one line each | G.25 `g25_focus_gain_and_loss` (app rig) | `memory().focused() == Some(panel_id())` after the press frame and after the release; `pad press control=a source=mouse` in the press frame; lines `keys=on source=click`, `keys=off reason=elsewhere` / `esc` / `dialog` (firmware dialog via a foreign board, as G.18) / `window_blur` (`Event::WindowFocused(false)`; and a `logic`-only frame with `visible() == Some(false)` via `App::pad_background`) |
| 1 | Tab-in and AccessKit focus | G.25 `g25_focus_gain_and_loss` | Tab × 4 from nothing → panel, `keys=on source=tab`; AccessKit `Focus` on the group (`Node::focus()` / an `ActionRequest`) → `keys=on source=other` |
| 1 | press through a dialog does not focus the panel | G.25 `g25_dialog_blocks_focus` | firmware dialog open, a press at A's centre → focus stays in the dialog, no `pad` line |
| 2 | lock holds from the first frame | G.25 `g25_focus_lock` | after Tab-in and after a click: Tab, Shift+Tab, each arrow in the very next frame and 10 frames later → focus still the panel, no `keys=off` line; arrows press the D-pad (`pad sent … hat=`); Esc → focus `None`, `keys=off reason=esc` |
| 3 | ring, wash, chip only while focused; chip disjoint from the tab | G.24 `g24_focused_look` (kittest, `main_screen::show` with the panel focused / not, 1280 × 800 and 1024 × 680, ppp 1 and 2) | focused: one inside-stroked rect on the region of width `max(2, 1 px)` `AMBER`; six inside strokes in `AMBER_WASH`-based colours (Dev Notes › Focused look) inside the region; one filled rounded rect in `AMBER` with a galley "Keys active" in `INK` 11.5 pt weight 700; chip rect ⊂ region, chip ∩ tab = ∅, chip bottom = underline − 3 ± 0.5; unfocused: none of these shapes |
| 4 | the mapping table, physical key only | unit `gamepad` tests `key_table` and G.25 `g25_every_key_bit` | each of the 18 `PadKey`s → the Story 4.8 bit or hat (Dev Notes › Key table); in the app each key press (physical Some) → one `SetState` with exactly that bit/hat, then NEUTRAL ≥ 50 ms later; the same events with `physical_key: None` → nothing; Home/Capture/sticks have no key (table has 18 rows, none maps them) |
| 5 | fresh press in the press frame; release ≥ 50 ms; repeat ignored | G.25 `g25_key_tap_and_hold`, `g25_key_repeat_ignored` | press + release in one frame → [A, NEUTRAL], NEUTRAL at UI time ≥ press + 50 000 µs; 3 s hold with a repeat event every 33 ms → [A, NEUTRAL] only, 1 `pad press`, 0 other `pad` lines between press and release |
| 5 | chords, duplicates, opposites, union with the mouse | G.25 `g25_chords_and_union` and unit tests | L+K → `buttons=0x000c`; K up → `0x0004`; W + ArrowUp → one `hat=0`, W up → no send, ArrowUp up → `hat=8`; ArrowLeft + D → `hat=8`, ArrowLeft up → `hat=2`; mouse holds A and key L pressed+released → A stays until the pointer is up; key L held and mouse clicks B → `0x0006` then `0x0004` |
| 5 | Cmd/Ctrl presses left alone; releases always honoured; unknown releases ignored; mapped events consumed | G.25 `g25_modifiers_and_consumption` | Cmd+L, Ctrl+L, Cmd+Enter → no `SetState`, no `pad` line, the event still in `ctx.input().events` after the pad read; L held then Cmd pressed and L released → NEUTRAL sent; a release of K never pressed → nothing; with the panel focused a plain L event is gone from the events after the pad read |
| 5 | keys only with focus at frame start and end | G.25 `g25_focus_frames` | a key in the frame of the click that focuses → nothing; a key in the Esc frame → nothing; keys with the panel unfocused → nothing, no line |
| 6a | blur and hidden window release keys and drop focus | G.25 `g25_blur_releases_keys` | L held, `WindowFocused(false)` → `pad release control=a reason=window_blur key=l`, one NEUTRAL `SetState`, `keys=off reason=window_blur`, focus `None`; same through `pad_background(true, false, false)` with no `ui` call; focus back and an L repeat event → nothing |
| 6a′ | focus loss releases | G.25 `g25_focus_loss_releases` | L held, a click on the note view → `keys=off reason=elsewhere`, `release … reason=focus_lost key=l`, one NEUTRAL `SetState`; L held, the firmware dialog opens → `keys=off reason=dialog` and the same release (the gate also turns blocked: rule 8c STOP; record which line comes first). **Decided 2026-11-28 (D-2, session lead):** the release order is measured with a generic dialog (L held; also opened in the same frame as an Enter, and with a disabled primary, `g25_dialog_takes_the_keys`); the firmware dialog only for its `keys=off reason=dialog`, because the gate is already blocked when the firmware dialog opens (it opens on a new connection), so nothing can be held then |
| 6b | STOP releases keys, nothing resent | G.25 `g25_stop_releases_keys` | L held, Esc → `stop requested source=esc`, `release … reason=stop key=l`, `keys=off reason=esc`, no `SetState` after; L held, AccessKit click on STOP → `source=button`, same release, focus as egui leaves it; Shift+Esc with L held → STOP, release, focus still the panel, the next L repeat → nothing, a fresh L → `SetState(A)` |
| 6c, 6d | gate block and controller idle with keys | G.25 `g25_block_and_idle_with_keys` | L held, `console_ready` false → `release … reason=gate key=l`, `stop requested source=gamepad`; L held, `force_abort(None)` → `release … reason=controller_idle key=l`, nothing sent |
| 7 | gated keys | G.25 `g25_gated_keys` (each block reason as G.23 `g23_blocked_clicks_send_nothing`) | a fresh L → `pad ignored control=a reason=<r> key=l`, controller `log().direct` unchanged; repeats and releases → no line |
| 8 | Space, Enter, Backspace only as specified | G.25 `g25_global_keys` | panel focused: Space → no `pad`/`stop` line; Enter → `SetState` Plus; Backspace → Minus; Cmd+Enter → nothing; panel unfocused (STOP focused via Shift+Tab): Enter → one STOP, no `pad` line |
| 9 | 200 pt region below 1280, grid geometry, inside and disjoint | G.1 `g1_sizes_match_the_tokens` (updated), G.24 `g24_compact_layout` (pure `ui::gamepad::compact_layout`), `g24_compact_geometry`, `g24_compact_drawn_inside` (kittest 1024 × 680, 1279 × 800, ppp 1 and 2, open and each block reason, focused and not) | gamepad width 200 for content width < 1280, 300 at ≥ 1280, note view takes the rest, regions still tile; every control at Dev Notes › Compact geometry + grid origin ± 0.5; every shape and text ink ⊂ region; tab / chip / caption / grid / hint pairwise disjoint; hint top = grid bottom + 8 ± 0.5; caption ≤ 2 lines inside its 34 pt box |
| 9 | no silhouette, sticks, chips, legend, "mouse" in compact; tooltips | G.24 `g24_compact_contents`, `g24_compact_tooltips` | no body outline path, no `WELL` stick rings, no chip galleys, no legend `Label`s, no "mouse" text; hovering A past the delay → a tooltip with text "A: L key" in the tooltip frame; one tooltip per control with the Copy table text |
| 9 | stick drag ends on the switch to compact; button holds survive | G.25 `g25_layout_switch` | drag the left stick at 1280, resize to 1100 → `pad stick=left end reason=layout`, one `SetState` with axes 128; L held at 1280, resize to 1100 → still held, no line |
| 10 | group node, name, children | G.24 `g24_accesskit_group` (both widths) and G.10 (updated) | one `Role::Group` node named "Gamepad – keys off" (unfocused) / "Gamepad – keys active" (focused) with the `Focus` action; the 16 `Button`, 2 `Slider` (full) and the panel `Label`s have it as parent (AccessKit tree walk); compact: 16 `Button`, 0 `Slider`; no control has `Focus` |
| 11 | Tab order | G.20 `g20_tab_order` (updated) | Tab × 4 → LED × 3 → panel; Tab 5 → still panel; Esc → `None`; Shift+Tab → STOP |
| 12 | line formats, order, quiet when idle | G.25 (`line_matches`) | each new line matches its pattern; the `press … source=keyboard` line precedes the worker's `pad sent` for it; 100 idle frames with focus → 0 `pad` lines |
| 13 | CI | CI `app` workflow, three OS | green |
| 14 | keyboard on the rig reaches the Switch once per press; tap length; latency | **H.36** steps 2, 4 (instrument: `bonooru.log` `pad`/`mode` lines + owner) | `pad press control=<c> source=keyboard key=<k>` → `pad sent` with its bit, `+ms(pad sent) − +ms(pad press)` ≤ 20 *(story; as H.35)*; release `reason=tap` → `+ms(sent release) − +ms(sent press)` in [49, 70]; `reason=key_up` → ≥ 49 and `+ms(sent release) − +ms(pad release)` ≤ 20; owner: the game reacts once per key press |
| 14 | a 3 s held key with macOS key repeat stays one press | **H.36** step 5 | between `pad press control=a source=keyboard key=l` and `pad release control=a reason=key_up key=l` (2 500–4 000 ms apart): no other `pad press`, `pad sent`, `pad ignored`, `mode … to=idle`, `nak`, `stop` or `reboot` line; owner: one continuous note |
| 14 | chords, duplicate keys, opposite keys on the wire | **H.36** steps 6, 7 | the `pad sent` sequences of AC 5's row (0x0004 → 0x000c → 0x0004 → 0x0000; hat 0 once, then 8 only after both keys up; hat 6 → 8 → 2 → 8) |
| 14 | focus loss, Esc, blur release held keys | **H.36** steps 8–10 | the lines of AC 6's rows; no `pad sent` after a STOP until the next fresh press; after each, a key press writes no `pad` line until the panel is clicked; owner: the note stops at the key/click/Cmd+Tab |
| 14 | lock, Space, Cmd+Enter on the rig | **H.36** step 11 | owner: the ring stays on the panel through Tab × 3 and Shift+Tab; no `keys=off`; Space and Cmd+Enter → no `pad`/`stop` line; Enter → `buttons=0x0200`, Backspace → `0x0100` |
| 14 | gated keys on the rig | **H.36** step 12 | `gate … gamepad_reason=no_bridge` (a short `bridge_not_ready` first is allowed), L → `pad ignored control=a reason=no_bridge key=l`, no `pad sent` within 1 s |
| 14 | compact on the rig | **H.36** step 13 | owner: below 1280 the compact grid shows, the tooltip on A says "A: L key"; a click on A and a press of L each give `pad sent buttons=0x0004` then NEUTRAL |
| 14 | M5 keyboard exit | **H.36** steps 2–4 | free play reached and notes played with the keyboard only (ArrowLeft takes the ocarina out; L, K, H, Q, E play), Joy-Cons untouched |
| 14 | expected bench lines before the bench | G.25 `g25_h36_dry_run` (`cargo test -p ocb-app -- --nocapture h36_dry_run`) | the exact H.36 line sequence per step, green on the bench commit |

Numbers no tool measures (Open questions 18): **the time from `pad sent` to the game's reaction** (owner's perception only, as H.35); **H.36 on Windows and Linux** (no machine; issue #43; winit's key repeat and IME paths differ there); **IME input sources and non-US layouts on the rig** (the bench uses the macOS ABC/US layout; positional mapping is host-tested only through `physical_key`); **keyboard rollover limits** (how many keys a given keyboard reports at once is hardware; chords of more than 2–3 keys are not benched).

## Tasks / Subtasks

- [x] **Task 1: keys in the pad model, pure (AC 4–7, 12).** `app/gui/src/gamepad.rs` (still no egui import; G.22 `g22_model_has_no_egui`).
  - [x] 1.1 `PadKey` enum (18: `ArrowUp, ArrowDown, ArrowLeft, ArrowRight, W, A, S, D, H, J, K, L, Q, E, Z, C, Enter, Backspace`) with `ALL`, `control()` (Dev Notes › Key table) and `log_name()` (`arrow_up arrow_down arrow_left arrow_right w a s d h j k l q e z c enter backspace`). `Control::key_hint()` for the compact tooltips (Copy table) built from the same table, so the two never disagree; a unit test checks every `PadKey` appears in exactly one hint and Home/Capture say "mouse only".
  - [x] 1.2 Key holds beside the mouse hold: a small fixed array or `Vec<KeyHold { key, since_us, release_at }>` (no allocation per frame beyond the lines). `key_down(key, now_us)`: ignored if that key is held (no line); else a hold + `press_line(c, Source::Keyboard, Some(key))`. `key_up(key, now_us)`: ignored if not held; else releases at `max(now, since + TAP_MIN_US)` (`reason=key_up`, or `tap` when it had to wait). `tick` handles key deadlines too; `next_wake_us` includes them.
  - [x] 1.3 `state()` becomes the union: buttons OR-ed from the mouse hold and every key hold; the hat from `hat(any up, any down, any left, any right)` over both (replace the single-control `apply` for the hat); sticks as today. `is_active()` counts key holds. `free()` stays about the mouse hold and the drag only (a mouse press while keys are held is not `busy`).
  - [x] 1.4 `Release` gains `KeyUp` (`key_up`), `FocusLost` (`focus_lost`, sends the new state like `WindowBlur`) and `Layout` (`layout`, sticks only). `release_all(reason)` ends every key hold with a `release … key=<k>` line, then the mouse hold and the drag as today; `neutral_without_send` unchanged for `Stop | Gate | ControllerIdle`. `end_drag_for_layout()` → `stick end reason=layout`.
  - [x] 1.5 `Source::Keyboard` (`keyboard`); `press_line`/`release_line`/`ignored_line` take an optional key (Dev Notes › Log lines); `keys_on_line(source)` and `keys_off_line(reason)`; `ignored_key(key, block)` writes the gated line.
  - [x] 1.6 Unit tests: `key_table`, tap/hold/repeat-free semantics, chords, duplicate keys, opposite keys, union with the mouse (both directions), `release_all` for each reason with keys held (lines and `last_sent`), rule 8d with only a key hold, `next_wake_us` with several key deadlines.
- [x] **Task 2: tokens (AC 3, 9).** Tokens first (single source, G.3).
  - [x] 2.1 `DESIGN.md` frontmatter: `typography.chip` (Nunito Sans, 11.5px, 700, no tracking; mock `.ph em`); `components.chip-keys-active` gains `typography: '{typography.chip}'`, `padding-x: 8px`, `padding-y: 1px`, `gap-bottom: 3px`; `components.gamepad-panel-focused` gains `ring-width: 2px`, `wash-steps: 6`, `wash-step: 3px`; `components.gamepad-compact` (`grid-width: 180px`, `grid-height: 124px`, `caption-lines: 2`); `spacing.gamepad-compact-width` loses its `[ASSUMPTION]` mark (owner can overrule, Open questions 9). `theme.rs`: `CHIP` in `typography_tokens()`, `components::chip_keys_active::{…}`, `components::gamepad_panel_focused::{…}`, `components::gamepad_compact::{…}`; extend `tests/design_tokens.rs` (G.3) and `g3`'s DESIGN parse for the new keys.
- [x] **Task 3: compact layout (AC 9).** `app/gui/src/ui/layout.rs`, `app/gui/src/ui/gamepad.rs`.
  - [x] 3.1 `layout::regions(content)`: gamepad width `GAMEPAD_COMPACT_WIDTH` when `content.width() < COMPACT_BELOW` (`pub const COMPACT_BELOW: f32 = 1280.0`, doc: UX-DR6), else `GAMEPAD_PANEL_WIDTH`; replace the "always 300" comment; `pub fn is_compact(content: Rect) -> bool` used by both `regions` and the panel so they never disagree. Update the G.1 unit tests in `layout.rs` (sizes per width, tiling, reading order unchanged).
  - [x] 3.2 `ui::gamepad`: `pub enum Variant { Full, Compact }` chosen from the region width (200 → compact); `pub fn compact_layout(region, text) -> CompactLayout { tab, chip, underline_y, underline_x, caption, grid, hint }` (pure, as `layout`); `PadGeometry::compact(origin)` with the Dev Notes › Compact geometry (same `ControlShape`s, no wells for sticks, no chips, no "mouse" anchors; `hit()` returns controls only); `PadGeometry::stick_wells()` empty in compact.
  - [x] 3.3 `show` draws the compact panel: header, caption wrapped to 180 pt (fixed 2 × `CAPTION_HEIGHT` box, G.1 ppp rule), the grid (wells for the D-pad and ABXY, controls with letters/arrows/house/dot, tints as Story 4.8), the hint at `compact_layout.hint`; no body, sticks, chips, legend. Each control in compact gets a tooltip (`Tooltip::for_enabled(&resp)` with `theme::tooltip_frame()`, text `Control::key_hint()`), on hover only (controls are not focusable).
  - [x] 3.4 `PadInput` gains `layout_switched_with_drag: bool` or the app compares `view.dragging` with the variant: if a stick is dragged and the variant is compact, the app calls `pad.end_drag_for_layout()` (AC 9).
- [x] **Task 4: panel focus, lock, look and the group node (AC 1–3, 10, 11).** `app/gui/src/ui/gamepad.rs`, `app/gui/src/ui/main_screen.rs`.
  - [x] 4.1 `pub fn panel_id() -> Id` (e.g. `Id::new("gamepad-panel-focus")`). At the start of `show`: `let panel = ui.interact(region, panel_id(), Sense::FOCUSABLE)`; `panel.widget_info(|| WidgetInfo::labeled(Role::Group, true, name))` with the name of AC 10 (from `panel.has_focus()`); this creates the group node first.
  - [x] 4.2 Everything else of the panel (header labels, caption, controls, sticks, legend, hint) goes into `ui.scope_builder(UiBuilder::new().max_rect(region).accessibility_parent(panel_id()), …)`. Check with the AccessKit tree in G.24 that each node's parent is the group (not the region's `Ui`).
  - [x] 4.3 The focused look (Dev Notes › Focused look): painted right after the region fill and before the content when `panel.has_focus()`; the chip in the header (full and compact) through `paint::layout_line` with `theme::CHIP`, `INK` on an `AMBER` rounded rect; no node.
  - [x] 4.4 `PadInput` gains `panel_press: bool` (a primary press or release/click this frame landed on the panel: `panel.hovered()` or any control/stick response `is_pointer_button_down_on() || clicked() || contains_pointer()`, as Story 4.8's "press reached the pad"; never through a dialog) and `panel_focused: bool` (`panel.has_focus()` after `show`).
  - [x] 4.5 `main_screen::show` passes the variant through unchanged region order (status → song → note view → gamepad → bottom); `MainOutput` keeps `pad: PadInput`.
- [x] **Task 5: keys from egui (AC 4, 5, 8).** New `app/gui/src/ui/pad_keys.rs` (`pub mod pad_keys;` in `ui/mod.rs`).
  - [x] 5.1 `pub fn pad_key(physical: egui::Key) -> Option<PadKey>` (the only egui ↔ model mapping; arrows, W A S D, H J K L, Q E, Z C, Enter, Backspace).
  - [x] 5.2 `pub fn take(ctx: &Context) -> Vec<KeyEdge>` where `KeyEdge { key: PadKey, pressed: bool, repeat: bool, command: bool }`: reads `Event::Key` in order, maps by `physical_key` only (None → skip), and removes from `ctx.input_mut(|i| i.events)` the mapped events without Cmd/Ctrl (consumption, AC 5); events with Cmd/Ctrl stay. Called by the app only when keys are taken this frame (AC 5 focus rule), first pass only. `keys::command_held` becomes `pub(crate)` and is reused (one definition of "Cmd/Ctrl held").
- [x] **Task 6: app integration (AC 1, 2, 5–8, 12).** `app/gui/src/app.rs`.
  - [x] 6.1 New fields: `panel_focused: bool` (end of the previous first pass), `focus_lost_pending: Option<&'static str>` if needed. In `pad_background` (from `logic`): on blur also `ctx.memory_mut(|m| m.surrender_focus(panel_id()))` and, if the panel had focus, `keys=off reason=window_blur` (pass `ctx` or return a flag; keep it testable without `ui`). `release_all(WindowBlur)` already ends key holds (Task 1.4).
  - [x] 6.2 `pad_input` order (first pass, after STOP was requested for the frame, as Story 4.8): (1) STOP this frame → `release_all(Stop)` (keys included), spend the frame's press; (2) blur frame → spend; (3) focus: `now = out.pad.panel_focused`; if the panel had focus and lost it without STOP or blur: if a Tab/arrow key event is in this frame and no click elsewhere/Esc/dialog → `request_focus` (lock fallback, AC 2), no line; else `keys=off reason=<esc|dialog|elsewhere>` and `release_all(FocusLost)` (skip the release if STOP already released); if a press/release landed on the panel (`panel_press`) and the panel is not focused (or was surrendered by the click) → `request_focus(panel_id())`, and on a gain `keys=on source=click`; a gain without a press → `source=tab` if a Tab event is in the frame, else `other`; (4) gate and pointer as Story 4.8; (5) keys: only if the panel had focus at the end of the previous first pass and has it now, and the frame is not a STOP or blur frame: `pad_keys::take(ctx)` → for each edge in order: `repeat` → skip; press with `command` → skip (left in the input by `take`); gate blocked → `pad ignored … key=` per press; else `key_down`/`key_up`; (6) set the lock filter (`ctx.memory_mut(|m| m.set_focus_lock_filter(panel_id(), filter))`) last, if focused; (7) the layout-switch drag end; then `pad_flush` as today. Store `panel_focused` for the next frame. Request one repaint after a focus gain (so the lock is set in the next frame without waiting for input).
  - [x] 6.3 Esc frame: keep Story 4.7's order (`stop requested` first, then every `pad` line of the frame); the `keys=off reason=esc` line comes after the `release … reason=stop` lines.
  - [x] 6.4 Test hooks: `App::panel_focused() -> bool` (read only). Never `App::new` in tests (G.13).
- [x] **Task 7: tests (AC 1–13).**
  - [x] 7.1 `app/gui/tests/gamepad_focus.rs` (G.24): `g24_focused_look`, `g24_compact_layout`, `g24_compact_geometry`, `g24_compact_drawn_inside`, `g24_compact_contents`, `g24_compact_tooltips`, `g24_accesskit_group`, `g24_no_online_wording` (chip text, group names, every tooltip string through `leds::BANNED_WORDS`), `g24_pad_keys_table` (`pad_keys::pad_key` for every egui key in the table and `None` for a sample of others, e.g. `Space`, `Tab`, `Escape`, `X`, `Num1`).
  - [x] 7.2 `app/gui/tests/gamepad_keys_app.rs` (G.25): every G.25 row of the AC table plus `g25_h36_dry_run`. Rig as G.23 (copy or share its `Rig` with the starvation-safe `frame`, `fake_worker_logged`, `ocb-sim` controller, `TestClock`, bounded polls); a helper `key(h, Key, pressed, modifiers)` that pushes `Event::Key { key, physical_key: Some(key), pressed, repeat: false, modifiers }` (egui fills `repeat`); focus the panel by a click on empty panel space (Dev Notes › Compact geometry gives free spots; for the full pad e.g. the region's top-right corner under the header) or by Tab × 4.
  - [x] 7.3 Update the existing checks named in Dev Notes › Regressions to expect, each change with its reason in the Debug Log.
- [x] **Task 8: red proofs (AC 13, E3-A2).** For each new check the Debug Log names the code change that makes it fail and the failure seen (minimum list: Dev Notes › Red proofs). No bench tool is new or changed (`ocb-probe`, `ocb-dev`, `link_probe.py` untouched); the H.36 instrument is the `pad`, `mode`, `stop`, `gate` and `led` lines plus the owner's observation, and each H.36 verdict has a host test showing the failing value too (Dev Notes › Instrument review); say so in the Debug Log.
- [x] **Task 9: instrument review and dry run before the bench (E3-A2, E3-A3).** Written in the Debug Log before any bench result: every H.36 field maps to a line the code writes; the expected lines per step from `cargo test -p ocb-app -- --nocapture h36_dry_run` on the bench commit pasted into the Debug Log; the rig block's checklist checked against the runbook; the gotchas re-read; `g23_h35_dry_run` still green (the H.35 runbook's step 7 is now historical: see Regressions to expect).
- [x] **Task 10: docs (AC 14).**
  - [x] 10.1 `EXPERIENCE.md`: Interaction Primitives › Gamepad focus (drop the two `[ASSUMPTION]` marks; Story 4.9 behaviour: gained by press/Tab/AccessKit, lost by click elsewhere, Esc, a dialog, window blur; the lock and its first-frame fallback; keys only with focus at frame start); › Virtual gamepad mapping (fresh press, 50 ms minimum, repeat ignored, chords, duplicates, opposites cancel, union with the mouse, Cmd/Ctrl left alone); Component Patterns › Virtual gamepad (release rules a′ and keys; accepted limits from Dev Notes › Accepted limits; "The keyboard, panel focus and the compact variant are Story 4.9's" → the Epic 4 text); Tab row: from nothing focused Tab reaches LEDs then the gamepad, which keeps Tab; Shift+Tab reaches STOP; Responsive table (1024–1279: compact, buttons only, sticks not shown, key hints in tooltips); Spine-only surfaces (compact gamepad now specified in DESIGN.md); Accessibility Floor (the `Group` node, its names, children; keyboard trap note: Esc or a click leaves, D3); Voice table: "Keys active", the group names, the tooltip strings.
  - [x] 10.2 `DESIGN.md`: the Task 2.1 tokens; Virtual gamepad › Focused (6 stacked inset strokes, Dev Notes › Focused look; chip geometry); › Compact variant (the grid of Dev Notes › Compact geometry; no sticks; tooltips; caption wraps); remove "Spine-only: no mock" or say "specified here, no mock".
  - [x] 10.3 `epics.md`: Story 4.9 gets a bench line "Bench: keyboard on the real rig with OoT running (held keys, key repeat, chords, focus loss, Esc, blur, compact) → H.36 (Story 4.9)".
  - [x] 10.4 `docs/test-plan.md`: G.24, G.25 (App GUI tests; intro "G.24 and G.25 come with Story 4.9"); the changed rows (G.1, G.10, G.20, G.22, G.23: one clause each with "(Story 4.9)"); H.36 (Hardware-in-the-loop, Result empty until the bench); the rig block "Gamepad keyboard bench rig (Story 4.9)" (Dev Notes › Rig block) after the Gamepad bench rig; the M5 keyboard exit line; a note under H.35 that its step 7 (Tab × 4 to STOP while the mouse holds A) no longer applies from Story 4.9 (a press on the pad takes panel focus; use H.36 step 10's Esc/AccessKit path).
  - [x] 10.5 `_bmad-output/specs/spec-bonooru/roadmap.md` M5: the Done-when cell gets the macOS evidence (H.33–H.36 PASS dates and commits; M5 gamepad exit met with mouse 2026-11-28 and with keyboard on the H.36 date) and "M5 is DONE only when the deferred Windows/Linux rows (H.28–H.36) pass too, tracked as GitHub issue #43 (retro action E3-A6, as M4)". State stays TODO (D16).
  - [x] 10.6 `app/README.md`: module line for `ui::pad_keys`; the keyboard `pad` lines and `keys=on|off`; the compact variant below 1280 pt.
- [x] **Task 11: verify on the host (AC 13).** In `app/` with `/opt/homebrew/opt/rustup/bin` first on `PATH`: fmt, clippy, `cargo test --locked --workspace` (twice), deny, G.8 scripts, `cargo run -p ocb-app -- --smoke-test` and `-- --smoke-test --gallery` (checker green, `worker=` ≥ 1; boards unplugged), `cargo tree -e normal -i ocb-sim` empty for ocb-app, ocb-services, ocb-core. Push the branch; the `app` workflow green on three OS.
- [x] **Task 12 (OWNER + agent): H.36 on macOS** (Dev Notes › H.36, › Rig block, › H.36 runbook). Record the results in `docs/test-plan.md` (H.36 Result, rig block table, M5 keyboard line), `roadmap.md` (M5 evidence, state TODO) and the Debug Log. Windows/Linux: "deferred: owner has no Windows/Linux machine yet; issue #43" (comment text for the coordinator to post on #43 in the Debug Log).
- [x] **Task 13 (OWNER, late): eye check** (Dev Notes › Eye check). Record each answer; make and re-check any change before review.

## Dev Notes

### Scope in one paragraph

Story 4.8 left the pad model, the drawing, the worker command, the gate and the release rules done for the mouse; the keyboard chips and the legend already describe the keys but nothing reads them, and the panel is 300 pt at every width. This story makes the panel one focusable widget (click, Tab or AccessKit), locks Tab and the arrows inside it while it has focus, draws the amber ring, wash and "Keys active" chip, turns the 18 mapped physical keys into holds that join the mouse hold in the one canonical state the pad already sends, extends every release rule to the key holds and adds one (panel focus lost), switches to a 200 pt button grid below 1280 pt with key hints in tooltips, and makes the panel a `Role::Group` AccessKit node parent of every panel node. Nothing changes below the UI: the worker, the session and the firmware already take any `SET_STATE` chord (DIRECT is unmasked, AD-9).

### Decisions taken at story creation (change only with a reason in the Debug Log; the owner can overrule any)

- **D1. Physical keys only.** `physical_key` is what UX-DR37 asks (WASD/HJKL positional on AZERTY etc.). egui-winit sets it from winit's `PhysicalKey::Code` (`egui-winit-0.37.0/src/lib.rs:1107`); `None` (a key winit cannot name) does nothing. No fallback to the logical `key`: on non-Latin layouts egui-winit already falls back the other way for `key`, so the logical key would map a different position.
- **D2. A key tap is a 50 ms tap.** Same `TAP_MIN_US` as the mouse (Story 4.8 D2: U7 33.3 ms + one 60 Hz frame). The press goes out at once; only an early release waits.
- **D3. Tab-in with a lock, as the epic AC says; Esc and a click are the ways out.** The AC asks for both "tabs into it" and "a focus-lock filter keeps arrows and Tab inside the panel". The result is a keyboard trap with two documented exits: Esc (egui drops focus on an unmodified Esc; it is also STOP, which is harmless when nothing plays, Story 4.7) and a click elsewhere. The group's AccessKit name says whether keys are active; EXPERIENCE.md records the exits. STOP stays reachable by Shift+Tab from nothing focused. Open questions 3.
- **D4. Window blur drops panel focus.** After Cmd+Tab back, the keys do nothing until the user clicks the panel again (the ring and chip are gone, so the look tells the truth). This also avoids the one case where a key still held across the focus change would look fresh: egui clears `keys_down` on `WindowFocused(false)` (`egui-0.37.0/src/input_state/mod.rs` ~l.444–450), so an OS repeat after refocus would arrive with `repeat == false`. Open questions 4.
- **D5. Focus loss = `SET_STATE` NEUTRAL (like blur), not STOP.** The gate is open then and NEUTRAL is accepted; if the controller would refuse it, Story 4.8's worker rule turns it into STOP (`pad dropped … action=stop`).
- **D6. Keys are taken only with focus at both ends of the frame's input.** The panel must have had focus at the end of the previous first pass and still have it after `main_screen::show`. So the Tab or click that focuses it, and the Esc or click that unfocuses it, never press anything in their own frame.
- **D7. Cmd/Ctrl presses are not pad presses; releases always count.** The same "command held" test as `keys::frame_start` (`m.command || m.ctrl || m.mac_cmd`); events with Cmd/Ctrl stay in the input for later global shortcuts. Shift and Alt/Option do not block (Option+letter has no app meaning; Shift+Tab is locked anyway). Note: on Windows/Linux egui-winit turns Ctrl+C into `Event::Copy` and pushes no key press, but the C release still arrives: the "release of a key not held is ignored" rule covers it.
- **D8. Consume the mapped events while focused.** After the pad reads them, mapped key events without Cmd/Ctrl are removed from `InputState::events` so no later widget (Epic 5's note view Backspace, Epic 6's buttons) ever sees them while the gamepad has focus. Esc is never mapped and never consumed here (Story 4.7). Space is already consumed by `keys::frame_start`.
- **D9. Compact = buttons only, no sticks.** UX-DR6 and DESIGN.md say "buttons only"; the sticks are mouse only anyway and reaching free play needs no stick (D-pad left takes the ocarina out, U9). A drag in progress ends on the switch (`reason=layout`). Open questions 7.
- **D10. One group node, everything inside it.** The group is created first (`ui.interact` + `widget_info` at the start of `show`), and the panel's content `Ui` names it with `accessibility_parent`, so the tab, caption, controls, legend and hint are its children (Story 4.2 review finding, `egui-0.37.0/src/ui_builder.rs:204`).
- **D11. Re-request focus on every panel press and release.** The panel's response lies under the controls, so egui's `SurrenderFocusOn::Clicks` would drop its focus at the click on a control (`context.rs:1844-1853`: the focused widget is surrendered when it is not hovered at a click, and an interactive widget above it stops the hover search, `interaction.rs:257-300`). The app requests the panel's focus again after `show` in every frame a press or release lands on the panel. The lock filter is set after that request (a `request_focus` creates a new `FocusWidget` with the default filter, `memory/mod.rs:923-926`).
- **D12. Lock fallback for the first frame.** `set_focus_lock_filter` only works when the panel had focus last frame (`memory/mod.rs:908-916`), and `begin_pass` applies the previous frame's filter (`memory/mod.rs` ~l.571–606). A Tab or arrow in the frame right after a click or Tab-in would therefore move focus away. The app pulls focus back in that case (AC 2) and asks for one repaint after every focus gain so the filter is normally set before the next key arrives.
- **D13. Key hints in compact tooltips are hover-only.** The controls are not focusable, so their tooltips cannot show on focus; the hints repeat the chips and legend of the full layout, which are information, not actions (EXPERIENCE's "every tooltip-only fix is also shown on focus" is about fixes). Open questions 8.
- **D14. The "Keys active" chip and focus look show whatever the gate says.** While gated the keys are ignored with a hint and a `pad ignored` line, like clicks; the chip still tells the user the keyboard goes to the pad.
- **D15. A short bench is needed: H.36, about 25 minutes, OoT running.** See Bench decision.
- **D16. M5 stays TODO after this story.** M5's Done-when says "the GUI app starts on Windows, Linux and macOS … and its on-screen gamepad drives the console (mouse + keyboard)". With H.36 the macOS part is complete (H.33 LEDs, H.34 STOP, H.35 mouse, H.36 keyboard), but Windows and Linux (H.28–H.36 rows) are deferred to issue #43. The E3-A6 rule that keeps M4 TODO until the Windows/Linux rows pass applies to M5 in the same way: record the macOS evidence in the M5 cell, keep the state TODO, close Epic 4 with M5 open on #43. Open questions 17.

### Bench decision (E3-A1 / E3-A3)

**A bench is needed (macOS, owner present, OoT 1.1.1 running on the Switch 2, about 25 minutes): H.36.** Host tests cover the logic on fake ports and the `ocb-sim` controller (G.24, G.25), but only the rig shows: real macOS key events (winit's physical keys, the OS auto-repeat after ~0.5 s at the system rate, modifier handling, Cmd+Tab and Cmd+H) becoming the right SET_STATE frames; the game reacting once per key press, a held key sustaining one note through the auto-repeat, chords and duplicate/opposite keys arriving as the expected wire states; a held key released by Esc, by a click elsewhere and by blur; Tab staying on the panel; and the compact layout at a real window size. The M5 keyboard exit (free play reached and notes played with the keyboard only) is part of it.

**Audible instrument.** As H.35: in free play a held A sustains one note (U1, up to 6.5 s), so "held through key repeat", "released at Esc / click / Cmd+Tab" are heard at once; the log gives the objective side. The note buttons are A, X, Y, L, R (keys L, K, H, Q, E).

**What H.36 does not do, on purpose.** No restart of `bonooru` during the session and no close with the bridge alone (issue #45); the bridge-out step replugs into the same socket with the app running. No gate-block-while-key-held on the rig (host-tested, `g25_block_and_idle_with_keys`). No IME or non-US layout (host-tested through `physical_key`; Open questions 18). No Cmd+Q or other app-quitting combo.

**Dry run.** `g25_h36_dry_run` replays the H.36 steps on a fake worker with an `ocb-sim` controller and prints the expected lines; it must be green on the bench commit (rig block item 5).

### H.36 (for `docs/test-plan.md` › Hardware-in-the-loop; Result empty until the bench)

**H.36 Gamepad keyboard and compact layout on the real rig with OoT running (Story 4.9 AC 1–9, 12; M5 keyboard exit; Gamepad keyboard bench rig).** macOS: `app/target/release/bonooru`, the agent reads `~/Library/Logs/Bonooru/bonooru.log` (this run's lines after its `start` line); the owner does each step on the agent's prompt and says what the Switch shows or plays. "Quick press" = a normal key tap. Window at 1280 × 800 or wider until step 13. Line order: the app's `pad press` line always precedes the worker's `pad sent` for it; the app's and the worker's lines may otherwise interleave (two threads, one log). Pass, per step:

1. Both boards plugged before the start, controller in the Switch's top USB-C port, Wired Communication On, console awake, OoT running (record where). Start the app; wait for the three `led … =ok` lines and `gate gamepad=on …`. Owner **clicks an empty spot of the gamepad panel** (above the pad, right of the tab) → `pad keys=on source=click`, no `pad press`; owner: the amber ring, the wash and "Keys active" show. Owner **presses Esc** → `stop requested source=esc …`, `pad keys=off reason=esc`; ring and chip gone. Owner presses **Tab four times** → `pad keys=on source=tab` at the fourth; ring on the panel.
2. Owner **quick-presses L** (on a connection screen this is A: close it; it may take two presses, as at H.35) → `pad press control=a source=keyboard key=l`, `pad sent port=<p> buttons=0x0004 hat=8 lx=128 ly=128 rx=128 ry=128` with `+ms(sent) − +ms(press)` ≤ 20; `mode … from=idle to=direct …` within 100 ms on the session's first press; `pad release control=a reason=tap key=l` (key up within 50 ms) or `reason=key_up` (longer); `pad sent … buttons=0x0000 hat=8 …`; with `reason=tap`: `+ms(sent release) − +ms(sent press)` in [49, 70]; with `reason=key_up`: ≥ 49 and `+ms(sent release) − +ms(pad release)` ≤ 20; no `nak`. Owner: the game reacts exactly once per press.
3. In game, owner **quick-presses ArrowLeft** → `pad sent … hat=6 …` then `hat=8`. Owner: the ocarina comes out (U9); wait 2 s.
4. Owner **quick-presses L, K, H, Q, E** (one at a time, about 1 s apart) → five `pad press … source=keyboard key=<l|k|h|q|e>` with `pad sent buttons=0x0004`, `0x0008`, `0x0001`, `0x0010`, `0x0020`, each followed by `0x0000`. Owner: five notes, five pitches. **M5 keyboard exit:** steps 2–4 with the keyboard only (Joy-Cons untouched).
5. Owner **holds L for about 3 s** (macOS key repeat runs), then releases → exactly one `pad press control=a source=keyboard key=l` and one `pad sent buttons=0x0004`, then `pad release control=a reason=key_up key=l` 2 500–4 000 ms later and one `pad sent buttons=0x0000`; between them no `pad press`, `pad sent`, `pad ignored`, `mode … to=idle`, `nak`, `stop` or `reboot` line. Owner: one continuous note that ends at the key release.
6. Chord: owner **holds L, then also K**, releases **K**, then **L** → `pad sent buttons=0x0004`, `0x000c`, `0x0004`, `0x0000` in that order. Owner: records what the game plays (any; the verdict is the wire).
7. Owner **presses Enter** (pause menu opens: `buttons=0x0200` then `0x0000`). In the menu: **hold W, then also ArrowUp**, release **W**, then **ArrowUp** → `pad sent … hat=0`, no `pad sent` for the second press nor for the W release, `hat=8` at the ArrowUp release. **Hold ArrowLeft, then also D**, release **ArrowLeft**, then **D** → `hat=6`, `hat=8`, `hat=2`, `hat=8`. **Enter** again (menu closes). Owner: records what the menu cursor did.
8. Back in free play (ArrowLeft if needed): owner **holds L**, after about 1 s **presses Esc**, then releases L → app lines: `stop requested source=esc dialog=none command=queued`, `pad release control=a reason=stop key=l`, `pad keys=off reason=esc`; worker lines: `stop sent`, `stop acked … after_us=<n>` n ≤ 100 000, `mode … from=direct to=idle …`, `stop after … mode=idle …`; no `pad sent` after the Esc; the L release writes nothing; a new **L press writes no `pad` line** (keys off). Owner: the note stops at Esc.
9. Owner **clicks the panel**, **holds L**, then **clicks the note view** (centre card) with the mouse, then releases L → `pad keys=on source=click`, `pad press … key=l`, `pad sent 0x0004`, then `pad keys=off reason=elsewhere`, `pad release control=a reason=focus_lost key=l`, `pad sent … buttons=0x0000 …`; the L release writes nothing. Owner: the note stops at the click.
10. Owner **clicks the panel**, **holds L**, **presses Cmd+Tab** to another app, releases L, Cmd+Tab back → `pad release control=a reason=window_blur key=l`, one `pad sent … buttons=0x0000 …`, `pad keys=off reason=window_blur`; after coming back an **L press writes no `pad` line** and the ring is gone. 10b: same with **Cmd+H** (bring Bonooru back from the Dock) → the same lines. Owner: the note stops at Cmd+Tab / Cmd+H.
11. Owner **clicks the panel**, then presses **Tab three times** and **Shift+Tab once** → no `keys=off` line; owner: the ring stays on the panel. **Space** → no `pad` or `stop` line. **Cmd+Enter** → no `pad` line. **Backspace** → `pad press control=minus source=keyboard key=backspace`, `buttons=0x0100` then `0x0000` (owner records the effect; the game may open a menu: close it with Backspace or B = J).
12. **Bridge USB out of the Mac** (panel still focused) → `port_lost`/`port_vanished`, `gate gamepad=off gamepad_reason=no_bridge …` (a short `bridge_not_ready` line may come first); owner **presses L** → `pad ignored control=a reason=no_bridge key=l`, no `pad sent`, no `stop` line. Bridge back into the same socket → LEDs ok, `gate gamepad=on …`; a **quick L** works as in step 2.
13. Owner **resizes the window to about 1100 pt wide** → the compact grid shows (no silhouette, no sticks, no chips, no legend); owner hovers A: the tooltip says "A: L key". Owner **quick-clicks A** and **quick-presses L** (click the panel first if the ring is gone) → each `pad sent buttons=0x0004` then NEUTRAL. Owner resizes back to 1280 pt or wider → the full pad.
14. Stop the app by closing its window (bridge and controller connected) → `port_closed`, the process exits within 1 s.

M5 keyboard exit: pass if steps 2–4 reached free play and played notes with the keyboard only. Windows / Linux: deferred: owner has no Windows/Linux machine yet; issue #43 (needs a `bonooru` build on that PC; the same steps; note that winit's key-repeat and IME paths differ there).

### Rig block (for `docs/test-plan.md`, "Gamepad keyboard bench rig (Story 4.9)"; E3-A3)

Rig: the Gamepad bench rig of Story 4.8 unchanged (own-bridge rig, wiring GP0/GP1/GND not rechecked; the Switch 2 with OoT running; versions from `docs/test-plan.md` › Test environment: system 23.5.0, OoT 1.1.1; a save where Link can walk and take out the ocarina). Boards: bridge Story 3.1 `69f4d2c` `fw-release`, controller `fw-debug-o3`, both `proto` 1. Keyboard: the Mac's built-in keyboard, macOS input source ABC/US (record it), key repeat at the system default (record "Key repeat rate" and "Delay until repeat" from System Settings › Keyboard). The tool is the release `bonooru` (`cargo build --locked --release -p ocb-app` in `app/`); no other serial tool at the same time. The instrument is the `pad` (including `keys=`) and `mode` lines plus the Story 4.4–4.7 worker, `led`, `gate` and `stop` lines, all stamped `+<ms>` by one log, and what the owner sees and hears; every expected line is asserted first by `g25_h36_dry_run` on the bench commit.

**Bench start checklist (every session; retro action E3-A3):**

1. Console auto-sleep off (System Settings › Sleep Mode). If OoT's controller-connection screen appears while `bonooru` runs with the three LEDs ok, close it from Bonooru: a quick click on A or a quick L with the panel focused; it may need **two** presses (the first registers, the second confirms; H.35, 2026-11-28). Only when `bonooru` is not running: `python3 firmware/tools/link_probe.py --bridge own --port "$P" press --button A`.
2. Boards plugged (controller in the Switch and wired, bridge on the Mac) **before** the app starts.
3. To stop the app: close its window; if it hangs, `kill -INT "$(pgrep -x bonooru)"` (its own PID, never a wrapper shell). Never restart the app while a previous `bonooru` is still running (issue #45).
4. Keep the Bonooru window visible and focused, not minimised or hidden (keys reach only the focused window; macOS throttles hidden windows). Steps 10/10b are the only planned focus losses; the resize of step 13 is the only planned size change.
5. Record once: host OS and version, machine, keyboard and input source, key-repeat settings, `bonooru` commit (`h36_dry_run` green on it), bridge firmware, controller firmware, Switch 2 system version, OoT version and where it was at step 1, cable, USB sockets.

### Bench gotchas (from Epics 1–4)

- macOS completes the Pro Controller handshake by itself: H.36 uses the console for the controller (as H.33–H.35).
- The console may ignore the first input after the controller connects (Story 1.4): a first press with no effect is recorded and repeated, not failed. The connection screen may need two A presses (H.35).
- Free play accepts notes about 2 s after the ocarina comes out (U9); B (J) or D-pad left (ArrowLeft/A) leaves it.
- A held note stops by itself after about 6.6 s (U1): keep holds at about 3 s.
- macOS key repeat starts after the "Delay until repeat" (default about 0.5 s) — step 5's hold must be longer than that to test anything (3 s is).
- Esc clears egui focus: after step 8 the ring is gone; click the panel again (step 9 does).
- macOS renames the bridge port with the USB socket: replug into the same socket in step 12.
- Issue #45: never start a second `bonooru` while one is closing.
- Home and Capture have no key; game screenshots stay on the console and never go into the repo.
- Cmd+Tab back to Bonooru: keys stay off until a click on the panel (D4) — that is the expected result of step 10, not a defect.
- The pause menu does not open during ocarina free play: leave free play with J (B) before step 7's Enter (H.36, 2026-11-28).
- Every reply typed in a terminal blurs the window (`pad keys=off reason=window_blur`): start each step with a click on the panel or Tab (H.36, 2026-11-28).

### H.36 runbook (macOS, Task 12)

Agent: `cd app && cargo build --locked --release -p ocb-app`; `cargo test -p ocb-app -- --nocapture h36_dry_run` on the same commit (paste the expected lines into the Debug Log); start `app/target/release/bonooru` in the background; after each owner step, `tail` the log from this run's `start` line and match the lines. Owner: one step at a time on the agent's prompt (one question at a time), says what the Switch shows or plays. The agent never sends SIGINT unless the window close hangs.

### Instrument review (E3-A2 for the bench verdicts)

Each H.36 verdict has a host test showing both the passing and the failing value of its line: one press per held key (G.25 `g25_key_repeat_ignored`) vs one per repeat (red proof: repeats not skipped); tap length ≥ 50 ms (`g25_key_tap_and_hold`) vs the release in the press frame (red proof: key release ignores `TAP_MIN_US`); chord `0x000c` (`g25_chords_and_union`) vs `0x0008` (red proof: `state()` takes only the last key); duplicate keep (same test) vs `hat=8` at the W release (red proof: release by control, not by key); opposite cancel vs `hat=6` kept (red proof: hat from the last direction); `reason=stop` and no `pad sent` after Esc (`g25_stop_releases_keys`) vs a resent A (red proof: `release_all` skips key holds); `focus_lost` release (`g25_focus_loss_releases`) vs none (red proof: focus loss not handled); `window_blur` release and `keys=off` (`g25_blur_releases_keys`) vs keys still on (red proof: no surrender on blur); no `keys=off` through Tab × 3 (`g25_focus_lock`) vs a loss (red proof: no lock filter / no fallback); `pad ignored … key=` while gated (`g25_gated_keys`) vs a `pad sent` (red proof: gate check skipped for keys); Space/Cmd+Enter silent (`g25_global_keys`, `g25_modifiers_and_consumption`) vs a press (red proof: modifiers not checked).

### Log lines (AC 12; kind tag `pad`; the bench reads exactly these)

App (UI thread, `App::log`; `<c>` = `Control::log_name()`, `<k>` = `PadKey::log_name()`):
- `press control=<c> source=<mouse|accesskit|keyboard>[ key=<k>]` — ` key=` only with `source=keyboard`; written before the `try_send` of the state it causes; one per accepted fresh key press, also when the control is already held by the mouse or another key (then no `pad sent` follows).
- `release control=<c> reason=<pointer_up|tap|key_up|focus_lost|window_blur|stop|gate|controller_idle>[ key=<k>]` — one per ended hold; ` key=` for a key hold; `tap`: the release waited for the 50 ms minimum.
- `stick=<s> start` / `stick=<s> end reason=<pointer_up|window_blur|focus_lost|stop|gate|controller_idle|layout>`.
- `ignored control=<c|stick_left|stick_right> reason=<worker|no_bridge|bridge_not_ready|firmware_update|controller_link|switch_not_ready|busy>[ key=<k>]` — ` key=` for a gated key press (keys are never `busy`).
- `keys=on source=<click|tab|other>` — the panel gained focus (`other`: AccessKit or anything else).
- `keys=off reason=<esc|dialog|window_blur|elsewhere>` — the panel lost focus; written before the release lines it causes, except in a STOP frame, where `stop requested` and the `release … reason=stop` lines come first, and on window blur, where the `release … reason=window_blur` lines come first (D-1, 2026-11-28).
- `command=<…>` unchanged.

Worker lines unchanged (`pad sent|dropped`, `mode`). Patterns (`tests/common::line_matches`; no regex crate): extend `press` with `source` OneOf[mouse, accesskit, keyboard] + optional `key` OneOf[the 18 names]; `release` reason OneOf[…] + optional `key`; `stick … end` reason gains `focus_lost`, `layout`; `ignored` optional `key`; `keys=on` + `source` OneOf[click, tab, other]; `keys=off` + `reason` OneOf[esc, dialog, window_blur, elsewhere].

### Key table (`gamepad::PadKey`; G.25 `g25_every_key_bit`, H.36)

| Physical key (`egui::Key`) | `log_name` | Control | Wire |
|---|---|---|---|
| `ArrowUp`, `W` | `arrow_up`, `w` | D-pad up | `hat=0` |
| `ArrowDown`, `S` | `arrow_down`, `s` | D-pad down | `hat=4` |
| `ArrowLeft`, `A` | `arrow_left`, `a` | D-pad left | `hat=6` |
| `ArrowRight`, `D` | `arrow_right`, `d` | D-pad right | `hat=2` |
| `H` | `h` | Y | `buttons=0x0001` |
| `J` | `j` | B | `0x0002` |
| `K` | `k` | X | `0x0008` |
| `L` | `l` | A | `0x0004` |
| `Q` | `q` | L | `0x0010` |
| `E` | `e` | R | `0x0020` |
| `Z` | `z` | ZL | `0x0040` |
| `C` | `c` | ZR | `0x0080` |
| `Enter` | `enter` | Plus | `0x0200` |
| `Backspace` | `backspace` | Minus | `0x0100` |

Diagonals: two adjacent directions → `HAT_UP_RIGHT` etc. (`gamepad::hat`); opposite directions cancel on that axis. Home (`0x1000`), Capture (`0x2000`), sticks: no key.

### Focused look (DESIGN.md › Virtual gamepad › Focused; mock `.right.gfocus{box-shadow:inset 0 0 0 2px var(--amber),inset 0 0 22px #f0b43c40}`)

On the gamepad region rect `r`, after the region fill and before the content: wash = 6 strokes, stroke `i` (0..6) of width 3 pt (`wash-step`) centred at inset `2 + 3·i + 1.5` pt, colour `AMBER_WASH` (`#f0b43c22`) with its alpha scaled by `(6 − i) / 6` (so the edge is strongest and the wash fades over about 20 pt, like the mock's 22 px blur); then the ring = `rect_stroke(r, 0, paint::stroke(2.0, AMBER, ppp), StrokeKind::Inside)`. The region is square-cornered (`theme::corner(0.0)`). The chip: text "Keys active" (`theme::CHIP`, `INK`), box = galley + 8 pt left/right + 1 pt top/bottom, `rounded.full`, `AMBER` fill, right edge at `inner.right()`, bottom 3 pt above the underline (mock `.ph em{margin-bottom:3px}`, `.ph{align-items:flex-end;justify-content:space-between}`). If the eye check finds the wash too strong or weak, change only the tokens.

### Compact geometry (no mock; points; origin = the grid's top-left = (inner.left, caption.bottom + 6), grid 180 × 124)

- Shoulder row: ZL rect (0, 0, 42 × 15) r 6; L rect (46, 0, 42 × 15) r 6; R rect (92, 0, 42 × 15) r 6; ZR rect (138, 0, 42 × 15) r 6.
- Middle row (centres at the quarters of 180, y 34): Minus c (22.5, 34) r 7; Capture rect (62, 28.5, 11 × 11) r 2 with the dot r 2.6 at its centre; Home c (112.5, 34) r 6.5 with the Story 4.8 house shape translated by its centre (points relative to the centre: (−3.5, 1), (0, −2.5), (3.5, 1), (3.5, 3.5), (−3.5, 3.5)); Plus c (157.5, 34) r 7.
- D-pad: well c (45, 90) r 22; arms 14 × 14 r 2 at up (38, 71), down (38, 95), left (26, 83), right (50, 83), centre (38, 83) (no hit target); arrows as Story 4.8; one cross outline as Story 4.8.
- ABXY: well c (135, 90) r 31; X c (135, 72), Y c (117, 90), A c (153, 90), B c (135, 108), each r 10.
- Letters, symbols, tints, hit tests (circles by distance, rects by rect, D-pad by arm) exactly as Story 4.8. Free spots for tests that click empty panel space: e.g. (90, 90) between the wells (grid coordinates) or any point of the region outside the controls under the header.
- Panel: tab and chip as full; caption 4 pt under the underline in a 180 × 34 box (wrapped, ≤ 2 lines, `TEXT_MUTED`, `typography.small`); grid 6 pt under the caption; hint 8 pt under the grid, down to the inner bottom. At 1024 × 680 the region is 548 pt high: header ≈ 22 + caption 38 + grid 130 + hint ≤ 60 fits.

### Copy (approval in Open questions 10 where "draft")

| Where | String | Source |
|---|---|---|
| Chip (focused) | "Keys active" | mock `.kbd-on`, DESIGN.md, UX-DR37 |
| Group node name | "Gamepad – keys active" / "Gamepad – keys off" | draft (Story 4.2 table: "name includes keys active"); the en dash as in the LED names |
| Compact tooltips | "A: L key", "B: J key", "X: K key", "Y: H key", "D-pad up: W or Up arrow", "D-pad down: S or Down arrow", "D-pad left: A or Left arrow", "D-pad right: D or Right arrow", "L: Q key", "R: E key", "ZL: Z key", "ZR: C key", "Plus: Enter", "Minus: Backspace", "Home: mouse only", "Capture: mouse only" | draft (from the key table; names = the AccessKit names) |
| Legend, chips, caption, hints | unchanged (Story 4.8) | — |

### Current state of the files this story touches (read before editing)

- `app/gui/src/gamepad.rs` (1039 lines, pure, no egui): `Control` (16) with `a11y_name`/`log_name`/`effect`/`alone`; `Stick`; `Target`; `Source { Mouse, AccessKit }`; `Release { PointerUp, Tap, WindowBlur, Stop, Gate, ControllerIdle }` with `neutral_without_send` for the last three; `hat()` already handles every combination of the four directions (written for this story, 16-case test); `axis`, `stick_value`; line builders; `Pad { held: Option<Held>, release_at, dragging, sticks, pointer_down, spent, last_sent, pending, in_flight, fail_logged, last_mode, last_status_rx, sent_after_rx, lines }` with `press`, `tap`, `stick_start/drag/end`, `spend_press`, `pointer_up`, `tick`, `release_all`, `observe_mode(mode, rx_us)` (the stale-STATUS guard), `take_send`, `sent(Result)`, `next_wake_us`. `state()` applies one held control (`apply` sets the hat from one direction). **Change:** Task 1. **Preserve:** every mouse behaviour and line (G.23 must stay green), the stale-STATUS rule 8d, resend after a failed send, `RESEND_AFTER_US`, the busy rule for the mouse.
- `app/gui/src/ui/gamepad.rs` (841 lines): `layout(region, TextHeights)`, fixed text heights (19/17/17 pt, G.1 ppp rule), `PadGeometry::new(origin)` from the mock, `hit`, `control_id`/`stick_id`, `PadView`, `PadInput { press, press_offset, released, primary_down, drag_offset, tap }`, `show(ui, region, view)` (interaction first, then painting; press only if it reached the pad), `paint_*`, `view_of`. **Change:** Tasks 3, 4. **Preserve:** the full pad's geometry, look and every G.22 check at 1280 × 800; `Sense::CLICK | Sense::DRAG` for buttons and `Sense::DRAG` for sticks (no `FOCUSABLE`); the press detection from the press event's own position.
- `app/gui/src/ui/layout.rs`: `regions(content)` always gives the gamepad 300 pt (comment "the compact variant below 1280 is Story 4.9"); `in_reading_order`; G.1 unit tests over 1024/1280/1920/2560. **Change:** Task 3.1. **Preserve:** reading order, tiling, the clamps for tiny windows, ppp independence.
- `app/gui/src/ui/main_screen.rs`: regions in reading order, each a scoped `Ui`; the gamepad region calls `gamepad::show` with the gate's hint; `hint_rect(ctx, rect)` returns the full layout's hint rect (G.1/G.18 use it). **Change:** `hint_rect` picks the variant's hint rect. **Preserve:** region order (it is the Tab order).
- `app/gui/src/ui/keys.rs`: `frame_start` (Esc read, never consumed; Space consumed unless a text edit has focus, Cmd/Ctrl+Space only with focus; Enter repeats dropped); private `command_held`. **Change:** `command_held` → `pub(crate)`. **Preserve:** everything else (G.20).
- `app/gui/src/app.rs` (814 lines): `logic` drains the worker in pass 0 and runs `pad_background(focused, visible, blur_event)` in every pass (blur incl. hidden window, rule 8d, tap timer, STOP retry, send); `ui`: `keys::frame_start` → LEDs → gate → firmware dialog (`request_discard`) → `view_of` → `main_screen::show` → STOP request (first pass) → `pad_input(&out.pad, stop_result, &gate)` → `pad_flush` → repaint requests (every pass for `last_pad_wake`) → late STOP → `Modals::show` → firmware outcome → smoke. **Change:** Task 6. **Preserve:** that order, "STOP before any pad line of the frame", first-pass-only input, `request_discard`, the STOP retry, `on_exit`, the smoke frame, the `logic`-only blur path.
- `app/gui/src/ui/modal.rs`: the open dialog requests focus for its primary button (`modal.rs:421`) and consumes Esc after `keys::frame_start` read it. **Not changed**; the panel loses focus to it (`keys=off reason=dialog`).
- `app/gui/src/ui/theme.rs`: `AMBER`, `AMBER_WASH`, `INK`, `spacing::GAMEPAD_COMPACT_WIDTH = 200` (already a token), `tooltip_frame()`, `KEY_CHIP`, `components::gamepad_*`. **Change:** Task 2.
- `app/gui/src/ui/status_strip.rs`: the LED tooltip pattern (`Tooltip::for_enabled` + `tooltip_frame`) — reuse it for the compact tooltips. **Not changed.**
- `app/services/*`, `app/core/*`, `firmware/*`: **not changed** (a chord is just another `State`; DIRECT is unmasked, AD-9).

### egui 0.37 facts used (checked in `~/.cargo/registry/src/*/egui-0.37.0`, `egui-winit-0.37.0`, `egui_kittest-0.37.0`)

- `Event::Key { key, physical_key: Option<Key>, pressed, repeat, modifiers }`; egui-winit fills `physical_key` from winit's `PhysicalKey::Code` and `key` from the logical key or, failing that, the physical one (`egui-winit/src/lib.rs:1107-1160`); `repeat` is set by egui from `keys_down` (keyed on `key`), not by winit (`input_state/mod.rs` ~l.405–415).
- egui clears `keys_down` on `Event::WindowFocused(false)` (`input_state/mod.rs` ~l.444–450), so the first OS repeat after a refocus arrives with `repeat == false` (D4 avoids it by dropping panel focus on blur).
- egui-winit: on Windows/Linux Ctrl+C/X/V (Cmd on macOS) become `Event::Copy/Cut/Paste` with no key press; the release is still a `Event::Key { pressed: false }` (`lib.rs:1128-1160`). IME-processed key presses (Windows `Process` key) push no key event, and their releases are swallowed too (`lib.rs:~660-705`); egui allows IME only while a text edit asks for it, so with the panel focused IME is off.
- `Memory::set_focus_lock_filter(id, EventFilter)` works only if `had_focus_last_frame(id) && has_focus(id)` (`memory/mod.rs:908-916`); `begin_pass` applies the previous frame's filter to Tab (any modifiers), arrows (`EventFilter::matches` checks only the key) and Esc; without the filter Tab/Shift+Tab move focus, unmodified arrows move focus, unmodified Esc clears focus (`memory/mod.rs` ~l.571–606). `request_focus` replaces the `FocusWidget` (default filter) (`memory/mod.rs:923-926`).
- `SurrenderFocusOn::Clicks` (default): a focused widget not hovered at a click loses focus, checked inside its own `interact` call (`context.rs:1844-1853`); hover stops at the topmost interactive widget, non-interactive widgets above it stay hovered (`interaction.rs:257-300`).
- `Sense::FOCUSABLE` alone: focusable, gets the AccessKit `Focus` action, no `Click` action, no fake click on Enter/Space (`context.rs:1758-1772`, `response.rs:999-1005`).
- `UiBuilder::accessibility_parent(id)` (`ui_builder.rs:204`): "Pass `Ui::unique_id` or `Response::id`".
- kittest: `Harness::key_press` pushes `physical_key: None` (`egui_kittest-0.37.0/src/lib.rs:674-749`) — push your own events; kittest calls `App::logic` and `App::ui` but not `raw_input_hook` (`app_kind.rs:52`).

### Regressions to expect (update with the reason in the Debug Log)

- G.1 (`layout.rs` unit tests, `main_screen.rs` tests, `g1_gamepad_hint_geometry`): the gamepad is 200 pt below 1280; the note view grows by 100; the hint rect at 1024 is the compact one.
- G.10 (`tests/a11y.rs`): a new `Group` node; at 1024 no `Slider` nodes.
- G.15/G.17/G.18 (`g18_hint`, the hint geometry at 1024): compact hint rect. G.18's Tab loop in `firmware_app.rs` (~l.565): the panel now joins the ring and locks Tab.
- G.20 (`stop_app.rs`): `g20_tab_order` (AC 11), `Rig::focus_stop` (Tab × 4 → now Shift+Tab once from nothing focused), H.34 dry run steps that tab to STOP. `bottom_bar.rs` (~l.359) and `button_states.rs` (~l.210) tab four times to STOP: same change.
- G.22 (`tests/gamepad.rs`): `g22_drawn_inside` at 1024 × 680 now draws the compact panel: run the full-pad checks at 1280 × 800 (and 1920 × 1080), the compact ones in G.24; `g22_accesskit`: controls' parent is the group.
- G.23 (`gamepad_app.rs`): `g23_stop_drops_held`'s "STOP button activated by Enter while A is held" — a press on A now takes panel focus, so Enter would press +: use an AccessKit click on STOP instead (record why); `g23_h35_dry_run` step 7 (Tab × 4, hold A, Enter) likewise; any test that assumed a pad press leaves egui focus untouched.
- C.23–C.29: unchanged (no worker change).

### Red proofs (E3-A2 for host checks; minimum list for the Debug Log)

Model: repeats not skipped (`key_down` on a held key adds a press) → repeat test; key release without `TAP_MIN_US`; `state()` from the last key only; release by control instead of by key (duplicates); hat from the last direction; `release_all` skipping key holds; mouse `free()` counting keys (a mouse press while a key is held becomes `busy`). G.24: ring/wash drawn without focus; chip overlapping the tab at 200 pt (move it 20 pt left); a compact control 2 pt off; the stick wells drawn in compact; a tooltip text not matching the key table; the controls parented to the region instead of the group (drop `accessibility_parent`); the group named the same with and without focus; `pad_key` mapping `Key::X`. G.25: keys taken without focus; keys taken in the frame focus is gained; Cmd not checked; mapped events not consumed; physical `None` mapped through the logical key; no re-request of focus after a click on a control (focus lost at the release); lock filter set before `request_focus` (Tab in the next frame leaves); no lock fallback (Tab in the first frame after Tab-in leaves); no `surrender_focus` on blur (keys stay on); focus loss not releasing; STOP not releasing keys; gate check skipped for keys; drag not ended on the layout switch.

### Accepted limits (for EXPERIENCE.md › Virtual gamepad; nothing stays held in any of them)

- A key pressed in the same frame as the click or Tab that focuses the panel, or the Esc/click that unfocuses it, does nothing (D6).
- A key still held when the panel regains focus by a click (without a blur in between) does nothing until it is released and pressed again (egui flags its repeats).
- A key held through Cmd+Tab (or Cmd+H) and still held when the panel is clicked again after coming back presses its control once more at its next OS repeat: egui clears its held keys on `WindowFocused(false)`, so that repeat arrives unflagged and looks fresh; the hold ends at the key's release as usual (review 2026-11-28, accepted).
- More keys than the keyboard can report at once (hardware rollover) are simply not seen.
- Non-US layouts and IME input sources: positional by design, not benched (Open questions 18).
- The panel traps Tab while focused; Esc (also STOP) and a click are the exits (D3).

### Previous story intelligence (4.3–4.8, Epic 3)

- 4.3: painting helpers (convex-only fills, strokes floored at 1 px, symbols without nodes); `paint_focus_ring` is for controls, the panel uses its own inset ring (DESIGN); the modal's Esc contract (read at frame start, consumed by `Modals::show`, closes `Dismissed`).
- 4.5: the LED tooltip pattern (`Tooltip::for_enabled` + `theme::tooltip_frame`); hand-stepped frames hide missing repaints (ask for a repaint after a focus gain; test frames driven only by repaint requests where timing matters); rig block / runbook format.
- 4.6: input only in the first pass; `request_discard` when the firmware dialog opens (the dialog takes focus: `keys=off reason=dialog`); C.27.
- 4.7: Esc read at frame start, never consumed; at most one STOP per frame; Space consumed; Enter repeats dropped; tests must use realistic event sequences (egui marks a press of a held key as repeat); forward note: after STOP the gamepad drops its own held set **mouse and keys** (AC 6b).
- 4.8: the pad model is pure and returns lines as data; the app writes `press` before the `try_send`; the hidden-window path runs in `logic` (keys too: `release_all(WindowBlur)` covers them); a STOP that cannot be queued is retried; a stale STATUS must not drop a newer press (`observe_mode(mode, rx_us)`); the starvation-safe G.23 rig (`Rig::frame` waits for a whole worker pass); review lessons: list bench lines in the order the code writes them, and allow transient `bridge_not_ready`; H.35: the connection screen needed two A presses; at the eye check the owner noticed the keyboard bindings do nothing yet (this story); the full pad at 1024 was accepted only until this story.
- Epic 3 retro: E3-A1 table above; E3-A2 red proofs; E3-A3 checklist in the rig block; E3-A6: a milestone stays TODO while its deferred Windows/Linux rows are open (applied to M5, D16).

### Git intelligence

Last commits: `d62be7a` (Story 4.8 done), PR #49 merged (`5f0d896`), `d4da8ee` (H.35 bench recorded), `90ab7ef` (Story 4.8 review fixes: logic-only pad background, NEUTRAL refused → STOP, stale-STATUS rule 8d, STOP retry, starvation-safe rig), `77b79d6` (the virtual gamepad with mouse). Commit style: one long descriptive sentence per commit, story number in parentheses. Branch `story/4-9-gamepad-keyboard-focus-and-compact-layout` is checked out from up-to-date `main`.

### Library / framework requirements

No new dependency. egui/eframe stay pinned at `=0.37.0` (G.9); `egui_kittest` 0.37.0; `crossbeam-channel` `try_send` only (G.13); `ocb-sim` stays a dev-dependency (never in the normal graph). `cargo deny` bans unchanged (AD-21). Everything used here (`EventFilter`, `set_focus_lock_filter`, `Sense::FOCUSABLE`, `UiBuilder::accessibility_parent`, `Tooltip::for_enabled`, `Event::Key::physical_key`) exists in 0.37.0 (paths above).

### Architecture compliance

- AD-7 / AD-16: the UI sends `SetState` with `try_send` and never waits; PING while DIRECT comes from the core session.
- AD-8: STOP releases everything (keys included); NEUTRAL is the core constant (AD-10).
- AD-9: DIRECT is unmasked, so chords reach the Switch as sent.
- AD-18: pure policy at the crate root (`gamepad` gains `PadKey`), egui input under `ui/` (`ui::pad_keys`), drawing in `ui::gamepad`.
- AD-21: no network; new strings pass the banned-words check.
- UX-DR6, UX-DR37, UX-DR41, UX-DR61, UX-DR62: as in the ACs.

### File structure

- New: `app/gui/src/ui/pad_keys.rs`, `app/gui/tests/gamepad_focus.rs` (G.24), `app/gui/tests/gamepad_keys_app.rs` (G.25).
- Changed: `app/gui/src/gamepad.rs`, `app/gui/src/ui/gamepad.rs`, `app/gui/src/ui/layout.rs`, `app/gui/src/ui/main_screen.rs`, `app/gui/src/ui/keys.rs` (visibility only), `app/gui/src/ui/mod.rs`, `app/gui/src/ui/theme.rs`, `app/gui/src/app.rs`, `app/gui/tests/common/mod.rs` (patterns, key helper), `app/gui/tests/{design_tokens.rs, a11y.rs, main_screen.rs, gamepad.rs, gamepad_app.rs, stop_app.rs, bottom_bar.rs, button_states.rs, firmware_app.rs, gating.rs}` as needed, `app/README.md`, `docs/test-plan.md`, `_bmad-output/planning-artifacts/epics.md`, `_bmad-output/planning-artifacts/ux-designs/ux-bonooru-2026-11-20/{EXPERIENCE.md, DESIGN.md}`, `_bmad-output/specs/spec-bonooru/roadmap.md`.
- Not touched: `app/core`, `app/services`, `app/sim`, `app/tools/*`, `firmware/`, `app/gui/src/ui/modal.rs`, `app/gui/src/leds.rs`, `app/gui/src/gating.rs` (unless a test helper needs it), `app/gui/src/ui/bottom_bar.rs`.

### Project Structure Notes

Matches AD-18 and `app/README.md`: pure UI policy at the crate root of `ocb-app` (`leds`, `gating`, `stop`, `gamepad`), drawing and egui input under `ui/` (`ui::gamepad`, `ui::keys`, new `ui::pad_keys`). No variance. `ui::pad_keys` is separate from `ui::keys` because `keys::frame_start` runs before every widget in every mode, while the pad keys are read only in the main screen after the panel is drawn (they depend on its focus).

### Project Context Rules

No `project-context.md` exists. Repo rules that apply: every repo file in English; `_bmad/`, `.claude/` and bmad-*/gds-* skill files are never staged or committed; game screenshots (Capture on the bench) never committed; one question at a time to the owner (Tasks 12–13); the owner's notes under `~/Documents/org/` are read-only; test environment facts (Switch 2 23.5.0, OoT 1.1.1) live in `docs/test-plan.md`; the GP0/GP1/GND wiring is not rechecked unless rewired; search upstream (egui sources) before raising an open question.

### Eye check (Task 13, owner, late)

`cd app && cargo run -p ocb-app` (no boards needed for the look). Questions, one at a time: (1) Click the gamepad panel: the inset amber ring and wash against the mock (`mockups/directions-oot-2026-11-20.html` direction 3, `key-empty-state.html` `.gfocus`): ok? (2) The "Keys active" chip: size, colour, place next to the tab? (3) Resize below 1280: the compact grid (layout, sizes, the caption on two lines): ok? (4) Hover a few compact controls: tooltips readable, wording ok? (5) The compact variant without sticks: acceptable (D9)? (6) Tab into the panel and try to Tab out: is "Esc or a click leaves" acceptable (D3)? Record each answer.

### Open questions for the owner (defaults applied; the story is written with them)

1. **Run H.36 now?** Default: **yes**, about 25 minutes on macOS with OoT running (Bench decision). Alternative: host tests only, bench later (then the M5 keyboard exit waits).
2. **Key tap minimum.** Default: **50 ms, same as the mouse** (D2).
3. **Tab into a locked panel.** Default: **as the epic AC: Tab enters, Tab/arrows locked, Esc or a click leaves** (D3). Alternatives: (a) the lock covers only the arrows, Tab leaves to STOP; (b) the panel is not in the Tab ring (click only).
4. **Window blur drops panel focus.** Default: **yes**, keys stay off after Cmd+Tab back until a click (D4). Alternative: keep focus across a blur (keys work again at once after refocus; a key held through the switch may then press on its first OS repeat).
5. **Panel focus loss releases with NEUTRAL.** Default: **SET_STATE NEUTRAL** (D5). Alternative: STOP.
6. **Cmd/Ctrl presses.** Default: **Cmd and Ctrl block a press; Shift and Alt/Option do not** (D7). Alternative: any modifier blocks.
7. **Compact without sticks.** Default: **buttons only** (D9, UX-DR6). Alternative: two small stick wells under the grid.
8. **Compact key hints.** Default: **hover tooltips only** (D13). Alternative: also as each control's AccessKit description, or a one-line legend under the grid.
9. **Compact threshold and width.** Default: **below 1280 pt, region 200 pt** (UX-DR6; the `[ASSUMPTION]` on `gamepad-compact-width` dropped). Alternative: another width after the eye check.
10. **Copy.** Default: the Copy table (group names and tooltips draft).
11. **Focused look numbers.** Default: **ring 2 pt, wash 6 × 3 pt strokes fading from `amber-wash`** (Focused look). Alternative: tune at the eye check (tokens only).
12. **Chip typography.** Default: **new `typography.chip` (11.5 px, 700, no tracking, mock `.ph em`)**. Alternative: reuse `typography.label` (same size and weight, 0.06 em tracking).
13. **Keys while gated.** Default: **chip and ring still shown, each fresh key press logged `pad ignored … key=`** (D14).
14. **Consume mapped keys while focused.** Default: **yes** (D8).
15. **Esc with Shift/Alt held.** Default: **STOP, panel keeps focus** (egui behaviour; AC 8). Alternative: drop the panel focus on any Esc.
16. **Log format.** Default: **` key=<k>` suffix on keyboard `press`/`release`/`ignored` lines; `keys=on|off` lines** (Log lines).
17. **M5 in `roadmap.md`.** Default: **stays TODO** with the macOS evidence written in its cell; DONE only when the Windows/Linux rows on issue #43 pass (E3-A6, as M4; D16). Alternative: set M5 DONE for macOS now with a note (would break the E3-A6 rule used for M4).
18. **Numbers without an instrument.** Default: **listed and accepted**: time from `pad sent` to the game's reaction, H.36 on Windows/Linux (#43), IME and non-US layouts on the rig, keyboard rollover.
19. **H.36 on Windows and Linux.** Default: **deferred to issue #43** (no machine).
20. **Bench: Joy-Cons untouched in steps 1–13; Backspace (−) pressed in step 11.** Default: yes.

### References

- `_bmad-output/planning-artifacts/epics.md` › Epic 4 header and notes (AccessKit "basic"), Story 4.8 (boundary), Story 4.9, FR-22, UX-DR6, UX-DR37, UX-DR41, UX-DR61, UX-DR62.
- `_bmad-output/planning-artifacts/architecture/architecture-bonooru-2026-11-19/ARCHITECTURE-SPINE.md` › AD-7, AD-8, AD-9, AD-10, AD-16, AD-18, AD-21.
- `_bmad-output/planning-artifacts/ux-designs/ux-bonooru-2026-11-20/DESIGN.md` › frontmatter `spacing.gamepad-compact-width`, `components.gamepad-panel-focused`, `components.chip-keys-active`, `components.tooltip`, `typography.label`; Virtual gamepad (Focused, Compact variant), Focus ring, Tooltips; `mockups/directions-oot-2026-11-20.html` (`.d3 .right.focus`, `.d3 .kbd-on`, `.ph em`), `mockups/key-empty-state.html` (`.right.gfocus`, `.kbd-on`, `.ph`, `.ph em`).
- `_bmad-output/planning-artifacts/ux-designs/ux-bonooru-2026-11-20/EXPERIENCE.md` › Component Patterns › Virtual gamepad; State Patterns › Focus; Interaction Primitives (Global shortcuts, Context shortcuts, Gamepad focus, Virtual gamepad mapping, Banned); Accessibility Floor; Responsive & Platform.
- `docs/test-plan.md` › Test environment, Gamepad bench rig (Story 4.8), H.35, G.1, G.10, G.20–G.23; `docs/ocarina-input-mechanics.md` › U1, U9, U11; Story 1.4 U7 (min press).
- `_bmad-output/implementation-artifacts/4-2-…md` (review finding on `accessibility_parent`, the 4.9 row), `4-3-…md` (Esc contract), `4-5-…md` (tooltips, rig block), `4-6-…md` (first pass, gate), `4-7-stop-esc-and-the-bottom-bar.md` (keys at frame start, forward notes), `4-8-virtual-gamepad-with-mouse.md` (pad model, release rules, review findings, H.35 and its lessons, eye check); `epic-3-retro-2026-11-26.md` §5–6.
- `_bmad-output/specs/spec-bonooru/roadmap.md` › M4 (E3-A6 wording), M5.
- Code: `app/gui/src/{app.rs, gamepad.rs, gating.rs, stop.rs}`, `app/gui/src/ui/{gamepad.rs, layout.rs, main_screen.rs, keys.rs, modal.rs, status_strip.rs, paint.rs, theme.rs}`, `app/gui/tests/{gamepad.rs, gamepad_app.rs, stop_app.rs, common/mod.rs}`, `app/services/src/worker.rs` (unchanged; `handle_set_state`).
- egui sources: `egui-0.37.0/src/{memory/mod.rs, context.rs, interaction.rs, input_state/mod.rs, data/input/event_filter.rs, ui_builder.rs}`, `egui-winit-0.37.0/src/lib.rs`, `egui_kittest-0.37.0/src/{lib.rs, app_kind.rs}`.
- Issues: #43 (Windows/Linux benches), #45 (macOS restart hang).

### Review Findings (2026-11-28)

CI on `5653090` green on the three OS. Decisions by the session lead and patches from the review; each patch has a red proof in the Debug Log (Review fixes).

- [x] **D-1 (decision) Blur log order.** Kept the code (`release … reason=window_blur` lines, then `keys=off reason=window_blur`); Dev Notes › Log lines now reads "except in a STOP frame and on window blur".
- [x] **D-2 (decision) AC 6a′ with the firmware dialog.** Kept: the generic dialog measures the release order (L held), the firmware dialog only its `keys=off reason=dialog`, because the gate is already blocked when it opens; recorded in the AC table and the Debug Log. The held-key generic-dialog case exists (`g25_focus_loss_releases`) and is extended by `g25_dialog_takes_the_keys`.
- [x] **1 Key-up lost in a logic-only pass.** `logic` reads the frame's key releases from `i.raw.events` (`pad_keys::pad_key` of `physical_key`) and passes them to the new `App::pad_background_keys`, which ends their holds; idempotent with `ui` (a key not held or already waiting for its tap is ignored). Test `g25_key_up_in_a_logic_only_pass`.
- [x] **2 Re-press inside the 50 ms tap window.** `Pad::key_down` on a hold whose release is pending cancels the release, keeps `since_us` and writes the press line. Unit test `key_repress_in_the_tap_window`.
- [x] **3 Keys not gated by an open dialog.** With a dialog open (or requested in this frame: `Modals::request` opens it at once) the panel's focus is surrendered, `keys=off reason=dialog` and `release_all(FocusLost)` are written, key presses are skipped (key-ups still honoured, by `logic` and `ui`), and the lock filter is not set. Test `g25_dialog_takes_the_keys` (same-frame Enter; disabled primary).
- [x] **4 Lock fallback.** Counts only the keys egui's navigation acts on: unmodified arrows and Tab, Shift-only Shift+Tab; skipped in a frame with an AccessKit `Action::Focus` request (test `g25_accesskit_focus_leaves_the_panel`). **Deviation:** key repeats still count, because egui's `Memory::begin_pass` moves the focus on a repeated press too (egui 0.37 `memory/mod.rs`: the key match has no `repeat` test); excluding them would let an arrow held while clicking the panel take the focus away (test `g25_focus_lock_held_arrow`, red with the literal rule).
- [x] **5 Consume mapped keys in every focused frame.** `pad_keys::take` runs whenever the panel has focus (focus-gain frame, Shift/Alt+Esc STOP frame); presses are applied only under the D6 condition. Test `g25_keys_consumed_in_every_focused_frame`.
- [x] **6 One compact threshold.** `layout::is_compact` is `width < COMPACT_BELOW − 0.5`; `main_screen::show` sets `PadView::variant` from it (`Variant::for_content`), the panel draws that variant and reports it in `PadInput.variant`; `Variant::of(region)` removed; `gamepad::hint_rect` takes the variant and `main_screen::hint_rect` takes the content rect. Tests `g24_one_compact_threshold` (ppp 1.1: 1279 compact, 1279.6 and 1280 full) and `g1_compact_threshold` (1279.4 → 200, 1279.6 → 300).
- [x] **7 Right or middle click inside the panel.** `PadInput.panel_press` counts any button that reaches the panel (with `any_down`/`any_released` for the release keep); only the primary button presses a control. Test `g25_other_buttons_keep_focus`.
- [x] **8 Missing red proofs.** Added for `g24_compact_layout`, `g24_compact_drawn_inside`, `g24_no_online_wording`, `g25_dialog_blocks_focus`, `g25_chords_and_union`, `g25_block_and_idle_with_keys`, `g25_global_keys`, `g25_pad_lines`, the unit tests `key_table`, `key_hints`, `next_wake_with_key_deadlines`, `controller_idle_drops_key_holds`, and the H.36 verdicts step 1 (`source=click` vs `tab`), step 11 (Space silent) and step 13 (compact A and L each 0x0004, new host test `g25_compact_presses`).
- [x] **9 Stale "Tab × 4" text in `docs/test-plan.md`.** H.34 step 3, the G.19 row, the Gamepad rig limitations and the STOP rig limitations carry "since Story 4.9" notes (Shift+Tab from nothing focused reaches STOP; a press on the pad takes the panel's focus).
- [x] **10 G.1 full-layout hint on the real main screen.** `g1_gamepad_hint_geometry` adds 1320×800 (full width inside the kittest screen's margins); the G.1 row gives the per-width panel size with a "(Story 4.9)" clause.
- [x] **11 Flake hardening.** `gamepad_keys_app.rs`: `LIMIT` 10 s, log markers numbered by an `AtomicU64`. `gamepad_focus.rs` has no thread waits or markers.
- [x] **Defer: phantom press after Cmd+Tab refocus** added to Accepted limits (here and EXPERIENCE.md).
- [x] **Defer: tooltip delay 0** in the G.24 tooltip harness (`all_styles_mut`).

## Dev Agent Record

### Agent Model Used

Claude Opus 5.5 (claude-opus-5-5), gds-dev-story, 2026-11-28.

### Debug Log References

**Owner decisions (2026-11-28, "ok" to the Open questions; recorded as decided by the owner):** 1 H.36 runs now, in this story (about 25 minutes on macOS with OoT running; the bench itself is the owner's Task 12, still open); 3 Tab locks into the pad panel: Tab enters, Tab/Shift+Tab/arrows stay, leave with Esc or a click, STOP reached with Shift+Tab from nothing focused; 4 after Cmd+Tab the panel needs a click to reactivate the keys (window blur drops panel focus); 7 compact layout = buttons only, no sticks; 8 key hints in hover tooltips; 6 Cmd/Ctrl+key stays for app shortcuts and never presses the pad (Shift and Alt/Option do not block); 10, 11 copy and the focused look (ring, wash, chip) are drafts tuned at the owner eye check (tokens only). The other questions take the story's stated defaults: 2 key tap minimum 50 ms (same as the mouse); 5 focus loss → `SET_STATE` NEUTRAL; 9 compact below 1280 pt, region 200 pt (`[ASSUMPTION]` dropped from `spacing.gamepad-compact-width`); 12 new `typography.chip` (11.5 px, 700, no tracking); 13 chip and ring shown while gated, each fresh key press `pad ignored … key=`; 14 mapped keys consumed while focused; 15 Shift/Alt+Esc = STOP with the panel keeping its focus; 16 the Log lines format; 17 M5 stays TODO with the macOS evidence in its cell; 18 numbers without an instrument accepted as listed; 19 H.36 on Windows/Linux deferred to issue #43; 20 Joy-Cons untouched in steps 1–13, Backspace (−) pressed in step 11.

**Implementation notes.**
- Model (Task 1, `src/gamepad.rs`, still no egui): `PadKey` (18, `ALL` in key-table order, `control()`, `log_name()`, `hint()`); `Control::key_hint()` builds "A: L key" / "D-pad up: W or Up arrow" / "Home: mouse only" from `PadKey::ALL`, so the tooltips cannot disagree with the table. Key holds are a `Vec<KeyHold { key, since_us, release_at }>` beside the mouse hold (capacity 18, no per-frame allocation); `key_down` ignores a key already held (no line), `key_up` releases at once (`key_up`) or at `since + TAP_MIN_US` (`tap`), ignores a key not held. `state()` is the union: one `apply_all` over the mouse hold and every key hold (buttons OR-ed, the hat from `hat(any up, any down, any left, any right)`); `is_active()` counts key holds; `free()` stays mouse-only (a key hold never makes a mouse press `busy`). `Release` gains `KeyUp`, `FocusLost` (sends the new state like `WindowBlur`) and `Layout`; `release_all` ends every key hold first, each with its own `release … key=<k>` line; `end_drag_for_layout()`; `next_wake_us` takes the earliest key deadline. `Source::Keyboard`; `press_line`/`release_line`/`ignored_line` take `Option<PadKey>`; `KeysOn`/`KeysOff` and `keys_on`/`keys_off`/`ignored_key`/`ignored` push their lines into the pad's queue, so every `pad` line of a frame keeps its order.
- Tokens (Task 2): `typography.chip`; `components.gamepad-panel-focused.{ring-width, wash-steps, wash-step}`, `components.gamepad-compact.{grid-width, grid-height, caption-lines}`, `components.chip-keys-active.{typography, padding-x, padding-y, gap-bottom}`; `theme::CHIP` and `theme::components::{gamepad_panel_focused, gamepad_compact, chip_keys_active}`; G.3 `g3_gamepad_focus_and_compact_tokens` (also checks that `gamepad-compact-width` carries no `[ASSUMPTION]` any more).
- Compact layout (Task 3): `layout::COMPACT_BELOW` (1280) and `layout::is_compact(content)`; `regions` gives 200 pt below it. `ui::gamepad::Variant::of(region)` is compact when the region is narrower than the full panel (`regions` gives 200 pt exactly when `is_compact`, so both agree; at tiny windows the region is narrower still and stays compact). `compact_layout(region, text)` (pure), `PadGeometry::compact(origin)` (Dev Notes › Compact geometry; `has_sticks`/`has_body` false, zero-radius wells never hit or drawn, `stick_wells()` empty), the D-pad cross outline and Home's house now come from the geometry (`cross_outline`, `house_at`) so both variants share the painting code. `gamepad::hint_rect(region, text)` picks the variant's hint; `main_screen::hint_rect` uses it. Tooltips: `Tooltip::for_enabled` + `theme::tooltip_frame()` with `Control::key_hint()` in `typography.body`, hover only.
- Focus, look, group (Task 4): `panel_id()` = `Id::unique("gamepad-panel-focus")` (egui 0.37 deprecates `Id::new`); `show` interacts the region with `Sense::FOCUSABLE` first and gives it `WidgetInfo::labeled(Role::Group, …)` ("Gamepad – keys active" / "Gamepad – keys off"), paints the focused look (six inset wash strokes `amber-wash.gamma_multiply((6 − i)/6)` at insets 2 + 3·i, 3 pt wide, then the 2 pt inset ring) right after the region fill, then builds everything else in `scope_builder(UiBuilder::new().max_rect(region).accessibility_parent(panel.id))`. The chip is painted in the header (no node) through `chip_rect` (right edge at the inner right, bottom 3 pt above the underline). `PadInput` gains `panel_press` (a primary press that reached the panel: a control or stick response, or the panel's own `contains_pointer`, which a modal above blocks) and `variant`. `PadView` gains `key_held` (a bit per control held by a key) so a key hold draws its control pressed like a mouse hold (Story 4.8's held look; needed for the eye check, no new look).
- Keys (Task 5): `ui::pad_keys::{pad_key, take, KeyEdge}`; `take` maps by `physical_key` only and removes the mapped events without Cmd/Ctrl from `InputState::events` (`retain`); `keys::command_held` is `pub(crate)` and shared.
- App (Task 6): new fields `panel_focused`, `panel_pointer_down`, `lock_event_last_frame`, `tab_last_frame`, `panel_surrender_pending`; `pad_input(ctx, …)`: STOP release → spend → `panel_focus` (keep / gain / fallback / loss and the `keys=` lines, `release_all(FocusLost)` unless STOP already released) → gate and pointer (Story 4.8) → keys (only with focus at the end of the previous first pass and now, never in a STOP or blur frame; repeats and Cmd/Ctrl presses skipped; gated → `pad ignored … key=`) → the lock filter, set last → the layout-switch drag end. `pad_background` (also `logic`-only frames) on blur writes `keys=off reason=window_blur` after the release lines and marks the panel's focus for surrender, applied right after in `logic` (and at the start of `pad_input` as a fallback), so the `App::pad_background` test hook stays usable without a context. One repaint is requested after every focus gain. `App::panel_focused()` test hook.

**Deviations and choices beyond the story text (each kept small; the owner can overrule):**
- D12 refined: egui applies a Shift+Tab move one frame late (`id_next_frame`) and an arrow move at the end of the pass, so the lock fallback also pulls the focus back in the frame after a Tab/arrow event that arrived while the panel had focus (`lock_event_last_frame`), still with no line and only without another loss cause (no press or click, no unmodified Esc, no dialog, no blur). For the same reason a Shift+Tab gain that lands a frame late is still `keys=on source=tab` (`tab_last_frame`).
- D11 refined: a press that reaches the panel gives it focus; the release of such a press only keeps a focus the panel already had (so a mouse hold released after an Esc does not take the focus back). No focus is given in a STOP or blur frame (the press is spent there; `g23_esc_and_press_same_frame` stays as it was) and never while a dialog is open.
- Blur line order: `release … reason=window_blur key=<k>` lines, then `keys=off reason=window_blur` (as H.36 step 10 lists them); the "keys=off before the release lines it causes" rule of Dev Notes › Log lines applies to `elsewhere` and `dialog` (`keys=off`, then `release … reason=focus_lost`).
- The gated mouse `ignored` line now goes through the pad's line queue (`Pad::ignored`) instead of straight to the log, so a press that focuses the panel while gated writes `keys=on source=click` before its `ignored` line.
- AC 6a′ with the firmware dialog: that dialog opens only on a new connection, when the gate is already blocked (no session before it), so nothing can be held when its `keys=off reason=dialog` comes; G.25 checks the release order with a generic dialog requested through `App::modals_mut()` and the firmware dialog's `keys=off reason=dialog` with nothing held (the "which line comes first" question of the AC table: the gate's release and STOP always come first, in an earlier frame).
- The lock filter's red proof needed an observable: with the fallback in place a Tab that leaves is pulled back in the same frame, so G.25 `g25_focus_lock` also checks that no other widget paints its focus ring in the frames after the key (later, and right after a click that re-requests the focus); without the filter, or with the filter set before the `request_focus`, a ring shows for a frame (red proofs below).
- `TextHeights` gains `chip` (the chip text's size); G.22's literal helper adds it.
- Smoke run (`cargo run -p ocb-app -- --smoke-test`, `--gallery`) **not run locally**: a `bonooru` process from the previous eye check was running (PID 11527, `./target/release/bonooru`), and the session rule (issue #45) forbids a second instance; no bridge was listed (`ls /dev/cu.usbmodem*` empty). CI's smoke run (G.7) covers it on the three OS.

**Test changes to existing checks (AC 13; each deliberate, none deleted).**
- G.1 (`layout.rs`): `g1_sizes_match_the_tokens` expects 200 pt below 1280 and the note view `w − 260 − gamepad`; sizes gain 1279×800; new `g1_compact_threshold`. Reason: UX-DR6 compact width. `main_screen.rs` `g1_gamepad_hint_geometry`: the hint is 8 pt under the compact grid where the region is compact (the kittest screen's content is the window less its margins, so even its "1280" window is compact: decided by `Variant::of(pad)`, with an assert that it matches the width).
- G.10 (`a11y.rs`) `g10_gamepad_nodes`: one `Group` node; a 200 pt panel has 16 `Button` and 0 `Slider`. Reason: AC 10.
- G.19 (`bottom_bar.rs` `g19_stop_focus_ring`) and G.21 (`button_states.rs` `g21_stop_states`): STOP reached by one Shift+Tab instead of Tab × 4. Reason: Tab × 4 is the gamepad panel now (AC 11).
- G.20 (`stop_app.rs`): `Rig::focus_stop` is one Shift+Tab (used by `g20_space_never_clicks`, `g20_click_enter_accesskit`, `g20_h34_dry_run` step 3, renamed "Shift+Tab: STOP; Space"); `g20_tab_order` rewritten to AC 11: LED, LED, LED, panel, panel, panel; Shift+Tab stays; Esc → nothing focused (one `stop requested source=esc`); Shift+Tab → STOP; 100 idle frames → no new `stop requested` (counting only the app's requests: the first local run flaked once when the worker's own line for that Esc arrived during the idle frames).
- G.22 (`gamepad.rs`): the full-pad `SIZES` are 1280×800 and 1920×1080 (below 1280 the region is the compact grid, checked by G.24); `g22_accesskit` also expects one `Group` node; `heights()` adds `chip`.
- G.23 (`gamepad_app.rs`): `g23_click_is_a_tap` and `g23_stick_drag_and_spring` expect `keys=on source=click` before the first press line; `g23_blocked_clicks_send_nothing` expects `[keys=on source=click, ignored …]`; `g23_stop_drops_held` (by button) activates STOP by its AccessKit click and asserts that a press takes the panel's focus (replacing "a press keeps STOP's focus"; Enter would now press +); `g23_h35_dry_run` step 7 is "hold A, STOP by AccessKit click" (historical note in the test and under H.35 in the test plan); the pad line matcher moved to `tests/common::is_app_pad_line` and accepts the keyboard and focus lines (`g23_pad_lines`' rejected samples still rejected: `press control=a source=keyboard` without a key is invalid).
- G.15, G.17, G.18 needed no change (G.18's Tab loop in `firmware_app.rs` stays inside the dialog; G.15/G.17 do not depend on the gamepad width). C.23–C.29 unchanged (no worker change).

**Red proofs (E3-A2; break → fail → restore, each on a fresh build, restored and green after).**
Model (unit tests in `gamepad.rs`):
- `key_down` without the "already held" check (repeats not skipped) → `key_tap_hold_and_repeat` red ("assertion failed: !p.is_pressed(Control::A)": the second press adds a hold that outlives the release).
- `key_up` releasing at once (no `TAP_MIN_US`) → `key_tap_hold_and_repeat` red ("held for the tap minimum").
- `state()` from the last key only → `key_chords_duplicates_opposites` red (`buttons` 0x0008, want 0x000c).
- release by control instead of by key (a key up drops every hold of its control) → `key_chords_duplicates_opposites` red ("ArrowUp still holds up"). (A first mutation that only picked the first hold of the control stayed green, since it removes the same hold here; the stronger mutation is the real bug class.)
- hat from the last direction only → `key_chords_duplicates_opposites` red ("opposites cancel").
- `release_all` skipping key holds → `release_all_with_keys` red ("window_blur": no release lines).
- mouse `free()` counting key holds → `keys_and_mouse_union` red ("a key hold is not busy").

G.24 (`gamepad_focus.rs`), G.3, G.1:
- `paint_focus_look` called without focus → `g24_focused_look` red ("1280x800 ppp 1: no focused look without focus").
- the chip moved 20 pt left in the 180 pt panel → `g24_focused_look` red ("1024x680 ppp 1: chip [[918 54] - [994 72]] ∩ tab [[834 54] - [923 75]]").
- compact A 2 pt off (x 155) → `g24_compact_geometry` red ("A").
- stick wells drawn in compact (`has_sticks: true`, r 17 wells) → `g24_compact_contents` red ("no stick wells or knobs").
- tooltip text = the AccessKit name instead of `key_hint()` → `g24_compact_tooltips` red (`"A"` vs `"A: L key"`).
- `accessibility_parent` dropped → `g24_accesskit_group` red ("Button Some("ZL") not under the group").
- the group named "keys active" without focus → `g24_accesskit_group` red (name mismatch).
- `pad_key` mapping `Key::X` → `g24_pad_keys_table` red ("X").
- `chip_keys_active::PADDING_X` 7 → `g3_gamepad_focus_and_compact_tokens` red ("token mismatch"); `[ASSUMPTION]` put back on `gamepad-compact-width` → the same test red.
- `COMPACT_BELOW` 1279 → `g1_sizes_match_the_tokens` ("1279x800") and `g1_compact_threshold` ("1279.75 at 0") red.

G.25 (`gamepad_keys_app.rs`):
- keys taken without focus (no focus condition) → `g25_focus_frames` red (`press … key=l` and `key=enter` lines while unfocused).
- keys taken in the frame focus is gained (only "focused now") → `g25_focus_frames` red.
- Cmd not checked → `g25_modifiers_and_consumption` red (`press control=a … key=l`, `press control=plus … key=enter`).
- mapped events not consumed → `g25_modifiers_and_consumption` red ("a plain L is consumed").
- `physical_key: None` mapped through the logical key → `g25_every_key_bit` red (18 press lines for events without a physical key).
- no re-request of the focus at the release of a press on a control → `g25_focus_gain_and_loss` red ("focus kept at the release" / `panel_has_focus()`).
- the lock filter set before `request_focus` (start of the frame) → `g25_focus_lock` red ("Tab … after a click, frame 0": STOP painted its ring for a frame).
- no lock filter at all → `g25_focus_lock` red ("Tab … later, frame 0").
- no lock fallback → `g25_focus_lock` red ("Tab Modifiers::NONE by_click=true: first frame").
- no `surrender_focus` on blur → `g25_blur_releases_keys` red (focus still the panel after the blur).
- focus loss not releasing → `g25_focus_loss_releases` red ("controller got [A], want 2").
- STOP not releasing key holds (`release_all` skips keys for `Stop`) → `g25_stop_releases_keys` red (no `pad release control=a reason=stop key=l`).
- gate check skipped for keys → `g25_gated_keys` red ("worker": a press instead of the `ignored … key=l` line).
- drag not ended on the layout switch → `g25_layout_switch` red (`dragging()` still `Some`).
- repeats not skipped (app and model) → `g25_key_repeat_ignored` red ("still held").
- key release without `TAP_MIN_US` → `g25_key_tap_and_hold` red (NEUTRAL in the press frame).
- the dry run itself: repeats not skipped → `g25_h36_dry_run` red (step 5 "still held"); focus loss not releasing → red (step 9); no surrender on blur → red (step 10: an `L` press after coming back wrote `pad` lines); `state()` from the last key only → red (step 6 `[0x0004, 0x0008, …]`).

**Instrument review (Task 9, E3-A2/E3-A3, before any bench result).** No bench tool is new or changed (`ocb-probe`, `ocb-dev`, `link_probe.py` untouched); H.36's instrument is the `pad` (including `keys=`), `mode`, `stop`, `gate` and `led` lines plus the owner's eyes and ears. Field → line: press latency `+ms(pad sent) − +ms(pad press)` (app `press_line(…, Keyboard, Some(k))` written before the `try_send`, worker `pad sent`); tap length `+ms(sent release) − +ms(sent press)` (worker `pad sent` pair; `release … reason=tap key=l` says the release waited for `TAP_MIN_US`); key-up latency `+ms(sent release) − +ms(pad release)` (`release … reason=key_up`); one press per held key (one `pad press`, one `pad sent` between press and `key_up` release; host: `g25_key_repeat_ignored` vs the repeats-not-skipped proof); chords / duplicates / opposites (the `pad sent` sequences; host: `g25_chords_and_union` vs the last-key / by-control / last-direction proofs); Esc (`stop requested`, `release … reason=stop key=l`, `keys=off reason=esc`, worker `stop sent/acked/after`, no `pad sent` after; host: `g25_stop_releases_keys` vs the STOP-skips-keys proof); click elsewhere (`keys=off reason=elsewhere`, `release … reason=focus_lost key=l`, NEUTRAL; host: `g25_focus_loss_releases` vs the no-release proof); Cmd+Tab / Cmd+H (`release … reason=window_blur key=l`, `keys=off reason=window_blur`, NEUTRAL; afterwards an L writes no `pad` line; host: `g25_blur_releases_keys` vs the no-surrender proof); lock (no `keys=off` through Tab × 3 and Shift+Tab; host: `g25_focus_lock` vs the no-filter / no-fallback proofs); Space and Cmd+Enter silent (host: `g25_global_keys`, `g25_modifiers_and_consumption` vs the Cmd-not-checked proof); gated keys (`pad ignored control=a reason=no_bridge key=l`, no `pad sent`; host: `g25_gated_keys` vs the gate-skipped proof); compact (`pad sent buttons=0x0004` for a click on A and for L at about 1100 pt; host: `g25_layout_switch`, G.24). The rig block's checklist was checked against the runbook (items 1–5 match H.36 steps 1, 12, 14, 10/13 and the record table) and the Bench gotchas re-read; `g23_h35_dry_run` is still green (its step 7 updated, see above).

**Dry run (Task 9): `cargo test -p ocb-app --test gamepad_keys_app -- --nocapture h36_dry_run`, green on the working tree of this commit (fake clocks, so `after_us` is in 16 ms frame steps; the `unknown` LED lines and `port_listed` lines left out). To repeat on the bench commit (rig block item 5):**

```
H.36 setup (start, three LEDs ok, gate on):
  port_opened port=/dev/cu.usbmodem2101 class=ours serial=53032847189C2E9C
  gate gamepad=off gamepad_reason=bridge_not_ready play=off play_reason=bridge_not_ready controller_proto=- app_proto=-
  led pc_bridge=ok name="Bridge – found" reason=ours_open status_age_us=- session_age_us=-
  gate gamepad=off gamepad_reason=controller_link play=off play_reason=controller_link controller_proto=- app_proto=-
  version connected port=/dev/cu.usbmodem2101 proto=1 fw=0.0.0 bridge=0.0.0 hid=P2 queue_cap=512 app_timeout_ms=1000 report_period_us=8000
  led pc_bridge=ok name="Bridge v0.0.0" reason=info status_age_us=- session_age_us=-
  led bridge_controller=ok name="Controller v0.0.0" reason=info status_age_us=16000 session_age_us=-
  led switch=ok name="Switch – ready" reason=console_ready status_age_us=- session_age_us=-
  gate gamepad=on gamepad_reason=none play=on play_reason=none controller_proto=- app_proto=-
H.36 step 1a (click an empty spot of the panel):
  pad keys=on source=click
H.36 step 1b (Esc):
  stop requested source=esc dialog=none command=queued
  pad keys=off reason=esc
  stop sent port=/dev/cu.usbmodem2101
  stop acked port=/dev/cu.usbmodem2101 after_us=48000 by=command
  stop after port=/dev/cu.usbmodem2101 mode=idle abort_count=0
H.36 step 1c (Tab four times):
  pad keys=on source=tab
H.36 step 2 (quick L):
  pad press control=a source=keyboard key=l
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0004 hat=8 lx=128 ly=128 rx=128 ry=128
  mode port=/dev/cu.usbmodem2101 from=idle to=direct abort_count=0
  pad release control=a reason=tap key=l
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0000 hat=8 lx=128 ly=128 rx=128 ry=128
H.36 step 3 (quick ArrowLeft):
  pad press control=dleft source=keyboard key=arrow_left
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0000 hat=6 lx=128 ly=128 rx=128 ry=128
  pad release control=dleft reason=tap key=arrow_left
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0000 hat=8 lx=128 ly=128 rx=128 ry=128
H.36 step 4 (quick l):
  pad press control=a source=keyboard key=l
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0004 hat=8 lx=128 ly=128 rx=128 ry=128
  pad release control=a reason=tap key=l
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0000 hat=8 lx=128 ly=128 rx=128 ry=128
H.36 step 4 (quick k):
  pad press control=x source=keyboard key=k
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0008 hat=8 lx=128 ly=128 rx=128 ry=128
  pad release control=x reason=tap key=k
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0000 hat=8 lx=128 ly=128 rx=128 ry=128
H.36 step 4 (quick h):
  pad press control=y source=keyboard key=h
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0001 hat=8 lx=128 ly=128 rx=128 ry=128
  pad release control=y reason=tap key=h
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0000 hat=8 lx=128 ly=128 rx=128 ry=128
H.36 step 4 (quick q):
  pad press control=l source=keyboard key=q
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0010 hat=8 lx=128 ly=128 rx=128 ry=128
  pad release control=l reason=tap key=q
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0000 hat=8 lx=128 ly=128 rx=128 ry=128
H.36 step 4 (quick e):
  pad press control=r source=keyboard key=e
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0020 hat=8 lx=128 ly=128 rx=128 ry=128
  pad release control=r reason=tap key=e
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0000 hat=8 lx=128 ly=128 rx=128 ry=128
H.36 step 5 (hold L about 3 s, key repeat on):
  pad press control=a source=keyboard key=l
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0004 hat=8 lx=128 ly=128 rx=128 ry=128
  pad release control=a reason=key_up key=l
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0000 hat=8 lx=128 ly=128 rx=128 ry=128
H.36 step 6 (chord: hold L, then K; release K, then L):
  pad press control=a source=keyboard key=l
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0004 hat=8 lx=128 ly=128 rx=128 ry=128
  pad press control=x source=keyboard key=k
  pad sent port=/dev/cu.usbmodem2101 buttons=0x000c hat=8 lx=128 ly=128 rx=128 ry=128
  pad release control=x reason=key_up key=k
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0004 hat=8 lx=128 ly=128 rx=128 ry=128
  pad release control=a reason=key_up key=l
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0000 hat=8 lx=128 ly=128 rx=128 ry=128
H.36 step 7 (Enter; hold W, then ArrowUp, release W, then ArrowUp; hold ArrowLeft, then D, release ArrowLeft, then D; Enter):
  pad press control=plus source=keyboard key=enter
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0200 hat=8 lx=128 ly=128 rx=128 ry=128
  pad release control=plus reason=tap key=enter
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0000 hat=8 lx=128 ly=128 rx=128 ry=128
  pad press control=dup source=keyboard key=w
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0000 hat=0 lx=128 ly=128 rx=128 ry=128
  pad press control=dup source=keyboard key=arrow_up
  pad release control=dup reason=key_up key=w
  pad release control=dup reason=key_up key=arrow_up
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0000 hat=8 lx=128 ly=128 rx=128 ry=128
  pad press control=dleft source=keyboard key=arrow_left
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0000 hat=6 lx=128 ly=128 rx=128 ry=128
  pad press control=dright source=keyboard key=d
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0000 hat=8 lx=128 ly=128 rx=128 ry=128
  pad release control=dleft reason=key_up key=arrow_left
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0000 hat=2 lx=128 ly=128 rx=128 ry=128
  pad release control=dright reason=key_up key=d
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0000 hat=8 lx=128 ly=128 rx=128 ry=128
  pad press control=plus source=keyboard key=enter
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0200 hat=8 lx=128 ly=128 rx=128 ry=128
  pad release control=plus reason=tap key=enter
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0000 hat=8 lx=128 ly=128 rx=128 ry=128
H.36 step 8 (hold L, Esc, release L; a new L press):
  pad press control=a source=keyboard key=l
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0004 hat=8 lx=128 ly=128 rx=128 ry=128
  stop requested source=esc dialog=none command=queued
  pad release control=a reason=stop key=l
  pad keys=off reason=esc
  stop sent port=/dev/cu.usbmodem2101
  stop acked port=/dev/cu.usbmodem2101 after_us=48000 by=command
  mode port=/dev/cu.usbmodem2101 from=direct to=idle abort_count=0
  stop after port=/dev/cu.usbmodem2101 mode=idle abort_count=0
H.36 step 9 (click the panel, hold L, click the note view, release L):
  pad keys=on source=click
  pad press control=a source=keyboard key=l
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0004 hat=8 lx=128 ly=128 rx=128 ry=128
  mode port=/dev/cu.usbmodem2101 from=idle to=direct abort_count=0
  pad keys=off reason=elsewhere
  pad release control=a reason=focus_lost key=l
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0000 hat=8 lx=128 ly=128 rx=128 ry=128
H.36 step 10 (click the panel, hold L, Cmd+Tab away, release L, back; an L press):
  pad keys=on source=click
  pad press control=a source=keyboard key=l
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0004 hat=8 lx=128 ly=128 rx=128 ry=128
  pad release control=a reason=window_blur key=l
  pad keys=off reason=window_blur
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0000 hat=8 lx=128 ly=128 rx=128 ry=128
H.36 step 10b (click the panel, hold L, Cmd+H, release L, back; an L press):
  pad keys=on source=click
  pad press control=a source=keyboard key=l
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0004 hat=8 lx=128 ly=128 rx=128 ry=128
  pad release control=a reason=window_blur key=l
  pad keys=off reason=window_blur
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0000 hat=8 lx=128 ly=128 rx=128 ry=128
H.36 step 11a (click the panel, Tab x3, Shift+Tab, Space, Cmd+Enter):
  pad keys=on source=click
H.36 step 11b (Backspace):
  pad press control=minus source=keyboard key=backspace
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0100 hat=8 lx=128 ly=128 rx=128 ry=128
  pad release control=minus reason=tap key=backspace
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0000 hat=8 lx=128 ly=128 rx=128 ry=128
H.36 step 12a (bridge out, L):
  port_lost port=/dev/cu.usbmodem2101 reason="the port left the port list"
  port_vanished port=/dev/cu.usbmodem2101
  led pc_bridge=error name="Bridge – not found" reason=not_found status_age_us=- session_age_us=-
  gate gamepad=off gamepad_reason=no_bridge play=off play_reason=no_bridge controller_proto=- app_proto=-
  pad ignored control=a reason=no_bridge key=l
H.36 step 12b (bridge back, quick L):
  port_opened port=/dev/cu.usbmodem2101 class=ours serial=53032847189C2E9C
  led pc_bridge=ok name="Bridge – found" reason=ours_open status_age_us=- session_age_us=-
  gate gamepad=off gamepad_reason=controller_link play=off play_reason=controller_link controller_proto=- app_proto=-
  version connected port=/dev/cu.usbmodem2101 proto=1 fw=0.0.0 bridge=0.0.0 hid=P2 queue_cap=512 app_timeout_ms=1000 report_period_us=8000
  led pc_bridge=ok name="Bridge v0.0.0" reason=info status_age_us=- session_age_us=-
  led bridge_controller=ok name="Controller v0.0.0" reason=info status_age_us=32000 session_age_us=-
  led switch=ok name="Switch – ready" reason=console_ready status_age_us=- session_age_us=-
  gate gamepad=on gamepad_reason=none play=on play_reason=none controller_proto=- app_proto=-
  pad press control=a source=keyboard key=l
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0004 hat=8 lx=128 ly=128 rx=128 ry=128
  mode port=/dev/cu.usbmodem2101 from=idle to=direct abort_count=1
  pad release control=a reason=tap key=l
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0000 hat=8 lx=128 ly=128 rx=128 ry=128
H.36 step 13 (about 1100 pt: click A, quick L; back to 1280):
  pad press control=a source=mouse
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0004 hat=8 lx=128 ly=128 rx=128 ry=128
  pad release control=a reason=tap
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0000 hat=8 lx=128 ly=128 rx=128 ry=128
  pad press control=a source=keyboard key=l
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0004 hat=8 lx=128 ly=128 rx=128 ry=128
  pad release control=a reason=tap key=l
  pad sent port=/dev/cu.usbmodem2101 buttons=0x0000 hat=8 lx=128 ly=128 rx=128 ry=128
```

**Host verification (Task 11, partial).** In `app/` with `/opt/homebrew/opt/rustup/bin` first on `PATH`: `cargo fmt --all --check` ok; `cargo clippy --locked --workspace --all-targets -- -D warnings` ok; `cargo test --locked --workspace` twice, 530 passed / 0 failed each run; `cargo deny --locked check licenses bans` ok; `gui/ci/check-accesskit.sh` ok for the three targets; `cargo tree -e normal --target all -i ocb-sim` empty for `ocb-app`, `ocb-services`, `ocb-core`; eframe v0.37.0. Not done here: the smoke runs (see Deviations), the push and the `app` workflow on three OS (for the coordinator).

**Review fixes (2026-11-28).** Decisions D-1 (blur log order kept; Log lines amended) and D-2 (firmware dialog: the gate is already blocked when it opens, so nothing can be held; the generic dialog measures the release order) were taken by the session lead. Patches 1–11 and both defers as listed in Review Findings. The H.36 runbook steps and the dry run's expected lines are unchanged (re-run: identical except the fake clock's `after_us` quantum in step 1b). Host gates after the fixes: fmt ok, clippy `-D warnings` ok, `cargo test --locked --workspace` twice 539 passed / 0 failed, `cargo deny` ok, accesskit check ok, no `ocb-sim` in the normal graphs. Waits in G.25 are now bounded at 10 s (the AC table's "≤ 2 s" polls are 10 s bounds; no upper bound on the code under test).

Red proofs of the review fixes (break → red → restore):
- P1 `pad_background_keys` ignores its key-ups → `g25_key_up_in_a_logic_only_pass` red ("released without ui").
- P2 `key_down` ignores a pending hold again → `key_repress_in_the_tap_window` red ("held again, the tap release cancelled").
- P3 no surrender when a dialog is open → `g25_dialog_takes_the_keys` red ("disabled primary false: no +": the same-frame Enter pressed +).
- P4 fallback also on an AccessKit focus frame → `g25_accesskit_focus_leaves_the_panel` red (focus pulled back from STOP); the review's literal "repeat == false" rule → `g25_focus_lock_held_arrow` red ("the held arrow did not take the focus": the focus left), hence the deviation in Review Findings 4. The modifier rule has no observable red proof: a modified arrow or Tab never moves egui's focus, so the fallback is not reached through it (no widget requests focus programmatically).
- P5 keys consumed only when applied → `g25_keys_consumed_in_every_focused_frame` red ("consumed in the gain frame").
- P6 the panel's variant from `width < COMPACT_BELOW` (no slack: a second threshold) → `g24_one_compact_threshold` red ("1279.6 pt at ppp 1.1: the panel drawn matches the region"); `is_compact` without the slack → `g1_compact_threshold` red ("1279.6 at 0").
- P7 only the primary button counts on the panel → `g25_other_buttons_keep_focus` red ("Secondary at [1202 209]": focus lost); any button presses a control (press and origin) → red ("B never pressed"). (A first mutation changing only `primary_pressed` stayed green because the press origin is still read from primary events; the stronger mutation is recorded.)
- `g24_compact_layout`: grid 8 pt under the caption → red. `g24_compact_drawn_inside`: the grid drawn 30 pt right → red (the ABXY well outside the region). (A caption wrapped 60 pt wider stayed green: the caption fits one 180 pt line at 12.5 pt, so it never reaches the box edge.) `g24_no_online_wording`: "keys offline" in the group name → red.
- `g25_dialog_blocks_focus`: panel press from the region rect (through the modal) and no dialog check → red (`keys=on source=click`, `keys=off reason=dialog`). `g25_chords_and_union`: a key hold makes the mouse busy → red. `g25_block_and_idle_with_keys`: the gate release skips key holds → red (no `release … reason=gate key=l`). (Making `is_active` ignore key holds stayed green: `last_sent` ≠ NEUTRAL keeps the pad active, which is intended.) `g25_global_keys`: Enter unmapped → red; Space not consumed and mapped to A → red. `g25_pad_lines`: ` key:` for ` key=` → red.
- Units: L pressing X (K pressing A) → `key_table` red ("K"); L's hint "L" → `key_hints` red; key deadlines left out of `next_wake_us` → `next_wake_with_key_deadlines` red; `release_all` skipping keys for `ControllerIdle` → `controller_idle_drops_key_holds` red.
- H.36 verdicts: `source=click`/`source=tab` swapped → `g25_h36_dry_run` red (step 1a "pad keys=on source=tab") and `g25_focus_gain_and_loss` red; Space not consumed and mapped to A → `g25_h36_dry_run` red (step 11a: `pad press … key=l` after Space); compact A at B's position → `g25_compact_presses` red ("controller got [], want 2"); keys ignored in the compact variant → `g25_compact_presses` red (only the mouse A sent).

**CI flake fixes (tests only, no app code changed after `7164e29`).** `dc280e9`: the G.23 and G.25 settle helpers also wait until the log has a `pad sent` line for every DIRECT state (on the slow macOS runner the controller can hold the last state before the worker's `pad sent` line is in the log); `g25_h36_dry_run` failed on macos-latest at `7164e29`. `f102f45`: the G.3 gallery harness allows 32 steps (`max_steps`) for the tall gallery (the fake worker's repaint can land after kittest's default 4 steps on a slow runner); `g3_every_focusable_node_shows_its_ring` failed on windows-latest at `dc280e9` ("would have settled after 6"). The smoke runs not run locally (see Deviations) are covered by CI's G.7 on the three OS. **CI:** the `app` workflow on `f102f45` green on ubuntu, windows and macos (run 46136020542, 2026-11-28).

**H.36 (Task 12), macOS PASS 2026-11-28** (owner present; release `bonooru` built at `7164e29`, `h36_dry_run` green on it; `dc280e9` and `f102f45` change only tests; macOS 27.2.1, Mac17,9; built-in keyboard, input source ABC (`com.apple.keylayout.ABC`), key repeat at the system defaults (no `InitialKeyRepeat`/`KeyRepeat` override); bridge Story 3.1 `69f4d2c` `fw-release` at `/dev/cu.usbmodem101`, controller `fw-debug-o3`, `proto` 1, in the Switch's top USB-C port; Switch 2 23.5.0, OoT 1.1.1, Link in ocarina free play with the Joy-Con connection screen still open; log `~/Library/Logs/Bonooru/bonooru.log`, run pid 35260, `start` 2026-11-28T10:24:02Z; the three LEDs ok and `gate gamepad=on` at +80 ms). Every reply the owner typed in the terminal blurred the window (`pad keys=off reason=window_blur`), so each step began with a click on the panel or Tab. Every step passed against the runbook: 1 click on empty panel space → `keys=on source=click`, no press (owner: ring, wash, chip); click + Esc → `stop requested source=esc dialog=none command=queued`, `keys=off reason=esc`, `stop acked after_us=1873 by=command`, `stop after mode=idle`, ring and chip gone; Tab × 4 from a click outside → `keys=on source=tab`. 2 the owner first clicked the mouse L shoulder twice (0x0010) and mouse A, which closed the connection screen (keyboard L closes it as well; not a defect); `mode from=idle to=direct` 5 ms after the first press; keyboard L → `pad press control=a source=keyboard key=l`, sent 0x0004 1 ms later, `release reason=key_up` after about 594 ms, 0x0000 1 ms later, no `nak`, the game reacted once. 3 ArrowLeft → `hat=6`, `hat=8`; ocarina out. 4 L, K, H, Q, E → 0x0004, 0x0008, 0x0001, 0x0010, 0x0020, each followed by 0x0000, each sent within 1 ms; five notes: **M5 keyboard exit met on macOS**. 5 first hold 2447 ms (below the 2500–4000 window), repeated: one press, one 0x0004, `key_up` after 3816 ms, one 0x0000, nothing between (OS repeats ignored), one continuous note. 6 chord → 0x0004, 0x000c, 0x0004, 0x0000 (heard L, K, L, silence). 7 the pause menu does not open in free play: left with J (0x0002), then Enter 0x0200 / 0x0000 opened it (runbook gotcha added); W + ArrowUp → one `hat=0`, no sent for the second press, `hat=8` only at the last release (released in the opposite order to the runbook over several tries: the same expected wire; owner: "D-pad up stays until both are released"); ArrowLeft + D → `hat=6`, `hat=8`, `hat=2`, `hat=8` (owner: "right cancels left"); Enter closed the menu, ArrowLeft brought the ocarina back. 8 Esc with L held → `stop requested source=esc dialog=none command=queued`, `pad release control=a reason=stop key=l`, `keys=off reason=esc`, `stop sent`, `stop acked after_us=1865 by=command`, `mode from=direct to=idle`, `stop after mode=idle`; no `pad sent` after the Esc; the L release and a new L press wrote nothing until the panel was focused again. 9 click elsewhere with L held → `keys=off reason=elsewhere`, `release … reason=focus_lost key=l`, 0x0000, same ms; the note stopped at the click. 10 Cmd+Tab with L held → `release … reason=window_blur key=l`, `keys=off reason=window_blur`, one 0x0000; back in Bonooru L silent, ring gone; 10b Cmd+H → the same lines. 11 click, Tab × 3, Shift+Tab → no `keys=off`, the ring stayed (owner: Tab/Shift+Tab "no longer work" on the panel: the approved D3 design); Space and Cmd+Enter → no `pad` or `stop` line; Backspace → `pad press control=minus … key=backspace`, 0x0100 / 0x0000 (the map opens only outside free play). 12 unplug with the panel focused → `port_lost reason="read: Broken pipe"`, `gate … bridge_not_ready`, `port_vanished`, `gamepad_reason=no_bridge`; L → `pad ignored control=a reason=no_bridge key=l`, no sent, no stop; replug into the same socket → `port_opened` 5 s after the unplug, LEDs ok, `gate gamepad=on` 48 ms after it; a quick L → 0x0004 / 0x0000 (55 ms, `key_up`), `mode from=idle to=direct abort_count=1`. 13 resized to about 1100 pt → the compact grid, no silhouette, sticks, chips or legend (owner screenshot, not committed; the caption fitted on one line at that width); tooltip on A "A: L key"; mouse quick click A → 0x0004 / 0x0000 (`pointer_up`, 100 ms); keyboard L → 0x0004 / 0x0000; back to 1280 or wider → the full pad. 14 window closed with both boards connected → `port_closed`, exit rc=0 promptly. Windows / Linux: deferred, issue #43.

**Owner eye check (Task 13), 2026-11-28, release app:** the inset ring and wash, the "Keys active" chip, the compact grid and its tooltips: all ok; no changes. The keyboard trap (D3) was seen on the bench (H.36 step 11) and stays as approved.

**Issue #43 comment text (for the coordinator):** "Story 4.9 adds H.36 (gamepad keyboard and compact layout on the real rig with OoT running) to the deferred Windows/Linux rows. macOS PASS 2026-11-28 at `7164e29` (M5 keyboard exit met on macOS; with H.33–H.35 the macOS part of M5 is complete). To do here: run the H.36 steps of `docs/test-plan.md` on a Windows and a Linux PC with a `bonooru` build of the bench commit (dry run `cargo test -p ocb-app -- --nocapture h36_dry_run` green on it); record the keyboard layout and key-repeat settings. winit's key-repeat and IME paths differ there; Ctrl+C/X/V become clipboard events with only the key release reaching the app (host-tested: a release of a key not held is ignored). macOS gotchas that may apply: the pause menu does not open in ocarina free play (leave it with J before step 7); a reply typed in another window blurs Bonooru, so start each step with a click on the panel or Tab. M5 stays TODO until these rows (H.28–H.36) pass."

### Completion Notes List

- Ultimate context engine analysis completed - comprehensive developer guide created (gds-create-story, 2026-11-28).
- 2026-11-28 (gds-dev-story): Tasks 1–10 done: the keys in the pure pad model (`PadKey`, key holds, union state, new release reasons and lines), the tokens, the compact 200 pt variant below 1280 pt with key-hint tooltips, the focusable `Role::Group` panel with the focused look and the "Keys active" chip, `ui::pad_keys`, the app's focus/lock/keys/release integration, G.24 (9 tests) and G.25 (18 tests including the H.36 dry run), the deliberate updates of G.1, G.10, G.19, G.20, G.21, G.22, G.23 (reasons above), red proofs for every new check, the instrument review, a green H.36 dry run, and the docs (EXPERIENCE.md, DESIGN.md, test plan with G.24, G.25, H.36, the keyboard rig block and the M5 keyboard line, epics.md bench line, roadmap M5 evidence with M5 TODO, app README).
- Tasks 1–13 done: host gates green, CI green on three OS at `f102f45` (the smoke runs covered by CI's G.7), two CI flakes fixed in the tests (`dc280e9` G.23/G.25 settle waits for the `pad sent` line; `f102f45` G.3 gallery `max_steps` 32), H.36 PASS on macOS (2026-11-28 at `7164e29`, M5 keyboard exit met; Windows/Linux deferred to #43; roadmap M5 stays TODO), owner eye check ok with no changes. Status review.
- 2026-11-28 review fixes: D-1, D-2 recorded; patches 1–11 and two defers done (Review Findings), red proofs in the Debug Log; new tests `key_repress_in_the_tap_window`, `g24_one_compact_threshold`, `g25_key_up_in_a_logic_only_pass`, `g25_dialog_takes_the_keys`, `g25_accesskit_focus_leaves_the_panel`, `g25_focus_lock_held_arrow`, `g25_keys_consumed_in_every_focused_frame`, `g25_other_buttons_keep_focus`, `g25_compact_presses`.
- For the eye check (Task 13): (1) the inset ring and the six-stroke wash (tokens `gamepad-panel-focused.ring-width/wash-steps/wash-step`); (2) the "Keys active" chip at the panel's top right, bottom 3 pt above the underline (at 200 pt the tab and the chip use 165 of the 180 pt); (3) the compact grid below 1280 pt (shoulders 42 pt wide in one row, the caption on two lines); (4) the tooltip wording ("A: L key", "D-pad up: W or Up arrow", "Home: mouse only"); (5) no sticks in compact (a drag in progress ends at the switch); (6) the keyboard trap: Esc or a click leaves, Shift+Tab from nothing reaches STOP. A key-held control is drawn with the Story 4.8 pressed tint.

### File List

- `_bmad-output/implementation-artifacts/4-9-gamepad-keyboard-focus-and-compact-layout.md` (story file)
- `_bmad-output/implementation-artifacts/sprint-status.yaml`
- `_bmad-output/planning-artifacts/epics.md`
- `_bmad-output/planning-artifacts/ux-designs/ux-bonooru-2026-11-20/DESIGN.md`
- `_bmad-output/planning-artifacts/ux-designs/ux-bonooru-2026-11-20/EXPERIENCE.md`
- `_bmad-output/specs/spec-bonooru/roadmap.md`
- `app/README.md`
- `app/gui/src/app.rs`
- `app/gui/src/gamepad.rs`
- `app/gui/src/ui/gamepad.rs`
- `app/gui/src/ui/keys.rs`
- `app/gui/src/ui/layout.rs`
- `app/gui/src/ui/main_screen.rs`
- `app/gui/src/ui/mod.rs`
- `app/gui/src/ui/pad_keys.rs` (new)
- `app/gui/src/ui/theme.rs`
- `app/gui/tests/a11y.rs`
- `app/gui/tests/bottom_bar.rs`
- `app/gui/tests/button_states.rs`
- `app/gui/tests/common/mod.rs`
- `app/gui/tests/design_tokens.rs`
- `app/gui/tests/firmware_app.rs` (review fix 6: `main_screen::hint_rect` takes the content rect)
- `app/gui/tests/gamepad.rs`
- `app/gui/tests/gamepad_app.rs`
- `app/gui/tests/gallery.rs` (CI flake fix `f102f45`: G.3 `max_steps` 32)
- `app/gui/tests/gamepad_focus.rs` (new, G.24)
- `app/gui/tests/gamepad_keys_app.rs` (new, G.25)
- `app/gui/tests/main_screen.rs`
- `app/gui/tests/stop_app.rs`
- `docs/test-plan.md`

## Change Log

- 2026-11-28: story created (gds-create-story), status ready-for-dev.
- 2026-11-28: owner approved the Open questions defaults ("ok"); implementation (gds-dev-story): Tasks 1–10 done, host gates green, H.36 dry run green; status in-progress (CI, H.36 bench and owner eye check open).
- 2026-11-28: review findings fixed (D-1, D-2, patches 1–11, two defers; Review Findings); status in-progress (H.36 bench and owner eye check open).
- 2026-11-28: CI flakes fixed in the tests (`dc280e9`, `f102f45`), CI green on three OS at `f102f45`; H.36 PASS on macOS at `7164e29` (M5 keyboard exit met; roadmap M5 stays TODO per D16, Windows/Linux on #43); owner eye check ok, no changes; status review.
