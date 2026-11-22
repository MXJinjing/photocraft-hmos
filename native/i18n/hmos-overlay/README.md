# HarmonyOS host translation overlay

Upstream v0.23.1 moved PhotoCraft UI catalogs from
`upstream/photocraft/crates/ui-egui/src/i18n/*.tsv` into the external
[storytold/craft-i18n](https://github.com/storytold/craft-i18n) repo
(`locales/<code>/photocraft.tsv`, pinned by `craft-i18n.rev`). The in-tree
TSVs were deleted with that change.

These files keep the HarmonyOS-only rows that used to live in those TSVs
(the `# HarmonyOS host additions retained during upstream sync` blocks),
so the strings are not lost. They are not yet embedded: `crates/ui-egui/build.rs`
reads `CRAFT_I18N_DIR`, and this overlay is not wired into that embedder yet.
Without `CRAFT_I18N_DIR`, the HarmonyOS UI falls back to English, same as a
PhotoCraft build that has not checked out craft-i18n.

To restore translated Hmos strings, either set `CRAFT_I18N_DIR` to a
craft-i18n checkout at the pinned revision and merge these rows into the
catalogs, or extend `build.rs` to load this overlay.
