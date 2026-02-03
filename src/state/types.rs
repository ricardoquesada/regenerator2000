use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Platform {
    Commodore128,
    Commodore1541,
    #[default]
    Commodore64,
    CommodorePET20,
    CommodorePET40,
    CommodorePlus4,
    CommodoreVIC20,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HexdumpViewMode {
    #[default]
    ScreencodeShifted,
    ScreencodeUnshifted,
    PETSCIIShifted,
    PETSCIIUnshifted,
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Platform::Commodore128 => write!(f, "Commodore 128"),
            Platform::Commodore1541 => write!(f, "Commodore 1541"),
            Platform::Commodore64 => write!(f, "Commodore 64"),
            Platform::CommodorePET20 => write!(f, "Commodore PET 2.0"),
            Platform::CommodorePET40 => write!(f, "Commodore PET 4.0"),
            Platform::CommodorePlus4 => write!(f, "Commodore Plus/4"),
            Platform::CommodoreVIC20 => write!(f, "Commodore VIC 20"),
        }
    }
}

impl Platform {
    pub fn all() -> &'static [Platform] {
        &[
            Platform::Commodore128,
            Platform::Commodore1541,
            Platform::Commodore64,
            Platform::CommodorePET20,
            Platform::CommodorePET40,
            Platform::CommodorePlus4,
            Platform::CommodoreVIC20,
        ]
    }
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
    /// - ExternalJump, AbsoluteAddress, Field, Pointer use 4 hex digits (e.g., "a00FF")
    /// - Other types use 2 hex digits (e.g., "aFF")
    ///
    /// For non-zero-page addresses (0x100+):
    /// - All types use 4 hex digits (e.g., "a1234")
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
                    format!("{}{:04X}", prefix, addr)
                }
                _ => {
                    // Use 2 digits for zero page types
                    format!("{}{:02X}", prefix, addr)
                }
            }
        } else {
            // Non-zero page: always use 4 digits
            format!("{}{:04X}", prefix, addr)
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
    LowByte(u16),
    HighByte(u16),
}
