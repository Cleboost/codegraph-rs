# Spec 05 — Reference resolution + frameworks

**État**: pending

## Objectif

Transformer imports textuels et patterns de framework en edges précis (`imports`, `references`, `route → handler`).

## Pipeline

```
Db (post-extraction)
   ↓
ImportResolver
   ↓
NameMatcher
   ↓
FrameworkResolvers (express, laravel, rails, fastapi, django, flask,
                    spring, gin, axum, aspnet, vapor, react-router,
                    sveltekit, vue-nuxt, cargo-workspace, nestjs, drupal)
   ↓
new edges + new route nodes inserted
```

## ImportResolver

Input: `RawImport { from_file, module_spec, imported_names }`.

Étapes:
1. **Relative** (`./foo`, `../bar`): join + résolution extension (`.ts → .tsx → /index.ts`...).
2. **Alias** (tsconfig `paths`, jsconfig, vite alias, cargo workspace members, pyproject src layout): lus une fois via `path-aliases.rs` à l'init du resolver.
3. **Bare module** (`react`, `lodash`): pas résolu — emis comme edge `imports → external` (target = node fictif `external:react` ou skip selon flag).

Output: edges `imports(file_node → file_node or symbol_node)`.

## NameMatcher

Pour les appels `calls` où la cible n'a été identifiée que par nom à l'extraction, résolution post-pass:
- Cherche `nodes` de kind `function|method|class` avec `name = target_name`.
- Si 1 candidat dans le même fichier ou un fichier importé: lien direct.
- Sinon: skip (évite faux positifs).

## Frameworks

Un module par framework. Trait commun:

```rust
pub trait FrameworkResolver: Send + Sync {
    fn name(&self) -> &'static str;
    fn detect(&self, root: &Utf8Path) -> bool;        // package.json scan, Gemfile, etc.
    fn resolve(&self, db: &Db) -> Result<FrameworkArtifacts>;
}

pub struct FrameworkArtifacts {
    pub route_nodes: Vec<NodeDraft>,
    pub edges: Vec<EdgeDraft>,
}
```

### Patterns critiques (référence archive)

| Framework | Détection | Pattern |
|---|---|---|
| Express | `express` dans package.json | `app.get('/x', handler)` → route node + ref edge |
| Laravel | `composer.json/laravel` | `Route::get(...)`, controller@method |
| Rails | `Gemfile/rails` | `routes.rb` DSL |
| FastAPI | `pyproject/fastapi` | `@app.get('/x')` décorateur |
| Django | `manage.py` | `urls.py` `path()` |
| Flask | `flask` dep | `@app.route('/x')` |
| Spring | `pom.xml` / gradle | `@GetMapping` etc |
| Gin | go.mod gin-gonic | `r.GET("/x", handler)` |
| Axum | Cargo.toml axum | `Router::new().route("/x", get(h))` |
| ASP.NET | `.csproj` | `[HttpGet("/x")]` |
| Vapor | `Package.swift` vapor | `app.get("x", use: h)` |
| React Router | `react-router` | `<Route path="/x" element={<C/>} />` |
| SvelteKit | `svelte.config.js` | `src/routes/**/+page.svelte` |
| Vue/Nuxt | `nuxt.config` | `pages/**/*.vue` |
| Cargo workspace | `[workspace]` | members glob → cross-crate imports |
| NestJS | `@nestjs/core` | `@Controller('x')` + `@Get('y')` |
| Drupal | `*.info.yml` | hooks + services.yml |

Chaque framework émet `route` node avec `qualified_name = METHOD path` (ex `"GET /users/:id"`), edge `references → handler symbol`.

## Tests

`tests/frameworks-integration.rs` (équivalent archive). Fixture par framework avec 2-3 routes attendues.

## Pièges

- Détection multi-framework: un projet peut avoir Vue + Express; tous les resolvers qui détectent run, pas de mutex exclusion.
- Réentrant: appel `sync` ne doit pas dupliquer routes — purge edges de kind `references` issues des resolvers avant ré-exécution. Marqueur `meta.source='framework:express'` sur l'edge.
