//! The hidden debug gallery (`bonooru --gallery`, `Mode::Gallery`): every
//! colour token, the status colours on their backgrounds, the `amber-ink`
//! ring on parchment, the `scrim`, a text-score sample and one specimen per
//! typography token (Story 4.1 AC 11), then one section per painting helper
//! of Story 4.3: focus ring, glow, text halo, dashed rects, convex fills,
//! stroke floor, digit cells, the 12 painted symbols, corner brackets, a
//! readiness-style sample dialog and the real firmware dialog of Story 4.6,
//! then the Story 4.6 inline hint at two widths. Not reachable from normal
//! navigation; the gallery never reads a dialog's outcome.
//!
//! Only characters the bundled fonts cover (egui_kittest fails on a missing
//! glyph; Nunito Sans has no arrows, play triangles or music signs: the
//! symbols are painted). No visible text equals a colour or typography token
//! name, except the one row that shows it (G.3 "exactly once").

use egui::epaint::PathShape;
use egui::{Align2, Color32, Pos2, Rect, ScrollArea, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

use super::modal::{ModalAction, ModalBody, ModalSpec, Modals};
use super::paint;
use super::symbols::Symbol;
use super::theme::{self, ColorToken};
use super::widgets::{self, ButtonKind};

const SWATCH: f32 = 28.0;
// Gallery sample sizes that are not tokens (like the 120 x 28 ring sample
// control).
const STEPPER_SAMPLE: egui::Vec2 = egui::Vec2::new(44.0, 28.0);
const DIALOG_BUTTON_SAMPLE: egui::Vec2 = egui::Vec2::new(72.0, 34.0);
const STOP_SAMPLE: egui::Vec2 = egui::Vec2::new(120.0, 60.0);
const DASHED_SAMPLE: egui::Vec2 = egui::Vec2::new(170.0, 40.0);
const DASH: f32 = 5.0;
const GAP: f32 = 3.0;
const SHEET_SAMPLE: egui::Vec2 = egui::Vec2::new(240.0, 120.0);
const RULE_SAMPLE: f32 = 220.0;
const CONVEX_CELL: f32 = 56.0;
const PARCHMENT_CONTROL_HEIGHT: f32 = 28.0;
const HINT_SAMPLE: egui::Vec2 = egui::Vec2::new(300.0, 120.0);
const HINT_SAMPLE_COMPACT: egui::Vec2 = egui::Vec2::new(200.0, 120.0);
/// The symbol sizes the gallery shows.
pub const SYMBOL_SIZES: [f32; 4] = [10.0, 12.0, 14.0, 18.0];

/// The gallery's readiness-style sample dialog (copy from `EXPERIENCE.md`
/// Microcopy, draft).
#[must_use]
pub fn readiness_dialog() -> ModalSpec {
    ModalSpec::new(
        "gallery-readiness",
        "Ready to play?",
        ModalBody::Checklist(vec![
            "The ocarina is out in free play.".to_owned(),
            "Don\u{2019}t touch the controller until the song ends.".to_owned(),
            "Don\u{2019}t dock the Switch.".to_owned(),
        ]),
        ModalAction::new("Play"),
    )
    .with_secondary(ModalAction::new("Cancel"))
}

/// The real "Firmware update needed" dialog of Story 4.6 (OK only, 440 pt),
/// so the gallery shows it for the owner's eye check.
#[must_use]
pub fn update_dialog() -> ModalSpec {
    super::firmware::required_update_dialog()
}

/// Draws the gallery over `ui.max_rect()`; its dialog buttons request the
/// sample dialogs from `modals`.
pub fn show(ui: &mut Ui, modals: &mut Modals) {
    let content = ui.max_rect();
    ui.painter()
        .rect_filled(content, theme::corner(0.0), theme::FRAME);
    let inner = content.shrink(theme::spacing::S5);
    ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.label(theme::rich(&theme::BRAND, "Bonooru token gallery"));
                ui.label(theme::rich(
                    &theme::SMALL,
                    "UX Open Question 7: status-warning, status-error, status-unknown, \
                     amber-ink, scrim and the editor font (Hack 13) are proposals.",
                ));
                section(ui, "Colour tokens");
                color_swatches(ui);
                section(ui, "Status colours");
                status_samples(ui);
                section(ui, "amber-ink focus ring on parchment");
                amber_ink_rings(ui);
                section(ui, "scrim over panel and parchment");
                scrim_sample(ui);
                section(ui, "Text score (code, Hack 13 on well)");
                text_score(ui);
                section(ui, "Typography");
                typography(ui);
                helper_sections(ui, modals);
            });
    });
}

fn section(ui: &mut Ui, title: &str) {
    ui.add_space(theme::spacing::S5);
    ui.label(theme::label(title).color(theme::TEXT_MUTED));
    ui.add_space(theme::spacing::S2);
}

fn hex(token: &ColorToken) -> String {
    format!("#{:08x}", token.rgba)
}

/// Paints a swatch: the colour over a split frame/parchment backing, so
/// alpha tokens show how they composite on both.
fn swatch(ui: &mut Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(SWATCH, SWATCH), Sense::hover());
    let p = ui.painter();
    let r = theme::corner(theme::rounded::SM);
    let (left, right) = rect.split_left_right_at_fraction(0.5);
    p.rect_filled(left, theme::corner(0.0), theme::FRAME);
    p.rect_filled(right, theme::corner(0.0), theme::PARCHMENT);
    p.rect_filled(rect, r, color);
    p.rect_stroke(
        rect,
        r,
        Stroke::new(1.0, theme::RULE_STRONG),
        StrokeKind::Outside,
    );
}

fn color_swatches(ui: &mut Ui) {
    egui::Grid::new("gallery-colors")
        .num_columns(3)
        .spacing(vec2(theme::spacing::S3, theme::spacing::S1))
        .show(ui, |ui| {
            for token in theme::color_tokens() {
                swatch(ui, token.color);
                ui.label(theme::rich(&theme::BODY_STRONG, token.name));
                ui.label(theme::rich(&theme::CODE, hex(token)).color(theme::TEXT_MUTED));
                ui.end_row();
            }
        });
}

/// One strip of the four status colours as text and swatch on `bg`.
fn status_strip(ui: &mut Ui, bg_name: &str, bg: Color32, items: &[(&str, Color32)]) {
    egui::Frame::new()
        .fill(bg)
        .inner_margin(theme::spacing::S3)
        .corner_radius(theme::corner(theme::rounded::MD))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    theme::rich(&theme::SMALL, format!("on {bg_name}")).color(theme::TEXT_MUTED),
                );
                for (name, color) in items {
                    ui.add_space(theme::spacing::S4);
                    let led = theme::components::STATUS_LED_SIZE;
                    let (rect, _) = ui.allocate_exact_size(vec2(led, led), Sense::hover());
                    ui.painter().circle_filled(rect.center(), led / 2.0, *color);
                    ui.label(theme::rich(&theme::BODY_STRONG, *name).color(*color));
                }
            });
        });
    ui.add_space(theme::spacing::S2);
}

fn status_samples(ui: &mut Ui) {
    let all = [
        ("ok", theme::STATUS_OK),
        ("warning", theme::STATUS_WARNING),
        ("error", theme::STATUS_ERROR),
        ("unknown", theme::STATUS_UNKNOWN),
    ];
    status_strip(ui, "panel-raised", theme::PANEL_RAISED, &all);
    let error = [("error", theme::STATUS_ERROR)];
    status_strip(ui, "panel", theme::PANEL, &error);
    status_strip(ui, "well", theme::WELL, &error);
}

/// A sample control on `bg` with the 2 pt `amber-ink` ring at 2 pt offset,
/// its label laid out first and the control stacked below it, so the ring
/// never covers the label (owner review 2026-11-27).
fn ring_sample(ui: &mut Ui, bg_name: &str, bg: Color32) {
    let font = theme::SMALL.font_id();
    let label = ui
        .painter()
        .layout_no_wrap(bg_name.to_owned(), font.clone(), theme::INK_SOFT);
    let inset = theme::spacing::CARD_INSET;
    let gap = theme::spacing::S2;
    let control_size = vec2(120.0, 28.0);
    let radius = theme::rounded::MD;
    let offset = theme::spacing::FOCUS_RING_OFFSET;
    let ring_outer = offset + theme::spacing::FOCUS_RING_WIDTH;
    let label_ink = label.rect.union(label.mesh_bounds);
    let width = (control_size.x + 2.0 * ring_outer)
        .max(label_ink.width())
        .max(220.0)
        + 2.0 * inset;
    let height = inset + label_ink.height() + gap + control_size.y + 2.0 * ring_outer + inset;
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, theme::corner(radius), bg);
    let label_pos = rect.left_top() + vec2(inset, inset) - label_ink.min.to_vec2();
    let control = Rect::from_min_size(
        egui::pos2(
            rect.center().x - control_size.x / 2.0,
            rect.top() + inset + label_ink.height() + gap + ring_outer,
        ),
        control_size,
    );
    p.rect_filled(control, theme::corner(radius), theme::PARCHMENT_HEADER_ALT);
    p.rect_stroke(
        control,
        theme::corner(radius),
        Stroke::new(1.0, theme::INK),
        StrokeKind::Inside,
    );
    p.rect_stroke(
        control.expand(offset),
        theme::corner(radius + offset),
        Stroke::new(theme::spacing::FOCUS_RING_WIDTH, theme::AMBER_INK),
        StrokeKind::Outside,
    );
    p.text(
        control.center(),
        Align2::CENTER_CENTER,
        "Control",
        font,
        theme::INK,
    );
    p.galley(label_pos, label, theme::INK_SOFT);
}

fn amber_ink_rings(ui: &mut Ui) {
    ui.horizontal(|ui| {
        ring_sample(ui, "parchment", theme::PARCHMENT);
        ui.add_space(theme::spacing::S4);
        ring_sample(ui, "parchment-header", theme::PARCHMENT_HEADER);
    });
}

fn scrim_sample(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(440.0, 90.0), Sense::hover());
    let p = ui.painter();
    let (left, right) = rect.split_left_right_at_fraction(0.5);
    p.rect_filled(left, theme::corner(0.0), theme::PANEL);
    p.rect_filled(
        right.shrink(theme::spacing::CARD_INSET),
        theme::corner(theme::rounded::XL),
        theme::PARCHMENT,
    );
    let font = theme::BODY.font_id();
    p.text(
        left.center(),
        Align2::CENTER_CENTER,
        "panel text",
        font.clone(),
        theme::TEXT,
    );
    p.text(
        right.center(),
        Align2::CENTER_CENTER,
        "parchment ink",
        font,
        theme::INK,
    );
    // The scrim over the right two thirds, so both halves show with and
    // without it.
    let scrim = Rect::from_min_max(rect.lerp_inside(vec2(0.33, 0.0)), rect.max);
    p.rect_filled(scrim, theme::corner(0.0), theme::SCRIM);
}

fn text_score(ui: &mut Ui) {
    egui::Frame::new()
        .fill(theme::WELL)
        .inner_margin(theme::spacing::S3)
        .corner_radius(theme::corner(theme::rounded::MD))
        .show(ui, |ui| {
            ui.label(
                theme::rich(&theme::CODE, "D5/4 E5/8 F#5/8 G5/2  # Ode to Joy").color(theme::TEXT),
            );
            ui.label(theme::rich(&theme::CODE, "error underline").color(theme::STATUS_ERROR));
        });
}

/// One row per typography token: its exact name (queryable by tests), a
/// specimen in the token, and its family, size and weight. The specimen's
/// cell is sized from the laid-out galley's ink bounds (layout rect united
/// with the glyph mesh bounds): Cormorant Garamond's default old-style
/// figures descend below the baseline, so the `countdown` "3" is taller
/// than its 1.0 line height (owner review 2026-11-27).
fn typography(ui: &mut Ui) {
    egui::Grid::new("gallery-typography")
        .num_columns(3)
        .spacing(vec2(theme::spacing::S4, theme::spacing::S2))
        .show(ui, |ui| {
            for (name, style) in theme::typography_tokens() {
                ui.label(theme::rich(&theme::BODY_STRONG, *name).color(theme::TEXT_MUTED));
                let sample = if style.size > 60.0 {
                    "3"
                } else {
                    "Song of Storms"
                };
                let text = if style.letter_spacing_em > 0.0 {
                    sample.to_uppercase()
                } else {
                    sample.to_owned()
                };
                specimen(ui, theme::rich(style, text).color(theme::TEXT));
                let spec = format!(
                    "{} {} pt, wght {}",
                    style.family.design_name(),
                    style.size,
                    style.weight
                );
                ui.label(theme::rich(&theme::SMALL, spec).color(theme::TEXT_MUTED));
                ui.end_row();
            }
        });
}

/// Paints `text` in a cell as large as its ink bounds.
fn specimen(ui: &mut Ui, text: egui::RichText) {
    let galley = egui::WidgetText::from(text).into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Body,
    );
    let ink = galley.rect.union(galley.mesh_bounds);
    let (cell, _) = ui.allocate_exact_size(ink.size(), Sense::hover());
    ui.painter()
        .galley(cell.min - ink.min.to_vec2(), galley, theme::TEXT);
}

/// The Story 4.3 helper sections.
fn helper_sections(ui: &mut Ui, modals: &mut Modals) {
    section(ui, "Focus ring (Tab to focus each sample)");
    focus_ring_samples(ui);
    section(ui, "Glow at each blur token");
    glow_samples(ui);
    section(ui, "Text halo sized from ink bounds");
    halo_sample(ui);
    section(ui, "Dashed rounded rects");
    dashed_samples(ui);
    section(ui, "Convex fills");
    convex_samples(ui);
    section(ui, "Stroke floor");
    stroke_floor(ui);
    section(ui, "Digit cells");
    digit_samples(ui);
    section(ui, "Painted symbols at 10, 12, 14 and 18 pt");
    symbol_samples(ui);
    section(ui, "Corner brackets");
    bracket_samples(ui);
    section(ui, "Dialogs");
    dialog_buttons(ui, modals);
    section(ui, "inline-hint in a panel at 300 and 200 pt");
    inline_hint_samples(ui);
}

/// The Story 4.6 gamepad hint in a `panel` box at the regular and at the
/// compact (Story 4.9) width, inset as in the gamepad region.
fn inline_hint_samples(ui: &mut Ui) {
    use theme::components::inline_hint::INSET;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme::spacing::S4;
        for size in [HINT_SAMPLE, HINT_SAMPLE_COMPACT] {
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            ui.painter()
                .rect_filled(rect, theme::corner(0.0), theme::PANEL);
            let inner = Rect::from_min_max(
                pos2(rect.left() + INSET, rect.top() + INSET),
                pos2(rect.right() - INSET, rect.bottom()),
            );
            ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                widgets::inline_hint(ui, inner, crate::gating::HINT_FIRMWARE);
            });
        }
    });
    ui.add_space(theme::spacing::S5);
}

/// A themed button on dark (with and without an icon), and a focusable
/// control on parchment that paints the `amber-ink` ring.
fn focus_ring_samples(ui: &mut Ui) {
    ui.horizontal(|ui| {
        egui::Frame::new()
            .fill(theme::PANEL)
            .inner_margin(theme::spacing::S4)
            .corner_radius(theme::corner(theme::rounded::MD))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = theme::spacing::S4;
                    widgets::button(ui, ButtonKind::Ghost, "Sample button", true, None);
                    widgets::button(
                        ui,
                        ButtonKind::Primary,
                        "Play sample",
                        true,
                        Some(Symbol::Play),
                    );
                });
            });
        ui.add_space(theme::spacing::S4);
        egui::Frame::new()
            .fill(theme::PARCHMENT)
            .inner_margin(theme::spacing::S4)
            .corner_radius(theme::corner(theme::rounded::MD))
            .show(ui, parchment_control);
    });
}

/// A focusable sample control on parchment (`amber-ink` ring, AC 1).
fn parchment_control(ui: &mut Ui) {
    let name = "Sample on parchment";
    let galley = paint::layout_line(ui.ctx(), &theme::SMALL, name, theme::INK);
    let size = vec2(
        galley.rect.width() + 2.0 * theme::spacing::S3,
        PARCHMENT_CONTROL_HEIGHT,
    );
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let radius = theme::rounded::MD;
    let p = ui.painter();
    p.rect_filled(rect, theme::corner(radius), theme::PARCHMENT_HEADER_ALT);
    p.rect_stroke(
        rect,
        theme::corner(radius),
        paint::stroke(1.0, theme::INK, ui.ctx().pixels_per_point()),
        StrokeKind::Inside,
    );
    let pos = rect.center() - galley.rect.size() / 2.0;
    p.galley(pos, galley, theme::INK);
    paint::paint_focus_ring(ui, &resp, radius, true);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
}

/// One glow sample per blur token, each on its control shape, with its
/// caption below.
fn glow_samples(ui: &mut Ui) {
    use theme::glow;
    let led = theme::components::STATUS_LED_SIZE;
    let orb = theme::spacing::ORB_ICON_SIZE;
    let play = vec2(
        theme::spacing::BIG_BUTTON_MIN_WIDTH,
        theme::spacing::BIG_BUTTON_HEIGHT,
    );
    // (caption, size, radius, blur, glow colour, fill)
    let samples = [
        (
            "LED 8",
            vec2(led, led),
            led / 2.0,
            glow::STATUS_LED,
            theme::STATUS_OK,
            theme::STATUS_OK,
        ),
        (
            "Stepper 10",
            STEPPER_SAMPLE,
            theme::rounded::MD,
            glow::STEPPER_ACTIVE,
            theme::AMBER_GLOW,
            theme::AMBER,
        ),
        (
            "Dialog button 12",
            DIALOG_BUTTON_SAMPLE,
            theme::rounded::LG,
            glow::MODAL_PRIMARY,
            theme::AMBER_GLOW,
            theme::AMBER,
        ),
        (
            "Play 16",
            play,
            theme::rounded::XL,
            glow::PLAY_PRIMARY,
            theme::AMBER_GLOW,
            theme::AMBER,
        ),
        (
            "STOP 18",
            STOP_SAMPLE,
            theme::rounded::XL,
            glow::STOP_LIVE,
            theme::EMBER,
            theme::EMBER,
        ),
        (
            "Orb 8",
            vec2(orb, orb),
            orb / 2.0,
            glow::ORB,
            theme::AMBER,
            theme::ON_EMBER,
        ),
    ];
    // Room for the widest glow: half the blur plus the spread stays below
    // the blur itself for every sample here.
    let room = f32::from(glow::STOP_LIVE);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for (caption, size, radius, blur, glow_color, fill) in samples {
            let caption = paint::layout_line(ui.ctx(), &theme::SMALL, caption, theme::TEXT_MUTED);
            let width = size.x.max(caption.rect.width()) + 2.0 * room;
            let shape_h = play.y + 2.0 * room;
            let (cell, _) = ui
                .allocate_exact_size(vec2(width, shape_h + caption.rect.height()), Sense::hover());
            let shape =
                Rect::from_center_size(pos2(cell.center().x, cell.top() + shape_h / 2.0), size);
            let p = ui.painter();
            paint::paint_glow(p, shape, radius, blur, glow_color);
            p.rect_filled(shape, theme::corner(radius), fill);
            let pos = pos2(
                cell.center().x - caption.rect.width() / 2.0,
                cell.top() + shape_h,
            );
            p.galley(pos, caption, theme::TEXT_MUTED);
        }
    });
}

/// A Cormorant numeral with its halo centred on the glyph's ink bounds.
fn halo_sample(ui: &mut Ui) {
    let galley = paint::layout_line(ui.ctx(), &theme::COUNTDOWN, "7", theme::TEXT);
    let pad = theme::spacing::S3;
    let blur = theme::glow::PLAY_PRIMARY;
    // The cell holds the layout rect and the ink (no text overlaps); the
    // halo itself is centred on the ink alone.
    let bounds = galley.rect.union(paint::ink_rect(&galley, Pos2::ZERO));
    let room = pad + f32::from(paint::feather_from_css_blur(blur));
    let (rect, _) = ui.allocate_exact_size(bounds.size() + vec2(2.0, 2.0) * room, Sense::hover());
    let pos = rect.min + vec2(room, room) - bounds.min.to_vec2();
    let p = ui.painter();
    paint::paint_text_halo(
        p,
        paint::ink_rect(&galley, pos),
        pad,
        blur,
        theme::AMBER_GLOW,
    );
    p.galley(pos, galley, theme::TEXT);
}

/// Dashed rounded rects at 1 pt and 1.6 pt, dash 5 / gap 3, on parchment.
fn dashed_samples(ui: &mut Ui) {
    let pad = theme::spacing::S4;
    let width = 2.0 * DASHED_SAMPLE.x + 3.0 * pad;
    let (rect, _) =
        ui.allocate_exact_size(vec2(width, DASHED_SAMPLE.y + 2.0 * pad), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, theme::corner(theme::rounded::MD), theme::PARCHMENT);
    let first = Rect::from_min_size(rect.min + vec2(pad, pad), DASHED_SAMPLE);
    let second = first.translate(vec2(DASHED_SAMPLE.x + pad, 0.0));
    let items = [
        (
            first,
            1.0,
            theme::rounded::SM,
            theme::OUT_OF_RANGE_INK,
            "1 pt, dash 5, gap 3",
        ),
        (
            second,
            1.6,
            theme::rounded::LG,
            theme::EMBER,
            "1.6 pt, dash 5, gap 3",
        ),
    ];
    for (r, w, radius, color, caption) in items {
        paint::paint_dashed_rect(p, r, radius, Stroke::new(w, color), DASH, GAP);
        p.text(
            r.center(),
            Align2::CENTER_CENTER,
            caption,
            theme::SMALL.font_id(),
            theme::INK,
        );
    }
}

/// Three convex fills and one concave L that `fill_convex` refuses (drawn
/// as an outline only).
fn convex_samples(ui: &mut Ui) {
    let cell = CONVEX_CELL;
    let gap = theme::spacing::S5;
    let caption = paint::layout_line(
        ui.ctx(),
        &theme::SMALL,
        "concave L: refused",
        theme::TEXT_MUTED,
    );
    let (rect, _) = ui.allocate_exact_size(
        vec2(
            3.0 * (cell + gap) + cell.max(caption.rect.width()),
            cell + caption.rect.height(),
        ),
        Sense::hover(),
    );
    let p = ui.painter();
    let ppp = ui.ctx().pixels_per_point();
    let at = |i: usize, pts: &[(f32, f32)]| -> Vec<Pos2> {
        let left = rect.left() + i as f32 * (cell + gap);
        pts.iter()
            .map(|&(x, y)| pos2(left + x * cell, rect.top() + y * cell))
            .collect()
    };
    let shapes: [&[(f32, f32)]; 3] = [
        &[(0.1, 0.9), (0.5, 0.1), (0.9, 0.9)],
        &[(0.5, 0.05), (0.95, 0.5), (0.5, 0.95), (0.05, 0.5)],
        &[
            (0.3, 0.1),
            (0.7, 0.1),
            (0.9, 0.5),
            (0.7, 0.9),
            (0.3, 0.9),
            (0.1, 0.5),
        ],
    ];
    for (i, pts) in shapes.iter().enumerate() {
        // Convex by construction.
        let _ = paint::fill_convex(p, at(i, pts), theme::AMBER, Stroke::NONE);
    }
    let l_shape = at(
        3,
        &[
            (0.1, 0.1),
            (0.4, 0.1),
            (0.4, 0.6),
            (0.9, 0.6),
            (0.9, 0.9),
            (0.1, 0.9),
        ],
    );
    if paint::fill_convex(p, l_shape.clone(), theme::AMBER, Stroke::NONE).is_err() {
        p.add(PathShape::closed_line(
            l_shape,
            paint::stroke(1.0, theme::TEXT_MUTED, ppp),
        ));
    }
    let left = rect.left() + 3.0 * (cell + gap);
    p.galley(pos2(left, rect.top() + cell), caption, theme::TEXT_MUTED);
}

/// A 0.5 pt request (floored to one physical pixel) next to a 1 pt rule.
fn stroke_floor(ui: &mut Ui) {
    let ppp = ui.ctx().pixels_per_point();
    egui::Grid::new("gallery-stroke-floor")
        .num_columns(2)
        .spacing(vec2(theme::spacing::S4, theme::spacing::S2))
        .show(ui, |ui| {
            for (caption, width) in [("0.5 pt asked", 0.5), ("1 pt asked", 1.0)] {
                ui.label(theme::rich(&theme::SMALL, caption).color(theme::TEXT_MUTED));
                let (rect, _) =
                    ui.allocate_exact_size(vec2(RULE_SAMPLE, theme::spacing::S3), Sense::hover());
                let y = rect.center().y;
                ui.painter().line_segment(
                    [pos2(rect.left(), y), pos2(rect.right(), y)],
                    paint::stroke(width, theme::TEXT, ppp),
                );
                ui.end_row();
            }
        });
}

/// Times in digit cells next to the same text drawn normally.
fn digit_samples(ui: &mut Ui) {
    egui::Grid::new("gallery-digits")
        .num_columns(2)
        .spacing(vec2(theme::spacing::S6, theme::spacing::S2))
        .show(ui, |ui| {
            ui.label(theme::rich(&theme::SMALL, "in digit cells").color(theme::TEXT_MUTED));
            ui.label(theme::rich(&theme::SMALL, "drawn normally").color(theme::TEXT_MUTED));
            ui.end_row();
            for text in ["0:11 / 1:11", "8:88 / 8:88"] {
                widgets::time(ui, text, &theme::BODY);
                ui.label(theme::rich(&theme::BODY, text).color(theme::TEXT));
                ui.end_row();
            }
        });
}

/// All 12 symbols at four sizes, each named in plain words.
fn symbol_samples(ui: &mut Ui) {
    egui::Grid::new("gallery-symbols")
        .num_columns(1 + SYMBOL_SIZES.len())
        .spacing(vec2(theme::spacing::S4, theme::spacing::S1))
        .show(ui, |ui| {
            for symbol in Symbol::ALL {
                ui.label(theme::rich(&theme::BODY_STRONG, symbol.name()).color(theme::TEXT_MUTED));
                for size in SYMBOL_SIZES {
                    widgets::symbol(ui, symbol, size, theme::TEXT, symbol.name());
                }
                ui.end_row();
            }
        });
}

/// Brackets in `bracket` on a parchment sheet and in `rim` on a dark
/// overlay.
fn bracket_samples(ui: &mut Ui) {
    let ppp = ui.ctx().pixels_per_point();
    ui.horizontal(|ui| {
        let (sheet, _) = ui.allocate_exact_size(SHEET_SAMPLE, Sense::hover());
        let p = ui.painter();
        p.rect_filled(sheet, theme::corner(theme::rounded::LG), theme::PARCHMENT);
        paint::paint_corner_brackets(p, sheet, theme::BRACKET, ppp);
        p.text(
            sheet.center(),
            Align2::CENTER_CENTER,
            "on a parchment sheet",
            theme::SMALL.font_id(),
            theme::INK_SOFT,
        );
        ui.add_space(theme::spacing::S4);
        let (overlay, _) = ui.allocate_exact_size(SHEET_SAMPLE, Sense::hover());
        let p = ui.painter();
        p.rect_filled(overlay, theme::corner(theme::rounded::LG), theme::PANEL);
        p.rect_filled(overlay, theme::corner(theme::rounded::LG), theme::VEIL);
        paint::paint_corner_brackets(p, overlay, theme::RIM, ppp);
        p.text(
            overlay.center(),
            Align2::CENTER_CENTER,
            "on a dark overlay",
            theme::SMALL.font_id(),
            theme::TEXT_MUTED,
        );
    });
}

/// Buttons that open the readiness sample and the real firmware dialog (its
/// outcome is never read here: no `dialog` line from the gallery), and a
/// disabled sample.
fn dialog_buttons(ui: &mut Ui, modals: &mut Modals) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme::spacing::S4;
        if widgets::button(ui, ButtonKind::Ghost, "Open readiness dialog", true, None).clicked() {
            modals.request(readiness_dialog());
        }
        if widgets::button(ui, ButtonKind::Ghost, "Open update dialog", true, None).clicked() {
            modals.request(update_dialog());
        }
        widgets::button(ui, ButtonKind::Primary, "Disabled sample", false, None);
    });
    ui.add_space(theme::spacing::S5);
}
