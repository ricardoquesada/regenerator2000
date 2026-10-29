//! Tests for `--update` (T1.A2.33–T1.A2.38) plus the preservation rules for
//! reviewed decisions, ground-truth members and R2000 failures.

use super::*;
use crate::corpus::{
    Corpus, Fnv64, Group, HexAddr, INITIAL_REVIEW_MARKER, Overlap, Unp64Record, Unp64Saved,
};
use crate::update::{ExceptionChange, RecordChange, update_case, update_corpus};

/// The saved record of an updated case.
fn saved_record(case: &Case) -> &Unp64Saved {
    match &case.unp64 {
        Some(Unp64Record::Saved(saved)) => saved,
        other => panic!("expected a saved record, got {other:?}"),
    }
}

/// Traveller-like observation: unp64 saved up to `$FFFD`, R2000 to `$7949`.
fn traveller(case: Case) -> Observation {
    observation(
        case,
        ok_result("t.prg", image(0x0801, 0x7949, 0x0911)),
        saved(0x0801, 0xFFFD, Unp64Entry::Address(0x0911)),
    )
}

/// T1.A2.33: the record holds unp64's results, hash and version, and the
/// overlap window is the intersection hashed over unp64's bytes.
#[test]
fn update_writes_unp64_table_and_overlap() {
    let update = update_case(&traveller(bare_case("t.prg")), VERSION);
    assert_eq!(update.record, RecordChange::Added);
    let record = saved_record(&update.case);
    assert_eq!(
        record,
        &Unp64Saved {
            start: HexAddr(0x0801),
            end: HexAddr(0xFFFD),
            entry: Unp64Entry::Address(0x0911),
            len: 0xFFFD - 0x0801 + 1,
            payload_fnv64: Fnv64::of(&mem_bytes(0x0801, 0xFFFD)),
            overlap: Some(Overlap {
                start: HexAddr(0x0801),
                end: HexAddr(0x7949),
                fnv64: Fnv64::of(&mem_bytes(0x0801, 0x7949)),
            }),
            version: VERSION.to_string(),
        }
    );

    // Equal ranges need no overlap; the whole-payload hash decides.
    let agree = update_case(
        &observation(
            bare_case("a.prg"),
            ok_result("a.prg", image(0x0801, 0x2000, 0x080D)),
            saved(0x0801, 0x2000, Unp64Entry::Address(0x080D)),
        ),
        VERSION,
    );
    assert_eq!(saved_record(&agree.case).overlap, None);
    assert_eq!(agree.exception, ExceptionChange::NotNeeded);
    assert_eq!(agree.case.accepted_divergence, None);

    // The written manifest parses back to the same corpus (canonical form).
    let corpus = Corpus {
        groups: Vec::new(),
        cases: vec![update.case.clone(), agree.case.clone()],
    };
    let text = corpus.to_toml_string().unwrap();
    assert_eq!(
        Corpus::parse(&text).unwrap().to_toml_string().unwrap(),
        text
    );
}

/// T1.A2.34: a new difference gets an unsigned proposal that lists only
/// `start`/`end`/`entry` fields (never `len`) and carries the evidence.
#[test]
fn update_proposes_unsigned_divergence_for_new_difference() {
    let update = update_case(&traveller(bare_case("t.prg")), VERSION);
    let proposal = update.case.accepted_divergence.clone().unwrap();
    assert_eq!(proposal.kind, DivergenceKind::Unp64DefaultEnd);
    assert_eq!(proposal.fields.as_slice(), &[Field::End]);
    assert_eq!(proposal.reviewed_by, None);
    let note = proposal.note.clone().unwrap();
    assert!(note.contains("$FFFD") && note.contains("$7949"), "{note}");
    assert_eq!(
        update.exception,
        ExceptionChange::Proposed {
            proposal,
            previous: None
        }
    );

    // An unexplained start + end difference lists exactly those two fields.
    let both = update_case(
        &observation(
            bare_case("d.prg"),
            ok_result("d.prg", image(0x0801, 0xFF91, 0x0810)),
            saved(0x0810, 0xFFF9, Unp64Entry::Address(0x0810)),
        ),
        VERSION,
    );
    let proposal = both.case.accepted_divergence.unwrap();
    assert_eq!(proposal.kind, DivergenceKind::Unexplained);
    assert_eq!(proposal.fields.as_slice(), &[Field::Start, Field::End]);
    assert_eq!(proposal.reviewed_by, None);
}

/// T1.A2.35: payload and entry mismatches are never proposed.
#[test]
fn update_never_proposes_payload_or_entry_mismatch() {
    let mut corrupted = image(0x0801, 0x2000, 0x080D);
    corrupted.data[0x10] ^= 0xFF;
    let payload = update_case(
        &observation(
            bare_case("p.prg"),
            ok_result("p.prg", corrupted),
            saved(0x0801, 0x2000, Unp64Entry::Address(0x080D)),
        ),
        VERSION,
    );
    let entry = update_case(
        &observation(
            bare_case("e.prg"),
            ok_result("e.prg", image(0x0801, 0x2000, 0x0900)),
            saved(0x0801, 0x2000, Unp64Entry::Address(0x080D)),
        ),
        VERSION,
    );
    for update in [payload, entry] {
        assert_eq!(
            update.case.accepted_divergence, None,
            "{}",
            update.case.file
        );
        assert_eq!(update.exception, ExceptionChange::NeedsReview);
        assert!(update.case.unp64.is_some());
    }
}

/// T1.A2.36: a signed exception survives a re-run with the same results,
/// including the initial-classification marker and a different unp64
/// version string.
#[test]
fn update_preserves_signature_when_classification_unchanged() {
    let first = update_case(&traveller(bare_case("t.prg")), VERSION);
    for signature in [INITIAL_REVIEW_MARKER, "ricardoq"] {
        let mut case = first.case.clone();
        let mut signed = case.accepted_divergence.clone().unwrap();
        signed.reviewed_by = Some(signature.to_string());
        signed.note = Some("reviewed by hand".to_string());
        case.accepted_divergence = Some(signed.clone());

        for version in [VERSION, "2.43"] {
            let again = update_case(&traveller(case.clone()), version);
            assert_eq!(
                again.record,
                RecordChange::Unchanged,
                "{signature} {version}"
            );
            assert_eq!(again.case.accepted_divergence, Some(signed.clone()));
            assert_eq!(
                again.exception,
                ExceptionChange::Kept {
                    exception: signed.clone()
                }
            );
        }
    }

    // A reviewer's `unexplained` decision on an entry mismatch is a human
    // decision and survives too.
    let hbfs = |case: Case| {
        observation(
            case,
            ok_result("h.prg", image(0x0801, 0xFEFF, 0xEFB0)),
            saved(0x0400, 0xFEFF, Unp64Entry::Address(0x080D)),
        )
    };
    let recorded = update_case(&hbfs(bare_case("h.prg")), VERSION);
    assert_eq!(recorded.exception, ExceptionChange::NeedsReview);
    let mut reviewed = recorded.case.clone();
    let decision = exception(
        DivergenceKind::Unexplained,
        &[Field::Start, Field::Entry],
        Some(INITIAL_REVIEW_MARKER),
    );
    reviewed.accepted_divergence = Some(decision.clone());
    let again = update_case(&hbfs(reviewed), VERSION);
    assert_eq!(again.case.accepted_divergence, Some(decision));
}

/// T1.A2.37: the signature is dropped when the recorded values change or
/// the exception no longer describes the case.
#[test]
fn update_drops_signature_when_kind_or_values_change() {
    let mut case = update_case(&traveller(bare_case("t.prg")), VERSION).case;
    let mut signed = case.accepted_divergence.clone().unwrap();
    signed.reviewed_by = Some("ricardoq".to_string());
    case.accepted_divergence = Some(signed.clone());

    // unp64 now saves to $FFFF: same kind, new values → new unsigned proposal.
    let values = update_case(
        &observation(
            case.clone(),
            ok_result("t.prg", image(0x0801, 0x7949, 0x0911)),
            saved(0x0801, 0xFFFF, Unp64Entry::Address(0x0911)),
        ),
        VERSION,
    );
    assert_eq!(values.record, RecordChange::Changed);
    let proposal = values.case.accepted_divergence.clone().unwrap();
    assert_eq!(proposal.kind, DivergenceKind::Unp64DefaultEnd);
    assert_eq!(proposal.reviewed_by, None);
    assert_eq!(
        values.exception,
        ExceptionChange::Proposed {
            proposal,
            previous: Some(signed.clone())
        }
    );

    // A recorded kind that no longer matches (and is not `unexplained`) is
    // replaced: the same difference is now classified as unexplained.
    let mut wrong_kind = case.clone();
    wrong_kind.accepted_divergence = Some(exception(
        DivergenceKind::PaddingByte,
        &[Field::End],
        Some("ricardoq"),
    ));
    let replaced = update_case(&traveller(wrong_kind), VERSION);
    assert_eq!(
        replaced.case.accepted_divergence.unwrap().kind,
        DivergenceKind::Unp64DefaultEnd
    );

    // The tools agree now: the exception is removed (stale, ruling R13).
    let agree = update_case(
        &observation(
            case,
            ok_result("t.prg", image(0x0801, 0xFFFD, 0x0911)),
            saved(0x0801, 0xFFFD, Unp64Entry::Address(0x0911)),
        ),
        VERSION,
    );
    assert_eq!(agree.case.accepted_divergence, None);
    assert_eq!(
        agree.exception,
        ExceptionChange::Removed { previous: signed }
    );
}

/// T1.A2.38: a failed unp64 run is recorded as `failed` and proposed as an
/// unsigned `unexplained` exception over all three fields; a missing-opcode
/// stop is proposed as `unp64_missing_opcode`.
#[test]
fn update_records_failed_unp64_as_unexplained_proposal() {
    let update = update_case(
        &observation(
            bare_case("f.prg"),
            ok_result("f.prg", image(0x0801, 0x2000, 0x080D)),
            failed("Max Iterations 0x01000000 Reached, quitting..."),
        ),
        VERSION,
    );
    assert!(matches!(
        &update.case.unp64,
        Some(Unp64Record::Failed(failed)) if failed.reason.starts_with("Max Iterations")
            && failed.version == VERSION
    ));
    let proposal = update.case.accepted_divergence.unwrap();
    assert_eq!(proposal.kind, DivergenceKind::Unexplained);
    assert_eq!(proposal.fields.as_slice(), &Field::ALL);
    assert_eq!(proposal.reviewed_by, None);

    let opcode = update_case(
        &observation(
            bare_case("o.prg"),
            ok_result("o.prg", image(0x0801, 0x2000, 0x080D)),
            Unp64Outcome::Failed {
                reason: "unimplemented opcode $02 @ $0812".to_string(),
                missing_opcode: true,
            },
        ),
        VERSION,
    );
    let proposal = opcode.case.accepted_divergence.unwrap();
    assert_eq!(proposal.kind, DivergenceKind::Unp64MissingOpcode);
    assert_eq!(proposal.fields.as_slice(), &Field::ALL);
}

/// Ruling R17: on a case marked `payload_mismatch_under_investigation`, the
/// reviewer's `unexplained` exception for the non-payload fields survives
/// re-runs; without the marker a payload mismatch never keeps an exception.
#[test]
fn update_keeps_reviewed_fields_of_a_payload_mismatch_under_investigation() {
    let hbfs = |case: Case| {
        let mut inner_stage = image(0x0801, 0xFEFF, 0xEFB0);
        inner_stage.data[2] ^= 0xFF;
        observation(
            case,
            ok_result("h.prg", inner_stage),
            saved(0x0400, 0xFEFF, Unp64Entry::Address(0x080D)),
        )
    };
    let recorded = update_case(&hbfs(bare_case("h.prg")), VERSION);
    assert!(matches!(
        recorded.divergence,
        crate::corpus::Divergence::PayloadMismatch { .. }
    ));
    assert_eq!(recorded.exception, ExceptionChange::NeedsReview);

    let decision = exception(
        DivergenceKind::Unexplained,
        &[Field::Start, Field::Entry],
        Some(INITIAL_REVIEW_MARKER),
    );
    let mut marked = recorded.case.clone();
    marked.accepted_divergence = Some(decision.clone());
    marked.expected_fail =
        Some(crate::corpus::ExpectedFailReason::PayloadMismatchUnderInvestigation);
    let again = update_case(&hbfs(marked.clone()), VERSION);
    assert_eq!(again.case.accepted_divergence, Some(decision.clone()));
    assert_eq!(again.case.expected_fail, marked.expected_fail);

    let mut unmarked = marked;
    unmarked.expected_fail = None;
    let removed = update_case(&hbfs(unmarked), VERSION);
    assert_eq!(removed.case.accepted_divergence, None);
    assert_eq!(
        removed.exception,
        ExceptionChange::Removed { previous: decision }
    );
}

/// Ground-truth members never get an exception, and an R2000 failure leaves
/// any exception exactly as it was (there is nothing to judge it by).
#[test]
fn update_leaves_ground_truth_and_r2000_failures_alone() {
    let mut member = bare_case("m.prg");
    member.group = Some("mule".to_string());
    let grouped = update_case(&traveller(member), VERSION);
    assert_eq!(grouped.exception, ExceptionChange::GroundTruth);
    assert_eq!(grouped.case.accepted_divergence, None);
    assert!(grouped.case.unp64.is_some());

    let mut signed_case = bare_case("x.prg");
    let decision = exception(DivergenceKind::Unexplained, &[Field::End], Some("ricardoq"));
    signed_case.accepted_divergence = Some(decision.clone());
    let failed_run = update_case(
        &observation(
            signed_case,
            err_result("x.prg", "Phase2Timeout"),
            saved(0x0801, 0x2000, Unp64Entry::Address(0x080D)),
        ),
        VERSION,
    );
    assert_eq!(failed_run.exception, ExceptionChange::R2000Failed);
    assert_eq!(failed_run.case.accepted_divergence, Some(decision));
}

/// The corpus update replaces exactly the observed cases, keeps groups and
/// unobserved cases, and lists changes sorted by file.
#[test]
fn update_corpus_replaces_only_observed_cases() {
    let group = Group {
        id: "mule".to_string(),
        original: "orig.prg".to_string(),
        entry: HexAddr(0x1100),
    };
    let corpus = Corpus {
        groups: vec![group.clone()],
        cases: vec![bare_case("z.prg"), bare_case("t.prg"), bare_case("u.prg")],
    };
    let observations = vec![traveller(bare_case("z.prg")), traveller(bare_case("t.prg"))];
    let update = update_corpus(&corpus, &observations, VERSION);
    assert_eq!(update.corpus.groups, vec![group]);
    let files: Vec<&str> = update.cases.iter().map(|c| c.case.file.as_str()).collect();
    assert_eq!(files, ["t.prg", "z.prg"]);
    assert!(update.corpus.case("t.prg").unwrap().unp64.is_some());
    assert!(update.corpus.case("z.prg").unwrap().unp64.is_some());
    assert_eq!(update.corpus.case("u.prg"), Some(&bare_case("u.prg")));
}
