//! Command-line arguments (hand-written, no clap).

use crate::app::Mode;

/// Usage text, printed by `--help` (stdout) and on a usage error (stderr).
pub const USAGE: &str = "\
Usage: bonooru [--gallery] [--smoke-test] [--help]

  (no args)     open the main screen
  --gallery     open the token gallery instead (design review)
  --smoke-test  render a few frames, print one OCB_SMOKE line and exit
  --help        print this help";

/// What the command line asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Help,
    Run { mode: Mode, smoke: bool },
}

/// An argument the app does not know.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsageError(pub String);

/// Parses the arguments after the program name.
///
/// # Errors
/// [`UsageError`] naming the first unknown argument.
pub fn parse<I, S>(args: I) -> Result<Command, UsageError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut mode = Mode::Main;
    let mut smoke = false;
    let mut help = false;
    for arg in args {
        match arg.as_ref() {
            "--gallery" => mode = Mode::Gallery,
            "--smoke-test" => smoke = true,
            "--help" | "-h" => help = true,
            other => return Err(UsageError(other.to_owned())),
        }
    }
    Ok(if help {
        Command::Help
    } else {
        Command::Run { mode, smoke }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Result<Command, UsageError> {
        parse(args.iter().copied())
    }

    #[test]
    fn no_args_is_the_main_screen() {
        assert_eq!(
            p(&[]),
            Ok(Command::Run {
                mode: Mode::Main,
                smoke: false
            })
        );
    }

    #[test]
    fn gallery_and_smoke_combine_in_any_order() {
        let want = Ok(Command::Run {
            mode: Mode::Gallery,
            smoke: true,
        });
        assert_eq!(p(&["--gallery", "--smoke-test"]), want);
        assert_eq!(p(&["--smoke-test", "--gallery"]), want);
        assert_eq!(
            p(&["--smoke-test"]),
            Ok(Command::Run {
                mode: Mode::Main,
                smoke: true
            })
        );
    }

    #[test]
    fn help_wins() {
        assert_eq!(p(&["--help"]), Ok(Command::Help));
        assert_eq!(p(&["--gallery", "-h"]), Ok(Command::Help));
    }

    #[test]
    fn unknown_argument_is_a_usage_error() {
        assert_eq!(p(&["--galery"]), Err(UsageError("--galery".to_owned())));
        assert_eq!(p(&["--gallery", "x"]), Err(UsageError("x".to_owned())));
    }
}
