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

    /// Whether `addr` is backed by real code (RAM or BIOS), as opposed to
    /// unmapped space that silently reads back as all-zero (NOP). Used to
    /// detect a CPU that's run off the rails into empty memory, so the
    /// emulator can stop instead of executing NOPs forever.
    pub fn is_code_mapped(&self, addr: u32) -> bool {
        let phys_addr = addr & 0x1FFFFFFF;
        phys_addr < (MAIN_MEMORY_SIZE as u32)
            || (phys_addr >= 0x1FC00000 && phys_addr < 0x1FC00000 + (self.bios.len() as u32))
    }

    pub fn read32(&mut self, addr: u32) -> u32 {
        let phys_addr = addr & 0x1FFFFFFF; // Basic physical address masking
        
        if phys_addr < (MAIN_MEMORY_SIZE as u32) {
            // RAM read (with safe bounds checking for unaligned accesses)
            let offset = phys_addr as usize;
            let b0 = *self.ram.get(offset).unwrap_or(&0) as u32;
            let b1 = *self.ram.get(offset + 1).unwrap_or(&0) as u32;
            let b2 = *self.ram.get(offset + 2).unwrap_or(&0) as u32;
            let b3 = *self.ram.get(offset + 3).unwrap_or(&0) as u32;
            b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
        } else if phys_addr >= 0x1FC00000 && phys_addr < 0x1FC00000 + (self.bios.len() as u32) {
            // BIOS read (with safe bounds checking)
            let offset = (phys_addr - 0x1FC00000) as usize;
            let b0 = *self.bios.get(offset).unwrap_or(&0) as u32;
            let b1 = *self.bios.get(offset + 1).unwrap_or(&0) as u32;
            let b2 = *self.bios.get(offset + 2).unwrap_or(&0) as u32;
            let b3 = *self.bios.get(offset + 3).unwrap_or(&0) as u32;
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
            let b0 = *self.ram.get(offset).unwrap_or(&0) as u16;
            let b1 = *self.ram.get(offset + 1).unwrap_or(&0) as u16;
            b0 | (b1 << 8)
        } else if phys_addr >= 0x1FC00000 && phys_addr < 0x1FC00000 + (self.bios.len() as u32) {
            let offset = (phys_addr - 0x1FC00000) as usize;
            let b0 = *self.bios.get(offset).unwrap_or(&0) as u16;
            let b1 = *self.bios.get(offset + 1).unwrap_or(&0) as u16;
            b0 | (b1 << 8)
        } else {
            0 // TODO MMIO read16
        }
    }

    pub fn read8(&mut self, addr: u32) -> u8 {
        let phys_addr = addr & 0x1FFFFFFF;
        if phys_addr < (MAIN_MEMORY_SIZE as u32) {
            *self.ram.get(phys_addr as usize).unwrap_or(&0)
        } else if phys_addr >= 0x1FC00000 && phys_addr < 0x1FC00000 + (self.bios.len() as u32) {
            *self.bios.get((phys_addr - 0x1FC00000) as usize).unwrap_or(&0)
        } else if phys_addr >= 0x10000000 && phys_addr < 0x10010000 {
            self.hw.read8(phys_addr)
        } else {
            0
        }
    }

    pub fn read128(&mut self, addr: u32) -> u128 {
        // 128-bit reads are only expected from RAM (or maybe some HW). 
        // Addresses must be 16-byte aligned, but we just read 16 bytes.
        let phys_addr = addr & 0x1FFFFFFF;
        if phys_addr < (MAIN_MEMORY_SIZE as u32) {
            let offset = phys_addr as usize;
            if offset + 15 < MAIN_MEMORY_SIZE {
                let mut buf = [0u8; 16];
                buf.copy_from_slice(&self.ram[offset..offset+16]);
                u128::from_le_bytes(buf)
            } else {
                0
            }
        } else {
            0
        }
    }

    pub fn write32(&mut self, addr: u32, val: u32) {
        let phys_addr = addr & 0x1FFFFFFF;
        
        if phys_addr < (MAIN_MEMORY_SIZE as u32) {
            // RAM write
            let offset = phys_addr as usize;
            if offset + 3 < MAIN_MEMORY_SIZE {
                self.ram[offset] = (val & 0xFF) as u8;
                self.ram[offset + 1] = ((val >> 8) & 0xFF) as u8;
                self.ram[offset + 2] = ((val >> 16) & 0xFF) as u8;
                self.ram[offset + 3] = ((val >> 24) & 0xFF) as u8;
            }
        } else if phys_addr >= 0x1FC00000 && phys_addr < 0x1FC00000 + (self.bios.len() as u32) {
            // BIOS is read-only
        } else if phys_addr >= 0x10000000 && phys_addr < 0x10010000 {
            // Hardware/MMIO write
            self.hw.write32(phys_addr, val);
            if let Some(ch) = self.hw.pending_dma_kick.take() {
                self.execute_dma(ch);
            }
        }
    }

    pub fn write16(&mut self, addr: u32, val: u16) {
        let phys_addr = addr & 0x1FFFFFFF;
        if phys_addr < (MAIN_MEMORY_SIZE as u32) {
            let offset = phys_addr as usize;
            if offset + 1 < MAIN_MEMORY_SIZE {
                self.ram[offset] = (val & 0xFF) as u8;
                self.ram[offset + 1] = ((val >> 8) & 0xFF) as u8;
            }
        } else if phys_addr >= 0x1FC00000 && phys_addr < 0x1FC00000 + (self.bios.len() as u32) {
            // BIOS is read-only
        } else if phys_addr >= 0x10000000 && phys_addr < 0x10010000 {
            // HW write16
        }
    }

    pub fn write8(&mut self, addr: u32, val: u8) {
        let phys_addr = addr & 0x1FFFFFFF;
        if phys_addr < (MAIN_MEMORY_SIZE as u32) {
            if let Some(byte) = self.ram.get_mut(phys_addr as usize) {
                *byte = val;
            }
        } else if phys_addr >= 0x1FC00000 && phys_addr < 0x1FC00000 + (self.bios.len() as u32) {
            // BIOS is read-only
        } else if phys_addr >= 0x10000000 && phys_addr < 0x10010000 {
            self.hw.write8(phys_addr, val);
        }
    }

    pub fn write128(&mut self, addr: u32, val: u128) {
        let phys_addr = addr & 0x1FFFFFFF;
        if phys_addr < (MAIN_MEMORY_SIZE as u32) {
            let offset = phys_addr as usize;
            if offset + 15 < MAIN_MEMORY_SIZE {
                let bytes = val.to_le_bytes();
                self.ram[offset..offset+16].copy_from_slice(&bytes);
            }
        } else if phys_addr >= 0x10000000 && phys_addr < 0x10010000 {
            // Ignored for now or handled if DMA
        }
    }

    pub fn read64(&mut self, addr: u32) -> u64 {
        let phys_addr = addr & 0x1FFFFFFF;

        if phys_addr >= 0x12000000 && phys_addr < 0x12002000 {
            return self.hw.gs.read64(phys_addr);
        }
        if phys_addr < (MAIN_MEMORY_SIZE as u32) {
            let offset = phys_addr as usize;
            if offset + 7 < MAIN_MEMORY_SIZE {
                let mut buf = [0u8; 8];
                buf.copy_from_slice(&self.ram[offset..offset + 8]);
                return u64::from_le_bytes(buf);
            }
            return 0;
        }
        if phys_addr >= 0x1FC00000 && phys_addr < 0x1FC00000 + (self.bios.len() as u32) {
            let offset = (phys_addr - 0x1FC00000) as usize;
            if offset + 7 < self.bios.len() {
                let mut buf = [0u8; 8];
                buf.copy_from_slice(&self.bios[offset..offset + 8]);
                return u64::from_le_bytes(buf);
            }
            return 0;
        }
        // Fallback: assemble from two 32-bit accesses (covers HW/MMIO ranges)
        let lo = self.read32(addr) as u64;
        let hi = self.read32(addr.wrapping_add(4)) as u64;
        lo | (hi << 32)
    }

    pub fn write64(&mut self, addr: u32, val: u64) {
        let phys_addr = addr & 0x1FFFFFFF;

        if phys_addr >= 0x12000000 && phys_addr < 0x12002000 {
            self.hw.gs.write64(phys_addr, val);
            return;
        }
        if phys_addr < (MAIN_MEMORY_SIZE as u32) {
            let offset = phys_addr as usize;
            if offset + 7 < MAIN_MEMORY_SIZE {
                self.ram[offset..offset + 8].copy_from_slice(&val.to_le_bytes());
            }
            return;
        }
        if phys_addr >= 0x1FC00000 && phys_addr < 0x1FC00000 + (self.bios.len() as u32) {
            // BIOS is read-only
            return;
        }
        // Fallback: split into two 32-bit accesses (covers HW/MMIO ranges)
        self.write32(addr, (val & 0xFFFFFFFF) as u32);
        self.write32(addr.wrapping_add(4), (val >> 32) as u32);
    }

    /// Executes a DMAC channel transfer that was just kicked (STR bit set).
    /// Runs to completion instantly rather than modeling per-cycle bus timing.
    pub fn execute_dma(&mut self, ch: usize) {
        let chcr = self.hw.dmac.channels[ch].chcr;
        let mode = (chcr >> 2) & 0x3; // 0=normal, 1=chain, 2=interleave
        let mut madr = self.hw.dmac.channels[ch].madr;
        let mut qwc = self.hw.dmac.channels[ch].qwc;
        let mut payload: Vec<u8> = Vec::new();

        if mode == 0 {
            // Normal mode: QWC quadwords starting at MADR
            for _ in 0..qwc {
                payload.extend_from_slice(&self.read128(madr).to_le_bytes());
                madr = madr.wrapping_add(16);
            }
            qwc = 0;
        } else if mode == 1 {
            // Chain mode: follow DMAtags starting at MADR
            let mut addr = madr;
            let mut guard = 0u32;
            loop {
                guard += 1;
                if guard > 65536 {
                    break; // malformed/looping chain safety valve
                }

                let tag_lo = self.read32(addr);
                let tag_hi = self.read32(addr.wrapping_add(4));
                let tag_qwc = tag_lo & 0xFFFF;
                let id = (tag_lo >> 28) & 0x7;
                let tag_addr = tag_hi & 0x7FFFFFFF;
                let data_start = addr.wrapping_add(16);

                match id {
                    1 | 7 => {
                        // CNT / END: data follows the tag immediately
                        for i in 0..tag_qwc {
                            payload.extend_from_slice(&self.read128(data_start.wrapping_add(i * 16)).to_le_bytes());
                        }
                        if id == 1 {
                            addr = data_start.wrapping_add(tag_qwc * 16);
                        } else {
                            break; // END
                        }
                    },
                    2 => {
                        // NEXT: data follows the tag; next tag address = ADDR field
                        for i in 0..tag_qwc {
                            payload.extend_from_slice(&self.read128(data_start.wrapping_add(i * 16)).to_le_bytes());
                        }
                        addr = tag_addr;
                    },
                    0 | 3 | 4 => {
                        // REFE / REF / REFS: data lives at ADDR; tag chain continues sequentially
                        for i in 0..tag_qwc {
                            payload.extend_from_slice(&self.read128(tag_addr.wrapping_add(i * 16)).to_le_bytes());
                        }
                        if id == 0 {
                            break; // REFE
                        }
                        addr = addr.wrapping_add(16);
                    },
                    _ => {
                        // CALL/RET not implemented; stop rather than hang
                        break;
                    }
                }
            }
            madr = addr;
            qwc = 0;
        } else {
            // Interleave mode not implemented
            qwc = 0;
        }

        if ch == crate::hw::dmac::CH_GIF {
            self.hw.gs.receive_gif_data(&payload);
        } else if ch == crate::hw::dmac::CH_SIF1 {
            self.handle_sif1_packet(&payload);
        }
        // Other channels: no VIF/IPU/SIF0/SIF2 backend yet, data is simply consumed.

        self.hw.dmac.channels[ch].madr = madr;
        self.hw.dmac.channels[ch].qwc = qwc;
        self.hw.dmac.clear_str(ch);

        self.hw.dmac.d_stat |= 1 << ch;
        // Approximate mapping (mirrors the existing "artificial VBlank" style elsewhere in this codebase).
        self.hw.trigger_irq(9 + ch as u32);
    }

    /// Minimal SIF (Sub-system Interface) RPC HLE: the EE has no real IOP to talk to (no IOP
    /// CPU, no SIF0/SIF2 backend, no actual module behavior), so games attempting to bind to
    /// an IOP RPC service (pad, sound, CDVD, etc. via sceSifBindRpc) would otherwise wait
    /// forever on a response that never arrives. This intercepts the well-documented,
    /// protocol-level SIF_CMD_RPC_BIND packet format (fixed across all games/services - see
    /// ps2sdk's SifCmdHeader_t/SifRpcBindPkt_t/SifRpcClientData_t) and synthesizes an
    /// immediate "bind succeeded" response by writing directly into the client structure the
    /// game specified, without needing any real IOP-side processing.
    ///
    /// This only unblocks the *bind* step. Any actual SIF_CMD_RPC_CALL made to one of these
    /// fake-bound "services" still has no real backend behind it - implementing genuine pad/
    /// sound/CD IOP module behavior is separate, future work.
    fn handle_sif1_packet(&mut self, payload: &[u8]) {
        const SIF_CMD_RPC_BIND: u32 = 0x80000009;
        const CD_FIELD_COMMAND_OFFSET: u32 = 16;
        const CD_FIELD_SERVER_OFFSET: u32 = 36;

        if payload.len() < 36 {
            return;
        }
        let cid = u32::from_le_bytes(payload[8..12].try_into().unwrap());
        if cid != SIF_CMD_RPC_BIND {
            return;
        }

        let cd_addr = u32::from_le_bytes(payload[28..32].try_into().unwrap());
        if cd_addr == 0 {
            return;
        }
        // SifRpcClientData_t.server: NULL until bind completes: games poll/wait on this.
        // The exact value is unobservable to correct game code (only nullness is checked),
        // so a recognizably-fake sentinel is used rather than a real memory address.
        self.write32(cd_addr + CD_FIELD_SERVER_OFFSET, 0xBAD00001);
        self.write32(cd_addr + CD_FIELD_COMMAND_OFFSET, 0);
    }

    /// Reads a null-terminated string from memory starting at the given address.
    pub fn read_string(&mut self, mut addr: u32) -> String {
        let mut result = String::new();
        loop {
            let b = self.read8(addr);
            if b == 0 {
                break;
            }
            result.push(b as char);
            addr = addr.wrapping_add(1);
        }
        result
    }
}
