---
baseline_commit: 7135c23
---

# Story 4.3: Shared painting helpers and modal dialog

Status: in-progress

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As the owner,
I want one set of painting helpers for focus, glow, dashes, brackets and symbols,
so that every widget looks consistent and the egui limits are handled once.

Origin: `epics.md` Epic 4, Story 4.3 (UX-DR3, UX-DR4, UX-DR7, UX-DR62; Story 4.2 decision "basic", test row G.10). It builds on Story 4.1 (`app/gui/`, crate `ocb-app`: theme tokens as the single source, fonts, region layout, `--gallery`, kittest tests, smoke mode) and Story 4.2 (every shared helper that draws an interactive or informative element emits `widget_info`).

**Owner constraint: host-side only.** No bench, no Switch, no board. Verification is `cargo test` (unit + `egui_kittest`, CPU only, no pixel snapshots: geometry and colours from `FullOutput::shapes`, accessibility from the AccessKit tree), the CI steps on three OS, and one look by the owner at the extended gallery (last task, AC 17).

**Who uses these helpers next:** 4.5 LEDs (glow, symbols, focus ring), 4.6 the "Firmware update needed" modal (OK only), 4.7 STOP and the bottom bar (glow, focus ring, painted ■ and ⏮ inside buttons), 4.8/4.9 gamepad (glow, convex fills, strokes), 5.x song panel and note view (digit cells, dashed rects, symbols, corner brackets), 6.x readiness confirm, countdown halo, end card, 9.x Staff. Keep the API small and obvious; do not build those widgets here.

## Acceptance Criteria

Numbers in points (egui logical points = `DESIGN.md` px). "Floored" means `max(width, 1 / pixels_per_point)`.

1. **Focus ring (UX-DR3, UX-DR62).** `ui::paint::paint_focus_ring(resp, radius, on_parchment)` paints, only while `resp.has_focus()`, one rect stroke of `spacing.focus-ring-width` (2 pt) in `amber` (`amber-ink` when `on_parchment`), `StrokeKind::Outside`, around `resp.rect` expanded by `spacing.focus-ring-offset` (2 pt), corner radius `radius + 2`. It paints on `resp.layer_id`, clipped to the ring's own outer rect, so the widget's own rect never clips it. Without focus it paints nothing. Every focusable themed widget of this story (the themed button of AC 10) calls it, and the gallery test of AC 13 proves that every focusable node in the gallery shows its ring when focused.
2. **Glow (UX-DR3).** `paint::paint_glow(painter, rect, radius, blur, color)` takes only a rect and a corner radius (a circle is a square rect with radius = half its side; there is no path variant). It paints one `epaint::Shadow { offset: [0, 0], blur, spread, color }` shape, painted before (under) the control it belongs to. `spread` is the smallest whole number for which the shadow rect's shorter side is > `blur` + 0.1 + 2 × stroke, so the tessellator never clamps the blur (`tessellator.rs:2098`: blur is capped at the shorter side). The blur values are theme tokens taken from `DESIGN.md` › `components` (`status-led` 8, `stepper-button-active` 10, `button-play-primary` 16, `button-stop-live` 18, `fairy-orb-icon` 8, and the new `modal-dialog` primary 12 of AC 14), used as the epaint `Shadow.blur` value as written.
3. **Text halo (UX-DR3).** `paint::paint_text_halo(painter, ink_rect, pad, blur, color)` paints a blurred rounded rect (via `paint_glow`) behind text: `ink_rect` is the galley's ink bounds (`galley.rect ∪ galley.mesh_bounds`, Story 4.1: Cormorant's old-style figures draw outside the layout rect), grown by `pad`, painted before the text. Pad and blur are caller arguments (the countdown values belong to Story 6.4).
4. **Dashed rounded rect (UX-DR3).** `paint::paint_dashed_rect(painter, rect, radius, stroke, dash, gap)` flattens the rounded rect into one closed point list (arcs flattened finely enough that no chord is more than 0.1 pt from the true arc) and calls `Shape::dashed_line_many` once on it, so the dash phase runs on around the corners. The stroke width is floored (AC 6). Every dash endpoint lies within 0.1 pt of the true outline; the summed dash length is perimeter × dash / (dash + gap) within one dash length.
5. **Convex-only fills (UX-DR3).** `paint::fill_convex(painter, points, fill, stroke) -> Result<(), NotConvex>` adds the polygon only if it has ≥ 3 points, non-zero area and every turn has the same sign (no concave vertex, no self-intersection); otherwise it adds nothing and returns `Err`. Outside `app/gui/src/ui/paint.rs`, no file in `app/gui/src/` builds a filled path (`convex_polygon(`, `PathShape::convex_polygon`, `PathShape { … fill`): a source scan enforces it. Rect and circle fills are not paths and stay free.
6. **Stroke floor (UX-DR3).** `paint::stroke(width, color, ppp) -> Stroke` returns `max(width, 1 / ppp)`; every stroke drawn by the helpers of this story goes through it. At ppp 1, 1.25, 1.5, 2 and 3 a 0.5 pt request gives 1/ppp pt and a 2 pt request gives 2 pt.
7. **Fixed-width digit cells (UX-DR3).** `paint::paint_digits(painter, pos, text, style, color)` draws each character with its own `Painter` text call: every ASCII digit sits in a cell as wide as the widest of `0`–`9` in that style's font (measured from the laid-out galleys, centred in the cell); any other character takes its own advance. "0:11 / 1:11" and "8:88 / 8:88" put every character at the same x. The widget variant `widgets::time(ui, text, style)` allocates the size, paints the same and is one `Role::Label` node named `text`; the painter variant adds no node (for times inside another widget, e.g. the Play sub-line).
8. **Painted symbols (UX-DR3).** `ui::symbols::Symbol` has exactly the 12 symbols ▶ ■ ⏮ ▾ ▲ ⌫ ↵ ↑ ♯ ♪ ↶ ↷. `paint::paint_symbol(painter, rect, symbol, color, ppp)` draws one without any text: only rect/circle fills, `fill_convex` polygons and floored strokes, all inside `rect` (stroke half-width included). ↷ is ↶ mirrored on the vertical axis. Geometry follows the approved mocks (Dev Notes › Symbol geometry). The widget variant `widgets::symbol(ui, symbol, size, color, name)` is one `Role::Image` node named `name` (plain words); `name` containing any of the 12 glyphs is refused (`debug_assert!` plus a test). The painter variant adds no node (symbols inside a button, the Roll or the Staff).
9. **Corner brackets (UX-DR4).** `paint::paint_corner_brackets(painter, rect, color, ppp)` draws four brackets mirrored per corner, each in a `spacing.corner-bracket-size` (22 pt) box inset `components.corner-bracket.inset` (4 pt, AC 14) from the corner: a 2 pt outer L path with a rounded (quadratic) elbow, a 1.2 pt inner quadratic curve (stroke only) and a filled dot of radius 1.6 pt at the elbow, all in `color` (`bracket` on parchment, `rim` on dark overlays). The main screen's note-view card now shows its four brackets in `bracket` (Story 4.1 left them to this story). They appear nowhere else in this story except the gallery.
10. **Modal dialog component (UX-DR7).** `ui::modal` renders a `ModalSpec { id, title, width, body, primary, secondary }` with `egui::Modal`: backdrop `scrim` over the whole content rect; frame `panel-raised`, 1 pt `rim`, radius `rounded.lg` (8), no drop shadow (DESIGN › Elevation: none as hierarchy), padding 20 / 26 pt, centred in the window; outer width = `width` clamped to 440–460 pt (default 460). Title in `dialog-title`, `parchment`, centred over the flourish rule: a 220 × 14 pt box with two 1 pt `rim` hairlines, two 1.2 pt leaf curves and one convex `rim` diamond (mock geometry, Dev Notes). Body: either one centred text in the new `dialog-body` token (14.5 pt, AC 14) or checklist rows in `dialog-body` with a 9 pt amber diamond bullet (convex), 7 pt vertical padding, 12 pt gap, separated by 1 pt `rule` lines (none after the last row). Actions right-aligned, 10 pt apart, 34 pt high, 18 pt side padding, text in the new `button-md` token (14 pt, 700): primary on the right (`amber` fill, `ink` text, glow 12 from AC 2), ghost secondary to its left (`well` fill, 1 pt `rim`, `text`). The primary button has keyboard focus when the dialog opens and Enter activates the focused button. A disabled button is drawn at 45 % opacity with no glow (`button-disabled`). The result is `ModalOutcome::{Primary, Secondary, Dismissed}`.
11. **One modal at a time, Esc (UX-DR7, UX-DR62).** `modal::Modals` (one per `App`, used by every screen) holds at most one open dialog: a request while one is open waits in a FIFO queue (a second request with the same `id` as the open or a queued one is ignored) and opens after the current one closes; two dialog areas are never shown in the same frame. Esc closes the open dialog with `Dismissed`, consumed by the modal only (Story 4.7 reads Esc for STOP at frame start, before any modal; nothing in this story consumes Esc earlier). A click on the backdrop does not close the dialog. While a dialog is open, Tab and Shift+Tab never move focus to a widget behind it.
12. **Accessibility, basic (Story 4.2, G.10).** Every helper that draws an interactive or informative element takes an accessible label and calls `Response::widget_info` with the closest `egui::Role`: the themed button `Role::Button` named its label; the standalone symbol `Role::Image` named with plain words (never the glyph); the time `Role::Label` named its text; the modal's dialog node (`Role::Dialog`, from egui's `UiKind::Modal`) named with the title. Decorative painting adds no node: focus ring, glow, halo, dashed rect, convex fill, corner brackets, the flourish, the checklist diamond bullets and the painter variants of symbol and time; the number of nodes under the harness root is the same in a frame with and without each of them drawn. The harness accessibility check stays on: no `with_accessibility_check(false)` in `app/gui/tests/` (source scan).
13. **Hidden debug gallery (extends Story 4.1).** `bonooru --gallery` (`Mode::Gallery`, still outside normal navigation) gains one section per helper: focus ring on dark and on parchment (a focusable sample button in each), glow at each blur token on its control shape, a text halo behind a Cormorant numeral sized from its ink bounds, dashed rects (1 pt and 1.6 pt, 5 / 3), convex fills, the stroke floor (0.5 pt vs 1 pt rules), times in digit cells next to the same text drawn normally, all 12 symbols at 10, 12, 14 and 18 pt with their names, corner brackets on a parchment sheet and in `rim` on a dark overlay, and buttons that open two sample dialogs (readiness-style: title "Ready to play?", three checklist rows, Cancel + Play; required-update-style: title "Firmware update needed", one body line, OK only; copy from `EXPERIENCE.md` › Microcopy, "draft"). At ppp 1.0, 1.25, 1.5, 1.75 and 2.0 no two texts overlap (ink bounds), no glyph is missing, and each colour and typography token name still appears exactly once. `--smoke-test --gallery` still passes on the three OS (G.7).
14. **Tokens stay single-source (UX-DR1).** The numbers this story needs that `DESIGN.md` states only in prose or in the approved mock are added to the `DESIGN.md` frontmatter and to `ui::theme`, and G.3 compares them both ways: typography `dialog-body` (Nunito Sans 14.5 px, 400, line height 1.5) and `button-md` (Nunito Sans 14 px, 700); `components.modal-dialog` `width: 460px`, `min-width: 440px`, `padding-y: 20px`, `padding-x: 26px`, `action-height: 34px`, `action-padding-x: 18px`, `action-gap: 10px`, `row-padding-y: 7px`, `row-gap: 12px`, `bullet-size: 9px`, `flourish-width: 220px`, `flourish-height: 14px`, `glow: 'Shadow blur 12px {colors.amber-glow}'`; `components.corner-bracket.inset: 4px`; and the glow blur of every `components.*.glow` string that names `Shadow blur <n>px` equals its `theme::glow` constant. No module other than `ui::theme` writes these numbers.
15. **eframe's `accesskit` feature guarded (Story 4.2 forward note).** A CI step fails if eframe's `accesskit` feature is not enabled by `ocb-app` in the normal (non-dev) dependency graph, for `x86_64-unknown-linux-gnu`, `aarch64-apple-darwin` and `x86_64-pc-windows-msvc` (G.8 amended). `egui_kittest`'s dev feature re-enables it in test builds, so this cannot be a `cargo test`.
16. **Everything green, red proofs.** On ubuntu, windows and macos: `cargo fmt --all --check`, `cargo test --locked --workspace` (G.1 to G.12; C rows unchanged), `cargo clippy --locked --workspace --all-targets -- -D warnings`, the deny gate, G.8 and the smoke run of G.7. Every new check has a red proof in the Debug Log that fails for the reason it claims (E3-A2 practice).
17. **Owner look (last task).** The owner runs `cargo run -p ocb-app -- --gallery` from `app/` and says, per item of Dev Notes › Owner eye check, "ok" or what to change. A changed token value is applied to `DESIGN.md` and `ui::theme` together (G.3 keeps them equal) and re-checked by G.6.

### AC → measurement (retro action E3-A1)

Every number in the ACs and what measures it. All host-side. Only "looks right" (AC 17) needs the owner.

| AC | Number | Measured by | Field / result |
|---|---|---|---|
| 1 | ring 2 pt, offset 2 pt, radius + 2, amber / amber-ink | `tests/paint.rs` (G.11) `g11_focus_ring`: kittest `build_ui` with one focusable widget at ppp 1, 1.25, 1.5, 2; `node.focus()`, `run()` | exactly one `Shape::Rect` with `stroke.width == 2.0`, `stroke.color == AMBER` (`AMBER_INK` with `on_parchment`), `stroke_kind == Outside`, `rect == resp.rect.expand(2.0)` (±0.01), `corner_radius == CornerRadius::same(r + 2)`; zero such shapes before focusing |
| 1 | not clipped by the widget | same test | the ring shape's `ClippedShape::clip_rect` contains `resp.rect.expand(4.0)` |
| 1 | every focusable gallery node shows its ring | `tests/gallery.rs` `g3_every_focusable_node_shows_its_ring` | for each AccessKit node with `Action::Focus` in the gallery (sample modals opened in turn), after `focus()` + `run()` a ring shape as above surrounds its bounds |
| 2 | Shadow offset [0, 0], blur = token, painted first | `paint.rs` `g11_glow` | the frame's first shape for the control is a `Shape::Rect` with `blur_width == blur`, `fill == color`, centred on `rect` (offset 0); the control's own shape has a higher index |
| 2 | spread so the blur is never clamped | `paint.rs` `g11_glow_never_clamped` at rect sizes 4, 11, 26, 60 pt and blurs 8, 10, 12, 16, 18 | `shadow_rect.size().min_elem() > blur + 0.1 + 2 × stroke` and `spread` is the smallest whole number that satisfies it |
| 2 | blur values 8 / 10 / 16 / 18 / 8 / 12 | `tests/design_tokens.rs` (G.3) `g3_glow_tokens` | each `components.<name>.glow` string's `blur <n>px` == `theme::glow::<NAME>` |
| 3 | halo from ink bounds, behind the text | `paint.rs` `g11_text_halo` with a Cormorant "3" at 128 pt | blurred rect ⊇ `ink_rect.expand(pad)`; its shape index < the text shape's index |
| 4 | ≤ 0.1 pt chord error; dashes on the outline; total dash length | `paint.rs` `g11_dashed_rect` at radii 0, 4, 12, 9999 (pill) and dash/gap 5/3, 4/4 | max distance of every dash endpoint to the analytic rounded-rect outline ≤ 0.1; Σ dash lengths = P × d / (d + g) ± d (P analytic perimeter); `dashed_line_many` called once (one contiguous run of shapes) |
| 5 | ≥ 3 points, non-zero area, one turn sign | `paint.rs` `g11_fill_convex` | triangle, square, rotated diamond, the flourish diamond: `Ok` and one shape added; L-shape, bow-tie, 2 points, three collinear points: `Err` and no shape added |
| 5 | no filled path outside `paint.rs` | `paint.rs` `g11_no_filled_path_outside_paint` (source scan of `app/gui/src/`, comments skipped, self-tested on good and bad snippets) | no `convex_polygon` / `PathShape::convex_polygon` / `fill:` in a `PathShape` literal outside `src/ui/paint.rs` |
| 6 | 1 physical px floor | `paint.rs` `g11_stroke_floor` (unit) | ppp 1 / 1.25 / 1.5 / 2 / 3: `stroke(0.5).width == 1/ppp` (0.5 at ppp 2 stays 0.5), `stroke(2.0).width == 2.0` |
| 6 | helpers use the floor | `paint.rs` `g11_helper_strokes_floored`: every helper drawn at ppp 1 with a 0.5 pt request | every stroke width in the output ≥ 1 / ppp − 1e-6 |
| 7 | cell = widest digit; same x | `paint.rs` `g11_digit_cells` in `body`, `button-lg-sub`, `small` | the x positions of the text shapes of "0:11 / 1:11" and "8:88 / 8:88" are equal pairwise (±0.01); each digit cell width == max advance of `0`–`9` |
| 7 | time is a Label node | `tests/a11y.rs` (G.10) | `get_by_role_and_label(Role::Label, "0:12 / 0:31")` exists; painter variant: node count unchanged |
| 8 | 12 symbols, fills convex, inside rect, no text, ↷ mirrors ↶ | `paint.rs` `g11_symbols` for each `Symbol::ALL` at 10, 12, 14, 18 pt, ppp 1 and 2 | ≥ 1 shape each; no `Shape::Text`; every filled `Shape::Path` passes the convexity check; every shape's visual bounds ⊆ `rect` (±0.01); the 12 shape lists are pairwise different; ↷ points == ↶ points reflected about `rect.center().x` |
| 8 | Image node, plain-words name | `a11y.rs` | `get_by_role_and_label(Role::Image, "Play")` etc.; `widgets::symbol` with a name containing `▶` panics in debug (`#[should_panic]`) |
| 9 | 22 pt box, 4 pt inset, 2 / 1.2 pt strokes, 1.6 pt dot | `paint.rs` `g11_corner_brackets` at ppp 1 and 2 | 4 × (1 outer path stroke width 2, 1 quadratic Bézier stroke width 1.2, 1 circle radius 1.6); each corner's shapes inside the 22 × 22 box whose outer corner is the rect corner moved 4 pt inward on both axes; the four sets are mirror images |
| 9 | brackets on the note-view card | `tests/main_screen.rs` (G.1) `g1_note_card_has_corner_brackets` at 1024×680 and 1280×800 | four bracket sets in `BRACKET` inside `regions().note_card`, one per corner as above; the G.1 fills still pass |
| 10 | width 440–460, centred, padding 20 / 26 | `tests/modal.rs` (G.12) `g12_frame` at window 1024×680 and 1280×800, ppp 1 and 1.5, widths 400, 440, 450, 460, 500 | the modal frame `Shape::Rect` (`fill == PANEL_RAISED`, stroke 1 pt `RIM`, radius 8, no shadow shape under it) has width == clamp(w, 440, 460) ±0.5 and centre == content-rect centre ±1; title text's top ≥ frame top + 20, buttons' right ≤ frame right − 26 |
| 10 | scrim over the whole window | `modal.rs` `g12_scrim` | a filled rect `== ctx.content_rect()` with `fill == SCRIM` |
| 10 | flourish 220 × 14, 1 / 1.2 pt, convex diamond | `modal.rs` `g12_flourish` | 2 line strokes width 1 `RIM`, 2 (or 4) quadratic strokes width 1.2, 1 filled convex `RIM` path; all inside a 220 × 14 box centred under the title |
| 10 | rows: 14.5 pt text, 9 pt diamond, 7 / 12 pt, 1 pt rule | `modal.rs` `g12_checklist` with 3 rows | 3 texts with font size 14.5; 3 amber filled diamonds whose side == 9 (diagonal 9√2 ±0.01); text left − diamond right == 12 ±0.5; 2 `RULE` lines width 1 (none after the last row); row height == text height + 14 ±0.5 |
| 10 | actions 34 high, 10 apart, padding 18, text 14 / 700 | `modal.rs` `g12_actions` | both button rects 34 ±0.01 high, gap 10 ±0.5, primary right of ghost; each button width == text width + 36 ±0.5; primary fill `AMBER` with a glow rect (blur 12) under it; ghost fill `WELL` stroke 1 pt `RIM` |
| 10 | primary focused on open; Enter activates | `modal.rs` `g12_initial_focus_and_enter` | after opening, the primary node `is_focused()`; `key_press(Enter)` → outcome `Primary`; focus ghost + Enter → `Secondary` |
| 10 | 45 % opacity, no glow when disabled | `modal.rs` `g12_disabled_button` | the button's fill and text alpha == round(0.45 × α); no blurred rect under it |
| 11 | depth one, FIFO, dedup | `modal.rs` `g12_one_at_a_time` | open A, B, A: only A shown (one `Role::Dialog` node, one scrim rect); close A → B shown; close B → none; never two scrim rects in any frame |
| 11 | Esc closes, backdrop click does not, Tab stays inside | `modal.rs` `g12_esc_backdrop_tab` | `key_press(Escape)` → `Dismissed`; a click at the window's top-left (backdrop) → still open; with a focusable gallery button behind, 5 × Tab and 5 × Shift+Tab → focus is always a node inside the dialog |
| 12 | roles, names, no nodes for decoration | `tests/a11y.rs` (G.10) | `Role::Button` "Play" / "Cancel" / "OK", `Role::Image` per symbol name, `Role::Label` time, `Role::Dialog` named with the title (`get_by_role_and_label`); node count under the root equal with and without each decorative helper and painter variant; source scan for `with_accessibility_check(false)` |
| 13 | ppp 1.0–2.0, no overlap, glyphs, names once | `tests/gallery.rs` (G.3) extended: `PPPS` = 1.0, 1.25, 1.5, 1.75, 2.0; the tall harness grows if needed | no ink-rect overlap of any two texts; no missing glyph (kittest default); every colour and typography token name exactly once |
| 13 | smoke with the gallery | CI smoke step (G.7) | `OCB_SMOKE ok …` for `--smoke-test --gallery` on the three OS |
| 14 | new frontmatter keys equal the theme | `design_tokens.rs` (G.3) `g3_component_sizes` extended, typography both ways | each listed key == its `theme` constant; a theme constant without a key (or the reverse) fails naming it |
| 15 | accesskit feature in the normal graph, 3 targets | CI step (G.8) `cargo tree --locked -p ocb-app -e normal,features -i eframe --target <t>` | a line `eframe feature "accesskit"` whose next line is `└── ocb-app v…` or `├── ocb-app v…` for each target (output shape verified at story creation, Dev Notes) |
| 16 | three OS | the `app` workflow matrix | all steps green on `ubuntu-24.04`, `windows-latest`, `macos-latest` |
| 2, 10, 13 | contrast of the new pairs | `tests/contrast.rs` (G.6) extended | `ink` on `amber` ≥ 4.5:1 (primary text); `text` on `well` ≥ 4.5:1 (ghost text); `amber` on `panel`, `panel-raised` and `well` ≥ 3:1 (focus ring on dark, non-text); `parchment` on `panel-raised` ≥ 4.5:1 (dialog title) |

No AC number lacks a host-side measurement. Glow softness, symbol legibility at 10 pt and the overall match with the mock are judgements, not numbers: AC 17.

## Tasks / Subtasks

- [x] **Task 1: Tokens first (AC 14).**
  - [x] 1.1 `DESIGN.md` frontmatter: add the typography tokens `dialog-body` and `button-md`, the `components.modal-dialog` keys and `components.corner-bracket.inset` of AC 14 (values from `mockups/key-dialogs.html`, which `DESIGN.md` › Modal dialog references; the prose already says "checklist rows (14.5 px)"). Add one sentence to the Modal dialog prose naming the width rule (460 default, 440 minimum) and that the dialog has no drop shadow. Bump `updated:`. Do not change any existing value.
  - [x] 1.2 `ui/theme.rs`: `DIALOG_BODY`, `BUTTON_MD` in `typography_tokens()`; `components::modal_dialog::{WIDTH, MIN_WIDTH, PADDING_Y, PADDING_X, ACTION_HEIGHT, ACTION_PADDING_X, ACTION_GAP, ROW_PADDING_Y, ROW_GAP, BULLET_SIZE, FLOURISH_WIDTH, FLOURISH_HEIGHT}`, `components::CORNER_BRACKET_INSET`; `pub mod glow { STATUS_LED = 8, STEPPER_ACTIVE = 10, PLAY_PRIMARY = 16, STOP_LIVE = 18, ORB = 8, MODAL_PRIMARY = 12 }` as `u8` (epaint `Shadow.blur` is `u8`).
  - [x] 1.3 `tests/design_tokens.rs`: extend `g3_component_sizes` to every new component key; add `g3_glow_tokens` (every `components.*.glow` string with `Shadow blur <n>px` maps to a `theme::glow` constant and equal; a glow string with a blur and no constant fails; `'none'` and the inset-wash string are skipped by name). Red proof: change one value on one side.
  - [x] 1.4 `tests/contrast.rs`: the four new pairs of the table. Red proof: a darker `amber` fails the ring pair.
- [x] **Task 2: `ui/paint.rs`, the painter-level helpers (AC 1–7, 9).** No `Ui`, no nodes, except `paint_focus_ring`, which takes the `Response`. Every function takes `ppp` (or reads it from the painter's context) where it strokes.
  - [x] 2.1 `stroke(width, color, ppp)`; `paint_focus_ring(resp, radius, on_parchment)` (painter `Painter::new(resp.ctx.clone(), resp.layer_id, ring_outer_rect)`); `paint_glow`; `paint_text_halo`.
  - [x] 2.2 `rounded_rect_points(rect, radius, max_chord_error)` (pure, unit-tested: closed, first point repeated at the end, radius clamped to half the shorter side, every point on the outline); `paint_dashed_rect`.
  - [x] 2.3 `is_convex(points)` (pure) and `fill_convex`; `NotConvex` error type (`Debug`, `Display`).
  - [x] 2.4 `paint_digits` (+ a pure `digit_cell_width(fonts, font_id)` so the widget variant can size itself).
  - [x] 2.5 `paint_corner_brackets` from the mock path (Dev Notes › Corner bracket geometry); `paint_flourish(painter, center_top, color, ppp)` for the modal.
- [x] **Task 3: `ui/symbols.rs` (AC 8).** `pub enum Symbol { Play, Stop, ToStart, ChevronDown, TriangleUp, Backspace, Return, ArrowUp, Sharp, Note, Undo, Redo }` with `ALL`, `glyph()` (the character, for tests and docs only, never painted) and `paint_symbol`. Each symbol in a unit box from the mock viewBox, scaled into `rect` (aspect kept, centred). Fills only through `fill_convex`; ♪ heads are ellipses as convex polygons (or circles); ↶ from the mock path, ↷ = mirrored points.
- [x] **Task 4: `ui/widgets.rs`, the nodes (AC 1, 7, 8, 10, 12).**
  - [x] 4.1 `button(ui, ButtonKind::{Primary, Ghost}, label, enabled, icon: Option<Symbol>) -> Response`: `allocate_exact_size` with `Sense::click()` (focusable), paint glow (primary, enabled only), fill, stroke, optional leading symbol (painter variant), text in `button-md`; `paint_focus_ring(&resp, rounded::LG, false)`; `resp.widget_info(|| WidgetInfo::labeled(Role::Button, enabled, label))`. Disabled = `button-disabled` (fill and text × 0.45 alpha, no glow) and `ui.add_enabled_ui` semantics (not clickable).
  - [x] 4.2 `symbol(ui, symbol, size, color, name) -> Response` (`Sense::hover()`, `Role::Image`, `debug_assert!` on glyphs in `name`); `time(ui, text, style) -> Response` (`Role::Label`).
- [x] **Task 5: `ui/modal.rs` (AC 10, 11, 12).**
  - [x] 5.1 `ModalSpec`, `ModalBody::{Text(String), Checklist(Vec<String>)}`, `ModalAction { label, enabled }`, `ModalOutcome`; `Modals { open: Option<Open>, queue: VecDeque<ModalSpec> }` with `request(spec)`, `is_open()`, `show(ctx) -> Option<(Id, ModalOutcome)>`.
  - [x] 5.2 Rendering with `egui::Modal::new(id).backdrop_color(theme::SCRIM).frame(Frame { fill: PANEL_RAISED, stroke: 1 pt RIM, corner_radius: LG, inner_margin: 20/26, shadow: Shadow::NONE, .. })`; set the content width so the outer frame width is the clamped `width` (account for margins and stroke; G.12 measures the painted rect). Title, `paint_flourish`, body, action row laid out right-to-left (primary first in code, so it is rightmost).
  - [x] 5.3 Close logic: do **not** use `ModalResponse::should_close()` (it also closes on a backdrop click). Close on Esc only when `is_top_modal && !any_popup_open`, via `ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))`, or on an action click.
  - [x] 5.4 Initial focus: `request_focus()` on the primary button in the first frame the dialog is shown (store "focused once" per open dialog). Verify the Tab behaviour of AC 11 in the test; if egui lets Tab reach a widget behind the modal layer, add a focus guard (keep focus inside the dialog's widget ids) and say so in the Debug Log.
  - [x] 5.5 Name the dialog node: write the title as the label of the `Role::Dialog` node (find its id: the modal `Area`'s id / `ModalResponse::response.id`; check with `has_accesskit_node`, then `Context::accesskit_node_builder(id, |n| n.set_label(title))` or `labelled_by` the title label). G.10 asserts `get_by_role_and_label(Role::Dialog, title)`.
  - [x] 5.6 `app.rs`: `App` owns `Modals` (`pub fn modals_mut(&mut self)` for later stories and tests); `ui()` draws the screen first, then `self.modals.show(ctx)`, then the smoke bookkeeping. Leave a comment where Story 4.7 reads Esc (at the top of `ui()`/`logic()`, before any screen or modal code).
- [x] **Task 6: Main screen brackets (AC 9).** `main_screen::paint_region` for `NoteView`: `paint_corner_brackets(painter, card, theme::BRACKET, ppp)` after the card fill. G.1 test `g1_note_card_has_corner_brackets`; the existing G.1 tests must stay green unchanged.
- [x] **Task 7: Gallery (AC 13).** Extend `ui/gallery.rs` with the sections of AC 13 after the existing ones, built only from the helpers (no new literals: sizes from `theme`; sample sizes that are not tokens, e.g. a 120 × 28 sample control, stay as they are today in the gallery). Gallery state (`Modals` comes from `App`; the gallery gets `&mut Modals`). Copy from `EXPERIENCE.md` › Microcopy (the "Ready to play?" checklist; "Firmware update needed" body "This firmware is too old for this app. Update both boards to continue.", button "OK" as in Story 4.6). Avoid every colour and typography token name as a visible label (the "exactly once" test), and only use characters the bundled fonts cover (Nunito Sans has `’ – · …`, no arrows or music signs).
- [x] **Task 8: Tests (all ACs).** New files `tests/paint.rs` (G.11), `tests/modal.rs` (G.12), `tests/a11y.rs` (G.10); extend `tests/gallery.rs`, `tests/main_screen.rs`, `tests/design_tokens.rs`, `tests/contrast.rs`. Pure parts as unit tests in their modules. File-level `#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]` in test files as today. Helper: a `shapes_flat(&FullOutput) -> Vec<(ClippedShape-level index, &Shape)>` that walks `Shape::Vec` (as `text_rects` in `tests/gallery.rs` does).
- [x] **Task 9: CI guard (AC 15).** `.github/workflows/app.yml`: a step after "No eframe wayland feature (G.8)", Linux only (the feature graph is computed per `--target`, so one runner checks all three), named "eframe accesskit feature on (G.8, Story 4.2)": for each target run `cargo tree --locked -p ocb-app -e normal,features -i eframe --target $t`, and fail with `::error::` unless a line `eframe feature "accesskit"` is directly followed by a line ending in `ocb-app v0.0.0 (…)` (anchor on `ocb-app v`, not the path). Red proof locally: drop `"accesskit"` from `app/gui/Cargo.toml`, the step's script fails for each target (and `cargo test` still passes, which is why the step exists); restore the file and `Cargo.lock` from a copy.
- [x] **Task 10: Docs.** `docs/test-plan.md` › App GUI tests: G.10 marked implemented (keep its text; add the dialog node and the time label), new rows G.11 (painting helpers) and G.12 (modal dialog) as in the AC → measurement table, G.1 (brackets), G.3 (new tokens, glow tokens, gallery ppp list), G.6 (new pairs), G.8 (accesskit guard); intro sentence updated. `app/README.md`: the new modules (`ui::paint`, `ui::symbols`, `ui::widgets`, `ui::modal`) in the `gui/` line and one line on the gallery's helper sections.
- [ ] **Task 11: Verify (AC 16).** Locally in `app/` with `/opt/homebrew/opt/rustup/bin` first on `PATH`: fmt, clippy `-D warnings`, `cargo test --locked --workspace`, `cargo deny --locked check licenses bans`, the G.8 scripts, `cargo run -p ocb-app -- --smoke-test` and `-- --smoke-test --gallery`. Red proofs in the Debug Log (list in Dev Notes › Testing standards). Push the branch and get the `app` workflow green on the three OS.
- [ ] **Task 12: Owner look (AC 17), last.** Ask the owner to run `cargo run -p ocb-app -- --gallery` from `app/` and to answer each item of Dev Notes › Owner eye check with "ok" or a change. Apply changes to `DESIGN.md` + `theme.rs` together, rerun G.3, G.6 and G.11. If the owner is not available, leave this task open and set the story to `review` with the open item in the Completion Notes. Then `sprint-status.yaml`: story to `review`.

## Dev Notes

### Scope in one paragraph

A painting toolkit and one container. `ui::paint` holds the low-level helpers that handle egui's limits once (focus ring, glow, halo, dashes, convex fills, stroke floor, digit cells, brackets, flourish); `ui::symbols` paints the 12 symbols no bundled font covers; `ui::widgets` is the thin layer that turns a helper into an accessible node (button, standalone symbol, time); `ui::modal` is the dialog component plus a depth-one host that every screen shares. Nothing here talks to a board. No LED (4.5), no STOP (4.7), no checkbox ("Don't ask again" belongs to the readiness confirm, Epic 6), no transport or history buttons (Epics 5 and 9), no Space handling (4.7), no global Esc for STOP (4.7).

### Decisions taken at story creation (change only with a reason in the Debug Log)

- **D1. Four modules, not one.** `paint` (no nodes), `symbols` (geometry), `widgets` (nodes), `modal` (container + host). It keeps the G.10 rule visible in the module structure: anything in `paint`/`symbols` adds no node; anything in `widgets`/`modal` adds exactly the documented ones.
- **D2. Mock numbers go into the `DESIGN.md` frontmatter (AC 14).** Story 4.1 AC 3 made `ui::theme` the single source and G.3 checks it against the frontmatter. The modal and bracket numbers exist only in the mock (and 14.5 px in the prose). Writing them as frontmatter keys keeps both rules. No value is invented: each comes from `mockups/key-dialogs.html` (`.modal`, `.checklist`, `.diamond`, `.btn`, `.flourish`, `.corner`). The mock's `box-shadow` on the modal is dropped because the spine wins ("No drop shadows as hierarchy"). Listed for the owner in Open questions 3.
- **D3. Glow values are epaint numbers.** `DESIGN.md` writes them as `Shadow blur <n>px` and names `epaint::Shadow`, so `<n>` is `Shadow.blur` as is. egui's blur is a linear penumbra of width `blur` centred on the edge (it reaches `blur / 2` outside), softer and shorter than a CSS Gaussian with the same number. Whether the result is strong enough is judged by eye (AC 17 item 1), and the fix is a token change, not code.
- **D4. Modal close rules.** Esc closes; a backdrop click does not (`should_close()` is not used). Reasons: EXPERIENCE says only "Esc closes the topmost dialog"; the required-update dialog is "blocking"; an accidental click beside the readiness confirm should not count as an answer. Listed in Open questions 1.
- **D5. Depth one with a FIFO queue.** "Modal stack depth one" (UX-DR7) bans stacking, not a second request. Queueing (deduplicated by id) never loses a dialog and never stacks. Example: the firmware dialog (4.6) arrives while the readiness confirm (Epic 6) is open; it shows after the user answers. Listed in Open questions 2.
- **D6. Width 460 by default, 440 allowed.** `DESIGN.md`: "about 440–460 px"; the mock uses 460 for the readiness confirm and 440 for the firmware dialog. `width` is clamped to that range.
- **D7. The focus ring paints on the widget's layer, clipped to its own outer rect.** `Response` carries no clip rect, and clipping to the widget rect would hide the ring (it is outside the widget by design). Accepted side effect: inside a scroll area the ring can draw up to 4 pt over the area's edge. Focus visibility wins (UX-DR62).
- **D8. No themed replacement of egui's built-in widgets yet.** Built-in widgets keep egui's focus look until a story uses one (TextEdit in 9.2, ComboBox in 5.x): that story wraps it and calls `paint_focus_ring`. Leave a comment in `theme::visuals()` saying so (it currently says "Widget restyling (focus ring, buttons) is Story 4.3").
- **D9. Symbols are always painted.** No coverage check of Cormorant: the same symbol must look the same in every context (Nunito has none of the 12; Cormorant has only ▶ ■ ▲ ↑, Story 4.1 Dev Notes).

### Current state of the files this story touches (read before editing)

- `app/gui/src/ui/theme.rs`: colour macro + tables, `rounded`, `spacing`, `components::STATUS_LED_SIZE`, `TypeStyle` constants and `typography_tokens()`, `corner()`, `rich()`, `label()`, `visuals()`. Adding typography tokens means adding them to `typography_tokens()` (the gallery and G.3 iterate it, G.4 checks the families).
- `app/gui/src/ui/gallery.rs`: `show(ui)` = frame fill, `ScrollArea` with sections (colour swatches in a `Grid`, status strips, amber-ink ring samples, scrim, text score, typography `Grid` with ink-bounds specimens). Its `ring_sample` draws the amber-ink ring by hand; replace that with `paint_focus_ring`'s geometry only if the two stay identical (G.3's `g3_amber_ink_ring_clears_its_label` looks for exactly two `AMBER_INK` stroked rects: new ring samples on parchment add more, so update that test's count or filter by section).
- `app/gui/src/ui/main_screen.rs`: `show(ui)` fills five regions in reading order; `NoteView` paints `frame` then the parchment card (`XL` radius). `hairline()` has a unit test. Add the brackets only.
- `app/gui/src/app.rs`: `App { mode, smoke }`; `logic()` re-asserts dark; `ui()` draws the screen then `smoke_frame`. `App` derives `Debug`; `Modals` must implement `Debug` (workspace lint `missing_debug_implementations`).
- `app/gui/tests/gallery.rs`: `g3_gallery_lists_every_colour_and_typography_token` (1024×680, `query_all_by_label(name).count() == 1`), `g3_gallery_texts_never_overlap` (1024×4000 harness, ppp 1/1.5/2, `texts.len() > 60`), `g3_amber_ink_ring_clears_its_label` (expects exactly 2 amber-ink rings and 4 labels named `parchment`/`parchment-header`). All three must stay green; extend `PPPS` to 1.25 and 1.75 and raise the harness height if the longer gallery needs it (off-screen texts are not painted, so they would silently escape the overlap check: assert the last section's text is in the output).
- `app/gui/tests/main_screen.rs`, `window.rs`, `fonts.rs`, `design_tokens.rs`, `contrast.rs`, `licences.rs`: unchanged meaning. `fonts.rs` scans `app/gui/src/` for `strong(`: new code must not call it.
- `.github/workflows/app.yml`: steps Format, Test, Clippy, pins, G.8 (ubuntu), Build, Smoke (three OS, `check-smoke.sh`), deny, summary. Add the accesskit step next to G.8.
- `docs/test-plan.md` › App GUI tests: G.1–G.10 (G.10 "scheduled for Story 4.3").
- Must be preserved: G.1–G.9 green on three OS; the smoke verdict (`--smoke-test --gallery` must still reach 3 completed frames: no modal opens by default, nothing in the gallery requests repaints forever).

### egui / epaint 0.37 facts (checked in `~/.cargo/registry/src/*/{egui,epaint,egui_kittest,kittest}-*` and with `ctx7` on `/emilk/egui/0.37.0`)

- **Shadow** (`epaint-0.37.0/src/shadow.rs`): `Shadow { offset: [i8; 2], blur: u8, spread: u8, color }`; `as_shape(rect, corner_radius) -> RectShape` = the rect translated by `offset`, expanded by `spread`, `RectShape::filled(..).with_blur_width(blur)`; `margin()` = spread + 0.5 × blur. Paint with `painter.add(shadow.as_shape(rect, r))`.
- **Blur clamp** (`tessellator.rs:2098`): `blur_width` is capped at `rect.size().min_elem() − 0.1 − 2 × stroke.width`, and the corner radius grows by `0.5 × blur`. A glow on an 11 pt LED with blur 16 is silently clamped to ~10.9 unless the shadow rect is grown (AC 2's spread rule). The blur is linear in sRGBA gamma space.
- **Dashes** (`shapes/shape.rs:178–255`): `Shape::dashed_line(path, stroke, dash, gap) -> Vec<Shape>`, `dashed_line_many(points, stroke, dash, gap, &mut Vec<Shape>)`, `_with_offset` variants take arrays of dash/gap lengths and a start offset. The path is open: close a rounded rect by repeating its first point. Dashes come out as line segments.
- **Fills** (`shapes/path_shape.rs`): "Fill is only supported for convex polygons"; `closed` must be true for a fill; prefer clockwise order. `Shape::convex_polygon(points, fill, stroke)`. Beziers: `QuadraticBezierShape::from_points_stroke([p0, p1, p2], closed, fill, stroke)` (stroke only for the brackets, `fill = TRANSPARENT`).
- **Pixels**: `Painter::round_to_pixel_center(f32)`, `ctx.pixels_per_point()`. `RectShape` has `round_to_pixels: Option<bool>` (the tessellator rounds rects by default).
- **Modal** (`containers/modal.rs`): `Modal::new(id).backdrop_color(c).frame(f).show(ctx, |ui| ..) -> ModalResponse { response, backdrop_response, inner, is_top_modal, any_popup_open }`. The area is `UiKind::Modal`, anchored centre, `Order::Foreground`; the backdrop covers `ctx.content_rect()` and senses click + drag; default frame is `Frame::popup` (with a shadow: replace it). `should_close()` = backdrop clicked **or** `ui.close()` called **or** (top modal, no popup, Esc consumed). egui itself allows stacking modals ("the topmost is the most recently shown"): depth one is the app's job (`Modals`).
- **Modal layer and input**: widgets behind the top modal layer do not get pointer interaction (`Memory::is_above_modal_layer`, `layer_id_at`). Whether Tab can still focus them is not documented: AC 11's test decides; add a guard only if needed.
- **Focus and keys** (`context.rs:1761`): a focused widget that senses clicks is clicked by Space **or** Enter. Space is Story 4.7's (consumed at frame start); Enter activating the focused modal button is wanted here. Esc in `Memory::begin_pass` also clears focus.
- **AccessKit roles**: `UiKind::Modal` and `Popup` map to `Role::Dialog` (`ui_stack.rs:76`); the root is `Role::Window`. `Response::widget_info(|| WidgetInfo::labeled(role, enabled, label))` (`response.rs:951`) gives a custom widget its node; `Response::labelled_by(id)` and `accessible_name(name)` (`response.rs:1065, 1087`); `Context::accesskit_node_builder(id, |node| ..)` and `has_accesskit_node(id)` (`context.rs:4203, 4215`). Only widgets that report `WidgetInfo` get a node.
- **kittest** (`egui_kittest-0.37.0`, `kittest-0.5.0`): `Harness::builder().with_size(..).with_pixels_per_point(..).build_ui(|ui| ..)` / `build_ui_state` / `build_eframe`; `run()`, `step()`, `output()` (`FullOutput`), `key_press(Key)`, `key_press_modifiers`, `event(..)`; queries `get_by_role_and_label(Role, &str)`, `query_all_by_role`, `get_by_label`; `Node::focus()`, `is_focused()`, `click()` (pointer), `click_accesskit()`, `rect()`; `root()` + `children_recursive()` for node counts. The accessibility check panics on any visible unnamed input-role node after `run()` (`accessibility.rs`); `Image`, `Label` and `Dialog` are not input roles, so their names are checked by G.10 itself.
- **Fonts**: `Galley::mesh_bounds` for ink; egui shapes text with no OpenType features (no `tnum`, no `lnum`), hence the digit cells. Nunito Sans digits are lining but proportional; Cormorant's are old-style (Story 4.1 forward note): if a time is ever drawn in Cormorant, size its cells from ink bounds too.

### Symbol geometry (original shapes; sources in the approved mocks, `mockups/`)

| Symbol | Name in the gallery | Source / shape |
|---|---|---|
| ▶ | Play | `key-dialogs.html` transport: `M1.5 0.5 L9.5 5.5 L1.5 10.5 Z` in 10 × 11 (convex triangle); bottom-bar Play `M2 1 L15 9 L2 17 Z` in 16 × 18 |
| ■ | Stop | `rect 9 × 9 rx 1` (transport), `14 × 14 rx 2` in 16 × 16 (STOP): rounded rect fill |
| ⏮ | Back to start | bottom-bar "Play from start": `rect x1 y2 w3 h14` + a left-pointing triangle in 18 × 18 (two convex fills; read the full path in `key-dialogs.html`) |
| ▾ | Expand | track-picker caret `M0.5 0.5 L9.5 0.5 L5 6.5 Z` in 10 × 7 |
| ▲ | Up | the same caret pointing up |
| ⌫ | Backspace | key-chip glyph in the gamepad (`directions-oot-2026-11-20.html`, direction 3, and `key-dialogs.html` pad): pentagon outline + × (strokes) |
| ↵ | Return | key-chip glyph: down-then-left stroke with an arrowhead (strokes) |
| ↑ | Arrow up | fingering arrow in lane headers ("X+D↑"): stem + head (strokes) |
| ♯ | Sharp | lane names ("D♯5"): two slanted verticals + two thick horizontals (strokes) |
| ♪ | Note | filename icon `key-dialogs.html`: the mock draws a beamed pair (♫); `DESIGN.md` names ♪: one ellipse head (convex polygon) + stem + flag (strokes). Keep ♪ |
| ↶ | Undo | `M4.2 2.2 L1.4 5 L4.2 7.8` + `M1.8 5 L8 5 Q11.6 5 11.6 7.8 Q11.6 10.6 8.4 10.6 L5.6 10.6`, stroke 1.6, round caps, in 13 × 12 |
| ↷ | Redo | ↶ mirrored (`scaleX(-1)`) |

Stroke widths in the mock are for its nominal size; scale them with the symbol and floor them (AC 6). The names are what the gallery passes; real call sites pass their own context ("Play from cursor", "Undo").

### Corner bracket geometry (`key-dialogs.html` `.corner`, 22 × 22 viewBox, top-left; mirror for the others)

Outer: `M2,20 L2,6 Q2,2 6,2 L20,2`, stroke 2. Inner: `M7,14 Q7,7 14,7`, stroke 1.2, no fill. Dot: centre (7, 7), r 1.6, filled. Box placed 4 pt inside the card corner (`.corner.tl { top: 4px; left: 4px }`); `tr` = `scaleX(-1)`, `bl` = `scaleY(-1)`, `br` = both. Colour `#7a5a33` = `bracket`.

### Flourish geometry (`key-dialogs.html` `.flourish`, 220 × 14, margin 6 above / 10 below)

Hairlines `M4 7 L92 7` and `M128 7 L216 7`, stroke 1. Leaves `M92 7 Q100 1 106 7 Q100 13 92 7` and `M128 7 Q120 1 114 7 Q120 13 128 7`, stroke 1.2, no fill (each leaf = two quadratic curves). Diamond `M110 2.5 L114.5 7 L110 11.5 L105.5 7 Z`, filled (convex). Colour `#a37a3e` = `rim`. Checklist diamond: CSS 9 × 9 square rotated 45°, `amber`.

### Owner eye check (AC 17, Task 12)

Run from `app/`: `cargo run -p ocb-app -- --gallery`. Answer each with "ok" or a change:

1. **Glow strength** at each blur token (LED 8, stepper 10, modal primary 12, Play 16, STOP 18) compared with the mocks (D3).
2. **Symbols** at 10, 12, 14, 18 pt: is each one recognisable, especially ⌫, ↵, ♯ and ♪ at 10 pt?
3. **Corner brackets** on parchment and in `rim` on dark, and on the main screen's note-view card (`cargo run -p ocb-app`).
4. **Both sample dialogs** against `mockups/key-dialogs.html`: width, flourish, checklist rows, buttons, scrim. Open each, press Tab, Enter and Esc.
5. **Focus ring** on dark and on parchment: visible enough; acceptable at your display scale.
6. **Digit cells**: the time does not jitter when digits change (compare with the normally drawn line next to it).

### Testing standards

- Tests run on the CPU with `egui_kittest` (no GPU, no snapshots, Story 4.1 D6). Read geometry from `FullOutput::shapes` (walk `Shape::Vec`), accessibility from the AccessKit tree. Run at fractional ppp (1.25, 1.75) wherever a size is checked: Story 4.1's review found two bugs that only show there.
- Red proofs (Debug Log, each failing for the reason it claims, then reverted): G.10 (drop `widget_info` from `widgets::button` → the test names "button"; give the symbol painter variant a node → the node-count check names it); G.11 (ring offset 1 pt; glow without the spread rule at 11 pt / blur 16; dashes via per-side `dashed_line` (phase restart) or a coarse arc → the chord/length checks; a concave L accepted; stroke floor removed; digit cells from `'0'` only (Nunito's `1` is narrower: the x check fails); ↷ not mirrored; bracket inset 0); G.12 (backdrop click closing via `should_close()`; a second request shown at once → two scrims; no initial focus; width not clamped); G.1 (brackets dropped); G.3 (one new key changed on one side; a glow string without a constant); G.6 (darker amber); G.8 (accesskit removed from `Cargo.toml`); the source scans (a `convex_polygon(` added in `gallery.rs`; `with_accessibility_check(false)` added in a test).
- Test files opt out of the panic lints with a file-level `#![allow(...)]` as today. The C rows of `docs/test-plan.md` keep their meaning; G rows are additive.

### Previous story intelligence (4.1, 4.2)

- 4.1 review lessons that apply directly: a check must be able to fail on its own fields (the smoke verdict); test at fractional ppp (1.25, 1.75); assert exact labels (the `label` vs `note-label` miss); a pure function tested with its own input is a tautology (the ppp test); clipped or scrolled-out content escapes shape checks (the gallery overlap test needs a tall harness); hard-coded design numbers outside `theme` are review findings (gallery LED size).
- 4.1 owner review: gallery texts overlapped (Cormorant "3" ink; ring over label) → sizes from ink bounds; keep that pattern for every new sample.
- 4.1 forward notes: Cormorant old-style figures (countdown, halo sized from `mesh_bounds`); hairlines snapped with `main_screen::hairline` (reuse its idea for 1 pt modal rules if they look blurred at 1.25).
- 4.2: the decision and its propagation; G.10 text; `Role::Image` for a standalone symbol, painter variant without a node; `Role::Dialog` from egui for the modal; the suggested CI guard for eframe's `accesskit` (AC 15). Verified at story creation: `cargo tree --locked --offline -p ocb-app -e normal,features -i eframe --target x86_64-unknown-linux-gnu` prints `├── eframe feature "accesskit"` directly followed by `│   └── ocb-app v0.0.0 (…/app/gui)`, while plain `-e features` also lists the `egui_kittest` `[dev-dependencies]` path; so `-e normal,features` is the form that can go red (the red proof must confirm it).
- Exact pins, `--locked`, workspace lints (no `unwrap`/`expect`/`panic`/indexing in non-test code; every type `Debug`), no `strong()`.

### Git intelligence

Last commits: `7135c23 Mark Story 4.2 done`, `129884b` (4.2 review fixes, docs), `bc968bd` (4.2 decision), `338dfa7 Mark Story 4.1 done`, `feafd40` (4.1 merge). Code under `app/gui/` was last changed by Story 4.1 (`985a06a` gallery overlaps, `7d48352` review fixes, `e56dc0b` shell). Branch `story/4-3-shared-painting-helpers-and-modal-dialog` is checked out. Commit style: one imperative sentence naming the story, e.g. "Add the shared painting helpers, painted symbols and the modal dialog, with the helper gallery and the accessibility tests (Story 4.3)".

### Project Structure Notes

New:
```
app/gui/src/ui/{paint.rs, symbols.rs, widgets.rs, modal.rs}
app/gui/tests/{paint.rs, modal.rs, a11y.rs}
```
Changed: `app/gui/src/ui/{mod.rs, theme.rs, gallery.rs, main_screen.rs}`, `app/gui/src/app.rs`, `app/gui/tests/{gallery.rs, main_screen.rs, design_tokens.rs, contrast.rs}`, `.github/workflows/app.yml`, `app/README.md`, `docs/test-plan.md`, `DESIGN.md` (frontmatter + one sentence). Untouched: `app/core`, `app/sim`, `app/tools/ocb-dev`, `firmware/`, `spikes/`, `app/gui/Cargo.toml` (no new dependency is needed).

### Project Context Rules

No `project-context.md` exists. Rules from memory and the repo: all repo content in English; never edit or commit `_bmad/` or `bmad-*` / `gds-*` skill folders; game screenshots in `ux-designs/*/imports/` are never embedded, linked or committed (the gallery uses tokens and original shapes only); evoke, don't copy (UX-DR64): symbols, brackets and the flourish are original shapes; ask the owner one question at a time.

### Open questions for the owner

None blocks the start: each has a default the dev agent applies (D2, D4, D5) unless you say otherwise.

1. **Backdrop click (D4).** Default: clicking outside a dialog does nothing; only Esc or a button closes it. Say if a click outside should close it like Esc.
2. **A second dialog while one is open (D5).** Default: it waits and opens when the first one closes (no stacking, nothing lost). Alternatives: drop it, or replace the open one.
3. **New `DESIGN.md` frontmatter keys (D2, AC 14).** Default: the modal and bracket numbers from the approved mock and two typography tokens (`dialog-body` 14.5 px / 400, `button-md` 14 px / 700) are added to `DESIGN.md`, no existing value changes. Say if any of them should differ from the mock.
4. **Scope (no answer needed unless you disagree).** The "Don't ask again" checkbox, transport/history icon buttons and the restyling of egui's built-in widgets are left to the stories that first use them (D8).

### References

- `_bmad-output/planning-artifacts/epics.md` › Epic 4 (notes), Story 4.3 (and 4.5–4.9, 5.8, 9.7 for later users); UX-DR1, UX-DR3, UX-DR4, UX-DR7, UX-DR61, UX-DR62, UX-DR64; OQ-9
- `_bmad-output/planning-artifacts/ux-designs/ux-bonooru-2026-11-20/DESIGN.md` › frontmatter (`spacing.corner-bracket-size`, `focus-ring-*`, `components.*.glow`, `modal-dialog`, `corner-bracket`, `focus-ring`, `button-disabled`); Typography (egui notes: digit cells, painted symbols); Elevation & Depth (glow in egui); Shapes (corner brackets, egui shape rules); Components › Focus ring, Modal dialog, Checkbox
- `_bmad-output/planning-artifacts/ux-designs/ux-bonooru-2026-11-20/EXPERIENCE.md` › Other surfaces (modal depth one), Component Patterns (Readiness confirm, Firmware update needed), Interaction Primitives (Esc, Enter, Space, banned stacked modals), Accessibility Floor, Microcopy
- `_bmad-output/planning-artifacts/ux-designs/ux-bonooru-2026-11-20/mockups/key-dialogs.html` (`.modal`, `.flourish`, `.checklist`, `.diamond`, `.btn`, `.corner`, transport and bottom-bar SVGs), `key-editor-staff.html`, `directions-oot-2026-11-20.html` (key chips, lane glyphs)
- `_bmad-output/planning-artifacts/ux-designs/ux-bonooru-2026-11-20/review-egui-feasibility.md` › Key facts; rows 4, 5, 7, 9, 10, 14, 24, 25, 27, 32, 36, 52
- `_bmad-output/implementation-artifacts/4-1-app-shell-with-theme-fonts-and-main-screen-regions.md` › AC 3, D6, Review Findings, owner review, forward notes; `4-2-decide-the-screen-reader-accesskit-level.md` › Facts, Custom widgets table, Debug Log (CI guard suggestion)
- `_bmad-output/implementation-artifacts/epic-3-retro-2026-11-26.md` › §5 actions 1 and 2 (E3-A1, E3-A2)
- `docs/test-plan.md` › App GUI tests (G.1–G.10)
- Crate sources: `epaint-0.37.0/src/{shadow.rs, tessellator.rs (2085–2110), shapes/shape.rs (150–290), shapes/path_shape.rs, shapes/bezier_shape.rs (351), shapes/rect_shape.rs (140–152)}`; `egui-0.37.0/src/{containers/modal.rs, memory/mod.rs (840–860, 960–980), context.rs (1750–1770, 4203–4220), response.rs (951, 1065, 1087), ui_stack.rs (64–80), painter.rs (189)}`; `egui_kittest-0.37.0/src/{builder.rs, node.rs, accessibility.rs, lib.rs}`; `kittest-0.5.0/src/query.rs`
- `ctx7` `/emilk/egui/0.37.0`: `docs/accessibility.md` (Custom widgets, Testing), `crates/egui_kittest/README.md`

## Dev Agent Record

### Agent Model Used

Claude Opus 5.5 (`claude-opus-5-5`), gds-dev-story, 2026-11-27.

### Debug Log References

**Open questions 1–4: default applied, owner may override** (the owner has been told):
1. Backdrop click: does nothing; only Esc or a button closes a dialog (D4). `ModalResponse::should_close()` is not used.
2. A second dialog while one is open: queued FIFO, deduplicated by id, opens from the frame after the current one closes (D5).
3. New `DESIGN.md` frontmatter keys: added from `mockups/key-dialogs.html`, no existing value changed (D2). See the additions below.
4. Scope: no "Don't ask again" checkbox, no transport/history icon buttons, no restyling of egui's built-in widgets (D8; comment in `theme::visuals()`).

**Decisions and deviations taken during development (reason given):**
- AC 14 keys: besides the listed ones, four more mock numbers the layout needs went into `components.modal-dialog` (and `theme::components::modal_dialog`): `flourish-margin-top: 6px`, `flourish-margin-bottom: 10px` (`.flourish` margin), `checklist-margin-bottom: 14px` (`.checklist` margin), `text-margin-bottom: 16px` (`.mtext` margin). Otherwise `ui::modal` would have written them as literals. `components.button-disabled.opacity` (already in `DESIGN.md`) got its constant `BUTTON_DISABLED_OPACITY`. G.3 compares all of them; the modal group both ways (every px key vs `modal_dialog_tokens()`). `DESIGN.md` `updated:` was already 2026-11-27 (today), so it stays.
- AC 1 clip rect: the ring's painter is clipped to the ring's outer rect grown by 1 pt (anti-aliasing fringe; at ppp 1.25 a 2 pt stroke ends mid-pixel and a scissor exactly at the outer edge would cut half the feather). Still its own clip, never the widget's (D7). G.11 checks clip ⊇ `rect.expand(4)` with the widget inside a scope clipped to its own rect.
- AC 2: the glow shape is built with `round_to_pixels(false)`: the tessellator rounds rects to pixels before the blur clamp, which could shrink the shadow rect below `blur + 0.1` at fractional scales.
- AC 7: `paint::digit_cell_width(ctx, style)` takes the typography token rather than `(fonts, font_id)`, so the `wght` variation of the token is measured too. Whitespace in `paint_digits` only advances (no empty text shape).
- Finding (AC 7): Nunito Sans's bundled digits are already tabular (all ten 8.094 pt in `body`, 7.5 pt in `small` and `button-lg-sub`, measured with a probe test); the story's note that they are proportional does not hold for these files. Cells are still applied (no cost, and the countdown/display face Cormorant is proportional: `1` 42.5 pt vs `0` 61.1 pt at 128 pt). Because a Nunito-only test cannot go red, `g11_digit_cells` also runs `card-title` (Cormorant 24).
- AC 10 disabled look: inside `ui.add_enabled_ui(false)` egui already fades the painter by `visuals.disabled_alpha()` (measured alpha 58 instead of 115); `widgets::button` undoes that factor so the token's 45 % is the only fade. The disabled button is still `ui.add_enabled_ui` semantics (not clickable, not focusable, AccessKit `disabled`).
- AC 11 Tab: no focus guard needed. egui gives keyboard focus only to widgets on the top modal layer or above (`Memory::interested_in_focus` returns early unless `allows_interaction(layer)`); `g12_esc_backdrop_tab` confirms 5 × Tab and 5 × Shift+Tab stay on the dialog buttons.
- AC 12: the dialog node is named with `Area::accessible_name(title)` on `Modal::default_area(id)` (egui 0.37 API, `containers/area.rs`), so no `accesskit_node_builder` call is needed. `Role::Label` stores its name in AccessKit `value`; kittest's label queries read it there.
- AC 13 / G.3 ring test: egui advertises the AccessKit Focus action on a disabled widget too (`fill_accesskit_node_common` checks only the sense), but never focuses it; the test iterates enabled nodes only. Gallery harness height raised from 4000 to 7000 pt; the overlap test now asserts the last text ("Disabled sample") is painted above the bottom.
- Gallery focus samples: the themed button cannot paint the `amber-ink` ring (its signature has no `on_parchment`), so the parchment sample is a gallery-local focusable control that calls `paint_focus_ring(&resp, radius, true)` and reports `Role::Button`. The two hand-drawn `amber-ink` ring samples of Story 4.1 stay (G.3 `g3_amber_ink_ring_clears_its_label` unchanged).
- `ModalSpec::new` uses `Id::unique` (`Id::new` is deprecated in egui 0.37).
- egui/epaint API facts: `ctx7` (`/emilk/egui/0.37.0`, accessibility docs: custom widget `widget_info`, kittest accessibility check) plus the 0.37.0 sources in `~/.cargo/registry` (`containers/modal.rs`, `containers/area.rs` `accessible_name`, `memory/mod.rs` focus and modal layer, `context.rs` Enter/Space click, `response.rs` `widget_info`, `epaint` `shadow.rs`, `tessellator.rs` blur clamp, `shapes/shape.rs` `dashes_from_line`, `kittest` `filter.rs`).

**Red proofs (E3-A2): each check failed for the reason it claims, then the change was reverted** (logs in the session scratchpad):
- G.3 tokens: `ROW_GAP` 13 in theme → `components.modal-dialog.row-gap: DESIGN.md 12, theme 13`; `BUTTON_MD` weight 600 → `typography.button-md.fontWeight: DESIGN.md 700, theme 600`; `("modal-dialog", MODAL_PRIMARY)` removed from `glow_tokens()` → `glow.modal-dialog: in DESIGN.md, missing in theme`. `g3_glow_self_test` covers a changed blur, a constant without a string and an unlisted non-blur glow string.
- G.6: `amber` = `#5a4010` → `ink on amber 1.65:1 < 4.5:1`, `amber on panel 1.87:1 < 3:1`, `amber on panel-raised 1.78:1`, `amber on well 1.68:1`.
- G.10: `widget_info` removed from `widgets::button` → `g10_buttons_are_named_buttons`: `No nodes found matching the query: By { label: "Play", role: Button }`; the symbol painter given an AccessKit node → `` `symbol painter` added a node (6 vs 5) ``; a `with_accessibility_check(false)` call added to `tests/window.rs` → the scan names `tests/window.rs`.
- G.11: ring offset 1 pt → `ring [[7 37]-[89 63]] vs [[6 36]-[90 64]]`; glow spread forced to 0 → `side 4 blur 8: 4 <= 8.1`; arcs flattened at 2 pt chord error → `radius 4: [163.1 42.1] is 1.17 off`; dash phase restarted at each sharp corner (per side) → `radius 0 5/3: dashes 210, want 202.5 ± 5`; `is_convex` accepting a 6-point polygon → `L` accepted; stroke floor removed → `g11_stroke_floor` (`ppp 1: 0.5 pt gives 0.5`) and `g11_helper_strokes_floored` fail; ↷ not mirrored (identity) → `Undo and Redo paint the same at 10`; ↷ mirrored on the wrong axis → `redo [43.4 47.6] is not undo [43.4 42.4] mirrored`; bracket inset 0 → `corner 0: (0, 1, 1) != (1, 1, 1)`; digit cell from `'0'` only → `card-title: cell 11.44 vs widest digit 11.75`; from `'1'` only → `cell 7.97 vs 11.75`; no cells (own advance) → `` `0` at 55.72, `8` at 55.88 ``. Before `card-title` was added, the last two proofs stayed green (Nunito tabular, see above): that is why it was added.
- G.11 source scan: `egui::Shape::convex_polygon(...)` added to `gallery.rs` → `gallery.rs builds a filled path; use paint::fill_convex`.
- G.12: backdrop closing through `should_close()` → `backdrop click closed the dialog`; the queued dialog also shown in the same frame → `two scrims in one frame`; `request_focus` removed → `primary focused on open`; width not clamped → `1024x680 ppp 1 width 400: 400`; Esc not consumed → `left: [] right: [(…gallery-readiness, Dismissed)]`; a focusable button in an `Order::Tooltip` area above the dialog → `focus left the dialog: ["Above"]` (the Tab check can fail; moving the dialog's own area to `Order::Background` did not break the trap, egui still treated it as the modal layer).
- G.1: brackets painted in `TRANSPARENT` instead of `bracket` → `1024x680 ppp 1: corner 0: (0, 0, 0) != (1, 1, 1)`.
- G.3 gallery: `paint_focus_ring` removed from the parchment sample → `` `Sample on parchment` shows no focus ring ``; gallery harness back to 4000 pt → `ppp 1: the last section is not painted`.
- G.8 accesskit guard: `"accesskit"` removed from `app/gui/Cargo.toml` → `gui/ci/check-accesskit.sh` exits 1 with `::error::eframe's accesskit feature is not enabled by ocb-app for <t>` for each of the three targets, while `cargo test -p ocb-app --test a11y --test modal` still passes (17 tests); plain `-e features` still lists `eframe feature "accesskit"` (via `egui_kittest feature "eframe"`), confirming `-e normal,features` is the form that can go red. `Cargo.toml` and `Cargo.lock` restored from a copy (no diff). The script's error path was also seen with `cargo` off `PATH` (`::error::cargo tree failed for <t>`, exit 1).

**Verification (local):** see Completion Notes.

### Completion Notes List

- Story created 2026-11-27 by gds-create-story: ultimate context engine analysis completed, comprehensive developer guide created.
- Tasks 1–10 implemented and tested. New modules `ui::paint`, `ui::symbols`, `ui::widgets`, `ui::modal`; `App` owns `Modals` (`modals_mut()`), draws dialogs after the screen; the note-view card shows its brackets; the gallery has the helper sections and two sample dialogs; new tests `tests/paint.rs` (G.11, 13), `tests/modal.rs` (G.12, 9), `tests/a11y.rs` (G.10, 8), shared `tests/common/mod.rs`; G.1, G.3, G.6 extended; CI step and `gui/ci/check-accesskit.sh` (G.8); docs.
- Task 11, local part done (macOS, Rust 1.100.0): `cargo fmt --all --check` ok; `cargo clippy --locked --workspace --all-targets -- -D warnings` ok; `cargo test --locked --workspace` 221 passed, 0 failed; `cargo deny --locked check licenses bans` → `bans ok, licenses ok`; `gui/ci/check-accesskit.sh` ok for the three targets; `bonooru --smoke-test` and `--smoke-test --gallery` → `OCB_SMOKE ok frames=3 ppp=2 inner=1280x800 min=1024x680 theme=dark backend=AppKit`, rc 0, `check-smoke.sh` ok. Linux reproduced in Docker (`ubuntu:24.04`, aarch64, the CI apt list plus `curl ca-certificates build-essential git xauth python3`, Rust 1.100.0 from `rust-toolchain.toml`, as in Story 4.1): `cargo build --locked -p ocb-app` rc 0; `WAYLAND_DISPLAY=ocb-no-such-socket xvfb-run -a target/debug/bonooru --smoke-test` and `--smoke-test --gallery` → `OCB_SMOKE ok frames=3 ppp=1 inner=1280x800 min=1024x680 theme=dark backend=Xlib`, rc 0, checker rc 0; `check-accesskit.sh` ok for the three targets. Task 11 stays open: the `app` workflow on the three OS runs after the owner pushes the branch.
- Task 12 (owner eye check, AC 17) open: the gallery builds and runs (`cargo run -p ocb-app -- --gallery` from `app/`); the owner answers the six items of Dev Notes › Owner eye check. Story stays `in-progress` until then.

### File List

- `_bmad-output/planning-artifacts/ux-designs/ux-bonooru-2026-11-20/DESIGN.md` (modified: typography `dialog-body`, `button-md`; `components.modal-dialog` sizes and glow; `components.corner-bracket.inset`; one sentence in Modal dialog)
- `_bmad-output/implementation-artifacts/4-3-shared-painting-helpers-and-modal-dialog.md` (story record)
- `_bmad-output/implementation-artifacts/sprint-status.yaml` (4-3 in-progress)
- `app/gui/src/ui/paint.rs` (new)
- `app/gui/src/ui/symbols.rs` (new)
- `app/gui/src/ui/widgets.rs` (new)
- `app/gui/src/ui/modal.rs` (new)
- `app/gui/src/ui/mod.rs` (modified)
- `app/gui/src/ui/theme.rs` (modified)
- `app/gui/src/ui/gallery.rs` (modified)
- `app/gui/src/ui/main_screen.rs` (modified)
- `app/gui/src/app.rs` (modified)
- `app/gui/tests/common/mod.rs` (new)
- `app/gui/tests/paint.rs` (new)
- `app/gui/tests/modal.rs` (new)
- `app/gui/tests/a11y.rs` (new)
- `app/gui/tests/gallery.rs` (modified)
- `app/gui/tests/main_screen.rs` (modified)
- `app/gui/tests/design_tokens.rs` (modified)
- `app/gui/tests/contrast.rs` (modified)
- `app/gui/ci/check-accesskit.sh` (new)
- `.github/workflows/app.yml` (modified)
- `app/README.md` (modified)
- `docs/test-plan.md` (modified)

### Change Log

- 2026-11-27: Implemented Tasks 1–10 (shared painting helpers, painted symbols, themed widgets, modal dialog and host, note-card brackets, helper gallery, G.10–G.12 and G.1/G.3/G.6/G.8 extensions, docs); local verification of Task 11 done; CI on three OS and the owner eye check (Task 12) open.
