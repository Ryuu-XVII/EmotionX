use crate::memory::bus::Bus;
use crate::cpu::instruction::Instruction;
use crate::cpu::cop0::Cop0;
use crate::cpu::vu0::Vu0;

/// The Emotion Engine (EE) - Custom MIPS R5900 core
pub struct EmotionEngine {
    // 32 General Purpose Registers (GPRs) (128-bit on Emotion Engine)
    pub gpr: [u128; 32],
    
    // HI / LO Registers for multiplication and division
    pub hi: u64,
    pub lo: u64,
    // The EE has a second, independent HI/LO pair ("pipeline 1"), used by
    // the MULT1/MULTU1/DIV1/DIVU1/MADD1/MADDU1/MFHI1/MTHI1/MFLO1/MTLO1 family.
    pub hi1: u64,
    pub lo1: u64,
    
    // Program Counter
    pub pc: u32,
    pub next_pc: u32,
    
    // Branch Delay Slot tracking
    pub current_pc: u32,
    pub branch: bool,
    pub delay_slot: bool,
    // Set when a "likely" branch is not taken; squashes the following delay slot instruction
    pub nullify: bool,

    // System Coprocessor (COP0)
    pub cop0: Cop0,

    // Coprocessor 1 (FPU) - 32 single-precision registers + control/status
    pub fpr: [f32; 32],
    pub fcr31: u32,
    // EE-specific FPU accumulator, used by ADDA.S/SUBA.S/MULA.S/MADDA.S/MSUBA.S
    pub facc: f32,

    // Coprocessor 2 (VU0 in macro mode)
    pub vu0: Vu0,

    // Shift-amount register set by MTSAB/MTSAH, consumed by QFSRV (not yet implemented)
    pub sa: u32,

    // Fake semaphore ID counter for the HLE'd kernel semaphore syscalls (see handle_syscall).
    // We have no real thread scheduler, so semaphores never actually block - see the note there.
    pub next_sema_id: i32,
    pub next_handler_id: i32,
    pub next_thread_id: i32,

    // Consecutive instruction fetches from unmapped memory (see Bus::is_code_mapped). Used to
    // detect a derailed CPU (jumped somewhere with no real code) so callers can stop instead of
    // executing NOPs forever.
    pub consecutive_unmapped_fetches: u32,

    // Reference to the system bus (Memory)
    pub bus: Bus,
}

impl EmotionEngine {
    pub fn new(bus: Bus) -> Self {
        Self {
            gpr: [0; 32],
            hi: 0,
            lo: 0,
            hi1: 0,
            lo1: 0,
            pc: 0x1FC00000,
            next_pc: 0x1FC00004,
            current_pc: 0x1FC00000,
            branch: false,
            delay_slot: false,
            nullify: false,
            cop0: Cop0::new(),
            fpr: [0.0; 32],
            fcr31: 0,
            facc: 0.0,
            vu0: Vu0::new(),
            sa: 0,
            next_sema_id: 1,
            next_handler_id: 1,
            next_thread_id: 1,
            consecutive_unmapped_fetches: 0,
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
            self.gpr[reg as usize] = Self::guard_sp_zero(reg, (val as i64) as i128 as u128);
        }
    }

    /// Real PS2 kernels never let $sp (register 29) be zero when user code runs - it's always
    /// pre-established by the kernel/loader before any game code executes. We skip that real
    /// kernel/IOP boot sequence, and games' own crt0 startup routinely does a defensive "clear
    /// every GPR" pass early on that (harmlessly, on real hardware, since $sp was never really
    /// zero to begin with) would zero $sp too. On our emulator, without this guard, $sp then
    /// stays at literal 0, and every subsequent stack-frame allocation (`ADDIU sp,sp,-N`)
    /// produces a small negative-wrapped address like 0xFFFFFFF0 that - after physical masking
    /// (`& 0x1FFFFFFF`) - lands inside the read-only BIOS ROM's mapped region (which sits right
    /// at the top of that 29-bit address window, so any small wrap from zero always lands
    /// there). Every stack save then silently goes nowhere and every restore reads back garbage
    /// BIOS bytes, eventually corrupting a saved return address enough to jump into unmapped
    /// memory. This substitutes a sane stack-top value instead of ever allowing that.
    fn guard_sp_zero(reg: u32, val: u128) -> u128 {
        const BOOT_STACK_TOP: u128 = 0x81FFFFF0;
        if reg == 29 && val == 0 {
            BOOT_STACK_TOP
        } else {
            val
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
            self.gpr[reg as usize] = Self::guard_sp_zero(reg, val);
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
            self.bus.hw.gs.toggle_vblank();
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

        if self.bus.is_code_mapped(self.current_pc) {
            self.consecutive_unmapped_fetches = 0;
        } else {
            self.consecutive_unmapped_fetches += 1;
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

        // A "likely" branch that wasn't taken squashes its delay slot instruction
        if self.nullify {
            self.nullify = false;
            log.push_str("NOP (squashed branch-likely delay slot)");
            return log;
        }

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
                    0b001111 => {
                        // SYNC - memory ordering barrier; a no-op for our single-threaded interpreter
                        log.push_str("SYNC");
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
            0b010100 => {
                // BEQL rs, rt, offset
                let rs = instr.rs();
                let rt = instr.rt();
                let offset = instr.imm_sign_extended();
                log.push_str(&format!("BEQL $t{}, $t{}, {:#06X}", rs, rt, offset));

                if self.get_reg(rs) == self.get_reg(rt) {
                    let offset_addr = (offset as u32) << 2;
                    self.next_pc = self.pc.wrapping_add(offset_addr);
                    self.branch = true;
                } else {
                    self.nullify = true;
                }
            },
            0b010101 => {
                // BNEL rs, rt, offset
                let rs = instr.rs();
                let rt = instr.rt();
                let offset = instr.imm_sign_extended();
                log.push_str(&format!("BNEL $t{}, $t{}, {:#06X}", rs, rt, offset));

                if self.get_reg(rs) != self.get_reg(rt) {
                    let offset_addr = (offset as u32) << 2;
                    self.next_pc = self.pc.wrapping_add(offset_addr);
                    self.branch = true;
                } else {
                    self.nullify = true;
                }
            },
            0b010110 => {
                // BLEZL rs, offset
                let rs = instr.rs();
                let offset = instr.imm_sign_extended();
                log.push_str(&format!("BLEZL $t{}, {:#06X}", rs, offset));

                let val = self.get_reg(rs) as i64;
                if val <= 0 {
                    let offset_addr = (offset as u32) << 2;
                    self.next_pc = self.pc.wrapping_add(offset_addr);
                    self.branch = true;
                } else {
                    self.nullify = true;
                }
            },
            0b010111 => {
                // BGTZL rs, offset
                let rs = instr.rs();
                let offset = instr.imm_sign_extended();
                log.push_str(&format!("BGTZL $t{}, {:#06X}", rs, offset));

                let val = self.get_reg(rs) as i64;
                if val > 0 {
                    let offset_addr = (offset as u32) << 2;
                    self.next_pc = self.pc.wrapping_add(offset_addr);
                    self.branch = true;
                } else {
                    self.nullify = true;
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
            0b100010 => {
                // LWL rt, offset(rs) - Load Word Left (unaligned)
                const LWL_MASK: [u32; 4] = [0x00FFFFFF, 0x0000FFFF, 0x000000FF, 0x00000000];
                const LWL_SHIFT: [u32; 4] = [24, 16, 8, 0];
                let rt = instr.rt();
                let rs = instr.rs();
                let offset = instr.imm_sign_extended();
                let addr = (self.get_reg(rs) as u32).wrapping_add(offset as u32);
                log.push_str(&format!("LWL $t{}, {:#06X}($t{}) [ADDR: {:#010X}]", rt, offset as u16, rs, addr));

                let shift = (addr & 3) as usize;
                let mem = self.bus.read32(addr & !3);
                let rt_val = self.get_reg(rt) as u32;
                let val = (rt_val & LWL_MASK[shift]) | (mem << LWL_SHIFT[shift]);
                self.set_reg(rt, val as i32 as i64 as u64);
            },
            0b100110 => {
                // LWR rt, offset(rs) - Load Word Right (unaligned)
                const LWR_MASK: [u32; 4] = [0x00000000, 0xFF000000, 0xFFFF0000, 0xFFFFFF00];
                const LWR_SHIFT: [u32; 4] = [0, 8, 16, 24];
                let rt = instr.rt();
                let rs = instr.rs();
                let offset = instr.imm_sign_extended();
                let addr = (self.get_reg(rs) as u32).wrapping_add(offset as u32);
                log.push_str(&format!("LWR $t{}, {:#06X}($t{}) [ADDR: {:#010X}]", rt, offset as u16, rs, addr));

                let shift = (addr & 3) as usize;
                let mem = self.bus.read32(addr & !3);
                let rt_val = self.get_reg(rt) as u32;
                let val = (rt_val & LWR_MASK[shift]) | (mem >> LWR_SHIFT[shift]);
                self.set_reg(rt, val as i32 as i64 as u64);
            },
            0b101010 => {
                // SWL rt, offset(rs) - Store Word Left (unaligned)
                const SWL_MASK: [u32; 4] = [0xFFFFFF00, 0xFFFF0000, 0xFF000000, 0x00000000];
                const SWL_SHIFT: [u32; 4] = [24, 16, 8, 0];
                let rt = instr.rt();
                let rs = instr.rs();
                let offset = instr.imm_sign_extended();
                let addr = (self.get_reg(rs) as u32).wrapping_add(offset as u32);
                log.push_str(&format!("SWL $t{}, {:#06X}($t{}) [ADDR: {:#010X}]", rt, offset as u16, rs, addr));

                if (self.cop0.status & 0x10000) == 0 {
                    let shift = (addr & 3) as usize;
                    let aligned = addr & !3;
                    let mem = self.bus.read32(aligned);
                    let rt_val = self.get_reg(rt) as u32;
                    let val = (mem & SWL_MASK[shift]) | (rt_val >> SWL_SHIFT[shift]);
                    self.bus.write32(aligned, val);
                }
            },
            0b101110 => {
                // SWR rt, offset(rs) - Store Word Right (unaligned)
                const SWR_MASK: [u32; 4] = [0x00000000, 0x000000FF, 0x0000FFFF, 0x00FFFFFF];
                const SWR_SHIFT: [u32; 4] = [0, 8, 16, 24];
                let rt = instr.rt();
                let rs = instr.rs();
                let offset = instr.imm_sign_extended();
                let addr = (self.get_reg(rs) as u32).wrapping_add(offset as u32);
                log.push_str(&format!("SWR $t{}, {:#06X}($t{}) [ADDR: {:#010X}]", rt, offset as u16, rs, addr));

                if (self.cop0.status & 0x10000) == 0 {
                    let shift = (addr & 3) as usize;
                    let aligned = addr & !3;
                    let mem = self.bus.read32(aligned);
                    let rt_val = self.get_reg(rt) as u32;
                    let val = (mem & SWR_MASK[shift]) | (rt_val << SWR_SHIFT[shift]);
                    self.bus.write32(aligned, val);
                }
            },
            0b011010 => {
                // LDL rt, offset(rs) - Load Doubleword Left (unaligned), 64-bit analog of LWL
                const LDL_MASK: [u64; 8] = [
                    0x00FFFFFFFFFFFFFF, 0x0000FFFFFFFFFFFF, 0x000000FFFFFFFFFF, 0x00000000FFFFFFFF,
                    0x0000000000FFFFFF, 0x000000000000FFFF, 0x00000000000000FF, 0x0000000000000000,
                ];
                const LDL_SHIFT: [u32; 8] = [56, 48, 40, 32, 24, 16, 8, 0];
                let rt = instr.rt();
                let rs = instr.rs();
                let offset = instr.imm_sign_extended();
                let addr = (self.get_reg(rs) as u32).wrapping_add(offset as u32);
                log.push_str(&format!("LDL $t{}, {:#06X}($t{}) [ADDR: {:#010X}]", rt, offset as u16, rs, addr));

                let shift = (addr & 7) as usize;
                let mem = self.bus.read64(addr & !7);
                let rt_val = self.get_reg(rt);
                let val = (rt_val & LDL_MASK[shift]) | (mem << LDL_SHIFT[shift]);
                self.set_reg(rt, val);
            },
            0b011011 => {
                // LDR rt, offset(rs) - Load Doubleword Right (unaligned), 64-bit analog of LWR
                const LDR_MASK: [u64; 8] = [
                    0x0000000000000000, 0xFF00000000000000, 0xFFFF000000000000, 0xFFFFFF0000000000,
                    0xFFFFFFFF00000000, 0xFFFFFFFFFF000000, 0xFFFFFFFFFFFF0000, 0xFFFFFFFFFFFFFF00,
                ];
                const LDR_SHIFT: [u32; 8] = [0, 8, 16, 24, 32, 40, 48, 56];
                let rt = instr.rt();
                let rs = instr.rs();
                let offset = instr.imm_sign_extended();
                let addr = (self.get_reg(rs) as u32).wrapping_add(offset as u32);
                log.push_str(&format!("LDR $t{}, {:#06X}($t{}) [ADDR: {:#010X}]", rt, offset as u16, rs, addr));

                let shift = (addr & 7) as usize;
                let mem = self.bus.read64(addr & !7);
                let rt_val = self.get_reg(rt);
                let val = (rt_val & LDR_MASK[shift]) | (mem >> LDR_SHIFT[shift]);
                self.set_reg(rt, val);
            },
            0b101100 => {
                // SDL rt, offset(rs) - Store Doubleword Left (unaligned), 64-bit analog of SWL
                const SDL_MASK: [u64; 8] = [
                    0xFFFFFFFFFFFFFF00, 0xFFFFFFFFFFFF0000, 0xFFFFFFFFFF000000, 0xFFFFFFFF00000000,
                    0xFFFFFF0000000000, 0xFFFF000000000000, 0xFF00000000000000, 0x0000000000000000,
                ];
                const SDL_SHIFT: [u32; 8] = [56, 48, 40, 32, 24, 16, 8, 0];
                let rt = instr.rt();
                let rs = instr.rs();
                let offset = instr.imm_sign_extended();
                let addr = (self.get_reg(rs) as u32).wrapping_add(offset as u32);
                log.push_str(&format!("SDL $t{}, {:#06X}($t{}) [ADDR: {:#010X}]", rt, offset as u16, rs, addr));

                if (self.cop0.status & 0x10000) == 0 {
                    let shift = (addr & 7) as usize;
                    let aligned = addr & !7;
                    let mem = self.bus.read64(aligned);
                    let rt_val = self.get_reg(rt);
                    let val = (mem & SDL_MASK[shift]) | (rt_val >> SDL_SHIFT[shift]);
                    self.bus.write64(aligned, val);
                }
            },
            0b101101 => {
                // SDR rt, offset(rs) - Store Doubleword Right (unaligned), 64-bit analog of SWR
                const SDR_MASK: [u64; 8] = [
                    0x0000000000000000, 0x00000000000000FF, 0x000000000000FFFF, 0x0000000000FFFFFF,
                    0x00000000FFFFFFFF, 0x000000FFFFFFFFFF, 0x0000FFFFFFFFFFFF, 0x00FFFFFFFFFFFFFF,
                ];
                const SDR_SHIFT: [u32; 8] = [0, 8, 16, 24, 32, 40, 48, 56];
                let rt = instr.rt();
                let rs = instr.rs();
                let offset = instr.imm_sign_extended();
                let addr = (self.get_reg(rs) as u32).wrapping_add(offset as u32);
                log.push_str(&format!("SDR $t{}, {:#06X}($t{}) [ADDR: {:#010X}]", rt, offset as u16, rs, addr));

                if (self.cop0.status & 0x10000) == 0 {
                    let shift = (addr & 7) as usize;
                    let aligned = addr & !7;
                    let mem = self.bus.read64(aligned);
                    let rt_val = self.get_reg(rt);
                    let val = (mem & SDR_MASK[shift]) | (rt_val << SDR_SHIFT[shift]);
                    self.bus.write64(aligned, val);
                }
            },
            0b110000 => {
                // LL rt, offset(rs) - Load Linked (32-bit). Single-core simplification:
                // the reservation always succeeds, so this behaves like a plain LW.
                let rt = instr.rt();
                let rs = instr.rs();
                let offset = instr.imm_sign_extended();
                let addr = (self.get_reg(rs) as u32).wrapping_add(offset as u32);
                log.push_str(&format!("LL $t{}, {:#06X}($t{}) [ADDR: {:#010X}]", rt, offset as u16, rs, addr));
                let val = self.bus.read32(addr) as i32 as i64 as u64;
                self.set_reg(rt, val);
            },
            0b111000 => {
                // SC rt, offset(rs) - Store Conditional (32-bit). Single-core simplification:
                // the store always succeeds, so rt is unconditionally set to 1.
                let rt = instr.rt();
                let rs = instr.rs();
                let offset = instr.imm_sign_extended();
                let addr = (self.get_reg(rs) as u32).wrapping_add(offset as u32);
                log.push_str(&format!("SC $t{}, {:#06X}($t{}) [ADDR: {:#010X}]", rt, offset as u16, rs, addr));
                if (self.cop0.status & 0x10000) == 0 {
                    self.bus.write32(addr, self.get_reg(rt) as u32);
                }
                self.set_reg(rt, 1);
            },
            0b110100 => {
                // LLD rt, offset(rs) - Load Linked Doubleword (64-bit)
                let rt = instr.rt();
                let rs = instr.rs();
                let offset = instr.imm_sign_extended();
                let addr = (self.get_reg(rs) as u32).wrapping_add(offset as u32);
                log.push_str(&format!("LLD $t{}, {:#06X}($t{}) [ADDR: {:#010X}]", rt, offset as u16, rs, addr));
                let val = self.bus.read64(addr);
                self.set_reg(rt, val);
            },
            0b111100 => {
                // SCD rt, offset(rs) - Store Conditional Doubleword (64-bit)
                let rt = instr.rt();
                let rs = instr.rs();
                let offset = instr.imm_sign_extended();
                let addr = (self.get_reg(rs) as u32).wrapping_add(offset as u32);
                log.push_str(&format!("SCD $t{}, {:#06X}($t{}) [ADDR: {:#010X}]", rt, offset as u16, rs, addr));
                if (self.cop0.status & 0x10000) == 0 {
                    self.bus.write64(addr, self.get_reg(rt));
                }
                self.set_reg(rt, 1);
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
                    0b00010 => {
                        // BLTZL
                        log.push_str(&format!("BLTZL $t{}, {:#06X}", rs, offset));
                        if val < 0 {
                            let offset_addr = (offset as u32) << 2;
                            self.next_pc = self.pc.wrapping_add(offset_addr);
                            self.branch = true;
                        } else {
                            self.nullify = true;
                        }
                    },
                    0b00011 => {
                        // BGEZL
                        log.push_str(&format!("BGEZL $t{}, {:#06X}", rs, offset));
                        if val >= 0 {
                            let offset_addr = (offset as u32) << 2;
                            self.next_pc = self.pc.wrapping_add(offset_addr);
                            self.branch = true;
                        } else {
                            self.nullify = true;
                        }
                    },
                    0b10000 => {
                        // BLTZAL
                        log.push_str(&format!("BLTZAL $t{}, {:#06X}", rs, offset));
                        self.set_reg(31, self.pc.wrapping_add(4) as u64);
                        if val < 0 {
                            let offset_addr = (offset as u32) << 2;
                            self.next_pc = self.pc.wrapping_add(offset_addr);
                            self.branch = true;
                        }
                    },
                    0b10001 => {
                        // BGEZAL
                        log.push_str(&format!("BGEZAL $t{}, {:#06X}", rs, offset));
                        self.set_reg(31, self.pc.wrapping_add(4) as u64);
                        if val >= 0 {
                            let offset_addr = (offset as u32) << 2;
                            self.next_pc = self.pc.wrapping_add(offset_addr);
                            self.branch = true;
                        }
                    },
                    0b10010 => {
                        // BLTZALL
                        log.push_str(&format!("BLTZALL $t{}, {:#06X}", rs, offset));
                        self.set_reg(31, self.pc.wrapping_add(4) as u64);
                        if val < 0 {
                            let offset_addr = (offset as u32) << 2;
                            self.next_pc = self.pc.wrapping_add(offset_addr);
                            self.branch = true;
                        } else {
                            self.nullify = true;
                        }
                    },
                    0b10011 => {
                        // BGEZALL
                        log.push_str(&format!("BGEZALL $t{}, {:#06X}", rs, offset));
                        self.set_reg(31, self.pc.wrapping_add(4) as u64);
                        if val >= 0 {
                            let offset_addr = (offset as u32) << 2;
                            self.next_pc = self.pc.wrapping_add(offset_addr);
                            self.branch = true;
                        } else {
                            self.nullify = true;
                        }
                    },
                    0b11000 => {
                        // MTSAB rs, imm - sets the SA (shift-amount) register, byte granularity
                        let imm = instr.imm_sign_extended();
                        log.push_str(&format!("MTSAB $t{}, {:#06X}", rs, instr.imm()));
                        self.sa = (self.get_reg(rs).wrapping_add(imm) & 0xF) as u32;
                    },
                    0b11001 => {
                        // MTSAH rs, imm - sets the SA register, halfword granularity
                        let imm = instr.imm_sign_extended();
                        log.push_str(&format!("MTSAH $t{}, {:#06X}", rs, instr.imm()));
                        self.sa = ((self.get_reg(rs).wrapping_add(imm) & 0x7) as u32) * 2;
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
                // COP1 (FPU)
                self.execute_cop1(&instr, &mut log);
            },
            0b010010 => {
                // COP2 (VU0 macro mode)
                self.execute_cop2(&instr, &mut log);
            },
            0b011100 => {
                // SPECIAL2 / MMI (MultiMedia Instructions)
                self.execute_mmi(&instr, &mut log);
            },
            0b110010 => {
                // LQC2 ft, offset(rs) - load a quadword from memory into a VF register
                let ft = instr.rt();
                let rs = instr.rs();
                let offset = instr.imm_sign_extended();
                let addr = (self.get_reg(rs) as u32).wrapping_add(offset as u32);
                log.push_str(&format!("LQC2 vf{}, {:#06X}($t{}) [ADDR: {:#010X}]", ft, offset as u16, rs, addr));
                let bytes = self.bus.read128(addr).to_le_bytes();
                let mut v = [0f32; 4];
                for c in 0..4 {
                    let mut b = [0u8; 4];
                    b.copy_from_slice(&bytes[c * 4..c * 4 + 4]);
                    v[c] = f32::from_le_bytes(b);
                }
                self.vu0.set_vf_masked(ft as usize, v, 0xF);
            },
            0b111010 => {
                // SQC2 ft, offset(rs) - store a VF register to memory as a quadword
                let ft = instr.rt();
                let rs = instr.rs();
                let offset = instr.imm_sign_extended();
                let addr = (self.get_reg(rs) as u32).wrapping_add(offset as u32);
                log.push_str(&format!("SQC2 vf{}, {:#06X}($t{}) [ADDR: {:#010X}]", ft, offset as u16, rs, addr));
                if (self.cop0.status & 0x10000) == 0 {
                    let v = self.vu0.get_vf(ft as usize);
                    let mut bytes = [0u8; 16];
                    for c in 0..4 {
                        bytes[c * 4..c * 4 + 4].copy_from_slice(&v[c].to_le_bytes());
                    }
                    self.bus.write128(addr, u128::from_le_bytes(bytes));
                }
            },
            0b110111 => {
                // LD rt, offset(rs) - Load 64-bit doubleword
                let rt = instr.rt();
                let rs = instr.rs();
                let offset = instr.imm_sign_extended();
                let addr = (self.get_reg(rs) as u32).wrapping_add(offset as u32);
                log.push_str(&format!("LD $t{}, {:#06X}($t{}) [ADDR: {:#010X}]", rt, offset as u16, rs, addr));
                let val = self.bus.read64(addr);
                self.set_reg(rt, val);
            },
            0b111111 => {
                // SD rt, offset(rs) - Store 64-bit doubleword
                let rt = instr.rt();
                let rs = instr.rs();
                let offset = instr.imm_sign_extended();
                let addr = (self.get_reg(rs) as u32).wrapping_add(offset as u32);
                log.push_str(&format!("SD $t{}, {:#06X}($t{}) [ADDR: {:#010X}]", rt, offset as u16, rs, addr));
                if (self.cop0.status & 0x10000) == 0 {
                    self.bus.write64(addr, self.get_reg(rt));
                }
            },
            0b110001 => {
                // LWC1 ft, offset(rs) - Load word into FPU register
                let ft = instr.rt();
                let rs = instr.rs();
                let offset = instr.imm_sign_extended();
                let addr = (self.get_reg(rs) as u32).wrapping_add(offset as u32);
                log.push_str(&format!("LWC1 $f{}, {:#06X}($t{}) [ADDR: {:#010X}]", ft, offset as u16, rs, addr));
                let bits = self.bus.read32(addr);
                self.fpr[ft as usize] = f32::from_bits(bits);
            },
            0b111001 => {
                // SWC1 ft, offset(rs) - Store FPU register to memory
                let ft = instr.rt();
                let rs = instr.rs();
                let offset = instr.imm_sign_extended();
                let addr = (self.get_reg(rs) as u32).wrapping_add(offset as u32);
                log.push_str(&format!("SWC1 $f{}, {:#06X}($t{}) [ADDR: {:#010X}]", ft, offset as u16, rs, addr));
                if (self.cop0.status & 0x10000) == 0 {
                    self.bus.write32(addr, self.fpr[ft as usize].to_bits());
                }
            },
            0b011000 => {
                // DADDI rt, rs, imm - 64-bit immediate add (overflow trap not modeled, same simplification as ADDI)
                let rt = instr.rt();
                let rs = instr.rs();
                let imm = instr.imm_sign_extended();
                log.push_str(&format!("DADDI $t{}, $t{}, {:#06X}", rt, rs, instr.imm()));
                let val = self.get_reg(rs).wrapping_add(imm);
                self.set_reg(rt, val);
            },
            0b011001 => {
                // DADDIU rt, rs, imm - 64-bit immediate add unsigned
                let rt = instr.rt();
                let rs = instr.rs();
                let imm = instr.imm_sign_extended();
                log.push_str(&format!("DADDIU $t{}, $t{}, {:#06X}", rt, rs, instr.imm()));
                let val = self.get_reg(rs).wrapping_add(imm);
                self.set_reg(rt, val);
            },
            0b100111 => {
                // LWU rt, offset(rs) - Load Word Unsigned (zero-extended, vs LW's sign extension)
                let rt = instr.rt();
                let rs = instr.rs();
                let offset = instr.imm_sign_extended();
                let addr = (self.get_reg(rs) as u32).wrapping_add(offset as u32);
                log.push_str(&format!("LWU $t{}, {:#06X}($t{}) [ADDR: {:#010X}]", rt, offset as u16, rs, addr));
                let val = self.bus.read32(addr) as u64; // Zero extend
                self.set_reg(rt, val);
            },
            0b110011 => {
                // PREF - prefetch hint. No cache to model; safe no-op.
                log.push_str("PREF");
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
            // INTC / DMAC Handler management
            0x10 => {
                // AddIntcHandler(cause, handler, next, arg)
                let id = self.next_handler_id;
                self.next_handler_id += 1;
                log.push_str(&format!(" [HLE Syscall 0x10: AddIntcHandler -> id {}]", id));
                self.set_reg(2, id as u64);
            },
            0x11 => {
                // RemoveIntcHandler(cause, id)
                log.push_str(" [HLE Syscall 0x11: RemoveIntcHandler]");
                self.set_reg(2, 0);
            },
            0x12 => {
                // EnableIntc(cause)
                log.push_str(" [HLE Syscall 0x12: EnableIntc]");
                self.set_reg(2, 0);
            },
            0x13 => {
                // DisableIntc(cause)
                log.push_str(" [HLE Syscall 0x13: DisableIntc]");
                self.set_reg(2, 0);
            },
            0x14 => {
                // AddDmacHandler(channel, handler, next, arg)
                let id = self.next_handler_id;
                self.next_handler_id += 1;
                log.push_str(&format!(" [HLE Syscall 0x14: AddDmacHandler -> id {}]", id));
                self.set_reg(2, id as u64);
            },
            0x15 => {
                // RemoveDmacHandler(channel, id)
                log.push_str(" [HLE Syscall 0x15: RemoveDmacHandler]");
                self.set_reg(2, 0);
            },
            0x16 => {
                // EnableDmac(channel)
                log.push_str(" [HLE Syscall 0x16: EnableDmac]");
                self.set_reg(2, 0);
            },
            0x17 => {
                // DisableDmac(channel)
                log.push_str(" [HLE Syscall 0x17: DisableDmac]");
                self.set_reg(2, 0);
            },
            // Thread management
            0x20 => {
                // CreateThread(thread_param)
                let tid = self.next_thread_id;
                self.next_thread_id += 1;
                log.push_str(&format!(" [HLE Syscall 0x20: CreateThread -> id {}]", tid));
                self.set_reg(2, tid as u64);
            },
            0x22 => {
                // StartThread(thread_id, args)
                log.push_str(" [HLE Syscall 0x22: StartThread]");
                self.set_reg(2, 0);
            },
            0x23 => {
                // ExitThread()
                log.push_str(" [HLE Syscall 0x23: ExitThread]");
                self.set_reg(2, 0);
            },
            0x24 => {
                // ExitDeleteThread()
                log.push_str(" [HLE Syscall 0x24: ExitDeleteThread]");
                self.set_reg(2, 0);
            },
            0x29 => {
                // RotateThreadReadyQueue(priority)
                log.push_str(" [HLE Syscall 0x29: RotateThreadReadyQueue]");
                self.set_reg(2, 0);
            },
            // Alarm management
            0x2C => {
                // SetAlarm(time, callback, arg)
                let id = self.next_handler_id;
                self.next_handler_id += 1;
                log.push_str(&format!(" [HLE Syscall 0x2C: SetAlarm -> id {}]", id));
                self.set_reg(2, id as u64);
            },
            0x2D => {
                // ReleaseAlarm(id)
                log.push_str(" [HLE Syscall 0x2D: ReleaseAlarm]");
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
            // Kernel semaphore syscalls. We have no real thread scheduler, so there's no
            // meaningful way to actually block a thread on WaitSema - instead every one of
            // these succeeds immediately, which is a safe simplification (never deadlocks)
            // at the cost of not modeling real inter-thread synchronization. This is what
            // lets synchronous SIF RPC binds (sceSifBindRpc etc., which wait on a semaphore
            // signaled by the SIF response) proceed once the DMAC's SIF1 bind handler below
            // has already written a fake-but-plausible response into the client struct.
            0x40 => {
                // CreateSema
                let id = self.next_sema_id;
                self.next_sema_id += 1;
                log.push_str(&format!(" [HLE Syscall 0x40: CreateSema -> id {}]", id));
                self.set_reg(2, id as u64);
            },
            0x41 => {
                // DeleteSema
                log.push_str(" [HLE Syscall 0x41: DeleteSema]");
                self.set_reg(2, 0);
            },
            0x42 => {
                // SignalSema
                log.push_str(" [HLE Syscall 0x42: SignalSema]");
                self.set_reg(2, 0);
            },
            0x44 => {
                // WaitSema - always succeeds immediately, see note above
                log.push_str(" [HLE Syscall 0x44: WaitSema]");
                self.set_reg(2, 0);
            },
            0x45 => {
                // PollSema
                log.push_str(" [HLE Syscall 0x45: PollSema]");
                self.set_reg(2, 0);
            },
            0x47 => {
                // ReferSemaStatus
                log.push_str(" [HLE Syscall 0x47: ReferSemaStatus]");
                self.set_reg(2, 0);
            },
            0x7C => {
                // GetOsdConfigParam(config_ptr)
                let ptr = self.get_reg(4) as u32;
                if ptr != 0 {
                    // Zero-initialize config struct (English language, NTSC, 4:3, standard time)
                    for i in 0..32 {
                        self.bus.write8(ptr.wrapping_add(i), 0);
                    }
                }
                log.push_str(" [HLE Syscall 0x7C: GetOsdConfigParam]");
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

    /// Executes a COP1 (FPU) instruction. Single-precision only (no doubles on the EE FPU).
    pub fn execute_cop1(&mut self, instr: &Instruction, log: &mut String) {
        let rs = instr.rs(); // sub-op / format field
        let rt = instr.rt(); // GPR index for transfer ops
        let fs = instr.rd(); // FPR index (bits 15-11)
        let fd = instr.shamt(); // FPR index (bits 10-6)

        match rs {
            0b00000 => {
                // MFC1 rt, fs
                log.push_str(&format!("MFC1 $t{}, $f{}", rt, fs));
                let bits = self.fpr[fs as usize].to_bits();
                self.set_reg(rt, bits as i32 as i64 as u64);
            },
            0b00010 => {
                // CFC1 rt, fs (only FCR31 is meaningful on the EE)
                log.push_str(&format!("CFC1 $t{}, FCR{}", rt, fs));
                let val = if fs == 31 { self.fcr31 } else { 0 };
                self.set_reg(rt, val as i32 as i64 as u64);
            },
            0b00100 => {
                // MTC1 rt, fs
                log.push_str(&format!("MTC1 $t{}, $f{}", rt, fs));
                self.fpr[fs as usize] = f32::from_bits(self.get_reg(rt) as u32);
            },
            0b00110 => {
                // CTC1 rt, fs
                log.push_str(&format!("CTC1 $t{}, FCR{}", rt, fs));
                if fs == 31 {
                    self.fcr31 = self.get_reg(rt) as u32;
                }
            },
            0b01000 => {
                // BC1F / BC1T / BC1FL / BC1TL
                let cond = (self.fcr31 >> 23) & 1;
                let offset = instr.imm_sign_extended();
                let likely = (rt & 0b10) != 0;
                let want_true = (rt & 0b01) != 0;

                let taken = (cond == 1) == want_true;
                log.push_str(&format!("BC1{}{} {:#06X}", if want_true { "T" } else { "F" }, if likely { "L" } else { "" }, offset));

                if taken {
                    let offset_addr = (offset as u32) << 2;
                    self.next_pc = self.pc.wrapping_add(offset_addr);
                    self.branch = true;
                } else if likely {
                    self.nullify = true;
                }
            },
            0b10000 => {
                // Single-precision ("S") arithmetic/comparison ops
                let funct = instr.funct();
                let a = self.fpr[fs as usize];
                let b = self.fpr[instr.rt() as usize]; // ft
                match funct {
                    0b000000 => { log.push_str(&format!("ADD.S $f{}, $f{}, $f{}", fd, fs, rt)); self.fpr[fd as usize] = a + b; },
                    0b000001 => { log.push_str(&format!("SUB.S $f{}, $f{}, $f{}", fd, fs, rt)); self.fpr[fd as usize] = a - b; },
                    0b000010 => { log.push_str(&format!("MUL.S $f{}, $f{}, $f{}", fd, fs, rt)); self.fpr[fd as usize] = a * b; },
                    0b000011 => { log.push_str(&format!("DIV.S $f{}, $f{}, $f{}", fd, fs, rt)); self.fpr[fd as usize] = a / b; },
                    0b000100 => { log.push_str(&format!("SQRT.S $f{}, $f{}", fd, fs)); self.fpr[fd as usize] = a.sqrt(); },
                    0b000101 => { log.push_str(&format!("ABS.S $f{}, $f{}", fd, fs)); self.fpr[fd as usize] = a.abs(); },
                    0b000110 => { log.push_str(&format!("MOV.S $f{}, $f{}", fd, fs)); self.fpr[fd as usize] = a; },
                    0b000111 => { log.push_str(&format!("NEG.S $f{}, $f{}", fd, fs)); self.fpr[fd as usize] = -a; },
                    0b001010 => {
                        // RSQRT.S fd, fs, ft - fd = fs / sqrt(ft)
                        log.push_str(&format!("RSQRT.S $f{}, $f{}, $f{}", fd, fs, rt));
                        self.fpr[fd as usize] = a / b.sqrt();
                    },
                    0b001011 => {
                        // ADDA.S fs, ft - ACC = fs + ft (EE FPU accumulator extension)
                        log.push_str(&format!("ADDA.S $f{}, $f{}", fs, rt));
                        self.facc = a + b;
                    },
                    0b001100 => {
                        // SUBA.S fs, ft - ACC = fs - ft
                        log.push_str(&format!("SUBA.S $f{}, $f{}", fs, rt));
                        self.facc = a - b;
                    },
                    0b001101 => {
                        // MULA.S fs, ft - ACC = fs * ft
                        log.push_str(&format!("MULA.S $f{}, $f{}", fs, rt));
                        self.facc = a * b;
                    },
                    0b001110 => {
                        // MADD.S fd, fs, ft - fd = ACC + fs*ft (does not modify ACC)
                        log.push_str(&format!("MADD.S $f{}, $f{}, $f{}", fd, fs, rt));
                        self.fpr[fd as usize] = self.facc + a * b;
                    },
                    0b001111 => {
                        // MSUB.S fd, fs, ft - fd = ACC - fs*ft
                        log.push_str(&format!("MSUB.S $f{}, $f{}, $f{}", fd, fs, rt));
                        self.fpr[fd as usize] = self.facc - a * b;
                    },
                    0b010000 => {
                        // MADDA.S fs, ft - ACC += fs*ft
                        log.push_str(&format!("MADDA.S $f{}, $f{}", fs, rt));
                        self.facc += a * b;
                    },
                    0b010001 => {
                        // MSUBA.S fs, ft - ACC -= fs*ft
                        log.push_str(&format!("MSUBA.S $f{}, $f{}", fs, rt));
                        self.facc -= a * b;
                    },
                    0b011000 => {
                        // MAX.S fd, fs, ft
                        log.push_str(&format!("MAX.S $f{}, $f{}, $f{}", fd, fs, rt));
                        self.fpr[fd as usize] = a.max(b);
                    },
                    0b011001 => {
                        // MIN.S fd, fs, ft
                        log.push_str(&format!("MIN.S $f{}, $f{}, $f{}", fd, fs, rt));
                        self.fpr[fd as usize] = a.min(b);
                    },
                    0b100100 => {
                        // CVT.W.S fd, fs - convert float to 32-bit int (truncate toward zero)
                        log.push_str(&format!("CVT.W.S $f{}, $f{}", fd, fs));
                        self.fpr[fd as usize] = f32::from_bits(a.trunc() as i32 as u32);
                    },
                    0b110010 | 0b110000 => {
                        // C.EQ.S / C.F.S
                        let result = funct == 0b110010 && a == b;
                        log.push_str(&format!("C.EQ.S $f{}, $f{}", fs, rt));
                        self.set_fp_condition(result);
                    },
                    0b110100 | 0b111100 => {
                        // C.OLT.S / C.LT.S
                        log.push_str(&format!("C.LT.S $f{}, $f{}", fs, rt));
                        self.set_fp_condition(a < b);
                    },
                    0b110110 | 0b111110 => {
                        // C.OLE.S / C.LE.S
                        log.push_str(&format!("C.LE.S $f{}, $f{}", fs, rt));
                        self.set_fp_condition(a <= b);
                    },
                    _ => {
                        log.push_str(&format!("UNKNOWN COP1.S: {:#08b}", funct));
                    }
                }
            },
            0b10100 => {
                // Word format ("W") - CVT.S.W
                let funct = instr.funct();
                match funct {
                    0b100000 => {
                        // CVT.S.W fd, fs - convert 32-bit int to float
                        log.push_str(&format!("CVT.S.W $f{}, $f{}", fd, fs));
                        let bits = self.fpr[fs as usize].to_bits() as i32;
                        self.fpr[fd as usize] = bits as f32;
                    },
                    _ => {
                        log.push_str(&format!("UNKNOWN COP1.W: {:#08b}", funct));
                    }
                }
            },
            _ => {
                log.push_str(&format!("UNKNOWN COP1: {:#07b}", rs));
            }
        }
    }

    fn set_fp_condition(&mut self, val: bool) {
        if val {
            self.fcr31 |= 1 << 23;
        } else {
            self.fcr31 &= !(1 << 23);
        }
    }

    /// Executes a COP2 (VU0 macro mode) instruction. See `cpu::vu0` for the
    /// scope and confidence notes on which opcodes are implemented.
    pub fn execute_cop2(&mut self, instr: &Instruction, log: &mut String) {
        let rs = instr.rs();
        let rt = instr.rt();
        let id = instr.rd(); // VF/VI register id for transfer ops (bits 15-11)

        if rs & 0x10 != 0 {
            // FMAC arithmetic macro instruction: rs's low 4 bits are the dest write-mask.
            let dest_mask = rs & 0xF;
            let ft_idx = rt as usize;
            let fs_idx = instr.rd() as usize;
            let fd_idx = instr.shamt() as usize;
            let funct = instr.funct();

            let fs = self.vu0.get_vf(fs_idx);
            let ft = self.vu0.get_vf(ft_idx);
            let acc = self.vu0.acc;

            let bc_component = |funct: u32| -> usize { 3 - (funct & 0x3) as usize };

            match funct {
                0x00..=0x03 => {
                    let bc = bc_component(funct);
                    log.push_str(&format!("VADDbc vf{}, vf{}, vf{}[{}]", fd_idx, fs_idx, ft_idx, bc));
                    let mut r = [0f32; 4];
                    for i in 0..4 { r[i] = fs[i] + ft[bc]; }
                    self.vu0.set_vf_masked(fd_idx, r, dest_mask);
                },
                0x04..=0x07 => {
                    let bc = bc_component(funct);
                    log.push_str(&format!("VSUBbc vf{}, vf{}, vf{}[{}]", fd_idx, fs_idx, ft_idx, bc));
                    let mut r = [0f32; 4];
                    for i in 0..4 { r[i] = fs[i] - ft[bc]; }
                    self.vu0.set_vf_masked(fd_idx, r, dest_mask);
                },
                0x08..=0x0B => {
                    let bc = bc_component(funct);
                    log.push_str(&format!("VMADDbc vf{}, vf{}, vf{}[{}]", fd_idx, fs_idx, ft_idx, bc));
                    let mut r = [0f32; 4];
                    for i in 0..4 { r[i] = acc[i] + fs[i] * ft[bc]; }
                    self.vu0.set_vf_masked(fd_idx, r, dest_mask);
                },
                0x0C..=0x0F => {
                    let bc = bc_component(funct);
                    log.push_str(&format!("VMSUBbc vf{}, vf{}, vf{}[{}]", fd_idx, fs_idx, ft_idx, bc));
                    let mut r = [0f32; 4];
                    for i in 0..4 { r[i] = acc[i] - fs[i] * ft[bc]; }
                    self.vu0.set_vf_masked(fd_idx, r, dest_mask);
                },
                0x10..=0x13 => {
                    let bc = bc_component(funct);
                    log.push_str(&format!("VMAXbc vf{}, vf{}, vf{}[{}]", fd_idx, fs_idx, ft_idx, bc));
                    let mut r = [0f32; 4];
                    for i in 0..4 { r[i] = fs[i].max(ft[bc]); }
                    self.vu0.set_vf_masked(fd_idx, r, dest_mask);
                },
                0x14..=0x17 => {
                    let bc = bc_component(funct);
                    log.push_str(&format!("VMINIbc vf{}, vf{}, vf{}[{}]", fd_idx, fs_idx, ft_idx, bc));
                    let mut r = [0f32; 4];
                    for i in 0..4 { r[i] = fs[i].min(ft[bc]); }
                    self.vu0.set_vf_masked(fd_idx, r, dest_mask);
                },
                0x18..=0x1B => {
                    let bc = bc_component(funct);
                    log.push_str(&format!("VMULbc vf{}, vf{}, vf{}[{}]", fd_idx, fs_idx, ft_idx, bc));
                    let mut r = [0f32; 4];
                    for i in 0..4 { r[i] = fs[i] * ft[bc]; }
                    self.vu0.set_vf_masked(fd_idx, r, dest_mask);
                },
                0x20..=0x23 => {
                    let bc = bc_component(funct);
                    log.push_str(&format!("VMULAbc ACC, vf{}, vf{}[{}]", fs_idx, ft_idx, bc));
                    let mut r = [0f32; 4];
                    for i in 0..4 { r[i] = fs[i] * ft[bc]; }
                    self.vu0.acc = r;
                },
                0x24..=0x27 => {
                    let bc = bc_component(funct);
                    log.push_str(&format!("VMADDAbc ACC, vf{}, vf{}[{}]", fs_idx, ft_idx, bc));
                    let mut r = [0f32; 4];
                    for i in 0..4 { r[i] = acc[i] + fs[i] * ft[bc]; }
                    self.vu0.acc = r;
                },
                0x28 => {
                    log.push_str(&format!("VADDA ACC, vf{}, vf{}", fs_idx, ft_idx));
                    let mut r = [0f32; 4];
                    for i in 0..4 { r[i] = fs[i] + ft[i]; }
                    self.vu0.acc = r;
                },
                0x29 => {
                    log.push_str(&format!("VMADDA ACC, vf{}, vf{}", fs_idx, ft_idx));
                    let mut r = [0f32; 4];
                    for i in 0..4 { r[i] = acc[i] + fs[i] * ft[i]; }
                    self.vu0.acc = r;
                },
                0x2A => {
                    log.push_str(&format!("VMULA ACC, vf{}, vf{}", fs_idx, ft_idx));
                    let mut r = [0f32; 4];
                    for i in 0..4 { r[i] = fs[i] * ft[i]; }
                    self.vu0.acc = r;
                },
                0x2C => {
                    log.push_str(&format!("VSUBA ACC, vf{}, vf{}", fs_idx, ft_idx));
                    let mut r = [0f32; 4];
                    for i in 0..4 { r[i] = fs[i] - ft[i]; }
                    self.vu0.acc = r;
                },
                0x2D => {
                    log.push_str(&format!("VMSUBA ACC, vf{}, vf{}", fs_idx, ft_idx));
                    let mut r = [0f32; 4];
                    for i in 0..4 { r[i] = acc[i] - fs[i] * ft[i]; }
                    self.vu0.acc = r;
                },
                0x2E => {
                    // VOPMULA: outer product multiply into ACC.xyz (used for cross products)
                    log.push_str(&format!("VOPMULA ACC, vf{}, vf{}", fs_idx, ft_idx));
                    self.vu0.acc[0] = fs[1] * ft[2];
                    self.vu0.acc[1] = fs[2] * ft[0];
                    self.vu0.acc[2] = fs[0] * ft[1];
                },
                0x1F | 0x3F => {
                    // VCLIP: test fs against +/- ft.w
                    let w = ft[3].abs();
                    let inside = fs[0].abs() <= w && fs[1].abs() <= w && fs[2].abs() <= w;
                    self.vu0.clip_flag = inside;
                    log.push_str(&format!("VCLIP vf{}, vf{}", fs_idx, ft_idx));
                },
                0x3C => {
                    let bc = bc_component(funct);
                    log.push_str(&format!("VDIV Q, vf{}[0], vf{}[{}]", fs_idx, ft_idx, bc));
                    if ft[bc] != 0.0 {
                        self.vu0.q = fs[0] / ft[bc];
                    }
                },
                0x3D => {
                    let bc = bc_component(funct);
                    log.push_str(&format!("VSQRT Q, vf{}[{}]", ft_idx, bc));
                    self.vu0.q = ft[bc].abs().sqrt();
                },
                0x3E => {
                    log.push_str("WAITQ");
                },
                _ => {
                    log.push_str(&format!("UNKNOWN COP2 FMAC funct {:#08b}", funct));
                }
            }
            return;
        }

        match rs {
            0x01 => {
                // QMFC2 rt, id - full 128-bit VF register to GPR
                log.push_str(&format!("QMFC2 $t{}, vf{}", rt, id));
                let v = self.vu0.get_vf(id as usize);
                let mut bytes = [0u8; 16];
                for c in 0..4 {
                    bytes[c * 4..c * 4 + 4].copy_from_slice(&v[c].to_le_bytes());
                }
                self.set_reg128(rt, u128::from_le_bytes(bytes));
            },
            0x02 => {
                // CFC2 rt, id - VI register to GPR (only vi0-15 are modeled)
                log.push_str(&format!("CFC2 $t{}, vi{}", rt, id));
                let val = if (id as usize) < 16 { self.vu0.get_vi(id as usize) as u64 } else { 0 };
                self.set_reg(rt, val);
            },
            0x05 => {
                // QMTC2 rt, id - GPR to full 128-bit VF register
                log.push_str(&format!("QMTC2 $t{}, vf{}", rt, id));
                let bytes = self.get_reg128(rt).to_le_bytes();
                let mut v = [0f32; 4];
                for c in 0..4 {
                    let mut b = [0u8; 4];
                    b.copy_from_slice(&bytes[c * 4..c * 4 + 4]);
                    v[c] = f32::from_le_bytes(b);
                }
                self.vu0.set_vf_masked(id as usize, v, 0xF);
            },
            0x06 => {
                // CTC2 rt, id - GPR to VI register
                log.push_str(&format!("CTC2 $t{}, vi{}", rt, id));
                if (id as usize) < 16 {
                    self.vu0.set_vi(id as usize, self.get_reg(rt) as u16);
                }
            },
            0x08 => {
                // BC2F / BC2T / BC2FL / BC2TL, on the VU0 clip flag
                let offset = instr.imm_sign_extended();
                let likely = (rt & 0b10) != 0;
                let want_true = (rt & 0b01) != 0;
                let taken = self.vu0.clip_flag == want_true;
                log.push_str(&format!("BC2{}{} {:#06X}", if want_true { "T" } else { "F" }, if likely { "L" } else { "" }, offset));

                if taken {
                    let offset_addr = (offset as u32) << 2;
                    self.next_pc = self.pc.wrapping_add(offset_addr);
                    self.branch = true;
                } else if likely {
                    self.nullify = true;
                }
            },
            0x00 | 0x10 => {
                // Integer VI operations
                let funct = instr.funct();
                let fs_vi = instr.rd() as usize;
                let ft_vi = instr.rt() as usize;
                let fd_vi = instr.shamt() as usize;
                match funct {
                    0x30 => {
                        // VIADD fd, fs, ft
                        log.push_str(&format!("VIADD vi{}, vi{}, vi{}", fd_vi, fs_vi, ft_vi));
                        let val = self.vu0.get_vi(fs_vi).wrapping_add(self.vu0.get_vi(ft_vi));
                        self.vu0.set_vi(fd_vi, val);
                    },
                    0x31 => {
                        // VISUB fd, fs, ft
                        log.push_str(&format!("VISUB vi{}, vi{}, vi{}", fd_vi, fs_vi, ft_vi));
                        let val = self.vu0.get_vi(fs_vi).wrapping_sub(self.vu0.get_vi(ft_vi));
                        self.vu0.set_vi(fd_vi, val);
                    },
                    0x32 => {
                        // VIADDI rt, rs, imm5
                        let imm = instr.shamt() as u16;
                        log.push_str(&format!("VIADDI vi{}, vi{}, {}", ft_vi, fs_vi, imm));
                        let val = self.vu0.get_vi(fs_vi).wrapping_add(imm);
                        self.vu0.set_vi(ft_vi, val);
                    },
                    0x34 => {
                        // VIAND fd, fs, ft
                        log.push_str(&format!("VIAND vi{}, vi{}, vi{}", fd_vi, fs_vi, ft_vi));
                        let val = self.vu0.get_vi(fs_vi) & self.vu0.get_vi(ft_vi);
                        self.vu0.set_vi(fd_vi, val);
                    },
                    0x35 => {
                        // VIOR fd, fs, ft
                        log.push_str(&format!("VIOR vi{}, vi{}, vi{}", fd_vi, fs_vi, ft_vi));
                        let val = self.vu0.get_vi(fs_vi) | self.vu0.get_vi(ft_vi);
                        self.vu0.set_vi(fd_vi, val);
                    },
                    _ => {
                        log.push_str(&format!("UNKNOWN COP2 VI funct {:#08b}", funct));
                    }
                }
            },
            _ => {
                log.push_str(&format!("UNKNOWN COP2: {:#07b}", rs));
            }
        }
    }

    /// Executes an MMI (MultiMedia Instructions / SIMD) instruction. See
    /// `cpu::mmi` for the scope and confidence notes on which opcodes are
    /// implemented.
    pub fn execute_mmi(&mut self, instr: &Instruction, log: &mut String) {
        use crate::cpu::mmi;

        let rs_idx = instr.rs();
        let rt_idx = instr.rt();
        let rd_idx = instr.rd();
        let shamt = instr.shamt();
        let funct = instr.funct();

        let rs = self.get_reg128(rs_idx);
        let rt = self.get_reg128(rt_idx);

        macro_rules! bin_op {
            ($name:expr, $f:expr) => {{
                log.push_str(&format!("{} $v{}, $v{}, $v{}", $name, rd_idx, rs_idx, rt_idx));
                let result = $f(rs, rt);
                self.set_reg128(rd_idx, result);
            }};
        }
        macro_rules! un_op {
            ($name:expr, $f:expr) => {{
                log.push_str(&format!("{} $v{}, $v{}", $name, rd_idx, rs_idx));
                let result = $f(rs);
                self.set_reg128(rd_idx, result);
            }};
        }

        match funct {
            0x04 => un_op!("PLZCW", mmi::plzcw),
            0x2B | 0x3B => {
                // QFSRV rd, rs, rt - Quadword Funnel Shift Right Variable using sa
                log.push_str(&format!("QFSRV $v{}, $v{}, $v{}", rd_idx, rs_idx, rt_idx));
                let result = mmi::qfsrv(rs, rt, self.sa);
                self.set_reg128(rd_idx, result);
            },
            0x08 => {
                // MMI0
                match shamt {
                    0 => bin_op!("PADDW", mmi::paddw),
                    1 => bin_op!("PSUBW", mmi::psubw),
                    2 => bin_op!("PCGTW", mmi::pcgtw),
                    3 => bin_op!("PMAXW", mmi::pmaxw),
                    4 => bin_op!("PADDH", mmi::paddh),
                    5 => bin_op!("PSUBH", mmi::psubh),
                    6 => bin_op!("PCGTH", mmi::pcgth),
                    7 => bin_op!("PMAXH", mmi::pmaxh),
                    8 => bin_op!("PADDB", mmi::paddb),
                    9 => bin_op!("PSUBB", mmi::psubb),
                    10 => bin_op!("PCGTB", mmi::pcgtb),
                    16 => bin_op!("PADDSW", mmi::paddsw),
                    17 => bin_op!("PSUBSW", mmi::psubsw),
                    18 => bin_op!("PEXTLW", mmi::pextlw),
                    19 => bin_op!("PPACW", mmi::ppacw),
                    20 => bin_op!("PADDSH", mmi::paddsh),
                    21 => bin_op!("PSUBSH", mmi::psubsh),
                    22 => bin_op!("PEXTLH", mmi::pextlh),
                    23 => bin_op!("PPACH", mmi::ppach),
                    24 => bin_op!("PADDSB", mmi::paddsb),
                    25 => bin_op!("PSUBSB", mmi::psubsb),
                    26 => bin_op!("PEXTLB", mmi::pextlb),
                    27 => bin_op!("PPACB", mmi::ppacb),
                    _ => log.push_str(&format!("UNKNOWN MMI0: shamt={:#07b}", shamt)),
                }
            },
            0x09 => {
                // MMI2
                match shamt {
                    14 => bin_op!("PCPYLD", mmi::pcpyld),
                    18 => bin_op!("PAND", mmi::pand),
                    19 => bin_op!("PXOR", mmi::pxor),
                    _ => log.push_str(&format!("UNKNOWN MMI2: shamt={:#07b}", shamt)),
                }
            },
            0x28 => {
                // MMI1
                match shamt {
                    1 => un_op!("PABSW", mmi::pabsw),
                    2 => bin_op!("PCEQW", mmi::pceqw),
                    3 => bin_op!("PMINW", mmi::pminw),
                    5 => un_op!("PABSH", mmi::pabsh),
                    6 => bin_op!("PCEQH", mmi::pceqh),
                    7 => bin_op!("PMINH", mmi::pminh),
                    10 => bin_op!("PCEQB", mmi::pceqb),
                    16 => bin_op!("PADDUW", mmi::padduw),
                    17 => bin_op!("PSUBUW", mmi::psubuw),
                    18 => bin_op!("PEXTUW", mmi::pextuw),
                    20 => bin_op!("PADDUH", mmi::padduh),
                    21 => bin_op!("PSUBUH", mmi::psubuh),
                    22 => bin_op!("PEXTUH", mmi::pextuh),
                    24 => bin_op!("PADDUB", mmi::paddub),
                    25 => bin_op!("PSUBUB", mmi::psubub),
                    26 => bin_op!("PEXTUB", mmi::pextub),
                    _ => log.push_str(&format!("UNKNOWN MMI1: shamt={:#07b}", shamt)),
                }
            },
            0x29 => {
                // MMI3
                match shamt {
                    14 => bin_op!("PCPYUD", mmi::pcpyud),
                    18 => bin_op!("POR", mmi::por),
                    19 => bin_op!("PNOR", mmi::pnor),
                    27 => {
                        log.push_str(&format!("PCPYH $v{}, $v{}", rd_idx, rt_idx));
                        let result = mmi::pcpyh(rt);
                        self.set_reg128(rd_idx, result);
                    },
                    _ => log.push_str(&format!("UNKNOWN MMI3: shamt={:#07b}", shamt)),
                }
            },
            0x00 => {
                // MADD rs, rt - HI:LO += (rs * rt), signed 32-bit inputs
                log.push_str(&format!("MADD $t{}, $t{}", rs_idx, rt_idx));
                let a = self.get_reg(rs_idx) as i32 as i64;
                let b = self.get_reg(rt_idx) as i32 as i64;
                let hilo = ((self.hi << 32) | (self.lo & 0xFFFFFFFF)) as i64;
                let result = hilo.wrapping_add(a.wrapping_mul(b)) as u64;
                self.hi = result >> 32;
                self.lo = result & 0xFFFFFFFF;
            },
            0x01 => {
                // MADDU rs, rt - unsigned variant
                log.push_str(&format!("MADDU $t{}, $t{}", rs_idx, rt_idx));
                let a = (self.get_reg(rs_idx) & 0xFFFFFFFF) as u64;
                let b = (self.get_reg(rt_idx) & 0xFFFFFFFF) as u64;
                let hilo = (self.hi << 32) | (self.lo & 0xFFFFFFFF);
                let result = hilo.wrapping_add(a.wrapping_mul(b));
                self.hi = result >> 32;
                self.lo = result & 0xFFFFFFFF;
            },
            0x10 => {
                // MFHI1 rd
                log.push_str(&format!("MFHI1 $t{}", rd_idx));
                self.set_reg(rd_idx, self.hi1);
            },
            0x11 => {
                // MTHI1 rs
                log.push_str(&format!("MTHI1 $t{}", rs_idx));
                self.hi1 = self.get_reg(rs_idx);
            },
            0x12 => {
                // MFLO1 rd
                log.push_str(&format!("MFLO1 $t{}", rd_idx));
                self.set_reg(rd_idx, self.lo1);
            },
            0x13 => {
                // MTLO1 rs
                log.push_str(&format!("MTLO1 $t{}", rs_idx));
                self.lo1 = self.get_reg(rs_idx);
            },
            0x18 => {
                // MULT1 rs, rt - signed 32x32->64 multiply into HI1:LO1
                log.push_str(&format!("MULT1 $t{}, $t{}", rs_idx, rt_idx));
                let a = self.get_reg(rs_idx) as i32 as i64;
                let b = self.get_reg(rt_idx) as i32 as i64;
                let res = (a * b) as u64;
                self.hi1 = res >> 32;
                self.lo1 = res & 0xFFFFFFFF;
            },
            0x19 => {
                // MULTU1 rs, rt - unsigned variant
                log.push_str(&format!("MULTU1 $t{}, $t{}", rs_idx, rt_idx));
                let a = (self.get_reg(rs_idx) & 0xFFFFFFFF) as u64;
                let b = (self.get_reg(rt_idx) & 0xFFFFFFFF) as u64;
                let res = a * b;
                self.hi1 = res >> 32;
                self.lo1 = res & 0xFFFFFFFF;
            },
            0x1A => {
                // DIV1 rs, rt
                log.push_str(&format!("DIV1 $t{}, $t{}", rs_idx, rt_idx));
                let a = self.get_reg(rs_idx) as i32;
                let b = self.get_reg(rt_idx) as i32;
                if b != 0 {
                    if a == i32::MIN && b == -1 {
                        self.lo1 = a as u64;
                        self.hi1 = 0;
                    } else {
                        self.lo1 = (a / b) as i64 as u64;
                        self.hi1 = (a % b) as i64 as u64;
                    }
                }
            },
            0x1B => {
                // DIVU1 rs, rt
                log.push_str(&format!("DIVU1 $t{}, $t{}", rs_idx, rt_idx));
                let a = (self.get_reg(rs_idx) & 0xFFFFFFFF) as u32;
                let b = (self.get_reg(rt_idx) & 0xFFFFFFFF) as u32;
                if b != 0 {
                    self.lo1 = (a / b) as i32 as i64 as u64;
                    self.hi1 = (a % b) as i32 as i64 as u64;
                }
            },
            0x20 => {
                // MADD1 rs, rt - HI1:LO1 += (rs * rt), signed
                log.push_str(&format!("MADD1 $t{}, $t{}", rs_idx, rt_idx));
                let a = self.get_reg(rs_idx) as i32 as i64;
                let b = self.get_reg(rt_idx) as i32 as i64;
                let hilo = ((self.hi1 << 32) | (self.lo1 & 0xFFFFFFFF)) as i64;
                let result = hilo.wrapping_add(a.wrapping_mul(b)) as u64;
                self.hi1 = result >> 32;
                self.lo1 = result & 0xFFFFFFFF;
            },
            0x21 => {
                // MADDU1 rs, rt - unsigned variant
                log.push_str(&format!("MADDU1 $t{}, $t{}", rs_idx, rt_idx));
                let a = (self.get_reg(rs_idx) & 0xFFFFFFFF) as u64;
                let b = (self.get_reg(rt_idx) & 0xFFFFFFFF) as u64;
                let hilo = (self.hi1 << 32) | (self.lo1 & 0xFFFFFFFF);
                let result = hilo.wrapping_add(a.wrapping_mul(b));
                self.hi1 = result >> 32;
                self.lo1 = result & 0xFFFFFFFF;
            },
            _ => {
                log.push_str(&format!("UNKNOWN MMI funct: {:#08b}", funct));
            }
        }
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

    #[test]
    fn test_fpu_add_s() {
        let mut ee = EmotionEngine::new(crate::memory::bus::Bus::new());
        ee.fpr[1] = 2.5;
        ee.fpr[2] = 4.5;

        // ADD.S $f3, $f1, $f2 (COP1, fmt=S, ft=2, fs=1, fd=3, funct=0)
        let op = (0x11u32 << 26) | (0x10 << 21) | (2 << 16) | (1 << 11) | (3 << 6);
        ee.bus.write32(0x00100000, op);
        ee.set_pc(0x00100000);

        let log = ee.step();
        assert!(log.contains("ADD.S"));
        assert_eq!(ee.fpr[3], 7.0);
    }

    #[test]
    fn test_beql_not_taken_squashes_delay_slot() {
        let mut ee = EmotionEngine::new(crate::memory::bus::Bus::new());
        ee.set_reg(9, 1); // $t1 = 1, so BEQL $zero, $t1 is NOT taken

        // BEQL $zero, $t1, 4
        let beql = (0x14u32 << 26) | (0 << 21) | (9 << 16) | 4;
        ee.bus.write32(0x00100000, beql);
        // Delay slot: ADDIU $t2, $zero, 5 (should be squashed, never executed)
        let addiu = (0x09u32 << 26) | (0 << 21) | (10 << 16) | 5;
        ee.bus.write32(0x00100004, addiu);

        ee.set_pc(0x00100000);
        let log1 = ee.step();
        assert!(log1.contains("BEQL"));
        assert!(ee.nullify);

        let log2 = ee.step();
        assert!(log2.contains("squashed"));
        assert_eq!(ee.get_reg(10), 0, "delay slot instruction must not have executed");
        assert_eq!(ee.pc, 0x00100008, "execution should fall through, not branch");
    }

    #[test]
    fn test_lwl_unaligned() {
        let mut ee = EmotionEngine::new(crate::memory::bus::Bus::new());
        // Memory word at 0x5000 = 0x44332211 (bytes: 11 22 33 44)
        ee.bus.write32(0x5000, 0x44332211);
        ee.set_reg(8, 0x5000); // $t0 = base address

        // LWL $t1, 1($t0) -> addr 0x5001, byte-offset 1 within the word
        let op = (0x22u32 << 26) | (8 << 21) | (9 << 16) | 1;
        ee.bus.write32(0x00100000, op);
        ee.set_pc(0x00100000);

        ee.step();
        // Bottom 2 bytes preserved from rt (0), top 2 bytes come from mem bytes 0,1 (0x11, 0x22)
        assert_eq!(ee.get_reg(9), 0x22110000);
    }

    #[test]
    fn test_lwr_swr_roundtrip_aligned() {
        let mut ee = EmotionEngine::new(crate::memory::bus::Bus::new());
        ee.bus.write32(0x6000, 0x12345678);
        ee.set_reg(8, 0x6000); // $t0 = base

        // LWR $t1, 0($t0) then LWL $t1, 3($t0) reconstructs the full word (classic LE idiom)
        let lwr = (0x26u32 << 26) | (8 << 21) | (9 << 16) | 0;
        let lwl = (0x22u32 << 26) | (8 << 21) | (9 << 16) | 3;
        ee.bus.write32(0x00100000, lwr);
        ee.bus.write32(0x00100004, lwl);
        ee.set_pc(0x00100000);
        ee.step();
        ee.step();
        assert_eq!(ee.get_reg(9), 0x12345678);
    }

    #[test]
    fn test_daddiu_lwu_pref() {
        let mut ee = EmotionEngine::new(crate::memory::bus::Bus::new());
        ee.set_reg(8, 0xFFFFFFFF00000000); // $t0
        ee.bus.write32(0x5000, 0x80000000); // top-bit-set word, to prove LWU zero-extends

        // DADDIU $t1, $t0, 1
        let daddiu = (0x19u32 << 26) | (8 << 21) | (9 << 16) | 1;
        // LWU $t2, 0($t3) where $t3 = 0x5000
        let lwu = (0x27u32 << 26) | (11 << 21) | (10 << 16) | 0;
        // PREF (should just no-op, not except)
        let pref = 0x33u32 << 26;
        ee.bus.write32(0x00100000, daddiu);
        ee.bus.write32(0x00100004, lwu);
        ee.bus.write32(0x00100008, pref);
        ee.set_reg(11, 0x5000); // $t3
        ee.set_pc(0x00100000);

        let log1 = ee.step();
        assert!(log1.contains("DADDIU"));
        assert_eq!(ee.get_reg(9), 0xFFFFFFFF00000001);

        let log2 = ee.step();
        assert!(log2.contains("LWU"));
        assert_eq!(ee.get_reg(10), 0x0000000080000000, "LWU must zero-extend, not sign-extend");

        let log3 = ee.step();
        assert!(log3.contains("PREF"));
        assert_eq!(ee.pc, 0x0010000C, "PREF must not raise an exception or branch");
    }

    #[test]
    fn test_bgezal_sets_return_address() {
        let mut ee = EmotionEngine::new(crate::memory::bus::Bus::new());
        ee.set_reg(8, 5); // $t0 >= 0

        // BGEZAL $t0, 4
        let op = (0x01u32 << 26) | (8 << 21) | (0b10001 << 16) | 4;
        ee.bus.write32(0x00100000, op);
        ee.set_pc(0x00100000);
        ee.step();

        assert_eq!(ee.get_reg(31), 0x00100008, "$ra should point past the delay slot");
        assert!(ee.branch);
    }

    #[test]
    fn test_mtsah_sets_sa_register() {
        let mut ee = EmotionEngine::new(crate::memory::bus::Bus::new());
        ee.set_reg(8, 3);

        // MTSAH $t0, 1 -> sa = ((3+1) & 0x7) * 2 = 8
        let op = (0x01u32 << 26) | (8 << 21) | (0b11001 << 16) | 1;
        ee.bus.write32(0x00100000, op);
        ee.set_pc(0x00100000);
        ee.step();

        assert_eq!(ee.sa, 8);
    }

    #[test]
    fn test_sp_never_allowed_to_become_zero() {
        let mut ee = EmotionEngine::new(crate::memory::bus::Bus::new());

        // A 64-bit write of exactly 0 to $sp (e.g. via ADDIU) must be substituted.
        ee.set_reg(29, 0);
        assert_ne!(ee.get_reg(29), 0);
        assert_eq!(ee.get_reg(29), 0x81FFFFF0);

        // A 128-bit write of exactly 0 to $sp (e.g. via the MMI PADDUW register-clear idiom) must too.
        ee.set_reg128(29, 0xDEAD); // first set to something else, to prove the next write is what's guarded
        ee.set_reg128(29, 0);
        assert_eq!(ee.get_reg128(29), 0x81FFFFF0);

        // Other registers are unaffected - zero is a perfectly normal value for them.
        ee.set_reg(8, 0);
        assert_eq!(ee.get_reg(8), 0);
    }

    #[test]
    fn test_consecutive_unmapped_fetches_tracks_and_resets() {
        let mut ee = EmotionEngine::new(crate::memory::bus::Bus::new());
        // PC pointed well outside RAM/BIOS/HW ranges: every fetch reads back 0 (NOP) from unmapped space.
        ee.set_pc(0x08000000);

        for i in 1..=5u32 {
            ee.step();
            assert_eq!(ee.consecutive_unmapped_fetches, i);
        }

        // A real instruction fetched from mapped RAM must reset the counter.
        let nop = 0u32;
        ee.bus.write32(0x00100000, nop);
        ee.set_pc(0x00100000);
        ee.step();
        assert_eq!(ee.consecutive_unmapped_fetches, 0);
    }

    #[test]
    fn test_sif1_bind_hle_unblocks_client_struct() {
        let mut ee = EmotionEngine::new(crate::memory::bus::Bus::new());

        // Client struct at 0x8000; verify `server` field (offset 36) starts NULL.
        let cd_addr = 0x8000u32;
        ee.bus.write32(cd_addr + 36, 0);

        // Build a SIF_CMD_RPC_BIND packet (SifCmdHeader + SifRpcBindPkt_t fields) in RAM.
        let packet_addr = 0x3000u32;
        ee.bus.write32(packet_addr + 0, 36);           // psize:dsize bitfield (value doesn't matter here)
        ee.bus.write32(packet_addr + 4, 0);             // dest
        ee.bus.write32(packet_addr + 8, 0x80000009);    // cid = SIF_CMD_RPC_BIND
        ee.bus.write32(packet_addr + 12, 0);            // opt
        ee.bus.write32(packet_addr + 16, 0);            // rec_id
        ee.bus.write32(packet_addr + 20, 0);            // pkt_addr
        ee.bus.write32(packet_addr + 24, 0);            // rpc_id
        ee.bus.write32(packet_addr + 28, cd_addr);      // cd -> client struct
        ee.bus.write32(packet_addr + 32, 0x12345678);   // sid

        // Kick DMAC channel SIF1 (index 6), normal mode, 3 quadwords (36 bytes rounds up to 3*16=48, use exact quadwords)
        ee.bus.write32(0x1000C410, packet_addr); // D6_MADR (SIF1 base 0x1000C400 + 0x10)
        ee.bus.write32(0x1000C420, 3);           // D6_QWC (3 quadwords = 48 bytes, covers the 36-byte packet)
        ee.bus.write32(0x1000C400, 0x100);       // D6_CHCR: STR set, normal mode

        assert_eq!(ee.bus.read32(cd_addr + 36), 0xBAD00001, "server field should be unblocked (non-null)");
        assert_eq!(ee.bus.read32(cd_addr + 16), 0, "command field should be reset to 0");
    }

    #[test]
    fn test_semaphore_syscalls_never_block() {
        let mut ee = EmotionEngine::new(crate::memory::bus::Bus::new());

        // SYSCALL with $v1 = 0x40 (CreateSema)
        let syscall_op = 12u32;
        ee.bus.write32(0x00100000, syscall_op);
        ee.set_reg(3, 0x40);
        ee.set_pc(0x00100000);
        let log = ee.step();
        assert!(log.contains("CreateSema"));
        let sema_id = ee.get_reg(2);
        assert!(sema_id > 0);

        // WaitSema on that ID must return immediately (no real blocking possible in this interpreter)
        ee.bus.write32(0x00100004, syscall_op);
        ee.set_reg(3, 0x44);
        ee.set_reg(4, sema_id);
        ee.step();
        assert_eq!(ee.pc, 0x00100008, "WaitSema must not block or branch away");
    }

    #[test]
    fn test_fpu_max_s_and_madda_s() {
        let mut ee = EmotionEngine::new(crate::memory::bus::Bus::new());
        ee.fpr[1] = 2.0;
        ee.fpr[2] = 5.0;

        // MAX.S $f3, $f1, $f2 (fmt=S=0x10, ft=2, fs=1, fd=3, funct=0x18)
        let max_s = (0x11u32 << 26) | (0x10 << 21) | (2 << 16) | (1 << 11) | (3 << 6) | 0x18;
        ee.bus.write32(0x00100000, max_s);
        // MADDA.S $f1, $f2 (fs=1, ft=2, funct=0x10) - ACC += fs*ft
        let madda_s = (0x11u32 << 26) | (0x10 << 21) | (2 << 16) | (1 << 11) | 0x10;
        ee.bus.write32(0x00100004, madda_s);
        ee.set_pc(0x00100000);

        let log1 = ee.step();
        assert!(log1.contains("MAX.S"));
        assert_eq!(ee.fpr[3], 5.0);

        ee.facc = 1.0;
        let log2 = ee.step();
        assert!(log2.contains("MADDA.S"));
        assert_eq!(ee.facc, 11.0); // 1.0 + 2.0*5.0
    }

    #[test]
    fn test_mult1_mfhi1_mflo1() {
        let mut ee = EmotionEngine::new(crate::memory::bus::Bus::new());
        ee.set_reg(8, 100000);
        ee.set_reg(9, 100000);

        // MULT1 $t8, $t9: opcode SPECIAL2(0x1C), rs=8, rt=9, funct=0x18
        let mult1 = (0x1Cu32 << 26) | (8 << 21) | (9 << 16) | 0x18;
        // MFLO1 $t10: rd=10, funct=0x12
        let mflo1 = (0x1Cu32 << 26) | (10 << 11) | 0x12;
        // MFHI1 $t11: rd=11, funct=0x10
        let mfhi1 = (0x1Cu32 << 26) | (11 << 11) | 0x10;
        ee.bus.write32(0x00100000, mult1);
        ee.bus.write32(0x00100004, mflo1);
        ee.bus.write32(0x00100008, mfhi1);
        ee.set_pc(0x00100000);

        ee.step();
        ee.step();
        ee.step();

        let expected = 100000i64 * 100000i64; // 10,000,000,000 - exceeds 32 bits, exercises HI1
        assert_eq!(ee.get_reg(10), (expected as u64) & 0xFFFFFFFF);
        assert_eq!(ee.get_reg(11), ((expected as u64) >> 32) & 0xFFFFFFFF);
        // regular (pipeline 0) HI/LO must be untouched by MULT1
        assert_eq!(ee.hi, 0);
        assert_eq!(ee.lo, 0);
    }

    #[test]
    fn test_madd_accumulates_into_hilo() {
        let mut ee = EmotionEngine::new(crate::memory::bus::Bus::new());
        ee.hi = 0;
        ee.lo = 5;
        ee.set_reg(8, 3);
        ee.set_reg(9, 4);

        // MADD $t8, $t9: funct=0x00
        let madd = (0x1Cu32 << 26) | (8 << 21) | (9 << 16);
        ee.bus.write32(0x00100000, madd);
        ee.set_pc(0x00100000);
        ee.step();

        assert_eq!(ee.lo, 17); // 5 + 3*4
        assert_eq!(ee.hi, 0);
    }

    #[test]
    fn test_mmi_padduw_via_decode() {
        let mut ee = EmotionEngine::new(crate::memory::bus::Bus::new());
        ee.set_reg128(8, u128::from_le_bytes({
            let mut b = [0u8; 16];
            b[0..4].copy_from_slice(&0xFFFFFFFFu32.to_le_bytes());
            b[4..8].copy_from_slice(&5u32.to_le_bytes());
            b
        }));
        ee.set_reg128(9, u128::from_le_bytes({
            let mut b = [0u8; 16];
            b[0..4].copy_from_slice(&1u32.to_le_bytes());
            b[4..8].copy_from_slice(&3u32.to_le_bytes());
            b
        }));

        // PADDUW $v10, $v8, $v9: opcode SPECIAL2(0x1C), rs=8, rt=9, rd=10, shamt=16, funct=0x28 (MMI1 dispatch)
        let op = (0x1Cu32 << 26) | (8 << 21) | (9 << 16) | (10 << 11) | (16 << 6) | 0x28;
        ee.bus.write32(0x00100000, op);
        ee.set_pc(0x00100000);
        let log = ee.step();

        assert!(log.contains("PADDUW"));
        let result = ee.get_reg128(10).to_le_bytes();
        assert_eq!(u32::from_le_bytes(result[0..4].try_into().unwrap()), 0xFFFFFFFF); // saturated
        assert_eq!(u32::from_le_bytes(result[4..8].try_into().unwrap()), 8);
    }

    #[test]
    fn test_ld_sd_gs_register() {
        let mut ee = EmotionEngine::new(crate::memory::bus::Bus::new());
        ee.set_reg(8, 0x12000000); // $t0 = GS PMODE register address
        ee.set_reg(9, 0x1122334455667788); // $t1 = value to store

        // SD $t1, 0($t0)
        let sd = (0x3Fu32 << 26) | (8 << 21) | (9 << 16) | 0;
        // LD $t2, 0($t0)
        let ld = (0x37u32 << 26) | (8 << 21) | (10 << 16) | 0;
        ee.bus.write32(0x00100000, sd);
        ee.bus.write32(0x00100004, ld);
        ee.set_pc(0x00100000);

        ee.step();
        assert_eq!(ee.bus.hw.gs.pmode, 0x1122334455667788);

        ee.step();
        assert_eq!(ee.get_reg(10), 0x1122334455667788);
    }

    #[test]
    fn test_dma_gif_plots_pixel() {
        let mut ee = EmotionEngine::new(crate::memory::bus::Bus::new());

        // Build a PACKED-mode GIF packet: 1 loop of 2 registers (A+D RGBAQ, then XYZ2)
        let nloop: u128 = 1;
        let nreg: u128 = 2;
        let regs: u128 = 0x0E | (0x05 << 4); // reg0 = A+D, reg1 = XYZ2
        let tag = nloop | (nreg << 60) | (regs << 64);

        // A+D targeting RGBAQ (reg 0x01), native layout R=0xFF,G=0x00,B=0x00,A=0x80
        let rgbaq_val: u128 = 0x800000FF;
        let ad_qword = rgbaq_val | (0x01u128 << 64);

        // XYZ2 packed: pixel (100, 50), 12.4 fixed point
        let x_fixed: u128 = 100 << 4;
        let y_fixed: u128 = 50 << 4;
        let xyz_qword = x_fixed | (y_fixed << 32);

        ee.bus.write128(0x2000, tag);
        ee.bus.write128(0x2010, ad_qword);
        ee.bus.write128(0x2020, xyz_qword);

        // Kick the GIF DMA channel: normal mode, 3 quadwords (tag + 2 registers) from 0x2000
        ee.bus.write32(0x1000A010, 0x2000); // D2_MADR
        ee.bus.write32(0x1000A020, 3);      // D2_QWC
        ee.bus.write32(0x1000A000, 0x100);  // D2_CHCR: STR bit set, normal mode

        assert_eq!(ee.bus.hw.gs.pixels_drawn, 1);
        assert_eq!(ee.bus.hw.gs.framebuffer[50 * crate::hw::gs::FB_WIDTH + 100], 0x80FF0000);
        assert_eq!(ee.bus.hw.dmac.channels[2].chcr & (1 << 8), 0, "STR should clear on completion");
        assert_ne!(ee.bus.hw.dmac.d_stat & (1 << 2), 0, "D_STAT channel bit should be set");
        assert_ne!(ee.bus.hw.intc_stat & (1 << 11), 0, "DMAC completion IRQ should fire");
    }

    #[test]
    fn test_dma_gif_triangle_rasterization() {
        let mut ee = EmotionEngine::new(crate::memory::bus::Bus::new());

        // One GIFtag per register write: PRIM(triangle), RGBAQ, then 3x XYZ2 vertices.
        let mk_tag = |reg: u128| -> u128 { 1 /* nloop */ | (1u128 << 60) /* nreg */ | (reg << 64) };
        let xyz = |x: i32, y: i32| -> u128 { ((x << 4) as u32 as u128) | (((y << 4) as u32 as u128) << 32) };

        let mut addr = 0x3000u32;
        let mut push = |ee: &mut EmotionEngine, qword: u128| {
            ee.bus.write128(addr, qword);
            addr += 16;
        };

        push(&mut ee, mk_tag(0x00)); // tag: PRIM register
        push(&mut ee, 0x03);         // PRIM = TRIANGLE
        push(&mut ee, mk_tag(0x01)); // tag: RGBAQ register
        // Packed RGBAQ: opaque red (R=0xFF, G=0x00, B=0x00, A=0xFF)
        push(&mut ee, 0xFFu128 | (0x00u128 << 32) | (0x00u128 << 64) | (0xFFu128 << 96));
        push(&mut ee, mk_tag(0x05)); // tag: XYZ2 vertex 1
        push(&mut ee, xyz(100, 100));
        push(&mut ee, mk_tag(0x05)); // tag: XYZ2 vertex 2
        push(&mut ee, xyz(300, 100));
        push(&mut ee, mk_tag(0x05)); // tag: XYZ2 vertex 3
        push(&mut ee, xyz(100, 300));

        let total_qwords = (addr - 0x3000) / 16;
        ee.bus.write32(0x1000A010, 0x3000);        // D2_MADR
        ee.bus.write32(0x1000A020, total_qwords);   // D2_QWC
        ee.bus.write32(0x1000A000, 0x100);          // D2_CHCR: STR set, normal mode

        // Point inside the right triangle (100,100)-(300,100)-(100,300)
        assert_ne!(ee.bus.hw.gs.framebuffer[150 * crate::hw::gs::FB_WIDTH + 150], 0);
        // Point clearly outside the triangle
        assert_eq!(ee.bus.hw.gs.framebuffer[10 * crate::hw::gs::FB_WIDTH + 10], 0);
    }

    #[test]
    fn test_ldl_unaligned() {
        let mut ee = EmotionEngine::new(crate::memory::bus::Bus::new());
        // Doubleword at 0x7000 = 0x8877665544332211 (bytes 11 22 33 44 55 66 77 88)
        ee.bus.write64(0x7000, 0x8877665544332211);
        ee.set_reg(8, 0x7000); // $t0 = base

        // LDL $t1, 1($t0) -> addr 0x7001, byte-offset 1 within the doubleword
        let op = (0x1Au32 << 26) | (8 << 21) | (9 << 16) | 1;
        ee.bus.write32(0x00100000, op);
        ee.set_pc(0x00100000);
        ee.step();

        // Bottom 6 bytes preserved from rt (0), top 2 bytes come from mem bytes 0,1 (0x11, 0x22)
        assert_eq!(ee.get_reg(9), 0x2211_0000_0000_0000);
    }

    #[test]
    fn test_ldr_sdr_roundtrip_aligned() {
        let mut ee = EmotionEngine::new(crate::memory::bus::Bus::new());
        ee.bus.write64(0x7100, 0x1122334455667788);
        ee.set_reg(8, 0x7100); // $t0 = base

        // LDR $t1, 0($t0) then LDL $t1, 7($t0) reconstructs the full doubleword (classic LE idiom)
        let ldr = (0x1Bu32 << 26) | (8 << 21) | (9 << 16) | 0;
        let ldl = (0x1Au32 << 26) | (8 << 21) | (9 << 16) | 7;
        ee.bus.write32(0x00100000, ldr);
        ee.bus.write32(0x00100004, ldl);
        ee.set_pc(0x00100000);
        ee.step();
        ee.step();
        assert_eq!(ee.get_reg(9), 0x1122334455667788);
    }

    #[test]
    fn test_qmtc2_qmfc2_roundtrip() {
        let mut ee = EmotionEngine::new(crate::memory::bus::Bus::new());
        ee.set_reg128(9, u128::from_le_bytes({
            let mut b = [0u8; 16];
            b[0..4].copy_from_slice(&1.0f32.to_le_bytes());
            b[4..8].copy_from_slice(&2.0f32.to_le_bytes());
            b[8..12].copy_from_slice(&3.0f32.to_le_bytes());
            b[12..16].copy_from_slice(&4.0f32.to_le_bytes());
            b
        }));

        // QMTC2 $t1, vf5 (rs=0x05, rt=9, rd/id=5)
        let qmtc2 = (0x12u32 << 26) | (0x05 << 21) | (9 << 16) | (5 << 11);
        // QMFC2 $t2, vf5 (rs=0x01, rt=10, rd/id=5)
        let qmfc2 = (0x12u32 << 26) | (0x01 << 21) | (10 << 16) | (5 << 11);
        ee.bus.write32(0x00100000, qmtc2);
        ee.bus.write32(0x00100004, qmfc2);
        ee.set_pc(0x00100000);

        ee.step();
        assert_eq!(ee.vu0.get_vf(5), [1.0, 2.0, 3.0, 4.0]);

        ee.step();
        let result = ee.get_reg128(10).to_le_bytes();
        assert_eq!(f32::from_le_bytes(result[0..4].try_into().unwrap()), 1.0);
        assert_eq!(f32::from_le_bytes(result[12..16].try_into().unwrap()), 4.0);
    }

    #[test]
    fn test_vu0_broadcast_matrix_row_transform() {
        let mut ee = EmotionEngine::new(crate::memory::bus::Bus::new());
        // vf1 = identity row 0 (1,0,0,0), vf10 = input vector (2,3,4,1)
        ee.vu0.vf[1] = [1.0, 0.0, 0.0, 0.0];
        ee.vu0.vf[10] = [2.0, 3.0, 4.0, 1.0];

        // VMULx.xyzw vf2, vf1, vf10  =>  vf2 = vf1 * broadcast(vf10.x=2.0)
        // rs = 0x10 (FMAC marker) | dest mask 0xF, rt=ft=10, rd=fs=1, shamt=fd=2, funct=0x1B (MULbc, bc=3=X)
        let vmulx = (0x12u32 << 26) | ((0x10 | 0xF) << 21) | (10 << 16) | (1 << 11) | (2 << 6) | 0x1B;
        ee.bus.write32(0x00100000, vmulx);
        ee.set_pc(0x00100000);
        ee.step();

        // vf1 * 2.0 broadcast = (2,0,0,0)
        assert_eq!(ee.vu0.get_vf(2), [2.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_qfsrv_decode_and_execution() {
        let mut ee = EmotionEngine::new(crate::memory::bus::Bus::new());
        // Set $sa = 4 (shift by 4 bytes)
        ee.sa = 4;
        ee.set_reg128(8, 0x11223344_55667788_99AABBCC_DDEEFF00u128); // $t0
        ee.set_reg128(9, 0x01020304_05060708_090A0B0C_0D0E0F10u128); // $t1

        // QFSRV $v10, $v8, $v9 (opcode 0x1C, rs=8, rt=9, rd=10, shamt=0, funct=0x2B)
        let qfsrv = (0x1Cu32 << 26) | (8 << 21) | (9 << 16) | (10 << 11) | 0x2B;
        ee.bus.write32(0x00100000, qfsrv);
        ee.set_pc(0x00100000);

        let log = ee.step();
        assert!(log.contains("QFSRV"));
        assert_ne!(ee.get_reg128(10), 0);
    }

    #[test]
    fn test_vu0_vclip_and_viadd() {
        let mut ee = EmotionEngine::new(crate::memory::bus::Bus::new());
        // Set vf1 = (0.5, 0.5, 0.5, 1.0) inside w, vf2 = (0, 0, 0, 1.0)
        ee.vu0.vf[1] = [0.5, 0.5, 0.5, 1.0];
        ee.vu0.vf[2] = [0.0, 0.0, 0.0, 1.0];

        // VCLIP vf1, vf2 (FMAC marker 0x10, rs=0x10, ft=2, fs=1, funct=0x1F)
        let vclip = (0x12u32 << 26) | (0x10 << 21) | (2 << 16) | (1 << 11) | 0x1F;
        ee.bus.write32(0x00100000, vclip);
        ee.set_pc(0x00100000);
        let log = ee.step();
        assert!(log.contains("VCLIP"));
        assert!(ee.vu0.clip_flag, "Point (0.5, 0.5, 0.5) should be inside +/- 1.0 w");

        // VIADD vi3, vi1, vi2: vi1=10, vi2=25 -> vi3=35
        ee.vu0.set_vi(1, 10);
        ee.vu0.set_vi(2, 25);
        // rs=0x00, fs=1 (rd), ft=2 (rt), fd=3 (shamt), funct=0x30
        let viadd = (0x12u32 << 26) | (0x00 << 21) | (2 << 16) | (1 << 11) | (3 << 6) | 0x30;
        ee.bus.write32(0x00100004, viadd);
        let log2 = ee.step();
        assert!(log2.contains("VIADD"));
        assert_eq!(ee.vu0.get_vi(3), 35);
    }

    #[test]
    fn test_intc_and_dmac_syscalls() {
        let mut ee = EmotionEngine::new(crate::memory::bus::Bus::new());

        // SYSCALL instruction
        let syscall_op = 12u32;
        ee.bus.write32(0x00100000, syscall_op);
        ee.bus.write32(0x00100004, syscall_op);
        ee.bus.write32(0x00100008, syscall_op);

        // 1. AddIntcHandler (v1 = 0x10)
        ee.set_reg(3, 0x10);
        ee.set_pc(0x00100000);
        ee.step();
        assert_eq!(ee.get_reg(2), 1); // returns handler id 1

        // 2. EnableIntc (v1 = 0x12)
        ee.set_reg(3, 0x12);
        ee.set_pc(0x00100004);
        ee.step();
        assert_eq!(ee.get_reg(2), 0);

        // 3. GetOsdConfigParam (v1 = 0x7C, a0 = 0x5000)
        ee.set_reg(3, 0x7C);
        ee.set_reg(4, 0x5000);
        ee.set_pc(0x00100008);
        ee.step();
        assert_eq!(ee.get_reg(2), 0);
        assert_eq!(ee.bus.read32(0x5000), 0);
    }

    #[test]
    fn test_gs_csr_field_toggle() {
        let mut ee = EmotionEngine::new(crate::memory::bus::Bus::new());
        ee.set_pc(0x00100000); // Point to RAM NOPs to avoid BIOS startup GS reset
        let initial_field = (ee.bus.hw.gs.csr >> 13) & 1;

        // Step enough cycles to trigger VBlank toggle
        for _ in 0..100_000 {
            ee.step();
        }

        let new_field = (ee.bus.hw.gs.csr >> 13) & 1;
        assert_ne!(initial_field, new_field, "GS_CSR FIELD bit should toggle on VBlank");
        assert_ne!(ee.bus.hw.gs.csr & (1 << 3), 0, "GS_CSR VSINT bit should be set on VBlank");
    }

    #[test]
    fn test_gs_tex0_and_alpha_packed_writes() {
        let mut ee = EmotionEngine::new(crate::memory::bus::Bus::new());

        // A+D write to TEX0_1 (0x06) and ALPHA_1 (0x42)
        let nloop: u128 = 1;
        let nreg: u128 = 2;
        let regs: u128 = 0x0E | (0x0E << 4);
        let tag = nloop | (nreg << 60) | (regs << 64);

        let tex0_val: u128 = 0x00100000_12345678u128 | (0x06u128 << 64);
        let alpha_val: u128 = 0x00000000_00000044u128 | (0x42u128 << 64);

        ee.bus.write128(0x4000, tag);
        ee.bus.write128(0x4010, tex0_val);
        ee.bus.write128(0x4020, alpha_val);

        // Kick GIF DMA
        ee.bus.write32(0x1000A010, 0x4000);
        ee.bus.write32(0x1000A020, 3);
        ee.bus.write32(0x1000A000, 0x100);

        assert_eq!(ee.bus.hw.gs.tex0_1, 0x00100000_12345678);
        assert_eq!(ee.bus.hw.gs.alpha_1, 0x44);
    }
}
