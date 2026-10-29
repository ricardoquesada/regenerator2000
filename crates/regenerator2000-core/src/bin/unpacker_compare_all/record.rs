//! Turning an unp64 outcome into the manifest's `[case.unp64]` record.
//!
//! `--update` and `--check` build records the same way; they differ only in
//! the overlap window: `--update` uses the current intersection with R2000's
//! range (Addendum B1), `--check` re-hashes the window that was recorded, so
//! drift detection never depends on R2000.

use crate::corpus::{
    AddrRange, CaseResult, Fnv64, HexAddr, Overlap, Unp64Failed, Unp64Record, Unp64Saved,
    hash_window,
};
use crate::unp64::Unp64Outcome;

/// Builds the record of `outcome`, produced by unp64 `version`, with the
/// overlap hash of unp64's bytes over `window` (if given and covered).
///
/// # Example
///
/// A saved payload of 16 bytes at `$0801` with entry `$080D` gives
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
                // Exact and infallible: an `AddrRange` holds 1 to 65 536
                // bytes, so `end - start` fits a `u16` and `+ 1` a `u32`.
                len: u32::from(range.end() - range.start()) + 1,
                payload_fnv64: Fnv64::of(&saved.payload),
                overlap,
                version: version.to_string(),
            })
        }
    }
}

/// The overlap window `--update` records: the intersection of unp64's and
/// R2000's ranges when they differ, or `None` when they are equal (the whole
/// payload hash is compared then), disjoint, or either side failed.
///
/// # Example
///
/// unp64 `$0801-$FFFD` and R2000 `$0801-$7949` give `$0801-$7949`.
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

/// The window recorded in `record`, if any (what `--check` re-hashes).
#[must_use]
pub fn recorded_window(record: Option<&Unp64Record>) -> Option<AddrRange> {
    match record? {
        Unp64Record::Saved(saved) => saved.overlap.as_ref()?.range(),
        Unp64Record::Failed(_) => None,
    }
}

/// Returns `true` if both records hold the same results, ignoring the unp64
/// version that produced them.
///
/// A reviewed exception is bound to the recorded values, not to the binary's
/// version string, so an unp64 upgrade that reproduces the same results keeps
/// every signature.
#[must_use]
pub fn same_results(left: &Unp64Record, right: &Unp64Record) -> bool {
    without_version(left) == without_version(right)
}

/// A copy of `record` with an empty version.
fn without_version(record: &Unp64Record) -> Unp64Record {
    let mut copy = record.clone();
    match &mut copy {
        Unp64Record::Saved(saved) => saved.version.clear(),
        Unp64Record::Failed(failed) => failed.version.clear(),
    }
    copy
}
