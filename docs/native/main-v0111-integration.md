# main → v0.11.1 integration (2026-10-21)

Merged `main` at `7c328eb8` into the fork tracking baseline `6a7a86aa`
(origin/upstream-track, PhotoCraft 0.11.1). Merge commit `b6cfb960` retains
both parents and the full subtree history. No upstream remote fetch was used
for this integration. Package version is 0.11.1.1 / 110101.

## Compatibility decisions

- Retain main's ArkTS/C++ host, native adapter, subwindows, fullscreen, canvas
  dragging, pen diagnostics, palm/finger rejection, Restore Defaults actions,
  ArkUI themes and system file pickers. Host and native adapter source/test
  files match main after the merge (the simulator test setup is updated below).
- Combine 0.11.1's tolerant per-field preference loader with main's migration
  of explicitly saved finger-input policy.
- Keep 0.11.1's live preview refresh handling and menu mnemonic implementation,
  together with main's GPU fallback diagnostics and themed menu styling.
- Keep the new file-name visibility preference while retaining the HarmonyOS
  application title. Preserve both new Serbian content-aware-fill translations
  and main's fullscreen translations.
- Preserve the public bundle identity, numeric version format and local wgpu-hal
  dependency patch. Both Cargo lockfiles resolve in locked/offline mode.

## Regressions found and fixed

The first complete UI run exposed two compatibility issues; `e649c6de` fixes both:

- The longer HarmonyOS heading widened the Home card and clipped buttons in a
  390-point canvas. Fit the heading to the card; the existing narrow-canvas test
  passes at all three tested widths.
- Eraser pressure-size and pressure-opacity options were absent from 0.11.1's
  tool-option inventory. Include both in Eraser reset/preset keys, with a test
  proving reset restores their defaults without changing another tool's option.

Also add a regression test for preserving a CPU-refresh GPU fallback reason and
clearing it after GPU rendering resumes.

## Verification

- Native adapter: 92 tests passed (plus its subprocess child invocation).
- Engine: 1,814 passed, 13 ignored; UI: 3,243 passed, 13 ignored;
  paint: 114 passed. Total: 5,263 passed, zero final failures.
- All 26 non-simulator host regression scripts passed.
- Locked offline Cargo metadata and dependency layering passed (31 crates).

- Final Clippy for UI/engine/paint, all targets, HarmonyOS UI feature, with
  `-D warnings` passed. `cargo xtask wasm` passed for all supported shared crates.
- Engine adversarial-parameter `panic_hunt`: 1 passed, no crash/hang.
- Final `scripts/dev.sh build`: ARM64 OHOS release Rust library and debug
  unsigned HAP passed, including ABI, pages, UTD and public bundle verification.
- Installed and launched the final public bundle on the 2880 × 1920 Pad emulator
  `127.0.0.1:5555`; inspected its rendered Home screen, menus and panels.
- Independent `.pinchtest` package: 20 OS pinch gestures and 20 staggered-release/
  echo replays passed. Document revisions `[1, 1, 1, 2]` prove navigation did not
  paint and the positive-control pen stroke did.
- Independent `.brushpreviewtest` package: all ten screenshots passed pixel and
  revision checks. Pen hover/resume changed 612 pixels, mouse hover 602; hidden
  previews changed zero. Revisions `[1, 1, 2, 2]` prove only the stroke edited the
  document. Updated the test runner to explicitly enable opt-in stylus hover
  preview: its previous assumption that a fresh package enabled it was stale.
  Used the bundled Python/Pillow runtime for image verification.
- Temporary test package identities were restored; only the intended simulator
  test precondition change is committed. Logs and full-size screenshots remain
  outside Git under `/tmp/photocraft-v0111-*`.

The 26 ignored Rust tests retain their existing configuration. Simulator tests
cover the recorded input/rendering workflows, not every editor feature. Physical
Pad pen firmware, signed-device deployment, and exhaustive system picker/browser
workflows were not exercised. The original dirty user checkout was not changed.

