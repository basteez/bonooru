//! A fake controller: the controller link and core rules the app relies on,
//! written from `docs/link-protocol.md` (AD-6, AD-7, AD-8, AD-19 and the
//! "Rules the controller … applies" sections of Stories 2.4, 2.6 and 2.7).
//! It is a test model, not the firmware: no USB, no UART timing, a fake clock.

use std::collections::VecDeque;

use ocb_core::protocol::consts::{
    APP_TIMEOUT_MS_DEFAULT, HAT_NEUTRAL, MSG_ACK, MSG_EVENT, MSG_HEARTBEAT, MSG_HELLO, MSG_INFO,
    MSG_NAK, MSG_PING, MSG_QUEUE_EVENT, MSG_SET_STATE, MSG_START, MSG_STATUS, MSG_STOP,
    PROTO_VERSION, QUEUE_CAP_DEFAULT, STATUS_CONSOLE_READY, STATUS_LINK_OK, STATUS_MIN_GAP_MS,
    STATUS_PERIOD_MS, T_US_LIMIT, TYPE_APP_MAX, TYPE_APP_MIN,
};
use ocb_core::protocol::{
    Ack, Event, EventCode, HidProfile, Info, Message, Mode, Nak, NakReason, QueueEvent, RxEvent,
    RxSide, Splitter, State, Status, encode_frame,
};

/// How the fake controller is set up.
#[derive(Debug, Clone, Copy)]
pub struct ControllerConfig {
    /// INFO `proto_ver` (a mismatch closes the session).
    pub proto_ver: u8,
    /// INFO `fw_ver`.
    pub fw_ver: [u8; 3],
    /// INFO `bridge_ver`.
    pub bridge_ver: [u8; 3],
    /// Queue capacity (INFO `queue_cap`).
    pub queue_cap: u16,
    /// INFO `app_timeout_ms` (also the DIRECT app timeout).
    pub app_timeout_ms: u16,
    /// INFO `report_period_us`.
    pub report_period_us: u16,
    /// Test mode: outside PLAYING, pop one queued event every this many µs
    /// (a sink that frees credits without a START).
    pub sink_every_us: Option<u64>,
}

impl Default for ControllerConfig {
    fn default() -> Self {
        Self {
            proto_ver: PROTO_VERSION,
            fw_ver: [0, 0, 0],
            bridge_ver: [0, 0, 0],
            queue_cap: QUEUE_CAP_DEFAULT,
            app_timeout_ms: APP_TIMEOUT_MS_DEFAULT,
            report_period_us: 8000,
            sink_every_us: None,
        }
    }
}

/// What the fake controller did, for test assertions.
#[derive(Debug, Clone, Default)]
pub struct ControllerLog {
    /// Every QUEUE_EVENT accepted into the queue, in order.
    pub enqueued: Vec<(u32, State)>,
    /// Every event applied by the scheduler or the sink, in order.
    pub applied: Vec<(u32, State)>,
    /// Every DIRECT state applied.
    pub direct: Vec<State>,
    /// Every NAK sent: (reason, expected, got).
    pub naks: Vec<(u8, u8, u8)>,
    /// STOPs executed.
    pub stops: u32,
    /// HELLOs answered.
    pub hellos: u32,
    /// PINGs ACKed.
    pub pings: u32,
    /// DIRECT app timeouts.
    pub app_timeouts: u32,
    /// The highest queue fill seen.
    pub max_fill: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Reply {
    Ack(u8),
    Nak(NakReason, u8, u8),
}

/// The fake controller.
#[derive(Debug, Clone)]
pub struct FakeController {
    cfg: ControllerConfig,
    rx: Splitter,
    out: VecDeque<Vec<u8>>,
    tx_seq: u8,
    // link
    session_open: bool,
    expected: u8,
    cached: Option<Reply>,
    transport_nak_armed: bool,
    // core
    mode: Mode,
    queue: VecDeque<(u32, State)>,
    end_queued: bool,
    last_t: Option<u32>,
    aborted: bool,
    abort_count: u8,
    last_event: EventCode,
    t0: u64,
    // inputs from the test
    link_ok: bool,
    console_ready: bool,
    inject: Option<(u8, NakReason)>,
    // timing
    last_status: Option<u64>,
    status_after_ack: bool,
    last_app_frame: u64,
    last_sink: u64,
    log: ControllerLog,
}

impl FakeController {
    /// A powered controller with the session closed and the link up.
    #[must_use]
    pub fn new(cfg: ControllerConfig) -> Self {
        Self {
            cfg,
            rx: Splitter::new(RxSide::Controller),
            out: VecDeque::new(),
            tx_seq: 0,
            session_open: false,
            expected: 0,
            cached: None,
            transport_nak_armed: true,
            mode: Mode::Idle,
            queue: VecDeque::new(),
            end_queued: false,
            last_t: None,
            aborted: false,
            abort_count: 0,
            last_event: EventCode::None,
            t0: 0,
            link_ok: true,
            console_ready: true,
            inject: None,
            last_status: None,
            status_after_ack: false,
            last_app_frame: 0,
            last_sink: 0,
            log: ControllerLog::default(),
        }
    }

    /// The log.
    #[must_use]
    pub fn log(&self) -> &ControllerLog {
        &self.log
    }

    /// The current mode.
    #[must_use]
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Queued events.
    #[must_use]
    pub fn queue_len(&self) -> usize {
        self.queue.len()
    }

    /// The configured capacity.
    #[must_use]
    pub fn queue_cap(&self) -> usize {
        usize::from(self.cfg.queue_cap)
    }

    /// The abort latch.
    #[must_use]
    pub fn aborted(&self) -> bool {
        self.aborted
    }

    /// Sets `console_ready` (STATUS bit 1 and the `NOT_READY` gate).
    pub fn set_console_ready(&mut self, ready: bool) {
        self.console_ready = ready;
    }

    /// Link loss (`false`: abort with `EVENT LINK_LOST`) or recovery.
    pub fn set_link_ok(&mut self, ok: bool) {
        if self.link_ok && !ok {
            self.abort(Some(EventCode::LinkLost));
        }
        self.link_ok = ok;
    }

    /// The next frame of type `ty` the core would handle gets this NAK.
    pub fn inject_nak(&mut self, ty: u8, reason: NakReason) {
        self.inject = Some((ty, reason));
    }

    /// Sends an EVENT with any code, changing nothing else.
    pub fn inject_event(&mut self, code: u8) {
        self.send(MSG_EVENT, &[code]);
    }

    /// Sends a NAK with any reason, changing nothing else.
    pub fn inject_raw_nak(&mut self, reason: u8, expected: u8, got: u8) {
        self.send(MSG_NAK, &[reason, expected, got]);
    }

    /// An abort the app did not request; `None` sends no EVENT (app
    /// timeout, DTR edge, core stall).
    pub fn force_abort(&mut self, event: Option<EventCode>) {
        self.abort(event);
    }

    /// A reboot: session closed, `expected` 0, everything cleared.
    pub fn reboot(&mut self) {
        let cfg = self.cfg;
        let log = std::mem::take(&mut self.log);
        *self = Self::new(cfg);
        self.log = log;
    }

    fn send(&mut self, ty: u8, body: &[u8]) {
        if let Ok(f) = encode_frame(ty, self.tx_seq, body) {
            self.out.push_back(f);
        }
        self.tx_seq = self.tx_seq.wrapping_add(1);
    }

    fn send_event(&mut self, code: EventCode) {
        self.last_event = code;
        self.send(MSG_EVENT, &Event { code }.encode());
    }

    fn enter_idle(&mut self) {
        self.mode = Mode::Idle;
        self.queue.clear();
        self.end_queued = false;
        self.last_t = None;
    }

    fn abort(&mut self, event: Option<EventCode>) {
        if self.aborted {
            return;
        }
        self.enter_idle();
        self.aborted = true;
        self.abort_count = self.abort_count.wrapping_add(1);
        if let Some(e) = event {
            self.send_event(e);
        }
    }

    fn sync(&mut self, seq: u8) {
        self.enter_idle();
        self.aborted = false;
        self.expected = seq.wrapping_add(1);
        self.cached = None;
        self.transport_nak_armed = true;
    }

    fn reply(&mut self, r: &Reply) {
        match *r {
            Reply::Ack(s) => {
                self.send(MSG_ACK, &Ack { seq: s }.encode());
                self.status_after_ack = true;
            }
            Reply::Nak(reason, expected, got) => {
                self.log.naks.push((reason.to_u8(), expected, got));
                self.send(
                    MSG_NAK,
                    &Nak {
                        reason,
                        expected,
                        got,
                    }
                    .encode(),
                );
            }
        }
    }

    fn transport_nak(&mut self, reason: NakReason, got: u8) {
        if self.transport_nak_armed {
            self.transport_nak_armed = false;
            let r = Reply::Nak(reason, self.expected, got);
            self.reply(&r);
        }
    }

    fn info(&mut self) {
        let info = Info {
            proto_ver: self.cfg.proto_ver,
            fw_ver: self.cfg.fw_ver,
            bridge_ver: self.cfg.bridge_ver,
            hid_profile: HidProfile::P2,
            queue_cap: self.cfg.queue_cap,
            app_timeout_ms: self.cfg.app_timeout_ms,
            report_period_us: self.cfg.report_period_us,
        };
        self.send(MSG_INFO, &info.encode());
    }

    /// Feeds bytes from the bridge.
    pub fn handle_rx(&mut self, bytes: &[u8], now_us: u64) {
        for &b in bytes {
            match self.rx.push(b) {
                RxEvent::Frame(f) => self.on_frame(f.ty, f.seq, &f.body, now_us),
                RxEvent::Dropped(e) => {
                    use ocb_core::protocol::FrameError as E;
                    if matches!(e, E::Cobs | E::Crc | E::Overlong) {
                        self.transport_nak(NakReason::Crc, 0);
                    }
                }
                RxEvent::None => {}
            }
        }
    }

    fn on_frame(&mut self, ty: u8, seq: u8, body: &[u8], now_us: u64) {
        if (TYPE_APP_MIN..=TYPE_APP_MAX).contains(&ty) {
            self.last_app_frame = now_us;
        }
        if ty == MSG_HEARTBEAT {
            return;
        }
        if ty == MSG_STOP {
            self.sync(seq);
            self.log.stops += 1;
            self.reply(&Reply::Ack(seq));
            return;
        }
        if ty == MSG_HELLO {
            self.log.hellos += 1;
            let ok = body.first() == Some(&PROTO_VERSION) && self.cfg.proto_ver == PROTO_VERSION;
            self.sync(seq);
            self.session_open = ok;
            self.info();
            return;
        }
        if !self.session_open {
            self.expected = seq.wrapping_add(1);
            self.reply(&Reply::Nak(NakReason::Version, self.expected, seq));
            return;
        }
        if seq == self.expected {
            let r = self.core(ty, seq, body, now_us);
            match &r {
                Reply::Nak(reason, _, _) if reason.is_transport() => {
                    if self.transport_nak_armed {
                        self.transport_nak_armed = false;
                        self.reply(&r);
                    }
                }
                _ => {
                    self.expected = seq.wrapping_add(1);
                    self.transport_nak_armed = true;
                    self.cached = Some(r.clone());
                    self.reply(&r);
                }
            }
            return;
        }
        let d = self.expected.wrapping_sub(seq);
        if d == 1 {
            let r = self
                .cached
                .clone()
                .unwrap_or(Reply::Ack(self.expected.wrapping_sub(1)));
            self.reply(&r);
        } else if (2..=64).contains(&d) {
            self.reply(&Reply::Ack(self.expected.wrapping_sub(1)));
        } else {
            self.transport_nak(NakReason::Seq, seq);
        }
    }

    fn state_ok(s: &State) -> bool {
        s.hat <= HAT_NEUTRAL && s.reserved_bits_ok()
    }

    /// The core's decision for a frame with `seq == expected` (Story 2.4
    /// precedence: ABORTED, NO_LINK, BUSY, NOT_READY, BAD_STATE, ORDER, FULL).
    fn core(&mut self, ty: u8, seq: u8, body: &[u8], now_us: u64) -> Reply {
        let after = seq.wrapping_add(1);
        let nak = |r: NakReason| Reply::Nak(r, after, seq);
        if let Some((ity, reason)) = self.inject
            && ity == ty
        {
            self.inject = None;
            return if reason.is_transport() {
                Reply::Nak(reason, self.expected, seq)
            } else {
                nak(reason)
            };
        }
        match ty {
            MSG_PING => {
                if !body.is_empty() {
                    return nak(NakReason::BadState);
                }
                self.log.pings += 1;
                Reply::Ack(seq)
            }
            MSG_SET_STATE | MSG_START => {
                if self.aborted {
                    return nak(NakReason::Aborted);
                }
                if !self.link_ok {
                    return nak(NakReason::NoLink);
                }
                if self.mode == Mode::Playing {
                    return nak(NakReason::Busy);
                }
                if !self.console_ready {
                    return nak(NakReason::NotReady);
                }
                if ty == MSG_SET_STATE {
                    let Ok(Message::SetState(m)) = Message::decode(ty, body) else {
                        return nak(NakReason::BadState);
                    };
                    if !Self::state_ok(&m.state) || m.state.flags != 0 {
                        return nak(NakReason::BadState);
                    }
                    self.mode = Mode::Direct;
                    self.log.direct.push(m.state);
                    Reply::Ack(seq)
                } else {
                    let Ok(Message::Start(m)) = Message::decode(ty, body) else {
                        return nak(NakReason::BadState);
                    };
                    if self.queue.is_empty() {
                        return nak(NakReason::BadState);
                    }
                    self.mode = Mode::Playing;
                    self.t0 = now_us + u64::from(m.lead_us);
                    Reply::Ack(seq)
                }
            }
            MSG_QUEUE_EVENT => {
                if self.aborted {
                    return nak(NakReason::Aborted);
                }
                let Ok(Message::QueueEvent(QueueEvent { t_us, state })) = Message::decode(ty, body)
                else {
                    return nak(NakReason::BadState);
                };
                if !Self::state_ok(&state)
                    || (state.has_end() && !state.is_neutral_pose())
                    || self.end_queued
                {
                    return nak(NakReason::BadState);
                }
                if t_us >= T_US_LIMIT || self.last_t.is_some_and(|l| t_us <= l) {
                    return nak(NakReason::Order);
                }
                if self.queue.len() >= usize::from(self.cfg.queue_cap) {
                    return Reply::Nak(NakReason::Full, self.expected, seq);
                }
                self.queue.push_back((t_us, state));
                self.log.max_fill = self.log.max_fill.max(self.queue.len());
                self.log.enqueued.push((t_us, state));
                self.last_t = Some(t_us);
                self.end_queued |= state.has_end();
                Reply::Ack(seq)
            }
            _ => nak(NakReason::UnknownType),
        }
    }

    fn status(&mut self, now_us: u64) {
        let now = if self.mode == Mode::Playing && now_us >= self.t0 {
            u32::try_from(now_us - self.t0).unwrap_or(u32::MAX)
        } else {
            0
        };
        let flags = if self.link_ok { STATUS_LINK_OK } else { 0 }
            | if self.console_ready {
                STATUS_CONSOLE_READY
            } else {
                0
            };
        let free = usize::from(self.cfg.queue_cap).saturating_sub(self.queue.len());
        let st = Status {
            free: u16::try_from(free).unwrap_or(u16::MAX),
            now_us: now,
            last_seq: self.expected.wrapping_sub(1),
            mode: self.mode,
            abort_count: self.abort_count,
            last_event: self.last_event,
            flags,
            extra: Vec::new(),
        };
        if let Ok(body) = st.encode() {
            self.send(MSG_STATUS, &body);
        }
        self.last_status = Some(now_us);
        self.status_after_ack = false;
    }

    /// Advances the controller to `now_us`: the scheduler (or the sink), the
    /// DIRECT app timeout and the STATUS cadence.
    pub fn tick(&mut self, now_us: u64) {
        if self.mode == Mode::Playing {
            if let Some(&(t, s)) = self.queue.front()
                && now_us >= self.t0 + u64::from(t)
            {
                let late = now_us - (self.t0 + u64::from(t));
                if late > u64::from(self.cfg.report_period_us.max(1)) {
                    self.abort(Some(EventCode::Late));
                } else {
                    self.queue.pop_front();
                    self.log.applied.push((t, s));
                    if s.has_end() {
                        self.abort(Some(EventCode::PlaybackDone));
                    } else if self.queue.is_empty() {
                        self.abort(Some(EventCode::Underrun));
                    }
                }
            }
        } else if let Some(every) = self.cfg.sink_every_us
            && now_us >= self.last_sink + every
        {
            self.last_sink = now_us;
            if let Some(e) = self.queue.pop_front() {
                self.log.applied.push(e);
            }
        }
        if self.mode == Mode::Direct
            && now_us > self.last_app_frame + u64::from(self.cfg.app_timeout_ms) * 1000
        {
            self.log.app_timeouts += 1;
            self.abort(None);
        }
        let due = match self.last_status {
            None => true,
            Some(last) => {
                now_us >= last + u64::from(STATUS_PERIOD_MS) * 1000
                    || (self.status_after_ack
                        && now_us >= last + u64::from(STATUS_MIN_GAP_MS) * 1000)
            }
        };
        if due {
            self.status(now_us);
        }
    }

    /// The next frame to send (whole, with its `0x00`).
    pub fn poll_transmit(&mut self) -> Option<Vec<u8>> {
        self.out.pop_front()
    }
}
