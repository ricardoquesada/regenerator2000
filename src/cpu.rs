#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum AddressingMode {
    Implied,
    Accumulator,
    Immediate,
    ZeroPage,
    ZeroPageX,
    ZeroPageY,
    Relative,
    Absolute,
    AbsoluteX,
    AbsoluteY,
    Indirect,
    IndirectX,
    IndirectY,
    // For undocumented or special cases if needed
    Unknown,
}

#[derive(Debug, Clone)]
pub struct Opcode {
    pub mnemonic: &'static str,
    pub mode: AddressingMode,
    pub size: u8,
    #[allow(dead_code)]
    pub cycles: u8,
    #[allow(dead_code)]
    pub description: &'static str,
    pub illegal: bool,
}

impl Opcode {
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
}

pub fn get_opcodes() -> [Option<Opcode>; 256] {
    const UNKNOWN: Option<Opcode> = None;
    let mut opcodes = [UNKNOWN; 256];

    // ADC
    opcodes[0x69] = Some(Opcode::new(
        "ADC",
        AddressingMode::Immediate,
        2,
        2,
        "Add with Carry",
    ));
    opcodes[0x65] = Some(Opcode::new(
        "ADC",
        AddressingMode::ZeroPage,
        2,
        3,
        "Add with Carry",
    ));
    opcodes[0x75] = Some(Opcode::new(
        "ADC",
        AddressingMode::ZeroPageX,
        2,
        4,
        "Add with Carry",
    ));
    opcodes[0x6D] = Some(Opcode::new(
        "ADC",
        AddressingMode::Absolute,
        3,
        4,
        "Add with Carry",
    ));
    opcodes[0x7D] = Some(Opcode::new(
        "ADC",
        AddressingMode::AbsoluteX,
        3,
        4,
        "Add with Carry",
    )); // +1 if page crossed
    opcodes[0x79] = Some(Opcode::new(
        "ADC",
        AddressingMode::AbsoluteY,
        3,
        4,
        "Add with Carry",
    )); // +1 if page crossed
    opcodes[0x61] = Some(Opcode::new(
        "ADC",
        AddressingMode::IndirectX,
        2,
        6,
        "Add with Carry",
    ));
    opcodes[0x71] = Some(Opcode::new(
        "ADC",
        AddressingMode::IndirectY,
        2,
        5,
        "Add with Carry",
    )); // +1 if page crossed

    // AND
    opcodes[0x29] = Some(Opcode::new(
        "AND",
        AddressingMode::Immediate,
        2,
        2,
        "Logical AND",
    ));
    opcodes[0x25] = Some(Opcode::new(
        "AND",
        AddressingMode::ZeroPage,
        2,
        3,
        "Logical AND",
    ));
    opcodes[0x35] = Some(Opcode::new(
        "AND",
        AddressingMode::ZeroPageX,
        2,
        4,
        "Logical AND",
    ));
    opcodes[0x2D] = Some(Opcode::new(
        "AND",
        AddressingMode::Absolute,
        3,
        4,
        "Logical AND",
    ));
    opcodes[0x3D] = Some(Opcode::new(
        "AND",
        AddressingMode::AbsoluteX,
        3,
        4,
        "Logical AND",
    )); // +1 if page crossed
    opcodes[0x39] = Some(Opcode::new(
        "AND",
        AddressingMode::AbsoluteY,
        3,
        4,
        "Logical AND",
    )); // +1 if page crossed
    opcodes[0x21] = Some(Opcode::new(
        "AND",
        AddressingMode::IndirectX,
        2,
        6,
        "Logical AND",
    ));
    opcodes[0x31] = Some(Opcode::new(
        "AND",
        AddressingMode::IndirectY,
        2,
        5,
        "Logical AND",
    )); // +1 if page crossed

    // ASL
    opcodes[0x0A] = Some(Opcode::new(
        "ASL",
        AddressingMode::Accumulator,
        1,
        2,
        "Arithmetic Shift Left",
    ));
    opcodes[0x06] = Some(Opcode::new(
        "ASL",
        AddressingMode::ZeroPage,
        2,
        5,
        "Arithmetic Shift Left",
    ));
    opcodes[0x16] = Some(Opcode::new(
        "ASL",
        AddressingMode::ZeroPageX,
        2,
        6,
        "Arithmetic Shift Left",
    ));
    opcodes[0x0E] = Some(Opcode::new(
        "ASL",
        AddressingMode::Absolute,
        3,
        6,
        "Arithmetic Shift Left",
    ));
    opcodes[0x1E] = Some(Opcode::new(
        "ASL",
        AddressingMode::AbsoluteX,
        3,
        7,
        "Arithmetic Shift Left",
    ));

    // BCC
    opcodes[0x90] = Some(Opcode::new(
        "BCC",
        AddressingMode::Relative,
        2,
        2,
        "Branch if Carry Clear",
    )); // +1 if branch taken, +2 if to new page

    // BCS
    opcodes[0xB0] = Some(Opcode::new(
        "BCS",
        AddressingMode::Relative,
        2,
        2,
        "Branch if Carry Set",
    ));

    // BEQ
    opcodes[0xF0] = Some(Opcode::new(
        "BEQ",
        AddressingMode::Relative,
        2,
        2,
        "Branch if Equal",
    ));

    // BIT
    opcodes[0x24] = Some(Opcode::new(
        "BIT",
        AddressingMode::ZeroPage,
        2,
        3,
        "Bit Test",
    ));
    opcodes[0x2C] = Some(Opcode::new(
        "BIT",
        AddressingMode::Absolute,
        3,
        4,
        "Bit Test",
    ));

    // BMI
    opcodes[0x30] = Some(Opcode::new(
        "BMI",
        AddressingMode::Relative,
        2,
        2,
        "Branch if Minus",
    ));

    // BNE
    opcodes[0xD0] = Some(Opcode::new(
        "BNE",
        AddressingMode::Relative,
        2,
        2,
        "Branch if Not Equal",
    ));

    // BPL
    opcodes[0x10] = Some(Opcode::new(
        "BPL",
        AddressingMode::Relative,
        2,
        2,
        "Branch if Positive",
    ));

    // BRK
    opcodes[0x00] = Some(Opcode::new(
        "BRK",
        AddressingMode::Implied,
        1,
        7,
        "Force Interrupt",
    ));

    // BVC
    opcodes[0x50] = Some(Opcode::new(
        "BVC",
        AddressingMode::Relative,
        2,
        2,
        "Branch if Overflow Clear",
    ));

    // BVS
    opcodes[0x70] = Some(Opcode::new(
        "BVS",
        AddressingMode::Relative,
        2,
        2,
        "Branch if Overflow Set",
    ));

    // CLC
    opcodes[0x18] = Some(Opcode::new(
        "CLC",
        AddressingMode::Implied,
        1,
        2,
        "Clear Carry Flag",
    ));

    // CLD
    opcodes[0xD8] = Some(Opcode::new(
        "CLD",
        AddressingMode::Implied,
        1,
        2,
        "Clear Decimal Mode",
    ));

    // CLI
    opcodes[0x58] = Some(Opcode::new(
        "CLI",
        AddressingMode::Implied,
        1,
        2,
        "Clear Interrupt Disable",
    ));

    // CLV
    opcodes[0xB8] = Some(Opcode::new(
        "CLV",
        AddressingMode::Implied,
        1,
        2,
        "Clear Overflow Flag",
    ));

    // CMP
    opcodes[0xC9] = Some(Opcode::new(
        "CMP",
        AddressingMode::Immediate,
        2,
        2,
        "Compare",
    ));
    opcodes[0xC5] = Some(Opcode::new(
        "CMP",
        AddressingMode::ZeroPage,
        2,
        3,
        "Compare",
    ));
    opcodes[0xD5] = Some(Opcode::new(
        "CMP",
        AddressingMode::ZeroPageX,
        2,
        4,
        "Compare",
    ));
    opcodes[0xCD] = Some(Opcode::new(
        "CMP",
        AddressingMode::Absolute,
        3,
        4,
        "Compare",
    ));
    opcodes[0xDD] = Some(Opcode::new(
        "CMP",
        AddressingMode::AbsoluteX,
        3,
        4,
        "Compare",
    ));
    opcodes[0xD9] = Some(Opcode::new(
        "CMP",
        AddressingMode::AbsoluteY,
        3,
        4,
        "Compare",
    ));
    opcodes[0xC1] = Some(Opcode::new(
        "CMP",
        AddressingMode::IndirectX,
        2,
        6,
        "Compare",
    ));
    opcodes[0xD1] = Some(Opcode::new(
        "CMP",
        AddressingMode::IndirectY,
        2,
        5,
        "Compare",
    ));

    // CPX
    opcodes[0xE0] = Some(Opcode::new(
        "CPX",
        AddressingMode::Immediate,
        2,
        2,
        "Compare X Register",
    ));
    opcodes[0xE4] = Some(Opcode::new(
        "CPX",
        AddressingMode::ZeroPage,
        2,
        3,
        "Compare X Register",
    ));
    opcodes[0xEC] = Some(Opcode::new(
        "CPX",
        AddressingMode::Absolute,
        3,
        4,
        "Compare X Register",
    ));

    // CPY
    opcodes[0xC0] = Some(Opcode::new(
        "CPY",
        AddressingMode::Immediate,
        2,
        2,
        "Compare Y Register",
    ));
    opcodes[0xC4] = Some(Opcode::new(
        "CPY",
        AddressingMode::ZeroPage,
        2,
        3,
        "Compare Y Register",
    ));
    opcodes[0xCC] = Some(Opcode::new(
        "CPY",
        AddressingMode::Absolute,
        3,
        4,
        "Compare Y Register",
    ));

    // DEC
    opcodes[0xC6] = Some(Opcode::new(
        "DEC",
        AddressingMode::ZeroPage,
        2,
        5,
        "Decrement Memory",
    ));
    opcodes[0xD6] = Some(Opcode::new(
        "DEC",
        AddressingMode::ZeroPageX,
        2,
        6,
        "Decrement Memory",
    ));
    opcodes[0xCE] = Some(Opcode::new(
        "DEC",
        AddressingMode::Absolute,
        3,
        6,
        "Decrement Memory",
    ));
    opcodes[0xDE] = Some(Opcode::new(
        "DEC",
        AddressingMode::AbsoluteX,
        3,
        7,
        "Decrement Memory",
    ));

    // DEX
    opcodes[0xCA] = Some(Opcode::new(
        "DEX",
        AddressingMode::Implied,
        1,
        2,
        "Decrement X Register",
    ));

    // DEY
    opcodes[0x88] = Some(Opcode::new(
        "DEY",
        AddressingMode::Implied,
        1,
        2,
        "Decrement Y Register",
    ));

    // EOR
    opcodes[0x49] = Some(Opcode::new(
        "EOR",
        AddressingMode::Immediate,
        2,
        2,
        "Exclusive OR",
    ));
    opcodes[0x45] = Some(Opcode::new(
        "EOR",
        AddressingMode::ZeroPage,
        2,
        3,
        "Exclusive OR",
    ));
    opcodes[0x55] = Some(Opcode::new(
        "EOR",
        AddressingMode::ZeroPageX,
        2,
        4,
        "Exclusive OR",
    ));
    opcodes[0x4D] = Some(Opcode::new(
        "EOR",
        AddressingMode::Absolute,
        3,
        4,
        "Exclusive OR",
    ));
    opcodes[0x5D] = Some(Opcode::new(
        "EOR",
        AddressingMode::AbsoluteX,
        3,
        4,
        "Exclusive OR",
    ));
    opcodes[0x59] = Some(Opcode::new(
        "EOR",
        AddressingMode::AbsoluteY,
        3,
        4,
        "Exclusive OR",
    ));
    opcodes[0x41] = Some(Opcode::new(
        "EOR",
        AddressingMode::IndirectX,
        2,
        6,
        "Exclusive OR",
    ));
    opcodes[0x51] = Some(Opcode::new(
        "EOR",
        AddressingMode::IndirectY,
        2,
        5,
        "Exclusive OR",
    ));

    // INC
    opcodes[0xE6] = Some(Opcode::new(
        "INC",
        AddressingMode::ZeroPage,
        2,
        5,
        "Increment Memory",
    ));
    opcodes[0xF6] = Some(Opcode::new(
        "INC",
        AddressingMode::ZeroPageX,
        2,
        6,
        "Increment Memory",
    ));
    opcodes[0xEE] = Some(Opcode::new(
        "INC",
        AddressingMode::Absolute,
        3,
        6,
        "Increment Memory",
    ));
    opcodes[0xFE] = Some(Opcode::new(
        "INC",
        AddressingMode::AbsoluteX,
        3,
        7,
        "Increment Memory",
    ));

    // INX
    opcodes[0xE8] = Some(Opcode::new(
        "INX",
        AddressingMode::Implied,
        1,
        2,
        "Increment X Register",
    ));

    // INY
    opcodes[0xC8] = Some(Opcode::new(
        "INY",
        AddressingMode::Implied,
        1,
        2,
        "Increment Y Register",
    ));

    // JMP
    opcodes[0x4C] = Some(Opcode::new("JMP", AddressingMode::Absolute, 3, 3, "Jump"));
    opcodes[0x6C] = Some(Opcode::new("JMP", AddressingMode::Indirect, 3, 5, "Jump"));

    // JSR
    opcodes[0x20] = Some(Opcode::new(
        "JSR",
        AddressingMode::Absolute,
        3,
        6,
        "Jump to Subroutine",
    ));

    // LDA
    opcodes[0xA9] = Some(Opcode::new(
        "LDA",
        AddressingMode::Immediate,
        2,
        2,
        "Load Accumulator",
    ));
    opcodes[0xA5] = Some(Opcode::new(
        "LDA",
        AddressingMode::ZeroPage,
        2,
        3,
        "Load Accumulator",
    ));
    opcodes[0xB5] = Some(Opcode::new(
        "LDA",
        AddressingMode::ZeroPageX,
        2,
        4,
        "Load Accumulator",
    ));
    opcodes[0xAD] = Some(Opcode::new(
        "LDA",
        AddressingMode::Absolute,
        3,
        4,
        "Load Accumulator",
    ));
    opcodes[0xBD] = Some(Opcode::new(
        "LDA",
        AddressingMode::AbsoluteX,
        3,
        4,
        "Load Accumulator",
    ));
    opcodes[0xB9] = Some(Opcode::new(
        "LDA",
        AddressingMode::AbsoluteY,
        3,
        4,
        "Load Accumulator",
    ));
    opcodes[0xA1] = Some(Opcode::new(
        "LDA",
        AddressingMode::IndirectX,
        2,
        6,
        "Load Accumulator",
    ));
    opcodes[0xB1] = Some(Opcode::new(
        "LDA",
        AddressingMode::IndirectY,
        2,
        5,
        "Load Accumulator",
    ));

    // LDX
    opcodes[0xA2] = Some(Opcode::new(
        "LDX",
        AddressingMode::Immediate,
        2,
        2,
        "Load X Register",
    ));
    opcodes[0xA6] = Some(Opcode::new(
        "LDX",
        AddressingMode::ZeroPage,
        2,
        3,
        "Load X Register",
    ));
    opcodes[0xB6] = Some(Opcode::new(
        "LDX",
        AddressingMode::ZeroPageY,
        2,
        4,
        "Load X Register",
    ));
    opcodes[0xAE] = Some(Opcode::new(
        "LDX",
        AddressingMode::Absolute,
        3,
        4,
        "Load X Register",
    ));
    opcodes[0xBE] = Some(Opcode::new(
        "LDX",
        AddressingMode::AbsoluteY,
        3,
        4,
        "Load X Register",
    ));

    // LDY
    opcodes[0xA0] = Some(Opcode::new(
        "LDY",
        AddressingMode::Immediate,
        2,
        2,
        "Load Y Register",
    ));
    opcodes[0xA4] = Some(Opcode::new(
        "LDY",
        AddressingMode::ZeroPage,
        2,
        3,
        "Load Y Register",
    ));
    opcodes[0xB4] = Some(Opcode::new(
        "LDY",
        AddressingMode::ZeroPageX,
        2,
        4,
        "Load Y Register",
    ));
    opcodes[0xAC] = Some(Opcode::new(
        "LDY",
        AddressingMode::Absolute,
        3,
        4,
        "Load Y Register",
    ));
    opcodes[0xBC] = Some(Opcode::new(
        "LDY",
        AddressingMode::AbsoluteX,
        3,
        4,
        "Load Y Register",
    ));

    // LSR
    opcodes[0x4A] = Some(Opcode::new(
        "LSR",
        AddressingMode::Accumulator,
        1,
        2,
        "Logical Shift Right",
    ));
    opcodes[0x46] = Some(Opcode::new(
        "LSR",
        AddressingMode::ZeroPage,
        2,
        5,
        "Logical Shift Right",
    ));
    opcodes[0x56] = Some(Opcode::new(
        "LSR",
        AddressingMode::ZeroPageX,
        2,
        6,
        "Logical Shift Right",
    ));
    opcodes[0x4E] = Some(Opcode::new(
        "LSR",
        AddressingMode::Absolute,
        3,
        6,
        "Logical Shift Right",
    ));
    opcodes[0x5E] = Some(Opcode::new(
        "LSR",
        AddressingMode::AbsoluteX,
        3,
        7,
        "Logical Shift Right",
    ));

    // NOP
    opcodes[0xEA] = Some(Opcode::new(
        "NOP",
        AddressingMode::Implied,
        1,
        2,
        "No Operation",
    ));

    // ORA
    opcodes[0x09] = Some(Opcode::new(
        "ORA",
        AddressingMode::Immediate,
        2,
        2,
        "Logical Inclusive OR",
    ));
    opcodes[0x05] = Some(Opcode::new(
        "ORA",
        AddressingMode::ZeroPage,
        2,
        3,
        "Logical Inclusive OR",
    ));
    opcodes[0x15] = Some(Opcode::new(
        "ORA",
        AddressingMode::ZeroPageX,
        2,
        4,
        "Logical Inclusive OR",
    ));
    opcodes[0x0D] = Some(Opcode::new(
        "ORA",
        AddressingMode::Absolute,
        3,
        4,
        "Logical Inclusive OR",
    ));
    opcodes[0x1D] = Some(Opcode::new(
        "ORA",
        AddressingMode::AbsoluteX,
        3,
        4,
        "Logical Inclusive OR",
    ));
    opcodes[0x19] = Some(Opcode::new(
        "ORA",
        AddressingMode::AbsoluteY,
        3,
        4,
        "Logical Inclusive OR",
    ));
    opcodes[0x01] = Some(Opcode::new(
        "ORA",
        AddressingMode::IndirectX,
        2,
        6,
        "Logical Inclusive OR",
    ));
    opcodes[0x11] = Some(Opcode::new(
        "ORA",
        AddressingMode::IndirectY,
        2,
        5,
        "Logical Inclusive OR",
    ));

    // PHA
    opcodes[0x48] = Some(Opcode::new(
        "PHA",
        AddressingMode::Implied,
        1,
        3,
        "Push Accumulator",
    ));

    // PHP
    opcodes[0x08] = Some(Opcode::new(
        "PHP",
        AddressingMode::Implied,
        1,
        3,
        "Push Processor Status",
    ));

    // PLA
    opcodes[0x68] = Some(Opcode::new(
        "PLA",
        AddressingMode::Implied,
        1,
        4,
        "Pull Accumulator",
    ));

    // PLP
    opcodes[0x28] = Some(Opcode::new(
        "PLP",
        AddressingMode::Implied,
        1,
        4,
        "Pull Processor Status",
    ));

    // ROL
    opcodes[0x2A] = Some(Opcode::new(
        "ROL",
        AddressingMode::Accumulator,
        1,
        2,
        "Rotate Left",
    ));
    opcodes[0x26] = Some(Opcode::new(
        "ROL",
        AddressingMode::ZeroPage,
        2,
        5,
        "Rotate Left",
    ));
    opcodes[0x36] = Some(Opcode::new(
        "ROL",
        AddressingMode::ZeroPageX,
        2,
        6,
        "Rotate Left",
    ));
    opcodes[0x2E] = Some(Opcode::new(
        "ROL",
        AddressingMode::Absolute,
        3,
        6,
        "Rotate Left",
    ));
    opcodes[0x3E] = Some(Opcode::new(
        "ROL",
        AddressingMode::AbsoluteX,
        3,
        7,
        "Rotate Left",
    ));

    // ROR
    opcodes[0x6A] = Some(Opcode::new(
        "ROR",
        AddressingMode::Accumulator,
        1,
        2,
        "Rotate Right",
    ));
    opcodes[0x66] = Some(Opcode::new(
        "ROR",
        AddressingMode::ZeroPage,
        2,
        5,
        "Rotate Right",
    ));
    opcodes[0x76] = Some(Opcode::new(
        "ROR",
        AddressingMode::ZeroPageX,
        2,
        6,
        "Rotate Right",
    ));
    opcodes[0x6E] = Some(Opcode::new(
        "ROR",
        AddressingMode::Absolute,
        3,
        6,
        "Rotate Right",
    ));
    opcodes[0x7E] = Some(Opcode::new(
        "ROR",
        AddressingMode::AbsoluteX,
        3,
        7,
        "Rotate Right",
    ));

    // RTI
    opcodes[0x40] = Some(Opcode::new(
        "RTI",
        AddressingMode::Implied,
        1,
        6,
        "Return from Interrupt",
    ));

    // RTS
    opcodes[0x60] = Some(Opcode::new(
        "RTS",
        AddressingMode::Implied,
        1,
        6,
        "Return from Subroutine",
    ));

    // SBC
    opcodes[0xE9] = Some(Opcode::new(
        "SBC",
        AddressingMode::Immediate,
        2,
        2,
        "Subtract with Carry",
    ));
    opcodes[0xE5] = Some(Opcode::new(
        "SBC",
        AddressingMode::ZeroPage,
        2,
        3,
        "Subtract with Carry",
    ));
    opcodes[0xF5] = Some(Opcode::new(
        "SBC",
        AddressingMode::ZeroPageX,
        2,
        4,
        "Subtract with Carry",
    ));
    opcodes[0xED] = Some(Opcode::new(
        "SBC",
        AddressingMode::Absolute,
        3,
        4,
        "Subtract with Carry",
    ));
    opcodes[0xFD] = Some(Opcode::new(
        "SBC",
        AddressingMode::AbsoluteX,
        3,
        4,
        "Subtract with Carry",
    ));
    opcodes[0xF9] = Some(Opcode::new(
        "SBC",
        AddressingMode::AbsoluteY,
        3,
        4,
        "Subtract with Carry",
    ));
    opcodes[0xE1] = Some(Opcode::new(
        "SBC",
        AddressingMode::IndirectX,
        2,
        6,
        "Subtract with Carry",
    ));
    opcodes[0xF1] = Some(Opcode::new(
        "SBC",
        AddressingMode::IndirectY,
        2,
        5,
        "Subtract with Carry",
    ));

    // SEC
    opcodes[0x38] = Some(Opcode::new(
        "SEC",
        AddressingMode::Implied,
        1,
        2,
        "Set Carry Flag",
    ));

    // SED
    opcodes[0xF8] = Some(Opcode::new(
        "SED",
        AddressingMode::Implied,
        1,
        2,
        "Set Decimal Flag",
    ));

    // SEI
    opcodes[0x78] = Some(Opcode::new(
        "SEI",
        AddressingMode::Implied,
        1,
        2,
        "Set Interrupt Disable",
    ));

    // STA
    opcodes[0x85] = Some(Opcode::new(
        "STA",
        AddressingMode::ZeroPage,
        2,
        3,
        "Store Accumulator",
    ));
    opcodes[0x95] = Some(Opcode::new(
        "STA",
        AddressingMode::ZeroPageX,
        2,
        4,
        "Store Accumulator",
    ));
    opcodes[0x8D] = Some(Opcode::new(
        "STA",
        AddressingMode::Absolute,
        3,
        4,
        "Store Accumulator",
    ));
    opcodes[0x9D] = Some(Opcode::new(
        "STA",
        AddressingMode::AbsoluteX,
        3,
        5,
        "Store Accumulator",
    ));
    opcodes[0x99] = Some(Opcode::new(
        "STA",
        AddressingMode::AbsoluteY,
        3,
        5,
        "Store Accumulator",
    ));
    opcodes[0x81] = Some(Opcode::new(
        "STA",
        AddressingMode::IndirectX,
        2,
        6,
        "Store Accumulator",
    ));
    opcodes[0x91] = Some(Opcode::new(
        "STA",
        AddressingMode::IndirectY,
        2,
        6,
        "Store Accumulator",
    ));

    // STX
    opcodes[0x86] = Some(Opcode::new(
        "STX",
        AddressingMode::ZeroPage,
        2,
        3,
        "Store X Register",
    ));
    opcodes[0x96] = Some(Opcode::new(
        "STX",
        AddressingMode::ZeroPageY,
        2,
        4,
        "Store X Register",
    ));
    opcodes[0x8E] = Some(Opcode::new(
        "STX",
        AddressingMode::Absolute,
        3,
        4,
        "Store X Register",
    ));

    // STY
    opcodes[0x84] = Some(Opcode::new(
        "STY",
        AddressingMode::ZeroPage,
        2,
        3,
        "Store Y Register",
    ));
    opcodes[0x94] = Some(Opcode::new(
        "STY",
        AddressingMode::ZeroPageX,
        2,
        4,
        "Store Y Register",
    ));
    opcodes[0x8C] = Some(Opcode::new(
        "STY",
        AddressingMode::Absolute,
        3,
        4,
        "Store Y Register",
    ));

    // TAX
    opcodes[0xAA] = Some(Opcode::new(
        "TAX",
        AddressingMode::Implied,
        1,
        2,
        "Transfer Accumulator to X",
    ));

    // TAY
    opcodes[0xA8] = Some(Opcode::new(
        "TAY",
        AddressingMode::Implied,
        1,
        2,
        "Transfer Accumulator to Y",
    ));

    // TSX
    opcodes[0xBA] = Some(Opcode::new(
        "TSX",
        AddressingMode::Implied,
        1,
        2,
        "Transfer Stack Pointer to X",
    ));

    // TXA
    opcodes[0x8A] = Some(Opcode::new(
        "TXA",
        AddressingMode::Implied,
        1,
        2,
        "Transfer X to Accumulator",
    ));

    // TXS
    opcodes[0x9A] = Some(Opcode::new(
        "TXS",
        AddressingMode::Implied,
        1,
        2,
        "Transfer X to Stack Pointer",
    ));

    // TYA
    opcodes[0x98] = Some(Opcode::new(
        "TYA",
        AddressingMode::Implied,
        1,
        2,
        "Transfer Y to Accumulator",
    ));

    // ========================================================================
    // UNDOCUMENTED OPCODES
    // ========================================================================

    // SLO (Shift Left then OR) -> ASL + ORA
    let mut add_slo = |opcode, mode, size, cycles| {
        opcodes[opcode] = Some(Opcode::new_illegal("SLO", mode, size, cycles, "ASL + ORA"));
    };
    add_slo(0x07, AddressingMode::ZeroPage, 2, 5);
    add_slo(0x17, AddressingMode::ZeroPageX, 2, 6);
    add_slo(0x03, AddressingMode::IndirectX, 2, 8);
    add_slo(0x13, AddressingMode::IndirectY, 2, 8);
    add_slo(0x0F, AddressingMode::Absolute, 3, 6);
    add_slo(0x1F, AddressingMode::AbsoluteX, 3, 7);
    add_slo(0x1B, AddressingMode::AbsoluteY, 3, 7);

    // RLA (Rotate Left then AND) -> ROL + AND
    let mut add_rla = |opcode, mode, size, cycles| {
        opcodes[opcode] = Some(Opcode::new_illegal("RLA", mode, size, cycles, "ROL + AND"));
    };
    add_rla(0x27, AddressingMode::ZeroPage, 2, 5);
    add_rla(0x37, AddressingMode::ZeroPageX, 2, 6);
    add_rla(0x23, AddressingMode::IndirectX, 2, 8);
    add_rla(0x33, AddressingMode::IndirectY, 2, 8);
    add_rla(0x2F, AddressingMode::Absolute, 3, 6);
    add_rla(0x3F, AddressingMode::AbsoluteX, 3, 7);
    add_rla(0x3B, AddressingMode::AbsoluteY, 3, 7);

    // SRE (Logical Shift Right then EOR) -> LSR + EOR
    let mut add_sre = |opcode, mode, size, cycles| {
        opcodes[opcode] = Some(Opcode::new_illegal("SRE", mode, size, cycles, "LSR + EOR"));
    };
    add_sre(0x47, AddressingMode::ZeroPage, 2, 5);
    add_sre(0x57, AddressingMode::ZeroPageX, 2, 6);
    add_sre(0x43, AddressingMode::IndirectX, 2, 8);
    add_sre(0x53, AddressingMode::IndirectY, 2, 8);
    add_sre(0x4F, AddressingMode::Absolute, 3, 6);
    add_sre(0x5F, AddressingMode::AbsoluteX, 3, 7);
    add_sre(0x5B, AddressingMode::AbsoluteY, 3, 7);

    // RRA (Rotate Right then ADC) -> ROR + ADC
    let mut add_rra = |opcode, mode, size, cycles| {
        opcodes[opcode] = Some(Opcode::new_illegal("RRA", mode, size, cycles, "ROR + ADC"));
    };
    add_rra(0x67, AddressingMode::ZeroPage, 2, 5);
    add_rra(0x77, AddressingMode::ZeroPageX, 2, 6);
    add_rra(0x63, AddressingMode::IndirectX, 2, 8);
    add_rra(0x73, AddressingMode::IndirectY, 2, 8);
    add_rra(0x6F, AddressingMode::Absolute, 3, 6);
    add_rra(0x7F, AddressingMode::AbsoluteX, 3, 7);
    add_rra(0x7B, AddressingMode::AbsoluteY, 3, 7);

    // SAX (Store A AND X) -> A & X -> Mem
    let mut add_sax = |opcode, mode, size, cycles| {
        opcodes[opcode] = Some(Opcode::new_illegal(
            "SAX",
            mode,
            size,
            cycles,
            "Store A & X",
        ));
    };
    add_sax(0x87, AddressingMode::ZeroPage, 2, 3);
    add_sax(0x97, AddressingMode::ZeroPageY, 2, 4); // Note: ZeroPageY for SAX
    add_sax(0x83, AddressingMode::IndirectX, 2, 6);
    add_sax(0x8F, AddressingMode::Absolute, 3, 4);

    // LAX (Load A AND X) -> Mem -> A, Mem -> X
    let mut add_lax = |opcode, mode, size, cycles| {
        opcodes[opcode] = Some(Opcode::new_illegal(
            "LAX",
            mode,
            size,
            cycles,
            "Load A and X",
        ));
    };
    add_lax(0xA7, AddressingMode::ZeroPage, 2, 3);
    add_lax(0xB7, AddressingMode::ZeroPageY, 2, 4); // Note: ZeroPageY
    add_lax(0xA3, AddressingMode::IndirectX, 2, 6);
    add_lax(0xB3, AddressingMode::IndirectY, 2, 5);
    add_lax(0xAF, AddressingMode::Absolute, 3, 4);
    add_lax(0xBF, AddressingMode::AbsoluteY, 3, 4);

    // DCP (Decrement then Compare) -> DEC + CMP
    let mut add_dcp = |opcode, mode, size, cycles| {
        opcodes[opcode] = Some(Opcode::new_illegal("DCP", mode, size, cycles, "DEC + CMP"));
    };
    add_dcp(0xC7, AddressingMode::ZeroPage, 2, 5);
    add_dcp(0xD7, AddressingMode::ZeroPageX, 2, 6);
    add_dcp(0xC3, AddressingMode::IndirectX, 2, 8);
    add_dcp(0xD3, AddressingMode::IndirectY, 2, 8);
    add_dcp(0xCF, AddressingMode::Absolute, 3, 6);
    add_dcp(0xDF, AddressingMode::AbsoluteX, 3, 7);
    add_dcp(0xDB, AddressingMode::AbsoluteY, 3, 7);

    // ISC (Increment then Subtract) -> INC + SBC
    let mut add_isc = |opcode, mode, size, cycles| {
        opcodes[opcode] = Some(Opcode::new_illegal("ISC", mode, size, cycles, "INC + SBC"));
    };
    add_isc(0xE7, AddressingMode::ZeroPage, 2, 5);
    add_isc(0xF7, AddressingMode::ZeroPageX, 2, 6);
    add_isc(0xE3, AddressingMode::IndirectX, 2, 8);
    add_isc(0xF3, AddressingMode::IndirectY, 2, 8);
    add_isc(0xEF, AddressingMode::Absolute, 3, 6);
    add_isc(0xFF, AddressingMode::AbsoluteX, 3, 7);
    add_isc(0xFB, AddressingMode::AbsoluteY, 3, 7);

    // ANC (aka AAC) - AND #imm then update Carry (some sources say it does ASL too implicitly on internal register?)
    // This opcode is ANC #imm. It performs AND #imm, and updates N and Z.
    // AND it also moves bit 7 of the result into the Carry flag.
    let mut add_anc = |opcode, mode, size, cycles| {
        opcodes[opcode] = Some(Opcode::new_illegal(
            "ANC",
            mode,
            size,
            cycles,
            "AND #imm + Carry",
        ));
    };
    add_anc(0x0B, AddressingMode::Immediate, 2, 2);
    add_anc(0x2B, AddressingMode::Immediate, 2, 2);

    // ASR (aka ALR) - AND #imm then LSR A.
    let mut add_asr = |opcode, mode, size, cycles| {
        opcodes[opcode] = Some(Opcode::new_illegal(
            "ASR",
            mode,
            size,
            cycles,
            "AND #imm + LSR A",
        ));
    };
    add_asr(0x4B, AddressingMode::Immediate, 2, 2);

    // ARR - AND #imm then ROR A (with some weird C flag behavior involving Decimal mode on real hardware)
    let mut add_arr = |opcode, mode, size, cycles| {
        opcodes[opcode] = Some(Opcode::new_illegal(
            "ARR",
            mode,
            size,
            cycles,
            "AND #imm + ROR A",
        ));
    };
    add_arr(0x6B, AddressingMode::Immediate, 2, 2);

    // SBX (aka AXS) - (A & X) - #imm -> X
    // CMP (A&X) #imm ... sets flags. Result stored in X.
    let mut add_sbx = |opcode, mode, size, cycles| {
        opcodes[opcode] = Some(Opcode::new_illegal(
            "SBX",
            mode,
            size,
            cycles,
            "(A & X) - #imm -> X",
        ));
    };
    add_sbx(0xCB, AddressingMode::Immediate, 2, 2);

    // LAX Immediate - Not usually listed as stable LAX, or sometimes called OAL / ATX / LXA.
    // However unstable, some docs say $AB is LAX #imm.
    // "LAX #$00" for $AB $00
    // It loads A and X with the same immediate value (unstable, depends on line noise/temperature sometimes?
    // but often simplified as A=X=imm).
    // User specifically asked for $AB to be LAX (Immediate).
    let mut add_lax_imm = |opcode, mode, size, cycles| {
        opcodes[opcode] = Some(Opcode::new_illegal(
            "LAX",
            mode,
            size,
            cycles,
            "Load A and X (Immediate)",
        ));
    };
    add_lax_imm(0xAB, AddressingMode::Immediate, 2, 2);

    opcodes
}
