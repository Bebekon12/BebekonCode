//! Official SDK usage control, without inference or access to credentials.
use super::{capture, failure, ClaudeProvider};
use crate::{model::*, process, Result};
use serde_json::Value;
use std::{ffi::OsString, path::PathBuf, process::Command, time::Duration};

const HELPER: &str = include_str!(concat!(env!("OUT_DIR"), "/claude-usage.mjs"));
const SOURCE: &str = "claude-sdk-experimental";

// No credentials are written here; the official SDK is embedded in the Rust executable.
struct HelperFile(PathBuf);
impl Drop for HelperFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
        if let Some(parent) = self.0.parent() {
            let _ = std::fs::remove_dir(parent);
        }
    }
}

pub(super) async fn read(
    provider: &ClaudeProvider,
    account: &AccountProfile,
    status: &mut AccountStatus,
) -> Result<()> {
    let node = which::which("node").map_err(|_| {
        failure("Для экспериментального чтения лимитов Claude установите Node.js 18+ и обновите лимиты.", None)
    })?;
    let detection = provider.ready()?;
    let binary = detection
        .binary
        .ok_or(crate::error::CoreError::ProviderUnavailable)?;
    let profile = account
        .config_dir
        .as_deref()
        .ok_or(crate::error::CoreError::ProviderUnavailable)?;
    let directory = std::env::temp_dir().join(format!("bebekon-usage-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&directory)?;
    let helper = HelperFile(directory.join("usage.mjs"));
    std::fs::write(&helper.0, HELPER)?;
    let mut command = Command::new(node);
    command
        .arg(&helper.0)
        .arg(binary)
        .arg(profile)
        .current_dir(profile);
    process::harden(
        &mut command,
        process::child_env(&[
            ("CLAUDE_CONFIG_DIR", OsString::from(profile)),
            ("BEBEKON_USAGE_HELPER", OsString::from("1")),
            ("DISABLE_TELEMETRY", OsString::from("1")),
            ("DISABLE_ERROR_REPORTING", OsString::from("1")),
            // NONESSENTIAL_TRAFFIC also blocks the explicitly requested usage read.
            // Keep telemetry, error reporting and updates separately disabled instead.
            ("DISABLE_AUTOUPDATER", OsString::from("1")),
        ]),
    );
    let unavailable = || {
        failure("Экспериментальный Claude SDK не вернул лимиты. Нужны Node.js 18+ и совместимый Claude Code; обновите CLI и повторите проверку.", None)
    };
    let (success, bytes) = capture(command, Duration::from_secs(20))
        .await
        .map_err(|_| unavailable())?;
    if !success {
        return Err(unavailable());
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| unavailable())?;
    apply(&value, status).ok_or_else(unavailable)?;
    status.checked_at = now();
    if status.usage.is_empty() {
        status.usage_error =
            Some("Claude SDK не предоставил проценты лимитов для этой подписки.".into());
    }
    Ok(())
}

fn apply(value: &Value, status: &mut AccountStatus) -> Option<()> {
    let session = value.get("session")?;
    if session["total_cost_usd"].as_f64() != Some(0.0)
        || session["total_api_duration_ms"].as_f64() != Some(0.0)
        || !session["model_usage"]
            .as_object()
            .is_some_and(|models| models.is_empty())
    {
        return None;
    }
    let available = value["rate_limits_available"].as_bool()?;
    if let Some(plan) = value["subscription_type"]
        .as_str()
        .filter(|s| s.len() <= 100)
    {
        status.plan = Some(crate::redaction::redact(plan));
    }
    if !available {
        return Some(());
    }
    let limits = &value["rate_limits"];
    for (key, minutes, label) in [
        ("five_hour", 300, None),
        ("seven_day", 10080, None),
        (
            "seven_day_oauth_apps",
            10080,
            Some("Неделя · OAuth-приложения"),
        ),
        ("seven_day_opus", 10080, Some("Неделя · Opus")),
        ("seven_day_sonnet", 10080, Some("Неделя · Sonnet")),
    ] {
        if let Some(window) = window(&limits[key], minutes, label.map(str::to_string)) {
            status.usage.push(window);
        }
    }
    for model in limits["model_scoped"]
        .as_array()
        .into_iter()
        .flatten()
        .take(30)
    {
        let Some(label) = model["display_name"]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 100)
        else {
            continue;
        };
        if let Some(window) = window(
            model,
            10080,
            Some(format!("Неделя · {}", crate::redaction::redact(label))),
        ) {
            status.usage.push(window);
        }
    }
    status.limit_reached = status
        .usage
        .iter()
        .any(|w| w.used_percent >= 100.0)
        .then(|| "usage_limit".into());
    Some(())
}

fn window(value: &Value, minutes: i64, label: Option<String>) -> Option<UsageWindow> {
    let used = value["utilization"].as_f64()?;
    if !(0.0..=100.0).contains(&used) {
        return None;
    }
    Some(UsageWindow {
        label,
        window_minutes: Some(minutes),
        used_percent: used,
        resets_at: value["resets_at"]
            .as_str()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|t| t.timestamp()),
        source: SOURCE.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn snapshot() -> Value {
        json!({"session":{"total_cost_usd":0,"total_api_duration_ms":0,"model_usage":{}},
        "subscription_type":"pro","rate_limits_available":true,"rate_limits":{
            "five_hour":{"utilization":24.5,"resets_at":"2026-10-10T00:00:00Z"},
            "seven_day":{"utilization":65,"resets_at":null},
            "seven_day_opus":{"utilization":99,"resets_at":"invalid"},
            "model_scoped":[{"display_name":"Model A","utilization":17,"resets_at":null}]
        }})
    }
    #[test]
    fn uses_only_provider_quota_and_preserves_reset_and_model_labels() {
        let mut status = AccountStatus::default();
        apply(&snapshot(), &mut status).unwrap();
        assert_eq!(status.plan.as_deref(), Some("pro"));
        assert_eq!(status.usage.len(), 4);
        assert_eq!(status.usage[0].used_percent, 24.5);
        assert_eq!(status.usage[0].resets_at, Some(1791590400));
        assert_eq!(status.usage[2].resets_at, None);
        assert_eq!(status.usage[2].label.as_deref(), Some("Неделя · Opus"));
        assert_eq!(status.usage[3].label.as_deref(), Some("Неделя · Model A"));
    }
    #[test]
    fn never_infers_missing_or_invalid_percentages() {
        let mut value = snapshot();
        value["rate_limits"] =
            json!({"five_hour":{"utilization":null},"seven_day":{"utilization":101}});
        let mut status = AccountStatus::default();
        apply(&value, &mut status).unwrap();
        assert!(status.usage.is_empty());
        value["rate_limits_available"] = json!(false);
        value["rate_limits"] = snapshot()["rate_limits"].clone();
        apply(&value, &mut status).unwrap();
        assert!(status.usage.is_empty());
    }
    #[test]
    fn rejects_model_inference_and_changed_response_shapes() {
        for field in ["total_cost_usd", "total_api_duration_ms"] {
            let mut value = snapshot();
            value["session"][field] = json!(1);
            assert!(apply(&value, &mut AccountStatus::default()).is_none());
        }
        let mut value = snapshot();
        value["session"]["model_usage"] = json!({"model":{}});
        assert!(apply(&value, &mut AccountStatus::default()).is_none());
        assert!(apply(&json!({}), &mut AccountStatus::default()).is_none());
    }
}
