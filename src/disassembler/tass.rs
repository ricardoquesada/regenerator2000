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

                // Check for @w forcing
                // Only if settings.use_w_prefix is true AND address fits in ZP (<= 0xFF)
                // This logic mirrors what was in exporter.rs
                if settings.use_w_prefix && addr <= 0xFF {
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

                if settings.use_w_prefix && addr <= 0xFF {
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

                if settings.use_w_prefix && addr <= 0xFF {
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

    fn format_byte(&self, byte: u8) -> String {
        format!("${:02X}", byte)
    }

    fn format_word(&self, word: u16) -> String {
        format!("${:04X}", word)
    }

    fn format_text(
        &self,
        bytes: &[u8],
        text: &str,
        is_start: bool,
        is_end: bool,
    ) -> Vec<(String, String, bool)> {
        let mut lines = Vec::new();

        if is_start {
            lines.push((".ENCODE".to_string(), String::new(), false));
            lines.push((".ENC".to_string(), "\"ASCII\"".to_string(), false)); // Or "NONE", but "ASCII" usually implies raw
        }

        // Special handling for single byte logic if needed, but for now standard text
        if bytes.len() == 1 {
            lines.push((".BYTE".to_string(), format!("${:02X}", bytes[0]), true));
        } else {
            lines.push((".TEXT".to_string(), format!("\"{}\"", text), true));
        }

        if is_end {
            lines.push((".ENDENCODE".to_string(), String::new(), false));
        }

        lines
    }

    fn format_screencode(
        &self,
        bytes: &[u8],
        text: &str,
        is_start: bool,
        is_end: bool,
    ) -> Vec<(String, String, bool)> {
        let mut lines = Vec::new();

        if is_start {
            lines.push((".ENCODE".to_string(), String::new(), false));
            lines.push((".ENC \"SCREEN\"".to_string(), String::new(), false));
        }

        // Special handling for single byte or non-printable blocks
        if bytes.len() == 1 {
            lines.push((".BYTE".to_string(), format!("${:02X}", bytes[0]), true));
        } else {
            lines.push((".TEXT".to_string(), format!("\"{}\"", text), true));
        }

        if is_end {
            lines.push((".ENDENCODE".to_string(), String::new(), false));
        }

        lines
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
