//! `--update` mode: records `unp64` results in the manifest and proposes
//! exceptions for new differences.
//!
//! # Rules
//!
//! * Every observed case receives a fresh `[case.unp64]` record (results,
//!   hash, overlap window, and `unp64` version).
//! * Ground-truth group members never receive an exception because the
//!   original binary is the oracle.
//! * The tool keeps an existing exception and its signature when the exception
//!   still describes the case. Specifically, the recorded results must match,
//!   the exception must list the exact differing fields, and its kind must
//!   match the classified kind or be `unexplained` for non-payload
//!   differences. On a case marked `payload_mismatch_under_investigation`, the
//!   tool also keeps an `unexplained` exception for the non-payload fields.
//! * Otherwise, the classifier proposes an **unsigned** exception that the
//!   corpus test rejects until a reviewer signs it. The classifier never
//!   proposes an exception for `PayloadMismatch` or `EntryMismatch`.

use std::fmt;

use crate::corpus::{
    AcceptedDivergence, Case, CaseResult, Corpus, Divergence, DivergenceKind, ExpectedFailReason,
    Field, FieldSet, HexAddr, ReviewState, Unp64Record, UnpackedImage,
};
use crate::markdown::divergence_label;
use crate::observation::Observation;
use crate::record::{build_record, same_results, update_window};

/// Change to the `[case.unp64]` record of a case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordChange {
    /// The case had no previous record.
    Added,
    /// The results match the previous record (the version may differ).
    Unchanged,
    /// The results differ from the previous record.
    Changed,
}

/// Change to the `accepted_divergence` of a case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExceptionChange {
    /// Ground-truth group member: the original binary decides, so the tool
    /// does not manage an exception.
    GroundTruth,
    /// R2000 failed, so the tool leaves any existing exception untouched.
    R2000Failed,
    /// The tools agree and the case has no exception.
    NotNeeded,
    /// The existing exception still describes the case and keeps its
    /// signature.
    Kept {
        /// The kept exception.
        exception: AcceptedDivergence,
    },
    /// A new unsigned proposal replaces any previous exception.
    Proposed {
        /// The proposal stored in the updated case.
        proposal: AcceptedDivergence,
        /// The previous exception, if any.
        previous: Option<AcceptedDivergence>,
    },
    /// The previous exception no longer applies and the tool cannot propose a
    /// replacement.
    Removed {
        /// The removed exception.
        previous: AcceptedDivergence,
    },
    /// The case has no exception, and a human must review the divergence.
    NeedsReview,
}

/// Update result for one case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseUpdate {
    /// The updated case.
    pub case: Case,
    /// How its record changed.
    pub record: RecordChange,
    /// How R2000 and `unp64` disagree.
    pub divergence: Divergence,
    /// How its exception changed.
    pub exception: ExceptionChange,
}

/// Updated corpus and per-case changes, sorted by file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpusUpdate {
    /// The corpus with every observed case updated.
    pub corpus: Corpus,
    /// One entry per observed case.
    pub cases: Vec<CaseUpdate>,
}

/// Updates one observed case with `unp64` `version`.
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

/// Decides the exception for `case` after the update.
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

/// Returns the fields where the `unp64` record and R2000 differ, or `None`
/// when all fields match.
///
/// A failed `unp64` run saves nothing, so every field differs. The `len` field
/// derives from the range and never appears in the set.
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

/// Returns `true` if `existing` still describes the case.
///
/// The exception must list the exact differing fields, and its kind must match
/// the classified kind or be `unexplained`. A reviewer can accept a range or
/// entry difference as `unexplained`. The tool rejects `unexplained` for a
/// payload mismatch unless the case carries
/// `payload_mismatch_under_investigation`, where the exception covers the
/// non-payload fields and the marker masks the payload failure.
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

/// Returns the unsigned exception that the classifier proposes, or `None` when
/// the tools agree or when a human must review the divergence.
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

/// Formats the evidence note for a proposed exception.
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

/// Formats an exception as `kind [fields] (review)`.
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

/// Renders every case update, followed by the cases that require human review.
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

/// Returns `true` if a human must review the case after the update.
fn needs_attention(update: &CaseUpdate) -> bool {
    matches!(
        update.exception,
        ExceptionChange::Proposed { .. }
            | ExceptionChange::Removed { .. }
            | ExceptionChange::NeedsReview
            | ExceptionChange::R2000Failed
    )
}
