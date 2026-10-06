//! Minimal owned test-directory fixture; no additional runtime dependency.
use std::path::{Path, PathBuf};

pub struct TestDirectory {
    path: PathBuf,
}
impl TestDirectory {
    pub fn new() -> std::io::Result<Self> {
        let path = std::env::temp_dir().join(format!("agent-core-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&path)?;
        Ok(Self { path })
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
}
impl Drop for TestDirectory {
    fn drop(&mut self) {
        // This immutable path was uniquely created by this fixture, never supplied by a test.
        if self.path.parent() == Some(std::env::temp_dir().as_path()) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}
