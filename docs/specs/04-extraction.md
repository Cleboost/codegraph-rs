# Spec 04 — Extraction (tree-sitter natif)

**État**: pending

## Objectif

Parser un workspace en parallèle, émettre Nodes + Edges + FileRow vers la DB. 15 langages tree-sitter natifs + 3 extractors texte (Svelte, Vue, Liquid). Delphi DFM reporté.

## Architecture

```
ExtractionOrchestrator
  ├── FileWalker (ignore crate, gitignore, .codegraphignore)
  ├── LanguageRegistry (path → Box<dyn Extractor>)
  ├── ParsePool (rayon)
  │     └── for each file:
  │          extractor.extract(source) -> ExtractResult
  └── DbBatcher (chunks de 500, une tx par chunk)
```

## Trait extractor

```rust
pub trait Extractor: Send + Sync {
    fn language(&self) -> &'static str;            // "typescript"
    fn extensions(&self) -> &'static [&'static str];
    fn ts_language(&self) -> tree_sitter::Language;
    fn extract(&self, source: &str, file: &Utf8Path) -> Result<ExtractResult>;
}

pub struct ExtractResult {
    pub nodes: Vec<NodeDraft>,    // no id yet
    pub edges: Vec<EdgeDraft>,    // refer to NodeDraft by local index
    pub imports: Vec<RawImport>,  // resolved later by codegraph-resolve
}
```

`NodeDraft` = `Node` sans `id`, `EdgeDraft` = indices locaux dans le Vec de nodes; orchestrator résoud après insert.

## Langages

Un module par langage dans `src/languages/`:
- typescript.rs (gère aussi tsx via grammaire séparée du même crate)
- javascript.rs (jsx)
- python.rs
- rust.rs
- go.rs
- java.rs
- c.rs / cpp.rs
- csharp.rs
- ruby.rs
- php.rs
- scala.rs
- swift.rs
- kotlin.rs
- lua.rs

Chacun:
1. Parse source en arbre tree-sitter.
2. Walk avec `tree-sitter::Query` quand possible (queries S-expr déclaratives) sinon visit récursif.
3. Émet nodes pour: déclarations (fn/class/struct/etc.), imports, exports.
4. Émet edges: `contains` (parent → enfant), `calls` (sites d'appel), `extends`/`implements`.

Queries stockées en `include_str!("queries/typescript/symbols.scm")` — fichiers `.scm` versionnés avec le code.

## File walker

`ignore::WalkBuilder` avec:
- `.gitignore` honoré
- `.codegraphignore` custom (suffix layer)
- `hidden(true)` (skip `.git`, `.node_modules` etc — `ignore` les a déjà)
- `parents(true)` pour héritage gitignore amont
- Filtre extension via `LanguageRegistry::extension_set()`

## Parallélisme

`rayon::ThreadPoolBuilder` configuré sur `num_cpus`. Chaque worker:
- Reçoit `(PathBuf, &dyn Extractor)`.
- Lit fichier (`fs::read_to_string` — taille limite 4MB sinon skip).
- Hash sha256 du contenu pour `files.sha256`.
- Parse + extract.
- Pousse `(FileRow, ExtractResult)` dans un crossbeam channel.

Thread principal lit le channel, batch 500 → `Db::insert_*` en transaction.

## Modes

- `index_all(root)` — purge + reindex tout.
- `sync(root)` — compare sha256 par fichier; reindex seulement les changés.

## Tests

- Fixtures `tests/fixtures/typescript/sample.ts` etc.
- Assert: count nodes/edges, présence symbole précis, contains edge parent.
- `pr19-improvements.test.ts` archive → ré-utiliser fixtures comme regression suite.

## Pièges

- Tree-sitter `Language` n'est pas `Sync` pour certaines versions; wrap dans `parking_lot::Mutex<Parser>` par thread OU créer parser par fichier (cheap).
- Encoding non-UTF8: skip avec warn.
- Fichiers générés (`*.min.js`, `dist/`, `build/`): filtrer par défaut via `.codegraphignore` template.
