//! The virtual gamepad as data (Stories 4.8 and 4.9; FR-11, FR-22, UX-DR36,
//! UX-DR37, AD-7, AD-16): the 16 button controls and two sticks, the mouse
//! semantics (press = press, release = release, a click is a tap of at least
//! [`TAP_MIN_US`], sticks follow the drag and spring back), the keyboard
//! ([`PadKey`]: physical keys, fresh presses only, a key tap lasts
//! [`TAP_MIN_US`] too, any number of keys held at once), the release rules,
//! the one canonical [`State`] the app sends on change (the union of the
//! mouse hold, every key hold and the sticks), the copy and the `pad` log
//! lines. Pure: no egui (G.22 checks the source); `ui::gamepad`
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

impl Control {
    /// The key hint of the compact layout's tooltip (Story 4.9 AC 9, Copy
    /// table): the control's AccessKit name, then its keys from
    /// [`PadKey::ALL`] (each key's [`PadKey::hint`], joined with " or "), or
    /// "mouse only" for Home and Capture. Built from the key table, so the
    /// tooltips never disagree with it.
    #[must_use]
    pub fn key_hint(self) -> String {
        let keys: Vec<&str> = PadKey::ALL
            .iter()
            .filter(|k| k.control() == self)
            .map(|k| k.hint())
            .collect();
        if keys.is_empty() {
            format!("{}: mouse only", self.a11y_name())
        } else {
            format!("{}: {}", self.a11y_name(), keys.join(" or "))
        }
    }
}

/// A keyboard key that drives the pad while the panel has focus (Story 4.9
/// AC 4, UX-DR37), matched by its physical position (`ui::pad_keys` maps
/// egui's `physical_key` to it). Home, Capture and the sticks have none.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PadKey {
    /// Up arrow: D-pad up.
    ArrowUp,
    /// Down arrow: D-pad down.
    ArrowDown,
    /// Left arrow: D-pad left.
    ArrowLeft,
    /// Right arrow: D-pad right.
    ArrowRight,
    /// W: D-pad up.
    W,
    /// A: D-pad left.
    A,
    /// S: D-pad down.
    S,
    /// D: D-pad right.
    D,
    /// H: Y.
    H,
    /// J: B.
    J,
    /// K: X.
    K,
    /// L: A.
    L,
    /// Q: L.
    Q,
    /// E: R.
    E,
    /// Z: ZL.
    Z,
    /// C: ZR.
    C,
    /// Enter: Plus.
    Enter,
    /// Backspace: Minus.
    Backspace,
}

impl PadKey {
    /// Every key, in the key table's order (W before the Up arrow in the
    /// hints).
    pub const ALL: [Self; 18] = [
        Self::W,
        Self::ArrowUp,
        Self::S,
        Self::ArrowDown,
        Self::A,
        Self::ArrowLeft,
        Self::D,
        Self::ArrowRight,
        Self::H,
        Self::J,
        Self::K,
        Self::L,
        Self::Q,
        Self::E,
        Self::Z,
        Self::C,
        Self::Enter,
        Self::Backspace,
    ];

    /// The control it presses (Dev Notes › Key table).
    #[must_use]
    pub const fn control(self) -> Control {
        match self {
            Self::ArrowUp | Self::W => Control::DUp,
            Self::ArrowDown | Self::S => Control::DDown,
            Self::ArrowLeft | Self::A => Control::DLeft,
            Self::ArrowRight | Self::D => Control::DRight,
            Self::H => Control::Y,
            Self::J => Control::B,
            Self::K => Control::X,
            Self::L => Control::A,
            Self::Q => Control::L,
            Self::E => Control::R,
            Self::Z => Control::Zl,
            Self::C => Control::Zr,
            Self::Enter => Control::Plus,
            Self::Backspace => Control::Minus,
        }
    }

    /// The name in the `pad` log lines (` key=<k>`).
    #[must_use]
    pub const fn log_name(self) -> &'static str {
        match self {
            Self::ArrowUp => "arrow_up",
            Self::ArrowDown => "arrow_down",
            Self::ArrowLeft => "arrow_left",
            Self::ArrowRight => "arrow_right",
            Self::W => "w",
            Self::A => "a",
            Self::S => "s",
            Self::D => "d",
            Self::H => "h",
            Self::J => "j",
            Self::K => "k",
            Self::L => "l",
            Self::Q => "q",
            Self::E => "e",
            Self::Z => "z",
            Self::C => "c",
            Self::Enter => "enter",
            Self::Backspace => "backspace",
        }
    }

    /// The key's words in a compact tooltip ([`Control::key_hint`]).
    #[must_use]
    pub const fn hint(self) -> &'static str {
        match self {
            Self::ArrowUp => "Up arrow",
            Self::ArrowDown => "Down arrow",
            Self::ArrowLeft => "Left arrow",
            Self::ArrowRight => "Right arrow",
            Self::W => "W",
            Self::A => "A",
            Self::S => "S",
            Self::D => "D",
            Self::H => "H key",
            Self::J => "J key",
            Self::K => "K key",
            Self::L => "L key",
            Self::Q => "Q key",
            Self::E => "E key",
            Self::Z => "Z key",
            Self::C => "C key",
            Self::Enter => "Enter",
            Self::Backspace => "Backspace",
        }
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
    /// A key while the panel has focus (Story 4.9).
    Keyboard,
}

impl Source {
    /// The log value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mouse => "mouse",
            Self::AccessKit => "accesskit",
            Self::Keyboard => "keyboard",
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
    /// The key came up after the tap minimum (Story 4.9).
    KeyUp,
    /// The panel lost keyboard focus (a click elsewhere, a dialog; Story
    /// 4.9 rule a′): SET_STATE with what remains, like a blur.
    FocusLost,
    /// The layout switched to the compact variant, which has no sticks
    /// (Story 4.9 AC 9; sticks only).
    Layout,
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
            Self::KeyUp => "key_up",
            Self::FocusLost => "focus_lost",
            Self::Layout => "layout",
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
    apply_all(s, [c]);
}

/// Sets the bits of every control held and the hat from every direction
/// held (two keys on opposite directions cancel on their axis, Story 4.9).
fn apply_all(s: &mut State, held: impl IntoIterator<Item = Control>) {
    let (mut up, mut down, mut left, mut right) = (false, false, false, false);
    for c in held {
        match c.effect() {
            Effect::Button(bit) => s.buttons |= bit,
            Effect::Dpad(Dir::Up) => up = true,
            Effect::Dpad(Dir::Down) => down = true,
            Effect::Dpad(Dir::Left) => left = true,
            Effect::Dpad(Dir::Right) => right = true,
        }
    }
    s.hat = hat(up, down, left, right);
}

/// The ` key=<k>` suffix of a keyboard line (empty without a key).
fn key_suffix(key: Option<PadKey>) -> String {
    key.map_or_else(String::new, |k| format!(" key={}", k.log_name()))
}

/// `pad press` (without the kind tag); ` key=<k>` for a key press.
#[must_use]
pub fn press_line(c: Control, source: Source, key: Option<PadKey>) -> String {
    format!(
        "press control={} source={}{}",
        c.log_name(),
        source.as_str(),
        key_suffix(key)
    )
}

/// `pad release`; ` key=<k>` for a key hold.
#[must_use]
pub fn release_line(c: Control, reason: Release, key: Option<PadKey>) -> String {
    format!(
        "release control={} reason={}{}",
        c.log_name(),
        reason.as_str(),
        key_suffix(key)
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

/// `pad ignored`: a press while the gate blocks the gamepad (`reason` is
/// `gating::Block::as_str`) or while the pad holds another input
/// ([`BUSY`]).
/// ` key=<k>` for a gated key press (Story 4.9 AC 7).
#[must_use]
pub fn ignored_line(t: Target, reason: &str, key: Option<PadKey>) -> String {
    format!(
        "ignored control={} reason={reason}{}",
        t.ignored_name(),
        key_suffix(key)
    )
}

/// How the panel gained keyboard focus (`pad keys=on source=<s>`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeysOn {
    /// A primary press on the panel.
    Click,
    /// Tab.
    Tab,
    /// AccessKit or anything else.
    Other,
}

impl KeysOn {
    /// The log value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Click => "click",
            Self::Tab => "tab",
            Self::Other => "other",
        }
    }
}

/// Why the panel lost keyboard focus (`pad keys=off reason=<r>`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeysOff {
    /// Esc without modifiers (egui clears focus; also STOP).
    Esc,
    /// A dialog took the focus.
    Dialog,
    /// The window lost focus or was hidden.
    WindowBlur,
    /// A click elsewhere (or anything else).
    Elsewhere,
}

impl KeysOff {
    /// The log value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Esc => "esc",
            Self::Dialog => "dialog",
            Self::WindowBlur => "window_blur",
            Self::Elsewhere => "elsewhere",
        }
    }
}

/// `pad keys=on source=<s>`.
#[must_use]
pub fn keys_on_line(source: KeysOn) -> String {
    format!("keys=on source={}", source.as_str())
}

/// `pad keys=off reason=<r>`.
#[must_use]
pub fn keys_off_line(reason: KeysOff) -> String {
    format!("keys=off reason={}", reason.as_str())
}

/// The `pad ignored` reason of a press while the pad already holds an
/// input (one at a time by mouse; review 2026-11-27).
pub const BUSY: &str = "busy";

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

/// One key held (Story 4.9): released at `release_at` when the key came up
/// before the tap minimum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct KeyHold {
    key: PadKey,
    since_us: u64,
    release_at: Option<u64>,
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
    /// The keys held (Story 4.9), in press order; tracked apart from the
    /// mouse hold.
    keys: Vec<KeyHold>,
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
    /// When the newest STATUS seen arrived (worker clock, µs).
    last_status_rx: Option<u64>,
    /// The newest STATUS arrival known when the last state was handed to
    /// the worker: rule 8d honours only a STATUS that arrived later.
    sent_after_rx: Option<u64>,
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
            keys: Vec::with_capacity(PadKey::ALL.len()),
            dragging: None,
            sticks: [(STICK_CENTER, STICK_CENTER); 2],
            pointer_down: false,
            spent: false,
            last_sent: NEUTRAL,
            pending: false,
            in_flight: None,
            fail_logged: false,
            last_mode: None,
            last_status_rx: None,
            sent_after_rx: None,
            lines: Vec::new(),
        }
    }

    /// The control held (mouse or AccessKit tap), if any.
    #[must_use]
    pub fn held(&self) -> Option<Control> {
        self.held.map(|h| h.control)
    }

    /// The keys held, in press order (Story 4.9).
    pub fn keys_held(&self) -> impl Iterator<Item = PadKey> + '_ {
        self.keys.iter().map(|k| k.key)
    }

    /// Whether `c` is held by the mouse, an AccessKit tap or a key.
    #[must_use]
    pub fn is_pressed(&self, c: Control) -> bool {
        self.held() == Some(c) || self.keys.iter().any(|k| k.key.control() == c)
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
        self.held.is_some()
            || !self.keys.is_empty()
            || self.dragging.is_some()
            || self.last_sent != NEUTRAL
    }

    /// The canonical state now: the union of the mouse hold and every key
    /// hold (buttons OR-ed, the hat from every direction held), plus the
    /// sticks.
    #[must_use]
    pub fn state(&self) -> State {
        let mut s = NEUTRAL;
        apply_all(
            &mut s,
            self.held
                .map(|h| h.control)
                .into_iter()
                .chain(self.keys.iter().map(|k| k.key.control())),
        );
        let [(lx, ly), (rx, ry)] = self.sticks;
        s.lx = lx;
        s.ly = ly;
        s.rx = rx;
        s.ry = ry;
        s
    }

    /// The mouse is free: no mouse hold, no drag (key holds do not count:
    /// a mouse press while keys are held is not `busy`).
    fn free(&self) -> bool {
        self.held.is_none() && self.dragging.is_none()
    }

    /// A pointer press on a control (its rising edge). Ignored while
    /// something is held (one control at a time: `pad ignored … reason=busy`)
    /// or the press is spent.
    pub fn press(&mut self, control: Control, now_us: u64) {
        if !self.spent && !self.free() {
            self.lines
                .push(ignored_line(Target::Control(control), BUSY, None));
        }
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
        self.lines.push(press_line(control, Source::Mouse, None));
    }

    /// An AccessKit `Click`: a tap (press now, release [`TAP_MIN_US`]
    /// later). Ignored while something is held.
    pub fn tap(&mut self, control: Control, now_us: u64) {
        if !self.free() {
            self.lines
                .push(ignored_line(Target::Control(control), BUSY, None));
            return;
        }
        self.held = Some(Held {
            control,
            since_us: now_us,
        });
        self.release_at = Some(now_us.saturating_add(TAP_MIN_US));
        self.lines
            .push(press_line(control, Source::AccessKit, None));
    }

    /// A pointer press on a stick's well starts a drag at `offset`.
    pub fn stick_start(&mut self, stick: Stick, offset: (f32, f32), full: f32) {
        if !self.spent && !self.free() {
            self.lines
                .push(ignored_line(Target::Stick(stick), BUSY, None));
        }
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

    /// A fresh press of `key` (Story 4.9 AC 5; the caller skips OS key
    /// repeats, Cmd/Ctrl presses and gated presses): its control is held
    /// from now. A key already held is ignored (no line). The `press` line
    /// is written also when the control is already held by the mouse or
    /// another key (then the state does not change and nothing is sent).
    ///
    /// A fresh press of a key whose release is still waiting for its tap
    /// minimum (pressed again within 50 ms) keeps the hold: the pending
    /// release is cancelled, the hold keeps its start, and the press line
    /// is written (review 2026-11-28), so the next release ends it.
    pub fn key_down(&mut self, key: PadKey, now_us: u64) {
        if let Some(k) = self.keys.iter_mut().find(|k| k.key == key) {
            if k.release_at.take().is_some() {
                self.lines
                    .push(press_line(key.control(), Source::Keyboard, Some(key)));
            }
            return;
        }
        self.keys.push(KeyHold {
            key,
            since_us: now_us,
            release_at: None,
        });
        self.lines
            .push(press_line(key.control(), Source::Keyboard, Some(key)));
    }

    /// `key` came up: its hold ends now if it lasted [`TAP_MIN_US`]
    /// (`reason=key_up`), else when it has (`reason=tap`). A key not held
    /// (or already waiting for its tap minimum) is ignored.
    pub fn key_up(&mut self, key: PadKey, now_us: u64) {
        let Some(i) = self
            .keys
            .iter()
            .position(|k| k.key == key && k.release_at.is_none())
        else {
            return;
        };
        let due = self
            .keys
            .get(i)
            .map_or(now_us, |k| k.since_us.saturating_add(TAP_MIN_US));
        if now_us >= due {
            self.release_key(i, Release::KeyUp);
        } else if let Some(k) = self.keys.get_mut(i) {
            k.release_at = Some(due);
        }
    }

    fn release_key(&mut self, i: usize, reason: Release) {
        if i < self.keys.len() {
            let k = self.keys.remove(i);
            self.lines
                .push(release_line(k.key.control(), reason, Some(k.key)));
        }
    }

    /// Applies the due tap releases (mouse, AccessKit and keys).
    pub fn tick(&mut self, now_us: u64) {
        if self.release_at.is_some_and(|t| now_us >= t) {
            self.release(Release::Tap);
        }
        while let Some(i) = self
            .keys
            .iter()
            .position(|k| k.release_at.is_some_and(|t| now_us >= t))
        {
            self.release_key(i, Release::Tap);
        }
    }

    fn release(&mut self, reason: Release) {
        self.release_at = None;
        if let Some(h) = self.held.take() {
            self.lines.push(release_line(h.control, reason, None));
        }
    }

    /// The layout switched to the compact variant while a stick is dragged:
    /// the drag ends at once and the stick springs back (Story 4.9 AC 9,
    /// `stick=<s> end reason=layout`); the pointer press is spent until it
    /// comes up. Button holds are kept.
    pub fn end_drag_for_layout(&mut self) {
        if self.dragging.is_some() {
            self.end_stick(Release::Layout);
            if self.pointer_down {
                self.spent = true;
            }
        }
    }

    /// Writes `pad keys=on source=<s>` (the panel gained keyboard focus).
    pub fn keys_on(&mut self, source: KeysOn) {
        self.lines.push(keys_on_line(source));
    }

    /// Writes `pad keys=off reason=<r>` (the panel lost keyboard focus).
    pub fn keys_off(&mut self, reason: KeysOff) {
        self.lines.push(keys_off_line(reason));
    }

    /// Writes `pad ignored control=<c> reason=<block>`: a pointer press or
    /// AccessKit tap while the gate blocks the gamepad (through the pad's
    /// lines, so it follows a `keys=on` line of the same press).
    pub fn ignored(&mut self, t: Target, block: &str) {
        self.lines.push(ignored_line(t, block, None));
    }

    /// Writes `pad ignored control=<c> reason=<block> key=<k>`: a fresh
    /// key press while the gate blocks the gamepad (Story 4.9 AC 7).
    pub fn ignored_key(&mut self, key: PadKey, block: &str) {
        self.lines.push(ignored_line(
            Target::Control(key.control()),
            block,
            Some(key),
        ));
    }

    fn end_stick(&mut self, reason: Release) {
        if let Some(s) = self.dragging.take() {
            if let Some(v) = self.sticks.get_mut(s.index()) {
                *v = (STICK_CENTER, STICK_CENTER);
            }
            self.lines.push(stick_end_line(s, reason));
        }
    }

    /// Releases everything (rules 8a–8d and Story 4.9's a′): every key
    /// hold (one `release … key=<k>` line each), the mouse hold and the
    /// drag; spends a pointer press that is still down. For `Stop`, `Gate`
    /// and `ControllerIdle` nothing is sent: the controller is neutral
    /// already or STOP makes it so.
    pub fn release_all(&mut self, reason: Release) {
        while !self.keys.is_empty() {
            self.release_key(0, reason);
        }
        self.release(reason);
        self.end_stick(reason);
        if self.pointer_down {
            self.spent = true;
        }
        if reason.neutral_without_send() {
            self.last_sent = NEUTRAL;
            self.pending = false;
        }
        if matches!(reason, Release::Stop | Release::Gate) {
            // The STATUS the UI holds may predate the STOP: the next one only
            // re-arms rule 8d (review 2026-11-27).
            self.last_mode = None;
        }
    }

    /// The controller mode from the latest STATUS and when it arrived
    /// (worker clock; `None` without one): a DIRECT → IDLE change the pad
    /// did not ask for drops what it holds (rule 8d, D7), but only for a
    /// STATUS that arrived after the last state was handed over (an older
    /// one cannot know about it). IDLE → IDLE never does.
    pub fn observe_mode(&mut self, mode: Option<Mode>, rx_us: Option<u64>) {
        let newer = match (rx_us, self.sent_after_rx) {
            (Some(rx), Some(sent)) => rx > sent,
            _ => true,
        };
        if self.last_mode == Some(Mode::Direct) && mode == Some(Mode::Idle) && newer {
            self.release_all(Release::ControllerIdle);
        }
        if rx_us.is_none() || rx_us != self.last_status_rx {
            self.last_mode = mode;
        }
        self.last_status_rx = rx_us;
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
                self.sent_after_rx = self.last_status_rx;
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
    /// release (mouse, AccessKit or a key), or a resend after a failed
    /// `try_send`.
    #[must_use]
    pub fn next_wake_us(&self, now_us: u64) -> Option<u64> {
        let resend = self.pending.then(|| now_us.saturating_add(RESEND_AFTER_US));
        [self.release_at, resend]
            .into_iter()
            .chain(self.keys.iter().map(|k| k.release_at))
            .flatten()
            .min()
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
        p.observe_mode(Some(Mode::Idle), None);
        p.press(Control::A, 0);
        p.observe_mode(Some(Mode::Idle), None);
        assert_eq!(
            p.held(),
            Some(Control::A),
            "IDLE → IDLE keeps the first press"
        );
        let _ = p.take_send();
        p.sent(Ok(()));
        p.observe_mode(Some(Mode::Direct), None);
        p.observe_mode(Some(Mode::Idle), None);
        assert_eq!(p.held(), None);
        assert_eq!(p.take_send(), None, "nothing sent for it");
        assert!(
            p.take_lines()
                .contains(&"release control=a reason=controller_idle".to_owned())
        );
    }

    /// Review 2026-11-27: after a STOP the UI may still hold a DIRECT
    /// STATUS; a press sent before the IDLE STATUS arrives is not dropped by
    /// that older IDLE STATUS.
    #[test]
    fn controller_idle_ignores_a_stale_status_after_stop() {
        let mut p = Pad::new();
        p.observe_mode(Some(Mode::Direct), Some(1_000));
        p.press(Control::A, 0);
        let _ = p.take_send();
        p.sent(Ok(()));
        p.release_all(Release::Stop);
        p.pointer_up(10);
        // The same (pre-STOP) STATUS seen again: nothing re-armed.
        p.observe_mode(Some(Mode::Direct), Some(1_000));
        p.press(Control::B, 20);
        let _ = p.take_send();
        p.sent(Ok(()));
        // The IDLE STATUS of the STOP arrives late.
        p.observe_mode(Some(Mode::Idle), Some(1_050));
        assert_eq!(p.held(), Some(Control::B), "B is not dropped");
        // A STATUS older than the send never drops it either.
        let mut q = Pad::new();
        q.observe_mode(Some(Mode::Direct), Some(2_000));
        q.press(Control::X, 0);
        let _ = q.take_send();
        q.sent(Ok(()));
        q.observe_mode(Some(Mode::Idle), Some(2_000));
        assert_eq!(q.held(), Some(Control::X));
        q.observe_mode(Some(Mode::Idle), Some(2_050));
        assert_eq!(q.held(), None, "a newer IDLE STATUS drops it");
    }

    #[test]
    fn busy_presses_are_logged() {
        let mut p = Pad::new();
        p.press(Control::A, 0);
        p.pointer_up(10);
        p.press(Control::B, 20);
        p.tap(Control::X, 30);
        let lines = p.take_lines();
        assert!(
            lines.contains(&"ignored control=b reason=busy".to_owned()),
            "{lines:?}"
        );
        assert!(
            lines.contains(&"ignored control=x reason=busy".to_owned()),
            "{lines:?}"
        );
        assert_eq!(p.held(), Some(Control::A));
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

    // Story 4.9: the keyboard.

    /// The key table (Dev Notes › Key table): 18 keys, each to its Story
    /// 4.8 bit or hat; Home, Capture and the sticks have none.
    #[test]
    fn key_table() {
        let want: [(PadKey, &str, State); 18] = [
            (PadKey::ArrowUp, "arrow_up", hat_state(HAT_UP)),
            (PadKey::W, "w", hat_state(HAT_UP)),
            (PadKey::ArrowDown, "arrow_down", hat_state(HAT_DOWN)),
            (PadKey::S, "s", hat_state(HAT_DOWN)),
            (PadKey::ArrowLeft, "arrow_left", hat_state(HAT_LEFT)),
            (PadKey::A, "a", hat_state(HAT_LEFT)),
            (PadKey::ArrowRight, "arrow_right", hat_state(HAT_RIGHT)),
            (PadKey::D, "d", hat_state(HAT_RIGHT)),
            (PadKey::H, "h", btn(BTN_Y)),
            (PadKey::J, "j", btn(BTN_B)),
            (PadKey::K, "k", btn(BTN_X)),
            (PadKey::L, "l", btn(BTN_A)),
            (PadKey::Q, "q", btn(BTN_L)),
            (PadKey::E, "e", btn(BTN_R)),
            (PadKey::Z, "z", btn(BTN_ZL)),
            (PadKey::C, "c", btn(BTN_ZR)),
            (PadKey::Enter, "enter", btn(BTN_PLUS)),
            (PadKey::Backspace, "backspace", btn(BTN_MINUS)),
        ];
        assert_eq!(PadKey::ALL.len(), 18);
        for (k, name, state) in want {
            assert!(PadKey::ALL.contains(&k), "{k:?}");
            assert_eq!(k.log_name(), name);
            let mut p = Pad::new();
            p.key_down(k, 0);
            assert_eq!(p.state(), state, "{k:?}");
        }
        for c in [Control::Home, Control::Capture] {
            assert!(PadKey::ALL.iter().all(|k| k.control() != c), "{c:?}");
        }
    }

    fn hat_state(h: u8) -> State {
        State { hat: h, ..NEUTRAL }
    }

    fn btn(b: u16) -> State {
        State {
            buttons: b,
            ..NEUTRAL
        }
    }

    /// The compact tooltips come from the key table: every key is named in
    /// exactly one hint (its control's), Home and Capture say "mouse only",
    /// and the strings are the Copy table's.
    #[test]
    fn key_hints() {
        let hints: Vec<(Control, String)> =
            Control::ALL.iter().map(|c| (*c, c.key_hint())).collect();
        for k in PadKey::ALL {
            let owners: Vec<Control> = hints
                .iter()
                .filter(|(_, h)| {
                    h.split(": ")
                        .nth(1)
                        .is_some_and(|keys| keys.split(" or ").any(|w| w == k.hint()))
                })
                .map(|(c, _)| *c)
                .collect();
            assert_eq!(owners, vec![k.control()], "{k:?}");
        }
        let copy = [
            (Control::A, "A: L key"),
            (Control::B, "B: J key"),
            (Control::X, "X: K key"),
            (Control::Y, "Y: H key"),
            (Control::DUp, "D-pad up: W or Up arrow"),
            (Control::DDown, "D-pad down: S or Down arrow"),
            (Control::DLeft, "D-pad left: A or Left arrow"),
            (Control::DRight, "D-pad right: D or Right arrow"),
            (Control::L, "L: Q key"),
            (Control::R, "R: E key"),
            (Control::Zl, "ZL: Z key"),
            (Control::Zr, "ZR: C key"),
            (Control::Plus, "Plus: Enter"),
            (Control::Minus, "Minus: Backspace"),
            (Control::Home, "Home: mouse only"),
            (Control::Capture, "Capture: mouse only"),
        ];
        for (c, text) in copy {
            assert_eq!(c.key_hint(), text);
        }
    }

    /// A key tap is a 50 ms tap; a long hold ends at the key's release; a
    /// second press of a held key (an OS repeat the caller let through)
    /// adds nothing.
    #[test]
    fn key_tap_hold_and_repeat() {
        let mut p = Pad::new();
        p.key_down(PadKey::L, 1_000);
        assert_eq!(p.take_send(), Some(btn(BTN_A)));
        p.sent(Ok(()));
        p.key_down(PadKey::L, 2_000);
        p.key_up(PadKey::L, 2_000);
        assert!(p.is_pressed(Control::A), "held for the tap minimum");
        assert_eq!(p.next_wake_us(2_000), Some(51_000));
        p.tick(50_999);
        assert!(p.is_pressed(Control::A));
        p.tick(51_000);
        assert!(!p.is_pressed(Control::A));
        assert_eq!(p.take_send(), Some(NEUTRAL));
        p.sent(Ok(()));
        assert_eq!(
            p.take_lines(),
            vec![
                "press control=a source=keyboard key=l",
                "release control=a reason=tap key=l"
            ]
        );
        p.key_down(PadKey::L, 100_000);
        p.key_up(PadKey::L, 3_100_000);
        assert_eq!(
            p.take_lines(),
            vec![
                "press control=a source=keyboard key=l",
                "release control=a reason=key_up key=l"
            ]
        );
        // A release of a key never pressed: nothing.
        p.key_up(PadKey::K, 4_000_000);
        assert!(p.take_lines().is_empty());
        assert_eq!(p.take_send(), None);
    }

    /// Review 2026-11-28: a key pressed again while its release waits for
    /// the tap minimum is held again (one press line), and its next
    /// release ends it; nothing is sent in between.
    #[test]
    fn key_repress_in_the_tap_window() {
        let mut p = Pad::new();
        p.key_down(PadKey::L, 0);
        let _ = p.take_send();
        p.sent(Ok(()));
        p.key_up(PadKey::L, 10_000);
        p.key_down(PadKey::L, 20_000);
        p.tick(60_000);
        assert!(
            p.is_pressed(Control::A),
            "held again, the tap release cancelled"
        );
        assert_eq!(p.take_send(), None, "no change on the wire");
        assert_eq!(p.next_wake_us(60_000), None);
        p.key_up(PadKey::L, 2_000_000);
        assert!(!p.is_pressed(Control::A));
        assert_eq!(
            p.take_lines(),
            vec![
                "press control=a source=keyboard key=l",
                "press control=a source=keyboard key=l",
                "release control=a reason=key_up key=l"
            ]
        );
        assert_eq!(p.take_send(), Some(NEUTRAL));
    }

    /// Chords: the union of every key held; duplicates keep the control
    /// while either key is held; opposite directions cancel on their axis.
    #[test]
    fn key_chords_duplicates_opposites() {
        let mut p = Pad::new();
        p.key_down(PadKey::L, 0);
        p.key_down(PadKey::K, 10);
        assert_eq!(p.state().buttons, BTN_A | BTN_X);
        p.key_up(PadKey::K, 100_000);
        assert_eq!(p.state().buttons, BTN_A);
        p.key_up(PadKey::L, 100_000);
        assert_eq!(p.state(), NEUTRAL);

        p.key_down(PadKey::W, 200_000);
        p.key_down(PadKey::ArrowUp, 200_010);
        assert_eq!(p.state().hat, HAT_UP);
        p.key_up(PadKey::W, 300_000);
        assert_eq!(p.state().hat, HAT_UP, "ArrowUp still holds up");
        p.key_up(PadKey::ArrowUp, 300_000);
        assert_eq!(p.state().hat, HAT_NEUTRAL);

        p.key_down(PadKey::ArrowLeft, 400_000);
        assert_eq!(p.state().hat, HAT_LEFT);
        p.key_down(PadKey::D, 400_010);
        assert_eq!(p.state().hat, HAT_NEUTRAL, "opposites cancel");
        p.key_up(PadKey::ArrowLeft, 500_000);
        assert_eq!(p.state().hat, HAT_RIGHT);
        p.key_down(PadKey::W, 500_010);
        assert_eq!(p.state().hat, HAT_UP_RIGHT, "a diagonal");
    }

    /// Key holds and the mouse hold are separate; the state is their union.
    #[test]
    fn keys_and_mouse_union() {
        let mut p = Pad::new();
        p.press(Control::A, 0);
        p.key_down(PadKey::L, 10);
        p.key_up(PadKey::L, 100_000);
        assert_eq!(
            p.held(),
            Some(Control::A),
            "the key never ends the mouse hold"
        );
        assert_eq!(p.state().buttons, BTN_A);
        p.pointer_up(200_000);
        assert_eq!(p.state(), NEUTRAL);
        p.key_down(PadKey::L, 300_000);
        p.press(Control::B, 300_010);
        assert_eq!(p.held(), Some(Control::B), "a key hold is not busy");
        assert_eq!(p.state().buttons, BTN_A | BTN_B);
        p.pointer_up(400_000);
        assert_eq!(p.state().buttons, BTN_A, "the mouse never ends a key hold");
        assert!(!p.take_lines().iter().any(|l| l.contains("busy")));
    }

    /// `release_all` ends every key hold with its own line, for each reason;
    /// NEUTRAL is sent for blur and focus loss, not for STOP, gate, idle.
    #[test]
    fn release_all_with_keys() {
        for (reason, sends) in [
            (Release::WindowBlur, true),
            (Release::FocusLost, true),
            (Release::Stop, false),
            (Release::Gate, false),
            (Release::ControllerIdle, false),
        ] {
            let mut p = Pad::new();
            p.key_down(PadKey::L, 0);
            p.key_down(PadKey::ArrowUp, 0);
            let _ = p.take_send();
            p.sent(Ok(()));
            let _ = p.take_lines();
            p.release_all(reason);
            let r = reason.as_str();
            assert_eq!(
                p.take_lines(),
                vec![
                    format!("release control=a reason={r} key=l"),
                    format!("release control=dup reason={r} key=arrow_up"),
                ],
                "{r}"
            );
            assert!(!p.is_active() || sends, "{r}");
            assert_eq!(p.take_send(), sends.then_some(NEUTRAL), "{r}");
            assert_eq!(p.keys_held().count(), 0);
        }
    }

    /// Rule 8d with only a key hold: dropped, nothing sent.
    #[test]
    fn controller_idle_drops_key_holds() {
        let mut p = Pad::new();
        p.observe_mode(Some(Mode::Direct), Some(1));
        p.key_down(PadKey::L, 0);
        let _ = p.take_send();
        p.sent(Ok(()));
        p.observe_mode(Some(Mode::Idle), Some(2));
        assert_eq!(p.keys_held().count(), 0);
        assert_eq!(p.take_send(), None);
        assert!(
            p.take_lines()
                .contains(&"release control=a reason=controller_idle key=l".to_owned())
        );
    }

    /// `next_wake_us` is the earliest of several key deadlines.
    #[test]
    fn next_wake_with_key_deadlines() {
        let mut p = Pad::new();
        p.key_down(PadKey::L, 0);
        p.key_down(PadKey::K, 20_000);
        p.key_up(PadKey::K, 21_000);
        p.key_up(PadKey::L, 30_000);
        assert_eq!(p.next_wake_us(30_000), Some(50_000));
        p.tick(50_000);
        assert_eq!(p.next_wake_us(50_000), Some(70_000));
        p.tick(70_000);
        assert_eq!(p.next_wake_us(70_000), None);
        assert_eq!(p.state(), NEUTRAL);
    }

    /// The layout switch ends a drag (`reason=layout`), keeps button holds.
    #[test]
    fn layout_ends_drag_only() {
        let mut p = Pad::new();
        p.key_down(PadKey::L, 0);
        p.stick_start(Stick::Left, (17.0, 0.0), FULL);
        p.end_drag_for_layout();
        assert_eq!(p.dragging(), None);
        assert_eq!(p.state(), btn(BTN_A));
        assert!(p.is_spent());
        assert!(
            p.take_lines()
                .contains(&"stick=left end reason=layout".to_owned())
        );
    }

    /// The focus lines and the gated key line.
    #[test]
    fn focus_and_ignored_key_lines() {
        let mut p = Pad::new();
        p.keys_on(KeysOn::Click);
        p.keys_on(KeysOn::Tab);
        p.keys_on(KeysOn::Other);
        for r in [
            KeysOff::Esc,
            KeysOff::Dialog,
            KeysOff::WindowBlur,
            KeysOff::Elsewhere,
        ] {
            p.keys_off(r);
        }
        p.ignored_key(PadKey::Enter, "no_bridge");
        assert_eq!(
            p.take_lines(),
            vec![
                "keys=on source=click",
                "keys=on source=tab",
                "keys=on source=other",
                "keys=off reason=esc",
                "keys=off reason=dialog",
                "keys=off reason=window_blur",
                "keys=off reason=elsewhere",
                "ignored control=plus reason=no_bridge key=enter",
            ]
        );
        assert_eq!(p.take_send(), None);
    }
}
