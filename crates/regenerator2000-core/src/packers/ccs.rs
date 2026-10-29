//! Commodore Cruncher System (CCS) packer specification.
//!
//! Per Design Doc Addendum B (Ruling B5) and Ruling R14, this specification
//! preserves the legacy bug-compatible indexing (`mem[target - load_addr]` on a
//! 64 KB memory buffer) and the `load_addr <= 0x0801` guard via `resolve_ccs`.

use crate::state::types::TargetSystem;

use super::spec::{AddrSource, Hints, LoadedImage, PackerSpec, ResolvedSpec, SignatureMatch};

const HINTS: Hints = Hints {
    dep_addr: Some(AddrSource::Const(0x0100)),
    start_addr: Some(AddrSource::BasicStart(0)),
    ..Hints::NONE
};

/// Bug-compatible detection for Commodore Cruncher System (Rulings B5 and R14).
///
/// Preserves the legacy `mem[target - load_addr]` indexing and the `load_addr <= 0x0801`
/// guard so `target - load_addr` never underflows when `load_addr > 0x0801`.
#[must_use]
fn resolve_ccs(image: &LoadedImage<'_>, _sig_match: &SignatureMatch) -> Option<ResolvedSpec> {
    let mem = image.mem;
    let load_addr = image.load_addr;
    if mem.len() <= 0x081C || load_addr > 0x0801 {
        return None;
    }
    let load = usize::from(load_addr);
    let matches_at_bug_index = |target: usize, min_len: usize, expected: &[u8]| -> bool {
        mem.len() > min_len
            && target
                .checked_sub(load)
                .and_then(|idx| mem.get(idx..))
                .is_some_and(|slice| slice.starts_with(expected))
    };

    // Variant 1: SEI at $0817 followed by INC $01, LDA ... ($78, $E6, $01, $B9)
    let is_v1 = matches_at_bug_index(0x0817, 0x081B, &[0x78, 0xE6, 0x01, 0xB9]);
    // Variant 2: LDY #$00, SEI, INC $01 at $0812 ($A0, $00, $78, $E6)
    let is_v2 = matches_at_bug_index(0x0812, 0x0816, &[0xA0, 0x00, 0x78, 0xE6]);
    // Variant 3: SEI, INC $01, LDA at $0814 ($78, $E6, $01, $B9)
    let is_v3 = matches_at_bug_index(0x0814, 0x0818, &[0x78, 0xE6, 0x01, 0xB9]);
    // Variant 4: LDY #$00, SEI, STY at $080B ($A0, $00, $78, $8C)
    let is_v4 = matches_at_bug_index(0x080B, 0x080F, &[0xA0, 0x00, 0x78, 0x8C]);

    (is_v1 || is_v2 || is_v3 || is_v4).then_some(ResolvedSpec::from_hints(HINTS))
}

/// Declarative specification for Commodore Cruncher System (CCS).
pub static SPEC: PackerSpec = PackerSpec {
    id: "ccs",
    name: "Commodore Cruncher System",
    priority: 170,
    systems: &[TargetSystem::C64],
    load_addr_max: Some(0x0801),
    signatures: &[],
    hints: HINTS,
    patches: &[],
    resolve: Some(resolve_ccs),
    hooks: None,
    // Loaded at $040A (`2 * $040A == $0814`), so Variant 3 (`$0814 - load_addr == $040A`)
    // matches the 4-byte fixture directly.
    fixture: &[0x78, 0xE6, 0x01, 0xB9],
    fixture_load_addr: 0x040A,
};
