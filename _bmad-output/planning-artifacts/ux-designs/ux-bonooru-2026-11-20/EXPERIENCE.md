---
name: bonooru
status: final
updated: 2026-11-20
sources:
  - _bmad-output/specs/spec-bonooru/SPEC.md
  - _bmad-output/specs/spec-bonooru/requirements.md
  - _bmad-output/specs/spec-bonooru/decisions.md
  - _bmad-output/specs/spec-bonooru/architecture-diagrams.md
  - _bmad-output/specs/spec-bonooru/risk-register.md
  - _bmad-output/planning-artifacts/architecture/architecture-bonooru-2026-11-19/ARCHITECTURE-SPINE.md
  - docs/link-protocol.md
  - docs/usb-controller-emulation.md
  - docs/score-format-and-mapping.md
  - docs/ocarina-input-mechanics.md
  - docs/test-plan.md
---

# Bonooru — Experience Spine

Decisions come from `.memlog.md` (canonical). Anything not backed by the memlog or a source is tagged **[ASSUMPTION]**.

Mockups (original CSS/SVG, in `mockups/`): [layouts-main-screen-2026-11-20.html](mockups/layouts-main-screen-2026-11-20.html) (low-fi layout options; Variant A chosen), [directions-oot-2026-11-20.html](mockups/directions-oot-2026-11-20.html) (visual directions; 3 "Hybrid" chosen), and the approved Hybrid key screens [key-wizard.html](mockups/key-wizard.html), [key-empty-state.html](mockups/key-empty-state.html), [key-editor-staff.html](mockups/key-editor-staff.html), [key-dialogs.html](mockups/key-dialogs.html). **Spines win on conflict with any mock.**

Known mock drift (this spine supersedes all of it): the layouts mock still says "Switch linked", "PLAY" as one button, "?" for Home/Capture/+/−, and the spike key mapping (J K I U); the directions mock's confirm and end-card copy predate the Voice table, and its bottom bar has no Listen button.

Spine-only surfaces (no mock): Settings modal, Log view, song rail at 1024 px (the compact gamepad is specified in DESIGN.md › Compact variant since Story 4.9, still without a mock), unsaved-changes and restore-draft dialogs, countdown in Hybrid beyond direction 3.

## Foundation

- **Form factor:** desktop, single window. Windows, macOS, Linux; pre-built binaries (NFR-01).
- **UI system:** Rust + egui/eframe 0.37 (ADR-008 / AD-18). Non-native look: the app draws its own themed widgets. File dialogs use the `rfd` crate; window decorations are the OS's (Linux runs on X11/XWayland, see Responsive & Platform).
- **Input:** keyboard + mouse. Every function works with the mouse alone; the keyboard adds shortcuts and drives the virtual gamepad.
- **Language:** English UI only.
- **Theme:** dark only; OS light/dark setting ignored.
- **Usage context:** the owner opens the app mid-session while already playing on a Switch in handheld mode next to the PC. Attention is split between console and app, so state must be glanceable and STOP must be big and always reachable.
- **Visual identity:** `DESIGN.md` is the visual reference; this file references its tokens by name in braces (e.g. `{components.focus-ring}`).
- **Open-loop:** the app cannot see the game. It knows only the link state (see Hardware & Protocol Coupling). It never claims the game is in free play.

**Glossary.** *Mapping profile* = the TOML profile (pitches, modifiers, timing rules, range; AD-17); the UI uses this term, never "timing profile". *Preview* = the spec's dry run (CAP-9): PC audio only. *Bridge* = board 1 on the PC; *controller* = board 2 on the Switch; *the Switch* = the console.

**Component names** are identical in both spines: Status LED · Track picker · Transpose stepper · Fit proposal (shown as ghost note + fold target) · Tempo control · Roll / Staff toggle · Note view (Roll, Staff) · Context menu · Text editor · Split divider · Preview transport · Virtual gamepad · Play from cursor / Play from start · Listen · Undo / Redo buttons · STOP · Readiness confirm (a modal dialog) · Firmware update needed (a modal dialog) · Countdown · Interruption banner · Update banner (slim banner) · End-of-song card · Settings modal · Log view · Setup wizard (step list, progress bar, port pick list) · Empty state (drop zone) · Song rail · Inline hint · Focus ring.

## Information Architecture

### Main screen (Variant A, "everything on one canvas")

→ Reference: [mockups/layouts-main-screen-2026-11-20.html](mockups/layouts-main-screen-2026-11-20.html), variant A (regions); Hybrid states in [mockups/key-empty-state.html](mockups/key-empty-state.html), [mockups/key-editor-staff.html](mockups/key-editor-staff.html) and [mockups/key-dialogs.html](mockups/key-dialogs.html).

| Region | Contents |
|---|---|
| Status strip (top) | Brand · 3 status LEDs (PC↔Bridge, Bridge↔Controller, Controller↔Switch) · "Set up / Reflash…" · gear (Settings) |
| Update banner slot (under strip) | "Firmware update available" (recommended update only) |
| Song panel (left) | File name + Open… · Track picker (`.mid` only) · Transpose stepper + fit summary · official-song warning · Tempo control (% + BPM) · mapping-profile hint. Collapses to the song rail. |
| Note view (centre) | Header: Roll / Staff toggle · preview transport (▶ ■, time) · "Preview · PC only" · Undo ↶ / Redo ↷. Body: Roll or Staff view. With a text score: text editor on top, split divider, note view below. Overlays: empty state, countdown, interruption banner, end-of-song card |
| Gamepad panel (right) | Virtual gamepad + key legend; "Use it to reach ocarina free play." |
| Bottom bar | Play from cursor · Play from start · Listen ("on this PC") · safety note · STOP |

Reading order left to right: song → notes → gamepad → play.

### Setup wizard

→ Reference: [mockups/key-wizard.html](mockups/key-wizard.html) (steps 1, 3 and 5). Opens automatically only on first run (until a valid bridge has ever been seen). Later reached from "Set up / Reflash…" or from an update/required-update action. A clickable step list with completion check marks; on first run it is walked in order, on re-entry any step can be opened directly.

| # | Step | Done when |
|---|---|---|
| 0 | Welcome (short; not a numbered step) | user continues |
| 1 | Flash the bridge (board 1: hold BOOT, plug into PC) | the RPI-RP2 drive vanishes and a new `2E8A:000A` serial port appears; the product string "Bonooru Bridge" is checked where the OS supplies it (not on Windows). Full confirmation comes with INFO at step 4. |
| 2 | Flash the controller (board 2, on the PC) | drive vanished after copy (verification completes at step 4/6) |
| 3 | Wire the two boards (wiring diagram + photo + wiring rules 1–4) | user confirms with Next |
| 4 | Plug the bridge into the PC and the controller into the Switch | PC↔Bridge LED ok; INFO received (Bridge↔Controller ok) |
| 5 | Turn on "Pro Controller Wired Communication" (original illustration + exact menu path) | `console_ready` = 1 |
| 6 | Test: 3 LEDs ok, press A on the virtual gamepad | user confirms A reached the game |

### Other surfaces

| Surface | Reached from | Purpose |
|---|---|---|
| Main screen | Launch (after first run) | Load, tune, preview, play songs; drive the gamepad |
| Setup wizard | First run; "Set up / Reflash…"; "Update now"; "Set up…" on a foreign-board message | Flash, wire, connect, test |
| Settings modal | Gear in status strip (disabled during Play) | Countdown length, preview volume, reset "Don't ask again", "Set up / Reflash…", About + licences, Advanced (mapping-profile timing fields, Show log, Copy diagnostics) |
| Readiness confirm (dialog) | Play from cursor / Play from start / Resume | Restates assumed game state; "Don't ask again" |
| Firmware update needed (required-update dialog) | Protocol mismatch detected (INFO) | "Update now" opens the wizard at the right flash step; "Not now" closes it and leaves Play and the gamepad disabled. Epic 4 (Story 4.6): OK only, body = "The boards' firmware doesn't match this app. Reflash both with the .uf2 files of this app version: hold BOOT, plug the board in and drag its .uf2 onto the RPI-RP2 drive."; OK and Esc behave like the later Not now |
| Unsaved changes (dialog) | Close window / open another song with unsaved edits | Save / Discard / Cancel |
| Restore draft (dialog) | Launch after a crash with a draft | Restore / Discard |
| Log view | Settings › Advanced › Show log; "Details" on any error banner | Recent protocol events, versions, ports |
| About | Settings | Versions, third-party licences (fonts OFL, Hack, serialport MPL-2.0, …) |

Modal stack depth is one. The Settings modal closes with Esc or ×.

## Voice and Tone

Brand posture lives in `DESIGN.md` › Brand & Style. Rules:

- **Operational copy** (errors, wizard steps, LEDs, banners): plain, direct, second person, action first. No exclamation marks. Every error says what happened, then what to do.
- **Light game flavour** only in ambient moments: end-of-song card, empty state, example-song descriptions, wizard and dialog titles. Calm and warm, never jokey.
- **No franchise names** in our copy: no game, character, place or logo names, except naming a detected official song. Platform and settings names needed to operate the hardware are allowed ("the Switch", "Pro Controller Wired Communication", "Switch 2" in the mapping-profile name).
- Name the boards "bridge" and "controller" consistently; the console is "the Switch".
- The Listen button's fairy-orb icon is a silent easter egg: no copy anywhere refers to it, and the game's companion catchphrase ("Hey! Listen!" or variants) never appears.

| Moment | String | Source |
|---|---|---|
| Wizard step 1 | "Hold BOOT on board 1 and plug it into your PC." | draft |
| Wizard step 2 | "Take board 2 off the Switch, hold BOOT and plug it into your PC." | draft |
| Wizard step 3 | "Connect GP0, GP1 and GND only. Never 5V or 3V3. Unplug both cables before you rewire." | draft (rules from risk register) |
| Wizard step 4 | "Plug the bridge into your PC." / "Plug the controller into the Switch's top USB-C port." | memlog example / draft |
| Wizard step 4, no answer | "No answer from the controller. Check the wires: GP0 goes to GP1, GP1 to GP0." | memlog (Flow 4) |
| Wizard step 5 | "On the Switch, open System Settings › Controllers & Accessories › Pro Controller Wired Communication and turn it On." (under the illustration) | memlog path |
| Wizard step 6 | "All three lights are on. Press A on the gamepad and check the Switch reacts." | draft |
| Wizard welcome title | "A melody waits to be played." | memlog |
| LED ok | "Bridge v0.3" · "Controller v0.3" · "Switch – ready" | memlog pattern |
| LED unknown | label only: "Switch" (no detail text) | mock (approved) |
| LED warning | "Switch – not ready" (tooltip: "Turn on Pro Controller Wired Communication, wake the Switch, check the cable.") | memlog label / draft tooltip |
| LED error | "Controller – not answering" (tooltip: "Check the three wires between the boards and that the controller is plugged into the Switch.") | draft |
| LED states added by Story 4.5 | "Bridge – found" (tooltip "Bridge found on <port>.") · "Bridge – in use" · "Bridge – no access" (tooltip: the exact Linux group message) · "Bridge – other board" (tooltip "Found a “<product>” board – not a Bonooru bridge." (several: "… – not a Bonooru bridge – and <n> more.") + "Flash it with the bridge firmware: hold BOOT, plug it in and drag the bridge .uf2 onto the RPI-RP2 drive.") · "Bridge – not found" (tooltip "No bridge found." + "Plug the bridge into your PC.") · unconfirmed "Bridge" (tooltip in State Patterns "Windows, no product string") · "Controller – no link" (tooltip "The controller doesn't hear the bridge." + "Check the wire from the bridge's GP0 to the controller's GP1, and GND.") · "Controller v0.2.0" on a required update (tooltip "This firmware doesn't match this app." (Story 4.6: a board can be newer than the app) + "Update both boards: hold BOOT, plug in, drag the matching .uf2 onto the RPI-RP2 drive.") · waiting tooltips "Checking…", "Opening <port>…", "Waiting for the bridge.", "Asking the controller…", "Waiting for the controller." · worker tooltips (State Patterns "Serial worker stopped"). Versions in labels are three numbers ("Bridge v0.3.0"). | draft (Story 4.5 Copy table) |
| Foreign board | "Found a “RiiDraw” board – not an Bonooru bridge." [Set up…] | memlog [ASSUMPTION] |
| Flash: no drive | "No RPI-RP2 drive appeared. Unplug the board, hold BOOT, plug it back in." [Retry] | memlog |
| Flash: two drives | "Two boards are in BOOT mode. Unplug one, then Retry." | draft |
| Flash: wrong board | "This drive isn't an RP2040 board. Use an RP2040-Zero." | draft |
| Flash: no answer after flash | "The board didn't come back after flashing. Unplug it and plug it in again." | draft |
| Flash: fallback | "Flash manually: drag the .uf2 file onto the RPI-RP2 drive." | memlog |
| Required update | Title "Firmware update needed" — "This firmware is too old for this app. Update both boards to continue." [Not now] [Update now] (Epic 8). Epic 4 (Story 4.6): "The boards' firmware doesn't match this app. Reflash both with the .uf2 files of this app version: hold BOOT, plug the board in and drag its .uf2 onto the RPI-RP2 drive." [OK] | memlog title + buttons / draft body (Epic 4 body: draft, Story 4.6) |
| Required update, gamepad hint | "The gamepad is off until both boards are updated." | draft (mock) |
| Gamepad caption | "Use it to reach ocarina free play." (exempt from the "free play" ban: an instruction to the user, not a claim about the game state; Story 4.8 D13, owner 2026-11-27) | epics.md Story 4.8 (UX-DR64) |
| Gamepad hint, worker stuck | "Gamepad clicks are ignored: Bonooru's USB check is not responding." | draft (Story 4.8, owner ok 2026-11-27) |
| Gamepad hint, no bridge | "Plug the bridge into your PC." ("Set up / Reflash…" comes with Epic 8) | memlog |
| Gamepad hint, bridge not ready | "Gamepad clicks are ignored until the bridge is ready." | draft (Story 4.8, owner ok 2026-11-27) |
| Gamepad hint, controller | "Gamepad clicks are ignored until the controller answers." | draft (Story 4.8, owner ok 2026-11-27) |
| Gamepad hint, Switch not ready | "Gamepad clicks are ignored until the Switch is ready." | epics.md Story 4.8, mock |
| Recommended update | "Firmware update available" [Update] [Later] | memlog |
| Readiness confirm | Title "Ready to play?" — "The ocarina is out in free play." · "Don't touch the controller until the song ends." · "Don't dock the Switch." [Play] [Cancel] ☐ Don't ask again | memlog title + checklist |
| Countdown | "Starting from 0:12 · hands off the controller" | mock |
| Wizard waiting | "Waiting for the RPI-RP2 drive…" | draft (mock) |
| Wizard step 3 warning | "Circled pins: 5V and 3V3. Never connect them." | draft (mock) |
| Wizard step 5 sub-line | "Switch LED: “Switch – not ready” until the setting is On." | draft (mock) |
| Interruption banner | Tab "Playback stopped" — "Playback stopped at 1:12 – the Switch went to sleep. Wake it, take the ocarina out, then Resume." [Resume from 1:12] [Dismiss] Details | memlog |
| Import failure | Tab "Couldn't open" — "Couldn't open <file>: <reason>." [Dismiss] Details; reasons: "not a MIDI file or score", "the file is damaged", "the MIDI file has no notes", "the file was moved or deleted" | memlog default (6) / draft reasons |
| Play disabled tooltip | Names the **first** blocking reason only, in the order Bridge › Controller › Switch › firmware update › song issues. Links: "Play needs all three lights on." + fix line "<Link>: <fix>.", e.g. "Switch: turn on Pro Controller Wired Communication." Firmware: "Play needs a firmware update." + "Open Set up / Reflash… to update both boards." Song: "Load a song to enable Play." · "N notes too short – slow down to play." · "Fix the score errors to enable Play." | memlog order / draft wording |
| Play buttons | "Play from cursor" + sub-line with the cursor position ("0:42") · "Play from start" + "0:00" | memlog (gap default) |
| Gamepad focus | Chip "Keys active" (while the panel has keyboard focus); AccessKit group name "Gamepad – keys active" / "Gamepad – keys off" | mock `.kbd-on` / draft (Story 4.9) |
| Compact gamepad tooltips | "A: L key", "B: J key", "X: K key", "Y: H key", "D-pad up: W or Up arrow", "D-pad down: S or Down arrow", "D-pad left: A or Left arrow", "D-pad right: D or Right arrow", "L: Q key", "R: E key", "ZL: Z key", "ZR: C key", "Plus: Enter", "Minus: Backspace", "Home: mouse only", "Capture: mouse only" | draft, built from the key table (Story 4.9) |
| Listen button | "Listen" + sub-line "on this PC"; tooltip "Preview the song on this PC (Space). Nothing is sent to the Switch." | memlog label / draft tooltip |
| Undo / Redo tooltips | "Undo (Cmd/Ctrl+Z)" · "Redo (Cmd/Ctrl+Shift+Z)" (Ctrl on Windows/Linux, Cmd on macOS) | draft |
| Editor error (example) | "Unknown note length “q”." + fix line "Use 1, 2, 4, 8 or 16, e.g. D5/4." | draft (mock) |
| End card | "You played Ode to Joy!" · "1:48 · 62 notes · on time" [Play again] [Close] | memlog |
| Empty state | "Drop a .mid or score here" [Open…] [New song] · "Or try an example" · "Recent" | memlog |
| Official song | Note label "Triggers Song of Storms"; summary "1 official-song sequence: could end free play" | memlog |
| Too-short notes | "8 notes too short – slow down to play" | memlog pattern, draft wording |
| Converted `.mid` | "Converted to text score – save to keep changes" | memlog |
| Restore | "Restore unsaved draft?" | memlog |
| Safety note | "Don't dock the Switch mid-song." / "Esc stops at any time." | ARCHITECTURE-SPINE UI safety content ("don't dock mid-song") + memlog (Esc = STOP always) |
| Audio failure | "Can't open the audio device." [Retry] | draft |
| Mapping profile refused | "This mapping profile can't be read by this version. Defaults are in use." [Open profile file] | AD-17 rule / draft wording |

"draft" = written to the rules above, not yet approved [ASSUMPTION].

## Component Patterns

Behavioural rules. Visual specs live in `DESIGN.md` › Components.

| Component | Behavioural rules |
|---|---|
| Button (every button: dialog actions, STOP, later Play/Listen) | Enabled: hover lightens and pressed darkens the fill and border (`DESIGN.md` › Button states), so a click always gives visual feedback; the label does not move. Disabled: no hover or pressed change, not focusable. Activated by click, Enter with focus or an AccessKit click; never by Space (owner eye check, Story 4.7). |
| Status LED | Three LEDs, always visible. State from the coupling table below. Tooltip = detail + fix, shown on hover **and** on keyboard focus [ASSUMPTION]. Never says "linked to game" or "in free play". Click: [ASSUMPTION] opens the wizard at the relevant step when not ok. |
| Set up / Reflash… | Opens the wizard with the step list; no step forced. |
| Update banner | Recommended update only. Update → wizard at the right flash step. Later → hidden until next launch. |
| Track picker | `.mid` only. Auto-selects the likely melody track (most notes in range, least polyphony). Each entry: name · channel · note count · % in range. Change recompiles. Hidden for text scores (single voice). |
| Transpose stepper | −1/+1 semitone, −12/+12 octave, Auto (best fit −24..+24). `auto` chip shown while the value equals the automatic result. Live fit summary "212 in range · 2 folded · 0 dropped" updates on every change; "N long notes re-pressed" is added when notes longer than the game's ~6 s limit are re-pressed. Each change is one undo step. |
| Fit proposal | The pipeline's octave fold / drop is a proposed default, shown as ghost note + fold target; affected notes stay marked in the note view. Pressing Play accepts the current proposal [ASSUMPTION]; "Accept all" / "Reset to proposal" may live next to the fit summary [ASSUMPTION]. |
| Tempo control | Percentage stepper −5/+5 %, Reset, 50–150 %, linked to a BPM field (initial tempo; edits scale the whole tempo map). Every change recompiles and is one undo step. If notes get shorter than the in-game minimum: value turns `{colors.status-error}`, "N notes too short", Play disabled until slowed down. |
| Roll / Staff toggle | Two views of the same song and selection; switching keeps scroll, zoom, cursor, selection. Choice persists [ASSUMPTION]. |
| Note view (Roll) | 19 lanes B4–F6 only (sounding pitch, Story 1.1); fingering once per lane header (depends only on pitch). Ruler click sets the cursor shared by preview, Listen and Play from cursor. Scroll wheel / trackpad pans (Shift+wheel horizontal), Cmd/Ctrl+scroll zooms around the pointer (spike), middle-drag pans. Follows the playhead during preview and Play. When the lanes don't fit, the lane area scrolls vertically (ruler pinned). |
| Note view (Staff) | Same data drawn as staff rules; each note bar starts with its button glyph (`{components.staff-glyph}`). Lane headers stay (note name + fingering). Same selection, editing and playhead rules as Roll. → [mockups/key-editor-staff.html](mockups/key-editor-staff.html) |
| Note selection | Click a note = select it (replaces selection). Shift+click = add/remove it. Drag on empty canvas = marquee (`{components.marquee}`) selecting every note it touches; Shift+marquee adds. Click on empty canvas = clear selection. Selection spans the out-of-range zones (ghost notes are selectable). Esc does **not** clear selection (Esc is STOP). |
| Note editing | Mouse can add, move, resize, delete notes in either view. **Double-click an empty spot = add a note** at the pointer's lane and time (frame-grid snap) with the **last-used duration** (one beat before any note has been placed or resized [ASSUMPTION]). Drag a selected note body = move the selection: pitch snaps to the 19 lanes, time snaps to the mapping-profile frame grid. Drag a note's right edge = resize (frame grid). Delete or Backspace = delete the selection, **only while the note view has focus**. Right-click = context menu (below). Edits write back to the text score (source of truth); editor and note view stay in two-way sync; roll edits rewrite only the affected note tokens and keep the editor caret. Each gesture is one undo step. |
| Context menu | Right-click on a note (selects it if not selected) or on a selection: Move up an octave · Move down an octave · (separator) · Delete. Out-of-range notes add "Reset to proposal" [ASSUMPTION]. Items that would move notes outside B3–F5 + zones are disabled. Keyboard: the menu key or Shift+F10 opens it on the selection [ASSUMPTION]. |
| Out-of-range notes | Shown dimmed (`{components.out-of-range-zone}`, `{components.ghost-note}`) in the zones above F6 / below B4 with the proposed fix (fold target or "drop"). The user can move by octave, delete, or leave the proposal (via the context menu or drag). A zone with no notes collapses to a thin strip. |
| Official-song group | `{components.official-song-group}`. Highlighted for every detected sequence: the game matches songs by pitch, so no fingering avoids it (Spike 1.8). Play stays enabled; the user decides. |
| Text editor | Visible only for text scores (split: editor top, note view bottom). Note view updates live as the user types; syntax and validation errors are underlined inline with a hover message. Last valid parse stays drawn while the text is invalid [ASSUMPTION]. Plain Enter is a newline; Space types a space. egui's built-in TextEdit undoer is not used (app-level undo). |
| Split divider | Drag to resize editor vs note view; position persisted. The editor keeps at least 4 text lines; the note view scrolls vertically when its lanes don't fit. Double-click resets to the default [ASSUMPTION]. |
| Undo / redo | One app-level history: Cmd/Ctrl+Z undo, Cmd/Ctrl+Shift+Z redo, everywhere (including the editor), and the visible **↶ Undo / ↷ Redo buttons** at the right of the note-card header (`{components.history-button}`; each disabled when its stack is empty; tooltips name the shortcut). Covers text edits, note-view edits, out-of-range octave moves/deletes, transpose and tempo changes. Not covered: track choice, view toggle, cursor, Settings. History is per song and cleared when another song is loaded [ASSUMPTION]. Disabled during Play. |
| `.mid` conversion | First note-view edit on a `.mid` converts it to a text score (selected track, transpose applied), opens the split editor, shows the conversion notice. The original `.mid` is never modified. Undo of that first edit keeps the converted score [ASSUMPTION]. |
| Preview transport | PC audio only, ocarina-like synthesised timbre, nothing sent to the Switch. ▶/■ plus moving playhead. Starts from the cursor. Volume in Settings. Disabled during Play. Three equivalent triggers: header ▶, the Listen button, Space. |
| Listen | Secondary button in the bottom bar after Play from start (`{components.button-listen}`, fairy-orb icon `{components.fairy-orb-icon}`), label "Listen", sub-line "on this PC". Same action as the header ▶ / Space: starts the preview from the cursor; while the preview plays, pressing it again stops it [ASSUMPTION toggle; label stays "Listen"]. Not gated by the LEDs or by a firmware update. Disabled with no song loaded and during countdown/Play. With an invalid text score it previews the last valid parse [ASSUMPTION]. |
| Virtual gamepad | Full Pro Controller set: A B X Y, L R ZL ZR, D-pad, both sticks, +, −, Home, Capture. Mouse: click = tap, press-and-hold = hold, drag sticks. Keyboard mapping active only while the gamepad panel has focus (see Interaction Primitives › Gamepad focus). Releases everything on focus loss and window blur (spike). Key auto-repeat ignored. While the app is in DIRECT mode it sends PING when idle (AD-7). During countdown and Play: locked (`{components.chip-locked}`, no input) and acts as a live visualizer (`{components.gamepad-control-lit}`): the lit state is derived from the compiled timeline at the interpolated playback time (STATUS `now_us`), since the protocol echoes no input state. When the Switch is not ready or the link is down, clicks are ignored with an inline hint. Below 1280 px: compact variant. **Epic 4 (Story 4.8), mouse:** press = press in the frame it arrives, release = release; a click is a tap of at least 50 ms (U7's 33.3 ms + one 60 Hz frame; the release waits, the press does not); a hold lasts as long as the button is held; a press dragged out of its control keeps it held until the release; one control at a time by mouse; for round controls only a press inside the circle counts; sticks: the value follows the drag, full deflection at 17 pt from the well centre, circular clamp, no dead zone, spring back to centre on release. The pad sends one canonical state on every change (none while nothing changes; a failed send is retried, last state wins). Release rules: (a) window blur, or the window hidden or minimized (also when no frame is drawn) → one NEUTRAL; (b) STOP (Esc or the button) → STOP releases, nothing else sent, the press is spent until the button comes up; (c) the gate turning blocked while something is held → STOP (`source=gamepad`), because NEUTRAL could be refused; (d) the controller leaving DIRECT by itself → the held input is dropped, nothing sent. While gated the pad keeps its look, no hover or pressed tint reacts, nodes stay enabled, and each click is logged `pad ignored` with the hint of the first blocking reason (worker › no bridge › bridge not ready › firmware update › controller › Switch) under the legend. Accepted limits (review 2026-11-27; nothing stays held, the press is just not applied): a second press within the 50 ms tail of a click, an AccessKit tap during a hold (both logged `pad ignored … reason=busy`), a press and release on a stick within one frame (no minimum for sticks), a release and a new press batched in one frame, an AccessKit tap in a frame where the primary button comes up elsewhere, a press right after STOP that the worker drops while STOP waits for its ACK (drawn held until the release; the user presses again). A release the controller would refuse while it may still hold an input becomes a STOP. **Epic 4 (Story 4.9), keyboard, panel focus and compact variant:** the keys drive the pad only while the panel has keyboard focus (Interaction Primitives › Gamepad focus and › Virtual gamepad mapping); key holds and the mouse hold are tracked apart and the pad sends their union (buttons OR-ed, the hat from every direction held, the sticks). Every release rule ends the key holds too, each with its own `pad release … key=<k>` line, and one rule is added: (a′) the panel losing focus for any other reason (a click elsewhere, a dialog) → one NEUTRAL, like a blur; a blur also drops the panel's focus. A key still physically held after a release rule is not pressed again by its auto-repeat; only a fresh press is. While gated each fresh key press is logged `pad ignored … key=<k>` and the ring and chip stay. Below 1280 pt the panel is the compact variant (Responsive & Platform): the same controls in a 180 × 124 pt button grid, no sticks (a stick drag in progress ends when the layout switches), key hints in hover tooltips; mouse semantics, keys, gating, release rules and lines are the same at both widths. Accepted limits (nothing stays held in any of them): a key pressed in the same frame as the click or Tab that focuses the panel, or the Esc or click that unfocuses it, does nothing; a key still held when the panel regains focus by a click (without a blur in between) does nothing until it is released and pressed again; a key held through Cmd+Tab (or Cmd+H) and still held when the panel is clicked again after coming back presses its control once more at its next OS repeat, because egui forgets the keys held at a blur and that repeat arrives looking fresh (the hold then ends at the key's release as usual); more keys than the keyboard reports at once (hardware rollover) are not seen; non-US layouts and IME input sources are positional by design but not benched; the panel traps Tab while focused, and Esc (also STOP) or a click are the two ways out. |
| Play from cursor | Label "Play from cursor"; the sub-line shows the cursor position ("0:42"), everywhere the action appears (bottom bar, tooltips, docs). Compiles from that offset. Goes through readiness confirm + countdown. |
| Play from start | Always t = 0. Same confirm + countdown. |
| Play gating | Both Play actions are disabled unless **all three LEDs are ok**, and also while a required firmware update is pending, when no song is loaded, notes are too short, the text score is invalid [ASSUMPTION], or during Play. The disabled button's tooltip names only the **first** blocking reason and its fix, checked in the order Bridge › Controller › Switch › firmware update [ASSUMPTION position] › song issues (no song › too-short notes › invalid score). Preview is always available. The `console_ready` 100 ms rule was verified on hardware (Story 3.5 H.3, 2026-11-26; kept by the owner: worst console polling gap 24 ms in normal use, the flag falls 144 ms after the last poll on Wired Communication OFF). |
| STOP | Always enabled. Esc anywhere. Sends STOP (release every input). `{components.button-stop-idle}` when idle, `{components.button-stop-live}` during countdown and Play. Epic 4 (Story 4.7): idle look only; with a dialog open the scrim takes the click and Esc is the way to STOP; Space never activates it; hover and pressed states like every button (owner eye check). |
| Readiness confirm | Shown on every Play unless "Don't ask again" is ticked; reset from Settings. Play has initial focus: Enter = Play, Esc = Cancel (and STOP, harmless when idle). → [mockups/key-dialogs.html](mockups/key-dialogs.html) |
| Firmware update needed | Blocking modal on protocol mismatch (INFO `proto_ver` ≠ app). Update now (initial focus, Enter) opens the wizard at the right flash step. Not now (or Esc) closes it: both Play buttons and the gamepad stay disabled (gamepad inline hint, Play tooltip "Play needs a firmware update."), while Listen/preview, editing, loading and saving keep working. The update stays reachable from "Set up / Reflash…"; the dialog shows again on the next launch or reconnect while the mismatch persists (decided, Story 4.6: once per connection, never again on the same connection). Epic 4 (Story 4.6): OK only, body = "The boards' firmware doesn't match this app. Reflash both with the .uf2 files of this app version: hold BOOT, plug the board in and drag its .uf2 onto the RPI-RP2 drive."; OK and Esc behave like the later Not now. → [mockups/key-dialogs.html](mockups/key-dialogs.html) |
| Countdown | 3·2·1 by default (Settings 0–5 s), large numerals (`{components.countdown-numeral}`) over the note view, audible tick per number: an original soft wood-block sound. STOP already live; Esc cancels. If a link or console signal drops during the countdown, it is cancelled, no START is sent, and the interruption banner explains why. Separate from the technical `start_lead` (500 ms). |
| Interruption banner | → [mockups/key-dialogs.html](mockups/key-dialogs.html). `{components.banner-dialogue}` over the note view. States cause + fix + time. Cursor stays at the stop point. Actions: Resume from <time> (= Play from cursor, through confirm + countdown; disabled per Play gating), Dismiss, Details (opens the log view). Persists until the user acts on it or dismisses it. Also used for import failures (no Resume). |
| End-of-song card | `{components.end-card}` on PLAYBACK_DONE. Title + detail (actual duration · notes · on time vs expected). Play again / Close. Auto-dismisses after 6 s; the timer pauses while hovered. No sound. |
| Song panel during Play | Locked (mock state 3). Note view read-only; scroll allowed [ASSUMPTION]. Drop and Open… are ignored during Play (drop shows nothing; Cmd/Ctrl+O is disabled) [ASSUMPTION]. |
| Song rail | Below 1280 px the user can collapse the song panel into the song rail with a chevron in its tab [ASSUMPTION: user toggle, never automatic; state persisted]. Rail icons (Open, Track, Transpose, Tempo) show the current value in a tooltip; clicking any icon expands the panel with that control focused. |
| Settings modal | Gear disabled during Play. Advanced section collapsed by default: mapping-profile timing fields (min press, min gap same button, modifier lead, start lead, …) each with a one-line explanation, Reset to defaults, Open profile file, Show log, Copy diagnostics. A user mapping profile that cannot be migrated is refused (AD-17) with an inline message in Advanced; defaults stay in use. |
| Log view | Recent EVENT/NAK/versions/ports. Copy diagnostics = clipboard summary (app + firmware versions, OS, recent events). Every error banner has "Details", which opens the log view at the relevant entry. CSV timeline export (AD-15) is not user-facing in v1. |
| Empty state | → [mockups/key-empty-state.html](mockups/key-empty-state.html). Drop zone, Open…, New song (opens the split view with an empty score), 3–5 bundled public-domain examples already in range (one click loads), Recent (last 5). Play, Listen, the preview transport and Undo/Redo are disabled; the gamepad stays usable. |
| Persistence | On launch reopen the last song with its track, transpose, tempo and cursor. Per-song memory of those when a file is reopened. If the last song or a Recent entry is missing, the empty state opens with the import-failure banner and the entry is removed from Recent. |
| Drafts | Continuous autosave of a draft to an internal app folder. Window title shows a dot while there are unsaved changes. Cmd/Ctrl+S saves (save-as for new/converted scores). |
| Wizard step list | `{components.wizard-step}`. 6 clickable steps with check marks; footer Back / Next. First run: Next is enabled only when the current step is done (on step 3, Next itself is the confirmation); re-entry: any step clickable. Steps auto-complete from detection where possible (see IA table). Closing the wizard on first run returns to the main screen in the "bridge not found" state [ASSUMPTION]. |
| Wizard flasher | Snapshots volumes before the BOOT prompt; shows "Waiting for the RPI-RP2 drive…" with a spinner; accepts exactly one new RPI-RP2 volume with `INFO_UF2.TXT`, RP2040 family. Progress bar (`{components.progress-bar}`, "Copying… 64 %") while copying. Each failure has a specific message + Retry; after 2 failures also "Flash manually" (opens the folder with the `.uf2`, explains drag-and-drop). Never 1200-baud-touches an unconfirmed port. |
| Port pick list | Windows only, when a reflash needs a 1200-baud touch and the OS gives no product string: lists every `2E8A:000A` port (`{components.port-pick-row}`); the user picks one, then "Use this port". Never shown on the main screen (AD-13). |

## State Patterns

| State | Surface | Detection | Treatment |
|---|---|---|---|
| First run | Wizard | no valid bridge ever seen | Wizard opens at Welcome, then step 1. Step list visible, walked in order. |
| Unsigned app first run | Outside the app | OS gatekeeper | Handled by the README (macOS Open Anyway; Windows SmartScreen More info › Run anyway). The app shows nothing [ASSUMPTION per memlog]. |
| Cold launch, bridge present | Main | VID:PID + product string, then INFO | LEDs start `unknown` (hollow ring), resolve within ~1 s. Last song reopened. |
| Bridge not found | Main | no VID:PID match | PC↔Bridge `error` "Bridge – not found"; other two `unknown`. Inline hint under the gamepad legend (Story 4.8): "Plug the bridge into your PC." ("Set up / Reflash…" next to it from Epic 8). Song features stay usable (offline preview). Play disabled (gating). |
| Foreign Pico / RiiDraw | Main / Wizard | VID:PID match, product string ≠ "Bonooru Bridge" | PC↔Bridge `warning`; message names the board when the OS gives a product string, with "Set up…". Never touched over serial; flashing only via BOOT + plug [ASSUMPTION per memlog]. |
| Windows, no product string | Wizard / Main | `serialport` gives no string | Wizard: step 1 completes on drive-vanish + new port (IA table). A reflash that needs a 1200-baud touch shows the port pick list first (AD-13). Main screen never asks: until INFO, PC↔Bridge `unknown` "Bridge" with the tooltip "A board that may be the bridge is on <port>. Bonooru can confirm it once the controller answers." + "Wire the controller to the bridge and plug it into the Switch."; Bridge↔Controller `error` "Controller – not answering" after the 500 ms INFO grace; Switch `unknown`. With INFO: "Bridge v…", "Controller v…" (resolved Open Question 4, provisional until issue #43). |
| Bridge only, no controller | Main | port open, HELLO unanswered, no STATUS | PC↔Bridge `ok` but without version ("Bridge – found"; the version needs INFO, resolved Open Question 5); Bridge↔Controller `unknown` for the 500 ms INFO grace, then `error` "Controller – not answering"; Switch `unknown`. Wizard offers to reflash the bridge (AD-13). |
| Serial worker stopped | Main | the worker thread ended, or no snapshot for 3 s | All three LEDs `unknown` (screen reader: "<label> – unknown"); every LED's tooltip "Bonooru stopped checking the USB ports." + "Restart Bonooru. If it happens again, send the file bonooru.log." (stuck: "Bonooru's USB check is not responding." + "Wait a moment. If it stays like this, restart Bonooru."). Never a frozen "connected" (Story 4.5). |
| Port in use | Main | the OS reports the port busy | PC↔Bridge `warning` "Bridge – in use"; tooltip "Another program is using <port>." + "Close it (a serial monitor or another Bonooru window); Bonooru retries by itself." (Story 4.5). |
| Controller not ready | Main | STATUS `console_ready` = 0, or NAK NOT_READY | Switch `warning` "Switch – not ready"; tooltip lists fixes (Wired Communication, wake, cable). Gamepad clicks are ignored with the inline hint "Gamepad clicks are ignored until the Switch is ready." (Story 4.8). Play disabled. |
| USB suspend / dock (idle) | Main | `console_ready` drops to 0 | Same as controller not ready; tooltip adds "Undock the Switch". |
| Link lost (idle) | Main | `link_ok` = 0, NAK NO_LINK, or no STATUS for 150 ms (resolved Open Question 3) | Bridge↔Controller `error`; Switch `unknown`. Gamepad clicks ignored with the inline hint "Gamepad clicks are ignored until the controller answers." (Story 4.8). Play disabled. |
| Required update | Blocking dialog "Firmware update needed" | `INFO.proto_ver` ≠ app | LEDs show the boards' current versions (e.g. "Bridge v0.2"). Update now → wizard at the right flash step. Not now → dialog closes; Play and gamepad stay disabled until fixed; Listen/preview and editing work. Epic 4 (Story 4.6): OK only, body = "The boards' firmware doesn't match this app. Reflash both with the .uf2 files of this app version: hold BOOT, plug the board in and drag its .uf2 onto the RPI-RP2 drive."; OK and Esc behave like the later Not now. |
| Recommended update | Update banner | `fw_ver` / `bridge_ver` ≠ app | Slim banner under status strip, Update / Later. |
| Controller rebooted | Main | NAK VERSION mid-session | App re-sends HELLO; LEDs briefly `unknown`; no message unless it fails [ASSUMPTION]. |
| Empty (no song) | Main | — | Drop zone, Open…, New song, examples, Recent. Play disabled ("Load a song to enable Play"); Listen, transport and Undo/Redo disabled. Gamepad usable. |
| File hovering | Main | `hovered_files` | Drop zone border turns amber-ink; outside the empty state the whole note-view card shows a drop outline [ASSUMPTION]. |
| Song loaded | Main | — | Song panel filled; Play enabled per gating. |
| Import failed | Banner over note view | unreadable/corrupt file, unsupported type (MusicXML/ABC etc.: export to MIDI, ADR-007), `.mid` with no notes or percussion only | Banner "Couldn't open <file>: <reason>." The current song is kept (or the empty state if none). |
| Track 0 % in range | Song panel | range fit | Track loads; every note shows as a ghost note with its fold proposal; fit summary states it; the track picker shows "0 % in range" so another track can be chosen [ASSUMPTION]. |
| Last song / Recent missing | Launch / Recent | file not found | Empty state + banner "Couldn't open <file>: the file was moved or deleted."; the entry is removed from Recent. |
| Out-of-range notes | Note view | range fit | Dimmed zones with ghost notes + proposed fold/drop; fit summary counts. Play enabled (accepts proposal). |
| Official song | Note view + song panel | pipeline R2, no alternative | Ember group + label; summary line. Play enabled. |
| Too-short notes | Song panel | timing validation | Tempo value `{colors.status-error}`, "N notes too short", Play disabled with tooltip. |
| Text score invalid | Editor | parser | Inline underline + hover message; Play disabled while invalid [ASSUMPTION]. |
| Unsaved draft | Title bar | edits since save | Dot in title. Close → Save / Discard / Cancel. |
| Crash restore | Launch | draft newer than file | "Restore unsaved draft?" Restore / Discard. |
| Audio device failure | Note-view header | audio adapter error | Inline message "Can't open the audio device." + Retry; re-open on default-device change; no silent fallback. |
| Mapping profile unmigratable | Settings › Advanced | `profile` module refuses the user copy (AD-17) | Inline message; bundled defaults in use; "Open profile file". |
| Readiness confirm | Dialog | Play pressed | Checklist; Play / Cancel; Don't ask again. |
| Countdown | Note view | after confirm | Veil + numerals + tick; STOP live; gamepad locked; song panel locked. |
| Link or console drop during countdown | Banner | `link_ok` = 0, `console_ready` = 0, port vanished | Countdown cancelled, no START sent; interruption banner with cause + fix, "Resume from <cursor>". |
| Playing | Main | STATUS mode PLAYING, `now_us` | Playhead + time from interpolated `now_us`; gamepad visualizer; STOP live; gear disabled; drop/Open ignored. |
| Underrun | Banner | EVENT UNDERRUN | "Playback stopped at 1:12 – notes didn't arrive in time. Close heavy apps, then Resume." (draft). App sends STOP to clear the latch. |
| LATE | Banner | EVENT LATE | "Playback stopped at 1:12 – the timing slipped. Resume to try again." (draft). STOP sent. |
| Link lost (playing) | Banner | EVENT LINK_LOST | "Playback stopped at 1:12 – the two boards lost contact. Check the wires, then Resume." (draft). |
| USB suspend / docked (playing) | Banner | EVENT USB_SUSPENDED | "Playback stopped at 1:12 – the Switch went to sleep. Wake it, take the ocarina out, then Resume." (memlog) |
| Bridge unplugged (playing) | Banner | port vanished | "Playback stopped at 1:12 – the bridge was unplugged. Plug it back in, then Resume." (draft). |
| Semantic NAK on QUEUE/START | Banner | NAK BUSY/ORDER/BAD_STATE/ABORTED | Piece abandoned, STOP sent (AD-6); generic "Playback couldn't start" + Details [ASSUMPTION]. |
| Done | End card | EVENT PLAYBACK_DONE | "You played <Song>!" card; auto-dismiss after 6 s. |
| Flash: no RPI-RP2 drive | Wizard | no new volume | Specific message + Retry. |
| Flash: more than one new drive | Wizard | >1 new volume | Specific message + Retry. |
| Flash: wrong UF2 family / board | Wizard | `INFO_UF2.TXT` check | Specific message + Retry. |
| Flash: board not answering after flash | Wizard | no new `2E8A:000A` port (step 1) / no INFO (step 4) | Specific message + Retry. |
| Flash: 2 failures | Wizard | counter | Adds "Flash manually". |
| Linux: no serial permission | Main / Wizard | adapter detects `dialout` | Plain message with the exact command, e.g. `sudo usermod -aG dialout $USER`, then "log out and back in". Never escalates privileges (ARCHITECTURE-SPINE UI safety content). Group name per distro [ASSUMPTION]. |
| Linux: RPI-RP2 not mounted | Wizard | adapter detects udisks | Plain message: "Open the RPI-RP2 drive in your file manager to mount it, then Retry." + exact `udisksctl mount` command [ASSUMPTION wording]. |
| Focus | All | — | Visible `{components.focus-ring}` on every focusable control; gamepad panel focus is the amber panel ring + "Keys active". |

## Interaction Primitives

**Global shortcuts** (work everywhere, including editor and gamepad focus; consumed at the start of each frame before widgets read input):

| Key | Action |
|---|---|
| Esc | STOP, always (emergency brake). Also closes the topmost dialog or the Settings modal. Read (not consumed) at frame start, before any egui `Modal` consumes it: read at the top of every frame in every mode, key repeats ignored, one STOP per frame (Story 4.7). |
| Cmd/Ctrl+Enter | Play from cursor |
| Cmd/Ctrl+Shift+Enter | Play from start |
| Cmd/Ctrl+O | Open song |
| Cmd/Ctrl+S | Save (save-as for new or converted scores) |
| Cmd/Ctrl+Z / Cmd/Ctrl+Shift+Z | Undo / redo (app-level history); same as the ↶ ↷ buttons in the note-card header |

**Context shortcuts:**

| Key | When | Action |
|---|---|---|
| Space | everywhere except the text editor (including while the gamepad has focus; Space is not a gamepad key) | Preview play / stop (same as Listen and the header ▶). Space is consumed at frame start (since Story 4.7; no preview yet), with any modifiers while a widget other than a text edit has focus, so it never activates a focused button. |
| Enter | a button has focus | Activates it (buttons activate with Enter, never Space); once per fresh press: key repeats of a held Enter are dropped at frame start outside a text edit (Story 4.7) |
| Delete / Backspace | the note view has focus and a selection exists | Delete selected notes |
| Double-click | empty spot in the note view | Add a note with the last-used duration |
| Tab / Shift+Tab | anywhere outside the editor and the gamepad | Move focus between controls and regions in the order status → song → note view → gamepad → bottom bar (regions laid out manually to keep this order). Since Story 4.9: from nothing focused, Tab reaches the three LEDs, then the gamepad panel, which keeps Tab and Shift+Tab (the focus lock); Shift+Tab from nothing focused reaches STOP. |

**Gamepad focus.** The gamepad panel takes egui focus **as a whole** (one panel-level widget id; the 18 child controls are not focusable). It gains focus by a primary press anywhere in the panel that reaches it (a control, a stick or empty panel space; a press on a control still presses it in the same frame, and the app requests the panel's focus again at every press and release on the panel, because egui would otherwise drop it at a click on a control above it), by tabbing to it (owner, Story 4.9, 2026-11-28), or by an AccessKit `Focus` action. A right or middle click inside the panel keeps (or gives) the focus too, but only the primary button presses a control. It loses focus by a click elsewhere, by Esc without modifiers (Esc is also STOP; with Shift or Alt held STOP still happens and the panel keeps its focus), when a dialog opens (the dialog takes the keys even when its own button cannot take focus, and in the very frame it opens) and when the window loses focus or is hidden or minimized: after Cmd+Tab back the keys stay off until the panel is clicked again (owner, Story 4.9, 2026-11-28), and the ring and chip are gone so the look tells the truth. While focused, a focus-lock filter keeps the arrows, Tab and Shift+Tab from moving egui focus, so the arrows press the D-pad and Tab does not leave the panel; egui accepts the filter only from the panel's second frame of focus, so in the first frame the app pulls the focus back when a Tab or an arrow took it. Esc or a click elsewhere are the two ways out (owner, Story 4.9, 2026-11-28). Keys are read only when the panel had focus at the end of the previous frame and still has it, so the click, Tab or Esc that changes the focus never presses anything in its own frame. Keys are matched by **physical position** (`physical_key`), so WASD/HJKL stay positional on AZERTY and other layouts; a key event without a physical key does nothing. Losing focus or window blur releases every held input.

**Virtual gamepad mapping** (active only while the gamepad panel has focus):

| Keys | Control |
|---|---|
| Arrow keys and W A S D | D-pad |
| H · J · K · L | Y · B · X · A (vim-positional on the face-button diamond: H left = Y, J down = B, K up = X, L right = A) |
| Q · E | L · R |
| Z · C | ZL · ZR |
| Enter | + |
| Backspace | − (only while the gamepad has focus; in the note view Backspace deletes) |
| — | Home, Capture, both sticks: mouse only |

Cmd/Ctrl combos keep their global meaning while the gamepad has focus (Cmd/Ctrl+Enter is Play, plain Enter is +): a key press with Cmd, Ctrl or macOS Cmd held presses nothing and stays in the input for the app's shortcuts; Shift and Alt/Option do not block a press (owner, Story 4.9, 2026-11-28). Hold semantics (Story 4.9): a fresh press presses its control in the frame it arrives; the key's release releases it, never earlier than 50 ms after the press (a quick key tap is a 50 ms tap, as a click); OS key auto-repeat presses nothing; any number of keys can be held at once (chords: the state is the union); two keys on the same control (W and Up arrow) keep it pressed while either is held; opposite directions held together cancel on that axis (Left arrow + D: no horizontal direction); key holds and the mouse hold are separate, so releasing one never ends the other. A release is honoured whatever the modifiers; a release of a key that is not held is ignored. While the panel has focus the mapped key events are consumed after the pad reads them, so no other widget sees them.

**Mouse equivalence:** every gamepad control is clickable; press-and-hold holds; sticks drag. Every shortcut has a visible button.

**Note-view mouse gestures:** click = select · Shift+click = extend/toggle · drag on empty = marquee · drag a note = move (lane + frame-grid snap) · drag its right edge = resize · double-click empty = add · right-click = context menu · wheel / middle-drag = pan · Cmd/Ctrl+wheel = zoom.

**Play sequence:** Play from cursor / start (enabled only with 3 LEDs ok) → readiness confirm (unless suppressed) → countdown (0–5 s, default 3, wood-block tick per number) → playback. STOP is live from the countdown on.

**Confirm suppression:** "Don't ask again" on the readiness confirm; reset in Settings.

**Banned:** stacked modals; hover-only actions (every tooltip-only fix is also shown on focus); automatic reconnect that silently restarts a song; any gamepad input during Play; 1200-baud touches on unconfirmed ports; Space activating buttons.

## Accessibility Floor

Behavioural. Visual contrast lives in `DESIGN.md`.

- LEDs are never colour alone: colour + symbol (`ok` filled dot `{colors.status-ok}`, `warning` triangle `{colors.status-warning}`, `error` cross `{colors.status-error}`, `unknown` hollow ring `{colors.status-unknown}`) + short text label; tooltip gives detail + fix, on hover and on focus. Warning must never be confused with the amber accent `{colors.amber}`.
- Focus is always visible: every focusable control paints `{components.focus-ring}` (`{components.focus-ring-on-parchment}` on parchment) through one shared helper, because egui draws no ring by default and has no focus-visible (the ring therefore also shows after mouse use). The gamepad panel's focus (`{components.gamepad-panel-focused}` + `{components.chip-keys-active}`) is mandatory because it changes what the keyboard does.
- Every function is reachable by keyboard except the sticks, Home and Capture (mouse only, by decision) and note drawing/moving/resizing (mouse); keyboard deletion and the context menu work on a selection [ASSUMPTION for Shift+F10].
- Esc as global STOP never needs focus.
- The virtual gamepad (Story 4.8) is 16 `Button` nodes ("A" … "Capture", "D-pad up" …) and 2 `Slider` nodes ("Left stick", "Right stick"; AccessKit has no two-axis role), none focusable (they stay out of the Tab ring); an AccessKit click on a button is a tap (press, release 50 ms later); the tab, caption and each legend row are `Label` nodes; chips and painted symbols add none. Since Story 4.9 all of them are children of the panel's one focusable `Group` node, named "Gamepad – keys active" while it has keyboard focus and "Gamepad – keys off" otherwise (an AccessKit `Focus` on it gives the keys to the pad); the "Keys active" chip adds no node (the name says it). The compact panel is the same group with the 16 `Button` nodes and the tab, caption and hint labels (no `Slider`, no legend).
- Keyboard trap, on purpose (Story 4.9, D3): while the gamepad panel has focus Tab and Shift+Tab stay on it, so the arrows can drive the D-pad; Esc (also STOP, harmless when nothing plays) or a click elsewhere leave it, and STOP stays reachable by Shift+Tab from nothing focused. The group's name says whether the keys are active.
- STOP's AccessKit node is one `Button` named "Stop" (plain case, so screen readers do not spell it) with the description "Esc stops at any time."; the painted ■ adds no node (Story 4.7).
- Minimum window 1024 × 680 logical points. HiDPI through egui `pixels_per_point`.
- No information conveyed only by sound (countdown numerals are visible; the end card is silent).
- Screen-reader support: basic (owner, Story 4.2, 2026-11-27). Every shared widget helper emits `widget_info` (closest `egui::Role` + the visible label as the accessible name, plain words where nothing visible names it); custom widgets that do not go through a helper do the same; the Roll and the Staff expose one summary node each. No live regions, no per-note nodes, no screen-reader test on real hardware. eframe's `accesskit` feature stays on (egui builds the tree only when an assistive technology asks for it).

## Key Flows

### Flow 1 — First evening from zero (Tiziano, two boards still running RiiDraw firmware)

1. Tiziano downloads the app and unblocks it with the README (outside the app).
2. First launch: the wizard opens at the Welcome screen, "A melody waits to be played."
3. Step 1: he holds BOOT on board 1 and plugs it in. The app sees one RPI-RP2 drive and flashes the bridge; the drive vanishes and the bridge port appears with its product string. Check mark.
4. Step 2: he takes board 2 off the Switch, holds BOOT, plugs it into the PC. Flash, drive vanishes. Check mark.
5. Step 3: he wires GP0/GP1 crossed + GND using the wiring diagram and photo; the wiring rules are on screen.
6. Step 4: bridge into the PC, controller into the Switch's top port. PC↔Bridge and Bridge↔Controller go ok.
7. Step 5: Switch LED shows "Switch – not ready". He turns on Pro Controller Wired Communication following the illustration and path; the LED turns ok.
8. Step 6: three LEDs ok. He presses A on the virtual gamepad and sees the game react. Wizard done; main screen opens on the empty state.
9. He uses the gamepad to take the ocarina out in game, then loads the example "Ode to Joy".
10. Play from start → "Ready to play?" → 3·2·1 → the gamepad lights up as the notes go out.
11. **Climax:** the parchment card appears: "You played Ode to Joy!" — and the song just played on the Switch next to him.

Failure: step 1 shows no drive → "No RPI-RP2 drive appeared. Unplug the board, hold BOOT, plug it back in." + Retry; after two failures, "Flash manually".

### Flow 2 — Mid-game session from a .mid (Tiziano, handheld Switch next to the PC)

1. He opens the app; it reopens last night's song with its track, transpose, tempo and cursor.
2. He drops a new `.mid` onto the window. The app picks the melody track, auto-transposes, and reports "212 in range · 2 folded · 0 dropped".
3. In the Roll he sees the two folded notes dimmed above F6 with their proposed fold. He right-clicks one and deletes it, and leaves the other as proposed. He clicks Listen to check the result on the PC.
4. He takes out the ocarina in game, presses Cmd+Enter (Play from cursor at 0:00) → confirm → countdown → playing.
5. At 1:12 the Switch goes to sleep. Playback stops; the banner says "Playback stopped at 1:12 – the Switch went to sleep. Wake it, take the ocarina out, then Resume."
6. He wakes the Switch, takes the ocarina out, clicks "Resume from 1:12" → confirm → countdown.
7. **Climax:** the resumed song plays through to the end and the "You played …" card appears.

Failure: the Switch LED stays "not ready" after waking → "Resume from 1:12" stays disabled (Play gating) and its tooltip names the fix ("Switch: turn on Pro Controller Wired Communication, wake the Switch, check the cable"); once the LED is ok, Resume enables.

### Flow 3 — Composing (Tiziano, writing his own melody)

1. From the empty state he clicks "New song". The split view opens with an empty score.
2. He types notes in the editor; the Roll fills in live; a typo is underlined until fixed.
3. He adjusts notes with the mouse in the Roll (drag one up a lane, stretch another by its right edge, double-click an empty spot to add one); the editor text updates. One drag was wrong: Cmd+Z (or the ↶ button) puts it back.
4. He drags the split divider down to see more lanes, then switches to Staff to read the button glyphs.
5. He clicks the ruler to move the cursor and clicks **Listen** (or presses Space) to hear it on the PC first.
6. Cmd+S → save-as a `.txt` [ASSUMPTION extension] score.
7. Play from start → confirm → countdown.
8. **Climax:** he hears his own melody played on the ocarina in the game.

Failure: tempo too fast → "N notes too short", Play disabled until he lowers the tempo.

### Flow 4 — A stranger sets it up alone (Giulia, A.5 / CAP-10, Windows)

Giulia is a Zelda fan on a Windows laptop. She has never used a terminal or flashed a board. After seeing the project she bought two RP2040-Zero boards and three jumper wires. (Protagonist invented by the facilitator at the owner's request.)

1. She downloads the Windows build from the GitHub release page. SmartScreen blocks it; she follows the README: More info › Run anyway.
2. First launch: the wizard opens at Welcome, "A melody waits to be played."
3. Step 1: she holds BOOT on board 1 and plugs it in. The app sees one RPI-RP2 drive and flashes the bridge. Windows gives no product string, so the step completes when the drive vanishes and a new `2E8A:000A` port appears. Check mark.
4. Step 2: same for board 2, the controller. Check mark.
5. Step 3: she follows the wiring diagram, but connects GP0 to GP0 by mistake, and confirms.
6. Step 4: she plugs the bridge into the PC and the controller into the Switch. PC↔Bridge goes ok; Bridge↔Controller shows the error cross: "No answer from the controller. Check the wires: GP0 goes to GP1, GP1 to GP0." *(failure beat 1)*
7. She unplugs both cables, swaps the two wires, plugs back in. INFO arrives; Bridge↔Controller turns ok; the bridge label now shows its version. Check mark.
8. Step 5: the Switch LED shows the warning triangle "Switch – not ready": Wired Communication is off. *(failure beat 2)* She follows the illustration and the path System Settings › Controllers & Accessories › Pro Controller Wired Communication › On. The LED turns ok.
9. Step 6: three LEDs ok. She clicks A on the virtual gamepad and sees the Switch react. Wizard done; the empty state opens.
10. Using the gamepad panel ("Use it to reach ocarina free play."), she takes the ocarina out in game.
11. She clicks the example "Ode to Joy". Play from start is enabled (all three LEDs ok) → "Ready to play?" → 3·2·1.
12. **Climax:** "You played Ode to Joy!" — without asking anyone for help.

Failure paths exercised: wrong wiring (flow step 6) and Wired Communication off (flow step 8). Covered by state rows: flash failures, foreign board, port pick list.

Not covered by a flow: the required/recommended update path (CAP-6; states only). See Open Questions.

## Inspiration & Anti-patterns

- **RiiDraw** (closed source, same two-RP2040 shape): one-time flash via in-app buttons after BOOT; virtual gamepad appears once connected; drop file → adjust → one big action button; casual, reassuring copy; UI evokes its game without copying it. Lifted: all of these.
- **TomodachiDrawer-V2** (`xoaninc/TomodachiDrawer-V2`, GPL-3.0): status text that names the fix ("hold BOOT, tap RESET…"); welcome re-openable from Help; manual drag-and-drop UF2 fallback; persistent log. Lifted: fix-in-status-text, flash-manually fallback, diagnostics.
- **Friend Maker** (`zhouxiyu1997/friendmaker`): staged connection tiles gating Run; pre-run confirm listing assumed game state; on-screen test pad; actual vs expected time after a run. Lifted: three-stage LEDs gating Play, readiness confirm with "Don't ask again", end card with actual vs expected.
- **Pitfalls avoided:** a dense window that doesn't fit small screens (TD #133) → min size + compact gamepad + scrolling lanes; dialogs that apologise without a next step → every error names the fix; no resume after interruption (TD) → Resume from the stop point; users reporting "error" with no data (FM #82) → Copy diagnostics; tab order not matching the documented flow (FM) → one screen, left-to-right order with manual region layout; a cancel that hides what it really stops (TD) → STOP is honest and immediate.

## Responsive & Platform

| Window width | Behaviour |
|---|---|
| ≥ 1280 px | Full layout: song panel, note view, full gamepad with silhouette and key chips. Extra width goes to the note view. |
| 1024–1279 px | Gamepad switches to compact (Story 4.9: the region is `spacing.gamepad-compact-width`, 200 pt, the note view takes the 100 pt it frees; buttons only, the sticks not shown, key hints in hover tooltips; owner 2026-11-28). Song panel can be collapsed to the song rail. Note view keeps priority. The bottom bar keeps all four big buttons; the safety note wraps [ASSUMPTION]. |
| < 1024 × 680 | Not allowed (minimum window size, logical points). A 1366 × 768 laptop at 125 % scaling (1093 × 614 pt) cannot fit it and needs 100 % scaling; accepted. |

Height: lanes shrink to `{spacing.lane-height-min}`, then the lane area scrolls vertically. In the text-score split the editor keeps at least 4 lines and the user sets the divider. Empty out-of-range zones collapse to a thin strip.

| Platform | Rules |
|---|---|
| macOS | Show and open only `/dev/cu.*`, never `/dev/tty.*` (AD-18). RPI-RP2 copy uses a plain read/write loop (spike). Universal `.app`, unsigned; unblock via README. |
| Windows | `serialport` gives no product string: VID:PID + HELLO probe; wizard step 1 completes on drive-vanish + new port; explicit port pick before any 1200-baud touch (AD-13). SmartScreen unblock via README. |
| Linux | Built **without eframe's `wayland` feature**, so the AppImage runs on X11/XWayland: winit 0.30 delivers file drops only on X11 (not native Wayland), and window decorations stay the system's. Open… always works. `dialout` permission and udisks mounting detected and explained with exact commands; never escalate privileges. |
| All | Offline only; no network connections (AD-21). |

## Hardware & Protocol Coupling

What the UI can know, and when. Sources: `docs/link-protocol.md`, ARCHITECTURE-SPINE AD-5/7/8/13/19.

| Signal | Arrives when | UI consequence |
|---|---|---|
| USB enumeration (VID:PID `2E8A:000A`, product string) | any time; port rescan every 1 s [ASSUMPTION interval] | PC↔Bridge: none → `error`; foreign product string → `warning`; "Bonooru Bridge" → `ok` (no version yet). |
| `INFO` (answer to HELLO, sent every 100 ms) | only when a controller is wired and powered | Confirms the full bridge+controller path. Gives `proto_ver` (mismatch → required update), `fw_ver`, `bridge_ver` (≠ app → recommended update; fills LED labels), `queue_cap`, `app_timeout_ms`, `report_period_us`. Bridge↔Controller → `ok`. |
| No INFO / no STATUS | bridge alone, or no STATUS for 150 ms (3 × the STATUS period) | Bridge↔Controller `error`, Switch `unknown`; bridge version unknown. |
| `STATUS.flags.link_ok` | every 50 ms | 0 → Bridge↔Controller `error`; during countdown → cancel. |
| `STATUS.flags.console_ready` | every 50 ms | 1 → Switch `ok`; 0 → Switch `warning` "not ready" (during countdown → cancel). Drops on USB suspend/unmount/bus reset and when the console stops polling > 100 ms (verified on hardware, Story 3.5 H.3: the console polls every 8 ms, worst gap 24 ms; Wired Communication OFF shows as 0 within 150 ms of its last poll). In sleep the console cuts USB power: the controller turns off, STATUS stops, and after wake the app sees "controller rebooted" and HELLOs again. |
| `STATUS.mode`, `now_us` | every 50 ms | Mode drives Play/STOP state. The playhead and time **interpolate locally** from the last `now_us` plus elapsed time and re-sync on each STATUS (otherwise they step every 50 ms); 0 during lead. The gamepad visualizer reads the compiled timeline at the same interpolated time. |
| `NAK NOT_READY` | on START / SET_STATE before handshake | Same as `console_ready` = 0. |
| `NAK NO_LINK` | on START / SET_STATE while link down | Same as `link_ok` = 0. |
| `NAK BUSY` | SET_STATE during Play | Should not happen (gamepad locked); logged. |
| `NAK ABORTED` | after an unrequested abort | App sends STOP to clear the latch. |
| `NAK VERSION` mid-session | controller rebooted | App re-sends HELLO. |
| `EVENT PLAYBACK_DONE` | END applied | End-of-song card. |
| `EVENT UNDERRUN` / `LATE` / `LINK_LOST` / `USB_SUSPENDED` | during Play | Interruption banner with cause + Resume; app sends STOP. |
| Unknown EVENT / NAK codes | — | Logged, never fatal. |
| Port disappears | USB unplug | PC↔Bridge `error`; during countdown → cancel; during Play → banner. |

Gaps: on Windows with the bridge alone, the main screen cannot confirm the board is ours (no product string, no INFO): it shows PC↔Bridge `unknown` "Bridge" with an explaining tooltip until INFO (resolved Open Question 4, provisional until issue #43). The STATUS staleness timeout is closed: 150 ms without a STATUS → Bridge↔Controller `error`, Switch `unknown` (Open Question 3, resolved 2026-11-26).

## Scope deltas vs spec

Additions or changes this UX makes beyond `SPEC.md`; flag for spec/epics update (see also Upstream edits required).

| Delta | Spec says | UX decision |
|---|---|---|
| Piano-roll note drawing | Open `.mid` / text only (FR-01/02) | Mouse add/move/resize/delete in Roll and Staff, written back to the text score |
| In-app text editor | Open text files (FR-02) | Split editor with live note view, inline errors, draggable divider |
| `.mid` → text score conversion | — | On first edit; original never modified |
| Per-note out-of-range edits | Auto fold/drop, reported (FR-04) | Dimmed in the note view; move by octave, delete, or leave the proposal per note |
| App-level undo / redo | — | One history for text, note, out-of-range, transpose and tempo edits |
| Staff view | Timeline with fingerings (FR-12) | Second view with button glyphs |
| Full gamepad incl. Home/Capture/sticks | Subset unspecified (FR-11) | Full Pro Controller set, no extra protection |
| Switch LED / `console_ready` | STATUS had `link_ok` only | Protocol amended in place (proto v1): STATUS flags bit1 `console_ready`. Already applied to link-protocol.md, ARCHITECTURE-SPINE AD-19/AD-8, usb-controller-emulation.md, test-plan F.10/H.3 (uncommitted); to verify on hardware (H.3). |
| Play gated on 3 LEDs | — | Both Play actions disabled until all links are ok |
| Mapping-profile timing fields editable in GUI | Profile file (CAP-3) | Advanced settings fields + Open profile file |
| Persistence, recents, drafts, examples, diagnostics | Not specified | As in Component Patterns |
| Calibration wizard (CAP-12, M10) | M10 | Out of this UX pass; no calibration step |
| Live MIDI (CAP-11, M10) | M10 | Not designed; no reserved place |

## Upstream edits required

Edits to source documents that this UX implies. Not applied by this pass. Plan: once the UX is finalized, run `bmad-spec` (Update) for the scope deltas and `bmad-architecture` (Update) for the ADR-002 / ARCHITECTURE-SPINE screenshot wording and the Linux X11 build note, as two separate subagents.

| Document | Location | Edit |
|---|---|---|
| `_bmad-output/specs/spec-bonooru/decisions.md` | ADR-002, line "the GUI setup wizard shows this step with a screenshot" | Replace "with a screenshot" with "with an original illustration and the exact menu path (System Settings › Controllers & Accessories › Pro Controller Wired Communication › On); no console screenshots in the app or README". |
| `ARCHITECTURE-SPINE.md` | Conventions › UI safety content (the "Pro Controller Wired Communication" step) | Add "shown as an original illustration + menu path, never a console screenshot". |
| `SPEC.md` / `requirements.md` (FR-01, FR-02, FR-04, FR-12) and epics | Scope | Add: in-app text editor (split view, live note view, inline errors); piano-roll note drawing (add/move/resize/delete, written back to the score); `.mid` → text score conversion on first edit; per-note out-of-range edits (octave move, delete); app-level undo/redo. Also note: Staff view, full gamepad set, Play gated on three LEDs. |
| `ARCHITECTURE-SPINE.md` | Build / platform (AD-18 or build conventions) | Linux build: eframe without the `wayland` feature (X11/XWayland) so file drag-and-drop works with winit 0.30; add `rfd` for file dialogs and `egui_extras` image loaders for the wiring photo. |

## Open Questions

Resolved and folded into the spines: note selection and editing model (incl. double-click add); key-screen mocks (rendered, approved, promoted to `mockups/`); Play tooltip reason order; "Play from cursor" label; visible Undo/Redo; required-update dialog.

All remaining questions are **non-blocking**, to resolve during build or hardware testing:

1. **Ocarina auto-open — RESOLVED 2026-11-20: left to the user.** The app does not take the ocarina out before Play. Reason (U9, Story 1.1): D-pad ← is a fixed toggle that both opens and closes the ocarina, and the app cannot see the game, so an automatic press would close an ocarina the user already opened; the game also takes note input only ~2 s after opening, and Link must be somewhere he can play. The readiness confirm keeps asking the user to have the ocarina open in free play.
2. **Fingering chart — RESOLVED 2026-11-20 for the 19 chart fingerings** (Story 1.1): every fingering is right, but the game sounds one octave above the community chart, so the app names pitches by sounding pitch (B4–F6). The mocks still show the old chart names (B3–F5, one octave low): read them +1 octave. Alternative fingerings: **resolved by Story 1.3 (chart data)**, 2026-11-20: all 45 button × modifier combinations measured, 1 to 4 fingerings per pitch (`docs/ocarina-input-mechanics.md`, "Alternative fingerings"); the left stick is a graded bend, used at full vertical travel. The copy/screenshot check stays with Epic 11 (OQ-2 in `epics.md`).
3. **STATUS staleness timeout — RESOLVED 2026-11-26 (owner, Story 3.5 H.3): 150 ms** (3 × the 50 ms STATUS period) without a STATUS → Bridge↔Controller `error` "Controller – not answering", Switch `unknown` (as the coupling table and "Link lost (idle)"; aligned 2026-11-27, Story 4.5 review). Measured on the own bridge: STATUS interval median 50.0 ms, p99 50.1 ms, max 52.4 ms over 1852 frames (macOS host); Windows and Linux app-side timing to be checked in Epic 4. The companion `console_ready` 100 ms polling-stall rule (`epics.md` OQ-4) is kept as is (worst console polling gap 24 ms; OFF detected 144 ms after the last poll); see the Play gating row and the coupling table.
4. **Windows, bridge alone, main screen — RESOLVED 2026-11-27 (Story 4.5) for the main screen; Windows data pending.** An `Unconfirmed` port (no usable product string: always on Windows, elsewhere when the OS gives none) that is open with no INFO shows PC↔Bridge `unknown` "Bridge" (hollow ring, no detail) with the tooltip "A board that may be the bridge is on <port>. Bonooru can confirm it once the controller answers." and the fix "Wire the controller to the bridge and plug it into the Switch."; Bridge↔Controller `unknown`, then `error` "Controller – not answering" after the 500 ms INFO grace; Switch `unknown`. As soon as INFO arrives the port is confirmed and shown with versions. Reason: the main screen never claims a board is ours without proof (UX-DR11; an unconfirmed board is only probed with HELLO, AD-13) and does not show an error for a board that may well be fine; `warning` stays reserved for a foreign board. macOS data: H.28 and H.31 (Story 4.4). Provisional until issue #43: Windows H.28 (`class=unconfirmed`, `product_raw` recorded) and H.31 (the bridge alone accepts HELLO through `usbser.sys`) confirm the inputs, Windows H.33 lets the owner see the strip, and the round-robin flicker with several unconfirmed boards (Story 4.5 D12) is looked at then. Same question as `epics.md` OQ-5.
5. **Bridge LED label without INFO — RESOLVED 2026-11-27 (Story 4.5): "Bridge – found".** An `Ours` port (`2E8A:000A`, product exactly "Bonooru Bridge") open with no INFO shows PC↔Bridge `ok` "Bridge – found" (dot with glow, no version), Bridge↔Controller `unknown` during the INFO grace, then `error` "Controller – not answering", Switch `unknown`. macOS data: H.28 reports `class=ours` with the product string before any INFO; H.31 the bridge alone accepted HELLO every 100 ms for 10 s with `hello_dropped=0`, so "bridge found, controller not answering" is observable as port open + no INFO. Provisional until issue #43: Linux must also report the product string (H.28 on Linux) and accept HELLO alone (H.31 on Linux); a Linux bridge without the product string falls under item 4 by its class, so no code change is expected, only this sentence. Same question as `epics.md` OQ-6.
6. **Visual fine-tuning — RESOLVED 2026-11-27 (owner, Story 4.1 token gallery `bonooru --gallery`): every proposed value kept.** `status-warning #f08a4b`, `status-error #ef6a55`, `status-unknown #9a8a72`, `amber-ink #8a5a12`, `scrim #0b0806b3` and the editor font Hack 13 px; DESIGN.md's `[ASSUMPTION]` tags for these are removed. Contrast floors checked by G.6 (`docs/test-plan.md`). Same question as `epics.md` OQ-7.
7. **Update flow (CAP-6):** add a Key Flow for required/recommended update → wizard → reflash → Play re-enabled, or accept states-only coverage.
8. **Screen reader / AccessKit — RESOLVED 2026-11-27 (owner, Story 4.2): basic.** Shared helpers and custom widgets emit `widget_info` with the closest role and the visible label; the Roll and the Staff are one summary node each; eframe's `accesskit` feature stays on. Reason: one call per helper now instead of a retrofit over every widget later, and it makes custom widgets queryable by role + name in `egui_kittest`; "none" saved little (Linux dependency graph only), "full" meant weeks of work, keyboard note editing and manual screen-reader tests on three OS. See Accessibility Floor. Same question as `epics.md` OQ-9.
9. **Text score file extension** and save-as defaults; grammar details are deferred to `score` (SPINE).
10. **Bundled example list:** memlog names Ode to Joy, Greensleeves, Frère Jacques; the layouts mock also shows Twinkle Twinkle and a B4–F6 scale. Final 3–5?
11. **Wiring photo:** a real photo of the owner's assembly is required (SPINE convention); not yet taken.
12. **Draft microcopy:** one approval pass over every "draft" string (Voice table and State rows).
13. **Fit proposal actions:** explicit "Accept all" / "Reset to proposal" buttons, or Play-accepts only?
