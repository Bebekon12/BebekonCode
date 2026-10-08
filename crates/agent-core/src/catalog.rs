//! Public marketplace metadata only. No authentication, arbitrary URLs, installs or execution.
use crate::{model::now, CoreError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

const MAX_BYTES: usize = 2 * 1024 * 1024;
const SOURCES: [(&str, &str); 2] = [
    ("openai", "https://raw.githubusercontent.com/openai/plugins/main/.agents/plugins/marketplace.json"),
    ("anthropic", "https://raw.githubusercontent.com/anthropics/claude-plugins-official/main/.claude-plugin/marketplace.json"),
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub name: String,
    pub provider: String,
    pub description: String,
    pub category: String,
    pub source_url: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Catalog {
    pub entries: Vec<CatalogEntry>,
    pub errors: Vec<String>,
    pub checked_at: i64,
}

pub fn source_url(provider: &str) -> Result<&'static str> {
    match provider {
        "openai" => Ok("https://github.com/openai/plugins"),
        "anthropic" => Ok("https://github.com/anthropics/claude-plugins-official"),
        _ => Err(CoreError::Invalid("Неизвестный каталог".into())),
    }
}
fn parse(provider: &str, bytes: &[u8]) -> Result<Vec<CatalogEntry>> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| CoreError::Invalid("Каталог вернул некорректные данные".into()))?;
    let plugins = value
        .get("plugins")
        .and_then(Value::as_array)
        .ok_or_else(|| CoreError::Invalid("Неизвестный формат каталога".into()))?;
    if plugins.len() > 2000 {
        return Err(CoreError::Invalid("Слишком большой каталог".into()));
    }
    let mut entries = Vec::new();
    for item in plugins {
        let Some(name) = item.get("name").and_then(Value::as_str) else {
            continue;
        };
        if name.is_empty()
            || name.len() > 128
            || !name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
        {
            continue;
        }
        if entries
            .iter()
            .any(|entry: &CatalogEntry| entry.name == name)
        {
            continue;
        }
        entries.push(CatalogEntry {
            name: name.into(),
            provider: provider.into(),
            description: item
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("Описание доступно в официальном каталоге.")
                .chars()
                .take(1200)
                .collect(),
            category: item
                .get("category")
                .and_then(Value::as_str)
                .unwrap_or("Other")
                .chars()
                .take(80)
                .collect(),
            source_url: source_url(provider)?.into(),
        });
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}
async fn load(client: &reqwest::Client, provider: &str, url: &str) -> Result<Vec<CatalogEntry>> {
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|_| CoreError::Invalid("Каталог недоступен по сети".into()))?
        .error_for_status()
        .map_err(|_| CoreError::Invalid("Не удалось загрузить каталог".into()))?;
    if response
        .content_length()
        .is_some_and(|size| size > MAX_BYTES as u64)
    {
        return Err(CoreError::Invalid("Слишком большой каталог".into()));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| CoreError::Invalid("Загрузка каталога прервана".into()))?
    {
        if bytes.len() + chunk.len() > MAX_BYTES {
            return Err(CoreError::Invalid("Слишком большой каталог".into()));
        }
        bytes.extend_from_slice(&chunk);
    }
    parse(provider, &bytes)
}
pub async fn fetch() -> Result<Catalog> {
    let client = reqwest::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(15))
        .user_agent("BebekonCode public catalog")
        .build()
        .map_err(|_| CoreError::Invalid("Не удалось открыть каталог".into()))?;
    let (a, b) = tokio::join!(
        load(&client, SOURCES[0].0, SOURCES[0].1),
        load(&client, SOURCES[1].0, SOURCES[1].1)
    );
    let mut catalog = Catalog {
        entries: vec![],
        errors: vec![],
        checked_at: now(),
    };
    for (provider, result) in [("OpenAI", a), ("Claude", b)] {
        match result {
            Ok(entries) => catalog.entries.extend(entries),
            Err(_) => catalog.errors.push(format!(
                "Каталог {provider} сейчас недоступен. Обновите позже."
            )),
        }
    }
    Ok(catalog)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn public_catalog_does_not_trust_urls_commands_or_duplicate_entries() {
        let entries = parse("openai", br#"{"plugins":[{"name":"github","homepage":"javascript:bad","command":"bad"},{"name":"../escape"},{"name":"github"}]}"#).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].source_url, "https://github.com/openai/plugins");
        assert!(source_url("https://evil.example").is_err());
        assert!(parse("anthropic", br#"{}"#).is_err());
    }
}
