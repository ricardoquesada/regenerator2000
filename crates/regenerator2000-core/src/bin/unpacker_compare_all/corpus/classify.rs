//! Divergence classifier: determines why R2000 and an `unp64` run disagree.
//!
//! The compare tool calls this module to propose `accepted_divergence`
//! entries. The range-based kinds share the same predicates as the oracle in
//! `kinds` so the corpus test accepts every signed proposal.

use super::kinds::{
    DivergenceKind, padding_byte_holds, unp64_default_end_holds, unp64_default_start_holds,
};
use super::values::{AddrRange, Addressed, Unp64Entry, first_difference};
use super::{CaseResult, UnpackedImage};

/// Observed result of one `unp64` run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unp64Observation {
    /// `unp64` saved a payload.
    Saved {
        /// First saved address.
        start: u16,
        /// The reported entry point.
        entry: Unp64Entry,
        /// Saved payload without the PRG header.
        payload: Vec<u8>,
    },
    /// `unp64` saved nothing.
    Failed {
        /// Short description of why `unp64` failed.
        reason: String,
        /// `true` if `unp64` stopped on an unimplemented opcode.
        missing_opcode: bool,
    },
}

/// Reason why R2000 and `unp64` disagree on a case, ordered from benign to
/// suspicious.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Divergence {
    /// Identical range, entry, and payload.
    Agree,
    /// Only the start differs, and the starts form the BASIC link byte pair.
    PaddingByte,
    /// Only the end differs, and the `unp64` end is its top-page default.
    Unp64DefaultEnd,
    /// Only the start differs, and the `unp64` start is its default.
    Unp64DefaultStart,
    /// `unp64` stopped on an unimplemented opcode.
    Unp64MissingOpcode,
    /// The tools differ and no benign rule matches.
    Unexplained,
    /// The payloads differ over the shared range.
    PayloadMismatch {
        /// Address of the first differing byte.
        first: u16,
        /// Number of differing bytes.
        count: usize,
    },
    /// Both tools resolved different entry addresses.
    EntryMismatch,
    /// At least one tool failed.
    OneSideFailed {
        /// R2000 error variant, if R2000 failed.
        r2000: Option<String>,
        /// `unp64` failure reason, if `unp64` failed.
        unp64: Option<String>,
    },
}

impl Divergence {
    /// Returns the manifest [`DivergenceKind`] for this divergence, or `None`
    /// when the tools agree or when the tool must not propose an exception.
    ///
    /// A failed `unp64` run with a successful R2000 run maps to `unexplained`.
    #[must_use]
    pub fn accepted_kind(&self) -> Option<DivergenceKind> {
        match self {
            Self::Agree | Self::PayloadMismatch { .. } | Self::EntryMismatch => None,
            Self::PaddingByte => Some(DivergenceKind::PaddingByte),
            Self::Unp64DefaultEnd => Some(DivergenceKind::Unp64DefaultEnd),
            Self::Unp64DefaultStart => Some(DivergenceKind::Unp64DefaultStart),
            Self::Unp64MissingOpcode => Some(DivergenceKind::Unp64MissingOpcode),
            Self::Unexplained => Some(DivergenceKind::Unexplained),
            Self::OneSideFailed { r2000, unp64 } => match (r2000, unp64) {
                (None, Some(_)) => Some(DivergenceKind::Unexplained),
                _ => None,
            },
        }
    }
}

/// Classifies how the R2000 `result` differs from `unp64`.
///
/// Suspicious findings take precedence over benign ones: tool failures first,
/// then payload differences over the shared range, then entry differences, and
/// finally range-based kinds.
///
/// # Example
///
/// R2000 `$0801-$7949` and `unp64` `$0801-$FFFD` with identical shared bytes
/// and entry classify as [`Divergence::Unp64DefaultEnd`].
#[must_use]
pub fn classify(result: &CaseResult, unp64: &Unp64Observation) -> Divergence {
    match (&result.outcome, unp64) {
        (Err(error), Unp64Observation::Failed { reason, .. }) => Divergence::OneSideFailed {
            r2000: Some(error.variant.clone()),
            unp64: Some(reason.clone()),
        },
        (Err(error), Unp64Observation::Saved { .. }) => Divergence::OneSideFailed {
            r2000: Some(error.variant.clone()),
            unp64: None,
        },
        (
            Ok(_),
            Unp64Observation::Failed {
                missing_opcode: true,
                ..
            },
        ) => Divergence::Unp64MissingOpcode,
        (Ok(_), Unp64Observation::Failed { reason, .. }) => Divergence::OneSideFailed {
            r2000: None,
            unp64: Some(reason.clone()),
        },
        (
            Ok(image),
            Unp64Observation::Saved {
                start,
                entry,
                payload,
            },
        ) => classify_saved(
            image,
            Addressed {
                start: *start,
                data: payload,
            },
            *entry,
            result,
        ),
    }
}

/// Classifies two successful unpack runs.
fn classify_saved(
    image: &UnpackedImage,
    unp64: Addressed<'_>,
    unp64_entry: Unp64Entry,
    result: &CaseResult,
) -> Divergence {
    let (Some(r2000_range), Some(unp64_range)) = (
        image.range(),
        AddrRange::from_start_len(unp64.start, unp64.data.len()),
    ) else {
        // An empty or inconsistent payload cannot match any benign rule.
        return Divergence::Unexplained;
    };
    let Some(window) = r2000_range.intersect(unp64_range) else {
        // Disjoint ranges share no bytes to compare.
        return Divergence::Unexplained;
    };

    // Report a full-window mismatch if either slice does not cover `window`.
    let (Some(r2000_bytes), Some(unp64_bytes)) =
        (image.addressed().slice(window), unp64.slice(window))
    else {
        return Divergence::PayloadMismatch {
            first: window.start(),
            count: window.len(),
        };
    };
    if let Some((first, count)) = first_difference(window.start(), r2000_bytes, unp64_bytes) {
        return Divergence::PayloadMismatch { first, count };
    }

    let entry_unresolved = match unp64_entry {
        Unp64Entry::Address(addr) if addr != image.entry => return Divergence::EntryMismatch,
        Unp64Entry::Address(_) => false,
        // `unp64` returned to BASIC `RUN` without resolving a `SYS` address.
        Unp64Entry::BasicRun => true,
    };

    let start_differs = unp64_range.start() != r2000_range.start();
    let end_differs = unp64_range.end() != r2000_range.end();
    match (start_differs, end_differs, entry_unresolved) {
        (false, false, false) => Divergence::Agree,
        (true, false, false)
            if padding_byte_holds(
                unp64_range.start(),
                r2000_range.start(),
                result.load_addr,
                &result.system,
            ) =>
        {
            Divergence::PaddingByte
        }
        (true, false, false)
            if unp64_default_start_holds(
                unp64_range.start(),
                r2000_range.start(),
                result.load_addr,
                &result.system,
            ) =>
        {
            Divergence::Unp64DefaultStart
        }
        (false, true, false) if unp64_default_end_holds(unp64_range.end(), r2000_range.end()) => {
            Divergence::Unp64DefaultEnd
        }
        _ => Divergence::Unexplained,
    }
}
