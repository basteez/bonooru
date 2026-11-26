//! `bonooru`: the Bonooru desktop app.
//!
//! Exit codes: 0 ok, 1 eframe error (or a smoke run whose window closed
//! before its report), 2 usage, 3 smoke watchdog, 4 smoke verdict failed.

use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ocb_app::app::App;
use ocb_app::cli::{self, Command};
use ocb_app::smoke::{self, OutcomeCell, Smoke};

fn main() -> ExitCode {
    let (mode, smoke_test) = match cli::parse(std::env::args_os().skip(1)) {
        Ok(Command::Help) => {
            println!(
                "bonooru {}\n\n{}",
                ocb_core::version::APP_VERSION,
                cli::USAGE
            );
            return ExitCode::SUCCESS;
        }
        Ok(Command::Run { mode, smoke }) => (mode, smoke),
        Err(cli::UsageError(arg)) => {
            eprintln!("bonooru: unknown argument '{arg}'\n\n{}", cli::USAGE);
            return ExitCode::from(smoke::EXIT_USAGE);
        }
    };

    // Set only after `run_native` returns: the watchdog stays armed until
    // the window has really closed (review finding 2).
    let exited = Arc::new(AtomicBool::new(false));
    if smoke_test {
        smoke::spawn_watchdog(Arc::clone(&exited));
    }
    let outcome = OutcomeCell::default();
    let app_outcome = outcome.clone();
    let result = eframe::run_native(
        ocb_app::TITLE,
        ocb_app::native_options(),
        Box::new(move |cc| {
            let app = if smoke_test {
                App::with_smoke(cc, mode, Smoke::new(app_outcome))
            } else {
                App::new(cc, mode)
            };
            Ok(Box::new(app))
        }),
    );
    exited.store(true, Ordering::SeqCst);
    match result {
        Err(e) => {
            eprintln!("bonooru: {e}");
            ExitCode::from(smoke::EXIT_ERROR)
        }
        Ok(()) if smoke_test => {
            if outcome.get() == smoke::Outcome::Pending {
                println!(
                    "OCB_SMOKE fail (the window closed before frame {})",
                    smoke::FRAMES
                );
            }
            ExitCode::from(smoke::exit_code(outcome.get()))
        }
        Ok(()) => ExitCode::SUCCESS,
    }
}
