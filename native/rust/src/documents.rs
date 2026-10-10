//! Ordinary Save targets: local paths or host-authorized picker tokens.
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
        // Tokens carry no ambient filesystem authority: the host retains the matching URI.
        let parts: Vec<_> = path.split('/').collect();
        let picker_target = matches!(parts.as_slice(), ["save-default", request, name]
            if request.parse::<u64>().is_ok_and(|id| id > 0)
                && !name.is_empty() && *name != "." && *name != ".." && !name.contains('\\'));
        if !self.owns(path) && !picker_target {
            return Err("Invalid application document path".into());
        }
        self.targets.insert(id, path.into());
        Ok(path.into())
    }
    pub fn existing(&mut self, id: u64, source: Option<&str>) -> Option<String> {
        if let Some(target) = self.target(id) {
            return Some(target);
        }
        let path = source.filter(|path| {
            self.owns(path)
                && Path::new(path).extension().is_some_and(|ext| {
                    ext.eq_ignore_ascii_case("psd") || ext.eq_ignore_ascii_case("psb")
                })
        })?;
        self.select(id, path).ok()
    }
    pub fn target(&self, id: u64) -> Option<String> {
        self.targets.get(&id).cloned()
    }
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[path = "../tests/unit/documents.rs"]
mod tests;
