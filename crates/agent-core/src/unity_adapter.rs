//! Explicit installation into a user-selected Unity project; never edits scenes or settings.
use crate::{files, Core, CoreError, Result};
use std::{fs, path::Path};
const EDITOR: &str = include_str!("../../../adapters/unity/Editor/DesktopAdapter.cs");
const RUNTIME: &str = include_str!("../../../adapters/unity/Runtime/EndFrameCapture.cs");
fn install(root: &Path, executable: &Path) -> Result<()> {
    files::resolve(root, "ProjectSettings/ProjectVersion.txt", false).map_err(|_| {
        CoreError::Invalid("Выберите папку Unity-проекта с Assets и ProjectSettings".into())
    })?;
    files::resolve(root, "Assets", false)?;
    let config = serde_json::to_string_pretty(&serde_json::json!({"executable":executable}))?;
    let sources = [
        (
            "Assets/BebekonSmartComputer/Editor/DesktopAdapter.cs",
            EDITOR,
        ),
        (
            "Assets/BebekonSmartComputer/Runtime/EndFrameCapture.cs",
            RUNTIME,
        ),
        (
            "Assets/BebekonSmartComputer/Editor/host.json",
            config.as_str(),
        ),
    ];
    // Existing user edits are never overwritten. Check every existing parent for junctions.
    for (path, content) in sources {
        let mut relative = String::new();
        for component in path.split('/') {
            if !relative.is_empty() {
                relative.push('/');
            }
            relative.push_str(component);
            let candidate = root.join(&relative);
            if candidate.exists() {
                files::resolve(root, &relative, false)?;
            }
        }
        if root.join(path).exists()
            && fs::read_to_string(files::resolve(root, path, false)?)? != content
        {
            return Err(CoreError::Invalid("Unity Adapter уже существует и отличается. Установка остановлена; ваши файлы сохранены.".into()));
        }
    }
    for directory in [
        "Assets/BebekonSmartComputer",
        "Assets/BebekonSmartComputer/Editor",
        "Assets/BebekonSmartComputer/Runtime",
    ] {
        let resolved = files::resolve(root, directory, true)?;
        if !resolved.exists() {
            fs::create_dir(resolved)?;
        }
    }
    for (path, content) in sources {
        let path = files::resolve(root, path, true)?;
        if !path.exists() {
            use std::io::Write;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)?;
            file.write_all(content.as_bytes())?;
        }
    }
    Ok(())
}
impl Core {
    pub async fn install_unity_adapter(&self, root: String) -> Result<()> {
        let executable = std::env::current_exe()?;
        tokio::task::spawn_blocking(move || install(Path::new(&root), &executable))
            .await
            .map_err(|_| CoreError::Busy)?
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn installation_is_explicit_idempotent_and_preserves_edits() {
        let dir = crate::test_support::TestDirectory::new().unwrap();
        let root = dir.path();
        assert!(install(root, Path::new("host.exe")).is_err());
        fs::create_dir(root.join("Assets")).unwrap();
        fs::create_dir(root.join("ProjectSettings")).unwrap();
        fs::write(
            root.join("ProjectSettings/ProjectVersion.txt"),
            "m_EditorVersion: 6000.0.1f1",
        )
        .unwrap();
        install(root, Path::new("host.exe")).unwrap();
        install(root, Path::new("host.exe")).unwrap();
        let path = root.join("Assets/BebekonSmartComputer/Editor/DesktopAdapter.cs");
        fs::write(&path, "user modification").unwrap();
        assert!(install(root, Path::new("host.exe")).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "user modification");
    }
}
