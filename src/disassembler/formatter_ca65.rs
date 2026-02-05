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
        let _target_context = ctx.target_context;
        let _labels = ctx.labels;
        let _settings = ctx.settings;
        let immediate_value_formats = ctx.immediate_value_formats;

        let get_label = |addr: u16, _l_type: LabelType| -> Option<String> {
            ctx.resolve_label(addr).map(|l| l.name.clone())
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

        enum Token {
            Char(char),
            Byte(u8),
        }

        let mut tokens = Vec::new();
        for fragment in fragments {
            match fragment {
                // ca65 doesn't support PETSCII, so we need to convert ASCII to PETSCII
                TextFragment::Text(s) => {
                    for c in s.chars() {
                        let b = c as u8;
                        if (0x20..=0x5f).contains(&b) {
                            tokens.push(Token::Char(c.to_ascii_lowercase()));
                        } else {
                            tokens.push(Token::Byte(b));
                        }
                    }
                }
                TextFragment::Byte(b) => tokens.push(Token::Byte(*b)),
            }
        }

        let mut lines = Vec::new();
        let mut current_directive_is_text = None; // Some(true) for text, Some(false) for byte
        let mut pending_text = String::new();
        let mut pending_bytes = Vec::new();

        let flush = |lines: &mut Vec<(String, String, bool)>,
                     is_text: Option<bool>,
                     p_text: &mut String,
                     p_bytes: &mut Vec<u8>| {
            match is_text {
                Some(true) => {
                    if !p_text.is_empty() {
                        let mut parts = Vec::new();
                        let mut first = true;
                        // ca65 doesn't support \" escapes. We must split by " and insert $22.
                        // quoting logic
                        for part in p_text.split('"') {
                            if !first {
                                parts.push("$22".to_string());
                            }
                            if !part.is_empty() {
                                parts.push(format!("\"{}\"", part.replace('\\', "\\\\")));
                            }
                            first = false;
                        }
                        if !parts.is_empty() {
                            lines.push((".byte".to_string(), parts.join(", "), true));
                        }
                        p_text.clear();
                    }
                }
                Some(false) => {
                    if !p_bytes.is_empty() {
                        let parts: Vec<String> =
                            p_bytes.iter().map(|b| format!("${:02x}", b)).collect();
                        lines.push((".byte".to_string(), parts.join(", "), true));
                        p_bytes.clear();
                    }
                }
                None => {}
            }
        };

        for token in tokens {
            match token {
                Token::Char(c) => {
                    if current_directive_is_text == Some(false) {
                        flush(
                            &mut lines,
                            current_directive_is_text,
                            &mut pending_text,
                            &mut pending_bytes,
                        );
                    }
                    current_directive_is_text = Some(true);
                    pending_text.push(c);
                }
                Token::Byte(b) => {
                    if current_directive_is_text == Some(true) {
                        flush(
                            &mut lines,
                            current_directive_is_text,
                            &mut pending_text,
                            &mut pending_bytes,
                        );
                    }
                    current_directive_is_text = Some(false);
                    pending_bytes.push(b);
                }
            }
        }
        flush(
            &mut lines,
            current_directive_is_text,
            &mut pending_text,
            &mut pending_bytes,
        );

        lines
    }

    fn format_screencode_pre(&self) -> Vec<(String, String)> {
        Vec::new()
    }

    fn format_screencode(
        &self,
        fragments: &[super::formatter::TextFragment],
    ) -> Vec<(String, String, bool)> {
        use super::formatter::TextFragment;
        let mut lines = Vec::new();
        let mut pending_bytes = Vec::new();

        let flush_bytes = |bytes: &mut Vec<u8>, lines: &mut Vec<(String, String, bool)>| {
            if !bytes.is_empty() {
                let parts: Vec<String> = bytes.iter().map(|b| format!("${:02x}", b)).collect();
                lines.push((".byte".to_string(), parts.join(", "), true));
                bytes.clear();
            }
        };

        for fragment in fragments {
            match fragment {
                TextFragment::Text(s) => {
                    flush_bytes(&mut pending_bytes, &mut lines);

                    let s_swapped: String = s
                        .chars()
                        .map(|c| {
                            if c.is_ascii_uppercase() {
                                c.to_ascii_lowercase()
                            } else if c.is_ascii_lowercase() {
                                c.to_ascii_uppercase()
                            } else {
                                c
                            }
                        })
                        .collect();

                    let mut parts = Vec::new();
                    let mut first = true;
                    for part in s_swapped.split('"') {
                        if !first {
                            parts.push("$22".to_string());
                        }
                        if !part.is_empty() {
                            parts.push(format!("\"{}\"", part));
                        }
                        first = false;
                    }
                    if !parts.is_empty() {
                        lines.push(("scrcode".to_string(), parts.join(", "), true));
                    }
                }
                TextFragment::Byte(b) => {
                    pending_bytes.push(*b);
                }
            }
        }
        flush_bytes(&mut pending_bytes, &mut lines);
        lines
    }

    fn format_screencode_post(&self) -> Vec<(String, String)> {
        Vec::new()
    }

    fn format_header_origin(&self, origin: u16) -> String {
        format!(".org ${:04x}", origin)
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
            ";   cl65 -t c64 -C c64-asm.cfg {}.asm -o {}.prg\n",
            file_name, file_name
        ));
        s.push_str(";\n");
        s.push_str(
            ";=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-\n",
        );
        s.push_str("\n.macpack cbm                            ; adds support for scrcode\n");
        s.push('\n');
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disassembler::formatter::{Formatter, TextFragment};

    #[test]
    fn test_format_text_petscii() {
        let formatter = Ca65Formatter;

        // Test case 1: Range $40-$5f (should be lowercase)
        let fragments = vec![TextFragment::Text("A".to_string())];
        let result = formatter.format_text(&fragments, true, true);

        assert!(
            result
                .iter()
                .any(|(directive, operands, _)| directive == ".byte" && operands.contains("\"a\"")),
            "Expected .byte \"a\", got {:?}",
            result
        );

        // Test case 2: Range $60-$7f (should be .byte)
        // 'a' is $61. Should become .byte $61
        let fragments = vec![TextFragment::Text("a".to_string())];
        let result = formatter.format_text(&fragments, true, true);

        assert!(
            result
                .iter()
                .any(|(directive, operands, _)| directive == ".byte" && operands.contains("$61")),
            "Expected .byte $61, got {:?}",
            result
        );

        // Test case 3: Mixed "Aa" -> .byte "a", .byte $61
        let fragments = vec![TextFragment::Text("Aa".to_string())];
        let result = formatter.format_text(&fragments, true, true);

        // Should have two parts or combined correctly?
        // Based on plan: separate them.
        // First part: .byte "a"
        // Second part: .byte $61
        // (Order matters)
        let part1 = &result[0];
        assert_eq!(part1.0, ".byte");
        assert!(part1.1.contains("\"a\""));

        let part2 = &result[1];
        assert_eq!(part2.0, ".byte");
        assert!(part2.1.contains("$61"));
    }
}
