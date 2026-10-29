//! `--update`: record unp64's results in the manifest and propose
//! exceptions for new differences.
//!
//! # Rules
//!
//! * Every observed case gets a fresh `[case.unp64]` record (results, hash,
//!   overlap window, unp64 version).
//! * Ground-truth group members never get an exception: the original
//!   decides, and validation rejects one.
//! * An existing exception is **kept with its signature** while it still
//!   describes the case: the recorded results are unchanged (the version may
//!   differ), it lists exactly the differing fields, and its kind is the
//!   classified kind, or `unexplained` for anything but a payload mismatch.
//!   On a case marked `payload_mismatch_under_investigation` (ruling R17) an
//!   `unexplained` exception for the non-payload fields is kept too. This is
//!   how a reviewer's decision survives re-runs.
//! * Otherwise the classifier proposes an **unsigned** exception, which the
//!   corpus test rejects until someone signs it. `PayloadMismatch` and
//!   `EntryMismatch` are never proposed.

use std::fmt;

use crate::corpus::{
    AcceptedDivergence, Case, CaseResult, Corpus, Divergence, DivergenceKind, ExpectedFailReason,
    Field, FieldSet, HexAddr, ReviewState, Unp64Record, UnpackedImage,
};
use crate::markdown::divergence_label;
use crate::observation::Observation;
use crate::record::{build_record, same_results, update_window};

/// How a case's `[case.unp64]` record changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordChange {
    /// The case had no record.
    Added,
    /// Same results as before (the version may differ).
    Unchanged,
    /// The results differ from the previous record.
    Changed,
}

/// What happened to a case's `accepted_divergence`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExceptionChange {
    /// A ground-truth group member: the original decides, so the tool
    /// manages no exception for it.
    GroundTruth,
    /// R2000 failed, so there is nothing to judge the exception against; it
    /// is left exactly as it was.
    R2000Failed,
    /// The tools agree and the case has no exception.
    NotNeeded,
    /// The existing exception still describes the case and keeps its
    /// signature.
    Kept {
        /// The kept exception.
        exception: AcceptedDivergence,
    },
    /// A new unsigned proposal replaces whatever was there.
    Proposed {
        /// The proposal (also stored in the updated case).
        proposal: AcceptedDivergence,
        /// The exception it replaces, if any.
        previous: Option<AcceptedDivergence>,
    },
    /// The previous exception no longer applies, and nothing can be proposed
    /// (the tools agree now, or the divergence must be reviewed by hand).
    Removed {
        /// The removed exception.
        previous: AcceptedDivergence,
    },
    /// No exception, and the divergence must never be accepted
    /// automatically: a human must investigate.
    NeedsReview,
}

/// The update of one case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseUpdate {
    /// The updated case.
    pub case: Case,
    /// How its record changed.
    pub record: RecordChange,
    /// How R2000 and unp64 disagree.
    pub divergence: Divergence,
    /// What happened to its exception.
    pub exception: ExceptionChange,
}

/// The updated corpus and what changed per case (sorted by file).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpusUpdate {
    /// The corpus with every observed case updated; groups and unobserved
    /// cases are unchanged.
    pub corpus: Corpus,
    /// One entry per observed case.
    pub cases: Vec<CaseUpdate>,
}

/// Updates one observed case with unp64 `version`.
#[must_use]
pub fn update_case(observation: &Observation, version: &str) -> CaseUpdate {
    let Observation {
        case,
        result,
        unp64,
    } = observation;
    let record = build_record(unp64, version, update_window(unp64, result));
    let record_change = match &case.unp64 {
        None => RecordChange::Added,
        Some(previous) if same_results(previous, &record) => RecordChange::Unchanged,
        Some(_) => RecordChange::Changed,
    };
    let divergence = observation.divergence();
    let (accepted_divergence, exception) = decide_exception(
        case,
        result,
        &record,
        record_change == RecordChange::Unchanged,
        &divergence,
    );
    CaseUpdate {
        case: Case {
            unp64: Some(record),
            accepted_divergence,
            ..case.clone()
        },
        record: record_change,
        divergence,
        exception,
    }
}

/// Updates every observed case of `corpus`.
#[must_use]
pub fn update_corpus(corpus: &Corpus, observations: &[Observation], version: &str) -> CorpusUpdate {
    let mut cases: Vec<CaseUpdate> = observations
        .iter()
        .map(|observation| update_case(observation, version))
        .collect();
    cases.sort_by(|a, b| a.case.file.cmp(&b.case.file));

    let mut updated = corpus.clone();
    for case in &mut updated.cases {
        if let Some(update) = cases.iter().find(|update| update.case.file == case.file) {
            *case = update.case.clone();
        }
    }
    CorpusUpdate {
        corpus: updated,
        cases,
    }
}

/// Decides the case's exception after the update (see the module rules).
fn decide_exception(
    case: &Case,
    result: &CaseResult,
    record: &Unp64Record,
    record_unchanged: bool,
    divergence: &Divergence,
) -> (Option<AcceptedDivergence>, ExceptionChange) {
    let previous = case.accepted_divergence.clone();
    if case.group.is_some() {
        return (previous, ExceptionChange::GroundTruth);
    }
    let Ok(image) = &result.outcome else {
        return (previous, ExceptionChange::R2000Failed);
    };
    let differing = differing_fields(record, image);

    if let Some(existing) = &previous
        && record_unchanged
        && still_describes(existing, divergence, differing.as_ref(), case.expected_fail)
    {
        return (
            previous.clone(),
            ExceptionChange::Kept {
                exception: existing.clone(),
            },
        );
    }

    let proposal = propose(divergence, differing, record, image);
    match (previous, proposal) {
        (None, None) if *divergence == Divergence::Agree => (None, ExceptionChange::NotNeeded),
        (None, None) => (None, ExceptionChange::NeedsReview),
        (Some(previous), None) => (None, ExceptionChange::Removed { previous }),
        (previous, Some(proposal)) => (
            Some(proposal.clone()),
            ExceptionChange::Proposed { proposal, previous },
        ),
    }
}

/// The fields on which unp64's record and R2000 differ, or `None` if they
/// agree on all of them. A failed unp64 run saved nothing, so every field
/// differs. `len` is derived from the range and never listed (ruling R1b).
fn differing_fields(record: &Unp64Record, image: &UnpackedImage) -> Option<FieldSet> {
    let fields: Vec<Field> = match record {
        Unp64Record::Failed(_) => Field::ALL.to_vec(),
        Unp64Record::Saved(saved) => Field::ALL
            .into_iter()
            .filter(|field| match field {
                Field::Start => saved.start.0 != image.start,
                Field::End => saved.end.0 != image.end,
                Field::Entry => !saved.entry.matches(image.entry),
            })
            .collect(),
    };
    FieldSet::new(fields).ok()
}

/// Returns `true` if `existing` still describes the case: it lists exactly
/// the differing fields, and its kind is the classified kind or
/// `unexplained`.
///
/// A reviewer may always accept a range or entry difference as
/// `unexplained`. A payload mismatch is never accepted, except on a case
/// marked `payload_mismatch_under_investigation` (ruling R17): there the
/// exception covers only the non-payload fields, and the marker masks the
/// payload failure.
fn still_describes(
    existing: &AcceptedDivergence,
    divergence: &Divergence,
    differing: Option<&FieldSet>,
    expected_fail: Option<ExpectedFailReason>,
) -> bool {
    if differing != Some(&existing.fields) {
        return false;
    }
    if divergence.accepted_kind() == Some(existing.kind) {
        return true;
    }
    let payload_mismatch = matches!(divergence, Divergence::PayloadMismatch { .. });
    let payload_under_investigation =
        expected_fail == Some(ExpectedFailReason::PayloadMismatchUnderInvestigation);
    existing.kind == DivergenceKind::Unexplained
        && (!payload_mismatch || payload_under_investigation)
}

/// The unsigned exception the classifier proposes, or `None` if the
/// divergence is agreement or must never be accepted automatically.
fn propose(
    divergence: &Divergence,
    differing: Option<FieldSet>,
    record: &Unp64Record,
    image: &UnpackedImage,
) -> Option<AcceptedDivergence> {
    let kind = divergence.accepted_kind()?;
    let fields = match kind.required_fields() {
        Some(required) => FieldSet::new(required.iter().copied()).ok()?,
        None => differing?,
    };
    Some(AcceptedDivergence {
        kind,
        fields,
        reviewed_by: None,
        note: Some(proposal_note(record, image)),
    })
}

/// The evidence a reviewer needs, written into the proposal's `note`.
///
/// # Example
///
/// `proposed by unpacker_compare_all --update: unp64 $0801-$FFFD entry
/// $0911; R2000 $0801-$7949 entry $0911`.
fn proposal_note(record: &Unp64Record, image: &UnpackedImage) -> String {
    let unp64 = match record {
        Unp64Record::Saved(saved) => {
            format!("unp64 {}-{} entry {}", saved.start, saved.end, saved.entry)
        }
        Unp64Record::Failed(failed) => format!("unp64 failed ({})", failed.reason),
    };
    format!(
        "proposed by unpacker_compare_all --update: {unp64}; R2000 {}-{} entry {}",
        HexAddr(image.start),
        HexAddr(image.end),
        HexAddr(image.entry)
    )
}

// =============================================================================
// Rendering
// =============================================================================

impl fmt::Display for RecordChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Added => "recorded",
            Self::Unchanged => "unchanged",
            Self::Changed => "CHANGED",
        })
    }
}

/// Describes an exception as `kind [fields] (review)`.
fn describe_exception(exception: &AcceptedDivergence) -> String {
    let review = match exception.review_state() {
        ReviewState::Unsigned => "unsigned".to_string(),
        ReviewState::PendingUser | ReviewState::Signed => format!(
            "reviewed_by {}",
            exception.reviewed_by.as_deref().unwrap_or_default()
        ),
    };
    format!("{} [{}] ({review})", exception.kind, exception.fields)
}

impl fmt::Display for CaseUpdate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let exception = match &self.exception {
            ExceptionChange::GroundTruth => "ground truth decides".to_string(),
            ExceptionChange::R2000Failed => "R2000 failed; exception left untouched".to_string(),
            ExceptionChange::NotNeeded => "no exception needed".to_string(),
            ExceptionChange::Kept { exception } => {
                format!("kept {}", describe_exception(exception))
            }
            ExceptionChange::Proposed { proposal, previous } => {
                let replaced = previous
                    .as_ref()
                    .map(|old| format!(", replacing {}", describe_exception(old)))
                    .unwrap_or_default();
                format!(
                    "PROPOSED {}{replaced}: review and sign it",
                    describe_exception(proposal)
                )
            }
            ExceptionChange::Removed { previous } => {
                format!("REMOVED {}", describe_exception(previous))
            }
            ExceptionChange::NeedsReview => {
                "NEEDS REVIEW: this divergence is never accepted automatically".to_string()
            }
        };
        write!(
            f,
            "{}: record {}; {}; {exception}",
            self.case.file,
            self.record,
            divergence_label(&self.divergence)
        )
    }
}

/// Renders every case update, one line each, followed by the cases that
/// need a human decision.
#[must_use]
pub fn render_update(update: &CorpusUpdate) -> String {
    let mut lines: Vec<String> = update.cases.iter().map(ToString::to_string).collect();
    let attention: Vec<&str> = update
        .cases
        .iter()
        .filter(|case| needs_attention(case))
        .map(|case| case.case.file.as_str())
        .collect();
    lines.push(String::new());
    if attention.is_empty() {
        lines.push("No case needs a review decision.".to_string());
    } else {
        lines.push(format!(
            "{} case(s) need a review decision: {}",
            attention.len(),
            attention.join(", ")
        ));
    }
    lines.join("\n")
}

/// Returns `true` if a human must act on the case: an unsigned proposal, a
/// removed exception, a divergence that is never accepted automatically, or
/// an R2000 failure.
fn needs_attention(update: &CaseUpdate) -> bool {
    matches!(
        update.exception,
        ExceptionChange::Proposed { .. }
            | ExceptionChange::Removed { .. }
            | ExceptionChange::NeedsReview
            | ExceptionChange::R2000Failed
    )
}
