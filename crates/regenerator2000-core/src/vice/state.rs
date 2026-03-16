#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakpointKind {
    Exec,
    Load,
    Store,
    LoadStore,
}

impl BreakpointKind {
    #[must_use]
    pub fn from_cpu_op(op: u8) -> Self {
        match op {
            0x01 => BreakpointKind::Load,
            0x02 => BreakpointKind::Store,
            0x03 => BreakpointKind::LoadStore,
            _ => BreakpointKind::Exec,
        }
    }

    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            BreakpointKind::Exec => "X",
            BreakpointKind::Load => "R",
            BreakpointKind::Store => "W",
            BreakpointKind::LoadStore => "RW",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ViceBreakpoint {
    pub id: u32,
    pub address: u16,
    pub enabled: bool,
    pub kind: BreakpointKind,
}

#[derive(Debug, Clone, Default)]
pub struct ViceState {
    pub connected: bool,
    pub pc: Option<u16>,
    pub a: Option<u8>,
    pub x: Option<u8>,
    pub y: Option<u8>,
    pub sp: Option<u8>,
    pub p: Option<u8>, // status flags
    pub status: String,
    pub running: bool,

    // Live memory snapshot: bytes fetched from VICE via MEMORY_GET
    pub live_memory: Option<Vec<u8>>,
    pub live_memory_start: u16, // the address that live_memory[0] corresponds to

    // Stack page snapshot ($0100–$01FF), fetched after each step
    pub stack_memory: Option<Vec<u8>>,

    // I/O block snapshot ($D000–$DFFF), fetched after each step if platform is C64/C128
    pub io_memory: Option<Vec<u8>>,

    pub zp00_01: Option<Vec<u8>>,
    pub vectors: Option<Vec<u8>>,

    // Persistent breakpoints (excludes temporary run-to-cursor checkpoints)
    pub breakpoints: Vec<ViceBreakpoint>,

    // Tracks temporary breakpoints (like run-to-cursor) so we can clear them
    // if the CPU stops before reaching them.
    pub temporary_breakpoints: Vec<u32>,

    // Human-readable reason the CPU stopped (e.g. "Breakpoint #2 at $C123")
    // Set when a checkpoint is hit, cleared on resume.
    pub stop_reason: Option<String>,

    // The checkpoint ID that was last hit (from an unsolicited CHECKPOINT_GET
    // response). Used to build `stop_reason` for watchpoints, where the PC
    // differs from the watched address.
    pub last_hit_checkpoint_id: Option<u32>,
}

impl ViceState {
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
            breakpoints: Vec::new(),
            temporary_breakpoints: Vec::new(),
            stop_reason: None,
            last_hit_checkpoint_id: None,
        }
    }

    pub fn reset_registers(&mut self) {
        self.pc = None;
        self.a = None;
        self.x = None;
        self.y = None;
        self.sp = None;
        self.p = None;
        self.running = false;
        self.live_memory = None;
        self.live_memory_start = 0;
        self.stack_memory = None;
        self.io_memory = None;
        self.zp00_01 = None;
        self.vectors = None;
        self.breakpoints.clear();
        self.temporary_breakpoints.clear();
        self.stop_reason = None;
        self.last_hit_checkpoint_id = None;
    }

    /// Returns true if there is a persistent breakpoint at `addr`.
    #[must_use]
    pub fn has_breakpoint_at(&self, addr: u16) -> bool {
        self.breakpoints.iter().any(|bp| bp.address == addr)
    }
}
