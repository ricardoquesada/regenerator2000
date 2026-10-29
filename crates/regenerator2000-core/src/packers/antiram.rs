//! Declarative [`PackerSpec`] for the Antiram family (`Antiram Packer v1.0` and `Antiram Packer v2.0`).

use crate::packers::spec::{
    AddrSource, Anchor, Hints, LoadedImage, MemoryPatch, PackerSpec, PatchAddr, PointerGuard,
    ResolvedSpec, Signature, SignatureMatch,
};
use crate::sig;
use crate::state::TargetSystem;

/// Primary common signature at `$080D..=$0810` (`78 A0 00 B9`) shared by v1.0 and v2.0.
const COMMON_SIGNATURE: Signature = Signature {
    anchor: Anchor::Absolute(0x080D),
    pattern: sig![0x78, 0xA0, 0x00, 0xB9],
};

/// Conditional pre-emulation patch for `Antiram Packer v1.0` (`Antiram.c:24-27`:
/// replaces `JSR` `0x20` at `$0904` with `BIT absolute` `0x2C`).
const V1_PATCHES: [MemoryPatch; 1] = [MemoryPatch {
    addr: PatchAddr::Absolute(0x0904),
    expected: Some(0x20),
    bytes: &[0x2C],
}];

/// Conditional pre-emulation patch for `Antiram Packer v2.0` (`Antiram.c:47-50`:
/// replaces `JSR` `0x20` at `$0911` with `BIT absolute` `0x2C`).
const V2_PATCHES: [MemoryPatch; 1] = [MemoryPatch {
    addr: PatchAddr::Absolute(0x0911),
    expected: Some(0x20),
    bytes: &[0x2C],
}];

/// Shared base hints for the Antiram family (`Antiram.c:22-23, 31-33, 45-46, 55-57`).
const BASE_HINTS: Hints = Hints {
    dep_addr: Some(AddrSource::Const(0xFF00)),
    start_addr: Some(AddrSource::BasicStart(0)),
    end_addr: Some(AddrSource::PointerAfterRun {
        ptr_addr: 0x002D,
        adjust: -1,
        guard: PointerGuard::NearDetectedEnd { slack: 512 },
    }),
    forced_entry: Some(AddrSource::Const(0x080D)),
    exact_return: true,
    ..Hints::NONE
};

/// Builds a synthetic `Antiram Packer v1.0` PRG payload loaded at `$0801` (`len > $0950` in memory).
///
/// The payload encodes:
/// - `$080D..=$0810`: `78 A0 00 B9` (`0xB900_A078`).
/// - `$081D..=$0820`: `4C 00 FF C6` (`0xC6FF_004C`).
/// - `$082D..=$0830`: `A5 AF C9 08` (`0x08C9_AFA5`).
/// - `$08ED..=$08F0`: `03 4C 35 FF` (`0xFF35_4C03`).
/// - `$0904..=$0906`: `4C 34 12` (`JMP $1234`).
///
/// # Examples
/// ```
/// use regenerator2000_core::packers::antiram::build_v1_fixture;
///
/// let prg = build_v1_fixture();
/// assert_eq!(prg[0x080D - 0x0801], 0x78);
/// ```
#[must_use]
pub const fn build_v1_fixture() -> [u8; 0x0152] {
    let mut buf = [0u8; 0x0152];
    let base: usize = 0x0801;

    // $080D..=$0810: 0xB900_A078
    buf[0x080D - base] = 0x78;
    buf[0x080E - base] = 0xA0;
    buf[0x080F - base] = 0x00;
    buf[0x0810 - base] = 0xB9;

    // $081D..=$0820: 0xC6FF_004C
    buf[0x081D - base] = 0x4C;
    buf[0x081E - base] = 0x00;
    buf[0x081F - base] = 0xFF;
    buf[0x0820 - base] = 0xC6;

    // $082D..=$0830: 0x08C9_AFA5
    buf[0x082D - base] = 0xA5;
    buf[0x082E - base] = 0xAF;
    buf[0x082F - base] = 0xC9;
    buf[0x0830 - base] = 0x08;

    // $08ED..=$08F0: 0xFF35_4C03
    buf[0x08ED - base] = 0x03;
    buf[0x08EE - base] = 0x4C;
    buf[0x08EF - base] = 0x35;
    buf[0x08F0 - base] = 0xFF;

    // $0904..=$0906: JMP $1234
    buf[0x0904 - base] = 0x4C;
    buf[0x0905 - base] = 0x34;
    buf[0x0906 - base] = 0x12;

    buf
}

/// Builds a synthetic `Antiram Packer v2.0` PRG payload loaded at `$0801` (`len > $0950` in memory).
///
/// The payload encodes:
/// - `$080D..=$0810`: `78 A0 00 B9` (`0xB900_A078`).
/// - `$081D..=$0820`: `4C 00 FF A9` (`0xA9FF_004C`).
/// - `$082D..=$0830`: `AE 91 62 C8` (`0xC862_91AE`).
/// - `$08ED..=$08F0`: `E6 2E CA D0` (`0xD0CA_2EE6`).
/// - `$0911..=$0913`: `4C 18 08` (`JMP $0818`).
///
/// # Examples
/// ```
/// use regenerator2000_core::packers::antiram::build_v2_fixture;
///
/// let prg = build_v2_fixture();
/// assert_eq!(prg[0x080D - 0x0801], 0x78);
/// ```
#[must_use]
pub const fn build_v2_fixture() -> [u8; 0x0152] {
    let mut buf = [0u8; 0x0152];
    let base: usize = 0x0801;

    // $080D..=$0810: 0xB900_A078
    buf[0x080D - base] = 0x78;
    buf[0x080E - base] = 0xA0;
    buf[0x080F - base] = 0x00;
    buf[0x0810 - base] = 0xB9;

    // $081D..=$0820: 0xA9FF_004C
    buf[0x081D - base] = 0x4C;
    buf[0x081E - base] = 0x00;
    buf[0x081F - base] = 0xFF;
    buf[0x0820 - base] = 0xA9;

    // $082D..=$0830: 0xC862_91AE
    buf[0x082D - base] = 0xAE;
    buf[0x082E - base] = 0x91;
    buf[0x082F - base] = 0x62;
    buf[0x0830 - base] = 0xC8;

    // $08ED..=$08F0: 0xD0CA_2EE6
    buf[0x08ED - base] = 0xE6;
    buf[0x08EE - base] = 0x2E;
    buf[0x08EF - base] = 0xCA;
    buf[0x08F0 - base] = 0xD0;

    // $0911..=$0913: JMP $0818
    buf[0x0911 - base] = 0x4C;
    buf[0x0912 - base] = 0x18;
    buf[0x0913 - base] = 0x08;

    buf
}

const V2_FIXTURE: [u8; 0x0152] = build_v2_fixture();

/// Reads a little-endian `u32` at `offset` in `mem`.
const fn read_u32_le(mem: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        mem[offset],
        mem[offset + 1],
        mem[offset + 2],
        mem[offset + 3],
    ])
}

/// Resolves `Antiram Packer v1.0` or `Antiram Packer v2.0` metadata from a matched C64 image.
fn resolve_antiram(image: &LoadedImage<'_>, _sig_match: &SignatureMatch) -> Option<ResolvedSpec> {
    let mem = image.mem;
    if mem.len() <= 0x0950 {
        return None;
    }

    if read_u32_le(mem, 0x080D) != 0xB900_A078 {
        return None;
    }

    // Antiram Packer v1.0
    if read_u32_le(mem, 0x081D) == 0xC6FF_004C
        && read_u32_le(mem, 0x082D) == 0x08C9_AFA5
        && read_u32_le(mem, 0x08ED) == 0xFF35_4C03
    {
        let mut p: usize = 0x0904;
        if mem[p] == 0x20 {
            p += 3;
        }
        let ret_addr = u16::from_le_bytes([mem[p + 1], mem[p + 2]]);
        return Some(ResolvedSpec::with_patches(
            "Antiram Packer v1.0",
            Hints {
                entry_point: Some(AddrSource::Const(ret_addr)),
                ..BASE_HINTS
            },
            &V1_PATCHES,
        ));
    }

    // Antiram Packer v2.0
    if read_u32_le(mem, 0x081D) == 0xA9FF_004C
        && read_u32_le(mem, 0x082D) == 0xC862_91AE
        && read_u32_le(mem, 0x08ED) == 0xD0CA_2EE6
    {
        let mut p: usize = 0x0911;
        if mem[p] == 0x20 {
            p += 3;
        }
        let ret_addr = u16::from_le_bytes([mem[p + 1], mem[p + 2]]);
        return Some(ResolvedSpec::with_patches(
            "Antiram Packer v2.0",
            Hints {
                entry_point: Some(AddrSource::Const(ret_addr)),
                ..BASE_HINTS
            },
            &V2_PATCHES,
        ));
    }

    None
}

/// Declarative specification for `Antiram Packer v1.0` and `Antiram Packer v2.0` (priority `200`).
pub static SPEC: PackerSpec = PackerSpec {
    id: "antiram",
    name: "Antiram Packer v2.0",
    priority: 200,
    systems: &[TargetSystem::C64],
    load_addr_max: None,
    signatures: &[COMMON_SIGNATURE],
    hints: BASE_HINTS,
    patches: &V2_PATCHES,
    resolve: Some(resolve_antiram),
    hooks: None,
    fixture: &V2_FIXTURE,
    fixture_load_addr: 0x0801,
};
