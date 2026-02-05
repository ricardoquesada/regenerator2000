use super::formatter::Formatter;
use crate::cpu::AddressingMode;
use crate::state::LabelType;

pub struct TassFormatter;

impl Formatter for TassFormatter {
    fn comment_prefix(&self) -> &'static str {
        ";"
    }

    fn byte_directive(&self) -> &'static str {
        ".byte"
    }

    fn word_directive(&self) -> &'static str {
        ".word"
    }

    fn format_byte(&self, byte: u8) -> String {
        format!("${:02x}", byte)
    }

    fn format_address(&self, address: u16) -> String {
        format!("${:04x}", address)
    }

    fn format_operand(&self, ctx: &super::formatter::FormatContext) -> String {
        let opcode = ctx.opcode;
        let operands = ctx.operands;
        let address = ctx.address;
        let _target_context = ctx.target_context;
        let _labels = ctx.labels;
        let settings = ctx.settings;
        let immediate_value_formats = ctx.immediate_value_formats;
        let get_label = |addr: u16, _l_type: LabelType| -> Option<String> {
            ctx.resolve_label(addr).map(|l| l.name.clone())
        };

        match opcode.mode {
            AddressingMode::Implied => String::new(),
            AddressingMode::Accumulator => "a".to_string(),
            AddressingMode::Immediate => {
                let val = operands[0];
                match immediate_value_formats.get(&address) {
                    Some(crate::state::ImmediateFormat::InvertedHex) => {
                        format!("#~${:02x}", !val)
                    }
                    Some(crate::state::ImmediateFormat::Decimal) => format!("#{}", val),
                    Some(crate::state::ImmediateFormat::NegativeDecimal) => {
                        format!("#{}", val as i8)
                    }
                    Some(crate::state::ImmediateFormat::Binary) => format!("#%{:08b}", val),
                    Some(crate::state::ImmediateFormat::InvertedBinary) => {
                        format!("#~%{:08b}", !val)
                    }
                    Some(crate::state::ImmediateFormat::LowByte(target)) => {
                        let name =
                            get_label(*target, LabelType::AbsoluteAddress).unwrap_or_else(|| {
                                if *target <= 0xFF {
                                    format!("${:02x}", target)
                                } else {
                                    format!("${:04x}", target)
                                }
                            });
                        format!("#<{}", name)
                    }
                    Some(crate::state::ImmediateFormat::HighByte(target)) => {
                        let name =
                            get_label(*target, LabelType::AbsoluteAddress).unwrap_or_else(|| {
                                if *target <= 0xFF {
                                    format!("${:02x}", target)
                                } else {
                                    format!("${:04x}", target)
                                }
                            });
                        format!("#>{}", name)
                    }
                    _ => format!("#${:02x}", val),
                }
            }
            AddressingMode::ZeroPage => {
                let addr = operands[0] as u16; // Zero page address
                if let Some(name) = get_label(addr, LabelType::ZeroPageAbsoluteAddress) {
                    name
                } else {
                    format!("${:02x}", addr)
                }
            }
            AddressingMode::ZeroPageX => {
                let addr = operands[0] as u16;
                if let Some(name) = get_label(addr, LabelType::ZeroPageField) {
                    format!("{},x", name)
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

                let base = if let Some(name) = get_label(addr, l_type) {
                    name
                } else {
                    format!("${:04x}", addr)
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
                    format!("{},x", name)
                } else {
                    format!("${:04x},x", addr)
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
                    format!("{},y", name)
                } else {
                    format!("${:04x},y", addr)
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

    fn format_label_definition(&self, name: &str) -> String {
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
            lines.push((".encode".to_string(), String::new(), false));
            lines.push((".enc".to_string(), "\"none\"".to_string(), false));
        }

        let mut parts = Vec::new();
        for fragment in fragments {
            match fragment {
                TextFragment::Text(s) => {
                    let escaped = s.replace('"', "\"\"");
                    parts.push(format!("\"{}\"", escaped))
                }
                TextFragment::Byte(b) => parts.push(format!("${:02x}", b)),
            }
        }
        lines.push((".text".to_string(), parts.join(", "), true));

        if is_end {
            lines.push((".endencode".to_string(), String::new(), false));
        }

        lines
    }

    fn format_screencode_pre(&self) -> Vec<(String, String)> {
        vec![
            (".encode".to_string(), String::new()),
            (".enc".to_string(), "\"screen\"".to_string()),
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
                TextFragment::Byte(b) => parts.push(format!("${:02x}", b)),
            }
        }
        lines.push((".text".to_string(), parts.join(", "), true));
        lines
    }

    fn format_screencode_post(&self) -> Vec<(String, String)> {
        vec![(".endencode".to_string(), String::new())]
    }

    fn format_header_origin(&self, origin: u16) -> String {
        format!("* = ${:04x}", origin)
    }

    fn format_file_header(&self, file_name: &str) -> String {
        let mut s = String::new();
        s.push_str(
            ";=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-\n",
        );
        s.push_str(";\n");
        s.push_str("; Auto-generated by Regenerator 2000\n");
        s.push_str("; https://github.com/ricardoquesada/regenerator2000\n");
        s.push_str(";\n");
        s.push_str("; Assemble with:\n");
        s.push_str(&format!(
            ";   64tass -o {}.prg {}.asm\n",
            file_name, file_name
        ));
        s.push_str(";\n");
        s.push_str(
            ";=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-\n",
        );
        s
    }

    fn format_definition(&self, name: &str, value: u16, is_zp: bool) -> String {
        let operand = if is_zp && value <= 0xFF {
            format!("${:02x}", value)
        } else {
            format!("${:04x}", value)
        };
        format!("{} = {}", name, operand)
    }
}
