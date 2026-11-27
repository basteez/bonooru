//! The virtual gamepad as data (Story 4.8; FR-11, FR-22, UX-DR36, AD-7,
//! AD-16): the 16 button controls and two sticks, the mouse semantics (press
//! = press, release = release, a click is a tap of at least
//! [`TAP_MIN_US`], sticks follow the drag and spring back), the release
//! rules, the one canonical [`State`] the app sends on change, the copy and
//! the `pad` log lines. Pure: no egui (G.22 checks the source); `ui::gamepad`
//! draws and senses, the app glues the two and talks to the worker.

use ocb_core::protocol::consts::{
    BTN_A, BTN_B, BTN_CAPTURE, BTN_HOME, BTN_L, BTN_MINUS, BTN_PLUS, BTN_R, BTN_X, BTN_Y, BTN_ZL,
    BTN_ZR, HAT_DOWN, HAT_DOWN_LEFT, HAT_DOWN_RIGHT, HAT_LEFT, HAT_NEUTRAL, HAT_RIGHT, HAT_UP,
    HAT_UP_LEFT, HAT_UP_RIGHT, STICK_CENTER,
};
use ocb_core::protocol::{Mode, NEUTRAL, State};

/// The shortest press the pad sends, µs: a click shorter than this is held
/// until it has lasted this long (AC 4, D2). Story 1.4 (U7) found a 2-frame
/// press (33.3 ms at 60 Hz) is the game's minimum; one more 60 Hz frame of
/// margin (Story 1.5's convention) gives 50 ms, so even a click shorter than
/// a UI frame reaches the console's 8 ms polls and the game.
pub const TAP_MIN_US: u64 = 50_000;

/// How soon a failed `try_send` is retried with no input, µs (one 60 Hz
/// frame).
pub const RESEND_AFTER_US: u64 = 16_000;

/// The panel tab (mock `.ph .tab`).
pub const TAB: &str = "Gamepad";
/// The caption under the tab (`epics.md` Story 4.8 AC, UX-DR64). It may say
/// "free play": an instruction to the user, not a claim about the game
/// state (D13; the only exemption from `leds::BANNED_WORDS`).
pub const CAPTION: &str = "Use it to reach ocarina free play.";
/// The phrase the caption alone may contain.
pub const CAPTION_EXEMPT_PHRASE: &str = "free play";
/// "mouse" next to each stick.
pub const MOUSE: &str = "mouse";
/// The legend rows (mock `.legend`): control names, then their keys (the
/// keys work from Story 4.9).
pub const LEGEND: [(&str, &str); 6] = [
    ("D-pad", "W A S D or arrows"),
    ("Y B X A", "H J K L"),
    ("L \u{b7} R", "Q \u{b7} E"),
    ("ZL \u{b7} ZR", "Z \u{b7} C"),
    ("+ \u{b7} \u{2212}", "Enter \u{b7} Backspace"),
    ("Sticks, Home, Capture", "mouse"),
];
/// The chip under the D-pad.
pub const CHIP_DPAD: &str = "WASD \u{b7} arrows";
/// The text chips beside the keyboard-mapped controls (⌫ and ↵ are painted
/// symbols, not text).
pub const CHIP_TEXTS: [&str; 9] = ["Z", "Q", "E", "C", "K", "H", "L", "J", CHIP_DPAD];

/// One of the 16 button controls of the pad.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Control {
    /// A.
    A,
    /// B.
    B,
    /// X.
    X,
    /// Y.
    Y,
    /// D-pad up.
    DUp,
    /// D-pad down.
    DDown,
    /// D-pad left.
    DLeft,
    /// D-pad right.
    DRight,
    /// L.
    L,
    /// R.
    R,
    /// ZL.
    Zl,
    /// ZR.
    Zr,
    /// Minus.
    Minus,
    /// Plus.
    Plus,
    /// Home.
    Home,
    /// Capture.
    Capture,
}

/// A D-pad direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    /// Up.
    Up,
    /// Down.
    Down,
    /// Left.
    Left,
    /// Right.
    Right,
}

/// What a control does to the state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    /// A `BTN_*` bit.
    Button(u16),
    /// A D-pad direction (the hat).
    Dpad(Dir),
}

impl Control {
    /// Every control, in drawing and AccessKit order.
    pub const ALL: [Self; 16] = [
        Self::Zl,
        Self::L,
        Self::R,
        Self::Zr,
        Self::Minus,
        Self::Plus,
        Self::Capture,
        Self::Home,
        Self::DUp,
        Self::DDown,
        Self::DLeft,
        Self::DRight,
        Self::X,
        Self::Y,
        Self::A,
        Self::B,
    ];

    /// The AccessKit name (AC 14).
    #[must_use]
    pub const fn a11y_name(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::X => "X",
            Self::Y => "Y",
            Self::DUp => "D-pad up",
            Self::DDown => "D-pad down",
            Self::DLeft => "D-pad left",
            Self::DRight => "D-pad right",
            Self::L => "L",
            Self::R => "R",
            Self::Zl => "ZL",
            Self::Zr => "ZR",
            Self::Minus => "Minus",
            Self::Plus => "Plus",
            Self::Home => "Home",
            Self::Capture => "Capture",
        }
    }

    /// The name in the `pad` log lines.
    #[must_use]
    pub const fn log_name(self) -> &'static str {
        match self {
            Self::A => "a",
            Self::B => "b",
            Self::X => "x",
            Self::Y => "y",
            Self::DUp => "dup",
            Self::DDown => "ddown",
            Self::DLeft => "dleft",
            Self::DRight => "dright",
            Self::L => "l",
            Self::R => "r",
            Self::Zl => "zl",
            Self::Zr => "zr",
            Self::Minus => "minus",
            Self::Plus => "plus",
            Self::Home => "home",
            Self::Capture => "capture",
        }
    }

    /// Its effect on the state (`ocb_core::protocol::consts`).
    #[must_use]
    pub const fn effect(self) -> Effect {
        match self {
            Self::A => Effect::Button(BTN_A),
            Self::B => Effect::Button(BTN_B),
            Self::X => Effect::Button(BTN_X),
            Self::Y => Effect::Button(BTN_Y),
            Self::L => Effect::Button(BTN_L),
            Self::R => Effect::Button(BTN_R),
            Self::Zl => Effect::Button(BTN_ZL),
            Self::Zr => Effect::Button(BTN_ZR),
            Self::Minus => Effect::Button(BTN_MINUS),
            Self::Plus => Effect::Button(BTN_PLUS),
            Self::Home => Effect::Button(BTN_HOME),
            Self::Capture => Effect::Button(BTN_CAPTURE),
            Self::DUp => Effect::Dpad(Dir::Up),
            Self::DDown => Effect::Dpad(Dir::Down),
            Self::DLeft => Effect::Dpad(Dir::Left),
            Self::DRight => Effect::Dpad(Dir::Right),
        }
    }

    /// The state with only this control held.
    #[must_use]
    pub fn alone(self) -> State {
        let mut s = NEUTRAL;
        apply(&mut s, self);
        s
    }
}

/// One of the two sticks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stick {
    /// The left stick (`lx`, `ly`).
    Left,
    /// The right stick (`rx`, `ry`).
    Right,
}

impl Stick {
    /// Both, in drawing order.
    pub const ALL: [Self; 2] = [Self::Left, Self::Right];

    /// The AccessKit name (AC 14).
    #[must_use]
    pub const fn a11y_name(self) -> &'static str {
        match self {
            Self::Left => "Left stick",
            Self::Right => "Right stick",
        }
    }

    /// The name in the `pad` log lines.
    #[must_use]
    pub const fn log_name(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::Left => 0,
            Self::Right => 1,
        }
    }
}

/// Something on the pad the pointer can press.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Target {
    /// A button control.
    Control(Control),
    /// A stick's well.
    Stick(Stick),
}

impl Target {
    /// The name in the `pad ignored` line (`stick_left`, `stick_right` for
    /// the sticks).
    #[must_use]
    pub const fn ignored_name(self) -> &'static str {
        match self {
            Self::Control(c) => c.log_name(),
            Self::Stick(Stick::Left) => "stick_left",
            Self::Stick(Stick::Right) => "stick_right",
        }
    }
}

/// Who pressed a control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The pointer (primary button).
    Mouse,
    /// An AccessKit `Click` (a tap).
    AccessKit,
}

impl Source {
    /// The log value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mouse => "mouse",
            Self::AccessKit => "accesskit",
        }
    }
}

/// Why a held control or stick was released.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Release {
    /// The primary button came up after the tap minimum.
    PointerUp,
    /// The tap minimum ran out (the pointer came up earlier, or an
    /// AccessKit tap).
    Tap,
    /// The window lost focus (rule 8a; SET_STATE NEUTRAL).
    WindowBlur,
    /// STOP was requested (rule 8b; STOP releases, nothing sent).
    Stop,
    /// The gate turned blocked (rule 8c; the app requests STOP).
    Gate,
    /// The controller left DIRECT by itself (rule 8d; nothing sent).
    ControllerIdle,
}

impl Release {
    /// The log value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PointerUp => "pointer_up",
            Self::Tap => "tap",
            Self::WindowBlur => "window_blur",
            Self::Stop => "stop",
            Self::Gate => "gate",
            Self::ControllerIdle => "controller_idle",
        }
    }

    /// The controller is already neutral (or STOP makes it so): the pad
    /// must not send NEUTRAL for this release.
    const fn neutral_without_send(self) -> bool {
        matches!(self, Self::Stop | Self::Gate | Self::ControllerIdle)
    }
}

/// `hat` for the D-pad directions held: one → that direction, two adjacent
/// → the diagonal, opposite pairs cancel on their axis, none → neutral.
#[must_use]
pub fn hat(up: bool, down: bool, left: bool, right: bool) -> u8 {
    let v = i8::from(down) - i8::from(up);
    let h = i8::from(right) - i8::from(left);
    match (v, h) {
        (-1, 0) => HAT_UP,
        (-1, 1) => HAT_UP_RIGHT,
        (0, 1) => HAT_RIGHT,
        (1, 1) => HAT_DOWN_RIGHT,
        (1, 0) => HAT_DOWN,
        (1, -1) => HAT_DOWN_LEFT,
        (0, -1) => HAT_LEFT,
        (-1, -1) => HAT_UP_LEFT,
        _ => HAT_NEUTRAL,
    }
}

/// One stick axis from a deflection `t` in [−1, 1] (clamped; NaN → centre):
/// `128 + round(127·t)` for `t ≥ 0`, `128 + round(128·t)` for `t < 0` (AC 5).
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is clamped to 0..=255 first"
)]
pub fn axis(t: f32) -> u8 {
    if t.is_nan() {
        return STICK_CENTER;
    }
    let t = t.clamp(-1.0, 1.0);
    let v = if t >= 0.0 {
        128.0 + (127.0 * t).round()
    } else {
        128.0 + (128.0 * t).round()
    };
    v.clamp(0.0, 255.0) as u8
}

/// The stick value `(x, y)` for a pointer offset from the well centre:
/// full deflection at `full` points, clamped to a circle, no dead zone
/// (x right = larger, y down = larger).
#[must_use]
pub fn stick_value(offset: (f32, f32), full: f32) -> (u8, u8) {
    let (dx, dy) = offset;
    if !(dx.is_finite() && dy.is_finite()) || full <= 0.0 {
        return (STICK_CENTER, STICK_CENTER);
    }
    let (mut tx, mut ty) = (dx / full, dy / full);
    let len = tx.hypot(ty);
    if len > 1.0 {
        tx /= len;
        ty /= len;
    }
    (axis(tx), axis(ty))
}

fn apply(s: &mut State, c: Control) {
    match c.effect() {
        Effect::Button(bit) => s.buttons |= bit,
        Effect::Dpad(d) => {
            s.hat = hat(
                d == Dir::Up,
                d == Dir::Down,
                d == Dir::Left,
                d == Dir::Right,
            );
        }
    }
}

/// `pad press` (without the kind tag).
#[must_use]
pub fn press_line(c: Control, source: Source) -> String {
    format!("press control={} source={}", c.log_name(), source.as_str())
}

/// `pad release`.
#[must_use]
pub fn release_line(c: Control, reason: Release) -> String {
    format!(
        "release control={} reason={}",
        c.log_name(),
        reason.as_str()
    )
}

/// `pad stick=<s> start`.
#[must_use]
pub fn stick_start_line(s: Stick) -> String {
    format!("stick={} start", s.log_name())
}

/// `pad stick=<s> end reason=<r>`.
#[must_use]
pub fn stick_end_line(s: Stick, reason: Release) -> String {
    format!("stick={} end reason={}", s.log_name(), reason.as_str())
}

/// `pad ignored`: a press while the gate blocks the gamepad; `reason` is
/// `gating::Block::as_str`.
#[must_use]
pub fn ignored_line(t: Target, reason: &str) -> String {
    format!("ignored control={} reason={reason}", t.ignored_name())
}

/// `pad command`: a `try_send` that failed (`queue_full`, `no_worker`,
/// `worker_gone`).
#[must_use]
pub fn command_line(result: &str) -> String {
    format!("command={result}")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Held {
    control: Control,
    since_us: u64,
}

/// The pad the app owns: what the mouse (or AccessKit) holds, the stick
/// values, what was last sent and the `pad` lines to write. Every method
/// takes the UI time in µs; the app writes [`Pad::take_lines`] before it
/// sends [`Pad::take_send`], so a `press` line always precedes the worker's
/// `pad sent` for it (AC 17).
#[derive(Clone, Debug)]
pub struct Pad {
    held: Option<Held>,
    /// The held control is released at this time (a tap).
    release_at: Option<u64>,
    /// The stick being dragged.
    dragging: Option<Stick>,
    sticks: [(u8, u8); 2],
    /// The primary button is down on the pad (pressed through it).
    pointer_down: bool,
    /// A pointer press that must be released before anything is pressed
    /// again (a release rule fired while it was down).
    spent: bool,
    last_sent: State,
    /// A `try_send` failed: the current state is sent again.
    pending: bool,
    in_flight: Option<State>,
    /// A `command` line was written for the current failure streak.
    fail_logged: bool,
    last_mode: Option<Mode>,
    lines: Vec<String>,
}

impl Default for Pad {
    fn default() -> Self {
        Self::new()
    }
}

impl Pad {
    /// Nothing held, sticks centred, NEUTRAL last sent.
    #[must_use]
    pub fn new() -> Self {
        Self {
            held: None,
            release_at: None,
            dragging: None,
            sticks: [(STICK_CENTER, STICK_CENTER); 2],
            pointer_down: false,
            spent: false,
            last_sent: NEUTRAL,
            pending: false,
            in_flight: None,
            fail_logged: false,
            last_mode: None,
            lines: Vec::new(),
        }
    }

    /// The control held (mouse or AccessKit tap), if any.
    #[must_use]
    pub fn held(&self) -> Option<Control> {
        self.held.map(|h| h.control)
    }

    /// The stick being dragged, if any.
    #[must_use]
    pub fn dragging(&self) -> Option<Stick> {
        self.dragging
    }

    /// The value of a stick.
    #[must_use]
    pub fn stick_value(&self, s: Stick) -> (u8, u8) {
        self.sticks
            .get(s.index())
            .copied()
            .unwrap_or((STICK_CENTER, STICK_CENTER))
    }

    /// The pointer press is spent (no press until it is released).
    #[must_use]
    pub fn is_spent(&self) -> bool {
        self.spent
    }

    /// The state last handed to the worker.
    #[must_use]
    pub fn last_sent(&self) -> State {
        self.last_sent
    }

    /// The pad holds something, or the controller may still apply a state
    /// that is not NEUTRAL.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.held.is_some() || self.dragging.is_some() || self.last_sent != NEUTRAL
    }

    /// The canonical state now.
    #[must_use]
    pub fn state(&self) -> State {
        let mut s = NEUTRAL;
        if let Some(h) = self.held {
            apply(&mut s, h.control);
        }
        let [(lx, ly), (rx, ry)] = self.sticks;
        s.lx = lx;
        s.ly = ly;
        s.rx = rx;
        s.ry = ry;
        s
    }

    fn free(&self) -> bool {
        self.held.is_none() && self.dragging.is_none()
    }

    /// A pointer press on a control (its rising edge). Ignored while
    /// something is held (one control at a time) or the press is spent.
    pub fn press(&mut self, control: Control, now_us: u64) {
        if self.spent || !self.free() {
            self.pointer_down = true;
            return;
        }
        self.pointer_down = true;
        self.held = Some(Held {
            control,
            since_us: now_us,
        });
        self.release_at = None;
        self.lines.push(press_line(control, Source::Mouse));
    }

    /// An AccessKit `Click`: a tap (press now, release [`TAP_MIN_US`]
    /// later). Ignored while something is held.
    pub fn tap(&mut self, control: Control, now_us: u64) {
        if !self.free() {
            return;
        }
        self.held = Some(Held {
            control,
            since_us: now_us,
        });
        self.release_at = Some(now_us.saturating_add(TAP_MIN_US));
        self.lines.push(press_line(control, Source::AccessKit));
    }

    /// A pointer press on a stick's well starts a drag at `offset`.
    pub fn stick_start(&mut self, stick: Stick, offset: (f32, f32), full: f32) {
        if self.spent || !self.free() {
            self.pointer_down = true;
            return;
        }
        self.pointer_down = true;
        self.dragging = Some(stick);
        self.lines.push(stick_start_line(stick));
        self.stick_drag(offset, full);
    }

    /// The dragged stick follows the pointer's offset from its well centre.
    pub fn stick_drag(&mut self, offset: (f32, f32), full: f32) {
        if let Some(s) = self.dragging
            && let Some(v) = self.sticks.get_mut(s.index())
        {
            *v = stick_value(offset, full);
        }
    }

    /// A press the app ignored and spent (STOP requested in the same frame,
    /// AC 16): nothing is pressed until the pointer comes up.
    pub fn spend_press(&mut self) {
        self.pointer_down = true;
        self.spent = true;
    }

    /// The primary button came up (or is no longer down): the mouse hold
    /// ends, at once if it lasted [`TAP_MIN_US`], else when it has; the
    /// dragged stick springs back; a spent press is usable again.
    pub fn pointer_up(&mut self, now_us: u64) {
        let was_down = std::mem::replace(&mut self.pointer_down, false);
        self.spent = false;
        if !was_down {
            return;
        }
        if let Some(h) = self.held
            && self.release_at.is_none()
        {
            let due = h.since_us.saturating_add(TAP_MIN_US);
            if now_us >= due {
                self.release(Release::PointerUp);
            } else {
                self.release_at = Some(due);
            }
        }
        if self.dragging.is_some() {
            self.end_stick(Release::PointerUp);
        }
    }

    /// Applies a due tap release.
    pub fn tick(&mut self, now_us: u64) {
        if self.release_at.is_some_and(|t| now_us >= t) {
            self.release(Release::Tap);
        }
    }

    fn release(&mut self, reason: Release) {
        self.release_at = None;
        if let Some(h) = self.held.take() {
            self.lines.push(release_line(h.control, reason));
        }
    }

    fn end_stick(&mut self, reason: Release) {
        if let Some(s) = self.dragging.take() {
            if let Some(v) = self.sticks.get_mut(s.index()) {
                *v = (STICK_CENTER, STICK_CENTER);
            }
            self.lines.push(stick_end_line(s, reason));
        }
    }

    /// Releases everything (rules 8a–8d) and spends a pointer press that
    /// is still down. For `Stop`, `Gate` and `ControllerIdle` nothing is
    /// sent: the controller is neutral already or STOP makes it so.
    pub fn release_all(&mut self, reason: Release) {
        self.release(reason);
        self.end_stick(reason);
        if self.pointer_down {
            self.spent = true;
        }
        if reason.neutral_without_send() {
            self.last_sent = NEUTRAL;
            self.pending = false;
        }
    }

    /// The controller mode from the latest STATUS (`None` without one):
    /// a DIRECT → IDLE change the pad did not ask for drops what it holds
    /// (rule 8d, D7). IDLE → IDLE never does.
    pub fn observe_mode(&mut self, mode: Option<Mode>) {
        if self.last_mode == Some(Mode::Direct) && mode == Some(Mode::Idle) {
            self.release_all(Release::ControllerIdle);
        }
        self.last_mode = mode;
    }

    /// The `pad` lines produced since the last call (kind tag `pad`).
    pub fn take_lines(&mut self) -> Vec<String> {
        std::mem::take(&mut self.lines)
    }

    /// The state to send now: the current one if it differs from the last
    /// sent or a send failed. Call [`Pad::sent`] with the result.
    pub fn take_send(&mut self) -> Option<State> {
        let s = self.state();
        if self.pending || s != self.last_sent {
            self.in_flight = Some(s);
            Some(s)
        } else {
            None
        }
    }

    /// What became of the state of [`Pad::take_send`]; `Err(result)` with
    /// the `command` value on a failed `try_send` (logged once per failure
    /// streak; the state is sent again next frame).
    pub fn sent(&mut self, result: Result<(), &str>) {
        let Some(s) = self.in_flight.take() else {
            return;
        };
        match result {
            Ok(()) => {
                self.last_sent = s;
                self.pending = false;
                self.fail_logged = false;
            }
            Err(r) => {
                self.pending = true;
                if !std::mem::replace(&mut self.fail_logged, true) {
                    self.lines.push(command_line(r));
                }
            }
        }
    }

    /// When the pad needs a frame with no input (UI time, µs): a tap's
    /// release, or a resend after a failed `try_send`.
    #[must_use]
    pub fn next_wake_us(&self, now_us: u64) -> Option<u64> {
        let resend = self.pending.then(|| now_us.saturating_add(RESEND_AFTER_US));
        match (self.release_at, resend) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use super::*;

    const FULL: f32 = 17.0;

    #[test]
    fn hat_all_16_combinations() {
        for bits in 0u8..16 {
            let (u, d, l, r) = (bits & 1 != 0, bits & 2 != 0, bits & 4 != 0, bits & 8 != 0);
            let v = (u, d);
            let h = (l, r);
            let vert = match v {
                (true, false) => Some(Dir::Up),
                (false, true) => Some(Dir::Down),
                _ => None,
            };
            let horiz = match h {
                (true, false) => Some(Dir::Left),
                (false, true) => Some(Dir::Right),
                _ => None,
            };
            let want = match (vert, horiz) {
                (None, None) => HAT_NEUTRAL,
                (Some(Dir::Up), None) => HAT_UP,
                (Some(Dir::Down), None) => HAT_DOWN,
                (None, Some(Dir::Left)) => HAT_LEFT,
                (None, Some(Dir::Right)) => HAT_RIGHT,
                (Some(Dir::Up), Some(Dir::Right)) => HAT_UP_RIGHT,
                (Some(Dir::Up), Some(Dir::Left)) => HAT_UP_LEFT,
                (Some(Dir::Down), Some(Dir::Right)) => HAT_DOWN_RIGHT,
                (Some(Dir::Down), Some(Dir::Left)) => HAT_DOWN_LEFT,
                _ => unreachable!(),
            };
            assert_eq!(hat(u, d, l, r), want, "u{u} d{d} l{l} r{r}");
        }
    }

    #[test]
    fn axis_boundaries() {
        assert_eq!(axis(0.0), 128);
        assert_eq!(axis(1.0), 255);
        assert_eq!(axis(-1.0), 0);
        assert_eq!(axis(2.0), 255);
        assert_eq!(axis(-2.0), 0);
        assert_eq!(axis(f32::NAN), 128);
        assert_eq!(axis(0.5), 192); // 128 + round(63.5)
        assert_eq!(axis(-0.5), 64);
    }

    #[test]
    fn stick_value_boundaries() {
        assert_eq!(stick_value((17.0, 0.0), FULL), (255, 128));
        assert_eq!(stick_value((-17.0, 0.0), FULL), (0, 128));
        assert_eq!(stick_value((0.0, -17.0), FULL), (128, 0));
        assert_eq!(stick_value((0.0, 17.0), FULL), (128, 255));
        assert_eq!(stick_value((0.0, 0.0), FULL), (128, 128));
        let (x, y) = stick_value((30.0, 30.0), FULL);
        let r = (f32::from(x) - 128.0).hypot(f32::from(y) - 128.0);
        assert!((r - 127.5).abs() <= 127.5 * 0.02, "{x} {y} r={r}");
        assert_eq!(stick_value((f32::INFINITY, 0.0), FULL), (128, 128));
    }

    #[test]
    fn click_is_a_tap() {
        let mut p = Pad::new();
        p.press(Control::A, 1_000);
        assert_eq!(p.take_send(), Some(Control::A.alone()));
        p.sent(Ok(()));
        p.pointer_up(1_000);
        assert_eq!(p.held(), Some(Control::A), "held for the tap minimum");
        assert_eq!(p.take_send(), None);
        assert_eq!(p.next_wake_us(1_000), Some(51_000));
        p.tick(50_999);
        assert_eq!(p.held(), Some(Control::A));
        p.tick(51_000);
        assert_eq!(p.held(), None);
        assert_eq!(p.take_send(), Some(NEUTRAL));
        p.sent(Ok(()));
        assert_eq!(
            p.take_lines(),
            vec![
                "press control=a source=mouse",
                "release control=a reason=tap"
            ]
        );
    }

    #[test]
    fn long_hold_releases_at_pointer_up() {
        let mut p = Pad::new();
        p.press(Control::DLeft, 0);
        assert_eq!(p.state().hat, HAT_LEFT);
        p.pointer_up(3_000_000);
        assert_eq!(p.held(), None);
        assert_eq!(
            p.take_lines(),
            vec![
                "press control=dleft source=mouse",
                "release control=dleft reason=pointer_up"
            ]
        );
    }

    #[test]
    fn one_control_at_a_time() {
        let mut p = Pad::new();
        p.press(Control::A, 0);
        p.press(Control::B, 10);
        assert_eq!(p.state(), Control::A.alone());
        p.tap(Control::X, 20);
        assert_eq!(p.state(), Control::A.alone());
    }

    #[test]
    fn spent_until_pointer_up() {
        let mut p = Pad::new();
        p.press(Control::A, 0);
        p.release_all(Release::WindowBlur);
        assert!(p.is_spent());
        p.press(Control::B, 10);
        assert_eq!(p.held(), None, "spent: no press");
        p.pointer_up(20);
        assert!(!p.is_spent());
        p.press(Control::B, 30);
        assert_eq!(p.held(), Some(Control::B));
    }

    #[test]
    fn blur_sends_neutral_stop_does_not() {
        let mut p = Pad::new();
        p.press(Control::A, 0);
        let _ = p.take_send();
        p.sent(Ok(()));
        p.release_all(Release::WindowBlur);
        assert_eq!(p.take_send(), Some(NEUTRAL));
        p.sent(Ok(()));
        p.pointer_up(10);
        p.press(Control::A, 100);
        let _ = p.take_send();
        p.sent(Ok(()));
        p.release_all(Release::Stop);
        assert_eq!(p.take_send(), None, "STOP releases; nothing sent");
        assert_eq!(p.last_sent(), NEUTRAL);
    }

    #[test]
    fn resend_after_failed_send() {
        let mut p = Pad::new();
        p.press(Control::A, 0);
        assert_eq!(p.take_send(), Some(Control::A.alone()));
        p.sent(Err("queue_full"));
        assert_eq!(p.next_wake_us(0), Some(RESEND_AFTER_US));
        assert_eq!(p.take_send(), Some(Control::A.alone()));
        p.sent(Err("queue_full"));
        p.pointer_up(100_000);
        assert_eq!(p.take_send(), Some(NEUTRAL), "the current state, last wins");
        p.sent(Ok(()));
        assert_eq!(p.take_send(), None);
        let lines = p.take_lines();
        assert_eq!(
            lines.iter().filter(|l| l.starts_with("command=")).count(),
            1,
            "{lines:?}"
        );
    }

    #[test]
    fn controller_idle_only_on_direct_to_idle() {
        let mut p = Pad::new();
        p.observe_mode(Some(Mode::Idle));
        p.press(Control::A, 0);
        p.observe_mode(Some(Mode::Idle));
        assert_eq!(
            p.held(),
            Some(Control::A),
            "IDLE → IDLE keeps the first press"
        );
        let _ = p.take_send();
        p.sent(Ok(()));
        p.observe_mode(Some(Mode::Direct));
        p.observe_mode(Some(Mode::Idle));
        assert_eq!(p.held(), None);
        assert_eq!(p.take_send(), None, "nothing sent for it");
        assert!(
            p.take_lines()
                .contains(&"release control=a reason=controller_idle".to_owned())
        );
    }

    #[test]
    fn stick_follows_and_springs_back() {
        let mut p = Pad::new();
        p.stick_start(Stick::Left, (0.0, -17.0), FULL);
        assert_eq!(p.stick_value(Stick::Left), (128, 0));
        p.stick_drag((17.0, 0.0), FULL);
        assert_eq!(p.state().lx, 255);
        p.pointer_up(10);
        assert_eq!(p.state(), NEUTRAL);
        assert_eq!(
            p.take_lines(),
            vec!["stick=left start", "stick=left end reason=pointer_up"]
        );
    }
}
