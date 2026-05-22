# AUR packaging

`codegraph-bin` — installs the prebuilt static binary from GitHub Releases.

## One-time setup

1. Create the package on AUR (manual, first time):
   ```sh
   ssh aur@aur.archlinux.org setup-repo codegraph-bin
   git clone ssh://aur@aur.archlinux.org/codegraph-bin.git
   cp packaging/aur/codegraph-bin/PKGBUILD packaging/aur/codegraph-bin/.SRCINFO codegraph-bin/
   cd codegraph-bin && git add . && git commit -m "init" && git push
   ```

2. Add an AUR SSH key (ed25519) and register the **public** half at
   <https://aur.archlinux.org/account/>. Add the **private** half + the
   maintainer username/email as GitHub repo secrets:

   - `AUR_SSH_PRIVATE_KEY`
   - `AUR_USERNAME`
   - `AUR_EMAIL`

## Release flow (automated)

The `aur` job in `.github/workflows/release.yml` runs after the `release`
job succeeds:

1. Downloads `codegraph-x86_64-unknown-linux-musl.tar.gz` and the aarch64
   variant from the release.
2. Computes SHA-256 sums.
3. Patches `PKGBUILD` (`pkgver`, `pkgrel=1`, both `sha256sums`).
4. Regenerates `.SRCINFO` via `makepkg --printsrcinfo` inside an
   `archlinux:base-devel` container.
5. Pushes the updated package to AUR via SSH.

Trigger manually with `gh workflow run release.yml -f tag=v0.1.0` (re-runs
the full release pipeline for that tag).

## Local test

```sh
cd packaging/aur/codegraph-bin
# edit pkgver to a real released version
makepkg -si
```
