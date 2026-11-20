//! Compress64k / Hermit (`v1.1`, `v1.2`, and `v1.3 (RLE)`) packer specification.
//!
//! Targets [`TargetSystem::C64`] images with `load_addr <= $0801`. Matches at `$080E`,
//! enters at `$080E` for the `$0100` depacker, and resolves `end_addr` from `$00FE - 1`
//! (`v1.1`/`v1.2`) or `mem[$00FE] + X - 1` via [`AddrSource::PointerRegAfterRun`] (`v1.3 (RLE)`).

use crate::state::TargetSystem;

use super::spec::{
    AddrSource, Anchor, CpuReg, Hints, LoadedImage, PackerSpec, PointerGuard, ResolvedSpec,
    Signature, SignatureMatch, sig,
};

/// Primary Compress64k signature at `$080E` matching v1.1 (`LDX #$E0, LDA $xx1B,X`)
/// and v1.2/v1.3 (`LDA #$38, STA $01`).
const SIGNATURES: [Signature; 1] = [Signature {
    anchor: Anchor::Absolute(0x080E),
    pattern: sig![[0xA2, 0xA9], [0xE0, 0x38], [0xBD, 0x85], [0x1B, 0x01]],
}];

const V11_V12_BASE_HINTS: Hints = Hints {
    dep_addr: Some(AddrSource::Const(0x0100)),
    end_addr: Some(AddrSource::PointerAfterRun {
        ptr_addr: 0x00FE,
        adjust: -1,
        guard: PointerGuard::NearDetectedEnd { slack: 512 },
    }),
    forced_entry: Some(AddrSource::Const(0x080E)),
    ..Hints::NONE
};

const BASE_HINTS: Hints = Hints {
    start_addr: Some(AddrSource::AbsoluteLe(0x0839)),
    end_addr: Some(AddrSource::PointerRegAfterRun {
        ptr_addr: 0x00FE,
        reg: CpuReg::X,
        adjust: -1,
        guard: PointerGuard::NearDetectedEnd { slack: 512 },
    }),
    entry_point: Some(AddrSource::AbsoluteLe(0x08FD)),
    ..V11_V12_BASE_HINTS
};

/// Builds a synthetic `Compress64k/Hermit v1.3 (RLE)` raw payload (no 2-byte PRG header)
/// for `$0801..=$08FE` (254 bytes).
#[must_use]
pub const fn build_v13_fixture() -> [u8; 0xFE] {
    let mut buf = [0u8; 0xFE];
    let base: usize = 0x0801;

    // $080E..=$0811: 0x0185_38A9
    buf[0x080E - base] = 0xA9;
    buf[0x080F - base] = 0x38;
    buf[0x0810 - base] = 0x85;
    buf[0x0811 - base] = 0x01;

    // $0817..=$081A: 0xCA00_FB9D
    buf[0x0817 - base] = 0x9D;
    buf[0x0818 - base] = 0xFB;
    buf[0x0819 - base] = 0x00;
    buf[0x081A - base] = 0xCA;

    // $0839..=$083A: start_addr = $0801
    buf[0x0839 - base] = 0x01;
    buf[0x083A - base] = 0x08;

    // $083F..=$0842: 0x8DFC_E538
    buf[0x083F - base] = 0x38;
    buf[0x0840 - base] = 0xE5;
    buf[0x0841 - base] = 0xFC;
    buf[0x0842 - base] = 0x8D;

    // $08EF..=$08F2: 0xA5FF_C602
    buf[0x08EF - base] = 0x02;
    buf[0x08F0 - base] = 0xC6;
    buf[0x08F1 - base] = 0xFF;
    buf[0x08F2 - base] = 0xA5;

    // $08FD..=$08FE: entry_point = $1000
    buf[0x08FD - base] = 0x00;
    buf[0x08FE - base] = 0x10;

    buf
}

const FIXTURE: [u8; 0xFE] = build_v13_fixture();

/// Resolves Compress64k/Hermit `v1.1`, `v1.2`, or `v1.3 (RLE)` at `$080E`.
#[must_use]
fn resolve_comp64k(image: &LoadedImage<'_>, _sig_match: &SignatureMatch) -> Option<ResolvedSpec> {
    if image.load_addr > 0x0801 {
        return None;
    }

    // v1.1 matches `LDX #$E0; LDA $xx1B,X`. Reads `start_addr` at `$081E` and `entry_point` at `$08FA`.
    if image.mem.len() > 0x08FB
        && image.read_u32_le(0x080E) == Some(0x1BBD_E0A2)
        && image.read_u32_le(0x0813) == Some(0xCA00_FB9D)
        && image.read_u32_le(0x0840) == Some(0xB04A_48B0)
        && image.read_u32_le(0x08E9) == Some(0x4A4A_4A4A)
    {
        let _ = image.read_u16_le(0x081E)?;
        let _ = image.read_u16_le(0x08FA)?;
        return Some(ResolvedSpec::named(
            "Compress64k/Hermit v1.1",
            Hints {
                start_addr: Some(AddrSource::AbsoluteLe(0x081E)),
                entry_point: Some(AddrSource::AbsoluteLe(0x08FA)),
                ..V11_V12_BASE_HINTS
            },
        ));
    }

    // v1.2 matches `LDA #$38; STA $01`. Reads `start_addr` at `$0839` and `entry_point` at `$08FD`.
    if image.mem.len() > 0x08FE
        && image.read_u32_le(0x080E) == Some(0x0185_38A9)
        && image.read_u32_le(0x0817) == Some(0xCA00_FB9D)
        && image.read_u32_le(0x0840) == Some(0xB04A_C802)
        && image.read_u32_le(0x08EF) == Some(0xFFE6_0590)
    {
        let _ = image.read_u16_le(0x0839)?;
        let _ = image.read_u16_le(0x08FD)?;
        return Some(ResolvedSpec::named(
            "Compress64k/Hermit v1.2",
            Hints {
                start_addr: Some(AddrSource::AbsoluteLe(0x0839)),
                entry_point: Some(AddrSource::AbsoluteLe(0x08FD)),
                ..V11_V12_BASE_HINTS
            },
        ));
    }

    // v1.3 (RLE): end_addr equals `mem[$00FE] + X - 1` (`PointerRegAfterRun` with `CpuReg::X`).
    if image.mem.len() > 0x08FE
        && image.read_u32_le(0x080E) == Some(0x0185_38A9)
        && image.read_u32_le(0x0817) == Some(0xCA00_FB9D)
        && image.read_u32_le(0x083F) == Some(0x8DFC_E538)
        && image.read_u32_le(0x08EF) == Some(0xA5FF_C602)
    {
        let _ = image.read_u16_le(0x0839)?;
        let _ = image.read_u16_le(0x08FD)?;
        return Some(ResolvedSpec::named(
            "Compress64k/Hermit v1.3 (RLE)",
            BASE_HINTS,
        ));
    }

    None
}

/// Declarative specification for Compress64k / Hermit (`v1.1`, `v1.2`, and `v1.3 (RLE)`).
pub static SPEC: PackerSpec = PackerSpec {
    id: "comp64k",
    name: "Compress64k/Hermit v1.3 (RLE)",
    priority: 152,
    systems: &[TargetSystem::C64],
    load_addr_max: Some(0x0801),
    signatures: &SIGNATURES,
    hints: BASE_HINTS,
    patches: &[],
    resolve: Some(resolve_comp64k),
    hooks: None,
    fixture: &FIXTURE,
    fixture_load_addr: 0x0801,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packers::{PackerInfo, Registers};

    #[test]
    fn comp64k_detects_all_three_versions() {
        // v1.3 (RLE) fixture
        let v13 = SPEC.build_fixture_memory();
        let v13_view = LoadedImage::for_c64(&v13.mem, v13.load_addr, v13.load_end);
        let m13 = SPEC.detect(&v13_view).expect("v1.3 must match");
        assert_eq!(m13.spec_name, "Compress64k/Hermit v1.3 (RLE)");
        assert_eq!(
            m13.resolved.hints.end_addr,
            Some(AddrSource::PointerRegAfterRun {
                ptr_addr: 0x00FE,
                reg: CpuReg::X,
                adjust: -1,
                guard: PointerGuard::NearDetectedEnd { slack: 512 },
            })
        );

        // v1.2
        let mut v12_mem = vec![0u8; 0x1_0000];
        v12_mem[0x080E..=0x0811].copy_from_slice(&[0xA9, 0x38, 0x85, 0x01]);
        v12_mem[0x0817..=0x081A].copy_from_slice(&[0x9D, 0xFB, 0x00, 0xCA]);
        v12_mem[0x0839..=0x083A].copy_from_slice(&0x0801_u16.to_le_bytes());
        v12_mem[0x0840..=0x0843].copy_from_slice(&[0x02, 0xC8, 0x4A, 0xB0]);
        v12_mem[0x08EF..=0x08F2].copy_from_slice(&[0x90, 0x05, 0xE6, 0xFF]);
        v12_mem[0x08FD..=0x08FE].copy_from_slice(&0x2000_u16.to_le_bytes());
        let v12_view = LoadedImage::for_c64(&v12_mem, 0x0801, 0x0900);
        let m12 = SPEC.detect(&v12_view).expect("v1.2 must match");
        assert_eq!(m12.spec_name, "Compress64k/Hermit v1.2");
        assert_eq!(
            PackerInfo::resolve(&v12_view, &m12).entry_point,
            Some(0x2000)
        );

        // v1.1
        let mut v11_mem = vec![0u8; 0x1_0000];
        v11_mem[0x080E..=0x0811].copy_from_slice(&[0xA2, 0xE0, 0xBD, 0x1B]);
        v11_mem[0x0813..=0x0816].copy_from_slice(&[0x9D, 0xFB, 0x00, 0xCA]);
        v11_mem[0x081E..=0x081F].copy_from_slice(&0x0801_u16.to_le_bytes());
        v11_mem[0x0840..=0x0843].copy_from_slice(&[0xB0, 0x48, 0x4A, 0xB0]);
        v11_mem[0x08E9..=0x08EC].copy_from_slice(&[0x4A, 0x4A, 0x4A, 0x4A]);
        v11_mem[0x08FA..=0x08FB].copy_from_slice(&0x3000_u16.to_le_bytes());
        let v11_view = LoadedImage::for_c64(&v11_mem, 0x0801, 0x0900);
        let m11 = SPEC.detect(&v11_view).expect("v1.1 must match");
        assert_eq!(m11.spec_name, "Compress64k/Hermit v1.1");
        assert_eq!(
            PackerInfo::resolve(&v11_view, &m11).entry_point,
            Some(0x3000)
        );

        // Short buffer (`<= $08FE` for v1.3, `<= $08FB` for v1.1), `load_addr > $0801`, and
        // `$1_0000` (`$FFFF`) boundary resolution on v1.3 `PointerRegAfterRun`:
        let short_v13 = LoadedImage::for_c64(&v13.mem[..0x08FE], 0x0801, 0x08FE);
        assert!(SPEC.detect(&short_v13).is_none());
        let short_v11 = LoadedImage::for_c64(&v11_mem[..0x08FB], 0x0801, 0x08FB);
        assert!(SPEC.detect(&short_v11).is_none());
        let high_load = LoadedImage::for_c64(&v13.mem, 0x0802, v13.load_end);
        assert!(SPEC.detect(&high_load).is_none());

        let mut end_mem = vec![0u8; 0x1_0000];
        end_mem[0x00FE..=0x00FF].copy_from_slice(&0xFFF0_u16.to_le_bytes());
        let regs = Registers {
            a: 0,
            x: 0x10,
            y: 0,
            sp: 0xF6,
        };
        assert_eq!(
            m13.resolved
                .hints
                .end_addr
                .expect("Comp64K spec should configure end_addr")
                .resolve_post_run_with_regs(&end_mem, 0x0801, 0xFE00, regs),
            Some(0xFFFF)
        );
    }
}
