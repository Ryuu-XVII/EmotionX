use goblin::elf::{Elf, program_header::PT_LOAD};
use std::fs;
use crate::cpu::ee::EmotionEngine;

pub fn load_elf(path: &str, ee: &mut EmotionEngine) -> Result<u32, String> {
    let buffer = fs::read(path).map_err(|e| format!("Failed to read ELF file: {}", e))?;
    
    let elf = Elf::parse(&buffer).map_err(|e| format!("Failed to parse ELF: {}", e))?;
    
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
