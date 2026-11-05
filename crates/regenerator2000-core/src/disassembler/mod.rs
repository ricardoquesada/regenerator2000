//! 6502/6510 disassembly pipeline, cross-assembler formatting, and shared per-line semantic lowering.
//!
//! This module decodes raw program bytes and [`AppState`] metadata into [`DisassemblyLine`] rows
//! across four target assemblers (`64tass`, `ACME`, `ca65`, and `KickAssembler`).
//!
//! This module also defines the shared per-[`DisassemblyLine`] semantic lowering and visual
//! sub-line geometry layer. Both the batch exporter IR ([`crate::exporter::ir`]) and interactive
//! 2D-cursor consumers (the TUI disassembly view, search engine, navigation helpers, and action
//! handlers) consume this layer:
//!
//! - [`DisassemblyLineKind`] and [`DisassemblyLine::classify_kind`]: Classify lines without ad-hoc
//!   string matching.
//! - [`LabelPlacement`] and [`DisassemblyLine::label_placement`]: Determine whether a formatted
//!   label fits inline within [`LABEL_COLUMN_WIDTH`] (`20` columns) or wraps onto its own preceding
//!   visual row (`OwnLine`).
//! - [`MidInstructionLabel`] and [`DisassemblyLine::mid_instruction_labels`]: Resolve at most one
//!   highest-priority relative label (`* + offset`) per intermediate instruction byte via
//!   [`resolve_label`].
//! - [`VisualLineCounts`] and [`DisassemblyLine::visual_line_counts`]: Compute the vertical
//!   sub-row breakdown (`0..labels`, `labels..labels + comments`, optional `OwnLine` label row, and
//!   `opcode_sub_index()`), and map between 6502 [`Addr`] values and visual sub-cursor indices.

use crate::cpu::Opcode;
use crate::state::{Addr, AppState, Assembler, LabelType};

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

/// Column width (`20`) reserved for inline labels before the instruction mnemonic column.
///
/// Labels whose formatted definition (`formatter.format_label_definition(name)`) has length
/// `>= LABEL_COLUMN_WIDTH` wrap onto their own preceding visual line ([`LabelPlacement::OwnLine`]).
/// This constant must match `.cc .il { width: 20ch }` and `.cc > .mn:first-child { padding-left: 20ch }`
/// in the HTML exporter CSS.
pub const LABEL_COLUMN_WIDTH: usize = 20;

/// Column width (`40`) for the `name = $value` portion of equate and mid-instruction relative
/// label definition lines in the batch ASM and HTML exporters.
///
/// This constant must match `.cc.as .lb { min-width: 40ch }` in the HTML exporter CSS.
pub const DEFINITION_COLUMN_WIDTH: usize = 40;

/// Column width (`40`) for the `label + mnemonic + operand` portion of instruction lines
/// before the separating space that precedes a trailing side comment.
pub const INSTRUCTION_COLUMN_WIDTH: usize = 40;

/// Total horizontal column width (`41`) of the code region (`INSTRUCTION_COLUMN_WIDTH + 1`)
/// before a side comment prefix (`;` or `//`) begins.
///
/// In the TUI disassembly renderer, normal instruction rows occupy `20` (label) + `6` (`"{:<5} "`
/// mnemonic) + `15` (operand) = `41` columns before the side comment. Padding mid-instruction
/// relative labels and external equates to `CODE_COLUMN_WIDTH` aligns trailing cross-reference
/// and side comments at column `41` across all row kinds.
pub const CODE_COLUMN_WIDTH: usize = INSTRUCTION_COLUMN_WIDTH + 1;

/// Canonical virtual mnemonic (`"{splitter}"`) for block splitter rows in [`DisassemblyLine`].
pub const SPLITTER_MNEMONIC: &str = "{splitter}";

/// High-level semantic classification of a [`DisassemblyLine`].
///
/// Replaces ad-hoc string matching (`line.mnemonic == "{splitter}"`,
/// `line.mnemonic == "{" || line.mnemonic == "}"`, and similar checks) across the exporter IR,
/// search engine, action handlers, and TUI disassembly renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisassemblyLineKind {
    /// Blank separator line (`bytes`, `mnemonic`, `label`, `comment`, and `line_comment` are all empty).
    Blank,
    /// Virtual block splitter marker (`mnemonic == SPLITTER_MNEMONIC`).
    Splitter,
    /// Collapsed block summary row (`is_collapsed == true`).
    CollapsedBlock,
    /// External label equate definition (`external_label_address == Some(addr)`).
    ExternalEquate(Addr),
    /// Preamble enum definition line emitted when `settings.all_labels` is active.
    ///
    /// [`DisassemblyLine::classify_kind`] checks this variant before [`Self::ScopeDelimiter`] so
    /// a closing `}` of a KickAssembler `.namespace` enum at [`Addr::ZERO`] classifies as
    /// `EnumDefinition` rather than `ScopeDelimiter`.
    EnumDefinition,
    /// Unindented lexical scope delimiter (`(mnemonic == "{" || mnemonic == "}") && !is_enum`).
    ScopeDelimiter,
    /// Full-line comment or assembler header directive stored inside `mnemonic`.
    HeaderComment,
    /// Standard instruction or data directive row (including standalone `line_comment` carriers).
    Instruction,
}

/// Placement of a label on an instruction or directive line relative to the
/// mnemonic column ([`LABEL_COLUMN_WIDTH`] = `20`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelPlacement {
    /// The line has no label, or is an external equate row whose label is embedded in `mnemonic`.
    None,
    /// The formatted label (for example, `main_loop:`) is shorter than [`LABEL_COLUMN_WIDTH`]
    /// (`20` characters) and fits inline to the left of the mnemonic.
    Inline,
    /// The formatted label is at least `20` characters long, or sits on an unindented scope
    /// delimiter (`{` or `}`), and wraps onto its own dedicated visual row above the instruction.
    OwnLine,
}

/// Resolved mid-instruction relative label (`label = * + offset`) for an
/// intermediate byte within a multi-byte [`DisassemblyLine`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MidInstructionLabel {
    /// Exact 6502 address of the intermediate byte (`line.address + offset`).
    pub address: Addr,
    /// Highest-priority resolved label name at `address`.
    pub name: String,
    /// Semantic [`LabelType`] of the resolved label.
    pub label_type: LabelType,
    /// Byte offset from `line.address` (`1..line.bytes.len()`).
    pub offset: usize,
}

impl MidInstructionLabel {
    /// Formats this mid-instruction relative label definition using `formatter`.
    ///
    /// Applies [`Formatter::local_label_prefix`](formatter::Formatter::local_label_prefix) when
    /// `self.label_type == LabelType::LocalUserDefined` and `!self.name.starts_with(prefix)`.
    #[must_use]
    pub fn format_definition(&self, formatter: &dyn formatter::Formatter) -> String {
        let effective_name = symbols::apply_local_prefix(
            &self.name,
            self.label_type,
            formatter.local_label_prefix(),
        );
        formatter.format_relative_label(&effective_name, self.offset)
    }
}

/// Breakdown of visual sub-lines occupied by a single [`DisassemblyLine`].
///
/// Visual sub-lines order top-to-bottom as:
///
/// 1. `0..labels`: Mid-instruction relative label definitions (`* + 1`, and so on).
/// 2. `labels..labels + comments`: Full-line `line_comment` rows.
/// 3. `labels + comments` (only when `instruction == 2`): Standalone [`LabelPlacement::OwnLine`] label row.
/// 4. [`Self::opcode_sub_index()`]: The main instruction, directive, splitter, or scope row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisualLineCounts {
    /// Number of mid-instruction relative label lines (`0..labels`).
    pub labels: usize,
    /// Number of full-line comment lines (`labels..labels + comments`).
    pub comments: usize,
    /// Number of instruction lines (`1` normally, or `2` when [`LabelPlacement::OwnLine`] holds).
    pub instruction: usize,
}

impl VisualLineCounts {
    /// Total number of visual rows occupied by this [`DisassemblyLine`].
    #[must_use]
    pub const fn total(&self) -> usize {
        self.labels + self.comments + self.instruction
    }

    /// Returns `true` when the line's label wraps onto its own dedicated visual row
    /// above the opcode or directive row (`instruction == 2`).
    #[must_use]
    pub const fn has_own_line_label(&self) -> bool {
        self.instruction > 1
    }

    /// Sub-line index of the standalone own-line label row (`labels + comments`), if present.
    #[must_use]
    pub const fn own_line_label_sub_index(&self) -> Option<usize> {
        if self.instruction > 1 {
            Some(self.labels + self.comments)
        } else {
            None
        }
    }

    /// Sub-line index of the main opcode, directive, splitter, or scope row.
    ///
    /// Unlike `self.labels + self.comments` (which points at the standalone label row
    /// when `self.instruction == 2`), `opcode_sub_index()` always points at the row
    /// containing the mnemonic and bytes (`self.total().saturating_sub(1)`).
    #[must_use]
    pub const fn opcode_sub_index(&self) -> usize {
        self.total().saturating_sub(1)
    }
}

/// One decoded instruction, data directive, equate, scope delimiter, or virtual marker line
/// produced by the disassembler pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisassemblyLine {
    /// Base 6502 memory address of this line (or [`Addr::ZERO`] for preamble equates and enums).
    pub address: Addr,
    /// Raw machine bytes covered by this instruction or data directive.
    pub bytes: Vec<u8>,
    /// Formatted instruction mnemonic, assembler directive, equate text, or [`SPLITTER_MNEMONIC`].
    pub mnemonic: String,
    /// Formatted instruction or directive operand string.
    pub operand: String,
    /// Trailing side comment text (including user/system side comments and formatted cross-references).
    pub comment: String,
    /// Optional multi-line block comment rendered on dedicated sub-rows above the instruction.
    pub line_comment: Option<String>,
    /// Unformatted symbol name attached to `address`, if any.
    pub label: Option<String>,
    /// Decoded MOS 6502/6510 [`Opcode`] metadata when this line represents a code instruction.
    pub opcode: Option<Opcode>,
    /// Whether the TUI and HTML renderers show raw hex `bytes` in the byte column.
    pub show_bytes: bool,
    /// Resolved control-flow target address for branches and jumps.
    pub target_address: Option<Addr>,
    /// Target address for preamble external label equate rows (`label = $addr`).
    pub external_label_address: Option<Addr>,
    /// Whether this row is a single-line summary of a collapsed memory range.
    pub is_collapsed: bool,
}

impl DisassemblyLine {
    /// Iterates over resolved mid-instruction labels at intermediate byte offsets (`1..self.bytes.len()`).
    fn iter_mid_instruction_labels<'a>(
        &'a self,
        state: &'a AppState,
    ) -> impl Iterator<Item = (usize, Addr, &'a crate::state::Label)> + 'a {
        let max_offset = if self.external_label_address.is_some() {
            0
        } else {
            self.bytes.len()
        };
        (1..max_offset).filter_map(move |offset| {
            let mid_addr = self.address.wrapping_add(offset as u16);
            let label_vec = state.labels.get(&mid_addr)?;
            let label = symbols::resolve_label(label_vec, mid_addr.0, &state.settings)?;
            Some((offset, mid_addr, label))
        })
    }

    /// Returns `true` if this line is a virtual `{splitter}` marker ([`SPLITTER_MNEMONIC`]).
    #[must_use]
    pub fn is_splitter(&self) -> bool {
        self.mnemonic == SPLITTER_MNEMONIC
    }

    /// Returns `true` if this line is a preamble enum definition line emitted when
    /// `settings.all_labels` is active.
    #[must_use]
    pub fn is_enum_definition(&self, comment_prefix: &str) -> bool {
        self.bytes.is_empty()
            && self.address == Addr::ZERO
            && self.external_label_address.is_none()
            && !self.mnemonic.is_empty()
            && !self.mnemonic.starts_with(comment_prefix)
    }

    /// Returns `true` if this line is an unindented lexical scope brace (`{` or `}`),
    /// excluding KickAssembler `.namespace` enum closing braces at [`Addr::ZERO`].
    #[must_use]
    pub fn is_unindented_scope(&self, comment_prefix: &str) -> bool {
        (self.mnemonic == "{" || self.mnemonic == "}") && !self.is_enum_definition(comment_prefix)
    }

    /// Returns `true` if the cursor can land on this line in the interactive disassembly view.
    #[must_use]
    pub fn is_navigable(&self) -> bool {
        !self.bytes.is_empty()
            || self.is_collapsed
            || self.label.is_some()
            || !self.mnemonic.is_empty()
    }

    /// Classifies the semantic role of this line given the active assembler's
    /// comment prefix (from [`Formatter::comment_prefix`](formatter::Formatter::comment_prefix)).
    #[must_use]
    pub fn classify_kind(&self, comment_prefix: &str) -> DisassemblyLineKind {
        if let Some(ext_addr) = self.external_label_address {
            return DisassemblyLineKind::ExternalEquate(ext_addr);
        }
        if self.is_collapsed {
            return DisassemblyLineKind::CollapsedBlock;
        }
        if self.is_splitter() {
            return DisassemblyLineKind::Splitter;
        }
        // Check `is_enum_definition` before `is_unindented_scope` so a KickAssembler
        // `.namespace` enum closing brace `}` at `Addr::ZERO` classifies as an enum line.
        if self.is_enum_definition(comment_prefix) {
            return DisassemblyLineKind::EnumDefinition;
        }
        if self.is_unindented_scope(comment_prefix) {
            return DisassemblyLineKind::ScopeDelimiter;
        }
        if self.bytes.is_empty() && self.label.is_none() {
            // Require `line_comment.is_none()` so standalone line-comment carriers classify
            // as `Instruction` rows rather than `Blank`.
            if self.mnemonic.is_empty() && self.comment.is_empty() && self.line_comment.is_none() {
                return DisassemblyLineKind::Blank;
            }
            if self.mnemonic.starts_with(comment_prefix) {
                return DisassemblyLineKind::HeaderComment;
            }
        }
        DisassemblyLineKind::Instruction
    }

    /// Returns the formatted label definition string (`formatter.format_label_definition`)
    /// if this line has a non-empty label.
    #[must_use]
    pub fn formatted_label(&self, formatter: &dyn formatter::Formatter) -> Option<String> {
        let label = self.label.as_deref().filter(|l| !l.is_empty())?;
        Some(formatter.format_label_definition(label))
    }

    /// Computes how this line's label (if any) sits relative to the mnemonic column
    /// under `formatter`.
    #[must_use]
    pub fn label_placement(&self, formatter: &dyn formatter::Formatter) -> LabelPlacement {
        // External equate rows embed their label inside `mnemonic` (`name = $addr`) and
        // never render a separate label column.
        if self.external_label_address.is_some() {
            return LabelPlacement::None;
        }
        let Some(formatted) = self.formatted_label(formatter) else {
            return LabelPlacement::None;
        };
        // Measure the formatted label (which includes `:` in `ca65` and `KickAssembler`)
        // and force `OwnLine` on unindented `{` / `}` scope delimiters.
        if formatted.len() >= LABEL_COLUMN_WIDTH
            || self.is_unindented_scope(formatter.comment_prefix())
        {
            LabelPlacement::OwnLine
        } else {
            LabelPlacement::Inline
        }
    }

    /// Counts how many intermediate byte addresses (`1..self.bytes.len()`) have a
    /// resolved mid-instruction label in `state` without heap allocation.
    #[must_use]
    pub fn mid_instruction_label_count(&self, state: &AppState) -> usize {
        self.iter_mid_instruction_labels(state).count()
    }

    /// Resolves all mid-instruction labels (`offset` in `1..self.bytes.len()`)
    /// using [`symbols::resolve_label`].
    #[must_use]
    pub fn mid_instruction_labels(&self, state: &AppState) -> Vec<MidInstructionLabel> {
        self.iter_mid_instruction_labels(state)
            .map(|(offset, address, label)| MidInstructionLabel {
                address,
                name: label.name.clone(),
                label_type: label.label_type,
                offset,
            })
            .collect()
    }

    /// Computes the visual sub-line counts for this line under `formatter`.
    #[must_use]
    pub fn visual_line_counts(
        &self,
        state: &AppState,
        formatter: &dyn formatter::Formatter,
    ) -> VisualLineCounts {
        if self.external_label_address.is_some() {
            return VisualLineCounts {
                labels: 0,
                comments: 0,
                instruction: 1,
            };
        }
        let labels = self.mid_instruction_label_count(state);
        let comments = self.line_comment.as_ref().map_or(0, |c| c.lines().count());
        let instruction = match self.label_placement(formatter) {
            LabelPlacement::OwnLine => 2,
            LabelPlacement::None | LabelPlacement::Inline => 1,
        };
        VisualLineCounts {
            labels,
            comments,
            instruction,
        }
    }

    /// Maps a visual `sub_index` back to the specific 6502 [`Addr`] it represents.
    ///
    /// When `sub_index` points to a mid-instruction relative label row (`0..labels`), this
    /// method returns that intermediate byte's [`Addr`]. Otherwise it returns
    /// `self.external_label_address.unwrap_or(self.address)`.
    #[must_use]
    pub fn address_at_sub_index(&self, state: &AppState, sub_index: usize) -> Addr {
        let base_addr = self.external_label_address.unwrap_or(self.address);
        self.iter_mid_instruction_labels(state)
            .nth(sub_index)
            .map_or(base_addr, |(_, mid_addr, _)| mid_addr)
    }

    /// Maps a target [`Addr`] within this line to its visual sub-line index.
    ///
    /// - If `target_addr` matches a resolved mid-instruction label (`self.address + offset`),
    ///   returns that mid-instruction label's sub-line index (`0..labels`).
    /// - Otherwise returns [`VisualLineCounts::opcode_sub_index()`], pointing at the opcode,
    ///   directive, collapsed block, or equate row.
    #[must_use]
    pub fn get_sub_index_for_address(&self, state: &AppState, target_addr: Addr) -> usize {
        if let Some(sub_index) = self
            .iter_mid_instruction_labels(state)
            .position(|(_, mid_addr, _)| mid_addr == target_addr)
        {
            return sub_index;
        }
        let formatter = Disassembler::formatter(state.settings.assembler);
        self.visual_line_counts(state, formatter).opcode_sub_index()
    }
}

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
            opcodes: crate::cpu::get_opcodes(),
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
        block_types: &[crate::state::BlockType],
        labels: &std::collections::BTreeMap<Addr, Vec<crate::state::Label>>,
        origin: Addr,
        settings: &crate::state::DocumentSettings,
        annotations: &crate::state::AnnotationManager,
        cross_refs: &std::collections::BTreeMap<Addr, Vec<crate::state::XRef>>,
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

    /// Disassembles the binary region described by `ctx` into [`DisassemblyLine`] rows.
    #[must_use]
    pub fn disassemble_ctx(&self, ctx: &DisassemblyContext) -> Vec<DisassemblyLine> {
        pipeline::disassemble_ctx(&self.opcodes, ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{AnnotationManager, BlockType, DocumentSettings, Label, LabelKind, XRef};
    use std::collections::{BTreeMap, BTreeSet};

    fn make_base_line() -> DisassemblyLine {
        DisassemblyLine {
            address: Addr(0x1000),
            bytes: vec![0xA9, 0x00],
            mnemonic: "LDA".to_string(),
            operand: "#$00".to_string(),
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: true,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        }
    }

    #[test]
    fn test_column_width_and_splitter_constants() {
        assert_eq!(LABEL_COLUMN_WIDTH, 20);
        assert_eq!(DEFINITION_COLUMN_WIDTH, 40);
        assert_eq!(INSTRUCTION_COLUMN_WIDTH, 40);
        assert_eq!(CODE_COLUMN_WIDTH, 41);
        assert_eq!(SPLITTER_MNEMONIC, "{splitter}");
    }

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
    fn test_classify_kind_all_variants_and_edge_cases() {
        // 1. ExternalEquate
        let mut ext_line = make_base_line();
        ext_line.address = Addr::ZERO;
        ext_line.bytes.clear();
        ext_line.mnemonic = "border = $d020".to_string();
        ext_line.external_label_address = Some(Addr(0xD020));
        assert_eq!(
            ext_line.classify_kind(";"),
            DisassemblyLineKind::ExternalEquate(Addr(0xD020))
        );
        assert!(ext_line.is_navigable());

        // 2. CollapsedBlock
        let mut collapsed_line = make_base_line();
        collapsed_line.bytes.clear();
        collapsed_line.is_collapsed = true;
        collapsed_line.mnemonic = "; Collapsed [1000-1010]".to_string();
        assert_eq!(
            collapsed_line.classify_kind(";"),
            DisassemblyLineKind::CollapsedBlock
        );
        assert!(collapsed_line.is_navigable());

        // 3. Splitter
        let mut splitter_line = make_base_line();
        splitter_line.bytes.clear();
        splitter_line.mnemonic = SPLITTER_MNEMONIC.to_string();
        assert!(splitter_line.is_splitter());
        assert_eq!(
            splitter_line.classify_kind(";"),
            DisassemblyLineKind::Splitter
        );
        assert!(splitter_line.is_navigable());

        // 4. EnumDefinition vs ScopeDelimiter
        let mut kick_enum_close = make_base_line();
        kick_enum_close.address = Addr::ZERO;
        kick_enum_close.bytes.clear();
        kick_enum_close.mnemonic = "}".to_string();
        assert!(kick_enum_close.is_enum_definition("//"));
        assert!(!kick_enum_close.is_unindented_scope("//"));
        assert_eq!(
            kick_enum_close.classify_kind("//"),
            DisassemblyLineKind::EnumDefinition
        );

        let mut scope_open = make_base_line();
        scope_open.address = Addr(0x1000);
        scope_open.bytes.clear();
        scope_open.mnemonic = "{".to_string();
        assert!(!scope_open.is_enum_definition("//"));
        assert!(scope_open.is_unindented_scope("//"));
        assert_eq!(
            scope_open.classify_kind("//"),
            DisassemblyLineKind::ScopeDelimiter
        );

        let mut scope_close = scope_open.clone();
        scope_close.mnemonic = "}".to_string();
        assert!(!scope_close.is_enum_definition("//"));
        assert!(scope_close.is_unindented_scope("//"));
        assert_eq!(
            scope_close.classify_kind("//"),
            DisassemblyLineKind::ScopeDelimiter
        );

        // 5. HeaderComment
        let mut header_comment = make_base_line();
        header_comment.bytes.clear();
        header_comment.mnemonic = "; Header".to_string();
        assert_eq!(
            header_comment.classify_kind(";"),
            DisassemblyLineKind::HeaderComment
        );
        assert!(header_comment.is_navigable());

        // 6. Blank vs line_comment
        let mut blank_line = make_base_line();
        blank_line.bytes.clear();
        blank_line.mnemonic.clear();
        blank_line.operand.clear();
        blank_line.comment.clear();
        blank_line.label = None;
        blank_line.line_comment = None;
        assert_eq!(blank_line.classify_kind(";"), DisassemblyLineKind::Blank);
        assert!(!blank_line.is_navigable());

        let mut comment_only_line = blank_line.clone();
        comment_only_line.line_comment = Some("standalone comment".to_string());
        assert_eq!(
            comment_only_line.classify_kind(";"),
            DisassemblyLineKind::Instruction
        );

        // 7. Standard Instruction
        let inst_line = make_base_line();
        assert_eq!(
            inst_line.classify_kind(";"),
            DisassemblyLineKind::Instruction
        );
        assert!(inst_line.is_navigable());
    }

    #[test]
    fn test_label_placement_19_char_boundary_and_scope_delimiters() {
        let tass = Disassembler::formatter(Assembler::Tass64);
        let acme = Disassembler::formatter(Assembler::Acme);
        let ca65 = Disassembler::formatter(Assembler::Ca65);
        let kick = Disassembler::formatter(Assembler::Kick);

        // No label or empty label
        let mut line = make_base_line();
        assert_eq!(line.formatted_label(tass), None);
        assert_eq!(line.label_placement(tass), LabelPlacement::None);

        line.label = Some(String::new());
        assert_eq!(line.formatted_label(tass), None);
        assert_eq!(line.label_placement(tass), LabelPlacement::None);

        // ExternalEquate with long label -> LabelPlacement::None
        let mut ext_line = make_base_line();
        ext_line.external_label_address = Some(Addr(0xD020));
        ext_line.label = Some("very_long_external_label_name".to_string());
        assert_eq!(ext_line.label_placement(tass), LabelPlacement::None);
        assert_eq!(ext_line.label_placement(ca65), LabelPlacement::None);

        // 19-character label ("nineteen_char_label", raw length 19)
        line.label = Some("nineteen_char_label".to_string());
        assert_eq!("nineteen_char_label".len(), 19);
        assert_eq!(
            line.formatted_label(tass).as_deref(),
            Some("nineteen_char_label")
        );
        assert_eq!(line.label_placement(tass), LabelPlacement::Inline);
        assert_eq!(line.label_placement(acme), LabelPlacement::Inline);

        assert_eq!(
            line.formatted_label(ca65).as_deref(),
            Some("nineteen_char_label:")
        );
        assert_eq!(line.label_placement(ca65), LabelPlacement::OwnLine);
        assert_eq!(line.label_placement(kick), LabelPlacement::OwnLine);

        // 20-character label ("twenty_char_label_xx", raw length 20)
        line.label = Some("twenty_char_label_xx".to_string());
        assert_eq!("twenty_char_label_xx".len(), 20);
        for fmt in [tass, acme, ca65, kick] {
            assert_eq!(line.label_placement(fmt), LabelPlacement::OwnLine);
        }

        // Named scope delimiter (`{` at $1000 with short label `my_scope`)
        let mut scope_line = make_base_line();
        scope_line.address = Addr(0x1000);
        scope_line.bytes.clear();
        scope_line.mnemonic = "{".to_string();
        scope_line.label = Some("my_scope".to_string());
        assert_eq!(scope_line.label_placement(kick), LabelPlacement::OwnLine);
    }

    #[test]
    fn test_mid_instruction_label_format_definition_local_prefix_and_dedup() {
        let tass = Disassembler::formatter(Assembler::Tass64);
        let acme = Disassembler::formatter(Assembler::Acme);
        let ca65 = Disassembler::formatter(Assembler::Ca65);
        let kick = Disassembler::formatter(Assembler::Kick);

        // LocalUserDefined without leading prefix
        let mid_local = MidInstructionLabel {
            address: Addr(0x1001),
            name: "loop".to_string(),
            label_type: LabelType::LocalUserDefined,
            offset: 1,
        };
        assert_eq!(mid_local.format_definition(tass), "_loop =*+$01");
        assert_eq!(mid_local.format_definition(acme), ".loop =*+$01");
        assert_eq!(mid_local.format_definition(ca65), "@loop =*+$01");
        assert_eq!(mid_local.format_definition(kick), ".label loop = * + 1");

        // LocalUserDefined with existing leading prefix must not double-prefix
        let mid_tass_prefixed = MidInstructionLabel {
            address: Addr(0x1001),
            name: "_loop".to_string(),
            label_type: LabelType::LocalUserDefined,
            offset: 1,
        };
        assert_eq!(mid_tass_prefixed.format_definition(tass), "_loop =*+$01");

        let mid_acme_prefixed = MidInstructionLabel {
            address: Addr(0x1001),
            name: ".loop".to_string(),
            label_type: LabelType::LocalUserDefined,
            offset: 1,
        };
        assert_eq!(mid_acme_prefixed.format_definition(acme), ".loop =*+$01");

        let mid_ca65_prefixed = MidInstructionLabel {
            address: Addr(0x1001),
            name: "@loop".to_string(),
            label_type: LabelType::LocalUserDefined,
            offset: 1,
        };
        assert_eq!(mid_ca65_prefixed.format_definition(ca65), "@loop =*+$01");

        // UserDefined (non-local) never prepends local prefix
        let mid_global = MidInstructionLabel {
            address: Addr(0x1001),
            name: "loop".to_string(),
            label_type: LabelType::UserDefined,
            offset: 1,
        };
        assert_eq!(mid_global.format_definition(tass), "loop =*+$01");
        assert_eq!(mid_global.format_definition(acme), "loop =*+$01");
        assert_eq!(mid_global.format_definition(ca65), "loop =*+$01");

        // Priority resolution: 2 labels at $1001 (Auto + User) -> resolves to 1 winning User label
        let mut state = AppState::new();
        state.labels.insert(
            Addr(0x1001),
            vec![
                Label {
                    name: "a1001".to_string(),
                    kind: LabelKind::Auto,
                    label_type: LabelType::AbsoluteAddress,
                },
                Label {
                    name: "user_mid".to_string(),
                    kind: LabelKind::User,
                    label_type: LabelType::UserDefined,
                },
            ],
        );

        let mut line = make_base_line();
        line.bytes = vec![0x4C, 0x00, 0x20];
        assert_eq!(line.mid_instruction_label_count(&state), 1);
        assert_eq!(
            line.mid_instruction_labels(&state),
            vec![MidInstructionLabel {
                address: Addr(0x1001),
                name: "user_mid".to_string(),
                label_type: LabelType::UserDefined,
                offset: 1,
            }]
        );
    }

    #[test]
    fn test_visual_line_counts_and_sub_index_helpers() {
        let mut state = AppState::new();
        state.labels.insert(
            Addr(0x1001),
            vec![Label {
                name: "mid1".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );

        let mut line = make_base_line();
        line.bytes = vec![0x4C, 0x00, 0x20];
        line.label = Some("nineteen_char_label".to_string());
        line.line_comment = Some("comment 1\ncomment 2".to_string());

        let tass = Disassembler::formatter(Assembler::Tass64);
        let acme = Disassembler::formatter(Assembler::Acme);
        let ca65 = Disassembler::formatter(Assembler::Ca65);
        let kick = Disassembler::formatter(Assembler::Kick);

        for fmt in [tass, acme] {
            let counts = line.visual_line_counts(&state, fmt);
            assert_eq!(
                counts,
                VisualLineCounts {
                    labels: 1,
                    comments: 2,
                    instruction: 1,
                }
            );
            assert_eq!(counts.total(), 4);
            assert!(!counts.has_own_line_label());
            assert_eq!(counts.own_line_label_sub_index(), None);
            assert_eq!(counts.opcode_sub_index(), 3);
        }

        for fmt in [ca65, kick] {
            let counts = line.visual_line_counts(&state, fmt);
            assert_eq!(
                counts,
                VisualLineCounts {
                    labels: 1,
                    comments: 2,
                    instruction: 2,
                }
            );
            assert_eq!(counts.total(), 5);
            assert!(counts.has_own_line_label());
            assert_eq!(counts.own_line_label_sub_index(), Some(3));
            assert_eq!(counts.opcode_sub_index(), 4);
        }

        // ExternalEquate line with line_comment -> { labels: 0, comments: 0, instruction: 1 }
        let mut ext_line = line.clone();
        ext_line.external_label_address = Some(Addr(0xD020));
        let ext_counts = ext_line.visual_line_counts(&state, ca65);
        assert_eq!(
            ext_counts,
            VisualLineCounts {
                labels: 0,
                comments: 0,
                instruction: 1,
            }
        );
        assert_eq!(ext_counts.opcode_sub_index(), 0);
    }

    #[test]
    fn test_address_at_sub_index_and_get_sub_index_for_address_roundtrip() {
        let mut state = AppState::new();
        state.settings.assembler = Assembler::Ca65;
        state.labels.insert(
            Addr(0x1001),
            vec![Label {
                name: "mid_target".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );

        let mut line = make_base_line();
        line.address = Addr(0x1000);
        line.bytes = vec![0x4C, 0x00, 0x20];
        line.line_comment = Some("c1\nc2".to_string());
        line.label = Some("nineteen_char_label".to_string());

        // Visual sub-lines in Ca65:
        // 0: mid-instruction label ($1001)
        // 1: comment "c1"
        // 2: comment "c2"
        // 3: OwnLine label ("nineteen_char_label:")
        // 4: opcode row ($1000)
        assert_eq!(line.address_at_sub_index(&state, 0), Addr(0x1001));
        assert_eq!(line.address_at_sub_index(&state, 1), Addr(0x1000));
        assert_eq!(line.address_at_sub_index(&state, 3), Addr(0x1000));
        assert_eq!(line.address_at_sub_index(&state, 4), Addr(0x1000));

        assert_eq!(line.get_sub_index_for_address(&state, Addr(0x1001)), 0);
        assert_eq!(line.get_sub_index_for_address(&state, Addr(0x1000)), 4);
        assert_eq!(line.get_sub_index_for_address(&state, Addr(0x1002)), 4);

        // ExternalEquate line
        let mut ext_line = make_base_line();
        ext_line.address = Addr::ZERO;
        ext_line.bytes.clear();
        ext_line.external_label_address = Some(Addr(0xD020));
        assert_eq!(ext_line.address_at_sub_index(&state, 0), Addr(0xD020));
        assert_eq!(ext_line.get_sub_index_for_address(&state, Addr(0xD020)), 0);
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
