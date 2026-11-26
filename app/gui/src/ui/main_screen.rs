//! The main screen (UX-DR5): five regions filled in reading order, each a
//! scoped `Ui` over its rect (never egui panels). Content is an empty
//! placeholder; widgets arrive with later stories.

use egui::{Rect, Stroke, Ui, UiBuilder, pos2};

use super::layout::{RegionId, regions};
use super::theme;

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

fn paint_region(ui: &Ui, id: RegionId, rect: Rect, card: Rect) {
    let painter = ui.painter();
    let square = theme::corner(0.0);
    match id {
        RegionId::Status => {
            painter.rect_filled(rect, square, theme::PANEL_RAISED);
            // components.status-strip: 1 pt rim-soft bottom hairline, inside
            // the strip.
            let y = rect.bottom() - 0.5;
            painter.line_segment(
                [pos2(rect.left(), y), pos2(rect.right(), y)],
                Stroke::new(1.0, theme::RIM_SOFT),
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
        }
    }
}
