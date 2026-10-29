//! 6502 binary packer detection and unpacker lifecycle strategies.

pub mod action_replay;
pub mod alz64;
pub mod antiram;
pub mod byteboozer;
pub mod card_cruncher;
pub mod ccs;
pub mod cruel_cruncher;
pub mod dali;
pub mod eagle;
pub mod eca;
pub mod exomizer;
pub mod final_cartridge;
pub mod mc_cracken;
pub mod pucrunch;
pub mod registry;
pub mod spec;
pub mod super_cruncher;
pub mod tbc;
pub mod time_cruncher;
pub mod tiny_crunch;
pub mod triad;
pub mod tscrunch;
pub mod turbo_cruncher;

pub use registry::{
    PACKERS, REGISTRY, detect, detect_all, detect_all_in_specs, detect_in_specs, detect_spec,
    detect_spec_in, find_entry_by_name, registry_specs,
};
pub use spec::{
    AddrSource, Anchor, Bounds, Hints, HooksFactory, LoadedImage, MemoryPatch, PackerHooks,
    PackerId, PackerSpec, Pat, PatchAddr, PointerGuard, PointerMonitor, RefineContext, Reg,
    Registers, ResolveFn, ResolvedSpec, Signature, SignatureMatch, StepContext, is_valid_packer_id,
    sig, system_matches,
};

/// Information about a detected packer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackerInfo {
    /// Human-readable name of the packer.
    pub name: &'static str,
    /// Depacker start address.
    pub dep_addr: Option<u16>,
    /// Unpacked output start address override.
    pub start_addr: Option<u16>,
    /// Unpacked output end address override.
    pub end_addr: Option<u16>,
    /// Entry point address override.
    pub entry_point: Option<u16>,
    /// Zero-page or memory pointer address containing end address.
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
        let end_addr_ptr = match hints.end_addr {
            Some(AddrSource::PointerAfterRun { ptr_addr, .. }) => Some(ptr_addr),
            _ => None,
        };

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

    fn detect_info(mem: &[u8], load_addr: u16, load_end: u16) -> Option<PackerInfo> {
        let image = LoadedImage::for_c64(mem, load_addr, load_end);
        let (_spec, sig_match) = detect_spec(&image)?;
        Some(PackerInfo::resolve(&image, &sig_match))
    }

    #[test]
    fn test_detect_packer_none() {
        let mem = vec![0; 0x1000];
        assert!(detect_info(&mem, 0x0801, 0x1000).is_none());
    }

    #[test]
    fn test_detect_exomizer_v3_0() {
        let mut mem = vec![0; 0x1000];
        let p = 0x0900;
        mem[p] = 0x69;
        mem[p + 1] = 0x80;
        mem[p + 2] = 0x0A;
        mem[p + 3] = 0x10;
        mem[p + 4] = 0x0F;
        mem[p + 5] = 0x06;
        mem[p + 6] = 0xFD;
        mem[p + 7] = 0xD0;

        mem[p + 8] = 0x08;
        mem[p + 9] = 0x48;
        mem[p + 10] = 0x20;
        mem[p + 11] = 0x1A;
        mem[p + 12] = 0x01;

        mem[p - 6] = 0x4C;
        mem[p - 5] = 0x34;
        mem[p - 4] = 0x01;

        mem[p + 0x10] = 0x20;
        mem[p + 0x11] = 0x00;
        mem[p + 0x12] = 0x01;
        mem[p + 0x15] = 0x4C;
        mem[p + 0x16] = 0x20;
        mem[p + 0x17] = 0x08;

        let info = detect_info(&mem, 0x0801, 0x0950).unwrap();
        assert_eq!(info.name, "Exomizer 3.0");
        assert_eq!(info.dep_addr, Some(0x0134));
        assert_eq!(info.start_addr, None);
        assert_eq!(info.entry_point, Some(0x0820));
    }

    #[test]
    fn test_detect_exomizer_v3_02() {
        let mut mem = vec![0; 0x1000];
        let p = 0x0900;
        mem[p] = 0x69;
        mem[p + 1] = 0x80;
        mem[p + 2] = 0x0A;
        mem[p + 3] = 0x10;
        mem[p + 4] = 0x0F;
        mem[p + 5] = 0x06;
        mem[p + 6] = 0xFD;
        mem[p + 7] = 0xD0;

        mem[p + 8] = 0x99;

        mem[p - 6] = 0x4C;
        mem[p - 5] = 0x34;
        mem[p - 4] = 0x01;

        let info = detect_info(&mem, 0x0801, 0x0950).unwrap();
        assert_eq!(info.name, "Exomizer v3.02+");
    }

    #[test]
    fn test_detect_exomizer_v1_v2() {
        let mut mem = vec![0; 0x1000];
        let p = 0x0900;
        mem[p] = 0xC8;
        mem[p + 1] = 0xC0;
        mem[p + 2] = 0x50;
        mem[p + 3] = 0xD0;
        mem[p + 7] = 0x4C;

        let info = detect_info(&mem, 0x0801, 0x0950).unwrap();
        assert_eq!(info.name, "Exomizer 2.x");
        assert_eq!(info.dep_addr, Some(0x0150));
        assert_eq!(info.start_addr, Some(0x0801));
    }

    #[test]
    fn test_detect_pucrunch() {
        let mut mem = vec![0; 0x1000];
        mem[0x813] = 0x85;
        mem[0x814] = 0x01;
        mem[0x815] = 0xA2;
        mem[0x816] = 0x34;
        mem[0x817] = 0xBD;
        mem[0x818] = 0x42;
        mem[0x819] = 0x08;
        mem[0x81A] = 0x9D;
        mem[0x81B] = 0xFF;
        mem[0x81C] = 0x01;
        mem[0x81D] = 0xCA;
        mem[0x81E] = 0xD0;

        mem[0x841] = 0x16;
        mem[0x842] = 0x01;

        mem[0x879] = 0x01;
        mem[0x87A] = 0x08;

        let ep = 0x0920;
        mem[ep] = 0xA5;
        mem[ep + 1] = 0xFA;
        mem[ep + 2] = 0x85;
        mem[ep + 3] = 0x2D;
        mem[ep + 4] = 0xA5;
        mem[ep + 5] = 0xFB;
        mem[ep + 6] = 0x85;
        mem[ep + 7] = 0x2E;
        mem[ep + 0x0A] = 0x00;
        mem[ep + 0x0B] = 0x20;

        let info = detect_info(&mem, 0x0801, 0x0950).unwrap();
        assert_eq!(info.name, "PUCrunch");
        assert_eq!(info.dep_addr, Some(0x0116));
        assert_eq!(info.start_addr, Some(0x0801));
        assert_eq!(info.entry_point, Some(0x2000));
        assert_eq!(info.end_addr_ptr, Some(0x00FA));
    }

    #[test]
    fn test_detect_time_cruncher() {
        let mut mem = vec![0; 0x1000];
        mem[0x810] = 0x78;
        mem[0x811] = 0xA9;
        mem[0x812] = 0x34;
        mem[0x813] = 0x85;
        mem[0x814] = 0x01;
        mem[0x815] = 0xA0;
        mem[0x816] = 0xC4;
        mem[0x817] = 0xB9;

        let info = detect_info(&mem, 0x0801, 0x0950).unwrap();
        assert_eq!(info.name, "Time Cruncher");
        assert_eq!(info.dep_addr, Some(0x0100));
        assert_eq!(info.start_addr, Some(0x0801));
    }

    #[test]
    fn test_detect_dali() {
        let mut mem = vec![0; 0x1000];
        let q = 0x080D;
        mem[q] = 0x78;
        mem[q + 1] = 0xA2;
        mem[q + 2] = 0x0B;
        mem[q + 3] = 0x9A;
        mem[q + 4] = 0xA0;
        mem[q + 5] = 0xEC;
        mem[q + 6] = 0x48;
        mem[q + 7] = 0xB7;
        mem[q + 0x11] = 0x4C;
        mem[q + 0x12] = 0x03;
        mem[q + 0x13] = 0x00;
        mem[q + 0x14] = 0x34;

        mem[q + 0xFE] = 0x00;
        mem[q + 0xFF] = 0x10;
        mem[q + 0x44] = 0x6B;

        let info = detect_info(&mem, 0x0801, 0x0950).unwrap();
        assert_eq!(info.name, "Dali");
        assert_eq!(info.dep_addr, Some(0x0003));
        // T1.B.1: no start hint. The generic output detector yields the
        // original load address (`$0800` on the C64, Design Doc Open
        // Question 1); `dali::tests` pins the resulting payload.
        assert_eq!(info.start_addr, None);
        assert_eq!(info.entry_point, Some(0x1000));
        assert_eq!(info.end_addr_ptr, Some(0x006B));
    }

    #[test]
    fn test_detect_byte_boozer() {
        let mut mem = vec![0; 0x1000];
        let q = 0x080D;
        mem[q] = 0x78;
        mem[q + 1] = 0xA9;
        mem[q + 2] = 0x34;
        mem[q + 3] = 0x85;
        mem[q + 6] = 0xB7;
        mem[q + 7] = 0xBD;
        mem[q + 8] = 0x1E;
        mem[q + 9] = 0x08;

        mem[0x886] = 0x01;
        mem[0x887] = 0x08;

        mem[0x8CB] = 0x00;
        mem[0x8CC] = 0x20;

        let info = detect_info(&mem, 0x0801, 0x0950).unwrap();
        assert_eq!(info.name, "ByteBoozer");
        assert_eq!(info.dep_addr, Some(0x0010));
        assert_eq!(info.start_addr, Some(0x0801));
        assert_eq!(info.entry_point, Some(0x2000));
        assert_eq!(info.end_addr_ptr, Some(0x0077));
    }

    #[test]
    fn test_detect_alz64_quiss() {
        let mut mem = vec![0; 0x1000];
        mem[0x80b] = 0xA2;
        mem[0x80c] = 0x00;
        mem[0x80d] = 0x78;
        mem[0x80e] = 0xB5;
        mem[0x814] = 0x00;
        mem[0x815] = 0x09;
        mem[0x819] = 0xD0;
        mem[0x81a] = 0xF3;
        mem[0x81b] = 0xA2;
        mem[0x81c] = 0x03;
        mem[0x824] = 0xA2;
        mem[0x825] = 0x10;
        mem[0x826] = 0x89;
        mem[0x827] = 0x38;
        mem[0x82c] = 0xF1;
        mem[0x82d] = 0x4C;
        mem[0x82e] = 0x5E;
        mem[0x82f] = 0x00;

        let p = 0x0900;
        mem[p + 0xf1] = 0x4C;
        mem[p + 0xf2] = 0x00;
        mem[p + 0xf3] = 0x20;

        let info = detect_info(&mem, 0x0801, 0x0950).unwrap();
        assert_eq!(info.name, "ALZ64/Quiss");
        assert_eq!(info.dep_addr, Some(0x005E));
        assert_eq!(info.start_addr, Some(0x080b));
        assert_eq!(info.entry_point, Some(0x2000));
        assert_eq!(info.end_addr_ptr, Some(0x00CF));
    }

    #[test]
    fn test_detect_alz64_kabuto() {
        let mut mem = vec![0; 0x1000];
        mem[0x80c] = 0x00;
        mem[0x80d] = 0x78;
        mem[0x80e] = 0x86;
        mem[0x813] = 0x00;
        mem[0x814] = 0x09;
        mem[0x818] = 0xCA;
        mem[0x819] = 0xD0;
        mem[0x81a] = 0xF7;
        mem[0x81b] = 0xCE;
        mem[0x822] = 0xD0;
        mem[0x823] = 0xEE;
        mem[0x824] = 0xA2;
        mem[0x825] = 0x03;
        mem[0x835] = 0xF1;
        mem[0x836] = 0x4C;
        mem[0x837] = 0x5E;
        mem[0x838] = 0x00;

        let p = 0x0900;
        mem[p + 0xfd] = 0x4C;
        mem[p + 0xfe] = 0x00;
        mem[p + 0xff] = 0x20;

        let info = detect_info(&mem, 0x0801, 0x0950).unwrap();
        assert_eq!(info.name, "ALZ64/Kabuto");
        assert_eq!(info.dep_addr, Some(0x005E));
        assert_eq!(info.start_addr, Some(0x080b));
        assert_eq!(info.entry_point, Some(0x2000));
        assert_eq!(info.end_addr_ptr, Some(0x00CF));
    }
}
