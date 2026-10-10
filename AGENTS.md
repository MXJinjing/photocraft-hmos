# Repository agent instructions

PhotoCraft's own source specific guidance is in `upstream/photocraft/AGENTS.md`; the user requested HarmonyOS native integration in this branch takes precedence for platform host work.

# PhotoCraft HarmonyOS Pad agent notes

## Repository layout

- `upstream/photocraft/` is the PhotoCraft v0.6.0 Rust source, imported as a Git subtree **without squashing history** and updated through v0.5.0 and v0.6.0 without squashing. Its own `AGENTS.md` applies to PhotoCraft code. This branch's native self-drawing HarmonyOS host is the user requested integration.
- `third_party/wgpu-hal-30.0.1/` is the verified wgpu-hal 30.0.1 crate with the WebGL uniform block fix. The path override is in `upstream/photocraft/Cargo.toml`.
- `entry/` is the native HarmonyOS ArkTS/C++ host. `rawfile` contains only file-type metadata; do not restore the web snapshot.
- `native/rust/` is an independent native adapter workspace. FFI and NativeWindow unsafe code stay here; upstream continues to forbid unsafe. Its lockfile and wgpu-hal patch must stay compatible with upstream.

## Git ancestry and upstream updates

- The `upstream` remote points to `https://github.com/storytold/photocraft.git`; tag `v0.3.0` is an ancestor of this repository's `main` branch through the subtree merge.
- Git remotes are local configuration and do not travel with clones. In a fresh clone, run `git remote add upstream https://github.com/storytold/photocraft.git` before fetching updates; the imported commit ancestry remains in the repository.
- Do not replace the subtree with a tarball, initialize another `.git` inside it, shallow fetch it, squash its history, or force push the upstream remote.
- For future upstream changes, fetch from `upstream`, review the range, then use `git subtree pull --prefix=upstream/photocraft upstream <reviewed-ref>` **without `--squash`**. Recheck the HarmonyOS compatibility changes and the vendored dependency after each update.
- The root `build-profile.json5` contains local signing material and is ignored. `build-profile.example.json5` is the safe template. Never commit signing certificates, private keys or passwords.

## Development loop

1. Edit shared Rust code under `upstream/photocraft/`, native adapter code under `native/rust/`, and platform code under `entry/`.
2. Install `aarch64-unknown-linux-ohos` for the configured Rust toolchain.
3. Run `scripts/dev.sh build` for a ARM64 native HAP, `scripts/dev.sh run` to build/install/launch on exactly one connected simulator/device, or `scripts/dev.sh launch` for an already installed HAP. Tool paths remain in ignored `scripts/dev.local.env`.
4. Rust and ArkTS/C++ edits require HAP rebuild/deployment. All builds and launches use the native host; there is no ArkWeb page, Trunk server, Web build command or hdc reverse-port mapping. Save documents before scripted relaunch/deployment.
5. `scripts/package_offline.sh` builds the native HAP; `--install` also installs it. Verify with `scripts/verify_native.py <HAP>`. Preserve imported upstream Web sources and history; they are not part of the HarmonyOS build/development workflow.

## Version numbers

Upstream PhotoCraft is early alpha, but SemVer already marks that with the leading `0`, so no channel word belongs in the version.

HarmonyOS accepts digits and dots only in `AppScope/app.json5` `versionName`. The DevEco check schema (`/Applications/DevEco-Studio.app/Contents/sdk/default/openharmony/toolchains/modulecheck/app.json`) leads with `^[0-9.]+`, so a suffix such as `0.3.0-hmos.1` or `0.5.0-alpha.1` only passes the loose match and risks rejection later. Use:

- `versionName`: `<upstream major>.<upstream minor>.<upstream patch>.<hmos revision>`. `0.5.0.1` is the first HarmonyOS package built from upstream v0.5.0, `0.5.0.2` the second; after an upstream bump to 0.6.0 it restarts at `0.6.0.1`.
- `versionCode`: a monotonic integer, `major*1000000 + minor*10000 + patch*100 + revision`, so `0.5.0.1` maps to `50001`.
- Channel markers (alpha/beta/rc) belong in the Git tag or release title, e.g. `v0.5.0.1-hmos.alpha`, never in `versionName`.

`.github/workflows/build-hap.yml` rejects a `version_name` that is not 2 to 4 dot-separated numbers.

## CI

`.github/workflows/build-hap.yml` runs only when dispatched by hand and builds the ARM64 Rust library and an **unsigned** native HAP from the repository. It takes a required `version_name` (see above), an optional `version_code` that defaults to the run number, a `build_mode` of release or debug, a `runner`, `commandline_tools_url`, and a `publish` choice of `none`, `prerelease` or `release`. It writes the version into `AppScope/app.json5`, generates `build-profile.json5` from the committed `build-profile.example.json5`, builds, and uploads `photocraft-hmos-<version>-<mode>-unsigned.hap` as an artifact. Signing material stays local, so the artifact installs on a simulator only.

The workflow's `build_mode` controls both HAP and Rust builds: `debug` uses Cargo's `dev` profile and links from the `debug/` target directory; `release` uses and links the `release/` profile. The workflow passes the same mode to `PHOTOCRAFT_RUST_PROFILE` and CMake's `-DPHOTOCRAFT_RUST_PROFILE`, so these must remain synchronized. Local `scripts/dev.sh build` continues to use release Rust libraries by default.

With `publish` set, the job also pushes that HAP to a GitHub release tagged `release_tag` (empty means `v<version_name>`), which is also where a channel marker belongs. Re-running against an existing tag replaces the asset and the notes, so a rebuild never fails on a duplicate tag; the release body records the version, the build mode and the HAP's SHA-256.

The HarmonyOS SDK is absent from GitHub-hosted images, and Huawei serves the DevEco command-line tools only to signed-in developer accounts (an anonymous request returns 403), so `runner` selects where the tools come from:

- `self-hosted` (preferred): uses the DevEco Studio already on the runner, with the same lookup as `scripts/dev.sh` (`DEVECO_SDK_HOME`/`HOS_SDK_HOME`, `PHOTOCRAFT_HVIGOR`, an ignored `scripts/dev.local.env`, then `/Applications/DevEco-Studio.app`).
- `ubuntu-latest`: downloads the tools from `commandline_tools_url`, which must be a freshly generated link from https://developer.huawei.com/consumer/cn/download/command-line-tools-for-hmos after signing in. The link is signed and time-limited, so it is supplied per run and never stored in the repository; the workflow masks it in the logs and keys the tool cache on the archive name.

## Verification

### Native Rust test file locations

- Put native adapter unit tests in `native/rust/tests/unit/<module>.rs`; use `native/rust/tests/unit/host.rs` for tests of `src/lib.rs`. Do not add inline test bodies or test-only helpers under `native/rust/src/`.
- In the corresponding source module, load its test file with `#[cfg(test)]`, `#[path = "../tests/unit/<module>.rs"]`, and `mod tests;` (`mod host_tests;` in `lib.rs`). These remain unit tests within the original module, so private access and existing test names are preserved. Keep test-only lint allowances on this module or in its test file.
- Put helpers used by one test module in that test file. Keep these unit-test files inside the `tests/unit/` subdirectory: top-level `tests/*.rs` are automatically discovered by Cargo as separate integration-test crates. Do not expose production internals just to relocate tests.
- Keep the exact child-test name used by the working-directory subprocess test in sync with its module path; changing it can silently skip the child test.
- Verify migrations by comparing `cargo test --offline --manifest-path native/rust/Cargo.toml --lib -- --list` before and after, then run `cargo test --offline --manifest-path native/rust/Cargo.toml`. Native builds must continue to exclude test code.

- Run `cargo metadata --offline` from `upstream/photocraft/` for Rust changes; validate the HarmonyOS branch with native builds. For wrapper changes, build the HAP and verify on the simulator before connecting a real Pad.
- Native host changes require tests, an ARM64 Rust build, a HAP build and simulator verification. Use fixed-size before/after screenshots for UI comparisons.
- Preserve the existing Save/Save As picker and system browser external link behavior during wrapper changes.
