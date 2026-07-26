pub mod cpu;
pub mod memory;
pub mod hw;

use cpu::ee::EmotionEngine;
use memory::bus::Bus;
use std::sync::Mutex;
use tauri::State;

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
fn step_cpu(state: State<'_, EmulatorState>) -> Result<String, String> {
    let mut engine = state.engine.lock().unwrap();
    if let Some(ee) = engine.as_mut() {
        Ok(ee.step())
    } else {
        Err("Emulator not running. Boot a game first.".into())
    }
}

#[tauri::command]
fn run_cpu_batch(steps: u32, state: State<'_, EmulatorState>) -> Result<Vec<String>, String> {
    let mut engine = state.engine.lock().unwrap();
    if let Some(ee) = engine.as_mut() {
        let mut logs = Vec::new();
        for _ in 0..steps {
            logs.push(ee.step());
        }
        Ok(logs)
    } else {
        Err("Emulator not running. Boot a game first.".into())
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
        .invoke_handler(tauri::generate_handler![get_status, boot_game, step_cpu, run_cpu_batch])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
