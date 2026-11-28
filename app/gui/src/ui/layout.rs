//! Main-screen regions (UX-DR5): a pure function of the content rect, in
//! points, so the result does not depend on `pixels_per_point`.

use egui::{Rect, pos2};

use super::theme::spacing;

/// The five regions of the main screen, in reading order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RegionId {
    Status,
    Song,
    NoteView,
    Gamepad,
    Bottom,
}

impl RegionId {
    /// A stable name, used as the region's id salt.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Status => "status-strip",
            Self::Song => "song-panel",
            Self::NoteView => "note-view",
            Self::Gamepad => "gamepad-panel",
            Self::Bottom => "bottom-bar",
        }
    }
}

/// The region rects of the main screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Regions {
    pub status: Rect,
    pub song: Rect,
    pub note_view: Rect,
    /// The parchment card: `note_view` inset by `card-inset` on each side.
    pub note_card: Rect,
    pub gamepad: Rect,
    pub bottom: Rect,
}

/// Below this content width (logical points) the gamepad panel is the
/// compact variant, `spacing.gamepad-compact-width` wide (UX-DR6: the window
/// between 1024 and 1279 pt; Story 4.9).
pub const COMPACT_BELOW: f32 = 1280.0;

/// Whether a content rect this wide gets the compact gamepad (the one test
/// both [`regions`] and the panel use, so they never disagree).
#[must_use]
pub fn is_compact(content: Rect) -> bool {
    content.width() < COMPACT_BELOW
}

/// Computes the regions from the content rect: status strip on top, bottom
/// bar at the bottom, song panel left, gamepad panel right, note view in the
/// rest. Sizes are clamped so no rect turns negative in a tiny window.
#[must_use]
pub fn regions(content: Rect) -> Regions {
    let (l, r, t, b) = (
        content.left(),
        content.right(),
        content.top(),
        content.bottom(),
    );
    let mid_top = (t + spacing::STATUS_STRIP_HEIGHT).min(b);
    let mid_bottom = (b - spacing::BOTTOM_BAR_HEIGHT).max(mid_top);
    let song_right = (l + spacing::SONG_PANEL_WIDTH).min(r);
    // 300 pt from 1280 pt up, the 200 pt compact variant below (Story 4.9,
    // UX-DR6); the collapsed song rail is Story 5.12.
    let gamepad_width = if is_compact(content) {
        spacing::GAMEPAD_COMPACT_WIDTH
    } else {
        spacing::GAMEPAD_PANEL_WIDTH
    };
    let gamepad_left = (r - gamepad_width).max(song_right);

    let note_view = Rect::from_min_max(pos2(song_right, mid_top), pos2(gamepad_left, mid_bottom));
    let inset = spacing::CARD_INSET
        .min(note_view.width() / 2.0)
        .min(note_view.height() / 2.0);
    Regions {
        status: Rect::from_min_max(pos2(l, t), pos2(r, mid_top)),
        song: Rect::from_min_max(pos2(l, mid_top), pos2(song_right, mid_bottom)),
        note_view,
        note_card: note_view.shrink(inset),
        gamepad: Rect::from_min_max(pos2(gamepad_left, mid_top), pos2(r, mid_bottom)),
        bottom: Rect::from_min_max(pos2(l, mid_bottom), pos2(r, b)),
    }
}

impl Regions {
    /// The regions in reading order (status, song, note view, gamepad,
    /// bottom bar): the order they are filled in, hence the tab order of
    /// later widgets.
    #[must_use]
    pub fn in_reading_order(&self) -> [(RegionId, Rect); 5] {
        [
            (RegionId::Status, self.status),
            (RegionId::Song, self.song),
            (RegionId::NoteView, self.note_view),
            (RegionId::Gamepad, self.gamepad),
            (RegionId::Bottom, self.bottom),
        ]
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing, clippy::panic)]

    use super::*;
    use egui::vec2;

    const SIZES: [(f32, f32); 5] = [
        (1024.0, 680.0),
        (1279.0, 800.0),
        (1280.0, 800.0),
        (1920.0, 1080.0),
        (2560.0, 1440.0),
    ];

    fn content(w: f32, h: f32) -> Rect {
        Rect::from_min_size(pos2(0.0, 0.0), vec2(w, h))
    }

    #[test]
    fn g1_sizes_match_the_tokens() {
        for (w, h) in SIZES {
            let r = regions(content(w, h));
            assert_eq!(r.status.height(), 44.0, "{w}x{h}");
            assert_eq!(r.bottom.height(), 88.0, "{w}x{h}");
            assert_eq!(r.song.width(), 260.0, "{w}x{h}");
            // Story 4.9 (UX-DR6): the compact gamepad below 1280 pt, the
            // note view takes the 100 pt it frees.
            let gamepad = if w < 1280.0 { 200.0 } else { 300.0 };
            assert_eq!(r.gamepad.width(), gamepad, "{w}x{h}");
            assert_eq!(r.note_view.width(), w - 260.0 - gamepad, "{w}x{h}");
            assert_eq!(r.note_view.height(), h - 132.0, "{w}x{h}");
            assert_eq!(r.note_card, r.note_view.shrink(6.0), "{w}x{h}");
            assert_eq!(r.status.width(), w, "{w}x{h}");
            assert_eq!(r.bottom.width(), w, "{w}x{h}");
        }
    }

    /// Story 4.9: the compact threshold is the content width, in points,
    /// independent of where the content rect starts.
    #[test]
    fn g1_compact_threshold() {
        for (w, compact) in [
            (1024.0, true),
            (1279.75, true),
            (1280.0, false),
            (1920.0, false),
        ] {
            for x in [0.0, 37.0] {
                let c = Rect::from_min_size(pos2(x, 10.0), vec2(w, 800.0));
                assert_eq!(is_compact(c), compact, "{w} at {x}");
                let g = regions(c).gamepad.width();
                assert_eq!(g, if compact { 200.0 } else { 300.0 }, "{w} at {x}");
            }
        }
    }

    #[test]
    fn g1_regions_tile_the_content_rect() {
        for (w, h) in SIZES {
            let c = content(w, h);
            let r = regions(c);
            let order = r.in_reading_order();
            let area: f32 = order.iter().map(|(_, x)| x.area()).sum();
            assert_eq!(area, c.area(), "gap or overlap at {w}x{h}");
            for (i, (a_id, a)) in order.iter().enumerate() {
                assert!(c.contains_rect(*a), "{a_id:?} outside at {w}x{h}");
                for (b_id, b) in order.iter().skip(i + 1) {
                    let inter = a.intersect(*b);
                    assert!(
                        !inter.is_positive(),
                        "{a_id:?} overlaps {b_id:?} at {w}x{h}"
                    );
                }
            }
        }
    }

    #[test]
    fn g1_reading_order() {
        let ids: Vec<_> = regions(content(1280.0, 800.0))
            .in_reading_order()
            .iter()
            .map(|(id, _)| *id)
            .collect();
        assert_eq!(
            ids,
            [
                RegionId::Status,
                RegionId::Song,
                RegionId::NoteView,
                RegionId::Gamepad,
                RegionId::Bottom
            ]
        );
    }

    /// Below the minimum window size (tiling WMs ignore min-size hints):
    /// no rect turns negative, the rects still tile without overlap.
    #[test]
    fn g1_clamps_below_the_minimum_size() {
        for (w, h) in [(400.0, 100.0), (100.0, 400.0), (0.0, 0.0), (559.0, 131.0)] {
            let c = content(w, h);
            let r = regions(c);
            let order = r.in_reading_order();
            let area: f32 = order.iter().map(|(_, x)| x.area()).sum();
            assert!((area - c.area()).abs() < 1e-3, "gap or overlap at {w}x{h}");
            for (id, x) in order
                .iter()
                .chain([(RegionId::NoteView, r.note_card)].iter())
            {
                assert!(
                    x.width() >= 0.0 && x.height() >= 0.0,
                    "{id:?} negative at {w}x{h}: {x:?}"
                );
                assert!(c.contains_rect(*x), "{id:?} outside at {w}x{h}: {x:?}");
            }
            for (i, (a_id, a)) in order.iter().enumerate() {
                for (b_id, b) in order.iter().skip(i + 1) {
                    assert!(
                        !a.intersect(*b).is_positive(),
                        "{a_id:?} overlaps {b_id:?} at {w}x{h}"
                    );
                }
            }
        }
    }

    #[test]
    fn g1_offset_content_rect() {
        let c = Rect::from_min_size(pos2(10.0, 20.0), vec2(1280.0, 800.0));
        let r = regions(c);
        assert_eq!(r.status.min, c.min);
        assert_eq!(r.bottom.max, c.max);
        assert_eq!(r.note_view.min, pos2(270.0, 64.0));
    }
}
