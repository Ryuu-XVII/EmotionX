<div align="center">

# 🎮 EmotionX
**A modern, high-performance PlayStation 2 (PS2) Emulator Core in Rust**

[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange.svg?style=for-the-badge&logo=rust)](https://www.rust-lang.org)
[![Tauri](https://img.shields.io/badge/Tauri-2.0-blue.svg?style=for-the-badge&logo=tauri)](https://tauri.app)
[![React](https://img.shields.io/badge/React-18-cyan.svg?style=for-the-badge&logo=react)](https://react.dev)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.0-blue.svg?style=for-the-badge&logo=typescript)](https://www.typescriptlang.org/)

EmotionX is an experimental PlayStation 2 emulator featuring a custom-built Rust backend for the Emotion Engine (EE) and a beautiful, modern React frontend built with Tauri.

</div>

---

## ✨ Features
- **Rust Backend**: Safe, fast, and concurrent emulation of the MIPS R5900 Emotion Engine.
- **Tauri Integration**: Seamless IPC communication between the emulator core and the UI.
- **Modern UI**: A sleek, dark-themed React UI with real-time logging, telemetry, and debugging tools.
- **Modular Architecture**: Clean separation of CPU (EE, COP0), Memory (Bus, RAM, BIOS), and Hardware (SIO, Timers).

## 🚀 Progress & Current Status (Phase 6)
We are currently actively developing the core execution loop and boot sequence of the Emotion Engine. 
- [x] **Phase 1-3**: Project setup, React/Tauri scaffolding, and basic IPC communication.
- [x] **Phase 4**: Implementation of the MIPS instruction decoder and basic CPU registers.
- [x] **Phase 5**: Basic memory bus (32MB RAM, 4MB BIOS mapping), memory-mapped IO setup.
- [x] **Phase 6**: BIOS boot sequence debugging! The CPU can now read the BIOS, decompress the Exception Handlers via `LHU`/`LBU` memory reads, and map the TLB refill vectors!
- [ ] **Phase 7**: Hardware Initialization & Coprocessor 0 (COP0) implementation.

## 🛠️ Getting Started

### Prerequisites
- [Rust](https://rustup.rs/) (latest stable)
- [Node.js](https://nodejs.org/) 18+
- PS2 BIOS (`scph39001.bin` or similar) placed in the `bios/` directory.

### Building & Running
1. Clone the repository:
   ```bash
   git clone https://github.com/Ryuu-XVII/EmotionX.git
   cd EmotionX
   ```
2. Install frontend dependencies:
   ```bash
   npm install
   ```
3. Run the development server (Tauri + Vite):
   ```bash
   npm run tauri dev
   ```

## 🏗️ Architecture
- **`src-tauri/src/cpu/`**: Contains the Emotion Engine (EE) MIPS core, instruction decoding, and execution loops.
- **`src-tauri/src/memory/`**: The main system bus, memory arrays (RAM/BIOS), and MMIO routing.
- **`src-tauri/src/hw/`**: Hardware subsystems like SIO (Serial I/O) for BIOS console output.
- **`src/`**: The React + Tailwind CSS frontend interface.

## 📝 License
This project is licensed under the MIT License.

---
*Disclaimer: EmotionX is an educational project and is not affiliated with Sony Computer Entertainment. You must legally own a PS2 console to dump and use its BIOS.*
