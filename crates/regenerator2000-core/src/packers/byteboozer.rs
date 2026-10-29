//! ByteBoozer 2.0 packer specification and emulation hooks.

use crate::state::types::TargetSystem;

use super::spec::{
    AddrSource, Anchor, Bounds, Hints, LoadedImage, PackerHooks, PackerSpec, PointerGuard,
    RefineContext, ResolvedSpec, Signature, SignatureMatch, sig,
};

/// ByteBoozer 2.0 10-byte signature at `$080D`.
///
/// Matches `SEI; LDA #$34; STA $xx; ...; LAX $xx,Y; LDA $081E,X`.
pub const BYTEBOOZER_SIGNATURE: Signature = Signature {
    anchor: Anchor::Absolute(0x080D),
    pattern: sig![0x78, 0xA9, 0x34, 0x85, __, __, 0xB7, 0xBD, 0x1E, 0x08],
};

const SIGNATURES: [Signature; 1] = [BYTEBOOZER_SIGNATURE];

const BASE_HINTS: Hints = Hints {
    dep_addr: Some(AddrSource::Const(0x0010)),
    // $080D + 0x79 = $0886..=$0887
    start_addr: Some(AddrSource::OperandLe { from_match: 0x79 }),
    end_addr: Some(AddrSource::PointerAfterRun {
        ptr_addr: 0x0077,
        adjust: -1,
        guard: PointerGuard::NearDetectedEnd { slack: 512 },
    }),
    // $080D + 0xBE = $08CB..=$08CC
    entry_point: Some(AddrSource::OperandLe { from_match: 0xBE }),
    ..Hints::NONE
};

/// Validates ByteBoozer 2.0 bounds (`mem.len() > 0x0887`, `load_addr <= 0x0801`, and
/// readable operands at `$0886..=$0887` and `$08CB..=$08CC`).
#[must_use]
fn resolve_byteboozer(image: &LoadedImage<'_>, sig_match: &SignatureMatch) -> Option<ResolvedSpec> {
    let q = usize::from(sig_match.anchor_addr);
    if image.mem.len() <= 0x0887 || image.load_addr > 0x0801 || q < usize::from(image.load_addr) {
        return None;
    }
    let _ = image.read_u16_le(0x0886)?;
    let _ = image.read_u16_le(0x08CB)?;
    Some(ResolvedSpec::from_hints(BASE_HINTS))
}

/// Emulation-time hooks for ByteBoozer 2.0.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ByteBoozerHooks;

impl ByteBoozerHooks {
    /// Creates a new [`ByteBoozerHooks`] instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl PackerHooks for ByteBoozerHooks {
    fn refine(&self, ctx: &RefineContext<'_>, bounds: &mut Bounds) {
        if !ctx.system.is_c64() || ctx.snapshot.len() < 0x08C4 {
            return;
        }

        let b0 = ctx.snapshot[0x080D..0x0811] == [0x78, 0xA9, 0x34, 0x85];
        let b1 = ctx.snapshot[0x0813..0x0817] == [0xB7, 0xBD, 0x1E, 0x08];
        let b2 = ctx.snapshot[0x0870..0x0874] == [0xA8, 0x20, 0xAD, 0x00];
        let b3 = ctx.snapshot[0x08C0..0x08C4] == [0xAE, 0xD0, 0x02, 0xE6];

        if b0 && b1 && b2 && b3 && ctx.mem.len() > 0x78 {
            let reported_end = u16::from_le_bytes([ctx.mem[0x77], ctx.mem[0x78]]);
            if reported_end > bounds.start_addr {
                bounds.end_addr = reported_end.saturating_sub(1);
            }
        }
    }
}

/// Constructs [`ByteBoozerHooks`] for a matched ByteBoozer image (Ruling B3).
#[must_use]
pub fn create_byteboozer_hooks(
    _image: &LoadedImage<'_>,
    _sig_match: &SignatureMatch,
) -> Box<dyn PackerHooks> {
    Box::new(ByteBoozerHooks::new())
}

/// Synthetic self-test fixture loaded at `$0801` (22 bytes covering `$0801..=$0816`):
/// - `$0801..=$080C`: 12 zero padding bytes
/// - `$080D..=$0816`: 10-byte [`BYTEBOOZER_SIGNATURE`] at `$080D`
const BYTEBOOZER_FIXTURE: [u8; 22] = [
    // $0801..=$080C (12 bytes)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // $080D..=$0816 (10 bytes)
    0x78, 0xA9, 0x34, 0x85, 0x01, 0x00, 0xB7, 0xBD, 0x1E, 0x08,
];

/// Declarative specification for ByteBoozer 2.0.
pub static SPEC: PackerSpec = PackerSpec {
    id: "byteboozer",
    name: "ByteBoozer",
    priority: 90,
    systems: &[TargetSystem::C64],
    load_addr_max: Some(0x0801),
    signatures: &SIGNATURES,
    hints: BASE_HINTS,
    patches: &[],
    resolve: Some(resolve_byteboozer),
    hooks: Some(create_byteboozer_hooks),
    fixture: &BYTEBOOZER_FIXTURE,
    fixture_load_addr: 0x0801,
};
