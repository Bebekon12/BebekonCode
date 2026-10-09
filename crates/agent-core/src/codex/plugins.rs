//! Official Codex CLI plugin management, scoped to the app-owned account home.
//! App-server plugin methods remain forbidden for production clients.
use super::{CodexProvider, Detection};
use crate::{model::AccountProfile, process, redaction::redact, CoreError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    ffi::OsString,
    io::Read,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const MAX_OUTPUT: u64 = 32 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginEntry {
    pub id: String,
    pub name: String,
    pub marketplace: String,
    pub version: Option<String>,
    pub description: String,
    pub installed: bool,
    pub enabled: bool,
    pub can_remove: bool,
    pub unavailable_reason: Option<String>,
    pub auth_policy: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PluginInventory {
    pub entries: Vec<PluginEntry>,
}

fn identifier(value: &str) -> bool {
    value
        .bytes()
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric())
        && value.len() <= 160
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
}

pub(super) fn desktop_only(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    [
        "computer-use",
        "computer_use",
        "desktop",
        "browser-use",
        "chrome",
    ]
    .iter()
    .any(|part| name.contains(part))
}

fn parse(bytes: &[u8]) -> Result<PluginInventory> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| CoreError::Invalid("Codex вернул неизвестный формат плагинов".into()))?;
    let mut entries: Vec<PluginEntry> = vec![];
    let mut seen = std::collections::HashSet::new();
    for (key, installed) in [("installed", true), ("available", false)] {
        let rows = value.get(key).and_then(Value::as_array).ok_or_else(|| {
            CoreError::Invalid("Обновите Codex CLI: список плагинов недоступен".into())
        })?;
        if rows.len() > 20000 {
            return Err(CoreError::Invalid("Слишком большой список плагинов".into()));
        }
        for row in rows {
            let (Some(name), Some(marketplace)) = (
                row.get("name").and_then(Value::as_str),
                row.get("marketplaceName").and_then(Value::as_str),
            ) else {
                continue;
            };
            if !identifier(name) || !identifier(marketplace) {
                continue;
            }
            let id = format!("{name}@{marketplace}");
            if row
                .get("pluginId")
                .and_then(Value::as_str)
                .is_some_and(|reported| reported != id)
            {
                continue;
            }
            if !seen.insert(id.clone()) {
                continue;
            }
            let unavailable_reason = if desktop_only(name) {
                Some(
                    "Нужна интеграция с официальным desktop-приложением. В BebekonCode недоступно."
                        .into(),
                )
            } else if row
                .get("installPolicy")
                .and_then(Value::as_str)
                .is_some_and(|p| {
                    ![
                        "AVAILABLE",
                        "NOT_INSTALLED",
                        "INSTALLED",
                        "available",
                        "allowed",
                    ]
                    .contains(&p)
                })
                && !installed
            {
                Some("Установка ограничена политикой провайдера".into())
            } else {
                None
            };
            entries.push(PluginEntry {
                id,
                name: name.into(),
                marketplace: marketplace.into(),
                version: row
                    .get("version")
                    .and_then(Value::as_str)
                    .map(|s| s.chars().take(80).collect()),
                description: row
                    .get("description")
                    .and_then(Value::as_str)
                    .map(|s| redact(s).chars().take(600).collect())
                    .unwrap_or_default(),
                installed,
                can_remove: installed
                    && row
                        .get("installPolicy")
                        .and_then(Value::as_str)
                        .is_none_or(|p| p == "AVAILABLE"),
                enabled: row
                    .get("enabled")
                    .and_then(Value::as_bool)
                    .unwrap_or(installed)
                    && unavailable_reason.is_none(),
                unavailable_reason,
                auth_policy: row
                    .get("authPolicy")
                    .and_then(Value::as_str)
                    .map(|s| s.chars().take(80).collect()),
            });
        }
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(PluginInventory { entries })
}

struct OwnedCommand {
    child: std::process::Child,
    _group: Option<process::ProcessGroup>,
}
impl Drop for OwnedCommand {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

async fn execute(
    detection: Detection,
    account: &AccountProfile,
    args: Vec<String>,
) -> Result<Vec<u8>> {
    let home = CodexProvider::profile(account)?;
    tokio::task::spawn_blocking(move || {
        std::fs::create_dir_all(&home)?;
        let binary = detection.binary.ok_or_else(|| CoreError::Invalid("Установите официальный Codex CLI".into()))?;
        let mut env = vec![("CODEX_HOME", OsString::from(&home))];
        // Profile paths on Windows exceed MAX_PATH after marketplace/cache nesting.
        // This affects only Git children of this CLI, never the user's global Git config.
        #[cfg(windows)]
        env.extend([
            ("GIT_CONFIG_COUNT", "1".into()),
            ("GIT_CONFIG_KEY_0", "core.longpaths".into()),
            ("GIT_CONFIG_VALUE_0", "true".into()),
        ]);
        if let Some(root) = detection.managed_root {
            env.push(("CODEX_MANAGED_BY_NPM", "1".into()));
            env.push(("CODEX_MANAGED_PACKAGE_ROOT", root.into_os_string()));
        }
        let mut command = Command::new(binary);
        command.args(["-c", "analytics.enabled=false", "-c", "feedback.enabled=false", "-c", "features.hooks=false", "-c", "cli_auth_credentials_store=\"keyring\""])
            .args(args).current_dir(&home).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
        process::harden(&mut command, process::child_env(&env));
        let child = command.spawn().map_err(|_| CoreError::Invalid("Не удалось запустить менеджер плагинов Codex".into()))?;
        let group = process::ProcessGroup::adopt(&child);
        let mut owned = OwnedCommand { child, _group: group };
        let stdout = owned.child.stdout.take().ok_or(CoreError::Busy)?;
        let reader = std::thread::spawn(move || { let mut bytes = vec![]; stdout.take(MAX_OUTPUT + 1).read_to_end(&mut bytes).map(|_| bytes) });
        let end = Instant::now() + Duration::from_secs(90);
        loop {
            if let Some(status) = owned.child.try_wait()? {
                if !status.success() { return Err(CoreError::Invalid("Codex не завершил операцию. Проверьте вход, Git и доступность каталога; обновите CLI при необходимости.".into())); }
                break;
            }
            if Instant::now() >= end { return Err(CoreError::Invalid("Менеджер плагинов Codex не ответил вовремя".into())); }
            std::thread::sleep(Duration::from_millis(25));
        }
        // End helpers before joining the bounded stdout reader.
        if let Some(group) = &owned._group { group.terminate(); }
        let bytes = reader.join().map_err(|_| CoreError::Busy)??;
        if bytes.len() as u64 > MAX_OUTPUT { return Err(CoreError::Invalid("Слишком большой ответ Codex".into())); }
        Ok(bytes)
    }).await.map_err(|_| CoreError::Busy)?
}

pub(super) async fn list(
    provider: &CodexProvider,
    account: &AccountProfile,
) -> Result<PluginInventory> {
    parse(
        &execute(
            provider.detection(),
            account,
            ["plugin", "list", "--available", "--json"]
                .map(str::to_string)
                .to_vec(),
        )
        .await?,
    )
}

pub(super) async fn installed(
    provider: &CodexProvider,
    account: &AccountProfile,
) -> Result<PluginInventory> {
    parse(
        &execute(
            provider.detection(),
            account,
            ["plugin", "list", "--json"].map(str::to_string).to_vec(),
        )
        .await?,
    )
}

pub(super) async fn change(
    provider: &CodexProvider,
    account: &AccountProfile,
    id: &str,
    install: bool,
) -> Result<()> {
    let entry = list(provider, account)
        .await?
        .entries
        .into_iter()
        .find(|entry| entry.id == id)
        .ok_or_else(|| {
            CoreError::Invalid("Плагина нет в каталоге этого аккаунта. Обновите список.".into())
        })?;
    if install {
        if let Some(reason) = entry.unavailable_reason {
            return Err(CoreError::Invalid(reason));
        }
        if entry.installed {
            return Ok(());
        }
    } else if !entry.installed {
        return Err(CoreError::Invalid(
            "Плагин не установлен в этом аккаунте".into(),
        ));
    } else if !entry.can_remove {
        return Err(CoreError::Invalid(
            "Этот плагин управляется провайдером и не может быть удалён здесь".into(),
        ));
    }
    execute(
        provider.detection(),
        account,
        vec![
            "plugin".into(),
            if install { "add" } else { "remove" }.into(),
            entry.id.clone(),
            "--json".into(),
        ],
    )
    .await?;
    let present = installed(provider, account)
        .await?
        .entries
        .iter()
        .any(|item| item.id == entry.id && item.installed);
    if present != install {
        return Err(CoreError::Invalid(
            "Codex не подтвердил изменение установки. Обновите список плагинов.".into(),
        ));
    }
    Ok(())
}

pub(super) async fn add_source(
    provider: &CodexProvider,
    account: &AccountProfile,
    source: &str,
) -> Result<()> {
    let repo = match source {
        "openai" => return Err(CoreError::Invalid("Каталог OpenAI предоставляется автоматически после входа через Codex. Обновите список плагинов.".into())),
        "unity" => "Unity-Technologies/unity-agent-plugin",
        _ => return Err(CoreError::Invalid("Неизвестный официальный каталог".into())),
    };
    execute(
        provider.detection(),
        account,
        ["plugin", "marketplace", "add", repo, "--json"]
            .map(str::to_string)
            .to_vec(),
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_is_scoped_deduplicated_and_rejects_command_arguments() {
        let list = parse(br#"{"installed":[{"pluginId":"unity@unity-agent-plugin","name":"unity","marketplaceName":"unity-agent-plugin","enabled":true}],"available":[{"name":"unity","marketplaceName":"unity-agent-plugin"},{"name":"--bad","marketplaceName":"x","pluginId":"other"},{"name":"../escape","marketplaceName":"x"},{"name":"computer-use","marketplaceName":"openai-bundled"}]}"#).unwrap();
        assert_eq!(list.entries.len(), 2);
        assert!(
            list.entries
                .iter()
                .find(|e| e.name == "unity")
                .unwrap()
                .installed
        );
        assert!(list
            .entries
            .iter()
            .find(|e| e.name == "computer-use")
            .unwrap()
            .unavailable_reason
            .is_some());
        assert!(parse(br#"{"data":[]}"#).is_err());
    }

    #[test]
    fn large_remote_catalog_is_bounded_and_provider_defaults_are_not_removable() {
        let mut available = vec![];
        for index in 0..5700 {
            available.push(serde_json::json!({"name":format!("plugin-{index}"),"marketplaceName":"openai-curated-remote","installPolicy":"AVAILABLE"}));
        }
        let bytes = serde_json::to_vec(&serde_json::json!({"installed":[{"name":"pages","marketplaceName":"openai-curated-remote","enabled":true,"installPolicy":"INSTALLED_BY_DEFAULT"}],"available":available})).unwrap();
        let parsed = parse(&bytes).unwrap();
        assert_eq!(parsed.entries.len(), 5701);
        let default = parsed
            .entries
            .iter()
            .find(|entry| entry.name == "pages")
            .unwrap();
        assert!(default.enabled);
        assert!(!default.can_remove);
        assert!(!identifier("--option"));
        assert!(!identifier("x;whoami"));
    }
}
