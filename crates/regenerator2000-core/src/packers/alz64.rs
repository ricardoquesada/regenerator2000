//! Declarative [`PackerSpec`] for the ALZ64 family (`ALZ64/Quiss` and `ALZ64/Kabuto`).

use crate::packers::spec::{
    AddrSource, Anchor, Hints, LoadedImage, MemoryPatch, PackerSpec, PatchAddr, PointerGuard,
    ResolvedSpec, Signature, SignatureMatch,
};
use crate::sig;
use crate::state::TargetSystem;

/// Primary common signature at `$080C..=$080D` (`00 78`) shared by `ALZ64/Quiss` and `ALZ64/Kabuto`.
const COMMON_SIGNATURE: Signature = Signature {
    anchor: Anchor::Absolute(0x080C),
    pattern: sig![0x00, 0x78],
};

/// Pre-emulation memory patch for `ALZ64/Kabuto` (`ALZ64.c:20`: `mem[0x080B] = 0xA2`, `LDX #imm`).
const KABUTO_PATCHES: [MemoryPatch; 1] = [MemoryPatch {
    addr: PatchAddr::Absolute(0x080B),
    expected: None,
    bytes: &[0xA2],
}];

/// Shared base hints for the ALZ64 family (`ALZ64.c:22-25, 50-53`).
const BASE_HINTS: Hints = Hints {
    dep_addr: Some(AddrSource::Const(0x005E)),
    start_addr: Some(AddrSource::Const(0x080B)),
    end_addr: Some(AddrSource::PointerAfterRun {
        ptr_addr: 0x00CF,
        adjust: -1,
        guard: PointerGuard::NearDetectedEnd { slack: 512 },
    }),
    forced_entry: Some(AddrSource::Const(0x080B)),
    ..Hints::NONE
};

/// Builds a synthetic `ALZ64/Quiss` PRG payload loaded at `$0801`.
///
/// The payload encodes:
/// - Common signature bytes `00 78` at `$080C..=$080D`.
/// - 16-byte Quiss stub check across `$080B..=$082F`.
/// - Indirect stub base pointer `p = $0900` at `$0814..=$0815`.
/// - Tail marker `02 E6 C7 8D 01 08` at `p + $CB..=p + $D0` (`$09CB..=$09D0`, overriding `start_addr = $0801`).
/// - Final `JMP $2000` (`4C 00 20`) at `p + $F1..=p + $F3` (`$09F1..=$09F3`).
///
/// # Examples
/// ```
/// use regenerator2000_core::packers::alz64::build_quiss_fixture;
///
/// let prg = build_quiss_fixture();
/// assert_eq!(prg[0x080B - 0x0801], 0xA2);
/// ```
#[must_use]
pub const fn build_quiss_fixture() -> [u8; 0x01F4] {
    let mut buf = [0u8; 0x01F4];
    let base: usize = 0x0801;

    // 16-byte Quiss stub header checks ($080B..=$082F).
    buf[0x080B - base] = 0xA2;
    buf[0x080C - base] = 0x00;
    buf[0x080D - base] = 0x78;
    buf[0x080E - base] = 0xB5;
    buf[0x0819 - base] = 0xD0;
    buf[0x081A - base] = 0xF3;
    buf[0x081B - base] = 0xA2;
    buf[0x081C - base] = 0x03;
    buf[0x0824 - base] = 0xA2;
    buf[0x0825 - base] = 0x10;
    buf[0x0826 - base] = 0x89;
    buf[0x0827 - base] = 0x38;
    buf[0x082C - base] = 0xF1;
    buf[0x082D - base] = 0x4C;
    buf[0x082E - base] = 0x5E;
    buf[0x082F - base] = 0x00;

    // Indirect pointer p = $0900 at $0814..=$0815.
    buf[0x0814 - base] = 0x00;
    buf[0x0815 - base] = 0x09;

    // Tail signature [0x02, 0xE6, 0xC7, 0x8D, 0x01, 0x08] at p + 0xCB ($09CB..=$09D0).
    let p: usize = 0x0900;
    buf[p + 0xCB - base] = 0x02;
    buf[p + 0xCC - base] = 0xE6;
    buf[p + 0xCD - base] = 0xC7;
    buf[p + 0xCE - base] = 0x8D;
    buf[p + 0xCF - base] = 0x01;
    buf[p + 0xD0 - base] = 0x08;

    // Final JMP $2000 at p + 0xF1..=p + 0xF3 ($09F1..=$09F3).
    buf[p + 0xF1 - base] = 0x4C;
    buf[p + 0xF2 - base] = 0x00;
    buf[p + 0xF3 - base] = 0x20;

    buf
}

/// Builds a synthetic `ALZ64/Kabuto` PRG payload loaded at `$0801`.
///
/// The payload encodes:
/// - Common signature bytes `00 78` at `$080C..=$080D`.
/// - 15-byte Kabuto stub check across `$080C..=$0838`.
/// - Indirect stub base pointer `p = $0900` at `$0813..=$0814`.
/// - Tail marker `02 E6 C7 8D 01 08` at `p + $E6..=p + $EB` (`$09E6..=$09EB`, overriding `start_addr = $0801`).
/// - Final `JMP $2000` (`4C 00 20`) at `p + $FD..=p + $FF` (`$09FD..=$09FF`).
///
/// # Examples
/// ```
/// use regenerator2000_core::packers::alz64::build_kabuto_fixture;
///
/// let prg = build_kabuto_fixture();
/// assert_eq!(prg[0x080E - 0x0801], 0x86);
/// ```
#[must_use]
pub const fn build_kabuto_fixture() -> [u8; 0x0200] {
    let mut buf = [0u8; 0x0200];
    let base: usize = 0x0801;

    // 15-byte Kabuto stub header checks ($080C..=$0838).
    buf[0x080C - base] = 0x00;
    buf[0x080D - base] = 0x78;
    buf[0x080E - base] = 0x86;
    buf[0x0818 - base] = 0xCA;
    buf[0x0819 - base] = 0xD0;
    buf[0x081A - base] = 0xF7;
    buf[0x081B - base] = 0xCE;
    buf[0x0822 - base] = 0xD0;
    buf[0x0823 - base] = 0xEE;
    buf[0x0824 - base] = 0xA2;
    buf[0x0825 - base] = 0x03;
    buf[0x0835 - base] = 0xF1;
    buf[0x0836 - base] = 0x4C;
    buf[0x0837 - base] = 0x5E;
    buf[0x0838 - base] = 0x00;

    // Indirect pointer p = $0900 at $0813..=$0814.
    buf[0x0813 - base] = 0x00;
    buf[0x0814 - base] = 0x09;

    // Tail signature [0x02, 0xE6, 0xC7, 0x8D, 0x01, 0x08] at p + 0xE6 ($09E6..=$09EB).
    let p: usize = 0x0900;
    buf[p + 0xE6 - base] = 0x02;
    buf[p + 0xE7 - base] = 0xE6;
    buf[p + 0xE8 - base] = 0xC7;
    buf[p + 0xE9 - base] = 0x8D;
    buf[p + 0xEA - base] = 0x01;
    buf[p + 0xEB - base] = 0x08;

    // Final JMP $2000 at p + 0xFD..=p + 0xFF ($09FD..=$09FF).
    buf[p + 0xFD - base] = 0x4C;
    buf[p + 0xFE - base] = 0x00;
    buf[p + 0xFF - base] = 0x20;

    buf
}

const QUISS_FIXTURE: [u8; 0x01F4] = build_quiss_fixture();

/// Resolves `ALZ64/Quiss` or `ALZ64/Kabuto` metadata from a matched C64 image.
fn resolve_alz64(image: &LoadedImage<'_>, _sig_match: &SignatureMatch) -> Option<ResolvedSpec> {
    let mem = image.mem;
    let load_addr = image.load_addr;

    // ALZ64/Quiss (`ALZ64.c:40-66`: `RtAFrc = 1`, `Forced = 0x080B`, no memory patch)
    if mem.len() > 0x082F
        && load_addr <= 0x080B
        && mem[0x080B] == 0xA2
        && mem[0x080C] == 0x00
        && mem[0x080D] == 0x78
        && mem[0x080E] == 0xB5
        && mem[0x0819] == 0xD0
        && mem[0x081A] == 0xF3
        && mem[0x081B] == 0xA2
        && mem[0x081C] == 0x03
        && mem[0x0824] == 0xA2
        && mem[0x0825] == 0x10
        && mem[0x0826] == 0x89
        && mem[0x0827] == 0x38
        && mem[0x082C] == 0xF1
        && mem[0x082D] == 0x4C
        && mem[0x082E] == 0x5E
        && mem[0x082F] == 0x00
    {
        let p = u16::from_le_bytes([mem[0x0814], mem[0x0815]]) as usize;
        if p + 0xF3 < mem.len() && mem[p + 0xF1] == 0x4C {
            let mut start_addr = 0x080B;
            for q in (p + 0xCB)..=(p + 0xF0) {
                if q + 5 < mem.len() && mem[q..q + 4] == [0x02, 0xE6, 0xC7, 0x8D] {
                    start_addr = u16::from_le_bytes([mem[q + 4], mem[q + 5]]);
                    break;
                }
            }
            let entry_point = u16::from_le_bytes([mem[p + 0xF2], mem[p + 0xF3]]);
            return Some(ResolvedSpec::named(
                "ALZ64/Quiss",
                Hints {
                    start_addr: Some(AddrSource::Const(start_addr)),
                    entry_point: Some(AddrSource::Const(entry_point)),
                    exact_return: true,
                    ..BASE_HINTS
                },
            ));
        }
    }

    // ALZ64/Kabuto (`ALZ64.c:12-37`: patches `$080B = 0xA2`, `Forced = 0x080B`, `RtAFrc = 0`)
    if mem.len() > 0x0838
        && load_addr <= 0x080B
        && mem[0x080C] == 0x00
        && mem[0x080D] == 0x78
        && mem[0x080E] == 0x86
        && mem[0x0818] == 0xCA
        && mem[0x0819] == 0xD0
        && mem[0x081A] == 0xF7
        && mem[0x081B] == 0xCE
        && mem[0x0822] == 0xD0
        && mem[0x0823] == 0xEE
        && mem[0x0824] == 0xA2
        && mem[0x0825] == 0x03
        && mem[0x0835] == 0xF1
        && mem[0x0836] == 0x4C
        && mem[0x0837] == 0x5E
        && mem[0x0838] == 0x00
    {
        let p = u16::from_le_bytes([mem[0x0813], mem[0x0814]]) as usize;
        if p + 0xFF < mem.len() && mem[p + 0xFD] == 0x4C {
            let mut start_addr = 0x080B;
            for q in (p + 0xE6)..=(p + 0xF0) {
                if q + 5 < mem.len() && mem[q..q + 4] == [0x02, 0xE6, 0xC7, 0x8D] {
                    start_addr = u16::from_le_bytes([mem[q + 4], mem[q + 5]]);
                    break;
                }
            }
            let entry_point = u16::from_le_bytes([mem[p + 0xFE], mem[p + 0xFF]]);
            return Some(ResolvedSpec::with_patches(
                "ALZ64/Kabuto",
                Hints {
                    start_addr: Some(AddrSource::Const(start_addr)),
                    entry_point: Some(AddrSource::Const(entry_point)),
                    ..BASE_HINTS
                },
                &KABUTO_PATCHES,
            ));
        }
    }

    None
}

/// Declarative specification for `ALZ64/Quiss` and `ALZ64/Kabuto` (priority `100`).
pub static SPEC: PackerSpec = PackerSpec {
    id: "alz64",
    name: "ALZ64/Quiss",
    priority: 100,
    systems: &[TargetSystem::C64],
    load_addr_max: Some(0x080B),
    signatures: &[COMMON_SIGNATURE],
    hints: BASE_HINTS,
    patches: &[],
    resolve: Some(resolve_alz64),
    hooks: None,
    fixture: &QUISS_FIXTURE,
    fixture_load_addr: 0x0801,
};
