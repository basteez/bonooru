//! G.2: window options, dark-only theme and keyboard zoom (AC 2).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use egui::{Theme, vec2};
use egui_kittest::Harness;
use ocb_app::app::{App, Mode};
use ocb_app::ui::theme;

#[test]
fn g2_native_options() {
    let o = ocb_app::native_options();
    assert_eq!(o.viewport.title.as_deref(), Some("Bonooru"));
    assert_eq!(o.viewport.inner_size, Some(vec2(1280.0, 800.0)));
    assert_eq!(o.viewport.min_inner_size, Some(vec2(1024.0, 680.0)));
    // The X11 hook exists exactly on Linux (AC 7).
    assert_eq!(o.event_loop_builder.is_some(), cfg!(target_os = "linux"));
}

#[test]
fn g2_dark_even_when_the_system_says_light() {
    let mut h = Harness::builder()
        .with_size(vec2(1280.0, 800.0))
        .with_theme(Theme::Light)
        .build_eframe(|cc| App::new(cc, Mode::Main));
    h.run();
    assert_eq!(h.ctx.theme(), Theme::Dark);
    assert_eq!(h.ctx.global_style().visuals.panel_fill, theme::FRAME);
    assert!(h.ctx.global_style().visuals.dark_mode);
}

#[test]
fn g2_keyboard_zoom_off() {
    let ctx = egui::Context::default();
    ocb_app::setup(&ctx);
    assert!(!ctx.options(|o| o.zoom_with_keyboard));
    assert_eq!(ctx.zoom_factor(), 1.0);
}
