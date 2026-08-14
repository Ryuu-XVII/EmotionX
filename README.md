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

## 🚀 Progress & Current Status (Phase 23)
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
- [x] **Phase 13**: VU0 macro mode (COP2) — register transfers, memory transfers, branch, and the broadcast FMAC/accumulate instruction families (scoped subset; see PROGRESS.md for the known gap around combined broadcast+accumulate opcodes).
- [x] **Phase 13.1**: Diagnosed and fixed the real-BIOS boot hang — two missing instruction families (`LDL`/`LDR`/`SDL`/`SDR` and `LL`/`SC`/`LLD`/`SCD`) were causing an unrecoverable Reserved Instruction exception loop. Confirmed the BIOS now progresses tens of millions of instructions further before hitting the real remaining blocker: it needs disc/IOP support (Milestone 2) to load the rest of its own boot code.
- [x] **Phase 17**: `boot_game` now actually reads the selected disc image — a minimal ISO9660 reader parses `SYSTEM.CNF`, resolves the `BOOT2` executable, and loads it straight into EE RAM (bypassing IOP/CDVD emulation, which doesn't exist yet).
- [x] **Phase 17.1**: Native `.chd` (compressed disc image) support, verified end-to-end against a real ~3GB retail PS2 CHD rip.
- [x] **Phase 18**: MMI (SIMD) instruction set — the real blocker for retail games, whose very first instruction uses it. ~34 instructions implemented; a few rarer gaps (HI1/LO1 pipeline, SA-register/`QFSRV`) remain and are the next thing to chase down.
- [x] **Phase 19**: Full instruction-set audit across every dispatch table (opcode/SPECIAL/REGIMM/COP0/COP1/COP2/MMI) against the standard MIPS + EE opcode space — found and fixed `DADDI`/`DADDIU`/`LWU`/`PREF`/`BLTZAL` family/`MTSAB`/`MTSAH`, with the remaining known gaps documented in PROGRESS.md.
- [x] **Phase 20**: HI1/LO1 multiply/divide pipeline and FPU accumulator extensions — reached **zero unknown-opcode hits across 20 million instructions** of a real retail game's boot trace. The remaining wall is very likely the IOP/SIF gap (Milestone 2), not a CPU bug — see PROGRESS.md for how that was traced.
- [x] **Phase 21**: SIF RPC bind HLE (verified against ps2sdk source) and kernel semaphore syscalls. Real, tested infrastructure — but honestly, it did *not* resolve the Phase 20 derailment; that stall's actual cause is still unidentified. See PROGRESS.md.
- [x] **Phase 22**: Confirmed via the real running app that disc booting now works end-to-end (`Booted 'SLUS_213.51;1'...`). Added runaway-execution detection so hitting the derailment auto-pauses with a clear message instead of flooding the log forever.
- [x] **Phase 23**: Found and fixed the actual root cause of the long-standing derailment — the stack pointer was never valid when the game first used it (a kernel invariant this emulator's fast-boot shortcut wasn't providing), causing every stack save/restore to silently corrupt via BIOS-ROM aliasing. **The real NFS Most Wanted disc image now runs 100 million instructions with zero derailment**, settling into a stable loop instead of crashing.
- [ ] **Phase 24**: TBD

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
