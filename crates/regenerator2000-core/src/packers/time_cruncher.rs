//! Time Cruncher packer specification.

use crate::state::types::TargetSystem;

use super::spec::{AddrSource, Anchor, Hints, PackerSpec, Signature, sig};

const SIGNATURES: [Signature; 1] = [Signature {
    anchor: Anchor::BasicStart { offset: 0x0F },
    pattern: sig![0x78, 0xA9, 0x34, 0x85, 0x01, 0xA0, 0xC4, 0xB9],
}];

/// Declarative specification for the Time Cruncher stub at `$0810`.
pub static SPEC: PackerSpec = PackerSpec {
    id: "time_cruncher",
    name: "Time Cruncher",
    priority: 50,
    systems: &[TargetSystem::C64],
    load_addr_max: None,
    signatures: &SIGNATURES,
    hints: Hints {
        dep_addr: Some(AddrSource::Const(0x0100)),
        start_addr: Some(AddrSource::BasicStart(0)),
        ..Hints::NONE
    },
    patches: &[],
    resolve: None,
    hooks: None,
    // 15 padding bytes ($0801..=$080F) + 8 signature bytes at $0810..=$0817.
    fixture: &[
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x78, 0xA9, 0x34, 0x85, 0x01, 0xA0, 0xC4, 0xB9,
    ],
    fixture_load_addr: 0x0801,
};
