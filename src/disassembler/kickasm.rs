use super::formatter::Formatter;
use crate::cpu::AddressingMode;
use crate::state::LabelType;

pub struct KickAsmFormatter;

impl Formatter for KickAsmFormatter {
    fn comment_prefix(&self) -> &'static str {
        "//"
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
            AddressingMode::Accumulator => String::new(), // KickAssembler often implies 'a', but accepts 'a' too? Let's check. 64tass and acme vary. I'll stick to implicit (empty) unless proven otherwise.
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
        name.to_string()
    }

    fn format_label_definition(&self, name: &str) -> String {
        format!("{}:", name)
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
                    current_byte_parts.push(format!("${:02x}", b));
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

    fn format_screencode(
        &self,
        fragments: &[super::formatter::TextFragment],
    ) -> Vec<(String, String, bool)> {
        use super::formatter::TextFragment;
        let mut lines = Vec::new();

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
                    let swapped: String = s
                        .chars()
                        .map(|c| {
                            if c.is_uppercase() {
                                c.to_lowercase().collect::<String>()
                            } else if c.is_lowercase() {
                                c.to_uppercase().collect::<String>()
                            } else {
                                c.to_string()
                            }
                        })
                        .collect();
                    current_text_parts.push(self.format_string_literal(&swapped));
                }
                TextFragment::Byte(b) => {
                    // Flush text if any
                    if !current_text_parts.is_empty() {
                        lines.push((".text".to_string(), current_text_parts.join(", "), true));
                        current_text_parts.clear();
                    }
                    current_byte_parts.push(format!("${:02x}", b));
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

    fn format_screencode_post(&self) -> Vec<(String, String)> {
        Vec::new()
    }

    fn format_header_origin(&self, origin: u16) -> String {
        format!("*=${:04x}", origin)
    }

    fn format_file_header(&self, file_name: &str) -> String {
        let mut s = String::new();
        s.push_str(
            "//=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-\n",
        );
        s.push_str("//\n");
        s.push_str("// Auto-generated by Regenerator 2000\n");
        s.push_str("// https://github.com/ricardoquesada/regenerator2000");
        s.push_str("//\n");
        s.push_str("// Assemble with:\n");
        s.push_str(&format!("//   java -jar KickAss.jar {}.asm\n", file_name));
        s.push_str("//\n");
        s.push_str(
            "//=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-\n",
        );
        s
    }

    fn format_definition(&self, name: &str, value: u16, is_zp: bool) -> String {
        let operand = if is_zp && value <= 0xFF {
            format!("${:02x}", value)
        } else {
            format!("${:04x}", value)
        };
        format!(".const {} = {}", name, operand)
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
                AddressingMode::Absolute => {
                    return (format!("{}.abs", mnemonic), operand);
                }
                AddressingMode::AbsoluteX => {
                    return (format!("{}.abs", mnemonic), operand);
                }
                AddressingMode::AbsoluteY => {
                    return (format!("{}.abs", mnemonic), operand);
                }
                _ => {}
            }
        }

        (mnemonic, operand)
    }
}

impl KickAsmFormatter {
    fn format_string_literal(&self, s: &str) -> String {
        // KickAssembler uses @ prefix to enable escape sequences.
        // If the string contains quotes or control characters, we need to escape it and use @.
        // Otherwise, we can use a plain string.
        if s.contains('"') || s.chars().any(|c| c.is_control()) || s.contains('\\') {
            let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
            format!("@\"{}\"", escaped)
        } else {
            format!("\"{}\"", s)
        }
    }
}
