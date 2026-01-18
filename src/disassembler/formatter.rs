use crate::cpu::Opcode;
use crate::state::Label;
use std::collections::BTreeMap;

pub enum TextFragment {
    Text(String),
    Byte(u8),
}

pub struct FormatContext<'a> {
    pub opcode: &'a Opcode,
    pub operands: &'a [u8],
    pub address: u16,
    pub target_context: Option<crate::state::LabelType>,
    pub labels: &'a BTreeMap<u16, Vec<Label>>,
    pub settings: &'a crate::state::DocumentSettings,
    pub immediate_value_formats: &'a BTreeMap<u16, crate::state::ImmediateFormat>,
}

pub trait Formatter {
    fn comment_prefix(&self) -> &'static str;
    fn byte_directive(&self) -> &'static str;
    fn word_directive(&self) -> &'static str;
    fn format_byte(&self, byte: u8) -> String;
    fn format_address(&self, address: u16) -> String;
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
    fn format_header_origin(&self, origin: u16) -> String;
    fn format_file_header(&self, file_name: &str) -> String {
        let _ = file_name;
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
