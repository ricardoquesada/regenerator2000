//! ZX02 SFX packer specification (found in Shadow cracks).
//!
//! Targets [`TargetSystem::C64`] images with `load_addr <= $0801`. Matches the stub at
//! `$080F`, enters at `$080D`, runs the `$0334` depacker, reads `entry_point` (`$0841`) and
//! `start_addr` (`$0846`), and derives inclusive `end_addr` from `$0081 - 1`.

use crate::state::types::TargetSystem;

use super::spec::{
    AddrSource, Anchor, Hints, LoadedImage, PackerSpec, PointerGuard, ResolvedSpec, Signature,
    SignatureMatch, sig,
};

/// Primary ZX02 4-byte signature at `$080F` (`#$34` operand of `LDA #$34`, `STA $01`, `LDX` opcode).
pub const ZX02_SIGNATURE: Signature = Signature {
    anchor: Anchor::Absolute(0x080F),
    pattern: sig![0x34, 0x85, 0x01, 0xA2],
};

const SIGNATURES: [Signature; 1] = [ZX02_SIGNATURE];

const BASE_HINTS: Hints = Hints {
    dep_addr: Some(AddrSource::Const(0x0334)),
    start_addr: Some(AddrSource::AbsoluteLe(0x0846)),
    end_addr: Some(AddrSource::PointerAfterRun {
        ptr_addr: 0x0081,
        adjust: -1,
        guard: PointerGuard::NearDetectedEnd { slack: 512 },
    }),
    entry_point: Some(AddrSource::AbsoluteLe(0x0841)),
    forced_entry: Some(AddrSource::Const(0x080D)),
    ..Hints::NONE
};

/// Builds a synthetic `ZX02` raw payload (no 2-byte PRG header) for `$0801..=$08CA` (202 bytes).
#[must_use]
pub const fn build_fixture() -> [u8; 0xCA] {
    let mut buf = [0u8; 0xCA];
    let base: usize = 0x0801;

    // $080F..=$0812: 0xA201_8534
    buf[0x080F - base] = 0x34;
    buf[0x0810 - base] = 0x85;
    buf[0x0811 - base] = 0x01;
    buf[0x0812 - base] = 0xA2;

    // $0834..=$0837: 0x0334_4CEE
    buf[0x0834 - base] = 0xEE;
    buf[0x0835 - base] = 0x4C;
    buf[0x0836 - base] = 0x34;
    buf[0x0837 - base] = 0x03;

    // $0841..=$0842: entry_point = $1000
    buf[0x0841 - base] = 0x00;
    buf[0x0842 - base] = 0x10;

    // $0846..=$0847: start_addr = $0801
    buf[0x0846 - base] = 0x01;
    buf[0x0847 - base] = 0x08;

    // $085B..=$085E: 0xE602_D081
    buf[0x085B - base] = 0x81;
    buf[0x085C - base] = 0xD0;
    buf[0x085D - base] = 0x02;
    buf[0x085E - base] = 0xE6;

    // $08C7..=$08CA: 0x0860_E7B0
    buf[0x08C7 - base] = 0xB0;
    buf[0x08C8 - base] = 0xE7;
    buf[0x08C9 - base] = 0x60;
    buf[0x08CA - base] = 0x08;

    buf
}

const FIXTURE: [u8; 0xCA] = build_fixture();

/// Validates ZX02 secondary markers and readable operands (`$0841`, `$0846`).
#[must_use]
fn resolve_zx02(image: &LoadedImage<'_>, _sig_match: &SignatureMatch) -> Option<ResolvedSpec> {
    if image.load_addr > 0x0801 || image.mem.len() <= 0x08CA {
        return None;
    }
    // Verify signature (`$080F`), `JMP $0334` depacker entry (`$0835`), `$0081` pointer loop (`$085B`),
    // and tail jump marker (`$08C7`), plus readable `entry_point` (`$0841`) and `start_addr` (`$0846`).
    if image.read_u32_le(0x080F) != Some(0xA201_8534)
        || image.read_u32_le(0x0834) != Some(0x0334_4CEE)
        || image.read_u32_le(0x085B) != Some(0xE602_D081)
        || image.read_u32_le(0x08C7) != Some(0x0860_E7B0)
    {
        return None;
    }
    let _ = image.read_u16_le(0x0841)?;
    let _ = image.read_u16_le(0x0846)?;
    Some(ResolvedSpec::from_hints(BASE_HINTS))
}

/// Declarative specification for ZX02 SFX.
pub static SPEC: PackerSpec = PackerSpec {
    id: "zx02",
    name: "ZX02",
    priority: 125,
    systems: &[TargetSystem::C64],
    load_addr_max: Some(0x0801),
    signatures: &SIGNATURES,
    hints: BASE_HINTS,
    patches: &[],
    resolve: Some(resolve_zx02),
    hooks: None,
    fixture: &FIXTURE,
    fixture_load_addr: 0x0801,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packers::PackerInfo;

    #[test]
    fn zx02_resolves_hints_and_rejects_short_or_corrupted_images() {
        let fixture = SPEC.build_fixture_memory();
        let view = LoadedImage::for_c64(&fixture.mem, fixture.load_addr, fixture.load_end);
        let sig_match = SPEC.detect(&view).expect("ZX02 fixture must match");
        let info = PackerInfo::resolve(&view, &sig_match);
        assert_eq!(info.name, "ZX02");
        assert_eq!(info.dep_addr, Some(0x0334));
        assert_eq!(info.start_addr, Some(0x0801));
        assert_eq!(info.entry_point, Some(0x1000));
        assert_eq!(info.end_addr_ptr, Some(0x0081));
        assert_eq!(
            sig_match.resolved.hints.forced_entry,
            Some(AddrSource::Const(0x080D))
        );

        let short_view = LoadedImage::for_c64(&fixture.mem[..0x08CA], 0x0801, 0x08CA);
        assert!(SPEC.detect(&short_view).is_none());
    }
}
