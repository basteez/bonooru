//! The timeline file (`t_us,state` CSV, Story 3.4 D5):
//!
//! ```text
//! # start_lead_us: 500000
//! t_us,state
//! 0,0000088080808000
//! ...
//! 44166667,0000088080808001
//! ```
//!
//! `#` lines are comments; `# start_lead_us: N` is required once. `state` is
//! the 8 wire bytes of AD-10 as 16 hex digits. `t_us` strictly increasing and
//! below 2^31; the last row is `NEUTRAL|END` and END appears nowhere else;
//! every state is on the AD-9 allow-list. Checked before the port opens.

use ocb_core::protocol::consts::{
    BTN_A, BTN_L, BTN_R, BTN_X, BTN_Y, HAT_DOWN, HAT_NEUTRAL, HAT_UP, STATE_FLAG_END, STICK_CENTER,
};
use ocb_core::protocol::{NEUTRAL, State};

/// At most this many rows (a melody has a few hundred).
pub const MAX_ROWS: usize = 100_000;

/// `t_us` must stay below this (AD-8).
pub const T_US_LIMIT: u32 = 1 << 31;

const HEADER: &str = "t_us,state";
const LEAD_KEY: &str = "start_lead_us:";
const ALLOWED_BUTTONS: u16 = BTN_Y | BTN_A | BTN_X | BTN_L | BTN_R;

/// A parsed timeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Timeline {
    /// `START lead_us` (the profile's start lead).
    pub lead_us: u32,
    /// The events, last one `NEUTRAL|END`.
    pub events: Vec<(u32, State)>,
}

/// The AD-9 queue-mode allow-list: buttons L R A X Y, D-pad up / down /
/// neutral, any left stick, right stick centred, no reserved flag bit.
#[must_use]
pub fn allowed(s: &State) -> bool {
    s.buttons & !ALLOWED_BUTTONS == 0
        && matches!(s.hat, HAT_UP | HAT_DOWN | HAT_NEUTRAL)
        && s.rx == STICK_CENTER
        && s.ry == STICK_CENTER
        && s.flags & !STATE_FLAG_END == 0
}

/// `NEUTRAL | END`.
#[must_use]
pub fn neutral_end() -> State {
    State {
        flags: STATE_FLAG_END,
        ..NEUTRAL
    }
}

fn hex_state(text: &str) -> Option<State> {
    if text.len() != 16 || !text.is_ascii() {
        return None;
    }
    let mut b = [0u8; 8];
    for (i, slot) in b.iter_mut().enumerate() {
        let pair = text.get(2 * i..2 * i + 2)?;
        *slot = u8::from_str_radix(pair, 16).ok()?;
    }
    Some(State::from_bytes(&b))
}

/// Parses a timeline file.
pub fn parse(text: &str) -> Result<Timeline, String> {
    let mut lead: Option<u32> = None;
    let mut header_seen = false;
    let mut events: Vec<(u32, State)> = Vec::new();
    let mut end_line = None;
    for (idx, raw) in text.lines().enumerate() {
        let n = idx + 1;
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(comment) = line.strip_prefix('#') {
            if let Some(v) = comment.trim().strip_prefix(LEAD_KEY) {
                if lead.is_some() {
                    return Err(format!("line {n}: start_lead_us given twice"));
                }
                lead = Some(
                    v.trim()
                        .parse::<u32>()
                        .map_err(|_| format!("line {n}: bad start_lead_us {:?}", v.trim()))?,
                );
            }
            continue;
        }
        if !header_seen {
            if line != HEADER {
                return Err(format!("line {n}: expected the header {HEADER:?}"));
            }
            header_seen = true;
            continue;
        }
        if let Some(e) = end_line {
            return Err(format!("line {n}: a row after the END row (line {e})"));
        }
        if events.len() >= MAX_ROWS {
            return Err(format!("line {n}: more than {MAX_ROWS} rows"));
        }
        let (t_text, s_text) = line
            .split_once(',')
            .ok_or_else(|| format!("line {n}: expected t_us,state"))?;
        let t_us: u32 = t_text
            .trim()
            .parse()
            .map_err(|_| format!("line {n}: bad t_us {:?}", t_text.trim()))?;
        if t_us >= T_US_LIMIT {
            return Err(format!("line {n}: t_us {t_us} is not below 2^31"));
        }
        if let Some(&(prev, _)) = events.last()
            && t_us <= prev
        {
            return Err(format!(
                "line {n}: t_us {t_us} is not after the previous {prev}"
            ));
        }
        let state = hex_state(s_text.trim())
            .ok_or_else(|| format!("line {n}: state must be 16 hex digits"))?;
        if !allowed(&state) {
            return Err(format!(
                "line {n}: state {} is outside the AD-9 allow-list",
                s_text.trim()
            ));
        }
        if state.has_end() {
            if !state.is_neutral_pose() {
                return Err(format!("line {n}: END on a state that is not neutral"));
            }
            end_line = Some(n);
        }
        events.push((t_us, state));
    }
    if !header_seen {
        return Err(format!("no header {HEADER:?}"));
    }
    let lead_us = lead.ok_or_else(|| "no \"# start_lead_us: N\" line".to_owned())?;
    match events.last() {
        None => Err("no events".to_owned()),
        Some((_, s)) if *s != neutral_end() => {
            Err("the last row must be NEUTRAL|END (0000088080808001)".to_owned())
        }
        Some(_) => Ok(Timeline { lead_us, events }),
    }
}
