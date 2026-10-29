//! `--check`: re-run unp64 and report where the recorded `[case.unp64]`
//! results no longer match the binary (for example after an unp64 upgrade).
//!
//! The overlap is re-hashed over the **recorded** window, so drift means
//! "unp64 changed", never "R2000 changed" (the corpus test covers R2000).

use std::fmt;

use crate::cli::ToolStatus;
use crate::corpus::{Case, Unp64Record};
use crate::observation::Observation;
use crate::record::{build_record, recorded_window};
use crate::unp64::Unp64Outcome;

/// Placeholder for a value that is absent on one side.
const ABSENT: &str = "-";

/// A recorded value that `--check` compares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DriftField {
    /// The case has no `[case.unp64]` at all.
    Record,
    /// Saved versus failed.
    Outcome,
    /// First saved address.
    Start,
    /// Last saved address.
    End,
    /// The reported entry.
    Entry,
    /// Saved payload length.
    Len,
    /// Whole-payload hash.
    PayloadFnv64,
    /// The overlap window and its hash.
    Overlap,
    /// The failure reason of a failed run.
    Reason,
    /// The unp64 version.
    Version,
}

impl fmt::Display for DriftField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Record => "record",
            Self::Outcome => "outcome",
            Self::Start => "start",
            Self::End => "end",
            Self::Entry => "entry",
            Self::Len => "len",
            Self::PayloadFnv64 => "payload_fnv64",
            Self::Overlap => "overlap",
            Self::Reason => "failed",
            Self::Version => "version",
        })
    }
}

/// One recorded value that the fresh unp64 run no longer reproduces.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Drift {
    /// The case.
    pub file: String,
    /// The value.
    pub field: DriftField,
    /// The recorded value.
    pub recorded: String,
    /// The value of the fresh run.
    pub fresh: String,
}

impl fmt::Display for Drift {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} recorded {}, unp64 now gives {}",
            self.file, self.field, self.recorded, self.fresh
        )
    }
}

/// Every drift of a `--check` run, sorted by file and field.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckReport {
    /// The drifts.
    pub drifts: Vec<Drift>,
    /// Number of cases checked.
    pub checked: usize,
}

impl CheckReport {
    /// [`ToolStatus::Drift`] if anything drifted, else
    /// [`ToolStatus::Success`].
    #[must_use]
    pub fn status(&self) -> ToolStatus {
        if self.drifts.is_empty() {
            ToolStatus::Success
        } else {
            ToolStatus::Drift
        }
    }
}

impl fmt::Display for CheckReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.drifts.is_empty() {
            return write!(
                f,
                "check: all {} recorded unp64 result(s) are reproduced",
                self.checked
            );
        }
        writeln!(
            f,
            "check: {} drift(s) in {} case(s); re-run with --update and review the diff:",
            self.drifts.len(),
            self.checked
        )?;
        let lines: Vec<String> = self.drifts.iter().map(|d| format!("- {d}")).collect();
        f.write_str(&lines.join("\n"))
    }
}

/// Compares `case`'s recorded unp64 result with a fresh `outcome` produced by
/// unp64 `version`, field by field.
#[must_use]
pub fn check_case(case: &Case, outcome: &Unp64Outcome, version: &str) -> Vec<Drift> {
    let drift = |field, recorded: String, fresh: String| Drift {
        file: case.file.clone(),
        field,
        recorded,
        fresh,
    };
    let fresh = build_record(outcome, version, recorded_window(case.unp64.as_ref()));
    let Some(recorded) = &case.unp64 else {
        return vec![drift(
            DriftField::Record,
            ABSENT.to_string(),
            summary(&fresh),
        )];
    };
    if std::mem::discriminant(recorded) != std::mem::discriminant(&fresh) {
        return vec![drift(
            DriftField::Outcome,
            summary(recorded),
            summary(&fresh),
        )];
    }
    tagged_values(recorded)
        .into_iter()
        .zip(tagged_values(&fresh))
        .filter(|((_, recorded), (_, fresh))| recorded != fresh)
        .map(|((field, recorded), (_, fresh))| drift(field, recorded, fresh))
        .collect()
}

/// Checks every observation.
#[must_use]
pub fn check_all(observations: &[Observation], version: &str) -> CheckReport {
    let mut drifts: Vec<Drift> = observations
        .iter()
        .flat_map(|observation| check_case(&observation.case, &observation.unp64, version))
        .collect();
    drifts.sort();
    CheckReport {
        drifts,
        checked: observations.len(),
    }
}

/// Every value of a record with its field tag, in a fixed order per shape.
fn tagged_values(record: &Unp64Record) -> Vec<(DriftField, String)> {
    match record {
        Unp64Record::Saved(saved) => vec![
            (DriftField::Start, saved.start.to_string()),
            (DriftField::End, saved.end.to_string()),
            (DriftField::Entry, saved.entry.to_string()),
            (DriftField::Len, saved.len.to_string()),
            (DriftField::PayloadFnv64, saved.payload_fnv64.to_string()),
            (
                DriftField::Overlap,
                saved.overlap.as_ref().map_or_else(
                    || ABSENT.to_string(),
                    |overlap| format!("{}-{} {}", overlap.start, overlap.end, overlap.fnv64),
                ),
            ),
            (DriftField::Version, saved.version.clone()),
        ],
        Unp64Record::Failed(failed) => vec![
            (DriftField::Reason, failed.reason.clone()),
            (DriftField::Version, failed.version.clone()),
        ],
    }
}

/// A one-line summary of a record, for whole-record drifts.
fn summary(record: &Unp64Record) -> String {
    match record {
        Unp64Record::Saved(saved) => format!("saved {}-{}", saved.start, saved.end),
        Unp64Record::Failed(failed) => format!("failed ({})", failed.reason),
    }
}
