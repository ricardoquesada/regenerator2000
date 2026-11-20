//! 6502 emulation unpacker for compressed Commodore 8-bit programs.
//!
//! Tools like Dali, Exomizer, `PUCrunch`, `TSCrunch`, and `ByteBoozer` pack Commodore 8-bit
//! programs. This module emulates the MOS 6502 CPU in a target-system sandbox to run the
//! decompression stub. It records memory writes and bound provenance in [`UnpackReport`]
//! and extracts the unpacked binary.
//!
//! The algorithm follows the **unp64** utility and uses a two-phase state machine:
//!
//! - [`Phase::FindDepacker`](machine::Phase::FindDepacker) (Phase 1): Runs from the entry point
//!   until the program counter reaches the depacker stub or drops below the return boundary.
//! - [`Phase::Decompress`](machine::Phase::Decompress) (Phase 2): Runs the depacker stub and
//!   tracks RAM writes until execution reaches the unpacked program or a trapped exit vector.

pub mod bus;
pub mod cia;
pub mod detector;
pub mod engine;
pub mod machine;
mod runner;

pub use crate::error::UnpackError;
pub use crate::packers::PackerId;
pub use bus::{C64Bus, UnpackerMemory};
pub use engine::{BoundSource, ExitReason, TRACE_TAIL_CAPACITY, UnpackEngine, UnpackReport};
pub use runner::{UnpackConfig, UnpackResult, find_sys_address, unpack};
