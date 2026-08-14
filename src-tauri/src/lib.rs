pub mod cpu;
pub mod memory;
pub mod hw;
pub mod elf_loader;
pub mod iso9660;
pub mod chd_source;

use cpu::ee::EmotionEngine;
use memory::bus::Bus;
use std::sync::Mutex;
use tauri::{State, Emitter};

struct EmulatorState {
    engine: Mutex<Option<EmotionEngine>>,
}

#[tauri::command]
fn get_status(state: State<'_, EmulatorState>) -> String {
    let engine = state.engine.lock().unwrap();
    if let Some(ee) = &*engine {
        format!("Emulator Running. PC: {:#010X}", ee.pc)
    } else {
        "Emulator Idle. Baked-in BIOS ready.".to_string()
    }
}

#[tauri::command]
fn boot_game(path: &str, state: State<'_, EmulatorState>) -> Result<String, String> {
    let mut engine = state.engine.lock().unwrap();

    // Initialize the bus (loads baked-in BIOS)
    let bus = Bus::new();
    let mut ee = EmotionEngine::new(bus);

    // Read SYSTEM.CNF off the disc image, find the BOOT2 executable it points
    // to, and load that ELF directly into RAM. This bypasses the IOP/BIOS's
    // own disc-reading boot sequence (not yet emulated) but gets the game's
    // EE-side code running the same way `load_elf` does for homebrew.
    let mut iso = iso9660::Iso9660::open(path)?;
    let system_cnf = iso.read_file("SYSTEM.CNF")?;
    let boot_path = iso9660::parse_boot_path(&system_cnf)
        .ok_or_else(|| "SYSTEM.CNF found but has no BOOT2 entry".to_string())?;
    let elf_bytes = iso.read_file(&boot_path)?;
    let entry = elf_loader::load_elf_bytes(&elf_bytes, &mut ee)?;
    ee.set_pc(entry);

    *engine = Some(ee);

    Ok(format!("Booted '{}' from ISO, entry point {:#010X}", boot_path, entry))
}

#[tauri::command]
fn step_cpu(app: tauri::AppHandle, state: State<'_, EmulatorState>) -> Result<String, String> {
    let mut engine = state.engine.lock().unwrap();
    if let Some(ee) = engine.as_mut() {
        let log = ee.step();
        let pending = std::mem::take(&mut ee.bus.hw.sio.pending_lines);
        for line in pending {
            app.emit("sio-log", line).unwrap_or(());
        }
        Ok(log)
    } else {
        Err("Emulator not running. Boot a game first.".into())
    }
}

#[tauri::command]
fn run_cpu_batch(steps: u32, app: tauri::AppHandle, state: State<'_, EmulatorState>) -> Result<Vec<String>, String> {
    let mut engine = state.engine.lock().unwrap();
    if let Some(ee) = engine.as_mut() {
        let mut last_logs = Vec::new();
        const RUNAWAY_THRESHOLD: u32 = 256;
        let mut derailed = false;

        for i in 0..steps {
            let log = ee.step();

            // Only keep the last 20 logs to avoid sending massive arrays over IPC
            if i >= steps.saturating_sub(20) {
                last_logs.push(log);
            }

            if ee.consecutive_unmapped_fetches >= RUNAWAY_THRESHOLD {
                derailed = true;
                break;
            }
        }

        let pending = std::mem::take(&mut ee.bus.hw.sio.pending_lines);
        for line in pending {
            app.emit("sio-log", line).unwrap_or(());
        }

        if derailed {
            return Err(format!(
                "Execution halted: PC ran off into unmapped memory ({:#010X}) and would spin forever. This usually means the emulated code is waiting on hardware/IOP functionality that isn't implemented yet.",
                ee.pc
            ));
        }

        Ok(last_logs)
    } else {
        Err("Emulator not running. Boot a game first.".into())
    }
}

#[tauri::command]
fn get_framebuffer(state: State<'_, EmulatorState>) -> Result<Vec<u8>, String> {
    let engine = state.engine.lock().unwrap();
    if let Some(ee) = &*engine {
        let fb = &ee.bus.hw.gs.framebuffer;
        let mut bytes = Vec::with_capacity(fb.len() * 4);
        for &pixel in fb.iter() {
            // Internal format is 0xAARRGGBB; canvas ImageData wants R,G,B,A byte order.
            let a = ((pixel >> 24) & 0xFF) as u8;
            let r = ((pixel >> 16) & 0xFF) as u8;
            let g = ((pixel >> 8) & 0xFF) as u8;
            let b = (pixel & 0xFF) as u8;
            bytes.push(r);
            bytes.push(g);
            bytes.push(b);
            bytes.push(a);
        }
        Ok(bytes)
    } else {
        Err("Emulator not running. Boot a game first.".into())
    }
}

#[tauri::command]
fn load_elf(path: String, state: State<'_, EmulatorState>) -> Result<String, String> {
    let mut engine = state.engine.lock().unwrap();
    
    // Auto-boot if not already running
    if engine.is_none() {
        let bus = Bus::new();
        let mut ee = EmotionEngine::new(bus);
        ee.step();
        *engine = Some(ee);
    }
    
    if let Some(ee) = engine.as_mut() {
        match elf_loader::load_elf(&path, ee) {
            Ok(entry) => {
                ee.set_pc(entry);
                Ok(format!("Successfully loaded ELF and set PC to {:#010X}", entry))
            },
            Err(e) => Err(e),
        }
    } else {
        Err("Failed to auto-initialize emulator.".into())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(EmulatorState {
            engine: Mutex::new(None),
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![get_status, boot_game, step_cpu, run_cpu_batch, load_elf, get_framebuffer])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
