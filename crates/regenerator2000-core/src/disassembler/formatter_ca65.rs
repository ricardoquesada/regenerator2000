use super::formatter::Formatter;
use crate::cpu::AddressingMode;
use crate::state::{Addr, LabelType};

pub struct Ca65Formatter;

impl Formatter for Ca65Formatter {
    fn name(&self) -> &'static str {
        "ca65"
    }

    fn homepage_url(&self) -> &'static str {
        "https://cc65.github.io/doc/ca65.html"
    }

    fn comment_prefix(&self) -> &'static str {
        ";"
    }

    fn byte_directive(&self) -> &'static str {
        ".byte"
    }

    fn word_directive(&self) -> &'static str {
        ".word"
    }

    fn fill_directive(&self) -> &'static str {
        ".res"
    }

    fn format_byte(&self, byte: u8) -> String {
        format!("${byte:02x}")
    }

    fn format_address(&self, address: Addr) -> String {
        format!("${:04x}", address.0)
    }

    fn format_operand(&self, ctx: &super::formatter::FormatContext) -> String {
        let opcode = ctx.opcode;
        let operands = ctx.operands;
        let address = ctx.address;
        let _target_context = ctx.target_context;
        let _labels = ctx.labels;
        let _settings = ctx.settings;
        let immediate_value_formats = ctx.immediate_value_formats;

        let get_label =
            |addr: Addr, _l_type: LabelType| -> Option<String> { ctx.resolve_label(addr) };

        match opcode.mode {
            AddressingMode::Implied => String::new(),
            AddressingMode::Accumulator => "a".to_string(), // ca65 usually accepts implicit 'a' or explicit 'a' for accumulation, but often explicit is safer.
            AddressingMode::Immediate => {
                let val = operands[0];
                match immediate_value_formats.get(&address) {
                    Some(crate::state::ImmediateFormat::InvertedHex) => {
                        format!("#~${:02x}", !val)
                    }
                    Some(crate::state::ImmediateFormat::Decimal) => format!("#{val}"),
                    Some(crate::state::ImmediateFormat::NegativeDecimal) => {
                        format!("#{}", val as i8)
                    }
                    Some(crate::state::ImmediateFormat::Binary) => format!("#%{val:08b}"),
                    Some(crate::state::ImmediateFormat::InvertedBinary) => {
                        format!("#~%{:08b}", !val)
                    }
                    Some(crate::state::ImmediateFormat::LowByte(target)) => {
                        let name =
                            get_label(*target, LabelType::AbsoluteAddress).unwrap_or_else(|| {
                                if *target <= 0xFF {
                                    format!("${target:02x}")
                                } else {
                                    format!("${target:04x}")
                                }
                            });
                        format!("#<{name}")
                    }
                    Some(crate::state::ImmediateFormat::HighByte(target)) => {
                        let name =
                            get_label(*target, LabelType::AbsoluteAddress).unwrap_or_else(|| {
                                if *target <= 0xFF {
                                    format!("${target:02x}")
                                } else {
                                    format!("${target:04x}")
                                }
                            });
                        format!("#>{name}")
                    }
                    _ => format!("#${val:02x}"),
                }
            }
            AddressingMode::ZeroPage => {
                let addr = Addr::from(u16::from(operands[0]));
                if let Some(name) = get_label(addr, LabelType::ZeroPageAbsoluteAddress) {
                    name
                } else {
                    format!("${addr:02x}")
                }
            }
            AddressingMode::ZeroPageX => {
                let addr = Addr::from(u16::from(operands[0]));
                if let Some(name) = get_label(addr, LabelType::ZeroPageField) {
                    format!("{name},x")
                } else {
                    format!("${addr:02x},x")
                }
            }
            AddressingMode::ZeroPageY => {
                let addr = Addr::from(u16::from(operands[0]));
                if let Some(name) = get_label(addr, LabelType::ZeroPageField) {
                    format!("{name},y")
                } else {
                    format!("${addr:02x},y")
                }
            }
            AddressingMode::Relative => {
                let offset = operands[0] as i8;
                let target = address.wrapping_add(2).wrapping_add(offset as u16);
                if let Some(name) = get_label(target, LabelType::Branch) {
                    name
                } else {
                    // ca65 uses *+offset usually for anonymous, but absolute addr is fine too
                    format!("${target:04x}")
                }
            }
            AddressingMode::Absolute => {
                let addr = Addr(u16::from(operands[1]) << 8 | u16::from(operands[0]));
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
                    format!("${addr:04x}")
                }
            }
            AddressingMode::AbsoluteX => {
                let addr = Addr(u16::from(operands[1]) << 8 | u16::from(operands[0]));
                if let Some(name) = get_label(addr, LabelType::Field) {
                    format!("{name},x")
                } else {
                    format!("${addr:04x},x")
                }
            }
            AddressingMode::AbsoluteY => {
                let addr = Addr(u16::from(operands[1]) << 8 | u16::from(operands[0]));
                if let Some(name) = get_label(addr, LabelType::Field) {
                    format!("{name},y")
                } else {
                    format!("${addr:04x},y")
                }
            }

            AddressingMode::Indirect => {
                let addr = Addr(u16::from(operands[1]) << 8 | u16::from(operands[0]));
                if let Some(name) = get_label(addr, LabelType::Pointer) {
                    format!("({name})")
                } else {
                    format!("(${addr:04x})")
                }
            }
            AddressingMode::IndirectX => {
                let addr = Addr::from(u16::from(operands[0]));
                if let Some(name) = get_label(addr, LabelType::ZeroPagePointer) {
                    format!("({name},x)")
                } else {
                    format!("(${addr:02x},x)")
                }
            }
            AddressingMode::IndirectY => {
                let addr = Addr::from(u16::from(operands[0]));
                if let Some(name) = get_label(addr, LabelType::ZeroPagePointer) {
                    format!("({name}),y")
                } else {
                    format!("(${addr:02x}),y")
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
        format!("{name}:")
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
                // ca65 with C64 target maps source ASCII to PETSCII.
                // Only $20-$5A round-trip correctly:
                //   $20-$3F: space, digits, punctuation (same in ASCII and PETSCII)
                //   $40: @ (same in ASCII and PETSCII)
                //   $41-$5A: A-Z → lowercased to a-z, ca65 maps a-z back to $41-$5A
                // Characters $5B-$5F are remapped by ca65's C64 charset
                // (e.g. _ → $A4, \ → $A9) and don't round-trip.
                TextFragment::Text(s) => {
                    for c in s.chars() {
                        let b = c as u8;
                        if (0x20..=0x5a).contains(&b) {
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
                                parts.push(format!("\"{part}\""));
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
                            p_bytes.iter().map(|b| format!("${b:02x}")).collect();
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

    fn screencode_byte_threshold(&self) -> u8 {
        // ca65's scrcode macro (from cbm macpack) can only round-trip
        // screen codes $00-$3F correctly. Values $40+ map to characters
        // that ca65's C64 charset remaps to different byte values.
        0x40
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
                let parts: Vec<String> = bytes.iter().map(|b| format!("${b:02x}")).collect();
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
                            parts.push(format!("\"{part}\""));
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

    fn format_header_origin(&self, origin: Addr) -> String {
        format!(".org ${origin:04x}")
    }

    fn format_file_header(&self, file_name: &str, use_illegal_opcodes: bool) -> String {
        let mut s = String::new();
        s.push_str("; Assemble with:\n");
        let cpu_flag = if use_illegal_opcodes {
            "--cpu 6502X "
        } else {
            ""
        };
        s.push_str(&format!(
            ";   cl65 -t c64 {cpu_flag}-C c64-asm.cfg {file_name}.asm -o {file_name}.prg\n"
        ));
        s.push_str(";\n");
        s.push_str("\n.macpack cbm                            ; adds support for scrcode\n");
        s.push('\n');
        s
    }

    fn format_definition(&self, name: &str, value: u16, is_zp: bool) -> String {
        let operand = if is_zp && value <= 0xFF {
            format!("${value:02x}")
        } else {
            format!("${value:04x}")
        };
        format!("{name} = {operand}")
    }

    fn format_instruction(&self, ctx: &super::formatter::FormatContext) -> (String, String) {
        let mnemonic = self.format_mnemonic(ctx.opcode.mnemonic);
        let operand = self.format_operand(ctx);

        // Check for forced absolute addressing
        let val = if ctx.operands.is_empty() {
            0
        } else if ctx.operands.len() >= 2 {
            u16::from(ctx.operands[1]) << 8 | u16::from(ctx.operands[0])
        } else {
            u16::from(ctx.operands[0])
        };

        if val <= 0xFF && ctx.settings.preserve_long_bytes {
            match ctx.opcode.mode {
                AddressingMode::Absolute
                | AddressingMode::AbsoluteX
                | AddressingMode::AbsoluteY => {
                    // ca65 uses a: prefix for absolute addressing override
                    return (mnemonic, format!("a:{operand}"));
                }
                _ => {}
            }
        }

        (mnemonic, operand)
    }

    fn supports_scopes(&self) -> bool {
        true
    }

    fn format_binary_include(&self, filename: &str) -> (String, String) {
        (".incbin".to_string(), format!("\"{}\"", filename))
    }

    fn local_label_prefix(&self) -> Option<&'static str> {
        Some("@")
    }

    fn scope_resolution_separator(&self) -> &'static str {
        "::"
    }

    fn format_scope_start(
        &self,
        name: Option<&str>,
    ) -> Option<(Option<String>, String, Option<String>)> {
        let label = name.unwrap_or("unnamed_scope").to_string();
        Some((None, ".proc".to_string(), Some(label)))
    }

    fn format_scope_end(&self) -> Option<String> {
        Some(".endproc".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disassembler::formatter::{Formatter, TextFragment};

    #[test]
    fn test_format_text_petscii() {
        let formatter = Ca65Formatter;

        // Test case 1: Range $41-$5a (should be lowercase text)
        let fragments = vec![TextFragment::Text("A".to_string())];
        let result = formatter.format_text(&fragments, true, true);

        assert!(
            result
                .iter()
                .any(|(directive, operands, _)| directive == ".byte" && operands.contains("\"a\"")),
            "Expected .byte \"a\", got {result:?}"
        );

        // Test case 2: Range $60-$7f (should be .byte)
        // 'a' is $61. Should become .byte $61
        let fragments = vec![TextFragment::Text("a".to_string())];
        let result = formatter.format_text(&fragments, true, true);

        assert!(
            result
                .iter()
                .any(|(directive, operands, _)| directive == ".byte" && operands.contains("$61")),
            "Expected .byte $61, got {result:?}"
        );

        // Test case 3: Mixed "Aa" -> .byte "a", .byte $61
        let fragments = vec![TextFragment::Text("Aa".to_string())];
        let result = formatter.format_text(&fragments, true, true);

        // First part: .byte "a"
        // Second part: .byte $61
        let part1 = &result[0];
        assert_eq!(part1.0, ".byte");
        assert!(part1.1.contains("\"a\""));

        let part2 = &result[1];
        assert_eq!(part2.0, ".byte");
        assert!(part2.1.contains("$61"));
    }

    #[test]
    fn test_format_text_petscii_unsafe_chars() {
        let formatter = Ca65Formatter;

        // PETSCII $5B-$5F ([, \, ], ^, _) don't round-trip through ca65's C64
        // charset mapping, so they must be emitted as raw bytes.
        // In handle_petscii_text, these PETSCII bytes become ASCII chars:
        //   $5B -> '[', $5C -> '\', $5D -> ']', $5E -> '^', $5F -> '_'
        let fragments = vec![TextFragment::Text("[\\]^_".to_string())];
        let result = formatter.format_text(&fragments, true, true);

        // All should be raw bytes
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, ".byte");
        assert_eq!(result[0].1, "$5b, $5c, $5d, $5e, $5f");
    }
}
