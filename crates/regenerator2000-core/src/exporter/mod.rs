//! Three-stage export pipeline for rendering disassembled 6502 projects into cross-assembler
//! source files (`.asm` + `.bin` sidecars) and interactive HTML documents (`.html` + sub-page sidecars).
//!
//! # Pipeline Architecture
//!
//! 1. **Semantic Lowering ([`ir`]):** [`build_export_document`] lowers [`crate::state::AppState`] and
//!    [`ExportOptions`] into a target-agnostic [`ExportDocument`] Intermediate Representation
//!    (`Vec<ExportRow>`) in a single pass with zero filesystem I/O and zero [`crate::state::AppState`]
//!    cloning. It resolves top-of-file metadata ([`ExportHeader`]), assembler setup directives
//!    ([`ExportRow::FileDirective`]), external/sidecar symbol equates ([`ExportRow::Equate`]), enum
//!    definitions ([`ExportRow::EnumLine`]), program origin placement ([`ExportRow::Origin`]),
//!    coalesced [`crate::state::BlockType::ExternalFile`] blocks ([`ExportRow::ExternalFile`]),
//!    mid-instruction relative labels ([`MidLabel`]), quote- and literal-aware operand tokens
//!    ([`OperandToken`]), and structured cross-reference side comments ([`ExportComment`]).
//! 2. **Pure In-Memory Backends ([`asm`], [`html`]):** Stateless renderers ([`render_asm`],
//!    [`render_asm_with_options`], [`render_asm_document`], [`render_html`],
//!    [`render_html_with_options`], and [`render_html_document`]) consume [`ExportDocument`] and
//!    produce an in-memory [`ExportBundle`] containing the primary file content and any associated
//!    [`ExportSidecar`] payloads.
//! 3. **Disk I/O & Roundtrip Verification ([`ExportBundle::write_to_disk`], [`verify`]):**
//!    [`export_asm`] and [`export_html`] write rendered [`ExportBundle`] artifacts to disk, while
//!    [`verify_roundtrip`] and [`verify_all_assemblers`] validate byte-identical reassembly across
//!    all supported cross-assemblers (`64tass`, `ACME`, `ca65`, and `KickAssembler`).

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
