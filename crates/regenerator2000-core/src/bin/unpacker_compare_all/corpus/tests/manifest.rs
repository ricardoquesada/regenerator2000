//! Tests for `manifest`: parsing (T1.A1.8–T1.A1.17), the deterministic
//! writer (T1.A1.62–T1.A1.63) and parser robustness (TX.6).

use super::*;
use regenerator2000_core::unpacker::UnpackConfig;
use std::num::NonZeroU64;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// A manifest exercising every optional field.
fn full_corpus() -> Corpus {
    let image = image(0x0801, 0x2000, 0x080D);
    let mut traveller = case(
        "c64_traveller.tiny_crunch.prg",
        record(0x0801, 0xFFFD, Unp64Entry::Address(0x080D), &image),
        Some(AcceptedDivergence {
            kind: DivergenceKind::Unp64DefaultEnd,
            fields: FieldSet::new([Field::End]).unwrap(),
            reviewed_by: Some(INITIAL_REVIEW_MARKER.to_string()),
            note: Some("R2000 write-diff ends earlier".to_string()),
        }),
    );
    traveller.max_instructions = NonZeroU64::new(350_000_000);
    let mut dali = case("c64_mule.dali.prg", matching_record(&image), None);
    dali.group = Some("mule".to_string());
    dali.expected_fail = Some(ExpectedFailReason::PaddingConvention);
    let mut hbfs = case("c64_HBFS.exo3.prg", matching_record(&image), None);
    hbfs.unp64 = Some(failed_record("Max Iterations"));
    hbfs.accepted_divergence = Some(signed(DivergenceKind::Unexplained, &Field::ALL));
    let mut basic = case(
        "basic_run.prg",
        record(0x0801, 0x2000, Unp64Entry::BasicRun, &image),
        Some(signed(DivergenceKind::Unexplained, &[Field::Entry])),
    );
    basic.max_instructions = NonZeroU64::new(1);
    Corpus {
        groups: vec![
            Group {
                id: "mule".to_string(),
                original: "c64_mule_1100.prg".to_string(),
                entry: HexAddr(0x1100),
            },
            Group {
                id: "moving_tubes".to_string(),
                original: "c64_moving_tubes_lxt_2e00.prg".to_string(),
                entry: HexAddr(0x2E00),
            },
        ],
        cases: vec![traveller, dali, hbfs, basic],
    }
}

// ═══════════════════════ parsing ═══════════════════════

/// T1.A1.8
#[test]
fn parse_minimal_valid_case() {
    let corpus = Corpus::parse(&manifest_with_unp64("")).unwrap();
    assert!(corpus.groups.is_empty());
    let [case] = corpus.cases.as_slice() else {
        panic!("expected one case, got {:?}", corpus.cases);
    };
    assert_eq!(case.file, "a.prg");
    assert_eq!(case.group, None);
    assert_eq!(case.max_instructions, None);
    assert_eq!(case.expected_fail, None);
    assert_eq!(case.accepted_divergence, None);
    assert_eq!(
        case.unp64,
        Some(Unp64Record::Saved(Unp64Saved {
            start: HexAddr(0x0801),
            end: HexAddr(0x0810),
            entry: Unp64Entry::Address(0x080D),
            len: 16,
            payload_fnv64: Fnv64(0xcbf2_9ce4_8422_2325),
            overlap: None,
            version: "2.42".to_string(),
        }))
    );

    let basic_run = manifest_with_unp64("").replace("\"$080D\"", "\"RUN\"");
    let corpus = Corpus::parse(&basic_run).unwrap();
    let Some(Unp64Record::Saved(saved)) = &corpus.cases[0].unp64 else {
        panic!("expected a saved record");
    };
    assert_eq!(saved.entry, Unp64Entry::BasicRun);
}

/// T1.A1.9
#[test]
fn parse_rejects_unknown_divergence_kind() {
    let message = parse_error(&manifest_with_divergence(
        "kind = \"padding\"\nfields = [\"start\"]\nreviewed_by = \"x\"",
    ));
    for kind in [
        "padding_byte",
        "unp64_default_end",
        "unp64_default_start",
        "unp64_missing_opcode",
        "unexplained",
    ] {
        assert!(
            message.contains(kind),
            "error must list `{kind}`: {message}"
        );
    }
}

/// T1.A1.10
#[test]
fn parse_rejects_unknown_fields_everywhere() {
    let valid_divergence = "kind = \"unexplained\"\nfields = [\"end\"]\nreviewed_by = \"x\"";
    let rows: Vec<(&str, String)> = vec![
        (
            "[[case]]",
            manifest_with_unp64("").replace(
                "file = \"a.prg\"",
                "file = \"a.prg\"\nacepted_divergence = \"typo\"",
            ),
        ),
        ("[case.unp64]", manifest_with_unp64("bogus = 1")),
        (
            "[case.accepted_divergence]",
            manifest_with_divergence(&format!("{valid_divergence}\nreviewd_by = \"typo\"")),
        ),
        (
            "[[group]]",
            format!(
                "[[group]]\nid = \"g\"\noriginal = \"o.prg\"\nentry = \"$1100\"\nextra = 1\n{}",
                manifest_with_unp64("")
            ),
        ),
        (
            "top level",
            format!("extra = 1\n{}", manifest_with_unp64("")),
        ),
        (
            "overlap",
            manifest_with_unp64(
                "overlap = { start = \"$0801\", end = \"$0802\", fnv64 = \"cbf29ce484222325\", x = 1 }",
            ),
        ),
    ];
    // Each base document must be valid without the unknown key.
    assert!(Corpus::parse(&manifest_with_divergence(valid_divergence)).is_ok());

    let accepted: Vec<&str> = rows
        .iter()
        .filter(|(_, text)| Corpus::parse(text).is_ok())
        .map(|(location, _)| *location)
        .collect();
    assert!(
        accepted.is_empty(),
        "unknown keys accepted in: {accepted:?}"
    );
}

/// T1.A1.11
#[test]
fn parse_rejects_disallowed_divergence_field_names() {
    let mut failures = Vec::new();
    for bad in ["len", "ending", "payload", "payload_fnv64"] {
        let message = parse_error(&manifest_with_divergence(&format!(
            "kind = \"unexplained\"\nfields = [\"{bad}\"]\nreviewed_by = \"x\""
        )));
        let lists_allowed = ["`start`", "`end`", "`entry`"]
            .iter()
            .all(|allowed| message.contains(allowed));
        if !lists_allowed {
            failures.push(format!("{bad}: {message}"));
        }
    }
    // `len` next to a valid field is still rejected.
    assert!(
        Corpus::parse(&manifest_with_divergence(
            "kind = \"padding_byte\"\nfields = [\"start\", \"len\"]\nreviewed_by = \"x\""
        ))
        .is_err()
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// T1.A1.12
#[test]
fn parse_rejects_payload_in_divergence_fields() {
    let message = parse_error(&manifest_with_divergence(
        "kind = \"unexplained\"\nfields = [\"payload\"]\nreviewed_by = \"x\"",
    ));
    assert!(
        message.contains("the overlap payload must always match"),
        "{message}"
    );
}

/// T1.A1.13
#[test]
fn parse_rejects_empty_or_duplicate_divergence_fields() {
    let empty = parse_error(&manifest_with_divergence(
        "kind = \"unexplained\"\nfields = []\nreviewed_by = \"x\"",
    ));
    assert!(empty.contains("at least one"), "{empty}");
    let duplicate = parse_error(&manifest_with_divergence(
        "kind = \"unexplained\"\nfields = [\"end\", \"end\"]\nreviewed_by = \"x\"",
    ));
    assert!(duplicate.contains("more than once"), "{duplicate}");
}

/// T1.A1.14 (extended by ruling R17): both closed reasons parse and
/// round-trip; any other value is rejected with the allowed list.
#[test]
fn parse_rejects_unknown_expected_fail_reason() {
    let with_reason = |reason: &str| {
        manifest_with_unp64("").replace(
            "file = \"a.prg\"",
            &format!("file = \"a.prg\"\nexpected_fail = \"{reason}\""),
        )
    };
    for (text, reason) in [
        ("padding_convention", ExpectedFailReason::PaddingConvention),
        (
            "payload_mismatch_under_investigation",
            ExpectedFailReason::PayloadMismatchUnderInvestigation,
        ),
    ] {
        let corpus = Corpus::parse(&with_reason(text)).unwrap();
        assert_eq!(corpus.cases[0].expected_fail, Some(reason));
        assert_eq!(reason.as_str(), text);
        let written = corpus.to_toml_string().unwrap();
        assert!(
            written.contains(&format!("expected_fail = \"{text}\"")),
            "{written}"
        );
        assert_eq!(Corpus::parse(&written).unwrap(), corpus);
    }
    let message = parse_error(&with_reason("flaky"));
    assert!(
        message.contains("padding_convention")
            && message.contains("payload_mismatch_under_investigation"),
        "{message}"
    );
}

/// T1.A1.15
#[test]
fn parse_unp64_failed_shape() {
    let failed = "[[case]]\nfile = \"a.prg\"\n[case.unp64]\nfailed = \"Max Iterations\"\nversion = \"2.42\"\n";
    let corpus = Corpus::parse(failed).unwrap();
    assert_eq!(corpus.cases[0].unp64, Some(failed_record("Max Iterations")));

    let conflicting = [
        "start = \"$0801\"",
        "end = \"$0810\"",
        "entry = \"$080D\"",
        "len = 16",
        "payload_fnv64 = \"cbf29ce484222325\"",
    ];
    let mut failures = Vec::new();
    for extra in conflicting {
        match Corpus::parse(&format!("{failed}{extra}\n")) {
            Err(CorpusError::Parse { message }) if message.contains("failed") => {}
            other => failures.push(format!("{extra}: {other:?}")),
        }
    }
    // A saved record missing a result key names the key.
    let missing_len = manifest_with_unp64("").replace("len = 16\n", "");
    let message = parse_error(&missing_len);
    if !message.contains("len") {
        failures.push(format!("missing len: {message}"));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// T1.A1.16
#[test]
fn parse_overlap_table() {
    let corpus = Corpus::parse(&manifest_with_unp64(
        "overlap = { start = \"$0801\", end = \"$0805\", fnv64 = \"cbf29ce484222325\" }",
    ))
    .unwrap();
    let Some(Unp64Record::Saved(saved)) = &corpus.cases[0].unp64 else {
        panic!("expected a saved record");
    };
    assert_eq!(
        saved.overlap,
        Some(Overlap {
            start: HexAddr(0x0801),
            end: HexAddr(0x0805),
            fnv64: Fnv64(0xcbf2_9ce4_8422_2325),
        })
    );

    let partial = [
        "overlap = { end = \"$0805\", fnv64 = \"cbf29ce484222325\" }",
        "overlap = { start = \"$0801\", fnv64 = \"cbf29ce484222325\" }",
        "overlap = { start = \"$0801\", end = \"$0805\" }",
    ];
    let accepted: Vec<&str> = partial
        .into_iter()
        .filter(|extra| Corpus::parse(&manifest_with_unp64(extra)).is_ok())
        .collect();
    assert!(
        accepted.is_empty(),
        "partial overlaps accepted: {accepted:?}"
    );
}

/// T1.A1.17
#[test]
fn parse_max_instructions_per_case() {
    let with_budget = |budget: &str| {
        manifest_with_unp64("").replace(
            "file = \"a.prg\"",
            &format!("file = \"a.prg\"\nmax_instructions = {budget}"),
        )
    };
    let corpus = Corpus::parse(&with_budget("350000000")).unwrap();
    assert_eq!(corpus.cases[0].effective_max_instructions(), 350_000_000);
    assert!(Corpus::parse(&with_budget("0")).is_err());

    let default_case = Corpus::parse(&manifest_with_unp64("")).unwrap();
    assert_eq!(
        default_case.cases[0].effective_max_instructions(),
        DEFAULT_MAX_INSTRUCTIONS
    );
    assert_eq!(
        DEFAULT_MAX_INSTRUCTIONS,
        UnpackConfig::default().max_instructions
    );
}

/// `review_state` classifies missing, blank, marker and named signatures.
#[test]
fn review_state_classification() {
    let with = |reviewed_by: Option<&str>| {
        divergence(DivergenceKind::Unexplained, &[Field::End], reviewed_by).review_state()
    };
    assert_eq!(with(None), ReviewState::Unsigned);
    assert_eq!(with(Some("")), ReviewState::Unsigned);
    assert_eq!(with(Some(" \t ")), ReviewState::Unsigned);
    assert_eq!(with(Some(INITIAL_REVIEW_MARKER)), ReviewState::PendingUser);
    assert_eq!(with(Some("ricardoq")), ReviewState::Signed);
}

// ═══════════════════════ writer ═══════════════════════

/// T1.A1.62
#[test]
fn manifest_writer_is_idempotent() {
    let first = full_corpus().to_toml_string().unwrap();
    let parsed = Corpus::parse(&first).unwrap();
    let second = parsed.to_toml_string().unwrap();
    assert_eq!(first, second);

    // Parsing the written form yields the same model (up to canonical order).
    let mut expected = full_corpus();
    expected.groups.sort_by(|a, b| a.id.cmp(&b.id));
    expected.cases.sort_by(|a, b| a.file.cmp(&b.file));
    assert_eq!(parsed, expected);

    // Non-canonical input (lowercase hex, unsorted fields) normalizes once.
    let messy = manifest_with_divergence(
        "kind = \"unexplained\"\nfields = [\"entry\", \"start\"]\nreviewed_by = \"x\"",
    )
    .replace("$080D", "$080d");
    let normalized = Corpus::parse(&messy).unwrap().to_toml_string().unwrap();
    assert!(
        normalized.contains("fields = [\"start\", \"entry\"]"),
        "{normalized}"
    );
    assert_eq!(
        Corpus::parse(&normalized)
            .unwrap()
            .to_toml_string()
            .unwrap(),
        normalized
    );
}

/// T1.A1.63
#[test]
fn manifest_writer_sorts_cases_and_groups() {
    let written = full_corpus().to_toml_string().unwrap();
    let order = |needles: &[&str]| -> Vec<usize> {
        needles
            .iter()
            .map(|needle| written.find(needle).unwrap())
            .collect()
    };
    let groups = order(&["id = \"moving_tubes\"", "id = \"mule\""]);
    assert!(groups[0] < groups[1], "{written}");
    let cases = order(&[
        "file = \"basic_run.prg\"",
        "file = \"c64_HBFS.exo3.prg\"",
        "file = \"c64_mule.dali.prg\"",
        "file = \"c64_traveller.tiny_crunch.prg\"",
    ]);
    assert!(cases.windows(2).all(|w| w[0] < w[1]), "{written}");
    assert!(
        groups[1] < cases[0],
        "groups are written before cases:\n{written}"
    );
}

// ═══════════════════════ robustness ═══════════════════════

/// TX.6: the manifest and baseline parsers return `Ok` or a typed error for
/// random bytes and for random single-byte corruptions of a valid manifest,
/// and never panic.
#[test]
fn corpus_parse_never_panics_on_garbage() {
    let valid = full_corpus().to_toml_string().unwrap();
    let valid_bytes = valid.as_bytes();
    let mut inputs: Vec<String> = Vec::new();

    for seed in 1..=200u32 {
        let len = usize::from(pseudo_random_bytes(seed, 1)[0]) * 4;
        inputs.push(String::from_utf8_lossy(&pseudo_random_bytes(seed, len)).into_owned());
    }
    for seed in 1..=400u32 {
        let noise = pseudo_random_bytes(seed ^ 0xA5A5_A5A5, 4);
        let position = (usize::from(noise[0]) << 8 | usize::from(noise[1])) % valid_bytes.len();
        let mut corrupted = valid_bytes.to_vec();
        match noise[2] % 3 {
            0 => corrupted[position] = noise[3],
            1 => {
                corrupted.remove(position);
            }
            _ => corrupted.truncate(position),
        }
        inputs.push(String::from_utf8_lossy(&corrupted).into_owned());
    }

    let panicking: Vec<usize> = inputs
        .iter()
        .enumerate()
        .filter(|(_, text)| {
            catch_unwind(AssertUnwindSafe(|| {
                // Either outcome is fine; only a panic is a failure.
                (Corpus::parse(text).is_ok(), Baseline::parse(text).is_ok())
            }))
            .is_err()
        })
        .map(|(index, _)| index)
        .collect();
    assert!(
        panicking.is_empty(),
        "parsers panicked on inputs {panicking:?}"
    );
}
