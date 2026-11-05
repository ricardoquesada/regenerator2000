//! Three-stage export pipeline for cross-assembler source files (`.asm` and `.bin` sidecars)
//! and interactive HTML documents (`.html` and sub-page sidecars).
//!
//! # Pipeline Stages
//!
//! 1. **Semantic Lowering ([`ir`]):** [`build_export_document`] lowers [`crate::state::AppState`]
//!    and [`ExportOptions`] into an [`ExportDocument`] (`Vec<ExportRow>`) in one pass. This stage
//!    does not perform disk I/O or clone [`crate::state::AppState`]. It resolves [`ExportHeader`]
//!    metadata, [`ExportRow::FileDirective`] rows, [`ExportRow::Equate`] definitions,
//!    [`ExportRow::EnumLine`] blocks, and [`ExportRow::Origin`] directives. It also groups
//!    [`crate::state::BlockType::ExternalFile`] ranges, formats [`MidLabel`] definitions,
//!    tokenizes [`OperandToken`] values, and parses [`ExportComment`] cross-references.
//! 2. **In-Memory Rendering ([`asm`], [`html`]):** [`render_asm`], [`render_asm_with_options`],
//!    [`render_asm_document`], [`render_html`], [`render_html_with_options`], and
//!    [`render_html_document`] consume an [`ExportDocument`]. Each renderer returns an in-memory
//!    [`ExportBundle`] that holds the primary file text and all [`ExportSidecar`] payloads.
//! 3. **Disk Write and Roundtrip Check ([`ExportBundle::write_to_disk`], [`verify`]):**
//!    [`export_asm`] and [`export_html`] write an [`ExportBundle`] to disk. [`verify_roundtrip`]
//!    and [`verify_all_assemblers`] assemble the exported source and check that the output bytes
//!    match `state.raw_data`.

pub mod asm;
pub mod html;
pub mod ir;
pub mod verify;

pub use asm::{export_asm, render_asm, render_asm_document, render_asm_with_options};
pub use html::{export_html, render_html, render_html_document, render_html_with_options};
pub use ir::{
    ExportBundle, ExportComment, ExportDocument, ExportHeader, ExportOptions, ExportRow,
    ExportSidecar, InstructionRow, LabelPlacement, MidLabel, OperandToken, XrefTarget,
    build_export_document, build_label_lookup_map, html_escape, tokenize_operand,
};
pub use verify::{VerifyResult, verify_all_assemblers, verify_roundtrip};
