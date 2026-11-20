# Spine Pair Review — ocarina-bot

Reviewed: `DESIGN.md` (542 lines) + `EXPERIENCE.md` (391 lines), against the bmad-ux examples (Quill mobile, Drift shadcn, editorial) and the 11 `sources` in EXPERIENCE.md frontmatter. `.memlog.md` treated as canonical. The prior self-check (`.working/finalize-gaps.md`) was re-verified, not copied: its token, contrast and conflict-resolution claims hold; this review adds findings it missed (countdown contrast, import-failure states, Windows wizard step 1, focus-ring spec, red-usage contradiction, glossary drift).

## Overall verdict

Adequate. A downstream architect or story developer can source-extract the pair cleanly: every `{token}` reference resolves, every source path resolves, memlog overrides are reflected, and the Hardware & Protocol Coupling table is an unusually strong bridge to the firmware contract. No critical findings. Four high findings would each make a downstream story guess: the spec's own release signal (A.5, a first-time non-developer) has no flow, song-import failures have no states, wizard step 1 cannot complete on Windows as written, and the countdown overlay fails contrast at the moment the user glances over from the console.

## 1. Flow coverage — adequate

Sources have no named user journeys; the requirement names are CAP-1…CAP-12 (SPEC.md). Checked each CAP against the three Key Flows for protagonist, numbered steps, climax and failure path. All three flows have a named protagonist (Tiziano), numbered steps, a bold climax and a failure line. CAP-1, 2, 4, 7, 8, 9 are exercised; CAP-5 is covered by states; CAP-11/12 are out of scope by decision.

### Findings
- **high** No flow for CAP-10 / A.5: a first-time user who is not the developer unblocks the app via the README, flashes both boards and plays with the GUI only. This is the SPEC's release success signal, and all three flows use the owner, who already knows the hardware (EXPERIENCE › Key Flows, line 304; Open Question 12). *Fix:* add Flow 4 with a non-developer protagonist on Windows. That OS has no product string, so the flow also exercises the explicit port pick, the unsigned-app README path and wiring for the first time. Or record an explicit deferral in the memlog.
- **medium** No flow for CAP-6 update offers (required = blocking dialog → wizard at a flash step; recommended = banner → Update/Later). Today the path from "Update now" back to a playing state is defined only by table rows (State Patterns, lines 176–177). *Fix:* add a short flow: the app is updated, the old firmware is detected, the user clicks "Update now", reflashes one or both boards, the LEDs come back, and Play is re-enabled.
- **low** CAP-3 (editing the mapping profile in Settings › Advanced) and the AD-15 CSV export have no flow. *Fix:* acceptable as states only; resolve Open Question 18 for CSV.
- **low** Flow 2 step 3 "accepts the other" uses an accept action that is itself [ASSUMPTION] (Component Patterns › Fit proposal: Play implicitly accepts). *Fix:* reword to "leaves the other as proposed" or decide on "Accept all".

## 2. Token completeness — adequate

Extracted every frontmatter token (44 colours, 22 typography, 6 rounded, 30 spacing, 46 components) and every `{…}` reference: DESIGN.md has 88 and all resolve. EXPERIENCE.md has 21; the only unresolved one is the literal syntax example `{path.to.token}` in Foundation. All colours are valid hex, with 8-digit hex for alpha. The theme is dark-only by decision, so no light pairs are needed. Re-computed the stated contrast ratios and all match: cream/frame 14.8, ink/parchment 11.7, amber-ink/parchment 4.35, out-of-range label 3.3, status colours on panel-raised 5.1–9.4, warning vs amber 1.34.

### Findings
- **high** Countdown overlay contrast is not stated and fails. The `veil` (#16110c at 55 %) over parchment composites to about #776d57. Amber numerals on it measure **2.75:1**, below 3:1 even for large text. The cream sub-line "hands off the controller" (14 px semibold) measures **4.02:1**, below 4.5:1. This is the one overlay the user reads by glancing over from the console (DESIGN › Components › Countdown; `countdown-veil`, `countdown-numeral`). *Fix:* raise the veil alpha to about 80 %, or draw the numerals in parchment/cream with an amber glow, and state the resulting ratio.
- **medium** Six tokens still carry [ASSUMPTION] and need owner approval before code mirrors them: `status-warning`, `status-error`, `status-unknown`, `amber-ink`, `scrim` and `typography.code`. The ratios are acceptable (≥ 5:1 on panel-raised), but warning and amber are only 1.34:1 apart in luminance, so the triangle symbol carries all the meaning (DESIGN frontmatter lines 36, 48–52, 57, 150). *Fix:* approve or replace them, then drop the tags.
- **medium** The out-of-range label stays at 3.3:1 (11 px), with an alternative (`#55504a`, 4.7:1) parked in Do's and Don'ts. Two candidate values leave the implementer guessing (DESIGN Colors line 433, Do's line 541). *Fix:* commit one value in frontmatter.
- **low** `button-stop-live.foreground` is a literal `#ffffff`, the only hard-coded hex in components (5.41:1 on ember, fine). *Fix:* reuse `on-ember` or add an `on-stop` token.
- **low** Many component dimensions live only in prose: key chip 13×13, stick ring 17 / knob 11, D-pad well 22, ABXY well 31 / buttons 10, playhead handle 12, staff bar 5, banner padding 16/20, big-button side padding 22 (DESIGN › Components). *Fix:* either tokenise the gamepad geometry or state that prose values are the spec.
- **low** Some load-bearing pairs have no stated ratio: ghost-note fill vs zone (1.27:1; the dashed border at 3.29:1 carries it), ember playhead on parchment (3.98:1), and on-ember label (4.96:1). *Fix:* add one line to Colors.

## 3. Component coverage — adequate

Cross-checked the 46 DESIGN component tokens and every named component in both spines' prose against DESIGN › Components (visual) and EXPERIENCE › Component Patterns (behaviour). Core components (status LED, track picker, transpose, tempo, note view Roll/Staff, out-of-range, official-song group, gamepad, Play/STOP, banner, countdown, end card, wizard step list) have real rules on both sides. Decorative-only items (corner bracket, panel tab, rim) need no behavioural row.

### Findings
- **medium** There is no visual spec for the generic focus ring. The Accessibility Floor requires a visible ring on **every** focusable control (EXPERIENCE lines 205 and 254), but DESIGN specifies only the gamepad panel focus (`gamepad-panel-focused`). *Fix:* add a `focus-ring` component (e.g. 2 px `{colors.amber}` outer stroke, `{rounded.*}` matching the control), plus its variant on parchment.
- **medium** EXPERIENCE uses components with no DESIGN row: the wizard flasher **progress bar**, the **checkbox** ("Don't ask again"), the out-of-range **context menu**, the **log view**, the Settings **Advanced fields** and collapsible section, the Windows **port-pick list**, the **inline hint/message** (bridge not found, "gamepad clicks ignored"), and the collapsed **song rail** icons. The example/Recent chips are marked [ASSUMPTION]. *Fix:* add short visual rows, or state "egui default, themed with tokens X/Y".
- **medium** The note selection model is unspecified: single click, Shift/Cmd multi-select, marquee, whether selection spans the out-of-range zones, and keyboard selection. Yet `note-bar-selected`, Delete, drag-move and the context menu all depend on it (EXPERIENCE › Note editing, line 138). *Fix:* add a selection row to Component Patterns.
- **low** The gamepad visualizer "mirrors what the controller sends", but the protocol echoes no input state. The visualizer must be derived from the compiled timeline at `STATUS.now_us` (EXPERIENCE line 144; link-protocol STATUS body). *Fix:* say so, so the story doesn't hunt for a non-existent message.
- **low** The song-panel collapse to a 48 px rail has no behaviour: whether it collapses automatically below 1280 px or via a user toggle, which icons it shows, and how it expands (Responsive line 318; DESIGN Layout). *Fix:* add a Component Patterns row.
- **low** Tooltip behaviour is undefined (delay, shown on keyboard focus). The LED's fix text lives only in the tooltip, so keyboard users may never see it. *Fix:* show the tooltip on focus as well as hover; consider a click-to-wizard action (currently [ASSUMPTION], line 128).

## 4. State coverage — adequate

Walked every IA surface (main screen regions, wizard steps 0–6, Settings, dialogs, log view) against empty / cold-load / focus / error / offline / permission-denied / in-progress. The State Patterns table is broad: connection states, flash failures ×4 plus manual fallback, playback interruptions, drafts, the Linux permission states, and focus.

### Findings
- **high** Song import failures are missing. There is no state for: an unreadable or corrupt `.mid`; a dropped file of an unsupported type (MusicXML/ABC are spec non-goals, ADR-007: "export to MIDI"); a `.mid` with no notes or only percussion; a track at 0 % in range; or the last song or a Recent entry missing on launch (Persistence, line 157). CAP-1 is a Must. *Fix:* add rows with plain copy and the fallback screen (empty state plus banner).
- **high** Wizard step 1 on Windows cannot complete as written. "Done when: bridge product string seen after flash" (IA table, line 56). On Windows `serialport` reports no product string, and INFO needs the controller, which isn't flashed or wired yet (Coupling lines 337–338, 352). A.4 requires the wizard on each OS. *Fix:* on Windows, mark step 1 done when the RPI-RP2 drive vanishes and a new `2E8A:000A` port appears, and defer confirmation to step 4 (INFO). State the copy.
- **medium** There is no state for a link or console change **during the countdown** (before START). If `console_ready` drops, the bridge is unplugged, or `link_ok` = 0 while numerals are showing, should the countdown abort with a banner, or should START go out and get NAK NOT_READY? *Fix:* add a row. The countdown should abort with the same interruption banner, with no START sent.
- **medium** Two items are still open and were verified as unresolved: the STATUS staleness timeout (the Link-lost idle row says "timeout: Open Question") and how a Windows bridge without a controller is shown (Open Questions 5 and 6). Both decide LED behaviour for the most common "nothing works" moment. *Fix:* decide; a candidate is staleness at 3× the STATUS period (150 ms) → LEDs `unknown`.
- **low** The Settings state for a profile that cannot be migrated (AD-17 "unmigratable copy refused with a message") is absent. Gamepad behaviour in Link lost (idle) is unspecified. So is drop or Open during Play. *Fix:* one row each.
- **low** About 14 operational strings are "draft" [ASSUMPTION] (Voice table and the State rows for underrun, LATE, link lost, unplug, flash failures 2–4, required update, audio). *Fix:* run one approval pass.

## 5. Visual reference coverage — thin

Files: `.working/layouts-main-screen-2026-11-20.html`, `.working/directions-oot-2026-11-20.html`, and 7 PNGs in `imports/`. Both HTML mocks are linked inline (EXPERIENCE intro, DESIGN › Layout & Spacing). "Spines win on conflict" is stated once (EXPERIENCE line 21), with the known mock drift listed. The 7 `imports/` PNGs are described in words in DESIGN › Brand & Style and deliberately never linked: they are copyrighted, git-ignored (`.gitignore:34`, verified), and the decision bans links. They are **not** orphans. Verified that neither mock embeds or references an `imports/` file.

### Findings
- **medium** The mocks still live in `.working/` (scratch) and are linked only from the EXPERIENCE intro and DESIGN Layout, never at the sections they illustrate: the IA main screen, the out-of-range zone, the official-song group, countdown, the gamepad focus and lit states, the banner. *Fix:* promote them to `mockups/` and add a "→ reference: <file>, direction 3 / variant A" line in each relevant section.
- **medium** There are no Hybrid mocks for the screens that A.4/A.5 depend on most: the setup wizard, the readiness confirm, Settings, the empty state, the end card, the compact gamepad and the 1024 px layout. Their DESIGN rows are all [ASSUMPTION]. *Fix:* render key-screen mocks for the wizard (with the wiring diagram), the empty state and the confirm/countdown sequence before story writing.
- **low** DESIGN › Staff view says "mock shows this alternation" without naming the file or direction. Staff exists only in direction 2 (Parchment), not in Hybrid (memlog). *Fix:* cite `.working/directions-oot-2026-11-20.html`, direction 2.

## 6. Bloat & overspecification — adequate

The pair is mostly tables, and EXPERIENCE prose is operational. The Hardware & Protocol Coupling table restates protocol timing, but every row ends in a UI consequence, so it earns its place. DESIGN carries editorial voice only in Brand & Style, as allowed.

### Findings
- **low** The DESIGN Layout table and prose repeat token values inline (`{spacing.status-strip-height}` 44, `{spacing.song-panel-width}` 260 …), giving two sources of truth if a token changes. *Fix:* drop the inline numbers or mark them "currently".
- **low** The mood-reference paragraph in DESIGN › Brand & Style (line 419) describes seven screenshots in detail. It is useful context, but no implementer acts on it. *Fix:* trim to the traits actually lifted (parchment card, corner brackets, glow selection, dialogue banner, controls-sheet callouts), or move it to the memlog.

## 7. Inheritance discipline — adequate

All 11 `sources` paths resolve. CAP/FR/AD IDs are cited verbatim. Memlog overrides are reflected correctly: full gamepad set, vim-positional HJKL (H=Y J=B K=X L=A), out-of-range dimmed in the note view, piano-roll drawing in v1, evoke-don't-copy confirmed. EXPERIENCE token references resolve to DESIGN tokens by name.

### Findings
- **medium** Glossary drift. The spec glossary defines **mapping profile** (TOML with pitches, modifiers, timing, range; `architecture.md` Glossary, AD-17). EXPERIENCE says "timing profile" (IA Settings line 69, Component Patterns › Settings modal line 154), and DESIGN shows "Profile: Default · Switch 2". *Fix:* use "mapping profile" everywhere, including UI copy.
- **medium** DESIGN contradicts itself on red. Colors › Avoid says "red used for anything other than the official-song warning, the playhead and live STOP" (line 437), yet `status-error #ef6a55` (red) is used for LED errors, the tempo "too short" value and editor underlines (lines 434, 494, 496). Do's and Don'ts only bans red "inside the note view". *Fix:* scope the Avoid line to `ember`, or to the note view, and name `status-error` as the error red.
- **medium** Unresolved source conflict: ADR-002 (`decisions.md` line 16) says the wizard shows the Wired Communication step "with a screenshot", which clashes with the confirmed no-screenshots constraint (Open Question 3). ARCHITECTURE-SPINE › UI safety content requires the step but not a screenshot. *Fix:* decide on the text path plus an original diagram, and log an ADR-002 amendment.
- **low** Component names drift between files: "Track picker" (EXPERIENCE) vs "Track select"/`select` (DESIGN); "Interruption banner"/"Update banner" vs "Dialogue-style banner"/"slim variant"; "Readiness confirm" vs generic "Modal dialog"; "Fit proposal" vs "fold target"/"ghost note"; spec "dry run" (CAP-9) vs UI "Preview". *Fix:* add one name column mapping, or align the names.
- **low** The Voice rule "No Nintendo names in our copy" (EXPERIENCE line 85) is contradicted by necessary strings: "the Switch", "Pro Controller Wired Communication", and "Switch 2" in the profile hint. *Fix:* scope it to franchise, game, character and logo names.
- **low** Two specifics are unsourced and untagged: "rescan every 1 s" (Coupling line 336), and "Esc stops at any time" credited to a "SPINE convention" (Voice line 117), although ARCHITECTURE-SPINE › UI safety content mandates only "don't dock mid-song". *Fix:* tag both [ASSUMPTION] or cite a source.

## 8. Shape fit — strong

DESIGN.md sections are in canonical order (Brand & Style → Colors → Typography → Layout & Spacing → Elevation & Depth → Shapes → Components → Do's and Don'ts), none dropped. EXPERIENCE.md has every required default (Foundation, IA, Voice and Tone, Component Patterns, State Patterns, Interaction Primitives, Accessibility Floor, Key Flows). The required-when-applicable sections are present and triggered: Inspiration & Anti-patterns (RiiDraw, TomodachiDrawer-V2 and Friend Maker in the memlog) and Responsive & Platform (3 OS, breakpoints). The invented sections earn their place. Hardware & Protocol Coupling is the contract between the UI and the device, Scope deltas flags spec/epic updates, and Open Questions keeps the [ASSUMPTION]s traceable. No findings.

## Mechanical notes

- **Frontmatter:** EXPERIENCE has `name`, `status: draft`, `updated` and 11 `sources`, all resolving. DESIGN has `name`, `status: draft`, `updated` and `description`, with no `sources`, matching the examples. Both are still `draft`.
- **Cross-refs:** 88/88 DESIGN references resolve. 20/21 EXPERIENCE references resolve; the exception is the literal `{path.to.token}` syntax example (Foundation), which is harmless but could be backticked as non-reference text. `colors.ink-ruler` is defined but referenced only by bare name in prose ("ink-ruler ticks").
- **Mermaid:** none in either spine; nothing to validate.
- **Memlog vs spine:** every override is applied. The memlog's interruption list includes "app timeout", but per link-protocol there is no app-level timeout during playback. The spine correctly has no such banner row, so no action is needed.
- **Mock drift:** the drift EXPERIENCE line 21 lists ("Switch linked", "?" on Home/Capture/+/−, spike keys J K I U) is still accurate. The spines win.
- **Fingerings:** every fingering string in the lane headers, staff glyphs and gamepad captions is unverified (Open Question 2). Release copy and screenshots must wait for M2.
- **Severity counts:** critical 0 · high 4 · medium 13 · low 16.
