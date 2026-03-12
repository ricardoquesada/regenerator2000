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
}

impl<'a> FormatContext<'a> {
    #[must_use]
    pub fn resolve_label(&self, address: Addr) -> Option<&'a Label> {
        if let Some(v) = self.labels.get(&address) {
            crate::disassembler::resolve_label(v, address.0, self.settings)
        } else {
            None
        }
    }
}

pub trait Formatter {
    fn comment_prefix(&self) -> &'static str;
    fn byte_directive(&self) -> &'static str;
    fn word_directive(&self) -> &'static str;
    fn format_byte(&self, byte: u8) -> String;
    fn format_address(&self, address: Addr) -> String;
    fn format_operand(&self, ctx: &FormatContext) -> String;

    fn format_mnemonic(&self, mnemonic: &str) -> String;
    fn format_label(&self, name: &str) -> String;
    fn format_label_definition(&self, name: &str) -> String;

    fn format_text(
        &self,
        fragments: &[TextFragment],
        is_start: bool,
        is_end: bool,
    ) -> Vec<(String, String, bool)>;
    fn format_screencode_pre(&self) -> Vec<(String, String)>;
    fn format_screencode(&self, fragments: &[TextFragment]) -> Vec<(String, String, bool)>;
    fn format_screencode_post(&self) -> Vec<(String, String)>;
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

    fn format_instruction(&self, ctx: &FormatContext) -> (String, String) {
        (
            self.format_mnemonic(ctx.opcode.mnemonic),
            self.format_operand(ctx),
        )
    }
}
