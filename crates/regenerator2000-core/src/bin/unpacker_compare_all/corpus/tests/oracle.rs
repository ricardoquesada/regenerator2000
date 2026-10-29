//! Tests for `oracle`: the unp64 reference rule, accepted divergences, ground
//! truth, consensus, `expected_fail` and stale exceptions
//! (T1.A1.28–T1.A1.53), plus edge cases and the aggregate `evaluate_all`.

use super::*;

// ─────────────────────────────── helpers ───────────────────────────────

/// Evaluates a single-field divergence where unp64 covers
/// `unp64_start..=unp64_end` and R2000 covers `r2000_start..=r2000_end`.
fn eval_range_divergence(
    kind: DivergenceKind,
    fields: &[Field],
    (unp64_start, unp64_end): (u16, u16),
    (r2000_start, r2000_end): (u16, u16),
    load_addr: u16,
    system: System,
) -> Result<CaseReport, Vec<FailureReason>> {
    let entry = r2000_start.max(0x0810).min(r2000_end);
    let image = image(r2000_start, r2000_end, entry);
    let saved = record(unp64_start, unp64_end, Unp64Entry::Address(entry), &image);
    let corpus = corpus(vec![case("a.prg", saved, Some(signed(kind, fields)))]);
    let result = CaseResult {
        file: "a.prg".to_string(),
        load_addr,
        system,
        outcome: Ok(image),
    };
    eval(&corpus, &result)
}

/// `Ok(())` for a pass, or the failure messages.
fn holds_or_reason(outcome: &Result<CaseReport, Vec<FailureReason>>) -> Result<(), String> {
    match outcome {
        Ok(_) => Ok(()),
        Err(found) => Err(failure_texts(found)),
    }
}

/// Runs a predicate table: each row must pass (`holds`) or fail with
/// "no longer holds". Returns one message per mismatching row.
fn predicate_mismatches<'a>(
    rows: impl IntoIterator<Item = (&'a str, Result<CaseReport, Vec<FailureReason>>, bool)>,
) -> Vec<String> {
    let mut mismatches = Vec::new();
    for (name, outcome, holds) in rows {
        match (holds, holds_or_reason(&outcome)) {
            (true, Ok(())) => {}
            (false, Err(text)) if text.contains("no longer holds") => {}
            (_, other) => mismatches.push(format!("{name}: {other:?}")),
        }
    }
    mismatches
}

/// Builds a one-group corpus whose original is `$0800-$0863` with entry
/// `$0810`, and the fixtures to load it.
fn ground_truth_setup(members: Vec<Case>) -> (Corpus, MemFixtures) {
    let group = Group {
        id: "prog".to_string(),
        original: "prog_orig.prg".to_string(),
        entry: HexAddr(0x0810),
    };
    let fixtures =
        MemFixtures::default().with("prog_orig.prg", prg(0x0800, &mem_bytes(0x0800, 0x0863)));
    let corpus = Corpus {
        groups: vec![group],
        cases: members,
    };
    (corpus, fixtures)
}

/// A member of the `prog` group.
fn member(file: &str, saved: Unp64Saved) -> Case {
    let mut case = case(file, saved, None);
    case.group = Some("prog".to_string());
    case
}

/// Evaluates one result against a corpus with ground truth.
fn eval_ground_truth(
    corpus: &Corpus,
    fixtures: &MemFixtures,
    result: &CaseResult,
) -> Result<CaseReport, Vec<FailureReason>> {
    Evaluator::new(corpus, fixtures)
        .unwrap()
        .evaluate(result)
        .map_err(reasons)
}

/// Runs consensus over `prog` group members and their images.
fn consensus_of(members: Vec<(Case, UnpackedImage)>) -> Vec<CaseFailure> {
    let (cases, images): (Vec<Case>, Vec<UnpackedImage>) = members.into_iter().unzip();
    let results: Vec<CaseResult> = cases
        .iter()
        .zip(images)
        .map(|(case, image)| ok_result(&case.file, image))
        .collect();
    let (corpus, fixtures) = ground_truth_setup(cases);
    Evaluator::new(&corpus, &fixtures)
        .unwrap()
        .evaluate_consensus(&results)
}

/// A `prog` member whose unp64 record matches `image`.
fn grouped_member(file: &str, image: &UnpackedImage) -> (Case, UnpackedImage) {
    (member(file, matching_record(image)), image.clone())
}

// ═══════════════════════ unp64 reference ═══════════════════════

/// T1.A1.28
#[test]
fn eval_exact_unp64_match_passes() {
    let image = image(0x0801, 0x2000, 0x080D);
    let corpus = corpus(vec![case("a.prg", matching_record(&image), None)]);
    let report = eval(&corpus, &ok_result("a.prg", image)).unwrap();
    assert_eq!(
        report,
        CaseReport {
            file: "a.prg".to_string(),
            oracle: Oracle::Unp64Reference,
            notes: Vec::new(),
        }
    );
}

/// T1.A1.29
#[test]
fn eval_each_field_mismatch_fails_without_divergence() {
    let image = image(0x0801, 0x2000, 0x080D);
    let base = matching_record(&image);
    type Mutation = fn(&mut Unp64Saved);
    let rows: [(&str, Mutation, &[&str]); 4] = [
        (
            "start",
            |s| s.start = HexAddr(0x0802),
            &["`start`", "$0802", "$0801"],
        ),
        (
            "end",
            |s| s.end = HexAddr(0x1FFF),
            &["`end`", "$1FFF", "$2000"],
        ),
        (
            "entry",
            |s| s.entry = Unp64Entry::Address(0x0810),
            &["`entry`", "$0810", "$080D"],
        ),
        (
            "payload",
            |s| s.payload_fnv64 = Fnv64(s.payload_fnv64.0 ^ 1),
            &["payload"],
        ),
    ];
    let mut failures = Vec::new();
    for (name, mutate, needles) in rows {
        let mut saved = base.clone();
        mutate(&mut saved);
        let corpus = corpus(vec![case("a.prg", saved, None)]);
        let Err(found) = eval(&corpus, &ok_result("a.prg", image.clone())) else {
            failures.push(format!("{name}: passed"));
            continue;
        };
        let text = failure_texts(&found);
        if let Some(needle) = needles.iter().find(|needle| !text.contains(*needle)) {
            failures.push(format!("{name}: missing `{needle}` in:\n{text}"));
        }
        if text.contains("`len`") {
            failures.push(format!("{name}: `len` must never be reported:\n{text}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// ═══════════════════════ accepted divergence ═══════════════════════

/// T1.A1.30
#[test]
fn eval_divergence_without_reviewed_by_fails() {
    let image = image(0x0801, 0x2000, 0x080D);
    let saved = record(0x0801, 0xFFFF, Unp64Entry::Address(0x080D), &image);
    let unsigned = divergence(DivergenceKind::Unp64DefaultEnd, &[Field::End], None);
    let corpus = corpus(vec![case("a.prg", saved, Some(unsigned))]);
    let found = eval(&corpus, &ok_result("a.prg", image)).unwrap_err();
    assert_eq!(found, vec![FailureReason::UnsignedDivergence]);
    assert!(found[0].to_string().contains("proposed, unsigned"));
}

/// T1.A1.31
#[test]
fn eval_divergence_with_blank_reviewed_by_fails() {
    let image = image(0x0801, 0x2000, 0x080D);
    let saved = record(0x0801, 0xFFFF, Unp64Entry::Address(0x080D), &image);
    let run = |reviewed_by: &str| {
        let corpus = corpus(vec![case(
            "a.prg",
            saved.clone(),
            Some(divergence(
                DivergenceKind::Unp64DefaultEnd,
                &[Field::End],
                Some(reviewed_by),
            )),
        )]);
        eval(&corpus, &ok_result("a.prg", image.clone()))
    };
    assert_eq!(run(""), Err(vec![FailureReason::UnsignedDivergence]));
    assert_eq!(run("  \t "), Err(vec![FailureReason::UnsignedDivergence]));

    let pending = run(INITIAL_REVIEW_MARKER).unwrap();
    assert_eq!(pending.notes, vec![ReportNote::PendingReview]);
    let items = render_action_items(&[pending]);
    assert!(
        items.contains("a.prg") && items.contains(INITIAL_REVIEW_MARKER),
        "{items}"
    );

    assert_eq!(run("ricardoq").unwrap().notes, Vec::new());
}

/// T1.A1.32
#[test]
fn eval_divergence_permits_only_listed_fields() {
    let image = image(0x0801, 0x2000, 0x080D);
    let end_only = record(0x0801, 0xFFFF, Unp64Entry::Address(0x080D), &image);
    let corpus_end = corpus(vec![case(
        "a.prg",
        end_only,
        Some(signed(DivergenceKind::Unp64DefaultEnd, &[Field::End])),
    )]);
    let report = eval(&corpus_end, &ok_result("a.prg", image.clone())).unwrap();
    assert_eq!(
        report.oracle,
        Oracle::AcceptedDivergence {
            kind: DivergenceKind::Unp64DefaultEnd
        }
    );

    let end_and_start = record(0x0800, 0xFFFF, Unp64Entry::Address(0x080D), &image);
    let corpus_both = corpus(vec![case(
        "a.prg",
        end_and_start,
        Some(signed(DivergenceKind::Unp64DefaultEnd, &[Field::End])),
    )]);
    let found = eval(&corpus_both, &ok_result("a.prg", image)).unwrap_err();
    assert_eq!(
        found,
        vec![FailureReason::FieldMismatch {
            field: Field::Start,
            unp64: "$0800".to_string(),
            r2000: HexAddr(0x0801),
        }]
    );
}

/// T1.A1.33
#[test]
fn eval_divergence_requires_overlap_when_range_differs() {
    let image = image(0x0801, 0x2000, 0x080D);
    let mut saved = record(0x0801, 0xFFFF, Unp64Entry::Address(0x080D), &image);
    saved.overlap = None;
    let corpus = corpus(vec![case(
        "a.prg",
        saved,
        Some(signed(DivergenceKind::Unp64DefaultEnd, &[Field::End])),
    )]);
    assert_eq!(
        eval(&corpus, &ok_result("a.prg", image)),
        Err(vec![FailureReason::MissingOverlap])
    );
}

/// T1.A1.34
#[test]
fn eval_divergence_overlap_hash_match_passes() {
    let image = image(0x0801, 0x2000, 0x080D);
    let saved = record(0x0801, 0xFFFF, Unp64Entry::Address(0x080D), &image);
    assert_eq!(
        saved.overlap.map(|o| (o.start, o.end)),
        Some((HexAddr(0x0801), HexAddr(0x2000)))
    );
    let corpus = corpus(vec![case(
        "a.prg",
        saved,
        Some(signed(DivergenceKind::Unp64DefaultEnd, &[Field::End])),
    )]);
    assert!(eval(&corpus, &ok_result("a.prg", image)).is_ok());
}

/// T1.A1.35
#[test]
fn eval_divergence_overlap_hash_mismatch_fails() {
    let image = image(0x0801, 0x2000, 0x080D);
    let mut changed = image.clone();
    changed.data[0x100] ^= 0xFF;
    let saved = record(0x0801, 0xFFFF, Unp64Entry::Address(0x080D), &image);
    let corpus = corpus(vec![case(
        "a.prg",
        saved,
        Some(signed(DivergenceKind::Unp64DefaultEnd, &[Field::End])),
    )]);
    let found = eval(&corpus, &ok_result("a.prg", changed)).unwrap_err();
    assert!(
        matches!(
            found.as_slice(),
            [FailureReason::OverlapHashMismatch { window, .. }] if *window == range(0x0801, 0x2000)
        ),
        "{found:?}"
    );
}

/// T1.A1.36
#[test]
fn eval_overlap_window_outside_r2000_range_fails() {
    let image = image(0x0801, 0x2000, 0x080D);
    let mut saved = record(0x0801, 0xFFFF, Unp64Entry::Address(0x080D), &image);
    saved.overlap = Some(Overlap {
        start: HexAddr(0x0801),
        end: HexAddr(0xA000),
        fnv64: Fnv64(0),
    });
    let corpus = corpus(vec![case(
        "a.prg",
        saved,
        Some(signed(DivergenceKind::Unp64DefaultEnd, &[Field::End])),
    )]);
    let found = eval(&corpus, &ok_result("a.prg", image)).unwrap_err();
    assert_eq!(
        found,
        vec![FailureReason::OverlapOutsideResult {
            window: range(0x0801, 0xA000),
            r2000: range(0x0801, 0x2000),
        }]
    );
}

/// A recorded overlap that is inside R2000's range but no longer equals the
/// current intersection is reported, so a stale window cannot hide bytes.
#[test]
fn eval_overlap_window_must_equal_current_intersection() {
    let image = image(0x0801, 0x2000, 0x080D);
    let mut saved = record(0x0801, 0xFFFF, Unp64Entry::Address(0x080D), &image);
    saved.overlap = Some(Overlap {
        start: HexAddr(0x0801),
        end: HexAddr(0x1000),
        fnv64: Fnv64::of(&mem_bytes(0x0801, 0x1000)),
    });
    let corpus = corpus(vec![case(
        "a.prg",
        saved,
        Some(signed(DivergenceKind::Unp64DefaultEnd, &[Field::End])),
    )]);
    assert_eq!(
        eval(&corpus, &ok_result("a.prg", image)),
        Err(vec![FailureReason::OverlapWindowMismatch {
            window: range(0x0801, 0x1000),
            intersection: Some(range(0x0801, 0x2000)),
        }])
    );
}

/// T1.A1.37 (tightened by ruling R16): the kind holds only for the BASIC
/// link byte pair `{basic_start - 1, basic_start}`. A signed `padding_byte`
/// exception on `$0802` vs `$0801` (one byte apart, but not the pair) fails
/// with "kind no longer holds".
#[test]
fn eval_padding_byte_kind_predicate() {
    let run = |unp64, r2000, load_addr, system, fields: &[Field]| {
        eval_range_divergence(
            DivergenceKind::PaddingByte,
            fields,
            unp64,
            r2000,
            load_addr,
            system,
        )
    };
    let start = &[Field::Start];
    let mut failures = predicate_mismatches([
        (
            "unp64 one below",
            run(
                (0x0800, 0x2000),
                (0x0801, 0x2000),
                0x0801,
                System::C64,
                start,
            ),
            true,
        ),
        (
            "unp64 one above (R2000 kept the padding byte)",
            run(
                (0x0801, 0x2000),
                (0x0800, 0x2000),
                0x0801,
                System::C64,
                start,
            ),
            true,
        ),
        (
            "Card Cruncher load $07C0",
            run(
                (0x07C0, 0x2000),
                (0x07C1, 0x2000),
                0x07C0,
                System::C64,
                start,
            ),
            true,
        ),
        (
            "C128 default BASIC start",
            run(
                (0x1C00, 0x2000),
                (0x1C01, 0x2000),
                0x1C01,
                System::C128,
                start,
            ),
            true,
        ),
        (
            "$0802 vs $0801 is not the link byte pair",
            run(
                (0x0802, 0x2000),
                (0x0801, 0x2000),
                0x0801,
                System::C64,
                start,
            ),
            false,
        ),
        (
            "$1000 vs $1001 on the C64",
            run(
                (0x1000, 0x2000),
                (0x1001, 0x2000),
                0x0801,
                System::C64,
                start,
            ),
            false,
        ),
        (
            "delta 0",
            run(
                (0x0801, 0x2000),
                (0x0801, 0x2000),
                0x0801,
                System::C64,
                start,
            ),
            false,
        ),
        (
            "delta 2",
            run(
                (0x0803, 0x2000),
                (0x0801, 0x2000),
                0x0801,
                System::C64,
                start,
            ),
            false,
        ),
    ]);
    // Wrong `fields` for the kind.
    let wrong_fields = run(
        (0x0800, 0x2001),
        (0x0801, 0x2000),
        0x0801,
        System::C64,
        &[Field::Start, Field::End],
    );
    match holds_or_reason(&wrong_fields) {
        Err(text) if text.contains("requires fields = [start]") => {}
        other => failures.push(format!("wrong fields: {other:?}")),
    }
    // `len` is not even parseable as a field (T1.A1.11).
    assert!(
        Corpus::parse(&manifest_with_divergence(
            "kind = \"padding_byte\"\nfields = [\"start\", \"len\"]\nreviewed_by = \"x\""
        ))
        .is_err()
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// T1.A1.38
#[test]
fn eval_unp64_default_end_kind_predicate() {
    let run = |unp64, r2000| {
        eval_range_divergence(
            DivergenceKind::Unp64DefaultEnd,
            &[Field::End],
            unp64,
            r2000,
            0x0801,
            System::C64,
        )
    };
    let failures = predicate_mismatches([
        ("traveller", run((0x0801, 0xFFFD), (0x0801, 0x7949)), true),
        ("$FFFF", run((0x0801, 0xFFFF), (0x0801, 0x7949)), true),
        ("$FF00", run((0x0801, 0xFF00), (0x0801, 0x7949)), true),
        ("$FEFF", run((0x0801, 0xFEFF), (0x0801, 0x7949)), false),
        (
            "unp64 end below R2000",
            run((0x0801, 0xFF00), (0x0801, 0xFF80)),
            false,
        ),
    ]);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// T1.A1.39
#[test]
fn eval_unp64_default_start_kind_predicate() {
    let run = |unp64, r2000, load_addr, system| {
        eval_range_divergence(
            DivergenceKind::Unp64DefaultStart,
            &[Field::Start],
            unp64,
            r2000,
            load_addr,
            system,
        )
    };
    let failures = predicate_mismatches([
        (
            "C64 min is RAM start",
            run((0x0800, 0x2000), (0x0810, 0x2000), 0x0801, System::C64),
            true,
        ),
        (
            "unp64 start is the load, not the min",
            run((0x0801, 0x2000), (0x0810, 0x2000), 0x0801, System::C64),
            false,
        ),
        (
            "load below RAM start",
            run((0x0400, 0x2000), (0x0801, 0x2000), 0x0400, System::C64),
            true,
        ),
        (
            "C128 RAM start",
            run((0x1C00, 0x3000), (0x1C10, 0x3000), 0x1C01, System::C128),
            true,
        ),
        (
            "C128 rejects the C64 value",
            run((0x0800, 0x3000), (0x1C10, 0x3000), 0x1C01, System::C128),
            false,
        ),
        (
            "equal starts",
            run((0x0800, 0x2000), (0x0800, 0x2000), 0x0801, System::C64),
            false,
        ),
    ]);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// T1.A1.40
#[test]
fn eval_unexplained_kind_passes_and_is_listed() {
    // HBFS-like: unp64 $0400-$FEFF entry $080D, R2000 $0801-$FEFF entry $EFB0.
    let image = image(0x0801, 0xFEFF, 0xEFB0);
    let saved = record(0x0400, 0xFEFF, Unp64Entry::Address(0x080D), &image);
    let mut exception = signed(DivergenceKind::Unexplained, &[Field::Start, Field::Entry]);
    exception.note = Some("HBFS: nobody knows yet".to_string());
    let corpus = corpus(vec![case("c64_HBFS.exo3.prg", saved, Some(exception))]);
    let report = eval(&corpus, &ok_result("c64_HBFS.exo3.prg", image)).unwrap();
    assert_eq!(
        report.notes,
        vec![ReportNote::Unexplained {
            note: Some("HBFS: nobody knows yet".to_string())
        }]
    );
    let items = render_action_items(&[report]);
    assert!(
        items.contains("c64_HBFS.exo3.prg") && items.contains("unexplained"),
        "{items}"
    );
}

// ═══════════════════════ ground truth ═══════════════════════

/// T1.A1.41
#[test]
fn eval_ground_truth_overrides_unp64() {
    let image = image(0x0800, 0x0863, 0x0810);
    // unp64 disagrees on the start (Dali-style padding byte).
    let saved = record(0x0801, 0x0863, Unp64Entry::Address(0x0810), &image);
    let (corpus, fixtures) = ground_truth_setup(vec![member("a.prg", saved)]);
    let report = eval_ground_truth(&corpus, &fixtures, &ok_result("a.prg", image)).unwrap();
    assert_eq!(
        report.oracle,
        Oracle::GroundTruth {
            group: "prog".to_string()
        }
    );
    assert_eq!(
        report.notes,
        vec![ReportNote::Unp64DisagreesWithGroundTruth {
            fields: vec![Field::Start],
            payload: false,
        }]
    );
}

/// T1.A1.42
#[test]
fn eval_ground_truth_range_derived_from_original_file() {
    let group = Group {
        id: "prog".to_string(),
        original: "prog_orig.prg".to_string(),
        entry: HexAddr(0x0810),
    };
    let truth = GroundTruth::from_prg(&group, &prg(0x0800, &[0xAA; 100])).unwrap();
    assert_eq!(truth.range, range(0x0800, 0x0863));
    assert_eq!(truth.entry, 0x0810);
    assert_eq!(truth.data.len(), 100);

    let edge = GroundTruth::from_prg(&group, &prg(0xFF00, &[0; 256])).unwrap();
    assert_eq!(edge.range, range(0xFF00, 0xFFFF));

    for bad in [vec![0x01], prg(0x0800, &[]), prg(0xFF00, &[0; 257])] {
        assert!(
            matches!(
                GroundTruth::from_prg(&group, &bad),
                Err(CorpusError::InvalidGroupOriginal { .. })
            ),
            "{bad:?}"
        );
    }

    let image = image(0x0800, 0x0863, 0x0810);
    let saved = matching_record(&image);
    let (corpus, fixtures) = ground_truth_setup(vec![member("a.prg", saved)]);
    assert!(eval_ground_truth(&corpus, &fixtures, &ok_result("a.prg", image)).is_ok());

    // An unreadable or unusable original is reported when the evaluator is
    // built, never silently skipped.
    let missing = Evaluator::new(&corpus, &MemFixtures::default()).unwrap_err();
    assert_eq!(
        missing,
        vec![CorpusError::MissingFixture {
            file: "prog_orig.prg".to_string()
        }]
    );
    let empty = MemFixtures::default().with("prog_orig.prg", prg(0x0800, &[]));
    assert!(matches!(
        Evaluator::new(&corpus, &empty).unwrap_err().as_slice(),
        [CorpusError::InvalidGroupOriginal { .. }]
    ));
}

/// T1.A1.43
#[test]
fn eval_ground_truth_range_mismatch_fails() {
    let image = image(0x0801, 0x0863, 0x0810);
    let saved = matching_record(&image);
    let (corpus, fixtures) = ground_truth_setup(vec![member("a.prg", saved)]);
    let found = eval_ground_truth(&corpus, &fixtures, &ok_result("a.prg", image)).unwrap_err();
    assert_eq!(
        found,
        vec![FailureReason::GroundTruthMismatch {
            group: "prog".to_string(),
            field: Field::Start,
            expected: HexAddr(0x0800),
            actual: HexAddr(0x0801),
        }]
    );
}

/// T1.A1.44
#[test]
fn eval_ground_truth_payload_mismatch_reports_first_address() {
    let mut image = image(0x0800, 0x0863, 0x0810);
    let saved = matching_record(&image);
    image.data[10] ^= 0x01;
    image.data[20] ^= 0x01;
    let (corpus, fixtures) = ground_truth_setup(vec![member("a.prg", saved)]);
    let found = eval_ground_truth(&corpus, &fixtures, &ok_result("a.prg", image)).unwrap_err();
    assert_eq!(
        found,
        vec![FailureReason::GroundTruthPayloadMismatch {
            group: "prog".to_string(),
            first: HexAddr(0x080A),
            count: 2,
        }]
    );
    assert!(found[0].to_string().contains("$080A"));
}

/// T1.A1.45
#[test]
fn eval_ground_truth_entry_must_equal_group_entry() {
    let image = image(0x0800, 0x0863, 0x0820);
    let saved = matching_record(&image);
    let (corpus, fixtures) = ground_truth_setup(vec![member("a.prg", saved)]);
    let found = eval_ground_truth(&corpus, &fixtures, &ok_result("a.prg", image)).unwrap_err();
    assert_eq!(
        found,
        vec![FailureReason::GroundTruthMismatch {
            group: "prog".to_string(),
            field: Field::Entry,
            expected: HexAddr(0x0810),
            actual: HexAddr(0x0820),
        }]
    );
}

/// A failed unp64 run on a ground-truth case is informational only.
#[test]
fn eval_ground_truth_with_failed_unp64_is_a_note() {
    let image = image(0x0800, 0x0863, 0x0810);
    let mut grouped = member("a.prg", matching_record(&image));
    grouped.unp64 = Some(failed_record("Max Iterations"));
    let (corpus, fixtures) = ground_truth_setup(vec![grouped]);
    let report = eval_ground_truth(&corpus, &fixtures, &ok_result("a.prg", image)).unwrap();
    assert_eq!(
        report.notes,
        vec![ReportNote::Unp64FailedOnGroundTruth {
            reason: "Max Iterations".to_string()
        }]
    );
    assert!(render_action_items(&[report]).is_empty());
}

// ═══════════════════════ consensus ═══════════════════════

/// T1.A1.46
#[test]
fn eval_consensus_all_members_agree_passes() {
    let image = image(0x0800, 0x0863, 0x0810);
    let failures = consensus_of(vec![
        grouped_member("a.prg", &image),
        grouped_member("b.prg", &image),
        grouped_member("c.prg", &image),
    ]);
    assert_eq!(failures, Vec::new());
}

/// T1.A1.47
#[test]
fn eval_consensus_disagreement_fails_and_names_members() {
    let good = image(0x0800, 0x0863, 0x0810);
    let other_entry = image(0x0800, 0x0863, 0x0820);
    let failures = consensus_of(vec![
        grouped_member("a.prg", &good),
        grouped_member("b.prg", &other_entry),
    ]);
    assert_eq!(
        failures,
        vec![CaseFailure {
            subject: Subject::Group("prog".to_string()),
            reason: FailureReason::ConsensusMismatch {
                property: ConsensusProperty::Entry,
                values: vec![
                    ("a.prg".to_string(), "$0810".to_string()),
                    ("b.prg".to_string(), "$0820".to_string()),
                ],
            },
        }]
    );
    let text = failures[0].reason.to_string();
    assert!(text.contains("a.prg") && text.contains("b.prg"), "{text}");
}

/// T1.A1.48
#[test]
fn eval_consensus_single_member_group_is_noop() {
    let odd = image(0x0801, 0x0900, 0x0820);
    assert_eq!(
        consensus_of(vec![grouped_member("a.prg", &odd)]),
        Vec::new()
    );
}

/// T1.A1.49
#[test]
fn eval_consensus_skips_expected_fail_members() {
    let good = image(0x0800, 0x0863, 0x0810);
    let padded = image(0x0801, 0x0863, 0x0810);
    let (mut dali, dali_image) = grouped_member("dali.prg", &padded);
    dali.expected_fail = Some(ExpectedFailReason::PaddingConvention);
    let failures = consensus_of(vec![
        grouped_member("a.prg", &good),
        grouped_member("b.prg", &good),
        (dali, dali_image),
    ]);
    assert_eq!(failures, Vec::new());
}

// ═══════════════════════ expected_fail and stale exceptions ═══════════════════════

/// T1.A1.50
#[test]
fn eval_expected_fail_that_fails_is_green_and_reported() {
    let padded = image(0x0801, 0x0863, 0x0810);
    let (mut dali, _) = grouped_member("dali.prg", &padded);
    dali.expected_fail = Some(ExpectedFailReason::PaddingConvention);
    let (corpus, fixtures) = ground_truth_setup(vec![dali]);
    let report = eval_ground_truth(&corpus, &fixtures, &ok_result("dali.prg", padded)).unwrap();
    assert!(
        matches!(
            report.notes.as_slice(),
            [ReportNote::ExpectedFailure {
                reason: ExpectedFailReason::PaddingConvention,
                failures,
            }] if failures.len() == 1
        ),
        "{:?}",
        report.notes
    );
    let items = render_action_items(&[report]);
    assert!(
        items.contains("dali.prg") && items.contains("padding_convention"),
        "{items}"
    );
}

/// T1.A1.51
#[test]
fn eval_expected_fail_that_passes_is_failure() {
    let good = image(0x0800, 0x0863, 0x0810);
    let (mut dali, _) = grouped_member("dali.prg", &good);
    dali.expected_fail = Some(ExpectedFailReason::PaddingConvention);
    let (corpus, fixtures) = ground_truth_setup(vec![dali]);
    let found = eval_ground_truth(&corpus, &fixtures, &ok_result("dali.prg", good)).unwrap_err();
    assert_eq!(
        found,
        vec![FailureReason::UnexpectedPass {
            reason: ExpectedFailReason::PaddingConvention
        }]
    );
    assert!(
        found[0]
            .to_string()
            .contains("unexpected pass: remove expected_fail")
    );
}

/// T1.A1.52
#[test]
fn eval_stale_divergence_is_failure() {
    // Every listed field now equals unp64.
    let image = image(0x0801, 0x2000, 0x080D);
    let corpus_all = corpus(vec![case(
        "a.prg",
        matching_record(&image),
        Some(signed(DivergenceKind::Unexplained, &[Field::End])),
    )]);
    let found = eval(&corpus_all, &ok_result("a.prg", image.clone())).unwrap_err();
    assert_eq!(
        found,
        vec![FailureReason::StaleDivergence {
            stale: vec![Field::End],
            all_listed: true,
        }]
    );
    assert!(
        found[0]
            .to_string()
            .contains("remove the accepted_divergence")
    );

    // Only one of two listed fields is stale.
    let saved = record(0x0801, 0xFFFF, Unp64Entry::Address(0x080D), &image);
    let corpus_partial = corpus(vec![case(
        "a.prg",
        saved,
        Some(signed(
            DivergenceKind::Unexplained,
            &[Field::End, Field::Entry],
        )),
    )]);
    let found = eval(&corpus_partial, &ok_result("a.prg", image)).unwrap_err();
    assert_eq!(
        found,
        vec![FailureReason::StaleDivergence {
            stale: vec![Field::Entry],
            all_listed: false,
        }]
    );
    assert!(found[0].to_string().contains("remove them from `fields`"));
}

/// T1.A1.53 (scoped by ruling R17: an R2000 error fails even under an
/// `expected_fail` marker)
#[test]
fn eval_r2000_error_fails_unless_expected() {
    let image = image(0x0801, 0x2000, 0x080D);
    let mut plain = case("a.prg", matching_record(&image), None);
    let found = eval(
        &corpus(vec![plain.clone()]),
        &err_result("a.prg", "Phase2Timeout"),
    )
    .unwrap_err();
    assert!(
        matches!(found.as_slice(), [FailureReason::R2000Failed { error }] if error.variant == "Phase2Timeout"),
        "{found:?}"
    );

    // Ruling R17: each reason masks only its own closed set of failures, so
    // no `expected_fail` marker masks an R2000 error.
    for marker in [
        ExpectedFailReason::PaddingConvention,
        ExpectedFailReason::PayloadMismatchUnderInvestigation,
    ] {
        plain.expected_fail = Some(marker);
        let found = eval(
            &corpus(vec![plain.clone()]),
            &err_result("a.prg", "Phase2Timeout"),
        )
        .unwrap_err();
        assert!(
            matches!(found.as_slice(), [FailureReason::R2000Failed { .. }]),
            "{marker}: {found:?}"
        );
    }
}

/// Ruling R17: `padding_convention` masks only a ground-truth start
/// mismatch on the BASIC link byte pair. Rows are collected before the
/// assertion.
#[test]
fn expected_fail_padding_convention_masks_only_the_padding_start() {
    // (label, R2000 image, masked?)
    let rows = [
        (
            "Dali $0801 vs original $0800",
            image(0x0801, 0x0863, 0x0810),
            true,
        ),
        ("start delta 2", image(0x0802, 0x0863, 0x0810), false),
        (
            "padding start plus entry mismatch",
            image(0x0801, 0x0863, 0x0820),
            false,
        ),
        (
            "padding start plus end mismatch",
            image(0x0801, 0x0864, 0x0810),
            false,
        ),
    ];
    let mut wrong = Vec::new();
    for (label, r2000, masked) in rows {
        let (mut dali, _) = grouped_member("dali.prg", &r2000);
        dali.expected_fail = Some(ExpectedFailReason::PaddingConvention);
        let (corpus, fixtures) = ground_truth_setup(vec![dali]);
        let outcome = eval_ground_truth(&corpus, &fixtures, &ok_result("dali.prg", r2000));
        let green = matches!(
            &outcome,
            Ok(report) if matches!(
                report.notes.as_slice(),
                [ReportNote::ExpectedFailure { failures, .. }] if failures.len() == 1
            )
        );
        if green != masked {
            wrong.push(format!("{label}: {outcome:?}"));
        }
    }

    // Nothing left to mask: unexpected pass, which names the marker.
    let good = image(0x0800, 0x0863, 0x0810);
    let (mut dali, _) = grouped_member("dali.prg", &good);
    dali.expected_fail = Some(ExpectedFailReason::PaddingConvention);
    let (corpus, fixtures) = ground_truth_setup(vec![dali]);
    let pass = eval_ground_truth(&corpus, &fixtures, &ok_result("dali.prg", good));
    if pass
        != Err(vec![FailureReason::UnexpectedPass {
            reason: ExpectedFailReason::PaddingConvention,
        }])
    {
        wrong.push(format!("unexpected pass: {pass:?}"));
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// One "must still fail" row: label, evaluation outcome, and the failure
/// that must remain.
type UnmaskedRow<'a> = (
    &'a str,
    Result<CaseReport, Vec<FailureReason>>,
    fn(&FailureReason) -> bool,
);

/// The failures masked by an `expected_fail` note, or `None` if the case
/// did not pass with one.
fn masked_failures(outcome: &Result<CaseReport, Vec<FailureReason>>) -> Option<Vec<FailureReason>> {
    outcome
        .as_ref()
        .ok()?
        .notes
        .iter()
        .find_map(|note| match note {
            ReportNote::ExpectedFailure { failures, .. } => Some(failures.clone()),
            _ => None,
        })
}

/// Ruling R17: `payload_mismatch_under_investigation` masks only payload and
/// overlap hash mismatches. Field mismatches, structural overlap errors and
/// unsigned exceptions still fail, and a repaired payload is an unexpected
/// pass. Rows are collected before the assertion.
#[test]
fn expected_fail_payload_mismatch_masks_only_payload_hashes() {
    let run = |saved: Unp64Saved, divergence: Option<AcceptedDivergence>, r2000: UnpackedImage| {
        let mut marked = case("a.prg", saved, divergence);
        marked.expected_fail = Some(ExpectedFailReason::PayloadMismatchUnderInvestigation);
        eval(&corpus(vec![marked]), &ok_result("a.prg", r2000))
    };
    let good = image(0x0801, 0x2000, 0x080D);
    let mut corrupted = good.clone();
    corrupted.data[0x10] ^= 0xFF;
    let longer = record(0x0801, 0x3000, Unp64Entry::Address(0x080D), &good);
    let unexplained_end = || Some(signed(DivergenceKind::Unexplained, &[Field::End]));
    let mut wrong = Vec::new();

    // Masked: the whole-payload hash (same range) and the overlap hash.
    let masked_rows = [
        (
            "whole-payload hash",
            run(matching_record(&good), None, corrupted.clone()),
        ),
        (
            "overlap hash",
            run(longer.clone(), unexplained_end(), corrupted.clone()),
        ),
    ];
    for (label, outcome) in masked_rows {
        let only_hashes = masked_failures(&outcome).is_some_and(|failures| {
            !failures.is_empty()
                && failures.iter().all(|failure| {
                    matches!(
                        failure,
                        FailureReason::PayloadHashMismatch { .. }
                            | FailureReason::OverlapHashMismatch { .. }
                    )
                })
        });
        if !only_hashes {
            wrong.push(format!(
                "{label}: expected a masked hash mismatch, got {outcome:?}"
            ));
        }
    }

    // Not masked: each row must still fail with the named failure.
    let mut no_overlap = longer.clone();
    no_overlap.overlap = None;
    let unmasked_rows: [UnmaskedRow<'_>; 4] = [
        (
            "field mismatch next to the payload mismatch",
            run(longer.clone(), None, corrupted.clone()),
            |failure| {
                matches!(
                    failure,
                    FailureReason::FieldMismatch {
                        field: Field::End,
                        ..
                    }
                )
            },
        ),
        (
            "missing overlap (structural)",
            run(no_overlap, unexplained_end(), corrupted.clone()),
            |failure| matches!(failure, FailureReason::MissingOverlap),
        ),
        (
            "unsigned exception",
            run(
                longer.clone(),
                Some(divergence(DivergenceKind::Unexplained, &[Field::End], None)),
                corrupted.clone(),
            ),
            |failure| matches!(failure, FailureReason::UnsignedDivergence),
        ),
        (
            "payload repaired (unexpected pass)",
            run(longer, unexplained_end(), good),
            |failure| {
                matches!(
                    failure,
                    FailureReason::UnexpectedPass {
                        reason: ExpectedFailReason::PayloadMismatchUnderInvestigation
                    }
                )
            },
        ),
    ];
    for (label, outcome, wanted) in unmasked_rows {
        if !matches!(&outcome, Err(found) if found.iter().any(wanted)) {
            wrong.push(format!(
                "{label}: expected the failure to remain, got {outcome:?}"
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Every `expected_fail` note is a user action item, so both R17 cases
/// appear in every report.
#[test]
fn expected_failures_are_action_items() {
    for reason in [
        ExpectedFailReason::PaddingConvention,
        ExpectedFailReason::PayloadMismatchUnderInvestigation,
    ] {
        let note = ReportNote::ExpectedFailure {
            reason,
            failures: vec![FailureReason::MissingOverlap],
        };
        assert!(note.is_action_item(), "{reason}");
        assert!(note.to_string().contains(reason.as_str()));
    }
}

// ═══════════════════════ edge cases and aggregation ═══════════════════════

/// Evaluation edge cases: an unknown file, a failed unp64 run with and
/// without an exception, an inconsistent R2000 result, and a BASIC `RUN`
/// entry that must be accepted explicitly.
#[test]
fn eval_edge_cases() {
    let image = image(0x0801, 0x2000, 0x080D);
    let corpus_a = corpus(vec![case("a.prg", matching_record(&image), None)]);
    assert_eq!(
        eval(&corpus_a, &ok_result("other.prg", image.clone())),
        Err(vec![FailureReason::UnknownCase])
    );

    let mut failed = case("a.prg", matching_record(&image), None);
    failed.unp64 = Some(failed_record("Max Iterations"));
    assert_eq!(
        eval(
            &corpus(vec![failed.clone()]),
            &ok_result("a.prg", image.clone())
        ),
        Err(vec![FailureReason::Unp64FailedUnaccepted {
            reason: "Max Iterations".to_string()
        }])
    );
    failed.accepted_divergence = Some(signed(DivergenceKind::Unp64MissingOpcode, &Field::ALL));
    assert!(eval(&corpus(vec![failed]), &ok_result("a.prg", image.clone())).is_ok());

    let mut broken = image.clone();
    broken.data.pop();
    assert!(matches!(
        eval(&corpus_a, &ok_result("a.prg", broken))
            .unwrap_err()
            .as_slice(),
        [FailureReason::InconsistentResult { .. }]
    ));

    let basic_run = record(0x0801, 0x2000, Unp64Entry::BasicRun, &image);
    let unaccepted = corpus(vec![case("a.prg", basic_run.clone(), None)]);
    assert_eq!(
        eval(&unaccepted, &ok_result("a.prg", image.clone())),
        Err(vec![FailureReason::FieldMismatch {
            field: Field::Entry,
            unp64: "RUN".to_string(),
            r2000: HexAddr(0x080D),
        }])
    );
    let accepted = corpus(vec![case(
        "a.prg",
        basic_run,
        Some(signed(DivergenceKind::Unexplained, &[Field::Entry])),
    )]);
    assert!(eval(&accepted, &ok_result("a.prg", image)).is_ok());
}

/// `evaluate_all` merges per-case failures with consensus failures, sorts
/// reports by file and failures by subject, and is green only when nothing
/// failed.
#[test]
fn evaluate_all_aggregates_cases_and_consensus_sorted() {
    let good = image(0x0800, 0x0863, 0x0810);
    let other_entry = image(0x0800, 0x0863, 0x0820);
    let plain = image(0x0801, 0x2000, 0x080D);

    let (mut corpus, fixtures) = ground_truth_setup(vec![
        member("b.prg", matching_record(&good)),
        member("a.prg", matching_record(&other_entry)),
    ]);
    corpus
        .cases
        .push(case("z.prg", matching_record(&plain), None));
    let evaluator = Evaluator::new(&corpus, &fixtures).unwrap();

    // Deliberately unsorted input order.
    let results = [
        ok_result("z.prg", plain.clone()),
        ok_result("b.prg", good.clone()),
        ok_result("a.prg", other_entry),
    ];
    let outcome = evaluator.evaluate_all(&results);
    assert!(!outcome.is_green());
    let report_files: Vec<&str> = outcome.reports.iter().map(|r| r.file.as_str()).collect();
    assert_eq!(report_files, vec!["b.prg", "z.prg"]);
    let subjects: Vec<Subject> = outcome
        .failures
        .iter()
        .map(|failure| failure.subject.clone())
        .collect();
    assert_eq!(
        subjects,
        vec![
            Subject::Case("a.prg".to_string()),
            Subject::Group("prog".to_string()),
        ]
    );

    let green = evaluator.evaluate_all(&[ok_result("z.prg", plain), ok_result("b.prg", good)]);
    assert!(green.is_green(), "{:?}", green.failures);
    assert_eq!(green.reports.len(), 2);
}
