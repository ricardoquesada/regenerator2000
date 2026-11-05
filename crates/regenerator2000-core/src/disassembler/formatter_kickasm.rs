//! `KickAssembler` syntax formatter implementation.

use super::formatter::Formatter;
use crate::state::Addr;
use std::fmt::Write;

/// Formatter for the `KickAssembler` 6502/6510 macro assembler.
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
                    if !current_byte_parts.is_empty() {
                        lines.push((".byte".to_string(), current_byte_parts.join(", "), true));
                        current_byte_parts.clear();
                    }
                    current_text_parts.push(self.format_string_literal(s));
                }
                TextFragment::Byte(b) => {
                    if !current_text_parts.is_empty() {
                        lines.push((".text".to_string(), current_text_parts.join(", "), true));
                        current_text_parts.clear();
                    }
                    current_byte_parts.push(format!("${b:02x}"));
                }
            }
        }

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
        // The `screencode_mixed` encoding in KickAssembler only round-trips screen codes
        // `$01..=$1A` (`A-Z`, via case swap) and `$20..=$3F` (space, digits, punctuation).
        // Screen codes `$00` and `$1B..=$1F` (`@`, `[`, `\`, `]`, `^`, `_`) are emitted as
        // raw bytes in `format_screencode`, and values `$40` and above are rejected here.
        0x40
    }

    fn format_screencode(
        &self,
        fragments: &[super::formatter::TextFragment],
    ) -> Vec<(String, String, bool)> {
        use super::formatter::TextFragment;
        let mut lines = Vec::new();

        // `current_text` stores unquoted characters for a `.text` directive.
        // `current_bytes` stores formatted hex bytes for a `.byte` directive.
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
                    // Under `screencode_mixed`, only letters in `$40..=$5F` round-trip via case swap.
                    // Non-letter characters in `$40..=$5F` (`@`, `[`, `\`, `]`, `^`, `_`) come from
                    // screen codes `$00` and `$1B..=$1F` and must be emitted as raw screen-code bytes
                    // (`b - 0x40`).
                    for c in s.chars() {
                        let b = c as u8;
                        if (0x40..=0x5F).contains(&b) && !c.is_ascii_alphabetic() {
                            flush_text(&mut current_text, &mut lines, self);
                            current_bytes.push(format!("${:02x}", b - 0x40));
                        } else {
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
                    flush_text(&mut current_text, &mut lines, self);
                }
                TextFragment::Byte(b) => {
                    flush_text(&mut current_text, &mut lines, self);
                    current_bytes.push(format!("${b:02x}"));
                }
            }
        }

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
        format!("// Assemble with:\n//   java -jar KickAss.jar {file_name}.asm\n//\n")
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
        format!(".label {} = * + {}", self.format_label(name), offset)
    }

    fn format_instruction(&self, ctx: &super::formatter::FormatContext) -> (String, String) {
        let mnemonic = self.format_mnemonic(ctx.opcode.mnemonic);
        let operand = self.format_operand(ctx);

        // KickAssembler uses the `.abs` mnemonic suffix to force 16-bit absolute addressing
        // when the operand value fits in zero page.
        if ctx.should_force_absolute() {
            return (format!("{mnemonic}.abs"), operand);
        }

        (mnemonic, operand)
    }

    fn supports_scopes(&self) -> bool {
        true
    }

    fn format_binary_include(&self, filename: &str) -> (String, String) {
        (".import binary".to_string(), format!("\"{filename}\""))
    }

    fn format_scope_start(
        &self,
        name: Option<&str>,
    ) -> Option<(Option<String>, String, Option<String>)> {
        let label = name.map(str::to_string);
        Some((label, "{".to_string(), None))
    }

    fn format_scope_end(&self) -> Option<String> {
        Some("}".to_string())
    }

    fn format_enum_reference(&self, enum_name: &str, variant_name: &str) -> String {
        format!("{enum_name}.{variant_name}")
    }

    fn format_enum_definition(&self, enum_def: &crate::state::EnumDefinition) -> String {
        let mut s = format!(".namespace {} {{\n", enum_def.name);
        for (&val, variant) in &enum_def.variants {
            let val_str = if val <= 0xFF {
                format!("${val:02x}")
            } else {
                format!("${val:04x}")
            };
            let _ = writeln!(s, "    .label {variant} = {val_str}");
        }
        s.push('}');
        s
    }
}

impl KickAsmFormatter {
    /// Formats a string literal for `KickAssembler`.
    ///
    /// Uses the `@"..."` escaped string prefix when `s` contains double quotes, backslashes,
    /// or control characters.
    fn format_string_literal(&self, s: &str) -> String {
        if s.contains('"') || s.chars().any(char::is_control) || s.contains('\\') {
            let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
            format!("@\"{escaped}\"")
        } else {
            format!("\"{s}\"")
        }
    }
}
