//! Runs the `unp64` binary: locates the executable, probes its version, and
//! runs it on one fixture with a unique temporary output file.
//!
//! `parse` holds pure functions that parse `unp64` console output and saved
//! files. This module adds process execution, file cleanup, and pure helpers
//! for binary candidates, command-line arguments, and temporary paths.

pub mod parse;

use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub use self::parse::Unp64Outcome;
use self::parse::{ParseError, interpret_run, parse_console, parse_version_banner};

/// Environment variable that sets the `unp64` binary path explicitly.
pub const UNP64_ENV: &str = "UNP64";

/// Program name on `PATH`.
const PROGRAM_NAME: &str = "unp64";

/// User-local binary directory relative to the home directory.
const LOCAL_BIN_DIR: [&str; 2] = [".local", "bin"];

/// `unp64` option that sets the output file name (`-o Name`).
///
/// Without this option, `unp64` writes `<input>.XXXX` next to the input file.
const OUTPUT_FLAG: &str = "-o";

/// Prefix for every temporary output file.
const TMP_PREFIX: &str = "unpacker_compare_all";

// =============================================================================
// Errors
// =============================================================================

/// Reason why `unp64` failed to run or produced an unrecordable result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unp64Error {
    /// No candidate binary answered the version probe.
    NotFound {
        /// Each candidate path and its rejection reason.
        tried: Vec<(PathBuf, String)>,
    },
    /// The operating system failed to start the binary.
    Spawn {
        /// The binary path.
        program: PathBuf,
        /// The operating-system error.
        message: String,
    },
    /// The no-argument call printed no usage banner.
    NoBanner {
        /// The binary path.
        program: PathBuf,
        /// Process exit code, or `None` if a signal terminated the process.
        exit_code: Option<i32>,
    },
    /// File I/O error on a temporary output file.
    Io {
        /// The file path.
        path: PathBuf,
        /// The operating-system error.
        message: String,
    },
    /// `unp64` produced an inconsistent or unrecordable result for `input`.
    Output {
        /// The input fixture path.
        input: PathBuf,
        /// The parse error.
        source: ParseError,
    },
}

impl fmt::Display for Unp64Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { tried } if tried.is_empty() => write!(
                f,
                "unp64 not found: no candidate binary (set {UNP64_ENV}=/path/to/unp64)"
            ),
            Self::NotFound { tried } => {
                let details = tried
                    .iter()
                    .map(|(program, reason)| format!("{}: {reason}", program.display()))
                    .collect::<Vec<_>>()
                    .join("; ");
                write!(
                    f,
                    "unp64 not found (tried {details}); install it or set \
                     {UNP64_ENV}=/path/to/unp64"
                )
            }
            Self::Spawn { program, message } => {
                write!(f, "cannot run {}: {message}", program.display())
            }
            Self::NoBanner { program, exit_code } => {
                let code = exit_code.map_or_else(|| "none".to_string(), |code| code.to_string());
                write!(
                    f,
                    "{} printed no `UNP64 vX.YY` usage banner (exit code {code}); is it unp64?",
                    program.display()
                )
            }
            Self::Io { path, message } => write!(f, "{}: {message}", path.display()),
            Self::Output { input, source } => write!(f, "{}: {source}", input.display()),
        }
    }
}

impl std::error::Error for Unp64Error {}

// =============================================================================
// Pure invocation helpers
// =============================================================================

/// Returns the candidate `unp64` binary paths in search order.
///
/// A non-empty `UNP64` environment variable is the only candidate so an
/// invalid explicit path fails immediately. Otherwise, this function returns
/// `unp64` on `PATH`, followed by `~/.local/bin/unp64` when `home` is non-empty.
///
/// # Example
///
/// With `UNP64` unset and `HOME=/home/a`, this function returns `["unp64",
/// "/home/a/.local/bin/unp64"]`.
#[must_use]
pub fn binary_candidates(unp64_env: Option<OsString>, home: Option<OsString>) -> Vec<PathBuf> {
    if let Some(explicit) = unp64_env.filter(|value| !value.is_empty()) {
        return vec![PathBuf::from(explicit)];
    }
    let mut candidates = vec![PathBuf::from(PROGRAM_NAME)];
    if let Some(home) = home.filter(|value| !value.is_empty()) {
        let mut local = PathBuf::from(home);
        local.extend(LOCAL_BIN_DIR);
        local.push(PROGRAM_NAME);
        candidates.push(local);
    }
    candidates
}

/// Returns the command-line arguments for one run: `<input> -o <output>`.
#[must_use]
pub fn unp64_args(input: &Path, output: &Path) -> Vec<OsString> {
    vec![
        input.as_os_str().to_owned(),
        OsString::from(OUTPUT_FLAG),
        output.as_os_str().to_owned(),
    ]
}

/// Returns the temporary output path for case `index` of `process_id` inside
/// `dir`.
///
/// The process ID separates concurrent tool runs, and `index` separates cases.
/// The tool never writes files next to a fixture.
///
/// # Example
///
/// `tmp_output_path("/tmp", 42, 7)` returns
/// `/tmp/unpacker_compare_all-42-007.prg`.
#[must_use]
pub fn tmp_output_path(dir: &Path, process_id: u32, index: usize) -> PathBuf {
    dir.join(format!("{TMP_PREFIX}-{process_id}-{index:03}.prg"))
}

/// Output from the no-argument version probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeOutput {
    /// Process exit code, or `None` if a signal terminated the process.
    pub exit_code: Option<i32>,
    /// Combined stdout and stderr text.
    pub text: String,
}

/// Interprets the no-argument version probe of `program`.
///
/// `unp64` prints its usage banner and exits non-zero when called without
/// arguments. Only a spawn error or a missing usage banner fails the probe.
///
/// # Errors
///
/// Returns [`Unp64Error::Spawn`] if the probe failed to start, or
/// [`Unp64Error::NoBanner`] if the output contains no usage banner.
pub fn interpret_version_probe(
    program: &Path,
    probe: io::Result<ProbeOutput>,
) -> Result<String, Unp64Error> {
    let output = probe.map_err(|error| Unp64Error::Spawn {
        program: program.to_path_buf(),
        message: error.to_string(),
    })?;
    parse_version_banner(&output.text).map_err(|_| Unp64Error::NoBanner {
        program: program.to_path_buf(),
        exit_code: output.exit_code,
    })
}

// =============================================================================
// Process
// =============================================================================

/// Located `unp64` binary and its version string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unp64 {
    program: PathBuf,
    version: String,
}

impl Unp64 {
    /// Returns the first candidate binary that answers the version probe.
    ///
    /// # Errors
    ///
    /// Returns [`Unp64Error::NotFound`] with the rejection reason for every
    /// candidate if none answers.
    pub fn locate(candidates: &[PathBuf]) -> Result<Self, Unp64Error> {
        let mut tried = Vec::new();
        for program in candidates {
            match interpret_version_probe(program, probe(program)) {
                Ok(version) => {
                    return Ok(Self {
                        program: program.clone(),
                        version,
                    });
                }
                Err(error) => tried.push((program.clone(), error.to_string())),
            }
        }
        Err(Unp64Error::NotFound { tried })
    }

    /// Returns the binary path that answered the probe.
    #[must_use]
    pub fn program(&self) -> &Path {
        &self.program
    }

    /// Returns the version from the usage banner, such as `"2.42"`.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Runs `unp64` on `input`, writes to `output`, and interprets the result.
    ///
    /// This method deletes `output` before and after the run on every exit
    /// path.
    ///
    /// # Errors
    ///
    /// Returns [`Unp64Error::Spawn`] if `unp64` fails to start,
    /// [`Unp64Error::Io`] if the output file cannot be read or deleted, or
    /// [`Unp64Error::Output`] if the result is inconsistent.
    pub fn run(&self, input: &Path, output: &Path) -> Result<Unp64Outcome, Unp64Error> {
        let _cleanup = TempOutput::claim(output)?;
        let process = Command::new(&self.program)
            .args(unp64_args(input, output))
            .stdin(Stdio::null())
            .output()
            .map_err(|error| Unp64Error::Spawn {
                program: self.program.clone(),
                message: error.to_string(),
            })?;
        let console = parse_console(&combined_text(&process.stdout, &process.stderr));
        let file = match fs::read(output) {
            Ok(bytes) => Some(bytes),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => {
                return Err(Unp64Error::Io {
                    path: output.to_path_buf(),
                    message: error.to_string(),
                });
            }
        };
        interpret_run(&console, file.as_deref()).map_err(|source| Unp64Error::Output {
            input: input.to_path_buf(),
            source,
        })
    }
}

/// Runs the no-argument version probe on `program`.
fn probe(program: &Path) -> io::Result<ProbeOutput> {
    let output = Command::new(program).stdin(Stdio::null()).output()?;
    Ok(ProbeOutput {
        exit_code: output.status.code(),
        text: combined_text(&output.stdout, &output.stderr),
    })
}

/// Concatenates `stdout` and `stderr` as UTF-8 text.
fn combined_text(stdout: &[u8], stderr: &[u8]) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(stdout),
        String::from_utf8_lossy(stderr)
    )
}

/// Guard for a temporary output path that deletes any existing file on
/// creation and deletes the output file on drop.
struct TempOutput<'a> {
    path: &'a Path,
}

impl<'a> TempOutput<'a> {
    /// Deletes any stale file at `path` and claims the path.
    fn claim(path: &'a Path) -> Result<Self, Unp64Error> {
        remove_if_present(path).map_err(|error| Unp64Error::Io {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
        Ok(Self { path })
    }
}

impl Drop for TempOutput<'_> {
    fn drop(&mut self) {
        // Print a warning if the temporary file cannot be deleted on drop.
        if let Err(error) = remove_if_present(self.path) {
            eprintln!(
                "warning: cannot remove temporary file {}: {error}",
                self.path.display()
            );
        }
    }
}

/// Deletes `path` if the file exists.
fn remove_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}
