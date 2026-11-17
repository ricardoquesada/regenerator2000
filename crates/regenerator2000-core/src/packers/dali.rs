//! Dali (`Dali`, `v0.3.3` fast/small, `v0.3.4 / sfx`, and `v0.3.5 / sfx`) packer specification
//! and emulation hooks.
//!
//! Targets [`TargetSystem::C64`] images with `load_addr <= $0801`. Probes `$080D` first, then
//! scans `load_addr..=load_addr + 0x0E` for sysless stubs. Checks `v0.3.5 / sfx` and sysless
//! `v0.3.3 / fast` (without a BASIC `SYS` line) before standard `0.3.3` because all three share
//! `78 A2 0B 9A`. [`DaliHooks`] trims residual top-of-memory payload writes across the largest
//! unwritten gap.

use crate::state::types::TargetSystem;
use crate::unpacker::find_sys_address;

use super::spec::{
    AddrSource, Anchor, Bounds, Hints, LoadedImage, MemoryPatch, PackerHooks, PackerSpec,
    PatchAddr, PointerGuard, RefineContext, ResolvedSpec, Signature, SignatureMatch, sig,
};

/// Dali 0.3.3 21-byte signature at `$080D`.
///
/// Matches `SEI, LDX #$0B, TXS, LDY #$EC, PHA, LAX $xx,Y, ..., JMP $0003, .byte $34`.
pub const DALI_SIGNATURE: Signature = Signature {
    anchor: Anchor::Absolute(0x080D),
    pattern: sig![
        0x78, 0xA2, 0x0B, 0x9A, 0xA0, 0xEC, 0x48, 0xB7, __, __, __, __, __, __, __, __, __, 0x4C,
        0x03, 0x00, 0x34
    ],
};

const DALI_CANDIDATES: [Anchor; 2] = [
    Anchor::Absolute(0x080D),
    Anchor::ScanFromLoad { max_offset: 0x0E },
];

/// Prefilter covering `LDX #$0B` (`0.3.3` fast / `0.3.5 sfx`), `LDY #$B7` (`0.3.3` small),
/// and `LDY #$BE` (`0.3.4 sfx`).
const SIGNATURES: [Signature; 1] = [Signature {
    anchor: Anchor::Candidates(&DALI_CANDIDATES),
    pattern: sig![0x78, [0xA2, 0xA0], [0x0B, 0xB7, 0xBE]],
}];

/// Zero-page restore patch for sysless Dali 0.3.3 fast (unp64 `ZX0.c:122`: `BIT` `$24` at `q + $F7`).
const DALI_033F_PATCHES: [MemoryPatch; 1] = [MemoryPatch {
    addr: PatchAddr::FromMatch(0xF7),
    expected: None,
    bytes: &[0x24],
}];

/// Zero-page restore patch for Dali 0.3.5 SFX (unp64 `ZX0.c:198`: `BIT` `$24` at `q + $FE`).
const DALI_035_PATCHES: [MemoryPatch; 1] = [MemoryPatch {
    addr: PatchAddr::FromMatch(0xFE),
    expected: None,
    bytes: &[0x24],
}];

/// Builds a post-run zero-page `end_addr` hint when `ptr_byte != 0`, avoiding `$0000/$0001`.
#[must_use]
fn zp_end_source(ptr_byte: u8) -> Option<AddrSource> {
    if ptr_byte == 0 {
        return None;
    }
    Some(AddrSource::PointerAfterRun {
        ptr_addr: u16::from(ptr_byte),
        adjust: -1,
        guard: PointerGuard::NearDetectedEnd { slack: 512 },
    })
}

const BASE_HINTS: Hints = Hints {
    dep_addr: Some(AddrSource::Const(0x0003)),
    // No start override. Dali rewrites the whole original image, including its first
    // byte one below the BASIC start (`$0800` on the C64). The generic output detector
    // scans from `System::ram_start()` and finds the original load address.
    start_addr: None,
    entry_point: Some(AddrSource::OperandLe { from_match: 0xFE }),
    ..Hints::NONE
};

/// Resolves Dali `0.3.3` (standard and sysless fast/small), `0.3.4 / sfx`, and `0.3.5 / sfx`.
#[must_use]
fn resolve_dali(image: &LoadedImage<'_>, sig_match: &SignatureMatch) -> Option<ResolvedSpec> {
    let q_addr = sig_match.anchor_addr;
    let q = usize::from(q_addr);
    if image.load_addr > 0x0801 || q < usize::from(image.load_addr) {
        return None;
    }

    // Check Dali 0.3.5 SFX before standard 0.3.3 because both share `78 A2 0B 9A` and `4C 03 00 34` (unp64 `ZX0.c:177-201`).
    if image.read_u32_le(q) == Some(0x9A0B_A278)
        && image.read_u32_le(q + 0x0C) == Some(0xD088_0096)
        && image.read_u32_le(q + 0x11) == Some(0x3400_034C)
        && image.read_u8(q + 0x2D) == Some(0x06)
        && image.read_u8(q + 0x104) == Some(0x4C)
        && let Some(ptr_byte) = image.read_u8(q + 0x44)
        && image.read_u16_le(q + 0x105).is_some()
    {
        return Some(ResolvedSpec::with_patches(
            "Dali v0.3.5 / sfx",
            Hints {
                end_addr: zp_end_source(ptr_byte),
                entry_point: Some(AddrSource::OperandLe { from_match: 0x105 }),
                forced_entry: Some(AddrSource::Const(q_addr)),
                ..BASE_HINTS
            },
            &DALI_035_PATCHES,
        ));
    }

    // Sysless Dali 0.3.3 fast (unp64 `ZX0.c:104-125`). Check before standard `$080D` Dali
    // whenever `q_addr != 0x080D` or no BASIC `SYS` line exists so stubs starting at `$080D` still
    // receive `forced_entry` and `DALI_033F_PATCHES`.
    if (q_addr != 0x080D
        || find_sys_address(image.mem, image.system.default_basic_start()).is_none())
        && image.read_u32_le(q) == Some(0x9A0B_A278)
        && image.read_u32_le(q + 4) == Some(0xB748_ECA0)
        && image.read_u32_le(q + 0x11) == Some(0x3400_034C)
        && image.read_u8(q + 0xFD) == Some(0x4C)
        && let Some(ptr_byte) = image.read_u8(q + 0x44)
        && image.read_u16_le(q + 0xFE).is_some()
    {
        return Some(ResolvedSpec::with_patches(
            "Dali v0.3.3 / fast",
            Hints {
                end_addr: zp_end_source(ptr_byte),
                forced_entry: Some(AddrSource::Const(q_addr)),
                ..BASE_HINTS
            },
            &DALI_033F_PATCHES,
        ));
    }

    // Standard Dali 0.3.3 fast at `$080D` (uses BASIC `SYS` entry and needs no zero-page patch).
    if q_addr == 0x080D && DALI_SIGNATURE.matches_at(image.mem, 0x080D) {
        if image.mem.len() <= 0x090C {
            return None;
        }
        let ptr_byte = image.read_u8(q + 0x44)?;
        let _ = image.read_u16_le(q + 0xFE)?;

        return Some(ResolvedSpec::from_hints(Hints {
            end_addr: zp_end_source(ptr_byte),
            ..BASE_HINTS
        }));
    }

    // Dali 0.3.3 small (unp64 `ZX0.c:129-151`).
    if image.read_u32_le(q) == Some(0xBEB7_A078)
        && image.read_u32_le(q + 6) == Some(0xD088_0096)
        && image.read_u32_le(q + 0x0B) == Some(0x3400_034C)
        && image.read_u8(q + 0x24) == Some(0x06)
        && image.read_u8(q + 0xC2) == Some(0x4C)
        && let Some(ptr_byte) = image.read_u8(q + 0x3A)
        && image.read_u16_le(q + 0xC3).is_some()
    {
        return Some(ResolvedSpec::named(
            "Dali v0.3.3 / small",
            Hints {
                end_addr: zp_end_source(ptr_byte),
                entry_point: Some(AddrSource::OperandLe { from_match: 0xC3 }),
                forced_entry: Some(AddrSource::Const(q_addr)),
                ..BASE_HINTS
            },
        ));
    }

    // Dali 0.3.4 SFX (unp64 `ZX0.c:153-175`).
    if image.read_u32_le(q) == Some(0xBEBE_A078)
        && image.read_u32_le(q + 6) == Some(0xD088_0096)
        && image.read_u32_le(q + 0x0B) == Some(0x3400_034C)
        && image.read_u8(q + 0x24) == Some(0x06)
        && image.read_u8(q + 0xC9) == Some(0x4C)
        && let Some(ptr_byte) = image.read_u8(q + 0x32)
        && image.read_u16_le(q + 0xCA).is_some()
    {
        return Some(ResolvedSpec::named(
            "Dali v0.3.4 / sfx",
            Hints {
                end_addr: zp_end_source(ptr_byte),
                entry_point: Some(AddrSource::OperandLe { from_match: 0xCA }),
                forced_entry: Some(AddrSource::Const(q_addr)),
                ..BASE_HINTS
            },
        ));
    }

    None
}

/// Emulation-time hooks for the Dali packer family.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DaliHooks;

impl DaliHooks {
    /// Creates a new [`DaliHooks`] instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl PackerHooks for DaliHooks {
    fn refine(&self, ctx: &RefineContext<'_>, bounds: &mut Bounds) {
        if !ctx.system.is_c64() {
            return;
        }

        let start_addr = usize::from(bounds.start_addr);

        // Dali copies its depacker to zero page and jumps to the entry point at $EB..=$EC.
        // It leaves the compressed payload at the top of memory, which defeats standard gap trim.
        // Find the true end of decompressed data before the largest unwritten memory block.
        if ctx.mem.len() >= 0xED
            && ctx.mem[0xEA] == 0x4C
            && bounds.entry_point == u16::from_le_bytes([ctx.mem[0xEB], ctx.mem[0xEC]])
        {
            let mut max_gap_len = 0;
            let mut max_gap_start = 0;
            let mut current_gap_len = 0;
            let mut current_gap_start = 0;

            for (i, &is_written) in ctx
                .written
                .iter()
                .enumerate()
                .take(0x10000)
                .skip(start_addr)
            {
                if !is_written {
                    if current_gap_len == 0 {
                        current_gap_start = i;
                    }
                    current_gap_len += 1;
                } else {
                    if current_gap_len > max_gap_len {
                        max_gap_len = current_gap_len;
                        max_gap_start = current_gap_start;
                    }
                    current_gap_len = 0;
                }
            }
            if current_gap_len > max_gap_len {
                max_gap_len = current_gap_len;
                max_gap_start = current_gap_start;
            }

            if max_gap_len > 256 {
                let e = max_gap_start.saturating_sub(1);
                if e >= start_addr {
                    bounds.end_addr = e as u16;
                }
            }
        }
    }
}

/// Constructs [`DaliHooks`] for a matched Dali image.
#[must_use]
pub fn create_dali_hooks(
    _image: &LoadedImage<'_>,
    _sig_match: &SignatureMatch,
) -> Box<dyn PackerHooks> {
    Box::new(DaliHooks::new())
}

/// Synthetic self-test fixture loaded at `$0801` (33 bytes covering `$0801..=$0821`):
/// - `$0801..=$080C`: 12 zero padding bytes
/// - `$080D..=$0821`: 21-byte [`DALI_SIGNATURE`] at `$080D`
const DALI_FIXTURE: [u8; 33] = [
    // $0801..=$080C (12 bytes)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // $080D..=$0821 (21 bytes)
    0x78, 0xA2, 0x0B, 0x9A, 0xA0, 0xEC, 0x48, 0xB7, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x4C, 0x03, 0x00, 0x34,
];

/// Declarative specification for the Dali packer family (`0.3.3`–`0.3.5`).
pub static SPEC: PackerSpec = PackerSpec {
    id: "dali",
    name: "Dali",
    priority: 120,
    systems: &[TargetSystem::C64],
    load_addr_max: Some(0x0801),
    signatures: &SIGNATURES,
    hints: BASE_HINTS,
    patches: &[],
    resolve: Some(resolve_dali),
    hooks: Some(create_dali_hooks),
    fixture: &DALI_FIXTURE,
    fixture_load_addr: 0x0801,
};

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::unpacker::{UnpackConfig, unpack};

    /// Reads a `tests/6502` fixture as a `(load address, payload)` pair.
    ///
    /// Resolves the path from `CARGO_MANIFEST_DIR`. A missing fixture fails
    /// the test instead of skipping it.
    fn read_prg(name: &str) -> (u16, Vec<u8>) {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/6502")
            .join(name);
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|e| panic!("cannot read PRG fixture {}: {e}", path.display()));
        assert!(bytes.len() > 2, "PRG fixture {name} has no payload");
        let load_addr = u16::from_le_bytes([bytes[0], bytes[1]]);
        (load_addr, bytes[2..].to_vec())
    }

    /// Verifies that the Dali output starts at the original load address and
    /// keeps the padding byte one below the BASIC start.
    #[test]
    fn dali_unpacked_payload_includes_padding_byte() {
        let (packed_load, packed) = read_prg("c64_mule.dali.prg");
        let (original_load, original) = read_prg("c64_mule_1100.prg");

        let result = unpack(&packed, packed_load, &UnpackConfig::default(), None)
            .unwrap_or_else(|e| panic!("c64_mule.dali.prg failed to unpack: {e}"));

        assert_eq!(result.packer_name.as_deref(), Some("Dali"));
        assert_eq!(result.start_addr, original_load);
        assert_eq!(result.start_addr, packed_load - 1);
        assert_eq!(result.data.len(), 38170);
        assert_eq!(result.data[0], original[0], "the padding byte must be kept");
        assert_eq!(result.data, original);
    }
}
