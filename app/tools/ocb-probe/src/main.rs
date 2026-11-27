//! `ocb-probe`: see `lib.rs` and `--help`.

use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ocb_probe::cli::{self, Command};
use ocb_probe::real::RealDriver;
use ocb_probe::runner::{self, EXIT_FAIL, EXIT_USAGE, host_name};
use ocb_services::port::SerialBackend;
use ocb_services::worker::host_rules;

fn main() -> ExitCode {
    let cmd = match cli::parse(std::env::args().skip(1)) {
        Ok(c) => c,
        Err(cli::UsageError(arg)) => {
            eprintln!("ocb-probe: bad argument '{arg}'\n\n{}", cli::USAGE);
            return ExitCode::from(EXIT_USAGE);
        }
    };
    let code = match cmd {
        Command::Help => {
            println!(
                "ocb-probe {}\n\n{}",
                ocb_core::version::APP_VERSION,
                cli::USAGE
            );
            0
        }
        Command::List => {
            // Enumeration only: `list` never opens a port.
            let (lines, code) =
                runner::list_lines(SerialBackend::new().list_raw(), host_rules(), host_name());
            for l in lines {
                println!("{l}");
            }
            code
        }
        Command::Run { .. } | Command::Permission { .. } | Command::Alone { .. } => {
            let interrupt = Arc::new(AtomicBool::new(false));
            let flag = Arc::clone(&interrupt);
            if let Err(e) = ctrlc::set_handler(move || flag.store(true, Ordering::SeqCst)) {
                eprintln!("ocb-probe: cannot catch Ctrl+C: {e}");
            }
            match RealDriver::start(interrupt) {
                Err(e) => {
                    println!("OCB_PROBE FAIL (cannot start the worker: {e})");
                    EXIT_FAIL
                }
                Ok(mut d) => match cmd {
                    Command::Run { step_timeout_s } => runner::run(&mut d, step_timeout_s),
                    Command::Permission { timeout_s } => runner::permission(&mut d, timeout_s),
                    Command::Alone { seconds } => runner::alone(&mut d, seconds),
                    Command::Help | Command::List => EXIT_FAIL,
                },
            }
        }
    };
    ExitCode::from(code)
}
