use crate::app_core::audio::{TimelineSynth, Transport, Wave};
use crate::app_core::timeline::{Timeline, note_name};
use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, vec2};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Instant;

pub struct PianoTab {
    tl: Timeline,
    notes: Arc<Vec<crate::app_core::timeline::Note>>,
    px_per_s: f32,
    view_start_s: f64,
    playhead_s: f64,
    follow: bool,
    wave: Wave,
    mode: Play,
    transport: Arc<Transport>,
    sink: Option<rodio::MixerDeviceSink>,
    midi_path: String,
    status: String,
    autoplay: Option<Instant>,
    pub last_drawn: usize,
    audio_t0: Option<(Instant, f64)>,
    audio_log: Option<Instant>,
}

#[derive(PartialEq)]
enum Play {
    Stopped,
    /// Playhead follows the audio thread's sample clock.
    Audio,
    /// No audio device (or autoplay without audio): playhead follows the wall clock.
    Silent {
        last: Instant,
    },
}

impl PianoTab {
    pub fn new(autoplay: bool) -> Self {
        let tl = Timeline::synthetic(2000, 42);
        let mut s = PianoTab {
            notes: Arc::new(tl.notes.clone()),
            status: format!(
                "synthetic: {} notes, {:.0} s",
                tl.notes.len(),
                tl.duration_s()
            ),
            tl,
            px_per_s: 120.0,
            view_start_s: 0.0,
            playhead_s: 0.0,
            follow: true,
            wave: Wave::Sine,
            mode: Play::Stopped,
            transport: Arc::new(Transport::default()),
            sink: None,
            midi_path: String::new(),
            autoplay: autoplay.then(Instant::now),
            last_drawn: 0,
            audio_t0: None,
            audio_log: None,
        };
        if autoplay {
            s.mode = Play::Silent {
                last: Instant::now(),
            };
        }
        s
    }

    fn set_timeline(&mut self, tl: Timeline) {
        self.stop();
        self.notes = Arc::new(tl.notes.clone());
        self.tl = tl;
        self.playhead_s = 0.0;
        self.view_start_s = 0.0;
    }

    pub fn start_audio(&mut self) {
        self.stop();
        if self.sink.is_none() {
            match rodio::DeviceSinkBuilder::open_default_sink() {
                Ok(mut s) => {
                    s.log_on_drop(false);
                    self.sink = Some(s);
                }
                Err(e) => {
                    self.status = format!("no audio device ({e}); playing silently");
                    self.mode = Play::Silent {
                        last: Instant::now(),
                    };
                    return;
                }
            }
        }
        let synth = TimelineSynth::new(
            self.notes.clone(),
            self.playhead_s,
            self.wave,
            self.transport.clone(),
        );
        self.sink.as_ref().unwrap().mixer().add(synth);
        self.mode = Play::Audio;
        self.audio_t0 = Some((Instant::now(), self.playhead_s));
        eprintln!(
            "[audio] started at {:.2} s ({:?})",
            self.playhead_s, self.wave
        );
    }

    fn stop(&mut self) {
        self.transport.stop.store(true, Ordering::Relaxed);
        self.mode = Play::Stopped;
    }

    fn advance(&mut self) {
        match &mut self.mode {
            Play::Stopped => {}
            Play::Audio => {
                self.playhead_s = self.transport.position_s();
                if let Some((w0, p0)) = self.audio_t0
                    && self
                        .audio_log
                        .is_none_or(|t| t.elapsed().as_secs_f64() > 2.0)
                {
                    let wall = w0.elapsed().as_secs_f64();
                    let clock = self.playhead_s - p0;
                    eprintln!(
                        "[audio] wall {wall:.3} s, audio clock {clock:.3} s, audio ahead by {:+.1} ms",
                        (clock - wall) * 1e3
                    );
                    self.audio_log = Some(Instant::now());
                }
                if self.transport.finished.load(Ordering::Relaxed) {
                    self.mode = Play::Stopped;
                }
            }
            Play::Silent { last } => {
                let now = Instant::now();
                self.playhead_s += (now - *last).as_secs_f64();
                *last = now;
                if self.playhead_s > self.tl.duration_s() {
                    self.playhead_s = 0.0;
                }
            }
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.advance();
        let playing = self.mode != Play::Stopped;

        ui.horizontal(|ui| {
            if playing {
                if ui.button("■ Stop").clicked() {
                    self.stop();
                }
            } else if ui.button("▶ Play (audio)").clicked() {
                self.start_audio();
            }
            if ui.button("⏮").clicked() {
                self.playhead_s = 0.0;
                self.view_start_s = 0.0;
                if self.mode == Play::Audio {
                    self.start_audio();
                }
            }
            ui.selectable_value(&mut self.wave, Wave::Sine, "sine");
            ui.selectable_value(&mut self.wave, Wave::Square, "square");
            ui.checkbox(&mut self.follow, "follow playhead");
            ui.add(
                egui::Slider::new(&mut self.px_per_s, 1.0..=1200.0)
                    .logarithmic(true)
                    .text("px/s"),
            );
            ui.label(format!("t = {:.2} s", self.playhead_s));
        });
        ui.horizontal(|ui| {
            ui.label(".mid path:");
            ui.text_edit_singleline(&mut self.midi_path);
            if ui.button("Load").clicked() {
                match std::fs::read(self.midi_path.trim())
                    .map_err(|e| e.to_string())
                    .and_then(|b| Timeline::from_midi(&b).map_err(|e| e.to_string()))
                {
                    Ok(tl) => {
                        self.status = format!(
                            "{}: {} notes (skyline), {:.0} s",
                            self.midi_path,
                            tl.notes.len(),
                            tl.duration_s()
                        );
                        self.set_timeline(tl);
                    }
                    Err(e) => self.status = format!("load failed: {e}"),
                }
            }
            if ui.button("Synthetic 2,000").clicked() {
                self.set_timeline(Timeline::synthetic(2000, 42));
                self.status = "synthetic: 2000 notes".into();
            }
            ui.label(&self.status);
        });
        ui.label(
            "Drag or scroll to pan · Ctrl/Cmd+scroll or pinch to zoom · click to move the playhead",
        );

        self.draw_roll(ui);

        if let Some(t0) = self.autoplay {
            let t = t0.elapsed().as_secs_f32();
            // Unattended stress: oscillate zoom between 1 px/s (whole piece, ~2,000 notes) and 600 px/s.
            self.px_per_s = 600f32.powf(0.5 - 0.5 * (t * 0.5).cos());
        }
        if playing || self.autoplay.is_some() {
            ui.ctx().request_repaint();
        }
    }

    fn draw_roll(&mut self, ui: &mut egui::Ui) {
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = resp.rect;
        let gutter = 44.0;
        let roll = Rect::from_min_max(rect.min + vec2(gutter, 0.0), rect.max);
        let visuals = ui.visuals();
        painter.rect_filled(rect, 0.0, visuals.extreme_bg_color);

        // Input: pan / zoom / seek
        let (scroll, zoom, hover) = ui.ctx().input(|i| {
            (
                i.smooth_scroll_delta(),
                i.zoom_delta(),
                i.pointer.hover_pos(),
            )
        });
        if resp.hovered() {
            if zoom != 1.0 {
                let anchor_x = hover.map_or(roll.center().x, |p| p.x);
                let anchor_t =
                    self.view_start_s + ((anchor_x - roll.left()) / self.px_per_s) as f64;
                self.px_per_s = (self.px_per_s * zoom).clamp(1.0, 1200.0);
                self.view_start_s = anchor_t - ((anchor_x - roll.left()) / self.px_per_s) as f64;
                self.follow = false;
            }
            let d = scroll.x + scroll.y;
            if d != 0.0 {
                self.view_start_s -= (d / self.px_per_s) as f64;
                self.follow = false;
            }
        }
        if resp.dragged() {
            self.view_start_s -= (resp.drag_delta().x / self.px_per_s) as f64;
            self.follow = false;
        }
        if resp.clicked()
            && let Some(p) = resp.interact_pointer_pos()
        {
            self.playhead_s =
                (self.view_start_s + ((p.x - roll.left()) / self.px_per_s) as f64).max(0.0);
            if self.mode == Play::Audio {
                self.start_audio();
            }
        }

        let span_s = (roll.width() / self.px_per_s) as f64;
        if self.follow {
            // keep playhead at 1/4 of the view
            self.view_start_s = self.playhead_s - span_s * 0.25;
        }
        let t0 = self.view_start_s;
        let t1 = t0 + span_s;

        let (lo, hi) = self.tl.pitch_range();
        let (lo, hi) = (lo.saturating_sub(2), hi.saturating_add(2));
        let rows = (hi - lo + 1) as f32;
        let row_h = roll.height() / rows;
        let y_of = |pitch: u8| roll.bottom() - (pitch - lo + 1) as f32 * row_h;
        let x_of = |t: f64| roll.left() + ((t - t0) as f32) * self.px_per_s;

        let painter = painter.with_clip_rect(rect);
        // Rows: shade black keys, label naturals.
        for p in lo..=hi {
            let y = y_of(p);
            let black = matches!(p % 12, 1 | 3 | 6 | 8 | 10);
            if black {
                painter.rect_filled(
                    Rect::from_min_size(Pos2::new(roll.left(), y), vec2(roll.width(), row_h)),
                    0.0,
                    visuals.faint_bg_color,
                );
            } else if row_h > 9.0 {
                painter.text(
                    Pos2::new(rect.left() + 4.0, y + row_h * 0.5),
                    Align2::LEFT_CENTER,
                    note_name(p),
                    FontId::monospace((row_h * 0.7).min(12.0)),
                    visuals.weak_text_color(),
                );
            }
        }
        // Beat grid every 0.5 s (only visible lines).
        let grid = if self.px_per_s < 40.0 { 4.0 } else { 0.5 };
        let mut g = (t0 / grid).floor() * grid;
        while g <= t1 {
            let x = x_of(g);
            if x >= roll.left() {
                painter.line_segment(
                    [Pos2::new(x, roll.top()), Pos2::new(x, roll.bottom())],
                    Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color),
                );
            }
            g += grid;
        }
        // Notes: binary-search the visible window, draw only those.
        let range = self.tl.visible(t0, t1);
        self.last_drawn = range.len();
        let current = self.tl.note_at(self.playhead_s).copied();
        let roll_painter = painter.with_clip_rect(roll);
        for n in &self.tl.notes[range] {
            let r = Rect::from_min_max(
                Pos2::new(x_of(n.start_s), y_of(n.pitch) + 1.0),
                Pos2::new(
                    x_of(n.end_s()).max(x_of(n.start_s) + 1.0),
                    y_of(n.pitch) + row_h - 1.0,
                ),
            );
            let fill = if Some(*n) == current {
                Color32::from_rgb(255, 170, 60)
            } else {
                Color32::from_rgb(70, 140, 220)
            };
            roll_painter.rect_filled(r, 2.0, fill);
        }
        // Playhead
        let px = x_of(self.playhead_s);
        roll_painter.line_segment(
            [Pos2::new(px, roll.top()), Pos2::new(px, roll.bottom())],
            Stroke::new(2.0, Color32::from_rgb(230, 60, 60)),
        );
        painter.text(
            roll.right_top() + vec2(-6.0, 4.0),
            Align2::RIGHT_TOP,
            format!(
                "{} notes · {:.0}..{:.0} s · drawing {}",
                self.tl.notes.len(),
                t0.max(0.0),
                t1,
                self.last_drawn
            ),
            FontId::monospace(11.0),
            visuals.text_color(),
        );
    }
}
