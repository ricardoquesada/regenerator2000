use super::formatter::Formatter;
use crate::cpu::{AddressingMode, Opcode};
use crate::state::{Label, LabelType};
use std::collections::HashMap;

pub struct TassFormatter;

impl Formatter for TassFormatter {
    fn byte_directive(&self) -> &'static str {
        ".BYTE"
    }

    fn word_directive(&self) -> &'static str {
        ".WORD"
    }

    fn format_byte(&self, byte: u8) -> String {
        format!("${:02X}", byte)
    }

    fn format_operand(
        &self,
        opcode: &Opcode,
        operands: &[u8],
        address: u16,
        target_context: Option<LabelType>,
        labels: &HashMap<u16, Vec<Label>>,
        settings: &crate::state::DocumentSettings,
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
            AddressingMode::Accumulator => "A".to_string(),
            AddressingMode::Immediate => format!("#${:02X}", operands[0]),
            AddressingMode::ZeroPage => {
                let addr = operands[0] as u16; // Zero page address
                if let Some(name) = get_label(addr, LabelType::ZeroPageAbsoluteAddress) {
                    name
                } else {
                    format!("${:02X}", addr)
                }
            }
            AddressingMode::ZeroPageX => {
                let addr = operands[0] as u16;
                if let Some(name) = get_label(addr, LabelType::ZeroPageField) {
                    format!("{},X", name)
                } else {
                    format!("${:02X},X", addr)
                }
            }
            AddressingMode::ZeroPageY => {
                let addr = operands[0] as u16;
                if let Some(name) = get_label(addr, LabelType::ZeroPageField) {
                    format!("{},Y", name)
                } else {
                    format!("${:02X},Y", addr)
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

                let base = if let Some(name) = get_label(addr, l_type) {
                    name
                } else {
                    format!("${:04X}", addr)
                };

                if settings.preserve_long_bytes && addr <= 0xFF {
                    format!("@w {}", base)
                } else {
                    base
                }
            }
            AddressingMode::AbsoluteX => {
                let addr = (operands[1] as u16) << 8 | (operands[0] as u16);
                let base = if let Some(name) = get_label(addr, LabelType::Field) {
                    format!("{},X", name)
                } else {
                    format!("${:04X},X", addr)
                };

                if settings.preserve_long_bytes && addr <= 0xFF {
                    format!("@w {}", base)
                } else {
                    base
                }
            }
            AddressingMode::AbsoluteY => {
                let addr = (operands[1] as u16) << 8 | (operands[0] as u16);
                let base = if let Some(name) = get_label(addr, LabelType::Field) {
                    format!("{},Y", name)
                } else {
                    format!("${:04X},Y", addr)
                };

                if settings.preserve_long_bytes && addr <= 0xFF {
                    format!("@w {}", base)
                } else {
                    base
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
                    format!("({},X)", name)
                } else {
                    format!("(${:02X},X)", addr)
                }
            }
            AddressingMode::IndirectY => {
                let addr = operands[0] as u16;
                if let Some(name) = get_label(addr, LabelType::ZeroPagePointer) {
                    format!("({}),Y", name)
                } else {
                    format!("(${:02X}),Y", addr)
                }
            }

            AddressingMode::Unknown => "???".to_string(),
        }
    }

    fn format_mnemonic(&self, mnemonic: &str) -> String {
        mnemonic.to_string()
    }

    fn format_label(&self, name: &str) -> String {
        name.to_string()
    }

    fn format_text(
        &self,
        fragments: &[super::formatter::TextFragment],
        is_start: bool,
        is_end: bool,
    ) -> Vec<(String, String, bool)> {
        use super::formatter::TextFragment;
        let mut lines = Vec::new();

        if is_start {
            lines.push((".ENCODE".to_string(), String::new(), false));
            lines.push((".ENC".to_string(), "\"NONE\"".to_string(), false));
        }

        let mut parts = Vec::new();
        for fragment in fragments {
            match fragment {
                TextFragment::Text(s) => {
                    let escaped = s.replace('"', "\"\"");
                    parts.push(format!("\"{}\"", escaped))
                }
                TextFragment::Byte(b) => parts.push(format!("${:02X}", b)),
            }
        }
        lines.push((".TEXT".to_string(), parts.join(", "), true));

        if is_end {
            lines.push((".ENDENCODE".to_string(), String::new(), false));
        }

        lines
    }

    fn format_screencode_pre(&self) -> Vec<(String, String)> {
        vec![
            (".ENCODE".to_string(), String::new()),
            (".ENC".to_string(), "\"SCREEN\"".to_string()),
        ]
    }

    fn format_screencode(
        &self,
        fragments: &[super::formatter::TextFragment],
    ) -> Vec<(String, String, bool)> {
        use super::formatter::TextFragment;
        let mut lines = Vec::new();
        let mut parts = Vec::new();
        for fragment in fragments {
            match fragment {
                TextFragment::Text(s) => {
                    let escaped = s.replace('"', "\"\"");
                    parts.push(format!("\"{}\"", escaped))
                }
                TextFragment::Byte(b) => parts.push(format!("${:02X}", b)),
            }
        }
        lines.push((".TEXT".to_string(), parts.join(", "), true));
        lines
    }

    fn format_screencode_post(&self) -> Vec<(String, String)> {
        vec![(".ENDENCODE".to_string(), String::new())]
    }

    fn format_header_origin(&self, origin: u16) -> String {
        format!("* = ${:04X}", origin)
    }

    fn format_definition(&self, name: &str, value: u16, is_zp: bool) -> String {
        let operand = if is_zp && value <= 0xFF {
            format!("${:02X}", value)
        } else {
            format!("${:04X}", value)
        };
        format!("{} = {}", name, operand)
    }
}
