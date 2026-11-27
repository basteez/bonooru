//! The `eframe::App`.

use egui::{Context, ThemePreference};
use raw_window_handle::HasWindowHandle as _;

use crate::smoke::{self, Report, Smoke};
use crate::ui::modal::Modals;
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
    /// The one dialog host every screen shares (Story 4.3, UX-DR7).
    modals: Modals,
}

impl App {
    /// Builds the app and applies [`crate::setup`] to the context.
    #[must_use]
    pub fn new(cc: &eframe::CreationContext<'_>, mode: Mode) -> Self {
        crate::setup(&cc.egui_ctx);
        Self {
            mode,
            smoke: None,
            modals: Modals::default(),
        }
    }

    /// Same as [`App::new`], in smoke mode.
    #[must_use]
    pub fn with_smoke(cc: &eframe::CreationContext<'_>, mode: Mode, smoke: Smoke) -> Self {
        Self {
            smoke: Some(smoke),
            ..Self::new(cc, mode)
        }
    }

    /// The dialog host, for screens and tests to request dialogs.
    pub fn modals_mut(&mut self) -> &mut Modals {
        &mut self.modals
    }

    /// The dialog host.
    #[must_use]
    pub fn modals(&self) -> &Modals {
        &self.modals
    }

    /// The smoke state, if this is a smoke run.
    #[must_use]
    pub fn smoke(&self) -> Option<&Smoke> {
        self.smoke.as_ref()
    }

    fn smoke_frame(&mut self, ctx: &Context, frame: &eframe::Frame) {
        let Some(smoke) = self.smoke.as_mut() else {
            return;
        };
        // Completed frames, not passes: an egui discard (e.g. a new Grid)
        // runs a second pass in the same frame (review finding 6).
        let completed = ctx.cumulative_frame_nr();
        if smoke.due(completed) {
            // Everything measured from the live window, nothing from the
            // configuration except the minimum the verdict compares with.
            let inner = ctx
                .input(|i| i.viewport().inner_rect)
                .map(|r| (r.width(), r.height()));
            let handle = frame.window_handle().ok().map(|h| h.as_raw());
            let line = smoke.finish(Report {
                frames: completed,
                ppp: ctx.pixels_per_point(),
                inner,
                min: (crate::MIN_INNER_SIZE[0], crate::MIN_INNER_SIZE[1]),
                dark: ctx.theme() == egui::Theme::Dark,
                backend: smoke::backend_name(handle),
            });
            println!("{line}");
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else if smoke.report().is_none() {
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
        // Story 4.7 reads Esc for STOP here, at the top of the frame, without
        // consuming it (Esc = STOP always, UX-DR61/62); nothing consumes Esc
        // before this point. The open dialog, if any, then consumes the same
        // Esc and closes as `Dismissed`: one press does both (contract on
        // `ui::modal::Modals`; `modals.is_open()` says whether a dialog will
        // take it).
        match self.mode {
            Mode::Main => main_screen::show(ui),
            Mode::Gallery => gallery::show(ui, &mut self.modals),
        }
        let ctx = ui.ctx().clone();
        // Dialogs last: over every screen, one at a time. Each outcome stays
        // in `Modals` until the requesting screen reads it with
        // `take_outcome(id)` (the gallery's samples never read theirs).
        let _ = self.modals.show(&ctx);
        self.smoke_frame(&ctx, frame);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // No default grey flash before the first frame.
        theme::FRAME.to_normalized_gamma_f32()
    }
}
