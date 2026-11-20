//! Shared corpus file paths, PRG layout constants, and comparison thresholds.

/// Directory that holds the fixtures, manifest, and baseline.
///
/// The compiler resolves this path from `CARGO_MANIFEST_DIR` so the compare
/// tool and the corpus test find it from any working directory.
pub const FIXTURE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/6502");

/// File name of the corpus manifest inside [`FIXTURE_DIR`].
pub const MANIFEST_FILE: &str = "unpack_corpus.toml";

/// File name of the baseline inside [`FIXTURE_DIR`].
pub const BASELINE_FILE: &str = "unpack_baseline.toml";

/// Length in bytes of the little-endian load address at the start of a PRG.
pub const PRG_HEADER_LEN: usize = 2;

/// `reviewed_by` value for exceptions from the initial classification that
/// still await user review.
///
/// This marker counts as signed so CI passes, while reports list the case as a
/// user action item.
pub const INITIAL_REVIEW_MARKER: &str = "initial-classification-2026-10-28 (pending ricardoq)";

/// Lowest end address that `unp64` produces when no end scanner fires.
///
/// `unp64` does not track memory writes. Without a scanner end address, it
/// saves up to `$FFFF` (or a few bytes below), so an `unp64` end on the top
/// page indicates its default rather than a real program boundary.
pub const UNP64_TOP_PAGE: u16 = 0xFF00;

/// Instruction budget for cases that omit `max_instructions`.
///
/// This constant equals `UnpackConfig::default().max_instructions`.
pub const DEFAULT_MAX_INSTRUCTIONS: u64 = 50_000_000;
