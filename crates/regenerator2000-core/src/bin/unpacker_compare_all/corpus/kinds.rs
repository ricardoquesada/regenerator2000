//! Closed enums of the manifest and the predicates that define each
//! divergence kind.
//!
//! The predicates are shared by the oracle evaluation and the classifier, so
//! a proposal that the compare tool makes is exactly one the corpus test
//! accepts.

use std::fmt;

use regenerator2000_core::state::types::System;
use regenerator2000_core::unpacker::machine::get_basic_start;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use super::UNP64_TOP_PAGE;
use super::values::{HexAddr, ManifestStr, deserialize_manifest_str};

// =============================================================================
// Field
// =============================================================================

/// An output field that an `AcceptedDivergence` may permit to differ.
///
/// Only `start`, `end` and `entry` exist. `len` is derived from the range,
/// and the payload over the overlap must always match, so neither can be
/// listed (ruling R1b).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Field {
    /// The first unpacked address.
    Start,
    /// The last unpacked address (inclusive).
    End,
    /// The entry point of the unpacked program.
    Entry,
}

impl Field {
    /// Every field, in canonical order.
    pub const ALL: [Field; 3] = [Field::Start, Field::End, Field::Entry];

    /// The manifest name of the field.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::End => "end",
            Self::Entry => "entry",
        }
    }

    /// The allowed names, for error messages: `` `start`, `end`, `entry` ``.
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
        // Name the two tempting-but-forbidden values explicitly, so the error
        // explains the rule instead of only listing the alternatives.
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

/// Formats fields as `start, end`.
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

/// A non-empty, duplicate-free set of [`Field`]s, kept in canonical order.
///
/// # Example
///
/// `fields = ["end", "start"]` parses to `[Start, End]` and is written back
/// as `["start", "end"]`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldSet(Vec<Field>);

impl FieldSet {
    /// Builds a set from `fields`.
    ///
    /// # Errors
    ///
    /// Returns a message when `fields` is empty or lists a field twice.
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

    /// Returns `true` if `field` is in the set.
    #[must_use]
    pub fn contains(&self, field: Field) -> bool {
        self.0.contains(&field)
    }

    /// The fields in canonical order.
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

/// Why R2000 deliberately differs from unp64 on a case (closed enum shared
/// with the compare tool's classifier).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DivergenceKind {
    /// The starts are the BASIC link byte pair `{basic_start - 1,
    /// basic_start}` (the padding byte, ruling R16).
    PaddingByte,
    /// unp64 had no end scanner and saved up to its top-page default.
    Unp64DefaultEnd,
    /// unp64 used its default start `min(load_addr, ram_start)`.
    Unp64DefaultStart,
    /// unp64 stopped on an opcode it does not implement.
    Unp64MissingOpcode,
    /// The difference is accepted but nobody has investigated it yet. Every
    /// report lists these cases.
    Unexplained,
}

impl DivergenceKind {
    /// The manifest name of the kind.
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

    /// The exact `fields` list that this kind requires, or `None` if any
    /// non-empty list is allowed.
    ///
    /// `unp64_missing_opcode` means unp64 produced no output at all, so every
    /// field differs and all three must be listed.
    #[must_use]
    pub fn required_fields(self) -> Option<&'static [Field]> {
        match self {
            Self::PaddingByte | Self::Unp64DefaultStart => Some(&[Field::Start]),
            Self::Unp64DefaultEnd => Some(&[Field::End]),
            Self::Unp64MissingOpcode => Some(&Field::ALL),
            Self::Unexplained => None,
        }
    }

    /// Returns `true` if this kind may describe a case on which unp64 failed.
    #[must_use]
    pub fn allowed_for_failed_unp64(self) -> bool {
        matches!(self, Self::Unp64MissingOpcode | Self::Unexplained)
    }

    /// Returns `true` if this kind only makes sense when unp64 failed.
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

/// Why a case is expected to fail its oracle until a planned change lands.
///
/// Each reason masks only its own, closed set of failures
/// (`ExpectedFailReason::masks`); every other failure of a marked case still
/// fails the run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedFailReason {
    /// The start differs from the original only by the padding convention:
    /// R2000 drops the original's first byte, one below the BASIC start
    /// (`$0801` versus the original `$0800`). Dali carried this marker until
    /// its start was fixed (M1.4); no manifest case uses it today.
    PaddingConvention,
    /// R2000's payload differs from unp64's over the shared range, and the
    /// cause is being investigated (ruling R17). Masks only payload and
    /// overlap hash mismatches.
    PayloadMismatchUnderInvestigation,
}

impl ExpectedFailReason {
    /// The manifest name of the reason.
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
// Kind predicates (shared by the oracle and the classifier)
// =============================================================================

/// The BASIC starts that may anchor the `padding_byte` pair.
///
/// 1. `system.default_basic_start()`: where a BASIC program lives on the
///    system when its stub was relocated there (C64: `$0801`), whatever the
///    load address. unp64 and R2000 may each report either side of that
///    stub's link byte.
/// 2. [`get_basic_start`]`(load_addr)`: the BASIC start the engine actually
///    used for this load (incl. the `$07C0` Card Cruncher case). This is the
///    engine's own function, the single source of truth, so the classifier
///    cannot drift from the engine.
///
/// On the C64, the only corpus system, this set is identical to the one
/// M1.2 used.
fn basic_start_candidates(load_addr: u16, system: &System) -> [u16; 2] {
    [system.default_basic_start(), get_basic_start(load_addr)]
}

/// `padding_byte` (ruling R16): the two starts are exactly the BASIC link
/// byte pair `{basic_start - 1, basic_start}`, in either order.
///
/// `basic_start` is either candidate of [`basic_start_candidates`]; either
/// one may anchor the pair. Any other one-byte difference is not the padding
/// byte.
///
/// # Example
///
/// On the C64 with a load at `$0801`: `$0800` vs `$0801` holds in both
/// orders; `$0802` vs `$0801` and `$1000` vs `$1001` do not. With a load at
/// `$07C0`, `$07C0` vs `$07C1` holds.
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

/// The BASIC link byte pairs that satisfy `padding_byte` for this load, as
/// `$0800/$0801` (deduplicated, ascending), for messages.
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

/// `unp64_default_end` (ruling R1): unp64's end is above R2000's and on the
/// top page ([`UNP64_TOP_PAGE`]), i.e. unp64 fell back to its default instead
/// of a real end.
///
/// # Example
///
/// Traveller's unp64 end `$FFFD` against R2000's `$7949` holds; `$FEFF` does
/// not.
pub(super) fn unp64_default_end_holds(unp64_end: u16, r2000_end: u16) -> bool {
    unp64_end > r2000_end && unp64_end >= UNP64_TOP_PAGE
}

/// `unp64_default_start`: unp64's start is its default
/// `min(load_addr, system.ram_start())` (`unp64.c:562`) and differs from
/// R2000's. There is no `ret_addr` term (Orchestrator ruling on T1.A1.39).
///
/// # Example
///
/// On the C64 (RAM start `$0800`) with a load at `$0801`, an unp64 start of
/// `$0800` against R2000's `$0810` holds; `$0801` does not.
pub(super) fn unp64_default_start_holds(
    unp64_start: u16,
    r2000_start: u16,
    load_addr: u16,
    system: &System,
) -> bool {
    unp64_start == load_addr.min(system.ram_start()) && r2000_start != unp64_start
}
