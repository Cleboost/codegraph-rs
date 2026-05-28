---
name: codegraph-orchestrator
description: Global coordination and structural codebase navigation via CodeGraph MCP
trigger: auto
paths:
  - "**/*"
---

# CodeGraph System Instructions

This project is backed by a custom **CodeGraph MCP server**. CodeGraph maintains a local Tree-sitter knowledge graph encompassing every symbol, edge, boundary, and file within this workspace. Reads operate at sub-millisecond speeds and deliver accurate structural insights that traditional text-based tools (like grep) cannot match.

---

## 🚨 CRITICAL CONSTRAINTS (Read First)

- **NEVER use generic text-search, grep, or file-reading tools** if a symbol, reference, or definition can be located using CodeGraph.
- **DO NOT double-check or re-verify** CodeGraph results with native file reads. Treat the knowledge graph as the absolute, single source of truth for codebase architecture.
- **Handle uninitialized states immediately:** If any CodeGraph tool returns a `"not initialized"` or missing index error, **STOP execution immediately** and instruct the user to run `codegraph init -i` in their terminal. Do not attempt to parse or scan the codebase manually to compensate.
- **Minimize token overhead:** Prefer targeted structural queries over dumping entire file contents into the context window.

---

## 🛠️ Tool Selection Guide

Always prefer `codegraph` tools for **structural** questions — tracing call hierarchies, mapping dependencies, determining definitions, and verifying signatures. Use standard filesystem tools *only* for literal text queries or applying actual code edits.

| Intent / Question | Recommended MCP Tool |
| :--- | :--- |
| *"Where is symbol X defined?"* | `codegraph_search` |
| *"What callers invoke function Y?"* | `codegraph_callers` |
| *"What methods or functions does Y call?"* | `codegraph_callees` |
| *"What components or files will break if I modify Z?"* | `codegraph_impact` |
| *"Show me Y's exact signature and internal block"* | `codegraph_node` |
| *"Give me focused, aggregated context for this task"* | `codegraph_context` |
| *"What files exist under a specific path/ directory?"* | `codegraph_files` |
| *"Is the local knowledge graph healthy and active?"* | `codegraph_status` |

---

## 💡 Rules of Thumb & Workflows

### 1. Unified Context Gathering
Do not chain manual searches and individual node inspections yourself. **`codegraph_context` is designed to perform aggregate lookups in a single call.** Execute it first when onboarding onto a new task or analyzing a localized bug.

### 2. Defensive Token Preservation
Before updating an API route, a UI component, or a system utility, query `codegraph_impact` to pinpoint downstream effects. This ensures you only request and modify files strictly relevant to the current objective, preventing context window saturation.

### 3. Strict AST Reliance
Because CodeGraph parses the Abstract Syntax Tree (AST), its structural insights are guaranteed. If a symbol look-up yields no results, assume the symbol does not exist in the current active workspace index.