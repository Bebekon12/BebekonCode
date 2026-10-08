//! Read only the documented built-in /usage result, never model-generated guesses.
use crate::model::UsageWindow;
use serde_json::Value;

pub(super) fn parse(value: &Value) -> Option<(Vec<UsageWindow>, String)> {
    if value["local_command"].as_str() != Some("usage")
        || value["num_turns"].as_u64() != Some(0)
        || value["duration_api_ms"].as_u64() != Some(0)
        || value["is_error"].as_bool() == Some(true)
        || !value["modelUsage"]
            .as_object()
            .is_some_and(|models| models.is_empty())
    {
        return None;
    }
    let text = value["result"].as_str()?;
    if text.len() > 32_000 {
        return None;
    }
    let ansi = regex::Regex::new(r"\x1b\[[0-9;]*[A-Za-z]").expect("static regex");
    let text = ansi.replace_all(text, "");
    let percent =
        regex::Regex::new(r"(?i)(\d+(?:\.\d+)?)\s*%\s*(used|remaining)").expect("static regex");
    let mut window = None;
    let mut distance = 0;
    let mut windows = Vec::new();
    for line in text.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.contains("current session") {
            window = Some(300);
            distance = 0;
        } else if lower.contains("current week")
            && (lower.contains("all models") || !lower.contains('('))
        {
            window = Some(10080);
            distance = 0;
        } else if lower.contains("current week") || lower.contains("extra usage") {
            window = None;
        }
        distance += 1;
        if distance > 5 {
            window = None;
        }
        if let (Some(minutes), Some(capture)) = (window, percent.captures(line)) {
            let amount = capture[1].parse::<f64>().ok()?;
            if !(0.0..=100.0).contains(&amount) {
                return None;
            }
            if !windows
                .iter()
                .any(|w: &UsageWindow| w.window_minutes == Some(minutes))
            {
                windows.push(UsageWindow {
                    window_minutes: Some(minutes),
                    used_percent: if capture[2].eq_ignore_ascii_case("remaining") {
                        100.0 - amount
                    } else {
                        amount
                    },
                    resets_at: None,
                    source: "claude-cli-usage".into(),
                });
            }
            window = None;
        }
    }
    Some((
        windows,
        crate::redaction::redact(&text).chars().take(2000).collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn quota_is_only_from_a_zero_inference_local_command() {
        let mut value = json!({"local_command":"usage","num_turns":0,"duration_api_ms":0,"modelUsage":{},"result":"Current session\n 24% used\nCurrent week (all models)\n 65% remaining\nCurrent week (Opus only)\n99% used"});
        let (windows, _) = parse(&value).unwrap();
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].used_percent, 24.0);
        assert_eq!(windows[1].used_percent, 35.0);
        value["num_turns"] = json!(1);
        assert!(parse(&value).is_none());
        value["num_turns"] = json!(0);
        value["result"] = json!("Total cost: $0\nUsage: 0 input");
        assert!(parse(&value).unwrap().0.is_empty());
    }
}
