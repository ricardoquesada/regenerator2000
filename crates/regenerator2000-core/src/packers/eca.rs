//! ECA Compactor packer specification.

use crate::state::types::TargetSystem;

use super::spec::{
    AddrSource, Anchor, Hints, LoadedImage, PackerSpec, PointerGuard, ResolvedSpec, Signature,
    SignatureMatch, sig,
};

/// Default static hints for ECA Compactor (`dep_addr: $0100`, `start_addr: BasicStart(-1)` = `$0800` on C64 per Ruling B4).
pub const DEFAULT_HINTS: Hints = Hints {
    dep_addr: Some(AddrSource::Const(0x0100)),
    start_addr: Some(AddrSource::BasicStart(-1)),
    ..Hints::NONE
};

/// Primary ECA Compactor signature: scans `p in 0x080D..=0x0830` for the 4-byte marker
/// `[0x2A, 0x2A, 0x2A, 0x2A]` (`"****"`) at `p + 0x3A..p + 0x3E` (total pattern length `0x3E = 62` bytes).
const SIGNATURES: [Signature; 1] = [Signature {
    anchor: Anchor::Window(0x080D..=0x0830),
    pattern: sig![
        __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __,
        __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __,
        __, __, __, __, __, __, __, __, __, __, __, __, 0x2A, 0x2A, 0x2A, 0x2A
    ],
}];

/// Returns `true` if `mem` matches one of the 4 ECA Compactor stub variants at position `p`.
#[must_use]
fn matches_eca_variant(image: &LoadedImage<'_>, p: usize) -> bool {
    let mem = image.mem;

    // Check 1: Standard ECA (position-dependent `w08 == 0x2D9D0032 + p`)
    if let (Some(w08), Some(w0c)) = (image.read_u32_le(p + 8), image.read_u32_le(p + 12))
        && w08 == 0x2D9D_0032_u32.wrapping_add(p as u32)
        && w0c == 0xF710_CA00
        && let (Some(w00), Some(w04)) = (image.read_u32_le(p), image.read_u32_le(p + 4))
    {
        if ((w00 & 0xF4FF_F000) == 0x8434_A000 && w04 == 0xBD05_A201)
            || ((w00 & 0xFFFF_FF00) == 0x04A2_7800 && w04 == 0xBDE8_0186)
        {
            return true;
        }
        if p >= 3
            && let Some(wm3) = image.read_u32_le(p - 3)
            && (((wm3 & 0xFFFF_FF00) == 0x04A2_7800 && w04 == 0xBDE8_0186) || wm3 == 0x8D00_A978)
        {
            return true;
        }
    }

    // Check 2: Variant 2
    if p >= 3
        && let Some(w02) = image.read_u32_le(p + 2)
        && w02 == 0x8534_A978
        && mem.get(p - 3) == Some(&0xA0)
    {
        return true;
    }

    // Check 3: FDT
    if let (Some(w03), Some(w0a)) = (image.read_u32_le(p + 3), image.read_u32_le(p + 10))
        && w03 == 0x8604_A278
        && w0a == 0x2D95_0842
    {
        return true;
    }

    // Check 4: Decibel hacks
    if p >= 6
        && let (Some(w00), Some(wm6)) = (image.read_u32_le(p), image.read_u32_le(p - 6))
        && w00 == 0x9D08_5EBD
        && wm6 == 0x0185_34A9
    {
        return true;
    }

    false
}

/// Resolves the ECA Compactor [`Hints`] at position `p`, using `Const(0x0100)` as the
/// `dep_addr` fallback and `BasicStart(-1)` (`$0800` on C64 per Ruling B4) as the
/// `start_addr` fallback when `mem` is shorter than `p + 0x32` / `p + 0x34`.
#[must_use]
pub fn resolve_hints_at(image: &LoadedImage<'_>, p: usize) -> Hints {
    let mem = image.mem;

    let mut entry_point = None;
    for q in 0xD6..0xDE {
        if let Some(op) = image.read_u8(p + q)
            && (op == 0x20 || op == 0x4C)
            && let Some(target) = image.read_u16_le(p + q + 1)
            && !matches!(target, 0xA659 | 0xFF81 | 0xE3BF | 0xE5A0 | 0xE518)
        {
            entry_point = Some(AddrSource::Const(target));
            break;
        }
    }

    let dep_addr = match image.read_u16_le(p + 0x30) {
        Some(addr) => Some(AddrSource::Const(addr)),
        None => Some(AddrSource::Const(0x0100)),
    };

    let start_addr = match image.read_u16_le(p + 0x32) {
        Some(addr) => Some(AddrSource::Const(addr)),
        None => Some(AddrSource::BasicStart(-1)),
    };

    let mut end_addr = None;
    for q in 0xED..0x108 {
        if p + q + 4 < mem.len() && mem[p + q..p + q + 4] == [0xD0, 0xF7, 0x18, 0xA5] {
            end_addr = Some(AddrSource::PointerAfterRun {
                ptr_addr: u16::from(mem[p + q + 4]),
                adjust: -1,
                guard: PointerGuard::NearDetectedEnd { slack: 512 },
            });
            break;
        }
    }

    Hints {
        dep_addr,
        start_addr,
        end_addr,
        entry_point,
        ..Hints::NONE
    }
}

/// Per-position `resolve` function for ECA Compactor (`T3.0.14`, `T3.2.3`, `T3.2.4`).
///
/// Returning `None` at candidate `p` causes [`PackerSpec::detect`] to continue scanning
/// the remaining positions in `0x080D..=0x0830`.
#[must_use]
fn resolve_eca(image: &LoadedImage<'_>, sig_match: &SignatureMatch) -> Option<ResolvedSpec> {
    if image.mem.len() < 0x0950 {
        return None;
    }
    let p = usize::from(sig_match.anchor_addr);
    if !matches_eca_variant(image, p) {
        return None;
    }
    Some(ResolvedSpec::from_hints(resolve_hints_at(image, p)))
}

/// Synthetic self-test fixture loaded at `$080D` (`p = $080D`, 62 bytes covering `$080D..=$084A`):
/// - `p + 0..p + 4`: `w00 = 0x8434A000` (`00 A0 34 84`)
/// - `p + 4..p + 8`: `w04 = 0xBD05A201` (`01 A2 05 BD`)
/// - `p + 8..p + 12`: `w08 = 0x2D9D0032 + 0x080D = 0x2D9D083F` (`3F 08 9D 2D`)
/// - `p + 12..p + 16`: `w0c = 0xF710CA00` (`00 CA 10 F7`)
/// - `p + 0x30..=p + 0x31`: `dep_addr = $0100` (`00 01`)
/// - `p + 0x32..=p + 0x33`: `start_addr = $0800` (`00 08`)
/// - `p + 0x3A..p + 0x3E`: `[0x2A, 0x2A, 0x2A, 0x2A]`
const FIXTURE: [u8; 0x3E] = [
    // +0x00..+0x0F (16 bytes: Standard ECA Check 1 at p = $080D)
    0x00, 0xA0, 0x34, 0x84, 0x01, 0xA2, 0x05, 0xBD, 0x3F, 0x08, 0x9D, 0x2D, 0x00, 0xCA, 0x10, 0xF7,
    // +0x10..+0x2F (32 zero bytes)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // +0x30..+0x33 (dep_addr = $0100, start_addr = $0800)
    0x00, 0x01, 0x00, 0x08, // +0x34..+0x39 (6 zero bytes)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // +0x3A..+0x3D ("****")
    0x2A, 0x2A, 0x2A, 0x2A,
];

/// Declarative specification for ECA Compactor.
pub static SPEC: PackerSpec = PackerSpec {
    id: "eca",
    name: "ECA Compactor",
    priority: 10,
    systems: &[TargetSystem::C64],
    load_addr_max: None,
    signatures: &SIGNATURES,
    hints: DEFAULT_HINTS,
    patches: &[],
    resolve: Some(resolve_eca),
    hooks: None,
    fixture: &FIXTURE,
    fixture_load_addr: 0x080D,
};
