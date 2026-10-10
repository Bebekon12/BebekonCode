//! Bounded, read-only snapshots for one user turn. Contents stay in memory only.
use crate::{
    model::{ChangedFile, EventPayload},
    Core, Result,
};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    time::UNIX_EPOCH,
};

const MAX_FILES: usize = 10_000;
const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 64 * 1024 * 1024;
const MAX_TEXT_BYTES: usize = 256 * 1024;

pub(crate) struct Snapshot {
    files: BTreeMap<String, File>,
    limited: bool,
    skipped: BTreeSet<String>,
    truncated: bool,
}
struct File {
    hash: Vec<u8>,
    text: Option<String>,
}

fn excluded(path: &str) -> bool {
    path.split('/').any(|part| {
        let name = part.to_ascii_lowercase();
        name.starts_with(".env")
            || matches!(
                name.as_str(),
                ".git"
                    | ".ssh"
                    | ".aws"
                    | ".azure"
                    | ".codex"
                    | ".claude"
                    | "auth.json"
                    | ".credentials.json"
                    | "credentials"
                    | "credentials.json"
                    | "id_rsa"
                    | "id_ed25519"
            )
            || name.ends_with(".pem")
            || name.ends_with(".pfx")
            || name.ends_with(".p12")
            || name.ends_with(".key")
    })
}

pub(crate) async fn snapshot(root: &Path) -> Option<Snapshot> {
    let paths = crate::git::read_git(
        root,
        &[
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ],
    )
    .await
    .ok()?;
    let root = root.to_owned();
    tokio::task::spawn_blocking(move || capture(&root, &paths))
        .await
        .ok()
}

fn capture(root: &Path, paths: &[u8]) -> Snapshot {
    let mut snapshot = Snapshot {
        files: BTreeMap::new(),
        limited: false,
        skipped: BTreeSet::new(),
        truncated: false,
    };
    let mut total = 0;
    for raw in paths
        .split(|byte| *byte == 0)
        .filter(|name| !name.is_empty())
    {
        let Ok(path) = std::str::from_utf8(raw) else {
            snapshot.limited = true;
            continue;
        };
        if excluded(path) {
            continue;
        }
        if snapshot.files.len() == MAX_FILES {
            snapshot.limited = true;
            snapshot.truncated = true;
            break;
        }
        let resolved = match crate::files::resolve(root, path, false) {
            Ok(path) => path,
            Err(crate::CoreError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                continue
            }
            Err(_) => {
                snapshot.limited = true;
                snapshot.skipped.insert(path.into());
                continue;
            }
        };
        let Ok(meta) = std::fs::metadata(&resolved) else {
            snapshot.limited = true;
            snapshot.skipped.insert(path.into());
            continue;
        };
        if !meta.is_file() {
            snapshot.limited = true;
            snapshot.skipped.insert(path.into());
            continue;
        }
        let file = if meta.len() <= MAX_FILE_BYTES && total + meta.len() <= MAX_TOTAL_BYTES {
            use std::io::Read;
            let bytes = std::fs::File::open(&resolved).and_then(|file| {
                let mut bytes = Vec::new();
                file.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes)?;
                Ok(bytes)
            });
            let Ok(bytes) = bytes else {
                snapshot.limited = true;
                snapshot.skipped.insert(path.into());
                continue;
            };
            if bytes.len() as u64 > MAX_FILE_BYTES {
                snapshot.limited = true;
                snapshot.skipped.insert(path.into());
                continue;
            }
            total += bytes.len() as u64;
            let text = if bytes.len() <= MAX_TEXT_BYTES && !bytes.contains(&0) {
                std::str::from_utf8(&bytes).ok().map(str::to_string)
            } else {
                None
            };
            File {
                hash: Sha256::digest(&bytes).to_vec(),
                text,
            }
        } else {
            // Large/budget-exceeding assets are compared by metadata, explicitly marked limited.
            snapshot.limited = true;
            let modified = meta
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok());
            File {
                hash: format!("metadata:{}:{modified:?}", meta.len()).into_bytes(),
                text: None,
            }
        };
        snapshot.files.insert(path.into(), file);
    }
    snapshot
}

fn counts(before: &str, after: &str) -> Option<(u32, u32)> {
    let mut left: Vec<_> = before.split_inclusive('\n').collect();
    let mut right: Vec<_> = after.split_inclusive('\n').collect();
    let prefix = left.iter().zip(&right).take_while(|(a, b)| a == b).count();
    left.drain(..prefix);
    right.drain(..prefix);
    while left.last().is_some() && left.last() == right.last() {
        left.pop();
        right.pop();
    }
    if left.len().saturating_mul(right.len()) > 2_000_000 {
        return None;
    }
    let mut row = vec![0u32; right.len() + 1];
    for line in &left {
        let mut diagonal = 0;
        for (index, other) in right.iter().enumerate() {
            let above = row[index + 1];
            row[index + 1] = if line == other {
                diagonal + 1
            } else {
                above.max(row[index])
            };
            diagonal = above;
        }
    }
    let common = row[right.len()];
    Some((right.len() as u32 - common, left.len() as u32 - common))
}

fn compare(before: Snapshot, after: Snapshot) -> (Vec<ChangedFile>, bool) {
    let mut changes = Vec::new();
    let mut limited = before.limited || after.limited;
    let paths: BTreeSet<_> = before.files.keys().chain(after.files.keys()).collect();
    for path in paths {
        let left = before.files.get(path);
        let right = after.files.get(path);
        if left.zip(right).is_some_and(|(a, b)| a.hash == b.hash) {
            continue;
        }
        // If either snapshot skipped files, absence cannot establish creation/deletion.
        if before.skipped.contains(path)
            || after.skipped.contains(path)
            || ((left.is_none() || right.is_none()) && (before.truncated || after.truncated))
        {
            continue;
        }
        let status = if left.is_none() {
            "added"
        } else if right.is_none() {
            "deleted"
        } else {
            "modified"
        };
        let text_before = left.map(|file| file.text.as_deref()).unwrap_or(Some(""));
        let text_after = right.map(|file| file.text.as_deref()).unwrap_or(Some(""));
        let delta = text_before.zip(text_after).and_then(|(a, b)| counts(a, b));
        if text_before.is_some() && text_after.is_some() && delta.is_none() {
            limited = true;
        }
        changes.push(ChangedFile {
            path: path.clone(),
            status: status.into(),
            added: delta.map(|(added, _)| added),
            removed: delta.map(|(_, removed)| removed),
        });
    }
    (changes, limited)
}

impl Core {
    pub(crate) async fn record_changes(
        &self,
        id: &str,
        run: &str,
        root: &str,
        before: Option<Snapshot>,
    ) -> Result<()> {
        let Some(before) = before else { return Ok(()) };
        let Some(after) = snapshot(Path::new(root)).await else {
            return self
                .emit(
                    id,
                    run,
                    EventPayload::RunChanges {
                        files: vec![],
                        limited: true,
                    },
                    None,
                )
                .await;
        };
        let (files, limited) = tokio::task::spawn_blocking(move || compare(before, after))
            .await
            .unwrap_or_else(|_| (vec![], true));
        self.emit(id, run, EventPayload::RunChanges { files, limited }, None)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compares_existing_dirty_files_new_deleted_and_binary_files_without_reading_secrets() {
        let dir = crate::test_support::TestDirectory::new().unwrap();
        for (path, content) in [
            ("old.rs", b"already dirty\n".as_slice()),
            ("delete.txt", b"delete\n"),
            ("asset.bin", b"\0one"),
            (".env", b"secret"),
        ] {
            std::fs::write(dir.path().join(path), content).unwrap();
        }
        let before = capture(dir.path(), b"old.rs\0delete.txt\0asset.bin\0.env\0");
        assert!(!before.files.contains_key(".env"));
        std::fs::write(dir.path().join("old.rs"), "already dirty\nnew\n").unwrap();
        std::fs::write(dir.path().join("new.txt"), "one\ntwo\n").unwrap();
        std::fs::write(dir.path().join("asset.bin"), b"\0two").unwrap();
        std::fs::remove_file(dir.path().join("delete.txt")).unwrap();
        let after = capture(
            dir.path(),
            b"old.rs\0delete.txt\0asset.bin\0new.txt\0.env\0",
        );
        let (changes, limited) = compare(before, after);
        assert!(!limited);
        assert_eq!(changes.len(), 4);
        let existing = changes.iter().find(|file| file.path == "old.rs").unwrap();
        assert_eq!((existing.added, existing.removed), (Some(1), Some(0)));
        assert!(changes
            .iter()
            .any(|file| file.path == "delete.txt" && file.removed == Some(1)));
        assert!(changes
            .iter()
            .any(|file| file.path == "asset.bin" && file.added.is_none()));
        let unchanged = capture(dir.path(), b"old.rs\0asset.bin\0new.txt\0");
        assert!(compare(
            capture(dir.path(), b"old.rs\0asset.bin\0new.txt\0"),
            unchanged
        )
        .0
        .is_empty());
    }
    #[test]
    fn line_counts_include_replacements_and_newline_changes_and_bound_cost() {
        assert_eq!(counts("a\nb\nc\n", "a\nx\nc\n"), Some((1, 1)));
        assert_eq!(counts("a", "a\n"), Some((1, 1)));
        assert_eq!(counts("", ""), Some((0, 0)));
        assert!(counts(&"a\n".repeat(2000), &"b\n".repeat(2000)).is_none());
    }
}
