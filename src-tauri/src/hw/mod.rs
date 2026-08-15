pub mod sio;
pub mod dmac;
pub mod gs;
pub mod timer;

pub struct Hardware {
    // INTC (Interrupt Controller) at 0x1000F000 / 0x1000F010
    pub intc_stat: u32,
    pub intc_mask: u32,
    pub sio: sio::Sio,
    pub dmac: dmac::Dmac,
    pub gs: gs::Gs,
    pub timers: timer::Timers,
    // Set by Dmac::write_reg when a channel's STR bit is newly set; the Bus
    // (which owns both RAM and the GS) drains this to perform the transfer.
    pub pending_dma_kick: Option<usize>,
}

impl Hardware {
    pub fn new() -> Self {
        Self {
            intc_stat: 0,
            intc_mask: 0,
            sio: sio::Sio::new(),
            dmac: dmac::Dmac::new(),
            gs: gs::Gs::new(),
            timers: timer::Timers::new(),
            pending_dma_kick: None,
        }
    }

    pub fn trigger_irq(&mut self, irq: u32) {
        self.intc_stat |= 1 << irq;
    }

    pub fn check_interrupts(&self) -> bool {
        (self.intc_stat & self.intc_mask) != 0
    }

    pub fn read32(&mut self, addr: u32) -> u32 {
        match addr {
            // INTC_STAT (0x1000F000)
            0x1000F000 => self.intc_stat,
            // INTC_MASK (0x1000F010)
            0x1000F010 => self.intc_mask,
            // Timers T0, T1, T2, T3 (0x10000000..=0x10001830)
            0x10000000..=0x10001830 => self.timers.read32(addr),
            // SIO
            0x1000F100..=0x1000F200 => self.sio.read32(addr),
            _ if dmac::Dmac::is_dmac_addr(addr) => self.dmac.read_reg(addr),
            // Default HW read
            _ => 0,
        }
    }

    pub fn write32(&mut self, addr: u32, val: u32) {
        match addr {
            // INTC_STAT (0x1000F000)
            0x1000F000 => {
                // Writing 1 to STAT clears the interrupt
                self.intc_stat &= !val;
            },
            // INTC_MASK (0x1000F010)
            0x1000F010 => {
                self.intc_mask = val;
            },
            // Timers T0, T1, T2, T3 (0x10000000..=0x10001830)
            0x10000000..=0x10001830 => {
                self.timers.write32(addr, val);
            },
            // SIO
            0x1000F100..=0x1000F200 => {
                self.sio.write32(addr, val);
            },
            _ if dmac::Dmac::is_dmac_addr(addr) => {
                if let Some(ch) = self.dmac.write_reg(addr, val) {
                    self.pending_dma_kick = Some(ch);
                }
            },
            _ => {}
        }
    }

    pub fn read8(&mut self, addr: u32) -> u8 {
        match addr {
            0x1000F100..=0x1000F200 => self.sio.read8(addr),
            _ => 0,
        }
    }

    pub fn write8(&mut self, addr: u32, val: u8) {
        match addr {
            0x1000F100..=0x1000F200 => self.sio.write8(addr, val),
            _ => {}
        }
    }
}
