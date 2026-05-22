# Spec 09 — CLI + file watcher

**État**: pending (squelette CLI fait)

## Objectif

Binaire `codegraph` final qui orchestre tout. Watcher live pour sync auto.

## Sous-commandes

| Cmd | Action |
|---|---|
| `codegraph` (no arg) | → `install` interactif |
| `install` | installer multi-agent (spec 08) |
| `init [-i/--index]` | crée `.codegraph/` + DB; `-i` lance indexation après |
| `uninit` | supprime `.codegraph/` après confirmation |
| `index` | full reindex |
| `sync` | incremental: rescan fichiers modifiés (compare mtime+sha256) |
| `status` | size DB, count nodes/edges/files, backend SQLite, dernière indexation |
| `query <q>` | search FTS, sortie tableau |
| `files [path]` | liste fichiers indexés sous path |
| `context <target>` | build markdown context, stdout |
| `affected <node>` | impact radius, stdout |
| `serve --mcp` | run MCP server stdio (spec 07) |
| `watch` | run file watcher en foreground (debug) |

## Watcher

- Crate `notify` + `notify-debouncer-full` (debounce ~500ms).
- Démarré automatiquement quand `serve --mcp` tourne — réindex live pendant que l'agent code.
- Filtre: même `ignore::WalkBuilder` qu'à l'index pour rejeter événements sur fichiers ignorés.
- Sur event:
  - Create/Modify → enqueue `sync_file(path)`.
  - Delete → `db.delete_file_cascade`.
  - Rename → delete old + sync new.
- Worker tokio task dédié.

## Output

- `--json` global flag → toutes les commandes sortent JSON au lieu de texte humain.
- Couleurs via `anstream` (auto-detect TTY).
- Progress bar via `indicatif` pour `index` / `sync` long.

## .codegraph layout

```
.codegraph/
  db.sqlite        // schéma v1
  config.toml      // ignore patterns custom, lang overrides
  .gitignore       // contient "*" (jamais commité)
  version          // texte: version du binaire ayant créé le dossier
```

## CLAUDE.md detection (existant archive)

`init` détecte si project a CLAUDE.md / AGENTS.md / `.cursor/rules/` → propose `codegraph install` à la suite.

## Tests

- Smoke test: `init -i` sur fixture, `status` montre N nodes > 0, `query foo` répond.
- Watcher test: créer fichier dans tempdir, attendre debounce, assert node apparaît dans DB.

## Pièges

- `tracing` doit écrire stderr — `serve --mcp` corrompt le protocole sinon.
- Lockfile concurrent: `.codegraph/db.sqlite.lock` (advisory `fs2::FileExt::try_lock_exclusive`) pour bloquer `index` + `serve --mcp` simultanés sur le même writer.
- Signal handling: SIGINT pendant index → flush transaction en cours puis exit clean.
