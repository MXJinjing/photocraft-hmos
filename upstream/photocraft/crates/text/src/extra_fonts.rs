//! Extra font files a host hands in, e.g. the fonts the user installed through HarmonyOS Settings >
//! System > 字体安装和管理 (photocraft-hmos). They live outside the scanned font folders; the host
//! lists them (on HarmonyOS via `native_drawing`, which needs FFI this crate forbids) and calls
//! [`set_extra_font_paths`] before the font database loads. They then go through the same
//! registration as the scanned system fonts.

use std::path::PathBuf;
use std::sync::Mutex;

static EXTRA: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// Sets the extra font files to load with the system fonts. Unreadable files are logged and
/// skipped. Takes effect for font databases that load system fonts afterwards.
pub fn set_extra_font_paths(paths: impl IntoIterator<Item = PathBuf>) {
    let readable = readable_font_paths(paths);
    if let Ok(mut g) = EXTRA.lock() {
        *g = readable;
    }
}

/// The extra font files set by the host.
pub(crate) fn extra_font_paths() -> Vec<PathBuf> {
    EXTRA.lock().map(|g| g.clone()).unwrap_or_default()
}

/// Keeps the paths that can be opened for reading, logging (and skipping) the rest, deduplicated
/// in order.
pub(crate) fn readable_font_paths(paths: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for p in paths {
        if out.contains(&p) {
            continue;
        }
        match std::fs::File::open(&p) {
            Ok(_) => out.push(p),
            Err(e) => eprintln!("photocraft-text: skipping font {}: {e}", p.display()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readable_paths_skip_missing_and_dedupe() {
        let dir = std::env::temp_dir().join(format!("pc-extra-fonts-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.ttf");
        std::fs::write(&a, b"x").unwrap();
        let missing = dir.join("missing.ttf");
        let got = readable_font_paths([a.clone(), missing, a.clone()]);
        assert_eq!(got, [a]);
        std::fs::remove_dir_all(&dir).ok();
    }
}
