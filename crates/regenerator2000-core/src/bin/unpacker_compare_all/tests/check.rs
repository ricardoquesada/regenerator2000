//! Tests for `--check`.

use super::*;
use crate::check::{DriftField, check_all, check_case};
use crate::cli::ToolStatus;
use crate::update::update_case;

/// Builds a case with a record from `--update` for this observation.
fn recorded(file: &str, r2000: UnpackedImage, unp64: Unp64Outcome) -> Case {
    update_case(
        &observation(bare_case(file), ok_result(file, r2000), unp64),
        VERSION,
    )
    .case
}

/// Checks that `--check` reports each drifting value once with both values,
/// and hashes the overlap only over the recorded window.
#[test]
fn check_reports_drift_per_field() {
    let case = recorded(
        "t.prg",
        image(0x0801, 0x7949, 0x0911),
        saved(0x0801, 0xFFFD, Unp64Entry::Address(0x0911)),
    );

    // Identical `unp64` output reproduces the record.
    assert_eq!(
        check_case(
            &case,
            &saved(0x0801, 0xFFFD, Unp64Entry::Address(0x0911)),
            VERSION
        ),
        vec![]
    );

    // A new binary that saves to $FFFF with another entry.
    let drifts = check_case(
        &case,
        &saved(0x0801, 0xFFFF, Unp64Entry::Address(0x0912)),
        "2.43",
    );
    let fields: Vec<DriftField> = drifts.iter().map(|drift| drift.field).collect();
    assert_eq!(
        fields,
        [
            DriftField::End,
            DriftField::Entry,
            DriftField::Len,
            DriftField::PayloadFnv64,
            DriftField::Version
        ]
    );
    let end = &drifts[0];
    assert_eq!(
        (end.recorded.as_str(), end.fresh.as_str()),
        ("$FFFD", "$FFFF")
    );
    // The overlap over the recorded window does not change, so it does not drift.
    assert!(!fields.contains(&DriftField::Overlap));

    // Saved versus failed produces one whole-record drift.
    let outcome = check_case(&case, &failed("Max Iterations"), VERSION);
    assert_eq!(outcome.len(), 1);
    assert_eq!(outcome[0].field, DriftField::Outcome);
    assert!(
        outcome[0].fresh.contains("Max Iterations"),
        "{:?}",
        outcome[0]
    );

    // A case without a record produces a `Record` drift.
    let missing = check_case(
        &bare_case("n.prg"),
        &saved(0x0801, 0x0810, Unp64Entry::Address(0x080D)),
        VERSION,
    );
    assert_eq!(missing.len(), 1);
    assert_eq!(missing[0].field, DriftField::Record);
}

/// Checks that any drift makes `--check` exit non-zero, and no drift exits 0.
#[test]
fn check_exit_code_is_nonzero_on_drift() {
    let case = recorded(
        "a.prg",
        image(0x0801, 0x2000, 0x080D),
        saved(0x0801, 0x2000, Unp64Entry::Address(0x080D)),
    );
    let clean = check_all(
        &[observation(
            case.clone(),
            ok_result("a.prg", image(0x0801, 0x2000, 0x080D)),
            saved(0x0801, 0x2000, Unp64Entry::Address(0x080D)),
        )],
        VERSION,
    );
    assert_eq!(clean.status(), ToolStatus::Success);
    assert_eq!(clean.status().code(), 0);
    assert!(clean.to_string().contains("all 1 recorded"));

    let drifted = check_all(
        &[observation(
            case,
            ok_result("a.prg", image(0x0801, 0x2000, 0x080D)),
            saved(0x0801, 0x2001, Unp64Entry::Address(0x080D)),
        )],
        VERSION,
    );
    assert_eq!(drifted.status(), ToolStatus::Drift);
    assert_ne!(drifted.status().code(), 0);
    assert!(drifted.to_string().contains("a.prg: end recorded $2000"));
}
