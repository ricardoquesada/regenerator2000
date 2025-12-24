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
            AddressingMode::Accumulator => "a".to_string(),
            AddressingMode::Immediate => format!("#${:02x}", operands[0]),
            AddressingMode::ZeroPage => {
                let addr = operands[0] as u16;
                if let Some(name) = get_label(addr, LabelType::ZeroPageAbsoluteAddress) {
                    name
                } else {
                    format!("${:02x}", addr)
                }
            }
            AddressingMode::ZeroPageX => {
                let addr = operands[0] as u16;
                if let Some(name) = get_label(addr, LabelType::ZeroPageField) {
                    format!("{},x", name) // ACME is case insensitive but often convention is lowercase regs
                } else {
                    format!("${:02x},x", addr)
                }
            }
            AddressingMode::ZeroPageY => {
                let addr = operands[0] as u16;
                if let Some(name) = get_label(addr, LabelType::ZeroPageField) {
                    format!("{},y", name)
                } else {
                    format!("${:02x},y", addr)
                }
            }
            AddressingMode::Relative => {
                let offset = operands[0] as i8;
                let target = address.wrapping_add(2).wrapping_add(offset as u16);
                if let Some(name) = get_label(target, LabelType::Branch) {
                    name
                } else {
                    format!("${:04x}", target)
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

                if let Some(name) = get_label(addr, l_type) {
                    name
                } else {
                    format!("${:04x}", addr)
                }
            }
            AddressingMode::AbsoluteX => {
                let addr = (operands[1] as u16) << 8 | (operands[0] as u16);
                if let Some(name) = get_label(addr, LabelType::Field) {
                    format!("{},x", name)
                } else {
                    format!("${:04x},x", addr)
                }
            }
            AddressingMode::AbsoluteY => {
                let addr = (operands[1] as u16) << 8 | (operands[0] as u16);
                if let Some(name) = get_label(addr, LabelType::Field) {
                    format!("{},y", name)
                } else {
                    format!("${:04x},y", addr)
                }
            }

            AddressingMode::Indirect => {
                let addr = (operands[1] as u16) << 8 | (operands[0] as u16);
                if let Some(name) = get_label(addr, LabelType::Pointer) {
                    format!("({})", name)
                } else {
                    format!("(${:04x})", addr)
                }
            }
            AddressingMode::IndirectX => {
                let addr = operands[0] as u16;
                if let Some(name) = get_label(addr, LabelType::ZeroPagePointer) {
                    format!("({},x)", name)
                } else {
                    format!("(${:02x},x)", addr)
                }
            }
            AddressingMode::IndirectY => {
                let addr = operands[0] as u16;
                if let Some(name) = get_label(addr, LabelType::ZeroPagePointer) {
                    format!("({}),y", name)
                } else {
                    format!("(${:02x}),y", addr)
                }
            }

            AddressingMode::Unknown => "???".to_string(),
        }
    }

    fn format_mnemonic(&self, mnemonic: &str) -> String {
        mnemonic.to_lowercase()
    }

    fn format_label(&self, name: &str) -> String {
        name.to_string()
    }

    fn format_byte(&self, byte: u8) -> String {
        format!("${:02x}", byte)
    }

    fn format_word(&self, word: u16) -> String {
        format!("${:04x}", word)
    }

    fn format_text(
        &self,
        fragments: &[super::formatter::TextFragment],
        _is_start: bool,
        _is_end: bool,
    ) -> Vec<(String, String, bool)> {
        use super::formatter::TextFragment;
        let mut parts = Vec::new();
        for fragment in fragments {
            match fragment {
                TextFragment::Text(s) => {
                    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
                    parts.push(format!("\"{}\"", escaped))
                }
                TextFragment::Byte(b) => parts.push(format!("${:02x}", b)),
            }
        }
        vec![("!text".to_string(), parts.join(", "), true)]
    }

    fn format_screencode_pre(&self) -> Vec<(String, String)> {
        Vec::new()
    }

    fn format_screencode(
        &self,
        fragments: &[super::formatter::TextFragment],
    ) -> Vec<(String, String, bool)> {
        use super::formatter::TextFragment;
        let mut parts = Vec::new();
        for fragment in fragments {
            match fragment {
                TextFragment::Text(s) => {
                    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
                    parts.push(format!("\"{}\"", escaped))
                }
                TextFragment::Byte(b) => parts.push(format!("${:02x}", b)),
            }
        }
        vec![("!scr".to_string(), parts.join(", "), true)]
    }

    fn format_screencode_post(&self) -> Vec<(String, String)> {
        Vec::new()
    }

    fn format_header_origin(&self, origin: u16) -> String {
        format!("* = ${:04x}", origin)
    }

    fn format_definition(&self, name: &str, value: u16, is_zp: bool) -> String {
        let operand = if is_zp && value <= 0xFF {
            format!("${:02x}", value)
        } else {
            format!("${:04x}", value)
        };
        format!("{} = {}", name, operand)
    }

    fn format_instruction(
        &self,
        opcode: &Opcode,
        operands: &[u8],
        address: u16,
        target_context: Option<LabelType>,
        labels: &HashMap<u16, Vec<Label>>,
        settings: &crate::state::DocumentSettings,
    ) -> (String, String) {
        let mnemonic = self.format_mnemonic(&opcode.mnemonic);
        let operand =
            self.format_operand(opcode, operands, address, target_context, labels, settings);

        // Check if we need to force 16-bit addressing with +2
        // Only if settings.use_w_prefix is true AND address fits in ZP (<= 0xFF)
        // And addressing mode is Absolute, AbsoluteX, or AbsoluteY
        if settings.preserve_long_bytes {
            let should_force = match opcode.mode {
                AddressingMode::Absolute
                | AddressingMode::AbsoluteX
                | AddressingMode::AbsoluteY => {
                    if operands.len() >= 2 {
                        let addr = (operands[1] as u16) << 8 | (operands[0] as u16);
                        addr <= 0xFF
                    } else {
                        false
                    }
                }
                _ => false,
            };

            if should_force {
                return (format!("{}+2", mnemonic), operand);
            }
        }

        (mnemonic, operand)
    }
}
