//! Path fields on sandboxed hosts (HarmonyOS).
//!
//! A sandboxed app cannot read or write the user's files by path, so a typed path is useless
//! there. When the host sets [`crate::Services::path_pickers_only`], every field that takes a
//! file or folder path is drawn read-only, with a "Choose…" button beside it that opens the
//! system file manager. The answer fills the field:
//!
//! - a file the command reads is copied into the host's sandbox ([`crate::Services::sandbox_dir`],
//!   under `imports/`, `plugins/` or `icc/`) and the field names that copy;
//! - a folder is whatever path the host's folder picker answered (a folder the system granted
//!   the app, or one of the app's own folders).
//!
//! Other hosts keep the ordinary text field: [`locked_field`] draws nothing and returns `false`.
//!
//! The button only records a [`Request`] in egui memory; [`crate::dialogs::show`] starts it once
//! the dialog's fields are written back, so a picker answered within the frame (synchronous
//! hosts) isn't overwritten by the frame's copy of the fields.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use crate::PhotocraftApp;

/// What a path field takes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PickKind {
    /// A file the command reads (any extension when `extensions` is empty). Sandboxed hosts copy
    /// it under `stage`.
    File { extensions: Vec<String>, stage: Stage },
    /// A folder the command reads from.
    InputFolder,
    /// A folder the command writes into.
    OutputFolder,
    /// A file the command writes: its folder is chosen, the field's file name (else
    /// `default_name`) is kept.
    OutputFile { default_name: String },
    /// A folder of `*.wasm` plug-ins. Sandboxed hosts copy the plug-ins into `plugins/` and the
    /// field names that folder, so they still load after the folder grant has lapsed.
    PluginFolder,
}

/// Where a picked file is copied in the sandbox.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Imports,
    Plugins,
    Profiles,
}

impl Stage {
    pub fn folder(self) -> &'static str {
        match self {
            Stage::Imports => "imports",
            Stage::Plugins => "plugins",
            Stage::Profiles => "icc",
        }
    }
}

/// A "Choose…" click: the dialog field (a JSON pointer into the dialog's fields) to fill.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub pointer: String,
    pub kind: PickKind,
}

fn only_id() -> egui::Id {
    egui::Id::new("photocraft-path-pickers-only")
}

fn request_id() -> egui::Id {
    egui::Id::new("photocraft-path-pick-request")
}

/// Record, for this frame's widgets, whether path fields are picker-only.
pub fn publish(ctx: &egui::Context, on: bool) {
    ctx.data_mut(|d| d.insert_temp(only_id(), on));
}

/// Whether path fields are picker-only (see [`publish`]).
pub fn pickers_only(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(only_id())).unwrap_or(false)
}

/// Ask for a picker for the field at `pointer` (started after the dialog is drawn).
pub fn request(ctx: &egui::Context, pointer: &str, kind: PickKind) {
    ctx.data_mut(|d| d.insert_temp(request_id(), Request { pointer: pointer.to_string(), kind }));
}

/// The request recorded this frame, if any.
pub fn take_request(ctx: &egui::Context) -> Option<Request> {
    ctx.data_mut(|d| {
        let r = d.get_temp::<Request>(request_id());
        d.remove::<Request>(request_id());
        r
    })
}

/// The "Choose…" button's label.
pub fn choose_label() -> &'static str {
    tl!("Choose…")
}

/// Draws the picker-only version of a path field (a disabled field showing `value`, then a
/// "Choose…" button) and returns `true`, when path fields are picker-only and `kind` is a path
/// field. Otherwise draws nothing and returns `false`: the caller draws its ordinary field.
pub fn locked_field(ui: &mut egui::Ui, kind: Option<PickKind>, pointer: &str, value: &str, width: f32) -> bool {
    let Some(kind) = kind.filter(|_| pickers_only(ui.ctx())) else { return false };
    ui.horizontal(|ui| {
        let mut shown = value.to_string();
        ui.add_enabled(false, egui::TextEdit::singleline(&mut shown).desired_width((width - 70.0).max(80.0))).on_disabled_hover_text(value);
        if crate::widgets::secondary_button(ui, choose_label(), 64.0).clicked() {
            request(ui.ctx(), pointer, kind);
        }
    });
    true
}

const IMAGE_EXTENSIONS: &[&str] = &["psd", "psb", "png", "jpg", "jpeg", "tif", "tiff", "webp", "bmp", "gif"];

fn file(extensions: &[&str], stage: Stage) -> Option<PickKind> {
    Some(PickKind::File { extensions: extensions.iter().map(|e| (*e).to_string()).collect(), stage })
}

/// The path field `key` of `command`'s dialog (`"prefs"` with a dotted preference path for
/// Preferences), or `None` when it isn't a path.
pub fn kind_for(command: &str, key: &str) -> Option<PickKind> {
    match (command, key) {
        ("plugin.install", "path") => file(&["wasm"], Stage::Plugins),
        ("plugin.reload", "path") | ("prefs", "plugIns.additionalPluginsFolder") => Some(PickKind::PluginFolder),
        ("prefs", "historyLog.filePath") => Some(PickKind::OutputFile { default_name: "PhotoCraft History Log.txt".into() }),
        ("filter.distort.displace", "mapPath") => file(&["psd", "psb"], Stage::Imports),
        (c, "file") if c.ends_with(".colorLookup") => file(&["cube", "3dl", "look"], Stage::Imports),
        ("image.variables", "pixelReplacement") => file(IMAGE_EXTENSIONS, Stage::Imports),
        ("file.automate.batch" | "file.scripts.imageProcessor" | "file.automate.lensCorrection", "input") => Some(PickKind::InputFolder),
        ("file.automate.batch" | "file.scripts.imageProcessor" | "file.automate.lensCorrection", "output") => Some(PickKind::OutputFolder),
        ("file.automate.photomerge" | "file.automate.mergeToHdrPro", "paths") => Some(PickKind::InputFolder),
        ("file.export.layersToFiles" | "file.export.layerCompsToFiles" | "file.export.artboardsToFiles", "dir") => Some(PickKind::OutputFolder),
        ("file.export.artboardsToPdf", "path") => Some(PickKind::OutputFile { default_name: "Artboards.pdf".into() }),
        ("file.export.colorLookupTables", "path") => Some(PickKind::OutputFile { default_name: "Color Lookup.cube".into() }),
        _ => None,
    }
}

/// A picked file's name reduced to a safe file name (no folders, no control characters).
pub fn safe_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("").trim();
    let clean: String = base.chars().map(|c| if c.is_control() || matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c }).collect();
    if clean.is_empty() || clean == "." || clean == ".." { "file".into() } else { clean }
}

/// The sandbox folder for `stage` under `sandbox`, created if missing.
pub fn stage_dir(sandbox: &Path, stage: Stage) -> Result<PathBuf, String> {
    let dir = sandbox.join(stage.folder());
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(dir)
}

/// Copies a picked file's bytes into the sandbox; the copy's path.
pub fn stage_bytes(sandbox: &Path, stage: Stage, name: &str, bytes: &[u8]) -> Result<String, String> {
    let path = stage_dir(sandbox, stage)?.join(safe_name(name));
    std::fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path.to_string_lossy().into_owned())
}

/// Copies the `*.wasm` files of `from` into the sandbox's `plugins/`; that folder's path.
pub fn stage_plugins(sandbox: &Path, from: &Path) -> Result<String, String> {
    let dir = stage_dir(sandbox, Stage::Plugins)?;
    if from == dir {
        return Ok(dir.to_string_lossy().into_owned());
    }
    let entries = std::fs::read_dir(from).map_err(|e| format!("{}: {e}", from.display()))?;
    let mut copied = 0;
    for e in entries.flatten() {
        let p = e.path();
        if p.is_file() && p.extension().and_then(|x| x.to_str()).is_some_and(|x| x.eq_ignore_ascii_case("wasm")) {
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            std::fs::copy(&p, dir.join(safe_name(&name))).map_err(|e| format!("{}: {e}", p.display()))?;
            copied += 1;
        }
    }
    if copied == 0 {
        return Err(format!("{}: no .wasm plug-ins in this folder", from.display()));
    }
    Ok(dir.to_string_lossy().into_owned())
}

/// Sets the dialog field at `pointer` (`/a/b/0/c`), creating the last key of an object.
pub fn set_pointer(fields: &mut Map<String, Value>, pointer: &str, value: Value) -> Result<(), String> {
    let mut root = Value::Object(std::mem::take(fields));
    let result = (|| {
        let (parent, last) = pointer.rsplit_once('/').ok_or("bad field pointer")?;
        let last = last.replace("~1", "/").replace("~0", "~");
        let slot = if parent.is_empty() { Some(&mut root) } else { root.pointer_mut(parent) };
        match slot {
            Some(Value::Object(o)) => {
                o.insert(last, value);
                Ok(())
            }
            Some(Value::Array(a)) => {
                let i: usize = last.parse().map_err(|_| "bad field pointer")?;
                *a.get_mut(i).ok_or("the field is gone")? = value;
                Ok(())
            }
            _ => Err("the field is gone".to_string()),
        }
    })();
    if let Value::Object(o) = root {
        *fields = o;
    }
    result
}

fn fill(app: &mut PhotocraftApp, dialog: u64, pointer: &str, value: String) -> Result<Value, String> {
    let d = app.ui.dialog_mut(dialog).ok_or("the dialog was closed")?;
    set_pointer(&mut d.fields, pointer, json!(value))?;
    Ok(Value::Null)
}

fn sandbox(app: &PhotocraftApp) -> Option<PathBuf> {
    app.services.sandbox_dir.as_deref().map(PathBuf::from)
}

/// Shows the picker for `request` on dialog `dialog`; the answer fills the field. Errors (but
/// not Cancel) go to the status bar.
pub fn start(app: &mut PhotocraftApp, dialog: u64, request: Request) {
    let Request { pointer, kind } = request;
    let current = app
        .ui
        .dialogs
        .iter()
        .find(|d| d.id == dialog)
        .and_then(|d| Value::Object(d.fields.clone()).pointer(&pointer).and_then(Value::as_str).map(str::to_string));
    let result = match kind {
        PickKind::File { extensions, stage } => {
            let exts: Vec<&str> = extensions.iter().map(String::as_str).collect();
            app.pick_file_bytes_filtered(&exts, move |app, name, bytes| {
                let path = match sandbox(app) {
                    Some(root) => stage_bytes(&root, stage, &name, &bytes)?,
                    // Desktop: the name is the file's path.
                    None => name,
                };
                if stage == Stage::Profiles {
                    photocraft_engine::installed_profiles::refresh();
                }
                fill(app, dialog, &pointer, path)
            })
        }
        PickKind::InputFolder => app.ask_folder(move |app, path| fill(app, dialog, &pointer, path)),
        PickKind::OutputFolder => app.ask_output_folder(move |app, path| fill(app, dialog, &pointer, path)),
        PickKind::OutputFile { default_name } => {
            let name = current
                .as_deref()
                .and_then(|c| Path::new(c).file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .filter(|n| !n.is_empty())
                .unwrap_or(default_name);
            app.ask_output_folder(move |app, dir| fill(app, dialog, &pointer, Path::new(&dir).join(&name).to_string_lossy().into_owned()))
        }
        PickKind::PluginFolder => app.ask_folder(move |app, dir| {
            let path = match sandbox(app) {
                Some(root) => stage_plugins(&root, Path::new(&dir))?,
                None => dir,
            };
            fill(app, dialog, &pointer, path)
        }),
    };
    if let Err(e) = result
        && e != crate::file_dialog::CANCELLED
    {
        app.ui.status = e;
        app.ui.status_error = true;
    }
}

#[cfg(test)]
#[path = "path_pick_tests.rs"]
mod tests;
