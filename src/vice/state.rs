#[derive(Debug, Clone, Default)]
pub struct ViceState {
    pub connected: bool,
    pub pc: Option<u16>,
    pub status: String,
}

impl ViceState {
    pub fn new() -> Self {
        Self {
            connected: false,
            pc: None,
            status: "Disconnected".to_string(),
        }
    }
}
