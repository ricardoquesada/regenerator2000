//! Time Cruncher and Time Cruncher v3.1 / SpeedPacker specification.
//!
//! Targets [`TargetSystem::C64`] images. Probes `BasicStart + 0x0F` (`$0810` on C64) for the
//! legacy stub and `$0818` for `v3.1` and `v3.1 / SpeedPacker`, extracting split `end_addr`
//! bytes and optional `$091A..=$0930` entry jumps.

use crate::state::types::TargetSystem;
use crate::unpacker::find_sys_address;

use super::spec::{
    AddrSource, Anchor, Hints, LoadedImage, PackerSpec, PointerGuard, ResolvedSpec, Signature,
    SignatureMatch, sig,
};

const CANDIDATE_ANCHORS: [Anchor; 2] = [
    Anchor::BasicStart { offset: 0x0F },
    Anchor::Absolute(0x0818),
];

/// Matches legacy `SEI, LDA #$34, STA $01, LDY #$C4, LDA ...` and `v3.1` `LDA #$30..#$3F`
/// or `JSR $xxD0, SEI, STA $01`.
const SIGNATURES: [Signature; 1] = [Signature {
    anchor: Anchor::Candidates(&CANDIDATE_ANCHORS),
    pattern: sig![
        [0x78, 0x20],
        [0xA9, 0xD0],
        [
            0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x3B, 0x3C, 0x3D,
            0x3E, 0x3F, 0x78
        ],
        0x85,
        0x01,
        0xA0,
        0xC4,
        0xB9
    ],
}];

const LEGACY_PATTERN: [u8; 8] = [0x78, 0xA9, 0x34, 0x85, 0x01, 0xA0, 0xC4, 0xB9];

const BASE_HINTS: Hints = Hints {
    dep_addr: Some(AddrSource::Const(0x0100)),
    start_addr: Some(AddrSource::BasicStart(0)),
    ..Hints::NONE
};

/// Resolves legacy `$0810` Time Cruncher or Time Cruncher v3.1 / SpeedPacker at `$0818`.
#[must_use]
fn resolve_time_cruncher(
    image: &LoadedImage<'_>,
    sig_match: &SignatureMatch,
) -> Option<ResolvedSpec> {
    let mem = image.mem;
    if sig_match.anchor_addr != 0x0818 {
        let q = usize::from(sig_match.anchor_addr);
        if mem.get(q..q + 8) == Some(&LEGACY_PATTERN) {
            return Some(ResolvedSpec::from_hints(BASE_HINTS));
        }
        return None;
    }

    if mem.len() <= 0x0935 {
        return None;
    }

    let w818 = image.read_u32_le(0x0818)?;
    let header_ok = (w818 & 0xFFF0_FFFF) == 0x8530_A978 || w818 == 0x8578_D020;
    if !header_ok
        || image.read_u32_le(0x081C) != Some(0xB9C4_A001)
        || image.read_u32_le(0x0820) != Some(0xF899_083C)
        || image.read_u8(0x083C) != Some(0x01)
    {
        return None;
    }

    let w838 = image.read_u32_le(0x0838)?;
    if w838 != 0x004C_2E85 && w838 != 0x004C_AF85 {
        return None;
    }

    let is_speedpacker = image.read_u8(0x091D) == Some(0x4C) && image.read_u8(0x0896) == Some(0x19);
    let name = if is_speedpacker {
        "Time Cruncher 3.1 / SpeedPacker"
    } else {
        "Time Cruncher 3.1"
    };

    // "TSS GROUP" variant loads exclusive end via `LDA #$lo` / `LDA #$hi` at `$082E` / `$0834` (`$082F` / `$0835`).
    // Standard `v3.1` places the immediate bytes at `$0833` / `$0837`. Convert `raw_end` to inclusive (`0` wraps to `$FFFF`).
    let end_src = if image.read_u8(0x082E) == Some(0xA9) && image.read_u8(0x0834) == Some(0xA9) {
        AddrSource::SplitBytes {
            lo_from_match: 0x17,
            hi_from_match: 0x1D,
        }
    } else {
        AddrSource::SplitBytes {
            lo_from_match: 0x1B,
            hi_from_match: 0x1F,
        }
    };
    let raw_end = end_src.resolve_static(image, 0x0818)?;
    let end_addr = (raw_end == 0 || raw_end > image.load_addr)
        .then(|| AddrSource::Const(raw_end.wrapping_sub(1)));

    let mut entry_point = None;
    let mut start_addr = Some(AddrSource::BasicStart(0));
    // Scan for `STA $01; <op>; JMP entry` (`85 01 __ 4C`) to extract entry point and `$00FC` post-run start pointer.
    for p in 0x091A..=0x0930 {
        if let Some(w) = image.read_u32_le(p)
            && (w & 0xFF00_FFFF) == 0x4C00_0185
            && let Some(ep) = image.read_u16_le(p + 4)
        {
            entry_point = Some(AddrSource::Const(ep));
            start_addr = Some(AddrSource::PointerAfterRun {
                ptr_addr: 0x00FC,
                adjust: 0,
                guard: PointerGuard::ValidStart,
            });
        }
    }

    // Without a BASIC `SYS` line, enter at `$081A` (`SEI`) if `$0818` starts with a 2-byte skip, else `$0818`.
    let forced_entry = if find_sys_address(mem, image.system.default_basic_start()).is_none() {
        let entry = if image.read_u8(0x081A) == Some(0x78) {
            0x081A
        } else {
            0x0818
        };
        Some(AddrSource::Const(entry))
    } else {
        None
    };

    Some(ResolvedSpec::named(
        name,
        Hints {
            dep_addr: Some(AddrSource::Const(0x0100)),
            start_addr,
            end_addr,
            entry_point,
            forced_entry,
            ..Hints::NONE
        },
    ))
}

/// Declarative specification for the Time Cruncher stub at `$0810` and v3.1 / SpeedPacker at `$0818`.
pub static SPEC: PackerSpec = PackerSpec {
    id: "time_cruncher",
    name: "Time Cruncher",
    priority: 50,
    systems: &[TargetSystem::C64],
    load_addr_max: None,
    signatures: &SIGNATURES,
    hints: BASE_HINTS,
    patches: &[],
    resolve: Some(resolve_time_cruncher),
    hooks: None,
    // 15 padding bytes ($0801..=$080F) + 8 signature bytes at $0810..=$0817.
    fixture: &[
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x78, 0xA9, 0x34, 0x85, 0x01, 0xA0, 0xC4, 0xB9,
    ],
    fixture_load_addr: 0x0801,
};
