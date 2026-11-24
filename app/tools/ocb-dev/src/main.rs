//! `ocb-dev <port> <timeline.csv | soak>` (Story 3.4): see README.md.

use std::io::BufRead;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use ocb_core::link::SessionConfig;
use ocb_dev::port_rule::port_rule;
use ocb_dev::runner::{self, EXIT_INTERRUPTED, EXIT_USAGE, Limits, Owner, Piece};
use ocb_dev::serial::SerialIo;
use ocb_dev::soak::{SOAK_DURATION_US, SOAK_LEAD_US, SOAK_PERIOD_US};
use ocb_dev::timeline;

const USAGE: &str = "usage: ocb-dev <port> <timeline.csv | soak | failsafe | watch>
  port      the bridge's serial port (macOS: /dev/cu.usbmodem…)
  timeline  a t_us,state CSV (app/tools/ocb-dev/timelines/), or a keyword:
            soak      the 10-minute link soak (Story 3.4, H.26)
            failsafe  20 rounds of the melody, each ended by unplugging the
                      bridge or pulling a UART wire (Story 3.5, A.3)
            watch     records console_ready, silences and reboots until
                      Ctrl+C, then judges H.3 (Story 3.5)
            (a file with one of these names: ./soak, ./failsafe, ./watch)
exit: 0 PASS, 1 FAIL, 2 usage or file error, 130 interrupted (Ctrl+C)";

/// The terminal: lines to stdout, Enter from a stdin thread.
struct TerminalOwner {
    enter: Receiver<()>,
}

impl TerminalOwner {
    fn new() -> Self {
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let stdin = std::io::stdin();
            for line in stdin.lock().lines() {
                if line.is_err() || tx.send(()).is_err() {
                    break;
                }
            }
        });
        Self { enter: rx }
    }
}

impl Owner for TerminalOwner {
    fn say(&mut self, line: &str) {
        println!("{line}");
    }

    fn discard_input(&mut self) {
        while self.enter.try_recv().is_ok() {}
    }

    fn enter_pressed(&mut self) -> bool {
        let mut any = false;
        loop {
            match self.enter.try_recv() {
                Ok(()) => any = true,
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => return any,
            }
        }
    }
}

fn exit(code: i32) -> ExitCode {
    ExitCode::from(u8::try_from(code).unwrap_or(1))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [port, what] = args.as_slice() else {
        eprintln!("{USAGE}");
        return exit(EXIT_USAGE);
    };
    if port.starts_with('-') || what.starts_with('-') {
        eprintln!("{USAGE}");
        return exit(EXIT_USAGE);
    }
    if let Err(e) = port_rule(port, cfg!(target_os = "macos")) {
        eprintln!("ocb-dev: {e}");
        return exit(EXIT_USAGE);
    }
    let piece = if what == "soak" {
        Piece::Soak {
            duration_us: SOAK_DURATION_US,
            period_us: SOAK_PERIOD_US,
            lead_us: SOAK_LEAD_US,
        }
    } else if what == "failsafe" {
        Piece::Failsafe { rounds: 20 }
    } else if what == "watch" {
        Piece::Watch {
            max_us: 30 * 60 * 1_000_000,
        }
    } else {
        let text = match std::fs::read_to_string(what) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("ocb-dev: {what}: {e}");
                return exit(EXIT_USAGE);
            }
        };
        match timeline::parse(&text) {
            Ok(tl) => Piece::Melody(tl),
            Err(e) => {
                eprintln!("ocb-dev: {what}: {e}");
                return exit(EXIT_USAGE);
            }
        }
    };
    let interrupt = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&interrupt);
    if let Err(e) = ctrlc::set_handler(move || {
        // A second Ctrl+C leaves at once, without STOP: closing the port
        // drops DTR, which aborts DIRECT (AD-5) but not PLAYING, so a
        // piece already on the controller plays on (unplug the bridge).
        if flag.swap(true, Ordering::SeqCst) {
            std::process::exit(EXIT_INTERRUPTED);
        }
    }) {
        eprintln!("ocb-dev: Ctrl+C handler: {e}");
        return exit(EXIT_USAGE);
    }
    let mut io = match SerialIo::open(port) {
        Ok(io) => io,
        Err(e) => {
            eprintln!("ocb-dev: {e}");
            return exit(runner::EXIT_FAIL);
        }
    };
    let mut owner = TerminalOwner::new();
    println!(
        "ocb-dev {} on {port}: {}",
        ocb_core::version::APP_VERSION,
        match what.as_str() {
            "soak" => "soak, 10 minutes",
            "failsafe" => "failsafe, 20 rounds",
            "watch" => "watch (H.3), until Ctrl+C",
            other => other,
        }
    );
    let out = runner::run(
        &mut io,
        &mut owner,
        interrupt,
        piece,
        Limits::default(),
        SessionConfig::default(),
    );
    exit(out.code)
}
