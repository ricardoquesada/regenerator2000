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
}
