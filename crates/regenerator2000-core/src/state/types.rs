use serde::{Deserialize, Serialize};

// =============================================================================
// System newtype
// =============================================================================

/// A target system identifier (e.g. "Commodore 64", "NES").
///
/// Wraps a `String` to prevent accidentally passing an arbitrary string where
/// a system name is expected. Serialises transparently as a plain JSON string
/// so existing project files keep working.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct System(String);

impl System {
    pub const C64: &'static str = "Commodore 64";
    pub const C128: &'static str = "Commodore 128";
    pub const VIC20: &'static str = "Commodore VIC-20";
    pub const PET: &'static str = "Commodore PET 4.0";
    pub const PLUS4: &'static str = "Commodore Plus4";
    pub const C1541: &'static str = "Commodore 1541";
    pub const PET20: &'static str = "Commodore PET 2.0";

    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Returns the inner string slice, just like `String::as_str`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the wrapper and returns the inner `String`.
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl Default for System {
    fn default() -> Self {
        default_system()
    }
}

impl std::fmt::Display for System {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::ops::Deref for System {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl PartialEq<str> for System {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for System {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl PartialEq<String> for System {
    fn eq(&self, other: &String) -> bool {
        self.0 == *other
    }
}

impl From<&str> for System {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for System {
    fn from(s: String) -> Self {
        Self(s)
    }
}

#[must_use]
pub fn default_system() -> System {
    System::new(System::C64)
}

// =============================================================================
// Addr newtype
// =============================================================================

/// A 16-bit address in the 6502 address space.
///
/// Wraps a `u16` to distinguish *addresses* from other numeric quantities
/// (lengths, byte values, indices). The inner field is `pub` for easy
/// interop with existing `u16`-heavy code; the type safety comes from
/// function signatures, not from hiding the value.
///
/// Serialises transparently as a plain JSON number so existing project files
/// are fully backward-compatible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Addr(pub u16);

impl Addr {
    /// Wraps a raw `u16` into an `Addr`.
    #[must_use]
    pub const fn new(raw: u16) -> Self {
        Self(raw)
    }

    /// Returns the underlying `u16` value.
    #[must_use]
    pub const fn raw(self) -> u16 {
        self.0
    }

    /// Wrapping addition, matching the 6502's 16-bit address bus.
    #[must_use]
    pub const fn wrapping_add(self, rhs: u16) -> Self {
        Self(self.0.wrapping_add(rhs))
    }

    /// Wrapping subtraction, matching the 6502's 16-bit address bus.
    #[must_use]
    pub const fn wrapping_sub(self, rhs: u16) -> Self {
        Self(self.0.wrapping_sub(rhs))
    }

    /// Returns the byte offset from `origin` to `self` as a `usize`.
    ///
    /// Uses wrapping subtraction so it works correctly even when the address
    /// space wraps around $FFFF → $0000.
    #[must_use]
    pub const fn offset_from(self, origin: Addr) -> usize {
        self.0.wrapping_sub(origin.0) as usize
    }

    /// Saturating addition.
    #[must_use]
    pub const fn saturating_add(self, rhs: u16) -> Self {
        Self(self.0.saturating_add(rhs))
    }

    /// Saturating subtraction.
    #[must_use]
    pub const fn saturating_sub(self, rhs: u16) -> Self {
        Self(self.0.saturating_sub(rhs))
    }

    /// Zero address constant.
    pub const ZERO: Addr = Addr(0);
}

impl Default for Addr {
    fn default() -> Self {
        Self::ZERO
    }
}

impl std::fmt::Display for Addr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "${:04X}", self.0)
    }
}

impl std::fmt::UpperHex for Addr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::UpperHex::fmt(&self.0, f)
    }
}

impl std::fmt::LowerHex for Addr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::LowerHex::fmt(&self.0, f)
    }
}

// ---- Conversions ----

impl From<u16> for Addr {
    fn from(val: u16) -> Self {
        Self(val)
    }
}

impl From<Addr> for u16 {
    fn from(addr: Addr) -> u16 {
        addr.0
    }
}

impl From<Addr> for usize {
    fn from(addr: Addr) -> usize {
        addr.0 as usize
    }
}

impl From<Addr> for i32 {
    fn from(addr: Addr) -> i32 {
        addr.0 as i32
    }
}

/// Allows `BTreeMap<Addr, T>` to be queried with `&u16` keys.
impl std::borrow::Borrow<u16> for Addr {
    fn borrow(&self) -> &u16 {
        &self.0
    }
}

// ---- Comparison with u16 ----

impl PartialEq<u16> for Addr {
    fn eq(&self, other: &u16) -> bool {
        self.0 == *other
    }
}

impl PartialOrd<u16> for Addr {
    fn partial_cmp(&self, other: &u16) -> Option<std::cmp::Ordering> {
        Some(self.0.cmp(other))
    }
}

// ---- Arithmetic operators ----

impl std::ops::Add<u16> for Addr {
    type Output = Addr;
    fn add(self, rhs: u16) -> Self {
        Self(self.0.wrapping_add(rhs))
    }
}

impl std::ops::Sub<u16> for Addr {
    type Output = Addr;
    fn sub(self, rhs: u16) -> Self {
        Self(self.0.wrapping_sub(rhs))
    }
}

impl std::ops::Sub<Addr> for Addr {
    type Output = u16;
    fn sub(self, rhs: Addr) -> u16 {
        self.0.wrapping_sub(rhs.0)
    }
}

impl std::ops::BitAnd<u16> for Addr {
    type Output = u16;
    fn bitand(self, rhs: u16) -> u16 {
        self.0 & rhs
    }
}

impl std::ops::Shr<u16> for Addr {
    type Output = u16;
    fn shr(self, rhs: u16) -> u16 {
        self.0 >> rhs
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HexdumpViewMode {
    #[default]
    ScreencodeShifted,
    ScreencodeUnshifted,
    PETSCIIShifted,
    PETSCIIUnshifted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Assembler {
    #[default]
    Tass64,
    Acme,
    Ca65,
    Kick,
}

impl std::fmt::Display for Assembler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Assembler::Tass64 => write!(f, "64tass"),
            Assembler::Acme => write!(f, "ACME"),
            Assembler::Ca65 => write!(f, "ca65"),
            Assembler::Kick => write!(f, "KickAssembler"),
        }
    }
}

impl Assembler {
    #[must_use]
    pub fn all() -> &'static [Assembler] {
        &[
            Assembler::Tass64,
            Assembler::Acme,
            Assembler::Ca65,
            Assembler::Kick,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockType {
    Code,
    DataByte,
    DataWord,
    Address,
    #[serde(alias = "Text")]
    PetsciiText,
    #[serde(alias = "Screencode")]
    ScreencodeText,
    #[serde(alias = "LoHi")]
    LoHiAddress,
    #[serde(alias = "HiLo")]
    HiLoAddress,
    LoHiWord,
    HiLoWord,
    ExternalFile,
    Undefined,
}

impl std::fmt::Display for BlockType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlockType::Code => write!(f, "Code"),
            BlockType::DataByte => write!(f, "Byte"),
            BlockType::DataWord => write!(f, "Word"),
            BlockType::Address => write!(f, "Address"),
            BlockType::PetsciiText => write!(f, "PETSCII Text"),
            BlockType::ScreencodeText => write!(f, "Screencode Text"),
            BlockType::LoHiAddress => write!(f, "Lo/Hi Address"),
            BlockType::HiLoAddress => write!(f, "Hi/Lo Address"),
            BlockType::LoHiWord => write!(f, "Lo/Hi Word"),
            BlockType::HiLoWord => write!(f, "Hi/Lo Word"),
            BlockType::ExternalFile => write!(f, "External File"),
            BlockType::Undefined => write!(f, "Undefined"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LabelKind {
    User,
    Auto,
    #[serde(alias = "Platform")]
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum LabelType {
    ZeroPageField = 0,
    Field = 1,
    ZeroPageAbsoluteAddress = 2,
    AbsoluteAddress = 3,
    Pointer = 4,
    ZeroPagePointer = 5,
    Branch = 6,
    Jump = 7,
    Subroutine = 8,
    ExternalJump = 9,
    Predefined = 10,
    UserDefined = 11,
    LocalUserDefined = 12,
    /// Target of a branch/JMP/JSR whose first instruction is `RTS` or `RTI`.
    /// Mirrors IDA Pro's `locret_` convention.
    Return = 13,
}

impl LabelType {
    #[must_use]
    pub fn prefix(&self) -> &'static str {
        match self {
            LabelType::ZeroPageField => "zpf_",
            LabelType::Field => "f_",
            LabelType::ZeroPageAbsoluteAddress => "zpa_",
            LabelType::AbsoluteAddress => "a_",
            LabelType::Pointer => "p_",
            LabelType::ZeroPagePointer => "zpp_",
            LabelType::ExternalJump => "e_",
            LabelType::Jump => "j_",
            LabelType::Subroutine => "s_",
            LabelType::Branch => "b_",
            LabelType::Return => "r_",
            LabelType::Predefined => "L_",
            LabelType::UserDefined => "L_",
            LabelType::LocalUserDefined => "L_",
        }
    }

    /// Formats a label name for the given address and label type.
    ///
    /// For zero-page addresses (0x00-0xFF):
    /// - `ExternalJump`, `AbsoluteAddress`, Field, Pointer use 4 hex digits (e.g., "a00FF")
    /// - Other types use 2 hex digits (e.g., "aFF")
    ///
    /// For non-zero-page addresses (0x100+):
    /// - All types use 4 hex digits (e.g., "a1234")
    #[must_use]
    pub fn format_label(&self, addr: u16) -> String {
        let prefix = self.prefix();

        if addr <= 0xFF {
            // Zero page address
            match self {
                LabelType::ExternalJump
                | LabelType::AbsoluteAddress
                | LabelType::Field
                | LabelType::Pointer => {
                    // Force 4 digits for these types even in zero page
                    format!("{prefix}{addr:04X}")
                }
                _ => {
                    // Use 2 digits for zero page types
                    format!("{prefix}{addr:02X}")
                }
            }
        } else {
            // Non-zero page: always use 4 digits
            format!("{prefix}{addr:04X}")
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImmediateFormat {
    Hex,
    InvertedHex,
    Decimal,
    NegativeDecimal,
    Binary,
    InvertedBinary,
    LowByte(Addr),
    HighByte(Addr),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedArrow {
    pub start: usize,
    pub end: usize,
    pub target_addr: Option<Addr>,
}

impl std::ops::Rem<u16> for Addr {
    type Output = u16;
    fn rem(self, rhs: u16) -> u16 {
        self.0 % rhs
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommentKind {
    Side,
    Line,
}

// =============================================================================
// Enum Support Types
// =============================================================================

/// A runtime representation of a value-to-name enum mapping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnumDefinition {
    pub name: String,
    pub description: Option<String>,
    pub variants: std::collections::BTreeMap<u16, String>,
}

impl EnumDefinition {
    /// Parse numeric key values (which are represented as string keys in TOML)
    /// and construct a BTreeMap<u16, String> map. Supports:
    /// - Decimal strings (e.g., "10", "255")
    /// - Hex strings with "0x" or "$" prefix (e.g., "0x0a", "$0A")
    /// - Binary strings with "0b" or "%" prefix (e.g., "0b0101", "%0101")
    #[must_use]
    pub fn parse_variants(
        raw: std::collections::BTreeMap<String, String>,
    ) -> std::collections::BTreeMap<u16, String> {
        let mut parsed = std::collections::BTreeMap::new();
        for (k, v) in raw {
            let k_trimmed = k.trim();
            let parsed_val = if let Some(hex) = k_trimmed
                .strip_prefix("0x")
                .or_else(|| k_trimmed.strip_prefix("0X"))
            {
                u16::from_str_radix(hex, 16)
            } else if let Some(hex) = k_trimmed.strip_prefix('$') {
                u16::from_str_radix(hex, 16)
            } else if let Some(bin) = k_trimmed
                .strip_prefix("0b")
                .or_else(|| k_trimmed.strip_prefix("0B"))
            {
                u16::from_str_radix(bin, 2)
            } else if let Some(bin) = k_trimmed.strip_prefix('%') {
                u16::from_str_radix(bin, 2)
            } else {
                k_trimmed.parse::<u16>()
            };

            match parsed_val {
                Ok(val) => {
                    parsed.insert(val, v);
                }
                Err(_) => {
                    log::warn!("Invalid numeric key in enum variants: {}", k_trimmed);
                }
            }
        }
        parsed
    }
}

/// TOML-serializable helper representation of an enum file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawEnumDefinition {
    pub name: String,
    pub description: Option<String>,
    pub variants: std::collections::BTreeMap<String, String>,
}

impl From<RawEnumDefinition> for EnumDefinition {
    fn from(raw: RawEnumDefinition) -> Self {
        Self {
            name: raw.name,
            description: raw.description,
            variants: EnumDefinition::parse_variants(raw.variants),
        }
    }
}

impl From<EnumDefinition> for RawEnumDefinition {
    fn from(def: EnumDefinition) -> Self {
        let mut variants = std::collections::BTreeMap::new();
        for (k, v) in def.variants {
            variants.insert(format!("0x{k:02X}"), v);
        }
        Self {
            name: def.name,
            description: def.description,
            variants,
        }
    }
}
