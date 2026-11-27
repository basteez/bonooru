//! The `eframe::App`.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use egui::{Context, ThemePreference};
use ocb_services::log::Logger;
use ocb_services::port::SerialBackend;
use ocb_services::worker::{self, LinkSnapshot, TimedEvent, WorkerHandle};
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

/// Events kept for the screens (Story 4.5 draws them, 4.6 reads versions).
pub const EVENTS_KEPT: usize = 64;
/// How long `on_exit` waits for the worker thread (Story 4.4 AC 2).
pub const WORKER_EXIT_LIMIT: Duration = Duration::from_millis(500);

/// The serial worker as the UI sees it (AD-16): channels only. The UI
/// never waits on them: one `try_recv` drain per frame.
#[derive(Debug)]
pub struct Link {
    handle: WorkerHandle,
    /// The real log thread, when this app started it.
    logger: Option<Logger>,
    snapshot: Option<Arc<LinkSnapshot>>,
    events: VecDeque<TimedEvent>,
}

impl Link {
    fn new(handle: WorkerHandle, logger: Option<Logger>) -> Self {
        Self {
            handle,
            logger,
            snapshot: None,
            events: VecDeque::with_capacity(EVENTS_KEPT),
        }
    }

    /// Takes what the worker sent since the last frame, without waiting.
    fn drain(&mut self) {
        while let Ok(e) = self.handle.events.try_recv() {
            if self.events.len() == EVENTS_KEPT {
                self.events.pop_front();
            }
            self.events.push_back(e);
        }
        while let Ok(s) = self.handle.snapshots.try_recv() {
            self.snapshot = Some(s);
        }
    }

    /// Ends the worker (bounded wait), then the log thread if the worker
    /// is gone (its log handle went with it).
    fn shut_down(&mut self) -> bool {
        let ended = self.handle.shutdown(WORKER_EXIT_LIMIT);
        if ended && let Some(l) = self.logger.take() {
            l.stop();
        }
        ended
    }
}

/// The app.
#[derive(Debug)]
pub struct App {
    mode: Mode,
    smoke: Option<Smoke>,
    /// The one dialog host every screen shares (Story 4.3, UX-DR7).
    modals: Modals,
    /// The serial worker (`None` only if its thread could not start).
    link: Option<Link>,
}

impl App {
    /// Builds the app, applies [`crate::setup`] to the context and starts
    /// the serial worker on the real backend with the real log (Story 4.4).
    /// Tests use [`App::with_link`] instead.
    #[must_use]
    pub fn new(cc: &eframe::CreationContext<'_>, mode: Mode) -> Self {
        let logger = Logger::start();
        let ctx = cc.egui_ctx.clone();
        let link = worker::spawn(
            SerialBackend::new(),
            logger.handle(),
            Box::new(move || ctx.request_repaint()),
        )
        .ok()
        .map(|h| Link::new(h, Some(logger)));
        Self::build(cc, mode, link)
    }

    /// Builds the app on a worker the caller started (tests: a fake
    /// backend and log).
    #[must_use]
    pub fn with_link(cc: &eframe::CreationContext<'_>, mode: Mode, handle: WorkerHandle) -> Self {
        Self::build(cc, mode, Some(Link::new(handle, None)))
    }

    fn build(cc: &eframe::CreationContext<'_>, mode: Mode, link: Option<Link>) -> Self {
        crate::setup(&cc.egui_ctx);
        Self {
            mode,
            smoke: None,
            modals: Modals::default(),
            link,
        }
    }

    /// Same as [`App::new`], in smoke mode.
    #[must_use]
    pub fn with_smoke(cc: &eframe::CreationContext<'_>, mode: Mode, smoke: Smoke) -> Self {
        Self::new(cc, mode).smoke_mode(smoke)
    }

    /// This app in smoke mode.
    #[must_use]
    pub fn smoke_mode(mut self, smoke: Smoke) -> Self {
        self.smoke = Some(smoke);
        self
    }

    /// The latest worker snapshot received.
    #[must_use]
    pub fn link_snapshot(&self) -> Option<&Arc<LinkSnapshot>> {
        self.link.as_ref().and_then(|l| l.snapshot.as_ref())
    }

    /// The last [`EVENTS_KEPT`] worker events, oldest first.
    #[must_use]
    pub fn link_events(&self) -> Option<&VecDeque<TimedEvent>> {
        self.link.as_ref().map(|l| &l.events)
    }

    /// Ends the worker with a bounded wait (what `on_exit` does). Returns
    /// whether the thread ended.
    pub fn shut_down_link(&mut self) -> bool {
        self.link.as_mut().is_none_or(Link::shut_down)
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
        let scans = self
            .link
            .as_ref()
            .and_then(|l| l.snapshot.as_ref())
            .map(|s| s.scans);
        let Some(smoke) = self.smoke.as_mut() else {
            return;
        };
        // Completed frames, not passes: an egui discard (e.g. a new Grid)
        // runs a second pass in the same frame (review finding 6).
        let completed = ctx.cumulative_frame_nr();
        // The first worker snapshot is waited for too (bounded by the
        // watchdog), so the line proves the worker runs (`worker=`).
        if smoke.due(completed, scans) {
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
                worker: scans.unwrap_or(0),
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
        // The worker's news since the last frame; never waits (AD-16).
        if let Some(link) = self.link.as_mut() {
            link.drain();
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

    fn on_exit(&mut self) {
        // Shutdown, then a bounded join: the port closes (DTR falls) and
        // the exit never hangs on the worker (Story 4.4 AC 2).
        let _ = self.shut_down_link();
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // No default grey flash before the first frame.
        theme::FRAME.to_normalized_gamma_f32()
    }
}
