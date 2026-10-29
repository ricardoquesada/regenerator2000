//! Dali 0.3.3 packer specification and emulation hooks.

use crate::state::types::TargetSystem;

use super::spec::{
    AddrSource, Anchor, Bounds, Hints, LoadedImage, PackerHooks, PackerSpec, PointerGuard,
    RefineContext, ResolvedSpec, Signature, SignatureMatch, sig,
};

/// Dali 0.3.3 21-byte signature at `$080D`.
///
/// Matches `SEI; LDX #$0B; TXS; LDY #$EC; PHA; LAX $xx,Y; ...; JMP $0003; .byte $34`.
pub const DALI_SIGNATURE: Signature = Signature {
    anchor: Anchor::Absolute(0x080D),
    pattern: sig![
        0x78, 0xA2, 0x0B, 0x9A, 0xA0, 0xEC, 0x48, 0xB7, __, __, __, __, __, __, __, __, __, 0x4C,
        0x03, 0x00, 0x34
    ],
};

const SIGNATURES: [Signature; 1] = [DALI_SIGNATURE];

const BASE_HINTS: Hints = Hints {
    dep_addr: Some(AddrSource::Const(0x0003)),
    // No start override. Dali rewrites the whole original
    // image, including its first byte one below the BASIC
    // start (`$0800` on the C64), so the generic output
    // detector, which scans from `System::ram_start()`,
    // already finds the original load address. This matches
    // both the original binary and unp64 (Design Doc Open
    // Question 1: the canonical start is the original load
    // address).
    start_addr: None,
    entry_point: Some(AddrSource::OperandLe { from_match: 0xFE }),
    ..Hints::NONE
};

/// Resolves Dali 0.3.3 dynamic zero-page end pointer at `anchor_addr + 0x44` and entry point
/// operand at `anchor_addr + 0xFE..=anchor_addr + 0xFF`.
#[must_use]
fn resolve_dali(image: &LoadedImage<'_>, sig_match: &SignatureMatch) -> Option<ResolvedSpec> {
    let q = usize::from(sig_match.anchor_addr);
    if image.mem.len() <= 0x090C || image.load_addr > 0x0801 || q < usize::from(image.load_addr) {
        return None;
    }
    let ptr_addr = u16::from(image.read_u8(q + 0x44)?);
    // Ensure entry_point operand at `q + 0xFE..=q + 0xFF` is readable.
    let _ = image.read_u16_le(q + 0xFE)?;

    Some(ResolvedSpec::from_hints(Hints {
        end_addr: Some(AddrSource::PointerAfterRun {
            ptr_addr,
            adjust: -1,
            guard: PointerGuard::NearDetectedEnd { slack: 512 },
        }),
        ..BASE_HINTS
    }))
}

/// Emulation-time hooks for Dali 0.3.3.
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

        // unp64 compatibility for Dali v0.3.3 / fast:
        // Dali copies its depacker to zero page and jumps to the entry point stored at $EB-$EC when done.
        // It leaves the compressed payload at the top of memory, which defeats standard gap trim.
        // We find the true end of decompressed data by finding the largest contiguous block of unwritten memory.
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

/// Constructs [`DaliHooks`] for a matched Dali image (Ruling B3).
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

/// Declarative specification for Dali 0.3.3.
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
    /// The path is resolved from `CARGO_MANIFEST_DIR`, so the test does not
    /// depend on the working directory, and a missing fixture fails the test
    /// instead of skipping it.
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

    /// T1.B.4: Dali's output starts at the original load address and keeps
    /// the padding byte, the original's first byte one below the BASIC start
    /// (Design Doc Open Question 1). For `c64_mule_1100.prg` that byte is
    /// `$20`, not `$00`, so it is compared with the original, not a literal.
    /// The unpacked image equals the original byte for byte.
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
