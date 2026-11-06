//! Manifest checks: cross-references, fixtures, and record consistency.
//!
//! Validation does not stop at the first problem. It returns **every** error,
//! in subject (file or group) and error-kind order, so one run reports the
//! full set of issues.

use std::collections::BTreeSet;

use super::fixtures::is_plain_file_name;
use super::kinds::Field;
use super::values::AddrRange;
use super::{Case, Corpus, CorpusError, FixtureResolver, Unp64Record, Unp64Saved};

impl Corpus {
    /// Checks cross-references, fixtures, and record consistency, and returns
    /// **every** problem in subject (file or group) and error-kind order.
    /// An empty list means a valid manifest.
    ///
    /// A missing fixture always produces an error.
    #[must_use]
    pub fn validate(&self, fixtures: &dyn FixtureResolver) -> Vec<CorpusError> {
        let mut errors = Vec::new();
        self.validate_groups(fixtures, &mut errors);
        self.validate_cases(fixtures, &mut errors);
        errors.sort_by_cached_key(CorpusError::sort_key);
        errors
    }

    /// Checks that group ids are unique and that every original file exists.
    fn validate_groups(&self, fixtures: &dyn FixtureResolver, errors: &mut Vec<CorpusError>) {
        let mut seen = BTreeSet::new();
        for group in &self.groups {
            if !seen.insert(group.id.as_str()) {
                errors.push(CorpusError::DuplicateGroup {
                    group: group.id.clone(),
                });
            }
            if !is_plain_file_name(&group.original) {
                errors.push(CorpusError::InvalidFileName {
                    file: group.original.clone(),
                });
            } else if !fixtures.exists(&group.original) {
                errors.push(CorpusError::MissingGroupOriginal {
                    group: group.id.clone(),
                    file: group.original.clone(),
                });
            }
        }
    }

    /// Checks that case files are unique and exist, group references resolve,
    /// and each case has consistent oracle data.
    fn validate_cases(&self, fixtures: &dyn FixtureResolver, errors: &mut Vec<CorpusError>) {
        let mut seen = BTreeSet::new();
        for case in &self.cases {
            if !seen.insert(case.file.as_str()) {
                errors.push(CorpusError::DuplicateCase {
                    file: case.file.clone(),
                });
            }
            if !is_plain_file_name(&case.file) {
                errors.push(CorpusError::InvalidFileName {
                    file: case.file.clone(),
                });
            } else if !fixtures.exists(&case.file) {
                errors.push(CorpusError::MissingFixture {
                    file: case.file.clone(),
                });
            }
            let in_group = match case.group.as_deref() {
                Some(id) if self.group(id).is_none() => {
                    errors.push(CorpusError::UnknownGroup {
                        file: case.file.clone(),
                        group: id.to_string(),
                    });
                    false
                }
                Some(_) => true,
                None => false,
            };
            validate_case_oracle_data(case, in_group, errors);
        }
    }
}

/// Checks the `unp64` record and the accepted divergence of one case.
fn validate_case_oracle_data(case: &Case, in_group: bool, errors: &mut Vec<CorpusError>) {
    let file = || case.file.clone();

    let Some(record) = &case.unp64 else {
        // Every case requires a `[case.unp64]` reference, even inside a
        // ground-truth group.
        errors.push(CorpusError::MissingUnp64 { file: file() });
        return;
    };

    if let Unp64Record::Saved(saved) = record {
        validate_saved_record(&case.file, saved, errors);
    }

    if in_group {
        // Ground truth decides for group members and reports `unp64`
        // disagreements automatically, so a reviewed exception is unused.
        if case.accepted_divergence.is_some() {
            errors.push(CorpusError::DivergenceOnGroundTruthCase {
                file: file(),
                group: case.group.clone().unwrap_or_default(),
            });
        }
        return;
    }

    let divergence = case.accepted_divergence.as_ref();
    let unp64_failed = matches!(record, Unp64Record::Failed(_));

    if unp64_failed && !divergence.is_some_and(|d| d.kind.allowed_for_failed_unp64()) {
        errors.push(CorpusError::FailedUnp64WithoutDivergence { file: file() });
    }

    let Some(divergence) = divergence else {
        return;
    };

    if divergence.kind.requires_failed_unp64() && !unp64_failed {
        errors.push(CorpusError::DivergenceKindNeedsFailedUnp64 {
            file: file(),
            kind: divergence.kind,
        });
    }

    // A failed `unp64` run saves no output, so every field differs.
    let required = if unp64_failed {
        Some(&Field::ALL[..])
    } else {
        divergence.kind.required_fields()
    };
    if let Some(required) = required
        && divergence.fields.as_slice() != required
    {
        errors.push(CorpusError::DivergenceFieldsMismatch {
            file: file(),
            kind: divergence.kind,
            required,
        });
    }
}

/// Checks that `len` matches the range length and that the overlap window lies
/// inside the `unp64` range.
fn validate_saved_record(file: &str, saved: &Unp64Saved, errors: &mut Vec<CorpusError>) {
    let range = saved.range();
    let derived = range.map(AddrRange::len);
    if derived != usize::try_from(saved.len).ok() {
        errors.push(CorpusError::Unp64LenMismatch {
            file: file.to_string(),
            recorded: saved.len,
            derived,
        });
    }

    let Some(overlap) = &saved.overlap else {
        return;
    };
    let inside = match (overlap.range(), range) {
        (Some(window), Some(range)) => range.contains_range(window),
        _ => false,
    };
    if !inside {
        errors.push(CorpusError::OverlapOutsideUnp64Range {
            file: file.to_string(),
            overlap: (overlap.start, overlap.end),
            unp64: (saved.start, saved.end),
        });
    }
}
