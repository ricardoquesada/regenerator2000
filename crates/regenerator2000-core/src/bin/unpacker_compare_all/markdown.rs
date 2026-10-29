//! `--markdown`: a deterministic comparison report for the
//! `compare-unpacker` skill.
//!
//! One row per case, sorted by file, plus a summary of divergence
//! categories. The same observations always render the same text,
//! regardless of their order.

use std::collections::BTreeMap;

use crate::corpus::{CaseResult, Divergence, DivergenceKind, HexAddr, ReviewState};
use crate::observation::Observation;
use crate::unp64::Unp64Outcome;

/// Placeholder for an empty cell.
const EMPTY_CELL: &str = "-";

/// The short, stable category name of a divergence (the `kind` names of
/// the manifest where one exists).
///
/// # Example
///
/// `Divergence::PayloadMismatch { .. }` is `"payload_mismatch"`.
#[must_use]
pub fn divergence_category(divergence: &Divergence) -> &'static str {
    match divergence {
        Divergence::Agree => "agree",
        Divergence::PaddingByte => "padding_byte",
        Divergence::Unp64DefaultEnd => "unp64_default_end",
        Divergence::Unp64DefaultStart => "unp64_default_start",
        Divergence::Unp64MissingOpcode => "unp64_missing_opcode",
        Divergence::Unexplained => "unexplained",
        Divergence::PayloadMismatch { .. } => "payload_mismatch",
        Divergence::EntryMismatch => "entry_mismatch",
        Divergence::OneSideFailed { .. } => "one_side_failed",
    }
}

/// The category plus its details.
///
/// # Example
///
/// `PayloadMismatch { first: 0xD000, count: 3 }` is
/// `"payload_mismatch (3 byte(s), first at $D000)"`.
#[must_use]
pub fn divergence_label(divergence: &Divergence) -> String {
    let category = divergence_category(divergence);
    match divergence {
        Divergence::PayloadMismatch { first, count } => {
            format!("{category} ({count} byte(s), first at {})", HexAddr(*first))
        }
        Divergence::OneSideFailed { r2000, unp64 } => format!(
            "{category} (R2000: {}; unp64: {})",
            r2000.as_deref().unwrap_or("ok"),
            unp64.as_deref().unwrap_or("ok")
        ),
        _ => category.to_string(),
    }
}

/// One rendered table row.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Row {
    file: String,
    unp64_packer: String,
    unp64: String,
    r2000: String,
    divergence: String,
    manifest: String,
}

/// unp64's result as `$0801-$FFFD → $0911`, or `failed: <reason>`.
fn unp64_cell(outcome: &Unp64Outcome) -> String {
    match outcome {
        Unp64Outcome::Saved { saved, entry, .. } => format!("{} → {entry}", saved.range),
        Unp64Outcome::Failed { reason, .. } => format!("failed: {reason}"),
    }
}

/// R2000's result as `$0801-$7949 → $0911 (start=..., end=..., entry=..., exit=...)`,
/// or `error: <variant>`.
fn r2000_cell(result: &CaseResult) -> String {
    match &result.outcome {
        Ok(image) => format!(
            "{}-{} → {} (start={}, end={}, entry={}, exit={})",
            HexAddr(image.start),
            HexAddr(image.end),
            HexAddr(image.entry),
            image.report.start_source,
            image.report.end_source,
            image.report.entry_source,
            image.report.exit_reason,
        ),
        Err(error) => format!("error: {}", error.variant),
    }
}

/// How the manifest treats the case: ground truth, a reviewed exception,
/// or the plain unp64 reference.
fn manifest_cell(observation: &Observation) -> String {
    let case = &observation.case;
    let expected_fail = case
        .expected_fail
        .map(|reason| format!(", expected_fail {reason}"))
        .unwrap_or_default();
    if let Some(group) = &case.group {
        return format!("ground truth `{group}`{expected_fail}");
    }
    let Some(exception) = &case.accepted_divergence else {
        return format!("unp64 reference{expected_fail}");
    };
    let review = match exception.review_state() {
        ReviewState::Unsigned => "UNSIGNED",
        ReviewState::PendingUser => "pending user review",
        ReviewState::Signed => "signed",
    };
    format!(
        "accepted `{}` [{}] ({review}){expected_fail}",
        exception.kind, exception.fields
    )
}

/// Escapes a table cell (`|` would end the cell).
fn cell(text: &str) -> String {
    if text.is_empty() {
        return EMPTY_CELL.to_string();
    }
    text.replace('|', "\\|")
}

/// Renders the report for unp64 `version`.
#[must_use]
pub fn render_markdown(observations: &[Observation], version: &str) -> String {
    let mut rows: Vec<Row> = observations
        .iter()
        .map(|observation| Row {
            file: observation.case.file.clone(),
            unp64_packer: observation
                .unp64
                .packer_name()
                .unwrap_or_default()
                .to_string(),
            unp64: unp64_cell(&observation.unp64),
            r2000: r2000_cell(&observation.result),
            divergence: divergence_label(&observation.divergence()),
            manifest: manifest_cell(observation),
        })
        .collect();
    rows.sort();

    let mut categories: BTreeMap<&'static str, usize> = BTreeMap::new();
    for observation in observations {
        *categories
            .entry(divergence_category(&observation.divergence()))
            .or_default() += 1;
    }

    let mut lines = vec![
        format!("# Regenerator 2000 unpacker vs unp64 v{version}"),
        String::new(),
        format!(
            "{} case(s) from `tests/6502/unpack_corpus.toml`.",
            rows.len()
        ),
        String::new(),
        "| Divergence | Cases |".to_string(),
        "| :-- | --: |".to_string(),
    ];
    lines.extend(
        categories
            .iter()
            .map(|(category, count)| format!("| {category} | {count} |")),
    );
    lines.extend([
        String::new(),
        "| File | unp64 packer | unp64 (range → entry) | R2000 (range → entry) | Divergence | Manifest |"
            .to_string(),
        "| :-- | :-- | :-- | :-- | :-- | :-- |".to_string(),
    ]);
    lines.extend(rows.iter().map(|row| {
        format!(
            "| `{}` | {} | {} | {} | {} | {} |",
            row.file,
            cell(&row.unp64_packer),
            cell(&row.unp64),
            cell(&row.r2000),
            cell(&row.divergence),
            cell(&row.manifest)
        )
    }));

    let mut items: Vec<String> = observations.iter().flat_map(action_items).collect();
    items.sort();
    lines.extend([
        String::new(),
        "## User action items".to_string(),
        String::new(),
    ]);
    if items.is_empty() {
        lines.push("None.".to_string());
    } else {
        lines.extend(items);
    }
    lines.join("\n")
}

/// What the user must do about one case, from its manifest entry: sign or
/// confirm its exception, investigate an `unexplained` difference, or
/// resolve an `expected_fail` marker.
fn action_items(observation: &Observation) -> Vec<String> {
    let case = &observation.case;
    let mut items = Vec::new();
    if let Some(exception) = &case.accepted_divergence {
        match exception.review_state() {
            ReviewState::Unsigned => items.push(format!(
                "- `{}`: accepted_divergence `{}` is unsigned; review it and set `reviewed_by`",
                case.file, exception.kind
            )),
            ReviewState::PendingUser => items.push(format!(
                "- `{}`: accepted_divergence `{}` [{}] carries the initial marker; confirm it \
                 and sign with your name",
                case.file, exception.kind, exception.fields
            )),
            ReviewState::Signed => {}
        }
        if exception.kind == DivergenceKind::Unexplained {
            items.push(format!(
                "- `{}`: unexplained divergence [{}], investigate",
                case.file, exception.fields
            ));
        }
    }
    if let Some(reason) = case.expected_fail {
        let hypothesis = case
            .accepted_divergence
            .as_ref()
            .and_then(|exception| exception.note.as_deref())
            .map(|note| format!(": {note}"))
            .unwrap_or_default();
        items.push(format!(
            "- `{}`: expected_fail `{reason}`; resolve it, then remove the marker{hypothesis}",
            case.file
        ));
    }
    items
}
