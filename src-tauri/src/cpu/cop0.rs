pub struct Cop0 {
    pub status: u32,
    pub cause: u32,
    pub epc: u32,
    pub config: u32,
    pub count: u32,
    pub compare: u32,
    pub bad_vaddr: u32,
    pub prid: u32,
    pub error_epc: u32,
}

impl Cop0 {
    pub fn new() -> Self {
        Self {
            status: 0,
            cause: 0,
            epc: 0,
            config: 0,
            count: 0,
            compare: 0,
            bad_vaddr: 0,
            prid: 0x00002E20, // EE Core revision
            error_epc: 0,
        }
    }

    pub fn read_reg(&self, reg: u32) -> u32 {
        match reg {
            8 => self.bad_vaddr,
            9 => self.count,
            11 => self.compare,
            12 => self.status,
            13 => self.cause,
            14 => self.epc,
            15 => self.prid,
            16 => self.config,
            30 => self.error_epc,
            _ => 0,
        }
    }

    pub fn write_reg(&mut self, reg: u32, val: u32) {
        match reg {
            8 => self.bad_vaddr = val,
            9 => self.count = val,
            11 => {
                self.compare = val;
                self.cause &= !(1 << 15); // Clear Timer Interrupt (IP7)
            },
            12 => self.status = val,
            13 => self.cause = val,
            14 => self.epc = val,
            16 => self.config = val,
            30 => self.error_epc = val,
            _ => {},
        }
    }
}
