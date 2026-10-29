//! Exomizer 1.x / 2.x / 3.x packer specification and emulation hooks.

use crate::state::types::TargetSystem;
use crate::unpacker::machine::Phase;

use super::spec::{
    AddrSource, Anchor, Bounds, Hints, LoadedImage, PackerHooks, PackerSpec, RefineContext,
    ResolvedSpec, Signature, SignatureMatch, StepContext, sig,
};

/// Exomizer 3.x core decruncher 8-byte signature (`ADC #$80; ASL A; BPL +15; ASL $FD; BNE ...`),
/// searched in descending order across `load_addr..=load_end - 8`.
///
/// A valid Exomizer 3.x match at `p` additionally requires `p >= 6`, `mem.len() > p + 8`,
/// `mem[p - 6] == 0x4C` (`JMP`), and `mem[p - 4] == 0x01` (stack page high byte `$01xx`).
pub const EXO3_SIGNATURE: Signature = Signature {
    anchor: Anchor::ScanBackFromEnd { max_offset: 0xFFFF },
    pattern: sig![0x69, 0x80, 0x0A, 0x10, 0x0F, 0x06, 0xFD, 0xD0],
};

/// Exomizer 1.x / 2.x 8-byte signature (`INY; CPY #$34/#$50; BNE ...; JMP ...`),
/// searched in descending order across `load_addr..=load_end - 8`.
pub const EXO2_SIGNATURE: Signature = Signature {
    anchor: Anchor::ScanBackFromEnd { max_offset: 0xFFFF },
    pattern: sig![0xC8, 0xC0, [0x34, 0x50], 0xD0, __, __, __, 0x4C],
};

/// Combined descending candidate signature covering both [`EXO3_SIGNATURE`] and [`EXO2_SIGNATURE`].
///
/// [`resolve_exomizer`] enforces the exact variant rules and guarantees that any Exomizer 3.x
/// match in `load_addr..=load_end - 8` takes precedence over Exomizer 2.x.
const SIGNATURES: [Signature; 1] = [Signature {
    anchor: Anchor::ScanBackFromEnd { max_offset: 0xFFFF },
    pattern: sig![
        [0x69, 0xC8],
        [0x80, 0xC0],
        [0x0A, 0x34, 0x50],
        [0x10, 0xD0],
        __,
        __,
        __,
        [0xD0, 0x4C]
    ],
}];

/// Returns `true` if `mem` contains a complete Exomizer 3.x decruncher signature at `p`.
#[must_use]
pub fn matches_exo3_at(mem: &[u8], p: usize) -> bool {
    let Ok(addr) = u16::try_from(p) else {
        return false;
    };
    p >= 6
        && mem.len() > p + 8
        && mem[p - 6] == 0x4C
        && mem[p - 4] == 0x01
        && EXO3_SIGNATURE.matches_at(mem, addr)
}

/// Returns `true` if `mem` contains an Exomizer 1.x / 2.x signature at `p`.
#[must_use]
pub fn matches_exo2_at(mem: &[u8], p: usize) -> bool {
    let Ok(addr) = u16::try_from(p) else {
        return false;
    };
    EXO2_SIGNATURE.matches_at(mem, addr)
}

/// Resolves Exomizer 3.0, v3.02+, or 2.x at candidate `sig_match.anchor_addr`.
///
/// Preserves the legacy two-pass descending scan precedence:
/// 1. All Exomizer 3.x positions from `load_end - 8` down to `load_addr` are checked first.
/// 2. Only if no Exomizer 3.x match exists anywhere in `load_addr..=load_end - 8` is an
///    Exomizer 1.x / 2.x match accepted (also in descending order).
#[must_use]
fn resolve_exomizer(image: &LoadedImage<'_>, sig_match: &SignatureMatch) -> Option<ResolvedSpec> {
    let mem = image.mem;
    let p = usize::from(sig_match.anchor_addr);

    if matches_exo3_at(mem, p) {
        let is_exo_30 = p + 12 < mem.len() && mem[p + 8..=p + 12] == [0x08, 0x48, 0x20, 0x1A, 0x01];
        let name = if is_exo_30 {
            "Exomizer 3.0"
        } else {
            "Exomizer v3.02+"
        };

        let entry_point = if p + 0x17 < mem.len() && mem[p + 0x10] == 0x20 && mem[p + 0x15] == 0x4C
        {
            Some(AddrSource::Const(u16::from_le_bytes([
                mem[p + 0x16],
                mem[p + 0x17],
            ])))
        } else {
            None
        };

        return Some(ResolvedSpec::named(
            name,
            Hints {
                dep_addr: Some(AddrSource::Const(0x0100 | u16::from(mem[p - 5]))),
                entry_point,
                ..Hints::NONE
            },
        ));
    }

    if matches_exo2_at(mem, p) {
        // Because `Anchor::ScanBackFromEnd` visits positions in descending order, any Exomizer 3.x
        // match above `p` would have already been returned. Check whether an Exomizer 3.x match
        // exists below `p` (`load_addr..p`); if so, reject `p` so the scan continues down to the
        // 3.x match.
        let lower_bound = usize::from(image.load_addr);
        if (lower_bound..p).rev().any(|q| matches_exo3_at(mem, q)) {
            return None;
        }

        let dep_low = mem[p + 2];
        return Some(ResolvedSpec::named(
            "Exomizer 2.x",
            Hints {
                dep_addr: Some(AddrSource::Const(0x0100 | u16::from(dep_low))),
                start_addr: Some(AddrSource::BasicStart(0)),
                ..Hints::NONE
            },
        ));
    }

    None
}

/// Emulation-time hooks for Exomizer 1.x / 2.x / 3.x packers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExomizerHooks {
    /// Major version (`2` for Exomizer 1.x / 2.x, `3` for Exomizer 3.x).
    pub version: u8,
    /// Exomizer 3 sub-version (`0x30` for 3.0, `0x32` for v3.02+, `0` for 2.x).
    pub exo_ver: u8,
    /// Low byte of the depacker stack address (`dep_addr & 0xFF`, fallback `0xFE`).
    pub dep_zp: u8,
    /// Dynamic minimum start address tracked during `Phase::Decompress` (`>= $0200`).
    pub min_start: Option<u16>,
}

impl ExomizerHooks {
    /// Creates a new [`ExomizerHooks`] instance.
    #[must_use]
    pub const fn new(version: u8, exo_ver: u8, dep_zp: u8) -> Self {
        Self {
            version,
            exo_ver,
            dep_zp,
            min_start: None,
        }
    }
}

impl PackerHooks for ExomizerHooks {
    fn on_step(&mut self, ctx: &mut StepContext<'_>) {
        if ctx.phase != Phase::Decompress || self.version != 3 {
            return;
        }
        let pc = ctx.pc;
        if pc < u16::MAX {
            let op = ctx.peek(pc);
            let arg = ctx.peek(pc + 1);
            if (op == 0xB1 || op == 0x91) && (arg == self.dep_zp || arg == 0xFE) {
                let lo_addr = u16::from(arg);
                let hi_addr = u16::from(arg.wrapping_add(1));
                let mut val = u16::from(ctx.peek(lo_addr)) + (u16::from(ctx.peek(hi_addr)) << 8);
                let y_val = u16::from(ctx.registers.y);
                if self.exo_ver == 0x30 {
                    val = val.wrapping_add(y_val);
                } else {
                    val = val.wrapping_add(y_val).wrapping_add(1);
                }
                // Ignore Zero Page ($0000..$00FF) and Stack Page ($0100..$01FF) writes
                // so min_start tracks user RAM payload instead of depacker temporary variables.
                if val >= 0x0200 {
                    self.min_start = Some(self.min_start.map_or(val, |min| min.min(val)));
                }
            }
        }
    }

    fn refine(&self, ctx: &RefineContext<'_>, bounds: &mut Bounds) {
        if !ctx.system.is_c64() {
            return;
        }

        if let Some(min_s) = self.min_start
            && min_s < bounds.start_addr
        {
            bounds.start_addr = min_s;
        }

        let snapshot = ctx.snapshot;
        let mem = ctx.mem;
        let y_reg = ctx.registers.y;

        let mut exomizer_end_lo = None;
        let mut exomizer_end_hi = None;
        let mut exomizer_version = None;

        // Scan pre-emulation snapshot ascending across $0200..=$FFF0 for the Exomizer 3
        // decruncher routine sequence:
        //   p - 6: 0x4C (JMP)
        //   p - 4: 0x01 (high byte of stack jump, e.g. $01xx)
        //   p    : 0x69 0x80 (ADC #$80)
        //   p + 2: 0x0A (ASL A)
        //   p + 3: 0x10 0x0F (BPL +15)
        //   p + 5: 0x06 0xFD (ASL $FD)
        //   p + 7: 0xD0 (BNE)
        for p in 0x0200..=0xFFF0 {
            if p >= 6
                && snapshot.len() > p + 8
                && snapshot[p] == 0x69
                && snapshot[p + 1] == 0x80
                && snapshot[p + 2] == 0x0A
                && snapshot[p + 3] == 0x10
                && snapshot[p + 4] == 0x0F
                && snapshot[p + 5] == 0x06
                && snapshot[p + 6] == 0xFD
                && snapshot[p + 7] == 0xD0
                && snapshot[p - 6] == 0x4C
                && snapshot[p - 4] == 0x01
            {
                let p_idx = p - 5;
                let mut q = 2;
                if snapshot[p_idx - q] == 0x8A {
                    q += 1;
                }
                let elo = snapshot[p_idx - q];
                let ehi = snapshot[p - 1];
                let is_exo_30 = snapshot[p_idx - q - 1] == snapshot[p_idx - q - 3]
                    && snapshot[p_idx - q - 2] == snapshot[p_idx - q];
                let ev = if is_exo_30 { 0x30 } else { 0x32 };
                exomizer_end_lo = Some(elo);
                exomizer_end_hi = Some(ehi);
                exomizer_version = Some(ev);
                break;
            }
        }

        if let Some(ver) = exomizer_version
            && exomizer_end_lo.is_some()
            && mem.len() > 0xFF
        {
            let mut dyn_start = u16::from(mem[0xFE]) + (u16::from(mem[0xFF]) << 8);
            if ver == 0x30 {
                dyn_start = dyn_start.wrapping_add(u16::from(y_reg));
            } else {
                dyn_start = dyn_start.wrapping_add(u16::from(y_reg)).wrapping_add(1);
            }
            if dyn_start >= 0x0400 {
                bounds.start_addr = bounds.start_addr.min(dyn_start);
            }

            if let Some(end_lo) = exomizer_end_lo {
                let end_hi = exomizer_end_hi.unwrap_or_else(|| snapshot[0xFF]);
                let mut dyn_end = u16::from(end_lo) + (u16::from(end_hi) << 8);
                if ver == 0x32 {
                    dyn_end = dyn_end.wrapping_add(1);
                }
                if dyn_end == 0 {
                    dyn_end = 0xFF00;
                }
                if dyn_end > bounds.start_addr {
                    bounds.end_addr = dyn_end.saturating_sub(1);
                }
            }
        }
    }
}

/// Constructs [`ExomizerHooks`] for a matched Exomizer image (Ruling B3).
#[must_use]
pub fn create_exomizer_hooks(
    image: &LoadedImage<'_>,
    sig_match: &SignatureMatch,
) -> Box<dyn PackerHooks> {
    let (version, exo_ver) = match sig_match.spec_name {
        "Exomizer 2.x" => (2, 0),
        "Exomizer v3.02+" => (3, 0x32),
        _ => (3, 0x30),
    };
    let dep_zp = sig_match
        .resolved
        .hints
        .dep_addr
        .and_then(|src| src.resolve_static(image, sig_match.anchor_addr))
        .map_or(0xFE, |d| (d & 0xFF) as u8);
    Box::new(ExomizerHooks::new(version, exo_ver, dep_zp))
}

/// Synthetic self-test fixture loaded at `$0801` (30 bytes covering `$0801..=$081E`, `p = $0807`):
/// - `$0801..=$0806` (`p - 6..p`): `4C 34 01 00 00 00` (`JMP $0134`)
/// - `$0807..=$080E` (`p..p + 8`): `69 80 0A 10 0F 06 FD D0` (Exomizer 3.x core signature)
/// - `$080F..=$0813` (`p + 8..=p + 12`): `08 48 20 1A 01` (Exomizer 3.0 marker)
/// - `$0814..=$0816` (`p + 13..=p + 15`): `00 00 00`
/// - `$0817..=$081E` (`p + 0x10..=p + 0x17`): `20 00 01 00 00 4C 20 08` (`JSR $0100; ...; JMP $0820`)
const EXOMIZER_FIXTURE: [u8; 30] = [
    // p - 6..p ($0801..=$0806)
    0x4C, 0x34, 0x01, 0x00, 0x00, 0x00, // p..p + 8 ($0807..=$080E)
    0x69, 0x80, 0x0A, 0x10, 0x0F, 0x06, 0xFD, 0xD0,
    // p + 8..=p + 12 ($080F..=$0813: Exomizer 3.0 sub-signature)
    0x08, 0x48, 0x20, 0x1A, 0x01, // p + 13..=p + 15 ($0814..=$0816)
    0x00, 0x00, 0x00, // p + 0x10..=p + 0x17 ($0817..=$081E: entry_point = $0820)
    0x20, 0x00, 0x01, 0x00, 0x00, 0x4C, 0x20, 0x08,
];

/// Declarative specification for Exomizer (`3.0`, `v3.02+`, and `2.x` variants).
pub static SPEC: PackerSpec = PackerSpec {
    id: "exomizer",
    name: "Exomizer 3.0",
    priority: 160,
    systems: &[TargetSystem::C64],
    load_addr_max: None,
    signatures: &SIGNATURES,
    hints: Hints::NONE,
    patches: &[],
    resolve: Some(resolve_exomizer),
    hooks: Some(create_exomizer_hooks),
    fixture: &EXOMIZER_FIXTURE,
    fixture_load_addr: 0x0801,
};
