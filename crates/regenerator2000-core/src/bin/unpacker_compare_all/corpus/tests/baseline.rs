//! Tests for `baseline` (T1.A1.58–T1.A1.61).

use super::*;
use regenerator2000_core::unpacker::UnpackError;
use std::collections::BTreeSet;

/// T1.A1.58
#[test]
fn baseline_row_captures_every_observable_output() {
    let image = image(0x0801, 0x2000, 0x080D);
    let row = BaselineRow::from_result(&ok_result("a.prg", image.clone()));
    assert_eq!(
        row,
        BaselineRow {
            file: "a.prg".to_string(),
            outcome: BaselineOutcome::Unpacked(BaselineUnpacked {
                start: HexAddr(0x0801),
                end: HexAddr(0x2000),
                entry: HexAddr(0x080D),
                dep: HexAddr(0x0100),
                packer: Some("Test Packer".to_string()),
                instructions: 12_345,
                len: image.data.len(),
                fnv64: Fnv64::of(&image.data),
            }),
        }
    );

    let failed = BaselineRow::from_result(&err_result("b.prg", "Phase1Timeout"));
    assert_eq!(
        failed.outcome,
        BaselineOutcome::Failed {
            error: "Phase1Timeout".to_string()
        }
    );

    // The variant name comes from the real error type, without its fields.
    let real = UnpackError::InvalidAddressRange {
        start_addr: 1,
        end_addr: 2,
        entry_point: 3,
    };
    assert_eq!(unpack_error_variant(&real), "InvalidAddressRange");
    assert_eq!(
        unpack_error_variant(&UnpackError::Phase2Timeout),
        "Phase2Timeout"
    );
    let from_unpack = CaseResult::from_unpack("c.prg", 0x0801, System::C64, &Err(real));
    assert_eq!(
        BaselineRow::from_result(&from_unpack).outcome,
        BaselineOutcome::Failed {
            error: "InvalidAddressRange".to_string()
        }
    );

    // The text form round-trips.
    let baseline = Baseline {
        rows: vec![row, failed],
    };
    let text = baseline.to_toml_string().unwrap();
    assert_eq!(Baseline::parse(&text).unwrap(), baseline);
}

/// T1.A1.59
#[test]
fn baseline_compare_detects_each_field_change() {
    let base_image = image(0x0801, 0x2000, 0x080D);
    let base = Baseline::from_results(&[ok_result("a.prg", base_image.clone())]);
    type Mutation = fn(&mut UnpackedImage);
    let rows: [(BaselineField, Mutation); 8] = [
        (BaselineField::Start, |i| {
            i.start += 1;
            i.data.remove(0);
        }),
        (BaselineField::End, |i| i.end = 0x3000),
        (BaselineField::Entry, |i| i.entry = 0x0900),
        (BaselineField::Dep, |i| i.dep = 0x0200),
        (BaselineField::Packer, |i| i.packer = None),
        (BaselineField::Instructions, |i| i.instructions += 1),
        (BaselineField::Len, |i| i.data.push(0)),
        (BaselineField::Fnv64, |i| i.data[0] ^= 0xFF),
    ];
    let mut failures = Vec::new();
    for (field, mutate) in rows {
        let mut changed = base_image.clone();
        mutate(&mut changed);
        let current = Baseline::from_results(&[ok_result("a.prg", changed)]);
        let diffs = base.compare(&current);
        let detected = diffs
            .iter()
            .any(|diff| matches!(diff, BaselineDiff::Changed { field: f, .. } if *f == field));
        if !detected {
            failures.push(format!("{field}: {diffs:?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));

    let errored = Baseline::from_results(&[err_result("a.prg", "Phase1Timeout")]);
    assert!(matches!(
        base.compare(&errored).as_slice(),
        [BaselineDiff::Changed {
            field: BaselineField::Outcome,
            ..
        }]
    ));
    let other_error = Baseline::from_results(&[err_result("a.prg", "Phase2Timeout")]);
    assert!(matches!(
        errored.compare(&other_error).as_slice(),
        [BaselineDiff::Changed {
            field: BaselineField::Error,
            ..
        }]
    ));
    assert_eq!(base.compare(&base), Vec::new());
}

/// T1.A1.60
#[test]
fn baseline_missing_or_extra_row_is_failure() {
    let a = ok_result("a.prg", image(0x0801, 0x2000, 0x080D));
    let b = ok_result("b.prg", image(0x0801, 0x3000, 0x080D));
    let committed = Baseline::from_results(&[a.clone(), b.clone()]);
    let current =
        Baseline::from_results(&[a.clone(), ok_result("c.prg", image(0x0801, 0x2000, 0x080D))]);
    assert_eq!(
        committed.compare(&current),
        vec![
            BaselineDiff::MissingRow {
                file: "b.prg".to_string()
            },
            BaselineDiff::ExtraRow {
                file: "c.prg".to_string()
            },
        ]
    );

    // A filtered run compares only the selected rows.
    let selected: BTreeSet<&str> = ["a.prg"].into_iter().collect();
    let only_a = Baseline::from_results(&[a]);
    assert_eq!(
        committed.restricted_to(&selected).compare(&only_a),
        Vec::new()
    );
}

/// T1.A1.61
#[test]
fn baseline_writer_is_deterministic_and_sorted() {
    let results = [
        ok_result("c.prg", image(0x0801, 0x2000, 0x080D)),
        err_result("a.prg", "NoEntryPoint"),
        ok_result("b.prg", image(0x0800, 0x0900, 0x0810)),
    ];
    let forward = Baseline::from_results(&results).to_toml_string().unwrap();
    let mut reversed = results.to_vec();
    reversed.reverse();
    let backward = Baseline::from_results(&reversed).to_toml_string().unwrap();
    assert_eq!(forward, backward);

    let positions: Vec<usize> = ["\"a.prg\"", "\"b.prg\"", "\"c.prg\""]
        .iter()
        .map(|needle| forward.find(needle).unwrap())
        .collect();
    assert!(positions.windows(2).all(|w| w[0] < w[1]), "{forward}");
    assert!(forward.contains("start = \"$0801\""), "{forward}");
    assert!(forward.contains("error = \"NoEntryPoint\""), "{forward}");

    let reparsed = Baseline::parse(&forward).unwrap().to_toml_string().unwrap();
    assert_eq!(reparsed, forward);
}

/// A baseline row must be either a full result or an error, never a mix or
/// a partial result.
#[test]
fn baseline_parse_rejects_mixed_or_partial_rows() {
    let mixed = "[[row]]\nfile = \"a.prg\"\nerror = \"Phase1Timeout\"\nstart = \"$0801\"\n";
    assert!(matches!(
        Baseline::parse(mixed),
        Err(CorpusError::Parse { message }) if message.contains("cannot be combined")
    ));
    let partial = "[[row]]\nfile = \"a.prg\"\nstart = \"$0801\"\n";
    assert!(matches!(
        Baseline::parse(partial),
        Err(CorpusError::Parse { message }) if message.contains("missing `end`")
    ));
}
