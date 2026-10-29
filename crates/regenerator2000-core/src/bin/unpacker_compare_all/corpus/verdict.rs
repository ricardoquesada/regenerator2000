//! Evaluation outcomes: what failed and why, and what a passing case's report
//! must show.

use std::fmt;

use super::kinds::{DivergenceKind, ExpectedFailReason, Field, join_fields, padding_byte_holds};
use super::values::{AddrRange, Fnv64, HexAddr};
use super::{CaseResult, INITIAL_REVIEW_MARKER, R2000Error, UnpackedImage};

// =============================================================================
// Subjects and consensus properties
// =============================================================================

/// What a failure or report row is about.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subject {
    /// One case, by file name.
    Case(String),
    /// A whole group, by id (consensus checks).
    Group(String),
}

impl fmt::Display for Subject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Case(file) => f.write_str(file),
            Self::Group(id) => write!(f, "group `{id}`"),
        }
    }
}

/// A group-level value compared by the consensus check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConsensusProperty {
    /// The unpacked range.
    Range,
    /// The entry point.
    Entry,
    /// The payload hash.
    Payload,
}

impl ConsensusProperty {
    /// Every property, in report order.
    pub const ALL: [ConsensusProperty; 3] = [Self::Range, Self::Entry, Self::Payload];

    /// The value of this property for `image`, in its display form.
    ///
    /// # Example
    ///
    /// For an image over `$0800-$9D19` with entry `$1100`, `Range` gives
    /// `"$0800-$9D19"` and `Entry` gives `"$1100"`.
    #[must_use]
    pub fn value_of(self, image: &UnpackedImage) -> String {
        match self {
            Self::Range => format!("{}-{}", HexAddr(image.start), HexAddr(image.end)),
            Self::Entry => HexAddr(image.entry).to_string(),
            Self::Payload => image.fnv64().to_string(),
        }
    }
}

impl fmt::Display for ConsensusProperty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Range => "range",
            Self::Entry => "entry",
            Self::Payload => "payload",
        })
    }
}

// =============================================================================
// Failures
// =============================================================================

/// Why a case (or group) fails its oracle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailureReason {
    /// The result names a file that is not in the manifest.
    UnknownCase,
    /// R2000 returned an error and the case is not `expected_fail`.
    R2000Failed {
        /// The error.
        error: R2000Error,
    },
    /// R2000's payload length does not match its range.
    InconsistentResult {
        /// Reported start.
        start: HexAddr,
        /// Reported end.
        end: HexAddr,
        /// Payload length.
        data_len: usize,
    },
    /// The case has no usable `[case.unp64]`.
    MissingUnp64,
    /// The `[case.unp64]` record is internally inconsistent.
    InvalidUnp64Record,
    /// unp64 failed and the case has no exception that accepts it.
    Unp64FailedUnaccepted {
        /// unp64's failure reason.
        reason: String,
    },
    /// A field differs from unp64 and is not permitted to.
    FieldMismatch {
        /// The field.
        field: Field,
        /// unp64's value (`$XXXX` or `RUN`).
        unp64: String,
        /// R2000's value.
        r2000: HexAddr,
    },
    /// The ranges match, but the payload hashes differ.
    PayloadHashMismatch {
        /// unp64's hash.
        unp64: Fnv64,
        /// R2000's hash.
        r2000: Fnv64,
    },
    /// The exception has no (or a blank) `reviewed_by`.
    UnsignedDivergence,
    /// The exception's `kind` predicate is false for the current result.
    KindNoLongerHolds {
        /// The kind.
        kind: DivergenceKind,
        /// Which part of the predicate failed.
        detail: String,
    },
    /// Listed fields now equal unp64 (ruling R13).
    StaleDivergence {
        /// The listed fields that no longer differ.
        stale: Vec<Field>,
        /// `true` if no listed field differs any more.
        all_listed: bool,
    },
    /// The ranges differ but no overlap window was recorded.
    MissingOverlap,
    /// The overlap window is not inside R2000's range.
    OverlapOutsideResult {
        /// The recorded window.
        window: AddrRange,
        /// R2000's range.
        r2000: AddrRange,
    },
    /// The overlap window is not the current intersection of both ranges.
    OverlapWindowMismatch {
        /// The recorded window.
        window: AddrRange,
        /// The intersection of R2000's and unp64's ranges.
        intersection: Option<AddrRange>,
    },
    /// R2000's bytes over the overlap window differ from unp64's.
    OverlapHashMismatch {
        /// The window.
        window: AddrRange,
        /// unp64's hash.
        unp64: Fnv64,
        /// R2000's hash.
        r2000: Fnv64,
    },
    /// A value differs from the group's original.
    GroundTruthMismatch {
        /// The group.
        group: String,
        /// The field.
        field: Field,
        /// The original's value.
        expected: HexAddr,
        /// R2000's value.
        actual: HexAddr,
    },
    /// Payload bytes differ from the group's original.
    GroundTruthPayloadMismatch {
        /// The group.
        group: String,
        /// Address of the first differing byte.
        first: HexAddr,
        /// Number of differing bytes over the compared window.
        count: usize,
    },
    /// Group members disagree.
    ConsensusMismatch {
        /// The compared property.
        property: ConsensusProperty,
        /// Each member and its value.
        values: Vec<(String, String)>,
    },
    /// An `expected_fail` case passed.
    UnexpectedPass {
        /// The marker's reason.
        reason: ExpectedFailReason,
    },
}

impl FailureReason {
    /// Stable rank of the reason kind, used as the secondary sort key.
    fn rank(&self) -> u8 {
        match self {
            Self::UnknownCase => 0,
            Self::R2000Failed { .. } => 1,
            Self::InconsistentResult { .. } => 2,
            Self::MissingUnp64 => 3,
            Self::InvalidUnp64Record => 4,
            Self::Unp64FailedUnaccepted { .. } => 5,
            Self::FieldMismatch { .. } => 6,
            Self::PayloadHashMismatch { .. } => 7,
            Self::UnsignedDivergence => 8,
            Self::KindNoLongerHolds { .. } => 9,
            Self::StaleDivergence { .. } => 10,
            Self::MissingOverlap => 11,
            Self::OverlapOutsideResult { .. } => 12,
            Self::OverlapWindowMismatch { .. } => 13,
            Self::OverlapHashMismatch { .. } => 14,
            Self::GroundTruthMismatch { .. } => 15,
            Self::GroundTruthPayloadMismatch { .. } => 16,
            Self::ConsensusMismatch { .. } => 17,
            Self::UnexpectedPass { .. } => 18,
        }
    }

    /// Which `expected_fail` reason could ever mask this failure.
    ///
    /// The match is exhaustive with no wildcard, so a new failure variant
    /// must be classified explicitly; the default is [`Maskable::Never`].
    fn maskable(&self) -> Maskable {
        match self {
            Self::GroundTruthMismatch {
                field: Field::Start,
                expected,
                actual,
                ..
            } => Maskable::GroundTruthStart {
                expected: expected.0,
                actual: actual.0,
            },
            Self::PayloadHashMismatch { .. } | Self::OverlapHashMismatch { .. } => {
                Maskable::PayloadHash
            }
            Self::UnknownCase
            | Self::R2000Failed { .. }
            | Self::InconsistentResult { .. }
            | Self::MissingUnp64
            | Self::InvalidUnp64Record
            | Self::Unp64FailedUnaccepted { .. }
            | Self::FieldMismatch { .. }
            | Self::UnsignedDivergence
            | Self::KindNoLongerHolds { .. }
            | Self::StaleDivergence { .. }
            | Self::MissingOverlap
            | Self::OverlapOutsideResult { .. }
            | Self::OverlapWindowMismatch { .. }
            | Self::GroundTruthMismatch { .. }
            | Self::GroundTruthPayloadMismatch { .. }
            | Self::ConsensusMismatch { .. }
            | Self::UnexpectedPass { .. } => Maskable::Never,
        }
    }
}

/// The classes of failures an `expected_fail` reason may mask.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Maskable {
    /// A ground-truth start mismatch (Dali's padding convention, if the two
    /// starts are the BASIC link byte pair).
    GroundTruthStart {
        /// The original's start.
        expected: u16,
        /// R2000's start.
        actual: u16,
    },
    /// A payload or overlap hash mismatch against unp64.
    PayloadHash,
    /// Nothing may mask it.
    Never,
}

impl ExpectedFailReason {
    /// Returns `true` if this reason masks `failure` of `result`'s case.
    ///
    /// Each reason has a closed allow-list (ruling R17); there is no
    /// "mask everything" path:
    ///
    /// * `padding_convention` masks only a ground-truth `start` mismatch
    ///   whose two starts are the BASIC link byte pair (the R16 predicate),
    ///   e.g. Dali's `$0801` against the original `$0800`.
    /// * `payload_mismatch_under_investigation` masks only payload and
    ///   overlap hash mismatches against unp64.
    ///
    /// Everything else (field mismatches, unsigned or stale exceptions,
    /// structural overlap errors, R2000 errors, consensus) still fails.
    #[must_use]
    pub fn masks(self, failure: &FailureReason, result: &CaseResult) -> bool {
        match (self, failure.maskable()) {
            (Self::PaddingConvention, Maskable::GroundTruthStart { expected, actual }) => {
                padding_byte_holds(expected, actual, result.load_addr, &result.system)
            }
            (Self::PayloadMismatchUnderInvestigation, Maskable::PayloadHash) => true,
            (Self::PaddingConvention, Maskable::PayloadHash | Maskable::Never)
            | (
                Self::PayloadMismatchUnderInvestigation,
                Maskable::GroundTruthStart { .. } | Maskable::Never,
            ) => false,
        }
    }
}

impl fmt::Display for FailureReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownCase => f.write_str("not listed in the corpus manifest"),
            Self::R2000Failed { error } => {
                write!(f, "R2000 failed: {} ({})", error.variant, error.message)
            }
            Self::InconsistentResult {
                start,
                end,
                data_len,
            } => write!(
                f,
                "R2000 result is inconsistent: range {start}-{end} with {data_len} payload bytes"
            ),
            Self::MissingUnp64 => f.write_str("no [case.unp64] recorded"),
            Self::InvalidUnp64Record => {
                f.write_str("[case.unp64] is inconsistent (end below start)")
            }
            Self::Unp64FailedUnaccepted { reason } => write!(
                f,
                "unp64 failed ({reason}) and the case has no accepted_divergence for it"
            ),
            Self::FieldMismatch {
                field,
                unp64,
                r2000,
            } => write!(
                f,
                "`{field}` differs from unp64: unp64 {unp64}, R2000 {r2000}"
            ),
            Self::PayloadHashMismatch { unp64, r2000 } => write!(
                f,
                "payload differs from unp64 over the same range: unp64 fnv64 {unp64}, R2000 \
                 fnv64 {r2000}"
            ),
            Self::UnsignedDivergence => f.write_str(
                "accepted_divergence is proposed, unsigned: review it and set `reviewed_by`",
            ),
            Self::KindNoLongerHolds { kind, detail } => write!(
                f,
                "accepted_divergence kind `{kind}` no longer holds: {detail}"
            ),
            Self::StaleDivergence { stale, all_listed } => {
                let fields = join_fields(stale);
                if *all_listed {
                    write!(
                        f,
                        "stale accepted_divergence: every listed field ({fields}) now equals \
                         unp64; remove the accepted_divergence"
                    )
                } else {
                    write!(
                        f,
                        "stale accepted_divergence: {fields} now equal unp64; remove them from \
                         `fields`"
                    )
                }
            }
            Self::MissingOverlap => f.write_str(
                "ranges differ but [case.unp64] has no `overlap`; re-run \
                 `unpacker_compare_all --update`",
            ),
            Self::OverlapOutsideResult { window, r2000 } => write!(
                f,
                "overlap window {window} is not inside R2000's range {r2000}"
            ),
            Self::OverlapWindowMismatch {
                window,
                intersection,
            } => match intersection {
                Some(intersection) => write!(
                    f,
                    "overlap window {window} is not the current intersection {intersection}; \
                     re-run `unpacker_compare_all --update`"
                ),
                None => write!(
                    f,
                    "overlap window {window} recorded, but R2000's and unp64's ranges no longer \
                     intersect"
                ),
            },
            Self::OverlapHashMismatch {
                window,
                unp64,
                r2000,
            } => write!(
                f,
                "payload differs from unp64 over the overlap {window}: unp64 fnv64 {unp64}, \
                 R2000 fnv64 {r2000}"
            ),
            Self::GroundTruthMismatch {
                group,
                field,
                expected,
                actual,
            } => write!(
                f,
                "`{field}` differs from the original of group `{group}`: original {expected}, \
                 R2000 {actual}"
            ),
            Self::GroundTruthPayloadMismatch {
                group,
                first,
                count,
            } => write!(
                f,
                "payload differs from the original of group `{group}`: {count} byte(s), first \
                 at {first}"
            ),
            Self::ConsensusMismatch { property, values } => {
                let members = values
                    .iter()
                    .map(|(file, value)| format!("{file} = {value}"))
                    .collect::<Vec<_>>()
                    .join("; ");
                write!(f, "group members disagree on {property}: {members}")
            }
            Self::UnexpectedPass { reason } => {
                write!(f, "unexpected pass: remove expected_fail = \"{reason}\"")
            }
        }
    }
}

/// One failure row: a subject and why it failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseFailure {
    /// What failed.
    pub subject: Subject,
    /// Why it failed.
    pub reason: FailureReason,
}

impl CaseFailure {
    /// A failure of the case `file`.
    pub(super) fn case(file: &str, reason: FailureReason) -> Self {
        Self {
            subject: Subject::Case(file.to_string()),
            reason,
        }
    }

    /// Sort key for failure lists: subject, then reason kind, then text (so
    /// the order is total and deterministic).
    pub(super) fn sort_key(&self) -> (Subject, u8, String) {
        (
            self.subject.clone(),
            self.reason.rank(),
            self.reason.to_string(),
        )
    }
}

// =============================================================================
// Reports
// =============================================================================

/// The oracle that decided a passing case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Oracle {
    /// R2000 matched the recorded unp64 result exactly.
    Unp64Reference,
    /// R2000 differs from unp64 only as a reviewed exception allows.
    AcceptedDivergence {
        /// The exception's kind.
        kind: DivergenceKind,
    },
    /// R2000 matched the group's original.
    GroundTruth {
        /// The group.
        group: String,
    },
}

/// Information about a passing case that a report must show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReportNote {
    /// An `unexplained` exception: green, but must be investigated.
    Unexplained {
        /// The exception's note, if any.
        note: Option<String>,
    },
    /// An exception signed with [`INITIAL_REVIEW_MARKER`]: the user must
    /// confirm it.
    PendingReview,
    /// An `expected_fail` case failed as expected.
    ExpectedFailure {
        /// The marker's reason.
        reason: ExpectedFailReason,
        /// The failures that the marker absorbed.
        failures: Vec<FailureReason>,
    },
    /// unp64 disagrees with a ground-truth-verified result (informational).
    Unp64DisagreesWithGroundTruth {
        /// Fields where unp64 differs from R2000.
        fields: Vec<Field>,
        /// `true` if the ranges match but the payload hashes differ.
        payload: bool,
    },
    /// unp64 failed on a ground-truth-verified case (informational).
    Unp64FailedOnGroundTruth {
        /// unp64's failure reason.
        reason: String,
    },
}

impl ReportNote {
    /// Returns `true` if the note asks the user to do something.
    #[must_use]
    pub fn is_action_item(&self) -> bool {
        matches!(
            self,
            Self::Unexplained { .. } | Self::PendingReview | Self::ExpectedFailure { .. }
        )
    }
}

impl fmt::Display for ReportNote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unexplained { note } => match note {
                Some(note) => write!(f, "unexplained divergence, investigate: {note}"),
                None => f.write_str("unexplained divergence, investigate"),
            },
            Self::PendingReview => write!(
                f,
                "exception signed with the initial marker \"{INITIAL_REVIEW_MARKER}\"; confirm \
                 it and sign with your name"
            ),
            Self::ExpectedFailure { reason, failures } => {
                let details = failures
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; ");
                write!(f, "expected failure ({reason}): {details}")
            }
            Self::Unp64DisagreesWithGroundTruth { fields, payload } => {
                let mut parts: Vec<String> = fields.iter().map(ToString::to_string).collect();
                if *payload {
                    parts.push("payload".to_string());
                }
                write!(
                    f,
                    "unp64 disagrees with the verified result on: {}",
                    parts.join(", ")
                )
            }
            Self::Unp64FailedOnGroundTruth { reason } => {
                write!(f, "unp64 failed ({reason}); the original verifies R2000")
            }
        }
    }
}

/// A passing case and what the report must show about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseReport {
    /// The case.
    pub file: String,
    /// The oracle that decided it.
    pub oracle: Oracle,
    /// Notes for the report.
    pub notes: Vec<ReportNote>,
}

/// The result of evaluating a set of cases.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CorpusOutcome {
    /// Passing cases, sorted by file.
    pub reports: Vec<CaseReport>,
    /// All failures, sorted by subject and then by reason kind.
    pub failures: Vec<CaseFailure>,
}

impl CorpusOutcome {
    /// Returns `true` if nothing failed.
    #[must_use]
    pub fn is_green(&self) -> bool {
        self.failures.is_empty()
    }
}
