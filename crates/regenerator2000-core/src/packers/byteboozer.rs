//! `ByteBoozer` family (`ByteBoozer`, `ByteBoozer 1.0`, `ByteBoozer 1.1`, `LamerBoozer`,
//! and `RoyalBoozer`) packer specification and emulation hooks.
//!
//! Targets [`TargetSystem::C64`] images with `load_addr <= $0801`. Probes candidate anchors
//! `$080D`, `$080E`, and `$0817` in order, and attaches [`ByteBoozerHooks`] to refine the
//! `ByteBoozer 2.0` inclusive end address from `$0077..=$0078`.

use crate::state::types::TargetSystem;

use super::spec::{
    AddrSource, Anchor, Bounds, Hints, LoadedImage, PackerHooks, PackerSpec, PointerGuard,
    RefineContext, ResolvedSpec, Signature, SignatureMatch, sig,
};

/// `ByteBoozer` 2.0 10-byte signature at `$080D`.
///
/// Matches `SEI, LDA #$34, STA $zp, <op> #$B7, LDA $081E,X`.
pub const BYTEBOOZER_SIGNATURE: Signature = Signature {
    anchor: Anchor::Absolute(0x080D),
    pattern: sig![0x78, 0xA9, 0x34, 0x85, __, __, 0xB7, 0xBD, 0x1E, 0x08],
};

const CANDIDATE_ANCHORS: [Anchor; 3] = [
    Anchor::Absolute(0x080D),
    Anchor::Absolute(0x080E),
    Anchor::Absolute(0x0817),
];

/// Prefilter signature covering `$080D` (`0x78` SEI / `0x4C` JMP for 1.1E),
/// `$080E` (`0xA2` LDX for `RoyalBoozer`), and `$0817` (`0x78` SEI for `LamerBoozer`).
const SIGNATURES: [Signature; 1] = [Signature {
    anchor: Anchor::Candidates(&CANDIDATE_ANCHORS),
    pattern: sig![[0x78, 0x4C, 0xA2]],
}];

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

const BB1_BASE_HINTS: Hints = Hints {
    dep_addr: Some(AddrSource::Const(0x0008)),
    end_addr: Some(AddrSource::PointerAfterRun {
        ptr_addr: 0x0004,
        adjust: -1,
        guard: PointerGuard::NearDetectedEnd { slack: 512 },
    }),
    forced_entry: Some(AddrSource::Const(0x080D)),
    ..Hints::NONE
};

/// Validates `ByteBoozer` 2.0 / 0.9 / 1.0 / 1.1 / `LamerBoozer` / `RoyalBoozer` variants.
#[must_use]
fn resolve_byteboozer(image: &LoadedImage<'_>, sig_match: &SignatureMatch) -> Option<ResolvedSpec> {
    let mem = image.mem;
    let q = usize::from(sig_match.anchor_addr);
    if image.load_addr > 0x0801 || q < usize::from(image.load_addr) {
        return None;
    }

    match sig_match.anchor_addr {
        0x080D => {
            // ByteBoozer 2.0 (primary `$080D` stub).
            if BYTEBOOZER_SIGNATURE.matches_at(mem, 0x080D) && mem.len() > 0x0887 {
                let _ = image.read_u16_le(0x0886)?;
                let _ = image.read_u16_le(0x08CB)?;
                return Some(ResolvedSpec::from_hints(BASE_HINTS));
            }

            // ByteBoozer 0.9/1.0 (unp64 v2.42 relaxed ID at $080D).
            if mem.len() > 0x091D
                && image.read_u8(0x080D) == Some(0x78)
                && image.read_u32_le(0x0812) == Some(0x1FBD_3BA2)
                && image.read_u32_le(0x0815) == Some(0x0695_081F)
                && image.read_u32_le(0x0819) == Some(0x4CF8_10CA)
                && image.read_u8(0x081D) == Some(0x08)
                && image.read_u32_le(0x08D2) == Some(0x3003_DD20)
            {
                let _ = image.read_u16_le(0x091C)?;
                let _ = image.read_u16_le(0x08DD)?;
                return Some(ResolvedSpec::named(
                    "ByteBoozer 1.0",
                    Hints {
                        start_addr: Some(AddrSource::AbsoluteLe(0x091C)),
                        entry_point: Some(AddrSource::AbsoluteLe(0x08DD)),
                        ..BB1_BASE_HINTS
                    },
                ));
            }

            // ByteBoozer 1.1C.
            if mem.len() > 0x0913
                && image.read_u32_le(0x080D) == Some(0x8534_A978)
                && image.read_u32_le(0x0811) == Some(0xBD3D_A201)
                && image.read_u32_le(0x0815) == Some(0x0695_081F)
                && image.read_u32_le(0x0819) == Some(0x4CF8_10CA)
                && image.read_u8(0x081D) == Some(0x08)
                && image.read_u32_le(0x08D4) == Some(0x3003_D120)
            {
                let _ = image.read_u16_le(0x0912)?;
                let _ = image.read_u16_le(0x08DF)?;
                return Some(ResolvedSpec::named(
                    "ByteBoozer 1.1",
                    Hints {
                        start_addr: Some(AddrSource::AbsoluteLe(0x0912)),
                        entry_point: Some(AddrSource::AbsoluteLe(0x08DF)),
                        ..BB1_BASE_HINTS
                    },
                ));
            }

            // ByteBoozer 1.1E.
            if mem.len() > 0x096B
                && image.read_u32_le(0x080D) == Some(0xA508_3C4C)
                && image.read_u32_le(0x0844) == Some(0x8534_A978)
                && image.read_u32_le(0x0848) == Some(0xBD3B_A201)
                && image.read_u32_le(0x0909) == Some(0x3003_CE20)
                && image.read_u8(0x0854) == Some(0x08)
            {
                let _ = image.read_u16_le(0x096A)?;
                let _ = image.read_u16_le(0x0911)?;
                return Some(ResolvedSpec::named(
                    "ByteBoozer 1.1",
                    Hints {
                        start_addr: Some(AddrSource::AbsoluteLe(0x096A)),
                        entry_point: Some(AddrSource::AbsoluteLe(0x0911)),
                        forced_entry: Some(AddrSource::Const(0x0810)),
                        ..BB1_BASE_HINTS
                    },
                ));
            }

            None
        }
        0x080E => {
            // RoyalBoozer (unp64 v2.42: Royal Arte / Interruptus by Booze Design).
            if mem.len() > 0x0945
                && image.read_u32_le(0x080E) == Some(0x1ABD_28A2)
                && image.read_u32_le(0x0817) == Some(0x0006_4CF8)
                && image.read_u32_le(0x0860) == Some(0x0890_02C5)
                && image.read_u32_le(0x093C) == Some(0xCACA_FBD6)
            {
                let raw_end = image.read_u16_le(0x0944)?;
                let _ = image.read_u16_le(0x0841)?;
                // `raw_end` is exclusive. Zero wraps to `$FFFF` via `wrapping_sub(1)`, and `1..=load_addr` is invalid.
                // `start_addr` comes from post-run `$00FA` validated by `PointerGuard::ValidStart`.
                let end_addr = (raw_end == 0 || raw_end > image.load_addr)
                    .then(|| AddrSource::Const(raw_end.wrapping_sub(1)));
                return Some(ResolvedSpec::named(
                    "RoyalBoozer",
                    Hints {
                        dep_addr: Some(AddrSource::Const(0x0200)),
                        start_addr: Some(AddrSource::PointerAfterRun {
                            ptr_addr: 0x00FA,
                            adjust: 0,
                            guard: PointerGuard::ValidStart,
                        }),
                        end_addr,
                        entry_point: Some(AddrSource::AbsoluteLe(0x0841)),
                        forced_entry: Some(AddrSource::Const(0x080E)),
                        ..Hints::NONE
                    },
                ));
            }
            None
        }
        0x0817 => {
            // LamerBoozer.
            if mem.len() > 0x08FA
                && image.read_u32_le(0x0817) == Some(0x8534_A978)
                && image.read_u32_le(0x081B) == Some(0xBDC6_A201)
                && image.read_u32_le(0x081F) == Some(0xF99D_083C)
                && image.read_u32_le(0x0824) == Some(0xBDF7_D0CA)
                && image.read_u32_le(0x08F7) == Some(0x60F2_10CA)
            {
                let _ = image.read_u16_le(0x083F)?;
                let _ = image.read_u16_le(0x08C5)?;
                return Some(ResolvedSpec::named(
                    "LamerBoozer",
                    Hints {
                        dep_addr: Some(AddrSource::Const(0x0100)),
                        start_addr: Some(AddrSource::AbsoluteLe(0x083F)),
                        end_addr: Some(AddrSource::PointerAfterRun {
                            ptr_addr: 0x00FC,
                            adjust: -1,
                            guard: PointerGuard::NearDetectedEnd { slack: 512 },
                        }),
                        entry_point: Some(AddrSource::AbsoluteLe(0x08C5)),
                        forced_entry: Some(AddrSource::Const(0x0817)),
                        ..Hints::NONE
                    },
                ));
            }
            None
        }
        _ => None,
    }
}

/// Emulation-time hooks for `ByteBoozer 2.0` (`$0077..=$0078` end refinement on C64).
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

/// Constructs [`ByteBoozerHooks`] for a matched `ByteBoozer` image.
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

/// Declarative specification for the `ByteBoozer` family (`2.0`, `0.9`/`1.0`, `1.1C`/`1.1E`,
/// `LamerBoozer`, and `RoyalBoozer`).
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
