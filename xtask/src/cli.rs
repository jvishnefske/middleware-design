//! Command-line parsing for `cargo xtask`, as a pure function over the
//! argument list.
//!
//! Accepted forms:
//!
//! - `cargo xtask trace` regenerates `docs/traceability.md` and prints
//!   problems;
//! - `cargo xtask trace --check` verifies traceability and that the report is
//!   current, writing nothing;
//! - `cargo xtask help`, `--help` or `-h` prints usage.
//!
//! Any other subcommand or flag is a [`UsageError`]. Subcommands are the
//! closed [`Command`] enum, so adding one (such as the planned `coverage`) is
//! a new variant plus a match arm.

use std::fmt;

/// Usage text printed for `help` and after usage errors.
pub const USAGE: &str = "\
usage: cargo xtask <command>

commands:
  trace           regenerate docs/traceability.md and list traceability problems
  trace --check   fail if any traceability problem exists or the report is stale
  help            show this message";

/// What `trace` should do with the report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceMode {
    /// Rewrite `docs/traceability.md`.
    Write,
    /// Compare against `docs/traceability.md` without writing.
    Check,
}

/// A parsed subcommand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// `trace [--check]`.
    Trace(TraceMode),
    /// `help`.
    Help,
}

/// A rejected command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageError(pub String);

impl fmt::Display for UsageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}\n\n{USAGE}", self.0)
    }
}

/// Parses the arguments after the program name.
pub fn parse<S: AsRef<str>>(args: &[S]) -> Result<Command, UsageError> {
    let mut iter = args.iter().map(AsRef::as_ref);
    let command = match iter.next() {
        None => return Err(UsageError("missing command".to_owned())),
        Some("help" | "--help" | "-h") => Command::Help,
        Some("trace") => parse_trace(&mut iter)?,
        Some(other) => return Err(UsageError(format!("unknown command `{other}`"))),
    };
    match iter.next() {
        None => Ok(command),
        Some(extra) => Err(UsageError(format!("unexpected argument `{extra}`"))),
    }
}

/// Parses `trace` flags, consuming at most one argument.
fn parse_trace<'a>(iter: &mut impl Iterator<Item = &'a str>) -> Result<Command, UsageError> {
    match iter.next() {
        None => Ok(Command::Trace(TraceMode::Write)),
        Some("--check") => Ok(Command::Trace(TraceMode::Check)),
        Some(other) => Err(UsageError(format!(
            "unknown argument `{other}` for `trace`"
        ))),
    }
}

#[cfg(test)]
// Test code: panicking on a failed expectation is the point of a test.
#[cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::panic
    )
)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_commands() {
        assert_eq!(parse(&["trace"]), Ok(Command::Trace(TraceMode::Write)));
        assert_eq!(
            parse(&["trace", "--check"]),
            Ok(Command::Trace(TraceMode::Check))
        );
        assert_eq!(parse(&["help"]), Ok(Command::Help));
        assert_eq!(parse(&["-h"]), Ok(Command::Help));
    }

    #[test]
    fn rejects_unknown_and_extra_arguments() {
        let empty: [&str; 0] = [];
        assert!(parse(&empty).is_err());
        assert!(parse(&["bogus"]).is_err());
        assert!(parse(&["coverage"]).is_err());
        assert!(parse(&["trace", "--chek"]).is_err());
        assert!(parse(&["trace", "--check", "x"]).is_err());
        assert!(parse(&["help", "x"]).is_err());
    }

    #[test]
    fn error_message_names_the_argument_and_shows_usage() {
        let msg = parse(&["bogus"]).unwrap_err().to_string();
        assert!(msg.contains("`bogus`"));
        assert!(msg.contains("usage: cargo xtask"));
    }
}
