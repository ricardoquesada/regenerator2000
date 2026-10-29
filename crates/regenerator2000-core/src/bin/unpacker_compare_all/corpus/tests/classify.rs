//! Tests for `classify` (T1.A2.20–T1.A2.32). They ship with the classifier
//! in M1.1 because the ledger places the classifier in M1.1 and its ground
//! rules bundle tests with the step that implements the logic (Option A).

use super::*;

/// unp64 saved `start..=end` from the shared memory.
fn observed(start: u16, end: u16, entry: Unp64Entry) -> Unp64Observation {
    Unp64Observation::Saved {
        start,
        entry,
        payload: mem_bytes(start, end),
    }
}

/// Classifies a successful C64 result loaded at `$0801`.
fn classify_image(image: UnpackedImage, unp64: &Unp64Observation) -> Divergence {
    classify(&ok_result("a.prg", image), unp64)
}

/// T1.A2.20
#[test]
fn classify_agree() {
    let unp64 = observed(0x0801, 0x2000, Unp64Entry::Address(0x080D));
    assert_eq!(
        classify_image(image(0x0801, 0x2000, 0x080D), &unp64),
        Divergence::Agree
    );
}

/// T1.A2.21 (tightened by ruling R16): only the BASIC link byte pair
/// `{basic_start - 1, basic_start}` is a padding byte, in either order. The
/// former `$0802` vs `$0801` row is not the pair and now classifies as
/// `Unexplained`.
#[test]
fn classify_padding_byte_both_directions() {
    // (label, system, load, unp64 start, R2000 start, is padding)
    let rows = [
        (
            "C64 unp64 $0800, R2000 $0801",
            System::C64,
            0x0801,
            0x0800,
            0x0801,
            true,
        ),
        (
            "C64 unp64 $0801, R2000 $0800",
            System::C64,
            0x0801,
            0x0801,
            0x0800,
            true,
        ),
        ("C64 load $0800", System::C64, 0x0800, 0x0800, 0x0801, true),
        (
            "Card Cruncher load $07C0",
            System::C64,
            0x07C0,
            0x07C0,
            0x07C1,
            true,
        ),
        (
            "C128 default BASIC start",
            System::C128,
            0x1C01,
            0x1C00,
            0x1C01,
            true,
        ),
        (
            "C64 $0802 vs $0801",
            System::C64,
            0x0801,
            0x0802,
            0x0801,
            false,
        ),
        (
            "C64 $1000 vs $1001",
            System::C64,
            0x0801,
            0x1000,
            0x1001,
            false,
        ),
        ("C64 delta 2", System::C64, 0x0801, 0x0803, 0x0801, false),
    ];
    let mut failures = Vec::new();
    for (label, system, load_addr, unp64_start, r2000_start, padding) in rows {
        let result = CaseResult {
            load_addr,
            system,
            ..ok_result("a.prg", image(r2000_start, 0x2000, 0x1C10))
        };
        let unp64 = observed(unp64_start, 0x2000, Unp64Entry::Address(0x1C10));
        let expected = if padding {
            Divergence::PaddingByte
        } else {
            Divergence::Unexplained
        };
        let actual = classify(&result, &unp64);
        if actual != expected {
            failures.push(format!("{label}: expected {expected:?}, got {actual:?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// R16 follow-up (M2.1): `padding_byte` keeps both anchors. The engine's
/// BASIC start comes from the shared `unpacker::machine::get_basic_start`
/// (no copy), and the system's default BASIC start is the other anchor.
#[test]
fn padding_byte_anchors_on_engine_and_default_basic_start() {
    use super::super::kinds::{padding_byte_holds, padding_byte_pairs};

    // (system, load, start a, start b, holds)
    let rows = [
        // Default anchor only (C64 `$0801`, C128 `$1C01`).
        (System::C64, 0x1000, 0x0800, 0x0801, true),
        (System::C128, 0x1C00, 0x1C00, 0x1C01, true),
        // Engine anchor only (load-derived; Card Cruncher).
        (System::C64, 0x1000, 0x0FFF, 0x1000, true),
        (System::C64, 0x07C0, 0x07C0, 0x07C1, true),
        // Both anchors coincide.
        (System::C64, 0x0800, 0x0800, 0x0801, true),
        // Neither anchor.
        (System::C64, 0x0801, 0x0802, 0x0801, false),
        (System::C64, 0x0801, 0x1000, 0x1001, false),
        (System::C64, 0x1000, 0x1000, 0x1001, false),
    ];
    for (system, load, a, b, holds) in rows {
        for (x, y) in [(a, b), (b, a)] {
            assert_eq!(
                padding_byte_holds(x, y, load, &system),
                holds,
                "{system} load ${load:04X}: ${x:04X} vs ${y:04X}"
            );
        }
    }
    assert_eq!(padding_byte_pairs(0x0801, &System::C64), "$0800/$0801");
    assert_eq!(
        padding_byte_pairs(0x1000, &System::C64),
        "$0800/$0801 or $0FFF/$1000"
    );
    assert_eq!(
        padding_byte_pairs(0x07C0, &System::C64),
        "$07C0/$07C1 or $0800/$0801"
    );
}

/// T1.A2.22
#[test]
fn classify_padding_byte_requires_matching_intersection() {
    let mut r2000 = image(0x0801, 0x2000, 0x080D);
    r2000.data[5] ^= 0xFF;
    let unp64 = observed(0x0800, 0x2000, Unp64Entry::Address(0x080D));
    assert_eq!(
        classify_image(r2000, &unp64),
        Divergence::PayloadMismatch {
            first: 0x0806,
            count: 1
        }
    );
}

/// T1.A2.23
#[test]
fn classify_unp64_default_end() {
    let rows = [
        (0xFFFD, true),
        (0xFFFF, true),
        (0xFF00, true),
        (0xFEFF, false),
    ];
    for (unp64_end, holds) in rows {
        let unp64 = observed(0x0801, unp64_end, Unp64Entry::Address(0x0911));
        let expected = if holds {
            Divergence::Unp64DefaultEnd
        } else {
            Divergence::Unexplained
        };
        assert_eq!(
            classify_image(image(0x0801, 0x7949, 0x0911), &unp64),
            expected,
            "unp64 end ${unp64_end:04X}"
        );
    }
}

/// T1.A2.24
#[test]
fn classify_unp64_default_start() {
    let unp64 = observed(0x0800, 0x2000, Unp64Entry::Address(0x0810));
    assert_eq!(
        classify_image(image(0x0810, 0x2000, 0x0810), &unp64),
        Divergence::Unp64DefaultStart
    );
    let c128 = CaseResult {
        file: "a.prg".to_string(),
        load_addr: 0x1C01,
        system: System::C128,
        outcome: Ok(image(0x1C10, 0x3000, 0x1C10)),
    };
    assert_eq!(
        classify(
            &c128,
            &observed(0x1C00, 0x3000, Unp64Entry::Address(0x1C10))
        ),
        Divergence::Unp64DefaultStart
    );
}

/// T1.A2.25
#[test]
fn classify_unexplained_range_difference() {
    let start_off = observed(0x0400, 0x2000, Unp64Entry::Address(0x0810));
    assert_eq!(
        classify_image(image(0x0810, 0x2000, 0x0810), &start_off),
        Divergence::Unexplained
    );
    let both_off = observed(0x0800, 0xFFFF, Unp64Entry::Address(0x0810));
    assert_eq!(
        classify_image(image(0x0801, 0x2000, 0x0810), &both_off),
        Divergence::Unexplained
    );
}

/// T1.A2.26
#[test]
fn classify_payload_mismatch_first_and_count() {
    let mut r2000 = image(0x0801, 0x2000, 0x080D);
    for offset in [0x10, 0x11, 0x400] {
        r2000.data[offset] ^= 0x55;
    }
    let unp64 = observed(0x0801, 0x2000, Unp64Entry::Address(0x080D));
    assert_eq!(
        classify_image(r2000, &unp64),
        Divergence::PayloadMismatch {
            first: 0x0811,
            count: 3
        }
    );
}

/// T1.A2.27
#[test]
fn classify_unp64_missing_opcode() {
    let unp64 = Unp64Observation::Failed {
        reason: "illegal opcode $93".to_string(),
        missing_opcode: true,
    };
    let divergence = classify_image(image(0x0801, 0x2000, 0x080D), &unp64);
    assert_eq!(divergence, Divergence::Unp64MissingOpcode);
    assert_eq!(
        divergence.accepted_kind(),
        Some(DivergenceKind::Unp64MissingOpcode)
    );
}

/// T1.A2.28
#[test]
fn classify_entry_mismatch() {
    let unp64 = observed(0x0801, 0x2000, Unp64Entry::Address(0x0900));
    let divergence = classify_image(image(0x0801, 0x2000, 0x080D), &unp64);
    assert_eq!(divergence, Divergence::EntryMismatch);
    assert_eq!(divergence.accepted_kind(), None);
}

/// T1.A2.29
#[test]
fn classify_suspicious_beats_benign() {
    let padded_other_entry = observed(0x0800, 0x2000, Unp64Entry::Address(0x0900));
    assert_eq!(
        classify_image(image(0x0801, 0x2000, 0x080D), &padded_other_entry),
        Divergence::EntryMismatch
    );
    let mut corrupt = image(0x0801, 0x2000, 0x080D);
    corrupt.data[0] ^= 1;
    let padded = observed(0x0800, 0x2000, Unp64Entry::Address(0x0900));
    assert!(matches!(
        classify_image(corrupt, &padded),
        Divergence::PayloadMismatch { first: 0x0801, .. }
    ));
}

/// T1.A2.30
#[test]
fn classify_disjoint_ranges() {
    let unp64 = observed(0x4000, 0x5000, Unp64Entry::Address(0x080D));
    assert_eq!(
        classify_image(image(0x0801, 0x2000, 0x080D), &unp64),
        Divergence::Unexplained
    );
}

/// T1.A2.31
#[test]
fn classify_one_side_failed() {
    let unp64_failed = Unp64Observation::Failed {
        reason: "Max Iterations".to_string(),
        missing_opcode: false,
    };
    let unp64_saved = observed(0x0801, 0x2000, Unp64Entry::Address(0x080D));

    let only_unp64 = classify_image(image(0x0801, 0x2000, 0x080D), &unp64_failed);
    assert_eq!(
        only_unp64,
        Divergence::OneSideFailed {
            r2000: None,
            unp64: Some("Max Iterations".to_string())
        }
    );
    assert_eq!(
        only_unp64.accepted_kind(),
        Some(DivergenceKind::Unexplained)
    );

    let only_r2000 = classify(&err_result("a.prg", "Phase2Timeout"), &unp64_saved);
    assert_eq!(
        only_r2000,
        Divergence::OneSideFailed {
            r2000: Some("Phase2Timeout".to_string()),
            unp64: None
        }
    );
    assert_eq!(only_r2000.accepted_kind(), None);

    assert_eq!(
        classify(&err_result("a.prg", "Phase2Timeout"), &unp64_failed),
        Divergence::OneSideFailed {
            r2000: Some("Phase2Timeout".to_string()),
            unp64: Some("Max Iterations".to_string())
        }
    );
}

/// T1.A2.32
#[test]
fn classify_unp64_basic_run_entry_is_not_entry_mismatch() {
    let unp64 = observed(0x0801, 0x2000, Unp64Entry::BasicRun);
    let divergence = classify_image(image(0x0801, 0x2000, 0x080D), &unp64);
    assert_eq!(divergence, Divergence::Unexplained);
    assert_ne!(divergence, Divergence::EntryMismatch);
}

/// The classifier's benign kinds map onto the manifest's closed enum, and
/// the suspicious ones map to nothing.
#[test]
fn divergence_accepted_kind_mapping() {
    let rows = [
        (Divergence::Agree, None),
        (Divergence::PaddingByte, Some(DivergenceKind::PaddingByte)),
        (
            Divergence::Unp64DefaultEnd,
            Some(DivergenceKind::Unp64DefaultEnd),
        ),
        (
            Divergence::Unp64DefaultStart,
            Some(DivergenceKind::Unp64DefaultStart),
        ),
        (Divergence::Unexplained, Some(DivergenceKind::Unexplained)),
        (Divergence::PayloadMismatch { first: 0, count: 1 }, None),
        (Divergence::EntryMismatch, None),
    ];
    for (divergence, expected) in rows {
        assert_eq!(divergence.accepted_kind(), expected, "{divergence:?}");
    }
}
