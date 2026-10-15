//! Persistent, exact project-file rules compiled into documented Claude permissions.allow.
use super::*;
use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileRule {
    pub root: String,
    pub path: String,
    pub tool: String,
}
fn location(account: &AccountProfile) -> Result<PathBuf> {
    let root = Path::new(
        account
            .config_dir
            .as_deref()
            .ok_or(CoreError::ProviderUnavailable)?,
    );
    crate::files::resolve(root, "bebekon-file-rules.json", true)
}
fn all(account: &AccountProfile) -> Result<Vec<FileRule>> {
    let path = location(account)?;
    if !path.exists() {
        return Ok(vec![]);
    }
    if std::fs::metadata(&path)?.len() > 128 * 1024 {
        return Err(failure("Слишком большой файл правил", None));
    }
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}
pub fn read(account: &AccountProfile, session: &Session) -> Result<Vec<FileRule>> {
    let root = Path::new(&session.working_directory)
        .canonicalize()?
        .to_string_lossy()
        .into_owned();
    let rules: Vec<_> = all(account)?
        .into_iter()
        .filter(|r| r.root == root)
        .collect();
    for rule in &rules {
        if !["Edit", "Write"].contains(&rule.tool.as_str())
            || rule.path.is_empty()
            || rule.path.len() > 1000
            || rule.path.contains(['*', '?', '[', ']', '(', ')', '\\'])
            || Path::new(&rule.path).is_absolute()
            || rule.path.contains(':')
            || rule
                .path
                .split('/')
                .any(|part| matches!(part, "" | "." | ".."))
        {
            return Err(failure(
                "Сохранённое правило имеет недопустимую область",
                None,
            ));
        }
        // A file or its parent may have been renamed since the rule was saved. Keep
        // dormant rules visible/revocable. Every actual call still passes the path guard.
    }
    Ok(rules)
}
pub fn save(account: &AccountProfile, session: &Session, rules: Vec<FileRule>) -> Result<()> {
    if account.provider != "anthropic" || session.account_profile_id != account.id {
        return Err(CoreError::ProviderUnavailable);
    }
    if rules.len() > 100 {
        return Err(failure("Не более 100 правил проекта", None));
    }
    let root = Path::new(&session.working_directory).canonicalize()?;
    let root_name = root.to_string_lossy().into_owned();
    let profile = Path::new(
        account
            .config_dir
            .as_deref()
            .ok_or(CoreError::ProviderUnavailable)?,
    )
    .canonicalize()?;
    let protected = profile
        .ancestors()
        .find(|p| p.file_name().is_some_and(|n| n == "providers"))
        .unwrap_or(&profile);
    for rule in &rules {
        if rule.root != root_name
            || rule
                .path
                .split('/')
                .any(|part| matches!(part, "" | "." | ".."))
            || !["Edit", "Write"].contains(&rule.tool.as_str())
            || rule.path.contains(['*', '?', '[', ']', '(', ')'])
            || rule.path.len() > 1000
            || Path::new(&rule.path).is_absolute()
            || rule.path.contains('\\')
        {
            return Err(failure(
                "Разрешены только точные пути Edit/Write внутри проекта",
                None,
            ));
        }
        let path = super::guard::checked_path(&root, &json!({"file_path":rule.path}), &rule.tool)?;
        if path.starts_with(protected) {
            return Err(failure("Профиль провайдера защищён", None));
        }
    }
    let mut stored = all(account)?;
    stored.retain(|r| r.root != root_name);
    stored.extend(rules);
    if stored.len() > 500 {
        return Err(failure("Не более 500 правил профиля", None));
    }
    let path = location(account)?;
    let before = if path.exists() {
        std::fs::read_to_string(&path)?
    } else {
        String::new()
    };
    let next = serde_json::to_string_pretty(&stored)?;
    let directory = path.parent().ok_or(CoreError::NotFound)?;
    if path.exists() {
        crate::files::save(directory, "bebekon-file-rules.json", &before, &next)?;
    } else {
        use std::io::Write;
        std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path)?
            .write_all(next.as_bytes())?;
    }
    Ok(())
}
