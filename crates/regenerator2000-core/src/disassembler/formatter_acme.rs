//! `ACME` cross-assembler syntax formatter.

use super::formatter::{FormatContext, Formatter, TextFragment};
use crate::cpu::AddressingMode;
use crate::state::Addr;
use std::fmt::Write;

/// Syntax formatter for the `ACME` cross-assembler.
pub struct AcmeFormatter;

impl Formatter for AcmeFormatter {
    fn name(&self) -> &'static str {
        "ACME"
    }

    fn homepage_url(&self) -> &'static str {
        "https://sourceforge.net/projects/acme-crossass/"
    }

    fn comment_prefix(&self) -> &'static str {
        ";"
    }

    fn byte_directive(&self) -> &'static str {
        "!byte"
    }

    fn word_directive(&self) -> &'static str {
        "!word"
    }

    fn fill_directive(&self) -> &'static str {
        "!fill"
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
        name.to_string()
    }

    fn format_text(
        &self,
        fragments: &[TextFragment],
        _is_start: bool,
        _is_end: bool,
    ) -> Vec<(String, String, bool)> {
        let mut parts = Vec::new();
        for fragment in fragments {
            match fragment {
                TextFragment::Text(s) => {
                    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
                    parts.push(format!("\"{escaped}\""));
                }
                TextFragment::Byte(b) => parts.push(format!("${b:02x}")),
            }
        }
        vec![("!text".to_string(), parts.join(", "), true)]
    }

    fn format_screencode_pre(&self) -> Vec<(String, String)> {
        Vec::new()
    }

    fn format_screencode(&self, fragments: &[TextFragment]) -> Vec<(String, String, bool)> {
        let mut parts = Vec::new();
        for fragment in fragments {
            match fragment {
                TextFragment::Text(s) => {
                    let mut current_literal = String::new();
                    for c in s.chars() {
                        if matches!(c, '{' | '|' | '}' | '~') {
                            if !current_literal.is_empty() {
                                let escaped =
                                    current_literal.replace('\\', "\\\\").replace('"', "\\\"");
                                parts.push(format!("\"{escaped}\""));
                                current_literal.clear();
                            }
                            let hex_val = match c {
                                '{' => "$5b",
                                '|' => "$5c",
                                '}' => "$5d",
                                '~' => "$5e",
                                _ => unreachable!(),
                            };
                            parts.push(hex_val.to_string());
                        } else {
                            // Invert ASCII case because ACME assumes the shifted charset in `!scr`.
                            let inverted_char = if c.is_ascii_lowercase() {
                                c.to_ascii_uppercase()
                            } else if c.is_ascii_uppercase() {
                                c.to_ascii_lowercase()
                            } else {
                                c
                            };
                            current_literal.push(inverted_char);
                        }
                    }
                    if !current_literal.is_empty() {
                        let escaped = current_literal.replace('\\', "\\\\").replace('"', "\\\"");
                        parts.push(format!("\"{escaped}\""));
                    }
                }
                TextFragment::Byte(b) => parts.push(format!("${b:02x}")),
            }
        }
        vec![("!scr".to_string(), parts.join(", "), true)]
    }

    fn format_screencode_post(&self) -> Vec<(String, String)> {
        Vec::new()
    }

    fn format_header_origin(&self, origin: Addr) -> String {
        format!("* = ${origin:04x}")
    }

    fn format_file_header(&self, file_name: &str, use_illegal_opcodes: bool) -> String {
        let cpu_flag = if use_illegal_opcodes {
            "--cpu 6510 "
        } else {
            ""
        };
        format!(
            "; Assemble with:\n;   acme {cpu_flag}--format cbm -o {file_name}.prg {file_name}.asm\n;\n"
        )
    }

    fn format_definition(&self, name: &str, value: u16, _is_zp: bool) -> String {
        // In ACME, a symbol with leading zeros (such as `$00c5`) sets the force-absolute flag on
        // that symbol. That flag forces 3-byte absolute addressing on every reference, even when
        // the original instruction used 2-byte zero-page addressing. Emit `$xx` for values `<= $FF`
        // and rely on the `+2` mnemonic suffix in `format_instruction` to force 16-bit addressing
        // per instruction.
        let operand = if value <= 0xFF {
            format!("${value:02x}")
        } else {
            format!("${value:04x}")
        };
        format!("{name} = {operand}")
    }

    fn format_instruction(&self, ctx: &FormatContext) -> (String, String) {
        let opcode = ctx.opcode;

        // ACME uses `"lxa"` for opcode `$AB` (`LAX` immediate) instead of `"lax"`.
        let mnemonic = if opcode.illegal
            && opcode.mnemonic == "LAX"
            && opcode.mode == AddressingMode::Immediate
        {
            "lxa".to_string()
        } else {
            self.format_mnemonic(opcode.mnemonic)
        };
        let operand = self.format_operand(ctx);

        if ctx.should_force_absolute() {
            return (format!("{mnemonic}+2"), operand);
        }

        (mnemonic, operand)
    }

    fn format_binary_include(&self, filename: &str) -> (String, String) {
        ("!binary".to_string(), format!("\"{filename}\""))
    }

    fn local_label_prefix(&self) -> Option<&'static str> {
        Some(".")
    }

    fn format_enum_reference(&self, enum_name: &str, variant_name: &str) -> String {
        format!("{enum_name}_{variant_name}")
    }

    fn format_enum_definition(&self, enum_def: &crate::state::EnumDefinition) -> String {
        let mut s = String::new();
        for (&val, variant) in &enum_def.variants {
            let val_str = if val <= 0xFF {
                format!("${val:02x}")
            } else {
                format!("${val:04x}")
            };
            let _ = writeln!(s, "{}_{variant} = {val_str}", enum_def.name);
        }
        if s.ends_with('\n') {
            s.pop();
        }
        s
    }
}
