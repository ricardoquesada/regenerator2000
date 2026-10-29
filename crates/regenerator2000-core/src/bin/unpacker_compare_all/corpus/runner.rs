//! Runs R2000 on corpus cases.
//!
//! Shared by the compare tool and the corpus integration test, so both run
//! every case with exactly the same configuration: the case's own
//! instruction budget (`max_instructions`) and the engine's default target
//! system. Fixtures come through a [`FixtureResolver`], so this module never
//! touches the file system itself.

use std::panic;
use std::thread;

use regenerator2000_core::state::types::default_system;
use regenerator2000_core::unpacker::{UnpackConfig, unpack};

use super::{Case, CaseResult, CorpusError, FixtureResolver};

/// Length of the little-endian load address that starts every PRG.
const PRG_HEADER_LEN: usize = 2;

/// How [`run_cases`] schedules the cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Schedule {
    /// One case after the other, on the calling thread.
    Serial,
    /// One scoped thread per case (`std::thread::scope`).
    ///
    /// The engine has no global state, so parallel runs produce the same
    /// results as serial ones; the corpus test asserts this.
    Parallel,
}

/// The unpacker configuration for `case`: its own instruction budget and
/// otherwise the defaults (the target system stays unset, so the engine
/// uses its default system exactly as a plain `unpack` call does).
///
/// # Example
///
/// A case with `max_instructions = 350000000` runs with a budget of 350 M;
/// a case without it runs with [`super::DEFAULT_MAX_INSTRUCTIONS`].
#[must_use]
pub fn unpack_config_for(case: &Case) -> UnpackConfig {
    UnpackConfig {
        max_instructions: case.effective_max_instructions(),
        ..UnpackConfig::default()
    }
}

/// Unpacks the fixture of `case` with R2000.
///
/// An unpack error is part of the returned [`CaseResult`] (the oracles
/// decide whether it is expected); only an unreadable fixture is an error.
///
/// # Errors
///
/// Returns the resolver's error ([`CorpusError::MissingFixture`] or
/// [`CorpusError::Io`]) if the fixture cannot be read, or
/// [`CorpusError::InvalidFixture`] if it is shorter than a PRG header.
pub fn run_case(case: &Case, fixtures: &dyn FixtureResolver) -> Result<CaseResult, CorpusError> {
    let prg = fixtures.read(&case.file)?;
    let Some((header, payload)) = prg.split_first_chunk::<PRG_HEADER_LEN>() else {
        return Err(CorpusError::InvalidFixture {
            file: case.file.clone(),
            reason: format!("shorter than the {PRG_HEADER_LEN}-byte PRG header"),
        });
    };
    let load_addr = u16::from_le_bytes(*header);
    let result = unpack(payload, load_addr, &unpack_config_for(case), None);
    Ok(CaseResult::from_unpack(
        &case.file,
        load_addr,
        default_system(),
        &result,
    ))
}

/// Unpacks every case and returns the results in the order of `cases`.
///
/// # Errors
///
/// Returns every fixture error (sorted like [`super::Corpus::validate`]'s
/// output) if any case could not be run. Validating the corpus first makes
/// this unreachable for missing fixtures.
pub fn run_cases(
    cases: &[&Case],
    fixtures: &(dyn FixtureResolver + Sync),
    schedule: Schedule,
) -> Result<Vec<CaseResult>, Vec<CorpusError>> {
    let outcomes: Vec<Result<CaseResult, CorpusError>> = match schedule {
        Schedule::Serial => cases.iter().map(|case| run_case(case, fixtures)).collect(),
        Schedule::Parallel => thread::scope(|scope| {
            let handles: Vec<_> = cases
                .iter()
                .map(|case| scope.spawn(move || run_case(case, fixtures)))
                .collect();
            handles
                .into_iter()
                // A panicking case must not be reported as a result: it is an
                // engine bug (ledger contingency rule). Re-raising the
                // original payload keeps its message and location intact.
                .map(|handle| handle.join().unwrap_or_else(|p| panic::resume_unwind(p)))
                .collect()
        }),
    };

    let mut results = Vec::with_capacity(outcomes.len());
    let mut errors = Vec::new();
    for outcome in outcomes {
        match outcome {
            Ok(result) => results.push(result),
            Err(error) => errors.push(error),
        }
    }
    if !errors.is_empty() {
        errors.sort_by_cached_key(CorpusError::sort_key);
        return Err(errors);
    }
    Ok(results)
}
