//! The sans-IO link session (Story 3.3, AD-6, AD-7, AD-19): bytes and a
//! clock in, bytes and typed events out. It never touches a port and never
//! reads a clock: the caller passes the time as µs on its own monotonic
//! clock, writes what `poll_transmit` returns and calls `handle_timeout` at
//! `poll_timeout`.
//!
//! The controller's side of every rule is in `docs/link-protocol.md`; the
//! app's side is listed there under "Rules the app link applies (Story 3.3)".

mod events;
pub mod send;

use std::collections::VecDeque;

pub use events::{AbandonReason, CommandError, LinkEvent, Phase, StatusView};
pub use send::{SendBuffer, SendStats};

use crate::protocol::consts::{
    APP_PING_DIVISOR, HAT_NEUTRAL, HELLO_RESEND_MS, MSG_HELLO, MSG_PING, MSG_QUEUE_EVENT,
    MSG_SET_STATE, MSG_START, MSG_STOP, PROTO_VERSION, STATUS_CONSOLE_READY, STATUS_LINK_OK,
    T_US_LIMIT,
};
use crate::protocol::{
    EventCode, Info, Message, NakReason, QueueEvent, RxSide, RxStats, SetState, Splitter, Start,
    State, Status, encode_frame,
};
use crate::version::APP_VERSION_TRIPLE;

const HELLO_RESEND_US: u64 = HELLO_RESEND_MS as u64 * 1000;

/// How a session is set up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionConfig {
    /// The seq of every HELLO of one `open` (INFO carries no seq, so one
    /// value tells which `expected` the controller has afterwards).
    pub hello_seq: u8,
    /// The app version compared with `INFO.fw_ver` and `bridge_ver`.
    pub app_version: [u8; 3],
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            hello_seq: 0,
            app_version: APP_VERSION_TRIPLE,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct Piece {
    queued: usize,
    last_t: Option<u32>,
    end_queued: bool,
    started: bool,
}

#[derive(Debug, Clone, Copy)]
struct StopState {
    seq: u8,
    due: u64,
}

/// One session with one controller.
#[derive(Debug, Clone)]
pub struct Session {
    cfg: SessionConfig,
    phase: Phase,
    splitter: Splitter,
    zero_pending: bool,
    hello_due: u64,
    send: SendBuffer,
    info: Option<Info>,
    status: Option<StatusView>,
    abort_baseline: Option<u8>,
    abort_rise_expected: bool,
    events: VecDeque<LinkEvent>,
    stop: Option<StopState>,
    piece: Piece,
    direct: bool,
    last_app_tx: u64,
}

impl Session {
    /// A closed session.
    #[must_use]
    pub fn new(cfg: SessionConfig) -> Self {
        Self {
            cfg,
            phase: Phase::Closed,
            splitter: Splitter::new(RxSide::App),
            zero_pending: false,
            hello_due: 0,
            send: SendBuffer::new(cfg.hello_seq),
            info: None,
            status: None,
            abort_baseline: None,
            abort_rise_expected: false,
            events: VecDeque::new(),
            stop: None,
            piece: Piece::default(),
            direct: false,
            last_app_tx: 0,
        }
    }

    /// Opens (or reopens) the session: one lone `0x00`, then HELLO every
    /// 100 ms until INFO (AD-7). Pending events are kept.
    pub fn open(&mut self, now_us: u64) {
        self.phase = Phase::Opening;
        self.splitter = Splitter::new(RxSide::App);
        self.zero_pending = true;
        self.hello_due = now_us;
        self.send = SendBuffer::new(self.cfg.hello_seq);
        self.info = None;
        self.status = None;
        self.abort_baseline = None;
        self.abort_rise_expected = false;
        self.stop = None;
        self.piece = Piece::default();
        self.direct = false;
        self.last_app_tx = now_us;
    }

    /// Where the session is.
    #[must_use]
    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// INFO of the open session (the runtime values: `queue_cap`,
    /// `app_timeout_ms`, `report_period_us`, versions).
    #[must_use]
    pub fn info(&self) -> Option<&Info> {
        self.info.as_ref()
    }

    /// The last STATUS.
    #[must_use]
    pub fn status(&self) -> Option<&StatusView> {
        self.status.as_ref()
    }

    /// `STATUS.free − QUEUE_EVENTs sent after STATUS.last_seq`.
    #[must_use]
    pub fn credit(&self) -> Option<i64> {
        self.send.credit()
    }

    /// Frames sent and not acknowledged.
    #[must_use]
    pub fn unacked(&self) -> usize {
        self.send.unacked()
    }

    /// The send counters.
    #[must_use]
    pub fn send_stats(&self) -> SendStats {
        self.send.stats()
    }

    /// The receive counters.
    #[must_use]
    pub fn rx_stats(&self) -> RxStats {
        self.splitter.stats()
    }

    /// The controller is believed to be in DIRECT (PING is due when idle).
    #[must_use]
    pub fn is_direct(&self) -> bool {
        self.direct
    }

    /// A piece has been started and not stopped.
    #[must_use]
    pub fn is_playing(&self) -> bool {
        self.piece.started
    }

    fn piece_active(&self) -> bool {
        self.piece.queued > 0 || self.piece.started || self.direct
    }

    // ------------------------------------------------------------ commands

    fn check_connected(&self) -> Result<(), CommandError> {
        match self.phase {
            Phase::Connected => Ok(()),
            Phase::Stopping => Err(CommandError::Stopping),
            Phase::Closed | Phase::Opening | Phase::Incompatible => Err(CommandError::NotConnected),
        }
    }

    fn check_link(&self) -> Result<(), CommandError> {
        if self.status.is_some_and(|s| s.link_ok) {
            Ok(())
        } else {
            Err(CommandError::NoLink)
        }
    }

    fn state_ok(state: &State) -> bool {
        state.hat <= HAT_NEUTRAL && state.reserved_bits_ok()
    }

    /// Queues one timeline event. Refused locally where the controller
    /// would refuse it (Story 2.4 rules): order, `t_us ≥ 2^31`, hat, reserved
    /// flags, END on a pose that is not neutral, anything after END.
    pub fn queue_event(&mut self, t_us: u32, state: State) -> Result<(), CommandError> {
        self.check_connected()?;
        if !Self::state_ok(&state)
            || (state.has_end() && !state.is_neutral_pose())
            || self.piece.end_queued
        {
            return Err(CommandError::BadState);
        }
        if t_us >= T_US_LIMIT || self.piece.last_t.is_some_and(|last| t_us <= last) {
            return Err(CommandError::Order);
        }
        self.send
            .push(MSG_QUEUE_EVENT, QueueEvent { t_us, state }.encode());
        self.piece.queued += 1;
        self.piece.last_t = Some(t_us);
        self.piece.end_queued |= state.has_end();
        Ok(())
    }

    /// Starts the queued piece after `lead_us`. It goes out after every
    /// QUEUE_EVENT queued before it.
    pub fn start(&mut self, lead_us: u32) -> Result<(), CommandError> {
        self.check_connected()?;
        self.check_link()?;
        if self.piece.started {
            return Err(CommandError::Busy);
        }
        if self.piece.queued == 0 {
            return Err(CommandError::EmptyQueue);
        }
        self.send.push(MSG_START, Start { lead_us }.encode());
        self.piece.started = true;
        self.direct = false;
        Ok(())
    }

    /// Sends one DIRECT state (the virtual gamepad).
    pub fn set_state(&mut self, state: State) -> Result<(), CommandError> {
        self.check_connected()?;
        self.check_link()?;
        if self.piece.started {
            return Err(CommandError::Busy);
        }
        if !Self::state_ok(&state) || state.flags != 0 {
            return Err(CommandError::BadState);
        }
        self.send.push(MSG_SET_STATE, SetState { state }.encode());
        self.direct = true;
        Ok(())
    }

    /// Abandons whatever is pending and sends STOP; nothing else goes out
    /// until its ACK (a late duplicate STOP would clear later frames).
    pub fn stop(&mut self) -> Result<(), CommandError> {
        match self.phase {
            Phase::Connected => {
                self.do_stop();
                Ok(())
            }
            Phase::Stopping => Ok(()),
            Phase::Closed | Phase::Opening | Phase::Incompatible => Err(CommandError::NotConnected),
        }
    }

    fn do_stop(&mut self) {
        let seq = self.send.next_free_seq();
        self.send = SendBuffer::new(seq);
        self.stop = Some(StopState { seq, due: 0 });
        self.phase = Phase::Stopping;
        self.piece = Piece::default();
        self.direct = false;
    }

    fn abandon(&mut self, reason: AbandonReason) {
        if self.phase != Phase::Connected {
            return;
        }
        if self.piece_active() {
            self.events.push_back(LinkEvent::PieceAbandoned { reason });
        }
        self.do_stop();
    }

    // ------------------------------------------------------------ input

    /// Feeds received bytes.
    pub fn handle_rx(&mut self, bytes: &[u8], now_us: u64) {
        for &b in bytes {
            if let crate::protocol::RxEvent::Frame(f) = self.splitter.push(b) {
                self.on_frame(f.ty, &f.body, now_us);
            }
        }
    }

    fn on_frame(&mut self, ty: u8, body: &[u8], now_us: u64) {
        if self.phase == Phase::Closed {
            return;
        }
        let msg = match Message::decode(ty, body) {
            Ok(m) => m,
            Err(_) => {
                self.events.push_back(LinkEvent::Malformed { ty });
                return;
            }
        };
        match msg {
            Message::Info(info) => self.on_info(info),
            Message::Status(s) => self.on_status(&s, now_us),
            Message::Ack(a) => match self.phase {
                Phase::Connected => {
                    self.send.ack(a.seq, now_us);
                }
                Phase::Stopping => {
                    if self.stop.is_some_and(|s| s.seq == a.seq) {
                        self.stop = None;
                        self.phase = Phase::Connected;
                        self.events.push_back(LinkEvent::Stopped);
                    }
                }
                Phase::Closed | Phase::Opening | Phase::Incompatible => {}
            },
            Message::Nak(n) => self.on_nak(n.reason, n.expected, n.got, now_us),
            Message::Event(e) => self.on_event(e.code),
            // Types of the other direction never pass the App range check.
            Message::Hello(_)
            | Message::SetState(_)
            | Message::QueueEvent(_)
            | Message::Start(_)
            | Message::Stop
            | Message::Ping
            | Message::Heartbeat(_) => self.events.push_back(LinkEvent::Malformed { ty }),
        }
    }

    fn on_info(&mut self, info: Info) {
        if self.phase != Phase::Opening {
            return;
        }
        if info.proto_ver != PROTO_VERSION {
            self.phase = Phase::Incompatible;
            self.events.push_back(LinkEvent::RequiredUpdate {
                controller_proto: info.proto_ver,
                app_proto: PROTO_VERSION,
                info,
            });
            return;
        }
        self.phase = Phase::Connected;
        self.info = Some(info);
        self.events.push_back(LinkEvent::Connected(info));
        let app = self.cfg.app_version;
        let fw_differs = info.fw_ver != app;
        let bridge_differs = info.bridge_ver != [0, 0, 0] && info.bridge_ver != app;
        if fw_differs || bridge_differs {
            self.events.push_back(LinkEvent::RecommendedUpdate {
                fw_ver: info.fw_ver,
                bridge_ver: info.bridge_ver,
                app_ver: app,
                fw_differs,
                bridge_differs,
            });
        }
    }

    fn on_status(&mut self, s: &Status, now_us: u64) {
        let view = StatusView {
            free: s.free,
            now_us: s.now_us,
            last_seq: s.last_seq,
            mode: s.mode,
            abort_count: s.abort_count,
            last_event: s.last_event,
            link_ok: s.flags & STATUS_LINK_OK != 0,
            console_ready: s.flags & STATUS_CONSOLE_READY != 0,
        };
        self.status = Some(view);
        self.events.push_back(LinkEvent::Status(view));
        if !matches!(self.phase, Phase::Connected | Phase::Stopping) {
            return;
        }
        self.send.status(s.free, s.last_seq, now_us);
        let Some(base) = self.abort_baseline.replace(s.abort_count) else {
            return;
        };
        if s.abort_count == base {
            return;
        }
        if std::mem::take(&mut self.abort_rise_expected) || self.phase != Phase::Connected {
            return;
        }
        // The latch rose without an EVENT reaching us (lost, or a cause that
        // sends none: app timeout, DTR edge, core stall).
        if !view.link_ok {
            self.zero_pending = true;
            self.abandon(AbandonReason::LinkLost);
        } else if view.last_event == EventCode::PlaybackDone && self.piece.started {
            self.events.push_back(LinkEvent::PlaybackDone);
            self.do_stop();
        } else {
            self.abandon(AbandonReason::ControllerAborted);
        }
    }

    fn on_nak(&mut self, reason: NakReason, expected: u8, got: u8, now_us: u64) {
        if reason == NakReason::Version {
            if matches!(self.phase, Phase::Connected | Phase::Stopping) {
                self.on_reboot(now_us);
            }
            return;
        }
        if self.phase != Phase::Connected {
            return;
        }
        if let NakReason::Unknown(r) = reason {
            // Transport or semantic is unknown: take only the cumulative part.
            self.send.ack(expected.wrapping_sub(1), now_us);
            self.events.push_back(LinkEvent::UnknownNak {
                reason: r,
                expected,
                got,
            });
            return;
        }
        if reason.is_transport() {
            self.send.transport_nak(expected, now_us);
            return;
        }
        let refused_ty = self.send.sent_frame(got).map(|f| f.ty);
        self.send.ack(expected.wrapping_sub(1), now_us);
        match refused_ty {
            Some(MSG_QUEUE_EVENT | MSG_START) => self.abandon(AbandonReason::Nak(reason)),
            Some(ty) => self.events.push_back(LinkEvent::Refused { reason, ty }),
            None => {}
        }
    }

    fn on_event(&mut self, code: EventCode) {
        self.events.push_back(LinkEvent::ControllerEvent(code));
        if self.phase != Phase::Connected {
            return;
        }
        match code {
            EventCode::PlaybackDone => {
                self.abort_rise_expected = true;
                if self.piece.started {
                    self.events.push_back(LinkEvent::PlaybackDone);
                }
                self.do_stop();
            }
            EventCode::LinkLost => {
                self.abort_rise_expected = true;
                self.zero_pending = true;
                self.abandon(AbandonReason::LinkLost);
            }
            EventCode::Late | EventCode::Underrun | EventCode::UsbSuspended => {
                self.abort_rise_expected = true;
                self.abandon(AbandonReason::ControllerAborted);
            }
            EventCode::None | EventCode::Unknown(_) => {}
        }
    }

    fn on_reboot(&mut self, now_us: u64) {
        self.events.push_back(LinkEvent::ControllerRebooted);
        if self.piece_active() {
            self.events.push_back(LinkEvent::PieceAbandoned {
                reason: AbandonReason::ControllerRebooted,
            });
        }
        self.open(now_us);
    }

    // ------------------------------------------------------------ time

    /// Runs the timers: the 50 ms resend and the DIRECT PING.
    pub fn handle_timeout(&mut self, now_us: u64) {
        if self.phase != Phase::Connected {
            return;
        }
        self.send.check_timeout(now_us);
        if let Some(due) = self.ping_due()
            && now_us >= due
            && !self.send.has_unsent()
        {
            self.send.push(MSG_PING, Vec::new());
        }
    }

    fn ping_interval_us(&self) -> Option<u64> {
        let ms = u64::from(self.info?.app_timeout_ms) / u64::from(APP_PING_DIVISOR);
        (ms > 0).then_some(ms * 1000)
    }

    fn ping_due(&self) -> Option<u64> {
        if !self.direct {
            return None;
        }
        Some(self.last_app_tx + self.ping_interval_us()?)
    }

    /// When `handle_timeout` (or `poll_transmit`) must next be called. A
    /// time at or before now means "now".
    #[must_use]
    pub fn poll_timeout(&self) -> Option<u64> {
        if self.zero_pending && self.phase != Phase::Closed {
            return Some(0);
        }
        match self.phase {
            Phase::Opening => Some(self.hello_due),
            Phase::Stopping => self.stop.map(|s| s.due),
            Phase::Connected => [self.send.timeout_at(), self.ping_due()]
                .into_iter()
                .flatten()
                .min(),
            Phase::Closed | Phase::Incompatible => None,
        }
    }

    // ------------------------------------------------------------ output

    /// The next bytes to write (one frame with its `0x00`, or a lone
    /// `0x00`), or `None`. Call until `None` after every input.
    pub fn poll_transmit(&mut self, now_us: u64) -> Option<Vec<u8>> {
        self.handle_timeout(now_us);
        if self.phase == Phase::Closed {
            return None;
        }
        if self.zero_pending {
            self.zero_pending = false;
            return Some(vec![0]);
        }
        match self.phase {
            Phase::Opening => {
                if now_us < self.hello_due {
                    return None;
                }
                self.hello_due = now_us + HELLO_RESEND_US;
                encode_frame(MSG_HELLO, self.cfg.hello_seq, &[PROTO_VERSION]).ok()
            }
            Phase::Stopping => {
                let stop = self.stop.as_mut()?;
                if now_us < stop.due {
                    return None;
                }
                stop.due = now_us + send::ACK_TIMEOUT_US;
                encode_frame(MSG_STOP, stop.seq, &[]).ok()
            }
            Phase::Connected => {
                let (seq, ty, body) = self.send.next_to_send(now_us)?;
                self.last_app_tx = now_us;
                encode_frame(ty, seq, &body).ok()
            }
            Phase::Closed | Phase::Incompatible => None,
        }
    }

    /// The next event, oldest first.
    pub fn poll_event(&mut self) -> Option<LinkEvent> {
        self.events.pop_front()
    }
}
