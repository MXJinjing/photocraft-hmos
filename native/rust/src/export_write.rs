//! Route engine exports through CoreFileKit; public folders cannot rename/unlink.
use std::path::Path;

fn publish(
    private: &Path,
    path: &Path,
    bytes: &[u8],
    write: impl FnOnce(&str, &[u8]) -> Result<(), String>,
) -> Option<std::io::Result<()>> {
    // Private caches/recovery keep their crash-safe POSIX atomic replacement.
    if !path.is_absolute() || path.starts_with(private) {
        return None;
    }
    Some((|| {
        let name = path
            .to_str()
            .ok_or_else(|| std::io::Error::other("Invalid export path"))?;
        write(name, bytes).map_err(std::io::Error::other)
    })())
}

#[cfg(target_env = "ohos")]
pub fn install(private: &str) {
    static PRIVATE: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    let _ = PRIVATE.set(private.into());
    photocraft_format::atomic::set_host_writer(|path, bytes| {
        let private = PRIVATE.get()?;
        publish(private, path, bytes, |name, data| {
            crate::sandbox_paths::check(name)?;
            crate::bridge::request("export_write", name, data).map(|_| ())
        })
    });
}

#[cfg(test)]
#[path = "../tests/unit/export_write.rs"]
mod tests;
