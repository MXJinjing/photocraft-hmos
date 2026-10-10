//! Relative script paths belong in the writable application sandbox, not `/`.
use std::path::{Path, PathBuf};

pub fn initialize(files: &str) -> Result<PathBuf, String> {
    // 公开下载目录的文件授权不等于完整 POSIX 目录权限，脚本相对路径使用可写沙箱。
    let root = Path::new(files);
    if !root.is_absolute() {
        return Err("Application files directory must be absolute".into());
    }
    let work = root.join("work");
    std::fs::create_dir_all(&work).map_err(|error| {
        format!(
            "Unable to create working directory {}: {error}",
            work.display()
        )
    })?;
    // The process-wide directory is selected once, before editor startup. Keep
    // preferences and document destinations absolute and independent of it.
    std::env::set_current_dir(&work).map_err(|error| {
        format!(
            "Unable to enter working directory {}: {error}",
            work.display()
        )
    })?;
    std::env::current_dir().map_err(|error| format!("Unable to read working directory: {error}"))
}

#[cfg(test)]
#[path = "../tests/unit/working_directory.rs"]
mod tests;
