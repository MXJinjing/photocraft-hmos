//! HarmonyOS sandbox path policy.
//!
//! The app can only reach its own directories and the folders the system file manager granted
//! it this session. Engine commands (scripts, actions, batch, plug-ins) that name a path outside
//! them are refused before they run, with an error that says to use the file picker, instead of
//! failing somewhere inside `std::fs`. Relative paths resolve in `filesDir/work` (see
//! `working_directory`) and must not climb out of it with `..`.
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

use serde_json::Value;

static ROOTS: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// Parameter names that carry a filesystem path, at any depth.
const PATH_KEYS: &[&str] = &[
    "path",
    "paths",
    "file",
    "files",
    "folder",
    "dir",
    "directory",
    "input",
    "output",
    "mapPath",
    "source",
    "destination",
    "filePath",
    "additionalPluginsFolder",
    "script",
    "xmp",
];

/// Lexically normalised absolute path (`.` and `..` folded); `None` for a relative path.
fn normalize(path: &Path) -> Option<PathBuf> {
    if !path.is_absolute() {
        return None;
    }
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    Some(out)
}

/// Starts the policy: the app's private files and its system-visible document folder.
pub fn init<P: AsRef<Path>>(roots: &[P]) {
    let mut r = ROOTS.lock().unwrap_or_else(|p| p.into_inner());
    r.clear();
    r.extend(roots.iter().filter_map(|p| normalize(p.as_ref())));
}

/// A folder the system file manager granted the app (or one the app made for a pick).
pub fn grant(path: &str) {
    if let Some(p) = normalize(Path::new(path)) {
        let mut r = ROOTS.lock().unwrap_or_else(|p| p.into_inner());
        if !r.contains(&p) {
            r.push(p);
        }
    }
}

/// `Err` with the reason when `path` is outside every allowed root.
pub fn check(path: &str) -> Result<(), String> {
    if path.is_empty() {
        return Ok(());
    }
    let p = Path::new(path);
    match normalize(p) {
        Some(abs) => {
            let roots = ROOTS.lock().unwrap_or_else(|p| p.into_inner());
            if roots.iter().any(|r| abs.starts_with(r)) {
                Ok(())
            } else {
                Err(format!(
                    "{path}: HarmonyOS apps cannot use paths outside the app sandbox. Choose the file or folder with the system file manager (the Choose… button) instead."
                ))
            }
        }
        None => {
            // Relative: stays inside the working directory.
            let mut depth: i32 = 0;
            for c in p.components() {
                match c {
                    Component::ParentDir => depth -= 1,
                    Component::Normal(_) => depth += 1,
                    _ => {}
                }
                if depth < 0 {
                    return Err(format!(
                        "{path}: a relative path cannot leave the app's working folder"
                    ));
                }
            }
            Ok(())
        }
    }
}

fn walk(value: &Value, under_path_key: bool) -> Result<(), String> {
    match value {
        Value::String(s) if under_path_key => check(s),
        Value::Array(items) => items.iter().try_for_each(|v| walk(v, under_path_key)),
        Value::Object(map) => map
            .iter()
            .try_for_each(|(k, v)| walk(v, PATH_KEYS.contains(&k.as_str()))),
        _ => Ok(()),
    }
}

/// Every path a command's params name (see [`PATH_KEYS`]) is inside the sandbox.
pub fn check_params(params: &Value) -> Result<(), String> {
    walk(params, false)
}

/// The engine's authorization gate (`Session::authorize`).
pub fn gate(id: &str, params: &Value) -> photocraft_engine::Result<()> {
    check_params(params).map_err(|e| photocraft_engine::EngineError::Other(format!("{id}: {e}")))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[path = "../tests/unit/sandbox_paths.rs"]
mod tests;
