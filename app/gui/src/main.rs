//! `bonooru`: the Bonooru desktop app.
//!
//! Exit codes: 0 ok, 1 eframe error (or a smoke run whose window closed
//! before its report), 2 usage, 3 smoke watchdog.

use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ocb_app::app::App;
use ocb_app::cli::{self, Command};
use ocb_app::smoke::{self, Smoke};

fn main() -> ExitCode {
    let (mode, smoke_test) = match cli::parse(std::env::args().skip(1)) {
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

    let reported = Arc::new(AtomicBool::new(false));
    if smoke_test {
        smoke::spawn_watchdog(Arc::clone(&reported));
    }
    let app_reported = Arc::clone(&reported);
    let result = eframe::run_native(
        ocb_app::TITLE,
        ocb_app::native_options(),
        Box::new(move |cc| {
            let app = if smoke_test {
                App::with_smoke(cc, mode, Smoke::new(app_reported))
            } else {
                App::new(cc, mode)
            };
            Ok(Box::new(app))
        }),
    );
    match result {
        Err(e) => {
            eprintln!("bonooru: {e}");
            ExitCode::from(smoke::EXIT_ERROR)
        }
        Ok(()) if smoke_test && !reported.load(Ordering::SeqCst) => {
            println!(
                "OCB_SMOKE fail: the window closed before frame {}",
                smoke::FRAMES
            );
            ExitCode::from(smoke::EXIT_ERROR)
        }
        Ok(()) => ExitCode::SUCCESS,
    }
}
