//! Themed widgets: the thin layer that turns a painting helper into one
//! accessible node (Story 4.3, Story 4.2 decision "basic", G.10). Each
//! widget calls `Response::widget_info` with the closest `egui::Role`; the
//! painting itself comes from `ui::paint` and `ui::symbols`.

use egui::{Align, Align2, Color32, Rect, Response, Role, Sense, StrokeKind, Ui, WidgetInfo, vec2};

use super::paint::{self, ButtonState};
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
/// glow; disabled (primary or ghost alike, owner decision 2026-11-27) is
/// neutral: `button-disabled` fill, 1 pt neutral border, label in
/// `button-disabled.foreground`, no glow, no amber; it is neither clickable
/// nor focusable. Enabled, it shows the pointer states of
/// `components.state-layer` (hover lightens, pressed darkens the fill and
/// border; label, glow and ring unchanged); disabled shows none. Paints the
/// focus ring and is one `Role::Button` node named `label`.
///
/// Disabled means `enabled == false` or a disabled parent `Ui`: the look,
/// the sense and the AccessKit state all follow that one effective state,
/// and the token opacity is the only fade (egui's own `disabled_alpha` fade
/// of a disabled parent is undone).
pub fn button(
    ui: &mut Ui,
    kind: ButtonKind,
    label: &str,
    enabled: bool,
    icon: Option<Symbol>,
) -> Response {
    let enabled = enabled && ui.is_enabled();
    let size = button_size(ui, label, icon.is_some());
    // A disabled button senses nothing: not clickable, not focusable. No
    // `add_enabled_ui` here, so egui adds no fade of its own.
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(size, sense);
    if ui.is_rect_visible(rect) {
        // A disabled parent `Ui` has faded its painter by egui's
        // `disabled_alpha` (once per `Ui::disable`, i.e. per disabled
        // scope); undo that so the token opacity below is the only fade.
        let mut faded = ui.painter().clone();
        if !ui.is_enabled() {
            let disabled = ui.visuals().disabled_alpha().max(f32::EPSILON);
            faded.set_opacity((faded.opacity() / disabled).min(1.0));
        }
        let state = ButtonState::of(&resp, enabled);
        paint_button(ui, &faded, rect, kind, label, enabled, icon, state);
    }
    paint::paint_focus_ring(ui, &resp, theme::rounded::LG, false);
    resp.widget_info(|| WidgetInfo::labeled(Role::Button, enabled, label));
    resp
}

/// A [`button`]'s look in a given pointer state, painted only (no sense,
/// no node, never focusable): the gallery's state samples (Story 4.7).
pub fn button_preview(
    ui: &mut Ui,
    kind: ButtonKind,
    label: &str,
    enabled: bool,
    state: ButtonState,
) -> Rect {
    let size = button_size(ui, label, false);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter().clone();
    let state = if enabled { state } else { ButtonState::Idle };
    paint_button(ui, &painter, rect, kind, label, enabled, None, state);
    rect
}

#[allow(clippy::too_many_arguments)]
fn paint_button(
    ui: &Ui,
    painter: &egui::Painter,
    rect: Rect,
    kind: ButtonKind,
    label: &str,
    enabled: bool,
    icon: Option<Symbol>,
    state: ButtonState,
) {
    let ppp = ui.ctx().pixels_per_point();
    use theme::components::{
        BUTTON_DISABLED_BORDER, BUTTON_DISABLED_BORDER_WIDTH, BUTTON_DISABLED_FILL,
        BUTTON_DISABLED_TEXT,
    };
    let radius = theme::rounded::LG;
    // Disabled: one neutral look for both kinds (no amber, no glow, no
    // hover or pressed change).
    let (fill, text_color, border) = match (kind, enabled) {
        (ButtonKind::Primary, true) => (theme::AMBER, theme::INK, None),
        (ButtonKind::Ghost, true) => (theme::WELL, theme::TEXT, Some((1.0, theme::RIM))),
        (_, false) => (
            BUTTON_DISABLED_FILL,
            BUTTON_DISABLED_TEXT,
            Some((BUTTON_DISABLED_BORDER_WIDTH, BUTTON_DISABLED_BORDER)),
        ),
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
    painter.rect_filled(rect, theme::corner(radius), state.tint(fill));
    if let Some((width, color)) = border {
        painter.rect_stroke(
            rect,
            theme::corner(radius),
            paint::stroke(width, state.tint(color), ppp),
            StrokeKind::Inside,
        );
    }
    let galley = paint::layout_line(ui.ctx(), &theme::BUTTON_MD, label, text_color);
    let content_w = rect.width() - 2.0 * theme::components::modal_dialog::ACTION_PADDING_X;
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
    text_node_colored(ui, rect, galley, align, theme::TEXT)
}

/// [`text_node`] whose galley's placeholder colour (sections laid out with
/// `Color32::PLACEHOLDER`) paints in `color`, e.g. the `parchment` brand
/// of the status strip (Story 4.5).
pub fn text_node_colored(
    ui: &mut Ui,
    rect: Rect,
    galley: std::sync::Arc<egui::Galley>,
    align: Align2,
    color: Color32,
) -> Response {
    let resp = ui.allocate_rect(rect, Sense::hover());
    let text = galley.text().to_owned();
    let pos =
        align.align_size_within_rect(galley.rect.size(), rect).min - galley.rect.min.to_vec2();
    ui.painter().galley(pos, galley, color);
    resp.widget_info(|| WidgetInfo::labeled(Role::Label, true, &text));
    resp
}

/// An `inline-hint` (Story 4.6, UX-DR38): `text` in `typography.small`,
/// `text-muted`, left-aligned at `rect.min` and wrapped to `rect`'s width.
/// One `Role::Label` node named with the text, not focusable. Story 4.8
/// reuses it for the gamepad's other hints.
pub fn inline_hint(ui: &mut Ui, rect: Rect, text: &str) -> Response {
    let galley = paint::layout(
        ui.ctx(),
        &theme::SMALL,
        text,
        theme::TEXT_MUTED,
        rect.width(),
        Align::Min,
    );
    let box_rect = Rect::from_min_size(rect.min, vec2(rect.width(), galley.rect.height()));
    text_node_colored(ui, box_rect, galley, Align2::LEFT_TOP, theme::TEXT_MUTED)
}

/// STOP, idle look (`button-stop-idle`, Story 4.7): `stop-idle-fill`, 1 pt
/// `ember` border, radius `rounded.xl`, the painted ■ (16 pt box, no node)
/// at 22 pt from the left, 12 pt gap, then "STOP" (`button-lg`) over "Esc"
/// (`button-lg-sub` at 85 %), the block vertically centred; everything at
/// 75 % opacity, no glow; hover and pressed tint the fill and border
/// (`components.state-layer`, before the 75 %). Always enabled; focusable, clicked by a click,
/// Enter with focus or an AccessKit click (Space never reaches it: the
/// frame start consumes it, `ui::keys`). The focus ring is painted at full
/// opacity. One `Role::Button` node "Stop" with the description "Esc stops
/// at any time.". Epic 6 adds the live look.
pub fn stop_button(ui: &mut Ui, rect: Rect) -> Response {
    use crate::stop::{STOP_A11Y_DESCRIPTION, STOP_A11Y_NAME};
    let resp = ui.interact(rect, super::bottom_bar::stop_id(), Sense::click());
    if ui.is_rect_visible(rect) {
        paint_stop(ui, rect, ButtonState::of(&resp, true));
    }
    paint::paint_focus_ring(ui, &resp, theme::rounded::XL, false);
    resp.widget_info(|| WidgetInfo::labeled(Role::Button, true, STOP_A11Y_NAME));
    ui.ctx()
        .accesskit_node_builder(resp.id, |n| n.set_description(STOP_A11Y_DESCRIPTION));
    resp
}

/// STOP's idle look in a given pointer state over `rect`, painted only (no
/// sense, no node, never focusable): the gallery's state samples.
pub fn stop_button_preview(ui: &mut Ui, rect: Rect, state: ButtonState) {
    paint_stop(ui, rect, state);
}

/// STOP's idle look (`button-stop-idle`) in `state`: fill and border tinted
/// by the state layer, then everything at 75 %.
fn paint_stop(ui: &Ui, rect: Rect, state: ButtonState) {
    use crate::stop::{STOP_LABEL, STOP_SUB};
    use theme::components::big_button::{ICON_GAP, ICON_SIZE, PADDING_X, SUB_OPACITY};
    let mut faded = ui.painter().clone();
    faded.multiply_opacity(theme::components::STOP_IDLE_OPACITY);
    let painter = &faded;
    let ppp = ui.ctx().pixels_per_point();
    let radius = theme::corner(theme::rounded::XL);
    painter.rect_filled(rect, radius, state.tint(theme::STOP_IDLE_FILL));
    painter.rect_stroke(
        rect,
        radius,
        paint::stroke(1.0, state.tint(theme::EMBER), ppp),
        StrokeKind::Inside,
    );
    let icon = Rect::from_min_size(
        egui::pos2(rect.left() + PADDING_X, rect.center().y - ICON_SIZE / 2.0),
        vec2(ICON_SIZE, ICON_SIZE),
    );
    symbols::paint_symbol(painter, icon, Symbol::Stop, theme::STOP_IDLE_TEXT, ppp);
    let sub_color = theme::STOP_IDLE_TEXT.gamma_multiply(SUB_OPACITY);
    let label = paint::layout_line(
        ui.ctx(),
        &theme::BUTTON_LG,
        STOP_LABEL,
        theme::STOP_IDLE_TEXT,
    );
    let sub = paint::layout_line(ui.ctx(), &theme::BUTTON_LG_SUB, STOP_SUB, sub_color);
    let x = icon.right() + ICON_GAP;
    let block_h = label.rect.height() + sub.rect.height();
    let top = rect.center().y - block_h / 2.0;
    let label_h = label.rect.height();
    painter.galley(egui::pos2(x, top), label, theme::STOP_IDLE_TEXT);
    painter.galley(egui::pos2(x, top + label_h), sub, sub_color);
}
