---
name: bonooru
status: final
updated: 2026-11-27
description: Desktop app (Rust + egui/eframe) that plays melodies on the ocarina in Ocarina of Time (Switch 2) through two RP2040 boards. Dark warm frame, music on a parchment card, amber glow for selection. Direction "Hybrid". Dark only.
colors:
  # Frame and panels (dark, warm). Values lifted from mockups/directions-oot-2026-11-20.html, direction 3.
  frame: '#16110c'
  panel: '#1c150f'
  panel-raised: '#221a13'
  well: '#2a1e13'
  control: '#56402a'
  rule: '#3a2a1b'
  rule-strong: '#4a3826'
  rim: '#a37a3e'
  rim-soft: '#a37a3e66'
  text: '#f3e3bd'
  text-muted: '#c4a676'
  # Parchment card (note view, end-of-song card, diagrams and illustrations)
  parchment: '#ecdcb2'
  parchment-lane-alt: '#e4d2a5'
  parchment-header: '#e2cf9f'
  parchment-header-alt: '#d9c491'
  parchment-ruler: '#dcc794'
  parchment-rule: '#d8c595'
  parchment-grid: '#b59a63'
  ink: '#2b2015'
  ink-soft: '#6e5132'
  ink-ruler: '#4a3520'
  bracket: '#7a5a33'
  note: '#5b3d22'
  # Accent: amber glow (selection, focus, primary action, lit keys)
  amber: '#f0b43c'
  amber-glow: '#f0b43c66'
  amber-wash: '#f0b43c22'
  amber-ink: '#8a5a12'            # parchment-safe accent (confirmed 2026-11-27, OQ-7) (song name on end card, focus ring on parchment); amber itself is ~1.5:1 on parchment
  # Ember: official-song warning, playhead, live STOP
  ember: '#b8432f'
  ember-wash: '#b8432f2e'
  ember-text: '#f2a58f'
  on-ember: '#fff4e0'
  stop-idle-fill: '#3a1712'
  stop-idle-text: '#e3a597'
  # Out-of-range zone (dimmed)
  out-of-range-zone: '#cfc6b4'
  out-of-range-ink: '#55504a'     # 4.71:1 on out-of-range-zone (AA for the 11 px label)
  out-of-range-note: '#b9b0a0'
  # Status (LEDs, banners, errors). status-ok is lifted from the mock; the other three hexes were confirmed by the owner 2026-11-27 (OQ-7, Story 4.1 gallery review).
  status-ok: '#8fd16f'
  status-warning: '#f08a4b'       # orange, hue ~22deg vs amber ~40deg; never used without the triangle symbol
  status-error: '#ef6a55'         # the error red itself is decided (LED error, too-short value, editor underline)
  status-unknown: '#9a8a72'       # confirmed 2026-11-27 (OQ-7)
  # Overlays (8-digit hex = alpha, egui Color32::from_rgba_unmultiplied)
  banner-fill: '#1c1610e0'
  banner-border: '#e9dcbc66'
  veil: '#16110ccc'               # 80 %; over parchment composites to ~#413a2d
  scrim: '#0b0806b3'              # modal backdrop (confirmed 2026-11-27, OQ-7)
typography:
  brand:
    fontFamily: Cormorant Garamond
    fontSize: 23px
    fontWeight: '700'
  dialog-title:
    fontFamily: Cormorant Garamond
    fontSize: 21px
    fontWeight: '700'
    lineHeight: '1.2'
  panel-tab:
    fontFamily: Cormorant Garamond
    fontSize: 16px
    fontWeight: '700'
  card-title:
    fontFamily: Cormorant Garamond
    fontSize: 24px
    fontWeight: '700'
    lineHeight: '1.2'
  countdown:
    fontFamily: Cormorant Garamond
    fontSize: 128px
    fontWeight: '700'
    lineHeight: '1'
  body:
    fontFamily: Nunito Sans
    fontSize: 13.5px
    fontWeight: '400'
    lineHeight: '1.45'
  body-strong:
    fontFamily: Nunito Sans
    fontSize: 13.5px
    fontWeight: '600'
  banner-text:
    fontFamily: Nunito Sans
    fontSize: 15px
    fontWeight: '400'
    lineHeight: '1.4'
  dialog-body:
    fontFamily: Nunito Sans
    fontSize: 14.5px
    fontWeight: '400'
    lineHeight: '1.5'
  label:
    fontFamily: Nunito Sans
    fontSize: 11.5px
    fontWeight: '700'
    letterSpacing: 0.06em
  small:
    fontFamily: Nunito Sans
    fontSize: 12.5px
    fontWeight: '400'
  filename:
    fontFamily: Nunito Sans
    fontSize: 15px
    fontWeight: '600'
  value-lg:
    fontFamily: Nunito Sans
    fontSize: 24px
    fontWeight: '700'
  value-md:
    fontFamily: Nunito Sans
    fontSize: 17px
    fontWeight: '700'
  button-lg:
    fontFamily: Nunito Sans
    fontSize: 17px
    fontWeight: '700'
    lineHeight: '1.1'
  button-lg-sub:
    fontFamily: Nunito Sans
    fontSize: 12.5px
    fontWeight: '400'
  button-md:
    fontFamily: Nunito Sans
    fontSize: 14px
    fontWeight: '700'
  lane-name:
    fontFamily: Nunito Sans
    fontSize: 12px
    fontWeight: '700'
  lane-fingering:
    fontFamily: Nunito Sans
    fontSize: 10.5px
    fontWeight: '400'
  ruler:
    fontFamily: Nunito Sans
    fontSize: 10.5px
    fontWeight: '400'
  note-label:
    fontFamily: Nunito Sans
    fontSize: 11.5px
    fontWeight: '700'
  glyph:
    fontFamily: Nunito Sans
    fontSize: 10.5px
    fontWeight: '700'
  key-chip:
    fontFamily: Nunito Sans
    fontSize: 10px
    fontWeight: '600'
  safety-note:                      # lifted from mockups/key-empty-state.html (Story 4.7)
    fontFamily: Nunito Sans
    fontSize: 13px
    fontWeight: '400'
    lineHeight: '1.35'
  code:
    note: 'egui bundled monospace (Hack, default_fonts feature) for the text-score editor and log view; size 13px (confirmed 2026-11-27, OQ-7)'
rounded:
  xs: 2px
  sm: 4px
  md: 6px
  lg: 8px
  xl: 12px
  full: 9999px
spacing:
  '1': 4px
  '2': 8px
  '3': 12px
  '4': 16px
  '5': 24px
  '6': 32px
  status-strip-height: 44px
  bottom-bar-height: 88px
  song-panel-width: 260px
  gamepad-panel-width: 300px
  gamepad-compact-width: 200px      # [ASSUMPTION]
  song-rail-width: 48px             # [ASSUMPTION] collapsed song panel (icon strip)
  card-inset: 6px
  note-view-header-height: 40px
  ruler-height: 22px
  lane-height: 21px
  lane-height-min: 18px             # [ASSUMPTION] floor before the lane area starts to scroll
  lane-header-width: 96px
  out-of-range-zone-height: 30px
  out-of-range-zone-collapsed: 6px  # thin strip when a zone holds no notes (mock-approved)
  split-divider-height: 6px         # draggable editor/notes divider hit area (mock-approved)
  note-bar-height: 15px
  big-button-height: 60px
  big-button-min-width: 190px
  corner-bracket-size: 22px
  wizard-list-width: 280px
  orb-icon-size: 22px
  focus-ring-width: 2px
  focus-ring-offset: 2px
components:
  status-strip:
    background: '{colors.panel-raised}'
    border-bottom: '1px {colors.rim-soft}'
    height: '{spacing.status-strip-height}'
    padding-x: 16px                  # lifted from mockups/key-empty-state.html (Story 4.5)
    brand-gap: 28px                  # lifted from mockups/key-empty-state.html (Story 4.5)
  status-led:
    size: 11px
    glow: 'Shadow blur 8px, same hue'
    label: '{typography.body-strong}'
    detail: '{colors.text-muted}'
    gap: 20px                        # lifted from mockups/key-empty-state.html (Story 4.5)
    symbol-gap: 7px                  # lifted from mockups/key-empty-state.html (Story 4.5)
    detail-gap: 7px                  # label to detail, same flex gap in mockups/key-empty-state.html (Story 4.5 eye check)
    padding: '2px 4px'               # lifted from mockups/key-empty-state.html (Story 4.5)
  panel-tab:
    background: '{colors.parchment}'
    foreground: '{colors.ink}'
    typography: '{typography.panel-tab}'
    radius: '{rounded.sm} {rounded.sm} 0 0'
    underline: '1px {colors.rim}'
  track-picker:
    background: '{colors.well}'
    border: '1px {colors.rim}'
    radius: '{rounded.md}'
    caret: '{colors.amber}'
    min-height: 34px
  stepper-button:
    background: '{colors.well}'
    border: '1px {colors.rule-strong}'
    radius: '{rounded.md}'
    height: 28px
  stepper-button-active:
    background: '{colors.amber}'
    foreground: '{colors.ink}'
    glow: 'Shadow blur 10px {colors.amber-glow}'
  chip-auto:
    background: '{colors.amber-wash}'
    foreground: '{colors.amber}'
    radius: '{rounded.lg}'
  button-small:
    border: '1px {colors.rim}'
    foreground: '{colors.amber}'
    radius: '{rounded.xl}'
  fit-warning:
    background: '{colors.ember-wash}'
    border-left: '3px {colors.ember}'
    foreground: '{colors.ember-text}'
  note-view-card:
    background: '{colors.parchment}'
    foreground: '{colors.ink}'
    border: '{spacing.card-inset} {colors.frame}'
    radius: '{rounded.xl}'
  note-view-header:
    background: '{colors.parchment-header}'
    border-bottom: '1px {colors.parchment-grid}'
    height: '{spacing.note-view-header-height}'
  segmented-toggle:
    border: '1px {colors.ink}'
    active-background: '{colors.ink}'
    active-foreground: '{colors.parchment}'
    radius: '{rounded.xl}'
  transport-button:
    border: '1px {colors.ink}'
    radius: '{rounded.full}'
    size: 26px
  history-button:
    border: '1px {colors.ink}'
    radius: '{rounded.full}'
    size: 26px
    icon: 'painted curved arrow, 1.6px {colors.ink} stroke; redo = undo mirrored'
    separator: '1px {colors.parchment-grid}'
  lane:
    background: '{colors.parchment}'
    background-accidental: '{colors.parchment-lane-alt}'
    separator: '1px {colors.parchment-rule}'
    height: '{spacing.lane-height}'
  lane-header:
    background: '{colors.parchment-header}'
    background-accidental: '{colors.parchment-header-alt}'
    name: '{typography.lane-name}'
    fingering: '{typography.lane-fingering}'
    fingering-color: '{colors.ink-soft}'
    width: '{spacing.lane-header-width}'
  note-bar:
    background: '{colors.note}'
    border: '1px {colors.ink}'
    radius: '{rounded.sm}'
    height: '{spacing.note-bar-height}'
  note-bar-selected:
    background: '{colors.note}'
    stroke: '2px {colors.amber}'
    glow: '{colors.amber-glow}'
  marquee:
    background: '{colors.amber-wash}'
    border: '1px dashed {colors.amber-ink}'
  playhead:
    color: '{colors.ember}'
    width: 2px
  staff-glyph:
    background: '{colors.ink}'
    foreground: '{colors.parchment}'
    radius: '{rounded.full}'
    diameter: 17px
    typography: '{typography.glyph}'
  out-of-range-zone:
    background: '{colors.out-of-range-zone}'
    label: '{colors.out-of-range-ink}'
    divider: '1px dashed {colors.parchment-grid}'
    height: '{spacing.out-of-range-zone-height}'
    height-collapsed: '{spacing.out-of-range-zone-collapsed}'
  ghost-note:
    background: '{colors.out-of-range-note}'
    border: '1px dashed {colors.out-of-range-ink}'
    radius: '{rounded.sm}'
  fold-target:
    background: 'none'
    border: '1px dashed {colors.out-of-range-ink}'
    radius: '{rounded.sm}'
  official-song-group:
    background: '{colors.ember-wash}'
    border: '1.6px dashed {colors.ember}'
    radius: '{rounded.lg}'
    note-fill: '{colors.ember}'
  official-song-label:
    background: '{colors.ember}'
    foreground: '{colors.on-ember}'
    typography: '{typography.note-label}'
    radius: '{rounded.sm}'
  gamepad-body:
    background: '{colors.rule}'
    border: '1.5px {colors.rim}'
  gamepad-control:
    background: '{colors.control}'
    border: '1px {colors.rim}'
    well: '{colors.well}'
    foreground: '{colors.text}'
  gamepad-control-lit:
    background: '{colors.amber}'
    foreground: '{colors.ink}'
    glow: '{colors.amber-glow}'
  gamepad-key-chip:
    background: '{colors.well}'
    border: '1px {colors.rim}'
    foreground: '{colors.text}'
    typography: '{typography.key-chip}'
    radius: 3px
    size: 13px
  gamepad-panel-focused:
    ring: 'inset 2px {colors.amber}'
    glow: 'inset wash {colors.amber-wash}, 5-6 stacked inset strokes'
  chip-keys-active:
    background: '{colors.amber}'
    foreground: '{colors.ink}'
    radius: '{rounded.full}'
  chip-locked:
    background: '{colors.well}'
    foreground: '{colors.text-muted}'
    border: '1px {colors.rule-strong}'
    radius: '{rounded.full}'
  button-play-primary:
    background: '{colors.amber}'
    foreground: '{colors.ink}'
    glow: 'Shadow blur 16px {colors.amber-glow}'
    radius: '{rounded.xl}'
    height: '{spacing.big-button-height}'
  button-play-secondary:
    background: '{colors.well}'
    border: '1px {colors.rim}'
    foreground: '{colors.text}'
    radius: '{rounded.xl}'
    height: '{spacing.big-button-height}'
  button-listen:
    background: '{colors.well}'
    border: '1px {colors.rim}'
    foreground: '{colors.text}'
    radius: '{rounded.xl}'
    height: '{spacing.big-button-height}'
    icon: '{components.fairy-orb-icon}'
  fairy-orb-icon:
    size: '{spacing.orb-icon-size}'
    core: '{colors.on-ember}'
    glow: 'Shadow blur 8px {colors.amber}'
    wings: 'one pair of rounded leaf ellipses, 1px {colors.text} stroke at 85 %, {colors.text} fill at 22 %'
    sparkle: 'two small {colors.amber} dots trailing below-left'
  button-disabled:
    background: '{colors.panel-raised}'
    border: '1px {colors.rule-strong}'
    foreground: '{colors.text-muted}'
    glow: 'none'
  button-stop-idle:
    background: '{colors.stop-idle-fill}'
    border: '1px {colors.ember}'
    foreground: '{colors.stop-idle-text}'
    opacity: 0.75
    radius: '{rounded.xl}'
  state-layer:                       # hover / pressed of every enabled button (Story 4.7 owner eye check)
    hover-toward: '{colors.text}'
    hover-amount: 0.10               # fill and border mixed 10 % toward text (gamma space)
    pressed-toward: '#000000'
    pressed-amount: 0.20             # fill and border mixed 20 % toward black
  button-stop-live:
    background: '{colors.ember}'
    foreground: '{colors.on-ember}'
    glow: 'Shadow blur 18px {colors.ember}'
    radius: '{rounded.xl}'
  bottom-bar:
    padding-x: 16px                  # lifted from mockups/key-empty-state.html (Story 4.7)
    gap: 14px                        # lifted from mockups/key-empty-state.html (Story 4.7)
  big-button:
    padding-x: 22px                  # lifted from mockups/key-empty-state.html (Story 4.7)
    icon-size: 16px                  # lifted from mockups/key-empty-state.html (Story 4.7)
    icon-gap: 12px                   # lifted from mockups/key-empty-state.html (Story 4.7)
    sub-opacity: 0.85                # lifted from mockups/key-empty-state.html (Story 4.7)
  safety-note:
    typography: '{typography.safety-note}'
    foreground: '{colors.text-muted}'
    padding-right: 6px               # lifted from mockups/key-empty-state.html (Story 4.7)
    second-line-opacity: 0.7         # lifted from mockups/key-empty-state.html (Story 4.7)
  banner-dialogue:
    background: '{colors.banner-fill}'
    border: '1px {colors.banner-border}'
    radius: '{rounded.md}'
    typography: '{typography.banner-text}'
    foreground: '{colors.text}'
  banner-name-tab:
    background: '{colors.panel-raised}'
    border: '1px {colors.rim}'
    foreground: '{colors.parchment}'
    typography: '{typography.panel-tab}'
    radius: 3px
  countdown-veil:
    background: '{colors.veil}'
  countdown-numeral:
    typography: '{typography.countdown}'
    color: '{colors.text}'
    outline: '2px {colors.frame}'
    halo: 'soft {colors.amber-glow} rounded-rect Shadow behind the numeral'
  end-card:
    background: '{colors.parchment}'
    foreground: '{colors.ink}'
    title: '{typography.card-title}'
    song-name: '{colors.amber-ink}'
    radius: '{rounded.lg}'
    ring: '{spacing.card-inset} {colors.frame}'
  corner-bracket:
    stroke: '{colors.bracket}'
    size: '{spacing.corner-bracket-size}'
    inset: 4px
  modal-dialog:
    background: '{colors.panel-raised}'
    border: '1px {colors.rim}'
    title: '{typography.dialog-title}'
    radius: '{rounded.lg}'
    backdrop: '{colors.scrim}'
    width: 460px
    min-width: 440px
    padding-y: 20px
    padding-x: 26px
    flourish-width: 220px
    flourish-height: 14px
    flourish-margin-top: 6px
    flourish-margin-bottom: 10px
    row-padding-y: 7px
    row-gap: 12px
    bullet-size: 9px
    checklist-margin-bottom: 14px
    text-margin-bottom: 16px
    action-height: 34px
    action-padding-x: 18px
    action-gap: 10px
    glow: 'Shadow blur 12px {colors.amber-glow}'
  wizard-step:
    list-width: '{spacing.wizard-list-width}'
    background: '{colors.well}'
    active-ring: '2px {colors.amber}'
    done-mark: '{colors.status-ok}'
  text-editor:
    background: '{colors.well}'
    foreground: '{colors.text}'
    typography: '{typography.code}'
    error-underline: '{colors.status-error}'
  split-divider:
    background: '{colors.frame}'
    grip: '{colors.rim-soft}'
    grip-hover: '{colors.amber}'
    height: '{spacing.split-divider-height}'
  drop-zone:
    border: '2px dashed {colors.parchment-grid}'
    radius: '{rounded.xl}'
  tooltip:
    background: '{colors.panel-raised}'
    border: '1px {colors.rim-soft}'
    foreground: '{colors.text}'
    radius: '{rounded.md}'
  focus-ring:
    stroke: '{spacing.focus-ring-width} {colors.amber}'
    offset: '{spacing.focus-ring-offset}'
    radius: 'control radius + {spacing.focus-ring-offset}'
  focus-ring-on-parchment:
    stroke: '{spacing.focus-ring-width} {colors.amber-ink}'
    offset: '{spacing.focus-ring-offset}'
  progress-bar:
    track: '{colors.well}'
    border: '1px {colors.rule-strong}'
    fill: '{colors.amber}'
    height: 8px
    radius: '{rounded.full}'
  checkbox:
    box: '14px {colors.well}'
    border: '1px {colors.rim}'
    check: '{colors.amber}'
    radius: '{rounded.xs}'
    label: '{colors.text-muted}'
  context-menu:
    background: '{colors.panel-raised}'
    border: '1px {colors.rim}'
    radius: '{rounded.md}'
    item: '{typography.body}'
    item-hover: '{colors.amber-wash}'
    separator: '1px {colors.rule-strong}'
  log-view:
    background: '{colors.well}'
    foreground: '{colors.text}'
    typography: '{typography.code}'
    timestamp: '{colors.text-muted}'
    error-row: '{colors.status-error}'
    radius: '{rounded.md}'
  port-pick-row:
    background: '{colors.well}'
    border: '1px {colors.rule-strong}'
    selected-border: '2px {colors.amber}'
    radius: '{rounded.md}'
    detail: '{colors.text-muted}'
  inline-hint:
    foreground: '{colors.text-muted}'
    typography: '{typography.small}'
    inset: 10px
  collapsible-header:
    foreground: '{colors.text-muted}'
    typography: '{typography.label}'
    chevron: '{colors.amber}'
  settings-field:
    label: '{typography.body-strong}'
    explanation: '{colors.text-muted}'
    input: '{components.stepper-button}'
  song-rail:
    background: '{colors.panel}'
    icon: '{colors.text-muted}'
    icon-active: '{colors.amber}'
    width: '{spacing.song-rail-width}'
---

## Brand & Style

Bonooru is a personal, playful tool that sits next to a Switch in handheld mode while its owner plays *Ocarina of Time* (Switch 2 remake). The app should feel like a menu from that world at night: warm dark wood around the edges, the music itself laid out on a parchment card, and a soft amber glow on whatever you are about to act on. The posture is **Hybrid**: controls, status and messages stay dark and quiet; the note view is the one bright surface, so the eye goes straight to the music when glancing over from the console.

The app recalls the remake's mood the way RiiDraw recalls Tomodachi Life. It does not reproduce the game.

**Evoke, don't copy (hard constraint).** The app is publicly distributed. Nothing from Nintendo ships in it, in a mock, or in the repo:

- no logos or marks: Nintendo, Switch, Zelda / Ocarina of Time wordmarks, Triforce, Hylian crest, Sheikah eye or any game emblem;
- no character art, portraits, sprites, models or item icons (Link, Navi, the ocarina item art, etc.);
- no ripped UI textures, frames, filigree, panel art or button-prompt glyph artwork;
- no game fonts or traced letterforms;
- no sound effects, music samples or ocarina recordings (the preview timbre and the countdown tick are original synthesised sounds);
- no screenshots of the game or of console menus in the app, mocks, README or repo. Console settings are shown as an **original illustration plus the exact menu path** (see Components › Setup wizard). The reference screenshots in `imports/` are git-ignored, are for mood only, and are never embedded, linked or copied.

Every ornament (corner brackets, the controller silhouette, button glyph circles, the settings illustration) is an original shape drawn with egui `Painter` paths. Song titles of detected official songs are the only game names that appear in copy.

Traits lifted from the mood references (described in words only): the parchment card as the one bright surface; corner brackets on parchment; a glowing selection outline; the dark translucent dialogue panel with a small name tab; and the controls-sheet idea of a dark controller silhouette with labelled callouts.

## Colors

Dark only. The app ignores the OS light/dark setting; there is no light theme.

- **Frame `#16110c`, Panel `#1c150f`, Panel-raised `#221a13`, Well `#2a1e13`** — four steps of warm dark wood. Frame is the window background and the border around the parchment card; Panel is the song and gamepad columns; Panel-raised is the status strip, bottom bar, dialogs, context menus and banner tabs; Well is inputs, steppers, the text editor, the log view and gamepad button wells.
- **Rule `#3a2a1b`, Rule-strong `#4a3826`** — column dividers and inactive control borders. Rule also fills the gamepad body.
- **Rim `#a37a3e`** — the brass edge: track picker borders, small outline buttons, gamepad outlines, the hairline under the status strip (`rim-soft`, 40 % alpha). Rim is decoration, never state.
- **Text `#f3e3bd` / Text-muted `#c4a676`** — cream body text and muted labels on dark. Cream on Frame is 14.8:1; muted on Panel is 7.8:1, on Well 7.0:1.
- **Parchment `#ecdcb2`** and its family (`parchment-lane-alt` for accidental lanes, `parchment-header`/`-alt` for lane headers and the card header, `parchment-ruler`, `parchment-rule`, `parchment-grid`) — only the note view, the end-of-song card and diagram/illustration sheets use parchment. Ink `#2b2015` on parchment is 11.7:1.
- **Note `#5b3d22`** — note bars (7.2:1 on parchment). **Control `#56402a`** — gamepad buttons at rest (cream on control 7.7:1).
- **Amber `#f0b43c`** — the one accent. It means *selected, focused, or the thing you will start*: the selected note's glow, focus rings, the focused gamepad ring and "Keys active" chip, the active stepper segment, the Play button, lit keys while playing, the countdown halo. Never used for warnings or status. Ink on amber is 8.6:1; amber on Panel 9.7:1, on Well 8.7:1 (focus ring on dark). **Amber-ink `#8a5a12`** (confirmed 2026-11-27, OQ-7) is the accent's parchment-safe variant: the song name on the end card (4.35:1, large text) and the focus ring on parchment (4.35:1 on parchment, 3.85:1 on parchment-header, both ≥ 3:1 for non-text).
- **Ember `#b8432f`** — the official-song warning (group outline, label tab, note fill), the playhead, and the live STOP button. Ember-text `#f2a58f` is its readable form on dark (8.6:1 on Panel-raised). On-ember `#fff4e0` on ember is 4.96:1 (label tab, live STOP label). The playhead on parchment is 3.98:1 (non-text, ≥ 3:1).
- **Out-of-range grey** (`out-of-range-zone #cfc6b4`, `out-of-range-ink #55504a`, `out-of-range-note #b9b0a0`) — deliberately desaturated so out-of-range notes read as "not part of the playable song yet". The zone label is 4.71:1 (AA for 11 px). The ghost-note fill vs the zone is only 1.27:1 by design; the dashed `out-of-range-ink` border (4.71:1) carries the shape.
- **Status colours** — `status-ok #8fd16f` (lifted from the mock), `status-warning #f08a4b`, `status-error #ef6a55`, `status-unknown #9a8a72` (the three hexes confirmed by the owner 2026-11-27 in the Story 4.1 token gallery, OQ-7). **Rule: the warning hue must be distinguishable from amber.** Warning sits at ~22° hue against amber's ~40°, but the two are only 1.34:1 apart in luminance, so the symbol (triangle) carries the meaning; colour is never the only signal. On Panel-raised: ok 9.4:1, warning 6.9:1, error 5.6:1, unknown 5.1:1; error on Panel 5.9:1, on Well 5.3:1. **`status-error` is the one error red**: LED error, the tempo "too short" value, editor error underlines, error rows in the log view.
- **Overlays** — `banner-fill` (88 % alpha; over parchment it composites to ~#352e24, cream text 10.6:1), `banner-border` (40 %), `veil` (80 %, countdown; over parchment ~#413a2d), `scrim` (modal backdrop, confirmed 2026-11-27, OQ-7). In egui these are `Color32::from_rgba_unmultiplied`.

Avoid: gradients (the Lantern direction's gold gradient was not chosen); extra accents; `ember` for anything other than the official-song warning, the playhead and live STOP; any red (ember or `status-error`) for generic errors inside the note view — note-view problems use the out-of-range grey or the ember official-song group only.

## Typography

Two OFL fonts, bundled as TTF in egui and listed in About with their licence texts:

- **Cormorant Garamond** (700) — display only: `{typography.brand}` (status strip), `{typography.panel-tab}` ("Song" / "Gamepad" tabs, banner name tab), `{typography.dialog-title}` ("Ready to play?"), `{typography.card-title}` ("You played <Song>!"), `{typography.countdown}` (numerals).
- **Nunito Sans** (400/600/700) — everything else. Labels are bold uppercase with 0.06 em tracking (`extra_letter_spacing`) in `{colors.text-muted}`. Values that change while you tune them (transpose, tempo) are large: `{typography.value-lg}`, `{typography.value-md}`.
- **Code** — the text-score editor and the log view need a monospace face that neither font provides. Decided 2026-11-27 (OQ-7): egui's bundled monospace (Hack, `default_fonts` feature) at 13 px, its licence shipped.

**egui implementation notes** (egui/epaint 0.37, per `review-egui-feasibility.md`):

- Weights come from the variable `wght` axis (`RichText::variation("wght", 600.0)`) or one static TTF per weight registered as its own family. `RichText::strong()` only changes colour; it is not bold.
- egui's shaper has no OpenType feature switch, so **no `tnum`**. Times ("0:12 / 0:31", "1:48") are drawn in fixed-width digit cells (one `Painter::text` per digit, cell = widest digit).
- Symbols are not guaranteed in either font and egui's emoji fallback mismatches at 10–12 px. Draw ▶ ■ ⏮ ▾ ▲ ⌫ ↵ ↑ ♯ ♪ ↶ ↷ as `Painter` shapes (key chips ⌫ ↵, fingering arrow ↑, lane sharp ♯, filename ♪, Undo/Redo arrows ↶ ↷) unless the bundled TTF is verified to cover a glyph at that size before copy is frozen. The fairy-orb icon is painted too.

Sizes are egui logical points; HiDPI scales through `pixels_per_point`.

## Layout & Spacing

4-based scale: `{spacing.1}` · `{spacing.2}` · `{spacing.3}` · `{spacing.4}` · `{spacing.5}` · `{spacing.6}`. Token values in the frontmatter are the single source; prose below names tokens, not numbers.

Mockups (original CSS/SVG, in `mockups/`) illustrate this spec. **Spines win on conflict with any mock.** Known conflicts are noted where they occur. The mood screenshots are described in words only (see Brand & Style) and never linked.

Main screen (layout Variant A, Hybrid skin). → Reference: [mockups/layouts-main-screen-2026-11-20.html](mockups/layouts-main-screen-2026-11-20.html), variant A (region layout; low-fi, superseded labels) and [mockups/directions-oot-2026-11-20.html](mockups/directions-oot-2026-11-20.html), direction 3 "Hybrid" (skin). The Hybrid main screen with a song loaded is in [mockups/key-dialogs.html](mockups/key-dialogs.html) (behind the readiness confirm).

| Region | Size | Fill |
|---|---|---|
| Status strip (top) | `{spacing.status-strip-height}` | `{colors.panel-raised}` |
| Song panel (left) | `{spacing.song-panel-width}`; collapses to `{spacing.song-rail-width}` | `{colors.panel}` |
| Note view (centre) | all remaining width (~440 px at the 1024 px minimum) | parchment card inset `{spacing.card-inset}` |
| Gamepad panel (right) | `{spacing.gamepad-panel-width}`; compact `{spacing.gamepad-compact-width}` | `{colors.panel}` |
| Bottom bar | `{spacing.bottom-bar-height}` | `{colors.panel-raised}` |

At larger widths every extra pixel goes to the note view. Note view internals: header `{spacing.note-view-header-height}`, ruler `{spacing.ruler-height}`, lane header width `{spacing.lane-header-width}`, lanes `{spacing.lane-height}` (19 lanes + two out-of-range zones of `{spacing.out-of-range-zone-height}`). Panel padding 12–14 px; label-to-control gap `{spacing.1}`; control groups `{spacing.3}` apart.

**Vertical fit.** When the lane area is short (small window, or the text-score split), lanes shrink to `{spacing.lane-height-min}`; below that the lane area scrolls vertically (lane headers scroll with it, ruler stays pinned). An out-of-range zone with no notes collapses to `{spacing.out-of-range-zone-collapsed}`.

**Text-score split.** Editor on top, note view below, separated by a draggable `split-divider` (`{spacing.split-divider-height}`, grip dots in `{colors.rim-soft}`, amber on hover/drag, resize cursor). Initial position 40/60 [ASSUMPTION]; the user's position is persisted. The editor never shrinks below 4 text lines.

Layout is done by computing region rects and filling them with `ui.scope_builder(UiBuilder::new().max_rect(r))` in reading order (not egui side/central `Panel`s), so the Tab order matches the documented order.

## Elevation & Depth

No drop shadows as hierarchy. Depth comes from tone (Frame → Panel → Panel-raised → Well) and from the one bright surface, the parchment card. Glow is the only "light" effect: amber for selection, focus and primary action; ember for live STOP; the LED's own hue for status.

**Glow in egui:** `epaint::Shadow { offset: [0, 0], blur, spread, color }` (or `RectShape::blur_width`) painted under the control. The `Shadow blur <n>px` values in `components` are CSS `box-shadow` blur radii, as in the mocks (a Gaussian with σ = n / 2); the app converts them to epaint's linear feather (`blur` = round(1.55 × n), the least-squares fit of the feather to that Gaussian) so the halo matches the mock. The colour is used as written. It works only on **rounded rects and circles** (full radius): LED dot, active stepper, Play, live STOP, selected note, lit gamepad buttons; the D-pad cross is two rects, Home uses its bounding rounded rect. No blur exists for text, arbitrary paths or inset shadows: text gets a soft rounded-rect halo behind it (countdown), and inset glows (gamepad panel focus) are 5–6 stacked inset strokes with falling alpha.

Overlays stack in this order: note view → countdown veil / interruption banner / end card (over the note view only) → tooltips and context menus → modal dialogs over `{colors.scrim}`. One modal at a time.

## Shapes

`{rounded.xs}` — D-pad arms, checkbox. `{rounded.sm}` — note bars, panel tabs (top corners), BPM field, ghost notes. `{rounded.md}` — track picker, stepper buttons, banners, tooltips, context menu, log view, port rows. `{rounded.lg}` — official-song group, end card, dialogs. `{rounded.xl}` — the big Play/STOP buttons, the parchment card, small pill buttons, segmented toggle ends. `{rounded.full}` — LEDs, transport buttons, staff glyphs, face buttons, chips, progress bar.

**Corner brackets** are the signature ornament: an original L-shaped bracket (`{spacing.corner-bracket-size}`) at each corner of a parchment surface — a 2 px outer path with a rounded elbow, a 1.2 px inner quarter-curve (`QuadraticBezierShape`, stroke only), and a 1.6 px dot at the elbow — mirrored per corner, stroked in `{colors.bracket}` (on dark overlays, `{colors.rim}`). Used on the note-view card, the end-of-song card and diagram/illustration sheets. Nowhere else.

**egui shape rules:** filled paths must be **convex** (epaint fan-triangulates fills); strokes may follow any path. Dashes use `Shape::dashed_line_many` (one helper flattens a rounded rect into points). Strokes are floored at 1 physical pixel (`1.0 / pixels_per_point`), so the 0.5 px staff rules and similar hairlines never wash out on 1× displays.

No octagonal buttons: the octagon shape belonged to direction 1 (Lantern), which was not chosen.

## Components

Visual specs. Behaviour lives in EXPERIENCE.md › Component Patterns; component names are identical in both files. Dimensions given only in prose here (gamepad geometry, playhead handle, staff bar, banner and big-button padding) are the spec; they are not tokenised because nothing else reuses them.

**Focus ring** (shared helper, every focusable control). egui draws focus with the pressed style and has no ring and no focus-visible, so every themed widget helper calls one `paint_focus_ring(resp, radius, on_parchment)`: `focus-ring` = `{spacing.focus-ring-width}` `{colors.amber}` stroke, `{spacing.focus-ring-offset}` outside the control, corner radius = control radius + offset. On parchment surfaces it uses `focus-ring-on-parchment` (`{colors.amber-ink}`). Because egui has no focus-visible, the ring also shows after mouse use; this is accepted. The gamepad panel uses its own panel-level ring (below).

**Button states** (shared helpers, every button; owner eye check of Story 4.7). An enabled button shows the pointer: hover mixes its fill and border 10 % toward `{colors.text}`, pressed (primary button held down on it) 20 % toward black (`{components.state-layer}`, gamma-space mix of the idle colours, before any token opacity such as STOP idle's 75 %). The label, the symbol, the glow and the focus ring do not change; a disabled button (`button-disabled`) shows no hover or pressed change. Examples: amber `#f0b43c` → hover `#f0b949`, pressed `#c09030`; well `#2a1e13` → `#3e3224` / `#22180f` (rim border `#ab854b` / `#826232`); STOP idle fill `#3a1712` → `#4d2b23` / `#2e120e`, ember border `#be533d` / `#933626` (then 75 %). The gallery shows every look in its three states.

**Status strip.** Brand left in `{typography.brand}`, `{colors.parchment}`, `{components.status-strip.padding-x}` from the left edge, then `{components.status-strip.brand-gap}` to the first LED. Three **status LEDs** (PC↔Bridge, Bridge↔Controller, Controller↔Switch), each an 11 px symbol, `{components.status-led.symbol-gap}`, a label, `{components.status-led.detail-gap}` and a muted detail (e.g. "Bridge" `v0.3.0`), inside a focusable box padded `{components.status-led.padding}`; `{components.status-led.gap}` between LEDs. Right side: "Set up / Reflash…" link in `{colors.text-muted}` and a gear icon; neither is drawn until it works: the gear arrives with Epic 5 (Settings), the link with Epic 8 (wizard) (Story 4.5).

**LED symbol set** (colour + shape + label, never colour alone):

| State | Symbol | Colour |
|---|---|---|
| ok | filled dot with soft glow | `{colors.status-ok}` |
| warning | filled triangle | `{colors.status-warning}` |
| error | cross (×, two strokes) | `{colors.status-error}` |
| unknown | hollow ring | `{colors.status-unknown}` |

An `unknown` LED shows its label only ("Switch"), with no detail text. → Reference: LED warning, error and unknown states in [mockups/key-empty-state.html](mockups/key-empty-state.html).

**Song panel.** Parchment **panel tab** "Song" over a 1 px rim underline. Filename in `{typography.filename}` `{colors.parchment}` with a small pill "Open…" (`button-small`). **Track picker** (`track-picker`) shows "Melody · ch 1 · 62 notes · 98 % in range" with an amber caret painted as a shape; popup rows use the same fields. **Transpose stepper**: large value "−5 st", an `auto` chip (`chip-auto`), and a row of stepper buttons −12 · −1 · +1 · +12 · Auto; the active one is amber with glow. **Fit summary** in `{typography.small}`, counts bold. **Fit warning** row (official song): ember wash, 3 px ember left border, ember-text. **Tempo control**: − / value / + steppers and a "BPM" field; the "N notes too short" state turns the value `{colors.status-error}`. Footer hint "Mapping profile: Default · Switch 2" at 75 % opacity.

**Song rail** (collapsed song panel). `song-rail`: a `{spacing.song-rail-width}` column in Panel fill with vertically stacked painted icons (open, track, transpose, tempo) in `{colors.text-muted}`; the expand chevron at the top in amber. Icons get the focus ring and a tooltip with the current value.

**Note-view card.** Parchment, `{rounded.xl}`, inset `{spacing.card-inset}` in Frame, four corner brackets. Header (`note-view-header`, the only place the Roll / Staff toggle appears): **Roll / Staff toggle** (`segmented-toggle`: ink outline, active segment ink fill with parchment text), **preview transport** ▶ ■ (`transport-button`, ink-outline circles, glyphs painted), time "0:12 / 0:31" in fixed-width digit cells, right-aligned "Preview · PC only" at 75 %, then **Undo ↶ / Redo ↷** at the far right. `history-button` uses the same 26 px ink-outline circles as the transport, with curved arrows painted and redo mirrored; a 1 px `{colors.parchment-grid}` vertical rule separates the pair from the preview label, and a button with nothing to undo or redo is at `button-disabled` opacity. → Reference: [mockups/key-editor-staff.html](mockups/key-editor-staff.html) (Undo enabled, Redo disabled). With a text score, the centre splits (see Layout › Text-score split): **text editor** on top (`text-editor`: Well fill, monospace, straight inline error underline in `{colors.status-error}`; no wavy underline), note view below.

**Roll view.** → Reference: direction 3 in [mockups/directions-oot-2026-11-20.html](mockups/directions-oot-2026-11-20.html) and the Hybrid roll in [mockups/key-dialogs.html](mockups/key-dialogs.html). The ruler corner above the lane headers carries the small label "note · input" in `{colors.ink-soft}`. Ruler on `{colors.parchment-ruler}` with `{colors.ink-ruler}` ticks every beat and labels every 2 s. Lane headers: note name (`lane-name`, e.g. "D♯5", ♯ painted) left, fingering right (`lane-fingering`, `{colors.ink-soft}`, e.g. "X+D↑", ↑ painted). Accidental lanes use the `-alt` fills. Vertical grid lines `{colors.parchment-grid}` at 55 %. **Note bars** (`note-bar`): no text inside. **Selected note** (`note-bar-selected`): amber 2 px stroke + amber Shadow glow. **Marquee** (`marquee`): amber-wash fill with a 1 px dashed amber-ink outline while dragging on empty canvas. **Resize affordance**: the last 6 px of a hovered bar shows the horizontal-resize cursor. **Playhead**: 2 px ember line with a convex pentagon handle (12 px wide) on the ruler.

**Staff view.** → Reference: [mockups/key-editor-staff.html](mockups/key-editor-staff.html) (Hybrid Staff under the text editor); the original Staff idea is direction 2 "Parchment" of [mockups/directions-oot-2026-11-20.html](mockups/directions-oot-2026-11-20.html). Staff **keeps the lane headers** (note name + fingering), exactly as Roll. Same lanes drawn as staff rules on parchment: natural-pitch rules heavy (1.2 px ink), accidental rules light (0.5 px ink at 45 %, floored at 1 physical px). *Mock conflict:* the mock draws accidental rules at 1 px, 30 %; this spec wins. Each note is a thin bar (5 px, ink 80 %, radius 2.5) whose start carries a **staff glyph**: a 17 px ink circle with the button letter in parchment (`{typography.glyph}`). Modifiers follow the circle as small ink tags (19 × 12 px, radius 3, parchment text, e.g. "D↑", "S↑"). Duration = bar length. Selection, marquee and playhead as in Roll. No clef artwork beyond an original drawn path.

**Dimmed out-of-range zone.** → Reference: direction 3 (ghost fold) and [mockups/key-editor-staff.html](mockups/key-editor-staff.html) (ghost G5 folding to G4; empty "below B3" zone collapsed; the mock uses the old chart names, one octave below the sounding pitch the app uses since Story 1.1). Two bands (`out-of-range-zone`), one above F6 and one below B4, labelled "above F6" / "below B4" in `{colors.out-of-range-ink}` and separated from the playable lanes by a dashed `{colors.parchment-grid}` rule; each collapses to a thin strip when empty. **Ghost notes** (`ghost-note`) sit in the zone. The **fold target** (`fold-target`) is a dashed outline with no fill in the target lane, linked to its ghost by a dashed leader line (70 %). Inline label: "G6 · out of range — proposed: fold to G5" in 11 px semibold `{colors.out-of-range-ink}` (4.71:1). Ghost notes take the same selected style as note bars.

**Official-song group.** → Reference: direction 3 (official-song group). A rounded rectangle (`{rounded.lg}`) around the affected notes: ember wash fill, 1.6 px dashed ember outline (5/3). Notes inside fill ember. A label tab above the group's top-left (`official-song-label`), e.g. "Triggers Song of Storms".

**Context menu** (note view). `context-menu`: Panel-raised, 1 px rim, `{rounded.md}`, rows in `{typography.body}` with 6/12 px padding, hover/focus row in `{colors.amber-wash}`, a `rule-strong` separator before Delete. Rows: Move up an octave · Move down an octave · (separator) · Delete; out-of-range notes add "Reset to proposal". Disabled rows at 45 % opacity.

**Virtual gamepad.** → Reference: direction 3 (gamepad, focus and lit states). An original Pro-Controller-like silhouette (`gamepad-body`: Rule fill, 1.5 px rim), **filled as a union of convex shapes** (rounded rects and circles for body and grips) and **outlined by one stroked path** on top, so the result looks like one closed shape without concave fills. Controls: ZL/ZR and L/R as rounded rects above the body; two sticks (17 px well ring + 11 px knob); D-pad cross (two rects) in a 22 px well; ABXY diamond (10 px circles in a 31 px well); −, +, Capture (square with dot), Home (house shape, convex). All controls `gamepad-control`. **Callouts**: a `gamepad-key-chip` beside each keyboard-mapped control — Z, Q, E, C, H, J, K, L, ⌫, ↵ (⌫ and ↵ painted as shapes), and one "WASD · arrows" chip under the D-pad; "mouse" in `{colors.text-muted}` next to the sticks. Where a chip cannot sit beside its control, a 1 px rim leader line joins them [ASSUMPTION]. A legend list under the pad repeats the mapping (control name in parchment, keys in body).

- **Focused** (`gamepad-panel-focused`): the whole panel gets an inset 2 px amber ring plus an inset amber wash (stacked inset strokes), and the panel tab shows `chip-keys-active` "Keys active".
- **Lit-key visualizer** (during countdown and Play): the panel shows `chip-locked` "Locked · live view"; every control the song is pressing turns `gamepad-control-lit` (amber fill with Shadow glow, ink letter); a caption under the pad names the current input, e.g. "Now: Y + D↑ (C5)".
- **Compact variant** (< 1280 px wide): the silhouette is dropped; the controls render as a tight grid of buttons in the same styles, with no key chips (key hints move to tooltips). Spine-only: no mock.

**Bottom bar.** → Reference: [mockups/key-editor-staff.html](mockups/key-editor-staff.html) (Play off, Listen on) and [mockups/key-empty-state.html](mockups/key-empty-state.html) (all off but STOP). Left to right: **Play from cursor** (`button-play-primary`, amber, ▶ painted, label "Play from cursor" + sub-line with the cursor position, e.g. "0:12"), **Play from start** (`button-play-secondary`, ⏮ painted, sub-line "0:00"), **Listen** (`button-listen`: the secondary style, fairy-orb icon, label "Listen" + sub-line "on this PC"). Disabled buttons use `button-disabled` (45 % opacity, no glow; the orb loses its glow too) and show a tooltip naming the first blocking reason. Centre-right: safety note, two lines, right-aligned in `{colors.text-muted}`: "Don't dock the Switch mid-song." / "Esc stops at any time." (second line 70 %). Right: **STOP** with sub-line "Esc". Idle STOP is `button-stop-idle` (dim, 75 % opacity, still clickable); during countdown and Play it becomes `button-stop-live` (ember fill, on-ember text 4.96:1, ember glow). Big buttons: `{spacing.big-button-height}` high, min `{spacing.big-button-min-width}` wide, 22 px side padding, `{rounded.xl}`, 14 px apart. Epic 4 (Story 4.7): STOP idle and the safety note; the three Play/Listen slots are reserved and empty until Epics 5–6; STOP's content sits at 22 px left padding (mock flex start). Tokens: `components.bottom-bar`, `components.big-button`, `components.safety-note`, `typography.safety-note`.

**Fairy-orb icon** (Listen; a quiet easter egg). `fairy-orb-icon`, `{spacing.orb-icon-size}`: an **original** small glowing winged light — a round cream core (`{colors.on-ember}`) with an amber `Shadow` glow, **one** pair of rounded leaf-shaped wings angled up (convex ellipses, cream outline, faint cream fill) and two tiny amber sparkle dots trailing below-left. It nods to the idea of a fairy companion without reproducing the game's character: never two wing pairs, never the blue/white palette, no face, no outline traced from game art, and no "Hey! Listen!" or other quoted catchphrase anywhere in copy, tooltips or docs. Drawn with `Painter` circles/ellipses only (all convex).

**Interruption banner** (dialogue-style). → Reference: direction 3 (dialogue banner) and [mockups/key-dialogs.html](mockups/key-dialogs.html) (USB-suspend banner). Used for playback interruptions and import failures, over the bottom of the note view. `banner-dialogue`: translucent dark fill, 1 px cream-40 % border, `{rounded.md}`, 16/20 px padding, centred `{typography.banner-text}`. A **name tab** (`banner-name-tab`) overlaps the top edge at the left, e.g. "Playback stopped". Actions centred: primary = parchment fill with ink text, secondary = ghost (cream-50 % outline). A small ▾ chevron (painted) sits centred on the bottom edge. A "Details" text link sits at the right end of the action row. The **Update banner** is the slim variant: full width under the status strip (8 px side margins), 38 px high, single line, same fill and border, no name tab; "Update" in the parchment primary style, "Later" ghost, both right-aligned. → Reference: [mockups/key-dialogs.html](mockups/key-dialogs.html) (recommended-update banner).

**Inline hint.** `inline-hint`: one line of `{typography.small}` in `{colors.text-muted}` with an optional small button, placed in the region it concerns (e.g. "Plug the bridge into your PC." in the gamepad panel; "Gamepad clicks are ignored until the Switch is ready." or "The gamepad is off until both boards are updated." under the pad). Until the pad exists (Story 4.8) the hint sits at the top of the gamepad panel, `components.inline-hint.inset` (10 px, the mock's panel padding) inside its left, right and top edges, wrapped to the panel width (Story 4.6).

**Countdown.** → Reference: direction 3 (countdown). `countdown-veil` (80 %) over the note-view card only; centred numeral in `{typography.countdown}`, **cream (`{colors.text}`) with a 2 px `{colors.frame}` outline** (8 offset draws) and a soft amber rounded-rect halo behind it (text cannot glow in egui). Sub-line 14 px semibold cream: "Starting from 0:12 · hands off the controller". Cream on the composited veil (~#413a2d) is 8.85:1 for both numeral and sub-line.

**End-of-song card.** → Reference: [mockups/key-dialogs.html](mockups/key-dialogs.html). A parchment card (`end-card`, `{rounded.lg}`, about 440 px wide) centred over the note view, with the four corner brackets and a `{spacing.card-inset}` Frame-coloured ring around it so it separates from the parchment beneath. Title in `{typography.card-title}`: "You played " in ink + song name in `{colors.amber-ink}` + "!". Detail line in `{typography.small}` `{colors.ink-soft}`: "1:48 · 62 notes · on time". Two buttons below: Play again (ink fill, parchment text) and Close (ink outline). This ink-fill / ink-outline pair is the button style on every parchment surface (also the empty state). No staff excerpt.

**Modal dialog** (readiness confirm, Settings modal, unsaved changes, restore draft, required update). → Reference: [mockups/key-dialogs.html](mockups/key-dialogs.html) (readiness confirm, "Firmware update needed"). `modal-dialog`, about 440–460 px wide, centred over the whole window; title in `{typography.dialog-title}` centred over a short original flourish rule (two rim hairlines meeting at a small diamond between two leaf curves, stroked path + one convex diamond); checklist rows (14.5 px) with a small amber diamond bullet (convex), separated by 1 px `{colors.rule}`; body text centred; actions right-aligned, primary amber with glow and focus, secondary ghost (Well fill, rim border) to its left; the "Don't ask again" checkbox sits left on the action row. Backdrop `{colors.scrim}`. The outer width is 460 px by default and never below 440 px (`components.modal-dialog.width` / `min-width`), and the dialog has no drop shadow (see Elevation: no drop shadows as hierarchy).

- **Readiness confirm:** title "Ready to play?", three checklist rows, checkbox, Cancel (ghost) + Play (primary, focused).
- **Firmware update needed** (required update): title "Firmware update needed", one centred body line, Not now (ghost) + Update now (primary, focused). Behind it the status LEDs keep showing the boards' current versions. Epic 4 (Story 4.6): one primary OK, no ghost, 440 pt wide; Not now + Update now arrive with the wizard (Epic 8).

**Checkbox.** `checkbox`: 14 px Well box, 1 px rim, `{rounded.xs}`, amber check drawn as a two-segment stroke; label in `{colors.text-muted}`; focus ring on the box. Used for "Don't ask again".

**Settings modal** (spine-only, no mock). A `modal-dialog` with plain rows (label left, control right). The **Advanced** section starts collapsed under a `collapsible-header` (uppercase label, amber chevron). Each **mapping-profile field** (`settings-field`): label in body-strong, a one-line explanation in muted small text below, a stepper/number field on the right with its unit. "Reset to defaults" and "Open profile file" as `button-small`; "Show log" and "Copy diagnostics" likewise.

**Log view.** `log-view`: Well fill, `{rounded.md}`, monospace rows "12:04:31.220  EVENT  UNDERRUN  t=1:12" with the timestamp in muted, error rows (NAK, EVENT errors) in `{colors.status-error}`; scrolls, newest at the bottom, auto-follow unless scrolled up. Opened from Settings › Advanced or a banner's "Details" (scrolled to the relevant entry, which gets an amber-wash row).

**Setup wizard.** → Reference: [mockups/key-wizard.html](mockups/key-wizard.html) (flash step with progress and failure, wiring step, Wired Communication step). Same window frame; the status strip with its three LEDs stays visible. Left: a `{spacing.wizard-list-width}` Panel column with a vertical clickable step list of 6 rows (`wizard-step`: Well fill, `{rounded.lg}`, a 24 px number circle with rim outline, title in body-strong, and a `{colors.status-ok}` check at the right when done; the current step has the 2 px amber ring and an amber number). Step titles: "Flash the bridge", "Flash the controller", "Wire the two boards", "Plug both boards in", "Turn on Wired Communication", "Test". Right: the step body — title in `{typography.card-title}` (parchment colour), one instruction line in Nunito Sans 15 px, one large illustration on a parchment sheet with corner brackets, a status line below it, and a footer with **Back** (ghost, left) and **Next** (primary amber, right; `button-disabled` until the step is done).

- **Status line:** muted small text with a painted spinner arc while waiting ("Waiting for the RPI-RP2 drive…"); on failure an error cross, the message in `{colors.text}`, and a `button-small` "Retry" at the right; after two failures a second row "Flash manually" with its own `button-small`.
- **Flash illustration** (steps 1–2): an original top-view board outline with USB-C at the top, BOOT drawn filled and ringed in `{colors.amber-ink}`, RESET outlined, a cable to a generic laptop outline, and two numbered pill callouts "1 · hold BOOT", "2 · plug into your PC".
- **Wiring diagram** (step 3): two original board outlines (pin circles with labels, "Board 1 · bridge (PC)" / "Board 2 · controller (Switch)"), three 2.6 px ink wires with pill labels "GP0 → GP1", "GP1 ← GP0" (this one dashed so the two crossing wires stay distinguishable), "GND – GND"; the 5V and 3V3 pins circled in ember with the line "Circled pins: 5V and 3V3. Never connect them." in ember; the owner's wiring photo beside the sheet (needs `egui_extras` image loaders); the four wiring rules as a numbered list below under a "Wiring rules" label.
- **Wired Communication illustration** (step 5): an original Hybrid-style schematic of a settings list on parchment — 4–5 generic rows drawn as ink bars without real labels, the one target row labelled "Pro Controller Wired Communication" with a toggle drawn On, an `{colors.amber-wash}` fill and a 2 px `{colors.amber-ink}` ring around it (amber-ink, because it sits on parchment). A muted line above the sheet reads "Switch LED: “Switch – not ready” until the setting is On." Below the sheet, the exact path as text: "System Settings › Controllers & Accessories › Pro Controller Wired Communication › On". No console screenshot.
- **Progress bar** (flashing): `progress-bar`, 8 px Well track, amber fill, full radius, with "Copying… 64 %" in muted small text above.
- **Port pick list** (Windows, no product string): one `port-pick-row` per candidate port — port name in body-strong ("COM5"), "2E8A:000A" in muted; selected row has the 2 px amber border; "Use this port" primary below.

**Empty state.** → Reference: [mockups/key-empty-state.html](mockups/key-empty-state.html). Inside the parchment card, centred: a dashed **drop zone** (`drop-zone`, about 470 px wide) with an original drop icon (arrow into a tray, `{colors.ink-soft}`), "Drop a .mid or score here" in 17 px bold, and "Open…" (ink fill) + "New song" (ink outline); while a file hovers over the window the zone border turns amber-ink. Below it are two groups with uppercase `{colors.ink-soft}` labels: "Or try an example" (chips, 600 weight) and "Recent" (chips, 400 weight); chips are ink text in `{colors.note}`-outlined pills. The card header stays, with transport and Undo/Redo dimmed and time "0:00 / 0:00". The song panel shows "No song loaded" in muted text with its "Open…" pill.

**Tooltips.** `tooltip`: Panel-raised, rim-soft border, `{rounded.md}`, body text; LED, disabled-Play and editor-error tooltips put the fix on its own line.

## Do's and Don'ts

| Do | Don't |
|---|---|
| Keep the parchment card the only bright surface on the main screen | Put parchment behind controls, panels or banners |
| Use amber only for selected / focused / about-to-start | Use amber for warnings, status or decoration |
| Pair every status colour with its symbol and a text label | Signal state by colour alone, or let warning orange stand in for amber |
| Reserve ember for official-song warning, playhead and live STOP; use `status-error` for errors outside the note view | Use any red for generic errors inside the note view |
| Draw ornaments, silhouettes, symbols and illustrations as original `Painter` shapes | Embed, trace or link any game or console screenshot, logo, crest, sprite, font or sound |
| Keep note bars clean; fingering lives once per lane header | Print fingerings inside every note bar |
| Dim out-of-range notes into their zones | Draw extra full-contrast lanes outside B4–F6 |
| Give the note view every extra pixel when the window grows | Grow the side panels past their token widths |
| Glow with `Shadow` on rounded rects and circles; halo behind text | Rely on text glow, blur shaders, gradients or image textures |
| Fill only convex shapes; build complex silhouettes from convex parts + one stroked outline | Fill a concave path |
| Paint one focus ring helper into every focusable widget | Rely on egui's default focus look |
| One modal at a time | Stack dialogs |
| Keep the Listen icon an original warm orb with one wing pair | Reproduce the game's fairy companion (two wing pairs, blue/white glow, face) or quote its catchphrase |
