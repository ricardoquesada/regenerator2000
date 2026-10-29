//! 1001 Card Cruncher (`1001 CardCruncher ACM` and `1001 CardCruncher v4`) packer specification.

use crate::state::types::TargetSystem;

use super::spec::{
    AddrSource, Anchor, Hints, LoadedImage, PackerSpec, ResolvedSpec, Signature, SignatureMatch,
    sig,
};

/// Candidate anchors tried in order for Variant 1 (`1001 CardCruncher ACM`).
pub const ACM_CANDIDATES: [Anchor; 2] = [Anchor::Absolute(0x080D), Anchor::Absolute(0x080F)];

/// Variant 1 (`1001 CardCruncher ACM`) 17-byte signature at candidate anchors `$080D` and `$080F`.
///
/// Matches `SEI; LDX #imm; STX $01; LDA abs,X; STA abs,X; DEX; BNE rel; JMP $0100`.
pub const ACM_SIGNATURE: Signature = Signature {
    anchor: Anchor::Candidates(&ACM_CANDIDATES),
    pattern: sig![
        0x78, 0xA2, __, 0x86, 0x01, 0xBD, __, __, 0x9D, __, __, 0xCA, 0xD0, __, 0x4C, 0x00, 0x01
    ],
};

/// Variant 2 (`1001 CardCruncher v4`) 9-byte signature at `$0815`.
///
/// Matches `STX $01; ...; LDA $082A,Y; STA $FAxx,Y` (`mem[$0815..=$081D]`).
pub const V4_SIGNATURE: Signature = Signature {
    anchor: Anchor::Absolute(0x0815),
    pattern: sig![0x86, 0x01, __, __, __, 0x2A, 0x08, 0x99, 0xFA],
};

/// Combined candidate anchors (`$080D`, `$080F` for ACM; `$0815` for v4) probed in order.
const ALL_CANDIDATES: [Anchor; 3] = [
    Anchor::Absolute(0x080D),
    Anchor::Absolute(0x080F),
    Anchor::Absolute(0x0815),
];

/// Primary candidate signature covering both Variant 1 (`$080D`/`$080F`) and Variant 2 (`$0815`).
///
/// `resolve_card_cruncher` verifies the full variant-specific signature (`ACM_SIGNATURE` or
/// `V4_SIGNATURE`) and minimum image length at each candidate position before reading the
/// fixed-address little-endian operands.
const SIGNATURES: [Signature; 1] = [Signature {
    anchor: Anchor::Candidates(&ALL_CANDIDATES),
    pattern: sig![
        [0x78, 0x86],
        [0xA2, 0x01],
        __,
        __,
        __,
        [0xBD, 0x2A],
        __,
        __,
        [0x9D, 0xFA]
    ],
}];

/// Resolves 1001 Card Cruncher Variant 1 (`ACM`) or Variant 2 (`v4`) at candidate `sig_match.anchor_addr`.
#[must_use]
fn resolve_card_cruncher(
    image: &LoadedImage<'_>,
    sig_match: &SignatureMatch,
) -> Option<ResolvedSpec> {
    let mem = image.mem;
    let load_addr = image.load_addr;
    if mem.len() <= 0x0840 || load_addr > 0x0801 {
        return None;
    }

    match sig_match.anchor_addr {
        q @ (0x080D | 0x080F) => {
            let q_idx = usize::from(q);
            if mem.len() <= q_idx + 17 || !ACM_SIGNATURE.matches_at(mem, q) {
                return None;
            }
            let end_val = image.read_u16_le(0x081E)?;
            let entry_val = image.read_u16_le(0x083F)?;
            let end_addr = (end_val > load_addr).then(|| AddrSource::Const(end_val - 1));

            Some(ResolvedSpec::named(
                "1001 CardCruncher ACM",
                Hints {
                    dep_addr: Some(AddrSource::Const(0x0100)),
                    start_addr: Some(AddrSource::BasicStart(0)),
                    end_addr,
                    entry_point: Some(AddrSource::Const(entry_val)),
                    ..Hints::NONE
                },
            ))
        }
        0x0815 => {
            if mem.len() <= 0x0941 || !V4_SIGNATURE.matches_at(mem, 0x0815) {
                return None;
            }
            let end_val = image.read_u16_le(0x082E)?;
            let entry_val = image.read_u16_le(0x0940)?;

            Some(ResolvedSpec::named(
                "1001 CardCruncher v4",
                Hints {
                    dep_addr: Some(AddrSource::Const(0x0100)),
                    start_addr: Some(AddrSource::BasicStart(0)),
                    end_addr: Some(AddrSource::Const(end_val)),
                    entry_point: Some(AddrSource::Const(entry_val)),
                    ..Hints::NONE
                },
            ))
        }
        _ => None,
    }
}

/// Synthetic self-test fixture loaded at `$0801` (64 bytes covering `$0801..=$0840`):
/// - `$0801..=$080C`: 12 zero padding bytes
/// - `$080D..=$081D`: 17-byte `1001 CardCruncher ACM` signature at `$080D`
/// - `$081E..=$081F`: exclusive end operand `$319D` (`9D 31` -> inclusive `end_addr = $319C`)
/// - `$0820..=$083E`: 31 zero padding bytes
/// - `$083F..=$0840`: entry operand `$3197` (`97 31`)
const FIXTURE: [u8; 0x40] = [
    // $0801..=$080C (12 bytes)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // $080D..=$081D (17 bytes: ACM signature)
    0x78, 0xA2, 0xF0, 0x86, 0x01, 0xBD, 0x40, 0x08, 0x9D, 0x00, 0x01, 0xCA, 0xD0, 0xF7, 0x4C, 0x00,
    0x01, // $081E..=$081F (2 bytes: LE end_val = $319D)
    0x9D, 0x31, // $0820..=$083E (31 bytes)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // $083F..=$0840 (2 bytes: LE entry_val = $3197)
    0x97, 0x31,
];

/// Declarative specification for 1001 Card Cruncher (`ACM` and `v4` variants).
pub static SPEC: PackerSpec = PackerSpec {
    id: "card_cruncher",
    name: "1001 CardCruncher ACM",
    priority: 60,
    systems: &[TargetSystem::C64],
    load_addr_max: Some(0x0801),
    signatures: &SIGNATURES,
    hints: Hints {
        dep_addr: Some(AddrSource::Const(0x0100)),
        start_addr: Some(AddrSource::BasicStart(0)),
        entry_point: Some(AddrSource::AbsoluteLe(0x083F)),
        ..Hints::NONE
    },
    patches: &[],
    resolve: Some(resolve_card_cruncher),
    hooks: None,
    fixture: &FIXTURE,
    fixture_load_addr: 0x0801,
};
