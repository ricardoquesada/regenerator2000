//! TinyCrunch packer specification.

use crate::state::types::TargetSystem;

use super::spec::{AddrSource, Anchor, Hints, PackerSpec, Signature, sig};

const SIGNATURES: [Signature; 1] = [Signature {
    anchor: Anchor::BasicStart { offset: 0x0C },
    pattern: sig![0x78, 0xA2, 0xBB, 0xBD, 0x1B, 0x08, 0x9D, 0xFF],
}];

/// Declarative specification for the TinyCrunch stub at `$080D` with entry operand at `$0836`.
pub static SPEC: PackerSpec = PackerSpec {
    id: "tiny_crunch",
    name: "TinyCrunch",
    priority: 130,
    systems: &[TargetSystem::C64],
    load_addr_max: Some(0x0801),
    signatures: &SIGNATURES,
    hints: Hints {
        dep_addr: Some(AddrSource::Const(0x010E)),
        start_addr: Some(AddrSource::BasicStart(0)),
        entry_point: Some(AddrSource::OperandLe { from_match: 0x29 }),
        ..Hints::NONE
    },
    patches: &[],
    resolve: None,
    hooks: None,
    // 12 padding bytes ($0801..=$080C) + 8 signature bytes ($080D..=$0814)
    // + 33 padding bytes ($0815..=$0835) + LE entry operand $0911 at $0836..=$0837.
    fixture: &[
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x78, 0xA2, 0xBB,
        0xBD, 0x1B, 0x08, 0x9D, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x11, 0x09,
    ],
    fixture_load_addr: 0x0801,
};
