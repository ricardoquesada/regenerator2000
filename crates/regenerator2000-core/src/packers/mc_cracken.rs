//! MC-Cracken Compressor packer specification.

use crate::state::types::TargetSystem;

use super::spec::{
    AddrSource, Anchor, Hints, LoadedImage, PackerSpec, PointerGuard, ResolvedSpec, Signature,
    SignatureMatch, sig,
};

/// MC-Cracken Compressor 8-byte signature searched in `$0810..=$0840`.
///
/// Matches `LDA #imm; STA $xx; LDY #$00; DEC $AF`.
pub const MCCRACKEN_SIGNATURE: Signature = Signature {
    anchor: Anchor::Window(0x0810..=0x0840),
    pattern: sig![0xA9, __, 0x85, __, 0xA0, 0x00, 0xC6, 0xAF],
};

const SIGNATURES: [Signature; 1] = [MCCRACKEN_SIGNATURE];

const BASE_HINTS: Hints = Hints {
    dep_addr: Some(AddrSource::Const(0x0100)),
    start_addr: Some(AddrSource::BasicStart(-1)),
    end_addr: Some(AddrSource::PointerAfterRun {
        ptr_addr: 0x00AE,
        adjust: -1,
        guard: PointerGuard::McCrackenEndOfProgram,
    }),
    entry_point: Some(AddrSource::Const(0x1100)),
    ..Hints::NONE
};

/// Resolves MC-Cracken per-position bounds check (`mem.len() > p + 0x40`) and optional
/// `JMP` entry point at `p + 0x61..=p + 0x63` (defaulting to `$1100`).
#[must_use]
fn resolve_mc_cracken(image: &LoadedImage<'_>, sig_match: &SignatureMatch) -> Option<ResolvedSpec> {
    let p = usize::from(sig_match.anchor_addr);
    if image.mem.len() <= p + 0x40 {
        return None;
    }

    let entry_point = if image.mem.len() > p + 0x64 && image.mem[p + 0x61] == 0x4C {
        image.read_u16_le(p + 0x62).unwrap_or(0x1100)
    } else {
        0x1100
    };

    Some(ResolvedSpec::from_hints(Hints {
        entry_point: Some(AddrSource::Const(entry_point)),
        ..BASE_HINTS
    }))
}

/// Synthetic self-test fixture loaded at `$0810` (8 bytes matching [`MCCRACKEN_SIGNATURE`]).
const MCCRACKEN_FIXTURE: [u8; 8] = [0xA9, 0x01, 0x85, 0x02, 0xA0, 0x00, 0xC6, 0xAF];

/// Declarative specification for MC-Cracken Compressor.
pub static SPEC: PackerSpec = PackerSpec {
    id: "mc_cracken",
    name: "McCracken Compressor",
    priority: 70,
    systems: &[TargetSystem::C64],
    load_addr_max: None,
    signatures: &SIGNATURES,
    hints: BASE_HINTS,
    patches: &[],
    resolve: Some(resolve_mc_cracken),
    hooks: None,
    fixture: &MCCRACKEN_FIXTURE,
    fixture_load_addr: 0x0810,
};
