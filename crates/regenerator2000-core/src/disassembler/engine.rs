//! Stateless MOS 6502/6510 disassembler engine and assembler formatter lookup.

use super::context::DisassemblyContext;
use super::line::DisassemblyLine;
use super::{
    formatter, formatter_64tass, formatter_acme, formatter_ca65, formatter_kickasm, pipeline,
};
use crate::cpu::{self, Opcode};
use crate::state::{Addr, AnnotationManager, Assembler, BlockType, DocumentSettings, Label, XRef};
use std::collections::{BTreeMap, BTreeSet};

/// Stateless MOS 6502/6510 disassembler engine holding the 256-entry [`Opcode`] lookup table.
#[derive(Debug, Clone)]
pub struct Disassembler {
    /// Complete 256-entry MOS 6502/6510 opcode table (including undocumented opcodes).
    pub opcodes: [Option<Opcode>; 256],
}

impl Default for Disassembler {
    fn default() -> Self {
        Self::new()
    }
}

impl Disassembler {
    /// Creates a new [`Disassembler`] initialized with the standard MOS 6502/6510 opcode table.
    #[must_use]
    pub fn new() -> Self {
        Self {
            opcodes: cpu::opcodes(),
        }
    }

    /// Returns a zero-allocation static reference to the [`Formatter`](formatter::Formatter) for `assembler`.
    ///
    /// Prefer this over [`Self::create_formatter`] in 60 FPS viewport layout, rendering, and
    /// search loops because all four formatter implementations are zero-sized unit structs.
    #[must_use]
    pub fn formatter(assembler: Assembler) -> &'static dyn formatter::Formatter {
        static ACME: formatter_acme::AcmeFormatter = formatter_acme::AcmeFormatter;
        static CA65: formatter_ca65::Ca65Formatter = formatter_ca65::Ca65Formatter;
        static KICK: formatter_kickasm::KickAsmFormatter = formatter_kickasm::KickAsmFormatter;
        static TASS: formatter_64tass::TassFormatter = formatter_64tass::TassFormatter;
        match assembler {
            Assembler::Acme => &ACME,
            Assembler::Ca65 => &CA65,
            Assembler::Kick => &KICK,
            Assembler::Tass64 => &TASS,
        }
    }

    /// Allocates a boxed [`Formatter`](formatter::Formatter) trait object for `assembler`.
    #[must_use]
    pub fn create_formatter(assembler: Assembler) -> Box<dyn formatter::Formatter> {
        match assembler {
            Assembler::Tass64 => Box::new(formatter_64tass::TassFormatter),
            Assembler::Acme => Box::new(formatter_acme::AcmeFormatter),
            Assembler::Ca65 => Box::new(formatter_ca65::Ca65Formatter),
            Assembler::Kick => Box::new(formatter_kickasm::KickAsmFormatter),
        }
    }

    /// Constructs a [`DisassemblyContext`] and disassembles `data` into [`DisassemblyLine`] items.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn disassemble(
        &self,
        data: &[u8],
        block_types: &[BlockType],
        labels: &BTreeMap<Addr, Vec<Label>>,
        origin: Addr,
        settings: &DocumentSettings,
        annotations: &AnnotationManager,
        cross_refs: &BTreeMap<Addr, Vec<XRef>>,
        collapsed_blocks: &[(usize, usize)],
        splitters: &BTreeSet<Addr>,
    ) -> Vec<DisassemblyLine> {
        let empty_enums = BTreeMap::new();
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
            scope_ends: annotations.scope_ends_set(),
            enums: &empty_enums,
            user_global_enums: &empty_enums,
            builtin_enums: &empty_enums,
        };
        self.disassemble_ctx(&ctx)
    }

    /// Disassembles the binary region described by `ctx` into [`DisassemblyLine`] rows.
    #[must_use]
    pub fn disassemble_ctx(&self, ctx: &DisassemblyContext) -> Vec<DisassemblyLine> {
        pipeline::disassemble_ctx(&self.opcodes, ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{LabelKind, LabelType};

    #[test]
    fn test_disassembler_formatter_static_lookup_matches_create_formatter() {
        for asm in [
            Assembler::Tass64,
            Assembler::Acme,
            Assembler::Ca65,
            Assembler::Kick,
        ] {
            let static_fmt = Disassembler::formatter(asm);
            let boxed_fmt = Disassembler::create_formatter(asm);
            assert_eq!(static_fmt.comment_prefix(), boxed_fmt.comment_prefix());
            assert_eq!(
                static_fmt.local_label_prefix(),
                boxed_fmt.local_label_prefix()
            );
            assert_eq!(
                static_fmt.format_label_definition("lbl"),
                boxed_fmt.format_label_definition("lbl")
            );
            assert_eq!(
                static_fmt.format_relative_label("lbl", 1),
                boxed_fmt.format_relative_label("lbl", 1)
            );
            assert_eq!(static_fmt.format_scope_end(), boxed_fmt.format_scope_end());
        }
    }

    #[test]
    fn test_scope_at_end_of_buffer_and_on_data_block_does_not_leak_or_duplicate_label() {
        let data = vec![0x01, 0x02];
        let block_types = vec![BlockType::DataByte, BlockType::DataByte];
        let mut labels = BTreeMap::new();
        labels.insert(
            Addr(0x1000),
            vec![Label {
                name: "data_scope".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );
        let mut annotations = AnnotationManager::default();
        annotations.update(Addr(0x1000), |e| {
            e.scope = Some(Addr(0x1001));
            e.user_line_comment = Some("scope header".to_string());
        });
        let settings = DocumentSettings {
            assembler: Assembler::Tass64,
            ..Default::default()
        };
        let cross_refs: BTreeMap<Addr, Vec<XRef>> = BTreeMap::new();
        let splitters = BTreeSet::new();

        let lines = Disassembler::new().disassemble(
            &data,
            &block_types,
            &labels,
            Addr(0x1000),
            &settings,
            &annotations,
            &cross_refs,
            &[],
            &splitters,
        );

        let block_lines: Vec<_> = lines.iter().filter(|l| l.mnemonic == ".block").collect();
        assert_eq!(block_lines.len(), 1);
        assert_eq!(block_lines[0].label.as_deref(), Some("data_scope"));
        assert_eq!(block_lines[0].line_comment.as_deref(), Some("scope header"));

        let byte_lines: Vec<_> = lines.iter().filter(|l| l.mnemonic == ".byte").collect();
        assert_eq!(byte_lines.len(), 1);
        assert_eq!(
            byte_lines[0].label, None,
            "DataByte row inside scope start must not duplicate the scope label"
        );
        assert_eq!(
            byte_lines[0].line_comment, None,
            "DataByte row inside scope start must not duplicate the scope line comment"
        );

        assert_eq!(
            lines.last().map(|l| l.mnemonic.as_str()),
            Some(".bend"),
            "Scope ending on the last byte of data must still emit its closing directive"
        );
    }

    #[test]
    fn test_split_byte_table_second_chunk_uses_label_priority_and_local_prefix() {
        let data = vec![0x00, 0x10, 0x20, 0x30];
        let block_types = vec![BlockType::LoHiWord; 4];
        let mut labels = BTreeMap::new();
        // Insert Auto label first and User local label second at the high-byte chunk start ($1002).
        labels.insert(
            Addr(0x1002),
            vec![
                Label {
                    name: "a1002".to_string(),
                    kind: LabelKind::Auto,
                    label_type: LabelType::AbsoluteAddress,
                },
                Label {
                    name: "hi_bytes".to_string(),
                    kind: LabelKind::User,
                    label_type: LabelType::LocalUserDefined,
                },
            ],
        );
        let annotations = AnnotationManager::default();
        let settings = DocumentSettings {
            assembler: Assembler::Tass64,
            ..Default::default()
        };
        let cross_refs: BTreeMap<Addr, Vec<XRef>> = BTreeMap::new();
        let splitters = BTreeSet::new();

        let lines = Disassembler::new().disassemble(
            &data,
            &block_types,
            &labels,
            Addr(0x1000),
            &settings,
            &annotations,
            &cross_refs,
            &[],
            &splitters,
        );

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[1].address, Addr(0x1002));
        assert_eq!(
            lines[1].label.as_deref(),
            Some("_hi_bytes"),
            "Second chunk of split-byte table must resolve highest-priority User label and apply local prefix"
        );
    }
}
