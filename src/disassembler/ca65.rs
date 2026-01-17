use super::formatter::Formatter;
use crate::cpu::AddressingMode;
use crate::state::LabelType;

pub struct Ca65Formatter;

impl Formatter for Ca65Formatter {
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
        let target_context = ctx.target_context;
        let labels = ctx.labels;
        let _settings = ctx.settings;
        let immediate_value_formats = ctx.immediate_value_formats;

        let get_label = |addr: u16, l_type: LabelType| -> Option<String> {
            if let Some(label_vec) = labels.get(&addr) {
                // 1. Try to match target_context if provided
                if let Some(target) = target_context
                    && let Some(l) = label_vec.iter().find(|l| l.label_type == target)
                {
                    return Some(l.name.clone());
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
            AddressingMode::Accumulator => "a".to_string(), // ca65 usually accepts implicit 'a' or explicit 'a' for accumulation, but often explicit is safer.
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
                    _ => format!("#${:02x}", val),
                }
            }
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
                    // ca65 uses *+offset usually for anonymous, but absolute addr is fine too
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
        // ca65 uses : for definition, but here we return the name for reference/storage
        name.to_string()
    }

    fn format_label_definition(&self, name: &str) -> String {
        format!("{}:", name)
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
        vec![(".byte".to_string(), parts.join(", "), true)]
    }

    fn format_screencode_pre(&self) -> Vec<(String, String)> {
        Vec::new()
    }

    fn format_screencode(
        &self,
        fragments: &[super::formatter::TextFragment],
    ) -> Vec<(String, String, bool)> {
        // ca65 doesn't have a direct !scr equivalent without macros.
        // We will output as bytes for safety, OR revert to .byte "string" but inverted manually if we want.
        // But since we don't have a reliable way to say "this string is screencode" in vanilla ca65 without macros/charmaps
        // let's stick to byte values for now to ensure correctness, OR
        // use .byte with comments?
        // Actually, users prefer readable text.
        // For ACME we inverted.
        // For ca65, we can use a similar strategy: emit bytes, but maybe comment specific chars?
        // Wait, Tass uses .text encoding "screen".
        // ca65 has .charmap.
        // But setting up charmaps is complex.
        // Let's just output bytes for screencodes to be safe and correct.
        // OR try to mimic ACME logic but output .byte
        use super::formatter::TextFragment;
        let mut parts = Vec::new();
        for fragment in fragments {
            match fragment {
                TextFragment::Text(s) => {
                    for c in s.chars() {
                        // Invert case logic similar to ACME
                        let inverted_char = if c.is_ascii_lowercase() {
                            c.to_ascii_uppercase()
                        } else if c.is_ascii_uppercase() {
                            c.to_ascii_lowercase()
                        } else {
                            c
                        };
                        // We output as byte to be safe
                        parts.push(format!("${:02x}", inverted_char as u8));
                    }
                }
                TextFragment::Byte(b) => parts.push(format!("${:02x}", b)),
            }
        }
        vec![(".byte".to_string(), parts.join(", "), true)]
    }

    fn format_screencode_post(&self) -> Vec<(String, String)> {
        Vec::new()
    }

    fn format_header_origin(&self, origin: u16) -> String {
        format!(".org ${:04x}", origin)
    }

    fn format_definition(&self, name: &str, value: u16, is_zp: bool) -> String {
        let operand = if is_zp && value <= 0xFF {
            format!("${:02x}", value)
        } else {
            format!("${:04x}", value)
        };
        format!("{} = {}", name, operand)
    }

    fn format_instruction(&self, ctx: &super::formatter::FormatContext) -> (String, String) {
        let mnemonic = self.format_mnemonic(ctx.opcode.mnemonic);
        let operand = self.format_operand(ctx);

        // Check for forced absolute addressing
        let val = if !ctx.operands.is_empty() {
            if ctx.operands.len() >= 2 {
                (ctx.operands[1] as u16) << 8 | (ctx.operands[0] as u16)
            } else {
                ctx.operands[0] as u16
            }
        } else {
            0
        };

        if val <= 0xFF && ctx.settings.preserve_long_bytes {
            match ctx.opcode.mode {
                AddressingMode::Absolute
                | AddressingMode::AbsoluteX
                | AddressingMode::AbsoluteY => {
                    // ca65 uses a: prefix for absolute addressing override
                    return (mnemonic, format!("a:{}", operand));
                }
                _ => {}
            }
        }

        (mnemonic, operand)
    }
}
