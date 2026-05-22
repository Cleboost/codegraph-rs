# CodeGraph — Rust Rewrite Plan

Port intégral du projet TS (`archive/`) vers Rust natif. Objectif: binaire `<15MB` stripped (vs 140MB Node bundle), parse 2-5× plus rapide, zero runtime dep.

## Non-objectifs

- Pas de compatibilité DB avec `archive/.codegraph/`. Schema repart neuf.
- Pas de wrapper npm. Distribution = `cargo install` + binaires GitHub Releases.
- Pas de port 1:1 du code TS. On reproduit le comportement observable (NodeKind, EdgeKind, surface MCP, CLI), pas la structure interne.

## Architecture cible

```
crates/
  codegraph-core/       types + erreurs (NodeKind, EdgeKind, Node, Edge)
  codegraph-db/         rusqlite + schema + prepared stmts + FTS5
  codegraph-extract/    tree-sitter natif + extractors par langage
  codegraph-resolve/    imports, name-match, frameworks
  codegraph-graph/      traversal (callers/callees/impact)
  codegraph-context/    builder markdown/json
  codegraph-mcp/        stdio JSON-RPC 2.0 hand-rolled
  codegraph-installer/  5 cibles agents (claude/cursor/codex/opencode/hermes)
  codegraph/            binaire CLI (clap) + watcher (notify)
```

Pipeline runtime:
```
files → ignore-walker → parse-workers (rayon, tree-sitter) → batch DB tx
                                                                   ↓
                                              ReferenceResolver (imports + frameworks)
                                                                   ↓
                                              GraphTraverser  ←  ContextBuilder
                                                                   ↓
                                                MCP server / CLI commands
```

## Ordre d'implémentation

| # | Étape | Spec | Dépend de |
|---|---|---|---|
| 1 | Bootstrap workspace | [01-bootstrap.md](specs/01-bootstrap.md) | — |
| 2 | Core types | [02-core-types.md](specs/02-core-types.md) | 1 |
| 3 | DB layer | [03-db-layer.md](specs/03-db-layer.md) | 2 |
| 4 | Extraction + langages | [04-extraction.md](specs/04-extraction.md) | 3 |
| 5 | Résolution + frameworks | [05-resolution.md](specs/05-resolution.md) | 4 |
| 6 | Graph + context | [06-graph-context.md](specs/06-graph-context.md) | 3 |
| 7 | MCP server | [07-mcp-server.md](specs/07-mcp-server.md) | 6 |
| 8 | Installer | [08-installer.md](specs/08-installer.md) | 1 |
| 9 | CLI + watcher | [09-cli-watcher.md](specs/09-cli-watcher.md) | 4,5,6,7,8 |
| 10 | Release pipeline | [10-release.md](specs/10-release.md) | 9 |

Étapes 1-2 done. Étape 6 peut paralléliser avec 4-5 (utilise seulement DB read).
Étape 8 indépendante du reste (pure file ops).

## Cibles binaire

- `cargo build --release`: profil `release` (LTO fat, codegen-units=1, strip, panic=abort)
- Estimation: ~12MB Linux x86_64 stripped avec 15 grammaires tree-sitter statiques + SQLite bundled
- Si dépasse 20MB: profil `release-small` (`opt-level=z`) + features off pour langages exotiques

## Tests

- Unit tests in-crate avec `#[cfg(test)]`
- Integration tests dans `crates/*/tests/`
- Fixtures synthétiques par langage dans `tests/fixtures/`
- Pas de DB mock — tempdir + rusqlite réel (cf archive/__tests__)
- Eval harness reporté post-MVP (équivalent `__tests__/evaluation/`)

## Suivi

État des tâches dans TaskList runtime. Cette doc + specs sont source de vérité pour le quoi/pourquoi.
