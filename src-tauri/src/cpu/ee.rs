use crate::memory::bus::Bus;
use crate::cpu::instruction::Instruction;
use crate::cpu::cop0::Cop0;

/// The Emotion Engine (EE) - Custom MIPS R5900 core
pub struct EmotionEngine {
    // 32 General Purpose Registers (GPRs) (128-bit on Emotion Engine)
    pub gpr: [u128; 32],
    
    // HI / LO Registers for multiplication and division
    pub hi: u64,
    pub lo: u64,
    
    // Program Counter
    pub pc: u32,
    pub next_pc: u32,
    
    // Branch Delay Slot tracking
    pub current_pc: u32,
    pub branch: bool,
    pub delay_slot: bool,
    
    // System Coprocessor (COP0)
    pub cop0: Cop0,

    // Reference to the system bus (Memory)
    pub bus: Bus,
}

impl EmotionEngine {
    pub fn new(bus: Bus) -> Self {
        Self {
            gpr: [0; 32],
            hi: 0,
            lo: 0,
            pc: 0x1FC00000,
            next_pc: 0x1FC00004,
            current_pc: 0x1FC00000,
            branch: false,
            delay_slot: false,
            cop0: Cop0::new(),
            bus,
        }
    }

    pub fn set_pc(&mut self, pc: u32) {
        self.pc = pc;
        self.next_pc = pc.wrapping_add(4);
    }

    pub fn set_reg(&mut self, reg: u32, val: u64) {
        if reg != 0 {
            // MIPS I/II/III instructions sign-extend the 64-bit result to 128-bits
            self.gpr[reg as usize] = (val as i64) as i128 as u128;
        }
    }

    pub fn get_reg(&self, reg: u32) -> u64 {
        if reg == 0 {
            0
        } else {
            self.gpr[reg as usize] as u64
        }
    }

    pub fn set_reg128(&mut self, reg: u32, val: u128) {
        if reg != 0 {
            self.gpr[reg as usize] = val;
        }
    }

    pub fn get_reg128(&self, reg: u32) -> u128 {
        if reg == 0 {
            0
        } else {
            self.gpr[reg as usize]
        }
    }

    /// Fetches the next instruction from memory at the current PC, decodes it,
    /// executes it, and advances the PC. Returns a string representation of the executed instruction.
    pub fn step(&mut self) -> String {
        // Increment COP0 count
        self.cop0.count = self.cop0.count.wrapping_add(1);
        if self.cop0.count == self.cop0.compare {
            self.cop0.cause |= 1 << 15; // Set IP7
        }

        // Artificial VBlank (IRQ 3) at 60Hz (approx 5,000,000 cycles for a 300MHz CPU)
        if self.cop0.count % 100_000 == 0 {
            self.bus.hw.trigger_irq(3);
        }

        // Sync INTC interrupts to IP2
        if self.bus.hw.check_interrupts() {
            self.cop0.cause |= 1 << 10; // Set IP2
        } else {
            self.cop0.cause &= !(1 << 10); // Clear IP2
        }

        // Check for enabled interrupts (Interrupt Exception)
        // IE (bit 0) must be 1, EXL (bit 1) must be 0, ERL (bit 2) must be 0
        let status = self.cop0.status;
        let cause = self.cop0.cause;
        let ie = (status & 1) != 0;
        let exl = (status & 2) != 0;
        let erl = (status & 4) != 0;
        let im = (status >> 8) & 0xFF; // Interrupt Mask
        let ip = (cause >> 8) & 0xFF;  // Interrupt Pending
        
        if self.cop0.count % 100_000 == 1 {
            let debug_log = format!("DEBUG INT: PC={:#010X} IE={} EXL={} ERL={} IP={:#010b} IM={:#010b} STATUS={:#010X} CAUSE={:#010X}", 
                     self.current_pc, ie, exl, erl, ip, im, status, cause);
            self.bus.hw.sio.pending_lines.push(debug_log);
        }

        if ie && !exl && !erl && (ip & im) != 0 {
            // For external interrupts, the EPC is the instruction we were ABOUT to execute (self.pc).
            // But trigger_exception uses self.current_pc. So we update it first.
            self.current_pc = self.pc;
            self.trigger_exception(0); // 0 = Interrupt Exception
            return format!("> [{:#010X}] INTERRUPT FIRED", self.current_pc);
        }

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
                    0b000010 => {
                        // SRL rd, rt, shamt
                        let rd = instr.rd();
                        let rt = instr.rt();
                        let shamt = instr.shamt();
                        log.push_str(&format!("SRL $t{}, $t{}, {}", rd, rt, shamt));
                        let val = self.get_reg(rt) >> shamt;
                        self.set_reg(rd, val);
                    },
                    0b000011 => {
                        // SRA rd, rt, shamt
                        let rd = instr.rd();
                        let rt = instr.rt();
                        let shamt = instr.shamt();
                        log.push_str(&format!("SRA $t{}, $t{}, {}", rd, rt, shamt));
                        let val = ((self.get_reg(rt) as i32) >> shamt) as u64;
                        self.set_reg(rd, val);
                    },
                    0b000100 => {
                        // SLLV rd, rt, rs
                        let rd = instr.rd();
                        let rt = instr.rt();
                        let rs = instr.rs();
                        log.push_str(&format!("SLLV $t{}, $t{}, $t{}", rd, rt, rs));
                        let shamt = self.get_reg(rs) & 0x1F;
                        let val = self.get_reg(rt) << shamt;
                        self.set_reg(rd, val);
                    },
                    0b000110 => {
                        // SRLV rd, rt, rs
                        let rd = instr.rd();
                        let rt = instr.rt();
                        let rs = instr.rs();
                        log.push_str(&format!("SRLV $t{}, $t{}, $t{}", rd, rt, rs));
                        let shamt = self.get_reg(rs) & 0x1F;
                        let val = self.get_reg(rt) >> shamt;
                        self.set_reg(rd, val);
                    },
                    0b000111 => {
                        // SRAV rd, rt, rs
                        let rd = instr.rd();
                        let rt = instr.rt();
                        let rs = instr.rs();
                        log.push_str(&format!("SRAV $t{}, $t{}, $t{}", rd, rt, rs));
                        let shamt = self.get_reg(rs) & 0x1F;
                        let val = ((self.get_reg(rt) as i32) >> shamt) as u64;
                        self.set_reg(rd, val);
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
                        self.set_reg(rd, self.next_pc as u64); // Return address
                        self.next_pc = self.get_reg(rs) as u32;
                        self.branch = true;
                    },
                    0b001100 => {
                        // SYSCALL
                        log.push_str("SYSCALL");
                        self.handle_syscall(&mut log);
                    },
                    0b001010 => {
                        // MOVZ rd, rs, rt (MIPS IV)
                        let rd = instr.rd();
                        let rs = instr.rs();
                        let rt = instr.rt();
                        log.push_str(&format!("MOVZ $t{}, $t{}, $t{}", rd, rs, rt));
                        if self.get_reg(rt) == 0 {
                            let val = self.get_reg(rs);
                            self.set_reg(rd, val);
                        }
                    },
                    0b001011 => {
                        // MOVN rd, rs, rt (MIPS IV)
                        let rd = instr.rd();
                        let rs = instr.rs();
                        let rt = instr.rt();
                        log.push_str(&format!("MOVN $t{}, $t{}, $t{}", rd, rs, rt));
                        if self.get_reg(rt) != 0 {
                            let val = self.get_reg(rs);
                            self.set_reg(rd, val);
                        }
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
                    0b100010 => {
                        // SUB rd, rs, rt
                        let rd = instr.rd();
                        let rs = instr.rs();
                        let rt = instr.rt();
                        log.push_str(&format!("SUB $t{}, $t{}, $t{}", rd, rs, rt));
                        let val = self.get_reg(rs).wrapping_sub(self.get_reg(rt));
                        self.set_reg(rd, val);
                    },
                    0b100011 => {
                        // SUBU rd, rs, rt
                        let rd = instr.rd();
                        let rs = instr.rs();
                        let rt = instr.rt();
                        log.push_str(&format!("SUBU $t{}, $t{}, $t{}", rd, rs, rt));
                        let val = self.get_reg(rs).wrapping_sub(self.get_reg(rt));
                        self.set_reg(rd, val);
                    },
                    0b100110 => {
                        // XOR rd, rs, rt
                        let rd = instr.rd();
                        let rs = instr.rs();
                        let rt = instr.rt();
                        log.push_str(&format!("XOR $t{}, $t{}, $t{}", rd, rs, rt));
                        let val = self.get_reg(rs) ^ self.get_reg(rt);
                        self.set_reg(rd, val);
                    },
                    0b100111 => {
                        // NOR rd, rs, rt
                        let rd = instr.rd();
                        let rs = instr.rs();
                        let rt = instr.rt();
                        log.push_str(&format!("NOR $t{}, $t{}, $t{}", rd, rs, rt));
                        let val = !(self.get_reg(rs) | self.get_reg(rt));
                        self.set_reg(rd, val);
                    },
                    0b101010 => {
                        // SLT rd, rs, rt
                        let rd = instr.rd();
                        let rs = instr.rs();
                        let rt = instr.rt();
                        log.push_str(&format!("SLT $t{}, $t{}, $t{}", rd, rs, rt));
                        let val = if (self.get_reg(rs) as i32) < (self.get_reg(rt) as i32) { 1 } else { 0 };
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
                    0b010000 => {
                        // MFHI rd
                        let rd = instr.rd();
                        log.push_str(&format!("MFHI $t{}", rd));
                        self.set_reg(rd, self.hi);
                    },
                    0b010001 => {
                        // MTHI rs
                        let rs = instr.rs();
                        log.push_str(&format!("MTHI $t{}", rs));
                        self.hi = self.get_reg(rs);
                    },
                    0b010010 => {
                        // MFLO rd
                        let rd = instr.rd();
                        log.push_str(&format!("MFLO $t{}", rd));
                        self.set_reg(rd, self.lo);
                    },
                    0b010011 => {
                        // MTLO rs
                        let rs = instr.rs();
                        log.push_str(&format!("MTLO $t{}", rs));
                        self.lo = self.get_reg(rs);
                    },
                    0b011000 => {
                        // MULT rs, rt
                        let rs = instr.rs();
                        let rt = instr.rt();
                        log.push_str(&format!("MULT $t{}, $t{}", rs, rt));
                        let val_rs = self.get_reg(rs) as i32 as i64;
                        let val_rt = self.get_reg(rt) as i32 as i64;
                        let res = (val_rs * val_rt) as u64;
                        self.hi = (res >> 32) & 0xFFFFFFFF;
                        self.lo = res & 0xFFFFFFFF;
                    },
                    0b011001 => {
                        // MULTU rs, rt
                        let rs = instr.rs();
                        let rt = instr.rt();
                        log.push_str(&format!("MULTU $t{}, $t{}", rs, rt));
                        let val_rs = (self.get_reg(rs) & 0xFFFFFFFF) as u64;
                        let val_rt = (self.get_reg(rt) & 0xFFFFFFFF) as u64;
                        let res = val_rs * val_rt;
                        self.hi = (res >> 32) & 0xFFFFFFFF;
                        self.lo = res & 0xFFFFFFFF;
                    },
                    0b011010 => {
                        // DIV rs, rt
                        let rs = instr.rs();
                        let rt = instr.rt();
                        log.push_str(&format!("DIV $t{}, $t{}", rs, rt));
                        let val_rs = self.get_reg(rs) as i32;
                        let val_rt = self.get_reg(rt) as i32;
                        if val_rt != 0 {
                            if val_rs == i32::MIN && val_rt == -1 {
                                self.lo = val_rs as u64;
                                self.hi = 0;
                            } else {
                                self.lo = (val_rs / val_rt) as i64 as u64;
                                self.hi = (val_rs % val_rt) as i64 as u64;
                            }
                        }
                    },
                    0b011011 => {
                        // DIVU rs, rt
                        let rs = instr.rs();
                        let rt = instr.rt();
                        log.push_str(&format!("DIVU $t{}, $t{}", rs, rt));
                        let val_rs = (self.get_reg(rs) & 0xFFFFFFFF) as u32;
                        let val_rt = (self.get_reg(rt) & 0xFFFFFFFF) as u32;
                        if val_rt != 0 {
                            self.lo = (val_rs / val_rt) as i32 as i64 as u64;
                            self.hi = (val_rs % val_rt) as i32 as i64 as u64;
                        }
                    },
                    0b010100 => {
                        // DSLLV rd, rt, rs
                        let rd = instr.rd();
                        let rt = instr.rt();
                        let rs = instr.rs();
                        log.push_str(&format!("DSLLV $t{}, $t{}, $t{}", rd, rt, rs));
                        let shift = self.get_reg(rs) & 0x3F; // 64-bit shift amount
                        let val = self.get_reg(rt) << shift;
                        self.set_reg(rd, val);
                    },
                    0b010110 => {
                        // DSRLV rd, rt, rs
                        let rd = instr.rd();
                        let rt = instr.rt();
                        let rs = instr.rs();
                        log.push_str(&format!("DSRLV $t{}, $t{}, $t{}", rd, rt, rs));
                        let shift = self.get_reg(rs) & 0x3F;
                        let val = self.get_reg(rt) >> shift;
                        self.set_reg(rd, val);
                    },
                    0b010111 => {
                        // DSRAV rd, rt, rs
                        let rd = instr.rd();
                        let rt = instr.rt();
                        let rs = instr.rs();
                        log.push_str(&format!("DSRAV $t{}, $t{}, $t{}", rd, rt, rs));
                        let shift = self.get_reg(rs) & 0x3F;
                        let val = ((self.get_reg(rt) as i64) >> shift) as u64;
                        self.set_reg(rd, val);
                    },
                    0b111000 => {
                        // DSLL rd, rt, shamt
                        let rd = instr.rd();
                        let rt = instr.rt();
                        let shamt = instr.shamt();
                        log.push_str(&format!("DSLL $t{}, $t{}, {}", rd, rt, shamt));
                        let val = self.get_reg(rt) << shamt;
                        self.set_reg(rd, val);
                    },
                    0b111010 => {
                        // DSRL rd, rt, shamt
                        let rd = instr.rd();
                        let rt = instr.rt();
                        let shamt = instr.shamt();
                        log.push_str(&format!("DSRL $t{}, $t{}, {}", rd, rt, shamt));
                        let val = self.get_reg(rt) >> shamt;
                        self.set_reg(rd, val);
                    },
                    0b111011 => {
                        // DSRA rd, rt, shamt
                        let rd = instr.rd();
                        let rt = instr.rt();
                        let shamt = instr.shamt();
                        log.push_str(&format!("DSRA $t{}, $t{}, {}", rd, rt, shamt));
                        let val = ((self.get_reg(rt) as i64) >> shamt) as u64;
                        self.set_reg(rd, val);
                    },
                    0b111100 => {
                        // DSLL32 rd, rt, shamt
                        let rd = instr.rd();
                        let rt = instr.rt();
                        let shamt = instr.shamt();
                        log.push_str(&format!("DSLL32 $t{}, $t{}, {}", rd, rt, shamt));
                        let val = self.get_reg(rt) << (shamt + 32);
                        self.set_reg(rd, val);
                    },
                    0b111110 => {
                        // DSRL32 rd, rt, shamt
                        let rd = instr.rd();
                        let rt = instr.rt();
                        let shamt = instr.shamt();
                        log.push_str(&format!("DSRL32 $t{}, $t{}, {}", rd, rt, shamt));
                        let val = self.get_reg(rt) >> (shamt + 32);
                        self.set_reg(rd, val);
                    },
                    0b111111 => {
                        // DSRA32 rd, rt, shamt
                        let rd = instr.rd();
                        let rt = instr.rt();
                        let shamt = instr.shamt();
                        log.push_str(&format!("DSRA32 $t{}, $t{}, {}", rd, rt, shamt));
                        let val = ((self.get_reg(rt) as i64) >> (shamt + 32)) as u64;
                        self.set_reg(rd, val);
                    },
                    0b101100 | 0b101101 => {
                        // DADD / DADDU rd, rs, rt
                        let rd = instr.rd();
                        let rs = instr.rs();
                        let rt = instr.rt();
                        log.push_str(&format!("DADDU $t{}, $t{}, $t{}", rd, rs, rt));
                        let val = self.get_reg(rs).wrapping_add(self.get_reg(rt));
                        self.set_reg(rd, val);
                    },
                    0b101110 | 0b101111 => {
                        // DSUB / DSUBU rd, rs, rt
                        let rd = instr.rd();
                        let rs = instr.rs();
                        let rt = instr.rt();
                        log.push_str(&format!("DSUBU $t{}, $t{}, $t{}", rd, rs, rt));
                        let val = self.get_reg(rs).wrapping_sub(self.get_reg(rt));
                        self.set_reg(rd, val);
                    },
                    _ => {
                        log.push_str(&format!("UNKNOWN SPECIAL: {:#08b}", funct));
                        self.trigger_exception(10); // Reserved Instruction (RI)
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
                        let val = self.cop0.read_reg(rd) as u64;
                        self.set_reg(rt, val);
                    },
                    0b00100 => {
                        // MTC0 rt, rd
                        log.push_str(&format!("MTC0 $t{}, COP0_R{}", rt, rd));
                        self.cop0.write_reg(rd, self.get_reg(rt) as u32);
                    },
                    0b10000 => {
                        let funct = instr.funct();
                        if funct == 0b011000 {
                            // ERET
                            log.push_str("ERET");
                            // Set PC to EPC
                            self.next_pc = self.cop0.epc;
                            self.branch = true;
                            
                            // Restore Status register flags (simplified for now)
                            self.cop0.status &= !(1 << 1); // Clear EXL
                        } else {
                            log.push_str(&format!("UNKNOWN COP0 CO: {:#08b}", funct));
                        }
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
            0b000110 => {
                // BLEZ rs, offset
                let rs = instr.rs();
                let offset = instr.imm_sign_extended();
                log.push_str(&format!("BLEZ $t{}, {:#06X}", rs, offset));
                
                let val = self.get_reg(rs) as i64;
                if val <= 0 {
                    let offset_addr = (offset as u32) << 2;
                    self.next_pc = self.pc.wrapping_add(offset_addr);
                    self.branch = true;
                }
            },
            0b000111 => {
                // BGTZ rs, offset
                let rs = instr.rs();
                let offset = instr.imm_sign_extended();
                log.push_str(&format!("BGTZ $t{}, {:#06X}", rs, offset));
                
                let val = self.get_reg(rs) as i64;
                if val > 0 {
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
            0b011110 => {
                // LQ rt, offset(rs) (PS2 Emotion Engine)
                let rs = instr.rs();
                let rt = instr.rt();
                let offset = instr.imm_sign_extended();
                // Mask the lower 4 bits of the address for 16-byte alignment
                let addr = (self.get_reg(rs).wrapping_add(offset as u64) & !0xF) as u32;
                log.push_str(&format!("LQ $t{}, {:#06X}($t{}) [ADDR: {:#010X}]", rt, offset as u16, rs, addr));
                let val = self.bus.read128(addr);
                self.set_reg128(rt, val);
            },
            0b011111 => {
                // SQ rt, offset(rs) (PS2 Emotion Engine)
                let rs = instr.rs();
                let rt = instr.rt();
                let offset = instr.imm_sign_extended();
                // Mask the lower 4 bits of the address for 16-byte alignment
                let addr = (self.get_reg(rs).wrapping_add(offset as u64) & !0xF) as u32;
                log.push_str(&format!("SQ $t{}, {:#06X}($t{}) [ADDR: {:#010X}]", rt, offset as u16, rs, addr));
                // Ignore Cache Isolation bit for EE (PS2 Kernel relies on valid memory writes)
                let val = self.get_reg128(rt);
                self.bus.write128(addr, val);
            },
            0b101011 => {
                // SW rt, offset(rs)
                let rs = instr.rs();
                let rt = instr.rt();
                let offset = instr.imm_sign_extended();
                let addr = self.get_reg(rs).wrapping_add(offset as u64) as u32;
                if (self.cop0.status & 0x10000) == 0 {
                    self.bus.write32(addr, self.get_reg(rt) as u32);
                }
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
                if (self.cop0.status & 0x10000) == 0 {
                    self.bus.write8(addr, (self.get_reg(rt) & 0xFF) as u8);
                }
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
                if (self.cop0.status & 0x10000) == 0 {
                    self.bus.write16(addr, (self.get_reg(rt) & 0xFFFF) as u16);
                }
                log.push_str(&format!("SH $t{}, {:#06X}($t{}) [ADDR: {:#010X}]", rt, offset as u16, rs, addr));
            },
            0b000010 => {
                // J target
                let target = instr.imm_jump_target();
                self.next_pc = (self.pc & 0xF0000000) | (target << 2);
                self.branch = true;
                
                log.push_str(&format!("J {:#010X}", self.next_pc));
            },
            0b000011 => {
                // JAL target
                let target = instr.imm_jump_target();
                self.next_pc = (self.pc & 0xF0000000) | (target << 2);
                self.branch = true;
                
                // JAL stores return address in $ra ($31)
                self.set_reg(31, self.pc.wrapping_add(4) as u64);
                
                log.push_str(&format!("JAL {:#010X}", self.next_pc));
            },
            0b000001 => {
                // REGIMM
                let rt = instr.rt();
                let rs = instr.rs();
                let offset = instr.imm_sign_extended();
                let val = self.get_reg(rs) as i64;
                
                match rt {
                    0b00000 => {
                        // BLTZ
                        log.push_str(&format!("BLTZ $t{}, {:#06X}", rs, offset));
                        if val < 0 {
                            let offset_addr = (offset as u32) << 2;
                            self.next_pc = self.pc.wrapping_add(offset_addr);
                            self.branch = true;
                        }
                    },
                    0b00001 => {
                        // BGEZ
                        log.push_str(&format!("BGEZ $t{}, {:#06X}", rs, offset));
                        if val >= 0 {
                            let offset_addr = (offset as u32) << 2;
                            self.next_pc = self.pc.wrapping_add(offset_addr);
                            self.branch = true;
                        }
                    },
                    _ => {
                        log.push_str(&format!("UNKNOWN REGIMM: {:#07b}", rt));
                    }
                }
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
            0b010001 => {
                // COP1 (FPU) Stub
                log.push_str(&format!("COP1 (FPU) INSTRUCTION: {:#010X}", op));
                // We're stubbing COP1 for now, but to avoid kernel crash, we might need 
                // to either ignore it (treat as NOP) or throw Coprocessor Unusable (11).
                // Let's just ignore it for now to let bootloader progress if it's just initializing FPU.
            },
            _ => {
                log.push_str(&format!("UNKNOWN OPCODE: {:#08b}", opcode));
                self.trigger_exception(10); // Reserved Instruction (RI)
            }
        }
        
        log
    }

    /// High-Level Emulation (HLE) for BIOS Syscalls
    pub fn handle_syscall(&mut self, log: &mut String) {
        let syscall_id = self.get_reg(3); // $v1 typically holds the syscall number in PS2 BIOS
        
        match syscall_id {
            0x01 => {
                // ResetEE
                log.push_str(&format!(" [HLE Syscall 0x01: ResetEE]"));
                self.set_reg(2, 0); // Return 0
            },
            0x02 => {
                // SetGsCrt
                log.push_str(&format!(" [HLE Syscall 0x02: SetGsCrt]"));
                self.set_reg(2, 0); // Return 0
            },
            0x04 => {
                // Exit
                log.push_str(&format!(" [HLE Syscall 0x04: Exit]"));
                self.set_reg(2, 0);
            },
            0x3D => {
                // Putc (char in $a0)
                let c = (self.get_reg(4) & 0xFF) as u8 as char;
                log.push_str(&format!(" [HLE Syscall 0x3D: Putc ('{}')]", c));
                self.set_reg(2, 0);
            },
            0x3E => {
                // Puts (string ptr in $a0)
                let ptr = self.get_reg(4) as u32;
                let s = self.bus.read_string(ptr);
                log.push_str(&format!(" [HLE Syscall 0x3E: Puts (\"{}\")]", s));
                self.set_reg(2, 0);
            },
            _ => {
                // Unimplemented Syscall - Per user feedback, log warning and return 0
                log.push_str(&format!(" [HLE Syscall {:#04X}: UNIMPLEMENTED - Returning 0]", syscall_id));
                self.set_reg(2, 0);
            }
        }
        
        // HLE means we don't jump to the BIOS vector (0x80000180).
        // The SYSCALL instruction's standard behavior in our step loop will naturally 
        // advance the PC to the next instruction (PC + 4). So we do nothing to PC here!
    }

    /// Triggers a MIPS Exception
    pub fn trigger_exception(&mut self, exc_code: u32) {
        let mut epc = self.current_pc;
        let mut cause = self.cop0.cause;
        
        // If in branch delay slot, EPC points to branch instruction
        if self.delay_slot {
            epc = self.current_pc.wrapping_sub(4);
            cause |= 1 << 31; // Branch Delay Bit
        } else {
            cause &= !(1 << 31);
        }

        self.cop0.epc = epc;
        
        // Set Exception Code
        cause &= !(0x1F << 2);
        cause |= (exc_code & 0x1F) << 2;
        self.cop0.cause = cause;
        
        // Push status stack (EXL)
        self.cop0.status |= 1 << 1; // EXL (Exception Level)
        
        // Determine exception vector based on BEV bit and ExcCode
        let bev = (self.cop0.status & (1 << 22)) != 0;
        let vector = if bev {
            0xBFC00180
        } else if exc_code == 0 {
            0x80000200 // PS2 Hardware Interrupt Vector
        } else {
            0x80000180 // General Exception Vector
        };
        
        self.pc = vector;
        self.next_pc = vector.wrapping_add(4);
        self.branch = false;
        self.delay_slot = false;
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hle_puts() {
        let mut ee = EmotionEngine::new(crate::memory::bus::Bus::new());
        
        // Write the string "Hello HLE\0" to RAM at 0x1000
        let s = "Hello HLE\0";
        for (i, b) in s.bytes().enumerate() {
            ee.bus.write8(0x1000 + i as u32, b);
        }
        
        // Construct SYSCALL instruction (opcode 0, funct 0b001100 = 12)
        let syscall_op = 12; // 0x0000000C
        ee.bus.write32(0x00100000, syscall_op);
        
        // Setup registers:
        // $v1 (3) = 0x3E (Puts syscall id)
        // $a0 (4) = 0x1000 (Pointer to string)
        ee.set_reg(3, 0x3E);
        ee.set_reg(4, 0x1000);
        
        ee.set_pc(0x00100000);
        
        // Step
        let log = ee.step();
        
        // Verify output
        assert!(log.contains("SYSCALL"));
        assert!(log.contains("HLE Syscall 0x3E: Puts (\"Hello HLE\")"));
        
        // Verify PC naturally advanced and didn't jump to exception vector
        assert_eq!(ee.pc, 0x00100004);
        
        // Verify return register $v0 (2) is 0
        assert_eq!(ee.get_reg(2), 0);
    }
}
