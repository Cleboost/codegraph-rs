# CodeGraph

[![CI](https://github.com/cleboost/codegraph/actions/workflows/ci.yml/badge.svg)](https://github.com/cleboost/codegraph/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

> Local-first code intelligence for AI agents. Built in Rust. Single static
> binary, ~5 MB. Tree-sitter knowledge graph in SQLite, served over MCP.

CodeGraph parses your codebase with tree-sitter, stores every symbol, edge,
and file in a local SQLite database (FTS5), and exposes the graph to
AI agents — Claude Code, Cursor, Codex CLI, opencode, Hermes — over the
Model Context Protocol (MCP).

Agents that consult the graph instead of grepping the filesystem make
**fewer tool calls**, **explore faster**, and **stay within context**.

## Highlights

- **One binary.** Rust + statically-linked SQLite + native tree-sitter
  grammars. No Node runtime, no `.wasm`, no `node_modules`.
- **Small.** ~5 MB stripped (vs ~140 MB for the previous TypeScript build).
- **Fast.** Parses a 139-file TypeScript project in ~190 ms (release, parallel).
- **Local.** Index lives in `.codegraph/db.sqlite` next to your code. Nothing
  leaves the machine.
- **Multi-agent.** A single `codegraph install` configures Claude Code, Cursor,
  Codex, opencode, Hermes and Antigravity CLI in one go.
- **Live.** Built-in file watcher keeps the index in sync while the MCP server
  serves your agent.

## Install

<details>
<summary><strong>Automatic (recommended)</strong></summary>

**Linux / macOS**

```sh
curl -fsSL https://raw.githubusercontent.com/Cleboost/codegraph-rs/main/scripts/install.sh | sh
```

Drops `codegraph` into `~/.local/bin`. Override with `CODEGRAPH_INSTALL_DIR`.

**Windows (PowerShell)**

```powershell
irm https://raw.githubusercontent.com/Cleboost/codegraph-rs/main/scripts/install.ps1 | iex
```

Installs to `%LOCALAPPDATA%\codegraph\bin` and adds it to the user PATH.

**Arch Linux (AUR)**

```sh
yay -S codegraph-rs-bin
```

</details>

<details>
<summary><strong>Manual</strong></summary>

1. Download the archive for your platform from the [latest release](https://github.com/Cleboost/codegraph-rs/releases/latest):

   | Platform | File |
   |---|---|
   | Linux x86_64 | `codegraph-x86_64-unknown-linux-musl.tar.gz` |
   | Linux aarch64 | `codegraph-aarch64-unknown-linux-gnu.tar.gz` |
   | macOS x86_64 | `codegraph-x86_64-apple-darwin.tar.gz` |
   | macOS arm64 | `codegraph-aarch64-apple-darwin.tar.gz` |
   | Windows x86_64 | `codegraph-x86_64-pc-windows-msvc.zip` |

2. Extract and place the `codegraph` binary somewhere on your `PATH`.

</details>

<details>
<summary><strong>From source</strong></summary>

Requires Rust stable (≥ 1.80).

```sh
git clone https://github.com/Cleboost/codegraph-rs
cd codegraph-rs
cargo build --release -p codegraph
# binary at target/release/codegraph
```

Or via Cargo directly:

```sh
cargo install --git https://github.com/Cleboost/codegraph-rs codegraph
```

</details>

## Quick start

```sh
# 1. Init, index, and configure your agents in one step
cd ~/code/my-project
codegraph init

# 2. Use it
codegraph query UserService
codegraph context "auth middleware"
```

Your agent now has tools like `codegraph_search`, `codegraph_callers`,
`codegraph_impact`, `codegraph_context` available over MCP. The file watcher
keeps the index fresh while you edit.

## CLI reference

| Command | What it does |
|---|---|
| `codegraph init [--no-index]` | Create `.codegraph/`, index, and configure agents; `--no-index` skips indexing |
| `codegraph uninit` | Remove `.codegraph/` |
| `codegraph index` | Full reindex of the workspace |
| `codegraph sync` | Incremental reindex (sha256-based skip) |
| `codegraph status` | Show counts, size, schema version |
| `codegraph query <q>` | Full-text search across symbols |
| `codegraph files [path]` | List indexed files under a prefix |
| `codegraph context <target>` | Build markdown context for a symbol |
| `codegraph serve --mcp` | Run as MCP server over stdio (used by agents) |
| `codegraph visualize` | Local web UI (2D/3D graph + table) at `http://127.0.0.1:7421` |

Global flag `--path <dir>` overrides the workspace root.

`visualize` is enabled by default. For a slimmer binary without the embedded
web UI: `cargo build -p codegraph --no-default-features`.

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
  codegraph-installer/  Claude / Cursor / Codex / opencode / Hermes targets
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
  config.toml      Language overrides (see below)
  .gitignore       Pre-filled so the index is never committed
  version          Codegraph version that created the directory
```

Add a `.codegraphignore` file at the workspace root to exclude additional
paths beyond your `.gitignore`. Same syntax.

### C vs C++ headers (`.h`)

By default, `.h` files are resolved automatically:

- **C++ project** (`.cpp`/`.hpp` present, no `.c`) → parsed as C++
- **C project** (`.c` present, no C++ sources) → parsed as C
- **Mixed C/C++** → each `.h` is inspected for C++ syntax (`namespace`, `class`, `template`, …)

Override in `.codegraph/config.toml`:

```toml
[languages]
headers = "auto"   # "auto" (default), "c", or "cpp"
```

After changing this setting, run `codegraph index` to re-index headers.

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

Result: **~5 MB** stripped, **sub-second** startup, **~5× faster** indexing
on the same workspace.

# Roadmap:

- Framework-aware route extraction (Express, Laravel, Rails, FastAPI, Django,
  Spring, Axum, …)
- Additional grammars (Kotlin, Dart, Pascal, Luau, Svelte/Vue/Liquid)
- Eval harness for accuracy regression testing

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
