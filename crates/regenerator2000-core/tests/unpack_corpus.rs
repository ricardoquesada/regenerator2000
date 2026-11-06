#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Corpus integration test for `tests/6502/unpack_corpus.toml`.
//!
//! This test runs every case in `tests/6502/unpack_corpus.toml` through Regenerator 2000.
//! It evaluates each result against its strongest oracle (ground-truth original binary,
//! recorded `unp64` reference, or reviewed exception) and checks ground-truth group consensus.
//! It also compares every observable output with `tests/6502/unpack_baseline.toml`.
//!
//! The test collects all failures and reports them together.
//! It shares the `corpus` module with `unpacker_compare_all` via `#[path]`.
//!
//! Environment variables:
//!
//! * `CORPUS_FILTER=mule` runs a subset of cases or groups whose name contains the text.
//!   The runner always executes matched groups as a whole.
//! * `UPDATE_UNPACK_BASELINE=1` rewrites `tests/6502/unpack_baseline.toml` instead of comparing it.
//!   The harness rejects this variable when `CORPUS_FILTER` is active so a partial run never drops unselected rows.
//! * `UNPACK_CORPUS_TIME_LIMIT_SECS` overrides the wall-clock limit of the full parallel run.

#[path = "../src/bin/unpacker_compare_all/corpus/mod.rs"]
mod corpus;

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use corpus::{
    BASELINE_FILE, Baseline, BaselineOutcome, Case, CaseResult, Corpus, CorpusFilter, DirFixtures,
    DivergenceKind, Evaluator, ExpectedFailReason, FIXTURE_DIR, Field, INITIAL_REVIEW_MARKER,
    MANIFEST_FILE, ReviewState, Schedule, render_action_items, render_failure_table, run_cases,
};

/// Environment variable that selects a subset of cases.
const FILTER_ENV: &str = "CORPUS_FILTER";

/// Environment variable that rewrites the baseline when set to [`UPDATE_BASELINE_VALUE`].
const UPDATE_BASELINE_ENV: &str = "UPDATE_UNPACK_BASELINE";

/// Value of [`UPDATE_BASELINE_ENV`] that rewrites the baseline.
const UPDATE_BASELINE_VALUE: &str = "1";

/// Environment variable that overrides [`CORPUS_TIME_LIMIT`].
const TIME_LIMIT_ENV: &str = "UNPACK_CORPUS_TIME_LIMIT_SECS";

/// Wall-clock limit for a full parallel corpus run.
///
/// A local run takes approximately 1 second.
/// A 30-second limit detects infinite loops without false failures on shared 2-vCPU CI runners.
const CORPUS_TIME_LIMIT: Duration = Duration::from_secs(30);

/// Alphabetical list of packed `.prg` fixtures in `tests/6502/unpack_corpus.toml`.
const CORPUS_CASES: [&str; 45] = [
    "c64_323_ice_psm.1001_card_cruncher.prg",
    "c64_8_bit_ball.meanteam_cruncher.prg",
    "c64_Bit_by_Bits-BZ!.exo3.prg",
    "c64_CopperBooze.byte_boozer2.prg",
    "c64_FppScroller.byte_boozer2.prg",
    "c64_HBFS.exo3.prg",
    "c64_Layers.exo3.prg",
    "c64_bluemarble4k_unk.prg",
    "c64_boilerplate.exo3.prg",
    "c64_boo_alz64.prg",
    "c64_chiller.antiram_packer.prg",
    "c64_connection-8580.pucrunch.prg",
    "c64_cubicdream.exo3.prg",
    "c64_daah_those_acid_pills.exo2.prg",
    "c64_druid_too.exo3.prg",
    "c64_endoskull.exo3.prg",
    "c64_f600.exo.prg",
    "c64_fantasy_intro.eca_compactor.prg",
    "c64_gianna_sister_remix_badboy.tbc_multicompactor.prg",
    "c64_goblin.unk.prg",
    "c64_hw20131031.exo.prg",
    "c64_leftovers-pl.exo3.prg",
    "c64_lft-nine.exo3.prg",
    "c64_lft-rodents-in-the-attic.exo3.prg",
    "c64_little_things.exo3.prg",
    "c64_moving_tubes_lxt.dali.prg",
    "c64_moving_tubes_lxt.exo3.prg",
    "c64_moving_tubes_lxt.pucrunch.prg",
    "c64_moving_tubes_lxt.tscrunch_x.prg",
    "c64_moving_tubes_lxt.tscrunch_x2.prg",
    "c64_mule.dali.prg",
    "c64_mule.exo3.prg",
    "c64_mule.mccracken_compressor.prg",
    "c64_mule.pucrunch.prg",
    "c64_mule.tscrunch_x.prg",
    "c64_mule.tscrunch_x2.prg",
    "c64_radiant-every_time_i_go_on_pouet.byte_boozer2prg.prg",
    "c64_robot - not human.exo3.prg",
    "c64_roma.exe.exo3.prg",
    "c64_soul_on_fire_unk.prg",
    "c64_spectro.exo3.prg",
    "c64_sprite_runners.exo3.prg",
    "c64_thats_the_way_scoop.time_cruncher.prg",
    "c64_traveller.tiny_crunch.prg",
    "c64_wavehero_alz64.prg",
];

/// Ground-truth groups: `(group_id, original_unpacked_prg, entry_point, packed_members)`.
const GROUND_TRUTH_GROUPS: [(&str, &str, u16, &[&str]); 2] = [
    (
        "moving_tubes",
        "c64_moving_tubes_lxt_2e00.prg",
        0x2E00,
        &[
            "c64_moving_tubes_lxt.dali.prg",
            "c64_moving_tubes_lxt.exo3.prg",
            "c64_moving_tubes_lxt.pucrunch.prg",
            "c64_moving_tubes_lxt.tscrunch_x.prg",
            "c64_moving_tubes_lxt.tscrunch_x2.prg",
        ],
    ),
    (
        "mule",
        "c64_mule_1100.prg",
        0x1100,
        &[
            "c64_mule.dali.prg",
            "c64_mule.exo3.prg",
            "c64_mule.mccracken_compressor.prg",
            "c64_mule.pucrunch.prg",
            "c64_mule.tscrunch_x.prg",
            "c64_mule.tscrunch_x2.prg",
        ],
    ),
];

/// Generic 2-phase emulation cases (`packer = None`) with a range divergence against `unp64`.
const UNKNOWN_PACKER_EXCEPTIONS: [&str; 4] = [
    "c64_8_bit_ball.meanteam_cruncher.prg",
    "c64_bluemarble4k_unk.prg",
    "c64_goblin.unk.prg",
    "c64_soul_on_fire_unk.prg",
];

/// Multi-layer Exomizer 3 case (`c64_HBFS.exo3.prg`) with an outer-vs-inner payload difference against `unp64`.
const HBFS: &str = "c64_HBFS.exo3.prg";

/// Cases with [`ExpectedFailReason::PayloadMismatchUnderInvestigation`].
const PAYLOAD_MISMATCH_CASES: [&str; 1] = ["c64_HBFS.exo3.prg"];

// =============================================================================
// Harness
// =============================================================================

/// Action that the corpus test performs on the baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BaselineMode {
    /// Compare the current run with the committed baseline.
    Compare,
    /// Rewrite the committed baseline from the current run.
    Update,
}

/// Determines the baseline mode from [`UPDATE_BASELINE_ENV`].
///
/// This function rejects updates when `filter` is active so a filtered run never drops unselected rows.
fn baseline_mode(update: Option<&str>, filter: &CorpusFilter) -> Result<BaselineMode, String> {
    match update {
        None | Some("") => Ok(BaselineMode::Compare),
        Some(UPDATE_BASELINE_VALUE) if filter.is_active() => Err(format!(
            "{UPDATE_BASELINE_ENV} cannot be combined with {FILTER_ENV}: a filtered run would \
             drop the unselected rows from the baseline. Unset {FILTER_ENV} to regenerate it."
        )),
        Some(UPDATE_BASELINE_VALUE) => Ok(BaselineMode::Update),
        Some(other) => Err(format!(
            "{UPDATE_BASELINE_ENV}={other:?} is not understood; use \
             {UPDATE_BASELINE_ENV}={UPDATE_BASELINE_VALUE} or leave it unset"
        )),
    }
}

/// Parses the wall-clock limit from [`TIME_LIMIT_ENV`].
fn time_limit(value: Option<&str>) -> Result<Duration, String> {
    let Some(text) = value.map(str::trim).filter(|text| !text.is_empty()) else {
        return Ok(CORPUS_TIME_LIMIT);
    };
    match text.parse::<u64>() {
        Ok(seconds) if seconds > 0 => Ok(Duration::from_secs(seconds)),
        _ => Err(format!(
            "{TIME_LIMIT_ENV}={text:?} must be a positive number of seconds"
        )),
    }
}

/// Returns the value of environment variable `name` when set and valid Unicode.
fn env_value(name: &str) -> Option<String> {
    env::var(name).ok()
}

/// Builds the case filter from [`FILTER_ENV`].
fn env_filter() -> CorpusFilter {
    CorpusFilter::from_env_value(env_value(FILTER_ENV).as_deref())
}

/// Returns the fixture directory loader.
fn fixtures() -> DirFixtures {
    DirFixtures::new(FIXTURE_DIR)
}

/// Loads the manifest and asserts that it validates with zero errors before evaluation.
fn load_valid_corpus() -> Corpus {
    let path = Path::new(FIXTURE_DIR).join(MANIFEST_FILE);
    let corpus = Corpus::load(&path).unwrap_or_else(|error| panic!("{error}"));
    let errors = corpus.validate(&fixtures());
    assert!(
        errors.is_empty(),
        "{} manifest error(s):\n{}",
        errors.len(),
        errors
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    );
    corpus
}

/// Runs `cases` through R2000.
fn run(cases: &[&Case], schedule: Schedule) -> Vec<CaseResult> {
    run_cases(cases, &fixtures(), schedule).unwrap_or_else(|errors| {
        panic!(
            "cannot run the corpus:\n{}",
            errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    })
}

/// Returns the cases selected by `filter`.
fn select<'c>(corpus: &'c Corpus, filter: &CorpusFilter) -> Vec<&'c Case> {
    filter
        .select(corpus)
        .unwrap_or_else(|error| panic!("{error}"))
}

// =============================================================================
// Manifest shape & invariants
// =============================================================================

/// Verifies that `tests/6502/` resolves from `CARGO_MANIFEST_DIR` and contains both TOML files.
#[test]
fn corpus_fixture_dir_resolves_from_manifest_dir() {
    let dir = Path::new(FIXTURE_DIR);
    assert!(dir.is_dir(), "{} is not a directory", dir.display());
    for file in [MANIFEST_FILE, BASELINE_FILE] {
        assert!(
            dir.join(file).is_file(),
            "{file} is missing in {}",
            dir.display()
        );
    }
}

/// Verifies that `tests/6502/unpack_corpus.toml` parses, validates, and has `[case.unp64]` for every case.
#[test]
fn corpus_manifest_loads_and_validates() {
    let corpus = load_valid_corpus();
    assert!(!corpus.cases.is_empty());
    let missing: Vec<&str> = corpus
        .cases
        .iter()
        .filter(|case| case.unp64.is_none())
        .map(|case| case.file.as_str())
        .collect();
    assert!(
        missing.is_empty(),
        "cases without [case.unp64]: {missing:?}"
    );
}

/// Verifies that `tests/6502/unpack_corpus.toml` contains the exact fixtures in [`CORPUS_CASES`].
#[test]
fn corpus_contains_expected_cases() {
    let corpus = load_valid_corpus();
    let expected: BTreeSet<&str> = CORPUS_CASES.into_iter().collect();
    assert_eq!(
        expected.len(),
        CORPUS_CASES.len(),
        "duplicate entry in CORPUS_CASES"
    );
    let actual: BTreeSet<&str> = corpus.cases.iter().map(|case| case.file.as_str()).collect();
    let missing: Vec<&&str> = expected.difference(&actual).collect();
    let extra: Vec<&&str> = actual.difference(&expected).collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "missing from unpack_corpus.toml: {missing:?}\nunlisted in CORPUS_CASES: {extra:?}"
    );
    assert!(
        corpus.group("moving_tubes").is_some(),
        "the moving_tubes group is missing"
    );
}

/// Verifies that the `moving_tubes` and `mule` groups reference their original binaries and cover 11 variants.
#[test]
fn corpus_ground_truth_groups_cover_eleven_variants() {
    let corpus = load_valid_corpus();
    let mut covered = 0;
    for (id, original, entry, members) in GROUND_TRUTH_GROUPS {
        let group = corpus
            .group(id)
            .unwrap_or_else(|| panic!("group `{id}` is missing"));
        assert_eq!(group.original, original, "group `{id}`");
        assert_eq!(group.entry.0, entry, "group `{id}`");
        let actual: BTreeSet<&str> = corpus.members(id).map(|case| case.file.as_str()).collect();
        let expected: BTreeSet<&str> = members.iter().copied().collect();
        assert_eq!(actual, expected, "members of group `{id}`");
        covered += actual.len();
    }
    assert_eq!(covered, 11);
    assert_eq!(corpus.groups.len(), GROUND_TRUTH_GROUPS.len());
}

/// Verifies that every `accepted_divergence` is signed and that action items list pending and expected-fail cases.
#[test]
fn corpus_exceptions_are_signed_and_listed_in_action_items() {
    let corpus = load_valid_corpus();
    let with_exception: Vec<&Case> = corpus
        .cases
        .iter()
        .filter(|case| case.accepted_divergence.is_some())
        .collect();
    assert!(!with_exception.is_empty());

    let mut pending = BTreeSet::new();
    for case in &with_exception {
        let Some(exception) = &case.accepted_divergence else {
            continue;
        };
        match exception.review_state() {
            ReviewState::Unsigned => panic!("{}: unsigned accepted_divergence", case.file),
            ReviewState::PendingUser => {
                pending.insert(case.file.as_str());
            }
            ReviewState::Signed => {}
        }
    }

    let results = run(&with_exception, Schedule::Parallel);
    let evaluator = Evaluator::new(&corpus, &fixtures()).unwrap();
    let outcome = evaluator.evaluate_all(&results);
    let action_items = render_action_items(&outcome.reports);
    let unlisted: Vec<&&str> = pending
        .iter()
        .filter(|file| {
            !action_items
                .lines()
                .any(|line| line.starts_with(&format!("- {file}: exception signed")))
        })
        .collect();
    assert!(
        unlisted.is_empty(),
        "pending exceptions missing from the action items: {unlisted:?}\n{action_items}"
    );
    assert!(action_items.contains(INITIAL_REVIEW_MARKER));
    let expected_fail_unlisted: Vec<&str> = PAYLOAD_MISMATCH_CASES
        .into_iter()
        .filter(|file| {
            !action_items.lines().any(|line| {
                line.starts_with(&format!("- {file}: expected failure"))
                    && line.contains(ExpectedFailReason::PayloadMismatchUnderInvestigation.as_str())
            })
        })
        .collect();
    assert!(
        expected_fail_unlisted.is_empty(),
        "expected-fail cases missing from the action items: {expected_fail_unlisted:?}\n{action_items}"
    );

    let unknown_packer: BTreeSet<&str> = results
        .iter()
        .filter(|result| {
            result
                .outcome
                .as_ref()
                .is_ok_and(|image| image.packer.is_none())
        })
        .map(|result| result.file.as_str())
        .collect();
    assert_eq!(
        unknown_packer,
        UNKNOWN_PACKER_EXCEPTIONS.into_iter().collect(),
        "unknown-packer cases with an accepted_divergence"
    );
}

/// Verifies that `c64_HBFS.exo3.prg` is the only `payload_mismatch_under_investigation` case and has an exception for `start` and `entry`.
#[test]
fn corpus_hbfs_payload_mismatch_is_under_investigation() {
    let corpus = load_valid_corpus();
    let hbfs = corpus.case(HBFS).expect("HBFS is in the corpus");
    assert_eq!(
        hbfs.expected_fail,
        Some(ExpectedFailReason::PayloadMismatchUnderInvestigation)
    );
    let exception = hbfs
        .accepted_divergence
        .as_ref()
        .expect("HBFS needs an exception for its start and entry");
    assert_eq!(exception.kind, DivergenceKind::Unexplained);
    assert_eq!(exception.fields.as_slice(), &[Field::Start, Field::Entry]);
    assert!(
        exception
            .note
            .as_deref()
            .is_some_and(|note| !note.trim().is_empty()),
        "{exception:?}"
    );

    let marked: BTreeSet<&str> = corpus
        .cases
        .iter()
        .filter(|case| {
            case.expected_fail == Some(ExpectedFailReason::PayloadMismatchUnderInvestigation)
        })
        .map(|case| case.file.as_str())
        .collect();
    assert_eq!(marked, PAYLOAD_MISMATCH_CASES.into_iter().collect());
}

// =============================================================================
// Oracles, baseline, determinism and budget
// =============================================================================

/// Runs every selected corpus case, checks its oracle and group consensus, and compares or updates `tests/6502/unpack_baseline.toml`.
#[test]
fn unpack_corpus_matches_oracles_and_baseline() {
    let corpus = load_valid_corpus();
    let filter = env_filter();
    let mode = baseline_mode(env_value(UPDATE_BASELINE_ENV).as_deref(), &filter)
        .unwrap_or_else(|error| panic!("{error}"));
    let selected = select(&corpus, &filter);
    let evaluator = Evaluator::new(&corpus, &fixtures()).unwrap_or_else(|errors| {
        panic!(
            "{}",
            errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });

    let results = run(&selected, Schedule::Parallel);
    let outcome = evaluator.evaluate_all(&results);
    let current = Baseline::from_results(&results);
    let baseline_path = Path::new(FIXTURE_DIR).join(BASELINE_FILE);

    let baseline_diffs: Vec<String> = match mode {
        BaselineMode::Update => {
            fs::write(&baseline_path, current.to_toml_string().unwrap()).unwrap();
            Vec::new()
        }
        BaselineMode::Compare => {
            let committed = Baseline::load(&baseline_path).unwrap_or_else(|e| panic!("{e}"));
            let files: BTreeSet<&str> = selected.iter().map(|case| case.file.as_str()).collect();
            let committed = if filter.is_active() {
                committed.restricted_to(&files)
            } else {
                committed
            };
            committed
                .compare(&current)
                .iter()
                .map(ToString::to_string)
                .collect()
        }
    };

    // Print action items (unexplained exceptions, pending reviews, and expected failures) on every run.
    let action_items = render_action_items(&outcome.reports);
    if !action_items.is_empty() {
        eprintln!("{action_items}");
    }
    assert!(
        outcome.is_green() && baseline_diffs.is_empty(),
        "{}\n\n{} baseline difference(s){}\n{}",
        render_failure_table(&outcome.failures),
        baseline_diffs.len(),
        if baseline_diffs.is_empty() {
            ""
        } else {
            " (intentional? regenerate with UPDATE_UNPACK_BASELINE=1 and review the git diff):"
        },
        baseline_diffs.join("\n")
    );
}

/// Verifies that `baseline_mode` rejects updates when `CORPUS_FILTER` is active and accepts only `UPDATE_UNPACK_BASELINE=1`.
#[test]
fn corpus_update_with_filter_is_rejected() {
    let all = CorpusFilter::All;
    let mule = CorpusFilter::from_env_value(Some("mule"));
    assert_eq!(baseline_mode(None, &mule), Ok(BaselineMode::Compare));
    assert_eq!(baseline_mode(Some(""), &all), Ok(BaselineMode::Compare));
    assert_eq!(baseline_mode(Some("1"), &all), Ok(BaselineMode::Update));
    let rejected = baseline_mode(Some("1"), &mule).unwrap_err();
    assert!(
        rejected.contains(UPDATE_BASELINE_ENV) && rejected.contains(FILTER_ENV),
        "{rejected}"
    );
    assert!(baseline_mode(Some("yes"), &all).is_err());
}

/// Verifies that serial and parallel corpus runs produce identical observable outputs for every case.
#[test]
fn corpus_runs_are_deterministic_serial_vs_parallel() {
    let corpus = load_valid_corpus();
    let selected = select(&corpus, &env_filter());
    let serial = Baseline::from_results(&run(&selected, Schedule::Serial));
    let parallel = Baseline::from_results(&run(&selected, Schedule::Parallel));
    let diffs = serial.compare(&parallel);
    assert!(
        diffs.is_empty(),
        "{}",
        diffs
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert_eq!(serial.rows.len(), selected.len());
    // Verify that every case produces a non-empty outcome row.
    assert!(serial.rows.iter().all(|row| match &row.outcome {
        BaselineOutcome::Unpacked(unpacked) => unpacked.len > 0,
        BaselineOutcome::Failed { error } => !error.is_empty(),
    }));
}

/// Verifies that a full parallel corpus run finishes within the wall-clock limit when `CORPUS_FILTER` is unset.
#[test]
fn corpus_wall_clock_limit() {
    let filter = env_filter();
    if filter.is_active() {
        eprintln!("corpus_wall_clock_limit: skipped because {FILTER_ENV} is set");
        return;
    }
    let limit =
        time_limit(env_value(TIME_LIMIT_ENV).as_deref()).unwrap_or_else(|error| panic!("{error}"));
    let corpus = load_valid_corpus();
    let selected = select(&corpus, &filter);
    let started = Instant::now();
    let results = run(&selected, Schedule::Parallel);
    let elapsed = started.elapsed();
    assert_eq!(results.len(), corpus.cases.len());
    assert!(
        elapsed <= limit,
        "the full corpus run took {elapsed:?}, above the {limit:?} limit: investigate the \
         slowdown (or set {TIME_LIMIT_ENV} on a slow machine)"
    );
}

/// Verifies that `UNPACK_CORPUS_TIME_LIMIT_SECS` defaults to [`CORPUS_TIME_LIMIT`] and accepts only positive integers.
#[test]
fn corpus_time_limit_parsing() {
    assert_eq!(time_limit(None), Ok(CORPUS_TIME_LIMIT));
    assert_eq!(time_limit(Some(" ")), Ok(CORPUS_TIME_LIMIT));
    assert_eq!(time_limit(Some("30")), Ok(Duration::from_secs(30)));
    for bad in ["0", "-1", "ten", "1.5"] {
        assert!(time_limit(Some(bad)).is_err(), "{bad}");
    }
}

/// Verifies that every named packer in `unpack_baseline.toml` maps to an entry in `packers::registry::REGISTRY`.
#[test]
fn every_corpus_packer_name_maps_to_a_registered_spec() {
    let baseline_path = Path::new(FIXTURE_DIR).join(BASELINE_FILE);
    let baseline = Baseline::load(&baseline_path).unwrap_or_else(|e| panic!("{e}"));
    let mut packer_names = BTreeSet::new();
    for row in &baseline.rows {
        if let BaselineOutcome::Unpacked(unpacked) = &row.outcome
            && let Some(name) = unpacked.packer.as_deref()
        {
            packer_names.insert(name);
        }
    }
    assert!(
        !packer_names.is_empty(),
        "baseline must contain at least one named packer"
    );
    for name in packer_names {
        assert!(
            regenerator2000_core::packers::registry::find_entry_by_name(name).is_some(),
            "baseline packer name {name:?} does not match any entry in REGISTRY"
        );
    }
}

/// Verifies that `registry::detect` and `registry::detect_all` match the packer name in `unpack_baseline.toml`.
#[test]
fn detect_all_over_corpus_matches_baseline_packer() {
    use regenerator2000_core::packers::{LoadedImage, registry};

    let corpus = load_valid_corpus();
    let baseline_path = Path::new(FIXTURE_DIR).join(BASELINE_FILE);
    let baseline = Baseline::load(&baseline_path).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(corpus.cases.len(), baseline.rows.len());

    for case in &corpus.cases {
        let row = baseline
            .rows
            .iter()
            .find(|r| r.file == case.file)
            .unwrap_or_else(|| panic!("missing baseline row for {}", case.file));
        let expected_packer = match &row.outcome {
            BaselineOutcome::Unpacked(unpacked) => unpacked.packer.as_deref(),
            BaselineOutcome::Failed { .. } => None,
        };

        let prg_path = Path::new(FIXTURE_DIR).join(&case.file);
        let prg = fs::read(&prg_path).unwrap_or_else(|e| panic!("{}: {e}", case.file));
        assert!(prg.len() >= 2, "{}: PRG shorter than 2 bytes", case.file);
        let load_addr = u16::from_le_bytes([prg[0], prg[1]]);
        let payload = &prg[2..];
        let mut mem = vec![0u8; 65536];
        let start = usize::from(load_addr);
        let copy_len = payload.len().min(65536usize.saturating_sub(start));
        mem[start..start + copy_len].copy_from_slice(&payload[..copy_len]);
        let load_end = load_addr.wrapping_add(copy_len as u16);

        let image = LoadedImage::for_c64(&mem, load_addr, load_end);
        let all_matches = registry::detect_all(&image);
        let first_match = registry::detect(&image);
        assert_eq!(
            first_match.as_ref().map(|m| m.spec_name),
            all_matches.first().map(|m| m.spec_name),
            "{}: detect vs detect_all first match mismatch",
            case.file
        );
        assert_eq!(
            first_match.as_ref().map(|m| m.spec_name),
            expected_packer,
            "{}: detected packer mismatch against baseline",
            case.file
        );
    }
}

// =============================================================================
// Documentation synchronization (`docs/unpacker.md`)
// =============================================================================

/// Path to `docs/unpacker.md` resolved from `CARGO_MANIFEST_DIR`.
const DOCS_UNPACKER_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/unpacker.md");

/// Start marker for the manifest-synced divergence table in `docs/unpacker.md`.
const DOCS_TABLE_BEGIN: &str = "<!-- BEGIN CORPUS DIVERGENCE TABLE -->";

/// End marker for the manifest-synced divergence table in `docs/unpacker.md`.
const DOCS_TABLE_END: &str = "<!-- END CORPUS DIVERGENCE TABLE -->";

/// One row of the divergence table in `docs/unpacker.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DocDivergenceRow {
    file: String,
    unp64: String,
    r2000: String,
    kind: String,
    fields: String,
    expected_fail: String,
}

/// Strips surrounding backticks and whitespace from a markdown table cell.
fn clean_md_cell(cell: &str) -> String {
    let trimmed = cell.trim();
    trimmed
        .strip_prefix('`')
        .and_then(|s| s.strip_suffix('`'))
        .unwrap_or(trimmed)
        .trim()
        .to_string()
}

/// Parses the divergence table bounded by [`DOCS_TABLE_BEGIN`] and [`DOCS_TABLE_END`]
/// in `docs_text`.
fn parse_docs_divergence_table(docs_text: &str) -> Result<Vec<DocDivergenceRow>, String> {
    let (_, after_begin) = docs_text
        .split_once(DOCS_TABLE_BEGIN)
        .ok_or_else(|| format!("missing `{DOCS_TABLE_BEGIN}` marker in docs/unpacker.md"))?;
    let (table_block, _) = after_begin
        .split_once(DOCS_TABLE_END)
        .ok_or_else(|| format!("missing `{DOCS_TABLE_END}` marker in docs/unpacker.md"))?;

    let mut rows = Vec::new();
    for (line_idx, raw_line) in table_block.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if !line.starts_with('|') || !line.ends_with('|') {
            return Err(format!(
                "non-table line inside divergence table block at line {}: {line}",
                line_idx + 1
            ));
        }
        let cells: Vec<&str> = line[1..line.len() - 1].split('|').collect();
        if cells.len() != 6 {
            return Err(format!(
                "expected 6 columns in divergence table row, got {}: {line}",
                cells.len()
            ));
        }
        // Skip header and separator rows.
        let c0 = clean_md_cell(cells[0]);
        if c0 == "File" || c0.starts_with(":-") || c0.starts_with("--") {
            continue;
        }
        rows.push(DocDivergenceRow {
            file: c0,
            unp64: clean_md_cell(cells[1]),
            r2000: clean_md_cell(cells[2]),
            kind: clean_md_cell(cells[3]),
            fields: clean_md_cell(cells[4]),
            expected_fail: clean_md_cell(cells[5]),
        });
    }
    Ok(rows)
}

/// Builds the expected divergence table rows from `corpus` and `baseline`.
fn expected_docs_divergence_rows(corpus: &Corpus, baseline: &Baseline) -> Vec<DocDivergenceRow> {
    let mut rows = Vec::new();
    for case in &corpus.cases {
        if case.accepted_divergence.is_none() && case.expected_fail.is_none() {
            continue;
        }
        let unp64 = match case.unp64.as_ref().expect("validated corpus has unp64") {
            corpus::Unp64Record::Saved(saved) => {
                format!("{}-{} → {}", saved.start, saved.end, saved.entry)
            }
            corpus::Unp64Record::Failed(failed) => format!("failed: {}", failed.reason),
        };
        let baseline_row = baseline
            .rows
            .iter()
            .find(|r| r.file == case.file)
            .unwrap_or_else(|| panic!("missing baseline row for {}", case.file));
        let r2000 = match &baseline_row.outcome {
            BaselineOutcome::Unpacked(unpacked) => {
                format!("{}-{} → {}", unpacked.start, unpacked.end, unpacked.entry)
            }
            BaselineOutcome::Failed { error } => format!("error: {error}"),
        };
        let (kind, fields) = match &case.accepted_divergence {
            Some(div) => (div.kind.as_str().to_string(), div.fields.to_string()),
            None => ("-".to_string(), "-".to_string()),
        };
        let expected_fail = case
            .expected_fail
            .map_or_else(|| "-".to_string(), |ef| ef.as_str().to_string());
        rows.push(DocDivergenceRow {
            file: case.file.clone(),
            unp64,
            r2000,
            kind,
            fields,
            expected_fail,
        });
    }
    rows
}

/// Verifies that the divergence table in `docs_text` matches `corpus` and `baseline`
/// with zero missing, stale, or mismatched rows.
fn check_docs_divergence_table(
    corpus: &Corpus,
    baseline: &Baseline,
    docs_text: &str,
) -> Result<(), String> {
    let actual_rows = parse_docs_divergence_table(docs_text)?;
    let expected_rows = expected_docs_divergence_rows(corpus, baseline);

    let expected_files: BTreeSet<&str> = expected_rows.iter().map(|r| r.file.as_str()).collect();
    let actual_files: BTreeSet<&str> = actual_rows.iter().map(|r| r.file.as_str()).collect();

    let missing: Vec<&&str> = expected_files.difference(&actual_files).collect();
    let stale: Vec<&&str> = actual_files.difference(&expected_files).collect();
    if !missing.is_empty() || !stale.is_empty() {
        return Err(format!(
            "docs/unpacker.md divergence table out of sync with unpack_corpus.toml:\n\
             missing cases: {missing:?}\n\
             stale/unexpected cases: {stale:?}"
        ));
    }

    if actual_rows.len() != expected_rows.len() {
        return Err(format!(
            "duplicate row(s) in docs/unpacker.md divergence table: expected {}, got {}",
            expected_rows.len(),
            actual_rows.len()
        ));
    }

    for (idx, (actual, expected)) in actual_rows.iter().zip(expected_rows.iter()).enumerate() {
        if actual != expected {
            return Err(format!(
                "row {} (`{}`) mismatch in docs/unpacker.md:\n  actual:   {actual:?}\n  expected: {expected:?}",
                idx + 1,
                expected.file
            ));
        }
    }
    Ok(())
}

/// Verifies that the divergence table embedded in `docs/unpacker.md` between
/// [`DOCS_TABLE_BEGIN`] and [`DOCS_TABLE_END`] stays 100% synchronized with
/// `tests/6502/unpack_corpus.toml` and `tests/6502/unpack_baseline.toml`.
#[test]
fn docs_divergence_table_in_sync_with_manifest() {
    let corpus = load_valid_corpus();
    let baseline_path = Path::new(FIXTURE_DIR).join(BASELINE_FILE);
    let baseline = Baseline::load(&baseline_path).unwrap_or_else(|e| panic!("{e}"));
    let docs_path = Path::new(DOCS_UNPACKER_PATH);
    let docs_text =
        fs::read_to_string(docs_path).unwrap_or_else(|e| panic!("{}: {e}", docs_path.display()));

    if let Err(err) = check_docs_divergence_table(&corpus, &baseline, &docs_text) {
        panic!("{err}");
    }

    // Verify negative detection: a missing row, a stale removed row, or a wrong kind fails.
    let missing_one = docs_text.replacen(
        "| `c64_traveller.tiny_crunch.prg` | `$0801-$FFFD → $0911` | `$0801-$7949 → $0911` | `unp64_default_end` | `end` | `-` |\n",
        "",
        1,
    );
    assert!(
        check_docs_divergence_table(&corpus, &baseline, &missing_one).is_err(),
        "sync check must reject a table missing a manifest divergence"
    );

    let with_stale = docs_text.replacen(
        DOCS_TABLE_END,
        "| `c64_mule.dali.prg` | `$0800-$9D19 → $1100` | `$0800-$9D19 → $1100` | `padding_byte` | `start` | `-` |\n<!-- END CORPUS DIVERGENCE TABLE -->",
        1,
    );
    assert!(
        check_docs_divergence_table(&corpus, &baseline, &with_stale).is_err(),
        "sync check must reject a stale case that has no divergence in unpack_corpus.toml"
    );

    let wrong_kind = docs_text.replacen(
        "| `c64_traveller.tiny_crunch.prg` | `$0801-$FFFD → $0911` | `$0801-$7949 → $0911` | `unp64_default_end` | `end` | `-` |",
        "| `c64_traveller.tiny_crunch.prg` | `$0801-$FFFD → $0911` | `$0801-$7949 → $0911` | `unexplained` | `end` | `-` |",
        1,
    );
    assert!(
        check_docs_divergence_table(&corpus, &baseline, &wrong_kind).is_err(),
        "sync check must reject a mismatched divergence kind"
    );
}
