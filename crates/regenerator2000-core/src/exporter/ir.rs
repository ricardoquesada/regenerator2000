//! Intermediate Representation ([`ExportDocument`]) and semantic lowering pass
//! ([`build_export_document`]) for the ASM and HTML exporter backends.
//!
//! This module implements **Stage 1** of the exporter pipeline. [`build_export_document`]
//! lowers [`AppState`] and [`ExportOptions`] into a sequence of typed [`ExportRow`] items
//! in memory. Backends render this IR directly without string-prefix heuristics on
//! [`DisassemblyLine::mnemonic`].

use crate::cpu::AddressingMode;
pub use crate::disassembler::LabelPlacement;
use crate::disassembler::{
    Disassembler, DisassemblyLine, DisassemblyLineKind, format_cross_references,
};
use crate::error::{CoreError, ExportError};
use crate::state::{Addr, AppState, Assembler, BlockType, DocumentSettings, LabelType, XRef};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Escapes `&`, `<`, and `>` for safe insertion into HTML text nodes.
#[must_use]
pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Configuration options that control export lowering and target assembler formatting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportOptions {
    /// Target cross-assembler dialect.
    pub assembler: Assembler,
    /// File stem of the destination export path (for example, `"program"` for `"program.asm"`).
    pub base_name: String,
    /// Optional source binary filename (for example, `"game.prg"`) shown in HTML headers.
    pub source_name: Option<String>,
}

impl ExportOptions {
    /// Builds [`ExportOptions`] from `state.settings.assembler` and the file stem of `path`.
    #[must_use]
    pub fn from_state_and_path(state: &AppState, path: &Path) -> Self {
        Self::for_assembler(state, path, state.settings.assembler)
    }

    /// Builds [`ExportOptions`] for `assembler` without cloning [`AppState`].
    #[must_use]
    pub fn for_assembler(state: &AppState, path: &Path, assembler: Assembler) -> Self {
        let base_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("export")
            .to_string();
        let source_name = state
            .file_path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(ToString::to_string);
        Self {
            assembler,
            base_name,
            source_name,
        }
    }
}

/// Secondary output file (such as a `.bin` slice or a sub-page `.html` file) produced with the
/// primary export file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportSidecar {
    /// Relative filename of the sidecar file (for example, `"export_1000_10ff.bin"`).
    pub filename: String,
    /// Full destination path on disk for the sidecar file.
    pub path: PathBuf,
    /// Raw payload bytes of the sidecar file.
    pub bytes: Vec<u8>,
}

impl ExportSidecar {
    /// Creates an [`ExportSidecar`] in the same directory as `primary_path`.
    #[must_use]
    pub fn new(
        primary_path: &Path,
        filename: impl Into<String>,
        bytes: impl Into<Vec<u8>>,
    ) -> Self {
        let filename = filename.into();
        let path = primary_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(&filename);
        Self {
            filename,
            path,
            bytes: bytes.into(),
        }
    }

    /// Returns the sidecar payload as a UTF-8 string slice if the bytes are valid UTF-8.
    #[must_use]
    pub fn content_utf8(&self) -> Option<&str> {
        std::str::from_utf8(&self.bytes).ok()
    }
}

/// Complete in-memory output of an [`ExportDocument`] render pass.
///
/// Holds the primary file content and all [`ExportSidecar`] files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportBundle {
    /// Destination path for the primary output file (`.asm` or `.html`).
    pub primary_path: PathBuf,
    /// Rendered UTF-8 text content of the primary output file.
    pub primary_content: String,
    /// Sidecar files (`.bin` slices for ASM or `_{addr:04x}.html` sub-pages for HTML).
    pub sidecars: Vec<ExportSidecar>,
}

impl ExportBundle {
    /// Writes all sidecar files and the primary export file to disk.
    ///
    /// This method writes sidecars first so that external `.bin` slices and HTML sub-pages
    /// exist on disk before the primary file appears.
    ///
    /// # Errors
    /// Returns [`CoreError::Export`] with [`ExportError::Io`] if any disk write fails.
    pub fn write_to_disk(&self) -> Result<(), CoreError> {
        for sidecar in &self.sidecars {
            std::fs::write(&sidecar.path, &sidecar.bytes).map_err(|source| ExportError::Io {
                path: sidecar.path.clone(),
                source,
            })?;
        }
        std::fs::write(&self.primary_path, &self.primary_content).map_err(|source| {
            ExportError::Io {
                path: self.primary_path.clone(),
                source,
            }
        })?;
        Ok(())
    }
}

/// Top-of-file metadata banner for an [`ExportDocument`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportHeader {
    /// Comment prefix for the active assembler (`";"` or `"//"`).
    pub comment_prefix: String,
    /// Version attribution line (for example, `"; Auto-generated by Regenerator 2000 v..."`).
    pub version_line: String,
    /// Project homepage URL.
    pub url: String,
    /// Source binary filename (`"unknown"` when no file path exists).
    pub source_name: String,
    /// Display name of the target assembler (for example, `"64tass"`).
    pub assembler_name: String,
    /// Official homepage URL of the target assembler.
    pub assembler_url: String,
    /// Commented build-command lines from
    /// [`Formatter::format_file_header`](crate::disassembler::formatter::Formatter::format_file_header).
    pub build_command_lines: Vec<String>,
}

/// One cross-reference target address in a side comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XrefTarget {
    /// Referenced memory address.
    pub address: Addr,
    /// `true` if `address` is outside the loaded binary range `[origin, origin + raw_data.len())`.
    pub is_external: bool,
}

/// Structured trailing side comment on a directive, equate, mid-label, or instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportComment {
    /// Optional primary comment text before `x-ref:` (user or system side comment).
    pub prefix_text: Option<String>,
    /// Sorted, deduplicated cross-reference addresses truncated to `max_xref_count`.
    pub xrefs: Vec<XrefTarget>,
    /// `true` when additional cross-references were truncated (`", ..."`).
    pub xrefs_truncated: bool,
    /// Optional trailing note after `x-ref:` (such as `"Border color"`).
    pub suffix_text: Option<String>,
}

/// Sorts, deduplicates, and truncates `refs` to `max_xref_count` [`XrefTarget`] items.
fn collect_xref_targets(
    refs: &[XRef],
    state: &AppState,
    max_xref_count: usize,
) -> (Vec<XrefTarget>, bool) {
    let mut all_refs: Vec<Addr> = refs.iter().map(|r| r.from).collect();
    all_refs.sort_unstable();
    all_refs.dedup();
    let xrefs_truncated = all_refs.len() > max_xref_count;
    let xrefs = all_refs
        .into_iter()
        .take(max_xref_count)
        .map(|r| XrefTarget {
            address: r,
            is_external: state.is_external(r),
        })
        .collect();
    (xrefs, xrefs_truncated)
}

impl ExportComment {
    /// Splits a non-empty [`DisassemblyLine::comment`] into its [`ExportComment`] parts.
    ///
    /// Returns `None` if `line_comment` is empty. This keeps wrapper lines (`.encode`,
    /// `.endencode`) and data continuation rows free of duplicate comments.
    #[must_use]
    pub fn from_line_comment(
        line_comment: &str,
        address: Addr,
        comment_prefix: &str,
        state: &AppState,
        settings: &DocumentSettings,
    ) -> Option<Self> {
        // Multi-line directives set `line.comment` to empty on wrapper and continuation rows.
        // Return `None` so those rows do not repeat the primary row's comment.
        if line_comment.is_empty() {
            return None;
        }

        let refs_opt = if settings.max_xref_count > 0 {
            state.cross_refs.get(&address).filter(|r| !r.is_empty())
        } else {
            None
        };

        // The disassembler joins the side comment and `format_cross_references` with
        // `" {comment_prefix} "`, then appends any `"; {target_comment}"` suffix.
        // Split once on `xref_str` to recover `prefix_text` and `suffix_text`.
        if let Some(refs) = refs_opt {
            let xref_str = format_cross_references(refs, settings.max_xref_count);
            if !xref_str.is_empty()
                && let Some((before, after)) = line_comment.split_once(&xref_str)
            {
                let sep = format!(" {comment_prefix} ");
                let cleaned_before = before.strip_suffix(&sep).unwrap_or(before);
                let prefix_text = if cleaned_before.is_empty() {
                    None
                } else {
                    Some(cleaned_before.to_string())
                };

                let cleaned_after = after.strip_prefix("; ").unwrap_or(after);
                let suffix_text = if cleaned_after.is_empty() {
                    None
                } else {
                    Some(cleaned_after.to_string())
                };

                let (xrefs, xrefs_truncated) =
                    collect_xref_targets(refs, state, settings.max_xref_count);

                return Some(Self {
                    prefix_text,
                    xrefs,
                    xrefs_truncated,
                    suffix_text,
                });
            }
        }

        // No cross-reference segment exists in `line_comment`.
        // Check whether `line_comment` starts with the annotation comment at `address`
        // followed by a `"; {suffix}"` target note.
        let base_comment = state.annotations.get(address).and_then(|entry| {
            entry
                .user_side_comment
                .as_deref()
                .or(entry.system_comment.as_deref())
        });

        if let Some(base) = base_comment
            && !base.is_empty()
            && let Some(after) = line_comment
                .strip_prefix(base)
                .and_then(|rest| rest.strip_prefix("; "))
            && !after.is_empty()
        {
            return Some(Self {
                prefix_text: Some(base.to_string()),
                xrefs: Vec::new(),
                xrefs_truncated: false,
                suffix_text: Some(after.to_string()),
            });
        }

        Some(Self {
            prefix_text: Some(line_comment.to_string()),
            xrefs: Vec::new(),
            xrefs_truncated: false,
            suffix_text: None,
        })
    }

    /// Builds an [`ExportComment`] that contains only cross-references for `address`.
    #[must_use]
    pub fn from_xrefs_only(
        state: &AppState,
        settings: &DocumentSettings,
        address: Addr,
    ) -> Option<Self> {
        if settings.max_xref_count == 0 {
            return None;
        }
        let refs = state.cross_refs.get(&address)?;
        if refs.is_empty() {
            return None;
        }

        let (xrefs, xrefs_truncated) = collect_xref_targets(refs, state, settings.max_xref_count);

        Some(Self {
            prefix_text: None,
            xrefs,
            xrefs_truncated,
            suffix_text: None,
        })
    }

    /// Formats this comment with custom text-escaping and cross-reference rendering functions.
    fn format_with(
        &self,
        comment_prefix: &str,
        escape: fn(&str) -> String,
        format_xref: fn(&XrefTarget) -> String,
    ) -> String {
        let mut base_parts = Vec::new();
        if let Some(prefix) = &self.prefix_text
            && !prefix.is_empty()
        {
            base_parts.push(escape(prefix));
        }
        if !self.xrefs.is_empty() {
            let refs_str: Vec<String> = self.xrefs.iter().map(format_xref).collect();
            let trunc = if self.xrefs_truncated { ", ..." } else { "" };
            base_parts.push(format!("x-ref: {}{}", refs_str.join(", "), trunc));
        }

        let sep = format!(" {} ", escape(comment_prefix));
        let mut out = base_parts.join(&sep);
        if let Some(suffix) = &self.suffix_text
            && !suffix.is_empty()
        {
            if !out.is_empty() {
                out.push_str("; ");
            }
            out.push_str(&escape(suffix));
        }
        out
    }

    /// Renders this comment as plain text for `.asm` output.
    #[must_use]
    pub fn format_plain(&self, comment_prefix: &str) -> String {
        self.format_with(comment_prefix, str::to_string, |x| {
            format!("${:04x}", x.address.0)
        })
    }

    /// Renders this comment as HTML with escaped text and linked internal cross-references.
    #[must_use]
    pub fn format_html(&self, comment_prefix: &str) -> String {
        // Anchor targets (`#L{:04X}`) use uppercase hex to match `<tr id="L{:04X}">` row IDs.
        // Display text (`${:04x}`) uses lowercase hex to match `format_plain`.
        self.format_with(comment_prefix, html_escape, |x| {
            if x.is_external {
                format!("${:04x}", x.address.0)
            } else {
                format!("<a href=\"#L{:04X}\">${:04x}</a>", x.address.0, x.address.0)
            }
        })
    }
}

/// Mid-instruction relative label definition (such as `rel1 =*+$01` or `.label rel1 = * + 1`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MidLabel {
    /// Exact memory address inside the multi-byte instruction or data item.
    pub address: Addr,
    /// Formatter-specific relative label definition text.
    pub definition: String,
    /// Optional cross-reference side comment for `address`.
    pub side_comment: Option<ExportComment>,
}

/// Parsed segment of an instruction or data directive operand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperandToken {
    /// Plain text (punctuation, register names, literals, or external symbol names).
    Text(String),
    /// Reference to an internal label within the loaded binary range.
    InternalLink {
        /// Visible symbol text in the operand.
        text: String,
        /// Target memory address of the internal symbol.
        target: Addr,
    },
}

/// Lowered representation of a body instruction, data directive, or scope delimiter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstructionRow {
    /// Memory address of the instruction or directive.
    pub address: Addr,
    /// Raw machine bytes for this row.
    pub bytes: Vec<u8>,
    /// `true` when raw hex bytes appear in the bytes column.
    pub show_bytes: bool,
    /// Full-line block comments above this instruction, split by line.
    pub line_comments: Vec<String>,
    /// Mid-instruction relative labels (`1..bytes.len()`) formatted for the active assembler.
    pub mid_labels: Vec<MidLabel>,
    /// Formatted label definition at `address`, if any.
    pub label: Option<String>,
    /// Column placement for `label` (`None`, `Inline`, or `OwnLine`).
    pub label_placement: LabelPlacement,
    /// `true` when `mnemonic` is a KickAssembler scope brace (`"{"` or `"}"`).
    pub is_scope_brace: bool,
    /// Formatted instruction mnemonic or directive name.
    pub mnemonic: String,
    /// Raw formatted operand string.
    pub operand_text: String,
    /// Tokenized operand segments for HTML escaping and link generation.
    pub operand_tokens: Vec<OperandToken>,
    /// Optional trailing side comment (`None` when [`DisassemblyLine::comment`] is empty).
    pub side_comment: Option<ExportComment>,
}

/// Semantic row in an [`ExportDocument`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportRow {
    /// Empty separator line.
    Blank,
    /// Prefixed section or legend comment line from the preamble (for example, `"; EXTERNAL LABELS"`).
    SectionComment {
        /// Full comment text including the leading comment prefix.
        text: String,
    },
    /// Top-level assembler setup directive outside the header banner (such as `.macpack cbm`).
    FileDirective {
        /// Directive text.
        directive: String,
        /// Optional side comment for the directive.
        side_comment: Option<ExportComment>,
    },
    /// Symbol equate definition from the external labels preamble (`name = $xxxx`).
    Equate {
        /// Target address of the external or `ExternalFile` label.
        address: Addr,
        /// Formatted equate definition (`name = $xxxx` or `.const name = $xxxx`).
        definition: String,
        /// Optional side comment and cross-references.
        side_comment: Option<ExportComment>,
    },
    /// One line of a formatted enum definition block.
    EnumLine {
        /// Line text from
        /// [`Formatter::format_enum_definition`](crate::disassembler::formatter::Formatter::format_enum_definition).
        text: String,
    },
    /// Program origin directive (`* = $xxxx` or `.org $xxxx`).
    Origin {
        /// Program origin address.
        address: Addr,
        /// Formatted origin directive string.
        directive: String,
    },
    /// Contiguous [`BlockType::ExternalFile`] region exported as a `.bin` or `.html` sidecar.
    ExternalFile {
        /// Inclusive start address of the external file block.
        start_address: Addr,
        /// Inclusive end address of the external file block.
        end_address: Addr,
        /// Relative `.bin` sidecar filename for ASM export.
        bin_filename: String,
        /// Relative `.html` sidecar filename for HTML export.
        html_filename: String,
        /// Raw binary slice for the `.bin` sidecar.
        raw_bytes: Vec<u8>,
        /// Disassembled inner rows rendered into the `.html` sidecar.
        inner_rows: Vec<ExportRow>,
        /// `(mnemonic, operand)` for the ASM binary include directive.
        asm_directive: (String, String),
        /// `(mnemonic, operand)` for the HTML binary include directive.
        html_directive: (String, String),
    },
    /// Standard code instruction, data directive, or scope delimiter row.
    Instruction(InstructionRow),
}

/// Complete lowered Intermediate Representation of an exported disassembly document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDocument {
    /// Top-of-file metadata and build instructions.
    pub header: ExportHeader,
    /// Ordered sequence of semantic rows in the document.
    pub rows: Vec<ExportRow>,
}

/// Builds a map from label names (including prefixed local names like `_loop`, `.loop`, and
/// `@loop`) to their [`Addr`].
#[must_use]
pub fn build_label_lookup_map(state: &AppState, assembler: Assembler) -> HashMap<String, Addr> {
    let formatter = Disassembler::formatter(assembler);
    let local_prefix = formatter.local_label_prefix();
    let mut label_name_to_addr = HashMap::new();
    for (addr, label_vec) in &state.labels {
        // Skip labels suppressed by an active Tier 2 `address_offset` so HTML anchors never
        // point to suppressed equate/body label IDs when `settings.auto_analyze` is `false`.
        if crate::disassembler::symbols::has_active_address_offset(
            *addr,
            &state.labels,
            &state.annotations,
        ) {
            continue;
        }
        for l in label_vec {
            label_name_to_addr.insert(l.name.clone(), *addr);
            if l.label_type == LabelType::LocalUserDefined
                && let Some(p) = local_prefix
                && !l.name.starts_with(p)
            {
                label_name_to_addr.insert(format!("{p}{}", l.name), *addr);
            }
        }
    }
    label_name_to_addr
}

/// Splits an instruction or data operand into plain text and internal label link tokens.
///
/// - Keeps quoted literals (`"..."` and `'...'`) as [`OperandToken::Text`]. It handles
///   backslash escapes (`\"`, `\\`) for `ACME` and `KickAssembler`, and doubled quotes (`""`)
///   for `64tass` and `ca65`.
/// - Keeps hexadecimal (`$xxxx`), binary (`%0101`), and decimal literals as
///   [`OperandToken::Text`] so hex digits like `$a0` or `$ff` never link to matching label names.
/// - Keeps 6502 index register suffixes (`,x`, `,y`, `,x)`) and the `ca65` `a:` / `64tass` `@w`
///   force-absolute prefixes as [`OperandToken::Text`].
/// - Splits [`SymbolicOffset`](crate::state::SymbolicOffset) expressions (`sub_C100 + 3`,
///   `#<(sub_C100 - 1)`, `<(sub_C100 - 1)`) so only the base identifier `sub_C100` becomes an
///   [`OperandToken::InternalLink`] to `sym.base`, while ` + 3` / ` - 1)` remains plain text.
/// - Resolves local labels prefixed with `_` (`64tass`), `.` (`ACME`), or `@` (`ca65`).
#[must_use]
pub fn tokenize_operand<S: std::hash::BuildHasher>(
    operand: &str,
    target_address: Option<Addr>,
    assembler: Assembler,
    state: &AppState,
    label_name_to_addr: &HashMap<String, Addr, S>,
) -> Vec<OperandToken> {
    if operand.is_empty() {
        return Vec::new();
    }

    // Fast path for branches and jumps whose entire operand resolves to one internal address.
    // Symbolic offset expressions (`sub_C100 + 3`, `sub_C100 - 1`) fall through to the
    // identifier scanner so only the base symbol links to `sym.base`, while PC-relative
    // `*+$nn` / `*-$nn` branch targets remain whole-operand links.
    if let Some(target) = target_address
        && !state.is_external(target)
        && (!operand.contains(['+', '-']) || operand.starts_with('*'))
    {
        return vec![OperandToken::InternalLink {
            text: operand.to_string(),
            target,
        }];
    }

    let backslash_escapes = matches!(assembler, Assembler::Acme | Assembler::Kick);

    let mut tokens = Vec::new();
    let mut text_buf = String::new();
    let mut chars = operand.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '"' || c == '\'' {
            // Keep the full quoted literal in `text_buf` so words inside strings do not become links.
            let quote = c;
            text_buf.push(c);
            while let Some(sc) = chars.next() {
                text_buf.push(sc);
                if backslash_escapes && sc == '\\' {
                    if let Some(escaped) = chars.next() {
                        text_buf.push(escaped);
                    }
                } else if sc == quote {
                    if !backslash_escapes && chars.peek() == Some(&quote) {
                        // `64tass` and `ca65` escape embedded quotes by doubling them (`""`).
                        if let Some(q2) = chars.next() {
                            text_buf.push(q2);
                        }
                    } else {
                        break;
                    }
                }
            }
        } else if c == '$' || c == '%' || c.is_ascii_digit() {
            // Keep numeric literals (`$hex`, `%bin`, or decimal) in `text_buf` so hex values
            // like `$a0`, `$ff`, or `$dead` do not match labels with the same name.
            text_buf.push(c);
            while let Some(&nc) = chars.peek() {
                if nc.is_alphanumeric() || nc == '_' {
                    text_buf.push(nc);
                    chars.next();
                } else {
                    break;
                }
            }
        } else if (c == '.' || c == '@')
            && chars
                .peek()
                .is_some_and(|&nc| nc.is_alphabetic() || nc == '_')
        {
            // A leading `.` or `@` can be a local label prefix (`.loop` in ACME, `@loop` in ca65)
            // or a field separator (`scope.field`). Try the prefixed symbol first, then the bare name.
            let mut ident = String::new();
            while let Some(&nc) = chars.peek() {
                if nc.is_alphanumeric() || nc == '_' {
                    ident.push(nc);
                    chars.next();
                } else {
                    break;
                }
            }
            let prefixed = format!("{c}{ident}");
            let is_local_prefix_for_asm = (c == '.' && assembler == Assembler::Acme)
                || (c == '@' && assembler == Assembler::Ca65);

            let prefixed_addr = label_name_to_addr.get(&prefixed).copied().or_else(|| {
                if is_local_prefix_for_asm {
                    label_name_to_addr.get(&ident).copied()
                } else {
                    None
                }
            });

            if let Some(addr) = prefixed_addr
                && !state.is_external(addr)
            {
                if !text_buf.is_empty() {
                    tokens.push(OperandToken::Text(std::mem::take(&mut text_buf)));
                }
                tokens.push(OperandToken::InternalLink {
                    text: prefixed,
                    target: addr,
                });
            } else if let Some(&addr) = label_name_to_addr.get(&ident)
                && !state.is_external(addr)
            {
                text_buf.push(c);
                tokens.push(OperandToken::Text(std::mem::take(&mut text_buf)));
                tokens.push(OperandToken::InternalLink {
                    text: ident,
                    target: addr,
                });
            } else {
                text_buf.push_str(&prefixed);
            }
        } else if c.is_alphabetic() || c == '_' {
            // Read an identifier or `64tass` `_`-prefixed local label (`_loop`).
            let mut word = String::from(c);
            while let Some(&nc) = chars.peek() {
                if nc.is_alphanumeric() || nc == '_' {
                    word.push(nc);
                    chars.next();
                } else {
                    break;
                }
            }

            // Skip `ca65` `a:` absolute-addressing prefixes (`lda a:$0012`) and 6502 index
            // register suffixes (`,x`, `,y`, `,x)`) so they never link to labels `a`, `x`, or `y`.
            let is_ca65_abs_prefix = assembler == Assembler::Ca65
                && word.eq_ignore_ascii_case("a")
                && chars.peek() == Some(&':')
                && chars.clone().nth(1) != Some(':');
            let is_index_register = text_buf.ends_with(',')
                && (word.eq_ignore_ascii_case("x") || word.eq_ignore_ascii_case("y"))
                && chars.peek().is_none_or(|&nc| nc == ')');

            if is_ca65_abs_prefix || is_index_register {
                text_buf.push_str(&word);
                continue;
            }

            let resolved_addr = label_name_to_addr.get(&word).copied().or_else(|| {
                if assembler == Assembler::Tass64
                    && let Some(stripped) = word.strip_prefix('_')
                {
                    label_name_to_addr.get(stripped).copied()
                } else {
                    None
                }
            });
            if let Some(addr) = resolved_addr
                && !state.is_external(addr)
            {
                if !text_buf.is_empty() {
                    tokens.push(OperandToken::Text(std::mem::take(&mut text_buf)));
                }
                tokens.push(OperandToken::InternalLink {
                    text: word,
                    target: addr,
                });
            } else {
                text_buf.push_str(&word);
            }
        } else {
            text_buf.push(c);
        }
    }

    if !text_buf.is_empty() {
        tokens.push(OperandToken::Text(text_buf));
    }

    tokens
}

/// Lowers one body [`DisassemblyLine`] into an [`InstructionRow`].
///
/// Uses the [`crate::disassembler`] helpers for mid-instruction labels, scope braces,
/// formatted labels, and [`LabelPlacement`] so batch exports match TUI layout rules.
fn lower_instruction_row(
    line: &DisassemblyLine,
    state: &AppState,
    settings: &DocumentSettings,
    formatter: &dyn crate::disassembler::formatter::Formatter,
    comment_prefix: &str,
    label_name_to_addr: &HashMap<String, Addr>,
) -> InstructionRow {
    let line_comments = line
        .line_comment
        .as_ref()
        .map(|c| c.lines().map(ToString::to_string).collect())
        .unwrap_or_default();

    let mid_labels = line
        .mid_instruction_labels(state)
        .into_iter()
        .map(|mid| {
            let definition = mid.format_definition(formatter);
            let side_comment = ExportComment::from_xrefs_only(state, settings, mid.address);
            MidLabel {
                address: mid.address,
                definition,
                side_comment,
            }
        })
        .collect();

    let is_scope_brace = line.is_unindented_scope(comment_prefix);
    let label = line.formatted_label(formatter);
    let label_placement = line.label_placement(formatter);

    let is_accumulator_mode = line
        .opcode
        .as_ref()
        .is_some_and(|op| op.mode == AddressingMode::Accumulator);
    let operand_tokens = if is_accumulator_mode {
        if line.operand.is_empty() {
            Vec::new()
        } else {
            vec![OperandToken::Text(line.operand.clone())]
        }
    } else {
        tokenize_operand(
            &line.operand,
            line.target_address,
            settings.assembler,
            state,
            label_name_to_addr,
        )
    };

    let side_comment = ExportComment::from_line_comment(
        &line.comment,
        line.address,
        comment_prefix,
        state,
        settings,
    );

    InstructionRow {
        address: line.address,
        bytes: line.bytes.clone(),
        show_bytes: line.show_bytes,
        line_comments,
        mid_labels,
        label,
        label_placement,
        is_scope_brace,
        mnemonic: line.mnemonic.clone(),
        operand_text: line.operand.clone(),
        operand_tokens,
        side_comment,
    }
}

/// Lowers [`AppState`] and [`ExportOptions`] into a target-agnostic [`ExportDocument`].
///
/// This function does not perform disk I/O or clone [`AppState`]. It lowers:
///
/// 1. Top-of-file metadata into [`ExportHeader`].
/// 2. Formatter file directives into [`ExportRow::FileDirective`].
/// 3. External and [`BlockType::ExternalFile`] label definitions into [`ExportRow::SectionComment`]
///    and [`ExportRow::Equate`].
/// 4. Enum definitions into [`ExportRow::SectionComment`] and [`ExportRow::EnumLine`].
/// 5. Program origin into [`ExportRow::Origin`] before any leading [`ExportRow::ExternalFile`]
///    or [`ExportRow::Instruction`].
/// 6. Body disassembly lines into [`ExportRow::ExternalFile`] and [`ExportRow::Instruction`].
#[must_use]
pub fn build_export_document(state: &AppState, options: &ExportOptions) -> ExportDocument {
    let formatter = Disassembler::formatter(options.assembler);
    let comment_prefix = formatter.comment_prefix().to_string();

    // Clone only `DocumentSettings` so `DisassemblyContext` uses `options.assembler`
    // without cloning `AppState`.
    let mut settings = state.settings.clone();
    settings.assembler = options.assembler;

    let formatter_header =
        formatter.format_file_header(&options.base_name, settings.use_illegal_opcodes);
    let build_command_lines = formatter_header.lines().map(ToString::to_string).collect();
    let header = ExportHeader {
        comment_prefix: comment_prefix.clone(),
        version_line: format!(
            "{comment_prefix} Auto-generated by Regenerator 2000 v{}",
            env!("CARGO_PKG_VERSION")
        ),
        url: "https://github.com/ricardoquesada/regenerator2000".to_string(),
        source_name: options
            .source_name
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
        assembler_name: formatter.name().to_string(),
        assembler_url: formatter.homepage_url().to_string(),
        build_command_lines,
    };

    let label_name_to_addr = build_label_lookup_map(state, options.assembler);
    let mut rows = Vec::new();

    // Lower preamble sections (directives, equates, and enums) before the body disassembly.
    // This avoids string heuristics and prevents collisions with body code at `$0000`.

    // 1. Top-level file directives (such as `.macpack cbm` in `ca65`).
    let file_directives = formatter.format_file_directives();
    if !file_directives.is_empty() {
        for fd in file_directives {
            let side_comment = fd.comment.filter(|c| !c.is_empty()).map(|c| ExportComment {
                prefix_text: Some(c),
                xrefs: Vec::new(),
                xrefs_truncated: false,
                suffix_text: None,
            });
            rows.push(ExportRow::FileDirective {
                directive: fd.directive,
                side_comment,
            });
        }
        rows.push(ExportRow::Blank);
    }

    // 2. External and `ExternalFile` label equates.
    let external_lines = state.get_external_label_definitions_with_formatter(true, formatter);
    for line in &external_lines {
        match line.classify_kind(&comment_prefix) {
            DisassemblyLineKind::Blank => rows.push(ExportRow::Blank),
            DisassemblyLineKind::HeaderComment => rows.push(ExportRow::SectionComment {
                text: line.mnemonic.clone(),
            }),
            _ => {
                let addr = line.external_label_address.unwrap_or(line.address);
                let side_comment = ExportComment::from_line_comment(
                    &line.comment,
                    addr,
                    &comment_prefix,
                    state,
                    &settings,
                );
                rows.push(ExportRow::Equate {
                    address: addr,
                    definition: line.mnemonic.clone(),
                    side_comment,
                });
            }
        }
    }

    // 3. Enum definitions.
    let enum_lines = state.get_enum_definition_lines_with_formatter(formatter);
    for line in &enum_lines {
        match line.classify_kind(&comment_prefix) {
            DisassemblyLineKind::Blank => rows.push(ExportRow::Blank),
            DisassemblyLineKind::HeaderComment => rows.push(ExportRow::SectionComment {
                text: line.mnemonic.clone(),
            }),
            _ => rows.push(ExportRow::EnumLine {
                text: line.mnemonic.clone(),
            }),
        }
    }

    // 4. Disassemble the body without collapsed blocks.
    let ctx = crate::disassembler::context::DisassemblyContext {
        data: &state.raw_data,
        block_types: &state.block_types,
        labels: &state.labels,
        origin: state.origin,
        settings: &settings,
        annotations: &state.annotations,
        cross_refs: &state.cross_refs,
        collapsed_blocks: &[],
        splitters: &state.splitters,
        scope_ends: state.annotations.scope_ends_set(),
        enums: &state.enums,
        user_global_enums: &state.user_global_enums,
        builtin_enums: &state.builtin_enums,
    };
    let full_disassembly = state.disassembler.disassemble_ctx(&ctx);

    let origin_directive = formatter.format_header_origin(state.origin);
    let mut origin_emitted = false;

    let mut i = 0;
    while i < full_disassembly.len() {
        let line = &full_disassembly[i];
        let kind = line.classify_kind(&comment_prefix);

        if kind == DisassemblyLineKind::Splitter {
            i += 1;
            continue;
        }

        if kind == DisassemblyLineKind::Blank {
            rows.push(ExportRow::Blank);
            i += 1;
            continue;
        }

        // Emit `ExportRow::Origin` before the first non-blank body row so it precedes
        // any leading `BlockType::ExternalFile` include directive.
        if !origin_emitted {
            rows.push(ExportRow::Origin {
                address: state.origin,
                directive: origin_directive.clone(),
            });
            origin_emitted = true;
        }

        let offset = line.address.offset_from(state.origin);
        if state.block_types.get(offset) == Some(&BlockType::ExternalFile) {
            let start_idx = offset;
            let mut end_idx = start_idx;
            while state.block_types.get(end_idx) == Some(&BlockType::ExternalFile) {
                end_idx += 1;
            }
            let byte_count = end_idx - start_idx;
            let start_address = line.address;
            let end_address = start_address
                .wrapping_add(byte_count as u16)
                .wrapping_sub(1);
            let raw_bytes = state
                .raw_data
                .get(start_idx..end_idx)
                .unwrap_or(&[])
                .to_vec();

            let bin_filename = format!(
                "{}_{:04x}_{:04x}.bin",
                options.base_name, start_address.0, end_address.0
            );
            let html_filename = format!("{}_{:04x}.html", options.base_name, start_address.0);
            let asm_directive = formatter.format_binary_include(&bin_filename);
            let html_directive = formatter.format_binary_include(&html_filename);

            let mut inner_rows = Vec::new();
            while i < full_disassembly.len() {
                let next_line = &full_disassembly[i];
                if next_line.address.offset_from(start_address) >= byte_count {
                    break;
                }
                match next_line.classify_kind(&comment_prefix) {
                    DisassemblyLineKind::Splitter => {}
                    DisassemblyLineKind::Blank => inner_rows.push(ExportRow::Blank),
                    _ => inner_rows.push(ExportRow::Instruction(lower_instruction_row(
                        next_line,
                        state,
                        &settings,
                        formatter,
                        &comment_prefix,
                        &label_name_to_addr,
                    ))),
                }
                i += 1;
            }

            rows.push(ExportRow::ExternalFile {
                start_address,
                end_address,
                bin_filename,
                html_filename,
                raw_bytes,
                inner_rows,
                asm_directive,
                html_directive,
            });
            continue;
        }

        if kind == DisassemblyLineKind::HeaderComment {
            rows.push(ExportRow::SectionComment {
                text: line.mnemonic.clone(),
            });
            i += 1;
            continue;
        }

        rows.push(ExportRow::Instruction(lower_instruction_row(
            line,
            state,
            &settings,
            formatter,
            &comment_prefix,
            &label_name_to_addr,
        )));
        i += 1;
    }

    if !origin_emitted {
        rows.push(ExportRow::Origin {
            address: state.origin,
            directive: origin_directive,
        });
    }

    ExportDocument { header, rows }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{EnumDefinition, Label, LabelKind};
    use std::collections::BTreeMap;

    fn make_state_with_internal_labels(
        origin: u16,
        size: usize,
        labels: &[(u16, &str, LabelType)],
    ) -> AppState {
        let mut state = AppState::new();
        state.origin = Addr(origin);
        state.raw_data = vec![0xEA; size];
        state.block_types = vec![BlockType::Code; size];
        for &(addr, name, label_type) in labels {
            state.labels.insert(
                Addr(addr),
                vec![Label {
                    name: name.to_string(),
                    kind: LabelKind::User,
                    label_type,
                }],
            );
        }
        state
    }

    #[test]
    fn test_tokenize_operand_quoted_strings_not_linkified() {
        let state = make_state_with_internal_labels(
            0x1000,
            0x40,
            &[
                (0x1000, "start", LabelType::UserDefined),
                (0x1010, "play", LabelType::UserDefined),
                (0x1020, "foo", LabelType::UserDefined),
            ],
        );
        let map = build_label_lookup_map(&state, Assembler::Tass64);

        // 1. Standard string (64tass / ca65): words inside quotes are never linkified
        let tokens = tokenize_operand(
            "\"press start to play\"",
            None,
            Assembler::Tass64,
            &state,
            &map,
        );
        assert_eq!(
            tokens,
            vec![OperandToken::Text("\"press start to play\"".to_string())]
        );

        // Single-quoted literal
        let single_q = tokenize_operand("'start'", None, Assembler::Tass64, &state, &map);
        assert_eq!(single_q, vec![OperandToken::Text("'start'".to_string())]);

        // 2. 64tass doubled quotes (""): `""start""` stays inside the string, trailing `, start` links
        let tokens_tass = tokenize_operand(
            "\"say \"\"start\"\" now\", start",
            None,
            Assembler::Tass64,
            &state,
            &map,
        );
        assert_eq!(
            tokens_tass,
            vec![
                OperandToken::Text("\"say \"\"start\"\" now\", ".to_string()),
                OperandToken::InternalLink {
                    text: "start".to_string(),
                    target: Addr(0x1000),
                },
            ]
        );

        // In 64tass, `\` is a literal character and does NOT escape the closing quote
        let tokens_tass_backslash =
            tokenize_operand("\"C:\\\", start", None, Assembler::Tass64, &state, &map);
        assert_eq!(
            tokens_tass_backslash,
            vec![
                OperandToken::Text("\"C:\\\", ".to_string()),
                OperandToken::InternalLink {
                    text: "start".to_string(),
                    target: Addr(0x1000),
                },
            ]
        );

        // 3. ACME / KickAssembler backslash-escaped quotes (\" and \\)
        for asm in [Assembler::Acme, Assembler::Kick] {
            let tokens_escaped =
                tokenize_operand("\"say \\\"start\\\" \\\\\", start", None, asm, &state, &map);
            assert_eq!(
                tokens_escaped,
                vec![
                    OperandToken::Text("\"say \\\"start\\\" \\\\\", ".to_string()),
                    OperandToken::InternalLink {
                        text: "start".to_string(),
                        target: Addr(0x1000),
                    },
                ]
            );
        }

        // 4. ca65 mixed string and byte operands
        let tokens_ca65 = tokenize_operand(
            "\"press start\", $22, \"play\", start",
            None,
            Assembler::Ca65,
            &state,
            &map,
        );
        assert_eq!(
            tokens_ca65,
            vec![
                OperandToken::Text("\"press start\", $22, \"play\", ".to_string()),
                OperandToken::InternalLink {
                    text: "start".to_string(),
                    target: Addr(0x1000),
                },
            ]
        );
    }

    #[test]
    fn test_tokenize_operand_numeric_literals_not_linkified() {
        let state = make_state_with_internal_labels(
            0x1000,
            0x20,
            &[
                (0x1000, "a0", LabelType::UserDefined),
                (0x1001, "ff", LabelType::UserDefined),
                (0x1002, "b0", LabelType::UserDefined),
                (0x1003, "c000", LabelType::UserDefined),
                (0x1004, "dead", LabelType::UserDefined),
            ],
        );
        let map = build_label_lookup_map(&state, Assembler::Tass64);

        // 1. Hex literals ($)
        for hex_op in [
            "#$a0",
            "$ff",
            "$c000,x",
            "$dead",
            "$a0, $ff, $b0",
            "#<$c000",
        ] {
            let tokens = tokenize_operand(hex_op, None, Assembler::Tass64, &state, &map);
            assert_eq!(
                tokens,
                vec![OperandToken::Text(hex_op.to_string())],
                "Hex literal {hex_op} should not be linkified"
            );
        }

        // 2. Binary literals (%)
        for bin_op in ["#%10101010", "%11111111"] {
            let tokens = tokenize_operand(bin_op, None, Assembler::Tass64, &state, &map);
            assert_eq!(tokens, vec![OperandToken::Text(bin_op.to_string())]);
        }

        // 3. Decimal literals (0-9)
        for dec_op in ["#123", "0", "255"] {
            let tokens = tokenize_operand(dec_op, None, Assembler::Tass64, &state, &map);
            assert_eq!(tokens, vec![OperandToken::Text(dec_op.to_string())]);
        }

        // 4. Mixed literal + label
        let mixed = tokenize_operand("$a0 + a0", None, Assembler::Tass64, &state, &map);
        assert_eq!(
            mixed,
            vec![
                OperandToken::Text("$a0 + ".to_string()),
                OperandToken::InternalLink {
                    text: "a0".to_string(),
                    target: Addr(0x1000),
                },
            ]
        );
    }

    #[test]
    fn test_tokenize_operand_expressions() {
        let mut state = make_state_with_internal_labels(
            0x0000,
            0x2000,
            &[
                (0x1234, "label", LabelType::UserDefined),
                (0x00FB, "zp_ptr", LabelType::ZeroPagePointer),
            ],
        );
        // Also add an external label at $D020
        state.labels.insert(
            Addr(0xD020),
            vec![Label {
                name: "ext_border".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::Field,
            }],
        );
        let map = build_label_lookup_map(&state, Assembler::Tass64);

        // 1. #<label
        assert_eq!(
            tokenize_operand("#<label", None, Assembler::Tass64, &state, &map),
            vec![
                OperandToken::Text("#<".to_string()),
                OperandToken::InternalLink {
                    text: "label".to_string(),
                    target: Addr(0x1234),
                },
            ]
        );

        // 2. #>label
        assert_eq!(
            tokenize_operand("#>label", None, Assembler::Tass64, &state, &map),
            vec![
                OperandToken::Text("#>".to_string()),
                OperandToken::InternalLink {
                    text: "label".to_string(),
                    target: Addr(0x1234),
                },
            ]
        );

        // 3. label+1
        assert_eq!(
            tokenize_operand("label+1", None, Assembler::Tass64, &state, &map),
            vec![
                OperandToken::InternalLink {
                    text: "label".to_string(),
                    target: Addr(0x1234),
                },
                OperandToken::Text("+1".to_string()),
            ]
        );

        // 4. Direct target_address shortcut (`label,x` with target_address: Some($1234))
        assert_eq!(
            tokenize_operand(
                "label,x",
                Some(Addr(0x1234)),
                Assembler::Tass64,
                &state,
                &map
            ),
            vec![OperandToken::InternalLink {
                text: "label,x".to_string(),
                target: Addr(0x1234),
            }]
        );

        // 5. (zp_ptr),y
        assert_eq!(
            tokenize_operand("(zp_ptr),y", None, Assembler::Tass64, &state, &map),
            vec![
                OperandToken::Text("(".to_string()),
                OperandToken::InternalLink {
                    text: "zp_ptr".to_string(),
                    target: Addr(0x00FB),
                },
                OperandToken::Text("),y".to_string()),
            ]
        );

        // 6. Expression combining internal and external labels (`#<label + ext_border`)
        assert_eq!(
            tokenize_operand(
                "#<label + ext_border",
                None,
                Assembler::Tass64,
                &state,
                &map
            ),
            vec![
                OperandToken::Text("#<".to_string()),
                OperandToken::InternalLink {
                    text: "label".to_string(),
                    target: Addr(0x1234),
                },
                OperandToken::Text(" + ext_border".to_string()),
            ]
        );
    }

    #[test]
    fn test_tokenize_operand_does_not_link_registers_or_ca65_prefix() {
        let mut state = make_state_with_internal_labels(
            0x0000,
            0x2000,
            &[
                (0x1000, "a", LabelType::UserDefined),
                (0x1001, "x", LabelType::UserDefined),
                (0x1002, "y", LabelType::UserDefined),
                (0x00FB, "zp_ptr", LabelType::ZeroPagePointer),
            ],
        );
        // ASL A ($0A), LDA a:$0012,X ($BD $12 $00), LDA ($FB),Y ($B1 $FB), JMP a ($4C $00 $10)
        state.raw_data[0x1010] = 0x0A;
        state.raw_data[0x1011] = 0xBD;
        state.raw_data[0x1012] = 0x12;
        state.raw_data[0x1013] = 0x00;
        state.raw_data[0x1014] = 0xB1;
        state.raw_data[0x1015] = 0xFB;
        state.raw_data[0x1016] = 0x4C;
        state.raw_data[0x1017] = 0x00;
        state.raw_data[0x1018] = 0x10;

        let map_ca65 = build_label_lookup_map(&state, Assembler::Ca65);

        // 1. Index register suffixes `,x`, `,y`, and `,x)` must not link to labels `x` or `y`
        assert_eq!(
            tokenize_operand("(zp_ptr),y", None, Assembler::Ca65, &state, &map_ca65),
            vec![
                OperandToken::Text("(".to_string()),
                OperandToken::InternalLink {
                    text: "zp_ptr".to_string(),
                    target: Addr(0x00FB),
                },
                OperandToken::Text("),y".to_string()),
            ]
        );
        assert_eq!(
            tokenize_operand("(zp_ptr,x)", None, Assembler::Ca65, &state, &map_ca65),
            vec![
                OperandToken::Text("(".to_string()),
                OperandToken::InternalLink {
                    text: "zp_ptr".to_string(),
                    target: Addr(0x00FB),
                },
                OperandToken::Text(",x)".to_string()),
            ]
        );

        // 2. ca65 `a:` force-absolute prefix must not link to label `a`
        assert_eq!(
            tokenize_operand("a:$0012,x", None, Assembler::Ca65, &state, &map_ca65),
            vec![OperandToken::Text("a:$0012,x".to_string())]
        );
        assert_eq!(
            tokenize_operand("a:zp_ptr,x", None, Assembler::Ca65, &state, &map_ca65),
            vec![
                OperandToken::Text("a:".to_string()),
                OperandToken::InternalLink {
                    text: "zp_ptr".to_string(),
                    target: Addr(0x00FB),
                },
                OperandToken::Text(",x".to_string()),
            ]
        );

        // 3. Accumulator instruction `asl a` must keep `"a"` as plain text, while `jmp a` links
        let options = ExportOptions::for_assembler(&state, Path::new("reg.asm"), Assembler::Ca65);
        let doc = build_export_document(&state, &options);
        let asl_row = doc
            .rows
            .iter()
            .find_map(|r| match r {
                ExportRow::Instruction(i) if i.address == Addr(0x1010) => Some(i),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            asl_row.operand_tokens,
            vec![OperandToken::Text("a".to_string())]
        );

        let jmp_row = doc
            .rows
            .iter()
            .find_map(|r| match r {
                ExportRow::Instruction(i) if i.address == Addr(0x1016) => Some(i),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            jmp_row.operand_tokens,
            vec![OperandToken::InternalLink {
                text: "a".to_string(),
                target: Addr(0x1000),
            }]
        );
    }

    #[test]
    fn test_tokenize_operand_local_labels_all_assemblers() {
        let state = make_state_with_internal_labels(
            0x1000,
            0x20,
            &[(0x1008, "loop", LabelType::LocalUserDefined)],
        );

        // 1. 64tass (`_loop`)
        let map_tass = build_label_lookup_map(&state, Assembler::Tass64);
        assert!(map_tass.contains_key("loop"));
        assert!(map_tass.contains_key("_loop"));
        assert_eq!(
            tokenize_operand("_loop", None, Assembler::Tass64, &state, &map_tass),
            vec![OperandToken::InternalLink {
                text: "_loop".to_string(),
                target: Addr(0x1008),
            }]
        );
        assert_eq!(
            tokenize_operand("#<_loop", None, Assembler::Tass64, &state, &map_tass),
            vec![
                OperandToken::Text("#<".to_string()),
                OperandToken::InternalLink {
                    text: "_loop".to_string(),
                    target: Addr(0x1008),
                },
            ]
        );

        // 2. ACME (`.loop`)
        let map_acme = build_label_lookup_map(&state, Assembler::Acme);
        assert!(map_acme.contains_key(".loop"));
        assert_eq!(
            tokenize_operand(".loop", None, Assembler::Acme, &state, &map_acme),
            vec![OperandToken::InternalLink {
                text: ".loop".to_string(),
                target: Addr(0x1008),
            }]
        );
        assert_eq!(
            tokenize_operand("#<.loop", None, Assembler::Acme, &state, &map_acme),
            vec![
                OperandToken::Text("#<".to_string()),
                OperandToken::InternalLink {
                    text: ".loop".to_string(),
                    target: Addr(0x1008),
                },
            ]
        );

        // 3. ca65 (`@loop`)
        let map_ca65 = build_label_lookup_map(&state, Assembler::Ca65);
        assert!(map_ca65.contains_key("@loop"));
        assert_eq!(
            tokenize_operand("@loop", None, Assembler::Ca65, &state, &map_ca65),
            vec![OperandToken::InternalLink {
                text: "@loop".to_string(),
                target: Addr(0x1008),
            }]
        );
        assert_eq!(
            tokenize_operand("#<@loop", None, Assembler::Ca65, &state, &map_ca65),
            vec![
                OperandToken::Text("#<".to_string()),
                OperandToken::InternalLink {
                    text: "@loop".to_string(),
                    target: Addr(0x1008),
                },
            ]
        );

        // 4. KickAssembler (`loop`) + relative branch target_address
        let map_kick = build_label_lookup_map(&state, Assembler::Kick);
        assert_eq!(
            tokenize_operand("loop", None, Assembler::Kick, &state, &map_kick),
            vec![OperandToken::InternalLink {
                text: "loop".to_string(),
                target: Addr(0x1008),
            }]
        );
        assert_eq!(
            tokenize_operand("#<loop", None, Assembler::Kick, &state, &map_kick),
            vec![
                OperandToken::Text("#<".to_string()),
                OperandToken::InternalLink {
                    text: "loop".to_string(),
                    target: Addr(0x1008),
                },
            ]
        );
        assert_eq!(
            tokenize_operand(
                "*+$08",
                Some(Addr(0x1008)),
                Assembler::Tass64,
                &state,
                &map_tass
            ),
            vec![OperandToken::InternalLink {
                text: "*+$08".to_string(),
                target: Addr(0x1008),
            }]
        );
    }

    #[test]
    fn test_tokenize_operand_external_labels_remain_text() {
        let mut state = make_state_with_internal_labels(0x1000, 0x10, &[]);
        state.labels.insert(
            Addr(0xD020),
            vec![Label {
                name: "border_color".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::Field,
            }],
        );
        state.labels.insert(
            Addr(0x0002),
            vec![Label {
                name: "zp_ext".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::ZeroPageField,
            }],
        );
        let map = build_label_lookup_map(&state, Assembler::Tass64);

        assert_eq!(
            tokenize_operand(
                "border_color",
                Some(Addr(0xD020)),
                Assembler::Tass64,
                &state,
                &map
            ),
            vec![OperandToken::Text("border_color".to_string())]
        );
        assert_eq!(
            tokenize_operand("#<zp_ext", None, Assembler::Tass64, &state, &map),
            vec![OperandToken::Text("#<zp_ext".to_string())]
        );
    }

    #[test]
    fn test_export_comment_decomposition_variants() {
        let mut state = make_state_with_internal_labels(0x1000, 0x1000, &[]);
        state.settings.max_xref_count = 2;

        // Address $1000: internal ($1500), external ($C000), and truncated 3rd ($1800)
        state.cross_refs.insert(
            Addr(0x1000),
            vec![
                crate::state::XRef::new(Addr(0xC000), crate::state::XRefKind::Read),
                crate::state::XRef::new(Addr(0x1500), crate::state::XRefKind::Read),
                crate::state::XRef::new(Addr(0x1800), crate::state::XRefKind::Read),
                crate::state::XRef::new(Addr(0x1500), crate::state::XRefKind::Read),
            ],
        );

        // 1. Prefix only (no xrefs at $1002)
        let c_prefix = ExportComment::from_line_comment(
            "User note <A & B>",
            Addr(0x1002),
            ";",
            &state,
            &state.settings,
        )
        .unwrap();
        assert_eq!(c_prefix.prefix_text.as_deref(), Some("User note <A & B>"));
        assert!(c_prefix.xrefs.is_empty());
        assert!(!c_prefix.xrefs_truncated);
        assert_eq!(c_prefix.suffix_text, None);

        // 2. Xrefs only with truncation (at $1000)
        // Sorted unique refs: $1500 (internal), $1800 (internal), $C000 (external) -> top 2 + ", ..."
        let c_xrefs = ExportComment::from_line_comment(
            "x-ref: $1500, $1800, ...",
            Addr(0x1000),
            ";",
            &state,
            &state.settings,
        )
        .unwrap();
        assert_eq!(c_xrefs.prefix_text, None);
        assert_eq!(
            c_xrefs.xrefs,
            vec![
                XrefTarget {
                    address: Addr(0x1500),
                    is_external: false,
                },
                XrefTarget {
                    address: Addr(0x1800),
                    is_external: false,
                },
            ]
        );
        assert!(c_xrefs.xrefs_truncated);
        assert_eq!(c_xrefs.suffix_text, None);

        // 3. Prefix + Xrefs + Suffix (both ";" and "//" comment prefixes)
        let c_full = ExportComment::from_line_comment(
            "Init border ; x-ref: $1500, $1800, ...; Border color",
            Addr(0x1000),
            ";",
            &state,
            &state.settings,
        )
        .unwrap();
        assert_eq!(c_full.prefix_text.as_deref(), Some("Init border"));
        assert!(c_full.xrefs_truncated);
        assert_eq!(c_full.suffix_text.as_deref(), Some("Border color"));

        let c_kick = ExportComment::from_line_comment(
            "Init border // x-ref: $1500, $1800, ...; Invalid or partial instruction",
            Addr(0x1000),
            "//",
            &state,
            &state.settings,
        )
        .unwrap();
        assert_eq!(c_kick.prefix_text.as_deref(), Some("Init border"));
        assert_eq!(
            c_kick.suffix_text.as_deref(),
            Some("Invalid or partial instruction")
        );

        // 4. Empty line comment returns None even when address has xrefs in state.cross_refs
        assert_eq!(
            ExportComment::from_line_comment("", Addr(0x1000), ";", &state, &state.settings),
            None
        );
    }

    #[test]
    fn test_export_comment_format_plain_vs_format_html() {
        let mut state = make_state_with_internal_labels(0x1000, 0x1000, &[]);
        state.settings.max_xref_count = 2;
        // One internal ($1500), one external ($C000), plus one truncated ($D000)
        state.cross_refs.insert(
            Addr(0x1000),
            vec![
                crate::state::XRef::new(Addr(0x1500), crate::state::XRefKind::Read),
                crate::state::XRef::new(Addr(0xC000), crate::state::XRefKind::Read),
                crate::state::XRef::new(Addr(0xD000), crate::state::XRefKind::Read),
            ],
        );

        for (asm, cp) in [
            (Assembler::Tass64, ";"),
            (Assembler::Acme, ";"),
            (Assembler::Ca65, ";"),
            (Assembler::Kick, "//"),
        ] {
            let _ = asm;
            let raw = format!("Check <A & B> {cp} x-ref: $1500, $c000, ...; Target > $00");
            let comment =
                ExportComment::from_line_comment(&raw, Addr(0x1000), cp, &state, &state.settings)
                    .unwrap();

            // format_plain reconstructs `raw` byte-for-byte
            assert_eq!(comment.format_plain(cp), raw);

            // format_html escapes `&`, `<`, `>` in prefix and suffix, and links internal xrefs
            // with uppercase `#L{:04X}` href and lowercase `${:04x}` display text.
            let expected_html = format!(
                "Check &lt;A &amp; B&gt; {cp} x-ref: <a href=\"#L1500\">$1500</a>, $c000, ...; Target &gt; $00"
            );
            assert_eq!(comment.format_html(cp), expected_html);
        }
    }

    #[test]
    fn test_export_comment_suppressed_on_empty_line_comment_even_when_address_has_xrefs() {
        let mut state = make_state_with_internal_labels(0x1000, 2, &[]);
        // 2-byte PETSCII text block at $1000 ("HI") in 64tass emits `.encode`, `.enc "none"`,
        // `.text "HI"`, and `.endencode` at address $1000. The wrapper lines have empty
        // `line.comment` fields, and only `.text "HI"` carries the comment.
        state.raw_data = vec![0x48, 0x49];
        state.block_types = vec![BlockType::PetsciiText; 2];
        state.cross_refs.insert(
            Addr(0x1000),
            vec![crate::state::XRef::new(
                Addr(0x1001),
                crate::state::XRefKind::Read,
            )],
        );
        state.annotations.update(Addr(0x1000), |e| {
            e.user_side_comment = Some("Greeting".to_string());
        });

        let options =
            ExportOptions::for_assembler(&state, Path::new("test.asm"), Assembler::Tass64);
        let doc = build_export_document(&state, &options);

        let inst_rows: Vec<&InstructionRow> = doc
            .rows
            .iter()
            .filter_map(|r| match r {
                ExportRow::Instruction(inst) => Some(inst),
                _ => None,
            })
            .collect();

        // Expect `.encode`, `.enc`, `.text`, `.endencode`
        assert!(inst_rows.len() >= 3);
        let encode_row = inst_rows.iter().find(|r| r.mnemonic == ".encode").unwrap();
        assert_eq!(
            encode_row.side_comment, None,
            "Wrapper line .encode must have side_comment == None"
        );
        let endencode_row = inst_rows
            .iter()
            .find(|r| r.mnemonic == ".endencode")
            .unwrap();
        assert_eq!(
            endencode_row.side_comment, None,
            "Wrapper line .endencode must have side_comment == None"
        );
        let text_row = inst_rows.iter().find(|r| r.mnemonic == ".text").unwrap();
        assert!(
            text_row.side_comment.is_some(),
            "Primary .text row must retain its side comment"
        );
    }

    #[test]
    fn test_build_export_document_separate_lowering_of_directives_equates_enums() {
        let mut state = AppState::new();
        // When origin is $0000, lower preamble directives, equates, and enums into
        // their distinct ExportRow variants before ExportRow::Origin and body instructions.
        state.origin = Addr::ZERO;
        state.raw_data = vec![0xA9, 0x00, 0x60]; // LDA #$00, RTS
        state.block_types = vec![BlockType::Code; 3];

        // External label at $D020
        state.labels.insert(
            Addr(0xD020),
            vec![Label {
                name: "border".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::Field,
            }],
        );

        // Enum definition used at $0000
        let mut variants = BTreeMap::new();
        variants.insert(0, "BLACK".to_string());
        variants.insert(1, "WHITE".to_string());
        state.enums.insert(
            "Colors".to_string(),
            EnumDefinition {
                name: "Colors".to_string(),
                variants,
                description: None,
                source_file: None,
            },
        );
        state.annotations.update(Addr::ZERO, |e| {
            e.enum_usage = Some("Colors".to_string());
        });

        let options = ExportOptions::for_assembler(&state, Path::new("test.asm"), Assembler::Ca65);
        let doc = build_export_document(&state, &options);

        let pos_directive = doc
            .rows
            .iter()
            .position(|r| matches!(r, ExportRow::FileDirective { directive, .. } if directive == ".macpack cbm"))
            .expect("Expected .macpack cbm FileDirective");
        let pos_equate = doc
            .rows
            .iter()
            .position(
                |r| matches!(r, ExportRow::Equate { address, .. } if *address == Addr(0xD020)),
            )
            .expect("Expected Equate for $D020");
        let pos_enum = doc
            .rows
            .iter()
            .position(
                |r| matches!(r, ExportRow::EnumLine { text } if text.contains(".enum Colors")),
            )
            .expect("Expected EnumLine for .enum Colors");
        let pos_origin = doc
            .rows
            .iter()
            .position(|r| matches!(r, ExportRow::Origin { address, .. } if *address == Addr::ZERO))
            .expect("Expected Origin row");
        let pos_inst = doc
            .rows
            .iter()
            .position(|r| matches!(r, ExportRow::Instruction(_)))
            .expect("Expected Instruction row");

        assert!(pos_directive < pos_equate);
        assert!(pos_equate < pos_enum);
        assert!(pos_enum < pos_origin);
        assert!(pos_origin < pos_inst);
    }

    #[test]
    fn test_build_export_document_origin_before_leading_external_file() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.raw_data = vec![0x11, 0x22, 0x60];
        state.block_types = vec![
            BlockType::ExternalFile,
            BlockType::ExternalFile,
            BlockType::Code,
        ];

        let options =
            ExportOptions::for_assembler(&state, Path::new("demo.asm"), Assembler::Tass64);
        let doc = build_export_document(&state, &options);

        let pos_origin = doc
            .rows
            .iter()
            .position(|r| matches!(r, ExportRow::Origin { address, directive } if *address == Addr(0x1000) && directive == "* = $1000"))
            .expect("Expected Origin row");
        let pos_ext = doc
            .rows
            .iter()
            .position(|r| {
                matches!(
                    r,
                    ExportRow::ExternalFile {
                        start_address: Addr(0x1000),
                        end_address: Addr(0x1001),
                        bin_filename,
                        raw_bytes,
                        ..
                    } if bin_filename == "demo_1000_1001.bin" && raw_bytes == &vec![0x11, 0x22]
                )
            })
            .expect("Expected ExternalFile row");

        assert!(
            pos_origin < pos_ext,
            "ExportRow::Origin must precede leading ExportRow::ExternalFile"
        );

        // Verify empty body fallback emits Origin at the end
        let mut empty_state = AppState::new();
        empty_state.origin = Addr(0x2000);
        let empty_doc = build_export_document(
            &empty_state,
            &ExportOptions::from_state_and_path(&empty_state, Path::new("empty.asm")),
        );
        assert!(matches!(
            empty_doc.rows.last(),
            Some(ExportRow::Origin {
                address: Addr(0x2000),
                ..
            })
        ));
    }

    #[test]
    fn test_build_export_document_label_overflow_splits_at_20_chars() {
        // 19 chars (< LABEL_COLUMN_WIDTH = 20) -> Inline
        // 20 chars (== LABEL_COLUMN_WIDTH = 20) -> OwnLine
        let label_19 = "label_nineteen_ch19";
        let label_20 = "label_twenty_chars20";
        assert_eq!(label_19.len(), 19);
        assert_eq!(label_20.len(), 20);

        let state = make_state_with_internal_labels(
            0x1000,
            2,
            &[
                (0x1000, label_19, LabelType::UserDefined),
                (0x1001, label_20, LabelType::UserDefined),
            ],
        );
        let options =
            ExportOptions::for_assembler(&state, Path::new("test.asm"), Assembler::Tass64);
        let doc = build_export_document(&state, &options);

        let insts: Vec<&InstructionRow> = doc
            .rows
            .iter()
            .filter_map(|r| match r {
                ExportRow::Instruction(i) => Some(i),
                _ => None,
            })
            .collect();
        assert_eq!(insts.len(), 2);
        assert_eq!(insts[0].label.as_deref(), Some(label_19));
        assert_eq!(insts[0].label_placement, LabelPlacement::Inline);
        assert_eq!(insts[1].label.as_deref(), Some(label_20));
        assert_eq!(insts[1].label_placement, LabelPlacement::OwnLine);
    }

    #[test]
    fn test_export_bundle_write_to_disk_and_error_handling() {
        static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let temp_dir = std::env::temp_dir().join(format!(
            "regen2000_ir_bundle_test_{}_{id}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let primary_path = temp_dir.join("main.asm");
        let sidecar = ExportSidecar::new(&primary_path, "main_1000_1001.bin", vec![0xAA, 0x55]);
        let bundle = ExportBundle {
            primary_path: primary_path.clone(),
            primary_content: "* = $1000\n.binary \"main_1000_1001.bin\"\n".to_string(),
            sidecars: vec![sidecar.clone()],
        };

        bundle.write_to_disk().unwrap();
        assert_eq!(
            std::fs::read_to_string(&primary_path).unwrap(),
            bundle.primary_content
        );
        assert_eq!(std::fs::read(&sidecar.path).unwrap(), vec![0xAA, 0x55]);

        let _ = std::fs::remove_dir_all(&temp_dir);

        // Verify error handling on non-existent parent directory
        let bad_primary = temp_dir.join("nonexistent_subdir").join("should_fail.asm");
        let bad_bundle = ExportBundle {
            primary_path: bad_primary,
            primary_content: "nop\n".to_string(),
            sidecars: Vec::new(),
        };
        let err = bad_bundle.write_to_disk().unwrap_err();
        assert!(matches!(err, CoreError::Export(ExportError::Io { .. })));
    }

    #[test]
    fn test_tokenize_operand_symbolic_offsets_link_base_identifier_only() {
        use crate::state::{OffsetRadix, SymbolicOffset};

        let mut state = make_state_with_internal_labels(
            0x1000,
            0x40,
            &[
                (0x1010, "sub_C100", LabelType::Subroutine),
                (0x1020, "sub_C200", LabelType::Subroutine),
            ],
        );
        state.labels.insert(
            Addr(0x00FB),
            vec![Label {
                name: "zp_ptr".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::ZeroPagePointer,
            }],
        );
        let map_tass = build_label_lookup_map(&state, Assembler::Tass64);
        let map_ca65 = build_label_lookup_map(&state, Assembler::Ca65);

        // 1. Control-flow jump with `target_address = Some($1013)` bypasses fast path and links only `sub_C100`
        assert_eq!(
            tokenize_operand(
                "sub_C100 + 3",
                Some(Addr(0x1013)),
                Assembler::Tass64,
                &state,
                &map_tass
            ),
            vec![
                OperandToken::InternalLink {
                    text: "sub_C100".to_string(),
                    target: Addr(0x1010),
                },
                OperandToken::Text(" + 3".to_string()),
            ]
        );

        // 2. Parenthesized byte operators `#<(sub_C100 - 1)` and `<(sub_C100 - 1)`
        assert_eq!(
            tokenize_operand(
                "#<(sub_C100 - 1)",
                None,
                Assembler::Tass64,
                &state,
                &map_tass
            ),
            vec![
                OperandToken::Text("#<(".to_string()),
                OperandToken::InternalLink {
                    text: "sub_C100".to_string(),
                    target: Addr(0x1010),
                },
                OperandToken::Text(" - 1)".to_string()),
            ]
        );
        assert_eq!(
            tokenize_operand(
                "<(sub_C100 - 1)",
                None,
                Assembler::Tass64,
                &state,
                &map_tass
            ),
            vec![
                OperandToken::Text("<(".to_string()),
                OperandToken::InternalLink {
                    text: "sub_C100".to_string(),
                    target: Addr(0x1010),
                },
                OperandToken::Text(" - 1)".to_string()),
            ]
        );

        // 3. Indexed & force-absolute (`sub_C100 - 1,x`, `a:sub_C100 - 1,x`, `@w sub_C100 - 1,x`)
        assert_eq!(
            tokenize_operand("sub_C100 - 1,x", None, Assembler::Tass64, &state, &map_tass),
            vec![
                OperandToken::InternalLink {
                    text: "sub_C100".to_string(),
                    target: Addr(0x1010),
                },
                OperandToken::Text(" - 1,x".to_string()),
            ]
        );
        assert_eq!(
            tokenize_operand("a:sub_C100 - 1,x", None, Assembler::Ca65, &state, &map_ca65),
            vec![
                OperandToken::Text("a:".to_string()),
                OperandToken::InternalLink {
                    text: "sub_C100".to_string(),
                    target: Addr(0x1010),
                },
                OperandToken::Text(" - 1,x".to_string()),
            ]
        );
        assert_eq!(
            tokenize_operand(
                "@w sub_C100 - 1,x",
                None,
                Assembler::Tass64,
                &state,
                &map_tass
            ),
            vec![
                OperandToken::Text("@w ".to_string()),
                OperandToken::InternalLink {
                    text: "sub_C100".to_string(),
                    target: Addr(0x1010),
                },
                OperandToken::Text(" - 1,x".to_string()),
            ]
        );

        // 4. Multi-word `.word sub_C100 - 1, sub_C200 - 1` (`target_address == None`)
        assert_eq!(
            tokenize_operand(
                "sub_C100 - 1, sub_C200 - 1",
                None,
                Assembler::Tass64,
                &state,
                &map_tass
            ),
            vec![
                OperandToken::InternalLink {
                    text: "sub_C100".to_string(),
                    target: Addr(0x1010),
                },
                OperandToken::Text(" - 1, ".to_string()),
                OperandToken::InternalLink {
                    text: "sub_C200".to_string(),
                    target: Addr(0x1020),
                },
                OperandToken::Text(" - 1".to_string()),
            ]
        );

        // 5. External base `(zp_ptr + 1),y` does not emit internal HTML anchor
        assert_eq!(
            tokenize_operand("(zp_ptr + 1),y", None, Assembler::Tass64, &state, &map_tass),
            vec![OperandToken::Text("(zp_ptr + 1),y".to_string())]
        );

        // 6. Full `build_export_document` across all assemblers with `JMP sub_C100 + 3`
        // $1000: 4C 13 10 (JMP $1013) with Tier 1 `sub_C100 + 3`
        state.raw_data[0..3].copy_from_slice(&[0x4C, 0x13, 0x10]);
        state.annotations.update(Addr(0x1000), |e| {
            e.operand_offset = Some(SymbolicOffset {
                base: Addr(0x1010),
                offset: 3,
                radix: OffsetRadix::Decimal,
            });
        });
        for &asm in Assembler::all() {
            let options = ExportOptions::for_assembler(&state, Path::new("test.html"), asm);
            let doc = build_export_document(&state, &options);
            let jmp_row = doc
                .rows
                .iter()
                .find_map(|r| match r {
                    ExportRow::Instruction(ir) if ir.address == Addr(0x1000) => Some(ir),
                    _ => None,
                })
                .expect("JMP row at $1000 must exist");
            assert_eq!(
                jmp_row.operand_tokens,
                vec![
                    OperandToken::InternalLink {
                        text: "sub_C100".to_string(),
                        target: Addr(0x1010),
                    },
                    OperandToken::Text(" + 3".to_string()),
                ],
                "failed for assembler {asm:?}"
            );
        }
    }
}
