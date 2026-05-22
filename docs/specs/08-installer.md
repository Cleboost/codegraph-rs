# Spec 08 — Multi-agent installer

**État**: pending

## Objectif

Configurer 5 agents (Claude Code, Cursor, Codex, opencode, Hermes) en une commande, idempotent, sans casser config existante.

## Trait

```rust
pub trait AgentTarget: Send + Sync {
    fn id(&self) -> &'static str;          // "claude"
    fn label(&self) -> &'static str;       // "Claude Code"
    fn detect(&self) -> DetectStatus;      // NotInstalled | Installed | PartiallyInstalled
    fn install(&self, opts: &InstallOpts) -> Result<InstallReport>;
    fn uninstall(&self) -> Result<InstallReport>;
}

pub enum DetectStatus { NotFound, Found, AlreadyConfigured }
pub enum InstallReport { Installed, Unchanged, Updated(Vec<PathBuf>) }
```

## Cibles

| Agent | Config | Notes |
|---|---|---|
| Claude Code | `~/.claude/settings.json` (global) ou `.claude/settings.local.json` (project) + `CLAUDE.md` | JSON, `mcpServers.codegraph` |
| Cursor | `.cursor/mcp.json` + `.cursor/rules/codegraph.mdc` | **Quirk**: cwd faux → injecter `--path` (absolu si project, `${workspaceFolder}` si global) |
| Codex | `~/.codex/config.toml` + `~/.codex/AGENTS.md` | TOML, table `[mcp_servers.codegraph]` — sérializer maison qui préserve siblings |
| opencode | `opencode.jsonc` ou `.json` + `~/.config/opencode/AGENTS.md` | Préfère `.jsonc`, edits via `jsonc-parser` pour préserver commentaires |
| Hermes | `~/.hermes/...` (TBD à partir d'archive `targets/hermes.ts`) | À documenter en porting |

Chaque cible vit dans `crates/codegraph-installer/src/targets/{id}.rs`.

## Shared

- `instructions-template.rs`: une seule chaîne agent-agnostique (titre + tableau tools + chains). Source de vérité partagée avec `codegraph-mcp/server-instructions.md` — un test compare les deux contenus.
- `config_writer.rs`: helpers pour JSON / JSONC / TOML surgical edits.
- `toml.rs`: sérializer minimal pour `[mcp_servers.X]` qui préserve tables sœurs (cf. archive `targets/toml.ts`).

## Détection installation existante

`detect()`:
- Lit le fichier config s'il existe.
- Parse, check présence de la clé `codegraph` dans le bloc MCP.
- Retourne `Found` si présente et args valides, `NotFound` sinon, `Installed` si match exact attendu.

## Idempotence

Test obligatoire (spec depuis archive `__tests__/installer-targets.test.ts`):
- `install` deux fois → second call retourne `Unchanged`, fichier byte-equal après le premier.
- `uninstall` après `install` restaure fichier à l'état initial (avec une tolérance EOL).
- Tables/clés sœurs (`[mcp_servers.other]`, `mcpServers.other`) intactes.

## CLI

`codegraph install` (interactif via `dialoguer` ou `inquire`):
1. Détecte agents présents.
2. Multi-select prompt — coche par défaut ceux détectés.
3. Per-agent confirm + install.
4. Résumé final.

Flags: `--all` pour install non-interactif sur agents détectés.

## Tests

- Per-target: parameterized contract suite. Pour chacun: fresh install, re-install (byte-equal), sibling preservation, uninstall reversal, partial-state recovery.
- ~50 tests cible.

## Pièges

- Cursor MCP working-dir: oublier `--path` casse silencieusement.
- Codex `~/.codex/config.toml`: arrays `[[mcp_servers]]` (table arrays) à preserver — pas le format qu'on écrit, mais on doit le rendre verbatim.
- opencode `.jsonc` peut contenir des commentaires importants — toujours passer par `jsonc-parser` edits.
- Permissions Windows sur `~/.claude/` — créer le dossier si absent.
