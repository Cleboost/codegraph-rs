# Spec 01 — Bootstrap workspace

**État**: ✅ done

## Objectif

Workspace Cargo compilable avec les 9 crates squelettes. Aucun comportement, juste structure.

## Livré

- `/Cargo.toml`: workspace resolver=2, `[workspace.package]` (version, edition, license, repo), `[workspace.dependencies]` centralisées (serde, rusqlite, tree-sitter + 15 grammaires, clap, tokio, notify, ignore, rayon, dirs, jsonc-parser, toml_edit).
- Profils:
  - `release`: `lto="fat"`, `codegen-units=1`, `strip="symbols"`, `panic="abort"`.
  - `release-small`: hérite + `opt-level="z"`.
- `rust-toolchain.toml`: channel stable + rustfmt + clippy.
- `.gitignore`: `/target`, `.codegraph/`, IDE noise.
- 9 crates avec `Cargo.toml` + `src/lib.rs` (ou `main.rs` pour le binaire) commenté TODO.

## Validation

`cargo check --workspace` finit sans erreur (~6s clean rebuild).

## Notes

- Versions tree-sitter grammars: `swift=0.7`, `scala=0.26`, `lua=0.5`, `kotlin=0.3`, le reste `0.23`. Certaines crates communautaires lèveraient des conflits — surveiller à l'ajout d'une grammaire neuve.
- Crate principal `codegraph` (binaire) — `Cargo.toml` workspace dir `crates/codegraph`. Nom du paquet sur crates.io reste `codegraph`.
