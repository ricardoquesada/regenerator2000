use serde::{Deserialize, Serialize};

// =============================================================================
// Platform newtype
// =============================================================================

/// A target platform identifier (e.g. "Commodore 64", "NES").
///
/// Wraps a `String` to prevent accidentally passing an arbitrary string where
/// a platform name is expected. Serialises transparently as a plain JSON string
/// so existing project files keep working.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Platform(String);

impl Platform {
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

impl Default for Platform {
    fn default() -> Self {
        default_platform()
    }
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::ops::Deref for Platform {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl PartialEq<str> for Platform {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for Platform {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl PartialEq<String> for Platform {
    fn eq(&self, other: &String) -> bool {
        self.0 == *other
    }
}

impl From<&str> for Platform {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for Platform {
    fn from(s: String) -> Self {
        Self(s)
    }
}

#[must_use]
pub fn default_platform() -> Platform {
    Platform::new("Commodore 64")
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
    Routine,
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
            BlockType::Routine => write!(f, "Routine"),
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
}

impl LabelType {
    #[must_use]
    pub fn prefix(&self) -> char {
        match self {
            LabelType::ZeroPageField => 'f',
            LabelType::Field => 'f',
            LabelType::ZeroPageAbsoluteAddress => 'a',
            LabelType::AbsoluteAddress => 'a',
            LabelType::Pointer => 'p',
            LabelType::ZeroPagePointer => 'p',
            LabelType::ExternalJump => 'e',
            LabelType::Jump => 'j',
            LabelType::Subroutine => 's',
            LabelType::Branch => 'b',
            LabelType::Predefined => 'L',
            LabelType::UserDefined => 'L',
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
