# EmotionX - Progress Tracker

This document tracks the progress of **EmotionX**, a custom PlayStation 2 emulator built with Rust (Tauri Backend) and React (Frontend).

## Phase 1: Project Setup & Architecture
*   **Initialization:** Bootstrapped the project using Tauri (Rust) and React + Vite.
*   **UI Foundation:** Designed a sleek, hacker-style dark mode interface using Tailwind CSS.
*   **Architecture:** Established the core separation between the React frontend (UI) and the Rust backend (Emulator Engine).

## Phase 2: CPU Foundation (The Emotion Engine)
*   **EE Module:** Created the core `EmotionEngine` struct in Rust.
*   **Registers:** Implemented the 32 General Purpose Registers (GPRs) and the Program Counter (PC).
*   **Fetch Cycle:** Set up the basic loop to fetch 32-bit instructions from memory.

## Phase 3: The Memory Map & BIOS Loading
*   **Baked-in BIOS:** Integrated a default `SCPH-10000` BIOS directly into the emulator binary.
*   **System Bus:** Created the `Bus` struct to route memory requests. 
*   **Memory Mapping:** Mapped the BIOS to physical address `0x1FC00000`, matching real hardware.
*   **First Boot:** The CPU successfully fetched its very first instruction (`0x401A7800`) directly from the BIOS!

## Phase 4: Extended Instruction Decoding
*   **R-Type Instructions:** Taught the decoder how to parse `SPECIAL` opcodes (like `NOP`, `ADD`, `OR`).
*   **Branch Delay Slots:** Implemented the PS2's notorious branch delay slot behavior (executing the instruction *after* a branch before taking the jump).
*   **Continuous Execution:** Added a "Run/Pause" toggle in the UI for infinite looping via Tauri IPC commands.

## Phase 5: Memory Management & Hardware Stubbing
*   **Hardware Interception:** The `Bus` was upgraded to intercept reads/writes to hardware registers (`0x10000000` range) and RAM (`0x00000000` range).
*   **Hardware Stubs (`hw` module):** Built dummy endpoints for the Interrupt Controller (INTC) and DMA Controller (DMAC) so the BIOS doesn't hang when polling them.
*   **Memory Opcodes:** Implemented `LW`, `SW`, `LB`, `SB`, `LH`, and `SH`.

## Phase 6: Control Flow & Basic Arithmetic
*   **Jumps:** Implemented `J`, `JAL`, `JR`, and `JALR`. The CPU can now dynamically compute and branch to addresses.
*   **Immediate Arithmetic:** Added `ADDI`, `ADDIU`, `SLTI`, `SLTIU`, `ANDI`, and `XORI`.
*   **Status:** The CPU successfully broke out of its exception handler loop and executed a massive register-zeroing initialization sequence!

## Next Up: Phase 7 (Serial I/O and Debug Logs)
*   **Goal:** Implement the Serial I/O (SIO) registers to intercept the BIOS's internal `printf` debug statements (e.g., "Sony Computer Entertainment Inc.") and stream them to our UI console.
