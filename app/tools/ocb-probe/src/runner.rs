//! The probe's verdicts (Story 4.4 AC 9, Dev Notes › Probe output and
//! Probe verdict branches), generic over a [`Driver`]: the bench drives the
//! real worker thread, the dry run (`tests/dry_run.rs`) steps the same
//! worker on a fake clock with `FakeBackend`. Both print every line through
//! the driver, so the bench and the dry run share every verdict line
//! (Story 3.4 D9).

use std::fmt::Write as _;
use std::sync::Arc;

use ocb_core::device::{Class, Rules, classify};
use ocb_core::link::LinkEvent;
use ocb_core::protocol::Info;
use ocb_services::port::RawListing;
use ocb_services::worker::{LinkSnapshot, TimedEvent, WorkerEvent};

/// Exit code: PASS (or `list` ok).
pub const EXIT_PASS: u8 = 0;
/// Exit code: FAIL (or a `list` enumeration error).
pub const EXIT_FAIL: u8 = 1;
/// Exit code: usage error.
pub const EXIT_USAGE: u8 = 2;
/// Exit code: interrupted (Ctrl+C).
pub const EXIT_INTERRUPTED: u8 = 130;

/// INFO must come this soon after the port opened, µs.
pub const INFO_AFTER_OPEN_US: u64 = 5_000_000;
/// A `PortBusy` not followed by an open within this long fails, µs.
pub const BUSY_LIMIT_US: u64 = 5_000_000;
/// AC 6: `PortLost` at most this long after the last byte, ms.
pub const LOST_LIMIT_MS: u64 = 1500;
/// AC 6: INFO at most this long after the port is listed again, ms.
pub const RELISTED_INFO_LIMIT_MS: u64 = 1500;
/// AC 7: the permission event at most this long after the listing, ms.
pub const PERMISSION_LIMIT_MS: u64 = 1000;
/// AC 4: the rescan period's median bounds and its maximum, ms.
pub const SCAN_P50_MS: (u64, u64) = (900, 1100);
/// AC 4: the longest rescan period, ms.
pub const SCAN_MAX_MS: u64 = 1500;
/// `alone`: the bridge port must open within this long, µs.
pub const ALONE_FIND_US: u64 = 10_000_000;
/// `alone`: HELLO frames per second at least (one every 100 ms, AD-7).
pub const ALONE_HELLO_PER_S: u64 = 9;

/// The prompt to unplug the bridge.
pub const PROMPT_UNPLUG: &str = "PROMPT: UNPLUG the bridge's USB cable now.";
/// The prompt to plug it back.
pub const PROMPT_PLUG: &str =
    "PROMPT: PLUG the bridge back in (on a Mac, into another USB socket if you have one).";

/// What the runner needs from the worker and the terminal.
pub trait Driver {
    /// The next worker event, waiting at most `timeout_us` of driver time;
    /// `None` on timeout or when interrupted.
    fn next(&mut self, timeout_us: u64) -> Option<TimedEvent>;
    /// Now on the worker's clock, µs.
    fn now_us(&self) -> u64;
    /// The latest worker snapshot.
    fn snapshot(&mut self) -> Option<Arc<LinkSnapshot>>;
    /// Ctrl+C was pressed.
    fn interrupted(&self) -> bool;
    /// `macos`, `windows` or `linux`.
    fn host(&self) -> &'static str;
    /// Prints one line for the owner.
    fn say(&mut self, line: &str);
    /// Ends the worker and the log; returns this run's log lines (from
    /// this process's `start` line on) and the log file's path, or why the
    /// log is unavailable.
    fn finish(&mut self) -> (Result<Vec<String>, String>, String);
}

/// The `EVT` line of an event (`None`: not printed).
#[must_use]
pub fn event_line(t0_us: u64, e: &TimedEvent) -> Option<String> {
    let ms = e.at_us.saturating_sub(t0_us) / 1000;
    let body = match &e.event {
        WorkerEvent::Link(LinkEvent::Status(_)) => return None,
        WorkerEvent::PortListed {
            port,
            class,
            serial,
        } => format!(
            "listed port={port} class={} serial={}",
            class.tag(),
            serial.as_deref().unwrap_or("-")
        ),
        WorkerEvent::PortVanished { port } => format!("vanished port={port}"),
        WorkerEvent::PortOpened {
            port,
            class,
            serial,
        } => format!(
            "opened port={port} class={} serial={}",
            class.tag(),
            serial.as_deref().unwrap_or("-")
        ),
        WorkerEvent::PortClosed { port } => format!("closed port={port}"),
        WorkerEvent::PortLost {
            port,
            reason,
            last_rx_us,
        } => format!(
            "lost port={port} last_rx=+{} reason=\"{reason}\"",
            last_rx_us.map_or_else(
                || "-".to_owned(),
                |t| (t.saturating_sub(t0_us) / 1000).to_string()
            )
        ),
        WorkerEvent::PortRenamed { from, to } => format!("renamed from={from} to={to}"),
        WorkerEvent::ForeignBoard { port, product } => {
            format!("foreign port={port} product=\"{product}\"")
        }
        WorkerEvent::PermissionDenied { port, group, .. } => {
            format!("permission_denied port={port} group={group}")
        }
        WorkerEvent::PortBusy { port } => format!("busy port={port}"),
        WorkerEvent::Link(LinkEvent::Connected(i)) => format!(
            "info proto={} fw={} bridge={}",
            i.proto_ver,
            ver(i.fw_ver),
            ver(i.bridge_ver)
        ),
        WorkerEvent::Link(ev) => format!("link {ev:?}"),
        WorkerEvent::LogUnavailable { reason } => format!("log_unavailable reason=\"{reason}\""),
    };
    Some(format!("EVT +{ms} {body}"))
}

fn ver(v: [u8; 3]) -> String {
    let [a, b, c] = v;
    format!("{a}.{b}.{c}")
}

fn opt(v: Option<u64>) -> String {
    v.map_or_else(|| "-".to_owned(), |n| n.to_string())
}

/// The numbers of one `run`, printed in the final line.
#[derive(Debug, Default, Clone)]
pub struct RunFields {
    port: Option<String>,
    serial: Option<String>,
    renamed: bool,
    info_after_open_ms: Option<u64>,
    lost_after_last_rx_ms: Option<u64>,
    info_after_listed_ms: Option<u64>,
    scans: Vec<(u64, u64)>,
    info: Option<Info>,
}

/// Median and maximum of the periods between scan starts, and the
/// longest listing, ms.
#[must_use]
pub fn scan_stats(scans: &[(u64, u64)]) -> Option<(u64, u64, u64)> {
    let mut periods: Vec<u64> = scans
        .windows(2)
        .filter_map(|w| match w {
            [a, b] => Some(b.0.saturating_sub(a.0) / 1000),
            _ => None,
        })
        .collect();
    if periods.is_empty() {
        return None;
    }
    periods.sort_unstable();
    let p50 = *periods.get(periods.len() / 2)?;
    let max = *periods.last()?;
    let enum_max = scans.iter().map(|s| s.1 / 1000).max().unwrap_or(0);
    Some((p50, max, enum_max))
}

impl RunFields {
    fn text(&self, host: &str) -> String {
        let stats = scan_stats(&self.scans);
        let mut s = format!(
            "host={host} port={} serial={} renamed={} info_after_open_ms={} lost_after_last_rx_ms={} info_after_listed_ms={}",
            self.port.as_deref().unwrap_or("-"),
            self.serial.as_deref().unwrap_or("-"),
            if self.renamed { "yes" } else { "no" },
            opt(self.info_after_open_ms),
            opt(self.lost_after_last_rx_ms),
            opt(self.info_after_listed_ms),
        );
        let _ = write!(
            s,
            " scan_p50_ms={} scan_max_ms={} enum_max_ms={}",
            opt(stats.map(|x| x.0)),
            opt(stats.map(|x| x.1)),
            opt(stats.map(|x| x.2))
        );
        let (fw, bridge, proto) = self.info.map_or_else(
            || ("-".to_owned(), "-".to_owned(), "-".to_owned()),
            |i| (ver(i.fw_ver), ver(i.bridge_ver), i.proto_ver.to_string()),
        );
        let _ = write!(s, " fw={fw} bridge={bridge} proto={proto}");
        s
    }
}

/// How a wait ended.
enum Wait {
    Event(TimedEvent),
    Timeout,
    Interrupted,
}

/// Adds the scans of the latest snapshot not seen yet (the worker keeps
/// the last 64 there; the probe reads it at least every 50 ms).
fn merge_scans(d: &mut dyn Driver, scans: &mut Vec<(u64, u64)>) {
    let Some(snap) = d.snapshot() else {
        return;
    };
    let last = scans.last().map(|s| s.0);
    scans.extend(
        snap.recent_scans
            .iter()
            .filter(|s| last.is_none_or(|l| s.0 > l))
            .copied(),
    );
}

/// Waits for the next event until `deadline_us` (driver clock), printing
/// its `EVT` line and keeping the scan times. An event already waiting is
/// returned even when the deadline has passed, so a timer that expired
/// never hides a queued event (and a caller re-checking its timers never
/// spins without reading).
fn wait(d: &mut dyn Driver, t0: u64, deadline_us: u64, scans: &mut Vec<(u64, u64)>) -> Wait {
    loop {
        if d.interrupted() {
            return Wait::Interrupted;
        }
        let now = d.now_us();
        let got = d.next(deadline_us.saturating_sub(now));
        merge_scans(d, scans);
        match got {
            Some(e) => {
                if let Some(line) = event_line(t0, &e) {
                    d.say(&line);
                }
                return Wait::Event(e);
            }
            None if d.interrupted() => return Wait::Interrupted,
            None if d.now_us() >= deadline_us => return Wait::Timeout,
            None => {}
        }
    }
}

fn interrupted(d: &mut dyn Driver, step: &str) -> u8 {
    let _ = d.finish();
    d.say(&format!("OCB_PROBE INTERRUPTED step={step}"));
    EXIT_INTERRUPTED
}

fn fail_run(d: &mut dyn Driver, f: &RunFields, reason: &str) -> u8 {
    let (_, path) = d.finish();
    let host = d.host();
    d.say(&format!("OCB_PROBE FAIL ({reason}) {}", f.text(host)));
    d.say(&format!("LOG {path}"));
    EXIT_FAIL
}

/// What the run's log must hold (AC 8, H.29), counted only on lines about
/// the ports this run used (` port=<name> `), so lines of another writer of
/// the same file (another `bonooru` on other ports) never count.
#[must_use]
pub fn log_problems(lines: &[String], ports: &[String]) -> Vec<String> {
    let ours = |l: &str| ports.iter().any(|p| l.contains(&format!(" port={p} ")));
    let count = |k: &str, extra: &str| {
        lines
            .iter()
            .filter(|l| l.split(' ').nth(2) == Some(k) && l.contains(extra) && ours(l))
            .count()
    };
    let mut missing = Vec::new();
    for (kind, extra, need) in [
        ("version", " connected port=", 1),
        ("port_opened", "", 2),
        ("port_lost", "", 1),
    ] {
        let n = count(kind, extra);
        if n < need {
            missing.push(format!("{kind} {n} < {need}"));
        }
    }
    missing
}

/// `ocb-probe run` (H.29): connect, unplug, replug, scan period, log.
#[allow(clippy::too_many_lines)]
pub fn run(d: &mut dyn Driver, step_timeout_s: u64) -> u8 {
    let step_us = step_timeout_s * 1_000_000;
    let t0 = d.now_us();
    let mut f = RunFields::default();
    let mut scans = Vec::new();
    let mut run_ports: Vec<String> = Vec::new();
    // ---- connect
    let deadline = t0 + step_us;
    let mut foreign: Vec<String> = Vec::new();
    let mut candidate_seen = false;
    let mut opened: Option<(String, u64)> = None;
    let mut busy: Option<(String, u64)> = None;
    let connected_at;
    loop {
        // Every timer here either fails the run or is cleared when it
        // expires, so the loop never re-waits on a past deadline.
        let mut until = deadline;
        if let Some((_, t)) = &opened {
            until = until.min(t + INFO_AFTER_OPEN_US);
        }
        if let Some((_, t)) = &busy {
            until = until.min(t + BUSY_LIMIT_US);
        }
        let w = wait(d, t0, until, &mut scans);
        f.scans.clone_from(&scans);
        match w {
            Wait::Interrupted => return interrupted(d, "connect"),
            Wait::Timeout => {
                let now = d.now_us();
                if let Some((p, t)) = &opened
                    && now >= t + INFO_AFTER_OPEN_US
                {
                    let r = format!("no INFO within 5 s of opening {p}");
                    return fail_run(d, &f, &r);
                }
                if let Some((p, t)) = &busy
                    && now >= t + BUSY_LIMIT_US
                {
                    let r = format!("port busy: {p}");
                    return fail_run(d, &f, &r);
                }
                if now >= deadline {
                    let r = if !candidate_seen && !foreign.is_empty() {
                        format!("only foreign boards: {}", foreign.join(", "))
                    } else {
                        format!("no bridge within {step_timeout_s} s")
                    };
                    return fail_run(d, &f, &r);
                }
            }
            Wait::Event(e) => match e.event {
                WorkerEvent::PortListed { class, .. } if class.is_candidate() => {
                    candidate_seen = true;
                }
                WorkerEvent::ForeignBoard { product, .. } => foreign.push(product),
                WorkerEvent::PermissionDenied { port, .. } => {
                    let r = format!("permission denied on {port}");
                    return fail_run(d, &f, &r);
                }
                WorkerEvent::PortBusy { port } => {
                    if busy.is_none() && opened.is_none() {
                        busy = Some((port, e.at_us));
                    }
                }
                WorkerEvent::PortOpened { port, serial, .. } => {
                    // Open now: the busy timer is over. The latest open
                    // counts: with several unconfirmed ports the worker
                    // moves on after 2 s, so only the bridge's open can
                    // reach the 5 s limit.
                    busy = None;
                    opened = Some((port.clone(), e.at_us));
                    if !run_ports.contains(&port) {
                        run_ports.push(port.clone());
                    }
                    f.port = Some(port);
                    f.serial = serial;
                }
                WorkerEvent::Link(LinkEvent::RequiredUpdate {
                    controller_proto,
                    app_proto,
                    info,
                }) => {
                    f.info = Some(info);
                    let r = format!(
                        "protocol mismatch: controller {controller_proto}, app {app_proto}"
                    );
                    return fail_run(d, &f, &r);
                }
                WorkerEvent::Link(LinkEvent::Connected(info)) => {
                    f.info = Some(info);
                    let open_at = opened.as_ref().map_or(e.at_us, |o| o.1);
                    f.info_after_open_ms = Some(e.at_us.saturating_sub(open_at) / 1000);
                    connected_at = e.at_us;
                    break;
                }
                _ => {}
            },
        }
    }
    let port = f.port.clone().unwrap_or_default();
    d.say(&format!(
        "STEP connect PASS port={port} serial={} info_after_open_ms={}",
        f.serial.as_deref().unwrap_or("-"),
        opt(f.info_after_open_ms)
    ));
    // ---- unplug: the port is lost AND leaves the port list (a physical
    // unplug, not a read glitch on a port that stays there).
    d.say(PROMPT_UNPLUG);
    let deadline = d.now_us() + step_us;
    let mut lost = false;
    let mut reopened: Option<(String, Option<String>)> = None;
    loop {
        let w = wait(d, t0, deadline, &mut scans);
        f.scans.clone_from(&scans);
        match w {
            Wait::Interrupted => return interrupted(d, "unplug"),
            Wait::Timeout => {
                d.say(&format!("STEP unplug FAIL timeout_s={step_timeout_s}"));
                let r = if lost {
                    format!(
                        "port lost but still listed after {step_timeout_s} s (no physical unplug)"
                    )
                } else {
                    format!("unplug not seen within {step_timeout_s} s")
                };
                return fail_run(d, &f, &r);
            }
            Wait::Event(e) => match &e.event {
                WorkerEvent::PortLost {
                    port: p,
                    last_rx_us,
                    ..
                } if *p == port && !lost => {
                    lost = true;
                    let last = last_rx_us.unwrap_or(connected_at);
                    let ms = e.at_us.saturating_sub(last) / 1000;
                    f.lost_after_last_rx_ms = Some(ms);
                    if ms > LOST_LIMIT_MS {
                        d.say(&format!("STEP unplug FAIL lost_after_last_rx_ms={ms}"));
                        let r = format!(
                            "loss detected {ms} ms after the last byte (limit {LOST_LIMIT_MS})"
                        );
                        return fail_run(d, &f, &r);
                    }
                }
                WorkerEvent::PortVanished { port: p } if *p == port && lost => {
                    d.say(&format!(
                        "STEP unplug PASS lost_after_last_rx_ms={}",
                        opt(f.lost_after_last_rx_ms)
                    ));
                    break;
                }
                WorkerEvent::PortOpened {
                    port: p, serial, ..
                } => {
                    reopened = Some((p.clone(), serial.clone()));
                }
                WorkerEvent::Link(LinkEvent::Connected(_)) => {
                    d.say("STEP unplug FAIL reconnected");
                    let r = match &reopened {
                        Some((p, serial)) if *serial != f.serial => format!(
                            "reconnected to another bridge: {p} serial {} (not the one unplugged, listed again)",
                            serial.as_deref().unwrap_or("-")
                        ),
                        _ => "reconnected without leaving the port list (no physical unplug)"
                            .to_owned(),
                    };
                    return fail_run(d, &f, &r);
                }
                _ => {}
            },
        }
    }
    // ---- replug: the same bridge listed again, then INFO.
    d.say(PROMPT_PLUG);
    let deadline = d.now_us() + step_us;
    let mut listed_at: Option<u64> = None;
    let mut opened_serial: Option<String> = None;
    loop {
        let w = wait(d, t0, deadline, &mut scans);
        f.scans.clone_from(&scans);
        match w {
            Wait::Interrupted => return interrupted(d, "replug"),
            Wait::Timeout => {
                d.say(&format!("STEP replug FAIL timeout_s={step_timeout_s}"));
                let r = if listed_at.is_some() {
                    format!("listed again but no INFO within {step_timeout_s} s")
                } else {
                    format!("replug not seen within {step_timeout_s} s")
                };
                return fail_run(d, &f, &r);
            }
            Wait::Event(e) => match e.event {
                WorkerEvent::PortListed { class, serial, .. }
                    if class.is_candidate() && serial == f.serial =>
                {
                    listed_at = Some(e.at_us);
                }
                WorkerEvent::PortRenamed { .. } => f.renamed = true,
                WorkerEvent::PortOpened {
                    port: p, serial, ..
                } => {
                    if !run_ports.contains(&p) {
                        run_ports.push(p.clone());
                    }
                    opened_serial = serial;
                    f.port = Some(p);
                }
                WorkerEvent::Link(LinkEvent::RequiredUpdate {
                    controller_proto,
                    app_proto,
                    ..
                }) => {
                    let r = format!(
                        "protocol mismatch: controller {controller_proto}, app {app_proto}"
                    );
                    return fail_run(d, &f, &r);
                }
                WorkerEvent::Link(LinkEvent::Connected(info)) => {
                    f.info = Some(info);
                    let Some(base) = listed_at.filter(|_| opened_serial == f.serial) else {
                        d.say("STEP replug FAIL other bridge");
                        let r = format!(
                            "reconnected to another bridge: {} serial {} (not the one unplugged, listed again)",
                            f.port.as_deref().unwrap_or("-"),
                            opened_serial.as_deref().unwrap_or("-")
                        );
                        return fail_run(d, &f, &r);
                    };
                    let ms = e.at_us.saturating_sub(base) / 1000;
                    f.info_after_listed_ms = Some(ms);
                    if ms > RELISTED_INFO_LIMIT_MS {
                        d.say(&format!("STEP replug FAIL info_after_listed_ms={ms}"));
                        let r = format!(
                            "INFO {ms} ms after the port was listed (limit {RELISTED_INFO_LIMIT_MS})"
                        );
                        return fail_run(d, &f, &r);
                    }
                    d.say(&format!(
                        "STEP replug PASS port={} renamed={} info_after_listed_ms={ms}",
                        f.port.as_deref().unwrap_or("-"),
                        if f.renamed { "yes" } else { "no" }
                    ));
                    break;
                }
                _ => {}
            },
        }
    }
    // ---- rescan period (AC 4)
    match scan_stats(&f.scans) {
        Some((p50, max, _))
            if (SCAN_P50_MS.0..=SCAN_P50_MS.1).contains(&p50) && max <= SCAN_MAX_MS => {}
        stats => {
            let (p50, max) = stats.map_or((0, 0), |s| (s.0, s.1));
            let r = format!(
                "rescan period p50 {p50} ms / max {max} ms (limits {}–{} / {SCAN_MAX_MS})",
                SCAN_P50_MS.0, SCAN_P50_MS.1
            );
            return fail_run(d, &f, &r);
        }
    }
    // ---- log
    let (lines, path) = d.finish();
    let host = d.host();
    let problems = match &lines {
        Ok(l) => log_problems(l, &run_ports),
        Err(e) => vec![format!("log unavailable ({e})")],
    };
    if problems.is_empty() {
        d.say("STEP log PASS");
        d.say(&format!("OCB_PROBE PASS {}", f.text(host)));
        d.say(&format!("LOG {path}"));
        EXIT_PASS
    } else {
        let what = problems.join(", ");
        d.say(&format!("STEP log FAIL {what}"));
        d.say(&format!(
            "OCB_PROBE FAIL (log check: {what}) {}",
            f.text(host)
        ));
        d.say(&format!("LOG {path}"));
        EXIT_FAIL
    }
}

/// `ocb-probe run --expect-permission-denied` (H.30).
pub fn permission(d: &mut dyn Driver, timeout_s: u64) -> u8 {
    let t0 = d.now_us();
    let deadline = t0 + timeout_s * 1_000_000;
    let mut scans = Vec::new();
    let mut listed: Vec<(String, u64)> = Vec::new();
    let verdict = loop {
        match wait(d, t0, deadline, &mut scans) {
            Wait::Interrupted => return interrupted(d, "permission"),
            Wait::Timeout => break Err(format!("no permission event within {timeout_s} s")),
            Wait::Event(e) => match e.event {
                WorkerEvent::PortListed { port, .. } => listed.push((port, e.at_us)),
                WorkerEvent::PortOpened { port, .. } => {
                    break Err(format!("port opened: the user can already open {port}"));
                }
                WorkerEvent::PermissionDenied {
                    port,
                    group,
                    message,
                } => {
                    let at_listed = listed
                        .iter()
                        .rev()
                        .find(|l| l.0 == port)
                        .map_or(e.at_us, |l| l.1);
                    let ms = e.at_us.saturating_sub(at_listed) / 1000;
                    if ms > PERMISSION_LIMIT_MS {
                        break Err(format!(
                            "permission event {ms} ms after the port was listed (limit {PERMISSION_LIMIT_MS})"
                        ));
                    }
                    break Ok((port, group, ms, message));
                }
                _ => {}
            },
        }
    };
    let _ = d.finish();
    match verdict {
        Ok((port, group, ms, message)) => {
            d.say(&format!(
                "OCB_PROBE PASS permission port={port} group={group} after_listed_ms={ms}"
            ));
            d.say(&format!("MSG {message}"));
            EXIT_PASS
        }
        Err(r) => {
            d.say(&format!("OCB_PROBE FAIL ({r})"));
            EXIT_FAIL
        }
    }
}

/// `ocb-probe alone` (H.31): the bridge without a powered controller.
/// The verdict judges the session's HELLO cadence (frames handed out);
/// frames the write rules dropped are reported (`hello_dropped=`), expected
/// while the bridge does not read USB.
pub fn alone(d: &mut dyn Driver, seconds: u64) -> u8 {
    let t0 = d.now_us();
    let mut scans = Vec::new();
    let verdict = alone_verdict(d, t0, seconds, &mut scans);
    let _ = d.finish();
    match verdict {
        Ok(Some(line)) => {
            d.say(&line);
            EXIT_PASS
        }
        Ok(None) => {
            d.say("OCB_PROBE INTERRUPTED step=alone");
            EXIT_INTERRUPTED
        }
        Err(r) => {
            d.say(&format!("OCB_PROBE FAIL ({r})"));
            EXIT_FAIL
        }
    }
}

fn alone_verdict(
    d: &mut dyn Driver,
    t0: u64,
    seconds: u64,
    scans: &mut Vec<(u64, u64)>,
) -> Result<Option<String>, String> {
    let find_deadline = t0 + ALONE_FIND_US;
    let (first, class, opened_at) = loop {
        match wait(d, t0, find_deadline, scans) {
            Wait::Interrupted => return Ok(None),
            Wait::Timeout => return Err("no bridge port".to_owned()),
            Wait::Event(e) => match e.event {
                WorkerEvent::PortOpened { port, class, .. } => break (port, class, e.at_us),
                WorkerEvent::Link(LinkEvent::Connected(_)) => {
                    return Err("INFO received: the controller is powered".to_owned());
                }
                _ => {}
            },
        }
    };
    // With several unconfirmed bridges (Windows, several Pico boards) the
    // worker probes each in turn (AC 4): every one of them is listed.
    let mut ports = vec![first];
    let end = opened_at + seconds * 1_000_000;
    loop {
        match wait(d, t0, end, scans) {
            Wait::Interrupted => return Ok(None),
            Wait::Timeout => break,
            Wait::Event(e) => match e.event {
                WorkerEvent::Link(LinkEvent::Connected(_)) => {
                    return Err("INFO received: the controller is powered".to_owned());
                }
                WorkerEvent::PortOpened { port, .. } if !ports.contains(&port) => ports.push(port),
                _ => {}
            },
        }
    }
    let snap = d
        .snapshot()
        .ok_or_else(|| "no worker snapshot".to_owned())?;
    let not_bridge = |p: &str| {
        snap.ports
            .iter()
            .find(|s| s.info.name == p)
            .is_some_and(|s| !s.class.is_candidate())
    };
    if let Some((p, _)) = snap
        .port_writes
        .iter()
        .find(|(p, n)| *n > 0 && not_bridge(p))
    {
        return Err(format!("wrote to a port that is not a bridge: {p}"));
    }
    let sent = snap.counters.hello_sent;
    let dropped = snap.counters.hello_dropped;
    if sent < ALONE_HELLO_PER_S * seconds {
        return Err(format!(
            "only {sent} HELLO frames in {seconds} s (expected ≥ {ALONE_HELLO_PER_S} per s)"
        ));
    }
    Ok(Some(format!(
        "OCB_PROBE PASS alone port={} class={} hello_frames={sent} hello_dropped={dropped} seconds={seconds} other_ports_written=0",
        ports.join(","),
        class.tag()
    )))
}

/// The `PORT` lines and the `OCB_PORTS` summary of `ocb-probe list`
/// (H.28), and the exit code.
#[must_use]
pub fn list_lines(
    listing: Result<Vec<RawListing>, String>,
    rules: Rules,
    host: &str,
) -> (Vec<String>, u8) {
    let ports = match listing {
        Ok(p) => p,
        Err(e) => return (vec![format!("OCB_PORTS error {e}")], EXIT_FAIL),
    };
    let mut lines = Vec::new();
    let (mut ours, mut unconfirmed, mut foreign) = (0, 0, 0);
    for l in &ports {
        let class = classify(&l.info, rules);
        match class {
            Class::Ours => ours += 1,
            Class::Unconfirmed => unconfirmed += 1,
            Class::Foreign { .. } => foreign += 1,
            Class::Ignored => {}
        }
        let mut line = format!("PORT name={}", l.info.name);
        match &l.info.usb {
            None => line.push_str(" usb=-"),
            Some(u) => {
                let _ = write!(
                    line,
                    " usb={:04X}:{:04X} serial={} manufacturer=\"{}\" product=\"{}\"",
                    u.vid,
                    u.pid,
                    u.serial.as_deref().unwrap_or("-"),
                    u.manufacturer.as_deref().unwrap_or("-"),
                    u.product.as_deref().unwrap_or("-")
                );
            }
        }
        if let Some(raw) = &l.product_raw {
            let _ = write!(line, " product_raw=\"{raw}\"");
        }
        let _ = write!(line, " class={}", class.tag());
        lines.push(line);
    }
    lines.push(format!(
        "OCB_PORTS n={} ours={ours} unconfirmed={unconfirmed} foreign={foreign} host={host}",
        ports.len()
    ));
    (lines, EXIT_PASS)
}

/// The host name used in probe lines.
#[must_use]
pub const fn host_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(windows) {
        "windows"
    } else {
        "linux"
    }
}
