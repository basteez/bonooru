//! G.2: window options, dark-only theme and keyboard zoom (AC 2).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

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
        .build_eframe(|cc| App::with_link(cc, Mode::Main, common::fake_link(&cc.egui_ctx)));
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

/// G.7 in a harness: smoke mode reports once, after 3 completed frames and
/// the first worker snapshot (Story 4.4, `worker=`), and
/// its verdict is judged on what the frame really has: the kittest frame
/// has no window handle, so the run must fail on the backend, not print ok.
#[test]
fn g7_smoke_mode_judges_the_real_frame() {
    use ocb_app::smoke::{Outcome, OutcomeCell, Smoke};
    let cell = OutcomeCell::default();
    let smoke_cell = cell.clone();
    let mut h = Harness::builder()
        .with_size(vec2(1280.0, 800.0))
        .with_theme(Theme::Light)
        .build_eframe(move |cc| {
            App::with_link(cc, Mode::Gallery, common::fake_link(&cc.egui_ctx))
                .smoke_mode(Smoke::new(smoke_cell))
        });
    // The worker thread runs on real time: step until its first snapshot
    // has reached the app (bounded).
    let start = std::time::Instant::now();
    h.run_steps(6);
    while h.state().smoke().and_then(|s| s.report()).is_none()
        && start.elapsed() < std::time::Duration::from_secs(5)
    {
        std::thread::sleep(std::time::Duration::from_millis(5));
        h.run_steps(1);
    }
    let report = h
        .state()
        .smoke()
        .and_then(|s| s.report())
        .cloned()
        .expect("a report");
    assert!(report.frames >= 3, "frames {}", report.frames);
    assert!(report.dark);
    assert_eq!(report.backend, "none");
    assert!(report.worker >= 1, "worker {}", report.worker);
    assert_eq!(cell.get(), Outcome::Fail);
}
