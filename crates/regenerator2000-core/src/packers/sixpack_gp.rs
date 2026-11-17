//! 6Pack/G*P packer specification (by Six/G*P^DLoC, based on ZX0).
//!
//! Targets [`TargetSystem::C64`] images with `load_addr <= $0801`. Matches at `$080D`,
//! selects variant 1 (`$0854`) or variant 2 (`$0861`) for `entry_point`, reads `start_addr`
//! via [`AddrSource::SplitBytes`] (`$084E`/`$0852`), and takes inclusive `end_addr` from `$00FA`.

use crate::state::types::TargetSystem;

use super::spec::{
    AddrSource, Anchor, Hints, LoadedImage, PackerSpec, PointerGuard, ResolvedSpec, Signature,
    SignatureMatch, sig,
};

/// Primary 6Pack/G*P 4-byte signature at `$080D` (`LDX #$FF, TXS, SEI`).
pub const SIXPACK_GP_SIGNATURE: Signature = Signature {
    anchor: Anchor::Absolute(0x080D),
    pattern: sig![0xA2, 0xFF, 0x9A, 0x78],
};

const SIGNATURES: [Signature; 1] = [SIXPACK_GP_SIGNATURE];

const BASE_HINTS: Hints = Hints {
    dep_addr: Some(AddrSource::Const(0x0100)),
    // `$080D + 0x41 = $084E` (low byte), `$080D + 0x45 = $0852` (high byte)
    start_addr: Some(AddrSource::SplitBytes {
        lo_from_match: 0x41,
        hi_from_match: 0x45,
    }),
    // `$00FA` holds the inclusive last unpacked byte (`adjust: 0`).
    end_addr: Some(AddrSource::PointerAfterRun {
        ptr_addr: 0x00FA,
        adjust: 0,
        guard: PointerGuard::NearDetectedEnd { slack: 512 },
    }),
    entry_point: Some(AddrSource::OperandLe { from_match: 0xB6 }),
    forced_entry: Some(AddrSource::Const(0x080D)),
    ..Hints::NONE
};

/// Builds a synthetic `6Pack/G*P` raw payload (no 2-byte PRG header) for `$0801..=$08C4` (196 bytes).
#[must_use]
pub const fn build_fixture() -> [u8; 0xC4] {
    let mut buf = [0u8; 0xC4];
    let base: usize = 0x0801;

    // $080D..=$0810: 0x789A_FFA2
    buf[0x080D - base] = 0xA2;
    buf[0x080E - base] = 0xFF;
    buf[0x080F - base] = 0x9A;
    buf[0x0810 - base] = 0x78;

    // $0819..=$0820: 0x0100_9D08 + 0xD080_E0E8
    buf[0x0819 - base] = 0x08;
    buf[0x081A - base] = 0x9D;
    buf[0x081B - base] = 0x00;
    buf[0x081C - base] = 0x01;
    buf[0x081D - base] = 0xE8;
    buf[0x081E - base] = 0xE0;
    buf[0x081F - base] = 0x80;
    buf[0x0820 - base] = 0xD0;

    // $083D..=$0844: 0x38EE_0835 + 0xEED0_CA08
    buf[0x083D - base] = 0x35;
    buf[0x083E - base] = 0x08;
    buf[0x083F - base] = 0xEE;
    buf[0x0840 - base] = 0x38;
    buf[0x0841 - base] = 0x08;
    buf[0x0842 - base] = 0xCA;
    buf[0x0843 - base] = 0xD0;
    buf[0x0844 - base] = 0xEE;

    // $084E / $0852: split start_addr = $0801
    buf[0x084E - base] = 0x01;
    buf[0x0852 - base] = 0x08;

    // $0854..=$0857: 0x0100_4CFB (variant 1)
    buf[0x0854 - base] = 0xFB;
    buf[0x0855 - base] = 0x4C;
    buf[0x0856 - base] = 0x00;
    buf[0x0857 - base] = 0x01;

    // $08C3..=$08C4: entry_point = $2000
    buf[0x08C3 - base] = 0x00;
    buf[0x08C4 - base] = 0x20;

    buf
}

const FIXTURE: [u8; 0xC4] = build_fixture();

/// Validates 6Pack/G*P markers and selects either variant 1 (`$0854`) or variant 2 (`$0861`).
#[must_use]
fn resolve_sixpack_gp(
    image: &LoadedImage<'_>,
    _sig_match: &SignatureMatch,
) -> Option<ResolvedSpec> {
    if image.load_addr > 0x0801 {
        return None;
    }
    if image.read_u32_le(0x080D) != Some(0x789A_FFA2)
        || image.read_u32_le(0x0819) != Some(0x0100_9D08)
        || image.read_u32_le(0x081D) != Some(0xD080_E0E8)
        || image.read_u32_le(0x083D) != Some(0x38EE_0835)
        || image.read_u32_le(0x0841) != Some(0xEED0_CA08)
    {
        return None;
    }

    // Distinguish variant 1 (`JMP $0100` at `$0855`, entry at `$080D + 0xB6 = $08C3`) from
    // variant 2 (`JMP $0100` at `$0862`, entry at `$080D + 0xC3 = $08D0`).
    let entry_from_match = if image.read_u32_le(0x0854) == Some(0x0100_4CFB) {
        0xB6
    } else if image.read_u32_le(0x0861) == Some(0x0100_4CF5) {
        0xC3
    } else {
        return None;
    };

    let entry_src = AddrSource::OperandLe {
        from_match: entry_from_match,
    };
    let _ = BASE_HINTS.start_addr?.resolve_static(image, 0x080D)?;
    let _ = entry_src.resolve_static(image, 0x080D)?;

    Some(ResolvedSpec::from_hints(Hints {
        entry_point: Some(entry_src),
        ..BASE_HINTS
    }))
}

/// Declarative specification for 6Pack/G*P.
pub static SPEC: PackerSpec = PackerSpec {
    id: "sixpack_gp",
    name: "6Pack/G*P",
    priority: 156,
    systems: &[TargetSystem::C64],
    load_addr_max: Some(0x0801),
    signatures: &SIGNATURES,
    hints: BASE_HINTS,
    patches: &[],
    resolve: Some(resolve_sixpack_gp),
    hooks: None,
    fixture: &FIXTURE,
    fixture_load_addr: 0x0801,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packers::PackerInfo;

    #[test]
    fn sixpack_gp_resolves_both_variants_and_inclusive_end_ptr() {
        // Variant 1 ($0854)
        let (mem1, load_addr, load_end) = SPEC.build_fixture_memory();
        let view1 = LoadedImage::for_c64(&mem1, load_addr, load_end);
        let m1 = SPEC.detect(&view1).expect("variant 1 must match");
        let info1 = PackerInfo::resolve(&view1, &m1);
        assert_eq!(info1.name, "6Pack/G*P");
        assert_eq!(info1.dep_addr, Some(0x0100));
        assert_eq!(info1.start_addr, Some(0x0801));
        assert_eq!(info1.entry_point, Some(0x2000));
        assert_eq!(info1.end_addr_ptr, Some(0x00FA));

        // Variant 2 ($0861 -> entry at $08D0..=$08D1)
        let mut mem2 = mem1.clone();
        mem2[0x0854] = 0x00;
        mem2[0x0861..=0x0864].copy_from_slice(&[0xF5, 0x4C, 0x00, 0x01]);
        mem2[0x08D0..=0x08D1].copy_from_slice(&0x3456_u16.to_le_bytes());
        let view2 = LoadedImage::for_c64(&mem2, load_addr, 0x08D2);
        let m2 = SPEC.detect(&view2).expect("variant 2 must match");
        assert_eq!(PackerInfo::resolve(&view2, &m2).entry_point, Some(0x3456));

        // Short buffer (`<= $08C4`), `load_addr > $0801`, and missing variant tag reject cleanly.
        let short_v1 = LoadedImage::for_c64(&mem1[..0x08C4], 0x0801, 0x08C4);
        assert!(SPEC.detect(&short_v1).is_none());
        let high_load = LoadedImage::for_c64(&mem1, 0x0802, load_end);
        assert!(SPEC.detect(&high_load).is_none());
        mem2[0x0861] = 0x00;
        let broken = LoadedImage::for_c64(&mem2, 0x0801, 0x08D2);
        assert!(SPEC.detect(&broken).is_none());
    }
}
