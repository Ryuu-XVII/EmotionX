pub struct Sio {
    pub buffer: String,
    pub pending_lines: Vec<String>,
}

impl Sio {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            pending_lines: Vec::new(),
        }
    }

    pub fn read8(&mut self, addr: u32) -> u8 {
        match addr {
            // SIO control/status registers can just return ready state
            // 0x1000F1A0 is LSR (Line Status Register), usually bit 0 = rx ready, bit 5 = tx ready
            0x1000F1A0 => 0x20, // TX ready
            _ => 0,
        }
    }

    pub fn read32(&mut self, addr: u32) -> u32 {
        self.read8(addr) as u32
    }

    pub fn write8(&mut self, addr: u32, val: u8) {
        match addr {
            // TX data register
            0x1000F180 => {
                let c = val as char;
                if c == '\n' {
                    let line = std::mem::take(&mut self.buffer);
                    self.pending_lines.push(line);
                } else {
                    self.buffer.push(c);
                }
            },
            _ => {}
        }
    }

    pub fn write32(&mut self, addr: u32, val: u32) {
        self.write8(addr, (val & 0xFF) as u8);
    }
}
