//! Pure unit tests for the shared corpus module (Test Plan T1.A1.1–T1.A1.63,
//! TX.6, and the classifier tests T1.A2.20–T1.A2.32 that ship with the
//! classifier per the ledger's Option A).
//!
//! The corpus module is compiled into two crates (the compare bin and the
//! corpus integration test), so these tests run twice. They are deliberately
//! pure: no file system and no environment variables (ruling R10). Fixtures
//! come from the in-memory [`MemFixtures`] resolver and payloads from one
//! deterministic xorshift "memory".
//!
//! This file holds the shared builders; each submodule tests one library
//! module and is named after it.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod baseline;
mod classify;
mod filter;
mod manifest;
mod oracle;
mod report;
mod runner;
mod validate;
mod values;

use super::*;
use regenerator2000_core::state::types::System;
use std::collections::BTreeMap;

/// Size of the 6502 address space in bytes.
const ADDRESS_SPACE: usize = 0x1_0000;

/// unp64 version recorded by every synthetic record.
const UNP64_VERSION: &str = "2.42";

// ─────────────────────────────── bytes ───────────────────────────────

/// Deterministic pseudo-random byte generator (xorshift32), as in
/// `tests/parser_fuzz_test.rs`. Seeds must be non-zero.
fn pseudo_random_bytes(seed: u32, len: usize) -> Vec<u8> {
    let mut state = seed;
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        out.push(state as u8);
    }
    out
}

/// One shared 64 KB "memory". Every image and unp64 observation takes its
/// bytes from here, so two windows over the same addresses always agree.
fn memory() -> Vec<u8> {
    pseudo_random_bytes(0x00C0_FFEE, ADDRESS_SPACE)
}

/// The shared memory's bytes over `start..=end`.
fn mem_bytes(start: u16, end: u16) -> Vec<u8> {
    memory()[usize::from(start)..=usize::from(end)].to_vec()
}

/// The range `start..=end`; panics on an inverted range (test input bug).
fn range(start: u16, end: u16) -> AddrRange {
    AddrRange::new(start, end).unwrap()
}

/// A PRG: little-endian load address followed by the payload.
fn prg(load_addr: u16, payload: &[u8]) -> Vec<u8> {
    let mut bytes = load_addr.to_le_bytes().to_vec();
    bytes.extend_from_slice(payload);
    bytes
}

// ─────────────────────────────── fixtures ───────────────────────────────

/// An in-memory fixture directory.
#[derive(Default)]
struct MemFixtures(BTreeMap<String, Vec<u8>>);

impl MemFixtures {
    /// Fixtures with placeholder bytes for each name.
    fn named(files: &[&str]) -> Self {
        Self(
            files
                .iter()
                .map(|file| ((*file).to_string(), vec![0x01, 0x08, 0x00]))
                .collect(),
        )
    }

    /// Adds (or replaces) one fixture.
    fn with(mut self, file: &str, bytes: Vec<u8>) -> Self {
        self.0.insert(file.to_string(), bytes);
        self
    }
}

impl FixtureResolver for MemFixtures {
    fn exists(&self, file: &str) -> bool {
        self.0.contains_key(file)
    }

    fn read(&self, file: &str) -> Result<Vec<u8>, CorpusError> {
        self.0
            .get(file)
            .cloned()
            .ok_or_else(|| CorpusError::MissingFixture {
                file: file.to_string(),
            })
    }
}

// ─────────────────────────────── R2000 results ───────────────────────────────

/// An R2000 image over `start..=end` with bytes from the shared memory.
fn image(start: u16, end: u16, entry: u16) -> UnpackedImage {
    use regenerator2000_core::unpacker::{BoundSource, ExitReason, UnpackReport};
    UnpackedImage {
        start,
        end,
        entry,
        dep: 0x0100,
        packer: Some("Test Packer".to_string()),
        instructions: 12_345,
        data: mem_bytes(start, end),
        report: UnpackReport {
            exit_reason: ExitReason::WrittenCodeReached { pc: entry },
            find_depacker_instructions: 345,
            decompress_instructions: 12_000,
            start_source: BoundSource::WriteDiff,
            end_source: BoundSource::WriteDiff,
            entry_source: BoundSource::ExitPc,
            trace_tail: Vec::new(),
        },
    }
}

/// A successful C64 result loaded at `$0801`.
fn ok_result(file: &str, image: UnpackedImage) -> CaseResult {
    CaseResult {
        file: file.to_string(),
        load_addr: 0x0801,
        system: System::C64,
        outcome: Ok(image),
    }
}

/// A failed result with the given `UnpackError` variant name.
fn err_result(file: &str, variant: &str) -> CaseResult {
    CaseResult {
        file: file.to_string(),
        load_addr: 0x0801,
        system: System::C64,
        outcome: Err(R2000Error {
            variant: variant.to_string(),
            message: format!("{variant} happened"),
        }),
    }
}

// ─────────────────────────────── manifest model ───────────────────────────────

/// An unp64 record over `start..=end` with bytes from the shared memory.
/// When the range differs from `r2000`'s, the overlap window is the
/// intersection, hashed over the shared memory (unp64 agrees on the overlap).
fn record(start: u16, end: u16, entry: Unp64Entry, r2000: &UnpackedImage) -> Unp64Saved {
    let unp64_range = range(start, end);
    let r2000_range = range(r2000.start, r2000.end);
    let overlap = (unp64_range != r2000_range)
        .then(|| unp64_range.intersect(r2000_range))
        .flatten()
        .map(|window| Overlap {
            start: HexAddr(window.start()),
            end: HexAddr(window.end()),
            fnv64: Fnv64::of(&mem_bytes(window.start(), window.end())),
        });
    Unp64Saved {
        start: HexAddr(start),
        end: HexAddr(end),
        entry,
        len: u32::try_from(unp64_range.len()).unwrap(),
        payload_fnv64: Fnv64::of(&mem_bytes(start, end)),
        overlap,
        version: UNP64_VERSION.to_string(),
    }
}

/// An unp64 record that matches `image` exactly.
fn matching_record(image: &UnpackedImage) -> Unp64Saved {
    record(
        image.start,
        image.end,
        Unp64Entry::Address(image.entry),
        image,
    )
}

/// A failed unp64 record.
fn failed_record(reason: &str) -> Unp64Record {
    Unp64Record::Failed(Unp64Failed {
        reason: reason.to_string(),
        version: UNP64_VERSION.to_string(),
    })
}

/// A case outside any group with a saved unp64 record.
fn case(file: &str, saved: Unp64Saved, divergence: Option<AcceptedDivergence>) -> Case {
    Case {
        file: file.to_string(),
        group: None,
        max_instructions: None,
        expected_fail: None,
        unp64: Some(Unp64Record::Saved(saved)),
        accepted_divergence: divergence,
    }
}

/// An exception with the given review signature.
fn divergence(
    kind: DivergenceKind,
    fields: &[Field],
    reviewed_by: Option<&str>,
) -> AcceptedDivergence {
    AcceptedDivergence {
        kind,
        fields: FieldSet::new(fields.iter().copied()).unwrap(),
        reviewed_by: reviewed_by.map(str::to_string),
        note: None,
    }
}

/// An exception signed by a reviewer.
fn signed(kind: DivergenceKind, fields: &[Field]) -> AcceptedDivergence {
    divergence(kind, fields, Some("ricardoq"))
}

/// A corpus without groups.
fn corpus(cases: Vec<Case>) -> Corpus {
    Corpus {
        groups: Vec::new(),
        cases,
    }
}

/// A valid saved record over `$0801-$0810`.
fn simple_record() -> Unp64Saved {
    matching_record(&image(0x0801, 0x0810, 0x080D))
}

/// A plain case with [`simple_record`].
fn simple_case(file: &str) -> Case {
    case(file, simple_record(), None)
}

/// The `mule` group with original `mule_orig.prg`.
fn mule_group() -> Group {
    Group {
        id: "mule".to_string(),
        original: "mule_orig.prg".to_string(),
        entry: HexAddr(0x1100),
    }
}

// ─────────────────────────────── evaluation ───────────────────────────────

/// The reasons of a failure list, without subjects.
fn reasons(failures: Vec<CaseFailure>) -> Vec<FailureReason> {
    failures.into_iter().map(|failure| failure.reason).collect()
}

/// Evaluates one result against a corpus without groups.
fn eval(corpus: &Corpus, result: &CaseResult) -> Result<CaseReport, Vec<FailureReason>> {
    let evaluator = Evaluator::new(corpus, &MemFixtures::default()).unwrap();
    evaluator.evaluate(result).map_err(reasons)
}

/// All failure messages, one per line.
fn failure_texts(failures: &[FailureReason]) -> String {
    failures
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

// ─────────────────────────────── manifest text ───────────────────────────────

/// A minimal valid manifest; `unp64_extra` is appended inside `[case.unp64]`.
fn manifest_with_unp64(unp64_extra: &str) -> String {
    format!(
        "[[case]]\n\
         file = \"a.prg\"\n\
         [case.unp64]\n\
         start = \"$0801\"\n\
         end = \"$0810\"\n\
         entry = \"$080D\"\n\
         len = 16\n\
         payload_fnv64 = \"cbf29ce484222325\"\n\
         version = \"2.42\"\n\
         {unp64_extra}\n"
    )
}

/// A manifest whose only case has a `[case.accepted_divergence]` table.
fn manifest_with_divergence(divergence_body: &str) -> String {
    format!(
        "{}[case.accepted_divergence]\n{divergence_body}\n",
        manifest_with_unp64("")
    )
}

/// The parse error message of `toml_text`; panics if it parses.
fn parse_error(toml_text: &str) -> String {
    match Corpus::parse(toml_text) {
        Err(CorpusError::Parse { message }) => message,
        other => panic!("expected a parse error, got {other:?}\n--- input ---\n{toml_text}"),
    }
}
