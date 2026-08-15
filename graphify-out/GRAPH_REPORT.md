# Graph Report - .  (2026-08-15)

## Corpus Check
- 57 files · ~60,849 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 528 nodes · 885 edges · 42 communities (38 shown, 4 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS
- Token cost: 0 input · 0 output

## Community Hubs (Navigation)
- [[_COMMUNITY_Community 0|Community 0]]
- [[_COMMUNITY_Community 1|Community 1]]
- [[_COMMUNITY_Community 2|Community 2]]
- [[_COMMUNITY_Community 3|Community 3]]
- [[_COMMUNITY_Community 4|Community 4]]
- [[_COMMUNITY_Community 5|Community 5]]
- [[_COMMUNITY_Community 6|Community 6]]
- [[_COMMUNITY_Community 7|Community 7]]
- [[_COMMUNITY_Community 8|Community 8]]
- [[_COMMUNITY_Community 9|Community 9]]
- [[_COMMUNITY_Community 10|Community 10]]
- [[_COMMUNITY_Community 11|Community 11]]
- [[_COMMUNITY_Community 12|Community 12]]
- [[_COMMUNITY_Community 13|Community 13]]
- [[_COMMUNITY_Community 14|Community 14]]
- [[_COMMUNITY_Community 15|Community 15]]
- [[_COMMUNITY_Community 16|Community 16]]
- [[_COMMUNITY_Community 17|Community 17]]
- [[_COMMUNITY_Community 18|Community 18]]
- [[_COMMUNITY_Community 19|Community 19]]
- [[_COMMUNITY_Community 20|Community 20]]
- [[_COMMUNITY_Community 21|Community 21]]
- [[_COMMUNITY_Community 22|Community 22]]
- [[_COMMUNITY_Community 23|Community 23]]
- [[_COMMUNITY_Community 24|Community 24]]
- [[_COMMUNITY_Community 25|Community 25]]
- [[_COMMUNITY_Community 26|Community 26]]
- [[_COMMUNITY_Community 27|Community 27]]
- [[_COMMUNITY_Community 28|Community 28]]
- [[_COMMUNITY_Community 29|Community 29]]
- [[_COMMUNITY_Community 30|Community 30]]
- [[_COMMUNITY_Community 31|Community 31]]
- [[_COMMUNITY_Community 32|Community 32]]
- [[_COMMUNITY_Community 33|Community 33]]
- [[_COMMUNITY_Community 34|Community 34]]
- [[_COMMUNITY_Community 35|Community 35]]

## God Nodes (most connected - your core abstractions)
1. `Bus` - 29 edges
2. `EmotionEngine` - 23 edges
3. `from_lanes32()` - 21 edges
4. `lanes32()` - 17 edges
5. `Gs` - 17 edges
6. `lanes16()` - 16 edges
7. `from_lanes16()` - 16 edges
8. `compilerOptions` - 16 edges
9. `Instruction` - 15 edges
10. `Hardware` - 15 edges

## Surprising Connections (you probably didn't know these)
- `EmotionEngine` --references--> `Cop0`  [EXTRACTED]
  src-tauri/src/cpu/ee.rs → src-tauri/src/cpu/cop0.rs
- `EmotionEngine` --references--> `Vu0`  [EXTRACTED]
  src-tauri/src/cpu/ee.rs → src-tauri/src/cpu/vu0.rs
- `EmotionEngine` --references--> `Bus`  [EXTRACTED]
  src-tauri/src/cpu/ee.rs → src-tauri/src/memory/bus.rs
- `load_elf()` --references--> `EmotionEngine`  [EXTRACTED]
  src-tauri/src/elf_loader.rs → src-tauri/src/cpu/ee.rs
- `load_elf_bytes()` --references--> `EmotionEngine`  [EXTRACTED]
  src-tauri/src/elf_loader.rs → src-tauri/src/cpu/ee.rs

## Import Cycles
- 1-file cycle: `src-tauri/src/iso9660.rs -> src-tauri/src/iso9660.rs`

## Communities (42 total, 4 thin omitted)

### Community 0 - "Community 0"
Cohesion: 0.08
Nodes (60): from_lanes16(), from_lanes32(), from_lanes64(), from_lanes8(), lanes16(), lanes32(), lanes64(), lanes8() (+52 more)

### Community 1 - "Community 1"
Cohesion: 0.14
Nodes (37): EmotionEngine, Self, String, test_beql_not_taken_squashes_delay_slot(), test_bgezal_sets_return_address(), test_consecutive_unmapped_fetches_tracks_and_resets(), test_daddiu_lwu_pref(), test_dma_gif_image_upload_and_textured_sprite() (+29 more)

### Community 2 - "Community 2"
Cohesion: 0.11
Nodes (26): Box, BufReader, Chd, Path, Send, ChdSource, Option, Result (+18 more)

### Community 3 - "Community 3"
Cohesion: 0.06
Nodes (14): Channel, Dmac, Option, Self, Hardware, Option, Self, Self (+6 more)

### Community 4 - "Community 4"
Cohesion: 0.11
Nodes (10): load_elf(), load_elf_bytes(), Result, String, test_load_elf(), Bus, Option, Self (+2 more)

### Community 5 - "Community 5"
Cohesion: 0.07
Nodes (28): dependencies, framer-motion, lucide-react, react, react-dom, @tauri-apps/api, @tauri-apps/plugin-dialog, @tauri-apps/plugin-opener (+20 more)

### Community 6 - "Community 6"
Cohesion: 0.11
Nodes (18): compilerOptions, allowImportingTsExtensions, isolatedModules, jsx, lib, module, moduleResolution, noEmit (+10 more)

### Community 7 - "Community 7"
Cohesion: 0.22
Nodes (4): Gs, Self, Vec, Vertex

### Community 8 - "Community 8"
Cohesion: 0.11
Nodes (17): app, security, windows, build, beforeBuildCommand, beforeDevCommand, devUrl, frontendDist (+9 more)

### Community 9 - "Community 9"
Cohesion: 0.28
Nodes (15): AppHandle, Mutex, Response, boot_game(), EmulatorState, get_framebuffer(), get_status(), load_elf() (+7 more)

### Community 10 - "Community 10"
Cohesion: 0.13
Nodes (14): anyOf, anyOf, description, definitions, Application, Target, Value, description (+6 more)

### Community 11 - "Community 11"
Cohesion: 0.13
Nodes (14): anyOf, anyOf, description, definitions, Application, Target, Value, description (+6 more)

### Community 13 - "Community 13"
Cohesion: 0.20
Nodes (10): $ref, description, items, type, uniqueItems, description, items, type (+2 more)

### Community 14 - "Community 14"
Cohesion: 0.20
Nodes (10): type, webviews, windows, items, description, items, type, description (+2 more)

### Community 15 - "Community 15"
Cohesion: 0.20
Nodes (10): $ref, description, items, type, uniqueItems, description, items, type (+2 more)

### Community 16 - "Community 16"
Cohesion: 0.20
Nodes (10): type, webviews, windows, items, description, items, type, description (+2 more)

### Community 17 - "Community 17"
Cohesion: 0.22
Nodes (9): properties, Identifier, description, oneOf, type, identifier, remote, anyOf (+1 more)

### Community 18 - "Community 18"
Cohesion: 0.22
Nodes (9): properties, Identifier, description, oneOf, type, identifier, remote, anyOf (+1 more)

### Community 19 - "Community 19"
Cohesion: 0.25
Nodes (8): description, properties, required, type, CapabilityRemote, urls, description, type

### Community 20 - "Community 20"
Cohesion: 0.25
Nodes (8): description, properties, required, type, CapabilityRemote, urls, description, type

### Community 22 - "Community 22"
Cohesion: 0.25
Nodes (7): compilerOptions, allowSyntheticDefaultImports, composite, module, moduleResolution, skipLibCheck, include

### Community 24 - "Community 24"
Cohesion: 0.33
Nodes (5): description, identifier, permissions, $schema, windows

### Community 26 - "Community 26"
Cohesion: 0.50
Nodes (4): description, required, type, Capability

### Community 27 - "Community 27"
Cohesion: 0.50
Nodes (4): default, description, type, description

### Community 28 - "Community 28"
Cohesion: 0.50
Nodes (4): default, description, type, local

### Community 29 - "Community 29"
Cohesion: 0.50
Nodes (4): description, required, type, Capability

### Community 30 - "Community 30"
Cohesion: 0.50
Nodes (4): default, description, type, description

### Community 31 - "Community 31"
Cohesion: 0.50
Nodes (4): default, description, type, local

### Community 32 - "Community 32"
Cohesion: 0.67
Nodes (3): Number, anyOf, description

### Community 33 - "Community 33"
Cohesion: 0.67
Nodes (3): PermissionEntry, anyOf, description

### Community 34 - "Community 34"
Cohesion: 0.67
Nodes (3): Number, anyOf, description

### Community 35 - "Community 35"
Cohesion: 0.67
Nodes (3): PermissionEntry, anyOf, description

## Knowledge Gaps
- **149 isolated node(s):** `name`, `private`, `version`, `type`, `dev` (+144 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **4 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `Bus` connect `Community 4` to `Community 1`, `Community 2`, `Community 3`, `Community 9`?**
  _High betweenness centrality (0.130) - this node is a cross-community bridge._
- **Why does `Hardware` connect `Community 3` to `Community 4`, `Community 7`?**
  _High betweenness centrality (0.079) - this node is a cross-community bridge._
- **Why does `Iso9660` connect `Community 2` to `Community 4`?**
  _High betweenness centrality (0.057) - this node is a cross-community bridge._
- **What connects `name`, `private`, `version` to the rest of the system?**
  _149 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Community 0` be split into smaller, more focused modules?**
  _Cohesion score 0.075990675990676 - nodes in this community are weakly interconnected._
- **Should `Community 1` be split into smaller, more focused modules?**
  _Cohesion score 0.138763197586727 - nodes in this community are weakly interconnected._
- **Should `Community 2` be split into smaller, more focused modules?**
  _Cohesion score 0.1091753774680604 - nodes in this community are weakly interconnected._