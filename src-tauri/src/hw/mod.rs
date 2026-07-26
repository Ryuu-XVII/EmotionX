pub struct Hardware {
    // Basic stubs for hardware registers
    // e.g., INTC (Interrupt Controller), DMAC (DMA Controller), Timers
    pub intc_stat: u32,
    pub intc_mask: u32,
}

impl Hardware {
    pub fn new() -> Self {
        Self {
            intc_stat: 0,
            intc_mask: 0,
        }
    }

    pub fn read32(&mut self, addr: u32) -> u32 {
        match addr {
            // INTC_STAT
            0x10000000 => self.intc_stat,
            // INTC_MASK
            0x10000010 => self.intc_mask,
            // Default HW read
            _ => {
                // If it's reading a status register, often returning 0 or 1 is enough to break a wait loop
                // For a stub, we can return 0.
                0
            }
        }
    }

    pub fn write32(&mut self, addr: u32, val: u32) {
        match addr {
            // INTC_STAT
            0x10000000 => {
                // Writing 1 to STAT clears the interrupt
                self.intc_stat &= !val;
            },
            // INTC_MASK
            0x10000010 => {
                self.intc_mask = val;
            },
            _ => {
                // Ignore other writes for now
            }
        }
    }
}
