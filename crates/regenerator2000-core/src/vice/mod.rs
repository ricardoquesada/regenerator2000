//! VICE Binary Monitor client, wire protocol, and debugger state.
//!
//! This module implements the TCP client (`client`), binary frame encoder and
//! decoder (`protocol`), debugger state container (`state`), and C64/C128 CIA
//! register decoder (`c64_hardware`).

pub mod c64_hardware;
pub mod client;
pub mod protocol;
pub mod state;

pub use c64_hardware::CiaState;
pub use client::{ViceClient, ViceEvent};
pub use protocol::{
    CheckpointInfo, MemoryGetResponse, Registers, ViceCommand, ViceMessage, parse_checkpoint_info,
    parse_memory_get, parse_registers,
};
pub use state::ViceState;
