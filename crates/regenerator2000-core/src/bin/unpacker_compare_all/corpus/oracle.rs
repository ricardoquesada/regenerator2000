//! Oracle rules: `unp64` reference, accepted divergence, ground truth,
//! consensus, and `expected_fail`.
//!
//! Every check collects all findings instead of stopping at the first failure
//! so a single run reports the complete regression picture.

use std::collections::{BTreeMap, BTreeSet};

use super::kinds::{
    DivergenceKind, Field, join_fields, padding_byte_holds, padding_byte_pairs,
    unp64_default_end_holds, unp64_default_start_holds,
};
use super::manifest::ReviewState;
use super::values::{AddrRange, Addressed, Fnv64, HexAddr, first_difference};
use super::verdict::ConsensusProperty;
use super::{
    AcceptedDivergence, Case, CaseFailure, CaseReport, CaseResult, Corpus, CorpusError,
    CorpusOutcome, FailureReason, FixtureResolver, Group, Oracle, PRG_HEADER_LEN, ReportNote,
    Subject, UNP64_TOP_PAGE, Unp64Record, Unp64Saved, UnpackedImage,
};

// =============================================================================
// Ground truth
// =============================================================================

/// Original, unpacked program of a [`Group`], loaded from its PRG file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroundTruth {
    /// The group ID.
    pub group: String,
    /// Address range of the original program from load address to last byte.
    pub range: AddrRange,
    /// Entry point of the group.
    pub entry: u16,
    /// Original payload without the two-byte PRG header.
    pub data: Vec<u8>,
}

impl GroundTruth {
    /// Derives the ground truth from the original PRG bytes.
    ///
    /// # Example
    ///
    /// A PRG with header `00 08` and 100 payload bytes covers `$0800-$0863`.
    ///
    /// # Errors
    ///
    /// Returns [`CorpusError::InvalidGroupOriginal`] if the PRG has no
    /// payload or extends past `$FFFF`.
    pub fn from_prg(group: &Group, prg: &[u8]) -> Result<Self, CorpusError> {
        let invalid = |reason: &str| CorpusError::InvalidGroupOriginal {
            group: group.id.clone(),
            file: group.original.clone(),
            reason: reason.to_string(),
        };
        let (header, payload) = prg
            .split_first_chunk::<PRG_HEADER_LEN>()
            .ok_or_else(|| invalid("shorter than the two-byte PRG header"))?;
        let load_addr = u16::from_le_bytes(*header);
        let range = AddrRange::from_start_len(load_addr, payload.len())
            .ok_or_else(|| invalid("empty, or extends past $FFFF"))?;
        Ok(Self {
            group: group.id.clone(),
            range,
            entry: group.entry.0,
            data: payload.to_vec(),
        })
    }

    /// Returns the original payload anchored at its load address.
    fn addressed(&self) -> Addressed<'_> {
        Addressed {
            start: self.range.start(),
            data: &self.data,
        }
    }
}

// =============================================================================
// Evaluator
// =============================================================================

/// Evaluates R2000 results against the manifest oracles.
///
/// [`Evaluator::new`] loads the original binary of every group once so
/// evaluation performs no file I/O.
#[derive(Debug)]
pub struct Evaluator<'c> {
    corpus: &'c Corpus,
    truths: BTreeMap<String, GroundTruth>,
}

impl<'c> Evaluator<'c> {
    /// Loads the ground truth of every group through `fixtures`.
    ///
    /// # Errors
    ///
    /// Returns all [`CorpusError`] values from reading or decoding the group
    /// originals, sorted by [`CorpusError::sort_key`].
    pub fn new(
        corpus: &'c Corpus,
        fixtures: &dyn FixtureResolver,
    ) -> Result<Self, Vec<CorpusError>> {
        let mut truths = BTreeMap::new();
        let mut errors = Vec::new();
        for group in &corpus.groups {
            match fixtures
                .read(&group.original)
                .and_then(|prg| GroundTruth::from_prg(group, &prg))
            {
                Ok(truth) => {
                    truths.insert(group.id.clone(), truth);
                }
                Err(error) => errors.push(error),
            }
        }
        if !errors.is_empty() {
            errors.sort_by_cached_key(CorpusError::sort_key);
            return Err(errors);
        }
        Ok(Self { corpus, truths })
    }

    /// Evaluates one case result.
    ///
    /// # Errors
    ///
    /// Returns every unmasked failure of the case. An `expected_fail` case
    /// whose failures are all masked returns `Ok` with a
    /// [`ReportNote::ExpectedFailure`] note. An `expected_fail` case with no
    /// masked failures fails with [`FailureReason::UnexpectedPass`].
    pub fn evaluate(&self, result: &CaseResult) -> Result<CaseReport, Vec<CaseFailure>> {
        let Some(case) = self.corpus.case(&result.file) else {
            return Err(vec![CaseFailure::case(
                &result.file,
                FailureReason::UnknownCase,
            )]);
        };
        let verdict = self.check_case(case, result);
        apply_expected_fail(case, result, verdict)
    }

    /// Checks that all unpacked, non-`expected_fail` members of each group in
    /// `results` agree on range, entry, and payload.
    #[must_use]
    pub fn evaluate_consensus(&self, results: &[CaseResult]) -> Vec<CaseFailure> {
        let mut failures = Vec::new();
        for group in &self.corpus.groups {
            let members: Vec<(&str, &UnpackedImage)> = self
                .corpus
                .members(&group.id)
                .filter(|case| case.expected_fail.is_none())
                .filter_map(|case| {
                    let result = results.iter().find(|r| r.file == case.file)?;
                    let image = result.outcome.as_ref().ok()?;
                    Some((case.file.as_str(), image))
                })
                .collect();
            failures.extend(consensus_failures(&group.id, &members));
        }
        failures
    }

    /// Evaluates every result and group consensus, returning sorted reports
    /// and failures.
    #[must_use]
    pub fn evaluate_all(&self, results: &[CaseResult]) -> CorpusOutcome {
        let mut outcome = CorpusOutcome::default();
        for result in results {
            match self.evaluate(result) {
                Ok(report) => outcome.reports.push(report),
                Err(failures) => outcome.failures.extend(failures),
            }
        }
        outcome.failures.extend(self.evaluate_consensus(results));
        outcome.reports.sort_by(|a, b| a.file.cmp(&b.file));
        outcome.failures.sort_by_cached_key(CaseFailure::sort_key);
        outcome
    }

    /// Runs the primary oracle of `case` on `result`.
    fn check_case(&self, case: &Case, result: &CaseResult) -> CaseVerdict {
        let truth = case.group.as_deref().and_then(|id| self.truths.get(id));
        let oracle = match (truth, &case.accepted_divergence) {
            (Some(truth), _) => Oracle::GroundTruth {
                group: truth.group.clone(),
            },
            (None, Some(divergence)) => Oracle::AcceptedDivergence {
                kind: divergence.kind,
            },
            (None, None) => Oracle::Unp64Reference,
        };
        let mut verdict = CaseVerdict::new(oracle);

        let image = match &result.outcome {
            Ok(image) => image,
            Err(error) => {
                verdict.failures.push(FailureReason::R2000Failed {
                    error: error.clone(),
                });
                return verdict;
            }
        };
        let Some(range) = image.range() else {
            verdict.failures.push(FailureReason::InconsistentResult {
                start: HexAddr(image.start),
                end: HexAddr(image.end),
                data_len: image.data.len(),
            });
            return verdict;
        };
        let checked = CheckedImage { image, range };

        match truth {
            Some(truth) => check_ground_truth(truth, checked, case.unp64.as_ref(), &mut verdict),
            None => check_unp64(case, result, checked, &mut verdict),
        }
        verdict
    }
}

/// Verdict of one case before [`apply_expected_fail`] runs.
struct CaseVerdict {
    oracle: Oracle,
    failures: Vec<FailureReason>,
    notes: Vec<ReportNote>,
}

impl CaseVerdict {
    fn new(oracle: Oracle) -> Self {
        Self {
            oracle,
            failures: Vec::new(),
            notes: Vec::new(),
        }
    }
}

/// R2000 image whose payload length matches its address range.
#[derive(Clone, Copy)]
struct CheckedImage<'a> {
    image: &'a UnpackedImage,
    range: AddrRange,
}

impl CheckedImage<'_> {
    /// Returns the R2000 value of `field`.
    fn value(&self, field: Field) -> u16 {
        match field {
            Field::Start => self.image.start,
            Field::End => self.image.end,
            Field::Entry => self.image.entry,
        }
    }
}

/// One field where `unp64` and R2000 disagree.
struct FieldDifference {
    field: Field,
    /// `unp64` value in display form (`$XXXX` or `RUN`).
    unp64: String,
}

/// Lists every field where `saved` and `image` differ, in canonical field
/// order.
fn unp64_differences(saved: &Unp64Saved, image: CheckedImage<'_>) -> Vec<FieldDifference> {
    Field::ALL
        .into_iter()
        .filter_map(|field| {
            let r2000 = image.value(field);
            let (unp64, equal) = match field {
                Field::Start => (saved.start.to_string(), saved.start.0 == r2000),
                Field::End => (saved.end.to_string(), saved.end.0 == r2000),
                Field::Entry => (saved.entry.to_string(), saved.entry.matches(r2000)),
            };
            (!equal).then_some(FieldDifference { field, unp64 })
        })
        .collect()
}

// =============================================================================
// unp64 reference and accepted divergence
// =============================================================================

/// Checks the `unp64` reference rule, with or without an accepted divergence.
fn check_unp64(
    case: &Case,
    result: &CaseResult,
    image: CheckedImage<'_>,
    verdict: &mut CaseVerdict,
) {
    let divergence = case.accepted_divergence.as_ref();
    if let Some(divergence) = divergence {
        record_review_state(divergence, verdict);
    }

    let saved = match &case.unp64 {
        None => {
            verdict.failures.push(FailureReason::MissingUnp64);
            return;
        }
        Some(Unp64Record::Failed(failed)) => {
            if !divergence.is_some_and(|d| d.kind.allowed_for_failed_unp64()) {
                verdict.failures.push(FailureReason::Unp64FailedUnaccepted {
                    reason: failed.reason.clone(),
                });
            }
            return;
        }
        Some(Unp64Record::Saved(saved)) => saved,
    };
    let Some(unp64_range) = saved.range() else {
        verdict.failures.push(FailureReason::InvalidUnp64Record);
        return;
    };

    let differences = unp64_differences(saved, image);
    for difference in &differences {
        if divergence.is_some_and(|d| d.fields.contains(difference.field)) {
            continue;
        }
        verdict.failures.push(FailureReason::FieldMismatch {
            field: difference.field,
            unp64: difference.unp64.clone(),
            r2000: HexAddr(image.value(difference.field)),
        });
    }

    if let Some(divergence) = divergence {
        let differing: Vec<Field> = differences.iter().map(|d| d.field).collect();
        check_divergence_kind(divergence, saved, image, result, &differing, verdict);
    }

    check_payload(saved, unp64_range, image, verdict);
}

/// Records the review state of an exception on `verdict`.
///
/// An unsigned exception adds a failure, while the initial review marker and
/// `unexplained` kind add report notes.
fn record_review_state(divergence: &AcceptedDivergence, verdict: &mut CaseVerdict) {
    match divergence.review_state() {
        ReviewState::Unsigned => verdict.failures.push(FailureReason::UnsignedDivergence),
        ReviewState::PendingUser => verdict.notes.push(ReportNote::PendingReview),
        ReviewState::Signed => {}
    }
    if divergence.kind == DivergenceKind::Unexplained {
        verdict.notes.push(ReportNote::Unexplained {
            note: divergence.note.clone(),
        });
    }
}

/// Checks that `divergence` has no stale fields and satisfies its kind
/// predicate.
fn check_divergence_kind(
    divergence: &AcceptedDivergence,
    saved: &Unp64Saved,
    image: CheckedImage<'_>,
    result: &CaseResult,
    differing: &[Field],
    verdict: &mut CaseVerdict,
) {
    let listed = divergence.fields.as_slice();
    let stale: Vec<Field> = listed
        .iter()
        .copied()
        .filter(|field| !differing.contains(field))
        .collect();
    if !stale.is_empty() {
        let all_listed = stale.len() == listed.len();
        verdict
            .failures
            .push(FailureReason::StaleDivergence { stale, all_listed });
    }

    if let Some(detail) = kind_violation(divergence, saved, image, result) {
        verdict.failures.push(FailureReason::KindNoLongerHolds {
            kind: divergence.kind,
            detail,
        });
    }
}

/// Returns the reason why `divergence.kind` fails for this result, or `None`
/// when the predicate holds.
fn kind_violation(
    divergence: &AcceptedDivergence,
    saved: &Unp64Saved,
    image: CheckedImage<'_>,
    result: &CaseResult,
) -> Option<String> {
    let kind = divergence.kind;
    // Check required fields here as well so an unvalidated manifest cannot
    // pass a mis-shaped exception.
    if let Some(required) = kind.required_fields()
        && divergence.fields.as_slice() != required
    {
        return Some(format!("it requires fields = [{}]", join_fields(required)));
    }
    let (unp64_start, unp64_end) = (saved.start.0, saved.end.0);
    let (r2000_start, r2000_end) = (image.image.start, image.image.end);
    match kind {
        DivergenceKind::PaddingByte
            if !padding_byte_holds(unp64_start, r2000_start, result.load_addr, &result.system) =>
        {
            Some(format!(
                "the starts must be the BASIC link byte pair {} (unp64 {}, R2000 {})",
                padding_byte_pairs(result.load_addr, &result.system),
                HexAddr(unp64_start),
                HexAddr(r2000_start)
            ))
        }
        DivergenceKind::Unp64DefaultEnd if !unp64_default_end_holds(unp64_end, r2000_end) => {
            Some(format!(
                "unp64's end must be above R2000's and at or above {} (unp64 {}, R2000 {})",
                HexAddr(UNP64_TOP_PAGE),
                HexAddr(unp64_end),
                HexAddr(r2000_end)
            ))
        }
        DivergenceKind::Unp64DefaultStart
            if !unp64_default_start_holds(
                unp64_start,
                r2000_start,
                result.load_addr,
                &result.system,
            ) =>
        {
            Some(format!(
                "unp64's start must be min(load {}, RAM start {}) and differ from R2000's \
                 (unp64 {}, R2000 {})",
                HexAddr(result.load_addr),
                HexAddr(result.system.ram_start()),
                HexAddr(unp64_start),
                HexAddr(r2000_start)
            ))
        }
        DivergenceKind::Unp64MissingOpcode => {
            Some("it requires a failed unp64 run, but unp64 saved a payload".to_string())
        }
        _ => None,
    }
}

/// Checks the payload hash over the full range when ranges match, or over the
/// recorded overlap window when ranges differ.
fn check_payload(
    saved: &Unp64Saved,
    unp64_range: AddrRange,
    image: CheckedImage<'_>,
    verdict: &mut CaseVerdict,
) {
    if unp64_range == image.range {
        let r2000 = image.image.fnv64();
        if r2000 != saved.payload_fnv64 {
            verdict.failures.push(FailureReason::PayloadHashMismatch {
                unp64: saved.payload_fnv64,
                r2000,
            });
        }
        return;
    }

    let Some(overlap) = &saved.overlap else {
        verdict.failures.push(FailureReason::MissingOverlap);
        return;
    };
    let Some(window) = overlap.range() else {
        verdict.failures.push(FailureReason::InvalidUnp64Record);
        return;
    };
    // `slice` returns `None` when `window` extends outside the R2000 payload.
    let Some(bytes) = image.image.addressed().slice(window) else {
        verdict.failures.push(FailureReason::OverlapOutsideResult {
            window,
            r2000: image.range,
        });
        return;
    };
    let intersection = image.range.intersect(unp64_range);
    if intersection != Some(window) {
        verdict.failures.push(FailureReason::OverlapWindowMismatch {
            window,
            intersection,
        });
    }
    let r2000 = Fnv64::of(bytes);
    if r2000 != overlap.fnv64 {
        verdict.failures.push(FailureReason::OverlapHashMismatch {
            window,
            unp64: overlap.fnv64,
            r2000,
        });
    }
}

// =============================================================================
// Ground truth
// =============================================================================

/// Checks `image` against `truth` and records any `unp64` disagreement as a
/// report note.
fn check_ground_truth(
    truth: &GroundTruth,
    image: CheckedImage<'_>,
    record: Option<&Unp64Record>,
    verdict: &mut CaseVerdict,
) {
    let expected = [
        (Field::Start, truth.range.start()),
        (Field::End, truth.range.end()),
        (Field::Entry, truth.entry),
    ];
    for (field, expected) in expected {
        let actual = image.value(field);
        if actual != expected {
            verdict.failures.push(FailureReason::GroundTruthMismatch {
                group: truth.group.clone(),
                field,
                expected: HexAddr(expected),
                actual: HexAddr(actual),
            });
        }
    }

    // Compare the overlapping bytes even when the ranges differ.
    if let Some(window) = image.range.intersect(truth.range)
        && let (Some(r2000), Some(original)) = (
            image.image.addressed().slice(window),
            truth.addressed().slice(window),
        )
        && let Some((first, count)) = first_difference(window.start(), r2000, original)
    {
        verdict
            .failures
            .push(FailureReason::GroundTruthPayloadMismatch {
                group: truth.group.clone(),
                first: HexAddr(first),
                count,
            });
    }

    // `unp64` disagreements are informational because the original binary is
    // the oracle.
    match record {
        Some(Unp64Record::Saved(saved)) => {
            let fields: Vec<Field> = unp64_differences(saved, image)
                .into_iter()
                .map(|difference| difference.field)
                .collect();
            let payload = unp64_payload_disagrees(saved, image);
            if !fields.is_empty() || payload {
                verdict
                    .notes
                    .push(ReportNote::Unp64DisagreesWithGroundTruth { fields, payload });
            }
        }
        Some(Unp64Record::Failed(failed)) => {
            verdict.notes.push(ReportNote::Unp64FailedOnGroundTruth {
                reason: failed.reason.clone(),
            });
        }
        None => verdict.failures.push(FailureReason::MissingUnp64),
    }
}

/// Returns `true` if the `unp64` full-payload hash or overlap hash disagrees
/// with `image`.
fn unp64_payload_disagrees(saved: &Unp64Saved, image: CheckedImage<'_>) -> bool {
    if saved.range() == Some(image.range) {
        return saved.payload_fnv64 != image.image.fnv64();
    }
    saved
        .overlap
        .as_ref()
        .and_then(|overlap| {
            let bytes = image.image.addressed().slice(overlap.range()?)?;
            Some(Fnv64::of(bytes) != overlap.fnv64)
        })
        .unwrap_or(false)
}

// =============================================================================
// expected_fail and consensus
// =============================================================================

/// Applies the `expected_fail` policy to `verdict`:
///
/// * Any failure that the `expected_fail` reason does not mask fails the case.
/// * A marked case with no masked failures fails with `UnexpectedPass`.
/// * Otherwise, the case passes with a [`ReportNote::ExpectedFailure`] note.
fn apply_expected_fail(
    case: &Case,
    result: &CaseResult,
    verdict: CaseVerdict,
) -> Result<CaseReport, Vec<CaseFailure>> {
    let CaseVerdict {
        oracle,
        failures,
        mut notes,
    } = verdict;
    let to_failures = |reasons: Vec<FailureReason>| -> Vec<CaseFailure> {
        reasons
            .into_iter()
            .map(|reason| CaseFailure::case(&case.file, reason))
            .collect()
    };
    let Some(reason) = case.expected_fail else {
        if !failures.is_empty() {
            return Err(to_failures(failures));
        }
        return Ok(CaseReport {
            file: case.file.clone(),
            oracle,
            notes,
        });
    };

    let (masked, unmasked): (Vec<FailureReason>, Vec<FailureReason>) = failures
        .into_iter()
        .partition(|failure| reason.masks(failure, result));
    if !unmasked.is_empty() {
        return Err(to_failures(unmasked));
    }
    if masked.is_empty() {
        return Err(to_failures(vec![FailureReason::UnexpectedPass { reason }]));
    }
    notes.push(ReportNote::ExpectedFailure {
        reason,
        failures: masked,
    });
    Ok(CaseReport {
        file: case.file.clone(),
        oracle,
        notes,
    })
}

/// Compares the members of `group` on range, entry, and payload when `members`
/// has at least two entries.
fn consensus_failures(group: &str, members: &[(&str, &UnpackedImage)]) -> Vec<CaseFailure> {
    if members.len() < 2 {
        return Vec::new();
    }
    ConsensusProperty::ALL
        .into_iter()
        .filter_map(|property| {
            let values: Vec<(String, String)> = members
                .iter()
                .map(|(file, image)| ((*file).to_string(), property.value_of(image)))
                .collect();
            let distinct: BTreeSet<&str> = values.iter().map(|(_, v)| v.as_str()).collect();
            (distinct.len() > 1).then(|| CaseFailure {
                subject: Subject::Group(group.to_string()),
                reason: FailureReason::ConsensusMismatch { property, values },
            })
        })
        .collect()
}
