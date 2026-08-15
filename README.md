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

## 🚀 Progress & Current Status (Phase 25)
We are currently actively developing the core execution loop, boot sequence, and homebrew loading support of the Emotion Engine. 
- [x] **Phase 1-3**: Project setup, React/Tauri scaffolding, and basic IPC communication.
- [x] **Phase 4**: Implementation of the MIPS instruction decoder and basic CPU registers.
- [x] **Phase 5**: Basic memory bus (32MB RAM, 4MB BIOS mapping), memory-mapped IO setup.
- [x] **Phase 6**: BIOS boot sequence debugging, memory opcodes, and control flow branching.
- [x] **Phase 7**: Hardware Interrupts & COP0 integration (INTC registers, VBlank timer).
- [x] **Phase 8**: Basic ELF Loader (loading `.elf` segments into RAM and executing them).
- [x] **Phase 9**: BIOS High-Level Emulation (HLE) & Syscall Interception (`Putc`, `Puts`).
- [x] **Phase 10**: CPU correctness pass — FPU (COP1), unaligned load/store (`LWL`/`LWR`/`SWL`/`SWR`), and branch-likely instructions.
- [x] **Phase 11**: DMA Controller (all 10 channels, normal + chain mode) and Graphics Synthesizer fundamentals (privileged registers + PACKED-mode GIFtag parsing into a software framebuffer).
- [x] **Phase 12**: Real GS primitive rasterization (points/lines/triangles/sprites) and a live Display tab in the UI showing the framebuffer.
- [x] **Phase 13**: VU0 macro mode (COP2) — register transfers, memory transfers, branch, and the broadcast FMAC/accumulate instruction families.
- [x] **Phase 13.1**: Diagnosed and fixed the real-BIOS boot hang — two missing instruction families (`LDL`/`LDR`/`SDL`/`SDR` and `LL`/`SC`/`LLD`/`SCD`) were causing an unrecoverable Reserved Instruction exception loop.
- [x] **Phase 17**: `boot_game` now actually reads the selected disc image — a minimal ISO9660 reader parses `SYSTEM.CNF`, resolves the `BOOT2` executable, and loads it straight into EE RAM.
- [x] **Phase 17.1**: Native `.chd` (compressed disc image) support, verified end-to-end against a real ~3GB retail PS2 CHD rip.
- [x] **Phase 18**: MMI (SIMD) instruction set — elementwise arithmetic/compare/min-max/logic and vector shuffle families.
- [x] **Phase 19**: Full instruction-set audit across every dispatch table — found and fixed `DADDI`/`DADDIU`/`LWU`/`PREF`/`BLTZAL`/`MTSAB`/`MTSAH`.
- [x] **Phase 20**: HI1/LO1 multiply/divide pipeline and FPU accumulator extensions — reached zero unknown-opcode hits on real retail game code.
- [x] **Phase 21**: SIF RPC bind HLE (verified against ps2sdk source) and kernel semaphore syscalls.
- [x] **Phase 22**: Runaway-execution detection auto-pauses when code fetches from unmapped memory.
- [x] **Phase 23**: Fixed root-cause stack pointer corruption: the real NFS Most Wanted disc image now runs 100 million instructions with zero derailment.
- [x] **Phase 24**: GS CSR field toggling (60Hz even/odd status), texture/context register parsing (`TEX0`/`TEX1`/`ALPHA`/`SCISSOR`), expanded PS2 kernel syscalls (`AddIntcHandler`/`EnableIntc`/`AddDmacHandler`/`CreateThread`/`GetOsdConfigParam`), MMI `QFSRV` funnel shift, and VU0 matrix transform / clipping (`VCLIP`/`VMULAbc`/`VMADDAbc`/`VIADD`/`VISUB`).
- [x] **Phase 25**: SIF RPC CDVD sector streaming (`sceCdRead`/`sceCdSearchFile`), DualShock 2 controller HLE (`scePadRead`), GS 4MB VRAM with GIF IMAGE mode (`FLG = 2`), UV/ST textured triangle & sprite rasterization, and VIF1 DMA `DIRECT` 3D pipeline.
- [ ] **Phase 26**: TBD

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
- **`src-tauri/src/hw/`**: Hardware subsystems — SIO (Serial I/O) for BIOS console output, DMAC (DMA Controller, `dmac.rs`) for RAM↔peripheral transfers, and the Graphics Synthesizer (`gs.rs`) with its privileged registers and software framebuffer.
- **`src/`**: The React + Tailwind CSS frontend interface.

## 📝 License
This project is licensed under the MIT License.

---
*Disclaimer: EmotionX is an educational project and is not affiliated with Sony Computer Entertainment. You must legally own a PS2 console to dump and use its BIOS.*
