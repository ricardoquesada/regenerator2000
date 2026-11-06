//! MOS 6502 and 6510 opcode table, addressing modes, and cross-reference classifications.
//!
//! Defines all 256 opcode bytes of the MOS 6502/6510 processor (both documented instructions
//! and undocumented opcodes), their [`AddressingMode`] variants, instruction sizes, cycle counts,
//! and memory access classifications ([`Opcode::xref_kind`]).

use crate::state::XRefKind;

/// Represents the operand addressing modes of the MOS 6502/6510 instruction set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub enum AddressingMode {
    /// Single-byte instruction with no explicit operand (such as `RTS`, `CLC`, or `NOP`).
    Implied,
    /// Single-byte instruction that operates on the accumulator register `A` (such as `ASL A`).
    Accumulator,
    /// Two-byte instruction with an 8-bit literal constant operand (`#$nn`).
    Immediate,
    /// Two-byte instruction that addresses zero-page memory `$0000..=$00FF` (`$nn`).
    ZeroPage,
    /// Two-byte instruction that addresses zero-page memory indexed by `X` (`$nn,X`).
    ZeroPageX,
    /// Two-byte instruction that addresses zero-page memory indexed by `Y` (`$nn,Y`).
    ZeroPageY,
    /// Two-byte conditional branch instruction with a signed 8-bit PC-relative offset.
    Relative,
    /// Three-byte instruction that addresses a 16-bit memory location (`$nnnn`).
    Absolute,
    /// Three-byte instruction that addresses a 16-bit memory location indexed by `X` (`$nnnn,X`).
    AbsoluteX,
    /// Three-byte instruction that addresses a 16-bit memory location indexed by `Y` (`$nnnn,Y`).
    AbsoluteY,
    /// Three-byte indirect jump instruction that dereferences a 16-bit pointer (`JMP ($nnnn)`).
    Indirect,
    /// Two-byte indexed indirect instruction that dereferences a zero-page pointer table (`($nn,X)`).
    IndirectX,
    /// Two-byte indirect indexed instruction that dereferences a zero-page base pointer (`($nn),Y`).
    IndirectY,
    /// Sentinel mode for undecodable or unknown opcodes.
    Unknown,
}

/// Describes a single MOS 6502/6510 opcode byte (`0x00..=0xFF`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opcode {
    /// Canonical uppercase instruction mnemonic (such as `"LDA"`, `"STA"`, or `"JSR"`).
    pub mnemonic: &'static str,
    /// Operand [`AddressingMode`] used by this opcode.
    pub mode: AddressingMode,
    /// Total instruction length in bytes (`1..=3`).
    pub size: u8,
    /// Base execution time in CPU clock cycles.
    #[allow(dead_code)]
    pub cycles: u8,
    /// Short summary of the instruction behavior.
    #[allow(dead_code)]
    pub description: &'static str,
    /// Indicates that this opcode is an undocumented 6502 instruction when `true`.
    pub illegal: bool,
}

impl Opcode {
    /// Creates a documented 6502 [`Opcode`] descriptor (`illegal = false`).
    #[must_use]
    pub const fn new(
        mnemonic: &'static str,
        mode: AddressingMode,
        size: u8,
        cycles: u8,
        description: &'static str,
    ) -> Self {
        Self {
            mnemonic,
            mode,
            size,
            cycles,
            description,
            illegal: false,
        }
    }

    /// Creates an undocumented 6502 [`Opcode`] descriptor (`illegal = true`).
    #[must_use]
    pub const fn new_illegal(
        mnemonic: &'static str,
        mode: AddressingMode,
        size: u8,
        cycles: u8,
        description: &'static str,
    ) -> Self {
        Self {
            mnemonic,
            mode,
            size,
            cycles,
            description,
            illegal: true,
        }
    }

    /// Returns `true` when this opcode transfers control to a direct static target address.
    ///
    /// Matches relative branches, `JMP $nnnn`, and `JSR $nnnn`. Excludes `JMP ($nnnn)`, `RTS`,
    /// `RTI`, and `BRK`.
    #[must_use]
    pub fn is_flow_control_with_target(&self) -> bool {
        matches!(self.mode, AddressingMode::Relative)
            || (matches!(self.mnemonic, "JMP" | "JSR") && self.mode == AddressingMode::Absolute)
    }

    /// Classifies the cross-reference access kind ([`XRefKind`]) when this opcode addresses memory.
    ///
    /// # Classification Rules
    ///
    /// - **Non-addressing modes**: Returns `None` for [`AddressingMode::Implied`],
    ///   [`AddressingMode::Accumulator`], [`AddressingMode::Immediate`], and
    ///   [`AddressingMode::Unknown`] because those modes do not address memory.
    /// - **[`XRefKind::Read`]**: Load, bitwise, arithmetic, and compare opcodes
    ///   (`LDA`, `LDX`, `LDY`, `EOR`, `AND`, `ORA`, `ADC`, `SBC`, `CMP`, `CPX`, `CPY`, `BIT`, `LAX`).
    /// - **[`XRefKind::Write`]**: Store opcodes (`STA`, `STX`, `STY`, `SAX`).
    /// - **[`XRefKind::ReadModifyWrite`]**: Memory shift, rotate, increment, and decrement
    ///   opcodes (`ASL`, `LSR`, `ROL`, `ROR`, `INC`, `DEC`, `SLO`, `RLA`, `SRE`, `RRA`, `DCP`, `ISC`, `ISB`).
    /// - **[`XRefKind::Call`]**: Subroutine call (`JSR`).
    /// - **[`XRefKind::Jump`]**: Unconditional jumps (`JMP`, absolute and indirect) and
    ///   relative branches (`BPL`, `BMI`, `BVC`, `BVS`, `BCC`, `BCS`, `BNE`, `BEQ`).
    ///
    /// # Examples
    /// ```
    /// use regenerator2000_core::cpu::{AddressingMode, Opcode, get_opcodes};
    /// use regenerator2000_core::state::XRefKind;
    ///
    /// let opcodes = get_opcodes();
    /// assert_eq!(opcodes[0xAD].as_ref().and_then(Opcode::xref_kind), Some(XRefKind::Read));
    /// assert_eq!(opcodes[0x8D].as_ref().and_then(Opcode::xref_kind), Some(XRefKind::Write));
    /// assert_eq!(opcodes[0x0A].as_ref().and_then(Opcode::xref_kind), None);
    /// assert_eq!(opcodes[0x0E].as_ref().and_then(Opcode::xref_kind), Some(XRefKind::ReadModifyWrite));
    /// assert_eq!(opcodes[0x20].as_ref().and_then(Opcode::xref_kind), Some(XRefKind::Call));
    /// ```
    #[must_use]
    pub fn xref_kind(&self) -> Option<XRefKind> {
        if matches!(
            self.mode,
            AddressingMode::Implied
                | AddressingMode::Accumulator
                | AddressingMode::Immediate
                | AddressingMode::Unknown
        ) {
            return None;
        }

        match self.mnemonic {
            "LDA" | "LDX" | "LDY" | "EOR" | "AND" | "ORA" | "ADC" | "SBC" | "CMP" | "CPX"
            | "CPY" | "BIT" | "LAX" => Some(XRefKind::Read),
            "STA" | "STX" | "STY" | "SAX" => Some(XRefKind::Write),
            "ASL" | "LSR" | "ROL" | "ROR" | "INC" | "DEC" | "SLO" | "RLA" | "SRE" | "RRA"
            | "DCP" | "ISC" | "ISB" => Some(XRefKind::ReadModifyWrite),
            "JSR" => Some(XRefKind::Call),
            "JMP" | "BPL" | "BMI" | "BVC" | "BVS" | "BCC" | "BCS" | "BNE" | "BEQ" => {
                Some(XRefKind::Jump)
            }
            _ => None,
        }
    }
}

/// Returns the 256-entry MOS 6502/6510 opcode table indexed by opcode byte (`0x00..=0xFF`).
#[must_use]
pub fn get_opcodes() -> [Option<Opcode>; 256] {
    use AddressingMode::{
        Absolute, AbsoluteX, AbsoluteY, Accumulator, Immediate, Implied, Indirect, IndirectX,
        IndirectY, Relative, ZeroPage, ZeroPageX, ZeroPageY,
    };

    const fn op(
        mnemonic: &'static str,
        mode: AddressingMode,
        size: u8,
        cycles: u8,
        description: &'static str,
    ) -> Option<Opcode> {
        Some(Opcode::new(mnemonic, mode, size, cycles, description))
    }

    const fn ill(
        mnemonic: &'static str,
        mode: AddressingMode,
        size: u8,
        cycles: u8,
        description: &'static str,
    ) -> Option<Opcode> {
        Some(Opcode::new_illegal(
            mnemonic,
            mode,
            size,
            cycles,
            description,
        ))
    }

    const UNKNOWN: Option<Opcode> = None;
    let mut opcodes = [UNKNOWN; 256];

    // ADC
    opcodes[0x69] = op("ADC", Immediate, 2, 2, "Add with Carry");
    opcodes[0x65] = op("ADC", ZeroPage, 2, 3, "Add with Carry");
    opcodes[0x75] = op("ADC", ZeroPageX, 2, 4, "Add with Carry");
    opcodes[0x6D] = op("ADC", Absolute, 3, 4, "Add with Carry");
    opcodes[0x7D] = op("ADC", AbsoluteX, 3, 4, "Add with Carry");
    opcodes[0x79] = op("ADC", AbsoluteY, 3, 4, "Add with Carry");
    opcodes[0x61] = op("ADC", IndirectX, 2, 6, "Add with Carry");
    opcodes[0x71] = op("ADC", IndirectY, 2, 5, "Add with Carry");

    // AND
    opcodes[0x29] = op("AND", Immediate, 2, 2, "Logical AND");
    opcodes[0x25] = op("AND", ZeroPage, 2, 3, "Logical AND");
    opcodes[0x35] = op("AND", ZeroPageX, 2, 4, "Logical AND");
    opcodes[0x2D] = op("AND", Absolute, 3, 4, "Logical AND");
    opcodes[0x3D] = op("AND", AbsoluteX, 3, 4, "Logical AND");
    opcodes[0x39] = op("AND", AbsoluteY, 3, 4, "Logical AND");
    opcodes[0x21] = op("AND", IndirectX, 2, 6, "Logical AND");
    opcodes[0x31] = op("AND", IndirectY, 2, 5, "Logical AND");

    // ASL
    opcodes[0x0A] = op("ASL", Accumulator, 1, 2, "Arithmetic Shift Left");
    opcodes[0x06] = op("ASL", ZeroPage, 2, 5, "Arithmetic Shift Left");
    opcodes[0x16] = op("ASL", ZeroPageX, 2, 6, "Arithmetic Shift Left");
    opcodes[0x0E] = op("ASL", Absolute, 3, 6, "Arithmetic Shift Left");
    opcodes[0x1E] = op("ASL", AbsoluteX, 3, 7, "Arithmetic Shift Left");

    // Branches and BIT
    opcodes[0x90] = op("BCC", Relative, 2, 2, "Branch if Carry Clear");
    opcodes[0xB0] = op("BCS", Relative, 2, 2, "Branch if Carry Set");
    opcodes[0xF0] = op("BEQ", Relative, 2, 2, "Branch if Equal");
    opcodes[0x24] = op("BIT", ZeroPage, 2, 3, "Bit Test");
    opcodes[0x2C] = op("BIT", Absolute, 3, 4, "Bit Test");
    opcodes[0x30] = op("BMI", Relative, 2, 2, "Branch if Minus");
    opcodes[0xD0] = op("BNE", Relative, 2, 2, "Branch if Not Equal");
    opcodes[0x10] = op("BPL", Relative, 2, 2, "Branch if Positive");
    opcodes[0x00] = op("BRK", Implied, 1, 7, "Force Interrupt");
    opcodes[0x50] = op("BVC", Relative, 2, 2, "Branch if Overflow Clear");
    opcodes[0x70] = op("BVS", Relative, 2, 2, "Branch if Overflow Set");

    // Flag clear instructions
    opcodes[0x18] = op("CLC", Implied, 1, 2, "Clear Carry Flag");
    opcodes[0xD8] = op("CLD", Implied, 1, 2, "Clear Decimal Mode");
    opcodes[0x58] = op("CLI", Implied, 1, 2, "Clear Interrupt Disable");
    opcodes[0xB8] = op("CLV", Implied, 1, 2, "Clear Overflow Flag");

    // CMP, CPX, CPY
    opcodes[0xC9] = op("CMP", Immediate, 2, 2, "Compare");
    opcodes[0xC5] = op("CMP", ZeroPage, 2, 3, "Compare");
    opcodes[0xD5] = op("CMP", ZeroPageX, 2, 4, "Compare");
    opcodes[0xCD] = op("CMP", Absolute, 3, 4, "Compare");
    opcodes[0xDD] = op("CMP", AbsoluteX, 3, 4, "Compare");
    opcodes[0xD9] = op("CMP", AbsoluteY, 3, 4, "Compare");
    opcodes[0xC1] = op("CMP", IndirectX, 2, 6, "Compare");
    opcodes[0xD1] = op("CMP", IndirectY, 2, 5, "Compare");
    opcodes[0xE0] = op("CPX", Immediate, 2, 2, "Compare X Register");
    opcodes[0xE4] = op("CPX", ZeroPage, 2, 3, "Compare X Register");
    opcodes[0xEC] = op("CPX", Absolute, 3, 4, "Compare X Register");
    opcodes[0xC0] = op("CPY", Immediate, 2, 2, "Compare Y Register");
    opcodes[0xC4] = op("CPY", ZeroPage, 2, 3, "Compare Y Register");
    opcodes[0xCC] = op("CPY", Absolute, 3, 4, "Compare Y Register");

    // DEC, DEX, DEY
    opcodes[0xC6] = op("DEC", ZeroPage, 2, 5, "Decrement Memory");
    opcodes[0xD6] = op("DEC", ZeroPageX, 2, 6, "Decrement Memory");
    opcodes[0xCE] = op("DEC", Absolute, 3, 6, "Decrement Memory");
    opcodes[0xDE] = op("DEC", AbsoluteX, 3, 7, "Decrement Memory");
    opcodes[0xCA] = op("DEX", Implied, 1, 2, "Decrement X Register");
    opcodes[0x88] = op("DEY", Implied, 1, 2, "Decrement Y Register");

    // EOR
    opcodes[0x49] = op("EOR", Immediate, 2, 2, "Exclusive OR");
    opcodes[0x45] = op("EOR", ZeroPage, 2, 3, "Exclusive OR");
    opcodes[0x55] = op("EOR", ZeroPageX, 2, 4, "Exclusive OR");
    opcodes[0x4D] = op("EOR", Absolute, 3, 4, "Exclusive OR");
    opcodes[0x5D] = op("EOR", AbsoluteX, 3, 4, "Exclusive OR");
    opcodes[0x59] = op("EOR", AbsoluteY, 3, 4, "Exclusive OR");
    opcodes[0x41] = op("EOR", IndirectX, 2, 6, "Exclusive OR");
    opcodes[0x51] = op("EOR", IndirectY, 2, 5, "Exclusive OR");

    // INC, INX, INY
    opcodes[0xE6] = op("INC", ZeroPage, 2, 5, "Increment Memory");
    opcodes[0xF6] = op("INC", ZeroPageX, 2, 6, "Increment Memory");
    opcodes[0xEE] = op("INC", Absolute, 3, 6, "Increment Memory");
    opcodes[0xFE] = op("INC", AbsoluteX, 3, 7, "Increment Memory");
    opcodes[0xE8] = op("INX", Implied, 1, 2, "Increment X Register");
    opcodes[0xC8] = op("INY", Implied, 1, 2, "Increment Y Register");

    // JMP, JSR
    opcodes[0x4C] = op("JMP", Absolute, 3, 3, "Jump");
    opcodes[0x6C] = op("JMP", Indirect, 3, 5, "Jump");
    opcodes[0x20] = op("JSR", Absolute, 3, 6, "Jump to Subroutine");

    // LDA, LDX, LDY
    opcodes[0xA9] = op("LDA", Immediate, 2, 2, "Load Accumulator");
    opcodes[0xA5] = op("LDA", ZeroPage, 2, 3, "Load Accumulator");
    opcodes[0xB5] = op("LDA", ZeroPageX, 2, 4, "Load Accumulator");
    opcodes[0xAD] = op("LDA", Absolute, 3, 4, "Load Accumulator");
    opcodes[0xBD] = op("LDA", AbsoluteX, 3, 4, "Load Accumulator");
    opcodes[0xB9] = op("LDA", AbsoluteY, 3, 4, "Load Accumulator");
    opcodes[0xA1] = op("LDA", IndirectX, 2, 6, "Load Accumulator");
    opcodes[0xB1] = op("LDA", IndirectY, 2, 5, "Load Accumulator");
    opcodes[0xA2] = op("LDX", Immediate, 2, 2, "Load X Register");
    opcodes[0xA6] = op("LDX", ZeroPage, 2, 3, "Load X Register");
    opcodes[0xB6] = op("LDX", ZeroPageY, 2, 4, "Load X Register");
    opcodes[0xAE] = op("LDX", Absolute, 3, 4, "Load X Register");
    opcodes[0xBE] = op("LDX", AbsoluteY, 3, 4, "Load X Register");
    opcodes[0xA0] = op("LDY", Immediate, 2, 2, "Load Y Register");
    opcodes[0xA4] = op("LDY", ZeroPage, 2, 3, "Load Y Register");
    opcodes[0xB4] = op("LDY", ZeroPageX, 2, 4, "Load Y Register");
    opcodes[0xAC] = op("LDY", Absolute, 3, 4, "Load Y Register");
    opcodes[0xBC] = op("LDY", AbsoluteX, 3, 4, "Load Y Register");

    // LSR, NOP
    opcodes[0x4A] = op("LSR", Accumulator, 1, 2, "Logical Shift Right");
    opcodes[0x46] = op("LSR", ZeroPage, 2, 5, "Logical Shift Right");
    opcodes[0x56] = op("LSR", ZeroPageX, 2, 6, "Logical Shift Right");
    opcodes[0x4E] = op("LSR", Absolute, 3, 6, "Logical Shift Right");
    opcodes[0x5E] = op("LSR", AbsoluteX, 3, 7, "Logical Shift Right");
    opcodes[0xEA] = op("NOP", Implied, 1, 2, "No Operation");

    // ORA
    opcodes[0x09] = op("ORA", Immediate, 2, 2, "Logical Inclusive OR");
    opcodes[0x05] = op("ORA", ZeroPage, 2, 3, "Logical Inclusive OR");
    opcodes[0x15] = op("ORA", ZeroPageX, 2, 4, "Logical Inclusive OR");
    opcodes[0x0D] = op("ORA", Absolute, 3, 4, "Logical Inclusive OR");
    opcodes[0x1D] = op("ORA", AbsoluteX, 3, 4, "Logical Inclusive OR");
    opcodes[0x19] = op("ORA", AbsoluteY, 3, 4, "Logical Inclusive OR");
    opcodes[0x01] = op("ORA", IndirectX, 2, 6, "Logical Inclusive OR");
    opcodes[0x11] = op("ORA", IndirectY, 2, 5, "Logical Inclusive OR");

    // Stack and rotate instructions
    opcodes[0x48] = op("PHA", Implied, 1, 3, "Push Accumulator");
    opcodes[0x08] = op("PHP", Implied, 1, 3, "Push Processor Status");
    opcodes[0x68] = op("PLA", Implied, 1, 4, "Pull Accumulator");
    opcodes[0x28] = op("PLP", Implied, 1, 4, "Pull Processor Status");
    opcodes[0x2A] = op("ROL", Accumulator, 1, 2, "Rotate Left");
    opcodes[0x26] = op("ROL", ZeroPage, 2, 5, "Rotate Left");
    opcodes[0x36] = op("ROL", ZeroPageX, 2, 6, "Rotate Left");
    opcodes[0x2E] = op("ROL", Absolute, 3, 6, "Rotate Left");
    opcodes[0x3E] = op("ROL", AbsoluteX, 3, 7, "Rotate Left");
    opcodes[0x6A] = op("ROR", Accumulator, 1, 2, "Rotate Right");
    opcodes[0x66] = op("ROR", ZeroPage, 2, 5, "Rotate Right");
    opcodes[0x76] = op("ROR", ZeroPageX, 2, 6, "Rotate Right");
    opcodes[0x6E] = op("ROR", Absolute, 3, 6, "Rotate Right");
    opcodes[0x7E] = op("ROR", AbsoluteX, 3, 7, "Rotate Right");
    opcodes[0x40] = op("RTI", Implied, 1, 6, "Return from Interrupt");
    opcodes[0x60] = op("RTS", Implied, 1, 6, "Return from Subroutine");

    // SBC and flag set instructions
    opcodes[0xE9] = op("SBC", Immediate, 2, 2, "Subtract with Carry");
    opcodes[0xE5] = op("SBC", ZeroPage, 2, 3, "Subtract with Carry");
    opcodes[0xF5] = op("SBC", ZeroPageX, 2, 4, "Subtract with Carry");
    opcodes[0xED] = op("SBC", Absolute, 3, 4, "Subtract with Carry");
    opcodes[0xFD] = op("SBC", AbsoluteX, 3, 4, "Subtract with Carry");
    opcodes[0xF9] = op("SBC", AbsoluteY, 3, 4, "Subtract with Carry");
    opcodes[0xE1] = op("SBC", IndirectX, 2, 6, "Subtract with Carry");
    opcodes[0xF1] = op("SBC", IndirectY, 2, 5, "Subtract with Carry");
    opcodes[0x38] = op("SEC", Implied, 1, 2, "Set Carry Flag");
    opcodes[0xF8] = op("SED", Implied, 1, 2, "Set Decimal Flag");
    opcodes[0x78] = op("SEI", Implied, 1, 2, "Set Interrupt Disable");

    // STA, STX, STY, and register transfers
    opcodes[0x85] = op("STA", ZeroPage, 2, 3, "Store Accumulator");
    opcodes[0x95] = op("STA", ZeroPageX, 2, 4, "Store Accumulator");
    opcodes[0x8D] = op("STA", Absolute, 3, 4, "Store Accumulator");
    opcodes[0x9D] = op("STA", AbsoluteX, 3, 5, "Store Accumulator");
    opcodes[0x99] = op("STA", AbsoluteY, 3, 5, "Store Accumulator");
    opcodes[0x81] = op("STA", IndirectX, 2, 6, "Store Accumulator");
    opcodes[0x91] = op("STA", IndirectY, 2, 6, "Store Accumulator");
    opcodes[0x86] = op("STX", ZeroPage, 2, 3, "Store X Register");
    opcodes[0x96] = op("STX", ZeroPageY, 2, 4, "Store X Register");
    opcodes[0x8E] = op("STX", Absolute, 3, 4, "Store X Register");
    opcodes[0x84] = op("STY", ZeroPage, 2, 3, "Store Y Register");
    opcodes[0x94] = op("STY", ZeroPageX, 2, 4, "Store Y Register");
    opcodes[0x8C] = op("STY", Absolute, 3, 4, "Store Y Register");
    opcodes[0xAA] = op("TAX", Implied, 1, 2, "Transfer Accumulator to X");
    opcodes[0xA8] = op("TAY", Implied, 1, 2, "Transfer Accumulator to Y");
    opcodes[0xBA] = op("TSX", Implied, 1, 2, "Transfer Stack Pointer to X");
    opcodes[0x8A] = op("TXA", Implied, 1, 2, "Transfer X to Accumulator");
    opcodes[0x9A] = op("TXS", Implied, 1, 2, "Transfer X to Stack Pointer");
    opcodes[0x98] = op("TYA", Implied, 1, 2, "Transfer Y to Accumulator");

    // Undocumented opcodes: SLO, RLA, SRE, RRA, SAX, LAX, DCP, ISC, ANC, ASR, ARR, SBX
    opcodes[0x07] = ill("SLO", ZeroPage, 2, 5, "ASL + ORA");
    opcodes[0x17] = ill("SLO", ZeroPageX, 2, 6, "ASL + ORA");
    opcodes[0x03] = ill("SLO", IndirectX, 2, 8, "ASL + ORA");
    opcodes[0x13] = ill("SLO", IndirectY, 2, 8, "ASL + ORA");
    opcodes[0x0F] = ill("SLO", Absolute, 3, 6, "ASL + ORA");
    opcodes[0x1F] = ill("SLO", AbsoluteX, 3, 7, "ASL + ORA");
    opcodes[0x1B] = ill("SLO", AbsoluteY, 3, 7, "ASL + ORA");

    opcodes[0x27] = ill("RLA", ZeroPage, 2, 5, "ROL + AND");
    opcodes[0x37] = ill("RLA", ZeroPageX, 2, 6, "ROL + AND");
    opcodes[0x23] = ill("RLA", IndirectX, 2, 8, "ROL + AND");
    opcodes[0x33] = ill("RLA", IndirectY, 2, 8, "ROL + AND");
    opcodes[0x2F] = ill("RLA", Absolute, 3, 6, "ROL + AND");
    opcodes[0x3F] = ill("RLA", AbsoluteX, 3, 7, "ROL + AND");
    opcodes[0x3B] = ill("RLA", AbsoluteY, 3, 7, "ROL + AND");

    opcodes[0x47] = ill("SRE", ZeroPage, 2, 5, "LSR + EOR");
    opcodes[0x57] = ill("SRE", ZeroPageX, 2, 6, "LSR + EOR");
    opcodes[0x43] = ill("SRE", IndirectX, 2, 8, "LSR + EOR");
    opcodes[0x53] = ill("SRE", IndirectY, 2, 8, "LSR + EOR");
    opcodes[0x4F] = ill("SRE", Absolute, 3, 6, "LSR + EOR");
    opcodes[0x5F] = ill("SRE", AbsoluteX, 3, 7, "LSR + EOR");
    opcodes[0x5B] = ill("SRE", AbsoluteY, 3, 7, "LSR + EOR");

    opcodes[0x67] = ill("RRA", ZeroPage, 2, 5, "ROR + ADC");
    opcodes[0x77] = ill("RRA", ZeroPageX, 2, 6, "ROR + ADC");
    opcodes[0x63] = ill("RRA", IndirectX, 2, 8, "ROR + ADC");
    opcodes[0x73] = ill("RRA", IndirectY, 2, 8, "ROR + ADC");
    opcodes[0x6F] = ill("RRA", Absolute, 3, 6, "ROR + ADC");
    opcodes[0x7F] = ill("RRA", AbsoluteX, 3, 7, "ROR + ADC");
    opcodes[0x7B] = ill("RRA", AbsoluteY, 3, 7, "ROR + ADC");

    opcodes[0x87] = ill("SAX", ZeroPage, 2, 3, "Store A & X");
    opcodes[0x97] = ill("SAX", ZeroPageY, 2, 4, "Store A & X");
    opcodes[0x83] = ill("SAX", IndirectX, 2, 6, "Store A & X");
    opcodes[0x8F] = ill("SAX", Absolute, 3, 4, "Store A & X");

    opcodes[0xA7] = ill("LAX", ZeroPage, 2, 3, "Load A and X");
    opcodes[0xB7] = ill("LAX", ZeroPageY, 2, 4, "Load A and X");
    opcodes[0xA3] = ill("LAX", IndirectX, 2, 6, "Load A and X");
    opcodes[0xB3] = ill("LAX", IndirectY, 2, 5, "Load A and X");
    opcodes[0xAF] = ill("LAX", Absolute, 3, 4, "Load A and X");
    opcodes[0xBF] = ill("LAX", AbsoluteY, 3, 4, "Load A and X");
    opcodes[0xAB] = ill("LAX", Immediate, 2, 2, "Load A and X (Immediate)");

    opcodes[0xC7] = ill("DCP", ZeroPage, 2, 5, "DEC + CMP");
    opcodes[0xD7] = ill("DCP", ZeroPageX, 2, 6, "DEC + CMP");
    opcodes[0xC3] = ill("DCP", IndirectX, 2, 8, "DEC + CMP");
    opcodes[0xD3] = ill("DCP", IndirectY, 2, 8, "DEC + CMP");
    opcodes[0xCF] = ill("DCP", Absolute, 3, 6, "DEC + CMP");
    opcodes[0xDF] = ill("DCP", AbsoluteX, 3, 7, "DEC + CMP");
    opcodes[0xDB] = ill("DCP", AbsoluteY, 3, 7, "DEC + CMP");

    opcodes[0xE7] = ill("ISC", ZeroPage, 2, 5, "INC + SBC");
    opcodes[0xF7] = ill("ISC", ZeroPageX, 2, 6, "INC + SBC");
    opcodes[0xE3] = ill("ISC", IndirectX, 2, 8, "INC + SBC");
    opcodes[0xF3] = ill("ISC", IndirectY, 2, 8, "INC + SBC");
    opcodes[0xEF] = ill("ISC", Absolute, 3, 6, "INC + SBC");
    opcodes[0xFF] = ill("ISC", AbsoluteX, 3, 7, "INC + SBC");
    opcodes[0xFB] = ill("ISC", AbsoluteY, 3, 7, "INC + SBC");

    opcodes[0x0B] = ill("ANC", Immediate, 2, 2, "AND #imm + Carry");
    opcodes[0x2B] = ill("ANC", Immediate, 2, 2, "AND #imm + Carry");
    opcodes[0x4B] = ill("ASR", Immediate, 2, 2, "AND #imm + LSR A");
    opcodes[0x6B] = ill("ARR", Immediate, 2, 2, "AND #imm + ROR A");
    opcodes[0xCB] = ill("SBX", Immediate, 2, 2, "(A & X) - #imm -> X");

    opcodes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opcode_xref_kind_documented_opcodes() {
        let opcodes = get_opcodes();

        let read_mnemonics = [
            "LDA", "LDX", "LDY", "EOR", "AND", "ORA", "ADC", "SBC", "CMP", "CPX", "CPY", "BIT",
        ];
        let write_mnemonics = ["STA", "STX", "STY"];
        let rmw_mnemonics = ["ASL", "LSR", "ROL", "ROR", "INC", "DEC"];
        let branch_mnemonics = ["BPL", "BMI", "BVC", "BVS", "BCC", "BCS", "BNE", "BEQ"];
        let implied_mnemonics = [
            "NOP", "RTS", "RTI", "BRK", "PHA", "PLA", "PHP", "PLP", "CLC", "SEC", "CLI", "SEI",
            "CLV", "CLD", "SED", "TAX", "TXA", "DEX", "INX", "TAY", "TYA", "DEY", "INY", "TXS",
            "TSX",
        ];

        let mut documented_count = 0usize;

        for (byte, maybe_opcode) in opcodes.iter().enumerate() {
            let Some(opcode) = maybe_opcode else {
                continue;
            };
            if opcode.illegal {
                continue;
            }
            documented_count += 1;

            let expected = match opcode.mode {
                AddressingMode::Implied
                | AddressingMode::Accumulator
                | AddressingMode::Immediate
                | AddressingMode::Unknown => None,
                AddressingMode::ZeroPage
                | AddressingMode::ZeroPageX
                | AddressingMode::ZeroPageY
                | AddressingMode::Absolute
                | AddressingMode::AbsoluteX
                | AddressingMode::AbsoluteY
                | AddressingMode::IndirectX
                | AddressingMode::IndirectY => {
                    if read_mnemonics.contains(&opcode.mnemonic) {
                        Some(XRefKind::Read)
                    } else if write_mnemonics.contains(&opcode.mnemonic) {
                        Some(XRefKind::Write)
                    } else if rmw_mnemonics.contains(&opcode.mnemonic) {
                        Some(XRefKind::ReadModifyWrite)
                    } else if opcode.mnemonic == "JSR" {
                        Some(XRefKind::Call)
                    } else if opcode.mnemonic == "JMP" {
                        Some(XRefKind::Jump)
                    } else {
                        panic!(
                            "Unexpected documented memory opcode ${byte:02X}: {} {:?}",
                            opcode.mnemonic, opcode.mode
                        );
                    }
                }
                AddressingMode::Relative => {
                    assert!(
                        branch_mnemonics.contains(&opcode.mnemonic),
                        "Unexpected relative opcode ${byte:02X}: {}",
                        opcode.mnemonic
                    );
                    Some(XRefKind::Jump)
                }
                AddressingMode::Indirect => {
                    assert_eq!(opcode.mnemonic, "JMP");
                    Some(XRefKind::Jump)
                }
            };

            assert_eq!(
                opcode.xref_kind(),
                expected,
                "Mismatch for documented opcode ${byte:02X} ({} {:?})",
                opcode.mnemonic,
                opcode.mode
            );
        }

        // Standard 6502 has 151 documented opcodes
        assert_eq!(documented_count, 151);

        // Explicit spot checks required by the Test Plan:
        // JSR Absolute ($20) -> Call
        assert_eq!(
            opcodes[0x20].as_ref().unwrap().xref_kind(),
            Some(XRefKind::Call)
        );
        // JMP Absolute ($4C) and Indirect ($6C) -> Jump
        assert_eq!(
            opcodes[0x4C].as_ref().unwrap().xref_kind(),
            Some(XRefKind::Jump)
        );
        assert_eq!(
            opcodes[0x6C].as_ref().unwrap().xref_kind(),
            Some(XRefKind::Jump)
        );
        // ASL/LSR/ROL/ROR Accumulator ($0A, $4A, $2A, $6A) -> None
        for acc_op in [0x0A, 0x4A, 0x2A, 0x6A] {
            let op = opcodes[acc_op].as_ref().unwrap();
            assert_eq!(op.mode, AddressingMode::Accumulator);
            assert_eq!(op.xref_kind(), None);
        }
        // Implied instructions -> None
        for mnemonic in implied_mnemonics {
            let found = opcodes
                .iter()
                .flatten()
                .find(|op| !op.illegal && op.mnemonic == mnemonic)
                .unwrap_or_else(|| panic!("Missing documented implied mnemonic {mnemonic}"));
            assert_eq!(found.xref_kind(), None);
        }
    }

    #[test]
    fn test_opcode_xref_kind_illegal_undocumented_opcodes() {
        let opcodes = get_opcodes();

        // Read: LAX ($A7, $B7, $AF, $BF, $A3, $B3)
        for code in [0xA7, 0xB7, 0xAF, 0xBF, 0xA3, 0xB3] {
            let op = opcodes[code].as_ref().unwrap();
            assert!(op.illegal);
            assert_eq!(op.mnemonic, "LAX");
            assert_eq!(
                op.xref_kind(),
                Some(XRefKind::Read),
                "LAX ${code:02X} should be Read"
            );
        }

        // LAX Immediate ($AB) has no memory target -> None
        assert_eq!(opcodes[0xAB].as_ref().unwrap().xref_kind(), None);

        // Write: SAX ($87, $97, $8F, $83)
        for code in [0x87, 0x97, 0x8F, 0x83] {
            let op = opcodes[code].as_ref().unwrap();
            assert!(op.illegal);
            assert_eq!(op.mnemonic, "SAX");
            assert_eq!(
                op.xref_kind(),
                Some(XRefKind::Write),
                "SAX ${code:02X} should be Write"
            );
        }

        // ReadModifyWrite: SLO, RLA, SRE, RRA, DCP, ISC
        let rmw_sets: &[(&str, &[usize])] = &[
            ("SLO", &[0x07, 0x17, 0x0F, 0x1F, 0x1B, 0x03, 0x13]),
            ("RLA", &[0x27, 0x37, 0x2F, 0x3F, 0x3B, 0x23, 0x33]),
            ("SRE", &[0x47, 0x57, 0x4F, 0x5F, 0x5B, 0x43, 0x53]),
            ("RRA", &[0x67, 0x77, 0x6F, 0x7F, 0x7B, 0x63, 0x73]),
            ("DCP", &[0xC7, 0xD7, 0xCF, 0xDF, 0xDB, 0xC3, 0xD3]),
            ("ISC", &[0xE7, 0xF7, 0xEF, 0xFF, 0xFB, 0xE3, 0xF3]),
        ];

        for (expected_mnemonic, codes) in rmw_sets {
            for &code in *codes {
                let op = opcodes[code].as_ref().unwrap();
                assert!(op.illegal);
                assert_eq!(op.mnemonic, *expected_mnemonic);
                assert_eq!(
                    op.xref_kind(),
                    Some(XRefKind::ReadModifyWrite),
                    "{expected_mnemonic} ${code:02X} should be ReadModifyWrite"
                );
            }
        }

        // Verify "ISB" alias for "ISC" is also classified as ReadModifyWrite
        let isb_opcode = Opcode::new_illegal("ISB", AddressingMode::Absolute, 3, 6, "INC + SBC");
        assert_eq!(isb_opcode.xref_kind(), Some(XRefKind::ReadModifyWrite));

        // Illegal immediate opcodes (ANC, ASR, ARR, SBX) -> None
        for code in [0x0B, 0x2B, 0x4B, 0x6B, 0xCB] {
            let op = opcodes[code].as_ref().unwrap();
            assert!(op.illegal);
            assert_eq!(op.xref_kind(), None);
        }
    }
}
