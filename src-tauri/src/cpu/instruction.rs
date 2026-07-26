pub struct Instruction {
    pub op: u32,
}

impl Instruction {
    pub fn new(op: u32) -> Self {
        Self { op }
    }

    pub fn opcode(&self) -> u32 {
        self.op >> 26
    }

    pub fn rs(&self) -> u32 {
        (self.op >> 21) & 0x1F
    }

    pub fn rt(&self) -> u32 {
        (self.op >> 16) & 0x1F
    }

    pub fn rd(&self) -> u32 {
        (self.op >> 11) & 0x1F
    }

    pub fn shamt(&self) -> u32 {
        (self.op >> 6) & 0x1F
    }

    pub fn funct(&self) -> u32 {
        self.op & 0x3F
    }

    pub fn imm(&self) -> u32 {
        self.op & 0xFFFF
    }

    pub fn imm_sign_extended(&self) -> u64 {
        let imm = self.imm() as i16;
        imm as i64 as u64
    }

    pub fn imm_jump_target(&self) -> u32 {
        self.op & 0x03FFFFFF
    }
}
