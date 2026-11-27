//! The main screen (UX-DR5): five regions filled in reading order, each a
//! scoped `Ui` over its rect (never egui panels). Content is an empty
//! placeholder; widgets arrive with later stories.

use egui::{Rect, Stroke, Ui, UiBuilder, pos2};

use super::layout::{RegionId, regions};
use super::{paint, theme};

/// Draws the main screen over `ui.max_rect()`.
pub fn show(ui: &mut Ui) {
    let content = ui.max_rect();
    ui.painter()
        .rect_filled(content, theme::corner(0.0), theme::FRAME);
    let r = regions(content);
    for (id, rect) in r.in_reading_order() {
        let card = r.note_card;
        ui.scope_builder(UiBuilder::new().max_rect(rect).id_salt(id.name()), |ui| {
            paint_region(ui, id, rect, card)
        });
    }
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
