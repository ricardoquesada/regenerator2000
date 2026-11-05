//! Cross-assembler formatting trait, operand context, and shared operand rendering.
//!
//! This module defines [`Formatter`], [`FormatContext`], [`TextFragment`], and [`FileDirective`].
//! It provides the default 6502 operand formatting logic shared across `64tass`, `ACME`, `ca65`,
//! and `KickAssembler`.

use crate::cpu::{AddressingMode, Opcode};
use crate::state::{Addr, ImmediateFormat, Label};
use std::collections::BTreeMap;

/// A fragment of a PETSCII or screen-code text block.
pub enum TextFragment {
    /// Printable text literal segment.
    Text(String),
    /// Non-printable or non-roundtripping raw byte value.
    Byte(u8),
}

/// A top-level assembler directive placed after the header banner and before symbol definitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDirective {
    /// Directive text (for example, `".macpack cbm"`).
    pub directive: String,
    /// Optional side comment that explains the directive (for example, `"adds support for scrcode"`).
    pub comment: Option<String>,
}

/// Read-only instruction context passed to [`Formatter::format_operand`] and [`Formatter::format_instruction`].
pub struct FormatContext<'a> {
    /// Decoded 6502 [`Opcode`] metadata.
    pub opcode: &'a Opcode,
    /// Raw operand bytes that follow the opcode byte (`0..=2` bytes).
    pub operands: &'a [u8],
    /// Memory [`Addr`] of the instruction opcode byte.
    pub address: Addr,
    /// Optional semantic [`LabelType`](crate::state::LabelType) hint for the operand target.
    pub target_context: Option<crate::state::LabelType>,
    /// Address-keyed map of project [`Label`]s.
    pub labels: &'a BTreeMap<Addr, Vec<Label>>,
    /// Document-level disassembly settings.
    pub settings: &'a crate::state::DocumentSettings,
    /// Sparse per-address annotations.
    pub annotations: &'a crate::state::AnnotationManager,
    /// Optional local label names within the active lexical scope.
    pub local_label_names: Option<&'a BTreeMap<Addr, String>>,
    /// Optional map from scoped addresses to their enclosing scope names.
    pub label_scope_names: Option<&'a BTreeMap<Addr, String>>,
    /// Name of the lexical scope that encloses [`Self::address`], if any.
    pub current_scope_name: Option<&'a str>,
    /// Scope resolution separator for the target assembler (for example, `"."` or `"::"`).
    pub scope_separator: &'a str,
    /// Optional local label prefix for the target assembler (for example, `"_"`, `"."`, or `"@"`).
    pub local_prefix: Option<&'a str>,

    /// Project-local enum definitions keyed by enum name.
    pub enums: &'a BTreeMap<String, crate::state::EnumDefinition>,
    /// User-level global enum definitions keyed by enum name.
    pub user_global_enums: &'a BTreeMap<String, crate::state::EnumDefinition>,
    /// Built-in platform enum definitions keyed by enum name.
    pub builtin_enums: &'a BTreeMap<String, crate::state::EnumDefinition>,
}

impl FormatContext<'_> {
    /// Resolves the formatted symbol name for `address` within the current scope context.
    #[must_use]
    pub fn resolve_label(&self, address: Addr) -> Option<String> {
        crate::disassembler::resolve_label_name(
            address,
            self.labels,
            self.settings,
            self.local_label_names,
            self.label_scope_names,
            self.current_scope_name,
            self.scope_separator,
            self.local_prefix,
        )
    }

    /// Resolves `value` against the enum annotation at [`Self::address`], if any.
    #[must_use]
    pub fn resolve_enum_value(&self, value: u16) -> Option<(String, String)> {
        super::context::resolve_enum_at(
            self.annotations,
            self.enums,
            self.user_global_enums,
            self.builtin_enums,
            self.address,
            value,
        )
    }

    /// Returns `true` when `preserve_long_bytes` is active and an `Absolute*` operand fits in `$00..=$FF`.
    #[must_use]
    pub fn should_force_absolute(&self) -> bool {
        self.settings.preserve_long_bytes
            && matches!(
                self.opcode.mode,
                AddressingMode::Absolute | AddressingMode::AbsoluteX | AddressingMode::AbsoluteY
            )
            && self
                .operands
                .get(..2)
                .is_some_and(|ops| (u16::from(ops[1]) << 8 | u16::from(ops[0])) <= 0xFF)
    }
}

/// Target-specific syntax formatter for a 6502 cross-assembler dialect.
pub trait Formatter {
    /// Returns the human-readable name of the target assembler.
    fn name(&self) -> &'static str;

    /// Returns the official homepage URL of the target assembler.
    fn homepage_url(&self) -> &'static str;

    /// Returns the line-comment prefix string (`";"` or `"//"`).
    fn comment_prefix(&self) -> &'static str;

    /// Returns the directive mnemonic for raw 8-bit byte data (for example, `".byte"` or `"!byte"`).
    fn byte_directive(&self) -> &'static str;

    /// Returns the directive mnemonic for 16-bit word data (for example, `".word"` or `"!word"`).
    fn word_directive(&self) -> &'static str;

    /// Returns the directive mnemonic for repeated byte runs (for example, `".fill"` or `".res"`).
    fn fill_directive(&self) -> &'static str;

    /// Formats a single 8-bit `byte` literal in hexadecimal.
    fn format_byte(&self, byte: u8) -> String;

    /// Formats a 16-bit memory `address` literal in hexadecimal.
    fn format_address(&self, address: Addr) -> String;

    /// Returns the operand string for [`AddressingMode::Accumulator`] (`"a"` or `""`).
    fn accumulator_operand(&self) -> &'static str {
        ""
    }

    /// Returns an operand prefix (such as `"@w "` in `64tass`) when a 16-bit absolute instruction
    /// targets a zero-page address (`$0000..=$00FF`) and `preserve_long_bytes` is enabled.
    fn absolute_zero_page_prefix(&self) -> Option<&'static str> {
        None
    }

    /// Formats the operand string for the instruction in `ctx`.
    fn format_operand(&self, ctx: &FormatContext) -> String {
        let immediate_format = ctx
            .annotations
            .get(ctx.address)
            .and_then(|e| e.immediate_format);

        let resolve_or_hex = |target: Addr| -> String {
            ctx.resolve_label(target).unwrap_or_else(|| {
                if target.0 <= 0xFF {
                    format!("${target:02x}")
                } else {
                    format!("${target:04x}")
                }
            })
        };

        match ctx.opcode.mode {
            AddressingMode::Implied => String::new(),
            AddressingMode::Accumulator => self.accumulator_operand().to_string(),
            AddressingMode::Immediate => {
                let val = ctx.operands[0];
                if let Some((enum_name, variant_name)) = ctx.resolve_enum_value(u16::from(val)) {
                    return format!("#{}", self.format_enum_reference(&enum_name, &variant_name));
                }
                match immediate_format {
                    Some(ImmediateFormat::InvertedHex) => format!("#~${:02x}", !val),
                    Some(ImmediateFormat::Decimal) => format!("#{val}"),
                    Some(ImmediateFormat::NegativeDecimal) => format!("#{}", val as i8),
                    Some(ImmediateFormat::Binary) => format!("#%{val:08b}"),
                    Some(ImmediateFormat::InvertedBinary) => format!("#~%{:08b}", !val),
                    Some(ImmediateFormat::LowByte(target)) => {
                        format!("#<{}", resolve_or_hex(target))
                    }
                    Some(ImmediateFormat::HighByte(target)) => {
                        format!("#>{}", resolve_or_hex(target))
                    }
                    _ => format!("#${val:02x}"),
                }
            }
            AddressingMode::ZeroPage => {
                let addr = Addr::from(u16::from(ctx.operands[0]));
                ctx.resolve_label(addr)
                    .unwrap_or_else(|| format!("${addr:02x}"))
            }
            AddressingMode::ZeroPageX => {
                let addr = Addr::from(u16::from(ctx.operands[0]));
                ctx.resolve_label(addr)
                    .map_or_else(|| format!("${addr:02x},x"), |name| format!("{name},x"))
            }
            AddressingMode::ZeroPageY => {
                let addr = Addr::from(u16::from(ctx.operands[0]));
                ctx.resolve_label(addr)
                    .map_or_else(|| format!("${addr:02x},y"), |name| format!("{name},y"))
            }
            AddressingMode::Relative => {
                let offset = ctx.operands[0] as i8;
                let target = ctx.address.wrapping_add(2).wrapping_add(offset as u16);
                ctx.resolve_label(target)
                    .unwrap_or_else(|| format!("${target:04x}"))
            }
            AddressingMode::Absolute | AddressingMode::AbsoluteX | AddressingMode::AbsoluteY => {
                let addr = Addr(u16::from(ctx.operands[1]) << 8 | u16::from(ctx.operands[0]));
                let suffix = match ctx.opcode.mode {
                    AddressingMode::AbsoluteX => ",x",
                    AddressingMode::AbsoluteY => ",y",
                    _ => "",
                };
                let base = ctx.resolve_label(addr).map_or_else(
                    || format!("${addr:04x}{suffix}"),
                    |name| format!("{name}{suffix}"),
                );
                if ctx.should_force_absolute()
                    && let Some(prefix) = self.absolute_zero_page_prefix()
                {
                    format!("{prefix}{base}")
                } else {
                    base
                }
            }
            AddressingMode::Indirect => {
                let addr = Addr(u16::from(ctx.operands[1]) << 8 | u16::from(ctx.operands[0]));
                ctx.resolve_label(addr)
                    .map_or_else(|| format!("(${addr:04x})"), |name| format!("({name})"))
            }
            AddressingMode::IndirectX => {
                let addr = Addr::from(u16::from(ctx.operands[0]));
                ctx.resolve_label(addr)
                    .map_or_else(|| format!("(${addr:02x},x)"), |name| format!("({name},x)"))
            }
            AddressingMode::IndirectY => {
                let addr = Addr::from(u16::from(ctx.operands[0]));
                ctx.resolve_label(addr)
                    .map_or_else(|| format!("(${addr:02x}),y"), |name| format!("({name}),y"))
            }
            AddressingMode::Unknown => "???".to_string(),
        }
    }

    /// Formats the target-specific syntax for a full enum definition block.
    fn format_enum_definition(&self, enum_def: &crate::state::EnumDefinition) -> String;

    /// Formats a scoped enum variant reference for operand and data contexts.
    fn format_enum_reference(&self, enum_name: &str, variant_name: &str) -> String;

    /// Formats an instruction mnemonic (typically lowercase).
    fn format_mnemonic(&self, mnemonic: &str) -> String;

    /// Formats a symbol name for operand reference or internal storage.
    fn format_label(&self, name: &str) -> String;

    /// Formats a symbol name at its definition site (for example, appending `":"` in `ca65`).
    fn format_label_definition(&self, name: &str) -> String;

    /// Returns the prefix for user-defined local labels, if the assembler uses one.
    fn local_label_prefix(&self) -> Option<&'static str> {
        None
    }

    /// Returns the separator between a scope name and an inner label name.
    fn scope_resolution_separator(&self) -> &'static str {
        "."
    }

    /// Formats PETSCII [`TextFragment`]s into `(directive, operand, has_bytes)` rows.
    fn format_text(
        &self,
        fragments: &[TextFragment],
        is_start: bool,
        is_end: bool,
    ) -> Vec<(String, String, bool)>;

    /// Returns setup directives to place before a screen-code text block.
    fn format_screencode_pre(&self) -> Vec<(String, String)>;

    /// Formats screen-code [`TextFragment`]s into `(directive, operand, has_bytes)` rows.
    fn format_screencode(&self, fragments: &[TextFragment]) -> Vec<(String, String, bool)>;

    /// Returns closing directives to place after a screen-code text block.
    fn format_screencode_post(&self) -> Vec<(String, String)>;

    /// Returns the screen-code byte value threshold at or above which bytes emit as raw `.byte` values.
    ///
    /// Each assembler's screen-code directive (`!scr`, `scrcode`, or `.text` with screen encoding)
    /// supports a different character range. The default is `0x5f` (screen codes `$00..=$5E` emit
    /// as text and `$5F+` emit as raw bytes), which matches `64tass` and `ACME`.
    fn screencode_byte_threshold(&self) -> u8 {
        0x5f
    }

    /// Formats the program origin directive (for example, `"* = $1000"` or `".org $1000"`).
    fn format_header_origin(&self, origin: Addr) -> String;

    /// Formats the commented build-command header for exported `.asm` files.
    fn format_file_header(&self, file_name: &str, use_illegal_opcodes: bool) -> String {
        let _ = file_name;
        let _ = use_illegal_opcodes;
        String::new()
    }

    /// Returns top-level assembler setup directives to place after the file header banner.
    fn format_file_directives(&self) -> Vec<FileDirective> {
        Vec::new()
    }

    /// Formats an external symbol equate definition (`name = $xxxx`).
    fn format_definition(&self, name: &str, value: u16, is_zp: bool) -> String;

    /// Formats a mid-instruction relative label definition (`name = * + offset`).
    fn format_relative_label(&self, name: &str, offset: usize) -> String {
        format!("{} =*+${offset:02x}", self.format_label(name))
    }

    /// Formats an auto-generated local symbol name (for example, `"l00"`).
    fn format_local_label(&self, index: usize) -> Option<String> {
        Some(format!("l{index:02}"))
    }

    /// Formats the opening line of a lexical scope as `(label, mnemonic, operand)` when supported.
    fn format_scope_start(
        &self,
        _name: Option<&str>,
    ) -> Option<(Option<String>, String, Option<String>)> {
        None
    }

    /// Formats the closing directive of a lexical scope (for example, `".bend"` or `".endproc"`).
    fn format_scope_end(&self) -> Option<String> {
        None
    }

    /// Returns `true` if the target assembler supports lexical scope blocks.
    fn supports_scopes(&self) -> bool {
        false
    }

    /// Formats an external binary file include directive as `(directive, quoted_filename)`.
    fn format_binary_include(&self, filename: &str) -> (String, String) {
        (".binary".to_string(), format!("\"{filename}\""))
    }

    /// Formats the `(mnemonic, operand)` pair for a decoded instruction.
    fn format_instruction(&self, ctx: &FormatContext) -> (String, String) {
        (
            self.format_mnemonic(ctx.opcode.mnemonic),
            self.format_operand(ctx),
        )
    }
}
