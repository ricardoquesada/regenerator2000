//! `unpacker_compare_all`: records unp64's results as the reference for the
//! unpacker corpus, and compares them with Regenerator 2000 (R2000).
//!
//! The case list lives in `tests/6502/unpack_corpus.toml`. For every case
//! the tool runs R2000 (with the case's own instruction budget) and unp64
//! (`<fixture> -o <unique temporary file>`), classifies how they disagree,
//! and then, depending on the mode:
//!
//! * `--update` rewrites the `[case.unp64]` tables and proposes unsigned
//!   `accepted_divergence` entries for new differences;
//! * `--check` exits non-zero if a recorded result no longer matches the
//!   binary;
//! * `--markdown` prints a comparison report.
//!
//! The corpus test (`tests/unpack_corpus.rs`) reads the recorded results, so
//! CI never needs unp64.

mod check;
mod cli;
mod corpus;
mod markdown;
mod observation;
mod record;
mod unp64;
mod update;

#[cfg(test)]
mod tests;

use std::env;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use cli::{Command, Markdown, Mode, ToolStatus, USAGE};
use corpus::{Corpus, CorpusError, CorpusFilter, DirFixtures, Schedule, run_cases};
use observation::Observation;
use unp64::{Unp64, Unp64Error};

/// Environment variable that holds the user's home directory, used for the
/// `~/.local/bin/unp64` candidate.
const HOME_ENV: &str = "HOME";

fn main() -> ExitCode {
    let command = match cli::parse_args(env::args_os().skip(1)) {
        Ok(command) => command,
        Err(error) => {
            eprintln!("unpacker_compare_all: {error}\n\n{USAGE}");
            return ToolStatus::Failure.into();
        }
    };
    let Command::Run { mode, markdown } = command else {
        println!("{USAGE}");
        return ToolStatus::Success.into();
    };
    match run(mode, markdown) {
        Ok(status) => status.into(),
        Err(error) => {
            eprintln!("unpacker_compare_all: {error}");
            ToolStatus::Failure.into()
        }
    }
}

/// Why a run could not complete.
#[derive(Debug)]
enum ToolError {
    /// The manifest is unreadable or its case list is broken.
    Corpus(Vec<CorpusError>),
    /// unp64 is missing or produced an unrecordable result.
    Unp64(Unp64Error),
    /// The updated manifest could not be written.
    Write {
        /// The manifest path.
        path: PathBuf,
        /// The operating-system error.
        message: String,
    },
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Corpus(errors) => {
                let lines: Vec<String> = errors.iter().map(|e| format!("- {e}")).collect();
                write!(f, "the corpus manifest is unusable:\n{}", lines.join("\n"))
            }
            Self::Unp64(error) => error.fmt(f),
            Self::Write { path, message } => {
                write!(f, "cannot write {}: {message}", path.display())
            }
        }
    }
}

impl From<Unp64Error> for ToolError {
    fn from(error: Unp64Error) -> Self {
        Self::Unp64(error)
    }
}

impl From<CorpusError> for ToolError {
    fn from(error: CorpusError) -> Self {
        Self::Corpus(vec![error])
    }
}

/// Loads the manifest, observes every case, and performs `mode`.
fn run(mode: Mode, markdown: Markdown) -> Result<ToolStatus, ToolError> {
    let fixture_dir = Path::new(corpus::FIXTURE_DIR);
    let manifest_path = fixture_dir.join(corpus::MANIFEST_FILE);
    let corpus = Corpus::load(&manifest_path)?;
    let fixtures = DirFixtures::new(fixture_dir);

    // Missing or stale oracle data is what `--update` repairs, so only a
    // broken case list stops the tool.
    let structural: Vec<CorpusError> = corpus
        .validate(&fixtures)
        .into_iter()
        .filter(CorpusError::is_structural)
        .collect();
    if !structural.is_empty() {
        return Err(ToolError::Corpus(structural));
    }

    let unp64 = Unp64::locate(&unp64::binary_candidates(
        env::var_os(unp64::UNP64_ENV),
        env::var_os(HOME_ENV),
    ))?;
    eprintln!(
        "using {} (unp64 v{})",
        unp64.program().display(),
        unp64.version()
    );

    let mut observations = observe(&corpus, &fixtures, fixture_dir, &unp64)?;
    let status = match mode {
        Mode::Update => {
            let updated = update::update_corpus(&corpus, &observations, unp64.version());
            write_manifest(&manifest_path, &updated.corpus)?;
            println!("{}", update::render_update(&updated));
            report_remaining_errors(&updated.corpus, &fixtures);
            // The report must describe the manifest as written.
            for observation in &mut observations {
                if let Some(case) = updated.corpus.case(&observation.case.file) {
                    observation.case = case.clone();
                }
            }
            ToolStatus::Success
        }
        Mode::Check => {
            let report = check::check_all(&observations, unp64.version());
            println!("{report}");
            report.status()
        }
        Mode::Report => ToolStatus::Success,
    };
    if markdown == Markdown::Print {
        println!(
            "\n{}",
            markdown::render_markdown(&observations, unp64.version())
        );
    }
    Ok(status)
}

/// Runs R2000 (in parallel) and unp64 (one case at a time, each with its
/// own temporary output file) on every case, sorted by file.
fn observe(
    corpus: &Corpus,
    fixtures: &DirFixtures,
    fixture_dir: &Path,
    unp64: &Unp64,
) -> Result<Vec<Observation>, ToolError> {
    let cases = CorpusFilter::All.select(corpus)?;
    let results = run_cases(&cases, fixtures, Schedule::Parallel).map_err(ToolError::Corpus)?;
    let tmp_dir = env::temp_dir();
    let process_id = std::process::id();
    cases
        .iter()
        .zip(results)
        .enumerate()
        .map(|(index, (case, result))| {
            let output = unp64::tmp_output_path(&tmp_dir, process_id, index);
            let outcome = unp64.run(&fixture_dir.join(&case.file), &output)?;
            Ok(Observation {
                case: (*case).clone(),
                result,
                unp64: outcome,
            })
        })
        .collect()
}

/// Writes `corpus` to `path` in its canonical form.
fn write_manifest(path: &Path, corpus: &Corpus) -> Result<(), ToolError> {
    let text = corpus.to_toml_string()?;
    fs::write(path, text).map_err(|error| ToolError::Write {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}

/// Prints the validation errors the updated manifest still has (for example
/// a failed unp64 run without an exception), which the corpus test reports
/// too until someone resolves them.
fn report_remaining_errors(corpus: &Corpus, fixtures: &DirFixtures) {
    let errors = corpus.validate(fixtures);
    if errors.is_empty() {
        return;
    }
    println!(
        "\n{} manifest error(s) remain; the corpus test fails until they are resolved:",
        errors.len()
    );
    for error in errors {
        println!("- {error}");
    }
}
