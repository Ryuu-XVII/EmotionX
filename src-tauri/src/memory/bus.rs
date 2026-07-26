use crate::hw::Hardware;

// The PS2 has 32MB of main RDRAM.
pub const MAIN_MEMORY_SIZE: usize = 32 * 1024 * 1024; 

pub struct Bus {
    pub ram: Vec<u8>,
    pub bios: &'static [u8], // Baked-in BIOS
    pub hw: Hardware,
}

impl Bus {
    pub fn new() -> Self {
        // Load the baked-in BIOS via the include_bytes! macro
        let bios_data = include_bytes!("../../../bios/SCPH-90001_BIOS_V18_USA_230.ROM0");
        
        Self {
            ram: vec![0; MAIN_MEMORY_SIZE],
            bios: bios_data,
            hw: Hardware::new(),
        }
    }

    pub fn read32(&mut self, addr: u32) -> u32 {
        let phys_addr = addr & 0x1FFFFFFF; // Basic physical address masking
        
        if phys_addr < (MAIN_MEMORY_SIZE as u32) {
            // RAM read
            let offset = phys_addr as usize;
            let b0 = self.ram[offset] as u32;
            let b1 = self.ram[offset + 1] as u32;
            let b2 = self.ram[offset + 2] as u32;
            let b3 = self.ram[offset + 3] as u32;
            b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
        } else if phys_addr >= 0x1FC00000 && phys_addr < 0x1FC00000 + (self.bios.len() as u32) {
            // BIOS read
            let offset = (phys_addr - 0x1FC00000) as usize;
            let b0 = self.bios[offset] as u32;
            let b1 = self.bios[offset + 1] as u32;
            let b2 = self.bios[offset + 2] as u32;
            let b3 = self.bios[offset + 3] as u32;
            b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
        } else if phys_addr >= 0x10000000 && phys_addr < 0x10010000 {
            // Hardware/MMIO read
            self.hw.read32(phys_addr)
        } else {
            // Unmapped read
            0
        }
    }

    pub fn read16(&mut self, addr: u32) -> u16 {
        let phys_addr = addr & 0x1FFFFFFF;
        if phys_addr < (MAIN_MEMORY_SIZE as u32) {
            let offset = phys_addr as usize;
            let b0 = self.ram[offset] as u16;
            let b1 = self.ram[offset + 1] as u16;
            b0 | (b1 << 8)
        } else if phys_addr >= 0x1FC00000 && phys_addr < 0x1FC00000 + (self.bios.len() as u32) {
            let offset = (phys_addr - 0x1FC00000) as usize;
            let b0 = self.bios[offset] as u16;
            let b1 = self.bios[offset + 1] as u16;
            b0 | (b1 << 8)
        } else {
            0 // TODO MMIO read16
        }
    }

    pub fn read8(&mut self, addr: u32) -> u8 {
        let phys_addr = addr & 0x1FFFFFFF;
        if phys_addr < (MAIN_MEMORY_SIZE as u32) {
            self.ram[phys_addr as usize]
        } else if phys_addr >= 0x1FC00000 && phys_addr < 0x1FC00000 + (self.bios.len() as u32) {
            self.bios[(phys_addr - 0x1FC00000) as usize]
        } else {
            0 // TODO MMIO read8
        }
    }

    pub fn write32(&mut self, addr: u32, val: u32) {
        let phys_addr = addr & 0x1FFFFFFF;
        
        if phys_addr < (MAIN_MEMORY_SIZE as u32) {
            // RAM write
            let offset = phys_addr as usize;
            self.ram[offset] = (val & 0xFF) as u8;
            self.ram[offset + 1] = ((val >> 8) & 0xFF) as u8;
            self.ram[offset + 2] = ((val >> 16) & 0xFF) as u8;
            self.ram[offset + 3] = ((val >> 24) & 0xFF) as u8;
        } else if phys_addr >= 0x10000000 && phys_addr < 0x10010000 {
            // Hardware/MMIO write
            self.hw.write32(phys_addr, val);
        }
    }

    pub fn write16(&mut self, addr: u32, val: u16) {
        let phys_addr = addr & 0x1FFFFFFF;
        if phys_addr < (MAIN_MEMORY_SIZE as u32) {
            let offset = phys_addr as usize;
            self.ram[offset] = (val & 0xFF) as u8;
            self.ram[offset + 1] = ((val >> 8) & 0xFF) as u8;
        }
    }

    pub fn write8(&mut self, addr: u32, val: u8) {
        let phys_addr = addr & 0x1FFFFFFF;
        if phys_addr < (MAIN_MEMORY_SIZE as u32) {
            self.ram[phys_addr as usize] = val;
        }
    }
}
