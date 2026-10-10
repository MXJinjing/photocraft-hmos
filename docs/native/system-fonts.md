# HarmonyOS system fonts for text layers

2026-10-10

The text engine now scans `/system/fonts` on OHOS, using the same font database and family/style selectors as desktop PhotoCraft. Previously OHOS fell through to Linux's `/usr/share/fonts` paths, leaving only Inter, JetBrains Mono and the separately registered HarmonyOS Sans SC/TC faces visible.

The scan discovers TTF/OTF files and all faces in TTC/OTC collections. System families become available to the Type tool, Character panel, glyph browser and the `type.fonts` engine command. Existing family search, weight/italic/variable-axis metadata and PostScript-name lookup continue to use the common database. Font files stay on the device; no system font asset is bundled or committed. Actual availability depends on the system image's readable font files. Missing directories and invalid files are skipped, retaining the bundled fallback fonts.

The native UI's Chinese fallback registration remains in place so UI language rendering is preserved. HarmonyOS's font directory is handled separately from desktop Linux and its Flatpak paths.

Source reference: [OpenHarmony system font build declarations](https://github.com/openharmony/utils_system_resources/blob/master/BUILD.gn).

Verification: 80 text tests passed / 3 ignored, including a directory-discovery regression with nested fonts, case-insensitive file extensions, multiple weights, invalid-file rejection and PostScript resolution. Native unit suite: 23 passed. Offline Cargo metadata, text all-target Clippy with warnings denied, 28-crate layer validation, WASM checks, both OHOS release architectures and native HAP verification passed.

API 20 simulator: independent `io.github.storytold.photocraft.savetitletest` bundle. Fixed 2880 × 1920 screenshots verify the [original four families](system-fonts-before.jpeg), [expanded HarmonyOS families](system-fonts-after.jpeg), [Noto Sans families](system-fonts-scroll.jpeg), [Noto Serif CJK collection faces](system-fonts-serif-list.jpeg), and [Noto Serif CJK SC selected and rendering text](system-fonts-render.jpeg). No real Pad installation or relaunch. The first emulator session suffered system-service timeouts while the corpus build used high parallelism; lower compiler concurrency and an emulator cold restart restored normal operation.

Required `cargo xtask test-corpus`: 78 test groups completed, 1,968 passed / 20 ignored / 0 failed across the PSD, codecs, IO and engine crates with pinned Photoshop, psd-tools, ag-psd, PngSuite and HEIF fixtures.
