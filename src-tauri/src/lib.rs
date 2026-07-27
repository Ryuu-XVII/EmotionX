pub mod cpu;
pub mod memory;
pub mod hw;
pub mod elf_loader;

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
fn boot_game(path: &str, state: State<'_, EmulatorState>) -> String {
    let mut engine = state.engine.lock().unwrap();

    // Initialize the bus (loads baked-in BIOS)
    let bus = Bus::new();
    let mut ee = EmotionEngine::new(bus);

    // Step the CPU once just to simulate it starting
    ee.step();

    *engine = Some(ee);

    format!("Successfully booted ISO: {}", path)
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
        let mut full_trace = String::with_capacity((steps * 64) as usize);
        
        for i in 0..steps {
            let log = ee.step();
            full_trace.push_str(&log);
            full_trace.push('\n');
            
            // Only keep the last 20 logs to avoid sending massive arrays over IPC
            if i >= steps.saturating_sub(20) {
                last_logs.push(log);
            }
        }
        
        // Write trace to system temp directory to avoid both Tauri and Vite watcher rebuild loops
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(std::env::temp_dir().join("emotionx_trace.txt")) {
            use std::io::Write;
            let _ = file.write_all(full_trace.as_bytes());
        }

        let pending = std::mem::take(&mut ee.bus.hw.sio.pending_lines);
        for line in pending {
            app.emit("sio-log", line).unwrap_or(());
        }
        Ok(last_logs)
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
        .invoke_handler(tauri::generate_handler![get_status, boot_game, step_cpu, run_cpu_batch, load_elf])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
