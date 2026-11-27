//! The bottom bar in its Epic 4 form (Story 4.7, UX-DR39): three reserved
//! slots for Play from cursor, Play from start and Listen (empty until
//! Epics 5–6: nothing drawn, no AccessKit node, nothing focusable), the
//! two-line safety note, and STOP at the right edge (`button-stop-idle`,
//! always enabled). Laid out left to right with `components.bottom-bar`
//! padding and gaps, every element vertically centred.

use egui::{Align, Align2, Context, Id, Rect, Ui, pos2, vec2};

use super::{paint, theme, widgets};
use crate::stop;

/// The rects of the bar's elements, in points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarLayout {
    /// Reserved: Play from cursor (Epic 6).
    pub play_cursor: Rect,
    /// Reserved: Play from start (Epic 6).
    pub play_start: Rect,
    /// Reserved: Listen (Epic 5).
    pub listen: Rect,
    /// The safety note's area (full bar height; the text is centred in it).
    pub safety: Rect,
    /// STOP.
    pub stop: Rect,
}

/// The bar's geometry (pure, no `Ui`): the slots from the left at
/// `16 + i × (190 + 14)`, STOP from the right (`bar.right() − 16 −
/// stop_width`), all `big-button-height` high and vertically centred; the
/// safety note between the last slot and STOP, 14 pt from each.
#[must_use]
pub fn layout(bar: Rect, stop_width: f32) -> BarLayout {
    use theme::components::bottom_bar::{GAP, PADDING_X};
    use theme::spacing::{BIG_BUTTON_HEIGHT, BIG_BUTTON_MIN_WIDTH};
    let top = bar.center().y - BIG_BUTTON_HEIGHT / 2.0;
    let slot = |i: f32| {
        Rect::from_min_size(
            pos2(
                bar.left() + PADDING_X + i * (BIG_BUTTON_MIN_WIDTH + GAP),
                top,
            ),
            vec2(BIG_BUTTON_MIN_WIDTH, BIG_BUTTON_HEIGHT),
        )
    };
    let (play_cursor, play_start, listen) = (slot(0.0), slot(1.0), slot(2.0));
    let stop = Rect::from_min_size(
        pos2(bar.right() - PADDING_X - stop_width, top),
        vec2(stop_width, BIG_BUTTON_HEIGHT),
    );
    let safety = Rect::from_min_max(
        pos2(listen.right() + GAP, bar.top()),
        pos2((stop.left() - GAP).max(listen.right() + GAP), bar.bottom()),
    );
    BarLayout {
        play_cursor,
        play_start,
        listen,
        safety,
        stop,
    }
}

/// STOP's width: its content (22 pt padding, the 16 pt symbol, the 12 pt
/// gap, the wider of "STOP" and "Esc", 22 pt padding), at least
/// `big-button-min-width`.
#[must_use]
pub fn stop_width(ctx: &Context) -> f32 {
    use theme::components::big_button::{ICON_GAP, ICON_SIZE, PADDING_X};
    let w = |style, text| {
        paint::layout_line(ctx, style, text, theme::TEXT)
            .rect
            .width()
    };
    let text = w(&theme::BUTTON_LG, stop::STOP_LABEL).max(w(&theme::BUTTON_LG_SUB, stop::STOP_SUB));
    (PADDING_X + ICON_SIZE + ICON_GAP + text + PADDING_X).max(theme::spacing::BIG_BUTTON_MIN_WIDTH)
}

/// STOP's stable id (G.19/G.20 find and focus it by id).
#[must_use]
pub fn stop_id() -> Id {
    Id::unique("bottom-bar-stop")
}

/// What the bar reports for the frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BottomBarOutput {
    /// STOP was activated (click, Enter with focus, AccessKit click).
    pub stop_clicked: bool,
}

/// Draws the bar's content over `bar` (the region background is the
/// caller's): nothing in the slots, the safety note, STOP.
pub fn show(ui: &mut Ui, bar: Rect) -> BottomBarOutput {
    let l = layout(bar, stop_width(ui.ctx()));
    safety_note(ui, l.safety);
    let resp = widgets::stop_button(ui, l.stop);
    BottomBarOutput {
        stop_clicked: resp.clicked(),
    }
}

/// The two lines, right-aligned 6 pt inside `area`'s right edge, wrapped to
/// its width, the pair centred vertically; one `Role::Label` node each.
fn safety_note(ui: &mut Ui, area: Rect) {
    use theme::components::safety_note::{PADDING_RIGHT, SECOND_LINE_OPACITY};
    let width = (area.width() - PADDING_RIGHT).max(0.0);
    let second = theme::TEXT_MUTED.gamma_multiply(SECOND_LINE_OPACITY);
    let g1 = paint::layout(
        ui.ctx(),
        &theme::SAFETY_NOTE,
        stop::SAFETY_LINE_1,
        theme::TEXT_MUTED,
        width,
        Align::Max,
    );
    let g2 = paint::layout(
        ui.ctx(),
        &theme::SAFETY_NOTE,
        stop::SAFETY_LINE_2,
        second,
        width,
        Align::Max,
    );
    let (h1, h2) = (g1.rect.height(), g2.rect.height());
    let top = area.center().y - (h1 + h2) / 2.0;
    let right = area.right() - PADDING_RIGHT;
    let r1 = Rect::from_min_max(pos2(right - width, top), pos2(right, top + h1));
    let r2 = Rect::from_min_max(pos2(right - width, top + h1), pos2(right, top + h1 + h2));
    widgets::text_node_colored(ui, r1, g1, Align2::RIGHT_TOP, theme::TEXT_MUTED);
    widgets::text_node_colored(ui, r2, g2, Align2::RIGHT_TOP, second);
}
