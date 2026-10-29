//! Cruel Cruncher v2.2 packer specification.

use crate::state::types::TargetSystem;

use super::spec::{AddrSource, Anchor, Hints, PackerSpec, Signature, sig};

const SIGNATURES: [Signature; 1] = [Signature {
    anchor: Anchor::BasicStart { offset: 0x0C },
    pattern: sig![
        0x78,
        0xA9,
        0x34,
        0x85,
        0x01,
        [0xA2, 0xA0],
        [0x1A, 0x1F, 0x20]
    ],
}];

/// Declarative specification for the Cruel Cruncher v2.2 stub at `$080D`.
pub static SPEC: PackerSpec = PackerSpec {
    id: "cruel_cruncher",
    name: "Cruel Cruncher",
    priority: 30,
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
    // 12 padding bytes ($0801..=$080C) + 7 signature bytes at $080D..=$0813.
    fixture: &[
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x78, 0xA9, 0x34,
        0x85, 0x01, 0xA2, 0x1A,
    ],
    fixture_load_addr: 0x0801,
};
