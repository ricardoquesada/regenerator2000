use crate::cpu::Opcode;
use crate::state::{Addr, Label};
use std::collections::BTreeMap;

pub enum TextFragment {
    Text(String),
    Byte(u8),
}

pub struct FormatContext<'a> {
    pub opcode: &'a Opcode,
    pub operands: &'a [u8],
    pub address: Addr,
    pub target_context: Option<crate::state::LabelType>,
    pub labels: &'a BTreeMap<Addr, Vec<Label>>,
    pub settings: &'a crate::state::DocumentSettings,
    pub immediate_value_formats: &'a BTreeMap<Addr, crate::state::ImmediateFormat>,
    pub local_label_names: Option<&'a BTreeMap<Addr, String>>,
    pub label_scope_names: Option<&'a BTreeMap<Addr, String>>,
    pub current_scope_name: Option<&'a str>,
    pub scope_separator: &'a str,
    pub local_prefix: Option<&'a str>,
}

impl<'a> FormatContext<'a> {
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
}

pub trait Formatter {
    fn name(&self) -> &'static str;
    fn homepage_url(&self) -> &'static str;

    fn comment_prefix(&self) -> &'static str;
    fn byte_directive(&self) -> &'static str;
    fn word_directive(&self) -> &'static str;
    fn format_byte(&self, byte: u8) -> String;
    fn format_address(&self, address: Addr) -> String;
    fn format_operand(&self, ctx: &FormatContext) -> String;

    fn format_mnemonic(&self, mnemonic: &str) -> String;
    fn format_label(&self, name: &str) -> String;
    fn format_label_definition(&self, name: &str) -> String;

    fn local_label_prefix(&self) -> Option<&'static str> {
        None
    }

    fn scope_resolution_separator(&self) -> &'static str {
        "."
    }

    fn format_text(
        &self,
        fragments: &[TextFragment],
        is_start: bool,
        is_end: bool,
    ) -> Vec<(String, String, bool)>;
    fn format_screencode_pre(&self) -> Vec<(String, String)>;
    fn format_screencode(&self, fragments: &[TextFragment]) -> Vec<(String, String, bool)>;
    fn format_screencode_post(&self) -> Vec<(String, String)>;

    /// Screen code values >= this threshold are emitted as raw `.byte` values
    /// rather than being converted to text characters. Each assembler's
    /// screencode directive (`!scr`, `scrcode`, `.text` with screen encoding,
    /// etc.) handles different character ranges, so this lets each formatter
    /// define the safe upper bound.
    ///
    /// Default is `0x5f` (screen codes $00–$5E are text, $5F+ are raw bytes),
    /// which works for 64tass and ACME.
    fn screencode_byte_threshold(&self) -> u8 {
        0x5f
    }
    fn format_header_origin(&self, origin: Addr) -> String;
    fn format_file_header(&self, file_name: &str, use_illegal_opcodes: bool) -> String {
        let _ = file_name;
        let _ = use_illegal_opcodes;
        String::new()
    }
    fn format_definition(&self, name: &str, value: u16, is_zp: bool) -> String;
    fn format_relative_label(&self, name: &str, offset: usize) -> String {
        format!("{} =*+${:02x}", self.format_label(name), offset)
    }

    /// Allows assemblers to define their own local symbol naming (e.g. `l15` instead of `_l0f`).
    fn format_local_label(&self, index: usize) -> Option<String> {
        Some(format!("l{:02}", index))
    }

    /// For assemblers that use `.block` or similar scoping directives.
    /// Returns (label, mnemonic, operand) if supported.
    fn format_scope_start(
        &self,
        _name: Option<&str>,
    ) -> Option<(Option<String>, String, Option<String>)> {
        None
    }

    /// For assemblers that use `.bend` or similar scoping directives.
    fn format_scope_end(&self) -> Option<String> {
        None
    }

    fn supports_scopes(&self) -> bool {
        false
    }

    fn format_binary_include(&self, filename: &str) -> (String, String) {
        (".binary".to_string(), format!("\"{}\"", filename))
    }

    fn format_instruction(&self, ctx: &FormatContext) -> (String, String) {
        (
            self.format_mnemonic(ctx.opcode.mnemonic),
            self.format_operand(ctx),
        )
    }
}
