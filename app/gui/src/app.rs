//! The `eframe::App`.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Context, ThemePreference};
use ocb_services::log::{LogHandle, Logger};
use ocb_services::port::SerialBackend;
use ocb_services::worker::{self, LinkSnapshot, TimedEvent, TryRecvError, WorkerHandle};
use raw_window_handle::HasWindowHandle as _;

use crate::leds::{self, LedTracker, Leds};
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

/// The UI's monotonic clock: the time since the app started (Story 4.5:
/// the LEDs judge how long ago the UI received a snapshot). Tests pass a
/// clock they move by hand ([`App::ui_clock`]).
pub type UiClock = Arc<dyn Fn() -> Duration + Send + Sync>;

/// A real [`UiClock`]: an `Instant` taken now.
#[must_use]
pub fn real_ui_clock() -> UiClock {
    let t0 = Instant::now();
    Arc::new(move || t0.elapsed())
}

/// A [`UiClock`] with a `Debug` (the closure has none).
#[derive(Clone)]
struct Clock(UiClock);

impl Clock {
    fn now(&self) -> Duration {
        (self.0)()
    }
}

impl std::fmt::Debug for Clock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Clock").field(&self.now()).finish()
    }
}

fn micros(d: Duration) -> u64 {
    u64::try_from(d.as_micros()).unwrap_or(u64::MAX)
}

/// The serial worker as the UI sees it (AD-16): channels only. The UI
/// never waits on them: one `try_recv` drain per frame.
#[derive(Debug)]
pub struct Link {
    handle: WorkerHandle,
    /// The real log thread, when this app started it.
    logger: Option<Logger>,
    snapshot: Option<Arc<LinkSnapshot>>,
    events: VecDeque<TimedEvent>,
    /// When the last snapshot arrived (or the link was built), on the UI
    /// clock.
    last_snapshot_at: Duration,
    ended: bool,
}

/// The worker publishes at least once per scan (1 s); no snapshot for
/// this long means it is stuck.
pub const WORKER_STALL_AFTER: Duration = Duration::from_secs(3);

/// Whether the serial worker is alive, as the UI can tell without waiting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkerHealth {
    /// Snapshots arrive.
    Running,
    /// No snapshot for [`WORKER_STALL_AFTER`]: the last one is stale.
    Stalled,
    /// The worker thread ended (a panic, or after shutdown).
    Ended,
    /// The worker thread could not be started.
    NotStarted,
}

impl Link {
    fn new(handle: WorkerHandle, logger: Option<Logger>, now: Duration) -> Self {
        Self {
            handle,
            logger,
            snapshot: None,
            events: VecDeque::with_capacity(EVENTS_KEPT),
            last_snapshot_at: now,
            ended: false,
        }
    }

    /// Takes what the worker sent since the last frame, without waiting.
    fn drain(&mut self, now: Duration) {
        while let Ok(e) = self.handle.events.try_recv() {
            if self.events.len() == EVENTS_KEPT {
                self.events.pop_front();
            }
            self.events.push_back(e);
        }
        loop {
            match self.handle.snapshots.try_recv() {
                Ok(s) => {
                    self.snapshot = Some(s);
                    self.last_snapshot_at = now;
                }
                // The worker's sender went with its thread (a panic ends
                // it too): the last snapshot is no longer news.
                Err(TryRecvError::Disconnected) => {
                    self.ended = true;
                    break;
                }
                Err(TryRecvError::Empty) => break,
            }
        }
    }

    fn health(&self, now: Duration) -> WorkerHealth {
        if self.ended || self.handle.is_finished() {
            WorkerHealth::Ended
        } else if now.saturating_sub(self.last_snapshot_at) >= WORKER_STALL_AFTER {
            WorkerHealth::Stalled
        } else {
            WorkerHealth::Running
        }
    }

    /// Ends the worker (bounded wait), then the log thread (bounded too):
    /// what is queued is written even when the worker did not end in time.
    fn shut_down(&mut self) -> bool {
        let ended = self.handle.shutdown(WORKER_EXIT_LIMIT);
        if let Some(l) = self.logger.take() {
            let _ = l.stop();
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
    /// The UI clock (real, or a test's).
    clock: Clock,
    /// Logs every LED change to `bonooru.log` (Story 4.5 AC 12).
    led_tracker: LedTracker,
    /// The delay last passed to `request_repaint_after` for the LEDs'
    /// next timed change (test hook, [`App::last_repaint_after`]).
    last_repaint_after: Option<Duration>,
}

impl App {
    /// Builds the app, applies [`crate::setup`] to the context and starts
    /// the serial worker on the real backend with the real log (Story 4.4).
    /// Tests use [`App::with_link`] instead.
    #[must_use]
    pub fn new(cc: &eframe::CreationContext<'_>, mode: Mode) -> Self {
        let logger = Logger::start();
        // Taken before the logger moves into the link: the LED lines share
        // the worker's log and its `+ms` stamps (AC 12).
        let led_log = logger.handle();
        let ctx = cc.egui_ctx.clone();
        let clock = Clock(real_ui_clock());
        let link = worker::spawn(
            SerialBackend::new(),
            logger.handle(),
            Box::new(move || ctx.request_repaint()),
        )
        .ok()
        .map(|h| Link::new(h, Some(logger), clock.now()));
        Self::build(cc, mode, link, clock, led_log)
    }

    /// Builds the app on a worker the caller started (tests: a fake
    /// backend and log).
    #[must_use]
    pub fn with_link(cc: &eframe::CreationContext<'_>, mode: Mode, handle: WorkerHandle) -> Self {
        let clock = Clock(real_ui_clock());
        let link = Link::new(handle, None, clock.now());
        Self::build(cc, mode, Some(link), clock, LogHandle::disabled())
    }

    fn build(
        cc: &eframe::CreationContext<'_>,
        mode: Mode,
        link: Option<Link>,
        clock: Clock,
        led_log: LogHandle,
    ) -> Self {
        crate::setup(&cc.egui_ctx);
        Self {
            mode,
            smoke: None,
            modals: Modals::default(),
            link,
            clock,
            led_tracker: LedTracker::new(led_log),
            last_repaint_after: None,
        }
    }

    /// This app on another UI clock (tests: a clock moved by hand). The
    /// link's "last snapshot" time restarts at the new clock's now.
    #[must_use]
    pub fn ui_clock(mut self, clock: UiClock) -> Self {
        self.clock = Clock(clock);
        let now = self.clock.now();
        if let Some(l) = self.link.as_mut() {
            l.last_snapshot_at = now;
        }
        self
    }

    /// This app logging its LED changes to `log` (tests: a memory sink).
    #[must_use]
    pub fn led_log(mut self, log: LogHandle) -> Self {
        self.led_tracker = LedTracker::new(log);
        self
    }

    /// The three LEDs now (Story 4.5): the latest snapshot, the UI clock's
    /// time since it arrived and the worker's health.
    #[must_use]
    pub fn leds(&self) -> Leds {
        let now = self.clock.now();
        let snapshot = self.link.as_ref().and_then(|l| l.snapshot.as_deref());
        let since = self
            .link
            .as_ref()
            .filter(|l| l.snapshot.is_some())
            .map_or(0, |l| micros(now.saturating_sub(l.last_snapshot_at)));
        leds::evaluate(snapshot, since, self.worker_health())
    }

    /// The delay the last main-screen frame passed to
    /// `Context::request_repaint_after` for the LEDs' next timed change
    /// (`None`: no timed rule was active).
    #[must_use]
    pub fn last_repaint_after(&self) -> Option<Duration> {
        self.last_repaint_after
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

    /// Whether the worker is alive (Story 4.5 shows a dead or stuck worker
    /// instead of a frozen "connected").
    #[must_use]
    pub fn worker_health(&self) -> WorkerHealth {
        let now = self.clock.now();
        self.link
            .as_ref()
            .map_or(WorkerHealth::NotStarted, |l| l.health(now))
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
        let now = self.clock.now();
        if let Some(link) = self.link.as_mut() {
            link.drain(now);
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
            Mode::Main => {
                let leds = self.leds();
                self.led_tracker.observe(&leds);
                main_screen::show(ui, &leds);
                // A staleness or grace boundary changes an LED with no new
                // snapshot: draw that frame on time (AC 6, 7).
                self.last_repaint_after = leds.next_change_in_us.map(Duration::from_micros);
                if let Some(d) = self.last_repaint_after {
                    ui.ctx().request_repaint_after(d);
                }
            }
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
