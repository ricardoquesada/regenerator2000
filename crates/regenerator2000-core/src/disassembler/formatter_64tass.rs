//! `64tass` cross-assembler syntax formatter.

use super::formatter::{Formatter, TextFragment};
use crate::state::Addr;
use std::fmt::Write;

/// Syntax formatter for the `64tass` cross-assembler.
pub struct TassFormatter;

impl TassFormatter {
    fn format_fragments(fragments: &[TextFragment]) -> String {
        fragments
            .iter()
            .map(|fragment| match fragment {
                TextFragment::Text(s) => {
                    let escaped = s.replace('"', "\"\"");
                    format!("\"{escaped}\"")
                }
                TextFragment::Byte(b) => format!("${b:02x}"),
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl Formatter for TassFormatter {
    fn name(&self) -> &'static str {
        "64tass"
    }

    fn homepage_url(&self) -> &'static str {
        "https://tass64.sourceforge.net/"
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
        ".fill"
    }

    fn format_byte(&self, byte: u8) -> String {
        format!("${byte:02x}")
    }

    fn format_address(&self, address: Addr) -> String {
        format!("${:04x}", address.0)
    }

    fn accumulator_operand(&self) -> &'static str {
        "a"
    }

    fn absolute_zero_page_prefix(&self) -> Option<&'static str> {
        Some("@w ")
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
        fragments: &[TextFragment],
        is_start: bool,
        is_end: bool,
    ) -> Vec<(String, String, bool)> {
        let mut lines = Vec::new();

        if is_start {
            lines.push((".encode".to_string(), String::new(), false));
            lines.push((".enc".to_string(), "\"none\"".to_string(), false));
        }

        lines.push((".text".to_string(), Self::format_fragments(fragments), true));

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

    fn format_screencode(&self, fragments: &[TextFragment]) -> Vec<(String, String, bool)> {
        vec![(".text".to_string(), Self::format_fragments(fragments), true)]
    }

    fn format_screencode_post(&self) -> Vec<(String, String)> {
        vec![(".endencode".to_string(), String::new())]
    }

    fn format_header_origin(&self, origin: Addr) -> String {
        format!("* = ${origin:04x}")
    }

    fn format_file_header(&self, file_name: &str, use_illegal_opcodes: bool) -> String {
        let illegal_flag = if use_illegal_opcodes { "-i " } else { "" };
        format!(
            "; Assemble with:\n;   64tass -C {illegal_flag}-o {file_name}.prg {file_name}.asm\n;\n"
        )
    }

    fn format_definition(&self, name: &str, value: u16, is_zp: bool) -> String {
        let operand = if is_zp && value <= 0xFF {
            format!("${value:02x}")
        } else {
            format!("${value:04x}")
        };
        format!("{name} = {operand}")
    }

    fn format_scope_start(
        &self,
        name: Option<&str>,
    ) -> Option<(Option<String>, String, Option<String>)> {
        Some((name.map(str::to_string), ".block".to_string(), None))
    }

    fn format_scope_end(&self) -> Option<String> {
        Some(".bend".to_string())
    }

    fn supports_scopes(&self) -> bool {
        true
    }

    fn local_label_prefix(&self) -> Option<&'static str> {
        Some("_")
    }

    fn format_enum_reference(&self, enum_name: &str, variant_name: &str) -> String {
        format!("{enum_name}.{variant_name}")
    }

    fn format_enum_definition(&self, enum_def: &crate::state::EnumDefinition) -> String {
        let mut s = format!("{} .block\n", enum_def.name);
        for (&val, variant) in &enum_def.variants {
            let val_str = if val <= 0xFF {
                format!("${val:02x}")
            } else {
                format!("${val:04x}")
            };
            let _ = writeln!(s, "    {variant} = {val_str}");
        }
        s.push_str(".bend");
        s
    }
}
