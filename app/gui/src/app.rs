//! The `eframe::App`.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Context, ThemePreference};
use ocb_core::protocol::Info;
use ocb_services::log::{LogHandle, Logger};
use ocb_services::port::SerialBackend;
use ocb_services::worker::{
    self, LinkSnapshot, TimedEvent, TryRecvError, TrySendError, WorkerCommand, WorkerHandle,
};
use raw_window_handle::HasWindowHandle as _;

use crate::gating::{self, FirmwareWatch, Gate, GateTracker};
use crate::leds::{self, LedTracker, Leds};
use crate::smoke::{self, Report, Smoke};
use crate::stop::{self, CommandResult, StopSource};
use crate::ui::keys::{self, FrameKeys};
use crate::ui::modal::{ModalOutcome, Modals};
use crate::ui::{firmware, gallery, main_screen, theme};

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

/// The worker's waker: every published snapshot (and the worker's end)
/// asks egui for a repaint, so even an idle window drains a new snapshot
/// within one frame (Story 4.5 D2: the time a snapshot waits in the
/// channel is not in the LED ages, so it must stay that short).
pub fn repaint_waker(ctx: &Context) -> Box<dyn Fn() + Send> {
    let ctx = ctx.clone();
    Box::new(move || ctx.request_repaint())
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
    /// Requests the firmware dialog once per mismatching connection
    /// (Story 4.6).
    firmware: FirmwareWatch,
    /// The mismatch whose dialog was requested but is not logged as shown
    /// yet: `dialog shown` is written in the frame the dialog opens (now, or
    /// after a dialog it was queued behind).
    firmware_pending: Option<(Info, u64)>,
    /// Logs every gate change (Story 4.6 AC 9).
    gate_tracker: GateTracker,
    /// The app's own log lines (`dialog`), same handle as the trackers'.
    log: LogHandle,
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
        // Taken before the logger moves into the link: the LED, gate and
        // dialog lines share the worker's log and its `+ms` stamps.
        let app_log = logger.handle();
        let clock = Clock(real_ui_clock());
        let link = worker::spawn(
            SerialBackend::new(),
            logger.handle(),
            repaint_waker(&cc.egui_ctx),
        )
        .ok()
        .map(|h| Link::new(h, Some(logger), clock.now()));
        Self::build(cc, mode, link, clock, app_log)
    }

    /// Builds the app on a worker the caller started (tests: a fake
    /// backend and log).
    #[must_use]
    pub fn with_link(cc: &eframe::CreationContext<'_>, mode: Mode, handle: WorkerHandle) -> Self {
        let clock = Clock(real_ui_clock());
        let link = Link::new(handle, None, clock.now());
        Self::build(cc, mode, Some(link), clock, LogHandle::disabled())
    }

    /// Builds the app with no serial worker: the state of a worker thread
    /// that could not be started (`WorkerHealth::NotStarted`). Tests only
    /// need it (G.20: STOP without a worker is harmless).
    #[must_use]
    pub fn without_link(cc: &eframe::CreationContext<'_>, mode: Mode) -> Self {
        Self::build(
            cc,
            mode,
            None,
            Clock(real_ui_clock()),
            LogHandle::disabled(),
        )
    }

    fn build(
        cc: &eframe::CreationContext<'_>,
        mode: Mode,
        link: Option<Link>,
        clock: Clock,
        log: LogHandle,
    ) -> Self {
        crate::setup(&cc.egui_ctx);
        Self {
            mode,
            smoke: None,
            modals: Modals::default(),
            link,
            clock,
            led_tracker: LedTracker::new(log.clone()),
            firmware: FirmwareWatch::default(),
            firmware_pending: None,
            gate_tracker: GateTracker::new(log.clone()),
            log,
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

    /// This app writing its own log lines to `log` (tests: a memory sink):
    /// the `led` lines (Story 4.5), the `gate` and `dialog` lines (Story
    /// 4.6). The name is Story 4.5's; it now covers all three.
    #[must_use]
    pub fn led_log(mut self, log: LogHandle) -> Self {
        self.led_tracker = LedTracker::new(log.clone());
        self.gate_tracker = GateTracker::new(log.clone());
        self.log = log;
        self
    }

    /// What the latest snapshot blocks (Story 4.6): the gamepad and Play
    /// are off while the controller needs a firmware update.
    #[must_use]
    pub fn gate(&self) -> Gate {
        gating::evaluate(self.link_snapshot().map(AsRef::as_ref))
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

    /// When the next frame must run with no news from the worker: the LEDs'
    /// next timed change, or the moment a silent worker counts as stalled
    /// ([`WORKER_STALL_AFTER`] after the last snapshot, while it runs), the
    /// earlier of the two (Story 4.5 AC 6, 8).
    fn repaint_after(&self, leds: &Leds) -> Option<Duration> {
        let led = leds.next_change_in_us.map(Duration::from_micros);
        let stall = self
            .link
            .as_ref()
            .filter(|_| self.worker_health() == WorkerHealth::Running)
            .map(|l| {
                let since = self.clock.now().saturating_sub(l.last_snapshot_at);
                // Stalled is `>=`: the frame at the deadline sees it.
                WORKER_STALL_AFTER
                    .saturating_sub(since)
                    .max(Duration::from_micros(1))
            });
        match (led, stall) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// The delay the last main-screen frame passed to
    /// `Context::request_repaint_after`: the LEDs' next timed change or the
    /// worker's stall deadline, whichever is first (`None`: neither).
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

    /// Requests STOP (Story 4.7, AD-16): `WorkerCommand::Stop` with
    /// `try_send`, never waiting, and one `stop requested` line saying what
    /// became of it. Harmless without a worker. Epic 6's countdown and Play
    /// reuse it.
    pub fn request_stop(&mut self, source: StopSource) {
        let now = self.clock.now();
        let result = match self.link.as_ref() {
            None => CommandResult::NoWorker,
            Some(l) if l.health(now) == WorkerHealth::Ended => CommandResult::WorkerGone,
            Some(l) => match l.handle.commands.try_send(WorkerCommand::Stop) {
                Ok(()) => CommandResult::Queued,
                Err(TrySendError::Full(_)) => CommandResult::QueueFull,
                Err(TrySendError::Disconnected(_)) => CommandResult::WorkerGone,
            },
        };
        self.log.log(
            "stop",
            stop::requested_line(source, self.modals.is_open(), result),
        );
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
        // eframe runs `logic` on every pass of a frame; only the first pass
        // drains, so a new snapshot (and the firmware dialog it may request,
        // Story 4.6) always lands in a pass that can still be discarded.
        let now = self.clock.now();
        if ctx.current_pass_index() == 0
            && let Some(link) = self.link.as_mut()
        {
            link.drain(now);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        // Esc = STOP, always (Story 4.7, UX-DR41/61): read here, first thing,
        // in every mode, without consuming it; the open dialog, if any, then
        // consumes the same Esc in `Modals::show` and closes as `Dismissed`
        // (contract on `ui::modal::Modals`). Space is consumed here so it
        // never clicks a focused button. Input reaches the first pass only.
        let first_pass = ui.ctx().current_pass_index() == 0;
        let keys = if first_pass {
            keys::frame_start(ui.ctx())
        } else {
            FrameKeys::default()
        };
        // At most one STOP per frame; the first source wins in the log.
        let mut stop = keys.stop.then_some(StopSource::Esc);
        match self.mode {
            Mode::Main => {
                let leds = self.leds();
                self.led_tracker.observe(&leds);
                let gate = self.gate();
                self.gate_tracker.observe(&gate);
                // Once per mismatching connection, requested before
                // `Modals::show` below: drawn in the frame the snapshot was
                // drained (Story 4.6 AC 2, 3).
                let snap = self.link.as_ref().and_then(|l| l.snapshot.as_deref());
                if let Some(new) = self.firmware.take_new(snap) {
                    let id = firmware::dialog_id();
                    // Already on screen (a replug while it is open, owner
                    // decision 4): nothing new to show or log.
                    if self.modals.open_id() != Some(id) {
                        self.modals.request(firmware::required_update_dialog());
                        // Opened now, or queued behind another dialog: logged
                        // when it opens (below).
                        self.firmware_pending = Some(new);
                    }
                }
                if let Some((info, at)) = self.firmware_pending
                    && self.modals.open_id() == Some(firmware::dialog_id())
                {
                    self.firmware_pending = None;
                    // egui draws a new area invisibly in its first pass (the
                    // sizing pass): discarding this pass makes egui run the
                    // frame again at once, and that second pass draws the
                    // dialog visibly, so it shows in this very frame. The
                    // snapshot is drained in the first pass only (`logic`),
                    // so this pass can still be discarded.
                    ui.ctx().request_discard("firmware dialog opens");
                    self.log.log("dialog", gating::dialog_shown_line(&info, at));
                }
                let out = main_screen::show(ui, &leds, &gate);
                if out.stop_clicked && stop.is_none() {
                    stop = Some(StopSource::Button);
                }
                // A staleness or grace boundary, or a silent worker turning
                // stalled, changes an LED with no new snapshot: draw that
                // frame on time (AC 6, 7, 8).
                self.last_repaint_after = self.repaint_after(&leds);
                if let Some(d) = self.last_repaint_after {
                    ui.ctx().request_repaint_after(d);
                }
            }
            Mode::Gallery => gallery::show(ui, &mut self.modals),
        }
        // Before `Modals::show`: `dialog=open` names the dialog this same
        // Esc closes.
        if let Some(source) = stop
            && first_pass
        {
            self.request_stop(source);
        }
        let ctx = ui.ctx().clone();
        // Dialogs last: over every screen, one at a time. Each outcome stays
        // in `Modals` until the requesting screen reads it with
        // `take_outcome(id)`: the main screen reads the firmware dialog's,
        // the gallery's samples (its update sample is the same dialog) never
        // read theirs.
        let _ = self.modals.show(&ctx);
        // The firmware dialog's answer: logged only; closing it never lifts
        // the gate (only a new connection with a matching INFO does).
        if self.mode == Mode::Main
            && let Some(outcome) = self.modals.take_outcome(firmware::dialog_id())
        {
            let outcome = match outcome {
                ModalOutcome::Primary => Some("primary"),
                ModalOutcome::Dismissed => Some("dismissed"),
                // The firmware dialog has no secondary button (Epic 4: OK
                // only); `dialog closed` knows only primary and dismissed.
                ModalOutcome::Secondary => {
                    debug_assert!(false, "the firmware dialog has no secondary button");
                    None
                }
            };
            if let Some(outcome) = outcome {
                self.log.log("dialog", gating::dialog_closed_line(outcome));
            }
        }
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
