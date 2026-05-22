# Spec 10 — Release pipeline (GitHub Actions)

**État**: pending

## Objectif

Builds reproductibles cross-platform + GitHub Releases avec binaires attachés. `cargo install codegraph` fonctionne en parallèle.

## Cibles

| OS | Target triple | Runner |
|---|---|---|
| Linux x86_64 | `x86_64-unknown-linux-gnu` | ubuntu-latest |
| Linux x86_64 musl | `x86_64-unknown-linux-musl` | ubuntu-latest (cross) |
| Linux aarch64 | `aarch64-unknown-linux-gnu` | ubuntu-latest (cross) |
| macOS x86_64 | `x86_64-apple-darwin` | macos-13 |
| macOS aarch64 | `aarch64-apple-darwin` | macos-latest |
| Windows x86_64 | `x86_64-pc-windows-msvc` | windows-latest |

Linux musl = bin statique zéro dep glibc → recommandé pour `curl | sh` install.

## Workflow `.github/workflows/release.yml`

Déclencheur: `push: tags: ['v*']`.

Steps:
1. Checkout.
2. `actions/cache` sur `~/.cargo`, `target/`.
3. Setup rust stable + target triple.
4. `cargo build --release --target $TRIPLE -p codegraph`.
5. Strip + UPX (optionnel — UPX casse macOS signing, à valider).
6. Archive: `tar.gz` Linux/macOS, `zip` Windows.
7. Checksums SHA256 par archive.
8. `gh release create $TAG --notes-file CHANGELOG_EXTRACT.md` (extract section `## [X.Y.Z]`).
9. `gh release upload` toutes les archives + `.sha256`.

## CI hors release `.github/workflows/ci.yml`

- Push/PR: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
- Matrix: Linux + macOS + Windows.
- Bench (optionnel): `cargo bench` sur Linux, comparaison vs baseline stockée.

## Install script

`scripts/install.sh`:
```sh
#!/bin/sh
# detect OS+arch, download from GH Releases latest, verify sha256, install to ~/.local/bin
```

Equivalent `install.ps1` pour Windows.

## crates.io

`cargo publish` manuel (pas dans CI) pour éviter publish accidentel. Publier dans l'ordre des deps:
1. codegraph-core
2. codegraph-db
3. codegraph-extract, codegraph-resolve, codegraph-graph
4. codegraph-context
5. codegraph-mcp, codegraph-installer
6. codegraph (binaire — utilisateurs feront `cargo install codegraph`)

## Tailles cibles

- Bin Linux x86_64 stripped + LTO: viser **<15MB**.
- Si dépasse: profil `release-small` ou retirer langages exotiques (Lua/Scala/Swift via feature flags off).

## Tests

- Job `release-smoke`: après build, run `codegraph --version`, `codegraph init -i` sur fixture, assert exit=0.

## Pièges

- macOS notarization: hors scope MVP, signature ad-hoc OK.
- musl + rusqlite bundled: vérifier que `cc` est statique (devrait être OK avec bundled).
- Windows: `\r\n` dans archives — utiliser `7z` propre, pas `tar` GNU sur Win.
- CHANGELOG.md: réutiliser format archive (sections Added/Changed/Fixed) pour script d'extraction notes.
