use crate::memory::bus::Bus;
use crate::cpu::instruction::Instruction;

/// The Emotion Engine (EE) - Custom MIPS R5900 core
pub struct EmotionEngine {
    // 32 General Purpose Registers (GPRs)
    pub gpr: [u64; 32],
    
    // Program Counter
    pub pc: u32,
    pub next_pc: u32,
    
    // Branch Delay Slot tracking
    pub current_pc: u32,
    pub branch: bool,
    pub delay_slot: bool,
    
    // System Coprocessor (COP0) Registers stub
    pub cop0: [u32; 32],

    // Reference to the system bus (Memory)
    pub bus: Bus,
}

impl EmotionEngine {
    pub fn new(bus: Bus) -> Self {
        Self {
            gpr: [0; 32],
            pc: 0x1FC00000,
            next_pc: 0x1FC00004,
            current_pc: 0x1FC00000,
            branch: false,
            delay_slot: false,
            cop0: [0; 32],
            bus,
        }
    }

    pub fn set_reg(&mut self, reg: u32, val: u64) {
        if reg != 0 {
            self.gpr[reg as usize] = val;
        }
    }

    pub fn get_reg(&self, reg: u32) -> u64 {
        if reg == 0 {
            0
        } else {
            self.gpr[reg as usize]
        }
    }

    /// Fetches the next instruction from memory at the current PC, decodes it,
    /// executes it, and advances the PC. Returns a string representation of the executed instruction.
    pub fn step(&mut self) -> String {
        self.current_pc = self.pc;
        
        if self.current_pc % 4 != 0 {
            return format!("ERROR: PC is not aligned! PC: {:#010X}", self.current_pc);
        }

        // Fetch 32-bit instruction from memory
        let op = self.bus.read32(self.current_pc);
        let instr = Instruction::new(op);
        
        // Decode
        let opcode = instr.opcode();
        
        let mut log = format!("> [{:#010X}] {:#010X} ", self.current_pc, op);
        
        // Advance PC for delay slot logic
        self.pc = self.next_pc;
        self.next_pc = self.next_pc.wrapping_add(4);

        // Delay slot tracking
        self.delay_slot = self.branch;
        self.branch = false;
        
        // Basic match for instructions
        match opcode {
            0b000000 => {
                // SPECIAL (R-Type)
                let funct = instr.funct();
                match funct {
                    0b000000 => {
                        // SLL rd, rt, shamt
                        let rd = instr.rd();
                        let rt = instr.rt();
                        let shamt = instr.shamt();
                        if op == 0 {
                            log.push_str("NOP");
                        } else {
                            log.push_str(&format!("SLL $t{}, $t{}, {}", rd, rt, shamt));
                            let val = self.get_reg(rt) << shamt;
                            self.set_reg(rd, val);
                        }
                    },
                    0b100000 => {
                        // ADD rd, rs, rt
                        let rd = instr.rd();
                        let rs = instr.rs();
                        let rt = instr.rt();
                        log.push_str(&format!("ADD $t{}, $t{}, $t{}", rd, rs, rt));
                        let val = self.get_reg(rs).wrapping_add(self.get_reg(rt));
                        self.set_reg(rd, val);
                    },
                    0b100101 => {
                        // OR rd, rs, rt
                        let rd = instr.rd();
                        let rs = instr.rs();
                        let rt = instr.rt();
                        log.push_str(&format!("OR $t{}, $t{}, $t{}", rd, rs, rt));
                        let val = self.get_reg(rs) | self.get_reg(rt);
                        self.set_reg(rd, val);
                    },
                    0b001000 => {
                        // JR rs
                        let rs = instr.rs();
                        log.push_str(&format!("JR $t{}", rs));
                        self.next_pc = self.get_reg(rs) as u32;
                        self.branch = true;
                    },
                    0b001001 => {
                        // JALR rd, rs
                        let rd = instr.rd();
                        let rs = instr.rs();
                        log.push_str(&format!("JALR $t{}, $t{}", rd, rs));
                        self.set_reg(rd, (self.pc + 4) as u64); // Return address
                        self.next_pc = self.get_reg(rs) as u32;
                        self.branch = true;
                    },
                    0b100100 => {
                        // AND rd, rs, rt
                        let rd = instr.rd();
                        let rs = instr.rs();
                        let rt = instr.rt();
                        log.push_str(&format!("AND $t{}, $t{}, $t{}", rd, rs, rt));
                        let val = self.get_reg(rs) & self.get_reg(rt);
                        self.set_reg(rd, val);
                    },
                    0b100001 => {
                        // ADDU rd, rs, rt
                        let rd = instr.rd();
                        let rs = instr.rs();
                        let rt = instr.rt();
                        log.push_str(&format!("ADDU $t{}, $t{}, $t{}", rd, rs, rt));
                        let val = self.get_reg(rs).wrapping_add(self.get_reg(rt));
                        self.set_reg(rd, val);
                    },
                    0b101011 => {
                        // SLTU rd, rs, rt
                        let rd = instr.rd();
                        let rs = instr.rs();
                        let rt = instr.rt();
                        log.push_str(&format!("SLTU $t{}, $t{}, $t{}", rd, rs, rt));
                        let val = if self.get_reg(rs) < self.get_reg(rt) { 1 } else { 0 };
                        self.set_reg(rd, val);
                    },
                    _ => {
                        log.push_str(&format!("UNKNOWN SPECIAL: {:#08b}", funct));
                    }
                }
            },
            0b010000 => {
                // COP0
                let rs = instr.rs();
                let rt = instr.rt();
                let rd = instr.rd();
                match rs {
                    0b00000 => {
                        // MFC0 rt, rd
                        log.push_str(&format!("MFC0 $t{}, COP0_R{}", rt, rd));
                        let val = self.cop0[rd as usize] as u64;
                        self.set_reg(rt, val);
                    },
                    0b00100 => {
                        // MTC0 rt, rd
                        log.push_str(&format!("MTC0 $t{}, COP0_R{}", rt, rd));
                        self.cop0[rd as usize] = self.get_reg(rt) as u32;
                    },
                    _ => {
                        log.push_str(&format!("UNKNOWN COP0: {:#07b}", rs));
                    }
                }
            },
            0b000101 => {
                // BNE rs, rt, offset
                let rs = instr.rs();
                let rt = instr.rt();
                let offset = instr.imm_sign_extended();
                log.push_str(&format!("BNE $t{}, $t{}, {:#06X}", rs, rt, offset));
                
                if self.get_reg(rs) != self.get_reg(rt) {
                    let offset_addr = (offset as u32) << 2;
                    self.next_pc = self.pc.wrapping_add(offset_addr);
                    self.branch = true;
                }
            },
            0b000100 => {
                // BEQ rs, rt, offset
                let rs = instr.rs();
                let rt = instr.rt();
                let offset = instr.imm_sign_extended();
                log.push_str(&format!("BEQ $t{}, $t{}, {:#06X}", rs, rt, offset));
                
                if self.get_reg(rs) == self.get_reg(rt) {
                    let offset_addr = (offset as u32) << 2;
                    self.next_pc = self.pc.wrapping_add(offset_addr);
                    self.branch = true;
                }
            },
            0b001111 => {
                // LUI rt, imm
                let rt = instr.rt();
                let imm = instr.imm();
                log.push_str(&format!("LUI $t{}, {:#06X}", rt, imm));
                
                let val = ((imm as i32) << 16) as i64 as u64; // Sign extend the 32-bit result to 64-bit
                self.set_reg(rt, val);
            },
            0b001101 => {
                // ORI rt, rs, imm
                let rt = instr.rt();
                let rs = instr.rs();
                let imm = instr.imm();
                log.push_str(&format!("ORI $t{}, $t{}, {:#06X}", rt, rs, imm));
                
                let val = self.get_reg(rs) | (imm as u64);
                self.set_reg(rt, val);
            },
            0b100011 => {
                // LW rt, offset(rs)
                let rt = instr.rt();
                let rs = instr.rs();
                let offset = instr.imm_sign_extended();
                let addr = (self.get_reg(rs) as u32).wrapping_add(offset as u32);
                log.push_str(&format!("LW $t{}, {:#06X}($t{})", rt, offset, rs));
                
                let val = self.bus.read32(addr) as i32 as i64 as u64; // Sign extend 32-bit load
                self.set_reg(rt, val);
            },
            0b101011 => {
                // SW rt, offset(rs)
                let rs = instr.rs();
                let rt = instr.rt();
                let offset = instr.imm_sign_extended();
                let addr = self.get_reg(rs).wrapping_add(offset as u64) as u32;
                self.bus.write32(addr, self.get_reg(rt) as u32);
                log.push_str(&format!("SW $t{}, {:#06X}($t{}) [ADDR: {:#010X}]", rt, offset as u16, rs, addr));
            },
            0b100000 => {
                // LB rt, offset(rs)
                let rt = instr.rt();
                let rs = instr.rs();
                let offset = instr.imm_sign_extended();
                let addr = (self.get_reg(rs) as u32).wrapping_add(offset as u32);
                log.push_str(&format!("LB $t{}, {:#06X}($t{})", rt, offset, rs));
                
                let val = self.bus.read8(addr) as i8 as i64 as u64; // Sign extend 8-bit load
                self.set_reg(rt, val);
            },
            0b101000 => {
                // SB rt, offset(rs)
                let rs = instr.rs();
                let rt = instr.rt();
                let offset = instr.imm_sign_extended();
                let addr = self.get_reg(rs).wrapping_add(offset as u64) as u32;
                self.bus.write8(addr, (self.get_reg(rt) & 0xFF) as u8);
                log.push_str(&format!("SB $t{}, {:#06X}($t{}) [ADDR: {:#010X}]", rt, offset as u16, rs, addr));
            },
            0b100101 => {
                // LHU rt, offset(rs)
                let rt = instr.rt();
                let rs = instr.rs();
                let offset = instr.imm_sign_extended();
                let addr = (self.get_reg(rs) as u32).wrapping_add(offset as u32);
                log.push_str(&format!("LHU $t{}, {:#06X}($t{})", rt, offset, rs));
                
                let val = self.bus.read16(addr) as u64; // Zero extend
                self.set_reg(rt, val);
            },
            0b100100 => {
                // LBU rt, offset(rs)
                let rt = instr.rt();
                let rs = instr.rs();
                let offset = instr.imm_sign_extended();
                let addr = (self.get_reg(rs) as u32).wrapping_add(offset as u32);
                log.push_str(&format!("LBU $t{}, {:#06X}($t{})", rt, offset, rs));
                
                let val = self.bus.read8(addr) as u64; // Zero extend
                self.set_reg(rt, val);
            },
            0b101111 => {
                // CACHE op, offset(base)
                log.push_str("CACHE");
                // No-op in emulator
            },
            0b100001 => {
                // LH rt, offset(rs)
                let rt = instr.rt();
                let rs = instr.rs();
                let offset = instr.imm_sign_extended();
                let addr = (self.get_reg(rs) as u32).wrapping_add(offset as u32);
                log.push_str(&format!("LH $t{}, {:#06X}($t{})", rt, offset, rs));
                
                let val = self.bus.read16(addr) as i16 as i64 as u64; // Sign extend 16-bit load
                self.set_reg(rt, val);
            },
            0b101001 => {
                // SH rt, offset(rs)
                let rs = instr.rs();
                let rt = instr.rt();
                let offset = instr.imm_sign_extended();
                let addr = self.get_reg(rs).wrapping_add(offset as u64) as u32;
                self.bus.write16(addr, (self.get_reg(rt) & 0xFFFF) as u16);
                log.push_str(&format!("SH $t{}, {:#06X}($t{}) [ADDR: {:#010X}]", rt, offset as u16, rs, addr));
            },
            0b000010 => {
                // J target
                let target = instr.imm_jump_target();
                log.push_str(&format!("J {:#09X}", target));
                self.next_pc = (self.pc & 0xF0000000) | (target << 2);
                self.branch = true;
            },
            0b000011 => {
                // JAL target
                let target = instr.imm_jump_target();
                log.push_str(&format!("JAL {:#09X}", target));
                self.set_reg(31, (self.pc + 4) as u64); // $ra
                self.next_pc = (self.pc & 0xF0000000) | (target << 2);
                self.branch = true;
            },
            0b001000 => {
                // ADDI rt, rs, imm
                let rt = instr.rt();
                let rs = instr.rs();
                let imm = instr.imm_sign_extended();
                log.push_str(&format!("ADDI $t{}, $t{}, {:#06X}", rt, rs, instr.imm()));
                let val = self.get_reg(rs).wrapping_add(imm);
                self.set_reg(rt, val);
            },
            0b001001 => {
                // ADDIU rt, rs, imm (same as ADDI for our purposes since we don't trap overflow)
                let rt = instr.rt();
                let rs = instr.rs();
                let imm = instr.imm_sign_extended();
                log.push_str(&format!("ADDIU $t{}, $t{}, {:#06X}", rt, rs, instr.imm()));
                let val = self.get_reg(rs).wrapping_add(imm);
                self.set_reg(rt, val);
            },
            0b001010 => {
                // SLTI rt, rs, imm
                let rt = instr.rt();
                let rs = instr.rs();
                let imm = instr.imm_sign_extended() as i64;
                log.push_str(&format!("SLTI $t{}, $t{}, {:#06X}", rt, rs, instr.imm()));
                let rs_val = self.get_reg(rs) as i64;
                let val = if rs_val < imm { 1 } else { 0 };
                self.set_reg(rt, val);
            },
            0b001011 => {
                // SLTIU rt, rs, imm
                let rt = instr.rt();
                let rs = instr.rs();
                let imm = instr.imm_sign_extended(); // Sign extend, but treat as unsigned comparison
                log.push_str(&format!("SLTIU $t{}, $t{}, {:#06X}", rt, rs, instr.imm()));
                let val = if self.get_reg(rs) < imm { 1 } else { 0 };
                self.set_reg(rt, val);
            },
            0b001100 => {
                // ANDI rt, rs, imm
                let rt = instr.rt();
                let rs = instr.rs();
                let imm = instr.imm(); // Zero extended
                log.push_str(&format!("ANDI $t{}, $t{}, {:#06X}", rt, rs, imm));
                let val = self.get_reg(rs) & (imm as u64);
                self.set_reg(rt, val);
            },
            0b001110 => {
                // XORI rt, rs, imm
                let rt = instr.rt();
                let rs = instr.rs();
                let imm = instr.imm(); // Zero extended
                log.push_str(&format!("XORI $t{}, $t{}, {:#06X}", rt, rs, imm));
                let val = self.get_reg(rs) ^ (imm as u64);
                self.set_reg(rt, val);
            },
            _ => {
                log.push_str(&format!("UNKNOWN OPCODE: {:#08b}", opcode));
            }
        }
        
        log
    }
}
