//! 6502/6510 disassembly pipeline, cross-assembler formatting, and shared per-line semantic lowering.
//!
//! This module decodes raw program bytes and [`crate::state::AppState`] metadata into
//! [`DisassemblyLine`] rows across four target assemblers (`64tass`, `ACME`, `ca65`, and `KickAssembler`).
//!
//! This module also defines the shared per-[`DisassemblyLine`] semantic lowering and visual
//! sub-line geometry layer. Both the batch exporter IR ([`crate::exporter::ir`]) and interactive
//! 2D-cursor consumers (the TUI disassembly view, search engine, navigation helpers, and action
//! handlers) consume this layer:
//!
//! - [`DisassemblyLineKind`] and [`DisassemblyLine::classify_kind`]: Classify lines without ad-hoc
//!   string matching.
//! - [`LabelPlacement`] and [`DisassemblyLine::label_placement`]: Check whether a formatted label
//!   fits within [`LABEL_COLUMN_WIDTH`] (`20` columns) or wraps onto [`LabelPlacement::OwnLine`].
//! - [`MidInstructionLabel`] and [`DisassemblyLine::mid_instruction_labels`]: Resolve at most one
//!   highest-priority relative label (`* + offset`) per intermediate instruction byte via
//!   [`resolve_label`].
//! - [`VisualLineCounts`] and [`DisassemblyLine::visual_line_counts`]: Compute the vertical
//!   sub-row breakdown (`0..labels`, `labels..labels + comments`, and `opcode_sub_index()`).
//!   These methods also map between 6502 [`crate::state::Addr`] values and visual sub-cursor indices.

pub mod context;
pub mod data_blocks;
mod engine;
pub mod formatter;
pub mod formatter_64tass;
pub mod formatter_acme;
pub mod formatter_ca65;
pub mod formatter_kickasm;
pub mod handlers;
mod line;
pub mod pipeline;
pub mod symbols;

pub use context::{DisassemblyContext, HandleArgs, ResolvedEnumValue, format_cross_references};
pub use engine::Disassembler;
pub use line::{
    CODE_COLUMN_WIDTH, DEFINITION_COLUMN_WIDTH, DisassembledChunk, DisassemblyLine,
    DisassemblyLineKind, INSTRUCTION_COLUMN_WIDTH, LABEL_COLUMN_WIDTH, LabelPlacement,
    MidInstructionLabel, SPLITTER_MNEMONIC, VisualLineCounts,
};
pub use symbols::{
    ScopeNameMap, resolve_label, resolve_label_name, resolve_symbolic_label_name,
    resolve_symbolic_offset,
};
