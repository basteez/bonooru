//! The main screen (UX-DR5): five regions filled in reading order, each a
//! scoped `Ui` over its rect (never egui panels). The status strip shows
//! the brand and the three LEDs (Story 4.5); the gamepad region shows the
//! gate's inline hint while the gamepad is blocked (Story 4.6); the bottom
//! bar shows STOP and the safety note, its Play/Listen slots reserved and
//! empty (Story 4.7); the other regions are empty placeholders until later
//! stories.

use egui::{Rect, Stroke, Ui, UiBuilder, pos2};

use super::layout::{RegionId, regions};
use super::{bottom_bar, paint, status_strip, theme, widgets};
use crate::gating::Gate;
use crate::leds::Leds;

/// Draws the main screen over `ui.max_rect()`, the status strip showing
/// `leds` and the gamepad region the hint of `gate.gamepad`, if blocked.
/// The status region comes first, so its LEDs are first in the tab order;
/// the bottom bar comes last, so STOP is last.
pub fn show(ui: &mut Ui, leds: &Leds, gate: &Gate) -> MainOutput {
    let mut out = MainOutput::default();
    let content = ui.max_rect();
    ui.painter()
        .rect_filled(content, theme::corner(0.0), theme::FRAME);
    let r = regions(content);
    for (id, rect) in r.in_reading_order() {
        let card = r.note_card;
        ui.scope_builder(UiBuilder::new().max_rect(rect).id_salt(id.name()), |ui| {
            paint_region(ui, id, rect, card);
            if id == RegionId::Status {
                status_strip::show(ui, rect, leds);
            }
            if id == RegionId::Gamepad
                && let Some(block) = gate.gamepad
            {
                widgets::inline_hint(ui, hint_rect(rect), block.gamepad_hint());
            }
            if id == RegionId::Bottom {
                out.stop_clicked = bottom_bar::show(ui, rect).stop_clicked;
            }
        });
    }
    out
}

/// What the main screen reports for the frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MainOutput {
    /// STOP was activated (click, Enter with focus, AccessKit click).
    pub stop_clicked: bool,
}

/// Where the gamepad hint goes in the gamepad region `rect`: inset by
/// `components.inline-hint.inset` (10 pt, the mock's panel padding) from
/// the left, right and top edges, down to the region's bottom. Story 4.8
/// moves the hint under the pad.
#[must_use]
pub fn hint_rect(rect: Rect) -> Rect {
    use theme::components::inline_hint::INSET;
    Rect::from_min_max(
        pos2(rect.left() + INSET, rect.top() + INSET),
        pos2(rect.right() - INSET, rect.bottom()),
    )
}

/// A 1 pt bottom hairline snapped to physical pixels: returns the line's
/// centre `y` and its width in points. The width is a whole number of
/// pixels (`round(ppp)`, at least 1) and the line ends exactly on the pixel
/// boundary nearest to `bottom`, so both of its edges fall on pixel
/// boundaries.
#[must_use]
pub fn hairline(bottom: f32, ppp: f32) -> (f32, f32) {
    let px = ppp.round().max(1.0);
    let bottom_px = (bottom * ppp).round();
    ((bottom_px - px / 2.0) / ppp, px / ppp)
}

fn paint_region(ui: &Ui, id: RegionId, rect: Rect, card: Rect) {
    let painter = ui.painter();
    let square = theme::corner(0.0);
    match id {
        RegionId::Status => {
            painter.rect_filled(rect, square, theme::PANEL_RAISED);
            // components.status-strip: 1 pt rim-soft bottom hairline, inside
            // the strip, snapped to whole physical pixels (crisp at any ppp).
            let (y, width) = hairline(rect.bottom(), ui.ctx().pixels_per_point());
            painter.line_segment(
                [pos2(rect.left(), y), pos2(rect.right(), y)],
                Stroke::new(width, theme::RIM_SOFT),
            );
        }
        RegionId::Bottom => {
            painter.rect_filled(rect, square, theme::PANEL_RAISED);
        }
        RegionId::Song | RegionId::Gamepad => {
            painter.rect_filled(rect, square, theme::PANEL);
        }
        RegionId::NoteView => {
            painter.rect_filled(rect, square, theme::FRAME);
            painter.rect_filled(card, theme::corner(theme::rounded::XL), theme::PARCHMENT);
            // UX-DR4: the card's four corner brackets (Story 4.3).
            paint::paint_corner_brackets(
                painter,
                card,
                theme::BRACKET,
                ui.ctx().pixels_per_point(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::hairline;

    #[test]
    fn hairline_edges_fall_on_pixel_boundaries() {
        for ppp in [1.0_f32, 1.25, 1.5, 1.75, 2.0, 3.0] {
            for bottom in [44.0_f32, 44.3, 100.5] {
                let (y, w) = hairline(bottom, ppp);
                for edge in [(y - w / 2.0) * ppp, (y + w / 2.0) * ppp] {
                    assert!(
                        (edge - edge.round()).abs() < 1e-3,
                        "ppp {ppp}, bottom {bottom}: edge at {edge} px"
                    );
                }
                assert!(w * ppp >= 1.0 - 1e-6, "at least one pixel wide");
                assert!(
                    (y + w / 2.0 - bottom).abs() * ppp <= 0.5 + 1e-3,
                    "ends at the strip bottom"
                );
            }
        }
    }
}
