# CodeGraph

[![CI](https://github.com/cleboost/codegraph/actions/workflows/ci.yml/badge.svg)](https://github.com/cleboost/codegraph/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

> Local-first code intelligence for AI agents. Built in Rust. Single static
> binary, ~30 MB. Tree-sitter knowledge graph in SQLite, served over MCP.

CodeGraph parses your codebase with tree-sitter, stores every symbol, edge,
and file in a local SQLite database (FTS5), and exposes the graph to
AI agents — Claude Code, Cursor, Codex CLI, opencode, Hermes, Antigravity CLI — over the
Model Context Protocol (MCP).

Agents that consult the graph instead of grepping the filesystem make
**fewer tool calls**, **explore faster**, and **stay within context**.

## Highlights

- **One binary.** Rust + statically-linked SQLite + native tree-sitter
  grammars. No Node runtime, no `.wasm`, no `node_modules`.
- **Small.** ~30 MB stripped (vs ~140 MB for the previous TypeScript build).
- **Fast.** Parses a 139-file TypeScript project in ~190 ms (release, parallel).
- **Local.** Index lives in `.codegraph/db.sqlite` next to your code. Nothing
  leaves the machine.
- **Multi-agent.** A single `codegraph install` configures Claude Code, Cursor,
  Codex, opencode, Hermes and Antigravity CLI in one go.
- **Live.** Built-in file watcher keeps the index in sync while the MCP server
  serves your agent.

## Install

### From a release binary (recommended)

Download the latest binary for your platform from the [Releases](https://github.com/cleboost/codegraph/releases) page.
Extract the archive and run the self-installer:

```sh
# macOS / Linux
./codegraph install
```

```powershell
# Windows
.\codegraph.exe install
```

The installer will automatically copy the binary to `~/.local/bin` and add it to your system's `PATH`.
Linux x86_64/aarch64, macOS x86_64/arm64, Windows x86_64 supported.

### From Cargo

```sh
cargo install --git https://github.com/cleboost/codegraph codegraph
```

### From source

```sh
git clone https://github.com/cleboost/codegraph
cd codegraph
cargo build --release -p codegraph
# binary at target/release/codegraph
```

## Quick start

```sh
# 1. Index this project
cd ~/code/my-project
codegraph init -i

# 2. Hook up your editor(s)
codegraph install

# 3. Use it
codegraph query UserService
codegraph context "auth middleware"
```

Your agent now has tools like `codegraph_search`, `codegraph_callers`,
`codegraph_impact`, `codegraph_context` available over MCP. The file watcher
keeps the index fresh while you edit.

## CLI reference

| Command | What it does |
|---|---|
| `codegraph install` | Detect installed agents and wire them up |
| `codegraph init [-i]` | Create `.codegraph/`; `-i` indexes immediately |
| `codegraph uninit` | Remove `.codegraph/` |
| `codegraph index` | Full reindex of the workspace |
| `codegraph sync` | Incremental reindex (sha256-based skip) |
| `codegraph status` | Show counts, size, schema version |
| `codegraph query <q>` | Full-text search across symbols |
| `codegraph files [path]` | List indexed files under a prefix |
| `codegraph context <target>` | Build markdown context for a symbol |
| `codegraph serve --mcp` | Run as MCP server over stdio (used by agents) |

Global flag `--path <dir>` overrides the workspace root.

## Supported languages

15 languages with full tree-sitter extraction:

TypeScript · TSX · JavaScript · Python · Go · Rust · Java · C · C++ · C# ·
Ruby · PHP · Scala · Swift · Lua

Each language emits:
- Declaration nodes (functions, classes, structs, interfaces, traits, enums…)
- `contains` edges (parent → child)
- `calls` edges (resolved by name-matcher post-pass)
- `imports` edges (raw imports captured for further resolution)

Coming back from the TypeScript version: Kotlin (blocked on upstream
tree-sitter grammar upgrade), Dart, Pascal, Luau, and text-based extractors
for Svelte/Vue/Liquid/DFM.

## MCP tools

Agents see nine tools through the MCP server:

| Tool | Use case |
|---|---|
| `codegraph_search` | Find symbols by name / signature / docstring (FTS5) |
| `codegraph_node` | Look up a symbol by id or exact name |
| `codegraph_callers` | What calls this function? |
| `codegraph_callees` | What does this function call? |
| `codegraph_impact` | Transitive impact radius (callers + references) |
| `codegraph_context` | Composed context for a symbol or topic |
| `codegraph_files` | List indexed files under a path |
| `codegraph_status` | Index health: counts, size, schema |
| `codegraph_explore` | (reserved) Survey an unfamiliar module |

Read the [server instructions](crates/codegraph-mcp/src/server-instructions.md)
that ship with the binary — they tell your agent when to reach for which tool.

## Architecture

```
crates/
  codegraph-core/       NodeKind / EdgeKind / Node / Edge / Error
  codegraph-db/         rusqlite (bundled) + FTS5 + migrations
  codegraph-extract/    tree-sitter native + per-language extractors
  codegraph-resolve/    imports + name-matching + (later) frameworks
  codegraph-graph/      callers / callees / impact radius (BFS)
  codegraph-context/    markdown + JSON context formatters
  codegraph-mcp/        hand-rolled JSON-RPC 2.0 server over stdio
  codegraph-installer/  Claude / Cursor / Codex / opencode / Hermes / Antigravity targets
  codegraph/            CLI binary (clap) + file watcher (notify)
```

Pipeline:

```
files → ignore::WalkBuilder → rayon parse pool (tree-sitter)
             ↓
        batched DB transactions (rusqlite WAL)
             ↓
        ReferenceResolver  (name-matcher, frameworks)
             ↓
        GraphTraverser  ←  ContextBuilder
             ↓
        MCP server  /  CLI commands
```

Full design in [`docs/PLAN.md`](docs/PLAN.md). One spec per crate in
[`docs/specs/`](docs/specs/).

## Configuration

A `.codegraph/` directory is created next to your project:

```
.codegraph/
  db.sqlite        SQLite v1 (WAL mode, FTS5)
  .gitignore       Pre-filled so the index is never committed
  version          Codegraph version that created the directory
```

Add a `.codegraphignore` file at the workspace root to exclude additional
paths beyond your `.gitignore`. Same syntax.

## Why Rust?

This project is a from-scratch Rust rewrite of the previous TypeScript
implementation. The old binary embedded a Node.js runtime, 20+ tree-sitter
WASM grammars, and a native SQLite addon — about **140 MB on disk**, with a
multi-second cold start.

The Rust port:

- Drops the Node runtime → static binary
- Replaces WASM grammars with statically-linked tree-sitter C libraries
- Bundles SQLite as a static C library (no system dependency)
- Parses in parallel via `rayon`
- Builds with `lto="fat"`, `codegen-units=1`, `strip`, `panic=abort`

Result: **~30 MB** stripped, **sub-second** startup, **~5× faster** indexing
on the same workspace.

## Status

This is a **0.x** release. The MVP is functional end-to-end:

- ✅ 15 languages indexed
- ✅ FTS5 search, graph traversal, impact analysis
- ✅ MCP stdio server with 9 tools
- ✅ Multi-agent installer (idempotent, sibling-preserving)
- ✅ File watcher + incremental sync
- ✅ Cross-platform release pipeline (Linux/macOS/Windows)

Still to come:

- Framework-aware route extraction (Express, Laravel, Rails, FastAPI, Django,
  Spring, Axum, …)
- Additional grammars (Kotlin, Dart, Pascal, Luau, Svelte/Vue/Liquid)
- Eval harness for accuracy regression testing

See [`docs/PLAN.md`](docs/PLAN.md) for the roadmap.

## Development

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

Per-crate test runs:

```sh
cargo test -p codegraph-db
cargo test -p codegraph-extract
cargo test -p codegraph-installer
```

## License

MIT. See [LICENSE](LICENSE).

## Acknowledgments

- The original TypeScript implementation by [@colbymchenry](https://github.com/colbymchenry).
- `tree-sitter` and all language grammar authors.
- `rusqlite`, `notify`, `clap`, `tokio`, `rayon`, `ignore`.
