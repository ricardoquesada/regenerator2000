//! In-memory and disk-backed assembler (`.asm` and `.bin` sidecars) exporter backend.
//!
//! This module implements Stage 2 (ASM Backend) of the exporter pipeline.
//! [`render_asm_document`] converts a lowered [`ExportDocument`] into an in-memory
//! [`ExportBundle`] without filesystem I/O:
//!
//! - Emits [`ExportRow::Equate`] and [`ExportRow::EnumLine`] rows at column 0 so multi-line
//!   `64tass` and `ca65` enum blocks do not shift by [`LABEL_COLUMN_WIDTH`].
//! - Emits [`ExportRow::FileDirective`] rows outside the header banner and aligns optional
//!   side comments to [`DEFINITION_COLUMN_WIDTH`].
//! - Records an [`ExportSidecar`] (`{stem}_{start:04x}_{end:04x}.bin`) for each
//!   [`ExportRow::ExternalFile`] block and emits the binary include directive after [`ExportRow::Origin`].
//! - Emits implied and operand-less instructions without trailing whitespace.

use super::ir::{
    ExportBundle, ExportComment, ExportDocument, ExportOptions, ExportRow, ExportSidecar,
    LabelPlacement, build_export_document,
};
use crate::disassembler::{DEFINITION_COLUMN_WIDTH, INSTRUCTION_COLUMN_WIDTH, LABEL_COLUMN_WIDTH};
use crate::error::CoreError;
use crate::state::AppState;
use std::fmt::Write as _;
use std::path::Path;

fn writeln_with_side_comment(
    output: &mut String,
    text: &str,
    width: usize,
    side_comment: Option<&ExportComment>,
    comment_prefix: &str,
) {
    if let Some(comment) = side_comment {
        let comment_text = comment.format_plain(comment_prefix);
        let _ = writeln!(output, "{text:<width$} {comment_prefix} {comment_text}");
    } else {
        let _ = writeln!(output, "{text}");
    }
}

/// Renders a lowered [`ExportDocument`] into an in-memory `.asm` [`ExportBundle`].
///
/// This function does not read or write the filesystem. It emits [`ExportRow::ExternalFile`]
/// blocks as binary include directives in `primary_content` and adds `.bin` payloads to
/// `bundle.sidecars`.
#[must_use]
pub fn render_asm_document(doc: &ExportDocument, path: &Path) -> ExportBundle {
    let mut output = String::new();
    let mut sidecars = Vec::new();

    let comment_prefix = &doc.header.comment_prefix;
    let separator = format!(
        "{comment_prefix}=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-\n"
    );
    output.push_str(&separator);
    let _ = writeln!(output, "{comment_prefix}");
    let _ = writeln!(output, "{}", doc.header.version_line);
    let _ = writeln!(output, "{comment_prefix} {}", doc.header.url);
    let _ = writeln!(output, "{comment_prefix}");

    for cmd_line in &doc.header.build_command_lines {
        let _ = writeln!(output, "{cmd_line}");
    }
    output.push_str(&separator);

    for row in &doc.rows {
        match row {
            ExportRow::Blank => {
                output.push('\n');
            }
            ExportRow::SectionComment { text } => {
                let _ = writeln!(output, "{text}");
            }
            ExportRow::FileDirective {
                directive,
                side_comment,
            } => {
                writeln_with_side_comment(
                    &mut output,
                    directive,
                    DEFINITION_COLUMN_WIDTH,
                    side_comment.as_ref(),
                    comment_prefix,
                );
            }
            ExportRow::Equate {
                definition,
                side_comment,
                ..
            } => {
                writeln_with_side_comment(
                    &mut output,
                    definition,
                    DEFINITION_COLUMN_WIDTH,
                    side_comment.as_ref(),
                    comment_prefix,
                );
            }
            ExportRow::EnumLine { text } => {
                // Emit enum lines at column 0 to preserve formatter-specific indentation.
                let _ = writeln!(output, "{text}");
            }
            ExportRow::Origin { directive, .. } => {
                let _ = writeln!(output, "{directive}");
            }
            ExportRow::ExternalFile {
                bin_filename,
                raw_bytes,
                asm_directive,
                ..
            } => {
                sidecars.push(ExportSidecar::new(
                    path,
                    bin_filename.clone(),
                    raw_bytes.clone(),
                ));
                let (mnemonic, operand) = asm_directive;
                if operand.is_empty() {
                    let _ = writeln!(output, "{mnemonic}");
                } else {
                    let _ = writeln!(output, "{mnemonic} {operand}");
                }
            }
            ExportRow::Instruction(inst) => {
                for comment_line in &inst.line_comments {
                    let _ = writeln!(output, "{comment_prefix} {comment_line}");
                }

                // Emit mid-instruction relative labels at column 0 and pad cross-reference
                // comments to `DEFINITION_COLUMN_WIDTH`.
                for mid in &inst.mid_labels {
                    writeln_with_side_comment(
                        &mut output,
                        &mid.definition,
                        DEFINITION_COLUMN_WIDTH,
                        mid.side_comment.as_ref(),
                        comment_prefix,
                    );
                }

                // Omit trailing spaces on implied and operand-less instructions (`nop`, `rts`).
                let instruction_part = if inst.operand_text.is_empty() {
                    inst.mnemonic.clone()
                } else if inst.mnemonic.is_empty() {
                    inst.operand_text.clone()
                } else {
                    format!("{} {}", inst.mnemonic, inst.operand_text)
                };

                if inst.is_scope_brace {
                    // Emit KickAssembler scope braces (`{` and `}`) at column 0 and place
                    // any attached scope label on the preceding line.
                    if let Some(label_text) = &inst.label {
                        let _ = writeln!(output, "{label_text}");
                    }
                    writeln_with_side_comment(
                        &mut output,
                        &instruction_part,
                        INSTRUCTION_COLUMN_WIDTH,
                        inst.side_comment.as_ref(),
                        comment_prefix,
                    );
                } else {
                    let label_part = match inst.label_placement {
                        LabelPlacement::OwnLine => {
                            if let Some(label_text) = &inst.label {
                                let _ = writeln!(output, "{label_text}");
                            }
                            ""
                        }
                        LabelPlacement::Inline => inst.label.as_deref().unwrap_or(""),
                        LabelPlacement::None => "",
                    };

                    let inst_line = if instruction_part.is_empty() && inst.side_comment.is_none() {
                        label_part.to_string()
                    } else {
                        format!("{label_part:<LABEL_COLUMN_WIDTH$}{instruction_part}")
                    };

                    writeln_with_side_comment(
                        &mut output,
                        &inst_line,
                        INSTRUCTION_COLUMN_WIDTH,
                        inst.side_comment.as_ref(),
                        comment_prefix,
                    );
                }
            }
        }
    }

    ExportBundle {
        primary_path: path.to_path_buf(),
        primary_content: output,
        sidecars,
    }
}

/// Lowers `state` with explicit [`ExportOptions`] and renders an in-memory `.asm` [`ExportBundle`].
#[must_use]
pub fn render_asm_with_options(
    state: &AppState,
    path: &Path,
    options: &ExportOptions,
) -> ExportBundle {
    let doc = build_export_document(state, options);
    render_asm_document(&doc, path)
}

/// Lowers `state` with the assembler in `state.settings` and renders an in-memory `.asm` [`ExportBundle`].
#[must_use]
pub fn render_asm(state: &AppState, path: &Path) -> ExportBundle {
    let options = ExportOptions::from_state_and_path(state, path);
    render_asm_with_options(state, path, &options)
}

/// Exports the disassembly to disk for the current assembler.
///
/// # Errors
/// Returns [`CoreError::Export`] if the primary `.asm` file or an external `.bin` sidecar cannot be written.
pub fn export_asm<P: AsRef<Path>>(state: &AppState, path: P) -> Result<(), CoreError> {
    render_asm(state, path.as_ref()).write_to_disk()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disassembler::DisassemblyLine;
    use crate::state::{
        Addr, AppState, Assembler, BlockType, EnumDefinition, Label, LabelKind, LabelType,
    };
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    #[test]
    fn test_export_external_file() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // Data: 0x1000..0x1004 (4 bytes)
        // 1000: NOP
        // 1001-1002: External File (2 bytes)
        // 1003: RTS
        state.raw_data = vec![0xEA, 0x11, 0x22, 0x60];
        state.block_types = vec![
            BlockType::Code,
            BlockType::ExternalFile,
            BlockType::ExternalFile,
            BlockType::Code,
        ];
        state.project_path = Some(PathBuf::from("/tmp/test_project.regen2000proj"));

        let path = Path::new("test_export_external.asm");

        // Test 64tass
        state.settings.assembler = Assembler::Tass64;
        let bundle = render_asm(&state, path);
        let content = &bundle.primary_content;

        // Check for .binary directive
        assert!(content.contains(".binary \"test_export_external_1001_1002.bin\""));
        assert!(!content.contains(".BYTE $11"));
        assert!(!content.contains(".byte $11"));

        // Check in-memory sidecar payload
        assert_eq!(bundle.sidecars.len(), 1);
        assert_eq!(
            bundle.sidecars[0].filename,
            "test_export_external_1001_1002.bin"
        );
        assert_eq!(bundle.sidecars[0].bytes, vec![0x11, 0x22]);

        // Test ACME
        state.settings.assembler = Assembler::Acme;
        let bundle_acme = render_asm(&state, path);
        assert!(
            bundle_acme
                .primary_content
                .contains("!binary \"test_export_external_1001_1002.bin\"")
        );
        assert_eq!(bundle_acme.sidecars.len(), 1);
        assert_eq!(bundle_acme.sidecars[0].bytes, vec![0x11, 0x22]);
    }

    #[test]
    fn test_export_compiles_with_64tass() {
        // 1. Setup AppState with some code
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // LDA #$00; STA $D020; RTS
        state.raw_data = vec![0xA9, 0x00, 0x8D, 0x20, 0xD0, 0x60];
        state.block_types = vec![BlockType::Code; state.raw_data.len()];

        // 2. Export to a temp file in std::env::temp_dir()
        let path =
            std::env::temp_dir().join(format!("test_output_64tass_{}.asm", std::process::id()));

        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }

        let res = export_asm(&state, &path);
        assert!(res.is_ok(), "Export failed: {:?}", res.err());

        // 3. Run 64tass (if available)
        let result = Command::new("64tass").arg(&path).output();

        match result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let _ = std::fs::remove_file(&path);

                assert!(
                    output.status.success(),
                    "64tass compilation failed. \nStdout: {stdout}\nStderr: {stderr}"
                );
            }
            Err(e) => {
                let _ = std::fs::remove_file(&path);
                if e.kind() == std::io::ErrorKind::NotFound {
                    println!("Skipping test: 64tass not found");
                    return;
                }
                panic!("Failed to execute 64tass: {e}");
            }
        }

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_export_mid_instruction_label() {
        let mut state = AppState::new();
        state.origin = Addr(0xC000);

        state.labels.insert(
            Addr(0xC000),
            vec![Label {
                name: "aC000".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );
        state.labels.insert(
            Addr(0xC001),
            vec![Label {
                name: "aC001".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );
        state.labels.insert(
            Addr(0xC002),
            vec![Label {
                name: "aC002".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );

        state.raw_data = vec![
            0x8D, 0x34, 0x12, // STA $1234
            0xAD, 0x01, 0xC0, // LDA $C001
        ];
        state.block_types = vec![BlockType::Code; state.raw_data.len()];

        let bundle = render_asm(&state, Path::new("test_mid_labels.asm"));
        let content = &bundle.primary_content;

        // Verify output contains the mid-instruction labels
        assert!(content.contains("aC001 =*+$01"));
        assert!(content.contains("aC002 =*+$02"));
    }

    #[test]
    fn test_export_includes_xrefs() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);

        state.labels.insert(
            Addr(0x1000),
            vec![Label {
                name: "MyLabel".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );

        state.raw_data = vec![0xEA];
        state.block_types = vec![BlockType::Code; 1];
        state.cross_refs.insert(
            Addr(0x1000),
            vec![
                crate::state::XRef::new(Addr(0x2000), crate::state::XRefKind::Read),
                crate::state::XRef::new(Addr(0x3000), crate::state::XRefKind::Read),
            ],
        );

        let bundle = render_asm(&state, Path::new("test_xref_export.asm"));
        let content = &bundle.primary_content;

        // Check that label, instruction and comment are on the same line
        assert!(content.contains("MyLabel"));
        assert!(!content.contains("MyLabel:"));
        assert!(content.contains("nop"));
        assert!(content.contains("; x-ref: $2000, $3000"));

        let line = content.lines().find(|l| l.contains("MyLabel")).unwrap();
        assert!(line.contains("nop"));
        assert!(line.contains("; x-ref"));
    }

    #[test]
    fn test_export_external_fields() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.raw_data = vec![0x00];

        state.labels.insert(
            Addr(0x0002),
            vec![Label {
                name: "zpf_0002".to_string(),
                kind: LabelKind::Auto,
                label_type: LabelType::ZeroPageField,
            }],
        );
        state.labels.insert(
            Addr(0xFFD2),
            vec![Label {
                name: "s_FFD2".to_string(),
                kind: LabelKind::Auto,
                label_type: LabelType::Subroutine,
            }],
        );

        state.disassembly.push(DisassemblyLine {
            address: Addr(0x1000),
            mnemonic: "NOP".to_string(),
            operand: String::new(),
            bytes: vec![0xEA],
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: true,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        });

        let externals = state.get_external_label_definitions(false);
        state.disassembly = externals.into_iter().chain(state.disassembly).collect();

        let bundle = render_asm(&state, Path::new("test_external_fields.asm"));
        let content = &bundle.primary_content;

        assert!(content.contains("zpf_0002 = $02"));
        assert!(content.contains("s_FFD2 = $ffd2"));
    }

    #[test]
    fn test_export_ca65_header() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.settings.assembler = Assembler::Ca65;
        state.raw_data = vec![0xEA];
        state.block_types = vec![BlockType::Code; 1];

        let bundle = render_asm(&state, Path::new("test_ca65_header.asm"));
        let content = &bundle.primary_content;

        assert!(content.contains(
            ";=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-"
        ));
        assert!(content.contains("; https://github.com/ricardoquesada/regenerator2000"));
        assert!(content.contains("; Assemble with:"));
        assert!(content.contains(
            ";   cl65 -t c64 -C c64-asm.cfg test_ca65_header.asm -o test_ca65_header.prg"
        ));
        assert!(content.contains(".macpack cbm"));
        assert!(content.contains("nop"));
    }

    #[test]
    fn test_export_headers_other_assemblers() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.raw_data = vec![0xEA];
        state.block_types = vec![BlockType::Code; 1];

        // ACME
        state.settings.assembler = Assembler::Acme;
        let bundle_acme = render_asm(&state, Path::new("test_acme_header.asm"));
        let content_acme = &bundle_acme.primary_content;
        assert!(content_acme.contains(
            ";=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-"
        ));
        assert!(content_acme.contains("; https://github.com/ricardoquesada/regenerator2000"));
        assert!(content_acme.contains("; Assemble with:"));
        assert!(
            content_acme
                .contains(";   acme --format cbm -o test_acme_header.prg test_acme_header.asm")
        );

        // KickAssembler
        state.settings.assembler = Assembler::Kick;
        let bundle_kick = render_asm(&state, Path::new("test_kick_header.asm"));
        let content_kick = &bundle_kick.primary_content;
        assert!(content_kick.contains(
            "//=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-"
        ));
        assert!(content_kick.contains("// https://github.com/ricardoquesada/regenerator2000"));
        assert!(content_kick.contains("// Assemble with:"));
        assert!(content_kick.contains("//   java -jar KickAss.jar test_kick_header.asm"));

        // 64tass
        state.settings.assembler = Assembler::Tass64;
        let bundle_tass = render_asm(&state, Path::new("test_tass_header.asm"));
        let content_tass = &bundle_tass.primary_content;
        assert!(content_tass.contains(
            ";=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-"
        ));
        assert!(content_tass.contains("; https://github.com/ricardoquesada/regenerator2000"));
        assert!(content_tass.contains("; Assemble with:"));
        assert!(
            content_tass.contains(";   64tass -C -o test_tass_header.prg test_tass_header.asm")
        );
    }

    #[test]
    fn test_export_label_ordering() {
        let mut state = AppState::new();
        state.origin = Addr(0xC000);
        state.raw_data = vec![0xEA]; // NOP at C000

        state.labels.insert(
            Addr(0x0010),
            vec![Label {
                name: "zpf_10".to_string(),
                kind: LabelKind::Auto,
                label_type: LabelType::ZeroPageField,
            }],
        );
        state.labels.insert(
            Addr(0x0005),
            vec![Label {
                name: "zpf_05".to_string(),
                kind: LabelKind::Auto,
                label_type: LabelType::ZeroPageField,
            }],
        );
        state.labels.insert(
            Addr(0x0020),
            vec![Label {
                name: "zpa_20".to_string(),
                kind: LabelKind::Auto,
                label_type: LabelType::ZeroPageAbsoluteAddress,
            }],
        );
        state.labels.insert(
            Addr(0x0030),
            vec![Label {
                name: "zpp_30".to_string(),
                kind: LabelKind::Auto,
                label_type: LabelType::ZeroPagePointer,
            }],
        );
        state.labels.insert(
            Addr(0x1000),
            vec![Label {
                name: "f_1000".to_string(),
                kind: LabelKind::Auto,
                label_type: LabelType::Field,
            }],
        );
        state.labels.insert(
            Addr(0x2000),
            vec![Label {
                name: "a_2000".to_string(),
                kind: LabelKind::Auto,
                label_type: LabelType::AbsoluteAddress,
            }],
        );
        state.labels.insert(
            Addr(0x3000),
            vec![Label {
                name: "p_3000".to_string(),
                kind: LabelKind::Auto,
                label_type: LabelType::Pointer,
            }],
        );
        state.labels.insert(
            Addr(0x4000),
            vec![Label {
                name: "e_4000".to_string(),
                kind: LabelKind::Auto,
                label_type: LabelType::ExternalJump,
            }],
        );
        state.labels.insert(
            Addr(0x5000),
            vec![Label {
                name: "b_5000".to_string(),
                kind: LabelKind::Auto,
                label_type: LabelType::Branch,
            }],
        );
        state.labels.insert(
            Addr(0x0011),
            vec![Label {
                name: "a_0011".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::AbsoluteAddress,
            }],
        );

        let externals = state.get_external_label_definitions(false);
        state.disassembly = externals.into_iter().chain(state.disassembly).collect();

        let bundle = render_asm(&state, Path::new("test_label_ordering.asm"));
        let content = &bundle.primary_content;

        assert!(content.contains("; EXTERNAL LABELS"));
        assert!(content.contains("zpf_ = Zero Page Field"));
        assert!(content.contains("zpa_ = Zero Page Absolute Address"));
        assert!(content.contains("zpp_ = Zero Page Pointer"));
        assert!(content.contains("f_   = Field"));
        assert!(content.contains("a_   = Absolute Address"));
        assert!(content.contains("p_   = Pointer"));
        assert!(content.contains("e_   = External Jump"));
        assert!(content.contains("L_   = Other / User-defined"));

        assert!(content.contains("zpf_05 = $05"));
        assert!(content.contains("zpf_10 = $10"));
        assert!(content.contains("a_0011 = $0011"));
        assert!(content.contains("zpa_20 = $20"));
        assert!(content.contains("zpp_30 = $30"));
        assert!(content.contains("f_1000 = $1000"));
        assert!(content.contains("a_2000 = $2000"));
        assert!(content.contains("p_3000 = $3000"));
        assert!(content.contains("e_4000 = $4000"));
        assert!(content.contains("b_5000 = $5000"));

        let order = [
            "zpf_05", "zpf_10", "a_0011", "zpa_20", "zpp_30", "f_1000", "a_2000", "p_3000",
            "e_4000", "b_5000",
        ];
        let mut last_pos = 0usize;
        for label in &order {
            let pos = content.find(label).expect("label not found in output");
            assert!(
                pos >= last_pos,
                "Label '{label}' appeared before the previous label in the output"
            );
            last_pos = pos;
        }

        assert!(!content.contains("; ZP FIELDS"));
        assert!(!content.contains("; ZP ABSOLUTE ADDRESSES"));
        assert!(!content.contains("; FIELDS"));
    }

    #[test]
    fn test_external_detection_no_name_check() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.raw_data = vec![0xEA];
        state.block_types = vec![BlockType::Code; 1];

        state.labels.insert(
            Addr(0x1000),
            vec![Label {
                name: "eUser".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );

        let bundle = render_asm(&state, Path::new("test_external_name_check.asm"));
        let content = &bundle.primary_content;

        assert!(content.contains("eUser"));
        assert!(!content.contains("eUser:"));
        assert!(!content.contains("eUser = $1000"));
    }

    #[test]
    fn test_export_absolute_zp_forcing() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);

        let data = vec![
            0xAD, 0x12, 0x00, // LDA $0012 (Absolute)
            0xBD, 0x12, 0x00, // LDA $0012,X (Absolute,X)
        ];
        state.raw_data = data.clone();
        state.block_types = vec![BlockType::Code; data.len()];

        let bundle = render_asm(&state, Path::new("test_export_force_zp.asm"));
        let content = &bundle.primary_content;

        assert!(
            content.contains("lda @w $0012"),
            "Output missing @w prefix for Absolute ZP target. content: {content}"
        );
        assert!(
            content.contains("lda @w $0012,x"),
            "Output missing @w prefix for AbsoluteX ZP target. content: {content}"
        );
    }

    #[test]
    fn test_export_ignores_collapsed_blocks() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);

        state.raw_data = vec![0xEA, 0xEA, 0xEA];
        state.block_types = vec![BlockType::Code; 3];
        state.collapsed_blocks.push((1, 1));
        state.disassemble();

        assert_eq!(state.disassembly.len(), 3);
        assert!(state.disassembly[1].mnemonic.contains("Collapsed"));

        let bundle = render_asm(&state, Path::new("test_export_collapsed.asm"));
        let content = &bundle.primary_content;

        assert!(
            !content.contains("Collapsed"),
            "Export should not contain collapsed block summary"
        );

        let nop_count = content.to_lowercase().matches("nop").count();
        assert_eq!(nop_count, 3, "Should export all 3 NOPs");
    }

    #[test]
    fn test_export_line_comments() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.settings.assembler = Assembler::Tass64;

        state.raw_data = vec![0xA9, 0x00];
        state.block_types = vec![BlockType::Code; 2];
        state.annotations.update(Addr(0x1000), |e| {
            e.user_line_comment = Some("Function Start".to_string());
        });
        state.labels.insert(
            Addr(0x1000),
            vec![Label {
                name: "MyLabel".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );

        let bundle = render_asm(&state, Path::new("test_export_line_comments.asm"));
        let content = &bundle.primary_content;

        assert!(content.contains("; Function Start"));

        let comment_idx = content.find("; Function Start").unwrap();
        let label_idx = content.find("MyLabel").unwrap();

        assert!(
            comment_idx < label_idx,
            "Line comment should appear before label"
        );
    }

    #[test]
    fn test_export_multiline_line_comments() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.settings.assembler = Assembler::Tass64;

        state.raw_data = vec![0xA9, 0x00];
        state.block_types = vec![BlockType::Code; 2];

        let multiline_comment = "Line 1\nLine 2\nLine 3".to_string();
        state.annotations.update(Addr(0x1000), |e| {
            e.user_line_comment = Some(multiline_comment);
        });

        let bundle = render_asm(&state, Path::new("test_export_multiline_line_comments.asm"));
        let content = &bundle.primary_content;

        assert!(content.contains("; Line 1"));
        assert!(content.contains("; Line 2"));
        assert!(content.contains("; Line 3"));

        for line in content.lines() {
            if line.contains("Line ") {
                assert!(
                    line.trim().starts_with(';'),
                    "Line should start with comment prefix: {line}"
                );
            }
        }
    }

    #[test]
    fn test_export_all_labels_disabled() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.raw_data = vec![0xEA];
        state.block_types = vec![BlockType::Code; 1];

        state.labels.insert(
            Addr(0x0010),
            vec![Label {
                name: "zpf_10".to_string(),
                kind: LabelKind::Auto,
                label_type: LabelType::ZeroPageField,
            }],
        );

        state.settings.all_labels = false;
        state.disassemble();

        for line in &state.disassembly {
            assert!(
                !line.mnemonic.contains("ZP FIELDS") && !line.mnemonic.contains("zpf_10 ="),
                "Disassembly contained external label definition but 'all_labels' is false!"
            );
        }

        let bundle = render_asm(&state, Path::new("test_export_all_labels_false.asm"));
        let content = &bundle.primary_content;

        assert!(content.contains("zpf_10 = $10"));
        assert!(content.contains("; EXTERNAL LABELS"));
        assert!(content.contains("zpf_ = Zero Page Field"));
    }

    #[test]
    fn test_export_kick_external_comments() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.settings.assembler = Assembler::Kick;
        state.settings.all_labels = true;
        state.raw_data = vec![0xEA];
        state.block_types = vec![BlockType::Code; 1];

        state.labels.insert(
            Addr(0x0002),
            vec![Label {
                name: "zpf_0002".to_string(),
                kind: LabelKind::Auto,
                label_type: LabelType::ZeroPageField,
            }],
        );

        let bundle = render_asm(&state, Path::new("test_kick_comments.asm"));
        let content = &bundle.primary_content;

        assert!(content.contains("// EXTERNAL LABELS"));
        assert!(content.contains("zpf_ = Zero Page Field"));
        assert!(content.contains("zpf_0002 = $02"));
    }

    #[test]
    fn test_export_kick_relative_labels() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.settings.assembler = Assembler::Kick;

        state.raw_data = vec![0x4C, 0x01, 0x10];
        state.block_types = vec![BlockType::Code; 3];

        state.labels.insert(
            Addr(0x1001),
            vec![Label {
                name: "rel1".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );

        let bundle = render_asm(&state, Path::new("test_kick_rel.asm"));
        let content = &bundle.primary_content;

        assert!(content.contains(".label rel1 = * + 1"));
    }

    /// Labels defined inside an `ExternalFile` block must appear in the exported
    /// ASM as constant definitions (under "EXTERNAL FILE LABELS"), since the
    /// exporter replaces those bytes with a .binary directive and the label
    /// definitions are never written inline.
    #[test]
    fn test_external_file_block_labels_declared_at_top() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.raw_data = vec![0xEA, 0x11, 0x22, 0x60];
        state.block_types = vec![
            BlockType::Code,
            BlockType::ExternalFile,
            BlockType::ExternalFile,
            BlockType::Code,
        ];

        state.labels.insert(
            Addr(0x1001),
            vec![Label {
                name: "ext_data".to_string(),
                label_type: LabelType::AbsoluteAddress,
                kind: LabelKind::User,
            }],
        );

        state.settings.assembler = Assembler::Tass64;
        state.disassemble();

        let definitions = state.get_external_label_definitions(false);
        let has_label_def = definitions
            .iter()
            .any(|l| l.external_label_address == Some(Addr(0x1001)));
        assert!(
            has_label_def,
            "Expected ext_data label definition line in external label definitions"
        );

        let bundle = render_asm(&state, Path::new("test_ext_file_labels.asm"));
        let content = &bundle.primary_content;

        assert!(
            content.contains("ext_data"),
            "Exported ASM must declare ext_data: got\n{content}"
        );
        assert!(
            content.contains(".binary"),
            "Exported ASM must have .binary directive"
        );
        assert_eq!(bundle.sidecars.len(), 1);
        assert_eq!(
            bundle.sidecars[0].filename,
            "test_ext_file_labels_1001_1002.bin"
        );
    }

    #[test]
    fn test_export_excludes_splitters() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.raw_data = vec![0xEA, 0x60]; // NOP, RTS
        state.block_types = vec![BlockType::Code; 2];
        state.splitters.insert(Addr(0x1001)); // Splitter at RTS

        let bundle = render_asm(&state, Path::new("test_splitter_exclude.asm"));
        let content = &bundle.primary_content;

        assert!(content.contains("nop"));
        assert!(content.contains("rts"));
        assert!(!content.contains("{splitter}"));
    }

    // ════════════════════════════════════════════════════════════════════════
    // Section 3 Regression Unit Tests
    // ════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_export_64tass_and_ca65_enums_unindented() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.raw_data = vec![0xA9, 0x00]; // LDA #$00
        state.block_types = vec![BlockType::Code; 2];

        let mut variants = BTreeMap::new();
        variants.insert(0, "BLACK".to_string());
        variants.insert(1, "WHITE".to_string());
        state.enums.insert(
            "Colors".to_string(),
            EnumDefinition {
                name: "Colors".to_string(),
                description: None,
                variants,
                source_file: None,
            },
        );
        state
            .annotations
            .update(Addr(0x1000), |e| e.enum_usage = Some("Colors".to_string()));

        // 1. 64tass: `.block` and `.bend` lack `=` and must not shift by 20 spaces
        state.settings.assembler = Assembler::Tass64;
        let tass_bundle = render_asm(&state, Path::new("enums_tass.asm"));
        let tass_content = &tass_bundle.primary_content;
        assert!(
            tass_content.contains("Colors .block\n    BLACK = $00\n    WHITE = $01\n.bend\n"),
            "64tass enum block must be unindented:\n{tass_content}"
        );
        assert!(
            !tass_content.contains("                    Colors .block"),
            "64tass .block must not be shifted by LABEL_COLUMN_WIDTH"
        );
        assert!(
            !tass_content.contains("                    .bend"),
            "64tass .bend must not be shifted by LABEL_COLUMN_WIDTH"
        );

        // 2. ca65: `.enum Colors` and `.endenum` lack `=` and must start at column 0
        state.settings.assembler = Assembler::Ca65;
        let ca65_bundle = render_asm(&state, Path::new("enums_ca65.asm"));
        let ca65_content = &ca65_bundle.primary_content;
        assert!(
            ca65_content.contains(".enum Colors\n    BLACK = $00\n    WHITE = $01\n.endenum\n"),
            "ca65 enum block must be unindented:\n{ca65_content}"
        );
        assert!(
            !ca65_content.contains("                    .enum Colors"),
            "ca65 .enum must not be shifted by LABEL_COLUMN_WIDTH"
        );
        assert!(
            !ca65_content.contains("                    .endenum"),
            "ca65 .endenum must not be shifted by LABEL_COLUMN_WIDTH"
        );

        // 3. KickAssembler & ACME sanity check
        state.settings.assembler = Assembler::Kick;
        let kick_bundle = render_asm(&state, Path::new("enums_kick.asm"));
        assert!(
            kick_bundle.primary_content.contains(
                ".namespace Colors {\n    .label BLACK = $00\n    .label WHITE = $01\n}\n"
            )
        );

        state.settings.assembler = Assembler::Acme;
        let acme_bundle = render_asm(&state, Path::new("enums_acme.asm"));
        assert!(
            acme_bundle
                .primary_content
                .contains("Colors_BLACK = $00\nColors_WHITE = $01\n")
        );
    }

    #[test]
    fn test_ca65_macpack_emitted_outside_header_banner() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.settings.assembler = Assembler::Ca65;
        state.raw_data = vec![0xEA];
        state.block_types = vec![BlockType::Code; 1];

        let bundle = render_asm(&state, Path::new("macpack_test.asm"));
        let content = &bundle.primary_content;

        let banner_sep =
            ";=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-";
        let first_sep = content.find(banner_sep).expect("first banner sep");
        let second_sep = content[first_sep + banner_sep.len()..]
            .find(banner_sep)
            .map(|offset| first_sep + banner_sep.len() + offset)
            .expect("second banner sep");

        let banner_body = &content[first_sep..second_sep + banner_sep.len()];
        assert!(
            !banner_body.contains(".macpack cbm"),
            ".macpack cbm must NOT appear inside the header comment banner:\n{banner_body}"
        );

        let after_banner = &content[second_sep + banner_sep.len()..];
        let expected_directive = format!(
            "{:<DEFINITION_COLUMN_WIDTH$} ; adds support for scrcode",
            ".macpack cbm"
        );
        assert!(
            after_banner.contains(&expected_directive),
            "Expected '{expected_directive}' after closing banner, got:\n{after_banner}"
        );
    }

    #[test]
    fn test_export_external_file_at_origin_emits_origin_before_binary() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.raw_data = vec![0x11, 0x22, 0x60];
        state.block_types = vec![
            BlockType::ExternalFile,
            BlockType::ExternalFile,
            BlockType::Code,
        ];

        let bundle = render_asm(&state, Path::new("leading_ext.asm"));
        let content = &bundle.primary_content;

        let origin_pos = content.find("* = $1000").expect("origin directive missing");
        let binary_pos = content
            .find(".binary \"leading_ext_1000_1001.bin\"")
            .expect(".binary directive missing");
        assert!(
            origin_pos < binary_pos,
            "Origin directive (* = $1000) must precede leading .binary directive:\n{content}"
        );
        assert_eq!(bundle.sidecars.len(), 1);
        assert_eq!(bundle.sidecars[0].filename, "leading_ext_1000_1001.bin");
        assert_eq!(bundle.sidecars[0].bytes, vec![0x11, 0x22]);
    }

    #[test]
    fn test_export_mid_instruction_label_includes_xrefs_and_formatter_syntax() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // BIT $1234 (2C 34 12) followed by JMP $1001 (4C 01 10)
        state.raw_data = vec![0x2C, 0x34, 0x12, 0x4C, 0x01, 0x10];
        state.block_types = vec![BlockType::Code; state.raw_data.len()];

        state.labels.insert(
            Addr(0x1001),
            vec![Label {
                name: "mid_target".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );
        state.cross_refs.insert(
            Addr(0x1001),
            vec![
                crate::state::XRef::new(Addr(0x2000), crate::state::XRefKind::Read),
                crate::state::XRef::new(Addr(0x1003), crate::state::XRefKind::Read),
            ],
        );

        // 1. 64tass syntax + `; x-ref: $1003, $2000`
        state.settings.assembler = Assembler::Tass64;
        let tass_bundle = render_asm(&state, Path::new("mid_xref_tass.asm"));
        let expected_tass = format!(
            "{:<DEFINITION_COLUMN_WIDTH$} ; x-ref: $1003, $2000",
            "mid_target =*+$01"
        );
        assert!(
            tass_bundle.primary_content.contains(&expected_tass),
            "Expected '{expected_tass}' in 64tass output:\n{}",
            tass_bundle.primary_content
        );

        // 2. KickAssembler syntax + `// x-ref: $1003, $2000`
        state.settings.assembler = Assembler::Kick;
        let kick_bundle = render_asm(&state, Path::new("mid_xref_kick.asm"));
        let expected_kick = format!(
            "{:<DEFINITION_COLUMN_WIDTH$} // x-ref: $1003, $2000",
            ".label mid_target = * + 1"
        );
        assert!(
            kick_bundle.primary_content.contains(&expected_kick),
            "Expected '{expected_kick}' in KickAssembler output:\n{}",
            kick_bundle.primary_content
        );
    }

    #[test]
    fn test_implied_instructions_have_no_trailing_space() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.raw_data = vec![0xEA, 0x60]; // NOP, RTS
        state.block_types = vec![BlockType::Code; 2];
        state.labels.insert(
            Addr(0x1000),
            vec![Label {
                name: "start".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );

        let bundle = render_asm(&state, Path::new("implied.asm"));
        let lines: Vec<&str> = bundle.primary_content.lines().collect();

        let nop_line = lines
            .iter()
            .find(|l| l.contains("nop"))
            .expect("nop line missing");
        assert_eq!(*nop_line, "start               nop");
        assert!(!nop_line.ends_with(' '));

        let rts_line = lines
            .iter()
            .find(|l| l.contains("rts"))
            .expect("rts line missing");
        assert_eq!(*rts_line, "                    rts");
        assert!(!rts_line.ends_with(' '));
    }

    #[test]
    fn test_export_scopes_and_long_label_wrapping() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.raw_data = vec![0xEA, 0x60, 0xEA]; // NOP at $1000, RTS at $1001, NOP at $1002
        state.block_types = vec![BlockType::Code; 3];

        // Add scope spanning $1000..=$1001 and a 20-char label at $1000
        state
            .annotations
            .update(Addr(0x1000), |e| e.scope = Some(Addr(0x1001)));
        state.labels.insert(
            Addr(0x1000),
            vec![Label {
                name: "twenty_chars_label_x".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::Subroutine,
            }],
        );

        // 1. KickAssembler: scope label on own line, `{` and `}` at column 0
        state.settings.assembler = Assembler::Kick;
        let kick_bundle = render_asm(&state, Path::new("scopes_kick.asm"));
        assert!(
            kick_bundle
                .primary_content
                .contains("twenty_chars_label_x:\n{\n")
        );
        assert!(kick_bundle.primary_content.contains("\n}\n"));

        // 2. ca65: `.proc twenty_chars_label_x` and `.endproc`
        state.settings.assembler = Assembler::Ca65;
        let ca65_bundle = render_asm(&state, Path::new("scopes_ca65.asm"));
        assert!(
            ca65_bundle
                .primary_content
                .contains(".proc twenty_chars_label_x")
        );
        assert!(ca65_bundle.primary_content.contains(".endproc"));

        // 3. 64tass without scope: 20-char label wraps to its own line (`LabelPlacement::OwnLine`)
        state.annotations.update(Addr(0x1000), |e| e.scope = None);
        state.settings.assembler = Assembler::Tass64;
        let tass_bundle = render_asm(&state, Path::new("long_label_tass.asm"));
        assert!(
            tass_bundle
                .primary_content
                .contains("twenty_chars_label_x\n                    nop")
        );
    }
}
