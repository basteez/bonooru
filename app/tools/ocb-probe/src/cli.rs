//! Command-line arguments (hand-written, like `ocb-app`'s `cli.rs`).

/// Usage text (`--help` on stdout, a usage error on stderr).
pub const USAGE: &str = "\
Usage: ocb-probe <command> [options]

Dev-only discovery bench tool (Story 4.4): runs the app's serial worker and
log headless and prints machine-checkable lines. Never shipped.

Commands:
  list                         every serial port with its USB ids and class
  run [--step-timeout-s N]     connect, unplug, replug, log check (H.29);
                               each step waits at most N s (default 120)
  run --expect-permission-denied [--step-timeout-s N]
                               Linux: the permission message on a port the
                               user cannot open (H.30; default wait 10 s)
  alone [--seconds N]          the bridge without a controller: HELLO only,
                               for N s (default 10) (H.31)
  --help                       this help

Exit codes: 0 PASS (list: ok), 1 FAIL, 2 usage, 130 interrupted (Ctrl+C).";

/// Default wait per `run` step, s.
pub const STEP_TIMEOUT_S: u64 = 120;
/// Default wait for the permission event, s.
pub const PERMISSION_TIMEOUT_S: u64 = 10;
/// Default length of `alone`, s.
pub const ALONE_SECONDS: u64 = 10;

/// What to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// `--help`.
    Help,
    /// `list`.
    List,
    /// `run`.
    Run {
        /// Wait per step, s.
        step_timeout_s: u64,
    },
    /// `run --expect-permission-denied`.
    Permission {
        /// Wait for the event, s.
        timeout_s: u64,
    },
    /// `alone`.
    Alone {
        /// Length, s.
        seconds: u64,
    },
}

/// A usage error, with the offending argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageError(pub String);

fn number(arg: Option<String>, flag: &str) -> Result<u64, UsageError> {
    let v = arg.ok_or_else(|| UsageError(format!("{flag} needs a number")))?;
    match v.parse::<u64>() {
        Ok(n) if n > 0 && n <= 86_400 => Ok(n),
        _ => Err(UsageError(format!("{flag} {v}"))),
    }
}

/// Parses the arguments after the program name.
///
/// # Errors
/// [`UsageError`] for an unknown command, flag or value.
pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Result<Command, UsageError> {
    let mut it = args.into_iter();
    let Some(cmd) = it.next() else {
        return Err(UsageError("no command".to_owned()));
    };
    match cmd.as_str() {
        "--help" | "-h" | "help" => Ok(Command::Help),
        "list" => match it.next() {
            None => Ok(Command::List),
            Some(a) => Err(UsageError(a)),
        },
        "run" => {
            let mut timeout = None;
            let mut permission = false;
            while let Some(a) = it.next() {
                match a.as_str() {
                    "--step-timeout-s" => timeout = Some(number(it.next(), "--step-timeout-s")?),
                    "--expect-permission-denied" => permission = true,
                    _ => return Err(UsageError(a)),
                }
            }
            Ok(if permission {
                Command::Permission {
                    timeout_s: timeout.unwrap_or(PERMISSION_TIMEOUT_S),
                }
            } else {
                Command::Run {
                    step_timeout_s: timeout.unwrap_or(STEP_TIMEOUT_S),
                }
            })
        }
        "alone" => {
            let mut seconds = ALONE_SECONDS;
            while let Some(a) = it.next() {
                match a.as_str() {
                    "--seconds" => seconds = number(it.next(), "--seconds")?,
                    _ => return Err(UsageError(a)),
                }
            }
            Ok(Command::Alone { seconds })
        }
        _ => Err(UsageError(cmd)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(a: &[&str]) -> Result<Command, UsageError> {
        parse(a.iter().map(|s| (*s).to_owned()))
    }

    #[test]
    fn commands() {
        assert_eq!(p(&["list"]), Ok(Command::List));
        assert_eq!(p(&["--help"]), Ok(Command::Help));
        assert_eq!(
            p(&["run"]),
            Ok(Command::Run {
                step_timeout_s: 120
            })
        );
        assert_eq!(
            p(&["run", "--step-timeout-s", "30"]),
            Ok(Command::Run { step_timeout_s: 30 })
        );
        assert_eq!(
            p(&["run", "--expect-permission-denied"]),
            Ok(Command::Permission { timeout_s: 10 })
        );
        assert_eq!(
            p(&["run", "--step-timeout-s", "5", "--expect-permission-denied"]),
            Ok(Command::Permission { timeout_s: 5 })
        );
        assert_eq!(p(&["alone"]), Ok(Command::Alone { seconds: 10 }));
        assert_eq!(
            p(&["alone", "--seconds", "3"]),
            Ok(Command::Alone { seconds: 3 })
        );
    }

    #[test]
    fn usage_errors() {
        for bad in [
            &[][..],
            &["frobnicate"],
            &["list", "x"],
            &["run", "--step-timeout-s"],
            &["run", "--step-timeout-s", "0"],
            &["run", "--step-timeout-s", "abc"],
            &["alone", "--seconds", "-1"],
            &["alone", "--fast"],
        ] {
            assert!(p(bad).is_err(), "{bad:?}");
        }
    }
}
