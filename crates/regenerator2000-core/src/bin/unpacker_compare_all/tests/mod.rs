//! Unit tests of the compare tool (Test Plan T1.A2.1–T1.A2.19 and
//! T1.A2.33–T1.A2.46; T1.A2.45 lives with the shared runner in the corpus
//! module). Everything here is pure: captured unp64 output is stored as
//! `const` strings, and payloads come from one deterministic "memory".
//!
//! This file holds the shared builders; each submodule tests one module of
//! the tool and is named after it.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod check;
mod cli;
mod markdown;
mod unp64;
mod update;

use regenerator2000_core::state::types::System;
use regenerator2000_core::unpacker::{BoundSource, ExitReason, UnpackReport};

use crate::corpus::{
    AcceptedDivergence, AddrRange, Case, CaseResult, DivergenceKind, Field, FieldSet, R2000Error,
    Unp64Entry, UnpackedImage,
};
use crate::observation::Observation;
use crate::unp64::Unp64Outcome;
use crate::unp64::parse::SavedPayload;

/// Size of the 6502 address space in bytes.
const ADDRESS_SPACE: usize = 0x1_0000;

/// unp64 version used by every synthetic run.
const VERSION: &str = "2.42";

/// One shared 64 KB "memory" (xorshift32, as in `tests/parser_fuzz_test.rs`).
/// Every R2000 image and unp64 payload takes its bytes from here, so two
/// windows over the same addresses always agree.
fn memory() -> Vec<u8> {
    let mut state: u32 = 0x00C0_FFEE;
    (0..ADDRESS_SPACE)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as u8
        })
        .collect()
}

/// The shared memory's bytes over `start..=end`.
fn mem_bytes(start: u16, end: u16) -> Vec<u8> {
    memory()[usize::from(start)..=usize::from(end)].to_vec()
}

/// The range `start..=end`.
fn range(start: u16, end: u16) -> AddrRange {
    AddrRange::new(start, end).unwrap()
}

/// An R2000 image over `start..=end`.
fn image(start: u16, end: u16, entry: u16) -> UnpackedImage {
    UnpackedImage {
        start,
        end,
        entry,
        dep: 0x0100,
        packer: Some("Test Packer".to_string()),
        instructions: 1_000,
        data: mem_bytes(start, end),
        report: UnpackReport {
            exit_reason: ExitReason::WrittenCodeReached { pc: entry },
            find_depacker_instructions: 100,
            decompress_instructions: 900,
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

/// A failed R2000 result.
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

/// unp64 saved `start..=end` from the shared memory.
fn saved(start: u16, end: u16, entry: Unp64Entry) -> Unp64Outcome {
    Unp64Outcome::Saved {
        saved: SavedPayload {
            range: range(start, end),
            payload: mem_bytes(start, end),
        },
        entry,
        packer: None,
    }
}

/// unp64 saved nothing.
fn failed(reason: &str) -> Unp64Outcome {
    Unp64Outcome::Failed {
        reason: reason.to_string(),
        missing_opcode: false,
    }
}

/// A case without oracle data.
fn bare_case(file: &str) -> Case {
    Case {
        file: file.to_string(),
        group: None,
        max_instructions: None,
        expected_fail: None,
        unp64: None,
        accepted_divergence: None,
    }
}

/// An exception with the given signature.
fn exception(
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

/// An observation of `case`.
fn observation(case: Case, result: CaseResult, unp64: Unp64Outcome) -> Observation {
    Observation {
        case,
        result,
        unp64,
    }
}
