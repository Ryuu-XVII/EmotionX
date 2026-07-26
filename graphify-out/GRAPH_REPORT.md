# Graph Report - .  (2026-07-27)

## Corpus Check
- cluster-only mode — file stats not available

## Summary
- 177 nodes · 187 edges · 18 communities (16 shown, 2 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS
- Token cost: 0 input · 0 output

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

## God Nodes (most connected - your core abstractions)
1. `compilerOptions` - 16 edges
2. `Bus` - 15 edges
3. `Instruction` - 11 edges
4. `EmotionEngine` - 9 edges
5. `EmulatorState` - 8 edges
6. `Hardware` - 6 edges
7. `run_cpu_batch()` - 6 edges
8. `compilerOptions` - 6 edges
9. `scripts` - 5 edges
10. `step_cpu()` - 5 edges

## Surprising Connections (you probably didn't know these)
- `EmulatorState` --references--> `EmotionEngine`  [EXTRACTED]
  src-tauri/src/lib.rs → src-tauri/src/cpu/ee.rs
- `EmotionEngine` --references--> `Bus`  [EXTRACTED]
  src-tauri/src/cpu/ee.rs → src-tauri/src/memory/bus.rs
- `Bus` --references--> `Hardware`  [EXTRACTED]
  src-tauri/src/memory/bus.rs → src-tauri/src/hw/mod.rs

## Import Cycles
- None detected.

## Communities (18 total, 2 thin omitted)

### Community 0 - "Bus"
Cohesion: 0.10
Nodes (8): EmotionEngine, Self, String, Hardware, Self, Bus, Self, Vec

### Community 1 - "compilerOptions"
Cohesion: 0.09
Nodes (22): DOM, DOM.Iterable, ES2020, src, compilerOptions, allowImportingTsExtensions, isolatedModules, jsx (+14 more)

### Community 2 - "devDependencies"
Cohesion: 0.10
Nodes (21): autoprefixer, devDependencies, autoprefixer, postcss, tailwindcss, @tailwindcss/vite, @tauri-apps/cli, @types/react (+13 more)

### Community 3 - "dependencies"
Cohesion: 0.13
Nodes (15): framer-motion, lucide-react, dependencies, framer-motion, lucide-react, react, react-dom, @tauri-apps/api (+7 more)

### Community 5 - "tauri.conf.json"
Cohesion: 0.14
Nodes (13): app, security, windows, build, beforeBuildCommand, beforeDevCommand, devUrl, frontendDist (+5 more)

### Community 6 - "lib.rs"
Cohesion: 0.32
Nodes (11): Mutex, Option, Result, boot_game(), EmulatorState, get_status(), String, Vec (+3 more)

### Community 7 - "package.json"
Cohesion: 0.20
Nodes (9): name, private, scripts, build, dev, preview, tauri, type (+1 more)

### Community 8 - "default.json"
Cohesion: 0.20
Nodes (9): core:default, dialog:default, main, opener:default, description, identifier, permissions, $schema (+1 more)

### Community 9 - "compilerOptions"
Cohesion: 0.22
Nodes (8): vite.config.ts, compilerOptions, allowSyntheticDefaultImports, composite, module, moduleResolution, skipLibCheck, include

### Community 10 - "icon"
Cohesion: 0.25
Nodes (8): icons/128x128@2x.png, icons/128x128.png, icons/32x32.png, icons/icon.ico, bundle, active, icon, targets

## Knowledge Gaps
- **74 isolated node(s):** `name`, `private`, `version`, `type`, `dev` (+69 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **2 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `Bus` connect `Bus` to `Instruction`, `lib.rs`?**
  _High betweenness centrality (0.051) - this node is a cross-community bridge._
- **Why does `devDependencies` connect `devDependencies` to `package.json`?**
  _High betweenness centrality (0.044) - this node is a cross-community bridge._
- **Why does `dependencies` connect `dependencies` to `package.json`?**
  _High betweenness centrality (0.034) - this node is a cross-community bridge._
- **What connects `name`, `private`, `version` to the rest of the system?**
  _74 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Bus` be split into smaller, more focused modules?**
  _Cohesion score 0.09538461538461539 - nodes in this community are weakly interconnected._
- **Should `compilerOptions` be split into smaller, more focused modules?**
  _Cohesion score 0.08695652173913043 - nodes in this community are weakly interconnected._
- **Should `devDependencies` be split into smaller, more focused modules?**
  _Cohesion score 0.09523809523809523 - nodes in this community are weakly interconnected._