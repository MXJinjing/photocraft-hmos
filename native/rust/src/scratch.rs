//! Resolve the OHOS sandbox cache before the engine applies its scratch preferences.
use std::path::{Path, PathBuf};

pub(crate) fn directory(cache: &str, files: &str) -> Result<PathBuf, String> {
    // Old host configurations did not pass cacheDir. Keep them sandboxed too.
    let root = if cache.is_empty() {
        Path::new(files).join("cache")
    } else {
        PathBuf::from(cache)
    };
    if !root.is_absolute() {
        return Err("Scratch cache directory must be absolute".into());
    }
    Ok(root.join("photocraft-scratch"))
}

#[cfg(target_env = "ohos")]
pub(crate) fn initialize(cache: &str, files: &str) -> Result<(), String> {
    let dir = directory(cache, files)?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Scratch cache {}: {e}", dir.display()))?;
    // configure() creates the actual scratch file and measures statvfs, reserving max(1 GiB,
    // 5% of the volume), plus allocator overhead. Unknown/full/unwritable disks earn no room.
    photocraft_raster::spill::set_host_scratch_dir(dir)
}

#[cfg(test)]
#[path = "../tests/unit/scratch.rs"]
mod tests;
