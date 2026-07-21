use serde::{Deserialize, Serialize};

// =============================================================================
// TargetSystem Enum
// =============================================================================

/// Strongly typed target machine architecture identifier.
#[derive(Debug, Clone, Eq)]
pub enum TargetSystem {
    C64,
    C128,
    Vic20,
    Pet20,
    Pet40,
    Plus4,
    C16,
    C1541,
    C1571,
    C1581,
    Custom(Box<str>),
}

pub type System = TargetSystem;

impl TargetSystem {
    pub const C64_NAME: &'static str = "Commodore 64";
    pub const C128_NAME: &'static str = "Commodore 128";
    pub const VIC20_NAME: &'static str = "Commodore VIC-20";
    pub const PET20_NAME: &'static str = "Commodore PET 2001";
    pub const PET40_NAME: &'static str = "Commodore PET 4000";
    pub const PLUS4_NAME: &'static str = "Commodore Plus4";
    pub const C16_NAME: &'static str = "Commodore 16";
    pub const C1541_NAME: &'static str = "Commodore 1541";
    pub const C1571_NAME: &'static str = "Commodore 1571";
    pub const C1581_NAME: &'static str = "Commodore 1581";

    /// Constructs a `TargetSystem` from any string-like reference using string parsing.
    #[must_use]
    pub fn new(s: impl AsRef<str>) -> Self {
        match s.as_ref().parse() {
            Ok(sys) => sys,
            Err(infallible) => match infallible {},
        }
    }

    /// Returns the canonical string representation of this system.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            TargetSystem::C64 => Self::C64_NAME,
            TargetSystem::C128 => Self::C128_NAME,
            TargetSystem::Vic20 => Self::VIC20_NAME,
            TargetSystem::Pet20 => Self::PET20_NAME,
            TargetSystem::Pet40 => Self::PET40_NAME,
            TargetSystem::Plus4 => Self::PLUS4_NAME,
            TargetSystem::C16 => Self::C16_NAME,
            TargetSystem::C1541 => Self::C1541_NAME,
            TargetSystem::C1571 => Self::C1571_NAME,
            TargetSystem::C1581 => Self::C1581_NAME,
            TargetSystem::Custom(s) => s,
        }
    }

    /// Consumes the enum and returns the string representation.
    #[must_use]
    pub fn into_inner(self) -> String {
        self.as_str().to_string()
    }

    /// Returns `true` if this system is Commodore 64.
    #[must_use]
    pub fn is_c64(&self) -> bool {
        matches!(self, TargetSystem::C64)
    }

    /// Returns the start address of default RAM for this system.
    #[must_use]
    pub fn ram_start(&self) -> u16 {
        match self {
            TargetSystem::C1541 | TargetSystem::C1571 | TargetSystem::C1581 => 0x0000,
            TargetSystem::C128 => 0x1C00,
            TargetSystem::Vic20 | TargetSystem::Plus4 | TargetSystem::C16 => 0x1000,
            TargetSystem::Pet20 | TargetSystem::Pet40 => 0x0400,
            _ => 0x0800,
        }
    }

    /// Returns the default BASIC start address for this system.
    #[must_use]
    pub fn default_basic_start(&self) -> u16 {
        match self {
            TargetSystem::C1541 | TargetSystem::C1571 | TargetSystem::C1581 => 0x0000,
            TargetSystem::C128 => 0x1C01,
            TargetSystem::Vic20 | TargetSystem::Plus4 | TargetSystem::C16 => 0x1001,
            TargetSystem::Pet20 | TargetSystem::Pet40 => 0x0401,
            _ => 0x0801,
        }
    }

    /// Returns the default screen RAM memory range if defined for this system.
    #[must_use]
    pub fn screen_ram(&self) -> Option<std::ops::RangeInclusive<u16>> {
        match self {
            TargetSystem::C64 | TargetSystem::C128 => Some(0x0400..=0x07E7),
            TargetSystem::Vic20 => Some(0x1E00..=0x1FFF),
            TargetSystem::Plus4 | TargetSystem::C16 => Some(0x0C00..=0x0FE7),
            TargetSystem::Pet20 | TargetSystem::Pet40 => Some(0x8000..=0x87CF),
            _ => None,
        }
    }

    /// Returns the default hardware IRQ vector address and handler value `(vector_addr, handler_addr)`.
    #[must_use]
    pub fn default_irq(&self) -> Option<(u16, u16)> {
        match self {
            TargetSystem::C64 => Some((0x0314, 0xEA31)),
            _ => None,
        }
    }

    /// Returns the scan boundary ceilings for output range detection on this system.
    #[must_use]
    pub fn memory_boundaries(&self) -> &'static [usize] {
        match self {
            TargetSystem::C128 => &[0x3FFF, 0xBFFF, 0xFFEF],
            TargetSystem::Vic20 => &[0x1FFF, 0x7FFF, 0xFFEF],
            _ => &[0x9FFF, 0xCFFF, 0xFFEF],
        }
    }

    /// Returns the hardware I/O memory range for this system, if defined.
    #[must_use]
    pub fn io_range(&self) -> Option<std::ops::RangeInclusive<u16>> {
        match self {
            TargetSystem::C64 | TargetSystem::C128 => Some(0xD000..=0xDFFF),
            TargetSystem::Vic20 => Some(0x9000..=0x97FF),
            TargetSystem::Plus4 | TargetSystem::C16 => Some(0xFF00..=0xFF3F),
            TargetSystem::Pet20 | TargetSystem::Pet40 => Some(0xE800..=0xE8FF),
            _ => None,
        }
    }

    /// Returns `true` if `addr` falls within the hardware I/O memory space for this system.
    #[must_use]
    pub fn is_in_io(&self, addr: u16) -> bool {
        self.io_range().is_some_and(|r| r.contains(&addr))
    }

    /// Returns `true` if `addr` falls within the BASIC ROM memory space for this system.
    #[must_use]
    pub fn is_in_basic_rom(&self, addr: u16) -> bool {
        match self {
            TargetSystem::C64 => (0xA000..=0xBFFF).contains(&addr),
            TargetSystem::C128 => (0x4000..=0x7FFF).contains(&addr),
            TargetSystem::Vic20 => (0xC000..=0xDFFF).contains(&addr),
            TargetSystem::Plus4 | TargetSystem::C16 => (0x8000..=0xBFFF).contains(&addr),
            TargetSystem::Pet20 | TargetSystem::Pet40 => (0xC000..=0xDFFF).contains(&addr),
            TargetSystem::C1541 | TargetSystem::C1571 | TargetSystem::C1581 => false,
            _ => false,
        }
    }

    /// Returns `true` if `addr` falls within the Kernal ROM memory space for this system.
    #[must_use]
    pub fn is_in_kernal_rom(&self, addr: u16) -> bool {
        match self {
            TargetSystem::C64 | TargetSystem::Vic20 | TargetSystem::Plus4 | TargetSystem::C16 => {
                addr >= 0xE000
            }
            TargetSystem::C128
            | TargetSystem::C1541
            | TargetSystem::C1571
            | TargetSystem::C1581 => addr >= 0xC000,
            _ => addr >= 0xE000,
        }
    }

    /// Returns `true` if `addr` is the main BASIC interpreter execution entry point (e.g. `$A7AE` on C64).
    #[must_use]
    pub fn is_basic_exec_entry(&self, addr: u16) -> bool {
        match self {
            TargetSystem::C64 => addr == 0xA7AE,
            _ => false,
        }
    }

    /// Returns `true` if this system uses Commodore BASIC V2 tokenized structure (C64, C128, VIC-20, PET, Plus/4).
    #[must_use]
    pub fn is_commodore_basic(&self) -> bool {
        matches!(
            self,
            TargetSystem::C64
                | TargetSystem::C128
                | TargetSystem::Vic20
                | TargetSystem::Pet20
                | TargetSystem::Pet40
                | TargetSystem::Plus4
                | TargetSystem::C16
        )
    }

    /// Returns the upper RAM boundary ceiling before hardware vectors ($FFF8..$FFFF).
    #[must_use]
    pub fn ram_ceiling(&self) -> u16 {
        0xFFEF
    }

    /// Parses a string slice into a standard `TargetSystem` variant without heap allocation.
    /// Returns `None` if `s` is not a standard machine architecture alias.
    #[must_use]
    pub fn parse_standard(s: &str) -> Option<Self> {
        let trimmed = s.trim();
        if trimmed.eq_ignore_ascii_case("c64")
            || trimmed.eq_ignore_ascii_case("commodore 64")
            || trimmed.eq_ignore_ascii_case("commodore_64")
            || trimmed.eq_ignore_ascii_case("c-64")
        {
            return Some(TargetSystem::C64);
        }
        if trimmed.eq_ignore_ascii_case("c128")
            || trimmed.eq_ignore_ascii_case("commodore 128")
            || trimmed.eq_ignore_ascii_case("commodore_128")
            || trimmed.eq_ignore_ascii_case("c-128")
        {
            return Some(TargetSystem::C128);
        }
        if trimmed.eq_ignore_ascii_case("vic20")
            || trimmed.eq_ignore_ascii_case("vic 20")
            || trimmed.eq_ignore_ascii_case("vic-20")
            || trimmed.eq_ignore_ascii_case("commodore vic-20")
            || trimmed.eq_ignore_ascii_case("vc20")
        {
            return Some(TargetSystem::Vic20);
        }
        if trimmed.eq_ignore_ascii_case("pet20")
            || trimmed.eq_ignore_ascii_case("pet2001")
            || trimmed.eq_ignore_ascii_case("pet 2001")
            || trimmed.eq_ignore_ascii_case("commodore pet 2001")
            || trimmed.eq_ignore_ascii_case("pet 2.0")
            || trimmed.eq_ignore_ascii_case("commodore pet 2.0")
        {
            return Some(TargetSystem::Pet20);
        }
        if trimmed.eq_ignore_ascii_case("pet40")
            || trimmed.eq_ignore_ascii_case("pet4000")
            || trimmed.eq_ignore_ascii_case("pet 4000")
            || trimmed.eq_ignore_ascii_case("commodore pet 4000")
            || trimmed.eq_ignore_ascii_case("pet 4.0")
            || trimmed.eq_ignore_ascii_case("commodore pet 4.0")
            || trimmed.eq_ignore_ascii_case("pet")
            || trimmed.eq_ignore_ascii_case("commodore pet")
            || trimmed.eq_ignore_ascii_case("pet80")
            || trimmed.eq_ignore_ascii_case("pet8000")
            || trimmed.eq_ignore_ascii_case("pet 8000")
            || trimmed.eq_ignore_ascii_case("commodore pet 8000")
            || trimmed.eq_ignore_ascii_case("pet 8.0")
            || trimmed.eq_ignore_ascii_case("commodore pet 8.0")
        {
            return Some(TargetSystem::Pet40);
        }
        if trimmed.eq_ignore_ascii_case("plus4")
            || trimmed.eq_ignore_ascii_case("plus 4")
            || trimmed.eq_ignore_ascii_case("plus-4")
            || trimmed.eq_ignore_ascii_case("plus/4")
            || trimmed.eq_ignore_ascii_case("commodore plus4")
        {
            return Some(TargetSystem::Plus4);
        }
        if trimmed.eq_ignore_ascii_case("c16")
            || trimmed.eq_ignore_ascii_case("c-16")
            || trimmed.eq_ignore_ascii_case("c 16")
            || trimmed.eq_ignore_ascii_case("commodore 16")
        {
            return Some(TargetSystem::C16);
        }
        if trimmed.eq_ignore_ascii_case("c1541")
            || trimmed.eq_ignore_ascii_case("1541")
            || trimmed.eq_ignore_ascii_case("commodore 1541")
        {
            return Some(TargetSystem::C1541);
        }
        if trimmed.eq_ignore_ascii_case("c1571")
            || trimmed.eq_ignore_ascii_case("1571")
            || trimmed.eq_ignore_ascii_case("commodore 1571")
        {
            return Some(TargetSystem::C1571);
        }
        if trimmed.eq_ignore_ascii_case("c1581")
            || trimmed.eq_ignore_ascii_case("1581")
            || trimmed.eq_ignore_ascii_case("commodore 1581")
        {
            return Some(TargetSystem::C1581);
        }
        None
    }
}

impl Default for TargetSystem {
    fn default() -> Self {
        default_system()
    }
}

impl std::fmt::Display for TargetSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AsRef<str> for TargetSystem {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::str::FromStr for TargetSystem {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();
        if let Some(sys) = TargetSystem::parse_standard(trimmed) {
            Ok(sys)
        } else {
            Ok(TargetSystem::Custom(trimmed.into()))
        }
    }
}

impl serde::Serialize for TargetSystem {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for TargetSystem {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct TargetSystemVisitor;

        impl<'de> serde::de::Visitor<'de> for TargetSystemVisitor {
            type Value = TargetSystem;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a valid target system identifier string")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                v.parse().map_err(serde::de::Error::custom)
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let trimmed = v.trim();
                if let Some(sys) = TargetSystem::parse_standard(trimmed) {
                    Ok(sys)
                } else if trimmed.len() == v.len() {
                    Ok(TargetSystem::Custom(v.into_boxed_str()))
                } else {
                    Ok(TargetSystem::Custom(trimmed.into()))
                }
            }
        }

        deserializer.deserialize_str(TargetSystemVisitor)
    }
}

impl PartialEq for TargetSystem {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (TargetSystem::Custom(a), TargetSystem::Custom(b)) => {
                if let (Some(sys_a), Some(sys_b)) =
                    (Self::parse_standard(a), Self::parse_standard(b))
                {
                    sys_a == sys_b
                } else if Self::parse_standard(a).is_some() || Self::parse_standard(b).is_some() {
                    false
                } else {
                    a.eq_ignore_ascii_case(b)
                }
            }
            (TargetSystem::Custom(a), std_sys) | (std_sys, TargetSystem::Custom(a)) => {
                if let Some(parsed) = Self::parse_standard(a) {
                    &parsed == std_sys
                } else {
                    false
                }
            }
            (a, b) => core::mem::discriminant(a) == core::mem::discriminant(b),
        }
    }
}

impl std::hash::Hash for TargetSystem {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            TargetSystem::Custom(s) => {
                if let Some(std_sys) = Self::parse_standard(s) {
                    std_sys.hash(state);
                } else {
                    for byte in s.bytes() {
                        byte.to_ascii_lowercase().hash(state);
                    }
                }
            }
            std_sys => {
                core::mem::discriminant(std_sys).hash(state);
            }
        }
    }
}

impl PartialEq<str> for TargetSystem {
    fn eq(&self, other: &str) -> bool {
        let trimmed = other.trim();
        if let Some(other_sys) = TargetSystem::parse_standard(trimmed) {
            self == &other_sys
        } else if let TargetSystem::Custom(s) = self {
            s.as_ref().eq_ignore_ascii_case(trimmed)
        } else {
            false
        }
    }
}

impl PartialEq<TargetSystem> for str {
    fn eq(&self, other: &TargetSystem) -> bool {
        other == self
    }
}

impl PartialEq<TargetSystem> for &str {
    fn eq(&self, other: &TargetSystem) -> bool {
        other == *self
    }
}

impl PartialEq<TargetSystem> for String {
    fn eq(&self, other: &TargetSystem) -> bool {
        other == self.as_str()
    }
}

impl PartialEq<&str> for TargetSystem {
    fn eq(&self, other: &&str) -> bool {
        self == *other
    }
}

impl PartialEq<String> for TargetSystem {
    fn eq(&self, other: &String) -> bool {
        self == other.as_str()
    }
}

impl From<&str> for TargetSystem {
    fn from(s: &str) -> Self {
        TargetSystem::new(s)
    }
}

impl From<String> for TargetSystem {
    fn from(s: String) -> Self {
        TargetSystem::new(&s)
    }
}

impl From<&String> for TargetSystem {
    fn from(s: &String) -> Self {
        TargetSystem::new(s)
    }
}

#[must_use]
pub fn default_system() -> TargetSystem {
    TargetSystem::C64
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Addr(pub u16);

impl Serialize for Addr {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u16(self.0)
    }
}

impl<'de> Deserialize<'de> for Addr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct AddrVisitor;

        impl<'de> serde::de::Visitor<'de> for AddrVisitor {
            type Value = Addr;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a 16-bit address as integer or string")
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                u16::try_from(v).map(Addr).map_err(serde::de::Error::custom)
            }

            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                u16::try_from(v).map(Addr).map_err(serde::de::Error::custom)
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let v_trimmed = v.trim();
                if let Some(hex) = v_trimmed.strip_prefix('$') {
                    u16::from_str_radix(hex, 16)
                        .map(Addr)
                        .map_err(serde::de::Error::custom)
                } else if let Some(hex) = v_trimmed
                    .strip_prefix("0x")
                    .or_else(|| v_trimmed.strip_prefix("0X"))
                {
                    u16::from_str_radix(hex, 16)
                        .map(Addr)
                        .map_err(serde::de::Error::custom)
                } else {
                    v_trimmed
                        .parse::<u16>()
                        .map(Addr)
                        .map_err(serde::de::Error::custom)
                }
            }
        }

        deserializer.deserialize_any(AddrVisitor)
    }
}

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
    #[serde(skip)]
    pub source_file: Option<String>,
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
            source_file: None,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_system_serde_roundtrip() {
        let systems = vec![
            TargetSystem::C64,
            TargetSystem::C128,
            TargetSystem::Vic20,
            TargetSystem::Pet20,
            TargetSystem::Pet40,
            TargetSystem::Plus4,
            TargetSystem::C16,
            TargetSystem::C1541,
            TargetSystem::C1571,
            TargetSystem::C1581,
            TargetSystem::Custom("Atari 800".into()),
        ];

        for sys in systems {
            let json = serde_json::to_string(&sys).expect("serialize target system");
            let deserialized: TargetSystem =
                serde_json::from_str(&json).expect("deserialize target system");
            assert_eq!(sys, deserialized, "Serde roundtrip failed for {sys}");
        }
    }

    #[test]
    fn test_target_system_alias_resolution() {
        use std::str::FromStr;

        assert_eq!(TargetSystem::from_str("c64").ok(), Some(TargetSystem::C64));
        assert_eq!(
            TargetSystem::from_str("Commodore 64").ok(),
            Some(TargetSystem::C64)
        );
        assert_eq!(TargetSystem::from_str("c-64").ok(), Some(TargetSystem::C64));
        assert_eq!(
            TargetSystem::from_str("commodore_64").ok(),
            Some(TargetSystem::C64)
        );

        assert_eq!(
            TargetSystem::from_str("c128").ok(),
            Some(TargetSystem::C128)
        );
        assert_eq!(
            TargetSystem::from_str("vic20").ok(),
            Some(TargetSystem::Vic20)
        );
        assert_eq!(
            TargetSystem::from_str("pet40").ok(),
            Some(TargetSystem::Pet40)
        );
        assert_eq!(
            TargetSystem::from_str("plus4").ok(),
            Some(TargetSystem::Plus4)
        );
        assert_eq!(
            TargetSystem::from_str("c1541").ok(),
            Some(TargetSystem::C1541)
        );

        let custom = TargetSystem::from_str("Unknown Machine").ok();
        assert_eq!(custom, Some(TargetSystem::Custom("Unknown Machine".into())));
    }

    #[test]
    fn test_target_system_partial_eq_consistency() {
        let sys = TargetSystem::C64;
        // TargetSystem == str / &str / String
        assert_eq!(sys, "Commodore 64");
        assert_eq!(sys, "commodore 64");
        assert_eq!(sys, String::from("Commodore 64"));

        // Symmetric: str / &str / String == TargetSystem
        assert_eq!("Commodore 64", sys);
        assert_eq!("commodore 64", sys);
        assert_eq!(&"Commodore 64", &sys);
        assert_eq!(String::from("Commodore 64"), sys);

        // Short aliases
        assert_eq!(TargetSystem::C64, "c64");
        assert_eq!("c64", TargetSystem::C64);
        assert_eq!(TargetSystem::C64, "commodore 64");
        assert_eq!(TargetSystem::Vic20, "vic20");
        assert_eq!(TargetSystem::Plus4, "plus4");
        assert_eq!(TargetSystem::C1541, "c1541");
        assert_ne!(TargetSystem::C64, "c128");

        // Custom variants
        let custom = TargetSystem::Custom("CustomSystem".into());
        assert_eq!(custom, "CustomSystem");
        assert_eq!("CustomSystem", custom);
        assert_eq!(&custom, &"CustomSystem");
        assert_eq!(&"CustomSystem", &custom);

        // Transitivity & Custom vs Standard equivalence
        let custom_c64 = TargetSystem::Custom("c64".into());
        let std_c64 = TargetSystem::C64;
        let str_c64 = "c64";

        assert_eq!(custom_c64, std_c64);
        assert_eq!(std_c64, custom_c64);
        assert_eq!(custom_c64, str_c64);
        assert_eq!(str_c64, custom_c64);
        assert_eq!(std_c64, str_c64);
        assert_eq!(str_c64, std_c64);

        // Case-insensitivity for custom variants
        let custom_lower = TargetSystem::Custom("my_sys".into());
        let custom_upper = TargetSystem::Custom("MY_SYS".into());
        assert_eq!(custom_lower, custom_upper);
        assert_eq!(custom_upper, custom_lower);
        assert_eq!(custom_lower, "MY_SYS");
        assert_eq!("my_sys", custom_upper);
    }

    #[test]
    fn test_target_system_pet_and_drive_hardware_specs() {
        // PET video RAM
        assert_eq!(TargetSystem::Pet20.screen_ram(), Some(0x8000..=0x87CF));
        assert_eq!(TargetSystem::Pet40.screen_ram(), Some(0x8000..=0x87CF));

        // PET BASIC ROM
        assert!(TargetSystem::Pet40.is_in_basic_rom(0xC000));
        assert!(TargetSystem::Pet40.is_in_basic_rom(0xDFFF));
        assert!(!TargetSystem::Pet40.is_in_basic_rom(0xA000));

        // Disk Drive RAM start & BASIC ROM
        for drive in [
            TargetSystem::C1541,
            TargetSystem::C1571,
            TargetSystem::C1581,
        ] {
            assert_eq!(drive.ram_start(), 0x0000);
            assert_eq!(drive.default_basic_start(), 0x0000);
            assert!(!drive.is_in_basic_rom(0xA000));
            assert!(!drive.is_in_basic_rom(0xC000));
        }

        // BASIC exec entry
        assert!(TargetSystem::C64.is_basic_exec_entry(0xA7AE));
        assert!(!TargetSystem::C128.is_basic_exec_entry(0xA7AE));
    }

    #[test]
    fn test_target_system_serde_visit_string() {
        let json_owned_str = "\"c64\"".to_string();
        let deserialized: TargetSystem =
            serde_json::from_str(&json_owned_str).expect("deserialize owned string");
        assert_eq!(deserialized, TargetSystem::C64);
    }
}
