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
mod info;
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

pub use info::PackerInfo;
pub use registry::{
    DetectedPacker, PACKERS, REGISTRY, detect, detect_all, detect_all_in_specs, detect_in_specs,
    detect_spec, detect_spec_in, find_entry_by_name,
};
pub use spec::{
    AddrSource, Anchor, Bounds, CpuReg, FixtureMemory, Hints, HooksFactory, LoadedImage,
    MemoryPatch, PackerHooks, PackerId, PackerSpec, Pat, PatchAddr, PointerGuard, RefineContext,
    Registers, ResolveFn, ResolvedSpec, Signature, SignatureMatch, StepContext, StepFilter,
    is_valid_packer_id, sig, system_matches,
};
