//! Human-readable rendering of evaluation outcomes.

use super::{CaseFailure, CaseReport};

/// Column header of the subject column in the failure table.
const SUBJECT_HEADER: &str = "Subject";

/// Renders every failure as a table, sorted by subject and then by reason
/// kind. Returns `"no failures"` for an empty list.
///
/// # Example
///
/// ```text
/// 2 unpack corpus failure(s):
/// | Subject | Failure
/// |---------|--------
/// | a.prg   | `end` differs from unp64: unp64 $FFFF, R2000 $2000
/// | b.prg   | accepted_divergence is proposed, unsigned: ...
/// ```
#[must_use]
pub fn render_failure_table(failures: &[CaseFailure]) -> String {
    if failures.is_empty() {
        return "no failures".to_string();
    }
    let mut sorted: Vec<&CaseFailure> = failures.iter().collect();
    sorted.sort_by_cached_key(|failure| failure.sort_key());

    let rows: Vec<(String, String)> = sorted
        .iter()
        .map(|failure| (failure.subject.to_string(), failure.reason.to_string()))
        .collect();
    let width = rows
        .iter()
        .map(|(subject, _)| subject.len())
        .max()
        .unwrap_or_default()
        .max(SUBJECT_HEADER.len());

    let mut lines = vec![
        format!("{} unpack corpus failure(s):", rows.len()),
        format!("| {SUBJECT_HEADER:<width$} | Failure"),
        format!("|-{}-|--------", "-".repeat(width)),
    ];
    lines.extend(
        rows.iter()
            .map(|(subject, reason)| format!("| {subject:<width$} | {reason}")),
    );
    lines.join("\n")
}

/// Renders every note that asks the user to act (unexplained divergences,
/// pending reviews, expected failures), sorted by file. Returns an empty
/// string when there is nothing to do.
#[must_use]
pub fn render_action_items(reports: &[CaseReport]) -> String {
    let mut items: Vec<(String, String)> = reports
        .iter()
        .flat_map(|report| {
            report
                .notes
                .iter()
                .filter(|note| note.is_action_item())
                .map(|note| (report.file.clone(), note.to_string()))
        })
        .collect();
    if items.is_empty() {
        return String::new();
    }
    items.sort();
    let mut lines = vec![format!("{} user action item(s):", items.len())];
    lines.extend(items.iter().map(|(file, note)| format!("- {file}: {note}")));
    lines.join("\n")
}
