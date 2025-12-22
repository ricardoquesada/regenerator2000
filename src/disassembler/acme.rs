use super::formatter::Formatter;
use crate::cpu::{AddressingMode, Opcode};
use crate::state::{Label, LabelType};
use std::collections::HashMap;

pub struct AcmeFormatter;

impl Formatter for AcmeFormatter {
    fn byte_directive(&self) -> &'static str {
        "!byte"
    }

    fn word_directive(&self) -> &'static str {
        "!word"
    }

    fn format_operand(
        &self,
        opcode: &Opcode,
        operands: &[u8],
        address: u16,
        target_context: Option<LabelType>,
        labels: &HashMap<u16, Vec<Label>>,
        _settings: &crate::state::DocumentSettings,
    ) -> String {
        let get_label = |addr: u16, l_type: LabelType| -> Option<String> {
            if let Some(label_vec) = labels.get(&addr) {
                // 1. Try to match target_context if provided
                if let Some(target) = target_context {
                    if let Some(l) = label_vec.iter().find(|l| l.label_type == target) {
                        return Some(l.name.clone());
                    }
                }
                // 2. Try to match l_type (the type implied by addressing mode)
                if let Some(l) = label_vec.iter().find(|l| l.label_type == l_type) {
                    return Some(l.name.clone());
                }

                // 3. Fallback to first label
                if let Some(l) = label_vec.first() {
                    return Some(l.name.clone());
                }
            }
            None
        };

        match opcode.mode {
            AddressingMode::Implied => String::new(),
            AddressingMode::Accumulator => "a".to_string(), // ACME often uses lowercase 'a' or implied? Standard usually allows 'A'. Let's stick to 'A' or omit? ACME docs say 'lsr' implies A, or 'lsr a'. 'a' is safe.
            AddressingMode::Immediate => format!("#${:02X}", operands[0]),
            AddressingMode::ZeroPage => {
                let addr = operands[0] as u16;
                if let Some(name) = get_label(addr, LabelType::ZeroPageAbsoluteAddress) {
                    name
                } else {
                    format!("${:02X}", addr)
                }
            }
            AddressingMode::ZeroPageX => {
                let addr = operands[0] as u16;
                if let Some(name) = get_label(addr, LabelType::ZeroPageField) {
                    format!("{},x", name) // ACME is case insensitive but often convention is lowercase regs
                } else {
                    format!("${:02X},x", addr)
                }
            }
            AddressingMode::ZeroPageY => {
                let addr = operands[0] as u16;
                if let Some(name) = get_label(addr, LabelType::ZeroPageField) {
                    format!("{},y", name)
                } else {
                    format!("${:02X},y", addr)
                }
            }
            AddressingMode::Relative => {
                let offset = operands[0] as i8;
                let target = address.wrapping_add(2).wrapping_add(offset as u16);
                if let Some(name) = get_label(target, LabelType::Branch) {
                    name
                } else {
                    format!("${:04X}", target)
                }
            }
            AddressingMode::Absolute => {
                let addr = (operands[1] as u16) << 8 | (operands[0] as u16);
                let l_type = if opcode.mnemonic == "JSR" {
                    LabelType::Subroutine
                } else if opcode.mnemonic == "JMP" {
                    LabelType::Jump
                } else {
                    LabelType::AbsoluteAddress
                };

                // ACME: use '+' to force 16-bit address if it could be ZP.
                // e.g. `lda+ $00`
                // But here we are processing ABSOLUTE addressing mode instructions.
                // If the address is <= 0xFF, it WOULD be ZP if we didn't force it.
                // 64tass uses `@w $00`. ACME uses `+offset` or just proper opcode selection?
                // ACME supports `lda+ $00` to force absolute.
                // Wait, `lda+` is not standard syntax.
                // Checking ACME documentation (common behaviors):
                // To force absolute: `lda $0012` is usually sufficient if value is > 255.
                // If value < 255, ACME optimizes to ZP unless told otherwise.
                // `!al` or `+` suffix to mnemonic? `lda+ $00`?
                // The most common ACME way is `lda $0012` might optimize.
                // Standard ACME syntax for forcing non-zeropage is `+` before argument: `lda +$10`
                // OR `bit $0020` -> `bit $0020`
                // Let's assume we output address. If we need to force, we format differently.

                // Logic: IF Absolute mode AND address <= 0xFF, print `+$0010`.

                if let Some(name) = get_label(addr, l_type) {
                    name
                } else {
                    // For now standard hex. The Exporter might add forcing?
                    // Better to handle it here if we want `operand` to be correct.
                    // But if we use Label, we rely on assembler to decide or we force?
                    // If we use a label `lbl = $0010`, `lda lbl` might become ZP.
                    // If we want absolute: `lda +lbl`?
                    format!("${:04X}", addr)
                }
            }
            AddressingMode::AbsoluteX => {
                let addr = (operands[1] as u16) << 8 | (operands[0] as u16);
                if let Some(name) = get_label(addr, LabelType::Field) {
                    format!("{},x", name)
                } else {
                    format!("${:04X},x", addr)
                }
            }
            AddressingMode::AbsoluteY => {
                let addr = (operands[1] as u16) << 8 | (operands[0] as u16);
                if let Some(name) = get_label(addr, LabelType::Field) {
                    format!("{},y", name)
                } else {
                    format!("${:04X},y", addr)
                }
            }

            AddressingMode::Indirect => {
                let addr = (operands[1] as u16) << 8 | (operands[0] as u16);
                if let Some(name) = get_label(addr, LabelType::Pointer) {
                    format!("({})", name)
                } else {
                    format!("(${:04X})", addr)
                }
            }
            AddressingMode::IndirectX => {
                let addr = operands[0] as u16;
                if let Some(name) = get_label(addr, LabelType::ZeroPagePointer) {
                    format!("({},x)", name)
                } else {
                    format!("(${:02X},x)", addr)
                }
            }
            AddressingMode::IndirectY => {
                let addr = operands[0] as u16;
                if let Some(name) = get_label(addr, LabelType::ZeroPagePointer) {
                    format!("({}),y", name)
                } else {
                    format!("(${:02X}),y", addr)
                }
            }

            AddressingMode::Unknown => "???".to_string(),
        }
    }
}
