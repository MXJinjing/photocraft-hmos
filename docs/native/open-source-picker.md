# Open source picker (file manager vs gallery)

When the user opens a document (File › Open, homepage Open, or Ctrl+O / Cmd+O),
the HarmonyOS host no longer jumps straight into `DocumentViewPicker`.

## Flow

1. Rust `pick_open` sends a bridge request with `kind: "open"` (see `native/rust/src/lib.rs`).
2. ArkTS `Index.openFile` shows a native ArkUI **ActionSheet** via
   `this.getUIContext().showActionSheet` (not `@CustomDialog`):
   - **从文件管理选取** → `picker.DocumentViewPicker.select` (`@kit.CoreFileKit`)
   - **从图库选取** → `photoAccessHelper.PhotoViewPicker.select` (`@kit.MediaLibraryKit`, images only)
   - **取消** / mask dismiss → reply `cancelled`
3. The chosen URI is opened read-only and handed to native via `native.replyFile`, same as before.

External “Open with PhotoCraft” (Want / `openInbox`) and drag-and-drop are unchanged;
they never go through `pick_open`.

## Why ActionSheet

The first implementation used `@CustomDialog` + a locally constructed
`CustomDialogController`. On device, tapping any dialog button crashed the app
(typical when the controller is not bound to the page UIContext and is opened
from the async native bridge). `UIContext.showActionSheet` is the documented
replacement for `ActionSheet.show` and runs on the bound UI context.

## Device check

On a connected tablet / 2in1:

```bash
# from photocraft-hmos worktree on branch feat/open-source-picker-dialog
scripts/dev.sh build   # or: scripts/dev.sh run  (build + install + launch)
```

Then trigger Open three ways (menu, homepage, Ctrl+O) and confirm the ActionSheet
appears before either system picker. Gallery path needs no extra permission
(Picker temporary URI).
