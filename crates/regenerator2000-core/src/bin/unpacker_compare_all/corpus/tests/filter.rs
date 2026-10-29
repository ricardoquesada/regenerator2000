//! Tests for `filter` (T1.A1.54–T1.A1.56).

use super::*;

/// Two groups (`mule` with two members, `moving_tubes` with one) and one
/// plain case.
fn filter_corpus() -> Corpus {
    let mut dali = simple_case("c64_mule.dali.prg");
    dali.group = Some("mule".to_string());
    let mut exo = simple_case("c64_mule.exo3.prg");
    exo.group = Some("mule".to_string());
    let mut tubes = simple_case("c64_moving_tubes_lxt.dali.prg");
    tubes.group = Some("moving_tubes".to_string());
    Corpus {
        groups: vec![
            mule_group(),
            Group {
                id: "moving_tubes".to_string(),
                original: "tubes_orig.prg".to_string(),
                entry: HexAddr(0x2E00),
            },
        ],
        cases: vec![
            simple_case("c64_traveller.tiny_crunch.prg"),
            exo,
            tubes,
            dali,
        ],
    }
}

/// The file names `filter` selects from `corpus`.
fn selected_files(filter: &CorpusFilter, corpus: &Corpus) -> Vec<String> {
    filter
        .select(corpus)
        .unwrap()
        .iter()
        .map(|case| case.file.clone())
        .collect()
}

/// T1.A1.54
#[test]
fn corpus_filter_none_selects_all() {
    let corpus = filter_corpus();
    for value in [None, Some(""), Some("   ")] {
        let filter = CorpusFilter::from_env_value(value);
        assert_eq!(filter, CorpusFilter::All);
        assert!(!filter.is_active());
        assert_eq!(
            selected_files(&filter, &corpus),
            vec![
                "c64_moving_tubes_lxt.dali.prg",
                "c64_mule.dali.prg",
                "c64_mule.exo3.prg",
                "c64_traveller.tiny_crunch.prg",
            ]
        );
    }
}

/// T1.A1.55
#[test]
fn corpus_filter_substring_selects_cases_and_whole_groups() {
    let corpus = filter_corpus();
    let traveller = CorpusFilter::from_env_value(Some(" traveller "));
    assert_eq!(traveller, CorpusFilter::Substring("traveller".to_string()));
    assert!(traveller.is_active());
    assert_eq!(
        selected_files(&traveller, &corpus),
        vec!["c64_traveller.tiny_crunch.prg"]
    );

    // Matching the group id selects every member.
    let tubes = CorpusFilter::from_env_value(Some("moving_tubes"));
    assert_eq!(
        selected_files(&tubes, &corpus),
        vec!["c64_moving_tubes_lxt.dali.prg"]
    );

    // Matching one member's file pulls in its whole group.
    let exo = CorpusFilter::from_env_value(Some("exo3"));
    assert_eq!(
        selected_files(&exo, &corpus),
        vec!["c64_mule.dali.prg", "c64_mule.exo3.prg"]
    );
}

/// T1.A1.56
#[test]
fn corpus_filter_matching_nothing_is_failure() {
    let filter = CorpusFilter::from_env_value(Some("zzz"));
    assert_eq!(
        filter.select(&filter_corpus()),
        Err(CorpusError::FilterMatchedNothing {
            filter: "zzz".to_string()
        })
    );
}
