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
| A file the command reads | `DocumentViewPicker.select` (`import` request — no files/gallery chooser; fd → bytes) | The bytes are copied into the sandbox, `filesDir/imports/`, `filesDir/plugins/` or `filesDir/icc/`; the field shows that copy's path |
| A folder the command reads | `folder` request (`input`) | 2in1/PC: the granted folder's path. Tablet: see below |
| A folder the command writes | `folder` request (`output`) | 2in1/PC: the granted folder's path. Tablet: DOWNLOAD document folder (or a named subfolder) |
| A file the command writes | `folder` request (`output`) | chosen folder + the field's current file name |

Code: `upstream/photocraft/crates/ui-egui/src/path_pick.rs` (widget, field table, staging),
`file_dialog.rs` (sync host `Folder` branch now calls `Services::pick_folder` instead of
cancelling; `ask_output_folder`), `entry/src/main/ets/folderPicker.ets` (ArkTS side).

### Choosing a folder

`DocumentSelectOptions.selectMode = FOLDER` (`SystemCapability.FileManagement.UserFileService.FolderSelection`)
is advertised only on **2in1/PC** in API 20 (SDK d.ts: "Only 2-in-1 devices are supported"). The host
always **tries** FOLDER mode first (and catches failure), then falls back:

- **2in1/PC**: the system file manager in folder mode; `FileUri(uri).path` goes back to Rust.
- **Tablet / no FolderSelection, input folder**: multi-file select (up to 500). The files are copied
  into a new sandbox folder `filesDir/picked/<time>/`, which is the answer. **Folder fields never
  open a single-file picker** — that would look like “只能选一个文件”.
- **Tablet / no FolderSelection, output folder**: there is no granted-folder UI, so the answer is the
  app's DOWNLOAD document folder (`~/Downloads/<bundleName>`, from `DocumentPickerMode.DOWNLOAD`),
  or `filesDir/Documents` when DOWNLOAD failed at startup. Defaults already point at named
  subfolders under that root (see below).

Folders the picker returns are added to the sandbox allow-list for this run
(`sandbox_paths::grant`). The grant is the system's temporary one: it ends when the app exits
(no `persistPermission`, which needs `ohos.permission.FILE_ACCESS_PERSIST`).

## Default output locations (`~/Downloads/<bundle>/…`)

On HarmonyOS, `Services::documents_dir` is the DOWNLOAD application directory (the same path as
`~/Downloads/io.github.mxjinjing.photocraft` on device). At startup the host creates these
subfolders and batch/export dialogs default into them:

| Subfolder | Used by |
|---|---|
| `processed` | File › Scripts › Image Processor… (`output`) |
| `batch` | File › Automate › Batch… (`output`) |
| `corrected` | File › Automate › Lens Correction… (`output`) |
| `exports` | Export Layers / Layer Comps / Artboards to Files, Artboards to PDF, Color Lookup Tables, Paths to Illustrator, Create Droplet `.pcdroplet` |
| `droplet-output` | Create Droplet (`output`) |
| `package` | File › Package… (`dir`) |

Input folders on HarmonyOS default to empty (user must Choose…). Desktop builds still default next
to the active document.

When DOWNLOAD authorization fails at startup, the same layout is created under
`filesDir/Documents` instead.

## Entry points (folder vs file)

### Folders (Choose… → folder picker / tablet degradation above)

- **Preferences › Plug-ins › Additional Plug-ins Folder**, `plugin.reload` path → `PluginFolder`
- **Batch / Image Processor / Lens Correction** `input` → `InputFolder`; `output` → `OutputFolder`
- **Contact Sheet II** `input` → `InputFolder`
- **Photomerge / Merge to HDR Pro** `paths` → `InputFolder`
- **Export Layers / Layer Comps / Artboards to Files**, **Package**, **dataSetsAsFiles** `dir` → `OutputFolder`
- **Create Droplet** `output` → `OutputFolder`
- **Preferences › Scratch Disks** → `OutputFolder`
- **Load Files into Stack…** Browse… in Folder mode → folder picker
- **File › Import › Video Frames to Layers** → folder picker (no dialog field)

### Files (Choose… → file picker, staged into the sandbox)

- **Install Plug-in…** `.wasm` → `filesDir/plugins/`
- **Displace** `mapPath`, Color Lookup `file`, Variables pixel replacement → `filesDir/imports/`
- ICC **Import…** → `filesDir/icc/`

### Output files (Choose… → folder, keep file name)

- Artboards to PDF, Color Lookup Tables, Paths to Illustrator, Create Droplet path, History Log file path

### Engine gate

The host sets `Session::authorize = sandbox_paths::gate`. Absolute paths outside `filesDir`, the
document folder or a granted folder are refused before they run.

### Open / Save / Import

- **File › Open** (and Open As / homepage Open): bridge kind `open` → files-vs-gallery
  chooser, then DocumentViewPicker or PhotoViewPicker (see `open-source-picker.md`).
- **Import / path_pick / presets / scripts / Place…**: bridge kind `import` →
  DocumentViewPicker only (`Services::pick_import`). No "从哪里导入" chooser.
- Save: DocumentViewPicker + fd + save tokens. DOWNLOAD failure falls back to
  `filesDir/Documents`. DOWNLOAD failure falls back to
`filesDir/Documents`.

## Not covered / risks

- Rust reads/writes a granted folder by path; not yet verified on-device for FOLDER-picker grants.
- Atomic temp+rename into DOWNLOAD may be refused (open/create only).
- Upstream UI edits are behind `path_pickers_only`, not `cfg(ohos)`.

### File › Import (and related one-shot imports)

These never show a typed-path dialog on HarmonyOS; they call `pick_file_bytes` →
`pick_import` → system file manager:

- File › Import › Notes…
- File › Import › Variable Data Sets… (CSV)
- File › Import › Video Frames to Layers… (folder picker)
- Edit › Presets › Export/Import Presets… (import action)
- Scripts / Actions load, keyboard-shortcut `.kys`, swatches, gradients, Place Embedded/Linked
