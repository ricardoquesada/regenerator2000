//! Shared model and oracle logic for the unpacker corpus.
//!
//! The corpus manifest (`tests/6502/unpack_corpus.toml`) lists every packed
//! fixture that the unpacker handles, together with the recorded `unp64`
//! result from `unpacker_compare_all --update`. CI reads the recorded results
//! and does not need the `unp64` binary. The baseline file
//! (`tests/6502/unpack_baseline.toml`) freezes every observable output of
//! Regenerator 2000 (R2000) so refactors can check for zero diff.
//!
//! # Module map
//!
//! | Module | Responsibility |
//! | :-- | :-- |
//! | `values` | Text-form scalars ([`HexAddr`], [`Fnv64`], [`Unp64Entry`]), [`AddrRange`], and [`fnv1a64`] |
//! | `kinds` | Closed manifest enums ([`Field`], [`FieldSet`], [`DivergenceKind`], [`ExpectedFailReason`]) and kind predicates |
//! | `manifest` | Manifest model ([`Corpus`], [`Case`], [`Group`], [`Unp64Record`], [`AcceptedDivergence`]), parser, and writer |
//! | `validate` | [`Corpus::validate`]: cross-references, fixtures, and record consistency |
//! | `error` | [`CorpusError`] |
//! | `fixtures` | [`FixtureResolver`] and directory-backed [`DirFixtures`] |
//! | `result` | R2000 results as plain data ([`CaseResult`], [`UnpackedImage`], [`R2000Error`]) |
//! | `verdict` | Evaluation outcomes ([`CaseFailure`], [`FailureReason`], [`CaseReport`], [`ReportNote`]) |
//! | `oracle` | Oracle rules ([`Evaluator`], [`GroundTruth`]) |
//! | `classify` | Divergence classifier ([`classify()`], [`Divergence`]) for exception proposals |
//! | `filter` | Case selection ([`CorpusFilter`]) |
//! | `report` | Human-readable rendering of failures and action items |
//! | `baseline` | Frozen-output snapshot ([`Baseline`]) with writer and comparison |
//! | `runner` | R2000 runner ([`run_case`], [`run_cases`]) with per-case budgets |
//!
//! # Oracle rules
//!
//! The evaluator checks every case against one primary rule:
//!
//! | Rule | Applies when | R2000 must |
//! | :-- | :-- | :-- |
//! | Ground truth | Case belongs to a [`Group`] | Match the original range, bytes, and group entry |
//! | `unp64` reference | No group, no exception | Match the `unp64` start, end, entry, and payload hash |
//! | Accepted divergence | No group, `[case.accepted_divergence]` | Differ **only** in the listed fields, satisfy the `kind` predicate, and match `unp64` bytes over the overlap |
//!
//! Group consensus requires all non-`expected_fail` members of a group to
//! agree on range, entry, and payload. Every case either passes its rule or
//! fails.
//!
//! # Purity
//!
//! This module does not read environment variables. Only [`Corpus::load`],
//! [`Baseline::load`], and [`DirFixtures`] access the file system, so unit
//! tests run in memory.

// Both the `unpacker_compare_all` binary and the `unpack_corpus` integration
// test compile this module and use different subsets of its items.
#![allow(dead_code)]

mod baseline;
mod classify;
mod constants;
mod error;
mod filter;
mod fixtures;
mod kinds;
mod manifest;
mod oracle;
mod report;
mod result;
mod runner;
mod validate;
mod values;
mod verdict;

#[cfg(test)]
mod tests;

// Each including crate uses a subset of this public surface. Allow
// `unused_imports` on this re-export statement only so stray imports inside
// submodules still fail the build.
#[allow(unused_imports)]
pub use self::baseline::{
    Baseline, BaselineDiff, BaselineField, BaselineOutcome, BaselineRow, BaselineUnpacked,
};
#[allow(unused_imports)]
pub use self::classify::{Divergence, Unp64Observation, classify};
#[allow(unused_imports)]
pub use self::constants::{
    BASELINE_FILE, DEFAULT_MAX_INSTRUCTIONS, FIXTURE_DIR, INITIAL_REVIEW_MARKER, MANIFEST_FILE,
    PRG_HEADER_LEN, UNP64_TOP_PAGE,
};
#[allow(unused_imports)]
pub use self::error::CorpusError;
#[allow(unused_imports)]
pub use self::filter::CorpusFilter;
#[allow(unused_imports)]
pub use self::fixtures::{DirFixtures, FixtureResolver};
#[allow(unused_imports)]
pub use self::kinds::{DivergenceKind, ExpectedFailReason, Field, FieldSet};
#[allow(unused_imports)]
pub use self::manifest::{
    AcceptedDivergence, Case, Corpus, Group, Overlap, ReviewState, Unp64Failed, Unp64Record,
    Unp64Saved,
};
#[allow(unused_imports)]
pub use self::oracle::{Evaluator, GroundTruth};
#[allow(unused_imports)]
pub use self::report::{render_action_items, render_failure_table};
#[allow(unused_imports)]
pub use self::result::{CaseResult, R2000Error, UnpackedImage, unpack_error_variant};
#[allow(unused_imports)]
pub use self::runner::{Schedule, run_case, run_cases, unpack_config_for};
#[allow(unused_imports)]
pub use self::values::{
    AddrRange, Fnv64, HexAddr, Unp64Entry, ValueFormatError, fnv1a64, hash_window,
};
#[allow(unused_imports)]
pub use self::verdict::{
    CaseFailure, CaseReport, ConsensusProperty, CorpusOutcome, FailureReason, Oracle, ReportNote,
    Subject,
};
