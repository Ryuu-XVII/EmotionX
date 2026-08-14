# Graph Report - EmotionX  (2026-08-14)

## Corpus Check
- 32 files · ~31,573 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 422 nodes · 728 edges · 26 communities (23 shown, 3 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `b90d06d5`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- Bus
- compilerOptions
- devDependencies
- dependencies
- Instruction
- tauri.conf.json
- lib.rs
- package.json
- default.json
- compilerOptions
- icon
- App.tsx
- EmotionX - Progress Tracker
- Sio
- 🎮 EmotionX
- Dmac
- Sio
- load_elf
- iso9660.rs
- load_elf

## God Nodes (most connected - your core abstractions)
1. `Bus` - 24 edges
2. `EmotionX - Progress Tracker` - 24 edges
3. `EmotionEngine` - 23 edges
4. `from_lanes32()` - 21 edges
5. `lanes32()` - 17 edges
6. `lanes16()` - 16 edges
7. `from_lanes16()` - 16 edges
8. `compilerOptions` - 16 edges
9. `Instruction` - 15 edges
10. `Gs` - 15 edges

## Surprising Connections (you probably didn't know these)
- `EmotionEngine` --references--> `Vu0`  [EXTRACTED]
  src-tauri/src/cpu/ee.rs → src-tauri/src/cpu/vu0.rs
- `EmotionEngine` --references--> `Bus`  [EXTRACTED]
  src-tauri/src/cpu/ee.rs → src-tauri/src/memory/bus.rs
- `load_elf()` --references--> `EmotionEngine`  [EXTRACTED]
  src-tauri/src/elf_loader.rs → src-tauri/src/cpu/ee.rs
- `load_elf_bytes()` --references--> `EmotionEngine`  [EXTRACTED]
  src-tauri/src/elf_loader.rs → src-tauri/src/cpu/ee.rs
- `EmulatorState` --references--> `EmotionEngine`  [EXTRACTED]
  src-tauri/src/lib.rs → src-tauri/src/cpu/ee.rs

## Import Cycles
- 1-file cycle: `src-tauri/src/iso9660.rs -> src-tauri/src/iso9660.rs`

## Communities (26 total, 3 thin omitted)

### Community 0 - "Bus"
Cohesion: 0.08
Nodes (7): Hardware, Option, Self, Bus, Self, String, Vec

### Community 1 - "compilerOptions"
Cohesion: 0.09
Nodes (22): DOM, DOM.Iterable, ES2020, src, compilerOptions, allowImportingTsExtensions, isolatedModules, jsx (+14 more)

### Community 2 - "devDependencies"
Cohesion: 0.10
Nodes (21): autoprefixer, devDependencies, autoprefixer, postcss, tailwindcss, @tailwindcss/vite, @tauri-apps/cli, @types/react (+13 more)

### Community 3 - "dependencies"
Cohesion: 0.08
Nodes (24): framer-motion, lucide-react, dependencies, framer-motion, lucide-react, react, react-dom, @tauri-apps/api (+16 more)

### Community 5 - "tauri.conf.json"
Cohesion: 0.09
Nodes (22): icons/128x128@2x.png, icons/128x128.png, icons/32x32.png, icons/icon.icns, icons/icon.ico, app, security, windows (+14 more)

### Community 6 - "lib.rs"
Cohesion: 0.32
Nodes (14): AppHandle, Mutex, boot_game(), EmulatorState, get_framebuffer(), get_status(), load_elf(), Option (+6 more)

### Community 7 - "package.json"
Cohesion: 0.08
Nodes (58): from_lanes16(), from_lanes32(), from_lanes64(), from_lanes8(), lanes16(), lanes32(), lanes64(), lanes8() (+50 more)

### Community 8 - "default.json"
Cohesion: 0.20
Nodes (9): core:default, dialog:default, main, opener:default, description, identifier, permissions, $schema (+1 more)

### Community 9 - "compilerOptions"
Cohesion: 0.22
Nodes (8): vite.config.ts, compilerOptions, allowSyntheticDefaultImports, composite, module, moduleResolution, skipLibCheck, include

### Community 10 - "icon"
Cohesion: 0.15
Nodes (28): Cop0, Self, EmotionEngine, Self, String, test_beql_not_taken_squashes_delay_slot(), test_bgezal_sets_return_address(), test_consecutive_unmapped_fetches_tracks_and_resets() (+20 more)

### Community 18 - "EmotionX - Progress Tracker"
Cohesion: 0.08
Nodes (24): EmotionX - Progress Tracker, Phase 10: CPU Correctness (FPU, Unaligned Memory Access, Branch-Likely), Phase 11: DMA Controller & Graphics Synthesizer Fundamentals, Phase 12: Real GS Rasterization & Live Display, Phase 13.1: Boot-Hang Diagnosis — Two Real CPU Gaps Fixed, Phase 13: VU0 Macro Mode (COP2) — Scoped Subset, Phase 17.1: Native CHD (Compressed Disc Image) Support, Phase 17: ISO9660 Reader — `boot_game` Actually Boots a Disc Image (+16 more)

### Community 19 - "Sio"
Cohesion: 0.24
Nodes (4): Self, String, Vec, Sio

### Community 20 - "🎮 EmotionX"
Cohesion: 0.22
Nodes (8): 🏗️ Architecture, Building & Running, 🎮 EmotionX, ✨ Features, 🛠️ Getting Started, 📝 License, Prerequisites, 🚀 Progress & Current Status (Phase 23)

### Community 21 - "Dmac"
Cohesion: 0.27
Nodes (4): Channel, Dmac, Option, Self

### Community 22 - "Sio"
Cohesion: 0.24
Nodes (4): Gs, Self, Vec, Vertex

### Community 24 - "iso9660.rs"
Cohesion: 0.11
Nodes (25): Box, BufReader, Chd, Path, ChdSource, Option, Result, Self (+17 more)

### Community 25 - "load_elf"
Cohesion: 0.60
Nodes (5): load_elf(), load_elf_bytes(), Result, String, test_load_elf()

## Knowledge Gaps
- **104 isolated node(s):** `name`, `private`, `version`, `type`, `dev` (+99 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **3 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `Bus` connect `Bus` to `load_elf`, `icon`, `lib.rs`?**
  _High betweenness centrality (0.085) - this node is a cross-community bridge._
- **Why does `Hardware` connect `Bus` to `Sio`, `Dmac`, `Sio`?**
  _High betweenness centrality (0.069) - this node is a cross-community bridge._
- **Why does `EmotionEngine` connect `icon` to `Bus`, `load_elf`, `lib.rs`, `load_elf`?**
  _High betweenness centrality (0.039) - this node is a cross-community bridge._
- **What connects `name`, `private`, `version` to the rest of the system?**
  _104 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Bus` be split into smaller, more focused modules?**
  _Cohesion score 0.08143939393939394 - nodes in this community are weakly interconnected._
- **Should `compilerOptions` be split into smaller, more focused modules?**
  _Cohesion score 0.08695652173913043 - nodes in this community are weakly interconnected._
- **Should `devDependencies` be split into smaller, more focused modules?**
  _Cohesion score 0.09523809523809523 - nodes in this community are weakly interconnected._