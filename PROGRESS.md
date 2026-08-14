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

## Phase 10: CPU Correctness (FPU, Unaligned Memory Access, Branch-Likely)
*   **FPU (COP1):** Added the 32 single-precision floating-point registers (`fpr`) and status/control register (`fcr31`). Implemented `MFC1`/`MTC1`/`CFC1`/`CTC1` transfers, `LWC1`/`SWC1` memory access, `ADD.S`/`SUB.S`/`MUL.S`/`DIV.S`/`SQRT.S`/`ABS.S`/`MOV.S`/`NEG.S` arithmetic, `CVT.S.W`/`CVT.W.S` conversions, `C.EQ.S`/`C.LT.S`/`C.LE.S` comparisons, and `BC1F`/`BC1T`/`BC1FL`/`BC1TL` branches.
*   **Unaligned Memory Access:** Implemented `LWL`/`LWR`/`SWL`/`SWR` using the standard little-endian mask/shift tables, allowing compiler-generated unaligned word access (common in real PS2 binaries) to work correctly.
*   **Branch-Likely Instructions:** Implemented `BEQL`/`BNEL`/`BLEZL`/`BGTZL`/`BLTZL`/`BGEZL`. Added a `nullify` flag so that when a "likely" branch is *not* taken, its delay-slot instruction is correctly squashed (never executed) instead of running unconditionally like a normal branch's delay slot.
*   **Test Coverage:** Added unit tests for FPU arithmetic, branch-likely squashing, and unaligned load/store (including a hand-verified byte-level LWL case).

## Phase 11: DMA Controller & Graphics Synthesizer Fundamentals
*   **64-bit Memory Access:** Implemented `LD`/`SD` (MIPS III doubleword load/store) and `Bus::read64`/`write64`, needed for the EE to reach the GS's 64-bit privileged registers.
*   **DMA Controller (`hw/dmac.rs`):** Implemented all 10 DMAC channels (VIF0/1, GIF, IPU_FROM/TO, SIF0/1/2, SPR_FROM/TO) with CHCR/MADR/QWC/TADR registers and the global D_CTRL/D_STAT/D_PCR/etc. registers. Writing the STR (start) bit in a channel's CHCR triggers `Bus::execute_dma`, which supports both **normal mode** (flat QWC-quadword transfer) and **chain mode** (following DMAtags: REFE/CNT/NEXT/REF/REFS/END), matching how real PS2 software feeds data to hardware.
*   **Graphics Synthesizer (`hw/gs.rs`):** Implemented the privileged register file (PMODE, SMODE1/2, DISPFB1/2, DISPLAY1/2, BGCOLOR, CSR, IMR) at `0x12000000`, plus a PACKED-mode GIFtag parser that decodes `PRIM`, `RGBAQ`, and `XYZ2`/`XYZ3`/`XYZF3` register writes (including the common `A+D` address+data form) into a 640x448 software framebuffer. Every vertex currently draws as a single colored point — line/triangle/sprite rasterization and texturing are not implemented yet.
*   **End-to-End Pipeline:** The GIF DMAC channel (channel 2) is wired all the way through: EE writes a GIF packet to RAM → kicks the DMAC → DMAC streams it to the GS → GS parses registers and plots pixels, verified by a full round-trip unit test.
*   **Scope note:** REGLIST/IMAGE GIF transfer modes, VIF0/1, IPU, and SIF channels are register-complete but don't yet drive real backends — they accept writes and complete instantly so polling code doesn't hang, mirroring the DMAC's own instant-completion timing model.

## Phase 12: Real GS Rasterization & Live Display
*   **Primitive Rasterization:** The GS now tracks a per-`PRIM`-type vertex queue and rasterizes real primitives instead of just points: `POINT`, `LINE`/`LINE_STRIP` (Bresenham), `TRIANGLE`/`TRIANGLE_STRIP`/`TRIANGLE_FAN` (edge-function scanline fill), and `SPRITE` (axis-aligned filled rect). Strip/fan modes correctly slide their vertex window between draws. Shading is flat (last vertex's color) — texturing and Gouraud interpolation are still future work.
*   **Frontend Display:** Added a `get_framebuffer` Tauri command that converts the GS's internal framebuffer to RGBA8888 bytes, and a new "Display" tab in the React UI with an HTML canvas that polls and renders it live via `ImageData`/`putImageData`.
*   **Test Coverage:** Added a triangle-rasterization test that drives a full GIF packet (PRIM + RGBAQ + 3x XYZ2) through the real DMA path and asserts the correct pixels are filled and the correct pixels are left untouched.
*   **Verified live:** Launched the app via `npm run tauri dev` and confirmed the Display tab renders correctly. Also surfaced a pre-existing (not introduced by this phase) responsiveness issue: `run_cpu_batch`'s 50,000-step synchronous loop builds a `format!` log string per instruction on every 50ms UI tick, which can make the window briefly report "Not Responding" under `Run`. Worth a future perf pass.

## Phase 13: VU0 Macro Mode (COP2) — Scoped Subset
*   **Register file (`cpu/vu0.rs`):** 32 128-bit vector registers (`vf0`-`vf31`, `vf0` hardwired to `(0,0,0,1)`), 16 integer registers (`vi0`-`vi15`, `vi0` hardwired to 0), an accumulator (`ACC`), and a `Q` scalar.
*   **Implemented instructions:** register transfers `QMFC2`/`QMTC2`/`CFC2`/`CTC2`, memory transfers `LQC2`/`SQC2`, branch `BC2F`/`BC2T`/`BC2FL`/`BC2TL`, the broadcast FMAC family (`ADDbc`/`SUBbc`/`MAXbc`/`MINIbc`/`MULbc`/`MADDbc`/`MSUBbc`, writing to `fd` with dest-component masking), and the non-broadcast accumulate family (`ADDA`/`SUBA`/`MULA`/`MADDA`/`MSUBA`/`OPMULA`, writing `ACC`).
*   **Known gap — flagged, not silently guessed:** the combined broadcast+accumulate opcodes (`MULAbc`/`MADDAbc`/etc.) used by the canonical `vmulax`/`vmadday`/`vmaddaz`/`vmaddw` 4x4 matrix-transform idiom, and the full "lower" integer/branch instruction family (`VIADD`, `VMTIR`, `VLQI`/`VSQI`, etc.), are **not implemented**. Their exact macro-mode bit encoding could not be confirmed against an authoritative source in this environment (no local PS2SDK toolchain to test against, and the official EE Core Instruction Set / VU User's Manual PDFs weren't cleanly extractable) — unknown opcodes are logged and safely ignored rather than guessed and silently shipped as fact. Filling this in is the natural next step before real ps2sdk homebrew can do a full matrix transform through VU0.
*   **Test coverage:** `QMTC2`/`QMFC2` round-trip, and a broadcast-multiply instruction building one row of a matrix-vector transform.

## Phase 13.1: Boot-Hang Diagnosis — Two Real CPU Gaps Fixed
*   **Symptom:** the app appeared to "hang" running the real baked-in BIOS, spamming repeated `DEBUG INT` lines with `EXL=true` stuck forever and cause code `0x28` (Reserved Instruction), never recovering.
*   **Root cause 1 — `LDR` (opcode `0x1B`) unimplemented:** found by instrumenting a standalone trace binary to run millions of steps and flag any `UNKNOWN OPCODE` hit. The BIOS's doubleword-unaligned-load routine (the 64-bit sibling of the `LWL`/`LWR` pair added in Phase 10) triggered an immediate Reserved Instruction exception at step ~3.2M, and because our exception handling has no real nested-exception recovery, the CPU could never get out of the resulting handler loop. **Fixed:** implemented `LDL`/`LDR`/`SDL`/`SDR` (64-bit unaligned load/store) using the same PCSX2-derived mask/shift-table technique as the existing 32-bit versions, generalized to 8-byte width and cross-checked by hand against a worked example (see `test_ldl_unaligned`).
*   **Root cause 2 — `LL`/`SC` (Load Linked / Store Conditional, opcodes `0x30`/`0x38`) unimplemented:** the same trace method found a second Reserved Instruction hit on `SC`, a MIPS II atomic/synchronization primitive the BIOS uses during low-level init. **Fixed:** implemented `LL`/`SC`/`LLD`/`SCD` using the standard single-core simplification (no other core can invalidate the reservation, so the store always succeeds and `rt` is unconditionally set to 1) — correct behavior for a single-CPU interpreter, though it doesn't model real inter-core contention.
*   **Verified further boot progress:** after both fixes, the BIOS runs past the original 3.2M-step stall and executes tens of millions of additional, non-repeating instructions (previously impossible). It eventually calls into a RAM region (`0x002C8020`) that's still all zeros and derails into it, executing implicit NOPs forever. This is **not a new bug** — that code would, on real hardware, have been loaded off the disc by the IOP, which this emulator doesn't emulate yet. This is exactly the Milestone 2 boundary (ISO9660/IOP/SIF, Phases 17-20 in the roadmap): the BIOS legitimately cannot progress further without disc/IOP support.
*   **New tests:** `test_ldl_unaligned` (hand-verified byte-level case) and `test_ldr_sdr_roundtrip_aligned`.

## Phase 17: ISO9660 Reader — `boot_game` Actually Boots a Disc Image
*   **`iso9660.rs`:** a minimal ISO9660 reader (standard 2048-byte-sector `.iso` images) — parses the Primary Volume Descriptor, walks directory records to resolve `\`/`/`-separated paths (case-insensitive, `;N` version suffix tolerant), and reads full file contents.
*   **`SYSTEM.CNF` parsing:** extracts the `BOOT2=cdrom0:\...` entry that every real PS2 disc uses to name its boot executable.
*   **`boot_game` rewritten:** previously a no-op that ignored the selected file entirely and just reset to the BIOS. It now actually opens the ISO, reads `SYSTEM.CNF`, resolves the `BOOT2` path, extracts that ELF's bytes, and loads them directly into EE RAM via the same path `load_elf` uses (`elf_loader::load_elf_bytes`, factored out for this reuse).
*   **Scope note — this is the "fast boot" shortcut from the roadmap, not full hardware accuracy:** it skips the IOP entirely and loads the boot ELF directly, the way a real console's IOP would only *after* reading it off the disc and handing control to the EE. This gets a real disc image's EE-side code running without needing IOP/CDVD emulation, but any code that itself makes further disc reads or IOP RPC calls (nearly all real retail games, for streaming assets/pad/sound) will still stall once it gets there — that's Phase 18+.
*   **Test coverage:** `SYSTEM.CNF` parsing, and a full synthetic-ISO round-trip test that hand-builds a structurally real ISO9660 image (PVD + root directory + two files) and verifies both file resolution and byte-exact extraction — the highest-risk part of this feature, since no real PS2 disc image was available to test against in this environment.

## Phase 17.1: Native CHD (Compressed Disc Image) Support
*   **Why:** most real-world PS2 rips are distributed as `.chd` (MAME's compressed disc image format), not raw `.iso` — confirmed directly when a user's real disc image failed to open. CHD v5's hunk map is itself a bespoke compressed/Huffman-coded structure; rather than hand-reimplement that blind from paraphrased documentation (a real risk of a subtly-corrupt decoder with no reference file to validate against), pulled in the purpose-built [`chd` crate](https://crates.io/crates/chd) (a from-scratch, `chd.cpp`-verified pure-Rust implementation) as a dependency.
*   **`chd_source.rs`:** a `DiscSource` adapter (see below) over the `chd` crate exposing a flat, randomly-addressable, 2048-byte-sector-aligned byte stream, with single-hunk decompression caching for sequential reads.
*   **`iso9660.rs` refactored** to read through a new `DiscSource` trait instead of owning a `File` directly, so the exact same directory-walking/file-extraction logic works unmodified over either a raw `.iso` (implemented directly on `File`) or a CHD-backed image. `Iso9660::open` auto-detects which by probing for the `MComprHD` magic.
*   **CD-sector-layout surprise, resolved empirically:** many PS2 CHD rips store raw CD-style sectors (`unit_bytes` 2352/2448) rather than flat 2048-byte DVD sectors, even for a DVD PS2 title — the ripping tool imaged it as a CD track. Verified against the user's real ~3GB CHD by brute-force scanning decompressed hunk bytes for the ISO9660 `CD001` signature: it landed at unit-local offset 1 with **zero** prefix skip, meaning (unlike a literal raw MODE1 CD dump) this format stores the 2048 bytes of user data at the very start of each unit with no 16-byte sync/header prefix. Implemented and confirmed end-to-end: opened the real CHD, correctly parsed its `SYSTEM.CNF` (`BOOT2 = cdrom0:\SLUS_213.51;1`), and extracted a byte-valid ELF (correct `0x7F 'E' 'L' 'F'` header) for the boot executable.
*   **Scope note:** the CD-sector offset-0 layout was confirmed against this specific rip; other CHDs that store a literal raw sync+header CD dump would need the offset made configurable/auto-detected rather than hardcoded, if one is encountered that doesn't match.

## Phase 18: MMI (MultiMedia Instructions / SIMD)
*   **Why:** booting the real NFS Most Wanted CHD (Phase 17.1) revealed the actual blocker for retail games specifically: the **very first instruction** at the game's ELF entry point is an MMI instruction (`PADDUW`). Real retail games use MMI — the EE's SIMD extension operating on the 128-bit GPRs as packed vectors — from their first instructions (compiler-generated `memcpy`/vector math), unlike BIOS/homebrew which mostly avoid it. This is a bigger, harder-blocking gap than VU0 was.
*   **`cpu/mmi.rs`:** pure lane-manipulation functions for ~34 instructions, cross-checked against PCSX2's `R5900OpcodeTables.cpp` (the top-level SPECIAL2 dispatch table plus the MMI0/MMI1/MMI2/MMI3 sub-tables, fetched and verified verbatim rather than paraphrased, given the size of the surface area).
    *   **High confidence** (standard, unambiguous elementwise SIMD semantics): `PADDW`/`PSUBW`/`PADDH`/`PSUBH`/`PADDB`/`PSUBB` (wrapping), the `S`-suffixed signed-saturating and `U`-suffixed unsigned-saturating variants, `PCGTW`/`PCGTH`/`PCGTB`/`PCEQW`/`PCEQH`/`PCEQB` (compare-to-mask), `PMAXW`/`PMINW`/`PMAXH`/`PMINH`, `PABSW`/`PABSH`, `PAND`/`POR`/`PXOR`/`PNOR`, `PLZCW`.
    *   **Best-effort** (lower confidence on exact operand-to-lane mapping, no reference implementation available to verify against): the extend/pack/copy shuffle family — `PEXTLW`/`PEXTUW`/`PEXTLH`/`PEXTUH`/`PEXTLB`/`PEXTUB`, `PPACW`/`PPACH`/`PPACB`, `PCPYLD`/`PCPYUD`/`PCPYH`.
*   **Verified fix:** the game's boot code now runs 30+ instructions past its previous immediate block (up from failing on instruction #0), confirmed via `PADDUW` executing correctly through the full CPU decode path (`test_mmi_padduw_via_decode`) and via a standalone trace against the real CHD.
*   **New gaps surfaced (not yet implemented) by getting further into real game code:**
    *   `MTHI1`/`MTLO1` and the rest of the "pipeline 1" multiply/divide family (`MULT1`/`DIV1`/`MADD1`/etc.) — the EE has a *second*, separate HI/LO register pair for these; not modeled yet.
    *   `MTSAH`/`MTSAB` (REGIMM-space EE-specific instructions that set the "SA" shift-amount register used by `QFSRV` for unaligned 128-bit shifts).
    *   At least one uncommon COP1 `S`-format funct value not in the Phase 10 FPU subset.
*   **Added as a trivial, safe fix:** `SYNC` (memory-ordering barrier) as a no-op — correct behavior for a single-threaded interpreter regardless.
*   **Honest status:** after these fixes, the game still ultimately derails into unpopulated memory over millions of steps (the same "PC climbs through zeroed RAM" symptom seen with the BIOS), and since no *further* `UNKNOWN` opcode hits occur before that happens, the remaining cause is most likely either the deferred HI1/LO1 pipeline, the SA-register mechanism, or an inaccuracy in one of the "best-effort" shuffle instructions above producing silently-wrong (rather than loudly-unimplemented) data. This is the next thing to instrument and pin down.

## Phase 19: Full Instruction-Set Audit
*   **Method:** systematically read every dispatch table in `ee.rs` (top-level opcode, SPECIAL, REGIMM, COP0, COP1, COP2, MMI) and cross-referenced against the complete standard MIPS III/IV + EE-specific opcode space, using the same verified sources established earlier this session (PCSX2's opcode tables, standard MIPS coprocessor-load numbering).
*   **Fixed (confirmed real gaps, all with new tests):**
    *   `DADDI`/`DADDIU` (top-level `0x18`/`0x19`) — 64-bit immediate add. High-impact: the EE is a genuine 64-bit CPU, and pointer/address arithmetic in real compiled code uses these constantly.
    *   `LWU` (`0x27`) — zero-extended word load (as opposed to `LW`'s sign extension).
    *   `PREF` (`0x33`) — prefetch hint. Previously would trigger a Reserved Instruction exception for something that's supposed to be a pure no-op hint — a real correctness bug, not just a missing feature.
    *   `BLTZAL`/`BGEZAL`/`BLTZALL`/`BGEZALL` (REGIMM `0x10`-`0x13`) — "branch and link" variants.
    *   `MTSAB`/`MTSAH` (REGIMM `0x18`/`0x19`) — the exact instruction confirmed blocking the real game trace in Phase 18. Added a new `sa` register field they write to.
    *   `SYNC` (SPECIAL `0x0F`) — memory-ordering barrier, no-op for our single-threaded interpreter (added in Phase 18 already, included here for completeness of the audit record).
*   **Verified against the real game again:** the confirmed `MTSAH` gap no longer appears in the trace. Only 3 `UNKNOWN` hits remain in 15M steps (down from a pervasive early block), none of them new discoveries from this pass — meaning the systematic sweep didn't miss anything else that's actively firing on this particular code path.
*   **Documented, deferred gaps** (lower priority — not hit by real code yet, or inherently rare):
    *   `MTHI1`/`MTLO1` and the rest of the MULT1/DIV1/MADD1 "pipeline 1" family (needs a second HI1/LO1 register pair) — still the top candidate for the remaining real-game derailment cause, since it's the largest chunk of the 3 remaining `UNKNOWN` hits.
    *   One EE-specific COP1 `S`-format funct (`0x18`, likely `ADDA.S` — an FPU accumulator extension) not covered by the Phase 10 FPU subset.
    *   `QFSRV` (MMI1 index 27) — the funnel-shift instruction `MTSAH`/`MTSAB` exist to feed; not implemented since its exact 256-bit-shift semantics weren't independently verified.
    *   COP0 TLB instructions (`TLBR`/`TLBWI`/`TLBWR`/`TLBP`) — relevant for full BIOS/kernel boot, not yet hit by game-code execution.
    *   `MOVCI` (SPECIAL `0x01`, FP-conditional move) and the trap family (`TEQ`/`TNE`/`TGE`/`TGEU`/`TLT`/`TLTU` under SPECIAL, `TEQI`/etc. under REGIMM) — used by some compiler-generated bounds-checks/assertions, not yet observed blocking anything.
    *   `LDC1`/`SDC1`/`LDC2`/`SDC2` — double-precision float and secondary COP2 memory transfer opcodes; likely genuinely unused since the EE FPU is single-precision only and VU0 already has `LQC2`/`SQC2`.
    *   `BREAK` (SPECIAL `0x0D`) — software breakpoint, rare in shipped code.

## Phase 20: HI1/LO1 Pipeline & FPU Accumulator — CPU Instruction Coverage Reaches Zero Unknown Opcodes
*   **HI1/LO1 pipeline (`cpu/ee.rs`):** added the EE's second, independent HI/LO register pair and implemented `MFHI1`/`MTHI1`/`MFLO1`/`MTLO1`, `MULT1`/`MULTU1`, `DIV1`/`DIVU1`, `MADD1`/`MADDU1` — the exact family flagged as the top suspect in the Phase 19 audit. Also filled in the plain (non-pipeline-1) `MADD`/`MADDU` that the audit found were missing from the MMI/SPECIAL2 dispatch entirely.
*   **FPU accumulator extensions:** added the EE-specific `facc` accumulator register and `RSQRT.S`/`ADDA.S`/`SUBA.S`/`MULA.S`/`MADD.S`/`MSUB.S`/`MADDA.S`/`MSUBA.S`/`MAX.S`/`MIN.S`, verified against source (`ps2tek`) rather than guessed — notably, the previously-guessed identity of funct `0x18` (assumed `ADDA.S` in the Phase 18 write-up) was **wrong**; verification showed it's actually `MAX.S`, which was the exact instruction the real game was hitting.
*   **Result: zero `UNKNOWN` opcode hits across 20 million instructions** of the real NFS Most Wanted boot trace — full instruction coverage for everything this game's boot path actually executes, up from a hard block on instruction #0 at the start of this multi-phase effort.
*   **The remaining derailment is very likely the legitimate Milestone 2 (IOP/SIF) wall, not a CPU bug.** Traced the exact transition point: right before `PC` leaves valid RAM, the code performs a textbook-correct function call (proper `$ra`/callee-saved-register stack save and restore, no corruption visible) that itself does a hash/handle-table lookup — computing an index, checking two tables for a null entry, branching on the result. That's the classic shape of looking up a dynamically-registered module or RPC service, exactly what would come back empty since this emulator never runs the IOP side of boot (no SIF, no CDVD, no IOP modules). Consistent hitting of the same derailment region across multiple independent fix-rounds (rather than drifting as bugs got fixed) supports this being a stable, deterministic wall rather than an accumulating computation error.
*   **Test coverage:** `MULT1`/`MFHI1`/`MFLO1` (exercises the >32-bit product path specifically, to catch HI1/LO1 confusion with the regular pipeline), `MADD` accumulation, `MAX.S`/`MADDA.S`.

## Phase 21: SIF RPC Bind HLE & Kernel Semaphores — Real Infrastructure, But Didn't Fix the Observed Stall
*   **Researched and implemented against verified ps2sdk source** (not guessed): the `SifCmdHeader_t`/`SifRpcBindPkt_t`/`SifRpcClientData_t` struct layouts and the `SIF_CMD_RPC_BIND` (`0x80000009`) command ID, fetched directly from `ps2dev/ps2sdk`.
*   **`Bus::handle_sif1_packet`:** intercepts DMAC-channel-SIF1 traffic; when a packet's `cid` matches `SIF_CMD_RPC_BIND`, writes a fake-but-non-null `server` pointer directly into the client structure the game specified (offset 36 within `SifRpcClientData_t`), synthesizing an immediate "bind succeeded" response without needing any real IOP-side processing. This only unblocks the *bind* handshake — no real pad/sound/CD module behavior exists behind it yet.
*   **Kernel semaphore syscalls** (`CreateSema`/`DeleteSema`/`SignalSema`/`WaitSema`/`PollSema`/`ReferSemaStatus`, numbers verified against ps2sdk's `syscallnr.h`): since there's no real thread scheduler, every one of these succeeds immediately rather than actually blocking — the safe simplification consistent with this project's established approach elsewhere, and specifically necessary because `sceSifBindRpc`'s synchronous mode waits on a semaphore that only a real (or HLE'd) IOP response would normally signal.
*   **Honest result: this did *not* unblock the derailment point identified in Phase 20.** Re-ran the same real-game trace — it leaves valid RAM at the exact same step and PC as before the fix. That means the earlier hypothesis (that the observed hash/handle-table polling loop was an `sceSifBindRpc`-style check) was **wrong**, or at minimum incomplete: the game hadn't even triggered a SIF1 DMA transfer by the time it derails, so this infrastructure never had a chance to act. The real cause of that specific stall remains unidentified.
*   **Value delivered anyway:** the SIF1 bind HLE and semaphore syscalls are correct, tested, protocol-level infrastructure that real games will need regardless, once whatever the *actual* blocker turns out to be is resolved and execution reaches an actual IOP bind call. Not wasted work, just not the fix for this specific symptom.
*   **Test coverage:** a full DMAC-driven SIF1 bind packet round-trip (`test_sif1_bind_hle_unblocks_client_struct`), and semaphore non-blocking behavior (`test_semaphore_syscalls_never_block`).

## Phase 22: Runaway-Execution Detection
*   **Why:** confirmed via the actual running app (not just the standalone trace) that a real disc image now genuinely boots — the console showed `Booted 'SLUS_213.51;1' from ISO, entry point 0x00436128`, matching the trace exactly. But once execution reaches the still-unresolved Phase 20/21 derailment, the CPU runs off into unmapped memory and executes implicit NOPs forever, flooding the UI log with useless output indefinitely rather than failing visibly.
*   **`Bus::is_code_mapped`:** checks whether an address is backed by real RAM or BIOS (the only places code can legitimately live), as opposed to unmapped space that silently reads back as all-zero (which decodes as `NOP`).
*   **`EmotionEngine::consecutive_unmapped_fetches`:** increments on every instruction fetch from unmapped memory, resets to 0 the moment execution returns to real code.
*   **`run_cpu_batch` now stops early** once this counter crosses a threshold (256 consecutive unmapped fetches), returning a clear error - `"Execution halted: PC ran off into unmapped memory (...) and would spin forever..."` - instead of silently spinning. The existing frontend error handling (`catch` → `setIsRunning(false)` + log) means this now auto-pauses the Run loop with a real explanation instead of an endless NOP flood.
*   **Test coverage:** verifies the counter increments while fetching from unmapped memory and resets immediately once execution returns to mapped RAM.
*   **Note:** this is a diagnostic/UX safety net, not a fix for the underlying Phase 20/21 stall - that root cause is still unidentified as of this phase.

## Phase 23: Root Cause Found and Fixed — 100 Million Instructions, Zero Derailment
*   **The actual root cause of the Phase 20/21 derailment, finally identified:** `$sp` (register 29, the stack pointer) is never valid at the point the game first uses it. The game's own crt0 startup does a defensive "clear every GPR to zero" pass (using the MMI `PADDUW $vN, $v0, $v0` idiom — 128-bit `$vN = 0 + 0`) very early in boot, which wipes out any value pre-set before jumping to the entry point (including the Phase 21 attempt to set `$sp = 0x81FFFFF0` beforehand — that fix was correct in spirit but got clobbered moments later). From then on, `$sp` stays at literal 0, and every subsequent stack-frame allocation (`ADDIU sp, sp, -N`) produces a small negative-wrapped address like `0xFFFFFFF0`.
*   **Why that's fatal:** physical-address masking (`addr & 0x1FFFFFFF`) puts any small negative wrap from zero at the very top of the 29-bit address window — which is exactly where the BIOS ROM is mapped (`0x1FC00000`–`0x20000000`). This is architecturally guaranteed, not a coincidence. So every stack save (`SD`) silently writes nowhere (BIOS is read-only) and every restore (`LD`) reads back garbage BIOS bytes instead. Traced this precisely: watched `$ra` and the exact stack memory address across the full run, confirmed a clean, correctly-saved return address (`0x001622CC`) got read back as pure garbage (`0xA40FA37CA407837C`) with no intervening write to that address — the write had simply gone nowhere.
*   **Fix — `EmotionEngine::guard_sp_zero`:** real PS2 kernels guarantee `$sp` is never zero when user code runs (it's always pre-established by the kernel/loader, which this emulator's fast-boot shortcut skips). A targeted compatibility shim in `set_reg`/`set_reg128` intercepts any write of exactly `0` to register 29 specifically and substitutes a sane stack-top value instead — emulating that real-hardware invariant without needing to reverse-engineer the exact kernel mechanism that provides it. Every other register is unaffected; zero is a perfectly normal value everywhere else.
*   **Verified: the real NFS Most Wanted CHD now runs 100 million instructions with zero unknown-opcode hits and zero derailment** (previously died at instruction ~758,000 every time, across many rounds of unrelated fixes). PC settles into a small, stable, valid address range — the game is now looping (most likely waiting on a VBlank interrupt or an IOP/SIF response this emulator doesn't yet provide) instead of crashing into unmapped memory.
*   **Test coverage:** `test_sp_never_allowed_to_become_zero` covers both the 64-bit (`set_reg`, e.g. `ADDIU`) and 128-bit (`set_reg128`, e.g. the MMI clear idiom) write paths, and confirms no other register is affected by the guard.

## Phase 24: TBD
*   **Goal:** the game now runs stably in a loop instead of crashing — next is figuring out what it's waiting on (VBlank timing? An IOP/SIF response?) to make real forward progress toward rendering. Other candidates: GS texturing, `QFSRV`, completing the VU0 broadcast+accumulate opcode family.
