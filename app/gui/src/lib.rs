//! `ocb-app`: the Bonooru desktop app (egui/eframe, AD-18). Story 4.1: the
//! window, the theme, the fonts, the main-screen regions, the token gallery
//! and the smoke run. The binary is `bonooru` (`src/main.rs`).

pub mod app;
pub mod cli;
pub mod smoke;
pub mod ui;

use egui::{Theme, ThemePreference};

/// Window title.
pub const TITLE: &str = "Bonooru";
/// Initial inner size in logical points (Story 4.1 D4).
pub const INITIAL_INNER_SIZE: [f32; 2] = [1280.0, 800.0];
/// Minimum inner size in logical points (UX-DR5).
pub const MIN_INNER_SIZE: [f32; 2] = [1024.0, 680.0];

/// The native window options.
#[must_use]
pub fn native_options() -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(TITLE)
            .with_app_id("bonooru")
            .with_inner_size(INITIAL_INNER_SIZE)
            .with_min_inner_size(MIN_INNER_SIZE),
        event_loop_builder: event_loop_hook(),
        ..Default::default()
    }
}

/// Linux: force winit's X11 backend (XWayland under Wayland). AD-18: file
/// drops need X11, so eframe is built without `wayland`, but
/// `accesskit_winit` 0.34's default features turn on `winit/wayland`
/// anyway (cargo features are additive), and winit 0.30.13 picks Wayland
/// whenever `WAYLAND_DISPLAY` or `WAYLAND_SOCKET` is set, with no fallback
/// (`src/platform_impl/linux/mod.rs`). Story 4.1 D3, AC 7.
#[cfg(target_os = "linux")]
fn event_loop_hook() -> Option<eframe::EventLoopBuilderHook> {
    use winit::platform::x11::EventLoopBuilderExtX11 as _;
    Some(Box::new(|builder| {
        builder.with_x11();
    }))
}

#[cfg(not(target_os = "linux"))]
fn event_loop_hook() -> Option<eframe::EventLoopBuilderHook> {
    None
}

/// Context setup, once at start: fonts, dark theme from the tokens,
/// keyboard zoom off (all sizes are points; zooming out could shrink the
/// window below its minimum in points).
pub fn setup(ctx: &egui::Context) {
    ui::fonts::install(ctx);
    ctx.set_theme(ThemePreference::Dark);
    ctx.set_visuals_of(Theme::Dark, ui::theme::visuals());
    ctx.options_mut(|o| o.zoom_with_keyboard = false);
}
