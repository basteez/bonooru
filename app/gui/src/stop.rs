//! STOP as the app sees it (Story 4.7; UX-DR41, AD-16): who asked for it,
//! what became of the command, the `stop requested` log line and the copy
//! of the bottom bar. Pure: no egui (G.19 checks the source).

/// What requested a STOP.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopSource {
    /// A fresh press of Esc, read at the top of the frame.
    Esc,
    /// The STOP button: a click, Enter while it has focus, or an AccessKit
    /// click.
    Button,
}

impl StopSource {
    /// The log value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Esc => "esc",
            Self::Button => "button",
        }
    }
}

/// What happened to the `WorkerCommand::Stop` the UI tried to send (it
/// never waits: `try_send`, AD-16).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandResult {
    /// In the worker's queue.
    Queued,
    /// The worker thread could not be started.
    NoWorker,
    /// The worker thread ended.
    WorkerGone,
    /// The worker's command queue is full (it is stuck).
    QueueFull,
}

impl CommandResult {
    /// The log value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::NoWorker => "no_worker",
            Self::WorkerGone => "worker_gone",
            Self::QueueFull => "queue_full",
        }
    }
}

/// The text of the app's `stop` line: `requested source=<esc|button>
/// dialog=<open|none> command=<queued|no_worker|worker_gone|queue_full>`.
#[must_use]
pub fn requested_line(source: StopSource, dialog_open: bool, result: CommandResult) -> String {
    format!(
        "requested source={} dialog={} command={}",
        source.as_str(),
        if dialog_open { "open" } else { "none" },
        result.as_str()
    )
}

/// The STOP button's label (`DESIGN.md` › Bottom bar).
pub const STOP_LABEL: &str = "STOP";
/// The STOP button's sub-line.
pub const STOP_SUB: &str = "Esc";
/// STOP's AccessKit name: plain case, so screen readers do not spell it.
pub const STOP_A11Y_NAME: &str = "Stop";
/// STOP's AccessKit description.
pub const STOP_A11Y_DESCRIPTION: &str = "Esc stops at any time.";
/// The safety note, first line (`EXPERIENCE.md` Voice table; ASCII
/// apostrophe, owner decision 6).
pub const SAFETY_LINE_1: &str = "Don't dock the Switch mid-song.";
/// The safety note, second line.
pub const SAFETY_LINE_2: &str = "Esc stops at any time.";

/// Every string of the bottom bar (G.19 runs the banned-words check on
/// them).
pub const COPY: [&str; 6] = [
    STOP_LABEL,
    STOP_SUB,
    STOP_A11Y_NAME,
    STOP_A11Y_DESCRIPTION,
    SAFETY_LINE_1,
    SAFETY_LINE_2,
];
