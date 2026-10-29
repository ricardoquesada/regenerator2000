//! R2000 unpack results reduced to plain data, so the oracles can be fed
//! synthetic results in unit tests.

use regenerator2000_core::state::types::System;
use regenerator2000_core::unpacker::{UnpackError, UnpackReport, UnpackResult};

use super::values::{AddrRange, Addressed, Fnv64};

/// A successful R2000 unpack, reduced to the values the oracles check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnpackedImage {
    /// First unpacked address.
    pub start: u16,
    /// Last unpacked address (inclusive).
    pub end: u16,
    /// Entry point.
    pub entry: u16,
    /// Depacker address.
    pub dep: u16,
    /// Display name of the detected packer.
    pub packer: Option<String>,
    /// Instructions executed across both emulation stages.
    pub instructions: u64,
    /// Payload bytes, `start..=end`.
    pub data: Vec<u8>,
    /// Structured provenance report for the unpack run.
    pub report: UnpackReport,
}

impl UnpackedImage {
    /// The unpacked range, or `None` if the result is inconsistent
    /// (`end < start`, or a payload length that does not match the range).
    #[must_use]
    pub fn range(&self) -> Option<AddrRange> {
        AddrRange::new(self.start, self.end).filter(|range| range.len() == self.data.len())
    }

    /// FNV-1a-64 of the payload.
    #[must_use]
    pub fn fnv64(&self) -> Fnv64 {
        Fnv64::of(&self.data)
    }

    /// The payload anchored at its start address.
    pub(super) fn addressed(&self) -> Addressed<'_> {
        Addressed {
            start: self.start,
            data: &self.data,
        }
    }
}

impl From<&UnpackResult> for UnpackedImage {
    fn from(result: &UnpackResult) -> Self {
        Self {
            start: result.start_addr,
            end: result.end_addr,
            entry: result.entry_point,
            dep: result.dep_addr,
            packer: result.packer_name.clone(),
            instructions: result.instructions_executed,
            data: result.data.clone(),
            report: result.report.clone(),
        }
    }
}

/// A failed R2000 unpack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct R2000Error {
    /// The `UnpackError` variant name, e.g. `Phase2Timeout`.
    pub variant: String,
    /// The full error message.
    pub message: String,
}

impl From<&UnpackError> for R2000Error {
    fn from(error: &UnpackError) -> Self {
        Self {
            variant: unpack_error_variant(error),
            message: error.to_string(),
        }
    }
}

/// Returns the variant name of an [`UnpackError`] (for example
/// `InvalidAddressRange`), without its fields.
///
/// `UnpackError` is `#[non_exhaustive]`, so the name is taken from the derived
/// `Debug` form instead of an exhaustive `match` that new variants would break.
#[must_use]
pub fn unpack_error_variant(error: &UnpackError) -> String {
    format!("{error:?}")
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect()
}

/// The outcome of running R2000 on one corpus case.
#[derive(Debug, Clone)]
pub struct CaseResult {
    /// The case's fixture file name.
    pub file: String,
    /// Load address from the fixture's PRG header.
    pub load_addr: u16,
    /// The target system the unpack ran for.
    pub system: System,
    /// The unpacked image, or the error.
    pub outcome: Result<UnpackedImage, R2000Error>,
}

impl CaseResult {
    /// Builds a result from a call to `unpacker::unpack`.
    #[must_use]
    pub fn from_unpack(
        file: impl Into<String>,
        load_addr: u16,
        system: System,
        result: &Result<UnpackResult, UnpackError>,
    ) -> Self {
        Self {
            file: file.into(),
            load_addr,
            system,
            outcome: result
                .as_ref()
                .map(UnpackedImage::from)
                .map_err(R2000Error::from),
        }
    }
}
