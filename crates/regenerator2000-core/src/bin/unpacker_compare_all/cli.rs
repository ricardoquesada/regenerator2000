//! Command-line parsing and exit statuses of `unpacker_compare_all`.

use std::ffi::OsString;
use std::fmt;
use std::process::ExitCode;

/// Usage text, printed for `--help` and after a usage error.
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

/// What the tool does with the observations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Rewrite the manifest's `[case.unp64]` tables.
    Update,
    /// Report drift between the manifest and the binary.
    Check,
    /// Only print the report.
    Report,
}

/// Whether the markdown report is printed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Markdown {
    /// Print it.
    Print,
    /// Do not print it.
    Skip,
}

/// What the user asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Print the usage text.
    Help,
    /// Run the comparison.
    Run {
        /// The mode.
        mode: Mode,
        /// Whether to print the markdown report.
        markdown: Markdown,
    },
}

/// A command line that cannot be run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UsageError {
    /// An argument the tool does not know.
    UnknownArgument(String),
    /// `--update` and `--check` were both given.
    ConflictingModes,
    /// No mode was given.
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

/// Parses the arguments (without the program name).
///
/// # Example
///
/// `["--check", "--markdown"]` gives `Run { mode: Check, markdown: Print }`;
/// `["--markdown"]` alone gives `Run { mode: Report, markdown: Print }`.
///
/// # Errors
///
/// Returns a [`UsageError`] for unknown arguments, both `--update` and
/// `--check`, or no mode at all.
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

/// How a run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolStatus {
    /// Everything requested was done; no drift.
    Success,
    /// `--check` found recorded results that unp64 no longer reproduces.
    Drift,
    /// A usage error, or the tool could not do its job (unp64 missing, an
    /// unreadable manifest, an unrecordable unp64 result, ...).
    Failure,
}

impl ToolStatus {
    /// The process exit code: 0, 1 or 2.
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
