# main → v0.10.0 integration (2026-10-21)

Merged remote main `050011f1` and local main `66d2d6cd` into the existing
v0.10.0 line (previous head `aaa0fbee`), retaining both parents and the
PhotoCraft 0.10.0 subtree history. Package version remains 0.10.0.1 / 100001.

## Compatibility decisions

- Register stylus diagnostics and all seven Restore Defaults actions in 0.10.0's
  shared engine shell-command table; retain its newer shortcuts and view commands.
- Keep duplicate-document detection and disk-change save prompts alongside main's
  rejection of ephemeral Save As tokens in recent files.
- Retain flat swatch layout and the existing touch/pen preset-browser regression
  suite; remove duplicate test definitions introduced by the merge.
- Preserve palm rejection, subwindows, native file/gallery source picker, stylus
  telemetry, ArkUI light highlights, and native adapter behavior from main.
- Preserve the public bundle identity from local main with matching UTDs. Retain
  two-page HAP verification and add expected-bundle validation plus a regression
  test, avoiding local main's older single-page verifier.

## Verification

All checks below passed on the merged tree:

- `cargo test --offline --manifest-path native/rust/Cargo.toml`: 92 passed.
- `cargo test --offline --manifest-path upstream/photocraft/Cargo.toml -p photocraft-ui-egui -p photocraft-engine --lib --features photocraft-ui-egui/harmonyos-ui`:
  engine 1,722 passed / 13 ignored; UI 3,102 passed / 13 ignored; no failures.
- All 26 non-simulator `scripts/test_*.cjs` and `scripts/test_*.py` scripts;
  JavaScript tests used DevEco's `tools/ohpm/node_modules/typescript` runtime.
- Upstream `cargo metadata --offline`, Clippy for both crates with `--all-targets`
  and `-D warnings`, `cargo xtask layers`, and `cargo xtask wasm`.
- `scripts/dev.sh build`: ARM64 OHOS release Rust library, debug unsigned HAP,
  native ABI/page/UTD/public bundle verification.
- Installed the resulting unsigned HAP on `127.0.0.1:5555` and launched
  `io.github.storytold.photocraft.hmos/EntryAbility` successfully. Inspected the
  rendered main window at 2880 × 1920. Screenshots were kept outside Git.

The 26 ignored Rust tests retain their existing configuration. Simulator startup
is a smoke check; physical Pad stylus hardware and exhaustive UI workflows were
not exercised in this integration run.
