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
pub use self::{
    baseline::{
        Baseline, BaselineDiff, BaselineField, BaselineOutcome, BaselineRow, BaselineUnpacked,
    },
    classify::{Divergence, Unp64Observation, classify},
    error::CorpusError,
    filter::CorpusFilter,
    fixtures::{DirFixtures, FixtureResolver},
    kinds::{DivergenceKind, ExpectedFailReason, Field, FieldSet},
    manifest::{
        AcceptedDivergence, Case, Corpus, Group, Overlap, ReviewState, Unp64Failed, Unp64Record,
        Unp64Saved,
    },
    oracle::{Evaluator, GroundTruth},
    report::{render_action_items, render_failure_table},
    result::{CaseResult, R2000Error, UnpackedImage, unpack_error_variant},
    runner::{Schedule, run_case, run_cases, unpack_config_for},
    values::{AddrRange, Fnv64, HexAddr, Unp64Entry, ValueFormatError, fnv1a64, hash_window},
    verdict::{
        CaseFailure, CaseReport, ConsensusProperty, CorpusOutcome, FailureReason, Oracle,
        ReportNote, Subject,
    },
};

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
