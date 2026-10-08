# PhotoCraft HarmonyOS Pad agent notes

## Repository layout

- `upstream/photocraft/` is the PhotoCraft v0.3.0 Rust source, imported as a Git subtree **without squashing history**. Its own `AGENTS.md` applies to PhotoCraft code. This repository's HarmonyOS ArkWeb wrapper is an intentional user requested integration.
- `third_party/wgpu-hal-30.0.1/` is the verified wgpu-hal 30.0.1 crate with the WebGL uniform block fix. The path override is in `upstream/photocraft/Cargo.toml`.
- `entry/` is the HarmonyOS app. Its `rawfile` assets are the offline packaged snapshot. They are not the source for development edits.
- `third_party/photocraft/` retains upstream release files, licenses, manifests and the original compatibility patches for audit.

## Git ancestry and upstream updates

- The `upstream` remote points to `https://github.com/storytold/photocraft.git`; tag `v0.3.0` is an ancestor of this repository's `main` branch through the subtree merge.
- Git remotes are local configuration and do not travel with clones. In a fresh clone, run `git remote add upstream https://github.com/storytold/photocraft.git` before fetching updates; the imported commit ancestry remains in the repository.
- Do not replace the subtree with a tarball, initialize another `.git` inside it, shallow fetch it, squash its history, or force push the upstream remote.
- For future upstream changes, fetch from `upstream`, review the range, then use `git subtree pull --prefix=upstream/photocraft upstream <reviewed-ref>` **without `--squash`**. Recheck the HarmonyOS compatibility changes and the vendored dependency after each update.
- The root `build-profile.json5` contains local signing material and is ignored. `build-profile.example.json5` is the safe template. Never commit signing certificates, private keys or passwords.

## Development loop

1. Edit Rust or web files under `upstream/photocraft/`.
2. Run `scripts/dev.sh run` with a HarmonyOS 6.0 Pad simulator and configured debug signing. The current Mac has ignored tool paths in `scripts/dev.local.env`; on other machines, install the tools on PATH or copy `scripts/dev.local.env.example` and fill in their paths. The script builds and installs the wrapper, runs Trunk, establishes `hdc rport` for port 8765 and launches ArkWeb in source development mode.
3. Keep the terminal open. Trunk watches source edits, rebuilds WASM and refreshes the page automatically. No HAP rebuild or reinstall is needed for PhotoCraft source edits. ArkTS wrapper edits still require a HAP rebuild.
4. For an already installed wrapper, use `scripts/dev.sh serve` in one terminal and `scripts/dev.sh launch` in another. A normal launch without the `photocraft.dev` Want parameter uses bundled offline assets.
5. Before shipping a new offline HAP, run the release Web build and intentionally update `entry/src/main/resources/rawfile`, resource names in `Index.ets`, and `third_party/photocraft/manifest.json`; verify them with `scripts/verify_assets.py`.

## Verification

- Run `cargo metadata --offline` from `upstream/photocraft/`, then a Trunk Web build for Rust changes. For wrapper changes, build the HAP and verify on the simulator before connecting a real Pad.
- Confirm live editing by changing a visible source string, waiting for Trunk to rebuild, observing the refreshed simulator page, and reverting the test change.
- Preserve the existing Save/Save As picker and system browser external link behavior during wrapper changes.
