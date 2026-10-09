//! User-managed local stdio definitions. Saving metadata never starts an executable.
use crate::{files, model::AccountProfile, redaction, CoreError, Result};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalMcpServer {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
}

fn invalid() -> CoreError {
    CoreError::Invalid("MCP: нужны уникальное имя, абсолютный путь к локальному exe и аргументы без секретов. Переменные окружения и удалённые серверы недоступны.".into())
}

pub fn validate(servers: &[LocalMcpServer]) -> Result<()> {
    if servers.len() > 20 {
        return Err(invalid());
    }
    let mut names = HashSet::new();
    for s in servers {
        if s.name.is_empty()
            || s.name.len() > 64
            || !s
                .name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-')
            || !names.insert(&s.name)
            || !Path::new(&s.command).is_absolute()
            || !s.command.to_ascii_lowercase().ends_with(".exe")
            || s.command.len() > 1024
            || s.args.len() > 64
        {
            return Err(invalid());
        }
        for value in std::iter::once(&s.command).chain(&s.args) {
            let lower = value.to_ascii_lowercase();
            if value.len() > 4096
                || value.chars().any(char::is_control)
                || redaction::redact(value) != *value
                || [
                    "token",
                    "password",
                    "secret",
                    "api_key",
                    "api-key",
                    "authorization",
                    "bearer",
                    "://",
                ]
                .iter()
                .any(|term| lower.contains(term))
            {
                return Err(invalid());
            }
        }
    }
    Ok(())
}

fn path(account: &AccountProfile) -> Result<std::path::PathBuf> {
    if account.provider != "anthropic" {
        return Err(CoreError::Invalid("Добавление MCP для Codex пока недоступно: требуется отдельный мост подтверждений каждого действия.".into()));
    }
    let root = account
        .config_dir
        .as_deref()
        .ok_or(CoreError::ProviderUnavailable)?;
    files::resolve(Path::new(root), "bebekon-mcp.json", true)
}

pub fn read(account: &AccountProfile) -> Result<Vec<LocalMcpServer>> {
    let path = path(account)?;
    if !path.exists() {
        return Ok(vec![]);
    }
    if std::fs::metadata(&path)?.len() > 128 * 1024 {
        return Err(invalid());
    }
    let servers = serde_json::from_slice::<Vec<LocalMcpServer>>(&std::fs::read(path)?)?;
    validate(&servers)?;
    Ok(servers)
}

pub fn save(account: &AccountProfile, servers: &[LocalMcpServer]) -> Result<()> {
    validate(servers)?;
    for server in servers {
        if !Path::new(&server.command).is_file() {
            return Err(invalid());
        }
    }
    let path = path(account)?;
    std::fs::write(path, serde_json::to_vec_pretty(servers)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn definitions_cannot_store_credentials_or_remote_endpoints() {
        let server = LocalMcpServer {
            name: "unity".into(),
            command: "C:/tools/unity.exe".into(),
            args: vec!["mcp".into()],
        };
        assert!(validate(std::slice::from_ref(&server)).is_ok());
        assert!(validate(&[server.clone(), server.clone()]).is_err());
        for arg in [
            "--token=private",
            "https://host.invalid",
            "--password",
            "a\nb",
        ] {
            let mut unsafe_server = server.clone();
            unsafe_server.args.push(arg.into());
            assert!(validate(&[unsafe_server]).is_err());
        }
        assert!(serde_json::from_str::<LocalMcpServer>(
            r#"{"name":"a","command":"C:/a.exe","args":[],"env":{"TOKEN":"private"}}"#
        )
        .is_err());
    }
}
