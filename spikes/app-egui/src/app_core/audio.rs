//! Audio preview: a `rodio::Source` that renders the timeline as tones. The source owns the
//! clock: it publishes its sample position through an atomic, and the UI draws the playhead
//! from that position, so audio and playhead cannot drift apart. GUI-free.

use super::timeline::{Note, midi_to_hz};
use rodio::{ChannelCount, Sample, SampleRate, Source};
use std::num::NonZero;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wave {
    Sine,
    Square,
}

/// Shared between the audio thread and the UI.
#[derive(Default)]
pub struct Transport {
    /// Samples rendered since the start of the timeline.
    pub pos_samples: AtomicU64,
    pub stop: AtomicBool,
    pub finished: AtomicBool,
}

pub struct TimelineSynth {
    notes: Arc<Vec<Note>>,
    rate: u32,
    wave: Wave,
    transport: Arc<Transport>,
    cursor: usize,
    phase: f64,
    end_sample: u64,
}

pub const RATE: u32 = 48_000;
const RAMP_S: f64 = 0.004; // 4 ms attack/release to avoid clicks

impl TimelineSynth {
    pub fn new(notes: Arc<Vec<Note>>, start_s: f64, wave: Wave, transport: Arc<Transport>) -> Self {
        let rate = RATE;
        let end = notes.last().map_or(0.0, |n| n.end_s());
        let pos = (start_s.max(0.0) * rate as f64) as u64;
        transport.pos_samples.store(pos, Ordering::Relaxed);
        transport.stop.store(false, Ordering::Relaxed);
        transport.finished.store(false, Ordering::Relaxed);
        let cursor = notes.partition_point(|n| n.end_s() <= start_s);
        TimelineSynth {
            notes,
            rate,
            wave,
            transport,
            cursor,
            phase: 0.0,
            end_sample: (end * rate as f64) as u64 + rate as u64 / 10,
        }
    }

    fn sample_at(&mut self, pos: u64) -> f32 {
        let t = pos as f64 / self.rate as f64;
        while self.cursor < self.notes.len() && self.notes[self.cursor].end_s() <= t {
            self.cursor += 1;
        }
        let Some(n) = self.notes.get(self.cursor) else {
            return 0.0;
        };
        if t < n.start_s {
            self.phase = 0.0;
            return 0.0;
        }
        let env = ((t - n.start_s) / RAMP_S)
            .min((n.end_s() - t) / RAMP_S)
            .clamp(0.0, 1.0);
        self.phase = (self.phase + midi_to_hz(n.pitch) / self.rate as f64).fract();
        let s = match self.wave {
            Wave::Sine => (self.phase * std::f64::consts::TAU).sin(),
            Wave::Square => {
                if self.phase < 0.5 {
                    0.5
                } else {
                    -0.5
                }
            }
        };
        (s * env * 0.2) as f32
    }
}

impl Iterator for TimelineSynth {
    type Item = Sample;

    fn next(&mut self) -> Option<Sample> {
        if self.transport.stop.load(Ordering::Relaxed) {
            return None;
        }
        let pos = self.transport.pos_samples.load(Ordering::Relaxed);
        if pos >= self.end_sample {
            self.transport.finished.store(true, Ordering::Relaxed);
            return None;
        }
        let s = self.sample_at(pos);
        self.transport.pos_samples.store(pos + 1, Ordering::Relaxed);
        Some(s as Sample)
    }
}

impl Source for TimelineSynth {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> ChannelCount {
        NonZero::new(1).unwrap()
    }
    fn sample_rate(&self) -> SampleRate {
        NonZero::new(self.rate).unwrap()
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

impl Transport {
    pub fn position_s(&self) -> f64 {
        self.pos_samples.load(Ordering::Relaxed) as f64 / RATE as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_core::timeline::Timeline;

    #[test]
    fn renders_tones_and_silence_and_advances_clock() {
        let tl = Timeline::synthetic(10, 3);
        let tr = Arc::new(Transport::default());
        let synth = TimelineSynth::new(Arc::new(tl.notes.clone()), 0.0, Wave::Sine, tr.clone());
        let samples: Vec<f32> = synth.collect();
        let expected = ((tl.duration_s() * RATE as f64) as usize) + RATE as usize / 10;
        assert!((samples.len() as i64 - expected as i64).abs() <= 1);
        assert!(samples.iter().any(|s| s.abs() > 0.1));
        assert!(samples.iter().all(|s| s.abs() <= 0.2 + 1e-6));
        assert!(tr.finished.load(Ordering::Relaxed));
        assert!((tr.position_s() - samples.len() as f64 / RATE as f64).abs() < 1e-6);
    }

    #[test]
    fn stop_flag_ends_stream() {
        let tl = Timeline::synthetic(10, 3);
        let tr = Arc::new(Transport::default());
        let mut synth = TimelineSynth::new(Arc::new(tl.notes), 1.0, Wave::Square, tr.clone());
        assert!((tr.position_s() - 1.0).abs() < 1e-3);
        assert!(synth.next().is_some());
        tr.stop.store(true, Ordering::Relaxed);
        assert!(synth.next().is_none());
    }
}
