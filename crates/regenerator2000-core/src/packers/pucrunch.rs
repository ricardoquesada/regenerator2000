//! PUCrunch packer specification.

use crate::state::types::TargetSystem;

use super::spec::{
    AddrSource, Anchor, Hints, LoadedImage, PackerSpec, PointerGuard, ResolvedSpec, Signature,
    SignatureMatch, sig,
};

/// Primary PUCrunch stub signature at `$0813` (`BasicStart + $12` on C64).
const SIGNATURES: [Signature; 1] = [Signature {
    anchor: Anchor::BasicStart { offset: 0x12 },
    pattern: sig![
        0x85, 0x01, 0xA2, 0x34, 0xBD, 0x42, 0x08, 0x9D, 0xFF, 0x01, 0xCA, 0xD0
    ],
}];

/// Secondary scan signature in `$0912..=$0937` (`0x0912..0x0938`) locating the optional
/// unpacked `entry_point` operand at `p + 0x0A..=p + 0x0B`.
pub const ENTRY_SIGNATURE: Signature = Signature {
    anchor: Anchor::Window(0x0912..=0x0937),
    pattern: sig![0xA5, 0xFA, 0x85, 0x2D, 0xA5, 0xFB, 0x85, 0x2E],
};

/// Static base hints for PUCrunch (`dep_addr` at `$0813 + 0x2E = $0841`, `start_addr` at
/// `$0813 + 0x66 = $0879`, and post-run end pointer at `$00FA`).
pub const BASE_HINTS: Hints = Hints {
    dep_addr: Some(AddrSource::OperandLe { from_match: 0x2E }),
    start_addr: Some(AddrSource::OperandLe { from_match: 0x66 }),
    end_addr: Some(AddrSource::PointerAfterRun {
        ptr_addr: 0x00FA,
        adjust: -1,
        guard: PointerGuard::NearDetectedEnd { slack: 512 },
    }),
    ..Hints::NONE
};

/// Resolves PUCrunch hints, running the secondary window scan `0x0912..0x0938` for `entry_point`.
#[must_use]
fn resolve_pucrunch(image: &LoadedImage<'_>, _sig_match: &SignatureMatch) -> Option<ResolvedSpec> {
    if image.mem.len() < 0x0944 {
        return None;
    }

    let entry_point = ENTRY_SIGNATURE
        .find_first(image)
        .and_then(|p| image.read_u16_le(usize::from(p) + 0x0A))
        .map(AddrSource::Const);

    Some(ResolvedSpec::from_hints(Hints {
        entry_point,
        ..BASE_HINTS
    }))
}

/// Synthetic self-test fixture loaded at `$0813` (104 bytes covering `$0813..=$087A`):
/// - `+0x00..=+0x0B` (`$0813..=$081E`): 12-byte PUCrunch signature
/// - `+0x2E..=+0x2F` (`$0841..=$0842`): `dep_addr = $0116` (`16 01`)
/// - `+0x66..=+0x67` (`$0879..=$087A`): `start_addr = $0801` (`01 08`)
const FIXTURE: [u8; 0x68] = [
    // +0x00..=+0x0B ($0813..=$081E: 12-byte signature)
    0x85, 0x01, 0xA2, 0x34, 0xBD, 0x42, 0x08, 0x9D, 0xFF, 0x01, 0xCA, 0xD0,
    // +0x0C..=+0x2D (34 zero padding bytes)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, // +0x2E..=+0x2F ($0841..=$0842: dep_addr = $0116)
    0x16, 0x01, // +0x30..=+0x65 (54 zero padding bytes)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // +0x66..=+0x67 ($0879..=$087A: start_addr = $0801)
    0x01, 0x08,
];

/// Declarative specification for PUCrunch.
pub static SPEC: PackerSpec = PackerSpec {
    id: "pucrunch",
    name: "PUCrunch",
    priority: 40,
    systems: &[TargetSystem::C64],
    load_addr_max: None,
    signatures: &SIGNATURES,
    hints: BASE_HINTS,
    patches: &[],
    resolve: Some(resolve_pucrunch),
    hooks: None,
    fixture: &FIXTURE,
    fixture_load_addr: 0x0813,
};
