//! Turbo Cruncher packer specification.

use crate::state::types::TargetSystem;

use super::spec::{AddrSource, Anchor, Hints, PackerSpec, Signature, sig};

const SIGNATURES: [Signature; 1] = [Signature {
    anchor: Anchor::BasicStart { offset: 0x0C },
    pattern: sig![0x78, 0xA9, 0x34, 0x85, 0x01, 0x20, __, __, 0x4C],
}];

/// Declarative specification for the Turbo Cruncher stub at `$080D`.
pub static SPEC: PackerSpec = PackerSpec {
    id: "turbo_cruncher",
    name: "Turbo Cruncher",
    priority: 210,
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
    // 12 padding bytes ($0801..=$080C) + 9 signature bytes at $080D..=$0815.
    fixture: &[
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x78, 0xA9, 0x34,
        0x85, 0x01, 0x20, 0x00, 0x00, 0x4C,
    ],
    fixture_load_addr: 0x0801,
};
