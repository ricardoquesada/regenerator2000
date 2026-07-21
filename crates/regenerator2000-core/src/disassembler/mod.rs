use crate::cpu::Opcode;
use crate::state::Addr;

pub mod context;
pub mod data_blocks;
pub mod formatter;
pub mod formatter_64tass;
pub mod formatter_acme;
pub mod formatter_ca65;
pub mod formatter_kickasm;
pub mod handlers;
pub mod pipeline;
pub mod symbols;

pub use context::{DisassemblyContext, HandleArgs, format_cross_references};
pub use symbols::{resolve_label, resolve_label_name};

pub const LABEL_COLUMN_WIDTH: usize = 20;
/// Column width for the `name = $value` portion of equate / external-label
/// definition lines. All renderers (TUI, ASM exporter, HTML exporter) pad
/// to this width so that trailing comments (x-refs) start at the same column.
/// Must match `.cc.as .lb { min-width: 40ch }` in the HTML exporter CSS.
pub const DEFINITION_COLUMN_WIDTH: usize = 40;

#[derive(Debug, Clone)]
pub struct DisassemblyLine {
    pub address: Addr,
    pub bytes: Vec<u8>,
    pub mnemonic: String,
    pub operand: String,
    pub comment: String,
    pub line_comment: Option<String>,
    #[allow(dead_code)]
    pub label: Option<String>,
    pub opcode: Option<Opcode>,
    pub show_bytes: bool,
    pub target_address: Option<Addr>,
    pub external_label_address: Option<Addr>,
    pub is_collapsed: bool,
}

impl DisassemblyLine {
    #[must_use]
    pub fn get_sub_index_for_address(
        &self,
        app_state: &crate::state::app_state::AppState,
        target_addr: u16,
    ) -> usize {
        let mut sub_index = 0;

        if self.bytes.len() > 1 {
            for offset in 1..self.bytes.len() {
                let mid_addr = self.address.wrapping_add(offset as u16);
                if let Some(l) = app_state.labels.get(&mid_addr) {
                    if mid_addr.0 == target_addr {
                        return sub_index;
                    }
                    sub_index += l.len();
                }
            }
        }

        if let Some(comment) = &self.line_comment {
            sub_index += comment.lines().count();
        }

        if let Some(label) = &self.label
            && label.len() >= LABEL_COLUMN_WIDTH
        {
            sub_index += 1;
        }

        sub_index
    }
}

#[derive(Debug, Clone)]
pub struct Disassembler {
    pub opcodes: [Option<Opcode>; 256],
}

impl Default for Disassembler {
    fn default() -> Self {
        Self::new()
    }
}

impl Disassembler {
    #[must_use]
    pub fn new() -> Self {
        Self {
            opcodes: crate::cpu::get_opcodes(),
        }
    }

    #[must_use]
    pub fn create_formatter(assembler: crate::state::Assembler) -> Box<dyn formatter::Formatter> {
        match assembler {
            crate::state::Assembler::Tass64 => Box::new(formatter_64tass::TassFormatter),
            crate::state::Assembler::Acme => Box::new(formatter_acme::AcmeFormatter),
            crate::state::Assembler::Ca65 => Box::new(formatter_ca65::Ca65Formatter),
            crate::state::Assembler::Kick => Box::new(formatter_kickasm::KickAsmFormatter),
        }
    }

    #[must_use]
    pub fn compute_local_label_names(
        &self,
        ctx: &DisassemblyContext,
        start_pc: usize,
        end_pc: usize,
        formatter: &dyn formatter::Formatter,
    ) -> std::collections::BTreeMap<Addr, String> {
        symbols::compute_local_label_names(&self.opcodes, ctx, start_pc, end_pc, formatter)
    }

    #[must_use]
    pub fn compute_scope_names(
        &self,
        ctx: &DisassemblyContext,
        formatter: &dyn formatter::Formatter,
    ) -> std::collections::BTreeMap<Addr, String> {
        symbols::compute_scope_names(ctx, formatter)
    }

    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn disassemble(
        &self,
        data: &[u8],
        block_types: &[crate::state::BlockType],
        labels: &std::collections::BTreeMap<Addr, Vec<crate::state::Label>>,
        origin: Addr,
        settings: &crate::state::DocumentSettings,
        annotations: &crate::state::AnnotationManager,
        cross_refs: &std::collections::BTreeMap<Addr, Vec<Addr>>,
        collapsed_blocks: &[(usize, usize)],
        splitters: &std::collections::BTreeSet<Addr>,
    ) -> Vec<DisassemblyLine> {
        let empty_enums = std::collections::BTreeMap::new();
        let ctx = DisassemblyContext {
            data,
            block_types,
            labels,
            origin,
            settings,
            annotations,
            cross_refs,
            collapsed_blocks,
            splitters,
            scope_ends: annotations.scope_ends(),
            enums: &empty_enums,
            user_global_enums: &empty_enums,
            builtin_enums: &empty_enums,
        };
        self.disassemble_ctx(&ctx)
    }

    #[must_use]
    pub fn disassemble_ctx(&self, ctx: &DisassemblyContext) -> Vec<DisassemblyLine> {
        pipeline::disassemble_ctx(&self.opcodes, ctx)
    }
}
