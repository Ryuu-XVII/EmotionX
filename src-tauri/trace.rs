use memory::bus::Bus;
use cpu::ee::EmotionEngine;

#[path = "src/cpu/mod.rs"]
pub mod cpu;
#[path = "src/memory/mod.rs"]
pub mod memory;
#[path = "src/hw/mod.rs"]
pub mod hw;

fn main() {
    let bus = Bus::new();
    let mut ee = EmotionEngine::new(bus);
    
    for _ in 0..200 {
        let log = ee.step();
        println!("{}", log);
    }
}
