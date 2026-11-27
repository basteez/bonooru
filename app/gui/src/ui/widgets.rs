//! Themed widgets: the thin layer that turns a painting helper into one
//! accessible node (Story 4.3, Story 4.2 decision "basic", G.10). Each
//! widget calls `Response::widget_info` with the closest `egui::Role`; the
//! painting itself comes from `ui::paint` and `ui::symbols`.

use egui::{Align2, Color32, Rect, Response, Role, Sense, StrokeKind, Ui, WidgetInfo, vec2};

use super::paint;
use super::symbols::{self, Symbol};
use super::theme::{self, TypeStyle};

/// The look of a themed [`button`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonKind {
    /// `amber` fill, `ink` text, amber glow.
    Primary,
    /// `well` fill, 1 pt `rim` border, `text`.
    Ghost,
}

/// The size of a themed button with `label` (and a leading icon).
#[must_use]
pub fn button_size(ui: &Ui, label: &str, icon: bool) -> egui::Vec2 {
    use theme::components::modal_dialog::{ACTION_HEIGHT, ACTION_PADDING_X};
    let text = paint::layout_line(ui.ctx(), &theme::BUTTON_MD, label, Color32::PLACEHOLDER);
    let icon_w = if icon {
        theme::BUTTON_MD.size + theme::spacing::S2
    } else {
        0.0
    };
    vec2(
        text.rect.width() + icon_w + 2.0 * ACTION_PADDING_X,
        ACTION_HEIGHT,
    )
}

/// A themed dialog-size button (AC 10): 34 pt high, 18 pt side padding,
/// text in `button-md`, radius `rounded.lg`. Primary has the 12 pt amber
/// glow; disabled is drawn at `button-disabled` opacity with no glow and
/// is neither clickable nor focusable. Paints the focus ring and is one
/// `Role::Button` node named `label`.
pub fn button(
    ui: &mut Ui,
    kind: ButtonKind,
    label: &str,
    enabled: bool,
    icon: Option<Symbol>,
) -> Response {
    ui.add_enabled_ui(enabled, |ui| button_inner(ui, kind, label, enabled, icon))
        .inner
}

fn button_inner(
    ui: &mut Ui,
    kind: ButtonKind,
    label: &str,
    enabled: bool,
    icon: Option<Symbol>,
) -> Response {
    let size = button_size(ui, label, icon.is_some());
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        // A disabled `Ui` fades its painter by egui's `disabled_alpha`;
        // undo that so the token opacity below is the only fade.
        let mut faded = ui.painter().clone();
        if !ui.is_enabled() {
            let disabled = ui.visuals().disabled_alpha().max(f32::EPSILON);
            faded.set_opacity((faded.opacity() / disabled).min(1.0));
        }
        let painter = &faded;
        let ppp = ui.ctx().pixels_per_point();
        let alpha = if enabled {
            1.0
        } else {
            theme::components::BUTTON_DISABLED_OPACITY
        };
        let radius = theme::rounded::LG;
        let (fill, text_color, border) = match kind {
            ButtonKind::Primary => (theme::AMBER, theme::INK, None),
            ButtonKind::Ghost => (theme::WELL, theme::TEXT, Some(theme::RIM)),
        };
        if kind == ButtonKind::Primary && enabled {
            paint::paint_glow(
                painter,
                rect,
                radius,
                theme::glow::MODAL_PRIMARY,
                theme::AMBER_GLOW,
            );
        }
        painter.rect_filled(rect, theme::corner(radius), fill.gamma_multiply(alpha));
        if let Some(border) = border {
            painter.rect_stroke(
                rect,
                theme::corner(radius),
                paint::stroke(1.0, border.gamma_multiply(alpha), ppp),
                StrokeKind::Inside,
            );
        }
        let text_color = text_color.gamma_multiply(alpha);
        let galley = paint::layout_line(ui.ctx(), &theme::BUTTON_MD, label, text_color);
        let content_w = size.x - 2.0 * theme::components::modal_dialog::ACTION_PADDING_X;
        let mut x = rect.center().x - content_w / 2.0;
        if let Some(symbol) = icon {
            let side = theme::BUTTON_MD.size;
            let icon_rect = Rect::from_min_size(
                egui::pos2(x, rect.center().y - side / 2.0),
                vec2(side, side),
            );
            symbols::paint_symbol(painter, icon_rect, symbol, text_color, ppp);
            x += side + theme::spacing::S2;
        }
        let y = rect.center().y - galley.rect.height() / 2.0;
        painter.galley(egui::pos2(x, y), galley, text_color);
    }
    paint::paint_focus_ring(&resp, theme::rounded::LG, false);
    resp.widget_info(|| WidgetInfo::labeled(Role::Button, enabled, label));
    resp
}

/// A standalone painted symbol (AC 8): `size` × `size`, one `Role::Image`
/// node named `name` in plain words. A name containing one of the 12 symbol
/// characters is a bug (debug assertion): a screen reader would read the
/// glyph, not its meaning.
pub fn symbol(ui: &mut Ui, symbol: Symbol, size: f32, color: Color32, name: &str) -> Response {
    debug_assert!(
        !symbols::has_glyph(name),
        "symbol name `{name}` must be plain words, not a glyph"
    );
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    if ui.is_rect_visible(rect) {
        symbols::paint_symbol(
            ui.painter(),
            rect,
            symbol,
            color,
            ui.ctx().pixels_per_point(),
        );
    }
    resp.widget_info(|| WidgetInfo::labeled(Role::Image, true, name));
    resp
}

/// A time (or any short numeric text) in fixed-width digit cells (AC 7):
/// one `Role::Label` node named `text`, painted in `text` colour.
pub fn time(ui: &mut Ui, text: &str, style: &TypeStyle) -> Response {
    let size = paint::digits_size(ui.ctx(), style, text);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    if ui.is_rect_visible(rect) {
        paint::paint_digits(ui.painter(), rect.min, text, style, theme::TEXT);
    }
    resp.widget_info(|| WidgetInfo::labeled(Role::Label, true, text));
    resp
}

/// Text painted in a box of its own size, one `Role::Label` node named with
/// the text. `galley` is laid out by the caller (e.g. centred with
/// [`paint::layout`]); `rect` is where its box goes.
pub fn text_node(
    ui: &mut Ui,
    rect: Rect,
    galley: std::sync::Arc<egui::Galley>,
    align: Align2,
) -> Response {
    let resp = ui.allocate_rect(rect, Sense::hover());
    let text = galley.text().to_owned();
    let pos =
        align.align_size_within_rect(galley.rect.size(), rect).min - galley.rect.min.to_vec2();
    ui.painter().galley(pos, galley, theme::TEXT);
    resp.widget_info(|| WidgetInfo::labeled(Role::Label, true, &text));
    resp
}
