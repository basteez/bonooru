mod devices;
mod flasher;
mod gamepad;
mod piano;

use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Devices,
    PianoRoll,
    Flasher,
    Gamepad,
}

pub struct SpikeApp {
    tab: Tab,
    devices: devices::DevicesTab,
    piano: piano::PianoTab,
    flasher: flasher::FlasherTab,
    pad: gamepad::PadTab,
    stats: FrameStats,
    t0: Instant,
    first_frame: Option<Duration>,
    exit_after: Option<f64>,
}

impl SpikeApp {
    pub fn new(cc: &eframe::CreationContext<'_>, t0: Instant) -> Self {
        let env = |k: &str| std::env::var(k).ok();
        let autoplay = env("SPIKE_AUTOPLAY").is_some();
        let mut piano = piano::PianoTab::new(autoplay);
        if autoplay && env("SPIKE_AUDIO").is_some() {
            piano.start_audio();
        }
        eprintln!(
            "[start] creation context ready at {:.1} ms",
            t0.elapsed().as_secs_f64() * 1e3
        );
        SpikeApp {
            tab: if autoplay {
                Tab::PianoRoll
            } else {
                Tab::Devices
            },
            devices: devices::DevicesTab::new(cc.egui_ctx.clone()),
            piano,
            flasher: flasher::FlasherTab::new(cc.egui_ctx.clone()),
            pad: gamepad::PadTab::default(),
            stats: FrameStats::default(),
            t0,
            first_frame: None,
            exit_after: env("SPIKE_EXIT_AFTER").and_then(|s| s.parse().ok()),
        }
    }
}

impl eframe::App for SpikeApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.first_frame.is_none() {
            let d = self.t0.elapsed();
            self.first_frame = Some(d);
            eprintln!("[start] first frame at {:.1} ms", d.as_secs_f64() * 1e3);
        }
        self.stats
            .record(frame.info().cpu_usage, self.piano.last_drawn);

        // Drain worker channels every frame regardless of the active tab.
        self.devices.poll();
        self.flasher.poll();

        egui::Panel::top("tabs").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, Tab::Devices, "1 · Devices");
                ui.selectable_value(&mut self.tab, Tab::PianoRoll, "2-3 · Piano roll + audio");
                ui.selectable_value(&mut self.tab, Tab::Flasher, "4 · UF2 flasher");
                ui.selectable_value(&mut self.tab, Tab::Gamepad, "Gamepad stub");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.monospace(self.stats.summary());
                });
            });
        });

        egui::CentralPanel::default().show(ui, |ui| match self.tab {
            Tab::Devices => self.devices.ui(ui),
            Tab::PianoRoll => self.piano.ui(ui),
            Tab::Flasher => self.flasher.ui(ui),
            Tab::Gamepad => self.pad.ui(ui),
        });

        // The gamepad must see key-up even if another tab is active, or a button sticks.
        self.pad.handle_keys(&ctx, self.tab == Tab::Gamepad);

        if let Some(secs) = self.exit_after
            && self.t0.elapsed().as_secs_f64() >= secs
        {
            self.stats.log(true);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if self.exit_after.is_some() {
            ctx.request_repaint();
        }
    }
}

/// Rolling frame statistics. `cpu_usage` is eframe's measurement of the previous frame
/// (layout + tessellation + paint submission), not GPU time.
#[derive(Default)]
struct FrameStats {
    cpu_ms: Vec<f32>,
    interval_ms: Vec<f32>,
    last: Option<Instant>,
    last_log: Option<Instant>,
    drawn: usize,
}

impl FrameStats {
    fn record(&mut self, cpu: Option<f32>, drawn: usize) {
        let now = Instant::now();
        if let Some(l) = self.last {
            self.interval_ms.push((now - l).as_secs_f32() * 1e3);
        }
        self.last = Some(now);
        if let Some(c) = cpu {
            self.cpu_ms.push(c * 1e3);
        }
        self.drawn = drawn;
        if self.cpu_ms.len() > 600 {
            self.cpu_ms.drain(..300);
        }
        if self.interval_ms.len() > 600 {
            self.interval_ms.drain(..300);
        }
        if self
            .last_log
            .is_none_or(|t| t.elapsed() > Duration::from_secs(2))
        {
            self.log(false);
            self.last_log = Some(now);
        }
    }

    fn pct(v: &[f32], p: f32) -> f32 {
        if v.is_empty() {
            return 0.0;
        }
        let mut s = v.to_vec();
        s.sort_by(f32::total_cmp);
        s[((s.len() - 1) as f32 * p) as usize]
    }

    fn summary(&self) -> String {
        let n = self.cpu_ms.len().min(120);
        let recent = &self.cpu_ms[self.cpu_ms.len() - n..];
        format!(
            "cpu/frame p50 {:.2} ms  p99 {:.2} ms",
            Self::pct(recent, 0.5),
            Self::pct(recent, 0.99)
        )
    }

    fn log(&self, final_: bool) {
        if self.cpu_ms.is_empty() {
            return;
        }
        eprintln!(
            "[frame{}] n={} cpu p50={:.2} p95={:.2} max={:.2} ms | interval p50={:.1} p95={:.1} max={:.1} ms | notes drawn={}",
            if final_ { "-final" } else { "" },
            self.cpu_ms.len(),
            Self::pct(&self.cpu_ms, 0.5),
            Self::pct(&self.cpu_ms, 0.95),
            Self::pct(&self.cpu_ms, 1.0),
            Self::pct(&self.interval_ms, 0.5),
            Self::pct(&self.interval_ms, 0.95),
            Self::pct(&self.interval_ms, 1.0),
            self.drawn
        );
    }
}
