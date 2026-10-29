//! Running the unp64 binary: locating it, probing its version, and running
//! it on one fixture with a unique temporary output file.
//!
//! The parsing of what unp64 prints and saves lives in `parse`, as pure
//! functions. This module adds the process and file handling around it, plus
//! the pure helpers that decide *how* unp64 is invoked (candidates, command
//! line, temporary path), so those decisions are unit tested too.

pub mod parse;

use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub use self::parse::Unp64Outcome;
use self::parse::{ParseError, interpret_run, parse_console, parse_version_banner};

/// Environment variable that names the unp64 binary explicitly.
pub const UNP64_ENV: &str = "UNP64";

/// Program name looked up on `PATH`.
const PROGRAM_NAME: &str = "unp64";

/// Location of the user-local install, relative to the home directory.
const LOCAL_BIN_DIR: [&str; 2] = [".local", "bin"];

/// unp64's option that forces the output file name (`-o Name`). Without it,
/// unp64 writes `<input>.XXXX` next to the input, which once overwrote a
/// tracked fixture.
const OUTPUT_FLAG: &str = "-o";

/// Prefix of every temporary output file.
const TMP_PREFIX: &str = "unpacker_compare_all";

// =============================================================================
// Errors
// =============================================================================

/// Why unp64 could not be used, or produced an unrecordable result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unp64Error {
    /// No candidate binary answered the version probe.
    NotFound {
        /// Each candidate tried, with the reason it was rejected.
        tried: Vec<(PathBuf, String)>,
    },
    /// The binary could not be started.
    Spawn {
        /// The binary.
        program: PathBuf,
        /// The operating-system error.
        message: String,
    },
    /// The no-argument call printed no usage banner.
    NoBanner {
        /// The binary.
        program: PathBuf,
        /// Its exit code (`None` if it was killed by a signal).
        exit_code: Option<i32>,
    },
    /// A temporary output file could not be read or removed.
    Io {
        /// The file.
        path: PathBuf,
        /// The operating-system error.
        message: String,
    },
    /// unp64's result for `input` cannot be recorded faithfully.
    Output {
        /// The fixture unp64 ran on.
        input: PathBuf,
        /// What is wrong with the result.
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

/// The binaries to try, in order.
///
/// An explicit, non-empty `UNP64` is the only candidate (a wrong path must
/// fail loudly, not fall back). Otherwise `unp64` on `PATH`, then
/// `~/.local/bin/unp64` when a home directory is known. There are no
/// hardcoded user paths.
///
/// # Example
///
/// With `UNP64` unset and `HOME=/home/a`: `["unp64",
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

/// The command-line arguments for one run: `<input> -o <output>`.
#[must_use]
pub fn unp64_args(input: &Path, output: &Path) -> Vec<OsString> {
    vec![
        input.as_os_str().to_owned(),
        OsString::from(OUTPUT_FLAG),
        output.as_os_str().to_owned(),
    ]
}

/// The temporary output path for case number `index` of the process
/// `process_id`, inside `dir` (normally `std::env::temp_dir()`).
///
/// The process id keeps concurrent tool runs apart; the index keeps cases
/// apart. Nothing is ever written next to a fixture.
///
/// # Example
///
/// `tmp_output_path("/tmp", 42, 7)` is `/tmp/unpacker_compare_all-42-007.prg`.
#[must_use]
pub fn tmp_output_path(dir: &Path, process_id: u32, index: usize) -> PathBuf {
    dir.join(format!("{TMP_PREFIX}-{process_id}-{index:03}.prg"))
}

/// What the no-argument version probe printed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeOutput {
    /// The exit code (`None` if the process was killed by a signal). Only
    /// reported in errors: unp64 exits non-zero here by design.
    pub exit_code: Option<i32>,
    /// stdout followed by stderr.
    pub text: String,
}

/// Interprets the no-argument version probe of `program` (ruling R2).
///
/// unp64 prints its usage banner and exits non-zero when called without
/// arguments, so a non-zero exit is **not** a tool failure: only the banner
/// decides. A spawn error (for example, binary not found) is.
///
/// # Errors
///
/// Returns [`Unp64Error::Spawn`] if the probe could not start, or
/// [`Unp64Error::NoBanner`] if its output has no usage banner.
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

/// A located unp64 binary and its version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unp64 {
    program: PathBuf,
    version: String,
}

impl Unp64 {
    /// Returns the first candidate that answers the version probe.
    ///
    /// # Errors
    ///
    /// Returns [`Unp64Error::NotFound`] with every candidate's rejection
    /// reason if none answers.
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

    /// The binary that answered the probe.
    #[must_use]
    pub fn program(&self) -> &Path {
        &self.program
    }

    /// The version from the usage banner, e.g. `"2.42"`.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Runs unp64 on `input`, writing to `output` (a path from
    /// [`tmp_output_path`]), and interprets the result.
    ///
    /// `output` is removed before the run (a stale file must not pass for a
    /// result) and after it, on every path including errors.
    ///
    /// # Errors
    ///
    /// Returns [`Unp64Error::Spawn`] if unp64 cannot start,
    /// [`Unp64Error::Io`] if the output file cannot be read or removed, or
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

/// Runs the no-argument version probe.
fn probe(program: &Path) -> io::Result<ProbeOutput> {
    let output = Command::new(program).stdin(Stdio::null()).output()?;
    Ok(ProbeOutput {
        exit_code: output.status.code(),
        text: combined_text(&output.stdout, &output.stderr),
    })
}

/// stdout followed by stderr, as text. unp64 writes diagnostics such as
/// "unimplemented opcode" through its logger, so both streams matter.
fn combined_text(stdout: &[u8], stderr: &[u8]) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(stdout),
        String::from_utf8_lossy(stderr)
    )
}

/// Owns a temporary output path: removes any stale file on creation and the
/// file on drop, so no exit path leaves it behind.
struct TempOutput<'a> {
    path: &'a Path,
}

impl<'a> TempOutput<'a> {
    /// Removes a stale file at `path` and takes ownership of the path.
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
        // `Drop` cannot return an error; a leftover temporary file is
        // harmless but must not go unnoticed.
        if let Err(error) = remove_if_present(self.path) {
            eprintln!(
                "warning: cannot remove temporary file {}: {error}",
                self.path.display()
            );
        }
    }
}

/// Removes `path`; a file that does not exist is not an error.
fn remove_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}
