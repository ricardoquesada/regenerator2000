//! Shared model and oracle logic for the unpacker corpus.
//!
//! The corpus manifest (`tests/6502/unpack_corpus.toml`) lists every packed
//! fixture the unpacker must handle, together with the result that **unp64**
//! produced for it (recorded once by `unpacker_compare_all --update`, so CI
//! never needs the unp64 binary). The machine-written baseline
//! (`tests/6502/unpack_baseline.toml`) freezes every observable output of
//! Regenerator 2000 (R2000) so that refactors can be checked for a zero diff.
//!
//! # Module map
//!
//! | Module | Responsibility |
//! | :-- | :-- |
//! | `values` | Text-form scalars ([`HexAddr`], [`Fnv64`], [`Unp64Entry`]), [`AddrRange`] and [`fnv1a64`] |
//! | `kinds` | Closed manifest enums ([`Field`], [`FieldSet`], [`DivergenceKind`], [`ExpectedFailReason`]) and the kind predicates |
//! | `manifest` | The manifest model ([`Corpus`], [`Case`], [`Group`], [`Unp64Record`], [`AcceptedDivergence`]), parsing and the deterministic writer |
//! | `validate` | [`Corpus::validate`]: cross-references, fixtures and record consistency |
//! | `error` | [`CorpusError`] |
//! | `fixtures` | [`FixtureResolver`] and the directory-backed [`DirFixtures`] |
//! | `result` | R2000 results as plain data ([`CaseResult`], [`UnpackedImage`], [`R2000Error`]) |
//! | `verdict` | Evaluation outcomes ([`CaseFailure`], [`FailureReason`], [`CaseReport`], [`ReportNote`]) |
//! | `oracle` | The oracle rules ([`Evaluator`], [`GroundTruth`]) |
//! | `classify` | The divergence classifier ([`classify()`], [`Divergence`]) used to propose exceptions |
//! | `filter` | Case selection ([`CorpusFilter`]) |
//! | `report` | Human-readable rendering of failures and action items |
//! | `baseline` | The frozen-output snapshot ([`Baseline`]) with its writer and comparison |
//! | `runner` | Running R2000 on cases ([`run_case`], [`run_cases`]) with each case's budget |
//!
//! # Oracle rules
//!
//! Every case is evaluated by exactly one primary rule:
//!
//! | Rule | Applies when | R2000 must |
//! | :-- | :-- | :-- |
//! | Ground truth | the case is in a [`Group`] | equal the original's range, bytes, and the group entry |
//! | unp64 reference | no group, no exception | equal unp64's start, end, entry and payload hash |
//! | Accepted divergence | no group, `[case.accepted_divergence]` | differ **only** in the listed fields, satisfy the `kind` predicate, and match unp64's bytes over the overlap |
//!
//! Consensus is an extra invariant: all members of a group that are not marked
//! `expected_fail` must agree on range, entry and payload. There is no third
//! state: a case either passes its rule or it is a failure.
//!
//! # Purity
//!
//! Nothing in this module reads environment variables, and the only file
//! system access lives in [`Corpus::load`], [`Baseline::load`] and
//! [`DirFixtures`]. All logic is testable in memory, which matters because
//! the unit tests run once per including crate (ruling R10).

// This module is compiled into two crates: the `unpacker_compare_all` bin (as
// `mod corpus`) and the `unpack_corpus` integration test (via `#[path]`). Each
// crate uses a different subset of the API, so unused items are expected in
// both and must not fail `-D warnings`.
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

// The re-export below is the module's public surface. For the same two-crate
// reason as `dead_code` above, each including crate leaves part of it unused,
// so `unused_imports` is allowed on this one statement only (not module-wide,
// so a stray import inside a submodule still fails the build).
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

/// Directory that holds every fixture, the manifest and the baseline.
///
/// Resolved from the core crate's manifest directory at compile time, so the
/// compare tool and the corpus test find it from any working directory
/// (`cargo run` keeps the caller's directory, `cargo test` uses the package
/// directory).
pub const FIXTURE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/6502");

/// File name of the corpus manifest inside [`FIXTURE_DIR`].
pub const MANIFEST_FILE: &str = "unpack_corpus.toml";

/// File name of the baseline inside [`FIXTURE_DIR`].
pub const BASELINE_FILE: &str = "unpack_baseline.toml";

/// `reviewed_by` value for every exception that the initial classification
/// recorded and that the user has not re-signed yet.
///
/// It counts as signed (so CI stays green), but every report lists the case
/// as a user action item. It deliberately does not impersonate the user.
pub const INITIAL_REVIEW_MARKER: &str = "initial-classification-2026-10-28 (pending ricardoq)";

/// Lowest end address that unp64 produces when no end scanner fired
/// (ruling R1).
///
/// unp64 has no write tracking. Without a scanner-provided end it saves up to
/// `$FFFF` (or a few bytes below, e.g. `$FFFD` for traveller), so an unp64 end
/// on the top page is the signature of its default, not of a real boundary.
pub const UNP64_TOP_PAGE: u16 = 0xFF00;

/// Instruction budget for cases without `max_instructions`.
///
/// Equal to `UnpackConfig::default().max_instructions` (a unit test pins the
/// equality), so a case without an explicit budget runs exactly like a
/// default unpack.
pub const DEFAULT_MAX_INSTRUCTIONS: u64 = 50_000_000;
