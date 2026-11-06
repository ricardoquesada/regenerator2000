//! Converts an `unp64` outcome into a `[case.unp64]` manifest record.
//!
//! `--update` and `--check` build records in the same way, except for the
//! overlap window. `--update` intersects the `unp64` range with the R2000
//! range. `--check` re-hashes the recorded window so drift detection does not
//! depend on R2000.

use crate::corpus::{
    AddrRange, CaseResult, Fnv64, HexAddr, Overlap, Unp64Failed, Unp64Record, Unp64Saved,
    hash_window,
};
use crate::unp64::Unp64Outcome;

/// Builds the record of `outcome` from `unp64` `version`, including the
/// overlap hash over `window` when `outcome` covers `window`.
///
/// # Example
///
/// A saved payload of 16 bytes at `$0801` with entry `$080D` produces
/// `start = "$0801"`, `end = "$0810"`, `len = 16`.
#[must_use]
pub fn build_record(
    outcome: &Unp64Outcome,
    version: &str,
    window: Option<AddrRange>,
) -> Unp64Record {
    match outcome {
        Unp64Outcome::Failed { reason, .. } => Unp64Record::Failed(Unp64Failed {
            reason: reason.clone(),
            version: version.to_string(),
        }),
        Unp64Outcome::Saved { saved, entry, .. } => {
            let range = saved.range;
            let overlap = window.and_then(|window| {
                hash_window(range.start(), &saved.payload, window).map(|fnv64| Overlap {
                    start: HexAddr(window.start()),
                    end: HexAddr(window.end()),
                    fnv64,
                })
            });
            Unp64Record::Saved(Unp64Saved {
                start: HexAddr(range.start()),
                end: HexAddr(range.end()),
                entry: *entry,
                // An `AddrRange` holds 1 to 65,536 bytes, so `end - start`
                // fits in `u16` and `+ 1` fits in `u32`.
                len: u32::from(range.end() - range.start()) + 1,
                payload_fnv64: Fnv64::of(&saved.payload),
                overlap,
                version: version.to_string(),
            })
        }
    }
}

/// Returns the overlap window that `--update` records.
///
/// This window is the intersection of the `unp64` and R2000 ranges when they
/// differ. Returns `None` when the ranges are equal, disjoint, or when either
/// side fails.
///
/// # Example
///
/// `unp64` `$0801-$FFFD` and R2000 `$0801-$7949` return `$0801-$7949`.
#[must_use]
pub fn update_window(outcome: &Unp64Outcome, result: &CaseResult) -> Option<AddrRange> {
    let Unp64Outcome::Saved { saved, .. } = outcome else {
        return None;
    };
    let r2000 = result.outcome.as_ref().ok()?.range()?;
    if r2000 == saved.range {
        return None;
    }
    r2000.intersect(saved.range)
}

/// Returns the overlap window from `record` for `--check`.
#[must_use]
pub fn recorded_window(record: Option<&Unp64Record>) -> Option<AddrRange> {
    match record? {
        Unp64Record::Saved(saved) => saved.overlap.as_ref()?.range(),
        Unp64Record::Failed(_) => None,
    }
}

/// Returns `true` if both records hold identical results, ignoring `version`.
///
/// A reviewed exception binds to the recorded values, not to the `unp64`
/// version string. An `unp64` upgrade that reproduces identical results keeps
/// every signature.
#[must_use]
pub fn same_results(left: &Unp64Record, right: &Unp64Record) -> bool {
    without_version(left) == without_version(right)
}

/// Returns a copy of `record` with an empty `version` string.
fn without_version(record: &Unp64Record) -> Unp64Record {
    let mut copy = record.clone();
    match &mut copy {
        Unp64Record::Saved(saved) => saved.version.clear(),
        Unp64Record::Failed(failed) => failed.version.clear(),
    }
    copy
}
