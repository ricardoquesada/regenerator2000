//! Tests for command-line parsing and exit codes.

use std::ffi::OsString;

use crate::cli::{Command, Markdown, Mode, ToolStatus, UsageError, parse_args};

/// Parses string arguments.
fn parse(args: &[&str]) -> Result<Command, UsageError> {
    parse_args(args.iter().map(OsString::from))
}

#[test]
fn modes_and_markdown_combine() {
    let run = |mode, markdown| Ok(Command::Run { mode, markdown });
    assert_eq!(parse(&["--update"]), run(Mode::Update, Markdown::Skip));
    assert_eq!(parse(&["--check"]), run(Mode::Check, Markdown::Skip));
    assert_eq!(parse(&["--markdown"]), run(Mode::Report, Markdown::Print));
    assert_eq!(
        parse(&["--markdown", "--check"]),
        run(Mode::Check, Markdown::Print)
    );
    assert_eq!(
        parse(&["--update", "--markdown"]),
        run(Mode::Update, Markdown::Print)
    );
    assert_eq!(parse(&["--update", "--help"]), Ok(Command::Help));
}

#[test]
fn usage_errors_are_rejected() {
    assert_eq!(parse(&[]), Err(UsageError::NoMode));
    assert_eq!(
        parse(&["--update", "--check"]),
        Err(UsageError::ConflictingModes)
    );
    assert_eq!(
        parse(&["--updat"]),
        Err(UsageError::UnknownArgument("--updat".to_string()))
    );
}

#[test]
fn exit_codes_are_distinct() {
    assert_eq!(ToolStatus::Success.code(), 0);
    assert_eq!(ToolStatus::Drift.code(), 1);
    assert_eq!(ToolStatus::Failure.code(), 2);
}
