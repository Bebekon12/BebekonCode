//! User-operated project files. These operations are not agent tools or a sandbox.
use crate::{CoreError, Result};
use serde::Serialize;
use std::{
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

const MAX_TEXT: u64 = 2 * 1024 * 1024;
const MAX_ENTRIES: usize = 5000;

#[derive(Debug, Serialize)]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub directory: bool,
    pub blocked: bool,
    pub size: u64,
}

fn invalid(message: &str) -> CoreError {
    CoreError::Invalid(message.into())
}

fn linked(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0 // FILE_ATTRIBUTE_REPARSE_POINT includes junctions.
    }
    #[cfg(not(windows))]
    {
        meta.file_type().is_symlink()
    }
}

// Relative names only: forbid traversal, device names, alternate data streams and Git internals.
fn relative(path: &str) -> Result<PathBuf> {
    let relative = Path::new(path);
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return Err(invalid("Укажите путь относительно проекта"));
        };
        let name = name
            .to_str()
            .ok_or_else(|| invalid("Кодировка имени файла не поддерживается"))?;
        let stem = name
            .split('.')
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        if name.eq_ignore_ascii_case(".git")
            || name.ends_with(['.', ' '])
            || name.chars().any(|c| c < ' ' || "<>:\"|?*".contains(c))
            || ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"].contains(&stem.as_str())
            || (stem.len() == 4
                && (stem.starts_with("COM") || stem.starts_with("LPT"))
                && matches!(stem.as_bytes()[3], b'1'..=b'9'))
        {
            return Err(invalid("Зарезервированное или недопустимое имя файла"));
        }
    }
    // Check separators explicitly too: Unix builds must follow the Windows path contract.
    if path.contains('\\') || path.contains('\0') || path.split('/').any(|s| s == "." || s == "..")
    {
        return Err(invalid(
            "Укажите путь внутри проекта без переходов за его пределы",
        ));
    }
    Ok(relative.to_owned())
}

pub fn resolve(root: &Path, path: &str, allow_new: bool) -> Result<PathBuf> {
    let root = root.canonicalize()?;
    if !root.is_dir() {
        return Err(invalid("Папка проекта недоступна"));
    }
    let relative = relative(path)?;
    let mut target = root.clone();
    let count = relative.components().count();
    for (index, component) in relative.components().enumerate() {
        target.push(component);
        match fs::symlink_metadata(&target) {
            Ok(meta) if linked(&meta) => {
                return Err(invalid(
                    "Ссылки и точки соединения нельзя открывать или изменять в этом редакторе",
                ))
            }
            Ok(_) => {}
            Err(error)
                if allow_new
                    && index + 1 == count
                    && error.kind() == std::io::ErrorKind::NotFound =>
            {
                return Ok(target)
            }
            Err(error) => return Err(error.into()),
        }
    }
    let target = target.canonicalize()?;
    if !target.starts_with(&root) {
        return Err(invalid("Путь находится за пределами проекта"));
    }
    Ok(target)
}

pub fn list(root: &Path, path: &str) -> Result<Vec<Entry>> {
    let directory = resolve(root, path, false)?;
    let mut entries = Vec::new();
    for entry in fs::read_dir(directory)? {
        if entries.len() == MAX_ENTRIES {
            return Err(invalid(
                "В папке больше 5000 элементов. Откройте её в Проводнике.",
            ));
        }
        let entry = entry?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| invalid("Кодировка имени файла не поддерживается"))?;
        let meta = fs::symlink_metadata(entry.path())?;
        let item = if path.is_empty() {
            name.clone()
        } else {
            format!("{path}/{name}")
        };
        entries.push(Entry {
            name,
            path: item.clone(),
            directory: meta.is_dir(),
            blocked: linked(&meta) || relative(&item).is_err(),
            size: meta.len(),
        });
    }
    entries.sort_by(|a, b| {
        b.directory
            .cmp(&a.directory)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

pub fn read(root: &Path, path: &str) -> Result<String> {
    let path = resolve(root, path, false)?;
    let file = fs::File::open(&path)?;
    let meta = file.metadata()?;
    if !meta.is_file() || meta.len() > MAX_TEXT {
        return Err(invalid("Редактор поддерживает текстовые файлы до 2 МиБ"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_TEXT + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_TEXT || bytes.contains(&0) {
        return Err(invalid(
            "Двоичный или слишком большой файл. Откройте его в Проводнике.",
        ));
    }
    String::from_utf8(bytes).map_err(|_| {
        invalid("Редактор поддерживает UTF-8. Для других кодировок используйте внешний редактор.")
    })
}

#[cfg(windows)]
fn replace(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };
    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    // Both owned buffers are NUL-terminated and live for the entire synchronous Win32 call.
    let ok = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if ok == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}
#[cfg(not(windows))]
fn replace(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::rename(source, destination)
}

pub fn save(root: &Path, path: &str, expected: &str, content: &str) -> Result<()> {
    if content.len() as u64 > MAX_TEXT || content.contains('\0') {
        return Err(invalid("Редактор поддерживает текст до 2 МиБ"));
    }
    let target = resolve(root, path, false)?;
    let metadata = fs::metadata(&target)?;
    if metadata.permissions().readonly() {
        return Err(invalid("Файл доступен только для чтения"));
    }
    if read(root, path)? != expected {
        return Err(invalid(
            "Файл изменился на диске. Перечитайте его перед сохранением.",
        ));
    }
    let temp = target
        .parent()
        .ok_or_else(|| invalid("Нельзя редактировать корневую папку проекта"))?
        .join(format!(".bebekon-save-{}", uuid::Uuid::new_v4()));
    let result = (|| -> Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
        drop(file);
        fs::set_permissions(&temp, metadata.permissions())?;
        // Recheck after preparing the temporary file, before replacing the original.
        if read(root, path)? != expected {
            return Err(invalid(
                "Файл изменился на диске. Перечитайте его перед сохранением.",
            ));
        }
        replace(&temp, &target)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

pub fn create(root: &Path, path: &str, directory: bool) -> Result<()> {
    if path.is_empty() {
        return Err(invalid("Укажите имя файла"));
    }
    let target = resolve(root, path, true)?;
    if directory {
        fs::create_dir(target)?;
    } else {
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(target)?;
    }
    Ok(())
}

pub fn rename(root: &Path, path: &str, destination: &str) -> Result<()> {
    if path.is_empty() || destination.is_empty() {
        return Err(invalid("Нельзя переименовать корневую папку проекта"));
    }
    let source = resolve(root, path, false)?;
    let destination = resolve(root, destination, true)?;
    if fs::symlink_metadata(&destination).is_ok() {
        return Err(invalid("Файл или папка с таким именем уже существует"));
    }
    fs::rename(source, destination)?;
    Ok(())
}

/// Never recursive. UI must explicitly confirm the displayed path before invoking this command.
pub fn remove(root: &Path, path: &str) -> Result<()> {
    if path.is_empty() {
        return Err(invalid("Нельзя удалить корневую папку проекта"));
    }
    let target = resolve(root, path, false)?;
    if target.is_dir() {
        fs::remove_dir(target)?;
    } else {
        fs::remove_file(target)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestDirectory;

    #[test]
    fn real_files_round_trip_and_conflict() -> Result<()> {
        let dir = TestDirectory::new()?;
        create(dir.path(), "проект", true)?;
        create(dir.path(), "проект/code.txt", false)?;
        save(dir.path(), "проект/code.txt", "", "hello\r\nснеговик")?;
        assert_eq!(read(dir.path(), "проект/code.txt")?, "hello\r\nснеговик");
        fs::write(dir.path().join("проект/code.txt"), "external change")?;
        assert!(save(
            dir.path(),
            "проект/code.txt",
            "hello\r\nснеговик",
            "overwrite"
        )
        .is_err());
        assert_eq!(read(dir.path(), "проект/code.txt")?, "external change");
        rename(dir.path(), "проект/code.txt", "проект/new.txt")?;
        assert!(remove(dir.path(), "проект").is_err());
        remove(dir.path(), "проект/new.txt")?;
        remove(dir.path(), "проект")?;
        assert!(list(dir.path(), "")?.is_empty());
        Ok(())
    }

    #[test]
    fn blocks_traversal_devices_and_binary() -> Result<()> {
        let dir = TestDirectory::new()?;
        for path in [
            "../outside",
            "/outside",
            "C:/outside",
            "file:stream",
            "a/../b",
            "NUL",
            "CON.txt",
            ".git/config",
            "bad.",
            "COM1",
            "a\\b",
        ] {
            assert!(create(dir.path(), path, false).is_err(), "{path}");
        }
        fs::write(dir.path().join("binary"), [0, 1, 2])?;
        assert!(read(dir.path(), "binary").is_err());
        create(dir.path(), "text", false)?;
        assert!(create(dir.path(), "text", false).is_err());
        fs::write(dir.path().join("other"), "keep")?;
        assert!(rename(dir.path(), "text", "other").is_err());
        assert!(remove(dir.path(), "").is_err());
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn junction_cannot_escape_project() -> Result<()> {
        let dir = TestDirectory::new()?;
        let outside = TestDirectory::new()?;
        fs::write(outside.path().join("secret"), "outside")?;
        let junction = dir.path().join("link");
        // Fixture paths are fixed, generated UUID names, never user input.
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(outside.path())
            .output()?;
        assert!(status.status.success());
        assert!(read(dir.path(), "link/secret").is_err());
        assert!(create(dir.path(), "link/new", false).is_err());
        assert!(rename(dir.path(), "link", "renamed").is_err());
        assert!(remove(dir.path(), "link").is_err());
        assert!(list(dir.path(), "")?[0].blocked);
        fs::remove_dir(junction)?; // Remove the link itself before fixture cleanup.
        assert_eq!(
            fs::read_to_string(outside.path().join("secret"))?,
            "outside"
        );
        Ok(())
    }
}
