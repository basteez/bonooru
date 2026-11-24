//! The soak timeline (Story 3.4 AC4, D6, D7): one event every `period_us`
//! for `duration_us`, alternating NEUTRAL and a left-stick X nudge far
//! inside any deadzone (harmless on any host), the last event `NEUTRAL|END`.
//! Generated lazily: 600 000 events are never held at once.

use ocb_core::protocol::{NEUTRAL, State};

use crate::timeline::neutral_end;

/// The bench soak: 10 minutes.
pub const SOAK_DURATION_US: u64 = 600_000_000;
/// One event per millisecond (D6).
pub const SOAK_PERIOD_US: u32 = 1_000;
/// The start lead of the soak (the profile's 500 ms).
pub const SOAK_LEAD_US: u32 = 500_000;
/// The nudge: left stick X 0x84 (centre 0x80).
pub const SOAK_NUDGE_LX: u8 = 0x84;

/// The soak events, lazily.
#[derive(Debug, Clone)]
pub struct Soak {
    n: u32,
    period_us: u32,
    i: u32,
}

impl Soak {
    /// `duration_us / period_us` events (at least 2), `t_us` from 0.
    #[must_use]
    pub fn new(duration_us: u64, period_us: u32) -> Self {
        let period = period_us.max(1);
        let n = u32::try_from(duration_us / u64::from(period))
            .unwrap_or(u32::MAX)
            .max(2);
        Self {
            n,
            period_us: period,
            i: 0,
        }
    }

    /// How many events in all.
    #[must_use]
    pub fn len(&self) -> u32 {
        self.n
    }

    /// Never empty (at least 2 events).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        false
    }

    /// The last event's `t_us`.
    #[must_use]
    pub fn last_t_us(&self) -> u64 {
        u64::from(self.n - 1) * u64::from(self.period_us)
    }
}

impl Iterator for Soak {
    type Item = (u32, State);

    fn next(&mut self) -> Option<(u32, State)> {
        if self.i >= self.n {
            return None;
        }
        let i = self.i;
        self.i += 1;
        let t = u32::try_from(u64::from(i) * u64::from(self.period_us)).ok()?;
        let state = if i + 1 == self.n {
            neutral_end()
        } else if i % 2 == 1 {
            State {
                lx: SOAK_NUDGE_LX,
                ..NEUTRAL
            }
        } else {
            NEUTRAL
        };
        Some((t, state))
    }
}
