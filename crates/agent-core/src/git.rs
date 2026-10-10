use crate::{CoreError, Result};
use serde::{Deserialize, Serialize};
use std::{path::Path, process::Stdio, time::Duration};
use tokio::{io::AsyncReadExt, process::Command};

const OUTPUT_LIMIT: u64 = 2 * 1024 * 1024;

#[derive(Debug, Serialize, Deserialize)]
pub struct GitFile {
    pub path: String,
    pub status: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct GitStatus {
    pub branch: String,
    pub files: Vec<GitFile>,
}

/// Read-only Git runner; no shell, no external diff drivers, bounded output and lifetime.
pub(crate) async fn read_git(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let binary = which::which("git").map_err(|_| CoreError::Git)?;
    let mut command = Command::new(binary);
    command
        .args(["-c", "core.fsmonitor=false"])
        .current_dir(root)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .env_clear();
    for key in ["SystemRoot", "WINDIR", "PATH", "TEMP", "TMP"] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    command
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        // Git for Windows recognizes /dev/null; a literal NUL path fails config probing.
        .env("GIT_CONFIG_GLOBAL", "/dev/null");
    #[cfg(windows)]
    command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    let mut child = command.spawn().map_err(|_| CoreError::Git)?;
    let stdout = child.stdout.take().ok_or(CoreError::Git)?;
    let operation = async {
        let mut bytes = Vec::new();
        stdout
            .take(OUTPUT_LIMIT + 1)
            .read_to_end(&mut bytes)
            .await
            .map_err(|_| CoreError::Git)?;
        if bytes.len() as u64 > OUTPUT_LIMIT {
            return Err(CoreError::Invalid(
                "Вывод Git превышает 2 МиБ. Используйте внешний клиент Git".into(),
            ));
        }
        if !child.wait().await.map_err(|_| CoreError::Git)?.success() {
            return Err(CoreError::Git);
        }
        Ok(bytes)
    };
    tokio::time::timeout(Duration::from_secs(10), operation)
        .await
        .map_err(|_| CoreError::Git)?
}

pub async fn status(root: &Path) -> Result<GitStatus> {
    let output = read_git(
        root,
        &["status", "--porcelain=v1", "-z", "--untracked-files=normal"],
    )
    .await?;
    let branch = read_git(root, &["branch", "--show-current"]).await?;
    let branch = String::from_utf8_lossy(&branch).trim().to_string();
    Ok(GitStatus {
        branch: if branch.is_empty() {
            "Отсоединённый HEAD".into()
        } else {
            branch
        },
        files: parse_status(&output),
    })
}

fn parse_status(output: &[u8]) -> Vec<GitFile> {
    let mut parts = output.split(|byte| *byte == 0);
    let mut files = Vec::new();
    while let Some(entry) = parts.next() {
        if entry.len() < 4 {
            continue;
        }
        let status = String::from_utf8_lossy(&entry[..2]).into_owned();
        files.push(GitFile {
            path: String::from_utf8_lossy(&entry[3..]).into_owned(),
            status,
        });
        if entry[..2].contains(&b'R') || entry[..2].contains(&b'C') {
            parts.next();
        }
    }
    files
}

pub async fn diff(root: &Path) -> Result<String> {
    let unstaged = read_git(root, &["diff", "--no-ext-diff", "--no-textconv", "--", "."]).await?;
    let staged = read_git(
        root,
        &[
            "diff",
            "--cached",
            "--no-ext-diff",
            "--no-textconv",
            "--",
            ".",
        ],
    )
    .await?;
    Ok(format!(
        "Изменения вне индекса\n{}\nИзменения в индексе\n{}",
        String::from_utf8_lossy(&unstaged),
        String::from_utf8_lossy(&staged)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn status_handles_spaces_and_rename_source() {
        let files = parse_status(b" M src/a b.rs\0R  new.rs\0old.rs\0?? new.txt\0");
        assert_eq!(files.len(), 3);
        assert_eq!(files[0].path, "src/a b.rs");
        assert_eq!(files[1].path, "new.rs");
    }
}
