//! Monophonic note timeline: the common internal model `(midi_pitch, start_s, duration_s)`
//! from docs/score-format-and-mapping.md. GUI-free.

use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Note {
    pub pitch: u8,
    pub start_s: f64,
    pub dur_s: f64,
}

impl Note {
    pub fn end_s(&self) -> f64 {
        self.start_s + self.dur_s
    }
}

/// Sorted by start, non-overlapping (monophonic).
#[derive(Clone, Debug, Default)]
pub struct Timeline {
    pub notes: Vec<Note>,
}

#[derive(Debug)]
pub enum MidiError {
    Parse(String),
    Timecode,
    Empty,
}

impl fmt::Display for MidiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MidiError::Parse(e) => write!(f, "MIDI parse error: {e}"),
            MidiError::Timecode => write!(f, "SMPTE timecode MIDI files are not supported"),
            MidiError::Empty => write!(f, "no notes found"),
        }
    }
}

impl std::error::Error for MidiError {}

impl Timeline {
    /// Deterministic pseudo-random monophonic melody (LCG, no external crate).
    pub fn synthetic(count: usize, seed: u64) -> Self {
        let mut state = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let mut next = move || {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (state >> 33) as u32
        };
        let durations = [0.125, 0.25, 0.25, 0.5, 0.375, 0.75];
        let mut notes = Vec::with_capacity(count);
        let mut t = 0.0;
        let mut pitch: i32 = 69;
        for _ in 0..count {
            // random walk inside the ocarina range B3..F5 (59..77)
            pitch = (pitch + (next() % 7) as i32 - 3).clamp(59, 77);
            let dur = durations[(next() % durations.len() as u32) as usize];
            let gap = if next() % 5 == 0 { 0.125 } else { 0.0 };
            notes.push(Note {
                pitch: pitch as u8,
                start_s: t,
                dur_s: dur * 0.9,
            });
            t += dur + gap;
        }
        Timeline { notes }
    }

    /// Parse a Standard MIDI File, merge all tracks, apply the tempo map, and reduce to
    /// monophonic with the "skyline" rule (highest sounding note wins, earlier note is cut).
    pub fn from_midi(bytes: &[u8]) -> Result<Self, MidiError> {
        use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};

        let smf = Smf::parse(bytes).map_err(|e| MidiError::Parse(e.to_string()))?;
        let tpq = match smf.header.timing {
            Timing::Metrical(t) => t.as_int() as f64,
            Timing::Timecode(..) => return Err(MidiError::Timecode),
        };

        // (abs_tick, kind) where kind: 0 = tempo(us/q), 1 = note off, 2 = note on
        #[derive(Clone, Copy)]
        enum Ev {
            Tempo(u32),
            Off(u8),
            On(u8),
        }
        let mut events: Vec<(u64, Ev)> = Vec::new();
        for track in &smf.tracks {
            let mut tick = 0u64;
            for ev in track {
                tick += ev.delta.as_int() as u64;
                match ev.kind {
                    TrackEventKind::Meta(MetaMessage::Tempo(us)) => {
                        events.push((tick, Ev::Tempo(us.as_int())))
                    }
                    TrackEventKind::Midi { message, .. } => match message {
                        MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                            events.push((tick, Ev::On(key.as_int())))
                        }
                        MidiMessage::NoteOn { key, .. } | MidiMessage::NoteOff { key, .. } => {
                            events.push((tick, Ev::Off(key.as_int())))
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
        // stable sort: tempo first, then offs, then ons at the same tick
        events.sort_by_key(|(t, e)| {
            (
                *t,
                match e {
                    Ev::Tempo(_) => 0,
                    Ev::Off(_) => 1,
                    Ev::On(_) => 2,
                },
            )
        });

        let mut us_per_q = 500_000.0;
        let mut last_tick = 0u64;
        let mut secs = 0.0;
        let mut sounding: Vec<(u8, f64)> = Vec::new(); // held keys
        let mut out: Vec<Note> = Vec::new();
        let mut current: Option<(u8, f64)> = None; // note currently emitted by skyline

        for (tick, ev) in events {
            secs += (tick - last_tick) as f64 / tpq * us_per_q / 1e6;
            last_tick = tick;
            match ev {
                Ev::Tempo(us) => us_per_q = us as f64,
                Ev::On(k) => {
                    sounding.push((k, secs));
                    if current.is_none_or(|(p, _)| k >= p) {
                        if let Some((p, s)) = current.take() {
                            push_note(&mut out, p, s, secs);
                        }
                        current = Some((k, secs));
                    }
                }
                Ev::Off(k) => {
                    if let Some(i) = sounding.iter().position(|(p, _)| *p == k) {
                        sounding.remove(i);
                    }
                    if current.is_some_and(|(p, _)| p == k) {
                        let (p, s) = current.take().unwrap();
                        push_note(&mut out, p, s, secs);
                        // skyline: fall back to the highest still-held key (restarted now)
                        if let Some(&(h, _)) = sounding.iter().max_by_key(|(p, _)| *p) {
                            current = Some((h, secs));
                        }
                    }
                }
            }
        }
        if out.is_empty() {
            return Err(MidiError::Empty);
        }
        let tl = Timeline { notes: out };
        debug_assert!(tl.is_monophonic());
        Ok(tl)
    }

    pub fn duration_s(&self) -> f64 {
        self.notes.last().map_or(0.0, |n| n.end_s())
    }

    /// Index range of notes overlapping `[t0, t1)`. O(log n).
    pub fn visible(&self, t0: f64, t1: f64) -> std::ops::Range<usize> {
        let first = self.notes.partition_point(|n| n.end_s() <= t0);
        let last = self.notes.partition_point(|n| n.start_s < t1);
        first..last.max(first)
    }

    /// Note sounding at time `t`, if any. O(log n).
    pub fn note_at(&self, t: f64) -> Option<&Note> {
        let i = self.notes.partition_point(|n| n.start_s <= t);
        let n = self.notes.get(i.checked_sub(1)?)?;
        (t < n.end_s()).then_some(n)
    }

    pub fn pitch_range(&self) -> (u8, u8) {
        let lo = self.notes.iter().map(|n| n.pitch).min().unwrap_or(60);
        let hi = self.notes.iter().map(|n| n.pitch).max().unwrap_or(72);
        (lo, hi)
    }

    pub fn is_monophonic(&self) -> bool {
        self.notes
            .windows(2)
            .all(|w| w[0].end_s() <= w[1].start_s + 1e-9)
    }
}

fn push_note(out: &mut Vec<Note>, pitch: u8, start: f64, end: f64) {
    if end - start > 1e-6 {
        out.push(Note {
            pitch,
            start_s: start,
            dur_s: end - start,
        });
    }
}

pub fn midi_to_hz(pitch: u8) -> f64 {
    440.0 * 2f64.powf((pitch as f64 - 69.0) / 12.0)
}

pub fn note_name(pitch: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    format!("{}{}", NAMES[pitch as usize % 12], pitch as i32 / 12 - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_is_monophonic_sorted_and_deterministic() {
        let a = Timeline::synthetic(2000, 7);
        let b = Timeline::synthetic(2000, 7);
        assert_eq!(a.notes.len(), 2000);
        assert_eq!(a.notes, b.notes);
        assert!(a.is_monophonic());
        let (lo, hi) = a.pitch_range();
        assert!(lo >= 59 && hi <= 77);
    }

    #[test]
    fn visible_and_note_at() {
        let tl = Timeline::synthetic(100, 1);
        let n = tl.notes[10];
        let mid = n.start_s + n.dur_s / 2.0;
        assert_eq!(tl.note_at(mid), Some(&n));
        let r = tl.visible(mid, mid + 0.001);
        assert_eq!(r, 10..11);
        assert!(tl.note_at(-1.0).is_none());
        assert!(tl.note_at(tl.duration_s() + 1.0).is_none());
        assert_eq!(tl.visible(0.0, 1e9), 0..100);
    }

    /// Hand-built SMF: format 0, 96 tpq, tempo 120 bpm, a C4+E4 chord then G4.
    fn tiny_midi() -> Vec<u8> {
        let mut trk: Vec<u8> = vec![];
        trk.extend([0x00, 0xFF, 0x51, 0x03, 0x07, 0xA1, 0x20]); // tempo 500000
        trk.extend([0x00, 0x90, 60, 100]); // C4 on
        trk.extend([0x00, 0x90, 64, 100]); // E4 on (chord, higher wins)
        trk.extend([0x60, 0x80, 64, 0]); // +96 ticks: E4 off -> C4 resumes
        trk.extend([0x30, 0x80, 60, 0]); // +48: C4 off
        trk.extend([0x00, 0x90, 67, 100]); // G4 on
        trk.extend([0x60, 0x90, 67, 0]); // +96: G4 off (vel 0)
        trk.extend([0x00, 0xFF, 0x2F, 0x00]);
        let mut f = b"MThd\x00\x00\x00\x06\x00\x00\x00\x01\x00\x60MTrk".to_vec();
        f.extend((trk.len() as u32).to_be_bytes());
        f.extend(trk);
        f
    }

    #[test]
    fn midi_skyline() {
        let tl = Timeline::from_midi(&tiny_midi()).unwrap();
        let p: Vec<u8> = tl.notes.iter().map(|n| n.pitch).collect();
        assert_eq!(p, vec![64, 60, 67]);
        assert!((tl.notes[0].dur_s - 0.5).abs() < 1e-9);
        assert!((tl.notes[1].dur_s - 0.25).abs() < 1e-9);
        assert!((tl.notes[2].start_s - 0.75).abs() < 1e-9);
        assert!(tl.is_monophonic());
    }

    #[test]
    fn names_and_freqs() {
        assert_eq!(note_name(60), "C4");
        assert_eq!(note_name(69), "A4");
        assert!((midi_to_hz(69) - 440.0).abs() < 1e-9);
    }
}
