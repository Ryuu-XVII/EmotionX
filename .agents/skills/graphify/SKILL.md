---
name: graphify
description: "Turn codebases, documents, and multi-file architectures into persistent, queryable knowledge graphs with community detection, God Nodes analysis, interactive HTML, and GraphRAG JSON outputs."
---

# Graphify Skill

Turn any folder of files into a navigable knowledge graph with community detection, an honest audit trail, and three outputs: interactive HTML, GraphRAG-ready JSON, and a plain-language GRAPH_REPORT.md.

## Core Capabilities
- **Codebase Graph Generation**: Performs deterministic AST extraction across Rust, TypeScript, Python, and other languages.
- **Interactive Visualization**: Generates `graphify-out/graph.html` for browser-based visual exploration.
- **Architectural Analysis**: Detects God Nodes (core abstractions), community clusters, cross-subsystem bridges, and potential architectural cycles.
- **Graph Queries**: Query code relationships via BFS/DFS traversal over `graphify-out/graph.json`.

## Quick Execution in Windows/Powershell

```powershell
python graphify-out/run_graphify.py
```

Outputs will be updated in:
- `graphify-out/graph.html`
- `graphify-out/graph.json`
- `graphify-out/GRAPH_REPORT.md`
