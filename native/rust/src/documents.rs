//! Local working documents; external destinations are one-shot Save As copies.
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

pub struct Documents {
    pub dir: PathBuf,
    targets: HashMap<u64, String>,
}
impl Documents {
    pub fn new(directory: &str) -> Self {
        Self {
            dir: PathBuf::from(directory),
            targets: HashMap::new(),
        }
    }
    pub fn owns(&self, name: &str) -> bool {
        let path = Path::new(name);
        path.parent() == Some(self.dir.as_path())
            && path.file_name().is_some()
            && !path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
    }
    pub fn select(&mut self, id: u64, path: &str) -> Result<String, String> {
        // 本地目标按文档 id 缓存；同名文档也不会共享首次保存时选定的路径。
        if !self.owns(path) {
            return Err("Invalid application document path".into());
        }
        self.targets.insert(id, path.into());
        Ok(path.into())
    }
    pub fn target(&self, id: u64) -> Option<String> {
        self.targets.get(&id).cloned()
    }
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[path = "../tests/unit/documents.rs"]
mod tests;
