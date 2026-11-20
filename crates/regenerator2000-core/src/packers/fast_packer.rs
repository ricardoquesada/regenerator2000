//! Fast Packer v1.0 (`lzss_compressor` by Repose/Style) specification.
//!
//! Targets [`TargetSystem::C64`] images with `load_addr <= $0801`. Matches at `$080E`,
//! resolves a high-RAM `dep_addr` from `$0856..=$0857` (so Phase 1 exits at `pc == dep_addr`),
//! and computes inclusive `end_addr = start + len - 1` from `$0A92` and `$0A90`.

use crate::state::types::TargetSystem;

use super::spec::{
    AddrSource, Anchor, Hints, LoadedImage, PackerSpec, ResolvedSpec, Signature, SignatureMatch,
    sig,
};

/// Primary Fast Packer v1.0 4-byte signature at `$080E` (`CLD, LDA $01, STA ...`).
pub const FAST_PACKER_SIGNATURE: Signature = Signature {
    anchor: Anchor::Absolute(0x080E),
    pattern: sig![0xD8, 0xA5, 0x01, 0x85],
};

const SIGNATURES: [Signature; 1] = [FAST_PACKER_SIGNATURE];

const BASE_HINTS: Hints = Hints {
    dep_addr: Some(AddrSource::AbsoluteLe(0x0856)),
    start_addr: Some(AddrSource::AbsoluteLe(0x0A92)),
    entry_point: Some(AddrSource::AbsoluteLe(0x0A88)),
    forced_entry: Some(AddrSource::Const(0x080E)),
    ..Hints::NONE
};

/// Builds a synthetic `Fast Packer v1.0` raw payload (no 2-byte PRG header)
/// for `$0801..=$0A93` (659 bytes).
#[must_use]
pub const fn build_fixture() -> [u8; 0x293] {
    let mut buf = [0u8; 0x293];
    let base: usize = 0x0801;

    // $080E..=$0811: 0x8501_A5D8
    buf[0x080E - base] = 0xD8;
    buf[0x080F - base] = 0xA5;
    buf[0x0810 - base] = 0x01;
    buf[0x0811 - base] = 0x85;

    // $0821..=$0824: 0x05A9_6285
    buf[0x0821 - base] = 0x85;
    buf[0x0822 - base] = 0x62;
    buf[0x0823 - base] = 0xA9;
    buf[0x0824 - base] = 0x05;

    // $0856..=$0857: dep_addr = $0900 (>= $0800 high-RAM depacker)
    buf[0x0856 - base] = 0x00;
    buf[0x0857 - base] = 0x09;

    // $0938..=$093B: 0x7585_70B1
    buf[0x0938 - base] = 0xB1;
    buf[0x0939 - base] = 0x70;
    buf[0x093A - base] = 0x85;
    buf[0x093B - base] = 0x75;

    // $0A0B..=$0A0E: 0x4A4A_4A4A
    buf[0x0A0B - base] = 0x4A;
    buf[0x0A0C - base] = 0x4A;
    buf[0x0A0D - base] = 0x4A;
    buf[0x0A0E - base] = 0x4A;

    // $0A88..=$0A89: entry_point = $2000
    buf[0x0A88 - base] = 0x00;
    buf[0x0A89 - base] = 0x20;

    // $0A90..=$0A91: length = $1000
    buf[0x0A90 - base] = 0x00;
    buf[0x0A91 - base] = 0x10;

    // $0A92..=$0A93: start_addr = $0801
    buf[0x0A92 - base] = 0x01;
    buf[0x0A93 - base] = 0x08;

    buf
}

const FIXTURE: [u8; 0x293] = build_fixture();

/// Validates Fast Packer v1.0 markers and computes `dep_addr`, `start_addr`, `end_addr`, and `entry_point`.
#[must_use]
fn resolve_fast_packer(
    image: &LoadedImage<'_>,
    _sig_match: &SignatureMatch,
) -> Option<ResolvedSpec> {
    if image.load_addr > 0x0801 || image.mem.len() <= 0x0A93 {
        return None;
    }
    if image.read_u32_le(0x080E) != Some(0x8501_A5D8)
        || image.read_u32_le(0x0821) != Some(0x05A9_6285)
        || image.read_u32_le(0x0938) != Some(0x7585_70B1)
        || image.read_u32_le(0x0A0B) != Some(0x4A4A_4A4A)
    {
        return None;
    }

    let _ = image.read_u16_le(0x0856)?;
    let _ = image.read_u16_le(0x0A88)?;
    let start = image.read_u16_le(0x0A92)?;
    let len = image.read_u16_le(0x0A90)?;
    // Treat zero length or overflow past `$1_0000` as absent so Phase 2 write tracking provides a
    // safe fallback. Exact `start + len == $1_0000` resolves cleanly to `$FFFF`.
    let excl = u32::from(start) + u32::from(len);
    let end_addr = if len == 0 || excl > 0x1_0000 {
        None
    } else {
        u16::try_from(excl - 1).ok().map(AddrSource::Const)
    };

    Some(ResolvedSpec::from_hints(Hints {
        end_addr,
        ..BASE_HINTS
    }))
}

/// Declarative specification for Fast Packer v1.0 (`lzss_compressor`).
pub static SPEC: PackerSpec = PackerSpec {
    id: "fast_packer",
    name: "Fast Packer v1.0",
    priority: 154,
    systems: &[TargetSystem::C64],
    load_addr_max: Some(0x0801),
    signatures: &SIGNATURES,
    hints: BASE_HINTS,
    patches: &[],
    resolve: Some(resolve_fast_packer),
    hooks: None,
    fixture: &FIXTURE,
    fixture_load_addr: 0x0801,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packers::PackerInfo;

    #[test]
    fn fast_packer_resolves_computed_end_and_high_ram_dep_addr() {
        let fixture = SPEC.build_fixture_memory();
        let view = LoadedImage::for_c64(&fixture.mem, fixture.load_addr, fixture.load_end);
        let sig_match = SPEC.detect(&view).expect("Fast Packer must match");
        let info = PackerInfo::resolve(&view, &sig_match);
        assert_eq!(info.name, "Fast Packer v1.0");
        assert_eq!(info.dep_addr, Some(0x0900));
        assert_eq!(info.start_addr, Some(0x0801));
        assert_eq!(info.end_addr, Some(0x1800));
        assert_eq!(info.entry_point, Some(0x2000));

        // Exact `$1_0000` boundary (`$8000 + $8000`) resolves `$FFFF`.
        let mut exact_mem = fixture.mem.clone();
        exact_mem[0x0A90..=0x0A91].copy_from_slice(&0x8000_u16.to_le_bytes());
        exact_mem[0x0A92..=0x0A93].copy_from_slice(&0x8000_u16.to_le_bytes());
        let exact_view = LoadedImage::for_c64(&exact_mem, fixture.load_addr, fixture.load_end);
        let exact_info = PackerInfo::resolve(
            &exact_view,
            &SPEC
                .detect(&exact_view)
                .expect("Fast Packer exact-end fixture should detect"),
        );
        assert_eq!(exact_info.end_addr, Some(0xFFFF));

        // Overflow past `$1_0000` (`$8001 + $8000`) drops `end_addr` to `None` instead of wrapping.
        let mut over_mem = fixture.mem.clone();
        over_mem[0x0A90..=0x0A91].copy_from_slice(&0x8000_u16.to_le_bytes());
        over_mem[0x0A92..=0x0A93].copy_from_slice(&0x8001_u16.to_le_bytes());
        let over_view = LoadedImage::for_c64(&over_mem, fixture.load_addr, fixture.load_end);
        assert_eq!(
            PackerInfo::resolve(
                &over_view,
                &SPEC
                    .detect(&over_view)
                    .expect("Fast Packer over-end fixture should detect")
            )
            .end_addr,
            None
        );

        // Zero length also leaves `end_addr = None`.
        let mut zero_mem = fixture.mem.clone();
        zero_mem[0x0A90..=0x0A91].copy_from_slice(&0x0000_u16.to_le_bytes());
        let zero_view = LoadedImage::for_c64(&zero_mem, fixture.load_addr, fixture.load_end);
        assert_eq!(
            PackerInfo::resolve(
                &zero_view,
                &SPEC
                    .detect(&zero_view)
                    .expect("Fast Packer zero-end fixture should detect")
            )
            .end_addr,
            None
        );

        // `SPEC.detect` rejects short buffers (`<= $0A93`) and `load_addr > $0801`.
        let short_view = LoadedImage::for_c64(&fixture.mem[..0x0A93], 0x0801, 0x0A93);
        assert!(SPEC.detect(&short_view).is_none());
        let high_load = LoadedImage::for_c64(&fixture.mem, 0x0802, fixture.load_end);
        assert!(SPEC.detect(&high_load).is_none());
    }
}
