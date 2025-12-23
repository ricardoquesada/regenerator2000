use crate::cpu::Opcode;
use crate::state::Label;
use std::collections::HashMap;

pub trait Formatter {
    fn byte_directive(&self) -> &'static str;
    fn word_directive(&self) -> &'static str;
    fn format_operand(
        &self,
        opcode: &Opcode,
        operands: &[u8],
        address: u16,
        target_context: Option<crate::state::LabelType>,
        labels: &HashMap<u16, Vec<Label>>,
        settings: &crate::state::DocumentSettings,
    ) -> String;

    fn format_mnemonic(&self, mnemonic: &str) -> String;
    fn format_label(&self, name: &str) -> String;

    fn format_byte(&self, byte: u8) -> String;
    fn format_word(&self, word: u16) -> String;
    fn format_text(
        &self,
        bytes: &[u8],
        text: &str,
        is_start: bool,
        is_end: bool,
    ) -> Vec<(String, String, bool)>;
    fn format_screencode(
        &self,
        bytes: &[u8],
        text: &str,
        is_start: bool,
        is_end: bool,
    ) -> Vec<(String, String, bool)>;
    fn format_header_origin(&self, origin: u16) -> String;
    fn format_definition(&self, name: &str, value: u16, is_zp: bool) -> String;

    fn format_instruction(
        &self,
        opcode: &Opcode,
        operands: &[u8],
        address: u16,
        target_context: Option<crate::state::LabelType>,
        labels: &HashMap<u16, Vec<Label>>,
        settings: &crate::state::DocumentSettings,
    ) -> (String, String) {
        (
            self.format_mnemonic(&opcode.mnemonic),
            self.format_operand(opcode, operands, address, target_context, labels, settings),
        )
    }
}
