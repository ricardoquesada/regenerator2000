//! `ca65` (cc65 suite) syntax formatter implementation.

use super::formatter::Formatter;
use crate::state::Addr;
use std::fmt::Write;

/// Formatter for the `ca65` macro assembler (`cc65` suite).
pub struct Ca65Formatter;

impl Ca65Formatter {
    /// Splits `text` on double quotes (`"`) and inserts raw `$22` byte literals between quoted chunks.
    ///
    /// `ca65` does not support `\"` escape sequences inside string literals.
    fn split_quoted_parts(text: &str) -> Vec<String> {
        let mut parts = Vec::new();
        for (i, part) in text.split('"').enumerate() {
            if i > 0 {
                parts.push("$22".to_string());
            }
            if !part.is_empty() {
                parts.push(format!("\"{part}\""));
            }
        }
        parts
    }
}

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

    fn accumulator_operand(&self) -> &'static str {
        "a"
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
        // Return the bare name for operand references. `format_label_definition` appends `:`.
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
                // `ca65` with the C64 target maps source ASCII characters to PETSCII.
                // Only bytes `$20..=$5A` round-trip accurately:
                // - `$20..=$40`: Space, digits, punctuation, and `@` match in ASCII and PETSCII.
                // - `$41..=$5A`: Convert `A-Z` to `a-z` so `ca65` maps `a-z` back to `$41..=$5A`.
                // Characters `$5B..=$5F` are remapped by the `ca65` C64 character set
                // (for example, `_` -> `$A4` and `\` -> `$A9`) and must be emitted as raw bytes.
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
        let mut current_directive_is_text = None;
        let mut pending_text = String::new();
        let mut pending_bytes = Vec::new();

        let flush = |lines: &mut Vec<(String, String, bool)>,
                     is_text: Option<bool>,
                     p_text: &mut String,
                     p_bytes: &mut Vec<u8>| {
            match is_text {
                Some(true) if !p_text.is_empty() => {
                    let parts = Self::split_quoted_parts(p_text);
                    if !parts.is_empty() {
                        lines.push((".byte".to_string(), parts.join(", "), true));
                    }
                    p_text.clear();
                }
                Some(false) if !p_bytes.is_empty() => {
                    let parts: Vec<String> = p_bytes.iter().map(|b| format!("${b:02x}")).collect();
                    lines.push((".byte".to_string(), parts.join(", "), true));
                    p_bytes.clear();
                }
                _ => {}
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
        // The `scrcode` macro from the `cbm` macro package only round-trips screen codes
        // `$00..=$3F`. Values `$40` and above are remapped to different byte values by `ca65`.
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

                    let parts = Self::split_quoted_parts(&s_swapped);
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
        let cpu_flag = if use_illegal_opcodes {
            "--cpu 6502X "
        } else {
            ""
        };
        format!(
            "; Assemble with:\n;   cl65 -t c64 {cpu_flag}-C c64-asm.cfg {file_name}.asm -o {file_name}.prg\n;\n"
        )
    }

    fn format_file_directives(&self) -> Vec<super::formatter::FileDirective> {
        vec![super::formatter::FileDirective {
            directive: ".macpack cbm".to_string(),
            comment: Some("adds support for scrcode".to_string()),
        }]
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

        // `ca65` uses the `a:` operand prefix to force 16-bit absolute addressing on zero-page values.
        if ctx.should_force_absolute() {
            return (mnemonic, format!("a:{operand}"));
        }

        (mnemonic, operand)
    }

    fn supports_scopes(&self) -> bool {
        true
    }

    fn format_binary_include(&self, filename: &str) -> (String, String) {
        (".incbin".to_string(), format!("\"{filename}\""))
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

    fn format_enum_reference(&self, enum_name: &str, variant_name: &str) -> String {
        format!("{enum_name}::{variant_name}")
    }

    fn format_enum_definition(&self, enum_def: &crate::state::EnumDefinition) -> String {
        let mut s = format!(".enum {}\n", enum_def.name);
        for (&val, variant) in &enum_def.variants {
            let val_str = if val <= 0xFF {
                format!("${val:02x}")
            } else {
                format!("${val:04x}")
            };
            let _ = writeln!(s, "    {variant} = {val_str}");
        }
        s.push_str(".endenum");
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disassembler::formatter::{Formatter, TextFragment};

    #[test]
    fn test_format_text_petscii() {
        let formatter = Ca65Formatter;

        // Range `$41..=$5A` converts to lowercase ASCII text.
        let fragments = vec![TextFragment::Text("A".to_string())];
        let result = formatter.format_text(&fragments, true, true);

        assert!(
            result
                .iter()
                .any(|(directive, operands, _)| directive == ".byte" && operands.contains("\"a\"")),
            "Expected .byte \"a\", got {result:?}"
        );

        // Range `$60..=$7F` emits raw `.byte` values (`'a'` is `$61`).
        let fragments = vec![TextFragment::Text("a".to_string())];
        let result = formatter.format_text(&fragments, true, true);

        assert!(
            result
                .iter()
                .any(|(directive, operands, _)| directive == ".byte" && operands.contains("$61")),
            "Expected .byte $61, got {result:?}"
        );

        // Mixed `"Aa"` emits `.byte "a"` followed by `.byte $61`.
        let fragments = vec![TextFragment::Text("Aa".to_string())];
        let result = formatter.format_text(&fragments, true, true);

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

        // PETSCII bytes `$5B..=$5F` (`[`, `\`, `]`, `^`, `_`) do not round-trip through
        // the `ca65` C64 character set mapping and must be emitted as raw bytes.
        let fragments = vec![TextFragment::Text("[\\]^_".to_string())];
        let result = formatter.format_text(&fragments, true, true);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, ".byte");
        assert_eq!(result[0].1, "$5b, $5c, $5d, $5e, $5f");
    }
}
