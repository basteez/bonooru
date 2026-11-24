//! H.3 (Story 3.5 Task 3): records `console_ready` and `link_ok` edges,
//! STATUS silences (the console cuts USB power in sleep) and controller
//! reboots, and probes `START` while the console is not ready (it must get
//! `NAK NOT_READY`). Runs until Ctrl+C or its limit, then judges AC3 and
//! prints the OQ-3 / OQ-4 data.
//!
//! The OFF criterion is measured by the controller itself: the debug v5
//! `in_age_ms` of the first not-ready STATUS is the time since the console's
//! last poll, so "back to 0 within 150 ms" is that age ≤ 150 ms (the 100 ms
//! rule plus one 50 ms STATUS period).

use ocb_core::link::{AbandonReason, Phase, StatusView};
use ocb_core::protocol::NakReason;

use super::{Io, Owner, Runner, Step, Stop};
use crate::timeline::neutral_end;

/// No STATUS for this long: "STATUS stopped" (six periods).
const SILENCE_US: u64 = 300_000;
/// AC3: `console_ready` back to 0 within 150 ms of the console's last poll.
const FALL_LIMIT_MS: u16 = 150;
/// A probe at most this often (a short not-ready window after wake, about
/// 1.7 s in H.16, still gets several).
const PROBE_EVERY_US: u64 = 500_000;
/// After a silence, probe for the reboot this long after STATUS resumed if
/// the session has not seen it by itself; at most this many times.
const REBOOT_PROBE_AFTER_US: u64 = 1_000_000;
const REBOOT_PROBES_MAX: u32 = 5;

/// One STATUS silence.
#[derive(Debug, Default, Clone, Copy)]
struct Silence {
    len_us: u64,
    reboot_seen: bool,
    ready_after_reboot: bool,
    probes: u32,
}

/// What `watch` saw.
#[derive(Debug, Default)]
pub(super) struct WatchLog {
    pub(super) active: bool,
    start: u64,
    info_at: Option<u64>,
    first_ready: Option<bool>,
    last_status_t: Option<u64>,
    last_ready: Option<bool>,
    last_link: Option<bool>,
    silent_since: Option<u64>,
    resume_at: Option<u64>,
    silences: Vec<Silence>,
    /// Per 1 → 0 edge with STATUS flowing: (v5 `in_age_ms`, a 0 → 1 edge
    /// came after it).
    falls: Vec<(Option<u16>, bool)>,
    rises: Vec<u64>,
    gaps: Vec<u64>,
    probes: u32,
    probe_pending: bool,
    probes_not_ready: u32,
    probes_played: u32,
    probe_other: Vec<String>,
    reboots: u32,
    next_probe: u64,
    /// `in_max_us` / `in_slow` of the last STATUS before the first OFF fall
    /// (later values include the OFF stall itself).
    in_before_off: Option<(Option<u16>, Option<u16>)>,
    in_last: (Option<u16>, Option<u16>),
    port_error: Option<String>,
}

fn ms(us: u64) -> f64 {
    us as f64 / 1000.0
}

fn percentile(sorted: &[u64], p: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted.get(idx).copied().unwrap_or(0)
}

fn join<T>(items: &[T], f: impl Fn(&T) -> String) -> String {
    items.iter().map(f).collect::<Vec<_>>().join(", ")
}

impl<I: Io, O: Owner> Runner<'_, I, O> {
    fn wsay(&mut self, now: u64, what: &str) {
        let t = ms(now.saturating_sub(self.watch.start));
        self.owner.say(&format!("{t:>10.1} ms  {what}"));
    }

    pub(super) fn watch(&mut self, max_us: u64) -> Step<String> {
        let now = self.io.now_us();
        self.watch.active = true;
        self.watch.start = now;
        self.s.open(now);
        self.owner.say(
            "WATCH (H.3): plug the controller into the console, then sleep, wake, Wired Communication \
             OFF and ON, one at a time; Ctrl+C ends and judges",
        );
        let end = now + max_us;
        loop {
            self.check_interrupt()?;
            let now = self.io.now_us();
            if now >= end {
                break;
            }
            if self.watch.info_at.is_none()
                && let Some(i) = self.info
            {
                self.watch.info_at = Some(now);
                self.wsay(
                    now,
                    &format!(
                        "INFO: fw_ver {:?} report_period_us {}",
                        i.fw_ver, i.report_period_us
                    ),
                );
            }
            // Silence: the controller lost power (sleep) or was unplugged.
            if let Some(t) = self.watch.last_status_t
                && self.watch.silent_since.is_none()
                && now > t + SILENCE_US
            {
                self.watch.silent_since = Some(t);
                self.watch.probe_pending = false;
                self.wsay(now, "STATUS stopped (the controller lost power?)");
            }
            self.probe(now);
            if let Err(e) = self.pump(now + 20_000) {
                // Keep what was recorded: judge it, naming the port error.
                self.wsay(now, &format!("port error: {e}"));
                self.watch.port_error = Some(e);
                break;
            }
        }
        self.watch_verdict()
    }

    /// The NOT_READY probe, and the probe that lets the session see a
    /// reboot after a silence.
    fn probe(&mut self, now: u64) {
        if self.watch.probe_pending
            || self.s.phase() != Phase::Connected
            || self.s.is_playing()
            || now < self.watch.next_probe
        {
            return;
        }
        let Some(st) = self.status else { return };
        if !st.link_ok || self.watch.silent_since.is_some() {
            return;
        }
        let reboot_unseen = self
            .watch
            .silences
            .last()
            .is_some_and(|s| !s.reboot_seen && s.probes < REBOOT_PROBES_MAX)
            && self
                .watch
                .resume_at
                .is_some_and(|t| now >= t + REBOOT_PROBE_AFTER_US);
        let not_ready = !st.console_ready;
        if !not_ready && !reboot_unseen {
            return;
        }
        if self.s.queue_event(0, neutral_end()).is_err() || self.s.start(0).is_err() {
            return;
        }
        self.watch.next_probe = now + PROBE_EVERY_US;
        self.watch.probe_pending = true;
        if not_ready {
            self.watch.probes += 1;
            self.wsay(now, "probe: START while console_ready 0");
        } else {
            if let Some(s) = self.watch.silences.last_mut() {
                s.probes += 1;
            }
            self.wsay(
                now,
                "probe: START after the silence (does the session see a reboot?)",
            );
        }
    }

    pub(super) fn watch_status(&mut self, st: &StatusView, now: u64) {
        let gap = self.watch.last_status_t.map(|t| now - t);
        let gap_text = gap.map_or_else(|| "first".to_owned(), |g| format!("+{:.0} ms", ms(g)));
        if let Some(since) = self.watch.silent_since.take() {
            let len = now - since;
            self.watch.silences.push(Silence {
                len_us: len,
                ..Silence::default()
            });
            self.watch.resume_at = Some(now);
            self.wsay(now, &format!("STATUS resumed after {:.0} ms", ms(len)));
        } else if let Some(g) = gap
            && st.link_ok
            && self.info.is_some()
        {
            self.watch.gaps.push(g);
        }
        let dbg = self.tap.last;
        let in_now = (dbg.and_then(|d| d.in_max_us), dbg.and_then(|d| d.in_slow));
        if self.watch.first_ready.is_none() && self.watch.info_at.is_some() {
            self.watch.first_ready = Some(st.console_ready);
            self.wsay(
                now,
                &format!(
                    "first STATUS after INFO: console_ready {}",
                    u8::from(st.console_ready)
                ),
            );
        }
        if self.watch.last_link != Some(st.link_ok) {
            self.wsay(
                now,
                &format!("link_ok {} ({gap_text})", u8::from(st.link_ok)),
            );
        }
        match (self.watch.last_ready, st.console_ready) {
            (Some(true), false) => {
                let flowing = gap.is_some_and(|g| g <= SILENCE_US);
                let age = dbg.and_then(|d| d.in_age_ms);
                if flowing {
                    self.watch.falls.push((age, false));
                    if self.watch.in_before_off.is_none() {
                        self.watch.in_before_off = Some(self.watch.in_last);
                    }
                }
                self.wsay(
                    now,
                    &format!(
                        "console_ready 1 -> 0 ({gap_text}), {} since the console's last poll{}",
                        age.map_or("? ms (no v5)".to_owned(), |a| format!("{a} ms")),
                        if flowing { "" } else { " (after a silence)" }
                    ),
                );
            }
            (prev, true) if prev != Some(true) => {
                let from = self.watch.resume_at.or(self.watch.info_at).unwrap_or(now);
                let since = now.saturating_sub(from);
                self.watch.rises.push(since);
                if let Some(f) = self.watch.falls.last_mut() {
                    f.1 = true;
                }
                if let Some(s) = self.watch.silences.last_mut()
                    && s.reboot_seen
                {
                    s.ready_after_reboot = true;
                }
                self.wsay(
                    now,
                    &format!(
                        "console_ready 0 -> 1 ({gap_text}), {:.0} ms after {}",
                        ms(since),
                        if self.watch.resume_at.is_some() {
                            "STATUS resumed"
                        } else {
                            "INFO"
                        }
                    ),
                );
                self.watch.resume_at = None;
            }
            _ => {}
        }
        self.watch.in_last = in_now;
        self.watch.last_ready = Some(st.console_ready);
        self.watch.last_link = Some(st.link_ok);
        self.watch.last_status_t = Some(now);
    }

    pub(super) fn watch_abandon(&mut self, reason: AbandonReason, now: u64) {
        let pending = std::mem::take(&mut self.watch.probe_pending);
        match reason {
            AbandonReason::Nak(NakReason::NotReady) if pending => {
                self.watch.probes_not_ready += 1;
                self.wsay(now, "probe answered NAK NOT_READY");
            }
            AbandonReason::ControllerRebooted => {
                if pending {
                    self.uncount_probe();
                }
                self.wsay(now, "probe abandoned: the controller rebooted");
            }
            other => {
                if pending {
                    self.watch.probe_other.push(format!("{other:?}"));
                }
                self.wsay(now, &format!("piece abandoned: {other:?}"));
            }
        }
    }

    /// A probe answered by the reboot (`NAK VERSION`) tested no NOT_READY.
    fn uncount_probe(&mut self) {
        if self.status.is_some_and(|s| !s.console_ready) {
            self.watch.probes = self.watch.probes.saturating_sub(1);
        }
    }

    /// The END of a probe was played: the console became ready while the
    /// probe was on its way (not a refusal to judge).
    pub(super) fn watch_played(&mut self, now: u64) {
        if std::mem::take(&mut self.watch.probe_pending) {
            self.watch.probes = self.watch.probes.saturating_sub(1);
            self.watch.probes_played += 1;
            self.wsay(now, "probe played (the console became ready meanwhile)");
        }
    }

    pub(super) fn watch_reboot(&mut self, now: u64) {
        self.watch.reboots += 1;
        if std::mem::take(&mut self.watch.probe_pending) {
            self.uncount_probe();
        }
        if let Some(s) = self.watch.silences.last_mut() {
            s.reboot_seen = true;
        }
        self.wsay(
            now,
            "the session reports: controller rebooted (HELLO again)",
        );
    }

    /// The analysis and the AC3 verdict.
    pub(super) fn watch_verdict(&mut self) -> Step<String> {
        let w = &self.watch;
        let mut gaps = w.gaps.clone();
        gaps.sort_unstable();
        let opt = |v: Option<u16>| v.map_or("-".to_owned(), |x| x.to_string());
        let (in_max_pre, in_slow_pre) = w.in_before_off.unwrap_or(w.in_last);
        let lines = [
            format!(
                "first STATUS after INFO: console_ready {}",
                w.first_ready
                    .map_or("-".to_owned(), |r| u8::from(r).to_string())
            ),
            format!(
                "console_ready 0 -> 1 edges: {} ({})",
                w.rises.len(),
                join(&w.rises, |r| format!("{:.0} ms", ms(*r)))
            ),
            format!(
                "console_ready 1 -> 0 with STATUS flowing: {} (age of the last poll at the fall: {}; limit {FALL_LIMIT_MS} ms)",
                w.falls.len(),
                join(&w.falls, |f| format!(
                    "{}{}",
                    f.0.map_or("? (no v5)".to_owned(), |a| format!("{a} ms")),
                    if f.1 { "" } else { ", never ready again" }
                ))
            ),
            format!(
                "STATUS silences: {} ({})",
                w.silences.len(),
                join(&w.silences, |s| format!(
                    "{:.0} ms, reboot {}, ready after it {}",
                    ms(s.len_us),
                    if s.reboot_seen { "seen" } else { "NOT seen" },
                    if s.ready_after_reboot { "yes" } else { "no" }
                ))
            ),
            format!(
                "NOT_READY probes: {} sent, {} NAK NOT_READY, {} played (ready meanwhile), other: {}",
                w.probes,
                w.probes_not_ready,
                w.probes_played,
                if w.probe_other.is_empty() {
                    "none".to_owned()
                } else {
                    w.probe_other.join(", ")
                }
            ),
            format!("controller rebooted (session): {}", w.reboots),
            format!(
                "STATUS interval with the link up: n {}, median {:.1} ms, p99 {:.1} ms, max {:.1} ms (OQ-3)",
                gaps.len(),
                ms(percentile(&gaps, 0.5)),
                ms(percentile(&gaps, 0.99)),
                ms(gaps.last().copied().unwrap_or(0))
            ),
            format!(
                "console polling before the first OFF (debug v4): in_max_us {}, in_slow {} (OQ-4)",
                opt(in_max_pre),
                opt(in_slow_pre)
            ),
        ];
        let mut fails = Vec::new();
        if let Some(e) = &w.port_error {
            fails.push(format!("the port failed: {e}"));
        }
        if w.first_ready != Some(false) {
            fails.push(
                "the first STATUS after INFO was not console_ready 0 (plug in after starting)"
                    .to_owned(),
            );
        }
        if w.falls.is_empty() {
            fails.push(
                "no console_ready 1 -> 0 with STATUS flowing (Wired Communication OFF)".to_owned(),
            );
        }
        for f in &w.falls {
            match f.0 {
                None => fails.push(
                    "a fall without the v5 debug block (flash a Story 3.5 debug build)".to_owned(),
                ),
                Some(a) if a > FALL_LIMIT_MS => {
                    fails.push(format!("a 1 -> 0 edge {a} ms after the last poll"))
                }
                Some(_) => {}
            }
            if !f.1 {
                fails.push(
                    "console_ready not 1 again after a fall (Wired Communication ON)".to_owned(),
                );
            }
        }
        if w.silences.is_empty() {
            fails.push("no STATUS silence (sleep)".to_owned());
        }
        if w.silences
            .iter()
            .any(|s| !s.reboot_seen || !s.ready_after_reboot)
        {
            fails
                .push("a silence without a reported reboot followed by console_ready 1".to_owned());
        }
        if w.probes == 0 || w.probes_not_ready != w.probes || !w.probe_other.is_empty() {
            fails.push(format!(
                "probes: {} sent, {} NOT_READY, other {:?}",
                w.probes, w.probes_not_ready, w.probe_other
            ));
        }
        for l in lines {
            self.owner.say(&format!("H.3: {l}"));
        }
        if fails.is_empty() {
            Ok("PASS".to_owned())
        } else {
            Err(Stop::Fail(fails.join("; ")))
        }
    }
}
