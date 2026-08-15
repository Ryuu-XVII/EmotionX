use crate::hw::Hardware;

// The PS2 has 32MB of main RDRAM.
pub const MAIN_MEMORY_SIZE: usize = 32 * 1024 * 1024; 

pub struct Bus {
    pub ram: Vec<u8>,
    pub bios: &'static [u8], // Baked-in BIOS
    pub hw: Hardware,
    pub iso: Option<crate::iso9660::Iso9660>,
}

impl Bus {
    pub fn new() -> Self {
        // Load the baked-in BIOS via the include_bytes! macro
        let bios_data = include_bytes!("../../../bios/SCPH-90001_BIOS_V18_USA_230.ROM0");
        
        Self {
            ram: vec![0; MAIN_MEMORY_SIZE],
            bios: bios_data,
            hw: Hardware::new(),
            iso: None,
        }
    }

    /// Attaches an active disc image reader to the bus for CDVD / FileIO HLE streaming.
    pub fn attach_iso(&mut self, iso: crate::iso9660::Iso9660) {
        self.iso = Some(iso);
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
        } else if ch == crate::hw::dmac::CH_VIF1 {
            self.handle_vif1_packet(&payload);
        }
        // Other channels: no IPU/SIF0/SIF2 backend yet, data is simply consumed.

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
        const SIF_CMD_RPC_CALL: u32 = 0x8000000A;
        const CD_FIELD_COMMAND_OFFSET: u32 = 16;
        const CD_FIELD_SERVER_OFFSET: u32 = 36;

        if payload.len() < 32 {
            return;
        }
        let cid = u32::from_le_bytes(payload[8..12].try_into().unwrap());

        if cid == SIF_CMD_RPC_BIND {
            if payload.len() < 36 {
                return;
            }
            let cd_addr = u32::from_le_bytes(payload[28..32].try_into().unwrap());
            if cd_addr == 0 {
                return;
            }
            // SifRpcClientData_t.server: NULL until bind completes: games poll/wait on this.
            self.write32(cd_addr + CD_FIELD_SERVER_OFFSET, 0xBAD00001);
            self.write32(cd_addr + CD_FIELD_COMMAND_OFFSET, 0);
            return;
        }

        if cid == SIF_CMD_RPC_CALL {
            if payload.len() < 52 {
                return;
            }
            let rpc_id = u32::from_le_bytes(payload[24..28].try_into().unwrap());
            let client_addr = u32::from_le_bytes(payload[28..32].try_into().unwrap());
            let send_addr = u32::from_le_bytes(payload[32..36].try_into().unwrap());
            let send_size = u32::from_le_bytes(payload[36..40].try_into().unwrap());
            let recv_addr = u32::from_le_bytes(payload[40..44].try_into().unwrap());
            let recv_size = u32::from_le_bytes(payload[44..48].try_into().unwrap());

            self.handle_sif_rpc_call(rpc_id, client_addr, send_addr, send_size, recv_addr, recv_size);
        }
    }

    /// Handles High-Level Emulated (HLE) SIF RPC calls, including CDVD and FileIO disc reads.
    fn handle_sif_rpc_call(
        &mut self,
        rpc_id: u32,
        client_addr: u32,
        send_addr: u32,
        _send_size: u32,
        recv_addr: u32,
        _recv_size: u32,
    ) {
        const CD_FIELD_COMMAND_OFFSET: u32 = 16;
        const CD_FIELD_BUFF_OFFSET: u32 = 20;

        match rpc_id {
            // CDVD Read (sceCdRead / sceCdReadDVD / sceCdReadCD / N-command read)
            1 | 2 | 3 => {
                if send_addr != 0 {
                    let lba = self.read32(send_addr);
                    let sectors = self.read32(send_addr + 4);
                    let dest_buf = self.read32(send_addr + 8);

                    if let Some(mut iso) = self.iso.take() {
                        let total_bytes = (sectors as usize) * 2048;
                        let mut temp = vec![0u8; total_bytes];
                        if iso.read_sectors(lba, sectors, &mut temp).is_ok() {
                            let phys_dst = (dest_buf & 0x1FFFFFFF) as usize;
                            if phys_dst + total_bytes <= self.ram.len() {
                                self.ram[phys_dst..phys_dst + total_bytes].copy_from_slice(&temp);
                            }
                        }
                        self.iso = Some(iso);
                    }
                }
                if recv_addr != 0 {
                    self.write32(recv_addr, 1); // 1 = success
                }
            },
            // CDVD SearchFile (sceCdSearchFile)
            10 => {
                if send_addr != 0 {
                    let path = self.read_string(send_addr);
                    let clean_path = path.trim_start_matches(|c| c == '\\' || c == '/');
                    let clean_path = clean_path.strip_prefix("cdrom0:").unwrap_or(clean_path);
                    let clean_path = clean_path.strip_prefix("cdrom:").unwrap_or(clean_path);
                    let clean_path = clean_path.trim_start_matches(|c| c == '\\' || c == '/');

                    if let Some(mut iso) = self.iso.take() {
                        if let Ok((lba, size)) = iso.find_file_info(clean_path) {
                            if recv_addr != 0 {
                                // sceCdlFILE struct layout (32 bytes):
                                // offset 0: lba (u32), offset 4: size (u32), offset 8..24: name (16 bytes)
                                self.write32(recv_addr, lba);
                                self.write32(recv_addr + 4, size);
                                for (i, b) in clean_path.as_bytes().iter().take(15).enumerate() {
                                    self.write8(recv_addr + 8 + (i as u32), *b);
                                }
                                self.write8(recv_addr + 8 + 15, 0);
                            }
                        }
                        self.iso = Some(iso);
                    }
                }
                if recv_addr != 0 {
                    self.write32(recv_addr, 1);
                }
            },
            // CDVD DiskReady / GetDiskType / GetTrayStatus / Status
            4 | 5 | 6 => {
                if recv_addr != 0 {
                    self.write32(recv_addr, 0x14); // 0x14 = PS2 DVD
                }
            },
            // PAD Init / Open / Read / GetState / InfoMode (DualShock 2 HLE)
            0x0100 | 0x0101 | 0x80000100 | 0x80000101 => {
                if recv_addr != 0 {
                    // Standard DualShock 2 response: status=OK, id=0x73, buttons=0xFFFF (unpressed), analog=128
                    self.write8(recv_addr, 0x00);
                    self.write8(recv_addr + 1, 0x73);
                    self.write8(recv_addr + 2, 0xFF);
                    self.write8(recv_addr + 3, 0xFF);
                    self.write8(recv_addr + 4, 128);
                    self.write8(recv_addr + 5, 128);
                    self.write8(recv_addr + 6, 128);
                    self.write8(recv_addr + 7, 128);
                }
            },
            // Default / Other RPC calls
            _ => {
                if recv_addr != 0 {
                    self.write32(recv_addr, 0);
                }
            }
        }

        // Signal RPC completion in SifRpcClientData_t
        if client_addr != 0 {
            self.write32(client_addr + CD_FIELD_COMMAND_OFFSET, 0); // command = 0 (completed)
            if recv_addr == 0 {
                let buff_ptr = self.read32(client_addr + CD_FIELD_BUFF_OFFSET);
                if buff_ptr != 0 {
                    self.write32(buff_ptr, 1);
                }
            }
        }
    }

    /// Handles VIF1 DMA packets, passing DIRECT commands straight through into the GS.
    fn handle_vif1_packet(&mut self, payload: &[u8]) {
        let mut pos = 0usize;
        while pos + 4 <= payload.len() {
            let cmd_word = u32::from_le_bytes(payload[pos..pos + 4].try_into().unwrap());
            let imm = (cmd_word & 0xFFFF) as usize;
            let num = ((cmd_word >> 16) & 0xFF) as usize;
            let cmd = ((cmd_word >> 24) & 0x7F) as u8;
            pos += 4;

            match cmd {
                0x00 => {
                    // NOP
                },
                0x50 => {
                    // DIRECT: imm specifies the number of QWORDS passed straight to GIF
                    let qwords = if imm == 0 { 65536 } else { imm };
                    let bytes = qwords * 16;
                    let end = (pos + bytes).min(payload.len());
                    if pos < end {
                        self.hw.gs.receive_gif_data(&payload[pos..end]);
                    }
                    pos = end;
                },
                0x10 | 0x11 | 0x13 | 0x14 | 0x17 => {
                    // FLUSH / MSCAL
                },
                0x60..=0x7F => {
                    // UNPACK: skip payload
                    let vl = (cmd & 0x3) as usize;
                    let vn = ((cmd >> 2) & 0x3) as usize;
                    let size_table = [4, 2, 1, 0];
                    let elem_size = size_table[vl] * (vn + 1);
                    let total_bytes = (num * elem_size + 3) & !3;
                    pos = (pos + total_bytes).min(payload.len());
                },
                _ => {}
            }
        }
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
