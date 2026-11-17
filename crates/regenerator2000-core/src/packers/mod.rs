//! 6502 binary packer detection and unpacker lifecycle specifications.
//!
//! Defines 26 Commodore 8-bit packer families (currently targeting
//! [`TargetSystem::C64`](crate::state::types::TargetSystem::C64)) and [`PackerInfo`] detection-time
//! hints. [`REGISTRY`] lists entries in ascending [`PackerSpec::priority`] order matching `unp64`
//! (`_Scanners.c`) so [`detect`] returns the highest-priority match.

pub mod action_replay;
pub mod alz64;
pub mod antiram;
pub mod byteboozer;
pub mod card_cruncher;
pub mod ccs;
pub mod comp64k;
pub mod cruel_cruncher;
pub mod dali;
pub mod eagle;
pub mod eca;
pub mod exomizer;
pub mod fast_packer;
pub mod final_cartridge;
pub mod mc_cracken;
pub mod pucrunch;
pub mod registry;
pub mod sixpack_gp;
pub mod spark_packer;
pub mod spec;
pub mod super_cruncher;
pub mod tbc;
pub mod time_cruncher;
pub mod tiny_crunch;
pub mod triad;
pub mod tscrunch;
pub mod turbo_cruncher;
pub mod zx02;

pub use registry::{
    PACKERS, REGISTRY, detect, detect_all, detect_all_in_specs, detect_in_specs, detect_spec,
    detect_spec_in, find_entry_by_name,
};
pub use spec::{
    AddrSource, Anchor, Bounds, CpuReg, Hints, HooksFactory, LoadedImage, MemoryPatch, PackerHooks,
    PackerId, PackerSpec, Pat, PatchAddr, PointerGuard, RefineContext, Registers, ResolveFn,
    ResolvedSpec, Signature, SignatureMatch, StepContext, StepFilter, is_valid_packer_id, sig,
    system_matches,
};

/// Information about a detected packer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackerInfo {
    /// Human-readable name of the packer.
    pub name: &'static str,
    /// Depacker start address.
    pub dep_addr: Option<u16>,
    /// Inclusive unpacked output start address override.
    pub start_addr: Option<u16>,
    /// Inclusive unpacked output end address override.
    pub end_addr: Option<u16>,
    /// Entry point address override.
    pub entry_point: Option<u16>,
    /// Zero-page or RAM low-byte address of the post-run end pointer (high byte at `addr.wrapping_add(1)`).
    pub end_addr_ptr: Option<u16>,
}

impl PackerInfo {
    /// Resolves detection-time [`PackerInfo`] from a [`LoadedImage`] and [`SignatureMatch`].
    ///
    /// # Examples
    /// ```
    /// use regenerator2000_core::packers::{LoadedImage, PackerInfo, eagle};
    ///
    /// let (mem, load_addr, load_end) = eagle::SPEC.build_fixture_memory();
    /// let image = LoadedImage::for_c64(&mem, load_addr, load_end);
    /// let sig_match = eagle::SPEC.detect(&image).unwrap();
    /// let info = PackerInfo::resolve(&image, &sig_match);
    /// assert_eq!(info.name, "Eagle Cruncher");
    /// ```
    #[must_use]
    pub fn resolve(image: &LoadedImage<'_>, sig_match: &SignatureMatch) -> Self {
        let hints = sig_match.resolved.hints;
        let anchor_addr = sig_match.anchor_addr;

        let dep_addr = hints
            .dep_addr
            .and_then(|src| src.resolve_static(image, anchor_addr));
        let start_addr = hints
            .start_addr
            .and_then(|src| src.resolve_static(image, anchor_addr));
        let end_addr = hints
            .end_addr
            .and_then(|src| src.resolve_static(image, anchor_addr));
        let entry_point = hints
            .entry_point
            .and_then(|src| src.resolve_static(image, anchor_addr));
        let end_addr_ptr = hints.end_addr.and_then(AddrSource::pointer_addr);

        Self {
            name: sig_match.spec_name,
            dep_addr,
            start_addr,
            end_addr,
            entry_point,
            end_addr_ptr,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packer_info_resolve_extracts_static_hints_and_pointer_addr() {
        let mut mem = vec![0; 0x1000];
        let image = LoadedImage::for_c64(&mem, 0x0801, 0x1000);
        assert!(detect(&image).is_none());

        let q = 0x080D;
        mem[q..q + 10]
            .copy_from_slice(&[0x78, 0xA9, 0x34, 0x85, 0x01, 0x00, 0xB7, 0xBD, 0x1E, 0x08]);
        mem[0x0886..=0x0887].copy_from_slice(&0x0801u16.to_le_bytes());
        mem[0x08CB..=0x08CC].copy_from_slice(&0x2000u16.to_le_bytes());

        let image = LoadedImage::for_c64(&mem, 0x0801, 0x0950);
        let sig_match = detect(&image).unwrap();
        let info = PackerInfo::resolve(&image, &sig_match);
        assert_eq!(info.name, "ByteBoozer");
        assert_eq!(info.dep_addr, Some(0x0010));
        assert_eq!(info.start_addr, Some(0x0801));
        assert_eq!(info.end_addr, None);
        assert_eq!(info.entry_point, Some(0x2000));
        assert_eq!(info.end_addr_ptr, Some(0x0077));
    }
}
