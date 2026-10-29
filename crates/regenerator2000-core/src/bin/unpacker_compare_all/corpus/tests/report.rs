//! Tests for `report` (T1.A1.57) and the action-item list.

use super::*;

/// T1.A1.57
#[test]
fn failure_table_lists_every_failure_sorted() {
    let failures = vec![
        CaseFailure {
            subject: Subject::Group("mule".to_string()),
            reason: FailureReason::ConsensusMismatch {
                property: ConsensusProperty::Range,
                values: vec![("x.prg".to_string(), "$0800-$0900".to_string())],
            },
        },
        CaseFailure {
            subject: Subject::Case("b.prg".to_string()),
            reason: FailureReason::UnsignedDivergence,
        },
        CaseFailure {
            subject: Subject::Case("a.prg".to_string()),
            reason: FailureReason::MissingOverlap,
        },
        CaseFailure {
            subject: Subject::Case("a.prg".to_string()),
            reason: FailureReason::FieldMismatch {
                field: Field::End,
                unp64: "$FFFF".to_string(),
                r2000: HexAddr(0x2000),
            },
        },
    ];
    let table = render_failure_table(&failures);
    let lines: Vec<&str> = table.lines().collect();
    assert!(
        lines[0].starts_with("4 unpack corpus failure(s)"),
        "{table}"
    );
    let rows = &lines[3..];
    assert_eq!(rows.len(), 4, "{table}");
    let expected_order = [
        ("a.prg", "`end` differs"),
        ("a.prg", "no `overlap`"),
        ("b.prg", "proposed, unsigned"),
        ("group `mule`", "disagree on range"),
    ];
    for (row, (subject, needle)) in rows.iter().zip(expected_order) {
        assert!(
            row.contains(subject) && row.contains(needle),
            "row {row:?} in:\n{table}"
        );
    }
    assert_eq!(render_failure_table(&[]), "no failures");
}

/// Only action-item notes are listed, sorted by file; informational notes
/// and note-free reports produce nothing.
#[test]
fn action_items_list_only_actionable_notes_sorted() {
    let reports = [
        CaseReport {
            file: "z.prg".to_string(),
            oracle: Oracle::Unp64Reference,
            notes: vec![ReportNote::PendingReview],
        },
        CaseReport {
            file: "g.prg".to_string(),
            oracle: Oracle::GroundTruth {
                group: "mule".to_string(),
            },
            notes: vec![ReportNote::Unp64DisagreesWithGroundTruth {
                fields: vec![Field::Start],
                payload: false,
            }],
        },
        CaseReport {
            file: "a.prg".to_string(),
            oracle: Oracle::AcceptedDivergence {
                kind: DivergenceKind::Unexplained,
            },
            notes: vec![ReportNote::Unexplained { note: None }],
        },
    ];
    let items = render_action_items(&reports);
    let lines: Vec<&str> = items.lines().collect();
    assert_eq!(lines.len(), 3, "{items}");
    assert_eq!(lines[0], "2 user action item(s):");
    assert!(lines[1].starts_with("- a.prg: unexplained"), "{items}");
    assert!(lines[2].starts_with("- z.prg: exception signed"), "{items}");
    assert!(!items.contains("g.prg"), "{items}");

    assert_eq!(render_action_items(&reports[1..2]), "");
    assert_eq!(render_action_items(&[]), "");
}
