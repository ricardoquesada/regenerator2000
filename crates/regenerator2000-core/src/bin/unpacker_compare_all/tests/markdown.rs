//! Tests for `--markdown` (T1.A2.41) and the divergence labels.

use super::*;
use crate::corpus::{Divergence, ExpectedFailReason, INITIAL_REVIEW_MARKER};
use crate::markdown::{divergence_label, render_markdown};

/// Three observations covering agreement, a pending exception and ground
/// truth.
fn observations() -> Vec<Observation> {
    let mut pending = bare_case("b_traveller.prg");
    pending.accepted_divergence = Some(exception(
        DivergenceKind::Unp64DefaultEnd,
        &[Field::End],
        Some(INITIAL_REVIEW_MARKER),
    ));
    let mut member = bare_case("c_mule.dali.prg");
    member.group = Some("mule".to_string());
    member.expected_fail = Some(ExpectedFailReason::PaddingConvention);
    vec![
        observation(
            pending,
            ok_result("b_traveller.prg", image(0x0801, 0x7949, 0x0911)),
            saved(0x0801, 0xFFFD, Unp64Entry::Address(0x0911)),
        ),
        observation(
            bare_case("a_agree.prg"),
            ok_result("a_agree.prg", image(0x0801, 0x2000, 0x080D)),
            saved(0x0801, 0x2000, Unp64Entry::Address(0x080D)),
        ),
        observation(
            member,
            err_result("c_mule.dali.prg", "Phase2Timeout"),
            failed("Max Iterations | quitting"),
        ),
    ]
}

/// T1.A2.41: one row per case, sorted by file, the same text for any input
/// order, and a summary per divergence category.
#[test]
fn markdown_report_has_one_row_per_case_and_is_deterministic() {
    let forward = observations();
    let mut backward = observations();
    backward.reverse();
    let report = render_markdown(&forward, VERSION);
    assert_eq!(report, render_markdown(&backward, VERSION));

    let rows: Vec<&str> = report
        .lines()
        .filter(|line| line.starts_with("| `"))
        .collect();
    assert_eq!(rows.len(), 3, "{report}");
    assert!(rows[0].starts_with("| `a_agree.prg` |"), "{report}");
    assert!(rows[1].starts_with("| `b_traveller.prg` |"), "{report}");
    assert!(rows[2].starts_with("| `c_mule.dali.prg` |"), "{report}");

    assert!(rows[0].contains("$0801-$2000 → $080D"));
    assert!(rows[0].contains("| agree |"));
    assert!(rows[1].contains("unp64_default_end"));
    assert!(rows[1].contains("pending user review"));
    assert!(rows[2].contains("ground truth `mule`, expected_fail padding_convention"));
    assert!(rows[2].contains("error: Phase2Timeout"));
    // A `|` inside a cell is escaped so it cannot break the table.
    assert!(rows[2].contains("Max Iterations \\| quitting"), "{report}");

    assert!(report.contains("unp64 v2.42"));
    assert!(report.contains("| agree | 1 |"));
    assert!(report.contains("| unp64_default_end | 1 |"));
    assert!(report.contains("| one_side_failed | 1 |"));

    // Every pending exception and expected_fail marker is a user action item.
    let items = report
        .split("## User action items")
        .nth(1)
        .unwrap_or_default();
    assert!(
        items.contains("`b_traveller.prg`: accepted_divergence `unp64_default_end` [end] carries the initial marker"),
        "{items}"
    );
    assert!(
        items.contains("`c_mule.dali.prg`: expected_fail `padding_convention`"),
        "{items}"
    );
    assert!(!items.contains("a_agree.prg"), "{items}");
}

/// Labels carry the details a reviewer needs.
#[test]
fn divergence_labels_include_details() {
    assert_eq!(
        divergence_label(&Divergence::PayloadMismatch {
            first: 0xD000,
            count: 3
        }),
        "payload_mismatch (3 byte(s), first at $D000)"
    );
    assert_eq!(
        divergence_label(&Divergence::OneSideFailed {
            r2000: None,
            unp64: Some("Max Iterations".to_string())
        }),
        "one_side_failed (R2000: ok; unp64: Max Iterations)"
    );
    assert_eq!(
        divergence_label(&Divergence::EntryMismatch),
        "entry_mismatch"
    );
}

/// T4.1.12: `render_markdown` renders the `start_source`, `end_source`, `entry_source`,
/// and `exit_reason` provenance strings from `UnpackReport`.
#[test]
fn compare_all_renders_report_provenance() {
    let mut img = image(0x0801, 0x2000, 0x080D);
    img.report = UnpackReport {
        exit_reason: ExitReason::KnownEntryReached { pc: 0x080D },
        find_depacker_instructions: 120,
        decompress_instructions: 4_880,
        start_source: BoundSource::PackerSpec { id: "dali" },
        end_source: BoundSource::PointerAfterRun { addr: 0x00AE },
        entry_source: BoundSource::ExitPc,
        trace_tail: Vec::new(),
    };

    let obs = observation(
        bare_case("provenance.dali.prg"),
        ok_result("provenance.dali.prg", img),
        saved(0x0801, 0x2000, Unp64Entry::Address(0x080D)),
    );
    let report = render_markdown(&[obs], VERSION);

    assert!(
        report.contains(&BoundSource::PackerSpec { id: "dali" }.to_string()),
        "{report}"
    );
    assert!(
        report.contains(&BoundSource::PointerAfterRun { addr: 0x00AE }.to_string()),
        "{report}"
    );
    assert!(
        report.contains(&BoundSource::ExitPc.to_string()),
        "{report}"
    );
    assert!(
        report.contains(&ExitReason::KnownEntryReached { pc: 0x080D }.to_string()),
        "{report}"
    );
}
