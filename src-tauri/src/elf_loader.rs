use goblin::elf::{Elf, program_header::PT_LOAD};
use std::fs;
use crate::cpu::ee::EmotionEngine;

pub fn load_elf(path: &str, ee: &mut EmotionEngine) -> Result<u32, String> {
    let buffer = fs::read(path).map_err(|e| format!("Failed to read ELF file: {}", e))?;
    load_elf_bytes(&buffer, ee)
}

pub fn load_elf_bytes(buffer: &[u8], ee: &mut EmotionEngine) -> Result<u32, String> {
    let elf = Elf::parse(buffer).map_err(|e| format!("Failed to parse ELF: {}", e))?;
    
    // Check if it is a MIPS binary
    if elf.header.e_machine != goblin::elf::header::EM_MIPS {
        return Err("Not a MIPS executable".to_string());
    }

    for ph in elf.program_headers.iter() {
        if ph.p_type == PT_LOAD {
            let offset = ph.p_offset as usize;
            let filesz = ph.p_filesz as usize;
            let memsz = ph.p_memsz as usize;
            let vaddr = ph.p_vaddr as u32;

            // Ensure the segment is within the file buffer
            if offset + filesz > buffer.len() {
                return Err("ELF segment goes out of bounds".to_string());
            }

            let segment_data = &buffer[offset..offset + filesz];

            // Copy file data to simulated PS2 RAM
            for i in 0..filesz {
                ee.bus.write8(vaddr + (i as u32), segment_data[i]);
            }

            // Zero-fill the rest (BSS segment)
            for i in filesz..memsz {
                ee.bus.write8(vaddr + (i as u32), 0);
            }
        }
    }

    // Real hardware always has the kernel set up $sp (and other initial thread state) before
    // handing control to any ELF - the executable's own code never initializes its own stack
    // pointer. Since we skip that whole kernel/BIOS boot sequence and jump straight to the
    // entry point, $sp is left at EmotionEngine::new()'s default of 0; the game's first
    // function-call prologue then computes stack addresses by subtracting from that near-zero
    // value, wrapping around to addresses near the top of the 32-bit range that alias into the
    // (read-only) BIOS ROM region instead of real RAM - so every stack save silently goes
    // nowhere and every restore reads back garbage BIOS bytes. Match the standard PS2 kernel
    // convention: initial stack near the top of the 32MB RAM (leaving a little headroom).
    ee.set_reg(29, 0x81FFFFF0);

    // Return the entry point
    Ok(elf.header.e_entry as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::bus::Bus;

    #[test]
    fn test_load_elf() {
        let bus = Bus::new();
        let mut ee = EmotionEngine::new(bus);
        let res = load_elf("../test_ps2.elf", &mut ee);
        println!("Load ELF result: {:?}", res);
        
        if let Ok(entry) = res {
            ee.set_pc(entry);
            println!("PC: {:#010X}", ee.pc);
            let log = ee.step();
            println!("Step 1: {}", log);
            let log = ee.step();
            println!("Step 2: {}", log);
        }
        
        assert!(res.is_ok());
    }
}
