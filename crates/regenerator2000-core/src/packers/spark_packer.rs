//! Spark Packer specification (found in Spark cracks).
//!
//! Targets [`TargetSystem::C64`] images with `load_addr <= $0801`. Matches at `$080D`,
//! reads `start_addr` via [`AddrSource::SplitBytes`] (`$0847`/`$0858`), takes inclusive
//! `end_addr` from `$00FE` (`adjust: 0`), and reads `entry_point` at `$0883..=$0884`.

use crate::state::types::TargetSystem;

use super::spec::{
    AddrSource, Anchor, Hints, LoadedImage, PackerSpec, PointerGuard, ResolvedSpec, Signature,
    SignatureMatch, sig,
};

/// Primary Spark Packer 4-byte signature at `$080D` (`SEI, LDY #$4C, STY $xx`).
pub const SPARK_PACKER_SIGNATURE: Signature = Signature {
    anchor: Anchor::Absolute(0x080D),
    pattern: sig![0x78, 0xA0, 0x4C, 0x84],
};

const SIGNATURES: [Signature; 1] = [SPARK_PACKER_SIGNATURE];

const BASE_HINTS: Hints = Hints {
    dep_addr: Some(AddrSource::Const(0x0100)),
    // $080D + 0x3A = $0847 (low byte), $080D + 0x4B = $0858 (high byte)
    start_addr: Some(AddrSource::SplitBytes {
        lo_from_match: 0x3A,
        hi_from_match: 0x4B,
    }),
    // `$00FE` holds the inclusive last unpacked byte (`adjust: 0`).
    end_addr: Some(AddrSource::PointerAfterRun {
        ptr_addr: 0x00FE,
        adjust: 0,
        guard: PointerGuard::NearDetectedEnd { slack: 512 },
    }),
    // $080D + 0x76 = $0883..=$0884
    entry_point: Some(AddrSource::OperandLe { from_match: 0x76 }),
    forced_entry: Some(AddrSource::Const(0x080D)),
    ..Hints::NONE
};

/// Builds a synthetic `Spark Packer` raw payload (no 2-byte PRG header) for `$0801..=$0884` (132 bytes).
#[must_use]
pub const fn build_fixture() -> [u8; 0x84] {
    let mut buf = [0u8; 0x84];
    let base: usize = 0x0801;

    // $080D..=$0810: 0x844C_A078
    buf[0x080D - base] = 0x78;
    buf[0x080E - base] = 0xA0;
    buf[0x080F - base] = 0x4C;
    buf[0x0810 - base] = 0x84;

    // $0813..=$0816: 0xFA99_0843
    buf[0x0813 - base] = 0x43;
    buf[0x0814 - base] = 0x08;
    buf[0x0815 - base] = 0x99;
    buf[0x0816 - base] = 0xFA;

    // $0847 / $0858: split start_addr = $0801
    buf[0x0847 - base] = 0x01;
    buf[0x0858 - base] = 0x08;

    // $084F..=$0852: 0x3C20_2CF0
    buf[0x084F - base] = 0xF0;
    buf[0x0850 - base] = 0x2C;
    buf[0x0851 - base] = 0x20;
    buf[0x0852 - base] = 0x3C;

    // $0879..=$087C: 0x0100_4CF1
    buf[0x0879 - base] = 0xF1;
    buf[0x087A - base] = 0x4C;
    buf[0x087B - base] = 0x00;
    buf[0x087C - base] = 0x01;

    // $0883..=$0884: entry_point = $2000
    buf[0x0883 - base] = 0x00;
    buf[0x0884 - base] = 0x20;

    buf
}

const FIXTURE: [u8; 0x84] = build_fixture();

/// Validates Spark Packer markers and readable operands (`$0847`, `$0858`, `$0883`).
#[must_use]
fn resolve_spark_packer(
    image: &LoadedImage<'_>,
    _sig_match: &SignatureMatch,
) -> Option<ResolvedSpec> {
    if image.load_addr > 0x0801 || image.mem.len() <= 0x0884 {
        return None;
    }
    // Verify signature (`$080D`), zp copy target (`$0813`), inner copy loop (`$084F`), and `JMP $0100` (`$0879`).
    if image.read_u32_le(0x080D) != Some(0x844C_A078)
        || image.read_u32_le(0x0813) != Some(0xFA99_0843)
        || image.read_u32_le(0x084F) != Some(0x3C20_2CF0)
        || image.read_u32_le(0x0879) != Some(0x0100_4CF1)
    {
        return None;
    }
    let _ = BASE_HINTS.start_addr?.resolve_static(image, 0x080D)?;
    let _ = BASE_HINTS.entry_point?.resolve_static(image, 0x080D)?;
    Some(ResolvedSpec::from_hints(BASE_HINTS))
}

/// Declarative specification for Spark Packer.
pub static SPEC: PackerSpec = PackerSpec {
    id: "spark_packer",
    name: "Spark Packer",
    priority: 158,
    systems: &[TargetSystem::C64],
    load_addr_max: Some(0x0801),
    signatures: &SIGNATURES,
    hints: BASE_HINTS,
    patches: &[],
    resolve: Some(resolve_spark_packer),
    hooks: None,
    fixture: &FIXTURE,
    fixture_load_addr: 0x0801,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packers::PackerInfo;

    #[test]
    fn spark_packer_resolves_split_start_and_inclusive_end_ptr() {
        let (mem, load_addr, load_end) = SPEC.build_fixture_memory();
        let view = LoadedImage::for_c64(&mem, load_addr, load_end);
        let sig_match = SPEC.detect(&view).expect("Spark Packer must match");
        let info = PackerInfo::resolve(&view, &sig_match);
        assert_eq!(info.name, "Spark Packer");
        assert_eq!(info.dep_addr, Some(0x0100));
        assert_eq!(info.start_addr, Some(0x0801));
        assert_eq!(info.entry_point, Some(0x2000));
        assert_eq!(info.end_addr_ptr, Some(0x00FE));

        // Short buffer (`<= $0884`), `load_addr > $0801`, and corrupted secondary marker reject cleanly.
        let short_view = LoadedImage::for_c64(&mem[..0x0884], 0x0801, 0x0884);
        assert!(SPEC.detect(&short_view).is_none());
        let high_load = LoadedImage::for_c64(&mem, 0x0802, load_end);
        assert!(SPEC.detect(&high_load).is_none());
        let mut corrupt = mem;
        corrupt[0x084F] = 0x00;
        let corrupt_view = LoadedImage::for_c64(&corrupt, load_addr, load_end);
        assert!(SPEC.detect(&corrupt_view).is_none());
    }
}
