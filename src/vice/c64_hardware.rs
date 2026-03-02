pub struct Vic2State {
    pub raster_line: u16,
    pub blanking: bool,
    pub bitmap_mode: bool,
    pub extended_bg_color: bool,
    pub text_mode: bool,
    pub multicolor_mode: bool,
    pub y_scroll: u8,
    pub x_scroll: u8,
    pub columns: u8,
    pub rows: u8,
    pub screen_mem_address: u16,
    pub charset_address: u16,
    pub bitmap_address: u16,
    pub irq_status: u8,
    pub border_color: u8,
    pub bg_color: u8,
}

impl Vic2State {
    pub fn decode(io_mem: &[u8]) -> Self {
        if io_mem.len() < 0x40 {
            return Self::default();
        }

        let d011 = io_mem[0x11];
        let d012 = io_mem[0x12];
        let d016 = io_mem[0x16];
        let d018 = io_mem[0x18];
        let d019 = io_mem[0x19];
        let d020 = io_mem[0x20];
        let d021 = io_mem[0x21];

        let raster_line = (((d011 as u16) & 0x80) << 1) | (d012 as u16);
        let blanking = (d011 & 0x10) == 0;
        let extended_bg_color = (d011 & 0x40) != 0;
        let bitmap_mode = (d011 & 0x20) != 0;
        let rows = if (d011 & 0x08) != 0 { 25 } else { 24 };
        let y_scroll = d011 & 0x07;

        let columns = if (d016 & 0x08) != 0 { 40 } else { 38 };
        let x_scroll = d016 & 0x07;
        let multicolor_mode = (d016 & 0x10) != 0;

        let screen_mem_address = ((d018 & 0xF0) as u16) << 6;
        let charset_address = ((d018 & 0x0E) as u16) << 10;
        let bitmap_address = ((d018 & 0x08) as u16) << 10;
        let text_mode = !bitmap_mode;

        Self {
            raster_line,
            blanking,
            bitmap_mode,
            extended_bg_color,
            text_mode,
            multicolor_mode,
            y_scroll,
            x_scroll,
            columns,
            rows,
            screen_mem_address,
            charset_address,
            bitmap_address,
            irq_status: d019,
            border_color: d020 & 0x0F,
            bg_color: d021 & 0x0F,
        }
    }
}

impl Default for Vic2State {
    fn default() -> Self {
        Self {
            raster_line: 0,
            blanking: false,
            bitmap_mode: false,
            extended_bg_color: false,
            text_mode: true,
            multicolor_mode: false,
            y_scroll: 3,
            x_scroll: 0,
            columns: 40,
            rows: 25,
            screen_mem_address: 0x0400,
            charset_address: 0x1000,
            bitmap_address: 0x2000,
            irq_status: 0,
            border_color: 14,
            bg_color: 6,
        }
    }
}

#[derive(Default)]
pub struct CiaState {
    pub pra: u8,
    pub prb: u8,
    pub ddra: u8,
    pub ddrb: u8,
    pub timer_a: u16,
    pub timer_b: u16,
}

impl CiaState {
    pub fn decode(io_mem: &[u8], offset: usize) -> Self {
        if io_mem.len() < offset + 16 {
            return Self::default();
        }
        let pra = io_mem[offset];
        let prb = io_mem[offset + 0x01];
        let ddra = io_mem[offset + 0x02];
        let ddrb = io_mem[offset + 0x03];
        let timer_a = (io_mem[offset + 0x04] as u16) | ((io_mem[offset + 0x05] as u16) << 8);
        let timer_b = (io_mem[offset + 0x06] as u16) | ((io_mem[offset + 0x07] as u16) << 8);

        Self {
            pra,
            prb,
            ddra,
            ddrb,
            timer_a,
            timer_b,
        }
    }
}
