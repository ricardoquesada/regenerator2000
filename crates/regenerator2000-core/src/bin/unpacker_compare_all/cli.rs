//! Command-line parsing and exit statuses for `unpacker_compare_all`.

use std::ffi::OsString;
use std::fmt;
use std::process::ExitCode;

/// Usage text for `--help` and usage errors.
pub const USAGE: &str = "\
usage: unpacker_compare_all (--update | --check | --markdown) [--markdown]

Runs Regenerator 2000 and unp64 on every case of tests/6502/unpack_corpus.toml.

  --update    record unp64's results in the manifest and propose unsigned
              accepted_divergence entries for new differences
  --check     exit 1 if the recorded unp64 results no longer match the binary
  --markdown  print a comparison report (alone, or after --update/--check)
  --help      print this text

unp64 is taken from $UNP64, else from PATH, else from ~/.local/bin/unp64.
Exit codes: 0 success, 1 drift (--check), 2 usage or tool error.";

/// Action that the tool runs on the observations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Rewrite the `[case.unp64]` tables in the manifest.
    Update,
    /// Report drift between the manifest and the binary.
    Check,
    /// Print only the report.
    Report,
}

/// Whether the tool prints the markdown report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Markdown {
    /// Print the report.
    Print,
    /// Do not print the report.
    Skip,
}

/// Parsed command-line request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Print the usage text.
    Help,
    /// Run the comparison.
    Run {
        /// The execution mode.
        mode: Mode,
        /// Whether to print the markdown report.
        markdown: Markdown,
    },
}

/// Invalid command-line arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UsageError {
    /// Unrecognized argument.
    UnknownArgument(String),
    /// Both `--update` and `--check` appear in the arguments.
    ConflictingModes,
    /// No mode argument appears.
    NoMode,
}

impl fmt::Display for UsageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownArgument(argument) => write!(f, "unknown argument `{argument}`"),
            Self::ConflictingModes => f.write_str("--update and --check cannot be combined"),
            Self::NoMode => f.write_str("give --update, --check or --markdown"),
        }
    }
}

impl std::error::Error for UsageError {}

/// Parses the command-line arguments without the program name.
///
/// # Example
///
/// `["--check", "--markdown"]` returns `Run { mode: Check, markdown: Print }`.
/// `["--markdown"]` alone returns `Run { mode: Report, markdown: Print }`.
///
/// # Errors
///
/// Returns [`UsageError`] for unknown arguments, conflicting modes, or missing
/// modes.
pub fn parse_args(args: impl IntoIterator<Item = OsString>) -> Result<Command, UsageError> {
    let mut update = false;
    let mut check = false;
    let mut markdown = false;
    for argument in args {
        match argument.to_str() {
            Some("--help" | "-h") => return Ok(Command::Help),
            Some("--update") => update = true,
            Some("--check") => check = true,
            Some("--markdown") => markdown = true,
            _ => {
                return Err(UsageError::UnknownArgument(
                    argument.to_string_lossy().into_owned(),
                ));
            }
        }
    }
    let mode = match (update, check, markdown) {
        (true, true, _) => return Err(UsageError::ConflictingModes),
        (true, false, _) => Mode::Update,
        (false, true, _) => Mode::Check,
        (false, false, true) => Mode::Report,
        (false, false, false) => return Err(UsageError::NoMode),
    };
    let markdown = if markdown {
        Markdown::Print
    } else {
        Markdown::Skip
    };
    Ok(Command::Run { mode, markdown })
}

/// Final outcome of a tool run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolStatus {
    /// The run completed without drift.
    Success,
    /// `--check` found recorded results that `unp64` no longer reproduces.
    Drift,
    /// A usage error or runtime error occurred.
    Failure,
}

impl ToolStatus {
    /// Returns the process exit code: 0, 1, or 2.
    #[must_use]
    pub fn code(self) -> u8 {
        match self {
            Self::Success => 0,
            Self::Drift => 1,
            Self::Failure => 2,
        }
    }
}

impl From<ToolStatus> for ExitCode {
    fn from(status: ToolStatus) -> Self {
        ExitCode::from(status.code())
    }
}
