//! Plain-data R2000 unpack results for oracle evaluation and unit tests.

use regenerator2000_core::state::types::System;
use regenerator2000_core::unpacker::{UnpackError, UnpackReport, UnpackResult};

use super::values::{AddrRange, Addressed, Fnv64};

/// A successful R2000 unpack with the values that the oracles check.
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
    /// Name of the detected packer.
    pub packer: Option<String>,
    /// Instructions executed across both emulation stages.
    pub instructions: u64,
    /// Payload bytes (`start..=end`).
    pub data: Vec<u8>,
    /// Provenance report for the unpack run.
    pub report: UnpackReport,
}

impl UnpackedImage {
    /// Returns the unpacked range, or `None` if `end < start` or the payload
    /// length does not match the range.
    #[must_use]
    pub fn range(&self) -> Option<AddrRange> {
        AddrRange::new(self.start, self.end).filter(|range| range.len() == self.data.len())
    }

    /// Returns the FNV-1a-64 hash of the payload.
    #[must_use]
    pub fn fnv64(&self) -> Fnv64 {
        Fnv64::of(&self.data)
    }

    /// Returns the payload at its start address.
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
    /// The `UnpackError` variant name (for example `Phase2Timeout`).
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
/// `UnpackError` is `#[non_exhaustive]`, so this function extracts the name
/// from the `Debug` output instead of an exhaustive `match`.
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
    /// The fixture file name of the case.
    pub file: String,
    /// Load address from the fixture PRG header.
    pub load_addr: u16,
    /// Target system for the unpack run.
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
