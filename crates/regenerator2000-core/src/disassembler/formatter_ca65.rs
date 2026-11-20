//! `ca65` (cc65 suite) syntax formatter implementation.

use super::formatter::{FileDirective, FormatContext, Formatter, TextFragment};
use crate::state::{Addr, EnumDefinition};
use std::fmt::Write;

/// Maximum number of arguments accepted by cc65's `scrcode` macro (`arg1`..`arg9` in `cbm.mac`).
const MAX_SCRCODE_ARGS: usize = 9;

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

    /// Classifies an ASCII character from a screen-code [`TextFragment::Text`](super::formatter::TextFragment::Text)
    /// as either a safe `scrcode` character (with ASCII letter case swapped) or a raw screen-code byte.
    ///
    /// Matches `format_text`'s `$5B..=$5F` boundary: screen codes `$1B..=$1F` (`[`, `\`, `]`,
    /// `^`, `_`) are emitted as raw `.byte` values (`b - 0x40`) because `ca65`'s C64 target
    /// charmap remaps characters in this range (for example, `\` -> `$BF` -> `$7F` and
    /// `_` -> `$A4` -> `$64`). Direct-caller characters `` ` `` and `{`..`~` (screen codes
    /// `$40` and `$5B..=$5E`) are also emitted as raw `.byte` values (`b - 0x20`).
    fn classify_screencode_char(c: char) -> Result<char, u8> {
        match c {
            ' '..='@' => Ok(c),
            'A'..='Z' => Ok(c.to_ascii_lowercase()),
            '['..='_' => Err((c as u8) - 0x40),
            'a'..='z' => Ok(c.to_ascii_uppercase()),
            '`' | '{'..='~' => Err((c as u8) - 0x20),
            _ => Err(c as u8),
        }
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
        fragments: &[TextFragment],
        _is_start: bool,
        _is_end: bool,
    ) -> Vec<(String, String, bool)> {
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
        // `$00..=$1A` (`@`, `A-Z` via case swap) and `$20..=$3F` cleanly. Screen codes
        // `$1B..=$1F` are emitted as raw `.byte` values in `format_screencode`, and values
        // `$40` and above are rejected here.
        0x40
    }

    fn format_screencode(&self, fragments: &[TextFragment]) -> Vec<(String, String, bool)> {
        let mut lines = Vec::new();
        let mut pending_text = String::new();
        let mut pending_bytes = Vec::new();

        let flush_text = |text: &mut String, lines: &mut Vec<(String, String, bool)>| {
            if !text.is_empty() {
                let parts = Self::split_quoted_parts(text);
                // `cbm.mac` defines `.macro scrcode arg1..arg9`, so chunk to at most 9 args per line.
                for chunk in parts.chunks(MAX_SCRCODE_ARGS) {
                    lines.push(("scrcode".to_string(), chunk.join(", "), true));
                }
                text.clear();
            }
        };

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
                    for c in s.chars() {
                        match Self::classify_screencode_char(c) {
                            Ok(ch) => {
                                flush_bytes(&mut pending_bytes, &mut lines);
                                pending_text.push(ch);
                            }
                            Err(raw) => {
                                flush_text(&mut pending_text, &mut lines);
                                pending_bytes.push(raw);
                            }
                        }
                    }
                }
                TextFragment::Byte(b) => {
                    flush_text(&mut pending_text, &mut lines);
                    pending_bytes.push(*b);
                }
            }
        }
        flush_text(&mut pending_text, &mut lines);
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

    fn format_file_directives(&self) -> Vec<FileDirective> {
        vec![FileDirective {
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

    fn format_instruction(&self, ctx: &FormatContext) -> (String, String) {
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

    fn format_enum_definition(&self, enum_def: &EnumDefinition) -> String {
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

    #[test]
    fn test_format_screencode_unsafe_chars() {
        let formatter = Ca65Formatter;

        // Screen codes `$1B..=$1F` (`[`, `\`, `]`, `^`, `_` in `disassemble_screencode`)
        // must be emitted as raw `.byte` values so `ca65`'s C64 charmap does not remap them.
        let fragments = vec![TextFragment::Text("[\\]^_".to_string())];
        let result = formatter.format_screencode(&fragments);
        assert_eq!(
            result,
            vec![(
                ".byte".to_string(),
                "$1b, $1c, $1d, $1e, $1f".to_string(),
                true
            )]
        );

        // Interleaved letters and unsafe chars split cleanly between `scrcode` and `.byte`.
        let fragments = vec![TextFragment::Text("A\\B_C".to_string())];
        let result = formatter.format_screencode(&fragments);
        assert_eq!(
            result,
            vec![
                ("scrcode".to_string(), "\"a\"".to_string(), true),
                (".byte".to_string(), "$1c".to_string(), true),
                ("scrcode".to_string(), "\"b\"".to_string(), true),
                (".byte".to_string(), "$1f".to_string(), true),
                ("scrcode".to_string(), "\"c\"".to_string(), true),
            ]
        );

        // Adjacent unsafe text chars and `TextFragment::Byte` coalesce into one `.byte` directive.
        let fragments = vec![
            TextFragment::Text("A_".to_string()),
            TextFragment::Byte(0x40),
            TextFragment::Text("\\B".to_string()),
        ];
        let result = formatter.format_screencode(&fragments);
        assert_eq!(
            result,
            vec![
                ("scrcode".to_string(), "\"a\"".to_string(), true),
                (".byte".to_string(), "$1f, $40, $1c".to_string(), true),
                ("scrcode".to_string(), "\"b\"".to_string(), true),
            ]
        );

        // Direct callers passing `` ` `` or `{`..`~` emit raw screen codes `$40` and `$5b..=$5e`.
        let fragments = vec![TextFragment::Text("`{|}~".to_string())];
        let result = formatter.format_screencode(&fragments);
        assert_eq!(
            result,
            vec![(
                ".byte".to_string(),
                "$40, $5b, $5c, $5d, $5e".to_string(),
                true
            )]
        );

        // Screen code `$00` (`@`) stays in `scrcode`.
        let fragments = vec![TextFragment::Text("@".to_string())];
        let result = formatter.format_screencode(&fragments);
        assert_eq!(
            result,
            vec![("scrcode".to_string(), "\"@\"".to_string(), true)]
        );
    }

    #[test]
    fn test_format_screencode_chunks_max_macro_args() {
        let formatter = Ca65Formatter;

        // 11 `split_quoted_parts` elements -> 9 args on first `scrcode`, 2 on second.
        let fragments = vec![TextFragment::Text("A\"B\"C\"D\"E\"F".to_string())];
        let result = formatter.format_screencode(&fragments);
        assert_eq!(
            result,
            vec![
                (
                    "scrcode".to_string(),
                    "\"a\", $22, \"b\", $22, \"c\", $22, \"d\", $22, \"e\"".to_string(),
                    true,
                ),
                ("scrcode".to_string(), "$22, \"f\"".to_string(), true),
            ]
        );

        // 10 consecutive double quotes -> 9 `$22` args, then 1 `$22` arg.
        let fragments = vec![TextFragment::Text("\"".repeat(10))];
        let result = formatter.format_screencode(&fragments);
        assert_eq!(
            result,
            vec![
                (
                    "scrcode".to_string(),
                    "$22, $22, $22, $22, $22, $22, $22, $22, $22".to_string(),
                    true,
                ),
                ("scrcode".to_string(), "$22".to_string(), true),
            ]
        );
    }
}
