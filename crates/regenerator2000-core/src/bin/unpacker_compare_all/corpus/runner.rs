//! Runs R2000 on corpus cases.
//!
//! Both the compare tool and the corpus integration test call this module so
//! every case runs with the same configuration: the case instruction budget
//! (`max_instructions`) and the default target system. [`FixtureResolver`]
//! supplies the fixture bytes so this module does not touch the file system.

use std::panic;
use std::thread;

use regenerator2000_core::state::types::default_system;
use regenerator2000_core::unpacker::{UnpackConfig, unpack};

use super::{Case, CaseResult, CorpusError, FixtureResolver, PRG_HEADER_LEN};

/// Execution schedule for [`run_cases`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Schedule {
    /// Runs one case after another on the calling thread.
    Serial,
    /// Spawns one scoped thread per case (`std::thread::scope`).
    ///
    /// The unpacker engine has no shared mutable state, so parallel and serial
    /// runs produce identical results.
    Parallel,
}

/// Builds the [`UnpackConfig`] for `case` with its effective instruction
/// budget and default system settings.
///
/// # Example
///
/// A case with `max_instructions = 350000000` runs with a budget of 350M. A
/// case without `max_instructions` uses [`super::DEFAULT_MAX_INSTRUCTIONS`].
#[must_use]
pub fn unpack_config_for(case: &Case) -> UnpackConfig {
    UnpackConfig {
        max_instructions: case.effective_max_instructions(),
        ..UnpackConfig::default()
    }
}

/// Unpacks the fixture of `case` with R2000.
///
/// An unpack error is stored inside the returned [`CaseResult`] so the oracle
/// can evaluate it. Only an unreadable or truncated fixture returns `Err`.
///
/// # Errors
///
/// Returns [`CorpusError::MissingFixture`] or [`CorpusError::Io`] if the
/// fixture cannot be read, or [`CorpusError::InvalidFixture`] if the file is
/// shorter than the PRG header.
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
/// Returns all fixture errors sorted by [`CorpusError::sort_key`] if any
/// fixture fails to load.
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
                // Propagate any panic from a worker thread without converting
                // it into a normal unpack result.
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
