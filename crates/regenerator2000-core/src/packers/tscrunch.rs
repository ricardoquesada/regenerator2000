//! TSCrunch (`TSCrunch v1.3+` and `TSCrunch v1.3+-X2`) packer specification and emulation hooks.

use crate::state::types::TargetSystem;
use crate::unpacker::machine::Phase;

use super::spec::{
    AddrSource, Anchor, Bounds, Hints, LoadedImage, PackerHooks, PackerSpec, RefineContext,
    ResolvedSpec, Signature, SignatureMatch, StepContext, sig,
};

/// Variant 1 (`TSCrunch v1.3+`, zero-page depacker at `$0002`) 14-byte signature at `$080D`.
///
/// Matches `SEI; LDX #imm; LDA abs,X; STA $00,X; DEX; BNE rel; JMP $0002`.
pub const V1_SIGNATURE: Signature = Signature {
    anchor: Anchor::Absolute(0x080D),
    pattern: sig![
        0x78, 0xA2, __, 0xBD, __, __, 0x95, 0x00, 0xCA, 0xD0, __, 0x4C, 0x02, 0x00
    ],
};

/// Variant 2 (`TSCrunch v1.3+-X2`, stack depacker at `$0100`) 19-byte signature at `$080D`.
///
/// Matches `SEI; LDA #$34; STA $01; LDX #imm; LDA abs,X; STA abs,X; DEX; BNE rel; JMP $0100`.
pub const V2_SIGNATURE: Signature = Signature {
    anchor: Anchor::Absolute(0x080D),
    pattern: sig![
        0x78, 0xA9, 0x34, 0x85, 0x01, 0xA2, __, 0xBD, __, __, 0x9D, __, __, 0xCA, 0xD0, __, 0x4C,
        0x00, 0x01
    ],
};

/// Secondary 6-byte sequence (`LDA #$37; STA $01; CLI; JMP $xxxx`) used to locate the
/// unpacked entry point in `q + 14..q + 0x80` (Variant 1) or `q + 19..q + 0x80` (Variant 2).
pub const ENTRY_SIGNATURE: Signature = Signature {
    anchor: Anchor::Absolute(0x080D),
    pattern: sig![0xA9, 0x37, 0x85, 0x01, 0x58, 0x4C],
};

/// Primary 19-byte candidate signature at `$080D` covering both [`V1_SIGNATURE`] and
/// [`V2_SIGNATURE`] and enforcing `mem.len() > 0x080D + 18`.
const SIGNATURES: [Signature; 1] = [Signature {
    anchor: Anchor::Absolute(0x080D),
    pattern: sig![
        0x78,
        [0xA2, 0xA9],
        __,
        [0xBD, 0x85],
        __,
        __,
        __,
        [0x00, 0xBD],
        __,
        __,
        __,
        __,
        __,
        [0x00, 0xCA],
        __,
        __,
        __,
        __,
        __
    ],
}];

/// Scans `q + start_offset..mem.len().min(q + 0x80)` (when `mem.len() >= q + 0x70`) for
/// [`ENTRY_SIGNATURE`] (`A9 37 85 01 58 4C lo hi`) and returns the 16-bit `JMP` operand.
#[must_use]
fn scan_entry_point(image: &LoadedImage<'_>, q: usize, start_offset: usize) -> Option<AddrSource> {
    let mem = image.mem;
    if mem.len() < q + 0x70 {
        return None;
    }
    let upper = mem.len().min(q + 0x80);
    for p in q + start_offset..upper {
        let Ok(addr) = u16::try_from(p) else {
            break;
        };
        if mem.len() >= p + 8 && ENTRY_SIGNATURE.matches_at(mem, addr) {
            let ep = image.read_u16_le(p + 6)?;
            return Some(AddrSource::Const(ep));
        }
    }
    None
}

/// Resolves TSCrunch Variant 1 (`TSCrunch v1.3+`) or Variant 2 (`TSCrunch v1.3+-X2`) at `$080D`.
#[must_use]
fn resolve_tscrunch(image: &LoadedImage<'_>, sig_match: &SignatureMatch) -> Option<ResolvedSpec> {
    let mem = image.mem;
    let q = sig_match.anchor_addr;
    let q_idx = usize::from(q);
    if mem.len() <= q_idx + 18 || image.load_addr > 0x0801 {
        return None;
    }

    if V1_SIGNATURE.matches_at(mem, q) {
        let entry_point = scan_entry_point(image, q_idx, 14);
        return Some(ResolvedSpec::named(
            "TSCrunch v1.3+",
            Hints {
                dep_addr: Some(AddrSource::Const(0x0002)),
                start_addr: Some(AddrSource::BasicStart(-1)),
                entry_point,
                ..Hints::NONE
            },
        ));
    }

    if V2_SIGNATURE.matches_at(mem, q) {
        let entry_point = scan_entry_point(image, q_idx, 19);
        return Some(ResolvedSpec::named(
            "TSCrunch v1.3+-X2",
            Hints {
                dep_addr: Some(AddrSource::Const(0x0100)),
                start_addr: Some(AddrSource::BasicStart(-1)),
                entry_point,
                ..Hints::NONE
            },
        ));
    }

    None
}

/// Extracts the high-memory stream relocation address (`reloc_start > $0800`) from `image`
/// for a matched TSCrunch stub:
/// - Variant 1 (`TSCrunch v1.3+`): little-endian word at `anchor_addr + 0x10` (`$081D..=$081E`)
/// - Variant 2 (`TSCrunch v1.3+-X2`): little-endian word at `anchor_addr + 0x18` (`$0825..=$0826`)
#[must_use]
pub fn resolve_reloc_start(image: &LoadedImage<'_>, sig_match: &SignatureMatch) -> Option<u16> {
    let q_idx = usize::from(sig_match.anchor_addr);
    let offset = if sig_match.spec_name == "TSCrunch v1.3+-X2" {
        0x18
    } else {
        0x10
    };
    let addr = image.read_u16_le(q_idx.checked_add(offset)?)?;
    (addr > 0x0800).then_some(addr)
}

/// Emulation-time hooks for TSCrunch (`v1.3+` and `v1.3+-X2`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TScrunchHooks {
    /// Start address of high-memory compressed stream relocation (`> $0800`).
    pub reloc_start: Option<u16>,
    /// Whether the initial stream relocation write flags have been cleared.
    pub reloc_cleared: bool,
}

impl TScrunchHooks {
    /// Creates a new [`TScrunchHooks`] instance.
    #[must_use]
    pub const fn new(reloc_start: Option<u16>) -> Self {
        Self {
            reloc_start,
            reloc_cleared: false,
        }
    }
}

impl PackerHooks for TScrunchHooks {
    fn on_step(&mut self, ctx: &mut StepContext<'_>) {
        if ctx.phase != Phase::Decompress || self.reloc_cleared {
            return;
        }
        let pc = ctx.pc;
        if (0x0021..=0x0090).contains(&pc) || (0x0121..=0x0190).contains(&pc) {
            if let Some(reloc) = self.reloc_start {
                ctx.forget_writes(reloc..=u16::MAX);
            }
            self.reloc_cleared = true;
        }
    }

    fn refine(&self, ctx: &RefineContext<'_>, bounds: &mut Bounds) {
        let mut last_written = bounds.start_addr;
        for a in (usize::from(bounds.start_addr)..=usize::from(bounds.end_addr)).rev() {
            if ctx.written.get(a).copied().unwrap_or(false) {
                last_written = a as u16;
                break;
            }
        }
        if last_written < bounds.end_addr {
            bounds.end_addr = last_written;
        }
    }
}

/// Constructs [`TScrunchHooks`] for a matched TSCrunch image (Ruling B3).
#[must_use]
pub fn create_tscrunch_hooks(
    image: &LoadedImage<'_>,
    sig_match: &SignatureMatch,
) -> Box<dyn PackerHooks> {
    Box::new(TScrunchHooks::new(resolve_reloc_start(image, sig_match)))
}

/// Synthetic self-test fixture loaded at `$0801` (31 bytes covering `$0801..=$081F`):
/// - `$0801..=$080C`: 12 zero padding bytes
/// - `$080D..=$081A`: 14-byte Variant 1 (`TSCrunch v1.3+`) signature at `$080D`
/// - `$081B..=$081C`: 2 padding bytes
/// - `$081D..=$081E` (`$080D + 0x10`): LE `reloc_start = $C000` (`00 C0`)
/// - `$081F` (`$080D + 18`): 1 trailing byte so `mem.len() > 0x080D + 18`
const TSCRUNCH_FIXTURE: [u8; 31] = [
    // $0801..=$080C (12 bytes)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // $080D..=$081A (14 bytes: Variant 1 signature)
    0x78, 0xA2, 0xCC, 0xBD, 0x1A, 0x08, 0x95, 0x00, 0xCA, 0xD0, 0xF8, 0x4C, 0x02, 0x00,
    // $081B..=$081C (2 bytes)
    0x00, 0x00, // $081D..=$081E (2 bytes: LE reloc_start = $C000)
    0x00, 0xC0, // $081F (1 byte)
    0x00,
];

/// Declarative specification for TSCrunch (`v1.3+` and `v1.3+-X2` variants).
pub static SPEC: PackerSpec = PackerSpec {
    id: "tscrunch",
    name: "TSCrunch v1.3+",
    priority: 110,
    systems: &[TargetSystem::C64],
    load_addr_max: Some(0x0801),
    signatures: &SIGNATURES,
    hints: Hints {
        dep_addr: Some(AddrSource::Const(0x0002)),
        start_addr: Some(AddrSource::BasicStart(-1)),
        ..Hints::NONE
    },
    patches: &[],
    resolve: Some(resolve_tscrunch),
    hooks: Some(create_tscrunch_hooks),
    fixture: &TSCRUNCH_FIXTURE,
    fixture_load_addr: 0x0801,
};
