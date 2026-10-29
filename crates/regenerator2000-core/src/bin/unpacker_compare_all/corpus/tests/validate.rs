//! Tests for `validate` (T1.A1.18–T1.A1.27) and the divergence-shape rules.

use super::*;

/// T1.A1.18
#[test]
fn validate_missing_fixture_is_failure() {
    let corpus = corpus(vec![simple_case("a.prg"), simple_case("ghost.prg")]);
    let errors = corpus.validate(&MemFixtures::named(&["a.prg"]));
    assert_eq!(
        errors,
        vec![CorpusError::MissingFixture {
            file: "ghost.prg".to_string()
        }]
    );
}

/// T1.A1.19
#[test]
fn validate_missing_group_original_is_failure() {
    let mut member = simple_case("a.prg");
    member.group = Some("mule".to_string());
    let corpus = Corpus {
        groups: vec![mule_group()],
        cases: vec![member],
    };
    let errors = corpus.validate(&MemFixtures::named(&["a.prg"]));
    assert_eq!(
        errors,
        vec![CorpusError::MissingGroupOriginal {
            group: "mule".to_string(),
            file: "mule_orig.prg".to_string()
        }]
    );
}

/// T1.A1.20
#[test]
fn validate_unknown_group_reference_is_failure() {
    let mut member = simple_case("a.prg");
    member.group = Some("nope".to_string());
    let errors = corpus(vec![member]).validate(&MemFixtures::named(&["a.prg"]));
    assert_eq!(
        errors,
        vec![CorpusError::UnknownGroup {
            file: "a.prg".to_string(),
            group: "nope".to_string()
        }]
    );
}

/// T1.A1.21
#[test]
fn validate_duplicate_case_file_is_failure() {
    let errors = corpus(vec![simple_case("a.prg"), simple_case("a.prg")])
        .validate(&MemFixtures::named(&["a.prg"]));
    assert_eq!(
        errors,
        vec![CorpusError::DuplicateCase {
            file: "a.prg".to_string()
        }]
    );
}

/// T1.A1.22
#[test]
fn validate_duplicate_group_id_is_failure() {
    let corpus = Corpus {
        groups: vec![mule_group(), mule_group()],
        cases: Vec::new(),
    };
    let errors = corpus.validate(&MemFixtures::named(&["mule_orig.prg"]));
    assert_eq!(
        errors,
        vec![CorpusError::DuplicateGroup {
            group: "mule".to_string()
        }]
    );
}

/// T1.A1.23
#[test]
fn validate_case_without_unp64_table_is_failure() {
    let mut plain = simple_case("a.prg");
    plain.unp64 = None;
    let mut grouped = simple_case("b.prg");
    grouped.unp64 = None;
    grouped.group = Some("mule".to_string());
    let corpus = Corpus {
        groups: vec![mule_group()],
        cases: vec![plain, grouped],
    };
    let errors = corpus.validate(&MemFixtures::named(&["a.prg", "b.prg", "mule_orig.prg"]));
    assert_eq!(
        errors,
        vec![
            CorpusError::MissingUnp64 {
                file: "a.prg".to_string()
            },
            CorpusError::MissingUnp64 {
                file: "b.prg".to_string()
            },
        ]
    );
}

/// T1.A1.24
#[test]
fn validate_unp64_failed_requires_unexplained_divergence() {
    let failed_case = |divergence: Option<AcceptedDivergence>| {
        let mut case = simple_case("a.prg");
        case.unp64 = Some(failed_record("Max Iterations"));
        case.accepted_divergence = divergence;
        case
    };
    let fixtures = MemFixtures::named(&["a.prg", "mule_orig.prg"]);
    let errors_for = |case: Case| corpus(vec![case]).validate(&fixtures);
    let expected_error = vec![CorpusError::FailedUnp64WithoutDivergence {
        file: "a.prg".to_string(),
    }];

    assert_eq!(errors_for(failed_case(None)), expected_error);
    assert_eq!(
        errors_for(failed_case(Some(signed(
            DivergenceKind::Unexplained,
            &Field::ALL
        )))),
        Vec::new()
    );
    assert_eq!(
        errors_for(failed_case(Some(signed(
            DivergenceKind::Unp64MissingOpcode,
            &Field::ALL
        )))),
        Vec::new()
    );
    let wrong_kind = errors_for(failed_case(Some(signed(
        DivergenceKind::PaddingByte,
        &[Field::Start],
    ))));
    assert!(wrong_kind.contains(&expected_error[0]), "{wrong_kind:?}");

    // Inside a ground-truth group the original decides; no exception needed.
    let mut grouped = failed_case(None);
    grouped.group = Some("mule".to_string());
    let corpus = Corpus {
        groups: vec![mule_group()],
        cases: vec![grouped],
    };
    assert_eq!(corpus.validate(&fixtures), Vec::new());
}

/// T1.A1.25
#[test]
fn validate_unp64_len_consistent_with_range() {
    // (start, end, recorded len, expected `derived` in the error, if any)
    let rows: [(u16, u16, u32, Option<Option<usize>>); 6] = [
        (0x0801, 0x0810, 16, None),
        (0x0800, 0xFFFF, 63_488, None),
        (0x0000, 0xFFFF, 65_536, None),
        (0x0801, 0x0810, 15, Some(Some(16))),
        (0x0800, 0xFFFF, 63_487, Some(Some(63_488))),
        (0x0810, 0x0801, 16, Some(None)),
    ];
    let mut failures = Vec::new();
    for (start, end, len, expected_derived) in rows {
        let mut saved = simple_record();
        saved.start = HexAddr(start);
        saved.end = HexAddr(end);
        saved.len = len;
        let errors =
            corpus(vec![case("a.prg", saved, None)]).validate(&MemFixtures::named(&["a.prg"]));
        let expected: Vec<CorpusError> = expected_derived
            .map(|derived| CorpusError::Unp64LenMismatch {
                file: "a.prg".to_string(),
                recorded: len,
                derived,
            })
            .into_iter()
            .collect();
        if errors != expected {
            failures.push(format!("${start:04X}-${end:04X} len {len}: {errors:?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// T1.A1.26
#[test]
fn validate_overlap_lies_inside_unp64_range() {
    let with_overlap = |start: u16, end: u16| {
        let mut saved = simple_record();
        saved.overlap = Some(Overlap {
            start: HexAddr(start),
            end: HexAddr(end),
            fnv64: Fnv64(0),
        });
        corpus(vec![case("a.prg", saved, None)]).validate(&MemFixtures::named(&["a.prg"]))
    };
    assert_eq!(with_overlap(0x0801, 0x0810), Vec::new());
    assert_eq!(with_overlap(0x0805, 0x0806), Vec::new());
    for (start, end) in [(0x0800, 0x0810), (0x0801, 0x0811), (0x0806, 0x0805)] {
        assert_eq!(
            with_overlap(start, end),
            vec![CorpusError::OverlapOutsideUnp64Range {
                file: "a.prg".to_string(),
                overlap: (HexAddr(start), HexAddr(end)),
                unp64: (HexAddr(0x0801), HexAddr(0x0810)),
            }],
            "${start:04X}-${end:04X}"
        );
    }
}

/// T1.A1.27
#[test]
fn validate_all_errors_are_reported_together() {
    let mut no_unp64 = simple_case("a.prg");
    no_unp64.unp64 = None;
    let mut unknown_group = simple_case("c.prg");
    unknown_group.group = Some("nope".to_string());
    // Deliberately unsorted input order.
    let corpus = corpus(vec![unknown_group, simple_case("b.prg"), no_unp64]);
    let errors = corpus.validate(&MemFixtures::named(&["c.prg"]));
    assert_eq!(
        errors,
        vec![
            CorpusError::MissingFixture {
                file: "a.prg".to_string()
            },
            CorpusError::MissingUnp64 {
                file: "a.prg".to_string()
            },
            CorpusError::MissingFixture {
                file: "b.prg".to_string()
            },
            CorpusError::UnknownGroup {
                file: "c.prg".to_string(),
                group: "nope".to_string()
            },
        ]
    );
}

/// Validation rules that tie a divergence `kind` to its `fields`, keep
/// exceptions off ground-truth cases, and reject path-like names.
#[test]
fn validate_divergence_shape_rules() {
    let fixtures = MemFixtures::named(&["a.prg", "mule_orig.prg"]);

    let wrong_fields = corpus(vec![case(
        "a.prg",
        simple_record(),
        Some(signed(DivergenceKind::PaddingByte, &[Field::End])),
    )]);
    assert_eq!(
        wrong_fields.validate(&fixtures),
        vec![CorpusError::DivergenceFieldsMismatch {
            file: "a.prg".to_string(),
            kind: DivergenceKind::PaddingByte,
            required: &[Field::Start],
        }]
    );

    let opcode_on_saved = corpus(vec![case(
        "a.prg",
        simple_record(),
        Some(signed(DivergenceKind::Unp64MissingOpcode, &Field::ALL)),
    )]);
    assert_eq!(
        opcode_on_saved.validate(&fixtures),
        vec![CorpusError::DivergenceKindNeedsFailedUnp64 {
            file: "a.prg".to_string(),
            kind: DivergenceKind::Unp64MissingOpcode,
        }]
    );

    let mut failed_partial = simple_case("a.prg");
    failed_partial.unp64 = Some(failed_record("quit"));
    failed_partial.accepted_divergence = Some(signed(DivergenceKind::Unexplained, &[Field::End]));
    assert_eq!(
        corpus(vec![failed_partial]).validate(&fixtures),
        vec![CorpusError::DivergenceFieldsMismatch {
            file: "a.prg".to_string(),
            kind: DivergenceKind::Unexplained,
            required: &Field::ALL,
        }]
    );

    let mut grouped = simple_case("a.prg");
    grouped.group = Some("mule".to_string());
    grouped.accepted_divergence = Some(signed(DivergenceKind::Unexplained, &[Field::End]));
    let grouped = Corpus {
        groups: vec![mule_group()],
        cases: vec![grouped],
    };
    assert_eq!(
        grouped.validate(&fixtures),
        vec![CorpusError::DivergenceOnGroundTruthCase {
            file: "a.prg".to_string(),
            group: "mule".to_string(),
        }]
    );

    for bad in ["../a.prg", "dir/a.prg", "dir\\a.prg", "..", ""] {
        let errors = corpus(vec![simple_case(bad)]).validate(&fixtures);
        assert_eq!(
            errors,
            vec![CorpusError::InvalidFileName {
                file: bad.to_string()
            }],
            "{bad:?}"
        );
    }
}

/// Only errors that break the case list are structural; errors in the
/// recorded oracle data (which `--update` rewrites) are not.
#[test]
fn structural_errors_are_the_case_list_errors() {
    let file = || "a.prg".to_string();
    let structural = [
        CorpusError::MissingFixture { file: file() },
        CorpusError::DuplicateCase { file: file() },
        CorpusError::UnknownGroup {
            file: file(),
            group: "g".to_string(),
        },
        CorpusError::InvalidFixture {
            file: file(),
            reason: "short".to_string(),
        },
    ];
    let oracle_data = [
        CorpusError::MissingUnp64 { file: file() },
        CorpusError::FailedUnp64WithoutDivergence { file: file() },
        CorpusError::DivergenceOnGroundTruthCase {
            file: file(),
            group: "g".to_string(),
        },
    ];
    let wrong: Vec<String> = structural
        .iter()
        .filter(|error| !error.is_structural())
        .chain(oracle_data.iter().filter(|error| error.is_structural()))
        .map(ToString::to_string)
        .collect();
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
