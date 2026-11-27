# HarmonyOS host translation overlay

Upstream v0.23.1 moved PhotoCraft UI catalogs from
`upstream/photocraft/crates/ui-egui/src/i18n/*.tsv` into the external
[storytold/craft-i18n](https://github.com/storytold/craft-i18n) repo
(`locales/<code>/photocraft.tsv`, pinned by `craft-i18n.rev`). The in-tree
TSVs were deleted with that change.

These files keep the HarmonyOS-only rows that used to live in those TSVs
(the `# HarmonyOS host additions retained during upstream sync` blocks),
so the strings are not lost. `scripts/build_native.sh` embeds them: it runs `scripts/prepare_craft_i18n.py`,
which fetches craft-i18n at the commit in `upstream/photocraft/craft-i18n.rev`
(cached under `native/i18n/.craft-i18n-cache/`), appends the rows here that the
catalogs lack (a row craft-i18n already has keeps its translation), and builds
with `CRAFT_I18N_DIR` pointing at the merged copy and `CRAFT_I18N_REQUIRED=1`.
Upstream `build.rs` is unchanged.

- Offline: set `PHOTOCRAFT_CRAFT_I18N_SRC` (or `CRAFT_I18N_DIR`) to a local
  craft-i18n checkout; the overlay is still merged on top.
- `PHOTOCRAFT_I18N=off` builds an English-only UI.
