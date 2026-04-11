use super::formatter::Formatter;
use crate::cpu::AddressingMode;
use crate::state::{Addr, LabelType};

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
            AddressingMode::Accumulator => String::new(),
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
                    format!("{name},x") // ACME is case insensitive but often convention is lowercase regs
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
        name.to_string()
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

    fn format_screencode(
        &self,
        fragments: &[super::formatter::TextFragment],
    ) -> Vec<(String, String, bool)> {
        use super::formatter::TextFragment;
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
                            // Output as hex
                            let hex_val = match c {
                                '{' => "$5b",
                                '|' => "$5c",
                                '}' => "$5d",
                                '~' => "$5e",
                                _ => unreachable!(),
                            };
                            parts.push(hex_val.to_string());
                        } else {
                            // Invert case: ACME assumes shifted charset in !scr
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
        let mut s = String::new();
        s.push_str("; Assemble with:\n");
        let cpu_flag = if use_illegal_opcodes {
            "--cpu 6510 "
        } else {
            ""
        };
        s.push_str(&format!(
            ";   acme {cpu_flag}--format cbm -o {file_name}.prg {file_name}.asm\n"
        ));
        s.push_str(";\n");
        s
    }

    fn format_definition(&self, name: &str, value: u16, _is_zp: bool) -> String {
        // In ACME, defining a symbol with leading zeros (e.g., $00c5) sets the
        // "force absolute" flag on that symbol, causing ALL references to use
        // 3-byte absolute addressing even when the original code used 2-byte
        // zero-page addressing.  To avoid this, we always use the shortest hex
        // representation: $xx for values <= $FF, $xxxx otherwise.  The +2
        // mnemonic suffix already forces absolute on a per-instruction basis
        // where needed (see format_instruction).
        let operand = if value <= 0xFF {
            format!("${value:02x}")
        } else {
            format!("${value:04x}")
        };
        format!("{name} = {operand}")
    }

    fn format_instruction(&self, ctx: &super::formatter::FormatContext) -> (String, String) {
        let opcode = ctx.opcode;
        let operands = ctx.operands;
        let _address = ctx.address;
        let settings = ctx.settings;

        // ACME uses "lxa" for opcode $AB (LAX immediate), not "lax"
        let mnemonic = if opcode.illegal
            && opcode.mnemonic == "LAX"
            && opcode.mode == AddressingMode::Immediate
        {
            "lxa".to_string()
        } else {
            self.format_mnemonic(opcode.mnemonic)
        };
        let operand = self.format_operand(ctx);

        // Check if we need to force 16-bit addressing with +2
        // Only if settings.use_w_prefix is true AND address fits in ZP (<= 0xFF)
        // And addressing mode is Absolute, AbsoluteX, or AbsoluteY
        if settings.preserve_long_bytes {
            let should_force = match opcode.mode {
                AddressingMode::Absolute
                | AddressingMode::AbsoluteX
                | AddressingMode::AbsoluteY => {
                    if operands.len() >= 2 {
                        let addr = Addr(u16::from(operands[1]) << 8 | u16::from(operands[0]));
                        addr <= 0xFF
                    } else {
                        false
                    }
                }
                _ => false,
            };

            if should_force {
                return (format!("{mnemonic}+2"), operand);
            }
        }

        (mnemonic, operand)
    }

    fn format_binary_include(&self, filename: &str) -> (String, String) {
        ("!binary".to_string(), format!("\"{}\"", filename))
    }

    fn local_label_prefix(&self) -> Option<&'static str> {
        Some(".")
    }
}
