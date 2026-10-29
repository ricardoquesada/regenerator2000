//! Unified declarative packer registry and detection entry points.
//!
//! [`REGISTRY`] is the single source of truth for packer detection order, mirroring
//! the scanner sequence in `unp64` (`_Scanners.c`).

use super::spec::{LoadedImage, PackerSpec, SignatureMatch};
use super::{
    action_replay, alz64, antiram, byteboozer, card_cruncher, ccs, cruel_cruncher, dali, eagle,
    eca, exomizer, final_cartridge, mc_cracken, pucrunch, super_cruncher, tbc, time_cruncher,
    tiny_crunch, triad, tscrunch, turbo_cruncher,
};

/// Unified ordered registry of all 21 [`PackerSpec`] entries in ascending `priority` order.
pub static REGISTRY: &[&PackerSpec] = &[
    &eca::SPEC,
    &action_replay::SPEC,
    &cruel_cruncher::SPEC,
    &pucrunch::SPEC,
    &time_cruncher::SPEC,
    &card_cruncher::SPEC,
    &mc_cracken::SPEC,
    &tbc::SPEC,
    &byteboozer::SPEC,
    &alz64::SPEC,
    &tscrunch::SPEC,
    &dali::SPEC,
    &tiny_crunch::SPEC,
    &super_cruncher::SPEC,
    &final_cartridge::SPEC,
    &exomizer::SPEC,
    &ccs::SPEC,
    &triad::SPEC,
    &eagle::SPEC,
    &antiram::SPEC,
    &turbo_cruncher::SPEC,
];

/// Alias for [`REGISTRY`] exposing all registered [`PackerSpec`] entries in priority order.
pub static PACKERS: &[&PackerSpec] = REGISTRY;

/// Returns all declarative [`PackerSpec`] entries in [`REGISTRY`].
#[must_use]
pub fn registry_specs() -> Vec<&'static PackerSpec> {
    REGISTRY.to_vec()
}

/// Returns `true` if `packer_name` is produced by `spec`.
#[must_use]
pub fn matches_packer_name(spec: &PackerSpec, packer_name: &str) -> bool {
    spec.name == packer_name
        || (spec.id == "card_cruncher" && packer_name == "1001 CardCruncher v4")
        || (spec.id == "tscrunch" && packer_name == "TSCrunch v1.3+-X2")
        || (spec.id == "exomizer" && matches!(packer_name, "Exomizer v3.02+" | "Exomizer 2.x"))
        || (spec.id == "alz64" && packer_name == "ALZ64/Kabuto")
        || (spec.id == "antiram" && packer_name == "Antiram Packer v1.0")
}

/// Looks up the [`PackerSpec`] in [`REGISTRY`] that produces the display name `packer_name`.
#[must_use]
pub fn find_entry_by_name(packer_name: &str) -> Option<&'static PackerSpec> {
    REGISTRY
        .iter()
        .copied()
        .find(|spec| matches_packer_name(spec, packer_name))
}

/// Returns the first matching `(&'static PackerSpec, SignatureMatch)` in `specs` for `image`.
#[must_use]
pub fn detect_spec_in(
    specs: &[&'static PackerSpec],
    image: &LoadedImage<'_>,
) -> Option<(&'static PackerSpec, SignatureMatch)> {
    specs
        .iter()
        .find_map(|&spec| spec.detect(image).map(|sig_match| (spec, sig_match)))
}

/// Returns the first matching `(&'static PackerSpec, SignatureMatch)` in [`REGISTRY`] for `image`.
#[must_use]
pub fn detect_spec(image: &LoadedImage<'_>) -> Option<(&'static PackerSpec, SignatureMatch)> {
    detect_spec_in(REGISTRY, image)
}

/// Returns the first matching packer in [`REGISTRY`] for `image` (production path).
#[must_use]
pub fn detect(image: &LoadedImage<'_>) -> Option<SignatureMatch> {
    detect_spec(image).map(|(_spec, sig_match)| sig_match)
}

/// Returns every matching packer in [`REGISTRY`] for `image`, in ascending priority order.
#[must_use]
pub fn detect_all(image: &LoadedImage<'_>) -> Vec<SignatureMatch> {
    detect_all_in_specs(REGISTRY, image)
}

/// Returns the first matching [`PackerSpec`] in `specs` for `image`.
#[must_use]
pub fn detect_in_specs(
    specs: &[&'static PackerSpec],
    image: &LoadedImage<'_>,
) -> Option<SignatureMatch> {
    specs.iter().find_map(|spec| spec.detect(image))
}

/// Returns every matching [`PackerSpec`] in `specs` for `image`, in slice order.
#[must_use]
pub fn detect_all_in_specs(
    specs: &[&'static PackerSpec],
    image: &LoadedImage<'_>,
) -> Vec<SignatureMatch> {
    specs.iter().filter_map(|spec| spec.detect(image)).collect()
}

// ===========================================================================
// Registry Invariant Tests (`T3.0.30`, `T3.0.38`–`T3.0.45`) & M3.2 (`T3.1.1`–`T3.1.9`)
// ===========================================================================

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::Path;

    use super::*;
    use crate::packers::PackerInfo;
    use crate::packers::spec::{
        AddrSource, Anchor, Bounds, Hints, MemoryPatch, PackerHooks, Pat, PatchAddr, PointerGuard,
        PointerMonitor, RefineContext, Registers, ResolvedSpec, Signature, StepContext,
        is_valid_packer_id, sig,
    };
    use crate::state::types::TargetSystem;
    use crate::unpacker::machine::Phase;
    use crate::unpacker::{UnpackConfig, UnpackerMemory, unpack};

    fn detect_c64_info(mem: &[u8], load_addr: u16, load_end: u16) -> Option<PackerInfo> {
        let view = LoadedImage::for_c64(mem, load_addr, load_end);
        let sig_match = detect(&view)?;
        Some(PackerInfo::resolve(&view, &sig_match))
    }

    // -----------------------------------------------------------------------
    // Sample spec set (`SAMPLE_SPECS`, Ruling R15)
    // -----------------------------------------------------------------------

    const SAMPLE_ALPHA_SIGS: [Signature; 1] = [Signature {
        anchor: Anchor::BasicStart { offset: 0x0C },
        pattern: sig![0x78, 0xA9, [0x34, 0x37], 0x85, 0x01, 0xA2, 0x00, 0x8E],
    }];
    static SAMPLE_ALPHA_SPEC: PackerSpec = PackerSpec {
        id: "sample.alpha",
        name: "Sample Alpha",
        priority: 10,
        systems: &[TargetSystem::C64],
        load_addr_max: Some(0x0801),
        signatures: &SAMPLE_ALPHA_SIGS,
        hints: Hints {
            dep_addr: Some(AddrSource::Const(0x0100)),
            start_addr: Some(AddrSource::BasicStart(0)),
            ..Hints::NONE
        },
        patches: &[],
        resolve: None,
        hooks: None,
        // 12 padding bytes ($0801..$080C) + 8 signature bytes at $080D.
        fixture: &[
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x78, 0xA9,
            0x37, 0x85, 0x01, 0xA2, 0x00, 0x8E,
        ],
        fixture_load_addr: 0x0801,
    };

    const SAMPLE_BETA_SIGS: [Signature; 1] = [Signature {
        anchor: Anchor::Window(0x0810..=0x0840),
        pattern: sig![0xA9, __, 0x85, __, 0xA0, 0x00, 0xC6, 0xAF],
    }];
    static SAMPLE_BETA_SPEC: PackerSpec = PackerSpec {
        id: "sample.beta",
        name: "Sample Beta",
        priority: 20,
        systems: &[TargetSystem::C64],
        load_addr_max: None,
        signatures: &SAMPLE_BETA_SIGS,
        hints: Hints {
            dep_addr: Some(AddrSource::Const(0x0100)),
            start_addr: Some(AddrSource::BasicStart(-1)),
            end_addr: Some(AddrSource::PointerAfterRun {
                ptr_addr: 0x00AE,
                adjust: -1,
                guard: PointerGuard::McCrackenEndOfProgram,
            }),
            entry_point: Some(AddrSource::Const(0x1100)),
            ..Hints::NONE
        },
        patches: &[],
        resolve: None,
        hooks: None,
        fixture: &[0xA9, 0x01, 0x85, 0x02, 0xA0, 0x00, 0xC6, 0xAF],
        fixture_load_addr: 0x0820,
    };

    fn sample_gamma_resolve(
        image: &LoadedImage<'_>,
        sig_match: &SignatureMatch,
    ) -> Option<ResolvedSpec> {
        let ep =
            AddrSource::OperandLe { from_match: 4 }.resolve_static(image, sig_match.anchor_addr)?;
        Some(ResolvedSpec::from_hints(Hints {
            dep_addr: Some(AddrSource::Const(0x010E)),
            start_addr: Some(AddrSource::BasicStart(0)),
            entry_point: Some(AddrSource::Const(ep)),
            ..Hints::NONE
        }))
    }

    const SAMPLE_GAMMA_SIGS: [Signature; 1] = [Signature {
        anchor: Anchor::Absolute(0x080D),
        pattern: sig![(0xFD, 0x84), 0x01, 0xCA, 0x9A],
    }];
    static SAMPLE_GAMMA_SPEC: PackerSpec = PackerSpec {
        id: "sample.gamma",
        name: "Sample Gamma",
        priority: 30,
        systems: &[TargetSystem::C64],
        load_addr_max: Some(0x0801),
        signatures: &SAMPLE_GAMMA_SIGS,
        hints: Hints::NONE,
        patches: &[],
        resolve: Some(sample_gamma_resolve),
        hooks: None,
        fixture: &[
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x84, 0x01,
            0xCA, 0x9A, 0x11, 0x09,
        ],
        fixture_load_addr: 0x0801,
    };

    const SAMPLE_SPECS: &[&PackerSpec] =
        &[&SAMPLE_ALPHA_SPEC, &SAMPLE_BETA_SPEC, &SAMPLE_GAMMA_SPEC];

    /// Non-C64 systems tested by `T3.0.30`.
    fn non_c64_systems() -> [TargetSystem; 10] {
        [
            TargetSystem::C128,
            TargetSystem::Vic20,
            TargetSystem::Plus4,
            TargetSystem::C16,
            TargetSystem::Pet20,
            TargetSystem::Pet40,
            TargetSystem::C1541,
            TargetSystem::C1571,
            TargetSystem::C1581,
            TargetSystem::Custom("Commodore 64".into()),
        ]
    }

    fn check_c64_specs_do_not_match_other_systems(specs: &[&'static PackerSpec]) {
        for &spec in specs {
            if !spec.systems.iter().any(TargetSystem::is_c64) {
                continue;
            }
            let (mem, load_addr, load_end) = spec.build_fixture_memory();
            let c64_view = LoadedImage::new(&mem, load_addr, load_end, &TargetSystem::C64);
            assert!(
                spec.detect(&c64_view).is_some(),
                "spec `{}` must match its fixture on C64",
                spec.id
            );

            for sys in non_c64_systems() {
                if spec
                    .systems
                    .iter()
                    .any(|allowed| super::super::spec::system_matches(allowed, &sys))
                {
                    continue;
                }
                let view = LoadedImage::new(&mem, load_addr, load_end, &sys);
                assert!(
                    spec.detect(&view).is_none(),
                    "C64 spec `{}` must not match on {sys}",
                    spec.id
                );
            }
        }
    }

    /// T3.0.30: C64 specs and registry entries never match on non-C64 systems.
    #[test]
    fn c64_spec_does_not_match_on_other_systems() {
        assert!(!REGISTRY.is_empty());
        assert!(!SAMPLE_SPECS.is_empty());
        check_c64_specs_do_not_match_other_systems(REGISTRY);
        check_c64_specs_do_not_match_other_systems(SAMPLE_SPECS);

        // Also verify via `registry::detect` using a known C64 packer image (Time Cruncher at $0810).
        let mut mem = vec![0u8; 0x1_0000];
        mem[0x0810..0x0818].copy_from_slice(&[0x78, 0xA9, 0x34, 0x85, 0x01, 0xA0, 0xC4, 0xB9]);
        let c64_view = LoadedImage::for_c64(&mem, 0x0801, 0x0900);
        assert!(detect(&c64_view).is_some());
        for sys in non_c64_systems() {
            let other_view = LoadedImage::new(&mem, 0x0801, 0x0900, &sys);
            assert!(
                detect(&other_view).is_none(),
                "registry::detect must not match C64 Time Cruncher on {sys}"
            );
        }
    }

    fn check_ids_unique_and_priorities_strictly_increase_specs(specs: &[&'static PackerSpec]) {
        let mut seen = BTreeSet::new();
        for &spec in specs {
            assert!(
                seen.insert(spec.id),
                "duplicate spec id `{}` in slice",
                spec.id
            );
        }
        for pair in specs.windows(2) {
            assert!(
                pair[0].priority < pair[1].priority,
                "priority of `{}` ({}) must be strictly less than `{}` ({})",
                pair[0].id,
                pair[0].priority,
                pair[1].id,
                pair[1].priority
            );
        }
    }

    /// T3.0.38: Every spec id is unique and priorities strictly increase across the registry.
    #[test]
    fn ids_are_unique_and_priorities_strictly_increase() {
        assert_eq!(REGISTRY.len(), 21);
        check_ids_unique_and_priorities_strictly_increase_specs(REGISTRY);
        check_ids_unique_and_priorities_strictly_increase_specs(SAMPLE_SPECS);
    }

    fn check_every_spec_matches_its_own_fixture(specs: &[&'static PackerSpec]) {
        for &spec in specs {
            let system = spec
                .systems
                .first()
                .expect("every spec declares at least one target system");
            let (mem, load_addr, load_end) = spec.build_fixture_memory();
            let view = LoadedImage::new(&mem, load_addr, load_end, system);
            let matched = spec.detect(&view).unwrap_or_else(|| {
                panic!(
                    "spec `{}` failed to match its own fixture at ${load_addr:04X}",
                    spec.id
                )
            });
            assert_eq!(matched.spec_id, spec.id);
        }
    }

    /// T3.0.39: Every spec matches its own synthetic `fixture` at `fixture_load_addr`.
    #[test]
    fn every_spec_matches_its_own_fixture() {
        assert!(!REGISTRY.is_empty());
        assert!(!SAMPLE_SPECS.is_empty());
        check_every_spec_matches_its_own_fixture(REGISTRY);
        check_every_spec_matches_its_own_fixture(SAMPLE_SPECS);
    }

    fn check_no_fixture_claimed_by_higher_priority_spec(specs: &[&'static PackerSpec]) {
        for &spec in specs {
            let system = spec
                .systems
                .first()
                .expect("every spec declares at least one target system");
            let (mem, load_addr, load_end) = spec.build_fixture_memory();
            let view = LoadedImage::new(&mem, load_addr, load_end, system);
            let matches = detect_all_in_specs(specs, &view);
            let first = matches.first().unwrap_or_else(|| {
                panic!(
                    "fixture for `{}` matched nothing in the spec slice",
                    spec.id
                )
            });
            assert_eq!(
                first.spec_id, spec.id,
                "fixture for `{}` (priority {}) was shadowed by higher-priority spec `{}`",
                spec.id, spec.priority, first.spec_id
            );
        }
    }

    /// T3.0.40: No spec's `fixture` is claimed by an earlier (higher-priority) spec in the registry.
    #[test]
    fn no_fixture_is_claimed_by_a_higher_priority_spec() {
        assert!(!REGISTRY.is_empty());
        assert!(!SAMPLE_SPECS.is_empty());
        check_no_fixture_claimed_by_higher_priority_spec(REGISTRY);
        check_no_fixture_claimed_by_higher_priority_spec(SAMPLE_SPECS);

        for &spec in REGISTRY {
            let system = spec.systems.first().unwrap();
            let (mem, load_addr, load_end) = spec.build_fixture_memory();
            let view = LoadedImage::new(&mem, load_addr, load_end, system);
            let first = detect(&view)
                .unwrap_or_else(|| panic!("fixture for `{}` matched nothing in REGISTRY", spec.id));
            assert_eq!(
                first.spec_id, spec.id,
                "fixture for `{}` was shadowed in REGISTRY by `{}`",
                spec.id, first.spec_id
            );
        }
    }

    fn check_spec_ids_are_stable_identifiers(specs: &[&'static PackerSpec]) {
        for &spec in specs {
            assert!(is_valid_packer_id(spec.id), "invalid spec id `{}`", spec.id);
        }
    }

    /// T3.0.41: Every registry id and spec id is a valid, canonical `PackerId`.
    #[test]
    fn spec_ids_are_stable_identifiers() {
        check_spec_ids_are_stable_identifiers(REGISTRY);
        check_spec_ids_are_stable_identifiers(SAMPLE_SPECS);

        for invalid in [
            "",
            "Eagle",
            "bad id",
            ".leading",
            "trailing.",
            "double..dot",
            "-dash",
        ] {
            assert!(
                !is_valid_packer_id(invalid),
                "expected `{invalid}` to be invalid"
            );
        }
    }

    fn check_every_spec_has_fixture_and_detection_logic(specs: &[&'static PackerSpec]) {
        for &spec in specs {
            assert!(
                !spec.name.trim().is_empty(),
                "spec `{}` has an empty display name",
                spec.id
            );
            assert!(
                !spec.systems.is_empty(),
                "spec `{}` declares no target systems",
                spec.id
            );
            assert!(
                !spec.fixture.is_empty(),
                "spec `{}` has an empty fixture",
                spec.id
            );
            assert!(
                !spec.signatures.is_empty() || spec.resolve.is_some(),
                "spec `{}` has neither signatures nor a resolve function",
                spec.id
            );
            for (idx, sig) in spec.signatures.iter().enumerate() {
                assert!(
                    !sig.pattern.is_empty(),
                    "spec `{}` signature #{idx} has an empty pattern",
                    spec.id
                );
            }
        }
    }

    /// T3.0.42: Every spec has a non-empty `fixture`, target `systems`, and detection logic.
    #[test]
    fn every_spec_has_fixture_and_detection_logic() {
        assert!(!REGISTRY.is_empty());
        assert!(!SAMPLE_SPECS.is_empty());
        check_every_spec_has_fixture_and_detection_logic(REGISTRY);
        check_every_spec_has_fixture_and_detection_logic(SAMPLE_SPECS);
    }

    /// T3.0.43: Detection over empty and uniform (`$00`, `$FF`, `$AA`) images returns `None`.
    #[test]
    fn detect_on_empty_and_uniform_images_is_none() {
        let empty: [u8; 0] = [];
        let empty_view = LoadedImage::for_c64(&empty, 0x0801, 0x0801);
        assert!(detect(&empty_view).is_none());
        assert!(detect_all(&empty_view).is_empty());
        assert!(detect_in_specs(SAMPLE_SPECS, &empty_view).is_none());

        for fill in [0x00_u8, 0xFF, 0xAA, 0x55] {
            let mem = vec![fill; 0x1_0000];
            for load_addr in [0x0000_u16, 0x0800, 0x0801, 0x1000, 0xFFFF] {
                let view = LoadedImage::for_c64(&mem, load_addr, 0xF000);
                assert!(
                    detect(&view).is_none(),
                    "fill={fill:#04X}, load_addr={load_addr:#06X}"
                );
                assert!(detect_all(&view).is_empty());
                assert!(detect_in_specs(SAMPLE_SPECS, &view).is_none());
                assert!(detect_all_in_specs(SAMPLE_SPECS, &view).is_empty());
            }
        }
    }

    /// T3.0.44: `detect(view)` always equals `detect_all(view).into_iter().next()`.
    #[test]
    fn detect_equals_first_of_detect_all() {
        // 1. Check over `SAMPLE_SPECS`, including an image where two sample specs match at once.
        for &spec in SAMPLE_SPECS {
            let (mem, load_addr, load_end) = spec.build_fixture_memory();
            let view = LoadedImage::for_c64(&mem, load_addr, load_end);
            let first = detect_in_specs(SAMPLE_SPECS, &view);
            let all = detect_all_in_specs(SAMPLE_SPECS, &view);
            assert!(first.is_some());
            assert_eq!(first, all.into_iter().next());
        }

        // Combined image matching both `SAMPLE_ALPHA_SPEC` (priority 10) and `SAMPLE_BETA_SPEC` (priority 20).
        let (mut combined, load_addr, _) = SAMPLE_ALPHA_SPEC.build_fixture_memory();
        let beta_start = usize::from(SAMPLE_BETA_SPEC.fixture_load_addr);
        combined[beta_start..beta_start + SAMPLE_BETA_SPEC.fixture.len()]
            .copy_from_slice(SAMPLE_BETA_SPEC.fixture);
        let view = LoadedImage::for_c64(&combined, load_addr, 0x0900);
        let all_sample = detect_all_in_specs(SAMPLE_SPECS, &view);
        assert_eq!(all_sample.len(), 2);
        assert_eq!(
            detect_in_specs(SAMPLE_SPECS, &view),
            all_sample.into_iter().next()
        );

        // 2. Check over production `REGISTRY` with both a real match (McCracken + Eagle Cruncher)
        // and each static spec fixture.
        for &spec in REGISTRY {
            let (mem, load_addr, load_end) = spec.build_fixture_memory();
            let view = LoadedImage::for_c64(&mem, load_addr, load_end);
            let first = detect(&view);
            let all = detect_all(&view);
            assert!(first.is_some());
            assert_eq!(first, all.into_iter().next());
        }

        let mut prod_mem = vec![0u8; 0x1_0000];
        prod_mem[0x080D..0x081B].copy_from_slice(&[
            0x78, 0xA2, 0x00, 0xBD, 0x00, 0x00, 0x9D, 0x00, 0x01, 0xD0, 0xFC, 0x4C, 0x00, 0x01,
        ]);
        prod_mem[0x0830..0x0838].copy_from_slice(&[0xA9, 0x01, 0x85, 0x02, 0xA0, 0x00, 0xC6, 0xAF]);
        let prod_view = LoadedImage::for_c64(&prod_mem, 0x0801, 0x0900);
        let all_prod = detect_all(&prod_view);
        assert_eq!(all_prod.len(), 2);
        assert_eq!(all_prod[0].spec_id, "mc_cracken");
        assert_eq!(all_prod[1].spec_id, "eagle");
        assert_eq!(detect(&prod_view), all_prod.into_iter().next());
    }

    /// T3.0.45: `REGISTRY` preserves the exact 21-packer order of the original scanner chain.
    #[test]
    fn registry_order_preserves_legacy_order() {
        const EXPECTED_FAMILIES: [&str; 21] = [
            "eca",
            "action_replay",
            "cruel_cruncher",
            "pucrunch",
            "time_cruncher",
            "card_cruncher",
            "mc_cracken",
            "tbc",
            "byteboozer",
            "alz64",
            "tscrunch",
            "dali",
            "tiny_crunch",
            "super_cruncher",
            "final_cartridge",
            "exomizer",
            "ccs",
            "triad",
            "eagle",
            "antiram",
            "turbo_cruncher",
        ];

        // Collapse consecutive multi-variant family prefixes (`foo.v1`, `foo.v2` -> `foo`)
        // so the order check works both before and after multi-variant specs are split.
        let mut actual_families: Vec<&str> = Vec::new();
        for &spec in REGISTRY {
            let family = spec.id.split('.').next().unwrap_or(spec.id);
            if actual_families.last().copied() != Some(family) {
                actual_families.push(family);
            }
        }
        assert_eq!(actual_families, EXPECTED_FAMILIES);
    }

    // =======================================================================
    // M3.2 — The 10 static packers (`T3.1.1`–`T3.1.9`)
    // =======================================================================

    const STATIC_PACKER_SPECS: [&PackerSpec; 10] = [
        &action_replay::SPEC,
        &cruel_cruncher::SPEC,
        &time_cruncher::SPEC,
        &tiny_crunch::SPEC,
        &super_cruncher::SPEC,
        &final_cartridge::SPEC,
        &ccs::SPEC,
        &triad::SPEC,
        &eagle::SPEC,
        &turbo_cruncher::SPEC,
    ];

    /// T3.1.1: Table-driven over all 10 static packers: each spec's `start_addr` uses
    /// `AddrSource::BasicStart(0)` (replacing hardcoded `0x0801`), and its resolved
    /// `PackerInfo` equals the pre-migration literal `PackerInfo`.
    #[test]
    fn static_spec_hints_equal_legacy_packer_info() {
        let cases: [(&PackerSpec, PackerInfo); 10] = [
            (
                &action_replay::SPEC,
                PackerInfo {
                    name: "Action Replay",
                    dep_addr: Some(0x0100),
                    start_addr: Some(0x0801),
                    end_addr: None,
                    entry_point: None,
                    end_addr_ptr: None,
                },
            ),
            (
                &cruel_cruncher::SPEC,
                PackerInfo {
                    name: "Cruel Cruncher",
                    dep_addr: Some(0x0100),
                    start_addr: Some(0x0801),
                    end_addr: None,
                    entry_point: None,
                    end_addr_ptr: None,
                },
            ),
            (
                &time_cruncher::SPEC,
                PackerInfo {
                    name: "Time Cruncher",
                    dep_addr: Some(0x0100),
                    start_addr: Some(0x0801),
                    end_addr: None,
                    entry_point: None,
                    end_addr_ptr: None,
                },
            ),
            (
                &tiny_crunch::SPEC,
                PackerInfo {
                    name: "TinyCrunch",
                    dep_addr: Some(0x010E),
                    start_addr: Some(0x0801),
                    end_addr: None,
                    entry_point: Some(0x0911),
                    end_addr_ptr: None,
                },
            ),
            (
                &super_cruncher::SPEC,
                PackerInfo {
                    name: "Super Cruncher",
                    dep_addr: Some(0x0100),
                    start_addr: Some(0x0801),
                    end_addr: None,
                    entry_point: None,
                    end_addr_ptr: None,
                },
            ),
            (
                &final_cartridge::SPEC,
                PackerInfo {
                    name: "Final Cartridge III",
                    dep_addr: Some(0x0100),
                    start_addr: Some(0x0801),
                    end_addr: None,
                    entry_point: None,
                    end_addr_ptr: None,
                },
            ),
            (
                &ccs::SPEC,
                PackerInfo {
                    name: "Commodore Cruncher System",
                    dep_addr: Some(0x0100),
                    start_addr: Some(0x0801),
                    end_addr: None,
                    entry_point: None,
                    end_addr_ptr: None,
                },
            ),
            (
                &triad::SPEC,
                PackerInfo {
                    name: "Triad Cruncher",
                    dep_addr: Some(0x0100),
                    start_addr: Some(0x0801),
                    end_addr: None,
                    entry_point: None,
                    end_addr_ptr: None,
                },
            ),
            (
                &eagle::SPEC,
                PackerInfo {
                    name: "Eagle Cruncher",
                    dep_addr: Some(0x0100),
                    start_addr: Some(0x0801),
                    end_addr: None,
                    entry_point: None,
                    end_addr_ptr: None,
                },
            ),
            (
                &turbo_cruncher::SPEC,
                PackerInfo {
                    name: "Turbo Cruncher",
                    dep_addr: Some(0x0100),
                    start_addr: Some(0x0801),
                    end_addr: None,
                    entry_point: None,
                    end_addr_ptr: None,
                },
            ),
        ];

        let mut failures = Vec::new();
        for (spec, expected_info) in cases {
            if spec.hints.start_addr != Some(AddrSource::BasicStart(0)) {
                failures.push(format!(
                    "{}: expected start_addr == Some(AddrSource::BasicStart(0)), got {:?}",
                    spec.id, spec.hints.start_addr
                ));
            }
            let (mem, load_addr, load_end) = spec.build_fixture_memory();
            let view = LoadedImage::for_c64(&mem, load_addr, load_end);
            let Some(sig_match) = spec.detect(&view) else {
                failures.push(format!("{}: spec failed to match its fixture", spec.id));
                continue;
            };
            if sig_match.resolved.hints.start_addr != Some(AddrSource::BasicStart(0)) {
                failures.push(format!(
                    "{}: resolved start_addr != Some(AddrSource::BasicStart(0))",
                    spec.id
                ));
            }
            let actual_info = PackerInfo::resolve(&view, &sig_match);
            if actual_info != expected_info {
                failures.push(format!(
                    "{}: resolved info {:?} != expected {:?}",
                    spec.id, actual_info, expected_info
                ));
            }
            let reg_info = detect_c64_info(&mem, load_addr, load_end);
            if reg_info.as_ref() != Some(&expected_info) {
                failures.push(format!(
                    "{}: registry info {:?} != expected {:?}",
                    spec.id, reg_info, expected_info
                ));
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    /// T3.1.2: Table-driven verification that each static packer preserves its exact
    /// legacy `load_addr` gate (`<= 0x0801` for 9 packers; no gate for `time_cruncher`).
    #[test]
    fn static_spec_load_gate_preserved() {
        let mut failures = Vec::new();

        for spec in STATIC_PACKER_SPECS {
            let expected_max = if spec.id == "time_cruncher" {
                None
            } else {
                Some(0x0801)
            };
            if spec.load_addr_max != expected_max {
                failures.push(format!(
                    "{}: expected load_addr_max {:?}, got {:?}",
                    spec.id, expected_max, spec.load_addr_max
                ));
            }

            let (mem, fixture_load, load_end) = spec.build_fixture_memory();

            if spec.id == "time_cruncher" {
                for load_addr in [0x0000_u16, 0x0800, 0x0801, 0x0802, 0x080D, 0x0810, 0x1000] {
                    let view = LoadedImage::for_c64(&mem, load_addr, 0x0900);
                    if spec.detect(&view).is_none() {
                        failures.push(format!(
                            "time_cruncher should match at load_addr=${load_addr:04X} (no load gate)"
                        ));
                    }
                }
            } else if spec.id == "ccs" {
                // CCS matches at fixture_load ($040A <= $0801), and rejects > $0801.
                let ok_view = LoadedImage::for_c64(&mem, fixture_load, load_end);
                if spec.detect(&ok_view).is_none() {
                    failures.push("ccs failed to match at fixture_load $040A".into());
                }
                for bad_load in [0x0802_u16, 0x080B, 0x080D, 0x0817, 0x1000] {
                    let bad_view = LoadedImage::for_c64(&mem, bad_load, 0x0900);
                    if spec.detect(&bad_view).is_some() {
                        failures.push(format!("ccs matched at load_addr=${bad_load:04X} > $0801"));
                    }
                }
            } else {
                // Standard $080D-anchored C64 static packer with `load_addr <= $0801`.
                for ok_load in [0x0000_u16, 0x0400, 0x0800, 0x0801] {
                    let view = LoadedImage::for_c64(&mem, ok_load, 0x0900);
                    if spec.detect(&view).is_none() {
                        failures.push(format!(
                            "{}: expected match at load_addr=${ok_load:04X}",
                            spec.id
                        ));
                    }
                }
                for bad_load in [0x0802_u16, 0x080B, 0x080D, 0x0810, 0x1000, 0xFFFF] {
                    let view = LoadedImage::for_c64(&mem, bad_load, 0x0900);
                    if spec.detect(&view).is_some() {
                        failures.push(format!(
                            "{}: expected None at load_addr=${bad_load:04X} > $0801",
                            spec.id
                        ));
                    }
                    if detect_c64_info(&mem, bad_load, 0x0900).is_some() {
                        failures.push(format!(
                            "{}: detect_c64_info expected None at load_addr=${bad_load:04X}",
                            spec.id
                        ));
                    }
                }
            }
        }

        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    /// T3.1.3: Table-driven single-byte corruption test across all 10 static packers:
    /// flipping any non-wildcard signature byte to a non-matching value causes the spec to reject.
    #[test]
    fn static_spec_rejects_each_single_byte_corruption() {
        let mut failures = Vec::new();

        for spec in STATIC_PACKER_SPECS {
            let (base_mem, load_addr, load_end) = spec.build_fixture_memory();
            let base_view = LoadedImage::for_c64(&base_mem, load_addr, load_end);
            let Some(matched) = spec.detect(&base_view) else {
                failures.push(format!("{}: failed to match baseline fixture", spec.id));
                continue;
            };

            if spec.id == "ccs" {
                // Corrupt each of the 4 bytes of the CCS fixture at `fixture_load_addr`.
                for offset in 0..spec.fixture.len() {
                    let mut corrupted = base_mem.clone();
                    let addr = usize::from(load_addr) + offset;
                    corrupted[addr] ^= 0xFF;
                    let view = LoadedImage::for_c64(&corrupted, load_addr, load_end);
                    if spec.detect(&view).is_some() {
                        failures.push(format!(
                            "ccs: still matched after corrupting byte at offset {offset} (${addr:04X})"
                        ));
                    }
                    if detect_c64_info(&corrupted, load_addr, load_end).is_some() {
                        failures.push(format!(
                            "ccs: detect_c64_info still matched after corrupting byte at offset {offset}"
                        ));
                    }
                }
                continue;
            }

            let sig = &spec.signatures[0];
            let anchor_addr = usize::from(matched.anchor_addr);
            for (offset, &pat) in sig.pattern.iter().enumerate() {
                if matches!(pat, Pat::Any) {
                    continue;
                }
                // Pick a byte value that does NOT satisfy `pat`.
                let Some(bad_byte) = (0x00_u8..=0xFF).find(|&b| !pat.matches(b)) else {
                    failures.push(format!(
                        "{}: non-Any pattern at offset {offset} matched all 256 bytes",
                        spec.id
                    ));
                    continue;
                };
                let mut corrupted = base_mem.clone();
                corrupted[anchor_addr + offset] = bad_byte;
                let view = LoadedImage::for_c64(&corrupted, load_addr, load_end);
                if spec.detect(&view).is_some() {
                    failures.push(format!(
                        "{}: still matched after corrupting offset +{offset:#04X} to {bad_byte:#04X}",
                        spec.id
                    ));
                }
            }
        }

        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    /// T3.1.4: Table-driven wildcard acceptance test across all static specs with `Pat::Any`
    /// wildcards (`super_cruncher`, `triad`, `eagle`, `turbo_cruncher`).
    #[test]
    fn static_spec_accepts_arbitrary_wildcards() {
        let expected_wildcard_offsets: [(&PackerSpec, &[usize]); 10] = [
            (&action_replay::SPEC, &[]),
            (&cruel_cruncher::SPEC, &[]),
            (&time_cruncher::SPEC, &[]),
            (&tiny_crunch::SPEC, &[]),
            (&super_cruncher::SPEC, &[6, 8, 9, 10, 11, 12, 15, 16]),
            (&final_cartridge::SPEC, &[]),
            (&ccs::SPEC, &[]),
            (&triad::SPEC, &[6, 8, 9, 11, 12]),
            (&eagle::SPEC, &[2, 4, 5]),
            (&turbo_cruncher::SPEC, &[6, 7]),
        ];

        let mut failures = Vec::new();

        for (spec, expected_offsets) in expected_wildcard_offsets {
            let actual_offsets: Vec<usize> = spec
                .signatures
                .first()
                .map(|sig| {
                    sig.pattern
                        .iter()
                        .enumerate()
                        .filter_map(|(i, pat)| matches!(pat, Pat::Any).then_some(i))
                        .collect()
                })
                .unwrap_or_default();

            if actual_offsets.as_slice() != expected_offsets {
                failures.push(format!(
                    "{}: expected wildcard offsets {:?}, got {:?}",
                    spec.id, expected_offsets, actual_offsets
                ));
                continue;
            }

            if expected_offsets.is_empty() {
                continue;
            }

            let (base_mem, load_addr, load_end) = spec.build_fixture_memory();
            let base_view = LoadedImage::for_c64(&base_mem, load_addr, load_end);
            let anchor_addr = usize::from(spec.detect(&base_view).unwrap().anchor_addr);

            // 1. Every individual wildcard offset accepts all 256 byte values (`0x00..=0xFF`).
            for &offset in expected_offsets {
                for byte in 0x00_u8..=0xFF {
                    let mut mem = base_mem.clone();
                    mem[anchor_addr + offset] = byte;
                    let view = LoadedImage::for_c64(&mem, load_addr, load_end);
                    if spec.detect(&view).is_none() {
                        failures.push(format!(
                            "{}: rejected wildcard byte {byte:#04X} at offset +{offset}",
                            spec.id
                        ));
                        break;
                    }
                }
            }

            // 2. Setting all wildcard offsets simultaneously to arbitrary fill values still matches.
            for fill in [0x00_u8, 0x55, 0xAA, 0xFF] {
                let mut mem = base_mem.clone();
                for &offset in expected_offsets {
                    mem[anchor_addr + offset] = fill;
                }
                let view = LoadedImage::for_c64(&mem, load_addr, load_end);
                if spec.detect(&view).is_none() {
                    failures.push(format!(
                        "{}: rejected simultaneous wildcard fill {fill:#04X}",
                        spec.id
                    ));
                }
            }
        }

        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    /// T3.1.5: `action_replay` accepts both `0x37` and `0x34` at `$080D + 2`, and
    /// `cruel_cruncher` accepts all 6 combinations of `[0xA2, 0xA0] × [0x1A, 0x1F, 0x20]`
    /// at `$080D + 5..=6`, while rejecting all other byte values.
    #[test]
    fn action_replay_and_cruel_accept_each_alternative() {
        let mut failures = Vec::new();

        // 1. Action Replay: byte at $080D + 2 ($080F) must be 0x37 or 0x34.
        let (ar_base, ar_load, ar_end) = action_replay::SPEC.build_fixture_memory();
        for b2 in 0x00_u8..=0xFF {
            let mut mem = ar_base.clone();
            mem[0x080D + 2] = b2;
            let view = LoadedImage::for_c64(&mem, ar_load, ar_end);
            let matched = action_replay::SPEC.detect(&view);
            let should_match = matches!(b2, 0x37 | 0x34);
            if matched.is_some() != should_match {
                failures.push(format!(
                    "action_replay at b2={b2:#04X}: expected match={should_match}, got {:?}",
                    matched.map(|m| m.spec_name)
                ));
            }
            if should_match {
                let info = detect_c64_info(&mem, ar_load, ar_end).unwrap();
                if info.name != "Action Replay"
                    || info.dep_addr != Some(0x0100)
                    || info.start_addr != Some(0x0801)
                {
                    failures.push(format!(
                        "action_replay at b2={b2:#04X}: unexpected info {info:?}"
                    ));
                }
            }
        }

        // 2. Cruel Cruncher: all 6 valid `(b5, b6)` pairs in `[0xA2, 0xA0] × [0x1A, 0x1F, 0x20]`.
        let (cc_base, cc_load, cc_end) = cruel_cruncher::SPEC.build_fixture_memory();
        for b5 in [0xA2_u8, 0xA0] {
            for b6 in [0x1A_u8, 0x1F, 0x20] {
                let mut mem = cc_base.clone();
                mem[0x080D + 5] = b5;
                mem[0x080D + 6] = b6;
                let view = LoadedImage::for_c64(&mem, cc_load, cc_end);
                let Some(matched) = cruel_cruncher::SPEC.detect(&view) else {
                    failures.push(format!(
                        "cruel_cruncher failed to match valid pair ({b5:#04X}, {b6:#04X})"
                    ));
                    continue;
                };
                if matched.spec_name != "Cruel Cruncher" {
                    failures.push(format!(
                        "cruel_cruncher unexpected name {:?} for ({b5:#04X}, {b6:#04X})",
                        matched.spec_name
                    ));
                }
                let info = detect_c64_info(&mem, cc_load, cc_end).unwrap();
                if info.name != "Cruel Cruncher"
                    || info.dep_addr != Some(0x0100)
                    || info.start_addr != Some(0x0801)
                {
                    failures.push(format!(
                        "cruel_cruncher unexpected info {info:?} for ({b5:#04X}, {b6:#04X})"
                    ));
                }
            }
        }

        // Exhaustively sweep `b5` (with valid `b6 = 0x1A`) and `b6` (with valid `b5 = 0xA2`).
        for b5 in 0x00_u8..=0xFF {
            let mut mem = cc_base.clone();
            mem[0x080D + 5] = b5;
            mem[0x080D + 6] = 0x1A;
            let view = LoadedImage::for_c64(&mem, cc_load, cc_end);
            let should_match = matches!(b5, 0xA2 | 0xA0);
            if cruel_cruncher::SPEC.detect(&view).is_some() != should_match {
                failures.push(format!(
                    "cruel_cruncher b5={b5:#04X}: expected match={should_match}"
                ));
            }
        }
        for b6 in 0x00_u8..=0xFF {
            let mut mem = cc_base.clone();
            mem[0x080D + 5] = 0xA2;
            mem[0x080D + 6] = b6;
            let view = LoadedImage::for_c64(&mem, cc_load, cc_end);
            let should_match = matches!(b6, 0x1A | 0x1F | 0x20);
            if cruel_cruncher::SPEC.detect(&view).is_some() != should_match {
                failures.push(format!(
                    "cruel_cruncher b6={b6:#04X}: expected match={should_match}"
                ));
            }
        }

        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    /// T3.1.6: `tiny_crunch` reads its 16-bit little-endian `entry_point` operand from
    /// `$0836..=$0837` (`$080D + 0x29`).
    #[test]
    fn tiny_crunch_entry_operand_from_0836() {
        assert_eq!(
            tiny_crunch::SPEC.hints.entry_point,
            Some(AddrSource::OperandLe { from_match: 0x29 })
        );

        let (base_mem, load_addr, load_end) = tiny_crunch::SPEC.build_fixture_memory();
        for expected_entry in [0x0000_u16, 0x080D, 0x0911, 0x1000, 0x4567, 0xC000, 0xFFFF] {
            let mut mem = base_mem.clone();
            let [lo, hi] = expected_entry.to_le_bytes();
            mem[0x0836] = lo;
            mem[0x0837] = hi;

            let view = LoadedImage::for_c64(&mem, load_addr, load_end);
            let sig_match = tiny_crunch::SPEC
                .detect(&view)
                .expect("tiny_crunch matches");
            assert_eq!(sig_match.anchor_addr, 0x080D);

            let spec_info = PackerInfo::resolve(&view, &sig_match);
            assert_eq!(spec_info.name, "TinyCrunch");
            assert_eq!(spec_info.dep_addr, Some(0x010E));
            assert_eq!(spec_info.start_addr, Some(0x0801));
            assert_eq!(spec_info.entry_point, Some(expected_entry));

            let reg_info = detect_c64_info(&mem, load_addr, load_end).unwrap();
            assert_eq!(reg_info, spec_info);
        }
    }

    /// T3.1.7 (Ruling B5): Bug-compatible CCS does NOT match when the 4 variant patterns
    /// are placed at their intended C64 addresses (`$0817`, `$0812`, `$0814`, `$080B`)
    /// with `load_addr == 0x0801`.
    #[test]
    fn ccs_bug_compat_does_not_match_with_correct_bytes() {
        const VARIANTS: [(usize, [u8; 4]); 4] = [
            (0x0817, [0x78, 0xE6, 0x01, 0xB9]),
            (0x0812, [0xA0, 0x00, 0x78, 0xE6]),
            (0x0814, [0x78, 0xE6, 0x01, 0xB9]),
            (0x080B, [0xA0, 0x00, 0x78, 0x8C]),
        ];

        // 1. Each variant individually at its intended $08xx address with load_addr = $0801.
        for (intended_addr, bytes) in VARIANTS {
            let mut mem = vec![0u8; 0x1_0000];
            mem[intended_addr..intended_addr + 4].copy_from_slice(&bytes);
            let view = LoadedImage::for_c64(&mem, 0x0801, 0x0900);
            assert!(
                ccs::SPEC.detect(&view).is_none(),
                "ccs must NOT match intended address ${intended_addr:04X} at load_addr=$0801 (B5)"
            );
            assert!(
                detect_c64_info(&mem, 0x0801, 0x0900).is_none(),
                "detect_c64_info must NOT match CCS at ${intended_addr:04X} with load_addr=$0801"
            );

            // Also test with C64 zero-page initialized ($00=$2F, $01=$37, BASIC pointers at $2B..$32).
            let mut zp_mem = vec![0u8; 0x1_0000];
            zp_mem[0x0000] = 0x2F;
            zp_mem[0x0001] = 0x37;
            zp_mem[0x002B] = 0x01;
            zp_mem[0x002C] = 0x08;
            zp_mem[intended_addr..intended_addr + 4].copy_from_slice(&bytes);
            let zp_view = LoadedImage::for_c64(&zp_mem, 0x0801, 0x0900);
            assert!(
                ccs::SPEC.detect(&zp_view).is_none(),
                "ccs must NOT match intended address ${intended_addr:04X} with C64 zero-page (B5)"
            );
        }

        // 2. All 4 variants placed simultaneously at their intended $08xx addresses with load_addr = $0801.
        let mut combined = vec![0u8; 0x1_0000];
        for (intended_addr, bytes) in VARIANTS {
            combined[intended_addr..intended_addr + 4].copy_from_slice(&bytes);
        }
        let combined_view = LoadedImage::for_c64(&combined, 0x0801, 0x0900);
        assert!(ccs::SPEC.detect(&combined_view).is_none());
    }

    /// T3.1.8 (Ruling R14): Bug-compatible CCS matches ONLY at the legacy index
    /// `target - load_addr` when `load_addr <= 0x0801`, and NEVER matches or underflows
    /// when `load_addr > 0x0801`.
    #[test]
    fn ccs_bug_compat_matches_only_legacy_index() {
        const VARIANTS: [(usize, [u8; 4]); 4] = [
            (0x0817, [0x78, 0xE6, 0x01, 0xB9]),
            (0x0812, [0xA0, 0x00, 0x78, 0xE6]),
            (0x0814, [0x78, 0xE6, 0x01, 0xB9]),
            (0x080B, [0xA0, 0x00, 0x78, 0x8C]),
        ];

        let mut failures = Vec::new();

        // 1. For every variant and every valid `load_addr <= 0x0801`, placing the 4 bytes at
        // `target - load_addr` matches, while `idx - 1` and `idx + 1` do not.
        for (target, bytes) in VARIANTS {
            for load_addr in [0x0000_u16, 0x0400, 0x0800, 0x0801] {
                let bug_idx = target - usize::from(load_addr);
                let mut mem = vec![0u8; 0x1_0000];
                mem[bug_idx..bug_idx + 4].copy_from_slice(&bytes);
                let view = LoadedImage::for_c64(&mem, load_addr, 0x0900);
                let Some(matched) = ccs::SPEC.detect(&view) else {
                    failures.push(format!(
                        "ccs failed to match variant ${target:04X} at bug index ${bug_idx:04X} (load=${load_addr:04X})"
                    ));
                    continue;
                };
                let info = PackerInfo::resolve(&view, &matched);
                if info.name != "Commodore Cruncher System"
                    || info.dep_addr != Some(0x0100)
                    || info.start_addr != Some(0x0801)
                {
                    failures.push(format!("ccs unexpected info: {info:?}"));
                }

                // Shifted by -1 or +1 must not match.
                if bug_idx > 0 {
                    let mut shifted_minus = vec![0u8; 0x1_0000];
                    shifted_minus[bug_idx - 1..bug_idx + 3].copy_from_slice(&bytes);
                    let minus_view = LoadedImage::for_c64(&shifted_minus, load_addr, 0x0900);
                    if ccs::SPEC.detect(&minus_view).is_some() {
                        failures.push(format!(
                            "ccs matched at bug_idx - 1 for target=${target:04X}, load=${load_addr:04X}"
                        ));
                    }
                }
                let mut shifted_plus = vec![0u8; 0x1_0000];
                shifted_plus[bug_idx + 1..bug_idx + 5].copy_from_slice(&bytes);
                let plus_view = LoadedImage::for_c64(&shifted_plus, load_addr, 0x0900);
                if ccs::SPEC.detect(&plus_view).is_some() {
                    failures.push(format!(
                        "ccs matched at bug_idx + 1 for target=${target:04X}, load=${load_addr:04X}"
                    ));
                }
            }
        }

        // 2. Ruling R14: `load_addr > 0x0801` is rejected before any subtraction and inside
        // `resolve_ccs` via `load_addr > 0x0801` and `checked_sub`, so it never matches or underflows.
        for bad_load in [
            0x0802_u16, 0x080B, 0x080C, 0x0812, 0x0814, 0x0817, 0x0818, 0x1000, 0xFFFF,
        ] {
            for (target, bytes) in VARIANTS {
                let mut mem = vec![0u8; 0x1_0000];
                // Place bytes at $0000, at `target`, and at `target.saturating_sub(bad_load)`.
                mem[0..4].copy_from_slice(&bytes);
                mem[target..target + 4].copy_from_slice(&bytes);
                let sat = target.saturating_sub(usize::from(bad_load));
                mem[sat..sat + 4].copy_from_slice(&bytes);

                let view = LoadedImage::for_c64(&mem, bad_load, 0x0900);
                if ccs::SPEC.detect(&view).is_some() {
                    failures.push(format!(
                        "ccs matched at bad_load=${bad_load:04X} > $0801 (target=${target:04X})"
                    ));
                }
                if detect_c64_info(&mem, bad_load, 0x0900).is_some() {
                    failures.push(format!(
                        "detect_c64_info matched CCS at bad_load=${bad_load:04X} > $0801"
                    ));
                }
                // Also call `resolve` directly to verify R14's guard inside `resolve_ccs` itself.
                if let Some(resolve_fn) = ccs::SPEC.resolve {
                    let dummy_match = SignatureMatch {
                        spec_id: ccs::SPEC.id,
                        spec_name: ccs::SPEC.name,
                        anchor_addr: bad_load,
                        match_addrs: Vec::new(),
                        resolved: ResolvedSpec::from_hints(ccs::SPEC.hints),
                    };
                    if resolve_fn(&view, &dummy_match).is_some() {
                        failures.push(format!(
                            "resolve_ccs returned Some at bad_load=${bad_load:04X} > $0801 (R14)"
                        ));
                    }
                }
            }
        }

        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    /// T3.1.9: Unpacking the corpus files covered by the M3.2 static packers
    /// (`c64_thats_the_way_scoop.time_cruncher.prg` and `c64_traveller.tiny_crunch.prg`)
    /// produces the exact baseline outputs (`start_addr`, `end_addr`, `entry_point`,
    /// `dep_addr`, `packer_name`, `instructions_executed`, and payload length).
    #[test]
    fn time_cruncher_corpus_unchanged() {
        struct ExpectedCorpusUnpack {
            file: &'static str,
            packer_name: &'static str,
            start_addr: u16,
            end_addr: u16,
            entry_point: u16,
            dep_addr: u16,
            instructions_executed: u64,
            len: usize,
        }

        const CASES: [ExpectedCorpusUnpack; 2] = [
            ExpectedCorpusUnpack {
                file: "c64_thats_the_way_scoop.time_cruncher.prg",
                packer_name: "Time Cruncher",
                start_addr: 0x0801,
                end_addr: 0xE750,
                entry_point: 0x0801,
                dep_addr: 0x0100,
                instructions_executed: 2_546_675,
                len: 57_168,
            },
            ExpectedCorpusUnpack {
                file: "c64_traveller.tiny_crunch.prg",
                packer_name: "TinyCrunch",
                start_addr: 0x0801,
                end_addr: 0x7949,
                entry_point: 0x0911,
                dep_addr: 0x010E,
                instructions_executed: 282_753,
                len: 29_001,
            },
        ];

        let mut failures = Vec::new();
        for case in CASES {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/6502")
                .join(case.file);
            let bytes = std::fs::read(&path)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
            assert!(bytes.len() > 2, "{} is too short", case.file);
            let load_addr = u16::from_le_bytes([bytes[0], bytes[1]]);
            let payload = &bytes[2..];

            let result = match unpack(payload, load_addr, &UnpackConfig::default(), None) {
                Ok(res) => res,
                Err(e) => {
                    failures.push(format!("{}: unpack failed: {e}", case.file));
                    continue;
                }
            };

            if result.packer_name.as_deref() != Some(case.packer_name) {
                failures.push(format!(
                    "{}: packer_name {:?} != Some({:?})",
                    case.file, result.packer_name, case.packer_name
                ));
            }
            if result.start_addr != case.start_addr {
                failures.push(format!(
                    "{}: start_addr ${:04X} != ${:04X}",
                    case.file, result.start_addr, case.start_addr
                ));
            }
            if result.end_addr != case.end_addr {
                failures.push(format!(
                    "{}: end_addr ${:04X} != ${:04X}",
                    case.file, result.end_addr, case.end_addr
                ));
            }
            if result.entry_point != case.entry_point {
                failures.push(format!(
                    "{}: entry_point ${:04X} != ${:04X}",
                    case.file, result.entry_point, case.entry_point
                ));
            }
            if result.dep_addr != case.dep_addr {
                failures.push(format!(
                    "{}: dep_addr ${:04X} != ${:04X}",
                    case.file, result.dep_addr, case.dep_addr
                ));
            }
            if result.instructions_executed != case.instructions_executed {
                failures.push(format!(
                    "{}: instructions_executed {} != {}",
                    case.file, result.instructions_executed, case.instructions_executed
                ));
            }
            if result.data.len() != case.len {
                failures.push(format!(
                    "{}: data.len() {} != {}",
                    case.file,
                    result.data.len(),
                    case.len
                ));
            }
        }

        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    // =======================================================================
    // M3.3 — The 4 computed packers (`T3.2.1`–`T3.2.7`)
    // =======================================================================

    /// T3.2.1: `card_cruncher` ACM variant tries candidate addresses `[$080D, $080F]` in order,
    /// checks the 17-byte `ACM_SIGNATURE` at `q`, reads absolute operands at fixed `$081E..=$081F`
    /// (`end_val > load_addr => Some(end_val - 1)`, `end_val <= load_addr => None`) and
    /// `$083F..=$0840` (`entry_point`), sets `start_addr` via `AddrSource::BasicStart(0)`, and
    /// enforces `load_addr <= 0x0801`.
    #[test]
    fn card_cruncher_candidates_and_absolute_operands() {
        const ACM_BYTES: [u8; 17] = [
            0x78, 0xA2, 0xF0, 0x86, 0x01, 0xBD, 0x40, 0x08, 0x9D, 0x00, 0x01, 0xCA, 0xD0, 0xF7,
            0x4C, 0x00, 0x01,
        ];

        assert_eq!(
            card_cruncher::ACM_CANDIDATES,
            [Anchor::Absolute(0x080D), Anchor::Absolute(0x080F)]
        );
        assert_eq!(card_cruncher::SPEC.load_addr_max, Some(0x0801));

        // 1. Candidate $080D matches and reads fixed operands at $081E and $083F.
        let (base_mem, load_addr, load_end) = card_cruncher::SPEC.build_fixture_memory();
        let base_view = LoadedImage::for_c64(&base_mem, load_addr, load_end);
        let matched_080d = card_cruncher::SPEC
            .detect(&base_view)
            .expect("ACM matches at $080D");
        assert_eq!(matched_080d.anchor_addr, 0x080D);
        assert_eq!(matched_080d.spec_name, "1001 CardCruncher ACM");
        assert_eq!(
            matched_080d.resolved.hints.start_addr,
            Some(AddrSource::BasicStart(0))
        );
        let info_080d = PackerInfo::resolve(&base_view, &matched_080d);
        assert_eq!(
            info_080d,
            PackerInfo {
                name: "1001 CardCruncher ACM",
                dep_addr: Some(0x0100),
                start_addr: Some(0x0801),
                end_addr: Some(0x319C),
                entry_point: Some(0x3197),
                end_addr_ptr: None,
            }
        );

        // 2. Candidate $080F matches when $080D does not match; operands remain at fixed $081E / $083F.
        //    Note: at $080F, the 17-byte signature covers $080F..=$081F, so $081E..=$081F is `0x00, 0x01`
        //    (`JMP $0100` target `0x0100 <= load_addr`), yielding `end_addr: None` when `load_addr == 0x0801`,
        //    or `Some(0x00FF)` when `load_addr == 0x0000`.
        let mut mem_080f = vec![0u8; 0x1_0000];
        mem_080f[0x080F..0x080F + 17].copy_from_slice(&ACM_BYTES);
        mem_080f[0x083F..0x0841].copy_from_slice(&0x2345_u16.to_le_bytes());
        let view_080f = LoadedImage::for_c64(&mem_080f, 0x0801, 0x0950);
        let matched_080f = card_cruncher::SPEC
            .detect(&view_080f)
            .expect("ACM matches at candidate $080F");
        assert_eq!(matched_080f.anchor_addr, 0x080F);
        let info_080f = PackerInfo::resolve(&view_080f, &matched_080f);
        assert_eq!(info_080f.name, "1001 CardCruncher ACM");
        assert_eq!(info_080f.dep_addr, Some(0x0100));
        assert_eq!(info_080f.start_addr, Some(0x0801));
        assert_eq!(info_080f.end_addr, None);
        assert_eq!(info_080f.entry_point, Some(0x2345));

        // When `$080D` has a partial `0x78, 0xA2` header and `$080F` has the full 17-byte `ACM_SIGNATURE`,
        // candidate probing advances from `$080D` to `$080F`.
        let mut mem_continue_to_080f = mem_080f.clone();
        mem_continue_to_080f[0x080D] = 0x78;
        mem_continue_to_080f[0x080E] = 0xA2;
        let view_continue_080f = LoadedImage::for_c64(&mem_continue_to_080f, 0x0801, 0x0950);
        let matched_continue_080f = card_cruncher::SPEC
            .detect(&view_continue_080f)
            .expect("ACM matches at $080F when $080D has partial prefix");
        assert_eq!(matched_continue_080f.anchor_addr, 0x080F);

        // When `$080F` matches the 9-byte `SIGNATURES` prefilter (`78 A2 __ __ __ BD __ __ 9D`)
        // but fails the 17-byte `ACM_SIGNATURE` check in `resolve_card_cruncher`, and `$0815`
        // has `V4_SIGNATURE`, per-position `continue` advances from `$080F` to `$0815`.
        let mut mem_080f_prefilter_then_v4 = vec![0u8; 0x1_0000];
        mem_080f_prefilter_then_v4[0x080F] = 0x78;
        mem_080f_prefilter_then_v4[0x0810] = 0xA2;
        mem_080f_prefilter_then_v4[0x0814] = 0xBD;
        // $0815..$081E = V4_SIGNATURE, with $0817 ($080F + 8) = 0x9D so $080F matches the 9-byte prefilter.
        mem_080f_prefilter_then_v4[0x0815..0x081E]
            .copy_from_slice(&[0x86, 0x01, 0x9D, 0x00, 0x00, 0x2A, 0x08, 0x99, 0xFA]);
        mem_080f_prefilter_then_v4[0x082E..0x0830].copy_from_slice(&0x3333_u16.to_le_bytes());
        mem_080f_prefilter_then_v4[0x0940..0x0942].copy_from_slice(&0x4444_u16.to_le_bytes());
        let view_prefilter_v4 = LoadedImage::for_c64(&mem_080f_prefilter_then_v4, 0x0801, 0x0950);
        let matched_v4 = card_cruncher::SPEC
            .detect(&view_prefilter_v4)
            .expect("rejects partial ACM at $080F and continues to V4 at $0815");
        assert_eq!(matched_v4.anchor_addr, 0x0815);
        assert_eq!(matched_v4.spec_name, "1001 CardCruncher v4");

        // 3. `end_val > load_addr` boundary at $081E..=$081F:
        //    - `end_val == load_addr` ($0801) -> `end_addr: None`
        //    - `end_val < load_addr` ($0800, $0000) -> `end_addr: None`
        //    - `end_val == load_addr + 1` ($0802) -> `end_addr: Some(0x0801)`
        for (end_val, expected_end) in [
            (0x0000_u16, None),
            (0x0800, None),
            (0x0801, None),
            (0x0802, Some(0x0801)),
            (0xFFFF, Some(0xFFFE)),
        ] {
            let mut mem = base_mem.clone();
            mem[0x081E..0x0820].copy_from_slice(&end_val.to_le_bytes());
            let view = LoadedImage::for_c64(&mem, 0x0801, 0x0950);
            let m = card_cruncher::SPEC.detect(&view).unwrap();
            let info = PackerInfo::resolve(&view, &m);
            assert_eq!(
                info.end_addr, expected_end,
                "end_val={end_val:#06X} -> expected {expected_end:?}"
            );
        }

        // 4. `load_addr <= 0x0801` gate and single-byte corruptions across all non-wildcard bytes
        //    of `ACM_SIGNATURE` at $080D..$081E, plus wildcards accepting arbitrary values.
        for bad_load in [0x0802_u16, 0x080D, 0x1000] {
            let view = LoadedImage::for_c64(&base_mem, bad_load, 0x0950);
            assert!(card_cruncher::SPEC.detect(&view).is_none());
        }
        for (offset, &pat) in card_cruncher::ACM_SIGNATURE.pattern.iter().enumerate() {
            let addr = 0x080D + offset;
            if matches!(pat, Pat::Any) {
                for b in [0x00_u8, 0x55, 0xAA, 0xFF] {
                    let mut m = base_mem.clone();
                    m[addr] = b;
                    let view = LoadedImage::for_c64(&m, 0x0801, 0x0950);
                    assert!(
                        card_cruncher::SPEC.detect(&view).is_some(),
                        "ACM wildcard at offset +{offset} (${addr:04X}) should accept {b:#04X}"
                    );
                }
            } else {
                let mut corrupted = base_mem.clone();
                corrupted[addr] ^= 0xFF;
                let view = LoadedImage::for_c64(&corrupted, 0x0801, 0x0950);
                assert!(
                    card_cruncher::SPEC.detect(&view).is_none(),
                    "ACM should reject single-byte corruption at offset +{offset} (${addr:04X})"
                );
            }
        }
    }

    /// T3.2.2: `card_cruncher` Variant 2 (`1001 CardCruncher v4`) matches at `$0815`
    /// (`86 01 __ __ __ 2A 08 99 FA`), reads `end_addr` from `$082E..=$082F` and `entry_point`
    /// from `$0940..=$0941`, sets `start_addr` via `AddrSource::BasicStart(0)`, and enforces
    /// `mem.len() > 0x0941`.
    #[test]
    fn card_cruncher_variant_two_signature() {
        let mut base_mem = vec![0u8; 0x1_0000];
        base_mem[0x0815..0x081E]
            .copy_from_slice(&[0x86, 0x01, 0x11, 0x22, 0x33, 0x2A, 0x08, 0x99, 0xFA]);
        base_mem[0x082E..0x0830].copy_from_slice(&0x4567_u16.to_le_bytes());
        base_mem[0x0940..0x0942].copy_from_slice(&0x1234_u16.to_le_bytes());

        // 1. Baseline match and resolved PackerInfo + registry name lookup.
        let view = LoadedImage::for_c64(&base_mem, 0x0801, 0x0950);
        let sig_match = card_cruncher::SPEC
            .detect(&view)
            .expect("CardCruncher v4 matches");
        assert_eq!(sig_match.spec_id, "card_cruncher");
        assert_eq!(sig_match.spec_name, "1001 CardCruncher v4");
        assert_eq!(sig_match.anchor_addr, 0x0815);
        assert_eq!(
            sig_match.resolved.hints.start_addr,
            Some(AddrSource::BasicStart(0))
        );

        let info = PackerInfo::resolve(&view, &sig_match);
        assert_eq!(
            info,
            PackerInfo {
                name: "1001 CardCruncher v4",
                dep_addr: Some(0x0100),
                start_addr: Some(0x0801),
                end_addr: Some(0x4567),
                entry_point: Some(0x1234),
                end_addr_ptr: None,
            }
        );
        assert_eq!(detect_c64_info(&base_mem, 0x0801, 0x0950).unwrap(), info);
        assert_eq!(
            find_entry_by_name("1001 CardCruncher v4").map(|s| s.id),
            Some("card_cruncher")
        );

        // 2. Wildcards at $0817, $0818, $0819 accept any byte (`0x00..=0xFF`), while fixed
        //    bytes at $0815, $0816, $081A, $081B, $081C, $081D reject corruptions.
        for wildcard_addr in [0x0817_usize, 0x0818, 0x0819] {
            for b in [0x00_u8, 0x55, 0xAA, 0xFF] {
                let mut mem = base_mem.clone();
                mem[wildcard_addr] = b;
                let v = LoadedImage::for_c64(&mem, 0x0801, 0x0950);
                assert!(card_cruncher::SPEC.detect(&v).is_some());
            }
        }
        for fixed_addr in [0x0815_usize, 0x0816, 0x081A, 0x081B, 0x081C, 0x081D] {
            let mut mem = base_mem.clone();
            mem[fixed_addr] ^= 0xFF;
            let v = LoadedImage::for_c64(&mem, 0x0801, 0x0950);
            assert!(
                card_cruncher::SPEC.detect(&v).is_none(),
                "CardCruncher v4 should reject corruption at ${fixed_addr:04X}"
            );
        }

        // 3. `mem.len() > 0x0941` boundary check (`0x0941` rejects, `0x0942` matches).
        let short_view = LoadedImage::for_c64(&base_mem[..0x0941], 0x0801, 0x0941);
        assert!(card_cruncher::SPEC.detect(&short_view).is_none());
        let exact_view = LoadedImage::for_c64(&base_mem[..0x0942], 0x0801, 0x0942);
        assert!(card_cruncher::SPEC.detect(&exact_view).is_some());
    }

    /// T3.2.3: `eca` scans `0x080D..=0x0830` for `[0x2A; 4]` at `p + 0x3A..p + 0x3E`, checks
    /// the position-dependent 32-bit word `w08 == 0x2D9D0032.wrapping_add(p as u32)` at
    /// `p + 8..p + 12` (along with `w0c == 0xF710CA00` at `p + 12..p + 16`), and continues
    /// scanning later window positions when an earlier position matches `[0x2A; 4]` at `p + 0x3A`
    /// but fails the `resolve` check.
    #[test]
    fn eca_position_dependent_bytes() {
        let mut failures = Vec::new();

        // 1. At every position `p` in `0x080D..=0x0830`, the position-dependent word
        //    `0x2D9D0032 + p` matches at `p`, and fails if `w08` is computed for `p + 1` or `p - 1`.
        for p in 0x080D_usize..=0x0830 {
            let mut mem = vec![0u8; 0x1_0000];
            mem[p + 0x3A..p + 0x3E].copy_from_slice(&[0x2A, 0x2A, 0x2A, 0x2A]);
            mem[p..p + 4].copy_from_slice(&0x8434_A000_u32.to_le_bytes());
            mem[p + 4..p + 8].copy_from_slice(&0xBD05_A201_u32.to_le_bytes());
            let w08 = 0x2D9D_0032_u32.wrapping_add(p as u32);
            mem[p + 8..p + 12].copy_from_slice(&w08.to_le_bytes());
            mem[p + 12..p + 16].copy_from_slice(&0xF710_CA00_u32.to_le_bytes());

            let view = LoadedImage::for_c64(&mem, 0x0801, 0x0950);
            let Some(matched) = eca::SPEC.detect(&view) else {
                failures.push(format!("eca failed to match at p=${p:04X}"));
                continue;
            };
            if usize::from(matched.anchor_addr) != p {
                failures.push(format!(
                    "eca anchor_addr ${:04X} != expected ${p:04X}",
                    matched.anchor_addr
                ));
            }

            // Wrong `w08` (computed for `p + 1`) must NOT match at `p`.
            let mut bad_mem = mem.clone();
            let bad_w08 = 0x2D9D_0032_u32.wrapping_add((p + 1) as u32);
            bad_mem[p + 8..p + 12].copy_from_slice(&bad_w08.to_le_bytes());
            let bad_view = LoadedImage::for_c64(&bad_mem, 0x0801, 0x0950);
            if eca::SPEC.detect(&bad_view).is_some() {
                failures.push(format!("eca matched wrong w08 (p+1) at p=${p:04X}"));
            }
        }

        // 2. Per-position `continue` semantics: place `[0x2A; 4]` at `p1 + 0x3A` ($080D + $3A)
        //    with an invalid `w08`, and place a valid ECA header at `p2 = $0825`.
        //    `eca::SPEC.detect` must reject `p1` and continue to match `p2 = $0825`.
        let mut multi_mem = vec![0u8; 0x1_0000];
        let p1 = 0x080D_usize;
        let p2 = 0x0825_usize;
        multi_mem[p1 + 0x3A..p1 + 0x3E].copy_from_slice(&[0x2A, 0x2A, 0x2A, 0x2A]);
        multi_mem[p1..p1 + 4].copy_from_slice(&0x8434_A000_u32.to_le_bytes());
        multi_mem[p1 + 4..p1 + 8].copy_from_slice(&0xBD05_A201_u32.to_le_bytes());
        multi_mem[p1 + 8..p1 + 12].copy_from_slice(&0xDEAD_BEEF_u32.to_le_bytes());
        multi_mem[p1 + 12..p1 + 16].copy_from_slice(&0xF710_CA00_u32.to_le_bytes());

        multi_mem[p2 + 0x3A..p2 + 0x3E].copy_from_slice(&[0x2A, 0x2A, 0x2A, 0x2A]);
        multi_mem[p2..p2 + 4].copy_from_slice(&0x8434_A000_u32.to_le_bytes());
        multi_mem[p2 + 4..p2 + 8].copy_from_slice(&0xBD05_A201_u32.to_le_bytes());
        let w08_p2 = 0x2D9D_0032_u32.wrapping_add(p2 as u32);
        multi_mem[p2 + 8..p2 + 12].copy_from_slice(&w08_p2.to_le_bytes());
        multi_mem[p2 + 12..p2 + 16].copy_from_slice(&0xF710_CA00_u32.to_le_bytes());

        let multi_view = LoadedImage::for_c64(&multi_mem, 0x0801, 0x0950);
        let matched_p2 = eca::SPEC
            .detect(&multi_view)
            .expect("eca should continue past p1 and match p2");
        assert_eq!(usize::from(matched_p2.anchor_addr), p2);

        // 3. Outside the window (`$080C` and `$0831`) or `mem.len() < 0x0950` returns `None`.
        for out_of_window in [0x080C_usize, 0x0831] {
            let mut mem = vec![0u8; 0x1_0000];
            mem[out_of_window + 0x3A..out_of_window + 0x3E]
                .copy_from_slice(&[0x2A, 0x2A, 0x2A, 0x2A]);
            mem[out_of_window..out_of_window + 4].copy_from_slice(&0x8434_A000_u32.to_le_bytes());
            mem[out_of_window + 4..out_of_window + 8]
                .copy_from_slice(&0xBD05_A201_u32.to_le_bytes());
            let w08 = 0x2D9D_0032_u32.wrapping_add(out_of_window as u32);
            mem[out_of_window + 8..out_of_window + 12].copy_from_slice(&w08.to_le_bytes());
            mem[out_of_window + 12..out_of_window + 16]
                .copy_from_slice(&0xF710_CA00_u32.to_le_bytes());
            let view = LoadedImage::for_c64(&mem, 0x0801, 0x0950);
            if eca::SPEC.detect(&view).is_some() {
                failures.push(format!(
                    "eca matched outside window at ${out_of_window:04X}"
                ));
            }
        }

        let (fix_mem, fix_load, _) = eca::SPEC.build_fixture_memory();
        let short_view = LoadedImage::for_c64(&fix_mem[..0x094F], fix_load, 0x094F);
        assert!(eca::SPEC.detect(&short_view).is_none());

        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    /// T3.2.4: `eca` validates all bitmask and variant branches (Check 1 masks on `w00` and `wm3`,
    /// Check 2 Variant 2, Check 3 FDT, Check 4 Decibel), secondary `entry_point` scan in
    /// `p + 0xD6..p + 0xDE` (rejecting excluded KERNAL/BASIC targets), `end_addr_ptr` scan in
    /// `p + 0xED..p + 0x108`, and `BasicStart(-1)` (`$0800` on C64 per Ruling B4) / `Const(0x0100)`
    /// fallback hints in `eca::SPEC.hints` and `eca::resolve_hints_at`.
    #[test]
    fn eca_mask_and_fallbacks() {
        assert_eq!(eca::SPEC.hints.dep_addr, Some(AddrSource::Const(0x0100)));
        assert_eq!(eca::SPEC.hints.start_addr, Some(AddrSource::BasicStart(-1)));

        let p = 0x0812_usize;
        let w08 = 0x2D9D_0032_u32.wrapping_add(p as u32).to_le_bytes();
        let w0c = 0xF710_CA00_u32.to_le_bytes();

        // Helper initializing `[0x2A; 4]` at `p + 0x3A`, `dep_addr = $0100` at `p + 0x30`,
        // and `start_addr = $0800` at `p + 0x32`.
        let make_base = || {
            let mut m = vec![0u8; 0x1_0000];
            m[p + 0x30..p + 0x32].copy_from_slice(&0x0100_u16.to_le_bytes());
            m[p + 0x32..p + 0x34].copy_from_slice(&0x0800_u16.to_le_bytes());
            m[p + 0x3A..p + 0x3E].copy_from_slice(&[0x2A; 4]);
            m
        };

        // 1. All 4 sub-conditions of Check 1 (`w08` + `w0c` + `w00`/`wm3` mask), plus Check 2, Check 3, Check 4.
        let mut cases: Vec<Vec<u8>> = Vec::new();

        // 1a: `(w00 & 0xF4FFF000) == 0x8434A000 && w04 == 0xBD05A201` (with non-zero unmasked bits set).
        let mut c1a = make_base();
        c1a[p..p + 4].copy_from_slice(&(0x8434_A000_u32 | 0x0B00_0FFF).to_le_bytes());
        c1a[p + 4..p + 8].copy_from_slice(&0xBD05_A201_u32.to_le_bytes());
        c1a[p + 8..p + 12].copy_from_slice(&w08);
        c1a[p + 12..p + 16].copy_from_slice(&w0c);
        cases.push(c1a);

        // 1b: `(w00 & 0xFFFFFF00) == 0x04A27800 && w04 == 0xBDE80186`.
        let mut c1b = make_base();
        c1b[p..p + 4].copy_from_slice(&0x04A2_78FF_u32.to_le_bytes());
        c1b[p + 4..p + 8].copy_from_slice(&0xBDE8_0186_u32.to_le_bytes());
        c1b[p + 8..p + 12].copy_from_slice(&w08);
        c1b[p + 12..p + 16].copy_from_slice(&w0c);
        cases.push(c1b);

        // 1c: `(wm3 & 0xFFFFFF00) == 0x04A27800 && w04 == 0xBDE80186`.
        let mut c1c = make_base();
        c1c[p - 3..p + 1].copy_from_slice(&0x04A2_78AB_u32.to_le_bytes());
        c1c[p + 4..p + 8].copy_from_slice(&0xBDE8_0186_u32.to_le_bytes());
        c1c[p + 8..p + 12].copy_from_slice(&w08);
        c1c[p + 12..p + 16].copy_from_slice(&w0c);
        cases.push(c1c);

        // 1d: `wm3 == 0x8D00A978`.
        let mut c1d = make_base();
        c1d[p - 3..p + 1].copy_from_slice(&0x8D00_A978_u32.to_le_bytes());
        c1d[p + 8..p + 12].copy_from_slice(&w08);
        c1d[p + 12..p + 16].copy_from_slice(&w0c);
        cases.push(c1d);

        // Check 2: Variant 2 (`w02 == 0x8534A978` at `p + 2..p + 6`, `mem[p - 3] == 0xA0`).
        let mut c2 = make_base();
        c2[p - 3] = 0xA0;
        c2[p + 2..p + 6].copy_from_slice(&0x8534_A978_u32.to_le_bytes());
        cases.push(c2);

        // Check 3: FDT (`w03 == 0x8604A278` at `p + 3..p + 7`, `w0a == 0x2D950842` at `p + 10..p + 14`).
        let mut c3 = make_base();
        c3[p + 3..p + 7].copy_from_slice(&0x8604_A278_u32.to_le_bytes());
        c3[p + 10..p + 14].copy_from_slice(&0x2D95_0842_u32.to_le_bytes());
        cases.push(c3);

        // Check 4: Decibel hacks (`w00 == 0x9D085EBD` at `p..p + 4`, `wm6 == 0x018534A9` at `p - 6..p - 2`).
        let mut c4 = make_base();
        c4[p..p + 4].copy_from_slice(&0x9D08_5EBD_u32.to_le_bytes());
        c4[p - 6..p - 2].copy_from_slice(&0x0185_34A9_u32.to_le_bytes());
        cases.push(c4);

        for (idx, mem) in cases.iter().enumerate() {
            let view = LoadedImage::for_c64(mem, 0x0801, 0x0950);
            let m = eca::SPEC
                .detect(&view)
                .unwrap_or_else(|| panic!("eca case #{idx} failed to match"));
            let info = PackerInfo::resolve(&view, &m);
            assert_eq!(
                info,
                PackerInfo {
                    name: "ECA Compactor",
                    dep_addr: Some(0x0100),
                    start_addr: Some(0x0800),
                    end_addr: None,
                    entry_point: None,
                    end_addr_ptr: None,
                },
                "case #{idx} PackerInfo mismatch"
            );
        }

        // Verify `eca::resolve_hints_at` fallback when `mem` is shorter than `p + 0x31` / `p + 0x33`:
        // `dep_addr` falls back to `Some(AddrSource::Const(0x0100))` and `start_addr` falls back to
        // `Some(AddrSource::BasicStart(-1))` (which resolves to `$0800` on C64 per Ruling B4).
        let short_slice = &cases[0][..p + 0x30];
        let short_image = LoadedImage::for_c64(short_slice, 0x0801, 0x0830);
        let fallback_hints = eca::resolve_hints_at(&short_image, p);
        assert_eq!(fallback_hints.dep_addr, Some(AddrSource::Const(0x0100)));
        assert_eq!(fallback_hints.start_addr, Some(AddrSource::BasicStart(-1)));
        assert_eq!(
            fallback_hints
                .start_addr
                .unwrap()
                .resolve_static(&short_image, p as u16),
            Some(0x0800)
        );

        // 2. Secondary `entry_point` scan (`p + 0xD6..p + 0xDE`):
        //    - Excluded targets (`0xA659`, `0xFF81`, `0xE3BF`, `0xE5A0`, `0xE518`) are skipped.
        //    - Both `0x4C` (`JMP`) and `0x20` (`JSR`) opcodes are recognized.
        for excluded in [0xA659_u16, 0xFF81, 0xE3BF, 0xE5A0, 0xE518] {
            let mut mem = cases[0].clone();
            mem[p + 0xD6] = 0x4C;
            mem[p + 0xD7..p + 0xD9].copy_from_slice(&excluded.to_le_bytes());
            let view = LoadedImage::for_c64(&mem, 0x0801, 0x0950);
            let m = eca::SPEC.detect(&view).unwrap();
            let info = PackerInfo::resolve(&view, &m);
            assert_eq!(
                info.entry_point, None,
                "excluded target ${excluded:04X} must not be accepted as entry_point"
            );
        }

        // Excluded `4C A659` at `p + 0xD6` followed by valid `20 3000` (`JSR $3000`) at `p + 0xD9` picks `$3000`.
        let mut mem_ep_and_ptr = cases[0].clone();
        mem_ep_and_ptr[p + 0xD6] = 0x4C;
        mem_ep_and_ptr[p + 0xD7..p + 0xD9].copy_from_slice(&0xA659_u16.to_le_bytes());
        mem_ep_and_ptr[p + 0xD9] = 0x20;
        mem_ep_and_ptr[p + 0xDA..p + 0xDC].copy_from_slice(&0x3000_u16.to_le_bytes());
        // Secondary `end_addr_ptr` scan (`p + 0xED..p + 0x108`): `[0xD0, 0xF7, 0x18, 0xA5, ptr]`.
        mem_ep_and_ptr[p + 0xF0..p + 0xF5].copy_from_slice(&[0xD0, 0xF7, 0x18, 0xA5, 0xAE]);
        let view_both = LoadedImage::for_c64(&mem_ep_and_ptr, 0x0801, 0x0950);
        let m_both = eca::SPEC.detect(&view_both).unwrap();
        let info_both = PackerInfo::resolve(&view_both, &m_both);
        assert_eq!(info_both.entry_point, Some(0x3000));
        assert_eq!(info_both.end_addr_ptr, Some(0x00AE));
        assert_eq!(info_both.start_addr, Some(0x0800));
    }

    /// T3.2.5: `pucrunch` matches at `$0813` (`Anchor::BasicStart { offset: 0x12 }`), extracts
    /// `dep_addr` from `$0841..=$0842` (`OperandLe { from_match: 0x2E }`) and `start_addr` from
    /// `$0879..=$087A` (`OperandLe { from_match: 0x66 }`), sets `end_addr_ptr` to `$00FA`, and
    /// scans `0x0912..0x0938` (`Anchor::Window(0x0912..=0x0937)`) for the optional secondary
    /// `entry_point` signature (`A5 FA 85 2D A5 FB 85 2E`).
    #[test]
    fn pucrunch_second_scan_and_operands() {
        assert_eq!(
            pucrunch::SPEC.signatures[0].anchor,
            Anchor::BasicStart { offset: 0x12 }
        );
        assert_eq!(
            pucrunch::SPEC.hints.dep_addr,
            Some(AddrSource::OperandLe { from_match: 0x2E })
        );
        assert_eq!(
            pucrunch::SPEC.hints.start_addr,
            Some(AddrSource::OperandLe { from_match: 0x66 })
        );
        assert_eq!(
            pucrunch::ENTRY_SIGNATURE.anchor,
            Anchor::Window(0x0912..=0x0937)
        );

        let (base_mem, load_addr, load_end) = pucrunch::SPEC.build_fixture_memory();

        // 1. Secondary signature at each boundary and interior address (`$0912`, `$0920`, `$0937`).
        for p in [0x0912_usize, 0x0920, 0x0937] {
            let mut mem = base_mem.clone();
            // Clear the fixture's secondary signature at $0912 first.
            mem[0x0912..0x0944].fill(0);
            mem[0x0841..0x0843].copy_from_slice(&0x0123_u16.to_le_bytes());
            mem[0x0879..0x087B].copy_from_slice(&0x0800_u16.to_le_bytes());
            mem[p..p + 8].copy_from_slice(&[0xA5, 0xFA, 0x85, 0x2D, 0xA5, 0xFB, 0x85, 0x2E]);
            mem[p + 0x0A..p + 0x0C].copy_from_slice(&0x2E00_u16.to_le_bytes());

            let view = LoadedImage::for_c64(&mem, load_addr, load_end);
            let m = pucrunch::SPEC
                .detect(&view)
                .expect("pucrunch matches with secondary sig");
            assert_eq!(m.anchor_addr, 0x0813);
            let info = PackerInfo::resolve(&view, &m);
            assert_eq!(
                info,
                PackerInfo {
                    name: "PUCrunch",
                    dep_addr: Some(0x0123),
                    start_addr: Some(0x0800),
                    end_addr: None,
                    entry_point: Some(0x2E00),
                    end_addr_ptr: Some(0x00FA),
                },
                "failed for secondary scan at p=${p:04X}"
            );
        }

        // 2. When secondary signature is absent (or placed outside `0x0912..=0x0937` at `$0911` or `$0938`),
        //    `pucrunch` still matches with `entry_point: None`.
        for out_of_range in [0x0911_usize, 0x0938] {
            let mut mem = base_mem.clone();
            mem[0x0912..0x0944].fill(0);
            mem[out_of_range..out_of_range + 8]
                .copy_from_slice(&[0xA5, 0xFA, 0x85, 0x2D, 0xA5, 0xFB, 0x85, 0x2E]);
            mem[out_of_range + 0x0A..out_of_range + 0x0C]
                .copy_from_slice(&0x4000_u16.to_le_bytes());
            let view = LoadedImage::for_c64(&mem, load_addr, load_end);
            let m = pucrunch::SPEC
                .detect(&view)
                .expect("pucrunch matches without secondary sig");
            let info = PackerInfo::resolve(&view, &m);
            assert_eq!(info.entry_point, None, "out_of_range=${out_of_range:04X}");
            assert_eq!(info.dep_addr, Some(0x0116));
            assert_eq!(info.start_addr, Some(0x0801));
            assert_eq!(info.end_addr_ptr, Some(0x00FA));
        }

        // 3. `mem.len() < 0x0944` guard rejects truncated buffers.
        let short_view = LoadedImage::for_c64(&base_mem[..0x0943], load_addr, 0x0943);
        assert!(pucrunch::SPEC.detect(&short_view).is_none());
    }

    /// T3.2.6: `tbc` checks `(mem[0x082C] & 0xFD) == 0x84` (`Pat::Mask { mask: 0xFD, eq: 0x84 }`),
    /// `$08A2..$08A6 == [0x01, 0x4C, 0x49, 0x01]`, normal (`$084A == 0x81`, `$0820..$0824`) and
    /// Firelord (`$084A == 0x7B`, `$081D..$0821`) variants, and the `$08EB` table-reduction edge
    /// values (`tbl_len == 0` -> `None`; multi-entry max; `0x0000` mapped to `0x10000` and
    /// `wrapping_sub(1)` -> `Some(0xFFFF)`; `0x0001` -> `Some(0x0000)`).
    #[test]
    fn tbc_table_reduction_edge_values() {
        const TBC_PRIMARY_SIG: [u8; 12] = [
            0x84, 0x01, 0xCA, 0x9A, 0x4C, 0x00, 0x01, 0xA0, 0x00, 0x84, 0xFD, 0x84,
        ];

        let (base_mem, load_addr, load_end) = tbc::SPEC.build_fixture_memory();

        // 1. Mask at $082C (`(b & 0xFD) == 0x84`) accepts only `0x84` and `0x86`.
        for b in 0x00_u8..=0xFF {
            let mut mem = base_mem.clone();
            mem[0x082C] = b;
            let view = LoadedImage::for_c64(&mem, load_addr, load_end);
            let should_match = b == 0x84 || b == 0x86;
            assert_eq!(
                tbc::SPEC.detect(&view).is_some(),
                should_match,
                "tbc mask check at $082C failed for byte {b:#04X}"
            );
        }

        // 2. Variant dispatch at $084A:
        //    - `$084A == 0x81`: header `[0xA2, 0xE9, 0xBD, 0x32]` at `$0820..$0824`, entry at `$08B2..=$08B3`.
        //    - `$084A == 0x7B` (Firelord): header `[0xA2, 0xE9, 0xBD, 0x32]` at `$081D..$0821`, entry at `$08AC..=$08AD`.
        //    - Any other value at `$084A` (or corrupted header bytes) returns `None`.
        let mut firelord = vec![0u8; 0x1_0000];
        firelord[0x081D..0x0821].copy_from_slice(&[0xA2, 0xE9, 0xBD, 0x32]);
        firelord[0x082C..0x082C + 12].copy_from_slice(&TBC_PRIMARY_SIG);
        firelord[0x082C] = 0x86;
        firelord[0x084A] = 0x7B;
        firelord[0x08A2..0x08A6].copy_from_slice(&[0x01, 0x4C, 0x49, 0x01]);
        firelord[0x08AC..0x08AE].copy_from_slice(&0x4000_u16.to_le_bytes());
        firelord[0x08EB] = 0x00; // `tbl_len == 0` -> `end_addr: None`
        firelord[0x08EC..0x08EE].copy_from_slice(&0x0801_u16.to_le_bytes());
        let fl_view = LoadedImage::for_c64(&firelord, 0x0801, 0x0900);
        let fl_match = tbc::SPEC.detect(&fl_view).expect("Firelord TBC matches");
        let fl_info = PackerInfo::resolve(&fl_view, &fl_match);
        assert_eq!(
            fl_info,
            PackerInfo {
                name: "TBC Multicompactor",
                dep_addr: Some(0x0100),
                start_addr: Some(0x0801),
                end_addr: None,
                entry_point: Some(0x4000),
                end_addr_ptr: None,
            }
        );

        let mut bad_variant = base_mem.clone();
        bad_variant[0x084A] = 0x00;
        assert!(
            tbc::SPEC
                .detect(&LoadedImage::for_c64(&bad_variant, load_addr, load_end))
                .is_none()
        );

        // 3. Table reduction edge cases at `$08EB` (`p = 0x08EB`, `tbl_len = mem[p]`,
        //    `q = p + tbl_len`, stepping down by 4 while `q > p`, reading `[mem[q - 1], mem[q]]`):
        // 3a: `tbl_len == 0` (`mem[0x08EB] == 0x00`) -> `end_addr: None`.
        let mut tbl_empty = base_mem.clone();
        tbl_empty[0x08EB] = 0x00;
        let m_empty = tbc::SPEC
            .detect(&LoadedImage::for_c64(&tbl_empty, load_addr, load_end))
            .unwrap();
        assert_eq!(m_empty.resolved.hints.end_addr, None);

        // 3b: `strtmp == 0x0001` (1 entry, `tbl_len == 4`, `q = 0x08EF`, reads `$08EE..=$08EF`)
        //     -> `end_addr: Some(0x0000)`.
        let mut tbl_one = base_mem.clone();
        tbl_one[0x08EB] = 4;
        tbl_one[0x08EE..0x08F0].copy_from_slice(&0x0001_u16.to_le_bytes());
        let view_one = LoadedImage::for_c64(&tbl_one, load_addr, load_end);
        let info_one = PackerInfo::resolve(&view_one, &tbc::SPEC.detect(&view_one).unwrap());
        assert_eq!(info_one.end_addr, Some(0x0000));

        // 3c: Multi-entry table (`tbl_len == 12`, 3 entries at `$08EE..=$08EF`, `$08F2..=$08F3`,
        //     `$08F6..=$08F7`) with values `0x2000`, `0x9000`, `0x4500`
        //     -> max is `0x9000`, `end_addr == Some(0x8FFF)`.
        let mut tbl_multi = base_mem.clone();
        tbl_multi[0x08EB] = 12;
        tbl_multi[0x08EE..0x08F0].copy_from_slice(&0x2000_u16.to_le_bytes());
        tbl_multi[0x08F2..0x08F4].copy_from_slice(&0x9000_u16.to_le_bytes());
        tbl_multi[0x08F6..0x08F8].copy_from_slice(&0x4500_u16.to_le_bytes());
        let view_multi = LoadedImage::for_c64(&tbl_multi, load_addr, load_end);
        let info_multi = PackerInfo::resolve(&view_multi, &tbc::SPEC.detect(&view_multi).unwrap());
        assert_eq!(info_multi.end_addr, Some(0x8FFF));

        // 3d: `strtmp == 0x0000` maps to `0x10000` (beating `0x9000`), and
        //     `(0x10000 as u16).wrapping_sub(1)` yields `Some(0xFFFF)`.
        let mut tbl_wrap = tbl_multi.clone();
        tbl_wrap[0x08F6..0x08F8].copy_from_slice(&0x0000_u16.to_le_bytes());
        let view_wrap = LoadedImage::for_c64(&tbl_wrap, load_addr, load_end);
        let info_wrap = PackerInfo::resolve(&view_wrap, &tbc::SPEC.detect(&view_wrap).unwrap());
        assert_eq!(info_wrap.end_addr, Some(0xFFFF));

        // 4. `mem.len() < 0x08F0` guard rejects truncated buffers.
        let short_view = LoadedImage::for_c64(&base_mem[..0x08EF], load_addr, 0x08EF);
        assert!(tbc::SPEC.detect(&short_view).is_none());
    }

    /// T3.2.7: Unpacking all 6 corpus files covered by the 4 M3.3 computed packers
    /// (`card_cruncher`, `eca`, `pucrunch`, `tbc`) produces the exact baseline outputs
    /// (`packer_name`, `start_addr`, `end_addr`, `entry_point`, `dep_addr`,
    /// `instructions_executed`, and payload length).
    #[test]
    fn computed_packers_corpus_unchanged() {
        struct ExpectedCorpusUnpack {
            file: &'static str,
            packer_name: &'static str,
            start_addr: u16,
            end_addr: u16,
            entry_point: u16,
            dep_addr: u16,
            instructions_executed: u64,
            len: usize,
        }

        const CASES: [ExpectedCorpusUnpack; 6] = [
            ExpectedCorpusUnpack {
                file: "c64_323_ice_psm.1001_card_cruncher.prg",
                packer_name: "1001 CardCruncher ACM",
                start_addr: 0x07C1,
                end_addr: 0x319C,
                entry_point: 0x3197,
                dep_addr: 0x0100,
                instructions_executed: 314_707,
                len: 10_716,
            },
            ExpectedCorpusUnpack {
                file: "c64_fantasy_intro.eca_compactor.prg",
                packer_name: "ECA Compactor",
                start_addr: 0x0800,
                end_addr: 0x37FF,
                entry_point: 0x3000,
                dep_addr: 0x0100,
                instructions_executed: 151_677,
                len: 12_288,
            },
            ExpectedCorpusUnpack {
                file: "c64_connection-8580.pucrunch.prg",
                packer_name: "PUCrunch",
                start_addr: 0x0801,
                end_addr: 0xF87D,
                entry_point: 0x0810,
                dep_addr: 0x0116,
                instructions_executed: 1_162_113,
                len: 61_565,
            },
            ExpectedCorpusUnpack {
                file: "c64_moving_tubes_lxt.pucrunch.prg",
                packer_name: "PUCrunch",
                start_addr: 0x0800,
                end_addr: 0x31FF,
                entry_point: 0x2E00,
                dep_addr: 0x0116,
                instructions_executed: 568_585,
                len: 10_752,
            },
            ExpectedCorpusUnpack {
                file: "c64_mule.pucrunch.prg",
                packer_name: "PUCrunch",
                start_addr: 0x0800,
                end_addr: 0x9D19,
                entry_point: 0x1100,
                dep_addr: 0x0116,
                instructions_executed: 1_663_122,
                len: 38_170,
            },
            ExpectedCorpusUnpack {
                file: "c64_gianna_sister_remix_badboy.tbc_multicompactor.prg",
                packer_name: "TBC Multicompactor",
                start_addr: 0x0801,
                end_addr: 0xC947,
                entry_point: 0x0810,
                dep_addr: 0x0100,
                instructions_executed: 1_690_927,
                len: 49_479,
            },
        ];

        let mut failures = Vec::new();
        for case in CASES {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/6502")
                .join(case.file);
            let bytes = std::fs::read(&path)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
            assert!(bytes.len() > 2, "{} is too short", case.file);
            let load_addr = u16::from_le_bytes([bytes[0], bytes[1]]);
            let payload = &bytes[2..];

            let result = match unpack(payload, load_addr, &UnpackConfig::default(), None) {
                Ok(res) => res,
                Err(e) => {
                    failures.push(format!("{}: unpack failed: {e}", case.file));
                    continue;
                }
            };

            if result.packer_name.as_deref() != Some(case.packer_name) {
                failures.push(format!(
                    "{}: packer_name {:?} != Some({:?})",
                    case.file, result.packer_name, case.packer_name
                ));
            }
            if result.start_addr != case.start_addr {
                failures.push(format!(
                    "{}: start_addr ${:04X} != ${:04X}",
                    case.file, result.start_addr, case.start_addr
                ));
            }
            if result.end_addr != case.end_addr {
                failures.push(format!(
                    "{}: end_addr ${:04X} != ${:04X}",
                    case.file, result.end_addr, case.end_addr
                ));
            }
            if result.entry_point != case.entry_point {
                failures.push(format!(
                    "{}: entry_point ${:04X} != ${:04X}",
                    case.file, result.entry_point, case.entry_point
                ));
            }
            if result.dep_addr != case.dep_addr {
                failures.push(format!(
                    "{}: dep_addr ${:04X} != ${:04X}",
                    case.file, result.dep_addr, case.dep_addr
                ));
            }
            if result.instructions_executed != case.instructions_executed {
                failures.push(format!(
                    "{}: instructions_executed {} != {}",
                    case.file, result.instructions_executed, case.instructions_executed
                ));
            }
            if result.data.len() != case.len {
                failures.push(format!(
                    "{}: data.len() {} != {}",
                    case.file,
                    result.data.len(),
                    case.len
                ));
            }
        }

        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    // =======================================================================
    // M3.4 — Exomizer + TSCrunch (`T3.3.1`–`T3.3.8`, `T3.3.12`, `T3.3.13`)
    // =======================================================================

    /// Helper writing an Exomizer 3.x decruncher signature at `p` in `mem`.
    fn write_exo3_at(
        mem: &mut [u8],
        p: usize,
        dep_low: u8,
        is_v30: bool,
        entry_point: Option<u16>,
    ) {
        mem[p - 6] = 0x4C;
        mem[p - 5] = dep_low;
        mem[p - 4] = 0x01;
        mem[p..p + 8].copy_from_slice(&[0x69, 0x80, 0x0A, 0x10, 0x0F, 0x06, 0xFD, 0xD0]);
        if is_v30 {
            mem[p + 8..=p + 12].copy_from_slice(&[0x08, 0x48, 0x20, 0x1A, 0x01]);
        } else {
            mem[p + 8] = 0x99;
        }
        if let Some(ep) = entry_point {
            mem[p + 0x10] = 0x20;
            mem[p + 0x15] = 0x4C;
            mem[p + 0x16..=p + 0x17].copy_from_slice(&ep.to_le_bytes());
        }
    }

    /// Helper writing an Exomizer 1.x / 2.x signature at `p` in `mem`.
    fn write_exo2_at(mem: &mut [u8], p: usize, cpy_imm: u8) {
        mem[p] = 0xC8;
        mem[p + 1] = 0xC0;
        mem[p + 2] = cpy_imm;
        mem[p + 3] = 0xD0;
        mem[p + 7] = 0x4C;
    }

    /// T3.3.1: `exomizer` detects all three variants (`Exomizer 3.0`, `Exomizer v3.02+`,
    /// and `Exomizer 2.x` with both `0x34` and `0x50` via `Pat::OneOf`), resolves their
    /// `dep_addr`, `start_addr` (`BasicStart(0)` for 2.x), and optional `entry_point`,
    /// and maps all three display names via `find_entry_by_name`.
    #[test]
    fn exomizer_variants_detect() {
        assert_eq!(
            exomizer::EXO2_SIGNATURE.pattern[2],
            Pat::OneOf(&[0x34, 0x50])
        );
        for name in ["Exomizer 3.0", "Exomizer v3.02+", "Exomizer 2.x"] {
            let entry = find_entry_by_name(name)
                .unwrap_or_else(|| panic!("missing registry entry for `{name}`"));
            assert_eq!(entry.id, "exomizer");
        }

        // 1. Exomizer 3.0 with and without the optional JSR/JMP entry point sequence.
        let mut mem_v30 = vec![0u8; 0x1_0000];
        write_exo3_at(&mut mem_v30, 0x0900, 0x34, true, Some(0x0820));
        let view_v30 = LoadedImage::for_c64(&mem_v30, 0x0801, 0x0950);
        let m_v30 = exomizer::SPEC
            .detect(&view_v30)
            .expect("Exomizer 3.0 matches");
        assert_eq!(m_v30.spec_name, "Exomizer 3.0");
        assert_eq!(m_v30.anchor_addr, 0x0900);
        let info_v30 = PackerInfo::resolve(&view_v30, &m_v30);
        assert_eq!(
            info_v30,
            PackerInfo {
                name: "Exomizer 3.0",
                dep_addr: Some(0x0134),
                start_addr: None,
                end_addr: None,
                entry_point: Some(0x0820),
                end_addr_ptr: None,
            }
        );

        let mut mem_v30_no_ep = vec![0u8; 0x1_0000];
        write_exo3_at(&mut mem_v30_no_ep, 0x0900, 0xAB, true, None);
        let view_v30_no_ep = LoadedImage::for_c64(&mem_v30_no_ep, 0x0801, 0x0950);
        let m_v30_no_ep = exomizer::SPEC.detect(&view_v30_no_ep).unwrap();
        let info_v30_no_ep = PackerInfo::resolve(&view_v30_no_ep, &m_v30_no_ep);
        assert_eq!(info_v30_no_ep.name, "Exomizer 3.0");
        assert_eq!(info_v30_no_ep.dep_addr, Some(0x01AB));
        assert_eq!(info_v30_no_ep.entry_point, None);

        // 2. Exomizer v3.02+ with and without entry point.
        let mut mem_v302 = vec![0u8; 0x1_0000];
        write_exo3_at(&mut mem_v302, 0x0900, 0x9F, false, Some(0x4800));
        let view_v302 = LoadedImage::for_c64(&mem_v302, 0x0801, 0x0950);
        let m_v302 = exomizer::SPEC
            .detect(&view_v302)
            .expect("Exomizer v3.02+ matches");
        assert_eq!(m_v302.spec_name, "Exomizer v3.02+");
        let info_v302 = PackerInfo::resolve(&view_v302, &m_v302);
        assert_eq!(
            info_v302,
            PackerInfo {
                name: "Exomizer v3.02+",
                dep_addr: Some(0x019F),
                start_addr: None,
                end_addr: None,
                entry_point: Some(0x4800),
                end_addr_ptr: None,
            }
        );

        let mut mem_v302_no_ep = vec![0u8; 0x1_0000];
        write_exo3_at(&mut mem_v302_no_ep, 0x0900, 0x34, false, None);
        let view_v302_no_ep = LoadedImage::for_c64(&mem_v302_no_ep, 0x0801, 0x0950);
        let m_v302_no_ep = exomizer::SPEC.detect(&view_v302_no_ep).unwrap();
        let info_v302_no_ep = PackerInfo::resolve(&view_v302_no_ep, &m_v302_no_ep);
        assert_eq!(info_v302_no_ep.name, "Exomizer v3.02+");
        assert_eq!(info_v302_no_ep.entry_point, None);

        // 3. Exomizer 1.x / 2.x (`Pat::OneOf(&[0x34, 0x50])` at `p + 2`, `start_addr: BasicStart(0)`).
        for dep_low in [0x34_u8, 0x50] {
            let mut mem_v2 = vec![0u8; 0x1_0000];
            write_exo2_at(&mut mem_v2, 0x0900, dep_low);
            let view_v2 = LoadedImage::for_c64(&mem_v2, 0x0801, 0x0950);
            let m_v2 = exomizer::SPEC
                .detect(&view_v2)
                .expect("Exomizer 2.x matches");
            assert_eq!(m_v2.spec_name, "Exomizer 2.x");
            assert_eq!(
                m_v2.resolved.hints.start_addr,
                Some(AddrSource::BasicStart(0))
            );
            let info_v2 = PackerInfo::resolve(&view_v2, &m_v2);
            assert_eq!(
                info_v2,
                PackerInfo {
                    name: "Exomizer 2.x",
                    dep_addr: Some(0x0100 | u16::from(dep_low)),
                    start_addr: Some(0x0801),
                    end_addr: None,
                    entry_point: None,
                    end_addr_ptr: None,
                }
            );
        }

        // Any byte other than `0x34` or `0x50` at `p + 2` in Exomizer 2.x is rejected.
        let mut mem_v2_bad = vec![0u8; 0x1_0000];
        write_exo2_at(&mut mem_v2_bad, 0x0900, 0x35);
        assert!(
            exomizer::SPEC
                .detect(&LoadedImage::for_c64(&mem_v2_bad, 0x0801, 0x0950))
                .is_none()
        );
    }

    /// T3.3.2: `exomizer` scans descending (`Anchor::ScanBackFromEnd`) from `load_end - 8` down
    /// to `load_addr`, selecting the higher-address signature when two signatures of the same
    /// generation are present, while always preferring an Exomizer 3.x match over an Exomizer 2.x
    /// match even when the 2.x signature lies at a higher address.
    #[test]
    fn exomizer_detect_scans_descending() {
        // 1. Two Exomizer 3.x signatures (`$0850` v3.0 and `$0920` v3.02+): higher address `$0920` wins.
        let mut mem_two_v3 = vec![0u8; 0x1_0000];
        write_exo3_at(&mut mem_two_v3, 0x0850, 0x11, true, Some(0x1111));
        write_exo3_at(&mut mem_two_v3, 0x0920, 0x22, false, Some(0x2222));
        let view_two_v3 = LoadedImage::for_c64(&mem_two_v3, 0x0801, 0x0950);
        let m_two_v3 = exomizer::SPEC.detect(&view_two_v3).unwrap();
        assert_eq!(m_two_v3.anchor_addr, 0x0920);
        assert_eq!(m_two_v3.spec_name, "Exomizer v3.02+");
        let info_two_v3 = PackerInfo::resolve(&view_two_v3, &m_two_v3);
        assert_eq!(info_two_v3.dep_addr, Some(0x0122));
        assert_eq!(info_two_v3.entry_point, Some(0x2222));

        // 2. Two Exomizer 2.x signatures (`$0840` with `$34` and `$0910` with `$50`): higher `$0910` wins.
        let mut mem_two_v2 = vec![0u8; 0x1_0000];
        write_exo2_at(&mut mem_two_v2, 0x0840, 0x34);
        write_exo2_at(&mut mem_two_v2, 0x0910, 0x50);
        let view_two_v2 = LoadedImage::for_c64(&mem_two_v2, 0x0801, 0x0950);
        let m_two_v2 = exomizer::SPEC.detect(&view_two_v2).unwrap();
        assert_eq!(m_two_v2.anchor_addr, 0x0910);
        assert_eq!(m_two_v2.spec_name, "Exomizer 2.x");
        let info_two_v2 = PackerInfo::resolve(&view_two_v2, &m_two_v2);
        assert_eq!(info_two_v2.dep_addr, Some(0x0150));

        // 3. Exomizer 3.x at lower address (`$0850`) + Exomizer 2.x at higher address (`$0950`):
        //    Exomizer 3.x still takes precedence over Exomizer 2.x.
        let mut mem_v3_beats_v2 = vec![0u8; 0x1_0000];
        write_exo3_at(&mut mem_v3_beats_v2, 0x0850, 0xAB, true, Some(0x3000));
        write_exo2_at(&mut mem_v3_beats_v2, 0x0950, 0x50);
        let view_v3_beats_v2 = LoadedImage::for_c64(&mem_v3_beats_v2, 0x0801, 0x0980);
        let m_v3_beats_v2 = exomizer::SPEC.detect(&view_v3_beats_v2).unwrap();
        assert_eq!(m_v3_beats_v2.anchor_addr, 0x0850);
        assert_eq!(m_v3_beats_v2.spec_name, "Exomizer 3.0");
        let info_v3_beats_v2 = PackerInfo::resolve(&view_v3_beats_v2, &m_v3_beats_v2);
        assert_eq!(info_v3_beats_v2.dep_addr, Some(0x01AB));
        assert_eq!(info_v3_beats_v2.entry_point, Some(0x3000));
    }

    /// Helper writing an Exomizer 3 decruncher header + core sequence into a snapshot at `p`.
    fn write_exo3_snapshot_at(
        snapshot: &mut [u8],
        p: usize,
        end_lo: u8,
        end_hi: u8,
        with_txa_8a: bool,
        is_exo_30: bool,
    ) {
        snapshot[p - 6] = 0x4C;
        snapshot[p - 5] = 0x34;
        snapshot[p - 4] = 0x01;
        snapshot[p - 1] = end_hi;
        let p_idx = p - 5;
        let q = if with_txa_8a {
            snapshot[p_idx - 2] = 0x8A;
            3
        } else {
            snapshot[p_idx - 2] = 0x00;
            2
        };
        snapshot[p_idx - q] = end_lo;
        if is_exo_30 {
            snapshot[p_idx - q - 1] = 0xAA;
            snapshot[p_idx - q - 3] = 0xAA;
            snapshot[p_idx - q - 2] = end_lo;
        } else {
            snapshot[p_idx - q - 1] = 0xAA;
            snapshot[p_idx - q - 3] = 0xBB;
            snapshot[p_idx - q - 2] = end_lo.wrapping_add(1);
        }
        snapshot[p..p + 8].copy_from_slice(&[0x69, 0x80, 0x0A, 0x10, 0x0F, 0x06, 0xFD, 0xD0]);
    }

    /// T3.3.3: `ExomizerHooks::refine` rescans `ctx.snapshot` **ascending** across `$0200..=$FFF0`
    /// (stopping at the lowest-address match, ignoring matches below `$0200`), handles the optional
    /// `0x8A` (`TXA`) byte, distinguishes `ver == 0x30` vs `ver == 0x32` end adjustments, maps
    /// `dyn_end == 0` to `$FF00` (`end_addr = $FEFF`), and is a no-op on non-C64 systems.
    #[test]
    fn exomizer_refine_rescans_snapshot_ascending_from_0200() {
        let hooks = exomizer::ExomizerHooks::new(3, 0x30, 0x34);
        let mut mem = vec![0u8; 0x1_0000];
        let written = vec![false; 0x1_0000];
        // Set mem[$FE..=$FF] = $0200 (< $0400 so dyn_start does not alter start_addr).
        mem[0xFE] = 0x00;
        mem[0xFF] = 0x02;

        // 1. Two snapshot signatures at `$0300` (end = $4000, v3.0) and `$0900` (end = $8000, v3.0):
        //    ascending scan from `$0200` picks `$0300` first (`end_addr = $3FFF`).
        //    Also place a signature at `$01FF` (below `$0200`), which must be ignored.
        let mut snapshot = vec![0u8; 0x1_0000];
        write_exo3_snapshot_at(&mut snapshot, 0x01FF, 0x00, 0x20, false, true);
        write_exo3_snapshot_at(&mut snapshot, 0x0300, 0x00, 0x40, false, true);
        write_exo3_snapshot_at(&mut snapshot, 0x0900, 0x00, 0x80, false, true);

        let ctx = RefineContext {
            mem: &mem,
            snapshot: &snapshot,
            written: &written,
            system: &TargetSystem::C64,
            registers: Registers::default(),
        };
        let mut bounds = Bounds {
            start_addr: 0x0801,
            end_addr: 0x9FFF,
            entry_point: 0x0810,
        };
        hooks.refine(&ctx, &mut bounds);
        assert_eq!(bounds.end_addr, 0x3FFF);

        // 2. With `0x8A` (`TXA` -> `q = 3`) and `is_exo_30 = false` (`ver == 0x32`),
        //    `dyn_end = ($5000).wrapping_add(1) = $5001` -> `bounds.end_addr = $5000`.
        let mut snapshot_v32 = vec![0u8; 0x1_0000];
        write_exo3_snapshot_at(&mut snapshot_v32, 0x0400, 0x00, 0x50, true, false);
        let ctx_v32 = RefineContext {
            mem: &mem,
            snapshot: &snapshot_v32,
            written: &written,
            system: &TargetSystem::C64,
            registers: Registers::default(),
        };
        let mut bounds_v32 = Bounds {
            start_addr: 0x0801,
            end_addr: 0x9FFF,
            entry_point: 0x0810,
        };
        hooks.refine(&ctx_v32, &mut bounds_v32);
        assert_eq!(bounds_v32.end_addr, 0x5000);

        // 3. When `dyn_end == 0` (`end_lo == 0x00, end_hi == 0x00` on v3.0), `dyn_end` becomes `$FF00`
        //    and `bounds.end_addr = $FEFF`.
        let mut snapshot_zero = vec![0u8; 0x1_0000];
        write_exo3_snapshot_at(&mut snapshot_zero, 0x0200, 0x00, 0x00, false, true);
        let ctx_zero = RefineContext {
            mem: &mem,
            snapshot: &snapshot_zero,
            written: &written,
            system: &TargetSystem::C64,
            registers: Registers::default(),
        };
        let mut bounds_zero = Bounds {
            start_addr: 0x0801,
            end_addr: 0x9FFF,
            entry_point: 0x0810,
        };
        hooks.refine(&ctx_zero, &mut bounds_zero);
        assert_eq!(bounds_zero.end_addr, 0xFEFF);

        // 4. Non-C64 system guard leaves `bounds` untouched.
        let ctx_c128 = RefineContext {
            mem: &mem,
            snapshot: &snapshot,
            written: &written,
            system: &TargetSystem::C128,
            registers: Registers::default(),
        };
        let mut bounds_c128 = Bounds {
            start_addr: 0x0801,
            end_addr: 0x9FFF,
            entry_point: 0x0810,
        };
        hooks.refine(&ctx_c128, &mut bounds_c128);
        assert_eq!(bounds_c128.end_addr, 0x9FFF);
    }

    /// T3.3.4: `ExomizerHooks::refine` reads `ctx.registers.y` from [`RefineContext`] and adds it
    /// to `mem[$FE..=$FF]` (`+ 0` for `ver == 0x30`, `+ 1` for `ver == 0x32`) to compute `dyn_start`,
    /// clamping `bounds.start_addr` downward when `dyn_start >= $0400` (in addition to `self.min_start`).
    #[test]
    fn exomizer_refine_uses_y_from_refine_context() {
        let mut mem = vec![0u8; 0x1_0000];
        let written = vec![false; 0x1_0000];
        // Set mem[$FE..=$FF] = $07F0.
        mem[0xFE] = 0xF0;
        mem[0xFF] = 0x07;

        let mut snapshot_v30 = vec![0u8; 0x1_0000];
        write_exo3_snapshot_at(&mut snapshot_v30, 0x0850, 0x00, 0x40, false, true);

        let mut snapshot_v32 = vec![0u8; 0x1_0000];
        write_exo3_snapshot_at(&mut snapshot_v32, 0x0850, 0x00, 0x40, false, false);

        let hooks = exomizer::ExomizerHooks::new(3, 0x30, 0x34);

        // 1. With `Y = 0x10` and `ver == 0x30`: `dyn_start = $07F0 + $10 = $0800`.
        //    `bounds.start_addr` lowers from `$0801` to `$0800`.
        let ctx_y10_v30 = RefineContext {
            mem: &mem,
            snapshot: &snapshot_v30,
            written: &written,
            system: &TargetSystem::C64,
            registers: Registers {
                y: 0x10,
                ..Registers::default()
            },
        };
        let mut bounds_v30 = Bounds {
            start_addr: 0x0801,
            end_addr: 0x9FFF,
            entry_point: 0x0810,
        };
        hooks.refine(&ctx_y10_v30, &mut bounds_v30);
        assert_eq!(bounds_v30.start_addr, 0x0800);

        // 2. With `Y = 0x10` and `ver == 0x32`: `dyn_start = $07F0 + $10 + 1 = $0801`.
        let ctx_y10_v32 = RefineContext {
            mem: &mem,
            snapshot: &snapshot_v32,
            written: &written,
            system: &TargetSystem::C64,
            registers: Registers {
                y: 0x10,
                ..Registers::default()
            },
        };
        let mut bounds_v32 = Bounds {
            start_addr: 0x0900,
            end_addr: 0x9FFF,
            entry_point: 0x0810,
        };
        hooks.refine(&ctx_y10_v32, &mut bounds_v32);
        assert_eq!(bounds_v32.start_addr, 0x0801);

        // 3. When `dyn_start < $0400` (e.g. `mem[$FE..=$FF] = $03F0, Y = 0x0E` -> `$03FE`),
        //    `dyn_start` is ignored, whereas `self.min_start` (`Some($07C0)`) still lowers `start_addr`.
        mem[0xFE] = 0xF0;
        mem[0xFF] = 0x03;
        let mut hooks_with_min = exomizer::ExomizerHooks::new(3, 0x30, 0x34);
        hooks_with_min.min_start = Some(0x07C0);
        let ctx_below_0400 = RefineContext {
            mem: &mem,
            snapshot: &snapshot_v30,
            written: &written,
            system: &TargetSystem::C64,
            registers: Registers {
                y: 0x0E,
                ..Registers::default()
            },
        };
        let mut bounds_min = Bounds {
            start_addr: 0x0801,
            end_addr: 0x9FFF,
            entry_point: 0x0810,
        };
        hooks_with_min.refine(&ctx_below_0400, &mut bounds_min);
        assert_eq!(bounds_min.start_addr, 0x07C0);
    }

    /// T3.3.5: `ExomizerHooks::on_step` reads instructions and zero-page pointers exclusively via
    /// [`StepContext::peek`] (Ruling B2), updating `min_start` for `$B1` / `$91` with `dep_zp` or
    /// `$FE` when the resolved target is `>= $0200`, ignoring `< $0200`, and never advancing
    /// `bus.total_cycles` or CIA1/CIA2 timers even when `pc` is in I/O or ROM address space.
    #[test]
    fn exomizer_on_step_uses_peek_only() {
        let mut bus = UnpackerMemory::new(TargetSystem::C64, None, None, None);
        // Place `$91 $34` (`STA ($34),Y`) in raw RAM at I/O address `$DC04` (CIA1 Timer A Lo),
        // and set ZP pointer at `$34..=$35` to `$07F0`.
        bus.mem[0xDC04] = 0x91;
        bus.mem[0xDC05] = 0x34;
        bus.mem[0x0034] = 0xF0;
        bus.mem[0x0035] = 0x07;

        let cycles_before = bus.total_cycles;
        let cia1_before = format!("{:?}", bus.cia1);
        let cia2_before = format!("{:?}", bus.cia2);

        let mut hooks_v30 = exomizer::ExomizerHooks::new(3, 0x30, 0x34);

        // 1. In `Phase::FindDepacker`, `on_step` is a no-op.
        {
            let mut ctx = StepContext::new(
                &mut bus,
                0xDC04,
                Phase::FindDepacker,
                Registers {
                    y: 0x10,
                    ..Registers::default()
                },
            );
            hooks_v30.on_step(&mut ctx);
        }
        assert_eq!(hooks_v30.min_start, None);

        // 2. For `version == 2` in `Phase::Decompress`, `on_step` is also a no-op.
        let mut hooks_v2 = exomizer::ExomizerHooks::new(2, 0, 0x34);
        {
            let mut ctx = StepContext::new(
                &mut bus,
                0xDC04,
                Phase::Decompress,
                Registers {
                    y: 0x10,
                    ..Registers::default()
                },
            );
            hooks_v2.on_step(&mut ctx);
        }
        assert_eq!(hooks_v2.min_start, None);

        // 3. In `Phase::Decompress` (`version == 3`, `exo_ver == 0x30`), `on_step` peeks raw RAM at
        //    `$DC04..=$DC05` and `$0034..=$0035`, computing `$07F0 + $10 = $0800`.
        {
            let mut ctx = StepContext::new(
                &mut bus,
                0xDC04,
                Phase::Decompress,
                Registers {
                    y: 0x10,
                    ..Registers::default()
                },
            );
            hooks_v30.on_step(&mut ctx);
        }
        assert_eq!(hooks_v30.min_start, Some(0x0800));
        assert_eq!(bus.total_cycles, cycles_before);
        assert_eq!(format!("{:?}", bus.cia1), cia1_before);
        assert_eq!(format!("{:?}", bus.cia2), cia2_before);

        // 4. With `$B1 $FE` (`LDA ($FE),Y`) on `exo_ver == 0x32` and `($FE) = $03FE, Y = $01`:
        //    `val = $03FE + $01 + 1 = $0400`, lowering `min_start` from `$0800` to `$0400`.
        bus.mem[0x0150] = 0xB1;
        bus.mem[0x0151] = 0xFE;
        bus.mem[0x00FE] = 0xFE;
        bus.mem[0x00FF] = 0x03;
        hooks_v30.exo_ver = 0x32;
        {
            let mut ctx = StepContext::new(
                &mut bus,
                0x0150,
                Phase::Decompress,
                Registers {
                    y: 0x01,
                    ..Registers::default()
                },
            );
            hooks_v30.on_step(&mut ctx);
        }
        assert_eq!(hooks_v30.min_start, Some(0x0400));

        // 5. Writes to Zero Page or Stack Page (`val < $0200`, e.g. `$01FF`) are ignored.
        bus.mem[0x00FE] = 0xFD;
        bus.mem[0x00FF] = 0x01;
        {
            let mut ctx = StepContext::new(
                &mut bus,
                0x0150,
                Phase::Decompress,
                Registers {
                    y: 0x01,
                    ..Registers::default()
                },
            );
            hooks_v30.on_step(&mut ctx);
        }
        assert_eq!(hooks_v30.min_start, Some(0x0400));
        assert_eq!(bus.total_cycles, cycles_before);
    }

    /// T3.3.6: `tscrunch` extracts `reloc_start` from `q + 0x10` (`$081D..=$081E`) for Variant 1
    /// (`TSCrunch v1.3+`) and from `q + 0x18` (`$0825..=$0826`) for Variant 2 (`TSCrunch v1.3+-X2`),
    /// accepting only addresses `> $0800`, and resolves optional `entry_point` from the secondary
    /// `A9 37 85 01 58 4C` scan when `mem.len() >= q + 0x70`.
    #[test]
    fn tscrunch_reloc_start_from_operand() {
        for name in ["TSCrunch v1.3+", "TSCrunch v1.3+-X2"] {
            let entry = find_entry_by_name(name)
                .unwrap_or_else(|| panic!("missing registry entry for `{name}`"));
            assert_eq!(entry.id, "tscrunch");
        }

        // 1. Variant 1 (`TSCrunch v1.3+`): `reloc_start` at `q + 0x10` (`$081D..=$081E`).
        let (mut mem_v1, load_addr, load_end) = tscrunch::SPEC.build_fixture_memory();
        let view_v1 = LoadedImage::for_c64(&mem_v1, load_addr, load_end);
        let m_v1 = tscrunch::SPEC
            .detect(&view_v1)
            .expect("TSCrunch v1.3+ matches");
        assert_eq!(m_v1.spec_name, "TSCrunch v1.3+");
        assert_eq!(tscrunch::resolve_reloc_start(&view_v1, &m_v1), Some(0xC000));

        // Values `<= 0x0800` (`$0000`, `$0800`) yield `reloc_start == None`; `$0801` yields `Some(0x0801)`.
        for (raw, expected) in [(0x0000_u16, None), (0x0800, None), (0x0801, Some(0x0801))] {
            mem_v1[0x080D + 0x10..0x080D + 0x12].copy_from_slice(&raw.to_le_bytes());
            let view = LoadedImage::for_c64(&mem_v1, load_addr, load_end);
            let m = tscrunch::SPEC.detect(&view).unwrap();
            assert_eq!(
                tscrunch::resolve_reloc_start(&view, &m),
                expected,
                "v1 raw=${raw:04X}"
            );
        }

        // Secondary entry_point scan in `q + 14..q + 0x80`:
        mem_v1[0x080D + 20..0x080D + 28]
            .copy_from_slice(&[0xA9, 0x37, 0x85, 0x01, 0x58, 0x4C, 0x00, 0x2E]);
        let view_v1_ep = LoadedImage::for_c64(&mem_v1, load_addr, 0x0900);
        let m_v1_ep = tscrunch::SPEC.detect(&view_v1_ep).unwrap();
        let info_v1_ep = PackerInfo::resolve(&view_v1_ep, &m_v1_ep);
        assert_eq!(
            info_v1_ep,
            PackerInfo {
                name: "TSCrunch v1.3+",
                dep_addr: Some(0x0002),
                start_addr: Some(0x0800),
                end_addr: None,
                entry_point: Some(0x2E00),
                end_addr_ptr: None,
            }
        );

        // 2. Variant 2 (`TSCrunch v1.3+-X2`): `reloc_start` at `q + 0x18` (`$0825..=$0826`).
        let mut mem_v2 = vec![0u8; 0x1_0000];
        let q = 0x080D_usize;
        mem_v2[q..q + 19].copy_from_slice(&[
            0x78, 0xA9, 0x34, 0x85, 0x01, 0xA2, 0xD0, 0xBD, 0x1F, 0x08, 0x9D, 0xFB, 0x00, 0xCA,
            0xD0, 0xF7, 0x4C, 0x00, 0x01,
        ]);
        mem_v2[q + 0x18..q + 0x1A].copy_from_slice(&0xD800_u16.to_le_bytes());
        mem_v2[q + 28..q + 36].copy_from_slice(&[0xA9, 0x37, 0x85, 0x01, 0x58, 0x4C, 0x00, 0x11]);

        let view_v2 = LoadedImage::for_c64(&mem_v2, 0x0801, 0x0900);
        let m_v2 = tscrunch::SPEC
            .detect(&view_v2)
            .expect("TSCrunch v1.3+-X2 matches");
        assert_eq!(m_v2.spec_name, "TSCrunch v1.3+-X2");
        assert_eq!(tscrunch::resolve_reloc_start(&view_v2, &m_v2), Some(0xD800));
        let info_v2 = PackerInfo::resolve(&view_v2, &m_v2);
        assert_eq!(
            info_v2,
            PackerInfo {
                name: "TSCrunch v1.3+-X2",
                dep_addr: Some(0x0100),
                start_addr: Some(0x0800),
                end_addr: None,
                entry_point: Some(0x1100),
                end_addr_ptr: None,
            }
        );

        for (raw, expected) in [(0x0000_u16, None), (0x0800, None), (0x0801, Some(0x0801))] {
            mem_v2[q + 0x18..q + 0x1A].copy_from_slice(&raw.to_le_bytes());
            let view = LoadedImage::for_c64(&mem_v2, 0x0801, 0x0900);
            let m = tscrunch::SPEC.detect(&view).unwrap();
            assert_eq!(
                tscrunch::resolve_reloc_start(&view, &m),
                expected,
                "v2 raw=${raw:04X}"
            );
        }
    }

    /// T3.3.7: `TScrunchHooks::on_step` clears `bus.written[reloc..=0xFFFF]` once when `pc` enters
    /// `$0021..=$0090` or `$0121..=$0190` in `Phase::Decompress`, leaving `bus.written_phase2`
    /// completely untouched (Ruling B2), and `TScrunchHooks::refine` trims `bounds.end_addr` to
    /// the highest remaining `written` byte in `start_addr..=end_addr`.
    #[test]
    fn tscrunch_on_step_forgets_reloc_to_ffff_written_only() {
        let mut bus = UnpackerMemory::new(TargetSystem::C64, None, None, None);
        bus.written.fill(true);
        bus.written_phase2.fill(true);

        let mut hooks = tscrunch::TScrunchHooks::new(Some(0xC000));

        // 1. In `Phase::FindDepacker`, or in `Phase::Decompress` outside `$0021..=$0090` / `$0121..=$0190`,
        //    `on_step` does not clear `written`.
        {
            let mut ctx =
                StepContext::new(&mut bus, 0x0021, Phase::FindDepacker, Registers::default());
            hooks.on_step(&mut ctx);
        }
        assert!(!hooks.reloc_cleared);
        assert!(bus.written[0xC000]);

        {
            let mut ctx =
                StepContext::new(&mut bus, 0x0020, Phase::Decompress, Registers::default());
            hooks.on_step(&mut ctx);
        }
        assert!(!hooks.reloc_cleared);
        assert!(bus.written[0xC000]);

        // 2. Entering `$0021..=$0090` in `Phase::Decompress` clears `written[0xC000..=0xFFFF]`
        //    while leaving `written[..0xC000]` and ALL of `written_phase2` untouched.
        {
            let mut ctx =
                StepContext::new(&mut bus, 0x0021, Phase::Decompress, Registers::default());
            hooks.on_step(&mut ctx);
        }
        assert!(hooks.reloc_cleared);
        assert!(bus.written[0xBFFF]);
        assert!(!bus.written[0xC000]);
        assert!(!bus.written[0xFFFF]);
        assert!(bus.written_phase2[0xC000]);
        assert!(bus.written_phase2[0xFFFF]);

        // 3. Subsequent writes during decompression (`0xC000..=0xC100`) are NOT cleared on later steps
        //    because `reloc_cleared` is already `true`.
        bus.written[0xB000..0xC000].fill(false);
        bus.written[0x9D19] = true;
        bus.written[0x9D1A..0xC000].fill(false);
        bus.written[0xC000] = true;
        {
            let mut ctx =
                StepContext::new(&mut bus, 0x0121, Phase::Decompress, Registers::default());
            hooks.on_step(&mut ctx);
        }
        assert!(bus.written[0xC000]);

        // 4. Clear `$C000` and verify `refine` trims `bounds.end_addr` from `$FFFF` down to `$9D19`.
        bus.written[0xC000] = false;
        let refine_ctx = RefineContext {
            mem: &bus.mem,
            snapshot: &bus.mem,
            written: &bus.written,
            system: &TargetSystem::C64,
            registers: Registers::default(),
        };
        let mut bounds = Bounds {
            start_addr: 0x0800,
            end_addr: 0xFFFF,
            entry_point: 0x1100,
        };
        hooks.refine(&refine_ctx, &mut bounds);
        assert_eq!(bounds.end_addr, 0x9D19);
    }

    /// T3.3.8: `tscrunch::SPEC` and both resolved variants (`TSCrunch v1.3+` and `TSCrunch v1.3+-X2`)
    /// declare `start_addr: Some(AddrSource::BasicStart(-1))` (Ruling B4), which resolves to `$0800`
    /// on C64.
    #[test]
    fn tscrunch_start_is_basic_start_minus_one() {
        assert_eq!(
            tscrunch::SPEC.hints.start_addr,
            Some(AddrSource::BasicStart(-1))
        );

        // Variant 1 (`TSCrunch v1.3+`)
        let (mem_v1, load_addr, load_end) = tscrunch::SPEC.build_fixture_memory();
        let view_v1 = LoadedImage::for_c64(&mem_v1, load_addr, load_end);
        let m_v1 = tscrunch::SPEC.detect(&view_v1).unwrap();
        assert_eq!(
            m_v1.resolved.hints.start_addr,
            Some(AddrSource::BasicStart(-1))
        );
        assert_eq!(
            m_v1.resolved
                .hints
                .start_addr
                .unwrap()
                .resolve_static(&view_v1, m_v1.anchor_addr),
            Some(0x0800)
        );

        // Variant 2 (`TSCrunch v1.3+-X2`)
        let mut mem_v2 = vec![0u8; 0x1_0000];
        mem_v2[0x080D..0x080D + 19].copy_from_slice(&[
            0x78, 0xA9, 0x34, 0x85, 0x01, 0xA2, 0xD0, 0xBD, 0x1F, 0x08, 0x9D, 0xFB, 0x00, 0xCA,
            0xD0, 0xF7, 0x4C, 0x00, 0x01,
        ]);
        let view_v2 = LoadedImage::for_c64(&mem_v2, 0x0801, 0x0830);
        let m_v2 = tscrunch::SPEC.detect(&view_v2).unwrap();
        assert_eq!(m_v2.spec_name, "TSCrunch v1.3+-X2");
        assert_eq!(
            m_v2.resolved.hints.start_addr,
            Some(AddrSource::BasicStart(-1))
        );
        assert_eq!(
            m_v2.resolved
                .hints
                .start_addr
                .unwrap()
                .resolve_static(&view_v2, m_v2.anchor_addr),
            Some(0x0800)
        );
    }

    /// T3.3.12: Re-verifies Invariant 2 (`T2.3.3` and `T2.3.4`) end-to-end through `PackerSpec`
    /// and real `PackerHooks` (`TScrunchHooks` and `ExomizerHooks`):
    /// 1. In `Phase::FindDepacker` (`T2.3.3`), `pc` is read *before* `on_step`, so `on_step` does
    ///    not fire with `Phase::Decompress` at `dep_addr` during Phase 1, and Phase 1 transitions
    ///    cleanly to `Phase::Decompress` at `dep_addr`.
    /// 2. In `Phase::Decompress` (`T2.3.4`), `on_step` runs *before* `pc` is checked against
    ///    `written`: when `TScrunchHooks` clears `written[reloc..=0xFFFF]` upon entering `$0121`,
    ///    a subsequent jump into the relocated high-memory stub at `reloc` is not mistaken for
    ///    `WrittenCodeReached`, allowing execution to continue to the unpacked payload entry.
    #[test]
    fn hook_order_invariant_with_real_hooks() {
        // Construct a synthetic TSCrunch v1.3+-X2 program loaded at $0801:
        // - $0801..=$080C: BASIC `10 SYS 2061` (`$080D`)
        // - $080D..=$081F: 19-byte TSCrunch v1.3+-X2 signature (`JMP $0100` at $081D..=$081F)
        //   Modified so that the copy loop (`LDA $0900,X; STA $00FB,X; DEX; BNE`) copies a
        //   dep_addr stub into `$0100..=$0126` and also writes to `$C000` (simulating Phase 1
        //   stream relocation at `reloc_start = $C000`).
        let mut prg = vec![0u8; 0x0200];
        // BASIC line `10 SYS 2061` at $0801..=$080C:
        prg[0..12].copy_from_slice(&[
            0x0B, 0x08, 0x0A, 0x00, 0x9E, b'2', b'0', b'6', b'1', 0x00, 0x00, 0x00,
        ]);
        // $080D..=$081F: V2_SIGNATURE with LDX #$2B, LDA $08FF,X, STA $00FF,X, DEX, BNE -7, JMP $0100
        // (copies 43 bytes from $0900..=$092A into $0100..=$012A).
        prg[12..31].copy_from_slice(&[
            0x78, 0xA9, 0x34, 0x85, 0x01, 0xA2, 0x2B, 0xBD, 0xFF, 0x08, 0x9D, 0xFF, 0x00, 0xCA,
            0xD0, 0xF7, 0x4C, 0x00, 0x01,
        ]);
        // At `q + 0x18` ($0825..=$0826, offset 36..38 from $0801): `reloc_start = $C000`.
        prg[36..38].copy_from_slice(&0xC000_u16.to_le_bytes());

        // Template at $0900..=$092A (offset 0x00FF..=0x0129 from $0801) copied to $0100..=$012A:
        // - $0100 (Phase 2 entry):
        //     LDA #$EA      ; $A9 $EA
        //     STA $C000     ; $8D $00 $C0  (marks $C000 written before entering $0121!)
        //     LDA #$4C      ; $A9 $4C
        //     STA $C001     ; $8D $01 $C0  (JMP $2000 at $C001..=$C003)
        //     LDA #$00      ; $A9 $00
        //     STA $C002     ; $8D $02 $C0
        //     LDA #$20      ; $A9 $20
        //     STA $C003     ; $8D $03 $C0
        //     JMP $0121     ; $4C $21 $01
        // - $0121 (triggers TScrunchHooks::on_step -> forget_writes($C000..=$FFFF)):
        //     LDA #$60      ; $A9 $60 (RTS)
        //     STA $2000     ; $8D $00 $20 (writes unpacked payload at $2000)
        //     JMP $C000     ; $4C $00 $C0 (jumps through relocated stream at $C000 -> $2000!)
        let stub_at_0100: [u8; 23] = [
            0xA9, 0xEA, 0x8D, 0x00, 0xC0, 0xA9, 0x4C, 0x8D, 0x01, 0xC0, 0xA9, 0x00, 0x8D, 0x02,
            0xC0, 0xA9, 0x20, 0x8D, 0x03, 0xC0, 0x4C, 0x21, 0x01,
        ];
        let stub_at_0121: [u8; 8] = [0xA9, 0x60, 0x8D, 0x00, 0x20, 0x4C, 0x00, 0xC0];
        prg[0x00FF..0x00FF + stub_at_0100.len()].copy_from_slice(&stub_at_0100);
        prg[0x0120..0x0120 + stub_at_0121.len()].copy_from_slice(&stub_at_0121);

        let res = unpack(&prg, 0x0801, &UnpackConfig::default(), None)
            .expect("synthetic TSCrunch program unpacks cleanly");
        assert_eq!(res.packer_name.as_deref(), Some("TSCrunch v1.3+-X2"));
        assert_eq!(res.dep_addr, 0x0100);
        // Because `TScrunchHooks::on_step` ran at `$0121` and cleared `written[$C000..=$FFFF]`
        // before `$C000` was executed, execution did NOT stop at `$C000`; it continued through
        // `$C000` (`NOP; JMP $2000`) and exited at `$2000` (`WrittenCodeReached`), and `refine`
        // trimmed `end_addr` to `$2000` (excluding `$C000..=$C003`).
        assert_eq!(res.entry_point, 0x2000);
        assert_eq!(res.start_addr, 0x0800);
        assert_eq!(res.end_addr, 0x2000);
    }

    /// T3.3.13: Unpacking all 24 Exomizer and TSCrunch corpus files (17 `Exomizer 3.0`,
    /// 3 `Exomizer 2.x`, 2 `TSCrunch v1.3+`, and 2 `TSCrunch v1.3+-X2`) produces the exact
    /// baseline outputs and preserves ground-truth/consensus agreement on the `moving_tubes`
    /// and `mule` groups.
    #[test]
    fn exomizer_and_tscrunch_corpus_unchanged() {
        #[derive(Clone, Copy)]
        struct ExpectedCorpusUnpack {
            file: &'static str,
            packer_name: &'static str,
            start_addr: u16,
            end_addr: u16,
            entry_point: u16,
            dep_addr: u16,
            instructions_executed: u64,
            len: usize,
        }

        const CASES: [ExpectedCorpusUnpack; 24] = [
            ExpectedCorpusUnpack {
                file: "c64_Bit_by_Bits-BZ!.exo3.prg",
                packer_name: "Exomizer 3.0",
                start_addr: 0x0800,
                end_addr: 0xEF83,
                entry_point: 0x083A,
                dep_addr: 0x019F,
                instructions_executed: 1_595_699,
                len: 59_268,
            },
            ExpectedCorpusUnpack {
                file: "c64_HBFS.exo3.prg",
                packer_name: "Exomizer 3.0",
                start_addr: 0x0801,
                end_addr: 0xFEFF,
                entry_point: 0xEFB0,
                dep_addr: 0x01AB,
                instructions_executed: 126_699,
                len: 63_231,
            },
            ExpectedCorpusUnpack {
                file: "c64_Layers.exo3.prg",
                packer_name: "Exomizer 3.0",
                start_addr: 0x0801,
                end_addr: 0xFBF1,
                entry_point: 0x0834,
                dep_addr: 0x01C4,
                instructions_executed: 878_118,
                len: 62_449,
            },
            ExpectedCorpusUnpack {
                file: "c64_boilerplate.exo3.prg",
                packer_name: "Exomizer 3.0",
                start_addr: 0x0800,
                end_addr: 0xFEA4,
                entry_point: 0x1000,
                dep_addr: 0x01AB,
                instructions_executed: 1_763_879,
                len: 63_141,
            },
            ExpectedCorpusUnpack {
                file: "c64_cubicdream.exo3.prg",
                packer_name: "Exomizer 3.0",
                start_addr: 0x0800,
                end_addr: 0xEF2A,
                entry_point: 0x080D,
                dep_addr: 0x01B2,
                instructions_executed: 1_481_907,
                len: 59_179,
            },
            ExpectedCorpusUnpack {
                file: "c64_daah_those_acid_pills.exo2.prg",
                packer_name: "Exomizer 2.x",
                start_addr: 0x0801,
                end_addr: 0xFF91,
                entry_point: 0x0810,
                dep_addr: 0x0134,
                instructions_executed: 1_621_541,
                len: 63_377,
            },
            ExpectedCorpusUnpack {
                file: "c64_druid_too.exo3.prg",
                packer_name: "Exomizer 3.0",
                start_addr: 0x0800,
                end_addr: 0xCE1F,
                entry_point: 0x4800,
                dep_addr: 0x019F,
                instructions_executed: 1_237_511,
                len: 50_720,
            },
            ExpectedCorpusUnpack {
                file: "c64_endoskull.exo3.prg",
                packer_name: "Exomizer 3.0",
                start_addr: 0x0800,
                end_addr: 0xFF29,
                entry_point: 0x0810,
                dep_addr: 0x0198,
                instructions_executed: 1_713_944,
                len: 63_274,
            },
            ExpectedCorpusUnpack {
                file: "c64_f600.exo.prg",
                packer_name: "Exomizer 2.x",
                start_addr: 0x0801,
                end_addr: 0xFEFF,
                entry_point: 0x0810,
                dep_addr: 0x0134,
                instructions_executed: 1_485_266,
                len: 63_231,
            },
            ExpectedCorpusUnpack {
                file: "c64_hw20131031.exo.prg",
                packer_name: "Exomizer 2.x",
                start_addr: 0x0801,
                end_addr: 0xF481,
                entry_point: 0x3000,
                dep_addr: 0x0134,
                instructions_executed: 1_570_376,
                len: 60_545,
            },
            ExpectedCorpusUnpack {
                file: "c64_leftovers-pl.exo3.prg",
                packer_name: "Exomizer 3.0",
                start_addr: 0x0800,
                end_addr: 0xF3F0,
                entry_point: 0x080E,
                dep_addr: 0x019C,
                instructions_executed: 1_122_817,
                len: 60_401,
            },
            ExpectedCorpusUnpack {
                file: "c64_lft-nine.exo3.prg",
                packer_name: "Exomizer 3.0",
                start_addr: 0x0800,
                end_addr: 0x7CBC,
                entry_point: 0x080D,
                dep_addr: 0x0198,
                instructions_executed: 743_332,
                len: 29_885,
            },
            ExpectedCorpusUnpack {
                file: "c64_lft-rodents-in-the-attic.exo3.prg",
                packer_name: "Exomizer 3.0",
                start_addr: 0x0800,
                end_addr: 0xC56B,
                entry_point: 0x080D,
                dep_addr: 0x01A1,
                instructions_executed: 1_232_669,
                len: 48_492,
            },
            ExpectedCorpusUnpack {
                file: "c64_little_things.exo3.prg",
                packer_name: "Exomizer 3.0",
                start_addr: 0x0800,
                end_addr: 0x98FF,
                entry_point: 0x080D,
                dep_addr: 0x01AB,
                instructions_executed: 958_054,
                len: 37_120,
            },
            ExpectedCorpusUnpack {
                file: "c64_moving_tubes_lxt.exo3.prg",
                packer_name: "Exomizer 3.0",
                start_addr: 0x0800,
                end_addr: 0x31FF,
                entry_point: 0x2E00,
                dep_addr: 0x01AE,
                instructions_executed: 362_335,
                len: 10_752,
            },
            ExpectedCorpusUnpack {
                file: "c64_moving_tubes_lxt.tscrunch_x.prg",
                packer_name: "TSCrunch v1.3+",
                start_addr: 0x0800,
                end_addr: 0x31FF,
                entry_point: 0x2E00,
                dep_addr: 0x0002,
                instructions_executed: 115_433,
                len: 10_752,
            },
            ExpectedCorpusUnpack {
                file: "c64_moving_tubes_lxt.tscrunch_x2.prg",
                packer_name: "TSCrunch v1.3+-X2",
                start_addr: 0x0800,
                end_addr: 0x31FF,
                entry_point: 0x2E00,
                dep_addr: 0x0100,
                instructions_executed: 116_479,
                len: 10_752,
            },
            ExpectedCorpusUnpack {
                file: "c64_mule.exo3.prg",
                packer_name: "Exomizer 3.0",
                start_addr: 0x0800,
                end_addr: 0x9D19,
                entry_point: 0x1100,
                dep_addr: 0x01AE,
                instructions_executed: 1_123_902,
                len: 38_170,
            },
            ExpectedCorpusUnpack {
                file: "c64_mule.tscrunch_x.prg",
                packer_name: "TSCrunch v1.3+",
                start_addr: 0x0800,
                end_addr: 0x9D19,
                entry_point: 0x1100,
                dep_addr: 0x0002,
                instructions_executed: 365_537,
                len: 38_170,
            },
            ExpectedCorpusUnpack {
                file: "c64_mule.tscrunch_x2.prg",
                packer_name: "TSCrunch v1.3+-X2",
                start_addr: 0x0800,
                end_addr: 0x9D19,
                entry_point: 0x1100,
                dep_addr: 0x0100,
                instructions_executed: 365_555,
                len: 38_170,
            },
            ExpectedCorpusUnpack {
                file: "c64_robot - not human.exo3.prg",
                packer_name: "Exomizer 3.0",
                start_addr: 0x0800,
                end_addr: 0xCBE6,
                entry_point: 0x0810,
                dep_addr: 0x01AB,
                instructions_executed: 1_223_450,
                len: 50_151,
            },
            ExpectedCorpusUnpack {
                file: "c64_roma.exe.exo3.prg",
                packer_name: "Exomizer 3.0",
                start_addr: 0x0800,
                end_addr: 0xC8C5,
                entry_point: 0x0820,
                dep_addr: 0x01B2,
                instructions_executed: 1_157_181,
                len: 49_350,
            },
            ExpectedCorpusUnpack {
                file: "c64_spectro.exo3.prg",
                packer_name: "Exomizer 3.0",
                start_addr: 0x0801,
                end_addr: 0xE7FF,
                entry_point: 0x08A1,
                dep_addr: 0x01C4,
                instructions_executed: 1_386_569,
                len: 57_343,
            },
            ExpectedCorpusUnpack {
                file: "c64_sprite_runners.exo3.prg",
                packer_name: "Exomizer 3.0",
                start_addr: 0x0800,
                end_addr: 0x95BF,
                entry_point: 0x6900,
                dep_addr: 0x01B3,
                instructions_executed: 603_651,
                len: 36_288,
            },
        ];

        let results: Vec<(
            ExpectedCorpusUnpack,
            Result<crate::unpacker::UnpackResult, String>,
        )> = std::thread::scope(|s| {
            let handles: Vec<_> = CASES
                .into_iter()
                .map(|case| {
                    s.spawn(move || {
                        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                            .join("../../tests/6502")
                            .join(case.file);
                        let bytes = std::fs::read(&path)
                            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
                        assert!(bytes.len() > 2, "{} is too short", case.file);
                        let load_addr = u16::from_le_bytes([bytes[0], bytes[1]]);
                        let payload = &bytes[2..];
                        let res = unpack(payload, load_addr, &UnpackConfig::default(), None)
                            .map_err(|e| e.to_string());
                        (case, res)
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });

        let mut failures = Vec::new();
        let mut moving_tubes_payloads: Vec<(&str, Vec<u8>)> = Vec::new();
        let mut mule_payloads: Vec<(&str, Vec<u8>)> = Vec::new();

        for (case, res) in results {
            let result = match res {
                Ok(r) => r,
                Err(e) => {
                    failures.push(format!("{}: unpack failed: {e}", case.file));
                    continue;
                }
            };

            if result.packer_name.as_deref() != Some(case.packer_name) {
                failures.push(format!(
                    "{}: packer_name {:?} != Some({:?})",
                    case.file, result.packer_name, case.packer_name
                ));
            }
            if result.start_addr != case.start_addr {
                failures.push(format!(
                    "{}: start_addr ${:04X} != ${:04X}",
                    case.file, result.start_addr, case.start_addr
                ));
            }
            if result.end_addr != case.end_addr {
                failures.push(format!(
                    "{}: end_addr ${:04X} != ${:04X}",
                    case.file, result.end_addr, case.end_addr
                ));
            }
            if result.entry_point != case.entry_point {
                failures.push(format!(
                    "{}: entry_point ${:04X} != ${:04X}",
                    case.file, result.entry_point, case.entry_point
                ));
            }
            if result.dep_addr != case.dep_addr {
                failures.push(format!(
                    "{}: dep_addr ${:04X} != ${:04X}",
                    case.file, result.dep_addr, case.dep_addr
                ));
            }
            if result.instructions_executed != case.instructions_executed {
                failures.push(format!(
                    "{}: instructions_executed {} != {}",
                    case.file, result.instructions_executed, case.instructions_executed
                ));
            }
            if result.data.len() != case.len {
                failures.push(format!(
                    "{}: data.len() {} != {}",
                    case.file,
                    result.data.len(),
                    case.len
                ));
            }

            if case.file.starts_with("c64_moving_tubes_lxt.") {
                moving_tubes_payloads.push((case.file, result.data));
            } else if case.file.starts_with("c64_mule.") {
                mule_payloads.push((case.file, result.data));
            }
        }

        assert!(failures.is_empty(), "{}", failures.join("\n"));

        // Verify ground-truth & consensus across Exomizer 3.0, TSCrunch v1.3+, and TSCrunch v1.3+-X2
        // on `moving_tubes` and `mule`.
        assert_eq!(moving_tubes_payloads.len(), 3);
        for pair in moving_tubes_payloads.windows(2) {
            assert_eq!(
                pair[0].1, pair[1].1,
                "moving_tubes payload mismatch between {} and {}",
                pair[0].0, pair[1].0
            );
        }
        assert_eq!(mule_payloads.len(), 3);
        for pair in mule_payloads.windows(2) {
            assert_eq!(
                pair[0].1, pair[1].1,
                "mule payload mismatch between {} and {}",
                pair[0].0, pair[1].0
            );
        }
    }

    // =======================================================================
    // M3.5 — Dali + ByteBoozer + McCracken (`T3.3.9`–`T3.3.11`, `T3.3.14`)
    // =======================================================================

    /// T3.3.9: `dali::SPEC` resolves `dep_addr = $0003`, `start_addr = None` (M1.4 Dali padding),
    /// `entry_point = OperandLe { from_match: 0xFE }`, and dynamic `end_addr = PointerAfterRun`
    /// from `mem[q + 0x44]`; `DaliHooks::refine` trims `bounds.end_addr` to `max_gap_start - 1`
    /// when `mem[$EA] == 0x4C`, `bounds.entry_point == u16::from_le_bytes([mem[$EB], mem[$EC]])`,
    /// and `max_gap_len > 256` on C64.
    #[test]
    fn dali_refine_compares_entry_with_eb_ec() {
        // 1. Static detection & `PackerInfo::resolve` resolution:
        let (mut mem, load_addr, load_end) = dali::SPEC.build_fixture_memory();
        let q = 0x080D_usize;
        mem[q + 0x44] = 0x6B;
        mem[q + 0xFE] = 0x00;
        mem[q + 0xFF] = 0x11;

        let view = LoadedImage::for_c64(&mem, load_addr, load_end);
        let sig_match = dali::SPEC.detect(&view).expect("Dali matches");
        assert_eq!(sig_match.spec_name, "Dali");
        assert_eq!(sig_match.anchor_addr, 0x080D);
        assert_eq!(
            sig_match.resolved.hints,
            Hints {
                dep_addr: Some(AddrSource::Const(0x0003)),
                start_addr: None,
                end_addr: Some(AddrSource::PointerAfterRun {
                    ptr_addr: 0x006B,
                    adjust: -1,
                    guard: PointerGuard::NearDetectedEnd { slack: 512 },
                }),
                entry_point: Some(AddrSource::OperandLe { from_match: 0xFE }),
                ..Hints::NONE
            }
        );

        let info = PackerInfo::resolve(&view, &sig_match);
        assert_eq!(
            info,
            PackerInfo {
                name: "Dali",
                dep_addr: Some(0x0003),
                start_addr: None,
                end_addr: None,
                entry_point: Some(0x1100),
                end_addr_ptr: Some(0x006B),
            }
        );
        assert_eq!(find_entry_by_name("Dali").unwrap().id, "dali");

        // Boundary guards on `load_addr` and `mem.len()`:
        let view_high_load = LoadedImage::for_c64(&mem, 0x0802, load_end);
        assert!(dali::SPEC.detect(&view_high_load).is_none());
        let short_view = LoadedImage::for_c64(&mem[..=0x090B], 0x0801, 0x090C);
        assert!(dali::SPEC.detect(&short_view).is_none());

        // 2. `DaliHooks::refine` positive case:
        //    `mem[$EA] == 0x4C`, `mem[$EB..=$EC] == $1100 == bounds.entry_point`,
        //    and an unwritten gap of 257 bytes (`0x3200..=0x3300`) between payload (`0x0800..=0x31FF`)
        //    and high-memory residual writes (`0x3301..=0xFBFF`).
        let hooks = dali::DaliHooks::new();
        let mut final_mem = vec![0u8; 0x1_0000];
        final_mem[0xEA] = 0x4C;
        final_mem[0xEB..=0xEC].copy_from_slice(&0x1100_u16.to_le_bytes());

        let mut written = vec![false; 0x1_0000];
        written[0x0800..=0x31FF].fill(true);
        // Unwritten gap of 257 bytes at `0x3200..=0x3300`, then written `0x3301..=0xFFFF`
        // so the gap at `0x3200` is the largest gap (`257 > 256`).
        written[0x3301..=0xFFFF].fill(true);

        let ctx = RefineContext {
            mem: &final_mem,
            snapshot: &mem,
            written: &written,
            system: &TargetSystem::C64,
            registers: Registers::default(),
        };
        let mut bounds = Bounds {
            start_addr: 0x0800,
            end_addr: 0xFBFF,
            entry_point: 0x1100,
        };
        hooks.refine(&ctx, &mut bounds);
        assert_eq!(bounds.end_addr, 0x31FF);

        // 3. Exact boundary `max_gap_len == 256` (must NOT trim, since condition is `> 256`):
        let mut written_256 = vec![true; 0x1_0000];
        written_256[0x0000..0x0800].fill(false);
        written_256[0x3200..0x3300].fill(false); // exactly 256 bytes unwritten
        let ctx_256 = RefineContext {
            written: &written_256,
            ..ctx
        };
        let mut bounds_256 = Bounds {
            start_addr: 0x0800,
            end_addr: 0xFBFF,
            entry_point: 0x1100,
        };
        hooks.refine(&ctx_256, &mut bounds_256);
        assert_eq!(bounds_256.end_addr, 0xFBFF);

        // 4. Negative guard: `mem[$EB..=$EC] != bounds.entry_point` -> no-op:
        let mut bounds_mismatch = Bounds {
            start_addr: 0x0800,
            end_addr: 0xFBFF,
            entry_point: 0x2000,
        };
        hooks.refine(&ctx, &mut bounds_mismatch);
        assert_eq!(bounds_mismatch.end_addr, 0xFBFF);

        // 5. Negative guard: `mem[$EA] != 0x4C` -> no-op:
        let mut mem_no_jmp = final_mem.clone();
        mem_no_jmp[0xEA] = 0x60;
        let ctx_no_jmp = RefineContext {
            mem: &mem_no_jmp,
            ..ctx
        };
        let mut bounds_no_jmp = Bounds {
            start_addr: 0x0800,
            end_addr: 0xFBFF,
            entry_point: 0x1100,
        };
        hooks.refine(&ctx_no_jmp, &mut bounds_no_jmp);
        assert_eq!(bounds_no_jmp.end_addr, 0xFBFF);

        // 6. Negative guard: non-C64 system (`TargetSystem::C128`) -> no-op:
        let ctx_c128 = RefineContext {
            system: &TargetSystem::C128,
            ..ctx
        };
        let mut bounds_c128 = Bounds {
            start_addr: 0x0800,
            end_addr: 0xFBFF,
            entry_point: 0x1100,
        };
        hooks.refine(&ctx_c128, &mut bounds_c128);
        assert_eq!(bounds_c128.end_addr, 0xFBFF);
    }

    /// T3.3.10: `byteboozer::SPEC` reads `start_addr` at `$0886..=$0887` (`OperandLe { from_match: 0x79 }`)
    /// and `entry_point` at `$08CB..=$08CC` (`OperandLe { from_match: 0xBE }`), and
    /// `ByteBoozerHooks::refine` rechecks all 4 snapshot slices (`$080D`, `$0813`, `$0870`, `$08C0`)
    /// before reading `mem[$77..=$78]` (`reported_end - 1`).
    #[test]
    fn byteboozer_refine_rechecks_snapshot_and_reads_absolute() {
        // 1. Static detection & operand resolution:
        let (mut mem, load_addr, load_end) = byteboozer::SPEC.build_fixture_memory();
        mem[0x0886..=0x0887].copy_from_slice(&0x0800_u16.to_le_bytes());
        mem[0x08CB..=0x08CC].copy_from_slice(&0x2800_u16.to_le_bytes());

        let view = LoadedImage::for_c64(&mem, load_addr, load_end);
        let sig_match = byteboozer::SPEC.detect(&view).expect("ByteBoozer matches");
        assert_eq!(sig_match.spec_name, "ByteBoozer");
        assert_eq!(sig_match.anchor_addr, 0x080D);

        let info = PackerInfo::resolve(&view, &sig_match);
        assert_eq!(
            info,
            PackerInfo {
                name: "ByteBoozer",
                dep_addr: Some(0x0010),
                start_addr: Some(0x0800),
                end_addr: None,
                entry_point: Some(0x2800),
                end_addr_ptr: Some(0x0077),
            }
        );
        assert_eq!(find_entry_by_name("ByteBoozer").unwrap().id, "byteboozer");

        // Boundary guards:
        let view_high_load = LoadedImage::for_c64(&mem, 0x0802, load_end);
        assert!(byteboozer::SPEC.detect(&view_high_load).is_none());
        let short_view = LoadedImage::for_c64(&mem[..=0x08CB], 0x0801, 0x08CC);
        assert!(byteboozer::SPEC.detect(&short_view).is_none());

        // 2. `ByteBoozerHooks::refine` positive case:
        let hooks = byteboozer::ByteBoozerHooks::new();
        let mut snapshot = vec![0u8; 0x1_0000];
        snapshot[0x080D..0x0811].copy_from_slice(&[0x78, 0xA9, 0x34, 0x85]);
        snapshot[0x0813..0x0817].copy_from_slice(&[0xB7, 0xBD, 0x1E, 0x08]);
        snapshot[0x0870..0x0874].copy_from_slice(&[0xA8, 0x20, 0xAD, 0x00]);
        snapshot[0x08C0..0x08C4].copy_from_slice(&[0xAE, 0xD0, 0x02, 0xE6]);

        let mut final_mem = vec![0u8; 0x1_0000];
        final_mem[0x77..=0x78].copy_from_slice(&0xA000_u16.to_le_bytes());
        let written = vec![false; 0x1_0000];

        let ctx = RefineContext {
            mem: &final_mem,
            snapshot: &snapshot,
            written: &written,
            system: &TargetSystem::C64,
            registers: Registers::default(),
        };
        let mut bounds = Bounds {
            start_addr: 0x0800,
            end_addr: 0xFFFF,
            entry_point: 0x2800,
        };
        hooks.refine(&ctx, &mut bounds);
        assert_eq!(bounds.end_addr, 0x9FFF);

        // 3. Corrupting each of the 4 snapshot slices individually makes `refine` a no-op:
        for slice_addr in [0x080D_usize, 0x0813, 0x0870, 0x08C0] {
            let mut bad_snapshot = snapshot.clone();
            bad_snapshot[slice_addr] ^= 0xFF;
            let bad_ctx = RefineContext {
                snapshot: &bad_snapshot,
                ..ctx
            };
            let mut b = Bounds {
                start_addr: 0x0800,
                end_addr: 0xFFFF,
                entry_point: 0x2800,
            };
            hooks.refine(&bad_ctx, &mut b);
            assert_eq!(
                b.end_addr, 0xFFFF,
                "corrupting snapshot slice at ${slice_addr:04X} must disable ByteBoozer refine"
            );
        }

        // 4. `reported_end <= bounds.start_addr` (`$0800`) -> no-op:
        let mut mem_low_end = final_mem.clone();
        mem_low_end[0x77..=0x78].copy_from_slice(&0x0800_u16.to_le_bytes());
        let ctx_low_end = RefineContext {
            mem: &mem_low_end,
            ..ctx
        };
        let mut bounds_low_end = Bounds {
            start_addr: 0x0800,
            end_addr: 0xFFFF,
            entry_point: 0x2800,
        };
        hooks.refine(&ctx_low_end, &mut bounds_low_end);
        assert_eq!(bounds_low_end.end_addr, 0xFFFF);

        // 5. Non-C64 system (`TargetSystem::C128`) -> no-op:
        let ctx_c128 = RefineContext {
            system: &TargetSystem::C128,
            ..ctx
        };
        let mut bounds_c128 = Bounds {
            start_addr: 0x0800,
            end_addr: 0xFFFF,
            entry_point: 0x2800,
        };
        hooks.refine(&ctx_c128, &mut bounds_c128);
        assert_eq!(bounds_c128.end_addr, 0xFFFF);
    }

    /// T3.3.11: `mc_cracken::SPEC` declares `end_addr: Some(AddrSource::PointerAfterRun { ptr_addr: 0x00AE, adjust: -1, guard: PointerGuard::McCrackenEndOfProgram })`,
    /// and the two-step end pointer order runs the generic `+512` slack guard first in `engine::finish`
    /// step 4 (`end_addr_ptr = Some(0x00AE)`), followed by `PointerGuard::McCrackenEndOfProgram`
    /// (`raw > start_addr && raw < 0xFFFF => raw - 1`) in step 7 on C64.
    #[test]
    fn mc_cracken_end_pointer_two_step_order() {
        // 1. Static detection in window `$0810..=$0840` (with default `$1100` and custom `JMP` at `p + 0x61`):
        let mut mem = vec![0u8; 0x1_0000];
        let p = 0x0825_usize;
        mem[p..p + 8].copy_from_slice(&[0xA9, 0x12, 0x85, 0x34, 0xA0, 0x00, 0xC6, 0xAF]);

        let view = LoadedImage::for_c64(&mem, 0x0801, 0x0900);
        let sig_match = mc_cracken::SPEC
            .detect(&view)
            .expect("McCracken matches at $0825");
        assert_eq!(sig_match.spec_name, "McCracken Compressor");
        assert_eq!(sig_match.anchor_addr, 0x0825);
        assert_eq!(
            sig_match.resolved.hints.start_addr,
            Some(AddrSource::BasicStart(-1))
        );
        assert_eq!(
            sig_match.resolved.hints.end_addr,
            Some(AddrSource::PointerAfterRun {
                ptr_addr: 0x00AE,
                adjust: -1,
                guard: PointerGuard::McCrackenEndOfProgram,
            })
        );

        let info_default = PackerInfo::resolve(&view, &sig_match);
        assert_eq!(
            info_default,
            PackerInfo {
                name: "McCracken Compressor",
                dep_addr: Some(0x0100),
                start_addr: Some(0x0800),
                end_addr: None,
                entry_point: Some(0x1100),
                end_addr_ptr: Some(0x00AE),
            }
        );

        // Custom JMP at `p + 0x61`:
        mem[p + 0x61] = 0x4C;
        mem[p + 0x62..=p + 0x63].copy_from_slice(&0x2468_u16.to_le_bytes());
        let view_jmp = LoadedImage::for_c64(&mem, 0x0801, 0x0900);
        let sig_match_jmp = mc_cracken::SPEC.detect(&view_jmp).unwrap();
        let info_jmp = PackerInfo::resolve(&view_jmp, &sig_match_jmp);
        assert_eq!(info_jmp.entry_point, Some(0x2468));

        // Per-position `mem.len() > p + 0x40` guard:
        let short_view = LoadedImage::for_c64(&mem[..p + 0x40], 0x0801, (p + 0x40) as u16);
        assert!(mc_cracken::SPEC.detect(&short_view).is_none());

        // 2. Two-step end-pointer order (`engine::finish` step 4 generic `+512` slack guard -> step 7 `McCrackenEndOfProgram`):
        //    Helper simulating `engine::finish` step 4 (`end_addr_ptr`) followed by step 7 (`McCrackenEndOfProgram`).
        let run_two_step = |raw_ptr: u16, detected_end: u16, system: &TargetSystem| -> (u16, u16) {
            let mut run_mem = vec![0u8; 0x1_0000];
            run_mem[0x00AE..=0x00AF].copy_from_slice(&raw_ptr.to_le_bytes());
            let mut range = (0x0800_u16, detected_end);

            // Step 4: generic `+512` slack guard via `end_addr_ptr` (`engine.rs`):
            if let Some(ptr_addr) = info_default.end_addr_ptr {
                let step4_src = AddrSource::PointerAfterRun {
                    ptr_addr,
                    adjust: -1,
                    guard: PointerGuard::NearDetectedEnd { slack: 512 },
                };
                if let Some(step4_end) = step4_src.resolve_post_run(
                    &run_mem,
                    Registers::default(),
                    range.0,
                    range.1,
                    &PointerMonitor::new(),
                ) {
                    range.1 = step4_end;
                }
            }
            let after_step4 = range.1;

            // Step 7: `PointerGuard::McCrackenEndOfProgram` on C64 (`engine.rs`):
            if system.is_c64()
                && let Some(
                    src @ AddrSource::PointerAfterRun {
                        guard: PointerGuard::McCrackenEndOfProgram,
                        ..
                    },
                ) = sig_match.resolved.hints.end_addr
                && let Some(step7_end) = src.resolve_post_run(
                    &run_mem,
                    Registers::default(),
                    range.0,
                    range.1,
                    &PointerMonitor::new(),
                )
            {
                range.1 = step7_end;
            }
            (after_step4, range.1)
        };

        // Case A (`raw_ptr = $A000`, `detected_end = $9000`):
        // - Step 4 (`NearDetectedEnd { slack: 512 }`) REJECTS `$9FFF` (`$9FFF > $9000 + 512`), leaving `$9000`.
        // - Step 7 (`McCrackenEndOfProgram` on C64) ACCEPTS `$9FFF` (`$A000 > $0800 && $A000 < $FFFF`).
        let (step4_a, step7_a) = run_two_step(0xA000, 0x9000, &TargetSystem::C64);
        assert_eq!(step4_a, 0x9000);
        assert_eq!(step7_a, 0x9FFF);

        // On non-C64 (`TargetSystem::C128`), step 7 is a no-op so step 4's `$9000` stands:
        let (step4_c128, step7_c128) = run_two_step(0xA000, 0x9000, &TargetSystem::C128);
        assert_eq!(step4_c128, 0x9000);
        assert_eq!(step7_c128, 0x9000);

        // Case B (`raw_ptr = $FFFF`, `detected_end = $FEFF`):
        // - Step 4 (`NearDetectedEnd { slack: 512 }`) ACCEPTS `$FFFE` (`$FFFE <= $FEFF.saturating_add(512)`).
        // - Step 7 (`McCrackenEndOfProgram`: `raw < 0xFFFF` is false) does NOT overwrite `range.1`,
        //   preserving step 4's `$FFFE` (proving step 4 ran before step 7).
        let (step4_b, step7_b) = run_two_step(0xFFFF, 0xFEFF, &TargetSystem::C64);
        assert_eq!(step4_b, 0xFFFE);
        assert_eq!(step7_b, 0xFFFE);
    }

    /// T3.3.14: Unpacking all 6 Dali, ByteBoozer, and McCracken corpus files (2 `Dali`,
    /// 3 `ByteBoozer`, and 1 `McCracken Compressor`) produces the exact baseline outputs
    /// and preserves ground-truth/consensus agreement on `mule`.
    #[test]
    fn dali_byteboozer_and_mc_cracken_corpus_unchanged() {
        #[derive(Clone, Copy)]
        struct ExpectedCorpusUnpack {
            file: &'static str,
            packer_name: &'static str,
            start_addr: u16,
            end_addr: u16,
            entry_point: u16,
            dep_addr: u16,
            instructions_executed: u64,
            len: usize,
        }

        const CASES: [ExpectedCorpusUnpack; 6] = [
            ExpectedCorpusUnpack {
                file: "c64_moving_tubes_lxt.dali.prg",
                packer_name: "Dali",
                start_addr: 0x0800,
                end_addr: 0x31FF,
                entry_point: 0x2E00,
                dep_addr: 0x0003,
                instructions_executed: 192_119,
                len: 10_752,
            },
            ExpectedCorpusUnpack {
                file: "c64_mule.dali.prg",
                packer_name: "Dali",
                start_addr: 0x0800,
                end_addr: 0x9D19,
                entry_point: 0x1100,
                dep_addr: 0x0003,
                instructions_executed: 597_330,
                len: 38_170,
            },
            ExpectedCorpusUnpack {
                file: "c64_CopperBooze.byte_boozer2.prg",
                packer_name: "ByteBoozer",
                start_addr: 0x0800,
                end_addr: 0xE7FF,
                entry_point: 0x1300,
                dep_addr: 0x0010,
                instructions_executed: 1_326_033,
                len: 57_344,
            },
            ExpectedCorpusUnpack {
                file: "c64_FppScroller.byte_boozer2.prg",
                packer_name: "ByteBoozer",
                start_addr: 0x0801,
                end_addr: 0xA057,
                entry_point: 0x080D,
                dep_addr: 0x0010,
                instructions_executed: 712_332,
                len: 38_999,
            },
            ExpectedCorpusUnpack {
                file: "c64_radiant-every_time_i_go_on_pouet.byte_boozer2prg.prg",
                packer_name: "ByteBoozer",
                start_addr: 0x0800,
                end_addr: 0x9FFF,
                entry_point: 0x2800,
                dep_addr: 0x0010,
                instructions_executed: 441_821,
                len: 38_912,
            },
            ExpectedCorpusUnpack {
                file: "c64_mule.mccracken_compressor.prg",
                packer_name: "McCracken Compressor",
                start_addr: 0x0800,
                end_addr: 0x9D19,
                entry_point: 0x1100,
                dep_addr: 0x0100,
                instructions_executed: 605_091,
                len: 38_170,
            },
        ];

        let results: Vec<(
            ExpectedCorpusUnpack,
            Result<crate::unpacker::UnpackResult, String>,
        )> = std::thread::scope(|s| {
            let handles: Vec<_> = CASES
                .into_iter()
                .map(|case| {
                    s.spawn(move || {
                        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                            .join("../../tests/6502")
                            .join(case.file);
                        let bytes = std::fs::read(&path)
                            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
                        assert!(bytes.len() > 2, "{} is too short", case.file);
                        let load_addr = u16::from_le_bytes([bytes[0], bytes[1]]);
                        let payload = &bytes[2..];
                        let res = unpack(payload, load_addr, &UnpackConfig::default(), None)
                            .map_err(|e| e.to_string());
                        (case, res)
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });

        let mut failures = Vec::new();
        let mut mule_payloads: Vec<(&str, Vec<u8>)> = Vec::new();

        for (case, res) in results {
            let result = match res {
                Ok(r) => r,
                Err(e) => {
                    failures.push(format!("{}: unpack failed: {e}", case.file));
                    continue;
                }
            };

            if result.packer_name.as_deref() != Some(case.packer_name) {
                failures.push(format!(
                    "{}: packer_name {:?} != Some({:?})",
                    case.file, result.packer_name, case.packer_name
                ));
            }
            if result.start_addr != case.start_addr {
                failures.push(format!(
                    "{}: start_addr ${:04X} != ${:04X}",
                    case.file, result.start_addr, case.start_addr
                ));
            }
            if result.end_addr != case.end_addr {
                failures.push(format!(
                    "{}: end_addr ${:04X} != ${:04X}",
                    case.file, result.end_addr, case.end_addr
                ));
            }
            if result.entry_point != case.entry_point {
                failures.push(format!(
                    "{}: entry_point ${:04X} != ${:04X}",
                    case.file, result.entry_point, case.entry_point
                ));
            }
            if result.dep_addr != case.dep_addr {
                failures.push(format!(
                    "{}: dep_addr ${:04X} != ${:04X}",
                    case.file, result.dep_addr, case.dep_addr
                ));
            }
            if result.instructions_executed != case.instructions_executed {
                failures.push(format!(
                    "{}: instructions_executed {} != {}",
                    case.file, result.instructions_executed, case.instructions_executed
                ));
            }
            if result.data.len() != case.len {
                failures.push(format!(
                    "{}: data.len() {} != {}",
                    case.file,
                    result.data.len(),
                    case.len
                ));
            }

            if case.file.starts_with("c64_mule.") {
                mule_payloads.push((case.file, result.data));
            }
        }

        assert!(failures.is_empty(), "{}", failures.join("\n"));

        // Verify ground-truth & consensus across Dali and McCracken on `mule`.
        assert_eq!(mule_payloads.len(), 2);
        assert_eq!(
            mule_payloads[0].1, mule_payloads[1].1,
            "mule payload mismatch between {} and {}",
            mule_payloads[0].0, mule_payloads[1].0
        );
    }

    // =======================================================================
    // M3.6a — ALZ64 & Antiram migration (`T3.6a.1`–`T3.6a.3`)
    // =======================================================================

    /// T3.6a.1: `alz64::SPEC` detects both `ALZ64/Quiss` and `ALZ64/Kabuto`, resolves exact
    /// legacy `PackerInfo` (`dep_addr = $005E`, default `start_addr = $080B` vs optional
    /// `[0x02, 0xE6, 0xC7, 0x8D]` scan override, indirect `entry_point`, `forced_entry = $080B`,
    /// `exact_return = true` for Quiss, and `$080B = $A2` patch for Kabuto), maps both variant
    /// names via `find_entry_by_name`, and unpacks `c64_boo_alz64.prg` identically to baseline.
    #[test]
    fn alz64_spec_hints_equal_legacy_packer_info() {
        for name in ["ALZ64/Quiss", "ALZ64/Kabuto"] {
            let entry = find_entry_by_name(name)
                .unwrap_or_else(|| panic!("missing registry entry for `{name}`"));
            assert_eq!(entry.id, "alz64");
        }

        fn load_fixture_at_0801(payload: &[u8]) -> (Vec<u8>, u16, u16) {
            let mut mem = vec![0u8; 0x1_0000];
            let load_addr: u16 = 0x0801;
            let start = usize::from(load_addr);
            mem[start..start + payload.len()].copy_from_slice(payload);
            let load_end = load_addr.wrapping_add(payload.len() as u16);
            (mem, load_addr, load_end)
        }

        // 1. ALZ64/Quiss with optional `[0x02, 0xE6, 0xC7, 0x8D]` start_addr override:
        let (quiss_mem, quiss_load, quiss_end) =
            load_fixture_at_0801(&alz64::build_quiss_fixture());
        let quiss_view = LoadedImage::for_c64(&quiss_mem, quiss_load, quiss_end);
        let quiss_match = alz64::SPEC
            .detect(&quiss_view)
            .expect("ALZ64/Quiss matches");
        assert_eq!(quiss_match.spec_name, "ALZ64/Quiss");
        assert_eq!(quiss_match.anchor_addr, 0x080C);
        assert!(quiss_match.resolved.patches.is_empty());
        assert_eq!(
            quiss_match.resolved.hints.forced_entry,
            Some(AddrSource::Const(0x080B))
        );
        assert!(quiss_match.resolved.hints.exact_return);
        assert_eq!(
            PackerInfo::resolve(&quiss_view, &quiss_match),
            PackerInfo {
                name: "ALZ64/Quiss",
                dep_addr: Some(0x005E),
                start_addr: Some(0x0801),
                end_addr: None,
                entry_point: Some(0x2000),
                end_addr_ptr: Some(0x00CF),
            }
        );

        // Without the `[0x02, 0xE6, 0xC7, 0x8D]` sequence, `start_addr` defaults to `$080B`:
        let mut quiss_default = quiss_mem.clone();
        quiss_default[0x09CB..=0x09D0].fill(0);
        let quiss_def_view = LoadedImage::for_c64(&quiss_default, quiss_load, quiss_end);
        let quiss_def_match = alz64::SPEC.detect(&quiss_def_view).unwrap();
        assert_eq!(
            PackerInfo::resolve(&quiss_def_view, &quiss_def_match).start_addr,
            Some(0x080B)
        );

        // Single-byte corruptions at each checked Quiss header position and indirect `JMP` at `p + 0xF1`:
        for corrupt_addr in [
            0x080B_usize,
            0x080C,
            0x080D,
            0x080E,
            0x0819,
            0x081A,
            0x081B,
            0x081C,
            0x0824,
            0x0825,
            0x0826,
            0x0827,
            0x082C,
            0x082D,
            0x082E,
            0x082F,
            0x09F1,
        ] {
            let mut bad = quiss_mem.clone();
            bad[corrupt_addr] ^= 0xFF;
            let bad_view = LoadedImage::for_c64(&bad, quiss_load, quiss_end);
            assert!(
                alz64::SPEC.detect(&bad_view).is_none(),
                "ALZ64/Quiss must reject corruption at ${corrupt_addr:04X}"
            );
        }

        // `p + 0xF3 >= mem.len()` guard:
        let short_quiss = LoadedImage::for_c64(&quiss_mem[..=0x09F2], quiss_load, 0x09F2);
        assert!(alz64::SPEC.detect(&short_quiss).is_none());

        // 2. ALZ64/Kabuto with optional `[0x02, 0xE6, 0xC7, 0x8D]` start_addr override:
        let (kabuto_mem, kabuto_load, kabuto_end) =
            load_fixture_at_0801(&alz64::build_kabuto_fixture());
        let kabuto_view = LoadedImage::for_c64(&kabuto_mem, kabuto_load, kabuto_end);
        let kabuto_match = alz64::SPEC
            .detect(&kabuto_view)
            .expect("ALZ64/Kabuto matches");
        assert_eq!(kabuto_match.spec_name, "ALZ64/Kabuto");
        assert_eq!(kabuto_match.anchor_addr, 0x080C);
        assert_eq!(
            kabuto_match.resolved.patches,
            &[MemoryPatch {
                addr: PatchAddr::Absolute(0x080B),
                expected: None,
                bytes: &[0xA2],
            }]
        );
        assert_eq!(
            kabuto_match.resolved.hints.forced_entry,
            Some(AddrSource::Const(0x080B))
        );
        assert!(!kabuto_match.resolved.hints.exact_return);
        assert_eq!(
            PackerInfo::resolve(&kabuto_view, &kabuto_match),
            PackerInfo {
                name: "ALZ64/Kabuto",
                dep_addr: Some(0x005E),
                start_addr: Some(0x0801),
                end_addr: None,
                entry_point: Some(0x2000),
                end_addr_ptr: Some(0x00CF),
            }
        );

        // Without the `[0x02, 0xE6, 0xC7, 0x8D]` sequence, `start_addr` defaults to `$080B`:
        let mut kabuto_default = kabuto_mem.clone();
        kabuto_default[0x09E6..=0x09EB].fill(0);
        let kabuto_def_view = LoadedImage::for_c64(&kabuto_default, kabuto_load, kabuto_end);
        let kabuto_def_match = alz64::SPEC.detect(&kabuto_def_view).unwrap();
        assert_eq!(
            PackerInfo::resolve(&kabuto_def_view, &kabuto_def_match).start_addr,
            Some(0x080B)
        );

        // Single-byte corruptions at each checked Kabuto header position and indirect `JMP` at `p + 0xFD`:
        for corrupt_addr in [
            0x080C_usize,
            0x080D,
            0x080E,
            0x0818,
            0x0819,
            0x081A,
            0x081B,
            0x0822,
            0x0823,
            0x0824,
            0x0825,
            0x0835,
            0x0836,
            0x0837,
            0x0838,
            0x09FD,
        ] {
            let mut bad = kabuto_mem.clone();
            bad[corrupt_addr] ^= 0xFF;
            let bad_view = LoadedImage::for_c64(&bad, kabuto_load, kabuto_end);
            assert!(
                alz64::SPEC.detect(&bad_view).is_none(),
                "ALZ64/Kabuto must reject corruption at ${corrupt_addr:04X}"
            );
        }

        // `p + 0xFF >= mem.len()` guard:
        let short_kabuto = LoadedImage::for_c64(&kabuto_mem[..=0x09FE], kabuto_load, 0x09FE);
        assert!(alz64::SPEC.detect(&short_kabuto).is_none());

        // 3. Corpus verification: `c64_boo_alz64.prg` unpacks identically to baseline.
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/6502")
            .join("c64_boo_alz64.prg");
        let bytes =
            std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let load_addr = u16::from_le_bytes([bytes[0], bytes[1]]);
        let result = unpack(&bytes[2..], load_addr, &UnpackConfig::default(), None)
            .expect("c64_boo_alz64.prg unpacks");
        assert_eq!(result.packer_name.as_deref(), Some("ALZ64/Quiss"));
        assert_eq!(result.start_addr, 0x0801);
        assert_eq!(result.end_addr, 0x4D3C);
        assert_eq!(result.entry_point, 0x2A78);
        assert_eq!(result.dep_addr, 0x005E);
        assert_eq!(result.instructions_executed, 2_829_809);
        assert_eq!(result.data.len(), 17_724);
    }

    /// T3.6a.2: `antiram::SPEC` detects both `Antiram Packer v1.0` and `Antiram Packer v2.0`,
    /// resolves exact `PackerInfo` (`dep_addr`, `BasicStart(0)` `start_addr`, `end_addr_ptr`
    /// at `$002D`, `entry_point` with and without `0x20` JSR skip, conditional `patches`,
    /// `forced_entry = $080D`, and `exact_return = true`), maps both variant names via
    /// `find_entry_by_name`, and unpacks `c64_chiller.antiram_packer.prg` identically to baseline.
    #[test]
    fn antiram_spec_hints_equal_legacy_packer_info() {
        fn load_fixture_at_0801(payload: &[u8]) -> (Vec<u8>, u16, u16) {
            let mut mem = vec![0u8; 0x1_0000];
            let load_addr: u16 = 0x0801;
            let start = usize::from(load_addr);
            mem[start..start + payload.len()].copy_from_slice(payload);
            let load_end = load_addr.wrapping_add(payload.len() as u16);
            (mem, load_addr, load_end)
        }

        for name in ["Antiram Packer v1.0", "Antiram Packer v2.0"] {
            let entry = find_entry_by_name(name)
                .unwrap_or_else(|| panic!("missing registry entry for `{name}`"));
            assert_eq!(entry.id, "antiram");
        }

        // 1. Antiram Packer v2.0 (without and with 0x20 JSR at $0911):
        let (mut v2_mem, v2_load, v2_end) = load_fixture_at_0801(&antiram::build_v2_fixture());
        let v2_view = LoadedImage::for_c64(&v2_mem, v2_load, v2_end);
        let v2_match = antiram::SPEC
            .detect(&v2_view)
            .expect("Antiram Packer v2.0 matches");
        assert_eq!(v2_match.spec_name, "Antiram Packer v2.0");
        assert_eq!(v2_match.anchor_addr, 0x080D);
        assert_eq!(
            v2_match.resolved.patches,
            &[MemoryPatch {
                addr: PatchAddr::Absolute(0x0911),
                expected: Some(0x20),
                bytes: &[0x2C],
            }]
        );
        assert_eq!(
            v2_match.resolved.hints.forced_entry,
            Some(AddrSource::Const(0x080D))
        );
        assert!(v2_match.resolved.hints.exact_return);
        assert_eq!(
            PackerInfo::resolve(&v2_view, &v2_match),
            PackerInfo {
                name: "Antiram Packer v2.0",
                dep_addr: Some(0xFF00),
                start_addr: Some(0x0801),
                end_addr: None,
                entry_point: Some(0x0818),
                end_addr_ptr: Some(0x002D),
            }
        );

        // With `mem[0x0911] == 0x20` (`JSR`), `entry_point` is read from `$0915..=$0916`:
        v2_mem[0x0911] = 0x20;
        v2_mem[0x0915..=0x0916].copy_from_slice(&0x4321_u16.to_le_bytes());
        let v2_jsr_view = LoadedImage::for_c64(&v2_mem, v2_load, v2_end);
        let v2_jsr_match = antiram::SPEC.detect(&v2_jsr_view).unwrap();
        assert_eq!(
            PackerInfo::resolve(&v2_jsr_view, &v2_jsr_match).entry_point,
            Some(0x4321)
        );

        // 2. Antiram Packer v1.0 (without and with 0x20 JSR at $0904):
        let (mut v1_mem, v1_load, v1_end) = load_fixture_at_0801(&antiram::build_v1_fixture());
        let v1_view = LoadedImage::for_c64(&v1_mem, v1_load, v1_end);
        let v1_match = antiram::SPEC
            .detect(&v1_view)
            .expect("Antiram Packer v1.0 matches");
        assert_eq!(v1_match.spec_name, "Antiram Packer v1.0");
        assert_eq!(v1_match.anchor_addr, 0x080D);
        assert_eq!(
            v1_match.resolved.patches,
            &[MemoryPatch {
                addr: PatchAddr::Absolute(0x0904),
                expected: Some(0x20),
                bytes: &[0x2C],
            }]
        );
        assert_eq!(
            v1_match.resolved.hints.forced_entry,
            Some(AddrSource::Const(0x080D))
        );
        assert!(v1_match.resolved.hints.exact_return);
        assert_eq!(
            PackerInfo::resolve(&v1_view, &v1_match),
            PackerInfo {
                name: "Antiram Packer v1.0",
                dep_addr: Some(0xFF00),
                start_addr: Some(0x0801),
                end_addr: None,
                entry_point: Some(0x1234),
                end_addr_ptr: Some(0x002D),
            }
        );

        // With `mem[0x0904] == 0x20` (`JSR`), `entry_point` is read from `$0908..=$0909`:
        v1_mem[0x0904] = 0x20;
        v1_mem[0x0908..=0x0909].copy_from_slice(&0x5678_u16.to_le_bytes());
        let v1_jsr_view = LoadedImage::for_c64(&v1_mem, v1_load, v1_end);
        let v1_jsr_match = antiram::SPEC.detect(&v1_jsr_view).unwrap();
        assert_eq!(
            PackerInfo::resolve(&v1_jsr_view, &v1_jsr_match).entry_point,
            Some(0x5678)
        );

        // Single-byte corruptions at `$080D`, `$081D`, `$082D`, `$08ED`:
        for corrupt_addr in [0x080D_usize, 0x081D, 0x082D, 0x08ED] {
            let mut bad_v2 = v2_mem.clone();
            bad_v2[corrupt_addr] ^= 0xFF;
            assert!(
                antiram::SPEC
                    .detect(&LoadedImage::for_c64(&bad_v2, v2_load, v2_end))
                    .is_none(),
                "Antiram v2.0 must reject corruption at ${corrupt_addr:04X}"
            );
            let mut bad_v1 = v1_mem.clone();
            bad_v1[corrupt_addr] ^= 0xFF;
            assert!(
                antiram::SPEC
                    .detect(&LoadedImage::for_c64(&bad_v1, v1_load, v1_end))
                    .is_none(),
                "Antiram v1.0 must reject corruption at ${corrupt_addr:04X}"
            );
        }

        // `mem.len() <= 0x0950` boundary guard (`0x0950` rejects, `0x0951` matches):
        let short_v2 = LoadedImage::for_c64(&v2_mem[..0x0950], v2_load, 0x0950);
        assert!(antiram::SPEC.detect(&short_v2).is_none());
        let exact_v2 = LoadedImage::for_c64(&v2_mem[..0x0951], v2_load, 0x0951);
        assert!(antiram::SPEC.detect(&exact_v2).is_some());

        // 3. Corpus verification: `c64_chiller.antiram_packer.prg` unpacks identically to baseline.
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/6502")
            .join("c64_chiller.antiram_packer.prg");
        let bytes =
            std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let load_addr = u16::from_le_bytes([bytes[0], bytes[1]]);
        let result = unpack(&bytes[2..], load_addr, &UnpackConfig::default(), None)
            .expect("c64_chiller.antiram_packer.prg unpacks");
        assert_eq!(result.packer_name.as_deref(), Some("Antiram Packer v2.0"));
        assert_eq!(result.start_addr, 0x0801);
        assert_eq!(result.end_addr, 0xCE00);
        assert_eq!(result.entry_point, 0x0818);
        assert_eq!(result.dep_addr, 0xFF00);
        assert_eq!(result.instructions_executed, 679_862);
        assert_eq!(result.data.len(), 50_688);
    }

    /// T3.6a.3: `alz64::SPEC` preserves `load_addr_max = Some(0x080B)` (`<= 0x080B` matches,
    /// `> 0x080B` rejects for both Quiss and Kabuto) and `antiram::SPEC` preserves
    /// `load_addr_max = None` (matches across all `load_addr` values for both v1.0 and v2.0).
    #[test]
    fn alz64_and_antiram_load_gates_preserved() {
        fn load_fixture_at_0801(payload: &[u8]) -> (Vec<u8>, u16) {
            let mut mem = vec![0u8; 0x1_0000];
            let load_addr: u16 = 0x0801;
            let start = usize::from(load_addr);
            mem[start..start + payload.len()].copy_from_slice(payload);
            let load_end = load_addr.wrapping_add(payload.len() as u16);
            (mem, load_end)
        }

        assert_eq!(alz64::SPEC.load_addr_max, Some(0x080B));
        assert_eq!(antiram::SPEC.load_addr_max, None);

        let (quiss_mem, quiss_end) = load_fixture_at_0801(&alz64::build_quiss_fixture());
        let (kabuto_mem, kabuto_end) = load_fixture_at_0801(&alz64::build_kabuto_fixture());
        for ok_load in [0x0000_u16, 0x0400, 0x0801, 0x080B] {
            let qv = LoadedImage::for_c64(&quiss_mem, ok_load, quiss_end);
            assert!(
                alz64::SPEC.detect(&qv).is_some(),
                "ALZ64/Quiss should match at load_addr=${ok_load:04X}"
            );
            let kv = LoadedImage::for_c64(&kabuto_mem, ok_load, kabuto_end);
            assert!(
                alz64::SPEC.detect(&kv).is_some(),
                "ALZ64/Kabuto should match at load_addr=${ok_load:04X}"
            );
        }
        for bad_load in [0x080C_u16, 0x080D, 0x1000, 0xFFFF] {
            let qv = LoadedImage::for_c64(&quiss_mem, bad_load, quiss_end);
            assert!(
                alz64::SPEC.detect(&qv).is_none(),
                "ALZ64/Quiss must reject load_addr=${bad_load:04X} > $080B"
            );
            let kv = LoadedImage::for_c64(&kabuto_mem, bad_load, kabuto_end);
            assert!(
                alz64::SPEC.detect(&kv).is_none(),
                "ALZ64/Kabuto must reject load_addr=${bad_load:04X} > $080B"
            );
        }

        let (v1_mem, v1_end) = load_fixture_at_0801(&antiram::build_v1_fixture());
        let (v2_mem, v2_end) = load_fixture_at_0801(&antiram::build_v2_fixture());
        for load_addr in [0x0000_u16, 0x0801, 0x080B, 0x080C, 0x080D, 0x0900, 0x1000] {
            let v1v = LoadedImage::for_c64(&v1_mem, load_addr, v1_end);
            assert!(
                antiram::SPEC.detect(&v1v).is_some(),
                "Antiram v1.0 should match at load_addr=${load_addr:04X} (no load gate)"
            );
            let v2v = LoadedImage::for_c64(&v2_mem, load_addr, v2_end);
            assert!(
                antiram::SPEC.detect(&v2v).is_some(),
                "Antiram v2.0 should match at load_addr=${load_addr:04X} (no load gate)"
            );
        }
    }

    /// T3.6.7: `antiram::SPEC` conditionally patches `$0904` (v1.0) or `$0911` (v2.0) from `0x20`
    /// (`JSR`) to `0x2C` (`BIT` absolute) and shifts the `entry_point` operand read offset by +3
    /// when `0x20` is present.
    #[test]
    fn antiram_conditional_patch_and_entry_offset() {
        fn load_fixture_at_0801(payload: &[u8]) -> (Vec<u8>, u16, u16) {
            let mut mem = vec![0u8; 0x1_0000];
            let load_addr: u16 = 0x0801;
            let start = usize::from(load_addr);
            mem[start..start + payload.len()].copy_from_slice(payload);
            let load_end = load_addr.wrapping_add(payload.len() as u16);
            (mem, load_addr, load_end)
        }

        // Antiram v2.0 without JSR ($0911 != 0x20): entry_point from $0912..=$0913, patch skipped.
        let (mut v2_mem, v2_load, v2_end) = load_fixture_at_0801(&antiram::build_v2_fixture());
        assert_ne!(v2_mem[0x0911], 0x20);
        let v2_match = antiram::SPEC
            .detect(&LoadedImage::for_c64(&v2_mem, v2_load, v2_end))
            .unwrap();
        assert_eq!(
            PackerInfo::resolve(&LoadedImage::for_c64(&v2_mem, v2_load, v2_end), &v2_match)
                .entry_point,
            Some(0x0818)
        );
        assert_eq!(v2_match.resolved.patches.len(), 1);
        assert!(
            !v2_match.resolved.patches[0].apply(&mut v2_mem, v2_match.anchor_addr),
            "v2.0 conditional patch must skip when mem[$0911] != 0x20"
        );

        // Antiram v2.0 with JSR ($0911 == 0x20): entry_point from $0915..=$0916, patch overwrites $0911 -> 0x2C.
        v2_mem[0x0911] = 0x20;
        v2_mem[0x0915..=0x0916].copy_from_slice(&0x4321_u16.to_le_bytes());
        let v2_jsr_view = LoadedImage::for_c64(&v2_mem, v2_load, v2_end);
        let v2_jsr_match = antiram::SPEC.detect(&v2_jsr_view).unwrap();
        assert_eq!(
            PackerInfo::resolve(&v2_jsr_view, &v2_jsr_match).entry_point,
            Some(0x4321)
        );
        assert!(v2_jsr_match.resolved.patches[0].apply(&mut v2_mem, v2_jsr_match.anchor_addr));
        assert_eq!(v2_mem[0x0911], 0x2C);

        // Antiram v1.0 without JSR ($0904 != 0x20): entry_point from $0905..=$0906, patch skipped.
        let (mut v1_mem, v1_load, v1_end) = load_fixture_at_0801(&antiram::build_v1_fixture());
        assert_ne!(v1_mem[0x0904], 0x20);
        let v1_match = antiram::SPEC
            .detect(&LoadedImage::for_c64(&v1_mem, v1_load, v1_end))
            .unwrap();
        assert_eq!(
            PackerInfo::resolve(&LoadedImage::for_c64(&v1_mem, v1_load, v1_end), &v1_match)
                .entry_point,
            Some(0x1234)
        );
        assert_eq!(v1_match.resolved.patches.len(), 1);
        assert!(
            !v1_match.resolved.patches[0].apply(&mut v1_mem, v1_match.anchor_addr),
            "v1.0 conditional patch must skip when mem[$0904] != 0x20"
        );

        // Antiram v1.0 with JSR ($0904 == 0x20): entry_point from $0908..=$0909, patch overwrites $0904 -> 0x2C.
        v1_mem[0x0904] = 0x20;
        v1_mem[0x0908..=0x0909].copy_from_slice(&0x5678_u16.to_le_bytes());
        let v1_jsr_view = LoadedImage::for_c64(&v1_mem, v1_load, v1_end);
        let v1_jsr_match = antiram::SPEC.detect(&v1_jsr_view).unwrap();
        assert_eq!(
            PackerInfo::resolve(&v1_jsr_view, &v1_jsr_match).entry_point,
            Some(0x5678)
        );
        assert!(v1_jsr_match.resolved.patches[0].apply(&mut v1_mem, v1_jsr_match.anchor_addr));
        assert_eq!(v1_mem[0x0904], 0x2C);
    }

    /// T3.6.8: both `Antiram Packer v1.0` and `Antiram Packer v2.0` declare
    /// `forced_entry = Some(AddrSource::Const(0x080D))` and `exact_return = true`.
    #[test]
    fn antiram_forced_entry_and_exact_return_hints() {
        for payload in [antiram::build_v1_fixture(), antiram::build_v2_fixture()] {
            let mut mem = vec![0u8; 0x1_0000];
            mem[0x0801..0x0801 + payload.len()].copy_from_slice(&payload);
            let load_end = 0x0801_u16.wrapping_add(payload.len() as u16);
            let view = LoadedImage::for_c64(&mem, 0x0801, load_end);
            let sig_match = antiram::SPEC
                .detect(&view)
                .expect("Antiram fixture matches");
            assert_eq!(
                sig_match.resolved.hints.forced_entry,
                Some(AddrSource::Const(0x080D))
            );
            assert!(sig_match.resolved.hints.exact_return);
        }
    }

    /// T3.6.9: `ALZ64/Kabuto` declares `forced_entry = Some(AddrSource::Const(0x080B))`,
    /// `exact_return = false`, and an unconditional pre-emulation patch writing `0xA2` (`LDX #imm`)
    /// at `$080B` (`unp64` `ALZ64.c:47`).
    #[test]
    fn alz64_kabuto_patch_and_forced_entry() {
        let payload = alz64::build_kabuto_fixture();
        let mut mem = vec![0u8; 0x1_0000];
        mem[0x0801..0x0801 + payload.len()].copy_from_slice(&payload);
        let load_end = 0x0801_u16.wrapping_add(payload.len() as u16);
        let view = LoadedImage::for_c64(&mem, 0x0801, load_end);
        let sig_match = alz64::SPEC
            .detect(&view)
            .expect("ALZ64/Kabuto fixture matches");

        assert_eq!(sig_match.spec_name, "ALZ64/Kabuto");
        assert_eq!(
            sig_match.resolved.hints.forced_entry,
            Some(AddrSource::Const(0x080B))
        );
        assert!(!sig_match.resolved.hints.exact_return);
        assert_eq!(
            sig_match.resolved.patches,
            &[MemoryPatch {
                addr: PatchAddr::Absolute(0x080B),
                expected: None,
                bytes: &[0xA2],
            }]
        );

        mem[0x080B] = 0x00;
        assert!(sig_match.resolved.patches[0].apply(&mut mem, sig_match.anchor_addr));
        assert_eq!(mem[0x080B], 0xA2);
    }

    /// T3.6.10: `ALZ64/Quiss` declares `forced_entry = Some(AddrSource::Const(0x080B))`,
    /// `exact_return = true` (`unp64` `ALZ64.c:30` `RtAFrc = 1`), and no patches (`patches.is_empty()`).
    #[test]
    fn alz64_quiss_has_exact_return_and_no_patch() {
        let payload = alz64::build_quiss_fixture();
        let mut mem = vec![0u8; 0x1_0000];
        mem[0x0801..0x0801 + payload.len()].copy_from_slice(&payload);
        let load_end = 0x0801_u16.wrapping_add(payload.len() as u16);
        let view = LoadedImage::for_c64(&mem, 0x0801, load_end);
        let sig_match = alz64::SPEC
            .detect(&view)
            .expect("ALZ64/Quiss fixture matches");

        assert_eq!(sig_match.spec_name, "ALZ64/Quiss");
        assert_eq!(
            sig_match.resolved.hints.forced_entry,
            Some(AddrSource::Const(0x080B))
        );
        assert!(sig_match.resolved.hints.exact_return);
        assert!(sig_match.resolved.patches.is_empty());
    }

    /// T3.6.13: `c64_chiller.antiram_packer.prg` contains `0x20` (`JSR $A659`) at `$0911`;
    /// `antiram::SPEC` skips the 3-byte `JSR` to read the `JMP $A7AE` operand (`$A7AE`, BASIC
    /// `RUN`) from `$0915..=$0916` and patches `$0911` to `0x2C` (`BIT $A659`), executing the
    /// `BIT $A659; JMP $A7AE` sequence (`679_862` instructions, matching `unp64 -v` `1029 + 678833`)
    /// before `finish` resolves the unpacked BASIC `SYS` line to `$0818`.
    #[test]
    fn chiller_antiram_patch_corpus_case() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/6502")
            .join("c64_chiller.antiram_packer.prg");
        let bytes =
            std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let load_addr = u16::from_le_bytes([bytes[0], bytes[1]]);
        assert_eq!(load_addr, 0x0801);

        let raw_payload = &bytes[2..];
        let mut mem = vec![0u8; 0x1_0000];
        mem[0x0801..0x0801 + raw_payload.len()].copy_from_slice(raw_payload);
        let load_end = load_addr.wrapping_add(raw_payload.len() as u16);
        assert_eq!(
            &mem[0x0911..=0x0916],
            &[0x20, 0x59, 0xA6, 0x4C, 0xAE, 0xA7],
            "chiller stub must have `JSR $A659; JMP $A7AE` at $0911..=$0916 before patching"
        );
        let view = LoadedImage::for_c64(&mem, load_addr, load_end);
        let sig_match = antiram::SPEC.detect(&view).expect("chiller matches v2.0");
        assert_eq!(
            PackerInfo::resolve(&view, &sig_match).entry_point,
            Some(0xA7AE)
        );

        let result = unpack(raw_payload, load_addr, &UnpackConfig::default(), None)
            .expect("c64_chiller.antiram_packer.prg unpacks");
        assert_eq!(result.packer_name.as_deref(), Some("Antiram Packer v2.0"));
        assert_eq!(result.start_addr, 0x0801);
        assert_eq!(result.end_addr, 0xCE00);
        assert_eq!(result.entry_point, 0x0818);
        assert_eq!(result.dep_addr, 0xFF00);
        assert_eq!(result.instructions_executed, 679_862);
        assert_eq!(result.data.len(), 50_688);
    }
}
