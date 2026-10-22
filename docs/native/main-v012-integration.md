# main → v0.12.0 integration (2026-10-22)

Merged `origin/main` (`736acf981`, including #71) into branch `v0.12.0`
(previous tip `64788597d` = #70 / upstream v0.12.0 sync). Merge commit keeps
both parents. Package version remains 0.12.0 on this track.

Same intent and resolved tree as #72 on `upstream-track`.

## Conflict resolution

- Keep upstream 0.12 engine/UI advances from the v0.12.0 side.
- Keep main HarmonyOS host behavior (sandbox path pickers / `path_pick` Import,
  brand::APP_NAME, finger-input pref migration, pen-hover tests, OHOS
  linux-only prefs gates, native subwindows, startup animation, export
  publication, etc.).
- i18n TSV conflicts: keep both sides' new strings.
- Spanish (`es.tsv`) not specially edited here; left to Prominent Coder beyond
  what the merge brings.
- GPU tests not exercised (host cannot pass them).

Conflicted paths (resolved to match #72):
`atn_cmds.rs`, `prefs.rs`, `prefs/tests.rs`, `canvas.rs`, `filter_dialog.rs`,
`menus.rs`, `panels.rs`, and i18n `hu`/`sr`/`tr`/`vi` TSVs.

## Verification

- `source /home/box/harmony/env.sh` then
  `cargo check --manifest-path native/rust/Cargo.toml --target aarch64-unknown-linux-ohos`
  with the OHOS NDK clang linker: **passed** (warnings only; no errors).
- GPU-related tests skipped per prior instruction.
