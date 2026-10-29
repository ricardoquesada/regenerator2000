//! Manifest validation: cross-references, fixtures and record consistency.
//!
//! Validation never stops at the first problem. It returns **every** error,
//! sorted by subject (file or group) and then by error kind, so one run shows
//! the complete picture.

use std::collections::BTreeSet;

use super::fixtures::is_plain_file_name;
use super::kinds::Field;
use super::values::AddrRange;
use super::{Case, Corpus, CorpusError, FixtureResolver, Unp64Record, Unp64Saved};

impl Corpus {
    /// Checks cross-references, fixtures and record consistency, and returns
    /// **every** problem, sorted by subject (file or group) and then by kind.
    /// An empty list means the manifest is valid.
    ///
    /// A missing fixture is always an error: cases are never skipped.
    #[must_use]
    pub fn validate(&self, fixtures: &dyn FixtureResolver) -> Vec<CorpusError> {
        let mut errors = Vec::new();
        self.validate_groups(fixtures, &mut errors);
        self.validate_cases(fixtures, &mut errors);
        errors.sort_by_cached_key(CorpusError::sort_key);
        errors
    }

    /// Group ids are unique and every original exists.
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

    /// Case files are unique and exist, group references resolve, and each
    /// case's oracle data is consistent.
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

/// Validates the unp64 record and the accepted divergence of one case.
fn validate_case_oracle_data(case: &Case, in_group: bool, errors: &mut Vec<CorpusError>) {
    let file = || case.file.clone();

    let Some(record) = &case.unp64 else {
        // Required even inside a ground-truth group (M1 gate: "every case has
        // a recorded [case.unp64]").
        errors.push(CorpusError::MissingUnp64 { file: file() });
        return;
    };

    if let Unp64Record::Saved(saved) = record {
        validate_saved_record(&case.file, saved, errors);
    }

    if in_group {
        // Ground truth decides for group members; unp64 disagreements are
        // listed automatically, so a reviewed exception would be dead data.
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

    // A failed unp64 run saved nothing, so every field differs.
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

/// Validates the internal consistency of a saved unp64 record: `len` is
/// derived from the range (ruling R1b), and the overlap window lies inside
/// unp64's range (Addendum B1).
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
