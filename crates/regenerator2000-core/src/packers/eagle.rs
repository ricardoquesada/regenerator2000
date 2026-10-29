//! Eagle Cruncher packer specification.

use crate::state::types::TargetSystem;

use super::spec::{AddrSource, Anchor, Hints, PackerSpec, Signature, sig};

const SIGNATURES: [Signature; 1] = [Signature {
    anchor: Anchor::BasicStart { offset: 0x0C },
    pattern: sig![
        0x78, 0xA2, __, 0xBD, __, __, 0x9D, 0x00, 0x01, 0xD0, 0xFC, 0x4C, 0x00, 0x01
    ],
}];

/// Declarative specification for the Eagle Cruncher stub at `$080D`.
pub static SPEC: PackerSpec = PackerSpec {
    id: "eagle",
    name: "Eagle Cruncher",
    priority: 190,
    systems: &[TargetSystem::C64],
    load_addr_max: Some(0x0801),
    signatures: &SIGNATURES,
    hints: Hints {
        dep_addr: Some(AddrSource::Const(0x0100)),
        start_addr: Some(AddrSource::BasicStart(0)),
        ..Hints::NONE
    },
    patches: &[],
    resolve: None,
    hooks: None,
    // 12 padding bytes ($0801..=$080C) + 14 signature bytes at $080D..=$081A.
    fixture: &[
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x78, 0xA2, 0x00,
        0xBD, 0x00, 0x00, 0x9D, 0x00, 0x01, 0xD0, 0xFC, 0x4C, 0x00, 0x01,
    ],
    fixture_load_addr: 0x0801,
};
