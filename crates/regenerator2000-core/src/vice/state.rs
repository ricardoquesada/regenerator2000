//! Live debugger state, register snapshots, and checkpoint records for VICE.

use super::protocol::ViceCpuOp;

/// CPU operation that triggers a VICE checkpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakpointKind {
    /// Stops when the CPU executes an instruction at the target address.
    Exec,
    /// Stops when the CPU reads from the target address.
    Load,
    /// Stops when the CPU writes to the target address.
    Store,
    /// Stops when the CPU reads from or writes to the target address.
    LoadStore,
}

impl BreakpointKind {
    /// Converts a VICE `cpu_operation` bitmask into a [`BreakpointKind`].
    ///
    /// Maps `0x01` to [`BreakpointKind::Load`], `0x02` to [`BreakpointKind::Store`],
    /// and `0x03` to [`BreakpointKind::LoadStore`]. All other values map to
    /// [`BreakpointKind::Exec`].
    #[must_use]
    pub fn from_cpu_op(op: u8) -> Self {
        const LOAD_STORE: u8 = ViceCpuOp::LOAD | ViceCpuOp::STORE;
        match op {
            ViceCpuOp::LOAD => Self::Load,
            ViceCpuOp::STORE => Self::Store,
            LOAD_STORE => Self::LoadStore,
            _ => Self::Exec,
        }
    }

    /// Returns the short UI label (`"X"`, `"R"`, `"W"`, or `"RW"`) for this kind.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Exec => "X",
            Self::Load => "R",
            Self::Store => "W",
            Self::LoadStore => "RW",
        }
    }
}

/// Persistent breakpoint or watchpoint registered in VICE.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViceBreakpoint {
    /// Checkpoint ID assigned by VICE.
    pub id: u32,
    /// Target 6502 memory address.
    pub address: u16,
    /// True when the checkpoint is active in VICE.
    pub enabled: bool,
    /// Trigger operation for this checkpoint.
    pub kind: BreakpointKind,
}

/// Live debugger state synchronized with a connected VICE instance.
#[derive(Debug, Clone)]
pub struct ViceState {
    /// True when the TCP connection to the VICE Binary Monitor is open.
    pub connected: bool,
    /// Program Counter (`PC`) register.
    pub pc: Option<u16>,
    /// Accumulator (`A`) register.
    pub a: Option<u8>,
    /// X index (`X`) register.
    pub x: Option<u8>,
    /// Y index (`Y`) register.
    pub y: Option<u8>,
    /// Stack Pointer (`SP`) register.
    pub sp: Option<u8>,
    /// Processor status flags (`P`) register.
    pub p: Option<u8>,
    /// Connection status label.
    pub status: String,
    /// True when the emulated CPU is executing instructions.
    pub running: bool,

    /// Memory bytes around `PC` fetched via `MEMORY_GET`.
    pub live_memory: Option<Vec<u8>>,
    /// Start address corresponding to `live_memory[0]`.
    pub live_memory_start: u16,

    /// Stack page snapshot (`$0100`–`$01FF`).
    pub stack_memory: Option<Vec<u8>>,

    /// Hardware I/O register block snapshot.
    pub io_memory: Option<Vec<u8>>,

    /// CPU port registers at `$0000`–`$0001` on C64 and C128 targets.
    pub zp00_01: Option<Vec<u8>>,
    /// Hardware vector table (`$FFFA`–`$FFFF`).
    pub vectors: Option<Vec<u8>>,

    /// Start address for the user-configured memory dump panel.
    pub dump_address: Option<u16>,
    /// Memory bytes fetched at `dump_address`.
    pub dump_memory: Option<Vec<u8>>,

    /// Persistent checkpoints (breakpoints and watchpoints).
    pub breakpoints: Vec<ViceBreakpoint>,

    /// Temporary checkpoint IDs (such as run-to-cursor) to delete on the next stop.
    pub temporary_breakpoints: Vec<u32>,

    /// Reason the CPU stopped (for example, `"Breakpoint #2 at $C123"`).
    pub stop_reason: Option<String>,

    /// Checkpoint ID from the latest hit notification.
    pub last_hit_checkpoint_id: Option<u32>,

    /// State snapshot from the previous CPU stop, used to highlight changed values.
    pub previous: Option<Box<ViceState>>,
}

impl Default for ViceState {
    fn default() -> Self {
        Self::new()
    }
}

impl ViceState {
    /// Creates a disconnected [`ViceState`] with default dump address `$0000`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            connected: false,
            pc: None,
            a: None,
            x: None,
            y: None,
            sp: None,
            p: None,
            status: "Disconnected".to_string(),
            running: false,
            live_memory: None,
            live_memory_start: 0,
            stack_memory: None,
            io_memory: None,
            zp00_01: None,
            vectors: None,
            dump_address: Some(0x0000),
            dump_memory: None,
            breakpoints: Vec::new(),
            temporary_breakpoints: Vec::new(),
            stop_reason: None,
            last_hit_checkpoint_id: None,
            previous: None,
        }
    }

    /// Clears all registers, memory snapshots, and checkpoints while preserving `connected` and `status`.
    pub fn reset_registers(&mut self) {
        let connected = self.connected;
        let status = std::mem::take(&mut self.status);
        *self = Self {
            connected,
            status,
            ..Self::new()
        };
    }

    /// Clones this state without the `previous` field to store as the next baseline snapshot.
    #[must_use]
    pub fn snapshot(&self) -> Box<ViceState> {
        let mut snap = self.clone();
        snap.previous = None;
        Box::new(snap)
    }

    /// Returns true if a persistent checkpoint exists at `addr`.
    #[must_use]
    pub fn has_breakpoint_at(&self, addr: u16) -> bool {
        self.breakpoints.iter().any(|bp| bp.address == addr)
    }
}
