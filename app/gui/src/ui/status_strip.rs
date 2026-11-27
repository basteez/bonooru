//! The status strip's content (Story 4.5; UX-DR8, UX-DR9, UX-DR62): the
//! brand, then the three status LEDs left to right. The strip background
//! and its hairline are painted by `main_screen`; the LED decisions come
//! from `crate::leds` (pure data), this module only draws them.
//!
//! The gear (Settings) arrives with Epic 5 and the "Set up / Reflash…" link
//! with Epic 8: neither is drawn here, not even disabled, so Tab and the
//! screen reader never reach a dead control (Story 4.5 D9).

use egui::containers::{Popup, PopupKind, Tooltip};
use egui::{Align2, Painter, Pos2, Rect, Response, Role, Sense, Ui, WidgetInfo, pos2, vec2};

use super::theme::components::{STATUS_LED_SIZE, status_led, status_strip};
use super::theme::{self, TypeStyle};
use super::{paint, widgets};
use crate::leds::{Led, LedState, Leds};

/// The brand text (`typography.brand`, `parchment`).
pub const BRAND_NAME: &str = "Bonooru";

/// The LED detail style: `typography.body` on one line (a single line has
/// no line height to apply, so label and detail share a baseline).
const DETAIL: TypeStyle = TypeStyle {
    line_height: None,
    ..theme::BODY
};

/// Draws the brand and the three LEDs in the strip `rect` (AC 1–3, 11).
pub fn show(ui: &mut Ui, rect: Rect, leds: &Leds) {
    let ctx = ui.ctx().clone();
    let brand = paint::layout_line(&ctx, &theme::BRAND, BRAND_NAME, theme::PARCHMENT);
    let size = brand.rect.size();
    let brand_rect = Rect::from_min_size(
        pos2(
            rect.left() + status_strip::PADDING_X,
            rect.center().y - size.y / 2.0,
        ),
        size,
    );
    let _ =
        widgets::text_node_colored(ui, brand_rect, brand, Align2::LEFT_CENTER, theme::PARCHMENT);
    let mut x = brand_rect.right() + status_strip::BRAND_GAP;
    for (link, led) in leds.all() {
        let resp = led_widget(ui, pos2(x, rect.center().y), led, link.tag());
        x = resp.rect.right() + status_led::GAP;
    }
}

/// The widget id of the LED whose `led` log tag is `tag` (stable, so tests
/// can check the focus order).
#[must_use]
pub fn led_id(tag: &str) -> egui::Id {
    egui::Id::unique(("status-led", tag))
}

/// The width of a space between label and detail, in `body-strong`
/// (measured: a lone space galley has no width).
fn space_width(ui: &Ui) -> f32 {
    let w = |t: &str| {
        paint::layout_line(ui.ctx(), &theme::BODY_STRONG, t, theme::TEXT)
            .rect
            .width()
    };
    w("a a") - w("aa")
}

/// One LED at `left_center` (its box's left edge, vertically centred):
/// symbol, label, detail, 2/4 pt padding; focusable for its tooltip only
/// (`Sense::focusable_noninteractive`: Enter does nothing, D10).
fn led_widget(ui: &mut Ui, left_center: Pos2, led: &Led, salt: &str) -> Response {
    let ctx = ui.ctx().clone();
    let label = paint::layout_line(&ctx, &theme::BODY_STRONG, led.label, theme::TEXT);
    let detail = led
        .detail
        .as_deref()
        .map(|d| paint::layout_line(&ctx, &DETAIL, d, theme::TEXT_MUTED));
    let space = space_width(ui);
    let text_w = label.rect.width() + detail.as_ref().map_or(0.0, |d| space + d.rect.width());
    let content_h = STATUS_LED_SIZE
        .max(label.rect.height())
        .max(detail.as_ref().map_or(0.0, |d| d.rect.height()));
    let size = vec2(
        2.0 * status_led::PADDING_X + STATUS_LED_SIZE + status_led::SYMBOL_GAP + text_w,
        2.0 * status_led::PADDING_Y + content_h,
    );
    let rect = Rect::from_min_size(pos2(left_center.x, left_center.y - size.y / 2.0), size);
    let resp = ui.interact(rect, led_id(salt), Sense::focusable_noninteractive());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let cy = rect.center().y;
        let symbol = Rect::from_min_size(
            pos2(
                rect.left() + status_led::PADDING_X,
                cy - STATUS_LED_SIZE / 2.0,
            ),
            vec2(STATUS_LED_SIZE, STATUS_LED_SIZE),
        );
        paint_led_symbol(painter, symbol, led.state, ctx.pixels_per_point());
        let mut x = symbol.right() + status_led::SYMBOL_GAP;
        let label_w = label.rect.width();
        let label_h = label.rect.height();
        painter.galley(pos2(x, cy - label_h / 2.0), label, theme::TEXT);
        x += label_w + space;
        if let Some(d) = detail {
            let h = d.rect.height();
            painter.galley(pos2(x, cy - h / 2.0), d, theme::TEXT_MUTED);
        }
    }
    paint::paint_focus_ring(ui, &resp, theme::rounded::SM, false);
    let name = led.name();
    resp.widget_info(|| WidgetInfo::labeled(Role::Status, true, &name));
    let description = led.tooltip_text();
    let _ = ctx.accesskit_node_builder(resp.id, |n| n.set_description(description));
    show_tooltip(&resp, led);
    resp
}

/// The tooltip (AC 3): at once while the LED has keyboard focus, on hover
/// (egui's delay) otherwise; never both, so one tooltip per LED.
fn show_tooltip(resp: &Response, led: &Led) {
    let contents = |ui: &mut Ui| tooltip_contents(ui, led);
    if resp.has_focus() {
        let _ = Popup::from_response(resp)
            .kind(PopupKind::Tooltip)
            .frame(theme::tooltip_frame())
            .show(contents);
    } else {
        let mut tip = Tooltip::for_enabled(resp);
        tip.popup = tip.popup.frame(theme::tooltip_frame());
        let _ = tip.show(contents);
    }
}

/// The detail line, then each fix on its own line, in `body` / `text`.
fn tooltip_contents(ui: &mut Ui, led: &Led) {
    ui.set_max_width(ui.spacing().tooltip_width);
    ui.spacing_mut().item_spacing.y = theme::components::TOOLTIP_FIX_GAP;
    for line in &led.tooltip {
        ui.label(theme::rich(&theme::BODY, line).color(theme::TEXT));
    }
}

/// Mock geometry of the LED symbols in an 11 × 11 box
/// (`mockups/key-empty-state.html`).
pub mod geometry {
    /// Warning triangle corners.
    pub const TRIANGLE: [(f32, f32); 3] = [(5.5, 0.6), (10.8, 10.2), (0.2, 10.2)];
    /// Error cross: two segments.
    pub const CROSS: [[(f32, f32); 2]; 2] = [[(1.6, 1.6), (9.4, 9.4)], [(9.4, 1.6), (1.6, 9.4)]];
    /// Error cross stroke width (round caps).
    pub const CROSS_WIDTH: f32 = 2.2;
    /// Unknown ring radius.
    pub const RING_RADIUS: f32 = 4.4;
    /// Unknown ring stroke width.
    pub const RING_WIDTH: f32 = 1.6;
}

/// Paints one LED symbol in its 11 × 11 `rect` (AC 2): ok = filled dot
/// with the 8 px glow of the same hue; warning = convex filled triangle;
/// error = a cross of two round-capped strokes; unknown = a hollow ring.
/// Strokes go through [`paint::stroke`] (1 physical px floor).
pub fn paint_led_symbol(painter: &Painter, rect: Rect, state: LedState, ppp: f32) {
    let at = |(x, y): (f32, f32)| {
        pos2(
            rect.left() + x * rect.width() / STATUS_LED_SIZE,
            rect.top() + y * rect.height() / STATUS_LED_SIZE,
        )
    };
    let scale = rect.width() / STATUS_LED_SIZE;
    match state {
        LedState::Ok => {
            paint::paint_glow(
                painter,
                rect,
                rect.width() / 2.0,
                theme::glow::STATUS_LED,
                theme::STATUS_OK,
            );
            painter.circle_filled(rect.center(), rect.width() / 2.0, theme::STATUS_OK);
        }
        LedState::Warning => {
            let pts = geometry::TRIANGLE.map(at).to_vec();
            // Three points are always convex unless degenerate.
            let _ = paint::fill_convex(painter, pts, theme::STATUS_WARNING, egui::Stroke::NONE);
        }
        LedState::Error => {
            let stroke = paint::stroke(geometry::CROSS_WIDTH * scale, theme::STATUS_ERROR, ppp);
            for [a, b] in geometry::CROSS {
                // Round caps: a dot of the stroke's width at each end.
                painter.line_segment([at(a), at(b)], stroke);
                for p in [at(a), at(b)] {
                    painter.circle_filled(p, stroke.width / 2.0, stroke.color);
                }
            }
        }
        LedState::Unknown => {
            painter.circle_stroke(
                rect.center(),
                geometry::RING_RADIUS * scale,
                paint::stroke(geometry::RING_WIDTH * scale, theme::STATUS_UNKNOWN, ppp),
            );
        }
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::{LedState, geometry, paint_led_symbol};
    use egui::epaint::Shape;
    use egui::{Context, Rect, pos2, vec2};

    fn shapes(state: LedState, ppp: f32) -> Vec<Shape> {
        fn walk(s: &Shape, out: &mut Vec<Shape>) {
            match s {
                Shape::Vec(v) => v.iter().for_each(|x| walk(x, out)),
                other => out.push(other.clone()),
            }
        }
        let ctx = Context::default();
        let mut full = ctx.run_ui(egui::RawInput::default(), |ui| {
            paint_led_symbol(
                ui.painter(),
                Rect::from_min_size(pos2(10.0, 20.0), vec2(11.0, 11.0)),
                state,
                ppp,
            );
        });
        full.textures_delta.clear();
        let mut out = Vec::new();
        for c in &full.shapes {
            walk(&c.shape, &mut out);
        }
        out
    }

    #[test]
    fn led_symbols_follow_the_mock() {
        let b = Rect::from_min_size(pos2(10.0, 20.0), vec2(11.0, 11.0));
        // ok: glow (a rect shadow) then the dot filling the box.
        let s = shapes(LedState::Ok, 1.0);
        assert_eq!(s.len(), 2, "{s:?}");
        assert!(matches!(s[0], Shape::Rect(_)));
        match &s[1] {
            Shape::Circle(c) => {
                assert_eq!((c.center, c.radius), (b.center(), 5.5));
            }
            other => panic!("{other:?}"),
        }
        // warning: one convex path through the mock's points.
        let s = shapes(LedState::Warning, 1.0);
        match s.as_slice() {
            [Shape::Path(p)] => {
                assert!(p.closed);
                assert_eq!(p.points.len(), 3);
                for (q, (x, y)) in p.points.iter().zip(geometry::TRIANGLE) {
                    assert!((q.x - (10.0 + x)).abs() < 1e-4 && (q.y - (20.0 + y)).abs() < 1e-4);
                }
            }
            other => panic!("{other:?}"),
        }
        // error: 2 segments of width 2.2 (+ round caps).
        let s = shapes(LedState::Error, 1.0);
        let segs: Vec<_> = s
            .iter()
            .filter_map(|x| match x {
                Shape::LineSegment { stroke, .. } => Some(stroke.width),
                _ => None,
            })
            .collect();
        assert_eq!(segs, vec![2.2, 2.2]);
        // unknown: one circle stroke, no fill; floored at 1 px at ppp 2.
        for ppp in [1.0, 2.0] {
            let s = shapes(LedState::Unknown, ppp);
            match s.as_slice() {
                [Shape::Circle(c)] => {
                    assert_eq!(c.fill, egui::Color32::TRANSPARENT);
                    assert_eq!((c.radius, c.stroke.width), (4.4, 1.6));
                }
                other => panic!("{other:?}"),
            }
        }
    }
}
