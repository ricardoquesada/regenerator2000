//! Pure in-memory and disk-backed interactive HTML (`.html` + `_{addr:04x}.html` sidecars)
//! exporter backend.
//!
//! This module implements **Stage 2 (HTML Backend)** of the exporter pipeline.
//! [`render_html_document`] consumes a pre-lowered [`ExportDocument`] and produces an in-memory
//! [`ExportBundle`] with zero filesystem I/O:
//!
//! - **Strict HTML Escaping:** Every interpolated string (`source_name`, `build_command_lines`,
//!   comments, labels, mnemonics, and [`OperandToken`] text) passes through [`html_escape`] so
//!   low/high-byte operands like `lda #<label` never inject raw `<label>` tags into the DOM.
//! - **Unique Anchor IDs (`emitted_anchors`):** Row anchor attributes (`id="L{:04X}"`) are tracked
//!   in a per-document `HashSet<Addr>` and emitted at most once per address on [`ExportRow::Equate`],
//!   [`super::ir::MidLabel`], and the first instruction/data `<tr>` for a given address — never on
//!   [`ExportRow::EnumLine`] or standalone [`LabelPlacement::OwnLine`] rows.
//! - **External File Sub-Pages:** [`ExportRow::ExternalFile`] blocks render their `inner_rows` into
//!   a standalone HTML sidecar (`{stem}_{start:04x}.html`) stored in [`ExportBundle::sidecars`].

use super::ir::{
    ExportBundle, ExportComment, ExportDocument, ExportOptions, ExportRow, ExportSidecar,
    LabelPlacement, OperandToken, build_export_document, html_escape,
};
use crate::error::CoreError;
use crate::state::{Addr, AppState};
use std::collections::HashSet;
use std::fmt::Write as _;
use std::path::Path;

// CSS class name abbreviations used throughout this file:
//   ad = address          by = bytes           mn = mnemonic
//   op = operand          cm = comment         lb = label
//   il = inline-label     cc = code-cell       ml = mid-label
//   mld = mid-label-def   sr = separator-row   bh = block-header
//   as = assignment

const HTML_START: &str = r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<title>Regenerator 2000 Disassembly</title>
<style>
:root {
  --bg-color: #ffffff;
  --text-color: #24292f;
  --address-color: #57606a;
  --bytes-color: #6e7781;
  --mnemonic-color: #cf222e;
  --operand-color: #0550ae;
  --comment-color: #6e7781;
  --label-color: #953800;
  --link-color: #0969da;
  --mid-label-color: #8250df;
  --highlight-bg: rgba(84, 174, 255, 0.3);
  --highlight-border: #0969da;
}
[data-theme="dark"] {
  --bg-color: #0d1117;
  --text-color: #c9d1d9;
  --address-color: #8b949e;
  --bytes-color: #6e7681;
  --mnemonic-color: #ff7b72;
  --operand-color: #79c0ff;
  --comment-color: #8b949e;
  --label-color: #ffa657;
  --link-color: #58a6ff;
  --mid-label-color: #d2a8ff;
  --highlight-bg: rgba(56, 139, 253, 0.15);
  --highlight-border: #1f6feb;
}
body { font-family: ui-monospace, SFMono-Regular, 'SF Pro Mono', Menlo, Consolas, 'Liberation Mono', monospace; font-size: 12px; line-height: 1.45; background-color: var(--bg-color); color: var(--text-color); margin: 0; padding: 20px; }
table { border-collapse: collapse; width: 100%; max-width: 1200px; margin: 0 auto; }
td { padding: 0px 8px; line-height: 1.4; vertical-align: top; white-space: pre; }
a { color: var(--link-color); text-decoration: none; }
a:hover { text-decoration: underline; }
.ad { color: var(--address-color); text-align: left; }
.ad a { color: inherit; }
.by { color: var(--bytes-color); text-align: left; }
.mn { color: var(--mnemonic-color); text-align: left; }
.op { color: var(--operand-color); text-align: left; }
.cm { color: var(--comment-color); }
.lb { color: var(--label-color); font-weight: bold; }
.ml { color: var(--mid-label-color); }
.sr { height: 15px; }
.cc { display: flex; align-items: top; padding: 0 !important; }
.cc .il { width: 20ch; flex-shrink: 0; color: var(--label-color); font-weight: bold; }
.cc .mn { min-width: 6ch; flex-shrink: 0; }
.cc > .mn:first-child { padding-left: 20ch; }
.cc .op { min-width: 15ch; flex-shrink: 0; padding-left: 1ch; }
.cc .cm { padding: 0 8px; margin-left: 4ch; }
.cc.as { padding: 0 8px !important; }
.cc.as .lb { min-width: 40ch; flex-shrink: 0; }
.cc.bh .cm { margin-left: 0ch; padding: 0; }
.mld { display: inline-block; min-width: 60ch; }
tr:target td { background-color: var(--highlight-bg); }
tr:target td:first-child { border-left: 3px solid var(--highlight-border); }
#theme-toggle { position: fixed; top: 20px; right: 20px; padding: 8px 16px; background-color: var(--text-color); color: var(--bg-color); border: 1px solid var(--text-color); cursor: pointer; font-family: inherit; font-weight: bold; }
</style>
</head>
<body>
<button id="theme-toggle">Toggle Theme</button>
<table>
"#;

const HTML_END: &str = r#"</table>
<script>
const toggleBtn = document.getElementById('theme-toggle');
const urlParams = new URLSearchParams(window.location.search);
let currentTheme = urlParams.get('theme') || 'dark';

function updateLinks() {
  document.querySelectorAll('a').forEach(link => {
    let href = link.getAttribute('href');
    if (href && !href.startsWith('#')) {
      let parts = href.split('?');
      let base = parts[0];
      let params = new URLSearchParams(parts[1] || '');
      params.set('theme', currentTheme);
      link.setAttribute('href', base + '?' + params.toString());
    }
  });
}

document.documentElement.setAttribute('data-theme', currentTheme);
toggleBtn.textContent = currentTheme === 'dark' ? 'Light Mode' : 'Dark Mode';
updateLinks();

toggleBtn.addEventListener('click', () => {
  currentTheme = currentTheme === 'dark' ? 'light' : 'dark';
  document.documentElement.setAttribute('data-theme', currentTheme);
  toggleBtn.textContent = currentTheme === 'dark' ? 'Light Mode' : 'Dark Mode';
  updateLinks();

  const newUrl = new URL(window.location.href);
  newUrl.searchParams.set('theme', currentTheme);
  window.history.pushState({}, '', newUrl);
});
</script>
</body>
</html>
"#;

fn render_side_comment_html(side_comment: Option<&ExportComment>, comment_prefix: &str) -> String {
    if let Some(comment) = side_comment {
        let formatted = comment.format_html(comment_prefix);
        if !formatted.is_empty() {
            let escaped_cp = html_escape(comment_prefix);
            return format!("<span class=\"cm\">{escaped_cp} {formatted}</span>");
        }
    }
    String::new()
}

fn render_operand_tokens_html(tokens: &[OperandToken]) -> String {
    let mut out = String::new();
    for token in tokens {
        match token {
            OperandToken::Text(text) => {
                out.push_str(&html_escape(text));
            }
            OperandToken::InternalLink { text, target } => {
                let escaped_text = html_escape(text);
                let _ = write!(out, "<a href=\"#L{:04X}\">{escaped_text}</a>", target.0);
            }
        }
    }
    out
}

fn render_rows_html(
    rows: &[ExportRow],
    comment_prefix: &str,
    primary_path: &Path,
    sidecars: &mut Vec<ExportSidecar>,
    emitted_anchors: &mut HashSet<Addr>,
) -> String {
    let mut output = String::new();
    let escaped_cp = html_escape(comment_prefix);

    for row in rows {
        match row {
            ExportRow::Blank => {
                output.push_str("<tr class=\"sr\"><td colspan=\"5\"></td></tr>\n");
            }
            ExportRow::SectionComment { text } => {
                let escaped = html_escape(text);
                let _ = writeln!(
                    output,
                    "<tr><td colspan=\"2\"></td><td colspan=\"3\" class=\"cc bh\"><span class=\"cm\">{escaped}</span></td></tr>"
                );
            }
            ExportRow::FileDirective {
                directive,
                side_comment,
            } => {
                let escaped = html_escape(directive);
                let comment_part = render_side_comment_html(side_comment.as_ref(), comment_prefix);
                let _ = writeln!(
                    output,
                    "<tr><td colspan=\"2\"></td><td colspan=\"3\" class=\"cc as\"><span class=\"lb\">{escaped}</span>{comment_part}</td></tr>"
                );
            }
            ExportRow::Equate {
                address,
                definition,
                side_comment,
            } => {
                let tr_id = if emitted_anchors.insert(*address) {
                    format!(" id=\"L{:04X}\"", address.0)
                } else {
                    String::new()
                };
                let escaped = html_escape(definition);
                let comment_part = render_side_comment_html(side_comment.as_ref(), comment_prefix);
                let _ = writeln!(
                    output,
                    "<tr{tr_id}><td colspan=\"2\"></td><td colspan=\"3\" class=\"cc as\"><span class=\"lb\">{escaped}</span>{comment_part}</td></tr>"
                );
            }
            ExportRow::EnumLine { text } => {
                // Enum definition lines are not tied to a memory address and must never emit
                // `id="L0000"`, preventing duplicate HTML element IDs and `#L0000` link collisions.
                let escaped = html_escape(text);
                let _ = writeln!(
                    output,
                    "<tr><td colspan=\"2\"></td><td colspan=\"3\" class=\"cc as\"><span class=\"lb\">{escaped}</span></td></tr>"
                );
            }
            ExportRow::Origin { .. } => {
                // HTML table rows display their own `$XXXX` address in column 1;
                // no standalone `* = $XXXX` row is emitted.
            }
            ExportRow::ExternalFile {
                start_address,
                html_filename,
                inner_rows,
                html_directive,
                ..
            } => {
                let mut ext_anchors = HashSet::new();
                let inner_html = render_rows_html(
                    inner_rows,
                    comment_prefix,
                    primary_path,
                    sidecars,
                    &mut ext_anchors,
                );
                let mut ext_doc =
                    String::with_capacity(HTML_START.len() + inner_html.len() + HTML_END.len());
                ext_doc.push_str(HTML_START);
                ext_doc.push_str(&inner_html);
                ext_doc.push_str(HTML_END);

                sidecars.push(ExportSidecar::new(
                    primary_path,
                    html_filename.clone(),
                    ext_doc.into_bytes(),
                ));

                let (mnemonic, operand) = html_directive;
                let escaped_mnemonic = html_escape(mnemonic);
                let escaped_filename = html_escape(html_filename);
                let escaped_operand = html_escape(operand);
                let linked_operand = escaped_operand.replace(
                    &escaped_filename,
                    &format!("<a href=\"{escaped_filename}\">{escaped_filename}</a>"),
                );
                let _ = writeln!(
                    output,
                    "<tr><td class=\"ad\">${:04X}</td><td></td><td colspan=\"3\" class=\"cc\"><span class=\"mn\">{escaped_mnemonic}</span><span class=\"op\">{linked_operand}</span></td></tr>",
                    start_address.0
                );
            }
            ExportRow::Instruction(inst) => {
                // 1. Mid-instruction relative labels
                for mid in &inst.mid_labels {
                    let mid_id = if emitted_anchors.insert(mid.address) {
                        format!(" id=\"L{:04X}\"", mid.address.0)
                    } else {
                        String::new()
                    };
                    let escaped_def = html_escape(&mid.definition);
                    let mid_comment =
                        render_side_comment_html(mid.side_comment.as_ref(), comment_prefix);
                    let _ = writeln!(
                        output,
                        "<tr{mid_id}><td colspan=\"2\"></td><td colspan=\"3\" class=\"cc ml\"><span class=\"mld\">{escaped_def}</span>{mid_comment}</td></tr>"
                    );
                }

                // 2. Full-line block comments preceding this instruction
                for comment_line in &inst.line_comments {
                    let escaped_line = html_escape(comment_line);
                    let _ = writeln!(
                        output,
                        "<tr><td colspan=\"2\"></td><td colspan=\"3\" class=\"cc bh\"><span class=\"cm\">{escaped_cp} {escaped_line}</span></td></tr>"
                    );
                }

                // 3. Label placement (`OwnLine` vs `Inline` vs `None`)
                let mut inline_label_html = String::new();
                match inst.label_placement {
                    LabelPlacement::OwnLine => {
                        if let Some(label_text) = &inst.label {
                            let escaped_label = html_escape(label_text);
                            let _ = writeln!(
                                output,
                                "<tr><td colspan=\"2\"></td><td colspan=\"3\" class=\"cc lb\">{escaped_label}</td></tr>"
                            );
                        }
                    }
                    LabelPlacement::Inline => {
                        if let Some(label_text) = &inst.label {
                            inline_label_html =
                                format!("<span class=\"il\">{}</span>", html_escape(label_text));
                        }
                    }
                    LabelPlacement::None => {}
                }

                // 4. Address cell and unique row anchor ID.
                // Multi-line directives (e.g. `.encode` / `.text` / `.endencode` in 64tass) emit
                // multiple rows sharing the same `inst.address`; `emitted_anchors.insert` ensures
                // only the first row with `has_addr` receives `id="L{:04X}"`.
                let has_addr = !inst.bytes.is_empty() || inst.label.is_some();
                let address_td = if has_addr {
                    format!(
                        "<td class=\"ad\"><a href=\"#L{:04X}\">${:04X}</a></td>",
                        inst.address.0, inst.address.0
                    )
                } else {
                    "<td></td>".to_string()
                };
                let tr_id = if has_addr && emitted_anchors.insert(inst.address) {
                    format!(" id=\"L{:04X}\"", inst.address.0)
                } else {
                    String::new()
                };

                // 5. Bytes cell
                let bytes_td = if inst.show_bytes {
                    let bytes_formatted = inst
                        .bytes
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<Vec<_>>()
                        .join(" ");
                    format!("<td class=\"by\">{bytes_formatted}</td>")
                } else {
                    "<td></td>".to_string()
                };

                // 6. Code cell (label/scope brace, mnemonic, operand, side comment)
                let operand_html = render_operand_tokens_html(&inst.operand_tokens);
                let comment_part =
                    render_side_comment_html(inst.side_comment.as_ref(), comment_prefix);
                let escaped_mnemonic = html_escape(&inst.mnemonic);

                if inst.is_scope_brace {
                    // Scope braces (`{` and `}`) are placed in the 20ch label column (`.il`)
                    // so they render at column 0 instead of being indented 20ch by
                    // `.cc > .mn:first-child`.
                    let _ = writeln!(
                        output,
                        "<tr{tr_id}>{address_td}{bytes_td}<td colspan=\"3\" class=\"cc\"><span class=\"il\">{escaped_mnemonic}</span><span class=\"mn\"></span><span class=\"op\">{operand_html}</span>{comment_part}</td></tr>"
                    );
                } else {
                    let _ = writeln!(
                        output,
                        "<tr{tr_id}>{address_td}{bytes_td}<td colspan=\"3\" class=\"cc\">{inline_label_html}<span class=\"mn\">{escaped_mnemonic}</span><span class=\"op\">{operand_html}</span>{comment_part}</td></tr>"
                    );
                }
            }
        }
    }

    output
}

/// Renders a pre-lowered [`ExportDocument`] into an in-memory `.html` [`ExportBundle`].
///
/// This function performs zero filesystem I/O. Any [`ExportRow::ExternalFile`] blocks in `doc`
/// are rendered as standalone HTML sub-page sidecars in `bundle.sidecars` and linked from
/// `primary_content`.
#[must_use]
pub fn render_html_document(doc: &ExportDocument, path: &Path) -> ExportBundle {
    let mut output = String::new();
    let mut sidecars = Vec::new();
    let mut emitted_anchors = HashSet::new();

    output.push_str(HTML_START);

    let comment = html_escape(&doc.header.comment_prefix);
    let separator = format!(
        "{comment}=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-"
    );
    let version_line = html_escape(&doc.header.version_line);
    let escaped_url = html_escape(&doc.header.url);
    let escaped_source = html_escape(&doc.header.source_name);

    let static_header_lines = [
        separator.as_str(),
        comment.as_str(),
        version_line.as_str(),
        &format!("{comment} <a href=\"{escaped_url}\">{escaped_url}</a>"),
        comment.as_str(),
        &format!("{comment} Exported from: {escaped_source}"),
        comment.as_str(),
    ];
    for header_line in &static_header_lines {
        let _ = writeln!(
            output,
            "<tr><td colspan=\"5\" class=\"cm\">{header_line}</td></tr>"
        );
    }

    let escaped_asm_url = html_escape(&doc.header.assembler_url);
    let escaped_asm_name = html_escape(&doc.header.assembler_name);
    if !doc.header.build_command_lines.is_empty() {
        let linked_header = format!(
            "{comment} Assemble with <a href=\"{escaped_asm_url}\">{escaped_asm_name}</a>:"
        );
        let _ = writeln!(
            output,
            "<tr><td colspan=\"5\" class=\"cm\">{linked_header}</td></tr>"
        );
        for line in doc.header.build_command_lines.iter().skip(1) {
            let escaped = html_escape(line);
            let _ = writeln!(
                output,
                "<tr><td colspan=\"5\" class=\"cm\">{escaped}</td></tr>"
            );
        }
    } else {
        let fallback = format!(
            "{comment} <a href=\"{escaped_asm_url}\">{escaped_asm_name}</a> assembly format."
        );
        let _ = writeln!(
            output,
            "<tr><td colspan=\"5\" class=\"cm\">{fallback}</td></tr>"
        );
    }

    let _ = writeln!(
        output,
        "<tr><td colspan=\"5\" class=\"cm\">{separator}</td></tr>"
    );
    output.push_str("<tr class=\"sr\"><td colspan=\"5\"></td></tr>\n");

    let rows_html = render_rows_html(
        &doc.rows,
        &doc.header.comment_prefix,
        path,
        &mut sidecars,
        &mut emitted_anchors,
    );
    output.push_str(&rows_html);
    output.push_str(HTML_END);

    ExportBundle {
        primary_path: path.to_path_buf(),
        primary_content: output,
        sidecars,
    }
}

/// Lowers `state` with explicit [`ExportOptions`] and renders an in-memory `.html` [`ExportBundle`].
#[must_use]
pub fn render_html_with_options(
    state: &AppState,
    path: &Path,
    options: &ExportOptions,
) -> ExportBundle {
    let doc = build_export_document(state, options);
    render_html_document(&doc, path)
}

/// Lowers `state` using the assembler in `state.settings` and renders an in-memory `.html` [`ExportBundle`].
#[must_use]
pub fn render_html(state: &AppState, path: &Path) -> ExportBundle {
    let options = ExportOptions::from_state_and_path(state, path);
    render_html_with_options(state, path, &options)
}

/// Exports the disassembly to an HTML file (and any external-file HTML sidecars).
///
/// # Errors
/// Returns [`CoreError::Export`] if the primary HTML file or any sidecar file cannot be written.
pub fn export_html<P: AsRef<Path>>(state: &AppState, path: P) -> Result<(), CoreError> {
    render_html(state, path.as_ref()).write_to_disk()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{
        Addr, Assembler, BlockType, EnumDefinition, ImmediateFormat, Label, LabelKind, LabelType,
    };
    use std::collections::{BTreeMap, HashSet};
    use std::path::{Path, PathBuf};

    #[test]
    fn test_export_html() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.raw_data = vec![0xA9, 0x00, 0x8D, 0x20, 0xD0, 0x60];
        state.block_types = vec![BlockType::Code; 6];

        state.labels.insert(
            Addr(0x1000),
            vec![Label {
                name: "Start".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );

        let bundle = render_html(&state, Path::new("test_output.html"));
        let content = &bundle.primary_content;

        assert!(content.contains("<!DOCTYPE html>"));
        assert!(content.contains("<table>"));
        assert!(content.contains("</table>"));

        assert!(content.contains("id=\"L1000\""));
        assert!(content.contains("id=\"L1002\""));

        assert!(content.contains("a9 00"));
        assert!(content.contains("8d 20 d0"));

        assert!(content.contains("class=\"mn\""));
        assert!(content.contains("class=\"op\""));
    }

    /// Regression test: line comments must align with the label column.
    ///
    /// The code-cell for a line comment must:
    /// - Use the `bh` (block-header) class (which has `margin-left: 0; padding: 0`)
    /// - NOT contain an `il` (inline-label) span (which would push content to the
    ///   mnemonic column)
    ///
    /// This ensures line comments start at the same column as labels.
    #[test]
    fn test_line_comment_aligns_with_label_column() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.raw_data = vec![0xA9, 0x00, 0x60];
        state.block_types = vec![BlockType::Code; 3];

        state.labels.insert(
            Addr(0x1000),
            vec![Label {
                name: "my_routine".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );
        state.annotations.update(Addr(0x1000), |e| {
            e.user_line_comment = Some("Load accumulator".to_string())
        });

        let bundle = render_html(&state, Path::new("test_line_comment_align.html"));
        let content = &bundle.primary_content;

        // Line comment row must use bh (block-header) class for label-column alignment.
        assert!(
            content.contains("class=\"cc bh\""),
            "Line comment must use bh class for label-column alignment"
        );

        // Line comment must NOT contain an il (inline-label) span, which would shift it right.
        for line in content.lines() {
            if line.contains("cc bh") {
                assert!(
                    !line.contains("class=\"il\""),
                    "Line comment row must not contain il span (would misalign): {line}"
                );
            }
        }

        // The il span must appear in regular instruction rows (sanity check
        // that labels are rendered in code-cells with inline-label).
        assert!(
            content.contains("<span class=\"il\">my_routine</span>"),
            "Inline labels must use the il span"
        );
    }

    /// Regression test: line comments must have a space between the comment prefix and text.
    ///
    /// Correct:   `; Load accumulator`
    /// Bug was:   `;Load accumulator`
    #[test]
    fn test_line_comment_has_space_after_prefix() {
        let mut state = AppState::new();
        state.origin = Addr(0x2000);
        state.raw_data = vec![0x60];
        state.block_types = vec![BlockType::Code; 1];

        state.annotations.update(Addr(0x2000), |e| {
            e.user_line_comment = Some("Return to caller".to_string())
        });

        let bundle = render_html(&state, Path::new("test_line_comment_space.html"));
        let content = &bundle.primary_content;

        // Must have "; Return" (with space), not ";Return"
        assert!(
            content.contains("; Return to caller"),
            "Line comment must have a space between ';' and the comment text"
        );
        assert!(
            !content.contains(";Return"),
            "Line comment must not have ';' directly adjacent to the comment text"
        );
    }

    /// Regression test: side comments (on instruction lines) must have a space after the prefix.
    ///
    /// Correct:   `; Save value`
    /// Bug would be:  `;Save value`
    #[test]
    fn test_side_comment_has_space_after_prefix() {
        let mut state = AppState::new();
        state.origin = Addr(0x3000);
        state.raw_data = vec![0x8D, 0x00, 0xD0];
        state.block_types = vec![BlockType::Code; 3];

        state.annotations.update(Addr(0x3000), |e| {
            e.user_side_comment = Some("Store to VIC".to_string())
        });

        let bundle = render_html(&state, Path::new("test_side_comment_space.html"));
        let content = &bundle.primary_content;

        // Side comment must have "; Store" (with space)
        assert!(
            content.contains("; Store to VIC"),
            "Side comment must have a space between ';' and the comment text"
        );
    }

    /// Regression test: the CSS must include `padding: 0` for block-header comments.
    ///
    /// Without this, block-header comments inherit `padding: 0 8px` from
    /// `.cc .cm`, causing a ~1ch horizontal offset vs labels.
    #[test]
    fn test_css_block_header_has_zero_padding() {
        let mut state = AppState::new();
        state.origin = Addr(0x4000);
        state.raw_data = vec![0x60];
        state.block_types = vec![BlockType::Code; 1];

        let bundle = render_html(&state, Path::new("test_css_padding.html"));
        let content = &bundle.primary_content;

        // The block-header comment rule must zero out padding to prevent
        // inheriting the 8px left-padding from `.cc .cm`.
        assert!(
            content.contains(".cc.bh .cm { margin-left: 0ch; padding: 0; }"),
            "CSS must include padding: 0 for block-header comments to align with labels"
        );
    }

    /// Regression test: multi-line comments must each be on their own row,
    /// all aligned at the label column.
    #[test]
    fn test_multiline_comment_alignment() {
        let mut state = AppState::new();
        state.origin = Addr(0x5000);
        state.raw_data = vec![0xA9, 0x00, 0x60];
        state.block_types = vec![BlockType::Code; 3];

        state.annotations.update(Addr(0x5000), |e| {
            e.user_line_comment = Some("First line\nSecond line\nThird line".to_string())
        });

        let bundle = render_html(&state, Path::new("test_multiline_comment.html"));
        let content = &bundle.primary_content;

        // Each line of a multi-line comment must be in its own bh (block-header) row
        let block_header_count = content.matches("cc bh").count();
        assert!(
            block_header_count >= 3,
            "Expected at least 3 block-header rows for 3 comment lines, got {block_header_count}"
        );

        // Each line must have proper spacing
        assert!(content.contains("; First line"));
        assert!(content.contains("; Second line"));
        assert!(content.contains("; Third line"));

        // No block-header row should contain an il (inline-label) span
        for line in content.lines() {
            if line.contains("cc bh") {
                assert!(
                    !line.contains("class=\"il\""),
                    "Multi-line comment row must not contain il span: {line}"
                );
            }
        }
    }

    /// Regression test: instruction rows without labels must NOT emit an empty
    /// inline-label span. The CSS rule `.cc > .mn:first-child` handles the
    /// 20ch indent automatically.
    #[test]
    fn test_no_empty_inline_label_span() {
        let mut state = AppState::new();
        state.origin = Addr(0x6000);
        state.raw_data = vec![0xA9, 0x00, 0x60]; // LDA #$00 ; RTS
        state.block_types = vec![BlockType::Code; 3];

        // No labels — all rows should omit the inline-label span
        let bundle = render_html(&state, Path::new("test_no_empty_il.html"));
        let content = &bundle.primary_content;

        // The CSS rule for first-child mnemonic must exist
        assert!(
            content.contains(".cc > .mn:first-child { padding-left: 20ch; }"),
            "CSS must have .cc > .mn:first-child rule for label-less alignment"
        );

        // No instruction row should contain an empty il span
        assert!(
            !content.contains("<span class=\"il\"></span>"),
            "Empty il spans must not be emitted; CSS :first-child handles alignment"
        );
    }

    #[test]
    fn test_html_escapes_low_high_byte_operands_strings_and_comments() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.file_path = Some(PathBuf::from("evil<name>&test.prg"));
        // $1000: LDA #<label   (A9 04)
        // $1002: LDA #>label   (A9 10)
        // $1004: LDA #<ext_lbl (A9 20)
        // $1006: .text "A<B&C>D" (7 bytes: 41 3C 42 26 43 3E 44)
        state.raw_data = vec![
            0xA9, 0x04, 0xA9, 0x10, 0xA9, 0x20, b'A', b'<', b'B', b'&', b'C', b'>', b'D',
        ];
        state.block_types = vec![
            BlockType::Code,
            BlockType::Code,
            BlockType::Code,
            BlockType::Code,
            BlockType::Code,
            BlockType::Code,
            BlockType::PetsciiText,
            BlockType::PetsciiText,
            BlockType::PetsciiText,
            BlockType::PetsciiText,
            BlockType::PetsciiText,
            BlockType::PetsciiText,
            BlockType::PetsciiText,
        ];

        // Internal label `label` at $1004
        state.labels.insert(
            Addr(0x1004),
            vec![Label {
                name: "label".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );
        // External label `ext_lbl` at $D020
        state.labels.insert(
            Addr(0xD020),
            vec![Label {
                name: "ext_lbl".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::Field,
            }],
        );

        state.annotations.update(Addr(0x1000), |e| {
            e.immediate_format = Some(ImmediateFormat::LowByte(Addr(0x1004)));
            e.user_line_comment = Some("Line <script> & \"quote\"".to_string());
            e.user_side_comment = Some("Side a < b && b > c".to_string());
        });
        state.annotations.update(Addr(0x1002), |e| {
            e.immediate_format = Some(ImmediateFormat::HighByte(Addr(0x1004)));
        });
        state.annotations.update(Addr(0x1004), |e| {
            e.immediate_format = Some(ImmediateFormat::LowByte(Addr(0xD020)));
        });

        let bundle = render_html(&state, Path::new("escaped.html"));
        let content = &bundle.primary_content;

        // 1. Source filename in header is escaped
        assert!(
            content.contains("Exported from: evil&lt;name&gt;&amp;test.prg"),
            "Header source_name must be HTML-escaped: {content}"
        );

        // 2. Low/high byte operands on internal labels escape `<` and `>` while linking the label
        assert!(
            content.contains("<span class=\"op\">#&lt;<a href=\"#L1004\">label</a></span>"),
            "Expected escaped `#&lt;` before linked internal label: {content}"
        );
        assert!(
            content.contains("<span class=\"op\">#&gt;<a href=\"#L1004\">label</a></span>"),
            "Expected escaped `#&gt;` before linked internal label: {content}"
        );
        assert!(
            !content.contains("#<label") && !content.contains("#<<a"),
            "Raw `<label` or `<<a` must never appear in HTML output"
        );

        // 3. Low-byte operand on external label escapes `<`
        assert!(
            content.contains("<span class=\"op\">#&lt;ext_lbl</span>"),
            "Expected escaped `#&lt;ext_lbl` for external low-byte operand: {content}"
        );

        // 4. Text literal escapes `<`, `&`, `>`
        assert!(
            content.contains("A&lt;B&amp;C&gt;D"),
            "Expected escaped characters inside .text operand: {content}"
        );

        // 5. Line and side comments escape `<`, `&`, `>`
        assert!(
            content.contains("; Line &lt;script&gt; &amp; \"quote\""),
            "Expected escaped line comment: {content}"
        );
        assert!(
            content.contains("; Side a &lt; b &amp;&amp; b &gt; c"),
            "Expected escaped side comment: {content}"
        );
    }

    #[test]
    fn test_html_does_not_linkify_words_inside_string_or_hex_literals() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.settings.assembler = Assembler::Tass64;
        // $1000..$100B: .text "press start" (11 bytes)
        // $100B..$100D: LDA #$A0 (A9 A0)
        // $100D..$100E: .byte $FF (FF)
        // $100E..$1010: BNE _loop (D0 FE -> targets $100E)
        let mut data = b"press start".to_vec();
        data.extend_from_slice(&[0xA9, 0xA0, 0xFF, 0xD0, 0xFE]);
        let mut blocks = vec![BlockType::PetsciiText; 11];
        blocks.push(BlockType::Code);
        blocks.push(BlockType::Code);
        blocks.push(BlockType::DataByte);
        blocks.push(BlockType::Code);
        blocks.push(BlockType::Code);

        state.raw_data = data;
        state.block_types = blocks;

        // Define internal labels `start` ($1000), `a0` ($100B), `ff` ($100D), and local `loop` ($100E)
        state.labels.insert(
            Addr(0x1000),
            vec![Label {
                name: "start".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );
        state.labels.insert(
            Addr(0x100B),
            vec![Label {
                name: "a0".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );
        state.labels.insert(
            Addr(0x100D),
            vec![Label {
                name: "ff".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );
        state.labels.insert(
            Addr(0x100E),
            vec![Label {
                name: "loop".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::LocalUserDefined,
            }],
        );

        let bundle = render_html(&state, Path::new("no_false_links.html"));
        let content = &bundle.primary_content;

        // 1. `"press start"` must not contain an `<a>` tag around `start`
        assert!(
            content.contains("<span class=\"op\">\"press start\"</span>"),
            "Quoted string must not linkify `start`: {content}"
        );
        assert!(
            !content.contains("<a href=\"#L1000\">start</a>"),
            "`start` inside string literal must not be linked"
        );

        // 2. `#$a0` and `$ff` must not linkify `a0` or `ff`
        assert!(
            content.contains("<span class=\"op\">#$a0</span>"),
            "Hex immediate `#$a0` must not linkify `a0`: {content}"
        );
        assert!(
            content.contains("<span class=\"op\">$ff</span>"),
            "Hex byte `$ff` must not linkify `ff`: {content}"
        );

        // 3. `64tass` local label `_loop` at $100E MUST be linked
        assert!(
            content.contains("<span class=\"op\"><a href=\"#L100E\">_loop</a></span>"),
            "Local label `_loop` must be linked to #L100E: {content}"
        );
    }

    #[test]
    fn test_html_no_duplicate_anchor_ids_on_enums_or_multiline_blocks() {
        let mut state = AppState::new();
        state.origin = Addr(0x0000);
        state.settings.assembler = Assembler::Tass64;

        // 20 bytes of DataByte at $0000 (wraps across multiple `.byte` lines at the same block)
        // followed by 4 bytes of LoHiAddress at $0014 (emits `.byte <...` and `.byte >...` at $0014)
        // followed by `LDA #$00` at $0018 with a 22-char label (`LabelPlacement::OwnLine`)
        let mut raw_data = vec![0xAA; 20];
        raw_data.extend_from_slice(&[0x00, 0x18, 0x10, 0x20]); // LoHiAddress
        raw_data.extend_from_slice(&[0xA9, 0x00]); // LDA #$00
        let mut block_types = vec![BlockType::DataByte; 20];
        block_types.extend(vec![BlockType::LoHiAddress; 4]);
        block_types.extend(vec![BlockType::Code; 2]);

        state.raw_data = raw_data;
        state.block_types = block_types;

        // Add an enum definition referenced at $0018 so multi-line enum rows are emitted
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
        state.annotations.update(Addr(0x0018), |e| {
            e.enum_usage = Some("Colors".to_string());
        });

        // Add a long label (>= 20 chars) at $0018 so `LabelPlacement::OwnLine` emits a separate label row
        state.labels.insert(
            Addr(0x0018),
            vec![Label {
                name: "very_long_label_name_at_0018".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );

        let bundle = render_html(&state, Path::new("unique_ids.html"));
        let content = &bundle.primary_content;

        // Extract every `id="L..."` occurrence and verify 100% uniqueness
        let mut seen_ids = HashSet::new();
        let mut search_slice = content.as_str();
        while let Some(pos) = search_slice.find("id=\"L") {
            let after = &search_slice[pos + 4..];
            if let Some(end_quote) = after.find('"') {
                let id_val = &after[..end_quote];
                assert!(
                    seen_ids.insert(id_val.to_string()),
                    "Duplicate HTML anchor id=\"{id_val}\" found in output!\n{content}"
                );
                search_slice = &after[end_quote + 1..];
            } else {
                break;
            }
        }

        // Verify `L0000` and `L0018` were emitted exactly once
        assert!(
            seen_ids.contains("L0000"),
            "Expected anchor L0000 for origin $0000"
        );
        assert!(
            seen_ids.contains("L0018"),
            "Expected anchor L0018 for instruction at $0018"
        );

        // Verify enum lines do not carry `id="L0000"`
        for line in content.lines() {
            if (line.contains("Colors") || line.contains("BLACK") || line.contains("WHITE"))
                && line.contains("cc as")
            {
                assert!(
                    !line.contains("id=\"L0000\""),
                    "Enum line must not have id=\"L0000\": {line}"
                );
            }
        }
    }

    #[test]
    fn test_html_scope_braces_and_20_char_label_overflow_and_css_width() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.settings.assembler = Assembler::Kick;
        // $1000: JMP $1000 (4C 00 10)
        // $1003: RTS (60)
        state.raw_data = vec![0x4C, 0x00, 0x10, 0x60];
        state.block_types = vec![BlockType::Code; 4];

        // Scope at $1000..=$1002 with label `my_scope`
        state.annotations.update(Addr(0x1000), |e| {
            e.scope = Some(Addr(0x1002));
        });
        state.labels.insert(
            Addr(0x1000),
            vec![Label {
                name: "my_scope".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );

        // Mid-instruction label `rel1` at $1001 with cross-reference from $1000
        state.labels.insert(
            Addr(0x1001),
            vec![Label {
                name: "rel1".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );
        state.cross_refs.insert(Addr(0x1001), vec![Addr(0x1000)]);

        // Exactly 19-char label name at $1003 -> formatted by KickAssembler as `"nineteen_char_label:"` (20 chars == LABEL_COLUMN_WIDTH)
        state.labels.insert(
            Addr(0x1003),
            vec![Label {
                name: "nineteen_char_label".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );

        let bundle = render_html(&state, Path::new("kick_scope.html"));
        let content = &bundle.primary_content;

        // 1. CSS `.cc.as .lb` must be 40ch (matching DEFINITION_COLUMN_WIDTH / INSTRUCTION_COLUMN_WIDTH)
        assert!(
            content.contains(".cc.as .lb { min-width: 40ch; flex-shrink: 0; }"),
            "CSS must define .cc.as .lb with min-width: 40ch"
        );

        // 2. Scope label `my_scope:` is emitted on its own `cc lb` row, and `{` / `}` are placed in `.il`
        assert!(
            content.contains("<td colspan=\"3\" class=\"cc lb\">my_scope:</td>"),
            "Scope label must be emitted on its own `cc lb` row: {content}"
        );
        assert!(
            content.contains("<span class=\"il\">{</span><span class=\"mn\"></span>"),
            "Opening scope brace `{{` must be placed in `.il` column: {content}"
        );
        assert!(
            content.contains("<span class=\"il\">}</span><span class=\"mn\"></span>"),
            "Closing scope brace `}}` must be placed in `.il` column: {content}"
        );

        // 3. Mid-instruction label uses KickAssembler `.label rel1 = * + 1` syntax and lowercase x-ref text
        assert!(
            content.contains("<tr id=\"L1001\"><td colspan=\"2\"></td><td colspan=\"3\" class=\"cc ml\"><span class=\"mld\">.label rel1 = * + 1</span><span class=\"cm\">// x-ref: <a href=\"#L1000\">$1000</a></span></td></tr>"),
            "Mid-instruction label must use KickAssembler syntax and linked x-ref comment: {content}"
        );

        // 4. 20-char formatted label (`"nineteen_char_label:"`) overflows to its own `cc lb` row
        assert!(
            content.contains("<tr><td colspan=\"2\"></td><td colspan=\"3\" class=\"cc lb\">nineteen_char_label:</td></tr>"),
            "20-char formatted label must overflow to its own `cc lb` row: {content}"
        );
        assert!(
            !content.contains("<span class=\"il\">nineteen_char_label:</span>"),
            "20-char formatted label must not be rendered inline in `.il`: {content}"
        );

        // 5. Verify ca65 `.macpack cbm` FileDirective is rendered as a `cc as` directive row outside header comments
        let ca65_opts =
            ExportOptions::for_assembler(&state, Path::new("ca65_out.html"), Assembler::Ca65);
        let ca65_bundle = render_html_with_options(&state, Path::new("ca65_out.html"), &ca65_opts);
        assert!(
            ca65_bundle.primary_content.contains(
                "<tr><td colspan=\"2\"></td><td colspan=\"3\" class=\"cc as\"><span class=\"lb\">.macpack cbm</span><span class=\"cm\">; adds support for scrcode</span></td></tr>"
            ),
            "ca65 `.macpack cbm` must be rendered as a `cc as` directive row outside the header banner: {}",
            ca65_bundle.primary_content
        );
    }

    #[test]
    fn test_html_external_file_sidecar_and_link_row() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // $1000: NOP (EA)
        // $1001..=$1002: ExternalFile (11 22)
        // $1003: RTS (60)
        state.raw_data = vec![0xEA, 0x11, 0x22, 0x60];
        state.block_types = vec![
            BlockType::Code,
            BlockType::ExternalFile,
            BlockType::ExternalFile,
            BlockType::Code,
        ];

        let primary_path = Path::new("/tmp/nonexistent_dir_for_in_memory_test/demo.html");
        let bundle = render_html(&state, primary_path);

        // 1. Primary HTML contains link row pointing to `demo_1001.html`
        assert!(
            bundle
                .primary_content
                .contains("<a href=\"demo_1001.html\">demo_1001.html</a>"),
            "Primary HTML must contain hyperlink to sidecar HTML file: {}",
            bundle.primary_content
        );

        // 2. Sidecar bundle is populated in memory without touching disk
        assert_eq!(bundle.sidecars.len(), 1);
        let sidecar = &bundle.sidecars[0];
        assert_eq!(sidecar.filename, "demo_1001.html");
        assert_eq!(
            sidecar.path,
            PathBuf::from("/tmp/nonexistent_dir_for_in_memory_test/demo_1001.html")
        );
        let sidecar_html = sidecar.content_utf8().expect("valid UTF-8 sidecar HTML");
        assert!(sidecar_html.contains("<!DOCTYPE html>"));
        assert!(sidecar_html.contains("id=\"L1001\""));
        assert!(sidecar_html.contains("$11, $22"));
        assert!(!primary_path.exists());
        assert!(!sidecar.path.exists());
    }
}
