# 🎮 EmotionX
> A custom PlayStation 2 emulator built with Rust and React.

<p align="center">
  <img src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white" alt="Rust" />
  <img src="https://img.shields.io/badge/Tauri-24C8DB?style=for-the-badge&logo=tauri&logoColor=FFFFFF" alt="Tauri" />
  <img src="https://img.shields.io/badge/React-20232A?style=for-the-badge&logo=react&logoColor=61DAFB" alt="React" />
  <img src="https://img.shields.io/badge/Tailwind_CSS-38B2AC?style=for-the-badge&logo=tailwind-css&logoColor=white" alt="Tailwind CSS" />
</p>

---

## ⚡ Overview
**EmotionX** is an ambitious project aiming to emulate the legendary PlayStation 2 hardware (the Emotion Engine) entirely from scratch using a highly optimized Rust backend (Tauri) and a beautiful, modern React frontend. 

The goal of this project is not just functionality, but aesthetic excellence and high-performance execution.

## 🚀 Progress & Features (So Far)

We are actively building out the core CPU and hardware interfaces. Here is what we've accomplished up to **Phase 6**:

### 🧠 The Emotion Engine (CPU)
- **Core Architecture:** 32 General Purpose Registers (GPRs), Program Counter (PC), and a customized fetch-decode-execute loop.
- **Instruction Decoding:** Built a robust decoder handling R-Type (`SPECIAL`), J-Type, and I-Type MIPS instructions.
- **Branch Delay Slots:** Implemented the PS2's notorious branch delay slot behavior (executing the instruction *after* a branch before taking the jump).
- **Control Flow & Logic:** Full support for `J`, `JAL`, `JR`, `JALR`, `ADDI`, `SLTI`, `BNE`, `BEQ`, and bitwise logic. The CPU actively runs initialization loops and clears registers exactly like real silicon.

### 💾 Memory & System Bus
- **Memory Map Routing:** A "traffic cop" system bus that routes data to `0x00000000` (RAM) or `0x1FC00000` (BIOS).
- **Baked-in BIOS:** Integrated an SCPH-10000 BIOS blob directly into the binary with the ability to dynamically load custom BIOS files via native OS dialogs.
- **Load/Store Ops:** Full support for memory instructions (`LW`, `SW`, `LB`, `SB`, `LH`, `SH`).

### 🛠️ Hardware Stubbing
- Created `hw/` module to intercept reads/writes to critical PS2 hardware.
- Faked the **Interrupt Controller (INTC)** and **DMA Controller (DMAC)** to allow the BIOS to boot without getting stuck in hardware-polling infinite loops.

### 🎨 The Interface
- **Tauri Integration:** Lightning-fast IPC between the Rust engine and React UI.
- **Developer Console:** Real-time terminal overlay in the UI to stream instruction execution logs and hardware states.
- **Continuous Execution Mode:** A "Run / Pause" system that allows the emulator to burst-process thousands of instructions rather than manually stepping.

## 🔮 Next Up: Phase 7 (Serial I/O)
The next major milestone is implementing the Serial I/O (SIO) hardware registers. By tapping into the SIO, we will be able to intercept the BIOS's internal `printf` debug statements (like `"Sony Computer Entertainment Inc."`) and stream them directly into our UI console!

---

### 💻 Getting Started
1. Clone the repository
2. Install dependencies with `npm install`
3. Run the development server with `npm run tauri dev`
