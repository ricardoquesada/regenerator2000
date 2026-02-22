#[derive(Debug, Clone)]
pub struct ViceBreakpoint {
    pub id: u32,
    pub address: u16,
    pub enabled: bool,
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

    // Persistent breakpoints (excludes temporary run-to-cursor checkpoints)
    pub breakpoints: Vec<ViceBreakpoint>,
}

impl ViceState {
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
            breakpoints: Vec::new(),
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
        self.breakpoints.clear();
    }

    /// Returns true if there is a persistent breakpoint at `addr`.
    pub fn has_breakpoint_at(&self, addr: u16) -> bool {
        self.breakpoints.iter().any(|bp| bp.address == addr)
    }

    /// Calculates the return address from the stack pointer and stack memory snapshot.
    /// This is used for stepping out of subroutines.
    pub fn get_return_address(&self) -> Option<u16> {
        let sp_usize = self.sp? as usize;
        let stack = self.stack_memory.as_ref()?;

        let lo_idx = (sp_usize + 1) & 0xFF; // Wrap properly within page 1
        let hi_idx = (sp_usize + 2) & 0xFF; // $0100-$01FF
        let lo = stack[lo_idx] as u16;
        let hi = stack[hi_idx] as u16;
        let ret_addr = (hi << 8) | lo;

        // RTS pulls this address minus 1 (i.e. address of 3rd byte of JSR)
        // It adds 1 to get the next instruction's address.
        Some(ret_addr.wrapping_add(1))
    }
}
