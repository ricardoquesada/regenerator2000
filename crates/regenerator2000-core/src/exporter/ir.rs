//! Shared Intermediate Representation ([`ExportDocument`]) and semantic lowering pass
//! ([`build_export_document`]) for the ASM and HTML exporter backends.
//!
//! This module implements **Stage 1** of the exporter pipeline. Rather than having each backend
//! independently invoke the disassembler and reverse-engineer semantic row types by inspecting
//! string prefixes on [`DisassemblyLine::mnemonic`], [`build_export_document`] lowers
//! [`AppState`] and [`ExportOptions`] into a strongly-typed sequence of [`ExportRow`] items
//! in memory.

pub use crate::disassembler::{CODE_COLUMN_WIDTH, LABEL_COLUMN_WIDTH, LabelPlacement};
use crate::disassembler::{Disassembler, DisassemblyLine, format_cross_references};
use crate::error::{CoreError, ExportError};
use crate::state::{Addr, AppState, Assembler, BlockType, DocumentSettings, LabelType};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Escapes `&`, `<`, and `>` for safe interpolation into HTML text nodes and attributes.
#[must_use]
pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Configuration options controlling export lowering and target assembler formatting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportOptions {
    /// Target cross-assembler dialect.
    pub assembler: Assembler,
    /// File stem of the destination export path (e.g. `"program"` for `"program.asm"`).
    pub base_name: String,
    /// Optional source binary filename (e.g. `"game.prg"`), used in HTML header banners.
    pub source_name: Option<String>,
}

impl ExportOptions {
    /// Constructs [`ExportOptions`] using the assembler configured in `state.settings`
    /// and the file stem of `path`.
    #[must_use]
    pub fn from_state_and_path(state: &AppState, path: &Path) -> Self {
        Self::for_assembler(state, path, state.settings.assembler)
    }

    /// Constructs [`ExportOptions`] overriding the target `assembler` without cloning [`AppState`].
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

/// A secondary output artifact (such as an external `.bin` binary slice or sub-page `.html` file)
/// produced alongside the primary export file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportSidecar {
    /// Relative filename of the sidecar artifact (e.g. `"export_1000_10ff.bin"` or `"export_1000.html"`).
    pub filename: String,
    /// Full destination path on disk where the sidecar should be written.
    pub path: PathBuf,
    /// Raw payload bytes of the sidecar file.
    pub bytes: Vec<u8>,
}

impl ExportSidecar {
    /// Creates a new [`ExportSidecar`] located in the same directory as `primary_path`.
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

    /// Returns the sidecar payload as a UTF-8 string slice if valid.
    #[must_use]
    pub fn content_utf8(&self) -> Option<&str> {
        std::str::from_utf8(&self.bytes).ok()
    }
}

/// Complete in-memory result of rendering an [`ExportDocument`], containing the primary
/// output file content and any associated [`ExportSidecar`] files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportBundle {
    /// Destination path for the primary output file (`.asm` or `.html`).
    pub primary_path: PathBuf,
    /// Rendered UTF-8 text content of the primary output file.
    pub primary_content: String,
    /// Additional sidecar files (`.bin` slices for ASM or `_{addr:04x}.html` pages for HTML).
    pub sidecars: Vec<ExportSidecar>,
}

impl ExportBundle {
    /// Writes all sidecar artifacts and the primary export file to disk.
    ///
    /// Sidecars are written first so that any external binary slices (`.bin`) or linked HTML
    /// sub-pages already exist on disk when the primary file is finalized.
    ///
    /// # Errors
    /// Returns [`CoreError::Export`] wrapping [`ExportError::Io`] if any sidecar or primary file
    /// cannot be written.
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

/// Structured top-of-file metadata banner for an [`ExportDocument`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportHeader {
    /// Comment prefix for the active assembler (`";"` or `"//"`).
    pub comment_prefix: String,
    /// Version attribution line (e.g. `"; Auto-generated by Regenerator 2000 v..."`).
    pub version_line: String,
    /// Project homepage URL.
    pub url: String,
    /// Source binary filename (`"unknown"` when not backed by a file).
    pub source_name: String,
    /// Human-readable name of the target assembler (e.g. `"64tass"`).
    pub assembler_name: String,
    /// Official homepage URL of the target assembler.
    pub assembler_url: String,
    /// Commented build-instruction lines returned by
    /// [`Formatter::format_file_header`](crate::disassembler::formatter::Formatter::format_file_header).
    pub build_command_lines: Vec<String>,
}

/// A single cross-reference target address in a side comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XrefTarget {
    /// Referenced memory address.
    pub address: Addr,
    /// `true` if `address` is outside the loaded binary's `[origin, origin + raw_data.len())` range.
    pub is_external: bool,
}

/// Structured representation of a trailing side comment on a directive, equate, mid-label, or instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportComment {
    /// Optional primary comment text preceding `x-ref:` (e.g. user or system side comment).
    pub prefix_text: Option<String>,
    /// Cross-reference addresses (sorted, deduplicated, truncated to `max_xref_count`).
    pub xrefs: Vec<XrefTarget>,
    /// Whether additional cross-references were truncated (`", ..."`).
    pub xrefs_truncated: bool,
    /// Optional trailing note appended after `x-ref:` (e.g. target address comment `"Border color"`
    /// or `"Invalid or partial instruction"`).
    pub suffix_text: Option<String>,
}

impl ExportComment {
    /// Decomposes a non-empty [`DisassemblyLine::comment`] into its structured [`ExportComment`]
    /// components (`prefix_text`, `xrefs`, `xrefs_truncated`, and `suffix_text`).
    ///
    /// Returns `None` if `line_comment` is empty, preserving intentional comment suppression
    /// on wrapper lines (`.encode`, `.endencode`) and multi-line data continuation rows.
    #[must_use]
    pub fn from_line_comment(
        line_comment: &str,
        address: Addr,
        comment_prefix: &str,
        state: &AppState,
        settings: &DocumentSettings,
    ) -> Option<Self> {
        // Multi-line data blocks (e.g. `.encode` / `.text` / `.endencode` in 64tass or multi-chunk
        // `.byte` rows) intentionally set `line.comment = String::new()` on wrapper and continuation
        // lines even when `address` has cross-references or user annotations. Respecting an empty
        // `line_comment` avoids duplicating side comments on every line of a multi-line directive.
        if line_comment.is_empty() {
            return None;
        }

        let refs_opt = if settings.max_xref_count > 0 {
            state.cross_refs.get(&address).filter(|r| !r.is_empty())
        } else {
            None
        };

        // The disassembler pipeline constructs `line.comment` by joining the user/system side
        // comment and `format_cross_references(refs, max_xref_count)` with `" {comment_prefix} "`,
        // and then optionally appending `"; {target_comment}"` or `"; Invalid or partial instruction"`.
        // Splitting once on the exact `xref_str` losslessly recovers both the leading `prefix_text`
        // and any disassembler-appended `suffix_text`.
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

                let mut all_refs = refs.clone();
                all_refs.sort_unstable();
                all_refs.dedup();
                let xrefs_truncated = all_refs.len() > settings.max_xref_count;
                let xrefs = all_refs
                    .into_iter()
                    .take(settings.max_xref_count)
                    .map(|r| XrefTarget {
                        address: r,
                        is_external: state.is_external(r),
                    })
                    .collect();

                return Some(Self {
                    prefix_text,
                    xrefs,
                    xrefs_truncated,
                    suffix_text,
                });
            }
        }

        // No cross-reference segment is present in `line_comment`.
        // Check if `line_comment` consists of a known annotation base comment at `address`
        // followed by a disassembler-appended `"; {suffix}"` note (e.g. target address comment).
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

    /// Constructs an [`ExportComment`] containing only cross-references for `address`
    /// (used for mid-instruction relative labels).
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

        let mut all_refs = refs.clone();
        all_refs.sort_unstable();
        all_refs.dedup();
        let xrefs_truncated = all_refs.len() > settings.max_xref_count;
        let xrefs = all_refs
            .into_iter()
            .take(settings.max_xref_count)
            .map(|r| XrefTarget {
                address: r,
                is_external: state.is_external(r),
            })
            .collect();

        Some(Self {
            prefix_text: None,
            xrefs,
            xrefs_truncated,
            suffix_text: None,
        })
    }

    /// Renders this comment as plain text for `.asm` output, reconstructing the exact
    /// comment body produced by [`DisassemblyLine::comment`].
    #[must_use]
    pub fn format_plain(&self, comment_prefix: &str) -> String {
        let mut base_parts = Vec::new();
        if let Some(prefix) = &self.prefix_text
            && !prefix.is_empty()
        {
            base_parts.push(prefix.clone());
        }
        if !self.xrefs.is_empty() {
            let refs_str: Vec<String> = self
                .xrefs
                .iter()
                .map(|x| format!("${:04x}", x.address.0))
                .collect();
            let trunc = if self.xrefs_truncated { ", ..." } else { "" };
            base_parts.push(format!("x-ref: {}{}", refs_str.join(", "), trunc));
        }

        let mut out = base_parts.join(&format!(" {comment_prefix} "));
        if let Some(suffix) = &self.suffix_text
            && !suffix.is_empty()
        {
            if !out.is_empty() {
                out.push_str("; ");
            }
            out.push_str(suffix);
        }
        out
    }

    /// Renders this comment as HTML with strict entity escaping on `prefix_text` and `suffix_text`
    /// and `<a href="#L{:04X}">${:04x}</a>` hyperlinks on internal cross-references.
    #[must_use]
    pub fn format_html(&self, comment_prefix: &str) -> String {
        let mut base_parts = Vec::new();
        if let Some(prefix) = &self.prefix_text
            && !prefix.is_empty()
        {
            base_parts.push(html_escape(prefix));
        }
        if !self.xrefs.is_empty() {
            // Anchor targets (`#L{:04X}`) use uppercase hex to match `<tr id="L{:04X}">` row IDs,
            // while visible address text (`${:04x}`) uses lowercase hex to match `format_plain`.
            let refs_str: Vec<String> = self
                .xrefs
                .iter()
                .map(|x| {
                    if x.is_external {
                        format!("${:04x}", x.address.0)
                    } else {
                        format!("<a href=\"#L{:04X}\">${:04x}</a>", x.address.0, x.address.0)
                    }
                })
                .collect();
            let trunc = if self.xrefs_truncated { ", ..." } else { "" };
            base_parts.push(format!("x-ref: {}{}", refs_str.join(", "), trunc));
        }

        let escaped_cp = html_escape(comment_prefix);
        let mut out = base_parts.join(&format!(" {escaped_cp} "));
        if let Some(suffix) = &self.suffix_text
            && !suffix.is_empty()
        {
            if !out.is_empty() {
                out.push_str("; ");
            }
            out.push_str(&html_escape(suffix));
        }
        out
    }
}

/// A mid-instruction relative label definition (e.g. `rel1 =*+$01` or `.label rel1 = * + 1`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MidLabel {
    /// Exact memory address inside the multi-byte instruction or data item.
    pub address: Addr,
    /// Formatter-specific relative label definition text.
    pub definition: String,
    /// Optional structured cross-reference side comment for `address`.
    pub side_comment: Option<ExportComment>,
}

/// A parsed segment of an instruction or data directive operand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperandToken {
    /// Plain unlinked text (punctuation, register suffixes, string literals, numeric literals,
    /// or external symbol names).
    Text(String),
    /// Reference to an internal label defined within the binary's address range.
    InternalLink {
        /// Visible symbol text in the operand.
        text: String,
        /// Target memory address of the internal symbol.
        target: Addr,
    },
}

/// Strongly-typed lowered representation of a body instruction, data directive, or scope delimiter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstructionRow {
    /// Memory address of the instruction or directive.
    pub address: Addr,
    /// Underlying raw machine bytes.
    pub bytes: Vec<u8>,
    /// Whether raw hex bytes should be displayed in the bytes column (HTML/listing views).
    pub show_bytes: bool,
    /// Full-line block comments preceding this instruction (split by line).
    pub line_comments: Vec<String>,
    /// Mid-instruction relative labels (`1..bytes.len()`) resolved via the active formatter.
    pub mid_labels: Vec<MidLabel>,
    /// Formatted label definition at `address`, if any.
    pub label: Option<String>,
    /// Layout placement for `label` (`None`, `Inline`, or `OwnLine`).
    pub label_placement: LabelPlacement,
    /// `true` when `mnemonic` is a KickAssembler scope brace (`"{"` or `"}"`).
    pub is_scope_brace: bool,
    /// Formatted instruction mnemonic or data/scope directive.
    pub mnemonic: String,
    /// Raw formatted operand string.
    pub operand_text: String,
    /// Tokenized operand segments for safe HTML linkification and escaping.
    pub operand_tokens: Vec<OperandToken>,
    /// Optional trailing side comment (`None` when [`DisassemblyLine::comment`] was empty).
    pub side_comment: Option<ExportComment>,
}

/// A semantic row in an [`ExportDocument`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportRow {
    /// Empty separator line.
    Blank,
    /// Pre-prefixed section or legend comment line from the preamble (e.g. `"; EXTERNAL LABELS"`).
    SectionComment {
        /// Full comment text including the leading comment prefix.
        text: String,
    },
    /// Top-level assembler setup directive emitted outside the header comment banner
    /// (e.g. `.macpack cbm` in `ca65`).
    FileDirective {
        /// Directive text.
        directive: String,
        /// Optional side comment explaining the directive.
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
    /// A single line of a formatted enum definition block.
    EnumLine {
        /// Raw line text from
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
    /// Coalesced [`BlockType::ExternalFile`] region exported as a binary sidecar in ASM
    /// and a linked sub-page table in HTML.
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
    /// Ordered sequence of semantic rows comprising the document.
    pub rows: Vec<ExportRow>,
}

/// Builds a lookup table mapping label names (including assembler-prefixed local label names
/// such as `_loop` in `64tass`, `.loop` in `ACME`, and `@loop` in `ca65`) to their [`Addr`].
#[must_use]
pub fn build_label_lookup_map(state: &AppState, assembler: Assembler) -> HashMap<String, Addr> {
    let formatter = Disassembler::formatter(assembler);
    let local_prefix = formatter.local_label_prefix();
    let mut label_name_to_addr = HashMap::new();
    for (addr, label_vec) in &state.labels {
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

/// Tokenizes an instruction or data operand into plain text and internal label link tokens.
///
/// - Respects quoted string and character literals (`"..."` and `'...'`), enabling backslash
///   escapes (`\"`, `\\`) only for assemblers that support them (`ACME` and `KickAssembler`)
///   and doubled-quote escapes (`""`) for `64tass`, so words inside `.text` or `!scr` strings
///   are never linkified.
/// - Consumes hexadecimal (`$xxxx`), binary (`%0101`), and decimal numeric literals directly
///   into [`OperandToken::Text`] so hex digits like `$a0`, `$ff`, or `$c000` are never falsely
///   matched against labels with the same name.
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

    // Fast path for relative branches and simple direct targets whose entire operand is a single
    // internal symbol (including relative branch expressions like `*+$02`).
    if let Some(target) = target_address
        && !state.is_external(target)
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
            // Consume the entire quoted string or character literal into `text_buf` so words
            // inside `.text "press start"` are never mistaken for label references.
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
                        // 64tass and ca65 escape embedded quotes by doubling them (`""`).
                        if let Some(q2) = chars.next() {
                            text_buf.push(q2);
                        }
                    } else {
                        break;
                    }
                }
            }
        } else if c == '$' || c == '%' || c.is_ascii_digit() {
            // Consume numeric literals (`$hex`, `%bin`, or decimal) together with all immediately
            // following alphanumeric characters so hex digits like `$a0`, `$ff`, or `$dead` are
            // never linkified when a label with that name exists in the project.
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
            // Leading `.` or `@` may be a local label prefix (`.loop` in ACME, `@loop` in ca65)
            // or a scoped field separator (`scope.field`). Try resolving the full prefixed token
            // first, then fall back to keeping the punctuation in `text_buf` and linking `ident`.
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
            // Standard identifier or `64tass` `_`-prefixed local label (`_loop`).
            let mut word = String::from(c);
            while let Some(&nc) = chars.peek() {
                if nc.is_alphanumeric() || nc == '_' {
                    word.push(nc);
                    chars.next();
                } else {
                    break;
                }
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

/// Lowers a single body [`DisassemblyLine`] into a structured [`InstructionRow`].
///
/// Delegates per-line semantic lowering—mid-instruction relative labels
/// ([`DisassemblyLine::mid_instruction_labels`]), local prefix deduplication
/// ([`MidInstructionLabel::format_definition`](crate::disassembler::MidInstructionLabel::format_definition)),
/// unindented scope detection ([`DisassemblyLine::is_unindented_scope`]), formatted label text
/// ([`DisassemblyLine::formatted_label`]), and [`LabelPlacement`]
/// ([`DisassemblyLine::label_placement`])—to [`crate::disassembler`] so batch exports and the
/// interactive TUI share identical column-wrapping and symbol-resolution rules.
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

    let operand_tokens = tokenize_operand(
        &line.operand,
        line.target_address,
        settings.assembler,
        state,
        label_name_to_addr,
    );

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

/// Lowers [`AppState`] and [`ExportOptions`] into a unified, target-agnostic [`ExportDocument`] IR.
///
/// This pass performs zero filesystem I/O and zero [`AppState`] cloning. It lowers:
///
/// 1. Top-of-file metadata into [`ExportHeader`].
/// 2. Formatter file directives ([`Formatter::format_file_directives`](crate::disassembler::formatter::Formatter::format_file_directives))
///    into [`ExportRow::FileDirective`].
/// 3. External and [`BlockType::ExternalFile`] label definitions into [`ExportRow::SectionComment`]
///    and [`ExportRow::Equate`].
/// 4. Enum definitions into [`ExportRow::SectionComment`] and [`ExportRow::EnumLine`].
/// 5. Program origin ([`ExportRow::Origin`]) guaranteed to precede any leading [`ExportRow::ExternalFile`]
///    or [`ExportRow::Instruction`].
/// 6. Body disassembly lines into [`ExportRow::ExternalFile`] and [`ExportRow::Instruction`].
#[must_use]
pub fn build_export_document(state: &AppState, options: &ExportOptions) -> ExportDocument {
    let formatter = Disassembler::formatter(options.assembler);
    let comment_prefix = formatter.comment_prefix().to_string();

    // Clone only the lightweight `DocumentSettings` struct so `DisassemblyContext` can use
    // `options.assembler` without cloning `AppState` (which holds `raw_data`, `labels`, `enums`, etc.).
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

    // Preamble sections (file directives, external equates, and enum definitions) are lowered in
    // separate, dedicated loops before the body disassembly rather than concatenated into a single
    // `Vec<DisassemblyLine>`. This eliminates string-prefix heuristics (`mnemonic.contains('=')`,
    // `address == Addr::ZERO`) and prevents preamble lines from colliding with body instructions at `$0000`.

    // 1. Top-level file directives (e.g. `.macpack cbm` in ca65)
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

    // 2. External & ExternalFile label equates
    let external_lines = state.get_external_label_definitions_with_formatter(true, formatter);
    for line in &external_lines {
        if line.mnemonic.is_empty() && line.bytes.is_empty() && line.comment.is_empty() {
            rows.push(ExportRow::Blank);
        } else if line.bytes.is_empty() && line.mnemonic.starts_with(&comment_prefix) {
            rows.push(ExportRow::SectionComment {
                text: line.mnemonic.clone(),
            });
        } else {
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

    // 3. Enum definitions
    let enum_lines = state.get_enum_definition_lines_with_formatter(formatter);
    for line in &enum_lines {
        if line.mnemonic.is_empty() && line.bytes.is_empty() && line.comment.is_empty() {
            rows.push(ExportRow::Blank);
        } else if line.bytes.is_empty() && line.mnemonic.starts_with(&comment_prefix) {
            rows.push(ExportRow::SectionComment {
                text: line.mnemonic.clone(),
            });
        } else {
            rows.push(ExportRow::EnumLine {
                text: line.mnemonic.clone(),
            });
        }
    }

    // 4. Disassemble body without collapsed blocks
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
        scope_ends: state.annotations.scope_ends(),
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

        if line.is_splitter() {
            i += 1;
            continue;
        }

        if line.mnemonic.is_empty()
            && line.bytes.is_empty()
            && line.comment.is_empty()
            && line.label.is_none()
            && line.line_comment.is_none()
        {
            rows.push(ExportRow::Blank);
            i += 1;
            continue;
        }

        // Emit `ExportRow::Origin` immediately before the first non-blank body row so that even if
        // the binary starts with a `BlockType::ExternalFile` region at `state.origin`, the origin
        // directive (`* = $xxxx`) precedes the `.binary` / `!binary` / `.incbin` include directive.
        if !origin_emitted {
            rows.push(ExportRow::Origin {
                address: state.origin,
                directive: origin_directive.clone(),
            });
            origin_emitted = true;
        }

        let offset = line.address.0 as isize - state.origin.0 as isize;
        let is_external_file = offset >= 0
            && (offset as usize) < state.block_types.len()
            && state.block_types[offset as usize] == BlockType::ExternalFile;

        if is_external_file {
            let start_idx = offset as usize;
            let mut end_idx = start_idx;
            while end_idx < state.block_types.len()
                && state.block_types[end_idx] == BlockType::ExternalFile
            {
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
                let line_in_range = if next_line.address >= start_address {
                    next_line.address.offset_from(start_address) < byte_count
                } else {
                    false
                };
                if !line_in_range {
                    break;
                }
                if !next_line.is_splitter() {
                    if next_line.mnemonic.is_empty()
                        && next_line.bytes.is_empty()
                        && next_line.comment.is_empty()
                        && next_line.label.is_none()
                        && next_line.line_comment.is_none()
                    {
                        inner_rows.push(ExportRow::Blank);
                    } else {
                        inner_rows.push(ExportRow::Instruction(lower_instruction_row(
                            next_line,
                            state,
                            &settings,
                            formatter,
                            &comment_prefix,
                            &label_name_to_addr,
                        )));
                    }
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

        if line.bytes.is_empty() && line.mnemonic.starts_with(&comment_prefix) {
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

        // 4. ca65 mixed string & byte operands
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
    fn test_tokenize_operand_ignores_identifiers_inside_quotes() {
        test_tokenize_operand_quoted_strings_not_linkified();
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
    fn test_tokenize_operand_ignores_hex_and_binary_literals() {
        test_tokenize_operand_numeric_literals_not_linkified();
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
    fn test_tokenize_operand_links_defined_and_external_labels_in_expressions() {
        test_tokenize_operand_expressions();
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
    fn test_tokenize_operand_handles_local_labels_and_relative_branches() {
        test_tokenize_operand_local_labels_all_assemblers();
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
            vec![Addr(0xC000), Addr(0x1500), Addr(0x1800), Addr(0x1500)],
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

        // 4. Empty line comment returns None even when address has xrefs in state.cross_refs!
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
        state
            .cross_refs
            .insert(Addr(0x1000), vec![Addr(0x1500), Addr(0xC000), Addr(0xD000)]);

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

            // format_plain must reconstruct `raw` byte-for-byte
            assert_eq!(comment.format_plain(cp), raw);

            // format_html must escape `&`, `<`, `>` in prefix and suffix, and link internal xrefs
            // with uppercase `#L{:04X}` href and lowercase `${:04x}` display text.
            let expected_html = format!(
                "Check &lt;A &amp; B&gt; {cp} x-ref: <a href=\"#L1500\">$1500</a>, $c000, ...; Target &gt; $00"
            );
            assert_eq!(comment.format_html(cp), expected_html);
        }
    }

    #[test]
    fn test_export_comment_decomposition_and_formatting_preserves_suffix_and_truncation() {
        test_export_comment_decomposition_variants();
        test_export_comment_format_plain_vs_format_html();
    }

    #[test]
    fn test_export_comment_suppressed_on_empty_line_comment_even_when_address_has_xrefs() {
        let mut state = make_state_with_internal_labels(0x1000, 2, &[]);
        // 2-byte PETSCII text block at $1000 ("HI") in 64tass emits `.encode`, `.enc "none"`,
        // `.text "HI"`, `.endencode` all at address $1000, with `line.comment` empty on the
        // wrapper lines and populated only on `.text "HI"`.
        state.raw_data = vec![0x48, 0x49];
        state.block_types = vec![BlockType::PetsciiText; 2];
        state.cross_refs.insert(Addr(0x1000), vec![Addr(0x1001)]);
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
        // Even when origin is $0000, preamble directives, equates, and enums must be lowered
        // into their distinct ExportRow variants before ExportRow::Origin and body instructions.
        state.origin = Addr::ZERO;
        state.raw_data = vec![0xA9, 0x00, 0x60]; // LDA #$00; RTS
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
    fn test_build_export_document_separates_directives_equates_enums_and_code() {
        test_build_export_document_separate_lowering_of_directives_equates_enums();
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

        // Also verify empty body fallback still emits Origin at the end
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
    fn test_build_export_document_external_file_at_origin_hoists_origin_and_emits_sidecar() {
        test_build_export_document_origin_before_leading_external_file();
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
    fn test_export_bundle_write_to_disk_writes_primary_and_sidecars() {
        test_export_bundle_write_to_disk_and_error_handling();
    }
}
