//! G.3 (gallery part): the token gallery renders without a missing glyph
//! (egui_kittest fails on one) and lists every colour token by name.

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
fn g3_gallery_lists_every_colour_token() {
    // Tall enough that the scroll area shows every row.
    let mut h = Harness::builder()
        .with_size(vec2(1024.0, 680.0))
        .with_theme(Theme::Light)
        .build_eframe(|cc| App::new(cc, Mode::Gallery));
    h.run();
    for token in theme::color_tokens() {
        let _ = h.get_by_label(token.name);
    }
    for (name, _) in theme::typography_tokens() {
        assert!(
            h.query_by_label_contains(&format!("{name} ")).is_some(),
            "no specimen for typography.{name}"
        );
    }
}
