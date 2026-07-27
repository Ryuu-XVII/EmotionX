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

## Phase 7: Hardware Interrupts & COP0 Integration
*   **COP0 Registers:** Implemented basic Coprocessor 0 state including `Status`, `Cause`, `EPC`, `Count`, and `Compare`.
*   **Hardware Interrupts:** Implemented the INTC MMIO registers (`INTC_STAT` and `INTC_MASK`) to queue and fire hardware interrupts to the CPU when unmasked.
*   **Exception Delivery:** The CPU accurately syncs the INTC status to `COP0.Cause.IP2`, checks IE/IM flags, and triggers a hardware interrupt exception (ExcCode 0) to `0x80000200` (or `0xBFC00180` if BEV).
*   **Timers:** Added an artificial VBlank interrupt to trigger every 100,000 cycles, helping the BIOS break out of idle states.

## Phase 8: Basic ELF Loader
*   **ELF Parsing:** Integrated the `goblin` crate to parse standard MIPS executables (.elf).
*   **Loading to RAM:** Extracted the `PT_LOAD` segments from the ELF and mapped them directly into the simulated main memory (RAM).
*   **Execution Setup:** Modified the engine to support manually updating the PC to the newly loaded ELF's entry point via `set_pc`.
*   **UI Integration:** Added a "Load ELF" button, allowing developers to boot test binaries. The UI successfully intercepted and executed basic `ADDIU` instructions loaded from a custom ELF.

## Phase 9: BIOS High-Level Emulation (HLE) & Syscalls
*   **SYSCALL Interception:** The `SYSCALL` instruction is intercepted natively in the Rust `ee.rs` emulator, preventing an infinite loop in the BIOS exception vector.
*   **Syscall Dispatcher:** Created a `handle_syscall` method in `EmotionEngine` to process the syscall number in `$v1` and handle arguments (in `$a0` - `$a3`).
*   **Text Output:** Implemented `read_string` memory traversal to support `0x3D Putc` and `0x3E Puts` syscalls, logging output directly to the Tauri SIO console.
*   **Safe Fallback:** Unknown syscalls log a warning, return 0, and continue safely instead of crashing, maximizing early compatibility with homebrew!

## Phase 10: TBD
*   **Goal:** Awaiting decision on the next target (e.g., Graphics Synthesizer fundamentals, VU0/VU1 implementation, or expanded COP0 capabilities).
