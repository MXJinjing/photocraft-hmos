# Open source picker (file manager vs gallery)

When the user opens a document (File › Open, homepage Open, or Ctrl+O / Cmd+O),
the HarmonyOS host no longer jumps straight into `DocumentViewPicker`.

## Flow

1. Rust `pick_open` sends a bridge request with `kind: "open"` (see `native/rust/src/lib.rs`).
2. ArkTS `Index.openFile` shows a centered ArkUI dialog via
   `this.getUIContext().getPromptAction().openCustomDialog` with `ComponentContent`:
   - **从文件管理导入** → `picker.DocumentViewPicker.select` (`@kit.CoreFileKit`)
   - **从图库导入** → `photoAccessHelper.PhotoViewPicker.select` (`@kit.MediaLibraryKit`, images only)
   - **取消** / mask dismiss / Back → reply `cancelled` after the dialog disappears
   - The three compact text buttons have no icons or descriptions. The dialog uses the existing egui `chrome` color for its background; button text and fills follow that color's brightness.
3. The chosen URI is opened read-only and handed to native via `native.replyFile`, same as before.

External “Open with PhotoCraft” (Want / `openInbox`) and drag-and-drop are unchanged;
they never go through `pick_open`.

## Dialog lifecycle

The first centered implementation used `@CustomDialog` with a locally constructed
`CustomDialogController`. On device, tapping any button crashed the app. The new
content and prompt action are both created from the page UIContext. A source
selection closes the dialog first; `onDidDisappear` then resolves the choice so
the system picker opens only after the dialog is gone. Dismissing the mask
resolves as cancellation.

## Device check

On a connected tablet / 2in1:

```bash
# from photocraft-hmos worktree on branch feat/open-source-picker-dialog
scripts/dev.sh build   # or: scripts/dev.sh run  (build + install + launch)
```

Then trigger Open three ways (menu, homepage, Ctrl+O) and confirm the centered dialog
appears before either system picker. Check both choices, Cancel, mask dismissal,
and that tapping an option does not crash. Gallery path needs no extra permission
(Picker temporary URI).
