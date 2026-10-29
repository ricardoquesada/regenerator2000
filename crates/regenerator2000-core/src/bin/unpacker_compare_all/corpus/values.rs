//! Text-form scalar values of the manifest and baseline, address ranges, and
//! the FNV-1a-64 hash.
//!
//! Every scalar here has exactly one canonical text form, so writing a parsed
//! file reproduces it byte for byte (a second `--update` run is a zero diff).

use std::fmt;
use std::marker::PhantomData;

use serde::de::{self, Deserializer, Visitor};
use serde::{Deserialize, Serialize, Serializer};

/// FNV-1a 64-bit offset basis.
const FNV64_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;

/// FNV-1a 64-bit prime.
const FNV64_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Number of hex digits in the manifest form of an [`Fnv64`].
const FNV64_HEX_DIGITS: usize = 16;

/// Number of hex digits in the manifest form of a [`HexAddr`].
const HEX_ADDR_DIGITS: usize = 4;

/// Manifest token for the BASIC `RUN` return that unp64 prints when it found
/// no `SYS` target (`pass2, return to mem: ($a7ae)->RUN`).
const BASIC_RUN_TOKEN: &str = "RUN";

// =============================================================================
// FNV-1a-64
// =============================================================================

/// Computes the 64-bit FNV-1a hash of `bytes`.
///
/// Used only for non-adversarial change detection of unpacked payloads,
/// always together with the payload length.
///
/// # Example
///
/// `fnv1a64(b"")` is the offset basis `0xcbf29ce484222325`, and
/// `fnv1a64(b"a")` is `0xaf63dc4c8601ec8c`.
#[must_use]
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(FNV64_OFFSET_BASIS, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(FNV64_PRIME)
    })
}

// =============================================================================
// String-only deserialization
// =============================================================================

/// A malformed scalar value in a manifest or baseline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueFormatError {
    /// The value is not a `$`-prefixed, four-digit hex address.
    HexAddr {
        /// The rejected text.
        input: String,
    },
    /// The value is not a 16-digit lowercase hex hash.
    Fnv64 {
        /// The rejected text.
        input: String,
    },
}

impl fmt::Display for ValueFormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HexAddr { input } => write!(
                f,
                "invalid address `{input}`: expected `$` followed by exactly four hex digits, \
                 e.g. \"$0801\""
            ),
            Self::Fnv64 { input } => write!(
                f,
                "invalid hash `{input}`: expected exactly 16 lowercase hex digits without a \
                 prefix, e.g. \"cbf29ce484222325\""
            ),
        }
    }
}

impl std::error::Error for ValueFormatError {}

/// A scalar that has a single canonical string form in the manifest.
///
/// Implementors get `Deserialize` through [`deserialize_manifest_str`], which
/// rejects non-string TOML values (for example a bare integer address) with a
/// message that states the expected form.
pub(super) trait ManifestStr: Sized {
    /// Describes the expected form, used in "invalid type" errors.
    const EXPECTING: &'static str;

    /// Parses the canonical (or accepted) string form.
    fn parse_manifest(text: &str) -> Result<Self, String>;
}

/// Serde visitor that accepts only strings and delegates to [`ManifestStr`].
struct ManifestStrVisitor<T>(PhantomData<T>);

impl<T: ManifestStr> Visitor<'_> for ManifestStrVisitor<T> {
    type Value = T;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(T::EXPECTING)
    }

    fn visit_str<E: de::Error>(self, text: &str) -> Result<T, E> {
        T::parse_manifest(text).map_err(E::custom)
    }
}

/// Deserializes any [`ManifestStr`] from a string value.
pub(super) fn deserialize_manifest_str<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: ManifestStr,
{
    deserializer.deserialize_str(ManifestStrVisitor(PhantomData))
}

// =============================================================================
// HexAddr
// =============================================================================

/// A 6502 address with the manifest text form `"$XXXX"`.
///
/// Parsing accepts upper- and lowercase hex digits; [`fmt::Display`] and the
/// serializer always write four uppercase digits, so a second `--update` run
/// yields a zero diff.
///
/// # Example
///
/// `HexAddr::parse("$e7ff")` gives `HexAddr(0xE7FF)`, which is written back as
/// `"$E7FF"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HexAddr(pub u16);

impl HexAddr {
    /// Parses `"$XXXX"` (exactly four hex digits, any case).
    ///
    /// # Errors
    ///
    /// Returns [`ValueFormatError::HexAddr`] when the `$` prefix is missing,
    /// the digit count is not four, or a digit is not hex.
    pub fn parse(text: &str) -> Result<Self, ValueFormatError> {
        let invalid = || ValueFormatError::HexAddr {
            input: text.to_string(),
        };
        let digits = text.strip_prefix('$').ok_or_else(invalid)?;
        if digits.len() != HEX_ADDR_DIGITS || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(invalid());
        }
        u16::from_str_radix(digits, 16)
            .map(Self)
            .map_err(|_| invalid())
    }
}

impl fmt::Display for HexAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "${:04X}", self.0)
    }
}

impl From<u16> for HexAddr {
    fn from(addr: u16) -> Self {
        Self(addr)
    }
}

impl ManifestStr for HexAddr {
    const EXPECTING: &'static str = "an address string such as \"$0801\"";

    fn parse_manifest(text: &str) -> Result<Self, String> {
        Self::parse(text).map_err(|e| e.to_string())
    }
}

impl Serialize for HexAddr {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for HexAddr {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserialize_manifest_str(deserializer)
    }
}

// =============================================================================
// Fnv64
// =============================================================================

/// An FNV-1a-64 hash with the manifest text form of 16 lowercase hex digits.
///
/// # Example
///
/// `Fnv64::of(b"")` is written as `"cbf29ce484222325"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Fnv64(pub u64);

impl Fnv64 {
    /// Hashes `bytes` with [`fnv1a64`].
    #[must_use]
    pub fn of(bytes: &[u8]) -> Self {
        Self(fnv1a64(bytes))
    }

    /// Parses exactly 16 lowercase hex digits (no `0x` prefix).
    ///
    /// # Errors
    ///
    /// Returns [`ValueFormatError::Fnv64`] for any other length, a prefix, or
    /// an uppercase or non-hex digit. Only the canonical form is accepted, so
    /// the writer is the single source of the hash text.
    pub fn parse(text: &str) -> Result<Self, ValueFormatError> {
        let invalid = || ValueFormatError::Fnv64 {
            input: text.to_string(),
        };
        let canonical = text.len() == FNV64_HEX_DIGITS
            && text
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if !canonical {
            return Err(invalid());
        }
        u64::from_str_radix(text, 16)
            .map(Self)
            .map_err(|_| invalid())
    }
}

impl fmt::Display for Fnv64 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

impl ManifestStr for Fnv64 {
    const EXPECTING: &'static str = "a hash string of 16 lowercase hex digits";

    fn parse_manifest(text: &str) -> Result<Self, String> {
        Self::parse(text).map_err(|e| e.to_string())
    }
}

impl Serialize for Fnv64 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Fnv64 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserialize_manifest_str(deserializer)
    }
}

// =============================================================================
// Unp64Entry
// =============================================================================

/// The entry point that unp64 reported after its second pass.
///
/// unp64 prints either an address or `RUN` when control returned to the
/// BASIC interpreter and it found no `SYS` target. The manifest form is
/// `"$080D"` or `"RUN"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Unp64Entry {
    /// unp64 resolved a concrete entry address.
    Address(u16),
    /// unp64 returned to BASIC `RUN` without resolving a `SYS` address.
    BasicRun,
}

impl Unp64Entry {
    /// Returns `true` if unp64 resolved exactly `addr`. A [`Self::BasicRun`]
    /// entry never matches an address.
    #[must_use]
    pub fn matches(self, addr: u16) -> bool {
        self == Self::Address(addr)
    }
}

impl fmt::Display for Unp64Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Address(addr) => HexAddr(*addr).fmt(f),
            Self::BasicRun => f.write_str(BASIC_RUN_TOKEN),
        }
    }
}

impl ManifestStr for Unp64Entry {
    const EXPECTING: &'static str = "an address string such as \"$080D\" or \"RUN\"";

    fn parse_manifest(text: &str) -> Result<Self, String> {
        if text == BASIC_RUN_TOKEN {
            return Ok(Self::BasicRun);
        }
        HexAddr::parse(text)
            .map(|addr| Self::Address(addr.0))
            .map_err(|e| format!("{e}, or \"{BASIC_RUN_TOKEN}\""))
    }
}

impl Serialize for Unp64Entry {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Unp64Entry {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserialize_manifest_str(deserializer)
    }
}

// =============================================================================
// Address ranges
// =============================================================================

/// An inclusive, non-empty 6502 address range.
///
/// # Example
///
/// `AddrRange::new(0x0800, 0xFFFF)` holds 63 488 bytes and displays as
/// `$0800-$FFFF`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AddrRange {
    start: u16,
    end: u16,
}

impl AddrRange {
    /// Returns the range `start..=end`, or `None` if `end < start`.
    #[must_use]
    pub fn new(start: u16, end: u16) -> Option<Self> {
        (start <= end).then_some(Self { start, end })
    }

    /// Returns the range that starts at `start` and holds `len` bytes, or
    /// `None` if `len` is zero or the range would pass `$FFFF`.
    ///
    /// # Example
    ///
    /// `from_start_len(0xFF00, 256)` is `$FF00-$FFFF`; with `257` it is
    /// `None`.
    #[must_use]
    pub fn from_start_len(start: u16, len: usize) -> Option<Self> {
        let last = usize::from(start).checked_add(len)?.checked_sub(1)?;
        let end = u16::try_from(last).ok()?;
        Self::new(start, end)
    }

    /// First address of the range.
    #[must_use]
    pub fn start(self) -> u16 {
        self.start
    }

    /// Last address of the range (inclusive).
    #[must_use]
    pub fn end(self) -> u16 {
        self.end
    }

    /// Number of bytes in the range (1 to 65 536).
    #[must_use]
    pub fn len(self) -> usize {
        usize::from(self.end - self.start) + 1
    }

    /// Returns `true` if `other` lies completely inside `self`.
    #[must_use]
    pub fn contains_range(self, other: Self) -> bool {
        self.start <= other.start && other.end <= self.end
    }

    /// Returns the overlap of both ranges, or `None` if they are disjoint.
    #[must_use]
    pub fn intersect(self, other: Self) -> Option<Self> {
        Self::new(self.start.max(other.start), self.end.min(other.end))
    }
}

impl fmt::Display for AddrRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", HexAddr(self.start), HexAddr(self.end))
    }
}

/// A byte buffer anchored at a 6502 address (an unpacked payload, an
/// original program, or unp64's saved bytes).
#[derive(Debug, Clone, Copy)]
pub(super) struct Addressed<'a> {
    /// Address of `data[0]`.
    pub(super) start: u16,
    /// The bytes.
    pub(super) data: &'a [u8],
}

impl<'a> Addressed<'a> {
    /// Returns the bytes of `window`, or `None` if the buffer does not cover
    /// all of it. Never panics and never reads outside the buffer.
    pub(super) fn slice(self, window: AddrRange) -> Option<&'a [u8]> {
        let offset = usize::from(window.start().checked_sub(self.start)?);
        self.data.get(offset..offset.checked_add(window.len())?)
    }
}

/// Returns the FNV-1a-64 of the bytes over `window` in a payload whose first
/// byte sits at `start`, or `None` if the payload does not cover the whole
/// window.
///
/// The compare tool uses it to record (and re-check) the overlap hash of
/// unp64's bytes; the oracle hashes R2000's bytes over the same window with
/// the same bounds-checked slice.
///
/// # Example
///
/// For a payload `[1, 2, 3]` at `$0800`, the window `$0801-$0802` hashes the
/// bytes `[2, 3]`, and the window `$0802-$0803` is `None`.
#[must_use]
pub fn hash_window(start: u16, payload: &[u8], window: AddrRange) -> Option<Fnv64> {
    Addressed {
        start,
        data: payload,
    }
    .slice(window)
    .map(Fnv64::of)
}

/// Returns the address of the first differing byte and the number of
/// differing bytes between two equally long slices that both start at `base`,
/// or `None` if they are identical.
///
/// # Example
///
/// `first_difference(0x0800, &[1, 2, 3], &[1, 9, 9])` is `Some((0x0801, 2))`.
pub(super) fn first_difference(base: u16, left: &[u8], right: &[u8]) -> Option<(u16, usize)> {
    let mut first = None;
    let mut count = 0usize;
    for (offset, (l, r)) in left.iter().zip(right).enumerate() {
        if l != r {
            count += 1;
            first.get_or_insert(offset);
        }
    }
    // `offset` is below the window length (at most 65 536), and the address
    // is reported modulo the address space like every other 6502 address.
    first.map(|offset| (base.wrapping_add(offset as u16), count))
}
