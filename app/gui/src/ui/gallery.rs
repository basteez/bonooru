//! The token gallery (`bonooru --gallery`, `Mode::Gallery`): every colour
//! token, the status colours on their backgrounds, the `amber-ink` ring on
//! parchment, the `scrim`, a text-score sample and one specimen per
//! typography token. It exists for the owner's review of UX Open Question 7
//! (Story 4.1 AC 11); Story 4.3 grows it into the hidden debug gallery of
//! its painting helpers. Not reachable from normal navigation.
//!
//! Only characters the bundled fonts cover (egui_kittest fails on a missing
//! glyph; Nunito Sans has no arrows, play triangles or music signs).

use egui::{Align2, Color32, Rect, ScrollArea, Sense, Stroke, StrokeKind, Ui, vec2};

use super::theme::{self, ColorToken};

const SWATCH: f32 = 28.0;

/// Draws the gallery over `ui.max_rect()`.
pub fn show(ui: &mut Ui) {
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
                    let (rect, _) = ui.allocate_exact_size(vec2(11.0, 11.0), Sense::hover());
                    ui.painter().circle_filled(rect.center(), 5.5, *color);
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

/// A sample control on `bg` with the 2 pt `amber-ink` ring at 2 pt offset.
fn ring_sample(ui: &mut Ui, bg_name: &str, bg: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(220.0, 64.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, theme::corner(theme::rounded::MD), bg);
    let control = Rect::from_center_size(rect.center(), vec2(120.0, 28.0));
    let radius = theme::rounded::MD;
    p.rect_filled(control, theme::corner(radius), theme::PARCHMENT_HEADER_ALT);
    p.rect_stroke(
        control,
        theme::corner(radius),
        Stroke::new(1.0, theme::INK),
        StrokeKind::Inside,
    );
    let offset = theme::spacing::FOCUS_RING_OFFSET;
    p.rect_stroke(
        control.expand(offset),
        theme::corner(radius + offset),
        Stroke::new(theme::spacing::FOCUS_RING_WIDTH, theme::AMBER_INK),
        StrokeKind::Outside,
    );
    let font = theme::SMALL.font_id();
    p.text(
        control.center(),
        Align2::CENTER_CENTER,
        "Control",
        font.clone(),
        theme::INK,
    );
    p.text(
        rect.left_top() + vec2(6.0, 4.0),
        Align2::LEFT_TOP,
        bg_name,
        font,
        theme::INK_SOFT,
    );
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

fn typography(ui: &mut Ui) {
    for (name, style) in theme::typography_tokens() {
        let spec = format!(
            "{name} {} {} {}",
            style.family.design_name(),
            style.size,
            style.weight
        );
        let sample = if style.size > 60.0 {
            "3".to_owned()
        } else {
            format!("Song of Storms, {spec}")
        };
        let text = if style.letter_spacing_em > 0.0 {
            sample.to_uppercase()
        } else {
            sample
        };
        ui.label(theme::rich(style, text).color(theme::TEXT));
        if style.size > 60.0 {
            ui.label(theme::rich(&theme::SMALL, spec).color(theme::TEXT_MUTED));
        }
    }
}
