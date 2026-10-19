# Sandbox path pickers (HarmonyOS)

A HarmonyOS app runs in a sandbox: it cannot read or write the user's files by path. Every
place in the editor that took a typed path now takes the path **only from the system file
manager**: on HarmonyOS the field is disabled (read-only, shows the chosen value) and a
**选择… (Choose…)** button sits after it. Desktop and web builds are unchanged: the switch is
the host capability `Services::path_pickers_only`, which only the HarmonyOS host
(`native/rust/src/lib.rs`) sets.

## How a pick works

| Field takes | Picker | Field gets |
|---|---|---|
| A file the command reads | `DocumentViewPicker.select` (existing `open` request, fd → bytes) | The bytes are copied into the sandbox, `filesDir/imports/`, `filesDir/plugins/` or `filesDir/icc/`; the field shows that copy's path |
| A folder the command reads | new `folder` request (`input`) | 2in1/PC: the granted folder's path. Tablet: see below |
| A folder the command writes | new `folder` request (`output`) | 2in1/PC: the granted folder's path. Tablet: the app's document folder |
| A file the command writes | `folder` request (`output`) | chosen folder + the field's current file name |

Code: `upstream/photocraft/crates/ui-egui/src/path_pick.rs` (widget, field table, staging),
`file_dialog.rs` (sync host `Folder` branch now calls `Services::pick_folder` instead of
cancelling; `ask_output_folder`), `entry/src/main/ets/folderPicker.ets` (ArkTS side).

### Choosing a folder

`DocumentSelectOptions.selectMode = FOLDER` (`SystemCapability.FileManagement.UserFileService.FolderSelection`)
exists only on **2in1/PC** in API 20 (SDK d.ts: "Only 2-in-1 devices are supported"). The host
checks `canIUse(...)` at run time:

- **2in1/PC**: the system file manager in folder mode; `FileUri(uri).path` goes back to Rust.
- **Tablet, input folder**: the file manager in multi-file mode (up to 500); the files are copied
  into a new sandbox folder `filesDir/picked/<time>/`, which is the answer.
- **Tablet, output folder**: there is no way to get a granted folder, so the answer is the app's
  system-visible document folder (DOWNLOAD/PhotoCraft, or `filesDir/Documents` when DOWNLOAD
  failed at startup).

Folders the picker returns are added to the sandbox allow-list for this run
(`sandbox_paths::grant`). The grant is the system's temporary one: it ends when the app exits
(no `persistPermission`, which needs `ohos.permission.FILE_ACCESS_PERSIST`).

## Entry points

### Plug-ins
- **Filter › Plug-ins › Install Plug-in…** (`plugin.install`, `path`): choose a `.wasm`; it is
  copied to `filesDir/plugins/` and installed from there.
- **Preferences › Plug-ins › Additional Plug-ins Folder** (`plugIns.additionalPluginsFolder`):
  choose a folder; its `*.wasm` are copied into `filesDir/plugins/` and the preference names that
  folder, so the plug-ins still load after the folder grant ends.
- `plugin.reload` `path` (when shown in a dialog): same as the preference.

### Scripts / automation / batch
- **File › Automate › Batch…**, **File › Scripts › Image Processor…**, **Lens Correction…**:
  `input` → input folder, `output` → output folder.
- **Photomerge…**, **Merge to HDR Pro…**: `paths` → input folder.
- **Export Layers / Layer Comps / Artboards to Files…**: `dir` → output folder.
- **Artboards to PDF…**, **Color Lookup Tables…**: `path` → output folder + file name.
- **Load Files into Stack…**: Browse… (folder mode) now uses the folder picker; files mode copies
  the picked file into `filesDir/imports/` instead of failing with "reads files from disk".
- **Image › Variables › Data Sets** pixel replacement: choose an image (copied to `imports/`).
- **Filter › Distort › Displace** `mapPath`, Color Lookup `file` in a generic dialog: file picker.
- **Preferences › Scratch Disks** (`scratchDisks.disks[].path`): each disk path is an output
  folder (Choose…); the default `(system temp)` stays until a folder is chosen.
- **Preferences › History Log › File** (`historyLog.filePath`): output folder + file name. The
  host now has an `append_text` service (it had none, so the log was never written), restricted
  to the sandbox and granted folders.
- **Engine gate**: the host sets `Session::authorize = sandbox_paths::gate`. Any command (from
  a script, an action, batch or the UI) whose `path`/`paths`/`file`/`input`/`output`/`dir`/
  `folder`/`mapPath`/`filePath`/`additionalPluginsFolder`/… parameter names an absolute path
  outside `filesDir`, the document folder or a granted folder is refused before it runs:
  "HarmonyOS apps cannot use paths outside the app sandbox. Choose the file or folder with the
  system file manager (the Choose… button) instead." Relative paths (resolved in
  `filesDir/work`) may not climb out with `..`. Save/export picker tokens (`save-as/…`) are
  relative and pass.

### ICC profiles
- `installed_profiles::profile_dirs()` has an OHOS branch: only the app's own folder
  `filesDir/icc` (set by the host via `set_app_profile_dir`), no system folders.
- Dialogs with a profile menu (Print, Proof Setup, Convert/Assign Profile) show **Import…**
  beside it on HarmonyOS: choose an `.icc`/`.icm`, it is copied into `filesDir/icc`, the scan
  cache is refreshed and the menu selects it.

### Open / Save
- File › Open / Save / Save As / Export already use DocumentViewPicker + fd + save tokens
  (unchanged).
- Startup: when the DOWNLOAD-mode document folder can't be obtained, the host no longer fails;
  documents go to `filesDir/Documents` and the save picker opens at its own default location.
- `LocalSaveDialog.ets` (typed file name dialog, unreferenced) and its script test were removed.

## Not covered / risks
- Rust reads and writes a granted folder by its path (`/storage/Users/currentUser/...`). This is
  how the existing document folder already works, but it has not been verified on a device for
  folders granted by the FOLDER picker; if `std::fs` is refused there, batch output to a chosen
  folder fails with that error message.
- Writing into the DOWNLOAD document folder from Rust with temp-file + rename (some exporters
  use atomic writes) may be refused; that folder only allows open/create.
- Upstream UI edits are not behind `cfg(target_env = "ohos")` but behind the host flag, so
  desktop tests cover them; the desktop app never sets the flag.
