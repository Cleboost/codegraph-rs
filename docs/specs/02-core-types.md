# Spec 02 — Core types (NodeKind, EdgeKind, errors)

**État**: ✅ done

## Objectif

Types partagés stable entre toutes les crates. Source unique pour les chaînes serialisées dans DB/MCP.

## Choix

- `NodeKind` / `EdgeKind`: enums C-like `#[derive(Serialize, Deserialize)]` `#[serde(rename_all = "snake_case")]`. Méthode `as_str(self) -> &'static str` pour insertion DB sans alloc.
- `Node`: id `i64` (rowid SQLite), `kind`, `name`, `qualified_name: Option<String>`, `file: Utf8PathBuf` (camino — pas de `OsString` partout), `start_line`, `end_line`, `signature`, `docstring`, `language`.
- `Edge`: `from`, `to`, `kind`, `file: Option<Utf8PathBuf>`, `line: Option<u32>`.
- `Error`: `thiserror`, variantes `Io`, `Db`, `Parse`, `Invalid`, `NotInitialized`, `Other`. `Result<T> = std::result::Result<T, Error>`.

## Mapping avec archive

NodeKind (22): file, module, class, struct, interface, trait, protocol, function, method, property, field, variable, constant, enum, enum_member, type_alias, namespace, parameter, import, export, route, component.

EdgeKind (12): contains, calls, imports, exports, extends, implements, references, type_of, returns, instantiates, overrides, decorates.

Strings exacts identiques à `archive/src/types.ts` — agents prompts existants restent valides.

## Hors scope

Pas de méthode `Node::new()` — construction directe par struct literal jusqu'à ce qu'un besoin émerge.
