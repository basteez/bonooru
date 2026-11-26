//! The `eframe::App`.

use egui::{Context, ThemePreference};
use raw_window_handle::HasWindowHandle as _;

use crate::smoke::{self, Report, Smoke};
use crate::ui::{gallery, main_screen, theme};

/// Which screen the window shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// The main screen (default).
    Main,
    /// The token gallery (`--gallery`), kept out of normal navigation.
    Gallery,
}

/// The app.
#[derive(Debug)]
pub struct App {
    mode: Mode,
    smoke: Option<Smoke>,
}

impl App {
    /// Builds the app and applies [`crate::setup`] to the context.
    #[must_use]
    pub fn new(cc: &eframe::CreationContext<'_>, mode: Mode) -> Self {
        crate::setup(&cc.egui_ctx);
        Self { mode, smoke: None }
    }

    /// Same as [`App::new`], in smoke mode.
    #[must_use]
    pub fn with_smoke(cc: &eframe::CreationContext<'_>, mode: Mode, smoke: Smoke) -> Self {
        Self {
            smoke: Some(smoke),
            ..Self::new(cc, mode)
        }
    }

    fn smoke_frame(&mut self, ctx: &Context, frame: &eframe::Frame) {
        let Some(smoke) = self.smoke.as_mut() else {
            return;
        };
        if smoke.frame() {
            let inner = ctx
                .input(|i| i.viewport().inner_rect)
                .map_or((0.0, 0.0), |r| (r.width(), r.height()));
            let handle = frame.window_handle().ok().map(|h| h.as_raw());
            let report = Report {
                frames: smoke.frames(),
                ppp: ctx.pixels_per_point(),
                inner,
                min: (crate::MIN_INNER_SIZE[0], crate::MIN_INNER_SIZE[1]),
                dark: ctx.theme() == egui::Theme::Dark,
                backend: smoke::backend_name(handle),
            };
            println!("{}", report.line());
            smoke.mark_reported();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else if smoke.frames() < smoke::FRAMES {
            ctx.request_repaint();
        }
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Dark only, whatever the OS (or a test harness) says (AC 2).
        if ctx.options(|o| o.theme_preference) != ThemePreference::Dark {
            ctx.set_theme(ThemePreference::Dark);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        match self.mode {
            Mode::Main => main_screen::show(ui),
            Mode::Gallery => gallery::show(ui),
        }
        let ctx = ui.ctx().clone();
        self.smoke_frame(&ctx, frame);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // No default grey flash before the first frame.
        theme::FRAME.to_normalized_gamma_f32()
    }
}
