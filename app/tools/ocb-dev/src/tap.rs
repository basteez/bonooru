//! A second reader of the received bytes (the session exposes neither):
//! the STATUS debug counters (`docs/link-protocol.md`, v1 bytes 0–6, v2
//! bytes 7–20) and the NAKs by reason.

use std::collections::BTreeMap;

use ocb_core::protocol::consts::{MSG_NAK, MSG_STATUS};
use ocb_core::protocol::{Message, NakReason, RxEvent, RxSide, Splitter};

/// The debug counters a fw-debug STATUS carries (v1 and v2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DebugCounters {
    /// Largest gap between two app frames, ms.
    pub max_app_gap_ms: u16,
    /// Link watchdog firings (wraps).
    pub watchdog_trips: u8,
    /// `rx_err_bytes + rx_overflow_bytes + overlong_bytes`.
    pub dropped_bytes: u16,
    /// EVENT/NAK entries dropped and output discarded while TX was gated.
    pub tx_dropped: u16,
    /// v2: characters with an error bit.
    pub rx_err_bytes: Option<u16>,
    /// v2: characters lost to a full receive buffer.
    pub rx_overflow_bytes: Option<u16>,
    /// v2: bytes of overlong runs.
    pub overlong_bytes: Option<u16>,
    /// v2: break characters seen.
    pub rx_be: Option<u16>,
    /// v4: the longest interval between two console polls (IN reports), µs.
    pub in_max_us: Option<u16>,
    /// v4: intervals longer than 1.5 × the median (missed polls).
    pub in_slow: Option<u16>,
    /// v5 (Story 3.5): the last link loss, from the last heartbeat to the
    /// abort, in 10 µs units (0 before any loss).
    pub loss_age_10us: Option<u16>,
    /// v5: 0 no loss yet, 1 heartbeat timeout, 2 break.
    pub loss_kind: Option<u8>,
    /// v5: ms since the last console poll when the STATUS was built.
    pub in_age_ms: Option<u16>,
}

impl DebugCounters {
    /// From a STATUS `extra` tail; `None` when shorter than the v1 block.
    #[must_use]
    pub fn parse(extra: &[u8]) -> Option<Self> {
        let u16_at = |i: usize| -> Option<u16> {
            Some(u16::from_le_bytes([*extra.get(i)?, *extra.get(i + 1)?]))
        };
        let mut d = Self {
            max_app_gap_ms: u16_at(0)?,
            watchdog_trips: *extra.get(2)?,
            dropped_bytes: u16_at(3)?,
            tx_dropped: u16_at(5)?,
            ..Self::default()
        };
        if extra.len() >= 21 {
            d.rx_err_bytes = u16_at(7);
            d.rx_overflow_bytes = u16_at(9);
            d.overlong_bytes = u16_at(11);
            d.rx_be = u16_at(17);
        }
        // v3: the P2 log window (`p2_log_n` records of 2 bytes from byte
        // 24), then the fixed v4 block: in_max_us at +28, in_slow at +30.
        if let Some(&n) = extra.get(23) {
            let v4 = 24 + 2 * usize::from(n);
            if extra.len() >= v4 + 41 {
                d.in_max_us = u16_at(v4 + 28);
                d.in_slow = u16_at(v4 + 30);
            }
            // v5, right after the 41-byte v4 block (5 bytes).
            let v5 = v4 + 41;
            if extra.len() >= v5 + 5 {
                d.loss_age_10us = u16_at(v5);
                d.loss_kind = extra.get(v5 + 2).copied();
                d.in_age_ms = u16_at(v5 + 3);
            }
        }
        Some(d)
    }

    /// The v2 split is present.
    #[must_use]
    pub fn has_v2(&self) -> bool {
        self.rx_err_bytes.is_some()
    }

    /// One line, `k=v`.
    #[must_use]
    pub fn line(&self) -> String {
        let opt = |v: Option<u16>| v.map_or_else(|| "-".to_owned(), |x| x.to_string());
        format!(
            "max_app_gap_ms={} watchdog_trips={} dropped_bytes={} tx_dropped={} rx_err_bytes={} rx_overflow_bytes={} overlong_bytes={}",
            self.max_app_gap_ms,
            self.watchdog_trips,
            self.dropped_bytes,
            self.tx_dropped,
            opt(self.rx_err_bytes),
            opt(self.rx_overflow_bytes),
            opt(self.overlong_bytes)
        )
    }

    /// The counters that must not move during a soak, by name, where they
    /// differ from `later`.
    #[must_use]
    pub fn moved(&self, later: &Self) -> Vec<String> {
        let pairs = [
            (
                "dropped_bytes",
                Some(self.dropped_bytes),
                Some(later.dropped_bytes),
            ),
            ("tx_dropped", Some(self.tx_dropped), Some(later.tx_dropped)),
            (
                "watchdog_trips",
                Some(u16::from(self.watchdog_trips)),
                Some(u16::from(later.watchdog_trips)),
            ),
            ("rx_err_bytes", self.rx_err_bytes, later.rx_err_bytes),
            (
                "rx_overflow_bytes",
                self.rx_overflow_bytes,
                later.rx_overflow_bytes,
            ),
            ("overlong_bytes", self.overlong_bytes, later.overlong_bytes),
        ];
        pairs
            .iter()
            .filter(|(_, a, b)| a != b)
            .map(|(k, a, b)| format!("{k} {a:?} -> {b:?}"))
            .collect()
    }
}

/// The tap.
#[derive(Debug)]
pub struct Tap {
    splitter: Splitter,
    /// The first STATUS debug counters seen.
    pub first: Option<DebugCounters>,
    /// The latest.
    pub last: Option<DebugCounters>,
    /// STATUS frames seen.
    pub statuses: u64,
    /// STATUS frames without a debug tail.
    pub statuses_plain: u64,
    /// NAKs received, by raw reason.
    pub naks: BTreeMap<u8, u64>,
}

impl Default for Tap {
    fn default() -> Self {
        Self::new()
    }
}

impl Tap {
    /// An empty tap.
    #[must_use]
    pub fn new() -> Self {
        Self {
            splitter: Splitter::new(RxSide::App),
            first: None,
            last: None,
            statuses: 0,
            statuses_plain: 0,
            naks: BTreeMap::new(),
        }
    }

    /// Forgets a partial frame (a new port after an unplug).
    pub fn reset_splitter(&mut self) {
        self.splitter = Splitter::new(RxSide::App);
    }

    /// Feeds received bytes.
    pub fn feed(&mut self, bytes: &[u8]) {
        for &b in bytes {
            let RxEvent::Frame(f) = self.splitter.push(b) else {
                continue;
            };
            match Message::decode(f.ty, &f.body) {
                Ok(Message::Status(s)) if f.ty == MSG_STATUS => {
                    self.statuses += 1;
                    match DebugCounters::parse(&s.extra) {
                        Some(d) => {
                            if self.first.is_none() {
                                self.first = Some(d);
                            }
                            self.last = Some(d);
                        }
                        None => self.statuses_plain += 1,
                    }
                }
                Ok(Message::Nak(n)) if f.ty == MSG_NAK => {
                    *self.naks.entry(n.reason.to_u8()).or_insert(0) += 1;
                }
                _ => {}
            }
        }
    }

    /// Transport NAKs (CRC, SEQ, FULL and other transport codes) received.
    #[must_use]
    pub fn transport_naks(&self) -> u64 {
        self.naks
            .iter()
            .filter(|(r, _)| NakReason::from_u8(**r).is_transport())
            .map(|(_, n)| n)
            .sum()
    }

    /// `NAK reason ×count` list.
    #[must_use]
    pub fn naks_line(&self) -> String {
        if self.naks.is_empty() {
            return "none".to_owned();
        }
        self.naks
            .iter()
            .map(|(r, n)| format!("{:?}x{n}", NakReason::from_u8(*r)))
            .collect::<Vec<_>>()
            .join(" ")
    }
}
