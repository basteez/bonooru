//! G.3 (gallery part): the token gallery renders without a missing glyph
//! (egui_kittest fails on one) and shows every colour token and every
//! typography token by its exact name.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use egui::{Theme, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use ocb_app::app::{App, Mode};
use ocb_app::ui::theme;

#[test]
fn g3_gallery_lists_every_colour_and_typography_token() {
    // At 1024x680 the scroll area clips the rows below the fold, but clipped
    // widgets stay in the AccessKit tree, so every row is queryable.
    let mut h = Harness::builder()
        .with_size(vec2(1024.0, 680.0))
        .with_theme(Theme::Light)
        .build_eframe(|cc| App::new(cc, Mode::Gallery));
    h.run();
    let names = theme::color_tokens()
        .iter()
        .map(|t| t.name)
        .chain(theme::typography_tokens().iter().map(|(n, _)| *n));
    for name in names {
        // Exact label match: `label` does not match `note-label`.
        assert_eq!(
            h.query_all_by_label(name).count(),
            1,
            "gallery must show `{name}` exactly once"
        );
    }
}
