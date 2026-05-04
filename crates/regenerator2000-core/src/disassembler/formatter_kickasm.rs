use super::formatter::Formatter;
use crate::cpu::AddressingMode;
use crate::state::{Addr, LabelType};

pub struct KickAsmFormatter;

impl Formatter for KickAsmFormatter {
    fn name(&self) -> &'static str {
        "Kick Assembler"
    }

    fn homepage_url(&self) -> &'static str {
        "https://theweb.dk/KickAssembler"
    }

    fn comment_prefix(&self) -> &'static str {
        "//"
    }

    fn byte_directive(&self) -> &'static str {
        ".byte"
    }

    fn word_directive(&self) -> &'static str {
        ".word"
    }

    fn fill_directive(&self) -> &'static str {
        ".fill"
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
            AddressingMode::Accumulator => String::new(), // KickAssembler often implies 'a', but accepts 'a' too? Let's check. 64tass and acme vary. I'll stick to implicit (empty) unless proven otherwise.
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
        name.to_string()
    }

    fn format_label_definition(&self, name: &str) -> String {
        format!("{name}:")
    }

    fn format_text(
        &self,
        fragments: &[super::formatter::TextFragment],
        is_start: bool,
        _is_end: bool,
    ) -> Vec<(String, String, bool)> {
        use super::formatter::TextFragment;
        let mut lines = Vec::new();

        if is_start {
            lines.push((
                ".encoding".to_string(),
                "\"petscii_upper\"".to_string(),
                false,
            ));
        }

        let mut current_text_parts = Vec::new();
        let mut current_byte_parts = Vec::new();

        for fragment in fragments {
            match fragment {
                TextFragment::Text(s) => {
                    // Flush bytes if any
                    if !current_byte_parts.is_empty() {
                        lines.push((".byte".to_string(), current_byte_parts.join(", "), true));
                        current_byte_parts.clear();
                    }
                    current_text_parts.push(self.format_string_literal(s));
                }
                TextFragment::Byte(b) => {
                    // Flush text if any
                    if !current_text_parts.is_empty() {
                        lines.push((".text".to_string(), current_text_parts.join(", "), true));
                        current_text_parts.clear();
                    }
                    current_byte_parts.push(format!("${b:02x}"));
                }
            }
        }

        // Flush remaining
        if !current_text_parts.is_empty() {
            lines.push((".text".to_string(), current_text_parts.join(", "), true));
        }
        if !current_byte_parts.is_empty() {
            lines.push((".byte".to_string(), current_byte_parts.join(", "), true));
        }

        lines
    }

    fn format_screencode_pre(&self) -> Vec<(String, String)> {
        vec![(".encoding".to_string(), "\"screencode_mixed\"".to_string())]
    }

    fn screencode_byte_threshold(&self) -> u8 {
        // KickAssembler's screencode_mixed encoding can only round-trip
        // screen codes $01-$1A (letters A-Z, via case-swap) and $20-$3F
        // (space, digits, punctuation). Screen codes $00 and $1B-$1F
        // ($40-$5F non-letter ASCII: @, [, \, ], ^, _) are NOT remapped by
        // screencode_mixed and must be emitted as raw bytes. Values $40+
        // are handled by the threshold check in handle_screencode_text.
        0x40
    }

    fn format_screencode(
        &self,
        fragments: &[super::formatter::TextFragment],
    ) -> Vec<(String, String, bool)> {
        use super::formatter::TextFragment;
        let mut lines = Vec::new();

        // `current_text`: raw (unquoted) characters to flush as a .text literal.
        // `current_bytes`: hex strings to flush as a .byte directive.
        let mut current_text = String::new();
        let mut current_bytes: Vec<String> = Vec::new();

        let flush_text =
            |text: &mut String, lines: &mut Vec<(String, String, bool)>, fmt: &KickAsmFormatter| {
                if !text.is_empty() {
                    let literal = fmt.format_string_literal(text);
                    lines.push((".text".to_string(), literal, true));
                    text.clear();
                }
            };
        let flush_bytes = |bytes: &mut Vec<String>, lines: &mut Vec<(String, String, bool)>| {
            if !bytes.is_empty() {
                lines.push((".byte".to_string(), bytes.join(", "), true));
                bytes.clear();
            }
        };

        for fragment in fragments {
            match fragment {
                TextFragment::Text(s) => {
                    // Process each character individually. Under KickAssembler's
                    // screencode_mixed encoding, only letters round-trip (via
                    // case swap). Non-letter characters in the ASCII $40-$5F
                    // range (@, [, \, ], ^, _) were derived from screen codes
                    // $00/$1B-$1F and must be re-emitted as raw screen-code
                    // bytes to avoid producing the wrong value.
                    for c in s.chars() {
                        let b = c as u8;
                        if (0x40..=0x5F).contains(&b) && !c.is_ascii_alphabetic() {
                            // Non-letter in $40-$5F: screencode_mixed won't remap
                            // this back, so emit the original screen code as a raw byte.
                            // Screen code = ASCII - $40 (e.g. $5E '^' → sc $1E).
                            flush_text(&mut current_text, &mut lines, self);
                            current_bytes.push(format!("${:02x}", b - 0x40));
                        } else {
                            // Letter or non-$40-$5F character: case-swap for
                            // screencode_mixed and accumulate in a text literal.
                            flush_bytes(&mut current_bytes, &mut lines);
                            let swapped = if c.is_uppercase() {
                                c.to_ascii_lowercase()
                            } else if c.is_lowercase() {
                                c.to_ascii_uppercase()
                            } else {
                                c
                            };
                            current_text.push(swapped);
                        }
                    }
                    // Flush any accumulated text at end of this fragment.
                    flush_text(&mut current_text, &mut lines, self);
                }
                TextFragment::Byte(b) => {
                    flush_text(&mut current_text, &mut lines, self);
                    current_bytes.push(format!("${b:02x}"));
                }
            }
        }

        // Flush any remaining raw bytes.
        flush_bytes(&mut current_bytes, &mut lines);

        lines
    }

    fn format_screencode_post(&self) -> Vec<(String, String)> {
        Vec::new()
    }

    fn format_header_origin(&self, origin: Addr) -> String {
        format!("*=${origin:04x}")
    }

    fn format_file_header(&self, file_name: &str, _use_illegal_opcodes: bool) -> String {
        let mut s = String::new();
        s.push_str("// Assemble with:\n");
        s.push_str(&format!("//   java -jar KickAss.jar {file_name}.asm\n"));
        s.push_str("//\n");
        s
    }

    fn format_definition(&self, name: &str, value: u16, is_zp: bool) -> String {
        let operand = if is_zp && value <= 0xFF {
            format!("${value:02x}")
        } else {
            format!("${value:04x}")
        };
        format!(".const {name} = {operand}")
    }

    fn format_relative_label(&self, name: &str, offset: usize) -> String {
        // KickAssembler syntax: .label myLabel = * + 10
        format!(".label {} = * + {}", self.format_label(name), offset)
    }

    fn format_instruction(&self, ctx: &super::formatter::FormatContext) -> (String, String) {
        let mnemonic = self.format_mnemonic(ctx.opcode.mnemonic);
        let operand = self.format_operand(ctx);

        // Check for forced absolute addressing
        // If mode is Absolute*, but value fits in ZP, we force .abs suffix
        let val = if ctx.operands.is_empty() {
            0
        } else if ctx.operands.len() >= 2 {
            u16::from(ctx.operands[1]) << 8 | u16::from(ctx.operands[0])
        } else {
            u16::from(ctx.operands[0])
        };

        if val <= 0xFF && ctx.settings.preserve_long_bytes {
            match ctx.opcode.mode {
                AddressingMode::Absolute => {
                    return (format!("{mnemonic}.abs"), operand);
                }
                AddressingMode::AbsoluteX => {
                    return (format!("{mnemonic}.abs"), operand);
                }
                AddressingMode::AbsoluteY => {
                    return (format!("{mnemonic}.abs"), operand);
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
        (".import binary".to_string(), format!("\"{}\"", filename))
    }

    fn format_scope_start(
        &self,
        name: Option<&str>,
    ) -> Option<(Option<String>, String, Option<String>)> {
        let label = name.map(|n| n.to_string());
        Some((label, "{".to_string(), None))
    }

    fn format_scope_end(&self) -> Option<String> {
        Some("}".to_string())
    }
}

impl KickAsmFormatter {
    fn format_string_literal(&self, s: &str) -> String {
        // KickAssembler uses @ prefix to enable escape sequences.
        // If the string contains quotes or control characters, we need to escape it and use @.
        // Otherwise, we can use a plain string.
        if s.contains('"') || s.chars().any(char::is_control) || s.contains('\\') {
            let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
            format!("@\"{escaped}\"")
        } else {
            format!("\"{s}\"")
        }
    }
}
