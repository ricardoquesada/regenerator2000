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

    // Live memory snapshot: bytes fetched from VICE via MEMORY_GET
    pub live_memory: Option<Vec<u8>>,
    pub live_memory_start: u16, // the address that live_memory[0] corresponds to
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
            live_memory: None,
            live_memory_start: 0,
        }
    }

    pub fn reset_registers(&mut self) {
        self.pc = None;
        self.a = None;
        self.x = None;
        self.y = None;
        self.sp = None;
        self.p = None;
        self.live_memory = None;
        self.live_memory_start = 0;
    }
}
