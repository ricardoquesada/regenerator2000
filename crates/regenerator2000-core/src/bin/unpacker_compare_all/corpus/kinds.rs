//! Closed manifest enums and divergence kind predicates.
//!
//! Both the oracle evaluator and the classifier share these predicates so the
//! corpus test accepts every proposal from the compare tool.

use std::fmt;

use regenerator2000_core::state::types::System;
use regenerator2000_core::unpacker::machine::get_basic_start;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use super::UNP64_TOP_PAGE;
use super::values::{HexAddr, ManifestStr, deserialize_manifest_str};

// =============================================================================
// Field
// =============================================================================

/// Output field that an `AcceptedDivergence` can allow to differ.
///
/// Only `start`, `end`, and `entry` exist. `len` derives from the address
/// range, and the overlapping payload bytes must always match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Field {
    /// First unpacked address.
    Start,
    /// Inclusive last unpacked address.
    End,
    /// Entry point of the unpacked program.
    Entry,
}

impl Field {
    /// Every field in canonical order.
    pub const ALL: [Field; 3] = [Field::Start, Field::End, Field::Entry];

    /// Returns the manifest name of the field.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::End => "end",
            Self::Entry => "entry",
        }
    }

    /// Formats the allowed field names as `` `start`, `end`, `entry` ``.
    fn allowed_list() -> String {
        Self::ALL
            .iter()
            .map(|field| format!("`{}`", field.as_str()))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl ManifestStr for Field {
    const EXPECTING: &'static str = "one of \"start\", \"end\", \"entry\"";

    fn parse_manifest(text: &str) -> Result<Self, String> {
        if let Some(field) = Self::ALL.into_iter().find(|f| f.as_str() == text) {
            return Ok(field);
        }
        let allowed = Self::allowed_list();
        // Explain why `len`, `payload`, `payload_fnv64`, and `overlap` cannot
        // appear in `fields`.
        Err(match text {
            "len" => format!(
                "`len` cannot be listed in `fields`: it is derived from the address range. \
                 Allowed values: {allowed}"
            ),
            "payload" | "payload_fnv64" | "overlap" => format!(
                "`{text}` cannot be listed in `fields`: the overlap payload must always match. \
                 Allowed values: {allowed}"
            ),
            other => format!("unknown field `{other}` in `fields`. Allowed values: {allowed}"),
        })
    }
}

impl Serialize for Field {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Field {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserialize_manifest_str(deserializer)
    }
}

/// Formats `fields` as `start, end`.
pub(super) fn join_fields(fields: &[Field]) -> String {
    fields
        .iter()
        .map(|field| field.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

// =============================================================================
// FieldSet
// =============================================================================

/// Non-empty, duplicate-free set of [`Field`] values in canonical order.
///
/// # Example
///
/// `fields = ["end", "start"]` parses to `[Start, End]` and serializes as
/// `["start", "end"]`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldSet(Vec<Field>);

impl FieldSet {
    /// Builds a set from `fields`.
    ///
    /// # Errors
    ///
    /// Returns an error message when `fields` is empty or contains duplicates.
    pub fn new(fields: impl IntoIterator<Item = Field>) -> Result<Self, String> {
        let mut sorted: Vec<Field> = fields.into_iter().collect();
        if sorted.is_empty() {
            return Err(format!(
                "`fields` must list at least one of {}",
                Field::allowed_list()
            ));
        }
        sorted.sort_unstable();
        if let Some(pair) = sorted.windows(2).find(|pair| pair[0] == pair[1]) {
            return Err(format!("`fields` lists `{}` more than once", pair[0]));
        }
        Ok(Self(sorted))
    }

    /// Returns `true` if `self` contains `field`.
    #[must_use]
    pub fn contains(&self, field: Field) -> bool {
        self.0.contains(&field)
    }

    /// Returns the fields in canonical order.
    #[must_use]
    pub fn as_slice(&self) -> &[Field] {
        &self.0
    }
}

impl fmt::Display for FieldSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&join_fields(&self.0))
    }
}

impl Serialize for FieldSet {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for FieldSet {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let fields = Vec::<Field>::deserialize(deserializer)?;
        Self::new(fields).map_err(de::Error::custom)
    }
}

// =============================================================================
// DivergenceKind
// =============================================================================

/// Reason why R2000 intentionally differs from `unp64` on a case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DivergenceKind {
    /// The start addresses form the BASIC link byte pair `{basic_start - 1,
    /// basic_start}`.
    PaddingByte,
    /// `unp64` had no end scanner and saved up to its top-page default.
    Unp64DefaultEnd,
    /// `unp64` selected its default start `min(load_addr, ram_start)`.
    Unp64DefaultStart,
    /// `unp64` stopped on an unimplemented opcode.
    Unp64MissingOpcode,
    /// Accepted difference that awaits further investigation.
    Unexplained,
}

impl DivergenceKind {
    /// Returns the manifest name of the kind.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PaddingByte => "padding_byte",
            Self::Unp64DefaultEnd => "unp64_default_end",
            Self::Unp64DefaultStart => "unp64_default_start",
            Self::Unp64MissingOpcode => "unp64_missing_opcode",
            Self::Unexplained => "unexplained",
        }
    }

    /// Returns the exact `fields` slice that this kind requires, or `None` if
    /// any non-empty field set is valid.
    #[must_use]
    pub fn required_fields(self) -> Option<&'static [Field]> {
        match self {
            Self::PaddingByte | Self::Unp64DefaultStart => Some(&[Field::Start]),
            Self::Unp64DefaultEnd => Some(&[Field::End]),
            Self::Unp64MissingOpcode => Some(&Field::ALL),
            Self::Unexplained => None,
        }
    }

    /// Returns `true` if this kind can describe a case where `unp64` failed.
    #[must_use]
    pub fn allowed_for_failed_unp64(self) -> bool {
        matches!(self, Self::Unp64MissingOpcode | Self::Unexplained)
    }

    /// Returns `true` if this kind requires a failed `unp64` record.
    #[must_use]
    pub fn requires_failed_unp64(self) -> bool {
        matches!(self, Self::Unp64MissingOpcode)
    }
}

impl fmt::Display for DivergenceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// =============================================================================
// ExpectedFailReason
// =============================================================================

/// Reason why a case is expected to fail its oracle until a fix lands.
///
/// Each reason masks only its matching failure variants in
/// `ExpectedFailReason::masks`. Any other failure still fails the test run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedFailReason {
    /// The start differs from the original by the one-byte padding convention
    /// (`basic_start` versus `basic_start - 1`).
    PaddingConvention,
    /// The R2000 payload differs from `unp64` over the shared range and is
    /// under investigation.
    PayloadMismatchUnderInvestigation,
}

impl ExpectedFailReason {
    /// Returns the manifest name of the reason.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PaddingConvention => "padding_convention",
            Self::PayloadMismatchUnderInvestigation => "payload_mismatch_under_investigation",
        }
    }
}

impl fmt::Display for ExpectedFailReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// =============================================================================
// Kind predicates
// =============================================================================

/// Returns the candidate BASIC start addresses for `padding_byte`:
///
/// 1. `system.default_basic_start()`: the default system BASIC start address.
/// 2. [`get_basic_start`]`(load_addr)`: the BASIC start address that the
///    unpacker engine computes from `load_addr`.
fn basic_start_candidates(load_addr: u16, system: &System) -> [u16; 2] {
    [system.default_basic_start(), get_basic_start(load_addr)]
}

/// Returns `true` if `unp64_start` and `r2000_start` form the BASIC link byte
/// pair `{basic_start - 1, basic_start}` in either order.
///
/// # Example
///
/// When `system.default_basic_start()` is `$0801` and `load_addr` is `$0801`,
/// `$0800` and `$0801` match in either order, whereas `$0802` and `$0801` do
/// not.
pub(super) fn padding_byte_holds(
    unp64_start: u16,
    r2000_start: u16,
    load_addr: u16,
    system: &System,
) -> bool {
    let (low, high) = (unp64_start.min(r2000_start), unp64_start.max(r2000_start));
    basic_start_candidates(load_addr, system)
        .into_iter()
        .any(|basic_start| high == basic_start && basic_start.checked_sub(1) == Some(low))
}

/// Formats the candidate BASIC link byte pairs for `load_addr` and `system`
/// in ascending order.
pub(super) fn padding_byte_pairs(load_addr: u16, system: &System) -> String {
    let mut starts = basic_start_candidates(load_addr, system).to_vec();
    starts.sort_unstable();
    starts.dedup();
    starts
        .into_iter()
        .filter_map(|basic_start| {
            let padding = basic_start.checked_sub(1)?;
            Some(format!("{}/{}", HexAddr(padding), HexAddr(basic_start)))
        })
        .collect::<Vec<_>>()
        .join(" or ")
}

/// Returns `true` if `unp64_end` is above `r2000_end` and at or above
/// [`UNP64_TOP_PAGE`].
///
/// # Example
///
/// `unp64_end = $FFFD` and `r2000_end = $7949` return `true`. `unp64_end =
/// $FEFF` returns `false`.
pub(super) fn unp64_default_end_holds(unp64_end: u16, r2000_end: u16) -> bool {
    unp64_end > r2000_end && unp64_end >= UNP64_TOP_PAGE
}

/// Returns `true` if `unp64_start` equals `min(load_addr, system.ram_start())`
/// (`unp64.c:562`) and differs from `r2000_start`.
///
/// # Example
///
/// When `system.ram_start()` is `$0800` and `load_addr` is `$0801`,
/// `unp64_start = $0800` and `r2000_start = $0810` return `true`.
pub(super) fn unp64_default_start_holds(
    unp64_start: u16,
    r2000_start: u16,
    load_addr: u16,
    system: &System,
) -> bool {
    unp64_start == load_addr.min(system.ram_start()) && r2000_start != unp64_start
}
