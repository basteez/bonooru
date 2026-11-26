---
baseline_commit: 004bda3
---

# Story 4.1: App shell with theme, fonts and main-screen regions

Status: in-progress

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As the owner,
I want the app window to open on all three OS with the Hybrid look and the main-screen regions in place,
so that every later feature lands in a consistent, themed layout.

Origin: `epics.md` Epic 4, Story 4.1 (UX-DR1, UX-DR2, UX-DR5; AD-18, AD-21). First story of Epic 4 after the cleanup Story 4.0. It builds on the Rust workspace of Stories 3.2–3.5 and on the egui spike (`spikes/app-egui/SPIKE-REPORT.md`).

**Owner constraint: host-side only.** No bench, no Switch, no board. Verification is `cargo test` (unit + `egui_kittest` UI tests, no GPU), the CI smoke run of the real window on ubuntu, windows and macos, `cargo deny` and `cargo tree` checks. The one human step is the owner's look at the token gallery for UX Open Question 7 (AC 11). No pixel snapshot tests: the plan only asks for a smoke test (`app-stack-options.md` › Test strategy, "GUI: starts headless in CI and exits").

## Acceptance Criteria

1. **Crate and pins.** A new workspace member, package `ocb-app` in `app/gui/` (library `ocb_app` + binary `bonooru`), depends on `ocb-core` by path and on `eframe` and `egui` pinned at exactly `=0.37.0`. `ocb-core` is not changed and stays GUI-free: its `[dependencies]` stay empty and C.4 (`app/core/tests/purity.rs`) still passes.
2. **Window.** The app opens one window titled "Bonooru", initial inner size 1280 × 800 logical points, minimum inner size 1024 × 680 logical points (UX-DR5). It is dark only: the theme preference is forced to dark whatever the OS says, and the egui visuals come from the theme tokens. HiDPI works through egui's `pixels_per_point` (all sizes are points); egui's keyboard zoom (Cmd/Ctrl + `+`/`-`/`0`) is turned off so the window can never be scaled below its minimum in points.
3. **Theme module, single source (UX-DR1).** `ocb_app::ui::theme` holds every token of the `DESIGN.md` frontmatter sections `colors` (8-digit hexes as `Color32::from_rgba_unmultiplied`), `rounded`, `spacing` (scale and named sizes) and `typography` (family, size, weight, line height, letter spacing; `code` = Hack 13 px). No other module writes a colour hex, a token size or a font size. A test reads the `DESIGN.md` frontmatter and fails when a token is missing on either side or differs in value.
4. **Fonts (UX-DR2).** The variable TTFs of Cormorant Garamond (`wght` 300–700) and Nunito Sans (`wght` 200–1000) are bundled unmodified; Hack comes from egui's `default_fonts`. Display styles (`brand`, `dialog-title`, `panel-tab`, `card-title`, `countdown`) use Cormorant Garamond, `code` uses Hack, everything else Nunito Sans. Weights come from the `wght` axis: text with no explicit weight renders at 400 (Nunito) or 700 (Cormorant display), never at the files' default minimum, and no code uses `RichText::strong()` as bold. Labels are uppercase with 0.06 em extra letter spacing (0.06 × 11.5 = 0.69 pt for `label`). The `system_fonts` feature is off, so no OS font is ever loaded.
5. **Font licences in the repo.** The licence texts of every font that ships in the binary are stored under `app/gui/assets/licenses/` for the later About view and the release notices (AD-21): OFL-1.1 for Cormorant Garamond and for Nunito Sans (each with its copyright line), Hack's licence, the Ubuntu Font Licence 1.0 (Ubuntu-Light, an egui default font) and the MIT licence of `emoji-icon-font` (egui's `egui-icons` subset). A test checks each file exists and names its font.
6. **Main-screen regions (UX-DR5).** A pure function computes the region rects from the content rect: status strip 44 pt high on top, bottom bar 88 pt high at the bottom, song panel 260 pt wide on the left, gamepad panel 300 pt wide on the right, note view = all remaining width and height. The regions are filled in reading order (status → song → note view → gamepad → bottom bar) with `ui.scope_builder(UiBuilder::new().max_rect(r))`, never egui `Panel`s. Each region shows only its fill (status strip and bottom bar `panel-raised`, song and gamepad panels `panel`, frame `frame`); the note view is the parchment card, inset `card-inset` (6 pt) in `frame`, corner radius `rounded.xl` (12 pt). Content is empty placeholder. The rects in points are the same at `pixels_per_point` 1.0, 1.5 and 2.0.
7. **Linux: X11/XWayland only (AD-18).** eframe is built without its `wayland` feature. Because eframe's `accesskit` feature turns on `winit`'s `wayland` feature through `accesskit_winit`'s default features (found at story creation, see Dev Notes), the app also forces winit's X11 backend on Linux with `EventLoopBuilderExtX11::with_x11()` in `NativeOptions::event_loop_builder`. Under a session with `WAYLAND_DISPLAY` set, the app still opens on X11.
8. **Headless smoke test in CI (three OS).** `bonooru --smoke-test` opens the real window, renders at least 3 frames, closes itself and exits 0, printing one line `OCB_SMOKE ok frames=<n> ppp=<f> inner=<w>x<h> min=1024x680 theme=dark backend=<raw window handle kind>`. A watchdog makes it exit non-zero if it has not closed after 60 s. The `app` workflow runs it on ubuntu (under `xvfb-run`, with a bogus `WAYLAND_DISPLAY`, and checks `backend=Xlib` or `backend=Xcb`), windows and macos.
9. **No network crates, licence gate (AD-21).** `app/deny.toml` gains a `[bans]` section that denies HTTP/network client and TLS crates (at least `reqwest`, `ureq`, `hyper`, `h2`, `ehttp`, `isahc`, `curl`, `attohttpc`, `minreq`, `surf`, `tungstenite`, `rustls`, `native-tls`, `openssl`), and CI runs `cargo deny --locked check licenses bans`. The licence allow-list gains exactly `OFL-1.1` and `Ubuntu-font-1.0` (decided by NFR-07 as amended 2026-11-19, AD-18) and `BSL-1.0` (owner decision 2026-11-26: `clipboard-win` and `error-code`, Windows only, through eframe's `clipboard` feature; found by the gate at dev time) and nothing else. A red proof (a banned crate added on purpose fails `check bans`) is in the Debug Log.
10. **Everything green.** On ubuntu, windows and macos: `cargo fmt --all --check`, `cargo test --locked --workspace` (C.1 to C.20 unchanged, plus the new G rows), `cargo clippy --locked --workspace --all-targets -- -D warnings`, the deny gate, the feature check of AC 7 and the smoke run of AC 8.
11. **UX Open Question 7, owner review.** `bonooru --gallery` shows a token gallery: every colour token as a named swatch with its hex, the four status colours as text and swatches on `panel-raised` (and `status-error` on `panel` and `well`), the `amber-ink` 2 pt ring on `parchment` and on `parchment-header`, the `scrim` over a sample of panel + parchment, a text-score sample in Hack 13 pt on `well`, and one specimen line per typography token. The owner looks at it in the running app; each proposed value (`status-warning`, `status-error`, `status-unknown`, `amber-ink`, `scrim`, editor font Hack 13 px) is confirmed or changed in `DESIGN.md` and in the theme module (AC 3's test keeps the two equal), the `[ASSUMPTION]` tags of confirmed values are removed, and the outcome is recorded in `EXPERIENCE.md` (Open Questions, item 6) and `epics.md` (OQ-7). Any changed colour still meets the contrast floors of G.6.

### AC → measurement (retro action E3-A1)

Every number in the ACs and the tool and field that measure it. All host-side; no number needs the owner except the subjective "looks right" of AC 11.

| AC | Number | Measured by | Field / result |
|---|---|---|---|
| 1 | eframe / egui `=0.37.0` | `app/gui/Cargo.toml` + CI step `cargo tree --locked -p ocb-app -i eframe --depth 0` and `-i egui --depth 0` | prints `eframe v0.37.0`, `egui v0.37.0` |
| 2 | initial 1280 × 800, min 1024 × 680 | unit test on `native_options()` (G.2); smoke line (G.7) | `viewport.inner_size == Some(1280×800)`, `viewport.min_inner_size == Some(1024×680)`; `min=1024x680`, `inner=` ≥ 1024×680 |
| 2 | dark only | kittest built with `with_theme(egui::Theme::Light)` (G.2); smoke line | `ctx.theme() == Theme::Dark`, `ctx.style().visuals.panel_fill == theme::FRAME`; `theme=dark` |
| 2 | zoom off | unit test after `ocb_app::setup(ctx)` (G.2) | `ctx.options(|o| o.zoom_with_keyboard) == false`, `ctx.zoom_factor() == 1.0` |
| 3 | every token value | `app/gui/tests/design_tokens.rs` (G.3) | each `DESIGN.md` frontmatter key in `colors`, `rounded`, `spacing`, `typography` has an equal theme value, and each theme token has a `DESIGN.md` key |
| 4 | `wght` 400 / 600 / 700, axis ranges 300–700 and 200–1000 | `app/gui/tests/fonts.rs` (G.4): `FontData::variation_axes()` on the bundled bytes; galley widths through `ctx.fonts_mut(...).layout_no_wrap` | `wght` axis min/max as stated; width(no weight) == width(400) > width(200 raw, no tweak); width(700) > width(600) > width(400) for Nunito; display text with no weight == width(700) |
| 4 | 0.06 em → 0.69 pt for `label` | `fonts.rs` (G.4) | `theme::label("abc")` text is `"ABC"`; its galley is wider than the same text without spacing by 0.69 × n ± 0.01 pt (n = glyph count or n − 1, whichever egui applies; assert the one measured and say which in a comment) |
| 4 | no `.strong()` | `fonts.rs` source scan (G.4) | no `.strong(` in `app/gui/src/` |
| 5 | licence files | `app/gui/tests/licences.rs` (G.5) | five files exist, non-empty, each contains its font name ("Cormorant", "Nunito Sans", "Hack", "UBUNTU FONT LICENCE", "emoji-icon-font" or its MIT copyright) |
| 6 | 44 / 88 / 260 / 300 pt, 6 pt inset, 12 pt radius | `layout` unit tests (G.1) at content rects 1024×680, 1280×800, 1920×1080, 2560×1440 | each rect's height / width equals the token; note view width = W − 560, height = H − 132; the card = note view shrunk by 6 on each side; rects tile the content rect without overlap or gap |
| 6 | reading order, fills | kittest `app/gui/tests/main_screen.rs` (G.1) at 1024×680 and 1280×800 | `Regions::in_reading_order()` returns status, song, note view, gamepad, bottom; the frame's `output().shapes` contain a filled rect of the right colour for each region (status/bottom `PANEL_RAISED`, song/gamepad `PANEL`, card `PARCHMENT` with radius 12) |
| 6 | ppp 1.0 / 1.5 / 2.0 | `main_screen.rs` with `Harness::builder().with_pixels_per_point(p)` (G.1) | the computed region rects are identical in points for all three |
| 7 | eframe `wayland` feature off | CI step (G.8) `cargo tree --locked -p ocb-app -e features -i eframe --target x86_64-unknown-linux-gnu` | output has no line `eframe feature "wayland"` |
| 7 | X11 forced | smoke on ubuntu with `WAYLAND_DISPLAY=ocb-no-such-socket` (G.7) | `backend=Xlib` or `backend=Xcb`, exit 0 (winit 0.30.13 picks Wayland whenever `WAYLAND_DISPLAY` is set and has no fallback, so without the hook the run fails: red proof) |
| 8 | ≥ 3 frames, exit 0, 60 s watchdog | smoke run in CI, three OS (G.7); `smoke` unit test of the frame counter and the arg parser | the `OCB_SMOKE ok` line with `frames=` ≥ 3 and exit code 0; watchdog exit code 3 proven by a unit test of the timeout path (inject a 0 s timeout) |
| 9 | banned crates; two new licences | `cargo deny --locked check licenses bans` in CI (G.9); red proof in the Debug Log | rc 0 on the real graph; rc ≠ 0 with a banned crate added to `ocb-app` (then removed) |
| 10 | three OS | the `app` workflow matrix | all steps green on `ubuntu-24.04`, `windows-latest`, `macos-latest` |
| 11 | contrast floors | `app/gui/tests/contrast.rs` (G.6), WCAG 2.x relative luminance | `text`, `text-muted`, `status-*` on `panel-raised` ≥ 4.5:1; `status-error` on `panel` and `well` ≥ 4.5:1; `amber-ink` on `parchment` and `parchment-header` ≥ 3:1; `ink` on `parchment` ≥ 4.5:1; `out-of-range-ink` on `out-of-range-zone` ≥ 4.5:1 |

## Tasks / Subtasks

- [x] **Task 1: Crate skeleton and pins (AC: 1, 2)**
  - [x] 1.1 `app/gui/Cargo.toml`: package `ocb-app`, workspace package fields and `[lints] workspace = true`; `[lib] name = "ocb_app"`; `[[bin]] name = "bonooru"` (`src/main.rs`); dependencies `ocb-core = { path = "../core" }`, `eframe = { version = "=0.37.0", default-features = false, features = ["accesskit", "clipboard", "default_fonts", "wgpu", "x11"] }`, `egui = "=0.37.0"`, `raw-window-handle = "=0.6.2"`; Linux only: `[target.'cfg(target_os = "linux")'.dependencies] winit = { version = "=0.30.13", default-features = false, features = ["x11"] }`. Dev: `egui_kittest = { version = "=0.37.0", features = ["eframe"] }` (no `wgpu`, no `snapshot`).
  - [x] 1.2 Add `"gui"` to `members` in `app/Cargo.toml`; update its header comment. Run `cargo update -p` only as needed and commit the new `Cargo.lock` (CI uses `--locked`).
  - [x] 1.3 `src/lib.rs`: modules `ui` (`theme`, `fonts`, `layout`, `main_screen`, `gallery`), `app` (the `eframe::App`), `cli` (args), `smoke`; `pub fn native_options() -> eframe::NativeOptions` and `pub fn setup(ctx: &egui::Context)` (fonts, theme, dark, zoom off). `src/main.rs`: parse args, `eframe::run_native("Bonooru", native_options(), ...)`, map errors to exit codes (0 ok, 1 eframe error printed to stderr, 2 usage, 3 smoke timeout). No `unwrap`/`expect`/`panic` (workspace lints).
- [x] **Task 2: Theme module (AC: 3)**
  - [x] 2.1 `ui/theme.rs`: one `pub const` per colour token (`FRAME`, `PANEL`, ..., `SCRIM`) built with `Color32::from_rgb` / `from_rgba_unmultiplied`; `Rounded` (`XS`..`XL`, `FULL` = 9999.0 kept as the token, clamp to 255 when building a `CornerRadius` and say so), `Spacing` scale `S1`..`S6` and every named size (`STATUS_STRIP_HEIGHT`, `BOTTOM_BAR_HEIGHT`, `SONG_PANEL_WIDTH`, `GAMEPAD_PANEL_WIDTH`, `GAMEPAD_COMPACT_WIDTH`, `SONG_RAIL_WIDTH`, `CARD_INSET`, ... `FOCUS_RING_OFFSET`); a `TypeStyle { family, size, weight, line_height: Option<f32>, letter_spacing_em: f32 }` const per typography token (`BRAND` ... `KEY_CHIP`, `CODE`).
  - [x] 2.2 A token table for the test: `pub fn color_tokens() -> &'static [(&'static str, Color32)]` (DESIGN names, e.g. `"panel-raised"`), and the same for rounded, spacing and typography. These tables are the only place names map to values.
  - [x] 2.3 Helpers: `fn rich(style: &TypeStyle, text) -> RichText` (font id, size, `variation("wght", weight)`, `extra_letter_spacing(em × size)`, line height), `fn label(text) -> RichText` (uppercase + `LABEL`), `fn visuals() -> egui::Visuals` from `Visuals::dark()` with `panel_fill = FRAME`, `window_fill = PANEL_RAISED`, `window_stroke = 1 pt RIM`, `extreme_bg_color = WELL`, `faint_bg_color = PANEL`, text colour `TEXT`, selection `AMBER_WASH`/`AMBER`, hyperlink `AMBER`, `window_corner_radius = LG`. Widget restyling (focus ring, buttons) is Story 4.3; keep this minimal.
  - [x] 2.4 `tests/design_tokens.rs` (G.3): read `../../_bmad-output/planning-artifacts/ux-designs/ux-bonooru-2026-11-20/DESIGN.md` (path from `CARGO_MANIFEST_DIR`), take the frontmatter between the first two `---`, parse the four sections with a small line parser (two-space keys, `key: 'value'` or `key: value`, comments after `#` stripped; no YAML crate), and compare both ways. `typography.code` is a note (no fields): map it to Hack 13 px by a named exception in the test. Red proof: change one hex in the theme, the test fails naming the token (Debug Log).
- [x] **Task 3: Fonts (AC: 4, 5)**
  - [x] 3.1 Put the two TTFs, unmodified, in `app/gui/assets/fonts/` as `CormorantGaramond-VF.ttf` and `NunitoSans-VF.ttf`, from `google/fonts` commit `5026800714d07231937d92eb4cceabf06e05d74e` (`ofl/cormorantgaramond/CormorantGaramond[wght].ttf`, 1 195 560 B, sha256 `b20b7d9626dd956b2c5e558692ad328b1f19e3275e2782db4fa07670d83f35e0`; `ofl/nunitosans/NunitoSans[YTLC,opsz,wdth,wght].ttf`, 571 240 B, sha256 `f934d7142fb4784bf828da485b7dcbd90c0c80d514e9d49a5da0ed3a1ae2491d`). Verify the sha256 after download and record source, commit and sha256 in `app/README.md`. No italics, no subsetting (OFL: unmodified, so no renaming duty).
  - [x] 3.2 `ui/fonts.rs`: `pub fn definitions() -> egui::FontDefinitions` starting from `FontDefinitions::default()` (keeps `Hack`, `Ubuntu-Light`, `egui-icons`), inserting `"NunitoSans"` with `FontTweak { coords: VariationCoords::new([("wght", 400.0)]), ..Default::default() }` and `"CormorantGaramond"` with `wght` 700 (both files default to their axis minimum, 200 and 300: without the tweak all text is ExtraLight). Families: `Proportional = [NunitoSans, Ubuntu-Light, egui-icons]`, `FontFamily::Name("display") = [CormorantGaramond, NunitoSans, Ubuntu-Light, egui-icons]`, `Monospace = [Hack, Ubuntu-Light, egui-icons]` (Hack first, as egui's default). Bytes via `include_bytes!` and `FontData::from_static`.
  - [x] 3.3 `setup()` installs the definitions with `ctx.set_fonts(...)` and sets `Style::text_styles`: `Body` = body (13.5), `Small` = small (12.5), `Button` = body (13.5), `Heading` = dialog-title in `display` (21), `Monospace` = code (Hack 13).
  - [x] 3.4 `tests/fonts.rs` (G.4): axis ranges from `FontData::variation_axes()`; the galley-width checks of the AC → measurement table (in a kittest harness or a bare `egui::Context` after one `run`); `label()` uppercase + spacing; the source scan for `.strong(`. Red proof: drop the `wght` 400 tweak, the "no weight == 400" check fails (Debug Log).
  - [x] 3.5 `app/gui/assets/licenses/`: `OFL-1.1-CormorantGaramond.txt` and `OFL-1.1-NunitoSans.txt` (the `OFL.txt` of each font directory at the same google/fonts commit, which carry the copyright lines), `Hack.txt`, `UFL-1.0-Ubuntu.txt`, `MIT-emoji-icon-font.txt` (copied from `epaint_default_fonts-0.37.0/fonts/`: `Hack-Regular.txt`, `UFL.txt`, `emoji-icon-font-mit-license.txt`). `tests/licences.rs` (G.5). Do not copy `epaint_default_fonts`' `OFL.txt`: it covers Noto Emoji, which only ships with the `monochrome_emoji_fonts` feature (off).
- [x] **Task 4: Window, dark theme, regions (AC: 2, 6)**
  - [x] 4.1 `native_options()`: `ViewportBuilder::default().with_title("Bonooru").with_app_id("bonooru").with_inner_size([1280.0, 800.0]).with_min_inner_size([1024.0, 680.0])`; renderer left at the default (wgpu, as the spike); on Linux `event_loop_builder: Some(Box::new(|b| { b.with_x11(); }))` (Task 5). No `persistence` feature, no `persist_window`.
  - [x] 4.2 `setup()`: `ctx.set_theme(egui::ThemePreference::Dark)`, `ctx.set_visuals_of(Theme::Dark, theme::visuals())` (or `set_style_of`), `ctx.options_mut(|o| o.zoom_with_keyboard = false)`, fonts (3.3). Called once from the `eframe::App` constructor (`CreationContext::egui_ctx`).
  - [x] 4.3 `ui/layout.rs`: `pub struct Regions { status, song, note_view, note_card, gamepad, bottom }` and `pub fn regions(content: Rect) -> Regions` (pure, no egui context), `pub fn in_reading_order(&self) -> [(RegionId, Rect); 5]`. Gamepad is always 300 here: the < 1280 compact variant is Story 4.9, the song rail Story 5.12; leave a one-line comment, not a parameter.
  - [x] 4.4 `ui/main_screen.rs`: `pub fn show(ui: &mut Ui)`: paint the frame fill over `ui.max_rect()`, then for each region in reading order `ui.scope_builder(UiBuilder::new().max_rect(r).id_salt(region_id), |ui| paint_region(ui, id))`: fill rect; status strip also its 1 pt `rim-soft` bottom hairline (`components.status-strip`); the note view paints the parchment card (`note_card`, radius `XL`). No widgets, no text: placeholders stay empty (corner brackets are Story 4.3).
  - [x] 4.5 `app.rs`: `eframe::App::ui(&mut self, ui, frame)` (0.37 API: not `update`) calls `main_screen::show(ui)` or `gallery::show(ui)`; `App::clear_color` returns `FRAME` so no default grey flashes at start.
  - [x] 4.6 `tests/main_screen.rs` and `layout` unit tests (G.1), `tests/window.rs` (G.2), per the AC → measurement table. Use `Harness::builder().with_size(..).with_pixels_per_point(..).with_theme(Theme::Light).build_eframe(|cc| App::new(cc, Mode::Main))`.
- [x] **Task 5: Linux X11 and the feature check (AC: 7)**
  - [x] 5.1 The `with_x11()` hook in `native_options()` behind `#[cfg(target_os = "linux")]`, with a comment naming the reason (`accesskit_winit` 0.34 default features enable `winit/wayland`; winit 0.30.13 picks Wayland whenever `WAYLAND_DISPLAY` is set, `platform_impl/linux/mod.rs`) and AD-18 (file drops need X11).
  - [x] 5.2 CI step (ubuntu) `cargo tree --locked -p ocb-app -e features -i eframe --target x86_64-unknown-linux-gnu | tee /tmp/eframe-features.txt` and fail if `grep -q 'eframe feature "wayland"'` (G.8). Red proof by hand: add `"wayland"` to the eframe features, the step's grep finds it (Debug Log), then remove.
- [x] **Task 6: Smoke mode (AC: 8)**
  - [x] 6.1 `cli.rs`: hand-written parser (no clap): no args = main screen; `--gallery`; `--smoke-test` (combinable with `--gallery`); `--help`; anything else → usage on stderr, exit 2. Unit tests.
  - [x] 6.2 `smoke.rs`: a frame counter in the app; after the 3rd `ui()` call it records `ppp`, `ctx.input(|i| i.viewport().inner_rect)`, `ctx.theme()`, and the raw window handle kind (`frame.window_handle()` → `as_raw()` variant name: `Xlib`, `Xcb`, `Wayland`, `AppKit`, `Win32`), prints the `OCB_SMOKE ok ...` line to stdout and sends `ViewportCommand::Close`; `ctx.request_repaint()` each frame until then. A watchdog thread started before `run_native` prints `OCB_SMOKE timeout` and calls `std::process::exit(3)` after 60 s (`std::process::exit` is fine in the app crate; the purity rules are for `ocb-core`). Unit test the line format and the timeout path with an injected duration.
  - [x] 6.3 CI (Task 8) runs `target/debug/bonooru --smoke-test` and `--smoke-test --gallery` on the three OS and greps the line.
- [x] **Task 7: Token gallery (AC: 11)**
  - [x] 7.1 `ui/gallery.rs`: a scrollable screen (plain egui `ScrollArea` + painter; no shared helpers yet) with the sections of AC 11. Every colour from `theme::color_tokens()` (so it can never miss one), status colours as text "ok / warning / error / unknown" in their colour on `panel-raised`, the 2 pt `amber-ink` ring at 2 pt offset around a sample control on `parchment` and on `parchment-header`, `scrim` over a strip of panel + parchment, the text-score line `D5/4 E5/8 F#5/8 G5/2  # Ode to Joy` in Hack 13 on `well`, one specimen per typography token labelled with its name, size and weight. Use only characters the bundled fonts cover (kittest panics on a missing glyph; Nunito Sans has no `↔`, `▶`, `♯`).
  - [x] 7.2 kittest test that the gallery renders at 1024×680 without a missing glyph and lists every colour token name (`get_by_label` per name) (part of G.3).
  - [x] 7.3 Story 4.3 extends this screen into the "hidden debug gallery" of its helpers; name it so (`Mode::Gallery`), keep it out of normal navigation.
- [x] **Task 8: CI and deny (AC: 8, 9, 10)**
  - [x] 8.1 `app/deny.toml`: add `OFL-1.1` and `Ubuntu-font-1.0` to `allow` with a comment (NFR-07 as amended 2026-11-19, AD-18: `epaint_default_fonts`; the bundled TTFs are files, not crates, and are covered by `assets/licenses/`); replace the "join when egui arrives" comment. Add `[bans]` with `multiple-versions = "allow"` (the egui graph has duplicates; this gate is about network crates) and `deny = [{ crate = "reqwest", reason = "AD-21: the app opens no network connections" }, ...]` for the AC 9 list. If `cargo deny check licenses` reports any licence other than these two, stop and ask the owner (NFR-07 decision); at story creation the only non-allowed licence in the egui 0.37 graph was `epaint_default_fonts`' `(MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0`.
  - [x] 8.2 `.github/workflows/app.yml`: a Linux step before the build installing what eframe/wgpu/winit need to build and to run under Xvfb. Start from the spike's proven list minus Wayland, udev and ALSA (those return with Stories 4.4 and 5.11): `pkg-config libxkbcommon-dev libxkbcommon-x11-0 libx11-dev libxcursor-dev libxrandr-dev libxi-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev xvfb mesa-vulkan-drivers libegl1 libgl1-mesa-dri`; add `libwayland-dev` only if the `clipboard` feature's `smithay-clipboard` fails to build without it (record which in the Debug Log).
  - [x] 8.3 Steps after Clippy: version check (`cargo tree ... --depth 0`, AC 1), feature check (Task 5.2, ubuntu only), `cargo build --locked -p ocb-app`, smoke (ubuntu: `WAYLAND_DISPLAY=ocb-no-such-socket xvfb-run -a target/debug/bonooru --smoke-test | tee smoke.txt` then `grep -E '^OCB_SMOKE ok .*backend=(Xlib|Xcb)' smoke.txt`; windows and macos: run directly, `grep '^OCB_SMOKE ok'`), the same for `--smoke-test --gallery`, `timeout-minutes: 5` on the smoke steps. The deny step becomes `cargo deny --locked check licenses bans` and is renamed "Licence and network-crate gate". Add the smoke line to the job summary. Rename the job from "app core" to "app" if it reads better; keep no path filter (vectors are shared).
  - [x] 8.4 If wgpu finds no adapter on a runner (most likely Windows), do not switch the renderer silently: record the error in the Debug Log, try the documented `WGPU_BACKEND` env var for that runner, and ask the owner before adding eframe's `glow` feature.
- [x] **Task 9: Docs and test plan (AC: 10, 11)**
  - [x] 9.1 `app/README.md`: the `gui/` crate (`ocb-app`, binary `bonooru`), the run commands (`cargo run -p ocb-app`, `-- --gallery`, `-- --smoke-test`), the font provenance (3.1) and the new checks (`cargo deny check licenses bans`).
  - [x] 9.2 `docs/test-plan.md`: a new section "App GUI tests (`ocb-app`, no hardware)" after the C table, rows G.1–G.9 as named in the AC → measurement table (G.1 regions, G.2 window, G.3 tokens + gallery, G.4 fonts, G.5 licences, G.6 contrast, G.7 smoke, G.8 Linux feature, G.9 deny), each with pass criterion and Binds (UX-DR1/2/5, AD-18, AD-21). Update the intro sentence of the C section that lists the CI gates.
- [ ] **Task 10: Verify, owner review, close (AC: 10, 11)**
  - [ ] 10.1 Locally in `app/`: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check licenses bans` (install `cargo-deny@0.22.0` as CI does if missing), `cargo run -p ocb-app -- --smoke-test`. Push the branch and get the `app` workflow green on the three OS.
  - [ ] 10.2 Ask the owner to run `cargo run -p ocb-app -- --gallery` (and the plain main screen) and to confirm or change each OQ-7 value; apply the result to `DESIGN.md` (frontmatter and prose, remove `[ASSUMPTION]` of confirmed values), `theme.rs`, `EXPERIENCE.md` Open Questions item 6 ("RESOLVED <date>: ..."), and `epics.md` OQ-7. Run G.3 and G.6 again. If the owner is not available, leave this subtask open, move the story to `review` with the open item stated in the Completion Notes.
  - [ ] 10.3 `sprint-status.yaml`: story to `review`.

## Dev Notes

### Scope in one paragraph

A new GUI crate that opens one themed window and draws five empty regions. Nothing talks to a board: no serial worker (Story 4.4), no LEDs (4.5), no widgets or focus ring (4.3), no STOP (4.7), no gamepad (4.8/4.9). What this story must get right, because every later story depends on it: the theme as the single source of tokens, the font setup (variable-font defaults, families, label tracking), the region layout in reading order, the Linux X11 build, and a CI job that really starts the window on three OS.

### Decisions taken at story creation (change only with a reason in the Debug Log)

- **D1. Crate path `app/gui/`, package `ocb-app`, lib `ocb_app`, bin `bonooru`.** `ocb-app` is the name in `app-stack-options.md` and Story 3.2 D1; existing crates sit in short directories (`core/`, `sim/`); release assets are named `bonooru-<ver>-<os>-<arch>` (`epics.md`, Additional Requirements), so the executable is `bonooru` from day one. The spine's `ui/`, `services/`, `adapters/` (Structural seed) become modules of this crate (`src/ui/` now, `src/services/` in 4.4, `src/adapters/` later). A lib target is needed so integration tests can reach the modules.
- **D2. eframe features `accesskit, clipboard, default_fonts, wgpu, x11`, default features off.** Dropped from eframe's defaults: `wayland` (AD-18), `system_fonts` (would add OS fonts through `egui_system_font_provider`: non-deterministic look, file I/O at start), `links` (`webbrowser`; no links in the app), `web_screen_reader` (web only), `winit/default` (would bring winit's Wayland back). Kept: `accesskit` until Story 4.2 decides the screen-reader level (if 4.2 says "none", drop it there; the X11 hook stays harmless), `clipboard` (Copy diagnostics, text editor later), `default_fonts` (Hack, UX-DR2), `wgpu` (the spike's renderer, eframe's default).
- **D3. Force X11 on Linux at run time** (AC 7). Verified in the crate sources at story creation: `egui-winit 0.37.0` depends on `accesskit_winit 0.34` with default features, whose defaults are `["accesskit_unix", "async-io", "rwh_06", "winit/x11", "winit/wayland"]`; `cargo tree -e features -i winit --target x86_64-unknown-linux-gnu` on a probe crate with D2's features shows `winit feature "wayland"` coming from `accesskit_winit feature "default"`. Cargo features are additive, so winit's Wayland backend is compiled in whatever eframe says. winit 0.30.13 (`src/platform_impl/linux/mod.rs`) selects Wayland when `WAYLAND_DISPLAY`/`WAYLAND_SOCKET` is set and has no fallback, so on a Wayland desktop the app would run natively and lose file drops. `EventLoopBuilderExtX11::with_x11()` forces X11 (XWayland). Removing `WAYLAND_DISPLAY` from the environment instead is `unsafe` in edition 2024 and `unsafe_code` is forbidden.
- **D4. Initial window 1280 × 800.** The full layout starts at 1280 (UX-DR6); 800 fits a 1280 × 800 laptop's work area poorly but the OS clamps an initial size to the screen; the minimum 1024 × 680 is the binding number. Tell the owner if he prefers another start size (not a blocking question).
- **D5. Weights via `FontTweak::coords` defaults + `RichText::variation`.** epaint 0.37 chains the font's tweak coords with the text's own coords (`font_face.rs:614`, `text_layout.rs:1583`), so a per-text `variation("wght", 600.0)` overrides the 400 default. The test in G.4 proves both. Nunito Sans' `opsz` axis (6–12, default 12) and `wdth` (default 100) stay at their defaults: all sizes are ≥ 10 pt, so the 12 pt optical size is right.
- **D6. No pixel snapshots.** `egui_kittest` with `wgpu` + `snapshot` needs a GPU adapter (egui itself runs its snapshot tests only on macOS runners) and the images differ per OS. The plan asks for a smoke test only. Geometry and colours are checked on the tessellation input (`FullOutput::shapes`) instead, which is CPU-only and identical on the three OS.
- **D7. The gallery is part of this story** because AC 11 needs the owner to see colours that the empty main screen does not show (status, `amber-ink`, `scrim`, Hack). Story 4.3 grows it into its helper gallery.

### Current state (read before editing)

- `app/Cargo.toml`: virtual workspace, `resolver = "3"`, members `core`, `sim`, `tools/ocb-dev`; `rust-version = "1.100"`, edition 2024; workspace lints `unsafe_code = "forbid"`, `missing_debug_implementations = "warn"`, clippy `unwrap_used`, `expect_used`, `panic`, `indexing_slicing` = deny (tests opt out with a file-level `#![allow(...)]`, see `core/tests/purity.rs`). Every new type needs `Debug` (derive or manual: `eframe::App` structs holding non-Debug fields need a manual impl).
- `app/rust-toolchain.toml`: Rust 1.100.0 (eframe/egui 0.37 need ≥ 1.95). CI checks the exact rustc.
- `app/deny.toml`: licences only, `[graph] all-features = true` (so `ocb-app` must not declare a feature that pulls Wayland), `confidence-threshold = 0.93`, `unused-allowed-license = "allow"`. The comment says OFL-1.1 and Ubuntu-font-1.0 join with egui (Epic 4, AD-18).
- `.github/workflows/app.yml`: job `core`, matrix `ubuntu-24.04`, `windows-latest`, `macos-latest`, `shell: bash`, `working-directory: app`, checkout with `fetch-depth: 0` (the `git describe` version), toolchain from the file, `Swatinem/rust-cache@v2`, then fmt, `cargo test --locked --workspace`, clippy `-D warnings`, `cargo-deny@0.22.0` via `taiki-e/install-action@v2`, `cargo deny --locked check licenses`, job summary. No Linux apt step today (the core needs none).
- `ocb-core` (`app/core`): protocol, sans-IO link, version (`ocb_core::version::APP_VERSION` from `git describe`). Not touched; the app does not need it yet beyond the dependency (the About view and the worker use it later). C.4 forbids any GUI, thread or I/O identifier in `core/src/`.
- `spikes/app-egui/` is a throwaway (default eframe features, its own lockfile); reuse its knowledge, not its code structure (AD-18, `epics.md` Starter note). Its CI file `spike-app-egui.yml` stays as is.

### egui / eframe 0.37 facts (checked in `~/.cargo/registry/src/*/eframe-0.37.0`, `egui-0.37.0`, `epaint-0.37.0`, `egui_kittest-0.37.0`, `winit-0.30.13` and with `ctx7` on `/emilk/egui/0.37.0`)

- `eframe::App` has `fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame)` (required) and optional `logic(ctx, frame)`, `clear_color`, `on_exit`; the `Ui` has no margin or background. `update(ctx)` and `TopBottomPanel` are gone (spike pain point 1). Online examples are mostly older: read the registry sources.
- `ViewportBuilder::with_title`, `with_app_id`, `with_inner_size`, `with_min_inner_size`; `NativeOptions::event_loop_builder: Option<EventLoopBuilderHook>` = `Box<dyn FnOnce(&mut EventLoopBuilder<UserEvent>)>`; `eframe::EventLoopBuilder` re-exports winit's, so the direct `winit = "=0.30.13"` dependency must stay on the exact version eframe uses.
- `Context::set_theme(ThemePreference::Dark)`, `set_visuals_of(Theme, Visuals)`, `set_style_of`, `all_styles_mut`, `set_fonts`, `content_rect()`, `pixels_per_point()`; `Options::zoom_with_keyboard`.
- `Ui::scope_builder(UiBuilder::new().max_rect(r).id_salt(..), |ui| ..)`. Tab order = registration order, hence the reading-order fill (`review-egui-feasibility.md` #46).
- Fonts: `FontDefinitions::default()` with `default_fonts` holds `Hack`, `Ubuntu-Light`, `egui-icons` (5 brand glyphs; Noto Emoji only with `monochrome_emoji_fonts`). `FontData::from_static(..).tweak(FontTweak { coords: VariationCoords::new([("wght", 400.0)]), .. })`; `FontData::variation_axes()` lists the axes. `RichText::variation("wght", w)`, `RichText::extra_letter_spacing(pt)`, `line_height(Some(pt))`. `RichText::strong()` only changes colour. No OpenType feature switch (no `tnum`).
- `egui_kittest 0.37.0` (MIT OR Apache-2.0, MSRV 1.95, features `eframe`, `snapshot`, `video`, `wgpu`, `x11`): `Harness::builder().with_size(v).with_pixels_per_point(p).with_theme(t).build_eframe(|cc| app)`, `build_ui(|ui| ..)`, `run()`, `run_steps(n)`, `output()` (the `FullOutput`, its `shapes`), `ctx`, `get_by_label`. It panics on a missing glyph by default (good: keep it) and runs an accessibility check that every input widget has a name (no input widgets here yet). Its `eframe` feature enables `eframe/accesskit` only, with eframe default features off.
- Variable-font defaults (measured at story creation with fontTools): Cormorant Garamond `wght` 300–700, default 300; Nunito Sans `wght` 200–1000 default 200, `wdth` 75–125 default 100, `opsz` 6–12 default 12, `YTLC` 440–540 default 500. Glyph coverage: Nunito Sans has `× – − · … “ ” ’` but none of `▶ ■ ⏮ ▾ ▲ ⌫ ↵ ↑ ♯ ♪ ↶ ↷ ↔`; Cormorant has `▶ ■ ▲ ↑ ↔` but not `⏮ ▾ ⌫ ↵ ♯ ♪ ↶ ↷`. This confirms UX-DR3 (painted symbols, Story 4.3) and matters for Story 4.5 if a label ever writes `PC↔Bridge`.

### Licences (AD-21, NFR-07)

The egui 0.37 graph with D2's features plus `egui_kittest` was listed with `cargo metadata` at story creation: every crate is under an allowed licence or an `OR` expression with an allowed side (MIT, Apache-2.0, BSD, ISC, Zlib, MPL-2.0 `colored`, Unicode-3.0, `0BSD OR MIT OR Apache-2.0`, `Unlicense OR MIT`, `Apache-2.0 OR GPL-2.0-only` for `self_cell`), except `epaint_default_fonts` = `(MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0`. (Amended 2026-11-26: the probe missed the Windows-only `clipboard-win` and `error-code`, both `BSL-1.0`; the owner allowed `BSL-1.0`, see the Debug Log.) So the allow-list grows by `OFL-1.1` and `Ubuntu-font-1.0`, plus `BSL-1.0` by the owner decision of 2026-11-26; both already decided by NFR-07 as amended (AD-18 bullet "egui's default fonts are OFL-1.1 / Ubuntu-font-1.0; their licence texts ship with the app"). None of the AC 9 banned crates is in that graph (checked in the probe's `Cargo.lock`). The bundled Cormorant and Nunito files are OFL-1.1 with no Reserved Font Name declared; they ship unmodified.

### Testing standards

- Unit tests in the modules for pure code (`layout`, `cli`, `smoke` line and timeout); integration tests in `app/gui/tests/` with `egui_kittest` (no GPU, no window): they run in `cargo test --workspace` on all three OS.
- Every check that can go red has a red proof in the Debug Log (practice of Stories 3.x and 4.0; retro action E3-A2 applies to bench tools, but the same habit caught real gaps in 4.0's review): G.1 (shift a region by 1 pt), G.3 (one hex changed), G.4 (no `wght` tweak), G.7 (no X11 hook with the bogus `WAYLAND_DISPLAY`, if reproducible on a Linux machine or in CI on a throwaway commit), G.8 (wayland feature added), G.9 (banned crate added).
- Test files opt out of the panic lints with a file-level `#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]`, like `core/tests/purity.rs`.
- The C rows of `docs/test-plan.md` must not change meaning; the new G rows are additive.

### Previous story intelligence (3.2–3.5, 4.0)

- Exact pins with `=` and a committed `Cargo.lock`; CI always `--locked`.
- AC → measurement table first, red proofs in the Debug Log, real trees must pass (4.0). The 4.0 review found a fixture that could not fail for its own rule: make each red proof fail for the reason it claims (e.g. G.3 must name the changed token, not just fail).
- Workspace lints deny `unwrap`/`expect`/`panic`/indexing in library and binary code; use `get`, `?`, and explicit exit codes.
- CI has no path filter on purpose (shared vectors); the new steps must keep the whole job under the existing matrix.
- The spike's pain points: 0.x API churn (read the registry sources, not blog posts), borrow-checker in UI closures (collect actions, apply after), `core` as a module name clashes with the `core` crate (the crate is `ocb_core`, fine).
- Spike numbers for orientation: release binary ~11 MB on macOS, cold start ~115 ms, first frame 60–266 ms (wgpu start-up), idle CPU 0.2 %. The two TTFs add about 1.8 MB.

### Git intelligence

Recent commits are Story 4.0 (purity gates, debug STATUS v6) and the Epic 3 retro/devlog; nothing touches `app/gui/`. The branch `story/4-1-app-shell-with-theme-fonts-and-main-screen-regions` is checked out. Commit style: one imperative sentence naming the story, e.g. "Add the ocb-app shell: theme tokens, bundled fonts, main-screen regions and the three-OS smoke run (Story 4.1)".

### Project Structure Notes

New:
```
app/gui/
  Cargo.toml
  assets/fonts/{CormorantGaramond-VF.ttf, NunitoSans-VF.ttf}
  assets/licenses/{OFL-1.1-CormorantGaramond.txt, OFL-1.1-NunitoSans.txt, Hack.txt, UFL-1.0-Ubuntu.txt, MIT-emoji-icon-font.txt}
  src/{lib.rs, main.rs, app.rs, cli.rs, smoke.rs}
  src/ui/{mod.rs, theme.rs, fonts.rs, layout.rs, main_screen.rs, gallery.rs}
  tests/{design_tokens.rs, fonts.rs, licences.rs, main_screen.rs, window.rs, contrast.rs}
```
Changed: `app/Cargo.toml` (members), `app/Cargo.lock`, `app/deny.toml`, `.github/workflows/app.yml`, `app/README.md`, `docs/test-plan.md`; at the owner review `DESIGN.md`, `EXPERIENCE.md`, `epics.md`. Untouched: `app/core`, `app/sim`, `app/tools/ocb-dev`, `firmware/`, `spikes/`.

Variance: the spine's Structural seed shows `app/ui/`, `app/services/`, `app/adapters/` as directories next to `core/`; here they are modules of `app/gui/` (D1). `app-stack-options.md` allows adapters in `ocb-app` or a separate crate, so this is within the architecture.

### Project Context Rules

No `project-context.md` exists. Rules from memory and the repo: all repo content in English; never edit or commit `_bmad/` or `bmad-*` / `gds-*` skill folders; game screenshots in `ux-designs/*/imports/` are never embedded, linked or committed (the gallery uses tokens only); evoke, don't copy (UX-DR64): nothing from Nintendo in fonts, art or copy.

### Open questions for the owner

1. **UX Open Question 7 (AC 11).** Confirm or change `status-warning #f08a4b`, `status-error #ef6a55`, `status-unknown #9a8a72`, `amber-ink #8a5a12`, `scrim #0b0806b3` and the editor font (Hack 13 px), by looking at `bonooru --gallery`. This is the AC itself; the dev agent prepares the gallery and applies the answer.
2. **Numbering, for the record (no decision needed unless you want one).** `epics.md` calls the visual fine-tuning question OQ-7 and the AccessKit question OQ-9; `EXPERIENCE.md` numbers the same questions 6 and 8. The story records the OQ-7 outcome under both numbers.
3. **Initial window size (D4).** 1280 × 800 is assumed; say if you want another start size. Not blocking.
4. **Only if it happens:** a CI runner without a wgpu adapter (Task 8.4) or a licence other than OFL-1.1 / Ubuntu-font-1.0 in `cargo deny` (Task 8.1) comes back to you before any renderer or allow-list change.

### References

- `_bmad-output/planning-artifacts/epics.md` › Epic 4, Story 4.1; UX-DR1, UX-DR2, UX-DR5, UX-DR6, UX-DR62, UX-DR64; OQ-7, OQ-9
- `_bmad-output/planning-artifacts/ux-designs/ux-bonooru-2026-11-20/DESIGN.md` › frontmatter tokens; Colors; Typography (egui notes); Layout & Spacing; Elevation & Depth; Components › Status strip, Note-view card
- `_bmad-output/planning-artifacts/ux-designs/ux-bonooru-2026-11-20/EXPERIENCE.md` › Foundation; Information Architecture › Main screen; Accessibility Floor; Responsive & Platform; Open Questions 6, 8
- `_bmad-output/planning-artifacts/ux-designs/ux-bonooru-2026-11-20/review-egui-feasibility.md` › Key facts; rows 1–6, 38, 41, 42, 45, 46, 52
- `_bmad-output/planning-artifacts/architecture/architecture-bonooru-2026-11-19/ARCHITECTURE-SPINE.md` › AD-16, AD-18, AD-21; Structural seed; Consistency Conventions
- `_bmad-output/planning-artifacts/architecture/architecture-bonooru-2026-11-19/app-stack-options.md` › Stack table, Licence impact, Test strategy (GUI smoke)
- `_bmad-output/implementation-artifacts/epic-3-retro-2026-11-26.md` › §5 (E3-A1, E3-A2), §6
- `_bmad-output/implementation-artifacts/3-2-rust-workspace-and-protocol-codec-ocb-core.md` › D1; `4-0-cleanup-purity-gates-and-p2-overflow-status.md` › AC → measurement, Review Findings
- `spikes/app-egui/SPIKE-REPORT.md` › Pain points, Licences, Numbers; `spikes/app-egui/Cargo.toml`; `.github/workflows/spike-app-egui.yml` (apt list)
- `app/Cargo.toml`, `app/deny.toml`, `app/rust-toolchain.toml`, `app/core/tests/purity.rs`, `.github/workflows/app.yml`, `docs/test-plan.md` › App core tests
- Crate sources: `eframe-0.37.0/Cargo.toml` (features), `egui-winit-0.37.0/Cargo.toml` (`accesskit_winit` dependency), `accesskit_winit-0.34.2/Cargo.toml` (default features), `winit-0.30.13/src/platform_impl/linux/mod.rs` (backend choice), `epaint-0.37.0/src/text/{font_definitions,font_data,font_tweak}.rs`, `egui_kittest-0.37.0/src/builder.rs`
- Fonts: `github.com/google/fonts` @ `5026800714d07231937d92eb4cceabf06e05d74e`, `ofl/cormorantgaramond/`, `ofl/nunitosans/` (METADATA.pb axes, OFL.txt)
- cargo-deny `[bans]` syntax: `deny = [{ crate = "name", reason = "..." }]` (`/embarkstudios/cargo-deny`, docs/src/checks/bans/cfg.md)

## Dev Agent Record

### Agent Model Used

Claude Opus 5.5 (claude-opus-5-5), gds-dev-story, 2026-11-26.

### Debug Log References

**HALT 2026-11-26 (Task 8.1, NFR-07 decision for the owner).** `cargo deny --locked check licenses bans` on the real graph: `bans ok, licenses FAILED` (rc 4). Besides the expected `OFL-1.1` and `Ubuntu-font-1.0` (now allowed), the graph holds **`BSL-1.0`** (Boost Software License 1.0, OSI approved, FSF free) in two crates: `clipboard-win 5.4.3` and `error-code 3.4.0`, path `error-code <- clipboard-win <- arboard 3.7.0 <- egui-winit 0.37.0 <- eframe 0.37.0` (eframe's `clipboard` feature; `clipboard-win` is the Windows clipboard backend of `arboard`, Windows-only target dependency, which is why the story-creation probe on macOS did not list it; `[graph] all-features`/all targets in cargo-deny does). Not decided by the dev agent. Options for the owner: (a) add `BSL-1.0` to the allow-list (permissive; its notice duty covers source copies, not binaries), or (b) drop eframe's `clipboard` feature (no Copy diagnostics / editor clipboard). No allow-list or feature change made at the HALT.

**Owner decision 2026-11-26: option (a), `BSL-1.0` allowed.** Added to `app/deny.toml` with a comment (`clipboard-win` and `error-code`, Windows only, via eframe's `clipboard` feature; the BSL-1.0 notice obligation covers source copies only). Recorded in AC 9 and the Licences dev note of this story and in the NFR-07 amendment bullet of `ARCHITECTURE-SPINE.md` (AD-18). Then `cargo deny --locked check licenses bans`: `bans ok, licenses ok`, rc 0.

Red proofs (each check made to fail for the reason it claims, then reverted; final tree green):

- **G.3 tokens** (`tests/design_tokens.rs`): `PANEL` hex `0x1c150fff` -> `0x1c1510ff` in `theme.rs`: `token mismatch: colors.panel: DESIGN.md #1c150fff, theme #1c1510ff`. `VEIL` removed from the theme: `colors.veil: in DESIGN.md, missing in theme`. Extra theme token `EXTRA = "extra"`: `colors.extra: in theme, missing in DESIGN.md`. `LABEL` letter spacing 0.06 -> 0.05: `typography.label.letterSpacing: DESIGN.md 0.06, theme 0.05`.
- **G.4 fonts** (`tests/fonts.rs`): Nunito inserted without the `wght` 400 tweak: `text with no weight must render at wght 400, left: 289.03125, right: 299.3125` (no weight = raw file default 289.03). `.strong()` added to `theme::label`: `theme.rs uses .strong(`. Label tracking measured: tracked 25.656 - plain 24.281 = 1.375 pt for `ABC` = 0.69 x (n - 1): egui spaces between glyphs, not after the last (the first run asserted n and failed with 1.375, which settled the comment).
- **G.5 licences** (`tests/licences.rs`): `Hack.txt` moved away: `Hack.txt: No such file or directory (os error 2)`.
- **G.6 contrast** (`tests/contrast.rs`): `status-error` set to `#b8432f`: `status-error on panel-raised: 3.17:1 < 4.5:1`, on panel 3.34, on well 3.00. Real values: text 13.49, text-muted 7.40, status-ok 9.40, status-warning 6.89, status-error 5.60 (panel 5.90, well 5.31), status-unknown 5.10 on panel-raised; amber-ink 4.35 on parchment, 3.85 on parchment-header; ink 11.70 on parchment; out-of-range-ink 4.71.
- **G.1 layout/main screen**: song/gamepad fill shifted by 1 pt in `main_screen.rs`: `song: no fill #1C_15_0F_FF r0 at [[0.0 44.0] - [260.0 592.0]] (1024x680)`. Song panel 1 pt wider in `layout.rs`: `g1_sizes_match_the_tokens` (left 261, right 260) and `g1_offset_content_rect` fail. Gamepad swapped before note view in `in_reading_order()`: unit `g1_reading_order` fails and kittest `gamepad painted out of reading order (1024x680)`.
- **G.2 window**: dark re-assertion removed from `App::logic`: `left: Light, right: Dark` (egui_kittest's `with_theme` calls `ctx.set_theme` after the app constructor, like an OS/user preference change; hence the app re-asserts `ThemePreference::Dark` in `logic()` each frame, cheap no-op when already dark).
- **G.7 smoke exit codes** (macOS, local): real run `OCB_SMOKE ok frames=3 ppp=2 inner=1280x800 min=1024x680 theme=dark backend=AppKit`, rc 0, 1.36 s wall; same with `--gallery`, rc 0. Watchdog: `FRAMES = u32::MAX`, `TIMEOUT = 2 s` (temporary): `OCB_SMOKE timeout after 2 s`, rc 3, 2.56 s wall. Usage error `--nope`: rc 2. Unit tests cover the line format, the frame counter and the timeout path with `Duration::ZERO`.
- **G.8 feature check** (`cargo tree --locked -p ocb-app -e features -i eframe --target x86_64-unknown-linux-gnu`): real tree has no `eframe feature "wayland"` (features: accesskit, clipboard, default_fonts, wgpu, x11); with `"wayland"` added to the eframe features the grep finds `eframe feature "wayland"` and the step returns 1. The same tree on `-i winit` shows `winit feature "wayland"` on, confirming D3 (`accesskit_winit` defaults).
- **G.9 bans**: real graph `bans ok, licenses ok`, rc 0. Red proof: `ureq = "2"` added to `ocb-app` (lockfile updated, then both restored from a copy): `error[banned]: crate 'ureq = 2.12.1' is explicitly banned` and, transitively, `error[banned]: crate 'rustls = 0.23.46' is explicitly banned`, `bans FAILED`, rc 2.

- **Linux, local reproduction in Docker** (`ubuntu:24.04`, aarch64, Docker Desktop on the Mac; the CI apt list of `app.yml` exactly, plus `curl ca-certificates build-essential git xauth python3` for the container): `libwayland-dev` not installed and not needed (Task 8.2: the `clipboard` feature's Linux deps build without it); `cargo build --locked -p ocb-app` rc 0 with Rust 1.100.0; `WAYLAND_DISPLAY=ocb-no-such-socket xvfb-run -a target/debug/bonooru --smoke-test` -> `OCB_SMOKE ok frames=3 ppp=1 inner=1280x800 min=1024x680 theme=dark backend=Xlib`, rc 0; same for `--smoke-test --gallery`, rc 0. wgpu found an adapter (Mesa lavapipe) under Xvfb, so Task 8.4 did not trigger locally. stderr shows four `error: XDG_RUNTIME_DIR is invalid or not set in the environment.` lines per run (Mesa's Wayland probing in a bare container; harmless, the run is on X11).
- **G.7 X11 red proof** (same container): the hook in `native_options()` replaced by `None` -> `bonooru: winit EventLoopError: os error at .../winit-0.30.13/src/platform_impl/linux/wayland/event_loop/mod.rs:89: Could not find wayland compositor`, rc 1. With the hook: `backend=Xlib`, rc 0. This confirms D3: winit's Wayland backend is compiled in and picked whenever `WAYLAND_DISPLAY` is set.
- **CI (Tasks 5.2, 6.3, 8.2, 8.3)**: written in `.github/workflows/app.yml` (job renamed `app`; steps: Linux deps, pins check, G.8 feature check, build, smoke on the three OS, "Licence and network-crate gate", smoke lines in the job summary). The YAML parses (ruby `YAML.load_file`); the three-OS run is checked after the push (Task 10.1).

Other notes:
- Font provenance (source commit, paths, sizes, sha256) recorded in `app/README.md` › Bundled fonts (Task 3.1 / 9.1).
- Fonts downloaded from `google/fonts@5026800714d07231937d92eb4cceabf06e05d74e`: sizes and sha256 match the story exactly (1 195 560 B `b20b7d96...35e0`, 571 240 B `f934d714...491d`). OFL texts from the same commit.
- Deviation (Task 2.2): `color_tokens()` returns `&[ColorToken { name, rgba: u32, color }]` instead of `&[(&str, Color32)]`. Reason: `Color32` is premultiplied, so two low-alpha hexes can map to the same value (e.g. `amber-wash` alpha 0x22: red 240 and 241 both give 32); the test compares the unmultiplied hex exactly and also asserts `color == Color32::from_rgba_unmultiplied(hex)`. One literal per token in a macro feeds both the const and the table.
- Hex literals are written `0x16110cff` (no `_` grouping: clippy `unusual_byte_groupings`).
- `ctx.style()` of the AC table is `ctx.global_style()` in egui 0.37.

### Completion Notes List

- 2026-11-26: Tasks 1 to 9 implemented. `ocb-app` (`app/gui/`, binary `bonooru`) opens one dark "Bonooru" window (1280×800, min 1024×680, keyboard zoom off) with the five main-screen regions filled in reading order, the theme module as the single token source (G.3 checks it against `DESIGN.md` both ways), the bundled Cormorant Garamond and Nunito Sans variable fonts with default weights 700/400 (G.4), the font licences (G.5), the token gallery (`--gallery`), the smoke mode with its 60 s watchdog (G.7) and the forced X11 backend on Linux (AD-18).
- Licence decision (owner, 2026-11-26): `BSL-1.0` added to the allow-list (`clipboard-win`, `error-code`; Windows only, eframe `clipboard` feature; notice obligation for source copies only). AC 9 now reads OFL-1.1, Ubuntu-font-1.0 and BSL-1.0; the NFR-07 amendment in `ARCHITECTURE-SPINE.md` says the same.
- Local verification (macOS): fmt, clippy `-D warnings`, `cargo test --locked --workspace` (38 test binaries, 168 tests, 0 failed), `cargo deny --locked check licenses bans` rc 0, pins `eframe v0.37.0` / `egui v0.37.0`, smoke rc 0 (`backend=AppKit`). Linux smoke reproduced in Docker (`backend=Xlib`).
- Open: Task 10.1 (CI on three OS after the push), Task 10.2 / AC 11 (owner looks at `cargo run -p ocb-app -- --gallery` and confirms or changes the OQ-7 values), Task 10.3. Story stays `in-progress`.

- Story created 2026-11-26 by gds-create-story: ultimate context engine analysis completed, comprehensive developer guide created.

### File List

- app/Cargo.toml (members, header comment)
- app/Cargo.lock
- app/deny.toml (OFL-1.1, Ubuntu-font-1.0, BSL-1.0, `[bans]`)
- app/README.md
- .github/workflows/app.yml
- docs/test-plan.md (G.1 to G.9)
- _bmad-output/planning-artifacts/architecture/architecture-bonooru-2026-11-19/ARCHITECTURE-SPINE.md (NFR-07 amendment: BSL-1.0)
- _bmad-output/implementation-artifacts/4-1-app-shell-with-theme-fonts-and-main-screen-regions.md
- app/gui/Cargo.toml
- app/gui/assets/fonts/CormorantGaramond-VF.ttf, app/gui/assets/fonts/NunitoSans-VF.ttf
- app/gui/assets/licenses/OFL-1.1-CormorantGaramond.txt, OFL-1.1-NunitoSans.txt, Hack.txt, UFL-1.0-Ubuntu.txt, MIT-emoji-icon-font.txt
- app/gui/src/lib.rs, main.rs, app.rs, cli.rs, smoke.rs
- app/gui/src/ui/mod.rs, theme.rs, fonts.rs, layout.rs, main_screen.rs, gallery.rs
- app/gui/tests/design_tokens.rs, fonts.rs, licences.rs, main_screen.rs, window.rs, contrast.rs, gallery.rs
- _bmad-output/implementation-artifacts/sprint-status.yaml (story in-progress)

### Change Log

- 2026-11-26: App shell implemented (Tasks 1 to 9): `ocb-app` crate, theme, fonts, regions, gallery, smoke run, X11 on Linux, CI steps, deny `[bans]`, BSL-1.0 by owner decision, README and test-plan G rows. Pending: CI on three OS (10.1), owner gallery review AC 11 (10.2).
