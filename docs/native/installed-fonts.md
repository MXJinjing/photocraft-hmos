# HarmonyOS user-installed fonts / 用户安装的字体

2026-11-01

Fonts the user installs through **Settings > System > 字体安装和管理** now appear in the Type tool, Character panel, glyph browser and `type.fonts`, alongside the `/system/fonts` families (see [system-fonts.md](system-fonts.md)).

系统「字体安装和管理」里安装的字体不在 `/system/fonts`，目录扫描找不到。现在启动时由鸿蒙宿主列出这些字体的文件并交给文字引擎注册。

## How

- `native/rust/src/installed_fonts.rs` (OHOS only) calls `native_drawing` (API 14+): `OH_Drawing_GetSystemFontFullNamesByType(INSTALLED)` for the full names, then `OH_Drawing_GetFontDescriptorByFullName(name, INSTALLED)` for each file path. Off OHOS it returns nothing.
- The paths go to `photocraft_text::extra_fonts::set_extra_font_paths` (the text crate forbids `unsafe`, so the FFI stays in the host). Files that can't be opened are logged and skipped.
- `FontDb::load_system_fonts` loads these paths together with the scanned system fonts, so families, weights, TTC faces and PostScript lookup behave the same. Called in the worker before the app (and its font database) is created; the count is sent as an `Installed fonts: N` diagnostic.
- Desktop/web builds are unaffected: no extra paths are set there.

## Needs real-device verification / 待实机验证

- Whether installed font paths returned by the descriptor are readable from the app sandbox. If not, the diagnostic shows `Installed fonts: 0` and stderr lists `skipping font …`.
- Fonts installed while the app runs appear only after a restart.

Tests: `extra_fonts` readable/dedupe test (text crate, 287 passed / 5 ignored), `installed_fonts` UTF-16 decode + non-OHOS-empty tests (native; the native test target currently doesn't compile on `upstream-track` due to the known `IMEOutput`/`DocWindow` test errors fixed in `fix/egui037-tests`). `cargo check` passes for x86_64 and aarch64 OHOS.
