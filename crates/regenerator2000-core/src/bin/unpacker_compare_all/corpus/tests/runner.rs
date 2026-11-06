//! Tests for `runner`: the per-case unpack configuration, fixture errors,
//! and the equivalence of serial and parallel scheduling.
//!
//! All fixtures reside in memory.

use super::*;

/// Builds a case for `file` with an optional budget and no oracle data.
fn bare_case(file: &str, max_instructions: Option<u64>) -> Case {
    Case {
        file: file.to_string(),
        group: None,
        max_instructions: max_instructions.and_then(std::num::NonZeroU64::new),
        expected_fail: None,
        unp64: None,
        accepted_divergence: None,
    }
}

/// Checks that `unpack_config_for` uses the case instruction budget, or the
/// default budget when the case has none.
#[test]
fn bin_uses_case_max_instructions() {
    assert_eq!(
        unpack_config_for(&bare_case("a.prg", Some(350_000_000))).max_instructions,
        350_000_000
    );
    assert_eq!(
        unpack_config_for(&bare_case("a.prg", None)).max_instructions,
        DEFAULT_MAX_INSTRUCTIONS
    );
    // Every other field stays at the engine default.
    let config = unpack_config_for(&bare_case("a.prg", Some(7)));
    assert_eq!(
        regenerator2000_core::unpacker::UnpackConfig {
            max_instructions: 7,
            ..config.clone()
        },
        regenerator2000_core::unpacker::UnpackConfig {
            max_instructions: 7,
            ..regenerator2000_core::unpacker::UnpackConfig::default()
        }
    );
}

/// Checks that `run_case` reports a missing fixture or a fixture shorter than
/// the PRG header as an error.
#[test]
fn run_case_reports_unreadable_fixtures() {
    let fixtures = MemFixtures::default().with("short.prg", vec![0x01]);
    assert_eq!(
        run_case(&bare_case("missing.prg", None), &fixtures).map(|r| r.file),
        Err(CorpusError::MissingFixture {
            file: "missing.prg".to_string()
        })
    );
    assert!(matches!(
        run_case(&bare_case("short.prg", None), &fixtures),
        Err(CorpusError::InvalidFixture { file, .. }) if file == "short.prg"
    ));
}

/// Checks that `run_case` reads the load address from the PRG header and
/// keeps an unpack error in the result for the oracles to evaluate.
#[test]
fn run_case_keeps_unpack_errors_in_the_result() {
    // A lone `BRK` at $C000 has no BASIC SYS line, so the engine finds no
    // entry point.
    let fixtures = MemFixtures::default().with("brk.prg", prg(0xC000, &[0x00]));
    let result = run_case(&bare_case("brk.prg", Some(1_000)), &fixtures).unwrap();
    assert_eq!(result.file, "brk.prg");
    assert_eq!(result.load_addr, 0xC000);
    assert_eq!(result.system, System::C64);
    let error = result.outcome.unwrap_err();
    assert!(!error.variant.is_empty(), "{error:?}");
}

/// Checks that serial and parallel runs return identical results in input
/// order and report every fixture error in sorted order.
#[test]
fn run_cases_serial_equals_parallel_and_collects_errors() {
    let fixtures = MemFixtures::default()
        .with("a.prg", prg(0xC000, &[0x00]))
        .with("b.prg", prg(0x0801, &[0x00, 0x00, 0x00]));
    let cases = [
        bare_case("b.prg", Some(1_000)),
        bare_case("a.prg", Some(1_000)),
    ];
    let selected: Vec<&Case> = cases.iter().collect();

    let rows = |schedule| -> Vec<BaselineRow> {
        run_cases(&selected, &fixtures, schedule)
            .unwrap()
            .iter()
            .map(BaselineRow::from_result)
            .collect()
    };
    let serial = rows(Schedule::Serial);
    assert_eq!(serial, rows(Schedule::Parallel));
    let files: Vec<&str> = serial.iter().map(|row| row.file.as_str()).collect();
    assert_eq!(files, ["b.prg", "a.prg"]);

    let broken = [bare_case("z.prg", None), bare_case("y.prg", None)];
    let broken: Vec<&Case> = broken.iter().collect();
    for schedule in [Schedule::Serial, Schedule::Parallel] {
        assert_eq!(
            run_cases(&broken, &fixtures, schedule).map(|results| results.len()),
            Err(vec![
                CorpusError::MissingFixture {
                    file: "y.prg".to_string()
                },
                CorpusError::MissingFixture {
                    file: "z.prg".to_string()
                },
            ])
        );
    }
}
